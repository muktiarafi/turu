//! Turu: reports of how earlier runs ended badly, in the data folder's `crash/`
//! (`Paths::crash_dir`). The Android app writes them: its panic hook a panic's, and
//! `CrashReports.kt` the system's record of each exit Turu didn't ask for (low memory, a native
//! crash with its tombstone, a hang, a kill). On Windows, the desktop's crash handler writes a
//! native crash's (`crash_uploads::write_native_report`). Each is a text file whose first line says what
//! happened. Diagnostics > Logs lists them, and the first frame after a new one shows a notice.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// The names of the reports a notice was shown for, one per line.
const SEEN_FILE: &str = ".seen";
/// The newest reports kept; older ones are deleted.
const KEEP: usize = 30;

#[derive(Debug, Clone, PartialEq)]
pub struct Report {
    pub path: PathBuf,
    pub name: String,
    /// The first line: what happened.
    pub summary: String,
    pub modified: SystemTime,
}

/// The folder the reports are in.
pub fn dir() -> PathBuf {
    turu_bot::paths::Paths::new().crash_dir()
}

/// The reports in `dir`, newest first.
pub fn list_in(dir: &Path) -> Vec<Report> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut reports: Vec<Report> = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let name = path.file_name()?.to_str()?.to_owned();
            if !name.ends_with(".txt") || name.starts_with('.') {
                return None;
            }
            let modified = entry
                .metadata()
                .and_then(|m| m.modified())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            let summary = std::fs::read_to_string(&path)
                .ok()
                .and_then(|text| text.lines().next().map(str::to_owned))
                .unwrap_or_default();
            Some(Report {
                path,
                name,
                summary,
                modified,
            })
        })
        .collect();
    reports.sort_by(|a, b| {
        b.modified
            .cmp(&a.modified)
            .then_with(|| b.name.cmp(&a.name))
    });
    reports
}

/// The reports, newest first.
pub fn list() -> Vec<Report> {
    list_in(&dir())
}

/// The reports in `dir` added since the last call (all of them the first time), newest first;
/// deletes all but the newest [`KEEP`].
pub fn take_unseen_in(dir: &Path) -> Vec<Report> {
    let reports = list_in(dir);
    for old in reports.iter().skip(KEEP) {
        let _ = std::fs::remove_file(&old.path);
    }
    let reports: Vec<Report> = reports.into_iter().take(KEEP).collect();
    let seen_file = dir.join(SEEN_FILE);
    let seen = std::fs::read_to_string(&seen_file).unwrap_or_default();
    let seen: Vec<&str> = seen.lines().collect();
    let unseen: Vec<Report> = reports
        .iter()
        .filter(|r| !seen.contains(&r.name.as_str()))
        .cloned()
        .collect();
    if !unseen.is_empty() {
        let names: Vec<&str> = reports.iter().map(|r| r.name.as_str()).collect();
        if let Err(e) = std::fs::write(&seen_file, names.join("\n")) {
            tracing::warn!("Couldn't write {}: {e}", seen_file.display());
        }
    }
    unseen
}

/// [`take_unseen_in`] for [`dir`].
pub fn take_unseen() -> Vec<Report> {
    take_unseen_in(&dir())
}

/// What a share sheet or the clipboard can take in one go (Android's binder limit is ~1 MB for
/// a whole transaction): the start of `text`, cut at a line, with a note when it was cut.
pub fn shareable(text: &str, max_bytes: usize) -> String {
    if text.len() <= max_bytes {
        return text.to_owned();
    }
    let mut end = max_bytes;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    let end = text[..end].rfind('\n').unwrap_or(end);
    format!(
        "{}\n... ({} more bytes in the file)\n",
        &text[..end],
        text.len() - end
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("turu-crash-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn new_reports_are_shown_once() {
        let dir = folder("unseen");
        std::fs::write(dir.join("exit-1.txt"), "Low memory\nmore").unwrap();
        std::fs::write(dir.join("notes.log"), "not a report").unwrap();
        let first = take_unseen_in(&dir);
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].summary, "Low memory");
        assert!(take_unseen_in(&dir).is_empty());
        std::fs::write(dir.join("panic-2.txt"), "Turu panicked").unwrap();
        let second = take_unseen_in(&dir);
        assert_eq!(second.len(), 1);
        assert_eq!(second[0].name, "panic-2.txt");
        assert_eq!(list_in(&dir).len(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_missing_folder_has_no_reports() {
        assert!(take_unseen_in(Path::new("/nonexistent/turu/crash")).is_empty());
    }

    #[test]
    fn long_texts_are_cut_at_a_line() {
        let text = "aaaa\nbbbb\ncccc\n";
        assert_eq!(shareable(text, 100), text);
        let cut = shareable(text, 12);
        assert!(cut.starts_with("aaaa\nbbbb\n"));
        assert!(!cut.contains("cccc"));
        assert!(cut.contains("more bytes"));
    }
}
