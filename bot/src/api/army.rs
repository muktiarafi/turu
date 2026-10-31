//! `bot.army`: what VibeSkua's CoreArmyLite reads outside the game. Its sync log (C#
//! `ArmyLogging`, a file every client of the army appends to), the accounts of Army > Accounts
//! (C# `Bot.Accounts.GetAllAccounts()`, for the one-client scripts that log in each account in
//! turn) and the army's private room number (C# `getRoomNr`).

use super::{Args, ok, servers};
use crate::Bot;
use crate::accounts::{self, Accounts};
use crate::error::{BotError, BotResult};
use chrono::{Datelike, Local, Timelike};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// `army/<name>.log` in the data folder, which every client of this machine shares (C# puts it
/// in Skua's options folder). The name must be a plain file name.
fn log_file(dir: &Path, name: &str) -> BotResult<PathBuf> {
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
    Ok(dir.join(format!("{name}.log")))
}

fn io_error(path: &std::path::Path, e: std::io::Error) -> BotError {
    BotError::Other(format!("{}: {e}", path.display()))
}

/// The log's lines (C# `ReadLog`); none when it doesn't exist yet.
pub fn read_log(bot: &Bot, name: &str) -> BotResult<Vec<String>> {
    read_log_in(&bot.paths.army_dir(), name)
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
    append_log_in(&bot.paths.army_dir(), name, line)
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
    write_log_in(&bot.paths.army_dir(), name, lines)
}

fn write_log_in(dir: &Path, name: &str, lines: &[String]) -> BotResult<()> {
    let path = log_file(dir, name)?;
    fs::create_dir_all(dir).map_err(|e| io_error(dir, e))?;
    let temp = dir.join(format!(".{name}.{}.tmp", std::process::id()));
    let text: String = lines.iter().map(|l| format!("{l}\n")).collect();
    fs::write(&temp, text).map_err(|e| io_error(&temp, e))?;
    fs::rename(&temp, &path).map_err(|e| io_error(&path, e))
}

/// Empties the log (C# `ClearLogFile`).
pub fn clear_log(bot: &Bot, name: &str) -> BotResult<()> {
    clear_log_in(&bot.paths.army_dir(), name)
}

fn clear_log_in(dir: &Path, name: &str) -> BotResult<()> {
    let path = log_file(dir, name)?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| io_error(dir, e))?;
    }
    fs::write(&path, "").map_err(|e| io_error(&path, e))
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
