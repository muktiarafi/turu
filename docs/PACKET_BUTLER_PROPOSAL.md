# Proposal: Butler followers without a game client

Status: proposed, not built (2026-11-11). Today every Butler account is a full game client: a
Ruffle player running AQW, plus its bot. This proposal adds a second kind of follower that
talks to the game server over its own socket, with no Flash, as aqw-python does
(`../aqw-python`), so a phone can hold a master and several followers.

## Summary

A **packet follower** logs in over HTTP, opens the game socket itself, keeps the little state
a follower needs (map, room, cell, the players and monsters around it, its own HP, MP and
skills) from the server's packets, and acts by sending packets (`goto`, `moveToCell`,
`joinMap`, `gar`, `mv`). It reads the master's `butler/<username>.state` file like any other
follower, so the master stays an ordinary Turu tab (or window, or `--headless` client) and
needs no change. The follower's decisions stay the ones `bot/src/butler/follower.rs` makes
today, run on top of a narrow interface that both the Flash client and the packet client
implement.

It is delivered in milestones, each one ending in something that runs and is checked before
the next starts. There's no time estimate: each milestone's exit criteria say when it's done.

## Why

Memory decides how many accounts a phone holds (`docs/ANDROID_PROPOSAL.md`, "Memory decides
how many"). Recorded so far, all PSS on the emulator (`docs/ANDROID.md`):

| What | Memory |
|---|---|
| One client, no window, at the title screen | ~164 MB |
| One client farming | ~410-470 MB |
| Two tabs at the login screen | ~245 MB |

A master and one follower are therefore ~0.8-1 GB, and the Tab Limit defaults to 4 (lower it on a
phone with less memory). Most of a client is the game itself (AS3 objects, classes, SWF data,
images: STATUS.md, "Windowless mode"), which the trims can't free. A follower doesn't
need the game: it never shows a picture, and the server decides combat.
aqw-python runs a whole account as one asyncio task with a plain socket; a Rust one
should be in the tens of MB or less (measured in M0 and M3, not assumed).

Not measured yet: a real Butler master and follower on a phone. M0 does that first, since it
is also the baseline every later milestone compares against.

## What aqw-python shows

What a packet-only client has to do, all of which aqw-python does today:

| Step | aqw-python | Note for Turu |
|---|---|---|
| Login | `POST https://game.aq.com/game/api/login/now` (`core/player.py`), takes `sToken`, `userid`, the server list | Turu logs in through the game's own screen today; this is new. The servers list is already fetched (`api/servers.rs`). |
| Socket login | XML `<login z='zone_master'>` with `SPIDER#0001~<user>~<version>` and the token (`core/bot.py`) | The version string must follow the game's. |
| Join | `firstJoin`, `retrieveUserDatas`, `moveToArea` with `mondef`/`monBranch`/`uoBranch` | |
| Follow | `%xt%zm%cmd%1%goto%<name>%` when the master's cell (from `uotls`) differs | Turu reads the master's cell from its sync file instead. |
| Combat | `gar` with targets, cooldowns from `sAct` armed when the packet goes out, a backoff on "spamming the server" | The part most likely to need tuning. |
| Death | `resPlayerTimed`, then `moveToCell` back | |
| Keep alive | relogin on AFK, invalid session or a closed socket | |

## What a packet follower keeps and drops

| Butler feature (STATUS.md, "Butler") | Packet follower |
|---|---|
| Follow: goto, rescue join of the master's `map-room`, houses by name, cell jump | Kept (packets). |
| Fighting, passive and leech modes | Kept, with its own skill rotation (see M4). |
| Packet warnings (locked zone, room full, ignoring goto) | Kept (they are packets). |
| Copy master's walking | Kept as `mv` packets. |
| Parking (house, whitemap, stay) | Kept. |
| Send to player (Send All) | Kept: it's a `goto`. |
| Follow master's quests | Kept: `acceptQuest` / `tryQuestComplete` packets (later milestone, optional). |
| Obey Summons, suspend while a script runs | Obey Summons kept; a packet follower runs no scripts, so nothing to suspend. |
| Relogin re-arm | Kept (its own relogin). |
| Quest bypass table, Unlock All Quests, level 100 fake, AntiLag, Skip Cutscenes, Infinite Range, CoreBots class type | Dropped: they change the follower's own game client, which doesn't exist. Whether Infinite Range changes what the server accepts for `gar` is checked in M4. |
| Debug log | Kept: the same `butler/debug_log.rs`. |
| A window showing the follower's game | None. Its state shows in Butler's list. |

## Design

### Where it lives

`bot/src/lite/` in `turu_bot` (no Ruffle, no app crate):

* `conn.rs`: the socket: TCP to the server's `sIP:iPort`, null-terminated frames, `Z`-prefixed
  frames decompressed, writes queued from any thread, reads on one thread per client.
* `login.rs`: the HTTP login (password from `accounts.rs`' secret store), the socket login,
  `firstJoin`, relogin.
* `state.rs`: `LiteState`, filled from packets only: username and ID, area ID, map, room,
  cell, pad, x/y, HP/MP, alive, in combat, the area's players (`uoBranch`, `uotls`,
  `exitArea`) and monsters (`monBranch`, `mondef`, `ct`), the class's skills (`sAct`) and
  cooldowns.
* `actions.rs`: `goto`, `join`, `jump`, `walk_to`, `attack`, `rest`, `respawn`, each a packet
  with VibeSkua's spacing (ActionDelay 800 ms, the `world.lock` cooldowns as the game's own
  client applies them; read the AS3 when unsure).
* `client.rs`: `LiteClient`, owning the above; one per packet follower.

What Turu already has is reused rather than copied: the Interceptor's `Stream`
(`capture.rs`) already cuts the socket's bytes into packets, and `events.rs` already reads
packets for the Flash client (`uotls` AFK, deaths, drops).

### One follower, two clients

`follower.rs` calls about 15 game functions (`player::cell`, `map::name`, `map::join`,
`map::join_packet`, `map::jump`, `player::goto`, `combat::attack`, `combat::cancel_target`,
`player::alive`, `player::in_combat`, `player::logged_in`, `player::x`/`y`, `walk_speed`,
`map::loaded`, the skill timer's pause/resume/stop). They become a trait, say
`FollowerGame`, with one implementation over `Bot` (today's calls, unchanged) and one over
`LiteClient`. The follower's decisions (V5's goto, rescue, park, passive/leech, warnings) then
run on both, and its existing tests (a master and a butler against two fake games) cover both.
Flash-only steps (quest fakes, AntiLag, Infinite Range, CBO class type) stay behind
`FollowerGame::has_game_client()`.

### Hosting

* **Android:** a packet follower is started from Butler (a "Follow without a game" choice on an
  account with a saved password) or from Army > Accounts. It isn't a tab: it doesn't count
  against Tab Limit, has its own limit (Options > Application, "Packet Followers"), and shows
  in Butler's list and in the notification's lines. It holds the wake lock like a running
  script.
* **Desktop and `--headless`:** the same, from Butler; `--headless` gets `--packet-follow
  <user>` (one process can host several). `docs/HEADLESS.md` is updated with the flag.
* **Control API:** `/butler` lists packet followers with their state; start and stop routes.

### Sync files

Unchanged. A packet follower writes its `butler/<username>.state` like any client (map, room,
cell, class, logged in, following, `beat`), so masters and Butler's list see it as one more
account. It writes `script_on=0` always.

## Milestones

Each milestone has exit criteria; the next starts once they're met. M1-M3 change nothing a
user sees.

### M0: baseline

Measure what this proposal claims to fix.

* A Butler master and follower as two tabs on a real phone for an hour, screen off, with the
  per-minute memory log (`crash/memory-<pid>.log`).
* aqw-python's follower (`bot_cmds/maid_follower.py`) on a desktop, RSS over an hour, as a
  reference for what a packet client costs.

**Exit:** the numbers are in `docs/ANDROID.md`, and this proposal's "Why" table cites them.
If two full clients turn out to fit comfortably, stop here and revisit.

### M1: `FollowerGame` seam

Extract the trait from `follower.rs` with the Flash implementation only.

**Exit:** no behaviour change; `cargo test -p turu_bot` passes, including the Butler master and
butler tests; a manual follow in game on the desktop behaves as before.

### M2: connect, log in, stay in

`lite/conn.rs`, `login.rs` and the joining part of `state.rs`. A test binary (or a
`--packet-follow` with no master) logs an account in, joins `battleon`, and stays.

**Exit:** logs in on two servers; survives an hour idle (AFK handling, keep-alive); relogs in
after the socket is closed by hand and after a server's "invalid session"; the packets it sends
match those the Flash client sends for the same steps, compared with Diagnostics >
Interceptor captures. Unit tests parse recorded captures (fixtures from the Interceptor, names
and IDs scrubbed).

### M3: follow without fighting

`LiteState` for players and cells, `actions.rs` for goto/join/jump/walk, and the follower on
`FollowerGame` over `LiteClient` with fighting off.

**Exit:** a Turu master with one packet follower: the follower goes where the master goes
(map changes, private rooms, cells, houses), parks when the master goes offline, obeys Send
All, and shows the warnings for a locked zone and a full room. Its memory (RSS/PSS) on desktop
and Android is measured and recorded. Fake-game tests run the follower on both clients.

### M4: fighting

Monsters from `monBranch`/`mondef`/`ct`, skills and cooldowns from `sAct`, targeting, `gar`,
death and respawn, rest. Passive and leech modes. The rotation reads the account's advanced
skill set (`skills.rs`) in its order; conditions that need the game client (none known yet)
are skipped with a log line.

**Exit:** a packet follower kills beside its master for an hour at a farm map and at a boss
with deaths, with no "spamming the server" warnings beyond a few at the start, and no kicks;
damage and kill counts compared against a Flash follower doing the same. Whether Infinite
Range or the follower's x/y affect accepted `gar`s is settled and written down.

### M5: Android and the UI

Hosting on Android and desktop as in "Hosting": Butler's choice, the limit, the notification,
the control API routes, `--packet-follow`.

**Exit:** on a phone, a master tab and two packet followers farm for an hour with the screen
off; total memory recorded against M0; the followers come back after the service restarts
(their list saved like `android-tabs.json`, without passwords). `docs/ANDROID.md`,
`docs/HEADLESS.md`, `docs/UI.md` and STATUS.md describe it.

### M6 (optional): quests and drops

Follow master's quests (`butler/<username>.quests`) with `acceptQuest`/`tryQuestComplete`, and
picking up whitelisted drops with `getDrop`.

**Exit:** a packet follower completes the master's registered quests during an hour's farm,
with the counts matching the master's.

## Testing

* Unit tests on recorded packet captures for `LiteState` (each milestone adds its packets).
* The existing Butler fake-game tests, run against both `FollowerGame` implementations from M3.
* A fake game server for `LiteClient` (a socket that answers login and join), as the bot's
  tests fake the Flash host.
* In-game runs as each milestone's exit criteria say; Turu's weakest area is in-game checking,
  so these are not skipped.

## Risks

* **No VibeSkua reference.** CLAUDE.md asks that behaviour follow VibeSkua's C#, but VibeSkua
  always runs the game. The references are the game's own AS3 (what its client sends and
  when), the packets captured from a Flash client with the Interceptor, and aqw-python. Every
  packet the follower sends is checked against a capture first.
* **The client version string** in the socket login has to follow game updates; a wrong one
  is refused. aqw-python hard-codes it; where Turu reads it from (the game's files or an
  option) is settled in M2.
* **Combat pacing.** aqw-python's history shows repeated fixes for "too soon" and spam
  warnings. M4 keeps the server's warnings counted and visible.
* **Server-side detection.** A client that never loads assets or sends the game's usual
  traffic may look different to the server. This is the account owner's risk to judge; the
  packet follower should send what the game's client sends for the same actions and nothing
  extra.
* **Two clients to maintain.** Every Butler change after M1 has to work on both
  implementations; the shared trait and tests keep that cheap, but it isn't free.

## Decisions to make

* Ship M6 or stop at M5.
* Packet followers on the desktop too, or Android only (the desktop has memory to spare, but
  `--headless` servers running many followers would benefit).
* Whether a packet follower may also be a master (summon others): not planned.
* The name in the UI ("Packet follower", "Light follower", ...).
