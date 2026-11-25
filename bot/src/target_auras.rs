//! The Target Auras HUD's table (Turu: VibeSkua has none): the auras on the monsters in your
//! room, who put each one there, its stacks and the time it has left, kept from the packets
//! without polling the game:
//!
//! * `ct`'s `a` list (and the same entries sent alone): `aura+` / `aura++` put an aura on
//!   `tInf` (`m:<map id>`), cast by `cInf` (`p:<entID>` a player, `m:<map id>` a monster), with
//!   `nam`, `dur` (seconds), `stk` (stacks, missing for one); `aura-` / `aura--` take one off by
//!   name (no caster); `aura*` takes every one off (a boss's cleanse);
//! * `ct`'s `m`: a monster's HP, its death (`intHP` 0 or `intState` 0);
//! * `mtls`: a monster's HP and max, its respawn;
//! * `moveToArea`: a new room, its players' `entID`s from `uoBranch`;
//! * the `gar` you send: the monster you attack is the one the HUD shows.
//!
//! The server keeps one aura of a name per monster: a second caster's `aura+` replaces the
//! first's, so the HUD names the latest caster. It sends no time stamp, so the time left counts
//! from when the packet came. Checked against a capture of an Ultra fight (2026-11-25).
//!
//! The game is read ([`TargetAuras::refresh`], by the bot's timer) only for what the packets
//! can't say: a caster's name (a `p:<entID>` not in the room's `uoBranch`), the monster's name,
//! and, once per monster, the auras it had before the HUD saw it (their caster unknown).

use crate::Bot;
use crate::error::BotResult;
use crate::models::Aura;
use parking_lot::Mutex;
use serde_json::{Map, Value};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// Auras shown in a line of their own above the casters', named for what puts them on: a party
/// scroll the whole fight depends on. Scroll of Enrage puts Focus (6 s) and Reckless (10 s) on
/// the monster at once; its name is in no packet.
pub const PINS: &[Pin] = &[Pin {
    label: "Enrage",
    auras: &["Focus", "Reckless"],
}];

/// A [`PINS`] line.
#[derive(Debug, Clone, Copy)]
pub struct Pin {
    pub label: &'static str,
    pub auras: &'static [&'static str],
}

/// An aura lasting this long or longer has no countdown (boss mechanics come with 7200 s).
const PERMANENT: f64 = 3600.0;
/// A timed aura is dropped this long after its time ran out if no `aura-` came.
const GRACE: Duration = Duration::from_millis(750);

/// Shows or hides the Target Auras HUD (Application options' `showTargetAuras`, saved at once).
pub fn set_shown(bot: &Bot, on: bool) {
    bot.settings.write().show_target_auras = on;
    bot.save_settings();
    if on {
        bot.target_auras.start();
    } else {
        bot.target_auras.clear();
    }
}

/// Who put an aura on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Caster {
    /// A player's `entID`.
    Player(i64),
    /// A monster's map ID (a boss's own mechanics).
    Monster(i64),
    /// Read from the game, which doesn't say.
    Unknown,
}

impl Caster {
    /// `p:<entID>`, `m:<map id>`.
    fn parse(value: Option<&Value>) -> Self {
        match value.and_then(Value::as_str).and_then(id_ref) {
            Some(('p', id)) => Self::Player(id),
            Some(('m', id)) => Self::Monster(id),
            _ => Self::Unknown,
        }
    }
}

/// `p:70177` -> `('p', 70177)`.
fn id_ref(value: &str) -> Option<(char, i64)> {
    let (kind, id) = value.split_once(':')?;
    let kind = kind.chars().next()?;
    Some((kind, id.trim().parse().ok()?))
}

#[derive(Debug, Clone)]
struct Live {
    name: String,
    caster: Caster,
    stacks: i64,
    /// Seconds, as the packet said; 0 or [`PERMANENT`] and over: no countdown.
    duration: f64,
    since: Instant,
    /// When it was first put on, to keep its place among the others.
    order: u64,
    /// Its icon's name in the game ([`icon_link`]).
    icon: Option<String>,
}

impl Live {
    fn ends(&self) -> Option<Instant> {
        (self.duration > 0.0 && self.duration < PERMANENT)
            .then(|| self.since + Duration::from_secs_f64(self.duration))
    }
}

#[derive(Debug, Default)]
struct Monster {
    /// `strMonName`; empty until read.
    name: String,
    hp: Option<i64>,
    max_hp: i64,
    auras: Vec<Live>,
    /// When each aura last came off, and whether a cleanse (`aura*`) took it.
    gone: HashMap<String, (Instant, bool)>,
    /// Its auras were read from the game once (those put on before the HUD saw it).
    seeded: bool,
}

impl Monster {
    /// Drops the timed auras whose time ran out (no `aura-` came).
    fn expire(&mut self, now: Instant) {
        let gone = &mut self.gone;
        self.auras.retain(|aura| match aura.ends() {
            Some(end) if end + GRACE <= now => {
                gone.insert(aura.name.to_lowercase(), (end, false));
                false
            }
            _ => true,
        });
    }

    fn remove(&mut self, name: &str, at: Instant, cleansed: bool) {
        let key = name.to_lowercase();
        self.auras.retain(|a| a.name.to_lowercase() != key);
        self.gone.insert(key, (at, cleansed));
    }
}

#[derive(Debug, Default)]
struct Table {
    /// By map ID.
    monsters: HashMap<i64, Monster>,
    /// Players' names by `entID`.
    players: HashMap<i64, String>,
    /// The monster you last attacked.
    target: Option<i64>,
    /// Casters not in `players`: the timer reads the room for them.
    pending: HashSet<i64>,
    /// Casters a read couldn't name either (gone from the room): not asked for again.
    tried: HashSet<i64>,
    next_order: u64,
}

/// The auras on your target, for the HUD.
#[derive(Debug, Clone, PartialEq)]
pub struct TargetView {
    pub monster: String,
    pub hp: Option<i64>,
    /// 0 when not known.
    pub max_hp: i64,
    pub pins: Vec<PinView>,
    /// You first, the other players by name, casters not known, then monsters (the boss's own).
    pub casters: Vec<CasterAuras>,
}

/// A [`PINS`] line: its auras, each with its time left, and who last cast them.
#[derive(Debug, Clone, PartialEq)]
pub struct PinView {
    pub label: &'static str,
    pub caster: Option<String>,
    pub auras: Vec<(&'static str, Option<AuraView>)>,
    pub state: PinState,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PinState {
    /// At least one of its auras is on.
    Up,
    /// None is on; the last came off `ago` (a cleanse when `cleansed`).
    Down { ago: Duration, cleansed: bool },
    /// None was seen on this monster.
    Never,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CasterKind {
    Me,
    Player,
    Unknown,
    Monster,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CasterAuras {
    pub name: String,
    pub kind: CasterKind,
    pub auras: Vec<AuraView>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AuraView {
    pub name: String,
    pub stacks: i64,
    /// Seconds it lasts; `None` without a countdown.
    pub duration: Option<f32>,
    /// Seconds left.
    pub left: Option<f32>,
    /// Its icon's name in the game, for [`TargetAuras::icon`].
    pub icon: Option<String>,
}

impl AuraView {
    fn of(aura: &Live, now: Instant) -> Self {
        let left = aura
            .ends()
            .map(|end| end.saturating_duration_since(now).as_secs_f32());
        Self {
            name: aura.name.clone(),
            stacks: aura.stacks,
            duration: left.map(|_| aura.duration as f32),
            left,
            icon: aura.icon.clone(),
        }
    }
}

/// An aura's icon as the game draws it: `size` pixels square, RGBA (not premultiplied).
#[derive(Debug, PartialEq, Eq)]
pub struct AuraIcon {
    pub size: usize,
    pub rgba: Vec<u8>,
}

#[derive(Debug)]
enum IconState {
    /// Shown on the HUD, not drawn yet.
    Wanted,
    Ready(Arc<AuraIcon>),
    /// The game couldn't draw it (its class file not loaded yet): tried again a few times.
    Missing {
        at: Instant,
        tries: u8,
    },
}

/// The icons' size in pixels: 16 points on a screen of two pixels a point.
const ICON_PX: u32 = 32;
/// The icon the game's aura HUD shows for one it can't find.
pub const FALLBACK_ICON: &str = "isp2";
const ICON_TRIES: u8 = 3;
const ICON_RETRY: Duration = Duration::from_secs(5);
/// Icons drawn per read of the game, so one read stays short.
const ICONS_PER_READ: usize = 8;

#[derive(Debug, Default)]
pub struct TargetAuras {
    table: Mutex<Table>,
    /// By the game's icon name; kept across rooms.
    icons: Mutex<HashMap<String, IconState>>,
    /// Following the packets (the HUD shown). Off, the handlers return at this check.
    active: AtomicBool,
}

impl TargetAuras {
    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::Relaxed)
    }

    /// Starts following the packets (the HUD shown).
    pub fn start(&self) {
        self.active.store(true, Ordering::Relaxed);
    }

    /// Forgets everything and stops following the packets (the HUD hidden, a logout).
    pub fn clear(&self) {
        self.active.store(false, Ordering::Relaxed);
        *self.table.lock() = Table::default();
    }

    /// A `moveToArea` packet's `uoBranch`: a new room, its players' `entID`s.
    pub fn on_room(&self, branch: &[Value]) {
        if !self.is_active() {
            return;
        }
        // The game forgets the classes it missed with each room: the icons it couldn't draw may
        // load now.
        for state in self.icons.lock().values_mut() {
            if matches!(state, IconState::Missing { .. }) {
                *state = IconState::Wanted;
            }
        }
        let mut table = self.table.lock();
        *table = Table::default();
        for info in branch.iter().filter_map(Value::as_object) {
            if let (Some(id), Some(name)) = (num(info.get("entID")), name_of(info)) {
                table.players.insert(id, name);
            }
        }
    }

    /// A JSON `uotls` packet: a player arriving brings their `entID`.
    pub fn on_player(&self, name: &str, fields: &Map<String, Value>) {
        if !self.is_active() {
            return;
        }
        if let Some(id) = num(fields.get("entID")) {
            let name = name_of(fields).unwrap_or_else(|| name.to_owned());
            self.table.lock().players.insert(id, name);
        }
    }

    /// `ct`'s `a` entries, or one sent alone (`{"cmd": "aura*", "tInf": "m:1"}`).
    pub fn on_auras(&self, entries: &[Value], now: Instant) {
        if !self.is_active() {
            return;
        }
        let mut table = self.table.lock();
        let table = &mut *table;
        for entry in entries {
            let Some(cmd) = entry.get("cmd").and_then(Value::as_str) else {
                continue;
            };
            let Some(('m', id)) = entry.get("tInf").and_then(Value::as_str).and_then(id_ref) else {
                continue;
            };
            match cmd {
                "aura+" | "aura++" => {
                    let caster = Caster::parse(entry.get("cInf"));
                    if let Caster::Player(p) = caster
                        && !table.players.contains_key(&p)
                        && !table.tried.contains(&p)
                    {
                        table.pending.insert(p);
                    }
                    let monster = table.monsters.entry(id).or_default();
                    for aura in entry
                        .get("auras")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                    {
                        let Some(name) = aura.get("nam").and_then(Value::as_str) else {
                            continue;
                        };
                        let stacks = num(aura.get("stk")).unwrap_or(1).max(1);
                        let icon = aura.get("icon").and_then(Value::as_str).and_then(icon_link);
                        let duration = float(aura.get("dur")).unwrap_or(0.0);
                        let key = name.to_lowercase();
                        monster.gone.remove(&key);
                        match monster
                            .auras
                            .iter_mut()
                            .find(|a| a.name.to_lowercase() == key)
                        {
                            Some(live) => {
                                live.caster = caster;
                                live.stacks = stacks;
                                live.duration = duration;
                                live.since = now;
                                if icon.is_some() {
                                    live.icon = icon;
                                }
                            }
                            None => {
                                table.next_order += 1;
                                monster.auras.push(Live {
                                    name: name.to_owned(),
                                    caster,
                                    stacks,
                                    duration,
                                    since: now,
                                    order: table.next_order,
                                    icon,
                                });
                            }
                        }
                    }
                }
                "aura-" | "aura--" => {
                    let name = entry
                        .get("aura")
                        .and_then(|a| a.get("nam"))
                        .and_then(Value::as_str);
                    if let (Some(name), Some(monster)) = (name, table.monsters.get_mut(&id)) {
                        monster.remove(name, now, false);
                    }
                }
                "aura*" => {
                    if let Some(monster) = table.monsters.get_mut(&id) {
                        let names: Vec<String> =
                            monster.auras.iter().map(|a| a.name.clone()).collect();
                        for name in names {
                            monster.remove(&name, now, true);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    /// `ct`'s `m` object: each monster's changed `intHP`, `intState`. One at 0 died: its
    /// auras go, and the HUD with it if it was your target.
    pub fn on_monsters(&self, monsters: &Map<String, Value>) {
        if !self.is_active() {
            return;
        }
        let mut table = self.table.lock();
        for (id, fields) in monsters {
            if let (Ok(id), Some(fields)) = (id.parse::<i64>(), fields.as_object()) {
                update_monster(&mut table, id, fields);
            }
        }
    }

    /// An `mtls` packet: monster `id`'s fields (`intHP`, `intHPMax`, `intState`).
    pub fn on_monster(&self, id: i64, fields: &Map<String, Value>) {
        if !self.is_active() {
            return;
        }
        update_monster(&mut self.table.lock(), id, fields);
    }

    /// A `gar` you sent (`a1>m:1,a1>m:2`): the first monster it hits becomes your target.
    pub fn on_attack(&self, targets: &str) {
        if !self.is_active() {
            return;
        }
        let first = targets
            .split(',')
            .filter_map(|t| t.split_once('>').map(|(_, at)| at))
            .find_map(|at| id_ref(at).filter(|(kind, _)| *kind == 'm'));
        if let Some((_, id)) = first {
            let mut table = self.table.lock();
            table.target = Some(id);
            table.monsters.entry(id).or_default();
        }
    }

    /// Whether the timer should read the game: a caster or your target's name unknown, or your
    /// target's earlier auras not read yet.
    pub fn refresh_due(&self) -> bool {
        if !self.is_active() {
            return false;
        }
        if !self.icons_due(Instant::now()).is_empty() {
            return true;
        }
        let table = self.table.lock();
        let target = table.target.and_then(|id| table.monsters.get(&id));
        !table.pending.is_empty() || target.is_some_and(|m| m.name.is_empty() || !m.seeded)
    }

    /// The icon named `link` for the HUD: the game's drawing of it once the timer has it (asked
    /// for here the first time), the game's own stand-in ([`FALLBACK_ICON`]) while the game can't
    /// find it, `None` until then.
    pub fn icon(&self, link: &str) -> Option<Arc<AuraIcon>> {
        let mut icons = self.icons.lock();
        match icons.get(link) {
            Some(IconState::Ready(icon)) => Some(icon.clone()),
            Some(IconState::Wanted) => None,
            Some(IconState::Missing { .. }) if link != FALLBACK_ICON => {
                drop(icons);
                self.icon(FALLBACK_ICON)
            }
            Some(IconState::Missing { .. }) => None,
            None => {
                icons.insert(link.to_owned(), IconState::Wanted);
                None
            }
        }
    }

    /// The icons to draw now: wanted ones, and missing ones due another try.
    fn icons_due(&self, now: Instant) -> Vec<String> {
        self.icons
            .lock()
            .iter()
            .filter(|(_, state)| match state {
                IconState::Wanted => true,
                IconState::Missing { at, tries } => *tries < ICON_TRIES && *at + ICON_RETRY <= now,
                IconState::Ready(_) => false,
            })
            .map(|(link, _)| link.clone())
            .take(ICONS_PER_READ)
            .collect()
    }

    /// Has turu.swf draw the icons due (`GetAuraIcon`). A host without the call (an older
    /// turu.swf) leaves them missing for good.
    fn draw_icons(&self, bot: &Bot) {
        let now = Instant::now();
        for link in self.icons_due(now) {
            let answer = bot
                .flash
                .call("GetAuraIcon", vec![link.as_str().into(), ICON_PX.into()]);
            let mut icons = self.icons.lock();
            let tries = match icons.get(&link) {
                Some(IconState::Missing { tries, .. }) => *tries,
                _ => 0,
            };
            let state = match answer {
                Ok(raw) => match parse_icon(&raw) {
                    Some(icon) => IconState::Ready(Arc::new(icon)),
                    None => IconState::Missing {
                        at: now,
                        tries: tries + 1,
                    },
                },
                Err(_) => IconState::Missing {
                    at: now,
                    tries: ICON_TRIES,
                },
            };
            icons.insert(link, state);
        }
    }

    /// Reads what the packets left unknown (Flash calls: never from the UI thread or a packet
    /// handler): the room's players' `entID`s, the monsters' names, your target's auras once.
    pub fn refresh(&self, bot: &Bot) -> BotResult<()> {
        self.draw_icons(bot);
        let (want_players, target) = {
            let table = self.table.lock();
            let target = table.target.filter(|id| {
                table
                    .monsters
                    .get(id)
                    .is_some_and(|m| m.name.is_empty() || !m.seeded)
            });
            (!table.pending.is_empty(), target)
        };
        let players = if want_players {
            room_players(bot)?
        } else {
            Vec::new()
        };
        let monsters = match target {
            Some(_) => crate::api::monsters::map_monsters(bot)?,
            None => Vec::new(),
        };
        let earlier: Vec<Aura> = match target {
            Some(id) => crate::api::auras::monster_auras(bot, crate::api::NameOrId::Id(id))?,
            None => Vec::new(),
        };
        let now = Instant::now();
        let mut table = self.table.lock();
        let table = &mut *table;
        table.players.extend(players);
        let asked: Vec<i64> = table.pending.drain().collect();
        table.tried.extend(asked);
        for found in &monsters {
            if let Some(monster) = table.monsters.get_mut(&found.map_id) {
                monster.name = found.name.clone();
                if monster.max_hp <= 0 {
                    monster.max_hp = found.max_hp;
                }
                monster.hp.get_or_insert(found.hp);
            }
        }
        if let Some(monster) = target.and_then(|id| table.monsters.get_mut(&id)) {
            if monster.name.is_empty() {
                // Gone from the map: don't ask again.
                monster.name = "Target".into();
            }
            if !monster.seeded {
                monster.seeded = true;
                for aura in earlier {
                    let key = aura.name.to_lowercase();
                    if aura.name.is_empty()
                        || monster.auras.iter().any(|a| a.name.to_lowercase() == key)
                    {
                        continue;
                    }
                    // With a time stamp, the seconds it has left (the game can keep one that ran
                    // out: skipped); without one, no countdown.
                    let (duration, since) = if aura.time_stamp > 0 {
                        if aura.remaining_time <= 0 {
                            continue;
                        }
                        let spent = (aura.duration - aura.remaining_time).max(0) as u64;
                        let since = now.checked_sub(Duration::from_secs(spent)).unwrap_or(now);
                        (aura.duration as f64, since)
                    } else {
                        (0.0, now)
                    };
                    table.next_order += 1;
                    monster.auras.push(Live {
                        name: aura.name,
                        caster: Caster::Unknown,
                        stacks: (aura.value as i64).max(1),
                        duration,
                        since,
                        order: table.next_order,
                        icon: icon_link(&aura.icon),
                    });
                }
            }
        }
        Ok(())
    }

    /// Your target's auras, `me` being your username; `None` without a target.
    pub fn snapshot(&self, me: Option<&str>, now: Instant) -> Option<TargetView> {
        if !self.is_active() {
            return None;
        }
        let mut table = self.table.lock();
        let id = table.target?;
        table.monsters.get_mut(&id)?.expire(now);
        let monster = table.monsters.get(&id)?;

        let pinned = |name: &str| {
            PINS.iter()
                .any(|pin| pin.auras.iter().any(|a| a.eq_ignore_ascii_case(name)))
        };
        let caster_name = |caster: Caster| match caster {
            Caster::Player(p) => table.players.get(&p).cloned(),
            Caster::Monster(m) => table
                .monsters
                .get(&m)
                .map(|m| m.name.clone())
                .filter(|n| !n.is_empty()),
            Caster::Unknown => None,
        };

        let pins = PINS
            .iter()
            .map(|pin| {
                let live: Vec<&Live> = pin
                    .auras
                    .iter()
                    .filter_map(|name| {
                        monster
                            .auras
                            .iter()
                            .find(|a| a.name.eq_ignore_ascii_case(name))
                    })
                    .collect();
                let latest = live.iter().max_by_key(|a| a.since);
                let state = if latest.is_some() {
                    PinState::Up
                } else {
                    pin.auras
                        .iter()
                        .filter_map(|name| monster.gone.get(&name.to_lowercase()))
                        .max_by_key(|(at, _)| *at)
                        .map_or(PinState::Never, |(at, cleansed)| PinState::Down {
                            ago: now.saturating_duration_since(*at),
                            cleansed: *cleansed,
                        })
                };
                PinView {
                    label: pin.label,
                    caster: latest
                        .map(|a| caster_name(a.caster).unwrap_or_else(|| unknown(a.caster))),
                    auras: pin
                        .auras
                        .iter()
                        .map(|name| {
                            let view = live
                                .iter()
                                .find(|a| a.name.eq_ignore_ascii_case(name))
                                .map(|a| AuraView::of(a, now));
                            (*name, view)
                        })
                        .collect(),
                    state,
                }
            })
            .collect();

        let mut groups: Vec<(Caster, Vec<&Live>)> = Vec::new();
        let mut auras: Vec<&Live> = monster.auras.iter().filter(|a| !pinned(&a.name)).collect();
        auras.sort_by_key(|a| a.order);
        for aura in auras {
            match groups.iter_mut().find(|(c, _)| *c == aura.caster) {
                Some((_, list)) => list.push(aura),
                None => groups.push((aura.caster, vec![aura])),
            }
        }
        let mut casters: Vec<CasterAuras> = groups
            .into_iter()
            .map(|(caster, list)| {
                let name = caster_name(caster).unwrap_or_else(|| unknown(caster));
                let kind = match caster {
                    Caster::Player(_) if me.is_some_and(|me| me.eq_ignore_ascii_case(&name)) => {
                        CasterKind::Me
                    }
                    Caster::Player(_) => CasterKind::Player,
                    Caster::Unknown => CasterKind::Unknown,
                    Caster::Monster(_) => CasterKind::Monster,
                };
                CasterAuras {
                    name,
                    kind,
                    auras: list.into_iter().map(|a| AuraView::of(a, now)).collect(),
                }
            })
            .collect();
        let rank = |kind: CasterKind| match kind {
            CasterKind::Me => 0,
            CasterKind::Player => 1,
            CasterKind::Unknown => 2,
            CasterKind::Monster => 3,
        };
        casters.sort_by(|a, b| {
            rank(a.kind)
                .cmp(&rank(b.kind))
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });

        Some(TargetView {
            monster: if monster.name.is_empty() {
                "Target".into()
            } else {
                monster.name.clone()
            },
            hp: monster.hp,
            max_hp: monster.max_hp,
            pins,
            casters,
        })
    }
}

/// Monster `id`'s changed fields (`intHP`, `intHPMax`, `intState`). One at 0 died: its auras
/// go, and the HUD with it if it was your target.
fn update_monster(table: &mut Table, id: i64, fields: &Map<String, Value>) {
    let hp = num(fields.get("intHP"));
    let dead = hp == Some(0) || num(fields.get("intState")) == Some(0);
    if dead {
        table.monsters.remove(&id);
        if table.target == Some(id) {
            table.target = None;
        }
        return;
    }
    let Some(monster) = table.monsters.get_mut(&id) else {
        return;
    };
    if hp.is_some() {
        monster.hp = hp;
    }
    if let Some(max) = num(fields.get("intHPMax")) {
        monster.max_hp = max;
    }
}

/// An icon's name in the game as its aura HUD takes it: the last of a comma list
/// (`i,i,i,i,i,LR2` is `LR2`); `None` when empty.
fn icon_link(icon: &str) -> Option<String> {
    let link = icon.rsplit(',').next().unwrap_or_default().trim();
    (!link.is_empty() && link != "undefined" && link != "null").then(|| link.to_owned())
}

/// `GetAuraIcon`'s answer, `"<size>:<Base64 of ARGB pixels>"`, as RGBA; `None` when it's empty,
/// the wrong length, or wholly transparent (nothing was drawn).
fn parse_icon(raw: &str) -> Option<AuraIcon> {
    let (size, pixels) = raw.trim().trim_matches('"').split_once(':')?;
    let size: usize = size.parse().ok().filter(|s| (1..=128).contains(s))?;
    let argb = crate::capture::base64(pixels);
    if argb.len() != size * size * 4 {
        return None;
    }
    let (pixels, _) = argb.as_chunks::<4>();
    if pixels.iter().all(|[a, ..]| *a == 0) {
        return None;
    }
    let rgba = pixels
        .iter()
        .flat_map(|&[a, r, g, b]| [r, g, b, a])
        .collect();
    Some(AuraIcon { size, rgba })
}

/// What a caster whose name isn't known is called.
fn unknown(caster: Caster) -> String {
    match caster {
        Caster::Monster(_) => "Monster".into(),
        _ => "Unknown".into(),
    }
}

/// The room's players' `entID`s and names (`world.areaUsers`, then each one's `world.uoTree`
/// entry in one `getMany`).
fn room_players(bot: &Bot) -> BotResult<Vec<(i64, String)>> {
    let names: Vec<String> = bot.flash.get_opt("world.areaUsers")?.unwrap_or_default();
    let mut paths = Vec::with_capacity(names.len() * 2);
    for (i, name) in names.iter().enumerate() {
        let key = name.to_lowercase();
        paths.push((format!("{i}|id"), format!("world.uoTree.{key}.entID")));
        paths.push((
            format!("{i}|name"),
            format!("world.uoTree.{key}.strUsername"),
        ));
    }
    if paths.is_empty() {
        return Ok(Vec::new());
    }
    let refs: Vec<(&str, &str)> = paths
        .iter()
        .map(|(k, p)| (k.as_str(), p.as_str()))
        .collect();
    let values = bot.flash.get_many(&refs)?;
    Ok(names
        .iter()
        .enumerate()
        .filter_map(|(i, name)| {
            let id = num(values.get(&format!("{i}|id")))?;
            let shown = values
                .get(&format!("{i}|name"))
                .and_then(Value::as_str)
                .filter(|n| !n.is_empty())
                .unwrap_or(name);
            Some((id, shown.to_owned()))
        })
        .collect())
}

/// A `uoBranch` entry's name as the game shows it: `strUsername`, else `uoName`.
fn name_of(info: &Map<String, Value>) -> Option<String> {
    ["strUsername", "uoName"]
        .iter()
        .filter_map(|k| info.get(*k).and_then(Value::as_str))
        .find(|n| !n.is_empty())
        .map(str::to_owned)
}

fn float(value: Option<&Value>) -> Option<f64> {
    let value = value?;
    value
        .as_f64()
        .or_else(|| value.as_str().and_then(|s| s.trim().parse().ok()))
}

fn num(value: Option<&Value>) -> Option<i64> {
    float(value).map(|n| n as i64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// A room of four (names made up), each with their `entID`.
    fn room() -> TargetAuras {
        let hud = TargetAuras::default();
        hud.start();
        let branch = json!([
            {"uoName": "questwalker", "strUsername": "Questwalker", "entID": 70171},
            {"uoName": "moonfarer", "strUsername": "Moonfarer", "entID": 70177},
            {"uoName": "astra", "entID": 70186},
            {"uoName": "lumenward", "strUsername": "Lumenward", "entID": 70191},
        ]);
        hud.on_room(branch.as_array().unwrap());
        hud.on_attack("a1>m:1");
        hud
    }

    fn a(value: Value) -> Vec<Value> {
        value.as_array().unwrap().clone()
    }

    fn after(start: Instant, secs: f32) -> Instant {
        start + Duration::from_secs_f32(secs)
    }

    #[test]
    fn enrage_names_who_cast_it_and_counts_down() {
        let hud = room();
        let t0 = Instant::now();
        // As the server sent it: the old Focus off, then Focus and Reckless from one player.
        hud.on_auras(
            &a(json!([
                {"cmd": "aura-", "tInf": "m:1", "aura": {"nam": "Focus"}},
                {"cmd": "aura+", "tInf": "m:1", "cInf": "p:70177",
                    "auras": [{"nam": "Focus", "dur": 6, "t": "s", "isNew": true, "icon": "iwd1,ied1"}]},
                {"cmd": "aura+", "tInf": "m:1", "cInf": "p:70177",
                    "auras": [{"nam": "Reckless", "dur": 10, "t": "s", "icon": "iwd1,ied1"}]},
            ])),
            t0,
        );
        let view = hud.snapshot(Some("questwalker"), after(t0, 2.0)).unwrap();
        let pin = &view.pins[0];
        assert_eq!(pin.label, "Enrage");
        assert_eq!(pin.state, PinState::Up);
        assert_eq!(pin.caster.as_deref(), Some("Moonfarer"));
        let focus = pin.auras[0].1.as_ref().unwrap();
        assert_eq!(focus.left.map(f32::round), Some(4.0));
        // Pinned auras aren't listed again under their caster.
        assert!(view.casters.is_empty());

        // Focus runs out (no aura- came); Reckless is still on.
        let view = hud.snapshot(None, after(t0, 7.0)).unwrap();
        assert!(view.pins[0].auras[0].1.is_none());
        assert_eq!(view.pins[0].state, PinState::Up);
        // Then Reckless too: down since it ran out.
        let view = hud.snapshot(None, after(t0, 13.0)).unwrap();
        assert_eq!(
            view.pins[0].state,
            PinState::Down {
                ago: Duration::from_secs(3),
                cleansed: false
            }
        );
    }

    #[test]
    fn a_second_caster_takes_over_an_aura() {
        let hud = room();
        let t0 = Instant::now();
        let reckless = |caster: &str| {
            json!([{"cmd": "aura+", "tInf": "m:1", "cInf": caster,
                "auras": [{"nam": "Reckless", "dur": 10, "t": "s"}]}])
        };
        hud.on_auras(&a(reckless("p:70177")), t0);
        // No aura- between: the server replaces it.
        hud.on_auras(&a(reckless("p:70171")), after(t0, 1.0));
        let view = hud.snapshot(Some("Questwalker"), after(t0, 2.0)).unwrap();
        assert_eq!(view.pins[0].caster.as_deref(), Some("Questwalker"));
        assert_eq!(
            view.pins[0].auras[1]
                .1
                .as_ref()
                .unwrap()
                .left
                .map(f32::round),
            Some(9.0)
        );
    }

    #[test]
    fn casters_group_their_auras_you_first_and_the_boss_last() {
        let hud = room();
        let t0 = Instant::now();
        hud.on_auras(
            &a(json!([
                {"cmd": "aura++", "tInf": "m:1", "cInf": "m:1",
                    "auras": [{"nam": "The Cycle’s End", "dur": 7200, "t": "s", "stk": 3}]},
                {"cmd": "aura+", "tInf": "m:1", "cInf": "p:70186",
                    "auras": [{"nam": "Commandment", "dur": 10, "t": "s", "stk": 4, "icon": "apal1"}]},
                {"cmd": "aura+", "tInf": "m:1", "cInf": "p:70171",
                    "auras": [{"nam": "Seared", "dur": 12, "t": "s", "stk": 10, "icon": "i,i,i,i,i,LR2"}]},
                {"cmd": "aura+", "tInf": "m:1", "cInf": "p:70171",
                    "auras": [{"nam": "Atramentous Shade", "dur": 12, "t": "s"}]},
                // On a player, not the monster: not the HUD's.
                {"cmd": "aura+", "tInf": "p:70171", "cInf": "m:1",
                    "auras": [{"nam": "Realm of the Arcana", "dur": 12, "t": "s"}]},
            ])),
            t0,
        );
        let view = hud.snapshot(Some("questwalker"), t0).unwrap();
        let names: Vec<_> = view
            .casters
            .iter()
            .map(|c| (c.name.as_str(), c.kind))
            .collect();
        assert_eq!(
            names,
            [
                ("Questwalker", CasterKind::Me),
                ("astra", CasterKind::Player),
                ("Monster", CasterKind::Monster),
            ]
        );
        let mine: Vec<_> = view.casters[0]
            .auras
            .iter()
            .map(|a| (a.name.as_str(), a.stacks))
            .collect();
        assert_eq!(mine, [("Seared", 10), ("Atramentous Shade", 1)]);
        let boss = &view.casters[2].auras[0];
        assert_eq!(
            (boss.stacks, boss.left),
            (3, None),
            "7200 s has no countdown"
        );
        assert_eq!(view.pins[0].state, PinState::Never);

        hud.on_auras(
            &a(json!([{"cmd": "aura--", "tInf": "m:1", "aura": {"nam": "The Cycle’s End"}}])),
            t0,
        );
        assert_eq!(hud.snapshot(None, t0).unwrap().casters.len(), 2);
    }

    #[test]
    fn a_cleanse_takes_everything_off() {
        let hud = room();
        let t0 = Instant::now();
        hud.on_auras(
            &a(json!([
                {"cmd": "aura+", "tInf": "m:1", "cInf": "p:70177", "auras": [{"nam": "Focus", "dur": 6, "t": "s"}]},
                {"cmd": "aura+", "tInf": "m:1", "cInf": "p:70177", "auras": [{"nam": "Bruised", "dur": 8, "t": "s", "stk": 5}]},
            ])),
            t0,
        );
        hud.on_auras(&a(json!([{"cmd": "aura*", "tInf": "m:1"}])), after(t0, 1.0));
        let view = hud.snapshot(None, after(t0, 3.0)).unwrap();
        assert!(view.casters.is_empty());
        assert_eq!(
            view.pins[0].state,
            PinState::Down {
                ago: Duration::from_secs(2),
                cleansed: true
            }
        );
    }

    #[test]
    fn the_target_follows_your_attacks_and_goes_when_it_dies() {
        let hud = room();
        let t0 = Instant::now();
        hud.on_auras(
            &a(json!([{"cmd": "aura+", "tInf": "m:2", "cInf": "p:70171", "auras": [{"nam": "Decay", "dur": 2, "t": "s"}]}])),
            t0,
        );
        assert!(
            hud.snapshot(None, t0).unwrap().casters.is_empty(),
            "m:1 is the target"
        );
        hud.on_attack("a3>p:70171,aa>m:2");
        assert_eq!(hud.snapshot(None, t0).unwrap().casters.len(), 1);
        hud.on_monsters(
            json!({"2": {"intHP": 0, "intState": 0}})
                .as_object()
                .unwrap(),
        );
        assert_eq!(hud.snapshot(None, t0), None);
    }

    #[test]
    fn an_unknown_caster_asks_for_a_read() {
        let hud = room();
        // A name for the target and its earlier auras: what a read would give.
        {
            let mut table = hud.table.lock();
            let monster = table.monsters.get_mut(&1).unwrap();
            monster.name = "Ultra Boss".into();
            monster.seeded = true;
        }
        assert!(!hud.refresh_due());
        hud.on_auras(
            &a(json!([{"cmd": "aura+", "tInf": "m:1", "cInf": "p:99999", "auras": [{"nam": "Sinkhole", "dur": 8, "t": "s"}]}])),
            Instant::now(),
        );
        assert!(hud.refresh_due());
        let player = json!({"uoName": "nightjar", "strUsername": "Nightjar", "entID": 99999});
        hud.on_player("nightjar", player.as_object().unwrap());
        let view = hud.snapshot(None, Instant::now()).unwrap();
        assert_eq!(view.monster, "Ultra Boss");
        assert_eq!(view.casters[0].name, "Nightjar");
    }

    #[test]
    fn icons_are_named_and_read_as_the_game_does() {
        assert_eq!(icon_link("i,i,i,i,i,LR2").as_deref(), Some("LR2"));
        assert_eq!(icon_link("iwd1,ied1").as_deref(), Some("ied1"));
        assert_eq!(icon_link("apal3").as_deref(), Some("apal3"));
        assert_eq!(icon_link(""), None);
        // Two by two pixels, opaque red, as ARGB.
        let icon = parse_icon("\"2://8AAP//AAD//wAA//8AAA==\"").unwrap();
        assert_eq!(icon.size, 2);
        assert_eq!(&icon.rgba[..4], [255, 0, 0, 255]);
        assert_eq!(parse_icon(""), None);
        assert_eq!(
            parse_icon("3://8AAP//AAD//wAA//8AAA=="),
            None,
            "the wrong length"
        );
        assert_eq!(parse_icon("1:AAAAAA=="), None, "nothing drawn");
    }

    #[test]
    fn off_it_ignores_packets() {
        let hud = TargetAuras::default();
        hud.on_attack("a1>m:1");
        hud.on_auras(
            &a(json!([{"cmd": "aura+", "tInf": "m:1", "cInf": "p:1", "auras": [{"nam": "Focus", "dur": 6}]}])),
            Instant::now(),
        );
        assert_eq!(hud.snapshot(None, Instant::now()), None);
        assert!(!hud.refresh_due());
        assert!(hud.table.lock().monsters.is_empty());
    }
}
