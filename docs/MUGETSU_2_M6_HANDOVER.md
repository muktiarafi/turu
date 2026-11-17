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

**CI:** the run for `e8d0213a4` was still going when this was written
(https://github.com/muktiarafi/turu/actions/runs/44096883886). It is the **first compile on
Windows and Linux** of everything below: check it before anything else.

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

**Not checked:** anything in game with an account (no login was done), Windows and Linux (no
build at all), real phones (M4), the slot notification's buttons (they show only with a script
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

1. **CI on Windows and Linux** (the run above). Most at risk: `app/src/windows.rs` (merged by hand,
   never compiled), `app/src/shell.rs`'s `set_cursor`, the Linux shared-surface client paths. Fix
   on `main`; nothing here was built on those systems.
2. **In game, with an account** (needs the user's login; use a quiet server, not Twilly, Artix
   or Alteon): two tabs with a `lean-verify` file next to the program (`RUFFLE_LEAN_VERIFY`; look
   for "LEAN VERIFY" in the log); a client's Options > Performance Mode, Memory Limit, Free Memory
   Now; a farm of 20+ minutes. Re-measure against the dropped marks' numbers (a frame's game logic:
   Battleon 2.2 → 1.3 ms, Yulgar 3.1 → 2.1, /fotia 4.9 → 1.95 with them; `TURU_PERF_LOG`) and lazy
   shapes (1331 → 5 shapes parsed at the login screen).
3. **Frame interpolation in clients** (its own task; the biggest gap). Clients draw once per game
   frame (`client_screen.rs` `step`). Needed: Turu's pacer (`frontend/src/pacing.rs`) in the
   client, the shell presenting in step (it shows the client's shared surface), then Mugetsu's
   uncapped pacing and FPS limit in client Options (the window's code: `app/src/app.rs`
   `pace_interpolation`, `gui/controller.rs` `set_vsync`), then Mugetsu's render thread
   (`ruffle_render_wgpu::render_thread::ThreadedBackend`; `client_screen.rs`'s
   `downcast_ref::<WgpuRenderBackend<MovieTexture>>` must go through `render_thread::with_wgpu_backend`).
4. **Mugetsu's Windows changes** (need Windows): DirectX 12 as the default with a Vulkan fallback
   and a crash marker (`ruffle/desktop/src/main.rs` and `mugetsu.rs` `graphics_startup_begin`,
   `save_dx12_crash`, `dx12_crashed_before`; Turu still defaults to Vulkan), the crash-handler
   rework (`crash.txt`, raw stack), releasing the DXGI adapter at close.
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
