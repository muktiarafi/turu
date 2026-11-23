//! Turu: process tabs on Android (docs/TAB_PROCESSES_PROPOSAL.md, M3). Each tab is a slot: a
//! foreground service in a process of its own (`:c1` ... `:c9`), running one account's engine.
//! The activity is the shell: it hands its surface to the slot on screen and keeps the tab list.
//!
//! Slots outlive the shell: Android restarts a killed slot on its own, with or without the
//! activity, so what a slot needs to start is on disk, not in the shell: its [`SlotRecord`]
//! (`slots/c<N>.json` in the data folder), written when the tab opens and removed when it closes.
//! A record holds the slot's startup flags and never a password (the data folder is shared
//! storage): the slot takes the login's password from Accounts, or from the Keystore's copy the
//! shell kept for it.
//!
//! The shell's side, without Android, is `turu_shell::slots::SlotShell`: it decides, and the
//! activity carries out its actions. This module holds what the slots' processes share with it:
//! the records and addresses, the flags a tab starts with, and the tab list the native screens
//! show ([`NativeTabs`]). Shell and slots talk in `crate::shell`'s messages, over Binder.

use crate::shell::TabGame;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The slots the manifest declares (`Slot1Service` ... `Slot9Service`): the most tabs, where
/// Options > Application > Tab Limit allows as many (`turu_shell::slots::SlotShell::set_limit`).
pub const POOL: u32 = 9;

/// Where a slot's rail starts, in points, below the shell's native Tabs item (docs/UI.md, "Native
/// shells"): the item's 8-point top padding and its 52-point height. `TabsItem.kt` draws it so.
pub const RAIL_TOP: f32 = 60.0;

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
    /// Its Streamer Mode was on when it last said: kept here so the shell names it "Tab n", not
    /// by its login, while it starts again (also after the app was stopped).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub streamer: bool,
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
                label: record.username().map_or_else(|| format!("Tab {slot}"), str::to_owned),
                api: address.api.parse().ok()?,
                token: address.token,
            })
        })
        .collect()
}

/// The value of `flag` in `args` (`--flag value` or `--flag=value`).
pub fn flag_value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if arg == flag {
            return iter.next().map(String::as_str);
        }
        if let Some(value) = arg.strip_prefix(flag).and_then(|rest| rest.strip_prefix('=')) {
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
        args.extend([if run_script { "--run-script" } else { "--load-script" }.into(), script]);
    }
    args
}

/// `--exit-on-script-stop` for a tab opened with `OpenTab`'s `exit_on_stop` (Army Queue's), after
/// [`open_args`]'.
pub fn exit_args(exit_on_stop: bool) -> Option<String> {
    exit_on_stop.then(|| "--exit-on-script-stop".to_owned())
}

/// The flags for a tab's playlist and Script Options storage (`OpenTab`'s), after
/// [`open_args`]'.
pub fn playlist_args(playlist: Option<String>, run_playlist: bool, options_storage: Option<String>) -> Vec<String> {
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
const APP_FLAGS_WITH_VALUE: [&str; 4] = ["--graphics", "--perf-log", "--releases-url", "--env"];
const APP_FLAGS: [&str; 5] = [
    "--safe-drawing",
    "--test-gpu-loss",
    "--test-memory-kills",
    "--test-low-prompt",
    "--test-performance-offer",
];

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

/// The tab list as the shell's native screens show it (`TabsItem.kt`, `TabPanel.kt`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct NativeTabs {
    pub tabs: Vec<NativeTab>,
    /// The tab on screen.
    pub shown: u32,
    /// Whether the tab on screen hid its rail: the Tabs item hides with it.
    pub rail_hidden: bool,
    /// Tab Limit's tabs are open (or every slot is taken): no New tab.
    pub full: bool,
    /// Tab Limit: how many tabs may be open.
    pub limit: u32,
    /// The most Tab Limit allows ([`POOL`]).
    pub most: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct NativeTab {
    pub id: u32,
    pub label: String,
    /// Its script or Auto runs.
    pub busy: bool,
    /// What its script waits for the user to answer.
    pub waits: Option<String>,
    /// Its process isn't up: "starting".
    pub down: Option<String>,
    /// Where its game is (in game, logging back in, out).
    pub game: TabGame,
    /// The map it's in, while in game.
    pub map: Option<String>,
    /// Its loaded script's name.
    pub script: Option<String>,
    /// Why it's held after kills for memory (`memory_kills`): it waits to be started again.
    pub held: Option<String>,
    /// It was opened for a saved login: the tab can log itself in (the control API's `/login`).
    pub account: bool,
    /// Its control API (`127.0.0.1:<port>`) and token, from its hello: the native screens read
    /// its view models there.
    pub api: Option<String>,
    pub token: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tabs_playlist_and_storage_become_its_flags() {
        assert_eq!(
            playlist_args(Some("daily.json".into()), true, Some("Squad A".into())),
            ["--run-playlist", "daily.json", "--script-options-storage", "Squad A"]
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
            ..Default::default()
        };
        record.write(dir.path(), 2).unwrap();
        assert_eq!(SlotRecord::read(dir.path(), 2), Some(record.clone()));
        assert_eq!(record.username(), Some("Hero"));
        assert_eq!(
            SlotRecord::read_all(dir.path()).keys().copied().collect::<Vec<_>>(),
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
            "--test-gpu-loss",
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
            [
                "--graphics",
                "vulkan",
                "--test-gpu-loss",
                "--safe-drawing",
                "--perf-log=60"
            ]
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
            SlotRecord {
                args,
                ..Default::default()
            }
            .write(dir.path(), slot)
            .unwrap();
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
        assert_eq!((peers[0].id.as_str(), peers[0].label.as_str()), ("tab:2", "Alt"));
        assert_eq!(peers[0].api.port(), 50002);
        SlotAddress::remove(dir.path(), 2);
        assert!(peers_from_files(dir.path(), 1).is_empty());
    }
}
