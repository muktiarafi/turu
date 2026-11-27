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
use turu_bot::testing::WorldFake;
use turu_bot::{Bot, tape};

const MARK: &str = "not yet read through by hand";

/// Drafts that fail because of a gap in the fake world, not a fault of their own, and why:
/// reported as "known", not failing the run. A listed draft that passes fails the run, so the
/// list can't go stale. A script fix goes in `muktiarafi/turu-scripts` instead.
const KNOWN_FAKE_GAPS: &[(&str, &str)] = &[(
    "story/TowerOfDoom.ts",
    "it looks up the monster in cell r10 of the map it joined; the fake keeps the seed tape's \
     monsters when a script joins another map",
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
    last_log: String,
    /// The last calls into the fake.
    recent: Vec<String>,
    /// The clock's report, when the draft ran out of real time before its clock's budget.
    clock: String,
}

fn run_one(world: &tape::Tape, path: &Path, budget: Duration, real_cap: Duration) -> Outcome {
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
    let fake = Arc::new(WorldFake::from_tape(world));
    // A virtual clock (decision 2): the draft's sleeps and waits take no real time, so its
    // budget is virtual and it gets as far as that many seconds of play would take it.
    bot.control.set_virtual();
    bot.set_host(Some(fake.clone()));
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
    Outcome {
        script,
        kind,
        detail,
        calls: fake.calls(),
        secs: started.elapsed().as_secs_f64(),
        clock_secs: bot.control.now().since(clock_start).as_secs_f64(),
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
    let workers: Vec<_> = (0..jobs)
        .map(|_| {
            let (queue, outcomes, world) = (queue.clone(), outcomes.clone(), world.clone());
            std::thread::spawn(move || {
                loop {
                    // Not in the `while let`: its lock would last the whole body.
                    let next = queue.lock().pop();
                    let Some(path) = next else { break };
                    let outcome = run_one(&world, &path, budget, real_cap);
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
