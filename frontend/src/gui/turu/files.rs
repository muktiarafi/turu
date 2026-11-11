//! File dialogs (never blocking the UI thread) and opening files with other programs.

use crate::platform::{FileDialog, platform};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};

/// A file dialog that is open (or about to answer) for one of Turu's windows.
#[derive(Default)]
pub struct Pending(Option<Receiver<Option<PathBuf>>>);

impl Pending {
    pub fn is_open(&self) -> bool {
        self.0.is_some()
    }

    /// The chosen path once the dialog was answered (`None` while open or when cancelled).
    pub fn poll(&mut self) -> Option<PathBuf> {
        let receiver = self.0.as_ref()?;
        match receiver.try_recv() {
            Ok(path) => {
                self.0 = None;
                path
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => None,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.0 = None;
                None
            }
        }
    }

    /// Opens an "open file" dialog.
    pub fn pick_file(&mut self, ctx: &egui::Context, dialog: FileDialog) {
        self.open(ctx, |answer| platform().pick_file(dialog, answer));
    }

    /// Opens a "save file" dialog.
    pub fn save_file(&mut self, ctx: &egui::Context, dialog: FileDialog) {
        self.open(ctx, |answer| platform().save_file(dialog, answer));
    }

    /// Waits for an answer `show` hands on (a dialog, or a window that picks: Search Scripts'
    /// pick mode); `show` says whether it was shown.
    pub fn open(&mut self, ctx: &egui::Context, show: impl FnOnce(crate::platform::Answer) -> bool) {
        if self.is_open() {
            return;
        }
        let (sender, receiver) = channel();
        let ctx = ctx.clone();
        let shown = show(Box::new(move |path| {
            let _ = sender.send(path);
            ctx.request_repaint();
        }));
        if shown {
            self.0 = Some(receiver);
        }
    }
}

/// A save dialog for `name` that, once answered, writes the text. Poll it each frame.
#[derive(Default)]
pub struct SaveText {
    pending: Pending,
    text: String,
}

impl SaveText {
    pub fn ask(&mut self, ctx: &egui::Context, file_name: &str, text: String) {
        if self.pending.is_open() {
            return;
        }
        self.text = text;
        self.pending.save_file(ctx, FileDialog::new().set_file_name(file_name));
    }

    pub fn poll(&mut self) {
        if let Some(path) = self.pending.poll()
            && let Err(e) = std::fs::write(&path, std::mem::take(&mut self.text))
        {
            tracing::error!("Couldn't save {}: {e}", path.display());
        }
    }
}

/// Opens `path` with the program the OS associates with it.
pub fn open_with_default(path: &Path) {
    platform().open_file(path);
}

/// The VS Code launcher, when there is one: the `code` command on `PATH`, or the macOS app.
pub fn find_vscode() -> Option<PathBuf> {
    let names: &[&str] = if cfg!(windows) {
        &["code.cmd", "code.exe"]
    } else {
        &["code"]
    };
    if let Some(paths) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&paths) {
            for name in names {
                let candidate = dir.join(name);
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        let bundled = Path::new("/Applications/Visual Studio Code.app/Contents/Resources/app/bin/code");
        if bundled.is_file() {
            return Some(bundled.to_path_buf());
        }
    }
    None
}

/// Opens `path` in VS Code; why it couldn't start, if it couldn't.
pub fn open_in_vscode(code: &Path, path: &Path) -> std::io::Result<()> {
    #[cfg(windows)]
    let result = std::process::Command::new("cmd").arg("/C").arg(code).arg(path).spawn();
    #[cfg(not(windows))]
    let result = std::process::Command::new(code).arg(path).spawn();
    if let Err(e) = &result {
        tracing::warn!("Couldn't start VS Code: {e}");
    }
    result.map(drop)
}

/// The folder Load Script starts in: the repo's `scripts/` (next to the working directory, or
/// above the executable when running from `target/`), else the bot's scripts folder.
pub fn scripts_start_dir(fallback: PathBuf) -> PathBuf {
    if let Ok(cwd) = std::env::current_dir() {
        let dir = cwd.join("scripts");
        if dir.is_dir() {
            return dir;
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        for dir in exe.ancestors().skip(1).take(5) {
            let dir = dir.join("scripts");
            if dir.join("types").is_dir() || dir.join("core").is_dir() {
                return dir;
            }
        }
    }
    fallback
}
