//! Logs > Crashes: the crash reports earlier runs left (`crate::crash_reports`), newest first,
//! and what a user does with one: read it, share or copy it, delete it (and undo that). The view
//! lists them; a report's text is answered by the `text` action, never in the view (a native
//! crash's tombstone or a hang's trace runs to hundreds of KB). Every action names a report by
//! its file name, never a path. Not per tab: the reports are the app's, in one folder.

use crate::crash_reports::{self, Report};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// How long a listing is kept: listing reads every report's first line, and the view runs on
/// each `/events` tick.
const LIST_FOR: Duration = Duration::from_secs(3);

/// Where Delete puts a report until the next Delete (or the next run), so Undo can put it back
/// with its name and time. `list_in` skips names starting with a dot.
const DELETED: &str = ".deleted";

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct CrashesView {
    /// Newest first.
    pub reports: Vec<CrashRow>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct CrashRow {
    /// The file's name: what the actions take.
    pub name: String,
    /// What happened (the report's first line).
    pub summary: String,
    /// When it was written, in local time ("Nov 21, 14:02").
    pub written: String,
}

/// A report's text, as a share sheet or the clipboard can take it.
#[derive(Debug, Clone, PartialEq)]
pub struct CrashText {
    pub text: String,
    /// The share sheet's subject: "Turu 1.2.3 crash report: exit-1.txt".
    pub subject: String,
    /// Whether `text` is only the start of the file.
    pub cut: bool,
}

static LISTED: Mutex<Option<(Instant, Vec<Report>)>> = Mutex::new(None);

fn listed() -> MutexGuard<'static, Option<(Instant, Vec<Report>)>> {
    LISTED.lock().unwrap_or_else(|e| e.into_inner())
}

/// The reports, newest first, listed again at most every few seconds. A test build lists none
/// (it never reads the real data folder).
pub fn reports() -> Vec<Report> {
    if cfg!(test) {
        return Vec::new();
    }
    let mut listed = listed();
    match &*listed {
        Some((at, reports)) if at.elapsed() < LIST_FOR => reports.clone(),
        _ => {
            let reports = crash_reports::list();
            *listed = Some((Instant::now(), reports.clone()));
            reports
        }
    }
}

/// Lists the folder again at the next read.
pub fn forget() {
    *listed() = None;
}

/// The row a report shows as.
pub fn row(report: &Report) -> CrashRow {
    CrashRow {
        name: report.name.clone(),
        summary: report.summary.clone(),
        written: chrono::DateTime::<chrono::Local>::from(report.modified)
            .format("%b %-d, %H:%M")
            .to_string(),
    }
}

/// The Crashes view now.
pub fn view() -> CrashesView {
    CrashesView {
        reports: reports().iter().map(row).collect(),
    }
}

/// `name` if it is a report's name: a plain `.txt` file name, not hidden.
fn checked(name: &str) -> Result<&str, String> {
    let plain = !name.is_empty()
        && !name.starts_with('.')
        && name.ends_with(".txt")
        && !name.contains(['/', '\\'])
        && Path::new(name).file_name().is_some_and(|n| n == name);
    if plain {
        Ok(name)
    } else {
        Err(format!("no crash report named {name}"))
    }
}

/// The text of the report `name` in `dir`, cut to what a share sheet takes.
pub fn text_in(dir: &Path, name: &str) -> Result<CrashText, String> {
    let path = dir.join(checked(name)?);
    let full = std::fs::read_to_string(&path).map_err(|e| format!("Couldn't read {name}: {e}"))?;
    let text = crash_reports::shareable(&full, crash_reports::SHARE_BYTES);
    Ok(CrashText {
        cut: text.len() != full.len(),
        text,
        subject: format!("Turu {} crash report: {name}", crate::turu::VERSION),
    })
}

/// [`text_in`] for the reports' folder.
pub fn text(name: &str) -> Result<CrashText, String> {
    text_in(&crash_reports::dir(), name)
}

fn deleted_dir(dir: &Path) -> PathBuf {
    dir.join(DELETED)
}

/// Deletes the report `name` in `dir`, kept aside until the next Delete so [`restore_in`] can
/// put it back. Only the report: a native crash's raw tombstone beside it stays.
pub fn delete_in(dir: &Path, name: &str) -> Result<(), String> {
    let path = dir.join(checked(name)?);
    if !path.is_file() {
        return Err(format!("no crash report named {name}"));
    }
    let aside = deleted_dir(dir);
    // Only the last Delete can be undone.
    let _ = std::fs::remove_dir_all(&aside);
    std::fs::create_dir_all(&aside)
        .and_then(|()| std::fs::rename(&path, aside.join(name)))
        .map_err(|e| format!("Couldn't delete {name}: {e}"))
}

/// Puts back the report `name` that [`delete_in`] took from `dir` (a rename: its time stays, so
/// it goes back to its place in the list).
pub fn restore_in(dir: &Path, name: &str) -> Result<(), String> {
    let kept = deleted_dir(dir).join(checked(name)?);
    let path = dir.join(name);
    if path.exists() {
        return Err(format!("{name} is already there"));
    }
    std::fs::rename(&kept, &path).map_err(|e| format!("Couldn't put back {name}: {e}"))
}

/// Forgets what Delete kept aside in `dir` (once a run: an Undo is long over by the next one).
pub fn purge_in(dir: &Path) {
    let _ = std::fs::remove_dir_all(deleted_dir(dir));
}

/// [`delete_in`] for the reports' folder; lists it again.
pub fn delete(name: &str) -> Result<(), String> {
    let done = delete_in(&crash_reports::dir(), name);
    forget();
    done
}

/// [`restore_in`] for the reports' folder; lists it again.
pub fn restore(name: &str) -> Result<(), String> {
    let done = restore_in(&crash_reports::dir(), name);
    forget();
    done
}

/// Crash reports' actions (`POST /models/crashes/do`), each on the report named `name`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[cfg_attr(
    feature = "typegen",
    derive(facet::Facet),
    facet(tag = "action", rename_all = "kebab-case")
)]
#[serde(tag = "action", rename_all = "kebab-case")]
#[repr(C)]
pub enum CrashesAct {
    /// Answers the report's text: `{ "text", "subject", "cut" }`.
    Text {
        name: String,
    },
    Delete {
        name: String,
    },
    Restore {
        name: String,
    },
}

pub fn act(act: CrashesAct) -> Result<serde_json::Value, String> {
    match act {
        CrashesAct::Text { name } => {
            let read = text(&name)?;
            return Ok(serde_json::json!({ "text": read.text, "subject": read.subject, "cut": read.cut }));
        }
        CrashesAct::Delete { name } => delete(&name)?,
        CrashesAct::Restore { name } => restore(&name)?,
    }
    Ok(serde_json::json!({}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::SystemTime;

    fn folder(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("turu-crashes-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn only_plain_report_names_are_taken() {
        for bad in [
            "",
            ".seen",
            "../x.txt",
            "a/b.txt",
            "a\\b.txt",
            "memory-1.log",
            ".deleted",
        ] {
            assert!(checked(bad).is_err(), "{bad}");
        }
        assert_eq!(checked("exit-1.txt"), Ok("exit-1.txt"));
    }

    #[test]
    fn a_report_reads_with_its_subject() {
        let dir = folder("text");
        std::fs::write(dir.join("panic-1.txt"), "Turu panicked\nat main.rs").unwrap();
        let read = text_in(&dir, "panic-1.txt").unwrap();
        assert_eq!(read.text, "Turu panicked\nat main.rs");
        assert!(!read.cut);
        assert!(read.subject.ends_with("crash report: panic-1.txt"));
        assert!(text_in(&dir, "missing.txt").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn delete_can_be_undone_in_place() {
        let dir = folder("delete");
        std::fs::write(dir.join("exit-1.txt"), "Low memory").unwrap();
        std::fs::write(dir.join("exit-2.txt"), "Killed").unwrap();
        let old = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
        std::fs::File::options()
            .write(true)
            .open(dir.join("exit-1.txt"))
            .unwrap()
            .set_modified(old)
            .unwrap();
        delete_in(&dir, "exit-1.txt").unwrap();
        let names = |dir: &Path| {
            crash_reports::list_in(dir)
                .into_iter()
                .map(|r| r.name)
                .collect::<Vec<_>>()
        };
        assert_eq!(names(&dir), ["exit-2.txt"]);
        restore_in(&dir, "exit-1.txt").unwrap();
        // Back where it was: the oldest, last.
        assert_eq!(names(&dir), ["exit-2.txt", "exit-1.txt"]);
        // Only the last Delete is kept.
        delete_in(&dir, "exit-1.txt").unwrap();
        delete_in(&dir, "exit-2.txt").unwrap();
        assert!(restore_in(&dir, "exit-1.txt").is_err());
        restore_in(&dir, "exit-2.txt").unwrap();
        purge_in(&dir);
        assert!(!dir.join(DELETED).exists());
        assert!(delete_in(&dir, "exit-1.txt").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
