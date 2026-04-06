use crate::server::Server;
use anyhow::Result;
use std::process::Command;

pub const DAYZ_GAME_ID: &str = "221100";

fn build_connect_args(ip: &str, port: u16, password: Option<&str>) -> Vec<String> {
    let mut args = vec![format!("-connect={ip}"), format!("-port={port}")];
    if let Some(pw) = password {
        args.push(format!("-password={pw}"));
    }
    args
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
        args.extend(build_connect_args(&server.endpoint.ip, server.game_port, pw));
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
    let mut args = vec!["-nolauncher".to_string(), format!("-name={player_name}")];
    args.extend(build_connect_args(ip, port, password));
    args.extend(extra_args.iter().cloned());
    args
}

pub fn launch_dayz(args: &[String]) -> Result<()> {
    let mut cmd = Command::new("steam");
    cmd.arg("-applaunch").arg(DAYZ_GAME_ID);
    cmd.args(args);

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
    Command::new("pkill")
        .args(["-f", "DayZ.*exe"])
        .status()?;
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

pub fn desktop_entry_exists(
    applications_dir: &std::path::Path,
    ip: &str,
    game_port: u16,
) -> bool {
    let filename = format!("dayz-cli-{ip}-{game_port}.desktop");
    applications_dir.join(&filename).exists()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::Server;
    use crate::server::types::ServerEndpoint;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path(prefix: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "dayz-ctl-{prefix}-{}-{}",
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

    #[test]
    fn builds_launch_args_for_known_server() {
        let server = sample_server();
        let args = build_launch_args(Some(&server), &[123, 456], "Survivor", &["-nosplash".into()], None);

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
    fn desktop_entry_round_trip() {
        let applications_dir = temp_path("desktop-entry");
        fs::create_dir_all(&applications_dir).expect("create applications dir");

        create_desktop_entry(
            &applications_dir,
            "Test Server",
            "1.2.3.4",
            2302,
            "/usr/bin/dayz-ctl",
        )
        .expect("create desktop entry");

        assert!(desktop_entry_exists(&applications_dir, "1.2.3.4", 2302));

        delete_desktop_entry(&applications_dir, "1.2.3.4", 2302).expect("delete desktop entry");
        assert!(!desktop_entry_exists(&applications_dir, "1.2.3.4", 2302));

        fs::remove_dir_all(&applications_dir).expect("remove applications dir");
    }
}
