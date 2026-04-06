use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};

use super::{Action, Screen, theme};
use crate::app::{App, LaunchTarget};
use crate::offline::sync::runtime_target_name;

pub struct OfflineSetupScreen {
    pub selected_mod_ids: Vec<u64>,
    pub spawn_enabled: bool,
    pub blocked: bool,
    pub mission_id: Option<String>,
    pub runtime_name: Option<String>,
    list_state: ListState,
}

impl OfflineSetupScreen {
    pub fn new() -> Self {
        Self {
            selected_mod_ids: Vec::new(),
            spawn_enabled: false,
            blocked: false,
            mission_id: None,
            runtime_name: None,
            list_state: ListState::default(),
        }
    }

    fn current_mission<'a>(&self, app: &'a App) -> Option<&'a crate::offline::discovery::OfflineMission> {
        let mission_id = self.mission_id.as_deref()?;
        app.offline_missions.iter().find(|mission| mission.id == mission_id)
    }

    fn rebuild_selection(&mut self, app: &App) {
        if !self.selected_mod_ids.is_empty() {
            return;
        }

        if let Some(mission_id) = self.mission_id.as_deref() {
            if let Some(prefs) = app.profile.offline_prefs(mission_id) {
                self.selected_mod_ids = prefs.mod_ids.clone();
                self.spawn_enabled = prefs.spawn_enabled;
                return;
            }
        }

        self.spawn_enabled = self
            .selected_mod_ids
            .is_empty()
            .then_some(false)
            .unwrap_or(self.spawn_enabled);
    }

    fn toggle_selected_mod(&mut self, app: &App) {
        let Some(index) = self.list_state.selected() else {
            return;
        };
        let Some(mod_info) = app.mods_db.mods.get(index) else {
            return;
        };

        if self.selected_mod_ids.contains(&mod_info.id) {
            self.selected_mod_ids.retain(|id| *id != mod_info.id);
        } else {
            self.selected_mod_ids.push(mod_info.id);
        }
    }

    fn selected_runtime_name(&self, app: &App) -> Option<String> {
        if let Some(runtime_name) = &self.runtime_name {
            return Some(runtime_name.clone());
        }

        self.current_mission(app).map(|mission| mission.runtime_name.clone())
    }
}

impl Screen for OfflineSetupScreen {
    fn on_enter(&mut self, app: &mut App) {
        self.blocked = false;
        self.mission_id = None;
        self.runtime_name = None;

        let Some(prep) = app.launch_prep.as_ref() else {
            app.status_message = Some("Offline launch target is unavailable".into());
            self.blocked = true;
            return;
        };

        let LaunchTarget::Offline {
            mission_id,
            runtime_name,
        } = &prep.target
        else {
            app.status_message = Some("Offline launch target is unavailable".into());
            self.blocked = true;
            return;
        };

        self.mission_id = Some(mission_id.clone());
        self.runtime_name = Some(runtime_name.clone());

        if app.dayz_path.is_none() {
            app.status_message = Some("Cannot prepare offline launch: DayZ path is not detected".into());
            app.clear_offline_launch_prep();
            self.blocked = true;
            return;
        }

        self.rebuild_selection(app);

        if self.list_state.selected().is_none() && !app.mods_db.mods.is_empty() {
            self.list_state.select(Some(0));
        }
    }

    fn render(&mut self, f: &mut Frame, app: &App) {
        let area = f.area();
        let chunks = Layout::vertical([
            Constraint::Length(7),
            Constraint::Min(0),
        ])
        .split(area);

        let mission = self.current_mission(app);
        let title = mission
            .map(|mission| mission.name.clone())
            .or_else(|| self.runtime_name.clone())
            .unwrap_or_else(|| "Offline mission".into());
        let runtime = self
            .selected_runtime_name(app)
            .map(|runtime| runtime_target_name(&runtime))
            .unwrap_or_else(|| "unknown".into());

        let mut lines = vec![
            Line::from(""),
            Line::from(Span::styled(format!(" {title}"), theme::TITLE)),
            Line::from(""),
            Line::from(Span::styled(format!(" Runtime: {runtime}"), theme::NORMAL)),
            Line::from(Span::styled(
                format!(" Selected mods: {}", self.selected_mod_ids.len()),
                theme::DIM,
            )),
            Line::from(Span::styled(
                format!(
                    " Spawn enabled: {}",
                    if self.spawn_enabled { "yes" } else { "no" }
                ),
                theme::DIM,
            )),
        ];
        if self.blocked {
            lines.push(Line::from(Span::styled(
                " DayZ path is not available for offline runtime sync",
                theme::WARNING,
            )));
        }

        let para = Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(theme::BORDER)
                    .title(" Offline Setup "),
            )
            .wrap(Wrap { trim: true });
        f.render_widget(para, chunks[0]);

        let mods: Vec<ListItem> = app
            .mods_db
            .mods
            .iter()
            .map(|mod_info| {
                let checked = if self.selected_mod_ids.contains(&mod_info.id) {
                    "[x]"
                } else {
                    "[ ]"
                };
                ListItem::new(format!("  {checked} {}", mod_info.name))
            })
            .collect();

        let list = List::new(mods)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(theme::BORDER)
                    .title(" Mods "),
            )
            .highlight_style(theme::SELECTED)
            .highlight_symbol("▶ ");
        f.render_stateful_widget(list, chunks[1], &mut self.list_state);
    }

    fn handle_key(&mut self, key: KeyEvent, app: &mut App) -> Action {
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            app.clear_offline_launch_prep();
            return Action::PopScreen;
        }

        if self.blocked {
            return match key.code {
                KeyCode::Esc | KeyCode::Enter => Action::PopScreen,
                _ => Action::None,
            };
        }

        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                app.clear_offline_launch_prep();
                Action::PopScreen
            }
            KeyCode::Up | KeyCode::Char('k') => {
                let count = app.mods_db.mods.len();
                if count == 0 {
                    return Action::None;
                }
                let current = self.list_state.selected().unwrap_or(0);
                let next = if current == 0 { count - 1 } else { current - 1 };
                self.list_state.select(Some(next));
                Action::None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let count = app.mods_db.mods.len();
                if count == 0 {
                    return Action::None;
                }
                let current = self.list_state.selected().unwrap_or(0);
                let next = (current + 1) % count;
                self.list_state.select(Some(next));
                Action::None
            }
            KeyCode::Char(' ') => {
                self.toggle_selected_mod(app);
                Action::None
            }
            KeyCode::Char('s') => {
                self.spawn_enabled = !self.spawn_enabled;
                Action::None
            }
            KeyCode::Enter => {
                let Some(mission_id) = self.mission_id.clone() else {
                    return Action::None;
                };
                app.save_offline_preferences(&mission_id, self.selected_mod_ids.clone(), self.spawn_enabled);

                if let Some(prep) = app.launch_prep.as_mut() {
                    prep.mod_ids = self.selected_mod_ids.clone();
                    prep.offline_spawn_enabled = Some(self.spawn_enabled);
                }

                Action::LaunchGame
            }
            _ => Action::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{LaunchPrep, LaunchTarget};
    use crate::config::Config;
    use crate::offline::types::{MissionSource, OfflineMissionPrefs};
    use crate::profile::Profile;
    use std::path::PathBuf;

    fn test_app() -> App {
        let data_dir = std::env::temp_dir().join("dayz-cmd-tests-offline-setup");
        App::new(
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
        )
    }

    fn offline_prep(mission_id: &str, runtime_name: &str) -> LaunchPrep {
        LaunchPrep {
            target: LaunchTarget::Offline {
                mission_id: mission_id.into(),
                runtime_name: runtime_name.into(),
            },
            mod_ids: Vec::new(),
            password: None,
            offline_spawn_enabled: None,
        }
    }

    #[test]
    fn offline_setup_preloads_remembered_per_mission_preferences() {
        let mission_id = "managed:DayZCommunityOfflineMode.ChernarusPlus";
        let mut app = test_app();
        app.profile.offline.insert(
            mission_id.into(),
            OfflineMissionPrefs {
                mod_ids: vec![101, 202],
                spawn_enabled: true,
            },
        );
        app.launch_prep = Some(offline_prep(mission_id, "DayZCommunityOfflineMode.ChernarusPlus"));
        app.dayz_path = Some(PathBuf::from("/tmp/dayz"));
        app.offline_missions = vec![crate::offline::discovery::OfflineMission {
            id: mission_id.into(),
            name: "Alpha".into(),
            source: MissionSource::Managed,
            source_path: PathBuf::from("/tmp/managed/Alpha"),
            runtime_name: "DayZCommunityOfflineMode.ChernarusPlus".into(),
        }];
        let mut screen = OfflineSetupScreen::new();

        screen.on_enter(&mut app);

        assert_eq!(screen.selected_mod_ids, vec![101, 202]);
        assert!(screen.spawn_enabled);
    }

    #[test]
    fn offline_setup_blocks_early_when_dayz_path_is_unavailable() {
        let mission_id = "managed:DayZCommunityOfflineMode.ChernarusPlus";
        let mut app = test_app();
        app.launch_prep = Some(offline_prep(mission_id, "DayZCommunityOfflineMode.ChernarusPlus"));
        app.offline_missions = vec![crate::offline::discovery::OfflineMission {
            id: mission_id.into(),
            name: "Alpha".into(),
            source: MissionSource::Managed,
            source_path: PathBuf::from("/tmp/managed/Alpha"),
            runtime_name: "DayZCommunityOfflineMode.ChernarusPlus".into(),
        }];
        let mut screen = OfflineSetupScreen::new();

        screen.on_enter(&mut app);

        assert!(screen.blocked);
        assert!(
            app.status_message
                .as_deref()
                .unwrap_or_default()
                .contains("DayZ path")
        );
    }

    #[test]
    fn offline_setup_stores_selected_mods_and_spawn_toggle_into_shared_launch_prep() {
        let mission_id = "managed:DayZCommunityOfflineMode.ChernarusPlus";
        let mut app = test_app();
        app.dayz_path = Some(PathBuf::from("/tmp/dayz"));
        app.profile.offline.insert(
            mission_id.into(),
            OfflineMissionPrefs {
                mod_ids: vec![111],
                spawn_enabled: false,
            },
        );
        app.launch_prep = Some(offline_prep(mission_id, "DayZCommunityOfflineMode.ChernarusPlus"));
        app.offline_missions = vec![crate::offline::discovery::OfflineMission {
            id: mission_id.into(),
            name: "Alpha".into(),
            source: MissionSource::Managed,
            source_path: PathBuf::from("/tmp/managed/Alpha"),
            runtime_name: "DayZCommunityOfflineMode.ChernarusPlus".into(),
        }];
        let mut screen = OfflineSetupScreen::new();

        screen.on_enter(&mut app);
        screen.selected_mod_ids = vec![111, 222];
        screen.spawn_enabled = true;
        let action = screen.handle_key(KeyEvent::from(KeyCode::Enter), &mut app);

        assert_eq!(action, Action::LaunchGame);
        assert_eq!(
            app.launch_prep
                .as_ref()
                .map(|prep| prep.mod_ids.clone())
                .unwrap_or_default(),
            vec![111, 222]
        );
        assert_eq!(
            app.launch_prep
                .as_ref()
                .and_then(|prep| prep.offline_spawn_enabled),
            Some(true)
        );
        assert_eq!(
            app.profile
                .offline_prefs(mission_id)
                .map(|prefs| prefs.mod_ids.clone()),
            Some(vec![111, 222])
        );
    }
}
