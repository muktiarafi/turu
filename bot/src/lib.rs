//! Turu's bot engine: VibeSkua's script API in Rust, driven by TypeScript scripts.
//!
//! Start at [`Bot`]. Architecture: docs/ARCHITECTURE.md.

pub mod accounts;
pub mod api;
pub mod army;
pub mod auto;
mod background;
pub mod butler;
pub mod capture;
pub mod cbo;
pub mod control;
pub mod control_api;
pub mod cosmetics;
pub mod error;
pub mod events;
pub mod flash;
pub mod gear;
pub mod grabber;
pub mod host;
pub mod junk;
pub mod loadouts;
pub mod log;
pub mod map_items;
pub mod map_scripts;
pub mod models;
pub mod monitor;
pub mod notifications;
pub mod options;
pub mod packet_filters;
pub mod parity;
pub mod paths;
pub mod perf;
pub mod player_packets;
pub mod quest_cache;
pub mod quick_scripts;
mod repeat_log;
pub mod repo;
pub mod scheduler;
pub mod script;
pub mod settings;
pub mod skill_editor;
pub mod skills;
pub mod spammer;
pub mod startup;
pub mod state;
pub mod stats;
pub mod updates;
pub mod webhooks;

pub use error::{BotError, BotResult};
pub use host::{FlashArg, FlashHost, HostError};

use control::ScriptControl;
use events::EventBus;
use flash::Flash;
use log::Logs;
use options::BotOptions;
use parking_lot::{Mutex, RwLock};
use settings::AppSettings;
use state::GameState;
use std::sync::Arc;
use std::time::Duration;

/// `skua.swf`, the AS3 host VibeSkua loads (Skua.AS3/skua/bin/skua.swf). It loads the AQW
/// client from game.aq.com and exposes the game to the bot through ExternalInterface.
pub const SKUA_SWF: &[u8] = include_bytes!("../assets/skua.swf");

/// `turu.swf`, Turu's own host SWF (`host/`, docs/HOST_SWF_PROPOSAL.md), still being written: run
/// only with `TURU_HOST_SWF=turu` ([`HostSwf`]).
pub const TURU_SWF: &[u8] = include_bytes!("../assets/turu.swf");

/// The host SWF Turu starts: VibeSkua's, or Turu's own (docs/HOST_SWF_PROPOSAL.md), chosen in
/// Options > Application or with `TURU_HOST_SWF`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostSwf {
    VibeSkua,
    Turu,
}

impl HostSwf {
    /// The one `TURU_HOST_SWF` asks for (`turu` or `vibeskua`), if it's set.
    pub fn from_env() -> Option<Self> {
        match std::env::var("TURU_HOST_SWF").as_deref() {
            Ok("turu") => Some(Self::Turu),
            Ok("vibeskua") => Some(Self::VibeSkua),
            Ok("") | Err(_) => None,
            Ok(other) => {
                tracing::warn!("TURU_HOST_SWF={other}: expected turu or vibeskua; ignored");
                None
            }
        }
    }

    /// `TURU_HOST_SWF`, else Options > Application's "Use Turu's Host SWF".
    pub fn chosen(settings: &settings::AppSettings) -> Self {
        Self::from_env().unwrap_or(if settings.turu_host_swf {
            Self::Turu
        } else {
            Self::VibeSkua
        })
    }

    /// Its file name in the data directory.
    pub fn file_name(self) -> &'static str {
        match self {
            Self::VibeSkua => "skua.swf",
            Self::Turu => "turu.swf",
        }
    }

    pub fn bytes(self) -> &'static [u8] {
        match self {
            Self::VibeSkua => SKUA_SWF,
            Self::Turu => TURU_SWF,
        }
    }
}

/// The script API (`import ... from "turu"`): the runtime scripts load, and the types editors
/// read from the scripts folder's `types/turu.ts`.
pub const TURU_TS: &str = include_str!("../js/turu.ts");

/// VibeSkua's shipped skill sets, used to seed the user's `AdvancedSkills.json`.
pub const DEFAULT_ADVANCED_SKILLS: &str = include_str!("../assets/AdvancedSkills.json");

/// The bot. One per game client; shared (`Arc`) by the UI, the script thread and the
/// background threads.
pub struct Bot {
    /// What's left of each game socket's last message (`player_packets`).
    socket_buffers: player_packets::SocketBuffers,
    pub flash: Flash,
    pub control: Arc<ScriptControl>,
    pub state: Mutex<GameState>,
    pub options: RwLock<BotOptions>,
    /// Options > Application (`settings.json`).
    pub settings: RwLock<AppSettings>,
    pub events: EventBus,
    pub logs: Logs,
    pub scripts: script::ScriptManager,
    pub skills: skills::SkillRunner,
    /// The counter-attack pause (`Combat.StopAttacking`, `UltraBossHelper`).
    pub combat: api::combat::CombatState,
    /// The Auto popup's Auto Attack / Auto Hunt.
    pub auto: auto::AutoState,
    pub drops: api::drops::DropRunner,
    pub handlers: api::handlers::HandlerRunner,
    pub boosts: api::boosts::BoostRunner,
    pub quests: api::quests::QuestRunner,
    pub servers: api::servers::ServerRunner,
    /// The command-line login, until the game has loaded.
    pub startup: Mutex<Option<startup::Startup>>,
    pub webhooks: webhooks::Webhooks,
    /// The web monitor's latest snapshot of this bot.
    pub monitor: monitor::Sampler,
    /// Following other accounts and summoning them (VibeSkua's ButlerV5 plugin).
    pub butler: butler::Butler,
    /// Keeps `gear/<username>.json` current (the launcher's view of this account's gear).
    pub gear: gear::GearCache,
    /// What Cosmetics shows over the real gear (VibeSkua's Cosmetics plugin).
    pub cosmetics: cosmetics::Look,
    /// Diagnostics > Interceptor: the game socket's packets (VibeSkua `GameProxy`). The app's
    /// socket wrapper holds a clone.
    pub capture: Arc<capture::CaptureProxy>,
    /// Diagnostics > Spammer.
    pub spammer: Arc<spammer::Spammer>,
    /// Scripts > Scheduler: the playlist this client runs.
    pub scheduler: scheduler::Scheduler,
    /// The army logs a script holds locked (`bot.army.withLock`).
    pub army: api::army::ArmyLocks,
    /// Diagnostics > Notification Center: the feed of game and script events.
    pub notifications: notifications::Feed,
    pub paths: paths::Paths,
}

impl Bot {
    pub fn new() -> Arc<Self> {
        let control = Arc::new(ScriptControl::default());
        let paths = paths::Paths::new();
        let options = paths.load_options().unwrap_or_default();
        let settings = paths.load_settings().unwrap_or_default();
        let bot = Arc::new(Self {
            flash: Flash::new(control.clone()),
            socket_buffers: player_packets::SocketBuffers::default(),
            control,
            state: Mutex::new(GameState::default()),
            options: RwLock::new(options),
            settings: RwLock::new(settings),
            events: EventBus::default(),
            logs: Logs::default(),
            scripts: script::ScriptManager::new(paths.clone()),
            skills: skills::SkillRunner::default(),
            combat: api::combat::CombatState::default(),
            auto: auto::AutoState::default(),
            drops: api::drops::DropRunner::default(),
            handlers: api::handlers::HandlerRunner::default(),
            boosts: api::boosts::BoostRunner::default(),
            quests: api::quests::QuestRunner::default(),
            servers: api::servers::ServerRunner::default(),
            startup: Mutex::new(None),
            webhooks: webhooks::Webhooks::default(),
            monitor: monitor::Sampler::default(),
            butler: butler::Butler::new(&paths),
            gear: gear::GearCache::default(),
            cosmetics: cosmetics::Look::default(),
            capture: Arc::default(),
            spammer: Arc::default(),
            scheduler: scheduler::Scheduler::default(),
            army: api::army::ArmyLocks::default(),
            notifications: notifications::Feed::default(),
            paths,
        });
        bot.flash.set_owner(&bot);
        background::start(&bot);
        webhooks::start(&bot);
        monitor::start(&bot);
        butler::start(&bot);
        gear::start(&bot);
        cosmetics::avatar::start(&bot);
        notifications::start(&bot);
        bot
    }

    /// Connects the bot to a running player (or disconnects it with `None`).
    pub fn set_host(&self, host: Option<Arc<dyn FlashHost>>) {
        if host.is_none() {
            *self.state.lock() = GameState::default();
        }
        self.flash.set_host(host);
    }

    /// Handles a call made by `skua.swf` (`ExternalInterface.call`). Called on the player's
    /// thread while the player is locked: it must not call into Flash or block.
    pub fn on_flash_call(self: &Arc<Self>, name: &str, args: Vec<serde_json::Value>) {
        events::on_flash_call(self, name, args);
    }

    /// `bot.log(...)`: a line in the script log. In the app's log (`ruffle.log`, the terminal) it
    /// starts with the account, as several clients write there (Turu).
    pub fn log(&self, text: impl Into<String>) {
        let text = text.into();
        match self.log_name() {
            Some(name) => tracing::info!(target: "turu::script", "[{name}] {text}"),
            None => tracing::info!(target: "turu::script", "{text}"),
        }
        self.logs.script.push(text);
    }

    /// A line in the Debug log that also goes to the app's log, with the account (Turu: what the
    /// game refused, for runs looked at afterwards). Never waits and never asks the game, so
    /// packet handlers can call it.
    pub(crate) fn note(&self, text: impl Into<String>) {
        let text = text.into();
        match self.log_name() {
            Some(name) => tracing::info!(target: "turu::debug", "[{name}] {text}"),
            None => tracing::info!(target: "turu::debug", "{text}"),
        }
        self.logs.debug.push(text);
    }

    /// The account, for the app's log: the logged-in character, else the login being used.
    /// Never waits on a lock (a caller may hold one) and never asks the game.
    fn log_name(&self) -> Option<String> {
        let character = self.state.try_lock().and_then(|s| s.username.clone());
        character
            .or_else(|| self.servers.try_login_info_name())
            .filter(|n| !n.trim().is_empty())
    }

    /// `bot.sleep(ms)`: on the script thread, wakes early (and fails) when the script stops.
    pub fn sleep(&self, ms: u64) -> BotResult<()> {
        self.control.sleep(Duration::from_millis(ms))
    }

    pub fn should_exit(&self) -> bool {
        self.control.should_exit()
    }

    pub fn opts(&self) -> parking_lot::RwLockReadGuard<'_, BotOptions> {
        self.options.read()
    }

    /// Writes the live options to `options.json`; a failure is logged too.
    pub fn save_options(&self) -> std::io::Result<()> {
        let result = self.paths.save_options(&self.options.read());
        if let Err(e) = &result {
            tracing::warn!("Couldn't save the bot options: {e}");
        }
        result
    }

    pub fn save_settings(&self) {
        if let Err(e) = self.paths.save_settings(&self.settings.read()) {
            tracing::warn!("Couldn't save the application settings: {e}");
        }
    }
}
