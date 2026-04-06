use ratatui::Frame;
use std::collections::HashMap;
use std::path::PathBuf;

use crate::api::news::NewsArticle;
use crate::api::releases::{ReleaseInfo, UpdateAvailability};
use crate::config::Config;
use crate::mods::ModsDb;
use crate::profile::Profile;
use crate::server::Server;
use crate::steam::{ItemState, SteamHandle};
use crate::ui::server_browser::{BrowseSource, ServerBrowserScreen};
use crate::ui::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum LaunchTarget {
    KnownServer(usize),
    DirectConnect { ip: String, port: u16 },
    Offline { mission_id: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LaunchPrep {
    pub(crate) target: LaunchTarget,
    pub(crate) mod_ids: Vec<u64>,
    pub(crate) password: Option<String>,
    pub(crate) offline_spawn_enabled: Option<bool>,
}

struct PendingLaunch {
    args: Vec<String>,
    all_mod_ids: Vec<u64>,
    pending_mod_ids: Vec<u64>,
    history_entry: Option<(String, String, u16)>,
    offline_update: Option<(String, Option<bool>)>,
    kind: PendingDownloadKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum PendingDownloadKind {
    Launch,
    RefreshInstalledMods,
}

trait WorkshopDownloadClient {
    fn ensure_mods_downloaded(
        &self,
        workshop_ids: &[u64],
        force_update: bool,
    ) -> anyhow::Result<Vec<u64>>;

    fn get_item_state(&self, workshop_id: u64) -> ItemState;

    fn get_download_progress(&self, workshop_id: u64) -> Option<(u64, u64)>;
}

impl WorkshopDownloadClient for SteamHandle {
    fn ensure_mods_downloaded(
        &self,
        workshop_ids: &[u64],
        force_update: bool,
    ) -> anyhow::Result<Vec<u64>> {
        SteamHandle::ensure_mods_downloaded(self, workshop_ids, force_update)
    }

    fn get_item_state(&self, workshop_id: u64) -> ItemState {
        SteamHandle::get_item_state(self, workshop_id)
    }

    fn get_download_progress(&self, workshop_id: u64) -> Option<(u64, u64)> {
        SteamHandle::get_download_progress(self, workshop_id)
    }
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
    pub(crate) launch_prep: Option<LaunchPrep>,
    pub server_runtime: HashMap<String, crate::server::ServerRuntimeInfo>,
    pub available_update: Option<ReleaseInfo>,
    pub update_mods_before_launch: bool,
    pub(crate) skip_running_check_once: bool,
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
            launch_prep: None,
            server_runtime: HashMap::new(),
            available_update: None,
            update_mods_before_launch: false,
            skip_running_check_once: false,
            pending_launch: None,
            asked_update_mods: false,
            screen_stack: vec![Box::new(main_menu::MainMenuScreen::new())],
        }
    }

    fn store_launch_prep(&mut self, prep: LaunchPrep) {
        self.launch_prep = Some(prep);
    }

    pub(crate) fn prepare_known_server_launch(&mut self, server_index: usize) {
        let Some(server) = self.servers.get(server_index) else {
            self.status_message = Some(format!(
                "Launch target server {server_index} is unavailable"
            ));
            return;
        };

        self.store_launch_prep(LaunchPrep {
            target: LaunchTarget::KnownServer(server_index),
            mod_ids: server.mods.iter().map(|m| m.steam_workshop_id).collect(),
            password: None,
            offline_spawn_enabled: None,
        });
    }

    pub(crate) fn prepare_direct_connect_launch(&mut self, ip: String, port: u16) {
        self.store_launch_prep(LaunchPrep {
            target: LaunchTarget::DirectConnect { ip, port },
            mod_ids: Vec::new(),
            password: None,
            offline_spawn_enabled: None,
        });
    }

    pub(crate) fn set_launch_password(&mut self, password: Option<String>) {
        if let Some(prep) = self.launch_prep.as_mut() {
            prep.password = password;
        }
    }

    pub(crate) fn clear_direct_connect_launch_prep(&mut self) {
        if matches!(
            self.launch_prep.as_ref().map(|prep| &prep.target),
            Some(LaunchTarget::DirectConnect { .. })
        ) {
            self.launch_prep = None;
        }
    }

    pub(crate) fn ensure_startup_max_map_count_gate(&mut self) -> anyhow::Result<bool> {
        match crate::config::current_max_map_count_state()? {
            crate::config::MaxMapCountState::Ready(_) => Ok(false),
            crate::config::MaxMapCountState::UnsupportedPlatform => Ok(false),
            crate::config::MaxMapCountState::NeedsFix(_) => {
                self.process_action(Action::PushScreen(ScreenId::Confirm(
                    ConfirmAction::FixMaxMapCount,
                )));
                Ok(true)
            }
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

        match crate::api::news::load_cached_news(&self.config.news_db_path, self.config.news_db_ttl)
        {
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
                let _ = crate::api::servers::save_server_cache(&self.config.server_db_path, &data);
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

    pub fn check_for_updates(&mut self) {
        match crate::api::releases::check_for_update(
            &self.config.github_owner,
            &self.config.github_repo,
            crate::config::VERSION,
            self.config.request_timeout,
        ) {
            Ok(availability) => self.apply_update_availability(availability),
            Err(e) => {
                tracing::warn!("Failed to check for updates: {e}");
            }
        }
    }

    fn check_for_updates_manually(&mut self) {
        match crate::api::releases::check_for_update(
            &self.config.github_owner,
            &self.config.github_repo,
            crate::config::VERSION,
            self.config.request_timeout,
        ) {
            Ok(availability) => self.apply_manual_update_availability(availability),
            Err(error) => {
                self.status_message = Some(format!("Update check failed: {error}"));
            }
        }
    }

    pub fn apply_update_availability(&mut self, availability: UpdateAvailability) {
        match availability {
            UpdateAvailability::UpToDate => {
                self.available_update = None;
            }
            UpdateAvailability::Available(release) => {
                self.available_update = Some(release);
                self.process_action(Action::PushScreen(ScreenId::UpdatePrompt));
            }
        }
    }

    fn apply_manual_update_availability(&mut self, availability: UpdateAvailability) {
        match availability {
            UpdateAvailability::UpToDate => {
                self.available_update = None;
                self.status_message =
                    Some(format!("Already up to date ({})", crate::config::VERSION));
            }
            UpdateAvailability::Available(release) => {
                self.status_message = Some(format!("Update available: {}", release.tag));
                self.apply_update_availability(UpdateAvailability::Available(release));
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

    pub(crate) fn process_action(&mut self, action: Action) {
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
            Action::CheckForUpdates => {
                self.check_for_updates_manually();
            }
            Action::RefreshInstalledMods => {
                if let Some(pending) = self.pending_launch.as_ref() {
                    self.status_message = Some(refresh_busy_message(&pending.kind).into());
                    return;
                }
                self.refresh_installed_mods();
            }
        }
    }

    fn create_screen(&self, id: ScreenId) -> Box<dyn Screen> {
        match id {
            ScreenId::MainMenu => Box::new(main_menu::MainMenuScreen::new()),
            ScreenId::ServerBrowser => Box::new(ServerBrowserScreen::new(BrowseSource::All)),
            ScreenId::FilteredBrowser(indices) => {
                Box::new(ServerBrowserScreen::new(BrowseSource::Filtered(indices)))
            }
            ScreenId::FavoritesBrowser => {
                Box::new(ServerBrowserScreen::new(BrowseSource::Favorites))
            }
            ScreenId::HistoryBrowser => Box::new(ServerBrowserScreen::new(BrowseSource::History)),
            ScreenId::ServerDetail(idx) => Box::new(server_detail::ServerDetailScreen::new(idx)),
            ScreenId::Config => Box::new(config_screen::ConfigScreen::new()),
            ScreenId::News => Box::new(news::NewsScreen::new()),
            ScreenId::DirectConnect => Box::new(direct_connect::DirectConnectScreen::new()),
            ScreenId::DirectConnectSetup => {
                Box::new(direct_connect_setup::DirectConnectSetupScreen::new())
            }
            ScreenId::PasswordPrompt => Box::new(password_prompt::PasswordPromptScreen::new()),
            ScreenId::FilterSelect => Box::new(filter::FilterSelectScreen::new(self)),
            ScreenId::UpdatePrompt => Box::new(update_prompt::UpdatePromptScreen::new()),
            ScreenId::Info(data) => Box::new(info_screen::InfoScreen::new(data)),
            ScreenId::Confirm(action) => Box::new(popup::ConfirmScreen::new(action)),
        }
    }

    fn do_launch(&mut self) {
        if !self.skip_running_check_once && crate::launch::is_dayz_running() {
            let mut screen = self.create_screen(ScreenId::Confirm(ConfirmAction::KillDayZ));
            screen.on_enter(self);
            self.screen_stack.push(screen);
            return;
        }
        self.skip_running_check_once = false;

        let Some(prep) = self.launch_prep.clone() else {
            self.status_message = Some("No launch target selected".into());
            return;
        };

        let LaunchPrep {
            target,
            mod_ids,
            password,
            offline_spawn_enabled,
        } = prep;

        if let LaunchTarget::KnownServer(idx) = &target {
            let Some(server) = self.servers.get(*idx) else {
                self.status_message = Some(format!("Launch target server {idx} is unavailable"));
                return;
            };

            if server.password && password.is_none() {
                let mut screen = self.create_screen(ScreenId::PasswordPrompt);
                screen.on_enter(self);
                self.screen_stack.push(screen);
                return;
            }
        }

        let has_mods = !mod_ids.is_empty();

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

        let offline_update = match &target {
            LaunchTarget::Offline { mission_id } => {
                Some((mission_id.clone(), offline_spawn_enabled))
            }
            _ => None,
        };

        let (args, history_entry) = match target {
            LaunchTarget::KnownServer(idx) => {
                let Some(server) = self.servers.get(idx) else {
                    self.status_message =
                        Some(format!("Launch target server {idx} is unavailable"));
                    return;
                };
                let history_entry = Some((
                    server.name.clone(),
                    server.endpoint.ip.clone(),
                    server.endpoint.port,
                ));
                let args = crate::launch::build_launch_args(
                    Some(server),
                    &mod_ids,
                    &player,
                    &extra_args,
                    password.as_deref(),
                );
                (args, history_entry)
            }
            LaunchTarget::DirectConnect { ip, port } => {
                let history_entry = Some((format!("{ip}:{port}"), ip.clone(), port));
                let args = crate::launch::build_direct_connect_args_with_selected_mod_ids(
                    &ip,
                    port,
                    &player,
                    &mod_ids,
                    &extra_args,
                    password.as_deref(),
                );
                (args, history_entry)
            }
            LaunchTarget::Offline { mission_id } => {
                let Some(_dayz_path) = self.dayz_path.as_ref() else {
                    self.status_message =
                        Some("Cannot launch offline: DayZ path not detected".into());
                    return;
                };

                let args = crate::launch::build_offline_launch_args(
                    &mission_id,
                    &mod_ids,
                    &player,
                    &extra_args,
                );
                (args, None)
            }
        };

        if has_mods && (self.dayz_path.is_none() || self.workshop_path.is_none()) {
            self.status_message =
                Some("Cannot manage server mods: Steam library path not detected".into());
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
                return;
            };

            match steam.ensure_mods_downloaded(&ids_to_check, self.update_mods_before_launch) {
                Ok(pending_mod_ids) if !pending_mod_ids.is_empty() => {
                    let statuses = collect_pending_download_statuses(steam, &pending_mod_ids);
                    self.status_message = Some(download_status_message(&statuses));
                    self.pending_launch = Some(PendingLaunch {
                        args,
                        all_mod_ids: mod_ids,
                        pending_mod_ids,
                        history_entry,
                        offline_update,
                        kind: PendingDownloadKind::Launch,
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

        if let Some((mission_id, spawn_enabled)) = offline_update {
            let Some(dayz_path) = self.dayz_path.as_ref() else {
                self.status_message = Some("Cannot launch offline: DayZ path not detected".into());
                self.asked_update_mods = false;
                self.update_mods_before_launch = false;
                return;
            };

            if let Err(e) =
                crate::launch::apply_offline_spawn_setting(dayz_path, &mission_id, spawn_enabled)
            {
                self.status_message = Some(format!("Failed to update offline spawn setting: {e}"));
                self.asked_update_mods = false;
                self.update_mods_before_launch = false;
                return;
            }
        }

        self.asked_update_mods = false;
        self.update_mods_before_launch = false;
        self.finish_launch(args, history_entry);
    }

    fn finish_launch(&mut self, args: Vec<String>, history_entry: Option<(String, String, u16)>) {
        match crate::launch::launch_dayz(&args) {
            Ok(()) => {
                self.launch_prep = None;
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

    fn refresh_installed_mods(&mut self) {
        let Some(workshop_path) = self.workshop_path.clone() else {
            self.status_message = Some("Steam library path not detected".into());
            return;
        };

        let db = match crate::mods::scan_installed_mods(&workshop_path) {
            Ok(db) => db,
            Err(e) => {
                self.status_message = Some(format!("Failed to scan installed mods: {e}"));
                return;
            }
        };

        let workshop_ids = crate::mods::get_installed_workshop_ids(&db);
        if let Err(e) = crate::mods::save_mods_db(&self.config.mods_db_path, &db) {
            self.status_message = Some(format!("Failed to refresh installed mods: {e}"));
            return;
        }
        self.mods_db = db;

        let Some(steam) = self.steam.as_ref() else {
            self.status_message =
                Some("Steam client not available; installed workshop mods refreshed locally".into());
            return;
        };

        if workshop_ids.is_empty() {
            self.status_message = Some("No installed mods to refresh".into());
            return;
        }

        self.status_message = Some(
            "DayZ game itself updates through Steam; refreshing installed workshop mods..."
                .into(),
        );

        let queued_mod_ids = match steam.ensure_mods_downloaded(&workshop_ids, true) {
            Ok(pending_mod_ids) => pending_mod_ids,
            Err(e) => {
                self.status_message = Some(format!("Failed to queue workshop downloads: {e}"));
                return;
            }
        };

        if queued_mod_ids.is_empty() {
            self.status_message = Some(
                "DayZ game itself updates through Steam; installed workshop mods refreshed"
                    .into(),
            );
            return;
        }

        let statuses = {
            let steam = self
                .steam
                .as_ref()
                .expect("steam handle is available after successful queueing");
            collect_pending_download_statuses(steam, &queued_mod_ids)
        };
        self.status_message = Some(format!(
            "DayZ game itself updates through Steam; {}",
            download_status_message(&statuses)
        ));
        self.pending_launch = Some(PendingLaunch {
            args: Vec::new(),
            all_mod_ids: workshop_ids,
            pending_mod_ids: queued_mod_ids,
            history_entry: None,
            offline_update: None,
            kind: PendingDownloadKind::RefreshInstalledMods,
        });
    }

    fn refresh_installed_mods_with(&mut self, steam: &dyn WorkshopDownloadClient) {
        if let Some(pending) = self.pending_launch.as_ref() {
            self.status_message = Some(refresh_busy_message(&pending.kind).into());
            return;
        }

        let Some(workshop_path) = self.workshop_path.clone() else {
            self.status_message = Some("Steam library path not detected".into());
            return;
        };

        let db = match crate::mods::scan_installed_mods(&workshop_path) {
            Ok(db) => db,
            Err(e) => {
                self.status_message = Some(format!("Failed to scan installed mods: {e}"));
                return;
            }
        };

        let workshop_ids = crate::mods::get_installed_workshop_ids(&db);
        if let Err(e) = crate::mods::save_mods_db(&self.config.mods_db_path, &db) {
            self.status_message = Some(format!("Failed to refresh installed mods: {e}"));
            return;
        }
        self.mods_db = db;

        if workshop_ids.is_empty() {
            self.status_message = Some("No installed mods to refresh".into());
            return;
        }

        self.status_message = Some(
            "DayZ game itself updates through Steam; refreshing installed workshop mods..."
                .into(),
        );

        let queued_mod_ids = match steam.ensure_mods_downloaded(&workshop_ids, true) {
            Ok(pending_mod_ids) => pending_mod_ids,
            Err(e) => {
                self.status_message = Some(format!("Failed to queue workshop downloads: {e}"));
                return;
            }
        };

        if queued_mod_ids.is_empty() {
            self.status_message = Some(
                "DayZ game itself updates through Steam; installed workshop mods refreshed"
                    .into(),
            );
            return;
        }

        let statuses = collect_pending_download_statuses(steam, &queued_mod_ids);
        self.status_message = Some(format!(
            "DayZ game itself updates through Steam; {}",
            download_status_message(&statuses)
        ));
        self.pending_launch = Some(PendingLaunch {
            args: Vec::new(),
            all_mod_ids: workshop_ids,
            pending_mod_ids: queued_mod_ids,
            history_entry: None,
            offline_update: None,
            kind: PendingDownloadKind::RefreshInstalledMods,
        });
    }

    fn ensure_symlinks(&mut self, mod_ids: &[u64]) -> anyhow::Result<()> {
        if let (Some(dp), Some(wp)) = (&self.dayz_path, &self.workshop_path) {
            crate::mods::ensure_mod_symlinks(dp, wp, mod_ids)?;
        }
        Ok(())
    }

    fn rescan_and_save_mods_db(&mut self, workshop_path: &PathBuf) -> anyhow::Result<()> {
        let db = crate::mods::scan_installed_mods(workshop_path.as_path())?;
        crate::mods::save_mods_db(&self.config.mods_db_path, &db)?;
        self.mods_db = db;
        Ok(())
    }

    fn advance_pending_downloads(&mut self) {
        let Some(pending) = self.pending_launch.take() else {
            return;
        };

        let resolution = {
            let steam = self
                .steam
                .as_ref()
                .map(|steam| steam as &dyn WorkshopDownloadClient);
            self.resolve_pending_downloads(pending, steam)
        };

        self.apply_pending_download_resolution(resolution);
    }

    fn advance_pending_downloads_with(
        &mut self,
        steam: Option<&dyn WorkshopDownloadClient>,
    ) {
        let Some(pending) = self.pending_launch.take() else {
            return;
        };

        let resolution = self.resolve_pending_downloads(pending, steam);
        self.apply_pending_download_resolution(resolution);
    }

    fn resolve_pending_downloads(
        &self,
        pending: PendingLaunch,
        steam: Option<&dyn WorkshopDownloadClient>,
    ) -> PendingDownloadResolution {
        let PendingLaunch {
            args,
            all_mod_ids,
            pending_mod_ids,
            history_entry,
            offline_update,
            kind,
        } = pending;

        if !pending_mod_ids.is_empty() {
            let Some(steam) = steam else {
                let status_message = match &kind {
                    PendingDownloadKind::Launch => {
                        "Waiting for Steam to resume workshop downloads".into()
                    }
                    PendingDownloadKind::RefreshInstalledMods => {
                        "DayZ game itself updates through Steam; waiting for Steam to resume workshop downloads"
                            .into()
                    }
                };
                return PendingDownloadResolution::Requeue {
                    pending: PendingLaunch {
                        args,
                        all_mod_ids,
                        pending_mod_ids,
                        history_entry,
                        offline_update,
                        kind,
                    },
                    status_message,
                };
            };

            let statuses = collect_pending_download_statuses(steam, &pending_mod_ids);
            if !downloads_ready(&statuses) {
                let status_message = match &kind {
                    PendingDownloadKind::Launch => download_status_message(&statuses),
                    PendingDownloadKind::RefreshInstalledMods => format!(
                        "DayZ game itself updates through Steam; {}",
                        download_status_message(&statuses)
                    ),
                };
                return PendingDownloadResolution::Requeue {
                    pending: PendingLaunch {
                        args,
                        all_mod_ids,
                        pending_mod_ids,
                        history_entry,
                        offline_update,
                        kind,
                    },
                    status_message,
                };
            }
        }

        PendingDownloadResolution::Continue(PendingLaunch {
            args,
            all_mod_ids,
            pending_mod_ids,
            history_entry,
            offline_update,
            kind,
        })
    }

    fn apply_pending_download_resolution(&mut self, resolution: PendingDownloadResolution) {
        match resolution {
            PendingDownloadResolution::Requeue {
                pending,
                status_message,
            } => {
                self.status_message = Some(status_message);
                self.pending_launch = Some(pending);
            }
            PendingDownloadResolution::Continue(pending) => match pending.kind {
                PendingDownloadKind::Launch => {
                    if let Err(e) = self.ensure_symlinks(&pending.all_mod_ids) {
                        self.status_message = Some(format!("Failed to create mod symlinks: {e}"));
                        return;
                    }

                    if let Some((mission_id, spawn_enabled)) = pending.offline_update {
                        let Some(dayz_path) = self.dayz_path.as_ref() else {
                            self.status_message =
                                Some("Cannot launch offline: DayZ path not detected".into());
                            return;
                        };

                        if let Err(e) = crate::launch::apply_offline_spawn_setting(
                            dayz_path,
                            &mission_id,
                            spawn_enabled,
                        ) {
                            self.status_message =
                                Some(format!("Failed to update offline spawn setting: {e}"));
                            return;
                        }
                    }

                    self.asked_update_mods = false;
                    self.update_mods_before_launch = false;
                    self.finish_launch(pending.args, pending.history_entry);
                }
                PendingDownloadKind::RefreshInstalledMods => {
                    let Some(workshop_path) = self.workshop_path.clone() else {
                        self.status_message = Some("Steam library path not detected".into());
                        return;
                    };

                    if let Err(e) = self.rescan_and_save_mods_db(&workshop_path) {
                        self.status_message =
                            Some(format!("Failed to refresh installed mods: {e}"));
                        return;
                    }

                    self.status_message = Some(format!(
                        "DayZ game itself updates through Steam; refreshed {} installed workshop mods",
                        pending.all_mod_ids.len()
                    ));
                }
            },
        }
    }

    fn continue_pending_downloads_with(&mut self, steam: &dyn WorkshopDownloadClient) {
        self.advance_pending_downloads_with(Some(steam));
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
        self.advance_pending_downloads();
    }
}

fn refresh_busy_message(kind: &PendingDownloadKind) -> &'static str {
    match kind {
        PendingDownloadKind::Launch => {
            "Cannot refresh installed mods while a launch download is pending"
        }
        PendingDownloadKind::RefreshInstalledMods => {
            "Cannot refresh installed mods while another refresh is already pending"
        }
    }
}

enum PendingDownloadResolution {
    Requeue {
        pending: PendingLaunch,
        status_message: String,
    },
    Continue(PendingLaunch),
}

fn collect_pending_download_statuses(
    steam: &dyn WorkshopDownloadClient,
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
    use crate::config::Config;
    use crate::mods::ModsDb;
    use crate::profile::Profile;
    use crate::server::Server;
    use crate::server::types::{ServerEndpoint, ServerMod};
    use crate::steam::ItemState;
    use std::collections::HashMap;
    use std::cell::RefCell;
    use std::ffi::OsString;
    use std::fs;
    use std::io::Read;
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use std::sync::{Mutex, MutexGuard};
    use std::time::{SystemTime, UNIX_EPOCH};
    use std::{thread, time::Duration};

    fn test_app() -> App {
        let data_dir = std::env::temp_dir().join("dayz-cmd-tests-app");
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

    fn sample_server(password: bool) -> Server {
        Server {
            name: "Test Server".into(),
            players: 12,
            max_players: 60,
            time: "12:00".into(),
            time_acceleration: Some(4.0),
            map: "chernarusplus".into(),
            password,
            battleye: true,
            vac: true,
            first_person_only: false,
            shard: "public".into(),
            version: "1.0".into(),
            environment: "w".into(),
            game_port: 2302,
            endpoint: ServerEndpoint {
                ip: "1.2.3.4".into(),
                port: 27016,
            },
            mods: Vec::<ServerMod>::new(),
        }
    }

    fn sample_server_with_mods(password: bool, mod_ids: &[u64]) -> Server {
        Server {
            mods: mod_ids
                .iter()
                .copied()
                .map(|id| ServerMod {
                    name: format!("Mod {id}"),
                    steam_workshop_id: id,
                })
                .collect(),
            ..sample_server(password)
        }
    }

    struct FakeSteam {
        queued: RefCell<Vec<(Vec<u64>, bool)>>,
        states: RefCell<HashMap<u64, ItemState>>,
        progress: RefCell<HashMap<u64, Option<(u64, u64)>>>,
    }

    impl FakeSteam {
        fn new() -> Self {
            Self {
                queued: RefCell::new(Vec::new()),
                states: RefCell::new(HashMap::new()),
                progress: RefCell::new(HashMap::new()),
            }
        }

        fn with_state(&self, workshop_id: u64, state: ItemState) {
            self.states.borrow_mut().insert(workshop_id, state);
        }

        fn queued_calls(&self) -> Vec<(Vec<u64>, bool)> {
            self.queued.borrow().clone()
        }
    }

    impl WorkshopDownloadClient for FakeSteam {
        fn ensure_mods_downloaded(
            &self,
            workshop_ids: &[u64],
            force_update: bool,
        ) -> anyhow::Result<Vec<u64>> {
            self.queued
                .borrow_mut()
                .push((workshop_ids.to_vec(), force_update));
            Ok(workshop_ids.to_vec())
        }

        fn get_item_state(&self, workshop_id: u64) -> ItemState {
            self.states
                .borrow()
                .get(&workshop_id)
                .cloned()
                .unwrap_or(ItemState::NotInstalled)
        }

        fn get_download_progress(&self, workshop_id: u64) -> Option<(u64, u64)> {
            self.progress
                .borrow()
                .get(&workshop_id)
                .cloned()
                .unwrap_or(None)
        }
    }

    fn temp_path(prefix: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "dayz-cmd-{prefix}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time before unix epoch")
                .as_nanos()
        ))
    }

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn env_lock() -> MutexGuard<'static, ()> {
        ENV_LOCK.lock().expect("lock env")
    }

    struct EnvVarGuard {
        key: &'static str,
        value: Option<OsString>,
    }

    impl EnvVarGuard {
        fn set(key: &'static str, value: &std::ffi::OsStr) -> Self {
            let previous = std::env::var_os(key);
            // SAFETY: tests serialize environment access with ENV_LOCK.
            unsafe { std::env::set_var(key, value) };
            Self {
                key,
                value: previous,
            }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            if let Some(value) = &self.value {
                // SAFETY: tests serialize environment access with ENV_LOCK.
                unsafe { std::env::set_var(self.key, value) };
            } else {
                // SAFETY: tests serialize environment access with ENV_LOCK.
                unsafe { std::env::remove_var(self.key) };
            }
        }
    }

    fn write_executable(path: &Path, body: &str) {
        fs::write(path, body).expect("write script");
        let mut perms = fs::metadata(path).expect("script metadata").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms).expect("set script permissions");
    }

    fn prepend_path(bin_dir: &Path) -> EnvVarGuard {
        let original_path = std::env::var_os("PATH").unwrap_or_default();
        let mut combined_path = OsString::from(bin_dir.as_os_str());
        if !original_path.is_empty() {
            combined_path.push(":");
            combined_path.push(&original_path);
        }
        EnvVarGuard::set("PATH", &combined_path)
    }

    fn setup_launch_bin(bin_dir: &Path, dayz_running: bool) {
        fs::create_dir_all(bin_dir).expect("create bin dir");
        write_executable(
            &bin_dir.join("pgrep"),
            if dayz_running {
                "#!/bin/sh\nexit 0\n"
            } else {
                "#!/bin/sh\nexit 1\n"
            },
        );
        write_executable(
            &bin_dir.join("steam"),
            "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$FAKE_STEAM_ARGS\"\nexit 0\n",
        );
    }

    fn prepare_launch_paths(prefix: &str) -> (PathBuf, PathBuf) {
        let dayz_path = temp_path(&format!("{prefix}-dayz"));
        let workshop_path = temp_path(&format!("{prefix}-workshop"));
        fs::create_dir_all(&dayz_path).expect("create dayz path");
        fs::create_dir_all(&workshop_path).expect("create workshop path");
        (dayz_path, workshop_path)
    }

    fn read_launch_args(capture: &Path) -> Vec<String> {
        for _ in 0..50 {
            if capture.exists() {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }

        let mut content = String::new();
        fs::File::open(capture)
            .expect("open captured args")
            .read_to_string(&mut content)
            .expect("read captured args");
        content.lines().map(|line| line.to_string()).collect()
    }

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

    #[test]
    fn refresh_installed_mods_queues_every_installed_workshop_id_with_force_update() {
        let mut app = test_app();
        let data_dir = temp_path("app-refresh-install");
        let workshop_path = temp_path("app-refresh-workshop");
        fs::create_dir_all(&data_dir).expect("create temp data dir");
        fs::create_dir_all(&workshop_path).expect("create workshop path");
        fs::create_dir_all(workshop_path.join("101")).expect("create mod 101 path");
        fs::write(
            workshop_path.join("101").join("meta.cpp"),
            "name = \"Mod 101\";\npublishedid = 101;\ntimestamp = 1;\n",
        )
        .expect("write mod 101 metadata");
        fs::create_dir_all(workshop_path.join("202")).expect("create mod 202 path");
        fs::write(
            workshop_path.join("202").join("meta.cpp"),
            "name = \"Mod 202\";\npublishedid = 202;\ntimestamp = 2;\n",
        )
        .expect("write mod 202 metadata");
        app.config.mods_db_path = data_dir.join("mods.json");
        app.workshop_path = Some(workshop_path.clone());

        let steam = FakeSteam::new();
        steam.with_state(101, ItemState::Installed);
        steam.with_state(202, ItemState::Installed);

        app.refresh_installed_mods_with(&steam);

        let queued_calls = steam.queued_calls();
        assert_eq!(queued_calls.len(), 1);
        let mut queued_ids = queued_calls[0].0.clone();
        queued_ids.sort();
        assert_eq!(queued_ids, vec![101, 202]);
        assert!(queued_calls[0].1);
        assert!(app.pending_launch.is_some());
        assert!(
            app.status_message
                .as_deref()
                .unwrap_or_default()
                .contains("DayZ game itself updates through Steam")
        );

        fs::remove_dir_all(workshop_path).expect("remove workshop path");
        fs::remove_dir_all(data_dir).expect("remove temp data dir");
    }

    #[test]
    fn refresh_installed_mods_rescans_workshop_state_before_queuing_downloads() {
        let mut app = test_app();
        let data_dir = temp_path("app-refresh-rescan");
        let workshop_path = data_dir.join("workshop");
        let mods_db_path = data_dir.join("mods.json");
        fs::create_dir_all(workshop_path.join("404")).expect("create workshop mod path");
        fs::write(
            workshop_path.join("404").join("meta.cpp"),
            "name = \"Fresh Mod\";\npublishedid = 404;\ntimestamp = 7;\n",
        )
        .expect("write mod metadata");
        app.config.mods_db_path = mods_db_path;
        app.workshop_path = Some(workshop_path.clone());
        app.mods_db = ModsDb {
            sum: "stale".into(),
            mods: vec![crate::mods::ModInfo {
                name: "Stale Mod".into(),
                id: 1,
                timestamp: 0,
                size: 0,
            }],
        };

        let steam = FakeSteam::new();
        steam.with_state(404, ItemState::Installed);

        app.refresh_installed_mods_with(&steam);

        assert_eq!(steam.queued_calls(), vec![(vec![404], true)]);
        assert!(app.pending_launch.is_some());
        assert!(app.mods_db.mods.iter().any(|mod_info| mod_info.id == 404));

        fs::remove_dir_all(data_dir).expect("remove temp data dir");
    }

    #[test]
    fn refresh_installed_mods_reports_save_failure_instead_of_success() {
        let mut app = test_app();
        let data_dir = temp_path("app-refresh-save-failure");
        let workshop_path = data_dir.join("workshop");
        let mods_db_path = data_dir.join("mods.json");
        fs::create_dir_all(workshop_path.join("404")).expect("create workshop mod path");
        fs::write(
            workshop_path.join("404").join("meta.cpp"),
            "name = \"Fresh Mod\";\npublishedid = 404;\ntimestamp = 7;\n",
        )
        .expect("write mod metadata");
        fs::create_dir_all(&mods_db_path).expect("create conflicting mods db directory");
        app.config.mods_db_path = mods_db_path;
        app.workshop_path = Some(workshop_path.clone());

        let steam = FakeSteam::new();
        steam.with_state(404, ItemState::Installed);

        app.refresh_installed_mods_with(&steam);

        assert!(steam.queued_calls().is_empty());
        assert!(
            app.status_message
                .as_deref()
                .unwrap_or_default()
                .contains("Failed to refresh installed mods")
        );

        fs::remove_dir_all(data_dir).expect("remove temp data dir");
    }

    #[test]
    fn refresh_installed_mods_does_not_overwrite_an_existing_pending_launch() {
        let mut app = test_app();
        app.pending_launch = Some(PendingLaunch {
            args: vec!["existing".into()],
            all_mod_ids: vec![11],
            pending_mod_ids: vec![11],
            history_entry: None,
            offline_update: None,
            kind: PendingDownloadKind::Launch,
        });

        let steam = FakeSteam::new();
        app.refresh_installed_mods_with(&steam);

        let pending = app.pending_launch.as_ref().expect("pending launch remains");
        assert_eq!(pending.kind, PendingDownloadKind::Launch);
        assert_eq!(pending.pending_mod_ids, vec![11]);
        assert!(steam.queued_calls().is_empty());
        assert!(
            app.status_message
                .as_deref()
                .unwrap_or_default()
                .contains("launch download is pending")
        );
    }

    #[test]
    fn refresh_installed_mods_reports_when_a_refresh_is_already_pending() {
        let mut app = test_app();
        app.pending_launch = Some(PendingLaunch {
            args: Vec::new(),
            all_mod_ids: vec![22],
            pending_mod_ids: vec![22],
            history_entry: None,
            offline_update: None,
            kind: PendingDownloadKind::RefreshInstalledMods,
        });

        let steam = FakeSteam::new();
        app.refresh_installed_mods_with(&steam);

        let pending = app.pending_launch.as_ref().expect("pending refresh remains");
        assert_eq!(pending.kind, PendingDownloadKind::RefreshInstalledMods);
        assert_eq!(pending.pending_mod_ids, vec![22]);
        assert!(steam.queued_calls().is_empty());
        assert!(
            app.status_message
                .as_deref()
                .unwrap_or_default()
                .contains("refresh is already pending")
        );
    }

    #[test]
    fn refresh_installed_mods_shows_status_when_steam_is_missing() {
        let mut app = test_app();
        let data_dir = temp_path("app-refresh-no-steam");
        let workshop_path = data_dir.join("workshop");
        let mods_db_path = data_dir.join("mods.json");
        fs::create_dir_all(workshop_path.join("303")).expect("create workshop mod path");
        fs::write(
            workshop_path.join("303").join("meta.cpp"),
            "name = \"Mod 303\";\npublishedid = 303;\ntimestamp = 0;\n",
        )
        .expect("write mod metadata");
        app.config.mods_db_path = mods_db_path;
        app.workshop_path = Some(workshop_path);
        app.mods_db = ModsDb {
            sum: "abc".into(),
            mods: vec![crate::mods::ModInfo {
                name: "Mod 303".into(),
                id: 303,
                timestamp: 0,
                size: 0,
            }],
        };
        app.status_message = Some("stale".into());

        app.refresh_installed_mods();

        assert!(app.pending_launch.is_none());
        assert!(
            app.status_message
                .as_deref()
                .unwrap_or_default()
                .contains("Steam client not available")
        );

        fs::remove_dir_all(data_dir).expect("remove temp data dir");
    }

    #[test]
    fn pending_refresh_rescans_and_saves_mods_db_after_downloads_finish() {
        let mut app = test_app();
        let data_dir = temp_path("app-refresh-complete");
        let workshop_path = data_dir.join("workshop");
        let mods_db_path = data_dir.join("mods.json");
        fs::create_dir_all(workshop_path.join("404")).expect("create workshop mod path");
        fs::write(
            workshop_path.join("404").join("meta.cpp"),
            "name = \"Refreshed Mod\";\npublishedid = 404;\ntimestamp = 7;\n",
        )
        .expect("write mod metadata");
        app.config.mods_db_path = mods_db_path.clone();
        app.workshop_path = Some(workshop_path.clone());
        app.mods_db = ModsDb {
            sum: String::new(),
            mods: vec![crate::mods::ModInfo {
                name: "Old Mod".into(),
                id: 1,
                timestamp: 0,
                size: 0,
            }],
        };
        app.pending_launch = Some(PendingLaunch {
            args: Vec::new(),
            all_mod_ids: vec![404],
            pending_mod_ids: vec![404],
            history_entry: None,
            offline_update: None,
            kind: PendingDownloadKind::RefreshInstalledMods,
        });

        let steam = FakeSteam::new();
        steam.with_state(404, ItemState::Installed);

        app.continue_pending_downloads_with(&steam);

        assert!(app.pending_launch.is_none());
        assert_eq!(app.mods_db.mods.len(), 1);
        assert_eq!(app.mods_db.mods[0].id, 404);
        let saved = fs::read_to_string(&mods_db_path).expect("read saved mods db");
        assert!(saved.contains("\"id\": 404"));
        assert!(
            app.status_message
                .as_deref()
                .unwrap_or_default()
                .contains("DayZ game itself updates through Steam")
        );

        fs::remove_dir_all(data_dir).expect("remove temp data dir");
    }

    #[test]
    fn pending_refresh_reports_save_failure_instead_of_success() {
        let mut app = test_app();
        let data_dir = temp_path("app-refresh-complete-failure");
        let workshop_path = data_dir.join("workshop");
        let mods_db_path = data_dir.join("mods.json");
        fs::create_dir_all(workshop_path.join("404")).expect("create workshop mod path");
        fs::write(
            workshop_path.join("404").join("meta.cpp"),
            "name = \"Refreshed Mod\";\npublishedid = 404;\ntimestamp = 7;\n",
        )
        .expect("write mod metadata");
        fs::create_dir_all(&mods_db_path).expect("create conflicting mods db directory");
        app.config.mods_db_path = mods_db_path;
        app.workshop_path = Some(workshop_path.clone());
        app.pending_launch = Some(PendingLaunch {
            args: Vec::new(),
            all_mod_ids: vec![404],
            pending_mod_ids: vec![404],
            history_entry: None,
            offline_update: None,
            kind: PendingDownloadKind::RefreshInstalledMods,
        });

        let steam = FakeSteam::new();
        steam.with_state(404, ItemState::Installed);

        app.continue_pending_downloads_with(&steam);

        assert!(app.pending_launch.is_none());
        assert!(
            app.status_message
                .as_deref()
                .unwrap_or_default()
                .contains("Failed to refresh installed mods")
        );

        fs::remove_dir_all(data_dir).expect("remove temp data dir");
    }

    #[test]
    fn update_availability_pushes_prompt_screen() {
        let mut app = test_app();

        app.apply_update_availability(UpdateAvailability::Available(ReleaseInfo {
            tag: "0.4.0".into(),
            installer_url: "https://example.test/installer.sh".into(),
        }));

        assert_eq!(
            app.available_update
                .as_ref()
                .map(|release| release.tag.as_str()),
            Some("0.4.0")
        );
        assert_eq!(app.screen_stack.len(), 2);
    }

    #[test]
    fn up_to_date_clears_available_update() {
        let mut app = test_app();
        app.available_update = Some(ReleaseInfo {
            tag: "0.4.0".into(),
            installer_url: "https://example.test/installer.sh".into(),
        });

        app.apply_update_availability(UpdateAvailability::UpToDate);

        assert!(app.available_update.is_none());
        assert_eq!(app.screen_stack.len(), 1);
    }

    #[test]
    fn manual_update_check_sets_up_to_date_status() {
        let mut app = test_app();

        app.apply_manual_update_availability(UpdateAvailability::UpToDate);

        assert_eq!(
            app.status_message.as_deref(),
            Some("Already up to date (0.3.0)")
        );
        assert!(app.available_update.is_none());
    }

    #[test]
    fn manual_update_check_prompts_when_update_available() {
        let mut app = test_app();

        app.apply_manual_update_availability(UpdateAvailability::Available(ReleaseInfo {
            tag: "0.4.0".into(),
            installer_url: "https://example.test/installer.sh".into(),
        }));

        assert_eq!(
            app.status_message.as_deref(),
            Some("Update available: 0.4.0")
        );
        assert_eq!(app.screen_stack.len(), 2);
    }

    #[test]
    fn startup_pushes_confirm_screen_when_vm_max_map_count_is_below_minimum() {
        let _guard = env_lock();
        let root = temp_path("app-max-map-count");
        fs::create_dir_all(&root).expect("create temp root");
        let path = root.join("max_map_count");
        fs::write(&path, "524288\n").expect("write low vm.max_map_count");
        let env = EnvVarGuard::set("DAYZ_MAX_MAP_COUNT_PATH", path.as_os_str());

        let mut app = test_app();
        app.ensure_startup_max_map_count_gate()
            .expect("evaluate startup gate");

        assert_eq!(app.screen_stack.len(), 2);

        drop(env);
        fs::remove_dir_all(root).expect("remove temp root");
    }

    #[test]
    fn known_server_launch_reads_target_state_from_shared_launch_prep() {
        let _guard = env_lock();
        let bin_dir = temp_path("app-known-server-bin");
        setup_launch_bin(&bin_dir, false);
        let path_env = prepend_path(&bin_dir);
        let capture = temp_path("app-known-server-args");
        let _capture_env = EnvVarGuard::set("FAKE_STEAM_ARGS", capture.as_os_str());
        let (dayz_path, workshop_path) = prepare_launch_paths("app-known-server");
        let mut app = test_app();
        app.servers = vec![
            sample_server(false),
            sample_server_with_mods(false, &[123456789]),
        ];
        app.mods_db = ModsDb {
            sum: String::new(),
            mods: vec![crate::mods::ModInfo {
                name: "Mod 123456789".into(),
                id: 123456789,
                timestamp: 0,
                size: 0,
            }],
        };
        app.dayz_path = Some(dayz_path.clone());
        app.workshop_path = Some(workshop_path.clone());
        app.prepare_known_server_launch(1);
        app.skip_running_check_once = true;

        app.process_action(Action::LaunchGame);

        let args = read_launch_args(&capture);
        assert!(args.iter().any(|arg| arg == "-connect=1.2.3.4"));
        assert!(args.iter().any(|arg| arg == "-port=2302"));
        assert!(args.iter().any(|arg| arg == "-mod=@123456789"));
        assert!(app.launch_prep.is_none());

        drop(path_env);
        fs::remove_dir_all(&dayz_path).expect("remove dayz path");
        fs::remove_dir_all(&workshop_path).expect("remove workshop path");
        fs::remove_dir_all(bin_dir).expect("remove bin dir");
    }

    #[test]
    fn direct_connect_launch_reads_ip_port_selected_mods_and_password_from_shared_launch_prep() {
        let _guard = env_lock();
        let bin_dir = temp_path("app-direct-connect-bin");
        setup_launch_bin(&bin_dir, false);
        let path_env = prepend_path(&bin_dir);
        let capture = temp_path("app-direct-connect-args");
        let _capture_env = EnvVarGuard::set("FAKE_STEAM_ARGS", capture.as_os_str());
        let (dayz_path, workshop_path) = prepare_launch_paths("app-direct-connect");
        let mut app = test_app();
        app.mods_db = ModsDb {
            sum: String::new(),
            mods: vec![
                crate::mods::ModInfo {
                    name: "Mod 111".into(),
                    id: 111,
                    timestamp: 0,
                    size: 0,
                },
                crate::mods::ModInfo {
                    name: "Mod 222".into(),
                    id: 222,
                    timestamp: 0,
                    size: 0,
                },
            ],
        };
        app.dayz_path = Some(dayz_path.clone());
        app.workshop_path = Some(workshop_path.clone());
        app.prepare_direct_connect_launch("5.6.7.8".into(), 2402);
        if let Some(prep) = app.launch_prep.as_mut() {
            prep.mod_ids = vec![111, 222];
        }
        app.set_launch_password(Some("secret".into()));
        app.skip_running_check_once = true;

        app.process_action(Action::LaunchGame);

        let args = read_launch_args(&capture);
        assert!(args.iter().any(|arg| arg == "-connect=5.6.7.8"));
        assert!(args.iter().any(|arg| arg == "-port=2402"));
        assert!(args.iter().any(|arg| arg == "-mod=@111;@222"));
        assert!(args.iter().any(|arg| arg == "-password=secret"));
        assert!(app.launch_prep.is_none());

        drop(path_env);
        fs::remove_dir_all(&dayz_path).expect("remove dayz path");
        fs::remove_dir_all(&workshop_path).expect("remove workshop path");
        fs::remove_dir_all(bin_dir).expect("remove bin dir");
    }

    #[test]
    fn launch_consumes_one_shot_password_and_prep_state_after_building_args() {
        let _guard = env_lock();
        let bin_dir = temp_path("app-consume-bin");
        setup_launch_bin(&bin_dir, false);
        let path_env = prepend_path(&bin_dir);
        let capture = temp_path("app-consume-args");
        let _capture_env = EnvVarGuard::set("FAKE_STEAM_ARGS", capture.as_os_str());
        let (dayz_path, workshop_path) = prepare_launch_paths("app-consume");
        let mut app = test_app();
        app.dayz_path = Some(dayz_path.clone());
        app.workshop_path = Some(workshop_path.clone());
        app.prepare_direct_connect_launch("9.8.7.6".into(), 2502);
        app.set_launch_password(Some("one-shot".into()));
        app.skip_running_check_once = true;

        app.process_action(Action::LaunchGame);

        let args = read_launch_args(&capture);
        assert!(args.iter().any(|arg| arg == "-password=one-shot"));
        assert!(app.launch_prep.is_none());

        drop(path_env);
        fs::remove_dir_all(&dayz_path).expect("remove dayz path");
        fs::remove_dir_all(&workshop_path).expect("remove workshop path");
        fs::remove_dir_all(bin_dir).expect("remove bin dir");
    }

    #[test]
    fn offline_launch_reads_mission_mods_and_spawn_flag_from_shared_launch_prep() {
        let _guard = env_lock();
        let bin_dir = temp_path("app-offline-bin");
        setup_launch_bin(&bin_dir, false);
        let path_env = prepend_path(&bin_dir);
        let capture = temp_path("app-offline-args");
        let _capture_env = EnvVarGuard::set("FAKE_STEAM_ARGS", capture.as_os_str());
        let dayz_path = temp_path("app-offline-dayz");
        let workshop_path = temp_path("app-offline-workshop");
        let mission_id = "DayZCommunityOfflineMode.ChernarusPlus".to_string();
        let mission_dir = dayz_path.join("Missions").join(&mission_id).join("core");
        fs::create_dir_all(&mission_dir).expect("create offline mission dir");
        fs::write(
            mission_dir.join("CommunityOfflineClient.c"),
            "bool HIVE_ENABLED = false;\n",
        )
        .expect("write offline mission file");
        fs::create_dir_all(&workshop_path).expect("create workshop path");

        let mut app = test_app();
        app.dayz_path = Some(dayz_path.clone());
        app.workshop_path = Some(workshop_path.clone());
        app.mods_db = ModsDb {
            sum: String::new(),
            mods: vec![crate::mods::ModInfo {
                name: "Mod 1564026768".into(),
                id: 1564026768,
                timestamp: 0,
                size: 0,
            }],
        };
        app.launch_prep = Some(LaunchPrep {
            target: LaunchTarget::Offline {
                mission_id: mission_id.clone(),
            },
            mod_ids: vec![1564026768],
            password: None,
            offline_spawn_enabled: Some(true),
        });
        app.skip_running_check_once = true;

        app.process_action(Action::LaunchGame);

        let args = read_launch_args(&capture);
        assert!(
            args.iter()
                .any(|arg| arg == &format!("-mission=./Missions/{mission_id}"))
        );
        assert!(args.iter().any(|arg| arg == "-mod=@1564026768"));
        assert!(args.iter().any(|arg| arg == "-filePatching"));
        assert!(args.iter().any(|arg| arg == "-doLogs"));
        assert!(args.iter().any(|arg| arg == "-scriptDebug=true"));
        assert!(app.launch_prep.is_none());

        let content = fs::read_to_string(
            dayz_path
                .join("Missions")
                .join(&mission_id)
                .join("core")
                .join("CommunityOfflineClient.c"),
        )
        .expect("read toggled offline mission");
        assert!(content.contains("HIVE_ENABLED = true"));

        drop(path_env);
        fs::remove_dir_all(&dayz_path).expect("remove dayz path");
        fs::remove_dir_all(&workshop_path).expect("remove workshop path");
        fs::remove_dir_all(bin_dir).expect("remove bin dir");
    }

    #[test]
    fn offline_launch_resume_applies_spawn_toggle_before_final_handoff() {
        let _guard = env_lock();
        let bin_dir = temp_path("app-offline-resume-bin");
        setup_launch_bin(&bin_dir, false);
        let path_env = prepend_path(&bin_dir);
        let capture = temp_path("app-offline-resume-args");
        let _capture_env = EnvVarGuard::set("FAKE_STEAM_ARGS", capture.as_os_str());
        let dayz_path = temp_path("app-offline-resume-dayz");
        let workshop_path = temp_path("app-offline-resume-workshop");
        let mission_id = "DayZCommunityOfflineMode.ChernarusPlus".to_string();
        let mission_dir = dayz_path.join("Missions").join(&mission_id).join("core");
        fs::create_dir_all(&mission_dir).expect("create offline mission dir");
        let client_file = mission_dir.join("CommunityOfflineClient.c");
        fs::write(&client_file, "bool HIVE_ENABLED = false;\n")
            .expect("write offline mission file");
        fs::create_dir_all(&workshop_path).expect("create workshop path");

        let mut app = test_app();
        app.dayz_path = Some(dayz_path.clone());
        app.workshop_path = Some(workshop_path.clone());
        app.pending_launch = Some(PendingLaunch {
            args: crate::launch::build_offline_launch_args(
                &mission_id,
                &[1564026768],
                "Survivor",
                &[],
            ),
            all_mod_ids: vec![1564026768],
            pending_mod_ids: Vec::new(),
            history_entry: None,
            offline_update: Some((mission_id.clone(), Some(true))),
            kind: PendingDownloadKind::Launch,
        });

        app.tick();

        let args = read_launch_args(&capture);
        assert!(
            args.iter()
                .any(|arg| arg == &format!("-mission=./Missions/{mission_id}"))
        );
        assert!(args.iter().any(|arg| arg == "-mod=@1564026768"));
        assert_eq!(
            fs::read_to_string(&client_file).expect("read mission file"),
            "bool HIVE_ENABLED = true;\n"
        );
        assert!(app.pending_launch.is_none());

        drop(path_env);
        fs::remove_dir_all(dayz_path).expect("remove dayz path");
        fs::remove_dir_all(workshop_path).expect("remove workshop path");
        fs::remove_dir_all(bin_dir).expect("remove bin dir");
    }

    #[test]
    fn offline_launch_defers_spawn_mutation_until_common_prereqs_pass() {
        let _guard = env_lock();
        let bin_dir = temp_path("app-offline-order-bin");
        setup_launch_bin(&bin_dir, false);
        let path_env = prepend_path(&bin_dir);
        let dayz_path = temp_path("app-offline-order-dayz");
        let mission_id = "DayZCommunityOfflineMode.ChernarusPlus".to_string();
        let mission_dir = dayz_path.join("Missions").join(&mission_id).join("core");
        fs::create_dir_all(&mission_dir).expect("create offline mission dir");
        let client_file = mission_dir.join("CommunityOfflineClient.c");
        fs::write(&client_file, "bool HIVE_ENABLED = false;\n")
            .expect("write offline mission file");

        let mut app = test_app();
        app.dayz_path = Some(dayz_path.clone());
        app.launch_prep = Some(LaunchPrep {
            target: LaunchTarget::Offline {
                mission_id: mission_id.clone(),
            },
            mod_ids: vec![1564026768],
            password: None,
            offline_spawn_enabled: Some(true),
        });
        app.skip_running_check_once = true;

        app.process_action(Action::LaunchGame);

        assert_eq!(
            app.status_message.as_deref(),
            Some("Cannot manage server mods: Steam library path not detected")
        );
        assert!(app.launch_prep.is_some());
        assert_eq!(
            fs::read_to_string(&client_file).expect("read mission file"),
            "bool HIVE_ENABLED = false;\n"
        );

        drop(path_env);
        fs::remove_dir_all(dayz_path).expect("remove dayz path");
        fs::remove_dir_all(bin_dir).expect("remove bin dir");
    }

    #[test]
    fn launch_game_requires_launch_prep() {
        let _guard = env_lock();
        let bin_dir = temp_path("app-no-prep-bin");
        setup_launch_bin(&bin_dir, false);
        let path_env = prepend_path(&bin_dir);
        let capture = temp_path("app-no-prep-args");
        let _capture_env = EnvVarGuard::set("FAKE_STEAM_ARGS", capture.as_os_str());
        let mut app = test_app();

        app.process_action(Action::LaunchGame);

        assert_eq!(
            app.status_message.as_deref(),
            Some("No launch target selected")
        );
        assert!(!capture.exists());
        assert!(app.launch_prep.is_none());

        drop(path_env);
        fs::remove_dir_all(bin_dir).expect("remove bin dir");
    }

    #[test]
    fn offline_launch_preserves_prep_when_spawn_toggle_fails() {
        let _guard = env_lock();
        let bin_dir = temp_path("app-offline-failure-bin");
        setup_launch_bin(&bin_dir, false);
        let path_env = prepend_path(&bin_dir);
        let capture = temp_path("app-offline-failure-args");
        let _capture_env = EnvVarGuard::set("FAKE_STEAM_ARGS", capture.as_os_str());
        let dayz_path = temp_path("app-offline-failure");
        fs::create_dir_all(&dayz_path).expect("create dayz path");

        let mut app = test_app();
        app.dayz_path = Some(dayz_path.clone());
        app.launch_prep = Some(LaunchPrep {
            target: LaunchTarget::Offline {
                mission_id: "DayZCommunityOfflineMode.ChernarusPlus".into(),
            },
            mod_ids: Vec::new(),
            password: None,
            offline_spawn_enabled: Some(true),
        });

        app.process_action(Action::LaunchGame);

        assert!(
            app.status_message
                .as_deref()
                .unwrap_or_default()
                .starts_with("Failed to update offline spawn setting:")
        );
        assert!(app.launch_prep.is_some());

        drop(path_env);
        fs::remove_dir_all(dayz_path).expect("remove dayz path");
        fs::remove_dir_all(bin_dir).expect("remove bin dir");
    }

    #[test]
    fn launch_prompts_to_kill_existing_dayz_process() {
        let _guard = env_lock();
        let bin_dir = temp_path("app-kill-bin");
        setup_launch_bin(&bin_dir, true);
        let path_env = prepend_path(&bin_dir);
        let mut app = test_app();
        app.prepare_direct_connect_launch("1.2.3.4".into(), 2302);

        app.process_action(Action::LaunchGame);

        assert_eq!(app.screen_stack.len(), 2);
        assert!(app.running);

        drop(path_env);
        fs::remove_dir_all(bin_dir).expect("remove bin dir");
    }

    #[test]
    fn launch_prompts_for_password_on_protected_server() {
        let _guard = env_lock();
        let bin_dir = temp_path("app-password-bin");
        setup_launch_bin(&bin_dir, false);
        let path_env = prepend_path(&bin_dir);
        let mut app = test_app();
        app.servers.push(sample_server(true));
        app.prepare_known_server_launch(0);

        app.process_action(Action::LaunchGame);

        assert_eq!(app.screen_stack.len(), 2);
        assert!(app.running);

        drop(path_env);
        fs::remove_dir_all(bin_dir).expect("remove bin dir");
    }
}
