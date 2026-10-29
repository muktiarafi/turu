# Turu UI

Turu uses **VibeSkua's layout and flow** (see `VIBESKUA_SPEC.md`, section 5) in **Mugetsu's
look** (its right-click panel: dark panel, purple accent, corner radius 12, soft shadow).
Code: `desktop/src/gui/turu/`. Integration points: `desktop/src/gui.rs` (`RuffleGui::update`
draws the Turu nav bar instead of Ruffle's menu bar and then the tool windows when
`mugetsu::plays_aqw()`), `gui::MENU_HEIGHT` (34), `mugetsu::window_title()` ("Turu <version>").

Rules: UI code runs on the winit thread and only reads cached bot state (`bot.state`,
`bot.logs`, `bot.options`, `bot.scripts`) with short locks. Anything that touches the game
goes through `crate::turu::spawn_action`, which runs on a bot thread. File dialogs use the
existing `FilePicker` (rfd, async) and never block the UI.

## Theme (`theme.rs`)

| Role | Colour |
|---|---|
| Panel / window | `#11131A` (Mugetsu `PANEL`, opaque) |
| Card | `#181B25` |
| Inset (text boxes, lists) | `#0C0D13` |
| Edge | Mugetsu `EDGE` (`#3A3E4E`) |
| Hover / track | Mugetsu `ROW_HOVER` / `TRACK` |
| Accent (primary buttons, switches, selection) | Mugetsu `ACCENT` `#7C6CF0` (VibeSkua's emerald is replaced) |
| Text / muted | Mugetsu `TEXT` / `MUTED` |
| Success / warning / danger | VibeSkua's `#34D399` / `#FBBF24` / `#EF4444` |

`theme::install` sets this style on both of egui's themes once, so the nav bar, popups and
windows (and Ruffle's own dialogs) are dark Turu-styled whatever the OS theme is. Mugetsu's
right-click menu is untouched. Body text is 13 px, buttons have radius 8, windows radius 12.
The windows are not scaled with the window height (Mugetsu's `z`): they keep VibeSkua's sizes
(370x500, 612x720, ...) and are clamped to the window.

## Nav bar (`nav_bar.rs`)

34 px high, left to right as in VibeSkua: **Scripts** (Script Loader, Scheduler), **Options**
(Game, Application, CoreBots, Application Themes, HotKeys), **Tools & Helpers** (Loadouts
Manager, Fast Travel, Loader, Grabber, Current Drops, Junk Items), **Combat** (Skills, Stats,
Runtime), **Bank** (direct action), **Diagnostics** (Logs, Console, Spammer, Logger,
Interceptor), **Plugins** (View Plugins). Right side: **Auto**, **Jump**, a divider, **+**.

| Item | State |
|---|---|
| Script Loader, Game, Current Drops, Skills, Stats, Logs, Logger | implemented |
| Bank | implemented: `bank.open` through `spawn_action` |
| Auto popup | Auto Attack and Stop Automation implemented (class/mode combos from `AdvancedSkills.json`); Auto Hunt and the MapIDs box are disabled |
| Jump popup | implemented, as VibeSkua's `JumpViewModel`: picking a cell jumps at once ("Enter" uses pad Spawn, otherwise the chosen pad or Left); the cell list is refreshed when the popup or the cell drop-down opens; pads are VibeSkua's list (Spawn, Center, Left, Right, Up, Down, Top, Bottom); Current selects your cell and pad without jumping; Jump re-jumps to the selection. Jumps need a login and don't auto-correct the pad (`map.jump(cell, pad, false)`); the result shows under the buttons. Arrow-key cell stepping isn't ported. |
| + | opens the Script Loader (VibeSkua opens the tabbed tool host) |
| everything else | greyed, tooltip "Coming soon" |

**Auto Attack** (`auto.rs`): a bot thread marked as a script thread, so `bot.control.request_stop()`
(Stop Automation) makes the next call fail. It calls `skills.startAdvanced(class|null, true, mode)`
(mode defaults to Base), then loops `kill.monster("*")` while in game. It is disabled while a
script runs, and Start Script is disabled while Auto Attack runs.

## Windows

* **Script Loader** (`script_loader.rs`, 370x500): header card ("LOADED SCRIPT", file name in
  monospace or "No script loaded", status pill with a coloured dot and `ScriptStatus::label()`);
  Load Script (rfd dialog filtered to .ts/.js, starting in the repo's `scripts/` found next to the
  working directory or above the executable, else the bot's scripts folder); Open VSCode (enabled
  when `code` is on PATH, or the macOS app bundle is found); Edit Script (OS default app);
  Search Scripts and Edit Script Options (disabled); Start/Stop Script (primary/danger);
  Pause/Resume; SCRIPT LOG with count badge, Copy/Save/Clear, following the end, selectable text.
  The "Report Issues" footer is omitted.
* **Game options** (`game_options.rs`, 612x720): search box, chips All/Visuals/Combat/Drops/
  Relogin/Gameplay, the five sections with the fields of `BotOptions` (switches, number boxes,
  text boxes, Relogin Server and Hunt Priority combos, a Reload Map button). Edits are written to
  `bot.options` at once; footer Defaults / Reset (reload `options.json`) / Save. Differences from
  VibeSkua: no Optimize Players, Upgrade/Staff Name Colour (not in `BotOptions`); Relogin Try
  Delay sits in the Relogin section because the Application window is not built; the server combo
  uses VibeSkua's fallback list (plus the current value).
* **Logs** (`logs.rs`): Script / Debug tabs, Copy/Save/Clear. **Logger** (`packet_logger.rs`):
  Start/Stop Logging subscribes to the bot's `PacketSent` events and fills `bot.logs.packets`
  (1000 lines), search box, count, Copy/Save/Clear. Not built: the filter button.
* **Stats** (`stats.rs`): script clock (keeps its last value after the script ends), KILLS,
  DEATHS, PICKUPS, ACCEPTED, COMPLETED, RE-LOGINS, Reset. The STORAGE section is not built.
* **Skills** (`skills.rs`): "Saved Skills" tab: Load Classes, class and mode combos from
  `bot.paths.load_advanced_skills()`, the skill entries with their rules as text, Refresh.
  "Create / Edit Skill" is a "Coming soon" placeholder; Open, Edit Selected, Remove Selected and
  Reset Sets are disabled.
* **Current Drops** (`current_drops.rs`): search, AC count, AC only, checkable list, Pick up all
  (`drops.pickupAll`) and Pick up selected (`drops.pickup` with the item IDs), through `spawn_action`.

Not built at all (greyed in the menus): Scheduler, Application, CoreBots, Application Themes,
HotKeys, Loadouts Manager, Fast Travel, Loader, Grabber, Junk Items, Runtime, Console, Spammer,
Interceptor, Plugins, Script Repo (Search Scripts), Auto Hunt.

## Adding a window

Add `foo.rs` with a struct and `show(&mut self, ctx, &mut bool)`, build the window with
`super::window(ctx, "Title", [w, h], slot)`, add a flag to `Windows`, a field and a call in
`TuruUi::show_windows`, and enable the entry in `nav_bar.rs`.
