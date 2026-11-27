//! The world fake's moving parts (docs/REPLAY_TESTS_PROPOSAL.md, phase 4): monsters that die and
//! come back, items that arrive, quests that are turned in. Each is there because the draft run
//! showed drafts stuck without it: most were hunting for an item no monster ever dropped.
//!
//! Monster names don't cross the Flash boundary (a kill is `attackMonsterID`), so they come from a
//! catalog the run builds from the scripts' own `huntMonster` / `killMonster` / `killQuest` calls,
//! not from a tape. Item names do: a hunt asks for its item by name on every pass
//! (`selectArrayObjectsWhere` on the inventory), so a kill drops what the draft has been asking
//! for, and what its quests in progress still need.
//!
//! Shops' contents don't cross it either until a shop loads, so they come from the merge scripts:
//! each names its shop (`startBuyAllMerge("map", id, ...)`), lists the shop's items in its options
//! and names the materials it farms (`findIngredients`' cases).

use crate::Bot;
use crate::control::ClockTime;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::time::Duration;

/// How long a dead monster stays dead (the game's `mtls` came about 2 s after a kill on tape).
pub(super) const RESPAWN: Duration = Duration::from_secs(2);
/// A monster dies on the third hit.
const HITS_TO_KILL: i64 = 3;
/// What a kill drops: the items asked for in the last few seconds.
const WANTED_FOR: Duration = Duration::from_secs(10);
/// A monster's HP (any number: three hits kill it).
const MONSTER_HP: i64 = 3000;

#[derive(Debug, Clone)]
pub(super) struct Monster {
    pub id: i64,
    pub name: String,
    pub hp: i64,
    pub died_at: Option<ClockTime>,
}

impl Monster {
    pub fn new(id: i64, name: &str) -> Self {
        Self {
            id,
            name: name.to_owned(),
            hp: MONSTER_HP,
            died_at: None,
        }
    }

    pub fn alive(&self) -> bool {
        self.died_at.is_none()
    }

    /// As turu.swf writes one (`getMonsters`), standing in `cell`.
    pub fn json(&self, cell: &str) -> Value {
        json!({
            "MonMapID": self.id,
            "MonID": self.id,
            "strMonName": self.name,
            "strLinkage": self.name,
            "intHP": self.hp,
            "intHPMax": MONSTER_HP,
            "intState": if self.alive() { 1 } else { 0 },
            "strFrame": cell,
            "intLevel": 1,
            "sRace": "None",
            "strBehave": "walk",
        })
    }
}

/// The monsters of the map you're in, and your target.
#[derive(Debug, Default)]
pub(super) struct Combat {
    pub monsters: Vec<Monster>,
    pub target: Option<i64>,
}

impl Combat {
    /// The dead whose time is up come back. Without a clock (a test with no bot), at once.
    pub fn respawn(&mut self, now: Option<ClockTime>) {
        for m in &mut self.monsters {
            if let Some(died) = m.died_at
                && now.is_none_or(|now| now.since(died) >= RESPAWN)
            {
                m.died_at = None;
                m.hp = MONSTER_HP;
            }
        }
    }

    /// A new map: its monsters, by name.
    pub fn enter(&mut self, names: &[String]) {
        self.monsters = names
            .iter()
            .enumerate()
            .map(|(i, name)| Monster::new(i as i64 + 1, name))
            .collect();
        self.target = None;
    }

    pub fn target(&self) -> Option<&Monster> {
        let id = self.target?;
        self.monsters.iter().find(|m| m.id == id && m.alive())
    }

    /// Targets a living monster: by ID, or the first with that name (`*` any).
    pub fn attack(&mut self, id: Option<i64>, name: Option<&str>) -> bool {
        let found = self.monsters.iter().find(|m| {
            m.alive()
                && match (id, name) {
                    (Some(id), _) => m.id == id,
                    (None, Some(name)) => name == "*" || m.name.eq_ignore_ascii_case(name.trim()),
                    _ => false,
                }
        });
        self.target = found.map(|m| m.id);
        self.target.is_some()
    }

    /// A hit on the target: the ID of the monster it killed, if it did.
    pub fn hit(&mut self, now: Option<ClockTime>) -> Option<i64> {
        let id = self.target?;
        let monster = self.monsters.iter_mut().find(|m| m.id == id && m.alive())?;
        monster.hp -= MONSTER_HP / HITS_TO_KILL;
        if monster.hp > 0 {
            return None;
        }
        monster.hp = 0;
        monster.died_at = Some(now.unwrap_or_default());
        self.target = None;
        Some(id)
    }
}

/// An item a script asked for (`selectArrayObjectsWhere` on a container), to drop on a kill.
#[derive(Debug, Clone)]
pub(super) struct Wanted {
    pub id: Option<i64>,
    pub name: Option<String>,
    /// Asked for in the temporary inventory: it drops there.
    pub temp: bool,
    pub asked: ClockTime,
}

impl Wanted {
    /// Asked for recently enough to drop now.
    pub fn fresh(&self, now: Option<ClockTime>) -> bool {
        now.is_none_or(|now| now.since(self.asked) <= WANTED_FOR)
    }

    pub fn same(&self, other: &Self) -> bool {
        self.temp == other.temp
            && match (self.id, other.id) {
                (Some(a), Some(b)) => a == b,
                _ => {
                    self.name.as_deref().map(str::to_lowercase)
                        == other.name.as_deref().map(str::to_lowercase)
                }
            }
    }
}

/// One of a quest's turn-in items: its ID, name, how many, and whether it is temporary.
#[derive(Debug, Clone)]
pub(super) struct Requirement {
    pub id: i64,
    pub name: String,
    pub quantity: i64,
    pub temp: bool,
}

/// A quest's turn-in items, from its `world.questTree` entry (`oItems`, quantities in `turnin`).
pub(super) fn requirements(quest: &Value) -> Vec<Requirement> {
    let quantities: HashMap<i64, i64> = values(&quest["turnin"])
        .filter_map(|t| Some((num(&t["ItemID"])?, num(&t["iQty"]).unwrap_or(1))))
        .collect();
    values(&quest["oItems"])
        .filter_map(|item| {
            let id = num(&item["ItemID"])?;
            Some(Requirement {
                id,
                name: item["sName"].as_str().unwrap_or_default().to_owned(),
                quantity: quantities.get(&id).copied().unwrap_or(1),
                // Quest items are temporary unless the game says otherwise.
                temp: item["bTemp"].as_bool().unwrap_or(true) || item["bTemp"] == json!(1),
            })
        })
        .collect()
}

/// An object's values or an array's items (the game uses both for lists).
pub(super) fn values(value: &Value) -> Box<dyn Iterator<Item = &Value> + '_> {
    match value {
        Value::Object(map) => Box::new(map.values()),
        Value::Array(items) => Box::new(items.iter()),
        _ => Box::new(std::iter::empty()),
    }
}

pub(super) fn num(value: &Value) -> Option<i64> {
    match value {
        Value::Number(n) => n.as_f64().map(|f| f as i64),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

/// An ID for an item known only by name, stable for that name.
pub(super) fn id_for(name: &str) -> i64 {
    let hash = name
        .to_lowercase()
        .bytes()
        .fold(1_469_598_103_934_665_603u64, |h, b| {
            (h ^ b as u64).wrapping_mul(1_099_511_628_211)
        });
    800_000_000 + (hash % 99_000_000) as i64
}

/// Adds `quantity` of an item to the container array `items`, as the game's `addItems` would.
pub(super) fn add_item(items: &mut Vec<Value>, id: i64, name: &str, quantity: i64, temp: bool) {
    if let Some(item) = items.iter_mut().find(|i| num(&i["ItemID"]) == Some(id)) {
        let have = num(&item["iQty"]).unwrap_or(0);
        item["iQty"] = json!(have + quantity);
        return;
    }
    items.push(json!({
        "ItemID": id,
        "CharItemID": id,
        "sName": name,
        "iQty": quantity,
        "iStk": 9999,
        "bTemp": if temp { 1 } else { 0 },
        "sType": "Item",
        "sES": "None",
        "bEquip": 0,
        "bCoins": 0,
        "bUpg": 0,
    }));
}

/// Takes `quantity` of an item off `items` (a turn-in), removing it at none.
pub(super) fn take_item(items: &mut Vec<Value>, id: i64, quantity: i64) {
    if let Some(at) = items.iter().position(|i| num(&i["ItemID"]) == Some(id)) {
        let left = num(&items[at]["iQty"]).unwrap_or(0) - quantity;
        if left > 0 {
            items[at]["iQty"] = json!(left);
        } else {
            items.remove(at);
        }
    }
}

/// How many of an item `items` holds.
pub(super) fn quantity(items: &[Value], id: i64) -> i64 {
    items
        .iter()
        .find(|i| num(&i["ItemID"]) == Some(id))
        .and_then(|i| num(&i["iQty"]))
        .unwrap_or(0)
}

/// A server packet as turu.swf hands it over (`pext`).
pub(super) fn pext(data: Value) -> (String, Vec<Value>) {
    let packet = json!({ "params": { "type": "json", "dataObj": data } });
    ("pext".into(), vec![Value::String(packet.to_string())])
}

/// A client packet as turu.swf reports it (`packet`).
pub(super) fn sent(text: String) -> (String, Vec<Value>) {
    ("packet".into(), vec![Value::String(text)])
}

/// Delivers `events` to `bot`, as the game would, with no lock of the fake held.
pub(super) fn deliver(bot: Option<std::sync::Arc<Bot>>, events: Vec<(String, Vec<Value>)>) {
    let Some(bot) = bot else { return };
    for (name, args) in events {
        bot.on_flash_call(&name, args);
    }
}

/// What a shop sells, from a merge script ([`super::WorldFake::with_shops`]): its items by ID and
/// name, and the names of the materials its script farms. Item `n` is merged from material `n`
/// (in turn), one of it, given the ID [`id_for`] its name: a kill drops that material when the
/// script asks for it by name or by that ID.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ShopStock {
    pub items: Vec<(i64, String)>,
    pub materials: Vec<String>,
}

/// A shop as the game's `loadShop` fills `world.shopinfo`, its items shaped after one recorded in
/// game (`merge_buy`'s): no quest, token, level, reputation or membership requirement, free (a
/// level-16 tape has little gold), and the item's materials in `turnin`.
pub(super) fn shop(id: i64, stock: &ShopStock) -> Value {
    let items: Vec<Value> = stock
        .items
        .iter()
        .enumerate()
        .map(|(i, (item_id, name))| {
            let turnin: Vec<Value> = if stock.materials.is_empty() {
                Vec::new()
            } else {
                let material = &stock.materials[i % stock.materials.len()];
                vec![json!({ "ItemID": id_for(material), "sName": material, "iQty": 1 })]
            };
            json!({
                "ItemID": item_id,
                "ShopItemID": id * 1000 + i as i64 + 1,
                "sName": name,
                "sType": "Armor",
                "sDesc": "",
                "sES": "co",
                "iQty": 1,
                "iStk": 1,
                "iCost": 0,
                "bCoins": false,
                "bUpg": false,
                "bTemp": false,
                "bHouse": false,
                "iLvl": 1,
                "sFaction": "None",
                "iReqRep": 0,
                "iQSindex": -1,
                "iQSvalue": 0,
                "TokID": 0,
                "TokQty": 0,
                "turnin": turnin,
            })
        })
        .collect();
    json!({
        "ShopID": id,
        "sName": format!("Shop {id}"),
        "iIndex": -1,
        "sField": "",
        "reqItems": null,
        "items": items,
    })
}
