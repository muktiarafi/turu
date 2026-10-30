//! `bot.self` and `bot.target` (VibeSkua `IScriptSelfAuras`, `IScriptTargetAuras`): the auras on
//! you and on your target, read through skua.swf's `Auras.as` callbacks.

use super::{Args, NameOrId, ok, player};
use crate::Bot;
use crate::error::{BotError, BotResult};
use crate::models::{Aura, AuraSnapshot};
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::sync::Arc;

/// Whose auras: you (`bot.self`) or your target (`bot.target`). The name is skua.swf's subject.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Subject {
    Self_,
    Target,
}

impl Subject {
    fn name(self) -> &'static str {
        match self {
            Self::Self_ => "Self",
            Self::Target => "Target",
        }
    }

    fn module(self) -> &'static str {
        match self {
            Self::Self_ => "self",
            Self::Target => "target",
        }
    }
}

pub fn dispatch(bot: &Arc<Bot>, subject: Subject, name: &str, args: Args) -> BotResult<Value> {
    match name {
        "auras" => ok(auras(bot, subject)?),
        "snapshots" => ok(snapshots(bot, subject)?),
        "getAura" => ok(find(auras(bot, subject)?, args.str(0)?, |a| &a.name)),
        "getAuraSnapshot" => ok(find(snapshots(bot, subject)?, args.str(0)?, |a| &a.name)),
        "getAuraStacks" => {
            ok(find(snapshots(bot, subject)?, args.str(0)?, |a| &a.name).map_or(0, |a| a.stacks))
        }
        "hasActiveAura" => ok(find(auras(bot, subject)?, args.str(0)?, |a| &a.name).is_some()),
        "getAuraValue" => {
            let raw = bot.flash.call(
                "GetAurasValue",
                vec![subject.name().into(), args.str(0)?.into()],
            )?;
            ok(raw.trim().trim_matches('"').parse::<f64>().unwrap_or(0.0))
        }
        "hasAnyActiveAura" => {
            let names = args.strings(0).join(",");
            ok(bot.flash.call_bool(
                "HasAnyActiveAura",
                vec![subject.name().into(), names.into()],
            )?)
        }
        "getPlayerAura" if subject == Subject::Self_ => ok(parse_list::<Aura>(
            &bot.flash.call("GetPlayerAura", vec![args.str(0)?.into()])?,
        )),
        "getMonsterAura" if subject == Subject::Target => {
            ok(monster_auras(bot, args.name_or_id(0)?)?)
        }
        _ => Err(BotError::UnknownMethod(format!(
            "{}.{name}",
            subject.module()
        ))),
    }
}

/// VibeSkua `Auras`: yours (`GetPlayerAura` with your lowercase name), or your target's
/// (`GetMonsterAuraByID` with its map ID, 0 without a target).
pub fn auras(bot: &Bot, subject: Subject) -> BotResult<Vec<Aura>> {
    match subject {
        Subject::Self_ => {
            let me = player::username(bot)?.to_lowercase();
            Ok(parse_list(
                &bot.flash.call("GetPlayerAura", vec![me.into()])?,
            ))
        }
        Subject::Target => {
            let map_id = player::target(bot)?.map_or(0, |m| m.map_id);
            monster_auras(bot, NameOrId::Id(map_id))
        }
    }
}

/// VibeSkua `GetMonsterAura(name | id)` (an ID is the monster's map ID).
fn monster_auras(bot: &Bot, monster: NameOrId) -> BotResult<Vec<Aura>> {
    let raw = match monster {
        NameOrId::Name(name) => bot.flash.call("GetMonsterAuraByName", vec![name.into()])?,
        NameOrId::Id(id) => bot.flash.call("GetMonsterAuraByID", vec![id.into()])?,
    };
    Ok(parse_list(&raw))
}

/// VibeSkua `Snapshots`: the auras as the HUD shows them (`GetAuraSnapshots`), without the
/// expired ones (see [`AuraSnapshot::expired`]).
pub fn snapshots(bot: &Bot, subject: Subject) -> BotResult<Vec<AuraSnapshot>> {
    let list: Vec<AuraSnapshot> = parse_list(
        &bot.flash
            .call("GetAuraSnapshots", vec![subject.name().into()])?,
    );
    Ok(list.into_iter().filter(|s| !s.expired()).collect())
}

/// A JSON list from skua.swf; empty when it's missing or malformed (VibeSkua catches the same).
fn parse_list<T: DeserializeOwned>(raw: &str) -> Vec<T> {
    serde_json::from_str::<Vec<Value>>(raw.trim())
        .unwrap_or_default()
        .into_iter()
        .filter_map(|v| serde_json::from_value(v).ok())
        .collect()
}

/// The first entry named `name`, ignoring case (as VibeSkua compares aura names).
fn find<T>(list: Vec<T>, name: &str, name_of: impl Fn(&T) -> &String) -> Option<T> {
    list.into_iter()
        .find(|x| name_of(x).eq_ignore_ascii_case(name))
}
