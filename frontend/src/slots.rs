//! Turu: process tabs on Android (docs/TAB_PROCESSES_PROPOSAL.md, M3). Each tab is a slot: a
//! foreground service in a process of its own (`:c1` ... `:c4`), running one account's engine.
//! The activity is the shell: it hands its surface to the slot on screen and keeps the tab list.
//!
//! Slots outlive the shell: Android restarts a killed slot on its own, with or without the
//! activity, so what a slot needs to start is on disk, not in the shell: its [`SlotRecord`]
//! (`slots/c<N>.json` in the data folder), written when the tab opens and removed when it closes.
//! A record holds the slot's startup flags and never a password (the data folder is shared
//! storage): the slot takes the login's password from Accounts, or from the Keystore's copy the
//! shell kept for it.
//!
//! [`SlotShell`] is the shell's side, without Android: it decides, and the activity carries out
//! its [`Action`]s (start or stop a slot's service, hand it the surface, write a record, send it a
//! message). Shell and slots talk in `crate::shell`'s messages, over Binder.

use crate::shell::{TabInfo, ToClient, ToShell};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The slots the manifest declares (`Slot1Service` ... `Slot4Service`).
pub const POOL: u32 = 4;

/// The folder of the slots' records, in the data folder.
pub fn dir(root: &Path) -> PathBuf {
    root.join("slots")
}

/// What a slot starts with, kept while its tab is open.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SlotRecord {
    /// Startup flags (`--user`, `--server`, `--run-script`, ...), never `--password`.
    pub args: Vec<String>,
}

impl SlotRecord {
    fn file(dir: &Path, slot: u32) -> PathBuf {
        dir.join(format!("c{slot}.json"))
    }

    /// Slot `slot`'s record, if its tab is open.
    pub fn read(dir: &Path, slot: u32) -> Option<Self> {
        let text = std::fs::read_to_string(Self::file(dir, slot)).ok()?;
        serde_json::from_str(&text).ok()
    }

    /// Every open tab's record, by slot.
    pub fn read_all(dir: &Path) -> BTreeMap<u32, Self> {
        (1..=POOL)
            .filter_map(|slot| Some((slot, Self::read(dir, slot)?)))
            .collect()
    }

    pub fn write(&self, dir: &Path, slot: u32) -> std::io::Result<()> {
        std::fs::create_dir_all(dir)?;
        let text = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        let temporary = dir.join(format!("c{slot}.json.tmp"));
        std::fs::write(&temporary, text)?;
        std::fs::rename(&temporary, Self::file(dir, slot))
    }

    pub fn remove(dir: &Path, slot: u32) -> std::io::Result<()> {
        match std::fs::remove_file(Self::file(dir, slot)) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    }

    /// The login it names (`--user`).
    pub fn username(&self) -> Option<&str> {
        flag_value(&self.args, "--user")
    }
}

/// Where a running slot's control API is (`slots/c<N>.api.json`), written when its engine starts:
/// the slot that runs the web monitor polls the others from these when no shell lists them. The
/// token is in the data folder as the app's own control API token is (`api-token`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlotAddress {
    pub api: String,
    pub token: String,
}

impl SlotAddress {
    fn file(dir: &Path, slot: u32) -> PathBuf {
        dir.join(format!("c{slot}.api.json"))
    }

    pub fn read(dir: &Path, slot: u32) -> Option<Self> {
        serde_json::from_str(&std::fs::read_to_string(Self::file(dir, slot)).ok()?).ok()
    }

    pub fn write(&self, dir: &Path, slot: u32) -> std::io::Result<()> {
        std::fs::create_dir_all(dir)?;
        let text = serde_json::to_string(self).map_err(std::io::Error::other)?;
        let temporary = dir.join(format!("c{slot}.api.json.tmp"));
        std::fs::write(&temporary, text)?;
        std::fs::rename(&temporary, Self::file(dir, slot))
    }

    pub fn remove(dir: &Path, slot: u32) {
        let _ = std::fs::remove_file(Self::file(dir, slot));
    }
}

/// The other open tabs' slots with their control APIs, from the files (for the monitor hub,
/// without a shell): each by its record's login, else "Tab n".
pub fn peers_from_files(dir: &Path, me: u32) -> Vec<turu_bot::monitor::hub::PeerSource> {
    SlotRecord::read_all(dir)
        .into_iter()
        .filter(|&(slot, _)| slot != me)
        .filter_map(|(slot, record)| {
            let address = SlotAddress::read(dir, slot)?;
            Some(turu_bot::monitor::hub::PeerSource {
                id: format!("tab:{slot}"),
                label: record
                    .username()
                    .map_or_else(|| format!("Tab {slot}"), str::to_owned),
                api: address.api.parse().ok()?,
                token: address.token,
            })
        })
        .collect()
}

/// The value of `flag` in `args` (`--flag value` or `--flag=value`).
fn flag_value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if arg == flag {
            return iter.next().map(String::as_str);
        }
        if let Some(value) = arg
            .strip_prefix(flag)
            .and_then(|rest| rest.strip_prefix('='))
        {
            return Some(value);
        }
    }
    None
}

/// The startup flags of a tab opened with this login (Army > Accounts' "In a tab", the strip's
/// "+"), as Army's clients get them; the password goes apart.
pub fn open_args(
    username: Option<String>,
    server: Option<String>,
    script: Option<String>,
    run_script: bool,
) -> Vec<String> {
    let mut args = Vec::new();
    if let Some(user) = username.filter(|u| !u.trim().is_empty()) {
        args.extend(["--user".into(), user]);
    }
    if let Some(server) = server.filter(|s| !s.trim().is_empty()) {
        args.extend(["--server".into(), server]);
    }
    if let Some(script) = script.filter(|s| !s.trim().is_empty()) {
        args.extend([
            if run_script {
                "--run-script"
            } else {
                "--load-script"
            }
            .into(),
            script,
        ]);
    }
    args
}

/// The flags for a tab's playlist and Script Options storage (`OpenTab`'s), after
/// [`open_args`]'.
pub fn playlist_args(
    playlist: Option<String>,
    run_playlist: bool,
    options_storage: Option<String>,
) -> Vec<String> {
    let mut args = Vec::new();
    if let Some(playlist) = playlist.filter(|p| !p.trim().is_empty()) {
        let flag = if run_playlist {
            "--run-playlist"
        } else {
            "--load-playlist"
        };
        args.extend([flag.into(), playlist]);
    }
    if let Some(storage) = options_storage.filter(|s| !s.trim().is_empty()) {
        args.extend(["--script-options-storage".into(), storage]);
    }
    args
}

/// Android's own flags in `turu-args.txt` (graphics, logs, test switches), with a value or without:
/// the app's, not a tab's, so every slot reads them there and no record keeps them.
const APP_FLAGS_WITH_VALUE: [&str; 3] = ["--graphics", "--perf-log", "--releases-url"];
const APP_FLAGS: [&str; 2] = ["--safe-drawing", "--test-gpu-loss"];

/// `turu-args.txt`'s flags split into a tab's (for its record, without `--password`), the
/// password, and the app's own.
pub fn split_args(args: &[String]) -> (Vec<String>, Option<String>, Vec<String>) {
    let (mut tab, mut password, mut app) = (Vec::new(), None, Vec::new());
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let name = arg.split_once('=').map_or(arg.as_str(), |(name, _)| name);
        let inline = arg.contains('=');
        if name == "--password" {
            password = if inline {
                arg.split_once('=').map(|(_, value)| value.to_owned())
            } else {
                iter.next().cloned()
            };
        } else if APP_FLAGS_WITH_VALUE.contains(&name) {
            app.push(arg.clone());
            if !inline && let Some(value) = iter.next() {
                app.push(value.clone());
            }
        } else if APP_FLAGS.contains(&name) {
            app.push(arg.clone());
        } else {
            tab.push(arg.clone());
        }
    }
    (tab, password, app)
}

/// What the shell does, in the activity.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    /// Write slot `n`'s record (`None`: remove it).
    Record(u32, Option<SlotRecord>),
    /// Keep (or with `None` forget) the password slot `n` logs in with, in the Keystore.
    Password(u32, Option<String>),
    /// Start slot `n`'s service and bind it.
    Start(u32),
    /// Unbind and stop slot `n`'s service.
    Stop(u32),
    /// Hand the surface to slot `n` (the one before lets go of it first).
    Show(u32),
    /// Send slot `n` a message.
    Send(u32, ToClient),
    /// Something only the activity can do, for slot `n`: the text field above the keyboard (for
    /// the slot on screen), a link, the share sheet, a file picker, installing an update.
    Activity(u32, ToShell),
}

/// A slot whose process is bound and has said hello.
#[derive(Debug, Clone, Default)]
struct Live {
    label: String,
    busy: bool,
    api: Option<String>,
    token: Option<String>,
}

/// The shell's tab list over the slots.
#[derive(Debug, Default)]
pub struct SlotShell {
    records: BTreeMap<u32, SlotRecord>,
    live: BTreeMap<u32, Live>,
    shown: u32,
    /// Headless Mode as a slot last turned it on or off (`None`: none has yet): every slot's.
    headless_mode: Option<bool>,
}

impl SlotShell {
    /// A shell (the activity starting) over the open tabs' records: each slot started (or bound
    /// again, when it runs), the first shown. With none, a first tab with `first` flags.
    pub fn new(records: BTreeMap<u32, SlotRecord>, first: Vec<String>) -> (Self, Vec<Action>) {
        let mut shell = Self {
            records,
            ..Self::default()
        };
        let mut actions = Vec::new();
        if shell.records.is_empty() {
            let record = SlotRecord { args: first };
            shell.records.insert(1, record.clone());
            actions.push(Action::Record(1, Some(record)));
        }
        actions.extend(shell.records.keys().map(|&slot| Action::Start(slot)));
        shell.shown = *shell.records.keys().next().expect("one at least");
        actions.push(Action::Show(shell.shown));
        (shell, actions)
    }

    /// The slot on screen.
    pub fn shown(&self) -> u32 {
        self.shown
    }

    /// The open tabs' slots, in order.
    pub fn slots(&self) -> Vec<u32> {
        self.records.keys().copied().collect()
    }

    /// Slot `slot`'s process is bound and said hello (its control API with it).
    pub fn connected(&mut self, slot: u32, hello: &ToShell) -> Vec<Action> {
        if !self.records.contains_key(&slot) {
            // A slot whose tab was closed meanwhile.
            return vec![Action::Stop(slot)];
        }
        let (api, token) = match hello {
            ToShell::Hello { api, api_token, .. } => (api.clone(), api_token.clone()),
            _ => (None, None),
        };
        let live = self.live.entry(slot).or_default();
        live.api = api;
        live.token = token;
        let mut actions = Vec::new();
        if slot == self.shown {
            actions.push(Action::Show(slot));
        }
        actions.extend(self.tab_lists());
        // As the other tabs have it, once one changed it.
        if let Some(on) = self.headless_mode {
            actions.push(Action::Send(slot, ToClient::HeadlessMode { on }));
        }
        actions
    }

    /// Slot `slot`'s process died (Android restarts it from its record).
    pub fn disconnected(&mut self, slot: u32) -> Vec<Action> {
        if self.live.remove(&slot).is_none() {
            return Vec::new();
        }
        self.tab_lists()
    }

    /// A message from slot `slot`.
    pub fn message(&mut self, slot: u32, message: ToShell) -> Vec<Action> {
        match message {
            ToShell::SelectTab { id } => self.select(id),
            ToShell::CloseTab { id } => self.close(id),
            ToShell::OpenTab {
                username,
                password,
                server,
                script,
                run_script,
                playlist,
                run_playlist,
                options_storage,
            } => {
                let mut args = open_args(username, server, script, run_script);
                args.extend(playlist_args(playlist, run_playlist, options_storage));
                self.open(args, password)
            }
            ToShell::Status { label, busy } => match self.live.get_mut(&slot) {
                Some(live) if live.label != label || live.busy != busy => {
                    live.label = label;
                    live.busy = busy;
                    self.tab_lists()
                }
                _ => Vec::new(),
            },
            hello @ ToShell::Hello { .. } => self.connected(slot, &hello),
            // Every tab's: to the other slots up.
            ToShell::HeadlessMode { on } => {
                if self.headless_mode == Some(on) {
                    return Vec::new();
                }
                self.headless_mode = Some(on);
                self.live
                    .keys()
                    .filter(|&&other| other != slot)
                    .map(|&other| Action::Send(other, ToClient::HeadlessMode { on }))
                    .collect()
            }
            // A hidden slot's text box (it lost the screen while it had the focus) shows nothing.
            field @ (ToShell::FieldShow { .. }
            | ToShell::FieldUpdate { .. }
            | ToShell::FieldHide { .. }
            | ToShell::SoftKeyboard { .. }) => {
                if slot == self.shown {
                    vec![Action::Activity(slot, field)]
                } else {
                    Vec::new()
                }
            }
            // From any slot: a hidden one's update may be installed, say.
            call @ (ToShell::OpenUrl { .. }
            | ToShell::Share { .. }
            | ToShell::PickFile { .. }
            | ToShell::SaveFile { .. }
            | ToShell::Install { .. }) => vec![Action::Activity(slot, call)],
            _ => Vec::new(),
        }
    }

    /// Opens a tab in a free slot and shows it; nothing when every slot is taken.
    pub fn open(&mut self, args: Vec<String>, password: Option<String>) -> Vec<Action> {
        let Some(slot) = (1..=POOL).find(|slot| !self.records.contains_key(slot)) else {
            return Vec::new();
        };
        let record = SlotRecord { args };
        self.records.insert(slot, record.clone());
        let mut actions = vec![
            Action::Record(slot, Some(record)),
            Action::Password(slot, password.filter(|p| !p.is_empty())),
            Action::Start(slot),
        ];
        actions.extend(self.select(slot));
        actions
    }

    /// Shows slot `slot`'s tab.
    pub fn select(&mut self, slot: u32) -> Vec<Action> {
        if !self.records.contains_key(&slot) || slot == self.shown {
            return Vec::new();
        }
        self.shown = slot;
        let mut actions = vec![Action::Show(slot)];
        actions.extend(self.tab_lists());
        actions
    }

    /// The activity couldn't hand the surface over (the slot on screen didn't let go of it):
    /// `slot` is still the one on screen, and the tab lists say so.
    pub fn still_shown(&mut self, slot: u32) -> Vec<Action> {
        if !self.records.contains_key(&slot) || slot == self.shown {
            return Vec::new();
        }
        self.shown = slot;
        self.tab_lists()
    }

    /// Closes slot `slot`'s tab (never the last one): its service stops and its record goes.
    pub fn close(&mut self, slot: u32) -> Vec<Action> {
        if self.records.len() < 2 || !self.records.contains_key(&slot) {
            return Vec::new();
        }
        let index = self.slots().iter().position(|&s| s == slot).unwrap_or(0);
        self.records.remove(&slot);
        self.live.remove(&slot);
        let mut actions = vec![
            Action::Stop(slot),
            Action::Record(slot, None),
            Action::Password(slot, None),
        ];
        if self.shown == slot {
            let slots = self.slots();
            self.shown = slots[index.min(slots.len() - 1)];
            actions.push(Action::Show(self.shown));
        }
        actions.extend(self.tab_lists());
        actions
    }

    /// Every live slot gets the tab list.
    fn tab_lists(&self) -> Vec<Action> {
        let tabs: Vec<TabInfo> = self
            .records
            .iter()
            .map(|(&slot, record)| {
                let live = self.live.get(&slot);
                TabInfo {
                    id: slot,
                    label: live
                        .map(|l| l.label.clone())
                        .filter(|l| !l.is_empty())
                        .or_else(|| record.username().map(str::to_owned))
                        .unwrap_or_else(|| "Login".into()),
                    busy: live.is_some_and(|l| l.busy),
                    down: live.is_none().then(|| "starting".into()),
                    api: live.and_then(|l| l.api.clone()),
                    token: live.and_then(|l| l.token.clone()),
                }
            })
            .collect();
        self.live
            .keys()
            .map(|&slot| {
                Action::Send(
                    slot,
                    ToClient::Tabs {
                        tabs: tabs.clone(),
                        shown: self.shown,
                        you: Some(slot),
                    },
                )
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hello() -> ToShell {
        ToShell::Hello {
            token: String::new(),
            pid: 1,
            version: "test".into(),
            api: Some("127.0.0.1:50001".into()),
            api_token: Some("a-token".into()),
        }
    }

    fn sent_to(actions: &[Action]) -> Vec<u32> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::Send(slot, ToClient::Tabs { .. }) => Some(*slot),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_tabs_playlist_and_storage_become_its_flags() {
        assert_eq!(
            playlist_args(Some("daily.json".into()), true, Some("Squad A".into())),
            [
                "--run-playlist",
                "daily.json",
                "--script-options-storage",
                "Squad A"
            ]
        );
        assert_eq!(
            playlist_args(Some("daily.json".into()), false, None),
            ["--load-playlist", "daily.json"]
        );
        assert!(playlist_args(Some(" ".into()), true, Some(String::new())).is_empty());
    }

    #[test]
    fn records_round_trip_without_passwords() {
        let dir = tempfile::tempdir().unwrap();
        let record = SlotRecord {
            args: open_args(
                Some("Hero".into()),
                Some("Yorumi".into()),
                Some("farm/Gold.ts".into()),
                true,
            ),
        };
        record.write(dir.path(), 2).unwrap();
        assert_eq!(SlotRecord::read(dir.path(), 2), Some(record.clone()));
        assert_eq!(record.username(), Some("Hero"));
        assert_eq!(
            SlotRecord::read_all(dir.path())
                .keys()
                .copied()
                .collect::<Vec<_>>(),
            [2]
        );
        let text = std::fs::read_to_string(dir.path().join("c2.json")).unwrap();
        assert!(!text.contains("password"));
        SlotRecord::remove(dir.path(), 2).unwrap();
        SlotRecord::remove(dir.path(), 2).unwrap();
        assert!(SlotRecord::read_all(dir.path()).is_empty());
    }

    #[test]
    fn turu_args_split_into_a_tab_its_password_and_the_apps_flags() {
        let args: Vec<String> = [
            "--user",
            "Hero",
            "--password=s3cret",
            "--graphics",
            "vulkan",
            "--server",
            "Yorumi",
            "--safe-drawing",
            "--perf-log=60",
        ]
        .map(String::from)
        .to_vec();
        let (tab, password, app) = split_args(&args);
        assert_eq!(tab, ["--user", "Hero", "--server", "Yorumi"]);
        assert_eq!(password.as_deref(), Some("s3cret"));
        assert_eq!(
            app,
            ["--graphics", "vulkan", "--safe-drawing", "--perf-log=60"]
        );
        let (_, password, _) = split_args(&["--password".into(), "other".into()]);
        assert_eq!(password.as_deref(), Some("other"));
    }

    #[test]
    fn the_other_slots_are_found_from_their_files() {
        let dir = tempfile::tempdir().unwrap();
        for slot in [1, 2, 3] {
            let args = if slot == 2 {
                open_args(Some("Alt".into()), None, None, false)
            } else {
                Vec::new()
            };
            SlotRecord { args }.write(dir.path(), slot).unwrap();
        }
        let address = |port: u16| SlotAddress {
            api: format!("127.0.0.1:{port}"),
            token: "a-token".into(),
        };
        address(50001).write(dir.path(), 1).unwrap();
        address(50002).write(dir.path(), 2).unwrap();
        // Slot 3 hasn't started (no address); slot 4 has an address but its tab is closed.
        address(50004).write(dir.path(), 4).unwrap();
        let peers = peers_from_files(dir.path(), 1);
        assert_eq!(peers.len(), 1);
        assert_eq!(
            (peers[0].id.as_str(), peers[0].label.as_str()),
            ("tab:2", "Alt")
        );
        assert_eq!(peers[0].api.port(), 50002);
        SlotAddress::remove(dir.path(), 2);
        assert!(peers_from_files(dir.path(), 1).is_empty());
    }

    #[test]
    fn the_first_shell_opens_a_first_tab() {
        let (shell, actions) =
            SlotShell::new(BTreeMap::new(), vec!["--user".into(), "Hero".into()]);
        assert_eq!(
            actions,
            [
                Action::Record(
                    1,
                    Some(SlotRecord {
                        args: vec!["--user".into(), "Hero".into()]
                    })
                ),
                Action::Start(1),
                Action::Show(1),
            ]
        );
        assert_eq!(shell.shown(), 1);
    }

    #[test]
    fn a_new_shell_binds_the_open_tabs_again() {
        let mut records = BTreeMap::new();
        records.insert(2, SlotRecord::default());
        records.insert(3, SlotRecord::default());
        let (shell, actions) = SlotShell::new(records, Vec::new());
        assert_eq!(
            actions,
            [Action::Start(2), Action::Start(3), Action::Show(2)]
        );
        assert_eq!(shell.slots(), [2, 3]);
    }

    #[test]
    fn opening_selecting_and_closing() {
        let (mut shell, _) = SlotShell::new(BTreeMap::new(), Vec::new());
        assert_eq!(sent_to(&shell.connected(1, &hello())), [1]);

        // "In a tab" from slot 1: slot 2, its password kept apart, shown.
        let actions = shell.message(
            1,
            ToShell::OpenTab {
                username: Some("Alt".into()),
                password: Some("secret".into()),
                server: None,
                script: None,
                run_script: false,
                playlist: None,
                run_playlist: false,
                options_storage: None,
            },
        );
        assert!(actions.contains(&Action::Record(
            2,
            Some(SlotRecord {
                args: vec!["--user".into(), "Alt".into()]
            })
        )));
        assert!(actions.contains(&Action::Password(2, Some("secret".into()))));
        assert!(actions.contains(&Action::Start(2)));
        assert!(actions.contains(&Action::Show(2)));
        assert_eq!(shell.shown(), 2);
        // Slot 2 isn't up yet: listed as starting, under its login.
        let tabs = actions.iter().find_map(|a| match a {
            Action::Send(1, ToClient::Tabs { tabs, .. }) => Some(tabs.clone()),
            _ => None,
        });
        let tabs = tabs.unwrap();
        assert_eq!(tabs[1].label, "Alt");
        assert_eq!(tabs[1].down.as_deref(), Some("starting"));
        assert_eq!(tabs[0].api.as_deref(), Some("127.0.0.1:50001"));

        assert_eq!(sent_to(&shell.connected(2, &hello())), [1, 2]);
        let actions = shell.message(2, ToShell::SelectTab { id: 1 });
        assert_eq!(actions[0], Action::Show(1));

        // Closing the shown tab shows the one after it; the last one stays.
        let actions = shell.close(1);
        assert_eq!(
            &actions[..3],
            [
                Action::Stop(1),
                Action::Record(1, None),
                Action::Password(1, None)
            ]
        );
        assert!(actions.contains(&Action::Show(2)));
        assert!(shell.close(2).is_empty());
    }

    #[test]
    fn a_refused_hand_over_keeps_the_slot_on_screen() {
        let (mut shell, _) = SlotShell::new(BTreeMap::new(), Vec::new());
        shell.connected(1, &hello());
        shell.open(Vec::new(), None);
        shell.connected(2, &hello());
        shell.select(1);
        // Slot 2 didn't let go: still on screen, and selecting slot 1 again tries again.
        assert_eq!(sent_to(&shell.still_shown(2)), [1, 2]);
        assert_eq!(shell.shown(), 2);
        assert_eq!(shell.select(1)[0], Action::Show(1));
    }

    #[test]
    fn only_the_slot_on_screen_shows_a_text_field() {
        let (mut shell, _) = SlotShell::new(BTreeMap::new(), Vec::new());
        shell.connected(1, &hello());
        shell.open(Vec::new(), None);
        shell.connected(2, &hello());
        let field = ToShell::FieldHide {
            field: crate::shell::FieldId::Game,
        };
        assert_eq!(
            shell.message(2, field.clone()),
            [Action::Activity(2, field.clone())]
        );
        assert!(shell.message(1, field).is_empty());
        // So does its soft keyboard.
        let keyboard = ToShell::SoftKeyboard { show: true };
        assert_eq!(
            shell.message(2, keyboard.clone()),
            [Action::Activity(2, keyboard.clone())]
        );
        assert!(shell.message(1, keyboard).is_empty());
        // A hidden slot's update goes to the installer all the same.
        let install = ToShell::Install {
            path: "/u.apk".into(),
        };
        assert_eq!(
            shell.message(1, install.clone()),
            [Action::Activity(1, install)]
        );
    }

    #[test]
    fn headless_mode_reaches_the_other_slots() {
        let (mut shell, _) = SlotShell::new(BTreeMap::new(), Vec::new());
        shell.connected(1, &hello());
        shell.open(Vec::new(), None);
        // Nothing to pass on before a slot changes it.
        assert!(
            !shell
                .connected(2, &hello())
                .iter()
                .any(|a| matches!(a, Action::Send(_, ToClient::HeadlessMode { .. })))
        );
        let on = ToShell::HeadlessMode { on: true };
        assert_eq!(
            shell.message(1, on.clone()),
            [Action::Send(2, ToClient::HeadlessMode { on: true })]
        );
        // The same again (another slot's echo) goes nowhere.
        assert!(shell.message(2, on).is_empty());
        // A slot coming back gets it as the others have it.
        shell.disconnected(2);
        assert!(
            shell
                .connected(2, &hello())
                .contains(&Action::Send(2, ToClient::HeadlessMode { on: true }))
        );
    }

    #[test]
    fn every_slot_taken_opens_nothing() {
        let (mut shell, _) = SlotShell::new(BTreeMap::new(), Vec::new());
        for _ in 1..POOL {
            assert!(!shell.open(Vec::new(), None).is_empty());
        }
        assert!(shell.open(Vec::new(), None).is_empty());
        assert_eq!(shell.slots().len(), POOL as usize);
    }

    #[test]
    fn a_dead_slot_shows_as_starting_and_a_closed_one_is_stopped() {
        let (mut shell, _) = SlotShell::new(BTreeMap::new(), Vec::new());
        shell.connected(1, &hello());
        shell.open(Vec::new(), None);
        shell.connected(2, &hello());
        let actions = shell.disconnected(2);
        assert_eq!(sent_to(&actions), [1]);
        // A slot that says hello after its tab closed is stopped.
        shell.close(2);
        assert_eq!(shell.connected(2, &hello()), [Action::Stop(2)]);
    }
}
