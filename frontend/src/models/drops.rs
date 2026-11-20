//! Tools > Current Drops: the items in the tab's drop stack (`bot.state.drops`), sorted by name,
//! with Pick up all and Pick up selected. The search, AC only and which are selected are each
//! window's own.

use serde::Serialize;
use serde_json::json;
use std::sync::Arc;
use turu_bot::Bot;
use turu_bot::api::dispatch;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct DropsView {
    /// By name, then id.
    pub drops: Vec<DropView>,
    /// How many are AdventureCoins items.
    pub ac_count: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct DropView {
    pub id: i64,
    pub name: String,
    pub quantity: i64,
    /// An AdventureCoins item.
    pub coins: bool,
}

pub fn view(bot: &Arc<Bot>) -> DropsView {
    let mut drops: Vec<DropView> = bot
        .state
        .lock()
        .drops
        .values()
        .map(|d| DropView {
            id: d.id,
            name: d.name.clone(),
            quantity: d.quantity,
            coins: d.coins,
        })
        .collect();
    drops.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then(a.id.cmp(&b.id)));
    DropsView {
        ac_count: drops.iter().filter(|d| d.coins).count() as u32,
        drops,
    }
}

/// Whether `drop` shows for this search (by name, any case) and AC only.
pub fn shows(drop: &DropView, search: &str, ac_only: bool) -> bool {
    let needle = search.trim().to_lowercase();
    (!ac_only || drop.coins) && (needle.is_empty() || drop.name.to_lowercase().contains(&needle))
}

/// Pick up all: every drop in the stack (off the caller's thread).
pub fn pickup_all(bot: &Arc<Bot>) {
    crate::turu::spawn_action_for(bot.clone(), "Pick up all", |bot| {
        dispatch(bot, "drops.pickupAll", &[]).map(|_| ())
    });
}

/// Pick up selected: the drops with these ids.
pub fn pickup(bot: &Arc<Bot>, ids: Vec<i64>) {
    if ids.is_empty() {
        return;
    }
    crate::turu::spawn_action_for(bot.clone(), "Pick up selected", move |bot| {
        dispatch(bot, "drops.pickup", &[json!(ids)]).map(|_| ())
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_and_ac_only() {
        let drop = |name: &str, coins| DropView {
            id: 1,
            name: name.into(),
            quantity: 1,
            coins,
        };
        assert!(shows(&drop("Dragon Scale", false), "scale", false));
        assert!(!shows(&drop("Dragon Scale", false), "", true));
        assert!(shows(&drop("Void Aura", true), " VOID ", true));
        assert!(view(&Bot::new()).drops.is_empty());
    }
}
