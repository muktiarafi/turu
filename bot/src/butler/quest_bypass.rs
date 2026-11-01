//! Client-side quest and level fakes for a butler (VibeSkua ButlerV5 `QuestBypass` and
//! `QuestClone.UnlockAll`). Some cells are locked and some monsters untargetable until a story
//! quest is done: telling this client the quest is done (`bot.quests.updateQuest`'s fake
//! `updateQuest` packet) opens them. Nothing here reaches the server.

use super::{Level, trace};
use crate::Bot;
use crate::api::{quests, send};
use serde_json::Value;
use std::sync::atomic::AtomicBool;

/// The quests to fake per map, to see its cells and bosses without the story (VibeSkua's
/// table). A map may have several; all are applied.
pub const MAP_BYPASSES: &[(&str, i64)] = &[
    // Ultras and army bosses.
    ("ultradage", 793),
    ("ultradarkon", 8733),
    ("ultradrago", 8395),
    ("ultraspeaker", 9125),
    ("ultradrakath", 3879),
    ("doomvault", 2954),
    ("doomvault", 3008),
    ("victormatsuri", 10294), // Masakado
    ("astralshrine", 9802),   // Astral Empyrean
    ("astralshrine", 9803),
    ("novashrine", 9802),
    ("bocklincastle", 102520),
    ("bocklingrove", 10239),
    ("necroproject", 9901),
    // CoreArmyLite's former blanket list, by map.
    ("wolfwing", 598),       // Wolfwing Evil End
    ("doomvaultb", 3004),    // Grim Underdungeon XXIX
    ("doomvaultb", 3008),    // I Command You, Help Me!
    ("towerofdoom10", 3484), // Defeat Slugbutter
    ("finalbattle", 3799),   // Beat Death!
    ("mummies", 4616),
    ("gluttony", 5915), // Glutus, Take 2
    ("borgars", 7522),  // Burglinster's Revenge
    ("downbelow", 8107),
    ("manacradle", 9126),  // Once Upon Another Time
    ("liatarahill", 9814), // Changeling
    // Story bosses and locked cells.
    ("vath", 354),
    ("chaoscave", 567),
    ("chaoscave", 597),
    ("ledgermayne", 847),
    ("chaoslord", 3879),
    ("chaoslord", 3880),
    ("queenbattle", 8361),
    ("trygve", 8298),
    ("thunderfang", 1170),
    ("shadowattack", 3799),
    ("hakuwar", 9607),
    ("championdrakath", 2814),
    ("alteonbattle", 3824),
    // Farming and quest maps.
    ("necrodungeon", 2059),
    ("battleunderc", 935),
    ("wanders", 976),
    ("wanders", 3773),
    ("titanattack", 8777),
    ("techfortress", 7646),
    ("stonewooddeep", 7650),
    ("blindingsnow", 899),
    ("mummies", 4614),
    ("pyramid", 4614),
    ("maloth", 6000),
    ("moonyardb", 1176),
    ("sandsea", 811),
    ("zorbaspalace", 7484),
    ("voidrefuge", 9531),
    ("darkoviagrave", 498),
    ("backroom", 8060),
    ("pyrewatch", 4077),
    ("astravia", 8000),
    ("firestorm", 1542),
    ("void", 904),
    ("rangda", 7622),
    ("starfest", 8094),
    ("badmoon", 9844),
    ("dawnfortress", 8297),
    ("dawnsanctum", 8297),
];

/// doomvaultb is gated by an achievement (CoreArmyLite `SetAchievement(18)`).
pub const ACHIEVEMENT_PACKET: &str =
    r#"{"t":"xt","b":{"r":-1,"o":{"cmd":"setAchievement","field":"ia0","index":18,"value":1}}}"#;

pub const LEVEL_PACKET: &str =
    r#"{"t":"xt","b":{"r":-1,"o":{"cmd":"levelUp","intExpToLevel":"0","intLevel":100}}}"#;

/// The `objData` strings holding the quest slots: 100 slots each, one base-36 character a slot.
const QUEST_STRINGS: [&str; 9] = [
    "strQuests",
    "strQuests2",
    "strQuests3",
    "strQuests4",
    "strQuests5",
    "strQuests6",
    "strQuests7",
    "strQuests8",
    "strQuests9",
];

/// The highest slot value.
const MAX_QUEST_CHAR: char = 'z';

/// The table's quests for `map`.
pub fn bypass_ids(map: &str) -> Vec<i64> {
    MAP_BYPASSES
        .iter()
        .filter(|(m, _)| m.eq_ignore_ascii_case(map))
        .map(|&(_, id)| id)
        .collect()
}

/// The "Custom bypass quest IDs" option ("1234, 5678"); anything else is skipped.
pub fn parse_custom_ids(csv: &str) -> Vec<i64> {
    csv.split(',')
        .filter_map(|p| p.trim().parse::<i64>().ok())
        .filter(|&id| id > 0)
        .collect()
}

/// Fakes the custom quest IDs (when the follow starts).
pub fn apply_custom(bot: &Bot, ids: &[i64], cancel: &AtomicBool) {
    if ids.is_empty() {
        trace(bot, "QuestBypass", Level::Info, "no custom quest IDs set");
        return;
    }
    let list: Vec<String> = ids.iter().map(i64::to_string).collect();
    trace(
        bot,
        "QuestBypass",
        Level::Info,
        format!("custom option has {} ID(s): {}", ids.len(), list.join(",")),
    );
    for &id in ids {
        fake_quest(bot, id, "custom option", cancel);
    }
}

/// Fakes the table's quests for `map`, and doomvaultb's achievement.
pub fn apply_for_map(bot: &Bot, map: &str, cancel: &AtomicBool) {
    if map.is_empty() {
        return;
    }
    let ids = bypass_ids(map);
    if ids.is_empty() {
        trace(
            bot,
            "QuestBypass",
            Level::Info,
            format!("map '{map}': no bypass entries"),
        );
    } else {
        let list: Vec<String> = ids.iter().map(i64::to_string).collect();
        trace(
            bot,
            "QuestBypass",
            Level::Info,
            format!(
                "map '{map}': {} bypass entr{}: {}",
                ids.len(),
                if ids.len() == 1 { "y" } else { "ies" },
                list.join(",")
            ),
        );
        for id in ids {
            fake_quest(bot, id, &format!("map {map}"), cancel);
        }
    }
    if map.eq_ignore_ascii_case("doomvaultb") {
        trace(
            bot,
            "QuestBypass",
            Level::Info,
            "doomvaultb: sending the achievement fake (ia0/18)",
        );
        if let Err(e) = send::client_packet(bot, ACHIEVEMENT_PACKET, "json") {
            trace(
                bot,
                "QuestBypass",
                Level::Error,
                format!("achievement fake failed: {e}"),
            );
        }
    }
}

/// Loads the quest and sends the fake (VibeSkua `Quests.Load` + `UpdateQuest(id)`).
fn fake_quest(bot: &Bot, id: i64, reason: &str, cancel: &AtomicBool) {
    let result =
        quests::load(bot, &[id]).and_then(|()| quests::update_quest_cancellable(bot, id, cancel));
    match result {
        Ok(true) => trace(
            bot,
            "QuestBypass",
            Level::Success,
            format!("UpdateQuest({id}) sent ({reason})"),
        ),
        Ok(false) => trace(
            bot,
            "QuestBypass",
            Level::Warning,
            format!("UpdateQuest({id}) skipped ({reason}): the quest didn't load"),
        ),
        Err(e) => trace(
            bot,
            "QuestBypass",
            Level::Error,
            format!("UpdateQuest({id}) FAILED ({reason}): {e}"),
        ),
    }
}

/// Shows this client as level 100, for level-gated maps (VibeSkua `ApplyLevelFake`).
pub fn apply_level_fake(bot: &Bot) {
    let level = bot
        .flash
        .get_or::<i64>("world.myAvatar.dataLeaf.intLevel", 0);
    if !matches!(level, Ok(l) if l < 100) {
        return;
    }
    trace(
        bot,
        "QuestBypass",
        Level::Info,
        "applying the client-side level 100 fake",
    );
    if let Err(e) = send::client_packet(bot, LEVEL_PACKET, "json") {
        trace(
            bot,
            "QuestBypass",
            Level::Error,
            format!("level fake failed: {e}"),
        );
    }
}

/// Maxes every quest slot on this client (VibeSkua `QuestClone.UnlockAll`): every quest-locked
/// cell, monster and map opens. Undone by a relogin.
pub fn unlock_all(bot: &Bot) {
    let ready = crate::api::player::logged_in(bot).unwrap_or(false)
        && crate::api::map::loaded(bot).unwrap_or(false);
    if !ready {
        return;
    }
    let data: Option<Value> = bot.flash.get_opt("world.myAvatar.objData").ok().flatten();
    let strings: Vec<(&str, usize)> = QUEST_STRINGS
        .iter()
        .filter_map(|&field| {
            let value = data.as_ref()?.get(field)?.as_str()?;
            Some((field, value.chars().count()))
        })
        .collect();
    if strings.is_empty() {
        trace(
            bot,
            "QuestClone",
            Level::Warning,
            "unlock-all: the objData quest strings aren't readable yet - skipping",
        );
        return;
    }
    let mut maxed = 0;
    for (field, len) in strings {
        if len == 0 {
            continue;
        }
        let max: String = std::iter::repeat_n(MAX_QUEST_CHAR, len).collect();
        match bot
            .flash
            .set(&format!("world.myAvatar.objData.{field}"), max)
        {
            Ok(()) => maxed += 1,
            Err(e) => trace(
                bot,
                "QuestClone",
                Level::Error,
                format!("unlock-all: setting {field} failed: {e}"),
            ),
        }
    }
    trace(
        bot,
        "QuestClone",
        Level::Success,
        format!("unlocked all quests (client-side, {maxed} strings maxed to '{MAX_QUEST_CHAR}')"),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_and_the_custom_ids() {
        assert_eq!(MAP_BYPASSES.len(), 64);
        assert_eq!(bypass_ids("DoomVaultB"), [3004, 3008]);
        assert!(bypass_ids("battleon").is_empty());
        assert_eq!(parse_custom_ids(" 1234, x, 5678 ,-3,,0"), [1234, 5678]);
    }
}
