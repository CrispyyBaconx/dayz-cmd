pub mod config_screen;
pub mod direct_connect;
pub mod direct_connect_setup;
pub mod filter;
pub mod info_screen;
pub mod main_menu;
pub mod news;
pub mod offline_browser;
pub mod offline_setup;
pub mod password_prompt;
pub mod popup;
pub mod server_browser;
pub mod server_detail;
pub mod theme;
pub mod update_prompt;

use crossterm::event::KeyEvent;
use ratatui::Frame;

use crate::app::App;

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    None,
    Quit,
    PushScreen(ScreenId),
    PopScreen,
    PopScreenAndLaunchGame,
    ReplaceScreen(ScreenId),
    LaunchGame,
    RunSelfUpdate,
    CheckForUpdates,
    RefreshInstalledMods,
    OfflineInstallOrUpdate,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ScreenId {
    MainMenu,
    ServerBrowser,
    FilteredBrowser(Vec<usize>),
    FavoritesBrowser,
    HistoryBrowser,
    ServerDetail(usize),
    Config,
    News,
    DirectConnect,
    OfflineBrowser,
    DirectConnectSetup,
    OfflineSetup,
    PasswordPrompt,
    FilterSelect,
    UpdatePrompt,
    Info(InfoScreenData),
    Confirm(ConfirmAction),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InfoScreenData {
    pub title: String,
    pub lines: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConfirmAction {
    Quit,
    KillDayZ,
    RemoveManagedMods,
    RemoveModLinks,
    UpdateModsBeforeLaunch,
    MigrateLegacy,
    FixMaxMapCount,
}

pub trait Screen {
    fn render(&mut self, f: &mut Frame, app: &App);
    fn handle_key(&mut self, key: KeyEvent, app: &mut App) -> Action;
    fn shows_status_bar(&self) -> bool {
        true
    }
    fn on_enter(&mut self, _app: &mut App) {}
    fn on_tick(&mut self, _app: &mut App) -> Action {
        Action::None
    }
}
