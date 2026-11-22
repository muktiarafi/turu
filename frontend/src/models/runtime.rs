//! Combat > Runtime (VibeSkua `RuntimeHelpersView`): the drop pickup list and its loop, the
//! registered quests and the boosts, editable while a script runs or without one.
//!
//! Everything shown is the bot's own (`bot.drops`, `bot.quests`, `bot.boosts`, the Accept AC Drops
//! option); what this adds per tab ([`State`]) is whether a boost search runs. The state is locked
//! only briefly, never while calling the bot, so a shell may call any function here from a frame.

use super::tab_state::{PerTab, lock};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use turu_bot::Bot;
use turu_bot::api::NameOrId;
use turu_bot::api::boosts::{self, BoostType};

#[derive(Default)]
pub struct State {
    /// A Search Inventory / Search Bank runs.
    searching: Arc<AtomicBool>,
}

pub static STATE: PerTab<State> = PerTab::new();

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct RuntimeView {
    /// The pickup list as VibeSkua lists it: the names, then the IDs as text.
    pub drops: Vec<String>,
    /// Waited on top of the loop's 1 s tick, in ms.
    pub interval: u64,
    pub reject_else: bool,
    pub accept_ac: bool,
    /// The pickup loop runs (Pick Drops is on).
    pub picking: bool,
    pub quests: Vec<RuntimeQuest>,
    pub boosts: Vec<RuntimeBoost>,
    /// The boost timer runs (Use Boosts is on).
    pub boosting: bool,
    /// A boost search runs: both search buttons wait.
    pub searching: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct RuntimeQuest {
    pub id: i64,
    /// The reward it's turned in for, -1 for none.
    pub reward: i64,
    /// VibeSkua `RegisteredQuestInfo.ToString`: "1234" or "1234 (Reward: 5678)".
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct RuntimeBoost {
    /// What the `boost` action takes: "Class", "Gold", "Experience", "Reputation".
    pub kind: String,
    /// "Class Boost".
    pub label: String,
    pub uses: bool,
    /// The item found for it (0: none yet).
    pub id: i64,
}

/// The boosts in the window's order.
const BOOSTS: [(&str, BoostType); 4] = [
    ("Class", BoostType::Class),
    ("Gold", BoostType::Gold),
    ("Experience", BoostType::Experience),
    ("Reputation", BoostType::Reputation),
];

pub fn view(bot: &Arc<Bot>) -> RuntimeView {
    let drops = bot
        .drops
        .to_pickup()
        .into_iter()
        .chain(bot.drops.to_pickup_ids().iter().map(i64::to_string))
        .collect();
    let quests = bot
        .quests
        .registered_rewards()
        .into_iter()
        .map(|(id, reward)| RuntimeQuest {
            id,
            reward,
            label: if reward == -1 {
                id.to_string()
            } else {
                format!("{id} (Reward: {reward})")
            },
        })
        .collect();
    let boosts = BOOSTS
        .iter()
        .map(|&(kind, boost)| RuntimeBoost {
            kind: kind.to_owned(),
            label: format!("{kind} Boost"),
            uses: bot.boosts.uses(boost),
            id: bot.boosts.id(boost),
        })
        .collect();
    RuntimeView {
        drops,
        interval: bot.drops.interval(),
        reject_else: bot.drops.reject_else(),
        accept_ac: bot.opts().accept_ac_drops,
        picking: bot.drops.running(),
        quests,
        boosts,
        boosting: bot.boosts.enabled(),
        searching: searching(bot).load(Ordering::Acquire),
    }
}

fn searching(bot: &Arc<Bot>) -> Arc<AtomicBool> {
    lock(&STATE.of(bot)).searching.clone()
}

/// VibeSkua `AddDrop`: `|`-separated, a number is an item ID. Returns whether anything was added.
pub fn add_drops(bot: &Arc<Bot>, input: &str) -> bool {
    let inputs: Vec<String> = input
        .split('|')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect();
    bot.drops.add(&names_or_ids(&inputs));
    !inputs.is_empty()
}

/// Removes rows of the pickup list, as [`RuntimeView::drops`] shows them.
pub fn remove_drops(bot: &Arc<Bot>, items: &[String]) {
    bot.drops.remove(&names_or_ids(items));
}

/// Pick Drops: starts the loop, or stops it (off the caller's thread: stopping waits for it).
pub fn set_picking(bot: &Arc<Bot>, on: bool) {
    if on {
        bot.drops.start(bot);
    } else {
        crate::turu::spawn_action_for(bot.clone(), "Stop drops", |bot| {
            bot.drops.stop();
            Ok(())
        });
    }
}

/// VibeSkua `AddQuest`: registers each quest for `reward` (-1: none).
pub fn add_quests(bot: &Arc<Bot>, input: &str, reward: i64) -> Result<(), String> {
    let ids = parse_quest_ids(input).ok_or("Type quest IDs (numbers, separated by spaces, commas or |)")?;
    let quests: Vec<(i64, i64)> = ids.into_iter().map(|id| (id, reward)).collect();
    bot.quests.register(bot, &quests);
    Ok(())
}

/// Turns one boost (a [`RuntimeBoost::kind`]) on or off.
pub fn set_boost(bot: &Arc<Bot>, kind: &str, on: bool) -> Result<(), String> {
    let boost = BoostType::parse(kind).map_err(|e| e.to_string())?;
    bot.boosts.set_uses(boost, on);
    Ok(())
}

/// Search Inventory (`bank` false) or Search Bank: finds the boost items' IDs.
pub fn search(bot: &Arc<Bot>, bank: bool) {
    let flag = searching(bot);
    if flag.swap(true, Ordering::AcqRel) {
        return;
    }
    let label = if bank { "Search Bank" } else { "Search Inventory" };
    crate::turu::spawn_action_for(bot.clone(), label, move |bot| {
        let result = boosts::set_all_ids(bot, bank);
        flag.store(false, Ordering::Release);
        result
    });
}

/// Use Boosts: starts the timer, or stops it (off the caller's thread: stopping waits for it).
pub fn set_boosting(bot: &Arc<Bot>, on: bool) {
    if on {
        bot.boosts.start(bot);
    } else {
        crate::turu::spawn_action_for(bot.clone(), "Stop boosts", |bot| {
            bot.boosts.stop();
            Ok(())
        });
    }
}

/// Runtime's actions (`POST /models/runtime/do`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[cfg_attr(
    feature = "typegen",
    derive(facet::Facet),
    facet(tag = "action", rename_all = "kebab-case")
)]
#[serde(tag = "action", rename_all = "kebab-case")]
#[repr(C)]
pub enum RuntimeAct {
    /// Adds the drops typed in `input`.
    AddDrops {
        input: String,
    },
    RemoveDrops {
        items: Vec<String>,
    },
    /// Milliseconds between pickups.
    Interval {
        ms: u64,
    },
    RejectElse {
        on: bool,
    },
    AcceptAc {
        on: bool,
    },
    Picking {
        on: bool,
    },
    /// Registers the quests typed in `input`, with `reward` (an item ID) when given.
    AddQuests {
        input: String,
        reward: Option<i64>,
    },
    RemoveQuests {
        ids: Vec<i64>,
    },
    /// Turns the boost of `kind` on or off.
    Boost {
        kind: String,
        on: bool,
    },
    /// Searches the bank, or (`bank` false) the inventory, for boosts.
    Search {
        bank: bool,
    },
    Boosting {
        on: bool,
    },
}

pub fn act(bot: &Arc<Bot>, act: RuntimeAct) -> Result<(), String> {
    match act {
        RuntimeAct::AddDrops { input } => {
            add_drops(bot, &input);
        }
        RuntimeAct::RemoveDrops { items } => remove_drops(bot, &items),
        RuntimeAct::Interval { ms } => bot.drops.set_interval(ms),
        RuntimeAct::RejectElse { on } => bot.drops.set_reject_else(on),
        RuntimeAct::AcceptAc { on } => bot.options.write().accept_ac_drops = on,
        RuntimeAct::Picking { on } => set_picking(bot, on),
        RuntimeAct::AddQuests { input, reward } => add_quests(bot, &input, reward.unwrap_or(-1))?,
        RuntimeAct::RemoveQuests { ids } => bot.quests.unregister(&ids),
        RuntimeAct::Boost { kind, on } => set_boost(bot, &kind, on)?,
        RuntimeAct::Search { bank } => search(bot, bank),
        RuntimeAct::Boosting { on } => set_boosting(bot, on),
    }
    Ok(())
}

/// Text from the drops list: a number is an item ID, anything else a name.
fn names_or_ids(items: &[String]) -> Vec<NameOrId<'_>> {
    items
        .iter()
        .map(|item| match item.parse::<i64>() {
            Ok(id) => NameOrId::Id(id),
            Err(_) => NameOrId::Name(item),
        })
        .collect()
}

/// VibeSkua `AddQuest`: IDs separated by `|`, `,` or spaces; nothing when anything else is typed.
pub fn parse_quest_ids(input: &str) -> Option<Vec<i64>> {
    let parts: Vec<&str> = input
        .split(['|', ',', ' '])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    if parts.is_empty() || !parts.iter().all(|p| p.bytes().all(|b| b.is_ascii_digit())) {
        return None;
    }
    parts.iter().map(|p| p.parse().ok()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quest_ids_follow_vibeskua() {
        assert_eq!(parse_quest_ids("1234"), Some(vec![1234]));
        assert_eq!(parse_quest_ids(" 1, 2 | 3  4 "), Some(vec![1, 2, 3, 4]));
        assert_eq!(parse_quest_ids("12a"), None);
        assert_eq!(parse_quest_ids(" , "), None);
    }

    #[test]
    fn a_number_in_the_drop_list_is_an_id() {
        let items = vec!["Unidentified 13".to_owned(), "5000".to_owned()];
        let parsed = names_or_ids(&items);
        assert!(matches!(parsed[0], NameOrId::Name("Unidentified 13")));
        assert!(matches!(parsed[1], NameOrId::Id(5000)));
    }
}
