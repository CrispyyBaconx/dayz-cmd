use crate::server::Server;
use anyhow::Result;
use std::process::Command;

pub const DAYZ_GAME_ID: &str = "221100";

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
        args.push(format!("-connect={}", server.endpoint.ip));
        args.push(format!("-port={}", server.game_port));

        if server.password {
            if let Some(pw) = password {
                args.push(format!("-password={pw}"));
            }
        }
    }

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
