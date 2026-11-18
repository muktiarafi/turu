use crate::player::LaunchOptions;
use ruffle_core::backend::storage::MemoryStorageBackend;
use ruffle_frontend_utils::backends::storage::DiskStorageBackend;
use std::path::{Component, PathBuf};
use std::str::FromStr;

#[derive(clap::ValueEnum, Copy, Clone, PartialEq, Eq, Debug, Default)]
pub enum StorageBackend {
    #[default]
    Disk,
    Memory,
}

impl FromStr for StorageBackend {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "disk" => Ok(StorageBackend::Disk),
            "memory" => Ok(StorageBackend::Memory),
            _ => Err(()),
        }
    }
}

impl StorageBackend {
    pub fn as_str(self) -> &'static str {
        match self {
            StorageBackend::Disk => "disk",
            StorageBackend::Memory => "memory",
        }
    }

    pub fn create_backend(
        self,
        opt: &LaunchOptions,
    ) -> Box<dyn ruffle_core::backend::storage::StorageBackend> {
        match self {
            // Turu: written whole (`WholeWrites`), as tabs' clients share the folder.
            StorageBackend::Disk => Box::new(WholeWrites::new(opt.save_directory.clone())),
            StorageBackend::Memory => Box::new(MemoryStorageBackend::new()),
        }
    }
}

/// Turu: Ruffle's disk storage, with each shared object written whole: to a file of its own, then
/// put in place. Ruffle's empties the file and writes it, and tabs' clients (processes of their
/// own) share the folder: one reading while another wrote got half a file (AQW then starts from
/// nothing and saves that), and clients closing together (each writes the game's saved data)
/// could interleave theirs. A client killed mid-write left half a file too.
struct WholeWrites {
    disk: DiskStorageBackend,
    path: PathBuf,
}

impl WholeWrites {
    fn new(path: PathBuf) -> Self {
        Self {
            disk: DiskStorageBackend::new(path.clone()),
            path,
        }
    }
}

impl ruffle_core::backend::storage::StorageBackend for WholeWrites {
    fn get(&self, name: &str) -> Option<Vec<u8>> {
        self.disk.get(name)
    }

    fn put(&mut self, name: &str, value: &[u8]) -> bool {
        // As Ruffle's: within the folder only.
        let path = self.path.join(format!("{name}.sol"));
        if path.components().any(|c| c == Component::ParentDir) {
            return false;
        }
        let Some(parent) = path.parent() else {
            return false;
        };
        if let Err(e) = std::fs::create_dir_all(parent) {
            tracing::warn!("Unable to create storage dir {e}");
            return false;
        }
        let Some(file_name) = path.file_name() else {
            return false;
        };
        // Named for this process, so two clients don't share one.
        let mut temp_name = std::ffi::OsString::from(".");
        temp_name.push(file_name);
        temp_name.push(format!(".{}.tmp", std::process::id()));
        let temp = parent.join(temp_name);
        let written = std::fs::write(&temp, value).and_then(|()| std::fs::rename(&temp, &path));
        let Err(e) = written else {
            return true;
        };
        let _ = std::fs::remove_file(&temp);
        // (Windows refuses to replace a file another process has open: written as Ruffle writes.)
        tracing::info!(
            "Couldn't save {} whole ({e}): writing it in place",
            path.display()
        );
        match std::fs::write(&path, value) {
            Ok(()) => true,
            Err(e) => {
                tracing::warn!("Unable to save {}: {e}", path.display());
                false
            }
        }
    }

    fn remove_key(&mut self, name: &str) {
        self.disk.remove_key(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ruffle_core::backend::storage::StorageBackend as _;

    #[test]
    fn writes_whole_and_leaves_nothing_else() {
        let dir = std::env::temp_dir().join(format!("turu-storage-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut storage = WholeWrites::new(dir.clone());
        assert!(storage.put("localhost/AQLite_Data", b"first"));
        assert!(storage.put("localhost/AQLite_Data", b"second, longer"));
        assert_eq!(
            storage.get("localhost/AQLite_Data").as_deref(),
            Some(&b"second, longer"[..])
        );
        let files: Vec<_> = std::fs::read_dir(dir.join("localhost"))
            .expect("the folder")
            .map(|entry| entry.expect("an entry").file_name())
            .collect();
        assert_eq!(files, vec![std::ffi::OsString::from("AQLite_Data.sol")]);
        assert!(!storage.put("../outside", b"no"));
        storage.remove_key("localhost/AQLite_Data");
        assert_eq!(storage.get("localhost/AQLite_Data"), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
