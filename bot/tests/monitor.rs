//! The web monitor over real HTTP, with a bot whose game isn't connected.

use reqwest::blocking::Client;
use serde_json::Value;
use std::io::{BufRead, BufReader};
use std::sync::Arc;
use std::time::{Duration, Instant};
use turu_bot::Bot;
use turu_bot::monitor::server::{Key, Server, Source};

const KEY: &str = "0123456789abcdef0123456789abcdef";

fn start() -> (Arc<Bot>, Server, String) {
    // SAFETY: every test of this crate sets the same value.
    unsafe { std::env::set_var("TURU_DATA_DIR", std::env::temp_dir().join("turu-test-data")) };
    let bot = Bot::new();
    let tab = bot.clone();
    let sources = Arc::new(move || {
        vec![Source {
            id: "tab:0".into(),
            label: "Tab 1".into(),
            bot: tab.clone(),
        }]
    });
    let server = Server::start(
        "127.0.0.1:0".parse().unwrap(),
        Key::new(KEY.into()),
        sources,
        "test".into(),
    )
    .unwrap();
    let base = format!("http://{}", server.addr());
    (bot, server, base)
}

fn get(url: &str, host: Option<&str>, key: Option<&str>) -> (u16, String) {
    let mut request = Client::new().get(url);
    if let Some(host) = host {
        request = request.header(reqwest::header::HOST, host);
    }
    if let Some(key) = key {
        request = request.bearer_auth(key);
    }
    let response = request.send().unwrap();
    (response.status().as_u16(), response.text().unwrap())
}

#[test]
fn serves_the_page_to_anyone_and_the_state_with_the_key() {
    let (_bot, _server, base) = start();
    let (status, page) = get(&format!("{base}/"), Some("my-pc.tail1234.ts.net"), None);
    assert_eq!(status, 200);
    assert!(page.contains("<title>Turu Monitor</title>"));

    // A browser on this computer needs no key.
    let (status, body) = get(&format!("{base}/api/state"), None, None);
    assert_eq!(status, 200);
    let state: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(state["version"], "test");
    assert_eq!(state["bots"][0]["id"], "tab:0");
    assert_eq!(state["bots"][0]["label"], "Tab 1");

    // A proxy (Tailscale Serve) or a rebound DNS name connects from loopback with another host.
    let other = Some("my-pc.tail1234.ts.net");
    assert_eq!(get(&format!("{base}/api/state"), other, None).0, 401);
    assert_eq!(
        get(
            &format!("{base}/api/state"),
            other,
            Some("wrong-key-wrong-key")
        )
        .0,
        401
    );
    assert_eq!(get(&format!("{base}/api/state"), other, Some(KEY)).0, 200);

    let posted = Client::new()
        .post(format!("{base}/api/state"))
        .send()
        .unwrap();
    assert_eq!(posted.status().as_u16(), 405);
    assert_eq!(get(&format!("{base}/api/nothing"), None, None).0, 404);
}

#[test]
fn streams_snapshots_while_watched() {
    let (_bot, _server, base) = start();
    let response = Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .unwrap()
        .get(format!("{base}/api/events"))
        .send()
        .unwrap();
    assert_eq!(response.status().as_u16(), 200);
    assert_eq!(
        response.headers()["content-type"].to_str().unwrap(),
        "text/event-stream"
    );
    // The stream makes the sampler read the bot; its snapshot shows up within a few frames.
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut lines = BufReader::new(response).lines();
    let snapshot = loop {
        assert!(Instant::now() < deadline, "no snapshot in the stream");
        let line = lines.next().unwrap().unwrap();
        let Some(data) = line.strip_prefix("data: ") else {
            continue;
        };
        let state: Value = serde_json::from_str(data).unwrap();
        let snapshot = &state["bots"][0]["snapshot"];
        if !snapshot.is_null() {
            break snapshot.clone();
        }
    };
    assert_eq!(snapshot["state"], "notLoaded");
    assert_eq!(snapshot["script"]["running"], false);
}

#[test]
fn frees_its_port_when_dropped() {
    let (_bot, server, _base) = start();
    let addr = server.addr();
    drop(server);
    let again = Server::start(
        addr,
        Key::new(KEY.into()),
        Arc::new(Vec::new),
        "test".into(),
    );
    assert!(again.is_ok(), "{:?}", again.err());
    assert!(
        Server::start(
            addr,
            Key::new("short".into()),
            Arc::new(Vec::new),
            "".into()
        )
        .is_err()
    );
}
