# Turu UI

Turu uses **VibeSkua's layout and flow** (see `VIBESKUA_SPEC.md`, section 5) in **Mugetsu's
look** (its right-click panel: dark panel, purple accent, corner radius 12, soft shadow).
Code: `app/src/gui/turu/`. Integration points: `app/src/gui.rs` (`RuffleGui::update`
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
(370x500, 612x720, ...) and are clamped to the window. They stay below the nav bar and shrink
with the app window, also after they opened; what no longer fits scrolls, so the footer buttons
stay reachable (`super::window`; `windows_fit_small_screens` checks the windows at 1400x1000,
900x600 and 640x420). Runtime stacks its two columns when the window is narrower than 680 px.

## Nav bar (`nav_bar.rs`)

34 px high, left to right as in VibeSkua: **Scripts** (Script Loader, Edit Script Options, Scheduler), **Options**
(Game, Application, CoreBots, Application Themes, HotKeys), **Tools & Helpers** (Loadouts
Manager, Fast Travel, Loader, Grabber, Current Drops, Junk Items), **Combat** (Skills, Stats,
Runtime), **Bank** (direct action), **Diagnostics** (Logs, Console, Spammer, Logger,
Interceptor), **Plugins** (View Plugins, Butler, Cosmetics), **Army** (Accounts, Army Control, New Tab; Turu's:
VibeSkua has them in its Manager app and its tabbed host). Right side: **Auto**, **Jump**, a
divider, **+**.

With two tabs or more, a 30 px tab strip (`tab_strip.rs`) sits above the nav bar
(`gui::top_height`): rounded tabs with a status dot (a yellow "!" badge while the tab's script
waits for you), the name and ×, the active one in the panel colour with an accent line on
top, and + at the end.

| Item | State |
|---|---|
| Script Loader, Game, Application, CoreBots, Loader, Grabber, Current Drops, Skills, Stats, Runtime, Logs, Console, Logger | implemented |
| Bank | implemented: `bank.open` through `spawn_action` |
| Auto popup | Auto Attack and Stop Automation implemented, as VibeSkua's `AutoViewModel`/`ScriptAuto`: the class drop-down lists the Class items you own (re-read whenever it opens, which also clears the selection); picking one equips it and lists its saved modes (first one selected); Auto Attack uses that class and mode, or the equipped class in Base when either is unset; it turns Infinite Range on, sets the spawn point and attacks the Manual MapIDs in priority order, else your current target, else any living monster. Auto Hunt is disabled. |
| Jump popup | implemented, as VibeSkua's `JumpViewModel`: picking a cell jumps at once ("Enter" uses pad Spawn, otherwise the chosen pad or Left); the cell list is refreshed when the popup or the cell drop-down opens; pads are VibeSkua's list (Spawn, Center, Left, Right, Up, Down, Top, Bottom); Current selects your cell and pad without jumping; Jump re-jumps to the selection. Jumps need a login and don't auto-correct the pad (`map.jump(cell, pad, false)`); the result shows under the buttons. Arrow-key cell stepping isn't ported. |
| + | opens the Script Loader (VibeSkua opens the tabbed tool host) |
| everything else | greyed, tooltip "Coming soon" |

**Auto Attack** (`auto.rs`): a bot thread marked as a script thread, so `bot.control.request_stop()`
(Stop Automation) makes the next call fail. It runs the steps in the Auto popup row above. It is
disabled while a script or a Console snippet runs, and Start Script is disabled while Auto Attack
runs.

## Windows

* **Script Loader** (`script_loader.rs`, 370x500): header card ("LOADED SCRIPT", file name in
  monospace or "No script loaded", status pill with a coloured dot and `ScriptStatus::label()`);
  Load Script (rfd dialog filtered to .ts/.js, starting in the repo's `scripts/` found next to the
  working directory or above the executable, else the bot's scripts folder); Open VSCode (enabled
  when `code` is on PATH, or the macOS app bundle is found); Edit Script (OS default app);
  Search Scripts (opens the window below); Edit Script Options (enabled once a script with options is loaded);
  Start/Stop Script (primary/danger; "Force
  Stop" while stopping handlers run); Pause/Resume; SCRIPT LOG with count badge, Copy/Save/Clear, following the end, selectable text.
  The "Report Issues" footer is omitted.
* **Search Scripts** (`script_repo.rs`, 969x500; VibeSkua's Script Repo; also Scripts > Search
  Scripts): search box ("Search scripts by name, path or tag") with a clear button, scope (All,
  Name, Tag, Description, File), category (All, Pinned, Local and the top folders), sort (Name or
  Date added, up or down); a count line; the list (without the libraries such as CoreBots: files
  with no `export default`, which VibeSkua lists), pinned first, then the sort, then the search
  rank (VibeSkua's `SearchRank`). Each row: star when pinned, name, badges (Downloaded, Update,
  Modified, Local, Removed), path, description, tag chips, Start (primary), Load and a "…" menu
  with Pin/Unpin, Download / Update / "Replace with the repository's", Open in VS Code and Delete.
  Start and Load download the script and what it imports first, then load it in the active tab.
  Empty states "Loading scripts..." and "No scripts found" (with Clear search). Bottom bar: Update
  all, Download all, refresh, a "…" menu (Reset Scripts..., which asks first; Open the scripts
  folder; the same in VS Code), and on the right the running task with its progress bar and
  Cancel, or the last result with the index's commit and when it was checked. Not built:
  Queue (no Scheduler), "Update dates from Git" (the index has the dates), custom script folders.
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
  uses VibeSkua's fallback list (plus the current value); Set FPS defaults to 0, shown as
  "Game's", which leaves the frame rate to the game's own FPS setting (VibeSkua: 30, forced at
  every login), and Headless Mode restores the rate the game had before it turned on.
  While Headless Mode is on in game, the game area shows VibeSkua's headless overlay ("Headless
  Mode Active", `show_headless_screen` in `mod.rs`) instead of the 2 FPS game: the movie isn't
  drawn and the overlay takes the game's clicks; the bar and tool windows work as usual.
* **Application Options** (`application_options.rs`, 580x650, VibeSkua `ApplicationOptionsView`):
  search box, chips All/General/Updates/Webhooks with counts, each setting with its description.
  Changes are saved at once (`settings.json`, `AppSettings`). General: Show Username in Title
  (the window title gets " – <character>" while you play), Client Animation Frame-rate (the
  `stage.frameRate` set when the client loads, default 24; the game's own FPS setting, or Set FPS, takes over in game; VibeSkua
  stores it and never reads it), Relogin Try Delay and Relogin Watchdog (bot options: applied
  and saved into `options.json` on their own), Clear Flash Cache (deletes Ruffle's SharedObjects
  folder after a second click, then restart). Updates: the six switches are stored but greyed
  ("Coming soon"): Turu downloads no updates. Webhooks: the Discord URL with Test, the notify
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
* **Accounts** (`accounts.rs`): add / edit form (Username, Password with Show, Display Name, Tags;
  the password goes to the system keychain off the UI thread), the list (check box, a green dot
  while it runs, tags, Start, Edit, up / down, Remove asking twice), a tag filter, Select /
  Unselect All, Add Selected to Group and Select group, and Launch: Server (the same drop-down
  as Relogin Server, `widgets::server_combo`), where (In a tab, In its own window, Without a
  window), the script (Browse / Clear) with Auto-start, Start Selected, Start All. Saved on every change.
* **Army Control** (`army.rs`, 560x620): one list, CLIENTS, of this window's tabs and the
  clients Accounts launched (from the web monitor's hub, which the Army poll feeds). Each row: a
  check box, a dot, the name ("tab" or "no window" pill), where it is (long text cut off, whole
  on hover), and below it its own buttons: ▶ Start / ■ Stop (its loaded script, shown on the
  right), Log in / Log out, Gear, Show (tabs), and ⋯ (Relog; launched clients also Logs…,
  Restart and Close). An ended client shows Restart and Remove. The header selects All, None or
  an Army > Accounts group's rows (Group ▾), and its ⋯ has Clear ended and Close all launched
  clients. ACTIONS go to the checked rows, or to every row when none is checked (it says which,
  in yellow for everyone): Start script, Stop script, Load script…, Log in, Log out, Relog, Go
  to house; Travel (join a map at "cell[, pad]", go to a player), Quest (accept with an optional
  reward) and Misc options fold away, closed at first. Stop script and Log out sent to
  everyone need a second click ("Log out all?") within 4 s. The last outcome shows below, with
  the earlier ones under History. VibeSkua's "Include this window's tabs" is gone: tabs are rows.
* **Gear** (`gear.rs`, 460x440, one per client, from a row's ⋯ menu): online or offline, the
  running script, "Bank as of …" (or "Bank not read yet") with Load bank and Refresh; per
  slot (class, weapon, armor, helm, cape, pet, necklace, ground) what's worn and a drop-down of
  what could be (inventory first, then bank items, marked) with Equip; the last outcome
  (`EquipReport::summary`). It reads the account's `gear/<username>.json`, so it shows an
  offline account's last gear. While a script runs, Equip and Load bank are disabled until
  "Stop the script first" is on (off each time the window opens); the script then stays stopped.
* **Logs** (`client_logs.rs`, one per launched client): its script and debug logs, read from
  its control API every 2 s while open, with Copy / Save / Clear.
* **Butler** (`butler.rs`, 520x560; VibeSkua ButlerV5's roster and settings windows, one per
  tab): this account and whom it follows or summons (Stop following, Release all, the last
  action's outcome); ACCOUNTS, the other online accounts read from the sync files every second,
  with chips (fighting, following me / following X / summoned, busy: script, parking, other
  server), map-room, server and class, and Follow/Stop and Summon/Release, disabled with
  V5's reasons; the online count, Summon all and Settings. Settings for every client (Enable,
  passive attack, leech mode, always goto, goto delay, rescue threshold, park location) and
  for this account (class type, obey summons), saved on change. Following stops the tab's Auto
  Attack.
* **Cosmetics & Outfits** (`cosmetics.rs`, 900x620; VibeSkua's Cosmetics plugin, one per tab,
  the catalog shared): a quick bar card of two aligned rows (the room player list and Copy
  Player, then Copy Target Look; on the right M / F / Swap gender. Below: a character name box
  and Copy Page, which copies from its character page (Enter works too); the last action's
  outcome; Reset to Real Gear on the right), then the tab chips with the catalog's state on the
  right. Every control is 28 px high. The Catalog, Outfits and CC tabs split into a list that
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
* **Logs** (`logs.rs`): Script / Debug tabs, Copy/Save/Clear. **Logger** (`packet_logger.rs`):
  Start/Stop Logging turns `bot.logs.packet_logging` on or off; while it is on the engine keeps
  the outgoing packets in `bot.logs.packets` (1000 lines), search box, count, Copy/Save/Clear. Not built: the filter button.
* **Console** (`console.rs`, 700x520, VibeSkua's `ConsoleView`, one per tab): header card "TS
  SNIPPET" ("runs with `bot` in scope"), a monospace editor (default `bot.log("Test");`), Clear and
  Run (Ctrl/Cmd+Enter too), Stop while it runs; OUTPUT below with Copy/Clear: "› " and the
  snippet's first line, its `bot.log` lines as they come, then its result (the value of the last
  expression, as JSON; `undefined` when there is none) in green, its error with the stack in red,
  or "Stopped". The engine side is `script::run_snippet` (`bot/src/script/mod.rs`): oxc strips the
  types (as a script, not a module, so `eval` gives the last value), QuickJS runs it with the
  "turu" module's exports (`bot`, ...) as globals, on a `spawn_action` thread marked as a script
  thread, so Stop (`script::stop_snippet`) ends a `while (true)` loop; `import` isn't available.
  Its `bot.log` lines also go to the script log. Run is disabled while a script or Auto Attack
  runs, and Start Script (Script Loader, Search Scripts) and Auto Attack while a snippet runs (the
  engine also refuses either way). VibeSkua shows no output, can't stop a snippet and lets it run
  beside a script.
* **Stats** (`stats.rs`): script clock (keeps its last value after the script ends), KILLS,
  DEATHS, PICKUPS, ACCEPTED, COMPLETED, RE-LOGINS, Reset. The STORAGE section is not built.
* **Runtime** (`runtime.rs`, 955x596, VibeSkua `RuntimeHelpersView`): two columns. DROPS HELPER:
  the pickup list (`bot.drops`: names, then IDs), multi-select with Remove Selected; "Drop Name
  (e.g. Unidentified 13)" with Add Drop (`|`-separated, a number is an item ID; Enter adds too);
  Interval (ms) (`drops.interval`, waited on top of the loop's 1 s tick), Reject Else, Accept AC
  Drops (the game option); Pick Drops starts or stops the pickup loop (accent while it runs).
  Notifications (VibeSkua's Notify Drop beeper) is greyed. QUESTS HELPER: the registered quests
  ("1234 (Reward: 99)"), Remove Selected, "Quest ID" (several with `,` `|` or spaces) and "Reward
  ID (opt)" with Add Quest (`bot.quests.register`, which also starts the pickup loop). BOOSTS
  HELPER: Search Inventory / Search Bank (`boosts.setAllIds` through `spawn_action`), the four
  boosts with their switch and "ID: n", Use Boosts starting or stopping the timer. The lists
  and switches are the bot's own state, so a script's changes show at once; what you set here
  lasts until the script ends (its end stops the loops, turns the boosts off and unregisters the
  quests, as before).
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
  IDs), QUESTS (Quests, GetMap Item IDs: greyed, VibeSkua decompiles the map SWF with FFDec),
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
  (exactly one) / Unregister All, Inventory Equip / Sell / Sell All (one item, not equipped; Sell
  asks the quantity) / To Bank, House To Bank, Bank To Inventory, Cell Monsters Kill, Map
  Monsters Kill / Teleport To. Kill is disabled while a script, a Console snippet or Auto Attack
  runs. The readers, texts, checks and actions are `turu_bot::grabber`.
* **Current Drops** (`current_drops.rs`): search, AC count, AC only, checkable list, Pick up all
  (`drops.pickupAll`) and Pick up selected (`drops.pickup` with the item IDs), through `spawn_action`.

Not built at all (greyed in the menus): Scheduler, Application Themes,
HotKeys, Loadouts Manager, Fast Travel, Junk Items, Spammer,
Interceptor, Plugins, Auto Hunt, Notify Drop, Grabber's GetMap Item IDs.

## Adding a window

Add `foo.rs` with a struct and `show(&mut self, ctx, &mut bool)`, build the window with
`super::window(ctx, "Title", [w, h], slot)` (lists that fill the window take
`ui.available_height()` minus the footer, with a small floor), add it to the
`windows_fit_small_screens` test in `mod.rs` when it needs no file picker, add a flag to `Windows`, a field and a call in
`TuruUi::show_windows`, and enable the entry in `nav_bar.rs`.
