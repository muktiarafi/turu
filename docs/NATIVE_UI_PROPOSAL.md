# Proposal: Turu as a library, with a native UI on each platform

Status: **withdrawn (2026-11-16). Nothing in this document is planned**: not L0 to L9, and not L1
to L3 either, which it says stand on their own. The owner decided on 2026-11-16 not to go ahead
with any of it. What stands instead: Turu's windows stay egui, shared by every frontend (the
desktop app, Turu on Android, Turu Mas), with the native Android pieces `docs/ANDROID_NATIVE_UI.md`
describes, whose "Kept in egui" decision stands; and process tabs (`docs/TAB_PROCESSES_PROPOSAL.md`)
go ahead as designed, the client drawing the whole tab. The rest of this document is kept as a
record of what was proposed (2026-11-15, nothing built), as it was written apart from notes marking
it withdrawn: where it says "recommended", "decided" or gives an order of work, that is what the
proposal said, not a plan.

As proposed: nothing below is measured yet; L0 is the spike that decides whether the rest goes
ahead as written. Touches `frontend/` (split in two), a new `ffi/`
(crate `turu_ffi`), `app/`, `android/`, `air/ane/`, new native shells (`apple/`, `windows/`,
`linux/`), CI, and three rules in `CLAUDE.md`. Reference: `docs/ARCHITECTURE.md` (threads, the Flash
bridge), `docs/UI.md` (the windows to port), `docs/ANDROID_NATIVE_UI.md` (what is native on Android
today, and the decision this proposal reverses), `docs/TURU_MAS.md` (AIR's limits and licenses),
`docs/HEADLESS.md` (the control API). Since this was written, `docs/TAB_PROCESSES_PROPOSAL.md` has
been partly built and moves each tab's bot out of the process that owns the window: "How this meets
process tabs" says what that would have changed here.

## Summary

Today every Turu frontend draws the same egui windows over the game: the desktop app, Turu on
Android and Turu Mas. This proposal turns what is under those windows into a library with a
plain Rust API and generated bindings, so each platform can write its windows with its own
toolkit: SwiftUI/AppKit on macOS, Jetpack Compose on Android, WinUI on Windows, GTK on Linux.

The binding generator is the small part. The large part is that the egui files hold the windows'
state and logic and reach the engine through process globals. So the plan is, in order:

1. **L0** a spike that answers the open questions about BoltFFI.
2. **L1** split `turu_frontend` into a client library without egui and the egui UI.
3. **L2** move each window's state and logic into view models; egui becomes their first consumer.
4. **L3** turn what the engine asks of the UI (dialogs, questions, alerts) into events and answers.
5. **L4** `turu_ffi`: the bindings, packaging and CI.
6. **L5** the first native shell: Android (Turu), beside egui, window by window.
7. **L6** the game view as a library API, so a shell that owns its main loop can host the game.
8. **L7 to L9** macOS, then Windows and Linux, then Turu Mas, and retiring egui where a shell
   reaches parity.

egui stays the reference frontend throughout, and stays for good wherever a native shell isn't
worth writing. L1 to L3 are worth doing even if no native shell is ever built: they make the
windows' logic testable without a UI.

## How this meets process tabs

`docs/TAB_PROCESSES_PROPOSAL.md` (2026-11-16) runs each tab's game and bot in a process of its own
(a **client**: a slot service on Android, `turu --client` on the desktop) behind a **shell** process
that owns the window or activity and runs no game. Its M2 is built, M3 (Android) and M5 (desktop)
are under way on branches, M6 removes the in-process tabs. This proposal was written the day before
and assumed one process holding the window, the bot and its models.

This section was written on 2026-11-16, when the two proposals were reconciled, and is kept as a
record of what a native shell would cost under process tabs. This proposal was withdrawn the same
day; process tabs go ahead without it, and their proposal no longer has a matching section.

### Where they conflict

* **Where a native window sits relative to its bot.** L5 puts Compose in GameActivity's view
  hierarchy, with "the game still drawn by `screen.rs`", and "Why Android first" counts on no
  change to who owns the loop. Under process tabs the activity's process is the shell, with no
  engine and no models (`android/src/slot_shell.rs`, `SlotShell.kt`); `screen.rs` and the bot are
  in a slot, which draws into the activity's `Surface` handed over Binder. So a Compose window in
  the activity is a process away from its model, and nothing here budgets for that. The desktop is
  the same: the window belongs to the shell (`app/src/shell.rs`), the clients draw into textures it
  shows, and macOS can't put another process's views in a window.
* **"One client per process" leaves a process out.** It holds for every client. The shell has no
  `Client` at all, yet it is where the native windows would be.
* **The tab list.** `Client::tabs`, `add_tab` and `select_tab`, and L2's tab strip model, assume
  the client owns the tabs. Under process tabs the shell owns the list (`slots::SlotShell`,
  `app/src/shell.rs`) and each client mirrors it (`client_link::set_tabs`); after M6 a Ruffle
  client has one tab. Turu Mas still has several bots in one process (`turu::add_tab` from
  `air/ane/rust/src/clients.rs`), so the tab methods stay for it.
* **Reading a view again on every change.** "Changes are revisions" re-reads `view()` whole, which
  is cheap in one process. Across the shell link a message is at most 1 MB of JSON
  (`shell::MAX_MESSAGE`), and on Android a string in a `Bundle` through a `Messenger`
  (`SlotService.toShell`), under Binder's transaction limit; a send that fails is logged and
  dropped. Logs and Packet Logger don't fit that way.
* **The split of `frontend/`.** M2 and M3 added modules L1 has to place, and they lean on egui:
  `client_screen.rs` draws Turu's egui UI into the client's texture, and `client_link.rs` asks
  `gui` for the bar's size and whether the tab is busy.

### Where they agree

* **`Host` is the shell's.** A client's `Platform` already forwards links, pickers, sharing and
  installs to the shell, which answers (`ToShell::OpenUrl`, `PickFile`, `SaveFile`, `Share`,
  `Install`). That is this proposal's `Host`, implemented by the process that owns the window.
* **Questions with an ID.** `ToClient::Answer { id, value }` and the picker and installer answers
  are L3's events and `answer(id, ...)` in small.
* **The player off winit's thread.** M2's client runs `--headless`'s loop with a wgpu device and
  no window, on the desktop: much of L6's "the desktop loop" risk is retired there. M2 left
  Android's `Screen` and its own `ClientScreen` apart; merging them is the same work as L6's
  `attach_surface`, to be done once.
* **Android's surface hand-over** (`attach` / `detach`, a slot letting go before the next takes
  it) is L6's `attach_surface` and `detach_surface`, already working across processes.
* **Typing.** Under process tabs a slot's egui text box reaches the native strip through
  `FieldShow` / `FieldEdited` messages. A native window's text fields are real ones in the shell's
  process and need none of that; only the game's field crosses. L5's "drops its
  `native_field::follow` path" helps process tabs too.
* **Settings and the data folder.** Process tabs write settings under a lock from every process
  (`Paths::merge_settings`); a native Options window in the shell does the same.

### The shape a native UI would have taken (moot)

Settled on 2026-11-16 as the shape a native UI would take if this proposal went ahead (decision 8);
moot since it was withdrawn. The view models stay with their bot, in the client. A native window
lives in the shell and reads a copy of its model that the client keeps current over the shell link;
actions go back over it.

* The client sends a model's view when its revision changes, only for models the shell has open
  and only from the tab on screen. L3's questions and alerts come from every client, shown or not
  (a hidden tab's script can ask). Actions are one-way messages; they post work already.
* The shell keeps the last view of each open model, so `view()` there is a local read, never a
  Binder call on the UI thread. Line-fed models send the lines added, or the window of lines
  shown.
* The shell's own windows come first: the tab strip, the "starting" and "closed" views, Accounts
  opening a tab. Their data is in the shell's process already (`slots::SlotShell`, the Keystore's
  passwords) or in files any process reads (the accounts list), and needs no copy of a model. That
  is the first step of the same path.
* The shell's copy and the in-process model satisfy one Kotlin (or Swift) interface, which "Shells
  bind to a protocol" already asks for. A Compose window doesn't know which it has, so the same
  window works in Turu Mas (in-process, several tabs) and in Turu (a copy, from a slot).
* Not recommended: native views in the clients' processes. An activity per slot gives each tab an
  entry in Recents, its own Back and a UI runtime per slot; embedding a slot's views in the
  activity (`SurfaceControlViewHost`) needs API 30 (from memory, to confirm) where Turu's minimum
  is 26, with the keyboard and TalkBack in embedded views unknown; macOS can't embed another
  process's views.

Why this way round: process tabs exist for containment, so the shell never runs a bot and the
models can't move there; the link is there already, in one format on both platforms, with Binder
telling each side when the other dies; and a slot keeps working with no shell, which a model in the
shell would break. What it costs: views must serialise (serde) and stay small, the shell's copies
are a revision behind, and the shell's process carries Compose (not measured; once, not per slot).
It also narrows "Typed records or bytes": the process boundary is bytes whatever L0 picks (serde
JSON, as the link's messages are), so L0's choice is only for the hop from Rust to Kotlin or Swift
inside the shell.

What it changed here, in the milestones below (moot with them): L0 gains a question (10) and L0's
sixth changes; L1 places the shell and client modules; L2's views serialise and stay small; L5
builds on process tabs and starts with the shell's own windows; L6 and L7 take the game from a
client process on the desktop. "One client per process" gains the shell. Order (decision 9): L0 any time;
L1 after M3's and M5's branches are on main (they change `frontend/src/shell.rs`, `slots.rs`,
`client_screen.rs`, `client_link.rs` and `gui/turu/tab_strip.rs`, which L1 moves); L2 and L3 after
L1, beside M4 and M6; L5 after M4 makes process tabs Android's default; L6 and L7's desktop work
after M5's shared textures (IOSurface) are in the shell.

## Where things stand

### What is already a library

* `turu_bot` (`bot/`, 41,800 lines) is the engine and depends on neither Ruffle nor a UI. It
  reaches the player through one trait, `FlashHost` (`bot/src/host.rs`).
* `turu_frontend` (`frontend/`) was split from the app for Android: no winit, rfd or window. It
  holds the tabs, the Flash host, the windowless player loop (`engine.rs`), the `Platform` trait
  and the egui UI.
* Three frontends use it: `app/` (winit), `android/` (GameActivity, `screen.rs`) and
  `air/ane/rust` (Turu Mas, built without the `ruffle` feature: AIR draws the game).
* Two consumers have no egui at all: the control API (`bot/src/control_api.rs`: 43 routes,
  server-sent events) and the web monitor (`bot/src/monitor/`, whose `Snapshot` is a plain,
  serialisable picture of a bot). They are a UI-free command, state and event surface already.

### What is not

* **The windows are egui and their logic lives in them.** `frontend/src/gui/turu/` is 46 files,
  31,080 lines. A window keeps its own state (`ScriptLoaderWindow`, `StatsWindow`, ...), reads the
  engine each frame and decides what a click does. Nearly every file calls `turu::bot()`,
  `tab_bot` or `spawn_action` directly. None of that is reachable without egui.
* **`turu_frontend` depends on egui at its root** (`Cargo.toml`; `gui.rs` uses it
  outside `gui/turu/`).
* **The client is process globals**: `TABS`, `TAB_REQUESTS`, `ARMY`, `MONITOR_HUB`, `SCRIPT_REPO`,
  `PLATFORM`, `STARTUP` and more in `frontend/src/turu.rs`. There is no handle to hold.
* **The desktop's loop is winit's.** The player lives on winit's event loop thread
  (`app/src/app.rs`); a Swift or C# shell owns the main loop itself. Headless and Android already
  run the player on a thread of the engine's own (`engine::run_loop_paced`); the desktop doesn't.
* **The game and the UI share one surface.** On the desktop and on Android the game draws into a
  texture, and that texture and egui are drawn to the window's surface in one pass
  (`app/src/gui/`, `android/src/screen.rs`). Input is split between them by where it lands.

### What is native already

`docs/ANDROID_NATIVE_UI.md` lists it: on Android the text field above the keyboard, the crash
question, file pickers, the installer, notifications, the share sheet. They reach Rust through
`Platform` (Rust asks the system) and hand-written JNI bound in each app's `java.rs`. That
document's "Kept in egui" says a Compose port of the windows "would be a second UI to keep in
step". **This proposal reverses that decision**, and "What it costs" below says what it buys and
what it costs.

## Goal

* A crate with a plain Rust API that holds everything a Turu frontend needs except drawing its
  windows: the engine, the tabs, each window's state and actions, what the engine asks of the
  user, and (with Ruffle) the game view.
* Bindings to it for Swift, Kotlin and C#, generated, packaged the way each platform expects.
* Native shells that keep VibeSkua's layout and flow (the same menus, windows, names and order)
  with the platform's own controls: text fields, lists, menus, dialogs, accessibility.
* No change to the engine's behaviour, the script API, the control API or the data directory.

Not goals: an iOS app (the Swift binding would serve one, but Ruffle on iOS is its own project);
a web UI (the monitor is that); changing how the game is drawn (`docs/GL_RENDERER_PROPOSAL.md` is
independent of this: it changes what the GPU is asked, not who owns the surface).

## What it costs

Every window exists once today. With native shells it exists once per shell, and a new feature's
window is written once per shell too. That is the cost the earlier decision avoided. Three
things keep it bounded:

* **View models.** What a window knows and does moves to Rust, once (L2). A shell's window is
  then layout and bindings to a model, not logic. How much of the 31,080 lines is logic and how
  much is drawing isn't known; L2's pilot measures it on three windows before the rest is sized.
* **egui as the fallback, per window.** A shell doesn't need every window before it ships. The
  library keeps drawing the egui windows a shell hasn't ported, over the game, as it does now
  (L5, L6). Rarely used windows (Packet Logger, Interceptor, Spammer) may never be ported.
* **Shells are optional, per platform.** Linux can stay on egui. So can Windows, if nobody
  maintains a WinUI shell. The desktop egui app keeps working from the same library.

What it buys, by platform: on Android, real text input everywhere, system lists, menus and
scrolling, TalkBack, and no keys-only keyboard path; on macOS, a menu bar, real windows that
leave the game window, system text fields and shortcuts; on Windows, the same with WinUI. What
it does not buy: speed. The windows aren't a measured cost today.

## Approach

### Crates

```
bot/        turu_bot       the engine (as now)
client/     turu_client    was most of turu_frontend: tabs, Flash host, engine loop, Host trait
                           (today's Platform), view models, the game view.
                           No egui, winit or rfd. Feature `ruffle` as today.
egui_ui/    turu_egui      was frontend/src/gui/turu: the egui windows, on turu_client's models
ffi/        turu_ffi       cdylib + staticlib: turu_client's API with BoltFFI's annotations,
                           written out (no macros), the same under every cfg
ffi_game/   turu_ffi_game  turu_ffi plus the game view, for the shells Ruffle draws for;
                           Turu Mas uses turu_ffi alone (if L0 finds cfg still isn't read)
app/        turu           the desktop egui app (as now, on turu_client + turu_egui)
android/, air/ane/rust     as now, on turu_client (+ turu_egui while windows remain in egui)
apple/, windows/, linux/   the native shells (L7, L8)
```

`turu_client`'s API is plain Rust with no binding annotations. `turu_ffi` is the only crate that
knows the generator. BoltFFI is under a year old and before 1.0 (below), so the generator must
be replaceable without touching the library.

### One client per process

The library keeps the process globals and puts one handle over them: `Client::start(config,
host)` succeeds once per process and a second call is an error. Reasons: Army starts one `turu`
process per account, so nothing needs two clients in one process; Ruffle's and Mugetsu's own
switches are process-wide (`SAFE_DRAWING`'s environment variables, frame interpolation); and
turning some 20 statics into fields would touch every window for no user-visible gain. The handle
makes the API honest about it.

Under process tabs (`docs/TAB_PROCESSES_PROPOSAL.md`) this holds for each client, and a Ruffle
client has one tab once M6 is done; Turu Mas keeps several tabs in its one client. The shell's
process starts no `Client`: if the recommendation in "How this meets process tabs" is taken, it
holds the process-tab list (today's `slots::SlotShell` and `app/src/shell.rs`, plain Rust) and a
copy of each open model per tab, fed over the shell link, behind the same interface a shell binds to.

### The shape of the API

```rust
// turu_client: plain Rust. A sketch, not the final names.
pub struct Client { /* a handle over the process's client */ }

impl Client {
    pub fn start(config: Config, host: Arc<dyn Host>) -> Result<Arc<Client>, StartError>;
    pub fn tabs(&self) -> Vec<TabInfo>;
    pub fn add_tab(&self, startup: Startup) -> TabId;       // posts to the engine loop
    pub fn select_tab(&self, tab: TabId);
    pub fn close_tab(&self, tab: TabId);
    pub fn events(&self) -> Events;                         // tab opened, question asked, alert, ...
    pub fn script_loader(&self, tab: TabId) -> Arc<ScriptLoaderModel>;
    // ... one accessor per window
}

pub struct ScriptLoaderModel { /* ... */ }

impl ScriptLoaderModel {
    /// What the window shows now: plain data, cheap, never calls Flash.
    pub fn view(&self) -> ScriptLoaderView;
    /// Bumped whenever `view()` would differ.
    pub fn revision(&self) -> u64;
    /// Actions return at once; the work runs on a thread of the engine's.
    pub fn load(&self, path: String);
    pub fn start(&self);
    pub fn stop(&self);
}
```

* **Reads are snapshots.** `view()` returns plain records (strings, numbers, lists, enums with
  data). It reads cached state and never calls Flash, so any thread may call it, the UI thread
  included.
* **Changes are revisions.** egui is immediate: it calls `view()` every frame. A native UI is
  retained: it needs to be told. Each model has a revision, and the client offers one stream of
  "model X of tab Y changed" (coalesced, at most once per frame's worth of time). The shell reads
  `view()` again and lets its toolkit diff (SwiftUI and Compose both do). No per-field deltas
  cross the boundary.
* **Actions never block.** Every action posts work, as `turu::spawn_action` does today. This is
  the existing rule ("never call into Flash from the UI thread") made part of the API: a shell
  can't break it, because no exported function waits on the player.
* **Events are queued.** Nothing calls into Swift or Kotlin from inside `Bot::on_flash_call`
  (which runs while the player is locked). The engine pushes to a queue; a thread of the
  library's delivers.
* **Callbacks never run inside a call.** The library calls `Host` and delivers events from its
  own threads, never from inside an exported function the shell is still in. A shell that hops
  to its UI thread then can't re-enter the library mid-call (Crux met this: "What Crux shows").
* **The client doesn't keep the shell alive.** `Client` holds `Host`, and the shell's `Host`
  tends to hold the client. `Client::stop` drops the host, so each shell needn't break the
  cycle its own way.
* **Shells bind to a protocol.** Each shell talks to an interface of its own language that the
  generated classes satisfy, so previews and UI tests run on canned views without the library.
* **Callbacks arrive on any thread.** `Host` (today's `Platform`) is implemented by the shell:
  file dialogs, the browser, the share sheet, alerts, the installer. Its methods are called from
  engine threads, as `Platform`'s are now. Each shell's implementation hops to its UI thread.

### Commands stay in the engine

The windows and the control API already share their commands: Script Loader's Start and the
API's `/script/start` both call `turu_bot::script::start`, and `turu_bot` can't depend on the
client (`control_api.rs` and `monitor/` live in it, below `frontend`). So there is no command
layer to build. What a window adds on top is deciding what to show and when an action is
allowed (Script Loader: the status in words, whether Start is offered while Auto runs, what
Stop means while stopping). That is what L2 moves, into models in `turu_client`.

The rule for L2 and for every shell: a model or a shell adds no engine behaviour. When a window
needs something `turu_bot` lacks, it is added to `turu_bot`, where the control API and the
monitor can use it too. A native shell must not become a second place where commands live.

### The game view

With the `ruffle` feature, the library draws the game. The shell gives it a place to draw:

```rust
impl Client {
    /// Draw the active tab's game on this window or layer from now on.
    pub fn attach_surface(&self, surface: RawSurface, size: Size, scale: f32);
    pub fn resize_surface(&self, size: Size, scale: f32);
    pub fn detach_surface(&self);
    pub fn pointer(&self, event: PointerEvent);
    pub fn key(&self, event: KeyEvent);
    pub fn text(&self, text: String);
    pub fn game_field(&self) -> Option<GameField>;   // the game's focused text field, for a native one
}
```

This is `android/src/screen.rs` made general: there the engine thread already owns the player,
creates a wgpu surface from a window pointer, draws after each tick, paces frame interpolation
(`pacing.rs`) and takes touches and keys through channels. `RawSurface` is a `CAMetalLayer` on
macOS, an `ANativeWindow` on Android, a Wayland or X11 surface on Linux; what WinUI hands over on
Windows (an `HWND`, or a swap chain panel) is a question for L8. Audio moves into the library too
(cpal needs no window: `android/src/audio.rs` uses it that way). The clipboard, the cursor and the
game's right-click menu become `Host` calls and events.

The shell puts its own controls around and over that view. While a shell still has windows in
egui, the library draws them over the game in the same pass, as today, and routes input to them
first (feature `egui_fallback`, which pulls in `turu_egui`).

Turu Mas has no game view: AIR draws the game. Its shell uses the models, events and `Host`
only, and the library builds without `ruffle` and without Mugetsu's code, as `turu_frontend`
does for it now (`docs/TURU_MAS.md`, "Licenses").

### Bindings, by platform

| Platform | UI | Binding | Notes |
|---|---|---|---|
| macOS | SwiftUI + AppKit | BoltFFI, Swift (XCFramework + `Package.swift`) | Listed as full support. |
| Android (Turu) | Jetpack Compose, in GameActivity's view hierarchy | BoltFFI, Kotlin (`jniLibs` + Kotlin sources) | Listed as full support. Must live in one `.so` with the hand-written JNI (`java.rs`): L0. |
| Android (Turu Mas) | Views or Compose in AIR's view hierarchy | BoltFFI, Java or Kotlin | AIR's `d8` needs Java 8 class files, and every dependency must reach the ANE's jar. BoltFFI's Java target says Java 8+; its Kotlin target uses coroutines. L0 decides which, or hand-written JNI as today. |
| Windows | WinUI 3, C# | BoltFFI, C# (NuGet with native assets) | Listed as full support. Which Windows runtimes its package covers (x64, arm64): L0. |
| Linux | GTK 4 / libadwaita through `gtk4-rs`, or stay on egui | none | BoltFFI's only fit would be C, which is experimental and synchronous only (no async, no streams). A Rust toolkit calls `turu_client` directly and needs no binding. |
| Headless, Docker | none | none | Unchanged. |

BoltFFI as read on 2026-11-15 (`github.com/boltffi/boltffi`, its docs' source):

* v0.32.1 (2026-10-30), MIT, repository created December 2025, releases every week or two. Before
  1.0: expect breaking changes, and pin the crate exactly and the `boltffi` CLI to the same version,
  checked by the build (as Crux does).
* Swift, Kotlin, Java, C#, TypeScript/WASM and Python are listed as full support; Kotlin
  Multiplatform and C as experimental; Dart in progress.
* Exports: `#[data]` records and enums, `#[export]` functions, `impl` blocks (classes) and traits
  (callback traits, taken as `Box<dyn Trait>` or `Arc<dyn Trait>`, `Send + Sync` when called from
  several threads), `#[error]` enums (thrown as native exceptions), `async fn` (Swift `async`,
  Kotlin `suspend`, C# `Task`; no Rust runtime needed by the binding itself), and
  `#[ffi_stream(item = T)]` returning `Arc<EventSubscription<T>>` (Swift `AsyncStream`, Kotlin
  `Flow`, C# `IAsyncEnumerable`, or a callback mode).
* Not supported: generic structs, `dyn Trait` other than callback traits, borrowed returns,
  `HashSet`; generic traits and associated types in callback traits. The view records are
  concrete types with owned fields, which fits.
* Packaging: `boltffi pack` writes the XCFramework, `jniLibs`, the NuGet package.
* Its speed figures are its own and aren't a reason here: a UI reads a few snapshots a frame.
  The reason to pick it is one set of annotations for Swift, Kotlin and C#, with streams,
  callbacks and async in each language's own form.

If L0 fails for a target, the fallback is per target, and only `turu_ffi` changes: UniFFI for Swift
and Kotlin, a C#-specific generator for Windows (UniFFI's third-party C# generator or
`csbindgen`: named from memory, to confirm in L0), or more hand-written JNI bound in `java.rs`
for Android, as today.

### What Crux shows

Crux (`github.com/redbadger/crux`, Apache-2.0, since 2022, `crux_core` 0.20.0) is a framework for
this same shape: a Rust core, native shells in SwiftUI, Compose, React and WinUI. It moved from
UniFFI, wasm-bindgen and a hand-written P/Invoke layer to BoltFFI in May 2026 (its PR 537). Read
on 2026-11-15, at its `master`.

**As a framework for Turu: no.** Three things don't fit, and none is a matter of effort:

* Its core is free of side effects and driven by the shell: the shell sends an event, gets back
  the effects (HTTP, time, storage) to perform, and resolves them. Turu's engine is the
  opposite: it owns its threads, the game's socket, the script runtime and the player, and
  blocks on Flash calls. Crux's way to handle effects on the Rust side (middleware) is due to be
  removed for effect routing (its issue 623).
* It has one `ViewModel` for the app, serialised whole on every render (sending diffs is being
  explored, not built). Turu has some 40 windows per tab, several of them fed line by line
  (Logs, Packet Logger), and several tabs.
* It has nothing for a game drawn by the core: no surface, input or audio.

**As evidence about BoltFFI: useful, and narrower than it looks.** Crux exports one class with
three methods, all bytes in and bytes out (`update`, `resolve`, `view`), and in two examples one
callback trait (`Arc<dyn CruxShell>`, `Send + Sync`, one method taking bytes). The typed side
(events, the view model, their bincode code in Swift, Kotlin, TypeScript and C#) comes from a
second generator of Crux's own, `facet-generate`. That surface is older than its BoltFFI use: it
had it with UniFFI. So Crux shows BoltFFI's packaging, classes, byte arrays and a callback trait
working on every target Turu wants. It doesn't exercise what this proposal leans on: `#[data]`
records, `async`, `#[ffi_stream]`.

What it confirms or adds:

* **Targets.** Its counter example builds from one `boltffi.toml`: an XCFramework with macOS
  (`include_macos = true`), Android Kotlin and `jniLibs`, WASM, and a NuGet package for
  `win-x64` and `win-arm64` on .NET 10, used by a WinUI 3 shell. That shell builds only on
  Windows (it needs Visual Studio's MSBuild). This answers part of L0's fifth question.
* **Pinning.** Crux pins the crate exactly (`=0.30.1`) and its build checks the `boltffi` CLI is
  the same version. It is two minor versions behind the current 0.32.1, and its history has a
  commit for each bump (0.29.1, 0.29.3, 0.30.1). Turu pins both the same way.
* **The generator reads source.** At 0.30 it doesn't expand macros, sees exports in a dependency
  only when it is a path dependency (Crux's generated-core RFC), and doesn't evaluate `#[cfg]`
  (a comment in its counter-middleware example: the exported surface must be the same under
  every `cfg`). BoltFFI has since closed issues about honouring `cfg` and about replacing the
  source scan, so this may have changed by 0.32; L0 checks. It matters here twice: the exports
  must be written out in `turu_ffi`, not produced by a macro; and the game view, which exists
  only with the `ruffle` feature, can't be gated inside one exported surface.
* **Callbacks that re-enter.** Where Rust called the shell's callback inside `update`, a Kotlin
  shell that hopped back with `Dispatchers.Main.immediate` ran inline and re-entered the core
  mid-call; and each shell had to break the cycle core, callback, core by hand (issue 623).
* **A seam for previews and tests.** Its shells talk to a small protocol, not to the generated
  class, so a SwiftUI or Compose preview runs on canned data without loading the Rust library.
* **The way back exists.** Crux ran the same core on UniFFI before, so UniFFI as the Swift and
  Kotlin fallback is a known shape, not a guess.

What this changes in the plan: L0 compares two shapes for the boundary (below, "Typed records or
bytes"); `turu_ffi` becomes two small crates; and two rules join the API's.

### Typed records or bytes

Two ways to carry a window's view across:

* **Typed records** (this proposal's sketch): each view is a `#[data]` record, each model an
  exported class, changes a stream. One generator, native types on the other side, no
  serialisation code. It uses the most of BoltFFI, including the parts Crux doesn't.
* **Bytes, as Crux does**: a handful of exported methods (`view(tab, model) -> bytes`,
  `act(tab, model, action bytes)`, one callback for "changed" and events), with the records
  serialised and their Swift, Kotlin and C# types from a type generator. Turu's records already
  derive serde, and the control API already sends the same data as JSON. It uses only what Crux
  proves, and changing the binding generator later costs a few lines. It costs a second
  generator and a decode on every read.

L0 builds the spike both ways for one model and picks one. The plain Rust API in `turu_client`
is the same either way.

### Rules that change

* **"Same UI/UX and flow as VibeSkua"** becomes: the same menus, windows, names, order and flow;
  the controls are the platform's. `docs/UI.md` stays the inventory, with a column for which
  shells have each window.
* **"Kotlin and Rust meet only through each Android app's `java.rs`"** becomes: hand-written JNI
  only through `java.rs`, checked at start as now; the generated bindings are a second path,
  where both sides come from the same Rust source in the same build, so they can't disagree. A
  build that mixes a stale generated Kotlin file with a new `.so` must fail: CI generates, never
  commits generated code.
* **"No winit, rfd or window in `frontend/`"** becomes: none of those and no egui in
  `turu_client`, checked in CI (`cargo tree -p turu_client` names none of them).
* The three threading rules stay as they are, and the API enforces the first.

## Milestones

Each ends with the three existing frontends building and `cargo test -p turu_bot` and the app's
tests passing. Sizes are relative (S, M, L, XL), from the files each touches; none is measured.

### L0: The spike (S)

A throwaway crate with a `Client`-shaped API (a class, a record with nested lists and an enum
with data, an action, a callback trait, a stream), bound and called from a Swift command-line
tool, the Android app and a C# console program. It answers:

1. Which thread do callback traits and stream items arrive on, in each language?
2. Does BoltFFI's Kotlin binding live in the same `.so` as jni 0.22's `bind_java_type!` and
   `RegisterNatives`, under GameActivity's class loader and from `TuruService` alone?
3. Turu Mas: do the Java or Kotlin bindings compile to Java 8 class files that AIR's `d8`
   accepts, and what do they add to the ANE's jar? And before L5 commits Android to Compose: do
   Compose and its runtime get through that `d8`, can a `ComposeView` be added to AIR's own
   activity, and does it show, given that Turu's overlay there is a surface drawn over every
   view (the text strip needed the overlay clipped around it)? If not, Turu Mas's windows are
   plain Android views or stay in egui, and L5 should know that before it starts.
4. Does a 40-field record with lists (the monitor's `Snapshot` is a ready test case) cross
   without surprises? Is `HashMap` supported, or do maps become lists of pairs?
5. Which Windows and Linux runtimes does the C# package cover, and does it build from macOS or
   only on Windows?
6. Can a wgpu surface be made from a `CAMetalLayer` handed in by Swift and drawn from a thread
   that isn't the main thread, at the display's rate? Under process tabs the game is drawn in a
   client process, so also: can a Swift window show an IOSurface a client renders (M2's
   `app/examples/iosurface_share.rs` did it between two wgpu devices)?
7. What does the binding add to the binary's size and the build's time?
8. Typed records or bytes ("Typed records or bytes")? Build one model both ways in Swift and
   Kotlin and compare the code a shell writes, what a change to a record costs, and how each
   fails when the two sides disagree.
9. Does the pinned version read `#[cfg]` and exports behind a feature? If not, the game view
   goes in a second crate (`turu_ffi_game`).
10. Process tabs: with M3's slots on, does a Compose view in the activity, bound to a copy of one
    model sent from a slot over the `Messenger`, keep up and stay correct? How big are the views
    (the monitor's `Snapshot`, Script Loader, Logs with a full buffer) against Binder's limit, how
    long from a change in the slot to the view redrawn, and what does Compose add to the shell's
    process (~45 MB on the emulator today)?

**Done when** the answers are written into this document's status, with a go, a go with
fallbacks named per target, or a stop.

### L1: `turu_client` without egui (M)

Split `frontend/` into `client/` and `egui_ui/`. Move the egui use out of `gui.rs`. Rename
`Platform` to `Host`. Add `Client` as a handle over the globals. The app, Android and Turu Mas use
both crates; nothing a user sees changes. Process tabs' modules go by what they need: the messages
and the slots' records and tab list (`shell.rs`, `slots.rs`) to `turu_client`; `client_screen.rs`,
which draws Turu's egui UI into the client's texture, to the egui side (the `egui_fallback` layer of
L6); `client_link.rs` to `turu_client` once it asks a model, not `gui`, for the bar's size and
whether the tab is busy. Best done after M3's and M5's branches are on main.

**Done when** `turu_client` builds with and without `ruffle`, CI checks it has no egui, winit or
rfd, and the three frontends pass their tests and start.

### L2: View models (XL)

Move each window's state and what it decides into `turu_client`. First what isn't a window but every
shell needs: the nav menu (its entries, what `Host` hides, which are enabled), the set of open
windows per tab (`Windows`' flags in `gui/turu/mod.rs`), the tab strip, and hotkeys (their bindings
and what each does: today `hotkeys::run_pressed` runs inside egui's frame). With those in the
library, a native rail can open a window that is still egui. Then pilot on three windows of
different kinds: Script Loader (a status and commands), Logs (lines arriving) and Game Options (a
form that saves). Record for each how many lines stayed in egui and how many moved: that ratio sizes
the rest and goes in this document. Then the rest, a nav bar menu at a time (Scripts, Options, Tools
& Helpers, Combat, Bank, Diagnostics, Plugins, Army), in the order the first shell wants them.

Each model gets tests on `turu_bot::testing`'s fake Flash host, with no UI: what the window
shows after an event, what an action posts.

By decision 8, each view and action also derives serde and stays small enough
for the shell link (well under 1 MB): models fed line by line (Logs, Packet Logger) offer the
lines added since a revision, or a window of lines, beside the whole view. The tab strip's model
is the shell's tab list, not the client's (`client_link::set_tabs` mirrors it today).

**Done when**, per window, its egui file imports nothing from `turu_bot` but record types, and its
model has tests; and egui's nav bar, open windows and hotkeys run on the library's models. For the
milestone: every window in `docs/UI.md` is marked moved or kept (with a reason).

### L3: Questions, dialogs and alerts as events (M)

What the engine asks of the UI today, each by its own route: a script's message box
(`bot.showMessageBox`), Edit Script Options (`ScriptManager`'s configure request, polled each
frame), "Send crash reports?", the update window, file dialogs, alerts, the Tab Limit notice,
tab requests. Make them one stream of events with an ID each, and `answer(id, ...)`. egui
answers them as it does now; a shell shows its own dialog.

Under process tabs the shell answers what a client asks with `ToClient::Answer { id, value }`;
the events' IDs can be that `id`, and the picker and installer answers (`DocumentPicked`,
`DocumentCreated`, `Installed`, new in M3) could fold into it.

**Done when** egui polls nothing but models and the event stream, and a test client with no UI
can answer a script's message box and options request.

### L4: `turu_ffi`, packaging, CI (M)

The annotated wrapper over `turu_client` in the shape L0 chose, at the version L0 pinned, with
the CLI's version checked by the build. `boltffi pack` in CI for the
targets L0 passed; the generated sources are build outputs. A check that every model and event
in `turu_client` is exported or listed as deliberately not.

**Done when**, in CI, a Swift, a Kotlin and a C# test each start a client without a game view
(Turu Mas's shape), list the tabs, read a model and receive an event.

### L5: The first shell, Android (L)

Turu on Android, with Compose in GameActivity's view hierarchy over the `SurfaceView`, where the
native text strip already sits. It builds on L2's nav, open-window and hotkey models. The Compose
code lives in the Turu app's module unless L0's third question passed, in which case it goes in
`android/shared` from the start, for Turu Mas. The game is still drawn by `screen.rs`, and so are
the egui windows not yet ported. First the nav rail and the tab strip, then the pilot windows, then
by use. Each ported window drops its `native_field::follow` path: its text fields are real ones.

Why Android first (see "Decisions to make"): it needs no change to who owns the loop, so it
doesn't wait for L6; Kotlin is a full-support target; a view hierarchy over the game exists; and
it is where egui is weakest (touch, the keyboard, scrolling, accessibility).

Under process tabs ("How this meets process tabs", decision 8)
that paragraph holds in part: the activity is the shell and the game and the egui windows are drawn
by the slot on screen, into the activity's surface, so Compose over it still works as a picture,
but its models are in the slot. So: L5 builds on process tabs only, after M4 makes them Android's
default (decision 9); it starts with the shell's own windows (the tab strip, the "starting" view,
Accounts opening a tab), which need no model from a slot; then the rail and the pilot windows, on
copies of the slot's models sent over the shell link (L0's tenth question). Touches on a Compose
view stay in the activity; the rest go to the slot as now.

**Done when**, on the Android 16 arm64 emulator, the rail and the pilot windows are Compose, a
script can be found, loaded, started and stopped without touching an egui window, the remaining
windows still open in egui, and a kill and restart by the service works as before. With process
tabs: the same with two slots, switching tabs switches what the Compose windows show, and a slot
killed while its window is open shows that window again, current, once the slot is back.

### L6: The game view (L)

`attach_surface` and the input API in `turu_client`, lifted from `android/src/screen.rs`, with
audio and the `egui_fallback` layer. Android moves onto it first (no behaviour change). Then a
desktop frontend runs the player on the engine's thread with no winit: the existing egui app can
stay on winit until its platform's shell replaces it.

Process tabs already did parts of this. Android's slots attach and detach the activity's surface
across processes (M1, M3), and the desktop's client (`turu --client`, M2) runs the player on the
engine's thread with no winit, drawing into a texture. Android's `Screen` and the client's
`ClientScreen` are still two; merging them is this milestone's `attach_surface`, done once. Under
process tabs a desktop shell doesn't hand a layer to an in-process library: the client draws into a
texture the shell's window shows (M5's IOSurface route on macOS), so a Swift shell takes the game
the way `app/src/shell.rs` does.

**Done when** Android runs on the shared code, and a minimal Swift window with one layer shows
the game, logs in with the game's own screen, takes the mouse and keyboard, plays sound, and
shows an egui window over the game through the fallback (with process tabs, from a client process).

### L7: macOS (L)

`apple/`: an AppKit window with the game's layer, the menus in the menu bar and a toolbar, the
windows as SwiftUI. Signing, notarisation and the updater (`turu_bot::updates` swaps the program
today) move with it. With process tabs on the desktop, this app is the shell: it starts the
clients (as `app/src/shell.rs` does) and shows their textures, and, by decision 8, its windows read copies of the clients' models, as on Android.

**Done when** the windows marked for macOS in `docs/UI.md` are SwiftUI, the rest open through
the fallback, and a release build updates itself.

### L8: Windows and Linux (L each, optional)

Windows: WinUI 3 in C# on the NuGet package, if L0 passed it and someone will maintain it.
Linux: GTK 4 through `gtk4-rs` on `turu_client` directly, or stay on egui. Each is a decision
made after L7, with L7's real cost known.

### L9: Turu Mas, and retiring egui (M)

Turu Mas takes Android's Compose windows through `android/shared`, placed in AIR's view hierarchy as
its text strip is, if L0 found that possible; else it stays on egui. Process tabs leave Turu Mas
out, so there the models are in-process (several tabs, one client) and need no link: the same
Compose windows bind to them through the same interface. Where a shell has every window
it wants, its build drops `egui_fallback`. The desktop egui app stays for the platforms without a
shell and as the reference.

## Risks

* **BoltFFI's age.** Under a year old, weekly releases, no stability promise. Kept to the FFI
  crates; version pinned; fallbacks named per target. Crux, its most visible user, exercises
  only a small part of it ("What Crux shows").
* **L2 is most of the work and shows nothing new to a user.** It is also the part with value on
  its own. The pilot's ratio decides whether to go on.
* **Several UIs drift.** A window ported to one shell and changed in another. `docs/UI.md`'s
  per-shell column and the models' tests are the guard; a change in behaviour belongs in the
  model, where every shell gets it.
* **Two UIs on one screen during the move** (native and egui fallback): focus, the keyboard and
  Back between them. Android's text strip already crosses that line, and its pitfalls are in
  `docs/ANDROID_NATIVE_UI.md`.
* **Turu Mas.** A window of its own takes AIR's focus and drops the game field's; views must go
  in AIR's hierarchy, built as Java 8 class files, and the AIR SDK's license allows only
  documented ways into the runtime. It is last for that reason.
* **The desktop loop.** Moving the player off winit's thread changes pacing, input and the
  surface's lifetime on three systems. L6 does it for one shell at a time and leaves the egui
  app on winit. Process tabs' desktop client (M2) already runs the player with no winit; what
  remains is showing its texture in a native window.
* **Models a process away from their windows.** Under process tabs (decision 8), a native window is in the shell and its model in a client ("How this meets process
  tabs"). Every view crosses a process,
  as serialised data under Binder's size limit on Android (a send that's too big is dropped, not
  delayed); the shell's copy is a revision behind; a slot that dies leaves its windows showing old
  data until it's back. L0's tenth question measures it before L5.
* **Licenses.** The library is GPL-3.0-or-later, so the shells are too; BoltFFI is MIT.
  Mugetsu's code stays out of Turu Mas's build, as now.

## Decisions to make

Decided 2026-11-16: decision 1 went "no", so decisions 2 to 9 are moot. They are kept as they were
written, apart from 8 and 9 marked moot.

1. ~~**Go ahead at all?**~~: decided 2026-11-16: no, none of it, L1 to L3 included. As written:
   L1 to L3 stand on their own; L4 on needs someone to write and keep each shell.
2. **First shell: Android (recommended) or macOS.** macOS has the quickest development loop on
   this machine, but needs L6 first. Android needs neither L6 nor a new loop, and gains most.
   Under process tabs (decision 8), Android's shell also needs copies of the
   slots' models over the link; so does a macOS shell under desktop process tabs, so that doesn't
   change the order.
3. **Windows: WinUI in C#, or stay on egui.** Decide after L7.
4. **Linux: GTK in Rust, or stay on egui (recommended until someone asks).**
5. **How far does the fallback go?** Port every window, or leave Diagnostics in egui for good.
6. **Turu Mas: Kotlin bindings, Java bindings, or hand-written JNI.** L0's third question.
7. **Typed records or bytes across the boundary.** L0's eighth question. Typed is less code if
   BoltFFI holds; bytes is what its most visible user ships and is cheaper to leave. By decision 8
   the views are bytes between processes either way (serde, as the link's
   messages are), so this choice is only for the hop from Rust to Kotlin or Swift in the shell's
   process, and serde on every view is needed whichever is picked.
8. ~~**Native windows under process tabs**~~ (with `docs/TAB_PROCESSES_PROPOSAL.md`, whose
   matching decision was removed with the withdrawal): moot (decision 1). Recorded earlier on
   2026-11-16 as decided if this proposal went ahead: models stay in the
   clients and native windows in the shell read copies sent over the shell link, the shell's own
   windows first. Not taken: native windows only for the shell's
   own windows, the bot windows staying egui for good; native views in the clients' processes
   (Recents and Back per slot, API 30 against a minimum of 26, no macOS); no native
   shell on platforms with process tabs.
9. ~~**Order with process tabs**~~ (its matching decision there removed too): moot (decision 1).
   Recorded earlier on 2026-11-16 as decided if this proposal went ahead: L0 any time; L1 after
   M3's and M5's branches are on main; L2 and L3 after L1, beside M4 and M6; L5 after M4 makes
   process tabs Android's default, on process tabs only; L6's and L7's desktop work after M5's
   shared textures are in the shell. Not taken: L5 earlier, working with both kinds of tabs, and M6
   waiting for it.
