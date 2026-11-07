//! The packet tools' "Show n / m" filters: the Interceptor's categories, matched by the commands
//! a packet carries (VibeSkua `PacketCommands`, `AppStartup/PacketInterceptor.cs` `CreateFilters`),
//! and the Logger's rules, matched on the packet's `%` fields (`AppStartup/PacketLogger.cs`).

use regex::Regex;
use std::sync::LazyLock;

/// A JSON packet's `"cmd":"x"`. Quotes inside a JSON string are escaped, so chat text can't
/// match it.
static JSON_CMD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#""cmd"\s*:\s*"([^"]+)""#).unwrap());
/// An XML packet's `action='x'` (either quote).
static XML_ACTION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"action=['"]([^'"]+)['"]"#).unwrap());

/// Every command a raw packet carries (VibeSkua `PacketCommands.Parse`): `%xt%` packets give
/// their command (and a `cmd` packet its sub-command), a JSON packet lists its nested ones too
/// (such as the auras in a `ct`), an XML packet its actions. Each once, in the order found.
pub fn commands(packet: &str) -> Vec<String> {
    let packet = packet.trim_start();
    if packet.starts_with('%') {
        return string_commands(packet);
    }
    let regex = if packet.starts_with('{') {
        &*JSON_CMD
    } else if packet.starts_with('<') {
        &*XML_ACTION
    } else {
        return Vec::new();
    };
    let mut found: Vec<String> = Vec::new();
    for capture in regex.captures_iter(packet) {
        let command = &capture[1];
        if !found.iter().any(|c| c == command) {
            found.push(command.to_owned());
        }
    }
    found
}

/// Outbound `%xt%zm%command%room%args%`, inbound `%xt%command%room%args%`. Only the first five
/// fields are read, so a packet full of `%` doesn't become a huge list.
fn string_commands(packet: &str) -> Vec<String> {
    let parts: Vec<&str> = packet
        .split('%')
        .filter(|p| !p.is_empty())
        .take(5)
        .collect();
    if parts.len() < 2 {
        return Vec::new();
    }
    if parts[1] != "zm" {
        return vec![parts[1].to_owned()];
    }
    if parts.len() < 3 {
        return Vec::new();
    }
    // "cmd" wraps a sub-command, such as %xt%zm%cmd%1%tfer%name%map%.
    let mut commands = vec![parts[2].to_owned()];
    if parts[2] == "cmd" && parts.len() >= 5 {
        commands.push(parts[4].to_owned());
    }
    commands
}

/// One of the Interceptor's categories: the commands of both directions of its requests.
pub struct Category {
    pub name: &'static str,
    pub commands: &'static [&'static str],
}

/// The Interceptor's categories, in VibeSkua's order; [`OTHER`] follows them.
pub const CATEGORIES: [Category; 17] = [
    Category {
        name: "Combat",
        commands: &[
            "ct",
            "gar",
            "sar",
            "sars",
            "anim",
            "mtls",
            "cb",
            "restRequest",
            "aggroMon",
            "resPlayerTimed",
            "resTimed",
            "respawnMon",
            "playerDeath",
        ],
    },
    Category {
        name: "User Data",
        commands: &[
            "retrieveUserData",
            "retrieveUserDatas",
            "initUserData",
            "initUserDatas",
        ],
    },
    Category {
        name: "Join",
        commands: &[
            "moveToArea",
            "tfer",
            "house",
            "joinOK",
            "firstJoin",
            "exitArea",
        ],
    },
    Category {
        name: "Jump",
        commands: &["moveToCell"],
    },
    Category {
        name: "Movement",
        commands: &["mv", "mtcid", "mvna", "mvnb"],
    },
    Category {
        name: "Get Map",
        commands: &["getMapItem"],
    },
    Category {
        name: "Quest",
        commands: &[
            "getQuest",
            "getQuests",
            "getQuests2",
            "acceptQuest",
            "tryQuestComplete",
            "updateQuest",
            "ccqr",
            "turnIn",
            "showQuestLink",
        ],
    },
    Category {
        name: "Shop",
        commands: &[
            "loadShop",
            "reloadShop",
            "loadHairShop",
            "loadEnhShop",
            "buyItem",
            "sellItem",
            "enhanceItem",
            "enhanceItemShop",
            "enhanceItemLocal",
        ],
    },
    Category {
        name: "Equip",
        commands: &[
            "equipItem",
            "forceEquipItem",
            "unequipItem",
            "wearItem",
            "unwearItem",
        ],
    },
    Category {
        name: "Drop",
        commands: &["getDrop", "dropItem", "getMonsterDrops", "monsterDrops"],
    },
    Category {
        name: "Chat",
        commands: &["message", "cc", "chatm", "whisper", "em", "emotea"],
    },
    Category {
        name: "Auras",
        commands: &[
            "aura+",
            "aura-",
            "aura++",
            "aura--",
            "aura*",
            "aura+p",
            "aura-p",
            "clearAuras",
            "auSnap",
            "showAuraResult",
        ],
    },
    Category {
        name: "Skills",
        commands: &["sAct", "gar", "sar", "sars", "geia", "seia"],
    },
    Category {
        name: "Stats",
        commands: &["uotls", "stu"],
    },
    Category {
        name: "Inventory",
        commands: &[
            "loadInventoryBig",
            "retrieveInventory",
            "initInventory",
            "addItems",
            "forceAddItem",
            "removeItem",
            "removeTempItem",
            "serverUseItem",
            "loadBank",
            "bankFromInv",
            "bankToInv",
            "bankSwapInv",
        ],
    },
    Category {
        name: "Class",
        commands: &["updateClass", "changeClass"],
    },
    Category {
        name: "Misc",
        commands: &["crafting", "setHomeTown", "afk", "summonPet"],
    },
];

/// The last Interceptor filter: packets no category lists.
pub const OTHER: &str = "Other";

/// The Interceptor's filters, as its flyout lists them: [`CATEGORIES`], then [`OTHER`].
pub fn interceptor_filters() -> Vec<&'static str> {
    CATEGORIES.iter().map(|c| c.name).chain([OTHER]).collect()
}

impl Category {
    /// Whether the packet with `commands` is in this category.
    pub fn matches(&self, commands: &[String]) -> bool {
        commands.iter().any(|c| self.commands.contains(&c.as_str()))
    }
}

/// Whether Interceptor filter `index` ([`interceptor_filters`]) takes the packet with `commands`:
/// a category when it lists one of them, Other when none does.
pub fn interceptor_filter_matches(index: usize, commands: &[String]) -> bool {
    match CATEGORIES.get(index) {
        Some(category) => category.matches(commands),
        None => !CATEGORIES.iter().any(|c| c.matches(commands)),
    }
}

/// Whether the Interceptor shows the packet with `commands`, `checked` holding each filter's
/// tick: a packet can be in several categories (a `ct` with auras is Combat and Auras), and any
/// ticked one shows it (VibeSkua `MatchesPacket`).
pub fn interceptor_shows(commands: &[String], checked: &[bool]) -> bool {
    checked
        .iter()
        .enumerate()
        .any(|(i, &on)| on && interceptor_filter_matches(i, commands))
}

/// The Logger's filters, in VibeSkua's order.
pub const LOGGER_FILTERS: [&str; 12] = [
    "Combat",
    "User Data",
    "Join",
    "Jump",
    "Movement",
    "Get Map",
    "Quest",
    "Shop",
    "Equip",
    "Drop",
    "Chat",
    "Misc",
];

/// Whether Logger filter `index` ([`LOGGER_FILTERS`]) takes the packet split into `p` (its
/// non-empty `%` fields, so `p[2]` is an outgoing packet's command).
fn logger_filter_matches(index: usize, p: &[&str]) -> bool {
    let at = |i: usize, names: &[&str]| p.len() > i && names.contains(&p[i]);
    match index {
        0 => at(2, &["restRequest", "gar", "aggroMon"]),
        1 => at(2, &["retrieveUserData", "retrieveUserDatas"]),
        // VibeSkua checks the length for both: a short "house" packet isn't Join.
        2 => p.len() >= 5 && (p[4] == "tfer" || p[2] == "house"),
        3 => at(2, &["moveToCell"]),
        4 => at(2, &["mv", "mtcid"]),
        5 => at(2, &["getMapItem"]),
        6 => at(
            2,
            &["getQuest", "acceptQuest", "tryQuestComplete", "updateQuest"],
        ),
        7 => at(2, &["loadShop", "buyItem", "sellItem"]),
        8 => at(2, &["equipItem"]),
        9 => at(2, &["getDrop"]),
        10 => at(2, &["message", "cc"]),
        11 => at(2, &["crafting", "setHomeTown", "afk", "summonPet"]),
        _ => false,
    }
}

/// Whether the Logger keeps `packet`, `checked` holding each [`LOGGER_FILTERS`] tick: dropped
/// when an unticked filter takes it (VibeSkua `LogPackets`), so a packet no filter takes is
/// always kept.
pub fn logger_keeps(packet: &str, checked: &[bool]) -> bool {
    if checked.iter().all(|&on| on) {
        return true;
    }
    let parts: Vec<&str> = packet.split('%').filter(|p| !p.is_empty()).collect();
    !checked
        .iter()
        .enumerate()
        .any(|(i, &on)| !on && logger_filter_matches(i, &parts))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cmds(packet: &str) -> Vec<String> {
        commands(packet)
    }

    /// The names of the Interceptor filters that take `packet`.
    fn categories(packet: &str) -> Vec<&'static str> {
        let commands = commands(packet);
        interceptor_filters()
            .into_iter()
            .enumerate()
            .filter(|(i, _)| interceptor_filter_matches(*i, &commands))
            .map(|(_, name)| name)
            .collect()
    }

    #[test]
    fn string_packets_give_their_command() {
        assert_eq!(cmds("%xt%zm%moveToCell%1234%Enter%Spawn%"), ["moveToCell"]);
        assert_eq!(cmds("%xt%ct%-1%{}%"), ["ct"]);
        assert_eq!(
            cmds("  %xt%uotls%-1%Hero%"),
            ["uotls"],
            "leading spaces are skipped"
        );
        // A cmd packet carries its sub-command too.
        assert_eq!(cmds("%xt%zm%cmd%1%tfer%Hero%battleon%"), ["cmd", "tfer"]);
        assert_eq!(cmds("%xt%zm%cmd%1%"), ["cmd"]);
    }

    #[test]
    fn short_or_odd_packets_give_nothing() {
        assert!(cmds("").is_empty());
        assert!(cmds("%xt%").is_empty());
        assert!(cmds("%xt%zm%").is_empty());
        assert!(cmds("hello").is_empty());
        assert!(cmds("Zabc").is_empty());
        // Only five fields are read: the rest of a long packet doesn't matter.
        let long = format!("%xt%zm%message%1%{}", "%".repeat(10_000));
        assert_eq!(cmds(&long), ["message"]);
    }

    #[test]
    fn json_packets_list_every_command_once() {
        let ct = r#"{"t":"xt","b":{"r":-1,"o":{"cmd":"ct","a":[{"cmd":"aura+","auras":[]},{"cmd" : "aura+"}],"p":{}}}}"#;
        assert_eq!(cmds(ct), ["ct", "aura+"]);
        // A quote inside chat text is escaped, so it isn't read as a command.
        let chat = r#"{"t":"xt","b":{"o":{"cmd":"chatm","msg":"say \"cmd\":\"ct\""}}}"#;
        assert_eq!(cmds(chat), ["chatm"]);
    }

    #[test]
    fn xml_packets_give_their_actions() {
        assert_eq!(
            cmds("<msg t='sys'><body action='verChk' r='0'></body></msg>"),
            ["verChk"]
        );
        assert_eq!(
            cmds(r#"<msg t="sys"><body action="login" r="0"></body></msg>"#),
            ["login"]
        );
    }

    #[test]
    fn packets_fall_in_their_categories() {
        assert_eq!(categories("%xt%zm%moveToCell%1%Enter%Spawn%"), ["Jump"]);
        assert_eq!(categories("%xt%zm%mv%1%400%300%8%"), ["Movement"]);
        assert_eq!(categories("%xt%zm%cmd%1%tfer%Hero%battleon%"), ["Join"]);
        assert_eq!(categories("%xt%zm%gar%1%aa>m:1%wvz%"), ["Combat", "Skills"]);
        assert_eq!(categories("%xt%serverUseItem%-1%+%1%"), ["Inventory"]);
        let ct = r#"{"t":"xt","b":{"o":{"cmd":"ct","a":[{"cmd":"aura+"}]}}}"#;
        assert_eq!(categories(ct), ["Combat", "Auras"]);
        // Nothing a category lists: Other.
        assert_eq!(categories("%xt%zm%somethingNew%1%"), [OTHER]);
        assert_eq!(categories("<msg><body action='verChk'/></msg>"), [OTHER]);
        assert_eq!(categories("garbage"), [OTHER]);
    }

    #[test]
    fn any_ticked_category_shows_a_packet() {
        let filters = interceptor_filters();
        assert_eq!(filters.len(), 18);
        assert_eq!(filters.last(), Some(&OTHER));
        let tick =
            |names: &[&str]| -> Vec<bool> { filters.iter().map(|f| names.contains(f)).collect() };
        let ct = commands(r#"{"t":"xt","b":{"o":{"cmd":"ct","a":[{"cmd":"aura+"}]}}}"#);
        assert!(interceptor_shows(&ct, &tick(&["Auras"])));
        assert!(interceptor_shows(&ct, &tick(&["Combat"])));
        assert!(!interceptor_shows(&ct, &tick(&["Chat", OTHER])));
        assert!(!interceptor_shows(&ct, &tick(&[])));
        let unknown = commands("%xt%zm%somethingNew%1%");
        assert!(interceptor_shows(&unknown, &tick(&[OTHER])));
        assert!(!interceptor_shows(&unknown, &tick(&["Combat"])));
    }

    #[test]
    fn the_logger_drops_what_an_unticked_filter_takes() {
        let all = [true; 12];
        let mut no_movement = all;
        no_movement[4] = false;
        assert!(logger_keeps("%xt%zm%mv%1%400%300%", &all));
        assert!(!logger_keeps("%xt%zm%mv%1%400%300%", &no_movement));
        assert!(!logger_keeps("%xt%zm%mtcid%1%3%", &no_movement));
        assert!(logger_keeps(
            "%xt%zm%moveToCell%1%Enter%Spawn%",
            &no_movement
        ));
        // Too short for the rule: kept (VibeSkua's Movement length fix).
        assert!(logger_keeps("%xt%mv%", &no_movement));
        // Join needs five fields, for "house" too.
        let mut no_join = all;
        no_join[2] = false;
        assert!(!logger_keeps("%xt%zm%cmd%1%tfer%Hero%battleon%", &no_join));
        assert!(!logger_keeps("%xt%zm%house%1%Hero%", &no_join));
        assert!(logger_keeps("%xt%zm%house%1%", &no_join));
        // What no filter takes is kept even with every filter unticked.
        assert!(logger_keeps("%xt%zm%somethingNew%1%", &[false; 12]));
        assert!(!logger_keeps("%xt%zm%message%1%hi%zone%", &[false; 12]));
    }
}
