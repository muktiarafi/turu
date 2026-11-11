//! Crash reports sent to Turu's developers through Sentry, once the user said yes: Turu Mas's side
//! of the desktop's `app/src/crash_uploads.rs` (the question the windows ask once, and Options >
//! Application's switch: `sendCrashReports` in `settings.json`, through `Platform`).
//!
//! A Rust panic is sent as it happens, by Sentry's panic hook (before Turu's own, which keeps it in
//! the crash folder and then waits for the report to leave: [`flush`], as a release build aborts on
//! a panic). Crashes in AIR's runtime itself aren't Rust's and aren't caught here.
//!
//! Only builds made with a DSN have it (`TURU_MAS_SENTRY_DSN` when building; the release workflow
//! passes its secret): without one, [`available`] is false, nothing starts and the question isn't
//! asked. Sentry starts with the engine either way, so the switch works at once; until the user
//! says yes it sends nothing. No personal data: no IP address (`send_default_pii` off), no device
//! name, no logs.

use std::borrow::Cow;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use turu_frontend::turu;

/// The project's DSN, given when building.
const DSN: Option<&str> = option_env!("TURU_MAS_SENTRY_DSN");

/// Whether the user said yes: events are dropped while it's false.
static ENABLED: AtomicBool = AtomicBool::new(false);

/// Sentry's client, alive for the whole process.
static CLIENT: OnceLock<sentry::ClientInitGuard> = OnceLock::new();

/// Whether this build can send crash reports.
pub(crate) fn available() -> bool {
    DSN.is_some_and(|dsn| !dsn.trim().is_empty())
}

/// Starts Sentry (builds with a DSN), once the data folder is set (the answer is in its
/// `settings.json`) and after Turu's panic hook (Sentry's runs first, then Turu's).
pub(crate) fn start() {
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
    options.release = Some(Cow::Owned(format!("turu-mas@{}", turu::VERSION)));
    options.environment = Some(Cow::Borrowed(if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    }));
    options.send_default_pii = false;
    options.before_send = Some(Arc::new(|mut event| {
        if !ENABLED.load(Ordering::SeqCst) {
            return None;
        }
        event.server_name = None;
        Some(event)
    }));
    let _ = CLIENT.set(sentry::init(options));
    sentry::configure_scope(|scope| scope.set_tag("client", "air"));
    tracing::info!(
        "Crash reports: {}",
        if consented { "on" } else { "off until asked" }
    );
}

/// The user's answer (Options > Application, or the question): applied at once.
pub(crate) fn set_enabled(on: bool) {
    if !available() || ENABLED.swap(on, Ordering::SeqCst) == on {
        return;
    }
    tracing::info!("Crash reports: {}", if on { "on" } else { "off" });
}

/// Waits (2 seconds at most) for what Sentry has queued to be sent: from the panic hook, before a
/// release build's abort takes the sending thread with it.
pub(crate) fn flush() {
    if !ENABLED.load(Ordering::SeqCst) {
        return;
    }
    if let Some(client) = sentry::Hub::main().client() {
        client.flush(Some(std::time::Duration::from_secs(2)));
    }
}
