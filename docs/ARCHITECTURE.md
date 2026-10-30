# Turu architecture

Turu is a cross-platform (macOS, Windows, Linux) AdventureQuest Worlds bot client. It aims
for feature parity with [VibeSkua](../../../VibeSkua) (a Windows-only C#/Avalonia Skua fork)
and keeps the same UI/UX, but:

| | VibeSkua | Turu |
|---|---|---|
| Flash player | ActiveX Flash OCX (CleanFlash, Windows only) | Ruffle + Mugetsu's AQW patches (wgpu: Metal / Vulkan / DX12) |
| UI | Avalonia | egui (inside Ruffle's desktop frontend), Mugetsu palette |
| Bot engine | C# (`Skua.Core`) | Rust (`bot/`, crate `turu_bot`) |
| Scripts | C# compiled with Roslyn, `//cs_include` | TypeScript, types stripped by `oxc`, run in QuickJS (`rquickjs`), ES `import` |
| Game host SWF | `skua.swf` (from `Skua.AS3`) | the **same** `skua.swf`, embedded at `bot/assets/skua.swf` |

## Repository layout

This repository is a Ruffle fork. Upstream Ruffle code stays where it is; Turu adds:

```
patches/                 Mugetsu's patched crates (wgpu-core, wgpu-hal, gc-arena, lyon_*); [patch.crates-io]
bot/                     crate `turu_bot`: the bot engine (no dependency on desktop or ruffle_core)
  assets/skua.swf        VibeSkua's AS3 host SWF (Skua.AS3/skua/bin/skua.swf), embedded with include_bytes!
  js/                    TypeScript runtime that ships inside the binary (module "turu")
  src/
    lib.rs               `Bot` (Arc handle shared by script thread, UI and background threads)
    flash.rs             `Flash`: typed calls into skua.swf's ExternalInterface callbacks
    host.rs              `FlashHost` trait (implemented by desktop) + `FlashArg`
    control.rs           Stop and Pause (`ScriptControl`), script-thread marking
    control_api.rs       the HTTP control API (`--api`): hyper on its own thread, token auth
    accounts.rs          the account list (`accounts.json`); passwords in the system keychain
    army.rs              launching one `turu` process per account and steering them via their API
    butler/              Butler (VibeSkua's ButlerV5 plugin): sync files, summons, the follower
    events.rs            inbound Flash calls (`pext`, `packet`, `loaded`, ...) -> state + event bus
    state.rs             game state derived from packets (drops, map, counters, ...)
    options.rs           bot options (VibeSkua `IScriptOption`) + persistence
    models.rs            game JSON models (InventoryItem, Monster, Quest, ShopItem, ...)
    skills.rs            skill rotations (advanced skill sets) + the background `Worker` loops
    cbo.rs               the CoreBots Options file (`CBO_Storage(<user>).txt`)
    paths.rs, log.rs     data directory, skill set files; log buffers
    repo.rs              Search Scripts: the script repository's index, downloads, update, reset
    api/                 one module per VibeSkua sub-interface: player, map, combat, kill, hunt,
                         monsters, quests, reputation, auras (self and target), drops, boosts,
                         inventory (also bank, tempInv, house, invHelper), shops, skills, wait,
                         send, servers (and auto-relogin), lite, options, packets
    background.rs        startup calls (`loadClient`, ...) and the 20 ms "ScriptInterface" timer:
                         login tracking, rest packets, option enforcement, the relogin watchdog
    script/              ScriptManager: load/start/stop/pause, TS transpile, QuickJS runtime, bindings,
                         script options (`options.rs`: the `scriptOptions` declaration and its values)
desktop/src/turu.rs      desktop integration: FlashHost impl, ExternalInterface provider, spawn_action
desktop/src/gui/turu/    egui UI: top nav bar, Script Loader, Options, Skills, Logs, ... (VibeSkua layout)
scripts/                 submodule: muktiarafi/turu-scripts, the TypeScript scripts (Core libraries,
                         ported farms) and their index generator
docs/turu/               these docs
```

## Process and threads

One window can hold several tabs, each a game client of its own: a `Player` and its `Bot`
(`desktop/src/turu.rs`). Every tab's player lives on the event loop thread below; Flash calls
carry their tab, and the window draws the active tab's player.

```
winit event loop thread (main)          script thread ("Script Thread")      background threads
───────────────────────────────         ───────────────────────────────      ────────────────────
Ruffle Player (Arc<Mutex<Player>>)       QuickJS runtime                       ScriptInterface timer (20 ms)
egui UI (reads Bot state)                user script + Core libraries          drops loop, quest loop,
TuruExternalInterface (inbound) ──┐      bot.* host calls ─┐                   skills timer, boost timer
                                  │                         │
                                  ▼                         ▼
                           Bot::on_flash_call       Flash::call(name, args)
                           (never blocks,           = send RuffleEvent::TuruFlashCall
                            never locks Player)       and block on a reply channel
```

* **Without a window** (`--headless`, `desktop/src/headless.rs`) there is no winit event loop:
  the main thread builds the player with the null renderer and UI and runs its own loop, and
  `Flash::call` sends to that loop's channel instead of the `EventLoopProxy`.
* **Ruffle's `Player` is `!Send` in practice** (its futures are `spawn_local` on the event
  loop). Every call into Flash is therefore marshalled to the event loop thread:
  `Flash::call` sends `RuffleEvent::TuruFlashCall { name, args, reply }` through the winit
  `EventLoopProxy`; `app.rs` locks the player, runs `Player::call_internal_interface`,
  converts the returned `ExternalValue` to a `String` and sends it back. This mirrors
  VibeSkua, where every OCX call is dispatched to the UI thread.
* Calls must **never** be made from the event loop thread itself (it would deadlock waiting
  for its own reply). The UI only reads cached state or spawns work on other threads.
* Inbound calls from AS3 (`ExternalInterface.call(...)`) arrive in
  `TuruExternalInterface::call_method` while the player is locked. It forwards them to
  `Bot::on_flash_call`, which only updates state / pushes to channels and returns
  immediately. Anything that needs to call Flash back (e.g. `requestLoadGame` → `loadClient`)
  is done from a bot thread.

## The Flash bridge

`skua.swf` registers ~70 callbacks (full list: `docs/turu/VIBESKUA_SPEC.md`, section 2). The calling
convention matches VibeSkua's `FlashUtil.Call`:

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

1. Desktop starts Ruffle with `skua.swf` (written from the embedded bytes to the data dir).
2. AS3 `Externalizer.init` registers callbacks and calls `requestLoadGame` → bot calls `loadClient`.
3. AS3 fetches `https://game.aq.com/game/api/data/gameversion`, loads the game SWF and calls
   `pre-load` (bot calls `setBackgroundValues`) and then `loaded` (the bot marks the game
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

See `docs/turu/SCRIPTING.md` for the script author guide.

## UI

The window keeps VibeSkua's layout: a 34 px top nav bar (Scripts, Options, Tools & Helpers,
Combat, Bank, Diagnostics, Plugins; Auto / Jump / + on the right) above the game, and tool
windows (Script Loader, Game Options, Skills, Logs, Packet Logger, ...) as egui windows over
the game. Colours, corner radii and fonts come from Mugetsu's menu style
(`desktop/src/gui/turu/theme.rs`). Details: `docs/turu/UI.md`.

## Data directory

`dirs::data_dir()/Turu` (macOS `~/Library/Application Support/Turu`, Windows
`%APPDATA%\Turu`, Linux `~/.local/share/Turu`):

| Path | Contents |
|---|---|
| `skua.swf` | written from the embedded copy at start |
| `api-token` | the control API's token (`--api` without `TURU_API_TOKEN`), owner-readable only |
| `options.json` | bot options (Options > Game, Save; Relogin Try Delay and Relogin Watchdog are saved from Options > Application) |
| `accounts.json` | Army > Accounts: accounts, groups, the launch setup (no passwords: those are in the system keychain, service "Turu") |
| `settings.json` | application settings (Options > Application, `bot/src/settings.rs`), saved on every change |
| `butler.json` | Butler's options, shared by every client (re-read when another client changes it) |
| `butler/<username>.state` | each logged-in account's Butler state (`key=value` lines, `bot/src/butler/sync.rs`): where it is, whom it summons and follows, a `beat` every 5 s; older than 15 s counts as offline |
| `scripts/` | the scripts folder: Search Scripts downloads here (the repo's `scripts/` can be used directly) |
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
push: the index stays off `main`; it is ~77 KB for the 139 ports, where VibeSkua's is 1.1 MB for
1,943 C# files, and a refresh that finds nothing new is an ETag 304; dates come with it (VibeSkua
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
