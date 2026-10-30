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

34 px high, left to right as in VibeSkua: **Scripts** (Script Loader, Edit Script Options, Scheduler), **Options**
(Game, Application, CoreBots, Application Themes, HotKeys), **Tools & Helpers** (Loadouts
Manager, Fast Travel, Loader, Grabber, Current Drops, Junk Items), **Combat** (Skills, Stats,
Runtime), **Bank** (direct action), **Diagnostics** (Logs, Console, Spammer, Logger,
Interceptor), **Plugins** (View Plugins). Right side: **Auto**, **Jump**, a divider, **+**.

| Item | State |
|---|---|
| Script Loader, Game, Application, CoreBots, Current Drops, Skills, Stats, Logs, Logger | implemented |
| Bank | implemented: `bank.open` through `spawn_action` |
| Auto popup | Auto Attack and Stop Automation implemented, as VibeSkua's `AutoViewModel`/`ScriptAuto`: the class drop-down lists the Class items you own (re-read whenever it opens, which also clears the selection); picking one equips it and lists its saved modes (first one selected); Auto Attack uses that class and mode, or the equipped class in Base when either is unset; it turns Infinite Range on, sets the spawn point and attacks the Manual MapIDs in priority order, else your current target, else any living monster. Auto Hunt is disabled. |
| Jump popup | implemented, as VibeSkua's `JumpViewModel`: picking a cell jumps at once ("Enter" uses pad Spawn, otherwise the chosen pad or Left); the cell list is refreshed when the popup or the cell drop-down opens; pads are VibeSkua's list (Spawn, Center, Left, Right, Up, Down, Top, Bottom); Current selects your cell and pad without jumping; Jump re-jumps to the selection. Jumps need a login and don't auto-correct the pad (`map.jump(cell, pad, false)`); the result shows under the buttons. Arrow-key cell stepping isn't ported. |
| + | opens the Script Loader (VibeSkua opens the tabbed tool host) |
| everything else | greyed, tooltip "Coming soon" |

**Auto Attack** (`auto.rs`): a bot thread marked as a script thread, so `bot.control.request_stop()`
(Stop Automation) makes the next call fail. It runs the steps in the Auto popup row above. It is
disabled while a script runs, and Start Script is disabled while Auto Attack runs.

## Windows

* **Script Loader** (`script_loader.rs`, 370x500): header card ("LOADED SCRIPT", file name in
  monospace or "No script loaded", status pill with a coloured dot and `ScriptStatus::label()`);
  Load Script (rfd dialog filtered to .ts/.js, starting in the repo's `scripts/` found next to the
  working directory or above the executable, else the bot's scripts folder); Open VSCode (enabled
  when `code` is on PATH, or the macOS app bundle is found); Edit Script (OS default app);
  Search Scripts (disabled); Edit Script Options (enabled once a script with options is loaded);
  Start/Stop Script (primary/danger; "Force
  Stop" while stopping handlers run); Pause/Resume; SCRIPT LOG with count badge, Copy/Save/Clear, following the end, selectable text.
  The "Report Issues" footer is omitted.
* **Script Options** (`script_options.rs`, 560x460; VibeSkua's "Script Options" dialog): the loaded
  script's options in one list, each with its name, description and editor (switch, number box,
  text box or a drop-down for a choice, its names with `_` as spaces; the default shows on hover).
  Each option group (`MultiOptions`) follows in its own card under its name. Reset Defaults, Cancel and Save. The window edits a copy
  taken when it opens, so Cancel and closing it drop the changes; Save writes `options/<Storage>.cfg`.
  Opened from Scripts > Edit Script Options or the Script Loader's button, and by itself when the
  script asks (on Start unless it sets `dontPreconfigure`, or `bot.config.configure()`): the script
  waits until Save or Cancel, and Stop closes it.
* **Message boxes** (`message_box.rs`): what a script shows with `bot.showMessageBox`, as an egui
  modal over the windows (caption, message, the buttons left to right, the last one primary).
  The script waits for the click; Escape or a click outside answers "closed", and Stop closes it.
* **Game options** (`game_options.rs`, 612x720): search box, chips All/Visuals/Combat/Drops/
  Relogin/Gameplay, the five sections with the fields of `BotOptions` (switches, number boxes,
  text boxes, Relogin Server and Hunt Priority combos, a Reload Map button). Edits are written to
  `bot.options` at once; footer Defaults / Reset (reload `options.json`) / Save. Differences from
  VibeSkua: no Optimize Players, Upgrade/Staff Name Colour (not in `BotOptions`); the server combo
  uses VibeSkua's fallback list (plus the current value); Set FPS defaults to 24, the rate
  AQW's `Game.swf` is made for (VibeSkua: 30, skua.swf's rate).
* **Application Options** (`application_options.rs`, 580x650, VibeSkua `ApplicationOptionsView`):
  search box, chips All/General/Updates/Webhooks with counts, each setting with its description.
  Changes are saved at once (`settings.json`, `AppSettings`). General: Show Username in Title
  (the window title gets " – <character>" while you play), Client Animation Frame-rate (the
  `stage.frameRate` set when the client loads, default 24; Set FPS takes over in game; VibeSkua
  stores it and never reads it), Relogin Try Delay and Relogin Watchdog (bot options: applied
  and saved into `options.json` on their own), Clear Flash Cache (deletes Ruffle's SharedObjects
  folder after a second click, then restart). Updates: the six switches are stored but greyed
  ("Coming soon"): Turu downloads no updates. Webhooks: Discord and Skua Nexus URLs with Test
  buttons, the notify switches, Drop List, Health Pings and their interval, sent by
  `bot/src/webhooks.rs` (VibeSkua `DiscordWebhookService`): one thread posting embeds, two a
  second, retrying on 429; the Nexus snapshot and health pings read the game on that thread. The
  drop alert has no screenshot (VibeSkua attaches one). Failures go to the Debug log.
* **Logs** (`logs.rs`): Script / Debug tabs, Copy/Save/Clear. **Logger** (`packet_logger.rs`):
  Start/Stop Logging turns `bot.logs.packet_logging` on or off; while it is on the engine keeps
  the outgoing packets in `bot.logs.packets` (1000 lines), search box, count, Copy/Save/Clear. Not built: the filter button.
* **Stats** (`stats.rs`): script clock (keeps its last value after the script ends), KILLS,
  DEATHS, PICKUPS, ACCEPTED, COMPLETED, RE-LOGINS, Reset. The STORAGE section is not built.
* **Skills** (`skills.rs`): "Saved Skills" tab: Load Classes, class and mode combos from
  `bot.paths.load_advanced_skills()`, the skill entries with their rules as text, Refresh.
  "Create / Edit Skill" is a "Coming soon" placeholder; Open, Edit Selected, Remove Selected and
  Reset Sets are disabled.
* **CoreBots Options** (`core_bots.rs`, 800x608, VibeSkua `CoreBotsView`): profile (username or
  "Not logged in"), Load / Load character / Save, tabs Loadout / Options / Other. Loadout: Solo,
  Farm, Dodge and Boss cards (class from the enhanced classes you own plus `[Current]`, skill set
  mode, "Use Dedicated Equipment Loadout") and a collapsible "Detailed Equipment Slots" with six
  gear drop-downs per role (with a "(none)" entry to clear a slot, which VibeSkua lacks); the
  cards split the window width in two, so long names are cut off rather than widening it.
  Options: the 17 CoreBots settings in two columns (switches, numbers, text), searchable. Other:
  Boosters (read by CoreFarms.toggleBoost, which runs `bot.boosts`), Nation Farms, PvP, Bot
  Creators switches. Opening the window
  loads `options/CBO_Storage(<username>).txt` (cached per character), closing it saves; Save asks
  before saving public rooms (a save without the answer keeps private rooms). Game reads
  (username, equipped class, inventory) run through `spawn_action`; VibeSkua's message boxes are
  egui modals. Not ported: adding a `[Current]` class with no skill set to the skill file
  (`EnsureSkillEntryExists`). Unlike VibeSkua, the drop-downs list your classes and gear as soon
  as the window knows who you are, even without a saved file.
* **Current Drops** (`current_drops.rs`): search, AC count, AC only, checkable list, Pick up all
  (`drops.pickupAll`) and Pick up selected (`drops.pickup` with the item IDs), through `spawn_action`.

Not built at all (greyed in the menus): Scheduler, Application Themes,
HotKeys, Loadouts Manager, Fast Travel, Loader, Grabber, Junk Items, Runtime, Console, Spammer,
Interceptor, Plugins, Script Repo (Search Scripts), Auto Hunt.

## Adding a window

Add `foo.rs` with a struct and `show(&mut self, ctx, &mut bool)`, build the window with
`super::window(ctx, "Title", [w, h], slot)`, add a flag to `Windows`, a field and a call in
`TuruUi::show_windows`, and enable the entry in `nav_bar.rs`.
