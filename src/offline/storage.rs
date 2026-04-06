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

pub fn staging_dir_for_tag(config: &Config, tag: &str) -> PathBuf {
    offline_root(config)
        .join("tmp")
        .join(format!("install-{tag}"))
}

pub fn release_dir_for_tag(config: &Config, tag: &str) -> PathBuf {
    offline_root(config).join("releases").join(tag)
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
    let tmp_path = path.with_extension("json.tmp");
    fs::write(&tmp_path, json).context("write offline state temp file")?;
    fs::rename(&tmp_path, &path).context("promote offline state temp file")?;
    Ok(())
}

pub fn cleanup_stale_staging(config: &Config) -> Result<usize> {
    let tmp_root = offline_root(config).join("tmp");
    if !tmp_root.exists() {
        return Ok(0);
    }

    let mut removed = 0usize;
    for entry in fs::read_dir(&tmp_root).context("read offline tmp dir")? {
        let entry = entry.context("read offline tmp entry")?;
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if path.is_dir() && name.starts_with("install-") {
            fs::remove_dir_all(&path).with_context(|| {
                format!("remove stale offline staging directory: {}", path.display())
            })?;
            removed += 1;
        }
    }

    Ok(removed)
}

pub fn validate_extracted_release(staging_dir: &Path) -> Result<Vec<String>> {
    let missions_dir = staging_dir.join("Missions");
    if !missions_dir.is_dir() {
        anyhow::bail!(
            "extracted release is missing Missions directory: {}",
            missions_dir.display()
        );
    }

    let mut missions = Vec::new();
    for entry in fs::read_dir(&missions_dir).with_context(|| {
        format!(
            "read missions directory for extracted release: {}",
            missions_dir.display()
        )
    })? {
        let entry = entry.context("read extracted release mission entry")?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let mission_name = entry.file_name().to_string_lossy().into_owned();
        let client_file = path.join("core/CommunityOfflineClient.c");
        if !client_file.is_file() {
            anyhow::bail!(
                "managed mission is missing CommunityOfflineClient.c: {}",
                client_file.display()
            );
        }
        missions.push(mission_name);
    }

    if missions.is_empty() {
        anyhow::bail!(
            "extracted release does not contain any missions: {}",
            missions_dir.display()
        );
    }

    missions.sort();
    Ok(missions)
}

pub fn promote_release(config: &Config, tag: &str, staging_dir: &Path) -> Result<PathBuf> {
    let release_dir = release_dir_for_tag(config, tag);
    let release_parent = release_dir
        .parent()
        .context("release directory has no parent")?;
    fs::create_dir_all(release_parent).context("create release parent directory")?;

    let backup_dir = release_parent.join(format!(
        "{}.previous",
        release_dir
            .file_name()
            .and_then(|name| name.to_str())
            .context("release directory has no final segment")?
    ));

    if backup_dir.exists() {
        fs::remove_dir_all(&backup_dir)
            .with_context(|| format!("remove stale release backup: {}", backup_dir.display()))?;
    }

    let had_existing_release = release_dir.exists();
    if had_existing_release {
        fs::rename(&release_dir, &backup_dir).with_context(|| {
            format!(
                "move existing release aside before promotion: {} -> {}",
                release_dir.display(),
                backup_dir.display()
            )
        })?;
    }

    let promote_result = fs::rename(staging_dir, &release_dir).with_context(|| {
        format!(
            "promote staging release into place: {} -> {}",
            staging_dir.display(),
            release_dir.display()
        )
    });

    match promote_result {
        Ok(()) => {
            if had_existing_release {
                fs::remove_dir_all(&backup_dir).with_context(|| {
                    format!("remove previous release backup: {}", backup_dir.display())
                })?;
            }
            Ok(release_dir)
        }
        Err(err) => {
            if had_existing_release && backup_dir.exists() {
                let _ = fs::rename(&backup_dir, &release_dir);
            }
            Err(err)
        }
    }
}

pub fn mission_identity_key(
    source: MissionSource,
    mission: &str,
    source_path: Option<&Path>,
) -> Result<String> {
    match source {
        MissionSource::Managed => Ok(format!("managed:{mission}")),
        MissionSource::Existing => {
            let source_path =
                source_path.context("existing mission identity requires a source path")?;
            Ok(format!("existing:{}", canonical_path_hash(source_path)?))
        }
    }
}

fn canonical_path_hash(path: &Path) -> Result<String> {
    let canonical = fs::canonicalize(path).with_context(|| {
        format!(
            "existing mission identity requires a source path: {}",
            path.display()
        )
    })?;
    let normalized = canonical.to_string_lossy();
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in normalized.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    Ok(format!("{hash:016x}"))
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
    fn save_offline_state_replaces_existing_state_atomically() {
        let root = test_root("offline-state-atomic");
        fs::create_dir_all(&root).expect("create temp root");
        let config = test_config(&root);
        let initial = OfflineState {
            installed_tag: Some("0.4.0".into()),
            latest_known_tag: None,
            managed_missions: vec!["DayZCommunityOfflineMode.ChernarusPlus".into()],
            last_check_ts: Some(1),
        };
        let updated = OfflineState {
            installed_tag: Some("0.5.0".into()),
            latest_known_tag: Some("0.5.0".into()),
            managed_missions: vec!["DayZCommunityOfflineMode.Enoch".into()],
            last_check_ts: Some(2),
        };

        save_offline_state(&config, &initial).expect("save initial state");
        fs::write(offline_state_path(&config), "").expect("corrupt final state");

        save_offline_state(&config, &updated).expect("save updated state");
        let loaded = load_offline_state(&config).expect("load updated state");

        assert_eq!(loaded, updated);
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

    #[test]
    fn existing_mission_identity_requires_an_existing_path() {
        let root = test_root("mission-missing");
        let missing = root.join("DayZ/Missions/MissingMission");

        let err = mission_identity_key(MissionSource::Existing, "MissingMission", Some(&missing))
            .expect_err("missing path");
        assert!(err
            .to_string()
            .contains("existing mission identity requires a source path"));
    }

    #[test]
    fn offline_storage_cleans_stale_staging_directories() {
        let root = test_root("staging-clean");
        fs::create_dir_all(&root).expect("create temp root");
        let config = test_config(&root);
        let stale_one = staging_dir_for_tag(&config, "v1.0.0");
        let stale_two = staging_dir_for_tag(&config, "v2.0.0");
        fs::create_dir_all(&stale_one).expect("create stale staging");
        fs::create_dir_all(&stale_two).expect("create stale staging");
        fs::write(root.join("offline/tmp/keep.txt"), "keep").expect("create unrelated file");

        cleanup_stale_staging(&config).expect("cleanup staging");

        assert!(!stale_one.exists());
        assert!(!stale_two.exists());
        assert!(root.join("offline/tmp/keep.txt").exists());
    }

    #[test]
    fn offline_storage_validates_extracted_layout_before_promotion() {
        let root = test_root("validate-release");
        fs::create_dir_all(&root).expect("create temp root");
        let staging = staging_dir_for_tag(&test_config(&root), "v1.0.0");
        fs::create_dir_all(staging.join("Missions/DayZCommunityOfflineMode.ChernarusPlus/core"))
            .expect("create mission dir");
        fs::write(
            staging.join(
                "Missions/DayZCommunityOfflineMode.ChernarusPlus/core/CommunityOfflineClient.c",
            ),
            "HIVE_ENABLED = true;",
        )
        .expect("write mission file");

        let missions = validate_extracted_release(&staging).expect("validate release");

        assert_eq!(
            missions,
            vec!["DayZCommunityOfflineMode.ChernarusPlus".to_string()]
        );
    }

    #[test]
    fn offline_storage_promotes_validated_content_without_mutating_state_json() {
        let root = test_root("promote-release");
        fs::create_dir_all(&root).expect("create temp root");
        let config = test_config(&root);
        let state = OfflineState {
            installed_tag: Some("v1.0.0".into()),
            latest_known_tag: Some("v1.0.0".into()),
            managed_missions: vec!["DayZCommunityOfflineMode.ChernarusPlus".into()],
            last_check_ts: Some(1),
        };
        save_offline_state(&config, &state).expect("save state");
        let staging = staging_dir_for_tag(&config, "v2.0.0");
        fs::create_dir_all(staging.join("Missions/DayZCommunityOfflineMode.ChernarusPlus/core"))
            .expect("create staging");
        fs::write(
            staging.join(
                "Missions/DayZCommunityOfflineMode.ChernarusPlus/core/CommunityOfflineClient.c",
            ),
            "HIVE_ENABLED = true;",
        )
        .expect("write staging mission");

        promote_release(&config, "v2.0.0", &staging).expect("promote release");

        assert!(release_dir_for_tag(&config, "v2.0.0")
            .join("Missions/DayZCommunityOfflineMode.ChernarusPlus/core/CommunityOfflineClient.c")
            .exists());
        assert_eq!(load_offline_state(&config).expect("load state"), state);
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
