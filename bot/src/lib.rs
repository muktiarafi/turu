//! Turu's bot engine: VibeSkua's script API in Rust, driven by TypeScript scripts.
//!
//! Start at [`Bot`]. Architecture: docs/turu/ARCHITECTURE.md.

pub mod api;
mod background;
pub mod cbo;
pub mod control;
pub mod control_api;
pub mod error;
pub mod events;
pub mod flash;
pub mod host;
pub mod log;
pub mod models;
pub mod options;
pub mod paths;
pub mod script;
pub mod settings;
pub mod skills;
pub mod startup;
pub mod state;
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

/// VibeSkua's shipped skill sets, used to seed the user's `AdvancedSkills.json`.
pub const DEFAULT_ADVANCED_SKILLS: &str = include_str!("../assets/AdvancedSkills.json");

/// The bot. One per game client; shared (`Arc`) by the UI, the script thread and the
/// background threads.
pub struct Bot {
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
    pub drops: api::drops::DropRunner,
    pub handlers: api::handlers::HandlerRunner,
    pub boosts: api::boosts::BoostRunner,
    pub quests: api::quests::QuestRunner,
    pub servers: api::servers::ServerRunner,
    /// The command-line login, until the game has loaded.
    pub startup: Mutex<Option<startup::Startup>>,
    pub webhooks: webhooks::Webhooks,
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
            control,
            state: Mutex::new(GameState::default()),
            options: RwLock::new(options),
            settings: RwLock::new(settings),
            events: EventBus::default(),
            logs: Logs::default(),
            scripts: script::ScriptManager::new(paths.clone()),
            skills: skills::SkillRunner::default(),
            combat: api::combat::CombatState::default(),
            drops: api::drops::DropRunner::default(),
            handlers: api::handlers::HandlerRunner::default(),
            boosts: api::boosts::BoostRunner::default(),
            quests: api::quests::QuestRunner::default(),
            servers: api::servers::ServerRunner::default(),
            startup: Mutex::new(None),
            webhooks: webhooks::Webhooks::default(),
            paths,
        });
        background::start(&bot);
        webhooks::start(&bot);
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

    /// `bot.log(...)`: a line in the script log.
    pub fn log(&self, text: impl Into<String>) {
        let text = text.into();
        tracing::info!(target: "turu::script", "{text}");
        self.logs.script.push(text);
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

    pub fn save_options(&self) {
        if let Err(e) = self.paths.save_options(&self.options.read()) {
            tracing::warn!("Couldn't save the bot options: {e}");
        }
    }

    pub fn save_settings(&self) {
        if let Err(e) = self.paths.save_settings(&self.settings.read()) {
            tracing::warn!("Couldn't save the application settings: {e}");
        }
    }
}
