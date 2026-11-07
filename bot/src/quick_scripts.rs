//! Favourite and recently started scripts: the rail's script chip (Android's side bar) lists them
//! to start one with a tap, without the Script Loader. Kept in `quick-scripts.json` in the data
//! folder ([`crate::paths::Paths::quick_scripts_file`]), shared by every tab: a cached copy per
//! file, so the UI can read it every frame and two tabs never write over each other.

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// How many recently started scripts are kept.
pub const MOST_RECENT: usize = 6;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct QuickScripts {
    /// Starred in the Script Loader, in the order they were starred.
    pub favorites: Vec<PathBuf>,
    /// Started lately, the newest first, at most [`MOST_RECENT`].
    pub recent: Vec<PathBuf>,
}

impl QuickScripts {
    /// The saved lists; empty when there is no file or it can't be read.
    pub fn load(path: &Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_else(|e| {
                tracing::warn!("Couldn't read {}: {e}", path.display());
                Self::default()
            }),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let text = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        std::fs::write(path, text)
    }

    pub fn is_favorite(&self, script: &Path) -> bool {
        self.favorites.iter().any(|p| p == script)
    }

    /// Stars `script`, or takes its star away.
    pub fn toggle_favorite(&mut self, script: &Path) {
        if self.is_favorite(script) {
            self.favorites.retain(|p| p != script);
        } else {
            self.favorites.push(script.to_path_buf());
        }
    }

    /// `script` was started: first in the recent list, which keeps [`MOST_RECENT`].
    pub fn note_started(&mut self, script: &Path) {
        self.recent.retain(|p| p != script);
        self.recent.insert(0, script.to_path_buf());
        self.recent.truncate(MOST_RECENT);
    }

    /// The recent scripts that aren't favourites too (those show once, as favourites).
    pub fn recent_only(&self) -> impl Iterator<Item = &PathBuf> {
        self.recent.iter().filter(|p| !self.is_favorite(p))
    }

    pub fn is_empty(&self) -> bool {
        self.favorites.is_empty() && self.recent.is_empty()
    }
}

/// The lists last read or written, by file.
static CACHE: Mutex<Option<HashMap<PathBuf, QuickScripts>>> = Mutex::new(None);

/// The lists in `file`: read once, then from memory.
pub fn get(file: &Path) -> QuickScripts {
    let mut cache = CACHE.lock();
    cache
        .get_or_insert_with(HashMap::new)
        .entry(file.to_path_buf())
        .or_insert_with(|| QuickScripts::load(file))
        .clone()
}

/// Changes the lists in `file` and saves them.
pub fn update(file: &Path, change: impl FnOnce(&mut QuickScripts)) {
    let mut cache = CACHE.lock();
    let lists = cache
        .get_or_insert_with(HashMap::new)
        .entry(file.to_path_buf())
        .or_insert_with(|| QuickScripts::load(file));
    let before = lists.clone();
    change(lists);
    if *lists != before
        && let Err(e) = lists.save(file)
    {
        tracing::warn!("Couldn't save {}: {e}", file.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recent_keeps_the_newest_once() {
        let mut lists = QuickScripts::default();
        for i in 0..MOST_RECENT + 2 {
            lists.note_started(Path::new(&format!("s{i}.ts")));
        }
        lists.note_started(Path::new("s3.ts"));
        assert_eq!(lists.recent.len(), MOST_RECENT);
        assert_eq!(lists.recent[0], Path::new("s3.ts"));
        assert_eq!(
            lists
                .recent
                .iter()
                .filter(|p| *p == Path::new("s3.ts"))
                .count(),
            1
        );
    }

    #[test]
    fn favorites_toggle_and_leave_the_recent_list() {
        let mut lists = QuickScripts::default();
        lists.note_started(Path::new("a.ts"));
        lists.note_started(Path::new("b.ts"));
        lists.toggle_favorite(Path::new("a.ts"));
        assert!(lists.is_favorite(Path::new("a.ts")));
        assert_eq!(lists.recent_only().collect::<Vec<_>>(), [Path::new("b.ts")]);
        lists.toggle_favorite(Path::new("a.ts"));
        assert!(!lists.is_favorite(Path::new("a.ts")));
    }

    #[test]
    fn update_saves_and_get_reads_it_back() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("quick-scripts.json");
        update(&file, |lists| lists.toggle_favorite(Path::new("x.ts")));
        assert!(QuickScripts::load(&file).is_favorite(Path::new("x.ts")));
        assert!(get(&file).is_favorite(Path::new("x.ts")));
    }
}
