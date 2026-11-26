//! Making a tape fit to commit: every player renamed to a made-up one, the account's numbers and
//! personal fields replaced (docs/REPLAY_TESTS_PROPOSAL.md, "Secrets and names"). A recording
//! keeps names (a replay needs them consistent); [`scrub`] replaces them all at once, the same way
//! everywhere, and marks the header `scrubbed`.
//!
//! What is renamed: the recording account ("Questwalker") and every other player the tape names
//! ("Player2", "Player3", ... in order of first appearance), wherever the name shows, in any case
//! (`questwalker` in `ct` packets). The account's `UserID` and `CharID` (and other players') are
//! renumbered; `sCountry`, `iAge` and `dCreated` get fixed values; the redaction a recording does
//! is done again (tapes made before it covered the email and the address). Room numbers and the
//! room's player numbers (`uid`, `entID`) stay: they last a session and name no one.
//!
//! [`scrub_file`] writes the result and reads it back, failing (and removing it) if any name or
//! number it replaced is still in it.

use super::{Line, Tape, redact};
use regex::{Captures, Regex};
use serde_json::Value;
use std::collections::HashMap;
use std::io::{self, Read};
use std::path::Path;
use std::sync::LazyLock;

/// What the recording account is renamed to.
pub const ACCOUNT: &str = "Questwalker";

/// The first number given to a renumbered `UserID` or `CharID`.
const FIRST_ID: u64 = 100_000_001;

/// What a scrub replaced.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Report {
    /// Each name it replaced (by what it became, not what it was) and in how many places.
    pub renamed: Vec<(String, usize)>,
    /// How many account numbers (`UserID`, `CharID`) it renumbered.
    pub ids: usize,
    /// How many lines it wrote.
    pub lines: usize,
}

/// Why [`scrub_file`] failed.
#[derive(Debug, thiserror::Error)]
pub enum ScrubError {
    #[error("{0}")]
    Io(#[from] io::Error),
    /// Something it replaced is still in the written tape (removed again).
    #[error("the scrubbed tape still holds {0}; it was removed")]
    Left(String),
}

/// The names and numbers a tape holds, found before anything is replaced.
#[derive(Default)]
struct Found {
    /// The recording account's name.
    account: Option<String>,
    /// Every player's name, in order of first appearance (the account's too).
    names: Vec<String>,
    /// `UserID`s and `CharID`s, in order of first appearance.
    ids: Vec<u64>,
    /// The email and address fields' values, which redaction replaces: checked for afterwards.
    secrets: Vec<String>,
}

impl Found {
    fn name(&mut self, name: &str) {
        let name = name.trim();
        if name.is_empty()
            || name.starts_with('<')
            || name.len() > 40
            || self.names.iter().any(|n| n.eq_ignore_ascii_case(name))
        {
            return;
        }
        self.names.push(name.to_owned());
    }

    fn id(&mut self, value: &Value) {
        let id = match value {
            Value::Number(n) => n.as_u64(),
            Value::String(s) => s.parse().ok(),
            _ => None,
        };
        // Small numbers would match too much else; the game's are 8 digits.
        if let Some(id) = id.filter(|id| *id >= 10_000)
            && !self.ids.contains(&id)
        {
            self.ids.push(id);
        }
    }
}

/// The paths whose read answers the account's name.
const ACCOUNT_PATHS: &[&str] = &[
    "loginInfo.strUsername",
    "sfc.myUserName",
    "world.myAvatar.objData.strUsername",
];

/// Field names whose value is a player's name.
const NAME_KEYS: &[&str] = &["strUsername", "uoName", "unm"];

fn is_id_key(key: &str) -> bool {
    key.eq_ignore_ascii_case("userid") || key.eq_ignore_ascii_case("charid")
}

/// A `get...` call's path, the last segment of which is the field read.
fn call_path(args: &[Value]) -> Option<&str> {
    args.first().and_then(Value::as_str)
}

fn collect(tape: &Tape) -> Found {
    let mut found = Found::default();
    for line in &tape.lines {
        if let Line::Call {
            name, args, answer, ..
        } = line
            && name.starts_with("getGameObject")
            && let Some(path) = call_path(args)
            && ACCOUNT_PATHS.contains(&path)
            && let Some(Ok(Value::String(user))) = answer.as_deref().map(serde_json::from_str)
            && !user.is_empty()
            && !user.starts_with('<')
        {
            found.account = Some(user);
            break;
        }
        if let Line::Received { text, .. } = line
            && text.contains("\"initUserData\"")
            && let Ok(v) = serde_json::from_str::<Value>(text)
            && let Some(user) = v.pointer("/b/o/data/strUsername").and_then(Value::as_str)
        {
            found.account = Some(user.to_owned());
            break;
        }
    }
    if let Some(account) = found.account.clone() {
        found.name(&account);
    }
    for line in &tape.lines {
        match line {
            Line::Call {
                args,
                answer,
                error,
                ..
            } => {
                args.iter().for_each(|a| collect_json(a, &mut found));
                let path = call_path(args).unwrap_or_default();
                let field = path.rsplit('.').next().unwrap_or_default();
                if let Some(answer) = answer {
                    match serde_json::from_str::<Value>(answer) {
                        Ok(Value::String(s)) if is_id_key(field) => found.id(&Value::String(s)),
                        Ok(Value::String(s)) if ACCOUNT_PATHS.contains(&path) => found.name(&s),
                        Ok(Value::Number(n)) if is_id_key(field) => found.id(&Value::Number(n)),
                        // `world.areaUsers`: the room's names.
                        Ok(Value::Array(names)) if path == "world.areaUsers" => {
                            names
                                .iter()
                                .filter_map(Value::as_str)
                                .for_each(|n| found.name(n));
                        }
                        // `world.uoTree`: keyed by (lowercase) name.
                        Ok(Value::Object(tree)) if field == "uoTree" => {
                            tree.keys().for_each(|n| found.name(n));
                            collect_json(&Value::Object(tree), &mut found);
                        }
                        Ok(v) => collect_json(&v, &mut found),
                        Err(_) => collect_text(answer, &mut found),
                    }
                }
                if let Some(error) = error {
                    collect_text(error, &mut found);
                }
            }
            Line::Event { args, .. } => args.iter().for_each(|a| collect_json(a, &mut found)),
            Line::Sent { text, .. } | Line::Received { text, .. } => collect_text(text, &mut found),
            Line::Note { detail, .. } => collect_json(detail, &mut found),
        }
    }
    for key in ["customName", "customGuild"] {
        if let Some(name) = tape.header.options.get(key).and_then(Value::as_str) {
            found.name(name);
        }
    }
    found
}

/// A string from the game: a JSON packet or answer, a `%xt%` packet, a SmartFox message.
fn collect_text(s: &str, found: &mut Found) {
    let s = s.trim_start();
    if s.starts_with('{') || s.starts_with('[') {
        if let Ok(v) = serde_json::from_str::<Value>(s) {
            collect_json(&v, found);
        }
    } else if s.starts_with("%xt%") {
        let parts: Vec<&str> = s.split('%').collect();
        // `%xt%uotls%-1%<name>%...`, `%xt%exitArea%-1%<id>%<name>%`.
        let at = match parts.get(2) {
            Some(&"uotls") => Some(4),
            Some(&"exitArea") => Some(5),
            _ => None,
        };
        if let Some(name) = at.and_then(|at| parts.get(at)) {
            found.name(name);
        }
    } else if s.starts_with('<') {
        // `<u i='..' ...><n><![CDATA[name]]></n>`: joinOK, uER.
        static NAME: LazyLock<Regex> =
            LazyLock::new(|| Regex::new(r"<n><!\[CDATA\[(.*?)\]\]></n>").unwrap());
        for c in NAME.captures_iter(s) {
            found.name(&c[1]);
        }
    }
}

fn collect_json(v: &Value, found: &mut Found) {
    match v {
        Value::Object(map) => {
            // `ct`: its `p` is keyed by (lowercase) name.
            if map.get("cmd").and_then(Value::as_str) == Some("ct")
                && let Some(Value::Object(players)) = map.get("p")
            {
                players.keys().for_each(|n| found.name(n));
            }
            for (key, field) in map {
                if NAME_KEYS.contains(&key.as_str()) {
                    if let Some(name) = field.as_str() {
                        found.name(name);
                    }
                } else if is_id_key(key) {
                    found.id(field);
                } else if redact::is_secret_key(key)
                    && let Some(s) = field.as_str()
                    && s.len() >= 4
                    && s != redact::REDACTED
                {
                    found.secrets.push(s.to_owned());
                }
                collect_json(field, found);
            }
        }
        Value::Array(items) => items.iter().for_each(|i| collect_json(i, found)),
        Value::String(s) => collect_text(s, found),
        _ => {}
    }
}

/// What a scrub replaces, built from what [`collect`] found.
struct Rewrite {
    /// Every name, longest first, case-insensitively, at word boundaries.
    names: Option<Regex>,
    /// Lowercase name -> its replacement.
    to: HashMap<String, String>,
    /// Lowercase replacement -> how many places.
    counts: HashMap<String, usize>,
    ids: HashMap<u64, u64>,
    /// The ids as text, longest first, with no digit on either side.
    id_text: Option<Regex>,
}

impl Rewrite {
    fn new(found: &Found) -> Self {
        let mut to = HashMap::new();
        let mut next = 2;
        for name in &found.names {
            let replacement = if found
                .account
                .as_deref()
                .is_some_and(|a| a.eq_ignore_ascii_case(name))
            {
                ACCOUNT.to_owned()
            } else {
                let r = format!("Player{next}");
                next += 1;
                r
            };
            to.insert(name.to_lowercase(), replacement);
        }
        let names = (!found.names.is_empty()).then(|| name_regex(&found.names));
        let ids: HashMap<u64, u64> = found
            .ids
            .iter()
            .enumerate()
            .map(|(i, id)| (*id, FIRST_ID + i as u64))
            .collect();
        let id_text = (!found.ids.is_empty()).then(|| {
            let mut alts: Vec<String> = found.ids.iter().map(u64::to_string).collect();
            alts.sort_by_key(|a| std::cmp::Reverse(a.len()));
            Regex::new(&format!(r"(^|\D)({})(\D|$)", alts.join("|"))).unwrap()
        });
        Self {
            names,
            to,
            counts: HashMap::new(),
            ids,
            id_text,
        }
    }

    /// A string anywhere in the tape: names, ids, personal fields.
    fn text(&mut self, s: &mut String) {
        if let Some(re) = &self.names {
            let mut counts = Vec::new();
            let replaced = re.replace_all(s, |c: &Captures| {
                let matched = &c[0];
                let to = &self.to[&matched.to_lowercase()];
                counts.push(to.to_lowercase());
                same_case(matched, to)
            });
            if let std::borrow::Cow::Owned(r) = replaced {
                *s = r;
            }
            for c in counts {
                *self.counts.entry(c).or_default() += 1;
            }
        }
        if let Some(re) = &self.id_text {
            // Twice: `%1%2%` shares the `%` between two matches.
            for _ in 0..2 {
                let replaced = re.replace_all(s, |c: &Captures| {
                    let id: u64 = c[2].parse().unwrap_or_default();
                    format!(
                        "{}{}{}",
                        &c[1],
                        self.ids.get(&id).copied().unwrap_or(id),
                        &c[3]
                    )
                });
                if let std::borrow::Cow::Owned(r) = replaced {
                    *s = r;
                }
            }
        }
        personal(s);
    }

    fn value(&mut self, v: &mut Value) {
        match v {
            Value::String(s) => self.text(s),
            Value::Array(items) => items.iter_mut().for_each(|i| self.value(i)),
            Value::Object(map) => {
                let old = std::mem::take(map);
                for (mut key, mut field) in old {
                    self.text(&mut key);
                    self.value(&mut field);
                    map.insert(key, field);
                }
            }
            Value::Number(n) => {
                if let Some(id) = n.as_u64().and_then(|n| self.ids.get(&n).copied()) {
                    *v = Value::from(id);
                }
            }
            _ => {}
        }
    }

    fn line(&mut self, line: &mut Line) {
        match line {
            Line::Call {
                args,
                answer,
                error,
                ..
            } => {
                args.iter_mut().for_each(|a| self.value(a));
                if let Some(answer) = answer {
                    self.text(answer);
                }
                if let Some(error) = error {
                    self.text(error);
                }
            }
            Line::Event { args, .. } => args.iter_mut().for_each(|a| self.value(a)),
            Line::Sent { text, .. } | Line::Received { text, .. } => self.text(text),
            Line::Note { detail, .. } => self.value(detail),
        }
    }
}

/// One pattern for every name: longest first (so `Ann` doesn't take `Anna`'s start), each with a
/// word boundary where it begins or ends with a letter or digit, so a short name doesn't rewrite
/// an item's.
fn name_regex(names: &[String]) -> Regex {
    let mut names: Vec<&String> = names.iter().collect();
    names.sort_by_key(|n| std::cmp::Reverse(n.len()));
    let alts: Vec<String> = names
        .iter()
        .map(|n| {
            let word = |c: Option<char>| c.is_some_and(char::is_alphanumeric);
            format!(
                "{}{}{}",
                if word(n.chars().next()) { r"\b" } else { "" },
                regex::escape(n),
                if word(n.chars().last()) { r"\b" } else { "" },
            )
        })
        .collect();
    Regex::new(&format!("(?i)(?:{})", alts.join("|"))).unwrap()
}

/// `to` in `matched`'s case: all lowercase (`ct`'s keys), all uppercase, or as given.
fn same_case(matched: &str, to: &str) -> String {
    let letters = || matched.chars().filter(|c| c.is_alphabetic());
    if letters().all(char::is_lowercase) {
        to.to_lowercase()
    } else if letters().all(char::is_uppercase) && matched.chars().count() > 1 {
        to.to_uppercase()
    } else {
        to.to_owned()
    }
}

/// `initUserData`'s personal fields, in JSON text: fixed values, written in place (the rest of the
/// text, its keys' order included, stays as it was).
fn personal(s: &mut String) {
    static FIELDS: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#""(sCountry|dCreated)"(\s*:\s*)"[^"]*"|"iAge"(\s*:\s*)"?\d+"?"#).unwrap()
    });
    if !(s.contains("\"sCountry\"") || s.contains("\"dCreated\"") || s.contains("\"iAge\"")) {
        return;
    }
    let replaced = FIELDS.replace_all(s, |c: &Captures| match c.get(1).map(|m| m.as_str()) {
        Some("sCountry") => format!(r#""sCountry"{}"XX""#, &c[2]),
        Some(_) => format!(r#""dCreated"{}"2000-01-01T00:00:00""#, &c[2]),
        None => format!(r#""iAge"{}0"#, &c[3]),
    });
    if let std::borrow::Cow::Owned(r) = replaced {
        *s = r;
    }
}

/// Scrubs a tape in memory: see the module's documentation. Scrubbing a scrubbed tape changes
/// nothing.
pub fn scrub(mut tape: Tape) -> (Tape, Report) {
    let found = collect(&tape);
    let mut rewrite = Rewrite::new(&found);
    redact::value(&mut tape.header.options);
    rewrite.value(&mut tape.header.options);
    for line in &mut tape.lines {
        redact::line(line);
        rewrite.line(line);
    }
    tape.header.scrubbed = true;
    let mut renamed: Vec<(String, usize)> = found
        .names
        .iter()
        .map(|n| {
            let to = rewrite.to[&n.to_lowercase()].clone();
            let count = rewrite.counts.get(&to.to_lowercase()).copied().unwrap_or(0);
            (to, count)
        })
        .collect();
    renamed.dedup();
    let report = Report {
        renamed,
        ids: found.ids.len(),
        lines: tape.lines.len(),
    };
    (tape, report)
}

/// Scrubs the tape at `input` into a new file at `output` (never written over), then reads it back
/// and checks that no name, number or secret it replaced is left; if one is, it removes the file.
pub fn scrub_file(input: &Path, output: &Path) -> Result<Report, ScrubError> {
    let tape = super::read(input)?;
    let found = collect(&tape);
    let (scrubbed, report) = scrub(tape);
    super::write(output, &scrubbed)?;
    if let Err(left) = check(output, &found) {
        let _ = std::fs::remove_file(output);
        return Err(left);
    }
    Ok(report)
}

/// Whether the tape at `path`, decompressed, holds anything of `found`.
fn check(path: &Path, found: &Found) -> Result<(), ScrubError> {
    let mut text = String::new();
    flate2::read::MultiGzDecoder::new(std::fs::File::open(path)?).read_to_string(&mut text)?;
    let lower = text.to_lowercase();
    let rewrite = Rewrite::new(found);
    // An original name that is also a replacement ("Questwalker" scrubbed again) may stay.
    let is_replacement = |m: &str| rewrite.to.values().any(|r| r.eq_ignore_ascii_case(m));
    if let Some(re) = &rewrite.names
        && let Some(m) = re.find_iter(&text).find(|m| !is_replacement(m.as_str()))
    {
        let to = rewrite.to.get(&m.as_str().to_lowercase()).cloned();
        return Err(ScrubError::Left(format!(
            "the real name of {}",
            to.unwrap_or_default()
        )));
    }
    for id in &found.ids {
        let replacement = rewrite.ids.get(id).copied().unwrap_or(*id);
        let unchanged = replacement == *id;
        let re = Regex::new(&format!(r"(^|\D){id}(\D|$)")).unwrap();
        if !unchanged && re.is_match(&text) {
            return Err(ScrubError::Left(format!(
                "an account number (renumbered {replacement})"
            )));
        }
    }
    if found
        .secrets
        .iter()
        .any(|s| lower.contains(&s.to_lowercase()))
    {
        return Err(ScrubError::Left("an email or address field's value".into()));
    }
    Ok(())
}

impl Report {
    /// Lines for a person: what was replaced, without what it was.
    pub fn summary(&self) -> String {
        let mut out = format!("{} lines", self.lines);
        for (to, count) in &self.renamed {
            out += &format!("\n  a player renamed {to}, in {count} places");
        }
        out += &format!("\n  {} account numbers renumbered", self.ids);
        out
    }
}
