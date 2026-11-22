//! Bank (the nav bar's and the rail's direct action, VibeSkua's): opens the bank window. While
//! it runs a spinner shows; when nothing opened (`bank.open` does nothing logged out or while a
//! map loads), why, for a few seconds.

use super::tab_state::{PerTab, lock};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};
use turu_bot::Bot;

/// How long the reason nothing opened shows.
pub const NOTE_FOR: Duration = Duration::from_secs(4);

#[derive(Default)]
struct State {
    busy: bool,
    failed: Option<(String, Instant)>,
}

static STATE: PerTab<State> = PerTab::new();

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct BankView {
    pub busy: bool,
    /// Why the last press opened nothing ("Not logged in", "The map is still loading", ...),
    /// for [`NOTE_FOR`].
    pub note: Option<String>,
}

pub fn view(bot: &Arc<Bot>) -> BankView {
    let state = STATE.of(bot);
    let mut state = lock(&state);
    if state.failed.as_ref().is_some_and(|(_, at)| at.elapsed() >= NOTE_FOR) {
        state.failed = None;
    }
    BankView {
        busy: state.busy,
        note: state.failed.as_ref().map(|(why, _)| why.clone()),
    }
}

/// How long the note still shows (for a UI that repaints only when asked).
pub fn note_left(bot: &Arc<Bot>) -> Option<Duration> {
    let state = STATE.of(bot);
    let state = lock(&state);
    state
        .failed
        .as_ref()
        .and_then(|(_, at)| NOTE_FOR.checked_sub(at.elapsed()))
}

/// Opens the bank, on a bot thread.
pub fn open(bot: &Arc<Bot>) {
    let state = STATE.of(bot);
    *lock(&state) = State {
        busy: true,
        failed: None,
    };
    crate::turu::spawn_action_for(bot.clone(), "Bank", move |bot| {
        use turu_bot::api::player;
        let failed = if !player::logged_in(bot).unwrap_or(false) {
            Some("Not logged in".to_owned())
        } else if !player::world_loaded(bot).unwrap_or(false) {
            Some("The map is still loading".to_owned())
        } else {
            turu_bot::api::dispatch(bot, "bank.open", &[])
                .err()
                .map(|e| e.to_string())
        };
        *lock(&state) = State {
            busy: false,
            failed: failed.map(|why| (why, Instant::now())),
        };
        Ok(())
    });
}

/// The bank's actions (`POST /models/bank/do`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[cfg_attr(
    feature = "typegen",
    derive(facet::Facet),
    facet(tag = "action", rename_all = "kebab-case")
)]
#[serde(tag = "action", rename_all = "kebab-case")]
#[repr(C)]
pub enum BankAct {
    Open,
}

pub fn act(bot: &Arc<Bot>, act: BankAct) {
    match act {
        BankAct::Open => open(bot),
    }
}
