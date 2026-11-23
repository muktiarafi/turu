//! Turu: the process's client as one handle (docs/NATIVE_UI_PROPOSAL.md, L1's client part): its
//! tabs and their bots, the engine's shared services (the Army, the monitor hub, the script
//! repository), where it runs (headless, a shell's tab), and the view models read and acted on
//! in process, the same way a native shell reads them over the control API.
//!
//! A [`Client`] is a handle over the process's state, which stays where it was (`crate::turu`'s
//! statics): it is cheap to copy, every copy is the same client, and the free functions it
//! wraps still work. A process has one client: a desktop tab's (`turu --client`), `--headless`,
//! an Android slot, or Turu Mas with its several tabs.
//!
//! What a shell reads crosses as JSON already; in process, [`Client::act`] takes a model's typed
//! actions (`<Name>Act`) and [`Client::view`] answers the view as the control API does
//! (`{revision, view}`), so a front, a test or a later binding can drive a feature without HTTP.

use crate::models::prompts::{self, PromptAnswer, PromptView};
use crate::models::{Model, ModelActs, script_options};
use crate::turu::{self, TabId, TabRequest};
use serde_json::Value;
use std::sync::Arc;
use turu_bot::Bot;
use turu_bot::startup::Startup;

/// The process's client: a handle over its state ([`client`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Client {
    _one: (),
}

/// The process's client.
pub fn client() -> Client {
    Client { _one: () }
}

/// What a tab waits for the user to answer (docs/NATIVE_UI_PROPOSAL.md, L3), each with an ID an
/// answer names, so an answer to one already gone does nothing ([`Client::question`],
/// [`Client::answer`]).
#[derive(Debug, Clone, PartialEq)]
pub enum Question {
    /// A dialog (`models::prompts`): a script's message box, Butler's notice, the crash notice,
    /// "Send crash reports?", the Tab Limit, a graphics tip. Answered with one of its buttons.
    Prompt(PromptView),
    /// The script waits for its options (`bot.config.configure()`): edited with
    /// `ScriptOptionsAct::Open` and `Set` ([`Client::act`]), answered with Save (button 1) or
    /// Cancel (0, or none).
    ScriptOptions { id: String },
}

impl Question {
    /// Its ID, for [`Client::answer`].
    pub fn id(&self) -> &str {
        match self {
            Question::Prompt(prompt) => &prompt.id,
            Question::ScriptOptions { id } => id,
        }
    }
}

/// A model's view as the control API answers it: what it shows now, and a revision that moves
/// only when it changes.
#[derive(Debug, Clone, PartialEq)]
pub struct ViewCopy {
    pub revision: String,
    pub view: Value,
}

impl Client {
    /// The tab on screen's bot (the active tab's).
    pub fn bot(self) -> Arc<Bot> {
        turu::bot()
    }

    /// The tab on screen.
    pub fn active_tab(self) -> TabId {
        turu::active_tab()
    }

    /// Every tab and its bot, in order (one, except in Turu Mas).
    pub fn tabs(self) -> Vec<(TabId, Arc<Bot>)> {
        turu::tabs()
    }

    /// A tab's bot; `None` once it was closed.
    pub fn tab_bot(self, tab: TabId) -> Option<Arc<Bot>> {
        turu::tab_bot(tab)
    }

    /// Adds a tab with a new bot; its player is the frontend's to make ([`Client::request_tab`]).
    pub fn add_tab(self, startup: Startup) -> (TabId, Arc<Bot>) {
        turu::add_tab(startup)
    }

    /// Removes a tab; its bot, if it was there.
    pub fn remove_tab(self, tab: TabId) -> Option<Arc<Bot>> {
        turu::remove_tab(tab)
    }

    /// Shows a tab; false if there is no such tab.
    pub fn set_active_tab(self, tab: TabId) -> bool {
        turu::set_active_tab(tab)
    }

    /// Asks the frontend for a tab (a new one, closing one, ...); false if none listens.
    pub fn request_tab(self, request: TabRequest) -> bool {
        turu::request_tab(request)
    }

    /// Runs a bot action for `tab` off the caller's thread (bot calls block until the player
    /// answers); nothing if the tab is gone.
    pub fn spawn_action(
        self,
        tab: TabId,
        name: &str,
        action: impl FnOnce(&Arc<Bot>) -> turu_bot::BotResult<()> + Send + 'static,
    ) {
        if let Some(bot) = self.tab_bot(tab) {
            turu::spawn_action_for(bot, name, action);
        }
    }

    /// The Army: the process's clients it launched and controls.
    pub fn army(self) -> &'static Arc<turu_bot::army::Army> {
        turu::army()
    }

    /// The web monitor's hub: this process's tabs and its peers.
    pub fn monitor_hub(self) -> &'static Arc<turu_bot::monitor::Hub> {
        turu::monitor_hub()
    }

    /// Search Scripts' repository.
    pub fn script_repo(self) -> &'static Arc<turu_bot::repo::ScriptRepo> {
        turu::script_repo()
    }

    /// It runs without a window (`--headless`).
    pub fn headless(self) -> bool {
        turu::headless()
    }

    /// The shell it draws for (`--client=<address>`), if it's a desktop shell's tab.
    pub fn shell(self) -> Option<String> {
        turu::client_of()
    }

    /// Model `model`'s view for `tab`, as `GET /models/<name>` answers it.
    pub fn view(self, tab: TabId, model: Model) -> Result<ViewCopy, String> {
        let bot = self.tab_bot(tab).ok_or("no such tab")?;
        registered();
        let path = format!("/models/{}", model.name());
        let (status, body) = turu_bot::control_api::local(&bot, false, &path, &[]);
        let mut body = answered(status, body)?;
        let revision = match body.get("revision") {
            Some(Value::String(revision)) => revision.clone(),
            other => return Err(format!("no revision in {model:?}'s view: {other:?}")),
        };
        Ok(ViewCopy {
            revision,
            view: body.get_mut("view").map(Value::take).unwrap_or_default(),
        })
    }

    /// Runs one of a model's actions for `tab`, as `POST /models/<name>/do` does: its answer
    /// (`{}` for most), or why not.
    pub fn act<A: ModelActs>(self, tab: TabId, act: &A) -> Result<Value, String> {
        let bot = self.tab_bot(tab).ok_or("no such tab")?;
        registered();
        let body = serde_json::to_vec(act).map_err(|e| e.to_string())?;
        let path = format!("/models/{}/do", A::MODEL.name());
        let (status, answer) = turu_bot::control_api::local(&bot, true, &path, &body);
        answered(status, answer)
    }
}

impl Client {
    /// What `tab` waits for the user to answer, the most pressing first: a dialog (the script's
    /// message box before the rest), else the script's request for its options. Reading it takes
    /// nothing from the windows: an egui window still opens for it.
    pub fn question(self, tab: TabId) -> Option<Question> {
        let bot = self.tab_bot(tab)?;
        if let Some(prompt) = prompts::current(&bot) {
            return Some(Question::Prompt(prompt));
        }
        script_options::request_id(&bot).map(|id| Question::ScriptOptions { id })
    }

    /// Answers `tab`'s question `id` with `button` (its index; none when closed without one):
    /// what the answer asks the windows to open. An answer to a question already gone does
    /// nothing.
    pub fn answer(self, tab: TabId, id: &str, button: Option<usize>) -> Result<PromptAnswer, String> {
        let bot = self.tab_bot(tab).ok_or("no such tab")?;
        if id.starts_with("options:") {
            script_options::answer_request(&bot, id, button == Some(1))?;
            return Ok(PromptAnswer {
                window: String::new(),
                open: String::new(),
                report: String::new(),
            });
        }
        prompts::answer_prompt(&bot, id, button)
    }
}

/// The models' routes are added once, whether or not a control API runs (`turu::start_api` adds
/// them too).
fn registered() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(crate::models::register);
}

/// A route's answer: its body, or the error it gave.
fn answered(status: u16, body: Value) -> Result<Value, String> {
    if (200..300).contains(&status) {
        return Ok(body);
    }
    Err(match body.get("error") {
        Some(Value::String(error)) => error.clone(),
        _ => format!("{status}: {body}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::scripts::ScriptsAct;

    /// Answers the few game calls a script makes, as `bot/tests/runtime.rs`'s fake does.
    struct FakeGame;

    impl turu_bot::FlashHost for FakeGame {
        fn call(&self, name: &str, args: Vec<turu_bot::FlashArg>) -> Result<String, turu_bot::HostError> {
            if let Some(answer) = turu_bot::testing::batched(name, &args, |n, a| self.call(n, a).unwrap_or_default()) {
                return Ok(answer);
            }
            Ok(if name == "isLoggedIn" { "true" } else { "null" }.into())
        }
    }

    fn wait_for(what: &str, mut done: impl FnMut() -> bool) {
        let start = std::time::Instant::now();
        while !done() {
            assert!(start.elapsed() < std::time::Duration::from_secs(10), "{what}");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    /// L3's "done when": with no UI at all, a script's message box and its request for options
    /// are answered through the client, and the script goes on with what was answered.
    #[test]
    fn a_script_s_questions_are_answered_without_a_window() {
        use crate::models::form::OptionValue;
        use crate::models::script_options::ScriptOptionsAct;
        use turu_bot::script::options::MAIN;
        let dir = std::env::temp_dir().join(format!("turu-client-questions-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        turu_bot::paths::set_default_root(dir.join("data"));
        let script = dir.join("asks.ts");
        std::fs::write(
            &script,
            r#"import { bot, defineScriptOptions, option } from "turu";
export const scriptOptions = defineScriptOptions("Questwalker", [option.bool("On", "On", "")], true);
export default function main(): void {
    const picked = bot.showMessageBox("Pick one", "Questwalker asks", "No", "Yes");
    bot.log(`picked ${picked.value}`);
    bot.config.configure();
    bot.log(`on ${bot.config.get("On")}`);
}
"#,
        )
        .unwrap();
        let (tab, bot) = turu::add_tab(Startup::default());
        bot.set_host(Some(Arc::new(FakeGame)));
        bot.scripts.set_interactive(true);
        bot.scripts.load(&script);
        turu_bot::script::start(&bot).unwrap();

        // The message box: a prompt, answered Yes.
        wait_for("no message box", || {
            matches!(client().question(tab), Some(Question::Prompt(_)))
        });
        let Some(Question::Prompt(asked)) = client().question(tab) else {
            unreachable!()
        };
        assert_eq!((asked.title.as_str(), asked.buttons.len()), ("Questwalker asks", 2));
        assert!(
            client().answer(tab, "box:stale", Some(0)).is_ok(),
            "a stale answer does nothing"
        );
        client().answer(tab, &asked.id, Some(1)).unwrap();

        // The options: edited in the model, then Save.
        wait_for("no options request", || {
            matches!(client().question(tab), Some(Question::ScriptOptions { .. }))
        });
        let question = client().question(tab).unwrap();
        client().act(tab, &ScriptOptionsAct::Open).unwrap();
        let on = ScriptOptionsAct::Set {
            category: MAIN.into(),
            name: "On".into(),
            value: OptionValue::Switch(true),
        };
        client().act(tab, &on).unwrap();
        client().answer(tab, question.id(), Some(1)).unwrap();

        wait_for("the script didn't go on", || !bot.scripts.is_running());
        let log = bot.logs.script.text();
        assert!(log.contains("picked 1") && log.contains("on true"), "{log}");
        assert_eq!(client().question(tab), None);
        turu::remove_tab(tab);
    }

    #[test]
    fn every_copy_is_the_same_client() {
        assert_eq!(client(), client());
        assert_eq!(client().active_tab(), turu::active_tab());
        assert!(Arc::ptr_eq(&client().bot(), &turu::bot()));
    }

    #[test]
    fn a_models_view_reads_in_process() {
        let tab = client().active_tab();
        let first = client().view(tab, Model::Scripts).expect("the scripts view");
        assert!(first.view.is_object(), "{first:?}");
        let again = client().view(tab, Model::Scripts).expect("the scripts view");
        assert_eq!(first.revision, again.revision, "nothing changed");
    }

    #[test]
    fn a_models_typed_action_runs_in_process() {
        let tab = client().active_tab();
        // No script loaded: Stop does nothing and answers; Load of a missing file says why not.
        assert!(client().act(tab, &ScriptsAct::Stop).is_ok());
        let missing = ScriptsAct::Load {
            path: "/nonexistent/Questwalker.ts".into(),
        };
        assert!(client().act(tab, &missing).is_err());
    }

    #[test]
    fn a_closed_tab_has_no_view() {
        let gone = turu::add_tab(Startup::default()).0;
        turu::remove_tab(gone);
        assert_eq!(client().view(gone, Model::Scripts), Err("no such tab".into()));
    }

    /// An action read from what a shell sends serializes back to the same JSON, so
    /// [`Client::act`] posts what a shell would.
    fn round_trip<A: ModelActs>(sent: Value) {
        let act: A = serde_json::from_value(sent.clone()).expect("a shell's action");
        assert_eq!(serde_json::to_value(&act).unwrap(), sent);
    }

    #[test]
    fn actions_with_data_round_trip() {
        use crate::models::{game_options::GameOptionsAct, grabber::GrabberAct, prompts::PromptsAct};
        use serde_json::json;
        round_trip::<GrabberAct>(
            json!({ "action": "run", "kind": "Inventory", "label": "To Inventory", "positions": [1, 2], "revision": 3 }),
        );
        round_trip::<GrabberAct>(json!({ "action": "search", "search": "x", "page": null }));
        round_trip::<PromptsAct>(json!({ "action": "answer", "id": "box:7", "button": null }));
        round_trip::<GameOptionsAct>(json!({ "action": "save" }));
        for value in [
            json!({ "switch": true }),
            json!({ "number": 5 }),
            json!({ "text": "HighHP" }),
            json!("none"),
        ] {
            round_trip::<GameOptionsAct>(json!({ "action": "set", "label": "Skip cutscenes", "value": value }));
        }
    }

    #[test]
    fn actions_serialize_as_the_shells_send_them() {
        let body = serde_json::to_value(ScriptsAct::Load {
            path: "farm/Gold.ts".into(),
        })
        .unwrap();
        assert_eq!(body, serde_json::json!({ "action": "load", "path": "farm/Gold.ts" }));
        let back: ScriptsAct = serde_json::from_value(body).unwrap();
        assert_eq!(
            back,
            ScriptsAct::Load {
                path: "farm/Gold.ts".into()
            }
        );
    }
}
