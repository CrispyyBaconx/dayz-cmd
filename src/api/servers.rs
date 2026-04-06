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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::types::{Server, ServerEndpoint};
    use std::path::PathBuf;
    use std::thread::sleep;

    fn temp_path(prefix: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "dayz-ctl-{prefix}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time before unix epoch")
                .as_nanos()
        ))
    }

    fn sample_response() -> ServerListResponse {
        ServerListResponse {
            status: 200,
            result: vec![Server {
                name: "Test Server".into(),
                players: 5,
                max_players: 60,
                time: "12:00".into(),
                time_acceleration: Some(4.0),
                map: "chernarusplus".into(),
                password: false,
                battleye: true,
                vac: true,
                first_person_only: false,
                shard: "public".into(),
                version: "1.0".into(),
                environment: "w".into(),
                game_port: 2302,
                endpoint: ServerEndpoint {
                    ip: "1.2.3.4".into(),
                    port: 27016,
                },
                mods: Vec::new(),
            }],
            players_online: Some(1234),
        }
    }

    #[test]
    fn loads_fresh_server_cache() {
        let path = temp_path("servers-cache");
        let response = sample_response();

        save_server_cache(&path, &response).expect("save server cache");
        let loaded = load_cached_servers(&path, 60).expect("load server cache");

        assert!(loaded.is_some());
        assert_eq!(loaded.expect("cache contents").players_online, Some(1234));

        fs::remove_file(path).expect("remove server cache");
    }

    #[test]
    fn expires_server_cache_after_ttl() {
        let path = temp_path("servers-cache-expired");
        let response = sample_response();

        save_server_cache(&path, &response).expect("save server cache");
        sleep(Duration::from_millis(1100));

        let loaded = load_cached_servers(&path, 0).expect("load expired cache");
        assert!(loaded.is_none());

        fs::remove_file(path).expect("remove expired cache");
    }
}
