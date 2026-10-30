# Turu status

What works, what is missing, and the things that trip up work on Turu. Keep it current when
something lands or a gap closes. Read `CLAUDE.md` and `docs/turu/ARCHITECTURE.md` first.

## What works

- **App:** the Ruffle fork (branch `turu`, Ruffle `fda41800` + Mugetsu 1.6.0's patches) starts
  VibeSkua's `skua.swf`, which loads AQW. You log in with the game's own screen.
- **Engine (`bot/`):** every `call("x.y")` in `bot/js/turu.ts` has a Rust implementation;
  background loops for drops, registered quests, skills and boosts; Stop and Pause.
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
  reads them (`docs/turu/SCRIPTING.md`, "Options").
- **UI:** VibeSkua's nav bar and the windows listed in `docs/turu/UI.md` (Script Loader, Script
  Options, Game and CoreBots options, Skills, Stats, Logs, Logger, Current Drops, Auto, Jump).

The automated tests (`cargo test -p turu_bot`, the desktop layout test) run against a fake game.
In-game behaviour is checked by hand, so expect fixes in game paths and timings.

## Known gaps

- **No servers API:** no auto-relogin or watchdog (`background.rs` has the TODO). `Core.relogin`
  logs out and waits for a relogin that never comes; VibeSkua's restart of a script after a
  logout is missing too.
- **Handlers wait during one engine wait:** a handler doesn't run while a single Rust-side wait
  is blocking (`bot.wait.forMapLoad`, `forCellChange`, `forActionCooldown`, `forFullyRested`,
  ...); it runs at the next bot call after that. VibeSkua's timer thread runs them all the time.
  Closing it needs the script's JS context inside those waits; the handler table is ready for it.
- **`FlashArg` has no object variant:** equipping goes through the `equipItem` packet; check
  consumables in game. The quest-limit handler (an object `setGameObject`) is dropped.
- **No counter-attack pause** (`Bot.UltraBossHelper`).
- **No reputation API:** CoreBots reads `world.myAvatar.factions` itself.
- **Script options:** `DontPreconfigure` isn't ported, so the Script Options window never opens by
  itself before a run (VibeSkua can show it on Start). Option groups (`MultiOptions`) and enum
  options aren't ported either.
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
