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
    // The scheme in any case; no other header.
    let with = |name: &str, value: String| {
        Client::new()
            .get(format!("{base}/status"))
            .header(name, value)
            .send()
            .unwrap()
            .status()
            .as_u16()
    };
    assert_eq!(with("Authorization", format!("bearer {TOKEN}")), 200);
    assert_eq!(with("X-Api-Token", TOKEN.into()), 401);
    // 32 characters at least.
    for short in ["short", "0123456789abcdef"] {
        assert!(
            turu_bot::control_api::start(&_bot, "127.0.0.1:0".parse().unwrap(), short.into())
                .is_err(),
            "{short}"
        );
    }
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
        &format!("{base}/game/options"),
        Some(TOKEN),
        Some(json!({ "autoRelogin": true, "reloginServer": "Artix" })),
    );
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["reloginServer"], "Artix");
    assert!(bot.opts().auto_relogin);

    let (status, _) = call(
        "POST",
        &format!("{base}/game/options"),
        Some(TOKEN),
        Some(json!({ "noSuchOption": 1 })),
    );
    assert_eq!(status, 400);
    let huge = json!({ "customName": "x".repeat(70 * 1024) });
    let (status, _) = call(
        "POST",
        &format!("{base}/game/options"),
        Some(TOKEN),
        Some(huge),
    );
    assert_eq!(status, 413);
    let (_, options) = call("GET", &format!("{base}/game/options"), Some(TOKEN), None);
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
    // A token from before the 32-character minimum is replaced.
    std::fs::write(&file, "0123456789abcdef0123").unwrap();
    let replaced = turu_bot::control_api::saved_token(&file).unwrap();
    assert_eq!(replaced.len(), 64);
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

/// The wrong method on any route answers 405 and says which method it takes.
#[test]
fn wrong_methods_say_which_one_to_use() {
    let (_bot, base) = start();
    let client = Client::new();
    for (wrong, path, allowed) in [
        ("GET", "/update", "POST"),
        ("POST", "/debug/perf", "GET"),
        ("GET", "/script/stop", "POST"),
        ("POST", "/status", "GET"),
        ("PUT", "/game/options", "GET, POST"),
        ("DELETE", "/scheduler/options?id=x", "GET, POST"),
    ] {
        let method = reqwest::Method::from_bytes(wrong.as_bytes()).unwrap();
        let response = client
            .request(method, format!("{base}{path}"))
            .bearer_auth(TOKEN)
            .body("")
            .send()
            .unwrap();
        assert_eq!(response.status().as_u16(), 405, "{wrong} {path}");
        assert_eq!(
            response.headers()["allow"].to_str().unwrap(),
            allowed,
            "{wrong} {path}"
        );
    }
}

/// Game calls that can't reach the game answer 503, not 500: the API works, the game doesn't.
#[test]
fn says_when_the_game_isnt_running() {
    let (_bot, base) = start();
    for path in [
        "/map/join?map=battleon",
        "/player/goto?name=Artix",
        "/map/house",
        "/logout",
    ] {
        let (status, body) = call("POST", &format!("{base}{path}"), Some(TOKEN), None);
        assert_eq!(status, 503, "{path}: {body}");
        assert_eq!(body["error"], "flash: the game isn't running", "{path}");
    }
}

/// Playlists are read and saved inside the data folder only.
#[test]
fn keeps_playlist_files_in_the_data_folder() {
    let (bot, base) = start();
    let save = |path: &str| {
        call(
            "POST",
            &format!("{base}/scheduler/save?path={path}"),
            Some(TOKEN),
            None,
        )
    };
    let outside = std::env::temp_dir().join("turu-test-outside.json");
    assert_eq!(save(&outside.display().to_string()).0, 403);
    assert_eq!(save("../escape.json").0, 403);
    assert_eq!(
        call(
            "POST",
            &format!("{base}/scheduler/load?path=../../etc/passwd"),
            Some(TOKEN),
            None
        )
        .0,
        403
    );
    let script = bot.paths.scripts_dir().join("api-playlist-test.ts");
    std::fs::create_dir_all(script.parent().unwrap()).unwrap();
    std::fs::write(&script, "export default function main(): void {}\n").unwrap();
    assert_eq!(
        call(
            "POST",
            &format!("{base}/scheduler/add?path=api-playlist-test"),
            Some(TOKEN),
            None
        )
        .0,
        200
    );
    let name = format!("api-test-{}.json", std::process::id());
    let (status, body) = save(&name);
    assert_eq!(status, 200, "{body}");
    let file = bot.paths.playlists_dir().join(&name);
    assert_eq!(body["saved"], file.display().to_string());
    assert!(file.exists());
    let (status, body) = call(
        "POST",
        &format!("{base}/scheduler/load?path={}", file.display()),
        Some(TOKEN),
        None,
    );
    assert_eq!(status, 200, "{body}");
    let _ = std::fs::remove_file(file);
}

/// `/tabs` lists the tabs; `/tabs/<id>/...` reaches one, the rest the first.
#[test]
fn routes_requests_to_tabs() {
    unsafe { std::env::set_var("TURU_DATA_DIR", std::env::temp_dir().join("turu-test-data")) };
    let (first, second) = (Bot::new(), Bot::new());
    let bots = [first.clone(), second.clone()];
    let tabs: turu_bot::monitor::hub::Sources = Arc::new(move || {
        bots.iter()
            .enumerate()
            .map(|(i, bot)| turu_bot::monitor::hub::Source {
                id: format!("tab:{}", i * 7),
                label: format!("Tab {}", i + 1),
                bot: bot.clone(),
            })
            .collect()
    });
    let addr =
        turu_bot::control_api::start_tabs(tabs, "127.0.0.1:0".parse().unwrap(), TOKEN.into())
            .unwrap();
    let base = format!("http://{addr}");
    let (status, body) = call("GET", &format!("{base}/tabs"), Some(TOKEN), None);
    assert_eq!(status, 200);
    assert_eq!(
        body["tabs"],
        json!([
            { "id": "0", "label": "Tab 1", "default": true },
            { "id": "7", "label": "Tab 2", "default": false },
        ])
    );
    second.logs.script.push("from the second tab");
    let text = |body: &Value| {
        body["lines"]
            .as_array()
            .unwrap()
            .iter()
            .map(|l| l["text"].clone())
            .collect::<Vec<_>>()
    };
    let (_, body) = call(
        "GET",
        &format!("{base}/tabs/7/log?type=script"),
        Some(TOKEN),
        None,
    );
    assert_eq!(text(&body), vec![json!("from the second tab")]);
    let (_, body) = call("GET", &format!("{base}/log"), Some(TOKEN), None);
    assert!(text(&body).is_empty(), "{body}");
    assert_eq!(
        call("GET", &format!("{base}/tabs/3/status"), Some(TOKEN), None).0,
        404
    );
    assert_eq!(
        call("GET", &format!("{base}/tabs/7"), Some(TOKEN), None).0,
        404
    );
    assert_eq!(
        call("POST", &format!("{base}/tabs"), Some(TOKEN), None).0,
        405
    );
    drop(first);
}

/// `/events` streams the script's state, then new log lines and monitor events as they come.
#[test]
fn streams_events() {
    use std::io::{BufRead, BufReader};
    let (bot, base) = start();
    bot.logs.script.push("before the stream: not sent");
    assert_eq!(
        call(
            "GET",
            &format!("{base}/events?logs=nothing"),
            Some(TOKEN),
            None
        )
        .0,
        400
    );
    let response = Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap()
        .get(format!("{base}/events?logs=script,debug"))
        .bearer_auth(TOKEN)
        .send()
        .unwrap();
    assert_eq!(response.status().as_u16(), 200);
    assert_eq!(response.headers()["content-type"], "text/event-stream");
    let mut lines = BufReader::new(response).lines();
    let mut next_event = || {
        let event = lines.next().unwrap().unwrap();
        let data = lines.next().unwrap().unwrap();
        assert_eq!(lines.next().unwrap().unwrap(), "");
        let data: Value = serde_json::from_str(data.strip_prefix("data: ").unwrap()).unwrap();
        (event.strip_prefix("event: ").unwrap().to_owned(), data)
    };
    let (event, data) = next_event();
    assert_eq!(event, "script");
    assert_eq!(data["running"], false);
    bot.logs.debug.push("a debug line");
    let (event, data) = next_event();
    assert_eq!(event, "log");
    assert_eq!(
        (&data["type"], &data["text"]),
        (&json!("debug"), &json!("a debug line"))
    );
    bot.monitor
        .event(turu_bot::monitor::EventKind::ScriptError, "it broke");
    let (event, data) = next_event();
    assert_eq!(event, "event");
    assert_eq!(data["text"], "it broke");
}

/// Failures carry a stable `code` next to the message.
#[test]
fn failures_say_what_went_wrong_in_a_code() {
    let (_bot, base) = start();
    for (method, path, status, code) in [
        ("GET", "/nothing", 404, "no_route"),
        ("GET", "/script/stop", 405, "wrong_method"),
        ("POST", "/map/join", 400, "missing_parameter"),
        ("POST", "/map/join?map=battleon", 503, "game_not_running"),
        ("POST", "/script/pause", 409, "no_script_running"),
        ("POST", "/script/load?path=nowhere", 404, "no_script"),
        (
            "POST",
            "/scheduler/save?path=../x.json",
            403,
            "path_outside_data",
        ),
        ("GET", "/tabs/9/status", 404, "unknown_tab"),
        ("GET", "/gear", 409, "not_logged_in"),
    ] {
        let (got, body) = call(method, &format!("{base}{path}"), Some(TOKEN), None);
        assert_eq!(
            (got, body["code"].as_str()),
            (status, Some(code)),
            "{method} {path}: {body}"
        );
        assert!(body["error"].is_string(), "{method} {path}: {body}");
    }
    let (status, body) = call("GET", &format!("{base}/status"), None, None);
    assert_eq!((status, body["code"].as_str()), (401, Some("unauthorized")));
}

/// Yes/no parameters take 1 or true (0 or false), and numbers must be numbers.
#[test]
fn checks_flags_and_numbers() {
    let (_bot, base) = start();
    for path in ["/equip?item=Sword&stop=1", "/equip?item=Sword&stop=true"] {
        let (status, body) = call("POST", &format!("{base}{path}"), Some(TOKEN), None);
        assert_eq!(
            (status, body["code"].as_str()),
            (409, Some("not_logged_in")),
            "{path}"
        );
    }
    for (method, path) in [
        ("POST", "/equip?item=Sword&stop=yes"),
        ("GET", "/snapshot?slow=2"),
        ("GET", "/log?since=abc"),
        ("POST", "/quest/register?id=1&reward=lots"),
        ("POST", "/quest/register?id=one"),
    ] {
        let (status, body) = call(method, &format!("{base}{path}"), Some(TOKEN), None);
        assert_eq!(
            (status, body["code"].as_str()),
            (400, Some("invalid_parameter")),
            "{path}: {body}"
        );
    }
}

/// The script commands answer with the script, as `/status` has it.
#[test]
fn script_commands_answer_with_the_script() {
    let (_bot, base) = start();
    let dir = std::env::temp_dir().join(format!("turu-test-api-state-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let script = dir.join("idle.ts");
    std::fs::write(&script, "export default function main(): void {}\n").unwrap();
    let (status, body) = call(
        "POST",
        &format!("{base}/script/load?path={}", script.display()),
        Some(TOKEN),
        None,
    );
    assert_eq!(status, 200, "{body}");
    let (_, current) = call("GET", &format!("{base}/status"), Some(TOKEN), None);
    assert_eq!(body, current["script"]);
    assert_eq!(body["path"], script.display().to_string());
    assert_eq!(body["running"], false);
    let (status, body) = call("POST", &format!("{base}/script/stop"), Some(TOKEN), None);
    assert_eq!((status, &body["running"]), (200, &json!(false)), "{body}");
    assert_eq!(body["path"], script.display().to_string());
    let _ = std::fs::remove_dir_all(dir);
}
