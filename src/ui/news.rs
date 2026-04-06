use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState};
use ratatui::Frame;

use super::{theme, Action, Screen};
use crate::app::App;

pub struct NewsScreen {
    pub list_state: ListState,
}

impl NewsScreen {
    pub fn new() -> Self {
        Self {
            list_state: ListState::default().with_selected(Some(0)),
        }
    }
}

impl Screen for NewsScreen {
    fn render(&mut self, f: &mut Frame, app: &App) {
        let area = f.area();

        let items: Vec<ListItem> = app
            .news
            .iter()
            .map(|article| {
                ListItem::new(vec![
                    Line::from(vec![
                        Span::styled(" • ", theme::INFO),
                        Span::styled(&article.title, theme::NORMAL),
                    ]),
                    Line::from(Span::styled(format!("   {}", article.url()), theme::DIM)),
                ])
            })
            .collect();

        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(theme::BORDER)
                    .title(" DayZ News (Enter: open in browser, Esc: back) "),
            )
            .highlight_style(theme::SELECTED)
            .highlight_symbol("▶ ");

        f.render_stateful_widget(list, area, &mut self.list_state);
    }

    fn handle_key(&mut self, key: KeyEvent, app: &mut App) -> Action {
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return Action::PopScreen;
        }

        match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Backspace => Action::PopScreen,
            KeyCode::Up | KeyCode::Char('k') => {
                let i = self.list_state.selected().unwrap_or(0);
                let len = app.news.len();
                let new = if i == 0 { len.saturating_sub(1) } else { i - 1 };
                self.list_state.select(Some(new));
                Action::None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let i = self.list_state.selected().unwrap_or(0);
                let len = app.news.len();
                let new = if len == 0 { 0 } else { (i + 1) % len };
                self.list_state.select(Some(new));
                Action::None
            }
            KeyCode::Enter => {
                if let Some(idx) = self.list_state.selected() {
                    if let Some(article) = app.news.get(idx) {
                        let _ = open::that(article.url());
                    }
                }
                Action::None
            }
            _ => Action::None,
        }
    }
}
