//! Diagnostics > Spammer against a fake game, and Diagnostics > Interceptor's capture.

use parking_lot::Mutex;
use std::sync::Arc;
use std::time::{Duration, Instant};
use turu_bot::capture::{self, CaptureProxy, InterceptedPacket, Interceptor, MessageInfo, Stream};
use turu_bot::spammer::{self, Spammer};
use turu_bot::{Bot, FlashArg, FlashHost, HostError};

/// Records what was sent: `(call, packet, type)`, `call` being `sfc.sendString`,
/// `sfc.sendJson` or `sendClientPacket`, with the time it came.
#[derive(Default)]
struct FakeGame {
    sent: Mutex<Vec<(String, String, String, Instant)>>,
}

impl FlashHost for FakeGame {
    fn call(&self, name: &str, args: Vec<FlashArg>) -> Result<String, HostError> {
        let arg = |i: usize| match args.get(i) {
            Some(FlashArg::String(s)) => s.clone(),
            _ => String::new(),
        };
        let now = Instant::now();
        match name {
            "callGameFunction" => self.sent.lock().push((arg(0), arg(1), String::new(), now)),
            "sendClientPacket" => self.sent.lock().push((name.into(), arg(0), arg(1), now)),
            _ => {}
        }
        Ok("null".into())
    }
}

fn setup() -> (Arc<Bot>, Arc<FakeGame>) {
    // SAFETY: every test in this file sets the same value.
    unsafe {
        std::env::set_var(
            "TURU_DATA_DIR",
            std::env::temp_dir().join("turu-test-spammer-interceptor"),
        )
    };
    let bot = Bot::new();
    let game = Arc::new(FakeGame::default());
    bot.set_host(Some(game.clone()));
    (bot, game)
}

fn wait_until(what: &str, mut done: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !done() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn spammer_sends_to_the_server_or_the_client() {
    let (bot, game) = setup();
    let spammer = Spammer::default();
    spammer.send(&bot, "%xt%zm%afk%1%true%").unwrap();
    spammer.send(&bot, "{\"t\":\"xt\"}").unwrap();
    spammer.set_to_client(true);
    spammer.send(&bot, "%xt%server%-1%Hi%").unwrap();
    spammer.send(&bot, "{\"t\":\"xt\"}").unwrap();
    let sent: Vec<_> = game
        .sent
        .lock()
        .iter()
        .map(|(call, packet, kind, _)| (call.clone(), packet.clone(), kind.clone()))
        .collect();
    let s = |a: &str, b: &str, c: &str| (a.to_owned(), b.to_owned(), c.to_owned());
    assert_eq!(
        sent,
        vec![
            s("sfc.sendString", "%xt%zm%afk%1%true%", ""),
            s("sfc.sendJson", "{\"t\":\"xt\"}", ""),
            s("sendClientPacket", "%xt%server%-1%Hi%", "str"),
            s("sendClientPacket", "{\"t\":\"xt\"}", "json"),
        ]
    );
}

#[test]
fn spammer_loops_the_queue_every_delay_until_stopped() {
    let (bot, game) = setup();
    let spammer = Arc::new(Spammer::default());
    assert!(spammer.add("%xt%zm%a%"));
    assert!(spammer.add("%xt%zm%b%"));
    assert!(!spammer.add("   "));
    spammer.set_delay_ms(60);
    spammer.start(&bot);
    assert!(spammer.is_running());
    wait_until("five packets", || game.sent.lock().len() >= 5);
    spammer.stop();
    assert!(!spammer.is_running());
    assert_eq!(spammer.progress(), None);
    std::thread::sleep(Duration::from_millis(150));
    let sent = game.sent.lock().clone();
    // Stop ends the loop: at most the packet being sent when it came.
    assert!(sent.len() <= 6, "{} sent after Stop", sent.len());
    let packets: Vec<&str> = sent.iter().map(|(_, p, _, _)| p.as_str()).collect();
    assert_eq!(
        &packets[..4],
        ["%xt%zm%a%", "%xt%zm%b%", "%xt%zm%a%", "%xt%zm%b%"]
    );
    for pair in sent.windows(2) {
        let gap = pair[1].3 - pair[0].3;
        assert!(
            gap >= Duration::from_millis(55),
            "only {gap:?} between packets"
        );
    }
}

#[test]
fn spammer_stops_when_the_queue_is_emptied_and_needs_packets_to_start() {
    let (bot, game) = setup();
    let spammer = Arc::new(Spammer::default());
    spammer.start(&bot);
    assert!(!spammer.is_running(), "nothing to send");
    spammer.add("%xt%zm%a%");
    spammer.set_delay_ms(20);
    spammer.start(&bot);
    wait_until("a packet", || !game.sent.lock().is_empty());
    let (row, status) = spammer.progress().expect("running");
    assert_eq!((row, status.as_str()), (0, "Sending 1 of 1"));
    spammer.packets.lock().clear();
    wait_until("the loop to end", || !spammer.is_running());
}

#[test]
fn spammer_stops_when_the_game_is_gone() {
    let (bot, _game) = setup();
    let spammer = Arc::new(Spammer::default());
    spammer.add("%xt%zm%a%");
    bot.set_host(None);
    spammer.start(&bot);
    wait_until("the loop to end", || !spammer.is_running());
    assert!(
        bot.logs
            .debug
            .snapshot()
            .iter()
            .any(|l| l.text.starts_with("Spammer stopped")),
        "the failure is logged"
    );
}

#[test]
fn spammer_queue_files_are_vibeskua_lines() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("queue.txt");
    let spammer = Spammer::default();
    spammer.add("%xt%zm%a%1%");
    spammer.add("{\"t\":\"xt\",\"b\":{}}");
    spammer.add("%xt%zm%a%1%");
    spammer.save(&path).unwrap();
    let newline = if cfg!(windows) { "\r\n" } else { "\n" };
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        format!("%xt%zm%a%1%{newline}{{\"t\":\"xt\",\"b\":{{}}}}{newline}%xt%zm%a%1%{newline}")
    );
    let loaded = Spammer::default();
    loaded.add("replaced");
    loaded.load(&path).unwrap();
    assert_eq!(*loaded.packets.lock(), *spammer.packets.lock());
    // A file saved on Windows reads the same, and keeps its blank lines (File.ReadAllLines).
    assert_eq!(spammer::parse_queue("a\r\n\r\nb\r\n"), vec!["a", "", "b"]);
    assert_eq!(spammer::format_queue(&[]), "");
}

/// Edits outbound `afk` packets and drops inbound `chatm` ones.
struct Editor;

impl Interceptor for Editor {
    fn intercept(&self, message: &mut MessageInfo, outbound: bool) {
        if outbound {
            message.content = message.content.replace("%afk%1%true%", "%afk%1%false%");
        } else if message.content.contains("%chatm%") {
            message.send = false;
        }
    }
}

/// Records the order interceptors run in.
struct Order(i32, Arc<Mutex<Vec<i32>>>);

impl Interceptor for Order {
    fn priority(&self) -> i32 {
        self.0
    }

    fn intercept(&self, _message: &mut MessageInfo, _outbound: bool) {
        self.1.lock().push(self.0);
    }
}

fn packet(text: &str, outbound: Option<bool>) -> InterceptedPacket {
    InterceptedPacket {
        packet: text.into(),
        outbound,
    }
}

#[test]
fn capture_passes_packets_untouched_until_connected() {
    let capture = CaptureProxy::default();
    capture.add_interceptor(Arc::new(Editor));
    let mut out = Stream::default();
    let bytes = b"%xt%zm%afk%1%true%\0".to_vec();
    assert_eq!(out.filter(&capture, bytes.clone(), true), bytes);
    assert!(capture.packets().is_empty());
}

#[test]
fn capture_logs_in_and_out_edits_and_blocks() {
    let capture = CaptureProxy::default();
    capture.add_interceptor(Arc::new(Editor));
    capture.start();
    let (mut out, mut inbound) = (Stream::default(), Stream::default());
    assert_eq!(
        out.filter(&capture, b"%xt%zm%afk%1%true%\0".to_vec(), true),
        b"%xt%zm%afk%1%false%\0"
    );
    // Inbound bytes come in any chunks; a packet waits for its NUL, a dropped one disappears.
    assert_eq!(
        inbound.filter(&capture, b"%xt%chatm%0%hi%\0%xt%mo".to_vec(), false),
        b""
    );
    assert_eq!(
        inbound.filter(&capture, b"veToArea%-1%\0\0".to_vec(), false),
        b"%xt%moveToArea%-1%\0\0"
    );
    assert_eq!(
        capture.packets(),
        vec![
            packet("%xt%zm%afk%1%false%", Some(true)),
            packet("%xt%chatm%0%hi%", None),
            packet("%xt%moveToArea%-1%", Some(false)),
        ]
    );
    // Log packets off: still intercepted, not listed.
    capture.set_logging(false);
    assert_eq!(
        out.filter(&capture, b"%xt%zm%afk%1%true%\0".to_vec(), true),
        b"%xt%zm%afk%1%false%\0"
    );
    assert_eq!(capture.packets().len(), 3);
    // Disconnect: untouched again.
    capture.stop();
    assert_eq!(
        out.filter(&capture, b"%xt%zm%afk%1%true%\0".to_vec(), true),
        b"%xt%zm%afk%1%true%\0"
    );
    capture.clear();
    assert!(capture.packets().is_empty());
}

#[test]
fn capture_starting_mid_packet_skips_to_the_next_one() {
    let capture = CaptureProxy::default();
    let mut inbound = Stream::default();
    assert_eq!(
        inbound.filter(&capture, b"%xt%a%\0%xt%b".to_vec(), false),
        b"%xt%a%\0%xt%b"
    );
    capture.start();
    assert_eq!(
        inbound.filter(&capture, b"%\0%xt%c%\0".to_vec(), false),
        b"%\0%xt%c%\0"
    );
    assert_eq!(capture.packets(), vec![packet("%xt%c%", Some(false))]);
}

#[test]
fn capture_runs_interceptors_by_priority_and_removes_them() {
    let capture = CaptureProxy::default();
    let order = Arc::new(Mutex::new(Vec::new()));
    let late: Arc<dyn Interceptor> = Arc::new(Order(5, order.clone()));
    capture.add_interceptor(late.clone());
    capture.add_interceptor(Arc::new(Order(-1, order.clone())));
    capture.add_interceptor(Arc::new(Order(2, order.clone())));
    capture.start();
    capture.process(b"x", true);
    assert_eq!(*order.lock(), vec![-1, 2, 5]);
    capture.remove_interceptor(&late);
    order.lock().clear();
    capture.process(b"x", true);
    assert_eq!(*order.lock(), vec![-1, 2]);
}

#[test]
fn capture_inflates_compressed_server_packets_and_packs_edits() {
    let json = r#"{"t":"xt","b":{"r":-1,"o":{"cmd":"moveToArea","strMapName":"battleon"}}}"#;
    let packed = capture::pack(json);
    assert_eq!(packed[0], b'Z');
    assert_eq!(
        capture::unpack(std::str::from_utf8(&packed).unwrap()).as_deref(),
        Some(json)
    );
    // Not compressed, or too short to be: left alone.
    assert_eq!(capture::unpack("%xt%moveToArea%"), None);
    assert_eq!(capture::unpack("Zabc"), None);

    struct Rename;
    impl Interceptor for Rename {
        fn intercept(&self, message: &mut MessageInfo, _outbound: bool) {
            message.content = message.content.replace("battleon", "yulgar");
        }
    }
    let capture = CaptureProxy::default();
    capture.start();
    // Logged inflated; unchanged it is forwarded byte for byte.
    assert_eq!(
        capture.process(&packed, false).unwrap().as_ref(),
        &packed[..]
    );
    assert_eq!(capture.packets()[0].packet, json);
    capture.add_interceptor(Arc::new(Rename));
    let edited = capture.process(&packed, false).unwrap().into_owned();
    assert_eq!(
        capture::unpack(std::str::from_utf8(&edited).unwrap()).unwrap(),
        json.replace("battleon", "yulgar")
    );
    // Outbound packets are never inflated.
    let z = b"Zsome outbound text";
    assert_eq!(capture.process(z, true).unwrap().as_ref(), &z[..]);
}

#[test]
fn capture_keeps_the_last_1000_packets_numbered() {
    let capture = CaptureProxy::default();
    capture.start();
    for i in 0..1005 {
        capture.process(format!("p{i}").as_bytes(), true);
    }
    let (first, packets) = capture.snapshot();
    assert_eq!((first, packets.len()), (5, capture::LOG_CAPACITY));
    assert_eq!(packets[0].packet, "p5");
}

#[test]
fn export_marks_each_packet_out_in_or_block() {
    let text = capture::export_text(&[
        packet("%xt%zm%a%", Some(true)),
        packet("{\"t\":\"xt\"}\r\nnext", Some(false)),
        packet("%xt%chatm%", None),
    ]);
    let newline = if cfg!(windows) { "\r\n" } else { "\n" };
    assert_eq!(
        text,
        format!(
            "[OUT] %xt%zm%a%{newline}[IN] {{\"t\":\"xt\"}}\\r\\nnext{newline}[BLOCK] %xt%chatm%{newline}"
        )
    );
}
