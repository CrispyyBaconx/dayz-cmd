use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModsDb {
    #[serde(default)]
    pub sum: String,
    #[serde(default)]
    pub mods: Vec<ModInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModInfo {
    pub name: String,
    pub id: u64,
    #[serde(default)]
    pub timestamp: i64,
    #[serde(default)]
    pub size: u64,
}

impl ModsDb {
    pub fn is_installed(&self, workshop_id: u64) -> bool {
        self.mods.iter().any(|m| m.id == workshop_id)
    }

    pub fn get_mod(&self, workshop_id: u64) -> Option<&ModInfo> {
        self.mods.iter().find(|m| m.id == workshop_id)
    }

    pub fn total_size_bytes(&self) -> u64 {
        self.mods.iter().map(|m| m.size).sum()
    }

    pub fn total_size_human(&self) -> String {
        let bytes = self.total_size_bytes();
        if bytes >= 1_073_741_824 {
            format!("{:.2} GB", bytes as f64 / 1_073_741_824.0)
        } else if bytes >= 1_048_576 {
            format!("{:.1} MB", bytes as f64 / 1_048_576.0)
        } else {
            format!("{} KB", bytes / 1024)
        }
    }
}

impl ModInfo {
    pub fn size_human(&self) -> String {
        if self.size >= 1_073_741_824 {
            format!("{:.2} GB", self.size as f64 / 1_073_741_824.0)
        } else if self.size >= 1_048_576 {
            format!("{:.1} MB", self.size as f64 / 1_048_576.0)
        } else {
            format!("{} KB", self.size / 1024)
        }
    }

    pub fn workshop_url(&self) -> String {
        format!(
            "https://steamcommunity.com/sharedfiles/filedetails/?id={}",
            self.id
        )
    }
}
