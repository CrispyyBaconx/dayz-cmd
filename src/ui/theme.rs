use ratatui::style::{Color, Modifier, Style};

pub const TITLE: Style = Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD);
pub const HIGHLIGHT: Style = Style::new()
    .fg(Color::Yellow)
    .add_modifier(Modifier::BOLD);
pub const SELECTED: Style = Style::new()
    .bg(Color::DarkGray)
    .fg(Color::White)
    .add_modifier(Modifier::BOLD);
pub const DIM: Style = Style::new().fg(Color::DarkGray);
pub const NORMAL: Style = Style::new().fg(Color::White);
pub const SUCCESS: Style = Style::new().fg(Color::Green);
pub const ERROR: Style = Style::new().fg(Color::Red);
pub const WARNING: Style = Style::new().fg(Color::Yellow);
pub const INFO: Style = Style::new().fg(Color::Cyan);
pub const BORDER: Style = Style::new().fg(Color::DarkGray);
pub const KEY_HINT: Style = Style::new().fg(Color::DarkGray);
pub const SEARCH_INPUT: Style = Style::new()
    .fg(Color::White)
    .add_modifier(Modifier::BOLD);
