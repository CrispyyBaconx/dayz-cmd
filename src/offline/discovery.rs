use crate::config::Config;
use crate::offline::storage::{load_offline_state, mission_identity_key};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

pub use crate::offline::types::MissionSource;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfflineMission {
    pub id: String,
    pub name: String,
    pub source: MissionSource,
    pub source_path: PathBuf,
    pub runtime_name: String,
}

pub fn discover_offline_missions(
    config: &Config,
    dayz_root: Option<&Path>,
) -> Result<Vec<OfflineMission>> {
    let state = load_offline_state(config)?;
    let mut missions = Vec::new();

    if let Some(tag) = state
        .installed_tag
        .as_deref()
        .or(state.latest_known_tag.as_deref())
    {
        let managed_root =
            crate::offline::storage::release_dir_for_tag(config, tag).join("Missions");
        missions.extend(discover_missions_at_root(
            &managed_root,
            MissionSource::Managed,
        )?);
    }

    if let Some(dayz_root) = dayz_root {
        let existing_root = dayz_root.join("Missions");
        missions.extend(discover_missions_at_root(
            &existing_root,
            MissionSource::Existing,
        )?);
    }

    disambiguate_display_names(&mut missions);
    Ok(missions)
}

fn discover_missions_at_root(root: &Path, source: MissionSource) -> Result<Vec<OfflineMission>> {
    if !root.is_dir() {
        return Ok(Vec::new());
    }

    let mut missions = Vec::new();
    for entry in fs::read_dir(root).with_context(|| {
        format!(
            "read missions root for offline discovery: {}",
            root.display()
        )
    })? {
        let entry = entry.context("read offline mission entry")?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let runtime_name = entry.file_name().to_string_lossy().into_owned();
        let id = mission_identity_key(source, &runtime_name, Some(&path))?;
        missions.push(OfflineMission {
            id,
            name: runtime_name.clone(),
            source,
            source_path: path,
            runtime_name,
        });
    }

    missions.sort_by(|left, right| left.runtime_name.cmp(&right.runtime_name));
    Ok(missions)
}

fn disambiguate_display_names(missions: &mut [OfflineMission]) {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for mission in missions.iter() {
        *counts.entry(mission.runtime_name.clone()).or_insert(0) += 1;
    }

    for mission in missions.iter_mut() {
        if counts.get(&mission.runtime_name).copied().unwrap_or(0) > 1 {
            let source_label = match mission.source {
                MissionSource::Managed => "managed",
                MissionSource::Existing => "existing",
            };
            mission.name = format!("{} ({source_label})", mission.runtime_name);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::offline::storage::{release_dir_for_tag, save_offline_state};
    use crate::offline::types::OfflineState;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn offline_discovery_discovers_managed_and_unmanaged_missions() {
        let root = test_root("discovery-merged");
        fs::create_dir_all(&root).expect("create temp root");
        let config = test_config(&root);
        install_managed_release(&config, "v1.0.0", &["Alpha", "Bravo"]);
        install_existing_missions(&root, &["Charlie"]);
        save_offline_state(
            &config,
            &OfflineState {
                installed_tag: Some("v1.0.0".into()),
                latest_known_tag: None,
                managed_missions: vec!["Alpha".into(), "Bravo".into()],
                last_check_ts: None,
            },
        )
        .expect("save state");

        let missions =
            discover_offline_missions(&config, Some(&root.join("DayZ"))).expect("discover");

        assert_eq!(missions.len(), 3);
        assert!(
            missions
                .iter()
                .any(|mission| mission.runtime_name == "Alpha")
        );
        assert!(
            missions
                .iter()
                .any(|mission| mission.runtime_name == "Bravo")
        );
        assert!(
            missions
                .iter()
                .any(|mission| mission.runtime_name == "Charlie")
        );
    }

    #[test]
    fn offline_discovery_disambiguates_duplicate_display_names_and_identity_keys() {
        let root = test_root("discovery-duplicate");
        fs::create_dir_all(&root).expect("create temp root");
        let config = test_config(&root);
        install_managed_release(&config, "v1.0.0", &["Shared"]);
        install_existing_missions(&root, &["Shared"]);
        save_offline_state(
            &config,
            &OfflineState {
                installed_tag: Some("v1.0.0".into()),
                latest_known_tag: None,
                managed_missions: vec!["Shared".into()],
                last_check_ts: None,
            },
        )
        .expect("save state");

        let missions =
            discover_offline_missions(&config, Some(&root.join("DayZ"))).expect("discover");
        let shared: Vec<_> = missions
            .into_iter()
            .filter(|mission| mission.runtime_name == "Shared")
            .collect();

        assert_eq!(shared.len(), 2);
        assert_ne!(shared[0].id, shared[1].id);
        assert_ne!(shared[0].name, shared[1].name);
        assert!(
            shared
                .iter()
                .any(|mission| mission.name.contains("managed"))
        );
        assert!(
            shared
                .iter()
                .any(|mission| mission.name.contains("existing"))
        );
    }

    fn install_managed_release(config: &Config, tag: &str, missions: &[&str]) {
        let release_dir = release_dir_for_tag(config, tag);
        for mission in missions {
            fs::create_dir_all(release_dir.join(format!("Missions/{mission}/core")))
                .expect("create managed mission");
            fs::write(
                release_dir.join(format!("Missions/{mission}/core/CommunityOfflineClient.c")),
                "HIVE_ENABLED = true;",
            )
            .expect("write managed mission");
        }
    }

    fn install_existing_missions(root: &Path, missions: &[&str]) {
        for mission in missions {
            fs::create_dir_all(root.join(format!("DayZ/Missions/{mission}/core")))
                .expect("create existing mission");
            fs::write(
                root.join(format!(
                    "DayZ/Missions/{mission}/core/CommunityOfflineClient.c"
                )),
                "HIVE_ENABLED = true;",
            )
            .expect("write existing mission");
        }
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
