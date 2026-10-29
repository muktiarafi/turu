//! Skill-string and AdvancedSkills.json parsing (VibeSkua `AdvancedSkillProvider`,
//! `AdvancedSkillContainer`).

use turu_bot::skills::{
    AuraCheck, SkillProvider, SkillRule, SkillUseMode, UseRule, find_skill_set,
    parse_advanced_skills, parse_skills, parse_use_rule,
};

fn rule(rule: SkillRule, skip: bool) -> UseRule {
    UseRule { rule, skip }
}

#[test]
fn parses_a_plain_sequence() {
    let commands = parse_skills("1 | 2 | 3 | 4");
    assert_eq!(
        commands.iter().map(|c| c.skill).collect::<Vec<_>>(),
        [1, 2, 3, 4]
    );
    assert!(
        commands
            .iter()
            .all(|c| c.rules == [rule(SkillRule::None, false)])
    );
}

#[test]
fn skips_entries_without_a_digit() {
    let commands = parse_skills(" | x | 3 |  | 5");
    assert_eq!(commands.iter().map(|c| c.skill).collect::<Vec<_>>(), [3, 5]);
}

#[test]
fn parses_health_mana_and_party_rules() {
    let commands = parse_skills("4 H<50 | 2 M>30# | 3 PH<40%");
    assert_eq!(
        commands[0].rules,
        [rule(
            SkillRule::Health {
                greater: false,
                value: 50,
                percent: true
            },
            false
        )]
    );
    assert_eq!(
        commands[1].rules,
        [rule(
            SkillRule::Mana {
                greater: true,
                value: 30,
                percent: false
            },
            false
        )]
    );
    assert_eq!(
        commands[2].rules,
        [rule(
            SkillRule::PartyHealth {
                greater: false,
                value: 40,
                percent: true
            },
            false
        )]
    );
}

#[test]
fn chains_rules_and_skips() {
    let commands = parse_skills("4 H>70% M<70% A<\"Corporeal Ascenion\" 0S");
    let rules = &commands[0].rules;
    assert_eq!(rules.len(), 3);
    assert!(rules.iter().all(|r| r.skip));
    assert_eq!(
        rules[2].rule,
        SkillRule::Aura(AuraCheck {
            name: "corporeal ascenion".into(),
            stacks: 0.0,
            greater: false,
            target: false
        })
    );
}

#[test]
fn parses_unquoted_auras_with_targets() {
    let rules = parse_use_rule(" a>focus 3 target");
    assert_eq!(
        rules,
        [rule(
            SkillRule::Aura(AuraCheck {
                name: "focus".into(),
                stacks: 3.0,
                greater: true,
                target: true
            }),
            false
        )]
    );
}

#[test]
fn parses_multi_auras() {
    let commands = parse_skills("1 MA>\"Alpha\" 2 TARGET: MA<\"Beta\" 1.5");
    let [
        UseRule {
            rule: SkillRule::MultiAura { checks, or },
            ..
        },
    ] = commands[0].rules.as_slice()
    else {
        panic!("{:?}", commands[0].rules);
    };
    assert!(*or);
    assert_eq!(checks.len(), 2);
    assert_eq!(
        checks[0],
        AuraCheck {
            name: "alpha".into(),
            stacks: 2.0,
            greater: true,
            target: true
        }
    );
    assert_eq!(
        checks[1],
        AuraCheck {
            name: "beta".into(),
            stacks: 1.5,
            greater: false,
            target: false
        }
    );
}

#[test]
fn parses_waits() {
    assert_eq!(
        parse_use_rule(" ww500"),
        [rule(SkillRule::Wait { ms: 500 }, false)]
    );
}

#[test]
fn provider_cycles_through_the_sequence() {
    let mut provider = SkillProvider::new("1 | 2 | 3");
    let slots: Vec<i64> = (0..5).map(|_| provider.next_skill().unwrap().1).collect();
    assert_eq!(slots, [1, 2, 3, 1, 2]);
    provider.reset();
    assert_eq!(provider.next_skill(), Some((0, 1)));
    assert_eq!(SkillProvider::new("").next_skill(), None);
}

const SETS: &str = r#"{
  "ArchMage": {
    "Base": {
      "skillUseMode": "WaitForCooldown",
      "skillTimeout": 100,
      "skills": [
        { "skillId": 1, "rules": [ { "type": "Aura", "value": 0, "comparison": "less", "auraName": "incinerate", "auraTarget": "target" } ] },
        { "skillId": 4, "rules": [
            { "type": "Health", "value": 70, "comparison": "greater", "isPercentage": true },
            { "type": "Mana", "value": 70, "comparison": "less", "isPercentage": false },
            { "type": "Wait", "timeout": 300 } ] },
        { "skillId": 3 }
      ]
    },
    "Farm": { "skillUseMode": "UseIfAvailable", "skillTimeout": 50, "resetComboOnTargetChange": true,
      "skills": [ { "skillId": 2, "isMultiAura": true, "multiAuraOperator": "OR", "skipOnMatch": true, "rules": [
          { "type": "MultiAura", "value": 1, "comparison": "greater", "auraName": "A" },
          { "type": "MultiAura", "value": 2, "comparison": "less", "auraName": "B", "auraTarget": "target" } ] } ] }
  },
  "Void Highlord": { "Solo": { "skills": [ { "skillId": 5 } ] } }
}"#;

#[test]
fn converts_advanced_skills_json_to_skill_strings() {
    let sets = parse_advanced_skills(SETS);
    assert_eq!(sets.len(), 3);
    assert_eq!(sets[0].class_name, "ArchMage");
    assert_eq!(sets[0].mode, "Base");
    assert_eq!(
        sets[0].skills,
        "1 A<\"incinerate\" 0 TARGET | 4 H>70% M<70# WW300 | 3"
    );
    assert_eq!(sets[0].skill_use_mode, SkillUseMode::WaitForCooldown);
    assert_eq!(sets[1].skills, "2 MA>\"A\" 1: MA<\"B\" 2 TARGETS");
    assert!(sets[1].reset_on_target);
    assert_eq!(sets[1].skill_use_mode, SkillUseMode::UseIfAvailable);
    // Defaults of a mode without settings.
    assert_eq!(sets[2].skill_timeout, 100);
    assert_eq!(sets[2].skill_use_mode, SkillUseMode::UseIfAvailable);
    // The generated strings parse back.
    let commands = parse_skills(&sets[1].skills);
    assert!(commands[0].rules.iter().all(|r| r.skip));
}

#[test]
fn finds_a_class_set_by_mode_with_fallback() {
    let sets = parse_advanced_skills(SETS);
    assert_eq!(
        find_skill_set(&sets, "archmage", "farm").unwrap().mode,
        "Farm"
    );
    assert_eq!(
        find_skill_set(&sets, "ArchMage", "Ultra").unwrap().mode,
        "Base"
    );
    assert!(find_skill_set(&sets, "Nope", "Base").is_none());
}

#[test]
fn shipped_skill_sets_parse() {
    let sets = parse_advanced_skills(turu_bot::DEFAULT_ADVANCED_SKILLS);
    assert!(sets.len() > 200, "{}", sets.len());
    // (The shipped file has one empty entry.)
    for set in sets.iter().filter(|s| !s.skills.is_empty()) {
        assert!(!parse_skills(&set.skills).is_empty(), "{set:?}");
    }
    assert!(parse_advanced_skills("not json").is_empty());
}
