use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

use super::{theme, Action, ConfirmAction, Screen};
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
        }
    }

    fn yes_label(&self) -> &str {
        match &self.action {
            ConfirmAction::UpdateModsBeforeLaunch => "Update",
            _ => "Yes",
        }
    }

    fn no_label(&self) -> &str {
        match &self.action {
            ConfirmAction::UpdateModsBeforeLaunch => "Skip",
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
            _ => Action::PopScreen,
        }
    }

    fn confirm(&self, app: &mut App) -> Action {
        match &self.action {
            ConfirmAction::Quit => Action::Quit,
            ConfirmAction::KillDayZ => {
                let _ = crate::launch::kill_dayz();
                Action::PopScreen
            }
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
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use std::fs;
    use std::os::unix::fs as unix_fs;
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

    fn test_app(dayz_path: PathBuf, workshop_path: PathBuf) -> App {
        let data_dir = temp_path("popup-data");
        let mut app = App::new(
            Config {
                path: data_dir.join("dayz-ctl.conf"),
                data_dir: data_dir.clone(),
                server_db_path: data_dir.join("servers.json"),
                news_db_path: data_dir.join("news.json"),
                mods_db_path: data_dir.join("mods.json"),
                profile_path: data_dir.join("profile.json"),
                api_url: "https://example.test".into(),
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

    #[test]
    fn confirming_remove_mod_links_executes_action() {
        let root = temp_path("popup-remove-links");
        let dayz_path = root.join("dayz");
        let workshop_path = root.join("workshop");
        fs::create_dir_all(&dayz_path).expect("create dayz path");
        fs::create_dir_all(&workshop_path).expect("create workshop path");
        fs::create_dir_all(workshop_path.join("123")).expect("create workshop mod");
        unix_fs::symlink(workshop_path.join("123"), dayz_path.join("@123")).expect("create symlink");

        let mut app = test_app(dayz_path.clone(), workshop_path);
        let mut screen = ConfirmScreen::new(ConfirmAction::RemoveModLinks);
        screen.selected = true;

        let action = screen.handle_key(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            &mut app,
        );

        assert_eq!(action, Action::PopScreen);
        assert!(!dayz_path.join("@123").exists());

        fs::remove_dir_all(root).expect("remove temp root");
    }
}
