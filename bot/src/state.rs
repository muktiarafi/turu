//! Game state the bot learns from Flash events and packets (not read through `getGameObject`).
//!
//! Never hold the `bot.state` lock across a Flash call: packets are handled on the player's
//! thread and lock it too, so waiting on Flash with it held deadlocks.

use crate::api::NameOrId;
use crate::models::ItemBase;
use std::collections::HashMap;
use std::time::Instant;

#[derive(Debug, Default)]
pub struct GameState {
    /// `requestLoadGame` arrived and `loadClient` was called.
    pub client_requested: bool,
    /// `loaded`: the game SWF finished loading.
    pub game_loaded: bool,
    /// Logged in and the world is up, as of the last background tick.
    pub in_game: bool,
    /// `catchPackets` was called for this login (VibeSkua: the timer's `catching`).
    pub catching_packets: bool,
    /// The logged-in username (`loginResponse`, then `loginInfo.strUsername`), as the game
    /// spells it. Used by packet handlers, which can't ask Flash.
    pub username: Option<String>,
    /// `moveToArea`: `strMapName` of the current map (what `world.strMapName` reads).
    pub map_name: Option<String>,
    /// `moveToArea`: `strMapFileName` of the current map (VibeSkua `Map.FilePath`).
    pub map_file_path: Option<String>,
    /// The last map name `Join` targeted or `moveToArea` reported (VibeSkua `Map.LastMap`).
    pub last_map: Option<String>,
    /// Items currently offered in the drop stack, by item ID (`dropItem` packets).
    pub drops: HashMap<i64, ItemBase>,
    /// Counters for Combat > Stats.
    pub stats: Stats,
    /// Shop data from the last `loadShop` packet.
    pub shop_loaded_at: Option<Instant>,
    /// Bank was loaded (`loadBank` packet) since login.
    pub bank_loaded: bool,
    /// The user clicked Logout / Change server (don't auto-relogin).
    pub intentional_logout: bool,
    /// `dataLeaf.afk` as the last `uotls` packet about us said.
    pub afk: bool,
    /// One-shot signals for `Wait.ForItemBuy`, `ForItemSell`, `ForBankLoad` (VibeSkua's
    /// `AutoResetEvent`s): set by the packet, consumed by the wait.
    pub signals: Signals,
    /// When `Hunt` last jumped to another cell (VibeSkua `_lastHuntTick`).
    pub last_hunt_jump: Option<Instant>,
    /// What `options::apply` last pushed into the game.
    pub options_applied: crate::api::options::Applied,
    /// The game's frame rate when it was slowed down (Headless Mode, or a hidden tab),
    /// restored when neither applies any more (kept across relogins, unlike
    /// `options_applied`).
    pub before_slowed: Option<i32>,
    /// The game's quality when Headless Mode set it to LOW, restored when it's turned off.
    pub before_low_quality: Option<String>,
    /// This client is a tab the window doesn't show: [`crate::api::options::apply`] slows its
    /// game down (VibeSkua's tabbed host `WM_SKUA_THROTTLE`).
    pub hidden_tab: bool,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct Signals {
    pub item_bought: bool,
    pub item_sold: bool,
    pub bank_loaded: bool,
}

#[derive(Debug, Default, Clone)]
pub struct Stats {
    pub started: Option<Instant>,
    pub kills: u64,
    pub deaths: u64,
    pub drops: u64,
    pub quests_accepted: u64,
    pub quests_completed: u64,
    pub relogins: u64,
}

impl GameState {
    /// Adds a `dropItem` drop; a drop already in the stack gets its quantity added
    /// (VibeSkua `ScriptDrop.AddDrop`).
    pub fn add_drop(&mut self, item: ItemBase) {
        match self.drops.get_mut(&item.id) {
            Some(existing) => existing.quantity += item.quantity,
            None => {
                self.drops.insert(item.id, item);
            }
        }
    }

    /// VibeSkua `Drops.Exists`: `"*"` is any drop; names compare case-insensitively.
    pub fn drop_exists(&self, item: NameOrId) -> bool {
        match item {
            NameOrId::Name(name) if name.trim() == "*" => !self.drops.is_empty(),
            NameOrId::Name(name) => self.drops.values().any(|d| d.name_is(name)),
            NameOrId::Id(id) => self.drops.contains_key(&id),
        }
    }

    /// The drop with this name or ID.
    pub fn find_drop(&self, item: NameOrId) -> Option<&ItemBase> {
        match item {
            NameOrId::Name(name) => self.drops.values().find(|d| d.name_is(name)),
            NameOrId::Id(id) => self.drops.get(&id),
        }
    }

    /// Forgets what belongs to the last session (drops, bank, applied options).
    pub fn reset_session(&mut self) {
        self.drops.clear();
        self.bank_loaded = false;
        self.afk = false;
        self.catching_packets = false;
        self.options_applied = Default::default();
    }
}
