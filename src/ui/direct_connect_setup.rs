use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Frame;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};

use super::{Action, Screen, ScreenId, theme};
use crate::app::{App, LaunchTarget};

pub struct DirectConnectSetupScreen {
    pub selected_mod_ids: Vec<u64>,
    list_state: ListState,
}

impl DirectConnectSetupScreen {
    pub fn new() -> Self {
        Self {
            selected_mod_ids: Vec::new(),
            list_state: ListState::default(),
        }
    }

    fn installed_mod_ids(app: &App) -> Vec<u64> {
        app.mods_db.mods.iter().map(|m| m.id).collect()
    }

    fn selected_mod_id<'a>(&self, app: &'a App) -> Option<u64> {
        let index = self.list_state.selected()?;
        app.mods_db.mods.get(index).map(|mod_info| mod_info.id)
    }

    fn toggle_selected_mod(&mut self, app: &mut App) {
        let Some(mod_id) = self.selected_mod_id(app) else {
            return;
        };

        if self.selected_mod_ids.contains(&mod_id) {
            self.selected_mod_ids.retain(|selected| *selected != mod_id);
        } else {
            self.selected_mod_ids.push(mod_id);
        }
    }
}

impl Screen for DirectConnectSetupScreen {
    fn on_enter(&mut self, app: &mut App) {
        if self.selected_mod_ids.is_empty() {
            self.selected_mod_ids = app
                .launch_prep
                .as_ref()
                .map(|prep| prep.mod_ids.clone())
                .unwrap_or_default();
        }

        if self.list_state.selected().is_none() && !app.mods_db.mods.is_empty() {
            self.list_state.select(Some(0));
        }
    }

    fn render(&mut self, f: &mut Frame, app: &App) {
        let area = f.area();
        let mods = Self::installed_mod_ids(app);
        let target = match app.launch_prep.as_ref().map(|prep| &prep.target) {
            Some(LaunchTarget::DirectConnect { ip, port }) => format!("{ip}:{port}"),
            _ => "unknown target".to_string(),
        };

        let lines = vec![
            Line::from(""),
            Line::from(Span::styled(" Direct Connect Setup", theme::TITLE)),
            Line::from(""),
            Line::from(Span::styled(format!(" Target: {target}"), theme::NORMAL)),
            Line::from(""),
            Line::from(Span::styled(
                format!(" Installed mods: {}", mods.len()),
                theme::DIM,
            )),
            Line::from(""),
            Line::from(Span::styled(
                " Space: toggle mod  P: password  Enter: launch  Esc: cancel",
                theme::KEY_HINT,
            )),
        ];

        let para = Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(theme::BORDER)
                    .title(" Direct Connect Setup "),
            )
            .wrap(Wrap { trim: true });
        f.render_widget(para, area);

        let items: Vec<ListItem> = mods
            .iter()
            .map(|id| {
                let selected = if self.selected_mod_ids.contains(id) {
                    "[x]"
                } else {
                    "[ ]"
                };
                ListItem::new(format!("  {selected} @{id}"))
            })
            .collect();

        let list_area = ratatui::layout::Rect::new(
            area.x + 2,
            area.y + 8,
            area.width.saturating_sub(4),
            area.height.saturating_sub(10),
        );
        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(theme::BORDER),
            )
            .highlight_style(theme::SELECTED)
            .highlight_symbol("▶ ");
        f.render_stateful_widget(list, list_area, &mut self.list_state);
    }

    fn handle_key(&mut self, key: KeyEvent, app: &mut App) -> Action {
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return Action::PopScreen;
        }

        match key.code {
            KeyCode::Esc => {
                app.clear_direct_connect_launch_prep();
                Action::PopScreen
            }
            KeyCode::Up | KeyCode::Char('k') => {
                let Some(count) = (!app.mods_db.mods.is_empty()).then_some(app.mods_db.mods.len())
                else {
                    return Action::None;
                };
                let current = self.list_state.selected().unwrap_or(0);
                let next = if current == 0 { count - 1 } else { current - 1 };
                self.list_state.select(Some(next));
                Action::None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let Some(count) = (!app.mods_db.mods.is_empty()).then_some(app.mods_db.mods.len())
                else {
                    return Action::None;
                };
                let current = self.list_state.selected().unwrap_or(0);
                let next = (current + 1) % count;
                self.list_state.select(Some(next));
                Action::None
            }
            KeyCode::Char(' ') => {
                self.toggle_selected_mod(app);
                Action::None
            }
            KeyCode::Char('p') => Action::PushScreen(ScreenId::PasswordPrompt),
            KeyCode::Enter => {
                if let Some(prep) = app.launch_prep.as_mut() {
                    prep.mod_ids = self.selected_mod_ids.clone();
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
    use crate::mods::ModInfo;
    use crate::mods::ModsDb;
    use crate::profile::Profile;
    use crate::ui::password_prompt::PasswordPromptScreen;
    use std::path::PathBuf;

    fn test_app() -> App {
        let data_dir = std::env::temp_dir().join("dayz-cmd-tests-direct-connect-setup");
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

    fn prep(ip: &str, port: u16) -> LaunchPrep {
        LaunchPrep {
            target: LaunchTarget::DirectConnect {
                ip: ip.into(),
                port,
            },
            mod_ids: Vec::new(),
            password: None,
            offline_spawn_enabled: None,
        }
    }

    #[test]
    fn toggles_installed_mod_ids_locally_until_confirm() {
        let mut app = test_app();
        app.mods_db = ModsDb {
            sum: String::new(),
            mods: vec![
                ModInfo {
                    name: "Mod 111".into(),
                    id: 111,
                    timestamp: 0,
                    size: 0,
                },
                ModInfo {
                    name: "Mod 222".into(),
                    id: 222,
                    timestamp: 0,
                    size: 0,
                },
            ],
        };
        app.launch_prep = Some(prep("5.6.7.8", 2402));
        let mut screen = DirectConnectSetupScreen::new();
        screen.on_enter(&mut app);

        let action = screen.handle_key(
            KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
            &mut app,
        );

        assert_eq!(action, Action::None);
        assert_eq!(screen.selected_mod_ids, vec![111]);
        assert_eq!(
            app.launch_prep.as_ref().map(|prep| prep.mod_ids.clone()),
            Some(Vec::new())
        );
    }

    #[test]
    fn password_prompt_flow_returns_to_setup_after_storing_password() {
        let mut app = test_app();
        app.mods_db = ModsDb {
            sum: String::new(),
            mods: vec![
                ModInfo {
                    name: "Mod 111".into(),
                    id: 111,
                    timestamp: 0,
                    size: 0,
                },
                ModInfo {
                    name: "Mod 222".into(),
                    id: 222,
                    timestamp: 0,
                    size: 0,
                },
            ],
        };
        app.launch_prep = Some(prep("5.6.7.8", 2402));
        let mut screen = DirectConnectSetupScreen::new();
        screen.on_enter(&mut app);

        let toggle_action = screen.handle_key(
            KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
            &mut app,
        );
        assert_eq!(toggle_action, Action::None);

        let action = screen.handle_key(
            KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE),
            &mut app,
        );

        assert_eq!(action, Action::PushScreen(ScreenId::PasswordPrompt));

        let mut prompt = PasswordPromptScreen::new();
        for ch in "secret".chars() {
            let _ = prompt.handle_key(
                KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE),
                &mut app,
            );
        }
        let prompt_action =
            prompt.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &mut app);

        assert_eq!(prompt_action, Action::PopScreen);
        assert_eq!(
            app.launch_prep
                .as_ref()
                .and_then(|prep| prep.password.as_deref()),
            Some("secret")
        );

        let launch_action = screen.handle_key(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            &mut app,
        );

        assert_eq!(launch_action, Action::LaunchGame);
        assert_eq!(
            app.launch_prep.as_ref().map(|prep| prep.mod_ids.clone()),
            Some(vec![111])
        );
        assert_eq!(
            app.launch_prep
                .as_ref()
                .and_then(|prep| prep.password.as_deref()),
            Some("secret")
        );
    }

    #[test]
    fn escape_clears_pending_direct_connect_launch_prep() {
        let mut app = test_app();
        app.launch_prep = Some(prep("5.6.7.8", 2402));
        let mut screen = DirectConnectSetupScreen::new();
        screen.on_enter(&mut app);

        let action = screen.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &mut app);

        assert_eq!(action, Action::PopScreen);
        assert!(app.launch_prep.is_none());
    }

    #[test]
    fn canceling_password_prompt_leaves_setup_active_and_selection_intact() {
        let mut app = test_app();
        app.mods_db = ModsDb {
            sum: String::new(),
            mods: vec![ModInfo {
                name: "Mod 111".into(),
                id: 111,
                timestamp: 0,
                size: 0,
            }],
        };
        app.launch_prep = Some(prep("5.6.7.8", 2402));
        let mut screen = DirectConnectSetupScreen::new();
        screen.on_enter(&mut app);
        let _ = screen.handle_key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE), &mut app);

        let action = screen.handle_key(
            KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE),
            &mut app,
        );
        assert_eq!(action, Action::PushScreen(ScreenId::PasswordPrompt));

        let mut prompt = PasswordPromptScreen::new();
        let cancel_action =
            prompt.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &mut app);

        assert_eq!(cancel_action, Action::PopScreen);
        assert!(app.launch_prep.is_some());
        assert!(
            app.launch_prep
                .as_ref()
                .and_then(|prep| prep.password.as_ref())
                .is_none()
        );

        screen.on_enter(&mut app);
        assert_eq!(screen.selected_mod_ids, vec![111]);

        let launch_action =
            screen.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &mut app);

        assert_eq!(launch_action, Action::LaunchGame);
        assert_eq!(
            app.launch_prep.as_ref().map(|prep| prep.mod_ids.clone()),
            Some(vec![111])
        );
    }
}
