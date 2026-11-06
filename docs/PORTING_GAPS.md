# VibeSkua features not in Turu

What VibeSkua has that Turu doesn't (as of 2026-11-01), from `docs/STATUS.md`, `docs/UI.md`,
`docs/VIBESKUA_SPEC.md` and VibeSkua's source. Mark each row's **Decision** column: `port`,
`skip` (won't port) or `later`. "C# uses" counts the files in `../Scripts` that call it.

## Client windows (greyed "Coming soon" in the nav bar)

| # | Feature | VibeSkua | Notes | Decision |
|---|---|---|---|---|
| 1 | Scheduler | Scripts > Scheduler | Queue of scripts run one after another; also the Script Repo's Queue button. **Ported** (`bot/src/scheduler.rs`, `frontend/src/gui/turu/scheduler.rs`), with the Queue button. | port (done) |
| 2 | Application Themes | Options > Application Themes | Accent editor, presets, user themes, backgrounds. Turu has one fixed Mugetsu palette. | |
| 3 | HotKeys | Options > HotKeys | Bindings for OpenBank, Toggle Headless/Streamer/HidePlayers/FX/Magnetise/InfiniteRange, Logout, StopScript, JumpToHome, ... and Army broadcasts. **Ported** (`frontend/src/gui/turu/hotkeys.rs`, `hotkeys_window.rs`): VibeSkua's whole catalog, rebindable, saved in `HotKeys.json`; works while Turu's window has focus (no global hook); hidden on Android. | port (done) |
| 4 | Loadouts Manager | Tools & Helpers | Save and equip gear sets. **Ported** (`frontend/src/gui/turu/loadouts.rs`, `bot/src/loadouts.rs`), laid out for phones too: VibeSkua's file format; Equip holds while a script runs (Stop script & equip) and enhancing is its own action. No `bot.loadouts` for scripts, army sets or control API routes yet (`ARMY_EQUIPMENT_PROPOSAL.md` phase 2). | port (done) |
| 5 | Fast Travel | Tools & Helpers | Saved map/cell/pad destinations, private room toggle. **Ported** (`frontend/src/gui/turu/fast_travel.rs`, `turu_bot::api::map::fast_travel`), laid out for phones too: tiles in 1-3 columns with a ⋮ menu, editing in the add form, saved at every change to `FastTravels.json`. | port (done) |
| 6 | Loader | Tools & Helpers | Load shops/quests by ID, quest cache (update, rebuild, range), copy IDs, fake complete. **Ported** (`frontend/src/gui/turu/loader.rs`, `bot/src/quest_cache.rs`): same `QuestData.json` format in Turu's data folder. Turu ships no cache (VibeSkua bundles a 19 MB one): the Loader downloads the community cache (`auqw/Scripts`) when it opens without one, and its Download button refreshes it. | port (done) |
| 7 | Grabber | Tools & Helpers | Dumps shops, quests, inventory, bank, monsters... into lists for script writing. **Ported** (`frontend/src/gui/turu/grabber.rs`, `bot/src/grabber.rs`) with every list's actions. GetMap Item IDs reads the map SWF's AS3 bytecode (`frontend/src/map_items.rs`) instead of running FFDec. | port (done) |
| 8 | Junk Items | Tools & Helpers | `JunkItems.json` sell list; tied to `Bot.SellJunk` (#41). **Ported** (`frontend/src/gui/turu/junk_items.rs`, `bot/src/junk.rs`): same `JunkItems.json` format in Turu's data folder, every control of VibeSkua's window. | port (done) |
| 9 | Console | Diagnostics | C# snippet runner. **Ported** as a TS snippet runner (`frontend/src/gui/turu/console.rs`, `script::run_snippet`): result, `bot.log` lines and errors shown, Stop works. | port (done) |
| 10 | Spammer | Diagnostics | Repeat packets to server or client on an interval, queue. **Ported** (`frontend/src/gui/turu/spammer.rs`, `bot/src/spammer.rs`): same queue file (one packet per line); the loop runs on its own thread and also stops when a send fails. | port (done) |
| 11 | Interceptor | Diagnostics | Local TCP MITM proxy (`CaptureProxy`) logging/editing/blocking packets both ways. **Ported** (`frontend/src/gui/turu/interceptor.rs`, `bot/src/capture.rs`, `frontend/src/capture.rs`) in process instead of a proxy: the app wraps Ruffle's navigator and filters the game socket's packets, so Connect needs no relog (picking a server still relogs there) and Disconnect leaves the game connected. Not built: the category filter button. | port (done) |
| 12 | View Plugins / plugin system | Plugins | Loads `ISkuaPlugin` DLLs. Turu builds Butler and Cosmetics in; no plugin loading. | |
| 13 | Daily & Weekly Tracker plugin | Plugins (`Skua.Plugin.DailyTracker`) | Shows which dailies/weeklies are done. | |
| 14 | BotWindow (the "+" tool host) | Nav bar "+" | Tabbed host of every tool window with a search sidebar. Turu's "+" opens the Script Loader. | |

## Smaller UI pieces

| # | Feature | Where | Notes | Decision |
|---|---|---|---|---|
| 15 | Auto Hunt | Auto popup | Disabled in Turu; Auto Attack works. **Ported** (`bot/src/auto.rs`, shared with Auto Attack; the popup in `frontend/src/gui/turu/auto.rs`). | port (done) |
| 16 | Notify Drop | Runtime > Drops helper | Beeps when a listed item drops. | |
| 17 | Stats STORAGE section | Combat > Stats | Inventory/bank use and Get space. | |
| 18 | Logger filter | Diagnostics > Logger | Packet filters (select all, clear, invert). | |
| 19 | Jump arrow-key cell stepping | Jump popup | | |
| 20 | Script Repo extras | Search Scripts | Queue (needs #1), Update dates from Git, custom scripts folder. Queue **ported** with the Scheduler; the other two not built. | |
| 21 | Game options: Optimize Players, Upgrade/Staff Name Colour | Options > Game | Not in `BotOptions`. | |
| 22 | Application > Updates switches | Options > Application | Stored but greyed: auto/check updates for scripts (Search Scripts does this on its own), skill sets, junk items. The junk-item ones stay greyed: VibeSkua stores them but nothing reads them (`GetScriptsService.UpdateJunkItemsFile` is never called), and the client starts with an empty list. | |
| 23 | Webhook drop screenshot | Discord webhook | Turu's drop alert has no image. | |
| 24 | CoreBots `[Current]` class skill entry | Options > CoreBots | `EnsureSkillEntryExists` not ported. | |
| 25 | Light theme | Everywhere | VibeSkua has light and dark dictionaries. | |
| 26 | Script Loader "Report Issues" footer | Script Loader | VibeSkua's Discord link. | |

## Multi-client (tabbed host and Manager app)

| # | Feature | VibeSkua | Notes | Decision |
|---|---|---|---|---|
| 27 | Grid View | Tabbed host | See every client at once. Turu draws only the active tab. | |
| 28 | Start/Stop Auto-Attack for All | Tabbed host Army menu |  **Ported** to Army Control (tabs directly, launched clients through new control API routes). | port (done) |
| 29 | Army Scheduler | Tabbed host Army menu | | |
| 30 | Army Queue | Manager tab | Squads, per-squad default scripts, per-role scripts, presets, Start/Pause/Resume/Stop. | |
| 31 | Account tag editing from scripts | `Bot.Accounts` (C# uses: 3) | Get/Add/Remove/Set/Clear tags, `GetAllAccounts`. Turu has `bot.army.accounts()` (usernames only). | |
| 32 | Per-account Edit Options / Save Setup, `--script-options-storage` | Manager Accounts tab, CLI | Share script options across accounts via a storage override. | |
| 33 | Updater (client self-update) | Manager tab (Velopack) | Turu ships from `release.yml`; no in-app update. | |
| 34 | Manager Themes / Options / About / change log | Manager tabs | | |
| 35 | `--use-theme`, `--gh-token` | CLI | Theme flag depends on #2; token is for GitHub API rate limits. | |

## Script API (`IScriptInterface`) members with no Turu counterpart

| # | Member | C# uses | Notes | Decision |
|---|---|---|---|---|
| 36 | `Bot.Auto` (StartAutoAttack/Hunt from a script) | 0 | | |
| 37 | `Bot.Stats` | 0 | Counters exist in the engine and Stats window, not in `turu.ts`. | |
| 38 | `Bot.InvHelper` (`Check`, `HasAll`, `HasAny` with moveToInventory) | 0 | | |
| 39 | `Bot.AuraMonitor` | 0 | | |
| 40 | `Bot.GameProxy` | 0 | Needs #11. **Ported** for Rust: `bot.capture` (`CaptureProxy`: start/stop, `add_interceptor` with an `Interceptor` that edits `MessageInfo.content` or clears `send`). Not exposed to scripts: interceptors run on the player's thread, a TS callback can't (no C# script uses it). | port (done, Rust only) |
| 41 | `Bot.SellJunk()` | 0 | Needs #8. **Ported** as `bot.sellJunk()` (`junk::sell_all`). | port (done) |
| 42 | `Bot.Schedule(delay, action)` | 0 | CoreTasks covers C#'s `Task.Run` patterns. | |
| 43 | `Bot.Version` / script version gate (`version: x.y.z.w`) | 2 | Search Scripts doesn't flag scripts made for a newer `turu.ts` either. | |
| 44 | `Bot.Manager.RestartScriptAsync` | — | CoreBots' restart after a logout in windowless mode. | |
| 45 | Handlers running during one engine wait | — | VibeSkua's 20 ms timer runs handlers all the time; Turu runs them at the next bot call. | |

## Ported but partial (from STATUS.md, for completeness)

| # | Feature | Missing part | Decision |
|---|---|---|---|
| 46 | Butler (ButlerV5) | AntiLag while following, quest bypass table, quest cloning, level fake, debug log viewer, suspend message box, offline write on close. **Ported** (`bot/src/butler/`: `quest_bypass.rs`, `antilag.rs`, `debug_log.rs`) with Unlock All Quests, except quest cloning. | port (done, but quest cloning) |
| 47 | Cosmetics | `world.CHARS[id]` fallbacks; Copy Page doesn't apply hair. | |
| 48 | Script Options | Never opens by itself for headless, HTTP API or relogin runs (it does for `--run-script` and Army Control's Start, as in VibeSkua). | |
| 49 | Ultras | All of `Ultrasv3` ported but `TestZone` (old `Ultras/` engine). Boss scripts are type-checked drafts, none run in game. | |
| 50 | Scripts skipped on purpose | `Templates`, unused `Prototypes`, the generator, some Tools (`scripts/README.md`). | |
| 51 | Release builds | No PGO speed profile; macOS app not notarized. | |
