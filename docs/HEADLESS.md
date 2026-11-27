# Running Turu without a window

Run Turu on a Linux server, or any machine you don't sit at: no display, GPU or Xvfb needed.
It logs in, runs a script or a playlist, and you watch and steer it over HTTP. The same flags
work on macOS and Windows.

> **Status:** windowless runs have logged in, farmed for hours and relogged after a drop
> (`docs/STATUS.md`). Watch your first runs.

Measuring Turu itself (`TURU_PERF_LOG`, memory, farmrun, tapes, PGO) is `docs/DIAGNOSTICS.md`.

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
/opt/turu/turu --headless --user YourName --server Twig --run-script farm/GoldFarm
```

It logs in once the game has loaded, then starts the script; the log goes to the terminal.
Ctrl+C quits. `--server` is also where relogins go for this run: pick a quiet server, so a
relogin finds room (without it, Twilly).

Everything Turu keeps is in its data folder (or `TURU_DATA_DIR`): `~/.local/share/Turu` on
Linux, `~/Library/Application Support/Turu` on macOS, `%APPDATA%\Turu` on Windows.

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
/opt/turu/turu --headless --user YourName --server Twig --run-playlist ~/playlists/daily.json
```

Its scripts are downloaded like `--run-script`'s; entries whose script isn't found are left
out (the log says how many loaded). The file is VibeSkua's playlist JSON
(`[{"Path", "Id", "Name"}]`, `Id` any GUID), so you can also write one by hand.

## Stopping and restarting

What holds for every way of running it (systemd and Docker below):

* A restart starts the script, or the playlist, again from the top.
* `--exit-on-script-stop` logs out and exits with status 0 when the script or playlist ends (not
  when auto-relogin stops it). `Restart=on-failure` (Docker's `restart: on-failure`) then leaves
  it stopped; use `Restart=always` (`restart: always`) to loop.
* An update (`POST /update`, under "Updates") exits with status 75, so "on failure" starts the
  new binary.
* Stopping it with a signal (`systemctl stop`, `docker compose stop`, Ctrl+C) doesn't log out;
  the game server drops the connection shortly after.

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
# A quiet server, so relogins find room
TURU_SERVER=Twig
# The control API's address: the unit passes it, so it can't be empty
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
* `restart: on-failure`, as the service above ("Stopping and restarting"). `POST /update`
  replaces the binary inside the container, which lasts until the container is recreated;
  rebuild the image to keep a new version.

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

**Frame rate:** nothing is drawn, so in game the client runs at 2 FPS, as a hidden tab does
(the login screen keeps full speed); the bot plays as fast. Options > Game's Set FPS doesn't
change that.

### Control API

`--api` serves it; every request needs the token (`TURU_API_TOKEN`, else the data folder's
`api-token`).

```bash
T="Authorization: Bearer $TURU_API_TOKEN"
curl -H "$T" localhost:9091/status
curl -H "$T" "localhost:9091/log?since=0"
curl -H "$T" -X POST "localhost:9091/script/start?path=farm/GoldFarm"
curl -H "$T" -X POST localhost:9091/script/stop      # with a playlist: moves on to the next
curl -H "$T" -X POST --data-binary @daily.json localhost:9091/scheduler/load
curl -H "$T" -X POST localhost:9091/scheduler/start
curl -H "$T" -X POST "localhost:9091/butler/summon?user=MyAlt"
curl -N -H "$T" "localhost:9091/events?logs=script,debug"       # follow it, instead of polling
```

| Route | Parameters | What |
|---|---|---|
| `GET /status` | | Connected, server, map and cell, the player (level, health), the run's kills, quests, deaths and relogins, the script, `update`. Asks the game each time. |
| `GET /log` | `since`, `type` (`script`, `debug`, `packet`) | Log lines after `since`, and the next `since`. |
| `GET /events` | `logs` (as `type`, comma-separated) | Server-sent events (below). |
| `GET /snapshot` | `since`, `slow` | What the web monitor polls: its sample, events, the process. |
| `POST /update` | `stop` | Install the new release (below). |
| `POST /script/load`, `/script/start` | `path`; `configure` for start | Load / start a script. |
| `POST /script/stop`, `/script/pause`, `/script/resume` | | |
| `GET`/`POST /script/options` | body: values; `reset` | The loaded script's options. |
| `GET`/`POST /game/options` | body: values | Options > Game. |
| `POST /login`, `/relogin`, `/logout` | `server` | Log in if not logged in / log in again / log out. |
| `POST /map/join` | `map`, `cell` (Enter), `pad` (Spawn) | Join a map, or jump in the one you're in. |
| `POST /player/goto` | `name` | Go to a player. |
| `POST /map/house` | | Join your house. |
| `POST /quest/register` | `id`, `reward` | Register a quest to accept and turn in. |
| `GET /gear`, `POST /bank/load` | `stop` for the bank | Inventory and bank. |
| `POST /equip` | `item` (name or ID), `stop` | Equip an item. |
| `POST /auto/attack`, `/auto/hunt`, `/auto/stop` | `class`, `mode` (Base), `maps` | Auto Attack / Hunt. |
| `GET /scheduler` | | The playlist. |
| `POST /scheduler/add`, `/remove`, `/move`, `/rename`, `/clear` | `path`; `id`; `id`, `by` (-1, 1); `id`, `name` | Edit it. |
| `POST /scheduler/load`, `/scheduler/save` | `path`, or the playlist as the body for load | Load / save a playlist file. |
| `GET`/`POST /scheduler/options` | `id`, body, `reset` | A playlist entry's options. |
| `POST /scheduler/start`, `/scheduler/stop` | | |
| `GET /butler` | | Online accounts, who can be summoned. |
| `POST /butler/summon`, `/release`, `/summon-all`, `/release-all` | `user` | As a master. |
| `POST /butler/follow`, `/unfollow` | `master` | As a follower. |
| `GET /tabs`, any route under `/tabs/<n>/` | | With a window: the tabs, and a route for one tab. |

`bot/src/control_api.rs` has the full list of failure codes with their statuses.

**Replies:** a command answers with what it changed: the script for `/script/*`, `/status` for
the logins, where the player is for the moves, the playlist for `/scheduler/*`. A failure is
`{"error": "...", "code": "..."}`: script on `code` (`not_logged_in`, `script_running`,
`login_under_way`, `game_not_running`, `game_timeout`, ...), not on the message. A refused
command answers 409, 503 means the game isn't running (still loading, or closing), 504 that it
didn't answer in time, and a wrong method 405 with an `Allow` header. Yes/no parameters (`stop`,
`configure`, `reset`, `slow`) take `1` or `true`. Playlist paths in `/scheduler/load?path=` and
`/scheduler/save?path=` are relative to the data folder's `playlists` folder, or absolute inside
the data folder.

**`/events`** sends server-sent events: `script` (path, status, running) when it changes, `log`
for each new line of the logs you ask for (`script` by default), `event` for each web monitor
event, and a comment every 15 s. It reads the bot's state, never the game, so it costs nothing
while idle.

### Updates

It checks Turu's releases at launch and every 6 hours (Options > Application > Check for Client
Updates in the shared `settings.json`) and only tells you: a log line
(`Update available: Turu X (you have Y): <release page>`), `update` in `/status` (version,
current, url, download, prerelease; `null` when up to date) and in the web monitor's summary.

```bash
curl -H "$T" localhost:9091/status | jq .update
curl -H "$T" -X POST localhost:9091/update            # ?stop=1 while a script runs
```

`POST /update` (409 while a script runs, unless `?stop=1`) downloads the package, checks it
against the release's `SHA256SUMS.txt`, replaces the program and exits with status 75, which
`Restart=on-failure` restarts on the new binary. The service's user must be able to write the
program's folder (`/opt/turu` above is root's: `chown` it to that user, or update by hand).
`SHA256SUMS.txt` proves the download is intact, not who published it. A nightly build (the
`nightly` release, `0.4.0-nightly.<date>`) never checks: `update` stays `null` and
`POST /update` refuses (`not_supported`). Replace it by hand.

## Reference

| Flag | What |
|---|---|
| `--headless` | No window. |
| `--user <name>`, `--password <password>` | Log in once the game has loaded (password better in `TURU_PASSWORD`). |
| `--server <name>` | The server to log in on (default Twilly), and the one relogins go back to for this run (Relogin Server; not saved). |
| `--run-script <path>` / `--load-script <path>` | Start the script after the login, once the player has loaded (up to 20 s) / only load it. |
| `--run-playlist <file>` / `--load-playlist <file>` | The same for a playlist (wins over a script). |
| `--exit-on-script-stop` | Log out and quit when the script, or the playlist, ends (not when auto-relogin stops it). |
| `--api[=<address>]` | The control API, default `127.0.0.1:9091`. |
| `--monitor[=<address>]` | The web monitor; without an address, Options > Application's port. |
| `--report-to <url>` | Report to another machine's web monitor. |
| `--record-tape <file or folder>` | Record a tape (`docs/DIAGNOSTICS.md`). |

The window's own flags (`--shell`, `--client`) are in `docs/ARCHITECTURE.md`, "Processes".

| Variable | What |
|---|---|
| `TURU_PASSWORD` | The password for `--user`. |
| `TURU_DATA_DIR` | The data folder. |
| `TURU_API_TOKEN` | The control API token (32+ characters); else the data folder's `api-token`. |
| `TURU_MONITOR_PUSH_KEY` | The key `--report-to` uses. |
| `TURU_SCRIPTS_REPO` | Another scripts repository (`owner/name`). |
| `TURU_RELEASES_URL` | Where to check for new Turu releases (GitHub's releases API format; default Turu's). |
| `TURU_HTTP_CACHE` | `0`: don't keep the game's files on disk (`<data>/cache/gamefiles`); every one is downloaded again. |
| `TURU_HTTP_CACHE_MB` | How large that folder may grow (default 1024); the files used longest ago go first. |
| `RUST_LOG` | What the log shows. |

Variables for measuring Turu (`TURU_PERF_LOG`, `TURU_PACKET_LOG`, `TURU_TAPE`, ...) are in
`docs/DIAGNOSTICS.md`.

## Troubleshooting

| Symptom | Cause |
|---|---|
| `Script ...: isn't in the scripts folder` | Not in Search Scripts' index and not on disk, or no network. |
| `Loaded 0 script(s) from the playlist` | None of its paths were found in the scripts folder. |
| `--user without --password` | `TURU_PASSWORD` isn't set where Turu runs. |
| `--api=: ... expected an address` | `TURU_API` is empty in the systemd env file. |
| Butler doesn't list the other accounts | They don't share the data folder, aren't logged in, or Butler is off. |
| Army clients wait in different rooms | They don't share the data folder. |
| `Web monitor: ...` address in use | Enable Web Monitor is on in the shared `settings.json`. |
| `Control API: ...` | The port is taken, or the token is under 32 characters. |
| `Couldn't install Turu ...: couldn't replace ...` | The service's user can't write the program's folder. |
| Clients overwrite each other's Ruffle log | It's one file per user (`~/.cache/ruffle/log`): use journald. |
