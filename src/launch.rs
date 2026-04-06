use crate::server::Server;
use anyhow::Result;
use std::path::Path;
use std::process::{Command, Stdio};

pub const DAYZ_GAME_ID: &str = "221100";

fn build_connect_args(ip: &str, port: u16, password: Option<&str>) -> Vec<String> {
    let mut args = vec![format!("-connect={ip}"), format!("-port={port}")];
    if let Some(pw) = password {
        args.push(format!("-password={pw}"));
    }
    args
}

fn build_mod_arg(mod_ids: &[u64]) -> Option<String> {
    if mod_ids.is_empty() {
        None
    } else {
        let mods_str: Vec<String> = mod_ids.iter().map(|id| format!("@{id}")).collect();
        Some(format!("-mod={}", mods_str.join(";")))
    }
}

pub fn build_launch_args(
    server: Option<&Server>,
    mod_ids: &[u64],
    player_name: &str,
    extra_args: &[String],
    password: Option<&str>,
) -> Vec<String> {
    let mut args = Vec::new();

    args.push("-nolauncher".to_string());
    args.push(format!("-name={player_name}"));

    if !mod_ids.is_empty() {
        let mods_str: Vec<String> = mod_ids.iter().map(|id| format!("@{id}")).collect();
        args.push(format!("-mod={}", mods_str.join(";")));
    }

    if let Some(server) = server {
        let pw = if server.password { password } else { None };
        args.extend(build_connect_args(
            &server.endpoint.ip,
            server.game_port,
            pw,
        ));
    }

    args.extend(extra_args.iter().cloned());
    args
}

pub fn build_direct_connect_args(
    ip: &str,
    port: u16,
    player_name: &str,
    extra_args: &[String],
    password: Option<&str>,
) -> Vec<String> {
    build_direct_connect_args_with_mods(ip, port, player_name, &[], extra_args, password)
}

pub fn build_direct_connect_args_with_mods(
    ip: &str,
    port: u16,
    player_name: &str,
    mod_ids: &[u64],
    extra_args: &[String],
    password: Option<&str>,
) -> Vec<String> {
    let mut args = vec!["-nolauncher".to_string(), format!("-name={player_name}")];
    if let Some(mod_arg) = build_mod_arg(&mod_ids) {
        args.push(mod_arg);
    }
    args.extend(build_connect_args(ip, port, password));
    args.extend(extra_args.iter().cloned());
    args
}

pub fn build_direct_connect_args_with_selected_mod_ids(
    ip: &str,
    port: u16,
    player_name: &str,
    selected_mod_ids: &[u64],
    extra_args: &[String],
    password: Option<&str>,
) -> Vec<String> {
    build_direct_connect_args_with_mods(
        ip,
        port,
        player_name,
        selected_mod_ids,
        extra_args,
        password,
    )
}

pub fn apply_offline_spawn_setting(
    dayz_path: &Path,
    _mission_id: &str,
    runtime_name: &str,
    spawn_enabled: Option<bool>,
) -> Result<()> {
    let Some(spawn_enabled) = spawn_enabled else {
        return Ok(());
    };

    let runtime_target = crate::offline::sync::runtime_target_name(runtime_name);
    crate::offline::launch::set_hive_enabled(dayz_path, &runtime_target, spawn_enabled)
}

pub fn build_offline_launch_args(
    _mission_id: &str,
    runtime_name: &str,
    mod_ids: &[u64],
    player_name: &str,
    extra_args: &[String],
) -> Vec<String> {
    let runtime_target = crate::offline::sync::runtime_target_name(runtime_name);
    let mod_ids = crate::offline::launch::inject_required_mods(runtime_name, mod_ids);
    let mut args = vec![
        "-nolauncher".to_string(),
        format!("-name={player_name}"),
        "-filePatching".to_string(),
        format!("-mission=./Missions/{runtime_target}"),
    ];

    if let Some(mod_arg) = build_mod_arg(&mod_ids) {
        args.push(mod_arg);
    }

    args.push("-doLogs".to_string());
    args.push("-scriptDebug=true".to_string());
    args.extend(extra_args.iter().cloned());
    args
}

pub fn launch_dayz(args: &[String]) -> Result<()> {
    let mut cmd = Command::new("steam");
    cmd.arg("-applaunch").arg(DAYZ_GAME_ID);
    cmd.args(args);
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::null());
    cmd.stderr(Stdio::null());

    tracing::info!("Launching DayZ with args: {:?}", args);

    cmd.spawn()?;
    Ok(())
}

pub fn is_dayz_running() -> bool {
    Command::new("pgrep")
        .args(["-f", "DayZ.*exe"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn kill_dayz() -> Result<()> {
    Command::new("pkill").args(["-f", "DayZ.*exe"]).status()?;
    Ok(())
}

pub fn is_steam_running() -> bool {
    Command::new("pgrep")
        .arg("steam")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn create_desktop_entry(
    applications_dir: &std::path::Path,
    server_name: &str,
    ip: &str,
    game_port: u16,
    exe_path: &str,
) -> Result<()> {
    let filename = format!("dayz-cli-{ip}-{game_port}.desktop");
    let path = applications_dir.join(&filename);
    let content = format!(
        "[Desktop Entry]\n\
         Name=DayZ - {server_name}\n\
         Comment=Play DayZ on Steam ({server_name})\n\
         Exec={exe_path} connect {ip} {game_port}\n\
         Icon=steam_icon_{DAYZ_GAME_ID}\n\
         Terminal=true\n\
         Type=Application\n\
         Categories=Game;\n"
    );
    std::fs::write(&path, content)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&path)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&path, perms)?;
    }

    Ok(())
}

pub fn delete_desktop_entry(
    applications_dir: &std::path::Path,
    ip: &str,
    game_port: u16,
) -> Result<()> {
    let filename = format!("dayz-cli-{ip}-{game_port}.desktop");
    let path = applications_dir.join(&filename);
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

pub fn desktop_entry_exists(applications_dir: &std::path::Path, ip: &str, game_port: u16) -> bool {
    let filename = format!("dayz-cli-{ip}-{game_port}.desktop");
    applications_dir.join(&filename).exists()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::offline::sync::runtime_target_name;
    use crate::server::Server;
    use crate::server::types::ServerEndpoint;
    use std::ffi::OsString;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use std::sync::{Mutex, MutexGuard};
    use std::thread;
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

    fn sample_server() -> Server {
        Server {
            name: "Test Server".into(),
            players: 12,
            max_players: 60,
            time: "12:00".into(),
            time_acceleration: Some(4.0),
            map: "chernarusplus".into(),
            password: false,
            battleye: true,
            vac: true,
            first_person_only: false,
            shard: "public".into(),
            version: "1.0".into(),
            environment: "w".into(),
            game_port: 2302,
            endpoint: ServerEndpoint {
                ip: "1.2.3.4".into(),
                port: 27016,
            },
            mods: Vec::new(),
        }
    }

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn env_lock() -> MutexGuard<'static, ()> {
        ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    struct EnvVarGuard {
        key: &'static str,
        value: Option<OsString>,
    }

    impl EnvVarGuard {
        fn set(key: &'static str, value: impl AsRef<std::ffi::OsStr>) -> Self {
            let previous = std::env::var_os(key);
            // SAFETY: Launch tests serialize environment mutation with ENV_LOCK.
            unsafe {
                std::env::set_var(key, value);
            }
            Self {
                key,
                value: previous,
            }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            if let Some(value) = &self.value {
                // SAFETY: Launch tests serialize environment mutation with ENV_LOCK.
                unsafe {
                    std::env::set_var(self.key, value);
                }
            } else {
                // SAFETY: Launch tests serialize environment mutation with ENV_LOCK.
                unsafe {
                    std::env::remove_var(self.key);
                }
            }
        }
    }

    fn write_executable(path: &std::path::Path, content: &str) {
        fs::write(path, content).expect("write file");
        let mut perms = fs::metadata(path).expect("stat file").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms).expect("chmod file");
    }

    fn prepend_path(dir: &std::path::Path) -> EnvVarGuard {
        let current = std::env::var_os("PATH").unwrap_or_default();
        let mut combined = OsString::from(dir.as_os_str());
        if !current.is_empty() {
            combined.push(":");
            combined.push(&current);
        }
        EnvVarGuard::set("PATH", &combined)
    }

    fn wait_for_file(path: &std::path::Path) {
        for _ in 0..100 {
            if path.exists() {
                return;
            }
            thread::sleep(std::time::Duration::from_millis(10));
        }
        panic!("timed out waiting for {}", path.display());
    }

    #[test]
    fn builds_launch_args_for_known_server() {
        let server = sample_server();
        let args = build_launch_args(
            Some(&server),
            &[123, 456],
            "Survivor",
            &["-nosplash".into()],
            None,
        );

        assert!(args.contains(&"-connect=1.2.3.4".to_string()));
        assert!(args.contains(&"-port=2302".to_string()));
        assert!(args.contains(&"-mod=@123;@456".to_string()));
        assert!(args.contains(&"-name=Survivor".to_string()));
    }

    #[test]
    fn builds_launch_args_for_raw_direct_connect() {
        let args =
            build_direct_connect_args("5.6.7.8", 2402, "Survivor", &["-nosplash".into()], None);

        assert!(args.contains(&"-connect=5.6.7.8".to_string()));
        assert!(args.contains(&"-port=2402".to_string()));
        assert!(!args.iter().any(|arg| arg.starts_with("-mod=")));
        assert!(args.contains(&"-name=Survivor".to_string()));
    }

    #[test]
    fn builds_launch_args_for_direct_connect_with_mods_and_password() {
        let args = build_direct_connect_args_with_mods(
            "5.6.7.8",
            2402,
            "Survivor",
            &[111, 222],
            &["-nosplash".into()],
            Some("secret"),
        );

        assert!(args.contains(&"-connect=5.6.7.8".to_string()));
        assert!(args.contains(&"-port=2402".to_string()));
        assert!(args.contains(&"-mod=@111;@222".to_string()));
        assert!(args.contains(&"-password=secret".to_string()));
        assert!(args.contains(&"-name=Survivor".to_string()));
    }

    #[test]
    fn builds_launch_args_for_direct_connect_with_selected_mod_ids() {
        let args = build_direct_connect_args_with_selected_mod_ids(
            "5.6.7.8",
            2402,
            "Survivor",
            &[111, 222],
            &["-nosplash".into()],
            None,
        );

        assert!(args.contains(&"-connect=5.6.7.8".to_string()));
        assert!(args.contains(&"-port=2402".to_string()));
        assert!(args.contains(&"-mod=@111;@222".to_string()));
    }

    #[test]
    fn builds_offline_launch_args_from_explicit_prep_values() {
        let args = build_offline_launch_args(
            "DayZCommunityOfflineMode.ChernarusPlus",
            "DayZCommunityOfflineMode.ChernarusPlus",
            &[123, 456],
            "Survivor",
            &["-nosplash".into()],
        );

        assert!(args.contains(&"-nolauncher".to_string()));
        assert!(args.contains(&"-name=Survivor".to_string()));
        assert!(args.contains(&"-filePatching".to_string()));
        assert!(args.contains(&format!(
            "-mission=./Missions/{}",
            runtime_target_name("DayZCommunityOfflineMode.ChernarusPlus")
        )));
        assert!(args.contains(&"-mod=@123;@456".to_string()));
        assert!(args.contains(&"-doLogs".to_string()));
        assert!(args.contains(&"-scriptDebug=true".to_string()));
    }

    #[test]
    fn builds_offline_launch_args_for_namalsk_runtime_target_with_required_mods() {
        let args = build_offline_launch_args(
            "managed:DayZCommunityOfflineMode.Namalsk",
            "DayZCommunityOfflineMode.Namalsk",
            &[123],
            "Survivor",
            &[],
        );

        assert!(args.contains(&format!(
            "-mission=./Missions/{}",
            runtime_target_name("DayZCommunityOfflineMode.Namalsk")
        )));
        assert!(args.contains(&"-mod=@123;@2289456201;@2289461232".to_string()));
    }

    #[test]
    fn launch_dayz_detaches_stdio_from_the_calling_terminal() {
        let _guard = env_lock();
        let bin_dir = temp_path("launch-detach-bin");
        fs::create_dir_all(&bin_dir).expect("create bin dir");
        let args_file = temp_path("launch-detach-args");
        let fds_file = temp_path("launch-detach-fds");
        write_executable(
            &bin_dir.join("steam"),
            "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$FAKE_STEAM_ARGS\"\nfd0=$(readlink /proc/$$/fd/0)\nfd1=$(readlink /proc/$$/fd/1)\nfd2=$(readlink /proc/$$/fd/2)\nprintf '%s\\n%s\\n%s\\n' \"$fd0\" \"$fd1\" \"$fd2\" > \"$FAKE_STEAM_FDS\"\n",
        );
        let path_env = prepend_path(&bin_dir);
        let _args_env = EnvVarGuard::set("FAKE_STEAM_ARGS", &args_file);
        let _fds_env = EnvVarGuard::set("FAKE_STEAM_FDS", &fds_file);

        launch_dayz(&["-connect=1.2.3.4".into()]).expect("launch dayz");

        wait_for_file(&args_file);
        wait_for_file(&fds_file);

        let fd_targets = fs::read_to_string(&fds_file).expect("read fd targets");
        assert_eq!(
            fd_targets.lines().collect::<Vec<_>>(),
            vec!["/dev/null", "/dev/null", "/dev/null"]
        );

        drop(path_env);
        fs::remove_file(args_file).expect("remove args file");
        fs::remove_file(fds_file).expect("remove fds file");
        fs::remove_dir_all(bin_dir).expect("remove bin dir");
    }

    #[test]
    fn offline_spawn_setting_toggles_mission_client_file() {
        let root = temp_path("offline-spawn");
        let runtime_name = "DayZCommunityOfflineMode.ChernarusPlus";
        let runtime_target = runtime_target_name(runtime_name);
        let client_file = root
            .join("Missions")
            .join(&runtime_target)
            .join("core")
            .join("CommunityOfflineClient.c");
        fs::create_dir_all(client_file.parent().expect("client parent")).expect("create dirs");
        fs::write(&client_file, "bool HIVE_ENABLED = false;\n").expect("write client file");

        apply_offline_spawn_setting(
            &root,
            "DayZCommunityOfflineMode.ChernarusPlus",
            runtime_name,
            Some(true),
        )
        .expect("toggle spawn");

        let content = fs::read_to_string(&client_file).expect("read client file");
        assert!(content.contains("HIVE_ENABLED = true"));

        fs::remove_dir_all(root).expect("remove temp root");
    }

    #[test]
    fn offline_spawn_setting_succeeds_when_state_is_already_correct() {
        let root = temp_path("offline-spawn-idempotent");
        let runtime_name = "DayZCommunityOfflineMode.ChernarusPlus";
        let runtime_target = runtime_target_name(runtime_name);
        let client_file = root
            .join("Missions")
            .join(&runtime_target)
            .join("core")
            .join("CommunityOfflineClient.c");
        fs::create_dir_all(client_file.parent().expect("client parent")).expect("create dirs");
        fs::write(&client_file, "bool HIVE_ENABLED = true;\n").expect("write client file");

        apply_offline_spawn_setting(
            &root,
            "DayZCommunityOfflineMode.ChernarusPlus",
            runtime_name,
            Some(true),
        )
        .expect("already-correct state should succeed");

        assert_eq!(
            fs::read_to_string(&client_file).expect("read client file"),
            "bool HIVE_ENABLED = true;\n"
        );

        fs::remove_dir_all(root).expect("remove temp root");
    }

    #[test]
    fn offline_spawn_setting_errors_when_marker_is_missing() {
        let root = temp_path("offline-spawn-missing-marker");
        let runtime_name = "DayZCommunityOfflineMode.ChernarusPlus";
        let runtime_target = runtime_target_name(runtime_name);
        let client_file = root
            .join("Missions")
            .join(&runtime_target)
            .join("core")
            .join("CommunityOfflineClient.c");
        fs::create_dir_all(client_file.parent().expect("client parent")).expect("create dirs");
        fs::write(&client_file, "bool SOME_OTHER_FLAG = false;\n").expect("write client file");

        let err = apply_offline_spawn_setting(
            &root,
            "DayZCommunityOfflineMode.ChernarusPlus",
            runtime_name,
            Some(true),
        )
        .expect_err("marker mismatch should fail");

        assert!(
            err.to_string()
                .contains("offline mission spawn toggle marker not found")
        );

        fs::remove_dir_all(root).expect("remove temp root");
    }

    #[test]
    fn offline_spawn_setting_targets_the_runtime_assignment_line_even_with_comments() {
        let root = temp_path("offline-spawn-comment");
        let runtime_name = "DayZCommunityOfflineMode.ChernarusPlus";
        let runtime_target = runtime_target_name(runtime_name);
        let client_file = root
            .join("Missions")
            .join(&runtime_target)
            .join("core")
            .join("CommunityOfflineClient.c");
        fs::create_dir_all(client_file.parent().expect("client parent")).expect("create dirs");
        fs::write(
            &client_file,
            "// HIVE_ENABLED = true; comment should be ignored\nbool HIVE_ENABLED = false;\n",
        )
        .expect("write client file");

        apply_offline_spawn_setting(&root, "existing:mission-identity", runtime_name, Some(true))
            .expect("toggle spawn");

        assert_eq!(
            fs::read_to_string(&client_file).expect("read client file"),
            "// HIVE_ENABLED = true; comment should be ignored\nbool HIVE_ENABLED = true;\n"
        );

        fs::remove_dir_all(root).expect("remove temp root");
    }

    #[test]
    fn desktop_entry_round_trip() {
        let applications_dir = temp_path("desktop-entry");
        fs::create_dir_all(&applications_dir).expect("create applications dir");

        create_desktop_entry(
            &applications_dir,
            "Test Server",
            "1.2.3.4",
            2302,
            "/usr/bin/dayz-cmd",
        )
        .expect("create desktop entry");

        assert!(desktop_entry_exists(&applications_dir, "1.2.3.4", 2302));

        delete_desktop_entry(&applications_dir, "1.2.3.4", 2302).expect("delete desktop entry");
        assert!(!desktop_entry_exists(&applications_dir, "1.2.3.4", 2302));

        fs::remove_dir_all(&applications_dir).expect("remove applications dir");
    }
}
