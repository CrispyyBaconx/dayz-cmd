use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

use super::{Action, Screen, theme};
use crate::app::App;

const UPDATE_PROMPT_TICKS: u16 = 20;

pub struct UpdatePromptScreen {
    pub selected_yes: bool,
    pub remaining_ticks: u16,
}

impl UpdatePromptScreen {
    pub fn new() -> Self {
        Self {
            selected_yes: false,
            remaining_ticks: UPDATE_PROMPT_TICKS,
        }
    }
}

impl Screen for UpdatePromptScreen {
    fn render(&mut self, f: &mut Frame, app: &App) {
        f.render_widget(Clear, f.area());
        f.render_widget(Block::default(), f.area());
        let area = centered_rect(70, 40, f.area());

        let version = app
            .available_update
            .as_ref()
            .map(|release| release.tag.as_str())
            .unwrap_or("unknown");
        let seconds = (self.remaining_ticks.saturating_add(3)) / 4;

        let lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                format!(" Update available: v{version}"),
                theme::WARNING,
            )),
            Line::from(""),
            Line::from(Span::styled(
                format!(" Auto-skip in {seconds}s"),
                theme::DIM,
            )),
            Line::from(""),
            Line::from(vec![
                Span::raw("    "),
                choice_span("Yes", self.selected_yes),
                Span::raw("    "),
                choice_span("No", !self.selected_yes),
            ]),
        ];

        let para = Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(theme::WARNING)
                    .title(" Self Update "),
            )
            .wrap(Wrap { trim: true });
        f.render_widget(para, area);
    }

    fn shows_status_bar(&self) -> bool {
        false
    }

    fn handle_key(&mut self, key: KeyEvent, _app: &mut App) -> Action {
        match key.code {
            KeyCode::Left | KeyCode::Char('h') => {
                self.selected_yes = true;
                Action::None
            }
            KeyCode::Right | KeyCode::Char('l') | KeyCode::Tab => {
                self.selected_yes = false;
                Action::None
            }
            KeyCode::Char('y') => {
                self.selected_yes = true;
                Action::RunSelfUpdate
            }
            KeyCode::Char('n') | KeyCode::Esc => Action::PopScreen,
            KeyCode::Enter => {
                if self.selected_yes {
                    Action::RunSelfUpdate
                } else {
                    Action::PopScreen
                }
            }
            _ => Action::None,
        }
    }

    fn on_tick(&mut self, _app: &mut App) -> Action {
        if self.remaining_ticks == 0 {
            return Action::PopScreen;
        }
        self.remaining_ticks = self.remaining_ticks.saturating_sub(1);
        if self.remaining_ticks == 0 {
            Action::PopScreen
        } else {
            Action::None
        }
    }
}

fn choice_span(label: &str, selected: bool) -> Span<'static> {
    let text = if selected {
        format!("[ {label} ]")
    } else {
        format!("  {label}  ")
    };
    let style = if selected {
        theme::WARNING.add_modifier(Modifier::BOLD)
    } else {
        theme::DIM
    };
    Span::styled(text, style)
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

    #[test]
    fn defaults_to_no_selection() {
        let screen = UpdatePromptScreen::new();
        assert!(!screen.selected_yes);
        assert_eq!(screen.remaining_ticks, UPDATE_PROMPT_TICKS);
    }

    #[test]
    fn countdown_expires_to_pop_screen() {
        let mut screen = UpdatePromptScreen::new();
        screen.remaining_ticks = 1;

        let action = screen.on_tick(&mut App::new(
            crate::config::Config::load().unwrap(),
            crate::profile::Profile::default(),
        ));
        assert_eq!(action, Action::PopScreen);
    }

    #[test]
    fn yes_and_no_keys_return_expected_actions() {
        let mut screen = UpdatePromptScreen::new();
        let mut app = App::new(
            crate::config::Config::load().unwrap(),
            crate::profile::Profile::default(),
        );

        assert_eq!(
            screen.handle_key(KeyEvent::from(KeyCode::Char('y')), &mut app),
            Action::RunSelfUpdate
        );
        assert_eq!(
            screen.handle_key(KeyEvent::from(KeyCode::Char('n')), &mut app),
            Action::PopScreen
        );
    }
}
