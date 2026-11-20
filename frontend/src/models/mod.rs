//! View models (docs/NATIVE_UI_PROPOSAL.md, L2): what a user can see and do about one feature,
//! as plain data and actions, with no UI in them. The egui windows draw them; a native shell reads
//! the same views over the tab's control API (`GET /models/<name>`, `event: model` on `/events`)
//! and sends the actions back as routes.
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

pub mod game_options;
pub mod logs;
pub mod scripts;

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

/// Adds every model's view and actions to the control API (before it starts), for a native
/// shell's screens: `GET /models/<name>`, `POST /models/<name>/<action>`.
pub fn register() {
    use serde_json::{Value, json};
    use turu_bot::control_api::{add_model, add_model_action};
    fn text(body: &Value, name: &str) -> Result<String, String> {
        body.get(name)
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| format!("{name} is missing"))
    }
    fn done() -> Result<Value, String> {
        Ok(json!({}))
    }

    add_model("scripts", Box::new(|bot| to_json(&scripts::view(bot))));
    add_model_action(
        "scripts",
        "start",
        Box::new(|bot, _| scripts::start(bot).and_then(|()| done())),
    );
    add_model_action(
        "scripts",
        "stop",
        Box::new(|bot, _| {
            scripts::stop(bot);
            done()
        }),
    );
    add_model_action(
        "scripts",
        "pause",
        Box::new(|bot, _| {
            scripts::set_paused(bot, true);
            done()
        }),
    );
    add_model_action(
        "scripts",
        "resume",
        Box::new(|bot, _| {
            scripts::set_paused(bot, false);
            done()
        }),
    );
    add_model_action(
        "scripts",
        "favourite",
        Box::new(|bot, _| {
            scripts::toggle_favourite(bot);
            done()
        }),
    );
    add_model_action(
        "scripts",
        "load",
        Box::new(|bot, body| scripts::load(bot, std::path::Path::new(&text(body, "path")?)).and_then(|()| done())),
    );

    add_model("logs", Box::new(|bot| to_json(&logs::view(bot))));
    add_model_action(
        "logs",
        "clear",
        Box::new(|bot, body| logs::clear(bot, &text(body, "kind")?).and_then(|()| done())),
    );

    add_model("game-options", Box::new(|bot| to_json(&game_options::view(bot))));
    add_model_action(
        "game-options",
        "set",
        Box::new(|bot, body| {
            let value = body.get("value").ok_or("value is missing")?;
            game_options::set(bot, &text(body, "label")?, value).and_then(|()| done())
        }),
    );
    add_model_action(
        "game-options",
        "save",
        Box::new(|bot, _| game_options::save(bot).and_then(|()| done())),
    );
    add_model_action(
        "game-options",
        "revert",
        Box::new(|bot, _| {
            game_options::revert(bot, None);
            done()
        }),
    );
    add_model_action(
        "game-options",
        "reset",
        Box::new(|bot, _| {
            game_options::reset(bot);
            done()
        }),
    );
}

fn to_json(view: &impl Serialize) -> serde_json::Value {
    serde_json::to_value(view).unwrap_or(serde_json::Value::Null)
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
}
