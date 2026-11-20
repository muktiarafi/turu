//! Tools & Helpers > Fast Travel (VibeSkua `FastTravelView` / `FastTravelViewModel`): saved
//! destinations (name, map, cell, pad) to travel to, VibeSkua's private room, Get current, and
//! adding, editing, removing and clearing them, every change saved (`FastTravels.json`). One
//! [`State`] per tab, shared by the egui window and a native page: the list, the travel in flight
//! and its outcome, Get current's answer and the private room. The search, the form and which
//! destination it edits stay each shell's.
//!
//! Actions name a destination by its place and its name together, so a list changed elsewhere
//! (the other shell, another tab) is refused rather than acting on the wrong one.

use super::scheduler::{Note, Tint};
use super::tab_state::{PerTab, lock};
use serde::Serialize;
use serde_json::Value as Json;
use std::sync::Arc;
use turu_bot::Bot;
use turu_bot::api::{map, player};
use turu_bot::settings::FastTravel;

/// VibeSkua `MapService.Pads`.
pub const PADS: [&str; 8] = ["Spawn", "Center", "Left", "Right", "Up", "Down", "Top", "Bottom"];
/// VibeSkua `MapService.PrivateRoomNumber`'s default.
pub const DEFAULT_ROOM: i64 = 111111;
pub const MAX_ROOM: i64 = 999_999_999;

/// Get current's answer: where the character is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct HereView {
    pub map: String,
    pub cell: String,
    pub pad: String,
}

pub struct State {
    pub list: Vec<FastTravel>,
    loaded: bool,
    /// The destination being travelled to.
    pub traveling: Option<FastTravel>,
    /// How the last travel (or Get current) went.
    pub note: Option<Note>,
    /// Get current's answer, and a count that changes with each: a shell's form takes it once.
    pub here: Option<HereView>,
    pub here_count: u32,
    /// VibeSkua `UsePrivateRoom` and `PrivateRoomNumber`: travels join `map-<room>`.
    pub private_room: bool,
    pub room: i64,
}

impl Default for State {
    fn default() -> Self {
        Self {
            list: Vec::new(),
            loaded: false,
            traveling: None,
            note: None,
            here: None,
            here_count: 0,
            private_room: true,
            room: DEFAULT_ROOM,
        }
    }
}

pub static STATE: PerTab<State> = PerTab::new();

fn note(text: impl Into<String>, tint: Tint) -> Option<Note> {
    Some(Note {
        text: text.into(),
        tint,
    })
}

/// VibeSkua's new destination: no name, `battleon`, `Enter`, `Spawn`.
pub fn empty_form() -> FastTravel {
    FastTravel::new("", "battleon", "Enter", "Spawn")
}

impl State {
    /// The saved list, read again (the window or page opened: another tab may have changed it).
    /// A file that can't be read is kept beside it as `.bad`, and the defaults take its place.
    pub fn load(&mut self, bot: &Bot) {
        let path = bot.paths.fast_travels_file();
        self.loaded = true;
        self.list = match FastTravel::load(&path) {
            Ok(list) => list,
            Err(e) => {
                let bad = path.with_extension("json.bad");
                let _ = std::fs::rename(&path, &bad);
                self.note = note(
                    format!("Couldn't read {} ({e}): kept as {}", path.display(), bad.display()),
                    Tint::Danger,
                );
                FastTravel::defaults()
            }
        };
    }

    pub fn ensure_loaded(&mut self, bot: &Bot) {
        if !self.loaded {
            self.load(bot);
        }
    }

    /// VibeSkua `SaveFastTravels`, after every change.
    fn save(&mut self, bot: &Bot) {
        let path = bot.paths.fast_travels_file();
        if let Err(e) = FastTravel::save(&path, &self.list) {
            self.note = note(format!("Couldn't save {}: {e}", path.display()), Tint::Danger);
        }
    }

    /// The destination at `index` if it's still called `name`.
    fn at(&self, index: usize, name: &str) -> Result<usize, String> {
        match self.list.get(index) {
            Some(d) if d.name == name => Ok(index),
            _ => Err(format!("{name} moved or went: the list changed")),
        }
    }

    /// VibeSkua `AddFastTravel`, or the edit dialog's OK (`at`: the place it edits).
    pub fn put(&mut self, bot: &Bot, dest: &FastTravel, at: Option<usize>) -> Result<(), String> {
        if !dest.is_valid() {
            return Err("Fill in the name, map, cell and pad".into());
        }
        let dest = dest.trimmed();
        match at.filter(|&i| i < self.list.len()) {
            Some(i) => self.list[i] = dest,
            None => self.list.push(dest),
        }
        self.save(bot);
        Ok(())
    }

    pub fn remove(&mut self, bot: &Bot, index: usize) {
        if index < self.list.len() {
            self.list.remove(index);
            self.save(bot);
        }
    }

    /// VibeSkua `ClearFastTravels`.
    pub fn clear(&mut self, bot: &Bot) {
        self.list.clear();
        self.save(bot);
    }

    pub fn set_private(&mut self, on: bool, room: i64) {
        self.private_room = on;
        self.room = room.clamp(1, MAX_ROOM);
    }
}

/// VibeSkua `MapService.Travel`, on a bot thread, saying how it went.
pub fn travel(bot: &Arc<Bot>, dest: FastTravel) {
    let state = STATE.of(bot);
    let room = {
        let mut s = lock(&state);
        if s.traveling.is_some() {
            return;
        }
        s.traveling = Some(dest.clone());
        s.note = note(format!("Traveling to {}...", dest.name), Tint::Muted);
        s.private_room.then_some(s.room)
    };
    crate::turu::spawn_action_for(bot.clone(), "Fast Travel", move |bot| {
        let outcome = travel_outcome(bot, &dest, room);
        let mut s = lock(&state);
        s.traveling = None;
        s.note = Some(match outcome {
            Ok(outcome) => outcome,
            Err(e) => Note {
                text: format!("Travel failed: {e}"),
                tint: Tint::Danger,
            },
        });
        Ok(())
    });
}

/// Travels to `dest` and says where that left you.
fn travel_outcome(bot: &Bot, dest: &FastTravel, room: Option<i64>) -> turu_bot::BotResult<Note> {
    let say = |text: String, tint| Ok(Note { text, tint });
    if !player::logged_in(bot)? {
        return say("Not logged in".to_owned(), Tint::Danger);
    }
    if !player::alive(bot)? {
        return say("You're dead: travel once you respawn".to_owned(), Tint::Warning);
    }
    map::fast_travel(bot, &dest.map, &dest.cell, &dest.pad, room)?;
    let (now, cell) = (map::name(bot)?, player::cell(bot)?);
    if now.eq_ignore_ascii_case(&dest.map) && cell.eq_ignore_ascii_case(&dest.cell) {
        say(format!("At {}", dest.name), Tint::Success)
    } else {
        say(
            format!("Didn't reach {} \u{B7} {}: in {now} \u{B7} {cell}", dest.map, dest.cell),
            Tint::Warning,
        )
    }
}

/// VibeSkua `GetCurrent`: the map, cell and pad you are in, for a shell's form.
pub fn get_current(bot: &Arc<Bot>) {
    let state = STATE.of(bot);
    crate::turu::spawn_action_for(bot.clone(), "Fast Travel current", move |bot| {
        let here = if player::logged_in(bot)? && player::world_loaded(bot)? {
            Some(HereView {
                map: map::name(bot)?,
                cell: player::cell(bot)?,
                pad: player::pad(bot)?,
            })
        } else {
            None
        };
        let mut s = lock(&state);
        match here {
            Some(here) if !here.map.is_empty() => {
                s.here = Some(here);
                s.here_count = s.here_count.wrapping_add(1);
            }
            _ => s.note = note("Not in a map: log in first", Tint::Danger),
        }
        Ok(())
    });
}

// --- the view a shell reads -------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct FastTravelView {
    pub destinations: Vec<DestinationView>,
    pub note: Option<Note>,
    pub private_room: bool,
    pub room: i64,
    pub here: Option<HereView>,
    pub here_count: u32,
    /// The pads a destination can name.
    pub pads: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct DestinationView {
    /// Its place in the list (the actions' `index`).
    pub position: u32,
    pub name: String,
    pub map: String,
    pub cell: String,
    pub pad: String,
    /// Being travelled to now.
    pub traveling: bool,
}

impl State {
    pub fn view(&self) -> FastTravelView {
        FastTravelView {
            destinations: self
                .list
                .iter()
                .enumerate()
                .map(|(i, d)| DestinationView {
                    position: i as u32,
                    name: d.name.clone(),
                    map: d.map.clone(),
                    cell: d.cell.clone(),
                    pad: d.pad.clone(),
                    traveling: self.traveling.as_ref() == Some(d),
                })
                .collect(),
            note: self.note.clone(),
            private_room: self.private_room,
            room: self.room,
            here: self.here.clone(),
            here_count: self.here_count,
            pads: PADS.iter().map(|p| (*p).to_owned()).collect(),
        }
    }
}

pub fn view(bot: &Arc<Bot>) -> FastTravelView {
    let state = STATE.of(bot);
    let mut s = lock(&state);
    s.ensure_loaded(bot);
    s.view()
}

fn destination(body: &Json) -> Result<FastTravel, String> {
    let text = |name: &str| {
        body.get(name)
            .and_then(Json::as_str)
            .map(str::to_owned)
            .ok_or_else(|| format!("{name} is missing"))
    };
    Ok(FastTravel::new(
        &text("name")?,
        &text("map")?,
        &text("cell")?,
        &text("pad")?,
    ))
}

/// `open` (read the list again), `travel` (`index`, `name`), `add` (`name`, `map`, `cell`,
/// `pad`), `edit` (`index`, `was`: its name before, and the four fields), `remove` (`index`,
/// `name`), `clear`, `get-current`, `private` (`on`, `room`), `dismiss` (the note).
pub fn act(bot: &Arc<Bot>, action: &str, body: &Json) -> Result<(), String> {
    let index = || {
        body.get("index")
            .and_then(Json::as_u64)
            .map(|i| i as usize)
            .ok_or_else(|| "index is missing".to_owned())
    };
    let named = |key: &str| {
        body.get(key)
            .and_then(Json::as_str)
            .map(str::to_owned)
            .ok_or_else(|| format!("{key} is missing"))
    };
    let state = STATE.of(bot);
    match action {
        "open" => lock(&state).load(bot),
        "travel" => {
            let dest = {
                let s = lock(&state);
                let i = s.at(index()?, &named("name")?)?;
                s.list[i].clone()
            };
            travel(bot, dest);
        }
        "add" => lock(&state).put(bot, &destination(body)?, None)?,
        "edit" => {
            let mut s = lock(&state);
            let i = s.at(index()?, &named("was")?)?;
            s.put(bot, &destination(body)?, Some(i))?;
        }
        "remove" => {
            let mut s = lock(&state);
            let i = s.at(index()?, &named("name")?)?;
            s.remove(bot, i);
        }
        "clear" => lock(&state).clear(bot),
        "get-current" => get_current(bot),
        "private" => {
            let mut s = lock(&state);
            let on = body.get("on").and_then(Json::as_bool).unwrap_or(s.private_room);
            let room = body.get("room").and_then(Json::as_i64).unwrap_or(s.room);
            s.set_private(on, room);
        }
        "dismiss" => lock(&state).note = None,
        other => return Err(format!("no action {other}")),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_a_destination_that_moved() {
        let s = State {
            list: vec![
                FastTravel::new("Quest hub", "battleon", "Enter", "Spawn"),
                FastTravel::new("Questwalker's farm", "farm", "r2", "Left"),
            ],
            ..Default::default()
        };
        assert_eq!(s.at(1, "Questwalker's farm"), Ok(1));
        assert!(s.at(0, "Questwalker's farm").is_err());
        assert!(s.at(5, "Quest hub").is_err());
        let view = s.view();
        assert_eq!(view.destinations[1].position, 1);
        assert_eq!(view.pads.len(), 8);
        assert!(view.private_room && view.room == DEFAULT_ROOM);
    }

    #[test]
    fn rooms_stay_in_range() {
        let mut s = State::default();
        s.set_private(false, 0);
        assert_eq!((s.private_room, s.room), (false, 1));
        s.set_private(true, i64::MAX);
        assert_eq!(s.room, MAX_ROOM);
    }
}
