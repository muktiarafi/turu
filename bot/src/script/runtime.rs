//! Runs one script in QuickJS on the script thread.
//!
//! Natives given to JS (wrapped with types by bot/js/turu.ts):
//! * `__turu(method, argsJson) -> resultJson`: `{"ok": value}` or `{"error": msg, "stopped": bool}`.
//! * `__turuEvents() -> eventsJson`: events since the last poll, `[{"name", "data"}]`.

use super::loader::{ScriptLoader, ScriptResolver, normalize};
use crate::Bot;
use crate::error::BotError;
use crate::events::BotEvent;
use parking_lot::Mutex;
use rquickjs::{CatchResultExt, CaughtError, Context, Function, Module, Object, Runtime};
use serde_json::{Value, json};
use std::collections::VecDeque;
use std::path::Path;
use std::sync::Arc;

pub enum RunError {
    Stopped,
    Failed(String),
}

/// Most events a script may have waiting before old ones are dropped.
const MAX_QUEUED_EVENTS: usize = 1000;

pub fn run(bot: &Arc<Bot>, path: &Path) -> Result<(), RunError> {
    let queue: Arc<Mutex<VecDeque<Value>>> = Arc::default();
    let subscription = {
        let queue = queue.clone();
        bot.events.subscribe(move |event: &BotEvent| {
            // Packets are frequent; scripts that want them read them through `extensionPacket`.
            let mut queue = queue.lock();
            if queue.len() >= MAX_QUEUED_EVENTS {
                queue.pop_front();
            }
            queue.push_back(json!({ "name": event.name(), "data": event.payload() }));
        })
    };
    let result = run_inner(bot, path, queue);
    bot.events.unsubscribe(subscription);
    result
}

fn run_inner(
    bot: &Arc<Bot>,
    path: &Path,
    queue: Arc<Mutex<VecDeque<Value>>>,
) -> Result<(), RunError> {
    let failed = |e: rquickjs::Error| RunError::Failed(e.to_string());
    let runtime = Runtime::new().map_err(failed)?;
    runtime.set_loader(ScriptResolver, ScriptLoader);
    let control = bot.control.clone();
    // Stops scripts that loop without calling the bot (and ones that catch ScriptStopped).
    runtime.set_interrupt_handler(Some(Box::new(move || control.should_exit())));
    let context = Context::full(&runtime).map_err(failed)?;

    let entry = normalize(path).to_string_lossy().into_owned();
    context.with(|ctx| {
        let globals = ctx.globals();
        let native_bot = bot.clone();
        let turu = Function::new(ctx.clone(), move |method: String, args: String| -> String {
            call(&native_bot, &method, &args)
        })
        .map_err(failed)?;
        globals.set("__turu", turu).map_err(failed)?;
        let events = Function::new(ctx.clone(), move || -> String {
            let drained: Vec<Value> = queue.lock().drain(..).collect();
            Value::Array(drained).to_string()
        })
        .map_err(failed)?;
        globals.set("__turuEvents", events).map_err(failed)?;

        let outcome = (|| {
            let namespace: Object = Module::import(&ctx, entry.as_str())?.finish()?;
            let main: Option<Function> = namespace.get("default")?;
            let Some(main) = main else {
                return Err(rquickjs::Error::new_from_js_message(
                    "module",
                    "function",
                    "the script has no `export default function main()`",
                ));
            };
            super::set_running(bot);
            main.call::<_, rquickjs::Value>(())?;
            Ok(())
        })()
        .catch(&ctx);
        match outcome {
            Ok(()) => Ok(()),
            Err(_) if bot.should_exit() => Err(RunError::Stopped),
            Err(CaughtError::Exception(e)) if e.message().as_deref() == Some("ScriptStopped") => {
                Err(RunError::Stopped)
            }
            Err(CaughtError::Exception(e)) => {
                let mut text = e.message().unwrap_or_else(|| "error".into());
                if let Some(stack) = e.stack().filter(|s| !s.is_empty()) {
                    text.push('\n');
                    text.push_str(stack.trim_end());
                }
                Err(RunError::Failed(text))
            }
            Err(other) => Err(RunError::Failed(other.to_string())),
        }
    })
}

/// One `__turu` call: dispatches to the Rust API and wraps the result for JS.
fn call(bot: &Arc<Bot>, method: &str, args: &str) -> String {
    let args: Vec<Value> = match serde_json::from_str(args) {
        Ok(Value::Array(a)) => a,
        Ok(other) => vec![other],
        Err(e) => {
            return json!({ "error": format!("bad arguments for {method}: {e}") }).to_string();
        }
    };
    match crate::api::dispatch(bot, method, &args) {
        Ok(value) => json!({ "ok": value }).to_string(),
        Err(BotError::Stopped) => json!({ "error": "ScriptStopped", "stopped": true }).to_string(),
        Err(e) => json!({ "error": e.to_string() }).to_string(),
    }
}
