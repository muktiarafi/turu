# Turu status

What works, what is missing, and the things that trip up work on Turu. Keep it current when
something lands or a gap closes. Read `CLAUDE.md` and `docs/ARCHITECTURE.md` first.

## What works

- **App:** `app/` on ruffle-aqw (`ruffle/`: Ruffle `f22d29d9` + Mugetsu 2.0.0's patches) starts
  Turu's host SWF, `turu.swf`, which loads AQW (docs/HOST_SWF_PROPOSAL.md). You log in with the
  game's own screen.
  On the Ruffle builds the bot's packets come from the player's socket (R5).
- **Mugetsu 2.0.0** (2026-11-17; was 1.6.0 on Ruffle `fda41800`): ruffle-aqw replays Turu's
  commits on it, less six: the "constructed", "dormant" and button marks for clips that stand
  still (Mugetsu's lean walks do that now: `lean-scripts`, `lean-frames`, turned on with
  Mugetsu's other 2.0.0 speed-ups by `platform::DEFAULT_ON`, on a computer and on Android), a
  leak diagnostic built on `Arc<SwfMovie>` (upstream made movies GC objects), and since M6 the
  per-player thread-local state and command segments that let several tabs share a thread (one
  player per process now). Mugetsu's render thread can be used in a tab's client since
  2026-11-18, with a `render-thread` file next to the program (below); its present thread follows Mugetsu's default
  in the window for a movie given on the command line (a client draws into a texture the shell
  shows, and never uses it).
  Mugetsu 2.0.0's player options are in the window's right-click menu (its 2.0.0 layout, one
  line per setting with a hint): Performance mode (the game drawn at 85% or 70% of the window's
  width and height and stretched, in every tab and Grid View tile), the memory limit (Task
  Manager's figure; over it everything that can be freed is, in every tab, at most every 20 s,
  backing off to 5 minutes while a freeing doesn't hold) and "Free memory now", uncapped frame
  pacing and the FPS limit (with frame interpolation: the window's frames stop waiting for
  v-sync). As in Mugetsu 2.0.0, edge smoothing is no longer a choice, in the menu or Options
  (Flash-style; an `msaa` file next to the program brings MSAA back), and the log folder left
  the menu (Diagnostics and Options open it). Not ported: Mugetsu's update reminders (Turu has
  its own updates) and its in-game prompts (Turu asks about a graphics restart itself).
  Mugetsu 2.0.0's Windows changes (2026-11-17): DirectX 12 is Automatic's choice on Windows
  (Vulkan on Linux; `mugetsu::automatic_graphics`), Vulkan where DirectX 12 crashed while
  starting (`mugetsu-dx12-crashed.txt` in Mugetsu's settings folder, until the crashed file
  changes) or can't be started (`mugetsu::open_gpu`, used by the window, the shell and the
  clients alike, so they stay on one backend; `RUFFLE_TEST_DX12_FAILS=1` tests it); the crash
  handler writes the record and a stack as module+offset to the native report before anything
  that can hang, adds the thread and the closing step (`mugetsu::exit_phase`), and the normal
  exit waits for a report another thread is writing (`windows::wait_for_crash_report`); the
  window, the shell and the clients let go of the GPU (and the DXGI adapter the video memory
  query keeps) while the driver is alive, and the window and the shell end with
  `process::exit` after the log is flushed. Type-checked for Windows from macOS only (see
  "Things to know"); nothing of it has run on Windows. Mugetsu's one-time tips for built-in
  graphics are in every tab (2026-11-18; `turu_frontend::graphics_tips`, a modal in Turu's UI):
  slow at Quality Medium/High for 15 s, the Quality Low tip; still slow at Low, Performance mode
  offered with a "Turn on" button (Fast below 60 % of the frame rate, else Balanced). Windows
  count only while the tab is drawn in full all along (not hidden, covered, a Grid tile or in
  Headless Mode; on Android not with frame interpolation, and a phone's refresh rate caps the
  yardstick); each tip once per data folder. Android has Performance mode too (Options >
  Application; `mugetsu-performance.txt` in the data folder, read at a slot's start, touches
  scaled), and since 2026-11-18 Show FPS, Memory Limit (over it the slot frees shapes and spare
  textures, at most every 20 s, on top of Turu's own budget) and Free Memory Now (unused SWFs,
  shapes not drawn for 30 s, spare textures), saved as Mugetsu's files in the data folder
  (`mugetsu-show-fps.txt`, `mugetsu-memory-limit.txt`). Frame Pacing and the FPS limit aren't:
  Android's compositor shows every picture at the display's rate, so uncapped has nothing to do
  on a phone. In a tab's client (every AQW tab since M6) Performance Mode,
  Memory Limit and Free Memory Now are in Options > Application, and since 2026-11-18 Frame
  Pacing (V-sync, Uncapped) and the FPS Limit (Off, Screen, 60, 120, 144).
  **Frame interpolation in a tab's client** (2026-11-18): with Frame Interpolation on, the client
  asks the shell to present on every refresh of the display (`ToShell::Pace`), and the shell tells
  it after each present (`ToClient::Refreshed`); the client draws a new in-between picture on the
  refreshes Turu's pacer picks (`frontend/src/pacing.rs`, as the window and Android do), so the
  pictures keep in step with the display rather than a timer of the client's own. Uncapped, the
  shell presents each picture as it comes without v-sync, at most the FPS Limit's a second. The
  shell sends the monitor's refresh rate (`ToClient::RefreshRate`). Only with shared surfaces
  (pixels stay at one picture per game frame), and only for the tab seen whole (not another
  tab's Grid View tile, not Headless Mode's screen). A shown client draws into up to 5 shared
  surfaces (`SHARED_POOL`, was 3): at 120 Hz, 3 left a third of the refreshes without a new
  picture and 4 a quarter, as each is the shell's for about 4 refreshes; the 4th and 5th are made
  only while interpolating (3 with it off, as before; `shared surface N of at most 5 made` in the
  log). Due pictures skipped (one still under way, no surface free) are counted in the perf log's
  `pictures:` line (`TURU_PERF_LOG`); Turu's pacer doesn't see them yet. Measured on the Mac (Apple M5, 120 Hz, release build, login screen,
  `TURU_SHELL_TEST`'s `rate`, which now also counts presents with no new picture, one, or more):
  120 presents a second, each with one new picture (a handful of misses in 3600); at every second
  refresh (the debug build's costlier draws) 60 new ones, half the presents, none doubled;
  uncapped 90 with the limit at 90, about 360 without (in later runs macOS held it to the
  refresh rate, 120, with the render thread or without); Options' Frame Pacing, FPS Limit and Frame
  Interpolation clicked on a running tab took effect at once (`TURU_SHELL_TEST`'s `click`). Not
  run: in game, Grid View, Windows and Linux.
  **Mugetsu's render thread in a tab's client** (2026-11-18, opt-in): with a `render-thread` file
  next to the program (not with `no-render-thread`, nor without the shape uploads it counts on,
  as in Mugetsu), a client's renderer is Mugetsu's `ThreadedBackend`: the game's picture is walked
  on the engine thread and its GPU commands are made on the render thread meanwhile. The client
  submits its own picture (the blit and Turu's UI) after the game's, from the render thread
  (`render_thread::run_after_picture`), and asks for "done" only then; it reads the game's
  texture by the renderer's target state (`render_thread::target_state`), so drawing doesn't wait
  for the picture being made. The pacer budgets by the slower of the two threads, and by a
  picture's whole way from its draw to the GPU having finished it (`Pacing::record_pipeline`, also
  without the render thread: the client draws the next only then; recorded once per game frame
  too, so a tab it took down to that comes back; the perf log's `pictures:` line shows it). Mugetsu turns it on with frame interpolation on Windows; Turu leaves it off until it has
  run there and in a crowded room. The perf log (`TURU_PERF_LOG`) says the thread's last picture
  and what waited for it, and its `memory:` lines split the process's memory (Mugetsu 2.0.0's
  `memory_split`, `app/src/memory_split.rs`: the allocator's committed memory and pages, on
  Windows the system heaps), for the 7.5 GB Mugetsu saw with the render thread and uncapped
  pacing on Windows. Its `heap-collect` test switch isn't ported: Turu already hands mimalloc's
  free memory back once a minute (`release_free_memory`).
  Checked on the Mac (login screen): 120 even pictures a second in the
  release build, with it and without; in the debug build 120 with it (the floor never engaged:
  about 3 ms from draw to finished), against 60 without; two tabs, switching, minimizing,
  uncapped with and without a limit, Options open, a resize, the fallback to pixels and the memory
  limit. Not confirmed: the client's own close with it (a killed shell's clients end by
  `--exit-with-parent`'s stdin watcher first). It can't show here that the thread pays: that needs
  a crowded room. (The pacer's budgets changed since: next paragraph.)
  **Pictures under way together** (2026-11-18, Windows, Radeon 780M, 165 Hz, release builds, in
  game): a client drew a picture only once the GPU had finished the last, and the pacer took that
  whole way (~30 ms in battleon: the draw, the GPU, the shell's turn) as the time between two, so
  frame interpolation gave up in every crowded room: 24 pictures a second where Mugetsu drew
  115-160. Now:
  * Up to 3 shared pictures are under way at once (`SHARED_IN_FLIGHT`; pixels still one), and with
    more than one the pacer has no floor drawn from a picture's way (`Pacing::set_in_flight`), as
    Mugetsu has none: pictures wait for a free slot and as many get through as the GPU allows,
    unevenly. With the floor, a busy GPU took a client to one picture per frame (24 in Yulgar
    beside Mugetsu, which kept interpolating at 30-35).
  * The shell gives a surface back from a thread that waits for the GPU (`turu_frontend::gpu_waiter`),
    writing the release straight to the client under the tab's write lock: polled from the main
    thread, which spends most of a refresh waiting for the next, a surface came back ~4 refreshes
    late. With this and the pictures under way together, the login screen went from 65-83 of 165
    to 165 (not measured apart).
  * With a render thread, its picture has a budget of its own (850 ms a second, Mugetsu's
    `RENDER_THREAD_BUDGET_MS`), and what the engine thread waited for it isn't charged to it.
  * Mugetsu's present thread in clients (Windows, as in Mugetsu; `RUFFLE_SERIAL_PRESENT` off):
    the game's picture is finished and submitted there (`MovieTexture::submit_later`), and the
    client's own picture is sent after it through the same thread (`ClientScreen::set_present_thread`),
    and the client's close waits for it before letting go of the device. Mugetsu's window spends
    ~3 ms a picture on its render thread and ~2 on its present thread in battleon, where a client
    without one spent 5-6.7 on its render thread; side by side with Mugetsu after the port (both
    GPU-bound, one GPU): 0.018 ms a pass on Turu's render thread (release build), 0.013 on
    Mugetsu's (dist + PGO).
  * Tried and dropped: a budget of the GPU's time drawn from when pictures finished. A picture's
    cost depends on the rate itself (battleon: 10 ms interpolating, 14 ms at one per frame), so it
    stepped down and never came back up (80, 55, then 30 for good); Mugetsu has none either.
  The perf log now has the client's `interpolation replay:` and `per picture drawn:` lines (as the
  window's) and the pacer's inputs on `pictures:`, and the shell logs each tab's new pictures and
  its presents a second (Mugetsu's "drawn, presented"). Measured: whitemap 165; battleon 24 → ~82
  (every 2nd refresh, render thread 4.5-6 ms a picture; Mugetsu's 2.7-3.4 ms gets every refresh);
  side by side with Mugetsu in one room (~450 passes a picture, one GPU for both), Turu 25-37 new
  pictures a second, Mugetsu 31-39 ("presented 52-64"). Yulgar alone (~20 players): 30 (one per
  frame) before, 55 after, 61-75 while aiming for 83 (the GPU's limit: due pictures skipped with
  all 3 slots busy); the step follows the engine thread's draw (4.3-7 ms there, against 650 ms a
  second for frames and pictures). Mugetsu's FPS counter counts presents
  (repeats too), Turu's badge new pictures: the two read differently for the same picture rate.
  Mugetsu's PGO profile (`pgo/aqw.profdata`) applied to a Turu `dist` build changed the render
  thread little. Known: near a step's edge (55 against one per frame) the pace can flip between
  `Refresh` and `Frames` within a second, a visible change of rhythm each time; Mugetsu's pacer
  puts a step on trial and undoes it (`PipelineLimit`, `LIMIT_TRIAL`), Turu's doesn't yet.
  `RUFFLE_GPU_TIMING` reads a frame's timestamps one frame later now (ruffle-aqw's `gpu_timing`,
  not committed there yet): a present thread submits later, and mapping the buffer at once failed
  that submit (`BufferStillMapped`, seen in Mugetsu's window). Not checked with the present
  thread: macOS and Linux (off there), the client's close with pictures still on it.
  Found on the way: the memory limit's freeing, Free Memory Now and a nearly full graphics memory
  freed the renderer's spare textures in the window only; a tab's client never did (since M6,
  every AQW tab). They do now (`memory::trim_renderer`).
  Safe Drawing is back (2026-11-19; removed 2026-11-17): Options > Application on Android (or
  `--safe-drawing` in `turu-args.txt`), a `safe-drawing` file next to the program on a computer
  (every process reads it, the shell's clients too), from each tab's next start. It turns
  Mugetsu's drawing speed-ups off (`platform::SAFE_DRAWING`: the five old switches and all eight
  of 2.0.0's, as Mugetsu's `safe-drawing` does) and, Turu's own, ruffle-aqw's skipped OpenGL calls
  (`RUFFLE_GL_LEVERS`, a suspect on phones drawing with OpenGL). A speed-up's own file
  (`lean-render`) brings it back under Safe Drawing; single `no-<switch>` files and `--env` lines
  still turn one off. It only says whether a speed-up is at fault, and costs game time in crowds
  (the lean walks are off). Unit-tested; the log lines checked on the Mac; not run on a phone.
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
  `TURU_VERSION` (the tag the release workflow builds from; local builds: the crate's, the
  release the current pre-releases lead to (0.4.0, above every 0.4.0 beta), so they're offered
  nothing; raise it when a release ships). A newer one is logged once and
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
  **Latest on GitHub:** `release.yml` publishes a beta tag without GitHub's pre-release flag and
  marks it Latest (GitHub's Latest can't be a pre-release), as it does a release; alpha and rc
  tags stay pre-releases. The updater ignores that flag for betas: the `-beta` in the version
  makes it a pre-release, offered only on that channel.
  **Nightly builds** (`release.yml`: nightly at 03:00 UTC when main has new commits, or by hand
  with "Publish as the nightly release"): built as `<next version>-nightly.<date>`
  (`0.4.0-nightly.20261118`, About shows the `nightly` channel) and published as one rolling
  `nightly` release (a pre-release, never Latest) that the next one replaces, for testers who
  download it by hand. They don't update and aren't offered: a nightly build never checks or
  installs (`updates::is_nightly`; Options greys the client update rows and says why, `POST
  /update` refuses), and `updates::newest` skips nightly releases. Its tag, `nightly`, isn't a
  version, so clients released before this skip it too. One release rather than one a night:
  the updater reads one page of GitHub's list (30 releases), which nightlies would fill, pushing
  real releases off it. Android version code: stage 8, between rc (3) and release (9), so a
  nightly installs over its version's pre-releases and the release installs over it. Not yet run
  on Actions.
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
  Sounds piled up headless too (2026-11-17, measured in a Mac probe; the VPS grew the same way):
  `--headless`, and a client that can't open an audio device, build the player with Ruffle's null
  audio backend, which reported every sound as playing forever, so AQW's mixer
  (`Game.mixer.activeChannels`) never got `soundComplete` and kept every `SoundChannel` and its
  listener: ~3.3 per kill, ~5,000 an hour at ~13 kills a minute. The null backend
  (`ruffle/core/src/backend/audio.rs`, marked `Turu:`) now knows each sound's length (with its in
  and out points and loops, as Ruffle's mixer plays it) and reports it over once it has played;
  streams play on as before. An MP3 loaded with `Sound.load` has no length there and completes at
  once (none seen in AQW's cached files).
  The Headless Mode option is separate: it lowers the game's frame rate in either mode, and
  with a window it covers the game with VibeSkua's overlay and stops drawing it.
- **Control API** (`--api`, or `--api=<address>`; default `127.0.0.1:9091`): HTTP routes for
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
  `curl -H "Authorization: Bearer $(cat ~/Library/Application\ Support/Turu/api-token)" localhost:9091/status`.
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
- **Tabs** (`frontend/src/turu.rs`, `gui/turu/tab_strip.rs`): since M6 (2026-11-17) every tab
  runs in a process of its own behind the window, the shell (docs/TAB_PROCESSES_PROPOSAL.md): on
  a computer `turu` is the shell over `--client` processes, on Android the activity is the shell
  over the slots. There is no Process Tabs switch and no `--in-process` any more (it's ignored);
  `--headless` is one tab in its own process (docs/ARCHITECTURE.md, "Processes"). VibeSkua's
  tabbed host embeds one process's window per tab, which macOS and Wayland don't allow; Turu's
  clients draw into surfaces the shell shows instead. Only the tab on screen is drawn and heard
  (a client starts muted and hidden: `app/src/client.rs`). Hidden tabs that are in game run at
  2 FPS, as VibeSkua's tabbed host throttles them (`WM_SKUA_THROTTLE`), and get their rate back
  when shown (`state.hidden_tab`, applied with Headless Mode in `api/options.rs`; tabs at the
  login screen keep full speed, so logins aren't slowed). Unlike VibeSkua they keep their
  quality: an undrawn tab gains nothing from LOW, and going back to HIGH re-tessellated every
  shape on each switch (only Headless Mode sets LOW). Before M6 the tabs were players on one
  thread of one process, each with Ruffle's thread-local state swapped in while its code ran;
  that went with ruffle-aqw's two commits behind it (docs/TAB_PROCESSES_PROPOSAL.md, "M6 as
  built").
- **Tabs' saved game data** (fixed 2026-11-18): Flash's shared objects (AQW's `AQLite_Data`,
  with its key bindings and options; `AQWUserPref`; `ignoreList`), which the window writes when
  its player drops at close, were never written by a tab's client: the shell killed it at once,
  and its own quit path ends with `exit_now`, which skips the drop. Data the game didn't write
  itself was lost each time a tab or the shell closed. Now the client writes it first when its
  connection closes, and the shell closes the connections and waits up to 3 s (`CLOSE_GRACE`)
  before killing a client that hasn't ended (closing one tab waits on a thread; closing the shell
  waits for all at once, about 20 ms in a run). Turu also writes each shared object whole (a file
  of its own, then renamed: `preferences/storage.rs` `WholeWrites`; in place if Windows refuses
  the rename), as clients share the folder and now write it together at close; Ruffle's emptied
  the file and wrote it. A client whose shell crashed still ends at once (`--exit-with-parent`)
  without writing.
  Looked into and left: the game's messages and a failed load of the root movie only go to a
  client's log (Ruffle never calls `UiBackend::message`; the root movie is `turu.swf`, and AQW's
  own load failure is reported by it to the bot), no file dialogs or `fscommand` in a client
  (AQW's client never browses for files: `FileReference` is only named in its SmartFoxServer
  library; and it has no `fscommand`).
- **Tabs' input** (2026-11-18, the gaps M6 left in a tab's client; the shell passes the window's
  on to the tab on screen):
  * **Input methods** (IME): the client asks for one (`ToShell::Ime`: allowed, a password's, where
    its candidates go) while one of Turu's text boxes has the focus (as egui-winit allows it in a
    window), or the game's focused text field does with Ruffle's IME preference on (`[ime]
    enabled = true` in Ruffle's `preferences.toml`, off by default, as in the window); the shell
    applies the tab on screen's while the window has the focus, and passes the composition on
    (`ToClient::Ime`): to the text box as egui-winit does, else to the game as the window's
    `app.rs` does. Keys go nowhere while a composition is under way (Enter mustn't send the game's
    chat). The game field's area is converted from the stage's pixels to the picture's (the
    window takes them as its viewport's, which puts the candidates off when the game is scaled).
  * **Keys** (fixed 2026-11-18): the client rebuilt the game's keys from egui's name for them,
    which lost the physical key (every key `PhysicalKey::Unknown`). Ruffle takes the game's
    `keyCode` from it on macOS and Linux (0: WASD didn't walk, Enter didn't log in or send chat)
    and tells held keys apart by it everywhere (on Windows, W then D held and let go left D down:
    the character walked on). Keys egui has no name for (Shift, Ctrl, punctuation, the numpad)
    didn't reach the game at all. The shell now sends the game's key whole, as the window's
    `app.rs` makes it (`util::winit_input_to_ruffle_key_descriptor`), with what it does in a text
    field (`winit_to_ruffle_text_control`: Ctrl+Home/End, Ctrl or Cmd+Backspace and arrows as in
    the window), by Ruffle's names (`ToClient::Key`'s `game`, `shell::GameKey`). Without it (a
    test's `key` line) the client finds the key from egui's name, as on a US keyboard.
  * **Typed text with Ctrl** goes to the game when Alt is down too (AltGr on Windows: `@` on a
    German keyboard; the shell had dropped all text with Ctrl, as egui-winit does), not with Ctrl
    alone or Cmd (Turu: the window types the letter a Ctrl shortcut gives on Linux). Turu's text
    boxes take none with Ctrl or Cmd, as egui-winit.
  * **A tab that stops taking input** (hidden, or another tab's tile in Grid View) lets go in the
    game of the keys and mouse button it had down and loses the focus (Flash's `deactivate`):
    their release would go to the next tab, and a tab left with W held walked on hidden. A key
    the game has down is let go there even if the focus went to Turu's UI meanwhile.
  * **The game's links** (upgrade, buying AC, account pages) open in the browser through the
    shell, unless Options deny websites; they were only logged (`engine::NoDialogs`, now with
    `open_links`). Turu: the window asks first with Options' "Ask"; a client has nobody to ask in.
  * **A trackpad's scrolling** reaches the game in pixels, as in the window (it went as lines of
    40 pixels, 2.5 times as fast as the window's 100). The game's full screen (`stage.
    displayState`) and `Mouse.hide` reach the shell's window too (from any tab's game, a hidden
    one's too; Esc leaving full screen isn't told to the game, which AQW doesn't mind).
  * **Gamepads**: the shell reads them on a thread of its own (gilrs; the event loop sleeps) and
    passes each button on (`ToClient::Gamepad`); they act only through `--gamepad-button`'s
    mappings to keys, as in the window (the clients get the shell's flags).
  * **Dropped files**: a script (`.ts`, `.js`) dropped on the window is loaded in the tab on
    screen, as Load Script loads it (not started; not while a script runs); anything else is
    refused in the log. The window opens a dropped file as a new movie, which would replace AQW.
  * **Flash's right-click menu**: a right press the game doesn't take opens Flash's items
    (`Player::prepare_context_menu`) in Turu's UI, holding the release back until it closes, as
    the window's menu does. AQW turns Flash's own items off and adds none at its login screen
    (the list was empty), so it shows only if the game adds items; in game it wasn't seen. A
    press outside closes it from its second frame on (egui lays a new area out unseen first, and
    the opening press counted as one elsewhere), and Esc; a checked item shows selected.
  Checked on the Mac with `TURU_SHELL_TEST`'s new lines (`ime`, `drop`, `gamepad`, `rclick`): a
  committed text in the game's Username field and in Options' search box; the window's IME
  allowed for the box, and for the game's field only with the preference, with the field's area
  on the field; a dropped script loaded, a picture refused; a right click at the login screen
  opened nothing, and with two items faked for one run the menu showed and a press elsewhere and
  Esc closed it; Backspace during a composition was held back and reached the box after it. Not run: a real input method and its candidate window, plain typing with the
  IME allowed (macOS didn't let this session send keys: on most systems winit doesn't send an
  input method's keys, which egui-winit counts on too), a real gamepad and mapping, a real drag,
  Flash's menu with items, Windows and Linux.
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
  (`bot/src/api/handlers.rs`) and the script runs them between bot calls; a `bot.sleep` ends its
  slice when one falls due, so they keep their interval while the script sleeps. CoreBots' AFK and registered-quest handlers use it.
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
- **Search Scripts** (`bot/src/repo.rs`, `egui_ui/src/gui/turu/script_repo.rs`): VibeSkua's
  Script Repo for the scripts in `muktiarafi/turu-scripts` (the `scripts/` submodule): search,
  categories, pins, Start/Load/Queue (downloading the script and what it imports), Update all,
  Download all and Reset Scripts, and the launch-time check and auto update (Options >
  Application). Edited scripts are kept by updates and backed up by a reset.
  `docs/ARCHITECTURE.md` ("Script repository") has the design.
- **Scheduler** (Scripts > Scheduler, `bot/src/scheduler.rs`, `egui_ui/src/gui/turu/scheduler.rs`,
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
  the title, the client's starting frame rate (24 by default, which leaves turu.swf's own 30, as VibeSkua's
  skua.swf ran: 24 judders on a 60 Hz screen, plain on a phone; any other value is passed to
  `loadClient`. Set FPS defaults to 0, leaving the game's own FPS setting alone),
  relogin delay and watchdog, Clear Flash Cache, and Discord webhooks for script
  start, stop and crash, relogins, listed drops and health pings (`bot/src/webhooks.rs`).
  Unverified against a real webhook. The update switches act at launch: script updates
  (Search Scripts) and, with both AdvanceSkill Sets switches on, the community skill sets
  (`auqw/Scripts`' `AdvancedSkills.json`) merged into the user's, new classes and modes only
  (`turu_bot::skills::startup_skill_sets`, also with `--headless`; tested with a fake download).
  The junk-item switches are stored only (VibeSkua never reads them either).
- **Junk Items:** Tools & Helpers > Junk Items (`egui_ui/src/gui/turu/junk_items.rs`) lists the
  inventory and bank items (filters, quantities, equipped / favourite / near-cap chips, quantity
  goals), marks the selected ones as junk and sells the junk in the inventory;
  scripts call `bot.sellJunk()`. The list is `JunkItems.json` in the data folder, in VibeSkua's
  format (`bot/src/junk.rs`; a VibeSkua file, or `auqw/Scripts`' community list, can be copied
  over). Tested against a fake game (`bot/tests/junk.rs`); not yet sold in game.
- **Web monitor** (`docs/WEB_MONITOR_PROPOSAL.md`, replacing the Skua
  Nexus webhook): Options > Application > Monitor serves a page (`bot/assets/monitor/index.html`)
  on `127.0.0.1:9021`, or on every interface with Allow Other Devices, showing this window's tabs
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
  comment every 15 s; the page keeps the server's clock going between frames. **CPU and memory**
  (`docs/MONITOR_RESOURCES_PROPOSAL.md`, phase 1 of 3): `monitor/resources.rs` reads this process
  (share of a core, resident memory and its peak, the footprint and its peak on macOS, a Linux
  container's cgroup limit) and this machine (cores, CPU, total and available memory, and the
  system's version, read once: "macOS 15.2", "Windows 11 (build 22631)", a Linux distribution
  with the kernel and whether it's in a container, "Android 14, kernel 5.15") with the
  OS's own calls (`/proc` and the cgroup files; `proc_pid_rusage`, `host_statistics`; Win32), no
  `sysinfo`. The hub reads every 2 s while the page is open and once a minute always, keeping an
  hour (`/api/machines/this/history`); `/api/state` has `machines`. The page shows a machine strip at
  the top of each machine's roster sheet (its own sheet above the toolbar when there are no
  machine headings): fixed-width CPU, memory and container readouts with a thin meter each,
  Turu's CPU and memory with its hour as wax dots, and details on click. Checked by hand on macOS (Apple Silicon): RSS against `ps`, the
  footprint against `footprint`, available memory against `vm_stat`, the version against `sw_vers`;
  the page at 375 px and in both
  themes. Linux, Windows and Android only type-checked (`cargo clippy --target`), with the
  `/proc`, cgroup and `os-release` parsers unit-tested; the page's other states (machine
  headings, a container, Android without machine CPU, the first reading) against a stub server.
  Not run yet on Linux, Windows or Android, nor in the Docker image with `--memory=1g`. **Phase 2:**
  an Army client's `/snapshot` carries its own `process` (its CPU since the last poll), shown in
  its side panel; a report carries `host` and `resources` (the machine reading, the reporter's
  process and its last hour, which the monitor takes on at the first report), and each pushed
  Army client its `process`. The hub groups every live process by host id, so one server running
  several Turu processes (or containers) has one line ("Turu ×3"), its details listing each
  process by its characters; `/api/machines/<host>/history` has the line's points and each
  process's own. Checked by hand with one `--monitor` and two `--report-to` processes on this Mac
  (one line, footprints against `footprint`, history taken on after the monitor restarted) and
  a real Army client (`army_clients_reach_the_hub`, ignored by default, run with `TURU_EXE`);
  older Turu on either side only through unit tests (an old report's JSON; the monitor's types
  ignore unknown fields). Not checked: two Compose containers on one host. **Phase 3:** machine
  entries head "Needs you", one per problem with its numbers and a Details button that opens the
  machine's line: almost out of memory (under 10% or 300 MB free, this reading and the minute's
  point before; not on phones), a container over 90% of its limit, CPU over 90% for 5 minutes
  (`cpu_busy_since`, kept by the probe), a Turu process growing over 100 MB an hour (after its
  first 10 minutes, seen for 20). The thresholds are constants at the top of the page's script.
  Checked by hand against a stub server with made-up states and history (every rule firing, and
  a phone low on memory staying quiet); not yet on a real server running out.
  **Control** (Allow Control, on by default): Start / Pause / Resume / Stop the loaded script, Log
  in, Relogin, Log out, House and Join map, on one bot or the selected ones (asking first for the
  ones that interrupt), and Forget for offline entries. The hub (`monitor/hub.rs`) keeps every
  entry: tabs from their sampler, Army clients from the Army poll's `GET /snapshot`; it saves to
  `monitor.json` every minute, so after a restart the last characters show offline for a day.
  Army Control reads the clients from the hub. **Other machines** report to it (Options >
  Application > Report, or `--report-to <url>` with `TURU_MONITOR_PUSH_KEY`; `monitor/push.rs`):
  their tabs and Army clients show under the machine's name and take the same commands, which
  ride back in the reply to the next report (the monitor holds it up to 4 s, so they land at
  once; their logs too). Several processes may report under one name (one per account on a
  server, sharing a data folder and so a name): each report carries a random id for its process
  (`push::instance`), and the hub keeps their entries and command queues apart
  (`remote:<machine>:<instance>:<bot>`); a restarted process's offline rows go when it reports
  again. Checked with one `--monitor` and two `--report-to` processes on one data folder at the
  login screen, and a reporter restarted. Reports need the monitor's push key (`monitor-push-key`, Copy Report
  Settings), which opens nothing else; Accept Commands on the reporting machine can refuse
  commands. `--monitor[=address]` serves the monitor without the setting. Options shows the
  phone link as a QR code. Each tab's sampler reads a `Snapshot` every 2 s
  only while a page is open (items and quests every 30 s or after they change); Army clients are
  read every 2 s, their slow fields only while a page is open. The webhooks format the same
  `Snapshot`. The `api` routes need the key from `monitor-key` except from a loopback connection
  naming a loopback host (so Tailscale Serve needs it); a command also needs the page's own
  `Origin` or the key. Clients launched by Army don't serve it. Checked: the tests (a real
  Army client reaching the hub is an ignored test, `TURU_EXE=target/debug/turu cargo test -p
  turu_bot monitor:: -- --ignored`), the page against the headless client at the login
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
  (`docs/DIAGNOSTICS.md`). The wait includes the queue on the player's thread, so a wait far above
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
    are seen and counts the repeats (`uncaught AS3 errors` line; `RUFFLE_ERRORS_EVERY_TIME` logs each). An
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

### GL renderer

OpenGL (docs/GL_RENDERER_PROPOSAL.md, G0 to G4, 2026-11-18, branch `gl-renderer`) draws the
whole game on OpenGL ES 3.0, and since then it's Android's default (Automatic: OpenGL ES, else
Vulkan; Options > Application > Graphics, or `--graphics` in `turu-args.txt`). Battleon on the
emulator once settled: Vulkan 29 pictures/s at 18.9 ms of render each, GL 27.4/s at 27.5 ms
(1.45x). The GL replay skips calls that set what's already set (11 GL calls per draw, from 21).
Switches (on Android `--env NAME=value` in `turu-args.txt`):

- `RUFFLE_GL_STATS=1`: a `GL replay` line every 5 s (calls by kind, skipped calls, uploads,
  submit time, context locks by thread).
- `RUFFLE_GL_LEVERS=none` (or `fbo`, `state`): the replay's skipping off, for A/B runs and when
  GL misdraws.
- `RUFFLE_NO_BASE_VERTEX=1`: shapes drawn without `base_vertex` (as on GLES 3.0 and 3.1).

Not yet run on a real phone (GLES 3.2: the `base_vertex` path), on desktop GL, or with the GL
memory budget (NVIDIA, AMD).

## Known gaps

- **Tabs are processes since M6** (2026-11-17): a crash or a lost GPU device ends one tab, which
  the shell starts again logged in. Since 2026-11-18 a tab's client has the window's input
  methods, gamepads, dropped files and Flash's right-click menu too ("What works", "Tabs' input").
  Turu's own UI fonts have no Chinese, Japanese or Korean glyphs (composed text shows as boxes in
  its text boxes, in the window too); AQW takes few special characters and none of those, so it
  isn't worth fonts of its own. Not run on Windows or Linux, nor on phones (M4).
  The tabs share the data folder and Ruffle's SharedObjects (the game's saved
  login), as VibeSkua's processes do. `--api` steers any tab (`/tabs/<id>/...`; routes without it
  act on the first). Army > Army Queue runs squads
  one after another (`bot/src/army/queue.rs`), in tabs with Accounts' "In a tab"; not built: VibeSkua's Army Scheduler. Logging
  in and farming in several tabs is unverified with real accounts.
- **Grid View is new and unchecked with real accounts** (`frontend/src/grid.rs`, nav bar Grid, with
  two tabs or more): every tab at once, each in a tile in VibeSkua's layout (rows of ⌈√n⌉),
  its player sized to the tile. The active tab keeps the keyboard, sound and its full frame rate,
  and its tile is outlined; the others run at 8 FPS (`grid::TILE_FPS`; VibeSkua's tiles run
  unthrottled) and draw each frame they run. Each tile is its tab's client's picture, drawn by
  the shell (each tab a process of its own since M6). Clicking another tile
  makes it the active tab (that click goes no further). Each tile shows the tab's name, a dot
  while it's busy and "!" while it waits for you. Not built: VibeSkua's hidden title and nav bars in
  the tiles (Turu's bar stays, for the active tab) and a hover state for tiles.

- **A script's relogin is checked in game** (2026-11-27, phase 3 of
  docs/REPLAY_TESTS_PROPOSAL.md): `bot.servers.relogin("Twig")` from the world, headless, on tape
  (`bot/tests/tapes/relogin`, replayed by `replay::the_relogin_tape_replays`). It failed in game
  until relogins waited for the login screen (see "Differences from VibeSkua"). The auto-relogin
  after a disconnect, the server-full and modal paths, and a window's client are still checked
  only against a fake login screen.
- **Android (phases 4 and 5, `docs/ANDROID.md`):** the game on screen (Vulkan) with Turu's nav
  bar and bot windows (the desktop's, by touch), keys, the soft keyboard and sound (muted in the
  background, as a hidden desktop tab; Notify Drop's beep still sounds); a foreground service
  keeps it online in the background (status and Stop / Pause in its notification, a wake lock
  while a script runs, back on its own after a kill). Checked on an emulator; on phones, a Redmi
  10 and a Pixel 8 have run it (docs/ANDROID.md). File dialogs are Android's system picker
  (`android/src/saf.rs`); OpenGL ES is the default since 2026-11-18 (see "GL renderer" below); the game's own text fields are typed in a native
  field above the keyboard (docs/ANDROID_NATIVE_UI.md). Tabs: each a slot, a service in a
  process of its own, under the activity as shell (process tabs, M3 and M6), the Army menu and
  Butler, a Tab Limit (default 4, at most 9 in slots since 2026-11-22; Turu Mas at most 4, in one
  process), tabs reopened after a kill; several tabs not run on a phone yet.
  Since 2026-11-20 (branch `native-ui`, docs/NATIVE_UI_PROPOSAL.md) the rail, the tabs and
  Accounts and the daily windows are native Android pages beside the game: Script Loader, Search
  Scripts, Scheduler, Logs, Auto, Jump, Options (Game, Application, CoreBots), Army Control and
  Notification Center, Stats, Current Drops, Skills, Loadouts, Fast Travel, Butler, Cosmetics, Script Options, Runtime, Notify Drop, Loader, Grabber, Junk Items, Console and the packet tools (with the engine's dialogs);
  each window's page has Full window for the egui window and what only it has (Auto and Jump are
  the rail's popups, with no window). Checked on the emulator.
  Vulkan misdrew on an Infinix Hot 30i (PowerVR): the debug log now names the
  GPU and driver (docs/ANDROID.md, "Misdrawing"; Safe Drawing, untried there, turns Mugetsu's
  speed-ups off: removed on 2026-11-17, back on 2026-11-19).
- **Windowless mode logs in and farms** (2026-11-03: desktop `--headless` and Android, GoldFarm).
  It needed a fix in ruffle-aqw (`core/src/bitmap/operations.rs`): with AQW's default settings
  (no saved `AQWUserPref`, as on a new server or phone) the game draws the stage into a bitmap
  when a map loads; the null renderer can't, `BitmapData.draw` threw, and the code that makes
  your avatar never ran, so every login timed out ("avatar state: 0"). Now that draw leaves the
  bitmap as it is. A desktop that had played AQW in a window kept settings that skip the draw,
  which hid it. The control API is the only way to steer it. CoreBots' restart after a logout
  (`Bot.Manager.RestartScriptAsync`) isn't ported.
  SIGTERM and Ctrl+C quit cleanly (`turu::shut_down`, 2026-11-27): the playlist and script
  stopped, a logout, Butler and the tapes closed, status 0; a second signal or 8 s quits at once
  with status 1. Before, a signal ended the process where it stood.
  Several on one data folder need no setup (2026-11-27): their startup script downloads take
  turns (`script-update`'s lock), new keys are written whole and linked into place so all agree
  (`control_api::saved_token`), Report To without a push key uses the folder's, and with Enable
  Web Monitor on the first serves it and the others report to it (`jobs/monitor-address.json`,
  same host only), one taking over when it ends (`start_headless_monitor_election`).
  In game it runs at a hidden tab's 2 FPS (`state.hidden_tab`, set in `app/src/headless.rs`;
  the login screen keeps full speed), and its player has no mouse (`set_mouse_in_stage(false)`).
  Ruffle starts with the mouse on the stage, and every update (about 50 a second, for timers and
  the socket, whatever the frame rate) then hit-tests the whole display list (`run_mouse_pick`):
  13% of the player's time at 30 FPS, 75% at 2 FPS. Android does the same while no window
  shows the game. Measured on FarmerJoe DoAll at Icestorm Arena (2026-11-08, debug build, 5
  minutes each): the process used 30% of a core at CoreBots' 30 FPS, 17% at 2 FPS, 13% with
  both; the player's ticks went from ~280 to ~80 ms/s, with as many kills.
  Of what's left, a 30 FPS frame went mostly to the display list's walks (gotos to the frame a
  clip is already on, and each frame's three walks over clips that stand still). Turu's own
  marks for such clips ("constructed", "dormant", buttons; 2026-11-10 and 11) were dropped with
  Mugetsu 2.0.0 (2026-11-17), whose lean walks (`lean-scripts`, `lean-frames`) do the same work.
  Still to measure them against Turu's marks, which had brought a frame's game logic (release
  build, window, 30 FPS) from 2.2 to 1.3 ms at Battleon, 3.1 to 2.1 ms at Yulgar and 4.9 to
  1.95 ms farming at /fotia, and, headless (debug build, GoldFarm at battlegrounde), the
  player's ticks from ~135 to ~93 ms/s. How those were measured and what each mark did: this
  file's history (`git log -S RUFFLE_DORMANT_AB -- docs/STATUS.md`).
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
  (the engine asks it to unless Lag Killer is on), and a stopping script turns the `LagKiller`
  module off even when Lag Killer was off (VibeSkua's `ScriptStopped` toggle). That the cutscene hides the world is inferred, not seen in game.
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
- **Auras are unverified in game:** `bot.self` and `bot.target` call `turu.swf`'s aura
  callbacks (`host/as3/turu/host/Auras.as`) as VibeSkua calls skua.swf's, but only a fake game
  has answered them. Unlike VibeSkua,
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
- **CoreStory's `mapItemQuest` and `killQuest` are checked in game** (2026-11-27, phase 3 of
  docs/REPLAY_TESTS_PROPOSAL.md): the first two Dark Dungeon quests (three map items, then eight
  drops from Dungeon Minions), with `preLoad` and `questProgression`, on tape
  (`bot/tests/tapes/core_story`, replayed by `replay::the_core_story_tape_replays` in about two
  minutes). The unlock-recovery loop of `questProgression` relogs, and CoreBots' `relogin` falls
  back to Twilly as in C#: tapes keep off it (each quest runs only when unlocked), so it is
  unchecked. It ran once by mistake on a quest that wasn't unlocked: it relogged 8 times, on Twilly
  after Twig failed (the relogin bug since fixed).
- **The merge template is checked in game on one item** (2026-11-27, phase 3 of
  docs/REPLAY_TESTS_PROPOSAL.md): SynderesMerge buying only "NO BOTS Armor", its missing souvenir
  handed to `findIngredients` and farmed through a registered quest, then the buy, on tape
  (`bot/tests/tapes/merge_buy`, replayed by `replay::the_merge_tape_replays`). Nested merges (a
  requirement bought from the same shop) and the buy modes other than one item still run only
  against a fake shop.
- **Enhancement is checked in game for `enhanceEquipped` only** (2026-11-27, phase 3 of
  docs/REPLAY_TESTS_PROPOSAL.md): Lucky on a level-16 free character's class gear through the
  real shops (Healer 1 to 14, the Battle Oracle hood, staff and wings to 16, 13 and 15), on tape
  (`bot/tests/tapes/enhance_equipped`, replayed by `replay::the_enhancement_tape_replays`).
  `enhanceItem`, `smartEnhance`, `gearStore`, the specials (cape, helm, weapon: C# picks a special
  by its name and, on a tie, the first plain "... Cape/Helm/Weapon" entry would win) and
  `rankUpClass` (Icestorm Arena) still run only against fake enhancement shops. C#'s odd spots
  are kept (Forge = 1 takes the Awe-shop path since the specials were renumbered). Items carry
  `enhancementPatternId` and `procId` now (`EnhPatternID`, `ProcID`); the weapon proc and the
  applied enhancement ID are read from `world.invTree` as C# does.
- **CoreDailies is unverified in game:** its tests check `checkDailyv2` (daily done, max
  stacks, the drop list) against a fake game. The dailies themselves, the friendship packets and
  `bot.player.webBadges` (a request to account.aq.com's character page, for `Core.hasWebBadge`)
  need real runs.
- **FarmerJoe is mostly unverified in game:** DoAll has run, a fresh character from level 1 to
  35 over about 14 hours headless (docs/HEADLESS_MONITORING_HANDOFF.md). It type-checks and its
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
- **The converted scripts are unverified in game:** 1,604 of the 1,868 script files (432 in
  `seasonal/`, 395 in `other/`, 343 in `story/`, the rest in 13 other folders) are drafts cs2ts
  made (a line under their header says so) that type-check and load but weren't read through by
  hand; expect C#/TS differences the types don't show (integer division, culture-dependent
  formats, async C# rewritten as handlers). Each one with a `main` (1,623 of the 1,639 marked
  files; the rest are Core libraries) now runs for 60 s of play on a virtual clock against a fake
  world seeded from a tape (2026-11-27, `bot/tests/draft_smoke.rs`, nightly in
  `.github/workflows/drafts-nightly.yml`). The fake world has monsters that die and drop what the
  draft asks for and quests that are turned in, so drafts get through their hunts and quests: all
  1,623 made 7,736 kills and turned in 3,160 quests between them. None throws or fails to start
  but `story/TowerOfDoom.ts`, a gap in the fake (no monster for the floors no script names).
  It found two port bugs, fixed in `ultrasv3/dependenciesultras/UltraGeneral.ts` (`Kolrv3` and
  `Masakadov3` read class options they don't declare, which C# allows). That checks they load and
  get into their own code (the story drafts do their quests); it doesn't check that what they do
  is right, nor anything past the fake's gaps.
  `bot.map.cellPlayers` reads each player's `world.uoTree` cell (VibeSkua tracks them from
  packets). Nothing is left to port but what is
  skipped on purpose: the Ultras' `TestZone`, `Templates`, the unused `Prototypes`, the generator, and the
  Tools listed in `scripts/docs/PORTING.md`.
- **Tests from recorded game sessions** are proposed in `docs/REPLAY_TESTS_PROPOSAL.md`: record a
  real run once, replay it in CI, and run every converted script against a fake world seeded
  from a recording. The recorder (phase 1) is built and checked on a 20-minute headless farm
  run: `TURU_TAPE` / `--record-tape` (docs/DIAGNOSTICS.md, "Recording a tape"). So is the
  scrubber (`turu-tape scrub`), checked on that run's tape, and the replay
  (`turu_bot::testing::ReplayHost`, 2026-11-27, `bot/tests/replay.rs`). Seven tapes recorded
  in game replay in the tests (`bot/tests/tapes/`: a quest loop, a merge buy, enhancement, bank
  moves, a relogin, CoreStory and a kill farm), on a virtual clock (decision 2: the bot's clock in
  `ScriptControl`, real unless a test makes it virtual), a second or two each. The converted
  scripts run against a fake world from a tape (phase 4, below). Phase 5 (recording from
  Diagnostics for bug reports) isn't built.
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
- **Release packaging** (`.github/workflows/release.yml`, from a `v*` tag) doesn't use a speed
  profile (PGO): `tools/pgo/pgo.sh` records one of Turu's own by playing (docs/DIAGNOSTICS.md), but
  none is committed, and CI can't record one (it would need a game login). Measured once
  (2026-11-24, FarmerJoe on Galanoth, a profile from 15 min with a window and 15 headless): no gain
  that stands out from the run-to-run differences (headless 3% of a core with both; a window's
  client 49 s of CPU in 6 min plain, 55 with the profile, in different rooms); the binary is 38 MB
  against 46. Telling a few percent apart needs a fixed workload (one room, the same actions). The
  macOS app is signed ad hoc, not notarized.

## Differences from VibeSkua

Where Turu's behaviour differs from VibeSkua's C# on purpose, and why (`CLAUDE.md`: behaviour
follows the C# by default; a difference needs a reason and is listed here). In the Rust code each
is marked `Turu:` (`grep -rn 'Turu:' bot/src frontend/src`); in the scripts, fixes to C# bugs are
marked `C#` and listed in `scripts/docs/PORTING.md` (the Nation fixes: "Known gaps" above, as
they are unverified in game). Don't remove one to match the C#. What VibeSkua has that Turu lacks
is in "Known gaps" and `docs/PORTING_GAPS.md`.

- **A relogin waits for the login screen before it logs in** (2026-11-27,
  `servers::log_out_and_in`): VibeSkua's `Relogin` calls `login` right after
  `gotoAndPlay("Login")`, which takes effect on the client's next frame (half a second later
  headless, at 2 frames a second); a `login` before it was lost, the server list never came and the
  relogin timed out (in game: every scripted relogin from the world failed, and CoreBots' `relogin`
  then fell back to Twilly). Turu waits up to 5 s for `mcLogin` to show.
- **A startup script waits for the player to load** (2026-11-27, `startup::auto_login`):
  VibeSkua starts `--run-script` (and Turu's `--run-playlist`) a second after the player is alive,
  while its inventory and avatar may still be loading, and `map.join` does nothing until they
  have: a script on the bare API whose first call was a join stayed in the map it logged into (seen
  recording the first replay tape: every turn-in after was refused). Turu waits up to 20 s more for
  the player to load, then starts anyway. CoreBots scripts already waited in `SetOptions`, so they
  start no later than before.

- **Player HUD** (2026-11-25, Turu's own: VibeSkua has none; `turu_bot::vitals`,
  `egui_ui/src/gui/turu/player_hud.rs`): Combat > Player HUD (also Options > Application, and the
  TogglePlayerHud hotkey) shows the HP and MP of the players in your cell in a small panel in the
  game's left, under its portrait of you (drag it anywhere: it stays there, a double-click puts it
  back), you first, then by name, 8 rows (5 on a short screen) and "+N more";
  under a quarter of their HP a player's bar turns red and says "Low HP". On Android (its menus
  are native, `models::rail`) the switch is in Options > Application.
  It follows the packets, without polling the game: `moveToArea`'s `uoBranch` (the room),
  JSON `uotls` (an arrival's whole entry, max HP and MP changing with gear), string `uotls`
  (others' cells, resting), `ct` (combat), `exitArea`, and the `moveToCell` you send (the
  server tells the others your cell, not you). The game (`world.uoTree`, a few fields per
  player in one `getMany`) is read only when they can't say: switched on in a room already
  entered, a packet about a player it doesn't know, an arrival without its max, HP over its max,
  you coming back to life without a cell.
  Off, every handler stops at one flag. A bar needs the player's max; without one only the
  figure shows. Saved in `settings.json` (`showPlayerHud`), so new tabs start with it; an open
  tab takes another tab's change when it next reads its settings. Hidden in Grid View and
  Headless Mode; not in Turu Mas. The arrival and gear packets were checked against a capture
  (2026-11-25); `moveToArea`'s `uoBranch` fields weren't, so a branch without a max reads once.
- **Target Auras** (2026-11-25, Turu's own: VibeSkua has none; `turu_bot::target_auras`,
  `egui_ui/src/gui/turu/target_auras.rs`): Combat > Target Auras (also Options > Application, and
  the ToggleTargetAuras hotkey) shows the auras on the monster you last attacked and **who put
  each one on**, for party fights (Ultras): its name and HP, then an "Enrage" line (Scroll of
  Enrage's Focus and Reckless: who cast them and the seconds each has left, or "Down"/"Cleansed
  Ns ago"), then each caster's auras (you first, the others by name, the boss's own last and in
  grey) with their stacks and a bar for the time left, yellow in its last 2 s; 9 lines of auras (4
  on a short screen) and "+N more". It starts on the right, can be dragged (saved,
  `targetAurasPos`), and a double-click puts it back. Each aura has its icon as the game draws it:
  turu.swf's `GetAuraIcon` takes the packet's `icon` as the game's aura HUD does (the last of a
  comma list, `i,i,i,i,i,LR2` is `LR2`), finds its class where the game's `world.getClass` looks
  but without its side effects (that one remembers a miss for the room and says so in chat), and
  draws its first frame 32 px square, sent as Base64 ARGB. The timer asks for an icon the panel
  shows, once per name; one the game can't find yet (its class file not loaded) shows the game's
  own stand-in, `isp2`, and is tried again 3 times, 5 s apart, and again in the next room. Until
  an icon is drawn its place is an empty square, so the line doesn't move when it comes.
  It follows the packets: `ct`'s `a` list (`aura+`/`aura++` with the caster in `cInf`,
  `aura-`/`aura--` by name, `aura*` a boss's cleanse, also sent alone), `ct`'s `m` (a monster's
  death clears it), `mtls`, `moveToArea`'s `uoBranch` (players' `entID`s), and the `gar` you send
  (your target). The server keeps one aura of a name per monster, so a second caster's `aura+`
  takes it over and the HUD names the latest. It sends no time stamp: the time left counts from
  the packet, and an aura of 3600 s or more (boss mechanics send 7200) has no countdown; a timed one
  whose time ran out goes even if no `aura-` came. The game is read (at most once a second) only for
  a caster the room's `uoBranch` didn't name, the monster's name, and once per target the auras it
  had before the HUD saw it (shown under "Unknown"). Checked against a capture of an Ultra fight
  with four players and of a room join (2026-11-25): `uoBranch` carries each player's `entID` and
  `strUsername`, so the room's casters need no read; an arrival's JSON `uotls` wasn't seen, so a
  player who came later is named by a read if it lacks `entID`. Hidden in Grid View and Headless
  Mode; not in Turu Mas.
- **Installing an update waits for every tab** (2026-11-22, `models::client_update`): VibeSkua is
  one process; Turu's tabs are processes of their own (Android's slots, a desktop shell's
  clients) and an update closes them all, so Download & install (and Download & restart) wait
  while any other tab runs a script or Auto, naming it, as they wait for this tab's script. The
  shell's tab list says only "busy", so Auto counts in other tabs. On Android two tabs can't
  download the update at once either (a lock in `jobs/`): the second is told another tab is.
- **Game Options asks before Private Rooms goes off, in CoreBots' words** (2026-11-22,
  `models::game_options::PUBLIC_ROOMS`, `warning_modal`, Android's `PublicRoomsDialog`): VibeSkua
  turns it off without a word, and only CoreBots Options warns, on Save ("Public Room Warning",
  Yes/No). Botting in a public room lets other players see the bot and report the account, which
  can get it banned, so Turu says so and keeps the option on unless the user confirms. CoreBots'
  Save asks with the same dialog ("Bot in public rooms?", Use public rooms or Keep private, both
  shells); Escape or a click outside closes it without saving. Scripts, Defaults, Revert and the
  control API set Game Options' without asking.
- **Streamer Mode hides names in Turu's own screens too** (2026-11-21, `turu::tab_label`,
  `slots::SlotShell::label`, `shell_accounts::accounts_view`, the Loadouts and Cosmetics models):
  VibeSkua's tabs and Manager keep the account names, but in Turu the tab list, Android's Accounts
  and New tab, Loadouts' heading and Cosmetics' room players are part of the screen being streamed
  ("Tab 2", "Account 3", "Your loadouts", "Player 1"; docs/UI.md, Game Options), and so are
  Butler (`models::butler::Masks`), CoreBots' character, the account forms and the desktop
  Accounts window. VibeSkua's works only in game; Turu's also masks the login screen's username
  field (`api::options::apply_out_of_game`, `Streamer.as`'s `loginFrame`).
- **Army Queue runs squads in tabs and follows the tabs** (2026-11-21, `army::queue::TabLauncher`,
  the window's `ShellTabs`): with Army > Accounts' "In a tab" (the default), each account opens in
  a tab of the window that runs the queue, as VibeSkua's clients join its tabbed host. VibeSkua then
  waits on the process it started, which hands its flags to the host and quits at once, so a squad
  could look done before its scripts ran; Turu waits on the tab instead: an account is done once its
  tab closes (with `--exit-on-script-stop` a tab logs out and asks the shell to close it, as a
  process quits). Two new account states: "Already Open" (its tab was open when its turn came: it's
  skipped, not taken over) and "Didn't Start" (its script hadn't run within the queue's
  `TimeoutMinutes`, 30 by default and unused by VibeSkua, or the shell never opened the tab: its tab
  is closed and the queue moves on, where a failed login held it for good). "In its own window"
  and "Without a window" launch processes as before; a window's only tab closing itself this way
  ends the window (the shell used to start the tab again, running the script once more, and the
  process never ended, so the queue waited on it for good). Not checked with real accounts.
- **A tab Android closed for memory too often stays down** (Android's process tabs, 2026-11-22,
  `android/src/memory_kills.rs`): VibeSkua has no processes the system starts again. A slot that
  comes back after a kill for memory logs in and runs its script again 3 times in 30 minutes
  (waiting up to 20 s while the phone is low on memory); the 4th kill holds it at the login
  screen, its script not started, until Start again in its notification or the Tabs panel (or it
  gets in game by hand). Its
  notification and tab row say why (docs/ANDROID.md, "Tabs closed for memory").

- **A script dropped on the window is loaded** (a process tab's, 2026-11-18): VibeSkua takes no
  dropped files; Turu loads a dropped `.ts` or `.js` in the tab on screen, as Load Script does.

- **The native Grabber's actions wait for a checked row** (Android, 2026-11-21, `GrabberPage.kt`):
  VibeSkua's buttons are always on and, with nothing selected, answer "No quests found/selected."
  (`grabber::prepare`, unchanged). On a touch screen that message came after the tap; the page now
  turns off every action but Unregister All until a row is checked and says so in its status
  ("Check quests in the list to use these"). The engine and the egui window are as VibeSkua.

- **Cosmetics empties a slot by deleting it** (2026-11-19, `cosmetics::avatar::unequip_slot`,
  turu.swf's `deleteGameObjectKey`): VibeSkua sets `objData.eqp[slot]` (and `eqp.pe`) to null.
  The game reads a field of every `eqp` value in two places, and a null one threw: the
  `equipItem` reply (`Avatar.calculateIntoBoosts`, before the item is marked equipped), so
  equips after an emptied slot went through on the server but never showed as equipped (CoreBots'
  `equip` retried 5 times, UltrasLW's `equipClass` stopped with "<class> could not be
  equipped."), and Character > Outfits (`outfitsetsr2.swf`'s `initAvatar`), which stuck half
  built. Copying a look and Reset to Real Gear empty slots. The game's own unequip deletes the key
  too. Found from the game's decompiled code and a log; not yet run in game. A session already
  in that state needs a relog.

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
- **The spell-effect queue is emptied every 10 s:** turu.swf's `Events` already emptied your
  avatar's `AvatarMC._spFXQueue` on a new map (`clearSpellFX`); it now also does it every 10 s on
  the same map (not while `eventsOff` is on). A headless kill-only probe (Greenguard West, ~13 kills
  a minute) gained ~2.7 queued entries per kill, about 1,400 in 26 minutes, with their
  `Object`/`Array`/string garbage: ~16k Flash objects an hour, roughly half of the ~27k an hour the
  probe leaked in all (the sound channels, under "Windowless mode" above, were the rest). That is
  the same order as the VPS's growth (+35k to 65k objects an hour), not yet confirmed there. Emptying the queue once
  freed the entries; with both fixes the object count stayed flat for 16 minutes (not yet measured
  over hours, or with quests and drops). What's dropped is effects queued in the last 10 s and
  not yet drawn, perhaps ones for monsters still there; only the avatar's animation was found to
  read the queue, so it looks visual only (not proven). Why the game's own animation doesn't keep
  up isn't known.

## Things to know

- **Type-checking the app for Windows from macOS:** `cargo check` (or `clippy`) with
  `--target x86_64-pc-windows-msvc` stops in C build scripts (rquickjs-sys, the app's icon
  resource) for want of MSVC's compiler and headers. `cargo check` links nothing, so a stand-in
  that only creates the files it's asked for gets past them: a script that, for `-o <file>`,
  `-Fo<file>`, `-out:<file>`, `/fo <file>` and `cq <file>`, writes an empty file, set as
  `CC_x86_64_pc_windows_msvc`, `CXX_x86_64_pc_windows_msvc` and `AR_x86_64_pc_windows_msvc`, and
  linked as `llvm-rc` early in `PATH`. Leave out `jpegxr` and `external_video` (bindgen needs real
  C headers): `--no-default-features --features software_video,lzma`. It checks the Rust only:
  no link, no run.
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
  `log/`, e.g. `~/Library/Caches/ruffle/log` on macOS; Options > Application > Open Log Folder), at once, as
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
  Checked only by type-checking that code for Windows from macOS (see "Things to know").
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
- **Clippy** lints only `bot/`, `frontend/` and `app/`, and `host/` in CI's Host SWF job (the
  `ruffle/` crates are path dependencies outside the workspace). It passes with `--all-targets`
  and `-A clippy::redundant_clone` (that lint, denied by Ruffle's workspace lints, still flags a
  few clones in tests and UI windows); CI runs exactly that (`.github/workflows/ci.yml`).
- **HTTPS in `turu_bot`:** the bot crate asks for reqwest's TLS itself, through its default
  `native-tls` feature (`rustls-tls` instead where there's no system TLS: Android); without one a
  `cargo test -p turu_bot` build could only fetch `http://` (the app got TLS from Ruffle's crates).
  `turu_frontend` takes `turu_bot` without default features: the app picks the backend.
- **Testing `turu_frontend`:** test it with the app (`cargo test -p turu -p turu_frontend` and the
  app's features as `turu/...`, as in `CLAUDE.md`). Alone, `ruffle_core` gets other features, so its
  build script compiles the AS3 playerglobals again, with Java and `ruffle/tools/asc/asc.jar`;
  that failed here with Java 25 ("Could not find or load main class
  macromedia.asc.embedding.ScriptCompiler"), while the app's cached build is reused.
- **Formatting:** the bot crate uses `cargo fmt`, and so does the shell library (`cargo fmt -p
  turu_shell`: its own `rustfmt.toml` sets `max_width = 120`, as `app/src/shell.rs` is). Not the app (Ruffle's files keep their own
  layout: `cargo fmt -p turu` rewraps 50 of them). The files in `egui_ui/src/gui/turu/` are
  formatted with `max_width = 120`, so format them with rustfmt rather than `cargo fmt`
  (`rustfmt --edition 2024 --config max_width=120 <file>`; on `mod.rs` it formats every
  window, which are all formatted). In `ruffle/`, `cargo fmt --all` formats Ruffle's workspace
  (Mugetsu's and Turu's changes included); the crates in `ruffle/patches/` are outside it:
  `rustfmt --edition <its Cargo.toml's edition> ruffle/patches/<crate>/src/lib.rs`.
- `docs/mugetsu-patch.md` describes what Mugetsu's patch changes, including the egui
  palette.
