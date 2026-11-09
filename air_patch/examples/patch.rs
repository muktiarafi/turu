//! Patches a client SWF for AIR and lists what changed:
//! `cargo run -p turu_air_patch --example patch -- Game.swf Game-air.swf`.

use turu_air_patch::patch_swf;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [input, output] = &args[..] else {
        anyhow::bail!("usage: patch <in.swf> <out.swf>");
    };
    let bytes = std::fs::read(input)?;
    let started = std::time::Instant::now();
    let (patched, report) = patch_swf(&bytes)?;
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
    match report.game_problem() {
        Some(problem) => println!("not runnable as a game client: {problem}"),
        None => println!("looks runnable as a game client"),
    }
    Ok(())
}
