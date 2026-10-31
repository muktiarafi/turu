//! The script API (VibeSkua `IScriptInterface` and its sub-interfaces), one module each.
//!
//! Every module exposes `pub fn dispatch(bot: &Arc<Bot>, method: &str, args: Args) -> BotResult<Value>`
//! for the TypeScript binding (`__turu("player.join", "[\"battleon\"]")`), next to plain Rust
//! functions the rest of the engine calls. Method names are camelCase, exactly as in
//! `bot/js/turu.ts`; keep the two in sync.

pub mod army;
pub mod auras;
pub mod boosts;
pub mod combat;
pub mod config;
pub mod drops;
pub mod handlers;
pub mod hunt;
pub mod inventory;
pub mod kill;
pub mod lite;
pub mod map;
pub mod monsters;
pub mod options;
pub mod packets;
pub mod player;
pub mod quests;
pub mod reputation;
pub mod send;
pub mod servers;
pub mod shops;
pub mod skills;
pub mod wait;

use crate::Bot;
use crate::error::{BotError, BotResult};
use serde::Serialize;
use serde_json::Value;
use std::sync::Arc;

/// Routes `"module.method"` to the module's `dispatch`.
pub fn dispatch(bot: &Arc<Bot>, method: &str, args: &[Value]) -> BotResult<Value> {
    let args = Args(args);
    let (module, name) = method.split_once('.').unwrap_or(("", method));
    match module {
        "" => root(bot, name, args),
        "player" => player::dispatch(bot, name, args),
        "map" => map::dispatch(bot, name, args),
        "combat" => combat::dispatch(bot, name, args),
        "kill" => kill::dispatch(bot, name, args),
        "hunt" => hunt::dispatch(bot, name, args),
        "monsters" => monsters::dispatch(bot, name, args),
        "quests" => quests::dispatch(bot, name, args),
        "drops" => drops::dispatch(bot, name, args),
        "handlers" => handlers::dispatch(bot, name, args),
        "boosts" => boosts::dispatch(bot, name, args),
        "inventory" | "bank" | "tempInv" | "house" | "invHelper" => {
            inventory::dispatch(bot, module, name, args)
        }
        "shops" => shops::dispatch(bot, name, args),
        "skills" => skills::dispatch(bot, name, args),
        "wait" => wait::dispatch(bot, name, args),
        "send" => send::dispatch(bot, name, args),
        "servers" => servers::dispatch(bot, name, args),
        "lite" => lite::dispatch(bot, name, args),
        "options" => options::dispatch(bot, name, args),
        "config" => config::dispatch(bot, name, args),
        "reputation" => reputation::dispatch(bot, name, args),
        "self" => auras::dispatch(bot, auras::Subject::Self_, name, args),
        "target" => auras::dispatch(bot, auras::Subject::Target, name, args),
        "army" => army::dispatch(bot, name, args),
        "flash" => flash_dispatch(bot, name, args),
        _ => Err(BotError::UnknownMethod(method.to_owned())),
    }
}

fn root(bot: &Arc<Bot>, name: &str, args: Args) -> BotResult<Value> {
    match name {
        "log" => {
            bot.log(args.str(0)?);
            ok(())
        }
        "sleep" => {
            bot.sleep(args.i64(0)?.max(0) as u64)?;
            ok(())
        }
        "shouldExit" => ok(bot.should_exit()),
        // `showMessageBox(message, caption, buttons)`: the clicked button's index, or null.
        "showMessageBox" => {
            let button = bot.scripts.show_message_box(
                bot,
                args.str_or(1, ""),
                args.str(0)?,
                args.strings(2),
            )?;
            ok(button)
        }
        "coreBotsOptions" => {
            let username = player::username(bot)?;
            if username.is_empty() {
                return ok(Value::Null);
            }
            let path = bot.paths.cbo_file(&username);
            let lines = crate::cbo::read_lines(&path, |old, new| {
                bot.log(format!("[CBO Migration] Renamed \"{old}\" → \"{new}\""));
            })
            .map_err(|e| BotError::Other(format!("couldn't read {}: {e}", path.display())))?;
            ok(lines)
        }
        "stop" => {
            bot.control.request_stop();
            Err(BotError::Stopped)
        }
        _ => Err(BotError::UnknownMethod(name.to_owned())),
    }
}

/// Raw access for scripts that need something the API doesn't cover (`bot.flash.get(...)`).
fn flash_dispatch(bot: &Arc<Bot>, name: &str, args: Args) -> BotResult<Value> {
    let flash = &bot.flash;
    match name {
        "call" => {
            let rest = args.0.iter().skip(1).cloned().map(Into::into).collect();
            ok(flash.call(args.str(0)?, rest)?)
        }
        "get" => ok(flash.get_opt::<Value>(args.str(0)?)?),
        "getStatic" => ok(flash.get_static::<Value>(args.str(0)?)?),
        "set" => {
            flash.set(args.str(0)?, args.value(1).clone())?;
            ok(())
        }
        "isNull" => ok(flash.is_null(args.str(0)?)?),
        "callGameFunction" => {
            let rest = args.0.iter().skip(1).cloned().map(Into::into).collect();
            ok(flash.call_game_function_json::<Value>(args.str(0)?, rest)?)
        }
        _ => Err(BotError::UnknownMethod(format!("flash.{name}"))),
    }
}

pub fn ok(value: impl Serialize) -> BotResult<Value> {
    serde_json::to_value(value).map_err(|e| BotError::Other(e.to_string()))
}

/// Positional arguments of a script call.
#[derive(Clone, Copy)]
pub struct Args<'a>(pub &'a [Value]);

static NULL: Value = Value::Null;

impl<'a> Args<'a> {
    pub fn value(&self, i: usize) -> &'a Value {
        self.0.get(i).unwrap_or(&NULL)
    }

    pub fn is_missing(&self, i: usize) -> bool {
        self.value(i).is_null()
    }

    pub fn str(&self, i: usize) -> BotResult<&'a str> {
        self.value(i).as_str().ok_or_else(|| {
            BotError::InvalidArgument(format!("argument {} must be a string", i + 1))
        })
    }

    pub fn opt_str(&self, i: usize) -> Option<&'a str> {
        self.value(i).as_str()
    }

    pub fn str_or(&self, i: usize, default: &'a str) -> &'a str {
        self.opt_str(i).unwrap_or(default)
    }

    pub fn i64(&self, i: usize) -> BotResult<i64> {
        self.opt_i64(i).ok_or_else(|| {
            BotError::InvalidArgument(format!("argument {} must be a number", i + 1))
        })
    }

    pub fn opt_i64(&self, i: usize) -> Option<i64> {
        let v = self.value(i);
        v.as_i64().or_else(|| v.as_f64().map(|f| f as i64))
    }

    pub fn i64_or(&self, i: usize, default: i64) -> i64 {
        self.opt_i64(i).unwrap_or(default)
    }

    pub fn f64_or(&self, i: usize, default: f64) -> f64 {
        self.value(i).as_f64().unwrap_or(default)
    }

    pub fn bool_or(&self, i: usize, default: bool) -> bool {
        self.value(i).as_bool().unwrap_or(default)
    }

    /// A string or number argument, for APIs that take an item/monster by name or ID.
    pub fn name_or_id(&self, i: usize) -> BotResult<NameOrId<'a>> {
        match self.value(i) {
            Value::String(s) => Ok(NameOrId::Name(s)),
            Value::Number(n) => Ok(NameOrId::Id(n.as_i64().unwrap_or_default())),
            _ => Err(BotError::InvalidArgument(format!(
                "argument {} must be a name or an ID",
                i + 1
            ))),
        }
    }

    /// An array of strings (a single string counts as one).
    pub fn strings(&self, i: usize) -> Vec<String> {
        match self.value(i) {
            Value::Array(items) => items
                .iter()
                .filter_map(|v| match v {
                    Value::String(s) => Some(s.clone()),
                    Value::Number(n) => Some(n.to_string()),
                    _ => None,
                })
                .collect(),
            Value::String(s) => vec![s.clone()],
            _ => Vec::new(),
        }
    }

    /// An array of numbers (a single number counts as one).
    pub fn i64s(&self, i: usize) -> Vec<i64> {
        match self.value(i) {
            Value::Array(items) => items.iter().filter_map(Value::as_i64).collect(),
            Value::Number(n) => n.as_i64().into_iter().collect(),
            _ => Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum NameOrId<'a> {
    Name(&'a str),
    Id(i64),
}
