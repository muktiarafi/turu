//! The Player HUD's numbers (Turu: VibeSkua has no HUD): every player's HP and MP in your room,
//! kept from the packets that carry them, without polling the game:
//!
//! * `moveToArea`: the room you arrive in, everyone in its `uoBranch`;
//! * JSON `uotls` (`unm`, `o`): a player arriving (their whole entry), or their max HP and MP
//!   changing with their gear (`intHPMax`, `intHP`, ...);
//! * string `uotls` (`key:value,...`): a player moving to another cell, resting;
//! * `ct`: combat ticks, a `p` entry per player hit or healed;
//! * `exitArea`: a player leaving.
//!
//! The game is read ([`Vitals::refresh`], by the bot's timer) only when the packets can't say:
//! the HUD switched on in a room already entered, a packet about a player the table doesn't
//! know, an arrival without its max, or a figure that makes no sense (HP over its max). The UI
//! reads [`Vitals::snapshot`]. While the HUD is off nothing here runs past one flag check.

use crate::Bot;
use crate::error::BotResult;
use parking_lot::Mutex;
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};

/// Shows or hides the Player HUD (Application options' `showPlayerHud`, saved at once).
pub fn set_shown(bot: &Bot, on: bool) {
    bot.settings.write().show_player_hud = on;
    bot.save_settings();
    if !on {
        bot.vitals.clear();
    }
}

/// What [`Vitals::refresh`] reads of each player's `world.uoTree` entry.
const FIELDS: [&str; 8] = [
    "uoName",
    "strUsername",
    "intHP",
    "intHPMax",
    "intMP",
    "intMPMax",
    "intState",
    "strFrame",
];

/// One player's line in the HUD.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Vital {
    /// As the game shows it (`strUsername`, else `uoName`).
    pub name: String,
    pub hp: i64,
    /// 0 when the game didn't say.
    pub max_hp: i64,
    pub mp: i64,
    /// 0 when the game didn't say.
    pub max_mp: i64,
    /// You (the logged-in account).
    pub me: bool,
    /// `intState` 0, or no HP left.
    pub dead: bool,
}

/// The players in your cell, you first and the others by name.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RoomVitals {
    pub cell: String,
    pub players: Vec<Vital>,
}

#[derive(Debug, Clone, Default)]
struct Entry {
    name: String,
    hp: i64,
    max_hp: i64,
    mp: i64,
    max_mp: i64,
    cell: String,
    /// `intState`: 0 dead, 1 idle, 2 in combat; `None` when unknown.
    state: Option<i64>,
}

#[derive(Debug, Default)]
struct Table {
    /// Keyed by the lower-case name, as `world.uoTree` is.
    players: HashMap<String, Entry>,
    /// Your name, lower case.
    me: Option<String>,
}

#[derive(Debug, Default)]
pub struct Vitals {
    table: Mutex<Table>,
    /// The table holds a room: packets update it. Off (the HUD hidden, or no room yet), the
    /// packet handlers return at this check, without the lock.
    active: AtomicBool,
    /// The packets left something unknown: the timer reads the room once.
    stale: AtomicBool,
}

impl Vitals {
    /// Whether the timer should read the room: there is none yet, or the packets left a gap.
    pub fn refresh_due(&self) -> bool {
        !self.is_active() || self.stale.load(Ordering::Relaxed)
    }

    /// Whether the table holds a room (it was filled since the last [`Vitals::clear`]).
    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::Relaxed)
    }

    /// Reads the room's players from `world.uoTree` (Flash calls: never from the UI thread or a
    /// packet handler): the names in `world.areaUsers`, then each one's fields in one `getMany`.
    /// Not the whole tree at once: its entries reference one another, which JSON can't write.
    pub fn refresh(&self, bot: &Bot) -> BotResult<()> {
        let names: Vec<String> = bot.flash.get_opt("world.areaUsers")?.unwrap_or_default();
        let mut paths = Vec::with_capacity(names.len() * FIELDS.len());
        for (i, name) in names.iter().enumerate() {
            let key = name.to_lowercase();
            for field in FIELDS {
                paths.push((
                    format!("{i}|{field}"),
                    format!("world.uoTree.{key}.{field}"),
                ));
            }
        }
        let refs: Vec<(&str, &str)> = paths
            .iter()
            .map(|(k, p)| (k.as_str(), p.as_str()))
            .collect();
        let values = if refs.is_empty() {
            Default::default()
        } else {
            bot.flash.get_many(&refs)?
        };
        let mut tree = Map::new();
        for (i, name) in names.iter().enumerate() {
            let mut entry = Map::new();
            for field in FIELDS {
                if let Some(value) = values.get(&format!("{i}|{field}")).filter(|v| !v.is_null()) {
                    entry.insert(field.to_owned(), value.clone());
                }
            }
            // Gone from the tree (left as the list was read).
            if entry.is_empty() {
                continue;
            }
            entry
                .entry("uoName")
                .or_insert_with(|| Value::String(name.clone()));
            tree.insert(name.to_lowercase(), Value::Object(entry));
        }
        let me = my_name(bot)?;
        self.set_tree(Some(&Value::Object(tree)), Some(&me));
        Ok(())
    }

    /// Replaces the table with `world.uoTree`-shaped entries (`tree`, by lower-case name), `me`
    /// being your username.
    pub fn set_tree(&self, tree: Option<&Value>, me: Option<&str>) {
        let players = tree
            .and_then(Value::as_object)
            .map(|entries| {
                entries
                    .iter()
                    .filter_map(|(key, info)| {
                        Some((key.to_lowercase(), Entry::from(info.as_object()?, key)))
                    })
                    .collect()
            })
            .unwrap_or_default();
        let mut table = self.table.lock();
        table.players = players;
        table.me = me.map(str::to_lowercase);
        self.stale.store(false, Ordering::Relaxed);
        self.active.store(true, Ordering::Relaxed);
    }

    /// A `moveToArea` packet's `uoBranch` (only while the HUD is shown): the room you arrived
    /// in. An entry without its max, or a branch without you, asks for a read.
    pub fn on_room(&self, branch: &[Value], me: Option<&str>) {
        let mut players = HashMap::with_capacity(branch.len());
        for info in branch.iter().filter_map(Value::as_object) {
            let Some(key) = name_of(info) else { continue };
            players.insert(key.to_lowercase(), Entry::from(info, &key));
        }
        let me = me.map(str::to_lowercase);
        let gaps = me
            .as_ref()
            .and_then(|me| players.get(me))
            .is_none_or(|mine: &Entry| mine.cell.is_empty())
            || players.values().any(|e| e.max_hp <= 0);
        let mut table = self.table.lock();
        table.players = players;
        table.me = me;
        self.stale.store(gaps, Ordering::Relaxed);
        self.active.store(true, Ordering::Relaxed);
    }

    /// Forgets everyone (the HUD hidden, a logout); the next room fills it again.
    pub fn clear(&self) {
        self.active.store(false, Ordering::Relaxed);
        self.stale.store(false, Ordering::Relaxed);
        self.table.lock().players.clear();
    }

    /// A `ct` packet's `p` object: each player's changed `intHP`, `intMP`, `intState`, ...
    pub fn on_combat(&self, players: &Map<String, Value>) {
        if !self.is_active() {
            return;
        }
        let mut table = self.table.lock();
        let table = &mut *table;
        for (name, fields) in players {
            let Some(fields) = fields.as_object() else {
                continue;
            };
            let key = name.to_lowercase();
            let mine = table.me.as_deref() == Some(key.as_str());
            match table.players.get_mut(&key) {
                Some(entry) => self.apply(entry, fields, mine),
                // A player the table missed: no max or cell to show them with.
                None => self.stale.store(true, Ordering::Relaxed),
            }
        }
    }

    /// A string `uotls` packet: `name`'s `key:value,key:value` updates (`strFrame:Enter`,
    /// `intHP:120`, ...).
    pub fn on_update(&self, name: &str, updates: &str) {
        if !self.is_active() {
            return;
        }
        let mut table = self.table.lock();
        let Some(entry) = table.players.get_mut(&name.to_lowercase()) else {
            self.stale.store(true, Ordering::Relaxed);
            return;
        };
        for pair in updates.split(',') {
            if let Some((k, v)) = pair.split_once(':') {
                entry.set(k.trim(), &Value::String(v.trim().to_owned()));
            }
        }
        self.check(entry);
    }

    /// A JSON `uotls` packet: `name` (`unm`) and its object (`o`): a whole entry when they
    /// arrive, the changed fields after (`intHPMax`, `intHP`, ... when their gear changes).
    pub fn on_object(&self, name: &str, fields: &Map<String, Value>) {
        if !self.is_active() {
            return;
        }
        let key = name.to_lowercase();
        let mut table = self.table.lock();
        let table = &mut *table;
        let mine = table.me.as_deref() == Some(key.as_str());
        match table.players.get_mut(&key) {
            Some(entry) => self.apply(entry, fields, mine),
            // An arrival: their whole entry, max and cell included.
            None if fields.contains_key("intHPMax") && fields.contains_key("strFrame") => {
                let entry = Entry::from(fields, name);
                self.check(&entry);
                table.players.insert(key, entry);
            }
            None => self.stale.store(true, Ordering::Relaxed),
        }
    }

    /// A `moveToCell` you sent: your cell (the server tells only the others).
    pub fn on_my_cell(&self, cell: &str) {
        if !self.is_active() || cell.is_empty() {
            return;
        }
        let mut table = self.table.lock();
        let me = table.me.clone();
        match me.and_then(|me| table.players.get_mut(&me)) {
            Some(mine) => mine.cell = cell.to_owned(),
            None => self.stale.store(true, Ordering::Relaxed),
        }
    }

    /// An `exitArea` packet: `name` left the room.
    pub fn on_leave(&self, name: &str) {
        if !self.is_active() {
            return;
        }
        self.table.lock().players.remove(&name.to_lowercase());
    }

    /// `fields` into `entry` (yours when `mine`). You coming back to life: the client puts you
    /// in the spawn cell, which the server may not tell you, so the room is read once.
    fn apply(&self, entry: &mut Entry, fields: &Map<String, Value>, mine: bool) {
        let was_dead = entry.state == Some(0);
        for (k, v) in fields {
            entry.set(k, v);
        }
        if mine
            && was_dead
            && entry.state.is_some_and(|s| s != 0)
            && !fields.contains_key("strFrame")
        {
            self.stale.store(true, Ordering::Relaxed);
        }
        self.check(entry);
    }

    /// A figure that makes no sense (HP over a known max) means a packet was missed.
    fn check(&self, entry: &Entry) {
        if (entry.max_hp > 0 && entry.hp > entry.max_hp)
            || (entry.max_mp > 0 && entry.mp > entry.max_mp)
        {
            self.stale.store(true, Ordering::Relaxed);
        }
    }

    /// The players in your cell, you first, then by name; `None` before the room was filled
    /// or while you aren't in it.
    pub fn snapshot(&self) -> Option<RoomVitals> {
        if !self.is_active() {
            return None;
        }
        let table = self.table.lock();
        let me_key = table.me.as_deref()?;
        let me = table.players.get(me_key)?;
        let cell = me.cell.clone();
        let mut players: Vec<Vital> = table
            .players
            .iter()
            .filter(|(_, e)| e.cell.eq_ignore_ascii_case(&cell))
            .map(|(key, e)| Vital {
                name: e.name.clone(),
                hp: e.hp.max(0),
                max_hp: e.max_hp.max(0),
                mp: e.mp.max(0),
                max_mp: e.max_mp.max(0),
                me: key == me_key,
                dead: e.state == Some(0) || (e.max_hp > 0 && e.hp <= 0),
            })
            .collect();
        players.sort_by(|a, b| {
            b.me.cmp(&a.me)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        Some(RoomVitals { cell, players })
    }
}

/// Your username as recorded at login, else read from the game.
fn my_name(bot: &Bot) -> BotResult<String> {
    let recorded = bot.state.lock().username.clone();
    match recorded {
        Some(name) if !name.is_empty() => Ok(name),
        _ => crate::api::player::username(bot),
    }
}

/// A `uoBranch` entry's name: `uoName`, else `strUsername`.
fn name_of(info: &Map<String, Value>) -> Option<String> {
    ["uoName", "strUsername"]
        .iter()
        .filter_map(|k| info.get(*k).and_then(Value::as_str))
        .find(|n| !n.is_empty())
        .map(str::to_owned)
}

impl Entry {
    /// An entry from a `world.uoTree`-shaped object; `fallback` names it when it has no name.
    fn from(info: &Map<String, Value>, fallback: &str) -> Self {
        let mut entry = Entry::default();
        for (k, v) in info {
            entry.set(k, v);
        }
        if entry.name.is_empty() {
            entry.name = fallback.to_owned();
        }
        entry
    }

    fn set(&mut self, key: &str, value: &Value) {
        let num = || {
            value
                .as_f64()
                .or_else(|| value.as_str().and_then(|s| s.parse().ok()))
                .map(|n| n as i64)
        };
        let text = || value.as_str().filter(|s| !s.is_empty());
        match key {
            "intHP" => self.hp = num().unwrap_or(self.hp),
            "intHPMax" => self.max_hp = num().unwrap_or(self.max_hp),
            "intMP" => self.mp = num().unwrap_or(self.mp),
            "intMPMax" => self.max_mp = num().unwrap_or(self.max_mp),
            "intState" => self.state = num().or(self.state),
            "strFrame" => {
                if let Some(cell) = text() {
                    self.cell = cell.to_owned();
                }
            }
            // The name as the game shows it ("Questwalker"), over `uoName`'s lower case.
            "strUsername" => {
                if let Some(name) = text() {
                    self.name = name.to_owned();
                }
            }
            "uoName" => {
                if let Some(name) = text().filter(|_| self.name.is_empty()) {
                    self.name = name.to_owned();
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn tree() -> Value {
        json!({
            "questwalker": {"uoName": "questwalker", "strUsername": "Questwalker", "intHP": 2400,
                "intHPMax": 3000, "intMP": "60", "intMPMax": 100, "intState": 1, "strFrame": "Enter"},
            "moonfarer": {"uoName": "Moonfarer", "intHP": 1900, "intHPMax": 2500,
                "intMP": 88, "intMPMax": 100, "intState": 2, "strFrame": "Enter"},
            "astra": {"uoName": "Astra", "intHP": 100, "intHPMax": 100, "intMP": 10,
                "intMPMax": 100, "intState": 1, "strFrame": "Boss"},
        })
    }

    fn object(value: Value) -> Map<String, Value> {
        value.as_object().unwrap().clone()
    }

    #[test]
    fn lists_your_cell_with_you_first() {
        let vitals = Vitals::default();
        assert_eq!(vitals.snapshot(), None);
        assert!(vitals.refresh_due());
        vitals.set_tree(Some(&tree()), Some("QuestWalker"));
        assert!(!vitals.refresh_due());
        let room = vitals.snapshot().unwrap();
        assert_eq!(room.cell, "Enter");
        let names: Vec<_> = room.players.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["Questwalker", "Moonfarer"]);
        assert!(room.players[0].me);
        assert_eq!((room.players[0].mp, room.players[0].max_mp), (60, 100));
    }

    #[test]
    fn packets_update_hp_mp_and_cells() {
        let vitals = Vitals::default();
        vitals.set_tree(Some(&tree()), Some("questwalker"));
        let ct = json!({"Moonfarer": {"intHP": 0, "intMP": 70, "intState": 0}});
        vitals.on_combat(ct.as_object().unwrap());
        let moon = vitals.snapshot().unwrap().players[1].clone();
        assert_eq!((moon.hp, moon.mp, moon.dead), (0, 70, true));
        // Astra walks into your cell.
        vitals.on_update("Astra", "sp:8,tx:181,ty:358,strFrame:Enter");
        vitals.on_update("astra", "intHP:40,intMP:5");
        let room = vitals.snapshot().unwrap();
        let astra = room.players.iter().find(|p| p.name == "Astra").unwrap();
        assert_eq!((astra.hp, astra.mp), (40, 5));
        assert!(!vitals.refresh_due());
        vitals.clear();
        assert_eq!(vitals.snapshot(), None);
    }

    #[test]
    fn arrivals_gear_and_departures_come_from_packets() {
        let vitals = Vitals::default();
        vitals.set_tree(Some(&tree()), Some("questwalker"));
        // As the server sends them (names made up).
        vitals.on_object(
            "lumenward",
            &object(
                json!({"uoName": "lumenward", "strUsername": "Lumenward", "entType": "p",
                "intLevel": 100, "intState": 1, "intHP": 2250, "intMP": 100, "intHPMax": 2250,
                "intMPMax": 100, "strFrame": "Enter", "strPad": "Spawn"}),
            ),
        );
        vitals.on_object(
            "lumenward",
            &object(json!({"intHPMax": 2935, "intMPMax": 100, "intHP": 2935, "intMP": 100})),
        );
        let room = vitals.snapshot().unwrap();
        let lumen = room.players.iter().find(|p| p.name == "Lumenward").unwrap();
        assert_eq!((lumen.hp, lumen.max_hp), (2935, 2935));
        assert!(!vitals.refresh_due());
        vitals.on_leave("lumenward");
        assert!(
            vitals
                .snapshot()
                .unwrap()
                .players
                .iter()
                .all(|p| p.name != "Lumenward")
        );
        assert!(!vitals.refresh_due());
    }

    #[test]
    fn gaps_ask_for_a_read() {
        let vitals = Vitals::default();
        vitals.set_tree(Some(&tree()), Some("questwalker"));
        vitals.on_combat(json!({"Nobody": {"intHP": 5}}).as_object().unwrap());
        assert!(vitals.refresh_due());

        vitals.set_tree(Some(&tree()), Some("questwalker"));
        vitals.on_object("nobody", &object(json!({"intHP": 5})));
        assert!(vitals.refresh_due());

        vitals.set_tree(Some(&tree()), Some("questwalker"));
        vitals.on_update("moonfarer", "intHP:9999");
        assert!(vitals.refresh_due(), "HP over its max");
    }

    #[test]
    fn a_room_comes_from_its_branch() {
        let vitals = Vitals::default();
        let branch = json!([
            {"uoName": "questwalker", "strUsername": "Questwalker", "intHP": 10, "intHPMax": 20,
                "intMP": 5, "intMPMax": 10, "intState": 1, "strFrame": "Enter"},
            {"uoName": "moonfarer", "intHP": 10, "intHPMax": 20, "strFrame": "Enter"},
        ]);
        vitals.on_room(branch.as_array().unwrap(), Some("Questwalker"));
        assert!(!vitals.refresh_due());
        let names: Vec<_> = vitals
            .snapshot()
            .unwrap()
            .players
            .into_iter()
            .map(|p| p.name)
            .collect();
        assert_eq!(names, ["Questwalker", "moonfarer"]);
        // Without a max, or without you: read once.
        let thin = json!([{"uoName": "questwalker", "intHP": 10, "strFrame": "Enter"}]);
        vitals.on_room(thin.as_array().unwrap(), Some("questwalker"));
        assert!(vitals.refresh_due());
        vitals.on_room(branch.as_array().unwrap(), Some("astra"));
        assert!(vitals.refresh_due());
    }

    #[test]
    fn cells_follow_your_moves_and_theirs() {
        let vitals = Vitals::default();
        vitals.set_tree(Some(&tree()), Some("questwalker"));
        // You go to Boss (only your own moveToCell says so): Astra is there, the others aren't.
        vitals.on_my_cell("Boss");
        let names = |v: &Vitals| -> Vec<String> {
            v.snapshot()
                .unwrap()
                .players
                .into_iter()
                .map(|p| p.name)
                .collect()
        };
        assert_eq!(names(&vitals), ["Questwalker", "Astra"]);
        // Moonfarer follows, as the server tells it: a cell change, then walking in it.
        vitals.on_update(
            "moonfarer",
            "strFrame:Boss,strPad:Left,px:482,py:444,mvts:-1,mvtd:0,tx:0,ty:0,bResting:false",
        );
        vitals.on_update(
            "moonfarer",
            "tx:615,ty:447,sp:10,strFrame:Boss,px:482,py:444,mvts:1795576337102,mvtd:578",
        );
        assert_eq!(names(&vitals), ["Questwalker", "Astra", "Moonfarer"]);
        // Moonfarer dies and comes back at the spawn (JSON uotls).
        vitals.on_object(
            "moonfarer",
            &object(json!({"intState": 1, "intHP": 2500, "intMP": 100, "strFrame": "Enter", "strPad": "Spawn"})),
        );
        assert_eq!(names(&vitals), ["Questwalker", "Astra"]);
        assert!(!vitals.refresh_due());
    }

    #[test]
    fn coming_back_to_life_reads_your_cell() {
        let vitals = Vitals::default();
        vitals.set_tree(Some(&tree()), Some("questwalker"));
        let ct = |fields: Value| json!({ "questwalker": fields });
        vitals.on_combat(ct(json!({"intState": 0, "intHP": 0})).as_object().unwrap());
        assert!(!vitals.refresh_due());
        vitals.on_combat(
            ct(json!({"intState": 1, "intHP": 3000}))
                .as_object()
                .unwrap(),
        );
        assert!(vitals.refresh_due(), "no cell came with it");

        vitals.set_tree(Some(&tree()), Some("questwalker"));
        vitals.on_combat(ct(json!({"intState": 0, "intHP": 0})).as_object().unwrap());
        vitals.on_object(
            "questwalker",
            &object(json!({"intState": 1, "intHP": 3000, "strFrame": "Enter"})),
        );
        assert!(!vitals.refresh_due(), "the packet said where");
    }

    #[test]
    fn your_death_and_respawn_as_the_server_sends_them() {
        let vitals = Vitals::default();
        vitals.set_tree(Some(&tree()), Some("questwalker"));
        vitals.on_my_cell("Boss");
        let me = |v: &Vitals| {
            v.snapshot()
                .unwrap()
                .players
                .into_iter()
                .find(|p| p.me)
                .unwrap()
        };
        let ct = |fields: Value| json!({ "questwalker": fields });
        // The killing blow: HP to 0 (with the max), then the state.
        vitals.on_object(
            "questwalker",
            &object(json!({"intHPMax": 2155, "intMPMax": 100, "intHP": 0, "intMP": 100})),
        );
        assert!(me(&vitals).dead);
        vitals.on_combat(
            ct(json!({"intState": 0, "intHP": 0, "intMP": 100}))
                .as_object()
                .unwrap(),
        );
        assert!(me(&vitals).dead);
        // Back at the spawn, the cell in the same packet: no read needed.
        vitals.on_object(
            "questwalker",
            &object(
                json!({"intState": 1, "intHP": 2155, "intMP": 100, "intSP": 100,
                "strFrame": "Enter", "strPad": "Spawn"}),
            ),
        );
        let alive = me(&vitals);
        assert!(!alive.dead);
        assert_eq!((alive.hp, alive.max_hp), (2155, 2155));
        assert_eq!(vitals.snapshot().unwrap().cell, "Enter");
        assert!(!vitals.refresh_due());
        // The client walks you back: moveToCell Enter, then Boss.
        vitals.on_my_cell("Enter");
        vitals.on_my_cell("Boss");
        assert_eq!(vitals.snapshot().unwrap().cell, "Boss");
        vitals.on_combat(
            ct(json!({"intHP": 2155, "intMP": 100, "intSP": 75}))
                .as_object()
                .unwrap(),
        );
        assert!(!me(&vitals).dead);
        assert!(!vitals.refresh_due());
    }

    #[test]
    fn another_players_return_death_and_respawn() {
        let vitals = Vitals::default();
        vitals.set_tree(Some(&tree()), Some("questwalker"));
        vitals.on_my_cell("Boss");
        let names = |v: &Vitals| -> Vec<String> {
            v.snapshot()
                .unwrap()
                .players
                .into_iter()
                .map(|p| p.name)
                .collect()
        };
        let moon = |v: &Vitals| {
            v.snapshot()
                .unwrap()
                .players
                .into_iter()
                .find(|p| p.name == "Moonfarer")
        };
        // They leave the room and come back (the whole entry), at the spawn.
        vitals.on_leave("moonfarer");
        vitals.on_object(
            "moonfarer",
            &object(
                json!({"uoName": "moonfarer", "strUsername": "Moonfarer", "entType": "p",
                "intLevel": 100, "intState": 1, "intHP": 2250, "intMP": 100, "intSP": 100,
                "intHPMax": 2250, "intMPMax": 100, "strFrame": "Enter", "strPad": "Spawn"}),
            ),
        );
        assert_eq!(names(&vitals), ["Questwalker", "Astra"]);
        // They walk to your cell and fight.
        vitals.on_update(
            "moonfarer",
            "strFrame:Boss,strPad:Left,px:482,py:444,mvts:-1,mvtd:0,tx:0,ty:0,bResting:false",
        );
        vitals.on_combat(
            json!({"moonfarer": {"intState": 2, "intMP": 100}})
                .as_object()
                .unwrap(),
        );
        vitals.on_object(
            "moonfarer",
            &object(json!({"intHPMax": 2250, "intMPMax": 100, "intHP": 1519, "intMP": 100})),
        );
        vitals.on_combat(json!({"moonfarer": {"intHP": 741}}).as_object().unwrap());
        assert_eq!(moon(&vitals).map(|m| (m.hp, m.dead)), Some((741, false)));
        // They die.
        vitals.on_object(
            "moonfarer",
            &object(json!({"intHPMax": 2250, "intMPMax": 100, "intHP": 0, "intMP": 100})),
        );
        vitals.on_combat(
            json!({"moonfarer": {"intState": 0, "intHP": 0, "intMP": 100}})
                .as_object()
                .unwrap(),
        );
        assert_eq!(moon(&vitals).map(|m| m.dead), Some(true));
        // They come back at the spawn: alive, and out of your cell.
        vitals.on_object(
            "moonfarer",
            &object(
                json!({"intState": 1, "intHP": 2250, "intMP": 100, "intSP": 100,
                "strFrame": "Enter", "strPad": "Spawn"}),
            ),
        );
        assert_eq!(names(&vitals), ["Questwalker", "Astra"]);
        vitals.on_my_cell("Enter");
        let back = moon(&vitals).unwrap();
        assert_eq!((back.hp, back.dead), (2250, false));
        assert!(!vitals.refresh_due());
    }

    #[test]
    fn off_it_ignores_packets() {
        let vitals = Vitals::default();
        vitals.on_object(
            "questwalker",
            &object(json!({"intHP": 1, "intHPMax": 2, "strFrame": "Enter"})),
        );
        vitals.on_leave("questwalker");
        vitals.on_combat(json!({"questwalker": {"intHP": 1}}).as_object().unwrap());
        assert_eq!(vitals.snapshot(), None);
        assert!(vitals.table.lock().players.is_empty());
    }
}
