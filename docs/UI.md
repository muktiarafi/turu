# Turu UI

Turu uses **VibeSkua's layout and flow** (see `VIBESKUA_SPEC.md`, section 5) in **Mugetsu's
look** (its right-click panel: dark panel, purple accent, corner radius 12, soft shadow).
That is the egui UI, drawn by every tab's client. Native shells follow their platform instead:
"Native shells" below records, per feature, where each puts it (`docs/NATIVE_UI_PROPOSAL.md`).
Code: `frontend/src/gui/turu/`. Integration points: `app/src/gui.rs` (`RuffleGui::update`
draws the Turu nav bar instead of Ruffle's menu bar and then the tool windows when
`mugetsu::plays_aqw()`), `gui::MENU_HEIGHT` (34), `mugetsu::window_title()` ("Turu <version>").

Rules: UI code runs on the winit thread and only reads cached bot state (`bot.state`,
`bot.logs`, `bot.options`, `bot.scripts`) with short locks. Anything that touches the game
goes through `crate::turu::spawn_action`, which runs on a bot thread. File dialogs use the
existing `FilePicker` (rfd, async) and never block the UI.

Tab goes to the game, as in Ruffle (`app/src/gui/controller.rs`, `handle_event`), except while
a Turu widget has focus (a text box clicked, or Tab got there): then egui has it, and Tab moves
to the next widget (boxes, drop-downs, buttons), Shift+Tab to the one before. A click elsewhere,
the game included, drops the focus. Boxes with `lock_focus` (the Console's) keep it.

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
Loadouts Manager, Fast Travel), **Combat** (Skills, Stats, Runtime), **Diagnostics** (Notification Center, Logs,
Console, Spammer, Packet Logger, Interceptor), **Plugins** (Butler, Cosmetics; no View Plugins:
Turu builds its plugins in), **Army** (Accounts, Army Control, Army Queue, New Tab; Turu's: VibeSkua has them
in its Manager app and its tabbed host). Android has them too (its tabs run in one engine
thread; Butler and Army Control are full-screen sheets with finger-sized buttons there, and the
tab strip's tabs are taller, with "×" on the active one only), but not HotKeys (`Platform::has_hotkeys`). Everything in the bar is one height (26, 40 on a touch screen), so the labels share
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
script"; opens the Script Loader, or with favourite or recent scripts a list beside it:
Script Loader, then Favourites and Recent, a tap loading and starting one; `quick-scripts.json`,
the Script Loader's ★ beside the name) with the run controls under it, 44 high. The symbol and
Menu > Hide Side Bar hide the rail (a `›` tab at the left edge brings it back), and it stays
hidden across restarts (`sideBarHidden`). Popups open beside
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
* **Game options** (`game_options.rs`, 612x720): search box, chips All/Visuals/Combat/Drops/
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
  and Safe Timings that only scripts read it. Options sit one per row, as in Application
  Options (`widgets::setting_row`, shared by both): label and description on the left, the
  control at the right edge, centred on them; when the text would be narrower than 150 px beside
  it (a phone, Set FPS), the control goes under the text, still at the right. Edits are written to
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
  Headless Mode is every tab's at once, as VibeSkua's tabbed host sends it to every tab: a tab
  that turns it on or off (Game Options, a HotKey, a script, Army Control, the control API) turns
  it on or off on all, and a new tab gets it as the others have it
  (`turu::start_headless_mode_sync`). The other tabs' Game Options mark it "Changed outside".
  The bar and the windows keep their own pace at its 2 FPS, as VibeSkua's separate window does:
  the desktop's event loop also wakes for egui's next repaint (`GuiController::next_repaint_at`),
  and Android's screen draws the UI alone between the game's frames, after a touch or when egui
  asks, at most 30 times a second (`Screen::next_ui_draw`, `engine::Step::Event`); neither renders
  the game under the cover.
  Streamer Mode (VibeSkua `ApplyStreamerMode`) turns Hide Players on and turu.swf's
  `StreamerMode` module (`host/as3/turu/host/Streamer.as`; one call when it changes, where VibeSkua
  rewrites everything every 500 ms) checks on every frame and writes only what the game changed back.
  It draws "TuruUser" / "Hidden" over the name tag and guild, and "TuruUser" over the
  target portrait only while it names you (VibeSkua writes "Hidden" there over every monster and
  player; never `objData`), hides the gold (`mcGold`, the interface's current one, as well as VibeSkua's old
  spellings), coins, the chat's log (not its entry field `te`, as VibeSkua does: Ruffle drops the
  focus of a field made invisible, which took it from you as you typed and closed Android's
  keyboard) and the portrait's name (the game rewrites that text whenever the bars
  change, which showed the real name for a frame in combat; it never touches its visibility),
  writes a house's sign as "TuruUser's House", and writes the area title without the room number;
  off, it gives back Hide Players, Custom Name/Guild, the chat and the house sign, and writes the name and
  guild back from `objData` (VibeSkua's `world.setUserData` alone leaves the name tag as it is). Beyond VibeSkua, whose tabs keep the character's name, it hides the
  name from Turu's own UI too: the tab is "Tab 2" in the tab strip, Grid View and Army
  Control, the window title drops the name, and an Army client with it on is "Client 3" in
  Army Control (its `GET /snapshot` says so), with its map shown without the room number. A
  client of the shell numbers itself by its place in the shell's tabs (it runs one tab), and
  tells the shell it is on (`ToShell::Status`'s `streamer`), which keeps it in the tab's record
  (`slots::SlotRecord`): Android's shell names a tab "Tab n" while it starts again too, and while
  any open tab has it on, a tab yet to report, and every account in Accounts and New tab
  ("Account 3", no username; the edit form keeps the real ones, so saving keeps them). Loadouts
  says "Your loadouts" and "What you wear now", and Cosmetics lists the room's players as
  "Player 1", "Player 2"... (Copy still goes by the name behind each) and names nobody in its
  outcome. Butler names this account by its tab's label and the others as Accounts does
  ("Account 2"; "Alt 1" for one not in the list) in its header, rows, reasons, outcomes and log,
  shows the room without its number and offers no recent Send to player names
  (`models::butler::Masks`, worked out with the roster, once a second). CoreBots names the
  character by its tab. The account edit form (both shells) and the desktop's Accounts window
  draw the username and display name as a password field is, with Show; the desktop's rows say
  "Account 3". At the login screen, where nothing is in game yet, the engine switches the module
  alone (`api::options::apply_out_of_game`), and it draws the login screen's typing fields, the
  remembered username's, as passwords (`displayAsPassword`; the text stays, so logging in works),
  giving them back when it goes off.
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
  (a pre-release build gets them anyway: every release so far is a pre-release), Check Now, and under
  the section the last check (when, the newest version, or why it failed). A nightly build greys
  the three with the reason ("This is a nightly build: it doesn't update. ...") in their tooltips
  and under the section. Check for Script
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
  and puts it in place when the window closes. On Android one button, **Download & install**
  (greyed while a script runs: installing stops the app), downloads the APK, checks it against
  `SHA256SUMS.txt` and hands it to Android's installer, which installs it over the app (asking
  first only before Android 12); the user opens the new version (cancelling Android's dialog
  brings the button back). The
  note under it: "Checked against SHA256SUMS.txt; your settings and scripts stay. The first time,
  Android asks you to allow installs from Turu." A
  release without this platform's package says to download it from the release page; an APK
  without sums gets **Download APK**, which opens its URL.
* **Accounts** (`accounts.rs`): add / edit form (Username, Password with Show, Display Name, Tags;
  the password goes to the system keychain off the UI thread, on both Android apps to a key in
  the Android Keystore (`android/src/storage.rs`, `air/ane/rust/src/passwords.rs`); no title
  when adding, "Editing
  Hero" when editing, whose greyed username says on hover it's the account's key; Enter in any
  field saves; Show has a width of its own so the two columns line up; where nothing keeps
  passwords, `accounts::keeps_passwords()` false (no store installed), the field and Show are replaced
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
  has Remove, which keeps its accounts), and Launch: Server (each server as
  VibeSkua's `ServerStatusInline`, "Twilly [M] - 42ms - 120/1000": the ping is the game's
  SmartFox version check answered, median of three, "No response" or "Offline", measured when
  the list is read and again a minute later; without the server list, the names alone), Where (In a tab, In its own window, Without a
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
  to house, Start Auto-Attack, Stop Auto-Attack; Travel (open: Map Travel…, Jump to Player…,
  Change Server…), Quest (accept with an optional reward) and Misc options fold away, closed at
  first. Travel is VibeSkua's tabbed host's travel menu (`army_travel.rs`): **Map Travel**
  (520x380; a sheet on a phone) has three sections, each a heading with its small buttons on
  the right. **Go to** (Turu's, in place of VibeSkua's "Jump to Map / Cell" box): Map and Room
  (digits only; empty for any room), then Cell (Enter when empty), the Pad drop-down (Spawn) and
  Jump, which Enter in a box also does; `map,room,cell,pad` pasted into Map fills every box.
  They start with the last jump; Use current fills in where the selected client is. A line
  under them says what Jump does ("Sends 3 clients to yulgar-12345 - Enter, Spawn"). Narrower
  than 320 points, Jump gets a row of its own. **Recent destinations** (the maps and rooms the
  clients actually visited, read every second while Turu runs, the newest first, at most 15;
  Clear) and **Saved presets** ("Name — map-room - cell, pad"; New…, Edit, Delete): a
  drop-down with its Jump beside it. Every Jump is off while no client would get it.
  Create/Edit Map Preset asks Preset name, Map, Room number (optional), Cell, Pad, with Use
  Current Client Location (the only checked row, else the active tab); a new preset starts with
  Go to's destination when a map is typed there, else where that client is. Jump to Player…
  lists the clients' logged-in players in a ▾ next to a box that takes any name; Change
  Server… loads the online servers (Artix first picked) and relogs the clients there
  (`/relogin?server=`). The last jump, player, history and presets are saved in
  `army-control-jump.json` with VibeSkua's field names (and Turu's `Pad`, the last jump's
  pad). The dialogs are modals, as wide as a phone allows.
* **Army Queue** (`army_queue.rs`, 600x640; VibeSkua Manager's Army Queue tab): ▶ Start Queue /
  Pause Queue / Resume Queue / ■ Stop Queue (asks first), How to Use (the Quick Guide, hidden for
  good with "Got it, hide guide"), the running status and progress, the squad tabs (name, count,
  ✕; drag one onto another to move it; + and Import Group), the selected squad (Squad Name, its
  default script with Browse (Search Scripts: the script it loads), File…, Options and ✕, "+
  Pick Accounts", then each account: name, its Script or "(Uses Squad Script)", its run state,
  and Script, File…, Options, ✕, Remove), and Preset: (drop-down, Load, Preset Name, Save
  Preset). Pick Accounts for Squad searches by name or username, with All (the shown ones),
  None, Done and Cancel. Squads run one after another (`bot/src/army/queue.rs`): each account
  with `--run-script`, `--script-options-storage ArmyQueue_<squad>[_<user>]_<script>` and
  `--exit-on-script-stop`, 3 s apart, where Army > Accounts launches: with "In a tab" in a tab of
  this window (`TabLauncher`; the tab logs out and closes once its script stops; an account open in
  a tab already shows "Already Open" and is skipped; one whose script hasn't run within the queue's
  timeout, 30 min, shows "Didn't Start" and its tab is closed), else in a `turu` of its own (a
  window, or none when headless). Once all have ended, a 5 s ghost-session countdown, then the next
  squad. Editing waits while it runs. Saved as you edit in
  `ArmyQueueActive.json`, presets in `ArmyQueues.json`. Stop script and Log out sent to
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
  V5's reasons; the footer: the online count on the left, Send to player (a pop-up: a player
  name box, Enter or Send, and up to 8 recent names to pick), Summon all, Settings and Debug
  Log on the right, the count on its own line and the buttons wrapping when the window is
  narrow. Settings for every
  client (Enable, passive attack, leech mode, always goto, copy master's walking, goto delay,
  rescue threshold, park location; WHILE FOLLOWING: quest bypasses, custom bypass quest IDs,
  follow master's quests, unlock all quests, fake
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
* **Notification Center** (`notifications.rs`, 560x480, VibeSkua's `NotificationCenterView`):
  the bot's feed of events (`turu_bot::notifications`: drops "(dropped)" and pickups "(picked
  up)", sales, deaths, relogins, map changes, quests accepted and completed, "Script started:
  name", "Script stopped: name", "Script error (name): ..."), newest first, the newest 500. Every
  bot collects it from its start, whether the window is open or not. Filter chips with counts
  (All, Items, Activity, Warnings, Errors, Maps; they wrap on a narrow window), then the rows: the
  kind's icon in its colour (painted: gift, warning triangle, error circle, map pin, play button;
  the kind as tooltip), the time and the message, wrapping. A click toggles a row's selection;
  Ctrl+C over the list copies it. Empty: a bell and VibeSkua's text for the filter. Footer: "N
  events · K selected" ("Copied N events" after a copy), Copy (the selection or all shown),
  Deselect, Save (Share on Android) and Clear (danger-soft); under 420 px the count goes above the
  buttons. VibeSkua's button icons and menu icon are left out: Turu's buttons and menus have none.
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
  on the desktop, AAudio on Android), so it sounds while the game is muted. On Android, while the
  app isn't on screen, the drop is also a notification ("<item> dropped", in the Drops channel;
  docs/ANDROID_NATIVE_UI.md, "Alerts"), as is a tab's script that stops. The list is the tab's
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
  egui modals. A `[Current]` class with no skill set gets VibeSkua's default `Base` set in the
  skill file (`EnsureSkillEntryExists`, `turu_bot::skills::ensure_skill_set`). Unlike VibeSkua,
  the drop-downs list your classes and gear as soon as the window knows who you are, even without
  a saved file.
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
* **Junk Items** (`junk_items.rs`, 700x500, VibeSkua `JunkItemsView`): search (ID, name or
  category) and ⟳; the category, AC (All / AC / Non-AC Items) and highlight (Goal Items, Near
  Stack Limit, Favorites, Equipped, Junk) filters; a select-all check box ("n selected", partly
  checked when only some shown rows are), Mark as junk and Unmark; 🎯 Goal quantity with Set goal
  and Clear goal for the selected rows (saved as `inventoryGoals` in `settings.json`, progress
  counts inventory + bank); the inventory and bank items by name, each row with a check box
  (a click on the row toggles it), the ID, the name (category on hover; amber and the row tinted
  when equipped), the AC, Equipped, ★, Near cap (an inventory stack at least 80% full) and goal
  progress chips, "Inv: n  |  Bank: n" and a red "Junk" chip. Narrow, the filters, goal row and
  sell row take two lines and the rows show their chips on a second line;
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

* **HotKeys** (Options > HotKeys; `hotkeys_window.rs`; the catalog, bindings, editing and running
  an action in `models::hotkeys` since 2026-11-21, the key presses and windows in `hotkeys.rs`,
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

Hotkeys run (`hotkeys::run_pressed` reads the presses, before the windows each frame, and
`models::hotkeys::run` runs the action) for the active tab while
Turu's window has focus (VibeSkua's Windows-only global keyboard hook isn't ported). The
modifiers must match exactly; a key that types is ignored while a Turu text box has focus;
the press is taken from egui, and the game gets it too, as it does Ruffle's own shortcuts.
What an action did, or why it did nothing, goes to the script log as `[HotKeys] ...`.
Android (both apps) has no HotKeys: neither the menu item nor the bindings.
Start Script follows Start Script's rules (a second press while stopping is Force Stop); Load
Script opens the Script Loader and its dialog; Open Bank is the bar's Bank (its note under
the bar); Auto Attack/Hunt use the Auto popup's class and mode, or stop the running one, and
don't start while a script or snippet runs; Game options are flipped in memory (not saved, as
VibeSkua); Lite settings are read and flipped in the game on a bot thread; Army actions go to
every tab and every account Army started (`army::broadcast`), Army toggles flip here and send
the new value; Load Script to All opens Army Control. Tooltips show the current key (Start,
Stop, Force Stop, Load Script, Bank). VibeSkua has no action for pausing, so Turu has none.
Android runs them from a hardware keyboard: `android/src/input.rs` gives egui every key (F1-F12,
letters, digits) with Ctrl, Shift and Alt, as winit does on a desktop.

## Native shells

`docs/NATIVE_UI_PROPOSAL.md` puts native windows in the shell, beside the client's picture, each
shell designed for its platform after a design pass the owner approves. This section is those
passes' record. Built so far: Android's Tabs item, tab panel, New tab and Accounts (L5a,
2026-11-20, "As built" below); everything else is the egui windows above.

### Android (design pass approved 2026-11-20; for L5a, and the frame for L5b)

**Mode and scene.** A phone held sideways (`sensorLandscape`, kept: native screens are designed
for landscape only), short check-ins on several accounts: which tab needs me, switch to it, start
another account. The game never leaves the screen: native screens are panes beside the rail over
the dimmed game, never full-screen pages. No bottom sheets (landscape is too short for them).

**Look.** Material 3 components with a fixed dark scheme, no wallpaper colours (Material You):
Compose and egui share one 64-dp column until L5b, so the surfaces are pinned to egui's tokens
(`theme.rs`), not generated from a seed. Surface `PANEL` `#11131A`, rows and cards `CARD`
`#181B25`, outlines `EDGE`, text `TEXT` and `MUTED`; primary `ACCENT` `#7C6CF0` with white on
it; success, warning and danger as egui's. Roboto in the M3 type scale, in sp; Material Symbols
Rounded for icons. Touch targets 48 dp. DESIGN.md's palette is the Monitor's and doesn't apply.

**Structure.** A navigation rail, the pattern for landscape. In L5a the shell owns its top item
and the client's egui rail starts below it; in L5b the whole rail is native.

**The room on the rail** decided the tabs' shape. The emulator is 411 dp tall in landscape, a
Redmi 10 about 393; the egui rail needs at least 320 (Menu, Auto and Jump at 52, the gap, the
48-dp chip, two 44-dp run controls, padding: `nav_bar.rs`), which leaves room for one item. So
the tabs are one **Tabs** item that opens a panel; on a screen 560 dp tall or more the tabs are
listed one by one in the rail instead. Auto and Jump don't fold into Menu to make room.

**Tabs item** (top of the rail, 64 x 52 dp, with the cutout's inset as the rail has):

* The active tab's avatar (two letters of its label, so army accounts with similar names stay
  apart) with its status dot; a count badge with two tabs or more, amber while another tab
  waits for you and red while one dropped out of the game (as built, below).
* Shown with one tab too: it is the way to New tab and Accounts.
* Hidden with the client's rail (Menu > Hide Side Bar); the `›` tab brings both back.

**Tab panel** (from the left, beside the rail, about 320 dp, full height, a scrim over the game;
Back or a tap on the scrim closes it):

* A row per tab: avatar, the whole label, and a status line: "Script running" (busy), "Waits for
  you: ...", "Restarting..." with a spinner, or "Closed" with **Restart**. The active row is
  selected. A tap switches to it and closes the panel; the native "Starting the tab..." view
  covers the hand-over as now.
* ✕ on each row closes it, asking first only while its script runs ("Hero's script is running.
  Close the tab?").
* At the foot, **New tab**; with every slot taken, "4 tabs at most" instead.

**New tab** (a page pushed in the same panel, with a back arrow): the accounts list, a tap starting
that account in a new tab, and under it "Blank tab (log in by hand)". With no tab open, the
shell's area shows "No tabs open" with the same list and Blank tab.

**Accounts** (the same panel, from New tab or from egui's Menu > Army > Accounts, which opens it;
the egui Accounts window goes on Android, Turu Mas keeps it):

* Rows: avatar (stepping aside at large text, so the name and its warning keep their words),
  the display name, the username muted, up to two group or tag chips with "+N". At the end, a
  tonal **Start**, or the account's state: "On screen", Switch to, "Starting…". An account
  without a password says so in amber with an "Add one" link to its form (Start stays: the tab
  opens at the login screen). Why a start failed (red) shows under the name ("No tab was free to
  start it in"). A ⋮ menu ("More for <account>"): Edit, Move up, Move down, Remove... (its dialog
  says the saved password goes too).
* Search in the top bar, and filter chips under it: All, each group, each tag. Under them, what
  every Start uses, on one line ("Starts on Twig · runs <script>"), opening Launch options.
* Above the rows, a box checking every shown account or none and the count ("5 accounts · 1 of
  4 tabs open", "2 of 5 selected"); with a group's chip on, **Start N** starts its accounts
  without a tab.
* A tap on a row checks it, as Army Control's (a long press too; only Start starts): a
  contextual bar with the count, Start, Add to group, Select all. Any Start, one account's too,
  goes through the tab check: with more accounts than free tabs it starts as many as fit and says
  so, with none free it says to close one. The panel stays, showing "Starting…", then Switch to
  (critique fixes, 2026-11-21; a row's tap started the account before).
* Add and Edit: a page pushed in the panel, Username, Password (with a show toggle; saved to the
  Keystore), Display name, Tags. Typing in landscape uses Android's full-screen keyboard field,
  so the form needn't fit above the keyboard. A username already saved: "Hero is already in the
  list" with an "Edit it" link.
* Launch options, a page of their own from the line above the list: Server (with its ping where the shell can measure
  it, the names alone otherwise) and the script, from favourites and recent
  (`quick-scripts.json`) or none, and Search scripts… (Search Scripts, picking), with
  Auto-start. (The owner overruled "a script's full search stays the Script Loader's" on
  2026-11-20: see "Search Scripts everywhere a script is chosen".)

**Where the rest goes** (L5b, after the checkpoint). The native rail, top down: Tabs, Scripts,
Logs, Army, More; then Auto, Jump and Bank; the script chip and run controls at the bottom.
Screens open as panes beside it. The egui rail's icons move to Material Symbols with it.

| Feature | Android |
|---|---|
| Tabs, Accounts | native (L5a) |
| Script Loader, Search Scripts, Scheduler, Logs, Options (Game, Application, CoreBots), Army Control, Notification Center, Stats, Current Drops, Skills, Loadouts, Fast Travel, Butler, Cosmetics, Script Options, Runtime, Notify Drop, Loader, Grabber, Junk Items, Console, Packet Logger, Spammer, Interceptor | native (L5b) |
| Army Queue | none (its squads run as processes of their own, which Android can't start) |
| HotKeys | egui, from More (unused without a keyboard); its model is shell-neutral (`/models/hotkeys`) |
| Grid View | none (a phone has no Grid View) |

**What L5a needs from the link** (`frontend/src/shell.rs`): that the shell draws the tabs (a field
of `Tabs` or `Show`) and where the client's rail starts (`rail_top`, apart from the strip's
height, since the game keeps its full height); `waits` and `rail_hidden` in `Status`; and
`ToShell::OpenAccounts`. It rests on L0 showing a `ComposeView` sharing the column with a slot's
surface through a tab switch; if it can't, the same panel opens from a Menu entry instead.

To check before building: what `SlotShell` does when the last tab closes (the empty state above
assumes it stays open), and whether the shell's Rust can read the server list for the ping.

### Android, as built (L5a, 2026-11-20)

Code: `android/app/src/main/kotlin/com/muktiarafi/turu/` (`ShellScreens.kt` hosts two
`ComposeView`s over the slot's surface: the item, and the panel only while open; `TabPanel.kt`,
`AccountsPage.kt`, `ShellState.kt`, `ShellTheme.kt`, `AccountsPanel.kt`), the list from
`slots::SlotShell::native_tabs` (link: `Tabs.rail_top`, `Status.waits`/`rail_hidden`,
`OpenAccounts`), the accounts from `android/src/shell_accounts.rs`. Where it differs from the pass
above, and why:

* **The last tab has no ✕**, and there is no "No tabs open" state: `SlotShell::close` keeps the
  last tab (the pass's question, answered: it does).
* **The rows say where each tab's game is** (2026-11-21, from the panel's critique): the slot's
  `Status` carries `game` (`shell::TabGame`: in game, logging back in, logged out, or dropped:
  in game before and not now, with no relogin under way), the map and the loaded script, and the
  status line reads "Running GoldFarm · battleon", "Idle in battleon", "Waits for you: ...",
  "Logging back in…" (amber ring), "Disconnected" (red), "Not logged in" or "At the login
  screen" (no saved login). A tab opened for a saved login that is out of the game has **Log
  in** ("Log in again" after a drop) under its status, the control API's `/login` (the error
  shown under it if it fails). There is no "Closed" with Restart: a slot whose process dies is
  restarted by Android and shows "Starting…" with a turning ring in its dot, apart from an
  idle tab's grey one. The Tabs item's count stays when another tab needs you: amber while one
  waits, red while one dropped (the "!" took the count's place).
* **✕ asks first whenever the tab is logged in** (or its script runs), since closing it logs the
  character out; a tab out of the game closes at once. Full: "Close a tab to open another".
* **New tab lists the accounts to start first** and those already in a tab last, with "On
  screen" or **Switch to** (not "In tab 3": a slot's number, which the list doesn't show), and
  Blank tab kept at the foot. Its top bar's link says Accounts, as the Tabs page's does.
* **Add/Save sit in the form's top bar**, not at its foot: Compose gets no full-screen keyboard
  field in landscape, and with the keyboard up the panel has room for a field or two above it
  (the page shrinks with `imePadding`, the focused field scrolls into view).
* **"No password" (amber) under an account without a saved password**, in place of "Log in by
  hand" at the end of its row (Start stays there; the longer words were cut off). "Failed: reason"
  isn't shown: the shell doesn't learn a slot's login result.
* **The long-press selection** (2026-11-20): the bar shows the count alone ("3", read out as "3
  selected"), ✕, Start, and a ⋮ with Add to group…, Select all shown and, under a group's chip,
  Remove from the group: the 320-dp panel had no room for "3 selected" and Add to group beside
  Start. Start asks first when the free tabs are fewer than the accounts ("Only 2 of 4 tabs are
  free": who starts, who won't), rather than starting some and saying so after, since the
  panel closes once tabs start. Add to group lists the groups (a tap adds) and takes a new name.
  Group editing: Remove from the group (the egui window has none: it only adds) and, from the
  page's ⋮ under a group's chip, Remove the group (asked first; its accounts stay). Army
  Control's Groups lists what these make.
* **Not built yet**: the 560-dp-tall layout listing tabs one by one in the rail.
* **The rail's top**: the item is 8 + 52 dp (`slots::RAIL_TOP`); the slot's rail starts below
  it and its symbol folds on a short screen as before.
* **The Script Loader** (L5b, 2026-11-20) is a page of the same panel, opened however the slot
  opens its Script Loader (the rail's script chip, Menu > Scripts, the hotkey, Search Scripts'
  Start). The pass puts it on the native rail; until Auto, Jump and Bank have models the rail
  stays egui, and it has no room for another item. The page: status card (state with its dot and
  clock or last run, last log's age, "Waits for you", the script with its favourite star, the
  error's first line, the whole error on a tap) and the script log (its last 500 lines,
  line-fed, following its last line unless scrolled back), laid out by state after a critique
  (2026-11-21): with no script, Search scripts as the filled button in Start's place; with one
  loaded and stopped, Start, then Search and Options (the reason a control is off under it);
  while it runs, Stop with Pause/Resume (Force Stop while stopping) alone, Search scripts and
  Script options moving to ⋮. While nothing runs, "Start a favourite" and "Start a recent
  script" list three of each as rows (full name, a play mark; a tap loads and starts it, all
  off while one starts). The top scrolls within 55% of the page while the log has lines, so the
  log keeps the rest; the tapped run control shows a spinner until the tab answers.
* **The native rail** (L5b, 2026-11-20) replaces the egui one on Android: the Tabs item, then
  Logs, Army, Auto, Jump, Bank (and Update while one is out), More last (three dots, the
  overflow: a hamburger said "drawer"; moved there 2026-11-21 after a critique), the script chip
  and its run controls at the bottom; the chip opens the Script Loader. The run controls are
  48 dp; a second control (Stop under Pause) stands 8 dp apart, so a slip from Pause doesn't stop
  the run. With no script loaded the control under the chip is Search scripts (Search Scripts,
  loading in the tab); an off control opens the Script Loader, which says why. The chip's word
  falls back to a shorter one where the long one doesn't fit ("None", "Done", "Ended", "3h"),
  its clock in tabular figures. Items, the Tabs item and the chip grow with the font scale
  (`railItemHeight`: 52 dp at 1.0), and the fold measures them, so larger text folds an item
  rather than crowding: 411 dp keeps Auto, Jump and More at 1.0 and 1.3. Each item reads out its
  badge or spinner ("Auto, running", "Update, failed: ...") and the page items are tabs with
  their selection; icons are muted until selected, the selection a stronger pill
  (`Turu.RailSelected`). All icons are the one stroke (`RailIcons`: Jump's pin and More's dots
  too). The `›` handle takes touches 48 dp wide. No Scripts item: the owner took it off
  (2026-11-20) to keep Jump on a short screen. On a short screen it folds least needed first
  (Army, Bank, Logs and Update, then Jump and Auto; More never), counting two run controls so
  starting a script moves nothing: the emulator's 411 dp keeps Auto, Jump and More.
  Auto, Jump, Logs and More open as pages of the side panel, the rail staying in use beside it;
  Army opens Army Control. Bank's reason for opening nothing shows as a toast (a snackbar
  would need the panel open). Auto's page, "Combat Automation" (critique fixes, 2026-11-21):
  what to fight with first: Class ("The equipped class" until one is picked; the list re-read on
  opening, the equipped one first and marked "Equipped"; "Picking one equips it" under it, not
  typed into, since the keyboard would cover the list), Skills ("Base" until a class's saved
  set is picked) and "Monster IDs to fight first" (its line says what it read: "Fights 1, 2, in
  that order"); then "Fights with <class>, <skills>" and **Attack** and **Hunt** (VibeSkua's Auto
  Attack and Auto Hunt, alike, each with a line on what it does), the tapped one with a spinner
  until it runs. While one runs, a card leads the page ("Hunt · 3m", "With Void Highlord,
  Base") with Stop beside it, the settings dimmed (they're the next run's) and the start buttons
  gone; the model says which runs, with what and for how long. More (440 dp) is list and detail: a search above it ("Search
  windows: drops, packets…") lists every window whose name or line matches each word typed, with
  its menu; under it Recent (the last four windows opened, once there are any) and the menus down
  the left (Scripts, Options, Tools & Helpers, Combat, Diagnostics, Plugins, Army), an arrow at the
  list's foot while more are below, the one picked on the right. Each window has a line on what
  it's for under its name (or why it's off); one that opens the slot's egui window, not a page
  here, says "Opens the full window" (critique fixes, 2026-11-21: the chevron every native row
  had is gone). The menu on show and Recent are kept on the device, through a font-size change
  too. Its bar has Bank and Update while the rail has folded them (no menu has either), and ⋮
  with Hide Side Bar; a `›` tab at the left edge brings the rail back, in every tab. The rail's
  icons are drawn in one stroke where Material Symbols' core set lacks them (`RailIcons.kt`);
  Jump's pin is the core set's filled one.
* **Search Scripts** (L5b, 2026-11-20) is a page of the panel, 440 dp wide (its rows need it),
  opened from the Script Loader's Search, More > Scripts, and however the slot opens Search
  Scripts (but
  while the egui Search Scripts picks a script for the egui Army Control). From the native Army
  Control's Search scripts… it picks (and from Accounts' launch options: "Pick the script launched
  accounts run", each row's button Use, a script that needs a download downloaded first while
  the page waits): a bar under the title says for whom ("Pick a script to
  load in Questwalker", "in 2 clients") with Cancel, each row's button is "Load in …" (a tap on the
  row too), and a pick goes back to Army Control. The search, with what it looks in inside the
  box ("All fields" and its menu); the keyboard's Search closes the keyboard to show the results,
  and while the keyboard is up the filters and the foot step aside. Then the filters as chips:
  folder, the sort (a menu of VibeSkua's four, the chip in short: "A to Z"), Pinned and On this
  device (VibeSkua's Local). Then the rows: the name (a star when pinned; a file name run
  together shown in words, "Demanding Approval From Nulgath"), and under it a badge only for what
  sets the script apart (Update, Modified, Local, Removed, or Not downloaded: "Downloaded" goes)
  and one line of description (none, or the index's "null": its folder). Start is a text button,
  filled only on the first row when its name starts with what's typed (VibeSkua's sort stays, so
  the first row isn't always the best match); a row's menu has the path, what the badge means,
  Start, Load, Add to the queue, Download, Update or Replace, Pin, each with why it rests. The
  task's progress with Cancel, or the last outcome, at the foot, a success going after 8 s;
  Update all, Download all and Refresh in the bar's menu, to leave the short screen to the rows.
  Delete and Reset Scripts stay in the egui window. Critique fixes, 2026-11-21.
* **Game Options** (L5b, 2026-11-20) is a page of the panel, 440 dp wide, from More > Game and
  however the slot opens it: the search and the sections as chips (with counts, and Changed while
  anything differs from options.json), the options under their section's heading (and group's)
  with a switch, a number box, Set FPS with its presets, a text box (Relogin Server's a text box
  too: the page has no server list), a choice or Reload Map; "Edited" or "Changed outside"
  outlined after the label; an option that depends on another indented and dimmed while that one
  is off; an unused one dimmed with "Coming soon". At the foot one status line, Defaults (asks
  first), Revert (asks first while a script changed options) and Save.
* **Application Options and CoreBots** (L5b, 2026-11-20), pages of the panel from More > Options.
  Application Options: search and section chips, each setting with its control and saved as it
  changes ("Restart required" under those that need it), Test, Clear Flash Cache (asks first),
  Check Now, Free and Open as buttons; the Web Monitor's links, QR code and keys say "In the full
  window" with Open. CoreBots: the character and whether its file is saved, Loadout, Options and
  Other as segments, a card per role (class, mode, Use equipment, the gear folded under "Gear · N
  of 6 set"), Options and Other as option rows under their groups, Refresh from game, Revert and
  Save (asking before public rooms); the page saves when it closes, as the window does.
* **Scheduler** (L5b, 2026-11-20), a page of the panel (440 dp) from More and however the slot
  opens the Scheduler: Start queue (Stop queue in red while it runs) with the summary, and the
  reason it's off under it; the playlist as rows (number, name, status chip in its colour, time;
  the running one tinted), each with a ⋮ of Move up, Move down, Rename, Script options and
  Remove (Move and Remove off while it runs, saying why); an empty state that says where scripts
  come from; Load, Save and Add scripts (Search Scripts, whose Add to the queue adds them) at the
  foot, with the note above. Load lists the playlists folder; Save and Rename ask for a name,
  selected so typing replaces it, and the keyboard's Done confirms (in landscape it covers the
  dialog's buttons). Script options opens the slot's Script Options window, the panel closing,
  or says why there's nothing to set. Double-click to rename is the egui window's.
* **Army Control** (L5b, 2026-11-20), a page of the panel (440 dp) from the rail's Army, More >
  Army and however the slot opens it. Above, a box that checks all or none, the count ("2 of 3
  selected"), and Groups (an Army > Accounts group's rows, and only those). Then a row per client
  (every tab, the others read from their own slots): a check box, a dot in its state's colour, the
  name with "on screen", "tab" or "no window", and on the second line its script (or Auto) and
  where it is; Start, or Stop while it runs, and a ⋮ of Show its tab, Log in or Log out, Relog,
  Start or Stop Auto-Attack (with why not), and for a launched client Restart and Close. A tap on
  the row checks it. Under the rows, who the commands go to ("To all 2 clients · check rows to
  pick", amber; or the checked names), then Script (Start script, Stop script, Search scripts…, which opens Search Scripts to pick one),
  Account (Log in, Log out, Relog, Go to house) and Auto-Attack, Quest and Misc options folded,
  and Travel (Map Travel…, Jump to player…, Change server…, since 2026-11-21: see below). A row's
  ⋮ also opens its Gear… and Logs… pages. Stop script and Log out
  to every client ask first in a dialog, where the window wants a second click. The last outcome
  sits at the foot with History. The shell keeps the checked rows, by a key every slot gives the
  same row, so they stay through a tab switch; the outcomes are the slot's, so the list after a
  switch is that tab's; a script is named by its file there ("Load Farm.ts: done"). Load file… and
  saving a client's logs to a file are the window's.
* **Map Travel** (L5b, 2026-11-21), a page of the panel (440 dp) from Army Control's Travel: who it
  sends (as Army Control says it), Go to (Map and Room; Cell and a Pad drop-down; Use current, the
  only checked row's location else the tab on screen's; `map,room,cell,pad` pasted into Map fills
  them all; the note "Sends 2 clients to battleon-2 · Enter, Spawn" and Jump), Recent destinations
  (Clear, with Undo in a snackbar) and Saved presets (New…), a line saying a tap fills Go to and
  Jump sends. A destination row's tap fills Go to and its Jump (a text button) sends it; a
  preset's ⋮ has Edit… and Delete (asked first). New… and Edit… open a form page (name, map,
  room, cell, pad, Use the client's location, Save in the bar), a new one starting from Go to,
  else where the client is. **Jump to player…** is a dialog: the clients' logged-in players as
  chips, the name field under them (the last one jumped to first). **Change server…** is a dialog
  listing the online servers as radio rows once loaded; none is picked until tapped.
* **Gear** (L5b, 2026-11-21), a page from a row's ⋮: online or not with its bank's age and Refresh,
  "Stop the script first" (a switch, off each time) while a script runs, then a row per slot, the
  item drop-down (worn first; "(bank)" after a bank item) with Equip, and what runs or last ran at
  the foot. **Logs** (same day): Script and Debug chips with the line count, the log monospaced in
  an inset well, selectable, kept at the newest line; ⋮: Copy, Full window (saving to a file).
* **Notification Center** (L5b, 2026-11-20), a page of the panel (440 dp) from More > Diagnostics
  and however the slot opens it: the tab's feed, newest first, under filter chips with their
  counts (All, Items, Activity, Warnings, Errors, Maps). A row is the kind's icon in its colour
  (a gift green, a play button muted, a warning amber, an alert red, a pin in the accent), the
  time and the message, wrapping. A tap selects; while anything is selected the bar is the
  selection's ("2 selected", ✕, Copy, Share), and Copy ends it with "Copied 2 events" where the
  count was. ⋮ has Copy and Share for every event shown, Clear (asked first, where the window
  clears at once) and Full window. Save is the window's.
* **Stats** (L5b, 2026-11-20), a page of the panel (440 dp) from More > Combat and however the
  slot opens it: the script clock, the six counters on flat tonal cells (tabular figures, a dot
  in each counter's colour), and Advanced, whose label toggles it as the switch does. With
  Advanced on: the session, the unit as chips (/hr, /min, /sec), Level and Rank toward their next
  step (a bar, the value, the rate and time left), the four rates, then the rate history: Save,
  Export (Android's share sheet, as CSV), Clear… (asked first) and each snapshot as two lines (who
  and when; its rates). Reset is in ⋮, asked first (the window resets at once). The history's
  sortable table is the full window's.
* **Current Drops** (L5b, 2026-11-20), a page of the panel from More > Tools & Helpers and
  however the slot opens it: a search and an "AC only" chip with the AC count, the drops as rows
  (a check box, the name, ×quantity, an AC tag), a tap checking one, and Pick up N and Pick up all
  at the foot.
* **Skills** (L5b, 2026-11-20), a page of the panel (440 dp) from More > Combat and however the
  slot opens Advanced Skills. VibeSkua's two tabs as segments. Edit: Class and Mode, the sequence
  (a row per skill: its place, the slot in a tonal circle, "Skill 2" or "Auto attack" / "Potion",
  its rules in words; a tap opens its rules, ⋮ has Edit rules, Move up, Move down, Remove), the
  0 to 5 buttons (the line under them says what 0 and 5 are), Rules for added skills (a row with
  On or Off), Use if available / Wait for cooldown as segments with a line on what each does,
  Skill timeout and Reset on target change; at the foot what Save does ("Save adds …" or "Saving
  replaces …'s Farm set"), or why it can't, beside Save set. A skill's rules are a page pushed
  over it, as Edit Rules (Done keeps them, Back and the arrow drop them; Reset in its bar): Use
  rules, then Health, Mana and Party HP as ≥/≤, the amount and %/# on one row, the aura (or
  Several auras with All of them / Any of them, each aura a card with ×), Wait before use and
  Skip if not available, all dimmed while Use rules is off. Rules for added skills is the same
  page, applying as it changes (Close). Saved sets: Find a class over the classes' list (the file
  holds about 270, so the page reads one class's sets at a time), then that class's sets (the mode
  as a chip, how it uses skills, the sequence); a tap opens one in Edit, ⋮ has Remove… (asked
  first). The page's ⋮: Refresh saved sets, Reset sets… (asked first, where the window wants a
  second click) and Full window. The editor is the slot's, shared with the egui window, so Full
  window shows what is being built. With the keyboard up the segments and the foot step aside so
  the field shows above it. Differs from the window: no selection (each row's ⋮ acts on it), and
  no Load Classes / mode drop-down / Open (a class's sets are listed whole).
* **Loadouts** (L5b, 2026-11-20), a page of the panel (440 dp) titled Loadouts Manager, from More >
  Tools & Helpers and however the slot opens it. The list: Currently equipped (its class) and the
  character's loadouts (name, class, a green Worn chip when every item is worn), each opening the
  loadout in the same page (Back returns). A loadout: class and item count, then the seven slots
  (the slot's name, the item or Nothing, the enhancement it keeps under it, amber "· worn with …"
  while the worn one differs, and a chip for where the item is: Worn, In bag, In bank, Not found,
  Not in bag), notes on what Equip meets (enhancements that differ, a running script, the bank not
  read yet or when it was), then the foot: the outcome, a spinner while reading, equipping or
  enhancing, and Equip (with Enhance, tonal, when enhancements differ). Equip while a script runs
  asks before stopping it ("Stop Farm.ts and equip?"), where the window's button says so; Enhance
  asks first (it may travel and spend gold). Save current gear, at the list's foot (Save as
  loadout on Currently equipped), opens the new loadout with Rename's dialog, its name selected,
  where the window puts the cursor in its name field. The page's ⋮: Rename…, Replace with worn,
  Delete… (asked first), Read again, Full window.
* **Fast Travel** (L5b, 2026-11-20), a page of the panel (440 dp) from More > Tools & Helpers and
  however the slot opens it: the search, Private room (its number in a box, the switch at the end,
  the row's line saying what it does; the number dimmed while off), then the destinations as rows
  (a pin in a tonal circle, the name, "map · cell · pad"; a tap travels, the row tinted with a
  spinner while it does, the others resting; ⋮ has Edit and Remove), and at the foot the outcome
  (or "3 saved · a tap travels") beside Add. Add and Edit are a page pushed over the list: Name,
  Map, Cell beside Pad (a drop-down of the eight), Use where I am (Get current, filling map, cell
  and pad), and Add or Save in its bar, the keyboard's Done on Cell confirming. Clear… (asked
  first), Read again and Full window in ⋮. The window's tiles in columns are a list here.
* **Butler** (L5b, 2026-11-20), a page of the panel (440 dp) from More > Plugins and however the
  slot opens it: this account and its state in its colour ("Following Hero (summoned)", "Not
  following anyone"), Release all and Stop following (red) when they apply, and the last outcome;
  then Accounts, Settings and Log as segments. Accounts: a row per online account (the name and
  its chips in their colours: fighting, following me, busy: Farm.ts, other server...; where it is;
  Follow or Stop, outlined, and Summon, tonal, or Release; a line under them with why either
  rests), and at the foot the count (or why Summon all rests) beside Summon all. Settings:
  Butler's options under Settings (every client), While following and This account, as Game
  Options' option rows (descriptions shown, where the window keeps them in tooltips), saved as
  they change. Log: the newest 300 lines in monospace in their level's colour, kept at the bottom,
  with a line saying when there are more or when Debug logs is off. ⋮ has Send to player… (a name
  and the recent ones; the keyboard's Go sends), Copy, Save and Clear the log, and Full window
  (which also opens the saved logs' folder). Butler's suspend question stays the slot's.
* **Cosmetics** (L5b, 2026-11-20), a page of the panel (440 dp) from More > Plugins and however
  the slot opens it, as segments. Copy: a room player (a drop-down of the room's players, read
  again when it opens) with Copy, Copy my target's look, a character name with Copy (its
  account.aq.com page; the keyboard's Go copies), Female, Male and Swap, and a line saying the
  changes are this client's only. Catalog: the search (name or ID), the category chips (All,
  Armor, Weapon, Helm, Cape, Pet, Ground Rune, Favorites with a heart), Kind within Weapon and
  Sort scroll away with the items, since a landscape phone has no height to pin them; a row is
  the name, "category · #ID", a heart (red when kept) and Equip; the foot is "1 to 50 of 67,619"
  with the page arrows. Outfits: the count and Save current look (asks a name), then each outfit
  with the slots it fills and when it was made, Equip, and ⋮ Delete… (asked). Colors: the six
  colors (a swatch, what it colors, the hex, red while it isn't one), Apply colors and Read from
  avatar, then the themed presets with their four swatches (a tap fills base, trim, accessory and
  eye and applies; skin and hair stay). The outcome of the last action sits at the foot (with a
  spinner) only while there is one. ⋮: Reset to Real Gear, Check catalog updates, Full window
  (Manual SWF, and the catalog beside its item, are the window's). With the keyboard up the
  segments and the foot step aside.
* **Script Options** (L5b, 2026-11-20), a page of the panel (440 dp) from the Script Loader's
  Options, More > Scripts > Edit Script Options, the Scheduler's Script options (a queued entry's)
  and whenever a script asks for its options (before a run, `bot.config.configure`). Under the bar
  where they are saved ("A queued entry's options · Saved in options/Farm.cfg") and, while the
  script waits, "The script waits for these options: Save or Cancel lets it go on" in amber. A
  filter from ten options on. Each option: its label, description and the picked choice's
  explanation, with a switch, a number box, a drop-down of the choices beside it, or a text box
  under it; an item option's box takes a typed name and a drop-down of the account's items of that
  kind ("Worn", "Bank"; what's typed narrows it), with a warning when the name isn't among them.
  Groups under their titles; a group of many switches is a checklist: "3 of 40 picked", All and
  None (for the rows shown), Hide owned (N), each row toggling as a whole with "Owned" or "In bank",
  and an amber note with "Use select" when a choice elsewhere leaves it unused. Cancel and Save
  at the foot; Back cancels. ⋮: Reset to defaults… (asked; saved only by Save), Full window. The
  draft is the slot's, so Full window shows the same. Leaving the page (the scrim, another page,
  another tab) drops the draft, unless the script waits for it: then the slot's own window shows
  it again, and in a tab not on screen it waits in that tab's picture until the tab is shown. With
  the keyboard up the filter steps aside unless it's the field typed in.
* **Runtime** (L5b, 2026-11-20), a page of the panel (440 dp) from More > Combat and however the
  slot opens it, as segments with their counts (Drops 2, Quests 1, Boosts with how many are on);
  the bar says "Picking drops · Boosting" in green while either runs, whichever part shows. Drops:
  Pick drops (tinted while on, saying what it does), the pickup list (a tap checks a row; Remove N
  in the heading), then Options: Reject else, Accept AC drops, Interval (ms, sent as typed, empty
  is 0) and Drop notifications with Open (pushes Notify Drop). Quests: what
  registering does, the registered quests ("1234 (Reward: 5678)") with Remove N. Boosts: Use
  boosts, the four boosts with their item ("Item 12345", or none found yet), Search inventory and
  Search bank ("Searching…" while one runs). The add fields sit at the foot: a drop name or IDs
  (`|` between several), or quest IDs and an optional reward; a quest typo stays with the error.
  ⋮: Full window. With the keyboard up the segments step aside.
* **Notify Drop** (L5b, 2026-11-20), a page of the panel (440 dp) pushed from Runtime's Drop
  notifications, and however the slot opens it. Beep on a drop with Test beep beside it, then
  "[5] beeps, [200 ms] apart" (number boxes, sent once in range: 1 to 25, 10 to 2000 ms); Watching
  with its count and Remove N for the checked rows, a tap checking one; empty, it says what a drop
  does (a beep, and on Android a notification heard with the screen off). The add field (a name,
  `|` between several) sits at the foot. ⋮: Clear all, Full window. With the keyboard up only the
  field typed in stays: the add bar, or the beep settings.
* **Loader** (L5b, 2026-11-20), a page of the panel (440 dp) from More > Tools & Helpers and
  however the slot opens it; the bar says how many quests are cached. Quests or Shop (segments),
  the IDs ("2566, 2567"; a shop takes one) and Load (the keyboard's Go loads), then the cache's
  search (name or ID), its quests a page of 50 at a time (name, ID; a tap checks one, kept across
  pages), and "1 to 50 of 158" with the page arrows; the fields scroll away with the quests on a
  landscape phone. The foot, only when there is something: what runs with a spinner (Cancel for
  a cache update) or the last notice, and with quests checked Copy (IDs, Names, As // Name | ID),
  Fake complete (one quest checked) and Load N. ⋮: Download community cache, Add missing quests,
  Rebuild all, Update range… (asks the first and last IDs), Full window. The first open with no
  cache file downloads the community one. With the keyboard up the foot steps aside.
* **Grabber** (L5b, 2026-11-21), a page of the panel (440 dp) from More > Tools & Helpers and
  however the slot opens it. List (a drop-down of the ten, "Inventory - 166 grabbed") and Grab
  (spanning the field's box at any font size), then the search (once something is grabbed) and the rows a page of 100 at a time: the row's
  text, its ID and an info button showing its fields (a dialog titled by the item's name). A tap
  checks a row; Shop IDs and Temp Inventory keep one. The foot: what runs with a spinner and
  Cancel, or the status ("Copied 1 item(s) to clipboard.", "3 checked", or with none checked
  "Check quests in the list to use these"), then the list's actions in an even grid on the
  page's edges, as few rows as fit (four in one row, five as 3 + 2), Sell and Sell All in red.
  An action that needs checked rows is off until one is (VibeSkua answers "No quests
  found/selected." after the tap); with nothing grabbed only Unregister All shows. Kill says why
  it waits. An action's quantity question
  and message box are dialogs. ⋮: Copy (the checked rows, or all shown), Full window (Export to a
  file). With the keyboard up the foot steps aside.
* **Junk Items** (L5b, 2026-11-21), a page of the panel (440 dp) from More > Tools & Helpers and
  however the slot opens it; opening it reads the inventory and bank again, as VibeSkua. The
  search with Refresh, then Category, AC and Highlight as chips on one row (a chip filtering is
  lit; its menu ticks the choice), a box checking every shown item ("1 checked · 166 items", the
  junk count), and the items a page of 100 at a time: the name (amber and bold when equipped, the
  row tinted), its ID, then the quantities, category and chips (AC, Equipped, a star for a
  favourite, Near cap, the goal, Junk in red). A tap checks a row. The foot: with items checked,
  Mark as junk, Unmark, Goal… and Sell checked on one row that scrolls sideways at large font
  sizes; else a hint and Sell all junk (N). Goal… swaps the foot for a goal bar (the quantity,
  Set, Clear, close) that stays above the keyboard. The sales ask first unless Skip sell warning
  is on, and turning that on asks too. ⋮: Sell all junk, Unmark all (asked first), Skip sell
  warning, Full window.
* **Console** (L5b, 2026-11-21), a page of the panel (440 dp) from More > Diagnostics and however
  the slot opens it. The snippet editor (monospaced, no autocorrect or capitals, starting from the
  snippet last run in the tab), then Clear, a note (the last expression's value is shown; why Run
  waits: a script or Auto runs) and Run, Stop in red with a spinner while it runs, then the output
  in an inset well, a monospaced line each in VibeSkua's colours (the run's first line grey, logs,
  the value green, errors red, Stopped amber), selectable, scrolling sideways, kept at the newest
  line. With the keyboard up the bar, note and output step aside: the editor fills the strip
  above it with Run beside it. ⋮: Copy output, Clear output, Full window.
* **Packet Logger, Spammer, Interceptor** (L5b, 2026-11-21), pages of the panel (440 dp) from More >
  Diagnostics. Logger: Start logging (Stop logging in red) with a dot and its state, "Show 12 / 12"
  opening the filters dialog (a check box a kind in two columns, All, None, Invert, Apply), the
  search with the count, and the packets monospaced in an inset well, newest last, selectable;
  ⋮: Copy shown, Clear, Full window (Save). Spammer: Start (Stop in red), "Every [1000 ms]", Send to
  client, "Queue · 3" with what it sends, the queue (number, packet, a remove button), and the
  composer (the packet, Add, Send now), which alone stays while the keyboard is up; ⋮: Clear the
  queue, Full window (Load, Save). Interceptor: the Server drop-down (Current server first) with
  Refresh, Connect ("Connect and relog" with a server picked; Disconnect in red), Log packets, the
  state dot, the search and "Show 18 / 18", the packets with their OUT, IN or BLOCK tag (a tap
  opens the packet in full in a dialog with Copy and Select; a long press selects, and while some
  are selected a tap selects too; a blocked one struck through), and the count ("the newest 300 shown" past them,
  "· 2 selected", Unselect); ⋮: Copy, Clear, Full window (Export).
* **Clear and Undo** (2026-11-21, after the critique): the Packet Logger's, the Interceptor's,
  the Spammer queue's and Map Travel's Clear act at once and show a snackbar ("Cleared 12 packets",
  Undo) for a few seconds; Undo puts back what was cleared before what came since. The Spammer's Add
  clears its field, and Send now says "Sent to the server" (or the client).
* **The engine's dialogs** (L5b, 2026-11-21): a script's message box, Butler's "a script started
  while following", the crash notice, "Send crash reports?" (its Privacy policy a text button under
  the note, opening the policy and leaving the question up), the Tab Limit notice and the graphics
  tips, one at a time
  over the game whether the panel is open or not, as Material dialogs on the panel's card colour:
  the title, the text (a script's or a report's scrolls past 320 dp), a smaller muted note, and
  the buttons as text buttons left to right, the last (the main one) in the accent and semibold.
  The words are egui's. A script's box isn't closed by a tap beside it; Back closes any of them.
  Show report opens that report's page in Logs > Crashes; Raise Tab Limit opens Application
  options.
* **Logs > Crashes** (L5b, 2026-11-21; `models::crashes`): a third segment, "Crashes (n)", once
  there is a report (most never crash) and while it's chosen; Clear leaves the bar. The reports
  newest first, each a row with what happened (two lines at most) and when, a chevron; none left
  says so. A tap opens the report's page (440 dp): what happened, when and the file's name, then
  Share (filled: the report goes to whoever helps), Copy and Delete (red text button, at the end),
  then the text monospaced in the inset well, long lines wrapped (most reports are sentences), selectable. A long report
  shows its first 400 KB with a line saying so (what Share and Copy take too). Delete goes back to
  the list with a snackbar ("Deleted "Low memory"", Undo); Undo puts the report back in its place.
  The egui window draws from the same model.
* **Search Scripts everywhere a script is chosen** (the owner's rule, 2026-11-20): searching is
  how users reach scripts, so it is never only the Script Loader's: More > Scripts has it, and
  every place that chooses a script offers Search scripts…, which opens it picking for that
  place: Army Control (load in the clients), Accounts' launch options (the launch script), the
  Scheduler's Add scripts (its queue). A new screen that chooses a script does the same.
* **Full window**: every native page's ⋮ ends with Full window, the slot's egui window (which
  stays egui until it closes), and says what it adds there.
* **Contrast**: white on the accent (`#7C6CF0`) is about 4.0:1, under 4.5:1 for small text. The
  pass pins it (as egui does); it is used on the filled buttons (Add/Save, Start queue, Start script) and the count badge.

## Adding a window

Add `foo.rs` with a struct and `show(&mut self, ctx, &mut bool)`, build the window with
`super::window(ctx, "Title", [w, h], slot)` (lists that fill the window take
`ui.available_height()` minus the footer, with a small floor), add it to the
`windows_fit_small_screens` test in `mod.rs` when it needs no file picker, add a flag to `Windows`, a field and a call in
`TuruUi::show_windows`, and enable the entry in `nav_bar.rs`.
