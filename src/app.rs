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
    pub launch_prep: Option<LaunchPrep>,
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
            launch_prep: None,
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
        let Some(prep) = self.launch_prep.as_ref() else {
            self.status_message = Some("No launch target selected".into());
            return;
        };

        let has_mods = !prep.mod_ids.is_empty();

        if has_mods && self.steam.is_some() && !self.asked_update_mods {
            self.asked_update_mods = true;
            let mut screen =
                self.create_screen(ScreenId::Confirm(ConfirmAction::UpdateModsBeforeLaunch));
            screen.on_enter(self);
            self.screen_stack.push(screen);
            return;
        }

        let Some(mut prep) = self.launch_prep.take() else {
            self.status_message = Some("No launch target selected".into());
            return;
        };
        let player = self
            .profile
            .player
            .clone()
            .unwrap_or_else(|| "Survivor".into());
        let extra_args = self.profile.get_launch_args();

        let password = prep.password.take();
        let offline_spawn_enabled = prep.offline_spawn_enabled.take();
        let mod_ids = prep.mod_ids.clone();

        let (args, history_entry) = match prep.target {
            LaunchTarget::KnownServer(idx) => {
                let Some(server) = self.servers.get(idx) else {
                    self.status_message = Some(format!("Launch target server {idx} is unavailable"));
                    self.asked_update_mods = false;
                    self.update_mods_before_launch = false;
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
                let args = crate::launch::build_direct_connect_args_with_mods(
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
                let Some(dayz_path) = self.dayz_path.as_ref() else {
                    self.status_message =
                        Some("Cannot launch offline: DayZ path not detected".into());
                    self.asked_update_mods = false;
                    self.update_mods_before_launch = false;
                    return;
                };

                if let Err(e) = crate::launch::apply_offline_spawn_setting(
                    dayz_path,
                    &mission_id,
                    offline_spawn_enabled,
                ) {
                    self.status_message = Some(format!(
                        "Failed to update offline spawn setting: {e}"
                    ));
                    self.asked_update_mods = false;
                    self.update_mods_before_launch = false;
                    return;
                }

                let args = crate::launch::build_offline_launch_args(
                    &mission_id,
                    &mod_ids,
                    &player,
                    &extra_args,
                );
                (args, None)
            }
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
    use crate::server::types::ServerEndpoint;
    use crate::steam::ItemState;
    use crate::server::Server;
    use std::ffi::OsString;
    use std::fs;
    use std::io::Read;
    use std::path::PathBuf;
    use std::sync::{Mutex, OnceLock};

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

    fn sample_server(name: &str, ip: &str, game_port: u16, mods: &[u64]) -> Server {
        Server {
            name: name.into(),
            players: 12,
            max_players: 60,
            time: "12:00".into(),
            time_acceleration: Some(4.0),
            map: "chernarusplus".into(),
            password: false,
            battleye: true,
            vac: true,
            first_person_only: false,
            shard: "public".into(),
            version: "1.0".into(),
            environment: "w".into(),
            game_port,
            endpoint: ServerEndpoint {
                ip: ip.into(),
                port: 27016,
            },
            mods: mods
                .iter()
                .copied()
                .map(|id| crate::server::types::ServerMod {
                    steam_workshop_id: id,
                    name: format!("Mod {id}"),
                })
                .collect(),
        }
    }

    fn test_guard() -> std::sync::MutexGuard<'static, ()> {
        static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
        GUARD
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
    }

    struct FakeSteam {
        old_path: Option<OsString>,
        old_capture: Option<OsString>,
        root: PathBuf,
        capture: PathBuf,
    }

    impl FakeSteam {
        fn install() -> Self {
            let root = std::env::temp_dir().join(format!(
                "dayz-cmd-fake-steam-{}-{}",
                std::process::id(),
                chrono::Utc::now().timestamp_nanos_opt().expect("timestamp")
            ));
            let bin_dir = root.join("bin");
            fs::create_dir_all(&bin_dir).expect("create fake steam bin dir");
            let capture = root.join("args.txt");
            let steam = bin_dir.join("steam");
            fs::write(
                &steam,
                "#!/bin/sh\nprintf '%s\n' \"$@\" > \"$FAKE_STEAM_ARGS\"\n",
            )
            .expect("write fake steam script");
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = fs::metadata(&steam).expect("stat fake steam").permissions();
                perms.set_mode(0o755);
                fs::set_permissions(&steam, perms).expect("chmod fake steam");
            }

            let old_path = std::env::var_os("PATH");
            let old_capture = std::env::var_os("FAKE_STEAM_ARGS");
            let new_path = match &old_path {
                Some(existing) => format!("{}:{}", bin_dir.display(), existing.to_string_lossy()),
                None => bin_dir.display().to_string(),
            };
            // Test-only process env mutation to steer Command::new("steam").
            unsafe {
                std::env::set_var("PATH", new_path);
                std::env::set_var("FAKE_STEAM_ARGS", &capture);
            }

            Self {
                old_path,
                old_capture,
                root,
                capture,
            }
        }

        fn launched_args(&self) -> Vec<String> {
            use std::thread;
            use std::time::Duration;

            for _ in 0..50 {
                if self.capture.exists() {
                    break;
                }
                thread::sleep(Duration::from_millis(10));
            }

            let mut content = String::new();
            fs::File::open(&self.capture)
                .expect("open captured args")
                .read_to_string(&mut content)
                .expect("read captured args");
            content
                .lines()
                .map(|line| line.to_string())
                .collect::<Vec<_>>()
        }

        fn was_launched(&self) -> bool {
            self.capture.exists()
        }
    }

    impl Drop for FakeSteam {
        fn drop(&mut self) {
            // Restore the process env after the test-specific fake launcher.
            unsafe {
                if let Some(old_path) = &self.old_path {
                    std::env::set_var("PATH", old_path);
                } else {
                    std::env::remove_var("PATH");
                }

                if let Some(old_capture) = &self.old_capture {
                    std::env::set_var("FAKE_STEAM_ARGS", old_capture);
                } else {
                    std::env::remove_var("FAKE_STEAM_ARGS");
                }
            }

            let _ = fs::remove_dir_all(&self.root);
        }
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
    fn known_server_launch_reads_target_from_shared_launch_prep() {
        let _guard = test_guard();
        let fake_steam = FakeSteam::install();
        let mut app = test_app();
        app.servers = vec![
            sample_server("Ignored Server", "10.0.0.1", 2302, &[]),
            sample_server("Shared Prep Server", "10.0.0.2", 2402, &[123456789]),
        ];
        app.mods_db = crate::mods::ModsDb {
            sum: String::new(),
            mods: vec![crate::mods::ModInfo {
                name: "Mod 123456789".into(),
                id: 123456789,
                timestamp: 0,
                size: 0,
            }],
        };
        app.dayz_path = Some(std::env::temp_dir().join("dayz-cmd-dayz-path"));
        app.workshop_path = Some(std::env::temp_dir().join("dayz-cmd-workshop-path"));
        fs::create_dir_all(app.dayz_path.as_ref().expect("dayz path")).expect("create dayz path");
        fs::create_dir_all(app.workshop_path.as_ref().expect("workshop path"))
            .expect("create workshop path");
        app.launch_prep = Some(LaunchPrep {
            target: LaunchTarget::KnownServer(1),
            mod_ids: vec![123456789],
            password: None,
            offline_spawn_enabled: None,
        });

        app.do_launch();

        let args = fake_steam.launched_args();
        assert!(args.iter().any(|arg| arg == "-connect=10.0.0.2"));
        assert!(args.iter().any(|arg| arg == "-port=2402"));
        assert!(args.iter().any(|arg| arg == "-mod=@123456789"));
    }

    #[test]
    fn direct_connect_launch_reads_ip_port_mods_and_password_from_shared_launch_prep() {
        let _guard = test_guard();
        let fake_steam = FakeSteam::install();
        let mut app = test_app();
        app.mods_db = crate::mods::ModsDb {
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
        app.dayz_path = Some(std::env::temp_dir().join("dayz-cmd-dayz-path-direct"));
        app.workshop_path = Some(std::env::temp_dir().join("dayz-cmd-workshop-path-direct"));
        fs::create_dir_all(app.dayz_path.as_ref().expect("dayz path")).expect("create dayz path");
        fs::create_dir_all(app.workshop_path.as_ref().expect("workshop path"))
            .expect("create workshop path");
        app.launch_prep = Some(LaunchPrep {
            target: LaunchTarget::DirectConnect {
                ip: "5.6.7.8".into(),
                port: 2402,
            },
            mod_ids: vec![111, 222],
            password: Some("secret".into()),
            offline_spawn_enabled: None,
        });

        app.do_launch();

        let args = fake_steam.launched_args();
        assert!(args.iter().any(|arg| arg == "-connect=5.6.7.8"));
        assert!(args.iter().any(|arg| arg == "-port=2402"));
        assert!(args.iter().any(|arg| arg == "-mod=@111;@222"));
        assert!(args.iter().any(|arg| arg == "-password=secret"));
    }

    #[test]
    fn launch_consumes_one_shot_password_and_prep_state_after_building_args() {
        let _guard = test_guard();
        let fake_steam = FakeSteam::install();
        let mut app = test_app();
        app.dayz_path = Some(std::env::temp_dir().join("dayz-cmd-dayz-path-consume"));
        app.workshop_path = Some(std::env::temp_dir().join("dayz-cmd-workshop-path-consume"));
        fs::create_dir_all(app.dayz_path.as_ref().expect("dayz path")).expect("create dayz path");
        fs::create_dir_all(app.workshop_path.as_ref().expect("workshop path"))
            .expect("create workshop path");
        app.launch_prep = Some(LaunchPrep {
            target: LaunchTarget::DirectConnect {
                ip: "9.8.7.6".into(),
                port: 2502,
            },
            mod_ids: Vec::new(),
            password: Some("one-shot".into()),
            offline_spawn_enabled: Some(true),
        });

        app.do_launch();

        assert!(fake_steam
            .launched_args()
            .iter()
            .any(|arg| arg == "-password=one-shot"));
        assert!(app.launch_prep.is_none());
    }

    #[test]
    fn launch_game_requires_launch_prep() {
        let _guard = test_guard();
        let fake_steam = FakeSteam::install();
        let mut app = test_app();

        app.process_action(Action::LaunchGame);

        assert_eq!(app.status_message.as_deref(), Some("No launch target selected"));
        assert!(!fake_steam.was_launched());
        assert!(app.launch_prep.is_none());
    }
}
