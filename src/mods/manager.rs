use super::types::{ModInfo, ModsDb};
use anyhow::Result;
use std::fs;
use std::os::unix::fs as unix_fs;
use std::path::{Path, PathBuf};

pub fn scan_installed_mods(workshop_path: &Path) -> Result<ModsDb> {
    let mut mods = Vec::new();

    if !workshop_path.exists() {
        return Ok(ModsDb {
            sum: checksum_dirs(workshop_path),
            mods,
        });
    }

    for entry in fs::read_dir(workshop_path)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let meta_path = path.join("meta.cpp");
        if !meta_path.exists() {
            continue;
        }

        if let Ok(content) = fs::read_to_string(&meta_path) {
            let name = extract_meta_field(&content, "name").unwrap_or_default();
            let id = extract_meta_field(&content, "publishedid")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            let timestamp = extract_meta_field(&content, "timestamp")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            let size = dir_size(&path).unwrap_or(0);

            if id > 0 {
                mods.push(ModInfo {
                    name,
                    id,
                    timestamp,
                    size,
                });
            }
        }
    }

    Ok(ModsDb {
        sum: checksum_dirs(workshop_path),
        mods,
    })
}

fn extract_meta_field(content: &str, field: &str) -> Option<String> {
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with(field) {
            if let Some((_key, val)) = line.split_once('=') {
                let val = val.trim().trim_end_matches(';').trim();
                let val = val.trim_matches('"');
                return Some(val.to_string());
            }
        }
    }
    None
}

fn dir_size(path: &Path) -> Result<u64> {
    let mut total = 0u64;
    if path.is_dir() {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let p = entry.path();
            if p.is_file() {
                total += entry.metadata()?.len();
            } else if p.is_dir() {
                total += dir_size(&p)?;
            }
        }
    }
    Ok(total)
}

fn checksum_dirs(workshop_path: &Path) -> String {
    let mut dirs: Vec<String> = Vec::new();
    if let Ok(entries) = fs::read_dir(workshop_path) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                if let Some(name) = entry.file_name().to_str() {
                    dirs.push(name.to_string());
                }
            }
        }
    }
    dirs.sort();
    let combined = dirs.join("/");
    format!("{:x}", md5_hash(combined.as_bytes()))
}

fn md5_hash(data: &[u8]) -> u64 {
    // Simple hash for change detection, not cryptographic
    let mut hash: u64 = 0xcbf29ce484222325;
    for &byte in data {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

pub fn load_mods_db(path: &Path) -> Result<ModsDb> {
    if path.exists() {
        let content = fs::read_to_string(path)?;
        let db: ModsDb = serde_json::from_str(&content)?;
        Ok(db)
    } else {
        Ok(ModsDb {
            sum: String::new(),
            mods: Vec::new(),
        })
    }
}

pub fn save_mods_db(path: &Path, db: &ModsDb) -> Result<()> {
    let json = serde_json::to_string_pretty(db)?;
    fs::write(path, json)?;
    Ok(())
}

pub fn ensure_mod_symlinks(dayz_path: &Path, workshop_path: &Path, mod_ids: &[u64]) -> Result<()> {
    for &id in mod_ids {
        let link_path = dayz_path.join(format!("@{id}"));
        let target = workshop_path.join(id.to_string());

        if !target.exists() {
            tracing::warn!("Workshop mod directory not found: {}", target.display());
            continue;
        }

        if link_path.exists() || link_path.is_symlink() {
            continue;
        }

        unix_fs::symlink(&target, &link_path)?;
        tracing::info!(
            "Created symlink: {} -> {}",
            link_path.display(),
            target.display()
        );
    }
    Ok(())
}

pub fn remove_mod_symlinks(dayz_path: &Path) -> Result<u32> {
    let mut count = 0;
    if let Ok(entries) = fs::read_dir(dayz_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(name) = entry.file_name().to_str() {
                if name.starts_with('@') && path.is_symlink() {
                    fs::remove_file(&path)?;
                    count += 1;
                }
            }
        }
    }
    Ok(count)
}

pub fn remove_managed_mods(workshop_path: &Path, dayz_path: &Path) -> Result<(u32, Vec<String>)> {
    let mut count = 0;
    let mut removed = Vec::new();

    if let Ok(entries) = fs::read_dir(workshop_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            let marker = path.join(".dayz-cmd");
            if marker.exists() {
                if let Some(name) = entry.file_name().to_str() {
                    removed.push(name.to_string());
                }
                fs::remove_dir_all(&path)?;
                count += 1;
            }
        }
    }

    remove_mod_symlinks(dayz_path)?;
    Ok((count, removed))
}

pub fn get_missing_mods(mods_db: &ModsDb, required_ids: &[u64]) -> Vec<u64> {
    required_ids
        .iter()
        .filter(|&&id| !mods_db.is_installed(id))
        .copied()
        .collect()
}

pub fn installed_workshop_ids(mods_db: &ModsDb) -> Vec<u64> {
    mods_db.mods.iter().map(|mod_info| mod_info.id).collect()
}

pub fn find_workshop_path(steam_root: &Path) -> PathBuf {
    steam_root.join("workshop").join("content").join("221100")
}

pub fn find_dayz_path(steam_root: &Path) -> PathBuf {
    steam_root.join("common").join("DayZ")
}

pub fn detect_steam_root() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let candidates = [
        format!("{home}/.steam/steam/steamapps"),
        format!("{home}/.local/share/Steam/steamapps"),
        format!("{home}/.var/app/com.valvesoftware.Steam/data/Steam/steamapps"),
    ];

    for path in &candidates {
        let p = PathBuf::from(path);
        if p.join("common/DayZ").exists() {
            return Some(p);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path(prefix: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "dayz-cmd-{prefix}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time before unix epoch")
                .as_nanos()
        ))
    }

    #[test]
    fn scans_installed_mods_from_meta_files() {
        let workshop_path = temp_path("mods-scan");
        let mod_dir = workshop_path.join("123456");
        fs::create_dir_all(&mod_dir).expect("create mod dir");
        fs::write(
            mod_dir.join("meta.cpp"),
            "name = \"Test Mod\";\npublishedid = 123456;\ntimestamp = 42;\n",
        )
        .expect("write meta.cpp");
        fs::write(mod_dir.join("file.txt"), "payload").expect("write payload");

        let db = scan_installed_mods(&workshop_path).expect("scan installed mods");

        assert_eq!(db.mods.len(), 1);
        assert_eq!(db.mods[0].id, 123456);
        assert_eq!(db.mods[0].name, "Test Mod");

        fs::remove_dir_all(workshop_path).expect("remove workshop dir");
    }

    #[test]
    fn installed_workshop_ids_preserve_mod_order() {
        let mods_db = ModsDb {
            sum: "checksum".into(),
            mods: vec![
                ModInfo {
                    name: "First".into(),
                    id: 1001,
                    timestamp: 0,
                    size: 0,
                },
                ModInfo {
                    name: "Second".into(),
                    id: 2002,
                    timestamp: 0,
                    size: 0,
                },
            ],
        };

        assert_eq!(installed_workshop_ids(&mods_db), vec![1001, 2002]);
    }

    #[test]
    fn creates_and_removes_mod_symlinks() {
        let root = temp_path("mods-links");
        let dayz_path = root.join("dayz");
        let workshop_path = root.join("workshop");
        fs::create_dir_all(dayz_path.as_path()).expect("create dayz path");
        fs::create_dir_all(workshop_path.join("123456")).expect("create workshop mod path");

        ensure_mod_symlinks(&dayz_path, &workshop_path, &[123456]).expect("create symlink");
        assert!(dayz_path.join("@123456").exists());

        let removed = remove_mod_symlinks(&dayz_path).expect("remove symlinks");
        assert_eq!(removed, 1);
        assert!(!dayz_path.join("@123456").exists());

        fs::remove_dir_all(root).expect("remove temp root");
    }
}
