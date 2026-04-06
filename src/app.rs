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
            selected_server: None,
            direct_connect_target: None,
            server_runtime: HashMap::new(),
            available_update: None,
            update_mods_before_launch: false,
            skip_running_check_once: false,
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
            ScreenId::FilterSelect => Box::new(filter::FilterSelectScreen::new(self)),
            ScreenId::UpdatePrompt => Box::new(update_prompt::UpdatePromptScreen::new()),
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
            self.status_message =
                Some("Cannot manage server mods: Steam library path not detected".into());
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
    use crate::config::Config;
    use crate::profile::Profile;
    use crate::steam::ItemState;
    use std::ffi::OsString;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use std::sync::{Mutex, MutexGuard};
    use std::time::{SystemTime, UNIX_EPOCH};

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

    fn write_executable(path: &PathBuf, body: &str) {
        fs::write(path, body).expect("write script");
        let mut perms = fs::metadata(path).expect("script metadata").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms).expect("set script permissions");
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
    fn launch_prompts_to_kill_existing_dayz_process() {
        let _guard = env_lock();
        let bin_dir = temp_path("app-bin");
        fs::create_dir_all(&bin_dir).expect("create bin dir");
        write_executable(&bin_dir.join("pgrep"), "#!/bin/sh\nexit 0\n");
        write_executable(&bin_dir.join("steam"), "#!/bin/sh\nexit 0\n");

        let original_path = std::env::var_os("PATH").unwrap_or_default();
        let mut combined_path = OsString::from(bin_dir.as_os_str());
        if !original_path.is_empty() {
            combined_path.push(":");
            combined_path.push(&original_path);
        }
        let path_env = EnvVarGuard::set("PATH", &combined_path);
        let mut app = test_app();
        app.direct_connect_target = Some(("1.2.3.4".into(), 2302));

        app.process_action(Action::LaunchGame);

        assert_eq!(app.screen_stack.len(), 2);
        assert!(app.running);

        drop(path_env);
        fs::remove_dir_all(bin_dir).expect("remove bin dir");
    }
}
