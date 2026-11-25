//! Options > Application (VibeSkua's `ApplicationOptionsView`): Turu's own settings in five
//! sections, every one saved at once: `settings.json` (`AppSettings`); Relogin Try Delay and
//! Relogin Watchdog, which are bot options, just those two fields of `options.json`; and what the
//! frontend keeps (Frame Interpolation, Graphics, Safe Drawing, ..., `Host`). The egui window
//! (`gui/turu/application_options.rs`) lays out [`entries`]; a native shell reads [`view`] and
//! sends [`set`] and [`act`]. The Web Monitor's links, phone QR code and keys are only in the full
//! window ([`KindView::Elsewhere`]).

use super::form::{KindView, OptionValue, OptionView, SectionView};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use turu_bot::Bot;
use turu_bot::options::BotOptions;
use turu_bot::settings::AppSettings;
use turu_bot::webhooks::Notice;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Section {
    General,
    Updates,
    Webhooks,
    Monitor,
    Report,
}

impl Section {
    pub const ALL: [Section; 5] = [
        Self::General,
        Self::Updates,
        Self::Webhooks,
        Self::Monitor,
        Self::Report,
    ];

    pub fn chip(self) -> &'static str {
        match self {
            Self::General => "General",
            Self::Updates => "Updates",
            Self::Webhooks => "Webhooks",
            Self::Monitor => "Monitor",
            Self::Report => "Report",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::General => "General & Performance",
            Self::Updates => "Auto Updates & Sync",
            Self::Webhooks => "Webhooks & Alerts",
            Self::Monitor => "Web Monitor",
            Self::Report => "Report to Another Monitor",
        }
    }
}

pub enum Field {
    Bool(fn(&mut AppSettings) -> &mut bool),
    /// A number with its suffix, smallest and largest value.
    Int(fn(&mut AppSettings) -> &mut i32, &'static str, i32, i32),
    Text(fn(&mut AppSettings) -> &mut String),
    /// Text shown as dots (a key).
    Secret(fn(&mut AppSettings) -> &mut String),
    /// Stored, but nothing reads it yet: shown greyed.
    Unused(fn(&mut AppSettings) -> &mut bool),
    /// A client update switch: greyed on a nightly build, which doesn't update.
    ClientUpdates(fn(&mut AppSettings) -> &mut bool),
    OptionBool(fn(&mut BotOptions) -> &mut bool),
    OptionInt(fn(&mut BotOptions) -> &mut i32, &'static str),
    Test,
    ClearFlashCache,
    /// Where the monitor is served, with Open, Copy Link and Copy Key.
    MonitorLinks,
    /// Show / Hide the phone link's QR code.
    PhoneQr,
    RegenerateMonitorKey,
    /// Copy Report Settings and Regenerate, for the push key.
    PushKey,
    /// `send_crash_reports` (builds with Sentry only), applied at once.
    CrashReports,
    /// Check Now, for client updates.
    CheckClientUpdates,
    /// Mugetsu's frame interpolation, applied and saved by the frontend (`Host`).
    FrameInterpolation,
    /// The graphics API, chosen by the frontend, which restarts (`Host::graphics_choices`).
    Graphics,
    /// Safe Drawing, saved by the frontend for the next start (`Host::safe_drawing`).
    SafeDrawing,
    /// The FPS counter (`Host::show_fps`).
    ShowFps,
    /// Mugetsu 2.0.0's frame pacing, V-sync or Uncapped (`Host::frame_pacing`).
    FramePacing,
    /// Mugetsu 2.0.0's FPS limit while uncapped (`Host::frame_pacing`).
    FpsLimit,
    /// Mugetsu 2.0.0's Performance mode (`Host::performance`).
    Performance,
    /// Mugetsu 2.0.0's memory limit (`Host::memory_limit_mb`).
    MemoryLimit,
    /// Free Memory Now (`Host::free_memory`).
    FreeMemory,
    /// The window full screen (`Host::full_screen`).
    FullScreen,
    /// Open, for the log's folder (`Host::open_log_folder`).
    OpenLogFolder,
}

pub struct Entry {
    pub section: Section,
    pub label: &'static str,
    pub description: &'static str,
    pub field: Field,
    /// VibeSkua's `RequiresRestart`.
    pub restart: bool,
}

macro_rules! entry {
    ($section:ident, $label:expr, $description:expr, $field:expr) => {
        Entry {
            section: Section::$section,
            label: $label,
            description: $description,
            field: $field,
            restart: false,
        }
    };
}

/// Every setting, in VibeSkua's order, then Turu's own.
pub fn entries() -> Vec<Entry> {
    let mut entries = vibeskua_entries();
    if crate::host::host().uploads_crash_reports() {
        entries.insert(
            1,
            entry!(
                General,
                "Send Crash Reports",
                "When Turu crashes, send a report to its developers so they can fix it: what crashed, the device and system version, Turu's version. No account, password or game data",
                Field::CrashReports
            ),
        );
    }
    if crate::host::host().graphics_choices().is_some() {
        entries.push(Entry {
            restart: true,
            ..entry!(
                General,
                "Graphics",
                "Which graphics API draws the game. Automatic picks the one that works best on this device; try another if the game crashes or draws wrongly. On a computer Turu starts again, every tab with it; on a phone each tab takes it at its next start",
                Field::Graphics
            )
        });
    }
    if crate::host::host().safe_drawing().is_some() {
        entries.push(Entry {
            restart: true,
            ..entry!(
                General,
                "Safe Drawing",
                "Turns off Turu's drawing speed-ups and draws the game the plain way. Try it if the game shows streaks, stripes or garbled colours, or parts of maps go missing. Slower, above all in crowded rooms, so leave it off when the game draws fine. Each tab takes it at its next start",
                Field::SafeDrawing
            )
        });
    }
    // The rest of the game's right-click menu in the window, where there's none (a process tab).
    let platform = crate::host::host();
    if platform.show_fps().is_some() {
        entries.push(entry!(
            General,
            "Show FPS",
            "A small counter in the corner of the game: how many pictures of it are drawn a second. This tab at once, the others from their next start",
            Field::ShowFps
        ));
    }
    entries.push(entry!(
        General,
        "Player HUD",
        "A small panel over the game with the HP and MP of everyone in your cell, you first. Also in Combat > Player HUD",
        Field::Bool(|s| &mut s.show_player_hud)
    ));
    entries.push(entry!(
        General,
        "Target Auras",
        "A small panel over the game with the auras on the monster you attack, who cast each one and the time it has left. Also in Combat > Target Auras",
        Field::Bool(|s| &mut s.show_target_auras)
    ));
    if platform.frame_pacing().is_some() {
        entries.push(entry!(
            General,
            "Frame Pacing",
            "With Frame Interpolation on: V-sync shows the in-between pictures in step with the display. Uncapped sends each one as soon as it's ready, for G-Sync and FreeSync monitors (on others the picture tears). This tab from its next game frame, the others from their next start",
            Field::FramePacing
        ));
        entries.push(entry!(
            General,
            "FPS Limit",
            "With Frame Pacing on Uncapped: the most pictures a second. Screen is just under the monitor's refresh rate, where a G-Sync or FreeSync monitor still follows. This tab from its next game frame, the others from their next start",
            Field::FpsLimit
        ));
    }
    if platform.performance().is_some() {
        entries.push(entry!(
            General,
            "Performance Mode",
            "Draws the game with fewer pixels (Balanced 85%, Fast 70% of its width and height) and stretches the picture: a little softer, and faster on laptops and built-in graphics. This tab at once, the others from their next start",
            Field::Performance
        ));
    }
    if platform.memory_limit_mb().is_some() {
        entries.push(entry!(
            General,
            "Memory Limit",
            "Over it, this tab frees what the game isn't using (shapes not drawn for a while, spare textures, free heap), at most every 20 seconds. It counts what Task Manager shows for the tab's process; a room that needs more than the limit can't be held under it. This tab at once, the others from their next start",
            Field::MemoryLimit
        ));
        entries.push(entry!(
            General,
            "Free Memory Now",
            "Frees at once what this tab's game isn't using, loaded item files no player on screen uses included",
            Field::FreeMemory
        ));
    }
    if platform.full_screen().is_some() {
        entries.push(entry!(
            General,
            "Full Screen",
            "Turu's window fills the screen. Esc leaves full screen",
            Field::FullScreen
        ));
    }
    if platform.opens_log_folder() {
        entries.push(entry!(
            General,
            "Log Folder",
            "Opens the folder with Turu's log (ruffle.log), to send with a bug report",
            Field::OpenLogFolder
        ));
    }
    if crate::host::host().limits_tabs() {
        entries.push(entry!(
            General,
            "Tab Limit",
            TAB_LIMIT_ABOUT,
            Field::Int(|s| &mut s.tab_limit, "tabs", 1, crate::models::prompts::tab_max())
        ));
    }
    entries
}

/// Tab Limit's description, and what it says instead while above [`TAB_LIMIT_SAFE`].
const TAB_LIMIT_ABOUT: &str = "How many tabs (game clients) can be open at once. Each one takes 200-400 MB of memory: on a phone with less of it, Android may stop Turu with more open";
const TAB_LIMIT_ABOVE_SAFE: &str = "More than 4 tabs: when the phone runs short of memory, Android closes some, and each comes back to log in and start its script again. Each one takes 200-400 MB. Lower it if tabs keep restarting";
/// The Tab Limit most phones hold (its default): above it, the description warns, and Android's
/// Options asks first (`AppOptionsPage.kt`).
pub const TAB_LIMIT_SAFE: i32 = 4;

fn vibeskua_entries() -> Vec<Entry> {
    vec![
        entry!(
            General,
            "Show Username in Title",
            "Whether to display logged-in character name in window title",
            Field::Bool(|s| &mut s.show_username_in_title)
        ),
        entry!(
            General,
            "Grandpa Mode",
            "Larger, easier-to-read text in logs and packet output",
            Field::Bool(|s| &mut s.grandpa_mode)
        ),
        Entry {
            restart: true,
            ..entry!(
                General,
                "Client Animation Frame-rate",
                "The frame rate the game client starts with; the game's own FPS setting, or Set FPS (Game options), takes over in game. 24 (the default) starts it at 30, which looks smoother on a 60 Hz screen",
                Field::Int(|s| &mut s.animation_frame_rate, "FPS", 1, i32::MAX)
            )
        },
        entry!(
            General,
            "Frame Interpolation",
            "Draws in-between pictures at the display's refresh rate, so movement looks smoother. Costs more CPU and GPU time (and battery on a phone), and the game shows up to one frame later. Also in the game's right-click menu on a computer",
            Field::FrameInterpolation
        ),
        entry!(
            General,
            "Relogin Try Delay",
            "Delay in milliseconds between login attempts when disconnected",
            Field::OptionInt(|o| &mut o.relogin_try_delay, "ms")
        ),
        entry!(
            General,
            "Relogin Watchdog",
            "Watch for unexpected disconnections, even with no script running, and relog when Auto Relogin (Game Options) is on. Off leaves relogs to kicks and server logouts",
            Field::OptionBool(|o| &mut o.relogin_watchdog)
        ),
        entry!(
            General,
            "Auto-jump on Position Change",
            "Automatically jump when selecting a map cell or pad in the Jump control",
            Field::Bool(|s| &mut s.auto_jump_on_position_change)
        ),
        entry!(
            General,
            "Clear Flash Cache",
            "Deletes the game's saved data (SharedObjects), such as the remembered login. Restart Turu afterwards",
            Field::ClearFlashCache
        ),
        entry!(
            Updates,
            "Check for Client Updates",
            "Whether to check for new Turu versions when launching Turu, and every 6 hours",
            Field::ClientUpdates(|s| &mut s.check_client_updates)
        ),
        entry!(
            Updates,
            "Include Pre-releases",
            "Whether to offer alpha, beta and release-candidate versions too. A pre-release of Turu always offers them",
            Field::ClientUpdates(|s| &mut s.check_client_prereleases)
        ),
        entry!(Updates, "Check for Client Updates Now", "", Field::CheckClientUpdates),
        entry!(
            Updates,
            "Check for Script Updates",
            "Whether to check for scripts updates when launching Turu",
            Field::Bool(|s| &mut s.check_bot_scripts_updates)
        ),
        entry!(
            Updates,
            "Auto Update Scripts",
            "Whether to auto update scripts when launching Turu, needs \"Check for Scripts Updates\" to be true",
            Field::Bool(|s| &mut s.auto_update_bot_scripts)
        ),
        entry!(
            Updates,
            "Check for AdvanceSkill Sets Updates",
            "Whether to check for scripts updates when launching Turu",
            Field::Bool(|s| &mut s.check_advance_skill_sets_updates)
        ),
        entry!(
            Updates,
            "Auto Update AdvanceSkill Sets",
            "Whether to auto update advance skill sets when launching Turu, needs \"Check for AdvanceSkill Sets updates\" to be true",
            Field::Bool(|s| &mut s.auto_update_advance_skill_sets_updates)
        ),
        entry!(
            Updates,
            "Check for Junk Items Updates",
            "Whether to check for junk items updates when launching the Manager",
            Field::Unused(|s| &mut s.check_junk_items_updates)
        ),
        entry!(
            Updates,
            "Auto Update Junk Items",
            "Whether to auto update junk items when launching the Manager, needs \"Check for Junk Items Updates\" to be true",
            Field::Unused(|s| &mut s.auto_update_junk_items)
        ),
        entry!(
            Webhooks,
            "Discord Webhook URL",
            "URL to send Discord notifications and live statistics when scripts start, stop, or crash",
            Field::Text(|s| &mut s.discord_webhook_url)
        ),
        entry!(Webhooks, "Test Discord Webhook", "", Field::Test),
        entry!(
            Webhooks,
            "Notify on Script Started",
            "Send alerts and initial statistics when a script starts running",
            Field::Bool(|s| &mut s.webhook_notify_started)
        ),
        entry!(
            Webhooks,
            "Notify on Script Stopped",
            "Send session summary and statistics when a script finishes or stops",
            Field::Bool(|s| &mut s.webhook_notify_stopped)
        ),
        entry!(
            Webhooks,
            "Notify on Script Crashed",
            "Send alerts when a script crashes with an error",
            Field::Bool(|s| &mut s.webhook_notify_crashed)
        ),
        entry!(
            Webhooks,
            "Notify on Relog",
            "Send alerts when Turu disconnects and relogs",
            Field::Bool(|s| &mut s.webhook_notify_relogged)
        ),
        entry!(
            Webhooks,
            "Notify on Item Drop",
            "Send webhook notifications when a specific item drops",
            Field::Bool(|s| &mut s.webhook_notify_item_drops)
        ),
        entry!(
            Webhooks,
            "Drop List",
            "Comma-separated list of items to notify on drop",
            Field::Text(|s| &mut s.webhook_notify_item_drops_list)
        ),
        entry!(
            Webhooks,
            "Enable Health Pings",
            "Whether to send periodic health and bot statistics pings to Discord",
            Field::Bool(|s| &mut s.webhook_notify_health_ping)
        ),
        entry!(
            Webhooks,
            "Health Ping Interval",
            "Minutes between health pings (e.g. 5, 10, 30)",
            Field::Int(|s| &mut s.webhook_ping_interval, "Min", 1, i32::MAX)
        ),
        entry!(
            Monitor,
            "Enable Web Monitor",
            "Show every tab's bot, live, on a web page: in a browser here, or on your phone",
            Field::Bool(|s| &mut s.monitor_enabled)
        ),
        entry!(
            Monitor,
            "Allow Other Devices",
            "Listen on your network too, for phones and Tailscale Serve (tailscale serve --bg <port>). They need the key; don't forward the port on your router",
            Field::Bool(|s| &mut s.monitor_allow_other_devices)
        ),
        entry!(
            Monitor,
            "Allow Control",
            "The page can start, stop, pause and relog bots, send them to a map and forget offline ones",
            Field::Bool(|s| &mut s.monitor_allow_control)
        ),
        entry!(
            Monitor,
            "Monitor Port",
            "The port the page is served on",
            Field::Int(|s| &mut s.monitor_port, "", 1024, 65535)
        ),
        entry!(Monitor, "Monitor Link", "", Field::MonitorLinks),
        entry!(
            Monitor,
            "Phone QR Code",
            "Scan it with your phone's camera: the link on your network, with the key (needs Allow Other Devices)",
            Field::PhoneQr
        ),
        entry!(
            Monitor,
            "Regenerate Key",
            "Devices given the old key or link lose access",
            Field::RegenerateMonitorKey
        ),
        entry!(
            Monitor,
            "Push Key",
            "Other machines report their bots here with it: Copy Report Settings gives their Report To address and key",
            Field::PushKey
        ),
        entry!(
            Report,
            "Report To",
            "Another Turu's monitor, e.g. https://my-pc.tail1234.ts.net or http://100.101.102.103:9021: this machine's bots show on its page. Empty: off",
            Field::Text(|s| &mut s.monitor_report_to)
        ),
        entry!(
            Report,
            "Push Key",
            "The push key from that monitor's Copy Report Settings",
            Field::Secret(|s| &mut s.monitor_report_key)
        ),
        entry!(
            Report,
            "Machine Name",
            "This machine's name on that page; empty: the computer's name",
            Field::Text(|s| &mut s.monitor_machine_name)
        ),
        entry!(
            Report,
            "Accept Commands",
            "Run the commands that monitor sends (start, stop, relog, ...). Its log views work either way",
            Field::Bool(|s| &mut s.monitor_accept_commands)
        ),
    ]
}

/// Applies Relogin Try Delay and Relogin Watchdog now and saves them, leaving the other saved
/// options as they are (those wait for the Game window's Save).
pub fn save_relogin_options(bot: &Arc<Bot>, edited: &BotOptions) {
    {
        let mut live = bot.options.write();
        live.relogin_try_delay = edited.relogin_try_delay;
        live.relogin_watchdog = edited.relogin_watchdog;
    }
    let mut saved = bot.paths.load_options().unwrap_or_default();
    saved.relogin_try_delay = edited.relogin_try_delay;
    saved.relogin_watchdog = edited.relogin_watchdog;
    if let Err(e) = bot.paths.save_options(&saved) {
        tracing::warn!("Couldn't save the bot options: {e}");
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct AppOptionsView {
    pub sections: Vec<SectionView>,
    pub options: Vec<OptionView>,
    /// The last action's outcome ("Cleared ...", "Checking for updates…"), and whether it went
    /// wrong.
    pub note: Option<String>,
    pub failed: bool,
}

/// What [`act`] did last, per process (the window's own notes are the egui window's).
static NOTE: std::sync::Mutex<Option<(String, bool)>> = std::sync::Mutex::new(None);

fn note(text: impl Into<String>, failed: bool) {
    *NOTE.lock().unwrap_or_else(|e| e.into_inner()) = Some((text.into(), failed));
}

/// The choices a platform-backed option offers: (key, words).
fn choices(field: &Field) -> Option<Vec<(String, String)>> {
    let platform = crate::host::host();
    let pairs = |list: &[(i64, &str)]| list.iter().map(|(v, w)| (v.to_string(), (*w).to_owned())).collect();
    Some(match field {
        Field::Graphics => platform
            .graphics_choices()?
            .0
            .into_iter()
            .map(|(value, name)| (value.to_owned(), name.to_owned()))
            .collect(),
        Field::FramePacing => pairs(&[(0, "V-sync"), (1, "Uncapped")]),
        Field::FpsLimit => {
            let limit = platform.frame_pacing().map_or(0, |(_, limit)| limit);
            let mut list: Vec<(String, String)> =
                pairs(&[(0, "Off"), (1, "Screen"), (60, "60"), (120, "120"), (144, "144")]);
            if !list.iter().any(|(v, _)| *v == limit.to_string()) {
                list.push((limit.to_string(), limit.to_string()));
            }
            list
        }
        Field::Performance => pairs(&[(0, "Off"), (1, "Balanced"), (2, "Fast")]),
        Field::MemoryLimit => pairs(&[(0, "Off"), (1024, "1 GB"), (2048, "2 GB"), (3072, "3 GB")]),
        _ => return None,
    })
}

fn kind(field: &Field) -> KindView {
    let action = |label| KindView::Action { label, login: false };
    match field {
        Field::Bool(_) | Field::Unused(_) | Field::ClientUpdates(_) | Field::OptionBool(_) => KindView::Switch,
        Field::FrameInterpolation | Field::SafeDrawing | Field::ShowFps | Field::FullScreen | Field::CrashReports => {
            KindView::Switch
        }
        Field::Int(_, unit, min, max) => KindView::Number {
            min: *min,
            max: Some(*max),
            unit: (!unit.is_empty()).then_some(*unit),
        },
        Field::OptionInt(_, unit) => KindView::Number {
            min: 0,
            max: None,
            unit: Some(*unit),
        },
        Field::Text(_) => KindView::Text { hint: "" },
        Field::Secret(_) => KindView::Secret,
        Field::Graphics | Field::FramePacing | Field::FpsLimit | Field::Performance | Field::MemoryLimit => {
            KindView::Choice {
                choices: choices(field).unwrap_or_default(),
            }
        }
        Field::Test => action("Test"),
        Field::ClearFlashCache => action("Clear"),
        Field::CheckClientUpdates => action("Check Now"),
        Field::FreeMemory => action("Free"),
        Field::OpenLogFolder => action("Open"),
        Field::MonitorLinks | Field::PhoneQr | Field::RegenerateMonitorKey | Field::PushKey => KindView::Elsewhere,
    }
}

/// An option's value now.
fn value(field: &Field, settings: &AppSettings, options: &BotOptions) -> OptionValue {
    let platform = crate::host::host();
    let (mut settings, mut options) = (settings.clone(), options.clone());
    match field {
        Field::Bool(access) | Field::Unused(access) | Field::ClientUpdates(access) => {
            OptionValue::Switch(*access(&mut settings))
        }
        Field::OptionBool(access) => OptionValue::Switch(*access(&mut options)),
        Field::Int(access, ..) => OptionValue::Number(i64::from(*access(&mut settings))),
        Field::OptionInt(access, _) => OptionValue::Number(i64::from(*access(&mut options))),
        Field::Text(access) | Field::Secret(access) => OptionValue::Text(access(&mut settings).clone()),
        #[cfg(feature = "ruffle")]
        Field::FrameInterpolation => OptionValue::Switch(ruffle_core::frame_interpolation::enabled()),
        #[cfg(not(feature = "ruffle"))]
        Field::FrameInterpolation => OptionValue::Switch(false),
        Field::SafeDrawing => OptionValue::Switch(platform.safe_drawing() == Some(true)),
        Field::ShowFps => OptionValue::Switch(platform.show_fps() == Some(true)),
        Field::FullScreen => OptionValue::Switch(platform.full_screen() == Some(true)),
        Field::CrashReports => OptionValue::Switch(settings.send_crash_reports == Some(true)),
        Field::Graphics => OptionValue::Text(
            platform
                .graphics_choices()
                .map(|(_, saved)| saved.to_owned())
                .unwrap_or_default(),
        ),
        Field::FramePacing => OptionValue::Text(
            if platform.frame_pacing().is_some_and(|(on, _)| on) {
                "1"
            } else {
                "0"
            }
            .into(),
        ),
        Field::FpsLimit => OptionValue::Text(platform.frame_pacing().map_or(0, |(_, limit)| limit).to_string()),
        Field::Performance => OptionValue::Text(platform.performance().unwrap_or(0).to_string()),
        Field::MemoryLimit => OptionValue::Text(platform.memory_limit_mb().unwrap_or(0).to_string()),
        _ => OptionValue::None,
    }
}

/// Whether the option can be changed here now (nightly builds don't update, unused ones wait).
fn usable(field: &Field) -> bool {
    match field {
        Field::Unused(_) => false,
        Field::ClientUpdates(_) => !turu_bot::updates::this_is_nightly(),
        Field::CheckClientUpdates => {
            !turu_bot::updates::this_is_nightly()
                && !turu_bot::updates::state().checking
                && (!crate::turu::launched_by_army() || crate::turu::updates_from_shell())
        }
        Field::ClearFlashCache => crate::turu::save_directory().is_some(),
        _ => true,
    }
}

/// Application Options' view now.
pub fn view(bot: &Arc<Bot>) -> AppOptionsView {
    let settings = bot.settings.read().clone();
    let options = bot.options.read().clone();
    let note = NOTE.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let description = |e: &Entry| match &e.field {
        Field::Test if settings.discord_webhook_url.trim().is_empty() => "Set the Discord Webhook URL first",
        Field::Int(..) if e.description == TAB_LIMIT_ABOUT && settings.tab_limit > TAB_LIMIT_SAFE => {
            TAB_LIMIT_ABOVE_SAFE
        }
        _ => e.description,
    };
    AppOptionsView {
        sections: Section::ALL
            .iter()
            .map(|s| SectionView {
                chip: s.chip(),
                title: s.title(),
            })
            .collect(),
        options: entries()
            .iter()
            .map(|e| {
                let v = value(&e.field, &settings, &options);
                OptionView {
                    label: e.label,
                    description: description(e),
                    section: e.section.chip(),
                    group: "",
                    kind: kind(&e.field),
                    saved: v.clone(),
                    value: v,
                    differs: false,
                    applies: !matches!(e.field, Field::Test) || !settings.discord_webhook_url.trim().is_empty(),
                    unused: !usable(&e.field),
                    restart: e.restart,
                }
            })
            .collect(),
        failed: note.as_ref().is_some_and(|(_, failed)| *failed),
        note: note.map(|(text, _)| text),
    }
}

/// Sets the option labelled `label` (in `section`: two are called "Push Key") to `value`, saving
/// it as the egui window does.
pub fn set(bot: &Arc<Bot>, section: &str, label: &str, value: &Value) -> Result<(), String> {
    let list = entries();
    let entry = list
        .iter()
        .find(|e| e.label == label && e.section.chip() == section)
        .ok_or_else(|| format!("no option named {label}"))?;
    if !usable(&entry.field) {
        return Err(format!("{label} can't be changed here"));
    }
    let wrong = || format!("not a value for {label}: {value}");
    let platform = crate::host::host();
    let mut settings = bot.settings.read().clone();
    let mut options = bot.options.read().clone();
    let (mut settings_changed, mut options_changed) = (false, false);
    let number = || value.as_i64().ok_or_else(wrong);
    let key = || value.as_str().map(str::to_owned).ok_or_else(wrong);
    match &entry.field {
        Field::Bool(access) | Field::ClientUpdates(access) => {
            *access(&mut settings) = value.as_bool().ok_or_else(wrong)?;
            settings_changed = true;
        }
        Field::OptionBool(access) => {
            *access(&mut options) = value.as_bool().ok_or_else(wrong)?;
            options_changed = true;
        }
        Field::Int(access, _, min, max) => {
            *access(&mut settings) = i32::try_from(number()?).map_err(|_| wrong())?.clamp(*min, *max);
            settings_changed = true;
        }
        Field::OptionInt(access, _) => {
            *access(&mut options) = i32::try_from(number()?).map_err(|_| wrong())?.max(0);
            options_changed = true;
        }
        Field::Text(access) | Field::Secret(access) => {
            *access(&mut settings) = key()?;
            settings_changed = true;
        }
        Field::FrameInterpolation => platform.set_frame_interpolation(value.as_bool().ok_or_else(wrong)?),
        Field::SafeDrawing => platform.set_safe_drawing(value.as_bool().ok_or_else(wrong)?),
        Field::ShowFps => platform.set_show_fps(value.as_bool().ok_or_else(wrong)?),
        Field::FullScreen => platform.set_full_screen(value.as_bool().ok_or_else(wrong)?),
        Field::CrashReports => {
            let on = value.as_bool().ok_or_else(wrong)?;
            settings.send_crash_reports = Some(on);
            platform.set_crash_uploads(on);
            settings_changed = true;
        }
        Field::Graphics => {
            let choice = key()?;
            let (list, _) = platform.graphics_choices().ok_or_else(wrong)?;
            let (value, _) = list.into_iter().find(|(v, _)| *v == choice).ok_or_else(wrong)?;
            platform.set_graphics(value);
        }
        Field::FramePacing => platform.set_uncapped(key()? == "1"),
        Field::FpsLimit => platform.set_fps_limit(key()?.parse().map_err(|_| wrong())?),
        Field::Performance => platform.set_performance(key()?.parse().map_err(|_| wrong())?),
        Field::MemoryLimit => platform.set_memory_limit_mb(key()?.parse().map_err(|_| wrong())?),
        _ => return Err(format!("{label} is an action, not an option")),
    }
    if settings_changed {
        crate::turu::apply_monitor(&settings);
        *bot.settings.write() = settings;
        bot.save_settings();
    }
    if options_changed {
        save_relogin_options(bot, &options);
    }
    Ok(())
}

/// Runs the action labelled `label`: Test (the webhook), Clear (the Flash cache; the shell asks
/// first), Check Now, Free (memory), Open (the log's folder).
pub fn run(bot: &Arc<Bot>, label: &str) -> Result<(), String> {
    let list = entries();
    let entry = list
        .iter()
        .find(|e| e.label == label)
        .ok_or_else(|| format!("no option named {label}"))?;
    if !usable(&entry.field) {
        return Err(format!("{label} can't run now"));
    }
    match &entry.field {
        Field::Test => {
            if bot.settings.read().discord_webhook_url.trim().is_empty() {
                return Err("Set the Discord Webhook URL first".into());
            }
            bot.webhooks.send(Notice::Test);
            note("Sent a test message to the webhook.", false);
        }
        Field::ClearFlashCache => {
            let dir = crate::turu::save_directory().ok_or("No SharedObjects folder")?;
            match std::fs::remove_dir_all(dir) {
                Ok(()) => note(format!("Cleared {}. Restart Turu.", dir.display()), false),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => note("The Flash cache is already empty.", false),
                Err(e) => note(format!("Couldn't clear {}: {e}", dir.display()), true),
            }
        }
        Field::CheckClientUpdates => {
            crate::turu::check_client_updates_now();
            note("Checking for updates…", false);
        }
        Field::FreeMemory => crate::host::host().free_memory(),
        Field::OpenLogFolder => crate::host::host().open_log_folder(),
        _ => return Err(format!("{label} isn't an action")),
    }
    Ok(())
}

/// Application Options' actions (`POST /models/app-options/do`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(
    feature = "typegen",
    derive(facet::Facet),
    facet(tag = "action", rename_all = "kebab-case")
)]
#[serde(tag = "action", rename_all = "kebab-case")]
#[repr(C)]
pub enum AppOptionsAct {
    /// Sets the option labelled `label` in the section whose chip is `section` ([`set`]).
    Set {
        section: String,
        label: String,
        value: OptionValue,
    },
    /// Runs the action labelled `label` ([`run`]).
    Run { label: String },
}

pub fn act(bot: &Arc<Bot>, act: AppOptionsAct) -> Result<(), String> {
    match act {
        AppOptionsAct::Set { section, label, value } => set(bot, &section, &label, &value.to_json()),
        AppOptionsAct::Run { label } => run(bot, &label),
    }
}
