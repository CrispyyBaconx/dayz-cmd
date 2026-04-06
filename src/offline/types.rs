use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct OfflineState {
    #[serde(default)]
    pub installed_tag: Option<String>,
    #[serde(default)]
    pub latest_known_tag: Option<String>,
    #[serde(default)]
    pub managed_missions: Vec<String>,
    #[serde(default)]
    pub last_check_ts: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct OfflineMissionPrefs {
    #[serde(default)]
    pub mod_ids: Vec<u64>,
    #[serde(default)]
    pub spawn_enabled: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum MissionSource {
    Managed,
    Existing,
}
