//! The "Auto" popup's Auto Attack and Auto Hunt for the active tab; the loop is
//! `turu_bot::auto` (VibeSkua `ScriptAuto`).

use turu_bot::Bot;
pub use turu_bot::auto::AutoMode;

/// Whether the bot's Auto Attack or Auto Hunt runs.
pub fn running(bot: &Bot) -> bool {
    bot.auto.is_running()
}

/// Stops the active tab's Auto Attack/Hunt (no-op when it isn't running).
pub fn stop() {
    turu_bot::auto::stop(&crate::turu::bot());
}
