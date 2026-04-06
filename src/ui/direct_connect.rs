use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

use super::{theme, Action, Screen};
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
                    app.selected_server = Some(idx);
                    app.direct_connect_target = None;
                    return Action::LaunchGame;
                }

                if let Some(idx) = app
                    .servers
                    .iter()
                    .position(|s| s.endpoint.ip == self.ip && s.endpoint.port == port)
                {
                    app.selected_server = Some(idx);
                    app.direct_connect_target = None;
                    return Action::LaunchGame;
                }

                app.selected_server = None;
                app.direct_connect_target = Some((self.ip.clone(), port));
                Action::LaunchGame
            }
            _ => Action::None,
        }
    }
}
