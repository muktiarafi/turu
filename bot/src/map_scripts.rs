//! The scripts made for a map: on joining one, the side bar's script chip lists the scripts that
//! farm or hunt there, so they needn't be searched for by hand. A script's maps are the ones its
//! own source names in a call that takes a map (`huntMonster("nulgath", ...)`,
//! `killMonster(...)`, `map.join(...)`, [`maps_in`]); the scripts repository's index lists them
//! the same way (`tools/build-index.mjs`, `ScriptInfo::maps`) for scripts not downloaded yet.
//! A script that takes its map from a variable or its options isn't found.
//!
//! The scripts folder is read once, in the background ([`scripts_for`] starts it), and again after
//! [`refresh`] (Search Scripts downloaded or deleted some).

use crate::repo;
use parking_lot::Mutex;
use regex::Regex;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};

pub use repo::Entry;

/// Maps scripts only pass through or park in (and the game's start): they would list half the
/// scripts.
const PASSING: [&str; 4] = ["whitemap", "battleon", "yulgar", "house"];

/// Larger files aren't scripts anyone wrote by hand (a generated table, a bundle).
const MOST_BYTES: u64 = 2 * 1024 * 1024;

/// A call whose first argument is a map, given as a string: the bot API's and the C# ports'
/// names. `join("...")` also matches `Array.join`, whose separators don't start with a letter.
static MAP_CALL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"\b(?:huntMonster|killMonster|HuntMonster|KillMonster|huntMonsterMapID|killMonsterMapID|join|Join|joinMap|JoinMap|joinSWF|goTo|ensureMap)\(\s*["'`]([A-Za-z][A-Za-z0-9_]*)(?:-[0-9]+)?["'`]"#,
    )
    .expect("valid regex")
});

/// The maps `source` names, lowercase, with how often (a room number like `-100000` dropped).
pub fn maps_in(source: &str) -> BTreeMap<String, u32> {
    let mut maps = BTreeMap::new();
    for capture in MAP_CALL.captures_iter(source) {
        let map = capture[1].to_ascii_lowercase();
        if !PASSING.contains(&map.as_str()) {
            *maps.entry(map).or_insert(0) += 1;
        }
    }
    maps
}

/// A script made for a map.
#[derive(Debug, Clone, PartialEq)]
pub struct MapScript {
    /// Relative to the scripts folder, with `/` (the index's `path`).
    pub path: String,
    /// Its header's name, else the file name without extension.
    pub name: String,
    /// How often it names the map: the scripts that name it most come first.
    pub mentions: u32,
    /// In the scripts folder (else only in the repository's index: downloaded on start).
    pub downloaded: bool,
}

#[derive(Default)]
struct Scan {
    /// Map to scripts, from the scripts folder.
    by_map: HashMap<String, Vec<MapScript>>,
    /// Being read (the lists are the last scan's until it's done).
    busy: bool,
    /// Read at least once since the last [`refresh`].
    done: bool,
}

static SCAN: LazyLock<Arc<Mutex<Scan>>> = LazyLock::new(Default::default);

/// The scripts for `map` (any case): the scripts folder's, then the Search Scripts `entries` not
/// downloaded yet whose index entry names it; downloaded first, then the ones naming the map
/// most. `None` while the folder is first read, which this starts.
pub fn scripts_for(
    scripts_dir: &Path,
    map: &str,
    entries: &[repo::Entry],
) -> Option<Vec<MapScript>> {
    let map = map.trim().to_ascii_lowercase();
    let mut scan = SCAN.lock();
    if !scan.done {
        if !scan.busy {
            scan.busy = true;
            let dir = scripts_dir.to_path_buf();
            let spawned = std::thread::Builder::new()
                .name("Turu Map Scripts".into())
                .spawn(move || {
                    let by_map = read_folder(&dir);
                    let mut scan = SCAN.lock();
                    scan.by_map = by_map;
                    scan.busy = false;
                    scan.done = true;
                });
            if let Err(e) = spawned {
                tracing::warn!("Couldn't read the scripts' maps: {e}");
                scan.busy = false;
                scan.done = true;
            }
        }
        return None;
    }
    let mut list = scan.by_map.get(&map).cloned().unwrap_or_default();
    let have: BTreeSet<String> = list.iter().map(|s| s.path.clone()).collect();
    for entry in entries {
        let info = &entry.info;
        if info.library || entry.status.is_downloaded() || have.contains(&info.path) {
            continue;
        }
        if info.maps.iter().any(|m| m.eq_ignore_ascii_case(&map)) {
            let name = if info.name.is_empty() {
                stem(&info.path)
            } else {
                info.name.clone()
            };
            list.push(MapScript {
                path: info.path.clone(),
                name,
                mentions: 1,
                downloaded: false,
            });
        }
    }
    list.sort_by(|a, b| {
        b.downloaded
            .cmp(&a.downloaded)
            .then(b.mentions.cmp(&a.mentions))
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Some(list)
}

/// The scripts folder changed: it's read again on the next [`scripts_for`].
pub fn refresh() {
    SCAN.lock().done = false;
}

/// Every runnable script under `dir` by the maps it names.
fn read_folder(dir: &Path) -> HashMap<String, Vec<MapScript>> {
    let started = std::time::Instant::now();
    let mut by_map: HashMap<String, Vec<MapScript>> = HashMap::new();
    let mut files = Vec::new();
    list_scripts(dir, &mut files);
    for file in &files {
        let Ok(meta) = std::fs::metadata(file) else {
            continue;
        };
        if meta.len() > MOST_BYTES {
            continue;
        }
        let Ok(source) = std::fs::read_to_string(file) else {
            continue;
        };
        if repo::is_library(&source) {
            continue;
        }
        let maps = maps_in(&source);
        if maps.is_empty() {
            continue;
        }
        let path = file
            .strip_prefix(dir)
            .unwrap_or(file)
            .to_string_lossy()
            .replace('\\', "/");
        let header = repo::parse_header(&source);
        let name = if header.name.is_empty() {
            stem(&path)
        } else {
            header.name
        };
        for (map, mentions) in maps {
            by_map.entry(map).or_default().push(MapScript {
                path: path.clone(),
                name: name.clone(),
                mentions,
                downloaded: true,
            });
        }
    }
    tracing::info!(
        "Read {} scripts' maps ({} maps) in {} ms",
        files.len(),
        by_map.len(),
        started.elapsed().as_millis()
    );
    by_map
}

/// The `.ts` and `.js` files under `dir`, leaving out hidden folders, `node_modules` and the API
/// types (as the index does).
fn list_scripts(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') || name == "node_modules" || name == "types" {
            continue;
        }
        let path = entry.path();
        match entry.file_type() {
            Ok(kind) if kind.is_dir() => list_scripts(&path, out),
            Ok(_) if name.ends_with(".ts") || name.ends_with(".js") => out.push(path),
            _ => {}
        }
    }
}

/// The file name without folder and extension.
fn stem(path: &str) -> String {
    let file = path.rsplit('/').next().unwrap_or(path);
    file.rsplit_once('.')
        .map_or(file, |(stem, _)| stem)
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_maps_a_script_names() {
        let source = r#"
            bot.map.join("tercessuinotlim-100000");
            core.huntMonster("Tercessuinotlim", "Dark Makai", "Defeated Makai");
            core.killMonster('evilwarnul', "r2", "Left", "*");
            core.join("whitemap");
            const s = parts.join(", ");
        "#;
        let maps = maps_in(source);
        assert_eq!(maps.get("tercessuinotlim"), Some(&2));
        assert_eq!(maps.get("evilwarnul"), Some(&1));
        assert!(!maps.contains_key("whitemap"));
        assert_eq!(maps.len(), 2);
    }

    #[test]
    fn reads_a_folder_and_skips_libraries() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("farm")).unwrap();
        std::fs::write(
            dir.path().join("farm/Nulgath.ts"),
            "/*\nname: Nulgath Farm\n*/\nexport default function main() { core.huntMonster(\"nulgath\", \"x\"); }",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("CoreFarms.ts"),
            "export function f() { core.huntMonster(\"nulgath\", \"x\"); }",
        )
        .unwrap();
        let by_map = read_folder(dir.path());
        let list = &by_map["nulgath"];
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].path, "farm/Nulgath.ts");
        assert_eq!(list[0].name, "Nulgath Farm");
    }
}
