//! Bot options (VibeSkua: `IScriptOption`, Options > Game). Defaults match VibeSkua.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum HuntPriority {
    #[default]
    None,
    HighHP,
    LowHP,
    Close,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct BotOptions {
    // Performance & Visuals
    pub lag_killer: bool,
    pub hide_players: bool,
    pub disable_fx: bool,
    pub disable_collisions: bool,
    pub show_fps: bool,
    pub set_fps: i32,
    // Combat & Targeting
    pub aggro_monsters: bool,
    pub aggro_all_monsters: bool,
    pub attack_without_target: bool,
    pub infinite_range: bool,
    pub magnetise: bool,
    pub hunt_delay: i32,
    pub hunt_buffer: i32,
    pub hunt_priority: HuntPriority,
    pub rest_packets: bool,
    pub safe_timings: bool,
    // Drops & Looting
    #[serde(rename = "acceptACDrops")]
    pub accept_ac_drops: bool,
    pub accept_all_drops: bool,
    pub reject_all_drops: bool,
    // Auto-Relogin & Network
    pub relogin_server: String,
    pub auto_relogin: bool,
    pub auto_relogin_any: bool,
    pub retry_relogin: bool,
    pub safe_relogin: bool,
    pub relogin_tries: i32,
    pub relogin_try_delay: i32,
    pub login_timeout: i32,
    /// VibeSkua's "Relogin Watchdog" (Application options): an unexpected disconnect or a
    /// lost connection starts auto-relogin.
    pub relogin_watchdog: bool,
    // Gameplay & Character
    pub private_rooms: bool,
    pub private_number: i32,
    pub skip_cutscenes: bool,
    pub headless_mode: bool,
    pub streamer_mode: bool,
    pub custom_name: String,
    pub custom_guild: String,
    pub walk_speed: i32,
    pub action_delay: i32,
    pub maximum_tries: i32,
    pub join_map_tries: i32,
    pub quest_accept_and_complete_tries: i32,
    pub load_timeout: i32,
    pub disable_death_ads: bool,
    pub use_function_based_skills: bool,
}

impl Default for BotOptions {
    fn default() -> Self {
        Self {
            lag_killer: false,
            hide_players: false,
            disable_fx: false,
            disable_collisions: false,
            show_fps: false,
            // VibeSkua: 30 (skua.swf's rate); the game itself is made for 24.
            set_fps: crate::settings::GAME_FRAME_RATE,
            aggro_monsters: false,
            aggro_all_monsters: false,
            attack_without_target: false,
            infinite_range: false,
            magnetise: false,
            hunt_delay: 1000,
            hunt_buffer: 1,
            hunt_priority: HuntPriority::None,
            rest_packets: false,
            safe_timings: true,
            accept_ac_drops: false,
            accept_all_drops: false,
            reject_all_drops: false,
            relogin_server: "Twilly".into(),
            auto_relogin: false,
            auto_relogin_any: false,
            retry_relogin: true,
            safe_relogin: false,
            relogin_tries: 5,
            relogin_try_delay: 800,
            login_timeout: 30000,
            relogin_watchdog: true,
            private_rooms: false,
            private_number: 100000,
            skip_cutscenes: false,
            headless_mode: false,
            streamer_mode: false,
            custom_name: String::new(),
            custom_guild: String::new(),
            walk_speed: 8,
            action_delay: 800,
            maximum_tries: 10,
            join_map_tries: 3,
            quest_accept_and_complete_tries: 30,
            load_timeout: 30000,
            disable_death_ads: false,
            use_function_based_skills: false,
        }
    }
}

impl BotOptions {
    /// Options a stopping script turns off again (VibeSkua `ScriptOption.ScriptStopped`).
    pub fn reset_after_script(&mut self) {
        self.auto_relogin = false;
        self.lag_killer = false;
        self.aggro_all_monsters = false;
        self.aggro_monsters = false;
        self.skip_cutscenes = false;
    }

    /// Reads one option by its camelCase name (as scripts use it: `bot.options.get("lagKiller")`).
    pub fn get(&self, name: &str) -> Option<serde_json::Value> {
        let value = serde_json::to_value(self).ok()?;
        value.get(name).cloned()
    }

    /// Writes one option by its camelCase name. Unknown names and wrong types are errors.
    pub fn set(&mut self, name: &str, new: serde_json::Value) -> Result<(), String> {
        let mut value = serde_json::to_value(&*self).map_err(|e| e.to_string())?;
        let obj = value.as_object_mut().ok_or("options aren't an object")?;
        if !obj.contains_key(name) {
            return Err(format!("unknown option `{name}`"));
        }
        obj.insert(name.to_owned(), new);
        *self = serde_json::from_value(value).map_err(|e| format!("option `{name}`: {e}"))?;
        Ok(())
    }
}
