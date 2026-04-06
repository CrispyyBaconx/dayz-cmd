use crate::server::ServerListResponse;
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime};

pub fn fetch_server_list(api_url: &str, timeout_secs: u64) -> Result<ServerListResponse> {
    let url = format!("{api_url}/launcher/servers/dayz");
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .user_agent(format!("dayz-ctl {}", env!("CARGO_PKG_VERSION")))
        .build()?;

    let resp: ServerListResponse = client
        .get(&url)
        .send()
        .context("Failed to fetch server list")?
        .json()
        .context("Failed to parse server list JSON")?;

    Ok(resp)
}

pub fn load_cached_servers(path: &Path, ttl_secs: u64) -> Result<Option<ServerListResponse>> {
    if !path.exists() {
        return Ok(None);
    }

    let metadata = fs::metadata(path)?;
    let modified = metadata.modified()?;
    let age = SystemTime::now()
        .duration_since(modified)
        .unwrap_or(Duration::from_secs(u64::MAX));

    if age.as_secs() > ttl_secs {
        return Ok(None);
    }

    let content = fs::read_to_string(path)?;
    let data: ServerListResponse = serde_json::from_str(&content)?;
    Ok(Some(data))
}

pub fn save_server_cache(path: &Path, data: &ServerListResponse) -> Result<()> {
    let json = serde_json::to_string(data)?;
    fs::write(path, json)?;
    Ok(())
}

pub fn fetch_players_online(timeout_secs: u64) -> Result<u64> {
    let url = "https://api.steampowered.com/ISteamUserStats/GetNumberOfCurrentPlayers/v1/?appid=221100";
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .user_agent(format!("dayz-ctl {}", env!("CARGO_PKG_VERSION")))
        .build()?;

    let resp: serde_json::Value = client.get(url).send()?.json()?;
    let count = resp["response"]["player_count"]
        .as_u64()
        .unwrap_or(0);
    Ok(count)
}
