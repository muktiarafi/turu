//! The shell's native Accounts (docs/UI.md, "Native shells", Android): `AccountsPanel.kt` reads
//! the account list and acts on it through these calls, in the activity's process, which runs no
//! engine. The list is `accounts.json` in the data folder, which every process reads; passwords
//! are the Keystore's (`storage::KeystoreStore`), under the account's name, where a slot started
//! for that account finds them (`slots.rs`'s `args`).

use crate::slot_shell::{shell_root, with_shell_actions};
use jni::EnvUnowned;
use jni::errors::ThrowRuntimeExAndDefault;
use jni::objects::{JClass, JString};
use serde::Deserialize;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use turu_bot::accounts::{self, Account, Accounts};
use turu_bot::accounts::ping::{self, Pings};

fn accounts_file(root: &Path) -> PathBuf {
    turu_bot::paths::Paths {
        root: root.to_owned(),
    }
    .accounts_file()
}

/// The account list as the panel shows it: each account with the tab it plays in (an open tab
/// logging in with it) and whether a password is saved; the groups, tags and launch options.
pub(crate) fn accounts_view(root: &Path, tabs: &[(u32, Option<String>)]) -> Value {
    let list = Accounts::load(&accounts_file(root));
    let accounts: Vec<Value> = list
        .accounts
        .iter()
        .map(|account| {
            let tab = tabs.iter().find_map(|(slot, username)| {
                username
                    .as_deref()
                    .is_some_and(|u| u.eq_ignore_ascii_case(&account.username))
                    .then_some(*slot)
            });
            let password = accounts::password(&account.username).ok().flatten().is_some();
            json!({
                "username": account.username,
                "label": account.label(),
                "displayName": account.display_name,
                "tags": account.tags,
                "groups": list.groups_of(&account.username),
                "tab": tab,
                "password": password,
            })
        })
        .collect();
    let quick = turu_bot::quick_scripts::get(&root.join("quick-scripts.json"));
    let mut scripts: Vec<String> = Vec::new();
    for script in quick.favorites.iter().chain(quick.recent.iter()) {
        let script = script.to_string_lossy().into_owned();
        if !scripts.contains(&script) {
            scripts.push(script);
        }
    }
    json!({
        "accounts": accounts,
        "groups": list.groups.iter().map(|g| g.name.clone()).collect::<Vec<_>>(),
        "tags": list.tags(),
        "server": list.server,
        "script": list.script,
        "autoStart": list.auto_start,
        "scripts": scripts,
        "keepsPasswords": accounts::keeps_passwords(),
    })
}

/// What the panel asks.
#[derive(Debug, Deserialize)]
#[serde(tag = "do", rename_all = "camelCase")]
enum Ask {
    /// Starts the account in a new tab, with the launch options.
    Start { username: String },
    /// Adds an account, or changes the one named `original`.
    #[serde(rename_all = "camelCase")]
    Save {
        original: Option<String>,
        username: String,
        password: Option<String>,
        display_name: String,
        tags: Vec<String>,
    },
    Remove { username: String },
    /// Up (-1) or down (1) the list.
    Move { username: String, by: isize },
    /// The launch options.
    #[serde(rename_all = "camelCase")]
    Launch {
        server: String,
        script: Option<String>,
        auto_start: bool,
    },
}

/// Carries out `ask`; the activity's actions (a new tab's) as `slot_shell` writes them, or an
/// error as `{"error": ...}`.
fn act(root: &Path, ask: Ask) -> String {
    let file = accounts_file(root);
    let mut list = Accounts::load(&file);
    let saved = |list: &Accounts| match list.save(&file) {
        Ok(()) => "[]".to_owned(),
        Err(e) => json!({ "error": format!("Couldn't save the accounts: {e}") }).to_string(),
    };
    match ask {
        Ask::Start { username } => {
            let script = list.script.as_ref().map(|s| s.to_string_lossy().into_owned());
            let args = turu_frontend::slots::open_args(
                Some(username),
                Some(list.server.clone()).filter(|s| !s.trim().is_empty()),
                script,
                list.auto_start,
            );
            with_shell_actions(|shell| shell.open(args, None))
        }
        Ask::Save {
            original,
            username,
            password,
            display_name,
            tags,
        } => {
            let username = username.trim().to_owned();
            if username.is_empty() {
                return json!({ "error": "A username is needed" }).to_string();
            }
            let renamed = original
                .as_deref()
                .filter(|o| !o.eq_ignore_ascii_case(&username));
            if (original.is_none() || renamed.is_some()) && list.find(&username).is_some() {
                return json!({ "error": format!("{username} is already in the list") }).to_string();
            }
            let account = Account {
                username: username.clone(),
                display_name: display_name.trim().to_owned(),
                tags: tags
                    .into_iter()
                    .map(|t| t.trim().to_owned())
                    .filter(|t| !t.is_empty())
                    .collect(),
                selected: list
                    .find(original.as_deref().unwrap_or(&username))
                    .is_some_and(|a| a.selected),
            };
            match renamed {
                // In place, keeping its spot in the list and its groups.
                Some(old) => {
                    if let Some(slot) = list
                        .accounts
                        .iter_mut()
                        .find(|a| a.username.eq_ignore_ascii_case(old))
                    {
                        *slot = account;
                    }
                    for group in &mut list.groups {
                        for name in &mut group.accounts {
                            if name.eq_ignore_ascii_case(old) {
                                name.clone_from(&username);
                            }
                        }
                    }
                    let kept = accounts::password(old).ok().flatten();
                    let _ = accounts::delete_password(old);
                    if password.as_deref().is_none_or(str::is_empty)
                        && let Some(kept) = kept
                    {
                        let _ = accounts::save_password(&username, &kept);
                    }
                }
                None => {
                    list.upsert(account);
                }
            }
            if let Some(password) = password.filter(|p| !p.is_empty())
                && let Err(e) = accounts::save_password(&username, &password)
            {
                return json!({ "error": format!("Couldn't keep the password: {e}") }).to_string();
            }
            saved(&list)
        }
        Ask::Remove { username } => {
            list.remove(&username);
            if let Err(e) = accounts::delete_password(&username) {
                tracing::warn!("Accounts: {username}'s password: {e}");
            }
            saved(&list)
        }
        Ask::Move { username, by } => {
            list.shift(&username, by);
            saved(&list)
        }
        Ask::Launch {
            server,
            script,
            auto_start,
        } => {
            list.server = server;
            list.script = script.filter(|s| !s.trim().is_empty()).map(PathBuf::from);
            list.auto_start = auto_start;
            saved(&list)
        }
    }
}

/// The servers, fetched now, each with its ping once measured (measured in the background, at
/// most once a minute): `[{name, line, online}]`. Blocks: called off the main thread.
fn servers_view() -> Value {
    // The shell is every tab's window: a failure here must not take it down.
    let fetched = std::panic::catch_unwind(turu_bot::api::servers::fetch_list)
        .unwrap_or_else(|_| Err("the server list couldn't be read".into()));
    match fetched {
        Ok(servers) => {
            let pings = Pings::global();
            pings.measure_in_background(servers.clone());
            Value::Array(
                servers
                    .iter()
                    .map(|server| {
                        let name = server.name.trim();
                        json!({
                            "name": name,
                            "line": ping::status_line(server, pings.get(name)),
                            "online": server.online,
                        })
                    })
                    .collect(),
            )
        }
        Err(e) => json!({ "error": e }),
    }
}

/// `AccountsPanel.nativeAccounts()`: the account list as JSON.
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_muktiarafi_turu_AccountsPanel_nativeAccounts<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
) -> JString<'local> {
    unowned_env
        .with_env(|env| -> jni::errors::Result<JString<'local>> {
            let view = shell_root()
                .map(|(root, tabs)| accounts_view(&root, &tabs))
                .unwrap_or(Value::Null);
            env.new_string(view.to_string())
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// `AccountsPanel.nativeAccountsDo(json)`: does what the panel asks; the activity's actions.
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_muktiarafi_turu_AccountsPanel_nativeAccountsDo<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    json: JString<'local>,
) -> JString<'local> {
    unowned_env
        .with_env(|env| -> jni::errors::Result<JString<'local>> {
            let json = json.try_to_string(env)?;
            let answer = match (serde_json::from_str::<Ask>(&json), shell_root()) {
                (Ok(ask), Some((root, _))) => act(&root, ask),
                (Err(e), _) => {
                    tracing::warn!("Accounts: a request I can't read: {e}");
                    "[]".into()
                }
                (_, None) => "[]".into(),
            };
            env.new_string(answer)
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// `AccountsPanel.nativeServers()`: the server list with pings, or `{"error": ...}`. Blocks.
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_muktiarafi_turu_AccountsPanel_nativeServers<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
) -> JString<'local> {
    unowned_env
        .with_env(|env| -> jni::errors::Result<JString<'local>> {
            env.new_string(servers_view().to_string())
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}
