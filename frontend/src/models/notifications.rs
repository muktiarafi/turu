//! Diagnostics > Notification Center (VibeSkua `NotificationCenterView`): the tab's feed of game
//! and script events (`bot.notifications`), newest first, the filters with their counts, and the
//! words a shell shows when a filter has nothing. Which events are selected is each window's own
//! state; Copy and Share take the entries' `line`s, oldest first, as the feed's `to_text` does.

use serde::Serialize;
use std::sync::Arc;
use turu_bot::Bot;
use turu_bot::notifications::{Notification, NotificationKind};

/// What an event is about, as a shell shows it (VibeSkua's kinds; `System` reads "Map").
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
#[repr(u8)]
pub enum NoticeKind {
    Item,
    Activity,
    Warning,
    Error,
    Map,
}

impl NoticeKind {
    pub fn of(kind: NotificationKind) -> Self {
        match kind {
            NotificationKind::Item => Self::Item,
            NotificationKind::Activity => Self::Activity,
            NotificationKind::Warning => Self::Warning,
            NotificationKind::Error => Self::Error,
            NotificationKind::System => Self::Map,
        }
    }

    /// Its word in "No item events yet."
    fn word(self) -> &'static str {
        match self {
            Self::Item => "item",
            Self::Activity => "activity",
            Self::Warning => "warning",
            Self::Error => "error",
            Self::Map => "map",
        }
    }
}

/// VibeSkua `Filters`: each chip's kind (`None`: all) and label, in order.
pub const FILTERS: [(Option<NoticeKind>, &str); 6] = [
    (None, "All"),
    (Some(NoticeKind::Item), "Items"),
    (Some(NoticeKind::Activity), "Activity"),
    (Some(NoticeKind::Warning), "Warnings"),
    (Some(NoticeKind::Error), "Errors"),
    (Some(NoticeKind::Map), "Maps"),
];

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct NotificationsView {
    /// Newest first.
    pub entries: Vec<NoticeView>,
    /// The chips, in order: "All" first.
    pub filters: Vec<NoticeFilter>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct NoticeView {
    /// Unique within the feed: a window's selection.
    pub id: u64,
    pub kind: NoticeKind,
    /// "Item", "Map": the kind as a word, for its icon's description.
    pub kind_name: String,
    /// `HH:mm:ss`.
    pub time: String,
    pub message: String,
    /// How it reads when copied or shared: "22:10:05 [Item] Dragon Scale x2".
    pub line: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct NoticeFilter {
    /// Empty for All.
    pub kind: Option<NoticeKind>,
    pub label: String,
    pub count: u32,
    /// What the list says while this filter shows nothing.
    pub empty: String,
}

/// What the list says when `filter` shows nothing (VibeSkua `EmptyText`).
pub fn empty_text(filter: Option<NoticeKind>) -> String {
    match filter {
        None => "No events yet. Drops, deaths, relogins, quests and script starts and stops show up here.".to_owned(),
        Some(kind) => format!("No {} events yet.", kind.word()),
    }
}

/// Whether `entry` shows under `filter`.
pub fn matches(filter: Option<NoticeKind>, entry: &Notification) -> bool {
    filter.is_none_or(|kind| NoticeKind::of(entry.kind) == kind)
}

pub fn view(bot: &Arc<Bot>) -> NotificationsView {
    let entries = bot.notifications.entries();
    let filters = FILTERS
        .iter()
        .map(|(kind, label)| NoticeFilter {
            kind: *kind,
            label: (*label).to_owned(),
            count: entries.iter().filter(|e| matches(*kind, e)).count() as u32,
            empty: empty_text(*kind),
        })
        .collect();
    NotificationsView {
        entries: entries
            .iter()
            .map(|e| NoticeView {
                id: e.id,
                kind: NoticeKind::of(e.kind),
                kind_name: e.kind.name().to_owned(),
                time: e.time_display(),
                message: e.message.clone(),
                line: e.line(),
            })
            .collect(),
        filters,
    }
}

/// Clear: every event of the tab's feed goes.
pub fn clear(bot: &Arc<Bot>) {
    bot.notifications.clear();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_count_their_events_and_say_when_empty() {
        let bot = Bot::new();
        bot.notifications.add(NotificationKind::Item, "Dragon Scale x2");
        bot.notifications.add(NotificationKind::System, "Joined battleon-1");
        bot.notifications.add(NotificationKind::Item, "Gold x100");
        let view = view(&bot);
        assert_eq!(view.entries.len(), 3);
        assert_eq!(view.entries[0].message, "Gold x100", "newest first");
        assert_eq!(view.entries[1].kind, NoticeKind::Map);
        assert!(view.entries[0].line.ends_with("[Item] Gold x100"));
        let count = |label: &str| view.filters.iter().find(|f| f.label == label).unwrap().count;
        assert_eq!(
            (count("All"), count("Items"), count("Maps"), count("Errors")),
            (3, 2, 1, 0)
        );
        let errors = view.filters.iter().find(|f| f.label == "Errors").unwrap();
        assert_eq!(errors.empty, "No error events yet.");
        clear(&bot);
        assert!(super::view(&bot).entries.is_empty());
    }
}
