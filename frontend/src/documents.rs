//! File dialogs through Android's system picker (the Storage Access Framework), the part both
//! Android apps share: the bot windows' [`Platform::pick_file`](crate::platform::Platform::pick_file)
//! and `save_file` answer with a path as on a desktop, so their callers (Load Script, Junk Items'
//! and Loadouts' import and export, the Scheduler, the Interceptor's export, the logs' Save, ...)
//! work as they are. Each app shows the picker its own way (Turu: `android/src/saf.rs`; Turu Mas:
//! `air/ane/rust/src/saf.rs`) and hands the answer here.
//!
//! * Open ([`open_request`]): the picker shows the dialog's filters as MIME types; the chosen
//!   document is copied into the app's cache, or for a script into the scripts folder's
//!   `Imported/` (its `../core/...` imports then find the scripts folder's), and that copy's path
//!   is the answer ([`picked`]).
//! * Save ([`save_request`]): the picker makes a document with the dialog's file name; the answer
//!   is a file in the app's cache, and once the window has written it (its size and time stop
//!   changing) the app copies it into the document ([`created`]).
//!
//! One dialog at a time, like a desktop's modal one.

use crate::platform::{Answer, FileDialog};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// The dialog on screen, waiting for the picker's answer.
pub enum Waiting {
    Open(Answer),
    Save(Answer),
}

static WAITING: Mutex<Option<Waiting>> = Mutex::new(None);

fn waiting() -> std::sync::MutexGuard<'static, Option<Waiting>> {
    WAITING.lock().unwrap_or_else(|e| e.into_inner())
}

/// Extensions of the files Turu runs as scripts: picked ones go into the scripts folder.
const SCRIPT_EXTENSIONS: &[&str] = &["ts", "js"];

/// How long a save waits for the window to write its file.
const SAVE_TIMEOUT: Duration = Duration::from_secs(600);

/// Shows a picker for `dialog` with `show` (false when it couldn't); false too while another
/// dialog waits for its answer.
pub fn begin(dialog: Waiting, show: impl FnOnce() -> bool) -> bool {
    {
        let mut waiting = waiting();
        if waiting.is_some() {
            return false;
        }
        // Before showing: the picker may answer (a failure) before `show` returns.
        *waiting = Some(dialog);
    }
    if show() {
        true
    } else {
        waiting().take();
        false
    }
}

/// What the "open" picker is asked for: its MIME types, one a line, and the folder a picked
/// document is copied into (empty: the app's cache).
pub fn open_request(dialog: &FileDialog) -> (String, String) {
    let mime_types = open_mime_types(&dialog.filters).join("\n");
    let folder = if is_script_dialog(&dialog.filters) {
        crate::turu::bot()
            .paths
            .scripts_dir()
            .join("Imported")
            .to_string_lossy()
            .into_owned()
    } else {
        String::new()
    };
    (mime_types, folder)
}

/// What the "save" picker is asked for: the document's MIME type and suggested name.
pub fn save_request(dialog: &FileDialog) -> (String, String) {
    let file_name = save_name(dialog);
    let mime_type = extension(&file_name)
        .and_then(mime_type)
        .unwrap_or("application/octet-stream");
    (mime_type.to_owned(), file_name)
}

/// The "open" picker's document, copied to `path`; `None` when cancelled or the copy failed.
pub fn picked(path: Option<PathBuf>) {
    match waiting().take() {
        Some(Waiting::Open(answer)) => {
            if let Some(path) = &path {
                tracing::info!("Picked {}", path.display());
            }
            answer(path);
        }
        Some(Waiting::Save(answer)) => answer(None),
        None => {}
    }
}

/// The "save" picker made the document `uri`; the window writes `path`, and `write` copies it
/// into the document afterwards (on a thread of its own: `Ok(())`, or why not). Both `None` when
/// cancelled.
pub fn created(
    uri: Option<String>,
    path: Option<PathBuf>,
    write: impl FnOnce(&str, &Path) -> Result<(), String> + Send + 'static,
) {
    let answer = match waiting().take() {
        Some(Waiting::Save(answer) | Waiting::Open(answer)) => answer,
        None => return,
    };
    match (uri, path) {
        (Some(uri), Some(path)) => {
            // Not there yet: the window's write is what's waited for.
            let _ = std::fs::remove_file(&path);
            answer(Some(path.clone()));
            write_when_saved(uri, path, write);
        }
        _ => answer(None),
    }
}

/// Copies `path` into the document `uri` with `write` once the window has written it, on a
/// thread of its own.
fn write_when_saved(
    uri: String,
    path: PathBuf,
    write: impl FnOnce(&str, &Path) -> Result<(), String> + Send + 'static,
) {
    let spawned = std::thread::Builder::new().name("Turu Save".into()).spawn(move || {
        let started = Instant::now();
        let mut last = None;
        loop {
            std::thread::sleep(Duration::from_millis(400));
            match std::fs::metadata(&path) {
                Ok(metadata) => {
                    let now = (metadata.len(), metadata.modified().ok());
                    if last == Some(now) {
                        break;
                    }
                    last = Some(now);
                }
                Err(_) if started.elapsed() > SAVE_TIMEOUT => {
                    tracing::warn!("Nothing was written to save in {uri}");
                    if let Some(folder) = path.parent() {
                        let _ = std::fs::remove_dir(folder);
                    }
                    return;
                }
                Err(_) => {}
            }
        }
        match write(&uri, &path) {
            Ok(()) => tracing::info!("Saved {}", path.file_name().unwrap_or_default().to_string_lossy()),
            Err(e) => tracing::error!("Couldn't save {}: {e}", path.display()),
        }
    });
    if let Err(e) = spawned {
        tracing::error!("Couldn't start saving the file: {e}");
    }
}

/// Whether the dialog picks a script (Load Script, Accounts', Army's).
fn is_script_dialog(filters: &[(String, Vec<String>)]) -> bool {
    filters
        .iter()
        .flat_map(|(_, extensions)| extensions)
        .any(|e| SCRIPT_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
}

/// The MIME types the "open" picker shows: the filters' (and any type a document provider
/// doesn't know, `application/octet-stream`), or everything when a filter has an extension
/// without one of its own (`.ts`, `*`).
fn open_mime_types(filters: &[(String, Vec<String>)]) -> Vec<&'static str> {
    let mut types = Vec::new();
    for extension in filters.iter().flat_map(|(_, extensions)| extensions) {
        match mime_type(extension) {
            Some(mime) if !types.contains(&mime) => types.push(mime),
            Some(_) => {}
            None => return vec!["*/*"],
        }
    }
    if types.is_empty() {
        return vec!["*/*"];
    }
    types.push("application/octet-stream");
    types
}

/// The MIME type documents with `extension` have; `None` for one with no type of its own.
fn mime_type(extension: &str) -> Option<&'static str> {
    Some(match extension.to_ascii_lowercase().as_str() {
        "json" => "application/json",
        "txt" | "log" => "text/plain",
        "csv" => "text/csv",
        "xml" => "text/xml",
        "html" | "htm" => "text/html",
        "png" => "image/png",
        _ => return None,
    })
}

fn extension(file_name: &str) -> Option<&str> {
    Path::new(file_name).extension().and_then(|e| e.to_str())
}

/// The name the "save" picker suggests: the dialog's, else "untitled" with its first filter's
/// extension.
fn save_name(dialog: &FileDialog) -> String {
    if let Some(name) = dialog.file_name.as_deref().filter(|n| !n.trim().is_empty()) {
        return name.to_owned();
    }
    let extension = dialog
        .filters
        .iter()
        .flat_map(|(_, extensions)| extensions)
        .find(|e| e.as_str() != "*");
    match extension {
        Some(extension) => format!("untitled.{extension}"),
        None => "untitled".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn filters(extensions: &[&str]) -> Vec<(String, Vec<String>)> {
        vec![("Files".into(), extensions.iter().map(|e| e.to_string()).collect())]
    }

    #[test]
    fn mime_types_follow_the_filters() {
        assert_eq!(
            open_mime_types(&filters(&["json"])),
            ["application/json", "application/octet-stream"]
        );
        // A script has no MIME type of its own: everything shows.
        assert_eq!(open_mime_types(&filters(&["ts", "json"])), ["*/*"]);
        assert_eq!(open_mime_types(&[]), ["*/*"]);
        assert!(is_script_dialog(&filters(&["TS"])));
        assert!(!is_script_dialog(&filters(&["json"])));
    }

    #[test]
    fn save_names() {
        let mut dialog = FileDialog::new().add_filter("Logs", &["txt"]);
        assert_eq!(save_request(&dialog), ("text/plain".into(), "untitled.txt".into()));
        dialog = dialog.set_file_name("junk.json");
        assert_eq!(save_request(&dialog), ("application/json".into(), "junk.json".into()));
    }

    /// One dialog at a time; a picker that couldn't show leaves none waiting; the answer reaches
    /// the window.
    #[test]
    fn one_dialog_at_a_time() {
        let (tx, rx) = std::sync::mpsc::channel();
        assert!(!begin(Waiting::Open(Box::new(|_| {})), || false));
        assert!(begin(
            Waiting::Open(Box::new(move |path| tx.send(path).unwrap())),
            || true
        ));
        assert!(!begin(Waiting::Open(Box::new(|_| {})), || true));
        picked(Some(PathBuf::from("a.ts")));
        assert_eq!(rx.recv().unwrap(), Some(PathBuf::from("a.ts")));
        // Answered: a new one can show.
        assert!(begin(Waiting::Save(Box::new(|_| {})), || true));
        created(None, None, |_, _| Ok(()));
    }
}
