//! SoureiGate authentication — runs before GUI starts.

use anyhow::Result;
use std::path::PathBuf;
use std::sync::OnceLock;

/// Global session storage
static SOUREIGATE_SESSION: OnceLock<SoureiGateSession> = OnceLock::new();

/// Result of SoureiGate authentication
#[derive(Clone)]
#[allow(dead_code)]
pub struct SoureiGateSession {
    pub categories: Vec<soureigate_core::config::Category>,
    pub ssh_key_path: Option<PathBuf>,
    pub api_url: String,
}

/// Get the global SoureiGate session (if authenticated)
pub fn get_session() -> Option<&'static SoureiGateSession> {
    SOUREIGATE_SESSION.get()
}

/// Run SoureiGate authentication flow.
/// This runs BEFORE the GUI window opens.
pub fn authenticate() -> Result<()> {
    let config = soureigate_core::config::Config::load().unwrap_or_default();

    // If we already have categories from local config, store and return
    if !config.categories.is_empty() {
        let session = SoureiGateSession {
            categories: config.categories.clone(),
            ssh_key_path: if config.settings.ssh_key.is_empty() {
                None
            } else {
                Some(PathBuf::from(&config.settings.ssh_key))
            },
            api_url: config.api_url().unwrap_or("").to_string(),
        };
        let _ = SOUREIGATE_SESSION.set(session);
        log::info!(
            "SoureiGate: loaded {} categories from local config",
            config.categories.len()
        );
        return Ok(());
    }

    // Determine API URL
    let api_url = config
        .api_url()
        .map(|s| s.to_string())
        .unwrap_or_else(|| "https://gate.sourei.dev.br".to_string());

    log::info!("SoureiGate: authenticating with API at {}", api_url);

    // Login via passkey (opens browser)
    let client = soureigate_core::api::ApiClient::login(&api_url)
        .map_err(|e| anyhow::anyhow!("SoureiGate login failed: {}", e))?;

    log::info!("SoureiGate: authenticated, fetching servers...");

    // Fetch categories
    let categories = client
        .fetch_categories()
        .map_err(|e| anyhow::anyhow!("Failed to fetch servers: {}", e))?;

    log::info!(
        "SoureiGate: found {} categories, {} servers total",
        categories.len(),
        categories.iter().map(|c| c.servers.len()).sum::<usize>()
    );

    // Fetch SSH key
    let ssh_key_path = match client.fetch_and_save_ssh_key() {
        Ok(path) => {
            log::info!("SoureiGate: SSH key saved to {:?}", path);
            Some(path)
        }
        Err(e) => {
            log::warn!("SoureiGate: could not fetch SSH key: {}", e);
            None
        }
    };

    // Save API config for next time
    let mut save_config = soureigate_core::config::Config::default();
    save_config.setup_api(&api_url);
    if let Err(e) = save_config.save_to_config_dir() {
        log::warn!("SoureiGate: could not save config: {}", e);
    }

    let session = SoureiGateSession {
        categories,
        ssh_key_path,
        api_url,
    };
    let _ = SOUREIGATE_SESSION.set(session);

    log::info!("SoureiGate: ready!");
    Ok(())
}

/// Generate `.soureigate.lua` config with server domains and launch menu.
/// This writes to `~/.soureigate.lua` so the config loader picks it up.
/// Must be called BEFORE `config::common_init()`.
pub fn generate_lua_config() -> Result<PathBuf> {
    let session = get_session()
        .ok_or_else(|| anyhow::anyhow!("No SoureiGate session"))?;

    let home_dir =
        dirs_next::home_dir().ok_or_else(|| anyhow::anyhow!("Cannot determine HOME dir"))?;
    let config_path = home_dir.join(".soureigate.lua");

    let total_servers: usize = session.categories.iter().map(|c| c.servers.len()).sum();

    let mut lua = String::new();
    lua.push_str("-- SoureiGate Configuration (auto-generated)\n");
    lua.push_str("-- Do not edit manually -- regenerated on login\n\n");
    lua.push_str("local wezterm = require 'wezterm'\n");
    lua.push_str("local config = wezterm.config_builder()\n\n");

    // Theme
    lua.push_str("-- Theme: Catppuccin Mocha\n");
    lua.push_str("config.color_scheme = 'Catppuccin Mocha'\n");
    lua.push_str("config.window_background_opacity = 0.95\n");
    lua.push_str("config.font_size = 13.0\n\n");

    // Window title
    lua.push_str("config.window_title = 'SoureiGate'\n\n");

    // SSH domains
    lua.push_str("-- SSH Domains (from Gate API)\n");
    lua.push_str("config.ssh_domains = {\n");

    for category in &session.categories {
        for server in &category.servers {
            let escaped_name = server.name.replace('\'', "\\'");
            lua.push_str(&format!("  {{ -- {}\n", category.name));
            lua.push_str(&format!("    name = 'sg:{}',\n", escaped_name));
            lua.push_str(&format!(
                "    remote_address = '{}:{}',\n",
                server.host, server.port
            ));
            lua.push_str(&format!("    username = '{}',\n", server.user));
            lua.push_str("    multiplexing = 'None',\n");
            lua.push_str("    no_agent_auth = true,\n");

            if let Some(ref key_path) = session.ssh_key_path {
                // Escape backslashes for Windows paths in Lua
                let escaped_path = key_path.to_string_lossy().replace('\\', "\\\\");
                lua.push_str(&format!(
                    "    ssh_option = {{ identityfile = '{}', stricthostkeychecking = 'no' }},\n",
                    escaped_path
                ));
            } else {
                lua.push_str(
                    "    ssh_option = { stricthostkeychecking = 'no' },\n",
                );
            }

            lua.push_str("  },\n");
        }
    }
    lua.push_str("}\n\n");

    // Launch menu organized by category
    lua.push_str("-- Launch Menu (Ctrl+Shift+L to open)\n");
    lua.push_str("config.launch_menu = {\n");

    for category in &session.categories {
        for server in &category.servers {
            let escaped_cat = category.name.replace('\'', "\\'");
            let escaped_name = server.name.replace('\'', "\\'");
            lua.push_str(&format!(
                "  {{ label = '[{}] {}  ({}@{}:{})', domain = {{ DomainName = 'sg:{}' }} }},\n",
                escaped_cat,
                escaped_name,
                server.user,
                server.host,
                server.port,
                escaped_name,
            ));
        }
    }
    lua.push_str("}\n\n");

    // Keybindings
    lua.push_str("-- Keybindings\n");
    lua.push_str("config.keys = {\n");
    lua.push_str(
        "  -- Ctrl+Shift+S: Show server launcher\n",
    );
    lua.push_str(
        "  { key = 'S', mods = 'CTRL|SHIFT', action = wezterm.action.ShowLauncher },\n",
    );
    lua.push_str("  -- Ctrl+Shift+R: Reload config\n");
    lua.push_str(
        "  { key = 'R', mods = 'CTRL|SHIFT', action = wezterm.action.ReloadConfiguration },\n",
    );
    lua.push_str("}\n\n");

    // Tab bar customization
    lua.push_str("-- Tab bar\n");
    lua.push_str("config.use_fancy_tab_bar = true\n");
    lua.push_str("config.tab_bar_at_bottom = false\n");
    lua.push_str("config.hide_tab_bar_if_only_one_tab = false\n\n");

    // Right status showing SoureiGate info
    lua.push_str("-- Status bar\n");
    lua.push_str("wezterm.on('update-right-status', function(window, pane)\n");
    lua.push_str(&format!(
        "  window:set_right_status('SoureiGate | {} servers ')\n",
        total_servers
    ));
    lua.push_str("end)\n\n");

    lua.push_str("return config\n");

    std::fs::write(&config_path, &lua)?;
    log::info!("SoureiGate: generated config at {:?}", config_path);

    Ok(config_path)
}
