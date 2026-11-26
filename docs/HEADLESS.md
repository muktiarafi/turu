# Running Turu without a window

Run Turu on a Linux server, or any machine you don't sit at: no display, GPU or Xvfb needed.
It logs in, runs a script or a playlist, and you watch and steer it over HTTP. The same flags
work on macOS and Windows.

> **Status:** windowless runs have logged in, farmed for hours and relogged after a drop
> (`docs/STATUS.md`). Watch your first runs.

## Quick start

Unpack `turu-<version>-linux-x86_64.tar.gz` from
[Releases](https://github.com/muktiarafi/turu/releases), e.g. into `/opt/turu`, and install
the libraries it needs (Debian/Ubuntu; on Ubuntu 24.04 use `libasound2t64`):

```bash
sudo apt install libasound2 libudev1 libssl3 libfontconfig1 libfreetype6
ldd /opt/turu/turu | grep "not found"     # should print nothing
```

Then:

```bash
export TURU_PASSWORD='your password'      # not --password: other users can read a command line
/opt/turu/turu --headless --user YourName --server Twilly --run-script farm/GoldFarm
```

It logs in once the game has loaded, then starts the script; the log goes to the terminal.
Ctrl+C quits. Everything Turu keeps is in its data folder, `~/.local/share/Turu` (or
`TURU_DATA_DIR`).

## Scripts, options and playlists

**Script paths** (`--run-script`, `--load-script`, playlist entries) are absolute, or relative
to the data folder's `scripts` folder, then the working folder; `.ts` is optional. A relative
path that Search Scripts lists is downloaded or updated, with what it imports, before the game
starts; scripts you edited are kept. Offline, clone
[turu-scripts](https://github.com/muktiarafi/turu-scripts) and pass absolute paths.

**Options:** there's no Script Options window, so a script runs with its saved options or its
defaults, and its log says which (`Script Options not shown (no window): ...`); with the defaults
the web monitor shows an event too, and `--run-script` warns at startup. Set them without a
desktop in the web monitor (a bot's Details > Script Options) or with the control API:

```bash
curl -H "$T" localhost:9091/script/options        # the loaded script's options and values
curl -H "$T" -X POST -d '{"UseBoost": true, "Group": {"Count": 5}}' localhost:9091/script/options
curl -H "$T" -X POST "localhost:9091/script/options?reset=1"     # back to the defaults
curl -H "$T" "localhost:9091/scheduler/options?id=<entry id>"    # a playlist entry's (GET or POST)
```

Or copy these from a desktop's data folder to the server's:

| File | What |
|---|---|
| `options/<storage>.cfg` | A script's options. |
| `options/Queue_<script>_<id>.cfg` | A playlist entry's options. |
| `options/CBO_Storage(<username>).txt` | Options > CoreBots for that character. |
| `options.json` | Options > Game. CoreBots scripts turn Auto Relogin on themselves; for others set `autoRelogin`. |
| `UserAdvancedSkills.json` | Your skill sets. At launch the community's new classes and modes are added unless Options > Application turns off an AdvanceSkill Sets switch. |
| `settings.json` | Options > Application: webhooks (they work without a window), relogin delay. |
| `butler.json` | Butler's options. |

**Playlists:** build one in Scripts > Scheduler on a desktop, set each entry's options, Save
it (scripts in the scripts folder are saved relative to it), and copy it with its
`options/Queue_*.cfg` files. Then:

```bash
/opt/turu/turu --headless --user YourName --run-playlist ~/playlists/daily.json
```

Its scripts are downloaded like `--run-script`'s; entries whose script isn't found are left
out (the log says how many loaded). The file is VibeSkua's playlist JSON
(`[{"Path", "Id", "Name"}]`, `Id` any GUID), so you can also write one by hand.

## Run it as a service

One systemd instance per account, restarted after a crash, logging to journald. Create a user
(`sudo useradd -r -m -d /var/lib/turu turu`) and `/etc/systemd/system/turu@.service`:

```ini
[Unit]
Description=Turu (%i)
Wants=network-online.target
After=network-online.target

[Service]
User=turu
EnvironmentFile=/etc/turu/%i.env
StateDirectory=turu
Environment=TURU_DATA_DIR=/var/lib/turu
ExecStart=/opt/turu/turu --headless --user ${TURU_USER} --server ${TURU_SERVER} \
    --run-playlist /var/lib/turu/playlists/%i.json --api=${TURU_API} $TURU_ARGS
Restart=on-failure
RestartSec=30

[Install]
WantedBy=multi-user.target
```

Each account gets `/etc/turu/<account>.env`, readable by root only
(`sudo install -m 600 /dev/null /etc/turu/main.env`):

```bash
TURU_USER=YourName
TURU_PASSWORD=your password
TURU_SERVER=Twilly
TURU_API=127.0.0.1:9091
# The control API token: 32 characters at least, e.g. from `openssl rand -hex 32`
TURU_API_TOKEN=a long random string
# Extra flags, e.g. --monitor=127.0.0.1:9021 (systemd reads no comments after a value)
TURU_ARGS=
```

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now turu@main
journalctl -u turu@main -f
```

* A restart starts the playlist again from the top.
* With `--exit-on-script-stop` it exits with status 0 when done, which `Restart=on-failure`
  leaves stopped; use `Restart=always` to loop.
* `systemctl stop` doesn't log out; the game server drops the connection shortly after.

## Docker

The `Dockerfile` builds Turu from this checkout (with the `ruffle` submodule checked out) and
runs it with `--headless` as user `turu`, on Debian slim; it builds for the machine's own
architecture (x86_64 or arm64). The data folder is `/data`, a volume.

```bash
git submodule update --init ruffle
cp docker/turu.env.example docker/main.env      # fill it in; git ignores docker/*.env
docker compose up -d --build
docker compose logs -f main
```

The first build takes a while (the `dist` profile, as the release packages, with LTO); later
ones reuse BuildKit's cargo cache. `--build-arg PROFILE=release` builds faster,
`--build-arg TURU_VERSION=0.4.0` sets the version Turu reports and compares with its releases.

`docker/entrypoint.sh` turns the env file into flags: `TURU_USER`, `TURU_SERVER`,
`TURU_RUN_SCRIPT` / `TURU_LOAD_SCRIPT`, `TURU_RUN_PLAYLIST` / `TURU_LOAD_PLAYLIST`,
`TURU_API` (the image sets `0.0.0.0:9091`; empty turns it off), `TURU_MONITOR`, `TURU_REPORT_TO`;
`TURU_PASSWORD`, `TURU_API_TOKEN` and the other variables (Reference, below) are read as they are.
Arguments after the image name are added last, so they win:

```bash
docker run --rm --env-file docker/main.env -v turu-data:/data turu --exit-on-script-stop
```

* `compose.yaml` publishes the control API and the web monitor on the host's loopback only.
  Requests come through Docker's network, so the web monitor asks for its key
  (`http://localhost:9021/#key=<key>`, the key from `docker compose exec main cat /data/monitor-key`);
  the control API wants its token as always.
* With a memory limit (`docker run --memory=1g`, or `mem_limit` in `compose.yaml`), the web
  monitor's machine strip shows the container's use of it ("Container 0.4 GB of 1.0 GB"), read from
  the cgroup: the free memory beside it is the host's.
* Several accounts: one service each, all on the `turu-data` volume (as "Several accounts"
  below), each with its own env file and host port; the others report to the first one's
  monitor at `http://main:9021`. `compose.yaml` has one commented out.
* Settings, options and playlists go in the volume: `docker compose cp daily.json
  main:/data/playlists/` (then `TURU_RUN_PLAYLIST=/data/playlists/daily.json`), or the control
  API and the web monitor as below.
* `restart: on-failure`, as the service above: a crash restarts it, `--exit-on-script-stop` ends
  it. `POST /update` replaces the binary inside the container, which lasts until the container
  is recreated; rebuild the image to keep a new version.
* `docker compose stop` doesn't log out either.

## Several accounts

Run one instance per account, all with **one data folder** (as above) and each with **its own
`TURU_API` port** (`9091`, `9092`, ...). The shared folder is how clients find each other:
Butler's sync files, army scripts' sync logs and their private room number
(`bot.army.roomNumber()`, a hash of the folder's path) all live there. Start the instances a
few seconds apart, so they don't update the same scripts at once.

Have one serve the web monitor and the others report to it:

```bash
# The first account's env file:
TURU_ARGS=--monitor=127.0.0.1:9021
# The others' (the key file appears after the first one's first run):
TURU_ARGS=--report-to http://127.0.0.1:9021
TURU_MONITOR_PUSH_KEY=<contents of /var/lib/turu/monitor-push-key>
```

Leave Enable Web Monitor off in the shared `settings.json`, or every client tries to serve it.
The reporters all show under the machine's name, one row per client (each run reports with
an id of its own, so sharing the name and settings is fine). The machine gets one CPU and memory
line for all of them ("Turu ×3"), with each process in its details; so do containers on one host,
which share the kernel's boot id. "Needs you" says when a machine is almost out of memory, a
container is near its limit, the CPU has been over 90% for 5 minutes, or a Turu process keeps
growing (over 100 MB an hour).

**Army:** Army > Accounts and Army Control need a window. On a server the systemd unit is the
launcher, and the web monitor covers most of Army Control (Start / Stop / Relog / Join map on
one bot or several). Army scripts run one per account as on a desktop. Scripts that call
`bot.army.login` read passwords from the system keychain, which a server usually doesn't run,
so they fail there.

**Butler** works between clients sharing the data folder, so on one machine only. A follower
obeys summons as it does with a window; if a script starts while it follows, it stays
suspended until the script stops. A master summons with the control API (below). Turn Obey
Summons off in `butler.json` for the account you play by hand.

## Watching and steering it

**Log:** stdout, so `journalctl -u turu@main -f` under systemd. `RUST_LOG=warn,turu=debug`
shows more. Text layout's "Fallback font not found" errors are left out (no device fonts are
loaded without a window, and nobody sees the text); a `RUST_LOG` of your own brings them back.

**Web monitor** (`--monitor=127.0.0.1:9021`): each bot's state, last-hour trace, logs, history and
commands, and a line for the machine: its CPU and free memory, and Turu's own CPU (a share of one
core, as `top` shows it) and memory, with the last hour of Turu's memory (click it for details).
Keep it on loopback and use a tunnel, which needs no key:

```bash
ssh -L 9021:127.0.0.1:9021 you@server     # then open http://localhost:9021
```

From anywhere else (Tailscale Serve, a proxy, `--monitor=0.0.0.0:9021`) open
`http://<address>:9021/#key=<key>` with the key from the data folder's `monitor-key`. Anyone with
the key can run commands on your bots.

**Report to your desktop instead:** on the desktop, Options > Application > Monitor: Enable
Web Monitor, Allow Other Devices, Copy Report Settings. On the server, `--report-to
http://<desktop>:9021` with the push key in `TURU_MONITOR_PUSH_KEY`. The server's bots show on
the desktop's page and take its commands.

**Control API** (`--api`): every request needs the token (`TURU_API_TOKEN`, else the data
folder's `api-token`). All routes, and what each status means, are listed in
`bot/src/control_api.rs`.

```bash
T="Authorization: Bearer $TURU_API_TOKEN"
curl -H "$T" localhost:9091/status
curl -H "$T" "localhost:9091/log?since=0"
curl -H "$T" -X POST "localhost:9091/script/start?path=farm/GoldFarm"
curl -H "$T" -X POST localhost:9091/script/stop      # with a playlist: moves on to the next
curl -H "$T" localhost:9091/scheduler
curl -H "$T" -X POST --data-binary @daily.json localhost:9091/scheduler/load
curl -H "$T" -X POST localhost:9091/scheduler/start  # and /scheduler/stop
curl -H "$T" localhost:9091/butler                   # online accounts, who can be summoned
curl -H "$T" -X POST "localhost:9091/butler/summon?user=MyAlt"   # also release, summon-all,
                                                                 # release-all, follow, unfollow
curl -N -H "$T" "localhost:9091/events?logs=script,debug"       # follow it, instead of polling
curl -H "$T" localhost:9091/tabs                     # with a window (or --shell): the tabs
curl -H "$T" localhost:9091/tabs/2/status            # any route, for one tab
```

`/events` sends server-sent events: `script` (path, status, running) when it changes, `log`
for each new line of the logs you ask for (`script` by default), `event` for each web monitor
event, and a comment every 15 s. It reads the bot's state, never the game, so it costs nothing
while idle; `/status` asks the game each time. Playlist paths in `/scheduler/load?path=` and
`/scheduler/save?path=` are relative to the data folder's `playlists` folder, or absolute inside
the data folder.

**Updates:** it checks Turu's releases at launch and every 6 hours (Options > Application >
Check for Client Updates in the shared `settings.json`) and only tells you: a log line
(`Update available: Turu X (you have Y): <release page>`), `update` in `/status` (version,
current, url, download, prerelease; `null` when up to date) and in the web monitor's summary.
To install it, `POST /update` (409 while a script runs, unless `?stop=1`): it downloads the
package, checks it against the release's `SHA256SUMS.txt`, replaces the program and exits with
status 75, which `Restart=on-failure` restarts on the new binary. The service's user must be
able to write the program's folder (`/opt/turu` above is root's: `chown` it to that user, or
update by hand). `SHA256SUMS.txt` proves the download is intact, not who published it.
A nightly build (the `nightly` release, `0.4.0-nightly.<date>`) never checks: `update` stays
`null` and `POST /update` refuses (`not_supported`). Replace it by hand.

```bash
curl -H "$T" localhost:9091/status | jq .update
curl -H "$T" -X POST localhost:9091/update            # ?stop=1 while a script runs
```

A command answers with what it changed: the script for `/script/*`, `/status` for the logins,
where the player is for the moves, the playlist for `/scheduler/*`. A failure is
`{"error": "...", "code": "..."}`: script on `code` (`not_logged_in`, `script_running`,
`login_under_way`, `game_not_running`, `game_timeout`, ...; `bot/src/control_api.rs` lists
them with their statuses), not on the message. A refused command answers 409, 503 means the game
isn't running (still loading, or closing), 504 that it didn't answer in time, and a wrong method
405 with an `Allow` header. Yes/no parameters (`stop`, `configure`, `reset`, `slow`) take `1` or
`true`.

**Frame rate:** nothing is drawn, so in game the client runs at 2 FPS, as a hidden tab does
(the login screen keeps full speed); the bot plays as fast. Options > Game's Set FPS doesn't
change that.

**What keeps the game thread busy:** `GET /debug/perf` lists the Flash calls the client made
since it started (by thread and callback, with the path for `getGameObject` and its kin: how
many, how long they waited, how long the player took to run them, reply bytes) and its player's
ticks. `TURU_PERF_LOG=<seconds>` logs the same for each tab every `seconds`, over that window
(`turu_perf` lines: the player thread's milliseconds per second, a `waits:` line with each
`bot.wait.forX` that ended in the window, how many timed out and their average time, then the
busiest calls, 12 unless `TURU_PERF_LOG_ROWS` says how many). Without a window it also logs a `memory:` line:
the process's resident and committed memory (with peaks), the Rust heap that's live, what the
allocator (mimalloc) has committed, in how many pages (how many abandoned by threads that ended)
and reserved, on Windows the system heaps' committed memory (the graphics driver's and the
system libraries' share; Mugetsu's `memory_split`, at the end of the line), the SWFs the game
loaded (their data in MB) and the Flash objects it holds, to tell a leak from the allocator
keeping freed memory. Off Windows mimalloc estimates the process's commit from its own, so the
two read the same there; its pages and reserved memory are what's new. With `TURU_PERF_LOG`,
Free Memory Now also logs the allocator's full statistics right after (`allocator statistics
after it:`). A `memory census:` line follows: the URLs with the most live
movies (images with their decoded size), so a file loaded again and again and never freed
stands out. A `memory biggest:` line lists the URLs whose live movies hold the most memory
(decompressed SWF data, plus decoded pixels for images), with their copies: what a map, a monster
or a piece of gear costs while it stays loaded, and which stays loaded when it shouldn't.
An `uncaught AS3 errors` line counts the game's uncaught AS3 errors and how many were logged (a
repeat is logged once, then only counted; `RUFFLE_ERRORS_EVERY_TIME=1` logs each).
Clips that stand still are skipped by Mugetsu 2.0.0's lean walks, which Turu turns on as
Mugetsu does (`lean-scripts`, `lean-frames`: `platform::DEFAULT_ON`); a `no-lean-scripts` or
`no-lean-frames` file next to the program turns one off, and `RUFFLE_LEAN_VERIFY=1` checks what
they skip.
`RUFFLE_JSON_TREE=1` makes `JSON.stringify` build its tree again for every call (the bot's reads
of the game go through it), and `RUFFLE_JSON_CHECK=1` builds it too and logs an error when the
direct text differs.
`TURU_RETAIN_URL=<part of a URL>` adds, every 5th reading, how many `LoaderInfo`s
of matching movies are alive and the chain of references that keeps the oldest, middle and
newest alive (a walk of the whole heap: a short pause). `tools/memrun/run.sh [script]
[minutes]` runs a long session this way, logging in from `turu-args.txt`, and
`tools/memrun/summarize.py` prints the trend per hour. To see what holds the heap, run with `TURU_SYSTEM_ALLOC=1`: the
system allocator instead of mimalloc (the memory line's commit then reads ~0), which heap
profilers see: on macOS `MallocStackLogging=1`, then `malloc_history <pid> -allBySize` (live
memory by call stack) or `-callTree -invert`; on Linux `heaptrack`. Use a debug build, which has
the symbols. With a
window it covers every tab, plus a total line and a `main thread` line: where the event loop's
time goes (idle, drawing and its parts, the games and bot calls, the rest).

**Measuring a farming run** (findings so far, open issues and next steps: `docs/HEADLESS_MONITORING_HANDOFF.md`): `tools/farmrun/run.sh [script] [minutes]` (default FarmerJoe DoAll,
120 minutes; `0` runs until the script stops) logs in from `turu-args.txt` as memrun does, with
`TURU_PERF_LOG` and a control API of its own (`API`, default `127.0.0.1:9031`). Every `PERIOD`
seconds (60) it adds a `farmrun:` line to the log (`tools/farmrun/sample.py`): `/status`'s level,
map, cell, kills, quests, deaths and relogins, and what the process uses:
- CPU time, user and system;
- resident memory, and macOS's footprint or Linux's PSS;
- threads, and open files and sockets;
- network bytes (`nettop` on macOS; on Linux the network namespace's);
- disk bytes (Linux only: macOS has no per-process disk I/O without root);
- the log's size.

It says at once when nothing
has moved for `STUCK_MIN` minutes (10: no change in those fields and no script log line; deaths
don't count as moving) or the client isn't logged in. `tools/farmrun/summarize.py <log>`, which
also works on a run still going or on any Turu log, reports:
- progress: kills, quests, deaths and relogins per hour, when each level came, time per map;
- the script's sections of 2 minutes or more, by the function that logged them;
- stretches without progress, skill-timer stalls and AFK messages;
- server warnings, refused or unanswered turn-ins, relogins and errors;
- with `TURU_PACKET_LOG=1`, the packets sent by command and the ones sent in the 2 s before each
  "Please slow down" (the server drops the action it calls too soon);
- resources: CPU (average, 95th percentile, user and system, per kill), memory and its growth per
  hour after the first 10 minutes (with the `memory:` lines' Rust heap, loaded SWFs and Flash
  objects, to tell a leak from warm-up; the biggest loaded files at the start and the end), threads, open files, network and disk rates, the log's
  growth, the game thread's time and the busiest bot calls.

It also writes the samples to `<log>.csv`.

**Recording a tape** (docs/REPLAY_TESTS_PROPOSAL.md): `TURU_TAPE=<file or folder>` (or
`--record-tape <file or folder>`) records what passes between the bot and the game: each call into
Flash with its answer, each call from `turu.swf` and each message of the game's socket, with
the time and the calling thread, plus a note when a script starts (its path under the scripts
folder and its options' values) and stops. It goes to a gzip-compressed JSON Lines file,
`*.tape.jsonl.gz` (`turu_bot::tape::read` reads one). A folder gets a new file per client
(`turu-<time>-<pid>.tape.jsonl.gz`); an existing file is never written over (the name gets `-2`,
`-3`, ...), so give tabs and Army clients a folder. Recording starts when the player first joins a
map; `TURU_TAPE_FROM_START=1` records from the start, for a relogin. Passwords (`loginInfo`
reads, the game's `login` call), session tokens (the socket's login, JSON fields named like
`sToken`), the account's email and IP address (`initUserData`) and chat (sent, received,
whispers) are replaced with `<redacted>` as each line is written; player names are kept, so a
tape is private until it is scrubbed: `cargo run --release -p turu_bot --bin turu-tape -- scrub
<tape> <new file>` renames every player (the account "Questwalker"), renumbers the account's
IDs and checks the result; `turu-tape info <tape>` says what a tape holds. A scrubbed tape
plays back to its script in a test, with no game (`turu_bot::testing::ReplayHost`,
`bot/tests/replay.rs`). A writer thread of its
own compresses and writes, flushing every second: a killed process leaves a tape readable up to
its last second, without its `end` line. Lines the writer couldn't keep up with are dropped,
never waited for, and counted in the `end` line (and as gaps in their numbers). Quitting through
Turu finishes the file: `--exit-on-script-stop` (or `POST /script/stop` with it), an update,
closing the window. A signal (`systemctl stop`, Ctrl+C) doesn't: the tape then ends at its last
flush. Use a quiet server and a spare account: a tape holds the account's name and everything it
saw. FarmerJoe DoAll writes about 9 MB of tape an hour (70 lines a second); the writer's work
is negligible in a release build. The redaction was checked on one 20-minute farm run (the
password and token absent); before sharing a tape made with another account or other content,
search the decompressed tape for that account's password (`gunzip -c <tape> | grep -cF
'<password>'` says 0).

**A speed profile (PGO)** of Turu's own: `tools/pgo/pgo.sh all [script...]` builds a `dist` binary
that counts what runs (`gen`), logs in from `turu-args.txt` as farmrun does and plays each script
for `MINUTES` (20) with a window and headless (`record`, `MODES`), merges the counts (`merge`,
into `tools/pgo/out/turu-<target>.profdata`; needs `rustup component add llvm-tools`) and builds
`target/<target>/dist/turu` with them (`use`). Each run ends with `POST /script/stop` and
`--exit-on-script-stop`: only a clean exit writes the counts, so a killed or crashed run's are
lost. Only Rust is profiled: the C code (mimalloc, QuickJS) goes through `tools/pgo/cc`, which drops
the profile flags cc-rs passes on (the system's clang writes another profile format, and the
instrumented mimalloc crashed on it). A headless run draws nothing, so its counts say nothing about drawing; the window's do. It
needs a quiet server (`SERVER`, and refuses Twilly, Artix and Alteon) and a spare account, and a
new recording after large changes (a profile covers only the code it saw). Mugetsu's own profile
(`pgo/aqw.profdata`) applied to Turu changed the render thread little (STATUS.md); compare a
`use` build with a plain `dist` one on the perf log's lines before shipping one.

## Reference

| Flag | What |
|---|---|
| `--headless` | No window. |
| `--user <name>`, `--password <password>` | Log in once the game has loaded (password better in `TURU_PASSWORD`). |
| `--server <name>` | The server to log in on (default Twilly), and the one relogins go back to for this run (Relogin Server; not saved). |
| `--run-script <path>` / `--load-script <path>` | Start the script after the login, once the player has loaded (up to 20 s) / only load it. |
| `--run-playlist <file>` / `--load-playlist <file>` | The same for a playlist (wins over a script). |
| `--client=<address>` | Run one tab without a window and draw it for a shell at `<address>` (needs `TURU_SHELL_TOKEN`; docs/TAB_PROCESSES_PROPOSAL.md, M2). Quits when the shell closes. |
| `--shell` | A window that runs each tab in a `--client` process and shows the one selected (or all, in Grid View); the first tab gets the other flags but `--api`, `--monitor` and `--report-to`. The shell runs the web monitor and Report To over every tab; each client gets a control API of its own on a free local port, and the shell's `--api` serves the same routes over the tabs, passing each request on to the tab's (`/tabs` lists them; a tab whose client is starting again answers `game_not_running`). A dead client is started again (docs/TAB_PROCESSES_PROPOSAL.md, M5). `TURU_SHELL_TEST=<file>` scripts it for tests. Every start of Turu's window is the shell (M6), Army > Accounts' "In its own window" ones included; not `--headless`. |
| `--in-process` | Ignored since M6 (each tab is a process of its own); it ran the window's tabs in its own process. |
| `--exit-on-script-stop` | Log out and quit when the script, or the playlist, ends (not when auto-relogin stops it). |
| `--api[=<address>]` | The control API, default `127.0.0.1:9091`. |
| `--monitor[=<address>]` | The web monitor; without an address, Options > Application's port. |
| `--report-to <url>` | Report to another machine's web monitor. |
| `--record-tape <file or folder>` | Record a tape (as `TURU_TAPE`; see "Recording a tape"). With `--shell`, its first tab's client records. |

| Variable | What |
|---|---|
| `TURU_PASSWORD` | The password for `--user`. |
| `TURU_SHELL_TOKEN` | Set by `--shell` for its clients: the token a `--client` says hello with. |
| `TURU_SHELL_TEST` | With `--shell`: a file of test steps (`wait`, `open`, `select`, `close`, `kill`, `click`, `dump`; `shell/src/lib.rs` lists them all). |
| `TURU_DATA_DIR` | The data folder. |
| `TURU_API_TOKEN` | The control API token (32+ characters); else the data folder's `api-token`. |
| `TURU_MONITOR_PUSH_KEY` | The key `--report-to` uses. |
| `TURU_SCRIPTS_REPO` | Another scripts repository (`owner/name`). |
| `TURU_RELEASES_URL` | Where to check for new Turu releases (GitHub's releases API format; default Turu's). |
| `TURU_PERF_LOG` | Every this many seconds, log each tab's Flash calls and player ticks (and memory, without a window). |
| `TURU_SYSTEM_ALLOC` | Set: the system allocator instead of mimalloc, for heap profilers. |
| `TURU_MI_COLLECT` | `0`: don't make mimalloc hand its free memory back to the system once a minute (to compare). |
| `TURU_RETAIN_URL` | Without a window, with `TURU_PERF_LOG`: what keeps the `LoaderInfo`s of movies whose URL contains this alive, every 5th memory reading. |
| `TURU_CACHE_CENSUS` | With a window and `TURU_PERF_LOG`: every window, a `cache census:` line: the live cacheAsBitmap textures, how many of them objects on the stage and orphans hold, and the classes holding the most. |
| `TURU_PACKET_LOG` | Set: every packet the client sends goes to the log too (`turu::packet` lines), as Diagnostics > Logger does with a window. `tools/farmrun/summarize.py` then shows which packets came before each "Please slow down". |
| `TURU_TAPE` | A file or folder: every client records a tape there ("Recording a tape"). |
| `TURU_TAPE_FROM_START` | `1`: tapes record from the start, the login included (redacted), not from the first map. |
| `TURU_HTTP_CACHE` | `0`: don't keep the game's files on disk (`<data>/cache/gamefiles`); every one is downloaded again. |
| `TURU_HTTP_CACHE_MB` | How large that folder may grow (default 1024); the files used longest ago go first. |
| `TURU_PERF_LOG_ROWS` | How many calls each of those lists (default 12). |
| `TURU_LEVER_AB` | `<lever>:<seconds>`: turns one of turu.swf's measuring levers (`stopAnimations`, `eventsOff`) on and off in turns while in game, logging each turn and where the playing clips are (HOST_SWF_PROPOSAL.md M7a). |
| `TURU_TEST_PANIC` | Set: panic 10 s after the start, to test crash reports (`sendCrashReports: true` in `settings.json` sends them; there is no question without a window). |
| `TURU_GRID_TEST` | With a window: hidden tabs run at this frame rate and draw every frame (to measure a grid of tiles). |
| `RUST_LOG` | What the log shows. |

## Troubleshooting

| Symptom | Cause |
|---|---|
| `Script ...: isn't in the scripts folder` | Not in Search Scripts' index and not on disk, or no network. |
| `Loaded 0 script(s) from the playlist` | None of its paths were found in the scripts folder. |
| `--user without --password` | `TURU_PASSWORD` isn't set where Turu runs. |
| Butler doesn't list the other accounts | They don't share the data folder, aren't logged in, or Butler is off. |
| Army clients wait in different rooms | They don't share the data folder. |
| `Web monitor: ...` address in use | Enable Web Monitor is on in the shared `settings.json`. |
| `Control API: ...` | The port is taken, or the token is under 32 characters. |
| `Couldn't install Turu ...: couldn't replace ...` | The service's user can't write the program's folder. |
| Clients overwrite each other's Ruffle log | It's one file per user (`~/.cache/ruffle/log`): use journald. |
