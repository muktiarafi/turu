//! What a bot is doing, read from the game in one go: the web monitor shows it, and the
//! webhooks' snapshot fields are made from it (VibeSkua `BuildDashboardSnapshotFields`).
//!
//! [`Snapshot::read`] reads the quick fields (a few `Flash` gets each); [`Snapshot::read_slow`]
//! adds the ones that read whole item and quest lists.

use crate::Bot;
use crate::api::{inventory, map, player, quests, servers};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BotState {
    /// No player, or `skua.swf` hasn't loaded the game yet.
    #[default]
    NotLoaded,
    /// The game is loaded; not logged in.
    LoggedOut,
    /// Logged in, no avatar yet (a map is loading).
    Loading,
    Dead,
    /// Logged in and alive (VibeSkua `Player.Playing`).
    Playing,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Snapshot {
    /// VibeSkua `GetBotName`: the character, else the remembered login, else "Turu Bot".
    pub name: String,
    pub state: BotState,
    pub relogging: bool,
    pub script: ScriptInfo,
    /// "Auto Attack" or "Auto Hunt" while one runs (`crate::auto`).
    pub auto: Option<String>,
    pub server: String,
    /// `map-room`, as VibeSkua shows it.
    pub map: String,
    pub room_id: i64,
    pub cell: String,
    pub pad: String,
    pub level: i64,
    pub gold: i64,
    /// AdventureCoins.
    pub coins: i64,
    pub xp: i64,
    pub hp: Pair,
    pub mp: Pair,
    pub target: Option<Target>,
    pub stats: Stats,
    /// What the running script has earned, once the monitor saw it start.
    pub session: Option<Session>,
    // The slow fields.
    pub class: Option<String>,
    pub equipment: Vec<String>,
    /// The active quests' names.
    pub quests: Vec<String>,
    pub inventory: Inventory,
    /// When it was read, in ms since 1970.
    pub sampled_at: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptInfo {
    /// The file name without `.ts`.
    pub name: Option<String>,
    pub path: Option<String>,
    /// `ScriptStatus::label`.
    pub status: String,
    pub running: bool,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct Pair {
    pub cur: i64,
    pub max: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Target {
    pub name: String,
    pub hp: i64,
    pub max_hp: i64,
}

/// Combat > Stats (`GameState::stats`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    pub kills: u64,
    pub deaths: u64,
    pub drops: u64,
    pub quests_accepted: u64,
    pub quests_completed: u64,
    pub relogins: u64,
    pub run_seconds: Option<u64>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub seconds: u64,
    pub gold: i64,
    pub xp: i64,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Inventory {
    /// Every item in the inventory, equipped ones too (what the webhooks count).
    pub items: usize,
    /// Bag slots used and owned.
    pub used: i64,
    pub slots: i64,
}

impl Snapshot {
    /// The quick fields; the slow ones are left empty.
    pub fn read(bot: &Bot) -> Self {
        let state_lock = bot.state.lock();
        let s = &state_lock.stats;
        let stats = Stats {
            kills: s.kills,
            deaths: s.deaths,
            drops: s.drops,
            quests_accepted: s.quests_accepted,
            quests_completed: s.quests_completed,
            relogins: s.relogins,
            run_seconds: s.started.map(|t| t.elapsed().as_secs()),
        };
        let loaded = state_lock.game_loaded && bot.flash.has_host();
        drop(state_lock);
        let path = bot.scripts.loaded();
        let mut snapshot = Self {
            name: bot_name(bot),
            relogging: bot.servers.is_relogging(),
            script: ScriptInfo {
                name: path
                    .as_ref()
                    .and_then(|p| p.file_stem())
                    .map(|s| s.to_string_lossy().into_owned()),
                path: path.map(|p| p.display().to_string()),
                status: bot.scripts.status().label().into(),
                running: bot.scripts.is_running(),
            },
            auto: bot.auto.mode().map(|m| m.label().to_owned()),
            stats,
            sampled_at: now_ms(),
            ..Self::default()
        };
        if !loaded {
            return snapshot;
        }
        snapshot.state = match play_state(bot) {
            Ok(state) => state,
            Err(_) => return snapshot,
        };
        if snapshot.state == BotState::LoggedOut {
            return snapshot;
        }
        let f = &bot.flash;
        snapshot.server = bot.servers.last_name();
        snapshot.map = map::full_name(bot).unwrap_or_default();
        snapshot.room_id = map::room_id(bot).unwrap_or(0);
        snapshot.cell = player::cell(bot).unwrap_or_default();
        snapshot.pad = player::pad(bot).unwrap_or_default();
        let int = |path: &str| f.get_or::<i64>(path, 0).unwrap_or(0);
        snapshot.level = int("world.myAvatar.dataLeaf.intLevel");
        snapshot.gold = int("world.myAvatar.objData.intGold");
        snapshot.coins = int("world.myAvatar.objData.intCoins");
        snapshot.xp = int("world.myAvatar.objData.intExp");
        snapshot.hp = Pair {
            cur: player::health(bot).unwrap_or(0),
            max: player::max_health(bot).unwrap_or(0),
        };
        snapshot.mp = Pair {
            cur: player::mana(bot).unwrap_or(0),
            max: player::max_mana(bot).unwrap_or(0),
        };
        if snapshot.state == BotState::Playing {
            snapshot.target = player::target(bot)
                .ok()
                .flatten()
                .filter(|m| m.alive())
                .map(|m| Target {
                    name: m.name,
                    hp: m.hp,
                    max_hp: m.max_hp,
                });
        }
        snapshot
    }

    /// Adds the class, equipment, active quests and inventory (only while playing).
    pub fn read_slow(&mut self, bot: &Bot) {
        if self.state != BotState::Playing {
            return;
        }
        let items = inventory::inventory_items(bot).unwrap_or_default();
        self.class = items
            .iter()
            .find(|i| i.equipped && i.is_class())
            .map(|i| i.name.clone());
        self.equipment = items
            .iter()
            .filter(|i| i.equipped && !i.name.trim().is_empty())
            .map(|i| i.name.clone())
            .collect();
        self.quests = quests::tree(bot)
            .unwrap_or_default()
            .into_iter()
            .filter(|q| q.active() && !q.name.trim().is_empty())
            .map(|q| q.name)
            .collect();
        self.inventory = Inventory {
            items: items.len(),
            used: inventory::used_slots(bot, inventory::Container::Inventory).unwrap_or(0),
            slots: inventory::slots(bot, inventory::Container::Inventory).unwrap_or(0),
        };
    }

    /// Takes the slow fields of an earlier read.
    pub fn keep_slow(&mut self, earlier: &Snapshot) {
        if self.state != BotState::Playing {
            return;
        }
        self.class.clone_from(&earlier.class);
        self.equipment.clone_from(&earlier.equipment);
        self.quests.clone_from(&earlier.quests);
        self.inventory = earlier.inventory;
    }

    pub fn playing(&self) -> bool {
        self.state == BotState::Playing
    }
}

/// `player::playing`, telling the reasons apart.
fn play_state(bot: &Bot) -> crate::BotResult<BotState> {
    if !player::logged_in(bot)? {
        return Ok(BotState::LoggedOut);
    }
    if bot.flash.is_null("world.myAvatar")? {
        return Ok(BotState::Loading);
    }
    let state: i64 = bot.flash.get_or("world.myAvatar.dataLeaf.intState", 0)?;
    Ok(if state > 0 {
        BotState::Playing
    } else {
        BotState::Dead
    })
}

/// VibeSkua `GetBotName`.
pub fn bot_name(bot: &Bot) -> String {
    if let Ok(name) = player::username(bot)
        && !name.trim().is_empty()
        && name != "loginInfo.strUsername"
    {
        return name;
    }
    servers::cached_username(bot)
        .ok()
        .flatten()
        .unwrap_or_else(|| "Turu Bot".into())
}

pub(crate) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}
