//! Turu: what a shell and a client say to each other (docs/TAB_PROCESSES_PROPOSAL.md, M2). The
//! shell owns the window; each client is a process running one tab's game and bot, drawing the
//! whole tab (the game and Turu's UI) for the shell to show, and taking the shell's input.
//!
//! The client connects to the shell's loopback address and says [`ToShell::Hello`] with the token
//! the shell started it with (`TURU_SHELL_TOKEN`), as Army's clients get theirs. Then each side
//! sends messages: a 4-byte big-endian length and that many bytes of JSON, tagged by `type`; a
//! [`ToShell::Frame`] is followed by its `len` bytes of pixels (BGRA, rows of `width * 4`).
//! Messages a side doesn't know come back as `Unknown` and are ignored, so either side can be
//! newer.
//!
//! Nothing here needs Ruffle or a window: [`serve`] reads the shell's messages into a
//! [`ClientEnd`] (the real one is the client's engine and screen, `crate::client_screen`; tests use
//! a recorder), and [`FrameSender`] sends frames newest-first, dropping those the shell is too slow
//! for.

use serde::{Deserialize, Serialize};
use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, Condvar, Mutex};

/// The token's environment variable.
pub const TOKEN_ENV: &str = "TURU_SHELL_TOKEN";
/// The longest JSON message taken (a frame's pixels aren't counted).
const MAX_MESSAGE: u32 = 1 << 20;
/// The largest frame taken: 8K by 8K BGRA.
const MAX_FRAME: u64 = 8192 * 8192 * 4;

/// The shell to the client.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToClient {
    /// The tab is on screen, `width` by `height` pixels at `scale` pixels per point: draw it
    /// and send frames.
    Show { width: u32, height: u32, scale: f32 },
    /// Another tab is on screen: no more frames.
    Hide,
    /// The mouse, in pixels of the tab.
    Pointer {
        kind: PointerKind,
        button: Button,
        x: f32,
        y: f32,
    },
    /// The wheel or a trackpad, in lines (`dx`, `dy`, positive down and right), at (`x`, `y`).
    Wheel { dx: f32, dy: f32, x: f32, y: f32 },
    /// A key, by egui's name for it (`egui::Key::name`: "Enter", "A", "ArrowLeft", ...).
    Key {
        key: String,
        pressed: bool,
        mods: Mods,
    },
    /// Text typed (a character, or what an input method committed).
    Text { text: String },
    /// The window gained or lost the keyboard focus.
    Focus { focused: bool },
    /// The shell's tabs, for the client's tab strip (M2 defines it; the client doesn't draw it
    /// yet).
    Tabs { tabs: Vec<TabInfo>, shown: u32 },
    /// The answer to a request the client made (a file dialog, say): its `id` and what came back.
    Answer { id: u32, value: Option<String> },
    /// Debugging: write the next frame drawn to `path` as a PNG.
    Dump { path: String },
    /// A message this client doesn't know.
    #[serde(other)]
    Unknown,
}

/// The client to the shell.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToShell {
    /// The first message: the shell's token, and who this is.
    Hello {
        token: String,
        pid: u32,
        version: String,
    },
    /// A picture of the tab, its `len` bytes of BGRA pixels right after this message.
    Frame {
        width: u32,
        height: u32,
        seq: u64,
        len: u64,
    },
    /// `Platform::open_url`: the shell opens it (in the browser).
    OpenUrl { url: String },
    /// The client's tab strip asked for another tab.
    SelectTab { id: u32 },
    /// A message this shell doesn't know.
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PointerKind {
    Move,
    Down,
    Up,
    /// The mouse left the tab.
    Leave,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Button {
    #[default]
    Left,
    Right,
    Middle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Mods {
    #[serde(default)]
    pub shift: bool,
    #[serde(default)]
    pub ctrl: bool,
    #[serde(default)]
    pub alt: bool,
    /// Cmd on a Mac, Ctrl elsewhere (egui's `command`).
    #[serde(default)]
    pub command: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TabInfo {
    pub id: u32,
    pub label: String,
}

/// Writes `message`, then `payload` (a frame's pixels) if any.
pub fn write<T: Serialize>(
    out: &mut impl Write,
    message: &T,
    payload: Option<&[u8]>,
) -> io::Result<()> {
    let json = serde_json::to_vec(message).map_err(io::Error::other)?;
    let len = u32::try_from(json.len()).map_err(io::Error::other)?;
    out.write_all(&len.to_be_bytes())?;
    out.write_all(&json)?;
    if let Some(payload) = payload {
        out.write_all(payload)?;
    }
    out.flush()
}

/// Reads one message; `None` at the end of the stream. A message that isn't JSON at all is an
/// error; one of a type this side doesn't know is `Unknown`.
pub fn read<T: for<'de> Deserialize<'de>>(input: &mut impl Read) -> io::Result<Option<T>> {
    let mut len = [0; 4];
    match input.read_exact(&mut len) {
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        result => result?,
    }
    let len = u32::from_be_bytes(len);
    if len > MAX_MESSAGE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("a message of {len} bytes"),
        ));
    }
    let mut json = vec![0; len as usize];
    input.read_exact(&mut json)?;
    serde_json::from_slice(&json)
        .map(Some)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

/// Reads a frame's pixels after its [`ToShell::Frame`].
pub fn read_payload(input: &mut impl Read, len: u64) -> io::Result<Vec<u8>> {
    if len > MAX_FRAME {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("a frame of {len} bytes"),
        ));
    }
    let mut pixels = vec![0; len as usize];
    input.read_exact(&mut pixels)?;
    Ok(pixels)
}

/// What a client does with the shell's messages.
pub trait ClientEnd {
    fn handle(&mut self, message: ToClient);
}

/// Connects to the shell at `address`, says hello with `token`, and returns the stream (a clone
/// for reading, one for writing).
pub fn connect(address: &str, token: &str) -> io::Result<(TcpStream, TcpStream)> {
    let stream = TcpStream::connect(address)?;
    stream.set_nodelay(true)?;
    let mut writer = stream.try_clone()?;
    write(
        &mut writer,
        &ToShell::Hello {
            token: token.to_owned(),
            pid: std::process::id(),
            version: crate::turu::VERSION.to_owned(),
        },
        None,
    )?;
    Ok((stream, writer))
}

/// Hands each of the shell's messages to `client` until the shell closes the stream (Ok) or it
/// fails. Unknown messages are skipped.
pub fn serve(mut input: impl Read, client: &mut impl ClientEnd) -> io::Result<()> {
    while let Some(message) = read::<ToClient>(&mut input)? {
        if message != ToClient::Unknown {
            client.handle(message);
        }
    }
    Ok(())
}

/// A frame waiting to go out.
struct Pending {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

struct Outbox {
    frame: Option<Pending>,
    messages: Vec<ToShell>,
    closed: bool,
}

/// Sends the client's messages and frames to the shell on a thread of its own, so the engine
/// never waits on the socket. Frames are newest-wins: one not yet sent is replaced by the next.
#[derive(Clone)]
pub struct FrameSender {
    shared: Arc<(Mutex<Outbox>, Condvar)>,
}

impl FrameSender {
    /// Starts the sending thread on `out`. It ends when `out` fails or [`Self::close`] is called.
    pub fn start(mut out: impl Write + Send + 'static) -> Self {
        let shared = Arc::new((
            Mutex::new(Outbox {
                frame: None,
                messages: Vec::new(),
                closed: false,
            }),
            Condvar::new(),
        ));
        let thread_shared = shared.clone();
        let spawned = std::thread::Builder::new()
            .name("Turu Shell Out".into())
            .spawn(move || {
                let (lock, wake) = &*thread_shared;
                let mut seq = 0;
                loop {
                    let (frame, messages) = {
                        let mut outbox = lock.lock().unwrap_or_else(|e| e.into_inner());
                        while outbox.frame.is_none() && outbox.messages.is_empty() && !outbox.closed
                        {
                            outbox = wake.wait(outbox).unwrap_or_else(|e| e.into_inner());
                        }
                        if outbox.closed {
                            return;
                        }
                        (outbox.frame.take(), std::mem::take(&mut outbox.messages))
                    };
                    let sent = messages
                        .iter()
                        .try_for_each(|message| write(&mut out, message, None))
                        .and_then(|()| {
                            let Some(frame) = frame else {
                                return Ok(());
                            };
                            seq += 1;
                            let header = ToShell::Frame {
                                width: frame.width,
                                height: frame.height,
                                seq,
                                len: frame.pixels.len() as u64,
                            };
                            write(&mut out, &header, Some(&frame.pixels))
                        });
                    if let Err(e) = sent {
                        tracing::info!("The shell went away ({e}): no more frames");
                        lock.lock().unwrap_or_else(|e| e.into_inner()).closed = true;
                        return;
                    }
                }
            });
        if let Err(e) = spawned {
            tracing::error!("Couldn't start sending to the shell: {e}");
        }
        Self { shared }
    }

    /// Queues a frame, replacing one not sent yet.
    pub fn frame(&self, width: u32, height: u32, pixels: Vec<u8>) {
        self.push(|outbox| {
            outbox.frame = Some(Pending {
                width,
                height,
                pixels,
            });
        });
    }

    /// Queues a message (sent before the next frame).
    pub fn send(&self, message: ToShell) {
        self.push(|outbox| outbox.messages.push(message));
    }

    /// Stops the sending thread.
    pub fn close(&self) {
        self.push(|outbox| outbox.closed = true);
    }

    /// Whether the shell is gone (a send failed) or [`Self::close`] was called.
    pub fn is_closed(&self) -> bool {
        self.shared
            .0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .closed
    }

    fn push(&self, change: impl FnOnce(&mut Outbox)) {
        let (lock, wake) = &*self.shared;
        change(&mut lock.lock().unwrap_or_else(|e| e.into_inner()));
        wake.notify_one();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::time::Duration;

    #[test]
    fn messages_go_both_ways_unchanged() {
        let to_client = [
            ToClient::Show {
                width: 1280,
                height: 720,
                scale: 2.0,
            },
            ToClient::Pointer {
                kind: PointerKind::Down,
                button: Button::Left,
                x: 10.5,
                y: 20.0,
            },
            ToClient::Key {
                key: "Enter".into(),
                pressed: true,
                mods: Mods {
                    shift: true,
                    ..Mods::default()
                },
            },
            ToClient::Tabs {
                tabs: vec![TabInfo {
                    id: 1,
                    label: "Tab 1".into(),
                }],
                shown: 1,
            },
            ToClient::Hide,
        ];
        let mut bytes = Vec::new();
        for message in &to_client {
            write(&mut bytes, message, None).unwrap();
        }
        let mut input = bytes.as_slice();
        for message in &to_client {
            assert_eq!(
                read::<ToClient>(&mut input).unwrap().as_ref(),
                Some(message)
            );
        }
        assert_eq!(read::<ToClient>(&mut input).unwrap(), None);
    }

    #[test]
    fn a_frame_carries_its_pixels() {
        let pixels = vec![1, 2, 3, 4, 5, 6, 7, 8];
        let header = ToShell::Frame {
            width: 2,
            height: 1,
            seq: 1,
            len: pixels.len() as u64,
        };
        let mut bytes = Vec::new();
        write(&mut bytes, &header, Some(&pixels)).unwrap();
        let mut input = bytes.as_slice();
        let Some(ToShell::Frame { len, .. }) = read::<ToShell>(&mut input).unwrap() else {
            panic!("not a frame");
        };
        assert_eq!(read_payload(&mut input, len).unwrap(), pixels);
    }

    #[test]
    fn unknown_messages_are_skipped() {
        let mut bytes = Vec::new();
        write(
            &mut bytes,
            &serde_json::json!({ "type": "from_the_future", "x": 1 }),
            None,
        )
        .unwrap();
        write(&mut bytes, &ToClient::Hide, None).unwrap();
        let mut recorder = Recorder::default();
        serve(bytes.as_slice(), &mut recorder).unwrap();
        assert_eq!(recorder.0, [ToClient::Hide]);
    }

    #[test]
    fn a_message_too_long_is_refused() {
        let mut bytes = (MAX_MESSAGE + 1).to_be_bytes().to_vec();
        bytes.extend([0; 16]);
        assert!(read::<ToClient>(&mut bytes.as_slice()).is_err());
    }

    #[derive(Default)]
    struct Recorder(Vec<ToClient>);

    impl ClientEnd for Recorder {
        fn handle(&mut self, message: ToClient) {
            self.0.push(message);
        }
    }

    /// A fake shell on loopback drives a client end: hello with the token, then its messages.
    #[test]
    fn a_fake_shell_drives_a_client() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap().to_string();
        let shell = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let hello = read::<ToShell>(&mut stream).unwrap().unwrap();
            let messages = [
                ToClient::Show {
                    width: 800,
                    height: 600,
                    scale: 1.0,
                },
                ToClient::Text { text: "é".into() },
                ToClient::Hide,
            ];
            for message in &messages {
                write(&mut stream, message, None).unwrap();
            }
            (hello, messages)
        });
        let (reader, _writer) = connect(&address, "secret").unwrap();
        let mut recorder = Recorder::default();
        serve(reader, &mut recorder).unwrap();
        let (hello, sent) = shell.join().unwrap();
        let ToShell::Hello { token, pid, .. } = hello else {
            panic!("no hello");
        };
        assert_eq!((token.as_str(), pid), ("secret", std::process::id()));
        assert_eq!(recorder.0, sent);
    }

    /// Frames queued faster than the shell reads are dropped, the newest kept; messages aren't.
    #[test]
    fn frames_are_newest_wins() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let out = TcpStream::connect(address).unwrap();
        let (mut stream, _) = listener.accept().unwrap();
        // A shell that hasn't read anything yet: the first frame may already be on its way; the
        // ones queued behind it collapse into the last.
        let sender = FrameSender::start(out);
        for n in 0..50u8 {
            sender.frame(1, 1, vec![n; 4]);
        }
        sender.send(ToShell::OpenUrl {
            url: "https://example.com".into(),
        });
        sender.frame(1, 1, vec![99; 4]);
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut frames = Vec::new();
        let mut urls = 0;
        while frames.last() != Some(&vec![99; 4]) {
            match read::<ToShell>(&mut stream).unwrap().unwrap() {
                ToShell::Frame { len, .. } => frames.push(read_payload(&mut stream, len).unwrap()),
                ToShell::OpenUrl { .. } => urls += 1,
                other => panic!("unexpected {other:?}"),
            }
        }
        sender.close();
        assert_eq!(urls, 1);
        assert!(frames.len() < 50, "{} frames sent of 51", frames.len());
    }
}
