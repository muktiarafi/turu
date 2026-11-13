//! turu.swf's batched reads (docs/HOST_SWF_PROPOSAL.md M4): the skill timer's one `snapshot` a
//! tick, `getMany` and `selectArrayObjectsWhere`, against a fake game.

use parking_lot::Mutex;
use serde_json::{Value, json};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use turu_bot::{Bot, FlashArg, FlashHost, HostError};

/// A logged-in, loaded player fighting a monster. Records the engine's calls by name and first
/// argument; a batched read is answered from the single reads, which aren't recorded.
#[derive(Default)]
struct FakeGame {
    calls: Mutex<Vec<String>>,
    /// The target is dead.
    target_dead: AtomicBool,
}

impl FlashHost for FakeGame {
    fn call(&self, name: &str, args: Vec<FlashArg>) -> Result<String, HostError> {
        let first = match args.first() {
            Some(FlashArg::String(s)) => s.clone(),
            Some(FlashArg::Bool(b)) => b.to_string(),
            _ => String::new(),
        };
        self.calls
            .lock()
            .push(format!("{name} {first}").trim().to_owned());
        if let Some(answer) = turu_bot::testing::batched(name, &args, |n, a| self.single(n, a)) {
            return Ok(answer);
        }
        Ok(self.single(name, args))
    }
}

impl FakeGame {
    fn single(&self, name: &str, args: Vec<FlashArg>) -> String {
        let path = match args.first() {
            Some(FlashArg::String(s)) => s.as_str(),
            _ => "",
        };
        match (name, path) {
            ("isLoggedIn", _) | ("canUseSkill", _) => "true".into(),
            ("isNull", _) => "false".into(),
            ("getTargetMonster", _) => {
                let hp = if self.target_dead.load(Ordering::SeqCst) {
                    0
                } else {
                    10
                };
                json!({ "MonID": 1, "MonMapID": 9, "strMonName": "Frogzard", "intHP": hp, "intState": 1 })
                    .to_string()
            }
            ("getGameObject", "world.myAvatar.dataLeaf.intState") => "2".into(),
            ("getGameObject", "world.myAvatar.items.length") => "3".into(),
            ("getGameObject", "world.mapLoadInProgress") => "false".into(),
            ("getGameObject", "world.myAvatar.objData.iRank") => "10".into(),
            ("getGameObject", "world.myAvatar.objData.intGold") => "1200".into(),
            ("getGameObject", "world.strFrame") => "\"Enter\"".into(),
            ("getGameObject", "world.myAvatar.items") => json!([
                { "ItemID": 1, "sName": "Gold Voucher 100k", "iQty": 2, "sType": "Item" },
                { "ItemID": 2, "sName": "Rogue", "iQty": 1, "sType": "Class", "bEquip": 1 },
                { "ItemID": 3, "sName": "Warrior", "iQty": 1, "sType": "Class", "bEquip": 0 }
            ])
            .to_string(),
            ("callGameFunction0", "world.myAvatar.pMC.artLoaded") => "true".into(),
            _ => "null".into(),
        }
    }

    fn count(&self, call: &str) -> usize {
        self.calls.lock().iter().filter(|c| *c == call).count()
    }
}

fn bot() -> (Arc<Bot>, Arc<FakeGame>) {
    // SAFETY: every test of this crate sets the same value.
    unsafe { std::env::set_var("TURU_DATA_DIR", std::env::temp_dir().join("turu-test-data")) };
    let game = Arc::new(FakeGame::default());
    let bot = Bot::new();
    bot.set_host(Some(game.clone()));
    (bot, game)
}

fn call(bot: &Arc<Bot>, method: &str, args: Value) -> Value {
    let args = args.as_array().cloned().unwrap_or_default();
    turu_bot::api::dispatch(bot, method, &args).unwrap_or_else(|e| panic!("{method}: {e}"))
}

#[test]
fn the_skill_timer_reads_one_snapshot_a_tick() {
    let (bot, game) = bot();
    call(
        &bot,
        "skills.startSkills",
        json!(["1 | 2", 10, "UseIfAvailable"]),
    );
    std::thread::sleep(Duration::from_millis(1000));
    call(&bot, "skills.stop", json!([]));
    let snapshots = game.count("snapshot true");
    assert!(
        (5..=12).contains(&snapshots),
        "{snapshots} snapshots in a second"
    );
    // It fought: the snapshot said alive, loaded, logged in and a living target.
    assert!(game.count("useSkill") >= 5, "{:?}", game.calls.lock());
    // None of the reads the snapshot replaces.
    let calls = game.calls.lock();
    for single in [
        "isLoggedIn",
        "isNull world.myAvatar",
        "getTargetMonster",
        "getGameObject world.myAvatar.dataLeaf.intState",
        "getGameObject world.myAvatar.items.length",
        "getGameObject world.mapLoadInProgress",
        "callGameFunction0 world.myAvatar.pMC.artLoaded",
        "getGameObject world.myAvatar.objData.iRank",
    ] {
        assert!(!calls.iter().any(|c| c == single), "{single} in {calls:?}");
    }
}

#[test]
fn the_skill_timer_without_a_living_target_doesnt_attack() {
    let (bot, game) = bot();
    game.target_dead.store(true, Ordering::SeqCst);
    call(
        &bot,
        "skills.startSkills",
        json!(["1 | 2", 10, "UseIfAvailable"]),
    );
    std::thread::sleep(Duration::from_millis(500));
    call(&bot, "skills.stop", json!([]));
    assert!(game.count("snapshot true") > 0);
    assert_eq!(game.count("useSkill"), 0, "{:?}", game.calls.lock());
}

#[test]
fn attacking_without_a_target_leaves_the_target_alone() {
    let (bot, game) = bot();
    bot.options.write().attack_without_target = true;
    call(
        &bot,
        "skills.startSkills",
        json!(["1 | 2", 10, "UseIfAvailable"]),
    );
    std::thread::sleep(Duration::from_millis(500));
    call(&bot, "skills.stop", json!([]));
    assert!(game.count("snapshot false") > 0, "{:?}", game.calls.lock());
    assert_eq!(game.count("snapshot true"), 0);
    assert!(game.count("useSkill") > 0);
}

#[test]
fn scripts_read_several_values_with_get_many() {
    let (bot, game) = bot();
    let values = call(
        &bot,
        "flash.getMany",
        json!([{ "gold": "world.myAvatar.objData.intGold", "cell": "world.strFrame", "nothing": "world.noSuch" }]),
    );
    assert_eq!(
        values,
        json!({ "gold": 1200, "cell": "Enter", "nothing": null })
    );
    assert_eq!(*game.calls.lock(), ["getMany"]);
    let error =
        turu_bot::api::dispatch(&bot, "flash.getMany", &[json!({ "gold": 1 })]).unwrap_err();
    assert!(error.to_string().contains("gold"), "{error}");
}

#[test]
fn item_checks_send_a_filter_and_check_its_answer() {
    let (bot, game) = bot();
    assert_eq!(
        call(
            &bot,
            "inventory.getQuantity",
            json!(["  gold voucher 100K "])
        ),
        json!(2)
    );
    assert_eq!(
        call(&bot, "inventory.getItem", json!([3]))["name"],
        json!("Warrior")
    );
    // The equipped class: `bEquip` true and `sType` Class (Warrior is a class, not worn).
    assert_eq!(call(&bot, "player.currentClass", json!([])), json!("Rogue"));
    // (`currentClass` first asks whether you're logged in.)
    let calls = game.calls.lock();
    assert!(
        calls
            .iter()
            .filter(|c| !c.starts_with("snapshot"))
            .all(|c| c == "selectArrayObjectsWhere world.myAvatar.items"),
        "{calls:?}"
    );
}
