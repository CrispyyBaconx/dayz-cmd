use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};

use super::{Action, ConfirmAction, Screen, ScreenId, theme};
use crate::app::App;

pub struct ConfigScreen {
    pub list_state: ListState,
    pub launch_options_state: ListState,
    pub editing: Option<EditField>,
    pub edit_buffer: String,
    pub show_launch_options: bool,
    launch_option_keys: Vec<String>,
    items: Vec<ConfigItem>,
}

#[derive(Debug, Clone)]
enum ConfigItem {
    LaunchOptions,
    PlayerName,
    SteamRoot,
    MigrateLegacyData,
    InstalledMods,
    RemoveManagedMods,
    RemoveModLinks,
    RefreshInstalledMods,
    RefreshServers,
    CheckForUpdates,
    About,
}

#[derive(Debug, Clone)]
pub enum EditField {
    PlayerName,
    SteamRoot,
    LaunchOptionValue(String),
}

impl ConfigScreen {
    pub fn new() -> Self {
        Self {
            list_state: ListState::default().with_selected(Some(0)),
            launch_options_state: ListState::default().with_selected(Some(0)),
            editing: None,
            edit_buffer: String::new(),
            show_launch_options: false,
            launch_option_keys: Vec::new(),
            items: Vec::new(),
        }
    }

    fn build_items(&mut self, app: &App) {
        let mut items = vec![
            ConfigItem::LaunchOptions,
            ConfigItem::PlayerName,
            ConfigItem::SteamRoot,
        ];

        if crate::config::has_legacy_data() {
            items.push(ConfigItem::MigrateLegacyData);
        }

        if !app.mods_db.mods.is_empty() {
            items.push(ConfigItem::InstalledMods);
            items.push(ConfigItem::RefreshInstalledMods);
            items.push(ConfigItem::RemoveManagedMods);
            items.push(ConfigItem::RemoveModLinks);
        }

        items.push(ConfigItem::RefreshServers);
        items.push(ConfigItem::CheckForUpdates);
        items.push(ConfigItem::About);

        self.items = items;
    }

    fn label(item: &ConfigItem) -> &str {
        match item {
            ConfigItem::LaunchOptions => "Game Launch Options",
            ConfigItem::PlayerName => "Change Player Name",
            ConfigItem::SteamRoot => "Change Steam Root Dir",
            ConfigItem::MigrateLegacyData => "Migrate from dayz-ctl",
            ConfigItem::InstalledMods => "Installed Mod Info",
            ConfigItem::RemoveManagedMods => "Remove Managed Mods",
            ConfigItem::RemoveModLinks => "Remove All Mod Links",
            ConfigItem::RefreshInstalledMods => "Refresh Installed Mods",
            ConfigItem::RefreshServers => "Refresh Server List",
            ConfigItem::CheckForUpdates => "Check for Updates",
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

        if self.show_launch_options {
            self.render_launch_options(f, area, app);
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

        if self.show_launch_options {
            return self.handle_launch_options_key(key, app);
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
                self.edit_buffer = app.profile.player.clone().unwrap_or_default();
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
                self.show_launch_options = true;
                self.launch_option_keys = app.profile.options.keys().cloned().collect();
                self.launch_options_state.select(Some(0));
                Action::None
            }
            ConfigItem::MigrateLegacyData => {
                Action::PushScreen(ScreenId::Confirm(ConfirmAction::MigrateLegacy))
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
                if app.workshop_path.is_some() && app.dayz_path.is_some() {
                    Action::PushScreen(ScreenId::Confirm(ConfirmAction::RemoveManagedMods))
                } else {
                    app.status_message = Some("Steam library path not detected".into());
                    Action::None
                }
            }
            ConfigItem::RemoveModLinks => {
                if app.dayz_path.is_some() {
                    Action::PushScreen(ScreenId::Confirm(ConfirmAction::RemoveModLinks))
                } else {
                    app.status_message = Some("Steam library path not detected".into());
                    Action::None
                }
            }
            ConfigItem::RefreshInstalledMods => Action::RefreshInstalledMods,
            ConfigItem::RefreshServers => {
                app.status_message = Some("Refreshing server list...".into());
                app.refresh_servers();
                Action::None
            }
            ConfigItem::CheckForUpdates => Action::CheckForUpdates,
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
                            app.profile.steam_root = Some(self.edit_buffer.clone());
                            app.steam_root = Some(path.clone());
                            app.dayz_path = Some(crate::mods::find_dayz_path(&path));
                            app.workshop_path = Some(crate::mods::find_workshop_path(&path));
                            let _ = app.profile.save(&app.config.profile_path);
                            app.status_message = Some("Steam root updated".into());
                        } else {
                            app.status_message = Some("Invalid Steam root (DayZ not found)".into());
                        }
                    }
                    EditField::LaunchOptionValue(key) => {
                        if app
                            .profile
                            .set_option_value(&key, &self.edit_buffer)
                            .is_some()
                        {
                            let _ = app.profile.save(&app.config.profile_path);
                            app.status_message = Some(format!("Updated launch option '{key}'"));
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

    fn render_edit(
        &self,
        f: &mut Frame,
        area: ratatui::layout::Rect,
        field: EditField,
        _app: &App,
    ) {
        let label = match field {
            EditField::PlayerName => "Player Name",
            EditField::SteamRoot => "Steam Root Path",
            EditField::LaunchOptionValue(_) => "Launch Option Value",
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

    fn render_launch_options(&mut self, f: &mut Frame, area: ratatui::layout::Rect, app: &App) {
        let items: Vec<ListItem> = self
            .launch_option_keys
            .iter()
            .filter_map(|key| app.profile.options.get(key).map(|option| (key, option)))
            .map(|(key, option)| {
                let enabled = if option.enabled { "[x]" } else { "[ ]" };
                let value = option
                    .value
                    .as_ref()
                    .map(format_option_value)
                    .unwrap_or_else(|| "-".into());
                ListItem::new(vec![
                    Line::from(format!("  {enabled} -{key} = {value}")),
                    Line::from(Span::styled(
                        format!("    {}", option.description),
                        theme::DIM,
                    )),
                ])
            })
            .collect();

        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(theme::BORDER)
                    .title(" Launch Options (Space: toggle, Enter: edit, Esc: back) "),
            )
            .highlight_style(theme::SELECTED)
            .highlight_symbol("▶ ");

        f.render_stateful_widget(list, area, &mut self.launch_options_state);
    }

    fn handle_launch_options_key(&mut self, key: KeyEvent, app: &mut App) -> Action {
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.show_launch_options = false;
            return Action::None;
        }

        match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Backspace => {
                self.show_launch_options = false;
                Action::None
            }
            KeyCode::Up | KeyCode::Char('k') => {
                let i = self.launch_options_state.selected().unwrap_or(0);
                let new = if i == 0 {
                    self.launch_option_keys.len().saturating_sub(1)
                } else {
                    i - 1
                };
                self.launch_options_state.select(Some(new));
                Action::None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let i = self.launch_options_state.selected().unwrap_or(0);
                let len = self.launch_option_keys.len();
                let new = if len == 0 { 0 } else { (i + 1) % len };
                self.launch_options_state.select(Some(new));
                Action::None
            }
            KeyCode::Char(' ') => {
                if let Some(key) = self.selected_launch_option_key() {
                    if app.profile.toggle_option(&key).is_some() {
                        let _ = app.profile.save(&app.config.profile_path);
                    }
                }
                Action::None
            }
            KeyCode::Enter => {
                if let Some(key) = self.selected_launch_option_key() {
                    let current = app
                        .profile
                        .options
                        .get(&key)
                        .and_then(|option| option.value.as_ref().map(format_option_value))
                        .unwrap_or_default();
                    self.editing = Some(EditField::LaunchOptionValue(key));
                    self.edit_buffer = current;
                }
                Action::None
            }
            _ => Action::None,
        }
    }

    fn selected_launch_option_key(&self) -> Option<String> {
        self.launch_options_state
            .selected()
            .and_then(|idx| self.launch_option_keys.get(idx))
            .cloned()
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

fn format_option_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(value) => value.clone(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::profile::Profile;
    use crate::ui::{ConfirmAction, ScreenId};
    use std::path::PathBuf;

    fn test_app() -> App {
        let data_dir = std::env::temp_dir().join("dayz-cmd-tests-config-screen");
        let mut app = App::new(
            Config {
                path: data_dir.join("dayz-cmd.conf"),
                data_dir: data_dir.clone(),
                server_db_path: data_dir.join("servers.json"),
                news_db_path: data_dir.join("news.json"),
                mods_db_path: data_dir.join("mods.json"),
                profile_path: data_dir.join("profile.json"),
                api_url: "https://example.test".into(),
                github_owner: "example".into(),
                github_repo: "dayz-cmd".into(),
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
        );
        app.dayz_path = Some(data_dir.join("dayz"));
        app.workshop_path = Some(data_dir.join("workshop"));
        app
    }

    #[test]
    fn destructive_config_actions_require_confirmation() {
        let mut screen = ConfigScreen::new();
        let mut app = test_app();

        assert_eq!(
            screen.execute_item(ConfigItem::RemoveManagedMods, &mut app),
            Action::PushScreen(ScreenId::Confirm(ConfirmAction::RemoveManagedMods))
        );
        assert_eq!(
            screen.execute_item(ConfigItem::RemoveModLinks, &mut app),
            Action::PushScreen(ScreenId::Confirm(ConfirmAction::RemoveModLinks))
        );
    }

    #[test]
    fn config_exposes_manual_update_check_action() {
        let mut screen = ConfigScreen::new();
        let mut app = test_app();

        assert_eq!(
            screen.execute_item(ConfigItem::CheckForUpdates, &mut app),
            Action::CheckForUpdates
        );
    }

    #[test]
    fn config_routes_refresh_installed_mods_action() {
        let mut screen = ConfigScreen::new();
        let mut app = test_app();

        assert_eq!(
            screen.execute_item(ConfigItem::RefreshInstalledMods, &mut app),
            Action::RefreshInstalledMods
        );
    }
}
