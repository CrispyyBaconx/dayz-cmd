use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Borders, Cell, Paragraph, Row, Scrollbar, ScrollbarOrientation, ScrollbarState, Table,
    TableState, Wrap,
};
use ratatui::Frame;

use super::{theme, Action, Screen, ScreenId};
use crate::app::App;

pub struct ServerBrowserScreen {
    pub table_state: TableState,
    pub search_input: String,
    pub search_active: bool,
    pub filtered_indices: Vec<usize>,
    pub scroll_offset: u16,
    source: BrowseSource,
    matcher: SkimMatcherV2,
}

#[derive(Debug, Clone)]
pub enum BrowseSource {
    All,
    Filtered(Vec<usize>),
    Favorites,
    History,
}

impl ServerBrowserScreen {
    pub fn new(source: BrowseSource) -> Self {
        Self {
            table_state: TableState::default().with_selected(Some(0)),
            search_input: String::new(),
            search_active: false,
            filtered_indices: Vec::new(),
            scroll_offset: 0,
            source,
            matcher: SkimMatcherV2::default(),
        }
    }

    fn apply_search(&mut self, app: &App) {
        let base_indices: Vec<usize> = match &self.source {
            BrowseSource::All => (0..app.servers.len()).collect(),
            BrowseSource::Filtered(indices) => indices.clone(),
            BrowseSource::Favorites => app
                .servers
                .iter()
                .enumerate()
                .filter(|(_, s)| {
                    app.profile
                        .favorites
                        .iter()
                        .any(|f| f.ip == s.endpoint.ip && f.port == s.endpoint.port)
                })
                .map(|(i, _)| i)
                .collect(),
            BrowseSource::History => app
                .servers
                .iter()
                .enumerate()
                .filter(|(_, s)| {
                    app.profile
                        .history
                        .iter()
                        .any(|h| h.ip == s.endpoint.ip && h.port == s.endpoint.port)
                })
                .map(|(i, _)| i)
                .collect(),
        };

        if self.search_input.is_empty() {
            self.filtered_indices = base_indices;
        } else {
            let mut scored: Vec<(usize, i64)> = base_indices
                .into_iter()
                .filter_map(|i| {
                    let server = &app.servers[i];
                    self.matcher
                        .fuzzy_match(&server.name, &self.search_input)
                        .map(|score| (i, score))
                })
                .collect();
            scored.sort_by(|a, b| b.1.cmp(&a.1));
            self.filtered_indices = scored.into_iter().map(|(i, _)| i).collect();
        }

        if let Some(sel) = self.table_state.selected() {
            if sel >= self.filtered_indices.len() {
                self.table_state
                    .select(Some(self.filtered_indices.len().saturating_sub(1)));
            }
        }
    }

    fn selected_server_index(&self) -> Option<usize> {
        self.table_state
            .selected()
            .and_then(|i| self.filtered_indices.get(i).copied())
    }
}

impl Screen for ServerBrowserScreen {
    fn on_enter(&mut self, app: &mut App) {
        self.apply_search(app);
    }

    fn render(&mut self, f: &mut Frame, app: &App) {
        let area = f.area();
        let wide = area.width >= 120;

        if wide {
            let chunks = Layout::horizontal([Constraint::Percentage(55), Constraint::Percentage(45)])
                .split(area);
            self.render_table(f, chunks[0], app);
            self.render_detail(f, chunks[1], app);
        } else {
            let chunks = Layout::vertical([Constraint::Percentage(60), Constraint::Percentage(40)])
                .split(area);
            self.render_table(f, chunks[0], app);
            self.render_detail(f, chunks[1], app);
        }
    }

    fn handle_key(&mut self, key: KeyEvent, app: &mut App) -> Action {
        if self.search_active {
            return self.handle_search_key(key, app);
        }

        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => Action::PopScreen,
            KeyCode::Char('/') => {
                self.search_active = true;
                Action::None
            }
            KeyCode::Up | KeyCode::Char('k') => {
                let i = self.table_state.selected().unwrap_or(0);
                let new = if i == 0 {
                    self.filtered_indices.len().saturating_sub(1)
                } else {
                    i - 1
                };
                self.table_state.select(Some(new));
                self.scroll_offset = 0;
                Action::None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let i = self.table_state.selected().unwrap_or(0);
                let len = self.filtered_indices.len();
                let new = if len == 0 { 0 } else { (i + 1) % len };
                self.table_state.select(Some(new));
                self.scroll_offset = 0;
                Action::None
            }
            KeyCode::PageUp => {
                let i = self.table_state.selected().unwrap_or(0);
                self.table_state.select(Some(i.saturating_sub(20)));
                Action::None
            }
            KeyCode::PageDown => {
                let i = self.table_state.selected().unwrap_or(0);
                let max = self.filtered_indices.len().saturating_sub(1);
                self.table_state.select(Some((i + 20).min(max)));
                Action::None
            }
            KeyCode::Home => {
                self.table_state.select(Some(0));
                Action::None
            }
            KeyCode::End => {
                self.table_state
                    .select(Some(self.filtered_indices.len().saturating_sub(1)));
                Action::None
            }
            KeyCode::Enter => {
                if let Some(idx) = self.selected_server_index() {
                    Action::PushScreen(ScreenId::ServerDetail(idx))
                } else {
                    Action::None
                }
            }
            _ => Action::None,
        }
    }
}

impl ServerBrowserScreen {
    fn handle_search_key(&mut self, key: KeyEvent, app: &mut App) -> Action {
        match key.code {
            KeyCode::Esc => {
                self.search_active = false;
                self.search_input.clear();
                self.apply_search(app);
                Action::None
            }
            KeyCode::Enter => {
                self.search_active = false;
                Action::None
            }
            KeyCode::Backspace => {
                self.search_input.pop();
                self.apply_search(app);
                Action::None
            }
            KeyCode::Char(c) => {
                if key.modifiers.contains(KeyModifiers::CONTROL) && c == 'u' {
                    self.search_input.clear();
                } else {
                    self.search_input.push(c);
                }
                self.apply_search(app);
                Action::None
            }
            _ => Action::None,
        }
    }

    fn render_table(&mut self, f: &mut Frame, area: Rect, app: &App) {
        let chunks =
            Layout::vertical([Constraint::Length(3), Constraint::Min(0)]).split(area);

        let search_label = if self.search_active { "Search: " } else { "/ search " };
        let search_line = Line::from(vec![
            Span::styled(search_label, theme::DIM),
            Span::styled(&self.search_input, theme::SEARCH_INPUT),
            if self.search_active {
                Span::styled("▌", theme::INFO)
            } else {
                Span::raw("")
            },
            Span::raw("  "),
            Span::styled(
                format!("{} servers", self.filtered_indices.len()),
                theme::DIM,
            ),
        ]);
        let search_bar = Paragraph::new(search_line).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(if self.search_active {
                    theme::INFO
                } else {
                    theme::BORDER
                })
                .title(match &self.source {
                    BrowseSource::All => " All Servers ",
                    BrowseSource::Filtered(_) => " Filtered Servers ",
                    BrowseSource::Favorites => " Favorites ",
                    BrowseSource::History => " Recently Played ",
                }),
        );
        f.render_widget(search_bar, chunks[0]);

        let header = Row::new(vec![
            Cell::from("Name").style(theme::TITLE),
            Cell::from("Players").style(theme::TITLE),
            Cell::from("Map").style(theme::TITLE),
            Cell::from("Mods").style(theme::TITLE),
        ]);

        let rows: Vec<Row> = self
            .filtered_indices
            .iter()
            .filter_map(|&i| app.servers.get(i))
            .map(|s| {
                let player_style = if s.players == 0 {
                    theme::DIM
                } else if s.is_full() {
                    theme::WARNING
                } else {
                    theme::SUCCESS
                };

                Row::new(vec![
                    Cell::from(truncate_str(&s.name, 40)),
                    Cell::from(format!("{}/{}", s.players, s.max_players)).style(player_style),
                    Cell::from(truncate_str(&s.map, 18)),
                    Cell::from(if s.mods.is_empty() {
                        "-".to_string()
                    } else {
                        s.mods.len().to_string()
                    }),
                ])
            })
            .collect();

        let total = rows.len();
        let table = Table::new(
            rows,
            [
                Constraint::Min(30),
                Constraint::Length(10),
                Constraint::Length(20),
                Constraint::Length(6),
            ],
        )
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(theme::BORDER),
        )
        .row_highlight_style(theme::SELECTED)
        .highlight_symbol("▶ ");

        f.render_stateful_widget(table, chunks[1], &mut self.table_state);

        let mut scrollbar_state =
            ScrollbarState::new(total).position(self.table_state.selected().unwrap_or(0));
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);
        f.render_stateful_widget(scrollbar, chunks[1], &mut scrollbar_state);
    }

    fn render_detail(&mut self, f: &mut Frame, area: Rect, app: &App) {
        let server = self.selected_server_index().and_then(|i| app.servers.get(i));

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(theme::BORDER)
            .title(" Server Details ");

        if let Some(s) = server {
            let mut lines = vec![
                Line::from(Span::styled(s.name.clone(), theme::HIGHLIGHT)),
                Line::from(""),
                detail_line("Players", format!("{}/{}", s.players, s.max_players)),
                detail_line("Time", format!("{} {}", s.time, s.time_icon())),
                detail_line(
                    "Time Accel",
                    format!("x{}", s.time_acceleration.unwrap_or(1.0)),
                ),
                detail_line("Map", s.map.clone()),
                detail_line(
                    "Password",
                    (if s.password { "Yes" } else { "No" }).into(),
                ),
                detail_line(
                    "BattlEye",
                    (if s.battleye { "On" } else { "Off" }).into(),
                ),
                detail_line(
                    "VAC",
                    (if s.vac { "On" } else { "Off" }).into(),
                ),
                detail_line(
                    "Perspective",
                    (if s.first_person_only { "1PP" } else { "1PP/3PP" }).into(),
                ),
                detail_line(
                    "Official",
                    (if s.is_official() { "Yes" } else { "No" }).into(),
                ),
                detail_line("Platform", s.platform_str().into()),
                detail_line("Version", s.version.clone()),
                Line::from(""),
                detail_line("IP", s.endpoint.ip.clone()),
                detail_line("Game Port", s.game_port.to_string()),
                detail_line("Query Port", s.endpoint.port.to_string()),
            ];

            if !s.mods.is_empty() {
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    format!("Mods [{}]:", s.mods.len()),
                    theme::TITLE,
                )));
                for m in &s.mods {
                    let installed = app.mods_db.is_installed(m.steam_workshop_id);
                    let icon = if installed { "✓" } else { "✗" };
                    let style = if installed { theme::SUCCESS } else { theme::ERROR };
                    lines.push(Line::from(vec![
                        Span::styled(format!(" {icon} "), style),
                        Span::styled(m.name.clone(), theme::NORMAL),
                    ]));
                }
            }

            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Enter: select  /: search  q: back",
                theme::KEY_HINT,
            )));

            let para = Paragraph::new(lines)
                .block(block)
                .wrap(Wrap { trim: true })
                .scroll((self.scroll_offset, 0));
            f.render_widget(para, area);
        } else {
            let para = Paragraph::new("No server selected")
                .block(block)
                .style(theme::DIM);
            f.render_widget(para, area);
        }
    }
}

fn detail_line(label: &str, value: String) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!(" {label}: "), theme::DIM),
        Span::styled(value, theme::NORMAL),
    ])
}

fn truncate_str(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max - 1])
    }
}
