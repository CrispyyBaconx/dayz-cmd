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

#[derive(Debug, Clone, Serialize, Deserialize)]
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
            (
                "world",
                true,
                Some("empty"),
                "empty, ChernarusPlus",
            ),
            ("profiles", false, Some(""), "Profiles path"),
            (
                "noPause",
                false,
                Some(""),
                "-1 Default, 0 Graphics Only, 1 Graphics and sounds",
            ),
            (
                "maxMem",
                false,
                Some(""),
                "Maximum RAM in megabytes",
            ),
            (
                "maxVRAM",
                false,
                Some(""),
                "Maximum VRAM in megabytes",
            ),
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
            (
                "scriptDebug",
                false,
                Some("false"),
                "Debug scripts",
            ),
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
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}
