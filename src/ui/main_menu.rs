use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;

use super::{theme, Action, Screen, ScreenId};
use crate::app::App;

pub struct MainMenuScreen {
    pub list_state: ListState,
    items: Vec<MenuItem>,
}

#[derive(Debug, Clone)]
struct MenuItem {
    label: String,
    action: MenuAction,
}

#[derive(Debug, Clone)]
enum MenuAction {
    AllServers,
    FilterServers,
    Favorites,
    RecentlyPlayed,
    DirectConnect,
    LaunchGame,
    News,
    Config,
}

impl MainMenuScreen {
    pub fn new() -> Self {
        Self {
            list_state: ListState::default().with_selected(Some(0)),
            items: Vec::new(),
        }
    }

    fn build_items(&mut self, app: &App) {
        let mut items = vec![
            MenuItem {
                label: "All Servers".into(),
                action: MenuAction::AllServers,
            },
            MenuItem {
                label: "Filter Servers".into(),
                action: MenuAction::FilterServers,
            },
        ];

        if !app.profile.favorites.is_empty() {
            items.push(MenuItem {
                label: "Favorite Servers".into(),
                action: MenuAction::Favorites,
            });
        }

        if !app.profile.history.is_empty() {
            items.push(MenuItem {
                label: "Recently Played".into(),
                action: MenuAction::RecentlyPlayed,
            });
        }

        items.extend([
            MenuItem {
                label: "Direct Connect".into(),
                action: MenuAction::DirectConnect,
            },
            MenuItem {
                label: "Launch Game".into(),
                action: MenuAction::LaunchGame,
            },
            MenuItem {
                label: "DayZ News".into(),
                action: MenuAction::News,
            },
            MenuItem {
                label: "Config".into(),
                action: MenuAction::Config,
            },
        ]);

        self.items = items;
    }
}

impl Screen for MainMenuScreen {
    fn on_enter(&mut self, app: &mut App) {
        self.build_items(app);
    }

    fn render(&mut self, f: &mut Frame, app: &App) {
        let area = f.area();
        let news_height = (app.news.len() as u16 + 2).min(8);
        let chunks = Layout::vertical([
            Constraint::Length(3),
            Constraint::Length(news_height),
            Constraint::Length(3),
            Constraint::Min(0),
        ])
        .split(area);

        render_header(f, chunks[0], app);
        render_news(f, chunks[1], app);
        render_stats(f, chunks[2], app);
        self.render_menu(f, chunks[3]);
    }

    fn handle_key(&mut self, key: KeyEvent, _app: &mut App) -> Action {
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return Action::Quit;
        }

        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => Action::Quit,
            KeyCode::Up | KeyCode::Char('k') => {
                let i = self.list_state.selected().unwrap_or(0);
                let new = if i == 0 { self.items.len() - 1 } else { i - 1 };
                self.list_state.select(Some(new));
                Action::None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let i = self.list_state.selected().unwrap_or(0);
                let new = (i + 1) % self.items.len();
                self.list_state.select(Some(new));
                Action::None
            }
            KeyCode::Enter => {
                if let Some(idx) = self.list_state.selected() {
                    if let Some(item) = self.items.get(idx) {
                        return match &item.action {
                            MenuAction::AllServers => Action::PushScreen(ScreenId::ServerBrowser),
                            MenuAction::FilterServers => Action::PushScreen(ScreenId::FilterSelect),
                            MenuAction::Favorites => Action::PushScreen(ScreenId::FavoritesBrowser),
                            MenuAction::RecentlyPlayed => {
                                Action::PushScreen(ScreenId::HistoryBrowser)
                            }
                            MenuAction::DirectConnect => {
                                Action::PushScreen(ScreenId::DirectConnect)
                            }
                            MenuAction::LaunchGame => Action::LaunchGame,
                            MenuAction::News => Action::PushScreen(ScreenId::News),
                            MenuAction::Config => Action::PushScreen(ScreenId::Config),
                        };
                    }
                }
                Action::None
            }
            _ => Action::None,
        }
    }
}

impl MainMenuScreen {
    fn render_menu(&mut self, f: &mut Frame, area: Rect) {
        let items: Vec<ListItem> = self
            .items
            .iter()
            .map(|item| {
                ListItem::new(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(&item.label, theme::NORMAL),
                ]))
            })
            .collect();

        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(theme::BORDER)
                    .title(" Menu "),
            )
            .highlight_style(theme::SELECTED)
            .highlight_symbol("▶ ");

        f.render_stateful_widget(list, area, &mut self.list_state);
    }
}

fn render_header(f: &mut Frame, area: Rect, _app: &App) {
    let title = Paragraph::new(Line::from(vec![
        Span::styled(
            " DayZ Launcher ",
            Style::default()
                .fg(ratatui::style::Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!("v{}", env!("CARGO_PKG_VERSION")), theme::DIM),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(theme::BORDER),
    );
    f.render_widget(title, area);
}

fn render_news(f: &mut Frame, area: Rect, app: &App) {
    let max_items = (area.height as usize).saturating_sub(2);
    let news_lines: Vec<Line> = app
        .news
        .iter()
        .take(max_items)
        .map(|article| {
            Line::from(vec![
                Span::styled(" • ", theme::DIM),
                Span::styled(&article.title, theme::NORMAL),
            ])
        })
        .collect();

    let news = Paragraph::new(news_lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(theme::BORDER)
                .title(" Latest News "),
        )
        .wrap(Wrap { trim: true });
    f.render_widget(news, area);
}

fn render_stats(f: &mut Frame, area: Rect, app: &App) {
    let player_name = app.profile.player.as_deref().unwrap_or("Unknown");

    let players_online = app
        .players_online
        .map(|n| n.to_string())
        .unwrap_or_else(|| "?".into());

    let server_count = app.servers.len();

    let stats = Paragraph::new(Line::from(vec![
        Span::styled(" Player: ", theme::DIM),
        Span::styled(player_name, theme::INFO),
        Span::raw("  "),
        Span::styled("Online: ", theme::DIM),
        Span::styled(&players_online, theme::SUCCESS),
        Span::raw("  "),
        Span::styled("Servers: ", theme::DIM),
        Span::styled(server_count.to_string(), theme::SUCCESS),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(theme::BORDER),
    );
    f.render_widget(stats, area);
}
