//! Army Control's map travel (VibeSkua `TabbedHostWindow`'s `ArmyJumpSettings`, saved in
//! `army-control-jump.json`): the last destination and player jumped to, map presets with a
//! room, cell and pad, and the maps the clients actually visited (the newest first, at most
//! [`HISTORY_LEN`]).
//!
//! The file keeps VibeSkua's field names (`Map`, `Room`, `Cell`, `Player`, `History`,
//! `Presets`), so one copied over from VibeSkua reads as it is.

use super::Command;
use crate::Bot;
use crate::api::{map, player};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::io;
use std::path::Path;

pub const FILE_NAME: &str = "army-control-jump.json";
/// How many visited maps the history keeps.
pub const HISTORY_LEN: usize = 15;

/// A place to jump to (VibeSkua `ArmyJumpDestination`): a preset, or a visited map (`name` is
/// then the client that was there).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct Destination {
    pub name: String,
    pub map: String,
    /// The room number, "" for any room.
    pub room: String,
    pub cell: String,
    pub pad: String,
}

impl Default for Destination {
    fn default() -> Self {
        Self {
            name: String::new(),
            map: String::new(),
            room: String::new(),
            cell: "Enter".into(),
            pad: "Spawn".into(),
        }
    }
}

impl Destination {
    /// VibeSkua `DisplayName`: "yulgar-12345 - Enter, Spawn".
    pub fn display_name(&self) -> String {
        format!("{} - {}, {}", self.map_to_join(), self.cell, self.pad)
    }

    /// A preset as the drop-downs list it: "Farm — yulgar-12345 - Enter, Spawn".
    pub fn preset_label(&self) -> String {
        format!("{} — {}", self.name, self.display_name())
    }

    /// The map to join: `map-room`, or the map alone without a room.
    pub fn map_to_join(&self) -> String {
        let (map, room) = (self.map.trim(), self.room.trim());
        if room.is_empty() {
            map.to_owned()
        } else {
            format!("{map}-{room}")
        }
    }

    /// The cell, "Enter" when empty.
    pub fn cell_or_default(&self) -> String {
        or(&self.cell, "Enter")
    }

    /// The pad, "Spawn" when empty.
    pub fn pad_or_default(&self) -> String {
        or(&self.pad, "Spawn")
    }

    /// The command that sends the clients there (VibeSkua `WM_SKUA_JUMP_MAP`: a client already
    /// on that map jumps to the cell, else joins it).
    pub fn command(&self) -> Command {
        Command::JumpMap {
            map: self.map_to_join(),
            cell: self.cell_or_default(),
            pad: self.pad_or_default(),
        }
    }
}

fn or(value: &str, default: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        default.to_owned()
    } else {
        value.to_owned()
    }
}

/// Where a client is (VibeSkua `ArmyClientLocation`): the map's name, the room number the
/// server gave (not the area ID), the cell and pad, and who is there.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Location {
    pub map: String,
    pub room: String,
    pub cell: String,
    pub pad: String,
    pub username: String,
}

impl Location {
    /// From a full map name as the game shows it ("yulgar-12345"); `None` without a map or a
    /// cell (VibeSkua reports nothing then).
    pub fn from_full_name(full: &str, cell: &str, pad: &str, username: &str) -> Option<Self> {
        let full = full.trim();
        if full.is_empty() || cell.trim().is_empty() {
            return None;
        }
        let (map, room) = match full.rsplit_once('-') {
            Some((map, room))
                if !map.is_empty()
                    && !room.is_empty()
                    && room.bytes().all(|b| b.is_ascii_digit()) =>
            {
                (map, room)
            }
            _ => (full, ""),
        };
        // VibeSkua reports the room only when the server gave one (`RoomNumber > 0`).
        let room = if room.parse::<i64>().is_ok_and(|n| n > 0) {
            room
        } else {
            ""
        };
        Some(Self {
            map: map.to_owned(),
            room: room.to_owned(),
            cell: cell.trim().to_owned(),
            pad: or(pad, "Spawn"),
            username: username.to_owned(),
        })
    }

    /// Reads where this client is from the game, once it plays (VibeSkua `MainWindow`'s
    /// one-second timer). Calls Flash: never on the UI thread.
    pub fn read(bot: &Bot) -> Option<Self> {
        {
            let state = bot.state.lock();
            if !state.game_loaded || !bot.flash.has_host() {
                return None;
            }
        }
        if !player::logged_in(bot).ok()? {
            return None;
        }
        // One call (turu.swf's `getMany`); the loading screen is up when its stage is (a number
        // to read: JSON can't write the stage itself).
        let read = bot
            .flash
            .get_many(&[
                ("loading", "world.mapLoadInProgress"),
                ("screen", "mcConnDetail.stage.frameRate"),
                ("map", "world.strMapName"),
                ("area", "world.strAreaName"),
                ("cell", "world.strFrame"),
                ("pad", "world.strPad"),
            ])
            .ok()?;
        let text = |key: &str| read.get(key).and_then(Value::as_str).unwrap_or_default();
        if read.get("loading").and_then(Value::as_bool) == Some(true)
            || !read.get("screen").is_none_or(Value::is_null)
        {
            return None;
        }
        let (name, cell) = (text("map").trim(), text("cell").trim());
        if name.is_empty() || cell.is_empty() {
            return None;
        }
        let room = map::parse_room_number(text("area"));
        Some(Self {
            map: name.to_owned(),
            room: if room > 0 {
                room.to_string()
            } else {
                String::new()
            },
            cell: cell.to_owned(),
            pad: or(text("pad"), "Spawn"),
            username: player::known_username(bot).unwrap_or_default(),
        })
    }

    /// The location as a preset to start from (VibeSkua's "Use Current Client Location").
    pub fn to_destination(&self) -> Destination {
        Destination {
            name: Destination {
                map: self.map.clone(),
                room: self.room.clone(),
                ..Default::default()
            }
            .map_to_join(),
            map: self.map.clone(),
            room: self.room.clone(),
            cell: or(&self.cell, "Enter"),
            pad: or(&self.pad, "Spawn"),
        }
    }
}

/// `army-control-jump.json` (VibeSkua `ArmyJumpSettings`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct JumpSettings {
    /// The last destination jumped to (Map Travel's Go to fields start with it).
    pub map: String,
    pub room: String,
    pub cell: String,
    /// Turu's: the last pad jumped to (VibeSkua keeps no pad; it reads files without it).
    pub pad: String,
    /// The last player jumped to.
    pub player: String,
    /// The maps the clients visited, the newest first.
    pub history: Vec<Destination>,
    pub presets: Vec<Destination>,
}

impl Default for JumpSettings {
    fn default() -> Self {
        Self {
            map: String::new(),
            room: String::new(),
            cell: "Enter".into(),
            pad: "Spawn".into(),
            player: String::new(),
            history: Vec::new(),
            presets: Vec::new(),
        }
    }
}

impl JumpSettings {
    /// The settings in `path`; the defaults when there are none or they can't be read.
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let text = serde_json::to_string_pretty(self).map_err(io::Error::other)?;
        std::fs::write(path, text)
    }

    /// VibeSkua `RecordClientLocation`'s list part: the map and room go to the top of the
    /// history (an older entry for them leaves), which keeps [`HISTORY_LEN`].
    pub fn record_visit(&mut self, location: &Location) {
        self.history.retain(|d| {
            !(d.map.eq_ignore_ascii_case(&location.map)
                && d.room.eq_ignore_ascii_case(&location.room))
        });
        self.history.insert(
            0,
            Destination {
                name: location.username.clone(),
                map: location.map.clone(),
                room: location.room.clone(),
                cell: or(&location.cell, "Enter"),
                pad: or(&location.pad, "Spawn"),
            },
        );
        self.history.truncate(HISTORY_LEN);
    }

    /// "Clear History".
    pub fn clear_history(&mut self) {
        self.history.clear();
    }

    /// VibeSkua `JumpArmyToDestination`'s bookkeeping: remembers the destination for Map
    /// Travel's Go to fields.
    pub fn remember_jump(&mut self, destination: &Destination) {
        self.map = destination.map.trim().to_owned();
        self.room = destination.room.trim().to_owned();
        self.cell = destination.cell_or_default();
        self.pad = destination.pad_or_default();
    }

    /// The last destination jumped to, which Map Travel's Go to fields start with.
    pub fn last_jump(&self) -> Destination {
        Destination {
            map: self.map.clone(),
            room: self.room.clone(),
            cell: or(&self.cell, "Enter"),
            pad: or(&self.pad, "Spawn"),
            ..Default::default()
        }
    }

    /// What the "Jump to Map / Cell" box starts with: `map,room,cell`.
    pub fn manual_text(&self) -> String {
        format!("{},{},{}", self.map, self.room, self.cell)
    }

    /// The preset editor's "Save Preset": `editing` is the index of the preset being edited,
    /// `None` for a new one. Another preset with the same name (any case) is replaced. Returns
    /// the preset's index.
    pub fn save_preset(
        &mut self,
        editing: Option<usize>,
        mut preset: Destination,
    ) -> Result<usize, String> {
        preset.name = preset.name.trim().to_owned();
        preset.map = preset.map.trim().to_owned();
        if preset.name.is_empty() || preset.map.is_empty() {
            return Err("Enter both a preset name and map name.".into());
        }
        preset.room = preset.room.trim().to_owned();
        preset.cell = preset.cell_or_default();
        preset.pad = preset.pad_or_default();
        let editing = editing.filter(|&i| i < self.presets.len());
        // Other presets with that name leave; the edited one keeps its place.
        let mut at = None;
        let mut kept = Vec::with_capacity(self.presets.len() + 1);
        for (i, p) in std::mem::take(&mut self.presets).into_iter().enumerate() {
            if Some(i) == editing {
                at = Some(kept.len());
                kept.push(p);
            } else if !p.name.eq_ignore_ascii_case(&preset.name) {
                kept.push(p);
            }
        }
        self.presets = kept;
        match at {
            Some(at) => {
                self.presets[at] = preset;
                Ok(at)
            }
            None => {
                self.presets.push(preset);
                Ok(self.presets.len() - 1)
            }
        }
    }

    /// "Delete": removes the preset at `index` (nothing when there is none).
    pub fn delete_preset(&mut self, index: usize) {
        if index < self.presets.len() {
            self.presets.remove(index);
        }
    }
}

/// VibeSkua's "Jump to Map / Cell" box (`MenuItem_JumpMap_Click`): `map,room,cell`, the room
/// optional (`map,cell`, or `map` alone for "Enter"); Turu also takes a pad after the cell
/// (`map,room,cell,pad`). Map Travel's Map field reads what is pasted into it this way. `None`
/// without a map.
pub fn parse_manual(text: &str) -> Option<Destination> {
    let parts: Vec<&str> = text.split(',').map(str::trim).collect();
    let map = parts.first().copied().unwrap_or_default();
    if map.is_empty() {
        return None;
    }
    let (room, cell) = match parts.len() {
        0 | 1 => ("", "Enter"),
        2 => ("", parts[1]),
        _ => (parts[1], parts[2]),
    };
    Some(Destination {
        map: map.to_owned(),
        room: room.to_owned(),
        cell: or(cell, "Enter"),
        pad: or(parts.get(3).copied().unwrap_or_default(), "Spawn"),
        ..Default::default()
    })
}

/// VibeSkua `RecordClientLocation`: a client's location goes into the history only when its
/// map or room changed since that client's last one, so the history holds the maps the clients
/// travelled to, not where they stand.
#[derive(Debug, Default)]
pub struct Visits {
    /// Each client's last `map|room`, lowercase.
    last: HashMap<String, String>,
}

impl Visits {
    /// Records `location` for `client` (any key naming it). True when the history changed and
    /// should be saved.
    pub fn record(
        &mut self,
        settings: &mut JumpSettings,
        client: &str,
        location: &Location,
    ) -> bool {
        let signature = format!("{}|{}", location.map, location.room).to_lowercase();
        if self.last.get(client) == Some(&signature) {
            return false;
        }
        self.last.insert(client.to_owned(), signature);
        settings.record_visit(location);
        true
    }

    /// Forgets the clients that are gone.
    pub fn retain(&mut self, keep: impl Fn(&str) -> bool) {
        self.last.retain(|client, _| keep(client));
    }
}
