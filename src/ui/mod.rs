pub mod config_screen;
pub mod direct_connect;
pub mod filter;
pub mod main_menu;
pub mod news;
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
    ReplaceScreen(ScreenId),
    LaunchGame,
    RunSelfUpdate,
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
    FilterSelect,
    UpdatePrompt,
    Confirm(ConfirmAction),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConfirmAction {
    Quit,
    KillDayZ,
    RemoveManagedMods,
    RemoveModLinks,
    UpdateModsBeforeLaunch,
    MigrateLegacy,
}

pub trait Screen {
    fn render(&mut self, f: &mut Frame, app: &App);
    fn handle_key(&mut self, key: KeyEvent, app: &mut App) -> Action;
    fn on_enter(&mut self, _app: &mut App) {}
    fn on_tick(&mut self, _app: &mut App) -> Action {
        Action::None
    }
}

#[allow(unused_imports)]
pub(crate) use crate::app::{LaunchPrep as AppLaunchPrep, LaunchTarget as AppLaunchTarget};
