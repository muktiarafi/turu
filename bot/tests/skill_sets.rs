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
