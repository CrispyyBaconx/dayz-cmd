use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq)]
struct ExecSpec {
    program: String,
    args: Vec<String>,
    env: Vec<(String, String)>,
}

pub fn run_installer_and_restart(installer_url: &str, version: &str) -> Result<()> {
    let current_exe = std::env::current_exe().context("Failed to resolve current executable")?;
    let installer_path = download_installer(installer_url)?;

    let installer = build_installer_exec(
        installer_path
            .to_str()
            .context("Installer path is not valid UTF-8")?,
        version,
        current_exe
            .to_str()
            .context("Executable path is not valid UTF-8")?,
    );
    run_exec(&installer).context("Installer execution failed")?;

    let restart = build_restart_exec(
        current_exe
            .to_str()
            .context("Executable path is not valid UTF-8")?,
    );
    spawn_exec(&restart).context("Failed to restart dayz-ctl")?;

    let _ = fs::remove_file(installer_path);
    Ok(())
}

fn download_installer(installer_url: &str) -> Result<PathBuf> {
    let response = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .user_agent(format!("dayz-ctl {}", env!("CARGO_PKG_VERSION")))
        .build()?
        .get(installer_url)
        .send()
        .context("Failed to download installer asset")?;

    let body = response.bytes().context("Failed to read installer asset")?;
    let path = temp_installer_path();
    fs::write(&path, body).context("Failed to write installer asset to disk")?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms)?;
    }

    Ok(path)
}

fn build_installer_exec(installer_path: &str, version: &str, restart_exe: &str) -> ExecSpec {
    ExecSpec {
        program: installer_path.to_string(),
        args: vec!["--restart-exe".to_string(), restart_exe.to_string()],
        env: vec![("DAYZ_CTL_VERSION".to_string(), version.to_string())],
    }
}

fn build_restart_exec(current_exe: &str) -> ExecSpec {
    ExecSpec {
        program: current_exe.to_string(),
        args: Vec::new(),
        env: Vec::new(),
    }
}

fn run_exec(spec: &ExecSpec) -> Result<()> {
    let mut command = Command::new(&spec.program);
    command.args(&spec.args);
    for (key, value) in &spec.env {
        command.env(key, value);
    }
    let status = command.status().with_context(|| format!("Failed to start {}", spec.program))?;
    if !status.success() {
        anyhow::bail!("{} exited with status {status}", spec.program);
    }
    Ok(())
}

fn spawn_exec(spec: &ExecSpec) -> Result<()> {
    let mut command = Command::new(&spec.program);
    command.args(&spec.args);
    for (key, value) in &spec.env {
        command.env(key, value);
    }
    command
        .spawn()
        .with_context(|| format!("Failed to spawn {}", spec.program))?;
    Ok(())
}

fn temp_installer_path() -> PathBuf {
    std::env::temp_dir().join(format!(
        "dayz-ctl-installer-{}-{}.sh",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before unix epoch")
            .as_nanos()
    ))
}

pub fn cleanup_installer(path: &Path) {
    let _ = fs::remove_file(path);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_installer_command_with_version_env() {
        let exec =
            build_installer_exec("/tmp/dayz-ctl-installer.sh", "0.4.0", "/tmp/dayz-ctl");

        assert_eq!(exec.program, "/tmp/dayz-ctl-installer.sh");
        assert_eq!(
            exec.args,
            vec!["--restart-exe".to_string(), "/tmp/dayz-ctl".to_string()]
        );
        assert_eq!(
            exec.env,
            vec![("DAYZ_CTL_VERSION".to_string(), "0.4.0".to_string())]
        );
    }

    #[test]
    fn restart_command_uses_current_exe_path() {
        let restart = build_restart_exec("/usr/bin/dayz-ctl");
        assert_eq!(restart.program, "/usr/bin/dayz-ctl");
        assert!(restart.args.is_empty());
    }

    #[test]
    fn cleanup_installer_ignores_missing_file() {
        let path = temp_installer_path();
        cleanup_installer(&path);
    }
}
