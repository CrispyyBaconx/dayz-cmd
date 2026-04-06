use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Frame;
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use std::fs;

use super::{Action, ConfirmAction, InfoScreenData, Screen, ScreenId, theme};
use crate::app::App;

pub struct ConfirmScreen {
    pub action: ConfirmAction,
    pub selected: bool,
}

impl ConfirmScreen {
    pub fn new(action: ConfirmAction) -> Self {
        Self {
            action,
            selected: false,
        }
    }

    fn message(&self) -> &str {
        match &self.action {
            ConfirmAction::Quit => "Are you sure you want to quit?",
            ConfirmAction::KillDayZ => "DayZ is already running. Kill existing process?",
            ConfirmAction::RemoveManagedMods => "Remove all launcher-managed mods?",
            ConfirmAction::RemoveModLinks => "Remove all mod symlinks?",
            ConfirmAction::UpdateModsBeforeLaunch => "Update all mods before launch?",
            ConfirmAction::MigrateLegacy => {
                "Legacy dayz-ctl config found. Migrate favorites and history?"
            }
            ConfirmAction::FixMaxMapCount => "vm.max_map_count is too low. Fix it now?",
        }
    }

    fn yes_label(&self) -> &str {
        match &self.action {
            ConfirmAction::UpdateModsBeforeLaunch => "Update",
            ConfirmAction::MigrateLegacy => "Migrate",
            ConfirmAction::FixMaxMapCount => "Fix",
            _ => "Yes",
        }
    }

    fn no_label(&self) -> &str {
        match &self.action {
            ConfirmAction::UpdateModsBeforeLaunch => "Skip",
            ConfirmAction::MigrateLegacy => "Skip",
            ConfirmAction::FixMaxMapCount => "Commands",
            _ => "No",
        }
    }
}

impl Screen for ConfirmScreen {
    fn render(&mut self, f: &mut Frame, _app: &App) {
        let area = centered_rect(50, 30, f.area());
        f.render_widget(Clear, area);

        let yes_style = if self.selected {
            theme::SELECTED
        } else {
            theme::NORMAL
        };
        let no_style = if !self.selected {
            theme::SELECTED
        } else {
            theme::NORMAL
        };

        let yes_text = format!(" {} ", self.yes_label());
        let no_text = format!(" {} ", self.no_label());
        let lines = vec![
            Line::from(""),
            Line::from(Span::styled(self.message(), theme::WARNING)),
            Line::from(""),
            Line::from(vec![
                Span::raw("    "),
                Span::styled(yes_text, yes_style),
                Span::raw("    "),
                Span::styled(no_text, no_style),
            ]),
        ];

        let para = Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(theme::WARNING)
                    .title(" Confirm "),
            )
            .wrap(Wrap { trim: true });
        f.render_widget(para, area);
    }

    fn handle_key(&mut self, key: KeyEvent, app: &mut App) -> Action {
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return Action::PopScreen;
        }

        match key.code {
            KeyCode::Esc => Action::PopScreen,
            KeyCode::Left | KeyCode::Right | KeyCode::Tab => {
                self.selected = !self.selected;
                Action::None
            }
            KeyCode::Char('y') => {
                self.selected = true;
                self.confirm(app)
            }
            KeyCode::Char('n') => self.decline(app),
            KeyCode::Enter => {
                if self.selected {
                    self.confirm(app)
                } else {
                    self.decline(app)
                }
            }
            _ => Action::None,
        }
    }
}

impl ConfirmScreen {
    fn decline(&self, app: &mut App) -> Action {
        match &self.action {
            ConfirmAction::UpdateModsBeforeLaunch => {
                app.update_mods_before_launch = false;
                Action::LaunchGame
            }
            ConfirmAction::MigrateLegacy => Action::PopScreen,
            ConfirmAction::FixMaxMapCount => Action::PushScreen(ScreenId::Info(
                InfoScreenData {
                    title: "vm.max_map_count check".into(),
                    lines: crate::config::max_map_count_commands().to_vec(),
                },
            )),
            _ => Action::PopScreen,
        }
    }

    fn confirm(&self, app: &mut App) -> Action {
        match &self.action {
            ConfirmAction::Quit => Action::Quit,
            ConfirmAction::KillDayZ => match crate::launch::kill_dayz() {
                Ok(()) => {
                    app.skip_running_check_once = true;
                    Action::LaunchGame
                }
                Err(error) => {
                    app.status_message = Some(format!("Error: {error}"));
                    Action::PopScreen
                }
            },
            ConfirmAction::RemoveManagedMods => {
                if let (Some(wp), Some(dp)) = (&app.workshop_path, &app.dayz_path) {
                    match crate::mods::remove_managed_mods(wp, dp) {
                        Ok((count, _)) => {
                            app.status_message = Some(format!("Removed {count} managed mods"));
                        }
                        Err(e) => {
                            app.status_message = Some(format!("Error: {e}"));
                        }
                    }
                }
                Action::PopScreen
            }
            ConfirmAction::RemoveModLinks => {
                if let Some(dp) = &app.dayz_path {
                    match crate::mods::remove_mod_symlinks(dp) {
                        Ok(count) => {
                            app.status_message = Some(format!("Removed {count} mod symlinks"));
                        }
                        Err(e) => {
                            app.status_message = Some(format!("Error: {e}"));
                        }
                    }
                }
                Action::PopScreen
            }
            ConfirmAction::UpdateModsBeforeLaunch => {
                app.update_mods_before_launch = true;
                Action::LaunchGame
            }
            ConfirmAction::FixMaxMapCount => match crate::config::fix_max_map_count() {
                Ok(()) => Action::Quit,
                Err(error) => {
                    app.status_message = Some(format!("Failed to fix vm.max_map_count: {error}"));
                    Action::Quit
                }
            },
            ConfirmAction::MigrateLegacy => {
                let legacy_profile = crate::config::legacy_data_dir().join("profile.json");
                let migrated_profile = legacy_profile.with_extension("json.migrated");

                match crate::profile::merge_legacy_profile(&mut app.profile, &legacy_profile)
                    .and_then(|_| app.profile.save(&app.config.profile_path))
                    .and_then(|_| {
                        fs::rename(&legacy_profile, &migrated_profile).map_err(Into::into)
                    }) {
                    Ok(()) => {
                        app.status_message = Some("Migrated legacy favorites and history".into());
                    }
                    Err(error) => {
                        app.status_message = Some(format!("Error: {error}"));
                    }
                }

                Action::PopScreen
            }
        }
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let [area] = Layout::horizontal([Constraint::Percentage(percent_x)])
        .flex(Flex::Center)
        .areas(area);
    let [area] = Layout::vertical([Constraint::Percentage(percent_y)])
        .flex(Flex::Center)
        .areas(area);
    area
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::profile::Profile;
    use crate::ui::{InfoScreenData, ScreenId};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use std::ffi::OsString;
    use std::fs;
    use std::os::unix::fs as unix_fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use std::sync::{Mutex, MutexGuard};
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

    fn test_app(dayz_path: PathBuf, workshop_path: PathBuf) -> App {
        let data_dir = temp_path("popup-data");
        let mut app = App::new(
            Config {
                path: data_dir.join("dayz-cmd.conf"),
                data_dir: data_dir.clone(),
                server_db_path: data_dir.join("servers.json"),
                news_db_path: data_dir.join("news.json"),
                mods_db_path: data_dir.join("mods.json"),
                profile_path: data_dir.join("profile.json"),
                api_url: "https://example.test".into(),
                github_owner: "example".into(),
                github_repo: "dayz-cmd".into(),
                request_timeout: 10,
                server_request_timeout: 30,
                server_db_ttl: 300,
                news_db_ttl: 3600,
                history_size: 10,
                steamcmd_enabled: true,
                filter_mod_limit: 10,
                filter_players_limit: 50,
                filter_players_slots: 60,
                applications_dir: PathBuf::from("/tmp"),
            },
            Profile::default(),
        );
        app.dayz_path = Some(dayz_path);
        app.workshop_path = Some(workshop_path);
        app
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

    fn setup_sudo_failure_script(bin_dir: &Path) {
        fs::create_dir_all(bin_dir).expect("create bin dir");
        write_executable(&bin_dir.join("sudo"), "#!/bin/sh\nexit 1\n");
    }

    fn setup_sh_success_script(bin_dir: &Path) {
        fs::create_dir_all(bin_dir).expect("create bin dir");
        write_executable(&bin_dir.join("sh"), "#!/bin/sh\nexit 0\n");
    }

    #[test]
    fn confirming_remove_mod_links_executes_action() {
        let root = temp_path("popup-remove-links");
        let dayz_path = root.join("dayz");
        let workshop_path = root.join("workshop");
        fs::create_dir_all(&dayz_path).expect("create dayz path");
        fs::create_dir_all(&workshop_path).expect("create workshop path");
        fs::create_dir_all(workshop_path.join("123")).expect("create workshop mod");
        unix_fs::symlink(workshop_path.join("123"), dayz_path.join("@123"))
            .expect("create symlink");

        let mut app = test_app(dayz_path.clone(), workshop_path);
        let mut screen = ConfirmScreen::new(ConfirmAction::RemoveModLinks);
        screen.selected = true;

        let action = screen.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &mut app);

        assert_eq!(action, Action::PopScreen);
        assert!(!dayz_path.join("@123").exists());

        fs::remove_dir_all(root).expect("remove temp root");
    }

    #[test]
    fn confirming_kill_dayz_retries_launch_flow() {
        let root = temp_path("popup-kill-dayz");
        let dayz_path = root.join("dayz");
        let workshop_path = root.join("workshop");
        fs::create_dir_all(&dayz_path).expect("create dayz path");
        fs::create_dir_all(&workshop_path).expect("create workshop path");

        let mut app = test_app(dayz_path, workshop_path);
        let screen = ConfirmScreen::new(ConfirmAction::KillDayZ);

        let action = screen.confirm(&mut app);

        assert_eq!(action, Action::LaunchGame);
        assert!(app.skip_running_check_once);

        fs::remove_dir_all(root).expect("remove temp root");
    }

    #[test]
    fn declining_max_map_count_fix_shows_commands_and_exits_via_info_screen() {
        let mut app = test_app(PathBuf::from("/tmp/dayz"), PathBuf::from("/tmp/workshop"));
        let screen = ConfirmScreen::new(ConfirmAction::FixMaxMapCount);

        let action = screen.decline(&mut app);

        assert_eq!(
            action,
            Action::PushScreen(ScreenId::Info(InfoScreenData {
                title: "vm.max_map_count check".into(),
                lines: vec![
                    r#"echo "vm.max_map_count=1048576" | sudo tee /etc/sysctl.d/50-dayz.conf"#
                        .into(),
                    "sudo sysctl -w vm.max_map_count=1048576".into(),
                ],
            }))
        );
    }

    #[test]
    fn confirming_max_map_count_fix_exits_when_the_fix_path_fails() {
        let _guard = env_lock();
        let root = temp_path("popup-max-map-count-fail");
        let bin_dir = root.join("bin");
        setup_sudo_failure_script(&bin_dir);
        let path_env = EnvVarGuard::set("PATH", bin_dir.as_os_str());

        let mut app = test_app(PathBuf::from("/tmp/dayz"), PathBuf::from("/tmp/workshop"));
        let screen = ConfirmScreen::new(ConfirmAction::FixMaxMapCount);

        let action = screen.confirm(&mut app);

        assert_eq!(action, Action::Quit);
        assert!(
            app.status_message
                .as_deref()
                .unwrap_or_default()
                .contains("Failed to fix vm.max_map_count")
        );

        drop(path_env);
        fs::remove_dir_all(root).expect("remove temp root");
    }

    #[test]
    fn confirming_max_map_count_fix_exits_after_a_successful_fix() {
        let _guard = env_lock();
        let root = temp_path("popup-max-map-count-success");
        let bin_dir = root.join("bin");
        setup_sh_success_script(&bin_dir);
        let shell_env =
            EnvVarGuard::set("DAYZ_MAX_MAP_COUNT_SHELL", bin_dir.join("sh").as_os_str());

        let mut app = test_app(PathBuf::from("/tmp/dayz"), PathBuf::from("/tmp/workshop"));
        let screen = ConfirmScreen::new(ConfirmAction::FixMaxMapCount);

        let action = screen.confirm(&mut app);

        assert_eq!(action, Action::Quit);
        assert!(app.status_message.is_none());

        drop(shell_env);
        fs::remove_dir_all(root).expect("remove temp root");
    }

    #[test]
    fn confirming_legacy_migration_merges_profile_and_renames_legacy_file() {
        let _guard = env_lock();
        let root = temp_path("popup-legacy-migration");
        let data_dir = root.join("data");
        let home = root.join("home");
        let legacy_dir = home.join(".local/share/dayz-ctl");
        let legacy_path = legacy_dir.join("profile.json");
        fs::create_dir_all(&data_dir).expect("create data dir");
        fs::create_dir_all(&legacy_dir).expect("create legacy dir");
        let home_env = EnvVarGuard::set("HOME", home.as_os_str());

        let mut current = Profile::default();
        current.add_favorite("Current", "1.1.1.1", 2302);
        current
            .save(&data_dir.join("profile.json"))
            .expect("save current profile");

        let mut legacy = Profile::default();
        legacy.add_favorite("Legacy", "2.2.2.2", 2402);
        legacy.save(&legacy_path).expect("save legacy profile");

        let mut app = App::new(
            Config {
                path: data_dir.join("dayz-cmd.conf"),
                data_dir: data_dir.clone(),
                server_db_path: data_dir.join("servers.json"),
                news_db_path: data_dir.join("news.json"),
                mods_db_path: data_dir.join("mods.json"),
                profile_path: data_dir.join("profile.json"),
                api_url: "https://example.test".into(),
                github_owner: "example".into(),
                github_repo: "dayz-cmd".into(),
                request_timeout: 10,
                server_request_timeout: 30,
                server_db_ttl: 300,
                news_db_ttl: 3600,
                history_size: 10,
                steamcmd_enabled: true,
                filter_mod_limit: 10,
                filter_players_limit: 50,
                filter_players_slots: 60,
                applications_dir: PathBuf::from("/tmp"),
            },
            current,
        );
        let mut screen = ConfirmScreen::new(ConfirmAction::MigrateLegacy);
        screen.selected = true;

        let action = screen.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &mut app);

        assert_eq!(action, Action::PopScreen);
        assert!(legacy_dir.join("profile.json.migrated").exists());
        assert!(!legacy_path.exists());
        assert!(app.profile.is_favorite("1.1.1.1", 2302));
        assert!(app.profile.is_favorite("2.2.2.2", 2402));

        drop(home_env);
        fs::remove_dir_all(root).expect("remove temp root");
    }
}
