#[cfg(feature = "steam")]
pub mod workshop;

#[cfg(feature = "steam")]
use std::sync::{Arc, Mutex};
#[cfg(feature = "steam")]
use std::thread;
#[cfg(feature = "steam")]
use steamworks::Client;

#[cfg(feature = "steam")]
pub use workshop::ItemState;

#[cfg(not(feature = "steam"))]
#[derive(Debug, Clone, PartialEq)]
pub enum ItemState {
    NotInstalled,
    Installed,
    Downloading,
    NeedsUpdate,
}

#[cfg(feature = "steam")]
pub struct SteamHandle {
    pub client: Arc<Mutex<Client>>,
    _pump_thread: thread::JoinHandle<()>,
}

#[cfg(not(feature = "steam"))]
pub struct SteamHandle;

#[cfg(feature = "steam")]
impl SteamHandle {
    pub fn init() -> anyhow::Result<Self> {
        let client = Client::init()?;
        let client = Arc::new(Mutex::new(client));
        let pump_client = Arc::clone(&client);

        let _pump_thread = thread::spawn(move || {
            loop {
                {
                    let c = pump_client.lock().unwrap();
                    c.run_callbacks();
                }
                thread::sleep(std::time::Duration::from_millis(50));
            }
        });

        Ok(SteamHandle {
            client,
            _pump_thread,
        })
    }

    pub fn user_name(&self) -> String {
        let client = self.client.lock().unwrap();
        let friends = client.friends();
        friends.name()
    }

    pub fn ensure_mods_downloaded(&self, workshop_ids: &[u64]) -> anyhow::Result<Vec<u64>> {
        workshop::ensure_mods_downloaded(&self.client, workshop_ids)
    }

    pub fn get_item_state(&self, workshop_id: u64) -> ItemState {
        workshop::get_item_state(&self.client, workshop_id)
    }

    pub fn get_download_progress(&self, workshop_id: u64) -> Option<(u64, u64)> {
        workshop::get_download_progress(&self.client, workshop_id)
    }
}

#[cfg(not(feature = "steam"))]
impl SteamHandle {
    pub fn init() -> anyhow::Result<Self> {
        anyhow::bail!("Steam support not compiled in")
    }

    pub fn user_name(&self) -> String {
        "Unknown".into()
    }

    pub fn ensure_mods_downloaded(&self, _workshop_ids: &[u64]) -> anyhow::Result<Vec<u64>> {
        anyhow::bail!("Steam support not compiled in")
    }

    pub fn get_item_state(&self, _workshop_id: u64) -> ItemState {
        ItemState::NotInstalled
    }

    pub fn get_download_progress(&self, _workshop_id: u64) -> Option<(u64, u64)> {
        None
    }
}
