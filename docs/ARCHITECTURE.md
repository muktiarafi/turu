# Turu architecture

Turu is a cross-platform (macOS, Windows, Linux, Android) AdventureQuest Worlds bot client. It
aims for feature parity with [VibeSkua](../../../VibeSkua) (a Windows-only C#/Avalonia Skua fork)
and keeps the same UI/UX, but:

| | VibeSkua | Turu |
|---|---|---|
| Flash player | ActiveX Flash OCX (CleanFlash, Windows only) | Ruffle + Mugetsu's AQW patches (wgpu: Metal / Vulkan / DX12) |
| UI | Avalonia | egui (inside Ruffle's desktop frontend), Mugetsu palette |
| Bot engine | C# (`Skua.Core`) | Rust (`bot/`, crate `turu_bot`) |
| Scripts | C# compiled with Roslyn, `//cs_include` | TypeScript, types stripped by `oxc`, run in QuickJS (`rquickjs`), ES `import` |
| Game host SWF | `skua.swf` (from `Skua.AS3`) | Turu's own `turu.swf` (`host/`, same callbacks and answers), embedded at `bot/assets/turu.swf` |

Android has two apps. Turu (`android/`) runs the game in Ruffle, as the desktop does, and is the
default. Turu Mas (`air/`, docs/TURU_MAS.md) runs it in the Adobe AIR runtime instead, the
alternative for when Ruffle misbehaves: it exists because of Ruffle's quirks on Android, and AIR
is much more stable at running Flash, though not as fast as Ruffle.

## Repository layout

Turu is its own workspace on top of **ruffle-aqw** (Ruffle with Mugetsu's patches):

```
ruffle/                  submodule: muktiarafi/ruffle-aqw (branch `aqw`), Ruffle `f22d29d9` + Mugetsu
                         2.0.0. The app uses its crates by path; its patched crates (ruffle/patches/:
                         wgpu-core, wgpu-hal, gc-arena, lyon_*, and egui-wgpu, whose renderer skips
                         a frame's meshes when the GPU has no staging buffer instead of panicking)
                         are wired up in the root Cargo.toml
host/                    crate `turu_host`: Turu's own host SWF (docs/HOST_SWF_PROPOSAL.md), its AS3
                         (as3/, package turu.host) and its build with Ruffle's asc.jar: `cargo run -p
                         turu_host` writes bot/assets/turu.swf (the default host SWF), `-- --check` (CI) checks it
bot/                     crate `turu_bot`: the bot engine (no dependency on the app or ruffle_core)
  assets/turu.swf        Turu's host SWF (built from host/as3), embedded with include_bytes!, the default
  js/                    TypeScript runtime that ships inside the binary (module "turu")
  src/
    lib.rs               `Bot` (Arc handle shared by script thread, UI and background threads)
    flash.rs             `Flash`: typed calls into the host SWF's ExternalInterface callbacks
    host.rs              `FlashHost` trait (implemented by desktop) + `FlashArg`
    control.rs           Stop and Pause (`ScriptControl`), script-thread marking
    control_api.rs       the HTTP control API (`--api`): hyper on its own thread, token auth, one
                         route table, `/tabs/<id>/...`, server-sent events on `/events`
    monitor/             the web monitor: a `Snapshot` sampler per bot, the hub (tabs, Army and
                         remote clients, events, history, `monitor.json`), the page server and its
                         commands, and `push.rs` (reporting to another machine's monitor)
    accounts.rs          the account list (`accounts.json`); passwords in the system keychain
    army.rs              launching one `turu` process per account and steering them via their API
    butler/              Butler (VibeSkua's ButlerV5 plugin): sync files, summons, the follower,
                         its quest/level/AntiLag fakes and the debug log
    cosmetics/           Cosmetics (VibeSkua's Cosmetics plugin): the item catalog, favorites and
                         outfits (`Library`, shared by the tabs), and the client-side avatar changes
    gear.rs              each account's gear file, equipping and loading the bank from outside a
                         script (`/equip`, `/bank/load`), refused while a script runs
    loadouts.rs          Loadouts Manager: VibeSkua's `LoadoutProfile` files, equip (through gear.rs),
                         and the enhancer (CoreAdvanced's `enhanceItem`, run as a script)
    events.rs            inbound Flash calls (`pext`, `packet`, `loaded`, ...) -> state + event bus
    state.rs             game state derived from packets (drops, map, counters, ...)
    options.rs           bot options (VibeSkua `IScriptOption`) + persistence
    models.rs            game JSON models (InventoryItem, Monster, Quest, ShopItem, ...)
    skills.rs            skill rotations (advanced skill sets) + the background `Worker` loops
    cbo.rs               the CoreBots Options file (`CBO_Storage(<user>).txt`)
    capture.rs           Diagnostics > Interceptor: the game socket's packets (VibeSkua `CaptureProxy`),
                         interceptors that edit or drop them
    spammer.rs           Diagnostics > Spammer: a packet queue sent on a timer
    auto.rs              the Auto popup's Auto Attack / Auto Hunt loop (VibeSkua `ScriptAuto`) on its
                         own script-marked thread
    scheduler.rs         Scripts > Scheduler: a playlist of scripts run one after another
    shared_job.rs        jobs shared by the Turu processes on one data folder (process tabs)
    paths.rs, log.rs     data directory, skill set files; log buffers
    repo.rs              Search Scripts: the script repository's index, downloads, update, reset
    updates.rs           client updates: newer Turu releases on GitHub, the package's SHA-256
                         check, swapping the program (self_replace; ditto on macOS)
    api/                 one module per VibeSkua sub-interface: player, map, combat, kill, hunt,
                         monsters, quests, reputation, auras (self and target), drops, boosts,
                         inventory (also bank, tempInv, house, invHelper), shops, skills, wait,
                         send, servers (and auto-relogin), lite, options, packets, config,
                         handlers, and army (what CoreArmyLite reads outside the game)
    background.rs        startup calls (`loadClient`, ...) and the 20 ms "ScriptInterface" timer:
                         login tracking, rest packets, option enforcement, the relogin watchdog
    script/              ScriptManager: load/start/stop/pause, TS transpile, QuickJS runtime, bindings,
                         script options (`options.rs`: the `scriptOptions` declaration and its values)
frontend/                crate `turu_frontend`: what any Turu frontend shares (no egui, winit, rfd or window)
  src/client.rs          `Client`: a handle over the process's client (tabs, services, the view
                         models read and acted on in process, typed)
  src/turu.rs            Flash integration: tabs, spawn_action; the app hands in how to reach its
                         loop (`set_tab_requests`, `TuruFlashHost::new`)
  src/turu/ruffle_host.rs  `TuruFlashHost` and the ExternalInterface provider (`TuruExternalInterface`)
  src/players.rs         the process's one player (a tab's client, `--headless`, an Android slot,
                         the window for a movie given on the command line)
  src/host.rs            `Host`: file dialogs, browser, file manager for the bot windows; the
                         app installs its own (`app/src/desktop_host.rs`, rfd)
  src/capture.rs         wraps Ruffle's navigator so the game socket goes through `bot.capture`
  src/map_items.rs, map_pads.rs   the map SWF's AS3 bytecode readers (Grabber, Jump's pads)
  src/grid.rs            Grid View's tiles
  src/engine.rs          the windowless player loop (a tab's client, `--headless`, Android's slots):
                         executor, no-dialog navigator, tick loop
  src/pacing.rs          frame interpolation's pacing (desktop window, process tabs' clients,
                         Android's screen)
  src/shell.rs, client_link.rs   process tabs (docs/TAB_PROCESSES_PROPOSAL.md):
                         what a shell and a client say, a tab's side of the link
  src/layout.rs          where Turu's UI sits around the game (bar or rail, tab strip, cutout)
  src/slots.rs           process tabs on Android (the shell's side is `turu_shell::slots`): each tab a slot, a service in a process of its own
egui_ui/                 crate `turu_egui`: the egui UI over `turu_frontend` (its modules re-exported
                         at the root, so the windows say `crate::turu`)
  src/gui/turu/          egui UI: top nav bar, Script Loader, Options, Skills, Logs, ... (VibeSkua layout)
  src/client_screen.rs   a client's screen: the game and the egui UI drawn into the shell's surfaces
android/                 crate `turu_android` (cdylib, empty off Android) and the Gradle app that
                         loads it: `docs/ANDROID.md`
  shared/                the Kotlin both Android apps share (crash records and uploads, the text
                         field above the keyboard); Turu Mas's extension includes it by path
air/                     Turu Mas (docs/TURU_MAS.md): crate `turu_air` (`ane/rust`, the engine as an
                         AIR native extension, its Kotlin in `ane/android`), the host SWF and
                         `app.xml` (`app/`); built with the AIR SDK (`air/app/build.sh`)
air_patch/               crate `turu_air_patch`: patches AQW's client and the SWFs it loads for AIR's
                         sandbox at run time
shell/                   crate `turu_shell`: the shells' logic without a window. The desktop's
                         (`desktop` feature: the tabs' clients and their restarts, the tab list,
                         Grid View, the pictures on the shell's GPU, pacing, dialogs asked for,
                         updates, Army launches; a front owns the window and carries out the
                         `Action`s it returns) and Android's (`slots`: `SlotShell`)
app/                     crate `turu`, the desktop app: started from ruffle-aqw's desktop frontend
                         (Ruffle's, with Mugetsu's changes marked `AQW build:`), plus the window,
                         the headless loop and the dialogs; it re-exports `turu_frontend`'s modules
                         at its root (`crate::turu`, `crate::grid`, ...)
scripts/                 submodule: muktiarafi/turu-scripts, the TypeScript scripts (Core libraries,
                         ported farms) and their index generator
docs/                    these docs
```

## Processes

Each tab is a process of its own (docs/TAB_PROCESSES_PROPOSAL.md, M6), on the desktop and on
Android. A process with a game holds one `Player` and its `Bot` (`frontend/src/players.rs`).

```
            shell (the window; no game)
  winit window, tab list, input, the pictures shown
     │ ▲                         │ ▲
     │ │ shell messages          │ │        (frontend/src/shell.rs: length-prefixed JSON,
     ▼ │ + pictures              ▼ │         on loopback TCP or Binder)
  client: tab 1                client: tab 2   ...
  one Player + Bot, Turu's UI, drawn into GPU surfaces the shell shows
```

* **Desktop.** Every start of Turu's window is the shell (`shell/`, crate `turu_shell`, behind
  its winit front `app/src/shell.rs`; `shell::runs_as_shell`). It runs no game: it starts each tab as `turu --client=<address>`
  (`app/src/client.rs`), shows the picture of the tab on screen, and sends that tab the window's
  mouse, keys, size, IME and gamepads. A client runs the windowless loop below, with a GPU set up
  without a window. `egui_ui/src/client_screen.rs` draws the game and Turu's egui UI into
  surfaces the shell opens (IOSurfaces on macOS; Vulkan or D3D12 memory elsewhere), or into pixels
  when it can't. A client that dies is started again; one GPU loss ends one tab. The tab strip is
  drawn by each client from the shell's list, and its clicks go back to the shell
  (`frontend/src/client_link.rs`). The window for a movie given on the command line (plain
  Ruffle) keeps one player of its own, without tabs.
* **Android.** The activity is the shell (`turu_shell::slots::SlotShell`, carried out by
  `SlotShell.kt`). Each tab is a slot: a foreground service in a process of its own (`:c1` ...
  `:c9`) running one engine, which draws into the surface the activity hands it. Shell and slots
  speak the same messages over Binder.
* **The desktop's flags.** `--shell` is the window (every start of Turu's window is the shell,
  Army > Accounts' "In its own window" ones included): its first tab gets the other startup flags
  but `--api`, `--monitor` and `--report-to`, which the shell keeps. The shell runs the web
  monitor and Report To over every tab; each client gets a control API of its own on a free local
  port, and the shell's `--api` serves the same routes over the tabs, passing each request on to
  the tab's (`/tabs` lists them; a tab whose client is starting again answers
  `game_not_running`). `--client=<address>` runs one tab without a window and draws it for the
  shell at `<address>`, saying hello with `TURU_SHELL_TOKEN` (set by the shell); it quits when
  the shell closes. `TURU_SHELL_TEST=<file>` scripts the shell for tests (`wait`, `open`,
  `select`, `close`, `kill`, `click`, `dump`; `shell/src/lib.rs` lists them all).
  `--in-process` is ignored since M6.
* **`--headless`** (`app/src/headless.rs`) is one process, one tab, with no shell, window or
  GPU: the null renderer and UI, the loop on the main thread (docs/HEADLESS.md).
* **Turu Mas** is the exception: AIR runs the game, and one process keeps several bots through
  `turu_frontend`'s tab list (`turu::TABS`, `air/ane/rust/src/clients.rs`).

The tabs share the data folder (and Flash's SharedObjects), as VibeSkua's processes do.

## Threads in a client

```
engine thread (frontend/src/engine.rs)   script thread ("Script Thread")      background threads
──────────────────────────────────────   ───────────────────────────────      ────────────────────
Ruffle Player (Arc<Mutex<Player>>)        QuickJS runtime                       ScriptInterface timer (20 ms)
egui UI (reads Bot state)                 user script + Core libraries          drops loop, quest loop,
TuruExternalInterface (inbound) ──┐       bot.* host calls ─┐                   skills timer, boost timer
                                  │                          │                   the shell link's reader
                                  ▼                          ▼
                           Bot::on_flash_call        Flash::call(name, args)
                           (never blocks,            = send EngineEvent::TuruFlashCall
                            never locks Player)        and block on a reply channel
```

* **Ruffle's `Player` is `!Send` in practice** (its futures are `spawn_local` on the thread
  that runs it). Every call into Flash is therefore marshalled to that thread: `Flash::call`
  goes through `TuruFlashHost` (`frontend/src/turu/ruffle_host.rs`), which sends the call (the
  tab, the callback's name and arguments, a reply channel) to the engine loop
  (`EngineEvent::TuruFlashCall`, `engine::flash_host`); the loop hands it to
  `turu::answer_flash_call`, which runs `Player::call_internal_interface` on the player and
  sends the `ExternalValue` back, and `TuruFlashHost` turns it into a `String`. This mirrors
  VibeSkua, where every OCX call is dispatched to the UI thread.
* The shell's messages are read on a thread of their own and run on the engine thread
  (`EngineEvent::Run`); pictures go out on another, newest first, so the engine never waits on
  the shell.
* Calls must **never** be made from the engine thread itself, which also draws Turu's UI (it
  would deadlock waiting for its own reply). The UI only reads cached state or spawns work on
  other threads (`turu::spawn_action`).
* Inbound calls from AS3 (`ExternalInterface.call(...)`) arrive in
  `TuruExternalInterface::call_method` while the player is locked. It forwards them to
  `Bot::on_flash_call`, which only updates state / pushes to channels and returns
  immediately. Anything that needs to call Flash back (e.g. `requestLoadGame` → `loadClient`)
  is done from a bot thread.

## The Flash bridge

The host SWF, `turu.swf`, registers the callbacks (VibeSkua's list: `docs/VIBESKUA_SPEC.md`, section 2;
`turu.swf`'s: its `hostInfo`, docs/HOST_SWF_PROPOSAL.md). The calling convention matches
VibeSkua's `FlashUtil.Call`:

* Arguments are strings, numbers, booleans, null, arrays or objects (`FlashArg`; the last two become
  AS3 `Array`s and `Object`s, as VibeSkua passes .NET lists and objects).
* Nearly every callback returns a **String**: JSON (`getGameObject`, `callGameFunction`, ...)
  or `"true"`/`"false"`. `Flash` offers typed helpers:
  `call_str`, `call_bool`, `call_json<T>`, `get<T>(path)`, `get_static<T>(path)`,
  `set(path, value)`, `call_game_function<T>(path, args)`, `is_null(path)`,
  `select<T>(path, selector)`.
* A call made from the script thread first checks the script's cancellation flag and fails
  with `BotError::Stopped` (VibeSkua: `OperationCanceledException`). That is how Stop works:
  the next bot API call or `bot.sleep` throws, unwinding the TS stack.

Startup sequence (same as VibeSkua):

1. Desktop starts Ruffle with the host SWF, `turu.swf` (written from the embedded bytes to the
   data dir); `Work::LoadClient` asks it `hostInfo` first.
2. `turu.swf` registers its callbacks and calls `requestLoadGame` → bot calls `loadClient`.
3. AS3 fetches `https://game.aq.com/game/api/data/gameversion`, loads the game SWF and calls
   `pre-load` (bot calls `setBackgroundValues`; Turu's login picture,
   `host/assets/login-background.jpg`, is compiled into `turu.swf`) and then `loaded` (the bot marks the game
   loaded and sends `GameLoaded`; its 20 ms timer runs from startup).
4. The user logs in with the game's own login screen, or the bot does with `--user` and
   `--password` (`startup.rs`: `loaded` → `ensure_relogin(server)`, then `--run-script`).
   Relogins (`api/servers.rs`) reuse that login: `login`, wait for the server list, then
   `clickServer` (or `connectToServer` / `connectTo`), on the script thread or, for
   auto-relogin, on a "Relogin" thread that `cancel_relogin` stops.

## Scripts

* Scripts are `.ts` files. Each file is transpiled on load with `oxc` (type stripping only:
  **no type checking at runtime**; editors and `npx tsc --noEmit` in `scripts/` do that).
* Modules are ES modules resolved relative to the importing file. `"turu"` resolves to the
  embedded runtime (`bot/js/turu.ts`), which exports the `bot` singleton and enums.
* Entry point: `export default function main(): void` (synchronous, like C#'s `ScriptMain`).
  All bot calls are **synchronous and blocking**, so scripts read top to bottom with no
  `await`.
* The host exposes one native function, `__turu(method: string, argsJson: string): string`.
  `bot/js/turu.ts` wraps it in typed classes (`bot.player.join(...)`); Rust routes
  `method` (`"player.join"`) to `api::<module>::dispatch`. Errors come back as
  `{"error": "...", "stopped": bool}` and are rethrown as JS `Error` / `ScriptStopped`.
  A successful reply also lists `"due"`, the names of the `bot.handlers` that are due
  (`bot/src/api/handlers.rs` keeps their timers, the closures stay in JS); `turu.ts` runs them
  before it returns the value.
* When `main` throws because it was stopped or failed (not when it returns), the runtime resets
  Stop and calls `__turuScriptStopping(error)` in the same JS context, which runs the
  `scriptStopping` handlers (VibeSkua `ScriptStopping`); a second Stop interrupts them.
* Script header block comment (`/* name: ... description: ... tags: ... */`) is kept from
  the C# scripts for the repo browser.

See `docs/SCRIPTING.md` for the script author guide.

## UI

The window keeps VibeSkua's layout: a 34 px top nav bar (Scripts, Options, Tools & Helpers,
Combat, Bank, Diagnostics, Plugins; Auto / Jump / + on the right) above the game (on a touch
screen, a rail down the left instead), and tool windows (Script Loader, Game Options, Skills, Logs, Packet Logger, ...) as egui windows over
the game. Colours, corner radii and fonts come from Mugetsu's menu style
(`egui_ui/src/gui/turu/theme.rs`). Details: `docs/UI.md`.

## Data directory

`dirs::data_dir()/Turu` (macOS `~/Library/Application Support/Turu`, Windows
`%APPDATA%\Turu`, Linux `~/.local/share/Turu`):

| Path | Contents |
|---|---|
| `turu.swf` | the host SWF, written from the embedded copy at start |
| `api-token` | the control API's token (`--api` without `TURU_API_TOKEN`), owner-readable only |
| `monitor-key` | the web monitor's key (Options > Application > Monitor), owner-readable only |
| `monitor-push-key` | the key other machines report to the web monitor with, owner-readable only |
| `monitor.json` | the web monitor's bots as last seen (snapshot, events, history), shown offline for a day after a restart |
| `options.json` | bot options (Options > Game, Save; Relogin Try Delay and Relogin Watchdog are saved from Options > Application) |
| `accounts.json` | Army > Accounts: accounts, groups, the launch setup (no passwords: those are in the system keychain, service "Turu") |
| `settings.json` | application settings (Options > Application, `bot/src/settings.rs`), saved on every change |
| `QuestData.json` | Tools > Loader's quest cache, VibeSkua's format (`bot/src/quest_cache.rs`); not shipped: downloaded from `auqw/Scripts` when the Loader first opens (and by its Download button), its ETag in `QuestData.json.etag` |
| `map-items.json` | Grabber > GetMap Item IDs: the map items found in each map's SWF, by map file (`bot/src/map_items.rs`) |
| `butler.json` | Butler's options, shared by every client (re-read when another client changes it) |
| `butler_logs/` | Butler's saved debug logs (`export_<account>_<time>.log`) and, with Auto-save logs on, `debug_<account>.log` |
| `cosmetics/` | Cosmetics' `catalog.json` (VibeSkua's community item sheet, downloaded on first open and by Check Updates), `favorites.json` and `outfits.json`, shared by every client |
| `gear/<username>.json` | each account's wearable items (inventory, and the bank as last loaded, dated `bankAsOf`), written by its client within 5 s of a change (`bot/src/gear.rs`); the Army Control Gear window reads it |
| `loadouts/<username>.json` | the character's Loadouts Manager sets, VibeSkua's `Loadouts/<username>.json` format (`bot/src/loadouts.rs`), and `LoadoutEnhancerTemp.ts`, the enhancer it writes before running it |
| `army/<name>.log`, `.barrier`, `.<name>.lock` | `bot.army`'s logs shared by the army scripts of every client (`bot/src/api/army.rs`); each operation holds the log's lock file, a barrier keeps its round and waiting members in `.barrier` |
| `butler/<username>.state` | each logged-in account's Butler state (`key=value` lines, `bot/src/butler/sync.rs`): where it is, whom it summons and follows, a `beat` every 5 s; older than 15 s counts as offline |
| `cache/gamefiles/` | the files the game downloaded, kept for as long as AQW's servers allow (ruffle-aqw's `HttpCache`, `turu_frontend::game_files`; Turu Mas's downloads too, through `TuruURLLoader`); shared by every client, safe to delete |
| `cache/airpatched/` | Turu Mas only: the game's client and the SWFs it loads as `turu_air_patch` rewrote them for AIR (`turu_air_patch::cache`), by the input's hash and the patcher's rules, so a start skips patching (0.2-0.5 s); the 16 used last are kept, safe to delete |
| `scripts/` | the scripts folder: Search Scripts downloads here (the repo's `scripts/` can be used directly) |
| `scripts/types/turu.ts`, `scripts/tsconfig.json` | for code editors: the script API this app runs (`Paths::write_script_types`, when Search Scripts starts; rewritten when it differs), and the scripts repository's `tsconfig.json`, downloaded with the scripts once per commit (its `"turu"` falls back to `./types/turu.ts`); a `tsconfig.json` you wrote or changed stays (`scripts-manifest.json` keeps the sha256 of the one Turu wrote) |
| `scripts-index.json` | the last script list (`index.json` of the scripts repository) and its ETag |
| `scripts-manifest.json` | the sha256 of every script as Search Scripts wrote it: tells an outdated script from one you edited |
| `pinned-scripts.json` | the scripts pinned in Search Scripts |
| `scripts-backup/<time>/` | what Reset Scripts moved aside |
| `options/CBO_Storage(<username>).txt` | CoreBots Options (`Key: Value` lines), written by Options > CoreBots and read by CoreBots' `setOptions()` through `bot.coreBotsOptions()` (`bot/src/cbo.rs`) |
| `options/<Storage>.cfg` | a script's option values (VibeSkua's `<Storage>.cfg`: `Options:Name=Value` lines), written by Scripts > Edit Script Options and `bot.config.set` (`bot/src/script/options.rs`) |
| `AdvancedSkills.json` | the shipped skill sets (from `auqw/Scripts` `Skills/AdvancedSkills.json`, rewritten from the app's copy) |
| `UserAdvancedSkills.json` | your skill sets, used by the bot: seeded from the shipped ones, then only gaining the shipped classes and modes it lacks (VibeSkua's merge) |

## Script repository

Turu's scripts live in their own repository, `muktiarafi/turu-scripts` (the `scripts/`
submodule here; `TURU_SCRIPTS_REPO=owner/name` points the client at another). Its CI type-checks
the scripts and publishes `index.json` as the only file of its `index` branch: per script the
path, the header block (name, description, tags), size, sha256, the dates it was added and
changed (from `git log`), `deps`, every file it imports, and `library`, set for a file with no
`export default` (the `Core*` files: nothing to run). `bot/src/repo.rs` (`ScriptRepo`, one
per app, `turu::script_repo()`) reads it, and the Search Scripts window shows its snapshot.

Differences from VibeSkua's `scripts.json`, which its CI commits to the working branch on every
push: the index stays off `main` (1.1 MB for 1,868 files, about the size of VibeSkua's for 1,943
C# files), and a refresh that finds nothing new is an ETag 304; dates come with it (VibeSkua
asks the GitHub API once per script, at 60 requests an hour); and `deps` lets one download bring
the Core files a script imports. Files are fetched from the index's commit and checked against
its sha256, each from raw.githubusercontent.com, or as one zip from codeload.github.com from 25
files up; neither counts against the API limit.

The manifest (`scripts-manifest.json`) records what Turu wrote, which gives each script a status:
missing, up to date, outdated (untouched since, the repository changed), modified (edited here:
Update all and Download all keep it, VibeSkua overwrites it), local (your own) or removed (gone
from the repository). Libraries aren't listed (VibeSkua lists its Core files, as "null"-named rows whose Start fails);
they are downloaded with the scripts that import them, and your own files are checked the same
way (`repo::is_library`). Reset Scripts moves the downloaded files to `scripts-backup/<time>/` and
downloads everything again, leaving your own scripts (VibeSkua deletes the whole folder). At
launch, Check for Script Updates fetches the index and Auto Update Scripts downloads what's
missing or outdated, as VibeSkua's Manager does; clients launched by Army > Accounts skip it.

## Script options

Script options (VibeSkua's `Options` and "Edit Script Options") are read when a script is loaded:
the engine runs the module's top level in a separate runtime where bot calls fail, and reads its
`scriptOptions` export (`script::options::declaration`). A script that calls the bot at top level
therefore has no options until that's fixed. When the script asks for the window (Start, or
`bot.config.configure`), the script thread posts a request in `ScriptManager` and waits; the UI
polls it each frame, opens the window and answers when it closes (`finish_configure`). Without a
UI (`set_interactive` not called, as with `--headless`) or for a silent start, there's no wait. Logs are kept in memory; the Logs and Script Loader
windows can save them.
