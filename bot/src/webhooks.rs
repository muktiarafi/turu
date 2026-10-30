//! Discord and Skua Nexus webhooks (VibeSkua `DiscordWebhookService`): embeds when a script
//! starts, stops or crashes, on relogins and on listed drops, plus periodic health pings with
//! a snapshot of the character. Settings: Options > Application (`AppSettings`).
//!
//! One thread posts everything, at most two messages a second. It reads the game itself for
//! the snapshots, so the threads that report events never wait for Flash or the network.

use crate::Bot;
use crate::api::{inventory, map, player, quests, servers};
use crate::events::BotEvent;
use crate::settings::AppSettings;
use crossbeam_channel::{Receiver, RecvTimeoutError, Sender, unbounded};
use serde_json::{Value, json};
use std::sync::{Arc, Weak};
use std::time::{Duration, Instant};

const GREEN: u32 = 0x00FF00;
const RED: u32 = 0xFF0000;
const ORANGE: u32 = 0xFFA500;
const GREY: u32 = 0x808080;
const GOLD: u32 = 0xFFD700;

/// Something to report.
#[derive(Debug)]
pub enum Notice {
    ScriptStarted {
        script: String,
    },
    /// `error` is the message when the script failed (VibeSkua sends `ScriptErrorMessage`,
    /// then `ScriptStoppedMessage`).
    ScriptStopped {
        script: String,
        error: Option<String>,
    },
    Relogin {
        kicked: bool,
    },
    ItemDropped {
        name: String,
        quantity: i64,
    },
    /// The window's Test buttons.
    Test {
        nexus: bool,
    },
}

pub struct Webhooks {
    tx: Sender<Notice>,
    rx: Receiver<Notice>,
}

impl Default for Webhooks {
    fn default() -> Self {
        let (tx, rx) = unbounded();
        Self { tx, rx }
    }
}

impl Webhooks {
    /// Queues a notice; never blocks.
    pub fn send(&self, notice: Notice) {
        let _ = self.tx.send(notice);
    }
}

/// The script running now, for the stop summary.
struct Session {
    started: Instant,
    gold: i64,
    xp: i64,
}

pub(crate) fn start(bot: &Arc<Bot>) {
    let weak = Arc::downgrade(bot);
    bot.events.subscribe(move |event| {
        let Some(bot) = weak.upgrade() else { return };
        match event {
            BotEvent::ReloginTriggered { was_kicked } => bot.webhooks.send(Notice::Relogin {
                kicked: *was_kicked,
            }),
            BotEvent::ItemDropped { item } => bot.webhooks.send(Notice::ItemDropped {
                name: item.name.clone(),
                quantity: item.quantity,
            }),
            _ => {}
        }
    });
    let rx = bot.webhooks.rx.clone();
    let weak = Arc::downgrade(bot);
    std::thread::Builder::new()
        .name("Turu Webhooks".into())
        .spawn(move || run(weak, rx))
        .expect("couldn't start the webhook thread");
}

fn run(weak: Weak<Bot>, rx: Receiver<Notice>) {
    let Ok(client) = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
    else {
        return;
    };
    let mut session: Option<Session> = None;
    let mut next_ping: Option<Instant> = None;
    loop {
        let received = match next_ping {
            Some(at) => rx.recv_timeout(at.saturating_duration_since(Instant::now())),
            None => rx.recv().map_err(|_| RecvTimeoutError::Disconnected),
        };
        let Some(bot) = weak.upgrade() else { break };
        let settings = bot.settings.read().clone();
        let mut out = Outbox {
            bot: &bot,
            client: &client,
            sent: 0,
        };
        match received {
            Ok(notice) => {
                handle(&mut out, &settings, notice, &mut session, &mut next_ping);
            }
            Err(RecvTimeoutError::Timeout) => {
                next_ping = ping_interval(&settings).map(|every| Instant::now() + every);
                if next_ping.is_some() {
                    out.ping(&settings, "is running flawlessly.");
                }
            }
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
}

/// VibeSkua `StartPingTimer`: pings go to Nexus whenever it's set, to Discord with Health
/// Pings on. `None` when there's nowhere to send them.
fn ping_interval(settings: &AppSettings) -> Option<Duration> {
    let nexus = !settings.nexus_webhook_url.trim().is_empty();
    let discord = discord_pings(settings);
    if !nexus && !discord {
        return None;
    }
    let minutes = match settings.webhook_ping_interval {
        n if n > 0 => n as u64,
        _ if discord => 10,
        _ => 1,
    };
    Some(Duration::from_secs(minutes * 60))
}

fn discord_pings(settings: &AppSettings) -> bool {
    settings.webhook_notify_health_ping && !settings.discord_webhook_url.trim().is_empty()
}

fn handle(
    out: &mut Outbox,
    settings: &AppSettings,
    notice: Notice,
    session: &mut Option<Session>,
    next_ping: &mut Option<Instant>,
) {
    let discord = settings.discord_webhook_url.trim();
    let nexus = settings.nexus_webhook_url.trim();
    if discord.is_empty() && nexus.is_empty() {
        // Nowhere to send anything: don't read the game either.
        *session = None;
        *next_ping = None;
        return;
    }
    match notice {
        Notice::ScriptStarted { script } => {
            let playing = player::playing(out.bot).unwrap_or(false);
            *session = Some(Session {
                started: Instant::now(),
                gold: if playing { gold(out.bot) } else { 0 },
                xp: if playing { xp(out.bot) } else { 0 },
            });
            if settings.webhook_notify_started {
                let text = format!("**{}** has begun execution of **{script}**.", out.name());
                out.broadcast(settings, embed("Script Started", &text, GREEN, None));
            }
            let text = format!("**{}** is running.", out.name());
            if !nexus.is_empty() {
                out.post(
                    nexus,
                    embed("Health Ping", &text, GREY, Some(out.snapshot())),
                );
            } else if discord_pings(settings) {
                out.post(
                    discord,
                    embed("Health Ping", &text, GREY, Some(out.snapshot())),
                );
            }
            *next_ping = ping_interval(settings).map(|every| Instant::now() + every);
        }
        Notice::ScriptStopped { script, error } => {
            let started = session.take();
            *next_ping = None;
            if let Some(error) = error {
                if settings.webhook_notify_crashed {
                    let fields = vec![field("Exception", &format!("```{error}```"), false)];
                    let text = format!("**{}** crashed unexpectedly.", out.name());
                    out.broadcast(settings, embed("Script Error", &text, RED, Some(fields)));
                }
                if !nexus.is_empty() {
                    let text = format!("**{}** crashed.", out.name());
                    out.post(
                        nexus,
                        embed("Health Ping", &text, RED, Some(out.snapshot())),
                    );
                }
            }
            if settings.webhook_notify_stopped
                && let Some(started) = started
                && player::playing(out.bot).unwrap_or(false)
            {
                let gold = gold(out.bot) - started.gold;
                let xp = xp(out.bot) - started.xp;
                let mut fields = Vec::new();
                if gold != 0 {
                    fields.push(field("Gold Earned", &thousands(gold.max(0)), true));
                }
                if xp != 0 {
                    fields.push(field("XP Earned", &thousands(xp.max(0)), true));
                }
                fields.push(field(
                    "Time Elapsed",
                    &clock(started.started.elapsed()),
                    false,
                ));
                let text = format!("**{}** has stopped execution of **{script}**.", out.name());
                out.broadcast(
                    settings,
                    embed("Farming Session Concluded", &text, RED, Some(fields)),
                );
            }
            if !nexus.is_empty() {
                let text = format!("**{}** has stopped.", out.name());
                out.post(
                    nexus,
                    embed("Health Ping", &text, GREY, Some(out.snapshot())),
                );
            }
        }
        Notice::Relogin { kicked } => {
            if settings.webhook_notify_relogged {
                let title = if kicked {
                    "Kicked & Relogging"
                } else {
                    "Disconnected & Relogging"
                };
                let text = format!("**{}** is attempting to reconnect.", out.name());
                out.broadcast(settings, embed(title, &text, ORANGE, None));
            }
        }
        Notice::ItemDropped { name, quantity } => {
            if !settings.webhook_notify_item_drops {
                return;
            }
            let lower = name.to_lowercase();
            let listed = settings
                .webhook_notify_item_drops_list
                .split(',')
                .map(str::trim)
                .any(|wanted| !wanted.is_empty() && lower.contains(&wanted.to_lowercase()));
            if listed {
                // VibeSkua attaches a screenshot; Turu sends the embed alone.
                let wiki = format!("https://aqwwiki.wikidot.com/{}", url_escape(&name));
                let text = format!(
                    "🏆 **Rare Drop!** - **{}** just got **[{name}]({wiki})** x{quantity}!",
                    out.name()
                );
                out.broadcast(settings, json!({ "description": text, "color": GOLD }));
            }
        }
        Notice::Test { nexus: to_nexus } => {
            let (url, title, text) = if to_nexus {
                (
                    nexus,
                    "Skua Nexus Webhook Test Successful!",
                    "Your Turu Nexus Webhooks are perfectly configured.",
                )
            } else {
                (
                    discord,
                    "Discord Webhook Test Successful!",
                    "Your Turu Discord Webhooks are perfectly configured.",
                )
            };
            if url.is_empty() {
                return;
            }
            if out.post(url, embed(title, text, GREEN, Some(out.snapshot()))) {
                out.bot
                    .logs
                    .debug
                    .push(format!("Webhook test sent: {title}"));
            }
        }
    }
}

/// Posts embeds, keeping to two messages a second (VibeSkua's queue).
struct Outbox<'a> {
    bot: &'a Bot,
    client: &'a reqwest::blocking::Client,
    sent: usize,
}

impl Outbox<'_> {
    /// VibeSkua `GetBotName`.
    fn name(&self) -> String {
        if let Ok(name) = player::username(self.bot)
            && !name.trim().is_empty()
            && name != "loginInfo.strUsername"
        {
            return name;
        }
        servers::cached_username(self.bot)
            .ok()
            .flatten()
            .unwrap_or_else(|| "Turu Bot".into())
    }

    /// VibeSkua `SendEmbedAsync`: to Discord and Nexus, whichever are set.
    fn broadcast(&mut self, settings: &AppSettings, embed: Value) {
        for url in [&settings.discord_webhook_url, &settings.nexus_webhook_url] {
            let url = url.trim();
            if !url.is_empty() {
                self.post(url, embed.clone());
            }
        }
    }

    /// VibeSkua `OnPingTimerElapsed`.
    fn ping(&mut self, settings: &AppSettings, status: &str) {
        let text = format!("**{}** {status}", self.name());
        let message = embed("Health Ping", &text, GREY, Some(self.snapshot()));
        if discord_pings(settings) {
            self.post(settings.discord_webhook_url.trim(), message.clone());
        }
        if !settings.nexus_webhook_url.trim().is_empty() {
            self.post(settings.nexus_webhook_url.trim(), message);
        }
    }

    /// VibeSkua `PostJsonToUrlAsync`: up to 3 tries, waiting on 429. True when it went through.
    fn post(&mut self, url: &str, embed: Value) -> bool {
        if self.sent > 0 {
            std::thread::sleep(Duration::from_millis(500));
        }
        self.sent += 1;
        let body = json!({ "embeds": [embed] }).to_string();
        for attempt in 0..3u64 {
            let request = self
                .client
                .post(url)
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .body(body.clone());
            match request.send() {
                Ok(response) if response.status().is_success() => return true,
                Ok(response) if response.status().as_u16() == 429 => {
                    std::thread::sleep(Duration::from_millis(1500 * (attempt + 1)));
                }
                Ok(response) => {
                    self.bot
                        .logs
                        .debug
                        .push(format!("Webhook refused ({}): {url}", response.status()));
                    return false;
                }
                Err(e) => {
                    self.bot.logs.debug.push(format!("Webhook failed: {e}"));
                    return false;
                }
            }
        }
        false
    }

    /// VibeSkua `BuildDashboardSnapshotFields`: character, location, script, equipment, quests
    /// and the Combat > Stats counters.
    fn snapshot(&self) -> Vec<Value> {
        let bot = self.bot;
        let name = self.name();
        if !player::playing(bot).unwrap_or(false) {
            return vec![
                field("Character", &name, true),
                field("Status", "Idle / Not in World", true),
            ];
        }
        let f = &bot.flash;
        let mut fields = vec![field("Character", &name, true)];
        let map_name = map::full_name(bot).unwrap_or_default();
        let map_name = if map_name.is_empty() {
            "Unknown".into()
        } else {
            map_name
        };
        fields.push(field("Map", &map_name, true));
        let room = format!(
            "{}-{} (Room {})",
            player::cell(bot).unwrap_or_default(),
            player::pad(bot).unwrap_or_default(),
            map::room_id(bot).unwrap_or(0)
        );
        fields.push(field("Room", &room, true));
        let script = bot
            .scripts
            .loaded()
            .and_then(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
            .unwrap_or_else(|| "None".into());
        fields.push(field("Script", &script, true));
        let level = f
            .get_or::<i64>("world.myAvatar.dataLeaf.intLevel", 0)
            .unwrap_or(0);
        fields.push(field("Level", &level.to_string(), true));
        fields.push(field("Gold", &thousands(gold(bot)), true));
        let coins = f
            .get_or::<i64>("world.myAvatar.objData.intCoins", 0)
            .unwrap_or(0);
        fields.push(field("AC", &thousands(coins), true));
        let hp = format!(
            "{}/{}",
            player::health(bot).unwrap_or(0),
            player::max_health(bot).unwrap_or(0)
        );
        fields.push(field("HP", &hp, true));
        let mp = format!(
            "{}/{}",
            player::mana(bot).unwrap_or(0),
            player::max_mana(bot).unwrap_or(0)
        );
        fields.push(field("MP", &mp, true));

        let items = inventory::inventory_items(bot).unwrap_or_default();
        let equipped: Vec<&str> = items
            .iter()
            .filter(|i| i.equipped && !i.name.trim().is_empty())
            .map(|i| i.name.as_str())
            .collect();
        let equipment = if equipped.is_empty() {
            "No equipped items reported.".into()
        } else {
            truncate(equipped.join("\n"))
        };
        let active: Vec<String> = quests::tree(bot)
            .unwrap_or_default()
            .into_iter()
            .filter(|q| q.active() && !q.name.trim().is_empty())
            .map(|q| q.name)
            .collect();
        let quests = if active.is_empty() {
            "No active quests reported.".into()
        } else {
            truncate(active.join("\n"))
        };
        fields.push(field("Equipment", &equipment, false));
        fields.push(field("Inventory", &format!("{} items", items.len()), true));
        fields.push(field("Quest", &quests, false));

        let stats = bot.state.lock().stats.clone();
        for (label, value) in [
            ("Kills", stats.kills),
            ("Drops", stats.drops),
            ("Quests", stats.quests_completed),
            ("Deaths", stats.deaths),
            ("Relogins", stats.relogins),
        ] {
            fields.push(field(label, &value.to_string(), true));
        }
        fields
    }
}

fn gold(bot: &Bot) -> i64 {
    bot.flash
        .get_or::<i64>("world.myAvatar.objData.intGold", 0)
        .unwrap_or(0)
}

fn xp(bot: &Bot) -> i64 {
    bot.flash
        .get_or::<i64>("world.myAvatar.objData.intExp", 0)
        .unwrap_or(0)
}

fn embed(title: &str, description: &str, color: u32, fields: Option<Vec<Value>>) -> Value {
    let mut embed = json!({ "title": title, "description": description, "color": color });
    if let Some(fields) = fields {
        embed["fields"] = Value::Array(fields);
    }
    embed
}

fn field(name: &str, value: &str, inline: bool) -> Value {
    json!({ "name": name, "value": value, "inline": inline })
}

/// Discord limits a field to 1024 characters; VibeSkua cuts at 1000.
fn truncate(text: String) -> String {
    match text.char_indices().nth(1000) {
        Some((end, _)) => text[..end].to_owned(),
        None => text,
    }
}

/// C#'s `N0`: 1234567 -> "1,234,567".
fn thousands(n: i64) -> String {
    let digits = n.unsigned_abs().to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    if n < 0 { format!("-{out}") } else { out }
}

/// `hh:mm:ss` (VibeSkua prints the hours of the day part only; Turu prints them all).
fn clock(elapsed: Duration) -> String {
    let s = elapsed.as_secs();
    format!("{:02}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
}

/// `Uri.EscapeDataString`.
fn url_escape(text: &str) -> String {
    let mut out = String::new();
    for b in text.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_like_csharp() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(1_234_567), "1,234,567");
        assert_eq!(thousands(-1000), "-1,000");
        assert_eq!(clock(Duration::from_secs(3725)), "01:02:05");
        assert_eq!(url_escape("Void Aura"), "Void%20Aura");
    }

    #[test]
    fn pings_only_with_somewhere_to_send() {
        let mut settings = AppSettings::default();
        assert_eq!(ping_interval(&settings), None);
        settings.discord_webhook_url = "https://example.invalid/hook".into();
        assert_eq!(ping_interval(&settings), None);
        settings.webhook_notify_health_ping = true;
        assert_eq!(ping_interval(&settings), Some(Duration::from_secs(600)));
        settings.webhook_ping_interval = 0;
        assert_eq!(ping_interval(&settings), Some(Duration::from_secs(600)));
        settings.webhook_notify_health_ping = false;
        settings.nexus_webhook_url = "https://example.invalid/nexus".into();
        assert_eq!(ping_interval(&settings), Some(Duration::from_secs(60)));
    }
}
