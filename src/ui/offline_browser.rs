use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};

use super::{Action, Screen, ScreenId, theme};
use crate::app::App;
use crate::offline::discovery::OfflineMission;

pub struct OfflineBrowserScreen {
    pub list_state: ListState,
    pub missions: Vec<OfflineMission>,
    items: Vec<BrowserItem>,
    release_label: Option<String>,
    metadata_error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum BrowserItem {
    InstallOrUpdate,
    Mission(usize),
}

impl OfflineBrowserScreen {
    pub fn new() -> Self {
        Self {
            list_state: ListState::default(),
            missions: Vec::new(),
            items: Vec::new(),
            release_label: None,
            metadata_error: None,
        }
    }

    fn rebuild_items(&mut self, app: &App) {
        self.missions = app.offline_missions.clone();
        self.release_label = app
            .offline_release
            .as_ref()
            .map(|release| format!("Install/Update {}", release.tag));
        self.metadata_error = app.offline_release_error.clone();

        let mut items = Vec::new();
        if self.release_label.is_some() {
            items.push(BrowserItem::InstallOrUpdate);
        }
        items.extend((0..self.missions.len()).map(BrowserItem::Mission));
        self.items = items;

        if self.list_state.selected().is_none() && !self.items.is_empty() {
            self.list_state.select(Some(0));
        }
    }

    fn selected_item(&self) -> Option<&BrowserItem> {
        let index = self.list_state.selected()?;
        self.items.get(index)
    }

    fn render_overview(&self, f: &mut Frame, area: Rect) {
        let mut lines = vec![
            Line::from(""),
            Line::from(Span::styled(" Offline Mode", theme::TITLE)),
            Line::from(""),
            Line::from(Span::styled(
                format!(" Missions discovered: {}", self.missions.len()),
                theme::NORMAL,
            )),
        ];

        if let Some(label) = &self.release_label {
            lines.push(Line::from(Span::styled(label, theme::INFO)));
        }

        if let Some(error) = &self.metadata_error {
            lines.push(Line::from(Span::styled(error, theme::WARNING)));
        }

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            " Enter: setup/install  Esc: back",
            theme::KEY_HINT,
        )));

        let block = Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(theme::BORDER)
                    .title(" Offline Mode "),
            )
            .wrap(Wrap { trim: true });
        f.render_widget(block, area);
    }
}

impl Screen for OfflineBrowserScreen {
    fn on_enter(&mut self, app: &mut App) {
        app.refresh_offline_browser();
        self.rebuild_items(app);
    }

    fn render(&mut self, f: &mut Frame, _app: &App) {
        let area = f.area();
        let chunks = ratatui::layout::Layout::vertical([
            ratatui::layout::Constraint::Length(7),
            ratatui::layout::Constraint::Min(0),
        ])
        .split(area);

        self.render_overview(f, chunks[0]);

        let items: Vec<ListItem> = self
            .items
            .iter()
            .map(|item| {
                let label = match item {
                    BrowserItem::InstallOrUpdate => self
                        .release_label
                        .as_deref()
                        .unwrap_or("Install/Update Offline Mode"),
                    BrowserItem::Mission(index) => {
                        self.missions.get(*index).map(|mission| mission.name.as_str()).unwrap_or("Unknown mission")
                    }
                };
                ListItem::new(format!("  {}", label))
            })
            .collect();

        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(theme::BORDER)
                    .title(" Missions "),
            )
            .highlight_style(theme::SELECTED)
            .highlight_symbol("▶ ");
        f.render_stateful_widget(list, chunks[1], &mut self.list_state);
    }

    fn handle_key(&mut self, key: KeyEvent, app: &mut App) -> Action {
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return Action::PopScreen;
        }

        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => Action::PopScreen,
            KeyCode::Up | KeyCode::Char('k') => {
                let current = self.list_state.selected().unwrap_or(0);
                let next = if current == 0 {
                    self.items.len().saturating_sub(1)
                } else {
                    current - 1
                };
                if !self.items.is_empty() {
                    self.list_state.select(Some(next));
                }
                Action::None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let current = self.list_state.selected().unwrap_or(0);
                let next = if self.items.is_empty() {
                    0
                } else {
                    (current + 1) % self.items.len()
                };
                if !self.items.is_empty() {
                    self.list_state.select(Some(next));
                }
                Action::None
            }
            KeyCode::Enter => match self.selected_item().cloned() {
                Some(BrowserItem::InstallOrUpdate) => Action::OfflineInstallOrUpdate,
                Some(BrowserItem::Mission(index)) => {
                    if let Some(mission) = self.missions.get(index).cloned() {
                        app.prepare_offline_launch(&mission.id);
                        if app.launch_prep.is_some() {
                            return Action::PushScreen(ScreenId::OfflineSetup);
                        }
                    }
                    Action::None
                }
                None => Action::None,
            },
            _ => Action::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;
    use crate::config::Config;
    use crate::offline::storage::{release_dir_for_tag, save_offline_state};
    use crate::offline::types::{MissionSource, OfflineState};
    use crate::profile::Profile;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_app(root: &Path) -> App {
        App::new(
            Config {
                path: root.join("dayz-cmd.conf"),
                data_dir: root.to_path_buf(),
                server_db_path: root.join("servers.json"),
                news_db_path: root.join("news.json"),
                mods_db_path: root.join("mods.json"),
                profile_path: root.join("profile.json"),
                api_url: "https://example.test/api".into(),
                github_owner: "owner".into(),
                github_repo: "repo".into(),
                request_timeout: 1,
                server_request_timeout: 1,
                server_db_ttl: 1,
                news_db_ttl: 1,
                history_size: 5,
                steamcmd_enabled: true,
                filter_mod_limit: 10,
                filter_players_limit: 50,
                filter_players_slots: 60,
                applications_dir: root.join("applications"),
            },
            Profile::default(),
        )
    }

    fn test_root(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "dayz-cmd-{name}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time before unix epoch")
                .as_nanos()
        ))
    }

    fn install_managed_release(config: &Config, tag: &str, missions: &[&str]) {
        let release_dir = release_dir_for_tag(config, tag);
        for mission in missions {
            fs::create_dir_all(release_dir.join(format!("Missions/{mission}/core")))
                .expect("create managed mission");
            fs::write(
                release_dir.join(format!("Missions/{mission}/core/CommunityOfflineClient.c")),
                "HIVE_ENABLED = true;",
            )
            .expect("write managed mission");
        }
    }

    fn install_existing_missions(root: &Path, missions: &[&str]) {
        for mission in missions {
            fs::create_dir_all(root.join(format!("DayZ/Missions/{mission}/core")))
                .expect("create existing mission");
            fs::write(
                root.join(format!(
                    "DayZ/Missions/{mission}/core/CommunityOfflineClient.c"
                )),
                "HIVE_ENABLED = true;",
            )
            .expect("write existing mission");
        }
    }

    fn no_release(_: u64) -> anyhow::Result<Option<crate::api::offline_releases::ReleaseInfo>> {
        Ok(None)
    }

    fn failing_release_fetcher(
        _: u64,
    ) -> anyhow::Result<Option<crate::api::offline_releases::ReleaseInfo>> {
        anyhow::bail!("github metadata unavailable")
    }

    #[test]
    fn offline_browser_shows_discovered_managed_and_existing_missions() {
        let root = test_root("offline-browser-discovery");
        fs::create_dir_all(&root).expect("create temp root");
        let config = Config {
            path: root.join("dayz-cmd.conf"),
            data_dir: root.clone(),
            server_db_path: root.join("servers.json"),
            news_db_path: root.join("news.json"),
            mods_db_path: root.join("mods.json"),
            profile_path: root.join("profile.json"),
            api_url: "https://example.test/api".into(),
            github_owner: "owner".into(),
            github_repo: "repo".into(),
            request_timeout: 1,
            server_request_timeout: 1,
            server_db_ttl: 1,
            news_db_ttl: 1,
            history_size: 5,
            steamcmd_enabled: true,
            filter_mod_limit: 10,
            filter_players_limit: 50,
            filter_players_slots: 60,
            applications_dir: root.join("applications"),
        };
        install_managed_release(&config, "v1.0.0", &["Alpha"]);
        install_existing_missions(&root, &["Charlie"]);
        save_offline_state(
            &config,
            &OfflineState {
                installed_tag: None,
                latest_known_tag: Some("v1.0.0".into()),
                managed_missions: vec!["Alpha".into()],
                last_check_ts: None,
            },
        )
        .expect("save offline state");

        let mut app = test_app(&root);
        app.dayz_path = Some(root.join("DayZ"));
        app.offline_release_fetcher = no_release;
        let mut screen = OfflineBrowserScreen::new();

        screen.on_enter(&mut app);

        assert!(screen
            .missions
            .iter()
            .any(|mission| mission.source == MissionSource::Managed));
        assert!(screen
            .missions
            .iter()
            .any(|mission| mission.source == MissionSource::Existing));
        assert!(screen.missions.iter().any(|mission| mission.name == "Alpha"));
        assert!(screen
            .missions
            .iter()
            .any(|mission| mission.name == "Charlie"));
    }

    #[test]
    fn offline_browser_still_opens_when_metadata_fetch_fails() {
        let root = test_root("offline-browser-fetch-failure");
        fs::create_dir_all(&root).expect("create temp root");
        let config = Config {
            path: root.join("dayz-cmd.conf"),
            data_dir: root.clone(),
            server_db_path: root.join("servers.json"),
            news_db_path: root.join("news.json"),
            mods_db_path: root.join("mods.json"),
            profile_path: root.join("profile.json"),
            api_url: "https://example.test/api".into(),
            github_owner: "owner".into(),
            github_repo: "repo".into(),
            request_timeout: 1,
            server_request_timeout: 1,
            server_db_ttl: 1,
            news_db_ttl: 1,
            history_size: 5,
            steamcmd_enabled: true,
            filter_mod_limit: 10,
            filter_players_limit: 50,
            filter_players_slots: 60,
            applications_dir: root.join("applications"),
        };
        install_managed_release(&config, "v1.0.0", &["Alpha"]);
        save_offline_state(
            &config,
            &OfflineState {
                installed_tag: None,
                latest_known_tag: Some("v1.0.0".into()),
                managed_missions: vec!["Alpha".into()],
                last_check_ts: None,
            },
        )
        .expect("save offline state");

        let mut app = test_app(&root);
        app.dayz_path = Some(root.join("DayZ"));
        app.offline_release_fetcher = failing_release_fetcher;
        let mut screen = OfflineBrowserScreen::new();

        screen.on_enter(&mut app);

        assert!(!screen.missions.is_empty());
        assert!(app
            .status_message
            .as_deref()
            .unwrap_or_default()
            .contains("github metadata unavailable"));
    }
}
