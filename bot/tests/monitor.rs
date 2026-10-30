//! The web monitor over real HTTP, with a bot whose game isn't connected.

use reqwest::blocking::Client;
use serde_json::Value;
use std::io::{BufRead, BufReader};
use std::sync::Arc;
use std::time::{Duration, Instant};
use turu_bot::Bot;
use turu_bot::monitor::Hub;
use turu_bot::monitor::hub::Source;
use turu_bot::monitor::push::{ReportTo, Reporter};
use turu_bot::monitor::server::{Key, Server};

const KEY: &str = "0123456789abcdef0123456789abcdef";
const PUSH_KEY: &str = "fedcba9876543210fedcba9876543210";

fn start() -> (Arc<Bot>, Server, String) {
    start_with(true)
}

fn start_with(allow_control: bool) -> (Arc<Bot>, Server, String) {
    // SAFETY: every test of this crate sets the same value.
    unsafe { std::env::set_var("TURU_DATA_DIR", std::env::temp_dir().join("turu-test-data")) };
    let bot = Bot::new();
    let tab = bot.clone();
    let hub = Hub::new(None);
    hub.set_tabs(Arc::new(move || {
        vec![Source {
            id: "tab:0".into(),
            label: "Tab 1".into(),
            bot: tab.clone(),
        }]
    }));
    hub.refresh_tabs();
    hub.start();
    let server = Server::start(
        "127.0.0.1:0".parse().unwrap(),
        Key::new(KEY.into()),
        Key::new(PUSH_KEY.into()),
        hub,
        "test".into(),
        allow_control,
    )
    .unwrap();
    let base = format!("http://{}", server.addr());
    (bot, server, base)
}

fn post(url: &str, origin: Option<&str>, key: Option<&str>) -> (u16, Value) {
    let mut request = Client::new().post(url);
    if let Some(origin) = origin {
        request = request.header(reqwest::header::ORIGIN, origin);
    }
    if let Some(key) = key {
        request = request.bearer_auth(key);
    }
    let response = request.send().unwrap();
    let status = response.status().as_u16();
    (
        status,
        serde_json::from_str(&response.text().unwrap()).unwrap_or(Value::Null),
    )
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

    let deleted = Client::new()
        .delete(format!("{base}/api/state"))
        .send()
        .unwrap();
    assert_eq!(deleted.status().as_u16(), 405);
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
fn takes_commands_from_its_own_page_or_with_the_key() {
    let (bot, _server, base) = start();
    let own = base.as_str();
    let stop = format!("{base}/api/bots/tab%3A0/script/stop");
    assert_eq!(
        post(&stop, Some(own), None),
        (200, serde_json::json!({ "stopping": false }))
    );
    // Another site's page, or a request with neither Origin nor key, is refused.
    assert_eq!(post(&stop, Some("https://evil.example"), None).0, 403);
    assert_eq!(post(&stop, Some("https://evil.example"), Some(KEY)).0, 403);
    assert_eq!(post(&stop, None, None).0, 403);
    assert_eq!(post(&stop, None, Some(KEY)).0, 200);

    // The control API's own refusals come through.
    let pause = format!("{base}/api/bots/tab:0/script/pause");
    assert_eq!(post(&pause, Some(own), None).0, 409);
    // Only the listed routes.
    assert_eq!(
        post(&format!("{base}/api/bots/tab:0/options"), Some(own), None).0,
        404
    );
    assert_eq!(
        post(
            &format!("{base}/api/bots/tab:9/script/stop"),
            Some(own),
            None
        )
        .0,
        404
    );
    assert_eq!(
        post(&format!("{base}/api/bots/tab:0/forget"), Some(own), None).0,
        409
    );

    // Logs and details.
    bot.log("hello from the monitor");
    let (status, body) = get(
        &format!("{base}/api/bots/tab:0/log?type=script"),
        None,
        None,
    );
    assert_eq!(status, 200);
    let log: Value = serde_json::from_str(&body).unwrap();
    let lines = log["lines"].as_array().unwrap();
    assert!(lines.iter().any(|l| l["text"] == "hello from the monitor"));
    assert_eq!(
        get(
            &format!("{base}/api/bots/tab:0/log?type=packet"),
            None,
            None
        )
        .0,
        400
    );
    let (status, body) = get(&format!("{base}/api/bots/tab:0"), None, None);
    assert_eq!(status, 200);
    let detail: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(detail["kind"], "tab");
    assert!(detail["events"].is_array() && detail["history"].is_array());
}

#[test]
fn refuses_commands_without_allow_control() {
    let (_bot, _server, base) = start_with(false);
    let (status, body) = get(&format!("{base}/api/state"), None, None);
    assert_eq!(status, 200);
    assert_eq!(
        serde_json::from_str::<Value>(&body).unwrap()["allowControl"],
        false
    );
    let stop = format!("{base}/api/bots/tab:0/script/stop");
    assert_eq!(post(&stop, Some(&base), Some(KEY)).0, 403);
}

#[test]
fn frees_its_port_when_dropped() {
    let (_bot, server, _base) = start();
    let addr = server.addr();
    drop(server);
    let again = Server::start(
        addr,
        Key::new(KEY.into()),
        Key::new(PUSH_KEY.into()),
        Hub::new(None),
        "test".into(),
        true,
    );
    assert!(again.is_ok(), "{:?}", again.err());
    drop(again);
    let short = Server::start(
        addr,
        Key::new("short".into()),
        Key::new(PUSH_KEY.into()),
        Hub::new(None),
        "".into(),
        true,
    );
    assert!(short.is_err());
}

/// A real `turu` client launched by Army reaches the hub through its `/snapshot`. Needs the
/// app built: `TURU_EXE=target/debug/turu cargo test -p turu_bot --test monitor -- --ignored`.
#[test]
#[ignore = "launches the turu app (TURU_EXE)"]
fn army_clients_reach_the_hub() {
    let exe = std::env::var("TURU_EXE").expect("TURU_EXE: the turu app to launch");
    let hub = Hub::new(None);
    let army = turu_bot::army::Army::new(exe.into());
    army.set_hub(hub.clone());
    let client = army
        .launch(turu_bot::army::Launch {
            username: "monitortest".into(),
            headless: true,
            ..Default::default()
        })
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(60);
    let snapshot = loop {
        if let Some(snapshot) = hub.army_snapshot("monitortest") {
            break snapshot;
        }
        assert!(
            Instant::now() < deadline,
            "no snapshot: {:?}",
            client.state()
        );
        std::thread::sleep(Duration::from_millis(500));
    };
    assert!(client.state().answered);
    let view = hub
        .views()
        .into_iter()
        .find(|v| v.id == "army:monitortest")
        .unwrap();
    assert!(view.online);
    assert!(matches!(
        hub.control("army:monitortest"),
        Some(turu_bot::monitor::hub::Control::Army(..))
    ));
    drop(snapshot);
    army.close("monitortest");
    assert!(hub.views().is_empty());
}

/// Waits up to 20 s for `check`.
fn eventually(what: &str, mut check: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while !check() {
        assert!(Instant::now() < deadline, "timed out: {what}");
        std::thread::sleep(Duration::from_millis(200));
    }
}

/// A remote machine's hub with one tab, reporting to the monitor at `base`.
fn remote(base: &str, accept_commands: bool) -> (Arc<Bot>, Arc<Hub>, Reporter) {
    let bot = Bot::new();
    let tab = bot.clone();
    let hub = Hub::new(None);
    hub.set_tabs(Arc::new(move || {
        vec![Source {
            id: "tab:0".into(),
            label: "Tab 1".into(),
            bot: tab.clone(),
        }]
    }));
    hub.start();
    let reporter = Reporter::start(
        hub.clone(),
        ReportTo {
            url: format!("{base}/"),
            push_key: PUSH_KEY.into(),
            machine: "vps 1".into(),
            accept_commands,
            version: "test".into(),
        },
    )
    .unwrap();
    (bot, hub, reporter)
}

#[test]
fn remote_machines_report_and_take_commands() {
    let (_bot, _server, base) = start();
    let (remote_bot, _remote_hub, reporter) = remote(&base, true);
    remote_bot.log("hello from the vps");
    let id = "remote:vps 1:tab:0";
    eventually("the remote tab on the page", || {
        let (_, body) = get(&format!("{base}/api/state"), None, None);
        let state: Value = serde_json::from_str(&body).unwrap();
        state["bots"].as_array().unwrap().iter().any(|b| {
            b["id"] == id
                && b["online"] == true
                && b["machine"] == "vps 1"
                && !b["snapshot"].is_null()
        })
    });
    assert!(reporter.status().error.is_none());

    // A command makes the round trip: queued here, run there, answered with the next report.
    let url = format!(
        "{base}/api/bots/{}/script/stop",
        id.replace(':', "%3A").replace(' ', "%20")
    );
    let started = Instant::now();
    assert_eq!(
        post(&url, Some(&base), None),
        (200, serde_json::json!({ "stopping": false }))
    );
    assert!(
        started.elapsed() < Duration::from_secs(8),
        "took {:?}",
        started.elapsed()
    );
    // So does the log.
    let (status, body) = get(
        &format!("{base}/api/bots/{}/log", id.replace(' ', "%20")),
        None,
        None,
    );
    assert_eq!(status, 200);
    assert!(body.contains("hello from the vps"));
}

#[test]
fn remote_commands_need_accept_commands() {
    let (_bot, _server, base) = start();
    let (_remote_bot, _hub, _reporter) = remote(&base, false);
    let id = "remote%3Avps%201%3Atab%3A0";
    eventually("the remote tab", || {
        get(&format!("{base}/api/bots/{id}"), None, None).0 == 200
    });
    let (status, body) = post(
        &format!("{base}/api/bots/{id}/script/stop"),
        Some(&base),
        None,
    );
    assert_eq!(status, 403, "{body}");
    // Reading the log still works.
    assert_eq!(get(&format!("{base}/api/bots/{id}/log"), None, None).0, 200);
}

#[test]
fn reports_need_the_push_key() {
    let (_bot, _server, base) = start();
    let report = |key: &str| {
        Client::new()
            .post(format!("{base}/api/ingest"))
            .bearer_auth(key)
            .body(r#"{"machine":"x","bots":[]}"#)
            .send()
            .unwrap()
            .status()
            .as_u16()
    };
    assert_eq!(report(KEY), 401, "the view key isn't the push key");
    assert_eq!(report("wrong-wrong-wrong-wrong"), 401);
    assert_eq!(report(PUSH_KEY), 200);
    // The push key doesn't open the page's routes.
    let other = Some("my-pc.tail1234.ts.net");
    assert_eq!(
        get(&format!("{base}/api/state"), other, Some(PUSH_KEY)).0,
        401
    );
}
