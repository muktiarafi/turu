//! Scrubbing tapes (docs/REPLAY_TESTS_PROPOSAL.md, phase 2): every player renamed the same way
//! everywhere, the account's numbers and personal fields replaced, and the result checked.
//! The "real" names here are made up too.

use serde_json::{Value, json};
use std::io::Read;
use std::path::{Path, PathBuf};
use turu_bot::tape::{self, End, FORMAT, Header, Line, Tape, redact};

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("turu-test-scrub-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn raw(path: &Path) -> String {
    let mut text = String::new();
    flate2::read::MultiGzDecoder::new(std::fs::File::open(path).unwrap())
        .read_to_string(&mut text)
        .unwrap();
    text
}

fn call(n: u64, name: &str, args: Value, answer: &str) -> Line {
    Line::Call {
        n,
        t: n * 10,
        thread: Some("Script Thread".into()),
        name: name.into(),
        args: args.as_array().unwrap().clone(),
        answer: Some(answer.into()),
        error: None,
        run_ms: None,
    }
}

fn received(n: u64, text: impl Into<String>) -> Line {
    Line::Received {
        n,
        t: n * 10,
        socket: 1,
        text: text.into(),
    }
}

fn packet(o: Value) -> String {
    json!({"t": "xt", "b": {"r": -1, "o": o}}).to_string()
}

/// A tape as a recording of the account "Brightmoss" would be, with Mossfoot, Kestrelwing and Al
/// in the room and a monster called Albatross.
fn recorded() -> Tape {
    let lines = vec![
        call(
            0,
            "getGameObjectS",
            json!(["loginInfo.strUsername"]),
            r#""Brightmoss""#,
        ),
        received(
            1,
            packet(json!({"cmd": "initUserData", "uid": 12851, "data": {
                "strUsername": "Brightmoss", "strEmail": "brightmoss@example.com",
                "strIP": "192.0.2.44", "UserID": 74000001, "CharID": 74000002,
                "sCountry": "ID", "iAge": 29, "dCreated": "2026-11-26T10:08:00", "ip0": 5,
            }})),
        ),
        received(
            2,
            packet(json!({"cmd": "moveToArea", "areaId": 54133, "uoBranch": [
                {"strUsername": "Brightmoss", "uoName": "brightmoss", "entID": 12851},
                {"strUsername": "Mossfoot", "uoName": "mossfoot", "entID": 12900},
            ], "mondef": [{"strMonName": "Albatross"}]})),
        ),
        received(
            3,
            "<msg t='sys'><body action='joinOK' r='54133'><uLs r='54133'><u i='12901' m='0'><n>\
             <![CDATA[Kestrelwing]]></n></u></uLs></body></msg>",
        ),
        received(4, "%xt%uotls%-1%al%afk:true%"),
        received(
            5,
            packet(
                json!({"cmd": "ct", "p": {"brightmoss": {"intHP": 100}, "mossfoot": {"intHP": 90}},
                "m": {"1": {"targets": ["al"]}}, "sarsa": [{"cInf": "p:12851"}]}),
            ),
        ),
        call(
            6,
            "getGameObject",
            json!(["world.areaUsers"]),
            r#"["Brightmoss","Mossfoot","Al","Kestrelwing"]"#,
        ),
        call(
            7,
            "callGameFunction",
            json!([
                "sfc.sendString",
                "%xt%zm%cmd%1%tfer%Brightmoss%whitemap-100000%"
            ]),
            "null",
        ),
        received(8, packet(json!({"cmd": "playerDeath", "userID": 74000002}))),
        Line::Event {
            n: 9,
            t: 90,
            name: "pext".into(),
            args: vec![Value::String(
                json!({"params": {"type": "json", "dataObj": {"cmd": "uotls", "unm": "Mossfoot"}}})
                    .to_string(),
            )],
        },
        Line::Note {
            n: 10,
            t: 100,
            what: "scriptStarted".into(),
            detail: json!({"script": "farm/x.ts", "options": {"follow": "Mossfoot"}}),
        },
        call(
            11,
            "getGameObject",
            json!(["world.myAvatar.objData.CharID"]),
            "74000002",
        ),
        received(12, "%xt%exitArea%-1%12900%Mossfoot%"),
    ];
    Tape {
        header: Header {
            format: FORMAT,
            turu: "0.4.0".into(),
            started_unix_ms: 0,
            from_world: true,
            options: json!({"customName": "", "reloginServer": "Twig"}),
            scrubbed: false,
        },
        end: Some(End {
            t: 200,
            lines: lines.len() as u64,
            dropped: 0,
            unanswered: vec![],
        }),
        lines,
        missing: vec![],
        truncated: false,
    }
}

fn scrubbed(dir: &Path) -> (PathBuf, Tape, tape::scrub::Report) {
    let input = dir.join("in.tape.jsonl.gz");
    let output = dir.join("out.tape.jsonl.gz");
    tape::write(&input, &recorded()).unwrap();
    let report = tape::scrub_file(&input, &output).unwrap();
    let tape = tape::read(&output).unwrap();
    (output, tape, report)
}

fn text_of(line: &Line) -> String {
    match line {
        Line::Received { text, .. } => text.clone(),
        Line::Call { args, answer, .. } => format!("{args:?} {answer:?}"),
        other => serde_json::to_string(other).unwrap(),
    }
}

#[test]
fn every_player_is_renamed_the_same_way_everywhere() {
    let dir = temp("names");
    let (output, tape, report) = scrubbed(&dir);
    let all = raw(&output).to_lowercase();
    for real in ["brightmoss", "mossfoot", "kestrelwing"] {
        assert!(!all.contains(real), "{real} is still in the tape");
    }
    let names: Vec<&str> = report.renamed.iter().map(|(to, _)| to.as_str()).collect();
    assert_eq!(names, ["Questwalker", "Player2", "Player3", "Player4"]);
    let lines = &tape.lines;
    assert!(text_of(&lines[0]).contains(r#"\"Questwalker\""#));
    // In the case each place had: `ct`'s keys and `uoName` are lowercase.
    let ct: Value = serde_json::from_str(&text_of(&lines[5])).unwrap();
    let ct = &ct["b"]["o"];
    assert!(ct["p"]["questwalker"].is_object() && ct["p"]["player2"].is_object());
    assert_eq!(ct["m"]["1"]["targets"], json!(["player4"]));
    let move_to = text_of(&lines[2]);
    assert!(
        move_to.contains(r#""uoName":"player2""#) && move_to.contains(r#""strUsername":"Player2""#)
    );
    assert!(text_of(&lines[3]).contains("<![CDATA[Player3]]>"));
    assert_eq!(text_of(&lines[4]), "%xt%uotls%-1%player4%afk:true%");
    assert!(
        matches!(&lines[6], Line::Call { answer: Some(a), .. } if a == r#"["Questwalker","Player2","Player4","Player3"]"#)
    );
    assert!(text_of(&lines[7]).contains("%tfer%Questwalker%"));
    assert!(text_of(&lines[9]).contains(r#"\"unm\":\"Player2\""#));
    assert!(text_of(&lines[10]).contains(r#""follow":"Player2""#));
    assert_eq!(text_of(&lines[12]), "%xt%exitArea%-1%12900%Player2%");
    // A short name doesn't rename what merely starts with it.
    assert!(move_to.contains("Albatross"));
}

#[test]
fn account_numbers_and_personal_fields_are_replaced() {
    let dir = temp("numbers");
    let (output, tape, report) = scrubbed(&dir);
    let all = raw(&output);
    for real in [
        "74000001",
        "74000002",
        "brightmoss@example.com",
        "192.0.2.44",
        "2026-11-26T10:08:00",
    ] {
        assert!(!all.contains(real), "{real} is still in the tape");
    }
    assert_eq!(report.ids, 2);
    let init: Value = serde_json::from_str(&text_of(&tape.lines[1])).unwrap();
    let data = &init["b"]["o"]["data"];
    assert_eq!(data["strEmail"], redact::REDACTED);
    assert_eq!(data["strIP"], redact::REDACTED);
    assert_eq!(
        (data["sCountry"].clone(), data["iAge"].clone()),
        (json!("XX"), json!(0))
    );
    assert_eq!(data["ip0"], 5, "quest progress isn't an address");
    // Renumbered the same way wherever the number shows.
    let char_id = data["CharID"].clone();
    assert_ne!(char_id, json!(74000002));
    let death: Value = serde_json::from_str(&text_of(&tape.lines[8])).unwrap();
    assert_eq!(death["b"]["o"]["userID"], char_id);
    assert!(
        matches!(&tape.lines[11], Line::Call { answer: Some(a), .. } if a == &char_id.to_string())
    );
    // The room's player numbers and the room's own stay.
    assert!(all.contains("p:12851") && all.contains("54133"));
}

#[test]
fn a_scrubbed_tape_keeps_its_lines_and_says_it_was_scrubbed() {
    let dir = temp("shape");
    let (_, tape, _) = scrubbed(&dir);
    let before = recorded();
    assert!(tape.header.scrubbed && !before.header.scrubbed);
    assert_eq!(
        tape.lines.iter().map(Line::n).collect::<Vec<_>>(),
        before.lines.iter().map(Line::n).collect::<Vec<_>>()
    );
    assert_eq!(tape.end, before.end);
    assert!(tape.is_complete());
}

#[test]
fn scrubbing_twice_changes_nothing() {
    let dir = temp("twice");
    let (once, _, _) = scrubbed(&dir);
    let twice = dir.join("twice.tape.jsonl.gz");
    tape::scrub_file(&once, &twice).unwrap();
    assert_eq!(raw(&once), raw(&twice));
}

#[test]
fn a_scrub_never_writes_over_a_file() {
    let dir = temp("over");
    let input = dir.join("in.tape.jsonl.gz");
    tape::write(&input, &recorded()).unwrap();
    let error = tape::scrub_file(&input, &input).unwrap_err();
    assert!(error.to_string().contains("exists"), "{error}");
    assert!(!tape::read(&input).unwrap().header.scrubbed);
}

#[test]
fn a_custom_name_in_the_options_is_renamed_too() {
    let mut recorded = recorded();
    recorded.header.options["customName"] = json!("Sir Brightmoss");
    recorded.lines.push(call(
        13,
        "setGameObject",
        json!(["world.myAvatar.pMC.pname.ti.text", "Sir Brightmoss"]),
        "null",
    ));
    let (tape, _) = tape::scrub(recorded);
    let all = serde_json::to_string(&tape.header).unwrap()
        + &tape.lines.iter().map(text_of).collect::<String>();
    assert!(!all.to_lowercase().contains("sir brightmoss"));
}
