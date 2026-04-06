use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Frame;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use super::{Action, Screen, ScreenId, theme};
use crate::app::App;

pub struct DirectConnectScreen {
    pub ip: String,
    pub port: String,
    pub active_field: Field,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Field {
    Ip,
    Port,
}

impl DirectConnectScreen {
    pub fn new() -> Self {
        Self {
            ip: String::new(),
            port: "2302".into(),
            active_field: Field::Ip,
        }
    }
}

impl Screen for DirectConnectScreen {
    fn render(&mut self, f: &mut Frame, _app: &App) {
        let area = f.area();

        let ip_style = if self.active_field == Field::Ip {
            theme::INFO
        } else {
            theme::NORMAL
        };
        let port_style = if self.active_field == Field::Port {
            theme::INFO
        } else {
            theme::NORMAL
        };

        let cursor = Span::styled("▌", theme::INFO);

        let lines = vec![
            Line::from(""),
            Line::from(Span::styled(" Direct Connect to Server", theme::TITLE)),
            Line::from(""),
            Line::from(vec![
                Span::styled(" IP Address: ", theme::DIM),
                Span::styled(&self.ip, ip_style),
                if self.active_field == Field::Ip {
                    cursor.clone()
                } else {
                    Span::raw("")
                },
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled(" Game Port:  ", theme::DIM),
                Span::styled(&self.port, port_style),
                if self.active_field == Field::Port {
                    cursor
                } else {
                    Span::raw("")
                },
            ]),
            Line::from(""),
            Line::from(Span::styled(
                " Tab: switch field  Enter: connect  Esc: cancel",
                theme::KEY_HINT,
            )),
        ];

        let para = Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(theme::BORDER)
                    .title(" Direct Connect "),
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
            KeyCode::Tab | KeyCode::BackTab => {
                self.active_field = match self.active_field {
                    Field::Ip => Field::Port,
                    Field::Port => Field::Ip,
                };
                Action::None
            }
            KeyCode::Backspace => {
                match self.active_field {
                    Field::Ip => {
                        self.ip.pop();
                    }
                    Field::Port => {
                        self.port.pop();
                    }
                }
                Action::None
            }
            KeyCode::Char(c) => {
                match self.active_field {
                    Field::Ip => {
                        if c.is_ascii_digit() || c == '.' {
                            self.ip.push(c);
                        }
                    }
                    Field::Port => {
                        if c.is_ascii_digit() {
                            self.port.push(c);
                        }
                    }
                }
                Action::None
            }
            KeyCode::Enter => {
                if self.ip.is_empty() {
                    return Action::None;
                }
                let port: u16 = self.port.parse().unwrap_or(2302);

                if let Some(idx) = app
                    .servers
                    .iter()
                    .position(|s| s.endpoint.ip == self.ip && s.game_port == port)
                {
                    app.prepare_known_server_launch(idx);
                    return Action::LaunchGame;
                }

                if let Some(idx) = app
                    .servers
                    .iter()
                    .position(|s| s.endpoint.ip == self.ip && s.endpoint.port == port)
                {
                    app.prepare_known_server_launch(idx);
                    return Action::LaunchGame;
                }

                app.prepare_direct_connect_launch(self.ip.clone(), port);
                Action::PushScreen(ScreenId::DirectConnectSetup)
            }
            _ => Action::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{LaunchPrep, LaunchTarget};
    use crate::config::Config;
    use crate::profile::Profile;
    use std::path::PathBuf;

    fn test_app() -> App {
        let data_dir = std::env::temp_dir().join("dayz-cmd-tests-direct-connect");
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
    fn unknown_direct_connect_routes_to_setup_instead_of_launching() {
        let mut screen = DirectConnectScreen::new();
        screen.ip = "5.6.7.8".into();
        screen.port = "2402".into();
        let mut app = test_app();

        let action = screen.handle_key(KeyEvent::from(KeyCode::Enter), &mut app);

        assert_eq!(action, Action::PushScreen(ScreenId::DirectConnectSetup));
        assert_eq!(
            app.launch_prep,
            Some(LaunchPrep {
                target: LaunchTarget::DirectConnect {
                    ip: "5.6.7.8".into(),
                    port: 2402,
                },
                mod_ids: Vec::new(),
                password: None,
                offline_spawn_enabled: None,
            })
        );
    }
}
