//! The user's skill sets follow VibeSkua's AdvancedSkillContainer: seeded from the shipped
//! sets, then only gaining the shipped classes and modes they lack.

use serde_json::{Value, json};
use turu_bot::paths::Paths;

fn paths(name: &str) -> Paths {
    let root = std::env::temp_dir().join(format!("turu-skills-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    Paths { root }
}

fn shipped() -> Value {
    serde_json::from_str(turu_bot::DEFAULT_ADVANCED_SKILLS).unwrap()
}

#[test]
fn seeds_user_sets_from_the_shipped_ones() {
    let paths = paths("seed");
    paths.sync_advanced_skills().unwrap();
    let user: Value =
        serde_json::from_str(&std::fs::read_to_string(paths.user_advanced_skills_file()).unwrap())
            .unwrap();
    assert_eq!(user, shipped());
    assert!(
        user.get("Archivist of Time").is_some(),
        "the shipped sets are the current ones"
    );
}

#[test]
fn adds_missing_classes_and_modes_but_keeps_the_users() {
    let paths = paths("merge");
    paths.ensure().unwrap();
    let mine = json!({
        "ArchMage": { "Base": { "skillUseMode": "UseIfAvailable", "skillTimeout": 1, "skills": [] } },
        "My Class": { "Farm": { "skillUseMode": "WaitForCooldown", "skillTimeout": 2, "skills": [] } }
    });
    std::fs::write(paths.user_advanced_skills_file(), mine.to_string()).unwrap();
    paths.sync_advanced_skills().unwrap();

    let user: Value =
        serde_json::from_str(&std::fs::read_to_string(paths.user_advanced_skills_file()).unwrap())
            .unwrap();
    // The user's own set is untouched and their own class stays.
    assert_eq!(user["ArchMage"]["Base"], mine["ArchMage"]["Base"]);
    assert_eq!(user["My Class"], mine["My Class"]);
    // Shipped classes and the shipped modes ArchMage lacked were added.
    let shipped = shipped();
    for (class, modes) in shipped.as_object().unwrap() {
        for mode in modes.as_object().unwrap().keys() {
            assert!(user[class].get(mode).is_some(), "{class} / {mode} missing");
        }
    }
}

#[test]
fn engine_reads_every_shipped_set() {
    let sets = turu_bot::skills::parse_advanced_skills(turu_bot::DEFAULT_ADVANCED_SKILLS);
    let expected: usize = shipped()
        .as_object()
        .unwrap()
        .values()
        .map(|modes| modes.as_object().unwrap().len())
        .sum();
    assert_eq!(sets.len(), expected);
}

#[test]
fn saving_the_shipped_sets_reads_back_the_same() {
    use turu_bot::skills::{advanced_skills_json, parse_advanced_skills};
    let sets = parse_advanced_skills(turu_bot::DEFAULT_ADVANCED_SKILLS);
    let saved = advanced_skills_json(&sets);
    assert_eq!(parse_advanced_skills(&saved), sets);
    let saved: Value = serde_json::from_str(&saved).unwrap();
    let names = |v: &Value| v.as_object().unwrap().keys().cloned().collect::<Vec<_>>();
    // serde_json sorts its maps; the file order is checked through parse_advanced_skills.
    assert_eq!(names(&saved), names(&shipped()));
}

#[test]
fn saved_json_follows_vibeskua() {
    use turu_bot::skills::{AdvancedSkill, SkillUseMode, advanced_skills_json};
    let set = AdvancedSkill {
        class_name: "My Class".into(),
        mode: "Farm".into(),
        skills: r#"1 | 2 H>50% WW300S | 3 MA>"Focus" 2 TARGET&MA<"Burn" 1.5 | 4 A<"Rage" 3"#.into(),
        skill_timeout: 250,
        skill_use_mode: SkillUseMode::WaitForCooldown,
        reset_on_target: true,
    };
    let saved: Value = serde_json::from_str(&advanced_skills_json(&[set])).unwrap();
    assert_eq!(
        saved,
        json!({ "My Class": { "Farm": {
            "skillUseMode": "WaitForCooldown",
            "skillTimeout": 250,
            "resetComboOnTargetChange": true,
            "skills": [
                { "skillId": 1 },
                { "skillId": 2, "skipOnMatch": true, "rules": [
                    { "type": "Health", "value": 50, "comparison": "greater", "isPercentage": true },
                    { "type": "Wait", "timeout": 300 }
                ] },
                { "skillId": 3, "isMultiAura": true, "multiAuraOperator": "AND", "rules": [
                    { "type": "MultiAura", "auraName": "Focus", "auraTarget": "target", "value": 2, "comparison": "greater" },
                    { "type": "MultiAura", "auraName": "Burn", "auraTarget": "self", "value": 1.5, "comparison": "less" }
                ] },
                { "skillId": 4, "rules": [
                    { "type": "Aura", "auraName": "Rage", "auraTarget": "self", "value": 3, "comparison": "less" }
                ] }
            ]
        } } })
    );
}

#[test]
fn saving_replaces_the_same_class_and_mode_and_reset_restores_the_shipped_sets() {
    use turu_bot::skills::{load_advanced_skills, override_skill_set, save_advanced_skills};
    let paths = paths("save");
    let mut sets = load_advanced_skills(&paths);
    let count = sets.len();
    let mut mine = sets
        .iter()
        .find(|s| s.class_name == "ArchMage")
        .unwrap()
        .clone();
    mine.skills = "1 | 2".into();
    override_skill_set(&mut sets, mine.clone());
    mine.class_name = "My Class".into();
    override_skill_set(&mut sets, mine);
    save_advanced_skills(&paths, &sets).unwrap();
    let read = turu_bot::skills::parse_advanced_skills(
        &std::fs::read_to_string(paths.user_advanced_skills_file()).unwrap(),
    );
    assert_eq!(read, sets);
    assert_eq!(read.len(), count + 1);
    paths.reset_advanced_skills().unwrap();
    let user: Value =
        serde_json::from_str(&std::fs::read_to_string(paths.user_advanced_skills_file()).unwrap())
            .unwrap();
    assert_eq!(user, shipped());
}

#[test]
fn the_editor_keeps_every_shipped_skill() {
    use turu_bot::skill_editor::{SkillItem, parse_sequence, sequence_string};
    use turu_bot::skills::{parse_advanced_skills, parse_skills};
    for set in parse_advanced_skills(turu_bot::DEFAULT_ADVANCED_SKILLS) {
        let edited = sequence_string(&parse_sequence(&set.skills));
        // The editor writes the rules in its own order; all of them must hold either way.
        let rules = |text: &str| {
            let mut commands = parse_skills(text);
            for command in &mut commands {
                command.rules.sort_by_key(|r| format!("{r:?}"));
            }
            commands
        };
        assert_eq!(
            rules(&edited),
            rules(&set.skills),
            "{} ({}): {} became {edited}",
            set.class_name,
            set.mode,
            set.skills
        );
        // And the editor reads its own output back unchanged.
        for item in parse_sequence(&edited) {
            assert_eq!(SkillItem::parse(&item.convert()), Some(item));
        }
    }
}

#[test]
fn the_editor_writes_and_shows_rules_as_vibeskua() {
    use turu_bot::skill_editor::{AuraRow, SkillItem, SkillRules};
    let mut rules = SkillRules {
        use_rule: true,
        health: 50,
        mana_greater: false,
        mana: 30,
        mana_percent: false,
        wait: 200,
        skip: true,
        ..SkillRules::default()
    };
    let item = SkillItem::new(3, &rules);
    assert_eq!(item.convert(), "3 WW200 H>50% M<30# S".replace("# S", "#S"));
    assert_eq!(
        item.display(),
        "3 - [Wait for 200] - [Health > 50%] - [Mana < 30#] - [Skip if not available]"
    );
    rules.skip = false;
    rules.multi_aura = true;
    rules.auras_or = true;
    rules.auras = vec![
        AuraRow {
            name: "Focus".into(),
            stacks: 2.0,
            greater: true,
            target: true,
        },
        AuraRow {
            name: "Burn".into(),
            stacks: 1.5,
            greater: false,
            target: false,
        },
    ];
    let item = SkillItem::new(4, &rules);
    assert_eq!(
        item.convert(),
        r#"4 WW200 H>50% M<30# MA>"Focus" 2 TARGET: MA<"Burn" 1.5"#
    );
    assert_eq!(SkillItem::parse(&item.convert()), Some(item));
    // Without Enable Rule the skill has no rules.
    rules.use_rule = false;
    assert_eq!(SkillItem::new(1, &rules).convert(), "1");
    // A skip after an aura on the target survives a reload.
    let item = SkillItem::parse(r#"2 A>"Focus" 3 TARGETS"#).unwrap();
    assert!(item.rules.skip && item.rules.aura_target);
    assert_eq!(item.rules.aura_name, "Focus");
}
