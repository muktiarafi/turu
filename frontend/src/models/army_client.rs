//! Army Control > a row's ⋮ > Gear and Logs, for a native page: one client's worn slots and what it
//! could wear instead (its gear file, `gear/<username>.json`, docs/ARMY_EQUIPMENT_PROPOSAL.md), its
//! Equip and Refresh, and its script and debug logs (its control API's `/log?since=`).
//!
//! Stateless but for the outcomes of Equip and Refresh: each call names the row by its key and is
//! answered at once, so a page that a slot restart left keeps working, and logs are read from
//! where the page left off. The egui windows (`gui::turu::gear`, `client_logs`) keep their own.

use super::army::{self, Peer};
use super::tab_state::lock;
use serde::{Deserialize, Serialize};
use serde_json::{Value as Json, json};
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use turu_bot::army::Command;
use turu_bot::gear::{self, EquipReport, GearItem, Place, Slot};

/// The last Equip or Refresh of each client (by its row key): what runs, or how it went.
#[derive(Default, Clone)]
struct Outcome {
    busy: Option<String>,
    last: Option<(bool, String)>,
}

static OUTCOMES: LazyLock<Mutex<HashMap<String, Outcome>>> = LazyLock::new(Mutex::default);

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct ArmyGearView {
    pub label: String,
    pub online: bool,
    /// The running script's file name; empty with none.
    pub script: String,
    /// "Bank as of 12 min ago", "Bank not read yet", or why there's nothing.
    pub bank: String,
    /// A gear file was found.
    pub known: bool,
    pub slots: Vec<ArmyGearSlot>,
    /// The action running now ("Equip Necrotic Sword"); empty with none.
    pub busy: String,
    /// How the last one went; empty before any.
    pub last: String,
    pub last_ok: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct ArmyGearSlot {
    pub slot: String,
    /// What's worn; "(none)" with nothing.
    pub worn: String,
    pub choices: Vec<ArmyGearChoice>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct ArmyGearChoice {
    pub id: i64,
    /// "Necrotic Sword (bank)", "Legion Revenant (worn)".
    pub label: String,
    pub worn: bool,
}

/// "Necrotic Sword (bank)", "Legion Revenant (worn)".
pub fn item_label(item: &GearItem) -> String {
    if item.equipped {
        format!("{} (worn)", item.name)
    } else if item.place == Place::Bank {
        format!("{} (bank)", item.name)
    } else {
        item.name.clone()
    }
}

/// "just now", "12 min ago", "3 h ago", "2 days ago".
pub fn ago(at_ms: u64) -> String {
    let minutes = gear::now_ms().saturating_sub(at_ms) / 60_000;
    match minutes {
        0 => "just now".into(),
        1..60 => format!("{minutes} min ago"),
        60..2880 => format!("{} h ago", minutes / 60),
        _ => format!("{} days ago", minutes / 1440),
    }
}

/// The row `key` names, as a peer.
fn peer(key: String) -> Result<(String, Peer), String> {
    let rows = army::rows();
    let row = rows.iter().find(|r| r.key == key).ok_or("That client is gone")?;
    Ok((key, row.peer.clone()))
}

/// The client's gear, from its gear file (so an offline account shows its last).
pub fn gear_view(key: &str, peer: &Peer) -> ArmyGearView {
    let username = peer.username();
    let file = (!username.is_empty())
        .then(|| gear::read(&turu_bot::paths::Paths::new().gear_dir(), &username))
        .flatten();
    let outcome = lock(&OUTCOMES).get(key).cloned().unwrap_or_default();
    let bank = match file.as_ref().map(|f| f.bank_as_of) {
        None => "No gear file yet: it's written once the account logs in".to_owned(),
        Some(None) => "Bank not read yet".to_owned(),
        Some(Some(at)) => format!("Bank as of {}", ago(at)),
    };
    let slots = file
        .as_ref()
        .map(|file| {
            Slot::ALL
                .iter()
                .map(|&slot| ArmyGearSlot {
                    slot: slot.label().to_owned(),
                    worn: file.worn(slot).map_or_else(|| "(none)".to_owned(), |i| i.name.clone()),
                    choices: file
                        .for_slot(slot)
                        .into_iter()
                        .map(|i| ArmyGearChoice {
                            id: i.id,
                            label: item_label(i),
                            worn: i.equipped,
                        })
                        .collect(),
                })
                .collect()
        })
        .unwrap_or_default();
    let (last_ok, last) = outcome.last.unwrap_or((true, String::new()));
    ArmyGearView {
        label: peer.label(),
        online: peer.online(),
        script: peer.running_script().unwrap_or_default(),
        bank,
        known: file.is_some(),
        slots,
        busy: outcome.busy.unwrap_or_default(),
        last,
        last_ok,
    }
}

/// Sends `command` to the client on another thread, then reads its gear again; the outcome is
/// kept for the view.
fn run(key: String, peer: Peer, action: String, command: Command) -> Result<(), String> {
    {
        let mut outcomes = lock(&OUTCOMES);
        let outcome = outcomes.entry(key.clone()).or_default();
        if outcome.busy.is_some() {
            return Err("Wait for the last one to finish".into());
        }
        outcome.busy = Some(action.clone());
    }
    let spawned = std::thread::Builder::new()
        .name("Turu Gear Action".into())
        .spawn(move || {
            let result = peer.send(&command).map(|reply| match command {
                Command::Equip { .. } => serde_json::from_value::<EquipReport>(reply)
                    .map_or_else(|_| "done".to_owned(), |report| report.summary()),
                _ => "gear read".to_owned(),
            });
            if !matches!(command, Command::RefreshGear) {
                let _ = peer.send(&Command::RefreshGear);
            }
            let mut outcomes = lock(&OUTCOMES);
            let outcome = outcomes.entry(key).or_default();
            outcome.busy = None;
            outcome.last = Some(match result {
                Ok(text) => (true, format!("{action}: {text}")),
                Err(e) => (false, format!("{action}: {e}")),
            });
        });
    spawned.map(|_| ()).map_err(|e| e.to_string())
}

/// Logs' answer: the lines from `since` on, and where the next read starts.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct ClientLog {
    pub lines: Vec<String>,
    pub next: u64,
}

fn json_of(answer: &impl Serialize) -> Result<Json, String> {
    serde_json::to_value(answer).map_err(|e| e.to_string())
}

/// An Army client's actions (`POST /models/army-client/do`), each on the row whose key is `key`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[cfg_attr(
    feature = "typegen",
    derive(facet::Facet),
    facet(tag = "action", rename_all = "kebab-case")
)]
#[serde(tag = "action", rename_all = "kebab-case")]
#[repr(C)]
pub enum ArmyClientAct {
    /// Answers its [`ArmyGearView`].
    Gear {
        key: String,
    },
    Refresh {
        key: String,
    },
    /// Equips the item `item` (its ID; `name` for the outcome), stopping the script first when
    /// `stop`.
    Equip {
        key: String,
        item: i64,
        name: Option<String>,
        stop: bool,
    },
    /// Its log of `kind` ("script", or "debug") from line `since`: answers `lines` and `next`.
    Logs {
        key: String,
        kind: String,
        since: u64,
    },
}

pub fn act(act: ArmyClientAct) -> Result<Json, String> {
    match act {
        ArmyClientAct::Gear { key } => {
            let (key, peer) = peer(key)?;
            json_of(&gear_view(&key, &peer))
        }
        ArmyClientAct::Refresh { key } => {
            let (key, peer) = peer(key)?;
            if !peer.online() {
                return Err("The account isn't online".into());
            }
            run(key, peer, "Refresh".into(), Command::RefreshGear).map(|()| json!({}))
        }
        ArmyClientAct::Equip { key, item, name, stop } => {
            let (key, peer) = peer(key)?;
            if !peer.online() {
                return Err("The account isn't online".into());
            }
            if peer.running_script().is_some() && !stop {
                return Err("A script is running: stop it first".into());
            }
            let name = name.unwrap_or_else(|| item.to_string());
            let command = Command::Equip {
                item: item.to_string(),
                stop,
            };
            run(key, peer, format!("Equip {name}"), command).map(|()| json!({}))
        }
        ArmyClientAct::Logs { key, kind, since } => {
            let (_, peer) = peer(key)?;
            let kind = if kind == "debug" { "debug" } else { "script" };
            let reply = peer.get(&format!("/log?type={kind}&since={since}"))?;
            let lines: Vec<&str> = reply["lines"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|l| l["text"].as_str().unwrap_or_default())
                .collect();
            json_of(&ClientLog {
                lines: lines.into_iter().map(str::to_owned).collect(),
                next: reply["next"].as_u64().unwrap_or(since),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_gone_client_is_refused() {
        assert!(serde_json::from_value::<ArmyClientAct>(json!({ "action": "gear" })).is_err());
        assert_eq!(
            act(ArmyClientAct::Gear {
                key: "questwalker-gone".into()
            }),
            Err("That client is gone".into())
        );
        assert_eq!(
            item_label(&GearItem {
                id: 1,
                name: "Necrotic Sword".into(),
                category: "Sword".into(),
                slot: Slot::Weapon,
                equipped: false,
                place: Place::Bank,
                enhancement_level: 0,
                enhancement_pattern_id: 0,
                proc_id: 0,
            }),
            "Necrotic Sword (bank)"
        );
        assert_eq!(ago(gear::now_ms()), "just now");
    }
}
