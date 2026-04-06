#![allow(dead_code)]

mod api;
mod app;
mod config;
mod event;
mod launch;
mod mods;
mod offline;
mod profile;
mod server;
mod steam;
mod ui;
mod update;

use anyhow::Result;
use clap::{Parser, Subcommand};
use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::io;

use crate::ui::{Action, ConfirmAction, ScreenId};

#[derive(Parser)]
#[command(
    name = "dayz-cmd",
    version,
    about = "DayZ server browser and launcher for Linux"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Connect directly to a server by IP and port
    Connect {
        /// Server IP address
        ip: String,
        /// Server game port
        port: u16,
    },
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("dayz_cmd=info")
        .with_writer(io::stderr)
        .init();

    let cli = Cli::parse();
    let config = config::Config::load()?;
    let profile = profile::Profile::load(&config.profile_path)?;

    match cli.command {
        Some(Commands::Connect { ip, port }) => {
            ensure_max_map_count_ready_for_cli()?;
            run_direct_connect(config, profile, &ip, port)?;
        }
        None => {
            run_tui(config, profile)?;
        }
    }

    Ok(())
}

fn run_tui(config: config::Config, profile: profile::Profile) -> Result<()> {
    let mut app = app::App::new(config, profile);

    app.init_paths();
    app.init_steam();
    app.load_data();

    if app.profile.player.is_none() {
        app.profile.player = Some("Survivor".into());
    }

    let _ = app.profile.save(&app.config.profile_path);
    app.init_main_menu();
    let startup_gate_blocked = app.ensure_startup_max_map_count_gate()?;

    if !startup_gate_blocked && crate::config::has_legacy_data() {
        app.process_action(Action::PushScreen(ScreenId::Confirm(
            ConfirmAction::MigrateLegacy,
        )));
    }
    if !startup_gate_blocked {
        app.check_for_updates();
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let event_handler = event::EventHandler::new(250);

    while app.running {
        terminal.draw(|f| app.render(f))?;

        match event_handler.next()? {
            event::AppEvent::Key(key) => app.handle_key(key),
            event::AppEvent::Tick => app.tick(),
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    if let Some(msg) = &app.status_message {
        println!("{msg}");
    }

    Ok(())
}

fn ensure_max_map_count_ready_for_cli() -> Result<()> {
    match config::current_max_map_count_state()? {
        config::MaxMapCountState::Ready(_) | config::MaxMapCountState::UnsupportedPlatform => {
            Ok(())
        }
        config::MaxMapCountState::NeedsFix(current) => anyhow::bail!(
            "vm.max_map_count is {current}, but the launcher requires {}.\n{}\n{}",
            config::REQUIRED_MAX_MAP_COUNT,
            config::max_map_count_commands()[0],
            config::max_map_count_commands()[1],
        ),
    }
}

fn run_direct_connect(
    config: config::Config,
    mut profile: profile::Profile,
    ip: &str,
    port: u16,
) -> Result<()> {
    let mut app = app::App::new(config, profile.clone());
    app.init_paths();
    app.load_data();

    let server = app
        .servers
        .iter()
        .find(|s| s.endpoint.ip == ip && s.game_port == port)
        .cloned();

    if let Some(server) = server {
        let player = profile.player.clone().unwrap_or_else(|| "Survivor".into());
        let mod_ids: Vec<u64> = server.mods.iter().map(|m| m.steam_workshop_id).collect();
        let extra_args = profile.get_launch_args();

        if let (Some(dp), Some(wp)) = (&app.dayz_path, &app.workshop_path) {
            crate::mods::ensure_mod_symlinks(dp, wp, &mod_ids)?;
        }

        profile.add_history(
            &server.name,
            &server.endpoint.ip,
            server.endpoint.port,
            app.config.history_size,
        );
        profile.save(&app.config.profile_path)?;

        let args = launch::build_launch_args(Some(&server), &mod_ids, &player, &extra_args, None);
        println!("Connecting to {} ({}:{})...", server.name, ip, port);
        launch::launch_dayz(&args)?;
    } else {
        let player = profile.player.clone().unwrap_or_else(|| "Survivor".into());
        let extra_args = profile.get_launch_args();

        profile.add_history(&format!("{ip}:{port}"), ip, port, app.config.history_size);
        profile.save(&app.config.profile_path)?;

        let args = launch::build_direct_connect_args(ip, port, &player, &extra_args, None);
        println!("Connecting directly to {ip}:{port}...");
        launch::launch_dayz(&args)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;
    use std::ffi::OsString;
    use std::fs;
    use std::sync::{Mutex, MutexGuard};

    #[test]
    fn cli_is_named_dayz_cmd() {
        let command = Cli::command();
        assert_eq!(command.get_name(), "dayz-cmd");
    }

    #[test]
    fn connect_cli_refuses_to_bypass_a_low_vm_max_map_count_gate() {
        let _guard = env_lock();
        let root = std::env::temp_dir().join(format!(
            "dayz-cmd-main-max-map-count-{}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("create temp root");
        let path = root.join("max_map_count");
        fs::write(&path, "524288\n").expect("write low value");
        let env = EnvVarGuard::set("DAYZ_MAX_MAP_COUNT_PATH", path.as_os_str());

        let error = ensure_max_map_count_ready_for_cli().expect_err("gate should fail");

        assert!(error.to_string().contains("vm.max_map_count"));

        drop(env);
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
}
