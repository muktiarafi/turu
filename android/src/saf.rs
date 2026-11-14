//! File dialogs through Android's system picker (the Storage Access Framework): Turu's half of
//! `turu_frontend::documents`, which says what's asked and handles the answer.
//!
//! * Open: `TuruActivity.openDocument` shows `ACTION_OPEN_DOCUMENT`; the chosen document is
//!   copied (`Storage.copyDocument`) and that copy's path is the answer
//!   ([`Java_com_muktiarafi_turu_TuruActivity_nativeDocumentPicked`]).
//! * Save: `TuruActivity.createDocument` shows `ACTION_CREATE_DOCUMENT`; the window writes a file
//!   in the app's cache, which `Storage.writeDocument` copies into the chosen document
//!   ([`Java_com_muktiarafi_turu_TuruActivity_nativeDocumentCreated`]).

use jni::errors::ThrowRuntimeExAndDefault;
use jni::objects::{JClass, JObject, JString, JValue};
use jni::{EnvUnowned, jni_sig, jni_str};
use std::path::{Path, PathBuf};
use turu_frontend::documents::{self, Waiting};
use turu_frontend::platform::{Answer, FileDialog};

/// Shows the system's "open" picker; false when it can't (no activity, or a dialog is open).
pub fn pick_file(dialog: &FileDialog, answer: Answer) -> bool {
    let (mime_types, folder) = documents::open_request(dialog);
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
    let (mime_type, file_name) = documents::save_request(dialog);
    show(Waiting::Save(answer), |env, activity| {
        let mime_type = env.new_string(&mime_type)?;
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
fn show(
    dialog: Waiting,
    call: impl FnOnce(&mut jni::Env, &JObject) -> jni::errors::Result<()>,
) -> bool {
    let Some(app) = crate::app() else {
        tracing::info!("No file dialog without the app on screen");
        return false;
    };
    documents::begin(dialog, || {
        crate::platform::with_activity(&app, call)
            .inspect_err(|e| tracing::warn!("Couldn't open the file picker: {e}"))
            .is_ok()
    })
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
            documents::picked(path);
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
            documents::created(uri, path, write_document);
            Ok(())
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// `Storage.writeDocument(uri, path)`: copies the file the window wrote into the document.
fn write_document(uri: &str, path: &Path) -> Result<(), String> {
    let path = path.to_string_lossy().into_owned();
    let error = crate::storage::with_storage(|env, class| {
        let uri = env.new_string(uri)?;
        let path = env.new_string(&path)?;
        let error = env
            .call_static_method(
                class,
                jni_str!("writeDocument"),
                jni_sig!("(Ljava/lang/String;Ljava/lang/String;)Ljava/lang/String;"),
                &[JValue::Object(&uri), JValue::Object(&path)],
            )?
            .l()?;
        crate::storage::optional_string(env, error)
    })?;
    error.map_or(Ok(()), Err)
}
