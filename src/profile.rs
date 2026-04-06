use crate::offline::types::OfflineMissionPrefs;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub steam_login: Option<String>,
    pub player: Option<String>,
    #[serde(default)]
    pub steam_root: Option<String>,
    #[serde(default)]
    pub favorites: Vec<FavoriteServer>,
    #[serde(default)]
    pub history: Vec<HistoryEntry>,
    #[serde(default)]
    pub options: BTreeMap<String, LaunchOption>,
    #[serde(default)]
    pub offline: BTreeMap<String, OfflineMissionPrefs>,
    #[serde(default = "default_version")]
    pub version: String,
}

fn default_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FavoriteServer {
    pub name: String,
    pub ip: String,
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub name: String,
    pub ip: String,
    pub port: u16,
    pub ts: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LaunchOption {
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<serde_json::Value>,
    pub description: String,
}

impl Profile {
    pub fn load(path: &Path) -> Result<Self> {
        if path.exists() {
            let content = fs::read_to_string(path)?;
            if content.trim().is_empty() {
                return Ok(Self::default());
            }
            let profile: Profile = serde_json::from_str(&content)?;
            Ok(profile)
        } else {
            Ok(Self::default())
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        fs::write(path, json)?;
        Ok(())
    }

    pub fn add_favorite(&mut self, name: &str, ip: &str, port: u16) {
        let fav = FavoriteServer {
            name: name.to_string(),
            ip: ip.to_string(),
            port,
        };
        if !self.favorites.contains(&fav) {
            self.favorites.push(fav);
        }
    }

    pub fn remove_favorite(&mut self, ip: &str, port: u16) {
        self.favorites.retain(|f| !(f.ip == ip && f.port == port));
    }

    pub fn is_favorite(&self, ip: &str, port: u16) -> bool {
        self.favorites.iter().any(|f| f.ip == ip && f.port == port)
    }

    pub fn add_history(&mut self, name: &str, ip: &str, port: u16, limit: usize) {
        self.history.retain(|h| !(h.ip == ip && h.port == port));
        self.history.insert(
            0,
            HistoryEntry {
                name: name.to_string(),
                ip: ip.to_string(),
                port,
                ts: chrono::Utc::now().timestamp(),
            },
        );
        self.history.truncate(limit);
    }

    pub fn get_launch_args(&self) -> Vec<String> {
        self.options
            .iter()
            .filter(|(_, opt)| opt.enabled)
            .map(|(key, opt)| match &opt.value {
                Some(serde_json::Value::String(v)) if !v.is_empty() => format!("-{key}={v}"),
                Some(serde_json::Value::Number(n)) => format!("-{key}={n}"),
                Some(serde_json::Value::Bool(true)) => format!("-{key}=true"),
                _ => format!("-{key}"),
            })
            .collect()
    }

    pub fn offline_prefs(&self, mission_id: &str) -> Option<&OfflineMissionPrefs> {
        self.offline.get(mission_id)
    }

    pub fn toggle_option(&mut self, key: &str) -> Option<bool> {
        let option = self.options.get_mut(key)?;
        option.enabled = !option.enabled;
        Some(option.enabled)
    }

    pub fn set_option_value(&mut self, key: &str, value: &str) -> Option<()> {
        let option = self.options.get_mut(key)?;
        option.value = if value.is_empty() {
            None
        } else {
            Some(serde_json::Value::String(value.to_string()))
        };
        Some(())
    }
}

pub fn merge_legacy_profile(current: &mut Profile, legacy_path: &Path) -> Result<()> {
    let legacy = Profile::load(legacy_path)?;

    for favorite in legacy.favorites {
        if !current
            .favorites
            .iter()
            .any(|fav| fav.ip == favorite.ip && fav.port == favorite.port)
        {
            current.favorites.push(favorite);
        }
    }

    for entry in legacy.history {
        if !current
            .history
            .iter()
            .any(|item| item.ip == entry.ip && item.port == entry.port)
        {
            current.history.push(entry);
        }
    }

    let default_profile = Profile::default();
    for (key, option) in legacy.options {
        match current.options.get(&key) {
            None => {
                current.options.insert(key, option);
            }
            Some(current_option)
                if default_profile.options.get(&key) == Some(current_option)
                    && default_profile.options.get(&key) != Some(&option) =>
            {
                current.options.insert(key, option);
            }
            _ => {}
        }
    }

    if current.steam_login.is_none() {
        current.steam_login = legacy.steam_login;
    }
    if current.player.is_none() {
        current.player = legacy.player;
    }
    if current.steam_root.is_none() {
        current.steam_root = legacy.steam_root;
    }

    for (mission, prefs) in legacy.offline {
        current.offline.entry(mission).or_insert(prefs);
    }

    Ok(())
}

impl Default for Profile {
    fn default() -> Self {
        let mut options = BTreeMap::new();
        let opts = [
            ("window", false, None, "Launches in windowed mode"),
            ("noborder", false, None, "Borderless windowed mode"),
            ("nosplash", true, None, "Disables the splash on startup"),
            ("skipintro", true, None, "Disables the intro on startup"),
            (
                "filePatching",
                false,
                None,
                "Enables the game to use unpacked local data",
            ),
            ("doLogs", false, None, "Force logging"),
            ("buldozer", false, None, "Starts Buldozer mode"),
            (
                "winxp",
                false,
                None,
                "Forces the game to use Direct3D version 9 only",
            ),
            ("high", true, None, "Giving the process more priority"),
            (
                "USEALLAVAILABLECORES",
                true,
                None,
                "Use all available cores",
            ),
            (
                "useallavailablecores",
                true,
                None,
                "Use all available cores",
            ),
            ("par", false, Some(""), "Parameters file"),
            ("world", true, Some("empty"), "empty, ChernarusPlus"),
            ("profiles", false, Some(""), "Profiles path"),
            (
                "noPause",
                false,
                Some(""),
                "-1 Default, 0 Graphics Only, 1 Graphics and sounds",
            ),
            ("maxMem", false, Some(""), "Maximum RAM in megabytes"),
            ("maxVRAM", false, Some(""), "Maximum VRAM in megabytes"),
            (
                "cpuCount",
                false,
                Some(""),
                "Defines number of CPUs/cores. 2,4,6,8",
            ),
            (
                "exThreads",
                false,
                Some(""),
                "Defines the amount of extra threads. 0,1,3,5,7",
            ),
            ("noBenchmark", false, None, "Disable benchmarking"),
            ("malloc", false, Some(""), "Custom memory allocator"),
            ("scriptDebug", false, Some("false"), "Debug scripts"),
        ];

        for (key, enabled, value, desc) in opts {
            options.insert(
                key.to_string(),
                LaunchOption {
                    enabled,
                    value: value.map(|v| serde_json::Value::String(v.to_string())),
                    description: desc.to_string(),
                },
            );
        }

        Profile {
            steam_login: None,
            player: None,
            steam_root: None,
            favorites: Vec::new(),
            history: Vec::new(),
            options,
            offline: BTreeMap::new(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggles_launch_option_enabled_state() {
        let mut profile = Profile::default();

        assert_eq!(
            profile.options.get("window").map(|opt| opt.enabled),
            Some(false)
        );
        assert!(profile.toggle_option("window").is_some());
        assert_eq!(
            profile.options.get("window").map(|opt| opt.enabled),
            Some(true)
        );
    }

    #[test]
    fn updates_launch_option_value_and_launch_args() {
        let mut profile = Profile::default();

        assert!(
            profile
                .set_option_value("profiles", "/tmp/dayz-profile")
                .is_some()
        );
        assert!(profile.toggle_option("profiles").is_some());

        let args = profile.get_launch_args();
        assert!(args.iter().any(|arg| arg == "-profiles=/tmp/dayz-profile"));
    }

    #[test]
    fn history_dedupes_and_respects_limit() {
        let mut profile = Profile::default();

        profile.add_history("One", "1.1.1.1", 2302, 2);
        profile.add_history("Two", "2.2.2.2", 2302, 2);
        profile.add_history("One Again", "1.1.1.1", 2302, 2);

        assert_eq!(profile.history.len(), 2);
        assert_eq!(profile.history[0].ip, "1.1.1.1");
        assert_eq!(profile.history[1].ip, "2.2.2.2");
    }

    #[test]
    fn serializes_remembered_offline_settings() {
        let mut profile = Profile::default();
        profile.offline.insert(
            "managed:DayZCommunityOfflineMode.ChernarusPlus".into(),
            OfflineMissionPrefs {
                mod_ids: vec![1564026768],
                spawn_enabled: true,
            },
        );

        let json = serde_json::to_string(&profile).expect("serialize profile");
        let restored: Profile = serde_json::from_str(&json).expect("deserialize profile");

        assert_eq!(
            restored
                .offline
                .get("managed:DayZCommunityOfflineMode.ChernarusPlus"),
            profile
                .offline
                .get("managed:DayZCommunityOfflineMode.ChernarusPlus")
        );
    }

    #[test]
    fn offline_prefs_expose_remembered_mod_ids_and_spawn_toggle() {
        let mut profile = Profile::default();
        profile.offline.insert(
            "managed:DayZCommunityOfflineMode.Namalsk".into(),
            OfflineMissionPrefs {
                mod_ids: vec![1564026768, 2289456201],
                spawn_enabled: false,
            },
        );

        let prefs = profile
            .offline_prefs("managed:DayZCommunityOfflineMode.Namalsk")
            .expect("offline prefs");

        assert_eq!(prefs.mod_ids, vec![1564026768, 2289456201]);
        assert!(!prefs.spawn_enabled);
    }

    #[test]
    fn merges_legacy_profile_without_duplicate_favorites_or_history() {
        let temp_dir = std::env::temp_dir().join(format!(
            "dayz-cmd-profile-merge-{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().expect("timestamp")
        ));
        let legacy_path = temp_dir.join("legacy-profile.json");
        std::fs::create_dir_all(&temp_dir).expect("create temp dir");

        let mut current = Profile::default();
        current.add_favorite("Current", "1.1.1.1", 2302);
        current.add_history("Current", "1.1.1.1", 2302, 10);

        let mut legacy = Profile::default();
        legacy.add_favorite("Legacy Dupe", "1.1.1.1", 2302);
        legacy.add_favorite("Legacy New", "2.2.2.2", 2402);
        legacy.add_history("Legacy Dupe", "1.1.1.1", 2302, 10);
        legacy.add_history("Legacy New", "2.2.2.2", 2402, 10);
        legacy.save(&legacy_path).expect("save legacy profile");

        merge_legacy_profile(&mut current, &legacy_path).expect("merge legacy profile");

        assert_eq!(current.favorites.len(), 2);
        assert!(current.is_favorite("1.1.1.1", 2302));
        assert!(current.is_favorite("2.2.2.2", 2402));

        assert_eq!(current.history.len(), 2);
        assert!(current.history.iter().any(|entry| entry.ip == "1.1.1.1"));
        assert!(current.history.iter().any(|entry| entry.ip == "2.2.2.2"));
    }
}
