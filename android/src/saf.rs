//! File dialogs through Android's system picker (the Storage Access Framework): Turu's half of
//! `turu_frontend::documents`, which says what's asked and handles the answer.
//!
//! A slot asks the shell's activity, which shows the picker:
//!
//! * Open: `TuruActivity.openDocument` shows `ACTION_OPEN_DOCUMENT`; the chosen document is
//!   copied (`Storage.copyDocument`) and that copy's path is the answer (`Answered::Path`).
//! * Save: `TuruActivity.createDocument` shows `ACTION_CREATE_DOCUMENT`; the window writes a file
//!   in the app's cache, which `Storage.writeDocument` copies into the chosen document
//!   (`Answered::Document`, [`write_document`]).
//!
//! Each request carries an ID and its answer comes back with it (`client_link::ask`), so an
//! answer the activity sends late, to a slot that asked again since, reaches nothing.

use std::path::Path;
use turu_frontend::client_link;
use turu_frontend::documents::{self, Waiting};
use turu_frontend::host::{Answer, FileDialog};
use turu_frontend::shell::{Answered, ToShell};

/// Asks the shell's activity for the system's "open" picker (only slots run the engine); its answer
/// comes back by the request's ID ([`answered`]). False when it can't (a dialog is open).
pub fn pick_file(dialog: &FileDialog, answer: Answer) -> bool {
    let (mime_types, folder) = documents::open_request(dialog);
    documents::begin(Waiting::Open(answer), || {
        client_link::ask(
            |id| {
                crate::slots::to_shell(ToShell::PickFile {
                    id,
                    mime_types,
                    folder,
                })
            },
            answered,
        );
        true
    })
}

/// Asks for the system's "save" picker, like [`pick_file`].
pub fn save_file(dialog: &FileDialog, answer: Answer) -> bool {
    let (mime_type, file_name) = documents::save_request(dialog);
    documents::begin(Waiting::Save(answer), || {
        client_link::ask(
            |id| {
                crate::slots::to_shell(ToShell::SaveFile {
                    id,
                    mime_type,
                    file_name,
                })
            },
            answered,
        );
        true
    })
}

/// A picker's answer, to the dialog waiting for it (`documents`).
fn answered(answer: Answered) {
    match answer {
        Answered::Path { path } => documents::picked(path.map(std::path::PathBuf::from)),
        Answered::Document { uri, path } => {
            documents::created(uri, path.map(std::path::PathBuf::from), write_document)
        }
        Answered::Installed { .. } => {
            tracing::warn!("A picker answered as an install");
            documents::picked(None);
        }
    }
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
