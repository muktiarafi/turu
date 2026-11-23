//! File dialogs through Android's system picker (the Storage Access Framework): Turu's half of
//! `turu_frontend::documents`, which says what's asked and handles the answer.
//!
//! A slot asks the shell's activity, which shows the picker:
//!
//! * Open: `TuruActivity.openDocument` shows `ACTION_OPEN_DOCUMENT`; the chosen document is
//!   copied (`Storage.copyDocument`) and that copy's path is the answer (`ToClient::DocumentPicked`).
//! * Save: `TuruActivity.createDocument` shows `ACTION_CREATE_DOCUMENT`; the window writes a file
//!   in the app's cache, which `Storage.writeDocument` copies into the chosen document
//!   (`ToClient::DocumentCreated`, [`write_document`]).

use std::path::Path;
use turu_frontend::documents::{self, Waiting};
use turu_frontend::host::{Answer, FileDialog};
use turu_frontend::shell::ToShell;

/// Asks the shell's activity for the system's "open" picker (only slots run the engine); its answer
/// comes back as `ToClient::DocumentPicked`. False when it can't (a dialog is open).
pub fn pick_file(dialog: &FileDialog, answer: Answer) -> bool {
    let (mime_types, folder) = documents::open_request(dialog);
    documents::begin(Waiting::Open(answer), || {
        crate::slots::to_shell(ToShell::PickFile { mime_types, folder });
        true
    })
}

/// Asks for the system's "save" picker, like [`pick_file`]; the answer comes back as
/// `ToClient::DocumentCreated`.
pub fn save_file(dialog: &FileDialog, answer: Answer) -> bool {
    let (mime_type, file_name) = documents::save_request(dialog);
    documents::begin(Waiting::Save(answer), || {
        crate::slots::to_shell(ToShell::SaveFile {
            mime_type,
            file_name,
        });
        true
    })
}

/// `Storage.writeDocument(uri, path)`: copies the file the window wrote into the document.
pub(crate) fn write_document(uri: &str, path: &Path) -> Result<(), String> {
    let path = path.to_string_lossy().into_owned();
    let error = crate::storage::with_storage(|env| {
        let uri = env.new_string(uri)?;
        let path = env.new_string(&path)?;
        let error = crate::java::Storage::write_document(env, &uri, &path)?;
        crate::storage::optional_string(env, error.into())
    })?;
    error.map_or(Ok(()), Err)
}
