//! Game JSON models. Field names follow the game's objects (VibeSkua: Skua.Core.Models);
//! they serialize to camelCase for scripts (see scripts/types/turu.d.ts).

use serde::{Deserialize, Deserializer, Serialize};

/// Accepts numbers, numeric strings, booleans and null as an i64 (the game mixes them).
pub fn lenient_i64<'de, D: Deserializer<'de>>(d: D) -> Result<i64, D::Error> {
    let v = serde_json::Value::deserialize(d)?;
    Ok(match v {
        serde_json::Value::Number(n) => n
            .as_i64()
            .or_else(|| n.as_f64().map(|f| f as i64))
            .unwrap_or(0),
        serde_json::Value::String(s) => s.trim().parse::<f64>().map(|f| f as i64).unwrap_or(0),
        serde_json::Value::Bool(b) => b as i64,
        _ => 0,
    })
}

/// Accepts numbers, numeric strings, booleans and null as an f64.
pub fn lenient_f64<'de, D: Deserializer<'de>>(d: D) -> Result<f64, D::Error> {
    let v = serde_json::Value::deserialize(d)?;
    Ok(match v {
        serde_json::Value::Number(n) => n.as_f64().unwrap_or(0.0),
        serde_json::Value::String(s) => s.trim().parse::<f64>().unwrap_or(0.0),
        serde_json::Value::Bool(b) => b as i64 as f64,
        _ => 0.0,
    })
}

/// Accepts `true`, `1`, `"1"`, `"true"` as true.
pub fn lenient_bool<'de, D: Deserializer<'de>>(d: D) -> Result<bool, D::Error> {
    let v = serde_json::Value::deserialize(d)?;
    Ok(match v {
        serde_json::Value::Bool(b) => b,
        serde_json::Value::Number(n) => n.as_f64().unwrap_or(0.0) != 0.0,
        serde_json::Value::String(s) => matches!(s.trim(), "1" | "true" | "True"),
        _ => false,
    })
}

/// Accepts strings and numbers as a String; null as empty.
pub fn lenient_string<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    let v = serde_json::Value::deserialize(d)?;
    Ok(match v {
        serde_json::Value::String(s) => s,
        serde_json::Value::Null => String::new(),
        other => other.to_string(),
    })
}

/// An item in the inventory, bank, temp inventory, house or a drop (VibeSkua `ItemBase`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all(serialize = "camelCase"))]
pub struct ItemBase {
    #[serde(
        rename(deserialize = "ItemID", serialize = "id"),
        deserialize_with = "lenient_i64",
        default
    )]
    pub id: i64,
    #[serde(
        rename(deserialize = "CharItemID"),
        deserialize_with = "lenient_i64",
        default
    )]
    pub char_item_id: i64,
    #[serde(
        rename(deserialize = "sName", serialize = "name"),
        deserialize_with = "lenient_string",
        default
    )]
    pub name: String,
    #[serde(
        rename(deserialize = "sDesc", serialize = "description"),
        deserialize_with = "lenient_string",
        default
    )]
    pub description: String,
    #[serde(
        rename(deserialize = "iQty", serialize = "quantity"),
        deserialize_with = "lenient_i64",
        default
    )]
    pub quantity: i64,
    #[serde(
        rename(deserialize = "iStk", serialize = "maxStack"),
        deserialize_with = "lenient_i64",
        default
    )]
    pub max_stack: i64,
    #[serde(
        rename(deserialize = "iLvl", serialize = "level"),
        deserialize_with = "lenient_i64",
        default
    )]
    pub level: i64,
    #[serde(
        rename(deserialize = "EnhLvl", serialize = "enhancementLevel"),
        deserialize_with = "lenient_i64",
        default
    )]
    pub enhancement_level: i64,
    #[serde(
        rename(deserialize = "bEquip", serialize = "equipped"),
        deserialize_with = "lenient_bool",
        default
    )]
    pub equipped: bool,
    #[serde(
        rename(deserialize = "bUpg", serialize = "upgrade"),
        deserialize_with = "lenient_bool",
        default
    )]
    pub upgrade: bool,
    #[serde(
        rename(deserialize = "bCoins", serialize = "coins"),
        deserialize_with = "lenient_bool",
        default
    )]
    pub coins: bool,
    #[serde(
        rename(deserialize = "bTemp", serialize = "temp"),
        deserialize_with = "lenient_bool",
        default
    )]
    pub temp: bool,
    #[serde(
        rename(deserialize = "bHouse", serialize = "house"),
        deserialize_with = "lenient_bool",
        default
    )]
    pub house: bool,
    #[serde(
        rename(deserialize = "sType", serialize = "category"),
        deserialize_with = "lenient_string",
        default
    )]
    pub category: String,
    #[serde(
        rename(deserialize = "sES", serialize = "equipSlot"),
        deserialize_with = "lenient_string",
        default
    )]
    pub equip_slot: String,
    #[serde(
        rename(deserialize = "sMeta", serialize = "meta"),
        deserialize_with = "lenient_string",
        default
    )]
    pub meta: String,
    /// `sFile`: the item's SWF (VibeSkua `FileLink`). Not sent to scripts.
    #[serde(
        rename(deserialize = "sFile"),
        deserialize_with = "lenient_string",
        default,
        skip_serializing
    )]
    pub file_link: String,
    /// `sLink`: the item's linkage name (VibeSkua `FileName`). Not sent to scripts.
    #[serde(
        rename(deserialize = "sLink"),
        deserialize_with = "lenient_string",
        default,
        skip_serializing
    )]
    pub file_name: String,
}

impl ItemBase {
    /// The item as the game spells it (VibeSkua `JsonConvert.SerializeObject(item)`), for
    /// `skua.swf` callbacks that take an item's JSON (`inventoryHasSpaceFor`, ...).
    pub fn game_json(&self) -> serde_json::Value {
        let flag = |b: bool| if b { "1" } else { "0" };
        serde_json::json!({
            "ItemID": self.id,
            "CharItemID": self.char_item_id,
            "sName": self.name,
            "sDesc": self.description,
            "iQty": self.quantity,
            "iStk": self.max_stack,
            "iLvl": self.level,
            "EnhLvl": self.enhancement_level,
            "bEquip": flag(self.equipped),
            "bUpg": flag(self.upgrade),
            "bCoins": flag(self.coins),
            "bTemp": flag(self.temp),
            "bHouse": flag(self.house),
            "sType": self.category,
            "sES": self.equip_slot,
            "sMeta": self.meta,
            "sFile": self.file_link,
            "sLink": self.file_name,
        })
    }

    pub fn is_class(&self) -> bool {
        self.category.eq_ignore_ascii_case("Class")
    }

    pub fn is_max_stack(&self) -> bool {
        self.max_stack > 0 && self.quantity >= self.max_stack
    }

    /// Case-insensitive, trimmed name match (how VibeSkua compares item names).
    pub fn name_is(&self, name: &str) -> bool {
        self.name.trim().eq_ignore_ascii_case(name.trim())
    }
}

/// A monster in the current map (VibeSkua `Monster`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all(serialize = "camelCase"))]
pub struct Monster {
    #[serde(
        rename(deserialize = "MonID", serialize = "id"),
        deserialize_with = "lenient_i64",
        default
    )]
    pub id: i64,
    #[serde(
        rename(deserialize = "MonMapID", serialize = "mapId"),
        deserialize_with = "lenient_i64",
        default
    )]
    pub map_id: i64,
    #[serde(
        rename(deserialize = "strMonName", serialize = "name"),
        deserialize_with = "lenient_string",
        default
    )]
    pub name: String,
    #[serde(
        rename(deserialize = "sRace", serialize = "race"),
        deserialize_with = "lenient_string",
        default
    )]
    pub race: String,
    #[serde(
        rename(deserialize = "strFrame", serialize = "cell"),
        deserialize_with = "lenient_string",
        default
    )]
    pub cell: String,
    #[serde(
        rename(deserialize = "intHP", serialize = "hp"),
        deserialize_with = "lenient_i64",
        default
    )]
    pub hp: i64,
    #[serde(
        rename(deserialize = "intHPMax", serialize = "maxHp"),
        deserialize_with = "lenient_i64",
        default
    )]
    pub max_hp: i64,
    #[serde(
        rename(deserialize = "intState", serialize = "state"),
        deserialize_with = "lenient_i64",
        default
    )]
    pub state: i64,
    #[serde(
        rename(deserialize = "iLvl", serialize = "level"),
        deserialize_with = "lenient_i64",
        default
    )]
    pub level: i64,
}

impl Monster {
    pub fn alive(&self) -> bool {
        self.hp > 0 && self.state > 0
    }

    /// VibeSkua matching: `*` matches all, otherwise case-insensitive name equality.
    pub fn name_matches(&self, name: &str) -> bool {
        let name = name.trim();
        name == "*" || self.name.trim().eq_ignore_ascii_case(name)
    }
}

/// An item a quest needs or gives (an entry of `oItems`, `oReqd`, `oRewards` or `reward`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestItem {
    pub id: i64,
    pub name: String,
    pub quantity: i64,
    pub temp: bool,
}

impl From<&ItemBase> for QuestItem {
    fn from(item: &ItemBase) -> Self {
        Self {
            id: item.id,
            name: item.name.trim().to_owned(),
            quantity: item.quantity,
            temp: item.temp,
        }
    }
}

/// A quest of `world.questTree` (VibeSkua `Quest`).
///
/// Deserializes from the game's JSON (`QuestID`, `sName`, `oItems` + `turnin`, ...) and
/// serializes to the `Quest` shape of bot/js/turu.ts.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(from = "RawQuest", rename_all(serialize = "camelCase"))]
pub struct Quest {
    pub id: i64,
    pub name: String,
    /// `"c"` when it can be completed; `None` when the quest isn't in progress.
    pub status: Option<String>,
    /// `bOnce`: can only be completed once.
    pub once: bool,
    /// `bUpg`: member-only.
    pub upgrade: bool,
    pub level: i64,
    /// Items needed to turn it in (`oItems`, quantities from `turnin`).
    pub requirements: Vec<QuestItem>,
    /// Items needed to accept it (`oReqd`).
    pub accept_requirements: Vec<QuestItem>,
    /// `oRewards`, flattened.
    pub rewards: Vec<QuestItem>,
    /// `reward`: the reward list with drop rates (VibeSkua `SimpleRewards`).
    pub simple_rewards: Vec<QuestItem>,
    /// `iSlot`: the quest chain slot (`world.getQuestValue(slot)`).
    pub slot: i64,
    /// `iValue`: the chain value this quest sets.
    pub value: i64,
    /// `sField`: the daily/achievement field.
    pub field: String,
    /// `iIndex`: the index in `field`.
    pub index: i64,
    /// `iClass`: class needed to accept it (VibeSkua `RequiredClassID`).
    #[serde(skip_serializing)]
    pub required_class_id: i64,
    /// `iReqCP`: class points needed with that class.
    #[serde(skip_serializing)]
    pub required_class_points: i64,
    /// `FactionID`: faction needed to accept it.
    #[serde(skip_serializing)]
    pub required_faction_id: i64,
    /// `iReqRep`: reputation needed with that faction.
    #[serde(skip_serializing)]
    pub required_faction_rep: i64,
}

impl Quest {
    /// VibeSkua `Quest.Active`: the quest is in progress (it has a status).
    pub fn active(&self) -> bool {
        self.status.is_some()
    }

    /// `status == "c"`: the turn-in requirements are met.
    pub fn completable(&self) -> bool {
        self.status.as_deref() == Some("c")
    }

    /// Parses `world.questTree` (an object keyed by quest ID, or an array), skipping entries
    /// that aren't quests.
    pub fn tree_from(value: serde_json::Value) -> Vec<Quest> {
        values_of(&value)
            .filter(|v| v.is_object())
            .filter_map(|v| serde_json::from_value::<Quest>(v.clone()).ok())
            .filter(|q| q.id != 0)
            .collect()
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawQuest {
    #[serde(rename = "QuestID", deserialize_with = "lenient_i64")]
    id: i64,
    #[serde(rename = "sName", deserialize_with = "lenient_string")]
    name: String,
    status: serde_json::Value,
    #[serde(rename = "bOnce", deserialize_with = "lenient_bool")]
    once: bool,
    #[serde(rename = "bUpg", deserialize_with = "lenient_bool")]
    upgrade: bool,
    #[serde(rename = "iLvl", deserialize_with = "lenient_i64")]
    level: i64,
    #[serde(rename = "iSlot", deserialize_with = "lenient_i64")]
    slot: i64,
    #[serde(rename = "iValue", deserialize_with = "lenient_i64")]
    value: i64,
    #[serde(rename = "sField", deserialize_with = "lenient_string")]
    field: String,
    #[serde(rename = "iIndex", deserialize_with = "lenient_i64")]
    index: i64,
    #[serde(rename = "iClass", deserialize_with = "lenient_i64")]
    class_id: i64,
    #[serde(rename = "iReqCP", deserialize_with = "lenient_i64")]
    class_points: i64,
    #[serde(rename = "FactionID", deserialize_with = "lenient_i64")]
    faction_id: i64,
    #[serde(rename = "iReqRep", deserialize_with = "lenient_i64")]
    faction_rep: i64,
    #[serde(rename = "oReqd")]
    accept_requirements: serde_json::Value,
    #[serde(rename = "oItems")]
    items: serde_json::Value,
    turnin: serde_json::Value,
    #[serde(rename = "oRewards")]
    rewards: serde_json::Value,
    reward: serde_json::Value,
}

/// The values of a JSON object, or the elements of an array (the game uses both for lists).
fn values_of(value: &serde_json::Value) -> Box<dyn Iterator<Item = &serde_json::Value> + '_> {
    match value {
        serde_json::Value::Object(map) => Box::new(map.values()),
        serde_json::Value::Array(items) => Box::new(items.iter()),
        _ => Box::new(std::iter::empty()),
    }
}

/// Items of a `{ "<ItemID>": item }` object or an array, with the ID taken from the key
/// when the item lacks `ItemID`.
fn items_of(value: &serde_json::Value) -> Vec<ItemBase> {
    let parse = |key: Option<&str>, v: &serde_json::Value| {
        let mut item = serde_json::from_value::<ItemBase>(v.clone()).ok()?;
        if item.id == 0 {
            item.id = key.and_then(|k| k.trim().parse().ok()).unwrap_or_default();
        }
        Some(item)
    };
    match value {
        serde_json::Value::Object(map) => map
            .iter()
            .filter(|(_, v)| v.is_object())
            .filter_map(|(k, v)| parse(Some(k), v))
            .collect(),
        serde_json::Value::Array(items) => items
            .iter()
            .filter(|v| v.is_object())
            .filter_map(|v| parse(None, v))
            .collect(),
        _ => Vec::new(),
    }
}

impl From<RawQuest> for Quest {
    fn from(raw: RawQuest) -> Self {
        // VibeSkua `Requirements`: `oItems` with the quantities of `turnin`.
        let turnin = items_of(&raw.turnin);
        let requirements = items_of(&raw.items)
            .iter()
            .map(|item| {
                let mut req = QuestItem::from(item);
                if let Some(t) = turnin.iter().find(|t| t.id == item.id) {
                    req.quantity = t.quantity;
                }
                req
            })
            .collect();
        // `oRewards` is `{ "<type>": { "<ItemID>": item } }` (QuestRewardConverter).
        let rewards = values_of(&raw.rewards)
            .flat_map(|group| {
                if group.get("ItemID").is_some() {
                    items_of(&serde_json::Value::Array(vec![group.clone()]))
                } else {
                    items_of(group)
                }
            })
            .map(|item| QuestItem::from(&item))
            .collect();
        let status = match raw.status {
            serde_json::Value::Null => None,
            serde_json::Value::String(s) => Some(s),
            other => Some(other.to_string()),
        };
        Self {
            id: raw.id,
            name: raw.name.trim().to_owned(),
            status,
            once: raw.once,
            upgrade: raw.upgrade,
            level: raw.level,
            requirements,
            accept_requirements: items_of(&raw.accept_requirements)
                .iter()
                .map(QuestItem::from)
                .collect(),
            rewards,
            simple_rewards: items_of(&raw.reward).iter().map(QuestItem::from).collect(),
            slot: raw.slot,
            value: raw.value,
            field: raw.field,
            index: raw.index,
            required_class_id: raw.class_id,
            required_class_points: raw.class_points,
            required_faction_id: raw.faction_id,
            required_faction_rep: raw.faction_rep,
        }
    }
}

/// An item of the loaded shop, `world.shopinfo.items` (VibeSkua `ShopItem`).
///
/// Serializes as `InventoryItem` plus `shopItemId`, `cost` and `requirements` (turu.ts `ShopItem`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(from = "RawShopItem", rename_all(serialize = "camelCase"))]
pub struct ShopItem {
    #[serde(flatten)]
    pub item: ItemBase,
    /// `ShopItemID`: the item's ID within the shop.
    pub shop_item_id: i64,
    /// `iCost`.
    pub cost: i64,
    /// What the purchase needs besides gold/AC: merge materials and faction rank, as text.
    pub requirements: String,
    /// `sFaction`.
    #[serde(skip_serializing)]
    pub faction: String,
    /// `iReqRep`.
    #[serde(skip_serializing)]
    pub required_reputation: i64,
    /// `turnin`: merge materials.
    #[serde(skip_serializing)]
    pub turnin: Vec<ItemBase>,
    /// `TokID`, `TokQty`, `TokName`: the token a purchase costs.
    #[serde(skip_serializing)]
    pub token_id: i64,
    #[serde(skip_serializing)]
    pub token_quantity: f64,
    #[serde(skip_serializing)]
    pub token_name: String,
}

impl ShopItem {
    /// The game-shaped JSON of the item (for `inventoryHasSpaceFor`).
    pub fn game_json(&self) -> serde_json::Value {
        let mut json = self.item.game_json();
        if let Some(obj) = json.as_object_mut() {
            obj.insert("ShopItemID".into(), self.shop_item_id.into());
            obj.insert("iCost".into(), self.cost.into());
        }
        json
    }
}

#[derive(Deserialize, Default)]
struct RawShopItem {
    #[serde(flatten)]
    item: ItemBase,
    #[serde(rename = "ShopItemID", deserialize_with = "lenient_i64", default)]
    shop_item_id: i64,
    #[serde(rename = "iCost", deserialize_with = "lenient_i64", default)]
    cost: i64,
    #[serde(rename = "sFaction", deserialize_with = "lenient_string", default)]
    faction: String,
    #[serde(rename = "iReqRep", deserialize_with = "lenient_i64", default)]
    required_reputation: i64,
    #[serde(default)]
    turnin: serde_json::Value,
    #[serde(rename = "TokID", deserialize_with = "lenient_i64", default)]
    token_id: i64,
    #[serde(rename = "TokQty", default)]
    token_quantity: serde_json::Value,
    #[serde(rename = "TokName", deserialize_with = "lenient_string", default)]
    token_name: String,
}

impl From<RawShopItem> for ShopItem {
    fn from(raw: RawShopItem) -> Self {
        let turnin = items_of(&raw.turnin);
        let mut parts: Vec<String> = turnin
            .iter()
            .map(|t| format!("{} x{}", t.name.trim(), t.quantity))
            .collect();
        let faction = raw.faction.trim();
        if !faction.is_empty()
            && !faction.eq_ignore_ascii_case("none")
            && raw.required_reputation > 0
        {
            parts.push(format!("{faction} reputation {}", raw.required_reputation));
        }
        let token_quantity = match &raw.token_quantity {
            serde_json::Value::Number(n) => n.as_f64().unwrap_or_default(),
            serde_json::Value::String(s) => s.trim().parse().unwrap_or_default(),
            _ => 0.0,
        };
        Self {
            item: raw.item,
            shop_item_id: raw.shop_item_id,
            cost: raw.cost,
            requirements: parts.join(", "),
            faction: raw.faction,
            required_reputation: raw.required_reputation,
            turnin,
            token_id: raw.token_id,
            token_quantity,
            token_name: raw.token_name,
        }
    }
}

/// A skill of the action bar, `world.actions.active[index]` (VibeSkua `SkillInfo`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all(serialize = "camelCase"))]
pub struct Skill {
    /// The slot (0 auto attack, 1-4 skills, 5 potion). Not in the game's JSON.
    #[serde(skip_deserializing)]
    pub index: i64,
    #[serde(
        rename(deserialize = "nam"),
        deserialize_with = "lenient_string",
        default
    )]
    pub name: String,
    #[serde(rename(deserialize = "mp"), deserialize_with = "lenient_i64", default)]
    pub mana_cost: i64,
    /// `cd`: cooldown in ms.
    #[serde(rename(deserialize = "cd"), deserialize_with = "lenient_i64", default)]
    pub cooldown: i64,
    #[serde(
        rename(deserialize = "isOK"),
        deserialize_with = "lenient_bool",
        default
    )]
    pub is_ok: bool,
    /// `ts`: when the skill was last used (ms). Not sent to scripts.
    #[serde(
        rename(deserialize = "ts"),
        deserialize_with = "lenient_i64",
        default,
        skip_serializing
    )]
    pub last_use: i64,
    /// `auto`: the auto attack. Not sent to scripts.
    #[serde(
        rename(deserialize = "auto"),
        deserialize_with = "lenient_bool",
        default,
        skip_serializing
    )]
    pub auto: bool,
}

impl Skill {
    /// Parses `world.actions.active` (an array; null entries become empty skills so the
    /// indexes stay right).
    pub fn list_from(value: serde_json::Value) -> Vec<Skill> {
        let serde_json::Value::Array(items) = value else {
            return Vec::new();
        };
        items
            .into_iter()
            .enumerate()
            .map(|(i, v)| {
                let mut skill: Skill = serde_json::from_value(v).unwrap_or_default();
                skill.index = i as i64;
                skill
            })
            .collect()
    }
}

/// A game server (VibeSkua `Server`): from the server list API, `serialCmd.servers`, or a script.
/// Reads the game's names (`sName`, `sIP`, ...) and the camelCase names scripts send back.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Server {
    #[serde(
        rename = "name",
        alias = "sName",
        deserialize_with = "lenient_string",
        default
    )]
    pub name: String,
    #[serde(
        rename = "ip",
        alias = "sIP",
        deserialize_with = "lenient_string",
        default
    )]
    pub ip: String,
    /// Canned chat 0, free chat 2.
    #[serde(
        rename = "chatLevel",
        alias = "iChat",
        deserialize_with = "lenient_i64",
        default
    )]
    pub chat_level: i64,
    #[serde(
        rename = "port",
        alias = "iPort",
        deserialize_with = "lenient_i64",
        default
    )]
    pub port: i64,
    #[serde(
        rename = "online",
        alias = "bOnline",
        deserialize_with = "lenient_bool",
        default
    )]
    pub online: bool,
    #[serde(
        rename = "language",
        alias = "sLang",
        deserialize_with = "lenient_string",
        default
    )]
    pub language: String,
    #[serde(
        rename = "playerCount",
        alias = "iCount",
        deserialize_with = "lenient_i64",
        default
    )]
    pub player_count: i64,
    /// Member-only.
    #[serde(
        rename = "upgrade",
        alias = "bUpg",
        deserialize_with = "lenient_bool",
        default
    )]
    pub upgrade: bool,
    #[serde(
        rename = "maxPlayers",
        alias = "iMax",
        deserialize_with = "lenient_i64",
        default
    )]
    pub max_players: i64,
    #[serde(
        rename = "level",
        alias = "iLevel",
        deserialize_with = "lenient_i64",
        default
    )]
    pub level: i64,
}

impl Server {
    /// The server in the game's own field names, for `connectToServer`.
    pub fn to_game_json(&self) -> String {
        serde_json::json!({
            "sName": self.name,
            "sIP": self.ip,
            "iChat": self.chat_level,
            "iPort": self.port,
            "bOnline": self.online as i64,
            "sLang": self.language,
            "iCount": self.player_count,
            "bUpg": self.upgrade as i64,
            "iMax": self.max_players,
            "iLevel": self.level,
        })
        .to_string()
    }
}

/// A faction you have reputation in (VibeSkua `Faction`, from `world.myAvatar.factions`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all(serialize = "camelCase"))]
pub struct Faction {
    #[serde(
        rename(deserialize = "FactionID", serialize = "id"),
        deserialize_with = "lenient_i64",
        default
    )]
    pub id: i64,
    #[serde(
        rename(deserialize = "sName", serialize = "name"),
        deserialize_with = "lenient_string",
        default
    )]
    pub name: String,
    #[serde(
        rename(deserialize = "iRank"),
        deserialize_with = "lenient_i64",
        default
    )]
    pub rank: i64,
    /// The total amount of rep you have for this faction.
    #[serde(
        rename(deserialize = "iRep"),
        deserialize_with = "lenient_i64",
        default
    )]
    pub total_rep: i64,
    /// The amount of rep you have for your current rank.
    #[serde(
        rename(deserialize = "iSpillRep"),
        deserialize_with = "lenient_i64",
        default
    )]
    pub rep: i64,
    /// The rep needed to rank up from your current rank.
    #[serde(
        rename(deserialize = "iRepToRank"),
        deserialize_with = "lenient_i64",
        default
    )]
    pub required_rep: i64,
    /// `required_rep - rep` (VibeSkua `RemainingRep`), filled when read.
    #[serde(skip_deserializing)]
    pub remaining_rep: i64,
}

/// An aura on a player or a monster (VibeSkua `Aura`, from skua.swf's `GetPlayerAura` and
/// `GetMonsterAuraByID`/`ByName`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all(serialize = "camelCase"))]
pub struct Aura {
    #[serde(
        rename(deserialize = "nam"),
        deserialize_with = "lenient_string",
        default
    )]
    pub name: String,
    /// The aura's value, its stack count for most auras (VibeSkua defaults it to 1).
    #[serde(
        rename(deserialize = "val"),
        deserialize_with = "lenient_f64",
        default = "one"
    )]
    pub value: f64,
    #[serde(
        rename(deserialize = "t"),
        deserialize_with = "lenient_string",
        default
    )]
    pub r#type: String,
    #[serde(
        rename(deserialize = "cat"),
        deserialize_with = "lenient_string",
        default
    )]
    pub category: String,
    /// Seconds it lasts.
    #[serde(rename(deserialize = "dur"), deserialize_with = "lenient_i64", default)]
    pub duration: i64,
    /// When it was applied, in Unix milliseconds (0 when unknown).
    #[serde(
        rename(deserialize = "ts", serialize = "timeStamp"),
        deserialize_with = "lenient_i64",
        default
    )]
    pub time_stamp: i64,
    #[serde(deserialize_with = "lenient_bool", default)]
    pub passive: bool,
    #[serde(
        rename(deserialize = "potionType"),
        deserialize_with = "lenient_string",
        default
    )]
    pub potion_type: String,
    #[serde(deserialize_with = "lenient_string", default)]
    pub icon: String,
}

fn one() -> f64 {
    1.0
}

/// An aura as the HUD shows it (VibeSkua `AuraSnapshot`, from skua.swf's `GetAuraSnapshots`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all(serialize = "camelCase"))]
pub struct AuraSnapshot {
    #[serde(
        rename(deserialize = "nam"),
        deserialize_with = "lenient_string",
        default
    )]
    pub name: String,
    #[serde(rename(deserialize = "n"), deserialize_with = "lenient_i64", default)]
    pub stacks: i64,
    /// Seconds it lasts.
    #[serde(rename(deserialize = "dur"), deserialize_with = "lenient_f64", default)]
    pub duration: f64,
    /// Seconds left.
    #[serde(
        rename(deserialize = "remaining", serialize = "remainingTime"),
        deserialize_with = "lenient_f64",
        default
    )]
    pub remaining_time: f64,
    #[serde(
        rename(deserialize = "persist"),
        deserialize_with = "lenient_bool",
        default
    )]
    pub persistent: bool,
    #[serde(deserialize_with = "lenient_string", default)]
    pub icon: String,
    #[serde(
        rename(deserialize = "desc"),
        deserialize_with = "lenient_string",
        default
    )]
    pub description: String,
}

impl AuraSnapshot {
    /// Turu: a timed aura whose time ran out. The game can keep it on the HUD when no `aura-`
    /// packet arrives, and counting its stacks breaks class combos (Grimlite-Li's `AuraManager`
    /// drops them the same way). VibeSkua counts them.
    pub fn expired(&self) -> bool {
        !self.persistent && self.duration > 0.0 && self.remaining_time <= 0.0
    }
}

/// A badge on your character page (CoreBots `Badge`, from `account.aq.com/CharPage/Badges`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all(serialize = "camelCase"))]
pub struct Badge {
    #[serde(
        rename(deserialize = "badgeID", serialize = "id"),
        deserialize_with = "lenient_i64",
        default
    )]
    pub id: i64,
    #[serde(
        rename(deserialize = "sTitle", serialize = "name"),
        deserialize_with = "lenient_string",
        default
    )]
    pub name: String,
    #[serde(
        rename(deserialize = "sCategory", serialize = "category"),
        deserialize_with = "lenient_string",
        default
    )]
    pub category: String,
}
