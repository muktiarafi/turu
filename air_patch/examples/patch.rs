//! Patches a SWF for AIR and lists what changed:
//! `cargo run -p turu_air_patch --example patch -- [--shared-stage] Game.swf Game-air.swf`
//! (AQW's client or a SWF it loads; `--shared-stage` as Turu Mas's tabs patch it,
//! `Options::shared_stage`).

use std::collections::BTreeMap;
use turu_air_patch::{Options, Target, patch_swf_with};

fn main() -> anyhow::Result<()> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut flag = |name: &str| match args.iter().position(|a| a == name) {
        Some(i) => {
            args.remove(i);
            true
        }
        None => false,
    };
    let target = Target::Game;
    let options = Options {
        shared_stage: flag("--shared-stage"),
    };
    let [input, output] = &args[..] else {
        anyhow::bail!("usage: patch [--shared-stage] <in.swf> <out.swf>");
    };
    let bytes = std::fs::read(input)?;
    let started = std::time::Instant::now();
    let (patched, report) = patch_swf_with(&bytes, target, options)?;
    let took = started.elapsed();
    std::fs::write(output, &patched)?;
    for (what, sites) in [("swap", &report.swapped), ("drop", &report.dropped)] {
        for site in sites {
            println!("{what}  {:<14} {}", site.name, site.method);
        }
    }
    for method in &report.undecodable {
        println!("undecodable  {method}");
    }
    // The shared stage's sites, counted by kind; the missed ones listed.
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for site in report.stage_roots.iter().chain(&report.stage_hooks) {
        *counts.entry(site.name.clone()).or_default() += 1;
    }
    for (name, count) in &counts {
        println!("stage  {name:<18} {count}");
    }
    for site in &report.stage_missed {
        println!("missed {:<18} {}", site.name, site.method);
    }
    println!(
        "{} ABC blocks, AIR loader: {}; {} swapped, {} dropped; {} -> {} bytes in {took:?}",
        report.abc_blocks,
        report.has_air_loader,
        report.swapped.len(),
        report.dropped.len(),
        bytes.len(),
        patched.len()
    );
    match report.problem() {
        Some(problem) => println!("not runnable under AIR ({target:?}): {problem}"),
        None => println!("looks runnable under AIR ({target:?})"),
    }
    Ok(())
}
