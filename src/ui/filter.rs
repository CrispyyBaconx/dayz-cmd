use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState};
use ratatui::Frame;

use super::{theme, Action, Screen, ScreenId};
use crate::app::App;
use crate::server::filter::{apply_filters, ServerFilter};

pub struct FilterSelectScreen {
    pub list_state: ListState,
    pub items: Vec<FilterItem>,
    pub editing: Option<TextFilterField>,
    pub edit_buffer: String,
    map_name: String,
    mod_name: String,
    mod_id: String,
}

pub struct FilterItem {
    pub label: String,
    pub selected: bool,
    pub kind: FilterItemKind,
}

pub enum FilterItemKind {
    Toggle(ServerFilter),
    MapName,
    ModName,
    ModId,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TextFilterField {
    MapName,
    ModName,
    ModId,
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
            FilterItem::text("Map", FilterItemKind::MapName),
            FilterItem::text("Mod Name", FilterItemKind::ModName),
            FilterItem::text("Mod ID", FilterItemKind::ModId),
        ];

            Self {
                list_state: ListState::default().with_selected(Some(0)),
                items,
                editing: None,
                edit_buffer: String::new(),
                map_name: String::new(),
                mod_name: String::new(),
                mod_id: String::new(),
            }
    }
}

impl FilterItem {
    fn new(label: &str, filter: ServerFilter) -> Self {
        Self {
            label: label.to_string(),
            selected: false,
            kind: FilterItemKind::Toggle(filter),
        }
    }

    fn text(label: &str, kind: FilterItemKind) -> Self {
        Self {
            label: label.to_string(),
            selected: false,
            kind,
        }
    }
}

impl Screen for FilterSelectScreen {
    fn render(&mut self, f: &mut Frame, _app: &App) {
        let area = f.area();

        if let Some(field) = self.editing.clone() {
            return self.render_edit(f, area, field);
        }

        let items: Vec<ListItem> = self
            .items
            .iter()
            .map(|item| {
                let (checkbox, label) = match item.kind {
                    FilterItemKind::Toggle(_) => {
                        let checkbox = if item.selected { "[x]" } else { "[ ]" };
                        (checkbox.to_string(), item.label.clone())
                    }
                    FilterItemKind::MapName => (
                        if self.map_name.is_empty() { "[ ]" } else { "[x]" }.to_string(),
                        format!("{}: {}", item.label, display_value(&self.map_name)),
                    ),
                    FilterItemKind::ModName => (
                        if self.mod_name.is_empty() { "[ ]" } else { "[x]" }.to_string(),
                        format!("{}: {}", item.label, display_value(&self.mod_name)),
                    ),
                    FilterItemKind::ModId => (
                        if self.mod_id.is_empty() { "[ ]" } else { "[x]" }.to_string(),
                        format!("{}: {}", item.label, display_value(&self.mod_id)),
                    ),
                };
                ListItem::new(Line::from(vec![
                    Span::styled(
                        format!(" {checkbox} "),
                        if item.selected || checkbox == "[x]" {
                            theme::SUCCESS
                        } else {
                            theme::DIM
                        },
                    ),
                    Span::styled(label, theme::NORMAL),
                ]))
            })
            .collect();

        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(theme::BORDER)
                    .title(" Select Filters (Space: toggle, Enter: edit/apply, Esc: cancel) "),
            )
            .highlight_style(theme::SELECTED)
            .highlight_symbol("▶ ");

        f.render_stateful_widget(list, area, &mut self.list_state);
    }

    fn handle_key(&mut self, key: KeyEvent, app: &mut App) -> Action {
        if self.editing.is_some() {
            return self.handle_edit_key(key);
        }

        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return Action::PopScreen;
        }

        match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Backspace => Action::PopScreen,
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
                        if matches!(item.kind, FilterItemKind::Toggle(_)) {
                            item.selected = !item.selected;
                        }
                    }
                }
                Action::None
            }
            KeyCode::Enter => {
                if let Some(idx) = self.list_state.selected() {
                    if let Some(item) = self.items.get(idx) {
                        match item.kind {
                            FilterItemKind::MapName => {
                                self.editing = Some(TextFilterField::MapName);
                                self.edit_buffer = self.map_name.clone();
                                return Action::None;
                            }
                            FilterItemKind::ModName => {
                                self.editing = Some(TextFilterField::ModName);
                                self.edit_buffer = self.mod_name.clone();
                                return Action::None;
                            }
                            FilterItemKind::ModId => {
                                self.editing = Some(TextFilterField::ModId);
                                self.edit_buffer = self.mod_id.clone();
                                return Action::None;
                            }
                            FilterItemKind::Toggle(_) => {}
                        }
                    }
                }

                let filters: Vec<ServerFilter> = self
                    .items
                    .iter()
                    .filter_map(|i| match (&i.kind, i.selected) {
                        (FilterItemKind::Toggle(filter), true) => Some(filter.clone()),
                        _ => None,
                    })
                    .collect();
                let mut filters = filters;

                if !self.map_name.trim().is_empty() {
                    filters.push(ServerFilter::MapName(self.map_name.trim().to_string()));
                }
                if !self.mod_name.trim().is_empty() {
                    filters.push(ServerFilter::ModName(self.mod_name.trim().to_string()));
                }
                if let Ok(id) = self.mod_id.trim().parse() {
                    filters.push(ServerFilter::ModId(id));
                }

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

impl FilterSelectScreen {
    fn handle_edit_key(&mut self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Esc => {
                self.editing = None;
                self.edit_buffer.clear();
            }
            KeyCode::Enter => {
                if let Some(field) = self.editing.take() {
                    match field {
                        TextFilterField::MapName => self.map_name = self.edit_buffer.trim().to_string(),
                        TextFilterField::ModName => self.mod_name = self.edit_buffer.trim().to_string(),
                        TextFilterField::ModId => self.mod_id = self.edit_buffer.trim().to_string(),
                    }
                }
                self.edit_buffer.clear();
            }
            KeyCode::Backspace => {
                self.edit_buffer.pop();
            }
            KeyCode::Char(c) => {
                if self.editing == Some(TextFilterField::ModId) {
                    if c.is_ascii_digit() {
                        self.edit_buffer.push(c);
                    }
                } else {
                    self.edit_buffer.push(c);
                }
            }
            _ => {}
        }
        Action::None
    }

    fn render_edit(&self, f: &mut Frame, area: ratatui::layout::Rect, field: TextFilterField) {
        let label = match field {
            TextFilterField::MapName => "Map Name",
            TextFilterField::ModName => "Mod Name",
            TextFilterField::ModId => "Mod ID",
        };

        let lines = vec![
            Line::from(""),
            Line::from(Span::styled(format!(" Enter {label}:"), theme::TITLE)),
            Line::from(""),
            Line::from(vec![
                Span::raw(" > "),
                Span::styled(&self.edit_buffer, theme::SEARCH_INPUT),
                Span::styled("▌", theme::INFO),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                " Enter: save  Esc: cancel",
                theme::KEY_HINT,
            )),
        ];

        let para = ratatui::widgets::Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(theme::INFO)
                .title(format!(" Edit {label} ")),
        );
        f.render_widget(para, area);
    }
}

fn display_value(value: &str) -> &str {
    if value.is_empty() { "-" } else { value }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::profile::Profile;
    use std::path::PathBuf;

    fn test_app() -> App {
        let data_dir = std::env::temp_dir().join("dayz-ctl-tests-filter");
        App::new(
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
        )
    }

    #[test]
    fn filter_screen_exposes_text_driven_filters() {
        let app = test_app();
        let screen = FilterSelectScreen::new(&app);
        let labels: Vec<&str> = screen.items.iter().map(|item| item.label.as_str()).collect();

        assert!(labels.iter().any(|label| label.contains("Map")));
        assert!(labels.iter().any(|label| label.contains("Mod Name")));
        assert!(labels.iter().any(|label| label.contains("Mod ID")));
    }
}
