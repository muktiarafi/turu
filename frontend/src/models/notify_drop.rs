//! Combat > Runtime > Notifications (VibeSkua `NotifyDropView` / `NotifyDropViewModel`): a list
//! of item names, and a beep (`Platform::beep`, VibeSkua's `ISoundService.Beep`) played a number
//! of times when one of them drops. As VibeSkua, the list isn't saved: it lasts as long as the tab
//! (closing the window keeps it), and each tab has its own. Where the platform posts alerts
//! (Android), a drop is also a notification, heard with the screen off.
//!
//! The list and the beep settings are per tab ([`State`]); the drop handler reads them. Every
//! function here locks the state only briefly and never while calling the bot, so a shell may call
//! them from a frame.

use super::tab_state::{PerTab, Shared, lock};
use serde::Serialize;
use serde_json::Value as Json;
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, Instant};
use turu_bot::Bot;
use turu_bot::events::BotEvent;

/// VibeSkua's `NumericUpDown`s: beeps per drop, and milliseconds between them.
pub const COUNT: (u32, u32) = (1, 25);
pub const DELAY: (u32, u32) = (10, 2000);
/// How long the same item's drop alerts only once ([`alert`]).
const ALERT_AGAIN_AFTER: Duration = Duration::from_secs(10);

pub struct State {
    items: Vec<String>,
    /// VibeSkua `SoundCount`.
    count: u32,
    /// VibeSkua `SoundDelay`.
    delay: u32,
    /// The drop handler's ID: subscribed while the list has items (VibeSkua registers on the
    /// first Add and unregisters when Remove empties it).
    subscription: Option<u64>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            count: 5,
            delay: 200,
            subscription: None,
        }
    }
}

pub static STATE: PerTab<State> = PerTab::new();

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct NotifyDropView {
    /// The watched names, in the order added.
    pub items: Vec<String>,
    /// Beeps per drop ([`COUNT`]).
    pub count: u32,
    pub count_min: u32,
    pub count_max: u32,
    /// Milliseconds between beeps ([`DELAY`]).
    pub delay: u32,
    pub delay_min: u32,
    pub delay_max: u32,
    /// The platform also posts a notification for a drop (Android).
    pub alerts: bool,
}

pub fn view(bot: &Arc<Bot>) -> NotifyDropView {
    let state = STATE.of(bot);
    let s = lock(&state);
    NotifyDropView {
        items: s.items.clone(),
        count: s.count,
        count_min: COUNT.0,
        count_max: COUNT.1,
        delay: s.delay,
        delay_min: DELAY.0,
        delay_max: DELAY.1,
        alerts: crate::platform::platform().posts_alerts(),
    }
}

/// VibeSkua `AddDrop`: `|`-separated, trimmed, names not in the list yet.
pub fn add(bot: &Arc<Bot>, input: &str) {
    let state = STATE.of(bot);
    let added = {
        let mut s = lock(&state);
        let before = s.items.len();
        for name in split(input) {
            if !s.items.iter().any(|i| i == name) {
                s.items.push(name.to_owned());
            }
        }
        s.items.len() > before
    };
    if added {
        subscribe(bot, &state);
    }
}

/// VibeSkua `RemoveDrops`: unwatches when the list is empty after it.
pub fn remove(bot: &Arc<Bot>, names: &[String]) {
    let state = STATE.of(bot);
    let empty = {
        let mut s = lock(&state);
        s.items.retain(|i| !names.contains(i));
        s.items.is_empty()
    };
    if empty {
        unsubscribe(bot, &state);
    }
}

/// VibeSkua `RemoveAllDropsCommand` (the list's `Clear`).
pub fn clear(bot: &Arc<Bot>) {
    let state = STATE.of(bot);
    lock(&state).items.clear();
    unsubscribe(bot, &state);
}

/// Beeps per drop, kept within [`COUNT`].
pub fn set_count(bot: &Arc<Bot>, count: u32) {
    lock(&STATE.of(bot)).count = count.clamp(COUNT.0, COUNT.1);
}

/// Milliseconds between beeps, kept within [`DELAY`].
pub fn set_delay(bot: &Arc<Bot>, delay: u32) {
    lock(&STATE.of(bot)).delay = delay.clamp(DELAY.0, DELAY.1);
}

/// Test beep: plays the alert's beeps.
pub fn test(bot: &Arc<Bot>) {
    play(&STATE.of(bot));
}

/// The control API's actions (`POST /models/notify-drop/do`, `action` in the body).
pub fn act(bot: &Arc<Bot>, action: &str, body: &Json) -> Result<(), String> {
    let number = |key: &str| {
        body.get(key)
            .and_then(Json::as_u64)
            .map(|n| n.min(u32::MAX.into()) as u32)
            .ok_or_else(|| format!("{key} is missing"))
    };
    match action {
        "add" => add(bot, body.get("input").and_then(Json::as_str).ok_or("input is missing")?),
        "remove" => {
            let names: Vec<String> = body
                .get("items")
                .and_then(Json::as_array)
                .ok_or("items is missing")?
                .iter()
                .filter_map(Json::as_str)
                .map(str::to_owned)
                .collect();
            remove(bot, &names);
        }
        "clear" => clear(bot),
        "count" => set_count(bot, number("count")?),
        "delay" => set_delay(bot, number("delay")?),
        "test" => test(bot),
        other => return Err(format!("no action {other}")),
    }
    Ok(())
}

/// VibeSkua `PlayBeepsAsync`, on a thread of its own: `count` beeps, `delay` ms apart.
fn play(state: &Mutex<State>) {
    let (count, delay) = {
        let s = lock(state);
        (s.count, Duration::from_millis(s.delay.into()))
    };
    let spawned = std::thread::Builder::new()
        .name("Turu Notify Drop".into())
        .spawn(move || {
            for i in 0..count {
                crate::platform::platform().beep();
                if i + 1 < count {
                    std::thread::sleep(delay);
                }
            }
        });
    if let Err(e) = spawned {
        tracing::warn!("Notify Drop beep failed: {e}");
    }
}

/// Watches `bot`'s drops: a listed item that drops (VibeSkua's `ItemDroppedMessage`, sent for the
/// drop and again when it reaches the inventory) plays the beeps. The handler runs where the event
/// is emitted (also inside `Bot::on_flash_call`): it only locks the list and starts threads.
fn subscribe(bot: &Arc<Bot>, state: &Shared<State>) {
    if lock(state).subscription.is_some() {
        return;
    }
    let watch: Weak<Mutex<State>> = Arc::downgrade(state);
    let tab = Arc::downgrade(bot);
    let id = bot.events.subscribe(move |event| {
        let (BotEvent::ItemDropped { item } | BotEvent::ItemAdded { item, .. }) = event else {
            return;
        };
        let Some(watch) = watch.upgrade() else { return };
        let listed = lock(&watch).items.contains(&item.name);
        if listed {
            play(&watch);
            alert(tab.clone(), &item.name);
        }
    });
    let mut s = lock(state);
    if s.subscription.is_some() {
        // Another caller subscribed meanwhile: one handler is enough.
        drop(s);
        bot.events.unsubscribe(id);
    } else {
        s.subscription = Some(id);
    }
}

fn unsubscribe(bot: &Arc<Bot>, state: &Shared<State>) {
    let id = lock(state).subscription.take();
    if let Some(id) = id {
        bot.events.unsubscribe(id);
    }
}

/// The drop's alert where the platform posts them (Android's notification, heard with the screen
/// off), on a thread of its own: the drop handler runs inside `Bot::on_flash_call` too. Once per
/// drop: the handler hears it drop and again reach the inventory, so the same item within
/// [`ALERT_AGAIN_AFTER`] alerts once.
fn alert(tab: Weak<Bot>, item: &str) {
    static LAST: Mutex<Option<(String, Instant)>> = Mutex::new(None);
    if !crate::platform::platform().posts_alerts() {
        return;
    }
    {
        let mut last = lock(&LAST);
        if last
            .as_ref()
            .is_some_and(|(name, at)| name == item && at.elapsed() < ALERT_AGAIN_AFTER)
        {
            return;
        }
        *last = Some((item.to_owned(), Instant::now()));
    }
    let item = item.to_owned();
    let spawned = std::thread::Builder::new().name("Turu Alert".into()).spawn(move || {
        let Some(bot) = tab.upgrade() else { return };
        let text = format!("{}: {item}", crate::turu::tab_label(&bot));
        tracing::info!("Alert: {item} dropped: {text}");
        crate::platform::platform().alert(crate::platform::Alert::Drop, &format!("{item} dropped"), &text);
    });
    if let Err(e) = spawned {
        tracing::warn!("Notify Drop alert failed: {e}");
    }
}

/// The names in `input`: split on `|`, trimmed, empty ones dropped.
fn split(input: &str) -> impl Iterator<Item = &str> {
    input.split('|').map(str::trim).filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_splits_on_bars_and_skips_duplicates() {
        let bot = Bot::new();
        let subscribed = || lock(&STATE.of(&bot)).subscription.is_some();
        add(&bot, " Void Aura | | Void Aura|Gem of Mastery ");
        assert_eq!(view(&bot).items, ["Void Aura", "Gem of Mastery"]);
        assert!(subscribed());
        add(&bot, "Gem of Mastery|Dragon Runestone");
        assert_eq!(view(&bot).items, ["Void Aura", "Gem of Mastery", "Dragon Runestone"]);
        remove(&bot, &["Void Aura".into(), "Dragon Runestone".into()]);
        assert_eq!(view(&bot).items, ["Gem of Mastery"]);
        assert!(subscribed());
        remove(&bot, &["Gem of Mastery".into()]);
        assert!(!subscribed());
    }

    #[test]
    fn beep_settings_stay_in_range() {
        let bot = Bot::new();
        act(&bot, "count", &serde_json::json!({ "count": 99 })).unwrap();
        act(&bot, "delay", &serde_json::json!({ "delay": 1 })).unwrap();
        let v = view(&bot);
        assert_eq!((v.count, v.delay), (COUNT.1, DELAY.0));
        assert!(act(&bot, "nope", &Json::Null).is_err());
    }
}
