//! Turu: the desktop's [`Platform`]: rfd's file dialogs (parented to the window), the browser,
//! the file manager, crash reports (`crash_uploads`), Notify Drop's beep (`beep`).

use crate::gui::FilePicker;
use crate::platform::{Answer, FileDialog, Platform};
use rfd::AsyncFileDialog;
use std::path::Path;

pub struct DesktopPlatform {
    picker: FilePicker,
}

impl DesktopPlatform {
    pub fn new(picker: FilePicker) -> Self {
        Self { picker }
    }
}

fn rfd_dialog(dialog: FileDialog) -> AsyncFileDialog {
    let mut rfd = AsyncFileDialog::new();
    for (name, extensions) in &dialog.filters {
        rfd = rfd.add_filter(name, extensions);
    }
    if let Some(title) = dialog.title {
        rfd = rfd.set_title(title);
    }
    if let Some(directory) = dialog.directory {
        rfd = rfd.set_directory(directory);
    }
    if let Some(file_name) = dialog.file_name {
        rfd = rfd.set_file_name(file_name);
    }
    rfd
}

impl Platform for DesktopPlatform {
    fn pick_file(&self, dialog: FileDialog, answer: Answer) -> bool {
        // The dialog is created here, on the UI thread (macOS needs that); only waiting for it
        // happens on the runtime.
        let Some(future) = self.picker.show_dialog(rfd_dialog(dialog), |d| d.pick_file()) else {
            return false;
        };
        tokio::spawn(async move { answer(future.await.map(|h| h.path().to_path_buf())) });
        true
    }

    fn save_file(&self, dialog: FileDialog, answer: Answer) -> bool {
        let Some(future) = self.picker.show_dialog(rfd_dialog(dialog), |d| d.save_file()) else {
            return false;
        };
        tokio::spawn(async move { answer(future.await.map(|h| h.path().to_path_buf())) });
        true
    }

    fn open_url(&self, url: &str) {
        if let Err(e) = webbrowser::open(url) {
            tracing::warn!("Couldn't open {url}: {e}");
        }
    }

    fn open_folder(&self, folder: &Path) {
        crate::mugetsu::open_folder(folder);
    }

    fn open_file(&self, path: &Path) {
        #[cfg(target_os = "macos")]
        let result = std::process::Command::new("open").arg(path).spawn();
        #[cfg(target_os = "windows")]
        let result = std::process::Command::new("cmd")
            .args(["/C", "start", ""])
            .arg(path)
            .spawn();
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        let result = std::process::Command::new("xdg-open").arg(path).spawn();
        if let Err(e) = result {
            tracing::warn!("Couldn't open {}: {e}", path.display());
        }
    }
    fn uploads_crash_reports(&self) -> bool {
        crate::crash_uploads::available()
    }

    fn set_crash_uploads(&self, on: bool) {
        crate::crash_uploads::set_enabled(on);
    }

    fn beep(&self) {
        crate::beep::beep();
    }
}
