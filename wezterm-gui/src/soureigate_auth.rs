//! SoureiGate authentication — runs before GUI starts.

use anyhow::Result;
use std::path::PathBuf;

/// Result of SoureiGate authentication
#[allow(dead_code)]
pub struct SoureiGateSession {
    pub categories: Vec<soureigate_core::config::Category>,
    pub ssh_key_path: Option<PathBuf>,
    pub api_url: String,
}

/// Run SoureiGate authentication flow.
/// This runs BEFORE the GUI window opens.
/// Returns None if not in API mode (local config with servers).
pub fn authenticate() -> Result<Option<SoureiGateSession>> {
    let config = soureigate_core::config::Config::load().unwrap_or_default();

    // If we already have categories from local config, no auth needed
    if !config.categories.is_empty() {
        return Ok(Some(SoureiGateSession {
            categories: config.categories.clone(),
            ssh_key_path: if config.settings.ssh_key.is_empty() {
                None
            } else {
                Some(PathBuf::from(&config.settings.ssh_key))
            },
            api_url: config.api_url().unwrap_or("").to_string(),
        }));
    }

    // Determine API URL
    let api_url = config
        .api_url()
        .map(|s| s.to_string())
        .unwrap_or_else(|| "https://gate.sourei.dev.br".to_string());

    eprintln!("======================================");
    eprintln!("   SoureiGate -- First Run Setup");
    eprintln!("======================================");
    eprintln!("  API: {}", api_url);
    eprintln!("  Authenticating via passkey...");
    eprintln!();

    // Login via passkey (opens browser)
    let client = soureigate_core::api::ApiClient::login(&api_url)
        .map_err(|e| anyhow::anyhow!("SoureiGate login failed: {}", e))?;

    eprintln!("[ok] Authenticated! Fetching servers...");

    // Fetch categories
    let categories = client
        .fetch_categories()
        .map_err(|e| anyhow::anyhow!("Failed to fetch servers: {}", e))?;

    eprintln!(
        "[ok] Found {} categories with {} servers",
        categories.len(),
        categories.iter().map(|c| c.servers.len()).sum::<usize>()
    );

    // Fetch SSH key
    let ssh_key_path = match client.fetch_and_save_ssh_key() {
        Ok(path) => {
            eprintln!("[ok] SSH key saved to {:?}", path);
            Some(path)
        }
        Err(e) => {
            eprintln!("[warn] Could not fetch SSH key: {}", e);
            None
        }
    };

    // Save API config for next time
    let mut save_config = soureigate_core::config::Config::default();
    save_config.setup_api(&api_url);
    if let Err(e) = save_config.save_to_config_dir() {
        eprintln!("[warn] Could not save config: {}", e);
    }

    eprintln!();
    eprintln!("[ok] SoureiGate ready! Opening terminal...");
    eprintln!();

    Ok(Some(SoureiGateSession {
        categories,
        ssh_key_path,
        api_url,
    }))
}
