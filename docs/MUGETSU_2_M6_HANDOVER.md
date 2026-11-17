# Mugetsu 2.0.0 and M6: handover

For whoever continues (a person or an agent). It says where things stand on 2026-11-17, what is
left, what was decided and why, and the traps hit on the way. Read `CLAUDE.md` first. The
background is in `docs/STATUS.md` ("Mugetsu 2.0.0", "Tabs", "Known gaps") and
`docs/TAB_PROCESSES_PROPOSAL.md` ("M6 as built").

Two pieces of work landed together on `main`:

1. **Mugetsu 2.0.0** (from 1.6.0): ruffle-aqw rebuilt on Mugetsu's new Ruffle base, Turu's
   commits replayed, the app ported to Mugetsu's 2.0.0 player changes.
2. **M6, one way of doing tabs** (`docs/TAB_PROCESSES_PROPOSAL.md`): every AQW tab is a process
   of its own behind a shell, on the desktop and on Android. The in-process tabs are gone.

## Where it stands

**Published** (2026-11-17):

| Repo | Branch | Tip | Notes |
|---|---|---|---|
| `muktiarafi/ruffle-aqw` | `aqw` | `121290d8d` | Force-pushed (new history): Ruffle `f22d29d9` + `chore: Apply Mugetsu 2.0.0 patches` (`56ff8f071`) + 32 Turu commits. PR #1 shows merged. `aqw`'s branch protection was put back as it was (force pushes off). |
| `muktiarafi/ruffle-aqw` | `aqw-2.0` | `121290d8d` | Same as `aqw`; can be deleted. |
| `muktiarafi/turu` | `main` | `e8d0213a4` | 13 commits from `14724433f` to `e8d0213a4` (list below). |

Local only (in this checkout's `ruffle/`): `aqw-1.6-backup` (the old `aqw`, Mugetsu 1.6.0) and
`aqw-2.0-before-m6-cleanup` (`aqw-2.0` before the two thread-state commits were dropped).

**CI:** green for `e8d0213a4` (https://github.com/muktiarafi/turu/actions/runs/44096883886):
format, clippy, tests and the host SWF, all on Ubuntu, so the app compiles and its tests pass on
Linux. **Nothing has been built on Windows**: CI has no Windows job (release.yml builds Windows
only from a `v*` tag). Since the Windows changes (open item 4) the app type-checks for Windows
from macOS (STATUS.md, "Things to know"), without linking or running.

**turu commits** (oldest first): `14724433f` Mugetsu 2.0.0 and its player options · `82398e4f1`
process tabs on by default (cherry-picked from `m3-android-tabs`) · `74424ce95` clients' memory
watch and Performance mode · `fc58bafc1` clients' player setup, GPU loss, cursor, perf log, close
· `54f714965` desktop always the shell · `cb8dce8d3` Android always slots, slot notification
buttons · `0484fc8be` Tab Limit across slots, Android in-process tabs gone · `7d2848fbf` one
player per process · `3862bc1fc` format/clippy · `24c1c0dcf` refresh rate to slots · `1f6218bdc`
docs · `4b075f9bf` follow Mugetsu where M6 removed the reasons not to · `e8d0213a4` Turu Mas lock.

**Checked** (macOS, Apple M5): `cargo nextest run -p turu_bot` (449 passed, before M6),
`cargo test -p turu -p turu_frontend` (50 + 131), clippy (CI's set; no new warnings), ruffle-aqw's
`cargo test -p ruffle_core --lib` (149) and `-p ruffle_render_wgpu --lib` (19), CI's ruffle-aqw
`cargo check --locked`, `cargo ndk check` of `turu_android` (Vulkan path) and of Turu Mas
(`air/ane/rust`), `./gradlew assembleDebug`. Runs: the shell and a client reach AQW's login
screen and draw 30 pictures a second; a client's GPU loss (`TURU_TEST_GPU_LOSS=1`) ends it and
the shell starts it again; the memory limit frees in a client
(`RUFFLE_TEST_MEMORY_LIMIT_MB=100`); Performance mode (window); uncapped pacing held the FPS
limit (120, 90) in the window. Android emulator (`turu_api36`, `-gpu host`): the shell and two
slots start, `java::check` passes in all three processes.

**Not checked:** anything in game with an account (no login was done), Windows (no build at
all), Linux beyond CI's build and tests (nothing run), real phones (M4), the slot notification's buttons (they show only with a script
loaded), the refresh rate reaching a slot, the cursor shapes on screen.

## What changed, in short

**ruffle-aqw** (Turu's commits on Mugetsu 2.0.0; the replay's choices):

- Dropped: Turu's "constructed", "dormant" and button marks (Mugetsu's lean walks,
  `lean-scripts`/`lean-frames`, do it), the old-movie-copies diagnostic (built on
  `Arc<SwfMovie>`, which upstream made a GC object), and since M6 the per-player thread-local
  state and segment swap (`PlayerThreadState`, `segments::swap`).
- Adapted: segment batching (16 per submit, Metal's 4096 command buffers) in `finish_all` and the
  shape-uploads path (`finish_in_batches`); the Vulkan command-buffer trim on by default before
  Mugetsu's opt-in pool switches; lazy shapes, `loaded_movie` (`Arc<SwfMovieData>`), the library
  sweeps on GC'd `SwfMovie`; the domain-group library fix capped by Mugetsu's `max_libraries`;
  "log repeated AS3 errors once" without its clip counts (`take_error_stats`).

**App** (`app/`, `frontend/`, `android/`):

- Mugetsu 2.0.0's speed-ups on by default (`frontend/src/platform.rs` `DEFAULT_ON`, set by
  `app/src/mugetsu.rs` `apply_drawing_switches` and `android/src/lib.rs`); `no-<name>` files turn
  one off. Forgiving child removal, least tessellation size by GPU, GPU state released at close.
- Mugetsu's 2.0.0 right-click menu (`app/src/gui/context_menu.rs`): Performance mode, memory
  limit, Free memory now, uncapped pacing, FPS limit. **Only the window for a movie given on the
  command line has it now**; AQW runs in clients (below).
- Removed: Safe Drawing (desktop and Android), Edge Smoothing (Mugetsu: always Flash-style, an
  `msaa` file brings MSAA back), Mugetsu's update reminders (Turu has its own updates).
- Kept as Turu's (not ported): `app/src/windows.rs` (Mugetsu's crash-handler rework), the graphics
  restart dialogs, `gui/controller.rs`'s window drawing (Mugetsu's present-thread rewrite).
- M6, desktop: `shell::runs_as_shell` (every AQW start, Army's "In its own window" too);
  `--in-process` accepted and ignored; the window keeps one player, no tabs or Grid View.
  Clients got: `app/src/memory.rs` (memory watch, limit, Free memory; every 5 s), Options >
  Application's Performance Mode / Memory Limit / Free Memory Now (`Platform` methods,
  `frontend/src/client_screen.rs` scales the viewport and the mouse), `headless::build_player`
  with the window's setup, GPU loss → exit 86 → the shell restarts it (`client.rs` `gpu_check`),
  `ToShell::Cursor`, the perf log, the engine thread's close.
- M6, Android: the activity is always the shell; no engine in the app's process; `TuruService`
  is a holder of statics (crash reports, alerts, GPU warnings); `SlotService` notification
  buttons (`nativeAction`), tabs log per slot, Tab Limit counts the shell's tabs
  (`gui/turu/tab_limit.rs`), `ToClient::RefreshRate` / `ToShell::PreferRefreshRate`.
- `frontend/src/players.rs` holds one player; `engine::run_loop_paced` has no hidden tabs.

## Open items, in the order recommended

1. **A Windows build** (no CI job: a Windows machine, or release.yml's Windows job on a test tag).
   The app now type-checks for Windows from macOS (STATUS.md, "Things to know"; done after item
   4 below), but nothing has linked or run there. Most at risk: `app/src/windows.rs`, the
   `#[cfg(windows)]` paths in `app/src/client.rs` and `shell.rs`, and the D3D12 shared surfaces
   (`shared_gpu.rs`), which every Windows user now takes, DirectX 12 being the default. Then run
   the shell with two tabs on Windows and on Linux (Linux only compiled in CI).
2. **In game, with an account** (needs the user's login; use a quiet server, not Twilly, Artix
   or Alteon): two tabs with a `lean-verify` file next to the program (`RUFFLE_LEAN_VERIFY`; look
   for "LEAN VERIFY" in the log); a client's Options > Performance Mode, Memory Limit, Free Memory
   Now; a farm of 20+ minutes. Re-measure against the dropped marks' numbers (a frame's game logic:
   Battleon 2.2 → 1.3 ms, Yulgar 3.1 → 2.1, /fotia 4.9 → 1.95 with them; `TURU_PERF_LOG`) and lazy
   shapes (1331 → 5 shapes parsed at the login screen).
3. **Frame interpolation in clients**: the pacer and uncapped pacing done (2026-11-18, branch
   `client-interpolation`, cut from `main`; STATUS.md, "Frame interpolation in a tab's client"). The client plans
   with Turu's pacer (`client_screen.rs` `plan_pace`) and tells the shell (`ToShell::Pace`); the
   shell presents on every refresh and ticks the client (`ToClient::Refreshed`), which draws only
   on those ticks; Frame Pacing and FPS Limit are in client Options. Shared surfaces went from 3 to
   5. Checked on the Mac only (login screen, no account). Left: Mugetsu's render thread
   (`ruffle_render_wgpu::render_thread::ThreadedBackend`; `client_screen.rs`'s
   `downcast_ref::<WgpuRenderBackend<MovieTexture>>` must go through
   `render_thread::with_wgpu_backend`); a run in game (a crowded room, where the pacer steps down);
   Windows and Linux, where the shell's present mode switch (`apply_pace`) and whether presenting
   waits for the refresh (the shell's ticks rely on it) are untried; Grid View's active tab
   interpolating (should, untried); Turu's pacer counts only `player.render()`, not the pictures
   skipped because one is still under way or no surface is free (the perf log counts those: feed
   them back into its divisor, as Mugetsu's window does with `PipelineLimit`, if a crowded room
   shows them). A picture is the shell's for about 4 refreshes, mostly
   waiting for the shell's release poll while its main thread waits in `get_current_texture`: a
   release thread could bring the pool back down.
4. ~~**Mugetsu's Windows changes**~~ ported (2026-11-17, type-checked for Windows only):
   DirectX 12 as Automatic's choice on Windows (`mugetsu::automatic_graphics`, unit-tested), the
   DirectX 12 crash marker, the Vulkan retry when DirectX 12 can't start (`mugetsu::open_gpu`, in
   the window, the shell and the clients), the crash-handler rework, exit phases, the DXGI
   adapter let go of at close, and `process::exit` after the window's or the shell's close.
   Choices: Mugetsu's `crash.txt` became Turu's per-process native report (`crash_uploads::
   NativeReport`, written part by part), as the shell and its clients would overwrite one file;
   Mugetsu's restart flow (`should_start_again`, `start_again_now`) isn't taken, Turu has its own.
   Left: a client that crashes while DirectX 12 starts saves the marker and comes back on Vulkan
   while the shell stays on DirectX 12, so that tab's pictures are copied, not shared, until Turu
   starts again (the same as the Vulkan crash before). To check on Windows:
   `RUFFLE_TEST_DX12_FAILS=1` (Vulkan taken, "DirectX 12 not available" in the title),
   `RUFFLE_TEST_NATIVE_CRASH=driver` with `RUFFLE_TEST_DRIVER_FILE=kernelbase.dll` (the Vulkan
   or DirectX 12 crash dialog, the marker, the report in `crash/`), and a few closes on DirectX 12.
   On macOS the shell's new close was run (`TURU_SHELL_TEST` line `quit`): exit code 0, its last
   log line written, no client left; the shell kills its clients when it closes, so a client's
   own close path runs only when it loses the shell otherwise.
5. **Client gaps left from M6**: IME composition, gamepads, drag-and-drop, Flash's own right-click
   items; Mugetsu's one-time tips in clients (Performance mode offer, Quality Low tip: they need
   `mugetsu::check_gpu_choice`, which only the window calls).
6. **Android**: a slot that loses its GPU goes on without a picture (the desktop client restarts);
   Mugetsu's options aren't in Android's Options; TuruActivity's `?: native…` fallbacks for the
   text field and documents are unreachable now and can go with their natives (`android/src/java.rs`
   `TuruActivity` binding, `ui_field.rs`, the document natives). (The web monitor and client
   update checks are fine: the slots elect one of them to run both, `turu::start_monitor_election`.)
7. **Cleanups**: delete `aqw-2.0` on ruffle-aqw and the two local backup branches once `main`'s
   CI is green; `app/src/mugetsu.rs` `log_folder` is unused (warning from before).

## Decisions taken (and why)

- Replay Turu's commits rather than re-derive them; stop at semantic conflicts and ask. Asked
  and answered: batching in `finish_all` and the uploads path; Vulkan trim on by default beside
  Mugetsu's switches; drop the leak diagnostics, keep the domain fix; drop Turu's still-clip
  marks for Mugetsu's lean walks.
- Don't take Mugetsu's 2.0.0 `windows.rs`, `gui/controller.rs` and `app.rs` wholesale: they're
  built around Mugetsu's present thread, update checker and restart flow, which collide with
  Turu's crash reports, Grid View and own updater. Options were ported by hand instead.
- M6 now, before M4 and M5's Windows/Linux runs (the user's call); keep a one-tab window for
  non-AQW movies; close the clients' core gaps first and defer interpolation, IME, gamepads,
  drag-and-drop.
- After M6, follow Mugetsu where Turu's reasons are gone: the thread-state commits dropped, the
  present thread no longer forced off. The render thread waits for interpolation in clients.

## Traps

- **A worktree's target dir:** building ruffle-aqw from a temporary worktree into a kept target
  folder bakes the worktree's path into the `asc` crate (`env!("CARGO_MANIFEST_DIR")` → its
  `asc.jar`). After the worktree is deleted, `ruffle_core`'s build script fails with "Could not
  find or load main class macromedia.asc.embedding.ScriptCompiler". Fix: `cargo clean -p asc -p
  build_playerglobal` in that target.
- **Pushing:** SSH to GitHub fails in these sessions; push over HTTPS with gh's credential
  helper (the agent memory `github-push-auth`). ruffle-aqw's `aqw` is protected (linear history,
  checks "Rebased on aqw" and "Check", no force pushes): a rebuilt history needs the protection
  relaxed and put back, with the user's yes.
- **Three-way merging `desktop/` into `app/`** (`git merge-file` with Mugetsu 1.6's file as base)
  gives clean-looking merges that interleave unrelated code; read every merged region.
- **Kotlin and Rust bindings** (`android/src/java.rs`) are checked only when the app starts
  (`java::check`, "Kotlin bindings checked: 5 classes" in logcat); no CI builds the Android crate.
- **Settings in tests:** run the app with a throwaway `HOME` and `TURU_DATA_DIR` (Mugetsu's
  settings live in `~/Library/Application Support/ruffle` on macOS) and the `RUFFLE_TEST_*`
  switches (`RUFFLE_TEST_PERFORMANCE`, `RUFFLE_TEST_MEMORY_LIMIT_MB`, `RUFFLE_TEST_UNCAPPED`,
  `RUFFLE_TEST_FPS_LIMIT`) instead of the user's saved choices.
- **Updating Mugetsu next time:** CLAUDE.md's flow holds. Mugetsu ships `source/RUFFLE_COMMIT`,
  `patches/ruffle.patch` and five crate patches applied as `source/build-macos.sh` does (crate
  versions in that script); replay with `git cherry-pick <old base>..aqw` onto the new base and
  compile after each conflict, as core API changes (this time `SwfMovie<'gc>`) break commits
  that apply cleanly.
