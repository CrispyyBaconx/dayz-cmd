use crate::config::Config;
use crate::offline::types::{MissionSource, OfflineState};
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

pub fn offline_root(config: &Config) -> PathBuf {
    config.offline_root()
}

pub fn offline_state_path(config: &Config) -> PathBuf {
    offline_root(config).join("state.json")
}

pub fn load_offline_state(config: &Config) -> Result<OfflineState> {
    let path = offline_state_path(config);
    if !path.exists() {
        return Ok(OfflineState::default());
    }

    let content = fs::read_to_string(&path).context("read offline state")?;
    if content.trim().is_empty() {
        return Ok(OfflineState::default());
    }

    let state = serde_json::from_str(&content).context("parse offline state")?;
    Ok(state)
}

pub fn save_offline_state(config: &Config, state: &OfflineState) -> Result<()> {
    let path = offline_state_path(config);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).context("create offline state dir")?;
    }

    let json = serde_json::to_string_pretty(state).context("serialize offline state")?;
    fs::write(path, json).context("write offline state")?;
    Ok(())
}

pub fn mission_identity_key(
    source: MissionSource,
    mission: &str,
    source_path: Option<&Path>,
) -> Result<String> {
    match source {
        MissionSource::Managed => Ok(format!("managed:{mission}")),
        MissionSource::Existing => source_path
            .map(|path| format!("existing:{}", canonical_path_hash(path)))
            .context("existing mission identity requires a source path"),
    }
}

fn canonical_path_hash(path: &Path) -> String {
    let canonical = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let normalized = canonical.to_string_lossy();
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in normalized.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::offline::types::{MissionSource, OfflineState};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn round_trips_managed_offline_state() {
        let root = test_root("offline-state");
        fs::create_dir_all(&root).expect("create temp root");
        let config = test_config(&root);
        let state = OfflineState {
            installed_tag: Some("0.4.0".into()),
            latest_known_tag: Some("0.5.0".into()),
            managed_missions: vec![
                "DayZCommunityOfflineMode.ChernarusPlus".into(),
                "DayZCommunityOfflineMode.Enoch".into(),
            ],
            last_check_ts: Some(1_717_000_000),
        };

        save_offline_state(&config, &state).expect("save state");
        let loaded = load_offline_state(&config).expect("load state");

        assert_eq!(loaded, state);
        assert!(offline_state_path(&config).exists());
    }

    #[test]
    fn mission_identity_keys_are_stable_and_path_derived_for_existing_missions() {
        let root = test_root("mission-identity");
        let mission_dir = root.join("DayZ/Missions/CommunityOfflineClient");
        fs::create_dir_all(&mission_dir).expect("create mission dir");
        let canonical = mission_dir
            .canonicalize()
            .expect("canonicalize mission dir");

        assert_eq!(
            mission_identity_key(
                MissionSource::Managed,
                "DayZCommunityOfflineMode.ChernarusPlus",
                None
            )
            .expect("managed identity"),
            "managed:DayZCommunityOfflineMode.ChernarusPlus"
        );

        let first = mission_identity_key(
            MissionSource::Existing,
            "CommunityOfflineClient",
            Some(&mission_dir),
        )
        .expect("existing identity");
        let second = mission_identity_key(
            MissionSource::Existing,
            "CommunityOfflineClient",
            Some(&canonical),
        )
        .expect("existing identity");

        assert_eq!(first, second);
        assert!(first.starts_with("existing:"));
    }

    #[test]
    fn existing_mission_identity_requires_a_real_source_path() {
        let err = mission_identity_key(MissionSource::Existing, "CommunityOfflineClient", None)
            .expect_err("missing source path");
        assert!(err
            .to_string()
            .contains("existing mission identity requires a source path"));
    }

    fn test_root(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("dayz-cmd-{name}-{}-{unique}", std::process::id()))
    }

    fn test_config(root: &Path) -> Config {
        Config {
            path: root.join("dayz-cmd.conf"),
            data_dir: root.to_path_buf(),
            server_db_path: root.join("servers.json"),
            news_db_path: root.join("news.json"),
            mods_db_path: root.join("mods.json"),
            profile_path: root.join("profile.json"),
            api_url: "https://example.test/api".into(),
            github_owner: "owner".into(),
            github_repo: "repo".into(),
            request_timeout: 1,
            server_request_timeout: 1,
            server_db_ttl: 1,
            news_db_ttl: 1,
            history_size: 5,
            steamcmd_enabled: true,
            filter_mod_limit: 10,
            filter_players_limit: 50,
            filter_players_slots: 60,
            applications_dir: root.join("applications"),
        }
    }
}
