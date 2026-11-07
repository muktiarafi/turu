//! After Android kills the process, the service comes back alone (no activity) and the engine
//! logs in and runs `turu-args.txt`'s script. This brings back the script started by hand too:
//! while a script runs, its file, its options storage, the account and the server are kept in
//! the data folder's `android-restart.txt`, and the account's password (when Accounts doesn't
//! have it) with the Keystore's key (`storage.rs`). A script that stops (not for a relogin)
//! forgets them. On the windowless restart they become startup flags, unless `turu-args.txt`
//! names a script or a playlist of its own.

use jni::EnvUnowned;
use jni::objects::JClass;
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use turu_bot::Bot;
use turu_bot::accounts::SecretStore;
use turu_bot::script::ScriptOptionsState;
use turu_frontend::turu;

/// What's kept while a script runs, in the data folder.
const RECORD_FILE: &str = "android-restart.txt";

/// The service started without an activity: Android restarted it after a kill.
static RESTARTED: AtomicBool = AtomicBool::new(false);

/// The options storage the restarted script loads with, once the startup flags loaded it.
static STORAGE: Mutex<Option<String>> = Mutex::new(None);

/// `TuruService.nativeRestarted()`: the service started with no activity in the process, before
/// it starts the engine.
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_muktiarafi_turu_TuruService_nativeRestarted<'local>(
    _env: EnvUnowned<'local>,
    _class: JClass<'local>,
) {
    RESTARTED.store(true, Ordering::Relaxed);
}

/// The running script, as `android-restart.txt` keeps it (one `key<TAB>value` per line).
#[derive(Debug, Default, PartialEq)]
struct Record {
    script: PathBuf,
    /// Its options file's name (`options/<storage>.cfg`), when it has options.
    storage: Option<String>,
    username: Option<String>,
    server: Option<String>,
}

impl Record {
    fn to_text(&self) -> String {
        let mut text = format!("script\t{}\n", self.script.display());
        for (key, value) in [
            ("storage", &self.storage),
            ("username", &self.username),
            ("server", &self.server),
        ] {
            if let Some(value) = value {
                text.push_str(&format!("{key}\t{value}\n"));
            }
        }
        text
    }

    fn parse(text: &str) -> Option<Self> {
        let mut record = Self::default();
        for line in text.lines() {
            let Some((key, value)) = line.split_once('\t') else {
                continue;
            };
            let value = value.trim();
            if value.is_empty() {
                continue;
            }
            match key {
                "script" => record.script = PathBuf::from(value),
                "storage" => record.storage = Some(value.to_owned()),
                "username" => record.username = Some(value.to_owned()),
                "server" => record.server = Some(value.to_owned()),
                _ => {}
            }
        }
        (!record.script.as_os_str().is_empty()).then_some(record)
    }
}

fn record_file() -> PathBuf {
    turu_bot::paths::Paths::new().root.join(RECORD_FILE)
}

fn read_record() -> Option<Record> {
    Record::parse(&std::fs::read_to_string(record_file()).ok()?)
}

/// The secret store's name for the restart's copy of an account's password (no AQW name starts
/// with `@`, so it never meets an account Accounts keeps).
fn password_key(username: &str) -> String {
    format!("@restart:{}", username.to_lowercase())
}

/// The account's password: Accounts', else the one kept for the restart.
fn saved_password(username: &str) -> Option<String> {
    if let Ok(Some(password)) = turu_bot::accounts::password(username) {
        return Some(password);
    }
    crate::storage::KeystoreStore
        .get(&password_key(username))
        .inspect_err(|e| tracing::warn!("{e}"))
        .ok()
        .flatten()
}

/// On a windowless restart, adds the startup flags that bring the remembered script back (a
/// login, when `args` has none, and `--run-script`), before `turu::take_startup_args`.
pub(crate) fn prepare(args: &mut Vec<OsString>) {
    if !RESTARTED.load(Ordering::Relaxed) {
        return;
    }
    let Some(record) = read_record() else {
        tracing::info!("Restarted after Android stopped Turu; no script was running");
        return;
    };
    let has = |flags: &[&str]| {
        args.iter().any(|arg| {
            arg.to_str().is_some_and(|arg| {
                flags
                    .iter()
                    .any(|flag| arg == *flag || arg.strip_prefix(flag).is_some_and(|rest| rest.starts_with('=')))
            })
        })
    };
    if has(&["--run-script", "--load-script", "--run-playlist", "--load-playlist"]) {
        tracing::info!("Restarted after Android stopped Turu: running turu-args.txt's script");
        return;
    }
    if !has(&["--user"]) {
        match record.username.as_deref().and_then(|u| Some((u, saved_password(u)?))) {
            Some((username, password)) => {
                args.extend(["--user", username, "--password", &password].map(OsString::from));
                if let Some(server) = &record.server {
                    args.extend(["--server", server].map(OsString::from));
                }
            }
            None => tracing::warn!(
                "No saved login to restart {} with: log in yourself",
                record.script.display()
            ),
        }
    }
    tracing::info!(
        "Restarted after Android stopped Turu: running {} again",
        record.script.display()
    );
    args.extend([OsString::from("--run-script"), record.script.into_os_string()]);
    *STORAGE.lock().unwrap_or_else(|e| e.into_inner()) = record.storage;
}

/// After `turu::configure_startup` loaded the startup script: its remembered options storage;
/// then watches the scripts that start and stop from now on.
pub(crate) fn start() {
    if let Some(storage) = STORAGE.lock().unwrap_or_else(|e| e.into_inner()).take() {
        let bot = turu::bot();
        if let Some(script) = bot.scripts.loaded() {
            bot.scripts.load_with_storage(&script, Some(&storage));
        }
    }
    let spawned = std::thread::Builder::new().name("Turu Restart".into()).spawn(|| {
        let mut running = false;
        loop {
            std::thread::sleep(Duration::from_secs(2));
            let bot = turu::bot();
            // A playlist's scripts come and go: the Scheduler isn't brought back.
            let now = bot.scripts.is_running() && !bot.scheduler.is_running();
            if now && !running {
                remember(&bot);
            } else if !now && running {
                if bot.servers.is_relogging() {
                    // Auto-relogin stopped it to start it again.
                    continue;
                }
                forget();
            }
            running = now;
        }
    });
    if let Err(e) = spawned {
        tracing::error!("Couldn't start watching the script for restarts: {e}");
    }
}

/// Keeps the running script for a restart.
fn remember(bot: &Bot) {
    let Some(script) = bot.scripts.loaded() else {
        return;
    };
    let storage = match bot.scripts.options() {
        ScriptOptionsState::Loaded(options) => options.path().file_stem().map(|s| s.to_string_lossy().into_owned()),
        _ => None,
    };
    let username = turu_bot::api::servers::cached_username(bot).ok().flatten();
    let server = bot
        .flash
        .get_opt::<String>("objServerInfo.sName")
        .ok()
        .flatten()
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty() && s != "null")
        .or_else(|| Some(bot.servers.last_name()).filter(|s| !s.is_empty()));
    if let Some(username) = &username {
        let password = bot
            .flash
            .get_static::<String>("loginInfo.strPassword")
            .ok()
            .flatten()
            .filter(|p| !p.is_empty());
        let in_accounts = turu_bot::accounts::password(username).ok().flatten();
        if let Some(password) = password
            && in_accounts.as_ref() != Some(&password)
            && let Err(e) = crate::storage::KeystoreStore.set(&password_key(username), &password)
        {
            tracing::warn!("The restart won't log in by itself: {e}");
        }
    }
    let record = Record {
        script,
        storage,
        username,
        server,
    };
    if let Err(e) = std::fs::write(record_file(), record.to_text()) {
        tracing::warn!("Couldn't keep the script for a restart: {e}");
    }
}

/// The script stopped: nothing to bring back.
fn forget() {
    if let Some(username) = read_record().and_then(|r| r.username) {
        let _ = crate::storage::KeystoreStore.delete(&password_key(&username));
    }
    match std::fs::remove_file(record_file()) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
            tracing::warn!("Couldn't forget the script for a restart: {e}");
        }
        _ => {}
    }
}
