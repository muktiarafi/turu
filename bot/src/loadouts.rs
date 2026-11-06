//! Tools & Helpers > Loadouts Manager (VibeSkua `LoadoutService`, `LoadoutProfile`): named gear
//! sets per account, saved in `loadouts/<username>.json` in VibeSkua's format (its
//! `Loadouts/<username>.json` copies in and out unchanged).
//!
//! Differences from VibeSkua, on purpose (docs/ARMY_EQUIPMENT_PROPOSAL.md, decision 1):
//!
//! * [`equip`] goes through [`gear::equip`], so it refuses while a script runs unless asked to
//!   stop it first (CoreBots puts its own gear back at its next `equipClass`), and a full bag
//!   fails the item rather than banking something else to make room.
//! * Equip never enhances. [`apply_enhancements`] is its own action: VibeSkua's
//!   `LoadoutEnhancerTemp` (CoreAdvanced's `enhanceItem` for each slot whose enhancement
//!   differs, then back to the map it started on), run as a TypeScript script.

use crate::Bot;
use crate::api::{NameOrId, inventory, map};
use crate::gear::{self, EquipReport, GearError, GearItem, Place, Slot};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

/// VibeSkua's name for the live entry ("Show current gear"), which can't be saved under.
pub const CURRENT: &str = "Currently Equipped";

/// The slots a loadout holds, in VibeSkua's order (no ground item).
pub const SLOTS: [Slot; 7] = [
    Slot::Class,
    Slot::Armor,
    Slot::Weapon,
    Slot::Helm,
    Slot::Cape,
    Slot::Pet,
    Slot::Necklace,
];

/// A slot's name in the Loadouts Manager (VibeSkua's: "Amulet" for the necklace).
pub fn slot_label(slot: Slot) -> &'static str {
    match slot {
        Slot::Necklace => "Amulet",
        other => other.label(),
    }
}

/// Whether a slot's enhancement is saved (VibeSkua: class, weapon, helm and cape).
pub fn enhanced(slot: Slot) -> bool {
    matches!(slot, Slot::Class | Slot::Weapon | Slot::Helm | Slot::Cape)
}

/// VibeSkua `LoadoutProfile`, field for field.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct Loadout {
    pub name: String,

    pub class: String,
    pub class_enhancement: String,
    #[serde(rename = "ClassPatternID")]
    pub class_pattern_id: i64,
    #[serde(rename = "ClassProcID")]
    pub class_proc_id: i64,

    pub armor: String,

    pub weapon: String,
    pub weapon_enhancement: String,
    #[serde(rename = "WeaponPatternID")]
    pub weapon_pattern_id: i64,
    #[serde(rename = "WeaponProcID")]
    pub weapon_proc_id: i64,

    pub helm: String,
    pub helm_enhancement: String,
    #[serde(rename = "HelmPatternID")]
    pub helm_pattern_id: i64,
    #[serde(rename = "HelmProcID")]
    pub helm_proc_id: i64,

    pub cape: String,
    pub cape_enhancement: String,
    #[serde(rename = "CapePatternID")]
    pub cape_pattern_id: i64,
    #[serde(rename = "CapeProcID")]
    pub cape_proc_id: i64,

    pub pet: String,
    pub amulet: String,
}

/// An item's enhancement: its pattern (Lucky, Forge, ...) and weapon proc.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Enhancement {
    pub pattern_id: i64,
    pub proc_id: i64,
}

impl Enhancement {
    /// VibeSkua `GetEnhancementName`.
    pub fn name(self) -> &'static str {
        enhancement_name(self.pattern_id, self.proc_id)
    }
}

impl Loadout {
    /// The item saved for `slot` ("" when none).
    pub fn item(&self, slot: Slot) -> &str {
        match slot {
            Slot::Class => &self.class,
            Slot::Armor => &self.armor,
            Slot::Weapon => &self.weapon,
            Slot::Helm => &self.helm,
            Slot::Cape => &self.cape,
            Slot::Pet => &self.pet,
            Slot::Necklace => &self.amulet,
            Slot::Ground => "",
        }
    }

    /// The enhancement's saved name, e.g. "Lucky" or "Arcana's Concerto".
    pub fn enhancement_name(&self, slot: Slot) -> &str {
        match slot {
            Slot::Class => &self.class_enhancement,
            Slot::Weapon => &self.weapon_enhancement,
            Slot::Helm => &self.helm_enhancement,
            Slot::Cape => &self.cape_enhancement,
            _ => "",
        }
    }

    /// The enhancement to have, as VibeSkua's `EquipLoadoutAsync` reads it: the saved IDs, or
    /// when there are none, parsed from the name (Lucky for an unknown one). `None` for a slot
    /// that keeps no enhancement or has no item.
    pub fn enhancement(&self, slot: Slot) -> Option<Enhancement> {
        if !enhanced(slot) || self.item(slot).trim().is_empty() {
            return None;
        }
        let (pattern, proc) = match slot {
            Slot::Class => (self.class_pattern_id, self.class_proc_id),
            Slot::Weapon => (self.weapon_pattern_id, self.weapon_proc_id),
            Slot::Helm => (self.helm_pattern_id, self.helm_proc_id),
            _ => (self.cape_pattern_id, self.cape_proc_id),
        };
        let name = self.enhancement_name(slot);
        Some(if pattern > 0 {
            Enhancement {
                pattern_id: pattern,
                proc_id: proc,
            }
        } else {
            Enhancement {
                pattern_id: parse_pattern_id(name),
                proc_id: parse_proc_id(name),
            }
        })
    }

    /// The items to put on, in VibeSkua's order, blank slots left out.
    pub fn items(&self) -> Vec<&str> {
        SLOTS
            .iter()
            .map(|&s| self.item(s).trim())
            .filter(|i| !i.is_empty())
            .collect()
    }

    /// VibeSkua `CreateFromCurrentEquipped`: what `items` has worn (`&amp;` spelled `&`).
    pub fn from_worn(name: &str, items: &[GearItem]) -> Self {
        let mut loadout = Self {
            name: name.to_owned(),
            ..Default::default()
        };
        for item in items
            .iter()
            .filter(|i| i.equipped && i.place == Place::Inventory)
        {
            let clean = item.name.replace("&amp;", "&");
            let enh = Enhancement {
                pattern_id: item.enhancement_pattern_id,
                proc_id: item.proc_id,
            };
            let (slot_item, fields) = match item.slot {
                Slot::Class => (
                    &mut loadout.class,
                    Some((
                        &mut loadout.class_enhancement,
                        &mut loadout.class_pattern_id,
                        &mut loadout.class_proc_id,
                    )),
                ),
                Slot::Weapon => (
                    &mut loadout.weapon,
                    Some((
                        &mut loadout.weapon_enhancement,
                        &mut loadout.weapon_pattern_id,
                        &mut loadout.weapon_proc_id,
                    )),
                ),
                Slot::Helm => (
                    &mut loadout.helm,
                    Some((
                        &mut loadout.helm_enhancement,
                        &mut loadout.helm_pattern_id,
                        &mut loadout.helm_proc_id,
                    )),
                ),
                Slot::Cape => (
                    &mut loadout.cape,
                    Some((
                        &mut loadout.cape_enhancement,
                        &mut loadout.cape_pattern_id,
                        &mut loadout.cape_proc_id,
                    )),
                ),
                Slot::Armor => (&mut loadout.armor, None),
                Slot::Pet => (&mut loadout.pet, None),
                Slot::Necklace => (&mut loadout.amulet, None),
                Slot::Ground => continue,
            };
            *slot_item = clean;
            if let Some((name, pattern, proc)) = fields {
                *name = enh.name().to_owned();
                *pattern = enh.pattern_id;
                *proc = enh.proc_id;
            }
        }
        loadout
    }

    /// The same items and enhancements under another name.
    pub fn renamed(&self, name: &str) -> Self {
        Self {
            name: name.to_owned(),
            ..self.clone()
        }
    }
}

/// VibeSkua `GetEnhancementName`: the proc's name for a weapon special, else the pattern's.
pub fn enhancement_name(pattern_id: i64, proc_id: i64) -> &'static str {
    if proc_id > 0 {
        return match proc_id {
            2 => "Spiral Carve",
            3 => "Awe Blast",
            4 => "Health Vamp",
            5 => "Mana Vamp",
            6 => "Powerword DIE",
            7 => "Lacerate",
            8 => "Smite",
            9 => "Valiance",
            10 => "Arcana's Concerto",
            11 => "Acheron",
            12 => "Elysium",
            13 => "Praxis",
            14 => "Dauntless",
            15 => "Ravenous",
            _ => "Unknown",
        };
    }
    match pattern_id {
        1 => "Adventurer",
        2 => "Fighter",
        3 => "Thief",
        4 => "Armsman",
        5 => "Hybrid",
        6 => "Wizard",
        7 => "Healer",
        8 => "Spellbreaker",
        9 => "Lucky",
        10 => "Forge",
        11 => "Absolution",
        12 => "Avarice",
        23 => "Depths",
        24 => "Vainglory",
        25 => "Vim",
        26 => "Examen",
        27 => "Pneuma",
        28 => "Anima",
        29 => "Penitence",
        30 => "Lament",
        32 => "Hearty",
        _ => "Unknown",
    }
}

/// VibeSkua `ParsePatternID` (Lucky when it doesn't know the name).
fn parse_pattern_id(name: &str) -> i64 {
    match name.trim() {
        "Adventurer" => 1,
        "Fighter" => 2,
        "Thief" => 3,
        "Armsman" => 4,
        "Hybrid" => 5,
        "Wizard" => 6,
        "Healer" => 7,
        "Spellbreaker" => 8,
        "Forge" => 10,
        "Absolution" => 11,
        "Avarice" => 12,
        "Depths" => 23,
        "Vainglory" => 24,
        "Vim" => 25,
        "Examen" => 26,
        "Pneuma" => 27,
        "Anima" => 28,
        "Penitence" => 29,
        "Lament" => 30,
        "Hearty" => 32,
        _ => 9,
    }
}

/// VibeSkua `ParseProcID`.
fn parse_proc_id(name: &str) -> i64 {
    match name.trim() {
        "Spiral Carve" => 2,
        "Awe Blast" => 3,
        "Health Vamp" => 4,
        "Mana Vamp" => 5,
        "Powerword DIE" => 6,
        "Lacerate" => 7,
        "Smite" => 8,
        "Valiance" => 9,
        "Arcana's Concerto" => 10,
        "Acheron" => 11,
        "Elysium" => 12,
        "Praxis" => 13,
        "Dauntless" => 14,
        "Ravenous" => 15,
        _ => 0,
    }
}

/// The account's file: VibeSkua's `<username>.json`, with characters a file name can't hold
/// replaced.
pub fn path_for(dir: &Path, username: &str) -> PathBuf {
    let name: String = username
        .trim()
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') {
                '_'
            } else {
                c
            }
        })
        .collect();
    dir.join(format!("{name}.json"))
}

/// The account's loadouts; none when it has no file yet.
pub fn load(path: &Path) -> io::Result<Vec<Loadout>> {
    match fs::read_to_string(path) {
        Ok(text) if text.trim().is_empty() => Ok(Vec::new()),
        Ok(text) => serde_json::from_str(&text).map_err(io::Error::other),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(e),
    }
}

/// Writes the list (indented, as VibeSkua does) through a temp file, so a reader never sees half.
pub fn save(path: &Path, loadouts: &[Loadout]) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let text = serde_json::to_vec_pretty(loadouts).map_err(io::Error::other)?;
    let temp = path.with_extension(format!("tmp{}", std::process::id()));
    fs::write(&temp, &text)?;
    if fs::rename(&temp, path).is_err() {
        let _ = fs::remove_file(&temp);
        fs::write(path, text)?;
    }
    Ok(())
}

/// VibeSkua `EquipLoadoutAsync`'s equipping, through [`gear::equip`] (bank fetch included):
/// refused while a script runs unless `stop`.
pub fn equip(bot: &Bot, loadout: &Loadout, stop: bool) -> Result<EquipReport, GearError> {
    bot.log(format!("[Loadouts] Equipping Loadout: {}", loadout.name));
    let items: Vec<NameOrId> = loadout.items().into_iter().map(NameOrId::Name).collect();
    let report = gear::equip(bot, &items, stop)?;
    bot.log(format!("[Loadouts] {}: {}", loadout.name, report.summary()));
    Ok(report)
}

/// A worn item of the loadout whose enhancement isn't the saved one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnhancementChange {
    pub slot: Slot,
    pub item: String,
    pub want: Enhancement,
    pub worn: Enhancement,
}

/// VibeSkua's check before `LoadoutEnhancerTemp`: the loadout's items that are worn, but with
/// another enhancement than saved. Items not worn aren't looked at (equip them first).
pub fn enhancement_changes(loadout: &Loadout, items: &[GearItem]) -> Vec<EnhancementChange> {
    SLOTS
        .iter()
        .filter_map(|&slot| {
            let want = loadout.enhancement(slot)?;
            let name = loadout.item(slot).replace("&amp;", "&");
            let worn = items.iter().find(|i| {
                i.equipped
                    && i.place == Place::Inventory
                    && i.name
                        .replace("&amp;", "&")
                        .trim()
                        .eq_ignore_ascii_case(name.trim())
            })?;
            let worn = Enhancement {
                pattern_id: worn.enhancement_pattern_id,
                proc_id: worn.proc_id,
            };
            (worn != want).then(|| EnhancementChange {
                slot,
                item: name.trim().to_owned(),
                want,
                worn,
            })
        })
        .collect()
}

/// Where the enhancer goes back to once done (VibeSkua's `returnJoin`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReturnTo {
    House,
    Home,
    /// The map's full name, room included.
    Map(String),
}

/// VibeSkua's `adv.EnhanceItem(...)` arguments for a change: the base type (at most Lucky), and
/// the cape, helm and weapon specials of CoreAdvanced's enums.
fn enhance_args(change: &EnhancementChange) -> (i64, i64, i64, i64) {
    let (pattern, proc) = (change.want.pattern_id, change.want.proc_id);
    let base = pattern.min(9);
    match change.slot {
        Slot::Class => (pattern, 0, 0, 0),
        Slot::Weapon => {
            // VibeSkua `MapWeaponSpecial`; CoreAdvanced's WeaponSpecial is numbered by proc.
            let special = if pattern == 10 && proc == 0 {
                1
            } else if (2..=15).contains(&proc) {
                proc
            } else {
                0
            };
            (base, 0, 0, special)
        }
        Slot::Helm => {
            let special = if matches!(pattern, 10 | 25 | 26 | 27 | 28 | 32) {
                pattern
            } else {
                0
            };
            (base, 0, special, 0)
        }
        _ => {
            let special = if matches!(pattern, 10 | 11 | 12 | 24 | 29 | 30) {
                pattern
            } else {
                0
            };
            (base, special, 0, 0)
        }
    }
}

/// A JS string literal.
fn js_string(text: &str) -> String {
    serde_json::to_string(text).unwrap_or_else(|_| "\"\"".into())
}

/// VibeSkua's `LoadoutEnhancerTemp.cs`, in TypeScript: CoreAdvanced's `enhanceItem` for each
/// change, then back to where it started. `core` is the scripts folder's `core/`.
pub fn enhancer_script(changes: &[EnhancementChange], core: &Path, back: &ReturnTo) -> String {
    let import = |file: &str| js_string(&core.join(file).to_string_lossy().replace('\\', "/"));
    let mut body = String::new();
    for change in changes {
        let (base, cape, helm, weapon) = enhance_args(change);
        body.push_str(&format!(
            "    adv.enhanceItem({}, {base}, {{ cSpecial: {cape}, hSpecial: {helm}, wSpecial: {weapon} }});\n",
            js_string(&change.item)
        ));
    }
    let back = match back {
        ReturnTo::House => "    bot.send.packet(`%xt%zm%house%1%${bot.player.username}%`);\n    \
                            bot.wait.forMapLoad(\"house\");\n"
            .to_owned(),
        ReturnTo::Home => "    Core.join(\"home\");\n".to_owned(),
        ReturnTo::Map(map) => format!("    Core.join({});\n", js_string(map)),
    };
    format!(
        "// Written by the Loadouts Manager (VibeSkua's LoadoutEnhancerTemp); replaced each time.\n\
         import {{ bot }} from \"turu\";\n\
         import {{ CoreAdvanced }} from {};\n\
         import {{ Core }} from {};\n\
         \n\
         export default function main(): void {{\n\
         \x20   const adv = new CoreAdvanced();\n\
         {body}{back}}}\n",
        import("CoreAdvanced.ts"),
        import("CoreBots.ts"),
    )
}

/// What [`apply_enhancements`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Enhanced {
    /// Every worn item of the loadout already has its saved enhancement.
    AlreadyDone,
    /// The enhancer ran for this many items; it stopped on its own (`true`) or was stopped.
    Ran { items: usize, finished: bool },
}

/// VibeSkua's enhancement step of `EquipLoadoutAsync`, on its own: refused while a script runs.
/// Runs the enhancer as a script (the Script Loader shows it, Stop stops it) and waits for it,
/// then loads the script that was loaded before again. Call it from a bot thread.
pub fn apply_enhancements(bot: &Arc<Bot>, loadout: &Loadout) -> Result<Enhanced, String> {
    if bot.scripts.is_running() || bot.scripts.is_snippet_running() {
        return Err("Stop the script first".into());
    }
    gear::prepare(bot, false).map_err(|e| e.to_string())?;
    let items: Vec<GearItem> = inventory::inventory_items(bot)
        .map_err(|e| e.to_string())?
        .iter()
        .filter_map(|i| GearItem::from_item(i, Place::Inventory))
        .collect();
    let changes = enhancement_changes(loadout, &items);
    if changes.is_empty() {
        bot.log(format!(
            "[Loadouts] All items are already properly enhanced for {}.",
            loadout.name
        ));
        return Ok(Enhanced::AlreadyDone);
    }
    let core = bot.paths.scripts_dir().join("core");
    if !core.join("CoreAdvanced.ts").is_file() {
        return Err(format!(
            "CoreAdvanced isn't in {}: get the scripts first (Search Scripts)",
            core.display()
        ));
    }
    let name = map::name(bot).map_err(|e| e.to_string())?.to_lowercase();
    let back = match name.as_str() {
        "house" => ReturnTo::House,
        "" | "home" => ReturnTo::Home,
        _ => ReturnTo::Map(map::full_name(bot).map_err(|e| e.to_string())?),
    };
    let path = bot.paths.loadouts_dir().join("LoadoutEnhancerTemp.ts");
    fs::create_dir_all(bot.paths.loadouts_dir())
        .and_then(|()| fs::write(&path, enhancer_script(&changes, &core, &back)))
        .map_err(|e| format!("couldn't write {}: {e}", path.display()))?;

    let before = bot.scripts.loaded();
    bot.scripts.load(&path);
    if let Err(e) = crate::script::start_silent(bot) {
        restore(bot, before);
        return Err(e);
    }
    while bot.scripts.is_running() {
        std::thread::sleep(Duration::from_millis(500));
    }
    let finished = !bot.control.should_exit();
    restore(bot, before);
    bot.gear.mark_dirty();
    bot.log(format!(
        "[Loadouts] Enhancements {} for {}",
        if finished { "applied" } else { "stopped" },
        loadout.name
    ));
    Ok(Enhanced::Ran {
        items: changes.len(),
        finished,
    })
}

/// Loads the script that was loaded before the enhancer again.
fn restore(bot: &Bot, before: Option<PathBuf>) {
    if let Some(path) = before {
        bot.scripts.load(&path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn worn(name: &str, slot: Slot, pattern: i64, proc: i64) -> GearItem {
        GearItem {
            id: 1,
            name: name.into(),
            category: String::new(),
            slot,
            equipped: true,
            place: Place::Inventory,
            enhancement_level: 100,
            enhancement_pattern_id: pattern,
            proc_id: proc,
        }
    }

    /// A file VibeSkua wrote reads unchanged, and writes back with its names.
    #[test]
    fn reads_and_writes_vibeskua_files() {
        let text = r#"[{"Name":"Farm","Class":"Legion Revenant","ClassEnhancement":"Wizard",
            "ClassPatternID":6,"ClassProcID":0,"Armor":"","Weapon":"Necrotic Sword of Doom",
            "WeaponEnhancement":"Valiance","WeaponPatternID":6,"WeaponProcID":9,"Helm":"",
            "HelmEnhancement":"Luck","HelmPatternID":0,"HelmProcID":0,"Cape":"","CapeEnhancement":"Luck",
            "CapePatternID":0,"CapeProcID":0,"Pet":"","Amulet":""}]"#;
        let list: Vec<Loadout> = serde_json::from_str(text).unwrap();
        assert_eq!(list[0].class, "Legion Revenant");
        assert_eq!(list[0].weapon_proc_id, 9);
        let json = serde_json::to_value(&list[0]).unwrap();
        assert_eq!(json["ClassPatternID"], 6);
        assert_eq!(json["WeaponEnhancement"], "Valiance");
        assert_eq!(
            list[0].items(),
            ["Legion Revenant", "Necrotic Sword of Doom"]
        );
        // No helm, so no enhancement to check there.
        assert_eq!(list[0].enhancement(Slot::Helm), None);

        let dir = std::env::temp_dir().join(format!("turu-loadouts-{}", std::process::id()));
        let path = path_for(&dir, "My Alt");
        assert_eq!(path.file_name().unwrap(), "My Alt.json");
        assert_eq!(load(&path).unwrap(), Vec::new());
        save(&path, &list).unwrap();
        assert_eq!(load(&path).unwrap(), list);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn saves_what_is_worn() {
        let items = [
            worn("Legion Revenant", Slot::Class, 6, 0),
            worn("Blade &amp; Shield", Slot::Weapon, 6, 10),
            worn("Hood", Slot::Helm, 26, 0),
            worn("Bag", Slot::Pet, 0, 0),
            GearItem {
                equipped: false,
                ..worn("Unworn Cape", Slot::Cape, 9, 0)
            },
        ];
        let loadout = Loadout::from_worn("Boss", &items);
        assert_eq!(loadout.name, "Boss");
        assert_eq!(loadout.weapon, "Blade & Shield");
        assert_eq!(loadout.weapon_enhancement, "Arcana's Concerto");
        assert_eq!(loadout.helm_enhancement, "Examen");
        assert_eq!(loadout.class_enhancement, "Wizard");
        assert_eq!(loadout.pet, "Bag");
        assert_eq!(loadout.cape, "");
        assert!(enhancement_changes(&loadout, &items).is_empty());
    }

    #[test]
    fn finds_enhancements_that_differ() {
        let loadout = Loadout {
            name: "Farm".into(),
            class: "Legion Revenant".into(),
            class_enhancement: "Lucky".into(),
            weapon: "Sword".into(),
            // No IDs saved: read from the name, as VibeSkua does.
            weapon_enhancement: "Valiance".into(),
            ..Default::default()
        };
        let items = [
            worn("legion revenant", Slot::Class, 9, 0),
            worn("Sword", Slot::Weapon, 9, 0),
        ];
        let changes = enhancement_changes(&loadout, &items);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].slot, Slot::Weapon);
        assert_eq!(
            changes[0].want,
            Enhancement {
                pattern_id: 9,
                proc_id: 9
            }
        );
        assert_eq!(enhance_args(&changes[0]), (9, 0, 0, 9));
    }

    #[test]
    fn writes_the_enhancer() {
        let change = |slot, pattern, proc| EnhancementChange {
            slot,
            item: "It \"Quoted\"".into(),
            want: Enhancement {
                pattern_id: pattern,
                proc_id: proc,
            },
            worn: Enhancement {
                pattern_id: 9,
                proc_id: 0,
            },
        };
        assert_eq!(enhance_args(&change(Slot::Weapon, 10, 0)), (9, 0, 0, 1));
        assert_eq!(enhance_args(&change(Slot::Helm, 26, 0)), (9, 0, 26, 0));
        assert_eq!(enhance_args(&change(Slot::Cape, 2, 0)), (2, 0, 0, 0));
        let script = enhancer_script(
            &[change(Slot::Cape, 11, 0)],
            Path::new("/data/Scripts/core"),
            &ReturnTo::Map("battleon-1234".into()),
        );
        assert!(script.contains("from \"/data/Scripts/core/CoreAdvanced.ts\""));
        assert!(script.contains(
            "adv.enhanceItem(\"It \\\"Quoted\\\"\", 9, { cSpecial: 11, hSpecial: 0, wSpecial: 0 });"
        ));
        assert!(script.contains("Core.join(\"battleon-1234\");"));
        assert!(
            enhancer_script(&[], Path::new("c"), &ReturnTo::House)
                .contains("forMapLoad(\"house\")")
        );
    }
}
