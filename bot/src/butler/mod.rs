//! Butler: follow another account, or have your other accounts follow you (VibeSkua's
//! ButlerV5 plugin, `Skua.Plugin.ButlerV5`).
//!
//! Every client publishes its account's state to a file of its own ([`sync`]): where it is,
//! whether it fights, and whom it summons (`followers`). A client whose account an online
//! master lists starts following it ([`follower`]); a master releases it by dropping the name.
//! Two threads per bot run it: "Butler Broadcast" writes the file every 500 ms (when it
//! changed, else every [`HEARTBEAT`]), "Butler Orders" reads the others' files and obeys.
//!
//! The follower also fakes quests, the level and AntiLag on its own client while it follows
//! ([`quest_bypass`], [`antilag`]); every component logs to [`debug_log`] for the window's
//! viewer. Not ported: quest cloning (copying the master's quest slots).

pub mod antilag;
pub mod debug_log;
pub mod follower;
pub mod quest_bypass;
pub mod sync;

use crate::Bot;
use crate::api::{inventory, map, player};
pub use debug_log::Level;
use debug_log::{DebugLog, LogLine};
use follower::{Follow, Mode};
use parking_lot::{Mutex, RwLock};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Weak};
use std::time::{Duration, Instant, SystemTime};
use sync::{SyncData, SyncDir};

/// How often an unchanged file is rewritten, to show the account is still online.
pub const HEARTBEAT: Duration = Duration::from_secs(5);
/// The broadcast and order threads' tick.
const TICK: Duration = Duration::from_millis(500);
/// How long a master's order may be unreadable before the butler parks (VibeSkua: 2.5 s).
const ORDER_GONE_AFTER: Duration = Duration::from_millis(2500);

/// The CoreBots class a butler equips when it starts following (VibeSkua `ButlerClassType`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClassType {
    /// Keep the current gear.
    #[default]
    None,
    Farm,
    Solo,
    Dodge,
    Boss,
}

impl ClassType {
    /// The CoreBots Options keys of the class and its mode (`FarmClassSelect`, `FarmModeSelect`).
    fn cbo_keys(self) -> Option<(&'static str, &'static str)> {
        match self {
            Self::None => None,
            Self::Farm => Some(("FarmClassSelect", "FarmModeSelect")),
            Self::Solo => Some(("SoloClassSelect", "SoloModeSelect")),
            Self::Dodge => Some(("DodgeClassSelect", "DodgeModeSelect")),
            Self::Boss => Some(("BossClassSelect", "BossModeSelect")),
        }
    }
}

/// Where a butler goes when it's released or its master is gone (VibeSkua `ParkSpot`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ParkSpot {
    /// Its own house (whitemap without one).
    #[default]
    House,
    Whitemap,
    /// Don't move.
    Stay,
}

/// Why this account follows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FollowSource {
    /// Follow was picked in this client.
    Manual,
    /// A master listed this account in its `followers`.
    Ordered,
}

/// Butler's options for one account. VibeSkua keeps the class type per account and "Obey
/// summons" for every client; Turu keeps both per account, since obeying is meant to be
/// turned off on the account you play by hand.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AccountSettings {
    pub class_type: ClassType,
    pub obey_summons: bool,
}

impl Default for AccountSettings {
    fn default() -> Self {
        Self {
            class_type: ClassType::None,
            obey_summons: true,
        }
    }
}

/// Butler's options (VibeSkua's plugin options), in `butler.json`, shared by every client:
/// a change in one is picked up by the others within a tick.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ButlerSettings {
    /// "Enable ButlerV5": off, no client broadcasts, follows or summons.
    pub enabled: bool,
    /// Only fight while the master fights.
    pub passive_attack: bool,
    /// Milliseconds between gotos while chasing the master (the 250 ms loop is the floor).
    pub goto_delay: u64,
    /// Keep sending goto while already in the master's room and cell.
    pub always_goto: bool,
    pub park: ParkSpot,
    /// Wait at the master's Enter cell and never fight.
    pub leech_mode: bool,
    /// Gotos without reaching the master before joining its room from its file (1-10).
    pub rescue_threshold: u32,
    /// "Quest bypasses": fake the table's quests for each map the butler enters.
    pub quest_bypass: bool,
    /// "Custom bypass quest IDs" ("1234, 5678"), faked when the follow starts.
    pub quest_bypass_custom: String,
    /// "Unlock all quests": max every quest slot instead (wins over the bypasses).
    pub quest_unlock_all: bool,
    /// "Fake level 100" on this client while following.
    pub fake_level: bool,
    /// AntiLag while summoned.
    pub anti_lag_summoned: bool,
    /// AntiLag while following by hand.
    pub anti_lag_following: bool,
    /// Record diagnostics in the window's log viewer.
    pub debug_logs: bool,
    /// Also append every log line to `butler_logs/debug_<account>.log`.
    pub auto_save_log: bool,
    /// Per account, by lowercase username.
    pub accounts: BTreeMap<String, AccountSettings>,
}

impl Default for ButlerSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            passive_attack: false,
            goto_delay: 500,
            always_goto: false,
            park: ParkSpot::House,
            leech_mode: false,
            rescue_threshold: 3,
            quest_bypass: true,
            quest_bypass_custom: String::new(),
            quest_unlock_all: true,
            fake_level: false,
            anti_lag_summoned: true,
            anti_lag_following: false,
            debug_logs: true,
            auto_save_log: false,
            accounts: BTreeMap::new(),
        }
    }
}

impl ButlerSettings {
    pub fn account(&self, username: &str) -> AccountSettings {
        self.accounts
            .get(&username.to_lowercase())
            .cloned()
            .unwrap_or_default()
    }

    pub fn set_account(&mut self, username: &str, account: AccountSettings) {
        self.accounts.insert(username.to_lowercase(), account);
    }
}

/// What the broadcast thread remembers between ticks. The UI reads it every frame
/// ([`Butler::username`]): never call Flash while holding it, or the UI thread waits for a
/// call only it can answer.
#[derive(Default)]
struct Broadcast {
    username: String,
    server: String,
    room: Option<String>,
    off_goto: bool,
    class_name: String,
    class_checked: Option<Instant>,
    attacking_seen: Option<Instant>,
    /// The last file written, and when.
    written: Option<(SyncData, Instant)>,
}

/// "A script started while following" (VibeSkua `ShowSuspendNotice`): the question the UI
/// shows until it's answered, the script stops or the follow ends.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuspendNotice {
    pub id: u64,
    pub master: String,
}

impl SuspendNotice {
    pub const CAPTION: &str = "Butler";

    pub fn message(&self) -> String {
        format!(
            "A script started while this account was following {}.\n\n\
             Following is now SUSPENDED and will resume automatically when the script stops.\n\n\
             Yes - keep it suspended (resume after the script)\n\
             No - stop following entirely",
            self.master
        )
    }
}

/// Butler's state on a bot.
pub struct Butler {
    pub dir: SyncDir,
    /// Saved logs and, with auto-save, the running log (`butler_logs`).
    pub logs_dir: PathBuf,
    settings_file: PathBuf,
    settings: RwLock<ButlerSettings>,
    /// The settings file's modification time when last read.
    settings_stamp: Mutex<Option<SystemTime>>,
    /// As master: the accounts this one summons.
    followers: Mutex<Vec<String>>,
    /// As butler: the master whose summon was declined, until it releases this account.
    declined: Mutex<Option<String>>,
    follow: Mutex<Option<Follow>>,
    broadcast: Mutex<Broadcast>,
    log: DebugLog,
    /// The account the auto-saved log is named after (not in `broadcast`: logging happens
    /// under that lock).
    log_name: Mutex<String>,
    notice: Mutex<Option<SuspendNotice>>,
    next_notice: AtomicU64,
    /// The client or tab closed: no more broadcasts.
    closed: AtomicBool,
}

impl Butler {
    pub fn new(paths: &crate::paths::Paths) -> Self {
        let butler = Self {
            dir: SyncDir::new(paths.butler_dir()),
            logs_dir: paths.butler_logs_dir(),
            settings_file: paths.butler_settings_file(),
            settings: RwLock::new(ButlerSettings::default()),
            settings_stamp: Mutex::new(None),
            followers: Mutex::new(Vec::new()),
            declined: Mutex::new(None),
            follow: Mutex::new(None),
            broadcast: Mutex::new(Broadcast::default()),
            log: DebugLog::default(),
            log_name: Mutex::new(String::new()),
            notice: Mutex::new(None),
            next_notice: AtomicU64::new(1),
            closed: AtomicBool::new(false),
        };
        butler.reload_settings();
        butler
    }

    pub fn settings(&self) -> ButlerSettings {
        self.settings.read().clone()
    }

    /// Saves the options for every client.
    pub fn set_settings(&self, settings: ButlerSettings) -> std::io::Result<()> {
        if let Some(dir) = self.settings_file.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let text = serde_json::to_string_pretty(&settings).map_err(std::io::Error::other)?;
        std::fs::write(&self.settings_file, text)?;
        *self.settings_stamp.lock() = modified(&self.settings_file);
        *self.settings.write() = settings;
        Ok(())
    }

    /// Reads `butler.json` again when another client changed it.
    fn reload_settings(&self) {
        let stamp = modified(&self.settings_file);
        let mut last = self.settings_stamp.lock();
        if stamp.is_none() || stamp == *last {
            return;
        }
        *last = stamp;
        if let Some(settings) = std::fs::read_to_string(&self.settings_file)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
        {
            *self.settings.write() = settings;
        }
    }

    pub fn enabled(&self) -> bool {
        self.settings.read().enabled
    }

    /// The accounts this one summons.
    pub fn followers(&self) -> Vec<String> {
        self.followers.lock().clone()
    }

    fn set_followers(&self, followers: Vec<String>) {
        *self.followers.lock() = followers;
    }

    fn is_follower(&self, username: &str) -> bool {
        self.followers
            .lock()
            .iter()
            .any(|f| f.eq_ignore_ascii_case(username))
    }

    /// The master this account follows and why, while the follow runs.
    pub fn following(&self) -> Option<(String, FollowSource)> {
        self.follow
            .lock()
            .as_ref()
            .filter(|f| f.running())
            .map(|f| (f.master.clone(), f.source))
    }

    /// Following, but idle at the park spot (master offline or elsewhere).
    pub fn is_parked(&self) -> bool {
        self.follow
            .lock()
            .as_ref()
            .is_some_and(|f| f.running() && f.shared.parked())
    }

    /// No follower thread runs: a stopped follow has put the options back and parked.
    pub fn follow_ended(&self) -> bool {
        self.follow.lock().as_ref().is_none_or(|f| f.finished())
    }

    /// On the way to the park spot.
    pub fn is_parking(&self) -> bool {
        self.follow
            .lock()
            .as_ref()
            .is_some_and(|f| !f.finished() && f.shared.parking())
    }

    /// This account's username, as last broadcast ("" before a login).
    pub fn username(&self) -> String {
        self.broadcast.lock().username.clone()
    }

    /// This account's server name, as last broadcast.
    pub fn server(&self) -> String {
        self.broadcast.lock().server.clone()
    }

    /// The other online accounts running Butler.
    pub fn roster(&self) -> Vec<SyncData> {
        let me = self.username();
        self.dir.online((!me.is_empty()).then_some(me.as_str()))
    }

    /// Adds a line to the log viewer (and the auto-saved file), when debug logs are on.
    pub fn record(&self, component: &str, message: &str, level: Level) {
        let (enabled, auto_save) = {
            let s = self.settings.read();
            (s.debug_logs, s.auto_save_log)
        };
        if !enabled {
            return;
        }
        let file = auto_save.then(|| {
            let name = self.log_name.lock().clone();
            let name = if name.is_empty() {
                format!("pid{}", std::process::id())
            } else {
                name.to_lowercase()
            };
            self.logs_dir.join(format!("debug_{name}.log"))
        });
        self.log.push(component, message, level, file.as_deref());
    }

    pub fn log_lines(&self) -> Vec<LogLine> {
        self.log.lines()
    }

    /// Changes whenever the log does.
    pub fn log_version(&self) -> u64 {
        self.log.version()
    }

    /// The log as text (Copy).
    pub fn log_text(&self) -> String {
        self.log.text()
    }

    pub fn clear_log(&self) {
        self.log.clear();
    }

    /// Writes the log to a new file in [`Butler::logs_dir`] (Save); its path.
    pub fn save_log(&self) -> std::io::Result<PathBuf> {
        let name = self.log_name.lock().clone();
        let path = self.log.save(&self.logs_dir, &name)?;
        self.record(
            "UI",
            &format!("saved the log to {}", path.display()),
            Level::Success,
        );
        Ok(path)
    }

    /// The suspend question waiting for an answer, if any.
    pub fn suspend_notice(&self) -> Option<SuspendNotice> {
        self.notice.lock().clone()
    }

    fn ask_suspend(&self, master: &str) {
        let id = self.next_notice.fetch_add(1, Ordering::Relaxed);
        *self.notice.lock() = Some(SuspendNotice {
            id,
            master: master.to_owned(),
        });
    }

    fn drop_suspend_notice(&self) {
        *self.notice.lock() = None;
    }
}

fn modified(path: &std::path::Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

/// A diagnostic: to the debug log and Butler's log viewer.
pub(crate) fn trace(bot: &Bot, component: &str, level: Level, text: impl AsRef<str>) {
    let text = text.as_ref();
    bot.logs.debug.push(format!("[Butler] {text}"));
    bot.butler.record(component, text, level);
}

fn debug(bot: &Bot, component: &str, text: impl AsRef<str>) {
    trace(bot, component, Level::Info, text);
}

/// A message for the user: to the script log and Butler's log viewer.
fn log(bot: &Bot, component: &str, text: impl AsRef<str>) {
    let text = text.as_ref();
    bot.log(format!("[Butler] {text}"));
    bot.butler.record(component, text, Level::Info);
}

pub(crate) fn start(bot: &Arc<Bot>) {
    for (name, body) in [
        ("Butler Broadcast", broadcast_loop as fn(Weak<Bot>)),
        ("Butler Orders", order_loop as fn(Weak<Bot>)),
    ] {
        let weak = Arc::downgrade(bot);
        std::thread::Builder::new()
            .name(name.into())
            .spawn(move || body(weak))
            .expect("couldn't start a Butler thread");
    }
}

/// The game is up and connected to this bot (the timer thread's check).
fn game_ready(bot: &Bot) -> bool {
    bot.state.lock().game_loaded && bot.flash.has_host()
}

fn broadcast_loop(weak: Weak<Bot>) {
    let mut was_enabled = true;
    loop {
        std::thread::sleep(TICK);
        let Some(bot) = weak.upgrade() else { break };
        if bot.butler.closed.load(Ordering::Acquire) {
            break;
        }
        bot.butler.reload_settings();
        let enabled = bot.butler.enabled();
        if !enabled {
            if was_enabled {
                write_offline(&bot);
            }
            was_enabled = false;
            continue;
        }
        was_enabled = true;
        if game_ready(&bot)
            && let Err(e) = broadcast(&bot)
        {
            tracing::debug!("Butler broadcast failed: {e}");
        }
    }
}

/// Writes this account's file (VibeSkua `Broadcaster.WriteNow`), or marks it offline after a
/// logout.
fn broadcast(bot: &Bot) -> crate::BotResult<()> {
    if !player::logged_in(bot)? {
        write_offline(bot);
        return Ok(());
    }
    let username = player::username(bot)?;
    if username.is_empty() {
        return Ok(());
    }
    let butler = &bot.butler;
    {
        let mut name = butler.log_name.lock();
        if *name != username {
            name.clone_from(&username);
        }
    }
    let map_name = map::name(bot)?.to_lowercase();
    // Mid-transition the map isn't loaded or is named "-1": keep the last state, but keep
    // the heartbeat going.
    if !map::loaded(bot)? || map_name.is_empty() || map_name == "-1" {
        let mut b = butler.broadcast.lock();
        if let Some((data, at)) = &b.written
            && !butler.closed.load(Ordering::Acquire)
            && data.username.eq_ignore_ascii_case(&username)
            && at.elapsed() >= HEARTBEAT
        {
            let data = SyncData {
                beat: sync::now(),
                ..data.clone()
            };
            if butler.dir.write(&data).is_ok() {
                b.written = Some((data, Instant::now()));
            }
        }
        return Ok(());
    }

    let raw_attacking = player::in_combat(bot)? || player::has_target(bot)?;
    let server = server_name(bot);
    let off_goto = bot.flash.get_opt::<bool>("uoPref.bGoto")?.map(|on| !on);
    let room = room_of(&map::full_name(bot)?);
    let cell = non_empty(player::cell(bot)?, "Enter");
    let pad = non_empty(player::pad(bot)?, "Spawn");
    let check_class = {
        let mut b = butler.broadcast.lock();
        if !b.username.eq_ignore_ascii_case(&username) {
            b.class_name.clear();
            b.class_checked = None;
            b.room = None;
        }
        b.class_checked
            .is_none_or(|t| t.elapsed() >= Duration::from_secs(2))
    };
    // The class scans the inventory: every 2 s is enough for the roster.
    let class = if check_class {
        inventory::current_class(bot).ok().flatten()
    } else {
        None
    };
    let script_name = bot.scripts.is_running().then(|| {
        bot.scripts
            .loaded()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
            .unwrap_or_else(|| "script".into())
    });
    let following = butler.following().filter(|_| !butler.is_parked());
    let parking = butler.is_parking();
    let declined = butler.declined.lock().clone().unwrap_or_default();
    let followers = butler.followers();

    let mut b = butler.broadcast.lock();
    b.username = username.clone();
    b.server = server.clone();
    if let Some(off) = off_goto {
        b.off_goto = off;
    }
    if let Some(room) = room {
        b.room = Some(room);
    }
    if check_class {
        b.class_checked = Some(Instant::now());
        if let Some(class) = class {
            b.class_name = class;
        }
    }
    // Attacking shows at once, but clears only after 4 s of calm: farming pauses 1-3 s
    // between pulls.
    if raw_attacking {
        b.attacking_seen = Some(Instant::now());
    }
    let attacking = b
        .attacking_seen
        .is_some_and(|t| t.elapsed() < Duration::from_secs(4));
    let data = SyncData {
        username,
        pid: std::process::id(),
        server,
        class_name: b.class_name.clone(),
        map: map_name,
        room: b.room.clone().unwrap_or_else(|| "1".into()),
        cell,
        pad,
        logged_in: true,
        attacking,
        off_goto: b.off_goto,
        followers,
        following: following.map(|(m, _)| m).unwrap_or_default(),
        script_on: script_name.is_some(),
        script_name: script_name.unwrap_or_default(),
        declined,
        parking,
        beat: sync::now(),
    };
    write_if_changed(butler, &mut b, data);
    Ok(())
}

fn write_if_changed(butler: &Butler, b: &mut Broadcast, data: SyncData) {
    // Checked under the lock `close` takes, so no broadcast lands after its offline write.
    if butler.closed.load(Ordering::Acquire) {
        return;
    }
    let due = match &b.written {
        Some((last, at)) => !last.same_state(&data) || at.elapsed() >= HEARTBEAT,
        None => true,
    };
    if due {
        match butler.dir.write(&data) {
            Ok(()) => b.written = Some((data, Instant::now())),
            Err(e) => tracing::debug!("Couldn't write the Butler file: {e}"),
        }
    }
}

/// Marks this account offline (on logout, when Butler is turned off and when the client
/// closes). Nothing to do when it never broadcast.
pub fn write_offline(bot: &Bot) {
    let butler = &bot.butler;
    let mut b = butler.broadcast.lock();
    if b.username.is_empty() || b.written.as_ref().is_some_and(|(d, _)| !d.logged_in) {
        return;
    }
    let data = SyncData {
        username: b.username.clone(),
        pid: std::process::id(),
        logged_in: false,
        followers: butler.followers(),
        beat: sync::now(),
        ..Default::default()
    };
    if butler.dir.write(&data).is_ok() {
        b.written = Some((data, Instant::now()));
    }
    b.class_name.clear();
    b.class_checked = None;
}

/// The client or its tab closes (VibeSkua's process-exit hook): marks the account offline at
/// once, so the other clients drop it without waiting for the heartbeat to age out, and ends
/// any follow. Doesn't call Flash or wait: safe on the UI thread.
pub fn close(bot: &Bot) {
    let butler = &bot.butler;
    {
        // Under the broadcast lock: a broadcast in progress finishes first and none follows.
        let _b = butler.broadcast.lock();
        butler.closed.store(true, Ordering::Release);
    }
    write_offline(bot);
    butler.drop_suspend_notice();
    stop_follow_with(bot, Mode::Abort);
}

fn non_empty(value: String, default: &str) -> String {
    if value.is_empty() {
        default.into()
    } else {
        value
    }
}

/// The room number of a `map-room` title ("ultradage-100000" -> "100000").
pub fn room_of(full_name: &str) -> Option<String> {
    let (_, room) = full_name.rsplit_once('-')?;
    (!room.is_empty() && room.chars().all(|c| c.is_ascii_digit())).then(|| room.to_owned())
}

/// The game server's name (`objServerInfo.sName`), else the last login's, else its IP. Not
/// the IP first: several servers share a socket host.
pub fn server_name(bot: &Bot) -> String {
    if let Ok(Some(name)) = bot.flash.get_opt::<String>("objServerInfo.sName") {
        let name = name.trim();
        if !name.is_empty() && name != "null" {
            return name.to_owned();
        }
    }
    let last = bot.servers.last_name();
    if !last.is_empty() {
        return last;
    }
    player::server_ip(bot).unwrap_or_default()
}

/// What the order thread remembers between ticks.
#[derive(Default)]
struct Orders {
    /// Since when the ordering master's file has been unreadable or offline.
    gone_since: Option<Instant>,
    busy_logged: bool,
}

fn order_loop(weak: Weak<Bot>) {
    let mut orders = Orders::default();
    let mut was_enabled = true;
    loop {
        std::thread::sleep(TICK);
        let Some(bot) = weak.upgrade() else { break };
        if bot.butler.closed.load(Ordering::Acquire) {
            break;
        }
        let enabled = bot.butler.enabled();
        if enabled != was_enabled {
            was_enabled = enabled;
            if enabled {
                log(&bot, "Orders", "Butler enabled - resuming.");
            } else {
                // Off: stop where it stands, no parking.
                log(&bot, "Orders", "Butler disabled - stopped.");
                stop_follow_with(&bot, Mode::Abort);
            }
        }
        if enabled && game_ready(&bot) {
            obey_orders(&bot, &mut orders);
        }
    }
}

/// The butler's side of a summon (VibeSkua's order watcher): follow the online master that
/// lists this account, park when it releases it. A manual follow wins over orders.
fn obey_orders(bot: &Arc<Bot>, orders: &mut Orders) {
    let butler = &bot.butler;
    let me = butler.username();
    if me.is_empty() || !player::logged_in(bot).unwrap_or(false) {
        return;
    }
    let online = butler.dir.online(Some(&me));
    handshakes(bot, &me, &online);
    let current = butler.following();
    if current
        .as_ref()
        .is_some_and(|(_, s)| *s == FollowSource::Manual)
    {
        return;
    }
    let ordered = current.filter(|(_, s)| *s == FollowSource::Ordered);
    if !butler.settings().account(&me).obey_summons {
        if ordered.is_some() {
            debug(
                bot,
                "Orders",
                "Obey summons turned off - dropping the ordered follow",
            );
            stop_follow_with(bot, Mode::StopAndPark);
        }
        return;
    }
    // Never obey an account this one summons (they'd follow each other), or one declined.
    let declined = butler.declined.lock().clone();
    let master = sync::master_ordering(&me, &online).filter(|m| {
        !butler.is_follower(&m.username)
            && declined
                .as_deref()
                .is_none_or(|d| !d.eq_ignore_ascii_case(&m.username))
    });
    if let Some(master) = master {
        orders.gone_since = None;
        if bot.scripts.is_running() {
            // Busy: the order stays listed, so it's picked up when the script stops.
            if !orders.busy_logged {
                orders.busy_logged = true;
                debug(
                    bot,
                    "Orders",
                    format!("{}'s summon on hold - a script is running", master.username),
                );
            }
        } else if ordered
            .as_ref()
            .is_none_or(|(m, _)| !m.eq_ignore_ascii_case(&master.username))
        {
            orders.busy_logged = false;
            debug(
                bot,
                "Orders",
                format!("{} summoned {me} - obeying", master.username),
            );
            follow(bot, &master.username, FollowSource::Ordered);
        }
    } else if let Some((current, _)) = ordered {
        // The master online without this account in its list released it: stop at once.
        // Missing or offline may be a file caught mid-write: wait a little first.
        let released = online
            .iter()
            .find(|d| d.username.eq_ignore_ascii_case(&current))
            .is_some_and(|d| !d.lists_follower(&me));
        if released {
            orders.gone_since = None;
            debug(bot, "Orders", format!("{current} released me - parking"));
            stop_follow_with(bot, Mode::StopAndPark);
        } else {
            let since = *orders.gone_since.get_or_insert_with(Instant::now);
            if since.elapsed() >= ORDER_GONE_AFTER {
                orders.gone_since = None;
                debug(
                    bot,
                    "Orders",
                    "Summon gone (master offline or unreadable) - parking",
                );
                stop_follow_with(bot, Mode::StopAndPark);
            }
        }
    } else {
        orders.gone_since = None;
    }
}

/// The decline handshake (VibeSkua `ProcessServingHandshakes`): a butler's decline is cleared
/// once its master dropped it; a master releases butlers that declined it.
fn handshakes(bot: &Bot, me: &str, online: &[SyncData]) {
    let butler = &bot.butler;
    {
        let mut declined = butler.declined.lock();
        if let Some(master) = declined.as_deref() {
            let still_listed = online
                .iter()
                .find(|d| d.username.eq_ignore_ascii_case(master))
                .is_some_and(|d| d.lists_follower(me));
            if !still_listed {
                debug(bot, "Orders", format!("{master} acknowledged the decline"));
                *declined = None;
            }
        }
    }
    let followers = butler.followers();
    if followers.is_empty() {
        return;
    }
    for d in online {
        if d.declined.eq_ignore_ascii_case(me)
            && followers
                .iter()
                .any(|f| f.eq_ignore_ascii_case(&d.username))
        {
            log(
                bot,
                "Orders",
                format!("{} declined the summon - released.", d.username),
            );
            butler.set_followers(
                followers
                    .iter()
                    .filter(|f| !f.eq_ignore_ascii_case(&d.username))
                    .cloned()
                    .collect(),
            );
        }
    }
}

/// Starts following `master`, ending any follow first. Calls Flash: not for the UI thread.
fn follow(bot: &Arc<Bot>, master: &str, source: FollowSource) {
    // Not under the lock: the broadcast thread reads it meanwhile.
    let old = bot.butler.follow.lock().take();
    if let Some(old) = old {
        old.abort_and_join();
    }
    let new = Follow::start(bot, master, source);
    let replaced = std::mem::replace(&mut *bot.butler.follow.lock(), new);
    if let Some(replaced) = replaced {
        replaced.stop(Mode::Abort);
    }
}

fn stop_follow_with(bot: &Bot, mode: Mode) {
    if let Some(follow) = bot.butler.follow.lock().as_ref() {
        follow.stop(mode);
    }
}

/// "Follow" in this client (VibeSkua `StartFollow`). Refused while Butler is off or a script
/// runs. Following an account this one summons releases it first. Waits for a previous follow
/// to end: not for the UI thread.
pub fn start_follow(bot: &Arc<Bot>, master: &str) -> Result<(), String> {
    let butler = &bot.butler;
    if !butler.enabled() {
        return Err("Butler is disabled. Turn it on first.".into());
    }
    if bot.scripts.is_running() {
        return Err("A script is running. Stop it first.".into());
    }
    if master.trim().is_empty() || master.eq_ignore_ascii_case(&butler.username()) {
        return Err("Pick another account to follow.".into());
    }
    if butler.is_follower(master) {
        log(
            bot,
            "UI",
            format!("Releasing {master} (can't follow an account that follows me)."),
        );
        let followers = butler
            .followers()
            .into_iter()
            .filter(|f| !f.eq_ignore_ascii_case(master))
            .collect();
        butler.set_followers(followers);
    }
    follow(bot, master.trim(), FollowSource::Manual);
    Ok(())
}

/// "Stop" in this client (VibeSkua `StopFollow`): stops where it stands. Stopping an ordered
/// follow declines the summon, or the order would be obeyed again at the next tick.
pub fn stop_follow(bot: &Bot) {
    if let Some((master, FollowSource::Ordered)) = bot.butler.following() {
        debug(bot, "UI", format!("Declining {master}'s summon"));
        *bot.butler.declined.lock() = Some(master);
    }
    stop_follow_with(bot, Mode::Stop);
}

/// The answer to the suspend question `id`: keep the follow suspended until the script stops
/// (Yes, or the box closed), or stop following (No). Doesn't call Flash or wait: safe on the UI
/// thread. An answer to a question no longer asked does nothing.
pub fn answer_suspend(bot: &Bot, id: u64, keep_suspended: bool) {
    {
        let mut notice = bot.butler.notice.lock();
        if notice.as_ref().is_none_or(|n| n.id != id) {
            return;
        }
        *notice = None;
    }
    if keep_suspended {
        debug(bot, "UI", "suspend notice: keep suspended");
    } else {
        debug(bot, "UI", "suspend notice: stop following");
        stop_follow(bot);
    }
}

/// Summons or releases one account (VibeSkua `ToggleFollower`); the message to show.
pub fn toggle_follower(bot: &Bot, username: &str) -> Result<String, String> {
    let butler = &bot.butler;
    let mut followers = butler.followers();
    if let Some(i) = followers
        .iter()
        .position(|f| f.eq_ignore_ascii_case(username))
    {
        followers.remove(i);
        butler.set_followers(followers);
        let message = format!("Released {username}.");
        log(bot, "UI", &message);
        return Ok(message);
    }
    // No chains: a butler can't summon.
    if let Some((master, _)) = butler.following() {
        return Err(format!(
            "Can't summon {username} while following {master}. Stop that first."
        ));
    }
    if !butler.enabled() {
        return Err("Butler is disabled. Turn it on first.".into());
    }
    followers.push(username.to_owned());
    butler.set_followers(followers);
    let message = format!("Summoned {username} to follow me.");
    log(bot, "UI", &message);
    Ok(message)
}

/// Summons one online account, with the checks the window's Summon button makes (the control
/// API's `/butler/summon`); summoning one already summoned does nothing.
pub fn summon(bot: &Bot, username: &str) -> Result<String, String> {
    let butler = &bot.butler;
    if butler.is_follower(username) {
        return Ok(format!("{username} is already summoned."));
    }
    if !butler.enabled() {
        return Err("Butler is disabled. Turn it on first.".into());
    }
    let Some(account) = butler
        .roster()
        .into_iter()
        .find(|d| d.username.eq_ignore_ascii_case(username))
    else {
        return Err(format!("{username} isn't online with Butler."));
    };
    if butler.following().is_none()
        && let Some(why) = unavailable(bot, &account)
    {
        return Err(format!("{} {why}.", account.username));
    }
    toggle_follower(bot, &account.username)
}

/// Releases one summoned account (the control API's `/butler/release`).
pub fn release(bot: &Bot, username: &str) -> Result<String, String> {
    if !bot.butler.is_follower(username) {
        return Err(format!("{username} isn't summoned."));
    }
    toggle_follower(bot, username)
}

/// Why an account can't be summoned now (VibeSkua's `Summon all` filter, which greys the
/// per-row Summon button out too); `None` when it can.
pub fn unavailable(bot: &Bot, account: &SyncData) -> Option<&'static str> {
    let butler = &bot.butler;
    let me = butler.username();
    let my_server = butler.server();
    if account.script_on {
        Some("a script is running")
    } else if account.following.eq_ignore_ascii_case(&me) && !butler.is_follower(&account.username)
    {
        Some("already follows me")
    } else if !account.following.is_empty() && !account.following.eq_ignore_ascii_case(&me) {
        Some("follows another account")
    } else if !account.followers.is_empty() {
        Some("is a master")
    } else if !account.server.is_empty()
        && !my_server.is_empty()
        && !account.server.eq_ignore_ascii_case(&my_server)
    {
        Some("on another server")
    } else if account.parking {
        Some("parking")
    } else {
        None
    }
}

/// Summons every available online account (VibeSkua `SummonAll`), keeping those already
/// summoned; the message to show.
pub fn summon_all(bot: &Bot) -> Result<String, String> {
    let butler = &bot.butler;
    if let Some((master, _)) = butler.following() {
        return Err(format!(
            "Can't summon while following {master}. Stop that first."
        ));
    }
    if !butler.enabled() {
        return Err("Butler is disabled. Turn it on first.".into());
    }
    let (available, skipped): (Vec<SyncData>, Vec<SyncData>) = butler
        .roster()
        .into_iter()
        .partition(|d| unavailable(bot, d).is_none());
    if !skipped.is_empty() {
        let names: Vec<&str> = skipped.iter().map(|d| d.username.as_str()).collect();
        log(
            bot,
            "UI",
            format!(
                "Skipped (busy, following, a master, another server or parking): {}",
                names.join(", ")
            ),
        );
    }
    if available.is_empty() {
        return Err("No available accounts to summon.".into());
    }
    let mut followers = butler.followers();
    for d in &available {
        if !followers
            .iter()
            .any(|f| f.eq_ignore_ascii_case(&d.username))
        {
            followers.push(d.username.clone());
        }
    }
    butler.set_followers(followers);
    let names: Vec<&str> = available.iter().map(|d| d.username.as_str()).collect();
    let message = format!("Summoned: {}", names.join(", "));
    log(bot, "UI", &message);
    Ok(message)
}

/// Releases every summoned account (VibeSkua `ReleaseFollowers`).
pub fn release_all(bot: &Bot) {
    bot.butler.set_followers(Vec::new());
    log(bot, "UI", "Released all followers.");
}
