//! A fake game world seeded from a tape, for running the converted scripts ("drafts") for a few
//! seconds each (docs/REPLAY_TESTS_PROPOSAL.md, phase 4).
//!
//! It answers reads from what the game answered on the tape (the latest answer wins): each
//! `getGameObject` path, the paths inside `getMany`'s answers, `isNull`, and every other read by
//! its name and arguments. On top is an overlay that sends change, through a short list of rules:
//! `setGameObject` sets the path, joining a map (the `tfer` packet) moves you there, a jump moves
//! you to the cell, a bank move (`bankToInv`, `bankFromInv`, `bankSwapInv`) moves the item. Batched
//! reads (`snapshot`, `getMany`, ...) are built from the single ones, as turu.swf builds them from
//! the game.
//!
//! It is a model, so it is wrong in places: its job is only to get a draft past its setup into its
//! own code. Each rule is there because a draft needed it; the answers come from the tape, not
//! from guessing.

mod sim;

use crate::Bot;
use crate::control::ClockTime;
use crate::host::{FlashArg, FlashHost, HostError};
use crate::tape::{Line, Tape};
use parking_lot::Mutex;
use serde_json::{Value, json};
pub use sim::ShopStock;
use sim::{Combat, Monster, Wanted};
use std::collections::HashMap;
use std::sync::{Arc, Weak};

const BANK: &str = "world.bankinfo.items";
const HOUSE: &str = "world.myAvatar.houseitems";
const BANK_COUNT: &str = "world.myAvatar.iBankCount";

/// A fake world (see the module's documentation).
pub struct WorldFake {
    /// `getGameObject` / `getGameObjectS` paths and their values, from the tape.
    paths: HashMap<String, Value>,
    /// `isNull` answers from the tape, by path.
    nulls: HashMap<String, bool>,
    /// Every other read's latest answer, by name and arguments.
    answers: HashMap<String, String>,
    /// Each callback's latest answer, by name and first argument (a path, a game function): for a
    /// call with other arguments the tape hasn't.
    by_name: HashMap<String, String>,
    /// What sends changed.
    overlay: Mutex<HashMap<String, Value>>,
    /// Calls made, for a report.
    calls: Mutex<u64>,
    /// Quests accepted and not yet turned in.
    in_progress: Mutex<std::collections::HashSet<i64>>,
    /// The last calls made, for a report.
    recent: Mutex<std::collections::VecDeque<String>>,
    /// Each map's monsters by name (lower-case map names), from the scripts ([`Self::with_monsters`]).
    catalog: HashMap<String, Vec<String>>,
    /// The monsters where you are, and your target.
    combat: Mutex<Combat>,
    /// Items scripts asked for, to drop on a kill.
    wanted: Mutex<Vec<Wanted>>,
    /// Quest slots' values set by turn-ins (`world.getQuestValue`).
    slot_values: Mutex<HashMap<i64, i64>>,
    /// The bot it plays for ([`Self::attach`]): its clock, and where packets go.
    bot: Mutex<Weak<Bot>>,
    /// Packets to deliver once the call is answered, with no lock of the fake held.
    pending: Mutex<Vec<(String, Vec<Value>)>>,
    /// Large values' JSON text by path, until the path changes ([`Self::text`]).
    texts: Mutex<HashMap<String, String>>,
    /// What each shop sells, by shop ID, from the scripts ([`Self::with_shops`]).
    shops: HashMap<i64, ShopStock>,
    /// The shops' materials' names by the IDs they're given, so an item asked for by ID drops
    /// under its name.
    materials: HashMap<i64, String>,
    /// Purchases made, for a report.
    purchases: Mutex<u64>,
}

impl WorldFake {
    /// The world as it was while the tape's script ran: what the game answered up to the
    /// script's end (not the logout at the client's exit after it).
    pub fn from_tape(tape: &Tape) -> Self {
        let end = tape
            .lines
            .iter()
            .position(|l| matches!(l, Line::Note { what, .. } if what == "scriptStopped"))
            .unwrap_or(tape.lines.len());
        let mut paths = HashMap::new();
        let mut nulls = HashMap::new();
        let mut answers = HashMap::new();
        let mut by_name = HashMap::new();
        for line in &tape.lines[..end] {
            let Line::Call {
                name,
                args,
                answer: Some(answer),
                ..
            } = line
            else {
                continue;
            };
            let first = args.first().and_then(Value::as_str);
            match (name.as_str(), first) {
                ("getGameObject" | "getGameObjectS", Some(path)) => {
                    paths.insert(path.to_owned(), parse(answer));
                }
                ("isNull", Some(path)) => {
                    nulls.insert(path.to_owned(), crate::flash::parse_bool(answer));
                }
                ("getMany", _) => {
                    if let (Some(Value::Object(keys)), Value::Object(values)) =
                        (args.first(), parse(answer))
                    {
                        for (key, path) in keys {
                            if let (Some(path), Some(value)) = (path.as_str(), values.get(key)) {
                                paths.insert(path.to_owned(), value.clone());
                            }
                        }
                    }
                }
                _ => {}
            }
            answers.insert(key(name, args), answer.clone());
            by_name.insert(loose_key(name, args), answer.clone());
        }
        // The monsters of the map the tape was in, as the game listed them last.
        let seed: Vec<Monster> = answers
            .iter()
            .filter(|(k, _)| k.starts_with("getMonsters"))
            .filter_map(|(_, answer)| serde_json::from_str::<Vec<Value>>(answer).ok())
            .max_by_key(Vec::len)
            .unwrap_or_default()
            .iter()
            .filter_map(|m| {
                Some(Monster::new(
                    sim::num(&m["MonMapID"])?,
                    m["strMonName"].as_str()?,
                ))
            })
            .collect();
        Self {
            paths,
            nulls,
            answers,
            by_name,
            overlay: Mutex::default(),
            calls: Mutex::default(),
            in_progress: Mutex::default(),
            recent: Mutex::default(),
            catalog: HashMap::new(),
            combat: Mutex::new(Combat {
                monsters: seed,
                target: None,
            }),
            wanted: Mutex::default(),
            slot_values: Mutex::default(),
            bot: Mutex::new(Weak::new()),
            pending: Mutex::default(),
            texts: Mutex::default(),
            shops: HashMap::new(),
            materials: HashMap::new(),
            purchases: Mutex::default(),
        }
    }

    /// What each shop sells, by shop ID, for the shops a script loads (the tape's shop keeps its
    /// own). Each item is merged from one of the shop's materials, in turn (see [`ShopStock`]).
    pub fn with_shops(mut self, shops: HashMap<i64, ShopStock>) -> Self {
        self.materials = shops
            .values()
            .flat_map(|shop| &shop.materials)
            .map(|name| (sim::id_for(name), name.clone()))
            .collect();
        self.shops = shops;
        self
    }

    /// Each map's monsters by name, for the maps a script joins (the tape's map keeps its own).
    /// Map names are matched without case or room number.
    pub fn with_monsters(mut self, catalog: HashMap<String, Vec<String>>) -> Self {
        self.catalog = catalog
            .into_iter()
            .map(|(map, names)| (map.to_lowercase(), names))
            .collect();
        self
    }

    /// Plays for `bot`: packets (kills, drops, turn-ins) go to it, and monsters come back by its
    /// clock. Without it the fake still answers, sending nothing.
    pub fn attach(&self, bot: &Arc<Bot>) {
        *self.bot.lock() = Arc::downgrade(bot);
    }

    fn now(&self) -> Option<ClockTime> {
        self.bot.lock().upgrade().map(|b| b.control.now())
    }

    fn queue(&self, event: (String, Vec<Value>)) {
        self.pending.lock().push(event);
    }

    /// The cell you're in.
    fn cell(&self) -> String {
        self.get("world.strFrame")
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_else(|| "Enter".into())
    }

    /// The monsters' answer: all of them, or only the living (`availableMonsters`); they stand in
    /// your cell, so a script's cell always has its monster.
    fn monsters(&self, alive_only: bool) -> String {
        let cell = self.cell();
        let now = self.now();
        let mut combat = self.combat.lock();
        combat.respawn(now);
        let list: Vec<Value> = combat
            .monsters
            .iter()
            .filter(|m| !alive_only || m.alive())
            .map(|m| m.json(&cell))
            .collect();
        Value::Array(list).to_string()
    }

    fn target(&self) -> Value {
        let cell = self.cell();
        let combat = self.combat.lock();
        combat.target().map_or_else(|| json!({}), |m| m.json(&cell))
    }

    /// A hit on the target (`useSkill`): on a kill, the game's packets and the drops.
    fn hit(&self) {
        let now = self.now();
        let killed = self.combat.lock().hit(now);
        let Some(id) = killed else { return };
        self.queue(sim::pext(json!({
            "cmd": "addGoldExp", "typ": "m", "id": id, "intGold": 10, "intExp": 10,
        })));
        self.queue(sim::pext(json!({
            "cmd": "ct", "m": { id.to_string(): { "intState": 0, "intHP": 0 } },
        })));
        self.drop_wanted();
        self.give_quest_items();
    }

    /// Notes an item a script asked a container for, to drop on a kill.
    fn want(&self, path: &str, filter: &std::collections::BTreeMap<String, FlashArg>) {
        let temp = match path {
            "world.myAvatar.tempitems" => true,
            "world.myAvatar.items" | "world.bankinfo.items" | "world.myAvatar.houseitems" => false,
            _ => return,
        };
        let id = match filter.get("ItemID") {
            Some(FlashArg::Number(n)) => Some(*n as i64),
            Some(FlashArg::String(s)) => s.trim().parse().ok(),
            _ => None,
        };
        let name = match filter.get("sName") {
            Some(FlashArg::String(s)) => Some(s.clone()),
            _ => None,
        };
        if id.is_none() && name.is_none() {
            return;
        }
        let wanted = Wanted {
            id,
            name,
            temp,
            asked: self.now().unwrap_or_default(),
        };
        let mut list = self.wanted.lock();
        list.retain(|w| !w.same(&wanted));
        list.push(wanted);
        let excess = list.len().saturating_sub(20);
        list.drain(..excess);
    }

    /// One of each item asked for lately, into the container it was asked of.
    fn drop_wanted(&self) {
        let now = self.now();
        let wanted: Vec<Wanted> = self
            .wanted
            .lock()
            .iter()
            .filter(|w| w.fresh(now))
            .cloned()
            .collect();
        // One of each: an item asked for by ID and by name is one item.
        let mut dropped = std::collections::HashSet::new();
        for w in wanted {
            let name = w
                .name
                .clone()
                .or_else(|| w.id.and_then(|id| self.materials.get(&id).cloned()))
                .unwrap_or_else(|| format!("Item {}", w.id.unwrap_or_default()));
            let id = w.id.unwrap_or_else(|| sim::id_for(&name));
            if dropped.insert((id, w.temp)) {
                self.give(id, &name, 1, w.temp);
            }
        }
    }

    /// One more of each item the quests in progress still need (a kill, a map item).
    fn give_quest_items(&self) {
        let quests: Vec<i64> = self.in_progress.lock().iter().copied().collect();
        for quest in quests {
            let Some(entry) = self.get(&format!("world.questTree.{quest}")) else {
                continue;
            };
            for req in sim::requirements(&entry) {
                if self.held(req.id, req.temp) < req.quantity {
                    self.give(req.id, &req.name, 1, req.temp);
                }
            }
        }
    }

    fn container(temp: bool) -> &'static str {
        if temp {
            "world.myAvatar.tempitems"
        } else {
            "world.myAvatar.items"
        }
    }

    fn held(&self, id: i64, temp: bool) -> i64 {
        match self.get(Self::container(temp)) {
            Some(Value::Array(items)) => sim::quantity(&items, id),
            _ => 0,
        }
    }

    fn find_item(&self, path: &str, id: i64) -> Option<Value> {
        match self.get(path) {
            Some(Value::Array(items)) => items
                .into_iter()
                .find(|i| sim::num(&i["ItemID"]) == Some(id)),
            _ => None,
        }
    }

    /// Moves an item's whole stack between the bank and the inventory or house (`to` None: the
    /// inventory, or the house for a house item), as the server's answer to a bank move does,
    /// and keeps the bank's count.
    fn move_item(&self, from: &str, to: Option<&str>, id: i64) {
        let Some(item) = self.find_item(from, id) else {
            return;
        };
        let house = item["bHouse"] == json!(1) || item["bHouse"] == json!("1");
        let to = to.unwrap_or(if house { HOUSE } else { Self::container(false) });
        let take = |path: &str, item: Option<Value>| {
            let mut items = match self.get(path) {
                Some(Value::Array(items)) => items,
                _ => Vec::new(),
            };
            items.retain(|i| sim::num(&i["ItemID"]) != Some(id));
            items.extend(item);
            self.set(path, Value::Array(items));
        };
        take(from, None);
        let mut moved = item;
        moved["bBank"] = json!(if to == BANK { 1 } else { 0 });
        // A stack already there grows; the item keeps its quantity otherwise.
        if let Some(there) = self.find_item(to, id) {
            let total =
                sim::num(&there["iQty"]).unwrap_or(0) + sim::num(&moved["iQty"]).unwrap_or(0);
            moved["iQty"] = json!(total);
        }
        take(to, Some(moved));
        if let Some(count) = self.get(BANK_COUNT).as_ref().and_then(sim::num) {
            let step = if to == BANK { 1 } else { -1 };
            self.set(BANK_COUNT, json!((count + step).max(0)));
        }
    }

    /// `inventoryHasSpaceFor(item, quantity, purchase, outgoing)`, as turu.swf answers it: a stack
    /// it adds to, an item swapped out for it, or a free bag slot (the house's for a house item).
    /// No slot count on the tape, or the game's category pools: room.
    fn has_space_for(&self, args: &[Value]) -> bool {
        let item: Value = args
            .first()
            .and_then(Value::as_str)
            .and_then(|text| serde_json::from_str(text).ok())
            .unwrap_or_default();
        if args.get(3).is_some_and(|out| !out.is_null()) {
            return true;
        }
        let house = item["bHouse"] == json!(1) || item["bHouse"] == json!("1");
        let (path, slots) = if house {
            (HOUSE, "world.myAvatar.objData.iHouseSlots")
        } else {
            (Self::container(false), "world.myAvatar.objData.iBagSlots")
        };
        if let Some(id) = sim::num(&item["ItemID"])
            && let Some(there) = self.find_item(path, id)
        {
            let quantity = args.get(1).and_then(sim::num).unwrap_or(1);
            let stack = sim::num(&there["iStk"]).unwrap_or(i64::MAX);
            return sim::num(&there["iQty"]).unwrap_or(0) + quantity <= stack;
        }
        // The game's category pools (`InvCat`) count the bag their own way, which the tape
        // doesn't show: a new item has room there.
        let categories = self.answers.get(&key(
            "callGameFunction",
            &[
                json!("loaderInfo.applicationDomain.hasDefinition"),
                json!("InvCat"),
            ],
        ));
        if categories.is_some_and(|answer| answer == "true") {
            return true;
        }
        let used = match self.get(path) {
            Some(Value::Array(items)) => items.len() as i64,
            _ => 0,
        };
        self.get(slots)
            .as_ref()
            .and_then(sim::num)
            .is_none_or(|slots| used < slots)
    }

    /// Adds an item to a container, as the game's `addItems` does.
    fn give(&self, id: i64, name: &str, quantity: i64, temp: bool) {
        let path = Self::container(temp);
        let mut items = match self.get(path) {
            Some(Value::Array(items)) => items,
            _ => Vec::new(),
        };
        sim::add_item(&mut items, id, name, quantity, temp);
        self.set(path, Value::Array(items));
        self.queue(sim::pext(json!({
            "cmd": "addItems",
            "items": { id.to_string(): { "ItemID": id, "sName": name, "iQty": quantity, "bTemp": temp } },
        })));
    }

    /// A turn-in: it always goes through (scripts wait for their items themselves). The quest's
    /// slot gets its value (done), its temporary items go, and the server's `ccqr` comes.
    fn turn_in(&self, quest: i64) {
        self.in_progress.lock().remove(&quest);
        let room = self.get("world.curRoom").unwrap_or(json!(1));
        self.queue(sim::sent(format!(
            "%xt%zm%tryQuestComplete%{room}%{quest}%-1%false%1%wvz%"
        )));
        if let Some(entry) = self.get(&format!("world.questTree.{quest}")) {
            if let (Some(slot), Some(value)) =
                (sim::num(&entry["iSlot"]), sim::num(&entry["iValue"]))
                && slot >= 0
            {
                let mut values = self.slot_values.lock();
                let have = values.entry(slot).or_insert(0);
                *have = (*have).max(value);
            }
            for req in sim::requirements(&entry).into_iter().filter(|r| r.temp) {
                let path = Self::container(true);
                if let Some(Value::Array(mut items)) = self.get(path) {
                    sim::take_item(&mut items, req.id, req.quantity);
                    self.set(path, Value::Array(items));
                }
            }
        }
        self.queue(sim::pext(
            json!({ "cmd": "ccqr", "bSuccess": 1, "QuestID": quest }),
        ));
    }

    /// How many calls it has answered.
    pub fn calls(&self) -> u64 {
        *self.calls.lock()
    }

    /// How many purchases went through.
    pub fn purchases(&self) -> u64 {
        *self.purchases.lock()
    }

    /// The last calls (up to 12), oldest first, each as `name(args) -> answer`.
    pub fn recent(&self) -> Vec<String> {
        self.recent.lock().iter().cloned().collect()
    }

    /// A path's value: the overlay, else the tape, else inside a value the tape has whole
    /// (`world.myAvatar.items.length` from `world.myAvatar.items`).
    fn get(&self, path: &str) -> Option<Value> {
        if path == "world.questTree" {
            return Some(self.quest_tree());
        }
        if let Some(value) = self.overlay.lock().get(path) {
            return Some(value.clone());
        }
        if let Some(value) = self.paths.get(path) {
            return Some(value.clone());
        }
        // An action's cooldown the tape never read is free (`world.lock.loadShop`).
        if path.starts_with("world.lock.") && path.matches('.').count() == 2 {
            return Some(serde_json::json!({ "ts": 0, "cd": 0 }));
        }
        let mut base = path;
        while let Some((parent, _)) = base.rsplit_once('.') {
            base = parent;
            let whole = self.overlay.lock().get(base).cloned();
            if let Some(whole) = whole.or_else(|| self.paths.get(base).cloned()) {
                return Some(sub_path(&whole, &path[base.len() + 1..]));
            }
        }
        None
    }

    /// Every quest loaded: the tape's and the ones scripts loaded since.
    fn quest_tree(&self) -> Value {
        let mut tree = serde_json::Map::new();
        if let Some(Value::Object(whole)) = self.paths.get("world.questTree") {
            tree.extend(whole.clone());
        }
        let overlay = self.overlay.lock();
        // The tape's single quests, then the overlay's (which win).
        for (path, quest) in self.paths.iter().chain(overlay.iter()) {
            if let Some(id) = path.strip_prefix("world.questTree.")
                && id.parse::<i64>().is_ok()
            {
                tree.insert(id.to_owned(), quest.clone());
            }
        }
        Value::Object(tree)
    }

    /// Loading quests (`world.showQuests("1,2,3")`): each one the world hasn't yet comes as a quest
    /// the tape has, its ID, name, slot, requirement and reward changed, as the server's
    /// `getQuests` answer would fill `world.questTree`.
    fn load_quests(&self, ids: &str) {
        let template = self
            .paths
            .iter()
            .find(|(path, value)| {
                path.strip_prefix("world.questTree.")
                    .is_some_and(|id| id.parse::<i64>().is_ok())
                    && value.is_object()
            })
            .map(|(_, quest)| quest.clone())
            .unwrap_or_else(|| serde_json::json!({ "iLvl": 1, "bOnce": 0 }));
        for id in ids
            .split(',')
            .filter_map(|id| id.trim().parse::<i64>().ok())
        {
            let path = format!("world.questTree.{id}");
            if self.get(&path).is_some_and(|q| !q.is_null()) {
                continue;
            }
            let mut quest = template.clone();
            if let Some(fields) = quest.as_object_mut() {
                fields.insert("QuestID".into(), Value::from(id));
                fields.insert("sName".into(), Value::from(format!("Quest {id}")));
                fields.remove("status");
                // A slot of its own (the quest's ID, above any real slot's count) at value 1: not
                // done (its value reads 0) but unlocked, so the script does the quest. A slot of
                // -1 (a repeatable quest) reads as done.
                fields.insert("iSlot".into(), Value::from(id));
                fields.insert("iValue".into(), Value::from(1));
                // One item to turn in and one reward, placeholders named after the quest: real
                // quests have both, and scripts read them (`rewards[0]`).
                let (need, give) = (900_000_000 + id, 910_000_000 + id);
                let item = |item_id: i64, name: String| serde_json::json!({ "ItemID": item_id, "sName": name, "iQty": 1, "iStk": 1 });
                fields.insert(
                    "oItems".into(),
                    serde_json::json!({ need.to_string(): item(need, format!("Item for quest {id}")) }),
                );
                fields.insert(
                    "turnin".into(),
                    serde_json::json!([{ "ItemID": need, "iQty": 1 }]),
                );
                fields.insert(
                    "oRewards".into(),
                    serde_json::json!({ "itemsS": { give.to_string(): item(give, format!("Reward of quest {id}")) } }),
                );
            }
            self.set(&path, quest);
        }
    }

    /// Loading a shop: the tape's shop when it is that one, else what the scripts say it sells
    /// ([`Self::with_shops`]), else an empty shop with its ID (a script then goes on into its own
    /// code, finding nothing to buy).
    fn load_shop(&self, id: i64) {
        let recorded = self
            .paths
            .get("world.shopinfo")
            .filter(|shop| shop["ShopID"].as_f64() == Some(id as f64))
            .cloned();
        let shop = recorded
            .unwrap_or_else(|| sim::shop(id, self.shops.get(&id).unwrap_or(&ShopStock::default())));
        self.set("world.shopinfo", shop);
    }

    /// The loaded shop's entry for an item (`shop_item_id` <= 0: any), as turu.swf's `shopItem`.
    fn shop_item(&self, id: i64, shop_item_id: i64) -> Option<Value> {
        match self.get("world.shopinfo.items") {
            Some(Value::Array(items)) => items.into_iter().find(|item| {
                sim::num(&item["ItemID"]) == Some(id)
                    && (shop_item_id <= 0 || sim::num(&item["ShopItemID"]) == Some(shop_item_id))
            }),
            _ => None,
        }
    }

    /// `getUnmetPurchaseRequirements`: the materials missing for `bundles` of `item`, worded as
    /// turu.swf words them (the fake's shops have no quest or token requirement).
    fn unmet(&self, item: Option<&Value>, bundles: i64) -> Vec<String> {
        let Some(item) = item else {
            return vec!["The shop doesn't sell this item.".into()];
        };
        sim::values(&item["turnin"])
            .filter_map(|m| {
                let need = sim::num(&m["iQty"]).unwrap_or(1) * bundles;
                let have = self.held(sim::num(&m["ItemID"])?, false);
                (have < need).then(|| {
                    format!(
                        "Missing {} x{}.",
                        m["sName"].as_str().unwrap_or_default(),
                        need - have
                    )
                })
            })
            .collect()
    }

    /// A purchase (`buyItemByID`, `buyItemByName`; `quantity` -1 is one bundle): with its
    /// materials held, they go and the item comes, with the server's `buyItem` answer; without,
    /// the answer says it failed.
    fn buy(&self, item: Option<Value>, quantity: i64) {
        let bundles = quantity.max(1);
        let Some(item) = item else { return };
        let (Some(id), name) = (
            sim::num(&item["ItemID"]),
            item["sName"].as_str().unwrap_or_default(),
        ) else {
            return;
        };
        let bought = sim::num(&item["iQty"]).unwrap_or(1).max(1) * bundles;
        if !self.unmet(Some(&item), bundles).is_empty() {
            self.queue(sim::pext(
                json!({ "cmd": "buyItem", "bitSuccess": 0, "ItemID": id }),
            ));
            return;
        }
        let path = Self::container(false);
        let mut items = match self.get(path) {
            Some(Value::Array(items)) => items,
            _ => Vec::new(),
        };
        for m in sim::values(&item["turnin"]) {
            if let Some(material) = sim::num(&m["ItemID"]) {
                sim::take_item(
                    &mut items,
                    material,
                    sim::num(&m["iQty"]).unwrap_or(1) * bundles,
                );
            }
        }
        sim::add_item(&mut items, id, name, bought, false);
        self.set(path, Value::Array(items));
        *self.purchases.lock() += 1;
        self.queue(sim::pext(json!({
            "cmd": "buyItem", "bitSuccess": 1, "CharItemID": id, "iQty": bought, "ItemID": id, "bBank": 0,
        })));
    }

    /// `selectArrayObjectsWhere(path, filter)`: the items of `path` whose fields match the
    /// filter's, as turu.swf matches them; `None` when the call isn't that shape.
    fn select_where(&self, args: &[FlashArg]) -> Option<String> {
        let (Some(FlashArg::String(path)), Some(FlashArg::Object(filter))) =
            (args.first(), args.get(1))
        else {
            return None;
        };
        self.want(path, filter);
        let pick = |items: &[Value]| -> String {
            let picked: Vec<&Value> = items
                .iter()
                .filter(|item| {
                    filter.iter().all(|(key, want)| {
                        crate::testing::loosely(item.get(key).unwrap_or(&Value::Null), want)
                    })
                })
                .collect();
            serde_json::to_string(&picked).unwrap_or_default()
        };
        // Borrowed where it is kept whole (not copied: an inventory is large).
        if let Some(value) = self.overlay.lock().get(path.as_str()) {
            return Some(
                value
                    .as_array()
                    .map_or_else(String::new, |items| pick(items)),
            );
        }
        if let Some(value) = self.paths.get(path.as_str()) {
            return Some(
                value
                    .as_array()
                    .map_or_else(String::new, |items| pick(items)),
            );
        }
        match self.get(path) {
            Some(Value::Array(items)) => Some(pick(&items)),
            _ => Some(String::new()),
        }
    }

    fn set(&self, path: &str, value: Value) {
        self.overlay.lock().insert(path.to_owned(), value);
        // Its text (and a parent's or child's that contains it) is out of date.
        self.texts
            .lock()
            .retain(|p, _| !(p.starts_with(path) || path.starts_with(p.as_str())));
    }

    /// A path's value as JSON text, written once until the path changes: a high-level
    /// character's whole inventory is large, and some scripts read it on every pass.
    fn text(&self, path: &str) -> String {
        if let Some(text) = self.texts.lock().get(path) {
            return text.clone();
        }
        let text = self
            .get(path)
            .map_or_else(|| "null".into(), |v| v.to_string());
        if text.len() > 4096 {
            self.texts.lock().insert(path.to_owned(), text.clone());
        }
        text
    }

    /// The rules: what a send changes.
    fn send(&self, name: &str, args: &[Value]) {
        let text = |i: usize| args.get(i).and_then(Value::as_str).unwrap_or_default();
        match name {
            "setGameObject" => {
                if let Some(value) = args.get(1) {
                    self.set(text(0), as_flash_writes(value));
                }
            }
            "jumpCorrectRoom" => {
                self.set("world.strFrame", Value::from(text(0)));
                self.set("world.strPad", Value::from(text(1)));
            }
            "sendClientPacket" => self.packet(text(0)),
            "callGameFunction" if text(0) == "world.showQuests" => self.load_quests(text(1)),
            "callGameFunction" if text(0) == "world.sendLoadShopRequest" => {
                if let Some(id) = args.get(1).and_then(Value::as_f64) {
                    self.load_shop(id as i64);
                }
            }
            "callGameFunction" if text(0) == "world.acceptQuest" => {
                if let Some(id) = args.get(1).and_then(sim::num) {
                    self.in_progress.lock().insert(id);
                    let room = self.get("world.curRoom").unwrap_or(json!(1));
                    self.queue(sim::sent(format!("%xt%zm%acceptQuest%{room}%{id}%")));
                }
            }
            "callGameFunction" if text(0) == "world.tryQuestComplete" => {
                if let Some(id) = args.get(1).and_then(sim::num) {
                    self.turn_in(id);
                }
            }
            // A map item: what the quests in progress still need.
            "callGameFunction" if text(0) == "world.getMapItem" => self.give_quest_items(),
            "callGameFunction" if text(0) == "sfc.sendString" => self.packet(text(1)),
            _ => {}
        }
    }

    /// A packet the client sends: joining a map (`%xt%zm%cmd%<room>%tfer%<user>%<map>%<cell>%<pad>%`).
    fn packet(&self, packet: &str) {
        let parts: Vec<&str> = packet.split('%').collect();
        let id = |i: usize| parts.get(i).and_then(|p| p.trim().parse::<i64>().ok());
        match parts.get(3).copied() {
            // The server's answer; the bank itself is the tape's.
            Some("loadBank") => self.queue(sim::pext(json!({ "cmd": "loadBank", "items": [] }))),
            Some("bankToInv") => {
                if let Some(item) = id(5) {
                    self.move_item(BANK, None, item);
                }
            }
            Some("bankFromInv") => {
                if let Some(item) = id(5) {
                    let from = [Self::container(false), HOUSE]
                        .into_iter()
                        .find(|path| self.find_item(path, item).is_some());
                    if let Some(from) = from {
                        self.move_item(from, Some(BANK), item);
                    }
                }
            }
            Some("bankSwapInv") => {
                if let (Some(out), Some(item)) = (id(5), id(7)) {
                    self.move_item(Self::container(false), Some(BANK), out);
                    self.move_item(BANK, None, item);
                }
            }
            _ => {}
        }
        if parts.get(5) == Some(&"tfer") && parts.len() >= 10 {
            let map = parts[7].split('-').next().unwrap_or_default();
            self.set("world.strMapName", Value::from(map));
            self.set("world.strFrame", Value::from(parts[8]));
            self.set("world.strPad", Value::from(parts[9]));
            self.set("world.mapLoadInProgress", Value::Bool(false));
            // The map's monsters, from the catalog (none for a map it doesn't have).
            let names = self
                .catalog
                .get(&map.to_lowercase())
                .cloned()
                .unwrap_or_default();
            self.combat.lock().enter(&names);
        }
    }

    fn answer(&self, name: &str, args: Vec<FlashArg>) -> String {
        let json: Vec<Value> = args.iter().map(crate::tape::arg_json).collect();
        let first = json.first().and_then(Value::as_str).unwrap_or_default();
        match name {
            "getGameObject" | "getGameObjectS" => return self.text(first),
            "isNull" => {
                let null = match self.get(first) {
                    Some(value) => value.is_null(),
                    None => self.nulls.get(first).copied().unwrap_or_else(|| {
                        // Not null when the tape has values under it (`world.myAvatar`, which
                        // turu.swf's `snapshot` checks without a read of its own).
                        let below = format!("{first}.");
                        !self.paths.keys().any(|p| p.starts_with(&below))
                    }),
                };
                return null.to_string();
            }
            "isLoggedIn" => return "true".into(),
            "inventoryHasSpaceFor" => return self.has_space_for(&json).to_string(),
            "getMonsters" => return self.monsters(false),
            "availableMonsters" => return self.monsters(true),
            "getTargetMonster" => return self.target().to_string(),
            "attackMonsterID" => {
                let id = json.first().and_then(sim::num);
                return self.combat.lock().attack(id, None).to_string();
            }
            "attackMonsterName" => {
                return self.combat.lock().attack(None, Some(first)).to_string();
            }
            "useSkill" => {
                self.hit();
                return "true".into();
            }
            // A quest slot's value says which quests are done: one the tape never read is 0 (not
            // done), never another slot's.
            "callGameFunction" if first == "world.getQuestValue" => {
                let slot = json.get(1).and_then(sim::num).unwrap_or(-1);
                if let Some(value) = self.slot_values.lock().get(&slot) {
                    return value.to_string();
                }
                return self
                    .answers
                    .get(&key(name, &json))
                    .cloned()
                    .unwrap_or_else(|| "0".into());
            }
            "callGameFunction" if first == "world.isQuestInProgress" => {
                let id = json.get(1).and_then(Value::as_f64).unwrap_or(-1.0) as i64;
                return self.in_progress.lock().contains(&id).to_string();
            }
            "callGameFunction0" if first == "world.myAvatar.pMC.artLoaded" => return "true".into(),
            "getUnmetPurchaseRequirements" => {
                let num = |i: usize| json.get(i).and_then(sim::num).unwrap_or(-1);
                let item = self.shop_item(num(0), num(1));
                let unmet = self.unmet(item.as_ref(), num(2).max(1));
                return Value::from(unmet).to_string();
            }
            "buyItemByID" => {
                let num = |i: usize| json.get(i).and_then(sim::num).unwrap_or(-1);
                self.buy(self.shop_item(num(0), num(1)), num(2));
                return "null".into();
            }
            "buyItemByName" => {
                let item = match self.get("world.shopinfo.items") {
                    Some(Value::Array(items)) => items.into_iter().find(|item| {
                        item["sName"]
                            .as_str()
                            .is_some_and(|n| n.eq_ignore_ascii_case(first))
                    }),
                    _ => None,
                };
                let quantity = json.get(1).and_then(sim::num).unwrap_or(-1);
                self.buy(item, quantity);
                return "null".into();
            }
            _ => {}
        }
        self.send(name, &json);
        self.answers
            .get(&key(name, &json))
            .or_else(|| self.by_name.get(&loose_key(name, &json)))
            .cloned()
            .unwrap_or_else(|| "null".into())
    }
}

impl FlashHost for WorldFake {
    fn call(&self, name: &str, args: Vec<FlashArg>) -> Result<String, HostError> {
        *self.calls.lock() += 1;
        let shown: Vec<String> = args
            .iter()
            .map(|a| crate::tape::arg_json(a).to_string())
            .collect();
        // `snapshot` as the game answered it (its parts aren't all on the tape as reads of their
        // own); the other batched reads built from the single ones.
        let recorded_snapshot = (name == "snapshot")
            .then(|| {
                let json: Vec<Value> = args.iter().map(crate::tape::arg_json).collect();
                self.answers.get(&key(name, &json)).cloned()
            })
            .flatten();
        // `selectArrayObjectsWhere` filtered on the value it has, not written out whole and read
        // back (a high-level character's inventory makes that slow, and scripts poll it).
        let selected = (name == "selectArrayObjectsWhere")
            .then(|| self.select_where(&args))
            .flatten();
        // Its target is the fake's, not the tape's.
        let recorded_snapshot = recorded_snapshot.map(|answer| {
            let with_target = matches!(args.first(), Some(FlashArg::Bool(true)));
            match serde_json::from_str::<Value>(&answer) {
                Ok(mut snap) if with_target => {
                    let target = self.target();
                    snap["target"] = if target.as_object().is_some_and(|t| t.is_empty()) {
                        Value::Null
                    } else {
                        target
                    };
                    snap.to_string()
                }
                _ => answer,
            }
        });
        let answer = match recorded_snapshot.or(selected) {
            Some(answer) => answer,
            None => crate::testing::batched(name, &args, |n, a| self.answer(n, a))
                .unwrap_or_else(|| self.answer(name, args)),
        };
        let mut recent = self.recent.lock();
        if recent.len() == 12 {
            recent.pop_front();
        }
        let cut: String = answer.chars().take(80).collect();
        recent.push_back(format!("{name}({}) -> {cut}", shown.join(", ")));
        drop(recent);
        // What the call set off reaches the bot as the game's packets would, no lock held.
        let events = std::mem::take(&mut *self.pending.lock());
        if !events.is_empty() {
            sim::deliver(self.bot.lock().upgrade(), events);
        }
        Ok(answer)
    }
}

fn key(name: &str, args: &[Value]) -> String {
    format!("{name}{}", Value::from(args.to_vec()))
}

/// `value` as turu.swf's JSON writes it: Flash's numbers are doubles, written without `.0` when
/// whole.
fn as_flash_writes(value: &Value) -> Value {
    match value {
        Value::Number(n) => match n.as_f64() {
            Some(f) if f.fract() == 0.0 && f.abs() < 1e15 => Value::from(f as i64),
            _ => value.clone(),
        },
        Value::Array(items) => Value::Array(items.iter().map(as_flash_writes).collect()),
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(k, v)| (k.clone(), as_flash_writes(v)))
                .collect(),
        ),
        other => other.clone(),
    }
}

/// A call's name and its first argument when that is text (a path, a game function).
fn loose_key(name: &str, args: &[Value]) -> String {
    match args.first().and_then(Value::as_str) {
        Some(first) => format!("{name}({first})"),
        None => name.to_owned(),
    }
}

fn parse(raw: &str) -> Value {
    serde_json::from_str(raw.trim()).unwrap_or(Value::Null)
}

/// `rest` (`items.length`, `items.0.sName`) inside `value`, as turu.swf's `getGameObject` would.
fn sub_path(value: &Value, rest: &str) -> Value {
    let mut value = value.clone();
    for segment in rest.split('.') {
        value = match (&value, segment) {
            (Value::Array(items), "length") => Value::from(items.len()),
            (Value::Array(items), index) => index
                .parse::<usize>()
                .ok()
                .and_then(|i| items.get(i).cloned())
                .unwrap_or(Value::Null),
            (Value::Object(fields), key) => fields.get(key).cloned().unwrap_or(Value::Null),
            _ => Value::Null,
        };
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tape::{Header, Line, Tape};
    use serde_json::json;

    fn call(n: u64, name: &str, args: Value, answer: &str) -> Line {
        Line::Call {
            n,
            t: n,
            thread: Some("Script Thread".into()),
            name: name.into(),
            args: args.as_array().unwrap().clone(),
            answer: Some(answer.into()),
            error: None,
            run_ms: None,
        }
    }

    fn world() -> WorldFake {
        let lines = vec![
            call(
                0,
                "getGameObject",
                json!(["world.strMapName"]),
                r#""yulgar""#,
            ),
            call(
                1,
                "getGameObject",
                json!(["world.myAvatar.items"]),
                r#"[{"sName":"Gear"},{"sName":"Cog"}]"#,
            ),
            call(
                2,
                "getMany",
                json!([{"gold": "world.myAvatar.objData.intGold"}]),
                r#"{"gold":40}"#,
            ),
            call(3, "isNull", json!(["mcConnDetail.stage"]), "true"),
            call(
                4,
                "callGameFunction",
                json!(["world.getQuestValue", 5.0]),
                "7",
            ),
            call(
                5,
                "callGameFunction",
                json!(["world.getQuestValue", 6.0]),
                "9",
            ),
            Line::Note {
                n: 6,
                t: 6,
                what: "scriptStopped".into(),
                detail: Value::Null,
            },
            // After the script: the client's logout at exit, which isn't the world.
            call(7, "getGameObject", json!(["world.strMapName"]), "null"),
        ];
        from_lines(lines)
    }

    fn from_lines(lines: Vec<Line>) -> WorldFake {
        WorldFake::from_tape(&Tape {
            header: Header {
                format: 1,
                turu: String::new(),
                started_unix_ms: 0,
                from_world: true,
                options: Value::Null,
                scrubbed: true,
            },
            lines,
            end: None,
            missing: Vec::new(),
            truncated: false,
        })
    }

    fn ask(world: &WorldFake, name: &str, args: Vec<FlashArg>) -> String {
        world.call(name, args).unwrap()
    }

    #[test]
    fn the_bank_moves_items_and_counts_the_bag() {
        let essence = r#"{"ItemID":73641,"CharItemID":11,"sName":"Void Essence","iQty":25,"iStk":25,"bBank":1,"bHouse":"0"}"#;
        let chair = r#"{"ItemID":500,"CharItemID":12,"sName":"Chair","iQty":1,"iStk":1,"bBank":1,"bHouse":"1"}"#;
        let world = from_lines(vec![
            call(0, "getGameObject", json!(["world.curRoom"]), "7"),
            call(
                1,
                "getGameObject",
                json!(["world.myAvatar.items"]),
                r#"[{"ItemID":1,"sName":"Gear","iQty":1,"iStk":1}]"#,
            ),
            call(
                2,
                "getGameObject",
                json!(["world.bankinfo.items"]),
                &format!("[{essence},{chair}]"),
            ),
            call(
                3,
                "getGameObject",
                json!(["world.myAvatar.objData.iBagSlots"]),
                "2",
            ),
            call(
                4,
                "getGameObject",
                json!(["world.myAvatar.iBankCount"]),
                "2",
            ),
        ]);
        let names = |path: &str| -> Vec<String> {
            let items: Vec<Value> =
                serde_json::from_str(&ask(&world, "getGameObject", vec![path.into()])).unwrap();
            items
                .iter()
                .map(|i| i["sName"].as_str().unwrap().to_owned())
                .collect()
        };
        let space = |item: &str| {
            ask(
                &world,
                "inventoryHasSpaceFor",
                vec![item.into(), 1.0.into(), false.into()],
            )
        };
        let send = |packet: &str| {
            ask(
                &world,
                "callGameFunction",
                vec!["sfc.sendString".into(), packet.into()],
            );
        };

        // One of two bag slots used: room for the essence.
        assert_eq!(space(essence), "true");
        send("%xt%zm%bankToInv%7%73641%11%");
        assert_eq!(names("world.myAvatar.items"), ["Gear", "Void Essence"]);
        assert_eq!(names("world.bankinfo.items"), ["Chair"]);
        assert_eq!(
            ask(
                &world,
                "getGameObject",
                vec!["world.myAvatar.iBankCount".into()]
            ),
            "1"
        );
        // The bag is full now, but a full stack is still a full stack and a house item goes to the house.
        assert_eq!(space(r#"{"ItemID":2,"sName":"Cog","iQty":1}"#), "false");
        assert_eq!(space(essence), "false");
        send("%xt%zm%bankToInv%7%500%12%");
        assert_eq!(names("world.myAvatar.houseitems"), ["Chair"]);

        // Back to the bank, and a swap.
        send("%xt%zm%bankFromInv%7%73641%11%");
        assert_eq!(names("world.bankinfo.items"), ["Void Essence"]);
        send("%xt%zm%bankSwapInv%7%1%1%73641%11%");
        assert_eq!(names("world.myAvatar.items"), ["Void Essence"]);
        assert_eq!(names("world.bankinfo.items"), ["Gear"]);

        // `loadBank` is answered, as the server does: the bot hears the bank loaded.
        let bot = Bot::new();
        world.attach(&bot);
        send("%xt%zm%loadBank%7%All%");
        assert!(bot.state.lock().bank_loaded);
    }

    #[test]
    fn reads_come_from_the_tape_up_to_the_scripts_end() {
        let world = world();
        assert_eq!(
            ask(&world, "getGameObject", vec!["world.strMapName".into()]),
            r#""yulgar""#
        );
        assert_eq!(
            ask(
                &world,
                "getGameObject",
                vec!["world.myAvatar.items.length".into()]
            ),
            "2"
        );
        assert_eq!(
            ask(
                &world,
                "getGameObject",
                vec!["world.myAvatar.objData.intGold".into()]
            ),
            "40"
        );
        // `world.myAvatar` has values under it: not null.
        assert_eq!(
            ask(&world, "isNull", vec!["world.myAvatar".into()]),
            "false"
        );
        assert_eq!(
            ask(&world, "isNull", vec!["mcConnDetail.stage".into()]),
            "true"
        );
        // A cooldown the tape never read is free. Compared by content: the app's build keeps
        // serde_json's key order (`preserve_order`, from Ruffle's dependencies), this crate's
        // alone doesn't, and CI builds them together.
        let lock: serde_json::Value = serde_json::from_str(&ask(
            &world,
            "getGameObject",
            vec!["world.lock.buyItem".into()],
        ))
        .unwrap();
        assert_eq!(lock, serde_json::json!({"cd": 0, "ts": 0}));
        // A quest slot answers its own value, else 0, never another slot's.
        let slot = |n: f64| {
            ask(
                &world,
                "callGameFunction",
                vec!["world.getQuestValue".into(), n.into()],
            )
        };
        assert_eq!(
            (slot(5.0), slot(6.0), slot(8.0)),
            ("7".into(), "9".into(), "0".into())
        );
    }

    #[test]
    fn sends_move_you_and_set_values() {
        let world = world();
        ask(
            &world,
            "callGameFunction",
            vec![
                "sfc.sendString".into(),
                "%xt%zm%cmd%1%tfer%Questwalker%battleon-1234%r2%Left%".into(),
            ],
        );
        assert_eq!(
            ask(&world, "getGameObject", vec!["world.strMapName".into()]),
            r#""battleon""#
        );
        assert_eq!(
            ask(&world, "getGameObject", vec!["world.strFrame".into()]),
            r#""r2""#
        );
        ask(&world, "jumpCorrectRoom", vec!["r5".into(), "Right".into()]);
        assert_eq!(
            ask(&world, "getGameObject", vec!["world.strPad".into()]),
            r#""Right""#
        );
        ask(
            &world,
            "setGameObject",
            vec!["world.myAvatar.objData.intGold".into(), 7.0.into()],
        );
        assert_eq!(
            ask(
                &world,
                "getGameObject",
                vec!["world.myAvatar.objData.intGold".into()]
            ),
            "7"
        );
    }

    #[test]
    fn quests_load_accept_and_turn_in() {
        let world = world();
        ask(
            &world,
            "callGameFunction",
            vec!["world.showQuests".into(), "4777,4778".into(), "q".into()],
        );
        let quest: Value = serde_json::from_str(&ask(
            &world,
            "getGameObject",
            vec!["world.questTree.4777".into()],
        ))
        .unwrap();
        assert_eq!(
            (
                quest["QuestID"].clone(),
                quest["iSlot"].clone(),
                quest["iValue"].clone()
            ),
            (json!(4777), json!(4777), json!(1))
        );
        let parsed: crate::models::Quest = serde_json::from_value(quest).unwrap();
        assert_eq!(parsed.requirements[0].name, "Item for quest 4777");
        assert_eq!(parsed.rewards[0].name, "Reward of quest 4777");
        let tree: Value = serde_json::from_str(&ask(
            &world,
            "getGameObject",
            vec!["world.questTree".into()],
        ))
        .unwrap();
        assert!(tree.get("4778").is_some(), "{tree}");
        let in_progress = || {
            ask(
                &world,
                "callGameFunction",
                vec!["world.isQuestInProgress".into(), 4777.0.into()],
            )
        };
        assert_eq!(in_progress(), "false");
        ask(
            &world,
            "callGameFunction",
            vec!["world.acceptQuest".into(), 4777.0.into()],
        );
        assert_eq!(in_progress(), "true");
        ask(
            &world,
            "callGameFunction",
            vec![
                "world.tryQuestComplete".into(),
                4777.0.into(),
                (-1.0).into(),
                false.into(),
            ],
        );
        assert_eq!(in_progress(), "false");
    }

    fn world_with_a_merge_shop() -> WorldFake {
        world_with_monsters().with_shops(HashMap::from([(
            332,
            ShopStock {
                items: vec![
                    (29576, "NO BOTS Armor".to_owned()),
                    (29753, "Sock Monkey Cape".to_owned()),
                ],
                materials: vec!["Synderes' Souvenir".to_owned()],
            },
        )]))
    }

    fn held_by_name(world: &WorldFake, name: &str) -> Vec<Value> {
        let filter = FlashArg::Object([("sName".to_owned(), FlashArg::from(name))].into());
        serde_json::from_str(&ask(
            world,
            "selectArrayObjectsWhere",
            vec!["world.myAvatar.items".into(), filter],
        ))
        .unwrap()
    }

    #[test]
    fn a_merge_shop_sells_its_scripts_items_for_its_materials() {
        let world = world_with_a_merge_shop();
        ask(
            &world,
            "callGameFunction",
            vec!["world.sendLoadShopRequest".into(), 332.0.into()],
        );
        let items: Vec<Value> = serde_json::from_str(&ask(
            &world,
            "getGameObject",
            vec!["world.shopinfo.items".into()],
        ))
        .unwrap();
        // Both readers take every item: the engine's `ShopItem`, and CoreBots' raw fields.
        let read: Vec<crate::models::ShopItem> = items
            .iter()
            .filter_map(|i| serde_json::from_value(i.clone()).ok())
            .collect();
        assert_eq!(read.len(), 2);
        assert_eq!(read[0].item.name, "NO BOTS Armor");
        let souvenir = sim::id_for("Synderes' Souvenir");
        assert_eq!(read[0].turnin[0].id, souvenir);
        assert_eq!(items[0]["turnin"][0]["sName"], "Synderes' Souvenir");
        let shop_item_id = read[0].shop_item_id as f64;

        let unmet = |world: &WorldFake| -> Value {
            serde_json::from_str(&ask(
                world,
                "getUnmetPurchaseRequirements",
                vec![29576.0.into(), shop_item_id.into(), 1.0.into()],
            ))
            .unwrap()
        };
        assert_eq!(unmet(&world), json!(["Missing Synderes' Souvenir x1."]));

        // Asked for by ID (the merge's check), then by name (`findIngredients`): a kill drops it
        // under its name.
        let by_id =
            FlashArg::Object([("ItemID".to_owned(), FlashArg::from(souvenir as f64))].into());
        ask(
            &world,
            "selectArrayObjectsWhere",
            vec!["world.myAvatar.items".into(), by_id],
        );
        held_by_name(&world, "Synderes' Souvenir");
        join(&world, "frozentower");
        ask(&world, "attackMonsterID", vec![1.0.into()]);
        for _ in 0..3 {
            ask(&world, "useSkill", vec![1.0.into()]);
        }
        assert_eq!(held_by_name(&world, "Synderes' Souvenir").len(), 1);
        assert_eq!(unmet(&world), json!([]));

        ask(
            &world,
            "buyItemByID",
            vec![29576.0.into(), shop_item_id.into(), 1.0.into()],
        );
        assert_eq!(held_by_name(&world, "NO BOTS Armor").len(), 1);
        assert!(held_by_name(&world, "Synderes' Souvenir").is_empty());
        assert_eq!(world.purchases(), 1);
        // Without the materials it doesn't go through.
        ask(
            &world,
            "buyItemByName",
            vec!["Sock Monkey Cape".into(), (-1.0).into()],
        );
        assert!(held_by_name(&world, "Sock Monkey Cape").is_empty());
        assert_eq!(world.purchases(), 1);
        // A shop no script lists is empty.
        ask(
            &world,
            "callGameFunction",
            vec!["world.sendLoadShopRequest".into(), 9.0.into()],
        );
        assert_eq!(
            ask(&world, "getGameObject", vec!["world.shopinfo.items".into()]),
            "[]"
        );
    }

    #[test]
    fn a_shop_loads_with_its_id() {
        let world = world();
        ask(
            &world,
            "callGameFunction",
            vec!["world.sendLoadShopRequest".into(), 332.0.into()],
        );
        assert_eq!(
            ask(&world, "isNull", vec!["world.shopinfo".into()]),
            "false"
        );
        assert_eq!(
            ask(
                &world,
                "getGameObject",
                vec!["world.shopinfo.ShopID".into()]
            ),
            "332"
        );
    }

    fn world_with_monsters() -> WorldFake {
        world().with_monsters(HashMap::from([(
            "frozentower".to_owned(),
            vec!["Polar Elemental".to_owned()],
        )]))
    }

    fn join(world: &WorldFake, map: &str) {
        ask(
            world,
            "callGameFunction",
            vec![
                "sfc.sendString".into(),
                format!("%xt%zm%cmd%1%tfer%Questwalker%{map}%Enter%Spawn%").into(),
            ],
        );
    }

    #[test]
    fn a_maps_monsters_come_from_the_catalog_and_stand_in_your_cell() {
        let world = world_with_monsters();
        join(&world, "frozentower-1234");
        ask(&world, "jumpCorrectRoom", vec!["r5".into(), "Left".into()]);
        let monsters: Vec<Value> =
            serde_json::from_str(&ask(&world, "getMonsters", vec![])).unwrap();
        assert_eq!(monsters.len(), 1);
        assert_eq!(monsters[0]["strMonName"], "Polar Elemental");
        assert_eq!(monsters[0]["strFrame"], "r5");
        // A map the catalog doesn't have: none.
        join(&world, "nowhere");
        assert_eq!(ask(&world, "getMonsters", vec![]), "[]");
    }

    #[test]
    fn a_monster_dies_on_the_third_hit_and_comes_back() {
        let world = world_with_monsters();
        join(&world, "frozentower");
        assert_eq!(
            ask(&world, "attackMonsterName", vec!["polar elemental".into()]),
            "true"
        );
        let target: Value = serde_json::from_str(&ask(&world, "getTargetMonster", vec![])).unwrap();
        assert_eq!(target["strMonName"], "Polar Elemental");
        ask(&world, "useSkill", vec![1.0.into()]);
        ask(&world, "useSkill", vec![2.0.into()]);
        assert_ne!(ask(&world, "getTargetMonster", vec![]), "{}");
        ask(&world, "useSkill", vec![3.0.into()]);
        assert_eq!(ask(&world, "getTargetMonster", vec![]), "{}");
        // With no bot (no clock), the dead come back at the next look.
        let alive: Vec<Value> =
            serde_json::from_str(&ask(&world, "availableMonsters", vec![])).unwrap();
        assert_eq!(alive.len(), 1);
    }

    #[test]
    fn a_kill_drops_what_the_script_asked_for() {
        let world = world_with_monsters();
        join(&world, "frozentower");
        let filter =
            FlashArg::Object([("sName".to_owned(), FlashArg::from("Frozen Shard"))].into());
        ask(
            &world,
            "selectArrayObjectsWhere",
            vec!["world.myAvatar.tempitems".into(), filter.clone()],
        );
        ask(&world, "attackMonsterID", vec![1.0.into()]);
        for _ in 0..3 {
            ask(&world, "useSkill", vec![1.0.into()]);
        }
        let held: Vec<Value> = serde_json::from_str(&ask(
            &world,
            "selectArrayObjectsWhere",
            vec!["world.myAvatar.tempitems".into(), filter],
        ))
        .unwrap();
        assert_eq!(held.len(), 1, "{held:?}");
        assert_eq!(held[0]["iQty"], 1);
    }

    #[test]
    fn a_map_item_gives_what_a_quest_needs_and_a_turn_in_marks_it_done() {
        let world = world();
        ask(
            &world,
            "callGameFunction",
            vec!["world.showQuests".into(), "4777".into(), "q".into()],
        );
        ask(
            &world,
            "callGameFunction",
            vec!["world.acceptQuest".into(), 4777.0.into()],
        );
        ask(
            &world,
            "callGameFunction",
            vec!["world.getMapItem".into(), 1.0.into()],
        );
        let need =
            FlashArg::Object([("sName".to_owned(), FlashArg::from("Item for quest 4777"))].into());
        let held = ask(
            &world,
            "selectArrayObjectsWhere",
            vec!["world.myAvatar.tempitems".into(), need.clone()],
        );
        assert!(held.contains("Item for quest 4777"), "{held}");

        let slot = || {
            ask(
                &world,
                "callGameFunction",
                vec!["world.getQuestValue".into(), 4777.0.into()],
            )
        };
        assert_eq!(slot(), "0");
        ask(
            &world,
            "callGameFunction",
            vec![
                "world.tryQuestComplete".into(),
                4777.0.into(),
                (-1.0).into(),
                false.into(),
            ],
        );
        // Done: its slot holds its value, it isn't in progress, and its item was handed in.
        assert_eq!(slot(), "1");
        assert_eq!(
            ask(
                &world,
                "callGameFunction",
                vec!["world.isQuestInProgress".into(), 4777.0.into()]
            ),
            "false"
        );
        assert_eq!(
            ask(
                &world,
                "selectArrayObjectsWhere",
                vec!["world.myAvatar.tempitems".into(), need]
            ),
            "[]"
        );
    }
}
