//! The draft smoke run (docs/REPLAY_TESTS_PROPOSAL.md, phase 4): each converted script that
//! wasn't read through by hand ("Converted by tools/cs2ts, not yet read through by hand") and has
//! a `main` (Core files are libraries) runs for
//! a few seconds against a [`WorldFake`] seeded from a tape. A draft fails when it throws; it
//! passes when it ends cleanly or is still running at its budget.
//!
//! Ignored: run by hand or nightly.
//!
//! ```text
//! cargo nextest run -p turu_bot draft_smoke --run-ignored only --no-capture
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

use parking_lot::Mutex;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use turu_bot::script::{self, ScriptStatus};
use turu_bot::testing::{ShopStock, WorldFake};
use turu_bot::{Bot, tape};

const MARK: &str = "not yet read through by hand";

/// Drafts that fail because of a gap in the fake world, not a fault of their own, and why:
/// reported as "known", not failing the run. A listed draft that passes fails the run, so the
/// list can't go stale. A script fix goes in `muktiarafi/turu-scripts` instead.
const KNOWN_FAKE_GAPS: &[(&str, &str)] = &[(
    "story/TowerOfDoom.ts",
    "it finds each floor's monster by its cell (r10), never by name, so the monster catalog \
     (built from scripts that name them) has none for towerofdoom2 to 10; floor 1 passes",
)];

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

/// Each map's monsters, by name, from the scripts' own calls: `huntMonster("map", "Monster"`,
/// `killMonster("map", "cell", "pad", "Monster"`, `killQuest(id, "map", "Monster"` or a list.
/// Monster names never reach the game's calls (a kill goes by ID), so the fake world can't learn
/// them from a tape (docs/REPLAY_TESTS_PROPOSAL.md, phase 4).
fn monster_catalog() -> std::collections::HashMap<String, Vec<String>> {
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
    catalog
}

/// Each merge shop's stock, by shop ID, from its merge script: the shop it names
/// (`startBuyAllMerge("map", id, ...)`), the items its options list (`option.bool("29574",
/// "Synderes' Cape of Doom", ...)`, the `Select` group's switches, named by item ID) and the
/// materials it farms (`findIngredients`' `case "Synderes' Souvenir":`). A shop's contents reach the
/// game only once it loads, so a tape can't supply them. Also the merge scripts it found nothing in,
/// by what was missing.
fn shop_catalog() -> (std::collections::HashMap<i64, ShopStock>, Vec<String>) {
    // A string literal in any quote: Prettier picks single quotes for a name with a `"`, and some
    // ports keep C#'s verbatim strings as template literals.
    const TEXT: &str = r#"(?:"((?:[^"\\]|\\.)*)"|'((?:[^'\\]|\\.)*)'|`([^`$]*)`)"#;
    let shop = regex::Regex::new(r#"startBuyAllMerge\(\s*["'`][^"'`]*["'`],\s*(\d+)"#).unwrap();
    let item =
        regex::Regex::new(&format!(r#"option\.bool\(\s*["'`](\d+)["'`],\s*{TEXT}"#)).unwrap();
    let case = regex::Regex::new(&format!(r#"case\s+{TEXT}\s*:"#)).unwrap();
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
    let bot = Bot::new();
    if let Ok(options) = serde_json::from_value(world.header.options.clone()) {
        *bot.options.write() = options;
    }
    bot.options.write().action_delay = 0;
    let fake = Arc::new(
        WorldFake::from_tape(world)
            .with_monsters(catalog.monsters.clone())
            .with_shops(catalog.shops.clone()),
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
    if let Err(e) = script::start_silent(&bot) {
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
        };
    }
    // On its clock's budget, or a real one: a draft that keeps a thread busy holds the clock still.
    while bot.scripts.is_running()
        && bot.control.now().since(clock_start) < budget
        && started.elapsed() < real_cap
    {
        std::thread::sleep(Duration::from_millis(50));
    }
    let running = bot.scripts.is_running();
    // Where it was at the budget, before Stop runs its handlers.
    let recent = fake.recent();
    // Out of real time before its clock's budget: who held the clock still.
    let clock = if running && bot.control.now().since(clock_start) < budget {
        bot.control.clock_report()
    } else {
        String::new()
    };
    if running {
        script::stop(&bot);
        let until = Instant::now() + Duration::from_secs(5);
        while bot.scripts.is_running() && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    let log = bot.logs.script.text();
    let lines: Vec<&str> = log.lines().collect();
    let failed = lines.iter().position(|l| l.contains(" failed:"));
    let (kind, detail) = match failed {
        _ if running => ("running", String::new()),
        Some(at) => (
            "error",
            lines[at..]
                .iter()
                .take(3)
                .copied()
                .collect::<Vec<_>>()
                .join(" | "),
        ),
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
        clock_secs: bot.control.now().since(clock_start).as_secs_f64(),
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
    let catalog = Arc::new(Catalogs {
        monsters: monster_catalog(),
        shops,
    });
    println!(
        "monster catalog: {} maps; shop catalog: {} shops",
        catalog.monsters.len(),
        catalog.shops.len()
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
}
