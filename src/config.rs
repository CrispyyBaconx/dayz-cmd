use anyhow::Result;
use std::collections::HashMap;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

pub const DAYZ_GAME_ID: u32 = 221100;
pub const REQUIRED_MAX_MAP_COUNT: u64 = 1_048_576;
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

    pub fn offline_root(&self) -> PathBuf {
        self.data_dir.join("offline")
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

pub fn legacy_data_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    Path::new(&home).join(".local/share/dayz-ctl")
}

pub fn has_legacy_data() -> bool {
    legacy_data_dir().join("profile.json").exists()
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

pub(crate) fn max_map_count_state_from_path(path: &Path) -> Result<MaxMapCountState> {
    if !path.exists() {
        return Ok(MaxMapCountState::UnsupportedPlatform);
    }

    let contents = fs::read_to_string(path)?;
    let current = contents
        .trim()
        .parse::<u64>()
        .map_err(|error| anyhow::anyhow!("failed to parse vm.max_map_count: {error}"))?;

    if current < REQUIRED_MAX_MAP_COUNT {
        Ok(MaxMapCountState::NeedsFix(current))
    } else {
        Ok(MaxMapCountState::Ready(current))
    }
}

pub fn current_max_map_count_state() -> Result<MaxMapCountState> {
    #[cfg(target_os = "linux")]
    {
        if let Some(path) = std::env::var_os("DAYZ_MAX_MAP_COUNT_PATH") {
            return max_map_count_state_from_path(Path::new(&path));
        }

        return max_map_count_state_from_path(Path::new("/proc/sys/vm/max_map_count"));
    }

    #[cfg(not(target_os = "linux"))]
    {
        Ok(MaxMapCountState::UnsupportedPlatform)
    }
}

pub fn max_map_count_commands() -> [String; 2] {
    [
        r#"echo "vm.max_map_count=1048576" | sudo tee /etc/sysctl.d/50-dayz.conf"#.to_string(),
        "sudo sysctl -w vm.max_map_count=1048576".to_string(),
    ]
}

pub fn fix_max_map_count() -> Result<()> {
    use std::process::Command;

    for command in max_map_count_commands() {
        let status = Command::new(max_map_count_shell())
            .args(["-c", &command])
            .status()?;
        if !status.success() {
            return Err(anyhow::anyhow!("failed to run `{command}`"));
        }
    }

    Ok(())
}

fn max_map_count_shell() -> OsString {
    std::env::var_os("DAYZ_MAX_MAP_COUNT_SHELL").unwrap_or_else(|| OsString::from("sh"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaxMapCountState {
    Ready(u64),
    NeedsFix(u64),
    UnsupportedPlatform,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;
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
        assert_eq!(
            config.path.file_name().and_then(|name| name.to_str()),
            Some("dayz-cmd.conf")
        );

        drop(xdg_env);
        drop(home_env);
    }

    #[test]
    fn detects_legacy_profile_and_clears_after_rename() {
        let _guard = env_lock();
        let home =
            std::env::temp_dir().join(format!("dayz-cmd-legacy-config-{}", std::process::id()));
        let legacy_dir = home.join(".local/share/dayz-ctl");
        let legacy_profile = legacy_dir.join("profile.json");
        let migrated_profile = legacy_dir.join("profile.json.migrated");

        fs::create_dir_all(&legacy_dir).expect("create legacy dir");
        fs::write(&legacy_profile, "{}").expect("write legacy profile");

        let home_env = EnvVarGuard::set("HOME", home.as_os_str());
        assert!(has_legacy_data());

        fs::rename(&legacy_profile, &migrated_profile).expect("rename legacy profile");
        assert!(!has_legacy_data());

        drop(home_env);
    }

    #[test]
    fn parses_and_compares_vm_max_map_count_from_file_contents() {
        let _guard = env_lock();
        let root =
            std::env::temp_dir().join(format!("dayz-cmd-max-map-count-{}", std::process::id()));
        fs::create_dir_all(&root).expect("create temp root");
        let path = root.join("max_map_count");

        fs::write(&path, "1048576\n").expect("write ready value");
        assert_eq!(
            max_map_count_state_from_path(&path).expect("read ready value"),
            MaxMapCountState::Ready(REQUIRED_MAX_MAP_COUNT)
        );

        fs::write(&path, "524288\n").expect("write low value");
        assert_eq!(
            max_map_count_state_from_path(&path).expect("read low value"),
            MaxMapCountState::NeedsFix(524288)
        );

        fs::remove_file(&path).expect("remove value file");
        assert_eq!(
            max_map_count_state_from_path(&path).expect("missing file"),
            MaxMapCountState::UnsupportedPlatform
        );

        fs::remove_dir_all(root).expect("remove temp root");
    }

    #[test]
    fn builds_the_manual_vm_max_map_count_commands() {
        assert_eq!(
            max_map_count_commands(),
            [
                r#"echo "vm.max_map_count=1048576" | sudo tee /etc/sysctl.d/50-dayz.conf"#
                    .to_string(),
                "sudo sysctl -w vm.max_map_count=1048576".to_string(),
            ]
        );
    }

    #[test]
    fn fix_max_map_count_uses_the_literal_shell_commands() {
        let _guard = env_lock();
        let root = std::env::temp_dir().join(format!(
            "dayz-cmd-fix-max-map-count-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test"),
        ));
        let bin_dir = root.join("bin");
        let log_path = root.join("sh.log");
        fs::create_dir_all(&bin_dir).expect("create bin dir");
        write_executable(
            &bin_dir.join("sh"),
            "#!/bin/sh\nprintf '%s\\n' \"$@\" >> \"$FAKE_SH_LOG\"\nexit 0\n",
        );
        let log_env = EnvVarGuard::set("FAKE_SH_LOG", log_path.as_os_str());
        let shell_env =
            EnvVarGuard::set("DAYZ_MAX_MAP_COUNT_SHELL", bin_dir.join("sh").as_os_str());

        fix_max_map_count().expect("run fix commands");

        assert_eq!(
            fs::read_to_string(&log_path).expect("read shell log"),
            format!(
                "-c\n{}\n-c\n{}\n",
                max_map_count_commands()[0],
                max_map_count_commands()[1]
            )
        );

        drop(shell_env);
        drop(log_env);
        fs::remove_dir_all(root).expect("remove temp root");
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
            Self {
                key,
                value: previous,
            }
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

    fn write_executable(path: &Path, body: &str) {
        fs::write(path, body).expect("write script");
        let mut perms = fs::metadata(path).expect("script metadata").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms).expect("set script permissions");
    }
}
