//! File dialogs through Android's system picker (the Storage Access Framework): Turu Mas's half
//! of `turu_frontend::documents`, which says what's asked and handles the answer, as the Ruffle
//! build's `android/src/saf.rs`. Kotlin's `Documents` shows the picker (from `PickerActivity`, an
//! activity of the extension's own: AIR's is AIR's) and answers through `Bridge`.

use crate::host::BRIDGE;
use jni::errors::ThrowRuntimeExAndDefault;
use jni::objects::{JClass, JObject, JString, JValue};
use jni::{Env, EnvUnowned, jni_sig, jni_str};
use std::path::{Path, PathBuf};
use turu_frontend::documents::{self, Waiting};
use turu_frontend::platform::{Answer, FileDialog};

/// Shows the system's "open" picker; false when it can't (no activity, or a dialog is open).
pub(crate) fn pick_file(dialog: &FileDialog, answer: Answer) -> bool {
    let (mime_types, folder) = documents::open_request(dialog);
    show(Waiting::Open(answer), "openDocument", &mime_types, &folder)
}

/// Shows the system's "save" picker, like [`pick_file`].
pub(crate) fn save_file(dialog: &FileDialog, answer: Answer) -> bool {
    let (mime_type, file_name) = documents::save_request(dialog);
    show(
        Waiting::Save(answer),
        "createDocument",
        &mime_type,
        &file_name,
    )
}

/// `Bridge.<method>(first, second)`, which shows a picker; waits for its answer.
fn show(dialog: Waiting, method: &str, first: &str, second: &str) -> bool {
    documents::begin(dialog, || {
        let shown = with_bridge(|env, class| {
            let first = env.new_string(first)?;
            let second = env.new_string(second)?;
            let method = jni::strings::JNIString::from(method);
            env.call_static_method(
                class,
                &method,
                jni_sig!("(Ljava/lang/String;Ljava/lang/String;)Z"),
                &[JValue::Object(&first), JValue::Object(&second)],
            )?
            .z()
        });
        match shown {
            Ok(shown) => shown,
            Err(e) => {
                tracing::warn!("Couldn't open the file picker: {e}");
                false
            }
        }
    })
}

/// Runs `work` with Kotlin's `Bridge` class on this thread.
fn with_bridge<T>(
    work: impl FnOnce(&mut Env, &JClass) -> jni::errors::Result<T>,
) -> Result<T, String> {
    let (vm, class) = BRIDGE.get().ok_or("the bridge isn't set up yet")?;
    crate::android::attach(vm, |env: &mut Env| work(env, class)).map_err(|e| e.to_string())
}

/// The text of a nullable Java string.
fn optional_string(env: &mut Env, object: JObject) -> jni::errors::Result<Option<String>> {
    if object.is_null() {
        return Ok(None);
    }
    let text = env.cast_local::<JString>(object)?;
    Ok(Some(text.try_to_string(env)?))
}

/// `Bridge.nativeDocumentPicked(path)`: the "open" picker's document, copied to `path`; null
/// when cancelled or the copy failed.
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_muktiarafi_turu_air_Bridge_nativeDocumentPicked<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    path: JString<'local>,
) {
    unowned_env
        .with_env(|env| -> jni::errors::Result<()> {
            let path = optional_string(env, path.into())?.map(PathBuf::from);
            documents::picked(path);
            Ok(())
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// `Bridge.nativeDocumentCreated(uri, path)`: the "save" picker made the document `uri`; the
/// window writes `path`, copied into it afterwards. Both null when cancelled.
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_muktiarafi_turu_air_Bridge_nativeDocumentCreated<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    uri: JString<'local>,
    path: JString<'local>,
) {
    unowned_env
        .with_env(|env| -> jni::errors::Result<()> {
            let uri = optional_string(env, uri.into())?;
            let path = optional_string(env, path.into())?.map(PathBuf::from);
            documents::created(uri, path, write_document);
            Ok(())
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// `Bridge.writeDocument(uri, path)`: copies the file the window wrote into the document.
fn write_document(uri: &str, path: &Path) -> Result<(), String> {
    let path = path.to_string_lossy().into_owned();
    let error = with_bridge(|env, class| {
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
        optional_string(env, error)
    })?;
    error.map_or(Ok(()), Err)
}
