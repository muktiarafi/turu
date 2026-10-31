//! What Cosmetics does to your avatar (VibeSkua `CosmeticsService`): every change is client side
//! (`objData.eqp`, `loadMovieAtES`, the colors), so other players and the server see your real
//! gear. These call into Flash: never from the UI thread.

use super::{CosmeticItem, CosmeticSet, Library, Slot};
use crate::api::{inventory, map, player, send};
use crate::error::BotResult;
use crate::events::BotEvent;
use crate::{Bot, FlashArg};
use parking_lot::Mutex;
use serde_json::{Map, Value, json};
use std::sync::Arc;

const EQP: &str = "world.myAvatar.objData.eqp";

/// Base, trim, accessory, eye, skin and hair colors (`intColor*`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Colors {
    pub base: i64,
    pub trim: i64,
    pub accessory: i64,
    pub eye: i64,
    pub skin: i64,
    pub hair: i64,
}

impl Colors {
    const FIELDS: [&'static str; 6] = [
        "intColorBase",
        "intColorTrim",
        "intColorAccessory",
        "intColorEye",
        "intColorSkin",
        "intColorHair",
    ];

    pub fn to_array(self) -> [i64; 6] {
        [
            self.base,
            self.trim,
            self.accessory,
            self.eye,
            self.skin,
            self.hair,
        ]
    }

    fn from_array(c: [i64; 6]) -> Self {
        Self {
            base: c[0],
            trim: c[1],
            accessory: c[2],
            eye: c[3],
            skin: c[4],
            hair: c[5],
        }
    }

    fn is_blank(self) -> bool {
        self.to_array().iter().all(|&c| c == 0)
    }

    /// Every color, as a change.
    pub fn all(self) -> ColorChange {
        ColorChange(self.to_array().map(Some))
    }
}

/// The colors to set; `None` leaves one as it is (in [`Colors`]' order).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ColorChange(pub [Option<i64>; 6]);

impl ColorChange {
    fn is_empty(self) -> bool {
        self.0.iter().all(Option::is_none)
    }
}

/// A bot's cosmetic state: the items it shows over its real gear (VibeSkua
/// `CurrentVisualSet`) and the colors and gender to go back to.
#[derive(Default)]
pub struct Look {
    inner: Mutex<LookState>,
}

#[derive(Default)]
struct LookState {
    current: Option<CosmeticSet>,
    original_colors: Option<Colors>,
    original_gender: Option<char>,
}

impl Look {
    /// What is shown over the real gear ("Active Look").
    pub fn current(&self) -> CosmeticSet {
        self.inner
            .lock()
            .current
            .clone()
            .unwrap_or_else(|| CosmeticSet::named("Active Look"))
    }

    fn set_slot(&self, slot: Slot, item: Option<CosmeticItem>) {
        let mut state = self.inner.lock();
        state
            .current
            .get_or_insert_with(|| CosmeticSet::named("Active Look"))
            .set(slot, item);
    }

    fn clear_current(&self) {
        if let Some(current) = &mut self.inner.lock().current {
            current.clear();
        }
    }

    pub fn original_colors(&self) -> Option<Colors> {
        self.inner.lock().original_colors
    }

    pub fn original_gender(&self) -> Option<char> {
        self.inner.lock().original_gender
    }

    /// Forgets everything (after a logout: the next login may be another character).
    pub fn reset(&self) {
        *self.inner.lock() = LookState::default();
    }
}

/// Turu: forgets the look at logout. VibeSkua's plugin keeps the first character's colors
/// and gender until the client closes, which a tab that logs into another character would
/// restore onto the new one.
pub(crate) fn start(bot: &Arc<Bot>) {
    let weak = Arc::downgrade(bot);
    bot.events.subscribe(move |event| {
        if let (BotEvent::Logout, Some(bot)) = (event, weak.upgrade()) {
            bot.cosmetics.reset();
        }
    });
}

fn logged_in(bot: &Bot) -> BotResult<bool> {
    player::logged_in(bot)
}

/// Remembers the real colors and gender before the first change (VibeSkua
/// `CaptureOriginalStateIfEmpty`).
pub fn capture_original_state(bot: &Bot) -> BotResult<()> {
    if !logged_in(bot)? {
        return Ok(());
    }
    if bot.cosmetics.original_colors().is_none() {
        let colors = real_colors(bot)?;
        if !colors.is_blank() {
            bot.cosmetics.inner.lock().original_colors = Some(colors);
        }
    }
    if bot.cosmetics.original_gender().is_none() {
        let gender = real_gender(bot)?;
        if gender == 'M' || gender == 'F' {
            bot.cosmetics.inner.lock().original_gender = Some(gender);
        }
    }
    Ok(())
}

/// `M` or `F` from the start of a gender string.
fn gender_of(text: &str) -> Option<char> {
    let first = text
        .trim_matches(['"', ' ', '\''])
        .chars()
        .next()?
        .to_ascii_uppercase();
    matches!(first, 'M' | 'F').then_some(first)
}

/// The gender the server sent (`dataLeaf`, which the plugin never changes), else the shown one.
pub fn real_gender(bot: &Bot) -> BotResult<char> {
    if !logged_in(bot)? {
        return Ok('M');
    }
    let leaf: Option<String> = bot.flash.get_opt("world.myAvatar.dataLeaf.strGender")?;
    match leaf.as_deref().and_then(gender_of) {
        Some(gender) => Ok(gender),
        None => current_gender(bot),
    }
}

/// The gender shown: skua.swf's `gender`, else `objData`, else `dataLeaf`; `M` by default.
pub fn current_gender(bot: &Bot) -> BotResult<char> {
    if !logged_in(bot)? {
        return Ok('M');
    }
    if let Some(gender) = gender_of(&bot.flash.call("gender", vec![])?) {
        return Ok(gender);
    }
    for path in [
        "world.myAvatar.objData.strGender",
        "world.myAvatar.dataLeaf.strGender",
    ] {
        let value: Option<String> = bot.flash.get_opt(path)?;
        if let Some(gender) = value.as_deref().and_then(gender_of) {
            return Ok(gender);
        }
    }
    Ok('M')
}

fn read_colors(bot: &Bot, root: &str) -> BotResult<Colors> {
    let mut colors = [0; 6];
    for (color, field) in colors.iter_mut().zip(Colors::FIELDS) {
        *color = bot.flash.get_or(&format!("{root}.{field}"), 0)?;
    }
    Ok(Colors::from_array(colors))
}

/// The colors the server sent (`dataLeaf`), else the shown ones.
pub fn real_colors(bot: &Bot) -> BotResult<Colors> {
    if !logged_in(bot)? {
        return Ok(Colors::default());
    }
    let colors = read_colors(bot, "world.myAvatar.dataLeaf")?;
    if colors.is_blank() {
        current_colors(bot)
    } else {
        Ok(colors)
    }
}

/// The colors shown (`objData`).
pub fn current_colors(bot: &Bot) -> BotResult<Colors> {
    if !logged_in(bot)? {
        return Ok(Colors::default());
    }
    read_colors(bot, "world.myAvatar.objData")
}

/// An `objData.eqp` entry.
fn eqp_entry(file: &str, link: &str, kind: &str, meta: &str) -> FlashArg {
    json!({ "sFile": file, "sLink": link, "sType": kind, "sMeta": meta }).into()
}

/// Shows `file`/`link` in the slot `group` (`eqp[group]` and `loadMovieAtES`).
fn load_at(
    bot: &Bot,
    group: &str,
    file: &str,
    link: &str,
    kind: &str,
    meta: &str,
) -> BotResult<()> {
    bot.flash
        .set_key(EQP, group, eqp_entry(file, link, kind, meta))?;
    bot.flash.call_game_function(
        "world.myAvatar.loadMovieAtES",
        vec![group.into(), file.into(), link.into()],
    )
}

/// Shows an item on your avatar (VibeSkua `Equip`).
pub fn equip(bot: &Bot, item: &CosmeticItem) -> BotResult<()> {
    if !logged_in(bot)? {
        bot.log("[Cosmetics] Cannot equip: You are not logged in.");
        return Ok(());
    }
    capture_original_state(bot)?;
    let kind = match item.kind() {
        "Armor" | "Class" => "Armor",
        _ => item.category.as_str(),
    };
    let group = if item.item_group == "ar" {
        "co"
    } else {
        item.item_group.as_str()
    };
    load_at(bot, group, &item.path, &item.link, kind, &item.meta)?;
    if let Some(slot) = Slot::from_group(group) {
        bot.cosmetics.set_slot(slot, Some(item.clone()));
    }
    bot.log(format!(
        "[Cosmetics] Equipping {}: {}",
        item.display_category(),
        item.name
    ));
    Ok(())
}

/// Shows an outfit: its gender, then its items, armor first (VibeSkua `EquipSetAsync`).
pub fn equip_set(bot: &Bot, set: &CosmeticSet) -> BotResult<()> {
    if !logged_in(bot)? {
        return Ok(());
    }
    capture_original_state(bot)?;
    if let Some(gender) = set.gender {
        swap_gender(bot, Some(gender), false)?;
        bot.sleep(100)?;
    }
    for slot in Slot::ALL {
        if let Some(item) = set.get(slot) {
            equip(bot, item)?;
            bot.sleep(140)?;
        }
    }
    Ok(())
}

/// Empties a slot; a pet is also taken off the map (VibeSkua `UnequipSlot`).
pub fn unequip_slot(bot: &Bot, group: &str) -> BotResult<()> {
    if !logged_in(bot)? {
        return Ok(());
    }
    let group = if group == "ar" { "co" } else { group };
    let f = &bot.flash;
    if group.eq_ignore_ascii_case("pe") {
        // VibeSkua tries each of these and ignores what fails.
        let _ = f.set("world.myAvatar.petMC.visible", false);
        let _ = f.set("world.myAvatar.petMC.x", -10000);
        let _ = f.set("world.myAvatar.petMC.y", -10000);
        let _ = f.call_game_function("world.myAvatar.unloadMovieAtES", vec!["pe".into()]);
        let _ = f.call_game_function("world.destroyPet", vec!["world.myAvatar".into()]);
        let _ = f.call_game_function("world.destroyPetMC", vec!["world.myAvatar".into()]);
        let _ = f.call_game_function("world.myAvatar.unloadPet", vec![]);
        let _ = f.set("world.myAvatar.objData.eqp.pe", FlashArg::Null);
        let _ = f.set("world.myAvatar.petMC", FlashArg::Null);
    } else {
        f.call_game_function("world.myAvatar.unloadMovieAtES", vec![group.into()])?;
        let _ = f.set_key(EQP, group, FlashArg::Null);
    }
    if let Some(slot) = Slot::from_group(group) {
        bot.cosmetics.set_slot(slot, None);
    }
    bot.log(format!("[Cosmetics] Unequipped slot: {group}"));
    Ok(())
}

/// The armor's file and link for `gender` (`/M/` <-> `/F/`, `m...` <-> `f...`). VibeSkua
/// keeps `mc...` links when making an armor female.
fn armor_for_gender(file: &str, link: &str, gender: char) -> (String, String) {
    let starts = |prefix: &str| {
        link.get(..prefix.len())
            .is_some_and(|s| s.eq_ignore_ascii_case(prefix))
    };
    if gender == 'M' {
        let file = file.replace("/F/", "/M/").replace("/f/", "/m/");
        let link = if starts("f") {
            format!("m{}", &link[1..])
        } else {
            link.to_owned()
        };
        (file, link)
    } else {
        let file = file.replace("/M/", "/F/").replace("/m/", "/f/");
        let link = if starts("m") && !starts("mc") {
            format!("f{}", &link[1..])
        } else {
            link.to_owned()
        };
        (file, link)
    }
}

/// Puts back the real gear, gender and colors (VibeSkua `RevertToRealGearAsync`).
pub fn revert_to_real_gear(bot: &Bot) -> BotResult<()> {
    if !logged_in(bot)? {
        return Ok(());
    }
    let equipped: Vec<_> = inventory::items(bot, inventory::Container::Inventory)?
        .into_iter()
        .filter(|item| item.equipped)
        .collect();
    let mut empty: Vec<&str> = Slot::ALL.iter().map(|slot| slot.group()).collect();
    bot.cosmetics.clear_current();

    let real_gender = match bot.cosmetics.original_gender() {
        Some(gender) => gender,
        None => real_gender(bot)?,
    };
    if current_gender(bot)? != real_gender {
        swap_gender(bot, Some(real_gender), true)?;
        bot.sleep(120)?;
    }

    match bot.cosmetics.original_colors() {
        Some(colors) => set_colors(bot, colors.all(), false)?,
        None => {
            let colors = real_colors(bot)?;
            if colors.base != 0 {
                set_colors(bot, colors.all(), false)?;
            }
        }
    }

    for item in &equipped {
        let group = if item.equip_slot == "ar" {
            "co"
        } else {
            item.equip_slot.as_str()
        };
        empty.retain(|slot| !slot.eq_ignore_ascii_case(group));
        let (file, link) = if group == "co" && !item.file_link.trim().is_empty() {
            armor_for_gender(&item.file_link, &item.file_name, real_gender)
        } else {
            (item.file_link.clone(), item.file_name.clone())
        };
        load_at(bot, group, &file, &link, &item.category, &item.meta)?;
        bot.sleep(100)?;
    }

    // Slots with no real item (the pet too) are emptied.
    for group in empty {
        unequip_slot(bot, group)?;
    }
    bot.log(
        "[Cosmetics] Reset avatar to actual equipped gear, restored original colors and gender.",
    );
    Ok(())
}

/// Copies the look of the player you target (VibeSkua `CopyTargetLookAsync`).
pub fn copy_target_look(bot: &Bot, library: &Library) -> Result<String, String> {
    let result = (|| {
        if !logged_in(bot)? {
            return Ok(Err("You are not logged in.".to_owned()));
        }
        let ent_id: i64 = bot
            .flash
            .get_or("world.myAvatar.target.dataLeaf.entID", 0)?;
        let name: String = bot
            .flash
            .get_or("world.myAvatar.target.dataLeaf.uoName", String::new())?;
        if ent_id <= 0 {
            return Ok(Err(
                "No player targeted. Click a player in-game first.".to_owned()
            ));
        }
        copy_player_look(bot, library, ent_id, &name)
    })();
    result.unwrap_or_else(|e| Err(format!("Failed to copy look: {e}")))
}

/// Copies the look of a player in your room (VibeSkua `CopyPlayerLookByNameAsync`).
pub fn copy_player_look_by_name(
    bot: &Bot,
    library: &Library,
    username: &str,
) -> Result<String, String> {
    let result = (|| {
        if !logged_in(bot)? {
            return Ok(Err("You are not logged in.".to_owned()));
        }
        if username.trim().is_empty() {
            return Ok(Err("Please select or enter a player name.".to_owned()));
        }
        let Some(player) = map::get_player(bot, username)? else {
            return Ok(Err(format!(
                "Player '{username}' was not found in the current map."
            )));
        };
        let ent_id = player["entId"].as_f64().unwrap_or(0.0) as i64;
        let name = player["name"].as_str().unwrap_or(username).to_owned();
        copy_player_look(bot, library, ent_id, &name)
    })();
    result.unwrap_or_else(|e| Err(format!("Failed to copy look: {e}")))
}

/// An integer field that may be a number or a string (C#'s `int.TryParse(token.ToString())`).
fn json_int(value: Option<&Value>) -> Option<i64> {
    match value? {
        Value::Number(n) => n.as_i64(),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

fn json_str<'a>(obj: &'a Map<String, Value>, key: &str) -> &'a str {
    obj.get(key).and_then(Value::as_str).unwrap_or("")
}

/// Has a file or a link (VibeSkua `HasValidArt`).
fn has_art(obj: &Map<String, Value>) -> bool {
    !json_str(obj, "sFile").trim().is_empty() || !json_str(obj, "sLink").trim().is_empty()
}

/// An `eqp` entry as an item, named from the catalog, else from its file (VibeSkua
/// `ParseCosmeticItemFromJObject`).
fn item_from_eqp(library: &Library, group: &str, obj: &Map<String, Value>) -> CosmeticItem {
    let file = json_str(obj, "sFile");
    let link = json_str(obj, "sLink");
    let mut name = library.name_of(file).unwrap_or_else(|| {
        std::path::Path::new(file)
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default()
    });
    if name.trim().is_empty() {
        name = link.to_owned();
    }
    CosmeticItem {
        id: json_int(obj.get("ItemID")).unwrap_or(0),
        name,
        path: file.to_owned(),
        link: link.to_owned(),
        item_group: group.to_owned(),
        category: json_str(obj, "sType").to_owned(),
        meta: json_str(obj, "sMeta").to_owned(),
    }
}

/// The entry of `eqp` under `key` when it has art.
fn art<'a>(eqp: &'a Map<String, Value>, key: &str) -> Option<&'a Map<String, Value>> {
    eqp.get(key)
        .and_then(Value::as_object)
        .filter(|o| has_art(o))
}

/// The outfit an `eqp` object shows: the armor (`co`, else the class `ar`), helm, cape,
/// weapon, pet and rune.
fn set_from_eqp(library: &Library, eqp: &Map<String, Value>, set: &mut CosmeticSet) {
    let armor = art(eqp, "co")
        .map(|o| ("co", o))
        .or_else(|| art(eqp, "ar").map(|o| ("ar", o)));
    if set.armor.is_none() {
        set.armor = armor.map(|(group, o)| item_from_eqp(library, group, o));
    }
    for slot in [Slot::Helm, Slot::Cape, Slot::Pet, Slot::Rune] {
        if set.get(slot).is_none() {
            let item = art(eqp, slot.group()).map(|o| item_from_eqp(library, slot.group(), o));
            set.set(slot, item);
        }
    }
    if set.weapon.is_none() {
        set.weapon = art(eqp, "Weapon")
            .or_else(|| art(eqp, "weapon"))
            .map(|o| item_from_eqp(library, "Weapon", o));
    }
}

/// The gender an armor's file or link suggests (`/F/`, `f...` but not `fl...`).
fn gender_of_armor(item: &CosmeticItem) -> Option<char> {
    let (path, link) = (item.path.to_lowercase(), item.link.to_lowercase());
    if path.contains("/f/") || (link.starts_with('f') && !link.starts_with("fl")) {
        Some('F')
    } else if path.contains("/m/") || link.starts_with('m') {
        Some('M')
    } else {
        None
    }
}

/// VibeSkua `CopyPlayerLookByEntIdAsync`: the player's gear, gender and colors from
/// skua.swf's `getAvatar`, and empties the slots they have nothing in.
///
/// VibeSkua also looks in `world.CHARS[id]` for the gender, skin and hair; its
/// `getGameObject` can't read a `[...]` path, so those always came back empty and are left out.
fn copy_player_look(
    bot: &Bot,
    library: &Library,
    ent_id: i64,
    name: &str,
) -> BotResult<Result<String, String>> {
    capture_original_state(bot)?;
    let raw = bot.flash.call("getAvatar", vec![ent_id.into()])?;
    let Some(avatar) = serde_json::from_str::<Value>(&raw)
        .ok()
        .filter(Value::is_object)
    else {
        return Ok(Err(format!("Could not retrieve avatar data for {name}.")));
    };
    let Some(eqp) = avatar.get("eqp").and_then(Value::as_object) else {
        return Ok(Err(format!("{name} has no equipment data.")));
    };
    let mut set = CosmeticSet::named(format!("{name}'s Look"));
    set_from_eqp(library, eqp, &mut set);

    let mut gender = [
        avatar.get("strGender"),
        avatar.pointer("/dataLeaf/strGender"),
        avatar.pointer("/objData/strGender"),
    ]
    .into_iter()
    .flatten()
    .find_map(|v| v.as_str())
    .and_then(gender_of);
    if gender.is_none() {
        let target: i64 = bot
            .flash
            .get_or("world.myAvatar.target.dataLeaf.entID", 0)?;
        if target == ent_id {
            for path in [
                "world.myAvatar.target.dataLeaf.strGender",
                "world.myAvatar.target.objData.strGender",
            ] {
                let value: Option<String> = bot.flash.get_opt(path)?;
                if let Some(found) = value.as_deref().and_then(gender_of) {
                    gender = Some(found);
                    break;
                }
            }
        }
    }
    if gender.is_none() {
        gender = set.armor.as_ref().and_then(gender_of_armor);
    }
    if let Some(gender) = gender {
        set.gender = Some(gender);
        swap_gender(bot, Some(gender), false)?;
        bot.sleep(100)?;
    }

    let colors = ColorChange(Colors::FIELDS.map(|field| json_int(avatar.get(field))));
    if !colors.is_empty() {
        set_colors(bot, colors, false)?;
    }

    for slot in Slot::ALL {
        match set.get(slot) {
            Some(item) => {
                equip(bot, item)?;
                bot.sleep(120)?;
            }
            None => unequip_slot(bot, slot.group())?,
        }
    }
    Ok(Ok(format!("Copied look from {name}!")))
}

/// The other players in your room (VibeSkua `GetRoomPlayerNames`).
pub fn room_player_names(bot: &Bot) -> BotResult<Vec<String>> {
    if !logged_in(bot)? {
        return Ok(Vec::new());
    }
    let me = player::username(bot)?;
    let names: Vec<String> = bot.flash.get_or("world.areaUsers", Vec::new())?;
    Ok(names
        .into_iter()
        .filter(|name| !name.eq_ignore_ascii_case(&me))
        .collect())
}

/// Sets the colors shown (`objData.intColor*`) and redraws (VibeSkua `SetColors`);
/// `reload_gear` shows the cosmetic armor, helm and cape again in the new colors.
pub fn set_colors(bot: &Bot, colors: ColorChange, reload_gear: bool) -> BotResult<()> {
    if !logged_in(bot)? {
        return Ok(());
    }
    capture_original_state(bot)?;
    for (value, field) in colors.0.into_iter().zip(Colors::FIELDS) {
        if let Some(value) = value {
            bot.flash
                .set(&format!("world.myAvatar.objData.{field}"), value)?;
        }
    }
    let _ = bot
        .flash
        .call_game_function("world.myAvatar.pMC.updateColor", vec![]);
    if reload_gear {
        let current = bot.cosmetics.current();
        for slot in [Slot::Armor, Slot::Helm, Slot::Cape] {
            if let Some(item) = current.get(slot) {
                equip(bot, item)?;
            }
        }
    }
    bot.log("[Cosmetics] Palette colors updated.");
    Ok(())
}

/// Shows the other gender, or `target` (VibeSkua `SwapGender`): `objData.strGender`, the
/// game's own `genderSwap` answer (sent to the client only) and the armor's other-gender art.
/// With a `target` you already show, nothing happens unless `force`.
pub fn swap_gender(bot: &Bot, target: Option<char>, force: bool) -> BotResult<()> {
    if !logged_in(bot)? {
        return Ok(());
    }
    capture_original_state(bot)?;
    let mut uid: i64 = bot.flash.get_or("world.myAvatar.uid", 0)?;
    if uid <= 0 {
        uid = bot.flash.get_or("world.myAvatar.dataLeaf.entID", 0)?;
    }
    let current = current_gender(bot)?;
    let next = target.unwrap_or(if current == 'M' { 'F' } else { 'M' });
    if !force && target.is_some() && current == next {
        return Ok(());
    }

    bot.flash
        .set("world.myAvatar.objData.strGender", next.to_string())?;
    let packet = json!({
        "t": "xt",
        "b": { "r": -1, "o": {
            "cmd": "genderSwap", "bitSuccess": 1, "gender": next.to_string(), "intCoins": 0,
            "uid": uid, "strHairFileName": "", "HairID": "", "strHairName": "",
        }},
    });
    send::client_packet(bot, &packet.to_string(), "json")?;

    // VibeSkua ignores a failure to redraw.
    let _ = (|| -> BotResult<()> {
        let mut file: String = bot
            .flash
            .get_or(&format!("{EQP}.co.sFile"), String::new())?;
        let mut link: String = bot
            .flash
            .get_or(&format!("{EQP}.co.sLink"), String::new())?;
        if file.trim().is_empty() {
            file = bot
                .flash
                .get_or(&format!("{EQP}.ar.sFile"), String::new())?;
            link = bot
                .flash
                .get_or(&format!("{EQP}.ar.sLink"), String::new())?;
        }
        if !file.trim().is_empty() && !link.trim().is_empty() {
            let (file, link) = armor_for_gender(&file, &link, next);
            bot.flash.set(&format!("{EQP}.co.sFile"), file.as_str())?;
            bot.flash.set(&format!("{EQP}.co.sLink"), link.as_str())?;
            bot.flash.call_game_function(
                "world.myAvatar.loadMovieAtES",
                vec!["co".into(), file.into(), link.into()],
            )
        } else {
            bot.flash
                .call_game_function("world.myAvatar.pMC.draw", vec![])
        }
    })();

    let name = if next == 'M' { "Male" } else { "Female" };
    bot.log(format!("[Cosmetics] Gender set to {name}"));
    Ok(())
}

/// The look shown now, as an outfit: the cosmetics first, then `objData.eqp`, then the real
/// equipped items (VibeSkua `CaptureCurrentLook`).
pub fn capture_current_look(bot: &Bot, library: &Library, name: &str) -> BotResult<CosmeticSet> {
    let mut set = bot.cosmetics.current();
    set.name = name.to_owned();
    set.created_at = chrono::Local::now().to_rfc3339();
    set.gender = Some(current_gender(bot)?);

    let eqp: Option<Map<String, Value>> = bot.flash.get_opt(EQP)?;
    if let Some(eqp) = eqp {
        set_from_eqp(library, &eqp, &mut set);
    }

    let equipped: Vec<_> = inventory::items(bot, inventory::Container::Inventory)?
        .into_iter()
        .filter(|item| item.equipped)
        .collect();
    for slot in Slot::ALL {
        if set.get(slot).is_none() {
            let found = equipped.iter().find(|item| {
                let group = item.equip_slot.as_str();
                match slot {
                    Slot::Armor => {
                        group.eq_ignore_ascii_case("co") || group.eq_ignore_ascii_case("ar")
                    }
                    _ => group.eq_ignore_ascii_case(slot.group()),
                }
            });
            set.set(slot, found.map(CosmeticItem::from_game));
        }
    }
    Ok(set)
}

/// Shows any SWF in a slot (VibeSkua's Manual SWF tab); `group` is an `eqp` key.
pub fn inject(bot: &Bot, group: &str, file: &str, link: &str, meta: &str) -> BotResult<()> {
    let category = match group {
        "he" => "Helm",
        "ba" => "Cape",
        "pe" => "Pet",
        "mi" => "Misc",
        "Weapon" => "Sword",
        _ => "Armor",
    };
    equip(
        bot,
        &CosmeticItem {
            id: 0,
            name: "Manual Injected Item".into(),
            path: file.to_owned(),
            link: link.to_owned(),
            item_group: group.to_owned(),
            category: category.into(),
            meta: meta.to_owned(),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn armor_art_follows_the_gender() {
        assert_eq!(
            armor_for_gender("classes/M/Warrior.swf", "mWarrior", 'F'),
            ("classes/F/Warrior.swf".into(), "fWarrior".into())
        );
        assert_eq!(
            armor_for_gender("classes/F/Warrior.swf", "fWarrior", 'M'),
            ("classes/M/Warrior.swf".into(), "mWarrior".into())
        );
        // VibeSkua leaves `mc...` links alone.
        assert_eq!(armor_for_gender("a/M/b.swf", "mcThing", 'F').1, "mcThing");
        assert_eq!(gender_of("\"female\""), Some('F'));
        assert_eq!(gender_of("x"), None);
    }
}
