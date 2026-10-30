//! Reporting to another machine's monitor (Options > Application > Report To, `--report-to`):
//! this machine's tabs and Army clients show up on its page, and its commands run here.
//!
//! A "Turu Monitor Report" thread posts a [`Report`] to `<url>/api/ingest` with the push key.
//! The monitor holds its reply up to 4 s for a command, so one report follows another about
//! every 5 s and a command arrives at once. Each command runs on a thread of its own; its result
//! goes with the next report, sent as soon as it's ready. Reporting counts as watching, so this
//! machine's samplers keep reading the game.

use super::hub::{CommandResult, Hub, RemoteCommand, Report, ReportReply, allowed_route};
use super::touch;
use crossbeam_channel::{Receiver, Sender, unbounded};
use parking_lot::Mutex;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

/// How long to wait for more results after a reply, before the next report.
const BETWEEN: Duration = Duration::from_secs(1);
/// After a failed report.
const RETRY: Duration = Duration::from_secs(5);
/// A report and its held reply.
const TIMEOUT: Duration = Duration::from_secs(15);

/// Where to report and how.
#[derive(Debug, Clone, PartialEq)]
pub struct ReportTo {
    /// The monitor's address, `http(s)://host[:port]`.
    pub url: String,
    pub push_key: String,
    /// This machine's name on that page.
    pub machine: String,
    /// Run the commands it sends (Accept Commands).
    pub accept_commands: bool,
    pub version: String,
}

/// How reporting goes, for Options > Application.
#[derive(Debug, Clone, Default)]
pub struct ReportStatus {
    /// ms since 1970 of the last report the monitor took.
    pub last_ok: Option<u64>,
    pub error: Option<String>,
}

/// The running reporter; stops when dropped.
pub struct Reporter {
    stop: Arc<AtomicBool>,
    status: Arc<Mutex<ReportStatus>>,
    thread: Option<JoinHandle<()>>,
}

impl Reporter {
    pub fn start(hub: Arc<Hub>, to: ReportTo) -> Result<Self, String> {
        let url = to.url.trim().trim_end_matches('/');
        let parsed = reqwest::Url::parse(url).map_err(|e| format!("Report To: {e}"))?;
        if !matches!(parsed.scheme(), "http" | "https") || parsed.host().is_none() {
            return Err("Report To must be an http:// or https:// address".into());
        }
        if to.push_key.trim().len() < 16 {
            return Err("the push key is missing (the monitor's Copy Report Settings)".into());
        }
        let client = reqwest::blocking::Client::builder()
            .timeout(TIMEOUT)
            .build()
            .map_err(|e| e.to_string())?;
        let stop = Arc::new(AtomicBool::new(false));
        let status = Arc::new(Mutex::new(ReportStatus::default()));
        let endpoint = format!("{url}/api/ingest");
        let thread = {
            let (stop, status) = (stop.clone(), status.clone());
            std::thread::Builder::new()
                .name("Turu Monitor Report".into())
                .spawn(move || run(&hub, &to, &client, &endpoint, &stop, &status))
                .map_err(|e| e.to_string())?
        };
        Ok(Self {
            stop,
            status,
            thread: Some(thread),
        })
    }

    pub fn status(&self) -> ReportStatus {
        self.status.lock().clone()
    }
}

impl Drop for Reporter {
    /// Waits for the report in flight (up to its timeout).
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn run(
    hub: &Arc<Hub>,
    to: &ReportTo,
    client: &reqwest::blocking::Client,
    endpoint: &str,
    stop: &AtomicBool,
    status: &Mutex<ReportStatus>,
) {
    let (results_tx, results_rx) = unbounded::<CommandResult>();
    // The event numbers the monitor has: sent in a report it took.
    let mut sent: HashMap<String, u64> = HashMap::new();
    let mut results: Vec<CommandResult> = Vec::new();
    while !stop.load(Ordering::Relaxed) {
        touch();
        results.extend(results_rx.try_iter());
        let bots = hub.export(&sent);
        let report = Report {
            machine: to.machine.clone(),
            version: to.version.clone(),
            bots,
            results: std::mem::take(&mut results),
        };
        let reply = client
            .post(endpoint)
            .bearer_auth(to.push_key.trim())
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(serde_json::to_vec(&report).unwrap_or_default())
            .send()
            .map_err(|e| e.without_url().to_string())
            .and_then(|response| {
                let code = response.status();
                let text = response.text().map_err(|e| e.to_string())?;
                if code.is_success() {
                    serde_json::from_str::<ReportReply>(&text).map_err(|e| e.to_string())
                } else {
                    let body: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
                    let message = body["error"].as_str().unwrap_or_default().to_owned();
                    Err(format!("{code} {message}").trim().to_owned())
                }
            });
        match reply {
            Ok(reply) => {
                for bot in &report.bots {
                    sent.insert(bot.id.clone(), bot.next);
                }
                *status.lock() = ReportStatus {
                    last_ok: Some(super::snapshot::now_ms()),
                    error: None,
                };
                for command in reply.commands {
                    execute(hub, to.accept_commands, command, results_tx.clone());
                }
                wait(&results_rx, &mut results, BETWEEN, stop);
            }
            Err(e) => {
                // Send them again with the next report.
                results = report.results;
                status.lock().error = Some(e);
                wait(&results_rx, &mut results, RETRY, stop);
            }
        }
    }
}

/// Waits up to `limit` for a result (which is sent at once) or for Stop.
fn wait(
    rx: &Receiver<CommandResult>,
    results: &mut Vec<CommandResult>,
    limit: Duration,
    stop: &AtomicBool,
) {
    let step = Duration::from_millis(250);
    let mut waited = Duration::ZERO;
    while waited < limit && !stop.load(Ordering::Relaxed) {
        if let Ok(result) = rx.recv_timeout(step) {
            results.push(result);
            return;
        }
        waited += step;
    }
}

/// Runs a command from the monitor on a thread of its own; its result goes to `done`.
fn execute(hub: &Arc<Hub>, accept: bool, command: RemoteCommand, done: Sender<CommandResult>) {
    let refuse = |status: u16, message: &str| CommandResult {
        seq: command.seq,
        status,
        body: json!({ "error": message }),
    };
    // Logs are reading, not a command: they come through with Accept Commands off.
    if !accept && command.post {
        let _ = done.send(refuse(403, "this machine doesn't accept commands"));
        return;
    }
    if !allowed_route(command.post, &command.route) {
        let _ = done.send(refuse(403, "not a route the monitor may use"));
        return;
    }
    let Some(control) = hub.control(&command.bot) else {
        let _ = done.send(refuse(404, "that bot isn't running"));
        return;
    };
    let hub = hub.clone();
    let spawned = std::thread::Builder::new()
        .name("Turu Monitor Command".into())
        .spawn(move || {
            let (status, body) = hub.run(control, command.post, &command.route);
            let _ = done.send(CommandResult {
                seq: command.seq,
                status,
                body,
            });
        });
    if let Err(e) = spawned {
        tracing::warn!("Couldn't run a monitor command: {e}");
    }
}

/// This computer's name, for the page: `COMPUTERNAME` on Windows, else `hostname`'s answer
/// without `.local`.
pub fn default_machine_name() -> String {
    let name = std::env::var("COMPUTERNAME")
        .ok()
        .filter(|n| !n.trim().is_empty())
        .or_else(|| {
            let output = std::process::Command::new("hostname").output().ok()?;
            Some(String::from_utf8_lossy(&output.stdout).trim().to_owned())
        })
        .unwrap_or_default();
    super::hub::machine_name(name.strip_suffix(".local").unwrap_or(&name))
}

/// Plain `http://` to an address that isn't this computer, a private network or Tailscale's:
/// the reports would cross the internet unencrypted.
pub fn insecure(url: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(url.trim()) else {
        return false;
    };
    if url.scheme() != "http" {
        return false;
    }
    let Some(host) = url.host_str() else {
        return false;
    };
    let host = host
        .trim_start_matches('[')
        .trim_end_matches(']')
        .to_ascii_lowercase();
    match host.parse::<std::net::IpAddr>() {
        Ok(std::net::IpAddr::V4(ip)) => {
            let [a, b, ..] = ip.octets();
            // 100.64.0.0/10: Tailscale's addresses.
            let tailscale = a == 100 && (64..128).contains(&b);
            !(ip.is_loopback() || ip.is_private() || ip.is_link_local() || tailscale)
        }
        Ok(std::net::IpAddr::V6(ip)) => !(ip.is_loopback() || ip.is_unique_local()),
        Err(_) => {
            !(host == "localhost"
                || host.ends_with(".ts.net")
                || host.ends_with(".local")
                || !host.contains('.'))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn warns_about_plain_http_across_the_internet() {
        assert!(insecure("http://203.0.113.5:8790"));
        assert!(insecure("http://monitor.example.com"));
        assert!(!insecure("https://monitor.example.com"));
        assert!(!insecure("http://100.101.102.103:8790"));
        assert!(!insecure("http://192.168.1.20:8790"));
        assert!(!insecure("http://my-pc.tail1234.ts.net"));
        assert!(!insecure("http://my-pc:8790"));
        assert!(!insecure("http://127.0.0.1:8790"));
    }

    #[test]
    fn names_machines() {
        assert!(!default_machine_name().is_empty());
        assert_eq!(super::super::hub::machine_name("  vps/1 "), "vps-1");
        assert_eq!(super::super::hub::machine_name(""), "remote");
    }

    #[test]
    fn checks_the_address_and_key() {
        let hub = Hub::new(None);
        let to = |url: &str, key: &str| ReportTo {
            url: url.into(),
            push_key: key.into(),
            machine: "vps".into(),
            accept_commands: true,
            version: "test".into(),
        };
        let key = "0123456789abcdef0123456789abcdef";
        assert!(Reporter::start(hub.clone(), to("ftp://x", key)).is_err());
        assert!(Reporter::start(hub.clone(), to("not a url", key)).is_err());
        assert!(Reporter::start(hub, to("http://127.0.0.1:1", "short")).is_err());
    }
}
