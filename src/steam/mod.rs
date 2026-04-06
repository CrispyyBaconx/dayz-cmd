#[cfg(feature = "steam")]
pub mod workshop;

#[cfg(feature = "steam")]
use steamworks::Client;
#[cfg(feature = "steam")]
use std::sync::{Arc, Mutex};
#[cfg(feature = "steam")]
use std::thread;

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
}

#[cfg(not(feature = "steam"))]
impl SteamHandle {
    pub fn init() -> anyhow::Result<Self> {
        anyhow::bail!("Steam support not compiled in")
    }

    pub fn user_name(&self) -> String {
        "Unknown".into()
    }
}
