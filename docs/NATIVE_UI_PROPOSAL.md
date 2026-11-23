# Proposal: native windows on each platform, over process tabs

Status: **reopened (2026-11-20), in steps. L0 and L5a are done (2026-11-20, on the emulator; "L0's
answers" and "L5a as built" below). Checkpoint passed (2026-11-20): go on with Android's bot
windows (L1's client part to L5b); macOS waits for its own decision. L2's pilot, L4 (Kotlin types)
and L5b's windows are done (2026-11-20; "L2's pilot", "L4 and L5b so far" below): every Android
window the pass made native, then past it Notification Center, Accounts' selection and groups,
Stats, Current Drops, Skills, Loadouts, Fast Travel, Butler, Cosmetics, Script Options, Runtime, Notify Drop, Loader, Grabber, Junk Items, Console and Update Turu. The branch `native-ui` is on
`main` since 2026-11-22, and the typed actions, answers and JNI (2026-11-23) went on `main`
directly. L1's shell part is built (2026-11-23, macOS; "L1's shell part as built"): `turu_shell`. Continuing: "Handover" under L5b.** Proposed on 2026-11-15, reconciled with process tabs on 2026-11-16 and withdrawn the
same day (`55737175c` records the decision, not a reason). Reopened on 2026-11-20, now that process
tabs are built, with the owner's decisions in "Decisions taken". Planned:

1. **L0**, a narrowed spike: what Compose costs in the Android shell, and copies of a client's
   models read over its control API.
2. **L5a**, after L0 and a design pass: the Android shell's own windows (the tab strip, Accounts
   opening a tab) in Compose, beside the bot windows, which stay egui. The design pass is
   approved (2026-11-20) and recorded in `docs/UI.md`, "Native shells".

Then a **checkpoint**: the owner decides from L5a whether to go on to view models and native bot
windows (L1 to L5b) and to macOS (L6, L7). Planned any time, as it pays without native UI: the
desktop shell's logic as a library (L1's second part; built 2026-11-23). Not planned: native shells on Windows and
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
| L1 (shell) | The desktop shell's logic out of winit, into `turu_shell` | Built 2026-11-23 (macOS), `SlotShell` with it |
| Checkpoint | Go on to native bot windows, or stop at L5a | Owner, after L5a |
| L1 (client) | `turu_frontend` split into a client library without egui and the egui UI | Split built 2026-11-23 (`turu_egui`); `Host` and `Client` later |
| L2 | View models in the client, by feature | Pilot and the Android windows' models done 2026-11-20 |
| L3 | Questions, dialogs and alerts as events with an ID and an answer | After the checkpoint |
| L4 | Generated types for Kotlin and Swift in CI; `turu_ffi` for macOS | Kotlin done 2026-11-20 (Android build); Swift with L6 |
| L5b | Android: navigation and the daily features in Compose | Built 2026-11-20 (emulator); on `main` since 2026-11-22 |
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
  *Done 2026-11-23: `turu_shell` holds that logic; `app/src/shell.rs` keeps the window, its GPU
  choice, input translation, gamepads and rfd ("L1's shell part as built").*
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
  shell logic (`slots.rs`: `SlotShell`, plain Rust, tested on the desktop; in `turu_shell::slots`
  since 2026-11-23) and the egui UI.
* Three frontends use it: `app/` (the shell, `turu --client`, `--headless`, a one-player window
  for a plain movie), `android/` (the shell in the activity's process, slots as services) and
  `air/ane/rust` (Turu Mas, built without the `ruffle` feature).
* Three consumers have no egui: the control API (`bot/src/control_api.rs`: some 40 routes and
  server-sent events at `/events`; every client has one, and the shell forwards `/tabs/<id>/...`
  to them), the web monitor (`bot/src/monitor/`, whose `Snapshot` is a serialisable picture of a
  bot) and the shell link. All three already carry a bot's state out of its process.

### What isn't

* **The windows are egui and their logic lives in them.** `egui_ui/src/gui/turu/` is 47 files,
  31,700 lines. A window keeps its own state, reads the engine each frame and decides what a click
  does, calling `turu::bot()`, `tab_bot` or `spawn_action` directly.
* **`turu_frontend` depends on egui at its root** (`Cargo.toml`; `gui.rs`, `client_screen.rs`,
  and `client_link.rs`, which asks `gui` for the bar's size and whether the tab is busy).
* **The client is process globals**: 27 statics in `frontend/src/turu.rs` (`TABS`, `ARMY`,
  `MONITOR_HUB`, `SCRIPT_REPO`, `STARTUP`, and since process tabs `SHELL_TABS`, `SHELL_ACTIONS`,
  `SHELL_CLIENTS`, `SHELL_ARMY`, `UPDATE_REQUESTS`, ...). There is no handle to hold.
* **The desktop shell is winit.** `app/src/shell.rs` mixes the shell's logic with winit's event
  loop and window; Android's shell is already split (`slots::SlotShell` in Rust, `SlotShell.kt`
  carrying out its actions). *Split since 2026-11-23: `turu_shell` and its winit front.*
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
  A model's actions are one route, `POST /models/<name>/do`, whose body is one of the model's
  actions (its `…Act` enum, the action's name in `action`; since 2026-11-23). The shell's UI layer (Kotlin, Swift) reads them itself over loopback
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
egui_ui/    turu_egui      egui_ui/src/gui/turu and client_screen.rs's UI: the egui windows, on
                           turu_client's models
shell/      turu_shell     on turu_client built without `ruffle`: Android's SlotShell, the desktop
                           shell's logic without winit (its GPU setup from app/src/shell.rs and
                           app/src/mugetsu.rs, the shared surfaces). No player, no egui.
                           (Built 2026-11-23 on turu_frontend without `ruffle`: the desktop
                           shell's logic and its pictures on the GPU; the GPU's choice, the shared
                           surfaces' platform code and SlotShell stay where they were.)
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
     so 2026.06.01 (Compose 1.11). Moving to 37 is a separate change. *Done 2026-11-21: BOM
     2026.11.01 (Compose 1.13, Material 3 1.5.0), the app on compileSdk 37.1 (docs/UI.md, "Look").*
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

* **The desktop shell (built 2026-11-23, below)**: move the logic of `app/src/shell.rs` into
  `turu_shell` (with `frontend/src/slots.rs`'s `SlotShell`); the winit shell keeps only the window,
  its events and rfd. The shell's tests run without a window, as `SlotShell`'s do.
* **The client (after the checkpoint)**: split `frontend/` into `client/` and `egui_ui/`:
  `client_screen.rs`'s egui drawing to the egui side; `client_link.rs` to the client once it asks
  a model, not `gui`, for the bar's size and whether the tab is busy. Rename `Platform` to `Host`.
  Add `Client` as a handle over the globals. *The split is built (2026-11-23, "L1's client part
  as built"), keeping the name `turu_frontend` for the client library (the owner's choice); the
  rename and the handle are later steps.*

**Done when**, for the shell part: `turu_shell` builds without a player and without winit, CI
checks it, and the shell starts with two tabs, Grid View and a client killed and back. For the
client part: `turu_client` builds with and without `ruffle` and has no egui, winit or rfd, and the
three frontends pass their tests and start.

#### L1's shell part as built (2026-11-23)

`shell/`, crate `turu_shell`, holds what `app/src/shell.rs` decided: starting and restarting the
clients (1, 4, 16, 60 s; closed after 5 deaths in 10 minutes unless no other tab runs; the
connect timeout), the tab list and `Status`, Grid View's tiles, the clients' pictures on the
shell's GPU (shared surfaces behind a `SharedSurface` trait the front implements, pixels
otherwise, the releases from the GPU waiter's thread), the pacing protocol, the dialog queue,
updates, Army launches, the monitor's and `--api`'s peers, and `TURU_SHELL_TEST`'s lines but
the window's own. Its threads hand `Event`s to a `Sink` the front gives; what the shell wants of
the window comes back as `Action`s the front carries out after each call (title, redraw, input
method, cursor, full screen, a file dialog, the graphics choice, quit), so the library never
calls into the window from inside one of its methods. It takes `turu_frontend` without `ruffle`,
which gained a `gpu` feature for what draws without the player (`blit.rs`, moved out of
`movie_texture.rs`, and `gpu_waiter.rs`). CI checks its dependencies have no winit, rfd or
player, runs its clippy, tests and format.

`app/src/shell.rs` is the winit front: the window and its event loop, its GPU chosen as the
clients choose theirs (`make_gpu`: Mugetsu's backend choice, the DirectX 12 fallback, then
`turu_shell::Gpu::new`), winit's keys, IME and cursor in the link's forms, the gamepads, rfd's
dialogs, Windows' shared handles (copied from the client's process, then handed to the shell),
and Mugetsu's startup and shutdown steps.

Checked: `turu_shell`'s tests (a fake client binary over loopback: two tabs, Grid View on and
off, the tab on screen's client killed, another shown, the dead one back as a new process,
closing; the restart delays and giving up; the title; one file dialog at a time); and the app on
macOS with `TURU_SHELL_TEST`, against the build before the move: two tabs at the login screen,
the same pictures a second, a client killed and back in a second, Grid View from the bar's
button, `dump`s of each, and the window presenting (71 pictures while it was in view, before it
was covered). Not checked: Linux (compiled by CI's clippy, not run), Windows (its shared handles
and the graphics choice compile only in the release workflow: its C dependencies don't
cross-compile from a Mac), frame interpolation's `Pace::Refresh` in game, a file dialog asked by
a tab, and real mouse, keys, IME and cursor through winit (`TURU_SHELL_TEST`'s `click`, `key`
and `text` enter past the front).

Android's `SlotShell` followed the same day: `turu_shell::slots`, with its `Action`s and its
tests; the records, addresses and flags a slot starts with, and the tab list the native screens
read (`NativeTabs`, generated for Kotlin), stay in `turu_frontend::slots`, since the slots'
processes and the typed JNI use them. The desktop shell is the crate's default `desktop` feature
(wgpu, `webbrowser`, `image`); the Android app takes `turu_shell` without it, so it links only the
slots' shell. CI checks the crate both ways. Checked: its tests (the 12 of `SlotShell`, moved
as they were) with and without `desktop`, and the Android app's crate built for arm64; not on
the emulator, as nothing it does changed.

Not done: the game view for a native macOS window (L6) hands `turu_shell` a layer, which
`Gpu::new` takes as any `wgpu::Surface`.

#### L1's client part as built (2026-11-23)

The owner chose (2026-11-23) to keep `frontend/` as `turu_frontend`, now the client library, rather
than rename it `turu_client`, and to do only the split now. `egui_ui/`, crate `turu_egui`, holds
the egui UI: the bot windows (`gui/turu/`, 47 files), `gui.rs`'s colours, and a client's screen
(`client_screen.rs`, its `ruffle` feature), with the symbol font (`assets/`) and Turu Mas's `air`
accent. It re-exports `turu_frontend`'s modules at its root, so the windows still say
`crate::turu`, `crate::models`; the only change inside them is a game options helper that was an
`impl` on a model's type (`field_width`).

What the client asked of the UI moved to the client: where the UI sits around the game
(`layout.rs`: the bar or rail, the tab strip, the cutout, the rail hidden; `turu_egui::gui`
re-exports it) and whether a tab is busy or waits for an answer (`turu::busy`, `waits_for`), so
`client_link.rs` stays in `turu_frontend`. `turu_frontend` has no egui, winit or rfd now, with or
without `ruffle`, and CI checks its dependencies and builds it alone both ways. The app, Android's
app and Turu Mas take `turu_egui` beside it (`turu_frontend::gui` and `client_screen` became
`turu_egui::...`).

Checked: `turu_frontend` and `turu_egui` build alone with and without `ruffle` (Turu Mas's way:
`air`), the typegen binary builds, the app builds, and the Android app's and Turu Mas's crates build
for arm64; clippy passes for every crate, both ways; the tests pass (the app's 56; `turu_egui`'s
58 and `turu_frontend`'s 159, the 217 the crate had); and the app on macOS with
`TURU_SHELL_TEST` drew as before: two tabs, the bar and tab strip, a client killed and back, Grid
View. Not checked: an APK or Turu Mas run after the split (their code changed only its paths).

Not done: `Platform` renamed `Host`, and `Client` as a handle over `turu.rs`'s globals.

### L2: View models (XL, after the checkpoint)

Move each feature's state and what it decides into `turu_client`, cut by feature rather than by
egui window where the two differ, with its route on the control API. First what every shell needs:
the list of features (what `Host` hides, which are enabled; egui shows it as its nav menu), and
hotkeys (today `hotkeys::run_pressed` runs inside egui's frame; since 2026-11-21 the catalog,
bindings, editing and running an action are `models::hotkeys`, and egui only reads the presses). Then pilot on three features of
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

#### L2's pilot (2026-11-20)

Three features of different kinds moved into `turu_frontend::models` (no egui; L1's split moves
the module as it is), each served by the control API (`GET /models/<name>`, `POST
/models/<name>/<action>`, `event: model`), tested without UI and through the API:

| Feature | egui before | egui after | Moved | Model (with its view and actions) |
|---|---|---|---|---|
| Scripts (a status and commands) | 516 | 446 | 70 (14%) | 331 |
| Game Options (a form that saves) | 1,330 | 786 | 544 (41%) | 779 |
| Logs (lines arriving) | 476 | 476 | 0 | 59 (the lines were already the engine's: `/log`, `event: log`) |

So: a feature's logic is 15 to 40% of its egui code, more for forms; line-fed features have
almost none. The rest of the 31,700 lines would move about 25% (8,000 lines) and add a view
and actions per feature. What stays in egui per window: layout, the window's own state (search,
filters, which confirm is pending, what this window edited), and drawing. "Edited here" against
"changed outside" is a window's state, passed into the model, not the bot's.

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

#### L4 and L5b so far (2026-11-20)

* **L4**: `turu-typegen` (`turu_frontend`'s `typegen` feature: facet 0.46.5 and facet-generate
  0.22.4, pinned exactly) writes Kotlin for the scripts view with kotlinx.serialization; the
  Android build's `turuTypes` task runs it into the build folder before the Kotlin compiles, so
  the types are never committed. Views use `String` for paths (facet-generate has no `PathBuf`).
  The game options and logs views aren't generated yet: game options' values are JSON (`Value`),
  which needs a typed form first.
* **Typed actions** (2026-11-23): what a shell sends is generated too. Each model's actions are
  one enum, `<Name>Act` (`#[serde(tag = "action", rename_all = "kebab-case")]`, and facet's same),
  posted to `POST /models/<name>/do` and read by `models::register()`'s `acts`, which refuses a
  body that isn't one ("not a grabber action: ..."). The Kotlin pages build the generated types
  (`GrabberAct.search(page = view.page + 1u)`) instead of `JSONObject`s, so an action or a field renamed in Rust breaks the Kotlin build, not a
  tap. The per-action routes went (only the shell used them). An option's value is the typed
  `OptionValue` both ways (Game and Application Options, Butler, CoreBots, Script Options).
  Checked on the emulator: Jump's `refresh`, Search Scripts' Pinned and Clear the filters.
* **Typed answers and model names** (2026-11-23): a model's name is written once, in Rust's
  `models::Model`; each view and action type says its model (`ModelView`, `ModelActs`), and
  `register()` takes the names from them, a test checking every model is added. `turu-typegen`
  writes `ModelApi.kt` beside the types: `Api.grabber` is the name with the view's and the
  actions' types, so `ModelClient.act(Api.grabber, action)` takes only a `GrabberAct`, and
  `ModelClient(models = listOf(Api.grabber), ...)` names what it follows. What an action answers
  is a generated record too (`CopiedText`, `CrashText`, `StatsCsv`, `LaunchScript`, `ClientLog`,
  `KeyMoved`, `PromptAnswer`; Army client's gear its `ArmyGearView`), read with
  `act(Api.<model>, action, <Answer>.serializer(), { ... })`; its JSON is what the untyped answers
  had (a test pins each). Which answer goes with which action isn't checked by the compiler: one
  that doesn't decode is logged and reaches `done` as the error. Checked on the emulator: a crash
  report's text (Logs > Crashes), Logs and More.
* **The shell's JNI typed** (2026-11-23): what `SlotShell.kt` and `AccountsPanel.kt` say to
  their Rust over JNI is `turu_frontend::native_shell`'s records, generated as the models' are.
  Each `native*` call answers a list of `ShellAction`s (start, stop, show, send, hello, activity,
  tabs; a hello is an action now, so the activity no longer reads a slot's messages itself), what
  only the activity carries out is an `ActivityMessage` (the slots' `ToShell`s of those names),
  and what the activity sends a slot or asks the shell is a `SlotMessage` or a `TabRequest`, which
  tests hold to `ToClient`'s and `ToShell`'s JSON (the link's own enums carry types facet can't
  generate, bot types and arrays, so these are their Kotlin-facing parts). The native Accounts
  read `AccountsView` and `ServersView`, ask with `AccountsAsk`, and get an `AccountsAnswer`
  (`error` or `actions`, no longer told apart by a leading `{`). The wire changed only where it
  was the shell's own: the actions carry their message and tab list as objects, not strings, and
  `NativeTabs` is camelCase. Only `ModelClient.kt` reads JSON by hand now, the control API's
  envelope (`{ revision, view }`, `/events`, `/log`). Not tried on the emulator yet.
* **L5b**: the Script Loader is native (`ScriptsPage.kt`, `ModelClient.kt`), in the shell's
  panel, read from the tab on screen's scripts model and its log over the control API; Search
  Scripts and Edit Script Options open the slot's egui windows (`ToClient::OpenWindow`). However
  the slot opens the Script Loader or Accounts (its chip, menus, a hotkey), the shell shows its
  own (`ToShell::OpenScreen`). Verified on the emulator: load, Start, Pause, Resume, Stop, the
  log arriving, the state on the native pane and the egui rail in step.
* **The native rail** (2026-11-20): Auto, Jump and Bank have models now (`models::auto`,
  `jump`, `bank`: what the egui popups kept per tab, the egui bar and rail use them too), and
  `models::rail` gives the update notice, the chip's favourite and recent scripts and VibeSkua's
  menus as the platform offers them. The shell draws the whole rail (`ToClient::Tabs`'
  `native_rail`; the slot keeps its room) and hides it in every tab (`ToClient::RailHidden`).
  Auto, Jump, Logs and More are pages of the side panel; More opens the slot's egui windows by
  key. Verified on the emulator: the items, each page, Hide Side Bar and its handle.
* **Search Scripts** (2026-11-20): `models::repo` holds what the egui window decided (the search,
  filters and sort, each row's rules and why not, running a row's work, the badges); the egui
  window lays it out, a native page reads a per-tab query and the first 80 results while it
  shows. With it, "Done when"'s first part holds on the emulator: a script found, loaded, started
  and stopped without an egui window.
* **Game Options** (2026-11-20): its view is typed for the shells (`OptionValue`, `KindView`;
  facet-generate doesn't rename fields inside enum variants, so they're one word) and generated;
  a native page edits it live, marks Edited against Changed outside (the page's own record), and
  has Defaults, Revert and Save. The first decode failed loudly on that rename, as planned.
* **Application Options and CoreBots** (2026-11-20): Application Options' field table is
  `models::application_options` (the egui window draws from it), with the option rows of
  `models::form` shared with Game Options; the Web Monitor's links, QR code and keys stay in the
  full window. CoreBots' state machine is `models::core_bots::Draft` (the egui window derefs to
  one); the slot keeps one per tab for the native page, opened with it and saved when it closes,
  never a copy in the shell.
* **Every egui window stays reachable**: each native page's menu has Full window, and a window the
  shell asks for stays egui until it closes (`Windows::egui`). Before that (from the native
  Script Loader to here) the native pages caught every opening of their windows, and what only
  the egui windows have (Delete and Reset Scripts, account groups and Start Selected, Revert
  mine, Relogin Server's list, the log's Find and Copy) couldn't be reached on Android.
* **A bug this work caused, fixed**: the app's `network_security_config.xml` (loopback, for the
  model reads) replaced rustls-platform-verifier's file of the same name, and with it the
  revocation lists the certificate checks fetch: every HTTPS request failed ("certificate was
  revoked") from the native Script Loader's build on. It's generated from the library's now.
  Notes here that blamed the emulator were wrong; the game was tried again after the fix (Jump,
  Bank, CoreBots' class list).
* **Scheduler** (2026-11-20): `models::scheduler` holds the playlist's view, its controls with
  their reasons, the note and an entry's options read (per tab, `scheduler::STATE`), which the
  egui window draws from. Load and Save also go by name in the playlists folder, so the native
  page needs no file dialog (the full window keeps the system's). Script options reads off the
  caller's thread; the egui window takes the read (`take_read`, called even while closed) and
  opens Script Options, and the view's `reads` count tells the page whether it did (the panel
  closes) or why not (a note). Checked on the emulator: Add scripts through Search Scripts'
  queue, Move, Remove, Save, Load, and Script options on a script without options. Not checked:
  Script options on a script that has some, and a running queue's statuses.
* **Army Queue stays hidden on Android**: its squads run as `turu` processes of their own, which
  Android can't start (`Platform::launches_processes`; docs/ANDROID.md). Nothing to port.
* **Army Control** (2026-11-20): `models::army` holds the rows (with `Peer`, which Gear, a
  client's logs and Map Travel use; native since 2026-11-21, below), who a command goes to, the commands and their outcomes; the
  egui window draws from it. A row's key is the same whichever slot answers (`tab:<the shell's
  number>`, the slot's own tab too), so the native page keeps the checked rows in Kotlin through a
  tab switch and sends them with each action (`send`, `row`). Reading the view counts as Army
  Control drawn, so the other tabs are polled while the page reads it. The outcomes are
  process-wide, which on Android means per slot: after a tab switch the page lists that slot's.
  Search scripts… opens the native Search Scripts in a pick mode (`ShellState.scriptPick`); the
  pick goes to army's `script` command, which downloads the script first when it needs it, as
  the egui pick mode does. Gear, client logs, Travel and Load file… stay in the full window. A
  tab's own row reads
  its monitor snapshot while fresh, as the other tabs read it, and Army Control open keeps the
  samplers going (`monitor::touch`). Checked on the emulator with two tabs: each row shows its
  map and HP the same from either tab; Hide Players on reached both ("done" from each; off was
  sent, its outcome not read); a checked row narrows the target; Stop script to both asks first;
  Show its tab switches and the check stays; Full window stays egui; the slot on screen killed
  with the page open, and the other one, each came back current; font scale 1.3 and the keyboard
  over Quest; Search scripts… picked a downloaded script for both clients and for one checked
  client, and Cancel went back with the check kept. Not checked: a pick that needs a download, Groups (no groups saved), Accept quest, a row's own Start, Stop and
  Log in or out, a launched client's rows (Android has none), the slot opening Army Control by
  itself (`openScreen("army")`), and typegen on the release job's Linux runner.
* **Notification Center** (2026-11-20, past the pass's list: picked as the next for checking in
  on a running script): `models::notifications` gives the feed with each entry's copied line, the
  filters with counts and their empty words; the selection and filter stay each window's. The
  page copies and shares in Kotlin (clipboard, share sheet) from the entries' lines. Checked on
  the emulator: the feed, an empty filter, selecting, Copy ending the selection, Clear's question
  (cancelled), Full window. Not seen on screen: the item, activity, warning and error icons (the
  feed held only a map change), Share, and a tab switch.
* **Accounts' selection and groups** (2026-11-20): the long press, Start several, Add to group,
  Remove from the group and Remove the group, as the pass gave them (where they differ: docs/UI.md,
  "The long-press selection"), through new requests in `shell_accounts.rs` (`startMany`,
  `addToGroup`, `removeFromGroup`, `removeGroup`). Checked on the emulator with three made-up
  accounts (removed after): selecting, Select all shown, Start's question with 2 tabs free (not
  started), a new group and its chip, filtering by it, Remove from the group, Army Control's
  Groups listing it ("0 of 1 open"), Remove the group. Not checked: starting several for real.
* **Search Scripts everywhere a script is chosen** (the owner's rule, 2026-11-20; docs/UI.md):
  Accounts' launch options get Search scripts… (`ScriptPick.Launch`; repo's `launch` action sets the launch script in the
  tab, downloading first when needed). Checked on the emulator: the rail item, a launch-script
  pick (set back to none after). Not checked: a pick that needs a download (every script here
  was downloaded).
* **Stats and Current Drops** (2026-11-20): `models::stats` (the clock kept after a run, the
  counters, Advanced's words in the unit, the shared history read every couple of seconds, Reset,
  Advanced, unit, Save, Clear, Export answering the CSV) and `models::drops` (the sorted stack,
  the AC count, the search's rule, the pickups on the tab's bot); the egui windows draw from them.
  Checked on the emulator: Stats' counters, Advanced on and off by its label, gameplay, Save and
  Clear on an empty history (left empty); Current Drops' empty state. Not checked: drops in the
  list and picking them up (none dropped), Export, Reset.
* **Skills** (2026-11-20, the owner's pick for the next page): `models::skills` holds the editor,
  Edit Rules' copy, the saved sets with the class picked, and Save, Remove, Open and Reset, one per
  tab in `skills::STATE`, which the egui window and the native page share (a first: Skills has no
  save on close, so a copy per shell would lose a sequence in progress at Full window). The view
  carries the class list and only the picked class's sets (the file holds about 270 classes,
  85 KB). Rules go back whole (`rules`, a `RulesView`) and are bounded in the model (a percentage
  stops at 100, the last aura removed turns Several auras off). facet-generate refuses a field
  named `index` (it shadows a local in the generated decoder): the views say `position`. Checked
  on the emulator: a made-up class typed with the keyboard up, skills added, a skill's rules set
  and confirmed, Full window showing the same sequence, Save, finding the class, Remove (asked;
  the class went with its set), Reset's question (cancelled), font scale 1.3, a tab switch with
  the page open (each tab shows its own slot's draft; back again, the first is as left). The
  rules page keeps its own copy and sends it whole a moment after the last change and before Done
  or Back, and mirrors the model's "last aura removed turns Several auras off"; Save carries the
  Class field's text as it shows (reported whenever it composes; text not sent yet goes when it
  leaves): checked with a set saved for one made-up class, another typed, the first opened from
  Saved sets, and Save naming the first (both removed after). Not checked: Several auras with real auras, Move, a script using a
  set saved here, the slot killed with the page open (the draft lives in the slot and goes with
  it, as the egui window's would).
* **Loadouts** (2026-11-20): `models::loadouts` holds the character's list, the gear cache,
  where each item is, what a bot thread does (reading, equipping, enhancing) and its outcome, and
  Save current gear, Replace with worn, Equip, Enhance, rename and Delete, one per tab
  (`loadouts::STATE`), shared by the egui window and the page; the pick and the name being typed
  stay each shell's. The rows are worked out when the gear cache or the list changes (taken at
  most once a second), not on each read. Actions name the loadout, not its place. Checked on the
  emulator (no script running): the list with no loadouts, Currently equipped's slots, Save as
  loadout opening Rename (confirmed from the keyboard), the Worn chip, Equip on what was already
  worn ("was already worn": nothing changed), Rename following the loadout, Delete (asked), font
  scale 1.3, Full window: Save current gear (its name selected), rename, Equip (already worn),
  Refresh and Delete in the egui window; the loadouts file and folder the checks made were
  removed. A first cut hung the egui window on those buttons: it holds the tab's state for the
  frame and the work locked it again; `State` methods now start the work on the caller's guard
  (a test calls them under a held one), and the free functions that lock are the control API's. Not checked: Enhance
  (spends gold), Equip of items in the bag or bank, Replace with worn, Equip while a script runs.
* **Fast Travel** (2026-11-20): `models::fast_travel` holds the destinations, the travel in
  flight and its outcome, Get current's answer (with a count, so each shell's form takes it once)
  and the private room, one per tab, shared by the egui window and the page; the search, the form
  and what it edits stay each shell's. Actions name a destination by its place and its name, so
  one that moved is refused. The owner chose layout-only checks for pages that move a character
  (2026-11-20): checked on the emulator were the list (the three defaults), Add with the keyboard
  up, Use where I am (Map typed as another map came back as the one the character is in), Edit
  opening filled and Back, Remove, Clear's question (cancelled), Full window; the
  FastTravels.json the check made (there was none) was removed. A travel starts on the caller's
  guard as Loadouts' work does (tested under a held one), and the egui form stops editing on
  opening unless its row still has the name it took. With the keyboard up only the field typed in
  stays (the search or the room number); checked editing the room number (set back). Not
  checked: travelling (a tap on a
  destination), the private room on a travel, a list changed in the other shell.
* **Butler** (2026-11-20): Butler's state is the bot's (`bot.butler`), so `models::butler` keeps
  per tab only the last outcome and the roster as read (once a second), and gives the header, each
  roster row's chips and why Follow and Summon rest, Summon all's rule, the options as
  `models::form` option rows (one table both shells read, with `set`) and the newest log lines.
  Nothing holds its lock across a frame or while calling into Butler (the lesson of the hang
  above). The native page reuses Game Options' `OptionRow`. Checked on the emulator, layout only
  as the owner chose: the roster with one account, Settings (one switch on and back off, read in
  butler.json; the file the check made, there was none, removed), Log empty, Send to player's
  dialog (cancelled), font scale 1.3, the keyboard up on Goto delay (the header and segments step
  aside), Full window with its settings drawn from the rows (Leech mode on and off, read in
  butler.json). Summon and Release call `butler::summon` and `release`, not the toggle, so a tap
  on a view a tick old can't undo itself (tested); Follow is refused unless the row's Follow is
  enabled now; the recent targets (a file) and the log lines are read with the roster and by the
  log's version, not on each view. Not checked: Follow, Summon, Summon all, Release, Send to player (they move characters), the log
  with lines, Copy and Save.
* **Cosmetics** (2026-11-20, the owner's pick): `models::cosmetics` keeps per tab what the game
  actions report (the quick bar's outcome, the catalog's status, colors read, by a count so each
  shell takes them once, the room's players) and the native page's catalog query with its filtered
  list (filtered outside the lock when the query or the catalog changes; 50 items a page), and has
  the actions, the catalog filter both shells use, the presets and the color rows. The catalog,
  favorites and outfits stay `turu_bot::cosmetics::Library`'s (per process). The egui window keeps
  its layout, filter and selections. The first native layout pinned search, chips and sort above
  the list and left it no height on the emulator's 411 dp: they scroll with the items now.
  Checked on the emulator: Copy's room players, a search, a favorite on and off, Equip then Reset
  to Real Gear (client side; "Reverted to actual equipped gear & colors!"), Save current look and
  Delete, Colors read from the avatar, the presets, Full window. The cosmetics folder the checks
  made (an empty favorites and outfits file and the 10 MB catalog download) was removed. Not
  checked: Copy Player, Copy my target's look and Copy page, Apply colors and a preset, gender,
  Check catalog updates, Kind within Weapon.
* **Script Options** (2026-11-20, the owner's pick): `models::script_options` holds per tab the
  draft (the loaded script's, or a queued entry's from the Scheduler and Army Queue), the script's
  waiting request, the last error and the checklists hiding owned items, and works out the groups,
  checklists, explanations and item candidates (moved from the egui window). The lifecycle is the
  delicate part: the waiting script is let go only by Save, Cancel or the window's own close. The
  egui window detects the request (`follow_request`) and opens; on Android the bounce to the
  native page calls `hand_over`, so the egui window hiding is no Cancel; the native page's Back is
  Cancel. The Script Loader's Options now pushes the native page (it asked the slot for its egui
  window before), and the Scheduler page no longer closes the panel when its read arrives (the
  slot opens the native page). Checked on the emulator with farm/BuyScrolls.ts loaded (not
  started): its options (a switch, a choice, a number), a switch changed then Cancel (dropped),
  changed again and Save (written to options/BuyScrolls.cfg), the Script Loader's Options, Full
  window showing the same draft and its Cancel; the .cfg the check wrote (there was none) was
  removed and the recent scripts are as they were. The page sends `leave` when it goes or the tab
  changes: with nothing waiting the draft goes; with a script waiting the slot's window takes the
  editor back. A draft follows the loaded script (one of a script no longer loaded is dropped), and
  a script's request starts from a fresh draft. The hand-over to the native page happens only from
  the tab on screen (`turu::tab_shown`, set where an Android slot gets and loses its window, and by
  `client_screen` on the desktop), since a native page shows the tab on screen: a background
  tab's script waits in that tab's picture until it's shown. A test drives a real script's request
  through Opened, hand_over, leave, Save and Cancel. Also checked on the emulator: a queued
  entry's options from the native Scheduler (now opening the native page; Cancel; the entry
  removed), the draft following a newly loaded script (EternalDragonDrops after BuyScrolls, the
  panel dismissed between), the keyboard up on a number. Not checked: a script asking for its
  options on the emulator (that starts it), a checklist (merge scripts), an item option, font
  scale 1.3 on this page.
* **Runtime** (2026-11-20, the next in the recommended order, for checking on a running script):
  `models::runtime` reads the pickup list, the loop's options, the registered quests and the
  boosts from the bot (they were always the bot's) and keeps per tab only whether a boost search
  runs; its functions add and remove drops (`|` between several, a number is an ID), register
  quests (VibeSkua's ID parsing, an error for anything else), and start or stop the drop loop and
  the boost timer (stops off the caller's thread, for the model's own tab). The egui window keeps
  its two columns, inputs and selections. Checked on the emulator: the three parts, a made-up drop
  added (keyboard up), checked and removed, font scale 1.3 (the reward hint shortened to fit).
  Not checked, as they act on the account: Pick drops, Use boosts, a boost on, the searches, a
  quest registered, the Reject else, Accept AC and Interval settings.
* **Notify Drop** (2026-11-20, the owner's pick): `models::notify_drop` now holds per tab what the
  window held (the watched names, beeps per drop and the gap, the drop handler subscribed while
  the list has names), with the beep, the once-per-10-s alert and VibeSkua's `|` splitting; the
  native page removes by name, the egui window maps its picked rows to names. Runtime's Drop
  notifications pushes the page. Checked on the emulator: a made-up name added (keyboard up),
  checked and removed, Test beep, the gap typed with the keyboard up and set back, Back to
  Runtime, font scale 1.3. Not checked: a real drop beeping and its notification.
* **Loader** (2026-11-20, the owner's pick): `models::loader` holds per tab the quest cache as
  read (with a revision), what runs (progress, Cancel), the notice, and a native page's search and
  page (filtered outside the lock; 50 quests a page, as the cache has over 10,000); its functions
  read (or, with no file, download) the cache on open, download, update (Add missing, Rebuild all,
  a range), cancel, fake complete, and load a shop or quests. The egui window keeps its search,
  selection and layout. The page copies the checked quests itself (the same three formats).
  Checked on the emulator: the first open downloading the community cache (10,345 quests), a
  search with the keyboard up, two quests checked and Copy IDs, the pages (a new page starts at
  its first quest), font scale 1.3; the downloaded QuestData.json and its .etag were removed.
  Fake complete shows only with one quest checked. Not checked, as they act in game: Load (IDs,
  a shop, the checked quests), Fake complete, Add missing, Rebuild all, Update range, Cancel.
* **Grabber** (2026-11-21, the owner's pick): `models::grabber` holds per tab each list's rows
  (with a revision), what runs and its status, the quantity an action asks for and the message box
  it shows (one question, which either shell answers), and a native page's list, search, page
  (100 rows) and details row. Shells keep their selection as positions in one grab; an action on
  rows of an older grab is refused. Kill waits while a script, a Console snippet or Auto runs, the
  reason under the buttons. Copy answers its text; Export stays the window's (Full window).
  Checked on the emulator: the drop-down, Grab of Inventory (166 rows), a row's fields, a check,
  Copy from the menu, Quests' five actions wrapping at font scale 1.3. Not checked, as they act
  in game: every list action (Buy, Equip, Sell, To Bank, Accept, Kill, ...) and so the quantity
  question and the message box on the page.
* **Junk Items** (2026-11-21, the owner's pick): `models::junk` holds per tab the inventory and
  bank items as read (junk marks and goals with them), whether a refresh or sale runs and the last
  error, and a native page's search, filters, page (100 items) and selection (by ID and category,
  kept over a refresh). The egui window keeps its own filters and selection and passes the
  selected items to the model's mark, goal and sell. The page checks a row at once, before the next
  view. Checked on the emulator: the list (166 items), a check, the Highlight menu and its filter,
  ⋮, the goal bar with the keyboard up (closed without setting), font scale 1.3 (the checked
  items' actions scroll sideways on one row); no junk list or goal was saved. Not checked, as they
  sell in game or change the owner's data: Mark, Unmark, Unmark all, Set and Clear goal, the sales
  and Skip sell warning.
* **Console** (2026-11-21, the owner's pick): `models::console` holds per tab the output (1,000
  lines, with a count of those dropped), whether a snippet runs and the snippet last run; Run
  refuses while a snippet, a script or Auto runs, with the reason. Runs go to the tab's own bot
  (the egui window ran on the tab on screen). The egui window keeps its own editor. Checked on the
  emulator: the page, typing with the keyboard up (the editor first lost its focus when it moved
  between two layouts; it now has one call site), ⋮, font scale 1.3. Not checked: Run, Stop and the
  output's colours, as a snippet runs on the owner's account.
* **Army travel, gear and logs** (2026-11-21, the owner's pick): `models::army_travel` holds the
  jump settings (one file every slot shares: each change reads it again first, so a slot doesn't
  save over another's; a test build uses a temporary file), where each client is (polled under
  one guard, from egui's frame and the page's view) and the server list; jumps are Army Control's
  commands (`jump`, `jump-player`, `change-server`) sent by value, presets edited and deleted by
  name. `models::army_client` answers a row's gear and logs by its key, statelessly (logs from a
  cursor), with Equip's and Refresh's outcomes per key. Army's panel counts the pages using it,
  so a page Army opens keeps its rows. Checked on the emulator: the Travel group, Map Travel (Go
  to, Use current, a visited map filling Go to, the presets' empty state, New… opening the form,
  font scale 1.3), both pickers (cancelled), Gear and Logs (both kinds). Not pressed: any Jump,
  Change server, Equip or Refresh, Save, Clear or Delete. While building, a model test sent a
  jump into the Mac's real `army-control-jump.json` and replaced its last jump (Go to's starting
  fields) with yulgar; the history and presets were untouched, and test builds can't reach it now.
* **HotKeys' model** (2026-11-21, the owner's request, though HotKeys has no native page):
  `models::hotkeys` holds the catalog, the bindings and `HotKeys.json`, editing (assign, a key
  moving from another action, clear, add, remove with Undo, defaults) and running an action for a
  tab, answering what it needs of the UI (`Needs`: a window, the bank, Auto, the script button).
  No egui: a key is kept by egui's name (`KEY_NAMES`), and a test in the egui glue keeps the table
  in step with `egui::Key::ALL`. `/models/hotkeys` serves the view and the edits for a shell that
  gets keys (macOS, later). Checked by the tests (the window drawn at every size included), not by
  pressing keys in the desktop app.
* **Engine dialogs** (2026-11-21, the owner's pick; L3's first part): `models::prompts` gives the
  one prompt waiting for the tab, the most pressing first (the script's message box, Butler's
  "a script started while following", the crash notice, the Tab Limit, a graphics tip), each with
  an ID an answer must name, so an answer to one already gone does nothing. The crash notice's
  reports and the refused tab moved there from egui. The shell follows it whenever a tab shows,
  from a view of its own (the panel's draws only while open), and shows a Material dialog; while
  a shell has read it within three seconds, egui's dialogs step aside. A script's box isn't
  closed by a tap beside it (Back still answers "closed"). Show report opens the report in the
  native Logs > Crashes (since the crash reports' page, below; it opened egui's Logs before);
  Raise Tab Limit opens Application options, searched to Tab Limit (since 2026-11-22). The
  crash upload question is one too since the same day (egui drew it in the slot's picture: a slot
  has no activity for `CrashQuestion.kt`), with its Privacy policy link as a text button. Checked on the emulator: a made-up crash
  report's notice and Show report (the report and the one it pushed out of the 30 kept were put
  back, with `.seen`). Not seen: the message box, Butler's notice, the Tab Limit and the tips
  (they need a script, Butler following, a refused tab or a slow GPU); they share the dialog.
* **Crash reports** (2026-11-21, the owner's request; the last real gap): `models::crashes`
  lists the reports (re-listed at most every 3 s, since the view runs each tick), answers a
  report's text cut to `SHARE_BYTES` with its share subject (`text`), and deletes one by its file
  name into `crash/.deleted/` so `restore` puts it back in its place (a rename keeps its time;
  only the last Delete is kept, and it's purged at the next start). Not per tab: one folder for
  the app. egui's Logs > Crashes draws from it. On Android (the owner's pick: a Logs segment, not
  a page of its own) Logs gains "Crashes (n)" once there is a report, a tap opens
  `Page.CrashReport` (Share, Copy, Delete with Undo, the text); `CrashesPanel` counts the pages
  holding it, so crossing from the list to a report keeps the connection. The crash notice's Show
  report answers `open: "crashes"` and `report: <name>` (`SHOW_REPORT` and egui's consumer are
  gone) and the shell opens Logs > Crashes on that report. "Crashes (30)" didn't fit a third of the
  320 dp panel: the segment says Crashes and the list's first line the count; the report's lines
  wrap (they are mostly sentences). Checked on the emulator (`-gpu host`, the owner's 30 real
  reports plus a made-up one, `crash/` backed up first and put back after with `.seen` and the
  times): the notice's Show report opening the made-up report, the list, a report's page, Delete
  and Undo (on the made-up report only), font scale 1.3. Not pressed: Share and Copy (they hand
  the text to the system). No text field, so no keyboard check.
* **Packet tools** (2026-11-21, decision 8 as changed): `models::packets` serves the Logger
  (logging and its filters, the engine's own; the lines are the control API's `packet` log, read
  from a point as the Logs page reads its own), the Spammer (`bot.spammer`: queue, loop, delay,
  Send to client, Add, Send now; a row is removed by its index and text) and the Interceptor
  (`bot.capture`: Connect, relogging on a chosen server, Log packets, and a page's server, search,
  filters and selection; the newest 300 shown packets; Copy answers its text). The egui windows
  use the same engine; Load, Save and Export stay theirs (Full window). Checked on the emulator:
  the three pages, the Logger's logging on and back off and its filters dialog (cancelled), the
  Spammer's composer with the keyboard up (the queue steps aside), the Interceptor at font scale
  1.3. Not pressed: Start, Send now, Add, Connect, Clear.
* **Update Turu** (2026-11-22, the owner's request): `models::client_update` gives the new
  release, how this platform takes it (`InstallWay`: an APK to the installer, in place, a link),
  the install's state in words (`UpdateStep`), the buttons with their reasons and the footnote,
  from a pure `build` tested without the process's state; the egui window draws from it. Not per
  tab: the release and the install are the process's, and since 2026-11-22 every Android slot
  shows what the checking slot found (`update-available.json`). The install now waits while any
  other tab runs (`shell_tabs`' busy), and two slots can't download at once (`jobs/client-update.lock`).
  The page is `Page.Update`, from the rail's Update and More's. Checked on the emulator (`-gpu
  host`) with a made-up 0.5.0 pre-release planted in `update-available.json` (the checking slot's
  launch check removes it: plant it after), its asset at `example.invalid`: the page, More's dot,
  Download & install failing on the download (red, dot on the first line, More's dot red), font
  scale 1.3 (it scrolls); the file was removed after. No text field, so no keyboard check. Not
  seen: the downloading state with a real download, Android's installer, the held state (no tab
  ran), the "is the newest" state, a release without a package.
* **Design hook** (2026-11-20, the owner's pick): `.claude/hooks/compose-design-check.py` runs
  after each edit of an Android Kotlin screen and on Stop (Impeccable's own skips this repo); it
  found and cleared nine findings in the pages before Skills.
* Not yet: a phone (M4).

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

#### Handover (2026-11-20)

Built on branch `native-ui`, on `main` since 2026-11-22; later work goes on `main`.

**Native on Android now**: Tabs, New tab, Accounts (with long-press selection, Start several and
groups), the rail (Logs, Army, More, Auto, Jump, Bank, Update, the script chip), Script Loader,
Search Scripts (with a pick mode), Scheduler, Logs, Game, Application and CoreBots options, Army
Control, Notification Center, Stats, Current Drops, Skills, Loadouts, Fast Travel, Butler, Cosmetics, Script Options, Runtime, Notify Drop, Loader, Grabber, Junk Items, Console, Logs > Crashes, Update Turu. Each window's page has Full window for the
egui one, and the engine's dialogs (a script's message box, Butler's notice, the crash notice,
"Send crash reports?", the Tab Limit, the graphics tips) are native too. **Still egui on Android**, from More: HotKeys only
(staying egui; decision 8 as changed 2026-11-21). Spammer, Packet Logger and Interceptor are native
since the same day. The owner's order (2026-11-20): Skills, Loadouts, Fast
Travel and Butler (all done). Crash reports (Logs > Crashes) are native since 2026-11-21.

**What's still egui on Android** (checked 2026-11-21 against the code: every More item maps to a
native page in `RailPages.kt`'s `nativePage`, every egui window bounces to its page, and the
engine's dialogs are `models::prompts`):

1. **The Headless Mode screen**: the "monitor off" page egui draws over the game area while
   Headless Mode is on. A placeholder with no controls, not a window.
2. **What only a Full window has** (each page's ⋮ says so):
   - Logs and Script Loader: the log's Find and Copy; opening the script in an editor.
   - Application options: the Web Monitor's links, QR code and keys.
   - Game options: Revert mine, and the Relogin Server list.
   - Cosmetics: Manual SWF.
   - Script Options: a picker under each item option.
   - Stats: the history as a table, sorted by any column.
   - Butler: opening the folder of saved logs.
   - Files: Spammer's Load and Save, saving the Packet Logger's packets, the Interceptor's and the
     Grabber's Export, saving an Army client's logs, Scheduler playlists anywhere on the device.

Hidden on Android by design, so nothing to port: HotKeys (no keyboard; its model is shell-neutral,
`models::hotkeys`) and Army Queue (Android can't start processes of its own). Turu Mas stays
egui throughout (decision 9).

**Owner decisions taken while building (2026-11-20)**, beyond "Decisions taken":

* **Search Scripts everywhere a script is chosen** (docs/UI.md has the rule): every screen that
  chooses a script offers Search scripts…, opening Search Scripts picking for it
  (`ScriptPick` in `ShellState.kt`: `Army`, `Launch`; add a variant for a new one). Not on the
  rail: a rail Scripts item was tried and taken off the same day, to keep Jump on a short screen.
* **Army Control's script** comes from Search scripts…, not "This tab's script" (removed).
* **Starting several accounts** asks first when the free tabs are fewer, naming who won't start.
* **Design checks**: Impeccable's hook is wired but skips every file here (its log says
  `skipped: native-platform`, PRODUCT.md's platform being `adaptive`), and its detector has no
  Compose rules. Since 2026-11-20 the owner's pick is a hook of Turu's beside it,
  `.claude/hooks/compose-design-check.py` (CLAUDE.md, "UI work follows Impeccable"): per edit on
  Android's Kotlin screens and a deep pass on Stop. Per page still: `impeccable context --target
  <page>` once, its Android reference and craft-floor, and screenshots at font scale 1.0 and 1.3
  and with the keyboard up. The shell's look is `ShellTheme.kt` and docs/UI.md's pass, not
  DESIGN.md (the web monitor's).
* **Commits**: one per model, page and docs, without asking; never push or open the PR unasked.
* **Layout-only checks for pages that move a character** (2026-11-20): Fast Travel's travel and
  Butler's Follow, Summon and Send to player aren't pressed on the emulator's real accounts; the
  pages, forms, dialogs and states are checked, and the rest listed as not checked.

**Patterns added since Army Control**: a model action may answer data (`Ok(json!(...))`), which
`ModelClient.act`'s `answer` hands the shell (Stats' Export gives the CSV, repo's `launch` whether
the script was set now); a native page shares and copies itself (`Intent.ACTION_SEND`,
`ClipboardManager`) rather than through the slot; a page shows "This tab is starting again" while
its slot is down and keeps a `PerTab`'s selection only for ids still in the view. Shell-side
requests that touch `accounts.json` go in `android/src/shell_accounts.rs`'s `Ask`.

The steps a page takes, for a later one (Scheduler's `a32d53d76` and `477523df6` are the
smallest example; Army Control's `0141dbc6a` and `d600f879c` one that is per process, not per
tab; Stats' `0d9ede239` and `e4bb580a4` one with settings and an answer):

1. **The model** (`frontend/src/models/<name>.rs`): a view struct (`Serialize`, and
   `cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))`; a
   field inside an enum variant must be one word; no tuples, so `Option<(String, bool)>` becomes
   two fields; no field named `index`: the generated decoder has a local of that name),
   its actions as one enum `<Name>Act` (`Deserialize`, `#[serde(tag = "action", rename_all =
   "kebab-case")]`, `cfg_attr(feature = "typegen", derive(facet::Facet), facet(tag = "action",
   rename_all = "kebab-case"))`, `#[repr(C)]`) and a `pub fn act(.., act: <Name>Act)`, per-tab
   state that isn't the bot's in a `PerTab` (`tab_state.rs`). No egui (`models_import_no_ui`).
   Add a `Model` variant and its name, the pair to `model_types!` (both in `models/mod.rs`),
   `view(..)` and `acts(..)` to `models::register()`, and `.model::<View, Act>()` (plus any answer
   record) to `bin/turu-typegen.rs`. An action that answers returns a record (`Serialize`, facet's
   derive), never `json!`. In an action: fields inside a variant are one word (or a newtype
   variant over a struct with `rename_all = "camelCase"`, as `RepoQuery`); integers as the view
   has them (`u32` for a position or page: Kotlin's `UInt`); an option's value is `OptionValue`;
   an enum of unit variants only comes out as a Kotlin `enum class` (`BankAct.OPEN`), one with
   data as a sealed interface (`JumpAct.cell(..)`, `JumpAct.refresh`); a name Kotlin reserves is
   escaped (``AutoAct.`class`(..)``). A test that reads the JSON a shell sends (a `null` for an
   option left out) keeps the wire honest. Type names are global in
   Kotlin: no two views may have an enum of the same name (hence `scheduler::Tint` beside
   `repo::Tone`; Army reuses `Tint`). `view()` runs on every `/events` tick (250 ms): nothing slow
   in it unthrottled.
2. **The egui window draws from it**, in the same commit, so both shells say the same. Two rules
   learnt the hard way (2026-11-20): **never lock the model's state twice on one thread**: an
   egui window that holds the `PerTab` guard for its frame calls `State` methods on that guard,
   never a free function that locks again (a std mutex taken twice never returns; Loadouts and
   Fast Travel hung on their buttons until this, and each has a test calling under a held guard);
   and **keep what the window's open and close did** (read the file again, reset the pick, drop
   a pending question, forget a stale edit), since a list the other shell changed meanwhile makes
   a kept index point at another row.
3. **The link**: a `NativeScreen` variant (`frontend/src/shell.rs`); in `gui/turu/mod.rs` the
   window's key in `open_now` and a bounce (`open_shell_screen`) unless `kept` (a per-process
   window, Army, also unless its egui window is open).
4. **The page** (`android/.../turu/<Name>Page.kt`): a panel class reading the model over
   `ModelClient` while it shows (copy `SchedulerPanel`), sending its actions as the generated
   `<Name>Act` (`client.act(Api.<name>, action)`, and with its answer's record
   `client.act(Api.<name>, action, <Answer>.serializer(), { ... })`); the generated types come from
   `com.muktiarafi.turu.models` (enum constants are UPPER_CASE). Then `Page.<Name>`
   (`ShellState.kt`), its route and panel field (`TabPanel.kt`, `widePages` if 440 dp),
   `ShellScreens` (`openScreen`'s string, the panel), and More's mapping (`RailPages.kt`).
5. **Formatting**: never `cargo fmt -p turu_frontend` (it rewraps every file at 100). Run
   `rustfmt --edition 2024 --config max_width=120 <file>` on each changed file, not on
   `gui/turu/mod.rs` or `models/mod.rs` (it follows their `mod` lines into every file; check
   `git status` after and restore what you didn't touch).
6. **Design**: the Impeccable skill's context (`impeccable context --target <page>`) and its
   Android reference; Turu's Compose hook checks each edit (`.claude/hooks/compose-design-check.py
   --check <page>` by hand), Impeccable's own skips Kotlin. The shell's look is `ShellTheme.kt` and the pass in
   `docs/UI.md`, not DESIGN.md's (the web monitor's).

**Before each change lands**: `cargo clippy` and `cargo test` as CLAUDE.md gives them; `air/ane/rust` still
builds (it shares `turu_frontend`); CI's Android job can run the host `cargo run -p
turu_frontend --no-default-features --features typegen --bin turu-typegen` that Gradle's
`turuTypes` needs.

**On the emulator** (`-gpu host`): the screenshots are 2400x1080, so a tap read off a scaled
picture lands wrong; tap by label from `adb shell uiautomator dump` (Compose's text is in it).
Delete what a check saves (`/sdcard/Android/data/com.muktiarafi.turu/files/playlists/`,
`CBO_Storage(<name>).txt`), and log in only on a quiet server. The emulator's own alerts (a
ShakeAlert) can open over the app mid-check: Back, then carry on. Back with the keyboard up closes
the panel, not the keyboard: hide it with the keyboard's own chevron. Test with made-up accounts
added through the form and removed after (search for each before Remove, so the row's ⋮ can't be
a real account's); never Clear or Reset data the owner keeps (the rate history is every client's)
unless it was empty; set back any setting a check changes (Advanced, the launch script). The
emulator's two tabs are the owner's real accounts: no scripts started on them unasked.

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
   *Changed 2026-11-21 (the owner): Android ports everything left, the engine's dialogs first, then
   Spammer, Packet Logger and Interceptor; HotKeys stays egui there (unused without a keyboard).
   Full window stays on each page as egui's way back, and Turu Mas keeps egui.*
9. **Turu Mas**: stays egui; if ever reopened, hand-written JNI.
10. **Across the boundary**: JSON, with Kotlin and Swift types generated from the Rust records.
    Android needs no generated FFI binding; BoltFFI only for macOS.

11. **The checkpoint** (2026-11-20): go on to Android's bot windows (L1's client part, L2 to
    L5b). macOS (L6, L7) is not decided. The work stacked on the branch `native-ui`, on `main` since 2026-11-22.
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
