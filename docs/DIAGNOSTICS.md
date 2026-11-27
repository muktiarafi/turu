# Measuring Turu

For working on Turu: where the game thread's time and the memory go, measuring a long run,
recording what passes between the bot and the game, and a speed profile. Running Turu on a
server is `docs/HEADLESS.md`. Most of this works with a window too; it was built on
windowless runs, and the findings so far are in `docs/HEADLESS_MONITORING_HANDOFF.md` and
`docs/STATUS.md`.

## The game thread: `/debug/perf` and `TURU_PERF_LOG`

`GET /debug/perf` (the control API) lists the Flash calls the client made since it started (by
thread and callback, with the path for `getGameObject` and its kin: how many, how long they
waited, how long the player took to run them, reply bytes) and its player's ticks.

`TURU_PERF_LOG=<seconds>` logs the same for each tab every `seconds`, over that window
(`turu_perf` lines: the player thread's milliseconds per second, a `waits:` line with each
`bot.wait.forX` that ended in the window, how many timed out and their average time, then the
busiest calls, 12 unless `TURU_PERF_LOG_ROWS` says how many). With a window it covers every tab,
plus a total line and a `main thread` line: where the event loop's time goes (idle, drawing and
its parts, the games and bot calls, the rest). `TURU_CACHE_CENSUS` (with a window) adds a
`cache census:` line every window: the live cacheAsBitmap textures, how many of them objects on
the stage and orphans hold, and the classes holding the most.

## Memory

Without a window, `TURU_PERF_LOG` also logs a `memory:` line: the process's resident and
committed memory (with peaks), the Rust heap that's live, what the allocator (mimalloc) has
committed, in how many pages (how many abandoned by threads that ended) and reserved, on Windows
the system heaps' committed memory (the graphics driver's and the system libraries' share;
Mugetsu's `memory_split`, at the end of the line), the SWFs the game loaded (their data in MB)
and the Flash objects it holds, to tell a leak from the allocator keeping freed memory. Off
Windows mimalloc estimates the process's commit from its own, so the two read the same there;
its pages and reserved memory are what's new. With `TURU_PERF_LOG`, Free Memory Now also logs
the allocator's full statistics right after (`allocator statistics after it:`).
`TURU_MI_COLLECT=0` stops mimalloc handing its free memory back to the system once a minute (to
compare).

Two lines follow it:

- `memory census:` the URLs with the most live movies (images with their decoded size), so a
  file loaded again and again and never freed stands out;
- `memory biggest:` the URLs whose live movies hold the most memory (decompressed SWF data, plus
  decoded pixels for images), with their copies: what a map, a monster or a piece of gear costs
  while it stays loaded, and which stays loaded when it shouldn't.

An `uncaught AS3 errors` line counts the game's uncaught AS3 errors and how many were logged (a
repeat is logged once, then only counted; `RUFFLE_ERRORS_EVERY_TIME=1` logs each).

`TURU_RETAIN_URL=<part of a URL>` adds, every 5th reading, how many `LoaderInfo`s of matching
movies are alive and the chain of references that keeps the oldest, middle and newest alive (a
walk of the whole heap: a short pause).

`tools/memrun/run.sh [script] [minutes]` runs a long session this way, logging in from
`turu-args.txt`, and `tools/memrun/summarize.py` prints the trend per hour.

To see what holds the heap, run with `TURU_SYSTEM_ALLOC=1`: the system allocator instead of
mimalloc (the memory line's commit then reads ~0), which heap profilers see: on macOS
`MallocStackLogging=1`, then `malloc_history <pid> -allBySize` (live memory by call stack) or
`-callTree -invert`; on Linux `heaptrack`. Use a debug build, which has the symbols.

## The player's switches

Clips that stand still are skipped by Mugetsu 2.0.0's lean walks, which Turu turns on as
Mugetsu does (`lean-scripts`, `lean-frames`: `platform::DEFAULT_ON`); a `no-lean-scripts` or
`no-lean-frames` file next to the program turns one off, and `RUFFLE_LEAN_VERIFY=1` checks what
they skip.

`RUFFLE_JSON_TREE=1` makes `JSON.stringify` build its tree again for every call (the bot's reads
of the game go through it), and `RUFFLE_JSON_CHECK=1` builds it too and logs an error when the
direct text differs.

`TURU_LEVER_AB=<lever>:<seconds>` turns one of turu.swf's measuring levers (`stopAnimations`,
`eventsOff`) on and off in turns while in game, logging each turn and where the playing clips
are (HOST_SWF_PROPOSAL.md M7a).

## Measuring a farming run

`tools/farmrun/run.sh [script] [minutes]` (default FarmerJoe DoAll, 120 minutes; `0` runs until
the script stops) logs in from `turu-args.txt` as memrun does, with `TURU_PERF_LOG` and a control
API of its own (`API`, default `127.0.0.1:9031`). Every `PERIOD` seconds (60) it adds a
`farmrun:` line to the log (`tools/farmrun/sample.py`): `/status`'s level, map, cell, kills,
quests, deaths and relogins, and what the process uses:

- CPU time, user and system;
- resident memory, and macOS's footprint or Linux's PSS;
- threads, and open files and sockets;
- network bytes (`nettop` on macOS; on Linux the network namespace's);
- disk bytes (Linux only: macOS has no per-process disk I/O without root);
- the log's size.

It says at once when nothing has moved for `STUCK_MIN` minutes (10: no change in those fields
and no script log line; deaths don't count as moving) or the client isn't logged in.
`tools/farmrun/summarize.py <log>`, which also works on a run still going or on any Turu log,
reports:

- progress: kills, quests, deaths and relogins per hour, when each level came, time per map;
- the script's sections of 2 minutes or more, by the function that logged them;
- stretches without progress, skill-timer stalls and AFK messages;
- server warnings, refused or unanswered turn-ins, relogins and errors;
- with `TURU_PACKET_LOG=1` (every packet the client sends goes to the log too, as `turu::packet`
  lines, as Diagnostics > Logger does with a window), the packets sent by command and the ones
  sent in the 2 s before each "Please slow down" (the server drops the action it calls too soon);
- resources: CPU (average, 95th percentile, user and system, per kill), memory and its growth per
  hour after the first 10 minutes (with the `memory:` lines' Rust heap, loaded SWFs and Flash
  objects, to tell a leak from warm-up; the biggest loaded files at the start and the end),
  threads, open files, network and disk rates, the log's growth, the game thread's time and the
  busiest bot calls.

It also writes the samples to `<log>.csv`.

## Recording a tape

`TURU_TAPE=<file or folder>` (or `--record-tape <file or folder>`; with `--shell`, its first
tab's client records) records what passes between the bot and the game: each call into Flash
with its answer, each call from `turu.swf` and each message of the game's socket, with the time
and the calling thread, plus a note when a script starts (its path under the scripts folder and
its options' values) and stops (docs/REPLAY_TESTS_PROPOSAL.md).

- **The file.** A gzip-compressed JSON Lines file, `*.tape.jsonl.gz` (`turu_bot::tape::read`
  reads one). A folder gets a new file per client (`turu-<time>-<pid>.tape.jsonl.gz`); an
  existing file is never written over (the name gets `-2`, `-3`, ...), so give tabs and Army
  clients a folder.
- **When.** Recording starts when the player first joins a map; `TURU_TAPE_FROM_START=1` records
  from the start, for a relogin.
- **Redaction.** Passwords (`loginInfo` reads, the game's `login` call), session tokens (the
  socket's login, JSON fields named like `sToken`), the account's email and IP address
  (`initUserData`) and chat (sent, received, whispers) are replaced with `<redacted>` as each
  line is written. Player names are kept, so a tape is private until it is scrubbed:
  `cargo run --release -p turu_bot --bin turu-tape -- scrub <tape> <new file>` renames every
  player (the account "Questwalker"), renumbers the account's IDs and checks the result;
  `turu-tape info <tape>` says what a tape holds.
- **Playback.** A scrubbed tape plays back to its script in a test, with no game
  (`turu_bot::testing::ReplayHost`, `bot/tests/replay.rs`).
- **Writing.** A writer thread of its own compresses and writes, flushing every second: a killed
  process leaves a tape readable up to its last second, without its `end` line. Lines the writer
  couldn't keep up with are dropped, never waited for, and counted in the `end` line (and as gaps
  in their numbers). Quitting through Turu finishes the file: `--exit-on-script-stop` (or
  `POST /script/stop` with it), an update, closing the window. A signal (`systemctl stop`,
  Ctrl+C) doesn't: the tape then ends at its last flush.
- **Cost.** FarmerJoe DoAll writes about 9 MB of tape an hour (70 lines a second); the writer's
  work is negligible in a release build.

Use a quiet server and a spare account: a tape holds the account's name and everything it saw.
The redaction was checked on one 20-minute farm run (the password and token absent); before
sharing a tape made with another account or other content, search the decompressed tape for
that account's password (`gunzip -c <tape> | grep -cF '<password>'` says 0).

## A speed profile (PGO)

`tools/pgo/pgo.sh all [script...]` builds a `dist` binary that counts what runs (`gen`), logs in
from `turu-args.txt` as farmrun does and plays each script for `MINUTES` (20) with a window and
headless (`record`, `MODES`), merges the counts (`merge`, into
`tools/pgo/out/turu-<target>.profdata`; needs `rustup component add llvm-tools`) and builds
`target/<target>/dist/turu` with them (`use`).

- Each run ends with `POST /script/stop` and `--exit-on-script-stop`: only a clean exit writes
  the counts, so a killed or crashed run's are lost.
- Only Rust is profiled: the C code (mimalloc, QuickJS) goes through `tools/pgo/cc`, which drops
  the profile flags cc-rs passes on (the system's clang writes another profile format, and the
  instrumented mimalloc crashed on it).
- A headless run draws nothing, so its counts say nothing about drawing; the window's do.
- It needs a quiet server (`SERVER`, and refuses Twilly, Artix and Alteon) and a spare account,
  and a new recording after large changes (a profile covers only the code it saw).

Mugetsu's own profile (`pgo/aqw.profdata`) applied to Turu changed the render thread little
(STATUS.md); compare a `use` build with a plain `dist` one on the perf log's lines before
shipping one.

## Variables

| Variable | What |
|---|---|
| `TURU_PERF_LOG` | Every this many seconds, log each tab's Flash calls and player ticks (and memory, without a window). |
| `TURU_PERF_LOG_ROWS` | How many calls each of those lists (default 12). |
| `TURU_SYSTEM_ALLOC` | Set: the system allocator instead of mimalloc, for heap profilers. |
| `TURU_MI_COLLECT` | `0`: don't make mimalloc hand its free memory back to the system once a minute (to compare). |
| `TURU_RETAIN_URL` | Without a window, with `TURU_PERF_LOG`: what keeps the `LoaderInfo`s of movies whose URL contains this alive, every 5th memory reading. |
| `TURU_CACHE_CENSUS` | With a window and `TURU_PERF_LOG`: a `cache census:` line every window. |
| `TURU_PACKET_LOG` | Set: every packet the client sends goes to the log too (`turu::packet` lines). |
| `TURU_TAPE` | A file or folder: every client records a tape there. |
| `TURU_TAPE_FROM_START` | `1`: tapes record from the start, the login included (redacted), not from the first map. |
| `TURU_LEVER_AB` | `<lever>:<seconds>`: turns one of turu.swf's measuring levers on and off in turns while in game. |
| `TURU_GRID_TEST` | With a window: hidden tabs run at this frame rate and draw every frame (to measure a grid of tiles). |
| `TURU_TEST_PANIC` | Set: panic 10 s after the start, to test crash reports (`sendCrashReports: true` in `settings.json` sends them; there is no question without a window). |
| `RUFFLE_ERRORS_EVERY_TIME` | `1`: log every uncaught AS3 error, not each once. |
| `RUFFLE_LEAN_VERIFY` | `1`: check what the lean walks skip. |
| `RUFFLE_JSON_TREE`, `RUFFLE_JSON_CHECK` | `1`: `JSON.stringify` builds its tree for every call / builds it and logs where the direct text differs. |
