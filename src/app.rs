use ratatui::Frame;
use std::collections::HashMap;
use std::path::PathBuf;

use crate::api::news::NewsArticle;
use crate::api::releases::ReleaseInfo;
use crate::config::Config;
use crate::mods::ModsDb;
use crate::profile::Profile;
use crate::server::Server;
use crate::steam::{ItemState, SteamHandle};
use crate::ui::server_browser::{BrowseSource, ServerBrowserScreen};
use crate::ui::*;

struct PendingLaunch {
    args: Vec<String>,
    all_mod_ids: Vec<u64>,
    pending_mod_ids: Vec<u64>,
    history_entry: Option<(String, String, u16)>,
}

#[derive(Clone, Debug, PartialEq)]
struct PendingDownloadStatus {
    workshop_id: u64,
    state: ItemState,
    progress: Option<(u64, u64)>,
}

pub struct App {
    pub running: bool,
    pub config: Config,
    pub profile: Profile,
    pub servers: Vec<Server>,
    pub news: Vec<NewsArticle>,
    pub mods_db: ModsDb,
    pub players_online: Option<u64>,
    pub steam_root: Option<PathBuf>,
    pub dayz_path: Option<PathBuf>,
    pub workshop_path: Option<PathBuf>,
    pub steam: Option<SteamHandle>,
    pub status_message: Option<String>,
    pub selected_server: Option<usize>,
    pub direct_connect_target: Option<(String, u16)>,
    pub server_runtime: HashMap<String, crate::server::ServerRuntimeInfo>,
    pub available_update: Option<ReleaseInfo>,
    pub update_mods_before_launch: bool,
    pending_launch: Option<PendingLaunch>,
    asked_update_mods: bool,
    screen_stack: Vec<Box<dyn Screen>>,
}

impl App {
    pub fn new(config: Config, profile: Profile) -> Self {
        App {
            running: true,
            config,
            profile,
            servers: Vec::new(),
            news: Vec::new(),
            mods_db: ModsDb {
                sum: String::new(),
                mods: Vec::new(),
            },
            players_online: None,
            steam_root: None,
            dayz_path: None,
            workshop_path: None,
            steam: None,
            status_message: None,
            selected_server: None,
            direct_connect_target: None,
            server_runtime: HashMap::new(),
            available_update: None,
            update_mods_before_launch: false,
            pending_launch: None,
            asked_update_mods: false,
            screen_stack: vec![Box::new(main_menu::MainMenuScreen::new())],
        }
    }

    pub fn init_main_menu(&mut self) {
        if let Some(mut screen) = self.screen_stack.pop() {
            screen.on_enter(self);
            self.screen_stack.push(screen);
        }
    }

    pub fn init_paths(&mut self) {
        if let Some(root_str) = &self.profile.steam_root {
            let path = PathBuf::from(root_str);
            if path.join("common/DayZ").exists() {
                self.dayz_path = Some(crate::mods::find_dayz_path(&path));
                self.workshop_path = Some(crate::mods::find_workshop_path(&path));
                self.steam_root = Some(path);
                return;
            }
        }

        if let Some(root) = crate::mods::detect_steam_root() {
            self.dayz_path = Some(crate::mods::find_dayz_path(&root));
            self.workshop_path = Some(crate::mods::find_workshop_path(&root));
            self.steam_root = Some(root);
        }
    }

    pub fn load_data(&mut self) {
        if let Some(ref wp) = self.workshop_path {
            match crate::mods::scan_installed_mods(wp) {
                Ok(db) => {
                    let _ = crate::mods::save_mods_db(&self.config.mods_db_path, &db);
                    self.mods_db = db;
                }
                Err(e) => tracing::warn!("Failed to scan mods: {e}"),
            }
        }

        match crate::api::servers::load_cached_servers(
            &self.config.server_db_path,
            self.config.server_db_ttl,
        ) {
            Ok(Some(data)) => {
                self.players_online = data.players_online;
                self.servers = data.result;
            }
            _ => {
                self.refresh_servers();
            }
        }

        match crate::api::news::load_cached_news(
            &self.config.news_db_path,
            self.config.news_db_ttl,
        ) {
            Ok(Some(articles)) => self.news = articles,
            _ => {
                self.refresh_news();
            }
        }
    }

    pub fn refresh_servers(&mut self) {
        match crate::api::servers::fetch_server_list(
            &self.config.api_url,
            self.config.server_request_timeout,
        ) {
            Ok(mut data) => {
                if let Ok(count) =
                    crate::api::servers::fetch_players_online(self.config.request_timeout)
                {
                    data.players_online = Some(count);
                }
                let _ =
                    crate::api::servers::save_server_cache(&self.config.server_db_path, &data);
                self.players_online = data.players_online;
                self.servers = data.result;
                self.status_message = Some(format!("Loaded {} servers", self.servers.len()));
            }
            Err(e) => {
                self.status_message = Some(format!("Failed to fetch servers: {e}"));
            }
        }
    }

    pub fn refresh_news(&mut self) {
        match crate::api::news::fetch_news(self.config.request_timeout) {
            Ok(articles) => {
                let _ = crate::api::news::save_news_cache(&self.config.news_db_path, &articles);
                self.news = articles;
            }
            Err(e) => {
                tracing::warn!("Failed to fetch news: {e}");
            }
        }
    }

    pub fn init_steam(&mut self) {
        match SteamHandle::init() {
            Ok(handle) => {
                if self.profile.player.is_none() {
                    self.profile.player = Some(handle.user_name());
                }
                self.steam = Some(handle);
                tracing::info!("Steam client initialized");
            }
            Err(e) => {
                tracing::warn!("Steam client not available: {e}");
            }
        }
    }

    pub fn render(&mut self, f: &mut Frame) {
        // Split rendering: take the screen out, render, put back
        if let Some(mut screen) = self.screen_stack.pop() {
            screen.render(f, self);
            self.screen_stack.push(screen);
        }

        if let Some(msg) = &self.status_message {
            render_status_bar(f, msg);
        }
    }

    pub fn handle_key(&mut self, key: crossterm::event::KeyEvent) {
        let action = if let Some(mut screen) = self.screen_stack.pop() {
            let action = screen.handle_key(key, self);
            self.screen_stack.push(screen);
            action
        } else {
            Action::Quit
        };
        self.process_action(action);
    }

    fn process_action(&mut self, action: Action) {
        match action {
            Action::None => {}
            Action::Quit => self.running = false,
            Action::PopScreen => {
                self.screen_stack.pop();
                if self.screen_stack.is_empty() {
                    self.running = false;
                } else if let Some(mut screen) = self.screen_stack.pop() {
                    screen.on_enter(self);
                    self.screen_stack.push(screen);
                }
            }
            Action::PushScreen(id) => {
                let mut screen = self.create_screen(id);
                screen.on_enter(self);
                self.screen_stack.push(screen);
            }
            Action::ReplaceScreen(id) => {
                self.screen_stack.pop();
                let mut screen = self.create_screen(id);
                screen.on_enter(self);
                self.screen_stack.push(screen);
            }
            Action::LaunchGame => {
                self.do_launch();
            }
            Action::RunSelfUpdate => {
                self.run_self_update();
            }
        }
    }

    fn create_screen(&self, id: ScreenId) -> Box<dyn Screen> {
        match id {
            ScreenId::MainMenu => Box::new(main_menu::MainMenuScreen::new()),
            ScreenId::ServerBrowser => {
                Box::new(ServerBrowserScreen::new(BrowseSource::All))
            }
            ScreenId::FilteredBrowser(indices) => {
                Box::new(ServerBrowserScreen::new(BrowseSource::Filtered(indices)))
            }
            ScreenId::FavoritesBrowser => {
                Box::new(ServerBrowserScreen::new(BrowseSource::Favorites))
            }
            ScreenId::HistoryBrowser => {
                Box::new(ServerBrowserScreen::new(BrowseSource::History))
            }
            ScreenId::ServerDetail(idx) => {
                Box::new(server_detail::ServerDetailScreen::new(idx))
            }
            ScreenId::Config => Box::new(config_screen::ConfigScreen::new()),
            ScreenId::News => Box::new(news::NewsScreen::new()),
            ScreenId::DirectConnect => Box::new(direct_connect::DirectConnectScreen::new()),
            ScreenId::FilterSelect => Box::new(filter::FilterSelectScreen::new(self)),
            ScreenId::UpdatePrompt => Box::new(update_prompt::UpdatePromptScreen::new()),
            ScreenId::Confirm(action) => Box::new(popup::ConfirmScreen::new(action)),
        }
    }

    fn do_launch(&mut self) {
        let server = self.selected_server.and_then(|i| self.servers.get(i));
        let has_mods = server.map(|s| !s.mods.is_empty()).unwrap_or(false);

        if has_mods && self.steam.is_some() && !self.asked_update_mods {
            self.asked_update_mods = true;
            let mut screen =
                self.create_screen(ScreenId::Confirm(ConfirmAction::UpdateModsBeforeLaunch));
            screen.on_enter(self);
            self.screen_stack.push(screen);
            return;
        }

        let player = self
            .profile
            .player
            .clone()
            .unwrap_or_else(|| "Survivor".into());
        let extra_args = self.profile.get_launch_args();

        let server = self.selected_server.and_then(|i| self.servers.get(i));
        let direct_target = self.direct_connect_target.take();
        let mod_ids: Vec<u64> = server
            .map(|s| s.mods.iter().map(|m| m.steam_workshop_id).collect())
            .unwrap_or_default();
        let history_entry = server
            .map(|server| {
                (
                    server.name.clone(),
                    server.endpoint.ip.clone(),
                    server.endpoint.port,
                )
            })
            .or_else(|| {
                direct_target
                    .as_ref()
                    .map(|(ip, port)| (format!("{ip}:{port}"), ip.clone(), *port))
            });

        let args = if let Some((ip, port)) = direct_target.as_ref() {
            crate::launch::build_direct_connect_args(ip, *port, &player, &extra_args, None)
        } else {
            crate::launch::build_launch_args(
                self.selected_server.and_then(|i| self.servers.get(i)),
                &mod_ids,
                &player,
                &extra_args,
                None,
            )
        };

        if !mod_ids.is_empty() && (self.dayz_path.is_none() || self.workshop_path.is_none()) {
            self.status_message = Some("Cannot manage server mods: Steam library path not detected".into());
            self.asked_update_mods = false;
            return;
        }

        let ids_to_check = if self.update_mods_before_launch {
            mod_ids.clone()
        } else {
            crate::mods::get_missing_mods(&self.mods_db, &mod_ids)
        };

        if !ids_to_check.is_empty() {
            let Some(steam) = self.steam.as_ref() else {
                self.status_message = Some(format!(
                    "Missing mods not installed locally: {}",
                    format_mod_ids(&ids_to_check)
                ));
                self.asked_update_mods = false;
                return;
            };

            match steam.ensure_mods_downloaded(&ids_to_check) {
                Ok(pending_mod_ids) if !pending_mod_ids.is_empty() => {
                    let statuses = collect_pending_download_statuses(steam, &pending_mod_ids);
                    self.status_message = Some(download_status_message(&statuses));
                    self.pending_launch = Some(PendingLaunch {
                        args,
                        all_mod_ids: mod_ids,
                        pending_mod_ids,
                        history_entry,
                    });
                    self.asked_update_mods = false;
                    self.update_mods_before_launch = false;
                    return;
                }
                Ok(_) => {}
                Err(e) => {
                    self.status_message = Some(format!("Failed to queue workshop downloads: {e}"));
                    self.asked_update_mods = false;
                    self.update_mods_before_launch = false;
                    return;
                }
            }
        }

        if let Err(e) = self.ensure_symlinks(&mod_ids) {
            self.status_message = Some(format!("Failed to create mod symlinks: {e}"));
            self.asked_update_mods = false;
            self.update_mods_before_launch = false;
            return;
        }

        self.asked_update_mods = false;
        self.update_mods_before_launch = false;
        self.finish_launch(args, history_entry);
    }

    fn finish_launch(&mut self, args: Vec<String>, history_entry: Option<(String, String, u16)>) {
        match crate::launch::launch_dayz(&args) {
            Ok(()) => {
                if let Some((name, ip, port)) = history_entry {
                    self.profile
                        .add_history(&name, &ip, port, self.config.history_size);
                    let _ = self.profile.save(&self.config.profile_path);
                }
                self.status_message = Some("DayZ launched!".into());
                self.running = false;
            }
            Err(e) => {
                self.status_message = Some(format!("Failed to launch: {e}"));
            }
        }
    }

    fn run_self_update(&mut self) {
        let Some(update) = self.available_update.clone() else {
            self.status_message = Some("No update is currently available".into());
            return;
        };

        match crate::update::run_installer_and_restart(&update.installer_url, &update.tag) {
            Ok(()) => {
                self.status_message = Some(format!("Updated to {}", update.tag));
                self.running = false;
            }
            Err(e) => {
                self.status_message = Some(format!("Self-update failed: {e}"));
            }
        }
    }

    fn ensure_symlinks(&mut self, mod_ids: &[u64]) -> anyhow::Result<()> {
        if let (Some(dp), Some(wp)) = (&self.dayz_path, &self.workshop_path) {
            crate::mods::ensure_mod_symlinks(dp, wp, mod_ids)?;
        }
        Ok(())
    }

    pub fn ensure_server_runtime_info(&mut self, ip: &str) {
        if self.server_runtime.contains_key(ip) {
            return;
        }
        let info = crate::server::runtime::lookup_runtime_info(ip);
        self.server_runtime.insert(ip.to_string(), info);
    }

    pub fn tick(&mut self) {
        let action = if let Some(mut screen) = self.screen_stack.pop() {
            let action = screen.on_tick(self);
            self.screen_stack.push(screen);
            action
        } else {
            Action::None
        };
        self.process_action(action);

        let Some(pending) = self.pending_launch.take() else {
            return;
        };

        let Some(steam) = self.steam.as_ref() else {
            self.status_message = Some("Waiting for Steam to resume workshop downloads".into());
            self.pending_launch = Some(pending);
            return;
        };

        let statuses = collect_pending_download_statuses(steam, &pending.pending_mod_ids);
        if downloads_ready(&statuses) {
            if let Err(e) = self.ensure_symlinks(&pending.all_mod_ids) {
                self.status_message = Some(format!("Failed to create mod symlinks: {e}"));
                return;
            }
            self.finish_launch(pending.args, pending.history_entry);
        } else {
            self.status_message = Some(download_status_message(&statuses));
            self.pending_launch = Some(pending);
        }
    }
}

fn collect_pending_download_statuses(
    steam: &SteamHandle,
    workshop_ids: &[u64],
) -> Vec<PendingDownloadStatus> {
    workshop_ids
        .iter()
        .copied()
        .map(|workshop_id| PendingDownloadStatus {
            workshop_id,
            state: steam.get_item_state(workshop_id),
            progress: steam.get_download_progress(workshop_id),
        })
        .collect()
}

fn downloads_ready(statuses: &[PendingDownloadStatus]) -> bool {
    statuses
        .iter()
        .all(|status| status.state == ItemState::Installed)
}

fn download_status_message(statuses: &[PendingDownloadStatus]) -> String {
    let details = statuses
        .iter()
        .map(|status| match status.progress {
            Some((downloaded, total)) if total > 0 => {
                let percent = downloaded.saturating_mul(100) / total;
                format!("{} ({}%)", status.workshop_id, percent)
            }
            _ => match status.state {
                ItemState::Installed => format!("{} (ready)", status.workshop_id),
                _ => format!("{} (queued)", status.workshop_id),
            },
        })
        .collect::<Vec<_>>()
        .join(", ");

    format!("Downloading {} mods: {details}", statuses.len())
}

fn format_mod_ids(mod_ids: &[u64]) -> String {
    mod_ids
        .iter()
        .map(u64::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

fn render_status_bar(f: &mut Frame, msg: &str) {
    use ratatui::layout::Rect;
    use ratatui::widgets::Paragraph;

    let area = f.area();
    let bar = Rect::new(0, area.height.saturating_sub(1), area.width, 1);

    let para = Paragraph::new(msg).style(theme::WARNING);
    f.render_widget(para, bar);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::steam::ItemState;

    #[test]
    fn download_status_message_includes_progress_and_waiting_items() {
        let message = download_status_message(&[
            PendingDownloadStatus {
                workshop_id: 10,
                state: ItemState::Downloading,
                progress: Some((25, 100)),
            },
            PendingDownloadStatus {
                workshop_id: 20,
                state: ItemState::NeedsUpdate,
                progress: None,
            },
        ]);

        assert!(message.contains("Downloading 2 mods"));
        assert!(message.contains("10 (25%)"));
        assert!(message.contains("20 (queued)"));
    }

    #[test]
    fn downloads_ready_only_when_all_items_installed() {
        assert!(downloads_ready(&[
            PendingDownloadStatus {
                workshop_id: 10,
                state: ItemState::Installed,
                progress: None,
            },
            PendingDownloadStatus {
                workshop_id: 20,
                state: ItemState::Installed,
                progress: None,
            },
        ]));

        assert!(!downloads_ready(&[
            PendingDownloadStatus {
                workshop_id: 10,
                state: ItemState::Installed,
                progress: None,
            },
            PendingDownloadStatus {
                workshop_id: 20,
                state: ItemState::Downloading,
                progress: Some((10, 100)),
            },
        ]));
    }
}
