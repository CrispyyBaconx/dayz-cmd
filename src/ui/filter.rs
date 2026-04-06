use crossterm::event::{KeyCode, KeyEvent};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState};
use ratatui::Frame;

use super::{theme, Action, Screen, ScreenId};
use crate::app::App;
use crate::server::filter::{apply_filters, ServerFilter};

pub struct FilterSelectScreen {
    pub list_state: ListState,
    pub items: Vec<FilterItem>,
}

pub struct FilterItem {
    pub label: String,
    pub selected: bool,
    pub filter: ServerFilter,
}

impl FilterSelectScreen {
    pub fn new(app: &App) -> Self {
        let items = vec![
            FilterItem::new("Official Servers", ServerFilter::Official),
            FilterItem::new("Community Servers", ServerFilter::NotOfficial),
            FilterItem::new("Modded", ServerFilter::Modded),
            FilterItem::new("Unmodded", ServerFilter::NotModded),
            FilterItem::new("Without Password", ServerFilter::WithoutPassword),
            FilterItem::new("With Password", ServerFilter::WithPassword),
            FilterItem::new("First Person", ServerFilter::FirstPerson),
            FilterItem::new("Third Person", ServerFilter::ThirdPerson),
            FilterItem::new("Day Time", ServerFilter::DayTime),
            FilterItem::new("Night Time", ServerFilter::NightTime),
            FilterItem::new("With BattlEye", ServerFilter::WithBattlEye),
            FilterItem::new("Without BattlEye", ServerFilter::WithoutBattlEye),
            FilterItem::new("With Players", ServerFilter::WithPlayers),
            FilterItem::new("Empty Servers", ServerFilter::WithoutPlayers),
            FilterItem::new("Not Full", ServerFilter::NotFull),
            FilterItem::new("Full", ServerFilter::Full),
            FilterItem::new("Linux Servers", ServerFilter::LinuxServers),
            FilterItem::new("Windows Servers", ServerFilter::WindowsServers),
            FilterItem::new(
                &format!("Mods > {}", app.config.filter_mod_limit),
                ServerFilter::ModsGreaterThan(app.config.filter_mod_limit),
            ),
            FilterItem::new(
                &format!("Mods < {}", app.config.filter_mod_limit),
                ServerFilter::ModsLessThan(app.config.filter_mod_limit),
            ),
            FilterItem::new(
                &format!("Players > {}%", app.config.filter_players_limit),
                ServerFilter::PlayersGreaterThan(app.config.filter_players_limit),
            ),
            FilterItem::new(
                &format!("Players < {}%", app.config.filter_players_limit),
                ServerFilter::PlayersLessThan(app.config.filter_players_limit),
            ),
            FilterItem::new(
                &format!("Slots >= {}", app.config.filter_players_slots),
                ServerFilter::PlayerSlotsAtLeast(app.config.filter_players_slots),
            ),
        ];

        Self {
            list_state: ListState::default().with_selected(Some(0)),
            items,
        }
    }
}

impl FilterItem {
    fn new(label: &str, filter: ServerFilter) -> Self {
        Self {
            label: label.to_string(),
            selected: false,
            filter,
        }
    }
}

impl Screen for FilterSelectScreen {
    fn render(&mut self, f: &mut Frame, _app: &App) {
        let area = f.area();

        let items: Vec<ListItem> = self
            .items
            .iter()
            .map(|item| {
                let checkbox = if item.selected { "[x]" } else { "[ ]" };
                ListItem::new(Line::from(vec![
                    Span::styled(
                        format!(" {checkbox} "),
                        if item.selected {
                            theme::SUCCESS
                        } else {
                            theme::DIM
                        },
                    ),
                    Span::styled(&item.label, theme::NORMAL),
                ]))
            })
            .collect();

        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(theme::BORDER)
                    .title(" Select Filters (Space: toggle, Enter: apply, Esc: cancel) "),
            )
            .highlight_style(theme::SELECTED)
            .highlight_symbol("▶ ");

        f.render_stateful_widget(list, area, &mut self.list_state);
    }

    fn handle_key(&mut self, key: KeyEvent, app: &mut App) -> Action {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => Action::PopScreen,
            KeyCode::Up | KeyCode::Char('k') => {
                let i = self.list_state.selected().unwrap_or(0);
                let new = if i == 0 { self.items.len() - 1 } else { i - 1 };
                self.list_state.select(Some(new));
                Action::None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let i = self.list_state.selected().unwrap_or(0);
                let new = (i + 1) % self.items.len();
                self.list_state.select(Some(new));
                Action::None
            }
            KeyCode::Char(' ') => {
                if let Some(idx) = self.list_state.selected() {
                    if let Some(item) = self.items.get_mut(idx) {
                        item.selected = !item.selected;
                    }
                }
                Action::None
            }
            KeyCode::Enter => {
                let filters: Vec<ServerFilter> = self
                    .items
                    .iter()
                    .filter(|i| i.selected)
                    .map(|i| i.filter.clone())
                    .collect();

                if filters.is_empty() {
                    return Action::ReplaceScreen(ScreenId::ServerBrowser);
                }

                let indices = apply_filters(&app.servers, &filters);
                Action::ReplaceScreen(ScreenId::FilteredBrowser(indices))
            }
            _ => Action::None,
        }
    }
}
