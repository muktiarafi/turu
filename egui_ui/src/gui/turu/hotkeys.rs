//! Options > HotKeys in egui: the key presses that run the bound actions, and what an action needs
//! of the egui UI (a window, the bank, Auto, the Script Loader's button). The catalog, the
//! bindings, editing them and running an action are the model's (`models::hotkeys`); the window is
//! `hotkeys_window.rs`.

use super::TabUi;
use super::auto;
use egui::{Context, Key, Modifiers};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::models::hotkeys::{self as model, Needs};
pub use crate::models::hotkeys::{ACTIONS, Action, Binding, Gesture, Group, STORE, find, hint};

/// The last frame the window waited for a key to bind: the press is the window's then, not an
/// action's. (A frame number rather than a flag, so a tab switched away mid-wait can't leave
/// the hotkeys off.)
static CAPTURE_FRAME: AtomicU64 = AtomicU64::new(u64::MAX);

/// Called each frame the window waits for a key.
pub fn mark_capturing(ctx: &Context) {
    CAPTURE_FRAME.store(ctx.cumulative_frame_nr(), Ordering::Relaxed);
}

fn capturing(ctx: &Context) -> bool {
    let frame = CAPTURE_FRAME.load(Ordering::Relaxed);
    frame != u64::MAX && ctx.cumulative_frame_nr().saturating_sub(frame) <= 1
}

/// A copy of the bindings.
pub fn bindings() -> Vec<Binding> {
    STORE.bindings()
}

/// The gesture of an egui key press, if the key can be bound.
pub fn of_press(key: Key, modifiers: Modifiers) -> Option<Gesture> {
    Gesture::of_press(key.name(), modifiers.command, modifiers.shift, modifiers.alt)
}

/// Whether an egui press is `gesture`: the same modifiers, no more (macOS's Control, not Cmd,
/// must be up).
fn matches(gesture: Gesture, key: Key, modifiers: Modifiers) -> bool {
    let stray_ctrl = cfg!(target_os = "macos") && modifiers.ctrl;
    gesture.matches(
        key.name(),
        modifiers.command,
        modifiers.shift,
        modifiers.alt,
        stray_ctrl,
    )
}

/// The actions whose keys were pressed this frame; their key presses are taken from egui. A key
/// that types (no Ctrl or Alt) or moves a caret is left alone while a Turu text box has focus.
fn pressed(ctx: &Context) -> Vec<&'static str> {
    if !crate::host::host().has_hotkeys() || capturing(ctx) {
        return Vec::new();
    }
    let typing = ctx.egui_wants_keyboard_input();
    let bindings = bindings();
    let mut actions = Vec::new();
    ctx.input_mut(|input| {
        input.events.retain(|event| {
            let egui::Event::Key {
                key,
                pressed: true,
                repeat: false,
                modifiers,
                ..
            } = event
            else {
                return true;
            };
            let hit = bindings.iter().find(|b| {
                b.gesture
                    .is_some_and(|g| matches(g, *key, *modifiers) && !(typing && (g.types() || g.navigates())))
            });
            match hit {
                Some(binding) => {
                    actions.push(binding.action);
                    false
                }
                None => true,
            }
        });
    });
    actions
}

/// Runs the actions whose keys were pressed, for the active tab: the model runs them, and what
/// one needs of the egui UI is done here.
pub(super) fn run_pressed(ctx: &Context, tab: &mut TabUi) {
    let bot = crate::turu::bot();
    for id in pressed(ctx) {
        let Some(action) = find(id) else { continue };
        match model::run(&bot, action) {
            Needs::Nothing => {}
            Needs::ToggleScript => super::script_loader::toggle_script(),
            Needs::Window { key, title, pick } => {
                tab.windows.open(key);
                if pick && crate::host::host().has_file_dialogs() {
                    tab.script_loader.pick_script(ctx);
                }
                raise(ctx, title);
            }
            Needs::OpenBank => tab.nav.open_bank(),
            Needs::ToggleAuto { hunt } => tab.nav.toggle_auto(if hunt {
                auto::AutoMode::Hunt
            } else {
                auto::AutoMode::Attack
            }),
        }
    }
}

/// Puts the window titled `title` over the others.
fn raise(ctx: &Context, title: &str) {
    let id = egui::Id::new(("turu_window", title));
    ctx.move_to_top(egui::LayerId::new(egui::Order::Middle, id));
    ctx.request_repaint();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The model keeps keys by egui's names, without egui: its table must stay egui's.
    #[test]
    fn the_models_key_names_are_eguis() {
        let egui_names: Vec<&str> = Key::ALL.iter().map(|k| k.name()).collect();
        assert_eq!(model::KEY_NAMES, egui_names.as_slice());
    }

    #[test]
    fn egui_presses_match_exact_modifiers() {
        let g = Gesture::parse("Shift+F5").unwrap();
        assert!(matches(g, Key::F5, Modifiers::SHIFT));
        assert!(!matches(g, Key::F5, Modifiers::NONE));
        assert!(!matches(g, Key::F5, Modifiers::SHIFT | Modifiers::ALT));
        assert_eq!(of_press(Key::Escape, Modifiers::NONE), None);
        assert_eq!(
            of_press(Key::F6, Modifiers::COMMAND).map(Gesture::text).as_deref(),
            Some("Ctrl+F6")
        );
    }
}
