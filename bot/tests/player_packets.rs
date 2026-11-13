//! Packets from the player (docs/HOST_SWF_PROPOSAL.md, R5): with a host whose player reports the
//! game's socket, the bot acts on those and not on the host SWF's own `packet` and `pext`.

use serde_json::json;
use std::sync::Arc;
use std::time::{Duration, Instant};
use turu_bot::events::BotEvent;
use turu_bot::player_packets::Direction;
use turu_bot::{Bot, FlashArg, FlashHost, HostError};

/// A player that reports its sockets, answering every call with nothing.
struct PlayerWithSockets;

impl FlashHost for PlayerWithSockets {
    fn call(&self, name: &str, args: Vec<FlashArg>) -> Result<String, HostError> {
        if let Some(answer) =
            turu_bot::testing::batched(name, &args, |n, a| self.call(n, a).unwrap_or_default())
        {
            return Ok(answer);
        }
        Ok("null".into())
    }

    fn player_packets(&self) -> bool {
        true
    }
}

fn bot() -> Arc<Bot> {
    // SAFETY: tests in this file set the same value.
    unsafe { std::env::set_var("TURU_DATA_DIR", std::env::temp_dir().join("turu-test-data")) };
    let bot = Bot::new();
    bot.set_host(Some(Arc::new(PlayerWithSockets)));
    bot
}

/// The map names of the `MapChanged` events `bot` emits while `f` runs.
fn maps_changed(bot: &Arc<Bot>, f: impl FnOnce()) -> Vec<String> {
    let seen = Arc::new(parking_lot::Mutex::new(Vec::new()));
    let into = seen.clone();
    let id = bot.events.subscribe(move |event| {
        if let BotEvent::MapChanged { map } = event {
            into.lock().push(map.clone());
        }
    });
    f();
    let start = Instant::now();
    while seen.lock().is_empty() && start.elapsed() < Duration::from_millis(200) {
        std::thread::sleep(Duration::from_millis(5));
    }
    bot.events.unsubscribe(id);
    seen.lock().clone()
}

#[test]
fn received_json_messages_reach_the_handlers_and_the_swfs_dont() {
    let bot = bot();
    let message = br#"{"t":"xt","b":{"r":-1,"o":{"cmd":"moveToArea","strMapName":"yulgar","strMapFileName":"yulgar.swf"}}}"#;

    let from_swf = maps_changed(&bot, || {
        let pext = json!({"params": {"type": "json", "dataObj": {"cmd": "moveToArea", "strMapName": "battleon"}}});
        bot.on_flash_call("pext", vec![json!([pext.to_string()])]);
    });
    assert!(
        from_swf.is_empty(),
        "the SWF's pext is dropped: {from_swf:?}"
    );

    let from_player = maps_changed(&bot, || {
        // Split across two reads, as a socket may deliver it.
        bot.on_socket(7, Direction::Received, &message[..20]);
        bot.on_socket(7, Direction::Received, &message[20..]);
        bot.on_socket(7, Direction::Received, b"\0");
    });
    assert_eq!(from_player, ["yulgar"]);
    assert_eq!(bot.state.lock().map_name.as_deref(), Some("yulgar"));
}

#[test]
fn compressed_messages_are_unpacked_like_the_clients() {
    // The server sends its longer messages as `Z` and zlib in Base64: the Staff of Inversion's
    // drops come so (Escherion's, shorter, don't), and they were lost before the bot saw them.
    let bot = bot();
    let drop = r#"{"t":"xt","b":{"r":-1,"o":{"cmd":"dropItem","items":{"1174":{"ItemID":1174,"sName":"1st Lord of Chaos Staff","sDesc":"You now posses the living staff of the very first Lord of Chaos!","iQty":1,"iStk":1,"bTemp":false,"sType":"Staff","sES":"Weapon","bQuest":1}}}}}"#;
    let mut packed = turu_bot::capture::pack(drop);
    packed.push(0);
    bot.on_socket(7, Direction::Received, &packed);
    assert!(
        bot.state
            .lock()
            .drop_exists(turu_bot::api::NameOrId::Name("1st Lord Of Chaos Staff")),
        "the compressed drop reached the drop stack"
    );

    let map = r#"{"t":"xt","b":{"r":-1,"o":{"cmd":"moveToArea","strMapName":"escherion","strMapFileName":"escherion.swf"}}}"#;
    let mut packed = turu_bot::capture::pack(map);
    packed.push(0);
    let changed = maps_changed(&bot, || bot.on_socket(7, Direction::Received, &packed));
    assert_eq!(changed, ["escherion"]);
}

#[test]
fn sent_game_packets_are_logged_once() {
    let bot = bot();
    bot.logs
        .packet_logging
        .store(true, std::sync::atomic::Ordering::Relaxed);
    bot.on_flash_call("packet", vec![json!(["%xt%zm%moveToCell%1%Enter%Spawn%"])]);
    bot.on_socket(
        7,
        Direction::Sent,
        b"%xt%zm%moveToCell%1%Enter%Spawn%\0<msg t='sys'><body action='verChk'/></msg>\0",
    );
    let packets = bot.logs.packets.text();
    assert_eq!(packets.matches("moveToCell").count(), 1, "{packets}");
    assert!(!packets.contains("verChk"), "{packets}");
}
