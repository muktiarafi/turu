# Turu status

What works, what is missing, and the things that trip up work on Turu. Keep it current when
something lands or a gap closes. Read `CLAUDE.md` and `docs/turu/ARCHITECTURE.md` first.

## What works

- **App:** the Ruffle fork (branch `turu`, Ruffle `fda41800` + Mugetsu 1.6.0's patches) starts
  VibeSkua's `skua.swf`, which loads AQW. You log in with the game's own screen.
- **Engine (`bot/`):** every `call("x.y")` in `bot/js/turu.ts` has a Rust implementation;
  background loops for drops, registered quests, skills and boosts; Stop and Pause.
- **Servers and auto-relogin:** `bot.servers` (VibeSkua `Bot.Servers`: server list, login,
  logout, connect, `relogin`, `ensureRelogin`) in `bot/src/api/servers.rs`. With Auto Relogin on,
  a server logout, a kick, a lost connection or an unexpected disconnect (the last two need
  Options > Relogin Watchdog) stops the script, logs back in with the game's remembered login
  and restarts the script; a loading screen stuck for `LoadTimeout` reloads the map or relogs.
  Logging out yourself (the game's Logout or Change Server, `bot.servers.logout()`) disarms it.
  CoreBots' `relogin()` uses it.
- **Command-line login** (VibeSkua's flags, long forms only, as Ruffle has `-p` and `-s`):
  `turu --user <name> --password <password> --server Artix --run-script farm/GoldFarm.ts`
  logs in once the game has loaded and then starts the script. `TURU_PASSWORD` can replace
  `--password` (a command line is visible to other users). `--load-script` only loads,
  `--exit-on-script-stop` logs out and quits when the script stops (not for auto-relogin).
  `bot/src/startup.rs`, flags in `desktop/src/turu.rs`.
- **Windowless mode** (`--headless`, `desktop/src/headless.rs`): no window, GPU or sound; the
  player draws nothing (Ruffle's null renderer) and a loop on the main thread runs it, its
  network tasks and the bot's calls. With the login flags and `--run-script` it farms on its
  own, logging to the terminal; it needs no display (a Linux server). At the login screen
  (debug build, macOS): ~6% CPU and ~96 MB against ~21% and ~420 MB with the window. Device
  fonts aren't loaded, so text in them isn't laid out (reading a text field still works).
  The Headless Mode option is separate: it lowers the game's frame rate in either mode.
- **Control API** (`--api`, or `--api=<address>`; default `127.0.0.1:8791`): HTTP routes from
  VibeSkua-Docker's for status, loading/starting/stopping/pausing scripts, logs (`since`
  paging), the bot options, login, relogin and logout (`bot/src/control_api.rs` lists them).
  Every request needs the token (`Authorization: Bearer` or `X-Api-Token`): `TURU_API_TOKEN`,
  else the data folder's `api-token`, made on first use and readable by you only. Works with
  or without the window. Example:
  `curl -H "Authorization: Bearer $(cat ~/Library/Application\ Support/Turu/api-token)" localhost:8791/status`.
- **Handlers:** `bot.handlers` (VibeSkua `Bot.Handlers`). The engine times them
  (`bot/src/api/handlers.rs`) and the script runs them between bot calls and every 100 ms of
  `bot.sleep`. CoreBots' AFK and registered-quest handlers use it.
- **Scripts:** TypeScript, run in QuickJS after oxc strips the types. Ported: the parts of
  CoreBots, CoreFarms, CoreAdvanced (shop buying) and CoreNation that the ported scripts reach,
  `nation/materials/0MaxBags.ts`, `farm/GoldFarm.ts` and `other/various/Potions.ts`.
- **CoreBots like VibeSkua:** reads Options > CoreBots (`CBO_Storage(<user>).txt`) in
  `setOptions()`, starts from your house when you are in battleon, goes to the CBO stop map
  when the script is stopped, and uses boosts from the CBO Boosters switches.
- **Script options:** a script exports `scriptOptions` (`defineScriptOptions`); Scripts > Edit
  Script Options edits them, saved in `options/<Storage>.cfg` like VibeSkua, and `bot.config.get`
  reads them (`docs/turu/SCRIPTING.md`, "Options"). Start opens the window and waits for it
  unless the script sets `dontPreconfigure`; CoreBots' `setOptions()` opens it until
  `Core.skipOptions` ("Skip this window next time") is ticked, as in VibeSkua.
- **UI:** VibeSkua's nav bar and the windows listed in `docs/turu/UI.md` (Script Loader, Script
  Options, Game and CoreBots options, Skills, Stats, Logs, Logger, Current Drops, Auto, Jump).

The automated tests (`cargo test -p turu_bot`, the desktop layout test) run against a fake game.
In-game behaviour is checked by hand, so expect fixes in game paths and timings.

## Known gaps

- **Relogin is unverified in game:** the tests run it against a fake login screen. Expect fixes
  in timings and in how the real `skua.swf` answers (`getActiveModalMessage`, `clickServer`).
- **Windowless mode is unverified in game:** it loads AQW to the login screen; a login and a
  farm without a window still need a run. The control API is the only way to steer it.
  CoreBots' restart after a logout (`Bot.Manager.RestartScriptAsync`) isn't ported.
- **Handlers wait during one engine wait:** a handler doesn't run while a single Rust-side wait
  is blocking (`bot.wait.forMapLoad`, `forCellChange`, `forActionCooldown`, `forFullyRested`,
  ...); it runs at the next bot call after that. VibeSkua's timer thread runs them all the time.
  Closing it needs the script's JS context inside those waits; the handler table is ready for it.
- **`FlashArg` has no object variant:** equipping goes through the `equipItem` packet; check
  consumables in game. The quest-limit handler (an object `setGameObject`) is dropped.
- **No counter-attack pause** (`Bot.UltraBossHelper`).
- **No reputation API:** CoreBots reads `world.myAvatar.factions` itself.
- **Script options:** option groups (`MultiOptions`) and enum options aren't ported. The window
  never opens by itself with `--headless`, or for runs started from the command line, the HTTP API
  or auto-relogin (VibeSkua's `SilentConfig` covers only the command line and the scheduler; its
  relogin restart can show it).
- **Potions is unverified in game:** its runtime test only checks that it loads, reads its
  options and stops (the fake game doesn't serve inventory, shops or maps). Farming, the alchemy
  crafts and buying are not checked yet.
- Quest rewards and shop items lack some fields; CoreBots reads them from the game objects.
- Not built in the UI: Scheduler, Application options, Themes, HotKeys, Loadouts, Fast Travel,
  Loader, Grabber, Junk Items, Runtime, Console, Spammer, Interceptor, Plugins, the script
  browser, the skills editor, Auto Hunt (`docs/turu/UI.md` has the full list).
- No release packaging yet: Mugetsu's `../mugetsu/source/build-*.sh` still need a Turu version.

## Things to know

- **Noise in the log:** before login, skua.swf's `QuestRequirementWiki` logs a `ModalStack` null
  error every 100 ms. It's harmless (Flash's release player hid it).
- **Clippy on the desktop crate** stops at an existing error in Ruffle's
  `render/wgpu/src/surface/commands.rs` (`redundant_clone`). Lint Turu's code past it with
  `cargo clippy -p turu <features> -- -A clippy::redundant_clone`; the findings left are in
  Ruffle and Mugetsu files.
- **Formatting:** the bot crate uses `cargo fmt`. The files in `desktop/src/gui/turu/` are
  formatted with `max_width = 120`, so format one file at a time
  (`rustfmt --edition 2024 --config max_width=120 <file>`). Running rustfmt on `mod.rs`
  reformats every window.
- `docs/turu/mugetsu-patch.md` describes what Mugetsu's patch changes, including the egui
  palette.
