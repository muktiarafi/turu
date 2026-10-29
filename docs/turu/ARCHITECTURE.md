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
    events.rs            inbound Flash calls (`pext`, `packet`, `loaded`, ...) -> state + event bus
    state.rs             game state derived from packets (drops, map, counters, ...)
    options.rs           bot options (VibeSkua `IScriptOption`) + persistence
    models.rs            game JSON models (InventoryItem, Monster, Quest, ShopItem, ...)
    api/                 one module per VibeSkua sub-interface (player, map, combat, kill, hunt,
                         quests, drops, inventory, bank, temp_inv, house, shops, skills, wait,
                         send, lite, monsters, servers)
    background.rs        20 ms "ScriptInterface" timer: option enforcement, watchdog, handlers
    script/              ScriptManager: load/start/stop/pause, TS transpile, QuickJS runtime, bindings
desktop/src/turu/        desktop integration: FlashHost impl, ExternalInterface provider, launch
desktop/src/gui/turu/    egui UI: top nav bar, Script Loader, Options, Skills, Logs, ... (VibeSkua layout)
scripts/                 TypeScript scripts (types, Core libraries, ported farms)
docs/turu/               these docs
```

## Process and threads

```
winit event loop thread (main)          script thread ("Script Thread")      background threads
───────────────────────────────         ───────────────────────────────      ────────────────────
Ruffle Player (Arc<Mutex<Player>>)       QuickJS runtime                       ScriptInterface timer (20 ms)
egui UI (reads Bot state)                user script + Core libraries          drops loop, quest loop,
TuruExternalInterface (inbound) ──┐      bot.* host calls ─┐                   skills timer
                                  │                         │
                                  ▼                         ▼
                           Bot::on_flash_call       Flash::call(name, args)
                           (never blocks,           = send RuffleEvent::TuruFlashCall
                            never locks Player)       and block on a reply channel
```

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

* Arguments are strings, numbers, booleans or null (`FlashArg`).
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
   `pre-load` (bot calls `setBackgroundValues`) and then `loaded` (bot starts the 20 ms timer).
4. The user logs in with the game's own login screen (auto-login via CLI flags comes later).

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
| `settings.json` | app settings, bot options (`options`) |
| `scripts/` | default scripts folder (the repo's `scripts/` can be used directly) |
| `options/<storage>.json` | per-script option values |
| `AdvancedSkills.json` | the shipped skill sets (from `auqw/Scripts` `Skills/AdvancedSkills.json`, rewritten from the app's copy) |
| `UserAdvancedSkills.json` | your skill sets, used by the bot: seeded from the shipped ones, then only gaining the shipped classes and modes it lacks (VibeSkua's merge) |
| `logs/` | script logs |
