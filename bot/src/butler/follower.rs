//! Following a master (VibeSkua ButlerV5 `Follower.cs`): a "Butler Follower" thread that
//! reads the master's sync file every 250 ms and goes after it. Goto first (it reaches houses
//! and is fastest); after failed gotos, a locked zone, a full room or goto turned off, a join
//! of the master's exact `map-room` from its file. In the master's room it jumps to its cell
//! and fights (or, in leech mode, waits at Enter). It parks while the master is offline or on
//! another server, and suspends while a script runs in this client. While it follows it fakes
//! quests and the level on its own client, and turns AntiLag on, as the options say.

use super::sync::{self, SyncData};
use super::{
    ButlerSettings, FollowSource, Level, ParkSpot, antilag, quest_bypass, server_name, trace,
};
use crate::api::inventory::{self, Container};
use crate::api::{NameOrId, combat, map, player, wait};
use crate::events::BotEvent;
use crate::{Bot, BotResult};
use serde_json::Value;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// The loop's tick.
const LOOP_MS: u64 = 250;
/// The shortest time between two cell jumps.
const JUMP_INTERVAL: Duration = Duration::from_millis(500);
/// The shortest time between two joins of the same room.
const JOIN_INTERVAL: Duration = Duration::from_secs(5);
const PARK_MAP: &str = "whitemap-100000";

/// How a follow is asked to end.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Mode {
    Run = 0,
    /// Stop where it stands.
    Stop = 1,
    /// Stop, then go to the park spot (a master released it).
    StopAndPark = 2,
    /// Stop at once, even mid-park (a new follow starts, or Butler was turned off).
    Abort = 3,
}

fn debug(bot: &Bot, text: impl AsRef<str>) {
    super::debug(bot, "Follower", text);
}

fn log(bot: &Bot, text: impl AsRef<str>) {
    super::log(bot, "Follower", text);
}

fn error(bot: &Bot, text: impl AsRef<str>) {
    trace(bot, "Follower", Level::Error, text);
}

/// What the follower thread shares with the rest of Butler.
#[derive(Default)]
pub struct Shared {
    mode: AtomicU8,
    /// Set with any stop: ends a wait for a quest to load.
    cancel: AtomicBool,
    parked: AtomicBool,
    parking: AtomicBool,
    /// Set by the packet listener: "Locked zone" / "is not available".
    locked_zone: AtomicBool,
    /// "is full".
    room_full: AtomicBool,
    /// "ignoring goto".
    goto_off: AtomicBool,
}

impl Shared {
    fn mode(&self) -> Mode {
        match self.mode.load(Ordering::Acquire) {
            0 => Mode::Run,
            1 => Mode::Stop,
            2 => Mode::StopAndPark,
            _ => Mode::Abort,
        }
    }

    pub fn parked(&self) -> bool {
        self.parked.load(Ordering::Acquire)
    }

    pub fn parking(&self) -> bool {
        self.parking.load(Ordering::Acquire)
    }

    /// The server's warnings Butler reacts to (VibeSkua's packet listener and its "instant
    /// warnings" on the raw packet text).
    pub fn on_packet(&self, packet: &Value) {
        let Some(data) = packet.pointer("/params/dataObj").and_then(Value::as_array) else {
            return;
        };
        if !matches!(
            data.first().and_then(Value::as_str),
            Some("server" | "warning")
        ) {
            return;
        }
        let text = packet.to_string().to_lowercase();
        if text.contains("a locked zone.") || text.contains("is not available.") {
            self.locked_zone.store(true, Ordering::Release);
        }
        if text.contains("is full") {
            self.room_full.store(true, Ordering::Release);
        }
        if text.contains("ignoring goto") {
            self.goto_off.store(true, Ordering::Release);
        }
    }
}

/// A running follow.
pub struct Follow {
    pub master: String,
    pub source: FollowSource,
    pub shared: Arc<Shared>,
    thread: JoinHandle<()>,
}

impl Follow {
    /// Starts the follower thread. `None` when the thread couldn't start.
    pub(crate) fn start(bot: &Arc<Bot>, master: &str, source: FollowSource) -> Option<Self> {
        let shared = Arc::new(Shared::default());
        let thread_shared = shared.clone();
        let bot = bot.clone();
        let name = master.to_owned();
        let thread = std::thread::Builder::new()
            .name("Butler Follower".into())
            .spawn(move || Follower::new(&bot, name, source, thread_shared).run())
            .map_err(|e| tracing::warn!("Couldn't start the Butler follower: {e}"))
            .ok()?;
        Some(Self {
            master: master.to_owned(),
            source,
            shared,
            thread,
        })
    }

    /// Follows (asked to stop or not, the thread is still going).
    pub fn running(&self) -> bool {
        self.shared.mode() == Mode::Run && !self.thread.is_finished()
    }

    pub fn finished(&self) -> bool {
        self.thread.is_finished()
    }

    /// Asks the follow to end; doesn't wait. A stronger request replaces a weaker one.
    pub fn stop(&self, mode: Mode) {
        self.shared.mode.fetch_max(mode as u8, Ordering::AcqRel);
        self.shared.cancel.store(true, Ordering::Release);
    }

    /// Ends the follow at once and waits for its thread.
    pub(crate) fn abort_and_join(self) {
        self.stop(Mode::Abort);
        let _ = self.thread.join();
    }
}

/// The follower thread's state (VibeSkua `Follower`'s fields).
struct Follower<'a> {
    bot: &'a Arc<Bot>,
    master: String,
    source: FollowSource,
    shared: Arc<Shared>,
    last_goto: Option<Instant>,
    last_jump: Option<Instant>,
    last_join: Option<(String, Instant)>,
    last_tick_log: Option<Instant>,
    goto_attempts: u32,
    direct_join_attempts: u32,
    direct_join_target: Option<String>,
    last_reported_off_goto: Option<bool>,
    /// The class and mode the skills run with (from CoreBots Options, else the worn class).
    skill_class: Option<String>,
    skill_mode: String,
    last_rescue_map: Option<String>,
    rescue_rounds: u32,
    /// A map the butler kept failing to enter, until the master leaves it.
    blocked_map: Option<String>,
    /// A full room, until the master leaves it.
    blocked_room: Option<String>,
    room_full_attempts: u32,
    full_count_room: Option<String>,
    bad_reads: u32,
    suspended: bool,
    was_logged_in: bool,
    last_with_master: Option<bool>,
    /// Infinite Range and Skip Cutscenes before the follow turned them on.
    prior_options: Option<(bool, bool)>,
    /// The map whose quest fakes were last applied (on arrival or before travelling there).
    last_bypass_map: Option<String>,
    /// The master's map whose fakes were last applied before travelling.
    last_pre_apply_map: Option<String>,
    /// Set while the follow has AntiLag on: the user's settings before (`None` when they
    /// couldn't be read).
    anti_lag: Option<Option<antilag::Snapshot>>,
    /// (username, `house-<CharID>`) of this account's own house.
    own_house: Option<(String, String)>,
}

impl<'a> Follower<'a> {
    fn new(bot: &'a Arc<Bot>, master: String, source: FollowSource, shared: Arc<Shared>) -> Self {
        Self {
            bot,
            master,
            source,
            shared,
            last_goto: None,
            last_jump: None,
            last_join: None,
            last_tick_log: None,
            goto_attempts: 0,
            direct_join_attempts: 0,
            direct_join_target: None,
            last_reported_off_goto: None,
            skill_class: None,
            skill_mode: "Base".into(),
            last_rescue_map: None,
            rescue_rounds: 0,
            blocked_map: None,
            blocked_room: None,
            room_full_attempts: 0,
            full_count_room: None,
            bad_reads: 0,
            suspended: false,
            was_logged_in: false,
            last_with_master: None,
            prior_options: None,
            last_bypass_map: None,
            last_pre_apply_map: None,
            anti_lag: None,
            own_house: None,
        }
    }

    fn settings(&self) -> ButlerSettings {
        self.bot.butler.settings()
    }

    fn stopping(&self) -> bool {
        self.shared.mode() != Mode::Run
    }

    fn run(mut self) {
        let bot = self.bot;
        let shared = self.shared.clone();
        let id = bot.events.subscribe(move |event| {
            if let BotEvent::ExtensionPacket(packet) = event {
                shared.on_packet(packet);
            }
        });
        let source = match self.source {
            FollowSource::Manual => "manual",
            FollowSource::Ordered => "summoned",
        };
        log(bot, format!("Following {} ({source}).", self.master));
        self.was_logged_in = player::logged_in(bot).unwrap_or(false);
        self.arm(false);
        while !self.stopping() {
            let ms = match self.tick() {
                Ok(ms) => ms,
                Err(e) => {
                    debug(bot, format!("Follow tick failed: {e}"));
                    1000
                }
            };
            self.nap(ms);
        }
        bot.events.unsubscribe(id);
        self.finish();
    }

    /// Prepares the character (VibeSkua `Arm`): the class and its skills, the level and quest
    /// fakes, AntiLag, Infinite Range and Skip Cutscenes. Again after every relogin, as a
    /// disconnect undoes all of it.
    fn arm(&mut self, fresh_login: bool) {
        let bot = self.bot;
        self.stop_skills();
        if let Err(e) = self.setup_class(fresh_login) {
            error(bot, format!("Class setup failed: {e}"));
        }
        let settings = self.settings();
        if settings.fake_level {
            quest_bypass::apply_level_fake(bot);
        }
        self.last_bypass_map = None;
        self.last_pre_apply_map = None;
        // Unlock All maxes every slot: the bypasses would only be overwritten.
        if settings.quest_unlock_all {
            quest_bypass::unlock_all(bot);
        } else if settings.quest_bypass {
            let ids = quest_bypass::parse_custom_ids(&settings.quest_bypass_custom);
            quest_bypass::apply_custom(bot, &ids, &self.shared.cancel);
        }
        self.apply_anti_lag(&settings);
        self.apply_butler_options();
    }

    /// AntiLag on, when the option for this follow's source is (VibeSkua `ApplyAntiLag`). The
    /// user's settings are saved the first time only: a relogin re-applies over its own.
    fn apply_anti_lag(&mut self, settings: &ButlerSettings) {
        let want = match self.source {
            FollowSource::Ordered => settings.anti_lag_summoned,
            FollowSource::Manual => settings.anti_lag_following,
        };
        if !want {
            return;
        }
        let bot = self.bot;
        if self.anti_lag.is_none() {
            let snapshot = antilag::capture(bot)
                .map_err(|e| {
                    error(bot, format!("AntiLag capture failed: {e}"));
                })
                .ok();
            self.anti_lag = Some(snapshot);
            debug(bot, format!("AntiLag ON (source={:?})", self.source));
        }
        if let Err(e) = antilag::set(bot, true) {
            error(bot, format!("AntiLag on failed: {e}"));
        }
    }

    /// Puts the user's AntiLag settings back (VibeSkua `RevertAntiLag`).
    fn revert_anti_lag(&mut self) {
        if let Some(snapshot) = self.anti_lag.take() {
            antilag::restore(self.bot, snapshot.as_ref());
            debug(
                self.bot,
                "AntiLag OFF (restored the settings from before the follow)",
            );
        }
    }

    /// The quest fakes for the map the butler just entered: Unlock All again, or the map's
    /// bypasses.
    fn apply_quest_access(&mut self, settings: &ButlerSettings) -> BotResult<()> {
        if !settings.quest_unlock_all && !settings.quest_bypass {
            return Ok(());
        }
        let map = map::name(self.bot)?;
        if self
            .last_bypass_map
            .as_deref()
            .is_some_and(|m| m.eq_ignore_ascii_case(&map))
        {
            return Ok(());
        }
        self.last_bypass_map = Some(map.clone());
        if settings.quest_unlock_all {
            quest_bypass::unlock_all(self.bot);
        } else {
            quest_bypass::apply_for_map(self.bot, &map, &self.shared.cancel);
        }
        Ok(())
    }

    /// Equips the CoreBots class picked for this account's class type (VibeSkua
    /// `SetupClassAndSkills`). With no class type, a fresh login re-equips the worn class so
    /// the client reloads its skills. The skills start later, once fighting beside the master:
    /// starting, pausing to travel and starting again breaks the rotation.
    fn setup_class(&mut self, fresh_login: bool) -> BotResult<()> {
        let bot = self.bot;
        self.skill_class = None;
        self.skill_mode = "Base".into();
        let me = player::username(bot)?;
        if me.is_empty() {
            return Ok(());
        }
        let class_type = self.settings().account(&me).class_type;
        let mut class = None;
        if let Some((class_key, mode_key)) = class_type.cbo_keys() {
            class = cbo_value(bot, &me, class_key);
            match &class {
                None => log(
                    bot,
                    format!(
                        "No CoreBots {class_type:?} class set for {me} - keeping the current class."
                    ),
                ),
                Some(_) => {
                    if let Some(mode) = cbo_value(bot, &me, mode_key) {
                        self.skill_mode = mode;
                    }
                }
            }
        } else if fresh_login {
            class = inventory::current_class(bot)?;
        }
        let Some(class) = class else {
            return Ok(());
        };
        log(bot, format!("Equipping class: {class}"));
        inventory::equip_item(bot, NameOrId::Name(&class))?;
        for _ in 0..6 {
            std::thread::sleep(Duration::from_millis(250));
            if inventory::current_class(bot)?
                .is_some_and(|w| w.trim().eq_ignore_ascii_case(class.trim()))
            {
                self.skill_class = Some(class);
                return Ok(());
            }
        }
        let worn = inventory::current_class(bot)?;
        log(
            bot,
            format!(
                "Couldn't equip {class} - not in the inventory? Keeping {}.",
                worn.as_deref().unwrap_or("the current class")
            ),
        );
        self.skill_class = worn;
        Ok(())
    }

    /// Infinite Range (hit the master's target from anywhere) and Skip Cutscenes, for the
    /// follow; the values before are restored when it ends.
    fn apply_butler_options(&mut self) {
        let mut o = self.bot.options.write();
        self.prior_options
            .get_or_insert((o.infinite_range, o.skip_cutscenes));
        o.infinite_range = true;
        o.skip_cutscenes = true;
    }

    fn revert_butler_options(&mut self) {
        if let Some((range, cutscenes)) = self.prior_options.take() {
            let mut o = self.bot.options.write();
            o.infinite_range = range;
            o.skip_cutscenes = cutscenes;
        }
    }

    /// One pass of the loop (VibeSkua `Run`); how long to wait before the next.
    fn tick(&mut self) -> BotResult<u64> {
        let bot = self.bot;
        if !player::logged_in(bot)? {
            self.was_logged_in = false;
            return Ok(1000);
        }
        if !player::alive(bot)? {
            return Ok(1000);
        }
        // Relogged: the class, skills and options died with the connection. The login
        // shows a beat before the character has loaded, so wait for it.
        if !self.was_logged_in {
            self.was_logged_in = true;
            debug(bot, "Relogin detected - waiting for the character to load");
            self.wait_for_character();
            self.arm(true);
        }
        // A script in this client takes the character over until it stops.
        if bot.scripts.is_running() {
            if !self.suspended {
                self.suspended = true;
                self.stop_skills();
                self.stop_attacking(true)?;
                log(
                    bot,
                    format!(
                        "A script is running - following {} is suspended until it stops.",
                        self.master
                    ),
                );
                bot.butler.ask_suspend(&self.master);
            }
            return Ok(1000);
        }
        if self.suspended {
            self.suspended = false;
            bot.butler.drop_suspend_notice();
            self.goto_attempts = 0;
            // A stopping script turns Skip Cutscenes and Lag Killer off.
            self.apply_butler_options();
            self.apply_anti_lag(&self.settings());
            log(
                bot,
                format!("Script stopped - following {} again.", self.master),
            );
        }
        if self.shared.parked() {
            self.force_aggro_off();
        }

        let master = bot.butler.dir.read_user(&self.master);
        if self
            .last_tick_log
            .is_none_or(|t| t.elapsed() >= Duration::from_secs(10))
        {
            self.last_tick_log = Some(Instant::now());
            debug(
                bot,
                format!(
                    "tick: map={} cell={} parked={} | master {:?}",
                    map::name(bot).unwrap_or_default(),
                    player::cell(bot).unwrap_or_default(),
                    self.shared.parked(),
                    master
                        .as_ref()
                        .map(|m| (m.map_with_room(), &m.cell, m.logged_in, m.beat)),
                ),
            );
        }
        let now = sync::now();
        let Some(m) = master.filter(|m| m.is_online(now)) else {
            // One bad read can be a file caught mid-write: three in a row is offline.
            self.bad_reads += 1;
            if self.bad_reads >= 3 {
                self.park_once(&format!(
                    "{} looks offline. Parked, waiting for them to return.",
                    self.master
                ));
            }
            return Ok(1000);
        };
        self.bad_reads = 0;

        // The server's "ignoring goto" may come before the master's file says goto is off:
        // keep joining directly until the file says it's on again.
        if self.last_reported_off_goto == Some(true) && !m.off_goto {
            self.shared.goto_off.store(false, Ordering::Release);
            self.goto_attempts = 0;
            self.direct_join_attempts = 0;
            self.direct_join_target = None;
            self.blocked_map = None;
            self.blocked_room = None;
            debug(bot, format!("{} turned goto on again", self.master));
        }
        self.last_reported_off_goto = Some(m.off_goto);

        let my_server = server_name(bot);
        if !m.server.is_empty() && !m.server.eq_ignore_ascii_case(&my_server) {
            self.park_once(&format!(
                "{} is on another server ({}, this one {my_server}). Parked.",
                self.master, m.server
            ));
            return Ok(2000);
        }
        if let Some(blocked) = &self.blocked_map {
            if blocked.eq_ignore_ascii_case(&m.map) {
                return Ok(2000);
            }
            debug(
                bot,
                format!("The master left the blocked map {blocked} - trying again"),
            );
            self.blocked_map = None;
        }
        if let Some(blocked) = &self.blocked_room {
            if blocked.eq_ignore_ascii_case(&m.map_with_room()) {
                return Ok(2000);
            }
            debug(
                bot,
                format!("The master left the full room {blocked} - trying again"),
            );
            self.blocked_room = None;
        }
        if self.shared.parked.swap(false, Ordering::AcqRel) {
            log(bot, format!("{} is back - following again.", self.master));
        }

        let locked = self.shared.locked_zone.swap(false, Ordering::AcqRel);
        let full = self.shared.room_full.swap(false, Ordering::AcqRel);
        if locked || full {
            return self.on_warning(&m, full);
        }

        // In the master's room? Both titles come from the same game UI.
        let my_room = self.my_map_room()?;
        if my_room.is_empty() {
            return Ok(LOOP_MS);
        }
        // A new map: the quest fakes again (entry gates and locked cells).
        let settings = self.settings();
        self.apply_quest_access(&settings)?;
        let same_room = my_room.eq_ignore_ascii_case(&m.map_with_room());
        if Some(same_room) != self.last_with_master {
            self.last_with_master = Some(same_room);
            debug(
                bot,
                format!(
                    "room: mine={my_room} master={} -> same={same_room}",
                    m.map_with_room()
                ),
            );
        }
        if !same_room {
            return self.travel(&m);
        }
        self.direct_join_attempts = 0;
        self.direct_join_target = None;
        self.rescue_rounds = 0;
        self.last_rescue_map = None;
        self.room_full_attempts = 0;
        self.full_count_room = None;
        self.goto_attempts = 0;
        if settings.leech_mode {
            // Stay in the room at Enter, never fight.
            self.stop_skills();
            self.stop_attacking(true)?;
            if !player::cell(bot)?.eq_ignore_ascii_case("Enter") && self.jump_due() {
                debug(bot, "leech: jump to Enter");
                self.last_jump = Some(Instant::now());
                self.jump("Enter", "Spawn")?;
            }
            return Ok(LOOP_MS);
        }
        // Goto is resolved by the server and always lands on the master: it fixes a butler
        // left in the wrong cell (after a death) whose own reads look right.
        if settings.always_goto && !m.off_goto && !self.shared.goto_off.load(Ordering::Acquire) {
            self.try_goto(&settings)?;
        }
        // Only the cell matters: walking in a cell changes the pad, and chasing it stutters.
        let (cell, pad) = self.master_position(&m)?;
        if !player::cell(bot)?.eq_ignore_ascii_case(&cell) && self.jump_due() {
            debug(bot, format!("jump to {cell}/{pad}"));
            self.last_jump = Some(Instant::now());
            self.jump(&cell, &pad)?;
        }
        self.fight(&m, &settings)?;
        Ok(LOOP_MS)
    }

    /// A locked zone or a full room: join the master's room from its file. A full private
    /// room (>= 1000) never overflows, so after 3 warnings it waits for the master to move.
    fn on_warning(&mut self, m: &SyncData, full: bool) -> BotResult<u64> {
        let room = m.map_with_room();
        if full {
            if self
                .full_count_room
                .as_deref()
                .is_some_and(|r| r.eq_ignore_ascii_case(&room))
            {
                self.room_full_attempts += 1;
            } else {
                self.full_count_room = Some(room.clone());
                self.room_full_attempts = 1;
            }
            if self.room_full_attempts >= 3 {
                self.blocked_room = Some(room.clone());
                self.room_full_attempts = 0;
                self.full_count_room = None;
                self.park_once(&format!(
                    "{room} is full after 3 tries - parked until {} changes rooms.",
                    self.master
                ));
                return Ok(2000);
            }
            log(
                self.bot,
                format!(
                    "Room full - joining {room} directly (try {}).",
                    self.room_full_attempts
                ),
            );
        } else {
            log(
                self.bot,
                format!("Locked zone - joining {room} from {}'s file.", self.master),
            );
        }
        self.join_master_room(m)?;
        Ok(LOOP_MS)
    }

    /// Not in the master's room: goto, or join its room from its file.
    fn travel(&mut self, m: &SyncData) -> BotResult<u64> {
        let bot = self.bot;
        // The skills would pick new targets and drag the butler back into combat.
        if bot.skills.timer_running() && !bot.skills.is_paused() {
            debug(bot, "Pausing the skills to travel");
            bot.skills.pause();
        }
        self.stop_attacking(true)?;
        let settings = self.settings();
        // Entry gates are checked on arrival: fake the master's map's quests before going
        // there (doomvaultb sends a late butler back to battleon). Unlock All has them already.
        if !settings.quest_unlock_all
            && settings.quest_bypass
            && self
                .last_pre_apply_map
                .as_deref()
                .is_none_or(|p| !p.eq_ignore_ascii_case(&m.map))
        {
            self.last_pre_apply_map = Some(m.map.clone());
            // Counts as the arrival's too, or the quests would be faked twice.
            self.last_bypass_map = Some(m.map.clone());
            debug(bot, format!("pre-applying the quest fakes for {}", m.map));
            quest_bypass::apply_for_map(bot, &m.map, &self.shared.cancel);
        }
        let on_master_map = || map::name(bot).is_ok_and(|n| n.eq_ignore_ascii_case(&m.map));
        let room = m.map_with_room();
        if m.off_goto || self.shared.goto_off.load(Ordering::Acquire) {
            // Count real joins, not throttled ticks, so a room that can't be entered isn't
            // tried forever.
            self.goto_attempts = 0;
            if self
                .direct_join_target
                .as_deref()
                .is_none_or(|t| !t.eq_ignore_ascii_case(&room))
            {
                self.direct_join_target = Some(room.clone());
                self.direct_join_attempts = 0;
                debug(
                    bot,
                    format!("{} has goto off - joining {room} directly", self.master),
                );
            }
            if self.direct_join_attempts >= 3 {
                if on_master_map() {
                    self.blocked_room = Some(room.clone());
                    let reason = if m.map.eq_ignore_ascii_case("house") {
                        format!(
                            "Can't reach {} in {room} through their house - they may be visiting another house. Parked until they change rooms.",
                            self.master
                        )
                    } else {
                        format!(
                            "Can't enter {room} after 3 joins - parked until {} changes rooms.",
                            self.master
                        )
                    };
                    self.park_once(&reason);
                } else {
                    self.blocked_map = Some(m.map.clone());
                    self.park_once(&format!(
                        "Can't enter {} after 3 joins - parked until {} changes maps.",
                        m.map, self.master
                    ));
                }
                return Ok(2000);
            }
            if self.join_master_room(m)? {
                self.direct_join_attempts += 1;
            }
            return Ok(LOOP_MS);
        }
        let threshold = settings.rescue_threshold.clamp(1, 10);
        if self.goto_attempts < threshold {
            if self.try_goto(&settings)? {
                self.goto_attempts += 1;
            }
            return Ok(LOOP_MS);
        }
        self.goto_attempts = 0;
        if self
            .last_rescue_map
            .as_deref()
            .is_some_and(|r| r.eq_ignore_ascii_case(&m.map))
        {
            self.rescue_rounds += 1;
        } else {
            self.last_rescue_map = Some(m.map.clone());
            self.rescue_rounds = 1;
        }
        if self.rescue_rounds >= 3 {
            // On the master's map but not its room: a full public room overflowed the join
            // into another room, so only that room is blocked. Off the map: an entry gate
            // sent the butler back (a quest it hasn't done), so the map is.
            self.rescue_rounds = 0;
            if on_master_map() {
                self.blocked_room = Some(room.clone());
                self.park_once(&format!(
                    "{room} is full after 3 rescues - parked until {} changes rooms.",
                    self.master
                ));
            } else {
                self.blocked_map = Some(m.map.clone());
                self.park_once(&format!(
                    "Can't enter {} after 3 rescues - a quest may lock it. Parked until {} changes maps.",
                    m.map, self.master
                ));
            }
            return Ok(2000);
        }
        debug(
            bot,
            format!(
                "Goto didn't land after {threshold} tries - joining from the file (round {})",
                self.rescue_rounds
            ),
        );
        self.join_master_room(m)?;
        Ok(LOOP_MS)
    }

    /// Fights beside the master; with passive attack only while it fights (VibeSkua
    /// `HandleCombat`). An idle passive butler pauses its skills (stopping and starting
    /// them again would reset the rotation).
    fn fight(&self, m: &SyncData, settings: &ButlerSettings) -> BotResult<()> {
        if !settings.passive_attack || m.attacking {
            self.start_skills()?;
            combat::attack(self.bot, NameOrId::Name("*"))?;
        } else {
            self.bot.skills.pause();
            self.stop_attacking(true)?;
        }
        Ok(())
    }

    fn start_skills(&self) -> BotResult<()> {
        let bot = self.bot;
        if bot.skills.timer_running() {
            bot.skills.resume();
            return Ok(());
        }
        let class = match &self.skill_class {
            Some(class) => class.clone(),
            None => inventory::current_class(bot)?.unwrap_or_else(|| "generic".into()),
        };
        debug(
            bot,
            format!("Starting the skills: {class} ({})", self.skill_mode),
        );
        bot.skills
            .start_advanced(bot, Some(&class), false, &self.skill_mode)
    }

    fn stop_skills(&self) {
        self.bot.skills.resume();
        self.bot.skills.stop();
    }

    /// Drops the target; with `deaggro` jumps in place, which drops the monsters' aggro too
    /// (cancelling the target alone leaves them hitting the butler).
    fn stop_attacking(&self, deaggro: bool) -> BotResult<()> {
        let bot = self.bot;
        if !player::has_target(bot)? && !player::in_combat(bot)? {
            return Ok(());
        }
        combat::cancel_auto_attack(bot)?;
        combat::cancel_target(bot)?;
        if deaggro {
            let (cell, pad) = (player::cell(bot)?, player::pad(bot)?);
            self.jump(&non_empty(cell, "Enter"), &non_empty(pad, "Spawn"))?;
        }
        Ok(())
    }

    fn jump_due(&self) -> bool {
        self.last_jump.is_none_or(|t| t.elapsed() >= JUMP_INTERVAL)
    }

    /// Without auto-correct: every target is a real cell, and the game's correction path is
    /// broken by an AQW update (VibeSkua and Butler v3 jump the same way).
    fn jump(&self, cell: &str, pad: &str) -> BotResult<()> {
        map::jump(self.bot, cell, pad, false, false)
    }

    /// The master's cell and pad: from the game when it's in this room, else its file.
    fn master_position(&self, m: &SyncData) -> BotResult<(String, String)> {
        let info: Option<Value> = self
            .bot
            .flash
            .get_key("world.uoTree", &self.master.to_lowercase())?;
        let field = |key: &str| {
            info.as_ref()
                .and_then(|i| i.get(key))
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
        };
        match field("strFrame") {
            Some(cell) => {
                let pad = field("strPad").unwrap_or_else(|| m.pad.clone());
                Ok((cell, pad))
            }
            None => Ok((m.cell.clone(), m.pad.clone())),
        }
    }

    /// Goto the master, at most every Goto delay; whether one was sent.
    fn try_goto(&mut self, settings: &ButlerSettings) -> BotResult<bool> {
        let delay = Duration::from_millis(settings.goto_delay.min(60_000));
        if self.last_goto.is_some_and(|t| t.elapsed() < delay) {
            return Ok(false);
        }
        self.last_goto = Some(Instant::now());
        debug(self.bot, format!("goto {}", self.master));
        player::goto(self.bot, &self.master)?;
        Ok(true)
    }

    /// Joins the master's room from its file, at most every 5 s per room (VibeSkua
    /// `JoinMasterRoom`): rejoining every tick keeps reloading the room. Whether it tried.
    fn join_master_room(&mut self, m: &SyncData) -> BotResult<bool> {
        let room = m.map_with_room();
        if self
            .last_join
            .as_ref()
            .is_some_and(|(r, t)| r.eq_ignore_ascii_case(&room) && t.elapsed() < JOIN_INTERVAL)
        {
            return Ok(false);
        }
        self.last_join = Some((room.clone(), Instant::now()));
        let bot = self.bot;
        debug(bot, format!("join {room} {}/{}", m.cell, m.pad));
        self.stop_attacking(false)?;
        if m.map.eq_ignore_ascii_case("house") {
            // The server ignores transfers to house-<number>: enter the master's own house
            // by name, then check it's the room its file names.
            let before = self.my_map_room()?;
            crate::api::send::packet(bot, &format!("%xt%zm%house%1%{}%", m.username), "String")?;
            for _ in 0..24 {
                if !self.nap(250) {
                    break;
                }
                let current = self.my_map_room()?;
                if current.eq_ignore_ascii_case(&room) {
                    debug(bot, format!("House join reached {current}"));
                    return Ok(true);
                }
                if current.starts_with("house-") && !current.eq_ignore_ascii_case(&before) {
                    self.blocked_room = Some(room.clone());
                    self.park_once(&format!(
                        "The house join reached {current}, but {} is in {room}. Parked until they change rooms.",
                        m.username
                    ));
                    return Ok(true);
                }
            }
            return Ok(true);
        }
        if map::name(bot)?.eq_ignore_ascii_case(&m.map) {
            // Another room of the same map: `map::join` only acts when the map changes.
            map::join_packet(bot, &room, &m.cell, &m.pad)?;
        } else {
            map::join(bot, &room, &m.cell, &m.pad, true, false)?;
        }
        wait::for_map_load(bot, &m.map, 20)?;
        Ok(true)
    }

    /// This account's `map-room` as the broadcast writes it; empty while loading.
    fn my_map_room(&self) -> BotResult<String> {
        let bot = self.bot;
        if !map::loaded(bot)? {
            return Ok(String::new());
        }
        let name = map::name(bot)?.to_lowercase();
        if name.is_empty() || name == "-1" {
            return Ok(String::new());
        }
        Ok(super::room_of(&map::full_name(bot)?)
            .map(|room| format!("{name}-{room}"))
            .unwrap_or_default())
    }

    /// Up to 10 s for the character to load after a relogin.
    fn wait_for_character(&self) {
        let bot = self.bot;
        for _ in 0..40 {
            let ready = player::playing(bot).unwrap_or(false)
                && map::loaded(bot).unwrap_or(false)
                && inventory::current_class(bot).ok().flatten().is_some();
            if ready || !self.nap(250) {
                break;
            }
        }
    }

    /// Sleeps in 100 ms slices; false when the follow was asked to stop. While parked, each
    /// slice turns aggro off again.
    fn nap(&self, ms: u64) -> bool {
        let deadline = Instant::now() + Duration::from_millis(ms);
        loop {
            if self.stopping() {
                return false;
            }
            if self.shared.parked() {
                self.force_aggro_off();
            }
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return true;
            }
            std::thread::sleep(left.min(Duration::from_millis(100)));
        }
    }

    /// A parked butler never fights, whatever turned aggro on (not restored afterwards).
    fn force_aggro_off(&self) {
        let mut o = self.bot.options.write();
        o.aggro_monsters = false;
        o.aggro_all_monsters = false;
    }

    fn park_once(&mut self, reason: &str) {
        if !self.shared.parked.swap(true, Ordering::AcqRel) {
            self.park(reason, false);
        }
    }

    /// Goes to the park spot (VibeSkua `Park`): the own house (several tries across the
    /// post-combat cooldown), else whitemap. `last` is the park after the follow ended,
    /// which only a new follow interrupts; otherwise a stop interrupts it too.
    fn park(&mut self, reason: &str, last: bool) {
        let bot = self.bot;
        log(bot, reason);
        self.shared.parking.store(true, Ordering::Release);
        if let Err(e) = self.park_inner(last) {
            debug(bot, format!("Parking failed: {e}"));
        }
        self.force_aggro_off();
        self.shared.parking.store(false, Ordering::Release);
    }

    fn park_inner(&mut self, last: bool) -> BotResult<()> {
        let bot = self.bot;
        self.force_aggro_off();
        self.stop_skills();
        self.stop_attacking(true)?;
        let spot = self.settings().park;
        let here = self.my_map_room()?;
        match spot {
            ParkSpot::Stay => return Ok(()),
            ParkSpot::House if self.in_own_house(&here)? => return Ok(()),
            ParkSpot::Whitemap if here.starts_with("whitemap-") => return Ok(()),
            _ => {}
        }
        // The server refuses house (and some map) joins in combat.
        for _ in 0..12 {
            if !player::in_combat(bot)? || !self.park_wait(250, last) {
                break;
            }
        }
        let has_house = inventory::items(bot, Container::House)?
            .iter()
            .any(|h| h.equipped);
        if spot == ParkSpot::House && has_house {
            // A "too soon after combat" cooldown outlives the combat flag: wait it out
            // first, then retry across it.
            if !self.park_wait(3000, last) {
                return Ok(());
            }
            for attempt in 1..=3 {
                if self.join_own_house(last)? {
                    return Ok(());
                }
                if attempt < 3 && !self.park_wait(5000, last) {
                    return Ok(());
                }
            }
            // Still in combat: an aggro room the same-cell jump can't shed. Enter is
            // usually free of monsters.
            if player::in_combat(bot)? {
                let _ = self.jump("Enter", "Spawn");
                for _ in 0..12 {
                    if !player::in_combat(bot)? || !self.park_wait(250, last) {
                        break;
                    }
                }
                if !self.park_wait(4000, last) || self.join_own_house(last)? {
                    return Ok(());
                }
            }
            if self.in_own_house(&self.my_map_room()?)? {
                return Ok(());
            }
            debug(bot, "The house join didn't land - parking in whitemap");
        }
        if self.park_interrupted(last) {
            return Ok(());
        }
        self.force_aggro_off();
        map::join(bot, PARK_MAP, "Enter", "Spawn", false, false)
    }

    /// One house join; whether it landed in this account's own house within 6 s.
    fn join_own_house(&mut self, last: bool) -> BotResult<bool> {
        let bot = self.bot;
        let me = player::username(bot)?;
        crate::api::send::packet(bot, &format!("%xt%zm%house%1%{me}%"), "String")?;
        for _ in 0..24 {
            if !self.park_wait(250, last) {
                return Ok(false);
            }
            if self.in_own_house(&self.my_map_room()?)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn park_interrupted(&self, last: bool) -> bool {
        match self.shared.mode() {
            Mode::Run => false,
            Mode::Abort => true,
            _ => !last,
        }
    }

    /// Sleeps in slices, turning aggro off; false when the park is interrupted.
    fn park_wait(&self, ms: u64, last: bool) -> bool {
        let deadline = Instant::now() + Duration::from_millis(ms);
        loop {
            self.force_aggro_off();
            if self.park_interrupted(last) {
                return false;
            }
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return true;
            }
            std::thread::sleep(left.min(Duration::from_millis(100)));
        }
    }

    /// The own house is `house-<CharID>` (not `myAvatar.uid`, a session ID); other players'
    /// houses are `house-` too.
    fn in_own_house(&mut self, room: &str) -> BotResult<bool> {
        let me = player::username(self.bot)?;
        if me.is_empty() {
            return Ok(false);
        }
        if self
            .own_house
            .as_ref()
            .is_none_or(|(u, _)| !u.eq_ignore_ascii_case(&me))
        {
            let id: i64 = self.bot.flash.get_or("world.myAvatar.objData.CharID", 0)?;
            if id <= 0 {
                return Ok(false);
            }
            self.own_house = Some((me, format!("house-{id}")));
        }
        Ok(self
            .own_house
            .as_ref()
            .is_some_and(|(_, house)| house.eq_ignore_ascii_case(room)))
    }

    /// The end of the follow (VibeSkua `Stop`): skills and attacks off, the options back,
    /// and on a release the park spot (unless already parked there).
    fn finish(&mut self) {
        let bot = self.bot;
        let mode = self.shared.mode();
        self.stop_skills();
        if mode != Mode::Abort {
            let _ = self.stop_attacking(true);
        }
        self.revert_anti_lag();
        self.revert_butler_options();
        bot.butler.drop_suspend_notice();
        if mode == Mode::StopAndPark {
            let home = self.settings().park == ParkSpot::House
                && self
                    .my_map_room()
                    .and_then(|r| self.in_own_house(&r))
                    .unwrap_or(false);
            if !self.shared.parked() || (self.settings().park == ParkSpot::House && !home) {
                self.park("Released by the master.", true);
            }
        }
        log(bot, format!("Stopped following {}.", self.master));
    }
}

fn non_empty(value: String, default: &str) -> String {
    if value.is_empty() {
        default.into()
    } else {
        value
    }
}

/// A CoreBots Options value of the account (`FarmClassSelect: Name`).
fn cbo_value(bot: &Bot, username: &str, key: &str) -> Option<String> {
    let lines = crate::cbo::read_lines(&bot.paths.cbo_file(username), |_, _| {}).ok()??;
    crate::cbo::parse(&lines)
        .into_iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(key))
        .map(|(_, v)| v)
        .filter(|v| !v.is_empty())
}
