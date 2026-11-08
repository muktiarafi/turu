# Running Turu without a window

Run Turu on a Linux server, or any machine you don't sit at: no display, GPU or Xvfb needed.
It logs in, runs a script or a playlist, and you watch and steer it over HTTP. The same flags
work on macOS and Windows.

> **Status:** windowless mode loads AQW to the login screen and loads scripts and playlists;
> a full login, farm and relogin without a window haven't been run in game yet
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
curl -H "$T" localhost:9021/script/options        # the loaded script's options and values
curl -H "$T" -X POST -d '{"UseBoost": true, "Group": {"Count": 5}}' localhost:9021/script/options
curl -H "$T" -X POST "localhost:9021/script/options?reset=1"     # back to the defaults
curl -H "$T" "localhost:9021/scheduler/options?id=<entry id>"    # a playlist entry's (GET or POST)
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
TURU_API=127.0.0.1:9021
# The control API token: 32 characters at least, e.g. from `openssl rand -hex 32`
TURU_API_TOKEN=a long random string
# Extra flags, e.g. --monitor=127.0.0.1:8790 (systemd reads no comments after a value)
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

## Several accounts

Run one instance per account, all with **one data folder** (as above) and each with **its own
`TURU_API` port** (`9021`, `9022`, ...). The shared folder is how clients find each other:
Butler's sync files, army scripts' sync logs and their private room number
(`bot.army.roomNumber()`, a hash of the folder's path) all live there. Start the instances a
few seconds apart, so they don't update the same scripts at once.

Have one serve the web monitor and the others report to it:

```bash
# The first account's env file:
TURU_ARGS=--monitor=127.0.0.1:8790
# The others' (the key file appears after the first one's first run):
TURU_ARGS=--report-to http://127.0.0.1:8790
TURU_MONITOR_PUSH_KEY=<contents of /var/lib/turu/monitor-push-key>
```

Leave Enable Web Monitor off in the shared `settings.json`, or every client tries to serve it.

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

**Web monitor** (`--monitor=127.0.0.1:8790`): each bot's state, last-hour trace, logs, history and commands.
Keep it on loopback and use a tunnel, which needs no key:

```bash
ssh -L 8790:127.0.0.1:8790 you@server     # then open http://localhost:8790
```

From anywhere else (Tailscale Serve, a proxy, `--monitor=0.0.0.0:8790`) open
`http://<address>:8790/#<key>` with the key from the data folder's `monitor-key`. Anyone with
the key can run commands on your bots.

**Report to your desktop instead:** on the desktop, Options > Application > Monitor: Enable
Web Monitor, Allow Other Devices, Copy Report Settings. On the server, `--report-to
http://<desktop>:8790` with the push key in `TURU_MONITOR_PUSH_KEY`. The server's bots show on
the desktop's page and take its commands.

**Control API** (`--api`): every request needs the token (`TURU_API_TOKEN`, else the data
folder's `api-token`). All routes, and what each status means, are listed in
`bot/src/control_api.rs`.

```bash
T="Authorization: Bearer $TURU_API_TOKEN"
curl -H "$T" localhost:9021/status
curl -H "$T" "localhost:9021/log?since=0"
curl -H "$T" -X POST "localhost:9021/script/start?path=farm/GoldFarm"
curl -H "$T" -X POST localhost:9021/script/stop      # with a playlist: moves on to the next
curl -H "$T" localhost:9021/scheduler
curl -H "$T" -X POST --data-binary @daily.json localhost:9021/scheduler/load
curl -H "$T" -X POST localhost:9021/scheduler/start  # and /scheduler/stop
curl -H "$T" localhost:9021/butler                   # online accounts, who can be summoned
curl -H "$T" -X POST "localhost:9021/butler/summon?user=MyAlt"   # also release, summon-all,
                                                                 # release-all, follow, unfollow
curl -N -H "$T" "localhost:9021/events?logs=script,debug"       # follow it, instead of polling
curl -H "$T" localhost:9021/tabs                     # with a window: the tabs
curl -H "$T" localhost:9021/tabs/2/status            # any route, for one tab
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

```bash
curl -H "$T" localhost:9021/status | jq .update
curl -H "$T" -X POST localhost:9021/update            # ?stop=1 while a script runs
```

A command answers with what it changed: the script for `/script/*`, `/status` for the logins,
where the player is for the moves, the playlist for `/scheduler/*`. A failure is
`{"error": "...", "code": "..."}`: script on `code` (`not_logged_in`, `script_running`,
`login_under_way`, `game_not_running`, `game_timeout`, ...; `bot/src/control_api.rs` lists
them with their statuses), not on the message. A refused command answers 409, 503 means the game
isn't running (still loading, or closing), 504 that it didn't answer in time, and a wrong method
405 with an `Allow` header. Yes/no parameters (`stop`, `configure`, `reset`, `slow`) take `1` or
`true`.

**What keeps the game thread busy:** `GET /debug/perf` lists the Flash calls the client made
since it started (by thread and callback, with the path for `getGameObject` and its kin: how
many, how long they waited, how long the player took to run them, reply bytes) and its player's
ticks. `TURU_PERF_LOG=<seconds>` logs the same for each tab every `seconds`, over that window
(`turu_perf` lines: the player thread's milliseconds per second, then the busiest calls, 12
unless `TURU_PERF_LOG_ROWS` says how many). Without a window it also logs a `memory:` line:
the process's resident and committed memory (with peaks), the Rust heap that's live, the SWFs
the game loaded (their data in MB) and the Flash objects it holds, to tell a leak from the
allocator keeping freed memory. A `memory census:` line follows: the URLs with the most live
movies (images with their decoded size), so a file loaded again and again and never freed
stands out. `TURU_RETAIN_URL=<part of a URL>` adds, every 5th reading, how many `LoaderInfo`s
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

## Reference

| Flag | What |
|---|---|
| `--headless` | No window. |
| `--user <name>`, `--password <password>` | Log in once the game has loaded (password better in `TURU_PASSWORD`). |
| `--server <name>` | The server to log in on (default Twilly). |
| `--run-script <path>` / `--load-script <path>` | Start the script after the login / only load it. |
| `--run-playlist <file>` / `--load-playlist <file>` | The same for a playlist (wins over a script). |
| `--exit-on-script-stop` | Log out and quit when the script, or the playlist, ends (not when auto-relogin stops it). |
| `--api[=<address>]` | The control API, default `127.0.0.1:9021`. |
| `--monitor[=<address>]` | The web monitor; without an address, Options > Application's port. |
| `--report-to <url>` | Report to another machine's web monitor. |

| Variable | What |
|---|---|
| `TURU_PASSWORD` | The password for `--user`. |
| `TURU_DATA_DIR` | The data folder. |
| `TURU_API_TOKEN` | The control API token (32+ characters); else the data folder's `api-token`. |
| `TURU_MONITOR_PUSH_KEY` | The key `--report-to` uses. |
| `TURU_SCRIPTS_REPO` | Another scripts repository (`owner/name`). |
| `TURU_RELEASES_URL` | Where to check for new Turu releases (GitHub's releases API format; default Turu's). |
| `TURU_PERF_LOG` | Every this many seconds, log each tab's Flash calls and player ticks (and memory, without a window). |
| `TURU_SYSTEM_ALLOC` | Set: the system allocator instead of mimalloc, for heap profilers. |
| `TURU_MI_COLLECT` | `0`: don't make mimalloc hand its free memory back to the system once a minute (to compare). |
| `TURU_RETAIN_URL` | Without a window, with `TURU_PERF_LOG`: what keeps the `LoaderInfo`s of movies whose URL contains this alive, every 5th memory reading. |
| `TURU_PERF_LOG_ROWS` | How many calls each of those lists (default 12). |
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
