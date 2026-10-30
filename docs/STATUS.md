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
- **Reputation and auras:** `bot.reputation` (VibeSkua `Bot.Reputation`: `factionList`, `getRank`,
  `hasRank`), `bot.self` and `bot.target` (VibeSkua `Bot.Self`/`Bot.Target`: `auras`, `snapshots`,
  `getAura`, `getAuraStacks`, `hasActiveAura`, `getAuraValue`, `hasAnyActiveAura`, ...), in
  `bot/src/api/reputation.rs` and `auras.rs`. Skill rules read aura stacks through the same code.
- **Counter-attack pause:** `bot.ultraBossHelper` (VibeSkua `Bot.UltraBossHelper`) and
  `bot.combat.stopAttacking` / `enableCounterHandler` (`bot/src/api/combat.rs`). With it on
  (CoreBots' `setOptions()` turns it on, as in VibeSkua), a `ct` packet's "prepares a counter
  attack!" stops attacks and drops the target, and the "Counter Attack" aura attacks that target
  again; meanwhile `combat.attack` refuses and the skill timer waits up to 10 s. The packet
  handler only sets flags; the target calls go through the bot's work thread.
- **Handlers:** `bot.handlers` (VibeSkua `Bot.Handlers`). The engine times them
  (`bot/src/api/handlers.rs`) and the script runs them between bot calls and every 100 ms of
  `bot.sleep`. CoreBots' AFK and registered-quest handlers use it.
- **Scripts:** TypeScript, run in QuickJS after oxc strips the types. Ported: the parts of
  CoreBots, CoreFarms, CoreAdvanced (shop buying, the merge-shop template, gear and enhancement)
  and CoreNation that the ported scripts and the FarmerJoe include tree reach (the Core boss kills,
  PvP moves, the brawls, fishing, boosts and 17 more reputations, The Assistant, the Shadow Blast
  medals), all of CoreStory (for the story scripts) and CoreDailies,
  `nation/materials/0MaxBags.ts`,
  `farm/GoldFarm.ts`, `other/various/Potions.ts` and `other/mergeshops/SynderesMerge.ts`.
- **FarmerJoe:** `other/farmjoekits/0FarmerJoeDoAll.ts` (VibeSkua's level 1 to 100
  progression) and the 127 content scripts its `DoAll` reaches: classes, weapons, the story cores,
  UnlockForgeEnhancements, the Nation, Legion, SDKA, BLoD and Awe trees, 19 merge scripts.
  `scripts/README.md` ("FarmerJoe") lists what each file leaves out, the 19 included files that
  aren't ported (DoAll never calls them) and the spots that differ from C#.
- **CoreBots like VibeSkua:** reads Options > CoreBots (`CBO_Storage(<user>).txt`) in
  `setOptions()`, starts from your house when you are in battleon, goes to the CBO stop map
  when the script is stopped, and uses boosts from the CBO Boosters switches.
- **Script options:** a script exports `scriptOptions` (`defineScriptOptions`); Scripts > Edit
  Script Options edits them, saved in `options/<Storage>.cfg` like VibeSkua, and `bot.config.get`
  reads them (`docs/turu/SCRIPTING.md`, "Options"). Start opens the window and waits for it
  unless the script sets `dontPreconfigure`; CoreBots' `setOptions()` opens it until
  `Core.skipOptions` ("Skip this window next time") is ticked, as in VibeSkua.
- **Application options and webhooks:** Options > Application (`settings.json`): username in
  the title, the client's starting frame rate (24 by default, as is Set FPS: AQW's own rate),
  relogin delay and watchdog, Clear Flash Cache, and Discord / Skua Nexus webhooks for script
  start, stop and crash, relogins, listed drops and health pings (`bot/src/webhooks.rs`).
  Unverified against a real webhook; the update switches are stored only.
- **UI:** VibeSkua's nav bar and the windows listed in `docs/turu/UI.md` (Script Loader, Script
  Options, Game, Application and CoreBots options, Skills, Stats, Logs, Logger, Current Drops, Auto, Jump).

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
- **Objects for Flash are new:** `FlashArg` carries arrays and objects now (`bot.flash.set(path,
  {})`, used by CoreStory's `preLoad`), but nothing has sent one in game yet. Equipping still goes
  through the `equipItem` packet (check consumables in game), and CoreBots' quest-limit handler
  isn't restored.
- **The counter-attack pause is unverified in game:** its tests run the `ct` packets against a
  fake game. Check the skill timer's 10 s wait and the re-attack on a real counter-attack boss.
- **The FarmerJoe Core members are unverified in game:** the new CoreBots (`killKitsune`,
  `killDoomKitten`, `huntMonsterMapID`, `pvpMove`, `usePotion`, ...), CoreFarms (the Bludrut and
  Death Pit brawls, `fishingREP`, `getBoost`, the new REPs) and CoreNation (`theAssistant`,
  `swindleReturn`, `emblemofNulgath`) members only type-check. `usePotion` sends the action
  object to `world.testAction` (an object for Flash, see below). Four C# bugs are fixed on
  purpose (marked `C#` in the code): `runRep` farms Blacksmithing (gold method) and Fishing
  instead of stopping, `huntMonsterMapID` without an item stops after one kill, `theAssistant`
  runs its return policy, and `getFish` waits for the dynamite to be used.
- **Auras are unverified in game:** `bot.self` and `bot.target` call skua.swf's `Auras.as`
  callbacks as VibeSkua does, but only a fake game has answered them. Unlike VibeSkua,
  `snapshots` (and so `getAuraStacks` and skill aura rules) drops a timed, non-persistent aura
  with no time left, as Grimlite-Li's `AuraManager` does; check in game that `remaining` is set
  for every timed aura, or live auras would read as gone. The same goes for
  `bot.reputation` (`world.myAvatar.factions`), which CoreBots' `factionRank` now uses.
- **Script options:** the window never opens by itself with `--headless`, or for runs started from the command line, the HTTP API
  or auto-relogin (VibeSkua's `SilentConfig` covers only the command line and the scheduler; its
  relogin restart can show it).
- **CoreStory is unverified in game:** its test only checks that `preLoad` finds the quests of a
  story function. `killQuest`, `mapItemQuest` and the unlock-recovery loop of `questProgression`
  need a story run.
- **The merge template is unverified in game:** its tests run it against a fake shop (picking the
  items, handing a requirement the shop doesn't sell to `findIngredients`); buying and the nested
  merges need a real merge run (SynderesMerge is the smallest).
- **Enhancement is unverified in game:** CoreAdvanced's `enhanceItem`, `enhanceEquipped`,
  `smartEnhance`, `gearStore` and `rankUpClass` follow C#, and their tests run them against fake
  enhancement shops (which shop, which enhancement, the `enhanceItemShop` packet, skipping what's
  done, locked specials). Check in game the real shops' item names: C# picks a special by its
  name and, on a tie, the first plain "... Cape/Helm/Weapon" entry would win. C#'s odd spots are
  kept (Forge = 1 takes the Awe-shop path since the specials were renumbered). Items carry
  `enhancementPatternId` and `procId` now (`EnhPatternID`, `ProcID`); the weapon proc and the
  applied enhancement ID are read from `world.invTree` as C# does. `rankUpClass` (Icestorm
  Arena) needs a real run.
- **CoreDailies is unverified in game:** its tests check `checkDailyv2` (daily done, max
  stacks, the drop list) against a fake game. The dailies themselves, the friendship packets and
  `bot.player.webBadges` (a request to account.aq.com's character page, for `Core.hasWebBadge`)
  need real runs.
- **FarmerJoe is unverified in game:** nothing of the tree has run. It type-checks and its
  runtime test only loads DoAll (all 128 modules), runs `setOptions` and stops. The C# was
  converted by a script and fixed by hand, so expect wrong arguments in calls no test reaches;
  the Core members it added (`killMonster` by monster map ID, `isSeasonalMapActive`,
  `configOr`/`configEnum`, `bot.quests.updateQuest(value, slot)`) only have fake-game or no
  tests. Content scripts read options of their own that DoAll doesn't declare as C#'s defaults
  (`configOr`), as VibeSkua does. Turu scripts can't ask Yes/No: Legion's join keeps the "Undead
  Warrior" armor instead of asking to sell it. Three C# bugs are fixed on purpose (marked `C#`):
  FarmerJoe's pet step reads its real option, TheLeeryContract uses the reward it's given, and
  Dage PvP reads the CBO solo-boss switch's value.
- **Potions is unverified in game:** its runtime test only checks that it loads, reads its
  options and stops (the fake game doesn't serve inventory, shops or maps). Farming, the alchemy
  crafts and buying are not checked yet.
- Quest rewards and shop items lack some fields; CoreBots reads them from the game objects.
- Not built in the UI: Scheduler, Themes, HotKeys, Loadouts, Fast Travel,
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
