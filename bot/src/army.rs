//! Launching accounts and steering them together (VibeSkua's Manager "Start" and the tabbed
//! host's "Army Control"). Every account runs in its own `turu` process, as in VibeSkua:
//!
//! * The process is started with VibeSkua's flags (`--user`, `--server`, `--run-script` /
//!   `--load-script`, `--headless`), the password in `TURU_PASSWORD` (not on the command
//!   line), and `--exit-with-parent`: its stdin is a pipe from here, and it quits when that
//!   closes, so clients don't outlive the window that launched them (even when it crashes).
//! * It runs the control API (`crate::control_api`) on a free local port with a token made
//!   for it (`TURU_API_TOKEN`). Army commands are requests to that API, where VibeSkua sends
//!   window messages; this process's own clients (its tabs) take them through
//!   `control_api::local`.
//! * A "Turu Army" thread asks every client for `/snapshot` every [`POLL_INTERVAL`] and hands
//!   the answer to the web monitor's hub ([`crate::monitor::Hub`]), where Army Control reads it.

use crate::Bot;
use parking_lot::Mutex;
use serde_json::Value;
use std::io::Write as _;
use std::net::{Ipv4Addr, SocketAddr, TcpListener};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command as Process, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};
use std::time::{Duration, Instant};

/// How often the clients are asked for their status.
pub const POLL_INTERVAL: Duration = Duration::from_secs(2);
/// VibeSkua's staggered launch: 500 ms plus up to 400 ms between two accounts.
const LAUNCH_DELAY_MS: u64 = 500;
const LAUNCH_JITTER_MS: u64 = 400;
/// `/snapshot` answers at once; logins and joins can take a minute.
const STATUS_TIMEOUT: Duration = Duration::from_secs(3);
pub const COMMAND_TIMEOUT: Duration = Duration::from_secs(120);

/// What to launch.
#[derive(Debug, Clone, Default)]
pub struct Launch {
    pub username: String,
    /// Shown in the Army window.
    pub label: String,
    /// `None`: the client opens on the login screen.
    pub password: Option<String>,
    pub server: Option<String>,
    pub script: Option<PathBuf>,
    /// Start the script once logged in, not only load it.
    pub run_script: bool,
    pub headless: bool,
}

/// How a client's process is doing; its game is in the hub.
#[derive(Debug, Clone, Default)]
pub struct ClientState {
    /// Its control API has answered once.
    pub answered: bool,
    /// Why it didn't answer.
    pub error: Option<String>,
    /// The process ended, with this description of its exit status.
    pub exited: Option<String>,
}

/// A client this process launched.
pub struct Client {
    pub username: String,
    pub label: String,
    pub headless: bool,
    pub started: Instant,
    /// What it was launched with, without the password (Restart reads it again).
    launch: Launch,
    api: SocketAddr,
    token: String,
    child: Mutex<Child>,
    /// Kept open: the client quits when it closes (`--exit-with-parent`).
    stdin: Mutex<Option<ChildStdin>>,
    state: Mutex<ClientState>,
}

impl Client {
    pub fn state(&self) -> ClientState {
        self.state.lock().clone()
    }

    pub fn is_running(&self) -> bool {
        self.state.lock().exited.is_none()
    }

    pub fn pid(&self) -> u32 {
        self.child.lock().id()
    }

    /// Ends the process.
    pub fn kill(&self) {
        drop(self.stdin.lock().take());
        let mut child = self.child.lock();
        let _ = child.kill();
        let _ = child.wait();
    }

    fn url(&self, path_and_query: &str) -> String {
        format!("http://{}{path_and_query}", self.api)
    }

    /// Notes when the process has ended.
    fn reap(&self) -> bool {
        if self.state.lock().exited.is_some() {
            return true;
        }
        match self.child.lock().try_wait() {
            Ok(Some(status)) => {
                let mut state = self.state.lock();
                state.exited = Some(status.to_string());
                true
            }
            Ok(None) => false,
            Err(e) => {
                self.state.lock().exited = Some(e.to_string());
                true
            }
        }
    }
}

/// An Army Control command (VibeSkua's `WM_SKUA_*` messages).
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    /// Start the loaded script, or load and start this one.
    StartScript(Option<String>),
    StopScript,
    LoadScript(String),
    Login,
    Logout,
    JumpMap {
        map: String,
        cell: String,
        pad: String,
    },
    JumpPlayer(String),
    JumpHouse,
    /// Register a quest (VibeSkua's "Accept Quest to All" registers it with this reward).
    AcceptQuest {
        id: i64,
        reward: i64,
    },
    /// Set bot options (Options > Game names, camelCase).
    SetOptions(Value),
    Relogin,
    /// Put an item on (name or ID); with `stop`, stop the running script first.
    Equip {
        item: String,
        stop: bool,
    },
    /// Load the bank into the gear file; with `stop`, stop the running script first.
    LoadBank {
        stop: bool,
    },
    /// Write the gear file from the game now.
    RefreshGear,
}

impl Command {
    /// What it says, for the logs.
    pub fn label(&self) -> String {
        match self {
            Self::StartScript(None) => "Start script".into(),
            Self::StartScript(Some(path)) => format!("Start {path}"),
            Self::StopScript => "Stop script".into(),
            Self::LoadScript(path) => format!("Load {path}"),
            Self::Login => "Log in".into(),
            Self::Logout => "Log out".into(),
            Self::JumpMap { map, .. } => format!("Join {map}"),
            Self::JumpPlayer(name) => format!("Go to {name}"),
            Self::JumpHouse => "Go to house".into(),
            Self::AcceptQuest { id, .. } => format!("Register quest {id}"),
            Self::SetOptions(values) => format!("Set {values}"),
            Self::Relogin => "Relog".into(),
            Self::Equip { item, .. } => format!("Equip {item}"),
            Self::LoadBank { .. } => "Load bank".into(),
            Self::RefreshGear => "Read gear".into(),
        }
    }

    /// The control API request: POST or GET, the path and query, and the body.
    pub fn request(&self) -> (bool, String, Vec<u8>) {
        let with = |path: &str, pairs: &[(&str, &str)]| {
            let mut url = reqwest::Url::parse("http://turu").expect("a valid URL");
            url.set_path(path);
            if !pairs.is_empty() {
                url.query_pairs_mut().extend_pairs(pairs);
            }
            match url.query() {
                Some(query) => format!("{}?{query}", url.path()),
                None => url.path().to_owned(),
            }
        };
        let (path, body) = match self {
            Self::StartScript(None) => (with("/script/start", &[]), Vec::new()),
            Self::StartScript(Some(path)) => (with("/script/start", &[("path", path)]), Vec::new()),
            Self::StopScript => (with("/script/stop", &[]), Vec::new()),
            Self::LoadScript(path) => (with("/script/load", &[("path", path)]), Vec::new()),
            Self::Login => (with("/login", &[]), Vec::new()),
            Self::Logout => (with("/logout", &[]), Vec::new()),
            Self::JumpMap { map, cell, pad } => {
                let mut pairs = vec![("map", map.as_str())];
                if !cell.trim().is_empty() {
                    pairs.push(("cell", cell));
                }
                if !pad.trim().is_empty() {
                    pairs.push(("pad", pad));
                }
                (with("/map/join", &pairs), Vec::new())
            }
            Self::JumpPlayer(name) => (with("/player/goto", &[("name", name)]), Vec::new()),
            Self::JumpHouse => (with("/house", &[]), Vec::new()),
            Self::AcceptQuest { id, reward } => {
                let (id, reward) = (id.to_string(), reward.to_string());
                (
                    with("/quests/register", &[("id", &id), ("reward", &reward)]),
                    Vec::new(),
                )
            }
            Self::SetOptions(values) => (with("/options", &[]), values.to_string().into_bytes()),
            Self::Relogin => (with("/relogin", &[]), Vec::new()),
            Self::Equip { item, stop } => {
                let mut pairs = vec![("item", item.as_str())];
                if *stop {
                    pairs.push(("stop", "1"));
                }
                (with("/equip", &pairs), Vec::new())
            }
            Self::LoadBank { stop } => {
                let pairs: &[(&str, &str)] = if *stop { &[("stop", "1")] } else { &[] };
                (with("/bank/load", pairs), Vec::new())
            }
            Self::RefreshGear => return (false, with("/gear", &[]), Vec::new()),
        };
        (true, path, body)
    }
}

/// The clients this process launched.
pub struct Army {
    /// The `turu` executable to launch.
    exe: PathBuf,
    clients: Mutex<Vec<Arc<Client>>>,
    http: reqwest::blocking::Client,
    polling: AtomicBool,
    /// Launches waiting for their turn (the staggered batch).
    queued: Mutex<Vec<String>>,
    /// Where the clients' snapshots go.
    hub: Mutex<Option<Arc<crate::monitor::Hub>>>,
}

impl Army {
    pub fn new(exe: PathBuf) -> Arc<Self> {
        Arc::new(Self {
            exe,
            clients: Mutex::default(),
            http: reqwest::blocking::Client::new(),
            polling: AtomicBool::new(false),
            queued: Mutex::default(),
            hub: Mutex::default(),
        })
    }

    pub fn set_hub(&self, hub: Arc<crate::monitor::Hub>) {
        *self.hub.lock() = Some(hub);
    }

    fn forget_in_hub(&self, username: &str) {
        if let Some(hub) = self.hub.lock().clone() {
            hub.remove(&crate::monitor::hub::army_id(username));
        }
    }

    pub fn clients(&self) -> Vec<Arc<Client>> {
        self.clients.lock().clone()
    }

    /// The running client of `username`.
    pub fn client(&self, username: &str) -> Option<Arc<Client>> {
        self.clients
            .lock()
            .iter()
            .find(|c| c.username.eq_ignore_ascii_case(username) && c.is_running())
            .cloned()
    }

    /// Usernames waiting in a staggered launch.
    pub fn queued(&self) -> Vec<String> {
        self.queued.lock().clone()
    }

    /// Starts a client now.
    pub fn launch(self: &Arc<Self>, launch: Launch) -> Result<Arc<Client>, String> {
        if self.client(&launch.username).is_some() {
            return Err(format!("{} is running already", launch.username));
        }
        let port = free_port()?;
        let token = crate::control_api::new_token()?;
        let mut process = Process::new(&self.exe);
        process
            .arg(format!("--api=127.0.0.1:{port}"))
            .arg("--exit-with-parent")
            .env("TURU_API_TOKEN", &token)
            .env_remove("TURU_PASSWORD")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        if !launch.username.trim().is_empty() {
            process.arg("--user").arg(&launch.username);
        }
        if let Some(password) = launch.password.as_deref().filter(|p| !p.is_empty()) {
            process.env("TURU_PASSWORD", password);
        }
        if let Some(server) = launch.server.as_deref().filter(|s| !s.trim().is_empty()) {
            process.arg("--server").arg(server);
        }
        if let Some(script) = &launch.script {
            let flag = if launch.run_script {
                "--run-script"
            } else {
                "--load-script"
            };
            process.arg(flag).arg(script);
        }
        if launch.headless {
            process.arg("--headless");
        }
        let mut child = process
            .spawn()
            .map_err(|e| format!("couldn't start {}: {e}", self.exe.display()))?;
        let stdin = child.stdin.take();
        let kept = Launch {
            password: launch.password.as_ref().map(|_| String::new()),
            ..launch.clone()
        };
        let client = Arc::new(Client {
            label: if launch.label.trim().is_empty() {
                launch.username.clone()
            } else {
                launch.label
            },
            username: launch.username,
            headless: launch.headless,
            started: Instant::now(),
            launch: kept,
            api: SocketAddr::from((Ipv4Addr::LOCALHOST, port)),
            token,
            child: Mutex::new(child),
            stdin: Mutex::new(stdin),
            state: Mutex::default(),
        });
        {
            let mut clients = self.clients.lock();
            clients.retain(|c| !c.username.eq_ignore_ascii_case(&client.username));
            clients.push(client.clone());
        }
        self.start_polling();
        Ok(client)
    }

    /// Starts the clients one after another, 500-900 ms apart (VibeSkua's batch launch), on
    /// a "Turu Launch" thread; `done` gets each one's outcome.
    pub fn launch_all(
        self: &Arc<Self>,
        launches: Vec<Launch>,
        done: impl Fn(&str, Result<(), String>) + Send + 'static,
    ) {
        if launches.is_empty() {
            return;
        }
        self.queued
            .lock()
            .extend(launches.iter().map(|l| l.username.clone()));
        let army = self.clone();
        let spawned = std::thread::Builder::new()
            .name("Turu Launch".into())
            .spawn(move || {
                for (i, launch) in launches.into_iter().enumerate() {
                    if i > 0 {
                        std::thread::sleep(launch_delay());
                    }
                    let username = launch.username.clone();
                    army.dequeue(&username);
                    done(&username, army.launch(launch).map(|_| ()));
                }
            });
        if let Err(e) = spawned {
            self.queued.lock().clear();
            tracing::error!("Couldn't start the launch thread: {e}");
        }
    }

    fn dequeue(&self, username: &str) {
        let mut queued = self.queued.lock();
        if let Some(i) = queued.iter().position(|u| u == username) {
            queued.remove(i);
        }
    }

    /// Ends the client and forgets it.
    pub fn close(&self, username: &str) {
        let removed: Vec<_> = {
            let mut clients = self.clients.lock();
            let (removed, kept) = std::mem::take(&mut *clients)
                .into_iter()
                .partition(|c| c.username.eq_ignore_ascii_case(username));
            *clients = kept;
            removed
        };
        for client in removed {
            client.kill();
        }
        self.forget_in_hub(username);
    }

    /// Ends the client and launches it again as it was, with its password from the keychain
    /// when it was launched with one. Blocks on the keychain: not on the UI thread.
    pub fn restart(self: &Arc<Self>, username: &str) -> Result<Arc<Client>, String> {
        let Some(old) = self
            .clients()
            .into_iter()
            .find(|c| c.username.eq_ignore_ascii_case(username))
        else {
            return Err(format!("{username} isn't one of the launched clients"));
        };
        let mut launch = old.launch.clone();
        if launch.password.is_some() {
            launch.password = crate::accounts::password(&launch.username)?;
        }
        self.close(username);
        self.launch(launch)
    }

    /// Forgets the clients that have ended.
    pub fn clear_exited(&self) {
        let ended: Vec<String> = {
            let mut clients = self.clients.lock();
            let ended = clients
                .iter()
                .filter(|c| !c.is_running())
                .map(|c| c.username.clone())
                .collect();
            clients.retain(|c| c.is_running());
            ended
        };
        for username in ended {
            self.forget_in_hub(&username);
        }
    }

    /// Ends every client (the window is closing).
    pub fn close_all(&self) {
        for client in std::mem::take(&mut *self.clients.lock()) {
            client.kill();
        }
    }

    /// Sends a request to one client; the reply's JSON, or why it failed.
    pub fn send(&self, client: &Client, command: &Command) -> Result<Value, String> {
        let (post, path, body) = command.request();
        self.request(client, post, &path, body, COMMAND_TIMEOUT)
    }

    /// Sends the command to the running clients (those `only` names, by username, or all),
    /// and to `local` (this process's own clients, named), all at once; `done` gets each
    /// one's outcome, labelled, as it comes.
    pub fn broadcast(
        self: &Arc<Self>,
        command: Command,
        local: Vec<(String, Arc<Bot>)>,
        only: Option<&[String]>,
        done: impl Fn(&str, Result<Value, String>) + Send + Sync + 'static,
    ) {
        let done = Arc::new(done);
        let command = Arc::new(command);
        let spawn = |name: String, job: Box<dyn FnOnce() -> Result<Value, String> + Send>| {
            let spawned = std::thread::Builder::new()
                .name("Turu Army Command".into())
                .spawn({
                    let (name, done) = (name.clone(), done.clone());
                    move || done(&name, job())
                });
            if let Err(e) = spawned {
                done(&name, Err(e.to_string()));
            }
        };
        for (name, bot) in local {
            let command = command.clone();
            spawn(name, Box::new(move || send_local(&bot, &command)));
        }
        let targeted = |c: &Client| {
            only.is_none_or(|names| names.iter().any(|n| n.eq_ignore_ascii_case(&c.username)))
        };
        for client in self
            .clients()
            .into_iter()
            .filter(|c| c.is_running() && targeted(c))
        {
            let (army, command) = (self.clone(), command.clone());
            spawn(
                client.label.clone(),
                Box::new(move || army.send(&client, &command)),
            );
        }
    }

    /// Sends any control API request to one client (the web monitor's commands and logs).
    pub fn forward(
        &self,
        client: &Client,
        post: bool,
        path: &str,
        body: Vec<u8>,
    ) -> Result<Value, String> {
        self.request(client, post, path, body, COMMAND_TIMEOUT)
    }

    fn request(
        &self,
        client: &Client,
        post: bool,
        path: &str,
        body: Vec<u8>,
        timeout: Duration,
    ) -> Result<Value, String> {
        let url = client.url(path);
        let request = if post {
            self.http.post(url).body(body)
        } else {
            self.http.get(url)
        };
        let response = request
            .bearer_auth(&client.token)
            .timeout(timeout)
            .send()
            .map_err(|e| e.without_url().to_string())?;
        let status = response.status().as_u16();
        let reply = response
            .text()
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or(Value::Null);
        reply_result(status, reply)
    }

    /// The "Turu Army" thread, once: refreshes every client's state until the army is gone.
    fn start_polling(self: &Arc<Self>) {
        if self.polling.swap(true, Ordering::AcqRel) {
            return;
        }
        let army: Weak<Self> = Arc::downgrade(self);
        let spawned = std::thread::Builder::new()
            .name("Turu Army".into())
            .spawn(move || {
                while let Some(army) = army.upgrade() {
                    for client in army.clients() {
                        if !client.reap() {
                            army.poll(&client);
                        }
                    }
                    drop(army);
                    std::thread::sleep(POLL_INTERVAL);
                }
            });
        if spawned.is_err() {
            self.polling.store(false, Ordering::Release);
        }
    }
}

impl Army {
    /// Asks the client for its snapshot and new events (the slow fields only while someone
    /// watches the monitor) and hands them to the hub.
    fn poll(self: &Arc<Self>, client: &Arc<Client>) {
        let hub = self.hub.lock().clone();
        let since = hub.as_ref().map_or(0, |h| h.next_event(&client.username));
        let slow = u8::from(crate::monitor::watched());
        let path = format!("/snapshot?since={since}&slow={slow}");
        let reply = self.request(client, false, &path, Vec::new(), STATUS_TIMEOUT);
        let reply = match reply {
            Ok(reply) => reply,
            Err(e) => {
                // Starting up (the API isn't listening yet), or busy.
                client.state.lock().error = Some(e);
                return;
            }
        };
        {
            let mut state = client.state.lock();
            state.answered = true;
            state.error = None;
        }
        let Some(hub) = hub else { return };
        let snapshot = serde_json::from_value(reply["snapshot"].clone());
        let events = serde_json::from_value(reply["events"].clone()).unwrap_or_default();
        let next = reply["next"].as_u64().unwrap_or(since);
        match snapshot {
            Ok(snapshot) => hub.record_army(self, client, snapshot, events, next),
            Err(e) => client.state.lock().error = Some(format!("a snapshot I can't read: {e}")),
        }
    }
}

impl Drop for Army {
    fn drop(&mut self) {
        self.close_all();
    }
}

/// Sends a command to one of this process's own clients (a tab), as [`Army::send`] does to
/// a launched one. Blocks as long as the route does.
pub fn send_local(bot: &Arc<Bot>, command: &Command) -> Result<Value, String> {
    let (post, path, body) = command.request();
    let (status, reply) = crate::control_api::local(bot, post, &path, &body);
    reply_result(status, reply)
}

/// A control API answer: the body when it succeeded, else its `error`.
pub fn reply_result(status: u16, reply: Value) -> Result<Value, String> {
    if (200..300).contains(&status) {
        Ok(reply)
    } else {
        Err(reply
            .get("error")
            .and_then(Value::as_str)
            .map_or_else(|| format!("HTTP {status}"), str::to_owned))
    }
}

/// How long to wait between two launches: 500-900 ms (VibeSkua's batch launch).
pub fn launch_delay() -> Duration {
    Duration::from_millis(LAUNCH_DELAY_MS + jitter(LAUNCH_JITTER_MS))
}

/// A local port nobody listens on (the client binds it a moment later).
fn free_port() -> Result<u16, String> {
    TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .and_then(|l| l.local_addr())
        .map(|a| a.port())
        .map_err(|e| format!("no free local port: {e}"))
}

fn jitter(max: u64) -> u64 {
    let mut bytes = [0u8; 8];
    if getrandom::fill(&mut bytes).is_err() {
        return max / 2;
    }
    u64::from_le_bytes(bytes) % (max + 1)
}

/// `--exit-with-parent`: quits the process (through `exit`) once stdin closes, which is when
/// the process that launched it ends.
pub fn exit_with_parent(exit: impl FnOnce() + Send + 'static) {
    let spawned = std::thread::Builder::new()
        .name("Turu Parent".into())
        .spawn(move || {
            let mut sink = std::io::sink();
            let _ = std::io::copy(&mut std::io::stdin().lock(), &mut sink);
            let _ = sink.flush();
            exit();
        });
    if let Err(e) = spawned {
        tracing::error!("--exit-with-parent: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn builds_requests() {
        let path = |c: Command| c.request().1;
        assert_eq!(path(Command::StartScript(None)), "/script/start");
        assert_eq!(
            path(Command::StartScript(Some("farm/Gold Farm.ts".into()))),
            "/script/start?path=farm%2FGold+Farm.ts"
        );
        assert_eq!(
            path(Command::JumpMap {
                map: "battleon-1e9".into(),
                cell: "Enter".into(),
                pad: String::new(),
            }),
            "/map/join?map=battleon-1e9&cell=Enter"
        );
        assert_eq!(
            path(Command::AcceptQuest {
                id: 2566,
                reward: -1
            }),
            "/quests/register?id=2566&reward=-1"
        );
        assert_eq!(
            path(Command::Equip {
                item: "Legion Revenant".into(),
                stop: false,
            }),
            "/equip?item=Legion+Revenant"
        );
        assert_eq!(
            path(Command::Equip {
                item: "Necrotic Sword of Doom".into(),
                stop: true,
            }),
            "/equip?item=Necrotic+Sword+of+Doom&stop=1"
        );
        assert_eq!(path(Command::LoadBank { stop: false }), "/bank/load");
        assert_eq!(path(Command::LoadBank { stop: true }), "/bank/load?stop=1");
        assert_eq!(path(Command::Relogin), "/relogin");
        assert!(!Command::RefreshGear.request().0);
        assert_eq!(path(Command::RefreshGear), "/gear");
        let (post, path, body) = Command::SetOptions(json!({ "lagKiller": true })).request();
        assert!(post);
        assert_eq!(path, "/options");
        assert_eq!(body, br#"{"lagKiller":true}"#);
    }

    #[test]
    fn reads_replies() {
        assert_eq!(reply_result(200, json!({ "a": 1 })), Ok(json!({ "a": 1 })));
        assert_eq!(
            reply_result(409, json!({ "error": "no script is running" })),
            Err("no script is running".into())
        );
        assert_eq!(reply_result(500, Value::Null), Err("HTTP 500".into()));
    }

    #[test]
    fn local_routes_answer_without_a_game() {
        let bot = Arc::new(Bot::new());
        let (post, path, _) = Command::JumpPlayer("Artix".into()).request();
        let (status, _) = crate::control_api::local(&bot, post, &path, &[]);
        // No player: the call fails, but the route is there.
        assert_ne!(status, 404);
        let (status, _) = crate::control_api::local(&bot, false, "/map/join?map=x", &[]);
        assert_eq!(status, 405);
        let (status, reply) = crate::control_api::local(&bot, true, "/quests/register", &[]);
        assert_eq!(status, 400, "{reply}");
    }
}
