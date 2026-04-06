use crate::offline::sync::runtime_target_name;
use crate::profile::Profile;
use anyhow::{Context, Result, bail};
use std::fs;
use std::path::{Path, PathBuf};

pub const NAMALSK_DEPENDENCY_MODS: [u64; 2] = [2289456201, 2289461232];

pub fn inject_required_mods(mission_name: &str, mod_ids: &[u64]) -> Vec<u64> {
    let mut selected = mod_ids.to_vec();
    if mission_name == "DayZCommunityOfflineMode.Namalsk" {
        for mod_id in NAMALSK_DEPENDENCY_MODS {
            if !selected.contains(&mod_id) {
                selected.push(mod_id);
            }
        }
    }
    selected
}

pub fn set_hive_enabled(dayz_root: &Path, runtime_mission_name: &str, enabled: bool) -> Result<()> {
    let client_file = runtime_client_file(dayz_root, runtime_mission_name);
    let content = fs::read_to_string(&client_file).with_context(|| {
        format!(
            "read offline mission client file for spawn toggle: {}",
            client_file.display()
        )
    })?;

    let (expected, replacement) = if enabled {
        ("HIVE_ENABLED = true;", "HIVE_ENABLED = false;")
    } else {
        ("HIVE_ENABLED = false;", "HIVE_ENABLED = true;")
    };

    if content.contains(expected) {
        return Ok(());
    }

    let updated = content.replace(replacement, expected);
    if updated == content {
        bail!(
            "offline mission spawn toggle marker not found in {}",
            client_file.display()
        );
    }

    fs::write(&client_file, updated).with_context(|| {
        format!(
            "write offline mission client file for spawn toggle: {}",
            client_file.display()
        )
    })?;

    Ok(())
}

pub fn build_offline_launch_args(
    profile: &Profile,
    mission_id: &str,
    mission_name: &str,
    extra_args: &[String],
) -> Vec<String> {
    let player_name = profile.player.as_deref().unwrap_or("Survivor");
    let mod_ids = profile
        .offline_prefs(mission_id)
        .map(|prefs| prefs.mod_ids.as_slice())
        .unwrap_or(&[]);

    build_offline_launch_args_from_ids(mission_name, mod_ids, player_name, extra_args)
}

pub fn build_offline_launch_args_from_ids(
    mission_name: &str,
    mod_ids: &[u64],
    player_name: &str,
    extra_args: &[String],
) -> Vec<String> {
    let mut args = vec![
        "-nolauncher".to_string(),
        format!("-name={player_name}"),
        "-filePatching".to_string(),
        format!("-mission=./Missions/{}", runtime_target_name(mission_name)),
    ];

    let mod_ids = inject_required_mods(mission_name, mod_ids);
    if !mod_ids.is_empty() {
        let mods_str: Vec<String> = mod_ids.iter().map(|id| format!("@{id}")).collect();
        args.push(format!("-mod={}", mods_str.join(";")));
    }

    args.push("-doLogs".to_string());
    args.push("-scriptDebug=true".to_string());
    args.extend(extra_args.iter().cloned());
    args
}

fn runtime_client_file(dayz_root: &Path, runtime_mission_name: &str) -> PathBuf {
    dayz_root
        .join("Missions")
        .join(runtime_mission_name)
        .join("core")
        .join("CommunityOfflineClient.c")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::offline::types::OfflineMissionPrefs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn forces_namalsk_dependency_mods() {
        let mods = inject_required_mods("DayZCommunityOfflineMode.Namalsk", &[123]);
        assert!(mods.contains(&123));
        assert!(mods.contains(&2289456201));
        assert!(mods.contains(&2289461232));
    }

    #[test]
    fn toggles_hive_enabled_in_the_runtime_mission_copy() {
        let root = temp_root("offline-launch-hive");
        let client_file = runtime_client_file(
            &root,
            "dayz-cmd-offline-DayZCommunityOfflineMode.ChernarusPlus",
        );
        fs::create_dir_all(client_file.parent().expect("client parent")).expect("create dirs");
        fs::write(&client_file, "bool HIVE_ENABLED = false;\n").expect("write client file");

        set_hive_enabled(
            &root,
            "dayz-cmd-offline-DayZCommunityOfflineMode.ChernarusPlus",
            true,
        )
        .expect("toggle hive");

        assert_eq!(
            fs::read_to_string(&client_file).expect("read client file"),
            "bool HIVE_ENABLED = true;\n"
        );
    }

    #[test]
    fn builds_offline_launch_args_with_runtime_mission_and_profile_launch_options() {
        let mut profile = Profile::default();
        profile.player = Some("Survivor".into());
        profile.offline.insert(
            "managed:DayZCommunityOfflineMode.ChernarusPlus".into(),
            OfflineMissionPrefs {
                mod_ids: vec![123, 456],
                spawn_enabled: true,
            },
        );

        let args = build_offline_launch_args(
            &profile,
            "managed:DayZCommunityOfflineMode.ChernarusPlus",
            "DayZCommunityOfflineMode.ChernarusPlus",
            &profile.get_launch_args(),
        );

        assert!(args.contains(&"-nolauncher".to_string()));
        assert!(args.contains(&"-name=Survivor".to_string()));
        assert!(args.contains(&"-filePatching".to_string()));
        assert!(
            args.contains(
                &"-mission=./Missions/dayz-cmd-offline-DayZCommunityOfflineMode.ChernarusPlus"
                    .to_string()
            )
        );
        assert!(args.contains(&"-mod=@123;@456".to_string()));
        assert!(args.contains(&"-doLogs".to_string()));
        assert!(args.contains(&"-scriptDebug=true".to_string()));
        assert!(args.contains(&"-nosplash".to_string()));
    }

    fn temp_root(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "dayz-cmd-{name}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time before unix epoch")
                .as_nanos()
        ))
    }
}
