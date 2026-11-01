//! Runs one script in QuickJS on the script thread.
//!
//! Natives given to JS (wrapped with types by bot/js/turu.ts):
//! * `__turu(method, argsJson) -> resultJson`: `{"ok": value, "due": [handler names]}` or
//!   `{"error": msg, "stopped": bool}`. The due handlers are the `bot.handlers` that must run now.
//! * `__turuEvents() -> eventsJson`: events since the last poll, `[{"name", "data"}]`.

use super::loader::{ScriptLoader, ScriptResolver, TURU_MODULE, normalize};
use crate::Bot;
use crate::error::BotError;
use crate::events::BotEvent;
use parking_lot::Mutex;
use rquickjs::{CatchResultExt, CaughtError, Context, Function, Module, Object, Runtime};
use serde_json::{Value, json};
use std::collections::VecDeque;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

/// Why a script or snippet ended other than by returning.
#[derive(Debug)]
pub enum RunError {
    Stopped,
    Failed(String),
}

/// Most events a script may have waiting before old ones are dropped.
const MAX_QUEUED_EVENTS: usize = 1000;

pub fn run(bot: &Arc<Bot>, path: &Path) -> Result<(), RunError> {
    with_events(bot, |queue| run_inner(bot, path, queue))
}

/// A Console snippet (already transpiled): evaluated as a script with the "turu" module's
/// exports (`bot`, ...) as globals. Returns the value of its last expression, formatted as
/// JSON. `on_log` gets each `bot.log` line as it is written.
pub fn run_snippet(bot: &Arc<Bot>, js: &str, on_log: LogSink) -> Result<String, RunError> {
    with_events(bot, |queue| run_snippet_inner(bot, js, queue, on_log))
}

/// Gets the `bot.log` lines of a Console snippet.
pub type LogSink = Arc<dyn Fn(&str) + Send + Sync>;

type EventQueue = Arc<Mutex<VecDeque<Value>>>;

/// Runs `f` with a queue of the bot's events, which `__turuEvents` hands to JS.
fn with_events<T>(bot: &Arc<Bot>, f: impl FnOnce(EventQueue) -> T) -> T {
    let queue: EventQueue = Arc::default();
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
    let result = f(queue);
    bot.events.unsubscribe(subscription);
    result
}

fn failed(e: rquickjs::Error) -> RunError {
    RunError::Failed(e.to_string())
}

/// A QuickJS runtime that Stop interrupts, with the module loader.
fn new_runtime(bot: &Arc<Bot>) -> Result<(Runtime, Context), RunError> {
    let runtime = Runtime::new().map_err(failed)?;
    runtime.set_loader(ScriptResolver, ScriptLoader);
    let control = bot.control.clone();
    // Stops scripts that loop without calling the bot (and ones that catch ScriptStopped).
    runtime.set_interrupt_handler(Some(Box::new(move || control.should_exit())));
    let context = Context::full(&runtime).map_err(failed)?;
    Ok((runtime, context))
}

/// Gives JS the natives bot/js/turu.ts calls: `__turu` and `__turuEvents`.
fn install_natives(
    ctx: &rquickjs::Ctx<'_>,
    bot: &Arc<Bot>,
    queue: EventQueue,
    on_log: Option<LogSink>,
) -> Result<(), RunError> {
    let globals = ctx.globals();
    let native_bot = bot.clone();
    let turu = Function::new(ctx.clone(), move |method: String, args: String| -> String {
        if let Some(on_log) = &on_log
            && method == "log"
            && let Ok(Value::Array(args)) = serde_json::from_str::<Value>(&args)
            && let Some(Value::String(line)) = args.first()
        {
            on_log(line);
        }
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
    Ok(())
}

fn run_inner(bot: &Arc<Bot>, path: &Path, queue: EventQueue) -> Result<(), RunError> {
    let (_runtime, context) = new_runtime(bot)?;
    let entry = normalize(path).to_string_lossy().into_owned();
    context.with(|ctx| {
        install_natives(&ctx, bot, queue, None)?;
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
        let result = outcome.map_err(|e| run_error(bot, e));
        if let Err(e) = &result {
            script_stopping(bot, &ctx, e);
        }
        result
    })
}

/// Turns a snippet's value into the Console's text: JSON, or what `String()` gives where JSON
/// has nothing to say (undefined, functions, errors, bigints, circular objects).
const FORMAT_SNIPPET_VALUE: &str = r#"(value) => {
    if (value === undefined) return "undefined";
    if (typeof value === "function" || typeof value === "symbol" || value instanceof Error) return String(value);
    if (typeof value === "bigint") return `${value}n`;
    try {
        return JSON.stringify(value, null, 2) ?? String(value);
    } catch (e) {
        return String(value);
    }
}"#;

fn run_snippet_inner(
    bot: &Arc<Bot>,
    js: &str,
    queue: EventQueue,
    on_log: LogSink,
) -> Result<String, RunError> {
    let (_runtime, context) = new_runtime(bot)?;
    context.with(|ctx| {
        install_natives(&ctx, bot, queue, Some(on_log))?;
        let outcome = (|| {
            let namespace: Object = Module::import(&ctx, TURU_MODULE)?.finish()?;
            let expose: Function = ctx.eval("(ns) => { Object.assign(globalThis, ns); }")?;
            expose.call::<_, ()>((namespace,))?;
            let value: rquickjs::Value = ctx.eval(js)?;
            let format: Function = ctx.eval(FORMAT_SNIPPET_VALUE)?;
            format.call::<_, String>((value,))
        })()
        .catch(&ctx);
        outcome.map_err(|e| run_error(bot, e))
    })
}

/// What a script's or snippet's uncaught error means: Stop, or a failure with its stack.
fn run_error(bot: &Bot, error: CaughtError<'_>) -> RunError {
    match error {
        _ if bot.should_exit() => RunError::Stopped,
        CaughtError::Exception(e) if e.message().as_deref() == Some("ScriptStopped") => {
            RunError::Stopped
        }
        CaughtError::Exception(e) => {
            let mut text = e.message().unwrap_or_else(|| "error".into());
            if let Some(stack) = e.stack().filter(|s| !s.is_empty()) {
                text.push('\n');
                text.push_str(stack.trim_end());
            }
            RunError::Failed(text)
        }
        other => RunError::Failed(other.to_string()),
    }
}

/// VibeSkua's `ScriptStopping` event, sent when the script was stopped or failed (not when
/// `main` returned): runs the script's `scriptStopping` handlers (CoreBots' `StopBot` goes to
/// the stop location there). Stop is reset first so they can use the bot, like VibeSkua's
/// handlers, which run off the script thread; pressing Stop again interrupts them.
fn script_stopping(bot: &Arc<Bot>, ctx: &rquickjs::Ctx<'_>, error: &RunError) {
    let Ok(Some(hook)) = ctx
        .globals()
        .get::<_, Option<Function>>("__turuScriptStopping")
    else {
        return;
    };
    let error = match error {
        RunError::Stopped => None,
        RunError::Failed(message) => Some(message.clone()),
    };
    bot.control.reset();
    super::set_stopping(bot);
    match hook.call::<_, ()>((error,)).catch(ctx) {
        Ok(()) => {}
        Err(_) if bot.should_exit() => {}
        Err(CaughtError::Exception(e)) if e.message().as_deref() == Some("ScriptStopped") => {}
        Err(e) => bot.log(format!("Error while the script was stopping: {e}")),
    }
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
        Ok(value) => {
            json!({ "ok": value, "due": bot.handlers.take_due(Instant::now()) }).to_string()
        }
        Err(BotError::Stopped) => json!({ "error": "ScriptStopped", "stopped": true }).to_string(),
        Err(e) => json!({ "error": e.to_string() }).to_string(),
    }
}
