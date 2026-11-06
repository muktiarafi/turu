//! Turu: crash reports sent to Turu's developers through Sentry, once the user said yes (the
//! question Turu asks once, and Options > Application's switch: `sendCrashReports` in
//! `settings.json`). The desktop's side of Android's `CrashUploads.kt`:
//!
//! * a Rust panic is sent as it happens, by Sentry's panic hook (before Turu's own message box);
//! * a native crash (Windows' `windows::install_crash_handler`, almost always a graphics driver)
//!   is written to the crash folder ([`write_native_report`]) and sent at the next start
//!   ([`send_pending`]): a crashed process is no place for HTTPS.
//!
//! Only builds made with a DSN have it (`TURU_SENTRY_DSN` when building; the release workflow
//! passes its secret): without one, [`available`] is false, nothing starts and the question
//! isn't asked. Sentry starts with the app either way, so the switch works at once; until the
//! user says yes it sends nothing. No personal data: no IP address (`send_default_pii` off), no
//! computer name, the home folder's path replaced with `~`, no logs.

use crate::turu;
use std::borrow::Cow;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};

/// The project's DSN, given when building.
const DSN: Option<&str> = option_env!("TURU_SENTRY_DSN");

/// Native crash reports in the crash folder start with this (`native-<time>.txt`).
const NATIVE_PREFIX: &str = "native-";

/// The reports [`send_pending`] has sent, one name a line (hidden from the crash list).
const SENT_FILE: &str = ".sent-to-sentry";

/// Whether the user said yes: events are dropped while it's false.
static ENABLED: AtomicBool = AtomicBool::new(false);

/// Sentry's client, alive for the whole process.
static CLIENT: OnceLock<sentry::ClientInitGuard> = OnceLock::new();

/// Whether this build can send crash reports.
pub fn available() -> bool {
    DSN.is_some_and(|dsn| !dsn.trim().is_empty())
}

/// Starts Sentry (builds with a DSN), and sends the native crash reports not sent yet if the user
/// said yes. Once, on the main thread, after Turu's panic hook is installed (Sentry's runs first).
pub fn start() {
    let Some(dsn) = DSN.filter(|_| available()) else {
        return;
    };
    let consented = turu_bot::paths::Paths::new()
        .load_settings()
        .and_then(|settings| settings.send_crash_reports)
        == Some(true);
    ENABLED.store(consented, Ordering::SeqCst);
    let mut options = sentry::ClientOptions::default();
    options.dsn = dsn.trim().parse().ok();
    options.release = Some(Cow::Owned(format!("turu-desktop@{}", turu::VERSION)));
    options.environment = Some(Cow::Borrowed(if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    }));
    options.send_default_pii = false;
    options.before_send = Some(Arc::new(|event| {
        if !ENABLED.load(Ordering::SeqCst) {
            return None;
        }
        Some(scrub(event))
    }));
    let _ = CLIENT.set(sentry::init(options));
    if consented {
        tracing::info!("Crash reports: on");
        std::thread::spawn(send_pending);
    }
    // `TURU_TEST_PANIC=1` panics on a thread of its own 10 s after the start, to test the
    // reports (a dev build carries on; a release build closes, as on a real panic).
    if std::env::var_os("TURU_TEST_PANIC").is_some() {
        std::thread::spawn(|| {
            std::thread::sleep(std::time::Duration::from_secs(10));
            panic!("TURU_TEST_PANIC: a test panic");
        });
    }
}

/// The user's answer (Options > Application, or the question): applied at once.
pub fn set_enabled(on: bool) {
    if !available() || ENABLED.swap(on, Ordering::SeqCst) == on {
        return;
    }
    tracing::info!("Crash reports: {}", if on { "on" } else { "off" });
    if on {
        std::thread::spawn(send_pending);
    }
}

/// Without the computer's name, and with the home folder's path (which holds the user's name)
/// replaced with `~` wherever it appears: messages, loaded libraries' paths, attachments' names.
fn scrub(mut event: sentry::protocol::Event<'static>) -> sentry::protocol::Event<'static> {
    event.server_name = None;
    let Some(home) = dirs::home_dir() else {
        return event;
    };
    // The path as it appears in JSON (Windows' backslashes escaped), without the quotes.
    let Ok(quoted) = serde_json::to_string(&home.to_string_lossy()) else {
        return event;
    };
    let escaped = &quoted[1..quoted.len() - 1];
    if escaped.len() < 3 {
        return event;
    }
    match serde_json::to_string(&event) {
        Ok(json) if json.contains(escaped) => {
            serde_json::from_str(&json.replace(escaped, "~")).unwrap_or(event)
        }
        _ => event,
    }
}

/// `text` with the home folder's path replaced with `~`.
fn scrub_text(text: &str) -> String {
    match dirs::home_dir() {
        Some(home) if home.as_os_str().len() >= 3 => text.replace(&*home.to_string_lossy(), "~"),
        _ => text.to_owned(),
    }
}

/// `crash/native-<time>.txt`, written now, from the crash handler: the first line says what
/// crashed (the crash notice shows it), then `details`. [`send_pending`] sends it at the next
/// start.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn write_native_report(summary: &str, details: &str) {
    write_native_report_in(&turu_bot::paths::Paths::new().crash_dir(), summary, details);
}

fn write_native_report_in(dir: &Path, summary: &str, details: &str) {
    let now = chrono::Local::now();
    let report = format!(
        "{summary}\n\nTuru {}\n{}\n{details}\n",
        turu::VERSION,
        now.to_rfc3339()
    );
    let _ = std::fs::create_dir_all(dir);
    let _ = std::fs::write(
        dir.join(format!("{NATIVE_PREFIX}{}.txt", now.format("%F_%H-%M-%S"))),
        report,
    );
}

/// The native crash reports in `dir` not sent yet, oldest first; marks them sent.
fn take_unsent_in(dir: &Path) -> Vec<turu_frontend::crash_reports::Report> {
    let sent_file = dir.join(SENT_FILE);
    let sent = std::fs::read_to_string(&sent_file).unwrap_or_default();
    let sent: Vec<&str> = sent.lines().collect();
    let mut unsent: Vec<_> = turu_frontend::crash_reports::list_in(dir)
        .into_iter()
        .filter(|r| r.name.starts_with(NATIVE_PREFIX) && !sent.contains(&r.name.as_str()))
        .collect();
    if unsent.is_empty() {
        return unsent;
    }
    unsent.reverse();
    // Only names still in the folder, so the file doesn't grow forever.
    let mut names: Vec<String> = turu_frontend::crash_reports::list_in(dir)
        .into_iter()
        .map(|r| r.name)
        .filter(|name| sent.contains(&name.as_str()))
        .collect();
    names.extend(unsent.iter().map(|r| r.name.clone()));
    if let Err(e) = std::fs::write(&sent_file, names.join("\n")) {
        tracing::warn!("Couldn't write {}: {e}", sent_file.display());
    }
    unsent
}

/// Sends the native crash reports not sent yet, with the report attached. Blocks until they're
/// sent (a few seconds at most): run it on a thread of its own.
fn send_pending() {
    if !ENABLED.load(Ordering::SeqCst) {
        return;
    }
    let reports = take_unsent_in(&turu_bot::paths::Paths::new().crash_dir());
    if reports.is_empty() {
        return;
    }
    for report in &reports {
        let text = std::fs::read_to_string(&report.path).unwrap_or_default();
        sentry::with_scope(
            |scope| {
                scope.set_tag("crash", "native");
                scope.add_attachment(sentry::protocol::Attachment {
                    buffer: scrub_text(&text).into_bytes(),
                    filename: report.name.clone(),
                    content_type: Some("text/plain".to_owned()),
                    ..Default::default()
                });
            },
            || sentry::capture_message(&scrub_text(&report.summary), sentry::Level::Fatal),
        );
    }
    if let Some(client) = sentry::Hub::main().client() {
        client.flush(Some(std::time::Duration::from_secs(10)));
    }
    tracing::info!("Sent {} crash report(s)", reports.len());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(name: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("turu-crash-uploads-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn native_reports_are_sent_once() {
        let dir = folder("once");
        write_native_report_in(
            &dir,
            "Turu crashed: exception 0xC0000005 in igvk64.dll",
            "at offset 0x10",
        );
        std::fs::write(dir.join("panic-1.txt"), "Turu panicked").unwrap();
        let first = take_unsent_in(&dir);
        assert_eq!(first.len(), 1);
        assert_eq!(
            first[0].summary,
            "Turu crashed: exception 0xC0000005 in igvk64.dll"
        );
        assert!(take_unsent_in(&dir).is_empty());
        // The crash list doesn't show the record of what was sent.
        assert_eq!(turu_frontend::crash_reports::list_in(&dir).len(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_home_folder_is_left_out() {
        let Some(home) = dirs::home_dir() else {
            return;
        };
        let path = home.join("Turu").join("turu.exe");
        let mut event = sentry::protocol::Event {
            message: Some(format!("crashed in {}", path.display())),
            server_name: Some("Someones-Laptop".into()),
            ..Default::default()
        };
        event = scrub(event);
        let message = event.message.unwrap();
        assert!(!message.contains(&*home.to_string_lossy()), "{message}");
        assert!(message.starts_with("crashed in ~"), "{message}");
        assert_eq!(event.server_name, None);
    }
}
