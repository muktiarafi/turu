//! `bot.army`: what VibeSkua's CoreArmyLite reads outside the game. Its sync log (C#
//! `ArmyLogging`, a file every client of the army appends to), the accounts of Army > Accounts
//! (C# `Bot.Accounts.GetAllAccounts()`, for the one-client scripts that log in each account in
//! turn) and the army's private room number (C# `getRoomNr`).
//!
//! Turu: every log operation holds the log's lock file (`army/.<name>.lock`), which stands in
//! for the machine-wide named mutex C# CoreUltra takes around its read-modify-write. A script
//! holds it across several operations with `bot.army.withLock`. `barrier` waits for the army
//! with a generation count, so a member can't miss the release.

use super::{Args, ok, servers};
use crate::Bot;
use crate::accounts::{self, Accounts};
use crate::error::{BotError, BotResult};
use chrono::{Datelike, Local, Timelike};
use parking_lot::Mutex;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// How long a client waits for a log's lock before giving up: a body under `withLock` is a few
/// file operations, so this is a client that hangs while holding it.
const LOCK_TIMEOUT: Duration = Duration::from_secs(30);
/// How often a client waiting for a lock tries again.
const LOCK_RETRY_MS: u64 = 20;
/// How often a member waiting at a barrier looks again and renews its arrival.
const BARRIER_TICK_MS: u64 = 250;
/// An arrival not renewed for this long is a member that left (a crash, a closed client).
const BARRIER_STALE_SECS: u64 = 15;

/// The army logs this client holds locked with `bot.army.withLock`, by name. Dropping a file
/// unlocks it; the script's end drops them all.
#[derive(Default)]
pub struct ArmyLocks(Mutex<HashMap<String, File>>);

impl ArmyLocks {
    fn holds(&self, name: &str) -> bool {
        self.0.lock().contains_key(name)
    }

    /// Unlocks every log (the script stopped, maybe inside `withLock`).
    pub fn release_all(&self) {
        self.0.lock().clear();
    }
}

/// `army/<name>.log` in the data folder, which every client of this machine shares (C# puts it
/// in Skua's options folder). The name must be a plain file name.
fn log_file(dir: &Path, name: &str) -> BotResult<PathBuf> {
    check_name(name)?;
    Ok(dir.join(format!("{name}.log")))
}

fn check_name(name: &str) -> BotResult<()> {
    let plain = !name.is_empty()
        && !name.starts_with('.')
        && name.chars().all(|c| {
            c.is_alphanumeric() || matches!(c, '_' | '-' | ' ' | '.' | '(' | ')' | '[' | ']')
        });
    if !plain {
        return Err(BotError::InvalidArgument(format!(
            "the army log name \"{name}\" must be a plain file name"
        )));
    }
    Ok(())
}

fn io_error(path: &std::path::Path, e: std::io::Error) -> BotError {
    BotError::Other(format!("{}: {e}", path.display()))
}

/// The log's lines (C# `ReadLog`); none when it doesn't exist yet.
pub fn read_log(bot: &Bot, name: &str) -> BotResult<Vec<String>> {
    locked(bot, name, |dir| read_log_in(dir, name))
}

fn read_log_in(dir: &Path, name: &str) -> BotResult<Vec<String>> {
    let path = log_file(dir, name)?;
    match fs::read_to_string(&path) {
        Ok(text) => Ok(text.lines().map(str::to_owned).collect()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(io_error(&path, e)),
    }
}

/// Appends a line (C# `WriteLog`). One `write` call, so lines from several clients don't mix.
pub fn append_log(bot: &Bot, name: &str, line: &str) -> BotResult<()> {
    locked(bot, name, |dir| append_log_in(dir, name, line))
}

fn append_log_in(dir: &Path, name: &str, line: &str) -> BotResult<()> {
    let path = log_file(dir, name)?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| io_error(dir, e))?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| io_error(&path, e))?;
    file.write_all(format!("{line}\n").as_bytes())
        .map_err(|e| io_error(&path, e))
}

/// Replaces the log's lines (C# CoreUltra's `File.WriteAllLines` on its sync file): written to a
/// temporary file and renamed over the log, so a reader never sees half of it.
pub fn write_log(bot: &Bot, name: &str, lines: &[String]) -> BotResult<()> {
    locked(bot, name, |dir| write_log_in(dir, name, lines))
}

fn write_log_in(dir: &Path, name: &str, lines: &[String]) -> BotResult<()> {
    // Each write gets its own temporary file: the clients of one process (tabs) write the same
    // log at once, and a shared name would be renamed away under another client's write.
    static WRITES: AtomicU64 = AtomicU64::new(0);
    let path = log_file(dir, name)?;
    fs::create_dir_all(dir).map_err(|e| io_error(dir, e))?;
    let write = WRITES.fetch_add(1, Ordering::Relaxed);
    let temp = dir.join(format!(".{name}.{}.{write}.tmp", std::process::id()));
    let text: String = lines.iter().map(|l| format!("{l}\n")).collect();
    fs::write(&temp, text).map_err(|e| io_error(&temp, e))?;
    fs::rename(&temp, &path).map_err(|e| io_error(&path, e))
}

/// Empties the log (C# `ClearLogFile`).
pub fn clear_log(bot: &Bot, name: &str) -> BotResult<()> {
    locked(bot, name, |dir| clear_log_in(dir, name))
}

fn clear_log_in(dir: &Path, name: &str) -> BotResult<()> {
    let path = log_file(dir, name)?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| io_error(dir, e))?;
    }
    fs::write(&path, "").map_err(|e| io_error(&path, e))
}

/// `army/.<name>.lock`, which every operation on the log `name` holds (the log itself is
/// renamed over, so it can't carry the lock).
fn lock_path(dir: &Path, name: &str) -> BotResult<PathBuf> {
    check_name(name)?;
    Ok(dir.join(format!(".{name}.lock")))
}

/// Opens and locks the log's lock file, trying again every [`LOCK_RETRY_MS`] through `sleep`
/// (so Stop interrupts the wait) for up to [`LOCK_TIMEOUT`]. Clients of one process (tabs)
/// open the file apart, so they exclude each other as other processes do.
fn acquire(dir: &Path, name: &str, sleep: impl Fn(u64) -> BotResult<()>) -> BotResult<File> {
    let path = lock_path(dir, name)?;
    fs::create_dir_all(dir).map_err(|e| io_error(dir, e))?;
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&path)
        .map_err(|e| io_error(&path, e))?;
    let started = Instant::now();
    loop {
        match file.try_lock() {
            Ok(()) => return Ok(file),
            Err(TryLockError::WouldBlock) if started.elapsed() < LOCK_TIMEOUT => {
                sleep(LOCK_RETRY_MS)?;
            }
            Err(TryLockError::WouldBlock) => {
                return Err(BotError::Other(format!(
                    "the army log \"{name}\" stayed locked by another client for {} s",
                    LOCK_TIMEOUT.as_secs()
                )));
            }
            Err(TryLockError::Error(e)) => return Err(io_error(&path, e)),
        }
    }
}

/// Runs `f` on the army folder while holding the log's lock, unless this client already holds
/// it (`bot.army.withLock`).
fn locked<T>(bot: &Bot, name: &str, f: impl FnOnce(&Path) -> BotResult<T>) -> BotResult<T> {
    let dir = bot.paths.army_dir();
    if bot.army.holds(name) {
        return f(&dir);
    }
    let _lock = acquire(&dir, name, |ms| bot.sleep(ms))?;
    f(&dir)
}

/// `bot.army.withLock`'s start: locks the log until [`unlock`] or the script's end.
pub fn lock(bot: &Bot, name: &str) -> BotResult<()> {
    if bot.army.holds(name) {
        return Err(BotError::InvalidArgument(format!(
            "the army log \"{name}\" is already locked by this script"
        )));
    }
    let file = acquire(&bot.paths.army_dir(), name, |ms| bot.sleep(ms))?;
    bot.army.0.lock().insert(name.to_owned(), file);
    Ok(())
}

pub fn unlock(bot: &Bot, name: &str) {
    bot.army.0.lock().remove(name);
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// A barrier's file, `army/<name>.barrier`: `gen=<n>`, then `<member>:<gen>:<unix seconds>` for
/// each member waiting in that generation.
#[derive(Debug, Default, PartialEq)]
struct BarrierState {
    generation: u64,
    /// Lowercase member name, the generation it waits in, when it last renewed its arrival.
    members: Vec<(String, u64, u64)>,
}

impl BarrierState {
    fn parse(text: &str) -> Self {
        let mut state = Self::default();
        for line in text.lines() {
            if let Some(n) = line.strip_prefix("gen=") {
                state.generation = n.trim().parse().unwrap_or(0);
                continue;
            }
            let mut parts = line.rsplitn(3, ':');
            let (Some(beat), Some(generation), Some(member)) =
                (parts.next(), parts.next(), parts.next())
            else {
                continue;
            };
            if let (Ok(generation), Ok(beat)) = (generation.parse(), beat.parse()) {
                state.members.push((member.to_owned(), generation, beat));
            }
        }
        state
    }

    fn serialize(&self) -> String {
        let mut text = format!("gen={}\n", self.generation);
        for (member, generation, beat) in &self.members {
            text.push_str(&format!("{member}:{generation}:{beat}\n"));
        }
        text
    }
}

#[derive(Debug, PartialEq)]
enum Arrival {
    /// Waiting in this generation.
    Waiting(u64),
    Released,
}

fn barrier_file(dir: &Path, name: &str) -> BotResult<PathBuf> {
    check_name(name)?;
    Ok(dir.join(format!("{name}.barrier")))
}

fn read_barrier(path: &Path) -> BotResult<BarrierState> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(BarrierState::parse(&text)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(BarrierState::default()),
        Err(e) => Err(io_error(path, e)),
    }
}

/// Under the barrier's lock, so one temporary name per process is enough.
fn write_barrier(path: &Path, state: &BarrierState) -> BotResult<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| io_error(dir, e))?;
    }
    let temp = path.with_extension(format!("barrier.{}.tmp", std::process::id()));
    fs::write(&temp, state.serialize()).map_err(|e| io_error(&temp, e))?;
    fs::rename(&temp, path).map_err(|e| io_error(path, e))
}

/// One look at the barrier, under its lock: `member` arrives (or renews its arrival in
/// `joined`, the generation it waits in). The arrival that makes `count` members starts the
/// next generation, which releases everyone who waited in the last one, however late they look.
fn arrive(
    dir: &Path,
    name: &str,
    member: &str,
    count: usize,
    joined: Option<u64>,
    now: u64,
) -> BotResult<Arrival> {
    let path = barrier_file(dir, name)?;
    let mut state = read_barrier(&path)?;
    if joined.is_some_and(|g| g != state.generation) {
        return Ok(Arrival::Released);
    }
    let generation = state.generation;
    let member = member.to_lowercase();
    state.members.retain(|(m, g, beat)| {
        *g == generation && *m != member && now.saturating_sub(*beat) <= BARRIER_STALE_SECS
    });
    state.members.push((member, generation, now));
    if state.members.len() >= count {
        state.generation += 1;
        state.members.clear();
        write_barrier(&path, &state)?;
        return Ok(Arrival::Released);
    }
    write_barrier(&path, &state)?;
    Ok(Arrival::Waiting(generation))
}

/// Takes `member` off the barrier (it timed out or stopped).
fn leave(dir: &Path, name: &str, member: &str, generation: u64) -> BotResult<()> {
    let path = barrier_file(dir, name)?;
    let mut state = read_barrier(&path)?;
    if state.generation != generation {
        return Ok(());
    }
    let member = member.to_lowercase();
    state.members.retain(|(m, _, _)| *m != member);
    write_barrier(&path, &state)
}

/// Waits until `count` members (this one included) have arrived at the barrier `name`. False
/// after `timeout_ms` (0 or less: no timeout), when this member leaves the barrier again.
pub fn barrier(
    bot: &Bot,
    name: &str,
    member: &str,
    count: i64,
    timeout_ms: i64,
) -> BotResult<bool> {
    if member.trim().is_empty() || member.contains(':') {
        return Err(BotError::InvalidArgument(format!(
            "the barrier member \"{member}\" must be a name without ':'"
        )));
    }
    let count = usize::try_from(count.max(1)).unwrap_or(1);
    let dir = bot.paths.army_dir();
    let started = Instant::now();
    let lock_name = format!("{name}.barrier");
    let step = |joined| {
        let _lock = acquire(&dir, &lock_name, |ms| bot.sleep(ms))?;
        arrive(&dir, name, member, count, joined, unix_now())
    };
    let mut generation = match step(None)? {
        Arrival::Released => return Ok(true),
        Arrival::Waiting(g) => g,
    };
    let leave_now = |generation| {
        // Not through bot.sleep: this runs when the script stops, too.
        let _lock = acquire(&dir, &lock_name, |ms| {
            std::thread::sleep(Duration::from_millis(ms));
            Ok(())
        })?;
        leave(&dir, name, member, generation)
    };
    loop {
        if timeout_ms > 0 && started.elapsed() >= Duration::from_millis(timeout_ms as u64) {
            leave_now(generation)?;
            return Ok(false);
        }
        if let Err(e) = bot.sleep(BARRIER_TICK_MS) {
            let _ = leave_now(generation);
            return Err(e);
        }
        match step(Some(generation)) {
            Ok(Arrival::Released) => return Ok(true),
            Ok(Arrival::Waiting(g)) => generation = g,
            Err(e) => {
                let _ = leave_now(generation);
                return Err(e);
            }
        }
    }
}

/// The usernames of Army > Accounts, in list order.
pub fn accounts(bot: &Bot) -> Vec<String> {
    Accounts::load(&bot.paths.accounts_file())
        .accounts
        .into_iter()
        .map(|a| a.username)
        .filter(|u| !u.trim().is_empty())
        .collect()
}

/// Logs in with an Army > Accounts account, its password read from the keychain (the script
/// never sees it). False when the account has no saved password.
pub fn login(bot: &Bot, username: &str) -> BotResult<bool> {
    let password = accounts::password(username).map_err(BotError::Other)?;
    match password {
        Some(password) if !password.is_empty() => {
            servers::login(bot, username, &password)?;
            Ok(true)
        }
        _ => Ok(false),
    }
}

/// C# `getRoomNr`: a 5-digit room number that every client of this machine and user picks
/// this hour. C# hashes the machine and user name; the data folder (one per user, shared by
/// the clients) stands in for the machine name here.
pub fn room_number(bot: &Bot) -> i64 {
    let user = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_default();
    let now = Local::now();
    room_number_of(&format!(
        "{}_{user}_{}{}{}{}",
        bot.paths.root.display(),
        now.year(),
        now.month(),
        now.day(),
        now.hour()
    ))
}

/// The Ultras' CoreEngine `GenerateRoomID`: a room from 1000 to 99999 that every client of this
/// machine and user picks, every time. C# hashes the machine GUID; the data folder and user name
/// stand in for it here.
pub fn machine_room_number(bot: &Bot) -> i64 {
    let user = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_default();
    machine_room_number_of(&format!("{}_{user}", bot.paths.root.display()))
}

fn machine_room_number_of(seed: &str) -> i64 {
    let hash = Sha256::digest(seed.as_bytes());
    let seed = u32::from_le_bytes([hash[0], hash[1], hash[2], hash[3]]);
    i64::from(seed % 99000) + 1000
}

fn room_number_of(seed: &str) -> i64 {
    let hash = Sha256::digest(seed.as_bytes());
    // Mix in the index to add variability: a digit per byte.
    let mut digits: String = hash
        .iter()
        .enumerate()
        .take(5)
        .map(|(i, b)| char::from(b'0' + ((*b as usize + i) % 10) as u8))
        .collect();
    if digits.starts_with('0') {
        digits.replace_range(0..1, "1");
    }
    digits.parse().unwrap_or(10000)
}

pub fn dispatch(bot: &Arc<Bot>, name: &str, args: Args) -> BotResult<Value> {
    match name {
        "readLog" => ok(read_log(bot, args.str(0)?)?),
        "appendLog" => ok(append_log(bot, args.str(0)?, args.str(1)?)?),
        "writeLog" => ok(write_log(bot, args.str(0)?, &args.strings(1))?),
        "clearLog" => ok(clear_log(bot, args.str(0)?)?),
        "lock" => ok(lock(bot, args.str(0)?)?),
        "unlock" => {
            unlock(bot, args.str(0)?);
            ok(())
        }
        "barrier" => ok(barrier(
            bot,
            args.str(0)?,
            args.str(1)?,
            args.i64(2)?,
            args.i64_or(3, 0),
        )?),
        "accounts" => ok(accounts(bot)),
        "login" => ok(login(bot, args.str(0)?)?),
        "roomNumber" => ok(room_number(bot)),
        "machineRoomNumber" => ok(machine_room_number(bot)),
        _ => Err(BotError::UnknownMethod(format!("army.{name}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_log_is_shared_lines_and_refuses_paths() {
        let dir = std::env::temp_dir().join(format!("turu-army-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        assert!(read_log_in(&dir, "ArmyLog").unwrap().is_empty());
        append_log_in(&dir, "ArmyLog", "alice:done:farm").unwrap();
        append_log_in(&dir, "ArmyLog", "bob:done:farm").unwrap();
        assert_eq!(
            read_log_in(&dir, "ArmyLog").unwrap(),
            ["alice:done:farm", "bob:done:farm"]
        );
        write_log_in(&dir, "ArmyLog", &["bob:1:5".to_owned()]).unwrap();
        assert_eq!(read_log_in(&dir, "ArmyLog").unwrap(), ["bob:1:5"]);
        clear_log_in(&dir, "ArmyLog").unwrap();
        assert!(read_log_in(&dir, "ArmyLog").unwrap().is_empty());
        assert!(append_log_in(&dir, "../escape", "x").is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn clients_of_one_process_can_write_the_log_at_once() {
        let dir = std::env::temp_dir().join(format!("turu-army-race-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let writers: Vec<_> = (0..4)
            .map(|i| {
                let dir = dir.clone();
                std::thread::spawn(move || {
                    for n in 0..200 {
                        write_log_in(&dir, "Sync", &[format!("c{i}:1:{n}")]).unwrap();
                    }
                })
            })
            .collect();
        for writer in writers {
            writer.join().unwrap();
        }
        assert_eq!(read_log_in(&dir, "Sync").unwrap().len(), 1);
        let _ = fs::remove_dir_all(&dir);
    }

    fn no_wait(_: u64) -> BotResult<()> {
        std::thread::sleep(Duration::from_millis(1));
        Ok(())
    }

    #[test]
    fn a_lock_shuts_out_other_clients_until_dropped() {
        let dir = std::env::temp_dir().join(format!("turu-army-lock-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let held = acquire(&dir, "Sync", no_wait).unwrap();
        // Another client of this process opens the file apart, as a tab does.
        let other = OpenOptions::new()
            .write(true)
            .open(lock_path(&dir, "Sync").unwrap())
            .unwrap();
        assert!(matches!(other.try_lock(), Err(TryLockError::WouldBlock)));
        drop(held);
        other.try_lock().unwrap();
        drop(other);
        assert!(lock_path(&dir, "../escape").is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn locked_read_modify_writes_lose_no_update() {
        let dir = std::env::temp_dir().join(format!("turu-army-rmw-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let writers: Vec<_> = (0..4)
            .map(|i| {
                let dir = dir.clone();
                std::thread::spawn(move || {
                    for n in 0..50 {
                        let _lock = acquire(&dir, "Sync", no_wait).unwrap();
                        let mut lines = read_log_in(&dir, "Sync").unwrap();
                        lines.push(format!("c{i}:{n}"));
                        write_log_in(&dir, "Sync", &lines).unwrap();
                    }
                })
            })
            .collect();
        for writer in writers {
            writer.join().unwrap();
        }
        assert_eq!(read_log_in(&dir, "Sync").unwrap().len(), 200);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_last_arrival_releases_a_generation_and_late_lookers_see_it() {
        let dir = std::env::temp_dir().join(format!("turu-army-barrier-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let t = 1_000;
        assert_eq!(
            arrive(&dir, "B", "Alice", 3, None, t).unwrap(),
            Arrival::Waiting(0)
        );
        // Renewing doesn't count twice.
        assert_eq!(
            arrive(&dir, "B", "alice", 3, Some(0), t + 1).unwrap(),
            Arrival::Waiting(0)
        );
        assert_eq!(
            arrive(&dir, "B", "Bob", 3, None, t + 1).unwrap(),
            Arrival::Waiting(0)
        );
        assert_eq!(
            arrive(&dir, "B", "Carol", 3, None, t + 2).unwrap(),
            Arrival::Released
        );
        // Alice looks only much later (a long pause): still released, not left behind.
        assert_eq!(
            arrive(&dir, "B", "Alice", 3, Some(0), t + 60).unwrap(),
            Arrival::Released
        );
        // The next round starts empty in generation 1.
        assert_eq!(
            arrive(&dir, "B", "Bob", 3, None, t + 61).unwrap(),
            Arrival::Waiting(1)
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_member_that_stops_renewing_or_leaves_no_longer_counts() {
        let dir = std::env::temp_dir().join(format!("turu-army-stale-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let t = 1_000;
        arrive(&dir, "B", "Ghost", 2, None, t).unwrap();
        // Ghost crashed: 16 s later it doesn't make up the pair.
        assert_eq!(
            arrive(&dir, "B", "Bob", 2, None, t + BARRIER_STALE_SECS + 1).unwrap(),
            Arrival::Waiting(0)
        );
        leave(&dir, "B", "bob", 0).unwrap();
        assert_eq!(
            read_barrier(&barrier_file(&dir, "B").unwrap()).unwrap(),
            BarrierState::default()
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn members_on_threads_all_pass_the_barrier_round_after_round() {
        let dir = std::env::temp_dir().join(format!("turu-army-rounds-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let members: Vec<_> = (0..4)
            .map(|i| {
                let dir = dir.clone();
                std::thread::spawn(move || {
                    for _round in 0..5 {
                        let mut joined = None;
                        loop {
                            let _lock = acquire(&dir, "R.barrier", no_wait).unwrap();
                            match arrive(&dir, "R", &format!("m{i}"), 4, joined, unix_now())
                                .unwrap()
                            {
                                Arrival::Released => break,
                                Arrival::Waiting(g) => joined = Some(g),
                            }
                            drop(_lock);
                            std::thread::sleep(Duration::from_millis(2));
                        }
                    }
                })
            })
            .collect();
        for member in members {
            member.join().unwrap();
        }
        let state = read_barrier(&barrier_file(&dir, "R").unwrap()).unwrap();
        assert_eq!(state.generation, 5);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn room_numbers_have_five_digits_and_follow_the_seed() {
        let a = room_number_of("machine_user_2026103112");
        assert!((10000..=99999).contains(&a));
        assert_eq!(a, room_number_of("machine_user_2026103112"));
        assert_ne!(a, room_number_of("machine_user_2026103113"));
        let m = machine_room_number_of("machine_user");
        assert!((1000..100000).contains(&m));
        assert_eq!(m, machine_room_number_of("machine_user"));
    }
}
