//! "Follow master's quests" (VibeSkua ButlerV5 `QuestSync.cs` and `Follower`'s
//! `SyncRegisteredQuests`): a master publishes its active and registered quests, with each
//! registered quest's chosen reward, to `<username>.quests` next to its sync file; a butler
//! registers the same quests on its own client, so it accepts them, collects its own drops and
//! turns them in from its own inventory. A butler only ever removes the registrations it added.

use super::sync::SyncDir;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io;
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime};

/// How often a butler checks the master's list for a change (VibeSkua: 1 s).
const CHECK_INTERVAL: Duration = Duration::from_secs(1);

/// One published quest (VibeSkua `QuestSyncEntry`): its ID and the reward to pick (-1: none).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub quest_id: i64,
    pub reward_id: i64,
}

/// The master's list: every active quest (no reward), and every registered one with its
/// reward (a registered quest's reward wins), by ID.
pub fn entries(active: &[i64], registered: &[(i64, i64)]) -> Vec<Entry> {
    let mut quests: BTreeMap<i64, i64> = BTreeMap::new();
    for &id in active.iter().filter(|&&id| id > 0) {
        quests.entry(id).or_insert(-1);
    }
    for &(id, reward) in registered.iter().filter(|(id, _)| *id > 0) {
        quests.insert(id, reward);
    }
    quests
        .into_iter()
        .map(|(quest_id, reward_id)| Entry {
            quest_id,
            reward_id,
        })
        .collect()
}

/// The account's quest list file.
pub fn path_for(dir: &SyncDir, username: &str) -> PathBuf {
    dir.path_for(username).with_extension("quests")
}

/// Writes the list unless the file already holds it; whether it wrote.
pub fn publish(dir: &SyncDir, username: &str, entries: &[Entry]) -> io::Result<bool> {
    let path = path_for(dir, username);
    let text = serde_json::to_string(entries).map_err(io::Error::other)?;
    if std::fs::read_to_string(&path).is_ok_and(|old| old == text) {
        return Ok(false);
    }
    std::fs::create_dir_all(&dir.dir)?;
    std::fs::write(&path, text)?;
    Ok(true)
}

/// When the master's list was last written; `None` without one.
pub fn file_time(dir: &SyncDir, username: &str) -> Option<SystemTime> {
    std::fs::metadata(path_for(dir, username))
        .and_then(|m| m.modified())
        .ok()
}

/// The master's list, without bad IDs, the last entry winning for a repeated one; `None`
/// when there's no readable list yet.
pub fn read(dir: &SyncDir, username: &str) -> Option<Vec<Entry>> {
    let text = std::fs::read_to_string(path_for(dir, username)).ok()?;
    let entries: Vec<Entry> = serde_json::from_str(&text).ok()?;
    let mut by_id: BTreeMap<i64, Entry> = BTreeMap::new();
    for entry in entries.into_iter().filter(|e| e.quest_id > 0) {
        by_id.insert(entry.quest_id, entry);
    }
    Some(by_id.into_values().collect())
}

/// What to change on the butler's client to match the master's list.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Plan {
    /// Registrations this butler added that the master dropped, or whose reward changed.
    pub unregister: Vec<i64>,
    /// The master's quests not registered here (after the removals), with their rewards.
    pub register: Vec<(i64, i64)>,
}

/// The registrations a butler added for its master, and when it last read the master's list
/// (VibeSkua `_syncedQuestIds`, `_syncedQuestRewards` and the check times).
#[derive(Debug, Default)]
pub struct Mirror {
    /// Quest ID to the reward it was registered with.
    synced: BTreeMap<i64, i64>,
    last_check: Option<Instant>,
    file_time: Option<SystemTime>,
}

impl Mirror {
    /// The quest IDs this butler registered.
    pub fn synced(&self) -> Vec<i64> {
        self.synced.keys().copied().collect()
    }

    /// Whether a check is due (at most once a second); starts the interval when it is.
    pub fn check_due(&mut self) -> bool {
        if self
            .last_check
            .is_some_and(|t| t.elapsed() < CHECK_INTERVAL)
        {
            return false;
        }
        self.last_check = Some(Instant::now());
        true
    }

    /// Whether the master's list written at `time` is newer than the one last applied.
    pub fn is_new(&self, time: SystemTime) -> bool {
        self.file_time.is_none_or(|t| time > t)
    }

    /// The changes for the master's `requested` list, given the quests `registered` on this
    /// client now, and records them as applied (from a list written at `time`). A quest
    /// registered here by something else is left alone; a synced one whose reward changed is
    /// unregistered and registered again with the new reward.
    pub fn plan(
        &mut self,
        requested: &[Entry],
        registered: &[i64],
        time: Option<SystemTime>,
    ) -> Plan {
        let wanted: BTreeMap<i64, i64> = requested
            .iter()
            .map(|e| (e.quest_id, e.reward_id))
            .collect();
        let unregister: Vec<i64> = self
            .synced
            .iter()
            .filter(|(id, reward)| wanted.get(id).is_none_or(|w| w != *reward))
            .map(|(id, _)| *id)
            .collect();
        for id in &unregister {
            self.synced.remove(id);
        }
        let register: Vec<(i64, i64)> = wanted
            .iter()
            .filter(|(id, _)| unregister.contains(id) || !registered.contains(id))
            .map(|(&id, &reward)| (id, reward))
            .collect();
        for &(id, reward) in &register {
            self.synced.insert(id, reward);
        }
        if time.is_some() {
            self.file_time = time;
        }
        Plan {
            unregister,
            register,
        }
    }

    /// Forgets everything (after a logout, which clears the client's registrations): the
    /// master's unchanged list is applied again after the relogin.
    pub fn forget(&mut self) {
        *self = Self::default();
    }

    /// Forgets everything; the quest IDs to unregister.
    pub fn take(&mut self) -> Vec<i64> {
        let ids = self.synced();
        self.forget();
        ids
    }
}
