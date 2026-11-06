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

#[test]
fn gear_routes_need_a_login() {
    let (_bot, base) = start();
    let (status, body) = call("GET", &format!("{base}/gear"), Some(TOKEN), None);
    assert_eq!(
        (status, body["error"].as_str()),
        (409, Some("not logged in"))
    );
    let (status, _) = call("POST", &format!("{base}/equip"), Some(TOKEN), None);
    assert_eq!(status, 400);
    let (status, body) = call(
        "POST",
        &format!("{base}/equip?item=Legion+Revenant&stop=1"),
        Some(TOKEN),
        None,
    );
    assert_eq!(
        (status, body["error"].as_str()),
        (409, Some("not logged in"))
    );
    let (status, _) = call("POST", &format!("{base}/bank/load"), Some(TOKEN), None);
    assert_eq!(status, 409);
    assert_eq!(
        call("GET", &format!("{base}/equip"), Some(TOKEN), None).0,
        405
    );
}

#[test]
fn reads_and_sets_script_options() {
    let (bot, base) = start();
    let dir = std::env::temp_dir().join(format!("turu-test-api-options-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let storage = format!("ApiOptions{}", std::process::id());
    let script = dir.join("opts.ts");
    std::fs::write(
        &script,
        format!(
            r#"import {{ bot, defineScriptOptions, option }} from "turu";
export const scriptOptions = defineScriptOptions("{storage}", [
    option.bool("Buy", "Buy potions", "Buys them first"),
    option.int("Count", "How many", "", 3),
    option.choice("Pet", "Pet", "", ["None", "Dragon_Pet"], "None"),
], false, {{ Extra: [option.text("Label", "Label", "", "x")] }});
export default function main(): void {{ bot.log(`buy=${{bot.config.get("Buy")}}`); }}
"#
        ),
    )
    .unwrap();
    let file = bot.paths.script_options_file(&storage);
    let _ = std::fs::remove_file(&file);

    // Nothing loaded yet.
    let (status, body) = call("GET", &format!("{base}/script/options"), Some(TOKEN), None);
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["options"], "none");
    assert_eq!(body["script"], Value::Null);

    let path = script.display().to_string();
    assert_eq!(
        call(
            "POST",
            &format!("{base}/script/load?path={path}"),
            Some(TOKEN),
            None
        )
        .0,
        200
    );
    let (status, body) = call("GET", &format!("{base}/script/options"), Some(TOKEN), None);
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["options"], "defaults");
    assert_eq!(body["saved"], false);
    assert_eq!(body["file"], format!("options/{storage}.cfg"));
    assert_eq!(body["categories"][0]["name"], "Options");
    let count = &body["categories"][0]["options"][1];
    assert_eq!(
        (
            &count["name"],
            &count["kind"],
            &count["value"],
            &count["default"]
        ),
        (&json!("Count"), &json!("int"), &json!(3), &json!(3))
    );
    assert_eq!(
        body["categories"][0]["options"][2]["choices"],
        json!(["None", "Dragon_Pet"])
    );
    assert_eq!(body["categories"][1]["name"], "Extra");

    // A wrong value changes nothing, not even the right ones next to it.
    let (status, body) = call(
        "POST",
        &format!("{base}/script/options"),
        Some(TOKEN),
        Some(json!({ "Buy": true, "Count": "many" })),
    );
    assert_eq!(status, 400, "{body}");
    assert!(body["error"].as_str().unwrap().contains("Count"), "{body}");
    assert!(!file.exists());

    let (status, body) = call(
        "POST",
        &format!("{base}/script/options"),
        Some(TOKEN),
        Some(json!({ "Buy": true, "Pet": "Dragon_Pet", "Extra": { "Label": "farm" } })),
    );
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["options"], "saved");
    assert_eq!(body["categories"][0]["options"][0]["value"], true);
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        "Options:Buy=True\nOptions:Count=3\nOptions:Pet=Dragon_Pet\nExtra:Label=farm\n"
    );
    assert_eq!(
        bot.scripts.option_value("Extra", "Label").unwrap(),
        json!("farm")
    );

    // The start says where the values came from, and that no window shows (no UI here).
    let (status, body) = call("POST", &format!("{base}/script/start"), Some(TOKEN), None);
    assert_eq!(status, 200, "{body}");
    assert_eq!(
        (&body["options"], &body["optionsWindow"]),
        (&json!("saved"), &json!(false))
    );
    let start = Instant::now();
    while bot.scripts.is_running() {
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "the script didn't finish"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    let log = bot.logs.script.text();
    assert!(log.contains("buy=true"), "{log}");

    let (status, body) = call(
        "POST",
        &format!("{base}/script/options?reset=1"),
        Some(TOKEN),
        None,
    );
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["categories"][0]["options"][0]["value"], false);
    assert!(
        std::fs::read_to_string(&file)
            .unwrap()
            .contains("Options:Buy=False")
    );
    assert_eq!(
        call(
            "POST",
            &format!("{base}/script/options"),
            Some(TOKEN),
            Some(json!([1]))
        )
        .0,
        400
    );
    let _ = std::fs::remove_file(&file);
}
