//! Turu: what the bot windows ask of the operating system (file dialogs, the browser, the file
//! manager), so the same windows run where those work differently (Android's system picker
//! answers through an `Intent`). The app installs its [`Platform`] once, with [`set`]; until then
//! (and without a window) nothing opens.

use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

/// What a file dialog asks: rfd's `AsyncFileDialog`, without the parent window.
#[derive(Debug, Clone, Default)]
pub struct FileDialog {
    pub title: Option<String>,
    pub directory: Option<PathBuf>,
    pub file_name: Option<String>,
    /// Name and extensions (without the dot; `*` for any).
    pub filters: Vec<(String, Vec<String>)>,
}

impl FileDialog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_filter(mut self, name: impl Into<String>, extensions: &[&str]) -> Self {
        self.filters
            .push((name.into(), extensions.iter().map(|e| (*e).to_owned()).collect()));
        self
    }

    pub fn set_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn set_directory(mut self, directory: impl AsRef<Path>) -> Self {
        self.directory = Some(directory.as_ref().to_owned());
        self
    }

    pub fn set_file_name(mut self, file_name: impl Into<String>) -> Self {
        self.file_name = Some(file_name.into());
        self
    }
}

/// Gets a file dialog's answer: the chosen path, `None` when cancelled. Called on any thread.
pub type Answer = Box<dyn FnOnce(Option<PathBuf>) + Send>;

pub trait Platform: Send + Sync {
    /// Shows an "open file" dialog; false when it can't (one is open already). Called on the UI
    /// thread, and never waits for the answer.
    fn pick_file(&self, dialog: FileDialog, answer: Answer) -> bool;

    /// Shows a "save file" dialog, like [`Platform::pick_file`].
    fn save_file(&self, dialog: FileDialog, answer: Answer) -> bool;

    /// Opens a web page in the browser.
    fn open_url(&self, url: &str);

    /// Shows a folder in the file manager.
    fn open_folder(&self, folder: &Path);

    /// Opens a file with the program the system associates with it.
    fn open_file(&self, path: &Path);
}

static PLATFORM: OnceLock<Arc<dyn Platform>> = OnceLock::new();

/// Installs the app's platform; false when one is already installed.
pub fn set(platform: Arc<dyn Platform>) -> bool {
    PLATFORM.set(platform).is_ok()
}

/// The installed platform, or one that opens nothing.
pub fn platform() -> &'static dyn Platform {
    PLATFORM.get().map_or(&NoPlatform, |p| p.as_ref())
}

/// Without a window: dialogs don't open, and what would open is logged.
struct NoPlatform;

impl Platform for NoPlatform {
    fn pick_file(&self, _dialog: FileDialog, _answer: Answer) -> bool {
        false
    }

    fn save_file(&self, _dialog: FileDialog, _answer: Answer) -> bool {
        false
    }

    fn open_url(&self, url: &str) {
        tracing::info!("Not opened without a window: {url}");
    }

    fn open_folder(&self, folder: &Path) {
        tracing::info!("Not opened without a window: {}", folder.display());
    }

    fn open_file(&self, path: &Path) {
        tracing::info!("Not opened without a window: {}", path.display());
    }
}
