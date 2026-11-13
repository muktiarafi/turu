//! The SWFs [`patch_swf_with`](crate::patch_swf_with) rewrote, kept on disk so the next start loads them
//! without patching again (the game's client takes 0.2-0.4 s on AIR's thread on an emulator, more
//! on a phone).
//!
//! A file is found by what went in: the input's hash and length, which caller it was for (the
//! client, another SWF) and this crate's rules (a hash of `lib.rs`, so a Turu build
//! that patches differently never loads what an older one wrote). A game update is a new input,
//! so a new file. The folder keeps the [`KEEP`] files used last.

use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

/// How many patched SWFs the folder keeps: the client of a few game versions and the SWFs it
/// loads.
pub const KEEP: usize = 16;

/// The patching rules: a hash of the source that holds them.
const RULES: u64 = fnv64(include_bytes!("lib.rs"));

/// FNV-1a, 64 bits: stable across builds and Rust versions (unlike `DefaultHasher`).
const fn fnv64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    let mut i = 0;
    while i < bytes.len() {
        hash = (hash ^ bytes[i] as u64).wrapping_mul(0x0000_0100_0000_01b3);
        i += 1;
    }
    hash
}

/// Patched SWFs in a folder of their own.
pub struct PatchCache {
    dir: PathBuf,
    /// Makes temporary file names unique within the process.
    serial: AtomicU64,
}

impl PatchCache {
    /// A cache in `dir`, made when the first SWF is kept.
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self {
            dir: dir.into(),
            serial: AtomicU64::new(0),
        }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn path(&self, input: &[u8], kind: &str) -> PathBuf {
        let kind: String = kind.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
        self.dir.join(format!(
            "{kind}-{:016x}-{}-{RULES:016x}.swf",
            fnv64(input),
            input.len()
        ))
    }

    /// What `input` became for `kind` before, if it was kept.
    pub fn get(&self, input: &[u8], kind: &str) -> Option<Vec<u8>> {
        let path = self.path(input, kind);
        let output = fs::read(&path).ok()?;
        // Anything but a whole SWF (a damaged file) is as good as missing.
        let signature = output.get(..3)?;
        let length = u32::from_le_bytes(output.get(4..8)?.try_into().ok()?);
        let whole = matches!(signature, b"FWS" | b"CWS" | b"ZWS")
            && (signature != b"FWS" || output.len() == length as usize);
        if !whole {
            let _ = fs::remove_file(&path);
            return None;
        }
        // The modification time is "last used", which trimming goes by.
        let _ = File::options()
            .write(true)
            .open(&path)
            .and_then(|file| file.set_modified(SystemTime::now()));
        Some(output)
    }

    /// Keeps `output`, what `input` became for `kind`, then drops the files used longest ago
    /// beyond [`KEEP`]. Written to a temporary file and renamed, so a reader never sees half of
    /// one.
    pub fn put(&self, input: &[u8], kind: &str, output: &[u8]) -> io::Result<()> {
        fs::create_dir_all(&self.dir)?;
        let path = self.path(input, kind);
        let temporary = self.dir.join(format!(
            "{}.{}-{}.tmp",
            path.file_stem().and_then(|s| s.to_str()).unwrap_or("swf"),
            std::process::id(),
            self.serial.fetch_add(1, Ordering::Relaxed)
        ));
        let written = fs::write(&temporary, output).and_then(|()| fs::rename(&temporary, &path));
        if written.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        written?;
        self.trim();
        Ok(())
    }

    /// Removes all but the [`KEEP`] files used last, and temporary files left behind.
    fn trim(&self) {
        let Ok(entries) = fs::read_dir(&self.dir) else {
            return;
        };
        let mut kept = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            let modified = entry
                .metadata()
                .and_then(|metadata| metadata.modified())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            match path.extension().and_then(|extension| extension.to_str()) {
                Some("swf") => kept.push((modified, path)),
                Some("tmp") => {
                    let abandoned = modified.elapsed().is_ok_and(|age| age.as_secs() > 60 * 60);
                    if abandoned {
                        let _ = fs::remove_file(path);
                    }
                }
                _ => {}
            }
        }
        if kept.len() <= KEEP {
            return;
        }
        kept.sort();
        let extra = kept.len() - KEEP;
        for (_, path) in kept.into_iter().take(extra) {
            let _ = fs::remove_file(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir()
                .join(format!("turu-patch-cache-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            Self(dir)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// A compressed SWF's first bytes: enough for [`PatchCache::get`]'s check.
    fn swf(body: &[u8]) -> Vec<u8> {
        let mut out = b"CWS\x0a".to_vec();
        out.extend_from_slice(&1000u32.to_le_bytes());
        out.extend_from_slice(body);
        out
    }

    #[test]
    fn keeps_by_input_and_kind() {
        let dir = TempDir::new("kind");
        let cache = PatchCache::new(&dir.0);
        assert_eq!(cache.get(b"client", "game"), None);
        cache.put(b"client", "game", &swf(b"patched")).unwrap();
        assert_eq!(cache.get(b"client", "game"), Some(swf(b"patched")));
        assert_eq!(cache.get(b"client", "asset"), None);
        assert_eq!(cache.get(b"client v2", "game"), None);
    }

    #[test]
    fn damaged_files_are_misses() {
        let dir = TempDir::new("damaged");
        let cache = PatchCache::new(&dir.0);
        cache.put(b"client", "game", b"CW").unwrap();
        assert_eq!(cache.get(b"client", "game"), None);
        // An uncompressed SWF shorter than its header says.
        let mut short = b"FWS\x0a".to_vec();
        short.extend_from_slice(&100u32.to_le_bytes());
        cache.put(b"asset", "asset", &short).unwrap();
        assert_eq!(cache.get(b"asset", "asset"), None);
        assert!(!cache.path(b"asset", "asset").exists());
    }

    #[test]
    fn keeps_the_files_used_last() {
        let dir = TempDir::new("trim");
        let cache = PatchCache::new(&dir.0);
        let old = SystemTime::now() - Duration::from_secs(3600);
        for i in 0..KEEP {
            let input = format!("input {i}");
            cache.put(input.as_bytes(), "asset", &swf(b"x")).unwrap();
            let file = File::options()
                .write(true)
                .open(cache.path(input.as_bytes(), "asset"))
                .unwrap();
            file.set_modified(old + Duration::from_secs(i as u64))
                .unwrap();
        }
        // Using the oldest makes it the newest; the next put drops "input 1".
        assert!(cache.get(b"input 0", "asset").is_some());
        cache.put(b"new", "asset", &swf(b"x")).unwrap();
        assert!(cache.get(b"input 0", "asset").is_some());
        assert_eq!(cache.get(b"input 1", "asset"), None);
        assert!(cache.get(b"input 2", "asset").is_some());
        assert!(cache.get(b"new", "asset").is_some());
    }

    #[test]
    fn the_rules_hash_follows_the_source() {
        assert_eq!(RULES, fnv64(include_bytes!("lib.rs")));
        assert_ne!(fnv64(b"a"), fnv64(b"b"));
    }
}
