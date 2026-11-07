//! File dialogs through Android's system picker (the Storage Access Framework): the bot windows'
//! [`Platform::pick_file`](turu_frontend::platform::Platform::pick_file) and `save_file`, which
//! answer with a path as on a desktop, so their callers (Load Script, Junk Items' and Loadouts'
//! import and export, the Scheduler, the Interceptor's export, the logs' Save, ...) work as they
//! are.
//!
//! * Open: `TuruActivity.openDocument` shows `ACTION_OPEN_DOCUMENT` with the dialog's filters as
//!   MIME types; the chosen document is copied (`Storage.copyDocument`) into the app's cache, or
//!   for a script into the scripts folder's `Imported/` (its `../core/...` imports then find the
//!   scripts folder's), and that copy's path is the answer.
//! * Save: `TuruActivity.createDocument` shows `ACTION_CREATE_DOCUMENT` with the dialog's file
//!   name; the answer is a file in the app's cache, and once the window has written it (its size
//!   and time stop changing) `Storage.writeDocument` copies it into the chosen document.
//!
//! One dialog at a time, like a desktop's modal one.

use jni::errors::ThrowRuntimeExAndDefault;
use jni::objects::{JClass, JObject, JString, JValue};
use jni::{EnvUnowned, jni_sig, jni_str};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use turu_frontend::platform::{Answer, FileDialog};

/// The dialog on screen, waiting for the activity's answer.
enum Waiting {
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

/// Shows the system's "open" picker; false when it can't (no activity, or a dialog is open).
pub fn pick_file(dialog: &FileDialog, answer: Answer) -> bool {
    let mime_types = open_mime_types(&dialog.filters).join("\n");
    let folder = if is_script_dialog(&dialog.filters) {
        turu_frontend::turu::bot()
            .paths
            .scripts_dir()
            .join("Imported")
            .to_string_lossy()
            .into_owned()
    } else {
        String::new()
    };
    show(Waiting::Open(answer), |env, activity| {
        let mime_types = env.new_string(&mime_types)?;
        let folder = env.new_string(&folder)?;
        env.call_method(
            activity,
            jni_str!("openDocument"),
            jni_sig!("(Ljava/lang/String;Ljava/lang/String;)V"),
            &[JValue::Object(&mime_types), JValue::Object(&folder)],
        )?;
        Ok(())
    })
}

/// Shows the system's "save" picker, like [`pick_file`].
pub fn save_file(dialog: &FileDialog, answer: Answer) -> bool {
    let file_name = save_name(dialog);
    let mime_type = extension(&file_name)
        .and_then(mime_type)
        .unwrap_or("application/octet-stream");
    show(Waiting::Save(answer), |env, activity| {
        let mime_type = env.new_string(mime_type)?;
        let file_name = env.new_string(&file_name)?;
        env.call_method(
            activity,
            jni_str!("createDocument"),
            jni_sig!("(Ljava/lang/String;Ljava/lang/String;)V"),
            &[JValue::Object(&mime_type), JValue::Object(&file_name)],
        )?;
        Ok(())
    })
}

/// Waits for the activity's answer to the dialog `call` shows.
fn show(dialog: Waiting, call: impl FnOnce(&mut jni::Env, &JObject) -> jni::errors::Result<()>) -> bool {
    let Some(app) = crate::app() else {
        tracing::info!("No file dialog without the app on screen");
        return false;
    };
    {
        let mut waiting = waiting();
        if waiting.is_some() {
            return false;
        }
        // Before the call: the activity may answer (a failure) before it returns.
        *waiting = Some(dialog);
    }
    match crate::platform::with_activity(&app, call) {
        Ok(()) => true,
        Err(e) => {
            tracing::warn!("Couldn't open the file picker: {e}");
            waiting().take();
            false
        }
    }
}

/// `TuruActivity.nativeDocumentPicked(path)`: the "open" picker's document, copied to `path`;
/// null when cancelled or the copy failed.
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_muktiarafi_turu_TuruActivity_nativeDocumentPicked<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    path: JString<'local>,
) {
    unowned_env
        .with_env(|env| -> jni::errors::Result<()> {
            let path = crate::storage::optional_string(env, path.into())?.map(PathBuf::from);
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
            Ok(())
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// `TuruActivity.nativeDocumentCreated(uri, path)`: the "save" picker made the document `uri`;
/// the window writes `path`, copied into it afterwards. Both null when cancelled.
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_muktiarafi_turu_TuruActivity_nativeDocumentCreated<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    uri: JString<'local>,
    path: JString<'local>,
) {
    unowned_env
        .with_env(|env| -> jni::errors::Result<()> {
            let uri = crate::storage::optional_string(env, uri.into())?;
            let path = crate::storage::optional_string(env, path.into())?.map(PathBuf::from);
            let answer = match waiting().take() {
                Some(Waiting::Save(answer) | Waiting::Open(answer)) => answer,
                None => return Ok(()),
            };
            match (uri, path) {
                (Some(uri), Some(path)) => {
                    // Not there yet: the window's write is what's waited for.
                    let _ = std::fs::remove_file(&path);
                    answer(Some(path.clone()));
                    write_when_saved(uri, path);
                }
                _ => answer(None),
            }
            Ok(())
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// Copies `path` into the document `uri` once the window has written it, on a thread of its own.
fn write_when_saved(uri: String, path: PathBuf) {
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
        let path_text = path.to_string_lossy().into_owned();
        let result = crate::storage::with_storage(|env, class| {
            let uri = env.new_string(&uri)?;
            let path = env.new_string(&path_text)?;
            let error = env
                .call_static_method(
                    class,
                    jni_str!("writeDocument"),
                    jni_sig!("(Ljava/lang/String;Ljava/lang/String;)Ljava/lang/String;"),
                    &[JValue::Object(&uri), JValue::Object(&path)],
                )?
                .l()?;
            crate::storage::optional_string(env, error)
        });
        match result {
            Ok(None) => tracing::info!("Saved {}", path.file_name().unwrap_or_default().to_string_lossy()),
            Ok(Some(e)) | Err(e) => tracing::error!("Couldn't save {}: {e}", path.display()),
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
