//! The state every account running Butler publishes (VibeSkua ButlerV5 `SyncFile.cs`): one
//! file per account in `<data dir>/butler/<username>`, `key=value` lines. Tabs, clients in
//! their own window and windowless clients all read and write the same folder.
//!
//! Turu: an account counts as online while its file says `loggedin=1` and its `beat=` (Unix
//! seconds, rewritten every few seconds) is recent. VibeSkua checks the writer's PID instead,
//! which can't tell apart tabs of one process.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// How old a file's `beat` may be before the account counts as offline.
pub const STALE_AFTER: Duration = Duration::from_secs(15);

/// One account's published state (VibeSkua `SyncData`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncData {
    pub username: String,
    pub pid: u32,
    /// The game server's name ("Artix"), not its IP: several servers share a socket host.
    pub server: String,
    /// The equipped class.
    pub class_name: String,
    /// Lowercase.
    pub map: String,
    pub room: String,
    pub cell: String,
    pub pad: String,
    pub logged_in: bool,
    pub attacking: bool,
    /// The account's "Enable Goto" game preference is off.
    pub off_goto: bool,
    /// The accounts this one summons.
    pub followers: Vec<String>,
    /// Who this account follows ("" for nobody).
    pub following: String,
    pub script_on: bool,
    pub script_name: String,
    /// A master whose summon this account declined; that master releases it.
    pub declined: String,
    /// On its way to its park spot: not summonable yet.
    pub parking: bool,
    /// Turu: when the file was written (Unix seconds).
    pub beat: u64,
}

impl Default for SyncData {
    fn default() -> Self {
        Self {
            username: String::new(),
            pid: 0,
            server: String::new(),
            class_name: String::new(),
            map: String::new(),
            room: "1".into(),
            cell: "Enter".into(),
            pad: "Spawn".into(),
            logged_in: false,
            attacking: false,
            off_goto: false,
            followers: Vec::new(),
            following: String::new(),
            script_on: false,
            script_name: String::new(),
            declined: String::new(),
            parking: false,
            beat: 0,
        }
    }
}

fn flag(value: bool) -> char {
    if value { '1' } else { '0' }
}

impl SyncData {
    /// `map-room`, the name a join takes (`map` alone without a room).
    pub fn map_with_room(&self) -> String {
        if self.room.is_empty() {
            self.map.clone()
        } else {
            format!("{}-{}", self.map, self.room)
        }
    }

    pub fn lists_follower(&self, username: &str) -> bool {
        self.followers
            .iter()
            .any(|f| f.eq_ignore_ascii_case(username))
    }

    /// Logged in, and the file was written in the last [`STALE_AFTER`] (VibeSkua `IsOnline`).
    pub fn is_online(&self, now: u64) -> bool {
        self.logged_in && now.saturating_sub(self.beat) <= STALE_AFTER.as_secs()
    }

    /// Everything but `beat`, to tell a real change from a heartbeat.
    pub fn same_state(&self, other: &Self) -> bool {
        Self {
            beat: 0,
            ..self.clone()
        } == Self {
            beat: 0,
            ..other.clone()
        }
    }

    pub fn serialize(&self) -> String {
        let lines = [
            ("username", self.username.clone()),
            ("pid", self.pid.to_string()),
            ("server", self.server.clone()),
            ("class", self.class_name.clone()),
            ("map", self.map.clone()),
            ("room", self.room.clone()),
            ("cell", self.cell.clone()),
            ("pad", self.pad.clone()),
            ("loggedin", flag(self.logged_in).into()),
            ("attacking", flag(self.attacking).into()),
            ("offgoto", flag(self.off_goto).into()),
            ("followers", self.followers.join(",")),
            ("following", self.following.clone()),
            ("script", flag(self.script_on).into()),
            ("scriptname", self.script_name.clone()),
            ("declined", self.declined.clone()),
            ("parking", flag(self.parking).into()),
            ("beat", self.beat.to_string()),
        ];
        let mut text = String::new();
        for (key, value) in lines {
            // A value never spans lines (names and maps don't have newlines).
            let value = value.replace(['\n', '\r'], " ");
            text.push_str(&format!("{key}={value}\n"));
        }
        text
    }

    /// `None` without a username (a half-written or foreign file).
    pub fn parse(text: &str) -> Option<Self> {
        let mut data = Self::default();
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let (key, value) = (key.trim().to_ascii_lowercase(), value.trim());
            match key.as_str() {
                "username" => data.username = value.into(),
                "pid" => data.pid = value.parse().unwrap_or(0),
                "server" => data.server = value.into(),
                "class" => data.class_name = value.into(),
                "map" => data.map = value.to_lowercase(),
                "room" => data.room = value.into(),
                "cell" => data.cell = value.into(),
                "pad" => data.pad = value.into(),
                "loggedin" => data.logged_in = value == "1",
                "attacking" => data.attacking = value == "1",
                "offgoto" => data.off_goto = value == "1",
                "followers" => {
                    data.followers = value
                        .split(',')
                        .map(str::trim)
                        .filter(|f| !f.is_empty())
                        .map(str::to_owned)
                        .collect();
                }
                "following" => data.following = value.into(),
                "script" => data.script_on = value == "1",
                "scriptname" => data.script_name = value.into(),
                "declined" => data.declined = value.into(),
                "parking" => data.parking = value == "1",
                "beat" => data.beat = value.parse().unwrap_or(0),
                _ => {}
            }
        }
        (!data.username.is_empty()).then_some(data)
    }
}

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// The folder of sync files (VibeSkua `%APPDATA%\Skua\character_locations`).
#[derive(Debug, Clone)]
pub struct SyncDir {
    pub dir: PathBuf,
}

impl SyncDir {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// The account's file: its name in lowercase, kept to characters safe in a file name.
    pub fn path_for(&self, username: &str) -> PathBuf {
        let name: String = username
            .to_lowercase()
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || matches!(c, '_' | '-') {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        self.dir.join(format!("{name}.state"))
    }

    /// Writes a temp file and renames it over the account's file, so readers never see half
    /// of it. Windows refuses the rename while a reader has the file open: then it retries,
    /// and falls back to writing in place (a reader that catches it half written reads it
    /// again on the next tick).
    pub fn write(&self, data: &SyncData) -> io::Result<()> {
        fs::create_dir_all(&self.dir)?;
        let path = self.path_for(&data.username);
        let temp = path.with_extension(format!("tmp{}", std::process::id()));
        let text = data.serialize();
        for _ in 0..3 {
            fs::write(&temp, &text)?;
            if fs::rename(&temp, &path).is_ok() {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(30));
        }
        let _ = fs::remove_file(&temp);
        fs::write(&path, text)
    }

    pub fn read(path: &Path) -> Option<SyncData> {
        SyncData::parse(&fs::read_to_string(path).ok()?)
    }

    pub fn read_user(&self, username: &str) -> Option<SyncData> {
        Self::read(&self.path_for(username))
    }

    /// Every account's file.
    pub fn read_all(&self) -> Vec<SyncData> {
        let Ok(entries) = fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        entries
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "state"))
            .filter_map(|p| Self::read(&p))
            .collect()
    }

    /// The online accounts but `except`, by name (VibeSkua `Roster.GetOnline`).
    pub fn online(&self, except: Option<&str>) -> Vec<SyncData> {
        let now = now();
        let mut online: Vec<SyncData> = self
            .read_all()
            .into_iter()
            .filter(|d| d.is_online(now))
            .filter(|d| except.is_none_or(|e| !d.username.eq_ignore_ascii_case(e)))
            .collect();
        online.sort_by_key(|d| d.username.to_lowercase());
        online
    }
}

/// The online master that lists `username` among its followers (VibeSkua
/// `Roster.FindMasterOrdering`).
pub fn master_ordering<'a>(username: &str, online: &'a [SyncData]) -> Option<&'a SyncData> {
    online.iter().find(|d| d.lists_follower(username))
}
