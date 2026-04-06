use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;

use super::{theme, Action, Screen};
use crate::app::App;

pub struct ServerDetailScreen {
    pub server_index: usize,
    pub list_state: ListState,
    items: Vec<DetailAction>,
}

#[derive(Debug, Clone)]
enum DetailAction {
    Play,
    AddFavorite,
    RemoveFavorite,
    CreateDesktopEntry,
    UpdateDesktopEntry,
    DeleteDesktopEntry,
}

impl ServerDetailScreen {
    pub fn new(server_index: usize) -> Self {
        Self {
            server_index,
            list_state: ListState::default().with_selected(Some(0)),
            items: Vec::new(),
        }
    }

    fn build_items(&mut self, app: &App) {
        let mut items = vec![DetailAction::Play];

        if let Some(server) = app.servers.get(self.server_index) {
            if app
                .profile
                .is_favorite(&server.endpoint.ip, server.endpoint.port)
            {
                items.push(DetailAction::RemoveFavorite);
            } else {
                items.push(DetailAction::AddFavorite);
            }

            if app.config.applications_dir.exists() {
                if crate::launch::desktop_entry_exists(
                    &app.config.applications_dir,
                    &server.endpoint.ip,
                    server.game_port,
                ) {
                    items.push(DetailAction::UpdateDesktopEntry);
                    items.push(DetailAction::DeleteDesktopEntry);
                } else {
                    items.push(DetailAction::CreateDesktopEntry);
                }
            }
        }

        self.items = items;
    }

    fn label(action: &DetailAction) -> &str {
        match action {
            DetailAction::Play => "Play",
            DetailAction::AddFavorite => "Add to Favorites",
            DetailAction::RemoveFavorite => "Remove from Favorites",
            DetailAction::CreateDesktopEntry => "Create Desktop Entry",
            DetailAction::UpdateDesktopEntry => "Update Desktop Entry",
            DetailAction::DeleteDesktopEntry => "Delete Desktop Entry",
        }
    }
}

impl Screen for ServerDetailScreen {
    fn on_enter(&mut self, app: &mut App) {
        self.build_items(app);
    }

    fn render(&mut self, f: &mut Frame, app: &App) {
        let area = f.area();
        let chunks = Layout::vertical([Constraint::Min(14), Constraint::Min(0)]).split(area);

        self.render_server_info(f, chunks[0], app);
        self.render_actions(f, chunks[1]);
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
            KeyCode::Enter => {
                if let Some(idx) = self.list_state.selected() {
                    if let Some(action) = self.items.get(idx) {
                        return self.execute_action(action.clone(), app);
                    }
                }
                Action::None
            }
            _ => Action::None,
        }
    }
}

impl ServerDetailScreen {
    fn execute_action(&self, action: DetailAction, app: &mut App) -> Action {
        let server = match app.servers.get(self.server_index) {
            Some(s) => s.clone(),
            None => return Action::PopScreen,
        };

        match action {
            DetailAction::Play => {
                app.selected_server = Some(self.server_index);
                app.direct_connect_target = None;
                Action::LaunchGame
            }
            DetailAction::AddFavorite => {
                app.profile
                    .add_favorite(&server.name, &server.endpoint.ip, server.endpoint.port);
                let _ = app.profile.save(&app.config.profile_path);
                app.status_message =
                    Some(format!("Added '{}' to favorites", server.name));
                Action::PopScreen
            }
            DetailAction::RemoveFavorite => {
                app.profile
                    .remove_favorite(&server.endpoint.ip, server.endpoint.port);
                let _ = app.profile.save(&app.config.profile_path);
                app.status_message =
                    Some(format!("Removed '{}' from favorites", server.name));
                Action::PopScreen
            }
            DetailAction::CreateDesktopEntry | DetailAction::UpdateDesktopEntry => {
                let exe = std::env::current_exe()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|_| "dayz-ctl".into());
                let _ = crate::launch::create_desktop_entry(
                    &app.config.applications_dir,
                    &server.name,
                    &server.endpoint.ip,
                    server.game_port,
                    &exe,
                );
                app.status_message = Some("Desktop entry created".into());
                Action::PopScreen
            }
            DetailAction::DeleteDesktopEntry => {
                let _ = crate::launch::delete_desktop_entry(
                    &app.config.applications_dir,
                    &server.endpoint.ip,
                    server.game_port,
                );
                app.status_message = Some("Desktop entry deleted".into());
                Action::PopScreen
            }
        }
    }

    fn render_server_info(&self, f: &mut Frame, area: ratatui::layout::Rect, app: &App) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(theme::BORDER)
            .title(" Server Info ");

        if let Some(s) = app.servers.get(self.server_index) {
            let players_str = format!("{}/{}", s.players, s.max_players);
            let time_str = format!("{} {}", s.time, s.time_icon());
            let addr_str = format!("{}:{}", s.endpoint.ip, s.game_port);
            let mods_str = if s.mods.is_empty() {
                "None".to_string()
            } else {
                format!("{} mods", s.mods.len())
            };

            let lines = vec![
                Line::from(Span::styled(s.name.clone(), theme::HIGHLIGHT)),
                Line::from(""),
                info_line("Players", &players_str),
                info_line("Map", &s.map),
                info_line("Time", &time_str),
                info_line("Platform", s.platform_str()),
                info_line(
                    "BattlEye",
                    if s.battleye { "On" } else { "Off" },
                ),
                info_line("IP", &addr_str),
                info_line("Mods", &mods_str),
            ];
            let para = Paragraph::new(lines).block(block).wrap(Wrap { trim: true });
            f.render_widget(para, area);
        } else {
            let para = Paragraph::new("Server not found").block(block);
            f.render_widget(para, area);
        }
    }

    fn render_actions(&mut self, f: &mut Frame, area: ratatui::layout::Rect) {
        let items: Vec<ListItem> = self
            .items
            .iter()
            .map(|a| ListItem::new(format!("  {}", Self::label(a))))
            .collect();

        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(theme::BORDER)
                    .title(" Actions "),
            )
            .highlight_style(theme::SELECTED)
            .highlight_symbol("▶ ");

        f.render_stateful_widget(list, area, &mut self.list_state);
    }
}

fn info_line<'a>(label: &'a str, value: &'a str) -> Line<'a> {
    Line::from(vec![
        Span::styled(format!(" {label}: "), theme::DIM),
        Span::styled(value, theme::NORMAL),
    ])
}
