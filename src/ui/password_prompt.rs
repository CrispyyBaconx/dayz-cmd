use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Frame;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use super::{Action, Screen, theme};
use crate::app::App;

pub struct PasswordPromptScreen {
    password: String,
}

impl PasswordPromptScreen {
    pub fn new() -> Self {
        Self {
            password: String::new(),
        }
    }
}

impl Screen for PasswordPromptScreen {
    fn render(&mut self, f: &mut Frame, _app: &App) {
        let area = f.area();
        let masked = "*".repeat(self.password.chars().count());

        let lines = vec![
            Line::from(""),
            Line::from(Span::styled(" Enter server password", theme::TITLE)),
            Line::from(""),
            Line::from(vec![
                Span::styled(" Password: ", theme::DIM),
                Span::styled(masked, theme::INFO),
                Span::styled("▌", theme::INFO),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                " Enter: connect  Esc: cancel",
                theme::KEY_HINT,
            )),
        ];

        let para = Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(theme::WARNING)
                    .title(" Server Password "),
            )
            .wrap(Wrap { trim: true });
        f.render_widget(para, area);
    }

    fn handle_key(&mut self, key: KeyEvent, app: &mut App) -> Action {
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            app.launch_password = None;
            return Action::PopScreen;
        }

        match key.code {
            KeyCode::Esc => {
                app.launch_password = None;
                Action::PopScreen
            }
            KeyCode::Backspace => {
                self.password.pop();
                Action::None
            }
            KeyCode::Enter => {
                if self.password.is_empty() {
                    return Action::None;
                }
                app.launch_password = Some(self.password.clone());
                Action::LaunchGame
            }
            KeyCode::Char(c) => {
                self.password.push(c);
                Action::None
            }
            _ => Action::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::profile::Profile;
    use std::path::PathBuf;

    fn test_app() -> App {
        let data_dir = std::env::temp_dir().join("dayz-cmd-tests-password-prompt");
        App::new(
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
        )
    }

    #[test]
    fn enter_submits_password_and_retries_launch() {
        let mut screen = PasswordPromptScreen::new();
        screen.password = "secret".into();
        let mut app = test_app();

        let action = screen.handle_key(KeyEvent::from(KeyCode::Enter), &mut app);

        assert_eq!(action, Action::LaunchGame);
        assert_eq!(app.launch_password.as_deref(), Some("secret"));
    }
}
