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

/// Register SoureiGate servers as SSH domains in the Mux.
/// Call this AFTER the Mux is created.
pub fn register_ssh_domains() {
    let session = match get_session() {
        Some(s) => s,
        None => return,
    };

    let mux = mux::Mux::get();

    for category in &session.categories {
        for server in &category.servers {
            let domain_name = format!("sg:{}", server.name);

            // Check if domain already exists
            if mux.get_domain_by_name(&domain_name).is_some() {
                continue;
            }

            // Build SSH options
            let mut ssh_option = std::collections::HashMap::new();

            // Add SSH key if available
            if let Some(ref key_path) = session.ssh_key_path {
                ssh_option.insert(
                    "identityfile".to_string(),
                    key_path.to_string_lossy().to_string(),
                );
            }

            // Disable strict host key checking for managed servers
            ssh_option.insert("stricthostkeychecking".to_string(), "no".to_string());

            let ssh_dom = config::SshDomain {
                name: domain_name.clone(),
                remote_address: format!("{}:{}", server.host, server.port),
                username: Some(server.user.clone()),
                multiplexing: config::SshMultiplexing::None,
                ssh_option,
                no_agent_auth: false,
                connect_automatically: false,
                ..Default::default()
            };

            match mux::ssh::RemoteSshDomain::with_ssh_domain(&ssh_dom) {
                Ok(domain) => {
                    let domain: std::sync::Arc<dyn mux::domain::Domain> =
                        std::sync::Arc::new(domain);
                    mux.add_domain(&domain);
                    log::trace!("SoureiGate: registered domain '{}'", domain_name);
                }
                Err(e) => {
                    log::warn!(
                        "SoureiGate: failed to register domain '{}': {}",
                        domain_name,
                        e
                    );
                }
            }
        }
    }

    log::info!("SoureiGate: registered SSH domains for all servers");
}
