use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Frame;
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

use super::{Action, InfoScreenData, Screen, theme};
use crate::app::App;

pub struct InfoScreen {
    data: InfoScreenData,
}

impl InfoScreen {
    pub fn new(data: InfoScreenData) -> Self {
        Self { data }
    }
}

impl Screen for InfoScreen {
    fn render(&mut self, f: &mut Frame, _app: &App) {
        let area = centered_rect(70, 40, f.area());
        f.render_widget(Clear, area);

        let mut lines = vec![Line::from("")];
        lines.push(Line::from(Span::styled(&self.data.title, theme::TITLE)));
        lines.push(Line::from(""));
        for line in &self.data.lines {
            lines.push(Line::from(Span::styled(line, theme::NORMAL)));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(" Press any key to exit ", theme::KEY_HINT)));

        let para = Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(theme::BORDER)
                    .title(" Info "),
            )
            .wrap(Wrap { trim: true });
        f.render_widget(para, area);
    }

    fn handle_key(&mut self, key: KeyEvent, _app: &mut App) -> Action {
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return Action::Quit;
        }

        let _ = key;
        Action::Quit
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
