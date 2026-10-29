//! `bot.skills` (VibeSkua `ScriptSkill`). The skill loop itself is crate::skills.

use super::{Args, ok};
use crate::Bot;
use crate::error::{BotError, BotResult};
use crate::skills::{self, SkillUseMode};
use serde_json::Value;
use std::sync::Arc;

pub fn dispatch(bot: &Arc<Bot>, name: &str, args: Args) -> BotResult<Value> {
    let runner = &bot.skills;
    match name {
        "timerRunning" => ok(runner.timer_running()),
        "startAdvanced" => {
            runner.start_advanced(
                bot,
                args.opt_str(0),
                args.bool_or(1, true),
                args.str_or(2, "Base"),
            )?;
            ok(())
        }
        "startSkills" => {
            let mode = args
                .opt_str(2)
                .map_or(Some(SkillUseMode::WaitForCooldown), SkillUseMode::parse);
            let Some(mode) = mode else {
                return Err(BotError::InvalidArgument(format!(
                    "unknown skill use mode `{}`",
                    args.str_or(2, "")
                )));
            };
            runner.start_skills(bot, args.str(0)?, args.i64_or(1, 250), mode);
            ok(())
        }
        "stop" => {
            runner.stop();
            ok(())
        }
        "pause" => {
            runner.pause();
            ok(())
        }
        "resume" => {
            runner.resume();
            ok(())
        }
        "canUseSkill" => ok(skills::can_use_skill(bot, args.i64(0)?)?),
        "useSkill" => {
            skills::use_skill(bot, args.i64(0)?)?;
            ok(())
        }
        _ => Err(BotError::UnknownMethod(format!("skills.{name}"))),
    }
}
