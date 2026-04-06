use crossterm::event::{KeyCode, KeyEvent};
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

        let lines = vec![
            Line::from(""),
            Line::from(Span::styled(self.message(), theme::WARNING)),
            Line::from(""),
            Line::from(vec![
                Span::raw("    "),
                Span::styled(" Yes ", yes_style),
                Span::raw("    "),
                Span::styled(" No ", no_style),
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

    fn handle_key(&mut self, key: KeyEvent, _app: &mut App) -> Action {
        match key.code {
            KeyCode::Esc => Action::PopScreen,
            KeyCode::Left | KeyCode::Right | KeyCode::Tab => {
                self.selected = !self.selected;
                Action::None
            }
            KeyCode::Char('y') => {
                self.selected = true;
                self.confirm()
            }
            KeyCode::Char('n') => Action::PopScreen,
            KeyCode::Enter => {
                if self.selected {
                    self.confirm()
                } else {
                    Action::PopScreen
                }
            }
            _ => Action::None,
        }
    }
}

impl ConfirmScreen {
    fn confirm(&self) -> Action {
        match &self.action {
            ConfirmAction::Quit => Action::Quit,
            ConfirmAction::KillDayZ => {
                let _ = crate::launch::kill_dayz();
                Action::PopScreen
            }
            _ => Action::PopScreen,
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
