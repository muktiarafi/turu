# Proposal: Turu Monitor, a built-in web dashboard

Status: phase 1 built (see "Phase 1 as built" below). Replaces the "Skua Nexus Webhook URL" in Options > Application
with a dashboard Turu serves itself. Reference: `../Skua-Nexus` (`relay.py`, `index.html`,
`dashboard_gui.py`).

## What Skua Nexus is, and why not integrate with it

Skua Nexus is three pieces next to VibeSkua:

| Piece | What it does |
|---|---|
| `relay.py` | A Python HTTP server on `127.0.0.1:8787`. Takes the client's **Discord webhook embeds** (`POST /api/webhooks/1/<push_key>`), scrapes them back into numbers (`extract_stats`: field names like "HP", "Gold", "Kills" through an alias table, `**bold**` names in the description, "Health Ping" titles), merges partial updates per bot name into `bots.json` (kept 24 h) and pushes snapshots to browsers over a hand-written WebSocket (`/ws`), with `/bots` polling as fallback. Optionally forwards non-ping embeds to a real Discord webhook. Reads the VibeSkua process's CPU / RAM / uptime (`/usage`, Windows only). |
| `index.html` | One page: a card per bot (status pill, map, room, level, gold, AC, inventory, script, HP/MP bars, target, Quest / Equipment / Character / Log sections, kills / drops / quests / deaths / relogins), table tabs (Scripts, Accounts, Resources), filter, select and delete, a streamer mode that blurs names, gold and rooms, "offline" after 3 min without a webhook. Display only. |
| `dashboard_gui.py` | A Tkinter manager: starts the relay, generates the webhook URL, Tailscale Serve setup for phones, appearance. Needs a **patched** `DiscordWebhookService.cs` in VibeSkua so pings carry a snapshot. |

Turu already sends that snapshot (`bot/src/webhooks.rs`, `Outbox::snapshot`), so pointing it at a
relay works today. Doing it natively is better because the relay's hard parts only exist to
undo the webhook format:

* **Lossy round trip.** Turu has typed values (`i64` gold, HP pairs, item lists) and formats
  them as Discord text ("1,234,567", "120/2500"), and the relay regex-parses them back. A bot
  is keyed by the name it finds in bold text, so a renamed field or a missing name breaks it.
* **Stale data.** State arrives once per health ping (1 min by default). Turu can read the game
  whenever a viewer asks.
* **Another runtime.** Python, a Windows-only installer and process probe, and a separate app to
  keep open. Turu is one binary on macOS, Windows and Linux, and often headless on a server.
* **Read only.** Nexus can't act. Turu already has a control API (`control_api.rs`) and Army
  commands, so the same page can start, stop and relog bots.
* **Turu knows its bots.** Its tabs are in-process (`turu::tabs()`), and Army children are
  polled every 2 s already (`army.rs`). No push key or name heuristics are needed.

## Goals

1. Feature parity with the Nexus dashboard (cards, tables, streamer mode, offline detection,
   process usage, phone access) with no extra install: tick a switch in Options > Application,
   open the link.
2. Live: values a couple of seconds old, events (drops, deaths, relogins, script crashes) as
   they happen.
3. Optional control from the page, off by default.
4. Cheap: no game reads while nobody is watching; the game never slows because of the page.
5. Works without a window (`--headless`) and across machines (a VPS farm reporting to a PC).

Not goals: Discord forwarding (the Discord webhook stays as it is), accounts / passwords over
the web, editing scripts or skills.

## Architecture

```
 Turu process hosting the monitor (the window you use)
 ┌──────────────────────────────────────────────────────────────────────────────┐
 │  tab bots (turu::tabs())          Army children (army.rs)   remote pushers   │
 │     │ Sampler thread per bot          │ GET /snapshot          │ POST /ingest│
 │     ▼                                 ▼  (their control API)   ▼             │
 │  ┌─────────────────────── Monitor hub (bot/src/monitor/) ──────────────────┐ │
 │  │ BotEntry { id, source, Snapshot, last_seen, events ring, history ring } │ │
 │  └───────────────┬─────────────────────────────────────────────────────────┘ │
 │                  │ "Turu Monitor" thread: hyper, same plumbing as control_api│
 │                  ▼                                                           │
 │   GET /  (embedded page)   GET /api/bots   GET /api/events (SSE)             │
 │   GET /api/bots/<id>/log   POST /api/bots/<id>/<command> (control only)      │
 └──────────────────────────────────────────────────────────────────────────────┘
          ▲ browser on this PC, the LAN, or a phone through Tailscale Serve
```

### 1. One typed snapshot (`bot/src/monitor/snapshot.rs`)

Pull the reading in `Outbox::snapshot` out into a struct that both the webhooks and the monitor
use; `webhooks.rs` keeps formatting it into embed fields (Discord output unchanged).

```rust
#[derive(Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub name: String,                 // Outbox::name (VibeSkua GetBotName)
    pub state: BotState,              // NotLoaded | LoginScreen | Playing | Relogging
    pub script: Option<ScriptInfo>,   // path, ScriptStatus label, running, started_at
    pub server: Option<String>,
    pub map: String, pub room_id: i64, pub cell: String, pub pad: String,
    pub level: i64, pub class: Option<String>,
    pub hp: Pair, pub mp: Pair,       // { cur, max }
    pub gold: i64, pub coins: i64, pub xp: i64,
    pub target: Option<Target>,       // name, hp, max_hp
    pub equipment: Vec<String>, pub quests: Vec<QuestRef>,
    pub inventory: Slots,             // used, total (Nexus shows "n/m" when it can)
    pub stats: Stats,                 // state.stats: kills, deaths, drops, quests, relogins, run time
    pub session: Option<Session>,     // gold / xp since script start, per hour
    pub sampled_at: u64,              // ms since epoch
}
```

Split into **fast** fields (state, HP/MP, map/cell, target, gold, stats: a handful of `Flash`
gets, most from `bot.state`) and **slow** fields (inventory, equipment, quests: whole-tree reads).
The sampler refreshes fast fields every 2 s and slow fields every 30 s, or at once after an
event that changes them (`ItemAdded`, `QuestAccepted`, `QuestTurnedIn`, `ItemBought`/`Sold`,
`MapChanged`). `bot.events.subscribe` only sets a "dirty" flag (it runs inside `on_flash_call`
and must not call Flash).

The control API gains `GET /snapshot` returning the same JSON, so Army children and remote
clients speak one schema. `/status` stays for compatibility.

### 2. The hub (`bot/src/monitor/mod.rs`)

One per app (like `turu::script_repo()`): `Arc<Monitor>` with
`RwLock<BTreeMap<BotId, BotEntry>>`, where `BotId` is `tab:<n>`, `army:<username>` or
`remote:<name>`.

* **Tabs:** one "Turu Monitor Sampler" thread per tab's `Bot` (started with the bot, stopped
  through a `Weak` like the webhook thread). It reads the game, then takes the hub's write lock
  only to store the result: never hold the lock across a Flash call (STATUS.md, "Things to
  know"); the egui UI may read the hub too.
* **Army children:** the Army poll thread already calls `/status` every `POLL_INTERVAL`; it asks
  for `/snapshot` instead while the monitor has viewers, and hands the result to the hub.
* **Remote:** `POST /ingest` (phase 4).
* **Sampling only while watched:** the hub counts open SSE streams and requests in the last
  10 s. With nobody watching, samplers sleep (`Condvar`) and do no Flash calls, the same idea as
  Nexus's `usage_broadcast_loop` that only works while a browser is connected.
* **Events:** a 200-entry ring per bot of `{ at, kind, text }` built from `BotEvent`s
  (`ItemDropped` matching the Drop List, `PlayerDeath`, `ReloginTriggered`, `ConnectionLost`,
  `Login`, `Logout`) and script start / stop / crash (the places that send `Notice`s today).
  New entries also go to SSE subscribers.
* **History:** per bot one point a minute (gold, xp, kills, drops), 6 h kept, for sparklines and
  "gold / h". In memory only.
* **Persistence:** like Nexus's `bots.json`, the last snapshot per bot goes to
  `monitor.json` in the data folder when a bot goes away and every minute, and entries older
  than 24 h are dropped, so a crashed client still shows as offline with its last state.
* **Offline:** no sample for 15 s (tabs, Army) or 3 × the push interval (remote). Nexus uses
  3 min because its data arrives once a minute.

### 3. The server (`bot/src/monitor/server.rs`)

Its own listener, on the "Turu Monitor" thread, a current-thread tokio runtime with hyper, as in
`control_api::start`; the token check (`authorized`, `same`) and `Reply` helpers move to a small
shared `http.rs`.

| Route | What |
|---|---|
| `GET /` | The page, embedded with `include_str!` (`bot/assets/monitor/index.html`; one file, no build step, no CDN, so it works offline on a LAN). |
| `GET /api/bots` | Every entry: id, source, online, last seen, snapshot. |
| `GET /api/bots/<id>` | One entry, plus its events and history. |
| `GET /api/events` | Server-Sent Events: `snapshot` (only the bots that changed, every 2 s) and `event` messages. SSE instead of Nexus's WebSocket: one-way is all the page needs, hyper streams it with a plain body, the browser reconnects itself, and Tailscale Serve passes it through. |
| `GET /api/bots/<id>/log?type=script\|debug&since=` | Log lines, paged like the control API's `/log` (Army children: proxied to theirs). |
| `GET /api/usage` | Turu's own counters: version, uptime, tabs, Army clients, bots online. No CPU / RAM (Nexus's process probe), so no `sysinfo` dependency. |
| `POST /api/bots/<id>/script/start\|stop\|pause\|resume`, `/relogin`, `/logout`, `/map/join` | **Control** (phase 3): tabs through `control_api::local`, Army children through `Army::send`, so it's the Army Control code path. Needs a control-scoped key. |
| `POST /ingest` | Phase 4: a remote client's snapshot and events. Needs the push key. |

### 4. Security

The page shows character names, gold, rooms; control can run scripts as the account. Modelled on
Nexus's, but stricter where it's cheap:

* Listens on `127.0.0.1:8790` by default (8791 is the control API). "Allow other devices"
  switches to `0.0.0.0`. Never suggests port forwarding; the docs describe Tailscale Serve, as
  Nexus does.
* Three keys in `monitor-keys.json` (owner-readable, like `api-token`): **view**, **control**,
  **push**. Requests from loopback may view without a key (Nexus does the same); control always
  needs the control key, even from loopback, because any local web page could otherwise POST
  to it. Keys go in `Authorization: Bearer`, never in query strings (except SSE, where
  `EventSource` can't set headers: the page trades the key for a short-lived ticket with
  `POST /api/ticket` first).
* Sharing a link puts the key in the URL **fragment** (`http://host:8790/#key=...`): browsers
  don't send fragments, so it's not in server or proxy logs. The page stores it in
  `localStorage` and removes it from the address bar.
* Every POST checks `Origin` against the page's own origin (no CORS allowed), and the server
  checks `Host` against the bound address or a configured name (DNS rebinding).
* "Regenerate keys" in Options cuts off every device.
* Streamer mode (Nexus's blur) stays, client side.

### 5. The page

Vanilla JS in one file, styled with the Mugetsu palette from `theme.rs` as CSS variables (dark,
plus a light theme), usable at phone width.

* **Header:** online / total, total kills, total gold / h, Turu's version and uptime, a live / reconnecting
  dot, streamer mode, settings (key, theme).
* **Cards view** (Nexus's): name, source chip (Tab 2, Army, Remote), state pill, map / room /
  cell, level and class, gold, AC, inventory, script and its run time, HP / MP bars, target,
  folding Quests / Equipment sections (open state remembered, as Nexus does), the five
  counters, gold / h and xp / h with a sparkline, the last few events.
* **Table views:** Scripts, Accounts, Resources (Nexus's three), sortable.
* **Bot detail** (new): full event list, live script log (polling `/log` with `since`), history
  charts, and with the control key: Start / Stop / Pause, Relogin, Logout, Join map, each
  asking for confirmation.
* **Selection** (Nexus's Select all) for bulk controls: stop all, relogin all; and "forget" for
  offline bots (Nexus's delete).
* Notifications: optional browser notifications for crashes and listed drops.

### 6. Turu UI changes

Options > Application gets a **Monitor** chip (in place of the Nexus entries under Webhooks):

| Setting | |
|---|---|
| Enable Web Monitor | Starts / stops the server at once. |
| Allow Other Devices | `127.0.0.1` or `0.0.0.0`; warns about the network. |
| Port | Default 8790. |
| Allow Control | Off by default. |
| Open Monitor / Copy Link / Copy Phone Link | Phone link: LAN or Tailscale name, with the view key in the fragment; shows a QR code (`qrcode` crate into an egui texture). |
| Regenerate Keys | Asks twice. |
| Report To | Phase 4: another Turu's monitor URL and push key. |

`nexus_webhook_url` stays readable in `settings.json` for one release and is ignored, with a
note in the Webhooks chip; the Discord webhook keeps everything it does now. Command line:
`--monitor[=<address>]` for headless servers, like `--api`.

## Phase 1 as built

Differences from the plan above, kept for the later phases:

* The hub is the bots themselves: each `Bot` has a `monitor::Sampler` (its latest snapshot and a
  "Turu Monitor" thread), and the server asks the app for the bots (`server::Sources`: this
  window's tabs). The hub of decision 4 comes with Army children in phase 2.
* One route, `GET /api/state` (version, uptime, every bot), instead of `/api/bots` and
  `/api/usage`; `GET /api/events` sends the same state every second as `event: state`.
* The page reads the event stream with `fetch`, which sends the key in `Authorization`, so no
  tickets are needed.
* No `--monitor` flag yet; a headless client serves it when `settings.json` has it on.
* The page lives in `bot/assets/monitor/index.html`; there's no shared `http.rs`
  (`control_api::authorized` and `saved_token` are reused).

## Phases

| Phase | Delivers | Main files |
|---|---|---|
| 1 | `Snapshot` refactor (webhooks unchanged, tests compare the embeds), hub with tab samplers, server with `/`, `/api/bots`, `/api/events`, the cards and tables, view key, Options > Monitor. Nexus parity for one process. | `bot/src/monitor/{mod,snapshot,server}.rs`, `bot/src/http.rs`, `bot/assets/monitor/index.html`, `webhooks.rs`, `settings.rs`, `application_options.rs` |
| 2 | Army children (`/snapshot` on the control API, the Army poll feeds the hub), the Army windows reading the hub (decision 4), events and history, `/api/usage`, persistence, logs in the detail view. | `control_api.rs`, `army.rs`, `desktop/src/gui/turu/{army,accounts}.rs` |
| 3 | Control key and routes, bot detail controls, bulk actions, browser notifications. | `monitor/server.rs`, page |
| 4 | Remote: `Report To` pushes snapshots and events to another hub's `/ingest` every 5 s (blocking reqwest on the sampler thread, as webhooks post); `--monitor` for headless. QR pairing. | `monitor/push.rs`, `desktop/src/turu.rs` |

Each phase is usable on its own; phase 1 is about the size of the webhook module plus the page.

## Testing

* `cargo test -p turu_bot` with the fake Flash host: the snapshot (playing / not playing), fast
  and slow refresh, no Flash calls without viewers, the hub never holding its lock across a game
  call (as `butler_state_reads_never_wait_for_a_game_call` does for Butler), offline timing.
* Server tests on a random port: key checks (loopback view without a key, control always keyed,
  bad `Origin` refused), `/api/bots` JSON, an SSE stream receiving a snapshot and an event.
* Webhook embeds unchanged after the refactor (compare field lists).
* By hand: a real login, two tabs and an Army child on the page, a phone through Tailscale
  Serve, the event loop's frame time with the page open and ten tabs.

## Decisions

1. **Only the host serves the monitor.** Army children don't open a monitor port; they report
   through Army's poll (and remote clients push to the host, phase 4). `--monitor` on a
   headless server makes that process the host.
2. **No Nexus-compatible webhook route.** VibeSkua clients don't show up; nothing scrapes
   embeds.
3. **Turu's own counters only.** No CPU / RAM and no `sysinfo`.
4. **The hub is the one store of bot state; Army keeps the processes.** `army.rs` still owns
   each `Client`'s process, PID, launch and exit (what `ClientState.exited` and `error` say),
   but its poll writes the answer into the hub instead of `ClientState.status`, and the Army
   Accounts / Army Control windows read game state from the hub. Why:
   * One poll per child instead of Army asking `/status` and the monitor asking `/snapshot`.
   * The window and the page can't disagree about a bot (online, map, script).
   * Army Control's "Include this window's tabs" gets the tabs' state from the same place.
   The hub therefore always exists (it's a map and a lock); only the HTTP server and the tab
   samplers depend on Enable Web Monitor or an open Army window, which counts as a viewer. The
   poll asks for fast fields every 2 s and slow fields only while the page is open, so Army
   costs what it does today when nobody watches. Done in phase 2, after phase 1 has settled the
   snapshot's shape.

Remote access is through Tailscale Serve (section 4).
