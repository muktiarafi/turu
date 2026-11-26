# Proposal: tests from recorded game sessions

Status: phases 0 and 1 done (2026-11-26): the spike (see "What a do-nothing game shows") and
the recorder, checked in game (see "As built, phase 1"). Phase 2 done (2026-11-27): the scrubber
and the replay (see "As built, phase 2"), with the first tape recorded in game committed and
replayed by the tests ("The first tape"). Phases 3 to 5 aren't built. Would touch `bot/src/flash.rs` (`Flash::call`), `bot/src/events.rs`
(`on_flash_call`), `bot/src/player_packets.rs` (`on_socket`), a new `bot/src/tape/` (format,
recorder, scrubber), `bot/src/testing.rs` (the replay host and the world fake), `bot/tests/`
(`replay.rs`, `draft_smoke.rs`, `tapes/`), `docs/HEADLESS.md` (the switch) and a nightly
workflow. Reference: `bot/tests/ensure_complete.rs` and `core_story.rs` (today's fake games),
`docs/HOST_SWF_PROPOSAL.md` ("Parity"), `docs/EVENT_WAITS_PROPOSAL.md` (phases 0 and 4),
`docs/HEADLESS_MONITORING_HANDOFF.md` (farmrun).

## Summary

Record what passes between the bot and the game during a real run: every call into Flash with
its answer, and every event and packet coming back. Save it as a **tape**, scrub it of secrets
and player names, and keep it as a test fixture. A **replay host** then plays the tape back to
the same script offline: it answers the script's reads from the tape and delivers the recorded
packets when the script sends what it sent in game. The test fails where the script's sends stop
matching.

Next to that, a **world fake** seeded from a tape runs each of the 1,604 cs2ts drafts for a few
seconds. It fails on a thrown error or a call the engine refuses, with no recording per script.

Neither of these exists in VibeSkua. Its solution (`../VibeSkua/Skua.sln`) has no test
project, and its scripts are checked only by running them in the game.

## Why

"Unverified in game" is the commonest phrase in STATUS.md's "Known gaps": relogin, auras,
enhancement, CoreDailies, CoreStory, the merge template, Potions, FarmerJoe, the Nation fixes,
and 1,604 of the 1,868 script files (drafts cs2ts made that type-check and load but weren't read
through). Each of these was tested against a fake game written by hand for that test: there are
about 20 `FakeGame`s in `bot/tests/`, each answering the few paths its test needs and `"null"`
to the rest. A fake can only check what its author already believed about the game. A run in
the game, the only real check, happens once and leaves nothing behind except log lines.

What a tape adds:

- **A flow checked once in game stays checked.** Record CoreAdvanced's `enhanceItem` on a
  real enhancement shop once; every later change to the engine, `turu.ts` or the Core scripts
  replays it in CI.
- **A run that went wrong in the field can be played back offline**, with the game's real
  answers, instead of being reconstructed from a log.
- **Engine changes that move timing get a check.** `docs/EVENT_WAITS_PROPOSAL.md`'s phases 4
  (state from packets) and 5 (handlers inside waits) change when things happen, and its phase 4
  asks for in-game proof, which tapes can carry.

## What we have, and why it isn't enough

- **Packet captures** (`packets*.txt` in the working tree, Diagnostics > Logger,
  `TURU_PACKET_LOG`): `[IN]`/`[OUT]` lines, with no time stamps and no Flash answers. Scripts
  learn most of what they know through `getGameObject`, `isNull`, `getMany` and `snapshot`
  reads, not packets, so a packet log can't drive a script. These files also hold real
  character names. None of them can become a tape: recording has to start fresh at the Flash
  boundary.
- **The host SWF parity mode** (`HOST_SWF_PROPOSAL.md`, "Parity") compared two SWFs live on the
  same game. That is a comparison of two hosts, not a recording, and it was removed after R6a.
- **EVENT_WAITS phase 0** already asks for packet logs of a kill, a respawn, equip, buy, sell,
  bank, map and cell changes, "kept as test fixtures". Tapes are those fixtures with the Flash
  answers added. If this proposal is taken up, it would replace that line of phase 0, not
  duplicate it.
- **Farmrun** (`tools/farmrun/`) runs a script headless for minutes and samples `/status`. It is
  the natural place to record from.

## What a do-nothing game shows (spike, 2026-11-26)

A temporary test (since removed) took 20 drafts, every 80th from the sorted list across
`army/`, `evil/`, `good/`, `legion/`, `nation/`, `other/`, `seasonal/` and `story/`. It ran
them one after another in one test against a fake game. The fake answered `isLoggedIn` true,
`intState` 1, the map `battleon`, the `world.lock` cooldowns as free, and `"null"` to
everything else. Each draft got 8 s, with `start_silent` and `ActionDelay` 0. Nextest ended the
test after 14 drafts: its `slow-timeout` (30 s, ended after 4 periods) is per test. A second run
of 3 drafts at 5 s each counted the Flash calls. In all, 15 different drafts were observed. The
last five on the list weren't, all from `seasonal/` and `story/`.

- **All 15 compiled, loaded and reached CoreBots' `SetOptions` ("Bot Started")** in 1.5 to 2 s.
  None threw while loading.
- **All 15 then stalled** and were stopped at the budget. In the counted run each made only a
  few dozen Flash calls in 3 s, led by `world.myAvatar.items.length`. The engine's
  `player::loaded` (`bot/src/api/player.rs`) reads that and answers "not loaded" while it is 0,
  so the waits behind it never end against a game with no inventory.
- **Per script:** about 2 s to start, then whatever the budget is. 1,604 drafts at 10 s each is
  about 4.5 hours on one core.

So a permissive host proves only that a draft loads, which `scripts_transpile.rs` and the
DoAll load test nearly prove already. To get past setup, the fake has to answer as a world
would: an inventory, a bank, a map that changes when joined, monsters that die. Seeding that
world from a tape's real answers is the reason the world fake comes after the recorder.

## Design

### Recording

The recorder sits at the engine's three entry points, so the desktop, headless and Android
clients all get it, and so does Turu Mas:

| Entry point | What is written |
|---|---|
| `Flash::call` (`bot/src/flash.rs`) | `call`: the name, its arguments, its answer, the thread (script, background, UI) |
| `Bot::on_flash_call` (`events::on_flash_call`) | `event`: the name (`pext`, `packet`, `event`, `error`, ...) and its arguments |
| `Bot::on_socket` (`player_packets.rs`) | `in`/`out`: the packet text, for hosts that report the socket (`player_packets()`) |

Each line also carries the time in milliseconds since the tape started. The file is JSON Lines
compressed with `flate2`, which the bot crate already depends on. A header line records Turu's
version, `hostInfo`'s version and features, the script and its options, and `BotOptions`.

`on_flash_call` and `on_socket` run on the player's thread while the player is locked, so they
must not block (CLAUDE.md, "Rules that keep it working"). The recorder pushes each line to a
channel, and a writer thread of its own compresses and writes it. When the channel is full,
lines are dropped and counted, and the tape's footer says how many. A tape with gaps fails to
load as a strict fixture.

It is switched on with `TURU_TAPE=<file>` or `--record-tape <file>` (documented in HEADLESS.md
with the other startup flags), and in Phase 5 from a Diagnostics window. By default it starts
once the player is in the world. A relogin tape has to cover the login, so redaction (below) is
what keeps secrets out, not when recording starts.

### Secrets and names

Secrets are removed by content when each line is written, not afterwards, wherever they appear:

- **Login calls:** the arguments of the login calls (`login`, the login screen's fields, what
  `servers.login` passes).
- **Credential reads:** the answers of reads under `loginInfo` (`loginInfo.strPassword`, which
  `api/servers.rs` reads for relogins, and the rest of it but `strUsername`), and any other read
  whose path names a password or token. The list is kept beside the redaction code, and a test
  fails if a new `get_static("loginInfo.…")` appears in `bot/src` without being on it.
- **The socket's login:** the socket's login packet and anything carrying a session token.
- **Chat** (`chatm`, whispers, `%xt%chatm%`): other people's words.

A redacted value is written as `"<redacted>"`. On replay a redacted argument matches anything
in its place, so a relogin tape still checks that the login was sent, and in order.

Before a tape becomes a fixture, `turu_bot::tape::scrub` renames every player in it to a made-up
one ("Questwalker", "Player 2", ...): usernames in packets (`uotls`, `ct`'s `p`, `uoBranch`), in
the answers of reads, and in the script log. Player IDs and room numbers are renumbered
consistently. The scrubbed tape's header says it was scrubbed, and the replay test refuses a
tape without that mark, so an unscrubbed recording can't be committed by accident. This
follows the owner's rule: made-up player names in fixtures, never names from logs.

### Replaying

The replay host (`turu_bot::testing::ReplayHost`, a `FlashHost`) keeps a **tape position**.
What it does is decided by the kind of line:

- **Reads** answer state, not a count. Reads include `getGameObject`, `isNull`, `getMany`,
  `snapshot`, `isLoggedIn` and the other callbacks that change nothing in the game (a list
  kept in `testing.rs`, from `docs/VIBESKUA_SPEC.md` §2). A read gets the latest answer
  recorded for the same name and arguments at or before the current position. Polling waits
  and background threads repeat reads as often as their timing makes them, so matching reads by
  how many times they happened would diverge at once.
- **Sends** move the position. Sends are calls that act on the game (`callGameFunction` of a
  `world.*` action, `sendString` packets, `out` socket lines). Each one the bot makes must equal
  the next send on the tape, with room numbers normalised. The position then moves to that send.
  The events and `in` packets recorded between it and the next send are delivered in order,
  through `on_flash_call` and `on_socket` as the game would deliver them, each after its
  recorded delay.
- **Sends are matched per thread.** The skill timer, drop pickups, registered quests and the
  background timer send from threads of their own, so how their sends interleave with the
  script's depends on timing. Each send is matched against the next send recorded from the same
  thread, and the script thread's sends decide the position. A background thread's sends are
  checked only for their own order, not for where they fall among the script's. Whether that is
  strict enough for combat is checked with the first combat tape (decision 7). The first tapes
  are quest and shop flows, which send from the script thread.
- **Noise sends** (`restRequest`, the background timer's re-applied modules, a `moveToCell` to
  the cell you're in) are kept in a per-tape table. They may appear in any number in either
  run.
- **A read with no recorded answer** answers `"null"` and is counted. In strict mode, the
  default for committed tapes, the test fails on the first one, naming the key and the tape
  position.

A test asserts the order of the sends, the script's last status, and that the replay reached
the tape's end. It doesn't assert timing or how many reads were made.

On failure the message shows the expected send and the actual one, the last ten tape lines
before the position, and the script log's tail. That message is the point of the tool: it
should say at a glance "CoreAdvanced picked the Awe-shop enhancement 1234, the tape picked
5678".

**Time** is the engine's real clock: `ScriptControl` sleeps on `Instant`, and the engine has no
test clock. Delays are cut where Turu owns them: `ActionDelay` and the skill interval come from
the tape's `BotOptions`, so a tape recorded with `ActionDelay` 800 replays at 800. Recorded
event delays can be shortened by a factor. Whether to give `ScriptControl` a virtual clock is a
decision below.

### The world fake, for the drafts

`turu_bot::testing::WorldFake` starts from the reads answered at the end of a tape's setup: a
character with an inventory, a bank, a class and a map. It then reacts to sends with a short
list of rules:

- `joinMap` sets the map and cell.
- `moveToCell` moves you.
- `acceptQuest` and `tryQuestComplete` move a quest through its states.
- `buyItem` and `sellItem` change the inventory and answer with their packets.
- Attacking a monster kills it after a few hits, with its `ct` packets.
- An item a script waits for drops after a kill.

It is a model, so it will be wrong in places. Its job is only to get a draft past setup into its
own code. It doesn't vouch for that code being right.

The draft smoke test (`bot/tests/draft_smoke.rs`) runs each draft against it for a budget. It
fails on:

- a thrown script error (status `Error`);
- a `BotError::InvalidArgument`, `UnknownMethod` or `NotInHostSwf` from `api::dispatch`;
- a call to something `turu.ts` doesn't have.

It passes when the draft ends cleanly or is still running at the budget. That catches the
cs2ts failures that leave a script dead in the field: an undefined member, wrong argument types,
an async C# pattern rewritten wrongly. It can't catch a draft that is wrong but runs, such as
integer division or the wrong quest ID. For those, only a tape of that flow or a comparison with
the C# helps (see the open decisions).

At about 12 s a draft it is too slow for every pull request. It runs nightly over all drafts,
and on pull requests over only the scripts and Core files they change.

## As built, phase 1

`bot/src/tape/` (`mod.rs`: the format, `read`, the slot; `recorder.rs`; `redact.rs`), hooked in
`Flash::call_host`, `Bot::on_flash_call` and `Bot::on_socket`, with notes from
`script::start_with` and `finish`; `Bot::start_tape` / `stop_tape`; `TURU_TAPE`,
`TURU_TAPE_FROM_START` and `--record-tape` (`frontend/src/turu.rs`; the shell passes it to its
first tab's client, and doesn't record itself); `close_butlers`, which every exit goes through,
finishes the tapes. Tests: `bot/tests/tape.rs` (with a fake game: when recording starts, the
socket, every secret above and chat absent from the file, a tape cut short, file names, the
script notes, and every `loginInfo.` path in `bot/src` redacted). As designed above, except:

- **Order:** each line's `n` is taken when its event begins (a call's when it is made), and
  `read` sorts by it, so a call comes before the packets it caused although its line is
  written after its answer. Dropped lines leave gaps in `n` (`Tape::missing`).
- **The header** holds the format, the engine's version, the start time, `fromWorld`, the bot's
  options and a `scrubbed` flag (false). `hostInfo` isn't in it: its call is on the tape when
  the client asks (a recording from the start), and phase 2 needs it from the header for tapes
  that start at a map, so it moves there then.
- **The script's options** are in its `scriptStarted` note (their values as the control API
  shows them, without the file's path), not the header: a tape can hold several runs. The script
  is named by its path under the scripts folder, else under any folder named `scripts` (the
  repo's checkout), else by its file name: never the whole path.
- **Redaction** also covers a JSON answer's or `pext`'s fields named like a password or token
  (`sToken`, `strPassword`; not `TokID` or `iTokens`), whatever the path, and sets of a secret
  path. Chat is replaced whole, `pext`s included, so a chat `pext` replays as text that isn't
  JSON.
- **Calls in flight at the stop:** a stop waits up to 0.5 s for calls already made to be
  answered; the `end` line lists the rest (`unanswered`), whose numbers aren't gaps.
- **`hostInfo`** is a note when the recording opens (from what the client kept when it loaded,
  without asking the game), and at once for a recording from the start.
- **The version** is the build's (`TURU_VERSION` in releases, `updates::current_version`).
- **Where it records:** a window's shell records nothing (`tape::disable_startup`, in the app
  before the first bot is made); its tabs' clients do, inheriting `TURU_TAPE` (only the first
  gets `--record-tape`). A client's own exits (`client::exit_now`, the end of `main`) finish the
  tape, as `close_butlers` does for the others.
- **Checked for real** (2026-11-26): a debug build headless with no login, `TURU_TAPE` a folder
  and `TURU_TAPE_FROM_START=1`, stopped with SIGTERM after about 40 s at the login screen: 22 KB,
  1,777 lines (1,702 of them the background timer's `isNull`s), no gaps, every caller named
  (`ScriptInterface`, `Butler Broadcast`, `Turu Startup`), no `end` line as expected.
- **Checked in game** (2026-11-26, phase 1's exit): FarmerJoe DoAll headless on a spare account
  on a quiet server (twig), debug build, 20 minutes recording (run A) and 20 without (run B),
  each ended by `POST /script/stop` with `--exit-on-script-stop`.
  - **The tape:** 86,478 lines, 0 dropped, no gaps, the `end` line written. 3.1 MB compressed
    (about 9 MB an hour), 39 MB of JSON before compression. Calls are 95% of the lines and
    almost all the bytes: `getMonsters` (8.4 MB), `snapshot` (5.2 MB), `world.myAvatar.dataLeaf`
    (5.0 MB), quest trees. By thread: Script Thread 45k, Skill Timer 26k, ScriptInterface 6k,
    Butler Broadcast 4k. `hostInfo` was noted (version 11, 68 callbacks).
  - **Secrets:** the account's password appears 0 times in the decompressed tape. No field named
    like a password or token is left. 35 chat lines were replaced. The username is there (3,855
    `loginInfo.strUsername` reads), for the scrubber.
  - **Cost:** the player's thread was unchanged: 77 ms/s and 0.79 ms per call with the tape, 76
    and 0.64 without. The 0.15 ms is within run-to-run differences: run A joined 14 maps and
    run B 8, as DoAll did different things. The process used 156 s of CPU with the tape and 116 s
    without, the same workload caveat. The writer's own work on that tape, timed offline:
    4.5 s for the 20 minutes in a debug build (redaction and JSON 2.3 s, gzip 2.2 s), 0.2 s in a
    release build, so it costs nothing to speak of. Redaction parses only JSON that names a
    password, token or `pword` field (parsing every JSON answer would have taken 1.1 s more).
  - **Not checked:** a window's shell with tabs, and a relogin on tape (`TURU_TAPE_FROM_START`).
  - **Found later** (phase 2): `initUserData` carries the account's email and IP address
    (`strEmail`, `strIP`), which this tape kept. Redaction now takes fields named like an email
    or an address (`strIP`, `ip`; not `ip0`..., quest progress), and the scrubber does the
    redaction again, so tapes recorded before are covered.
- **Not done:** Android and Turu Mas have no way to switch it on yet (no environment there); the
  Diagnostics switch is phase 5.

## As built, phase 2: the scrubber

`bot/src/tape/scrub.rs` (`tape::scrub`, `tape::scrub_file`), `tape::write`, and the
`turu-tape` command (`bot/src/bin/turu-tape.rs`: `info <tape>`, `scrub <tape> <scrubbed>`; run
it with `cargo run --release -p turu_bot --bin turu-tape -- ...`). Tests:
`bot/tests/tape_scrub.rs`. As designed above, except:

- **Who is found:** the account from its reads (`loginInfo.strUsername`, `sfc.myUserName`,
  `objData.strUsername`) or `initUserData`; other players from the places the real captures
  name them: `strUsername`, `uoName` and `unm` fields anywhere (`uotls`, `uoBranch`,
  `initUserDatas`, `houseData`), `ct`'s `p` keys, `world.areaUsers` and `world.uoTree`, string
  `uotls` and `exitArea` packets, SmartFox's `<n><![CDATA[...]]></n>` (`joinOK`, `uER`), and the
  options' `customName` and `customGuild`. A name only some other packet carries (a friends list,
  a guild's members) isn't found: none was in the captures.
- **How it is replaced:** the account becomes "Questwalker", the others "Player2", "Player3", ...
  in order of first appearance, everywhere in the tape (calls' arguments and answers, events,
  packets, notes, the options), in the case each place had (`ct`'s keys are lowercase), at word
  boundaries so a short name doesn't rename an item that starts with it. A player named like a
  word of the game's ("Dragon") renames that word where it stands alone too.
- **Numbers:** `UserID` and `CharID` (fields named so, and reads of them) are renumbered from
  100000001, wherever the number shows. The room's player numbers (`uid`, `entID`, `p:<uid>`) and
  room numbers stay, unlike the design above: they last a session and name no one, and they show
  in too many places (sends, answers, packets) to renumber without risking the tape's
  consistency.
- **Personal fields:** `sCountry` becomes "XX", `iAge` 0, `dCreated` a fixed date; `strEmail` and
  `strIP` are redacted (above).
- **Checked:** `scrub_file` writes a new file (never over one), reads it back, and removes it if
  any name, number, email or address it replaced is still there. Scrubbing twice gives the same
  tape.
- **Checked on the farm tape** (2026-11-27): 86,478 lines in 3.6 s (release). Afterwards the
  account's name (16,635 places before), email, address, `UserID` and `CharID` appear 0 times;
  the lines and the end line are the same; a second scrub is identical. That tape had no other
  player in it: the other players' shapes are checked only by the tests, which follow the real
  captures' packets with made-up names.

## As built, phase 2: the replay

`bot/src/tape/replay.rs` (`turu_bot::testing::ReplayHost`, `ReplayOptions`, `ReplayReport`,
`ReplayFailure`). A test makes a bot, `ReplayHost::open(tape, options)`, `attach(&bot)`, loads and
starts the script, then `wait(&bot, timeout)`: `Ok` with the sends matched, or `Err` with the
message, the script stopped. Tests: `bot/tests/replay.rs`, which records a quest loop (three
`ensureAccept`/`ensureComplete`, the fake answering after 80 and 150 ms from threads of its own,
then a `world.goto` to the account's own name) against a fake game, scrubs it and replays it:
the same script passes and counts the three turn-in replies the tape delivered, as `pext`s or,
when the player reported the socket, as its messages; the account's name is the scrubbed one in
the read and the send alike; the quest ID changed to 4778 fails naming both IDs; a read not on
the tape fails in strict mode and answers `"null"`, counted, otherwise; a script that stops
sending is reported stuck, one that ends early as not reaching the end; an unscrubbed tape is
refused. Each takes 1 to 4 s. As designed above, except:

- **What is a read:** a list of turu.swf's callbacks that change nothing, plus `callGameFunction`
  of a function named `is…`, `has…` or `can…` or on a short list (`artLoaded`, `getRep`,
  `getQuestValue`, ...). **Noise** is a list of the callbacks that switch the client's own
  behaviour (`setLever`, `killLag`, `modEnable`, ...), answered as reads, plus the tape's
  `noise` table, named by `replay::label` (`world.rest`, `packet:restRequest`). Everything else,
  `setGameObject` and any callback not on the lists included, is a send: a getter missing from
  the list fails, naming itself, rather than an action passing unchecked. The lists have run only
  over the tests' tapes, not a tape from the game: the first recording may move callbacks between
  them (a background thread's `setGameObject` as a strict send is a guess).
- **Where reads look:** between two of the script's sends the position walks at the recorded
  pace (`delay_scale`) and stops before the next send. A read answers the latest answer recorded
  for the same name and arguments before the position, else the first after it (a tape's first
  reads come before anything else). When a send matches, the lines before it on the tape are
  delivered first, then the position moves past it.
- **The socket's lines aren't matched:** the bot under replay doesn't write the socket (the player
  did), so `Sent` and `Received` lines are delivered through `on_socket`, like events, and only
  calls into Flash are matched.
- **Room numbers aren't normalised:** the scrubber keeps them and the replay's reads come from the
  same tape, so a send repeats the recorded room.
- **Other threads' sends** must each match the next send recorded from the same thread name;
  one that doesn't, or comes after its thread's last, fails the replay. The quest tape has none
  (its background threads only read), so decision 7 is still open.
- **The stall check:** starts at the script's first call (not while it compiles). Its next send
  is due when it came on the tape after the last, unscaled (`ActionDelay` and the script's sleeps
  aren't scaled), plus `stall` (10 s by default). Past that the replay fails with the awaited send, the ten tape lines
  before it and the script log's end. A replay that fails returns errors to every later call
  (the script sees "the app is closing") and `wait` stops the script.
- **`hostInfo`** comes from the recording's note (or a recorded `hostInfo` call), not the header:
  the note is written when the recording opens, so a tape that starts at a map has it too. When
  the tape has it, `attach` asks for it, so `Flash::call` refuses what that host lacked. The
  version check of decision 5 isn't done: the engine has no constant for turu.swf's version to
  compare with (it is in `host/as3`'s `Main.as`).
- **The options:** `attach` sets the bot's options from the header (`ActionDelay`, ...).
- **Strict mode** (the default) also refuses a tape with lines missing or cut short.

## The first tape (phase 2's exit)

`bot/tests/tapes/chute_quest.tape.jsonl.gz` and its script `chute_quest.ts` beside it: quest
8543 in /chute ("You Chute, You Score"), accepted, its map item (9837) picked up and turned in
three times, with the plain script API (not CoreBots, so a change to Core doesn't break it). No
combat: the script thread makes every send. Recorded 2026-11-27 on a spare account on twig with
a debug build headless: `ARGS=<the account's args file> TURU_TAPE=<file>
tools/farmrun/run.sh bot/tests/tapes/chute_quest.ts 10`, then `turu-tape scrub` into the folder.

- **The tape:** 547 lines over 13 s, none dropped, 20 KB. The account's name was replaced in 14
  places and its two IDs renumbered; the password, the name and the email appear 0 times.
- **Its replay** (`replay::the_chute_quest_tape_replays`): 12 sends matched, no read missing, no
  other thread's send, in 11 s (`ActionDelay` 800 from the header is real time; the events'
  delays at a quarter). `the_chute_quest_tape_fails_on_another_quest_id` changes the script's
  quest to 8544: it fails at the script's third send, the first `acceptQuest`, naming both IDs.
- **The secrets check** (`replay::committed_tapes_hold_no_secrets`, in CI with the other tests):
  every tape in `bot/tests/tapes/` is scrubbed, whole, and unchanged by redaction run again, so
  no password, token, login message, email, address or chat is left in it.
- **A first attempt failed in game, not in the replay:** the script started a second after the
  login, while the inventory was still loading, and `map.join` does nothing until the player is
  loaded (as VibeSkua's `Join`). It stayed in oaklore and every turn-in was refused ("Missing Turn
  In Item"). The script now waits for `bot.player.loaded` and stops if it isn't in /chute.
  CoreBots' `Join` waits the same way; a script on the bare API has to.
- **To record it again** (a change to what the script sends): the same command with the
  account's args file, then remove the old tape and scrub the new one into its place (`scrub`
  never writes over a file).

## Phases

No time estimates: each phase ends when its "Done when" holds.

| Phase | What | Done when |
|---|---|---|
| **0. Spike** | Done (above): drafts against a do-nothing game. | Findings written here. |
| **1. Recorder** | The tape format, the recorder at the three entry points, the writer thread, redaction, `TURU_TAPE` / `--record-tape`, HEADLESS.md. | A headless farmrun on a quiet server writes a tape. Its size per hour is measured. A test with a fake login finds no secret in it. Recording costs no measurable player time in `/debug/perf`. |
| **2. Scrubber and replay** | `tape::scrub`, `ReplayHost`, strict mode, the failure message, `bot/tests/replay.rs` with the first tape (a quest turn-in loop, like `ensure_complete.rs`'s, recorded for real). | CI replays it. Changing the quest ID the script sends makes it fail with a message that names both IDs. The scrubbed tape holds no name from the recording account. |
| **3. Core flows on tape** | One tape each, in this order: a merge buy (SynderesMerge), `enhanceItem`, bank moves, CoreStory's `mapItemQuest` then `killQuest`, a relogin on a quiet server, then a kill farm with drops (the first with busy background threads, which settles decision 7). | Each replays in CI, and its "unverified in game" line in STATUS.md is rewritten to point at its tape. |
| **4. World fake and draft smoke** | `WorldFake` seeded from a phase 3 tape; `draft_smoke.rs` with one test per draft (nextest's `slow-timeout` is per test and ended the spike's single test), or its own nextest profile; the nightly workflow; pull requests run only the changed scripts. | Every draft runs nightly. The first night's failures are sorted into script fixes (in `muktiarafi/turu-scripts`) and fake gaps. The time per draft is recorded here. |
| **5. Recording for bug reports** (optional) | Diagnostics > Record Session (both shells), scrubbing on save. | A user can attach a scrubbed tape to an issue, and a developer can replay it. |

Phases 1 and 2 are the core of it. Phase 3 is where the value accrues, and it grows one tape at
a time with no further engine work. Phase 4 can start after the first phase 3 tape.

## Decisions to make

1. **Where tapes live.** An hour of farming may be several MB compressed (phase 1 measures
   it). Options: in the repo under `bot/tests/tapes/`, kept short (a few minutes each); Git LFS;
   or a repo of their own fetched by the test. Short tapes in the repo are the default
   suggestion.
2. **A virtual clock.** Replays run in real time, which is fine for minutes-long tapes but not
   for hours. A test clock in `ScriptControl` (sleep and deadlines on a clock the test advances)
   would make replays instant. It would touch every wait, so decide after phase 2 shows how slow
   real time is.
3. **Which account records.** Tapes come from real runs. They need a test character that may
   be renamed in fixtures, run on a quiet server (never Twilly, Artix or Alteon). The emulator's
   tabs are off limits (CLAUDE.md, "standing requests").
4. **Comparing with the C#.** Wrong-but-valid drafts would be caught by comparing a draft's
   sends with VibeSkua's for the same script on the same tape. VibeSkua runs only on Windows and
   has no recorder. Its Logger's packet log for one run of the C# script could serve as the
   expected sends, with the reads left uncompared. Worth a spike once phase 2 exists.
5. **Old tapes after turu.swf changes.** A tape records `hostInfo`'s version. Replay refuses a
   tape whose host major version differs, and the fix is to record it again. The alternative,
   translating old answers, isn't worth it.
6. **Shadow mode offline.** EVENT_WAITS phase 4 compares packet state with Flash reads in game.
   A tape has both, so the comparison can run offline over every tape. Decide there whether it
   replaces the in-game shadow switch or comes before it.

7. **How strict to be about background threads in combat.** Matching per thread keeps each
   thread's order. Whether the skill timer's sends need checking at all, or only their count
   within a band, is decided on the first combat tape.

## Risks

- **Brittleness.** A replay fails on any change to a script's sends, including deliberate
  ones. The fix is to record the tape again, which needs the game. Keep tapes short and focused
  so recording again is cheap, and use the noise table rather than loose matching.
- **The world fake becomes a second game.** It is easy to keep adding rules. Each rule should
  exist because a draft needed it to get past setup, and the fake's answers come from tapes, not
  from guessing.
- **Secrets in fixtures.** Redaction at write time, the scrubbed mark, and a CI check that greps
  committed tapes for the login packet's fields.
