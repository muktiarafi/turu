//! `turu-tape`: tapes from the command line (docs/REPLAY_TESTS_PROPOSAL.md).
//!
//! ```text
//! turu-tape info <tape>               what a tape holds
//! turu-tape scrub <tape> <scrubbed>   rename every player, ready to commit
//! ```

use std::collections::BTreeMap;
use std::path::Path;
use std::process::ExitCode;
use turu_bot::tape::{self, Line};

const USAGE: &str = "usage:
  turu-tape info <tape>               what a tape holds
  turu-tape scrub <tape> <scrubbed>   rename every player (a new file), ready to commit";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.iter().map(String::as_str).collect::<Vec<_>>()[..] {
        ["info", path] => info(Path::new(path)),
        ["scrub", input, output] => tape::scrub_file(Path::new(input), Path::new(output))
            .map(|report| println!("{output}: {}", report.summary()))
            .map_err(|e| e.to_string()),
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("turu-tape: {e}");
            ExitCode::FAILURE
        }
    }
}

fn info(path: &Path) -> Result<(), String> {
    let tape = tape::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let h = &tape.header;
    println!(
        "format {}, Turu {}, {}, {}",
        h.format,
        h.turu,
        if h.from_world {
            "from the first map"
        } else {
            "from the start"
        },
        if h.scrubbed {
            "scrubbed"
        } else {
            "not scrubbed"
        },
    );
    let last_t = tape.lines.iter().map(line_t).max().unwrap_or(0);
    println!(
        "{} lines over {:.1} min; {}",
        tape.lines.len(),
        last_t as f64 / 60_000.0,
        match (&tape.end, tape.is_complete()) {
            (_, true) => "complete".to_owned(),
            (None, _) => format!(
                "no end line (stopped without finishing{})",
                if tape.truncated { ", cut short" } else { "" }
            ),
            (Some(end), false) =>
                format!("{} dropped, {} missing", end.dropped, tape.missing.len()),
        }
    );
    let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
    for line in &tape.lines {
        let kind = match line {
            Line::Call { name, .. } => format!("call {name}"),
            Line::Event { name, .. } => format!("event {name}"),
            Line::Sent { .. } => "sent".to_owned(),
            Line::Received { .. } => "received".to_owned(),
            Line::Note { what, .. } => format!("note {what}"),
        };
        *kinds.entry(kind).or_default() += 1;
    }
    let mut kinds: Vec<_> = kinds.into_iter().collect();
    kinds.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    for (kind, n) in kinds.iter().take(25) {
        println!("{n:>8}  {kind}");
    }
    Ok(())
}

fn line_t(line: &Line) -> u64 {
    match line {
        Line::Call { t, .. }
        | Line::Event { t, .. }
        | Line::Sent { t, .. }
        | Line::Received { t, .. }
        | Line::Note { t, .. } => *t,
    }
}
