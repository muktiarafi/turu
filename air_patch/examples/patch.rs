//! Patches a SWF for AIR and lists what changed:
//! `cargo run -p turu_air_patch --example patch -- [--skua] Game.swf Game-air.swf` (`--skua` for
//! VibeSkua's skua.swf, else AQW's client).

use turu_air_patch::{Target, patch_swf};

fn main() -> anyhow::Result<()> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let target = match args.iter().position(|a| a == "--skua") {
        Some(i) => {
            args.remove(i);
            Target::Skua
        }
        None => Target::Game,
    };
    let [input, output] = &args[..] else {
        anyhow::bail!("usage: patch [--skua] <in.swf> <out.swf>");
    };
    let bytes = std::fs::read(input)?;
    let started = std::time::Instant::now();
    let (patched, report) = patch_swf(&bytes, target)?;
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
