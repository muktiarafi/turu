//! The recent "Send to player" names (VibeSkua ButlerV5 `PlayerTargetHistory.cs`): a JSON list
//! in `butler_player_targets.json`, newest first, shared by every client.

use parking_lot::Mutex;
use std::path::Path;

/// How many names are kept (VibeSkua `MaxEntries`).
pub const MAX_ENTRIES: usize = 20;

/// One writer at a time in this process.
static LOCK: Mutex<()> = Mutex::new(());

/// The names, newest first: trimmed, no blanks, no repeats (ignoring case), at most
/// [`MAX_ENTRIES`]. Empty when the file is missing or unreadable.
pub fn load(path: &Path) -> Vec<String> {
    let _guard = LOCK.lock();
    load_unlocked(path)
}

/// Puts `target` first (dropping an earlier copy) and saves; the new list. A blank name
/// changes nothing.
pub fn add(path: &Path, target: &str) -> Vec<String> {
    let target = target.trim();
    let _guard = LOCK.lock();
    let mut history = load_unlocked(path);
    if target.is_empty() {
        return history;
    }
    history.retain(|h| !h.eq_ignore_ascii_case(target));
    history.insert(0, target.to_owned());
    history.truncate(MAX_ENTRIES);
    let saved = path
        .parent()
        .map_or(Ok(()), std::fs::create_dir_all)
        .and_then(|()| {
            let text = serde_json::to_string(&history).map_err(std::io::Error::other)?;
            std::fs::write(path, text)
        });
    if let Err(e) = saved {
        tracing::debug!("Couldn't save the Butler target history: {e}");
    }
    history
}

fn load_unlocked(path: &Path) -> Vec<String> {
    let Some(names) = std::fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str::<Vec<String>>(&t).ok())
    else {
        return Vec::new();
    };
    let mut out: Vec<String> = Vec::new();
    for name in names {
        let name = name.trim();
        if !name.is_empty() && !out.iter().any(|o| o.eq_ignore_ascii_case(name)) {
            out.push(name.to_owned());
        }
    }
    out.truncate(MAX_ENTRIES);
    out
}
