# Turu UI

Turu uses **VibeSkua's layout and flow** (see `VIBESKUA_SPEC.md`, section 5) in **Mugetsu's
look** (its right-click panel: dark panel, purple accent, corner radius 12, soft shadow).
Code: `frontend/src/gui/turu/`. Integration points: `app/src/gui.rs` (`RuffleGui::update`
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
right-click menu is untouched. Body text is 13 px, buttons have radius 8, windows radius 12. Buttons (`widgets::button`) shrink to 96% while pressed and spring back, and with a pointer a soft glow follows the cursor over a hovered button (VibeSkua `HoverGlow`; none on a touch screen).
The windows are not scaled with the window height (Mugetsu's `z`): they keep VibeSkua's sizes
(370x500, 612x720, ...) and are clamped to the window. They stay below the nav bar and shrink
with the app window, also after they opened; what no longer fits scrolls, so the footer buttons
stay reachable (`super::window`; `windows_fit_small_screens` checks the windows at 1400x1000,
900x600 and 640x420). Runtime stacks its two columns when the window is narrower than 680 px.

Symbols in labels (✕, ↑ ↓, ⋯ menus, ▾, ...) come from egui's own fonts or, for the ones those
lack, `frontend/assets/DejaVuSans-Symbols.ttf`: DejaVu Sans cut down to ← ↑ → ↓ ⋮ ⋯ ▾ ✕ (2 KB),
which `theme::add_symbol_font` adds as the last fallback on desktop (after the system fonts) and
Android. A symbol in neither draws as an empty box: `symbols_render` (in `mod.rs`) lists every
symbol the windows use; when you add one it lacks, re-cut the font with fonttools, e.g.
`pyftsubset DejaVuSans.ttf --unicodes=U+2190-2193,U+22EE,U+22EF,U+25BE,U+2715 --layout-features=''
--no-hinting --desubroutinize --output-file=frontend/assets/DejaVuSans-Symbols.ttf` (from
[DejaVu 2.37](https://github.com/dejavu-fonts/dejavu-fonts/releases/tag/version_2_37); licence
next to it).

## Nav bar (`nav_bar.rs`)

34 px high on the desktop; a rail down the left on a touch screen (below). Left: the Turu symbol (`docs/logo/turu-symbol-small.svg`,
drawn with egui's painter at 18 px), then VibeSkua's menus in its order: **Scripts** (Script
Loader, Search Scripts, Edit Script Options, Scheduler), **Options** (Game, Application, CoreBots,
HotKeys; no Application Themes: Turu keeps Mugetsu's palette), **Tools & Helpers** (Loader, Grabber, Current Drops, Junk Items,
Loadouts Manager, Fast Travel), **Combat** (Skills, Stats, Runtime), **Diagnostics** (Logs,
Console, Spammer, Packet Logger, Interceptor), **Plugins** (Butler, Cosmetics; no View Plugins:
Turu builds its plugins in), **Army** (Accounts, Army Control, New Tab; Turu's: VibeSkua has them
in its Manager app and its tabbed host). Android has no Army menu and no Butler yet (no tabs:
`Platform::has_army`), and no HotKeys (`has_hotkeys`). Everything in the bar is one height (26, 40 on a touch screen), so the labels share
a baseline; an open menu and an open Auto or Jump are tinted the same accent.

Right: **Update** (only while a newer Turu is out: below), the game's quick actions **Grid** (two tabs or more), **Auto**, **Jump** and **Bank**
(VibeSkua's direct action, moved here from between Combat and Diagnostics because it is one; a
spinner in it while it runs, then for 4 s a note under it saying why nothing opened), a divider,
and the **script status**: one capsule with the chip (status dot, a yellow "!" while the script
waits for you, its name, how long it has run or how its last run ended; the clock yellow while
paused, without the word, so pausing doesn't change the chip's width and move the buttons left
of it under a finger; red after an error; opens the Script Loader) and, joined to it, the Script Loader's run
controls as icon cells: Start (accent; greyed, with the reason, while Auto or a Console snippet
runs), Pause or Resume (accent: what you came back for) and Stop while it runs, Force Stop while
it stops (offered after 1.5 s, as in the Script Loader). No script loaded, the chip stands alone.
The clock keeps room for "59m", so the bar doesn't shift every second.

When the menus would run into the right side (its width is measured each frame it is drawn in
full), they fold: first to **Scripts** and **More** (the other menus as its submenus), then, as
on a phone held upright, to one **Menu** with everything and Bank as its last row; the symbol is
left out and the script's name gets what room is left, up to 72 px (150 otherwise), or is left
out (it stays in the tooltip). On a touch screen (`Platform::is_touch`, Android) the nav bar is a **rail** down the left side
instead (`gui::nav_rail`): Android is held sideways, where the game is as tall as the screen
allows, so a bar above it took its height; the rail sits in the room beside the game, which now
gets the whole height. It is 64 points wide (`gui::RAIL_WIDTH`), plus a display cutout on its side
(`gui::set_rail_inset`, from the activity's insets), and `gui::left_width` is the game's and the
tool windows' left offset as `top_height` is their top (0 on the rail without tabs). Top down:
the Turu symbol, then icon-over-label items (a pill behind the icon, tinted while open or
pressed; a dot badge for Auto running or an update; a spinner in place of Bank's icon):
**Menu** (every menu as a submenu, opening beside the rail), **Auto**, **Jump**, **Bank**, **Grid**
(two tabs or more) and **Update** (while one is out). At the bottom, the script's status in one
column: a 56x48 chip (status dot over the clock, or "Finished", "Stopped", "Error", "No
script"; opens the Script Loader) with the run controls under it, 44 high. Popups open beside
their item. A short screen folds the symbol, then Update, then Bank into Menu; the fit always
counts two run controls, so starting or stopping a script doesn't move the items. In the bar
(desktop) its buttons, toggles, chip and run cells are 26 high; menu rows on a touch screen
are about 44; in the Auto and
Jump popups the buttons, drop-downs and text boxes are 48 and the drop-downs' rows about 44,
and Auto's "RUNNING" is 11 px (9 before, on every platform).

With two tabs or more, a 30 px tab strip (`tab_strip.rs`; 40 points on a touch screen,
`gui::tab_strip_height`, with a 28-point target round each tab's ×) sits above the nav bar
(`gui::top_height`): rounded tabs with a status dot (a yellow "!" badge while the tab's script
waits for you), the name and ×, the active one in the panel colour with an accent line on
top, and + at the end.

| Item | State |
|---|---|
| Script Loader, Scheduler, Game, Application, CoreBots, Loader, Grabber, Current Drops, Skills, Stats, Runtime, Notify Drop, Logs, Console, Spammer, Packet Logger (VibeSkua's "Logger" entry), Interceptor | implemented; Edit Script Options, when disabled, gives the Script Loader's reasons (Load a script first, This script declares no options, The script's options couldn't be read) |
| Bank | implemented: `bank.open` through `spawn_action`. An action, not a menu: drawn like Auto and Jump (a row of More or Menu when folded). A spinner after the menus while it runs; since `bank.open` does nothing logged out or while a map loads, the bar checks first and then says why for 4 s in red ("Bank: Not logged in", "Bank: The map is still loading", or the error), cut to the room the right side leaves |
| Auto popup | implemented, as VibeSkua's `AutoViewModel`/`ScriptAuto`: the class drop-down lists the Class items you own (re-read whenever it opens, which also clears the selection); picking one equips it and lists its saved modes (first one selected); Auto Attack and Auto Hunt use that class and mode, or the equipped class in Base when either is unset. Both start the drop pickup when it has items and the boosts when any is on, turn Infinite Range on, and turn all of that (and the skills) off when stopped. Auto Attack sets the spawn point and attacks the Manual MapIDs in priority order, else your current target, else any living monster, without leaving your cell. Auto Hunt jumps (pad Left, at most once per HuntDelay) to the earliest living Manual MapID's cell and attacks it until it dies; else hunts your current target's monster type through the map's cells; else every monster type of your cell ("A|B": cells walked from the last, one kill each, empty cells dropped, the list re-read once empty). During a counter attack (`StopAttacking`) Auto Hunt reuses the previous run's target. Turu differences: VibeSkua's several-names hunt idled (it looked for a monster named "A|B") and stopped looking once its cell list emptied. |
| Jump popup | implemented, as VibeSkua's `JumpViewModel`: picking a cell jumps at once ("Enter" uses pad Spawn, otherwise the chosen pad or Left); the cell list is refreshed when the popup or the cell drop-down opens; pads are VibeSkua's list (Spawn, Center, Left, Right, Up, Down, Top, Bottom); Current selects your cell and pad without jumping; Jump re-jumps to the selection. Jumps need a login and don't auto-correct the pad (`map.jump(cell, pad, false)`); the result shows under the buttons. Arrow-key cell stepping isn't ported. |
| Script chip | Turu's, in place of VibeSkua's "+" (which opens its tabbed tool host): the active tab's script at a glance on a rounded chip, its status dot (the yellow "!" badge while it waits for you), the file name without extension (cut with "…" past 150 px) and, muted, how long it has run ("3h 05m", the Stats clock), "Paused · 12m", "Starting...", "Stopping...", "Error", after a run "Finished 6h 12m" or "Stopped 3h 40m" (the Script Loader's last run), or "Ready"; "No script loaded" without one. Its wording and clock are the Script Loader's status card's; it redraws every second while a script runs. Clicking it opens the Script Loader |
| Update | Turu's (VibeSkua's Manager asks in a message box at launch): only while `turu_bot::updates` found a newer Turu, a quick-action item left of Grid, "Update 0.2.0-alpha.7" ("Update" folded all the way), led by an accent dot (a spinner while it downloads, green once it waits for Turu to quit, red after a failure); the tooltip says "Update available: Turu X (you have Y)". It opens the Update Turu window. Never a modal: a script may be running |
| Grid | with two tabs or more: Grid View, every tab at once in tiles (VibeSkua's tabbed host's Grid View; `frontend/src/grid.rs`); clicking a tile makes its tab the active one |

**Auto Attack / Auto Hunt** (`auto.rs`, loop in `turu_bot::auto`): a bot thread marked as a
script thread, so `bot.control.request_stop()` (Stop Automation) makes the next call fail. It runs
the steps in the Auto popup row above. Both are disabled while a script or a Console snippet runs
(or either of them runs), and Start Script is disabled while one runs.

## Windows

* **Script Loader** (`script_loader.rs`, 370x500): status card: the state in words with a
  coloured dot (No script loaded, Ready, Starting..., Running, Paused, Stopping..., Stopped with an
  error; `ScriptStatus::label()` keeps VibeSkua's "[Running]" for the control API and the
  monitor) and, while a script runs, how long (the Stats clock, "45s", "12m", "3h 05m"); the file
  name in monospace (full path on hover) or "Load Script or Search Scripts to pick one" ("Search Scripts to pick one" without file
  dialogs, as on Android, where Load Script is hidden); after a
  failure the error's first line in red, which scrolls the log to the error when clicked;
  after a run, until the next start or load of another script, how it ended and how long it ran
  ("Finished · ran 6h 12m", "Stopped · ran 3h 40m", "Stopped with an error · ran 2h 10m"; "Ended
  at 04:12" on hover; from `RunEnd`, which records the script, finished or not, the run's length
  and its end time) instead of "Ready"; while a script runs, "last log 2m ago" on the right, or
  on a line under the state when the window is too narrow (the state itself is cut to fit, whole
  on hover) (the script log's newest line; amber
  once a running script has been quiet 5 minutes). When the error's line is no longer in the log
  (cleared, or pushed out), clicking it shows the whole error in the card instead, selectable,
  with a Hide button. Beside the file name, where files open in an app, a small "Open in
  VS Code" (when `code` is on PATH or the macOS app bundle is found) or "Open in editor" (the OS
  default app): VibeSkua's Edit Script and Open VSCode in one, kept out of the setup grid (a VS
  Code that won't start says so in the script log);
  under it the run controls, 38 px high: Start Script alone across the row while nothing runs;
  while it runs, Stop Script (danger, about 60% of the width) and Pause/Resume (Resume is
  primary while paused: it's what a check-in wants then); while stopping
  handlers run, Stop Script rests (disabled; the card says "Stopping...") and Force Stop takes
  Pause's place, enabled after `FORCE_STOP_AFTER` (1.5 s, which its reason names; F10 forces only
  then too), so a double press only stops; then,
  14 px lower, the setup buttons, outlined and 28 px high (VibeSkua's grid is reordered so the
  run controls lead; on a touch screen (`Platform::is_touch`, Android) every button is 48 points
  high, the log's Clear/Copy 40, the small labels 12 (11 otherwise), and why Start or Edit Script
  Options is disabled shows as a line under its group instead of a tooltip; buttons the platform
  can't serve are hidden: Load Script and the log's Save without file dialogs, the editor button
  where files don't open in an app):
  Load Script (rfd dialog filtered to .ts/.js, starting in the repo's `scripts/` found next to the
  working directory or above the executable, else the bot's scripts folder) and Search Scripts
  (opens the window below) side by side; Edit Script Options across the row (enabled once a
  script with options is loaded);
  SCRIPT LOG with a Find box (keeps the lines containing its text, any case, with their count;
  spaces around it ignored; Escape clears it; with no match the list says
  `No lines contain "..."`; revealing the error keeps it when the error's line passes it, else
  clears it), Copy/Save/Clear, following the end, selectable text, "The script's log shows here
  when it runs" while empty ("Nothing logged yet" while a script runs);
  long lines wrap to the width, continuing under the text after the time (also in
  Diagnostics > Logs).
  The "Report Issues" footer is omitted.
* **Search Scripts** (`script_repo.rs`, 969x500; VibeSkua's Script Repo; also Scripts > Search
  Scripts; on Android the only way to get a script): the search box ("Search N scripts", a ✕
  inside clears it), then the filters: what the search looks in (All fields, Name, Tag,
  Description, File), the folder (All folders and the top folders), Pinned and Local chips, sort
  (Name or Date added) with its order in words ("A to Z", "Newest first"), "N shown" and, with a
  filter on, Clear. The header follows the room the window has (`Header`): 900 points across or
  more, search and filters share a line; narrower, the filters wrap on the next line; narrower
  than 640 or shorter than 420 (a phone either way up), the filters fold behind a "Filters · N"
  chip (N filters on) with "N found" beside it, the rows drop to two lines (name; badge and
  description; the path heads the row's menu) and, under 520 across, the bottom bar's status
  takes a line above its buttons. On a phone the window fills the screen (`ToolWindow::sheet`,
  touch and under 1000x600) and follows it when the screen turns; Enter in the search
  box loads the top hit, Cmd/Ctrl+Enter starts it (the top hit's Load and Start say so); the list (without the libraries such as CoreBots: files with no
  `export default`, which VibeSkua lists), pinned first, then the sort, then the search rank
  (VibeSkua's `SearchRank`). Each row: star when pinned, name, badges (Downloaded, Update,
  Modified, Local, Removed), path, description, tag chips, Start, Load, Queue and a "…" menu with
  Pin/Unpin, Download / Update / "Replace with the repository's", Open in VS Code and Delete...;
  Start is plain but on the row under the pointer and the top search hit (primary), and the row
  a task runs for shows a spinner. Narrower than 640 points or on a touch screen, a row keeps
  Start and the menu (which takes Load and "Add to the queue"), and buttons, menus, drop-downs and
  chips are finger-sized (48, rows about 44, chips 40); on a touch screen the menu also says what
  the row's badge means and, under a resting item, why. Start and Load download the script and
  what it imports first, then load it in the active tab; after a Start that worked the Script
  Loader opens on top; Queue downloads it the same way and adds it to the active tab's Scheduler
  playlist (disabled while the playlist runs). One task runs at a time, but a script that's
  downloaded and current with what it imports (`ScriptRepo::ready`) loads and starts at once,
  also while a task runs; one that needs a download says "Wait for Download all to finish, or
  Cancel it (this script needs a download first)". Delete asks first, naming the script (your
  own script: "the repository has no copy of it"; one the repository dropped: "this is the only
  copy"; edited: "Your edits go with it"), and moves the file to
  `scripts-backup/<time>/` in the data folder.
  Empty states "Loading scripts...", "No scripts yet" (with Refresh) and "No scripts found" (with
  Clear the filters). Bottom bar: Update all (disabled when nothing is outdated) with "N to
  update", Download all, Refresh, a "…" menu (Reset Scripts..., which asks first; Open the
  scripts folder; the same in VS Code; on a narrow screen Download all too), and on the right
  (on a narrow screen on a line above the buttons) the running task with its progress bar
  and Cancel, or the last result (red when it failed, a green dot when it worked) and "List
  updated 2h ago" (commit and fetch time on hover). Not built: "Update dates from Git" (the index
  has the dates), custom script folders.
* **Scheduler** (`scheduler.rs`, 640x460, VibeSkua `ScriptSchedulerView`, one per tab: the
  playlist is the tab's `bot.scheduler`): Start queue / Stop queue (primary / danger), the summary
  ("3 scripts", or "Started at 22:10:05 · running 2 of 3"), Load and Save (VibeSkua's playlist
  JSON, `[{"Path", "Id", "Name"}]`; the dialogs open in the data folder's `playlists/`) and
  + Add scripts (opens Search Scripts, whose Queue adds them). The table: #, SCRIPT (double-click
  to rename: Enter or leaving the box ends it, Escape undoes it), STATUS (Queued, Running green,
  Completed dimmed, Stopped amber, Failed / File Not Found / Disconnected red), TIME (the last
  run's, "00:00" until it ends) and ⋮ (also on right-click): Move up, Move down, Script options,
  Remove. Adding, moving, removing and loading are refused while the playlist runs (a line under
  the buttons says so, where VibeSkua shows a message box). Script options (refused while a script
  runs) opens the Script Options window on the entry's own options, `options/Queue_<file
  stem>_<id>.cfg` (VibeSkua `OverrideStorage`); Save writes that file and leaves the loaded
  script alone. The engine is `turu_bot::scheduler`, on a thread of its own: it loads each entry
  with its own options and starts it without the Script Options window (`start_silent`, VibeSkua
  `SilentConfig`); when the script ends in any way (it finished, threw, or Stop Script) the entry
  is Completed and the next starts 2 s later; one that fails before reaching `main` (VibeSkua: a
  compile error in `StartScript`) is Failed and the next starts 3 s later; a missing file is File
  Not Found. Only Stop queue stops the playlist (the current entry Stopped, its script stopped).
  A script that ends while logged out (auto-relogin stops it) is Disconnected and not skipped:
  the playlist waits for the login (1.5 s checks, then 3 s) and takes over the script auto-relogin
  restarted, or starts it again. A script that isn't the playlist's is stopped first. While it
  runs, the default Script Started / Farming Session Concluded webhooks are off and the playlist
  sends VibeSkua's own embeds (Script Started [i/n], Farming Session Concluded [i/n] with the
  duration, Script Error, Script Resumed, Scheduler Paused / Finished / Stopped). Closing the tab
  stops its playlist. Unlike VibeSkua, the playlist's own stop of a lingering script doesn't
  complete the first entry, and a relogin restart keeps the entry's options (VibeSkua clears
  `OverrideStorage` on every stop). Not built: the Army Scheduler (VibeSkua's tabbed host).
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
* **Game options** (`game_options.rs`, 800x720): search box, chips All/Visuals/Combat/Drops/
  Relogin/Gameplay, the five sections with the fields of `BotOptions` (switches, number boxes,
  text boxes, Relogin Server and Hunt Priority combos, a Reload Map button), each with a one-line
  description under its label (search matches it; VibeSkua's window has none). Search also
  finds a section by the start of a word of its title (3 letters or more: "network" finds
  Relogin; "&" finds nothing); the chips count what it finds per section; the section headings
  (with a count) show only under All, as VibeSkua's `ShowSectionHeaders`; a search that finds
  nothing says so, with Clear search (and Search all sections when another section has
  matches). Reload Map is enabled only in game. The descriptions
  say what Turu does: Hunt Buffer, Hunt Priority and Use Function Based Skills say Turu doesn't
  use them yet (their controls are greyed, as Application Options' "coming soon" rows: `UNUSED`),
  and Safe Timings that only scripts read it. Each section's options sit two to a row
  (VibeSkua's two columns) from 640 px wide, one per row on a phone. Edits are written to
  `bot.options` at once. Footer: a status line (unsaved changes, "Saved.", or Save's error), then
  Defaults / Revert (VibeSkua's Reset: reload `options.json`) / Save. The window remembers which
  options it changed since the last Save or Revert; what else differs from `options.json` was
  changed elsewhere (a script's changes stay after it stops, but Auto Relogin). A row that
  differs gets a dot after its label (accent: changed here; amber: elsewhere), and the status
  line says whose ("You changed 2 options", "Aggro Monsters, Skip Cutscenes and 4 more were
  changed outside this window, most likely by a script. … Save keeps them too; Revert puts the
  saved options back"). Save still writes every live option, as VibeSkua's. Defaults asks once (the
  button turns into a red Reset all, and Revert into Cancel) and keeps Relogin Try Delay and Relogin Watchdog, which
  Application Options owns; Revert and Save are enabled only while the live options differ from
  `options.json` (read again every 2 s). Number boxes have floors (1 for the tries and Walk Speed,
  1000 ms for Login and Load Timeout), and the Drops card warns when Accept All and Reject All
  Drops are both on. On a touch screen smaller than 700x760 it opens as a full-screen sheet,
  and every control (switches, number boxes, text boxes, combos and their rows, buttons) is
  40 points tall; a row too narrow for its text beside the control (under 160 points left)
  puts the control under the text. In a window under 480 points tall (a phone, which Turu holds
  sideways) the search box and chips scroll with the list instead of staying above it; the list
  leaves room for the footer as tall as it was drawn last frame. Checked on the `turu_api36`
  emulator (2026-11-06). Differences from
  VibeSkua: no Optimize Players, Upgrade/Staff Name Colour (not in `BotOptions`); the server combo
  uses VibeSkua's fallback list (plus the current value); Set FPS defaults to 0, shown as
  "Game's", which leaves the frame rate to the game's own FPS setting (VibeSkua: 30, forced at
  every login), and Headless Mode restores the rate the game had before it turned on.
  While Headless Mode is on in game, the game area shows VibeSkua's headless overlay ("Headless
  Mode Active", `show_headless_screen` in `mod.rs`) instead of the 2 FPS game: the movie isn't
  drawn and the overlay takes the game's clicks; the bar and tool windows work as usual.
  Streamer Mode (VibeSkua `ApplyStreamerMode`, `api/options.rs`) turns Hide Players on and, on
  every options pass, draws "TuruUser" / "Hidden" over the name tag, guild and target portrait (never
  `objData`), hides the gold (`mcGold`, the interface's current one, as well as VibeSkua's old
  spellings), coins, chat and the portrait's name (the game rewrites that text whenever the bars
  change, which showed the real name for a frame in combat; it never touches its visibility),
  writes a house's sign as "TuruUser's House", and writes the area title without the room number;
  off, it gives back Hide Players, Custom Name/Guild, the chat and the house sign, and writes the name and
  guild back from `objData` (VibeSkua's `world.setUserData` alone leaves the name tag as it is). Beyond VibeSkua, whose tabs keep the character's name, it hides the
  name from Turu's own UI too: the tab is "Tab 2" in the tab strip, Grid View and Army
  Control, the window title drops the name, and an Army client with it on is "Client 3" in
  Army Control (its `GET /snapshot` says so), with its map shown without the room number.
* **Application Options** (`application_options.rs`, 580x650, VibeSkua `ApplicationOptionsView`):
  search box, chips All/General/Updates/Webhooks with counts, each setting with its description.
  Changes are saved at once (`settings.json`, `AppSettings`). General: Show Username in Title
  (the window title gets " – <character>" while you play), Grandpa Mode (larger log and packet text), Client Animation Frame-rate (the
  `stage.frameRate` set when the client loads, default 24; the game's own FPS setting, or Set FPS, takes over in game; VibeSkua
  stores it and never reads it), Frame Interpolation (Turu's: Mugetsu's switch, also in the game's
  right-click menu on a computer, which a phone can't open; `Platform::set_frame_interpolation`
  applies it and saves it as `mugetsu-interpolation.txt`, in Mugetsu's folder on a computer and
  the data folder on Android; both pace it with `turu_frontend::pacing`), Relogin Try Delay and Relogin Watchdog (bot options: applied
  and saved into `options.json` on their own), Clear Flash Cache (deletes Ruffle's SharedObjects
  folder after a second click, then restart). Updates: the switches act at launch, off the UI
  thread (`turu::start_updates`; skipped by Army-launched clients, which share their launcher's
  data folder). Check for Client Updates (on: at launch and every 6 hours), Include Pre-releases
  (a pre-release build gets them anyway: every release so far is an alpha), Check Now, and under
  the section the last check (when, the newest version, or why it failed). Check for Script
  Updates fetches Search Scripts' index, and with Auto Update Scripts downloads what's missing or
  outdated (`ScriptRepo::startup`; VibeSkua stores these two and never reads them, its client
  always loads the script list); `--headless` skips them, since the script it runs is updated
  before the game starts. With both AdvanceSkill Sets switches on, `auqw/Scripts`'
  `AdvancedSkills.json` becomes the default sets and its new classes and modes are added to
  `UserAdvancedSkills.json`, keeping yours (VibeSkua `RefreshDefaultSkillsAsync`;
  `turu_bot::skills::startup_skill_sets`, also with `--headless`); an open Skills window reads
  them again. The two junk-item switches stay greyed ("Coming soon"): VibeSkua never reads them. Webhooks: the Discord URL with Test, the notify
  switches, Drop List, Health Pings and their interval, sent by `bot/src/webhooks.rs` (VibeSkua
  `DiscordWebhookService`): one thread posting embeds, two a second, retrying on 429; the health
  pings' snapshot is read on that thread. The drop alert has no screenshot (VibeSkua attaches
  one). Failures go to the Debug log. Monitor (in place of VibeSkua's Skua Nexus webhook):
  Enable Web Monitor, Allow Other Devices, Allow Control (on by default: the page can start,
  stop and relog bots), Monitor Port (applied half a second after the last change), Monitor Link (Open, Copy Link with the key after `#`, the network address with Allow
  Other Devices; Copy Key), Phone QR Code (Show / Hide: that link as a QR code, needs Allow
  Other Devices), Regenerate Key (asks twice) and Push Key (Copy Report Settings: this monitor's
  address and push key for another machine; Regenerate, asking twice), with the server's state
  below. Report (to another machine's monitor): Report To, Push Key (hidden), Machine Name
  (empty: the computer's name) and Accept Commands, with the reporter's state and a warning for
  plain `http://` across the internet.
* **Update Turu** (`client_update.rs`, 400x250, from the nav bar's Update): a card with "Turu
  <version>", "You have <version> · pre-release" and a Release notes link (the GitHub release
  page), then the state: a spinner and what it does (Downloading, Unpacking, Installing), "Turu X
  is downloaded and installs when you quit Turu." in green, or the failure in red; while a script
  runs, "A script is running. Stop it to restart now, or update when you quit Turu.". Buttons:
  **Download & restart** (primary; greyed while any tab runs a script or a download runs, the
  tooltip says why) downloads the package, checks it against `SHA256SUMS.txt`, replaces Turu and
  starts it again with the same arguments; **Update when I quit** downloads and unpacks it now
  and puts it in place when the window closes. On Android one button, **Download APK**, opens the
  APK's URL (Android installs it over the app); a release without this platform's package says
  to download it from the release page.
* **Accounts** (`accounts.rs`): add / edit form (Username, Password with Show, Display Name, Tags;
  the password goes to the system keychain off the UI thread; no title when adding, "Editing
  Hero" when editing, whose greyed username says on hover it's the account's key; Enter in any
  field saves; Show has a width of its own so the two columns line up; where nothing keeps
  passwords, `accounts::keeps_passwords()` false as on Android, the field and Show are replaced
  by "Passwords can't be saved here yet: log in by hand in the account's tab", which "Log in by
  hand"'s tooltip says too) behind "+ Add account" (open while
  editing, or with no accounts yet; it closes on Add or Cancel; adding a username that's saved
  already, in any case, is refused with "Hero is already in the list" and an "Edit it" link, where
  it used to overwrite that account's display name and tags), the list (each row: Start and a ⋯
  menu, 40 points high on a touch screen with the ⋯ menu's rows about 44, on the right and placed first, with Edit, ↑ Move up, ↓ Move down and Remove…, which
  asks in a box and says the saved password goes too; then, in the width left and cut there, the
  check box, a green dot while it runs, the name (the username on hover when it has a display
  name), the launch note, then its groups (in the accent colour) and tags (grey) as pills, as
  many as fit with "+N" for the rest; the list takes the window's height that the form, the group
  row and Launch leave, at least about four rows, where it was 200 px), a search box (name,
  username or a tag containing it, ignoring case; "No account matches…" when nothing does), a tag
  filter, Select /
  Unselect All, Add Selected to
  Group and "Select a group's accounts" (checks that group's accounts and only those; each group
  has Remove, which keeps its accounts), and Launch: Server (the same drop-down
  as Relogin Server, `widgets::server_combo`), Where (In a tab, In its own window, Without a
  window), the script (Browse / Clear) with Auto-start (disabled without a script, saying so on
  hover), Start Selected, Start All. Saved on every change.
  Where Army can't start a `turu` of its own (`Platform::launches_processes`; Android, whose
  executable is app_process), Launch shows "In a tab" and starts accounts in tabs whatever the
  saved mode; without file dialogs "Use the loaded script" (the Script Loader's) replaces Browse.
  A launch reports on the rows (not only in the bottom line, which each account overwrote): a
  spinner and "Starting…" while it waits its turn, then the green dot; "Log in by hand" (amber)
  when it started without a saved password, until it's logged in in a tab (Army can't tell when
  a client of its own logs in, so there it stays until the next launch); "Failed: reason" (red)
  until the account is launched again. Passwords aren't read only to mark the rows: the
  keychain may ask to be unlocked. Under the Start buttons, on a line of its own, the last
  Start ("Starting 3 of 10…", then "Started 8 of 10 · 2 need a login · 1 failed"). Start
  Selected and Start All ask first ("Start 12 accounts?") when more than 3 would start. Select All and Unselect All act on
  the rows the tag filter shows, and while selected accounts
  are hidden by the filter a line next to the Start buttons says so in amber ("5 selected · 2
  hidden by the filter"), as Start Selected starts them too; the search counts as part of the
  filter. The bottom status line shows failures (a save, a password, a launch) in red.
* **Army Control** (`army.rs`, 560x620): one list, CLIENTS, of this window's tabs and the
  clients Accounts launched (from the web monitor's hub, which the Army poll feeds). Each row: a
  check box, a dot, the name ("tab" or "no window" pill), where it is (long text cut off, whole
  on hover), and below it its own buttons: ▶ Start / ■ Stop (its loaded script, shown on the
  right, or "Auto Attack"/"Auto Hunt" in green while one runs), Log in / Log out, Gear, Show
  (tabs), and ⋯ (Relog; Start Auto-Attack when logged in and no script runs, Stop Auto
  Attack/Hunt while one runs; launched clients also Logs…, Restart and Close). An ended client shows Restart and Remove. The header selects All, None or
  an Army > Accounts group's rows (Group ▾), and its ⋯ has Clear ended and Close all launched
  clients. ACTIONS go to the checked rows, or to every row when none is checked (it says which,
  in yellow for everyone): Start script, Stop script, Load script…, Log in, Log out, Relog, Go
  to house, Start Auto-Attack, Stop Auto-Attack; Travel (join a map at "cell[, pad]", go to a player), Quest (accept with an optional
  reward) and Misc options fold away, closed at first. Stop script and Log out sent to
  everyone need a second click ("Log out all?") within 4 s. The last outcome shows below, with
  the earlier ones under History. VibeSkua's "Include this window's tabs" is gone: tabs are rows.
  Start script (▶ Start too) shows each client's Script Options window first, as VibeSkua's Army
  start does, unless the script sets `dontPreconfigure` or the client is headless; in a tab it
  shows when you open that tab.
  Start/Stop Auto-Attack are VibeSkua's tabbed host's "Start/Stop Auto-Attack for All"
  (`WM_SKUA_AUTO_ATTACK`), sent through the control API (`/auto/attack`, `/auto/stop`),
  with no second click (VibeSkua asks none; stopping loses nothing). A client starts Auto Attack
  as its Auto popup would, but with its equipped class in Base and no Manual MapIDs (VibeSkua's
  clients used their own popup's picks; Turu's popup belongs to the window, not to a tab).
  Rows running a script or a Console snippet, already in Auto Attack/Hunt or not logged in are
  skipped ("Alt1: Start Auto-Attack: skipped (a script is running)"). Stop Auto-Attack stops
  Auto Hunt too, and leaves scripts alone. The web monitor has them too (the bulk bar and a
  bot's detail view; Stop also on its card).
* **Gear** (`gear.rs`, 460x440, one per client, from a row's ⋯ menu): online or offline, the
  running script, "Bank as of …" (or "Bank not read yet") with Refresh; per
  slot (class, weapon, armor, helm, cape, pet, necklace, ground) what's worn and a drop-down of
  what could be (inventory first, then bank items, marked) with Equip; the last outcome
  (`EquipReport::summary`). It reads the account's `gear/<username>.json`, so it shows an
  offline account's last gear. While a script runs, Equip is disabled until
  "Stop the script first" is on (off each time the window opens); the script then stays stopped.
* **Logs** (`client_logs.rs`, one per launched client): its script and debug logs, read from
  its control API every 2 s while open, with Copy / Save / Clear.
* **Butler** (`butler.rs`, 520x560; VibeSkua ButlerV5's roster and settings windows, one per
  tab): this account and whom it follows or summons (Stop following, Release all, the last
  action's outcome); ACCOUNTS, the other online accounts read from the sync files every second,
  with chips (fighting, following me / following X / summoned, busy: script, parking, other
  server), map-room, server and class, and Follow/Stop and Summon/Release, disabled with
  V5's reasons; the online count, Summon all, Settings and Debug Log. Settings for every
  client (Enable, passive attack, leech mode, always goto, goto delay, rescue threshold, park
  location; WHILE FOLLOWING: quest bypasses, custom bypass quest IDs, unlock all quests, fake
  level 100, AntiLag while summoned / following, debug logs, auto-save logs) and for this
  account (class type, obey summons), saved on change, V5's descriptions on hover. DEBUG LOG:
  Butler's log lines (time, component, message; repeats counted), colored by level, following
  the end, with Copy, Save (to `butler_logs/`), Clear and Open folder. Following stops the
  tab's Auto Attack/Hunt. When a script starts while following, a modal asks whether to keep the
  follow suspended until the script stops (Yes, or closing it) or stop following (No); a tab
  in the back shows "!" until it's answered.
* **Cosmetics & Outfits** (`cosmetics.rs`, 900x620; VibeSkua's Cosmetics plugin, one per tab,
  the catalog shared): a quick bar card of two aligned rows (the room player list and Copy
  Player, then Copy Target Look; on the right M / F / Swap gender. Below: a character name box
  and Copy Page, which copies from its character page (Enter works too); the last action's
  outcome; Reset to Real Gear on the right), then the tab chips with the catalog's state on the
  right. Every control is 28 px high (40 on a touch screen). The Catalog, Outfits and CC tabs split into a list that
  fills and a 270 px column on the right, both as tall as the window. **Catalog**: search (an
  ID when it is a number, else the name; × clears it), sort, Check Updates, the category chips
  (Weapon adds the weapon kind), a paged list in an inset (rows: name, `#id · file`, the
  category tag; 50 to 500 a page with "1–50 of N items"; double-click equips the item) and the
  selected item's card (ID, file and link; Equip Look, favorite and Open on AQW Wiki pinned to
  the bottom). **Outfits & Inspector**: Save Current Look, the saved outfits (each row with
  Equip; double-click equips), the inspector (each slot's item and Equip; Equip Full Set and
  Delete Set side by side at the bottom). **Color Custom (CC)**: six rows of swatch (a color
  picker), name and hex box (outlined red when it isn't hex), Apply Palette, Read Avatar Colors,
  VibeSkua's eight presets on the right. **Manual SWF**: a form (file, link, slot, meta) whose
  Inject is enabled once the file and link are filled. Empty lists and cards say what to do.
  The layout follows the room the window has (`Fit`): the quick bar sits on top only at 760 px
  wide and 480 px high, else it is a **Copy** tab (its two cards side by side when there's
  room: the copy rows, then Male / Female / Swap, Reset to Real Gear and the outcome), and Reset
  and the outcome move to the tab row. Narrower than 600 px (a phone held upright) it is one
  column with short tab labels (Copy, Catalog, Outfits, Colors, SWF) and a line under them with
  the outcome or the catalog's state and Reset Gear: the catalog's search, then category and
  sort side by side, rows with Equip, "‹ 1 / N ›" and Check Updates under the list; a tapped
  item or outfit opens on a page of its own with a Back button (Escape too); CC puts the
  presets under the rows; Manual SWF puts the labels above the fields. Lower than 480 px (a
  phone held sideways) the tab labels are short, there's no separator, the category is a
  drop-down and the page arrows join the search row (Check Updates goes to the tab row), and
  the selected item's actions share one row. On a phone the window fills the screen and its
  controls are 40 px high (the main buttons 44).
* **Logs** (`logs.rs`): Script / Debug tabs, Copy/Save/Clear. In every log's header
  (`logs::log_buttons`: Script Loader, Logs, Logger, client logs) Clear stands apart from Copy and
  Save at the end and turns into "Undo clear" for 8 s, which puts the lines back before those
  logged since (`LogBuffer::take` / `restore`). **Logger** (`packet_logger.rs`):
  Start/Stop Logging turns `bot.logs.packet_logging` on or off; while it is on the engine keeps
  the outgoing packets in `bot.logs.packets` (1000 lines), search box, the "Show n / m" filter button (`packet_filters.rs`, VibeSkua `PacketFiltersButton`: a flyout
  with "SHOW PACKETS", All / None / Invert and the twelve filters as check boxes in three columns; a packet an
  unticked filter takes isn't logged from then on: `bot.logs.packet_hidden`, `turu_bot::packet_filters::logger_keeps`),
  count, Copy/Save/Clear. Options > Application > Grandpa Mode makes the log text (every `LogView`: Logs, Script
  Loader, Logger, client logs), the Interceptor's and Spammer's packets and their counts larger (VibeSkua's 15 pt for 11).
* **Console** (`console.rs`, 700x520, VibeSkua's `ConsoleView`, one per tab): header card "TS
  SNIPPET" ("runs with `bot` in scope"), a monospace editor (default `bot.log("Test");`), Clear and
  Run (Ctrl/Cmd+Enter too), Stop while it runs; OUTPUT below with Copy/Clear: "› " and the
  snippet's first line, its `bot.log` lines as they come, then its result (the value of the last
  expression, as JSON; `undefined` when there is none) in green, its error with the stack in red,
  or "Stopped". The engine side is `script::run_snippet` (`bot/src/script/mod.rs`): oxc strips the
  types (as a script, not a module, so `eval` gives the last value), QuickJS runs it with the
  "turu" module's exports (`bot`, ...) as globals, on a `spawn_action` thread marked as a script
  thread, so Stop (`script::stop_snippet`) ends a `while (true)` loop; `import` isn't available.
  Its `bot.log` lines also go to the script log. Run is disabled while a script or Auto Attack/Hunt
  runs, and Start Script (Script Loader, Search Scripts) and Auto Attack/Hunt while a snippet runs (the
  engine also refuses either way). VibeSkua shows no output, can't stop a snippet and lets it run
  beside a script.
* **Spammer** (`spammer.rs`, 620x460, VibeSkua `PacketSpammerView`, one per tab: the queue is
  the tab's `bot.spammer`): Start / Stop (primary / danger; Start needs a packet), "Every" with a
  milliseconds box (default 1000), the Send to client switch (`sendClientPacket` with `str`, or
  `json` for a packet starting with `{`; otherwise `sfc.sendString` / `sfc.sendJson`), and a green
  "Sending n of m" chip while it runs. QUEUE with its count, Load (replaces the queue), Save and
  Clear; the numbered rows (monospace, cut off with the whole packet on hover; the one being sent
  is selected), ✕ on the hovered or selected row, Delete removes the selected one. Below: the
  packet box ("%xt%zm%...  packet to add or send"), + Add and Send now (through `spawn_action`).
  The loop (`turu_bot::spammer`) runs on its own thread, reads the queue live, waits the delay
  after each packet and stops on Stop, when the queue is emptied, or when a send fails (written
  to the Debug log). Queue files are VibeSkua's: one packet per line.
* **Interceptor** (`interceptor.rs`, 760x540, VibeSkua `PacketInterceptorView`, one per tab):
  the server box ("Current server" first, then the server list) with ⟳ (fetches the list again),
  Connect / Disconnect, the Log packets switch (on by default) and a green "Connected" chip;
  "Search packets"; the packets, newest at the bottom (1000 kept): an OUT (accent), IN (green)
  or BLOCK (red) tag and the packet in monospace (struck through when blocked); a click toggles
  a packet, Shift+click adds a range, Ctrl/Cmd+C copies. Footer: "n packets · m selected" with
  ✕ to unselect, Copy (the selection, or every shown packet), Export (every packet, search
  ignored, as `[OUT] packet` lines with `\r` `\n` escaped) and Clear. The capture
  (`turu_bot::capture`, VibeSkua `CaptureProxy`) sits in the game socket itself: the app wraps
  Ruffle's navigator (`frontend/src/capture.rs`), each direction's bytes are cut into NUL-terminated
  packets, inbound `Z...` packets are inflated for the interceptors (and packed again when one
  edits them), and the interceptors (`Interceptor`, lowest priority first) edit
  `MessageInfo.content` or clear `send` to drop a packet; the log runs after them. VibeSkua's
  proxy needs a relog through `127.0.0.1:5588`; here Connect starts capturing at once, and a
  picked server only makes it relog there. Disconnect stops capturing and leaves the game
  connected (VibeSkua's drops the connection). Beside the search, the "Show n / m" button (as the Logger's) with
  the categories Combat ... Misc and Other: a packet shows when the search finds it and any ticked category
  takes one of the commands it carries (`turu_bot::packet_filters::commands`: `%xt%` packets and a `cmd`
  packet's sub-command, every `"cmd"` of a JSON packet, an XML packet's `action`). Long packets aren't cut: the
  list scrolls sideways.
* **Stats** (`stats.rs`, 480 wide): script clock (H:MM:SS, keeps its last value after the script
  ends), the Advanced switch (`statsAdvancedMode`), Reset; KILLS, DEATHS, PICKUPS, ACCEPTED,
  COMPLETED, RE-LOGINS tiles. Advanced (VibeSkua's 760 high, a full-screen sheet on a phone):
  GAMEPLAY with the session clock and the unit selector (`/hr` `/min` `/sec`, `statsRateUnit`),
  Level and Rank (to rank 10, 302,500 CP) bars with their rate and time left, then Net gold,
  Items, Kills, Deaths (one column when narrow). The rates are `turu_bot::stats::Gameplay`,
  sampled once a second by the bot's background thread while in game, script or not; a
  disconnect stops the clock and rebaselines. RATE HISTORY: Save, Export (CSV; Share where there
  are no file dialogs), Clear (asks), and a table sortable by its headers, kept in
  `StatsRateHistory.json` in the data folder and shared by every client (written under a lock,
  re-read every 2 s). Not built: resizing the table's columns, the STORAGE section.
* **Runtime** (`runtime.rs`, 955x596, VibeSkua `RuntimeHelpersView`): two columns. DROPS HELPER:
  the pickup list (`bot.drops`: names, then IDs), multi-select with Remove Selected; "Drop Name
  (e.g. Unidentified 13)" with Add Drop (`|`-separated, a number is an item ID; Enter adds too);
  Interval (ms) (`drops.interval`, waited on top of the loop's 1 s tick), Reject Else, Accept AC
  Drops (the game option); Pick Drops starts or stops the pickup loop (accent while it runs).
  Notifications opens Notify Drop (below). QUESTS HELPER: the registered quests
  ("1234 (Reward: 99)"), Remove Selected, "Quest ID" (several with `,` `|` or spaces) and "Reward
  ID (opt)" with Add Quest (`bot.quests.register`, which also starts the pickup loop). BOOSTS
  HELPER: Search Inventory / Search Bank (`boosts.setAllIds` through `spawn_action`), the four
  boosts with their switch and "ID: n", Use Boosts starting or stopping the timer. The lists
  and switches are the bot's own state, so a script's changes show at once; what you set here
  lasts until the script ends (its end stops the loops, turns the boosts off and unregisters the
  quests, as before).
* **Notify Drop** (`notify_drop.rs`, 534x337, VibeSkua `NotifyDropView`, from Runtime's
  Notifications): "Item name (separate several with |)" with Add (Enter adds too; names already
  listed are skipped); WATCHING with its count and Clear all; the list (click picks several rows,
  Shift+click a range; each row's ✕, shown on hover or when picked and always on a touch screen,
  Delete, or the right-click Remove Selected / Clear All removes; empty, it says what the window
  does); "Beep [1-25] times every [10-2000 ms]" (5 and 200 to start) and Test beep, under the
  list (from 720 px wide, in the WATCHING row, so a phone on its side keeps three rows of list;
  under 440 px, Test beep takes a row of its own). A listed item that drops (`itemDropped`, and again
  `itemAdded` when it reaches the inventory, as VibeSkua's `ItemDroppedMessage`; exact,
  case-sensitive names) plays the beeps on a thread of their own: `Platform::beep`, .NET's
  `Console.Beep()` (800 Hz, 200 ms) on a sound stream apart from the game's (cpal's default output
  on the desktop, AAudio on Android), so it sounds while the game is muted. The list is the tab's
  and isn't saved (VibeSkua's neither): closing the window keeps it, closing the tab or Turu
  drops it. On a touch screen it opens at 560x480 (its controls and rows are finger-sized), and
  with a window under 560x400 it fills the screen.
* **Skills** (`skills.rs`, "Advanced Skills", 700x530, VibeSkua `AdvancedSkillsView`), two tabs.
  Create / Edit Skill: SKILL TIMEOUT (0-5000 ms); SKILLS SEQUENCE, each skill as VibeSkua's line
  ("4 - [Health > 50%] - [Skip if not available]"), with ↑ ↓, Edit (the Edit Rules modal:
  the same rules, Confirm / Cancel; double-clicking a skill opens it too) and ✕; Class Name;
  USE MODE (Wait for Cooldown / Use if available, Reset on Target Change); USE RULES (Enable
  Rule, Multi Aura, Reset; Health, Mana and Party HP with ≥/≤ and %/#; one aura, or with Multi
  Aura AND/OR, Add Aura and a row per aura; Skip if not available; Wait (ms)), then the 0-5
  buttons, which add a skill with those rules; the mode drop-down (`CLASS_USE_MODES`) and Save
  Skill, which replaces that class's set in that mode or adds it, and writes
  `UserAdvancedSkills.json` (classes and modes kept in file order). Saved Skills: Load Classes,
  the class and mode drop-downs, Open (that set into the editor); every set as VibeSkua's
  `SaveString` ("Base = ArchMage = 1 | 2 | 3 | 4 = Use if Available"), double-click to edit;
  Edit Selected, Remove Selected (saves at once), Refresh, Reset Sets (the shipped sets over
  yours; Turu asks with a second click within 4 s, VibeSkua doesn't). The last save, removal or
  reset shows below. The model and the file code are in the bot crate (`skill_editor.rs`,
  `skills::advanced_skills_json`), tested in `bot/tests/skill_sets.rs`. Differences: Save
  Skill needs a class name; a flat (#) value isn't capped at 100; a multi-aura `<` check after a
  `>` one stays `<`, and a skip after `TARGET` survives a reload (VibeSkua loses both). A
  running script keeps the set it loaded until it loads one again.
* **CoreBots Options** (`core_bots.rs`, 800x608, VibeSkua `CoreBotsView`): header with the
  character's name and its save state ("Saved", "Unsaved changes · saved when you close this
  window" in amber against what was last read or saved, "CoreBots' defaults · not saved yet"
  without a file, "Not logged in · log in, then press Refresh from game"); buttons named by what they do,
  VibeSkua's names in their tooltips: Revert to saved (VibeSkua's Load; enabled with unsaved
  changes, and asks first), Refresh from game (Load character: reads the classes and gear
  again and keeps the picks, which also stands in for VibeSkua's Reload Classes, which cleared
  every role's pick, and Refresh Inventory; for a character the window hasn't loaded, as after logging in or
  switching, it loads that character's file, saving the last one's changes first) and Save (it
  shows "Saved" next to the buttons for 3 s; VibeSkua's "Save Successful!" box is gone, errors
  keep theirs; after a switch of character it loads the new one's options rather than writing
  the last one's values over them). The name and state get the width the buttons leave, cut
  with "…" past it (the whole state on hover); with less than 300 px left for them (a small
  screen, a phone) the buttons go on a row of their own under them. Tabs Loadout / Options / Other, read-only while logged out (no character
  to save to, so changes would be lost on close). Loadout: one table, a row per role (Solo, Farm,
  Dodge, Boss), each with its coloured dot, its name and, muted under it, what it's for (Single
  target, AoE farming, Survivability, Heavy bosses: VibeSkua's coloured badges, as plain text),
  then its class (the enhanced classes you own plus `[Current]`; "Not set" when empty), its skill
  preset (VibeSkua's Skill Rule / Preset) and a Dedicated gear switch (VibeSkua's "Use Dedicated
  Equipment Loadout"), under column names. Picking `[Current]` while the equipped class has no
  skill set offers only `Base` and gives the class VibeSkua's default set (`1 | 2 | 3 | 4`, Base,
  use if available, 100 ms) in `UserAdvancedSkills.json` at once (`EnsureSkillEntryExists`; a
  saved `[Current]` pick does the same when the window reads it). While the switch is on, the role's six gear slots
  (VibeSkua's "Detailed Equipment Slots") open under its row, three to a line, each with a "None"
  entry to clear it (VibeSkua lacks one); while it's off, a muted line says how many picks it
  keeps. Below 600 px (a phone) each row stacks: name and switch, then class and preset, then the
  gear two to a line. The class column gets the width the others leave, so long names are cut
  off rather than widening the window. The card bars' own refresh buttons are gone: Refresh from
  game in the header reads both lists. Options: the 17 CoreBots settings, searchable, grouped
  by what they're about (Rooms, Timing, Quests, Gear and inventory, While the bot runs, When the
  bot stops; the file keeps VibeSkua's order), each group one surface of rows split by hairlines.
  A row holds the label, its whole description wrapped under it, and its control on the right;
  a click anywhere on a switch's row flips it. Every option has a description (VibeSkua shows
  none: Turu's come from CoreBots' own comments on the options in `CoreBots.cs`, or from what
  CoreBots and its farms do with them), the delays end in "ms", and a number or text that isn't
  the default gets a Reset beside it ("Back to the default, 700 ms"). Other: Boosters (read by
  CoreFarms.toggleBoost, which runs `bot.boosts`), Nation Farms, PvP, Bot Creators, as the same
  grouped rows, searchable too (the PvP and Story Test Mode ones reworded: VibeSkua's have
  typos). Opening the window
  loads `options/CBO_Storage(<username>).txt` (cached per character), closing it saves; Save asks
  before saving public rooms (a save without the answer keeps private rooms, and the window
  shows them on again). Game reads
  (username, equipped class, inventory) run through `spawn_action`; VibeSkua's message boxes are
  egui modals. Not ported: adding a `[Current]` class with no skill set to the skill file
  (`EnsureSkillEntryExists`). Unlike VibeSkua, the drop-downs list your classes and gear as soon
  as the window knows who you are, even without a saved file.
* **Loader** (`loader.rs`, 720x520, VibeSkua `LoaderView`, one per tab): Shop / Quests chips, the
  IDs box ("IDs, e.g. 2566 or 101, 102"; Load and Enter are enabled for digits, commas and
  spaces only; Shop loads a single ID with `shops.load`, Quests shows them with `quests.load`).
  QUEST CACHE with its count and "Search by name" (name or ID); the cached quests (name, ID on
  the right), where a click picks or unpicks a row and Shift+click picks a range. The card below:
  "Update cache" with + Add missing (loads after the last cached ID), Rebuild all (from 1,
  replacing the cache) and Download (the community cache, below), or while it runs the progress ("Loading Quests 30-59...") and Cancel;
  then the range box ("Range, e.g. 2566, 2599") and Update range. A bad range, a failure, or
  updating while logged out (which only reads the file, as VibeSkua) shows a yellow note there
  for a few seconds; a download's result ("Downloaded 10345 quests.", "The quest cache is up to
  date.") a green one. Footer: "None selected" / "N selected", Copy ▾ (Copy IDs "1,2", Copy names,
  Copy as `// Name | ID` lines), Fake complete (the row clicked last: `quests.updateQuest`, a
  client-side `updateQuest` packet) and Load selected. The engine is `turu_bot::quest_cache`:
  `QuestData.json` in the data folder, VibeSkua's format field for field (a VibeSkua file can be
  copied over; Turu ships none), read when the window opens (downloaded first when there is no
  file) (IDs and names only) and updated
  on a `spawn_action` thread: 29 quests a request (`world.questTree` cleared, `showQuests`, up to
  10 s for them, 1.5 s between requests), stopping at the first request that loads nothing;
  quests loaded again replace their cached copy; a cancelled Rebuild all keeps what it read plus
  the cached quests after it. Download (VibeSkua `UpdateQuestDataFile`, which VibeSkua runs
  with its updates) fetches `auqw/Scripts`' `QuestData.json` (Skua branch; `TURU_QUEST_DATA_URL`
  overrides it) gzipped, about 1 MB, and merges it in: a downloaded quest replaces the local
  copy, quests only the local file has stay. Its ETag is kept in `QuestData.json.etag`, so an
  unchanged file is a 304; a body that isn't a quest list leaves the cache alone. Unlike
  VibeSkua, the list keeps showing while the cache updates.
* **Grabber** (`grabber.rs`, 800x560, VibeSkua `GrabberView`, one per tab, each list keeping its
  rows, search and selection): a 164 px sidebar of boxed entries under SHOPS (Shop Items, Shop
  IDs), QUESTS (Quests, GetMap Item IDs),
  ITEMS (Inventory, House Inventory, Temp Inventory, Bank Items) and MONSTERS (Cell Monsters,
  Map Monsters). Each list: ⟳ Grab (reads it through `spawn_action`), Search (the row's summary
  contains it), Multi-select (not for Shop IDs and Temp Inventory); ITEMS with its count, the
  status ("Working...", "Finished.", the action's progress) and Cancel while busy, the selected
  count, Unselect, Copy (VibeSkua's one-line summaries under "=== Title (n items) ===", of the
  selection or of every shown row) and Export (every field, nested lists expanded, to a .txt
  through the save dialog); the rows (ID column, VibeSkua's summary such as "Blade | Cost: 25,000
  Gold [Member]"); DETAILS, the picked row's fields as scripts see them (`turu.ts` names; VibeSkua
  shows C# property names). The actions, right-aligned: Shop Items Buy (one item asks the
  quantity; AC items that cost AC are refused; not enough gold shows VibeSkua's box), Shop IDs
  Load Shop (the shops loaded in this client, from their `loadShop` packets; a shop loaded again
  replaces its row, VibeSkua lists it twice), Quests Open / Accept / Register / Fake Complete
  (exactly one) / Unregister All, GetMap Item IDs Open / Accept (the items' quests) / Get Map
  Item (asks the quantity, picks each item up that many times), Inventory Equip / Sell / Sell All (one item, not equipped; Sell
  asks the quantity) / To Bank, House To Bank, Bank To Inventory, Cell Monsters Kill, Map
  Monsters Kill / Teleport To. Kill is disabled while a script, a Console snippet or Auto Attack/Hunt
  runs. The readers, texts, checks and actions are `turu_bot::grabber`. GetMap Item IDs
  (`turu_bot::map_items`, VibeSkua `FindMapItems`) downloads the current map's SWF from
  `game.aq.com/game/gamefiles/maps/<strMapFileName>` and reads its AS3 bytecode
  (`frontend/src/map_items.rs`, Ruffle's `swf` crate; VibeSkua decompiles it with FFDec and searches
  the text): in each function, a constant set as `mapItem` / `itemdrop` on an object, its quest
  the `intQuest` / `questnum` set on the same object; and a constant passed to `getMapItem`, its
  quest the `isQuestInProgress` check before it. Rows read "[1465] Map Item (Quest: 2365)"; an
  item the map ties to no quest shows Quest 0 (VibeSkua drops it). The result is kept per map
  file in `map-items.json` (map files are versioned by name), so a second Grab doesn't download.
* **Current Drops** (`current_drops.rs`): search, AC count, AC only, checkable list, Pick up all
  (`drops.pickupAll`) and Pick up selected (`drops.pickup` with the item IDs), through `spawn_action`.
* **Junk Items** (`junk_items.rs`, 550x500, VibeSkua `JunkItemsView`): search (ID, name or
  category) and ⟳; a select-all check box ("n selected", partly checked when only some shown rows
  are), Mark as junk and Unmark; the inventory and bank items by name, each row with a check box
  (a click on the row toggles it), the ID, the name (category on hover) and a red "Junk" chip;
  Skip sell warning, Unmark all, Sell selected (marks them as junk, then sells) and Sell all junk
  (n). Opening the window refreshes it, loading the bank first if needed; refresh and selling run
  through `spawn_action` (`turu_bot::junk`), the list is saved to `JunkItems.json` at each change.
  Selling asks first (VibeSkua's texts) unless Skip sell warning is on, which asks once itself and
  is saved in `settings.json` (`junkSkipSellWarning`). Only unequipped, unfavourited inventory
  items are sold, the whole stack each.
* **Loadouts Manager** (`loadouts.rs`, 600x500, VibeSkua `LoadoutsView`; engine
  `turu_bot::loadouts`): the logged-in character's gear sets, in VibeSkua's
  `Loadouts/<username>.json` format (Turu's `loadouts/<username>.json`). Wide (520 px or more):
  VibeSkua's two panes. Left, SAVED LOADOUTS with the count and ⟳ (reads the file and the worn gear
  again), the list and "+ Save current gear". The list's first row is "Currently equipped"
  (VibeSkua's Show current gear, always there and live from the gear cache, `turu_bot::gear`);
  each saved row shows its name and class, and "Worn" (green) when every item of it is worn.
  Right, the picked loadout: its name field beside Replace with worn (renames on Enter or when the
  field loses focus; a blank name, "Currently Equipped" or another loadout's name is refused and
  the old one kept), then the seven slots (Class, Armor, Weapon, Helm, Cape, Pet, Amulet): the
  item ("—" for none), the saved enhancement as a chip (amber when the worn item has another one,
  which the tooltip names) and where the item is from the cache: Worn (green), In bag, In bank (with
  the bank's age on hover), Not found (amber: in neither, as last read) or Not in bag (amber: the
  bank wasn't read yet). The actions sit at the bottom: Delete (asks first) on the left, Equip
  (primary) on the right, and Enhance beside it when enhancements differ, under a line that says
  how many. Narrow (a phone held upright): the list and the loadout are two pages, the loadout's
  with ‹ (or Escape) back to the list; on a touch screen smaller than 620x520 the window is a
  full-screen sheet. In a short window (a phone held sideways) the slots scroll so the actions
  stay in view. Save current gear (and Save as loadout on the live row) reads the worn gear on a
  bot thread, names the loadout after its class ("Legion Revenant 2" when taken), picks it and
  selects its name for typing over, instead of VibeSkua's name dialog. Equip runs
  `loadouts::equip` (`gear::equip`: from the bank when needed) and says how it went in the window
  (green "Equipped Farm", amber "Farm partly equipped. Not put on: ..."), not in a message box.
  While a script runs, an amber line says equipping stops it and Equip reads "Stop script &
  equip": the script is stopped first and stays stopped (VibeSkua equips under it; CoreBots would
  put its gear back). Enhance is VibeSkua's `LoadoutEnhancerTemp`, its own action rather than part
  of Equip: CoreAdvanced's `enhanceItem` for each worn item of the loadout whose enhancement
  differs, then back to the map it started on, run as a script (`loadouts/LoadoutEnhancerTemp.ts`,
  importing the scripts folder's `core/`; the Script Loader shows it and Stop stops it; the script
  loaded before is loaded again after). Disabled while a script runs, as VibeSkua refuses it then.
  Not logged in, the list says to log in and the actions are disabled. Checked by eye from
  offscreen renders with made-up gear (pointer and touch: 1100x700, 390x800 both pages, 800x390);
  not yet in game or on the emulator.
* **Fast Travel** (`fast_travel.rs`, 600x470, VibeSkua `FastTravelView`): VibeSkua's controls,
  laid out for any window size. A search box (name, map or cell) with VibeSkua's private room
  switch and room number (111111; its own row below 440 px); DESTINATIONS with the count of what
  the search shows, how the last travel went (muted "Traveling to Nulgath...", green "At
  Nulgath", amber "Didn't reach ...", red "Not logged in" or the
  error; whole on hover) and Clear (VibeSkua's question first). The destinations are tiles: a
  navigation arrow (a spinner while travelling), the name and, in monospace, map · cell · pad,
  cut with "…" (whole on hover). As many columns as 190 px tiles fit, at most three (one on a
  phone); a click, or Enter when focused, travels; each tile's ⋮ menu has Edit and Remove
  (VibeSkua shows those buttons only on hover, which a touch screen lacks). Under them the form:
  Name and Map, then Cell, a Pad drop-down (VibeSkua's pads), Get current (the map, cell and pad
  you are in) and Add (enabled once every field is filled; Enter in a field adds too). Edit
  fills the same form instead of VibeSkua's dialog: the heading says "EDITING NAME" in the
  accent colour, the tile gets an accent edge, and Get current/Add become Cancel/Save. Below
  440 px the form takes four rows. Every change is saved, so VibeSkua's Save button is gone.
  The list is `FastTravels.json` in the data folder (VibeSkua's three tercessuinotlim
  destinations when there's none; a cleared list stays empty, where VibeSkua refills it at the
  next launch), read again when the window opens; a file that can't be read is kept as
  `FastTravels.json.bad`. A travel (`turu_bot::api::map::fast_travel`, VibeSkua
  `MapService.Travel`, through `spawn_action`) joins `map` (`map-<room>` with a private room)
  at the cell and pad; tercessuinotlim is reached by jumping to `m22` (Left) first. A destination
  in the map you are in (any room) is a jump to its cell and pad (Turu: VibeSkua does that only
  in tercessuinotlim; elsewhere it stays where you are). On a touch screen with a window under
  620x520 the window fills the screen; its controls, tile menus and rows are finger-sized.

* **HotKeys** (Options > HotKeys; `hotkeys_window.rs`, model and dispatch in `hotkeys.rs`,
  VibeSkua `HotKeysView`): VibeSkua's whole catalog of bindable actions (its `_actionList`, 76:
  script and windows, Auto, the Game options it toggles, Rest, Jump To Home, Logout, Reject All
  Drops, 37 AQW Lite settings, Army broadcasts), with its titles and descriptions. Wide (600 px
  or more): YOUR HOTKEYS (about 58%) beside ADD AN ACTION. Narrow: two pages, the catalog
  behind "+ Add an action" with ← (or Escape) back. YOUR HOTKEYS: a Filter box (title,
  description or key), each row's title and description (cut, whole on hover), its key as a
  keycap ("Set key" when it has none) and ✕ to remove it (a note says what went, with Undo).
  Clicking a keycap waits for a key: the keycap reads "Press a key…" with a slow accent pulse,
  and the next press (with Ctrl, Shift or Alt held) is the key; Escape cancels, Backspace or
  Delete leaves it without a key. Escape, the clipboard keys and modifiers alone can't be bound.
  A key another action holds moves to this one, and the note says from which (VibeSkua's
  dialog refused it instead). A key that types (a letter, digit, punctuation or Space without
  Ctrl or Alt) gets an amber dot and a line saying it fires in the game's chat too. Restore
  defaults (asks for a second click) brings back VibeSkua's seven. ADD AN ACTION: a search box
  (every word in the title, description or VibeSkua name), group chips (All, Script, Combat,
  Display, Player, Game UI, Army) and the actions without a key, under group headings when
  neither narrows them; clicking one adds it and waits for its key. Every change is saved to
  `HotKeys.json` in the data folder (VibeSkua's `"Action|Gesture"` strings, e.g.
  `"ToggleScript|Ctrl+Shift+F10"`, as a JSON array; VibeSkua's key names such as `D1` and
  `OemMinus` are read too). With no file, VibeSkua's defaults: F10 Start Script, F9 Load Script,
  F2 Open Bank, F3 Open Console, F4 Auto Attack, F5 Auto Hunt, F6 Lag Killer. Ctrl is Cmd on
  macOS (shown as "Cmd"). Checked by eye from offscreen renders (1100x720 wide, 520x760 both
  pages, 640x420); not yet in the running app.

Hotkeys run (`hotkeys::run_pressed`, before the windows each frame) for the active tab while
Turu's window has focus (VibeSkua's Windows-only global keyboard hook isn't ported). The
modifiers must match exactly; a key that types is ignored while a Turu text box has focus;
the press is taken from egui, and the game gets it too, as it does Ruffle's own shortcuts.
What an action did, or why it did nothing, goes to the script log as `[HotKeys] ...`.
Start Script follows Start Script's rules (a second press while stopping is Force Stop); Load
Script opens the Script Loader and its dialog; Open Bank is the bar's Bank (its note under
the bar); Auto Attack/Hunt use the Auto popup's class and mode, or stop the running one, and
don't start while a script or snippet runs; Game options are flipped in memory (not saved, as
VibeSkua); Lite settings are read and flipped in the game on a bot thread; Army actions go to
every tab and every account Army started (`army::broadcast`), Army toggles flip here and send
the new value; Load Script to All opens Army Control. Tooltips show the current key (Start,
Stop, Force Stop, Load Script, Bank). VibeSkua has no action for pausing, so Turu has none.
Android (`Platform::has_hotkeys` false: its key events carry no function keys or modifiers to
the UI, and phones rarely have a keyboard) hides the menu entry and runs no hotkeys.

## Adding a window

Add `foo.rs` with a struct and `show(&mut self, ctx, &mut bool)`, build the window with
`super::window(ctx, "Title", [w, h], slot)` (lists that fill the window take
`ui.available_height()` minus the footer, with a small floor), add it to the
`windows_fit_small_screens` test in `mod.rs` when it needs no file picker, add a flag to `Windows`, a field and a call in
`TuruUi::show_windows`, and enable the entry in `nav_bar.rs`.
