//! Cosmetics (`turu_bot::cosmetics`, VibeSkua's Cosmetics plugin) against a fake game whose
//! objects are one JSON tree: showing an item, copying another player's look, and going back
//! to the real gear.

use parking_lot::Mutex;
use serde_json::{Value, json};
use std::sync::Arc;
use turu_bot::cosmetics::avatar::{self, ColorChange};
use turu_bot::cosmetics::{CosmeticItem, Library, charpage};
use turu_bot::{Bot, FlashArg, FlashHost, HostError};

struct FakeGame {
    /// The game object, rooted like `getGameObject` paths.
    root: Mutex<Value>,
    /// Another player's `objData`, for `getAvatar`.
    other: Value,
    /// `path(args)` of every game function called, and the client packets.
    calls: Mutex<Vec<String>>,
}

fn value_of(arg: &FlashArg) -> Value {
    match arg {
        FlashArg::Null => Value::Null,
        FlashArg::Bool(b) => json!(b),
        FlashArg::Number(n) if n.fract() == 0.0 => json!(*n as i64),
        FlashArg::Number(n) => json!(n),
        FlashArg::String(s) => json!(s),
        FlashArg::List(items) => items.iter().map(value_of).collect(),
        FlashArg::Object(map) => map.iter().map(|(k, v)| (k.clone(), value_of(v))).collect(),
    }
}

fn text(arg: Option<&FlashArg>) -> String {
    match arg {
        Some(FlashArg::String(s)) => s.clone(),
        _ => String::new(),
    }
}

impl FakeGame {
    fn new() -> Self {
        Self {
            root: Mutex::new(json!({
                "world": {
                    "areaUsers": ["me", "ally"],
                    "myAvatar": {
                        "uid": 11,
                        "target": { "dataLeaf": { "entID": 22, "uoName": "Ally" } },
                        "dataLeaf": { "strGender": "M", "intColorBase": 1, "intColorTrim": 2,
                            "intColorAccessory": 3, "intColorEye": 4, "intColorSkin": 5, "intColorHair": 6 },
                        "objData": { "strGender": "M", "intColorBase": 1, "intColorTrim": 2,
                            "intColorAccessory": 3, "intColorEye": 4, "intColorSkin": 5, "intColorHair": 6,
                            "eqp": {
                                "ar": { "sFile": "classes/M/Warrior.swf", "sLink": "mWarrior", "ItemID": 7 },
                                "Weapon": { "sFile": "items/swords/real.swf", "sLink": "", "ItemID": 8 },
                            } },
                        "items": [
                            { "ItemID": 7, "sName": "Warrior", "sES": "ar", "sType": "Class", "bEquip": true,
                              "sFile": "classes/M/Warrior.swf", "sLink": "mWarrior" },
                            { "ItemID": 8, "sName": "Real Sword", "sES": "Weapon", "sType": "Sword", "bEquip": true,
                              "sFile": "items/swords/real.swf", "sLink": "" },
                        ],
                    },
                },
            })),
            other: json!({
                "strGender": "F",
                "intColorBase": "255", "intColorTrim": 0, "intColorSkin": 16777215,
                "eqp": {
                    "co": { "sFile": "items/armors/F/Fancy.swf", "sLink": "fFancy", "sType": "Armor", "ItemID": 100 },
                    "ar": { "sFile": "classes/F/Mage.swf", "sLink": "fMage", "sType": "Class" },
                    "he": { "sFile": "items/helms/Hat.swf", "sLink": "Hat", "sType": "Helm", "ItemID": "101" },
                    "ba": { "sFile": "", "sLink": "" },
                },
            }),
            calls: Mutex::default(),
        }
    }

    fn get(&self, path: &str) -> Value {
        let root = self.root.lock();
        path.split('.')
            .try_fold(&*root, |v, part| v.get(part))
            .cloned()
            .unwrap_or(Value::Null)
    }

    fn set(&self, path: &str, key: Option<&str>, value: Value) {
        let mut root = self.root.lock();
        let mut parts: Vec<&str> = path.split('.').collect();
        let last = match key {
            Some(key) => key,
            None => parts.pop().unwrap(),
        };
        let mut obj = &mut *root;
        for part in parts {
            obj = &mut obj[part];
        }
        obj[last] = value;
    }

    fn calls(&self) -> Vec<String> {
        self.calls.lock().clone()
    }
}

impl FlashHost for FakeGame {
    fn call(&self, name: &str, args: Vec<FlashArg>) -> Result<String, HostError> {
        let first = text(args.first());
        Ok(match name {
            "isLoggedIn" => "true".into(),
            "getGameObjectS" if first == "loginInfo.strUsername" => "\"Me\"".into(),
            "getGameObject" => self.get(&first).to_string(),
            "getGameObjectKey" => self
                .get(&format!("{first}.{}", text(args.get(1))))
                .to_string(),
            "setGameObject" => {
                self.set(&first, None, value_of(&args[1]));
                String::new()
            }
            "setGameObjectKey" => {
                self.set(&first, Some(&text(args.get(1))), value_of(&args[2]));
                String::new()
            }
            "gender" => self.get("world.myAvatar.objData.strGender").to_string(),
            "getAvatar" => self.other.to_string(),
            "callGameFunction" | "callGameFunction0" | "sendClientPacket" => {
                let rest: Vec<String> = args[1..].iter().map(|a| value_of(a).to_string()).collect();
                self.calls
                    .lock()
                    .push(format!("{first}({})", rest.join(",")));
                "null".into()
            }
            _ => "null".into(),
        })
    }
}

fn start() -> (Arc<Bot>, Arc<FakeGame>, Library, tempfile::TempDir) {
    // SAFETY: every test of this crate sets the same value.
    unsafe { std::env::set_var("TURU_DATA_DIR", std::env::temp_dir().join("turu-test-data")) };
    let game = Arc::new(FakeGame::new());
    let bot = Bot::new();
    bot.set_host(Some(game.clone()));
    let dir = tempfile::tempdir().unwrap();
    let library = Library::new(dir.path().to_owned());
    (bot, game, library, dir)
}

#[test]
fn equip_shows_the_item_and_remembers_the_real_state() {
    let (bot, game, _library, _dir) = start();
    let helm = CosmeticItem {
        id: 5,
        name: "Hat".into(),
        path: "items/helms/Hat.swf".into(),
        link: "Hat".into(),
        item_group: "he".into(),
        category: "Helm".into(),
        meta: String::new(),
    };
    avatar::equip(&bot, &helm).unwrap();
    assert_eq!(
        game.get("world.myAvatar.objData.eqp.he"),
        json!({ "sFile": "items/helms/Hat.swf", "sLink": "Hat", "sType": "Helm", "sMeta": "" })
    );
    assert!(
        game.calls().contains(
            &r#"world.myAvatar.loadMovieAtES("he","items/helms/Hat.swf","Hat")"#.to_owned()
        ),
        "{:?}",
        game.calls()
    );
    assert_eq!(bot.cosmetics.current().helm.map(|i| i.id), Some(5));
    assert_eq!(bot.cosmetics.original_gender(), Some('M'));
    assert_eq!(
        bot.cosmetics.original_colors().map(|c| c.to_array()),
        Some([1, 2, 3, 4, 5, 6])
    );

    // A class shows in the armor slot, as Armor.
    let class = CosmeticItem {
        item_group: "ar".into(),
        category: "Class".into(),
        ..helm.clone()
    };
    avatar::equip(&bot, &class).unwrap();
    assert_eq!(
        game.get("world.myAvatar.objData.eqp.co.sType"),
        json!("Armor")
    );

    avatar::set_colors(
        &bot,
        ColorChange([Some(9), None, None, None, None, Some(10)]),
        false,
    )
    .unwrap();
    assert_eq!(game.get("world.myAvatar.objData.intColorBase"), json!(9));
    assert_eq!(game.get("world.myAvatar.objData.intColorTrim"), json!(2));
    assert_eq!(game.get("world.myAvatar.objData.intColorHair"), json!(10));
}

#[test]
fn copying_a_look_takes_gear_gender_and_colors_and_empties_the_rest() {
    let (bot, game, library, _dir) = start();
    let message = avatar::copy_target_look(&bot, &library).unwrap();
    assert_eq!(message, "Copied look from Ally!");

    // The armor (co wins over the class), the helm; no cape (no art), no weapon.
    let current = bot.cosmetics.current();
    assert_eq!(
        current.armor.as_ref().map(|i| i.name.as_str()),
        Some("Fancy")
    );
    assert_eq!(current.helm.as_ref().map(|i| i.id), Some(101));
    assert!(current.cape.is_none() && current.weapon.is_none());
    assert_eq!(
        game.get("world.myAvatar.objData.eqp.co.sFile"),
        json!("items/armors/F/Fancy.swf")
    );
    assert_eq!(game.get("world.myAvatar.objData.eqp.Weapon"), Value::Null);

    // Female, through the game's own genderSwap answer.
    assert_eq!(game.get("world.myAvatar.objData.strGender"), json!("F"));
    let calls = game.calls();
    let swap = calls
        .iter()
        .find(|c| c.contains("genderSwap"))
        .expect("a genderSwap packet");
    assert!(
        swap.contains(r#""gender":"F""#) && swap.contains(r#""uid":11"#),
        "{swap}"
    );

    // The colors it has (a string one too); the others are left alone.
    assert_eq!(game.get("world.myAvatar.objData.intColorBase"), json!(255));
    assert_eq!(game.get("world.myAvatar.objData.intColorTrim"), json!(0));
    assert_eq!(
        game.get("world.myAvatar.objData.intColorSkin"),
        json!(16777215)
    );
    assert_eq!(game.get("world.myAvatar.objData.intColorEye"), json!(4));

    // Saving it keeps the copied items and fills the rest from the real gear.
    let saved = avatar::capture_current_look(&bot, &library, "Outfit #1").unwrap();
    assert_eq!(saved.gender, Some('F'));
    assert_eq!(saved.armor.as_ref().map(|i| i.name.as_str()), Some("Fancy"));
    assert_eq!(
        saved.weapon.as_ref().map(|i| i.name.as_str()),
        Some("Real Sword")
    );

    // Back to the real gear: male art for the class, the real weapon, the real colors.
    avatar::revert_to_real_gear(&bot).unwrap();
    assert_eq!(game.get("world.myAvatar.objData.strGender"), json!("M"));
    assert_eq!(
        game.get("world.myAvatar.objData.eqp.co"),
        json!({ "sFile": "classes/M/Warrior.swf", "sLink": "mWarrior", "sType": "Class", "sMeta": "" })
    );
    assert_eq!(
        game.get("world.myAvatar.objData.eqp.Weapon.sFile"),
        json!("items/swords/real.swf")
    );
    assert_eq!(game.get("world.myAvatar.objData.eqp.he"), Value::Null);
    assert_eq!(game.get("world.myAvatar.objData.intColorBase"), json!(1));
    assert_eq!(game.get("world.myAvatar.objData.intColorSkin"), json!(5));
    assert!(!bot.cosmetics.current().has_any_item());
    assert_eq!(avatar::room_player_names(&bot).unwrap(), ["ally"]);
}

#[test]
fn a_character_page_look_is_applied_like_a_copied_one() {
    let (bot, game, _library, _dir) = start();
    let page = charpage::parse(
        r#"<param name="FlashVars" value="&amp;strName=Lae&amp;strGender=F&amp;intColorBase=255&amp;strClassFile=Mage.swf&amp;strClassLink=Mage&amp;ArmorID=7&amp;CustHelmID=89450&amp;strCustHelmFile=items/helms/APIllusionSkinFace.swf&amp;strCustHelmLink=APIllusionSkinFace&amp;strCustHelmName=Unmasked&amp;strMiscFile=none" />"#,
    )
    .unwrap();
    assert_eq!(
        avatar::copy_char_page(&bot, &page).unwrap(),
        "Copied look from Lae!"
    );

    assert_eq!(game.get("world.myAvatar.objData.strGender"), json!("F"));
    assert_eq!(game.get("world.myAvatar.objData.intColorBase"), json!(255));
    assert_eq!(game.get("world.myAvatar.objData.intColorTrim"), json!(2));
    // The class shows in the armor slot; the helm from the Cust fields; the rest emptied.
    assert_eq!(
        game.get("world.myAvatar.objData.eqp.co.sFile"),
        json!("Mage.swf")
    );
    assert_eq!(
        game.get("world.myAvatar.objData.eqp.he.sFile"),
        json!("items/helms/APIllusionSkinFace.swf")
    );
    assert_eq!(game.get("world.myAvatar.objData.eqp.Weapon"), Value::Null);
    let current = bot.cosmetics.current();
    assert_eq!(current.helm.map(|i| i.name), Some("Unmasked".to_owned()));
    assert!(current.misc.is_none());
}
