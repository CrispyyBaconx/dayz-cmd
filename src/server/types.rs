use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerListResponse {
    pub status: i32,
    pub result: Vec<Server>,
    #[serde(default)]
    #[serde(rename = "playersOnline")]
    pub players_online: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Server {
    pub name: String,
    #[serde(default)]
    pub players: u32,
    #[serde(default, rename = "maxPlayers")]
    pub max_players: u32,
    #[serde(default)]
    pub time: String,
    #[serde(default, rename = "timeAcceleration")]
    pub time_acceleration: Option<f32>,
    #[serde(default)]
    pub map: String,
    #[serde(default)]
    pub password: bool,
    #[serde(default, rename = "battlEye")]
    pub battleye: bool,
    #[serde(default)]
    pub vac: bool,
    #[serde(default, rename = "firstPersonOnly")]
    pub first_person_only: bool,
    #[serde(default)]
    pub shard: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub environment: String,
    #[serde(default, rename = "gamePort")]
    pub game_port: u16,
    pub endpoint: ServerEndpoint,
    #[serde(default)]
    pub mods: Vec<ServerMod>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerEndpoint {
    pub ip: String,
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerMod {
    pub name: String,
    #[serde(rename = "steamWorkshopId")]
    pub steam_workshop_id: u64,
}

impl Server {
    pub fn is_day(&self) -> bool {
        self.time
            .split(':')
            .next()
            .and_then(|h| h.parse::<u32>().ok())
            .map(|h| (6..=18).contains(&h))
            .unwrap_or(true)
    }

    pub fn is_official(&self) -> bool {
        self.shard == "public"
    }

    pub fn is_modded(&self) -> bool {
        !self.mods.is_empty()
    }

    pub fn is_linux(&self) -> bool {
        self.environment == "l"
    }

    pub fn is_full(&self) -> bool {
        self.max_players > 0 && self.players >= self.max_players
    }

    pub fn player_percent(&self) -> u32 {
        if self.max_players == 0 {
            0
        } else {
            (self.players * 100) / self.max_players
        }
    }

    pub fn time_icon(&self) -> &str {
        if self.is_day() { "☀" } else { "🌙" }
    }

    pub fn platform_str(&self) -> &str {
        if self.is_linux() {
            "Linux"
        } else {
            "Windows"
        }
    }
}
