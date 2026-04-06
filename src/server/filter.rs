use super::types::Server;

#[derive(Debug, Clone)]
pub enum ServerFilter {
    Official,
    NotOfficial,
    Modded,
    NotModded,
    WithPassword,
    WithoutPassword,
    FirstPerson,
    ThirdPerson,
    DayTime,
    NightTime,
    WithBattlEye,
    WithoutBattlEye,
    WithPlayers,
    WithoutPlayers,
    NotFull,
    Full,
    LinuxServers,
    WindowsServers,
    ModsGreaterThan(u32),
    ModsLessThan(u32),
    PlayersGreaterThan(u32),
    PlayersLessThan(u32),
    PlayerSlotsAtLeast(u32),
    MapName(String),
    ModName(String),
    ModId(u64),
}

impl ServerFilter {
    pub fn matches(&self, server: &Server) -> bool {
        match self {
            ServerFilter::Official => server.is_official(),
            ServerFilter::NotOfficial => !server.is_official(),
            ServerFilter::Modded => server.is_modded(),
            ServerFilter::NotModded => !server.is_modded(),
            ServerFilter::WithPassword => server.password,
            ServerFilter::WithoutPassword => !server.password,
            ServerFilter::FirstPerson => server.first_person_only,
            ServerFilter::ThirdPerson => !server.first_person_only,
            ServerFilter::DayTime => server.is_day(),
            ServerFilter::NightTime => !server.is_day(),
            ServerFilter::WithBattlEye => server.battleye,
            ServerFilter::WithoutBattlEye => !server.battleye,
            ServerFilter::WithPlayers => server.players > 0,
            ServerFilter::WithoutPlayers => server.players == 0,
            ServerFilter::NotFull => !server.is_full(),
            ServerFilter::Full => server.is_full(),
            ServerFilter::LinuxServers => server.is_linux(),
            ServerFilter::WindowsServers => !server.is_linux(),
            ServerFilter::ModsGreaterThan(n) => server.is_modded() && server.mods.len() as u32 > *n,
            ServerFilter::ModsLessThan(n) => server.is_modded() && (server.mods.len() as u32) < *n,
            ServerFilter::PlayersGreaterThan(pct) => server.player_percent() > *pct,
            ServerFilter::PlayersLessThan(pct) => server.player_percent() < *pct,
            ServerFilter::PlayerSlotsAtLeast(n) => server.max_players >= *n,
            ServerFilter::MapName(name) => {
                server.map.to_lowercase().contains(&name.to_lowercase())
            }
            ServerFilter::ModName(name) => {
                let lower = name.to_lowercase();
                server.mods.iter().any(|m| m.name.to_lowercase().contains(&lower))
            }
            ServerFilter::ModId(id) => {
                server.mods.iter().any(|m| m.steam_workshop_id == *id)
            }
        }
    }

    pub fn label(&self) -> String {
        match self {
            ServerFilter::Official => "Official Servers".into(),
            ServerFilter::NotOfficial => "Community Servers".into(),
            ServerFilter::Modded => "Modded".into(),
            ServerFilter::NotModded => "Unmodded".into(),
            ServerFilter::WithPassword => "With Password".into(),
            ServerFilter::WithoutPassword => "Without Password".into(),
            ServerFilter::FirstPerson => "First Person".into(),
            ServerFilter::ThirdPerson => "Third Person".into(),
            ServerFilter::DayTime => "Day Time".into(),
            ServerFilter::NightTime => "Night Time".into(),
            ServerFilter::WithBattlEye => "With BattlEye".into(),
            ServerFilter::WithoutBattlEye => "Without BattlEye".into(),
            ServerFilter::WithPlayers => "With Players".into(),
            ServerFilter::WithoutPlayers => "Empty Servers".into(),
            ServerFilter::NotFull => "Not Full".into(),
            ServerFilter::Full => "Full".into(),
            ServerFilter::LinuxServers => "Linux Servers".into(),
            ServerFilter::WindowsServers => "Windows Servers".into(),
            ServerFilter::ModsGreaterThan(n) => format!("Mods > {n}"),
            ServerFilter::ModsLessThan(n) => format!("Mods < {n}"),
            ServerFilter::PlayersGreaterThan(n) => format!("Players > {n}%"),
            ServerFilter::PlayersLessThan(n) => format!("Players < {n}%"),
            ServerFilter::PlayerSlotsAtLeast(n) => format!("Slots >= {n}"),
            ServerFilter::MapName(m) => format!("Map: {m}"),
            ServerFilter::ModName(m) => format!("Mod: {m}"),
            ServerFilter::ModId(id) => format!("Mod ID: {id}"),
        }
    }
}

pub fn apply_filters(servers: &[Server], filters: &[ServerFilter]) -> Vec<usize> {
    servers
        .iter()
        .enumerate()
        .filter(|(_, s)| filters.iter().all(|f| f.matches(s)))
        .map(|(i, _)| i)
        .collect()
}
