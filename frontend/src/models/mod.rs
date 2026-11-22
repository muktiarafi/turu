//! View models (docs/NATIVE_UI_PROPOSAL.md, L2): what a user can see and do about one feature,
//! as plain data and actions, with no UI in them. The egui windows draw them; a native shell reads
//! the same views over the tab's control API (`GET /models/<name>`, `event: model` on `/events`)
//! and sends the actions back (`POST /models/<name>/do`), each model's as one enum the shells'
//! types are generated from.
//!
//! Rules (the proposal's "The shape of the API"):
//!
//! * A view is read from cached state (`bot.state`, `bot.scripts`, `bot.logs`, ...) and never calls
//!   Flash, so any thread may read it, the UI thread included.
//! * A view says what to show and what is allowed, not how to lay it out: no window tabs, no field
//!   order beyond the data's, no strings that only make sense in one layout. Words a user reads
//!   whatever the layout (a state, why a control is disabled) belong here, so every shell says
//!   the same.
//! * Actions return at once: work that touches the game goes through `turu::spawn_action`.
//! * No egui here (`models_import_no_ui` checks the sources): L1's client split moves this module
//!   to `turu_client` as it is.

pub mod application_options;
pub mod army;
pub mod army_client;
pub mod army_travel;
pub mod auto;
pub mod bank;
pub mod butler;
pub mod client_update;
pub mod console;
pub mod core_bots;
pub mod cosmetics;
pub mod crashes;
pub mod drops;
pub mod fast_travel;
pub mod form;
pub mod game_options;
pub mod grabber;
pub mod hotkeys;
pub mod jump;
pub mod junk;
pub mod loader;
pub mod loadouts;
pub mod logs;
pub mod notifications;
pub mod notify_drop;
pub mod packets;
pub mod prompts;
pub mod rail;
pub mod repo;
pub mod runtime;
pub mod scheduler;
pub mod script_options;
pub mod scripts;
pub mod skills;
pub mod stats;
pub mod tab_state;

use serde::Serialize;

/// Whether a control is offered and why not, as every shell shows it: enabled, or disabled with
/// the reason (a tooltip on the desktop, a line under it on a touch screen).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct Control {
    pub enabled: bool,
    /// Why it's disabled; empty while enabled.
    pub reason: String,
}

impl Control {
    pub fn on() -> Self {
        Self {
            enabled: true,
            reason: String::new(),
        }
    }

    pub fn off(reason: impl Into<String>) -> Self {
        Self {
            enabled: false,
            reason: reason.into(),
        }
    }

    /// Enabled when `enabled`, else disabled for `reason`.
    pub fn when(enabled: bool, reason: impl Into<String>) -> Self {
        if enabled { Self::on() } else { Self::off(reason) }
    }
}

/// `text` with each whole-word `word` (ASCII case ignored) replaced by `with`: Streamer Mode's
/// names out of a line.
pub fn replace_word(text: &str, word: &str, with: &str) -> String {
    let (bytes, needle) = (text.as_bytes(), word.as_bytes());
    let part_of_word = |b: Option<&u8>| b.is_some_and(|b| b.is_ascii_alphanumeric() || *b == b'_');
    let mut out = String::with_capacity(text.len());
    let (mut i, mut from) = (0, 0);
    while i + needle.len() <= bytes.len() {
        if bytes[i..i + needle.len()].eq_ignore_ascii_case(needle)
            && !part_of_word(i.checked_sub(1).and_then(|j| bytes.get(j)))
            && !part_of_word(bytes.get(i + needle.len()))
            && text.is_char_boundary(i)
            && text.is_char_boundary(i + needle.len())
        {
            out.push_str(&text[from..i]);
            out.push_str(with);
            i += needle.len();
            from = i;
        } else {
            i += 1;
        }
    }
    out.push_str(&text[from..]);
    out
}

/// Adds every model's view and actions to the control API (before it starts), for a native
/// shell's screens: `GET /models/<name>`, and `POST /models/<name>/do` with one of its actions.
pub fn register() {
    use serde_json::{Value, json};
    use std::sync::Arc;
    use turu_bot::Bot;
    use turu_bot::control_api::{add_model, add_model_action};
    fn done() -> Result<Value, String> {
        Ok(json!({}))
    }
    /// Model `name`'s actions, as one enum `A` (`POST /models/<name>/do`, the action's name in the
    /// body's `action`): the shells send `A` in the types generated from it, so an action or a
    /// field changed here fails their build, not a tap.
    fn acts<A: serde::de::DeserializeOwned>(
        name: &'static str,
        run: impl Fn(&Arc<Bot>, A) -> Result<Value, String> + Send + Sync + 'static,
    ) {
        add_model_action(
            name,
            "do",
            Box::new(move |bot, body| {
                run(
                    bot,
                    A::deserialize(body).map_err(|e| format!("not a {name} action: {e}"))?,
                )
            }),
        );
    }

    add_model("scripts", Box::new(|bot| to_json(&scripts::view(bot))));
    acts("scripts", |bot, act| scripts::act(bot, act).and_then(|()| done()));
    add_model("logs", Box::new(|bot| to_json(&logs::view(bot))));
    acts("logs", |bot, act| logs::act(bot, act).and_then(|()| done()));
    add_model("crashes", Box::new(|_| to_json(&crashes::view())));
    acts("crashes", |_, act| crashes::act(act));
    add_model("notifications", Box::new(|bot| to_json(&notifications::view(bot))));
    acts("notifications", |bot, act| {
        notifications::act(bot, act);
        done()
    });
    add_model("drops", Box::new(|bot| to_json(&drops::view(bot))));
    acts("drops", |bot, act| {
        drops::act(bot, act);
        done()
    });
    add_model("stats", Box::new(|bot| to_json(&stats::view(bot))));
    acts("stats", stats::act);
    add_model("auto", Box::new(|bot| to_json(&auto::view(bot))));
    acts("auto", |bot, act| auto::act(bot, act).and_then(|()| done()));
    add_model("jump", Box::new(|bot| to_json(&jump::view(bot))));
    acts("jump", |bot, act| {
        jump::act(bot, act);
        done()
    });
    add_model("bank", Box::new(|bot| to_json(&bank::view(bot))));
    acts("bank", |bot, act| {
        bank::act(bot, act);
        done()
    });
    add_model("client-update", Box::new(|_| to_json(&client_update::view())));
    acts("client-update", |_, act| client_update::act(act).and_then(|()| done()));
    add_model("rail", Box::new(|bot| to_json(&rail::view(bot))));
    acts("rail", |bot, act| rail::act(bot, act).and_then(|()| done()));
    add_model("repo", Box::new(|bot| to_json(&repo::view(bot))));
    acts("repo", repo::act);
    add_model("scheduler", Box::new(|bot| to_json(&scheduler::view(bot))));
    acts("scheduler", |bot, act| scheduler::act(bot, act).and_then(|()| done()));
    add_model("army", Box::new(|_| to_json(&army::view())));
    acts("army", |_, act| army::act(act).and_then(|()| done()));
    acts("army-client", |_, act| army_client::act(act));
    add_model("army-travel", Box::new(|_| to_json(&army_travel::view())));
    acts("army-travel", |_, act| {
        army_travel::act(&army_travel::TRAVEL, act).and_then(|()| done())
    });
    add_model("app-options", Box::new(|bot| to_json(&application_options::view(bot))));
    acts("app-options", |bot, act| {
        application_options::act(bot, act).and_then(|()| done())
    });
    add_model("core-bots", Box::new(|bot| to_json(&core_bots::view(bot))));
    acts("core-bots", |bot, act| core_bots::act(bot, act).and_then(|()| done()));
    add_model("script-options", Box::new(|bot| to_json(&script_options::view(bot))));
    acts("script-options", |bot, act| {
        script_options::act(bot, act).and_then(|()| done())
    });
    add_model("cosmetics", Box::new(|bot| to_json(&cosmetics::view(bot))));
    acts("cosmetics", |bot, act| cosmetics::act(bot, act).and_then(|()| done()));
    add_model("butler", Box::new(|bot| to_json(&butler::view(bot))));
    acts("butler", butler::act);
    add_model("grabber", Box::new(|bot| to_json(&grabber::view(bot))));
    acts("grabber", grabber::act);
    add_model("console", Box::new(|bot| to_json(&console::view(bot))));
    acts("console", |bot, act| console::act(bot, act).and_then(|()| done()));
    add_model("packet-logger", Box::new(|bot| to_json(&packets::logger_view(bot))));
    acts("packet-logger", |bot, act| {
        packets::logger_act(bot, act).and_then(|()| done())
    });
    add_model("spammer", Box::new(|bot| to_json(&packets::spammer_view(bot))));
    acts("spammer", |bot, act| {
        packets::spammer_act(bot, act).and_then(|()| done())
    });
    add_model("interceptor", Box::new(|bot| to_json(&packets::interceptor_view(bot))));
    acts("interceptor", packets::interceptor_act);
    add_model("hotkeys", Box::new(|_| to_json(&hotkeys::view())));
    acts("hotkeys", |_, act| hotkeys::act(&hotkeys::STORE, act));
    add_model("prompts", Box::new(|bot| to_json(&prompts::view(bot))));
    acts("prompts", prompts::act);
    add_model("junk", Box::new(|bot| to_json(&junk::view(bot))));
    acts("junk", |bot, act| junk::act(bot, act).and_then(|()| done()));
    add_model("loader", Box::new(|bot| to_json(&loader::view(bot))));
    acts("loader", |bot, act| loader::act(bot, act).and_then(|()| done()));
    add_model("notify-drop", Box::new(|bot| to_json(&notify_drop::view(bot))));
    acts("notify-drop", |bot, act| {
        notify_drop::act(bot, act).and_then(|()| done())
    });
    add_model("runtime", Box::new(|bot| to_json(&runtime::view(bot))));
    acts("runtime", |bot, act| runtime::act(bot, act).and_then(|()| done()));
    add_model("fast-travel", Box::new(|bot| to_json(&fast_travel::view(bot))));
    acts("fast-travel", |bot, act| {
        fast_travel::act(bot, act).and_then(|()| done())
    });
    add_model("loadouts", Box::new(|bot| to_json(&loadouts::view(bot))));
    acts("loadouts", |bot, act| loadouts::act(bot, act).and_then(|()| done()));
    add_model("skills", Box::new(|bot| to_json(&skills::view(bot))));
    acts("skills", |bot, act| skills::act(bot, act).and_then(|()| done()));
    add_model("game-options", Box::new(|bot| to_json(&game_options::view(bot))));
    acts("game-options", |bot, act| {
        game_options::act(bot, act).and_then(|()| done())
    });
}

fn to_json(view: &impl Serialize) -> serde_json::Value {
    serde_json::to_value(view).unwrap_or(serde_json::Value::Null)
}

/// VibeSkua's fallback server list for the server boxes (when neither the server list API
/// nor the game's login screen gave one).
const SERVERS: [&str; 12] = [
    "Artix",
    "Twilly",
    "Galanoth",
    "Gravelyn",
    "Safiria",
    "Sir Ver",
    "Sepulchure",
    "Yorumi",
    "Alteon",
    "Espada",
    "Yokai",
    "Zorbot",
];

/// VibeSkua `ServersList`: the names from the server list API, else from the game's login
/// screen, else [`SERVERS`]; without duplicates (ignoring case), sorted.
pub fn server_names(bot: &turu_bot::Bot) -> Vec<String> {
    let names = |servers: Vec<turu_bot::models::Server>| -> Vec<String> {
        servers
            .into_iter()
            .map(|s| s.name.trim().to_owned())
            .filter(|n| !n.is_empty())
            .collect()
    };
    let mut list = names(bot.servers.cached());
    if list.is_empty() {
        list = names(bot.servers.listed());
    }
    if list.is_empty() {
        list = SERVERS.iter().map(|&n| n.to_owned()).collect();
    }
    list.sort_by_key(|n| n.to_lowercase());
    list.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
    list
}

#[cfg(test)]
mod tests {
    /// The models stay free of any UI toolkit, so L1's split can move them as they are.
    #[test]
    fn models_import_no_ui() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/models");
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            let text = std::fs::read_to_string(&path).unwrap();
            for line in text.lines().filter(|l| !l.trim_start().starts_with("//")) {
                for ui in ["egui", "crate::gui", "winit", "rfd"] {
                    assert!(
                        !line.contains(&format!("{ui}::")) && !line.contains(&format!("use {ui}")),
                        "{} uses {ui}: {line}",
                        path.display()
                    );
                }
            }
        }
    }

    /// Streamer Mode's masking replaces a name only as a whole word, in any case.
    #[test]
    fn replace_word_takes_whole_words() {
        let text = "Questwalker follows questwalker2; QUESTWALKER is on Twig.";
        assert_eq!(
            super::replace_word(text, "Questwalker", "Account 1"),
            "Account 1 follows questwalker2; Account 1 is on Twig."
        );
        assert_eq!(
            super::replace_word("Nobody here", "Questwalker", "Account 1"),
            "Nobody here"
        );
        assert_eq!(
            super::replace_word("Questwalker", "Questwalker", "Account 1"),
            "Account 1"
        );
    }
}
