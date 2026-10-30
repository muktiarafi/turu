//! Application settings (VibeSkua: `ClientSettings`, Options > Application), saved in
//! `settings.json`. The bot's own options (Options > Game) are in [`crate::options`].

use serde::{Deserialize, Serialize};

/// The frame rate AQW's `Game.swf` is made for (its SWF header says 24).
pub const GAME_FRAME_RATE: i32 = 24;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AppSettings {
    // General & Performance
    pub show_username_in_title: bool,
    /// VibeSkua's "Client Animation Frame-rate": the frame rate the client starts with, until
    /// Options > Game > Set FPS applies in game.
    pub animation_frame_rate: i32,
    // Auto Updates & Sync (stored only: Turu doesn't download updates yet)
    pub check_bot_scripts_updates: bool,
    pub auto_update_bot_scripts: bool,
    pub check_advance_skill_sets_updates: bool,
    pub auto_update_advance_skill_sets_updates: bool,
    pub check_junk_items_updates: bool,
    pub auto_update_junk_items: bool,
    // Webhooks & Alerts (`crate::webhooks`)
    pub discord_webhook_url: String,
    pub nexus_webhook_url: String,
    pub webhook_notify_started: bool,
    pub webhook_notify_stopped: bool,
    pub webhook_notify_crashed: bool,
    pub webhook_notify_relogged: bool,
    pub webhook_notify_item_drops: bool,
    /// Comma-separated item names; a drop whose name contains one is announced.
    pub webhook_notify_item_drops_list: String,
    pub webhook_notify_health_ping: bool,
    /// Minutes between health pings.
    pub webhook_ping_interval: i32,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            show_username_in_title: true,
            animation_frame_rate: GAME_FRAME_RATE,
            check_bot_scripts_updates: true,
            auto_update_bot_scripts: true,
            check_advance_skill_sets_updates: true,
            auto_update_advance_skill_sets_updates: true,
            check_junk_items_updates: true,
            auto_update_junk_items: true,
            discord_webhook_url: String::new(),
            nexus_webhook_url: String::new(),
            webhook_notify_started: true,
            webhook_notify_stopped: true,
            webhook_notify_crashed: true,
            webhook_notify_relogged: true,
            webhook_notify_item_drops: false,
            webhook_notify_item_drops_list: "Unidentified 34, Void Aura".into(),
            webhook_notify_health_ping: false,
            webhook_ping_interval: 10,
        }
    }
}
