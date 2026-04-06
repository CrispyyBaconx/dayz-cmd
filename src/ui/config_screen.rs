use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;

use super::{theme, Action, Screen};
use crate::app::App;

pub struct ConfigScreen {
    pub list_state: ListState,
    pub editing: Option<EditField>,
    pub edit_buffer: String,
    items: Vec<ConfigItem>,
}

#[derive(Debug, Clone)]
enum ConfigItem {
    LaunchOptions,
    PlayerName,
    SteamRoot,
    InstalledMods,
    RemoveManagedMods,
    RemoveModLinks,
    RefreshServers,
    About,
}

#[derive(Debug, Clone)]
pub enum EditField {
    PlayerName,
    SteamRoot,
}

impl ConfigScreen {
    pub fn new() -> Self {
        Self {
            list_state: ListState::default().with_selected(Some(0)),
            editing: None,
            edit_buffer: String::new(),
            items: Vec::new(),
        }
    }

    fn build_items(&mut self, app: &App) {
        let mut items = vec![
            ConfigItem::LaunchOptions,
            ConfigItem::PlayerName,
            ConfigItem::SteamRoot,
        ];

        if !app.mods_db.mods.is_empty() {
            items.push(ConfigItem::InstalledMods);
            items.push(ConfigItem::RemoveManagedMods);
            items.push(ConfigItem::RemoveModLinks);
        }

        items.push(ConfigItem::RefreshServers);
        items.push(ConfigItem::About);

        self.items = items;
    }

    fn label(item: &ConfigItem) -> &str {
        match item {
            ConfigItem::LaunchOptions => "Game Launch Options",
            ConfigItem::PlayerName => "Change Player Name",
            ConfigItem::SteamRoot => "Change Steam Root Dir",
            ConfigItem::InstalledMods => "Installed Mod Info",
            ConfigItem::RemoveManagedMods => "Remove Managed Mods",
            ConfigItem::RemoveModLinks => "Remove All Mod Links",
            ConfigItem::RefreshServers => "Refresh Server List",
            ConfigItem::About => "About",
        }
    }
}

impl Screen for ConfigScreen {
    fn on_enter(&mut self, app: &mut App) {
        self.build_items(app);
    }

    fn render(&mut self, f: &mut Frame, app: &App) {
        let area = f.area();

        if let Some(ref field) = self.editing {
            self.render_edit(f, area, field.clone(), app);
            return;
        }

        let chunks = Layout::vertical([Constraint::Min(0), Constraint::Length(10)]).split(area);

        let items: Vec<ListItem> = self
            .items
            .iter()
            .map(|item| ListItem::new(format!("  {}", Self::label(item))))
            .collect();

        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(theme::BORDER)
                    .title(" Config "),
            )
            .highlight_style(theme::SELECTED)
            .highlight_symbol("▶ ");

        f.render_stateful_widget(list, chunks[0], &mut self.list_state);

        render_about_info(f, chunks[1], app);
    }

    fn handle_key(&mut self, key: KeyEvent, app: &mut App) -> Action {
        if self.editing.is_some() {
            return self.handle_edit_key(key, app);
        }

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
                    if let Some(item) = self.items.get(idx) {
                        return self.execute_item(item.clone(), app);
                    }
                }
                Action::None
            }
            _ => Action::None,
        }
    }
}

impl ConfigScreen {
    fn execute_item(&mut self, item: ConfigItem, app: &mut App) -> Action {
        match item {
            ConfigItem::PlayerName => {
                self.editing = Some(EditField::PlayerName);
                self.edit_buffer = app
                    .profile
                    .player
                    .clone()
                    .unwrap_or_default();
                Action::None
            }
            ConfigItem::SteamRoot => {
                self.editing = Some(EditField::SteamRoot);
                self.edit_buffer = app
                    .steam_root
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default();
                Action::None
            }
            ConfigItem::LaunchOptions => {
                // Toggle through launch options
                let keys: Vec<String> = app.profile.options.keys().cloned().collect();
                let mut toggled = Vec::new();
                for key in &keys {
                    if let Some(opt) = app.profile.options.get_mut(key) {
                        opt.enabled = !opt.enabled;
                        toggled.push(format!(
                            "{}: {}",
                            key,
                            if opt.enabled { "ON" } else { "OFF" }
                        ));
                        opt.enabled = !opt.enabled; // revert - just show info
                    }
                }
                let info: Vec<String> = app
                    .profile
                    .options
                    .iter()
                    .map(|(k, v)| {
                        format!(
                            "{} -{}{} ({})",
                            if v.enabled { "[x]" } else { "[ ]" },
                            k,
                            v.value
                                .as_ref()
                                .map(|val| format!("={val}"))
                                .unwrap_or_default(),
                            v.description
                        )
                    })
                    .collect();
                app.status_message = Some(format!("Launch options:\n{}", info.join(" | ")));
                Action::None
            }
            ConfigItem::InstalledMods => {
                let info = format!(
                    "Installed mods: {}, Total size: {}",
                    app.mods_db.mods.len(),
                    app.mods_db.total_size_human()
                );
                app.status_message = Some(info);
                Action::None
            }
            ConfigItem::RemoveManagedMods => {
                if let (Some(wp), Some(dp)) = (&app.workshop_path, &app.dayz_path) {
                    match crate::mods::remove_managed_mods(wp, dp) {
                        Ok((count, _)) => {
                            app.status_message =
                                Some(format!("Removed {count} managed mods"));
                        }
                        Err(e) => {
                            app.status_message = Some(format!("Error: {e}"));
                        }
                    }
                }
                Action::None
            }
            ConfigItem::RemoveModLinks => {
                if let Some(dp) = &app.dayz_path {
                    match crate::mods::remove_mod_symlinks(dp) {
                        Ok(count) => {
                            app.status_message =
                                Some(format!("Removed {count} mod symlinks"));
                        }
                        Err(e) => {
                            app.status_message = Some(format!("Error: {e}"));
                        }
                    }
                }
                Action::None
            }
            ConfigItem::RefreshServers => {
                app.status_message = Some("Refreshing server list...".into());
                app.refresh_servers();
                Action::None
            }
            ConfigItem::About => {
                let about = format!(
                    "DayZ CTL v{}\nAPI: {}\nData: {}",
                    crate::config::VERSION,
                    app.config.api_url,
                    app.config.data_dir.display()
                );
                app.status_message = Some(about);
                Action::None
            }
        }
    }

    fn handle_edit_key(&mut self, key: KeyEvent, app: &mut App) -> Action {
        match key.code {
            KeyCode::Esc => {
                self.editing = None;
                self.edit_buffer.clear();
                Action::None
            }
            KeyCode::Enter => {
                let field = self.editing.take().unwrap();
                match field {
                    EditField::PlayerName => {
                        if !self.edit_buffer.is_empty() {
                            app.profile.player = Some(self.edit_buffer.clone());
                            let _ = app.profile.save(&app.config.profile_path);
                            app.status_message =
                                Some(format!("Player name set to '{}'", self.edit_buffer));
                        }
                    }
                    EditField::SteamRoot => {
                        let path = std::path::PathBuf::from(&self.edit_buffer);
                        if path.join("common/DayZ").exists() {
                            app.profile.steam_root =
                                Some(self.edit_buffer.clone());
                            app.steam_root = Some(path.clone());
                            app.dayz_path = Some(crate::mods::find_dayz_path(&path));
                            app.workshop_path =
                                Some(crate::mods::find_workshop_path(&path));
                            let _ = app.profile.save(&app.config.profile_path);
                            app.status_message = Some("Steam root updated".into());
                        } else {
                            app.status_message =
                                Some("Invalid Steam root (DayZ not found)".into());
                        }
                    }
                }
                self.edit_buffer.clear();
                Action::None
            }
            KeyCode::Backspace => {
                self.edit_buffer.pop();
                Action::None
            }
            KeyCode::Char(c) => {
                self.edit_buffer.push(c);
                Action::None
            }
            _ => Action::None,
        }
    }

    fn render_edit(&self, f: &mut Frame, area: ratatui::layout::Rect, field: EditField, _app: &App) {
        let label = match field {
            EditField::PlayerName => "Player Name",
            EditField::SteamRoot => "Steam Root Path",
        };

        let lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                format!(" Enter {label}:"),
                theme::TITLE,
            )),
            Line::from(""),
            Line::from(vec![
                Span::raw(" > "),
                Span::styled(&self.edit_buffer, theme::SEARCH_INPUT),
                Span::styled("▌", theme::INFO),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                " Enter: confirm  Esc: cancel",
                theme::KEY_HINT,
            )),
        ];

        let para = Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(theme::INFO)
                    .title(format!(" Edit {label} ")),
            )
            .wrap(Wrap { trim: true });
        f.render_widget(para, area);
    }
}

fn render_about_info(f: &mut Frame, area: ratatui::layout::Rect, app: &App) {
    let lines = vec![
        Line::from(vec![
            Span::styled(" Version: ", theme::DIM),
            Span::styled(crate::config::VERSION, theme::NORMAL),
        ]),
        Line::from(vec![
            Span::styled(" Data: ", theme::DIM),
            Span::styled(app.config.data_dir.display().to_string(), theme::NORMAL),
        ]),
        Line::from(vec![
            Span::styled(" Steam: ", theme::DIM),
            Span::styled(
                app.steam_root
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "Not found".into()),
                theme::NORMAL,
            ),
        ]),
        Line::from(vec![
            Span::styled(" Mods: ", theme::DIM),
            Span::styled(app.mods_db.mods.len().to_string(), theme::NORMAL),
        ]),
    ];

    let para = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(theme::BORDER)
            .title(" Info "),
    );
    f.render_widget(para, area);
}
