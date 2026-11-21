//! Process tabs (docs/TAB_PROCESSES_PROPOSAL.md, M3): the shell's side, in the activity's process,
//! which runs no engine. `SlotShell.kt` binds the slots and passes on their messages; this keeps
//! the tab list (`turu_frontend::slots::SlotShell`), writes the slots' records and keeps their
//! passwords itself, and answers each call with what the activity is to do next: a JSON list of
//! `{"kind": "start" | "stop" | "show", "slot": n}`, `{"kind": "send", "slot": n, "json": ...}` and
//! `{"kind": "activity", "slot": n, "json": ...}` (what only the activity does: the text field above
//! the keyboard, links, sharing, file pickers, installing updates). The soft keyboard it shows and
//! hides itself, and keys go from here to the slot on screen ([`to_shown`]).

use jni::EnvUnowned;
use jni::errors::ThrowRuntimeExAndDefault;
use jni::objects::{JClass, JString};
use jni::sys::jint;
use serde_json::{Value, json};
use std::path::PathBuf;
use std::sync::Mutex;
use turu_bot::accounts::SecretStore;
use turu_frontend::shell::{ToClient, ToShell};
use turu_frontend::slots::{Action, SlotRecord, SlotShell};

/// The tab list, and the data folder, while the activity runs.
static SHELL: Mutex<Option<(SlotShell, PathBuf)>> = Mutex::new(None);

/// Carries out the actions this side does (records, passwords) and returns the activity's.
fn activity_actions(root: &std::path::Path, actions: Vec<Action>) -> String {
    let dir = turu_frontend::slots::dir(root);
    let mut list = Vec::new();
    for action in actions {
        match action {
            Action::Record(slot, Some(record)) => {
                if let Err(e) = record.write(&dir, slot) {
                    tracing::error!("Shell: couldn't write slot {slot}'s record: {e}");
                }
            }
            Action::Record(slot, None) => {
                if let Err(e) = SlotRecord::remove(&dir, slot) {
                    tracing::warn!("Shell: couldn't remove slot {slot}'s record: {e}");
                }
                turu_frontend::slots::SlotAddress::remove(&dir, slot);
                // A script it ran isn't brought back in the next tab there, nor its kills for
                // memory held against it (`memory_kills`).
                let _ = std::fs::remove_file(root.join(crate::restart::slot_record_file(slot)));
                turu_frontend::memory_kills::clear(
                    &turu_bot::paths::Paths {
                        root: root.to_owned(),
                    }
                    .crash_dir(),
                    slot,
                );
            }
            Action::Password(slot, password) => {
                let key = crate::slots::password_key(slot);
                let kept = match password {
                    Some(password) => crate::storage::KeystoreStore.set(&key, &password),
                    None => crate::storage::KeystoreStore.delete(&key),
                };
                if let Err(e) = kept {
                    tracing::warn!("Shell: slot {slot}'s password: {e}");
                }
            }
            Action::Start(slot) => list.push(json!({ "kind": "start", "slot": slot })),
            Action::Stop(slot) => list.push(json!({ "kind": "stop", "slot": slot })),
            Action::Show(slot) => {
                // The tab on screen before had it up for a text box of its own.
                crate::ui::soft_keyboard(false);
                list.push(json!({ "kind": "show", "slot": slot }));
            }
            // GameActivity's text input, whose keys come here (`lib.rs`'s `shell_input`).
            Action::Activity(_, ToShell::SoftKeyboard { show }) => crate::ui::soft_keyboard(show),
            Action::Activity(slot, message) => match serde_json::to_string(&message) {
                Ok(text) => list.push(json!({ "kind": "activity", "slot": slot, "json": text })),
                Err(e) => tracing::warn!("Shell: a message I can't write: {e}"),
            },
            // The activity's native Tabs item and panel.
            Action::Tabs(tabs) => match serde_json::to_string(&tabs) {
                Ok(text) => list.push(json!({ "kind": "tabs", "slot": 0, "json": text })),
                Err(e) => tracing::warn!("Shell: a tab list I can't write: {e}"),
            },
            Action::Send(slot, message) => match serde_json::to_string(&message) {
                Ok(text) => list.push(json!({ "kind": "send", "slot": slot, "json": text })),
                Err(e) => tracing::warn!("Shell: a message I can't write: {e}"),
            },
        }
    }
    Value::Array(list).to_string()
}

/// [`with_shell`], for the activity's native screens (`shell_accounts.rs`).
pub(crate) fn with_shell_actions(work: impl FnOnce(&mut SlotShell) -> Vec<Action>) -> String {
    with_shell(work)
}

/// The data folder and the open tabs' logins, while the activity runs.
pub(crate) fn shell_root() -> Option<(PathBuf, Vec<(u32, Option<String>)>)> {
    let shell = SHELL.lock().unwrap_or_else(|e| e.into_inner());
    shell
        .as_ref()
        .map(|(shell, root)| (root.clone(), shell.usernames()))
}

/// An open tab's Streamer Mode is on: the activity's Accounts hide the account names.
pub(crate) fn shell_streamer() -> bool {
    let shell = SHELL.lock().unwrap_or_else(|e| e.into_inner());
    shell.as_ref().is_some_and(|(shell, _)| shell.streamer())
}

/// Runs `work` on the shell, carrying out what comes of it.
fn with_shell(work: impl FnOnce(&mut SlotShell) -> Vec<Action>) -> String {
    let mut shell = SHELL.lock().unwrap_or_else(|e| e.into_inner());
    let Some((shell, root)) = shell.as_mut() else {
        return "[]".into();
    };
    let limit_changed = read_tab_limit(shell, root);
    let mut actions = work(shell);
    if limit_changed {
        actions.push(Action::Tabs(shell.native_tabs()));
    }
    activity_actions(root, actions)
}

/// When `settings.json` last changed, as [`read_tab_limit`] read it (`None`: not read yet).
static TAB_LIMIT_SEEN: Mutex<Option<Option<std::time::SystemTime>>> = Mutex::new(None);

/// Gives the shell Options > Application > Tab Limit, read again when `settings.json` changed (a
/// slot saves it); whether the limit changed.
fn read_tab_limit(shell: &mut SlotShell, root: &std::path::Path) -> bool {
    let paths = turu_bot::paths::Paths {
        root: root.to_owned(),
    };
    let modified = std::fs::metadata(paths.settings_file())
        .and_then(|m| m.modified())
        .ok();
    let mut seen = TAB_LIMIT_SEEN.lock().unwrap_or_else(|e| e.into_inner());
    if *seen == Some(modified) {
        return false;
    }
    *seen = Some(modified);
    let limit = paths.load_settings().unwrap_or_default().tab_limit;
    shell.set_limit(u32::try_from(limit).unwrap_or(1))
}

/// Sends the slot on screen `messages` (keys the activity got): `TuruActivity.shellActions` posts
/// them to the activity's main thread, which sends them on as it sends touches.
pub(crate) fn to_shown(app: &android_activity::AndroidApp, messages: Vec<ToClient>) {
    if messages.is_empty() {
        return;
    }
    let actions = with_shell(|shell| {
        let slot = shell.shown();
        messages
            .into_iter()
            .map(|message| Action::Send(slot, message))
            .collect()
    });
    let sent = crate::platform::with_activity(app, |env, activity| {
        let actions = env.new_string(&actions)?;
        activity.shell_actions(env, &actions)
    });
    if let Err(e) = sent {
        tracing::warn!("Shell: couldn't pass keys on to the tab on screen: {e}");
    }
}

/// The first tab's flags, from `turu-args.txt` (without the app's own: every slot reads those there),
/// its password apart for the Keystore.
fn first_tab(root: &std::path::Path) -> (Vec<String>, Option<String>) {
    let (tab, password, _) = turu_frontend::slots::split_args(&crate::args_lines(root));
    (tab, password)
}

/// `SlotShell.nativeShellStart(filesDir)`: the activity starts as the shell; what to do first.
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_muktiarafi_turu_SlotShell_nativeShellStart<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    files_dir: JString<'local>,
) -> JString<'local> {
    unowned_env
        .with_env(|env| -> jni::errors::Result<JString<'local>> {
            let root = PathBuf::from(files_dir.try_to_string(env)?);
            // The native Accounts reads and keeps passwords with the Keystore, as the slots do.
            static PASSWORDS: std::sync::Once = std::sync::Once::new();
            PASSWORDS.call_once(crate::storage::install);
            let records = SlotRecord::read_all(&turu_frontend::slots::dir(&root));
            let first = records.is_empty().then(|| first_tab(&root));
            // The first start with process tabs on (no records yet): the tabs the app had open in
            // its own process come too, as it would have reopened them (`android-tabs.json`).
            let more = if first.is_some() {
                turu_frontend::saved_tabs::tab_args(&root)
            } else {
                Vec::new()
            };
            let (args, password) = first.unwrap_or_default();
            let (mut shell, mut actions) = SlotShell::new(records, args, more);
            // A new shell: read the limit again.
            *TAB_LIMIT_SEEN.lock().unwrap_or_else(|e| e.into_inner()) = None;
            read_tab_limit(&mut shell, &root);
            if password.is_some() {
                actions.insert(0, Action::Password(1, password));
            }
            // The activity's Tabs item, before any slot is up.
            actions.push(Action::Tabs(shell.native_tabs()));
            tracing::info!("Shell: process tabs in slots {:?}", shell.slots());
            let answer = activity_actions(&root, actions);
            *SHELL.lock().unwrap_or_else(|e| e.into_inner()) = Some((shell, root));
            env.new_string(answer)
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// `SlotShell.nativeFromSlot(slot, json)`: a message from slot `slot`; what to do.
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_muktiarafi_turu_SlotShell_nativeFromSlot<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    slot: jint,
    json: JString<'local>,
) -> JString<'local> {
    unowned_env
        .with_env(|env| -> jni::errors::Result<JString<'local>> {
            let json = json.try_to_string(env)?;
            let answer = match serde_json::from_str::<ToShell>(&json) {
                Ok(message) => with_shell(|shell| shell.message(slot as u32, message)),
                Err(e) => {
                    tracing::warn!("Shell: a message from slot {slot} I can't read: {e}");
                    "[]".into()
                }
            };
            env.new_string(answer)
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// `SlotShell.nativeSlotGone(slot)`: slot `slot`'s process died; what to do.
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_muktiarafi_turu_SlotShell_nativeSlotGone<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    slot: jint,
) -> JString<'local> {
    unowned_env
        .with_env(|env| -> jni::errors::Result<JString<'local>> {
            let answer = with_shell(|shell| shell.disconnected(slot as u32));
            env.new_string(answer)
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// `SlotShell.nativeStillShown(slot)`: the surface couldn't go to another slot; `slot` is still on
/// screen. What to do.
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_muktiarafi_turu_SlotShell_nativeStillShown<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    slot: jint,
) -> JString<'local> {
    unowned_env
        .with_env(|env| -> jni::errors::Result<JString<'local>> {
            let answer = with_shell(|shell| shell.still_shown(slot as u32));
            env.new_string(answer)
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}
