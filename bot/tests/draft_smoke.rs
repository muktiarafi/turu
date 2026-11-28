//! The draft smoke run (docs/REPLAY_TESTS_PROPOSAL.md, phase 4): each converted script that
//! wasn't read through by hand ("Converted by tools/cs2ts, not yet read through by hand") and has
//! a `main` (Core files are libraries) runs for
//! a few seconds against a [`WorldFake`] seeded from a tape. A draft fails when it throws; it
//! passes when it ends cleanly or is still running at its budget.
//!
//! Ignored: run by hand or nightly.
//!
//! ```text
//! cargo nextest run --cargo-profile draft -p turu_bot draft_smoke --run-ignored only --no-capture
//! ```
//!
//! `DRAFT_EVERY` (32) runs every Nth draft of the sorted list, `DRAFT_FILTER` only paths with that
//! text (a comma-separated list: any of them; it runs all of those, not every Nth), `DRAFT_SECS`
//! (60) each one's budget on its virtual clock (decision 2: sleeps and waits take no real time),
//! `DRAFT_REAL_SECS` (10) the real time it may take at most, `DRAFT_JOBS` (6) how many at once,
//! `DRAFT_TAPE`
//! (`kill_farm`) the tape the world comes from. The report goes to `target/draft-smoke.txt`; the
//! test fails when a draft threw or couldn't start. `.github/workflows/drafts-nightly.yml` runs
//! every draft each night.
//!
//! Drafts run with their options' defaults, except those [`DRAFT_OPTIONS`] sets: values a draft
//! can't start without (the Lone Wolf ultras stop at once with no private room number and no
//! roster that holds the account).

use parking_lot::Mutex;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use turu_bot::script::{self, ScriptStatus};
use turu_bot::testing::{ShopStock, WorldFake, item_id_for};
use turu_bot::{Bot, tape};

const MARK: &str = "not yet read through by hand";

/// Drafts that fail because of a gap in the fake world, not a fault of their own, and why:
/// reported as "known", not failing the run. A listed draft that passes fails the run, so the
/// list can't go stale. A script fix goes in `muktiarafi/turu-scripts` instead.
const KNOWN_FAKE_GAPS: &[(&str, &str)] = &[];

/// Option values set before a draft starts, where its defaults would stop it at once: the drafts
/// whose path holds one of the texts, the option's name, its value as JSON, and why. Each is set
/// wherever a matching draft declares that name, in its options or in a group (`0AllUltras_LW`
/// keeps its own under "Setup"), and saved to its options file (in the draft's own data folder); a
/// draft that doesn't declare it skips it. An entry that none of the matching drafts run declares
/// fails the run, so the list can't go stale.
const DRAFT_OPTIONS: &[(&[&str], &str, &str, &str)] = &[
    (
        LONE_WOLF,
        "PrivateRoomNumber",
        "1001",
        "CoreLoneWolf stops the bot unless it is 1001 to 99999; the default is 0",
    ),
    (ROSTER, "player1", r#""Questwalker""#, ROSTER_WHY),
    (ROSTER, "player2", r#""Wanderer2""#, ROSTER_WHY),
    (ROSTER, "player3", r#""Wanderer3""#, ROSTER_WHY),
    (ROSTER, "player4", r#""Wanderer4""#, ROSTER_WHY),
    (ROSTER, "player5", r#""Wanderer5""#, ROSTER_WHY),
    (ROSTER, "player6", r#""Wanderer6""#, ROSTER_WHY),
    (ROSTER, "player7", r#""Wanderer7""#, ROSTER_WHY),
];

/// The Lone Wolf ultras (`CoreLoneWolf`).
const LONE_WOLF: &[&str] = &["ultraslw/"];

/// The drafts that stop without an Army roster (`player1`, ... up to 4, 5, 6 or 7 of them).
const ROSTER: &[&str] = &[
    "ultraslw/",
    "army/dailies/TheEncroachingShadows.ts",
    "army/dailies/WrongTurnatVoidbuquerque.ts",
];

const ROSTER_WHY: &str = "CoreLoneWolf stops the bot unless every name is given, none twice, the \
                          account's among them (the fake's player is \"Questwalker\"); \
                          CoreArmyLite's scripts stop with fewer than 4";

/// The roster's other players ([`DRAFT_OPTIONS`]' `player2` to `player7`), whom [`Partners`] plays.
const PARTNERS: [&str; 6] = [
    "Wanderer2",
    "Wanderer3",
    "Wanderer4",
    "Wanderer5",
    "Wanderer6",
    "Wanderer7",
];

/// The rest of a draft's army: a draft runs alone, as the roster's first player, the one who
/// leads. Without it, an army draft waits for the others until its budget.
///
/// - `CoreLoneWolf`'s sync log: when the draft appends its own `READY` (after its `RESET`) or
///   `ARRIVED` (after its `STEP`), each partner appends the same line with its own name, so the
///   draft writes `START` and `CONTINUE` and goes on. The lines are the draft's own, so they keep
///   to its protocol version, session and step. Names off the draft's roster are ignored by it.
/// - A barrier (`bot.army.barrier`, CoreArmySync's `WaitForArmy`): the others are there.
///
/// - The draft's [`SHARED_SIGNALS`]: when it sends one (`SIGNAL`), each partner sends it too.
///
/// Not played: the other signals one player sends another in a fight (`sendArmySignal`,
/// `hasArmySignal`), which are one player's own (player 4's Mystify, an owner's clear, a
/// failure): a partner copying one would claim what no partner did.
struct Partners {
    /// The names of the draft's shared signals, as patterns.
    shared: Vec<regex::Regex>,
}

impl Partners {
    /// The partners of the draft `script` (its path under `scripts/`).
    fn of(script: &str) -> Self {
        let shared = SHARED_SIGNALS
            .iter()
            .filter(|(s, _, _)| *s == script)
            .flat_map(|(_, signals, _)| signals.iter().map(|s| signal_pattern(s)))
            .collect();
        Self { shared }
    }
}

/// A signal's name as a script writes it (``CYCLE_${farmCycle}_COMPLETE``), as a pattern for the
/// names it sends (`CYCLE_1_COMPLETE`): each `${...}` any record name's characters.
fn signal_pattern(template: &str) -> regex::Regex {
    let mut pattern = String::from("^");
    let mut rest = template;
    while let Some(at) = rest.find("${") {
        pattern.push_str(&regex::escape(&rest[..at]));
        pattern.push_str("[A-Z0-9_]+");
        rest = rest[at..].find('}').map_or("", |end| &rest[at + end + 1..]);
    }
    pattern.push_str(&regex::escape(rest));
    pattern.push('$');
    regex::Regex::new(&pattern).unwrap()
}

/// Signals every player of a draft's army sends for its own part, which the draft waits to have
/// from all of them (`allPlayersSignaled`): the draft's path, the signals as its source writes
/// them, and why. [`Partners`] send each when the draft does, as if they got there with it. A
/// signal the source doesn't write any more fails the run, so the list can't go stale.
const SHARED_SIGNALS: &[(&str, &[&str], &str)] = &[(
    "ultraslw/extras/ArmyPrismatasGoldFarm_LW.ts",
    &[
        "CYCLE_${farmCycle}_BINDING_READY",
        "CYCLE_${farmCycle}_CURRENCY_COMPLETE",
        "CYCLE_${farmCycle}_COMPLETE",
    ],
    "each player sends them when its own Elemental Binding, gold voucher and goal are done, and \
     the cycle goes on when all have",
)];

impl turu_bot::testing::ArmyStandIn for Partners {
    fn answer(&self, _name: &str, line: &str) -> Vec<String> {
        let parts: Vec<&str> = line.split('|').collect();
        let name_at = match (parts.first(), parts.len()) {
            (Some(&"READY"), 5) => 3,
            (Some(&"ARRIVED"), 5) => 4,
            // `SIGNAL|<version>|<session>|<signal>|<sender>`.
            (Some(&"SIGNAL"), 5) if self.shared.iter().any(|p| p.is_match(parts[3])) => 4,
            _ => return Vec::new(),
        };
        PARTNERS
            .iter()
            .map(|partner| {
                let mut theirs = parts.clone();
                theirs[name_at] = partner;
                theirs.join("|")
            })
            .collect()
    }

    fn at_barrier(&self, _name: &str, _count: usize) -> bool {
        true
    }
}

/// Sets the loaded draft's [`DRAFT_OPTIONS`]: the indexes of the entries it declares, or why one
/// was refused.
fn set_draft_options(bot: &Bot, script: &str) -> Result<Vec<usize>, String> {
    let script::ScriptOptionsState::Loaded(options) = bot.scripts.options() else {
        return Ok(Vec::new());
    };
    let categories: Vec<String> = options
        .declaration()
        .categories()
        .map(|(category, _)| category.to_owned())
        .collect();
    let mut set = Vec::new();
    for (i, (drafts, name, value, _)) in DRAFT_OPTIONS.iter().enumerate() {
        if !drafts.iter().any(|d| script.contains(d)) {
            continue;
        }
        let value: serde_json::Value = serde_json::from_str(value).expect("DRAFT_OPTIONS' JSON");
        for category in categories
            .iter()
            .filter(|c| bot.scripts.has_option(c, name))
        {
            bot.scripts
                .set_option_value(category, name, value.clone())
                .map_err(|e| format!("DRAFT_OPTIONS' {name}: {e}"))?;
            set.push(i);
        }
    }
    Ok(set)
}

fn env_or<T: std::str::FromStr>(name: &str, default: T) -> T {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .canonicalize()
        .unwrap()
}

/// Every draft under `scripts/`, sorted.
fn drafts() -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if !path.ends_with("node_modules") {
                    walk(&path, out);
                }
            } else if path.extension().is_some_and(|e| e == "ts")
                // A draft with a `main` to run: Core files (`CoreFrostvale.ts`) are libraries,
                // run through the scripts that import them.
                && std::fs::read_to_string(&path)
                    .is_ok_and(|t| t.contains(MARK) && t.contains("export default function main"))
            {
                out.push(path);
            }
        }
    }
    let mut out = Vec::new();
    walk(&repo().join("scripts"), &mut out);
    out.sort();
    out
}

/// Monsters scripts find in a map without naming them, which [`monster_catalog`] can't read from
/// the source: the map, the monsters' names (as the game spells them), and where they come from.
const MONSTER_EXTRAS: &[(&str, &[&str], &str)] = &[
    ("towerofdoom2", &["Dread Arradia"], TOWER_OF_DOOM),
    ("towerofdoom3", &["Dread Moganth"], TOWER_OF_DOOM),
    ("towerofdoom7", &["Dread Warrior"], TOWER_OF_DOOM),
    ("towerofdoom8", &["Dread Avatar"], TOWER_OF_DOOM),
];
const TOWER_OF_DOOM: &str = "story/TowerOfDoom.ts joins `towerofdoom${Floor}` and fights the \
    monster in cell r10, the floor's boss (each floor's page on aqwwiki.wikidot.com); other \
    scripts name the other floors' monsters";

/// Each map's monsters, by name, from the scripts' own calls: `huntMonster("map", "Monster"`,
/// `killMonster("map", "cell", "pad", "Monster"`, `killQuest(id, "map", "Monster"` or a list.
/// Monster names never reach the game's calls (a kill goes by ID), so the fake world can't learn
/// them from a tape (docs/REPLAY_TESTS_PROPOSAL.md, phase 4).
///
/// Then each map is made as long as the highest map ID a script hunts or fights there by
/// (the fake numbers a map's monsters from 1, in this order): `huntMonsterMapID("map", 2, ...`,
/// and a class's `mapName = "map"` beside its `...MapId = 2` constants (the Lone Wolf ultras). A
/// monster only an ID names gets a stand-in name, which those scripts never read. Also the files
/// with map IDs and no map, as gap lines.
fn monster_catalog() -> (std::collections::HashMap<String, Vec<String>>, Vec<String>) {
    let calls = regex::Regex::new(
        r#"(?:huntMonster\(\s*"([^"]+)",\s*"([^"]+)"|killMonster\(\s*"([^"]+)",\s*"[^"]*",\s*"[^"]*",\s*"([^"]+)"|killQuest\(\s*\d+,\s*"([^"]+)",\s*(\[[^\]]*\]|"[^"]+"))"#,
    )
    .unwrap();
    let quoted = regex::Regex::new(r#""([^"]+)""#).unwrap();
    let mut catalog: std::collections::HashMap<String, Vec<String>> = Default::default();
    for file in all_scripts() {
        let Ok(text) = std::fs::read_to_string(&file) else {
            continue;
        };
        for c in calls.captures_iter(&text) {
            let (map, names): (&str, Vec<String>) = if let (Some(m), Some(n)) = (c.get(1), c.get(2))
            {
                (m.as_str(), vec![n.as_str().to_owned()])
            } else if let (Some(m), Some(n)) = (c.get(3), c.get(4)) {
                (m.as_str(), vec![n.as_str().to_owned()])
            } else if let (Some(m), Some(n)) = (c.get(5), c.get(6)) {
                let names = quoted
                    .captures_iter(n.as_str())
                    .map(|q| q[1].to_owned())
                    .collect();
                (m.as_str(), names)
            } else {
                continue;
            };
            let map = map.split('-').next().unwrap_or_default().to_lowercase();
            let list = catalog.entry(map).or_default();
            for name in names {
                if name != "*" && !list.iter().any(|n: &String| n.eq_ignore_ascii_case(&name)) {
                    list.push(name);
                }
            }
        }
    }
    for (map, names, _) in MONSTER_EXTRAS {
        let list = catalog.entry((*map).to_owned()).or_default();
        for name in *names {
            assert!(
                !list.iter().any(|n| n.eq_ignore_ascii_case(name)),
                "MONSTER_EXTRAS' {name} in {map} is read from the source now: take it off"
            );
            list.push((*name).to_owned());
        }
    }
    let (needed, gaps) = monster_map_ids();
    for (map, highest) in needed {
        let list = catalog.entry(map).or_default();
        while list.len() < highest {
            list.push(format!("Stand-in {}", list.len() + 1));
        }
    }
    (catalog, gaps)
}

/// The highest monster map ID scripts use in each map (see [`monster_catalog`]), and the files
/// with map IDs and no `mapName`.
fn monster_map_ids() -> (std::collections::BTreeMap<String, usize>, Vec<String>) {
    let hunt = regex::Regex::new(r#"huntMonsterMapID\(\s*["'`]([^"'`]+)["'`],\s*(\d+)"#).unwrap();
    let map_name = regex::Regex::new(r#"mapName\s*=\s*["'`]([^"'`]+)["'`]"#).unwrap();
    let map_id = regex::Regex::new(r"\w*MapId\s*=\s*(\d+)\s*;").unwrap();
    let mut needed: std::collections::BTreeMap<String, usize> = Default::default();
    let mut gaps = Vec::new();
    let mut need = |map: &str, id: usize| {
        let map = map.split('-').next().unwrap_or_default().to_lowercase();
        let highest = needed.entry(map).or_default();
        *highest = (*highest).max(id);
    };
    for file in all_scripts() {
        let Ok(text) = std::fs::read_to_string(&file) else {
            continue;
        };
        for c in hunt.captures_iter(&text) {
            if let Ok(id) = c[2].parse() {
                need(&c[1], id);
            }
        }
        // 0 is "none" (a target not picked yet).
        let ids: Vec<usize> = map_id
            .captures_iter(&text)
            .filter_map(|c| c[1].parse().ok())
            .filter(|&id| id > 0)
            .collect();
        let Some(&highest) = ids.iter().max() else {
            continue;
        };
        match map_name.captures(&text) {
            Some(c) => need(&c[1], highest),
            None => gaps.push(
                file.strip_prefix(repo().join("scripts"))
                    .unwrap_or(&file)
                    .display()
                    .to_string(),
            ),
        }
    }
    (needed, gaps)
}

/// Items scripts buy from a shop under a name built at run time, which [`shop_catalog`] can't read
/// from the source: the shop, the names, and where they come from.
const SHOP_EXTRAS: &[(i64, &[&str], &str)] = &[
    (
        1415,
        &[
            "Blinding Daggers of Destiny",
            "Blinding Bow of Destiny",
            "Blinding Mace of Destiny",
            "Blinding Scythe of Destiny",
            "Blinding Broadsword of Destiny",
            "Blinding Blade of Destiny",
        ],
        "CoreBLOD's getBlindingWeapon: `Blinding ${WeaponOfDestiny[weapon]} of Destiny`",
    ),
    (
        222,
        &["Behemoth Blade of Light"],
        "BehemothBlades: `Behemoth Blade of ${blade}` (EvolvedShadowOrbItems buys the Shadow one by name)",
    ),
    (
        1307,
        &["DOOMFire OF Gravelyn", "Horned DOOMFire Helm"],
        "GravelynDoomFire: its `shopItems` array",
    ),
    (
        2322,
        &["Vigil"],
        "KathoolDepths_LW: `vigilShopId`, a constant",
    ),
];

/// The auras potions put on you where the aura isn't named after the potion, as the scripts that
/// check for them spell them (the fake names the rest after the potion): CoreLoneWolf's
/// `getPotionAuraName`, which the run checks still names each.
const POTION_AURAS: &[(&str, &str)] = &[
    ("Sage Tonic", "Sage"),
    ("Might Tonic", "Might"),
    ("Fate Tonic", "Fate"),
    ("Body Tonic", "Body"),
    ("Potent Honor Potion", "Potent Honor Malice"),
];
const POTION_AURAS_FROM: &str = "scripts/ultraslw/CoreLoneWolf.ts";

/// Each merge shop's stock, by shop ID, from its merge script: the shop it names
/// (`startBuyAllMerge("map", id, ...)`), the items its options list (`option.bool("29574",
/// "Synderes' Cape of Doom", ...)`, the `Select` group's switches, named by item ID) and the
/// materials it farms (`findIngredients`' `case "Synderes' Souvenir":`); and what any script buys
/// outright (`buyItem(<map>, <shop>, "<name>" or <ID>)`, plus [`SHOP_EXTRAS`]), sold for nothing.
/// A shop's contents reach the game only once it loads, so a tape can't supply them. Also the merge
/// scripts it found nothing in, by what was missing.
fn shop_catalog() -> (std::collections::HashMap<i64, ShopStock>, Vec<String>) {
    // A string literal in any quote: Prettier picks single quotes for a name with a `"`, and some
    // ports keep C#'s verbatim strings as template literals.
    const TEXT: &str = r#"(?:"((?:[^"\\]|\\.)*)"|'((?:[^'\\]|\\.)*)'|`([^`$]*)`)"#;
    let shop = regex::Regex::new(r#"startBuyAllMerge\(\s*["'`][^"'`]*["'`],\s*(\d+)"#).unwrap();
    let item =
        regex::Regex::new(&format!(r#"option\.bool\(\s*["'`](\d+)["'`],\s*{TEXT}"#)).unwrap();
    let case = regex::Regex::new(&format!(r#"case\s+{TEXT}\s*:"#)).unwrap();
    // `buyItem(<map>, <shop>, <name or ID>, ...)`, the map a literal or an expression
    // (`bot.map.name`).
    let bought = regex::Regex::new(&format!(
        r#"buyItem\(\s*(?:{TEXT}|[\w.]+)\s*,\s*(\d+)\s*,\s*(?:{TEXT}|(\d+)\s*[,)])"#
    ))
    .unwrap();
    let mut sold: std::collections::HashMap<i64, Vec<(i64, String)>> = Default::default();
    for (shop, names, _) in SHOP_EXTRAS {
        let entry = sold.entry(*shop).or_default();
        entry.extend(names.iter().map(|n| (item_id_for(n), (*n).to_owned())));
    }
    let literal = |c: &regex::Captures, at: usize| {
        c.get(at)
            .or_else(|| c.get(at + 1))
            .or_else(|| c.get(at + 2))
            .map(|m| m.as_str().replace("\\'", "'").replace("\\\"", "\""))
            .unwrap_or_default()
    };
    let mut catalog: std::collections::HashMap<i64, ShopStock> = Default::default();
    let mut gaps = Vec::new();
    for file in all_scripts() {
        let Ok(text) = std::fs::read_to_string(&file) else {
            continue;
        };
        for c in bought.captures_iter(&text) {
            // `bot.shops.buyItem(name, quantity)` names no shop.
            if text[..c.get(0).unwrap().start()].ends_with("shops.") {
                continue;
            }
            let Ok(shop) = c[4].parse::<i64>() else {
                continue;
            };
            let item = match c.get(8) {
                Some(id) => id
                    .as_str()
                    .parse()
                    .ok()
                    .map(|id| (id, format!("Item {id}"))),
                None => {
                    let name = literal(&c, 5);
                    (!name.is_empty()).then(|| (item_id_for(&name), name))
                }
            };
            if let Some(item) = item {
                let stale = SHOP_EXTRAS.iter().any(|(s, names, _)| {
                    *s == shop && names.iter().any(|n| n.eq_ignore_ascii_case(&item.1))
                });
                assert!(
                    !stale,
                    "SHOP_EXTRAS' {} in shop {shop} is read from the source now: take it off",
                    item.1
                );
                sold.entry(shop).or_default().push(item);
            }
        }
        let Some(id) = shop.captures(&text).and_then(|c| c[1].parse::<i64>().ok()) else {
            continue;
        };
        let stock = catalog.entry(id).or_default();
        let items: Vec<(i64, String)> = item
            .captures_iter(&text)
            .filter_map(|c| Some((c[1].parse().ok()?, literal(&c, 2))))
            .collect();
        let materials: Vec<String> = case.captures_iter(&text).map(|c| literal(&c, 1)).collect();
        let name = file
            .strip_prefix(repo().join("scripts"))
            .unwrap_or(&file)
            .display()
            .to_string();
        match (items.is_empty(), materials.is_empty()) {
            (true, true) => gaps.push(format!("{name}: no items, no materials")),
            (true, false) => gaps.push(format!("{name}: no items")),
            (false, true) => gaps.push(format!("{name}: no materials")),
            _ => {}
        }
        for (id, name) in items {
            if !stock.items.iter().any(|(i, _)| *i == id) {
                stock.items.push((id, name));
            }
        }
        for material in materials {
            if !stock.materials.contains(&material) {
                stock.materials.push(material);
            }
        }
    }
    for (shop, items) in sold {
        let stock = catalog.entry(shop).or_default();
        for (id, name) in items {
            if !stock
                .sold
                .iter()
                .any(|(i, n)| *i == id || n.eq_ignore_ascii_case(&name))
            {
                stock.sold.push((id, name));
            }
        }
    }
    gaps.sort();
    (catalog, gaps)
}

/// Every script under `scripts/`.
fn all_scripts() -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() {
                if !path.ends_with("node_modules") {
                    walk(&path, out);
                }
            } else if path.extension().is_some_and(|e| e == "ts") {
                out.push(path);
            }
        }
    }
    let mut files = Vec::new();
    walk(&repo().join("scripts"), &mut files);
    // In order: the catalogs number a map's monsters as they find them, and fights go by number.
    files.sort();
    files
}

/// How one draft went.
#[derive(Debug)]
struct Outcome {
    script: String,
    /// "finished", "running", "error" or "not started".
    kind: &'static str,
    /// The error, for "error".
    detail: String,
    calls: u64,
    secs: f64,
    /// How far it got on its (virtual) clock.
    clock_secs: f64,
    /// What it got done, by the engine's counters: kills, quests accepted, quests turned in; and
    /// by the fake's, purchases.
    kills: u64,
    accepted: u64,
    completed: u64,
    bought: u64,
    last_log: String,
    /// The last calls into the fake.
    recent: Vec<String>,
    /// The clock's report, when the draft ran out of real time before its clock's budget.
    clock: String,
    /// The [`DRAFT_OPTIONS`] it declares (their indexes).
    options_set: Vec<usize>,
}

/// What the fake world knows from the scripts: each map's monsters, each merge shop's stock.
struct Catalogs {
    monsters: std::collections::HashMap<String, Vec<String>>,
    shops: std::collections::HashMap<i64, ShopStock>,
}

fn run_one(
    world: &tape::Tape,
    catalog: &Catalogs,
    path: &Path,
    budget: Duration,
    real_cap: Duration,
) -> Outcome {
    let script = path
        .strip_prefix(repo().join("scripts"))
        .unwrap_or(path)
        .display()
        .to_string();
    // A data folder of its own: drafts that run at once share no options file or army log
    // (`0AllUltras_LW` syncs under its ultras' own log names).
    static DRAFTS: AtomicUsize = AtomicUsize::new(0);
    let bot = Bot::with_paths(turu_bot::paths::Paths {
        root: crate::common::data_dir()
            .join("drafts")
            .join(DRAFTS.fetch_add(1, Ordering::Relaxed).to_string()),
    });
    bot.army.set_stand_in(Some(Arc::new(Partners::of(&script))));
    if let Ok(options) = serde_json::from_value(world.header.options.clone()) {
        *bot.options.write() = options;
    }
    bot.options.write().action_delay = 0;
    let fake = Arc::new(
        WorldFake::from_tape(world)
            .with_monsters(catalog.monsters.clone())
            .with_shops(catalog.shops.clone())
            .with_potion_auras(
                POTION_AURAS
                    .iter()
                    .map(|(potion, aura)| ((*potion).to_owned(), (*aura).to_owned()))
                    .collect(),
            ),
    );
    fake.attach(&bot);
    // A virtual clock (decision 2): the draft's sleeps and waits take no real time, so its
    // budget is virtual and it gets as far as that many seconds of play would take it.
    bot.control.set_virtual();
    bot.set_host(Some(fake.clone()));
    // Never the network: a draft that reaches a relogin would fetch AQW's server list.
    bot.servers
        .set_fetcher(|| Err("a draft run doesn't fetch the server list".into()));
    bot.scripts.load(path);
    let started = Instant::now();
    let clock_start = bot.control.now();
    // The run takes part in the draft's clock: it waits until the budget, and the clock can't
    // pass a participant's deadline while it hasn't looked, so a draft stops at exactly its
    // budget however fast the machine is (looking at the clock every 50 ms of real time, a draft
    // got a few seconds further on one machine than another, and threw past its budget or not).
    let participant = bot.control.participate();
    let options_set = set_draft_options(&bot, &script);
    if let Err(e) = options_set.clone().and_then(|_| script::start_silent(&bot)) {
        return Outcome {
            script,
            kind: "not started",
            detail: e,
            calls: 0,
            secs: 0.0,
            clock_secs: 0.0,
            kills: 0,
            accepted: 0,
            completed: 0,
            bought: 0,
            last_log: String::new(),
            recent: Vec::new(),
            clock: String::new(),
            options_set: Vec::new(),
        };
    }
    // On its clock's budget, or a real one: a draft that keeps a thread busy holds the clock still.
    // The ticker wakes the wait to look at the draft's end and the real time.
    let ticking = Arc::new(AtomicBool::new(true));
    let ticker = {
        let (ticking, control) = (ticking.clone(), bot.control.clone());
        std::thread::spawn(move || {
            while ticking.load(Ordering::Acquire) {
                std::thread::sleep(Duration::from_millis(20));
                control.notify();
            }
        })
    };
    let _ = bot.control.wait_until(Some(clock_start + budget), || {
        !bot.scripts.is_running() || started.elapsed() >= real_cap
    });
    let running = bot.scripts.is_running();
    // How far it played: its budget, or less when it ended or held the clock (not its stop
    // handlers' time).
    let played = bot.control.now().since(clock_start);
    // Where it was at the budget, before Stop runs its handlers.
    let recent = fake.recent();
    // Out of real time before its clock's budget: who held the clock still.
    let clock = if running && played < budget {
        bot.control.clock_report()
    } else {
        String::new()
    };
    // The log as far as the draft got on its own: a handler that throws while Stop ends the run
    // is Stop's doing, not the draft's.
    let logged_before_stop = bot.logs.script.text().lines().count();
    // Stop before letting go of the clock: let go first and the draft would play on past its
    // budget until Stop reached it. Then the clock runs for its stop handlers.
    if running {
        script::stop(&bot);
    }
    drop(participant);
    ticking.store(false, Ordering::Release);
    let _ = ticker.join();
    if running {
        let until = Instant::now() + Duration::from_secs(5);
        while bot.scripts.is_running() && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    let log = bot.logs.script.text();
    let lines: Vec<&str> = log.lines().collect();
    // The engine's own lines for a throw: the script's (`Script <file> failed:`), a handler's or
    // a listener's (bot/js/turu.ts: caught, and the script goes on). Not a script's own log line
    // that says something failed ("Scroll of Enrage preparation failed: ...") before it stops.
    // A throw counts even when the draft was still running at its budget: it threw on its own
    // while the run was asking it to stop. A throw after Stop isn't one: the engine ends that
    // run as stopped, as VibeSkua does (`_stoppedByScript`); a handler's is only counted before.
    let file = path
        .file_name()
        .map(|f| f.to_string_lossy().into_owned())
        .unwrap_or_default();
    let thrown = format!("Script {file} failed:");
    let failed = lines.iter().enumerate().position(|(i, l)| {
        // "[12:00:00] <the engine's text>"; a script's Core.logger line has a second time.
        let text = l.split_once("] ").map_or(*l, |(_, rest)| rest);
        text.starts_with(&thrown)
            || (i < logged_before_stop
                && ((text.starts_with("Handler \"") && text.contains("\" failed: "))
                    || (text.starts_with('"') && text.contains("\" listener failed: "))))
    });
    let (kind, detail) = match failed {
        Some(at) => (
            "error",
            lines[at..]
                .iter()
                .take(3)
                .copied()
                .collect::<Vec<_>>()
                .join(" | "),
        ),
        None if running => ("running", String::new()),
        None if bot.scripts.status() == ScriptStatus::Error => ("error", "status Error".into()),
        None => ("finished", String::new()),
    };
    // One lock: three `bot.state.lock()` in the literal below would deadlock (each guard lives to
    // the end of the expression).
    let stats = bot.state.lock().stats.clone();
    Outcome {
        script,
        kind,
        detail,
        calls: fake.calls(),
        secs: started.elapsed().as_secs_f64(),
        clock_secs: played.as_secs_f64(),
        kills: stats.kills,
        accepted: stats.quests_accepted,
        completed: stats.quests_completed,
        bought: fake.purchases(),
        last_log: {
            let mut meaningful: Vec<&str> = lines
                .iter()
                .rev()
                .filter(|l| !l.contains("Script ") && !l.contains("SetOptions"))
                .take(3)
                .copied()
                .collect();
            meaningful.reverse();
            meaningful.join(" | ")
        },
        recent,
        clock,
        options_set: options_set.unwrap_or_default(),
    }
}

#[test]
#[ignore = "runs drafts for seconds each: by hand or nightly"]
fn draft_smoke_sample() {
    crate::common::data_dir();
    let every: usize = env_or("DRAFT_EVERY", 32);
    let filter = std::env::var("DRAFT_FILTER").unwrap_or_default();
    let budget = Duration::from_secs_f64(env_or("DRAFT_SECS", 60.0));
    let real_cap = Duration::from_secs_f64(env_or("DRAFT_REAL_SECS", 10.0));
    let jobs: usize = env_or("DRAFT_JOBS", 6);
    let tape_name = std::env::var("DRAFT_TAPE").unwrap_or_else(|_| "kill_farm".into());
    let world = tape::read(&repo().join(format!("bot/tests/tapes/{tape_name}.tape.jsonl.gz")))
        .expect("the world's tape");

    let all = drafts();
    let picked: Vec<PathBuf> = all
        .iter()
        .filter(|p| {
            filter.is_empty()
                || filter
                    .split(',')
                    .map(str::trim)
                    .filter(|f| !f.is_empty())
                    .any(|f| p.to_string_lossy().contains(f))
        })
        .enumerate()
        .filter(|(i, _)| !filter.is_empty() || i % every.max(1) == 0)
        .map(|(_, p)| p.clone())
        .collect();
    println!(
        "{} drafts, running {} for {budget:?} each on a virtual clock (at most {real_cap:?} real), \
         {jobs} at once, world from {tape_name}",
        all.len(),
        picked.len()
    );

    let queue = Arc::new(Mutex::new(picked));
    let outcomes = Arc::new(Mutex::new(Vec::new()));
    let world = Arc::new(world);
    let (shops, shop_gaps) = shop_catalog();
    let source = std::fs::read_to_string(repo().join(POTION_AURAS_FROM)).unwrap_or_default();
    for (potion, aura) in POTION_AURAS {
        assert!(
            source.contains(&format!("\"{potion}\"")) && source.contains(&format!("\"{aura}\"")),
            "POTION_AURAS' {potion} -> {aura} isn't in {POTION_AURAS_FROM} any more"
        );
    }
    for (script, signals, _) in SHARED_SIGNALS {
        let source =
            std::fs::read_to_string(repo().join("scripts").join(script)).unwrap_or_default();
        for signal in *signals {
            assert!(
                source.contains(&format!("`{signal}`"))
                    || source.contains(&format!("\"{signal}\"")),
                "SHARED_SIGNALS' {signal} isn't in {script} any more"
            );
        }
    }
    let (monsters, monster_gaps) = monster_catalog();
    let catalog = Arc::new(Catalogs { monsters, shops });
    // A script that fights by map ID in a map it doesn't name (`mapName`): its IDs go unused.
    for gap in &monster_gaps {
        println!("monster catalog gap: map IDs and no mapName in {gap}");
    }
    println!(
        "monster catalog: {} maps; shop catalog: {} shops, {} items bought outright",
        catalog.monsters.len(),
        catalog.shops.len(),
        catalog.shops.values().map(|s| s.sold.len()).sum::<usize>()
    );
    // A merge script the catalog read nothing from: its shop is empty, or its items need nothing.
    for gap in &shop_gaps {
        println!("shop catalog gap: {gap}");
    }
    let workers: Vec<_> = (0..jobs)
        .map(|_| {
            let (queue, outcomes, world, catalog) = (
                queue.clone(),
                outcomes.clone(),
                world.clone(),
                catalog.clone(),
            );
            std::thread::spawn(move || {
                loop {
                    // Not in the `while let`: its lock would last the whole body.
                    let next = queue.lock().pop();
                    let Some(path) = next else { break };
                    let outcome = run_one(&world, &catalog, &path, budget, real_cap);
                    println!(
                        "{:9} {:6.1}s real {:6.1}s clock {:6} calls  {}",
                        outcome.kind,
                        outcome.secs,
                        outcome.clock_secs,
                        outcome.calls,
                        outcome.script
                    );
                    outcomes.lock().push(outcome);
                }
            })
        })
        .collect();
    for worker in workers {
        worker.join().unwrap();
    }

    let mut outcomes = std::mem::take(&mut *outcomes.lock());
    outcomes.sort_by(|a, b| a.kind.cmp(b.kind).then(a.script.cmp(&b.script)));
    let mut report = String::new();
    for outcome in &mut outcomes {
        if outcome.kind == "error"
            && let Some((_, why)) = KNOWN_FAKE_GAPS.iter().find(|(s, _)| *s == outcome.script)
        {
            outcome.kind = "known";
            outcome.detail = format!("{} (a known gap in the fake: {why})", outcome.detail);
        }
    }
    let did = |f: fn(&Outcome) -> u64| outcomes.iter().map(f).sum::<u64>();
    let with = |f: fn(&Outcome) -> u64| outcomes.iter().filter(|o| f(o) > 0).count();
    report.push_str(&format!(
        "progress: {} kills ({} drafts), {} quests accepted ({}), {} turned in ({}), {} purchases ({})\n",
        did(|o| o.kills),
        with(|o| o.kills),
        did(|o| o.accepted),
        with(|o| o.accepted),
        did(|o| o.completed),
        with(|o| o.completed),
        did(|o| o.bought),
        with(|o| o.bought),
    ));
    // Stopped by the real-time cap before their budget: how far they got depends on the machine.
    let cut: Vec<String> = outcomes
        .iter()
        .filter(|o| o.kind == "running" && o.clock_secs < budget.as_secs_f64() - 0.5)
        .map(|o| format!("{} ({:.1} s)", o.script, o.clock_secs))
        .collect();
    report.push_str(&format!(
        "cut at the real-time cap ({real_cap:?}): {}{}\n",
        cut.len(),
        if cut.is_empty() {
            String::new()
        } else {
            format!(": {}", cut.join(", "))
        }
    ));
    for kind in ["error", "not started", "known", "finished", "running"] {
        let these: Vec<&Outcome> = outcomes.iter().filter(|o| o.kind == kind).collect();
        report.push_str(&format!("{kind}: {}\n", these.len()));
        for o in these {
            report.push_str(&format!(
                "  {} ({:.1} s real, {:.1} s on its clock, {} calls){}\n      last log: {}\n      last calls:\n{}",
                o.script,
                o.secs,
                o.clock_secs,
                o.calls,
                if o.detail.is_empty() {
                    String::new()
                } else {
                    format!("\n      {}", o.detail)
                },
                o.last_log,
                o.recent
                    .iter()
                    .map(|c| format!("        {c}\n"))
                    .collect::<String>()
            ));
            if !o.clock.is_empty() {
                report.push_str(&format!(
                    "      clock: {}\n",
                    o.clock.replace('\n', "\n      ")
                ));
            }
        }
    }
    let out = repo().join("target/draft-smoke.txt");
    std::fs::write(&out, &report).unwrap();
    println!("\n{report}\nReport: {}", out.display());
    let failed: Vec<&str> = outcomes
        .iter()
        .filter(|o| matches!(o.kind, "error" | "not started"))
        .map(|o| o.script.as_str())
        .collect();
    assert!(
        failed.is_empty(),
        "{} drafts failed: {failed:?}",
        failed.len()
    );
    // A known gap that no longer fails: take it off the list.
    let mended: Vec<&str> = KNOWN_FAKE_GAPS
        .iter()
        .map(|(script, _)| *script)
        .filter(|script| {
            outcomes
                .iter()
                .any(|o| o.script == *script && o.kind != "known")
        })
        .collect();
    assert!(
        mended.is_empty(),
        "listed in KNOWN_FAKE_GAPS but no longer failing: {mended:?}"
    );
    // An override for drafts that ran, none of which declares it: take it off the list.
    let unused: Vec<String> = DRAFT_OPTIONS
        .iter()
        .enumerate()
        .filter(|(i, (drafts, ..))| {
            outcomes
                .iter()
                .any(|o| drafts.iter().any(|d| o.script.contains(d)))
                && !outcomes.iter().any(|o| o.options_set.contains(i))
        })
        .map(|(_, (drafts, name, ..))| format!("{name} for {drafts:?}"))
        .collect();
    assert!(
        unused.is_empty(),
        "in DRAFT_OPTIONS but declared by none of the drafts it's for: {unused:?}"
    );
}
