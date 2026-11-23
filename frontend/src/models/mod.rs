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

/// Each model a shell reads, by its name on the control API (`/models/<name>`). The name comes
/// from here alone: [`register`] takes it from each view's and action's type ([`ModelView`],
/// [`ModelActs`]), and `turu-typegen` writes it for the shells beside the types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Model {
    Scripts,
    Logs,
    Crashes,
    Notifications,
    Drops,
    Stats,
    Auto,
    Jump,
    Bank,
    ClientUpdate,
    Rail,
    Repo,
    Scheduler,
    Army,
    ArmyClient,
    ArmyTravel,
    AppOptions,
    CoreBots,
    ScriptOptions,
    Cosmetics,
    Butler,
    Grabber,
    Console,
    PacketLogger,
    Spammer,
    Interceptor,
    HotKeys,
    Prompts,
    Junk,
    Loader,
    NotifyDrop,
    Runtime,
    FastTravel,
    Loadouts,
    Skills,
    GameOptions,
}

impl Model {
    pub const ALL: [Model; 36] = [
        Model::Scripts,
        Model::Logs,
        Model::Crashes,
        Model::Notifications,
        Model::Drops,
        Model::Stats,
        Model::Auto,
        Model::Jump,
        Model::Bank,
        Model::ClientUpdate,
        Model::Rail,
        Model::Repo,
        Model::Scheduler,
        Model::Army,
        Model::ArmyClient,
        Model::ArmyTravel,
        Model::AppOptions,
        Model::CoreBots,
        Model::ScriptOptions,
        Model::Cosmetics,
        Model::Butler,
        Model::Grabber,
        Model::Console,
        Model::PacketLogger,
        Model::Spammer,
        Model::Interceptor,
        Model::HotKeys,
        Model::Prompts,
        Model::Junk,
        Model::Loader,
        Model::NotifyDrop,
        Model::Runtime,
        Model::FastTravel,
        Model::Loadouts,
        Model::Skills,
        Model::GameOptions,
    ];

    /// Its name on the control API.
    pub fn name(self) -> &'static str {
        match self {
            Model::Scripts => "scripts",
            Model::Logs => "logs",
            Model::Crashes => "crashes",
            Model::Notifications => "notifications",
            Model::Drops => "drops",
            Model::Stats => "stats",
            Model::Auto => "auto",
            Model::Jump => "jump",
            Model::Bank => "bank",
            Model::ClientUpdate => "client-update",
            Model::Rail => "rail",
            Model::Repo => "repo",
            Model::Scheduler => "scheduler",
            Model::Army => "army",
            Model::ArmyClient => "army-client",
            Model::ArmyTravel => "army-travel",
            Model::AppOptions => "app-options",
            Model::CoreBots => "core-bots",
            Model::ScriptOptions => "script-options",
            Model::Cosmetics => "cosmetics",
            Model::Butler => "butler",
            Model::Grabber => "grabber",
            Model::Console => "console",
            Model::PacketLogger => "packet-logger",
            Model::Spammer => "spammer",
            Model::Interceptor => "interceptor",
            Model::HotKeys => "hotkeys",
            Model::Prompts => "prompts",
            Model::Junk => "junk",
            Model::Loader => "loader",
            Model::NotifyDrop => "notify-drop",
            Model::Runtime => "runtime",
            Model::FastTravel => "fast-travel",
            Model::Loadouts => "loadouts",
            Model::Skills => "skills",
            Model::GameOptions => "game-options",
        }
    }
}

/// A model's view: what `GET /models/<name>` answers.
pub trait ModelView: Serialize {
    const MODEL: Model;
}

/// A model's actions, as one enum: what `POST /models/<name>/do` takes.
pub trait ModelActs: serde::de::DeserializeOwned {
    const MODEL: Model;
}

/// Each model's view and actions.
macro_rules! model_types {
    ($($model:ident: $view:ty, $acts:ty;)*) => {
        $(
            impl ModelView for $view {
                const MODEL: Model = Model::$model;
            }
            impl ModelActs for $acts {
                const MODEL: Model = Model::$model;
            }
        )*
    };
}

model_types! {
    Scripts: scripts::ScriptsView, scripts::ScriptsAct;
    Logs: logs::LogsView, logs::LogsAct;
    Crashes: crashes::CrashesView, crashes::CrashesAct;
    Notifications: notifications::NotificationsView, notifications::NotificationsAct;
    Drops: drops::DropsView, drops::DropsAct;
    Stats: stats::StatsView, stats::StatsAct;
    Auto: auto::AutoView, auto::AutoAct;
    Jump: jump::JumpView, jump::JumpAct;
    Bank: bank::BankView, bank::BankAct;
    ClientUpdate: client_update::ClientUpdateView, client_update::ClientUpdateAct;
    Rail: rail::RailView, rail::RailAct;
    Repo: repo::RepoView, repo::RepoAct;
    Scheduler: scheduler::SchedulerView, scheduler::SchedulerAct;
    Army: army::ArmyView, army::ArmyAct;
    ArmyTravel: army_travel::ArmyTravelView, army_travel::ArmyTravelAct;
    AppOptions: application_options::AppOptionsView, application_options::AppOptionsAct;
    CoreBots: core_bots::CoreBotsView, core_bots::CoreBotsAct;
    ScriptOptions: script_options::ScriptOptionsView, script_options::ScriptOptionsAct;
    Cosmetics: cosmetics::CosmeticsView, cosmetics::CosmeticsAct;
    Butler: butler::ButlerView, butler::ButlerAct;
    Grabber: grabber::GrabberView, grabber::GrabberAct;
    Console: console::ConsoleView, console::ConsoleAct;
    PacketLogger: packets::PacketLoggerView, packets::PacketLoggerAct;
    Spammer: packets::SpammerView, packets::SpammerAct;
    Interceptor: packets::InterceptorView, packets::InterceptorAct;
    HotKeys: hotkeys::HotKeysView, hotkeys::HotKeysAct;
    Prompts: prompts::PromptsView, prompts::PromptsAct;
    Junk: junk::JunkView, junk::JunkAct;
    Loader: loader::LoaderView, loader::LoaderAct;
    NotifyDrop: notify_drop::NotifyDropView, notify_drop::NotifyDropAct;
    Runtime: runtime::RuntimeView, runtime::RuntimeAct;
    FastTravel: fast_travel::FastTravelView, fast_travel::FastTravelAct;
    Loadouts: loadouts::LoadoutsView, loadouts::LoadoutsAct;
    Skills: skills::SkillsView, skills::SkillsAct;
    GameOptions: game_options::GameOptionsView, game_options::GameOptionsAct;
}

/// Army Control's clients have no view of their own: their gear answers an action.
impl ModelActs for army_client::ArmyClientAct {
    const MODEL: Model = Model::ArmyClient;
}

/// A text a shell copies or shares (Copy, Copy the log): none when there's nothing to copy.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct CopiedText {
    pub text: Option<String>,
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
    /// Model `V::MODEL`'s view (`GET /models/<name>`).
    fn view<V: ModelView>(view: impl Fn(&Arc<Bot>) -> V + Send + Sync + 'static) {
        add_model(V::MODEL.name(), Box::new(move |bot| view_json(&view(bot))));
    }
    /// Model `A::MODEL`'s actions, as one enum `A` (`POST /models/<name>/do`, the action's name in
    /// the body's `action`): the shells send `A` in the types generated from it, so an action or a
    /// field changed here fails their build, not a tap.
    fn acts<A: ModelActs>(run: impl Fn(&Arc<Bot>, A) -> Result<Value, String> + Send + Sync + 'static) {
        let name = A::MODEL.name();
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

    view(|bot| scripts::view(bot));
    acts(|bot, act| scripts::act(bot, act).and_then(|()| done()));
    view(|bot| logs::view(bot));
    acts(|bot, act| logs::act(bot, act).and_then(|()| done()));
    view(|_| crashes::view());
    acts(|_, act| crashes::act(act));
    view(notifications::view);
    acts(|bot, act| {
        notifications::act(bot, act);
        done()
    });
    view(drops::view);
    acts(|bot, act| {
        drops::act(bot, act);
        done()
    });
    view(stats::view);
    acts(stats::act);
    view(auto::view);
    acts(|bot, act| auto::act(bot, act).and_then(|()| done()));
    view(jump::view);
    acts(|bot, act| {
        jump::act(bot, act);
        done()
    });
    view(bank::view);
    acts(|bot, act| {
        bank::act(bot, act);
        done()
    });
    view(|_| client_update::view());
    acts(|_, act| client_update::act(act).and_then(|()| done()));
    view(rail::view);
    acts(|bot, act| rail::act(bot, act).and_then(|()| done()));
    view(repo::view);
    acts(repo::act);
    view(scheduler::view);
    acts(|bot, act| scheduler::act(bot, act).and_then(|()| done()));
    view(|_| army::view());
    acts(|_, act| army::act(act).and_then(|()| done()));
    acts(|_, act| army_client::act(act));
    view(|_| army_travel::view());
    acts(|_, act| army_travel::act(&army_travel::TRAVEL, act).and_then(|()| done()));
    view(application_options::view);
    acts(|bot, act| application_options::act(bot, act).and_then(|()| done()));
    view(core_bots::view);
    acts(|bot, act| core_bots::act(bot, act).and_then(|()| done()));
    view(script_options::view);
    acts(|bot, act| script_options::act(bot, act).and_then(|()| done()));
    view(cosmetics::view);
    acts(|bot, act| cosmetics::act(bot, act).and_then(|()| done()));
    view(butler::view);
    acts(butler::act);
    view(grabber::view);
    acts(grabber::act);
    view(console::view);
    acts(|bot, act| console::act(bot, act).and_then(|()| done()));
    view(packets::logger_view);
    acts(|bot, act| packets::logger_act(bot, act).and_then(|()| done()));
    view(packets::spammer_view);
    acts(|bot, act| packets::spammer_act(bot, act).and_then(|()| done()));
    view(packets::interceptor_view);
    acts(packets::interceptor_act);
    view(|_| hotkeys::view());
    acts(|_, act| hotkeys::act(&hotkeys::STORE, act));
    view(prompts::view);
    acts(prompts::act);
    view(junk::view);
    acts(|bot, act| junk::act(bot, act).and_then(|()| done()));
    view(loader::view);
    acts(|bot, act| loader::act(bot, act).and_then(|()| done()));
    view(notify_drop::view);
    acts(|bot, act| notify_drop::act(bot, act).and_then(|()| done()));
    view(runtime::view);
    acts(|bot, act| runtime::act(bot, act).and_then(|()| done()));
    view(fast_travel::view);
    acts(|bot, act| fast_travel::act(bot, act).and_then(|()| done()));
    view(loadouts::view);
    acts(|bot, act| loadouts::act(bot, act).and_then(|()| done()));
    view(skills::view);
    acts(|bot, act| skills::act(bot, act).and_then(|()| done()));
    view(|bot| game_options::view(bot));
    acts(|bot, act| game_options::act(bot, act).and_then(|()| done()));
}

/// An action's answer as JSON.
fn to_json(answer: &impl Serialize) -> Result<serde_json::Value, String> {
    serde_json::to_value(answer).map_err(|e| e.to_string())
}

fn view_json(view: &impl Serialize) -> serde_json::Value {
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
    use super::Model;

    /// The answers' JSON is what the actions answered before they were typed.
    #[test]
    fn answers_keep_their_json() {
        use serde_json::json;
        fn json(answer: &impl serde::Serialize) -> serde_json::Value {
            serde_json::to_value(answer).unwrap()
        }
        assert_eq!(json(&super::CopiedText { text: None }), json!({ "text": null }));
        assert_eq!(
            json(&super::CopiedText { text: Some("a".into()) }),
            json!({ "text": "a" })
        );
        let text = super::crashes::CrashText {
            text: "t".into(),
            subject: "s".into(),
            cut: true,
        };
        assert_eq!(json(&text), json!({ "text": "t", "subject": "s", "cut": true }));
        assert_eq!(json(&super::stats::StatsCsv { csv: None }), json!({ "csv": null }));
        assert_eq!(
            json(&super::repo::LaunchScript { ready: false }),
            json!({ "ready": false })
        );
        let lines = super::army_client::ClientLog {
            lines: vec!["l".into()],
            next: 2,
        };
        assert_eq!(json(&lines), json!({ "lines": ["l"], "next": 2 }));
        assert_eq!(
            json(&super::hotkeys::KeyMoved {
                moved: "Auto Hunt".into()
            }),
            json!({ "moved": "Auto Hunt" })
        );
        let opened = super::prompts::PromptAnswer {
            window: String::new(),
            open: "crashes".into(),
            report: "exit-1.txt".into(),
        };
        assert_eq!(
            json(&opened),
            json!({ "window": "", "open": "crashes", "report": "exit-1.txt" })
        );
    }

    /// Every model is added with its actions, and with a view unless it has none; no two share
    /// a name.
    #[test]
    fn every_model_is_registered() {
        super::register();
        let (views, actions) = turu_bot::control_api::added_models();
        let mut names = std::collections::HashSet::new();
        for model in Model::ALL {
            let name = model.name();
            assert!(names.insert(name), "{name} twice");
            assert!(actions.contains(&(name, "do")), "{name} has no actions");
            assert_eq!(views.contains(&name), model != Model::ArmyClient, "{name}'s view");
        }
        assert_eq!(views.len(), Model::ALL.len() - 1, "a view of no model");
        assert_eq!(actions.len(), Model::ALL.len(), "an action of no model");
    }
    /// What a shell sends and reads stays typed (docs/NATIVE_UI_PROPOSAL.md, "Typed actions"),
    /// so a change on one side breaks the other's build rather than a tap:
    ///
    /// * a model's view and actions are added only through `register()`'s `view` and `acts`;
    /// * no action takes the raw body (`body: &Json`), and none answers with `json!`: an answer
    ///   is a record the generator writes for the shells;
    /// * the generator writes every model's types (`turu-typegen.rs`);
    /// * Android's pages read no hand-built JSON (`org.json`), but the files that carry the
    ///   control API's envelope or the shell's own messages.
    #[test]
    fn models_stay_typed() {
        use std::path::{Path, PathBuf};
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        fn files(dir: &Path, ext: &str, out: &mut Vec<PathBuf>) {
            for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    if !path.ends_with("build") {
                        files(&path, ext, out);
                    }
                } else if path.extension().is_some_and(|e| e == ext) {
                    out.push(path);
                }
            }
        }
        // Code outside the tests.
        let code = |path: &Path| {
            let text = std::fs::read_to_string(path).unwrap();
            match text.find("#[cfg(test)]") {
                Some(at) => text[..at].to_owned(),
                None => text,
            }
        };
        let mut rust = Vec::new();
        files(&root.join("src"), "rs", &mut rust);
        for path in &rust {
            let text = code(path);
            let name = path.strip_prefix(root).unwrap().display().to_string();
            let adds = text.matches("add_model_action(").count() + text.matches("add_model(").count();
            let expected = if name.ends_with("models/mod.rs") { 2 } else { 0 };
            assert_eq!(
                adds, expected,
                "{name} adds a model or an action itself: use register()'s view and acts"
            );
            if name.contains("models/") {
                assert!(
                    !takes_raw_body(&text),
                    "{name}: an action takes the raw body; give the model an `…Act` enum"
                );
                // `json!({})` is "done, nothing to answer"; an answer with fields is a record.
                for answer in ["Ok(json!({ \"", "Ok(serde_json::json!({ \""] {
                    assert!(
                        !text.contains(answer),
                        "{name}: an action answers with json!; answer a record"
                    );
                }
            }
        }
        let typegen = std::fs::read_to_string(root.join("src/bin/turu-typegen.rs")).unwrap();
        let generated = typegen.matches(".model::<").count() + typegen.matches(".acts::<").count();
        assert_eq!(
            generated,
            Model::ALL.len(),
            "turu-typegen.rs doesn't write every model's types"
        );

        let mut kotlin = Vec::new();
        files(&root.join("../android/app/src/main/kotlin"), "kt", &mut kotlin);
        assert!(!kotlin.is_empty(), "no Android sources at {}", root.display());
        const ENVELOPE_OR_SHELL: [&str; 5] = [
            "ModelClient.kt",
            "AccountsPanel.kt",
            "SlotShell.kt",
            "ShellState.kt",
            "ShellScreens.kt",
        ];
        for path in &kotlin {
            let text = std::fs::read_to_string(path).unwrap();
            let file = path.file_name().unwrap().to_string_lossy();
            if !ENVELOPE_OR_SHELL.contains(&file.as_ref()) {
                assert!(
                    !text.contains("import org.json."),
                    "{file} reads JSON by hand: use the generated types (`Api.<model>`, an answer's record)"
                );
            }
        }
    }

    /// Whether `text` has a parameter `body: &Json` (or `&Value`): an action reading the raw body.
    fn takes_raw_body(text: &str) -> bool {
        ["body: &Json", "body: &Value", "body: &serde_json::Value"]
            .iter()
            .any(|needle| {
                text.match_indices(needle)
                    .any(|(at, _)| !text[at + needle.len()..].starts_with(|c: char| c.is_alphanumeric() || c == '_'))
            })
    }

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
