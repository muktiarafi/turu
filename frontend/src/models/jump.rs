//! Jump (VibeSkua's `JumpViewModel`, Fast Map Jump): the current map's cells and the chosen cell's
//! pads, picking a cell or a pad (jumping at once with Application Options' Auto-jump on Position
//! Change), Current (where the player is, without jumping), Jump, and the last jump's outcome.
//!
//! Turu: the map SWF says which pads each cell has and where the walk through the map's doors
//! arrives (`map_pads`), so a jump lands on a pad the cell has (VibeSkua corrects the pad after
//! jumping, in `jumpCorrectRoom`).

use super::Control;
use super::tab_state::{PerTab, lock};
use crate::map_pads::MapCells;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use turu_bot::Bot;
use turu_bot::api::dispatch;

/// VibeSkua `MapService.Pads`, in its order: the pads offered when the map's own aren't known.
pub const PADS: [&str; 8] = ["Spawn", "Center", "Left", "Right", "Up", "Down", "Top", "Bottom"];

#[derive(Default)]
struct State {
    cells: Vec<String>,
    cell: Option<String>,
    pad: String,
    /// The current map's file and what its SWF says (empty while it's read or when it can't be).
    map: Option<(String, MapCells)>,
    status: Option<String>,
}

impl State {
    fn map(&self) -> Option<&MapCells> {
        self.map.as_ref().map(|(_, map)| map)
    }
}

static STATE: PerTab<State> = PerTab::new();

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct JumpView {
    /// The current map's cells, as last read ([`refresh`]); empty outside a map.
    pub cells: Vec<String>,
    pub cell: Option<String>,
    /// The chosen pad ("" for none yet).
    pub pad: String,
    /// The pads to offer: the chosen cell's, from the map's SWF, else VibeSkua's list.
    pub pads: Vec<String>,
    /// Picking a cell or a pad jumps at once (Application Options).
    pub auto_jump: bool,
    pub jump: Control,
    /// The last jump's outcome ("Jumped to r2 (Left)", "Not logged in", ...).
    pub status: Option<String>,
}

pub fn view(bot: &Arc<Bot>) -> JumpView {
    let state = STATE.of(bot);
    let state = lock(&state);
    let pads = state
        .cell
        .as_deref()
        .and_then(|cell| state.map()?.pads(cell))
        .map(<[String]>::to_vec)
        .unwrap_or_else(|| PADS.map(str::to_owned).to_vec());
    JumpView {
        cells: state.cells.clone(),
        cell: state.cell.clone(),
        pad: state.pad.clone(),
        pads,
        auto_jump: bot.settings.read().auto_jump_on_position_change,
        jump: Control::when(
            state.cell.is_some() && !state.pad.is_empty(),
            "Select a cell and a pad first",
        ),
        status: state.status.clone(),
    }
}

/// Reads the current map's cells (`map.cells`), and once per map file each cell's pads from the
/// map's SWF. On a bot thread; the view has them once read.
pub fn refresh(bot: &Arc<Bot>) {
    let state = STATE.of(bot);
    crate::turu::spawn_action_for(bot.clone(), "Fast Map Jump cells", move |bot| {
        let cells: Vec<String> = serde_json::from_value(dispatch(bot, "map.cells", &[])?).unwrap_or_default();
        let file = turu_bot::map_items::current_file(bot).ok();
        {
            let mut state = lock(&state);
            if state.cells != cells {
                state.cells = cells;
            }
            let Some(file) = file
                .clone()
                .filter(|f| state.map.as_ref().is_none_or(|(known, _)| known != f))
            else {
                return Ok(());
            };
            state.map = Some((file, MapCells::default()));
        }
        let Some(file) = file else { return Ok(()) };
        let map = turu_bot::map_items::map_swf(bot, &file)
            .and_then(|swf| crate::map_pads::scan(&swf).map_err(turu_bot::BotError::Other))
            .map_err(|e| turu_bot::BotError::Other(format!("cells of {file}: {e}")))?;
        let mut state = lock(&state);
        if let Some((known, slot)) = state.map.as_mut()
            && *known == file
        {
            *slot = map;
        }
        Ok(())
    });
}

/// Picks `cell`: its pad as the map's SWF says, else "Spawn" for Enter, else the chosen pad or
/// "Left"; jumps at once with Auto-jump on.
pub fn select_cell(bot: &Arc<Bot>, cell: String) {
    let state = STATE.of(bot);
    let jump = {
        let mut state = lock(&state);
        let auto = state.map().and_then(|map| map.jump_pad(&cell, None, &state.pad));
        if let Some(pad) = auto {
            state.pad = pad;
        } else if cell == "Enter" {
            state.pad = "Spawn".into();
        } else if state.pad.is_empty() {
            state.pad = "Left".into();
        }
        state.cell = Some(cell.clone());
        state.pad.clone()
    };
    if bot.settings.read().auto_jump_on_position_change {
        jump_to(bot, cell, jump, true);
    }
}

/// Picks `pad`; jumps at once with Auto-jump on and a cell chosen.
pub fn select_pad(bot: &Arc<Bot>, pad: String) {
    let state = STATE.of(bot);
    let target = {
        let mut state = lock(&state);
        let changed = state.pad != pad;
        state.pad = pad.clone();
        state.cell.clone().filter(|_| changed)
    };
    if let Some(cell) = target
        && bot.settings.read().auto_jump_on_position_change
    {
        jump_to(bot, cell, pad, true);
    }
}

/// Current: selects where the player is (Enter / Spawn when not playing), without jumping.
pub fn current(bot: &Arc<Bot>) {
    refresh(bot);
    let state = STATE.of(bot);
    crate::turu::spawn_action_for(bot.clone(), "Fast Map Jump current", move |bot| {
        let playing = dispatch(bot, "player.playing", &[])?.as_bool() == Some(true);
        let (cell, pad) = if playing {
            (dispatch(bot, "player.cell", &[])?, dispatch(bot, "player.pad", &[])?)
        } else {
            (json!("Enter"), json!("Spawn"))
        };
        let mut state = lock(&state);
        state.cell = cell.as_str().filter(|c| !c.is_empty()).map(str::to_owned);
        state.pad = pad.as_str().unwrap_or_default().to_owned();
        Ok(())
    });
}

/// Jump: to the chosen cell and pad.
pub fn jump(bot: &Arc<Bot>) {
    let state = STATE.of(bot);
    let target = {
        let state = lock(&state);
        state
            .cell
            .clone()
            .filter(|_| !state.pad.is_empty())
            .map(|cell| (cell, state.pad.clone()))
    };
    if let Some((cell, pad)) = target {
        jump_to(bot, cell, pad, false);
    }
}

/// VibeSkua `MapService.Jump`: only when logged in, without auto-correcting the pad. With `auto`
/// (a cell picked), the pad is where the walk from the player's cell arrives, when the SWF says.
fn jump_to(bot: &Arc<Bot>, cell: String, pad: String, auto: bool) {
    let state = STATE.of(bot);
    lock(&state).status = Some(format!("Jumping to {cell} ({pad})..."));
    crate::turu::spawn_action_for(bot.clone(), "Fast Map Jump", move |bot| {
        let status = if dispatch(bot, "player.loggedIn", &[])?.as_bool() != Some(true) {
            "Not logged in".to_owned()
        } else {
            let mut pad = pad;
            if auto {
                let from = dispatch(bot, "player.cell", &[])?;
                let mut state = lock(&state);
                let walked = state.map().and_then(|map| map.jump_pad(&cell, from.as_str(), &pad));
                if let Some(walked) = walked.filter(|walked| *walked != pad) {
                    state.pad.clone_from(&walked);
                    state.status = Some(format!("Jumping to {cell} ({walked})..."));
                    pad = walked;
                }
            }
            match dispatch(bot, "map.jump", &[json!(cell), json!(pad), json!(false)]) {
                Ok(_) => format!("Jumped to {cell} ({pad})"),
                Err(e) => format!("Jump failed: {e}"),
            }
        };
        lock(&state).status = Some(status);
        Ok(())
    });
}

/// Jump's actions (`POST /models/jump/do`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(
    feature = "typegen",
    derive(facet::Facet),
    facet(tag = "action", rename_all = "kebab-case")
)]
#[serde(tag = "action", rename_all = "kebab-case")]
#[repr(C)]
pub enum JumpAct {
    /// Reads the map's cells and pads again.
    Refresh,
    Cell {
        cell: String,
    },
    Pad {
        pad: String,
    },
    /// Picks the cell and pad the player is in.
    Current,
    Jump,
}

pub fn act(bot: &Arc<Bot>, act: JumpAct) {
    match act {
        JumpAct::Refresh => refresh(bot),
        JumpAct::Cell { cell } => select_cell(bot, cell),
        JumpAct::Pad { pad } => select_pad(bot, pad),
        JumpAct::Current => current(bot),
        JumpAct::Jump => jump(bot),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Before a cell is read: VibeSkua's pads, and Jump off with its reason. Picking a cell picks
    /// a pad (Spawn for Enter, else Left) without a map's SWF; another tab keeps its own.
    #[test]
    fn a_cell_picks_its_pad_and_each_tab_keeps_its_own() {
        let bot = Bot::new();
        let view = view(&bot);
        assert_eq!(view.pads, PADS.map(str::to_owned).to_vec());
        assert_eq!(view.jump, Control::off("Select a cell and a pad first"));
        select_cell(&bot, "Enter".into());
        assert_eq!(super::view(&bot).pad, "Spawn");
        select_cell(&bot, "r2".into());
        let view = super::view(&bot);
        assert_eq!((view.cell.as_deref(), view.pad.as_str()), (Some("r2"), "Spawn"));
        assert!(view.jump.enabled);
        let other = Bot::new();
        assert_eq!(super::view(&other).cell, None);
    }
}
