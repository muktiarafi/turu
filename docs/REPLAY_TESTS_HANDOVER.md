# Tests from recorded game sessions: handover

For whoever continues (a person or an agent). It says where things stand on 2026-11-27, what is
left, what was decided and why, and the traps hit on the way. Read `CLAUDE.md` first. The design
and the full record are in `docs/REPLAY_TESTS_PROPOSAL.md` (its "As built" sections, "Phase 3",
"Phase 4 so far" and "Decision 2: the virtual clock"); this file is the short way in.

The idea: record what passes between the bot and the game in a real run (a **tape**), scrub it,
and replay it in CI against the same script; and run every converted script ("draft") against a
**fake world** seeded from a tape, on a **virtual clock**, to catch drafts that throw.

## Where it stands

| Phase | State |
|---|---|
| 0. Spike | Done. |
| 1. Recorder | Done, checked in game (`bot/src/tape/recorder.rs`, `TURU_TAPE`, `--record-tape`). |
| 2. Scrubber and replay | Done. `turu-tape scrub`, `turu_bot::testing::ReplayHost`. |
| 3. Core flows on tape | Done. Seven tapes in `bot/tests/tapes/`, each with its script beside it. |
| 4. World fake and draft smoke | Built: `WorldFake` (with monsters, drops, quests and merge shops), `bot/tests/draft_smoke.rs`, the nightly workflow and the pull-request job. Not yet seen running on GitHub. |
| Decision 2 (virtual clock) | Done: `ScriptControl`'s clock, real unless a test makes it virtual. |
| 5. Recording from Diagnostics | Not built. |

**Published:**

| Repo | Branch | Tip | Notes |
|---|---|---|---|
| `muktiarafi/turu` | `main` | `12f8829ab` | Everything up to the fake's monsters, combat, drops and turn-ins. |
| `muktiarafi/turu-scripts` | `main` | `0b1ba4d` | The `UltraGeneral` fix the draft run found. |
| `muktiarafi/ruffle-aqw` | `aqw` | `882183aa5` | Pushed with it; not part of this work (a readback fix from another session). |

**Local only, on `main`:** `e57b38d99` (HEADLESS.md split), `20d2aa453` (the fix for CI: a world
fake test compared JSON text, and CI builds with `serde_json`'s `preserve_order`), and the merge
shops (uncommitted). CI on `main` failed on that one test since `9dbfc7a75`; everything else
passed.

**Not yet looked at:** the first nightly draft run (`.github/workflows/drafts-nightly.yml`, 03:17
UTC). The "Changed drafts" job runs only on pull requests, so it hasn't run either.

## The pieces

- **Recorder** (`bot/src/tape/recorder.rs`, `redact.rs`): hooked in `Flash::call_host`,
  `Bot::on_flash_call`, `Bot::on_socket`. Redacts passwords, tokens, the login, chat, email and IP
  as each line is written. Notes what the tape can't see otherwise: `hostInfo`, the server list
  fetched over HTTP (`servers` notes, in the game's field names) and the CoreBots Options file
  (`cbo` note).
- **Scrubber** (`bot/src/tape/scrub.rs`, `cargo run --release -p turu_bot --bin turu-tape --
  scrub <tape> <new file>`): renames every player (the account "Questwalker"), renumbers account
  IDs, checks the result, never writes over a file.
- **Replay host** (`bot/src/tape/replay.rs`): reads answered from the tape (the script's own reads
  in the order it made them between its sends, others by the paced position), sends matched per
  thread (the skill timer's unchecked: decision 7), JSON text compared by content, redacted
  arguments match anything, the server list and the CBO file handed back from the tape. On the
  bot's clock; `ReplayOptions::virtual_clock`.
- **Tapes and their tests** (`bot/tests/tapes/`, `bot/tests/replay.rs`): `chute_quest`,
  `enhance_equipped`, `bank_moves`, `relogin`, `core_story` (combat), `kill_farm`, `merge_buy`.
  `replays_to_its_end` replays one to its script and checks its log; `tape_options()` is the
  virtual clock. `committed_tapes_hold_no_secrets` guards every committed tape.
- **Virtual clock** (`bot/src/control.rs`): `ClockTime`, `ScriptControl::now`, `set_virtual`,
  `participate`, `wait_until`, `spend`. Unit tests there, including the woken-waiter race.
- **World fake** (`bot/src/world_fake/mod.rs` for reads from the tape and shops, `sim.rs` for
  monsters, drops, quests and the shops' items): see the proposal's "Phase 4 so far" for each rule
  and why it exists.
- **Draft run** (`bot/tests/draft_smoke.rs`, ignored): every draft with a `main` for 60 s of play
  on a virtual clock (10 s real at most), several at once; the report (`target/draft-smoke.txt`)
  has a `progress` line (kills, quests accepted, turned in, purchases) and, per draft, the last log lines,
  the last calls at the budget and, if it ran out of real time, the clock's report.
  `KNOWN_FAKE_GAPS` lists drafts that fail for the fake's sake (now only `story/TowerOfDoom.ts`).
  `DRAFT_OPTIONS` sets the option values a draft can't start without (the Lone Wolf ultras' room
  number, a roster for them and two Army dailies); the rest run with their defaults. Each draft
  has a data folder of its own (`Bot::with_paths`), and `Partners` plays the rest of its army
  through `bot.army.set_stand_in` (`testing::ArmyStandIn`): it answers the draft's own
  `CoreLoneWolf` READY and ARRIVED lines inside its `appendLog`, on the bot's clock, and lets
  barriers through. Fight signals between players aren't played.
  `tools/drafts/changed.sh` picks a pull request's drafts for CI.

## Running things

```bash
cargo nextest run -p turu_bot replay::                       # the replays, a second or two each
cargo nextest run -p turu_bot world_fake control::tests      # the fake's and the clock's tests
DRAFT_EVERY=8 DRAFT_JOBS=10 cargo nextest run -p turu_bot draft_smoke --run-ignored only --no-capture   # ~200 drafts, ~20 s
DRAFT_EVERY=1 DRAFT_JOBS=10 cargo nextest run -p turu_bot draft_smoke --run-ignored only --no-capture   # all 1,623, ~2 min
DRAFT_FILTER=story/doomwood,farm/BuyScrolls cargo nextest run -p turu_bot draft_smoke --run-ignored only --no-capture
tools/drafts/changed.sh origin/main                          # which drafts a branch's scripts change
```

**Recording a tape** (in game; read "Rules for recording" first):

```bash
ARGS="$PWD/turu-args copy.txt" PERIOD=15 STUCK_MIN=3 TURU_TAPE=<scratch dir>/<name>.tape.jsonl.gz \
  tools/farmrun/run.sh bot/tests/tapes/<name>.ts 8
cargo run -q --release -p turu_bot --bin turu-tape -- scrub <scratch dir>/<name>.tape.jsonl.gz bot/tests/tapes/<name>.tape.jsonl.gz
```

Then check the scrubbed tape for the account's password and name (`gunzip -c <tape> | grep -cF`,
reading them from the args file without printing them) and add a test with `replays_to_its_end`.
A script that changes what it sends needs its tape recorded again: remove the old tape first
(`scrub` never writes over a file).

## Rules for recording (the owner's, and what went wrong once)

- **Credentials** are in `turu-args copy.txt` at the repo root (untracked, never commit it).
  `farmrun` passes the password as `TURU_PASSWORD`; never put it on a command line or in output.
  It has held two spare accounts: a level-16 free one (most phase 3 tapes) and, now, a level-100
  one (the kill farm and merge tapes). Never write either account's name in a file: the scrubber
  renames it in tapes.
- **Quiet server only: twig.** Never Twilly, Artix or Alteon, for anything that logs in or relogs.
- **Keep scripts off CoreBots' relogin.** `Core.relogin` falls back to `["Twilly", "Twig",
  "Safiria"]` as C# does, and the owner chose to keep it so: a CoreStory run on a quest that wasn't
  unlocked relogged to Twilly eight times (before the relogin fix, Twig always failed first). So:
  probe read-only first (which quests the account can take), make the tape's script run a quest
  only when unlocked and not done, and record with a watchdog that kills the client on any
  "Relogin Triggered", "Relogging on server", "fallback server" or "Attempting relogin" line (the
  loop in the proposal's "Phase 3"; it needs `turu pid` from farmrun's output).
- **Recording changes the account**, as any script would: CoreBots' `setOptions` banks AC items
  and toggles social options; enhancement spends gold; a merge spends its ingredients. Say what
  changed when you report.

## Next steps, in the order recommended

1. **Push `main`, then look at CI and the first nightly.** Sort any nightly failure into a
   script fix (in `muktiarafi/turu-scripts`) or a fake gap (`KNOWN_FAKE_GAPS` with the reason).
   Branch protection on `main` doesn't require the new jobs; that is the owner's setting.
2. ~~Shop contents, so merges buy.~~ Done: the draft run reads each merge script's shop, items
   and materials from its source (`shop_catalog`), and the fake sells them (proposal, "Shops").
   The merge drafts now buy (226 purchases) and farm their materials.
3. ~~Per-draft options.~~ Done: `DRAFT_OPTIONS` in the draft run (proposal, "Options per draft").
   ~~The Army's partners.~~ Done (proposal, "The Army's partners"): the ultras sync through their
   setup into their fights, and three finish. What stops them now is mostly the account (a
   SpellCrafting rank, classes it doesn't own). Left: the fight signals between players
   (`hasArmySignal`), each ultra's own; only worth it for an ultra whose fight code needs testing.
4. **Monsters no script names** (TowerOfDoom's floors find theirs by cell). A generic monster when
   the catalog has none is cheap but is guessing; only for drafts that need it.
5. **Correctness (decision 4).** The smoke run catches throws, not wrong behaviour. Comparing a
   draft's sends with VibeSkua's for the same script needs a packet log from VibeSkua on Windows.
6. **Smaller:** phase 5 (Diagnostics > Record Session, scrubbing on save); moving the few
   genuinely timer-bound engine tests (Butler follow, relogin, `gear`'s bank equip) onto the
   virtual clock (each needs `bot.control.participate()` on the test thread and fakes that reply on
   the bot's clock, not from real-time threads).

## Decisions taken (and why)

- **Tapes in the repo** (`bot/tests/tapes/`, 20 to 190 KB each), kept short.
- **Decision 7:** the skill timer's sends are unchecked (counted): it casts whatever is off
  cooldown at its own pace. Other threads stay strict.
- **Decision 2:** a fully virtual clock, not a scaled one (the owner's choice). Production keeps
  the real clock; tests that measure real timing on purpose (`turn_ins_keep_action_delay_apart`,
  `event_waits`) stay real. One committed tape (`chute_quest`) also replays on the real clock.
- **CoreBots' Twilly fallback stays as in C#**; tapes keep off it (above).
- **Turn-ins in the fake always succeed**: scripts wait for their items themselves; a strict check
  would only strand quests the fake can't supply.
- **Monster names come from the scripts**, not tapes: they never cross the Flash boundary.

## Traps

- **A parking_lot guard in one expression lives to its end.** `while let Some(x) = queue.lock().pop()`
  holds the lock for the whole body (the draft workers ran one at a time), and three
  `bot.state.lock()` in one struct literal deadlock (the draft run hung for 30 minutes). Take the
  value into a `let` first.
- **Real-time code next to the virtual clock.** A real `Instant` deadline around `bot.sleep` spins
  through instant sleeps (the relogin ran 10 virtual seconds in 100 ms and tripped the stall
  check); a real-time cache goes stale on no time (`SHARED_READ` gave 300 tries one snapshot). The
  `ClockTime` type catches mixing only where the clock is used: grep `Instant::now()` near any wait
  a script can reach.
- **A loop that polls without sleeping holds virtual time still** unless calls cost time:
  `ScriptControl::spend` charges 1 ms per Flash call on a virtual clock.
- **The app keeps `serde_json`'s key order (`preserve_order`, from Ruffle's dependencies); the
  engine's tests don't.** JSON text an engine call builds differs in key order between a recording
  and a replay: the replay compares it by content.
- **Headless runs at 2 frames a second**, so `gotoAndPlay("Login")` takes half a second: a `login`
  sent at once was lost and every scripted relogin failed in game. Relogins now wait for the login
  screen (`servers::log_out_and_in`).
- **Inputs outside Flash:** the server list (HTTP) and the CoreBots Options file are noted on the
  tape; anything else a script reads from outside the game will need the same.
- **Startup scripts started before the player loaded**, so a bare `map.join` did nothing; startup
  now waits for the player to load.
- **macOS has no `timeout` command**; and a background run you stop leaves no report.
