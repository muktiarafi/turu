//! Diagnostics > Interceptor: the game socket's packets both ways, and interceptors that edit
//! or drop them (VibeSkua `ICaptureProxy`, `CaptureProxy.cs`, `IInterceptor`, `MessageInfo`).
//!
//! VibeSkua runs a TCP proxy on `127.0.0.1:5588` and logs the game in again through it. Turu
//! sees the socket in process instead: the app wraps Ruffle's navigator (`app/src/capture.rs`)
//! and passes every chunk the socket reads or writes through a [`Stream`], which cuts it into
//! NUL-terminated packets and hands them to [`CaptureProxy::process`]. No reconnect is needed,
//! and Disconnect only stops the capture (VibeSkua's closes the game's connection).
//!
//! It runs on the player's thread, inside Ruffle's socket tasks: interceptors must be quick and
//! must not call into Flash.

use flate2::Compression;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use parking_lot::{Mutex, RwLock};
use std::borrow::Cow;
use std::collections::VecDeque;
use std::io::{Read, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// The captured packets kept for the Interceptor window (VibeSkua keeps 1000).
pub const LOG_CAPACITY: usize = 1000;

/// A bigger inflated packet is dropped rather than forwarded (VibeSkua's local limit).
const MAX_INFLATED: u64 = 16 * 1024 * 1024;

/// A packet on its way (VibeSkua `MessageInfo`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MessageInfo {
    /// The packet's text, inflated when the server compressed it. Change it to edit the packet.
    pub content: String,
    /// Set it to false to drop the packet.
    pub send: bool,
}

impl MessageInfo {
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            send: true,
        }
    }
}

/// Sees, edits or drops packets between the game and the server (VibeSkua `IInterceptor`).
pub trait Interceptor: Send + Sync {
    /// Lower priorities run first.
    fn priority(&self) -> i32 {
        0
    }

    /// `outbound` is true for a packet from the game to the server.
    fn intercept(&self, message: &mut MessageInfo, outbound: bool);
}

/// A logged packet (VibeSkua `InterceptedPacketViewModel`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InterceptedPacket {
    pub packet: String,
    /// `Some(true)` sent to the server, `Some(false)` from it, `None` dropped.
    pub outbound: Option<bool>,
}

impl InterceptedPacket {
    /// `OUT`, `IN` or `BLOCK`.
    pub fn direction(&self) -> &'static str {
        match self.outbound {
            Some(true) => "OUT",
            Some(false) => "IN",
            None => "BLOCK",
        }
    }

    pub fn is_blocked(&self) -> bool {
        self.outbound.is_none()
    }
}

/// Export's text: one `[OUT] packet` line per packet, line breaks inside a packet written as
/// `\r` and `\n` (VibeSkua `ExportPackets`).
pub fn export_text(packets: &[InterceptedPacket]) -> String {
    let mut text = String::new();
    for p in packets {
        let packet = p.packet.replace('\r', "\\r").replace('\n', "\\n");
        text.push_str(&format!("[{}] {packet}", p.direction()));
        text.push_str(if cfg!(windows) { "\r\n" } else { "\n" });
    }
    text
}

/// The capture of one client's game socket (VibeSkua `CaptureProxy`).
pub struct CaptureProxy {
    running: AtomicBool,
    logging: AtomicBool,
    interceptors: RwLock<Vec<Arc<dyn Interceptor>>>,
    log: Mutex<VecDeque<InterceptedPacket>>,
    /// Packets logged so far: the next one's number.
    logged: AtomicU64,
    version: AtomicU64,
}

impl Default for CaptureProxy {
    fn default() -> Self {
        Self {
            running: AtomicBool::new(false),
            // VibeSkua's window turns logging on when it is made.
            logging: AtomicBool::new(true),
            interceptors: RwLock::new(Vec::new()),
            log: Mutex::new(VecDeque::new()),
            logged: AtomicU64::new(0),
            version: AtomicU64::new(0),
        }
    }
}

impl CaptureProxy {
    /// Whether packets go through the interceptors (the window's Connected).
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Acquire)
    }

    /// Connect: packets go through the interceptors from the next one on.
    pub fn start(&self) {
        self.running.store(true, Ordering::Release);
        self.version.fetch_add(1, Ordering::Relaxed);
    }

    /// Disconnect: packets pass untouched. The game stays connected.
    pub fn stop(&self) {
        self.running.store(false, Ordering::Release);
        self.version.fetch_add(1, Ordering::Relaxed);
    }

    /// "Log packets": whether captured packets are added to [`Self::packets`].
    pub fn is_logging(&self) -> bool {
        self.logging.load(Ordering::Relaxed)
    }

    pub fn set_logging(&self, on: bool) {
        self.logging.store(on, Ordering::Relaxed);
    }

    /// Adds an interceptor; it runs after those with a lower or equal priority.
    pub fn add_interceptor(&self, interceptor: Arc<dyn Interceptor>) {
        let mut list = self.interceptors.write();
        list.push(interceptor);
        list.sort_by_key(|i| i.priority());
    }

    /// Removes an interceptor added with [`Self::add_interceptor`].
    pub fn remove_interceptor(&self, interceptor: &Arc<dyn Interceptor>) {
        self.interceptors
            .write()
            .retain(|i| !Arc::ptr_eq(i, interceptor));
    }

    /// The logged packets, oldest first.
    pub fn packets(&self) -> Vec<InterceptedPacket> {
        self.log.lock().iter().cloned().collect()
    }

    /// [`Self::packets`] and the first one's number (packets are numbered as they are logged,
    /// so a list can keep its selection while old packets drop off).
    pub fn snapshot(&self) -> (u64, Vec<InterceptedPacket>) {
        let log = self.log.lock();
        let first = self.logged.load(Ordering::Relaxed) - log.len() as u64;
        (first, log.iter().cloned().collect())
    }

    /// Bumped when the log, the state or the logging changes.
    pub fn version(&self) -> u64 {
        self.version.load(Ordering::Relaxed)
    }

    pub fn clear(&self) {
        self.log.lock().clear();
        self.version.fetch_add(1, Ordering::Relaxed);
    }

    /// One packet without its NUL: what to send on (`None`: dropped). While the capture runs
    /// the interceptors see it (an inbound packet inflated when the server compressed it) and
    /// it is logged after them, as VibeSkua's logger runs last.
    pub fn process<'a>(&self, data: &'a [u8], outbound: bool) -> Option<Cow<'a, [u8]>> {
        if !self.is_running() {
            return Some(Cow::Borrowed(data));
        }
        let logging = self.is_logging();
        let interceptors = self.interceptors.read().clone();
        if interceptors.is_empty() && !logging {
            return Some(Cow::Borrowed(data));
        }
        let raw = String::from_utf8_lossy(data);
        let inflated = if outbound { None } else { unpack(&raw) };
        let compressed = inflated.is_some();
        let content = inflated.unwrap_or_else(|| raw.into_owned());
        let mut message = MessageInfo::new(content.clone());
        for interceptor in &interceptors {
            interceptor.intercept(&mut message, outbound);
        }
        if logging {
            self.log_packet(InterceptedPacket {
                packet: message.content.clone(),
                outbound: message.send.then_some(outbound),
            });
        }
        if !message.send {
            None
        } else if message.content == content {
            Some(Cow::Borrowed(data))
        } else if compressed {
            Some(Cow::Owned(pack(&message.content)))
        } else {
            Some(Cow::Owned(message.content.into_bytes()))
        }
    }

    fn log_packet(&self, packet: InterceptedPacket) {
        let mut log = self.log.lock();
        if log.len() >= LOG_CAPACITY {
            log.pop_front();
        }
        log.push_back(packet);
        self.logged.fetch_add(1, Ordering::Relaxed);
        self.version.fetch_add(1, Ordering::Relaxed);
    }
}

/// One direction of a socket: cuts what it reads or writes into packets for the capture.
///
/// While the capture is stopped, chunks pass as they are (a binary socket isn't touched). When
/// it starts in the middle of a packet, that packet's rest passes untouched and the capture
/// begins with the next one.
#[derive(Default)]
pub struct Stream {
    /// A packet whose NUL hasn't come yet (only while the capture runs).
    pending: Vec<u8>,
    /// The last chunk passed while stopped ended inside a packet.
    mid_packet: bool,
}

impl Stream {
    /// What to send on for `chunk`; may be empty.
    pub fn filter(&mut self, capture: &CaptureProxy, chunk: Vec<u8>, outbound: bool) -> Vec<u8> {
        if !capture.is_running() {
            let mut out = std::mem::take(&mut self.pending);
            if out.is_empty() {
                out = chunk;
            } else {
                out.extend_from_slice(&chunk);
            }
            if let Some(&last) = out.last() {
                self.mid_packet = last != 0;
            }
            return out;
        }
        let mut out = Vec::new();
        let mut data = &chunk[..];
        if self.mid_packet {
            match data.iter().position(|&b| b == 0) {
                None => return chunk,
                Some(end) => {
                    out.extend_from_slice(&data[..=end]);
                    data = &data[end + 1..];
                    self.mid_packet = false;
                }
            }
        }
        self.pending.extend_from_slice(data);
        let mut start = 0;
        while let Some(len) = self.pending[start..].iter().position(|&b| b == 0) {
            let packet = &self.pending[start..start + len];
            if packet.is_empty() {
                out.push(0);
            } else if let Some(bytes) = capture.process(packet, outbound) {
                out.extend_from_slice(&bytes);
                out.push(0);
            }
            start += len + 1;
        }
        self.pending.drain(..start);
        out
    }
}

const BASE64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// A compressed server packet: `Z` and zlib data in Base64. Read leniently as the game does
/// (unknown characters count as zero, padding is optional); `None` when it isn't one.
pub fn unpack(message: &str) -> Option<String> {
    let body = message.strip_prefix('Z')?;
    let body = body.trim_end_matches('=');
    if body.len() * 3 / 4 < 8 || body.chars().any(|c| c as u32 > 255) {
        return None;
    }
    let mut bytes = Vec::with_capacity(body.len() * 3 / 4);
    let (mut bits, mut count) = (0u32, 0u32);
    for b in body.bytes() {
        let value = BASE64.iter().position(|&c| c == b).unwrap_or(0) as u32;
        bits = (bits << 6) | value;
        count += 6;
        if count >= 8 {
            count -= 8;
            bytes.push((bits >> count) as u8);
        }
    }
    let mut inflated = Vec::new();
    ZlibDecoder::new(&bytes[..])
        .take(MAX_INFLATED + 1)
        .read_to_end(&mut inflated)
        .ok()?;
    if inflated.len() as u64 > MAX_INFLATED {
        return None;
    }
    Some(String::from_utf8_lossy(&inflated).into_owned())
}

/// The reverse of [`unpack`], for an edited compressed packet.
pub fn pack(content: &str) -> Vec<u8> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    let _ = encoder.write_all(content.as_bytes());
    let data = encoder.finish().unwrap_or_default();
    let mut out = Vec::with_capacity(1 + data.len().div_ceil(3) * 4);
    out.push(b'Z');
    for chunk in data.chunks(3) {
        let n = (chunk[0] as u32) << 16
            | (*chunk.get(1).unwrap_or(&0) as u32) << 8
            | *chunk.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(BASE64[(n >> (18 - 6 * i)) as usize & 63]);
            } else {
                out.push(b'=');
            }
        }
    }
    out
}
