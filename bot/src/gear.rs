//! Gear from outside a script (docs/turu/ARMY_EQUIPMENT_PROPOSAL.md, phase 1):
//!
//! * Every client keeps `gear/<username>.json` up to date: its inventory, and its bank as last
//!   loaded (dated by `bankAsOf`). The launcher reads it to show and pick gear for an account
//!   that has no window, or isn't online. The bank is never loaded for the file's sake: it's
//!   taken whenever something loaded it this login (CoreBots at every script start, an equip
//!   that needed it, Load bank), and kept from the last file otherwise.
//! * [`equip`] and [`load_bank`] are what the control API's `/equip` and `/bank/load` run.
//!   They refuse while a script runs (the script owns the gear: CoreBots puts its own back at
//!   its next `equipClass`), unless asked to stop it first; it then stays stopped.

use crate::Bot;
use crate::api::inventory::{self, Container};
use crate::api::{NameOrId, player, wait};
use crate::error::BotResult;
use crate::events::BotEvent;
use crate::models::ItemBase;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// How often the writer thread looks for changes (and so the most often the file is written).
const TICK: Duration = Duration::from_secs(5);
/// The inventory is read at least this often while logged in (equips send no event).
const REFRESH: Duration = Duration::from_secs(30);
/// How long [`equip`] waits for a script it stopped to end.
const STOP_TIMEOUT: Duration = Duration::from_secs(30);

/// Where an item is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Place {
    Inventory,
    Bank,
}

/// An equipment slot (the item's `sES`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Slot {
    Class,
    Weapon,
    Armor,
    Helm,
    Cape,
    Pet,
    Necklace,
    /// Ground items (`mi`).
    Ground,
}

impl Slot {
    pub const ALL: [Self; 8] = [
        Self::Class,
        Self::Weapon,
        Self::Armor,
        Self::Helm,
        Self::Cape,
        Self::Pet,
        Self::Necklace,
        Self::Ground,
    ];

    /// The slot of an item's `sES`; `None` for what isn't worn (resources, quest items).
    pub fn of(equip_slot: &str) -> Option<Self> {
        Some(match equip_slot.trim() {
            "ar" => Self::Class,
            "Weapon" => Self::Weapon,
            "co" => Self::Armor,
            "he" => Self::Helm,
            "ba" => Self::Cape,
            "pe" => Self::Pet,
            "am" => Self::Necklace,
            "mi" => Self::Ground,
            _ => return None,
        })
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Class => "Class",
            Self::Weapon => "Weapon",
            Self::Armor => "Armor",
            Self::Helm => "Helm",
            Self::Cape => "Cape",
            Self::Pet => "Pet",
            Self::Necklace => "Necklace",
            Self::Ground => "Ground",
        }
    }
}

/// One wearable item of the account.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GearItem {
    pub id: i64,
    pub name: String,
    pub category: String,
    pub slot: Slot,
    pub equipped: bool,
    #[serde(rename = "where")]
    pub place: Place,
    pub enhancement_level: i64,
    pub enhancement_pattern_id: i64,
    pub proc_id: i64,
}

impl GearItem {
    /// The item, when it can be worn.
    pub fn from_item(item: &ItemBase, place: Place) -> Option<Self> {
        Some(Self {
            id: item.id,
            name: item.name.trim().to_owned(),
            category: item.category.clone(),
            slot: Slot::of(&item.equip_slot)?,
            equipped: place == Place::Inventory && item.equipped,
            place,
            enhancement_level: item.enhancement_level,
            enhancement_pattern_id: item.enhancement_pattern_id,
            proc_id: item.proc_id,
        })
    }
}

/// `gear/<username>.json`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct GearFile {
    pub username: String,
    /// When it was written, in ms since 1970.
    pub saved_at: u64,
    /// When the bank items were read, in ms since 1970; `None`: never.
    pub bank_as_of: Option<u64>,
    pub items: Vec<GearItem>,
}

impl GearFile {
    /// The item worn in `slot`.
    pub fn worn(&self, slot: Slot) -> Option<&GearItem> {
        self.items.iter().find(|i| i.equipped && i.slot == slot)
    }

    /// The items for `slot`: the inventory's first, then the bank's, each by name.
    pub fn for_slot(&self, slot: Slot) -> Vec<&GearItem> {
        let mut items: Vec<&GearItem> = self.items.iter().filter(|i| i.slot == slot).collect();
        items.sort_by_key(|i| (i.place == Place::Bank, i.name.to_lowercase()));
        items
    }

    /// Same contents, whenever they were written.
    fn same_as(&self, other: &Self) -> bool {
        self.username == other.username
            && self.bank_as_of == other.bank_as_of
            && self.items == other.items
    }
}

/// The account's file: its name in lowercase, kept to characters safe in a file name (as
/// Butler's sync files).
pub fn path_for(dir: &Path, username: &str) -> PathBuf {
    let name: String = username
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, '_' | '-') {
                c
            } else {
                '_'
            }
        })
        .collect();
    dir.join(format!("{name}.json"))
}

/// The account's file, if it has one that reads.
pub fn read(dir: &Path, username: &str) -> Option<GearFile> {
    serde_json::from_str(&fs::read_to_string(path_for(dir, username)).ok()?).ok()
}

/// Writes a temp file and renames it over the account's file, so readers never see half of it.
pub fn write(dir: &Path, file: &GearFile) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    let path = path_for(dir, &file.username);
    let temp = path.with_extension(format!("tmp{}", std::process::id()));
    let text = serde_json::to_vec_pretty(file).map_err(io::Error::other)?;
    for _ in 0..3 {
        fs::write(&temp, &text)?;
        if fs::rename(&temp, &path).is_ok() {
            return Ok(());
        }
        // Windows refuses the rename while a reader has the file open.
        std::thread::sleep(Duration::from_millis(30));
    }
    let _ = fs::remove_file(&temp);
    fs::write(&path, text)
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

/// The file `earlier` becomes with these game items: the bank's from `bank` when it was
/// loaded this login, else `earlier`'s (with its date).
pub fn merge(
    username: &str,
    inventory: &[ItemBase],
    bank: Option<&[ItemBase]>,
    earlier: Option<&GearFile>,
    now: u64,
) -> GearFile {
    let mut items: Vec<GearItem> = inventory
        .iter()
        .filter_map(|i| GearItem::from_item(i, Place::Inventory))
        .collect();
    let bank_as_of = match bank {
        Some(bank) => {
            items.extend(
                bank.iter()
                    .filter_map(|i| GearItem::from_item(i, Place::Bank)),
            );
            Some(now)
        }
        None => {
            let earlier = earlier.filter(|e| e.username.eq_ignore_ascii_case(username));
            if let Some(earlier) = earlier {
                // What the inventory holds now isn't in the bank any more.
                let kept: Vec<GearItem> = earlier
                    .items
                    .iter()
                    .filter(|b| b.place == Place::Bank && !items.iter().any(|i| i.id == b.id))
                    .cloned()
                    .collect();
                items.extend(kept);
            }
            earlier.and_then(|e| e.bank_as_of)
        }
    };
    GearFile {
        username: username.to_owned(),
        saved_at: now,
        bank_as_of,
        items,
    }
}

/// Keeps the account's gear file current.
#[derive(Default)]
pub struct GearCache {
    /// Something changed the items since the last read.
    dirty: AtomicBool,
    logged_in: AtomicBool,
    /// The bank was taken into the file since it was last loaded.
    bank_taken: AtomicBool,
    inner: Mutex<Inner>,
}

#[derive(Default)]
struct Inner {
    last: Option<GearFile>,
    last_read: Option<Instant>,
}

impl GearCache {
    /// Reads the game at the next tick.
    pub fn mark_dirty(&self) {
        self.dirty.store(true, Ordering::Relaxed);
    }

    /// Logged in, as the login and logout events said (no game call).
    pub fn logged_in(&self) -> bool {
        self.logged_in.load(Ordering::Relaxed)
    }
}

pub(crate) fn start(bot: &Arc<Bot>) {
    let weak = Arc::downgrade(bot);
    // Runs inside `on_flash_call`: only marks.
    bot.events.subscribe(move |event| {
        let Some(bot) = weak.upgrade() else { return };
        let gear = &bot.gear;
        match event {
            BotEvent::Login { .. } => {
                gear.logged_in.store(true, Ordering::Relaxed);
                gear.bank_taken.store(false, Ordering::Relaxed);
                gear.mark_dirty();
            }
            BotEvent::Logout => gear.logged_in.store(false, Ordering::Relaxed),
            BotEvent::ItemAdded { .. } | BotEvent::ItemBought | BotEvent::ItemSold => {
                gear.mark_dirty()
            }
            _ => {}
        }
    });
    let weak = Arc::downgrade(bot);
    std::thread::Builder::new()
        .name("Turu Gear".into())
        .spawn(move || run(weak))
        .expect("couldn't start the gear thread");
}

fn run(weak: Weak<Bot>) {
    loop {
        std::thread::sleep(TICK);
        let Some(bot) = weak.upgrade() else { break };
        let gear = &bot.gear;
        if !gear.logged_in.load(Ordering::Relaxed) {
            continue;
        }
        let bank_new = bot.state.lock().bank_loaded && !gear.bank_taken.load(Ordering::Relaxed);
        let due = gear.dirty.load(Ordering::Relaxed)
            || bank_new
            || gear
                .inner
                .lock()
                .last_read
                .is_none_or(|t| t.elapsed() >= REFRESH);
        if due && let Err(e) = refresh(&bot) {
            tracing::debug!("Gear: {e}");
        }
    }
}

/// Reads the game and writes the file when it changed; the file. Never holds a lock across a
/// game call.
pub fn refresh(bot: &Bot) -> BotResult<Option<GearFile>> {
    let gear = &bot.gear;
    gear.dirty.store(false, Ordering::Relaxed);
    if !bot.flash.has_host() || !player::logged_in(bot)? || !player::world_loaded(bot)? {
        return Ok(None);
    }
    let username = player::username(bot)?;
    if username.trim().is_empty() {
        return Ok(None);
    }
    let inventory = inventory::inventory_items(bot)?;
    let bank_loaded = bot.state.lock().bank_loaded;
    let bank = if bank_loaded {
        Some(inventory::items(bot, Container::Bank)?)
    } else {
        None
    };
    let dir = bot.paths.gear_dir();
    let mut inner = gear.inner.lock();
    inner.last_read = Some(Instant::now());
    let earlier = match inner.last.take() {
        Some(last) if last.username.eq_ignore_ascii_case(&username) => Some(last),
        _ => read(&dir, &username),
    };
    let file = merge(
        &username,
        &inventory,
        bank.as_deref(),
        earlier.as_ref(),
        now_ms(),
    );
    if bank_loaded {
        gear.bank_taken.store(true, Ordering::Relaxed);
    }
    if earlier.as_ref().is_none_or(|e| !e.same_as(&file))
        && let Err(e) = write(&dir, &file)
    {
        tracing::warn!("Couldn't write the gear file of {username}: {e}");
    }
    inner.last = Some(file.clone());
    Ok(Some(file))
}

/// Why gear couldn't be changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GearError {
    /// A script runs (its file name), and stopping it wasn't asked for.
    ScriptRunning(String),
    NotLoggedIn,
    /// The script didn't stop in time.
    WontStop(String),
    Game(String),
}

impl GearError {
    /// The control API's status for it.
    pub fn status(&self) -> u16 {
        match self {
            Self::Game(_) => 500,
            _ => 409,
        }
    }
}

impl std::fmt::Display for GearError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ScriptRunning(name) => write!(f, "a script is running: {name}"),
            Self::NotLoggedIn => f.write_str("not logged in"),
            Self::WontStop(name) => write!(f, "{name} didn't stop"),
            Self::Game(e) => f.write_str(e),
        }
    }
}

impl From<crate::BotError> for GearError {
    fn from(e: crate::BotError) -> Self {
        Self::Game(e.to_string())
    }
}

/// The running script's file name.
fn script_name(bot: &Bot) -> String {
    bot.scripts
        .loaded()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "a script".into())
}

/// Checks that gear may change: logged in, and no script runs (or, with `stop`, stops it and
/// waits until it ended). The script stopped, if any.
pub fn prepare(bot: &Bot, stop: bool) -> Result<Option<String>, GearError> {
    let mut stopped = None;
    if bot.scripts.is_running() && !crate::control::is_script_thread() {
        let name = script_name(bot);
        if !stop {
            return Err(GearError::ScriptRunning(name));
        }
        bot.log(format!("Stopping {name} to change gear"));
        crate::script::stop(bot);
        let deadline = Instant::now() + STOP_TIMEOUT;
        while bot.scripts.is_running() {
            if Instant::now() >= deadline {
                return Err(GearError::WontStop(name));
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        stopped = Some(name);
    }
    if !bot.flash.has_host() || !player::logged_in(bot)? || !player::world_loaded(bot)? {
        return Err(GearError::NotLoggedIn);
    }
    Ok(stopped)
}

/// Opens and loads the bank as CoreBots does (opening it loads it too, in which case
/// `bank_load` sends nothing), and waits for the answer. True when it's loaded.
fn open_and_load_bank(bot: &Bot) -> BotResult<bool> {
    inventory::bank_open(bot)?;
    inventory::bank_load(bot, true)?;
    wait::for_true(bot, || Ok(bot.state.lock().bank_loaded), None, 20, 100)?;
    bot.gear.mark_dirty();
    Ok(bot.state.lock().bank_loaded)
}

/// `POST /bank/load`: loads the bank and writes the gear file.
pub fn load_bank(bot: &Bot, stop: bool) -> Result<(Option<GearFile>, Option<String>), GearError> {
    let stopped = prepare(bot, stop)?;
    if !open_and_load_bank(bot)? {
        return Err(GearError::Game("the bank didn't load".into()));
    }
    Ok((refresh(bot)?, stopped))
}

/// An item that couldn't be put on, and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Failure {
    pub item: String,
    pub reason: String,
}

/// What [`equip`] did.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct EquipReport {
    pub equipped: Vec<String>,
    /// Of `equipped`, those taken out of the bank first.
    pub from_bank: Vec<String>,
    pub already_worn: Vec<String>,
    /// Neither in the inventory nor in the bank (checked live, not from the file).
    pub missing: Vec<String>,
    pub failed: Vec<Failure>,
    /// The script stopped to do it.
    pub stopped_script: Option<String>,
}

impl EquipReport {
    /// "equipped Legion Revenant (from the bank); missing: Necrotic Sword".
    pub fn summary(&self) -> String {
        let mut parts = Vec::new();
        if let Some(name) = &self.stopped_script {
            parts.push(format!("stopped {name}"));
        }
        for name in &self.equipped {
            if self.from_bank.contains(name) {
                parts.push(format!("equipped {name} (from the bank)"));
            } else {
                parts.push(format!("equipped {name}"));
            }
        }
        if !self.already_worn.is_empty() {
            parts.push(format!("already worn: {}", self.already_worn.join(", ")));
        }
        if !self.missing.is_empty() {
            parts.push(format!("missing: {}", self.missing.join(", ")));
        }
        for failure in &self.failed {
            parts.push(format!("{}: {}", failure.item, failure.reason));
        }
        if parts.is_empty() {
            "nothing to do".into()
        } else {
            parts.join("; ")
        }
    }
}

/// Puts the items on, taking them out of the bank when they aren't in the inventory (the
/// bank is loaded for it unless it was this login). Refused while a script runs unless
/// `stop`; a script calling it on its own thread isn't refused.
pub fn equip(bot: &Bot, items: &[NameOrId], stop: bool) -> Result<EquipReport, GearError> {
    let mut report = EquipReport {
        stopped_script: prepare(bot, stop)?,
        ..Default::default()
    };
    for &which in items {
        let label = match which {
            NameOrId::Name(name) => name.trim().to_owned(),
            NameOrId::Id(id) => id.to_string(),
        };
        let item = match inventory::get_item(bot, Container::Inventory, which)? {
            Some(item) => Some((item, false)),
            None => {
                if !bot.state.lock().bank_loaded && !open_and_load_bank(bot)? {
                    report.failed.push(Failure {
                        item: label,
                        reason: "the bank didn't load".into(),
                    });
                    continue;
                }
                inventory::get_item(bot, Container::Bank, which)?.map(|item| (item, true))
            }
        };
        let Some((item, in_bank)) = item else {
            report.missing.push(label);
            continue;
        };
        let name = item.name.trim().to_owned();
        if item.equipped {
            report.already_worn.push(name);
            continue;
        }
        if Slot::of(&item.equip_slot).is_none() {
            report.failed.push(Failure {
                item: name,
                reason: "can't be worn".into(),
            });
            continue;
        }
        if in_bank {
            if !inventory::has_space_for(bot, &item, 1)? {
                report.failed.push(Failure {
                    item: name,
                    reason: "in the bank, and the bag is full".into(),
                });
                continue;
            }
            if !inventory::bank_to_inventory(bot, NameOrId::Id(item.id))? {
                report.failed.push(Failure {
                    item: name,
                    reason: "didn't come out of the bank".into(),
                });
                continue;
            }
        }
        if inventory::equip_item(bot, NameOrId::Id(item.id))? {
            if in_bank {
                report.from_bank.push(name.clone());
            }
            report.equipped.push(name);
        } else {
            report.failed.push(Failure {
                item: name,
                reason: "the game didn't equip it".into(),
            });
        }
    }
    bot.gear.mark_dirty();
    Ok(report)
}

/// A control API `item` argument: an ID when it's a number, else a name.
pub fn name_or_id(text: &str) -> NameOrId<'_> {
    match text.trim().parse::<i64>() {
        Ok(id) if id > 0 => NameOrId::Id(id),
        _ => NameOrId::Name(text),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: i64, name: &str, slot: &str, equipped: bool) -> ItemBase {
        ItemBase {
            id,
            name: name.into(),
            equip_slot: slot.into(),
            equipped,
            ..Default::default()
        }
    }

    #[test]
    fn slots_come_from_the_equip_slot() {
        assert_eq!(Slot::of("ar"), Some(Slot::Class));
        assert_eq!(Slot::of("Weapon"), Some(Slot::Weapon));
        assert_eq!(Slot::of("co"), Some(Slot::Armor));
        assert_eq!(Slot::of(""), None);
        assert!(GearItem::from_item(&item(1, "Bone", "", false), Place::Inventory).is_none());
    }

    #[test]
    fn keeps_the_last_bank_until_it_is_loaded_again() {
        let inventory = [item(1, "Legion Revenant", "ar", true)];
        let bank = [item(2, "Necrotic Sword", "Weapon", false)];
        let first = merge("Alt", &inventory, Some(&bank), None, 100);
        assert_eq!(first.bank_as_of, Some(100));
        assert_eq!(first.items.len(), 2);
        assert_eq!(
            first.worn(Slot::Class).map(|i| i.name.as_str()),
            Some("Legion Revenant")
        );

        // Not loaded this login: the bank of the last file, with its date.
        let later = merge("alt", &inventory, None, Some(&first), 200);
        assert_eq!(later.bank_as_of, Some(100));
        assert_eq!(later.for_slot(Slot::Weapon)[0].place, Place::Bank);
        assert_eq!(later.items, first.items);

        // Taken out: no longer listed in the bank.
        let moved = [
            inventory[0].clone(),
            item(2, "Necrotic Sword", "Weapon", true),
        ];
        let after = merge("Alt", &moved, None, Some(&first), 300);
        let swords = after.for_slot(Slot::Weapon);
        assert_eq!(swords.len(), 1);
        assert_eq!(swords[0].place, Place::Inventory);

        // Loaded again: replaced, newly dated.
        let reloaded = merge("Alt", &inventory, Some(&[]), Some(&first), 400);
        assert_eq!(reloaded.bank_as_of, Some(400));
        assert!(reloaded.for_slot(Slot::Weapon).is_empty());

        // Another account's file isn't used.
        let other = merge("Main", &inventory, None, Some(&first), 500);
        assert_eq!(other.bank_as_of, None);
    }

    #[test]
    fn files_round_trip_and_are_named_safely() {
        let dir = std::env::temp_dir().join(format!("turu-gear-{}", std::process::id()));
        let file = merge(
            "My Alt",
            &[item(1, "Legion Revenant", "ar", true)],
            None,
            None,
            1,
        );
        write(&dir, &file).unwrap();
        assert_eq!(path_for(&dir, "My Alt").file_name().unwrap(), "my_alt.json");
        assert_eq!(read(&dir, "MY ALT"), Some(file));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn reports_read_well() {
        let report = EquipReport {
            equipped: vec!["Legion Revenant".into(), "Necrotic Sword".into()],
            from_bank: vec!["Necrotic Sword".into()],
            missing: vec!["Hollowborn Reaper".into()],
            stopped_script: Some("GoldFarm.ts".into()),
            ..Default::default()
        };
        assert_eq!(
            report.summary(),
            "stopped GoldFarm.ts; equipped Legion Revenant; equipped Necrotic Sword (from the \
             bank); missing: Hollowborn Reaper"
        );
        assert_eq!(EquipReport::default().summary(), "nothing to do");
    }
}
