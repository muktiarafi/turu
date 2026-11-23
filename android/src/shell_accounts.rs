//! The shell's native Accounts (docs/UI.md, "Native shells", Android): `AccountsPanel.kt` reads
//! the account list and acts on it through these calls, in the activity's process, which runs no
//! engine. The list is `accounts.json` in the data folder, which every process reads; passwords
//! are the Keystore's (`storage::KeystoreStore`), under the account's name, where a slot started
//! for that account finds them (`slots.rs`'s `args`).

use crate::slot_shell::{shell_root, shell_streamer, with_shell_actions};
use jni::EnvUnowned;
use jni::errors::ThrowRuntimeExAndDefault;
use jni::objects::{JClass, JString};
use std::path::{Path, PathBuf};
use turu_bot::accounts::ping::{self, Pings};
use turu_bot::accounts::{self, Account, Accounts};
use turu_frontend::native_shell::{
    AccountRow, AccountsAnswer, AccountsAsk as Ask, AccountsView, ServerRow, ServersView,
};

fn accounts_file(root: &Path) -> PathBuf {
    turu_bot::paths::Paths {
        root: root.to_owned(),
    }
    .accounts_file()
}

/// The account list as the panel shows it: each account with the tab it plays in (an open tab
/// logging in with it) and whether a password is saved; the groups, tags and launch options.
/// With `streamer` (an open tab's Streamer Mode is on), each is labelled "Account n" and the
/// panel shows no username (the edit form keeps the real ones, so saving keeps them).
pub(crate) fn accounts_view(
    root: &Path,
    tabs: &[(u32, Option<String>)],
    streamer: bool,
) -> AccountsView {
    let list = Accounts::load(&accounts_file(root));
    let accounts: Vec<AccountRow> = list
        .accounts
        .iter()
        .enumerate()
        .map(|(n, account)| {
            let tab = tabs.iter().find_map(|(slot, username)| {
                username
                    .as_deref()
                    .is_some_and(|u| u.eq_ignore_ascii_case(&account.username))
                    .then_some(*slot)
            });
            let password = accounts::password(&account.username)
                .ok()
                .flatten()
                .is_some();
            // Turu: VibeSkua's Streamer Mode leaves its Manager's account list as it is; here the
            // list is part of the window being streamed.
            let label = if streamer {
                format!("Account {}", n + 1)
            } else {
                account.label().to_owned()
            };
            AccountRow {
                username: account.username.clone(),
                label,
                display_name: account.display_name.clone(),
                tags: account.tags.clone(),
                groups: list
                    .groups_of(&account.username)
                    .into_iter()
                    .map(str::to_owned)
                    .collect(),
                tab,
                password,
            }
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
    AccountsView {
        accounts,
        groups: list.groups.iter().map(|g| g.name.clone()).collect(),
        tags: list.tags(),
        server: list.server.clone(),
        script: list
            .script
            .as_ref()
            .map(|s| s.to_string_lossy().into_owned()),
        auto_start: list.auto_start,
        scripts,
        keeps_passwords: accounts::keeps_passwords(),
        streamer,
    }
}

/// Carries out `ask` (`native_shell::AccountsAsk`): the activity's actions (a new tab's), or why
/// not.
fn act(root: &Path, ask: Ask) -> AccountsAnswer {
    let file = accounts_file(root);
    let mut list = Accounts::load(&file);
    let saved = |list: &Accounts| match list.save(&file) {
        Ok(()) => AccountsAnswer::default(),
        Err(e) => AccountsAnswer::failed(format!("Couldn't save the accounts: {e}")),
    };
    let start_args = |list: &Accounts, username: String| {
        let script = list
            .script
            .as_ref()
            .map(|s| s.to_string_lossy().into_owned());
        turu_frontend::slots::open_args(
            Some(username),
            Some(list.server.clone()).filter(|s| !s.trim().is_empty()),
            script,
            list.auto_start,
        )
    };
    match ask {
        Ask::Start { username } => {
            let args = start_args(&list, username);
            AccountsAnswer {
                error: None,
                actions: with_shell_actions(|shell| shell.open(args, None)),
            }
        }
        Ask::StartMany { usernames } => {
            // One answer for all: each `open` adds its slot's actions, the last tab shown. A
            // full pool opens nothing more.
            let all: Vec<Vec<String>> = usernames
                .into_iter()
                .map(|u| start_args(&list, u))
                .collect();
            AccountsAnswer {
                error: None,
                actions: with_shell_actions(|shell| {
                    all.into_iter()
                        .flat_map(|args| shell.open(args, None))
                        .collect()
                }),
            }
        }
        Ask::AddToGroup { group, usernames } => {
            let group = group.trim();
            if group.is_empty() {
                return AccountsAnswer::failed("Name the group");
            }
            for username in &usernames {
                list.add_to_group(group, username);
            }
            saved(&list)
        }
        Ask::RemoveFromGroup { group, usernames } => {
            if let Some(found) = list.groups.iter_mut().find(|g| g.name == group) {
                found
                    .accounts
                    .retain(|a| !usernames.iter().any(|u| u.eq_ignore_ascii_case(a)));
            }
            saved(&list)
        }
        Ask::RemoveGroup { group } => {
            list.remove_group(&group);
            saved(&list)
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
                return AccountsAnswer::failed("A username is needed");
            }
            let renamed = original
                .as_deref()
                .filter(|o| !o.eq_ignore_ascii_case(&username));
            if (original.is_none() || renamed.is_some()) && list.find(&username).is_some() {
                return AccountsAnswer::failed(format!("{username} is already in the list"));
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
                return AccountsAnswer::failed(format!("Couldn't keep the password: {e}"));
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
            list.shift(&username, by as isize);
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
/// most once a minute). Blocks: called off the main thread.
fn servers_view() -> ServersView {
    // The shell is every tab's window: a failure here must not take it down.
    let fetched = std::panic::catch_unwind(turu_bot::api::servers::fetch_list)
        .unwrap_or_else(|_| Err("the server list couldn't be read".into()));
    match fetched {
        Ok(servers) => {
            let pings = Pings::global();
            pings.measure_in_background(servers.clone());
            ServersView {
                servers: servers
                    .iter()
                    .map(|server| {
                        let name = server.name.trim();
                        ServerRow {
                            name: name.to_owned(),
                            line: ping::status_line(server, pings.get(name)),
                            online: server.online,
                        }
                    })
                    .collect(),
                error: None,
            }
        }
        Err(e) => ServersView {
            servers: Vec::new(),
            error: Some(e.to_string()),
        },
    }
}

/// `AccountsPanel.nativeAccounts()`: the account list (an [`AccountsView`], `null` before the shell
/// starts).
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_muktiarafi_turu_AccountsPanel_nativeAccounts<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
) -> JString<'local> {
    unowned_env
        .with_env(|env| -> jni::errors::Result<JString<'local>> {
            let view =
                shell_root().map(|(root, tabs)| accounts_view(&root, &tabs, shell_streamer()));
            env.new_string(serde_json::to_string(&view).unwrap_or_else(|_| "null".into()))
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// `AccountsPanel.nativeAccountsDo(json)`: does what the panel asks (an `AccountsAsk`); an
/// [`AccountsAnswer`].
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
                    AccountsAnswer::failed(format!("Turu couldn't read the request: {e}"))
                }
                (_, None) => AccountsAnswer::default(),
            };
            env.new_string(answer.json())
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// `AccountsPanel.nativeServers()`: the server list with pings, or why not (a [`ServersView`]).
/// Blocks.
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_muktiarafi_turu_AccountsPanel_nativeServers<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
) -> JString<'local> {
    unowned_env
        .with_env(|env| -> jni::errors::Result<JString<'local>> {
            let view = servers_view();
            env.new_string(serde_json::to_string(&view).unwrap_or_default())
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}
