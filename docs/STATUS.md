# Turu status

What works, what is missing, and the things that trip up work on Turu. Keep it current when
something lands or a gap closes. Read `CLAUDE.md` and `docs/ARCHITECTURE.md` first.

## What works

- **App:** `app/` on ruffle-aqw (`ruffle/`: Ruffle `fda41800` + Mugetsu 1.6.0's patches) starts
  Turu's host SWF, `turu.swf`, which loads AQW (docs/HOST_SWF_PROPOSAL.md). You log in with the
  game's own screen.
  On the Ruffle builds the bot's packets come from the player's socket (R5).
- **Engine (`bot/`):** every `call("x.y")` in `bot/js/turu.ts` has a Rust implementation;
  background loops for drops, registered quests, skills and boosts; Stop and Pause.
- **Servers and auto-relogin:** `bot.servers` (VibeSkua `Bot.Servers`: server list, login,
  logout, connect, `relogin`, `ensureRelogin`) in `bot/src/api/servers.rs`. With Auto Relogin on,
  a server logout, a kick, a lost connection or an unexpected disconnect (the last two need
  Options > Relogin Watchdog) stops the script, logs back in and restarts the script. The login
  is the last one the bot made with a username and password (`--user`, `bot.army.login`,
  `bot.servers.login(user, password)`), else the game's remembered one (the login screen's
  account, which is why an army login has to be kept: relogging with the game's took the tab
  back to that account); it is forgotten when another account logs in. A loading screen stuck
  for `LoadTimeout` reloads the map or relogs (logged out, only once per message: a login that
  never finished left "Authenticating Account Info..." up, and VibeSkua's rule logged out again
  every `LoadTimeout`).
  Logging out yourself (the game's Logout or Change Server, `bot.servers.logout()`) disarms it.
  CoreBots' `relogin()` uses it.
- **Command-line login** (VibeSkua's flags, long forms only, as Ruffle has `-p` and `-s`):
  `turu --user <name> --password <password> --server Artix --run-script farm/GoldFarm.ts`
  logs in once the game has loaded and then starts the script. `TURU_PASSWORD` can replace
  `--password` (a command line is visible to other users). `--load-script` only loads,
  `--exit-on-script-stop` logs out and quits when the script stops (not for auto-relogin).
  `--run-playlist <file>` / `--load-playlist <file>` do the same with a Scheduler playlist
  (below), and `--exit-on-script-stop` then waits for the playlist to end. Scripts given there
  or in the playlist are absolute, or relative to the scripts folder (else the working folder),
  `.ts` optional; one in Search Scripts' index is downloaded (or updated) with what it imports
  before the game starts (`configure_startup`), so an empty data folder works. Checked: a
  windowless client with an empty data folder downloaded a playlist's script and loaded it.
  `bot/src/startup.rs`, flags in `frontend/src/turu.rs`.
- **Client updates** (`bot/src/updates.rs`, gap #33; VibeSkua's Manager uses Velopack, which
  only updates installed apps): at launch and every 6 hours a thread reads Turu's GitHub releases
  (`TURU_RELEASES_URL` overrides the URL), skips drafts and, unless Include Pre-releases is on or
  this build is itself a pre-release, pre-releases, and compares versions with semver against
  `TURU_VERSION` (the tag the release workflow builds from; local builds: the crate's 0.2.0,
  which is above every 0.2.0 alpha, so they're offered nothing). A newer one is logged once and
  shown in the nav bar ("Update <version>"), the control API's `/status` and the web monitor's
  summary. Download & restart (desktops) downloads this platform's package, checks its SHA-256
  against the release's `SHA256SUMS.txt`, then on Windows and Linux swaps the running program
  with `self_replace` and starts it with the same arguments, and on macOS copies `Turu.app` next
  to the running bundle with `ditto`, swaps them by renames and starts it with `open -n`. Update
  when I quit installs it as the window closes; `--headless` exits with status 75 for its service
  to restart it (`POST /update`). Army-launched clients don't check. Android only opens the APK.
  Checked (macOS, debug build, a local release feed): a windowless client found the release,
  `/status` showed it, `POST /update` installed it into a copy of `Turu.app` and exited 75; the
  nav bar showed "Update 9.9.9" in the window. Windows and Linux installs are unit-tested
  (unpacking) but haven't run on those systems; Download & restart in the window wasn't clicked.
  **Trust:** `SHA256SUMS.txt` only proves the package arrived intact, not who made it: whoever
  can publish a release on `muktiarafi/turu` (or take over the account) can publish matching
  sums. The next step is to sign `SHA256SUMS.txt` with minisign in `release.yml` and check it
  with a public key built into Turu. Turu replacing itself also needs write access to its own
  folder: a copy in `/opt` owned by root, run by another user, fails with the reason shown.
- **Windowless mode** (`--headless`, `app/src/headless.rs` on `turu_frontend::engine`): no
  window, GPU or sound; the player draws nothing (Ruffle's null renderer) and a loop on the main
  thread runs it, its
  network tasks and the bot's calls. With the login flags and `--run-script` it farms on its
  own, logging to the terminal; it needs no display (a Linux server). At the login screen
  (debug build, macOS): ~6% CPU and ~96 MB against ~21% and ~420 MB with the window. Device
  fonts aren't loaded, so text in them isn't laid out (reading a text field still works).
  Memory in game (2026-11-06, debug build, macOS, `malloc_history`): ~290 MB of Rust heap, a
  third of it vector shapes parsed as each SWF loaded, drawn or not. ruffle-aqw now reads only
  a DefineShape's header then and parses the shape when it's first drawn or hit-tested
  (`GraphicShared::shape`), and trims parsed shapes' record vectors (login screen: 1331 shapes
  parsed -> 5, heap 77 -> 66 MB; in game 290 -> ~180 MB). Then, from a second profile: font
  glyphs are parsed when first used too (`Glyph::from_swf_data`, ~15 MB), a shape's
  tessellation cache is made on its first draw (~8 MB), the log writers queue 8192 lines, not
  128 000 (~8 MB), and Search Scripts' rows share the index's `ScriptInfo`s (login screen: heap
  66 -> 48 MB). What's left is mostly the game's own: AS3 and display objects, classes and
  methods (~80 MB), SWF data (~20 MB), images it loaded (~10 MB). In `malloc_history`, thread
  stacks and macOS's ICU data file (~80 MB) are reserved or shared, not heap.
  Growth over time (2026-11-08, `tools/memrun/run.sh`, NSoD for 2 h): flat while hopping maps,
  but at Primarch, dying every ~15 s, the footprint grew ~185 MB/h. The `memory census:` line
  (headless, with `TURU_PERF_LOG`: the URLs with the most live movies) showed only
  `DeathAds/*.jpg`: the game loads an ad on each death and none was freed. With
  `TURU_RETAIN_URL=DeathAds/` (a retention report for those movies' `LoaderInfo`s) the chain was
  the GC root itself: Ruffle's `LoadManager` never removed a finished AVM2 image load (its
  removal check only handles movie clips), keeping each `Loader`, `LoaderInfo` and image, ~1 MB
  a death. Upstream Ruffle has the same check. ruffle-aqw now removes such loads once they
  succeed, and its library cleanup also tests images (it skipped them as AVM1). Re-measured
  (30 min at Primarch, ads on, ~4 deaths a minute): live movies stayed at ~50, freed rose with
  each death, 1-2 death-ad `LoaderInfo`s alive at a time, footprint flat at ~250-275 MB
  (before: +185-225 MB/h). Disable Death Ads is also on by default (VibeSkua: off), though an
  `options.json` saved before keeps its `false`.
  Monster files piled up too (2026-11-09): one live copy of each of a map's monsters per join,
  x25 after 30 min of FarmerJoe's Blacksmithing loop (Greenguard East/West), +170 MB/h of heap.
  AQW loads a map's monsters into one shared `ApplicationDomain`, which each of their libraries
  holds and which exports all their classes, so the library cleanup, testing one library at a
  time, always found each kept in use by the others. ruffle-aqw now tests libraries sharing a
  domain together (`Library::sweep_movie_libraries`). Re-measured over 50 joins: live movies flat
  at 21-29 (before: 29 -> 123 by the 40th join), heap flat (`docs/HEADLESS_MONITORING_HANDOFF.md`).
  The allocator (mimalloc v3) kept ~170 MB resident beyond the live Rust heap, which its own
  purging never returned: purging at once (`MIMALLOC_PURGE_DELAY=0`) changed nothing, but a
  forced `mi_collect(true)` once a minute cut RSS from ~340 to ~250 MB at ~4 ms a call (debug
  build). The desktop app now does that once a minute on the thread that runs the players
  (`release_free_memory`; `TURU_MI_COLLECT=0` turns it off). macOS's footprint barely moved
  (~260 -> ~254 MB): it already left out those pages. Not measured on Linux or Windows, where
  RSS / the working set is what counts. Android uses its own allocator (scudo), not mimalloc.
  The Headless Mode option is separate: it lowers the game's frame rate in either mode, and
  with a window it covers the game with VibeSkua's overlay and stops drawing it.
- **Control API** (`--api`, or `--api=<address>`; default `127.0.0.1:9021`): HTTP routes for
  status, loading/starting/stopping/pausing scripts, logs (`since` paging), the bot options,
  login, relogin and logout (`bot/src/control_api.rs` lists them, with what each status means).
  One table of routes answers each method and gives a wrong one 405 with `Allow`. Commands
  answer with the state they changed; failures carry a stable `code` (a test keeps the module's
  table of codes complete), and flags take `1` or `true`. Game failures answer by kind: 503 when the game isn't running, 504 when it didn't answer, 409 when not
  logged in or a login is under way (`/login` and `/relogin` refuse to overlap an auto-relogin,
  but cancel one that only waits to retry).
  `GET /events` streams server-sent events (the script's state, new log lines, monitor events)
  from the bot's state, without game calls, so clients needn't poll `/status`. The routes act on
  the first tab; `/tabs` lists them and `/tabs/<id>/<route>` reaches one (ids as the web
  monitor's, `tab:<id>`). `/scheduler/load` and `/scheduler/save` keep `path` inside the data
  folder (relative paths are under `playlists/`; 403 otherwise).
  `/status` also tells whether the Scheduler's playlist runs (`script.scheduler`), and
  `/scheduler` edits, starts and stops it (`/script/stop` moves the playlist on;
  `/scheduler/stop` stops it). `/butler` summons, releases and follows (Butler without a
  window: `butler::summon` and `release` make the window's Summon checks).
  Every request needs the token (`Authorization: Bearer`, 32 characters at least):
  `TURU_API_TOKEN`, else the data folder's `api-token`, made on first use (64 characters) and
  readable by you only; a shorter one there is replaced. Works with
  or without the window. Example:
  `curl -H "Authorization: Bearer $(cat ~/Library/Application\ Support/Turu/api-token)" localhost:9021/status`.
  Army Control also uses `/map/join`, `/player/goto`, `/map/house`, `/quest/register`,
  `/game/options`, the gear routes `/gear`, `/equip` and `/bank/load` (below), and
  `/auto/attack` and `/auto/stop` (Start/Stop Auto-Attack; `/auto/hunt` starts Auto Hunt;
  `/status` and the snapshot carry `auto`).
- **Several accounts** (Army > Accounts and Army > Army Control; `bot/src/accounts.rs`,
  `bot/src/army.rs`): the account list and groups (VibeSkua Manager's Accounts tab) in
  `accounts.json`, with the passwords in the system keychain (`keyring`: Keychain, Credential
  Manager, Secret Service) instead of in the clear. Start launches the accounts 500-900 ms
  apart, by default each in a **tab** of this window (below), else ("In its own window",
  "Without a window") in a `turu` process of its own, as VibeSkua does: `--user`, `--server`,
  the script, optionally `--headless`, the password in `TURU_PASSWORD`, the control API on a
  free local port with its own token, and `--exit-with-parent` (the client quits when the
  launcher's stdin pipe closes, even if the launcher crashed). Army Control polls each process's
  `/status` every 2 s and sends VibeSkua's Army Control commands to all of them, and to every
  tab, or only to the checked rows (a group's with Group ▾); each row has its own Start/Stop,
  Log in/out, Gear and a ⋯ menu (Relog, Start/Stop Auto-Attack, Logs, Restart, Close).
  `docs/UI.md` has the layout.
- **Gear from the launcher** (`bot/src/gear.rs`, `docs/ARMY_EQUIPMENT_PROPOSAL.md` phase
  1): every client keeps `gear/<username>.json` (its wearable items, the bank as last loaded
  with its date; the bank is never loaded just for it), and the control API's `/equip` puts an
  item on, out of the bank if needed (loading it when it wasn't this login), with an
  `EquipReport`. `/equip` and `/bank/load` refuse while a script runs unless `stop=1`, which
  stops it and leaves it stopped. Army Control's Gear window uses them, for a client without a
  window too. Tested against a fake game (`bot/tests/gear.rs`); unverified in game: the `sES`
  slot codes, the bank opening in a client without a window, and the equip timings.
- **Tabs** (`frontend/src/turu.rs`, `player.rs`, `gui/turu/tab_strip.rs`): each tab is a game
  client of its own, a Ruffle player and its bot, in one window and one process (VibeSkua's
  tabbed host embeds one process's window per tab, which macOS and Wayland don't allow). All
  the players run on the event loop thread and are ticked every loop; only the active one is
  drawn and heard (the others are muted). Hidden tabs that are in game run at 2 FPS, as
  VibeSkua's tabbed host throttles them (`WM_SKUA_THROTTLE`), and get their rate back when
  shown (`state.hidden_tab`, applied with Headless Mode in `api/options.rs`; tabs at the login
  screen keep full speed, so logins aren't slowed). Unlike VibeSkua they keep their quality: an
  undrawn tab gains nothing from LOW, and going back to HIGH re-tessellated every shape on each
  switch (only Headless Mode sets LOW). Each player keeps its own copy of Ruffle's thread-local
  state (`ruffle_core::PlayerThreadState`: pending shape tessellations, replay's change count,
  frame interpolation's frame) and its renderer's command segments not submitted yet
  (`ruffle_render_wgpu::segments`), swapped in by `PlayerController` while a hidden tab's code
  runs (ticks, Flash calls, futures), so hidden tabs neither spoil the shown tab's replays and
  interpolation nor leave tessellations only their renderer can finish, and no tab submits
  another's segments (which crashed with seven tabs: "StagingBelt staging buffer is still
  mapped"). A tab keeps how far it got since its last cut, not the pass count at that cut
  (counted for every tab), else a hidden tab cut between nearly every pass.
- **Metal command buffers:** on Metal, wgpu makes a command buffer of every render pass when a
  frame's encoders are finished, and they count against the queue's 4096 until submitted.
  Crowded frames with skill animations on (2000+ passes) ran out in one submit, which wgpu
  treats as a lost device ("the graphics driver stopped working"). `segments::finish_all`
  submits a frame's segments 16 at a time (ruffle-aqw; Mugetsu's D3D12 merges them instead). Hidden tabs' players
  are ticked only when a frame or timer is due, or every 50 ms for their sockets
  (`PlayerController::tick_hidden`), not on every pass of the event loop, and only the active
  tab has the mouse (`give_mouse`, on every tab change): Ruffle hit-tests the whole stage on
  every tick of a player that thinks the mouse is over it, which was 22% of the main thread with
  seven tabs, and a tab shown again after the cursor left the window ignored clicks. Flash calls name their tab, and `turu::bot()` is the
  active tab's bot, so the UI and `spawn_action` act on the tab you look at; each tab keeps its
  own tool windows, Auto Attack/Hunt and nav bar state (Accounts and Army Control are shared). The
  tab strip shows above the nav bar once there are two tabs: the name (character, else login),
  a dot while its script or Auto Attack/Hunt runs (a yellow "!" while its script waits for its
  Script Options window or a message box, which show when you open that tab; Army Control
  says so too), × to close (stops its script), + for a new tab at the login screen (also
  Army > New Tab). Checked by hand: opening, switching and closing a second tab at the login
  screen, and the "!" for a message box in a hidden tab; the throttling only in tests.
- **Butler** (Plugins > Butler, `bot/src/butler/`, VibeSkua's ButlerV5 plugin): each client writes
  its account's state to `butler/<username>.state` every 500 ms (when it changed, else every
  5 s as a heartbeat; 15 s old is offline, as tabs share a PID). A master summons accounts by
  listing them (`butler::toggle_follower`, `summon_all`, `release_all`); a listed account
  follows it unless a script runs, it declined, or Obey Summons is off for it
  (`butler.json`, per account in Turu). The follower does V5's goto, the rescue join of the
  master's `map-room` (houses by name), the cell jump, fighting (passive and leech modes),
  the packet warnings (locked zone, room full, ignoring goto), the CoreBots class type,
  Infinite Range and Skip Cutscenes while following, relogin re-arm, suspending while a script
  runs (with V5's question: Yes stays suspended, No stops following; a modal in the tab and
  its "!" in the tab strip), and parking (house, whitemap, stay). While following, on its own
  client only (`butler/quest_bypass.rs`, `antilag.rs`): V5's per-map quest bypass table (64
  entries; faked through `quests::update_quest` on arming, on each new map and for the master's
  map before travelling there), the custom quest IDs, doomvaultb's achievement, Unlock All
  Quests (maxes the `strQuests` slot strings; wins over the bypasses), the level 100 fake, and
  AntiLag when summoned or following by hand (the nine settings are saved first and put back
  when the follow ends; re-applied after a relogin, and after a suspending script, whose stop
  turns Lag Killer off: V5 leaves it off). Every component logs to an in-memory
  debug log (`butler/debug_log.rs`, also to the debug log as before), shown in the window with
  Copy, Save (`butler_logs/`), Clear and Open folder, optionally auto-saved. A closing tab and
  a quitting app (window, `--exit-with-parent`, `--exit-on-script-stop`, the graphics restart)
  write the account offline at once (`butler::close`). Send to player
  (`butler::send_all_to_player`, V5's Send All): the client `/goto`s an outside player itself
  and puts a one-shot goto for every available account (the Summon all filter) in its sync file;
  a recipient obeys it once, within 60 s of its write, unless Obey Summons is off, a script runs
  or the sender is on another server; the names go in `butler_player_targets.json` (20 recent).
  Copy master's walking: in the master's room and cell the follower `walkTo`s the master's
  position (V5's 10 px dead zone, Goto delay throttle), Always goto only when the position is
  unknown. Follow master's quests (`butler/quest_sync.rs`): a master writes its active and
  registered quests and rewards to `butler/<username>.quests` every 2 s; a butler registers
  them (`bot.quests`), drops only its own registrations when the master does, and on a script
  start, park, setting off or the follow's end; a relogin applies the list again. Without a window a follower works as it
  is (the suspend question stays unanswered: it stays suspended), and a master uses the control
  API's `/butler` routes; Butler only sees clients sharing its data folder. Its tests run a master and a butler
  against two fake games. The window (`docs/UI.md`) lists the other online accounts with
  Follow and Summon, and edits the options; following stops the tab's Auto Attack/Hunt. Not built:
  quest cloning (V5 copies the master's quest slots; Follow master's quests is built). The quest fakes, the level fake, AntiLag
  and the question are unverified in game.
- **Cosmetics** (Plugins > Cosmetics, `bot/src/cosmetics/`, VibeSkua's Cosmetics plugin): shows
  any item on your avatar without owning it, client side only (`objData.eqp`, `loadMovieAtES`,
  the colors, a `genderSwap` packet to the client), from VibeSkua's 67,000-item community catalog
  (`cosmetics/catalog.json`, downloaded on first open); copies the targeted or a named room
  player's gear, gender and colors, empties the slots they have nothing in; Copy Page (under
  the character name box) does the same for any character by name, offline or elsewhere, from the `FlashVars` of
  its `account.aq.com/CharPage?id=<name>` page (`cosmetics/charpage.rs`; the page's hair isn't
  applied). Reset to Real
  Gear puts the inventory's gear, gender and colors back. Favorites and saved outfits are shared
  by the tabs. Differences: the look's remembered colors and gender are forgotten at logout (a
  tab may log into another character); VibeSkua's `world.CHARS[id]` fallbacks are left out, as
  its `getGameObject` can't read them. Tested against a fake game (`bot/tests/cosmetics.rs`)
  and a saved page; an ignored test reads a real page
  (`cargo test -p turu_bot charpage -- --ignored`);
  unverified in game and the window unverified on screen. Like VibeSkua, the look isn't put
  back after a map change or a respawn.
- **Reputation and auras:** `bot.reputation` (VibeSkua `Bot.Reputation`: `factionList`, `getRank`,
  `hasRank`), `bot.self` and `bot.target` (VibeSkua `Bot.Self`/`Bot.Target`: `auras`, `snapshots`,
  `getAura`, `getAuraStacks`, `hasActiveAura`, `getAuraValue`, `hasAnyActiveAura`, ...), in
  `bot/src/api/reputation.rs` and `auras.rs`. Skill rules read aura stacks through the same code.
  Unlike VibeSkua, `getRank`/`hasRank` match faction names ignoring case: the game calls one
  faction "EtherStorm", CoreFarms' `etherStormREP` asks for "Etherstorm", and the exact match
  read rank 0, so 0ArchMage farmed waterstorm forever on accounts already at rank 10.
  The same goes for `map.join`'s "already there" check and the drop list's `add`/`remove`, and
  in the core scripts for `Core.hasFaction`, the item-name checks (CoreBots' `sameName`: trimmed,
  any case, `&amp;` read as `&`) and `killMonster`'s cell spelling fix, which C# never applied
  (it tests the case-insensitive match twice).
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
  `farm/GoldFarm.ts`, `farm/CombatTrophy.ts`, `other/various/Potions.ts` and `other/mergeshops/SynderesMerge.ts`.
- **Merge scripts:** every merge outside the Ultras folders (436 more, 455 in all) and the 150
  scripts they include, drafted by the converter `scripts/tools/cs2ts` (tree-sitter C# + the
  TypeScript checker) and fixed by hand where it fell short; the Core members they reach were
  added (`scripts/docs/PORTING.md`, "Merge scripts", lists them and 5 C# bugs fixed on purpose). All
  type-check and load; none has run in game.
- **Story scripts:** every script in the C# `Story/` folder (the 282 left after the merges,
  drafted by cs2ts), with the rest of CoreToD, CoreDoomwood, Core7DD and CoreSummer (28 members).
  Their calls and literal arguments match the C# file by file; one C# bug is fixed on purpose
  (`CoreMemet.zorbasPalace`'s last three quest IDs; `scripts/docs/PORTING.md`, "Story scripts"). All
  type-check and load; none has run in game.
- **Core content areas:** every script in the C# `Nation`, `Legion`, `Hollowborn`, `Darkon`,
  `Good`, `Evil`, `Chaos`, `ShadowsOfWar`, `Farm`, `Dailies` and `Enhancement` folders (the 326
  left, drafted by cs2ts) and 7 scripts outside them that they include, with the rest of
  CoreFarms' reputations, CoreNSOD and 11 CoreNation members (`scripts/docs/PORTING.md`, "Core content
  areas", lists them, the 23 hand-fixed files and 8 C# bugs fixed on purpose). All type-check and
  load; none has run in game.
- **Seasonal scripts:** every script in the C# `Seasonal/` folder (the 235 left, drafted by
  cs2ts) but `CheckForDonatedACs` (Army), with CoreBots' `playerAFK` (`scripts/docs/PORTING.md`,
  "Seasonal", lists the 9 hand-fixed files and 4 C# bugs fixed on purpose). All type-check and
  load; none has run in game.
- **Other scripts:** every script in the C# `Other/` folder (the 187 left, drafted by cs2ts) and
  `prototypes/InfernalArenaLW`, with CoreBots' `ghostItem` and `QuestData.requiredFactionRep`
  (`scripts/docs/PORTING.md`, "Other", lists the 18 hand-fixed files, 4 C# bugs fixed on purpose, and
  the converter fixes; one of them, `Bot.Skills.StartAdvanced("1 | 4")` becoming a rotation
  instead of a class name, also fixed 7 earlier ports). All type-check and load; none has run in
  game.
- **Army and Tools scripts:** `army/CoreArmyLite.ts` on Turu's several accounts, the Army
  scripts (with the Ultras' army sync helpers in `army/CoreArmySync.ts`) and the in-game Tools.
  New engine API `bot.army` (`bot/src/api/army.rs`): a sync log shared by every client of the
  machine (C#'s Army log and sync files), locked per log (`army/.<name>.lock`, held across a
  read-modify-write by `bot.army.withLock`, released at the latest when the script ends), a
  barrier with a round count (`bot.army.barrier`, `army/<name>.barrier`; the army waits use it),
  the Army > Accounts usernames, logging in an account
  with its keychain password (`doForAll`), and the army's private room numbers; and
  `bot.map.getPlayer` / `playerExists` (VibeSkua `TryGetPlayer`). `scripts/docs/PORTING.md` ("Army",
  "Tools") lists what differs from C#, the three Ultra-engine Army scripts that are logged skips,
  the skipped tools and why, and 4 C# bugs fixed on purpose. All type-check and load; none has
  run in game, and the shared log, its lock, the barrier and keychain login are only tested
  without a game (the lock and barrier also from scripts on two clients).
- **Ultras:** everything in `Ultrasv3` but `TestZone` (it runs on the old `Ultras/` engine): the
  whole `DependenciesUltras` engine (CoreEnginev3's skill engine and every region, CoreUltrav3's
  sync, taunts, scrolls, alchemy and house joins, the class syncs, wait-for-army, wipe and death
  sync, taunt pulses, prerequisites), the 11 individual Ultras, the challenge bosses, the dailies,
  the DoAll queues, ArmyPrismatas and EmpoweredNationItem, and Entwined Eclipse on its own older
  CoreEngine/CoreUltra (with `DependenciesMerge/QueenIona`, which `LothianTreasuryMerge` now
  fights, and the Army scripts `ArmyFarmVoidAuras`, `TheEncroachingShadows` and
  `WrongTurnatVoidbuquerque`). C#'s threads and
  `async` work run as generator tasks (`scripts/core/CoreTasks.ts`, with C#'s
  `CancellationTokenSource` and `ManualResetEvent`). The boss scripts are cs2ts drafts fixed until
  they type-check; `scripts/docs/PORTING.md` ("Ultras") has the C# mapping and what differs. Tested:
  the tasks against a fake game and that every Ultrasv3 script loads (`bot/tests/core_tasks.rs`);
  none of it has run in game.
- **FarmerJoe:** `other/farmjoekits/0FarmerJoeDoAll.ts` (VibeSkua's level 1 to 100
  progression) and the 127 content scripts its `DoAll` reaches: classes, weapons, the story cores,
  UnlockForgeEnhancements, the Nation, Legion, SDKA, BLoD and Awe trees, 19 merge scripts.
  `scripts/docs/PORTING.md` ("FarmerJoe") lists what each file leaves out, the 19 included files that
  aren't ported (DoAll never calls them) and the spots that differ from C#.
- **CoreBots like VibeSkua:** reads Options > CoreBots (`CBO_Storage(<user>).txt`) in
  `setOptions()`, starts from your house when you are in battleon, goes to the CBO stop map
  when the script is stopped, and uses boosts from the CBO Boosters switches. Picking
  `[Current]` in Options > CoreBots gives an equipped class with no skill set a default `Base`
  set, saved at once, as VibeSkua's `EnsureSkillEntryExists` does (`turu_bot::skills::ensure_skill_set`).
- **Script options:** a script exports `scriptOptions` (`defineScriptOptions`); Scripts > Edit
  Script Options edits them, saved in `options/<Storage>.cfg` like VibeSkua, and `bot.config.get`
  reads them (`docs/SCRIPTING.md`, "Options"). Start opens the window and waits for it
  unless the script sets `dontPreconfigure`; CoreBots' `setOptions()` opens it until
  `Core.skipOptions` ("Skip this window next time") is ticked, as in VibeSkua.
- **Search Scripts** (`bot/src/repo.rs`, `frontend/src/gui/turu/script_repo.rs`): VibeSkua's
  Script Repo for the scripts in `muktiarafi/turu-scripts` (the `scripts/` submodule): search,
  categories, pins, Start/Load/Queue (downloading the script and what it imports), Update all,
  Download all and Reset Scripts, and the launch-time check and auto update (Options >
  Application). Edited scripts are kept by updates and backed up by a reset.
  `docs/ARCHITECTURE.md` ("Script repository") has the design.
- **Scheduler** (Scripts > Scheduler, `bot/src/scheduler.rs`, `frontend/src/gui/turu/scheduler.rs`,
  VibeSkua `ScriptSchedulerViewModel`): a playlist per tab, run top to bottom without the Script
  Options window, each entry with its own options (`options/Queue_<name>_<id>.cfg`); any end of a
  script moves on, Stop queue stops; a disconnection waits for the login and resumes the entry;
  VibeSkua's playlist files and webhook embeds. Save writes scripts in the scripts folder
  relative to it, and Load finds relative paths there, so a playlist moves between computers.
  Without a window: `--run-playlist` and the control API's `/scheduler` routes (list, add,
  remove, move, rename, clear, load from a file or the request body, save, start, stop).
  Tested against a fake game (`bot/tests/scheduler.rs`: order, errors and missing files, Stop
  Script vs Stop queue, the disconnection, the options, the playlist files, the API routes,
  the command line); not yet in game, nor with a real relogin. Per-entry options stay in the
  data folder (`options/Queue_*.cfg`): a playlist copied to another computer comes without them.
- **Application options and webhooks:** Options > Application (`settings.json`): username in
  the title, the client's starting frame rate (24 by default, AQW's own rate; Set FPS defaults to
  0, leaving the game's own FPS setting alone),
  relogin delay and watchdog, Clear Flash Cache, and Discord webhooks for script
  start, stop and crash, relogins, listed drops and health pings (`bot/src/webhooks.rs`).
  Unverified against a real webhook. The update switches act at launch: script updates
  (Search Scripts) and, with both AdvanceSkill Sets switches on, the community skill sets
  (`auqw/Scripts`' `AdvancedSkills.json`) merged into the user's, new classes and modes only
  (`turu_bot::skills::startup_skill_sets`, also with `--headless`; tested with a fake download).
  The junk-item switches are stored only (VibeSkua never reads them either).
- **Junk Items:** Tools & Helpers > Junk Items (`frontend/src/gui/turu/junk_items.rs`) lists the
  inventory and bank items (filters, quantities, equipped / favourite / near-cap chips, quantity
  goals), marks the selected ones as junk and sells the junk in the inventory;
  scripts call `bot.sellJunk()`. The list is `JunkItems.json` in the data folder, in VibeSkua's
  format (`bot/src/junk.rs`; a VibeSkua file, or `auqw/Scripts`' community list, can be copied
  over). Tested against a fake game (`bot/tests/junk.rs`); not yet sold in game.
- **Web monitor** (`docs/WEB_MONITOR_PROPOSAL.md`, replacing the Skua
  Nexus webhook): Options > Application > Monitor serves a page (`bot/assets/monitor/index.html`)
  on `127.0.0.1:8790`, or on every interface with Allow Other Devices, showing this window's tabs
  and the clients Army > Accounts launched, live: a "Needs you" list first (script errors, frozen
  tabs, logged-out bots, each with its fix), then one row per bot grouped by machine (state, script
  and run time, map, HP / MP, gold per hour, and a trace of the last hour: a dot per two minutes,
  marked at drops, deaths and relogins), its Scripts / Accounts / Resources tables, a filter, bulk
  actions on selected bots and streamer mode. A bot's side panel has its overview (map, level and
  class, gold, AC, bag slots, HP / MP / target, quests, equipment, the Combat > Stats counters,
  gold and XP per hour), its events, its script and debug log (streamed as it's written:
  `/api/bots/<id>/log/stream`, server-sent events read every half second through the hub, so
  tabs, Army clients and other machines alike), and an hour-by-hour history (gold, XP, kills,
  drops; a point a minute, 6 h). `/api/events` sends the state only when it changed, with a
  comment every 15 s; the page keeps the server's clock going between frames.
  **Control** (Allow Control, on by default): Start / Pause / Resume / Stop the loaded script, Log
  in, Relogin, Log out, House and Join map, on one bot or the selected ones (asking first for the
  ones that interrupt), and Forget for offline entries. The hub (`monitor/hub.rs`) keeps every
  entry: tabs from their sampler, Army clients from the Army poll's `GET /snapshot`; it saves to
  `monitor.json` every minute, so after a restart the last characters show offline for a day.
  Army Control reads the clients from the hub. **Other machines** report to it (Options >
  Application > Report, or `--report-to <url>` with `TURU_MONITOR_PUSH_KEY`; `monitor/push.rs`):
  their tabs and Army clients show under the machine's name and take the same commands, which
  ride back in the reply to the next report (the monitor holds it up to 4 s, so they land at
  once; their logs too). Reports need the monitor's push key (`monitor-push-key`, Copy Report
  Settings), which opens nothing else; Accept Commands on the reporting machine can refuse
  commands. `--monitor[=address]` serves the monitor without the setting. Options shows the
  phone link as a QR code. Each tab's sampler reads a `Snapshot` every 2 s
  only while a page is open (items and quests every 30 s or after they change); Army clients are
  read every 2 s, their slow fields only while a page is open. The webhooks format the same
  `Snapshot`. The `api` routes need the key from `monitor-key` except from a loopback connection
  naming a loopback host (so Tailscale Serve needs it); a command also needs the page's own
  `Origin` or the key. Clients launched by Army don't serve it. Checked: the tests (a real
  Army client reaching the hub is an ignored test, `TURU_EXE=target/debug/turu cargo test -p
  turu_bot --test monitor -- --ignored`), the page against the headless client at the login
  screen, and two headless processes (`--monitor` and `--report-to`): the remote's card, a
  command (about 0.1 s) and its log. The in-game cards, commands and history only with sample
  data; the Options window's Report section and QR code only built, not looked at. Not built:
  browser notifications.
- **UI:** VibeSkua's nav bar and the windows listed in `docs/UI.md` (Script Loader, Scheduler,
  Script Options, Game, Application and CoreBots options, Skills, Stats, Runtime, Notification
  Center, Logs, Console,
  Spammer, Logger, Interceptor, Current Drops, Loader, Grabber, Auto, Jump).
  Diagnostics > Spammer and Interceptor are tested against a fake game and fake socket bytes
  (`bot/tests/spammer_interceptor.rs`), not yet in game: the capture's socket wrapper
  (`frontend/src/capture.rs`) on a live connection, inflating the server's compressed packets, an
  edited or blocked packet reaching (or not) the server or the client, Connect's relog.
  Tools > Loader and Grabber are unverified in game: the quest cache (`quest_cache.rs`: VibeSkua's
  `QuestData.json` format, Add missing / Rebuild all / Update range, merging, cancel, the
  community download with its ETag, over a local HTTP server too) and the
  Grabber's readers and actions (`grabber.rs`) are tested against a fake game
  (`bot/tests/loader_grabber.rs`), the windows only opened. GetMap Item IDs' bytecode reader
  (`frontend/src/map_items.rs`) is tested on built SWFs and was checked on Mobius
  (`TURU_MAP_SWF=<file> cargo test -p turu ... real_map -- --nocapture`), where it agrees with the
  story scripts' map items and quests; other maps may need more patterns.
  The Jump popup reads each cell's pads from the map SWF too (`frontend/src/map_pads.rs`: frame labels
  and the named `Pad_<n>` instances on the root timeline, once per map file): picking a
  cell jumps to a pad it has (Spawn for Enter, else the chosen pad, else Left, else its first,
  as VibeSkua's `jumpCorrectRoom` corrects after the fact) and the pad drop-down lists that cell's
  pads. It reads the doors too (the exit components' `tCell`/`tPad`/`altCell`/`altPad`, set in
  Flash's generated `__setProp_...` functions, and the cell each door instance is shown in), so a
  picked cell lands on the pad the walk from the player's cell arrives on (the last door of the
  shortest walk), before the rules above. Checked on Mobius' SWF; not yet in game. Without the
  SWF it falls back to VibeSkua's choice.
  Both readers take the map SWF the game already loaded from Ruffle's library
  (`FlashHost::loaded_swf`, ruffle-aqw's `Player::loaded_movie`) and download it only when it
  isn't loaded.
  Combat > Runtime (VibeSkua's drops, quests and boosts helpers) is unverified in game; so is its
  Notifications window (Notify Drop: the list and its subscription are tested, the beep only
  built, not heard on Windows, Linux or Android). Combat > Skills' editor is unverified on
  screen: its model and file writing are tested, the window only built.
- **Perf counters** (`bot/src/perf.rs`): every Flash call is counted by the thread that made it
  and its callback (and path, for `getGameObject` and its kin), with how long the caller waited
  and how long the player ran it (timed in `answer_flash_call`); each tab's player ticks too.
  Read with `GET /debug/perf`, or logged per tab with `TURU_PERF_LOG=<seconds>`
  (`docs/HEADLESS.md`). The wait includes the queue on the player's thread, so a wait far above
  the run time means that thread is busy with ticks or other tabs' calls. With a window, the log
  also has a `main thread` line (`app/src/thread_perf.rs`): idle, drawing (the movie and its GPU
  wait, getting the window's next picture, the UI, submit and present), the games and bot calls,
  and the rest. `TURU_GRID_TEST=<fps>` measures what a grid of live tiles (VibeSkua's Grid View)
  would cost: hidden tabs run at `fps` instead of 2 and draw every frame into their own movie view
  (never shown, at the window's size, so drawing costs more than a tile would); the line then
  has the hidden tabs' drawing too, and what asked egui to repaint (by code location).
- **Redraws:** with the window shown, the UI alone redraws at most every 1.5 monitor refreshes
  (16 ms at least: 60/s at 120 Hz), with or without the nav bar (Mugetsu caps it only with the
  menu bar hidden). egui asks for a repaint on every mouse move, so the window presented at the
  monitor's rate, and each present waited for the next refresh: 35-40% of the main thread,
  which every tab's game and bot call waited behind. Nothing asks for a redraw while the window
  is minimized or covered (it spun the event loop, ~450 ms/s).
- **GPU time per frame** (`RUFFLE_GPU_TIMING=1`, ruffle-aqw's `ruffle_render_wgpu::gpu_timing`):
  the perf log's CPU timers say how long recording and submitting a frame took, not how long
  the GPU then ran it. With the variable set, a `gpu time per frame` line every 5 s gives the
  GPU's time for the game's frame (not the UI's or the copy into the window), split into the
  cacheAsBitmap and filter redraws and the stage, with the longest frame, the frames measured,
  and the target's size and MSAA samples. Three timestamps a frame, written by passes that clear
  a 1x1 texture (an empty compute pass wrote nothing on Metal, and Ruffle's device can't
  dispatch compute work) and copied out one frame later (copied in the same command buffer, a
  frame's last timestamp often wasn't written yet on Metal). A span runs from the frame's first
  command to its last, so it also counts the GPU waiting for later command segments. Off by
  default. Checked on macOS (Apple GPU, Metal, debug build, 2026-11-12) at the title screen only,
  1916x1100, 1 sample: 1.4-1.6 ms a frame, against 4-5 ms of `finish_encoding` on the CPU; not
  yet in game, on Windows (D3D12/Vulkan) or on a phone, where the rendering levers of
  `docs/HOST_SWF_PROPOSAL.md` (M7) and the MSAA and resolution caps need these numbers.
- **What the player's frames are made of, and what cuts them** (M7a of
  `docs/HOST_SWF_PROPOSAL.md`, 2026-11-13, macOS, release build, window shown, 30 FPS, each
  lever on and off in turns of 1-2 minutes within one session with `TURU_LEVER_AB`):
  - The perf log's `gotos:` line now counts, per frame, the clips skipped as dormant, the ones
    walked, the walked ones playing, and those awake only because they play. GoldFarm's map:
    ~560 dormant, ~255 walked, ~57 awake only because they play. Idle in Yulgar: ~965 / ~564 /
    ~158 of 182 playing, mostly other players' avatars (`AvatarMC`, up to 27 clips each), an NPC
    and the UI portrait's head.
  - `stopAnimations` (turu.swf: stop the clips of the map's art and of other players whose
    frame moves from one frame to the next; `isPlaying` is false for a timeline that plays by
    itself, as in Flash Player): ~240 clips stopped, playing 182 -> 42, walked ~564 -> ~297,
    **a frame's game logic 0.67 -> 0.26 ms**; drawing 5.07 -> 4.95 ms (no change).
  - No gain: `stripFilters` (~217 filters: render 4.47 -> 4.41 ms), `mouseOff`
    (`mouseChildren` off on 15 layers: frame 1.58 -> 1.49, render 5.65 -> 5.39 ms, within the
    turns' spread; hit tests aren't timed apart), `lowQuality` (render 4.64 -> 4.97 ms),
    `cacheStatic` (`cacheAsBitmap` on 52 map layers: 4.85 -> 4.81 ms).
  - M5's event watcher costs nothing measurable (`eventsOff`: frame 1.40 vs 1.49 ms).
  - ruffle-aqw now logs an uncaught AS3 error the first time its message and innermost frame
    are seen and counts the repeats (`gotos:` line; `RUFFLE_ERRORS_EVERY_TIME` logs each). An
    armour's frame scripts (`ShadedPlagisteGuise`, driven by the aura HUD) threw ~8 a second in
    one debug run whose player ticks were twice another's; the release runs here didn't get
    that stream, so its cost isn't measured.
  - Turu Mas (emulator, one tab, `--perf-log`): AIR holds 24 FPS, 41.7 ms between frames on
    average, ~1% over 50 ms (turu.swf's `frames:` report). Not yet measured with the levers.
  - With a window, drawing (~5 ms a frame) is most of the cost and none of the levers moved
    it; `stopAnimations` cuts the part that runs whether or not anything is drawn (hidden
    tabs, headless, background Turu Mas tabs).
  - M7b tried applying `stopAnimations` automatically where nobody sees what it stops (a tab off
    screen, Headless Mode, Lag Killer; other players with Hide Players) and dropped it: it
    stopped 155-347 clips per off-screen Turu Mas tab, yet four idle tabs used 116% CPU with it
    and 113% without, frame pacing alike; a hidden desktop tab already runs at 2 FPS, so at most
    ~1 ms a second was there to save, and none showed. The lever stays (`Animations.as`), and
    the four that didn't pay went.
  - Not from M7: four Turu Mas tabs idle in Battleon and Yulgar pace unevenly, about a quarter
    of frames over 50 ms at a steady 24 FPS (frames: report), against ~30 a minute per tab
    while farming in houses. Not explained yet.
- **Reads that copied whole objects:** `player::loaded` (asked by the skill timer every tick and
  by many waits) reads `world.myAvatar.items.length`, as VibeSkua does, not the whole inventory;
  `quests.get`/`canComplete` read `world.questTree.<id>`, not the whole quest tree. With nine
  tabs these copies were a large part of the bot calls' time. They led to the
  background timer checking "still in game?" every 100 ms and the loading screen every 250 ms
  while in game (VibeSkua: every 20 ms and 100 ms), out of game every tick as before: those
  checks were about half of all calls. And `isLoggedIn` and the avatar's `intState` (behind
  `alive`, `playing`, `inCombat`, ...) are read at most every 100 ms per client, each read
  shared by every thread (`api::player::shared`): nearly every wait and background loop asks.
  Measured while farming (2026-11-10, 0MaxBags on Artix, release build, a member with 366 kB of
  inventory and 909 kB of bank as JSON): the bot's calls took 47 ms/s of the player's thread,
  the game's frames 26, and 42 of the 47 were whole reads of `world.myAvatar.items` (about 5 a
  second) and `world.bankinfo.items` (1.6 a second), 4 to 10 ms each, nearly all of it in
  `JSON.stringify`. ruffle-aqw's `JSON.stringify` now writes its text directly when it has no
  replacer or indent, instead of building a tree first: 9.7 against 14.2 ms per MB, both in turns
  within one farm; 10,000 texts of that farm matched the tree's byte for byte
  (`RUFFLE_JSON_CHECK=1`); `RUFFLE_JSON_TREE=1` puts the tree back.
  And the item checks (`contains`, `getQuantity`, `getItem`, `isMaxStack`, `isEquipped`, the
  equipped class) no longer read the whole list (VibeSkua does): they read one field of every
  item (skua.swf's `selectArrayObjects`: the names are 10 kB of an inventory's 366) and then
  only the items that match (`getArrayObject`), and fall back to the whole list when a column
  comes back empty or an item isn't the one asked for any more. Since M4 of
  HOST_SWF_PROPOSAL.md, turu.swf's `selectArrayObjectsWhere` sends the matching items in one
  call (`inventory::matching_where`).
  The same farm after it: the bot's calls 47 -> 14 ms/s, the script thread's whole reads gone
  (a column read takes 0.6 ms for the inventory, 2.2 ms for the bank). The gameplay stats'
  sample, once a second, read the whole inventory for the equipped class's ID (~6 ms/s), and
  each drop read it to look the item up: both read narrowly too now, and the same farm's calls
  are at 8 ms/s. `containsMany` and whatever asks for the list itself (the gear file, a
  script's `items`) still read all of it.
- **Hit-tests with the pointer over the game:** Ruffle looked for the hovered object after
  every tick, not every frame, and the app ticks on each wake of its event loop: 190 to 250
  times a second with a bot running (each bot call wakes it), against 30 frames. A hit-test
  walks the stage (0.35 ms on a farm map, about 1 ms in a crowded Yulgar). ruffle-aqw's tick
  leaves it to the frame and to pointer and button events now. Measured in one session with a
  pointer put on the stage and taken off in turns (2026-11-10, release build, window covered),
  game ticks: 0MaxBags farm 49 ms/s with the pointer away, 121 with it over the game, 62 with
  the fix; Yulgar 14, 62 and 20.
- **Game files are cached:** Ruffle's desktop navigator has no HTTP cache (in a browser, Flash
  Player used the browser's), so every start downloaded the game's assets again (8 MB) and
  every map join its map, monsters and gear. AQW's CDN marks those files `Cache-Control:
  public, max-age=2678400` (31 days) with an `ETag`. ruffle-aqw's navigator now keeps them
  (`HttpCache`, crate `ruffle_http_cache` in ruffle-aqw's `http-cache/`; Turu's one cache per process:
  `turu_frontend::game_files`, in `<data>/cache/gamefiles`, shared by every tab and client) by
  a browser's rules: only a plain `GET` answered `200` with a `max-age` of an hour or more and
  a validator, no `Set-Cookie` unless it's `public` (Cloudflare's `__cflb` comes on a client's
  first request), no `Vary` beyond `Accept-Encoding` (the game's API answers,
  `max-age=2` and `Vary: *`, never are). A file is one entry whatever its query string: asked
  for with the query it was stored with, while fresh, it comes from disk with no request; with
  another query (`?v=5.32` after a game update, the host SWF's `?ver=<random>` on the game client)
  or once stale, the server is asked with `If-None-Match` and answers `304` without a body when
  nothing changed. With the network down, a cached file is used. The folder stays under 1 GB
  (`TURU_HTTP_CACHE_MB`), dropping the files used longest ago; `TURU_HTTP_CACHE=0` turns it
  off. The perf log's `game files:` line has the counts. Checked against the live servers up to
  the login screen only (2026-11-12, debug build, headless): the second start took the title
  screen from disk and the game client after a `304` (2.8 MB not downloaded; from start to the
  client loaded: 1.6 s on the first run, 0.9 s on the second, 1.9 s with the cache off, one run
  each). In game (2026-11-12, debug build, headless, GoldFarm on twig, `tools/farmrun/run.sh`, one
  run each: cache off, empty, full): with the cache full, the start fetched 0.17 MB in its first
  30 s instead of 15.75 MB (`game files:` 38 from disk, 1 after a `304`, 7 downloaded; 14.9 MB
  not downloaded), and the map came 0.01 s after the join instead of 0.10 s. Times hardly
  moved on a fast connection: login to world loaded 5.0 s off, 6.4 s empty, 4.7 s full; start
  to in battlegrounde 17.1, 19.2 and 16.9 s (server round trips and the script's setup, not
  files). farmrun's network bytes are only good for the first minutes: `nettop` stops counting
  a connection once it closes.
- **Turu Mas uses the same cache:** AIR on Android keeps none of its downloads, so each start
  fetched the game again. The client's `AirLoader` (maps, monsters, gear, the assets file) and
  the host's `TuruLoader` (the client) now load through `TuruURLLoader` (`air/app/src`; for
  `AirLoader`, `turu_air_patch` turns its `new URLLoader()` into one), which hands a binary
  `GET` under `/gamefiles/` to the engine (`turu_frontend::game_files::fetch`, the navigator's
  rules) and falls back to AIR's download when that fails. The patched SWFs are kept too
  (`<data>/cache/airpatched`, `turu_air_patch::cache`). Checked on an emulator up to the login
  screen (2026-11-12): the second start read the title screen from disk and the client after
  a `304` (2.8 MB not downloaded), and read the patched client in 25 ms instead of patching it
  (~0.45 s); from skua.swf loaded to the client patched, 4.5 s on the first start, 2.2 s on the
  second. In game (same day, emulator, login to Battleon on Twig, the device's received bytes,
  two runs each): from start to the title screen 3.3 MB with the cache empty, 0.3 MB full;
  from Login to 30 s in Battleon 11.8 MB empty, 2.2 MB full (the rest: the socket and gear of
  players not seen before), with the map, gear and players drawn. Turu's logcat lines stop
  once logged in, so the `Game files:` lines show only up to the login screen.

The automated tests (`cargo test -p turu_bot`, the desktop layout test) run against a fake game.
In-game behaviour is checked by hand, so expect fixes in game paths and timings.

## Known gaps

- **Tabs share one thread and one process:** every tab's game runs on the event loop thread, so
  busy tabs slow each other down (VibeSkua's processes use several cores; hidden tabs in game
  run at 2 FPS, which helps), and a crash in one (release builds abort on panic) closes them
  all; "In its own window" keeps an account apart. The tabs share the data folder and Ruffle's SharedObjects (the game's saved
  login), as VibeSkua's processes do. `--api` steers the first tab only. Army > Army Queue runs squads
  one after another (`bot/src/army/queue.rs`); not built: VibeSkua's Army Scheduler. Logging
  in and farming in several tabs is unverified with real accounts.
- **Grid View is new and unchecked with real accounts** (`frontend/src/grid.rs`, nav bar Grid, with
  two tabs or more): every tab at once, each in a tile in VibeSkua's layout (rows of ⌈√n⌉),
  its player sized to the tile. The active tab keeps the keyboard, sound and its full frame rate,
  and its tile is outlined; the others run at 8 FPS (`grid::TILE_FPS`; VibeSkua's tiles run
  unthrottled, each a process of its own) and draw each frame they run. Clicking another tile
  makes it the active tab (that click goes no further). Each tile shows the tab's name, a dot
  while it's busy and "!" while it waits for you. All on the one thread: `TURU_PERF_LOG`'s
  `main thread` line shows what it costs. Not built: VibeSkua's hidden title and nav bars in
  the tiles (Turu's bar stays, for the active tab) and a hover state for tiles.

- **Refusals are logged:** a turn-in the server refuses (`ccqr` without `bSuccess`), with its
  reply and how far apart the quest's last two `tryQuestComplete` were sent, and the server's
  `warning`/`server` messages go to the Debug log and to the app's log with the account
  (`Bot::note`). VibeSkua ignores both; CoreBots' turn-in watchdog ("completable without a
  turn-in") had no way to say why. A turn-in the server never answers is noted when the same
  quest is sent again, and turn-ins that went through go to the Debug log; each says how soon
  after another quest's turn-in it was sent.
- **Registered quests turn in and accept one at a time:** CoreBots' watcher sends one
  `tryQuestComplete`, checks it `ActionDelay * 2` later and only then goes on with the next
  quest, as C#'s `foreach` awaits `CompleteQuest`; the manager accepts one quest per
  `ActionDelay * 2`, as C# waits after each. Both used to send everything at once. A packet
  capture of 0MaxBags showed the cost: of two accepts sent together the server dropped the
  second ("Please slow down"), the client still showed that quest (The Dark Deal) in progress,
  and its turn-ins went unanswered until the stuck check accepted it again 20 tries later.
  Unlike C#, the watcher and the manager also keep `ActionDelay` between their quest packets
  (a turn-in right after an accept, or the reverse, was still dropped now and then); a quest
  whose turn is too soon waits for the next run.
- **Quest packets keep `ActionDelay` apart everywhere:** `quests.accept`/`complete` wait until
  `ActionDelay` has passed since the last `acceptQuest` or `tryQuestComplete` anyone sent, and
  `quests.load` since the last `getQuests`. The game's `world.lock` cooldowns let CoreBots'
  `ensureCompleteQuests` (Swordhaven's rep farm) send each turn-in ~500 ms after the last, and
  the server dropped every one after the first; CoreStory's `preLoad` sent its `getQuests` ~300 ms
  after `checkCompletion`'s. `complete` also waits up to 1 s more for a late `ccqr` instead of
  letting `ensureComplete` send the turn-in again. Checked live with KingsEcho (no "Please slow
  down" in those spots); `bot/tests/ensure_complete.rs` covers the turn-in spacing.
- **`ensureComplete` recovers a lost accept:** when none of its `questAcceptAndCompleteTries`
  turn-ins gets a `ccqr` while the quest still shows in progress, it abandons the quest, loads
  it, accepts it again (`ActionDelay * 2` apart) and tries one more round. A refused turn-in
  (a `ccqr` without `bSuccess`) is left alone. 0MaxBags stood still for hours in CoreNation's
  `deal` loop: the accept of The Refreshing Deal (4777) was dropped ("Please slow down", next
  to the registered quests' packets), and every turn-in after it went unanswered. VibeSkua's
  `EnsureComplete` has no such recovery. Tested with a fake game (`bot/tests/ensure_complete.rs`),
  not yet in game.
- **Combat stalls are logged:** when the skill timer goes 2 minutes without attacking, it notes
  the same way the player's state (dead or alive), HP, map, cell and pad, the living monsters in
  the cell and whether attacks are stopped; again every 10 minutes while that lasts, and once
  it attacks again. VibeSkua logs nothing; a 0MaxBags run stood for 2.5 h without a target
  and left no trace.
- **Logins that differ from VibeSkua:** a login that isn't an auto-relogin (`--user` at start,
  `ensureRelogin` from a script or the control API's `/login`) counts as a relogin under way, so
  the `connectionLost` that its own retries cause doesn't start a second, competing auto-relogin
  (seven tabs logging in at once kept logging in and out on two servers). Retries keep the
  requested server while it's usable (VibeSkua drops it after a failed attempt), and an
  auto-relogin goes back to the server you were on (or the last one asked for) unless
  `ReloginServer` or `AutoReloginAny` is set (VibeSkua: the first server of the list). A timed-out
  attempt logs what the world check saw, and script log lines in the app's log start with the
  account. Why seven simultaneous first logins missed the 15 s world check (three didn't) is
  still open: the next such run logs it. A login you start from the server list yourself is
  left to you until it reaches the world: when it fails (after the pick, the loading screen
  shows "Server is full. Try another server" and the connection drops) the watchdog logs it and
  doesn't relog (VibeSkua relogs, on a server of its own choosing), and the message isn't taken
  for a stuck load (no logout out of the server list after `LoadTimeout`). The bot's own `clickServer` also reports `selectServer`, so only
  a pick while no relogin runs counts. In the bot's relogins a full server ("full" on the
  loading screen or in a login dialog) ends the attempt at once and isn't tried again during
  that login unless every server was full (VibeSkua waits out the world timeout). A server at its player limit only rules itself out for a
  non-member (VibeSkua: for everyone): members get in, and a login asked for Artix went to
  another server whenever Artix was full; while the account isn't known yet it counts as a
  member's. Checked in game with a member on Artix at 1502/1500. When all
  Relogin Tries attempts fail, Retry Relogin (on by default; VibeSkua reads it nowhere and gives
  up) starts another round after 1, 2, 5, then every 10 minutes, so an unattended run survives a
  network drop and restarts its script; a `/login` or `/relogin` during that wait cancels it
  instead of being refused, and a login from the game's screen meanwhile counts as the relogin.
  The retries don't need Auto Relogin to stay on (stopping the script turns it off), and the
  script a relogin restarts runs with Auto Relogin on again (VibeSkua leaves it off until the
  script turns it on), so a drop right after a relogin (a Mac waking from sleep) relogs again
  and `--exit-on-script-stop` waits out a relogin that starts within 2 s of the script stopping.
- **Relogin is unverified in game:** the tests run it against a fake login screen. Expect fixes
  in timings and in how `turu.swf` answers (`getActiveModalMessage`, `clickServer`).
- **Android (phase 3, `docs/ANDROID.md`):** the game on screen (Vulkan) with Turu's nav bar and
  bot windows (the desktop's, by touch), keys, the soft keyboard and sound (muted in the
  background, as a hidden desktop tab; Notify Drop's beep still sounds); a foreground service
  keeps it online in the background (status and Stop / Pause in its notification, a wake lock
  while a script runs, back on its own after a kill). Checked on an emulator, not a phone. No file dialogs
  (scripts come from Search Scripts); OpenGL ES misdraws; the soft keyboard covers the game's
  own text fields. Tabs (phase 5): several clients in one engine thread, the Army menu and
  Butler, a Tab Limit (default 4), tabs reopened after a kill; built, not run on a device yet.
  Vulkan misdrew on an Infinix Hot 30i (PowerVR): the debug log now names the
  GPU and driver, and Options > Application > Safe Drawing turns Mugetsu's speed-ups off to test
  (docs/ANDROID.md, "Misdrawing"; not yet tried on that phone).
- **Windowless mode logs in and farms** (2026-11-03: desktop `--headless` and Android, GoldFarm).
  It needed a fix in ruffle-aqw (`core/src/bitmap/operations.rs`): with AQW's default settings
  (no saved `AQWUserPref`, as on a new server or phone) the game draws the stage into a bitmap
  when a map loads; the null renderer can't, `BitmapData.draw` threw, and the code that makes
  your avatar never ran, so every login timed out ("avatar state: 0"). Now that draw leaves the
  bitmap as it is. A desktop that had played AQW in a window kept settings that skip the draw,
  which hid it. The control API is the only way to steer it. CoreBots' restart after a logout
  (`Bot.Manager.RestartScriptAsync`) isn't ported.
  In game it runs at a hidden tab's 2 FPS (`state.hidden_tab`, set in `app/src/headless.rs`;
  the login screen keeps full speed), and its player has no mouse (`set_mouse_in_stage(false)`).
  Ruffle starts with the mouse on the stage, and every update (about 50 a second, for timers and
  the socket, whatever the frame rate) then hit-tests the whole display list (`run_mouse_pick`):
  13% of the player's time at 30 FPS, 75% at 2 FPS. Android does the same while no window
  shows the game. Measured on FarmerJoe DoAll at Icestorm Arena (2026-11-08, debug build, 5
  minutes each): the process used 30% of a core at CoreBots' 30 FPS, 17% at 2 FPS, 13% with
  both; the player's ticks went from ~280 to ~80 ms/s, with as many kills.
  Of what's left, ~25% of a 30 FPS frame was AS3 `gotoAndStop` to the frame a clip is already
  on: Ruffle's `MovieClip::no_op_goto` still runs a goto's phases over the whole stage
  (`run_inner_goto_frame`). Measured in game (2026-11-10, GoldFarm at battlegrounde, debug
  build): ~165 gotos a second, half of them to the current frame, each walking ~1000 clips for
  construction but only ~90 for frame scripts. The frame scripts a nested frame runs make gotos
  of their own, each a nested frame again that constructs the whole stage before any of those
  scripts has finished, so clips with a script still to run (which ruffle-aqw's `settle` can't
  mark) were constructed about 11 times per goto. ruffle-aqw now also marks a clip
  "constructed" at the end of its `construct_frame` (`MovieClip::can_mark_constructed`), which
  later construction walks skip until something changes; a goto to the current frame leaves a
  settled clip settled, and a `Loader` settles with its children. Same run, 5 minutes each way
  (`RUFFLE_OLD_GOTO_SETTLE=1` puts the old behaviour back): construction visits 10.4 -> 0.9
  million a minute, the player's ticks ~135 -> ~93 ms/s, as many gotos and kills. Ruffle's
  regression tests pass as before (4 focus and key-input tests fail with or without it).
  The perf log's `gotos:` line has the counts (`ruffle_core::take_goto_stats`). Left: buttons
  never settle (their hidden state sprites have no parent to unsettle them through) and keep
  ~25 clips per goto walked; not measured in a release build, with a window, or on a phone.
  A frame's own three walks (entering the frame, construction, frame scripts) went over every
  clip too, though most of a crowded room stands still. ruffle-aqw now marks a clip "dormant"
  when it settles while stopped, with nothing queued and only such children: the mark outlives
  the frame, and all three walks skip the clip until something wakes it (`unsettle`, which
  `play` now calls too) or 32 frames have passed (a safety net). Measured with a window
  (2026-11-10, release build, 30 FPS, the mark on and off in turns of 20 s within one session,
  `RUFFLE_DORMANT_AB=600`), a frame's game logic: Battleon 2.2 -> 1.3 ms, Yulgar 3.1 -> 2.1 ms,
  farming Legion Tokens at /fotia 4.9 -> 1.95 ms. A 20-minute farm with it on ran without a
  disconnection or script error. Ruffle's regression tests pass as before. `RUFFLE_NO_DORMANT=1`
  turns it off. Also: the search for AVM1 clips to remove (a walk of the display list each
  frame) only runs after one was queued, and the frame epoch the marks read is a shared atomic,
  not a thread-local. Buttons (`SimpleButton`) carry the marks too now: before, one anywhere
  below a clip kept the clip and its ancestors from settling. A button's states that aren't
  shown have no parent, so a change in one can't walk up to the button: it takes every mark of
  every object away instead (never seen while farming). Measured the same way (2026-11-11,
  0MaxBags, turns of 2 minutes, `RUFFLE_BUTTON_AB=3600`): the clips that gotos' nested frames
  walk went from 1.10 million a minute to 0.36 million, a frame from 1.9 to 1.75 ms, game ticks
  from 68 to 65 ms/s: the walks that were left were cheap ones. `RUFFLE_NO_BUTTON_MARKS=1` turns
  it off. Not tried: a session of hours over many maps and cutscenes, or a phone.
- **Handlers wait during one engine wait:** a handler doesn't run while a single Rust-side wait
  is blocking (`bot.wait.forMapLoad`, `forCellChange`, `forActionCooldown`, `forFullyRested`,
  ...); it runs at the next bot call after that. VibeSkua's timer thread runs them all the time.
  Closing it needs the script's JS context inside those waits; the handler table is ready for it.
- **Objects for Flash are new:** `FlashArg` carries arrays and objects now (`bot.flash.set(path,
  {})`, used by CoreStory's `preLoad`), but nothing has sent one in game yet. Equipping still goes
  through the `equipItem` packet (check consumables in game), and CoreBots' quest-limit handler
  isn't restored.
- **The black screen after a cutscene is fixed only in tests:** VibeSkua's `skipCutscenes` removes
  the cutscene but leaves `world.visible` off, so the game stayed black until Lag Killer was
  toggled. turu.swf's `skipCutscenes(showWorld)` shows the world again after a skipped cutscene
  (the engine asks it to unless Lag Killer is on), and a stopping script sends `killLag(false)`
  even when Lag Killer was off (VibeSkua's `ScriptStopped` toggle). That the cutscene hides the world is inferred, not seen in game.
- **Skip Cutscenes cancels the cutscene the game waits for:** a map's `world.cHandle.showCutscene`
  (lichwar's intro, while quest value 602 is 0) makes the game hold back the monsters of the cells
  you enter until the overlay appears, for up to 450 frames (~225 s at 2 FPS headless). Skipping
  removed the overlay before the game saw it, so a hunt in /lichwar found no monster to attack
  (ColossalWaresMerge's Grimskull's Favor). turu.swf's Skip Cutscenes module now cancels the
  pending cutscene and enters the cell again, checking every 100 ms (it was the engine's, every
  options pass). Seen in game with the real intro (2026-11-14, ColossalWaresMerge headless on a
  character that hadn't been to lichwar): cancelled 1.7 s after joining the map.
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
- **Script options:** the window never opens by itself with `--headless`, or for runs started from the HTTP API
  (unless it passes `configure=1`), auto-relogin or the Scheduler (VibeSkua's `SilentConfig` covers only
  `--script-options-storage` runs and the scheduler; its relogin restart shows it again, and CoreBots'
  `SetOptions` shows it even on silent runs, which Turu doesn't, so an unattended client never waits on it).
  As in VibeSkua it does open for `--run-script` (so Accounts' auto-start) and Army Control's Start, in each
  client. A run that skips it logs which values it uses, saved or defaults (`ScriptManager::note_unshown`; with
  the defaults also a monitor `scriptOptions` event, and `/script/start` answers `options`/`optionsWindow`, which
  Army Control and the monitor show); the control API's `/script/options` and `/scheduler/options` and the web
  monitor's Script Options tab read and set them (`docs/SCRIPT_OPTIONS_PROPOSAL.md`). The monitor tab was
  checked against a mock server only, not a live client.
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
  (`configOr`), as VibeSkua does. Legion's join asks whether to
  sell the "Undead Warrior" armor (`bot.showMessageBox`), as C# does; with nobody to answer it
  keeps the armor. Three C# bugs are fixed on purpose (marked `C#`):
  FarmerJoe's pet step reads its real option, TheLeeryContract uses the reward it's given, and
  Dage PvP reads the CBO solo-boss switch's value.
- **Nation fixes that differ from VibeSkua (unverified in game):** 11 bugs still in VibeSkua's
  C# Nation scripts are fixed in the ports (marked `C#`, listed in `scripts/docs/PORTING.md`): voucher
  turn-ins check the Gems not the Diamonds, the member voucher isn't sold before the quests that
  need it, the Dwobo shop map name, and smaller guard and loop fixes. Only type-checked.
- **The converted scripts are unverified in game:** 1559 of the 1658 files cs2ts made (586 for
  the merges, 282 for Story, 333 for the core content areas, 235 for Seasonal, 188 for Other, 34 for
  Army and Tools) are drafts (their first line says so)
  that type-check and load but weren't read through by hand; expect C#/TS differences the types
  don't show (integer division, culture-dependent formats, async C# rewritten as handlers).
  `bot.map.cellPlayers` reads each player's `world.uoTree` cell (VibeSkua tracks them from
  packets). Nothing is left to port but what is
  skipped on purpose: the Ultras' `TestZone`, `Templates`, the unused `Prototypes`, the generator, and the
  Tools listed in `scripts/docs/PORTING.md`.
- **Potions is unverified in game:** its runtime test only checks that it loads, reads its
  options and stops (the fake game doesn't serve inventory, shops or maps). Farming, the alchemy
  crafts and buying are not checked yet.
- **Search Scripts' tests use a fake repository** (statuses, deps, the zip, reset, bad paths
  and hashes). `muktiarafi/turu-scripts` is public now, with its `index` branch; the raw and
  codeload downloads and the ETag 304 against it aren't covered by a test. Scripts made for a
  newer `turu.ts` than the client's aren't flagged (the index has no client version yet).
- Quest rewards and shop items lack some fields; CoreBots reads them from the game objects.
- Not built in the UI: Themes, Plugins. HotKeys work only while Turu's window has
  focus (no global hook) and not on Android
  (`docs/UI.md` has the full list).
- **Release packaging** (`.github/workflows/release.yml`, from a `v*` tag) doesn't use Mugetsu's
  speed profile (PGO), and the macOS app is signed ad hoc, not notarized.

## Things to know

- **`scripts/` is a submodule** (`muktiarafi/turu-scripts`): clone with `--recursive` or run
  `git submodule update --init`; the tests that load scripts read it. Commit script changes in
  the submodule, then the new pointer here. Its `types/turu.ts` is a copy of `bot/js/turu.ts`
  for type-checking outside Turu; copy it again when the API changes (`script_repo` tests check).

- **Never hold a lock the UI reads across a Flash call.** The UI thread answers Flash calls, so
  a bot thread that waits on Flash while holding such a lock, and a UI frame that waits on the
  lock, freeze each other until the call times out; meanwhile no game runs and the server
  drops every tab (Butler's broadcast did this; `butler_state_reads_never_wait_for_a_game_call`
  tests for it). Read the game first, then lock.
- **Crashes:** a panic is written to `crash-<date>.log` in the log folder (Ruffle's cache folder's
  `log/`, e.g. `~/Library/Caches/ruffle/log` on macOS; Mugetsu's "Open Log Folder"), at once, as
  the log itself is written on another thread that an abort may cut off. `ruffle.log` is made anew
  at each start; the previous run's is kept as `ruffle.prev.log`. On Android, panics and the
  system's record of each unexpected exit go to the data folder's `crash/` instead, shown in
  Diagnostics > Logs > Crashes (docs/ANDROID.md, "Crash reports").
- **Crash reports to Sentry (desktop):** builds made with `TURU_SENTRY_DSN` set (the release
  workflow passes the `SENTRY_DESKTOP_DSN` secret) send crash reports to the `turu-desktop`
  project once the user agrees: the same "Send crash reports?" question and Options >
  Application switch as Android (`sendCrashReports` in `settings.json`; set it there for
  `--headless`). `app/src/crash_uploads.rs`: a panic is sent as it happens (Sentry's panic hook
  runs before Turu's), and Turu's hook writes `panic-<date>-<pid>.txt` in the data folder's
  `crash/` (it used to write `crash-<date>.log` next to ruffle.log, which the crash notice and
  Diagnostics > Logs > Crashes don't read, so a panic left nothing to see there); a native crash
  on Windows is written to the data folder's `crash/` by the crash handler
  (`native-<date>-<pid>.txt`, so the crash notice shows it too) and sent at the next start.
  The handler (`app/src/windows.rs`, `crash_filter`) skips continuable informational exceptions
  (DirectX's 0x87A and 0x87D), and ends quietly, with no report or message box, on any exception
  while Windows unloads the DLLs of a process that is already exiting (`RtlDllShutdownInProgress`):
  Sentry TURU-DESKTOP-4 (beta.7) was DXGI raising 0x87A as non-continuable during shutdown, after
  the window had closed. It ends with the exit's code if Windows has recorded it yet, else 0.
  Checked only by type-checking that code for Windows from macOS (the app doesn't cross-build).
  No IP address, no computer name, the home folder's path replaced with `~`, no logs. The release
  workflow builds with line tables (`CARGO_PROFILE_DIST_DEBUG`) and uploads the PDB / dSYM /
  Linux binary with `sentry-cli` (`SENTRY_AUTH_TOKEN`, `SENTRY_ORG`, `SENTRY_DESKTOP_PROJECT`),
  then strips the Linux binary. `TURU_TEST_PANIC=1` panics 10 s after the start, to test it
  (also in builds without a DSN: the crash file and message).
- **egui out of GPU memory:** egui-wgpu panicked when the GPU had no staging buffer for the
  overlay's vertices or indices (Sentry TURU-DESKTOP-3, beta.5, Windows). ruffle-aqw patches it
  (`ruffle/patches/egui-wgpu`, wired up in both workspaces' `[patch.crates-io]`): that frame draws
  no egui meshes and logs a warning, and a buffer that failed is made anew next frame. It's the
  overlay on the desktop and the whole UI on Android. Tested with a device whose buffers are too
  small; not seen on a real out-of-memory GPU.
- **A lost GPU device (desktop):** a driver reset (Windows' TDR, a driver crash) drops the
  device in every program at once: Sentry TURU-DESKTOP-5 (beta.8, Windows 11) was two clients in
  their own windows panicking in the same second, as getting the window's next picture failed
  ("Acquiring a texture failed with a validation error": wgpu reports a lost device's acquire that
  way). As on Android, the window now stops drawing (`GuiController::render` checks
  `ruffle_render_wgpu::device_lost()`, which ruffle-aqw's device-lost callback sets) and keeps its
  last picture, while the game and its scripts go on; it's said once in the log, every tab's debug
  log, the window title ("graphics stopped, restart Turu to see the game") and to Sentry (a warning,
  `gpu: lost`). An acquire's validation error with the device still working skips the frame, and
  panics only after 120 in a row. The UI can't be used until Turu restarts (closing the window
  works). `TURU_TEST_GPU_LOSS=1` destroys the device 20 s after the start: on macOS (debug build)
  the first acquire after it failed with that validation error before the device-lost callback
  ran (where TURU-DESKTOP-5 panicked), was skipped, and the next frame stopped drawing; the game
  went on (no panic, the debug log and title said so). Not seen on a real driver reset. Getting
  the picture back without a restart: docs/GPU_RECOVERY_PROPOSAL.md.
- **Clippy** lints only `bot/`, `frontend/` and `app/` (the `ruffle/` crates are path dependencies
  outside the workspace). It passes with `--all-targets` and `-A clippy::redundant_clone` (that
  lint, denied by Ruffle's workspace lints, still flags a few clones in tests and UI windows); CI
  runs exactly that (`.github/workflows/ci.yml`).
- **HTTPS in `turu_bot`:** the bot crate asks for reqwest's TLS itself, through its default
  `native-tls` feature (`rustls-tls` instead where there's no system TLS: Android); without one a
  `cargo test -p turu_bot` build could only fetch `http://` (the app got TLS from Ruffle's crates).
  `turu_frontend` takes `turu_bot` without default features: the app picks the backend.
- **Testing `turu_frontend`:** test it with the app (`cargo test -p turu -p turu_frontend` and the
  app's features as `turu/...`, as in `CLAUDE.md`). Alone, `ruffle_core` gets other features, so its
  build script compiles the AS3 playerglobals again, with Java and `ruffle/tools/asc/asc.jar`;
  that failed here with Java 25 ("Could not find or load main class
  macromedia.asc.embedding.ScriptCompiler"), while the app's cached build is reused.
- **Formatting:** the bot crate uses `cargo fmt`. Not the app (Ruffle's files keep their own
  layout: `cargo fmt -p turu` rewraps 50 of them). The files in `frontend/src/gui/turu/` are
  formatted with `max_width = 120`, so format one file at a time
  (`rustfmt --edition 2024 --config max_width=120 <file>`). Running rustfmt on `mod.rs`
  reformats every window.
- `docs/mugetsu-patch.md` describes what Mugetsu's patch changes, including the egui
  palette.
