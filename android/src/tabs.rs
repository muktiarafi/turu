//! The tabs after the first, kept in the data folder's `android-tabs.json` so they come back
//! when Android stopped the app (killed it in the background, after which the service starts the
//! engine alone): each one's account, server and script, and whether its script ran. The first
//! tab is `turu-args.txt`'s (phase 4's restart). The file is written when a tab opens or closes
//! and when what it holds changes (the notification thread looks every 2 s); never a password:
//! a reopened tab logs in with the account's saved one, if any, else at the login screen.

use serde_json::{Value, json};
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use turu_bot::Bot;
use turu_bot::startup::Startup;
use turu_frontend::turu::{self, TabRequest};

const FILE: &str = "android-tabs.json";

/// The file was read: from then on it's written (before, it would lose the tabs to reopen).
static READ: AtomicBool = AtomicBool::new(false);
/// What was last written.
static WRITTEN: Mutex<Option<String>> = Mutex::new(None);

fn file() -> PathBuf {
    turu_bot::paths::Paths::new().root.join(FILE)
}

/// Opens the tabs `android-tabs.json` lists, after the first (the engine's loop opens them, in
/// order); their scripts start once they're logged in.
pub fn reopen() {
    let text = std::fs::read_to_string(file()).unwrap_or_default();
    *WRITTEN.lock().unwrap_or_else(|e| e.into_inner()) = Some(text.clone());
    READ.store(true, Ordering::Relaxed);
    let entries = match serde_json::from_str::<Vec<Value>>(&text) {
        Ok(entries) => entries,
        Err(e) if !text.trim().is_empty() => {
            tracing::warn!("{FILE}: {e}; no tabs reopened");
            return;
        }
        Err(_) => return,
    };
    for entry in entries {
        let string = |key: &str| {
            entry
                .get(key)
                .and_then(Value::as_str)
                .map(str::to_owned)
                .filter(|s| !s.trim().is_empty())
        };
        let username = string("username");
        let password = username
            .as_deref()
            .and_then(|u| turu_bot::accounts::password(u).ok().flatten());
        let startup = Startup {
            password,
            username,
            server: string("server"),
            script: string("script").map(PathBuf::from),
            run_script: entry.get("runScript").and_then(Value::as_bool).unwrap_or(false),
            ..Default::default()
        };
        tracing::info!(
            "Reopening a tab for {} ({FILE})",
            startup.username.as_deref().unwrap_or("the login screen")
        );
        turu::request_tab(TabRequest::Open(Box::new(startup)));
    }
}

/// A tab's line in the file.
fn entry(bot: &Bot) -> Value {
    json!({
        "username": turu_bot::api::servers::login_name(bot),
        "server": bot.opts().relogin_server,
        "script": bot.scripts.loaded().map(|p| p.to_string_lossy().into_owned()),
        "runScript": bot.scripts.is_running(),
    })
}

/// Writes the tabs after the first to `android-tabs.json`, when that changed.
pub fn save() {
    if !READ.load(Ordering::Relaxed) {
        return;
    }
    let entries: Vec<Value> = turu::tabs().iter().skip(1).map(|(_, bot)| entry(bot)).collect();
    let text = serde_json::to_string_pretty(&entries).unwrap_or_default();
    let mut written = WRITTEN.lock().unwrap_or_else(|e| e.into_inner());
    if written.as_deref() == Some(text.as_str()) {
        return;
    }
    match std::fs::write(file(), &text) {
        Ok(()) => *written = Some(text),
        Err(e) => tracing::warn!("Couldn't write {FILE}: {e}"),
    }
}
