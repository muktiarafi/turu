//! Diagnostics > Console: runs a TypeScript snippet with `bot` in scope (VibeSkua's C# snippet
//! console). It runs on a bot thread that counts as a script thread, so Stop interrupts it, and
//! its `bot.log` lines, its result (the last expression's value, as JSON) or its error go to the
//! output.
//!
//! Per tab ([`State`]): the output, whether a snippet runs, and the snippet last run (a native
//! page starts from it). The egui window keeps its own editor. Every function here locks the state
//! only briefly and never while calling the bot, so a shell may call them from a frame.

use super::Control;
use super::tab_state::{PerTab, Shared, lock};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use turu_bot::Bot;
use turu_bot::script::{self, RunError};

/// VibeSkua's default snippet, in TypeScript.
pub const DEFAULT_CODE: &str = "bot.log(\"Test\");";

/// Most output lines kept; older ones are dropped (a snippet may log in a loop).
const MAX_OUTPUT: usize = 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
#[repr(u8)]
pub enum LineKind {
    /// "› code": a run started.
    Run,
    Log,
    Value,
    Error,
    Stopped,
}

pub struct State {
    output: Vec<(LineKind, String)>,
    /// Lines dropped from the front, so a shell can tell new lines from old.
    dropped: u64,
    /// From Run until the bot thread is done (the engine's flag is set a moment later).
    running: bool,
    code: String,
}

impl Default for State {
    fn default() -> Self {
        Self {
            output: Vec::new(),
            dropped: 0,
            running: false,
            code: DEFAULT_CODE.into(),
        }
    }
}

pub static STATE: PerTab<State> = PerTab::new();

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct ConsoleView {
    /// The snippet last run (VibeSkua's default before any).
    pub code: String,
    pub running: bool,
    /// Run, or why not (a script or Auto runs). An empty snippet is the shell's to refuse.
    pub run: Control,
    /// Lines dropped from the front of `output` so far.
    pub dropped: u64,
    pub output: Vec<ConsoleLine>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct ConsoleLine {
    pub kind: LineKind,
    pub text: String,
}

/// What a shell shows: a snippet runs, and Run or why not.
pub fn status(bot: &Arc<Bot>) -> (bool, Control) {
    let running = lock(&STATE.of(bot)).running || bot.scripts.is_snippet_running();
    let run = if running {
        Control::off("A snippet runs")
    } else if bot.scripts.status().is_active() {
        Control::off("Stop the script first")
    } else if bot.auto.is_running() {
        Control::off("Stop Auto Attack/Hunt first")
    } else {
        Control::on()
    };
    (running, run)
}

/// The output, for the egui window.
pub fn output(bot: &Arc<Bot>) -> Vec<(LineKind, String)> {
    lock(&STATE.of(bot)).output.clone()
}

pub fn view(bot: &Arc<Bot>) -> ConsoleView {
    let (running, run) = status(bot);
    let state = STATE.of(bot);
    let s = lock(&state);
    ConsoleView {
        code: s.code.clone(),
        running,
        run,
        dropped: s.dropped,
        output: s
            .output
            .iter()
            .map(|(kind, text)| ConsoleLine {
                kind: *kind,
                text: text.clone(),
            })
            .collect(),
    }
}

fn push(state: &Shared<State>, kind: LineKind, text: impl Into<String>) {
    let mut s = lock(state);
    if s.output.len() >= MAX_OUTPUT {
        s.output.remove(0);
        s.dropped += 1;
    }
    s.output.push((kind, text.into()));
}

/// Runs `code` on a bot thread of `bot`'s tab. An error when it can't run now.
pub fn run(bot: &Arc<Bot>, code: &str) -> Result<(), String> {
    let (_, can) = status(bot);
    if !can.enabled {
        return Err(can.reason);
    }
    if code.trim().is_empty() {
        return Err("Write a snippet first".into());
    }
    let state = STATE.of(bot);
    let trimmed = code.trim();
    let first = trimmed.lines().next().unwrap_or_default();
    let more = if trimmed.lines().nth(1).is_some() { " …" } else { "" };
    {
        let mut s = lock(&state);
        s.running = true;
        s.code = code.to_owned();
    }
    push(&state, LineKind::Run, format!("› {first}{more}"));
    let code = code.to_owned();
    crate::turu::spawn_action_for(bot.clone(), "Console", move |bot| {
        let sink = state.clone();
        let result = script::run_snippet(bot, &code, move |line| push(&sink, LineKind::Log, line));
        match result {
            Ok(value) => push(&state, LineKind::Value, value),
            Err(RunError::Stopped) => push(&state, LineKind::Stopped, "Stopped"),
            Err(RunError::Failed(error)) => push(&state, LineKind::Error, error),
        }
        lock(&state).running = false;
        Ok(())
    });
    Ok(())
}

pub fn stop(bot: &Arc<Bot>) {
    script::stop_snippet(bot);
}

/// Empties the output.
pub fn clear(bot: &Arc<Bot>) {
    let state = STATE.of(bot);
    let mut s = lock(&state);
    s.dropped += s.output.len() as u64;
    s.output.clear();
}

/// The output as text, a line each.
pub fn copy_text(bot: &Arc<Bot>) -> String {
    let text: Vec<String> = lock(&STATE.of(bot)).output.iter().map(|(_, t)| t.clone()).collect();
    text.join("\n")
}

/// The Console's actions (`POST /models/console/do`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[cfg_attr(
    feature = "typegen",
    derive(facet::Facet),
    facet(tag = "action", rename_all = "kebab-case")
)]
#[serde(tag = "action", rename_all = "kebab-case")]
#[repr(C)]
pub enum ConsoleAct {
    /// Runs `code` as a snippet.
    Run {
        code: String,
    },
    Stop,
    Clear,
}

pub fn act(bot: &Arc<Bot>, act: ConsoleAct) -> Result<(), String> {
    match act {
        ConsoleAct::Run { code } => run(bot, &code)?,
        ConsoleAct::Stop => stop(bot),
        ConsoleAct::Clear => clear(bot),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn output_keeps_the_newest_lines() {
        let bot = Bot::new();
        let state = STATE.of(&bot);
        for i in 0..MAX_OUTPUT + 5 {
            push(&state, LineKind::Log, i.to_string());
        }
        let v = view(&bot);
        assert_eq!((v.output.len(), v.dropped), (MAX_OUTPUT, 5));
        assert_eq!(v.output[0].text, "5");
        clear(&bot);
        let v = view(&bot);
        assert_eq!((v.output.len(), v.dropped), (0, MAX_OUTPUT as u64 + 5));
        assert_eq!(v.code, DEFAULT_CODE);
        assert!(act(&bot, ConsoleAct::Run { code: "  ".into() }).is_err());
        assert!(serde_json::from_value::<ConsoleAct>(json!({ "action": "nope" })).is_err());
    }
}
