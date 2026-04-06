use anyhow::{Context, Result};
use std::sync::{Arc, Mutex};
use steamworks::{Client, PublishedFileId};

pub fn subscribe_and_download(client: &Arc<Mutex<Client>>, workshop_id: u64) -> Result<()> {
    let client = client.lock().unwrap();
    let ugc = client.ugc();
    let file_id = PublishedFileId(workshop_id);

    let wid = workshop_id;
    ugc.subscribe_item(file_id, move |result| {
        if let Err(e) = result {
            eprintln!("Failed to subscribe to workshop item {wid}: {e:?}");
        }
    });

    ugc.download_item(file_id, true);
    Ok(())
}

pub fn is_item_installed(client: &Arc<Mutex<Client>>, workshop_id: u64) -> bool {
    let client = client.lock().unwrap();
    let ugc = client.ugc();
    ugc.item_install_info(PublishedFileId(workshop_id))
        .is_some()
}

pub fn get_download_progress(client: &Arc<Mutex<Client>>, workshop_id: u64) -> Option<(u64, u64)> {
    let client = client.lock().unwrap();
    let ugc = client.ugc();
    ugc.item_download_info(PublishedFileId(workshop_id))
        .map(|info| (info.0, info.1))
}

pub fn get_install_path(client: &Arc<Mutex<Client>>, workshop_id: u64) -> Result<Option<String>> {
    let client = client.lock().unwrap();
    let ugc = client.ugc();
    Ok(ugc
        .item_install_info(PublishedFileId(workshop_id))
        .map(|info| info.folder))
}

pub fn get_item_state(client: &Arc<Mutex<Client>>, workshop_id: u64) -> ItemState {
    let client = client.lock().unwrap();
    let ugc = client.ugc();
    let state = ugc.item_state(PublishedFileId(workshop_id));
    let installed = state.contains(steamworks::ItemState::INSTALLED);
    let downloading = state.contains(steamworks::ItemState::DOWNLOADING);
    let needs_update = state.contains(steamworks::ItemState::NEEDS_UPDATE);

    if downloading {
        ItemState::Downloading
    } else if needs_update {
        ItemState::NeedsUpdate
    } else if installed {
        ItemState::Installed
    } else {
        ItemState::NotInstalled
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ItemState {
    NotInstalled,
    Installed,
    Downloading,
    NeedsUpdate,
}

pub fn ensure_mods_downloaded(
    client: &Arc<Mutex<Client>>,
    workshop_ids: &[u64],
) -> Result<Vec<u64>> {
    let mut missing = Vec::new();
    for &id in workshop_ids {
        let state = get_item_state(client, id);
        match state {
            ItemState::NotInstalled | ItemState::NeedsUpdate => {
                subscribe_and_download(client, id)
                    .with_context(|| format!("Failed to download workshop item {id}"))?;
                missing.push(id);
            }
            ItemState::Downloading => {
                missing.push(id);
            }
            ItemState::Installed => {}
        }
    }
    Ok(missing)
}
