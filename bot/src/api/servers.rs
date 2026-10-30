//! `bot.servers` (VibeSkua `IScriptServers`, `ScriptServers.cs`) and auto-relogin (VibeSkua
//! `ScriptInterface.OnLogout` / `Relogin`, started by the watchdog in `background.rs`).
//!
//! The waits here run either on the script thread (a script calling `bot.servers.relogin()`,
//! where Stop ends them) or on the "Relogin" thread of an auto-relogin (where
//! [`cancel_relogin`] ends them).

use super::{Args, ok, player};
use crate::Bot;
use crate::error::{BotError, BotResult};
use crate::events::BotEvent;
use crate::models::Server;
use parking_lot::Mutex;
use serde_json::Value;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// The game's server list API (VibeSkua `GetServers`).
pub const SERVERS_URL: &str = "http://content.aq.com/game/api/data/servers";
/// The game server port when a server doesn't say.
const DEFAULT_PORT: i64 = 5588;

type Fetcher = Arc<dyn Fn() -> Result<Vec<Server>, String> + Send + Sync>;

/// Server state shared by scripts, the watchdog and the UI.
pub struct ServerRunner {
    /// VibeSkua `CachedServers`: the last list from [`SERVERS_URL`].
    cached: Mutex<Vec<Server>>,
    /// VibeSkua `SetLoginInfo`: the login relogins use instead of the game's `loginInfo`.
    login_info: Mutex<Option<(String, String)>>,
    last_ip: Mutex<String>,
    last_name: Mutex<String>,
    fetcher: Mutex<Fetcher>,
    /// A background refresh of the list (for the UI) is running.
    refreshing: AtomicBool,
    /// VibeSkua `_waitForLogin`: an auto-relogin is under way.
    waiting_for_login: AtomicBool,
    /// Cancels the running auto-relogin (VibeSkua `_reloginCTS`).
    cancel: Mutex<Option<Arc<AtomicBool>>>,
}

impl Default for ServerRunner {
    fn default() -> Self {
        Self {
            cached: Mutex::new(Vec::new()),
            login_info: Mutex::new(None),
            last_ip: Mutex::new(String::new()),
            last_name: Mutex::new(String::new()),
            fetcher: Mutex::new(Arc::new(fetch_servers)),
            refreshing: AtomicBool::new(false),
            waiting_for_login: AtomicBool::new(false),
            cancel: Mutex::new(None),
        }
    }
}

impl ServerRunner {
    /// The servers from the last fetch of the server list (empty before the first).
    pub fn cached(&self) -> Vec<Server> {
        self.cached.lock().clone()
    }

    /// Replaces how the server list is fetched (tests use a fixed list).
    pub fn set_fetcher(
        &self,
        fetcher: impl Fn() -> Result<Vec<Server>, String> + Send + Sync + 'static,
    ) {
        *self.fetcher.lock() = Arc::new(fetcher);
    }

    pub fn last_ip(&self) -> String {
        self.last_ip.lock().clone()
    }

    pub fn last_name(&self) -> String {
        self.last_name.lock().clone()
    }

    /// An auto-relogin is under way.
    pub fn is_relogging(&self) -> bool {
        self.waiting_for_login.load(Ordering::Acquire)
    }

    /// Remembers the server we're on (the background timer, once per login).
    pub(crate) fn set_last(&self, ip: String, name: String) {
        if !ip.is_empty() {
            *self.last_ip.lock() = ip;
        }
        if !name.is_empty() {
            *self.last_name.lock() = name;
        }
    }
}

fn fetch_servers() -> Result<Vec<Server>, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())?;
    let text = client
        .get(SERVERS_URL)
        .send()
        .and_then(|r| r.error_for_status())
        .and_then(|r| r.text())
        .map_err(|e| e.to_string())?;
    serde_json::from_str(&text).map_err(|e| e.to_string())
}

/// VibeSkua `GetServers(forceUpdate)`: the cached list, or a fresh one when it's empty or
/// `force`. A failed fetch returns an empty list and keeps the cache.
pub fn get_servers(bot: &Bot, force: bool) -> Vec<Server> {
    if !force {
        let cached = bot.servers.cached.lock();
        if !cached.is_empty() {
            return cached.clone();
        }
    }
    let fetcher = bot.servers.fetcher.lock().clone();
    match fetcher() {
        Ok(servers) if !servers.is_empty() => {
            *bot.servers.cached.lock() = servers.clone();
            servers
        }
        Ok(_) => Vec::new(),
        Err(e) => {
            bot.logs
                .debug
                .push(format!("Couldn't get the server list: {e}"));
            Vec::new()
        }
    }
}

/// Fetches the server list on another thread if it isn't cached yet (for the UI).
pub fn refresh_in_background(bot: &Arc<Bot>) {
    if !bot.servers.cached.lock().is_empty() || bot.servers.refreshing.swap(true, Ordering::AcqRel)
    {
        return;
    }
    let thread_bot = bot.clone();
    let spawned = std::thread::Builder::new()
        .name("Turu Servers".into())
        .spawn(move || {
            get_servers(&thread_bot, false);
            thread_bot
                .servers
                .refreshing
                .store(false, Ordering::Release);
        });
    if spawned.is_err() {
        bot.servers.refreshing.store(false, Ordering::Release);
    }
}

/// VibeSkua `ServerList`: `serialCmd.servers`, the servers the game's login screen lists.
pub fn server_list(bot: &Bot) -> BotResult<Vec<Server>> {
    Ok(bot.flash.get_opt("serialCmd.servers")?.unwrap_or_default())
}

/// `sfc.isConnected` (false without `sfc`).
pub fn is_connected(bot: &Bot) -> BotResult<bool> {
    if bot.flash.is_null("sfc")? {
        return Ok(false);
    }
    bot.flash.get_or("sfc.isConnected", false)
}

/// VibeSkua `Player.Kicked`: the login screen shows its warning.
pub fn kicked(bot: &Bot) -> BotResult<bool> {
    bot.flash.call_bool("isKicked", vec![])
}

fn password(bot: &Bot) -> BotResult<String> {
    Ok(bot
        .flash
        .get_static::<String>("loginInfo.strPassword")?
        .unwrap_or_default())
}

/// VibeSkua `CachedUsername`: the `setLoginInfo` username, else the game's.
pub fn cached_username(bot: &Bot) -> BotResult<Option<String>> {
    if let Some((username, _)) = &*bot.servers.login_info.lock()
        && !username.trim().is_empty()
    {
        return Ok(Some(username.clone()));
    }
    let username = player::username(bot)?;
    Ok((!username.trim().is_empty() && username != "loginInfo.strUsername").then_some(username))
}

/// The `setLoginInfo` username, without asking the game (callable from any thread).
pub fn login_name(bot: &Bot) -> Option<String> {
    bot.servers
        .login_info
        .lock()
        .as_ref()
        .map(|(username, _)| username.clone())
        .filter(|u| !u.trim().is_empty())
}

pub fn set_login_info(bot: &Bot, username: &str, password: &str) {
    *bot.servers.login_info.lock() = Some((username.to_owned(), password.to_owned()));
}

fn set_intentional(bot: &Bot, value: bool) {
    bot.state.lock().intentional_logout = value;
}

/// VibeSkua `Login(username, password)`: the game's `login`.
pub fn login(bot: &Bot, username: &str, password: &str) -> BotResult<()> {
    set_intentional(bot, false);
    bot.flash
        .call_game_function("login", vec![username.into(), password.into()])
}

/// VibeSkua `Login()`: logs in with the game's remembered `loginInfo`.
pub fn login_again(bot: &Bot) -> BotResult<()> {
    let (username, password) = (player::username(bot)?, password(bot)?);
    login(bot, &username, &password)
}

/// The login relogins use: `setLoginInfo`'s, else the game's.
fn login_for_relogin(bot: &Bot) -> BotResult<()> {
    let info = bot.servers.login_info.lock().clone();
    match info {
        Some((username, password)) => login(bot, &username, &password),
        None => login_again(bot),
    }
}

/// VibeSkua `Logout()`: the game's `logout`, then back to the login screen. An intentional
/// logout: auto-relogin doesn't bring you back.
pub fn logout(bot: &Bot) -> BotResult<()> {
    // VibeSkua sets this after `game.logout`; before it, the `logout` packet the call sends
    // can't be taken for a disconnect.
    set_intentional(bot, true);
    cancel_relogin(bot);
    bot.flash.call_game_function("logout", vec![])?;
    disconnect_to_login(bot);
    Ok(())
}

/// VibeSkua `DisconnectForRelogin`: back to the login screen without an intentional logout.
pub fn disconnect_for_relogin(bot: &Bot) {
    set_intentional(bot, false);
    disconnect_to_login(bot);
}

fn disconnect_to_login(bot: &Bot) {
    let flash = &bot.flash;
    let _ = flash.call("resetConnectionState", vec![]);
    if is_connected(bot).unwrap_or(false) {
        let _ = flash.call_game_function("sfc.disconnect", vec![]);
    }
    let _ = flash.call("killLoginModals", vec![]);
    let _ = flash.call_game_function("gotoAndPlay", vec!["Login".into()]);
}

/// Where a wait runs: the script thread (Stop ends it) or the relogin thread (`cancel` ends it).
pub struct Ctx<'a> {
    bot: &'a Bot,
    cancel: Option<Arc<AtomicBool>>,
}

impl<'a> Ctx<'a> {
    /// For calls from scripts: only Stop ends the waits.
    pub fn new(bot: &'a Bot) -> Self {
        Self { bot, cancel: None }
    }

    fn check(&self) -> BotResult<()> {
        self.bot.control.check()?;
        if self
            .cancel
            .as_ref()
            .is_some_and(|c| c.load(Ordering::Acquire))
        {
            return Err(BotError::Stopped);
        }
        Ok(())
    }

    fn sleep(&self, ms: u64) -> BotResult<()> {
        let deadline = Instant::now() + Duration::from_millis(ms);
        loop {
            self.check()?;
            let now = Instant::now();
            if now >= deadline {
                return Ok(());
            }
            let left = (deadline - now).as_millis() as u64;
            self.bot.sleep(left.min(100))?;
        }
    }

    /// VibeSkua `Wait.ForTrue(predicate, tries)`: 100 ms tries; errors count as false.
    fn wait_for(
        &self,
        mut predicate: impl FnMut() -> BotResult<bool>,
        tries: i64,
    ) -> BotResult<bool> {
        let mut i = 0;
        loop {
            self.check()?;
            match predicate() {
                Ok(true) => return Ok(true),
                Err(BotError::Stopped) => return Err(BotError::Stopped),
                _ => {}
            }
            if i >= tries {
                return Ok(false);
            }
            i += 1;
            self.sleep(100)?;
        }
    }
}

fn in_world(bot: &Bot) -> bool {
    player::playing(bot).unwrap_or(false) && player::world_loaded(bot).unwrap_or(false)
}

fn login_tries(bot: &Bot) -> i64 {
    (bot.opts().login_timeout / 100).into()
}

/// After a connect: waits for the world (VibeSkua `LoginTimeout / 100` tries).
fn wait_for_world(ctx: &Ctx) -> BotResult<bool> {
    set_intentional(ctx.bot, false);
    ctx.wait_for(|| Ok(in_world(ctx.bot)), login_tries(ctx.bot))?;
    Ok(player::playing(ctx.bot).unwrap_or(false))
}

/// VibeSkua `ConnectIP(ip[, port])`: the game's `connectTo`, then waits for the world.
pub fn connect_ip(ctx: &Ctx, ip: &str, port: Option<i64>) -> BotResult<bool> {
    let mut args = vec![ip.into()];
    if let Some(port) = port {
        args.push(port.into());
    }
    ctx.bot.flash.call_game_function("connectTo", args)?;
    wait_for_world(ctx)
}

/// VibeSkua `ConnectToServer`: skua.swf's `connectToServer` (checks the server like the
/// game's server list does), then waits for the world.
fn connect_to_server(ctx: &Ctx, server: &Server) -> BotResult<bool> {
    ctx.bot
        .flash
        .call("connectToServer", vec![server.to_game_json().into()])?;
    wait_for_world(ctx)
}

/// VibeSkua `Connect(serverName)`: the first server on the login screen whose name contains
/// `name`. (VibeSkua passes the IP back to `Connect(string)`, which never connects.)
pub fn connect(ctx: &Ctx, name: &str) -> BotResult<bool> {
    let server = server_list(ctx.bot)?
        .into_iter()
        .find(|s| s.name.contains(name));
    match server {
        Some(server) => connect_ip(ctx, &server.ip, None),
        None => Ok(false),
    }
}

/// A server by name, or a whole server (a script passing `bot.servers.serverList[0]`).
pub enum ServerArg {
    Name(String),
    Server(Server),
}

impl ServerArg {
    fn from_value(value: &Value) -> BotResult<Option<Self>> {
        match value {
            Value::Null => Ok(None),
            Value::String(name) => Ok(Some(Self::Name(name.clone()))),
            Value::Object(_) => serde_json::from_value(value.clone())
                .map(|s| Some(Self::Server(s)))
                .map_err(|e| BotError::InvalidArgument(format!("not a server: {e}"))),
            _ => Err(BotError::InvalidArgument(
                "a server must be a name or a server object".into(),
            )),
        }
    }
}

/// VibeSkua `Reconnect`: logs in again, waits `login_delay` ms and connects.
pub fn reconnect(ctx: &Ctx, server: &ServerArg, login_delay: u64) -> BotResult<bool> {
    login_again(ctx.bot)?;
    ctx.sleep(login_delay)?;
    match server {
        ServerArg::Name(name) => connect(ctx, name),
        ServerArg::Server(server) => connect_ip(ctx, &server.ip, None),
    }
}

fn relogin_log(bot: &Bot, message: impl AsRef<str>) {
    bot.log(format!("[Relogin] {}", message.as_ref()));
}

/// Why a server can't be used for a relogin (VibeSkua `GetBlacklistReason`).
pub fn blacklist_reason(server: &Server, non_member: bool) -> Option<&'static str> {
    if server.name.trim().is_empty() || server.ip.trim().is_empty() {
        Some("invalid")
    } else if !server.online {
        Some("offline")
    } else if server.name.to_lowercase().contains("test") {
        Some("test server")
    } else if server.max_players > 0 && server.player_count >= server.max_players {
        Some("full")
    } else if server.upgrade && non_member {
        Some("member-only")
    } else {
        None
    }
}

/// VibeSkua `IsNonMemberAccount`.
fn is_non_member(bot: &Bot) -> bool {
    if let Ok(Some(days)) = bot.flash.get_static::<i64>("objLogin.iUpgDays") {
        return days < 0;
    }
    if let Ok(Some(days)) = bot.flash.get_opt::<i64>("world.myAvatar.objData.iUpgDays") {
        return days < 0;
    }
    false
}

/// VibeSkua `BuildReloginCandidates`: the usable servers of the API list, then of the login
/// screen, once per name.
pub fn relogin_candidates(cached: &[Server], listed: &[Server], non_member: bool) -> Vec<Server> {
    let mut candidates: Vec<Server> = Vec::new();
    for server in cached.iter().chain(listed) {
        if blacklist_reason(server, non_member).is_some()
            || candidates
                .iter()
                .any(|c| c.name.eq_ignore_ascii_case(&server.name))
        {
            continue;
        }
        candidates.push(server.clone());
    }
    candidates
}

fn candidates(bot: &Bot) -> Vec<Server> {
    let cached = bot.servers.cached();
    let listed = server_list(bot).unwrap_or_default();
    relogin_candidates(&cached, &listed, is_non_member(bot))
}

/// The options [`select_server`] reads.
pub struct Preferences<'a> {
    /// `AutoReloginAny`: any server but the last one.
    pub any: bool,
    /// `ReloginServer`.
    pub relogin_server: &'a str,
    pub last_ip: &'a str,
}

impl Preferences<'_> {
    fn with<T>(bot: &Bot, f: impl FnOnce(&Preferences) -> T) -> T {
        let (any, relogin_server) = {
            let o = bot.opts();
            (o.auto_relogin_any, o.relogin_server.clone())
        };
        let last_ip = bot.servers.last_ip();
        f(&Preferences {
            any,
            relogin_server: &relogin_server,
            last_ip: &last_ip,
        })
    }
}

/// VibeSkua `FindByName`. `tried` holds lowercase names.
fn find_by_name<'s>(
    candidates: &'s [Server],
    name: Option<&str>,
    exact: bool,
    tried: Option<&HashSet<String>>,
) -> Option<&'s Server> {
    let name = name.filter(|n| !n.trim().is_empty())?.to_lowercase();
    candidates.iter().find(|s| {
        let server = s.name.to_lowercase();
        let matches = if exact {
            server == name
        } else {
            server.contains(&name)
        };
        matches && tried.is_none_or(|t| !t.contains(&server))
    })
}

/// VibeSkua `SelectReloginServer`: the preferred server, else (with `AutoReloginAny`) any
/// other than the last, else `ReloginServer`, else the first; servers in `attempted` go last,
/// and once all were tried the list starts over.
pub fn select_server(
    candidates: &[Server],
    preferred: Option<&str>,
    attempted: Option<&mut HashSet<String>>,
    prefs: &Preferences,
) -> Option<Server> {
    if candidates.is_empty() {
        return None;
    }
    let empty = HashSet::new();
    {
        let tried = attempted.as_deref().unwrap_or(&empty);
        let untried = |s: &&Server| !tried.contains(&s.name.to_lowercase());
        let found = find_by_name(candidates, preferred, true, Some(tried))
            .or_else(|| {
                prefs
                    .any
                    .then(|| {
                        candidates
                            .iter()
                            .filter(untried)
                            .find(|s| s.ip != prefs.last_ip)
                    })
                    .flatten()
            })
            .or_else(|| find_by_name(candidates, Some(prefs.relogin_server), true, Some(tried)))
            .or_else(|| candidates.iter().find(untried));
        if let Some(server) = found {
            return Some(server.clone());
        }
    }
    if let Some(attempted) = attempted {
        attempted.clear();
    }
    find_by_name(candidates, preferred, true, None)
        .or_else(|| {
            prefs
                .any
                .then(|| candidates.iter().find(|s| s.ip != prefs.last_ip))
                .flatten()
        })
        .or_else(|| find_by_name(candidates, Some(prefs.relogin_server), true, None))
        .or_else(|| candidates.first())
        .cloned()
}

/// VibeSkua `SelectReloginServerByName`.
fn select_by_name(
    bot: &Bot,
    name: &str,
    exact: bool,
    mut attempted: Option<&mut HashSet<String>>,
) -> Option<Server> {
    let candidates = candidates(bot);
    if let Some(server) = find_by_name(&candidates, Some(name), exact, attempted.as_deref()) {
        return Some(server.clone());
    }
    Preferences::with(bot, |prefs| {
        select_server(&candidates, Some(name), attempted.take(), prefs)
    })
}

fn select(bot: &Bot) -> Option<Server> {
    let candidates = candidates(bot);
    Preferences::with(bot, |prefs| select_server(&candidates, None, None, prefs))
}

/// VibeSkua `RefreshServersSync`: fetches the list unless it's cached.
fn refresh_servers(bot: &Bot) {
    if bot.servers.cached.lock().is_empty() {
        let n = get_servers(bot, true).len();
        relogin_log(
            bot,
            format!("Refreshed server list ({n} servers from API)."),
        );
    }
}

/// Turns auto-relogin off while a script relogs (VibeSkua's `autoRelogSwitch`), and back.
struct AutoReloginOff<'a> {
    bot: &'a Bot,
    was: bool,
}

impl<'a> AutoReloginOff<'a> {
    fn new(bot: &'a Bot) -> Self {
        let was = std::mem::replace(&mut bot.options.write().auto_relogin, false);
        Self { bot, was }
    }
}

impl Drop for AutoReloginOff<'_> {
    fn drop(&mut self) {
        self.bot.options.write().auto_relogin = self.was;
    }
}

/// The start of every scripted relogin: log out (not intentionally) and log in again.
fn log_out_and_in(ctx: &Ctx) -> BotResult<()> {
    let bot = ctx.bot;
    // VibeSkua's LogoutRequestedMessage cancels an auto-relogin.
    cancel_relogin(bot);
    bot.events.emit(BotEvent::Logout);
    disconnect_for_relogin(bot);
    bot.state.lock().stats.relogins += 1;
    login_for_relogin(bot)
}

/// Waits for the world after a relogin's connect and logs how it went.
fn finish_relogin(ctx: &Ctx, success: &str, failure: &str) -> BotResult<bool> {
    ctx.wait_for(|| Ok(in_world(ctx.bot)), login_tries(ctx.bot))?;
    let connected = player::playing(ctx.bot).unwrap_or(false);
    relogin_log(ctx.bot, if connected { success } else { failure });
    Ok(connected)
}

fn load_policy_and_clear(bot: &Bot, ip: &str, port: i64) {
    let _ = bot
        .flash
        .call("loadPolicyFile", vec![ip.into(), port.into()]);
    let _ = bot.flash.call("killLoginModals", vec![]);
}

/// VibeSkua `Relogin(Server)`: logs out and back in on `server` (or the best other one when
/// it can't be used, or none was given).
pub fn relogin(ctx: &Ctx, server: Option<Server>) -> BotResult<bool> {
    let bot = ctx.bot;
    refresh_servers(bot);
    let non_member = is_non_member(bot);
    let server = server.filter(|s| match blacklist_reason(s, non_member) {
        Some(reason) => {
            relogin_log(
                bot,
                format!("Skipping requested server {} ({reason}).", s.name),
            );
            false
        }
        None => true,
    });
    match server.or_else(|| select(bot)) {
        Some(server) => relogin_ip(ctx, &server.ip),
        None => {
            relogin_log(bot, "No eligible relogin server found.");
            Ok(false)
        }
    }
}

/// VibeSkua `ReloginIP`.
pub fn relogin_ip(ctx: &Ctx, ip: &str) -> BotResult<bool> {
    let bot = ctx.bot;
    if ip.trim().is_empty() {
        relogin_log(bot, "Cannot relogin: empty server IP.");
        return Ok(false);
    }
    let _off = AutoReloginOff::new(bot);
    relogin_log(bot, format!("Relogging using IP {ip}."));
    cancel_relogin(bot);
    ctx.sleep(2000)?;
    log_out_and_in(ctx)?;
    relogin_log(
        bot,
        format!("Waiting for server list screen before connecting to IP {ip}..."),
    );
    if !wait_for_server_list(ctx)? {
        relogin_log(
            bot,
            format!("Authentication timed out or froze while connecting to IP {ip}."),
        );
        return Ok(false);
    }
    ctx.sleep(100)?;
    load_policy_and_clear(bot, ip, DEFAULT_PORT);
    connect_ip(ctx, ip, None)?;
    finish_relogin(ctx, "Relogin by IP successful.", "Relogin by IP failed.")
}

/// VibeSkua `Relogin(serverName)`: the first usable server whose name contains `name`, else
/// the best other one.
pub fn relogin_named(ctx: &Ctx, name: &str) -> BotResult<bool> {
    let bot = ctx.bot;
    refresh_servers(bot);
    let lower = name.to_lowercase();
    let requested = bot
        .servers
        .cached()
        .into_iter()
        .chain(server_list(bot).unwrap_or_default())
        .find(|s| s.name.to_lowercase().contains(&lower));
    if let Some(requested) = requested
        && let Some(reason) = blacklist_reason(&requested, is_non_member(bot))
    {
        relogin_log(
            bot,
            format!(
                "Server {} is excluded ({reason}), selecting fallback.",
                requested.name
            ),
        );
    }
    let Some(server) = select_by_name(bot, name, false, None).or_else(|| select(bot)) else {
        relogin_log(bot, "No eligible relogin server found.");
        return Ok(false);
    };
    let _off = AutoReloginOff::new(bot);
    relogin_log(
        bot,
        format!(
            "Relogging on server {} [{}/{}].",
            server.name, server.player_count, server.max_players
        ),
    );
    log_out_and_in(ctx)?;
    relogin_log(
        bot,
        format!(
            "Waiting for server list screen before connecting to {}...",
            server.name
        ),
    );
    if !wait_for_server_list(ctx)? {
        relogin_log(
            bot,
            format!(
                "Authentication timed out or froze while connecting to {}.",
                server.name
            ),
        );
        return Ok(false);
    }
    ctx.sleep(100)?;
    let port = if server.port > 0 {
        server.port
    } else {
        DEFAULT_PORT
    };
    load_policy_and_clear(bot, &server.ip, port);
    connect_to_server(ctx, &server)?;
    finish_relogin(
        ctx,
        &format!("Relogin successful on {}.", server.name),
        &format!("Relogin failed on {}.", server.name),
    )
}

/// VibeSkua `EnsureRelogin([serverName])`: logs in (up to `ReloginTries` attempts, trying other
/// servers) unless you're already playing. Log out first.
pub fn ensure_relogin(ctx: &Ctx, name: Option<&str>) -> BotResult<bool> {
    let bot = ctx.bot;
    set_intentional(bot, false);
    get_servers(bot, true);
    ctx.check()?;
    let server = name
        .and_then(|n| select_by_name(bot, n, true, None))
        .or_else(|| select(bot));
    let Some(server) = server else {
        relogin_log(
            bot,
            match name {
                Some(n) => {
                    format!("No eligible servers available for requested relogin target {n}.")
                }
                None => "No eligible servers available for auto relogin.".into(),
            },
        );
        return Ok(false);
    };
    relogin_log(
        bot,
        match name {
            Some(n) => format!(
                "Auto relogin started for target {n}. Preferred server: {}.",
                server.name
            ),
            None => format!("Auto relogin started. Preferred server: {}.", server.name),
        },
    );
    ensure_login(ctx, &server)
}

/// VibeSkua `WaitReloginDelay`: `ReloginTryDelay` plus 400-1000 ms.
fn relogin_delay(ctx: &Ctx) -> BotResult<()> {
    let delay = ctx.bot.opts().relogin_try_delay;
    if delay <= 0 {
        return Ok(());
    }
    ctx.sleep(delay as u64 + jitter(400, 1000))
}

fn jitter(min: u64, max: u64) -> u64 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos() as u64);
    min + nanos % (max - min)
}

/// `mcConnDetail.txtDetail.text`, the loading screen's message ("null" when it's not shown).
pub fn conn_detail(bot: &Bot) -> String {
    if bot.flash.is_null("mcConnDetail.stage").unwrap_or(true) {
        return "null".into();
    }
    bot.flash
        .get_or("mcConnDetail.txtDetail.text", "null".to_owned())
        .unwrap_or_else(|_| "null".into())
}

/// VibeSkua `EnsureLogin`.
fn ensure_login(ctx: &Ctx, server: &Server) -> BotResult<bool> {
    let bot = ctx.bot;
    let max = bot.opts().relogin_tries;
    let mut tries = 0;
    let mut attempted = HashSet::new();
    let mut preferred = Some(server.name.clone());
    while !player::playing(bot).unwrap_or(false) && tries < max {
        ctx.check()?;
        tries += 1;
        let n = get_servers(bot, true).len();
        relogin_log(
            bot,
            format!("Attempt {tries}/{max}: refreshed server list ({n} servers)."),
        );
        let candidates = candidates(bot);
        let target = Preferences::with(bot, |prefs| {
            select_server(
                &candidates,
                preferred.as_deref(),
                Some(&mut attempted),
                prefs,
            )
        });
        let Some(target) = target else {
            relogin_log(
                bot,
                format!("Attempt {tries}/{max}: no eligible servers available."),
            );
            if tries < max {
                relogin_delay(ctx)?;
            }
            continue;
        };
        attempted.insert(target.name.to_lowercase());
        relogin_log(
            bot,
            format!(
                "Attempt {tries}/{max}: trying {} [{}/{}].",
                target.name, target.player_count, target.max_players
            ),
        );

        // Tear the last session down, and give the server time to drop it.
        if is_connected(bot).unwrap_or(false) || tries > 1 {
            disconnect_for_relogin(bot);
            let (safe, delay) = {
                let o = bot.opts();
                (o.safe_relogin, o.relogin_try_delay)
            };
            let backoff = if safe {
                (10000 + (tries - 2) * 3000).max(delay)
            } else {
                delay.max(4000)
            };
            relogin_log(
                bot,
                format!(
                    "Waiting {:.1}s ghost session backoff before attempt {tries}...",
                    backoff as f64 / 1000.0
                ),
            );
            ctx.sleep(backoff.max(0) as u64)?;
        }

        login_for_relogin(bot)?;
        relogin_log(
            bot,
            format!("Attempt {tries}/{max}: waiting for server list screen."),
        );
        if !wait_for_server_list(ctx)? {
            relogin_log(
                bot,
                format!("Attempt {tries}/{max}: server list screen not ready."),
            );
            preferred = None;
            if tries < max {
                relogin_delay(ctx)?;
            }
            continue;
        }

        let port = if target.port > 0 {
            target.port
        } else {
            DEFAULT_PORT
        };
        load_policy_and_clear(bot, &target.ip, port);
        let clicked = bot
            .flash
            .call_bool("clickServer", vec![target.name.as_str().into()])
            .unwrap_or(false);
        if !clicked {
            relogin_log(
                bot,
                format!(
                    "Server {} button not clickable in UI, initiating direct bridge connection to {}:{port}...",
                    target.name, target.ip
                ),
            );
            let direct = bot
                .flash
                .call("connectToServer", vec![target.to_game_json().into()]);
            if direct.is_err() {
                let port = (target.port > 0).then_some(target.port);
                let mut args = vec![target.ip.as_str().into()];
                args.extend(port.map(crate::FlashArg::from));
                bot.flash.call_game_function("connectTo", args)?;
            }
        }

        // At most 15 s for the world, so a stuck load doesn't hang the relogin.
        let deadline = Instant::now() + Duration::from_millis(server_list_timeout(bot));
        while !in_world(bot) && Instant::now() < deadline {
            let detail = conn_detail(bot);
            if ["lost", "banned", "closed"]
                .iter()
                .any(|w| detail.contains(w))
            {
                relogin_log(
                    bot,
                    format!("Connection dropped during world entry: {detail}"),
                );
                break;
            }
            ctx.sleep(250)?;
        }
        if in_world(bot) {
            relogin_log(
                bot,
                format!("Connected successfully to {}. World loaded.", target.name),
            );
            return Ok(true);
        }
        relogin_log(
            bot,
            format!(
                "Attempt {tries}/{max} failed on {}. Watchdog aborting attempt.",
                target.name
            ),
        );
        preferred = None;
        disconnect_for_relogin(bot);
        if tries < max {
            relogin_delay(ctx)?;
        }
    }
    let playing = player::playing(bot).unwrap_or(false);
    if !playing {
        relogin_log(bot, "Unable to reconnect within configured retries.");
    }
    Ok(playing)
}

/// `LoginTimeout`, at most 15 s.
fn server_list_timeout(bot: &Bot) -> u64 {
    let timeout = bot.opts().login_timeout;
    if timeout > 0 {
        timeout.min(15000) as u64
    } else {
        15000
    }
}

fn server_list_ready(bot: &Bot) -> bool {
    let flash = &bot.flash;
    flash.get_or("mcLogin.visible", false).unwrap_or(false)
        && !flash.is_null("mcLogin.sl.iList").unwrap_or(true)
        && flash
            .get_or::<i64>("mcLogin.sl.iList.numChildren", 0)
            .unwrap_or(0)
            > 0
        && server_list(bot)
            .unwrap_or_default()
            .iter()
            .any(|s| !s.name.trim().is_empty() && !s.ip.trim().is_empty())
}

/// VibeSkua `WaitForServerListReady`: after `login`, waits (at most 15 s) for the server list.
/// A login error dialog fails it at once; a ban ends the relogin.
fn wait_for_server_list(ctx: &Ctx) -> BotResult<bool> {
    let bot = ctx.bot;
    let deadline = Instant::now() + Duration::from_millis(server_list_timeout(bot));
    while Instant::now() < deadline {
        ctx.check()?;
        let modal = bot
            .flash
            .call_str("getActiveModalMessage", vec![])
            .unwrap_or_default();
        let modal = modal.trim();
        if !modal.is_empty() && modal != "null" {
            relogin_log(
                bot,
                format!("Login modal warning detected: '{modal}'. Clearing dialog."),
            );
            let _ = bot.flash.call("killLoginModals", vec![]);
            let lower = modal.to_lowercase();
            if lower.contains("banned") || lower.contains("disabled") {
                relogin_log(
                    bot,
                    format!(
                        "FATAL: Account lockout detected ('{modal}'). Halting auto re-login loop."
                    ),
                );
                set_intentional(bot, true);
                // VibeSkua swallows this and tries again; with the account locked that's no use.
                return Err(BotError::Other(format!("account lockout: {modal}")));
            }
            return Ok(false);
        }
        if server_list_ready(bot) {
            let _ = bot.flash.call("loadPolicyFiles", vec![]);
            return Ok(true);
        }
        ctx.sleep(250)?;
    }
    if server_list_ready(bot) {
        let _ = bot.flash.call("loadPolicyFiles", vec![]);
        return Ok(true);
    }
    relogin_log(
        bot,
        "Server list screen failed to load (authentication timeout or modal lock). Resetting login UI.",
    );
    let _ = bot.flash.call("killLoginModals", vec![]);
    let _ = bot.flash.call("resetConnectionState", vec![]);
    let _ = bot
        .flash
        .call_game_function("gotoAndPlay", vec!["Login".into()]);
    Ok(false)
}

/// VibeSkua `CancelRelogin`: ends a running auto-relogin.
pub fn cancel_relogin(bot: &Bot) {
    if let Some(cancel) = bot.servers.cancel.lock().take() {
        cancel.store(true, Ordering::Release);
    }
    bot.servers
        .waiting_for_login
        .store(false, Ordering::Release);
}

/// VibeSkua `BeginLogout`: unless auto-relogin is off, one is already running or the logout
/// was intentional, stops the script, waits, logs back in and restarts the script if it was
/// running. The work runs on a "Relogin" thread, so this can be called from anywhere (also
/// from `Bot::on_flash_call`).
pub fn begin_logout(bot: &Arc<Bot>, logout_notified: bool) {
    // A script holding the options lock only delays the check to the relogin thread.
    if bot.options.try_read().is_some_and(|o| !o.auto_relogin)
        || bot.state.lock().intentional_logout
    {
        return;
    }
    if bot.servers.waiting_for_login.swap(true, Ordering::AcqRel) {
        return;
    }
    let cancel = Arc::new(AtomicBool::new(false));
    *bot.servers.cancel.lock() = Some(cancel.clone());
    let thread_bot = bot.clone();
    let thread_cancel = cancel.clone();
    let spawned = std::thread::Builder::new()
        .name("Relogin".into())
        .spawn(move || {
            let bot = thread_bot;
            match auto_relogin(&bot, logout_notified, &thread_cancel) {
                Ok(()) => {}
                Err(BotError::Stopped) => bot.log("Auto re-login cancelled."),
                Err(e) => bot.log(format!("Auto re-login failed: {e}")),
            }
            finish_auto_relogin(&bot, &thread_cancel);
        });
    if spawned.is_err() {
        finish_auto_relogin(bot, &cancel);
    }
}

fn finish_auto_relogin(bot: &Bot, cancel: &Arc<AtomicBool>) {
    let mut current = bot.servers.cancel.lock();
    if current.as_ref().is_some_and(|c| Arc::ptr_eq(c, cancel)) {
        *current = None;
        bot.servers
            .waiting_for_login
            .store(false, Ordering::Release);
    }
}

/// VibeSkua `OnLogout` and `Relogin(delay, startScript)`.
fn auto_relogin(bot: &Arc<Bot>, logout_notified: bool, cancel: &Arc<AtomicBool>) -> BotResult<()> {
    if !bot.opts().auto_relogin || bot.state.lock().intentional_logout {
        return Ok(());
    }
    let ctx = Ctx {
        bot,
        cancel: Some(cancel.clone()),
    };
    bot.log("Auto re-login triggered.");
    let was_running = bot.scripts.is_running();
    if !logout_notified {
        bot.events.emit(BotEvent::Logout);
    }
    crate::script::stop(bot);
    while bot.scripts.is_running() {
        ctx.sleep(50)?;
    }
    ctx.check()?;

    let kicked = kicked(bot).unwrap_or(false);
    bot.events
        .emit(BotEvent::ReloginTriggered { was_kicked: kicked });
    let (safe, delay) = {
        let o = bot.opts();
        (o.safe_relogin, o.relogin_try_delay)
    };
    let delay = match (safe, kicked) {
        (true, true) => 20000,
        (true, false) => delay.max(10000),
        (false, _) => delay.max(800),
    };
    disconnect_for_relogin(bot);
    bot.log(format!("Waiting {delay}ms for re-login."));
    ctx.sleep(delay as u64)?;

    bot.state.lock().stats.relogins += 1;
    let relogged = ensure_relogin(&ctx, None)?;
    ctx.check()?;
    if was_running && relogged {
        if let Err(e) = crate::script::start_silent(bot) {
            bot.log(format!("Couldn't restart the script: {e}"));
        }
    } else if was_running {
        bot.log("Skipping script restart because re-login did not succeed.");
    }
    bot.log(format!(
        "Re-login was {}.",
        if relogged {
            "successful"
        } else {
            "cancelled or unsuccessful"
        }
    ));
    Ok(())
}

pub fn dispatch(bot: &Arc<Bot>, name: &str, args: Args) -> BotResult<Value> {
    let ctx = Ctx::new(bot);
    match name {
        "serverList" => ok(server_list(bot)?),
        "cachedServers" => ok(bot.servers.cached()),
        "getServers" => ok(get_servers(bot, args.bool_or(0, false))),
        "lastIp" => ok(bot.servers.last_ip()),
        "lastName" => ok(bot.servers.last_name()),
        "isIntentionalLogout" => ok(bot.state.lock().intentional_logout),
        "setIntentionalLogout" => {
            set_intentional(bot, args.bool_or(0, false));
            ok(())
        }
        "isConnected" => ok(is_connected(bot)?),
        "cachedUsername" => ok(cached_username(bot)?),
        "setLoginInfo" => {
            set_login_info(bot, args.str(0)?, args.str(1)?);
            ok(())
        }
        "login" => {
            match (args.opt_str(0), args.opt_str(1)) {
                (Some(username), Some(password)) => login(bot, username, password)?,
                _ => login_again(bot)?,
            }
            ok(())
        }
        "logout" => ok(logout(bot)?),
        "disconnectForRelogin" => {
            disconnect_for_relogin(bot);
            ok(())
        }
        "connect" => match ServerArg::from_value(args.value(0))? {
            Some(ServerArg::Name(name)) => ok(connect(&ctx, &name)?),
            Some(ServerArg::Server(server)) => ok(connect_ip(&ctx, &server.ip, None)?),
            None => Err(BotError::InvalidArgument(
                "argument 1 must be a server".into(),
            )),
        },
        "connectIp" => ok(connect_ip(&ctx, args.str(0)?, args.opt_i64(1))?),
        "reconnect" => {
            let Some(server) = ServerArg::from_value(args.value(0))? else {
                return Err(BotError::InvalidArgument(
                    "argument 1 must be a server".into(),
                ));
            };
            ok(reconnect(
                &ctx,
                &server,
                args.i64_or(1, 2000).max(0) as u64,
            )?)
        }
        "relogin" => match ServerArg::from_value(args.value(0))? {
            Some(ServerArg::Name(name)) => ok(relogin_named(&ctx, &name)?),
            Some(ServerArg::Server(server)) => ok(relogin(&ctx, Some(server))?),
            None => ok(relogin(&ctx, None)?),
        },
        "reloginIp" => ok(relogin_ip(&ctx, args.str(0)?)?),
        "ensureRelogin" => ok(ensure_relogin(&ctx, args.opt_str(0))?),
        _ => Err(BotError::UnknownMethod(format!("servers.{name}"))),
    }
}
