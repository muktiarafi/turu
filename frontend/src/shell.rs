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
/// How many shared surfaces a client draws into in turn: one the shell shows, one or two it may
/// still be reading (presents wait in a queue of two), one being drawn, and one to spare while
/// a release is on its way back: with frame interpolation a picture goes out on every refresh,
/// and three left a third of the refreshes at 120 Hz without one. The shell keeps the newest this
/// many of a client's.
pub const SHARED_POOL: usize = 5;
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
    /// The wheel or a trackpad, in lines (`dx`, `dy`, positive down and right), at (`x`, `y`);
    /// `pixels` when it came in pixels (a trackpad's), [`WHEEL_PIXELS_PER_LINE`] to a line.
    Wheel {
        dx: f32,
        dy: f32,
        x: f32,
        y: f32,
        #[serde(default)]
        pixels: bool,
    },
    /// A key, by egui's name for it (`egui::Key::name`: "Enter", "A", "ArrowLeft", ...; empty for
    /// one egui has no name for), for Turu's UI; and the game's key as the window's `app.rs` gives
    /// it to Ruffle ([`GameKey`]), when the window knows it.
    Key {
        key: String,
        pressed: bool,
        mods: Mods,
        #[serde(default)]
        game: Option<GameKey>,
    },
    /// Text typed (a character, or what an input method committed).
    Text { text: String },
    /// Desktop: an input method's composition over the window ([`ToShell::Ime`] allowed it): for
    /// Turu's focused text box, else the game's.
    Ime { ime: ImeInput },
    /// Desktop: a gamepad's button, by Ruffle's name for it (`--gamepad-button`'s: "south",
    /// "dpad-up", ...), pressed or released.
    Gamepad { button: String, pressed: bool },
    /// Desktop: a file dropped on the window.
    Dropped { path: String },
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
        /// Android: the shell draws the tabs itself, in a native item at the top of the rail
        /// (docs/UI.md, "Native shells"): the client draws no tab strip, and its rail starts this
        /// many points down. `None`: the client draws the strip, as before.
        #[serde(default)]
        rail_top: Option<f32>,
        /// Android: the shell draws the whole rail natively (docs/UI.md, "Native shells"): the
        /// client draws none, but keeps its room (`gui::left_width`) unless hidden.
        #[serde(default)]
        native_rail: bool,
    },
    /// Android: the shell's rail was hidden or shown (its More > Hide Side Bar, its `›`
    /// handle): the client gives the game the room and keeps the choice (`side_bar_hidden`).
    RailHidden { hidden: bool },
    /// The answer to a request the client made: a [`ToShell::FileDialog`]'s `id` and the path
    /// chosen (`None`: cancelled, or another dialog was open).
    Answer { id: u32, value: Option<String> },
    /// Debugging: write the next frame drawn to `path` as a PNG.
    Dump { path: String },
    /// Android: one of the tab's egui windows the shell's native screens open ("script_repo",
    /// "script_options"), while those windows aren't native yet.
    OpenWindow { window: String },
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
    /// The display refreshes `millihertz` times a thousand seconds (frame interpolation's pacing):
    /// to a client when it says hello, and to every client when it changes (Android: the display's
    /// mode; desktop: the monitor the shell's window is on).
    RefreshRate { millihertz: u32 },
    /// Desktop: the shell presented a picture of this tab on a refresh of the display, while the
    /// tab asks for [`Pace::Refresh`]. A new in-between picture is drawn on these, so its pictures
    /// keep in step with the display (a client's own timer drifts against it).
    Refreshed,
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

/// A mouse cursor's shape, as the tab on screen wants it (`ToShell::Cursor`): the game's
/// (Flash's arrow, hand, I-beam, grab) or one of Turu's UI's.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Cursor {
    #[default]
    Default,
    Hand,
    Text,
    Grab,
    Grabbing,
    Move,
    NotAllowed,
    Wait,
    ResizeHorizontal,
    ResizeVertical,
    ResizeNwSe,
    ResizeNeSw,
    /// Hidden.
    None,
}

/// An input method's composition, as winit gives it (`ToClient::Ime`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ImeInput {
    Enabled,
    /// The text being composed, and the cursor in it (byte offsets), if shown.
    Preedit {
        text: String,
        #[serde(default)]
        cursor: Option<(usize, usize)>,
    },
    /// The composition is done: `text` is typed.
    Commit {
        text: String,
    },
    Disabled,
    #[serde(other)]
    Unknown,
}

/// How the shell presents a tab's pictures ([`ToShell::Pace`]), as its frame interpolation needs.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Pace {
    /// Each picture when it comes, waiting for the display's refresh (one per game frame).
    #[default]
    Frames,
    /// On every refresh of the display, telling the tab after each ([`ToClient::Refreshed`]): its
    /// in-between pictures are drawn in step with them.
    Refresh,
    /// Each picture when it comes, without waiting for the refresh (Mugetsu 2.0.0's uncapped
    /// pacing): as many as the tab draws, or its FPS limit allows.
    Uncapped,
    /// A pace this shell doesn't know: taken as `Frames`.
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
    /// Desktop: the mouse cursor over the tab on screen changed (the game's, or Turu's UI's).
    Cursor { cursor: Cursor },
    /// Desktop: how the tab's pictures want presenting (frame interpolation), from now on.
    Pace { pace: Pace },
    /// Desktop: whether the window takes an input method's composition now (a text box of Turu's,
    /// or the game's with Ruffle's IME preference, has the focus), a password's or not, and where
    /// its candidates go: the text's area in the picture's pixels (left, top, width, height).
    Ime {
        allowed: bool,
        #[serde(default)]
        password: bool,
        #[serde(default)]
        area: Option<[f32; 4]>,
    },
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
    Status {
        label: String,
        busy: bool,
        /// What the tab's script (or Butler) waits for the user to answer ("Script Options", "a
        /// message box", "Butler's question"), for the shell's tab list.
        #[serde(default)]
        waits: Option<String>,
        /// Whether the tab's rail is hidden (Menu > Hide Side Bar): the shell's Tabs item hides
        /// with it.
        #[serde(default)]
        rail_hidden: bool,
    },
    /// Android: a window the shell has natively (docs/UI.md, "Native shells") was opened in the
    /// tab's egui menus: the shell shows its own instead.
    OpenScreen { screen: NativeScreen },
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
    /// Android: a display mode refreshing at least `hz` times a second (0: the system's choice),
    /// for frame interpolation; the shell's activity asks for it (only the slot on screen's).
    PreferRefreshRate { hz: u32 },
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

/// A screen Android's shell has natively.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeScreen {
    Accounts,
    /// The Script Loader.
    Scripts,
    /// Search Scripts.
    Search,
    /// Options > Game.
    GameOptions,
    /// Options > Application.
    AppOptions,
    /// Options > CoreBots.
    CoreBots,
    /// Scripts > Scheduler.
    Scheduler,
    /// Army > Army Control.
    Army,
    /// Diagnostics > Notification Center.
    Notifications,
    /// Combat > Stats.
    Stats,
    /// Tools > Current Drops.
    CurrentDrops,
    /// Combat > Skills.
    Skills,
    /// Tools > Loadouts.
    Loadouts,
    /// Tools > Fast Travel.
    FastTravel,
    /// Plugins > Butler.
    Butler,
    /// Plugins > Cosmetics.
    Cosmetics,
    /// Combat > Runtime.
    Runtime,
    /// Combat > Runtime > Notifications.
    NotifyDrop,
    /// Tools & Helpers > Loader.
    Loader,
    /// Scripts > Edit Script Options (also when a script asks for its options).
    ScriptOptions,
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

/// A trackpad's pixels to a line of [`ToClient::Wheel`] (for Turu's UI; the game takes the
/// pixels, as in the window).
pub const WHEEL_PIXELS_PER_LINE: f32 = 40.0;

/// A key for the game, as the window's `app.rs` hands it to Ruffle (`util::
/// winit_input_to_ruffle_key_descriptor`, `winit_to_ruffle_text_control`), by Ruffle's names
/// ([`keys`]): rebuilding it in the client from egui's name lost what egui has no name for (the
/// physical key Ruffle takes `keyCode` from on macOS and Linux and tells held keys apart by;
/// Shift, punctuation, the numpad).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameKey {
    /// Where it is on the keyboard ("KeyW", "ShiftLeft", ...; None for a key Ruffle doesn't know).
    #[serde(default)]
    pub physical: Option<String>,
    /// What it means: a character, else a named key ("Enter", "Shift", ...), else neither.
    #[serde(default)]
    pub character: Option<char>,
    #[serde(default)]
    pub named: Option<String>,
    /// Ruffle's `KeyLocation`: 0 standard, 1 left, 2 right, 3 numpad.
    #[serde(default)]
    pub location: u8,
    /// What it does in a text field instead of typing ("Enter", "SelectLeftWord", ...).
    #[serde(default)]
    pub control: Option<String>,
}

/// Ruffle's key types by the names [`GameKey`] carries them with (their variants' names).
#[cfg(feature = "ruffle")]
pub mod keys {
    use super::GameKey;
    use ruffle_core::events::{
        KeyDescriptor, KeyLocation, LogicalKey, NamedKey, PhysicalKey, TextControlCode,
    };

    macro_rules! table {
        ($table:ident, $ty:ident, $($v:ident),* $(,)?) => {
            const $table: &[($ty, &str)] = &[$(($ty::$v, stringify!($v))),*];
        };
    }
    #[rustfmt::skip]
    table!(PHYSICAL, PhysicalKey,
        Backquote, Digit0, Digit1, Digit2, Digit3, Digit4, Digit5, Digit6, Digit7, Digit8, Digit9,
        Minus, Equal, IntlYen, KeyA, KeyB, KeyC, KeyD, KeyE, KeyF, KeyG, KeyH, KeyI, KeyJ, KeyK,
        KeyL, KeyM, KeyN, KeyO, KeyP, KeyQ, KeyR, KeyS, KeyT, KeyU, KeyV, KeyW, KeyX, KeyY, KeyZ,
        BracketLeft, BracketRight, Backslash, Semicolon, Quote, IntlBackslash, Comma, Period, Slash,
        IntlRo, Backspace, Tab, CapsLock, Enter, ShiftLeft, ShiftRight, ControlLeft, SuperLeft,
        AltLeft, Space, AltRight, SuperRight, ContextMenu, ControlRight, Insert, Delete, Home, End,
        PageUp, PageDown, ArrowUp, ArrowLeft, ArrowDown, ArrowRight, NumLock, NumpadDivide,
        NumpadMultiply, NumpadSubtract, Numpad7, Numpad8, Numpad9, Numpad4, Numpad5, Numpad6,
        Numpad1, Numpad2, Numpad3, Numpad0, NumpadAdd, NumpadComma, NumpadEnter, NumpadDecimal,
        Escape, F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12, F13, F14, F15, F16, F17, F18,
        F19, F20, F21, F22, F23, F24, F25, F26, F27, F28, F29, F30, F31, F32, F33, F34, F35, Fn,
        FnLock, PrintScreen, ScrollLock, Pause,
    );
    #[rustfmt::skip]
    table!(NAMED, NamedKey,
        Alt, AltGraph, CapsLock, Control, Fn, FnLock, Super, NumLock, ScrollLock, Shift, Symbol,
        SymbolLock, Enter, Tab, ArrowDown, ArrowLeft, ArrowRight, ArrowUp, End, Home, PageDown,
        PageUp, Backspace, Clear, Copy, CrSel, Cut, Delete, EraseEof, ExSel, Insert, Paste, Redo,
        Undo, ContextMenu, Escape, Pause, Play, Select, ZoomIn, ZoomOut, PrintScreen, F1, F2, F3,
        F4, F5, F6, F7, F8, F9, F10, F11, F12, F13, F14, F15, F16, F17, F18, F19, F20, F21, F22,
        F23, F24, F25, F26, F27, F28, F29, F30, F31, F32, F33, F34, F35,
    );
    #[rustfmt::skip]
    table!(CONTROL, TextControlCode,
        MoveLeft, MoveLeftWord, MoveLeftLine, MoveLeftDocument, MoveRight, MoveRightWord,
        MoveRightLine, MoveRightDocument, SelectLeft, SelectLeftWord, SelectLeftLine,
        SelectLeftDocument, SelectRight, SelectRightWord, SelectRightLine, SelectRightDocument,
        SelectAll, Copy, Paste, Cut, Backspace, BackspaceWord, Enter, Delete, DeleteWord,
    );

    fn name<T: PartialEq + Copy>(table: &[(T, &'static str)], value: T) -> Option<&'static str> {
        table
            .iter()
            .find(|(v, _)| *v == value)
            .map(|(_, name)| *name)
    }

    fn by_name<T: Copy>(table: &[(T, &str)], name: &str) -> Option<T> {
        table.iter().find(|(_, n)| *n == name).map(|(v, _)| *v)
    }

    /// The key on the keyboard by its name (`PhysicalKey`'s variant: "KeyW", "Enter", ...).
    pub fn physical_key(name: &str) -> Option<PhysicalKey> {
        by_name(PHYSICAL, name)
    }

    impl GameKey {
        /// The window's key for the game, and what it does in a text field.
        pub fn new(key: KeyDescriptor, control: Option<TextControlCode>) -> Self {
            let (character, named) = match key.logical_key {
                LogicalKey::Character(c) => (Some(c), None),
                LogicalKey::Named(named) => (None, name(NAMED, named).map(str::to_owned)),
                LogicalKey::Unknown => (None, None),
            };
            Self {
                physical: name(PHYSICAL, key.physical_key).map(str::to_owned),
                character,
                named,
                location: key.key_location as u8,
                control: control
                    .and_then(|code| name(CONTROL, code))
                    .map(str::to_owned),
            }
        }

        /// Ruffle's key, as the window made it.
        pub fn descriptor(&self) -> KeyDescriptor {
            let logical_key = match (self.character, self.named.as_deref()) {
                (Some(c), _) => LogicalKey::Character(c),
                (None, Some(named)) => {
                    by_name(NAMED, named).map_or(LogicalKey::Unknown, LogicalKey::Named)
                }
                (None, None) => LogicalKey::Unknown,
            };
            KeyDescriptor {
                physical_key: self
                    .physical
                    .as_deref()
                    .and_then(physical_key)
                    .unwrap_or(PhysicalKey::Unknown),
                logical_key,
                key_location: match self.location {
                    1 => KeyLocation::Left,
                    2 => KeyLocation::Right,
                    3 => KeyLocation::Numpad,
                    _ => KeyLocation::Standard,
                },
            }
        }

        /// What it does in a text field, as the window found.
        pub fn control(&self) -> Option<TextControlCode> {
            by_name(CONTROL, self.control.as_deref()?)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        /// Every name goes over and comes back, and no two are alike.
        #[test]
        fn names_round_trip() {
            fn check<T: PartialEq + Copy + std::fmt::Debug>(table: &[(T, &'static str)]) {
                let mut names = std::collections::HashSet::new();
                for (value, n) in table {
                    assert!(names.insert(*n), "{n} twice");
                    assert_eq!(name(table, *value), Some(*n));
                    assert_eq!(by_name(table, n), Some(*value));
                    assert_eq!(format!("{value:?}"), *n);
                }
            }
            check(PHYSICAL);
            check(NAMED);
            check(CONTROL);
        }

        /// A key comes out of the wire as the window made it.
        #[test]
        fn game_keys_round_trip() {
            let keys = [
                (
                    KeyDescriptor {
                        physical_key: PhysicalKey::KeyW,
                        logical_key: LogicalKey::Character('w'),
                        key_location: KeyLocation::Standard,
                    },
                    None,
                ),
                (
                    KeyDescriptor {
                        physical_key: PhysicalKey::ShiftRight,
                        logical_key: LogicalKey::Named(NamedKey::Shift),
                        key_location: KeyLocation::Right,
                    },
                    None,
                ),
                (
                    KeyDescriptor {
                        physical_key: PhysicalKey::NumpadEnter,
                        logical_key: LogicalKey::Named(NamedKey::Enter),
                        key_location: KeyLocation::Numpad,
                    },
                    Some(TextControlCode::Enter),
                ),
                (
                    KeyDescriptor {
                        physical_key: PhysicalKey::Unknown,
                        logical_key: LogicalKey::Unknown,
                        key_location: KeyLocation::Standard,
                    },
                    None,
                ),
            ];
            for (key, control) in keys {
                let game = GameKey::new(key, control);
                let json = serde_json::to_string(&game).unwrap();
                let back: GameKey = serde_json::from_str(&json).unwrap();
                assert_eq!(back.descriptor(), key);
                assert_eq!(back.control(), control);
            }
        }
    }
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
                game: Some(GameKey {
                    physical: Some("Enter".into()),
                    named: Some("Enter".into()),
                    control: Some("Enter".into()),
                    ..GameKey::default()
                }),
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
                rail_top: Some(56.0),
                native_rail: true,
            },
            ToClient::RailHidden { hidden: true },
            ToClient::Hide,
            ToClient::RefreshRate {
                millihertz: 119_880,
            },
            ToClient::Refreshed,
            ToClient::Ime {
                ime: ImeInput::Preedit {
                    text: "にほ".into(),
                    cursor: Some((6, 6)),
                },
            },
            ToClient::Ime {
                ime: ImeInput::Commit {
                    text: "日本".into(),
                },
            },
            ToClient::Ime {
                ime: ImeInput::Disabled,
            },
            ToClient::Gamepad {
                button: "dpad-up".into(),
                pressed: true,
            },
            ToClient::Dropped {
                path: "/scripts/farm/Example.ts".into(),
            },
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
            ToShell::Cursor {
                cursor: Cursor::Hand,
            },
            ToShell::Cursor {
                cursor: Cursor::ResizeNwSe,
            },
            ToShell::Pace {
                pace: Pace::Refresh,
            },
            ToShell::Pace {
                pace: Pace::Uncapped,
            },
            ToShell::Ime {
                allowed: true,
                password: false,
                area: Some([10.0, 20.0, 1.0, 18.0]),
            },
            ToShell::Ime {
                allowed: false,
                password: false,
                area: None,
            },
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
                waits: Some("a message box".into()),
                rail_hidden: true,
            },
            ToShell::OpenScreen {
                screen: NativeScreen::Scripts,
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
                rail_top: None,
                native_rail: false,
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
    fn an_unknown_pace_is_frames() {
        let message: ToShell = serde_json::from_str(r#"{"type":"pace","pace":"triple"}"#).unwrap();
        assert_eq!(
            message,
            ToShell::Pace {
                pace: Pace::Unknown
            }
        );
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
