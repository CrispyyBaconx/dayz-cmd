use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

pub const DAYZ_GAME_ID: u32 = 221100;
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone)]
pub struct Config {
    pub path: PathBuf,
    pub data_dir: PathBuf,
    pub server_db_path: PathBuf,
    pub news_db_path: PathBuf,
    pub mods_db_path: PathBuf,
    pub profile_path: PathBuf,
    pub api_url: String,
    pub github_owner: String,
    pub github_repo: String,
    pub request_timeout: u64,
    pub server_request_timeout: u64,
    pub server_db_ttl: u64,
    pub news_db_ttl: u64,
    pub history_size: usize,
    pub steamcmd_enabled: bool,
    pub filter_mod_limit: u32,
    pub filter_players_limit: u32,
    pub filter_players_slots: u32,
    pub applications_dir: PathBuf,
}

impl Config {
    pub fn load() -> Result<Self> {
        let data_dir = dirs_data_dir();
        fs::create_dir_all(&data_dir)?;

        let config_path = data_dir.join("dayz-cmd.conf");
        let mut vars = HashMap::new();
        if config_path.exists() {
            let content = fs::read_to_string(&config_path)?;
            for line in content.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                if let Some((key, val)) = line.split_once('=') {
                    vars.insert(key.trim().to_string(), val.trim().to_string());
                }
            }
        }

        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
        let applications_dir = PathBuf::from(&home).join(".local/share/applications");

        Ok(Config {
            path: config_path,
            server_db_path: data_dir.join("servers.json"),
            news_db_path: data_dir.join("news.json"),
            mods_db_path: data_dir.join("mods.json"),
            profile_path: data_dir.join("profile.json"),
            api_url: vars
                .get("DAYZ_API")
                .cloned()
                .unwrap_or_else(|| "https://dayzsalauncher.com/api/v1".into()),
            github_owner: vars
                .get("DAYZ_GITHUB_OWNER")
                .cloned()
                .unwrap_or_else(|| "CrispyyBaconx".into()),
            github_repo: vars
                .get("DAYZ_GITHUB_REPO")
                .cloned()
                .unwrap_or_else(|| "dayz-cmd".into()),
            request_timeout: parse_or(&vars, "DAYZ_REQUEST_TIMEOUT", 10),
            server_request_timeout: parse_or(&vars, "DAYZ_SERVER_REQUEST_TIMEOUT", 30),
            server_db_ttl: parse_or(&vars, "DAYZ_SERVER_DB_TTL", 300),
            news_db_ttl: parse_or(&vars, "DAYZ_NEWS_DB_TTL", 3600),
            history_size: parse_or(&vars, "DAYZ_HISTORY_SIZE", 10),
            steamcmd_enabled: vars
                .get("DAYZ_STEAMCMD_ENABLED")
                .map(|v| v == "true")
                .unwrap_or(true),
            filter_mod_limit: parse_or(&vars, "DAYZ_FILTER_MOD_LIMIT", 10),
            filter_players_limit: parse_or(&vars, "DAYZ_FILTER_PLAYERS_LIMIT", 50),
            filter_players_slots: parse_or(&vars, "DAYZ_FILTER_PLAYERS_SLOTS", 60),
            applications_dir,
            data_dir,
        })
    }

    pub fn set_var(&mut self, key: &str, value: &str) -> Result<()> {
        let content = if self.path.exists() {
            fs::read_to_string(&self.path)?
        } else {
            String::new()
        };

        let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
        let entry = format!("{key}={value}");
        let mut found = false;
        for line in &mut lines {
            if line.starts_with(&format!("{key}=")) {
                *line = entry.clone();
                found = true;
                break;
            }
        }
        if !found {
            lines.push(entry);
        }
        fs::write(&self.path, lines.join("\n") + "\n")?;

        match key {
            "DAYZ_STEAMCMD_ENABLED" => self.steamcmd_enabled = value == "true",
            "DAYZ_API" => self.api_url = value.to_string(),
            "DAYZ_GITHUB_OWNER" => self.github_owner = value.to_string(),
            "DAYZ_GITHUB_REPO" => self.github_repo = value.to_string(),
            _ => {}
        }
        Ok(())
    }
}

fn dirs_data_dir() -> PathBuf {
    if let Some(proj_dirs) = directories::ProjectDirs::from("", "", "dayz-cmd") {
        proj_dirs.data_dir().to_path_buf()
    } else {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
        Path::new(&home).join(".local/share/dayz-cmd")
    }
}

fn parse_or<T: std::str::FromStr>(vars: &HashMap<String, String>, key: &str, default: T) -> T {
    vars.get(key)
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;
    use std::sync::{Mutex, MutexGuard};

    #[test]
    fn uses_dayz_cmd_paths() {
        let _guard = env_lock();
        let home = std::env::temp_dir().join(format!("dayz-cmd-config-{}", std::process::id()));
        let xdg_data_home = home.join(".local/share");

        fs::create_dir_all(&xdg_data_home).expect("create xdg data dir");

        let home_env = EnvVarGuard::set("HOME", home.as_os_str());
        let xdg_env = EnvVarGuard::set("XDG_DATA_HOME", xdg_data_home.as_os_str());

        let config = Config::load().expect("load config");

        assert!(config.data_dir.ends_with("dayz-cmd"));
        assert_eq!(config.path.file_name().and_then(|name| name.to_str()), Some("dayz-cmd.conf"));

        drop(xdg_env);
        drop(home_env);
    }

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn env_lock() -> MutexGuard<'static, ()> {
        ENV_LOCK.lock().expect("lock env")
    }

    struct EnvVarGuard {
        key: &'static str,
        value: Option<OsString>,
    }

    impl EnvVarGuard {
        fn set(key: &'static str, value: &std::ffi::OsStr) -> Self {
            let previous = std::env::var_os(key);
            // SAFETY: tests serialize environment access with ENV_LOCK.
            unsafe { std::env::set_var(key, value) };
            Self { key, value: previous }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            if let Some(value) = &self.value {
                // SAFETY: tests serialize environment access with ENV_LOCK.
                unsafe { std::env::set_var(self.key, value) };
            } else {
                // SAFETY: tests serialize environment access with ENV_LOCK.
                unsafe { std::env::remove_var(self.key) };
            }
        }
    }
}
