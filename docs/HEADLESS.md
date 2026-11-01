# Running Turu without a window

How to run Turu on a Linux server (or any machine you don't sit at): no window, no GPU, no
display, steered and watched over HTTP. The same flags work on macOS and Windows.

> **Status:** windowless mode loads AQW to the login screen and downloads and loads scripts
> and playlists; a full login, farm and relogin without a window still need a run in game
> (`docs/STATUS.md`). Try it with an account you can afford to watch for a while.

## What you get

`turu --headless` runs the game with Ruffle's null renderer: nothing is drawn, no sound, no
window, so it needs no X server, Wayland or Xvfb. With the login flags it logs in once the game
has loaded and starts a script or a Scheduler playlist, logging to the terminal. You watch and
steer it with:

* the **log** on stdout (journald under systemd);
* the **web monitor** (`--monitor`): the page with the bot's card, logs, history and commands;
* **reporting** to the monitor on another machine (`--report-to`), e.g. your desktop's;
* the **control API** (`--api`): HTTP routes for status, logs, scripts and the playlist;
* Discord **webhooks** (Options > Application), if you set them up.

At the login screen it takes about a quarter of the CPU and memory of a client with a window.

## Install

Download `turu-<version>-linux-x86_64.tar.gz` from
[Releases](https://github.com/muktiarafi/turu/releases) and unpack it anywhere, e.g.
`/opt/turu`. The program is the single file `turu`. Or build it (README, "Build"; the `dist`
profile is much faster in game).

The binary needs a few shared libraries even without a window. On Debian or Ubuntu:

```bash
sudo apt install libasound2 libudev1 libssl3 libfontconfig1 libfreetype6
```

(On Ubuntu 24.04 the first is `libasound2t64`.) To see if anything is missing:

```bash
ldd /opt/turu/turu | grep "not found"
```

No graphics drivers are needed for `--headless`.

## First run

```bash
export TURU_PASSWORD='your password'
/opt/turu/turu --headless --user YourName --server Twilly --run-script farm/GoldFarm
```

* `--user` and the password log in once the game has loaded; `--server` picks the server
  (default Twilly). Prefer `TURU_PASSWORD` to `--password`: other users on the machine can read
  a command line.
* `--run-script` starts the script after the login. `farm/GoldFarm` is a path in
  [turu-scripts](https://github.com/muktiarafi/turu-scripts): if it isn't in the scripts folder
  yet, or it's outdated, it's downloaded with the files it imports before the game starts.
* Ctrl+C quits.

The log shows the download, the login (`Logging in on Twilly.`) and then the script's own log.
`RUST_LOG=warn,turu=debug` shows more.

Everything Turu keeps (scripts, options, tokens, keys) is in its data folder,
`~/.local/share/Turu` on Linux. `TURU_DATA_DIR=/some/dir` uses another one: give each account
its own (see "Several accounts").

## Scripts

A script given with `--run-script`, `--load-script` or in a playlist is found like this:

1. an absolute path is used as it is;
2. a relative one is looked up in the data folder's `scripts` folder, then in the working
   folder; `.ts` is optional;
3. a relative path that Search Scripts lists is downloaded (or updated) first, with what it
   imports. Scripts you edited in the scripts folder are kept as they are.

Without network access, copy the scripts yourself: `git clone
https://github.com/muktiarafi/turu-scripts` and either point `TURU_DATA_DIR`'s `scripts` folder
at it or pass absolute paths. Imports are relative, so a clone works as it is.

### Script options

Without a window there is no Script Options window: a script runs with the options saved for
it, or its defaults. Set them on a desktop (Scripts > Script Options), then copy the files from
the desktop's data folder to the server's:

| File | What |
|---|---|
| `options/<storage>.cfg` | A script's options (`--run-script`). |
| `options/Queue_<script>_<id>.cfg` | A playlist entry's options (below). |
| `options/CBO_Storage(<username>).txt` | Options > CoreBots for that character. |
| `options.json` | Options > Game (the bot options). |
| `UserAdvancedSkills.json` | Your skill sets. |
| `settings.json` | Options > Application (webhooks, relogin delay, monitor settings). |

CoreBots scripts turn Auto Relogin on themselves; for others set `autoRelogin` in
`options.json`, or send it with the control API's `POST /options` once the script runs.

## Playlists (Scheduler)

A playlist runs several scripts one after another, as Scripts > Scheduler does: any end of a
script moves on to the next, a disconnection waits for the login and resumes the entry.

1. On a desktop, add the scripts in Scripts > Scheduler, set each entry's options (its ⋯ menu
   > Script options), and **Save** the playlist. Scripts in the scripts folder are saved
   relative to it, so the file works on another machine.
2. Copy the playlist file and the entries' `options/Queue_*.cfg` files to the server.
3. Run it:

   ```bash
   /opt/turu/turu --headless --user YourName --run-playlist ~/playlists/daily.json
   ```

   The playlist's scripts are downloaded if needed, like `--run-script`'s. Entries whose script
   can't be found are left out (the log says how many were loaded).

`--load-playlist` only loads it; start it later with `POST /scheduler/start` (below). The file
is VibeSkua's playlist JSON, which you can also write by hand:

```json
[
  { "Path": "farm/GoldFarm", "Id": "3f2504e0-4f89-11d3-9a0c-0305e82c3301", "Name": "Gold" },
  { "Path": "farm/Leveling.ts", "Id": "3f2504e0-4f89-11d3-9a0c-0305e82c3302", "Name": "" }
]
```

`Id` names the entry's options file; any GUID will do.

## Flags

| Flag | What |
|---|---|
| `--headless` | No window. |
| `--user <name>` | Log in as `name` once the game has loaded (needs the password). |
| `--password <password>` | The password; better in `TURU_PASSWORD`. |
| `--server <name>` | The server to log in on (default Twilly). |
| `--run-script <path>` / `--load-script <path>` | Start the script after the login / only load it. |
| `--run-playlist <file>` / `--load-playlist <file>` | The same for a playlist (it wins over a script). |
| `--exit-on-script-stop` | Log out and quit when the script stops, or with a playlist, when the playlist ends. Not when auto-relogin stops it. |
| `--api[=<address>]` | The control API, default `127.0.0.1:8791`. |
| `--monitor[=<address>]` | The web monitor, e.g. `127.0.0.1:8790`; without an address, Options > Application's port. |
| `--report-to <url>` | Report to another machine's web monitor (with `TURU_MONITOR_PUSH_KEY`). |

| Variable | What |
|---|---|
| `TURU_PASSWORD` | The password for `--user`. |
| `TURU_DATA_DIR` | The data folder. |
| `TURU_API_TOKEN` | The control API's token (at least 16 characters); else the data folder's `api-token`, made on first use. |
| `TURU_MONITOR_PUSH_KEY` | The key `--report-to` reports with. |
| `TURU_SCRIPTS_REPO` | Another scripts repository (`owner/name`). |
| `RUST_LOG` | What the log shows (default: Turu's messages, warnings and errors). |

## Run it as a service

A systemd template unit runs one client per account, restarts it after a crash and sends its
log to journald. Make a user for it (`sudo useradd -r -m -d /var/lib/turu turu`), then
`/etc/systemd/system/turu@.service`:

```ini
[Unit]
Description=Turu (%i)
Wants=network-online.target
After=network-online.target

[Service]
User=turu
EnvironmentFile=/etc/turu/%i.env
# Makes /var/lib/turu/<account>, owned by the turu user: the account's data folder.
StateDirectory=turu/%i
Environment=TURU_DATA_DIR=/var/lib/turu/%i
ExecStart=/opt/turu/turu --headless --user ${TURU_USER} --server ${TURU_SERVER} \
    --run-playlist /var/lib/turu/%i/playlist.json --api=${TURU_API}
Restart=on-failure
RestartSec=30

[Install]
WantedBy=multi-user.target
```

and for each account an environment file, readable by root only
(`sudo install -m 600 /dev/null /etc/turu/main.env`):

```bash
TURU_USER=YourName
TURU_PASSWORD=your password
TURU_SERVER=Twilly
TURU_API=127.0.0.1:8791
TURU_API_TOKEN=a long random string, at least 16 characters
```

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now turu@main
journalctl -u turu@main -f
```

Notes:

* A restart starts the playlist (or script) again from the top: the playlist isn't saved
  while it runs.
* With `--exit-on-script-stop` the client exits with status 0 when it's done, so
  `Restart=on-failure` leaves it stopped; use `Restart=always` to farm in a loop.
* `systemctl stop` ends the process without logging out; the game server drops the
  connection after a short while.

## Several accounts

Run one client per account, each with its **own data folder** (`TURU_DATA_DIR`) and its own
ports: two clients can't share an API or monitor port. With the unit above, give each account
its own env file and `TURU_API` port (`8791`, `8792`, ...) and start `turu@<name>`.

Serve the web monitor from one of them (`--monitor`) and have the others report to it, so one
page shows them all:

```bash
# the first account: the monitor
--monitor=127.0.0.1:8790
# the others: report to it, with the first one's push key
TURU_MONITOR_PUSH_KEY=$(sudo cat /var/lib/turu/main/monitor-push-key)
--report-to http://127.0.0.1:8790
```

(Army > Accounts, which launches and steers several clients, needs a window: on a server, use
one client per account as above.)

Ruffle's own log file (in `~/.cache/ruffle/log`) is one per user, so clients started as the
same user overwrite each other's; use journald for each client's log.

## Watching and steering it

### The log

Everything the Script Loader's log shows, plus logins, relogins and errors, goes to stdout:

```bash
journalctl -u turu@main -f
```

### The web monitor

`--monitor=127.0.0.1:8790` serves the monitor page on the server's loopback: the bot's card
(state, map, level, gold, HP, script and run time), gold and XP per hour, its events, its
script and debug log, an hour-by-hour history, and commands (Start / Stop / Pause the script,
Log in, Relogin, Log out, House, Join map). The page itself is described in `docs/STATUS.md`
("Web monitor").

Keep it on loopback and reach it through a tunnel:

```bash
ssh -L 8790:127.0.0.1:8790 you@server
# then open http://localhost:8790 on your computer
```

A browser coming through the tunnel counts as this computer, so it needs no key. Anything else
needs the monitor key, in the data folder's `monitor-key`: open
`http://<address>:8790/#<key>`. That's the case behind Tailscale Serve or another proxy, or
with `--monitor=0.0.0.0:8790`, which serves every interface. Only do the latter on a network
you trust, and keep the key secret: with it, anyone can run commands on your bot.

### Reporting to your desktop

If you run Turu with a window at home, the server's bots can show on its monitor instead:

1. On the desktop: Options > Application > Monitor: Enable Web Monitor and Allow Other
   Devices, then **Copy Report Settings** (the monitor's address and push key).
2. On the server: `--report-to http://<desktop address>:8790` with the push key in
   `TURU_MONITOR_PUSH_KEY`.

The server reports every few seconds; its bots show under its machine name and take the same
commands, which ride back in the reply. The server needs to reach the desktop, e.g. over
Tailscale; the push key only lets it report, nothing else.

### The control API

`--api` serves HTTP routes for scripts, status and logs. Every request needs the token
(`TURU_API_TOKEN`, or the data folder's `api-token`) as `Authorization: Bearer <token>` or
`X-Api-Token: <token>`. `bot/src/control_api.rs` lists every route.

```bash
T="Authorization: Bearer $TURU_API_TOKEN"
curl -H "$T" localhost:8791/status                       # logged in, map, HP, script, stats
curl -H "$T" "localhost:8791/log?since=0"                # the script log (and "next")
curl -H "$T" -X POST "localhost:8791/script/start?path=farm/GoldFarm"
curl -H "$T" -X POST localhost:8791/script/stop
curl -H "$T" -X POST "localhost:8791/relogin?server=Artix"
```

The playlist:

```bash
curl -H "$T" localhost:8791/scheduler                    # entries, statuses, the current one
curl -H "$T" -X POST --data-binary @daily.json localhost:8791/scheduler/load
curl -H "$T" -X POST "localhost:8791/scheduler/add?path=farm/Leveling"
curl -H "$T" -X POST localhost:8791/scheduler/start
curl -H "$T" -X POST localhost:8791/scheduler/stop
```

`/script/stop` stops the current script, and a running playlist moves on to the next one;
`/scheduler/stop` stops the playlist. The playlist can't be changed while it runs (409).
`/scheduler/add` and `/scheduler/load` find scripts in the scripts folder but don't download
them; `--run-script` and `--run-playlist` do.

### Webhooks

Options > Application's Discord webhooks (script start, stop and crash, relogins, listed drops,
health pings; a playlist sends its own start, finish and error embeds) work without a window.
Set them up on a desktop and copy `settings.json` to the server's data folder.

## Troubleshooting

| Symptom | Look at |
|---|---|
| `error while loading shared libraries` | A missing library: "Install". |
| `No script at ...` | The path: relative paths are looked up in the scripts folder, then the working folder. |
| `Script ...: isn't in the scripts folder` | The script isn't in Search Scripts' index and isn't on disk (or there was no network). |
| `Loaded 0 script(s) from the playlist` | None of its paths were found: check them against the scripts folder. |
| `--user without --password` | `TURU_PASSWORD` isn't set in the environment Turu runs in. |
| `Login failed` / `Couldn't log in.` | The account, the password, or the server name (`--server`). |
| `Control API: ...` | The port is taken (another client?) or the token is shorter than 16 characters. |
| The monitor page asks for a key | You aren't on loopback: add `#<key>` from `monitor-key` to the link. |
