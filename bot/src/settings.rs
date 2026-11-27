//! Application settings (VibeSkua: `ClientSettings`, Options > Application), saved in
//! `settings.json`. The bot's own options (Options > Game) are in [`crate::options`].

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// The frame rate turu.swf runs at (its SWF header, `host/src/main.rs`), as VibeSkua's skua.swf
/// did, and so the client's starting rate by default ([`AppSettings::starting_frame_rate`]).
/// AQW's `Game.swf` says 24 in its header, but 30 shows each picture for the same time on a 60 or
/// 90 Hz screen; 24 alternates two and three refreshes a picture at 60 Hz, a judder plain on a
/// phone.
pub const HOST_FRAME_RATE: i32 = 30;

/// What `animationFrameRate`, the setting before [`AppSettings::starting_frame_rate`], held
/// for turu.swf's own rate: its default, which every settings file saved with it holds.
const OLD_DEFAULT_FRAME_RATE: i64 = 24;

// The derived impls are `AppSettings::serialize` and `AppSettings::deserialize`; the traits'
// (below) carry over the setting a file saved before `startingFrameRate`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self", default, rename_all = "camelCase")]
pub struct AppSettings {
    // General & Performance
    pub show_username_in_title: bool,
    /// VibeSkua's "Client Animation Frame-rate": the frame rate the client starts with, until
    /// the game's own FPS setting (or Options > Game > Set FPS) applies in game. 30 by default,
    /// turu.swf's own ([`HOST_FRAME_RATE`]).
    pub starting_frame_rate: i32,
    /// VibeSkua's "Grandpa Mode" (`GrandpaMode`): larger text in the logs and the packet tools.
    pub grandpa_mode: bool,
    // Auto Updates & Sync. The script ones are read at launch (`repo::ScriptRepo::startup`);
    // the skill-set and junk-item ones are stored only.
    pub check_bot_scripts_updates: bool,
    pub auto_update_bot_scripts: bool,
    pub check_advance_skill_sets_updates: bool,
    pub auto_update_advance_skill_sets_updates: bool,
    pub check_junk_items_updates: bool,
    pub auto_update_junk_items: bool,
    /// Check Turu's GitHub releases at launch and every 6 hours (`crate::updates`; VibeSkua's
    /// Manager `CheckClientUpdates`).
    pub check_client_updates: bool,
    /// Offer pre-releases too (VibeSkua's Manager `CheckClientPrereleases`). A pre-release build
    /// gets them anyway: `updates::wants_prereleases`.
    pub check_client_prereleases: bool,
    /// Tools > Junk Items' "Skip sell warning" (VibeSkua `JunkSkipSellWarning`).
    pub junk_skip_sell_warning: bool,
    /// Tools > Junk Items' quantity goals, item ID to quantity (VibeSkua `InventoryGoals`).
    pub inventory_goals: std::collections::BTreeMap<i64, i64>,
    /// Fast Map Jump jumps as soon as a cell or pad is picked (VibeSkua
    /// `AutoJumpOnPositionChange`, Application options, off by default); off, Jump does it.
    pub auto_jump_on_position_change: bool,
    // Webhooks & Alerts (`crate::webhooks`)
    pub discord_webhook_url: String,
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
    // Web Monitor (`crate::monitor`)
    pub monitor_enabled: bool,
    /// Listen on every network interface (other devices, Tailscale), not only this computer.
    pub monitor_allow_other_devices: bool,
    pub monitor_port: i32,
    /// The page can start, stop and relog bots.
    pub monitor_allow_control: bool,
    /// Report this machine's bots to another machine's monitor (`monitor::push`): its address.
    pub monitor_report_to: String,
    /// That monitor's push key (its Copy Report Settings).
    pub monitor_report_key: String,
    /// This machine's name on that page; empty: the computer's name.
    pub monitor_machine_name: String,
    /// Run the commands that monitor sends.
    pub monitor_accept_commands: bool,
    /// Send crash reports to Turu's developers (builds with Sentry: Android's `CrashUploads.kt`
    /// and the desktop's `crash_uploads`, which read this at start). `None` until the user answered the question Turu asks once.
    pub send_crash_reports: Option<bool>,
    // Combat > Stats
    /// The Advanced switch (VibeSkua `StatsAdvancedMode`).
    pub stats_advanced_mode: bool,
    /// The rates' unit: `/hr`, `/min` or `/sec` (VibeSkua `StatsRateUnit`; `stats::RateUnit`).
    pub stats_rate_unit: String,
    /// Combat > Player HUD (Turu): the HP and MP of the players in your cell, over the game.
    pub show_player_hud: bool,
    /// Where it was dragged to: across the room left beside it (0 left, 1 right) and down the
    /// game's height (0 top); `None` on the left, under the game's portrait of you.
    pub player_hud_pos: Option<[f32; 2]>,
    /// Combat > Target Auras (Turu): the auras on the monster you attack and who cast them, over
    /// the game.
    pub show_target_auras: bool,
    /// Where it was dragged to, as [`AppSettings::player_hud_pos`]; `None` on the right, under
    /// the game's portrait of your target.
    pub target_auras_pos: Option<[f32; 2]>,
    // Army Queue
    /// Its "How to Use" guide was dismissed (VibeSkua `ArmyQueueHelpDismissed`).
    pub army_queue_help_dismissed: bool,
    /// Turu (Android): how many tabs (game clients) may be open at once, 1 to 9 (Turu Mas: 4), 4
    /// by default.
    /// Each one takes 200-400 MB, so a phone with less memory may want fewer
    /// (`docs/ANDROID_PROPOSAL.md`, phase 5).
    pub tab_limit: i32,
    /// The side bar (the rail on a touch screen) was hidden with its logo or Menu > Hide Side
    /// Bar: it stays hidden when Turu opens again, until the tab at the left edge brings it back.
    pub side_bar_hidden: bool,
}

impl AppSettings {
    /// The rate the client runs at when nothing else set one ([`Self::starting_frame_rate`]).
    pub fn start_rate(&self) -> i32 {
        self.starting_frame_rate.max(1)
    }

    /// The rate `loadClient` is given, or `None` when it is turu.swf's own already.
    pub fn client_frame_rate(&self) -> Option<i32> {
        let fps = self.start_rate();
        (fps != HOST_FRAME_RATE).then_some(fps)
    }
}

impl Serialize for AppSettings {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        AppSettings::serialize(self, serializer)
    }
}

impl<'de> Deserialize<'de> for AppSettings {
    /// A file without `startingFrameRate` takes `animationFrameRate`, where 24 (its default)
    /// stood for turu.swf's 30 and any other rate was the user's. The old key stays in the file
    /// (`Paths::merge_settings` keeps keys it doesn't know), for an older Turu reading it.
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let mut value = serde_json::Value::deserialize(deserializer)?;
        if let Some(map) = value.as_object_mut()
            && !map.contains_key("startingFrameRate")
            && let Some(old) = map
                .get("animationFrameRate")
                .and_then(serde_json::Value::as_i64)
            && old != OLD_DEFAULT_FRAME_RATE
        {
            map.insert("startingFrameRate".into(), old.into());
        }
        AppSettings::deserialize(value).map_err(serde::de::Error::custom)
    }
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            show_username_in_title: true,
            starting_frame_rate: HOST_FRAME_RATE,
            grandpa_mode: false,
            check_bot_scripts_updates: true,
            auto_update_bot_scripts: true,
            check_advance_skill_sets_updates: true,
            auto_update_advance_skill_sets_updates: true,
            check_junk_items_updates: true,
            auto_update_junk_items: true,
            check_client_updates: true,
            check_client_prereleases: false,
            junk_skip_sell_warning: false,
            inventory_goals: Default::default(),
            auto_jump_on_position_change: false,
            discord_webhook_url: String::new(),
            webhook_notify_started: true,
            webhook_notify_stopped: true,
            webhook_notify_crashed: true,
            webhook_notify_relogged: true,
            webhook_notify_item_drops: false,
            webhook_notify_item_drops_list: "Unidentified 34, Void Aura".into(),
            webhook_notify_health_ping: false,
            webhook_ping_interval: 10,
            monitor_enabled: false,
            monitor_allow_other_devices: false,
            monitor_port: crate::monitor::DEFAULT_PORT as i32,
            monitor_allow_control: true,
            monitor_report_to: String::new(),
            monitor_report_key: String::new(),
            monitor_machine_name: String::new(),
            monitor_accept_commands: true,
            send_crash_reports: None,
            stats_advanced_mode: false,
            stats_rate_unit: "/hr".into(),
            show_player_hud: false,
            player_hud_pos: None,
            show_target_auras: false,
            target_auras_pos: None,
            army_queue_help_dismissed: false,
            tab_limit: 4,
            side_bar_hidden: false,
        }
    }
}

/// A Tools & Helpers > Fast Travel destination (VibeSkua `FastTravelItemViewModel`, saved there
/// as `"name,map,cell,pad"` in `FastTravels`). Turu keeps the list in `FastTravels.json`
/// ([`crate::paths::Paths::fast_travels_file`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FastTravel {
    pub name: String,
    pub map: String,
    pub cell: String,
    pub pad: String,
}

impl FastTravel {
    pub fn new(name: &str, map: &str, cell: &str, pad: &str) -> Self {
        Self {
            name: name.into(),
            map: map.into(),
            cell: cell.into(),
            pad: pad.into(),
        }
    }

    /// VibeSkua `Validate`: every field filled in.
    pub fn is_valid(&self) -> bool {
        [&self.name, &self.map, &self.cell, &self.pad]
            .iter()
            .all(|field| !field.trim().is_empty())
    }

    /// The fields without the spaces around them.
    pub fn trimmed(&self) -> Self {
        Self::new(
            self.name.trim(),
            self.map.trim(),
            self.cell.trim(),
            self.pad.trim(),
        )
    }

    /// VibeSkua's search: the name, map or cell contains `filter` (trimmed), ignoring case.
    pub fn matches(&self, filter: &str) -> bool {
        let filter = filter.trim().to_lowercase();
        filter.is_empty()
            || [&self.name, &self.map, &self.cell]
                .iter()
                .any(|field| field.to_lowercase().contains(&filter))
    }

    /// The list a new client starts with (VibeSkua `ClientSettings.FastTravels`).
    pub fn defaults() -> Vec<Self> {
        vec![
            Self::new("Tercessuinotlim", "tercessuinotlim", "Enter", "Spawn"),
            Self::new("Nulgath", "tercessuinotlim", "Boss2", "Right"),
            Self::new("VHL & Taro", "tercessuinotlim", "Taro", "Left"),
        ]
    }

    /// The saved list; [`Self::defaults`] when there is no file yet. A file that can't be read
    /// is an error (the caller decides whether to replace it).
    pub fn load(path: &std::path::Path) -> std::io::Result<Vec<Self>> {
        match std::fs::read_to_string(path) {
            Ok(text) => serde_json::from_str(&text).map_err(std::io::Error::other),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::defaults()),
            Err(e) => Err(e),
        }
    }

    pub fn save(path: &std::path::Path, list: &[Self]) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let text = serde_json::to_string_pretty(list).map_err(std::io::Error::other)?;
        std::fs::write(path, text)
    }
}

#[cfg(test)]
mod tests {
    use super::{AppSettings, FastTravel, HOST_FRAME_RATE};

    #[test]
    fn the_default_starting_rate_is_turus_swfs_own() {
        let mut settings = AppSettings::default();
        assert_eq!(settings.starting_frame_rate, HOST_FRAME_RATE);
        assert_eq!(settings.client_frame_rate(), None);
        settings.starting_frame_rate = 24;
        assert_eq!(settings.client_frame_rate(), Some(24), "24 is 24 now");
        assert_eq!(settings.start_rate(), 24);
        settings.starting_frame_rate = 0;
        assert_eq!(settings.start_rate(), 1);
    }

    #[test]
    fn the_old_frame_rate_setting_carries_over_unless_it_was_its_default() {
        let rate = |json: &str| {
            serde_json::from_str::<AppSettings>(json)
                .unwrap()
                .starting_frame_rate
        };
        assert_eq!(rate(r#"{"animationFrameRate": 24}"#), HOST_FRAME_RATE);
        assert_eq!(rate(r#"{"animationFrameRate": 60}"#), 60);
        assert_eq!(
            rate(r#"{"animationFrameRate": 60, "startingFrameRate": 24}"#),
            24
        );
        assert_eq!(rate(r#"{"startingFrameRate": 45}"#), 45);
        assert_eq!(rate("{}"), HOST_FRAME_RATE);
        let saved = serde_json::to_value(AppSettings::default()).unwrap();
        assert_eq!(saved["startingFrameRate"], HOST_FRAME_RATE);
        assert!(saved.get("animationFrameRate").is_none());
    }

    #[test]
    fn the_old_host_swf_settings_are_ignored() {
        let old: super::AppSettings = serde_json::from_str(
            r#"{"turuHostSwf": false, "useVibeSkuaHostSwf": true, "tabLimit": 3}"#,
        )
        .unwrap();
        assert_eq!(old.tab_limit, 3);
        let saved = serde_json::to_string(&old).unwrap();
        assert!(
            !saved.contains("HostSwf"),
            "dropped on the next save: {saved}"
        );
    }

    #[test]
    fn fast_travels_start_with_vibeskua_defaults_and_round_trip() {
        let dir = std::env::temp_dir().join(format!("turu-fast-travel-{}", std::process::id()));
        let path = dir.join("FastTravels.json");
        let _ = std::fs::remove_file(&path);
        assert_eq!(FastTravel::load(&path).unwrap(), FastTravel::defaults());
        // A cleared list stays cleared (VibeSkua refills an empty one at launch).
        FastTravel::save(&path, &[]).unwrap();
        assert!(FastTravel::load(&path).unwrap().is_empty());
        let list = vec![FastTravel::new("Ultra Alteon", "ultraalteon", "r2", "Left")];
        FastTravel::save(&path, &list).unwrap();
        assert_eq!(FastTravel::load(&path).unwrap(), list);
        std::fs::write(&path, "not json").unwrap();
        assert!(FastTravel::load(&path).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn fast_travel_search_and_validation() {
        let nulgath = FastTravel::new("Nulgath", "tercessuinotlim", "Boss2", "Right");
        assert!(nulgath.matches("  boss "));
        assert!(nulgath.matches("TERC"));
        assert!(nulgath.matches(""));
        assert!(!nulgath.matches("Right"), "VibeSkua doesn't search the pad");
        assert!(nulgath.is_valid());
        assert!(!FastTravel::new("Nulgath", " ", "Boss2", "Right").is_valid());
    }
}
