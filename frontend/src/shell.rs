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
/// Set by a shell that takes pictures as shared GPU surfaces instead of pixels: where to hand them
/// over (macOS: the Mach service the shell checked in; Linux: its Unix socket; Windows: "handles",
/// sent as [`ToShell::SharedHandle`], M5). Without it, a client sends pixels.
pub const SURFACES_ENV: &str = "TURU_SHELL_SURFACES";
/// A GPU and backend, as both sides name it: shared surfaces need the same on both.
pub fn adapter_key(backend: &str, vendor: u32, device: u32, name: &str) -> String {
    format!("{backend}:{vendor:x}:{device:x}:{name}")
}
/// How many shared surfaces a client draws into in turn: one the shell shows, one it may still be
/// reading, one being drawn. The shell keeps the newest this many of a client's.
pub const SHARED_POOL: usize = 3;
/// The longest JSON message taken (a frame's pixels aren't counted).
const MAX_MESSAGE: u32 = 1 << 20;
/// The largest frame taken: 8K by 8K BGRA.
const MAX_FRAME: u64 = 8192 * 8192 * 4;

/// The shell to the client.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToClient {
    /// The tab is on screen, `width` by `height` pixels at `scale` pixels per point: draw it
    /// and send frames. In Grid View, `grid` says how (M5).
    Show {
        width: u32,
        height: u32,
        scale: f32,
        #[serde(default)]
        grid: Option<GridShow>,
    },
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
    Tabs {
        tabs: Vec<TabInfo>,
        shown: u32,
        /// The tab this client is.
        #[serde(default)]
        you: Option<u32>,
    },
    /// The answer to a request the client made: a [`ToShell::FileDialog`]'s `id` and the path
    /// chosen (`None`: cancelled, or another dialog was open).
    Answer { id: u32, value: Option<String> },
    /// Debugging: write the next frame drawn to `path` as a PNG.
    Dump { path: String },
    /// Android: the text field above the keyboard (`FieldShow`) was edited: all its text, the
    /// selection in UTF-16 units (M3).
    FieldEdited {
        field: FieldId,
        text: String,
        start: u32,
        end: u32,
    },
    /// Android: the field's Send (Enter).
    FieldEnter { field: FieldId },
    /// Android: the field went (Back, a touch outside it): the text box loses the focus.
    FieldClosed { field: FieldId },
    /// Android: the soft keyboard's height and the display cutout's left and right insets, in
    /// window pixels (Turu lays out around them).
    Insets {
        keyboard: u32,
        cutout_left: u32,
        cutout_right: u32,
    },
    /// Android: the "open" picker's document, copied to `path` (`None`: cancelled, or the copy
    /// failed): the answer to `PickFile`.
    DocumentPicked { path: Option<String> },
    /// Android: the "save" picker made the document `uri`; the slot writes `path`, then copies it
    /// in (`None`: cancelled). The answer to `SaveFile`.
    DocumentCreated {
        uri: Option<String>,
        path: Option<String>,
    },
    /// Android: how the installer ended `Install` (0 installed, 1 cancelled, 2 failed) and why.
    Installed {
        status: i32,
        message: Option<String>,
    },
    /// Android: a key the shell's activity got (a hardware keyboard's, or the soft keyboard's
    /// outside the field above it), for the slot on screen. Read with the device's key map there,
    /// which a slot has no activity to ask: Android's key `code`, its meta state, the character it
    /// types with that state, and with Ctrl the letter it is (`android/src/input.rs`).
    AndroidKey {
        code: u32,
        down: bool,
        meta: u32,
        #[serde(default)]
        typed: Option<char>,
        #[serde(default)]
        letter: Option<char>,
    },
    /// Headless Mode (the game option), as another tab turned it on or off: every tab has it as
    /// one, as VibeSkua's tabbed host sends it to every tab. Also to a tab when it connects, once a
    /// tab has changed it.
    HeadlessMode { on: bool },
    /// Desktop: the shell's window is covered or minimized (`covered`), or seen again. Nothing of
    /// the tab can be seen meanwhile, so it draws nothing; its game, sound and pace go on, as the
    /// window's own tabs do while it's covered.
    Covered { covered: bool },
    /// Desktop: the shell's window went full screen or left it (Esc, the system's own button, a
    /// tab's Options): to every tab, and to a tab when it connects.
    FullScreen { on: bool },
    /// The shell can't use this client's shared surfaces (it couldn't open one, or its GPU isn't
    /// the client's): send pixels from now on ([`ToShell::Frame`]).
    NoSurfaces { reason: String },
    /// Desktop: Turu's update state as the shell has it (the shell checks for updates and
    /// installs them, its clients don't): the client shows it as its own (Menu, Options). Sent
    /// when it changes, and to a client that connects.
    Updates { updates: Box<UpdateInfo> },
    /// Desktop: the accounts the shell's Army launched in processes of their own (Army >
    /// Accounts' "In its own window" and "Headless", asked from any tab: [`ToShell::Launch`]), for
    /// the tab's Army Control and monitor hub. Sent when they change, and to a tab that connects.
    ArmyClients { clients: Vec<ArmyClientInfo> },
    /// The shell is done with shared surface `id` ([`ToShell::Surface`]): the client may draw
    /// into it again.
    Release { id: u64 },
    /// A message this client doesn't know.
    #[serde(other)]
    Unknown,
}

/// The client to the shell.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToShell {
    /// The first message: the shell's token, and who this is. An Android slot gives its control
    /// API too (a desktop client's is the shell's choice, M3).
    Hello {
        token: String,
        pid: u32,
        version: String,
        #[serde(default)]
        api: Option<String>,
        #[serde(default)]
        api_token: Option<String>,
    },
    /// A picture of the tab, its `len` bytes of BGRA pixels right after this message.
    Frame {
        width: u32,
        height: u32,
        seq: u64,
        len: u64,
    },
    /// A picture of the tab in shared surface `id` (macOS: an IOSurface whose Mach port went to
    /// the shell before, [`SURFACES_ENV`]), drawn and finished on the client's GPU. The surface is
    /// the shell's until it sends [`ToClient::Release`]. Sent only to a shell that offered
    /// surfaces; the others get [`ToShell::Frame`]s.
    Surface { id: u64, width: u32, height: u32 },
    /// Desktop: an "open" (or with `save`, "save") file dialog over the shell's window, as
    /// `Platform::pick_file` describes it; answered with [`ToClient::Answer`] and this `id`.
    FileDialog {
        id: u32,
        #[serde(default)]
        save: bool,
        #[serde(default)]
        title: Option<String>,
        #[serde(default)]
        directory: Option<String>,
        #[serde(default)]
        file_name: Option<String>,
        /// Name and extensions (without the dot), as `platform::FileDialog`'s.
        #[serde(default)]
        filters: Vec<(String, Vec<String>)>,
    },
    /// Desktop: Army > Accounts' launches in processes of their own, from a tab: the shell's Army
    /// launches them, so they outlive the tab's client and its web monitor lists them.
    Launch {
        launches: Vec<turu_bot::army::Launch>,
    },
    /// Desktop: Army Control's Close (or with `restart`, Restart) of one of the shell's Army
    /// clients.
    ArmyClient {
        username: String,
        #[serde(default)]
        restart: bool,
    },
    /// Desktop: the client's Download & restart (or Update when I quit, `on_quit`): the shell
    /// installs the update and starts again, every tab with it.
    InstallUpdate {
        #[serde(default)]
        on_quit: bool,
    },
    /// Desktop: the client's Check Now: the shell checks for an update.
    CheckUpdates,
    /// Windows and Linux: Options > Application's Graphics in a client: the shell asks, saves the
    /// choice (`graphics`: "vulkan", "dx12", "gl", or "" for Automatic) and starts again, every
    /// tab with it.
    ChooseGraphics { graphics: String },
    /// Desktop: Options > Application's Full Screen in a client: the shell's window full screen
    /// or not (answered with [`ToClient::FullScreen`]).
    FullScreen { on: bool },
    /// The GPU the client draws on (`adapter_key`): shared surfaces need the shell's.
    Gpu { adapter: String },
    /// Windows: shared surface `id`'s handle in the client's process (`handle`), which the shell
    /// copies into its own (`DuplicateHandle`, from the client it started): a D3D12 resource's
    /// (`route` "dx12") or Vulkan memory's (`route` "vulkan-win32", `size` bytes in memory type
    /// `memory_type`, as allocated). Sent before the surface's first [`ToShell::Surface`].
    SharedHandle {
        id: u64,
        route: String,
        handle: u64,
        width: u32,
        height: u32,
        #[serde(default)]
        size: u64,
        #[serde(default)]
        memory_type: u32,
    },
    /// `Platform::open_url`: the shell opens it (in the browser).
    OpenUrl { url: String },
    /// The client's tab strip asked for another tab.
    SelectTab { id: u32 },
    /// The client's tab strip closed a tab.
    CloseTab { id: u32 },
    /// A new tab, logging in with this (Army > Accounts' "In a tab", the strip's "+"). The
    /// password goes to the new client in `TURU_PASSWORD`, as Army's.
    OpenTab {
        #[serde(default)]
        username: Option<String>,
        #[serde(default)]
        password: Option<String>,
        #[serde(default)]
        server: Option<String>,
        #[serde(default)]
        script: Option<String>,
        #[serde(default)]
        run_script: bool,
        /// A Scheduler playlist to load (or with `run_playlist`, run) in place of the script, with
        /// the Script Options storage its script runs with (`--load-playlist`, `--run-playlist`,
        /// `--script-options-storage`).
        #[serde(default)]
        playlist: Option<String>,
        #[serde(default)]
        run_playlist: bool,
        #[serde(default)]
        options_storage: Option<String>,
    },
    /// The tab's name (the character, once in game) and whether its script or Auto runs, for the
    /// tab strips and the window's title; sent when it changes.
    Status { label: String, busy: bool },
    /// The bar's Grid toggle: Grid View on or off. `left` and `top` are what Turu's UI takes of the
    /// window, in points (the rail, the bar and the tab strip): the tiles fill the rest.
    Grid {
        on: bool,
        #[serde(default)]
        left: f32,
        #[serde(default)]
        top: f32,
    },
    /// Android: a text box has the focus: the shell shows its native text field above the keyboard
    /// (`SurfaceField`) with this text and selection (UTF-16 units). A game field has `max`
    /// characters at most (0: no limit); Turu's say `hint`, `lines` (several) or `number`.
    FieldShow {
        field: FieldId,
        text: String,
        start: u32,
        end: u32,
        #[serde(default)]
        password: bool,
        #[serde(default)]
        max: u32,
        #[serde(default)]
        hint: String,
        #[serde(default)]
        lines: bool,
        #[serde(default)]
        number: bool,
    },
    /// Android: the focused text box changed what it holds.
    FieldUpdate {
        field: FieldId,
        text: String,
        start: u32,
        end: u32,
    },
    /// Android: no text box of this kind has the focus now.
    FieldHide { field: FieldId },
    /// Android: the system's "open" picker for these MIME types, the document copied into
    /// `folder` (`turu_frontend::documents`); answered with `DocumentPicked`.
    PickFile { mime_types: String, folder: String },
    /// Android: the system's "save" picker; answered with `DocumentCreated`.
    SaveFile {
        mime_type: String,
        file_name: String,
    },
    /// Android: the share sheet.
    Share { subject: String, text: String },
    /// Android: an update's APK to the installer; answered with `Installed`.
    Install { path: String },
    /// Android: one of egui's own text boxes (not typed through `FieldShow`'s field) has the focus
    /// (`show`) or lost it: the shell's activity shows or hides the soft keyboard, whose keys come
    /// back as `AndroidKey` and `Text`.
    SoftKeyboard { show: bool },
    /// This tab turned Headless Mode (the game option) on or off: the shell passes it on to the
    /// other tabs (`ToClient::HeadlessMode`).
    HeadlessMode { on: bool },
    /// A message this shell doesn't know.
    #[serde(other)]
    Unknown,
}

/// One of the shell's Army clients, as a tab sees it: its control API and token (commands, the
/// monitor hub's polls), and how its process ended, if it has.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArmyClientInfo {
    pub username: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub headless: bool,
    pub api: String,
    pub token: String,
    #[serde(default)]
    pub exited: Option<String>,
}

/// Turu's update state (`turu_bot::updates::State`) as it goes to a client: when the last check
/// ended as RFC 3339 text.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UpdateInfo {
    #[serde(default)]
    pub available: Option<turu_bot::updates::Release>,
    #[serde(default)]
    pub checked_at: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub checking: bool,
    #[serde(default)]
    pub install: turu_bot::updates::Install,
}

impl UpdateInfo {
    pub fn from_state(state: &turu_bot::updates::State) -> Self {
        Self {
            available: state.available.clone(),
            checked_at: state.checked_at.map(|at| at.to_rfc3339()),
            error: state.error.clone(),
            checking: state.checking,
            install: state.install.clone(),
        }
    }

    /// This process's update state, as the shell has it.
    pub fn apply(self) {
        turu_bot::updates::update_state(|state| {
            state.available = self.available;
            state.checked_at = self
                .checked_at
                .and_then(|at| chrono::DateTime::parse_from_rfc3339(&at).ok())
                .map(|at| at.with_timezone(&chrono::Local));
            state.error = self.error;
            state.checking = self.checking;
            state.install = self.install;
        });
    }
}

/// Which of Android's two text fields above the keyboard: the game's focused text field, or Turu's
/// own text box (egui's).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldId {
    Game,
    Ui,
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
    #[serde(default)]
    pub busy: bool,
    /// Its process isn't running: why ("restarting", "closed").
    #[serde(default)]
    pub down: Option<String>,
    /// Its client's control API (`127.0.0.1:<port>`) and token: the other tabs' monitor hubs poll
    /// it, and Army Control sends it commands (M5).
    #[serde(default)]
    pub api: Option<String>,
    #[serde(default)]
    pub token: Option<String>,
}

/// How a tab shows in Grid View: every tab's game at once, each in a tile of the window. The
/// active tab's client draws the whole window (the bar, its windows, the tiles' labels) with its
/// game in its tile and the other tiles left transparent, and has the keyboard; the others draw
/// only their game, the size of their tile, and the shell puts them under the active one's.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GridShow {
    /// Every tab's tile, in the window's pixels.
    pub tiles: Vec<GridTile>,
    /// This tab's id.
    pub mine: u32,
    pub active: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct GridTile {
    pub id: u32,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl GridShow {
    /// This tab's tile.
    pub fn tile(&self) -> Option<GridTile> {
        self.tiles.iter().copied().find(|tile| tile.id == self.mine)
    }
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
            api: None,
            api_token: None,
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
                grid: None,
            },
            ToClient::Show {
                width: 640,
                height: 360,
                scale: 2.0,
                grid: Some(GridShow {
                    tiles: vec![
                        GridTile {
                            id: 1,
                            x: 0,
                            y: 80,
                            width: 640,
                            height: 360,
                        },
                        GridTile {
                            id: 2,
                            x: 640,
                            y: 80,
                            width: 640,
                            height: 360,
                        },
                    ],
                    mine: 2,
                    active: false,
                }),
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
                    busy: true,
                    down: Some("restarting".into()),
                    api: Some("127.0.0.1:50123".into()),
                    token: Some("a-token".into()),
                }],
                shown: 1,
                you: Some(1),
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
    fn tab_messages_round_trip() {
        let to_shell = [
            ToShell::OpenTab {
                username: Some("player_one".into()),
                password: Some("secret".into()),
                server: Some("Twig".into()),
                script: Some("farm/Example.ts".into()),
                run_script: true,
                playlist: Some("daily.json".into()),
                run_playlist: true,
                options_storage: Some("Squad A".into()),
            },
            ToShell::CloseTab { id: 3 },
            ToShell::SelectTab { id: 2 },
            ToShell::Status {
                label: "Player One".into(),
                busy: true,
            },
            ToShell::Grid {
                on: true,
                left: 0.0,
                top: 64.0,
            },
            ToShell::FieldShow {
                field: FieldId::Game,
                text: "héllo".into(),
                start: 5,
                end: 5,
                password: false,
                max: 140,
                hint: String::new(),
                lines: false,
                number: false,
            },
            ToShell::FieldHide { field: FieldId::Ui },
            ToShell::PickFile {
                mime_types: "application/json".into(),
                folder: "/data/picked".into(),
            },
            ToShell::Install {
                path: "/data/updates/turu.apk".into(),
            },
            ToShell::SoftKeyboard { show: true },
            ToShell::HeadlessMode { on: true },
            ToShell::Surface {
                id: 7,
                width: 1280,
                height: 720,
            },
            ToShell::Gpu {
                adapter: adapter_key("metal", 0x106b, 0, "Apple M5"),
            },
            ToShell::InstallUpdate { on_quit: true },
            ToShell::Launch {
                launches: vec![turu_bot::army::Launch {
                    username: "Hero".into(),
                    label: "Hero".into(),
                    password: Some("secret".into()),
                    server: Some("Twig".into()),
                    script: Some("farm/Gold.ts".into()),
                    run_script: true,
                    headless: true,
                    ..Default::default()
                }],
            },
            ToShell::ArmyClient {
                username: "Hero".into(),
                restart: true,
            },
            ToShell::CheckUpdates,
            ToShell::ChooseGraphics {
                graphics: "dx12".into(),
            },
            ToShell::FullScreen { on: true },
            ToShell::SharedHandle {
                id: 4,
                route: "vulkan-win32".into(),
                handle: 0x2c4,
                width: 1280,
                height: 720,
                size: 3_686_400,
                memory_type: 7,
            },
            ToShell::FileDialog {
                id: 3,
                save: true,
                title: Some("Save the log".into()),
                directory: Some("/data/logs".into()),
                file_name: Some("log.txt".into()),
                filters: vec![("Text".into(), vec!["txt".into()])],
            },
        ];
        let mut bytes = Vec::new();
        for message in &to_shell {
            write(&mut bytes, message, None).unwrap();
        }
        let mut input = bytes.as_slice();
        for message in &to_shell {
            assert_eq!(read::<ToShell>(&mut input).unwrap().as_ref(), Some(message));
        }
    }

    /// An older shell's tab list (without `busy`, `down`, `api`, `token` and `you`) still reads.
    #[test]
    fn a_tab_without_the_newer_fields_reads() {
        let mut bytes = Vec::new();
        let older = serde_json::json!({ "type": "tabs", "tabs": [{ "id": 1, "label": "Tab 1" }], "shown": 1 });
        write(&mut bytes, &older, None).unwrap();
        let read_back = read::<ToClient>(&mut bytes.as_slice()).unwrap();
        assert_eq!(
            read_back,
            Some(ToClient::Tabs {
                tabs: vec![TabInfo {
                    id: 1,
                    label: "Tab 1".into(),
                    busy: false,
                    down: None,
                    api: None,
                    token: None,
                }],
                shown: 1,
                you: None,
            })
        );
        // And an open-tab request without its optional fields.
        let mut bytes = Vec::new();
        write(&mut bytes, &serde_json::json!({ "type": "open_tab" }), None).unwrap();
        assert!(matches!(
            read::<ToShell>(&mut bytes.as_slice()).unwrap(),
            Some(ToShell::OpenTab {
                username: None,
                run_script: false,
                ..
            })
        ));
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
                    grid: None,
                },
                ToClient::Text { text: "é".into() },
                ToClient::FieldEdited {
                    field: FieldId::Ui,
                    text: "abc".into(),
                    start: 1,
                    end: 3,
                },
                ToClient::Insets {
                    keyboard: 800,
                    cutout_left: 0,
                    cutout_right: 120,
                },
                ToClient::DocumentCreated {
                    uri: Some("content://a/b".into()),
                    path: None,
                },
                ToClient::Installed {
                    status: 2,
                    message: Some("no space".into()),
                },
                ToClient::AndroidKey {
                    code: 29,
                    down: true,
                    meta: 0x1000,
                    typed: None,
                    letter: Some('a'),
                },
                ToClient::HeadlessMode { on: true },
                ToClient::Release { id: 7 },
                ToClient::Covered { covered: true },
                ToClient::FullScreen { on: true },
                ToClient::ArmyClients {
                    clients: vec![ArmyClientInfo {
                        username: "Hero".into(),
                        label: "Hero".into(),
                        headless: true,
                        api: "127.0.0.1:50200".into(),
                        token: "a-token".into(),
                        exited: None,
                    }],
                },
                ToClient::Updates {
                    updates: Box::new(UpdateInfo {
                        available: None,
                        checked_at: Some("2026-11-17T05:00:00+07:00".into()),
                        error: None,
                        checking: false,
                        install: turu_bot::updates::Install::Busy("Downloading Turu 0.3.1…".into()),
                    }),
                },
                ToClient::NoSurfaces {
                    reason: "another GPU".into(),
                },
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
