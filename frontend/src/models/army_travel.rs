//! Army Control's travel (VibeSkua's tabbed host "Map Travel...", "Jump All to Player..." and
//! "Change Server for All..."): the jump settings (the last jump, the maps the clients visited,
//! the saved presets and the last player, `army-control-jump.json`), where each client is, and the
//! online servers. The jumps themselves go to Army Control's targets as any command
//! (`army::command`'s `jump`, `jump-player` and `change-server`).
//!
//! Where each client is gets read every second while Turu runs (egui's Army Control asks each
//! frame, a native page each time it reads the view; one guard keeps it to one read at a time),
//! and a client's new map or room goes to the top of the history (`turu_bot::army::jump`).
//!
//! Each slot is a process of its own sharing the one file: every change reads it again first, so
//! a slot doesn't save over what another wrote. One per process ([`TRAVEL`]); tests make their own.

use super::army::{Peer as ArmyPeer, Row};
use super::tab_state::lock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant};
use turu_bot::Bot;
use turu_bot::army::Command;
use turu_bot::army::jump::{Destination, JumpSettings, Location, Visits};

/// How often the clients' locations are read (VibeSkua's one-second timer).
const POLL_EVERY: Duration = Duration::from_secs(1);
/// A location older than this isn't current (VibeSkua: 5 s).
pub const FRESH_FOR: Duration = Duration::from_secs(5);
/// VibeSkua `MapService.Pads`, Go to's Pad drop-down.
pub const PADS: [&str; 8] = ["Spawn", "Center", "Left", "Right", "Up", "Down", "Top", "Bottom"];

/// A row of Army Control, as travel needs it.
pub struct Peer {
    /// Army Control's row key ("tab:..." or the username, lowercase).
    pub key: String,
    /// The tab's bot, to read its location; `None` for a launched client.
    pub bot: Option<Arc<Bot>>,
    /// A launched client's login (its location comes from the monitor hub).
    pub username: String,
    /// Its id in the monitor hub, where its location is read: a launched client's (`army:<name>`),
    /// or a shell's other tab's (`tab:<n>`, in a client); `None` for a tab of this process.
    pub hub_id: Option<String>,
    /// The logged-in player's name, for the player picker.
    pub player: Option<String>,
}

/// The rows as travel needs them: tabs read their own location, clients come from the hub.
pub fn peers(rows: &[Row]) -> Vec<Peer> {
    rows.iter()
        .filter(|r| !r.ended)
        .map(|r| {
            let hub_id = match &r.peer {
                ArmyPeer::Tab(_) => None,
                ArmyPeer::Shell { id, .. } => Some(format!("tab:{id}")),
                ArmyPeer::Client { username, .. } | ArmyPeer::ShellClient { username, .. } => {
                    Some(turu_bot::monitor::hub::army_id(username))
                }
            };
            let (bot, player) = match &r.peer {
                ArmyPeer::Tab(bot) => {
                    let name = bot.state.lock().username.clone().filter(|n| !n.trim().is_empty());
                    (Some(bot.clone()), name.or_else(|| Some(r.username.clone())))
                }
                ArmyPeer::Client { .. } | ArmyPeer::Shell { .. } | ArmyPeer::ShellClient { .. } => {
                    (None, Some(r.username.clone()))
                }
            };
            Peer {
                key: r.key.clone(),
                bot,
                username: r.username.clone(),
                hub_id,
                player: player.filter(|p| r.logged_in && !p.trim().is_empty()),
            }
        })
        .collect()
}

/// The online servers, as last asked for.
#[derive(Clone, Default)]
pub enum Servers {
    #[default]
    None,
    Loading,
    Ready(Vec<String>),
    Failed(String),
}

#[derive(Default)]
struct Inner {
    settings: JumpSettings,
    loaded: bool,
    visits: Visits,
    /// Each client's last location and when it was read.
    latest: HashMap<String, (Location, Instant)>,
    polled: Option<Instant>,
    servers: Servers,
    /// The history Clear took, for its Undo.
    cleared: Option<Vec<Destination>>,
}

pub struct Travel {
    file: PathBuf,
    inner: Arc<Mutex<Inner>>,
    polling: Arc<AtomicBool>,
}

/// The process's travel. A test build never touches the real data folder's file.
pub static TRAVEL: LazyLock<Travel> = LazyLock::new(|| {
    Travel::new(if cfg!(test) {
        std::env::temp_dir()
            .join(format!("turu-test-{}", std::process::id()))
            .join(turu_bot::army::jump::FILE_NAME)
    } else {
        turu_bot::paths::Paths::new().army_jump_file()
    })
});

impl Travel {
    pub fn new(file: PathBuf) -> Self {
        Self {
            file,
            inner: Arc::default(),
            polling: Arc::default(),
        }
    }

    /// The settings, read from the file the first time.
    pub fn settings(&self) -> JumpSettings {
        let mut inner = lock(&self.inner);
        if !inner.loaded {
            inner.loaded = true;
            inner.settings = JumpSettings::load(&self.file);
        }
        inner.settings.clone()
    }

    /// Reads the file again, applies `change`, saves, and keeps the result.
    pub fn change<R>(&self, change: impl FnOnce(&mut JumpSettings) -> R) -> R {
        let mut inner = lock(&self.inner);
        if self.file.exists() || !inner.loaded {
            inner.settings = JumpSettings::load(&self.file);
        }
        inner.loaded = true;
        let result = change(&mut inner.settings);
        if let Err(e) = inner.settings.save(&self.file) {
            tracing::warn!("Couldn't save Army Control's jump settings: {e}");
        }
        result
    }

    /// Reads where every client is, once a second, on another thread; new visits go to the
    /// history.
    pub fn poll(&self, peers: &[Peer]) {
        {
            let mut inner = lock(&self.inner);
            if inner.polled.is_some_and(|t| t.elapsed() < POLL_EVERY) || self.polling.swap(true, Ordering::AcqRel) {
                return;
            }
            inner.polled = Some(Instant::now());
        }
        let hub = crate::turu::monitor_hub();
        let now = turu_bot::gear::now_ms();
        let mut tabs = Vec::new();
        let mut found = Vec::new();
        for peer in peers {
            match &peer.bot {
                Some(bot) => tabs.push((peer.key.clone(), bot.clone())),
                None => {
                    let location = peer
                        .hub_id
                        .as_deref()
                        .and_then(|id| hub.snapshot(id))
                        .filter(|s| now.saturating_sub(s.sampled_at) < FRESH_FOR.as_millis() as u64)
                        .and_then(|s| Location::from_full_name(&s.map, &s.cell, &s.pad, &peer.username));
                    if let Some(location) = location {
                        found.push((peer.key.clone(), location));
                    }
                }
            }
        }
        let keys: Vec<String> = peers.iter().map(|p| p.key.clone()).collect();
        let (inner, polling, file) = (self.inner.clone(), self.polling.clone(), self.file.clone());
        let spawned = std::thread::Builder::new()
            .name("Turu Army Travel".into())
            .spawn(move || {
                // Flash calls: off the UI and control API threads.
                for (key, bot) in tabs {
                    if let Some(location) = Location::read(&bot) {
                        found.push((key, location));
                    }
                }
                record(&inner, &file, &keys, found);
                polling.store(false, Ordering::Release);
            });
        if spawned.is_err() {
            self.polling.store(false, Ordering::Release);
        }
    }

    /// The current location of the client `key`, while fresh.
    pub fn location(&self, key: &str) -> Option<Location> {
        let inner = lock(&self.inner);
        let (location, at) = inner.latest.get(key)?;
        (at.elapsed() < FRESH_FOR && !location.map.is_empty()).then(|| location.clone())
    }

    /// VibeSkua `JumpArmyToDestination`: remembers the destination and gives its command.
    pub fn jump(&self, destination: &Destination) -> Command {
        self.change(|s| s.remember_jump(destination));
        destination.command()
    }

    /// "Jump to Player...": remembers the name and gives its command.
    pub fn jump_player(&self, name: &str) -> Command {
        let name = name.trim().to_owned();
        self.change(|s| s.player = name.clone());
        Command::JumpPlayer(name)
    }

    /// The logged-in players of `peers`, sorted, once each.
    pub fn players(peers: &[Peer]) -> Vec<String> {
        let mut players: Vec<String> = peers.iter().filter_map(|p| p.player.clone()).collect();
        players.sort_by_key(|p| p.to_lowercase());
        players.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
        players
    }

    /// The player picker's first name: the last one jumped to when it's still there.
    pub fn initial_player(&self, players: &[String]) -> Option<String> {
        let last = self.settings().player;
        players
            .iter()
            .find(|p| p.eq_ignore_ascii_case(&last))
            .or(players.first())
            .cloned()
    }

    /// "Change Server...": loads the online servers on another thread.
    pub fn load_servers(&self) {
        {
            let mut inner = lock(&self.inner);
            if matches!(inner.servers, Servers::Loading) {
                return;
            }
            inner.servers = Servers::Loading;
        }
        let inner = self.inner.clone();
        let bot = crate::turu::bot();
        let spawned = std::thread::Builder::new().name("Turu Servers".into()).spawn(move || {
            let servers = turu_bot::api::servers::get_servers(&bot, true);
            let result = if servers.is_empty() {
                Servers::Failed("Could not load the current server list. Check your connection and try again.".into())
            } else {
                let mut names: Vec<String> = servers
                    .into_iter()
                    .filter(|s| s.online && !s.name.trim().is_empty() && !s.ip.trim().is_empty())
                    .map(|s| s.name.trim().to_owned())
                    .collect();
                names.sort_by_key(|n| n.to_lowercase());
                names.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
                if names.is_empty() {
                    Servers::Failed("The server API did not return any online servers.".into())
                } else {
                    Servers::Ready(names)
                }
            };
            lock(&inner).servers = result;
        });
        if spawned.is_err() {
            lock(&self.inner).servers = Servers::Failed("Couldn't start loading the servers".into());
        }
    }

    pub fn servers(&self) -> Servers {
        lock(&self.inner).servers.clone()
    }

    /// The server list is done with: the next picker loads it again.
    pub fn forget_servers(&self) {
        let mut inner = lock(&self.inner);
        if !matches!(inner.servers, Servers::Loading) {
            inner.servers = Servers::None;
        }
    }
}

/// Records what a poll found: visits go to the history (saved when it changed).
fn record(inner: &Mutex<Inner>, file: &std::path::Path, keys: &[String], found: Vec<(String, Location)>) {
    let mut guard = lock(inner);
    let Inner {
        settings,
        loaded,
        visits,
        latest,
        ..
    } = &mut *guard;
    visits.retain(|k| keys.iter().any(|key| key == k));
    latest.retain(|k, _| keys.iter().any(|key| key == k));
    let mut moved = Vec::new();
    for (key, location) in found {
        let mut probe = JumpSettings::default();
        if visits.record(&mut probe, &key, &location) {
            moved.push(location.clone());
        }
        latest.insert(key, (location, Instant::now()));
    }
    if moved.is_empty() {
        return;
    }
    // Another slot may have saved meanwhile: read the file again before adding the visits.
    if file.exists() || !*loaded {
        *settings = JumpSettings::load(file);
    }
    *loaded = true;
    for location in &moved {
        settings.record_visit(location);
    }
    if let Err(e) = settings.save(file) {
        tracing::warn!("Couldn't save Army Control's jump settings: {e}");
    }
}

// --- the native page's view and actions ------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct TravelDest {
    /// A preset's name, or the player a visit was recorded for.
    pub name: String,
    pub map: String,
    /// Empty for any room.
    pub room: String,
    pub cell: String,
    pub pad: String,
    /// "battleon-1234 · Enter, Spawn" (a visit) or "Farm: battleon-1234" (a preset).
    pub label: String,
}

fn dest_view(d: &Destination, preset: bool) -> TravelDest {
    TravelDest {
        name: d.name.clone(),
        map: d.map.clone(),
        room: d.room.clone(),
        cell: d.cell.clone(),
        pad: d.pad.clone(),
        label: if preset { d.preset_label() } else { d.display_name() },
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct TravelHere {
    /// Army Control's row key.
    pub key: String,
    pub here: TravelDest,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct TravelServers {
    pub loading: bool,
    pub names: Vec<String>,
    /// Why the list couldn't load; empty otherwise.
    pub error: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct ArmyTravelView {
    /// Go to starts with the last jump.
    pub go_to: TravelDest,
    /// The maps the clients travelled to, the newest first.
    pub history: Vec<TravelDest>,
    pub presets: Vec<TravelDest>,
    pub pads: Vec<String>,
    /// The logged-in players of the clients, and the picker's first.
    pub players: Vec<String>,
    pub player: String,
    /// Where each client is, while fresh.
    pub here: Vec<TravelHere>,
    pub servers: TravelServers,
}

/// The view, for a shell: reading it counts as Army Control being open (the shell's other tabs are
/// polled) and reads where the clients are.
pub fn view() -> ArmyTravelView {
    crate::turu::army_control_drawn();
    let travel = &*TRAVEL;
    let peers = peers(&super::army::rows());
    travel.poll(&peers);
    let settings = travel.settings();
    let players = Travel::players(&peers);
    let here = peers
        .iter()
        .filter_map(|p| {
            travel.location(&p.key).map(|l| TravelHere {
                key: p.key.clone(),
                here: dest_view(&l.to_destination(), false),
            })
        })
        .collect();
    let servers = match travel.servers() {
        Servers::None => TravelServers {
            loading: false,
            names: Vec::new(),
            error: String::new(),
        },
        Servers::Loading => TravelServers {
            loading: true,
            names: Vec::new(),
            error: String::new(),
        },
        Servers::Ready(names) => TravelServers {
            loading: false,
            names,
            error: String::new(),
        },
        Servers::Failed(error) => TravelServers {
            loading: false,
            names: Vec::new(),
            error,
        },
    };
    ArmyTravelView {
        go_to: dest_view(&settings.last_jump(), false),
        history: settings.history.iter().map(|d| dest_view(d, false)).collect(),
        presets: settings.presets.iter().map(|d| dest_view(d, true)).collect(),
        pads: PADS.iter().map(|p| (*p).to_owned()).collect(),
        player: travel.initial_player(&players).unwrap_or_default(),
        players,
        here,
        servers,
    }
}

/// A place a shell sends: where to jump, or a preset (`name` is a preset's; empty for a jump).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(default, rename_all = "camelCase")]
pub struct Place {
    pub name: String,
    pub map: String,
    /// Digits only: anything else is dropped.
    pub room: String,
    pub cell: String,
    pub pad: String,
}

/// `place` as a destination, trimmed.
pub fn destination(place: &Place) -> Destination {
    Destination {
        name: place.name.trim().to_owned(),
        map: place.map.trim().to_owned(),
        room: place.room.trim().chars().filter(char::is_ascii_digit).collect(),
        cell: place.cell.trim().to_owned(),
        pad: place.pad.trim().to_owned(),
    }
}

/// Army Travel's actions (`POST /models/army-travel/do`). Jumps are Army Control's commands.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(
    feature = "typegen",
    derive(facet::Facet),
    facet(tag = "action", rename_all = "kebab-case")
)]
#[serde(tag = "action", rename_all = "kebab-case")]
#[repr(C)]
pub enum ArmyTravelAct {
    ClearHistory,
    /// Puts the cleared history back.
    UndoClearHistory,
    /// Saves `preset`, over the one named `editing` when it's given.
    SavePreset {
        editing: Option<String>,
        preset: Place,
    },
    DeletePreset {
        name: String,
    },
    /// Loads the server list.
    Servers,
    ForgetServers,
}

pub fn act(travel: &Travel, act: ArmyTravelAct) -> Result<(), String> {
    match act {
        ArmyTravelAct::ClearHistory => {
            let history = travel.change(|s| std::mem::take(&mut s.history));
            lock(&travel.inner).cleared = Some(history);
        }
        // The cleared maps come back after those visited since, once each, as many as it keeps.
        ArmyTravelAct::UndoClearHistory => {
            let earlier = lock(&travel.inner).cleared.take().ok_or("Nothing to put back")?;
            travel.change(|s| {
                for d in earlier {
                    let known = s
                        .history
                        .iter()
                        .any(|h| h.map.eq_ignore_ascii_case(&d.map) && h.room.eq_ignore_ascii_case(&d.room));
                    if !known {
                        s.history.push(d);
                    }
                }
                s.history.truncate(turu_bot::army::jump::HISTORY_LEN);
            });
        }
        ArmyTravelAct::SavePreset { editing, preset } => {
            let preset = destination(&preset);
            let editing = editing.unwrap_or_default();
            travel.change(|s| {
                let at = s
                    .presets
                    .iter()
                    .position(|p| !editing.is_empty() && p.name.eq_ignore_ascii_case(&editing));
                s.save_preset(at, preset).map(|_| ())
            })?
        }
        ArmyTravelAct::DeletePreset { name } => travel.change(|s| {
            if let Some(at) = s.presets.iter().position(|p| p.name.eq_ignore_ascii_case(&name)) {
                s.delete_preset(at);
            }
        }),
        ArmyTravelAct::Servers => travel.load_servers(),
        ArmyTravelAct::ForgetServers => travel.forget_servers(),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn travel() -> (Travel, PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "turu-army-travel-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let file = dir.join("army-control-jump.json");
        (Travel::new(file.clone()), file)
    }

    fn place(name: &str, map: &str, room: &str) -> Place {
        Place {
            name: name.into(),
            map: map.into(),
            room: room.into(),
            ..Place::default()
        }
    }

    fn save(editing: Option<&str>, preset: Place) -> ArmyTravelAct {
        ArmyTravelAct::SavePreset {
            editing: editing.map(str::to_owned),
            preset,
        }
    }

    fn location(map: &str, room: &str) -> Location {
        Location {
            map: map.into(),
            room: room.into(),
            cell: "Enter".into(),
            pad: "Spawn".into(),
            username: "Questwalker".into(),
        }
    }

    #[test]
    fn presets_by_name_and_a_jump_is_remembered() {
        let (travel, file) = travel();
        act(&travel, save(None, place("Farm", "battleon", "12a34"))).unwrap();
        assert!(act(&travel, save(None, place("", "x", ""))).is_err());
        let saved = JumpSettings::load(&file);
        assert_eq!(saved.presets.len(), 1);
        assert_eq!(
            (saved.presets[0].room.as_str(), saved.presets[0].cell.as_str()),
            ("1234", "Enter")
        );
        // Edited by its old name, renamed.
        let edit = Place {
            cell: "Enter".into(),
            pad: "Spawn".into(),
            ..place("Grind", "yulgar", "")
        };
        act(&travel, save(Some("farm"), edit)).unwrap();
        assert_eq!(travel.settings().presets[0].name, "Grind");
        act(&travel, ArmyTravelAct::DeletePreset { name: "GRIND".into() }).unwrap();
        assert!(travel.settings().presets.is_empty());

        let command = travel.jump(&destination(&place("", "yulgar", "77")));
        assert_eq!(command, destination(&place("", "yulgar", "77")).command());
        assert_eq!(JumpSettings::load(&file).last_jump().map, "yulgar");
        travel.jump_player(" Questwalker ");
        assert_eq!(JumpSettings::load(&file).player, "Questwalker");
    }

    #[test]
    fn visits_keep_what_another_slot_saved() {
        let (travel, file) = travel();
        let keys = vec!["questwalker".to_owned()];
        record(
            &travel.inner,
            &file,
            &keys,
            vec![("questwalker".into(), location("battleon", "1"))],
        );
        // Another slot saves a preset meanwhile.
        let mut other = JumpSettings::load(&file);
        other
            .save_preset(None, destination(&place("Farm", "yulgar", "")))
            .unwrap();
        other.save(&file).unwrap();
        record(
            &travel.inner,
            &file,
            &keys,
            vec![("questwalker".into(), location("yulgar", "2"))],
        );
        let saved = JumpSettings::load(&file);
        assert_eq!(saved.presets.len(), 1);
        assert_eq!(saved.history[0].map, "yulgar");
        assert_eq!(saved.history.len(), 2);
        // The same place again isn't a visit; the location is current.
        record(
            &travel.inner,
            &file,
            &keys,
            vec![("questwalker".into(), location("yulgar", "2"))],
        );
        assert_eq!(JumpSettings::load(&file).history.len(), 2);
        assert_eq!(travel.location("questwalker").unwrap().map, "yulgar");
        // Clear, a new visit, Undo: the new one first, then the cleared ones not already there.
        act(&travel, ArmyTravelAct::ClearHistory).unwrap();
        assert!(JumpSettings::load(&file).history.is_empty());
        record(
            &travel.inner,
            &file,
            &keys,
            vec![("questwalker".into(), location("battleon", "1"))],
        );
        act(&travel, ArmyTravelAct::UndoClearHistory).unwrap();
        let maps: Vec<String> = JumpSettings::load(&file)
            .history
            .iter()
            .map(|d| d.map.clone())
            .collect();
        assert_eq!(maps, ["battleon", "yulgar"]);
        assert!(travel.location("gone").is_none());
    }
}
