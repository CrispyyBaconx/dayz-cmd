use crate::offline::discovery::OfflineMission;
use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeMissionSyncStatus {
    UpToDate { target_path: PathBuf },
    ConfirmationRequired { target_path: PathBuf },
    Synced { target_path: PathBuf },
}

pub fn runtime_target_name(mission_name: &str) -> String {
    format!("dayz-cmd-offline-{mission_name}")
}

pub fn sync_runtime_mission(
    dayz_root: &Path,
    mission: &OfflineMission,
    allow_overwrite: bool,
) -> Result<RuntimeMissionSyncStatus> {
    let target_path = dayz_root
        .join("Missions")
        .join(runtime_target_name(&mission.runtime_name));

    if !target_path.exists() {
        copy_dir_all(&mission.source_path, &target_path).with_context(|| {
            format!(
                "copy offline mission into runtime target: {} -> {}",
                mission.source_path.display(),
                target_path.display()
            )
        })?;
        return Ok(RuntimeMissionSyncStatus::Synced { target_path });
    }

    let source_snapshot = snapshot_directory(&mission.source_path)?;
    let target_snapshot = snapshot_directory(&target_path)?;

    if source_snapshot == target_snapshot {
        return Ok(RuntimeMissionSyncStatus::UpToDate { target_path });
    }

    if !allow_overwrite {
        return Ok(RuntimeMissionSyncStatus::ConfirmationRequired { target_path });
    }

    remove_existing_target(&target_path)?;
    copy_dir_all(&mission.source_path, &target_path).with_context(|| {
        format!(
            "overwrite offline mission runtime target: {} -> {}",
            mission.source_path.display(),
            target_path.display()
        )
    })?;

    Ok(RuntimeMissionSyncStatus::Synced { target_path })
}

fn snapshot_directory(root: &Path) -> Result<BTreeMap<PathBuf, Vec<u8>>> {
    let mut snapshot = BTreeMap::new();
    if !root.exists() {
        return Ok(snapshot);
    }

    collect_snapshot(root, root, &mut snapshot)?;
    Ok(snapshot)
}

fn collect_snapshot(
    root: &Path,
    current: &Path,
    snapshot: &mut BTreeMap<PathBuf, Vec<u8>>,
) -> Result<()> {
    for entry in fs::read_dir(current)
        .with_context(|| format!("read offline mission directory: {}", current.display()))?
    {
        let entry = entry.context("read offline mission entry")?;
        let path = entry.path();
        let relative = path
            .strip_prefix(root)
            .context("derive relative offline mission path")?
            .to_path_buf();

        if path.is_dir() {
            collect_snapshot(root, &path, snapshot)?;
            continue;
        }

        if path.is_file() {
            snapshot.insert(relative, normalized_file_bytes(&path)?);
        }
    }

    Ok(())
}

fn normalized_file_bytes(path: &Path) -> Result<Vec<u8>> {
    let bytes =
        fs::read(path).with_context(|| format!("read offline mission file: {}", path.display()))?;

    if path.file_name().and_then(|name| name.to_str()) != Some("CommunityOfflineClient.c") {
        return Ok(bytes);
    }

    let content = String::from_utf8(bytes).with_context(|| {
        format!(
            "parse offline mission client file as UTF-8: {}",
            path.display()
        )
    })?;
    Ok(normalize_hive_content(&content).into_bytes())
}

fn normalize_hive_content(content: &str) -> String {
    let mut normalized = String::new();
    for line in content.lines() {
        if line.trim_start().starts_with("HIVE_ENABLED = ") {
            normalized.push_str("HIVE_ENABLED = __launcher_normalized__;");
        } else {
            normalized.push_str(line);
        }
        normalized.push('\n');
    }
    normalized
}

fn copy_dir_all(source: &Path, target: &Path) -> Result<()> {
    fs::create_dir_all(target).with_context(|| {
        format!(
            "create offline mission runtime target directory: {}",
            target.display()
        )
    })?;

    for entry in fs::read_dir(source).with_context(|| {
        format!(
            "read offline mission source directory: {}",
            source.display()
        )
    })? {
        let entry = entry.context("read offline mission source entry")?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());

        if source_path.is_dir() {
            copy_dir_all(&source_path, &target_path)?;
        } else {
            fs::copy(&source_path, &target_path).with_context(|| {
                format!(
                    "copy offline mission file: {} -> {}",
                    source_path.display(),
                    target_path.display()
                )
            })?;
        }
    }

    Ok(())
}

fn remove_existing_target(target: &Path) -> Result<()> {
    if target.is_dir() {
        fs::remove_dir_all(target).with_context(|| {
            format!(
                "remove existing offline mission runtime target directory: {}",
                target.display()
            )
        })?;
    } else if target.exists() {
        fs::remove_file(target).with_context(|| {
            format!(
                "remove existing offline mission runtime target file: {}",
                target.display()
            )
        })?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::offline::discovery::MissionSource;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn derives_runtime_target_name_under_dayz_missions() {
        assert_eq!(
            runtime_target_name("DayZCommunityOfflineMode.ChernarusPlus"),
            "dayz-cmd-offline-DayZCommunityOfflineMode.ChernarusPlus"
        );
    }

    #[test]
    fn detects_drift_for_managed_mission_targets_before_overwrite() {
        let root = temp_root("offline-sync-drift");
        let dayz_root = root.join("DayZ");
        let source = managed_mission(&root, "DayZCommunityOfflineMode.ChernarusPlus");
        let target = runtime_target_path(&dayz_root, "DayZCommunityOfflineMode.ChernarusPlus");

        fs::create_dir_all(target.join("core")).expect("create target dir");
        fs::write(
            target.join("core/CommunityOfflineClient.c"),
            "HIVE_ENABLED = false;\n// local edit\n",
        )
        .expect("write drifted target");

        let status = sync_runtime_mission(&dayz_root, &source, false).expect("sync status");
        assert!(matches!(
            status,
            RuntimeMissionSyncStatus::ConfirmationRequired { .. }
        ));
    }

    #[test]
    fn requires_explicit_confirmation_when_managed_sync_would_replace_non_normalized_local_edits() {
        let root = temp_root("offline-sync-confirmation");
        let dayz_root = root.join("DayZ");
        let source = managed_mission(&root, "DayZCommunityOfflineMode.ChernarusPlus");
        let target = runtime_target_path(&dayz_root, "DayZCommunityOfflineMode.ChernarusPlus");

        fs::create_dir_all(target.join("core")).expect("create target dir");
        fs::write(
            target.join("core/CommunityOfflineClient.c"),
            "HIVE_ENABLED = false;\n// user changed this\n",
        )
        .expect("write local edit");

        let status = sync_runtime_mission(&dayz_root, &source, false).expect("sync status");
        assert!(matches!(
            status,
            RuntimeMissionSyncStatus::ConfirmationRequired { .. }
        ));
        assert_eq!(
            fs::read_to_string(target.join("core/CommunityOfflineClient.c"))
                .expect("read untouched target"),
            "HIVE_ENABLED = false;\n// user changed this\n"
        );
    }

    #[test]
    fn treats_only_hive_normalization_as_in_sync() {
        let root = temp_root("offline-sync-normalized");
        let dayz_root = root.join("DayZ");
        let source = managed_mission(&root, "DayZCommunityOfflineMode.ChernarusPlus");
        let target = runtime_target_path(&dayz_root, "DayZCommunityOfflineMode.ChernarusPlus");

        fs::create_dir_all(target.join("core")).expect("create target dir");
        fs::write(
            target.join("core/CommunityOfflineClient.c"),
            "HIVE_ENABLED = false;\n",
        )
        .expect("write normalized target");

        let status = sync_runtime_mission(&dayz_root, &source, false).expect("sync status");
        assert!(matches!(status, RuntimeMissionSyncStatus::UpToDate { .. }));
    }

    fn managed_mission(root: &Path, mission_name: &str) -> OfflineMission {
        let source_path = root.join(format!("managed/Missions/{mission_name}"));
        fs::create_dir_all(source_path.join("core")).expect("create source dir");
        fs::write(
            source_path.join("core/CommunityOfflineClient.c"),
            "HIVE_ENABLED = true;\n",
        )
        .expect("write source mission");
        OfflineMission {
            id: format!("managed:{mission_name}"),
            name: mission_name.to_string(),
            source: MissionSource::Managed,
            source_path,
            runtime_name: mission_name.to_string(),
        }
    }

    fn runtime_target_path(dayz_root: &Path, mission_name: &str) -> PathBuf {
        dayz_root
            .join("Missions")
            .join(runtime_target_name(mission_name))
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
