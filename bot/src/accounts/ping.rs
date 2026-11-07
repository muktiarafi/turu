//! The servers' latency in Army > Accounts' server list (VibeSkua Manager's
//! `_MeasureServerPings` and `ServerPingTextConverter`): not an ICMP ping but the game's own
//! handshake. A TCP connection to the server's port, SmartFox's version check sent, and the time
//! until the server answers `apiOK`; three tries, their median.

use crate::models::Server;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

/// VibeSkua's version check, as the game client sends it (nul-terminated).
pub const VERSION_CHECK: &[u8] =
    b"<msg t='sys'><body action='verChk' r='0'><ver v='165' /></body></msg>\0";
/// How long one try may take to connect, and then to be answered.
pub const TIMEOUT: Duration = Duration::from_millis(2500);
const SAMPLES: usize = 3;
/// VibeSkua's ping of a server that didn't answer.
pub const FAILED: i64 = 9999;
/// How long a measurement is shown before the list measures again.
const MEASURE_EVERY: Duration = Duration::from_secs(60);

/// One handshake: the milliseconds from the version check sent to `apiOK`, or `None` when the
/// server can't be reached, refuses (`apiKO`) or doesn't answer in `timeout`.
pub fn handshake(addr: SocketAddr, timeout: Duration) -> Option<f64> {
    let mut stream = TcpStream::connect_timeout(&addr, timeout).ok()?;
    let _ = stream.set_nodelay(true);
    stream.write_all(VERSION_CHECK).ok()?;
    stream.flush().ok()?;
    // Timed from the version check: the TCP connect alone can mislead.
    let started = Instant::now();
    let mut message = Vec::new();
    let mut buffer = [0u8; 512];
    loop {
        let left = timeout.checked_sub(started.elapsed())?;
        stream
            .set_read_timeout(Some(left.max(Duration::from_millis(1))))
            .ok()?;
        let read = stream.read(&mut buffer).ok()?;
        if read == 0 {
            return None;
        }
        for &byte in &buffer[..read] {
            if byte != 0 {
                message.push(byte);
                continue;
            }
            let text = String::from_utf8_lossy(&message).to_lowercase();
            message.clear();
            if text.contains("action='apiok'") || text.contains("action=\"apiok\"") {
                return Some(started.elapsed().as_secs_f64() * 1000.0);
            }
            if text.contains("action='apiko'") || text.contains("action=\"apiko\"") {
                return None;
            }
        }
    }
}

/// A server's ping in milliseconds: the median of three handshakes, [`FAILED`] when none
/// worked or the server is offline or has no address.
pub fn measure(server: &Server) -> i64 {
    if !server.online || server.ip.trim().is_empty() || !(1..=65535).contains(&server.port) {
        return FAILED;
    }
    let Some(addr) = (server.ip.trim(), server.port as u16)
        .to_socket_addrs()
        .ok()
        .and_then(|mut a| a.next())
    else {
        return FAILED;
    };
    let mut samples: Vec<f64> = (0..SAMPLES)
        .filter_map(|_| handshake(addr, TIMEOUT))
        .collect();
    if samples.is_empty() {
        return FAILED;
    }
    samples.sort_by(f64::total_cmp);
    samples[samples.len() / 2].round() as i64
}

/// VibeSkua `ServerPingTextConverter`: "Offline", "..." while measuring, "No response", or
/// "42ms". `ping` is `None` before it was measured.
pub fn ping_text(server: &Server, ping: Option<i64>) -> String {
    if !server.online {
        return "Offline".into();
    }
    match ping {
        None => "...".into(),
        Some(FAILED) => "No response".into(),
        Some(ms) => format!("{ms}ms"),
    }
}

/// VibeSkua's `ServerStatusInline`: "Twilly [M] - 42ms - 120/1000" (the players only while
/// online).
pub fn status_line(server: &Server, ping: Option<i64>) -> String {
    let member = if server.upgrade { " [M]" } else { "" };
    let mut line = format!(
        "{}{member} - {}",
        server.name.trim(),
        ping_text(server, ping)
    );
    if server.online {
        line.push_str(&format!(
            " - {}/{}",
            server.player_count, server.max_players
        ));
    }
    line
}

/// The pings measured so far, by server name (lowercase), shared by the windows.
#[derive(Default)]
pub struct Pings {
    pings: Mutex<HashMap<String, i64>>,
    measuring: AtomicBool,
    measured_at: Mutex<Option<Instant>>,
}

impl Pings {
    /// The process's list.
    pub fn global() -> &'static Arc<Pings> {
        static PINGS: OnceLock<Arc<Pings>> = OnceLock::new();
        PINGS.get_or_init(Arc::default)
    }

    /// `server`'s ping, once measured.
    pub fn get(&self, server: &str) -> Option<i64> {
        self.pings
            .lock()
            .get(&server.trim().to_lowercase())
            .copied()
    }

    pub fn is_measuring(&self) -> bool {
        self.measuring.load(Ordering::Acquire)
    }

    /// Measures every server at once on "Turu Ping" threads, unless that is under way or was
    /// done in the last minute (VibeSkua measures when the Manager reads the list).
    pub fn measure_in_background(self: &Arc<Self>, servers: Vec<Server>) {
        if servers.is_empty()
            || self
                .measured_at
                .lock()
                .is_some_and(|t| t.elapsed() < MEASURE_EVERY)
            || self.measuring.swap(true, Ordering::AcqRel)
        {
            return;
        }
        *self.measured_at.lock() = Some(Instant::now());
        let pings = self.clone();
        let spawned = std::thread::Builder::new()
            .name("Turu Ping".into())
            .spawn(move || {
                let handles: Vec<_> = servers
                    .into_iter()
                    .filter_map(|server| {
                        let pings = pings.clone();
                        std::thread::Builder::new()
                            .name("Turu Ping".into())
                            .spawn(move || {
                                let ms = measure(&server);
                                pings
                                    .pings
                                    .lock()
                                    .insert(server.name.trim().to_lowercase(), ms);
                            })
                            .ok()
                    })
                    .collect();
                for handle in handles {
                    let _ = handle.join();
                }
                pings.measuring.store(false, Ordering::Release);
            });
        if spawned.is_err() {
            self.measuring.store(false, Ordering::Release);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    fn server(online: bool) -> Server {
        Server {
            name: "Twilly".into(),
            ip: "127.0.0.1".into(),
            port: 5588,
            online,
            player_count: 120,
            max_players: 1000,
            ..Default::default()
        }
    }

    #[test]
    fn texts_follow_vibeskua() {
        assert_eq!(ping_text(&server(false), Some(40)), "Offline");
        assert_eq!(ping_text(&server(true), None), "...");
        assert_eq!(ping_text(&server(true), Some(FAILED)), "No response");
        assert_eq!(ping_text(&server(true), Some(42)), "42ms");
        assert_eq!(
            status_line(&server(true), Some(42)),
            "Twilly - 42ms - 120/1000"
        );
        let mut member = server(false);
        member.upgrade = true;
        assert_eq!(status_line(&member, None), "Twilly [M] - Offline");
    }

    /// A fake SmartFox server: reads the version check, answers `apiOK` (or `apiKO`).
    fn fake_server(answer: &'static str) -> SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            for stream in listener.incoming().take(1) {
                let mut stream = stream.unwrap();
                let mut got = Vec::new();
                let mut byte = [0u8; 1];
                while stream.read(&mut byte).unwrap_or(0) == 1 && byte[0] != 0 {
                    got.push(byte[0]);
                }
                assert!(String::from_utf8_lossy(&got).contains("action='verChk'"));
                // A policy-like message first: only the answer to the check counts.
                stream
                    .write_all(b"<msg t='sys'><body action='other'/></msg>\0")
                    .unwrap();
                stream.write_all(answer.as_bytes()).unwrap();
            }
        });
        addr
    }

    #[test]
    fn times_the_handshake() {
        let ok = fake_server("<msg t='sys'><body action='apiOK' r='0'></body></msg>\0");
        let ms = handshake(ok, TIMEOUT).expect("apiOK answers");
        assert!(ms < 2500.0, "{ms}");
        let ko = fake_server("<msg t='sys'><body action=\"apiKO\" r='0'></body></msg>\0");
        assert_eq!(handshake(ko, TIMEOUT), None);
    }

    #[test]
    fn unreachable_servers_fail() {
        let mut offline = server(false);
        assert_eq!(measure(&offline), FAILED);
        offline.online = true;
        offline.ip = String::new();
        assert_eq!(measure(&offline), FAILED);
    }
}
