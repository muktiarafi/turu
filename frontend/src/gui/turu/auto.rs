//! The "Auto" popup's Auto Attack and Auto Hunt for the active tab; the loop is
//! `turu_bot::auto` (VibeSkua `ScriptAuto`).

use turu_bot::Bot;
pub use turu_bot::auto::{AutoMode, parse_map_ids};

/// Whether the bot's Auto Attack or Auto Hunt runs.
pub fn running(bot: &Bot) -> bool {
    bot.auto.is_running()
}

/// Starts Auto Attack or Auto Hunt on the active tab with `class`'s skill set in `class_mode`.
pub fn start(mode: AutoMode, class: Option<String>, class_mode: String, map_ids: Vec<i64>) {
    turu_bot::auto::start(&crate::turu::bot(), mode, class, class_mode, map_ids);
}

/// Stops the active tab's Auto Attack/Hunt (no-op when it isn't running).
pub fn stop() {
    turu_bot::auto::stop(&crate::turu::bot());
}
