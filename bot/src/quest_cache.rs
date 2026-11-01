//! The quest cache of Tools > Loader (VibeSkua `QuestDataLoaderService`): `QuestData.json` in
//! the data folder, in VibeSkua's format (a VibeSkua file can be copied over), filled by
//! loading quests into the client 29 at a time, or by downloading the community cache
//! (VibeSkua `UpdateQuestDataFile`).

use crate::Bot;
use crate::api::{player, quests};
use crate::error::{BotError, BotResult};
use crate::models::{lenient_bool, lenient_f64, lenient_i64, lenient_string};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Value, json};
use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// The cache's file name in the data folder (VibeSkua `ClientFileSources.SkuaQuestsFile`).
pub const FILE_NAME: &str = "QuestData.json";

/// Quests loaded per request.
pub const CHUNK: i64 = 29;

/// Add missing and Rebuild all stop before this ID.
pub const LAST_ID: i64 = 13000;

/// The pause between two requests (ms).
const CHUNK_DELAY: u64 = 1500;

/// Tries of 100 ms to wait for a chunk (VibeSkua `Wait.ForQuestLoad(start, end, 100)`).
const LOAD_TRIES: u32 = 100;

fn opt_string<'de, D: Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    Ok(match Value::deserialize(d)? {
        Value::Null => None,
        Value::String(s) => Some(s),
        other => Some(other.to_string()),
    })
}

fn opt_f64<'de, D: Deserializer<'de>>(d: D) -> Result<Option<f64>, D::Error> {
    lenient_f64(d).map(Some)
}

fn opt_i64<'de, D: Deserializer<'de>>(d: D) -> Result<Option<i64>, D::Error> {
    lenient_i64(d).map(Some)
}

/// VibeSkua's `StringBoolConverter`: `"1"` / `"0"`.
#[allow(clippy::trivially_copy_pass_by_ref)] // serde's `serialize_with` passes a reference.
fn string_bool<S: Serializer>(value: &bool, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(if *value { "1" } else { "0" })
}

/// VibeSkua `ItemCategory` (names, and the `EnumMember` text written for some).
const CATEGORIES: [(&str, &str); 31] = [
    ("Sword", "Sword"),
    ("Axe", "Axe"),
    ("Dagger", "Dagger"),
    ("Gun", "Gun"),
    ("HandGun", "HandGun"),
    ("Rifle", "Rifle"),
    ("Bow", "Bow"),
    ("Mace", "Mace"),
    ("Gauntlet", "Gauntlet"),
    ("Polearm", "Polearm"),
    ("Staff", "Staff"),
    ("Wand", "Wand"),
    ("Whip", "Whip"),
    ("Class", "Class"),
    ("Armor", "Armor"),
    ("Helm", "Helm"),
    ("Cape", "Cape"),
    ("Pet", "Pet"),
    ("Amulet", "Amulet"),
    ("Necklace", "Necklace"),
    ("Note", "Note"),
    ("Resource", "Resource"),
    ("Item", "Item"),
    ("Misc", "Misc"),
    ("QuestItem", "Quest Item"),
    ("ServerUse", "Server Use"),
    ("House", "House"),
    ("WallItem", "Wall Item"),
    ("FloorItem", "Floor Item"),
    ("Enhancement", "Enhancement"),
    ("Unknown", "Unknown"),
];

/// VibeSkua `ItemBase.Category` as written to the file: `sType` parsed as an `ItemCategory`
/// (ignoring case), else "Unknown".
pub fn category_of(kind: Option<&str>) -> String {
    let kind = kind.unwrap_or_default().trim();
    CATEGORIES
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(kind))
        .map_or("Unknown", |(_, text)| text)
        .to_owned()
}

/// An item of a cached quest (VibeSkua `ItemBase`; `SimpleReward` adds `iRate` and `iType`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct QuestDataItem {
    #[serde(
        rename = "iRate",
        default,
        deserialize_with = "opt_f64",
        skip_serializing_if = "Option::is_none"
    )]
    pub rate: Option<f64>,
    #[serde(
        rename = "iType",
        default,
        deserialize_with = "opt_i64",
        skip_serializing_if = "Option::is_none"
    )]
    pub kind: Option<i64>,
    #[serde(rename = "ItemID", default, deserialize_with = "lenient_i64")]
    pub id: i64,
    #[serde(rename = "sName", default, deserialize_with = "opt_string")]
    pub name: Option<String>,
    #[serde(rename = "sDesc", default, deserialize_with = "opt_string")]
    pub description: Option<String>,
    #[serde(rename = "iQty", default, deserialize_with = "lenient_i64")]
    pub quantity: i64,
    #[serde(rename = "iStk", default, deserialize_with = "lenient_i64")]
    pub max_stack: i64,
    #[serde(
        rename = "bUpg",
        default,
        deserialize_with = "lenient_bool",
        serialize_with = "string_bool"
    )]
    pub upgrade: bool,
    #[serde(
        rename = "bWear",
        default,
        deserialize_with = "lenient_bool",
        serialize_with = "string_bool"
    )]
    pub wearing: bool,
    #[serde(
        rename = "bCoins",
        default,
        deserialize_with = "lenient_bool",
        serialize_with = "string_bool"
    )]
    pub coins: bool,
    #[serde(rename = "sType", default, deserialize_with = "opt_string")]
    pub category_string: Option<String>,
    #[serde(rename = "EnhPatternID", default, deserialize_with = "lenient_i64")]
    pub enhancement_pattern_id: i64,
    #[serde(rename = "ProcID", default, deserialize_with = "lenient_i64")]
    pub proc_id: i64,
    #[serde(
        rename = "bHouse",
        default,
        deserialize_with = "lenient_bool",
        serialize_with = "string_bool"
    )]
    pub house: bool,
    /// Worked out from `sType` ([`category_of`]); read back from the file as written.
    #[serde(rename = "Category", default, deserialize_with = "lenient_string")]
    pub category: String,
    #[serde(
        rename = "bTemp",
        default,
        deserialize_with = "lenient_bool",
        serialize_with = "string_bool"
    )]
    pub temp: bool,
    #[serde(rename = "sES", default, deserialize_with = "opt_string")]
    pub item_group: Option<String>,
    #[serde(rename = "sLink", default, deserialize_with = "opt_string")]
    pub file_name: Option<String>,
    #[serde(rename = "sFile", default, deserialize_with = "opt_string")]
    pub file_link: Option<String>,
    #[serde(rename = "sMeta", default, deserialize_with = "opt_string")]
    pub meta: Option<String>,
}

/// A cached quest (VibeSkua `QuestData`), field for field as VibeSkua writes it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct QuestData {
    #[serde(rename = "ID", default, deserialize_with = "lenient_i64")]
    pub id: i64,
    #[serde(rename = "Slot", default, deserialize_with = "lenient_i64")]
    pub slot: i64,
    #[serde(rename = "Value", default, deserialize_with = "lenient_i64")]
    pub value: i64,
    #[serde(rename = "Name", default, deserialize_with = "lenient_string")]
    pub name: String,
    #[serde(rename = "Once", default, deserialize_with = "lenient_bool")]
    pub once: bool,
    #[serde(rename = "Field", default, deserialize_with = "opt_string")]
    pub field: Option<String>,
    #[serde(rename = "Index", default, deserialize_with = "lenient_i64")]
    pub index: i64,
    #[serde(rename = "Upgrade", default, deserialize_with = "lenient_bool")]
    pub upgrade: bool,
    #[serde(rename = "Level", default, deserialize_with = "lenient_i64")]
    pub level: i64,
    #[serde(rename = "RequiredClassID", default, deserialize_with = "lenient_i64")]
    pub required_class_id: i64,
    #[serde(
        rename = "RequiredClassPoints",
        default,
        deserialize_with = "lenient_i64"
    )]
    pub required_class_points: i64,
    #[serde(
        rename = "RequiredFactionId",
        default,
        deserialize_with = "lenient_i64"
    )]
    pub required_faction_id: i64,
    #[serde(
        rename = "RequiredFactionRep",
        default,
        deserialize_with = "lenient_i64"
    )]
    pub required_faction_rep: i64,
    #[serde(rename = "Gold", default, deserialize_with = "lenient_i64")]
    pub gold: i64,
    #[serde(rename = "XP", default, deserialize_with = "lenient_i64")]
    pub xp: i64,
    #[serde(rename = "AcceptRequirements", default)]
    pub accept_requirements: Vec<QuestDataItem>,
    #[serde(rename = "Requirements", default)]
    pub requirements: Vec<QuestDataItem>,
    #[serde(rename = "Rewards", default)]
    pub rewards: Vec<QuestDataItem>,
    #[serde(rename = "SimpleRewards", default)]
    pub simple_rewards: Vec<QuestDataItem>,
}

/// What the Loader lists of a cached quest: the file read without the item lists.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct QuestSummary {
    #[serde(rename = "ID", default, deserialize_with = "lenient_i64")]
    pub id: i64,
    #[serde(rename = "Name", default, deserialize_with = "lenient_string")]
    pub name: String,
}

impl From<&QuestData> for QuestSummary {
    fn from(quest: &QuestData) -> Self {
        Self {
            id: quest.id,
            name: quest.name.clone(),
        }
    }
}

/// The items of a `{ "<ItemID>": item }` object or an array, with the ID taken from the key
/// when the item lacks `ItemID`.
fn items_of(value: Option<&Value>) -> Vec<QuestDataItem> {
    let parse = |key: Option<&str>, v: &Value| {
        let mut item = serde_json::from_value::<QuestDataItem>(v.clone()).ok()?;
        if item.id == 0 {
            item.id = key.and_then(|k| k.trim().parse().ok()).unwrap_or_default();
        }
        item.name = item.name.map(|n| n.trim().to_owned());
        item.category = category_of(item.category_string.as_deref());
        item.rate = None;
        item.kind = None;
        Some(item)
    };
    match value {
        Some(Value::Object(map)) => map
            .iter()
            .filter(|(_, v)| v.is_object())
            .filter_map(|(k, v)| parse(Some(k), v))
            .collect(),
        Some(Value::Array(items)) => items
            .iter()
            .filter(|v| v.is_object())
            .filter_map(|v| parse(None, v))
            .collect(),
        _ => Vec::new(),
    }
}

impl QuestData {
    /// A quest of `world.questTree` (the game's `QuestID`, `sName`, `oItems` + `turnin`, ...)
    /// as VibeSkua caches it (`ConvertToQuestData`). `None` when it isn't a quest.
    pub fn from_raw(raw: &Value) -> Option<Self> {
        if !raw.is_object() {
            return None;
        }
        let int = |key: &str| {
            raw.get(key)
                .cloned()
                .map(|v| lenient_i64(v).unwrap_or(0))
                .unwrap_or(0)
        };
        let flag = |key: &str| {
            raw.get(key)
                .cloned()
                .map(|v| lenient_bool(v).unwrap_or(false))
                .unwrap_or(false)
        };
        let id = int("QuestID");
        if id == 0 {
            return None;
        }
        // `Requirements`: `oItems` with the quantities of `turnin`.
        let turnin = items_of(raw.get("turnin"));
        let requirements = items_of(raw.get("oItems"))
            .into_iter()
            .map(|mut item| {
                if let Some(t) = turnin.iter().find(|t| t.id == item.id) {
                    item.quantity = t.quantity;
                }
                item
            })
            .collect();
        // `oRewards` is `{ "<type>": { "<ItemID>": item } }` (QuestRewardConverter).
        let rewards = match raw.get("oRewards") {
            Some(Value::Object(groups)) => groups
                .values()
                .flat_map(|group| items_of(Some(group)))
                .collect(),
            Some(Value::Array(groups)) => groups
                .iter()
                .flat_map(|group| items_of(Some(group)))
                .collect(),
            _ => Vec::new(),
        };
        let simple_rewards = match raw.get("reward") {
            Some(Value::Array(entries)) => entries
                .iter()
                .filter(|v| v.is_object())
                .filter_map(|v| {
                    let mut item = items_of(Some(&Value::Array(vec![v.clone()]))).pop()?;
                    item.rate = Some(
                        v.get("iRate")
                            .cloned()
                            .map(|r| lenient_f64(r).unwrap_or(0.0))
                            .unwrap_or(0.0),
                    );
                    item.kind = Some(
                        v.get("iType")
                            .cloned()
                            .map(|t| lenient_i64(t).unwrap_or(0))
                            .unwrap_or(0),
                    );
                    Some(item)
                })
                .collect(),
            _ => Vec::new(),
        };
        Some(Self {
            id,
            slot: int("iSlot"),
            value: int("iValue"),
            name: raw
                .get("sName")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .trim()
                .to_owned(),
            once: flag("bOnce"),
            field: raw
                .get("sField")
                .and_then(|f| f.as_str().map(str::to_owned)),
            index: int("iIndex"),
            upgrade: flag("bUpg"),
            level: int("iLvl"),
            required_class_id: int("iClass"),
            required_class_points: int("iReqCP"),
            required_faction_id: int("FactionID"),
            required_faction_rep: int("iReqRep"),
            gold: int("iGold"),
            xp: int("iExp"),
            accept_requirements: items_of(raw.get("oReqd")),
            requirements,
            rewards,
            simple_rewards,
        })
    }
}

/// The cache file's quests; none when there is no file (VibeSkua `GetFromFileAsync`).
pub fn read(path: &Path) -> io::Result<Vec<QuestData>> {
    match fs::read_to_string(path) {
        Ok(text) if text.trim().is_empty() => Ok(Vec::new()),
        Ok(text) => serde_json::from_str::<Option<Vec<QuestData>>>(&text)
            .map(Option::unwrap_or_default)
            .map_err(io::Error::other),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(e),
    }
}

/// The cache file's quest IDs and names, in file order (what the Loader lists).
pub fn read_summaries(path: &Path) -> io::Result<Vec<QuestSummary>> {
    match fs::read_to_string(path) {
        Ok(text) if text.trim().is_empty() => Ok(Vec::new()),
        Ok(text) => serde_json::from_str::<Option<Vec<QuestSummary>>>(&text)
            .map(Option::unwrap_or_default)
            .map_err(io::Error::other),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(e),
    }
}

/// Writes the quests, indented, as VibeSkua does (through a temporary file, so a crash
/// doesn't leave half a cache).
pub fn write(path: &Path, quests: &[QuestData]) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let text = serde_json::to_string_pretty(quests).map_err(io::Error::other)?;
    let temp = path.with_extension("json.tmp");
    fs::write(&temp, text)?;
    fs::rename(&temp, path)
}

/// `fetched` with the `existing` quests it lacks, sorted by ID: a quest loaded again replaces
/// its cached copy.
pub fn merge(fetched: Vec<QuestData>, existing: Vec<QuestData>) -> Vec<QuestData> {
    let mut seen = HashSet::new();
    let mut quests: Vec<QuestData> = fetched
        .into_iter()
        .chain(existing)
        .filter(|q| seen.insert(q.id))
        .collect();
    quests.sort_by_key(|q| q.id);
    quests
}

/// The Load box's IDs ("2566" or "101, 102"): digits, commas and spaces only (VibeSkua
/// `AllDigits`, which enables Load), split on commas and spaces.
pub fn parse_ids(text: &str) -> Option<Vec<i64>> {
    let digits: String = text.chars().filter(|c| *c != ',' && *c != ' ').collect();
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some(
        text.split([',', ' '])
            .filter(|p| !p.is_empty())
            .filter_map(|p| p.parse().ok())
            .collect(),
    )
}

/// The Load box's shop ID: a single number (VibeSkua `int.TryParse`).
pub fn parse_shop_id(text: &str) -> Option<i64> {
    parse_ids(text)?;
    text.trim().parse::<i32>().ok().map(i64::from)
}

/// The range box ("2566, 2599"), with VibeSkua's messages when it's wrong.
pub fn parse_range(text: &str) -> Result<(i64, i64), &'static str> {
    let parts: Vec<&str> = text.split([',', ' ']).filter(|p| !p.is_empty()).collect();
    let [start, end] = parts[..] else {
        return Err("Invalid format. Use: startID, endID");
    };
    let (Ok(start), Ok(end)) = (start.parse::<i32>(), end.parse::<i32>()) else {
        return Err("Invalid format. Use: startID, endID");
    };
    if start > end {
        return Err("Start ID must be less than or equal to End ID");
    }
    Ok((start.into(), end.into()))
}

/// What Update cache does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateMode {
    /// Add missing: loads the quests after the last cached one and keeps the cache.
    AddMissing,
    /// Rebuild all: loads every quest from 1 and replaces the cache.
    RebuildAll,
    /// Update range: loads `start..=end` into the cache.
    Range(i64, i64),
}

/// The ID ranges loaded one request each: `CHUNK` IDs from `start` while the first is
/// `<= last`; with `clip`, the last range ends at `last` (Update range), otherwise every range
/// is a whole chunk (Add missing / Rebuild all).
pub fn chunks(start: i64, last: i64, clip: bool) -> Vec<(i64, i64)> {
    let mut ranges = Vec::new();
    let mut i = start;
    while i <= last {
        let end = if clip {
            (i + CHUNK - 1).min(last)
        } else {
            i + CHUNK - 1
        };
        ranges.push((i, end));
        i += CHUNK;
    }
    ranges
}

/// Sleeps `ms` in short steps; false when `cancel` was set meanwhile.
fn nap(bot: &Bot, ms: u64, cancel: &AtomicBool) -> BotResult<bool> {
    let mut left = ms;
    while left > 0 {
        if cancel.load(Ordering::Acquire) {
            return Ok(false);
        }
        let step = left.min(100);
        bot.sleep(step)?;
        left -= step;
    }
    Ok(!cancel.load(Ordering::Acquire))
}

/// The raw quests of `world.questTree`.
fn raw_tree(bot: &Bot) -> BotResult<Vec<Value>> {
    Ok(match bot.flash.get_opt::<Value>("world.questTree")? {
        Some(Value::Object(map)) => map
            .into_iter()
            .map(|(_, v)| v)
            .filter(|v| v.get("QuestID").is_some())
            .collect(),
        Some(Value::Array(items)) => items
            .into_iter()
            .filter(|v| v.get("QuestID").is_some())
            .collect(),
        _ => Vec::new(),
    })
}

fn in_range(raw: &Value, start: i64, end: i64) -> bool {
    let id = raw
        .get("QuestID")
        .cloned()
        .map(|v| lenient_i64(v).unwrap_or(0))
        .unwrap_or(0);
    (start..=end).contains(&id)
}

/// Loads each range into the client and reads its quests, stopping at the first range that
/// loads nothing ("No more quests found.") or when `cancel` is set. Returns the quests read.
fn fetch(
    bot: &Bot,
    ranges: &[(i64, i64)],
    whole_chunks: bool,
    progress: &dyn Fn(String),
    cancel: &AtomicBool,
) -> BotResult<Vec<QuestData>> {
    let mut fetched = Vec::new();
    for &(start, end) in ranges {
        if cancel.load(Ordering::Acquire) {
            break;
        }
        bot.flash.set("world.questTree", json!({}))?;
        // VibeSkua's text for a whole chunk names one ID past it.
        let shown_end = if whole_chunks { start + CHUNK } else { end };
        progress(format!("Loading Quests {start}-{shown_end}..."));
        quests::load(bot, &(start..=end).collect::<Vec<_>>())?;
        // VibeSkua `Wait.ForQuestLoad(start, end, 100)`.
        let initial = raw_tree(bot)?.len();
        let mut loaded = false;
        for _ in 0..=LOAD_TRIES {
            let tree = raw_tree(bot)?;
            if !player::playing(bot)?
                || tree.len() > initial
                || tree.iter().any(|q| in_range(q, start, end))
            {
                loaded = true;
                break;
            }
            if !nap(bot, 100, cancel)? {
                break;
            }
        }
        if !loaded {
            // Cancel during the wait ends quietly; VibeSkua's wait couldn't be cancelled.
            if !cancel.load(Ordering::Acquire) {
                progress("No more quests found.".into());
            }
            break;
        }
        let quests: Vec<QuestData> = raw_tree(bot)?
            .iter()
            .filter(|q| in_range(q, start, end))
            .filter_map(QuestData::from_raw)
            .collect();
        if quests.is_empty() {
            progress("No more quests found.".into());
            break;
        }
        fetched.extend(quests);
        nap(bot, CHUNK_DELAY, cancel)?;
    }
    Ok(fetched)
}

fn io_error(path: &Path, e: io::Error) -> BotError {
    BotError::Other(format!("{}: {e}", path.display()))
}

/// Updates the cache file at `path` from the game (VibeSkua `UpdateAsync` / `UpdateRangeAsync`)
/// and returns what it holds afterwards. Logged out, it only reads the file. `progress` gets
/// VibeSkua's progress texts; `cancel` stops after the current request, keeping what was read.
pub fn update(
    bot: &Bot,
    path: &Path,
    mode: UpdateMode,
    progress: &dyn Fn(String),
    cancel: &AtomicBool,
) -> BotResult<Vec<QuestData>> {
    let existing = read(path).map_err(|e| io_error(path, e))?;
    if !player::logged_in(bot)? {
        return Ok(existing);
    }
    let quests = match mode {
        UpdateMode::AddMissing => {
            let start = existing
                .iter()
                .map(|q| q.id)
                .max()
                .map_or(1, |last| last + 1);
            let fetched = fetch(
                bot,
                &chunks(start, LAST_ID - 1, false),
                true,
                progress,
                cancel,
            )?;
            merge(fetched, existing)
        }
        UpdateMode::RebuildAll => {
            let fetched = fetch(bot, &chunks(1, LAST_ID - 1, false), true, progress, cancel)?;
            if !cancel.load(Ordering::Acquire) {
                merge(fetched, Vec::new())
            } else if fetched.is_empty() {
                progress("Update cancelled - keeping existing quest data".into());
                return Ok(existing);
            } else {
                // Cancelled part way: the new quests, then the cached ones after them.
                let last = fetched.iter().map(|q| q.id).max().unwrap_or_default();
                let newer = existing.into_iter().filter(|q| q.id > last).collect();
                let quests = merge(fetched, newer);
                progress(format!(
                    "Update cancelled - saved {} quests (partial data + existing)",
                    quests.len()
                ));
                quests
            }
        }
        UpdateMode::Range(start, end) => {
            let fetched = fetch(bot, &chunks(start, end, true), false, progress, cancel)?;
            merge(fetched, existing)
        }
    };
    write(path, &quests).map_err(|e| io_error(path, e))?;
    progress(format!("Getting quests from file {FILE_NAME}"));
    Ok(quests)
}

/// The cached quests whose name or ID contains `query` (ignoring case), as indices into
/// `quests`; all of them for an empty query.
pub fn search(quests: &[QuestSummary], query: &str) -> Vec<usize> {
    let query = query.trim().to_lowercase();
    quests
        .iter()
        .enumerate()
        .filter(|(_, q)| {
            query.is_empty()
                || q.name.to_lowercase().contains(&query)
                || q.id.to_string().contains(&query)
        })
        .map(|(i, _)| i)
        .collect()
}

/// Copy IDs: "1,2,3".
pub fn copy_ids(quests: &[&QuestSummary]) -> String {
    quests
        .iter()
        .map(|q| q.id.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

/// Copy names: "A,B".
pub fn copy_names(quests: &[&QuestSummary]) -> String {
    quests
        .iter()
        .map(|q| q.name.as_str())
        .collect::<Vec<_>>()
        .join(",")
}

/// Copy as `// Name | ID`, one line each.
pub fn copy_comments(quests: &[&QuestSummary]) -> String {
    quests
        .iter()
        .map(|q| format!("// {} | {}", q.name, q.id))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The community quest cache VibeSkua downloads (`auqw/Scripts`, Skua branch).
pub const COMMUNITY_URL: &str =
    "https://raw.githubusercontent.com/auqw/Scripts/refs/heads/Skua/QuestData.json";

/// What the download request answered.
pub enum Fetched {
    /// Same as the ETag sent.
    Unchanged,
    File {
        body: Vec<u8>,
        etag: Option<String>,
    },
}

/// Where Download gets the cache (the community file, or a fake one in tests).
pub trait QuestSource: Send + Sync {
    /// The file, or [`Fetched::Unchanged`] when it still has `etag`.
    fn fetch(&self, etag: Option<&str>) -> Result<Fetched, String>;
}

/// The cache at a URL, over HTTP (gzip, so the 20 MB file is about 1 MB on the wire).
pub struct HttpSource {
    url: String,
    client: reqwest::blocking::Client,
}

impl HttpSource {
    pub fn new(url: &str) -> Self {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(120))
            .user_agent(concat!("Turu/", env!("CARGO_PKG_VERSION")))
            .build()
            .unwrap_or_else(|_| reqwest::blocking::Client::new());
        Self {
            url: url.to_owned(),
            client,
        }
    }

    /// [`COMMUNITY_URL`], or the URL in `TURU_QUEST_DATA_URL`.
    pub fn from_env() -> Self {
        let url = std::env::var("TURU_QUEST_DATA_URL")
            .ok()
            .filter(|u| u.trim().starts_with("http"))
            .unwrap_or_else(|| COMMUNITY_URL.to_owned());
        Self::new(url.trim())
    }
}

impl QuestSource for HttpSource {
    fn fetch(&self, etag: Option<&str>) -> Result<Fetched, String> {
        let mut request = self.client.get(&self.url);
        if let Some(etag) = etag {
            request = request.header(reqwest::header::IF_NONE_MATCH, etag);
        }
        let response = request.send().map_err(|e| format!("{}: {e}", self.url))?;
        if response.status() == reqwest::StatusCode::NOT_MODIFIED {
            return Ok(Fetched::Unchanged);
        }
        let response = response
            .error_for_status()
            .map_err(|e| format!("{}: {e}", self.url))?;
        let etag = response
            .headers()
            .get(reqwest::header::ETAG)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        let body = response
            .bytes()
            .map_err(|e| format!("{}: {e}", self.url))?
            .to_vec();
        Ok(Fetched::File { body, etag })
    }
}

/// What a download did.
#[derive(Debug, Clone, PartialEq)]
pub enum Downloaded {
    /// The community file hasn't changed since the last download; the cache is as it was.
    Unchanged,
    /// The cache now holds the downloaded quests plus `kept` quests only the local file had.
    Updated {
        quests: Vec<QuestData>,
        downloaded: usize,
        kept: usize,
    },
}

/// The ETag of the last download, next to the cache (`QuestData.json.etag`).
fn etag_path(path: &Path) -> std::path::PathBuf {
    path.with_extension("json.etag")
}

/// VibeSkua `UpdateQuestDataFile`: downloads the community cache and merges it into the file
/// at `path` (a downloaded quest replaces the local copy; quests only the local file has are
/// kept). Sends the last download's ETag while the file exists, so an unchanged file costs an
/// empty answer. A local file that can't be read is replaced.
pub fn download(path: &Path, source: &dyn QuestSource) -> Result<Downloaded, String> {
    let etag_file = etag_path(path);
    let etag = path
        .exists()
        .then(|| fs::read_to_string(&etag_file).ok())
        .flatten();
    let (body, etag) = match source.fetch(etag.as_deref().map(str::trim))? {
        Fetched::Unchanged => return Ok(Downloaded::Unchanged),
        Fetched::File { body, etag } => (body, etag),
    };
    let remote: Vec<QuestData> = serde_json::from_slice::<Option<Vec<QuestData>>>(&body)
        .map_err(|e| format!("The downloaded quest cache isn't valid: {e}"))?
        .unwrap_or_default();
    let local = read(path).unwrap_or_default();
    let remote_ids: HashSet<i64> = remote.iter().map(|q| q.id).collect();
    let downloaded = remote_ids.len();
    let kept = local.iter().filter(|q| !remote_ids.contains(&q.id)).count();
    let quests = merge(remote, local);
    write(path, &quests).map_err(|e| format!("{}: {e}", path.display()))?;
    match etag {
        Some(etag) => {
            let _ = fs::write(&etag_file, etag);
        }
        None => {
            let _ = fs::remove_file(&etag_file);
        }
    }
    Ok(Downloaded::Updated {
        quests,
        downloaded,
        kept,
    })
}
