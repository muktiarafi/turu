# Proposal: native windows on each platform, over process tabs

Status: **reopened (2026-11-20), in steps. L0 and L5a are done (2026-11-20, on the emulator; "L0's
answers" and "L5a as built" below). Checkpoint passed (2026-11-20): go on with Android's bot
windows (L1's client part to L5b); macOS waits for its own decision. L2's pilot is under way.** Proposed on 2026-11-15, reconciled with process tabs on 2026-11-16 and withdrawn the
same day (`55737175c` records the decision, not a reason). Reopened on 2026-11-20, now that process
tabs are built, with the owner's decisions in "Decisions taken". Planned:

1. **L0**, a narrowed spike: what Compose costs in the Android shell, and copies of a client's
   models read over its control API.
2. **L5a**, after L0 and a design pass: the Android shell's own windows (the tab strip, Accounts
   opening a tab) in Compose, beside the bot windows, which stay egui. The design pass is
   approved (2026-11-20) and recorded in `docs/UI.md`, "Native shells".

Then a **checkpoint**: the owner decides from L5a whether to go on to view models and native bot
windows (L1 to L5b) and to macOS (L6, L7). Planned any time, as it pays without native UI: the
desktop shell's logic as a library (L1's second part). Not planned: native shells on Windows and
Linux (they stay egui), Turu Mas (stays egui unless L9 is reopened). Until each step lands, Turu's
windows stay egui, drawn by each tab's client, and `docs/ANDROID_NATIVE_UI.md`'s "Kept in egui"
stands for everything L5a doesn't touch.

Nothing below is measured unless it says so. Touches `android/` (Compose in the shell), `frontend/`
(the split, held), `app/` and a new `shell/` (crate `turu_shell`), `bot/src/control_api.rs` (routes
the client adds), later `ffi/` (crate `turu_ffi`, macOS only) and `apple/`, CI, and a rule in
`CLAUDE.md` once L5a ships. Reference: `docs/TAB_PROCESSES_PROPOSAL.md` (shell, clients, the link;
"M6 as built"), `docs/ARCHITECTURE.md`, `docs/UI.md`, `docs/ANDROID_NATIVE_UI.md`,
`docs/HEADLESS.md` (the control API).

## Summary

Since M6 (2026-11-17) every AQW tab, on the desktop and on Android, is a **client** process (one
player, one bot, Turu's egui UI) behind a **shell** process that owns the window or activity and
runs no game. The client draws the whole tab, the game and Turu's windows in one picture, and the
shell shows that picture. Turu Mas is the exception: several bots in one AIR process, no shell.

A native UI under that model means native windows **in the shell**, beside or over the client's
picture, while each feature's state and logic stay **in the client**, with its bot. A native window
reads a copy of its feature's state from the client's control API and sends actions back. Process
tabs fixed that shape: the shell can't hold a bot (it exists so that one tab's death doesn't end
the others), and a client can't put its views in the shell's window (macOS can't embed another
process's views; on Android `SurfaceControlViewHost` needs API 30, from memory and unconfirmed,
where Turu's minimum is 26, and an activity per slot gives each tab an entry in Recents and a Back
of its own).

Each native shell is designed for its platform, not copied from Turu's egui UI or VibeSkua's.
What a feature does stays the engine's and the models' (VibeSkua's behaviour, the script API, the
control API); how it is laid out, navigated, grouped and named is each shell's, following its
platform's conventions (Material on Android, Apple's guidelines on macOS), for the best UI and UX
there. The egui UI keeps VibeSkua's layout, as the reference for behaviour and the UI of every
platform without a native shell.

The steps, with their state:

| Step | What | State |
|---|---|---|
| L0 | Spike: Compose in the shell, model copies over the control API, type generation | Done 2026-11-20: go (branch `native-ui-l0`) |
| L5a | Android: design pass, then the tab strip and Accounts in Compose | Built 2026-11-20 (emulator); checkpoint next |
| L1 (shell) | The desktop shell's logic out of winit, into `turu_shell` | Planned, any time |
| Checkpoint | Go on to native bot windows, or stop at L5a | Owner, after L5a |
| L1 (client) | `turu_frontend` split into a client library without egui and the egui UI | After the checkpoint |
| L2 | View models in the client, by feature | After the checkpoint |
| L3 | Questions, dialogs and alerts as events with an ID and an answer | After the checkpoint |
| L4 | Generated types for Kotlin and Swift in CI; `turu_ffi` for macOS | After the checkpoint |
| L5b | Android: navigation and the daily features in Compose | After L2 to L4 |
| L6, L7 | macOS: the game view behind a native window, then the app | After the checkpoint |
| L8 | Windows and Linux native shells | Not planned: egui |
| L9 | Turu Mas | Not planned: egui |

## What process tabs changed

`docs/TAB_PROCESSES_PROPOSAL.md` is built on `main` except its phone runs: M0, M2, M3 (Android),
M5 (desktop) and M6 (one way of doing tabs) are in; M4 (real phones) isn't started; desktop process
tabs have run in game on macOS and on Windows (2026-11-18, `docs/STATUS.md`, "Pictures under way
together"), and on Linux only CI has built them. Anything below that rests on phones or Linux is
unverified.

What that settles or makes cheaper here:

* **Where the native windows go.** In the shell. On 2026-11-16 this was a decision recorded for
  the case the proposal went ahead; it is now the only shape.
* **`Host` is the shell's, and it is built.** A client's `Platform` already forwards to the shell
  what only the window's process can do: links (`ToShell::OpenUrl`), file dialogs (`FileDialog`,
  `PickFile`, `SaveFile`), sharing, installing, the graphics choice, full screen, Army launches,
  update checks and installs. The shell answers with `ToClient::Answer { id, value }`,
  `DocumentPicked`, `DocumentCreated`, `Installed`. That is the old `Host` trait and L3's
  `answer(id, ...)` in small.
* **Every client already serves its state.** Each client, desktop or slot, runs a control API on
  loopback with its own token (slots' addresses in `slots/c<N>.api.json`, and in their `Hello`),
  with `/events` streaming changes, and the web monitor polls each one's `Snapshot` every 2 s. A
  native window reading its feature from there is one more reader of a working channel.
* **State the shell owns already crosses to the clients.** On both platforms the tab list
  (`Tabs`, from each client's `Status`: label, busy), Headless Mode across tabs and the display's
  refresh rate; on the desktop also Turu's updates (`Updates`) and the Army clients the shell
  launched (`ArmyClients`). On Android the activity can go, so the monitor and update checks are
  run by one elected slot (`turu::start_monitor_election`) and the activity only installs; the
  Android shell holds the tab list, the slot records and the Keystore's passwords. Each client
  draws this state in egui today. What the shell holds is where its first native windows start
  (L5a), with nothing to copy.
* **The game is off winit's thread.** A client runs `--headless`'s loop with a GPU and no window
  (`app/src/client.rs`); `players.rs` holds one player. The old L6's main risk (moving the player
  off winit's loop on three systems) is gone: a native shell runs no player at all.
* **The game view is a texture the shell shows.** Desktop clients draw into shared GPU surfaces
  (IOSurface on macOS; D3D12 or Vulkan handles on Windows; Vulkan fds on Linux, `shared_gpu.rs`),
  falling back to pixels; Android's slot on screen draws into the activity's `Surface` handed over
  Binder. A native shell shows those; it doesn't host Ruffle.
* **Android needs no generated binding.** The shell's Kotlin already reaches its Rust through
  `java.rs`, and with models read over the control API as JSON, the Kotlin side needs only types
  generated from the Rust records. A generated FFI binding is left for macOS, where Swift hosts
  `turu_shell` (which runs no player), so the old plan's game-view crate (`turu_ffi_game`) and its
  question about `#[cfg]` go.
* **"One client per process" is literal.** A Ruffle client has one tab and one player. Turu Mas
  keeps several bots in one process (`turu::add_tab` from `air/ane/rust/src/clients.rs`), so the
  client library keeps its tab list for it.
* **Typing across processes works.** A slot's egui text box reaches the activity's native field
  through `FieldShow` / `FieldUpdate` / `FieldEdited`. A native window's text fields are real ones
  in the shell and need none of that; only the game's field and the windows left in egui keep it.
* **Settings are merged from every process** (`Paths::merge_settings`, under a lock), so a native
  Options window in the shell writes them the same way.

What it costs here, that the 2026-11-15 plan didn't budget:

* **A native desktop shell replaces `app/src/shell.rs`**: 2,865 lines of winit that start and
  restart clients (backoff, give-up, a click to retry), import their shared surfaces and keep the
  `Pace` / `Refreshed` / `Release` protocol frame interpolation needs, lay out Grid View, translate
  keys, IME, gamepads and the cursor for the client, queue dialogs, run updates, Army launches,
  the monitor and the shell's control API forwarding. L1's second part moves it into a winit-free
  library each shell hosts (decided), as `slots::SlotShell` is for Android's shell already.
* **egui is in the client's picture.** The fallback for windows a shell hasn't ported is the
  client's picture, Turu's egui windows drawn into it. Native windows in the shell and egui windows
  in the client are then a process apart: focus, the keyboard, Back and Escape have to be
  arbitrated across the link (the shell decides what has the focus; the client is told). Android's
  text strip already does this for one field.
* **Copies are a revision behind**, a slot that dies leaves its windows showing the last copy
  until it is back, and every view must serialise.
* **The shell's process carries the toolkit.** On Android the shell is about 45 MB today (M1,
  emulator); what Compose adds isn't measured (L0). It is paid once, not per slot.

## Where things stand

### What is a library already

* `turu_bot` (`bot/`, 42,500 lines) is the engine and depends on neither Ruffle nor a UI. It
  reaches the player through one trait, `FlashHost` (`bot/src/host.rs`).
* `turu_frontend` (`frontend/`) holds the tab, the Flash host, the windowless player loop
  (`engine.rs`), `Platform`, the shell link (`shell.rs`: messages, no Ruffle, no window; its key
  table behind `ruffle`), the client's end of it (`client_link.rs`, `client_screen.rs`), Android's
  shell logic (`slots.rs`: `SlotShell`, plain Rust, tested on the desktop) and the egui UI.
* Three frontends use it: `app/` (the shell, `turu --client`, `--headless`, a one-player window
  for a plain movie), `android/` (the shell in the activity's process, slots as services) and
  `air/ane/rust` (Turu Mas, built without the `ruffle` feature).
* Three consumers have no egui: the control API (`bot/src/control_api.rs`: some 40 routes and
  server-sent events at `/events`; every client has one, and the shell forwards `/tabs/<id>/...`
  to them), the web monitor (`bot/src/monitor/`, whose `Snapshot` is a serialisable picture of a
  bot) and the shell link. All three already carry a bot's state out of its process.

### What isn't

* **The windows are egui and their logic lives in them.** `frontend/src/gui/turu/` is 47 files,
  31,700 lines. A window keeps its own state, reads the engine each frame and decides what a click
  does, calling `turu::bot()`, `tab_bot` or `spawn_action` directly.
* **`turu_frontend` depends on egui at its root** (`Cargo.toml`; `gui.rs`, `client_screen.rs`,
  and `client_link.rs`, which asks `gui` for the bar's size and whether the tab is busy).
* **The client is process globals**: 27 statics in `frontend/src/turu.rs` (`TABS`, `ARMY`,
  `MONITOR_HUB`, `SCRIPT_REPO`, `STARTUP`, and since process tabs `SHELL_TABS`, `SHELL_ACTIONS`,
  `SHELL_CLIENTS`, `SHELL_ARMY`, `UPDATE_REQUESTS`, ...). There is no handle to hold.
* **The desktop shell is winit.** `app/src/shell.rs` mixes the shell's logic with winit's event
  loop and window; Android's shell is already split (`slots::SlotShell` in Rust, `SlotShell.kt`
  carrying out its actions).
* **The tab strip is drawn by each client** (`gui/turu/tab_strip.rs`, from the shell's list;
  `turu::tab_strip_count` gives the bar its height), with no way for the shell to say it draws the
  strip itself.

### What is native already

On Android: the text field above the keyboard, the crash question, file pickers, the installer,
notifications, the share sheet (`docs/ANDROID_NATIVE_UI.md`), and since M3 the "Starting the
tab..." view the shell shows while no slot draws (a plain Android view in `SlotShell.kt`). They
reach Rust through hand-written JNI bound in each app's `java.rs`.

## Goal

* A native shell per platform where one is planned (Android now; macOS after the checkpoint),
  designed for that platform: its own layout, navigation, grouping, flow and names where the
  platform's conventions call for them, not VibeSkua's or Turu's egui UI. Android can use bottom
  navigation, sheets and one-handed reach; macOS a menu bar, toolbars, inspectors and windows
  that leave the game's window. Every feature stays reachable (natively or in egui), and the
  platform's own controls bring text input, lists, menus, dialogs and accessibility.
* Every ported feature's state and actions in a plain Rust model in the client, with no egui.
* No change to the engine's behaviour, the script API, the control API's existing routes, the
  shell link's existing messages or the data directory.

Not goals: an iOS app; a web UI (the monitor is that); native shells on Windows and Linux; Turu Mas;
changing how the game is drawn (`docs/GL_RENDERER_PROPOSAL.md` is independent).

## What it costs

Every window exists once today. With native shells a ported feature exists once per shell, and a
new feature's screen is written once per shell too. That is the cost the 2026-11-16 decision
avoided. What keeps it bounded:

* **Steps with a checkpoint.** L5a costs no view models and no binding. The owner sees native and
  egui side by side on a phone before L2, the large step, is taken on.
* **View models.** What a feature knows and does moves to Rust, once (L2); a shell's screen is
  layout and bindings, free to arrange features its own way. How much of the 31,700 lines is
  logic isn't known; L2's pilot measures it on three features before the rest is sized.
* **egui for good, for the tools few use.** Packet Logger, Interceptor, Spammer and Diagnostics
  stay egui in every shell. The daily features are ported: scripts, logs, options, accounts, tabs
  and Army. The rest is decided feature by feature as a shell reaches it.
* **Two native shells at most.** Android and, after the checkpoint, macOS. Windows and Linux stay
  on the egui desktop, which keeps working from the same libraries.
* **The desktop shell's logic is written once** (`turu_shell`), not once per native shell.

What it buys: on Android, real text input everywhere, system lists, menus and scrolling, TalkBack,
no keys-only keyboard path, and screens laid out for a phone instead of a desktop app's windows
shrunk to fit; on macOS, a menu bar, windows that leave the game's window, system text fields and
shortcuts. Each shell can also rethink flows VibeSkua shaped around one Windows window (where
settings live, how a script is found and started, how tabs and accounts are managed). Under
process tabs also: the shell's own windows (the tab strip, Accounts) stay up while a tab's client
restarts. What it doesn't buy: speed. The windows aren't a measured cost.

## Design

### Three places code runs

```
shell (window or activity; no game)               client (one per tab: player, bot, models)
  native screens: tab strip, Accounts, ...   <--- control API: views, /events ---  view models
  the shell's own state (tabs; on the        ---- control API: actions -------->  engine, scripts
    desktop updates, army clients)           ---- link: focus, answers -------->
  shows the client's picture                 <--- link: questions, alerts ------  egui windows not
                                             <--- shared surface / Surface -----    ported, in its
                                                                                    picture
Turu Mas (one AIR process, several bots): egui, as now.
```

* **The client** owns the models. A model reads cached state, never calls Flash, and changes by
  revision (below).
* **The shell** owns what it owns today (both: the tab list and supervision; the desktop: updates,
  Army launches, the monitor). Its UI layer keeps the last copy of each client feature a native
  screen shows, only from the tab on screen. Questions and alerts come from every tab (a hidden
  tab's script can ask). A question asked while no shell is there (on Android the activity may be
  gone) waits in its client, which sends it again when a shell connects; Android's alerts post a
  notification while the app isn't on screen, as now.
* **A shell's screen binds to an interface of its own language** that a copy satisfies, so
  previews and UI tests run on canned views without a client.

### How a copy reaches the shell (decided: the control API)

Each client's control API carries the features' copies and actions; the shell link carries what
the shell owns and what must stay in order with the tab's other messages.

* **Copies over the control API.** A model's view is a route (`GET /models/<name>`, a sketch) and
  its revision an event on `/events`, so a screen reads again only when its feature changed;
  line-fed models (Logs) give the lines after a revision, never the whole buffer each time.
  Actions are routes too, where the API has none already (Script Loader's Start is
  `/script/start` today). The shell's UI layer (Kotlin, Swift) reads them itself over loopback
  with the client's token, from the address the client's `Hello` gives (on Android also
  `slots/c<N>.api.json`); a client that restarts says hello again from a new port and the shell
  follows. No size cap (the link's JSON message is at most 1 MB, `shell::MAX_MESSAGE`, and on
  Android it rides a `Bundle` under Binder's limit, dropped when too big), the same on both
  platforms, readable with no shell (tests, the web monitor).
* **What stays on the link**: the tab list and status, focus between native screens and egui,
  questions and their answers (`Answer { id, value }` is there already), and the area a client
  draws in. These need ordering with input and the picture, and Binder's death notice on Android.

To build (L0 tries it first): `control_api.rs` lives in `turu_bot`, below the client library, so
the client needs a way to add its models' routes and events to the API it starts (a registry of
handlers the API consults, filled by `turu_client`); and an HTTP and SSE reader in the shell's
Kotlin and Swift, ideally the platform's own (`HttpURLConnection`, `URLSession`) to add no
dependency.

Every view and action derives serde. Kotlin and Swift types for them are generated from the Rust
records in the build, never written by hand or committed ("Types across the boundary").

### The shell's own windows come first (L5a)

On Android: the tab strip (drawn in each client from the shell's `Tabs` today) and Accounts opening
a tab (the accounts list is a file any process reads; the shell keeps the Keystore's copies of
passwords), beside the "Starting the tab..." view, native already. They need no client model, so
they need neither L2 nor a generated binding: the shell's Kotlin reaches its Rust through
`java.rs`, as now. They test the shell's half (layout, the client picture in the area left, input
split, focus between Compose and a slot's egui) before any model crosses.

A client whose strip the shell draws must stop drawing it and give the bar its height back: the
shell says so on the link (a field of `Tabs` or `Show`, a sketch), and `turu::tab_strip_count`
reads it. The client is told its area by the shell (on Android the `SurfaceView`'s size), so the
game and the remaining egui windows draw in what's left.

On the desktop the same windows (the strip, Accounts, the "closed" tab with Restart, the Update
window, Army's launched clients) come first in L7, after the checkpoint.

### Design pass

Before any screen of a shell is built, a design pass for it: its screens, navigation, grouping and
names, starting from the platform's own patterns and mapping Turu's features onto them. Names stay
recognisable ("Script Loader", "Army", "Accounts") unless the platform's convention clearly wins,
so guides, a script's instructions and users helping each other still match. The pass is drafted
(by an agent session or a person) and approved by the owner, and `docs/UI.md` records the result:
per feature, where each shell puts it.

### The desktop shell as a library (L1, second part)

L1 moves the logic of `app/src/shell.rs` into `turu_shell` without winit, as `slots::SlotShell` is
for Android: starting and restarting clients, the tab list and `Status`, Grid View's layout and
clicks, shared surfaces (`shared_surface.rs`, `shared_gpu.rs`) and the pacing protocol, the
dialog queue, updates, Army launches, the monitor's peers and the forwarding API. The winit shell
becomes one front over it, with tests that need no window; a native macOS app would be another,
handing the library a layer to draw the clients' pictures into and forwarding its events.

Android's shell and the desktop's stay two cores: they start clients differently (services
against processes) and hand pictures over differently (a `Surface` against shared textures).
What they share is the link and its messages, already in `frontend/src/shell.rs`.

### The game view (shell side)

On Android nothing changes: the slot on screen draws into the activity's `Surface`, and Compose
sits over or beside the `SurfaceView` in the same activity.

On macOS (L6, after the checkpoint) the native window gives `turu_shell` a `CAMetalLayer` and its
size and scale; the library draws the tab's picture, or Grid View's tiles, into it with wgpu as the
winit shell does (`movie_texture::Blit`), keeps `Pace` / `Refreshed` / `Release`, and takes the
window's pointer, keys, text, IME and gamepad events in the link's forms. The window lays its own
controls around the layer and hit-tests them first. A lighter route exists: an IOSurface can be a
`CALayer`'s contents, with no wgpu in the shell; whether that keeps frame interpolation's
every-refresh presents and knows when Core Animation is done with a surface (the `Release`) is
unknown, to try before L6.

### egui beside native screens

While a feature is in egui, the client draws it in its picture and takes input for it, as now.
The shell sends input to the client only where no native view took it. Focus between the two goes
through the link: a native field taking the focus tells the client to drop egui's, and an egui
text box taking it tells the shell (on Android it shows the native strip, as today). Escape and
Back go to whichever has the focus.

### The shape of the API (after the checkpoint)

```rust
// turu_client: plain Rust. A sketch, not the final names.
pub struct Client { /* a handle over the process's client */ }

impl Client {
    pub fn start(config: Config, host: Arc<dyn Host>) -> Result<Arc<Client>, StartError>;
    pub fn events(&self) -> Events;                        // question asked, alert, ...
    pub fn answer(&self, id: QuestionId, answer: Answer);
    pub fn scripts(&self, tab: TabId) -> Arc<ScriptsModel>;
    // ... one accessor per feature; tabs for Turu Mas
}

impl ScriptsModel {
    /// The feature's state now: plain data, serde, cheap, never calls Flash.
    pub fn view(&self) -> ScriptsView;
    /// Bumped whenever `view()` would differ.
    pub fn revision(&self) -> u64;
    /// Actions return at once; the work runs on a thread of the engine's.
    pub fn load(&self, path: String);
    pub fn start(&self);
    pub fn stop(&self);
}
```

* **Models follow features, not egui's windows.** A model is what a user can see and do about one
  thing (scripts, logs, a form of options, the bank), with no layout in it: no tabs of a window,
  no order of fields beyond what the data has, no strings that only make sense in one layout. A
  shell may put two models on one screen, split one across screens, or leave a part out of its
  main flow. egui's windows are one arrangement of them.
* **Reads are snapshots** of cached state; any thread may read, the UI thread included.
* **Changes are revisions**, coalesced to at most one per frame's worth of time. egui reads
  `view()` every frame; a shell reads its copy again when `/events` says the revision moved and
  lets its toolkit diff. No per-field deltas, except line-fed models.
* **Actions never block**: every one posts work, as `turu::spawn_action` does. The rule "never call
  into Flash from the UI thread" becomes part of the API.
* **Events are queued**: nothing reaches the shell from inside `Bot::on_flash_call`.
* **`Host` is today's `Platform`**: in a client it forwards to the shell, as now; in Turu Mas the
  ANE implements it.

### Commands stay in the engine

The windows and the control API already share their commands (Script Loader's Start and
`/script/start` both call `turu_bot::script::start`). What a window adds is what to show and when
an action is allowed; that is what L2 moves into models. A model or a shell adds no engine
behaviour: what a screen needs and `turu_bot` lacks is added to `turu_bot`, where the control API
and the monitor can use it too.

### Crates

```
bot/        turu_bot       the engine (as now); its control API takes routes the client adds
client/     turu_client    most of turu_frontend: the tab(s), Flash host, engine loop, Host (today's
                           Platform), the link's messages (frontend/src/shell.rs, its key table
                           behind `ruffle`) and client end, Army, the monitor hub, updates, view
                           models and their routes. No egui, winit or rfd. Feature `ruffle`.
egui_ui/    turu_egui      frontend/src/gui/turu and client_screen.rs's UI: the egui windows, on
                           turu_client's models
shell/      turu_shell     on turu_client built without `ruffle`: Android's SlotShell, the desktop
                           shell's logic without winit (its GPU setup from app/src/shell.rs and
                           app/src/mugetsu.rs, the shared surfaces). No player, no egui.
ffi/        turu_ffi       macOS only (L4): BoltFFI's annotations, written out, over turu_shell
app/        turu           the egui desktop: winit over turu_shell, and turu --client
android/, air/ane/rust     as now; android/ adds Compose in the shell
apple/                     the macOS shell (L7)
```

The dependencies run one way: `turu_shell` on `turu_client`, never back; a client's process links
no `turu_shell`. The shell takes the client library without `ruffle` as Turu Mas does, since it
uses the link's messages, Army, the monitor hub and updates from it but runs no game. Where the
winit shell's key and gamepad translation uses Ruffle's types today (`app/src/shell.rs`), it moves
to the link's names. L1's second part (`turu_shell`) can come before the client split: it takes
`turu_frontend` without `ruffle` until `turu_client` exists.

### Bindings, by platform

| Platform | UI | Rust from the shell's UI | Client models |
|---|---|---|---|
| Android (Turu) | Compose in the activity (the shell) | `java.rs`, hand-written, as today | JSON over the slot's control API, Kotlin types generated |
| macOS | SwiftUI + AppKit, the shell | BoltFFI Swift over `turu_shell` (tabs, input, the game layer) | JSON over the client's control API, Swift types generated |
| Windows, Linux | egui | none | none |
| Turu Mas | egui | none | none |
| Headless, Docker | none | none | none |

### Types across the boundary (decided: JSON, types generated)

Views and actions cross as JSON, the control API's format. Kotlin and Swift types come from the
Rust records by a type generator run in the build (candidates, from memory and to confirm in L0:
`typeshare`, or the generator Crux uses, `facet-generate`), never committed, so a record changed in
Rust can't ship with stale types in the shell. A mismatch fails loudly in decoding, logged with the
route, rather than showing wrong data. Typed records through an FFI were set aside: every view
already crosses a process as JSON, so decoding it in the shell's Rust for a binding to encode again
adds a step and leans on the youngest part of BoltFFI.

### BoltFFI and Crux (as read on 2026-11-15, not re-read; macOS only)

BoltFFI is needed only for macOS's shell to host `turu_shell`, a small surface (the tab list,
input, the layer, the shell's events). As read: v0.32.1 (2026-10-30), MIT, created December 2025,
releases every week or two, before 1.0: pin the crate exactly and the `boltffi` CLI to the same
version, checked by the build. Swift listed as full support; exports records, functions, classes,
callback traits (`Send + Sync` across threads), errors, `async fn` and streams; `boltffi pack`
writes the XCFramework. At 0.30 it read source without expanding macros or evaluating `#[cfg]`;
`turu_ffi` has no feature-gated exports. Crux (`github.com/redbadger/crux`), a Rust core with native
shells, moved to BoltFFI in May 2026 and builds an XCFramework with macOS through it. From Crux: a
callback that hopped back to the UI thread inline re-entered the core mid-call (so the library
never calls Swift from inside an exported call), shells that talk to a small protocol preview
without the library, and UniFFI is a known way back if BoltFFI fails.

### Rules that change

When L5a ships, in `CLAUDE.md`:

* **"Same UI/UX and flow as VibeSkua"** becomes: the egui UI keeps VibeSkua's layout and flow;
  each native shell follows its platform's conventions and its approved design pass, and may change
  layout, navigation, flow and names, as long as every feature stays reachable and behaves the same.
  Behaviour still follows VibeSkua's C# ("Behaviour follows VibeSkua" is unchanged): that lives in
  the engine and the models, not in a shell. `docs/UI.md` records, per feature, where each shell
  puts it.

Later, with the steps that need them:

* **"Kotlin and Rust meet only through each Android app's `java.rs`"** stays as it is on Android
  (no generated JNI). Kotlin and Swift types for the models are generated in the build, never
  committed (L4).
* **"No winit, rfd or window in `frontend/`"** becomes: none of those and no egui in
  `turu_client` or `turu_shell`, checked in CI with `cargo tree` (L1).
* The threading rules stay; the API enforces the first.

## Milestones

Each ends with the three frontends building and `cargo nextest run -p turu_bot` and the app's tests
passing. Sizes are relative (S, M, L, XL), from the files each touches; none is measured.

### L0: The spike (S, done 2026-11-20)

Throwaway code on a branch, answering what L5a and the checkpoint need:

1. **Compose in the shell**: what it adds to the Android shell's process (~45 MB PSS on the
   emulator today) and to the APK; a `ComposeView` over the `SurfaceView` with a slot drawing under
   it, through a tab switch (the surface hand-over) and with the native text strip shown.
2. **A copy over the control API**: one feature (scripts: its status, what's loaded, Start and
   Stop) served by a slot as a route and an `/events` revision, read from the shell's Kotlin with
   the platform's HTTP client: the view's size, the time from a change in the slot to the Compose
   view redrawn, the slot killed and back on a new port, and how `turu_client` adds the route to
   `turu_bot`'s API. Also the sizes of Logs with a full buffer and the monitor's `Snapshot`.
3. **Generated types**: one record (`Snapshot`, 40 fields with lists and enums with data) to
   Kotlin and Swift through a type generator, and how a field changed on one side fails.

Before L6 (after the checkpoint), not now: the macOS game view (`turu_shell` into a `CAMetalLayer`
handed over by Swift at the display's rate with frame interpolation, or an IOSurface as a
`CALayer`'s contents), and BoltFFI's Swift binding over a `turu_shell`-shaped API (which thread
callbacks arrive on, binary size, build time).

**Done when** answers 1 to 3 are written into this document's status, with a go for L5a, or what
to change, or a stop.

#### L0's answers (2026-11-20)

Measured on the Android 16 arm64 emulator (`-gpu host`, Apple M5) with two slots in game, debug
APK, arm64 only. The spike's code is on the branch `native-ui-l0` (commit `2d3233730`), not on
`main`. **Go for L5a**, with the changes below.

1. **Compose in the shell works.** A `ComposeView` over the `SurfaceView`, in the activity's
   `FrameLayout`, draws over the slot's picture; taps on it reach Compose when
   `TuruActivity.dispatchTouchEvent` lets a gesture that starts on it go to the views (as the
   text strip's do) and every other touch goes to the slot. It stayed up through a tab switch
   (the surface handed from slot 1 to slot 3 in 20 ms, as before) and beside the native text
   strip with the keyboard up.
   * **Memory**: the shell's PSS went from 53.5 MB to 77.3 MB (+23.8 MB). Almost all of it is
     the activity's hardware renderer, not Compose: a plain `TextView` in the same place cost
     74.9 MB (+21.4 MB; "Unknown" in `meminfo` goes from 11 to 32 MB, which is the renderer's
     buffers). Compose itself adds about 2.4 MB. So: **the first view drawn over the surface
     costs ~21 MB**, whatever toolkit draws it. The "Starting the tab..." view pays it today only
     while it shows. The shell is still the smallest process (a slot is 380 to 480 MB).
   * **APK**: dex grew from 10.6 MB to 32.2 MB uncompressed (+21.6 MB; +7.3 MB in the APK) in a
     debug build without R8. Release builds have `isMinifyEnabled = false`, so they pay the same
     until R8 is turned on for the shell's classes (to measure in L5a).
   * **Compose's version**: the BOM 2026.09 (Compose 1.12) needs compileSdk 37; the SDK has 36,
     so 2026.06.01 (Compose 1.11). Moving to 37 is a separate change.
2. **A copy over the control API works and is fast.** A registry in `control_api.rs`
   (`add_model(name, view)`, filled by the client before it starts the API) serves
   `GET /models/<name>` (`{revision, view}`) and `event: model` on `/events?models=<name>` when the
   view's revision moves (the revision is a hash of the view: no bookkeeping in the model). The
   shell reads with `HttpURLConnection` on a thread, from the address in the slot's `Hello`;
   loopback HTTP needs a network security config allowing cleartext to `127.0.0.1` only.
   * The scripts view is 154 to 254 bytes and reads in 1 to 4 ms (17 ms the first time).
   * From a tap on a Compose Start or Stop to the Compose view redrawn with the new state: **50 to
     270 ms**, bounded by `/events`' 250 ms poll (`EVENTS_EVERY`). Fine for status; a model that
     needs less (a script's own message box) can poll faster or push.
   * A slot killed with its copy shown: the reader's stream ends (`EOFException`), its retries
     fail for 2 s, the slot says hello again from a new port and the shell follows it. The copy
     was current 2 s after the kill.
   * Sizes for later: the monitor's `Snapshot` is 698 bytes; a log line about 200 bytes, so a full
     buffer (5,000 lines) about 1 MB (estimated from 6 lines, not measured full): Logs must stay
     line-fed, as planned.
3. **Generated types: facet-generate over typeshare.** Both ran on `Snapshot`'s records
   (scratch crates, not committed).
   * **typeshare 1.13.4** dropped `Snapshot` and every record with an `i64`, `u64` or `usize`
     field, with an error each (it refuses 64-bit integers for JavaScript's sake unless every such
     field says `#[typeshare(serialized_as = "...")]`). Turu's records are full of them (gold,
     XP, counters). Only `ScriptInfo` and `BotState` came out.
   * **facet-generate 0.22.4** (Crux's generator) produced every record for Kotlin (with
     kotlinx.serialization: `Long`, `ULong`, enums with `@SerialName`) and Swift (`Codable`),
     plus a runtime package each (`com.novi.serde`, `Serde`). It needs `#[derive(Facet)]` (facet
     0.46, pinned exactly by the generator) on the records beside serde's, and its own
     `#[facet(rename_all = ...)]`. Two traps: serde's `#[serde(default)]` on a struct isn't seen,
     so a field Rust may leave out is required in Kotlin; and kotlinx.serialization refuses
     unknown keys unless the `Json` has `ignoreUnknownKeys = true`, which a shell must set
     (shell and client are the same build, but a missing field must not fail the whole view).
   * A field changed on one side fails decoding loudly in kotlinx (missing or wrong type), which
     is what "Types across the boundary" wants. Decoding a slot's live view in the Android build
     is L4's, after the checkpoint.

What changes for L5a: nothing in its plan. It needs only Compose and `java.rs` (no copies, no
generated types). L5a also turns R8 on for the release build if the shell's dex is still +20 MB
there, or records why not.

### L5a: Android, the shell's own windows (M, planned after L0)

* The design pass for Android (above), approved by the owner: at least the tab switcher, Accounts
  and where the rest of the app's navigation will go. **Approved 2026-11-20**, recorded in
  `docs/UI.md` ("Native shells", "Android"): landscape only; Material 3 on egui's colours; the
  tabs as one item at the top of the rail (a landscape phone has room for one, not a strip),
  opening a panel beside it that holds the tabs, New tab and Accounts; the whole rail native in
  L5b. It adds to the link: the shell drawing the tabs, `rail_top`, `waits` and `rail_hidden` in
  `Status`, and `ToShell::OpenAccounts`.
* The tab strip in Compose, from `slots::SlotShell`'s list (open, select, close, a down tab with its
  reason), and the clients told not to draw theirs: as the design pass shapes it, the Tabs item
  and its panel, with the client's rail starting below it.
* Accounts opening a tab in Compose (the accounts list, Start in a tab), through `java.rs` and the
  shell's existing open-tab path.
* Focus between Compose and a slot's egui windows; Back closes what's on top, native or egui.

**Done when**, on the Android 16 arm64 emulator (`-gpu host`) with two slots: the strip and Accounts
are Compose; opening, switching and closing tabs work from them; the game and the egui windows draw
in the area left; a slot killed while shown comes back in the same place; the egui text boxes still
open the native field. Then the **checkpoint**: the owner tries it and decides whether to go on.

#### L5a as built (2026-11-20)

On the Android 16 arm64 emulator with two slots in game: the Tabs item and its panel, New tab and
Accounts are Compose; opening (an account or a blank tab), switching and closing tabs work from
them; the game takes the full height and the slot's egui rail starts below the item; a slot killed
while shown comes back in the same place, the item and panel staying up meanwhile (they sit above
the "Starting the tab..." view); the game's text field still opens the native strip beside the
panel; Back and the scrim close what's on top. Not on a phone yet (M4). What differs from the
design pass, and what isn't built, is in `docs/UI.md` ("Android, as built"). Found on the way: the
shell's process had no rustls crypto provider (the server list's fetch aborted it); it installs
one now, and the fetch can't panic the shell.

Not done in L5a: R8 for the release build (the shell's dex is still +20 MB without it), the
long-press selection in Accounts, and Back closing a slot's egui window (Back still sends the app
to the background when no native screen is open).

### L1: Libraries without egui or winit (M)

* **The desktop shell (planned, any time)**: move the logic of `app/src/shell.rs` into
  `turu_shell` (with `frontend/src/slots.rs`'s `SlotShell`); the winit shell keeps only the window,
  its events and rfd. The shell's tests run without a window, as `SlotShell`'s do.
* **The client (after the checkpoint)**: split `frontend/` into `client/` and `egui_ui/`:
  `client_screen.rs`'s egui drawing to the egui side; `client_link.rs` to the client once it asks
  a model, not `gui`, for the bar's size and whether the tab is busy. Rename `Platform` to `Host`.
  Add `Client` as a handle over the globals.

**Done when**, for the shell part: `turu_shell` builds without a player and without winit, CI
checks it, and the shell starts with two tabs, Grid View and a client killed and back. For the
client part: `turu_client` builds with and without `ruffle` and has no egui, winit or rfd, and the
three frontends pass their tests and start.

### L2: View models (XL, after the checkpoint)

Move each feature's state and what it decides into `turu_client`, cut by feature rather than by
egui window where the two differ, with its route on the control API. First what every shell needs:
the list of features (what `Host` hides, which are enabled; egui shows it as its nav menu), and
hotkeys (today `hotkeys::run_pressed` runs inside egui's frame). Then pilot on three features of
different kinds: scripts (a status and commands), logs (lines arriving) and game options (a form
that saves). Record for each how many lines stayed in egui and how many moved: that ratio sizes the
rest. Then the daily features (accounts, tabs, Army, the rest of options), in the order the Android
design pass wants them. Packet Logger, Interceptor, Spammer and Diagnostics stay egui and get no
model unless a later need gives them one.

Each model gets tests on `turu_bot::testing`'s fake Flash host, with no UI, and its route a test
through a real control API.

**Done when**, per feature, its egui code imports nothing from `turu_bot` but record types and its
model has tests; egui's nav bar and hotkeys run on the models; every feature in `docs/UI.md` is
marked moved or kept in egui (with a reason).

### L3: Questions, dialogs and alerts as events (M, after the checkpoint)

What the engine asks of the user, each by its own route today: a script's message box
(`bot.showMessageBox`), Edit Script Options (polled each frame), "Send crash reports?", the Tab
Limit notice, tab requests. One stream of events with an ID each, and `answer(id, ...)`, carried on
the link to the shell. The link's `Answer { id, value }` and the picker and installer answers fold
into it. While a feature is in egui, the client answers in egui as now; a native shell shows its
own dialog, for any tab, naming it.

**Done when** egui polls nothing but models and the event stream, and a test with no UI answers a
script's message box and options request.

### L4: Generated types and `turu_ffi` (M, after the checkpoint)

* The type generator L0 chose, run by the Android build (and later the Xcode build) over the view
  and action records; the generated Kotlin and Swift are build outputs. A CI check that every
  model's records generate.
* For macOS, when L6 starts: `turu_ffi`, BoltFFI's annotations over `turu_shell`, pinned with the
  CLI's version checked, and `boltffi pack` in CI.

**Done when** the Android build decodes a slot's scripts view into generated types in a test, and
(for macOS) a Swift test reads `turu_shell`'s tab list.

### L5b: Android, the daily features (L, after L2 to L4)

The navigation and screens from the approved design pass, on copies of the slot's models: scripts,
logs, options, accounts, tabs and Army, in the order the pass gives. Touches on a Compose view stay
in the activity; the rest go to the slot as now. A ported feature's text fields are real ones and
drop their `native_field::follow` path. Features not ported still open in egui, in the slot's
picture.

**Done when**, on the emulator with two slots: a script can be found, loaded, started and stopped
without an egui window; switching tabs switches what the Compose screens show; a slot killed with
its screen open shows it again, current, once the slot is back. On a phone too if M4 has given the
shell's memory there.

### L6: The macOS game view behind a native window (M, after the checkpoint)

`turu_shell` draws into a layer a Swift window hands it (the route L0's later questions picked),
with input, IME, the cursor and gamepads in.

**Done when** a minimal Swift window shows two tabs' clients, switches between them, logs in with
the game's own screen, takes the mouse and keyboard, plays sound (each client's own), interpolates
frames, and shows a client's egui window in its picture.

### L7: macOS (L, after L6)

`apple/`: an AppKit app that is the shell, from its own design pass: the game's layer, the menus
in the menu bar and a toolbar, and the features as SwiftUI laid out the Mac's way, on copies of
the clients' models. The shell's own windows first (the strip, Accounts, the "closed" tab, the
Update window, Army's launched clients). It starts the bundled `turu` binary as clients. Signing,
notarisation and updates (the shell installs Turu's updates today: the bundle with the Swift app in
it is what gets swapped) move with it.

**Done when** the features the macOS design pass ports are SwiftUI, the rest show in egui in the
client's picture, and a release build updates itself.

### L8: Windows and Linux (not planned)

Both stay on the egui desktop over `turu_shell`. A native shell for either is a new decision, made
only if someone asks and will maintain it.

### L9: Turu Mas (not planned)

Turu Mas stays egui. AIR's Java 8 tooling, its focus (a native view takes it from the game field),
its overlay over every view and the AIR SDK's license make it the riskiest target for the least
gain. If it is reopened: hand-written JNI as today, bound to the models in-process (several bots,
one process, no control API needed), and Android's Compose screens through `android/shared` if
they get through AIR's `d8`.

## Risks

* **L2 is most of the work and shows nothing new to a user.** The checkpoint comes before it, and
  the pilot's ratio decides whether to go on after it.
* **Shells drift.** Shells differing in layout and flow is intended; differing in what a feature
  does is not. Behaviour lives in the models and the engine, with tests, so a shell can't add its
  own; `docs/UI.md` records where each shell puts each feature, so none is lost.
* **Help and support across platforms.** With layouts and names that differ, a guide, a script's
  instructions ("open Script Options") or a user helping another may describe what isn't on that
  user's screen. Recognisable names, `docs/UI.md`'s per-shell map, and scripts naming the feature
  rather than a place to click keep it small.
* **Two UIs, two processes, one screen**: focus, the keyboard, Back and Escape between native
  screens in the shell and egui in the client. Android's text strip crosses that line today; its
  pitfalls are in `docs/ANDROID_NATIVE_UI.md`. L5a meets it first, on two windows.
* **Copies a process away.** A copy is a revision behind; a slot that dies leaves stale screens
  until it's back on a new port. The control API's local HTTP adds a hop per read; L0 measures it.
* **The control API grows a second job.** It is the scripts' and Army's API today; model routes
  sit beside them under the same token. Model routes are versioned with the app (shell and client
  are always the same build), not promised to outside callers like `docs/HEADLESS.md`'s routes.
* **The shell's process grows.** Android may kill the activity's process first in the background,
  which is fine (slots go on), but a heavier shell is slower to come back. L0 measures Compose.
* **Unverified ground.** Process tabs haven't run on phones (M4) or on Linux. A native shell built
  on them inherits what those runs find.
* **BoltFFI's age** (macOS only): under a year old, no stability promise. Kept to `turu_ffi`;
  pinned; UniFFI as the way back.
* **Licenses.** The libraries are GPL-3.0-or-later, so the shells are too; BoltFFI is MIT.
  Mugetsu's code stays out of Turu Mas's build.

## Decisions taken (2026-11-20, the owner)

1. **How far**: L0 now, then the Android shell's own windows (L5a), then a checkpoint before view
   models and native bot windows. Not "everything" up front. The desktop shell library (L1's
   second part) any time.
2. **How a copy reaches the shell**: each client's control API for the features' copies and
   actions; the shell link for the shell's own state, focus, questions and answers.
3. **First shell**: Android.
4. **Windows**: stays on egui.
5. **Linux**: stays on egui.
6. **The desktop shell**: one library (`turu_shell`) that every shell hosts.
7. **Design**: each shell starts from its platform's patterns, with recognisable names; a design
   pass, drafted and approved by the owner, comes before each shell's first screen.
8. **The fallback**: Packet Logger, Interceptor, Spammer and Diagnostics stay egui for good; scripts,
   logs, options, accounts, tabs and Army are ported; the rest is decided as a shell reaches it.
9. **Turu Mas**: stays egui; if ever reopened, hand-written JNI.
10. **Across the boundary**: JSON, with Kotlin and Swift types generated from the Rust records.
    Android needs no generated FFI binding; BoltFFI only for macOS.

11. **The checkpoint** (2026-11-20): go on to Android's bot windows (L1's client part, L2 to
    L5b). macOS (L6, L7) is not decided. The work stacks on the branch `native-ui`, one PR later.
12. **Order after the checkpoint** (taken by the session doing it, 2026-11-20): L2's pilot comes
    before L1's client split. The pilot's models live in `turu_frontend::models`, which imports no
    egui (a test checks the module's sources), so the split later moves them as they are; the
    pilot's ratio says how much the split and the rest are worth first.

Still open:

* **Why it was withdrawn on 2026-11-16.** Not recorded; worth writing here if it bears on the
  checkpoint.
* **macOS** (L6, L7): not decided at the checkpoint.
* **Who drafts each design pass** (an agent session or a person); the owner approves either way.
  Android's was drafted by an agent session.

Order: L0 first; L5a after L0 and Android's design pass; L1's shell part any time; after the
checkpoint, L1's client part, then L2 and L3, then L4, then L5b; L6 after L1's shell part and the
checkpoint, L7 after L6.

## Docs that change with each step

* Now: `docs/TAB_PROCESSES_PROPOSAL.md`'s "Not in this proposal" and `docs/ANDROID_NATIVE_UI.md`'s
  "Kept in egui" point here for what is planned.
* With L5a: `CLAUDE.md`'s "Same UI/UX and flow as VibeSkua" ("Rules that change");
  `docs/ANDROID_NATIVE_UI.md` lists the strip and Accounts as native; `docs/UI.md` starts its
  per-shell map from the design pass.
* With L1: `CLAUDE.md`'s layout table (`turu_shell`, later `turu_client` and `turu_egui`) and the
  "No winit, rfd or window" rule.
* With L4: `CLAUDE.md` on generated types.
