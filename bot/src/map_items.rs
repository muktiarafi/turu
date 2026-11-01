//! Grabber > GetMap Item IDs (VibeSkua `Map.FindMapItems`): the current map's SWF, downloaded
//! from the game, scanned for map items by a [`Scanner`], with the results kept per map file.
//!
//! VibeSkua decompiles the SWF with FFDec and searches the ActionScript text. Turu reads the
//! AS3 bytecode instead; that reader is in the app (`app/src/map_items.rs`), because this crate
//! doesn't depend on Ruffle's crates, and is passed in as the [`Scanner`].

use crate::Bot;
use crate::error::{BotError, BotResult};
use crate::models::MapItem;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::Duration;

/// Where the game's map files are (VibeSkua `DownloadMapSWF`).
pub const MAPS_URL: &str = "https://game.aq.com/game/gamefiles/maps/";

/// Finds the map items in a map SWF: each item's ID and its quest (0 when none).
pub type Scanner = dyn Fn(&[u8]) -> Result<Vec<(i64, i64)>, String> + Send + Sync;

/// The saved results, by map file path.
type Saved = HashMap<String, Vec<MapItem>>;

fn read_saved(path: &Path) -> Saved {
    fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn save(path: &Path, saved: &Saved) {
    let written = serde_json::to_string_pretty(saved)
        .map_err(std::io::Error::other)
        .and_then(|text| {
            if let Some(dir) = path.parent() {
                fs::create_dir_all(dir)?;
            }
            fs::write(path, text)
        });
    if let Err(e) = written {
        tracing::warn!("Couldn't save {}: {e}", path.display());
    }
}

/// Downloads `url` (the game's map files need no login).
fn download(url: &str) -> Result<Vec<u8>, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(60))
        .user_agent(concat!("Turu/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| e.to_string())?;
    client
        .get(url)
        .send()
        .and_then(|r| r.error_for_status())
        .and_then(|r| r.bytes())
        .map(|b| b.to_vec())
        .map_err(|e| format!("{url}: {e}"))
}

/// The current map's items (VibeSkua `FindMapItems`): the saved result for its file, unless
/// `refresh`; otherwise its SWF downloaded from [`MAPS_URL`] and scanned. A result with items is
/// saved (map files are versioned by name, so it stays right).
pub fn find(bot: &Bot, scan: &Scanner, refresh: bool) -> BotResult<Vec<MapItem>> {
    find_at(bot, MAPS_URL, scan, refresh)
}

/// The current map's file path (`moveToArea`'s `strMapFileName`).
pub fn current_file(bot: &Bot) -> BotResult<String> {
    bot.state
        .lock()
        .map_file_path
        .clone()
        .filter(|f| !f.trim().is_empty())
        .ok_or_else(|| BotError::Other("Join a map first: its file isn't known yet.".into()))
}

/// Downloads a map file (a [`current_file`] path) from [`MAPS_URL`].
pub fn download_map(file: &str) -> BotResult<Vec<u8>> {
    download_at(MAPS_URL, file)
}

fn download_at(base_url: &str, file: &str) -> BotResult<Vec<u8>> {
    let url = format!("{}{}", base_url, file.trim_start_matches('/'));
    download(&url).map_err(BotError::Other)
}

/// [`find`] with the map files at `base_url` (tests use a local server).
pub fn find_at(
    bot: &Bot,
    base_url: &str,
    scan: &Scanner,
    refresh: bool,
) -> BotResult<Vec<MapItem>> {
    let file = current_file(bot)?;
    let name = {
        let state = bot.state.lock();
        let name = state.map_name.clone().or_else(|| state.last_map.clone());
        name.unwrap_or_default()
    };
    let saved_path = bot.paths.map_items_file();
    let mut saved = read_saved(&saved_path);
    if !refresh && let Some(items) = saved.get(&file) {
        return Ok(items.clone());
    }
    let swf = download_at(base_url, &file)?;
    let found = scan(&swf).map_err(|e| BotError::Other(format!("Couldn't read {file}: {e}")))?;
    let mut items: Vec<MapItem> = found
        .into_iter()
        .map(|(id, quest_id)| MapItem {
            id,
            quest_id,
            map_file_path: file.clone(),
            map_name: name.clone(),
        })
        .collect();
    items.sort_by_key(|i| i.id);
    if !items.is_empty() {
        saved.insert(file, items.clone());
        save(&saved_path, &saved);
    }
    Ok(items)
}
