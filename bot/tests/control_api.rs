//! The control API over real HTTP, against a game that isn't connected.

use reqwest::blocking::Client;
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::{Duration, Instant};
use turu_bot::Bot;

const TOKEN: &str = "0123456789abcdef0123456789abcdef";

fn start() -> (Arc<Bot>, String) {
    // SAFETY: every test of this crate sets the same value.
    unsafe { std::env::set_var("TURU_DATA_DIR", std::env::temp_dir().join("turu-test-data")) };
    let bot = Bot::new();
    let addr =
        turu_bot::control_api::start(&bot, "127.0.0.1:0".parse().unwrap(), TOKEN.into()).unwrap();
    (bot, format!("http://{addr}"))
}

fn call(method: &str, url: &str, token: Option<&str>, body: Option<Value>) -> (u16, Value) {
    let client = Client::new();
    let mut request = match method {
        "GET" => client.get(url),
        _ => client
            .post(url)
            .body(body.map_or_else(String::new, |b| b.to_string())),
    };
    if let Some(token) = token {
        request = request.bearer_auth(token);
    }
    let response = request.send().unwrap();
    let status = response.status().as_u16();
    (
        status,
        serde_json::from_str(&response.text().unwrap()).unwrap(),
    )
}

#[test]
fn refuses_requests_without_the_token() {
    let (_bot, base) = start();
    assert_eq!(call("GET", &format!("{base}/status"), None, None).0, 401);
    assert_eq!(
        call(
            "GET",
            &format!("{base}/status"),
            Some("wrong-token-wrong-token"),
            None
        )
        .0,
        401
    );
    let (status, body) = call("GET", &format!("{base}/status"), Some(TOKEN), None);
    assert_eq!(status, 200);
    assert_eq!(body["connected"], false);
    assert_eq!(body["script"]["running"], false);
    // X-Api-Token works too.
    let response = Client::new()
        .get(format!("{base}/status"))
        .header("X-Api-Token", TOKEN)
        .send()
        .unwrap();
    assert_eq!(response.status().as_u16(), 200);
    assert!(
        turu_bot::control_api::start(&_bot, "127.0.0.1:0".parse().unwrap(), "short".into())
            .is_err()
    );
}

#[test]
fn runs_scripts_and_reads_their_log() {
    let (bot, base) = start();
    let dir = std::env::temp_dir().join(format!("turu-test-api-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("hello.ts"),
        "import { bot } from \"turu\";\nexport default function main(): void { bot.log(\"hello from the API\"); }\n",
    )
    .unwrap();

    let missing = call(
        "POST",
        &format!("{base}/script/load?path=nowhere"),
        Some(TOKEN),
        None,
    );
    assert_eq!(missing.0, 404);
    // `.ts` is optional.
    let path = dir.join("hello").display().to_string();
    let (status, body) = call(
        "POST",
        &format!("{base}/script/start?path={path}"),
        Some(TOKEN),
        None,
    );
    assert_eq!(status, 200, "{body}");
    let start = Instant::now();
    while bot.scripts.is_running() {
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "the script didn't finish"
        );
        std::thread::sleep(Duration::from_millis(10));
    }

    let (_, log) = call("GET", &format!("{base}/log?type=script"), Some(TOKEN), None);
    let lines: Vec<&str> = log["lines"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["text"].as_str().unwrap())
        .collect();
    assert!(lines.contains(&"hello from the API"), "{lines:?}");
    let next = log["next"].as_u64().unwrap();
    let (_, later) = call(
        "GET",
        &format!("{base}/log?since={next}"),
        Some(TOKEN),
        None,
    );
    assert_eq!(later["lines"], json!([]));
    assert_eq!(later["next"], next);

    assert_eq!(
        call("POST", &format!("{base}/script/pause"), Some(TOKEN), None).0,
        409
    );
    assert_eq!(
        call("GET", &format!("{base}/script/stop"), Some(TOKEN), None).0,
        405
    );
    assert_eq!(
        call("GET", &format!("{base}/nothing"), Some(TOKEN), None).0,
        404
    );
}

#[test]
fn sets_options() {
    let (bot, base) = start();
    let (status, body) = call(
        "POST",
        &format!("{base}/options"),
        Some(TOKEN),
        Some(json!({ "autoRelogin": true, "reloginServer": "Artix" })),
    );
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["reloginServer"], "Artix");
    assert!(bot.opts().auto_relogin);

    let (status, _) = call(
        "POST",
        &format!("{base}/options"),
        Some(TOKEN),
        Some(json!({ "noSuchOption": 1 })),
    );
    assert_eq!(status, 400);
    let huge = json!({ "customName": "x".repeat(70 * 1024) });
    let (status, _) = call("POST", &format!("{base}/options"), Some(TOKEN), Some(huge));
    assert_eq!(status, 413);
    let (_, options) = call("GET", &format!("{base}/options"), Some(TOKEN), None);
    assert_eq!(options["autoRelogin"], true);
}

#[test]
fn keeps_the_token_in_a_private_file() {
    let file = std::env::temp_dir().join(format!("turu-test-token-{}", std::process::id()));
    let _ = std::fs::remove_file(&file);
    let token = turu_bot::control_api::saved_token(&file).unwrap();
    assert_eq!(
        turu_bot::control_api::saved_token(&file).unwrap(),
        token,
        "reused"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&file).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}
