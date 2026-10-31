# Proposal: equipment and per-client control for Army and Butler

Status: phase 1 built (see "As built"); phases 2 and 3 not. The open questions are settled
(see "Decisions"). Covers Army clients launched "Without a window" (and "In its
own window" or in a tab, which take the same path), and Butler followers. Reference:
VibeSkua's Loadouts Manager (`Skua.Core/Services/LoadoutService.cs`,
`Skua.Core.Models/LoadoutProfile.cs`), ButlerV5's `Follower.SetupClassAndSkills`, CoreBots'
`equipClass` (`scripts/core/CoreBots.ts`).

## The problem

A windowless client has no UI at all: no inventory screen, no Options > CoreBots, no Butler
window. What the launcher can do to it today is the Army Control broadcast list (`army.rs`
`Command`: scripts, login/logout, jumps, a quest, bot options) and Close. So:

* **Gear can't be changed** without a script. To switch an army from farm to boss gear you stop
  everything, relaunch with a window, or write a one-off script.
* **CoreBots Options can't be edited** for an account that isn't the active tab: the window
  reads the class and gear drop-downs from the logged-in game (`spawn_action`), so a headless
  account's `CBO_Storage(<username>).txt` is only editable by logging it in with a window.
* **Butler's per-account settings** (class type, obey summons, `butler.json`) are edited from
  the Butler window for "this account" only, so a headless follower's class type can't be set
  from the launcher. The follower equips only the CoreBots *class* of that type (as V5 does);
  the role's six gear slots are ignored.
* **Commands are all-or-nothing.** Army Control sends every command to every client (plus
  tabs). There is no per-client Start/Stop, log view, or "only this group".
* **A crashed headless client stays dead** until you notice it under "Clear Ended".

What already helps: every child shares the data folder (it inherits `TURU_DATA_DIR`), so the
launcher can read and write any account's `CBO_Storage`, `butler.json` and sync files; the
control API runs game calls off the UI thread already; the monitor snapshot has `class` and
`equipment` (slow fields, read only while the web page is open); the engine has
`inventory.equipItem`, `bank.toInventory` and CoreAdvanced's `enhanceItem`.

## Goals

1. See and change any client's gear from the launcher, windowless or not, one client or many.
2. Loadouts: named gear sets per account (VibeSkua's Loadouts Manager), and the CoreBots role
   gear (Solo/Farm/Dodge/Boss) as loadouts too, so there is one notion of "a set of gear".
3. Edit an offline or headless account's CoreBots and Butler settings from the launcher.
4. Butler followers put on the full role gear, not just the class.
5. Per-client and per-group commands, logs, and a restart watchdog.

Not goals: buying or farming missing gear (the scripts do that), cosmetics / visual-only
equips (VibeSkua's Cosmetics plugin), changing gear while a script runs.

## Design

### 1. An inventory cache per account (`bot/src/gear.rs`)

Each client writes `gear/<username>.json` when it logs in and after an inventory change
(the same event the monitor uses to mark its slow fields), throttled to once every 5 s:

```json
{ "savedAt": 1761900000000, "bankAsOf": 1761890000000, "class": "Legion Revenant",
  "items": [ { "id": 123, "name": "Legion Revenant", "category": "Class", "equipped": true,
               "where": "inventory", "enhancementPatternId": 9, "procId": 0 } ] }
```

Inventory and bank. The bank is never loaded for the cache's sake (decision 3): the cache
takes it whenever something loaded it this login (CoreBots' `setOptions` does at every script
start, an equip that needs it, the Gear window's Load bank), and otherwise keeps the bank items
of the last file, with `bankAsOf` saying when they were read (`null`: never). A move between
bank and inventory updates both lists. It is what the launcher's
drop-downs read, so CoreBots Options and loadouts can be edited for an account that is offline
or headless. It's written from a bot thread, never in `on_flash_call` (read the game, then
write; no lock held across the Flash call, STATUS.md "Things to know").

### 2. Loadouts (`bot/src/loadouts.rs`)

VibeSkua's `LoadoutProfile` as is (class, armor, weapon, helm, cape, pet, amulet, the
enhancement of each), saved in `loadouts/<username>.json` like VibeSkua's
`Loadouts/<username>.json`, so an exported VibeSkua file reads unchanged. Plus four built-in,
read-only entries per account made from its CBO file: "CoreBots Solo/Farm/Dodge/Boss" (the
`<Role>ClassSelect` class and the `Helm/Armor/Cape/Weapon/Pet/GroundItem<n>Select` gear),
so the role gear CoreBots scripts use is also something Army and Butler can put on.

Three sources, one format (decision 2):

| Source | File | Written by |
|---|---|---|
| Account | `loadouts/<username>.json` (VibeSkua's file) | Save current, the Gear window, `bot.loadouts.saveCurrent` |
| Army | `loadouts-army.json` in the data folder (outside `loadouts/`, so no username can clash) | the Gear window's "Save to army", "Promote to army" |
| CoreBots | the account's `CBO_Storage`, read-only here | Options > CoreBots |

A plain name is looked up in the account's file, then the army file: an account overrides an
army set by saving its own under the same name. `cbo:<Role>`, `account:<name>` and
`army:<name>` pick one source. Lookups re-read the files when they changed (as `butler.json`
is), and writes re-read, change one entry and replace the file (write a temporary file, then
rename), since several processes may save at once. An army set's enhancements apply per
account like an account set's. The report says which source it used.

`equip_loadout(bot, &Loadout) -> EquipReport` does what `LoadoutService.EquipLoadoutAsync`
does, minus the generated C# script:

1. Refuse when a script runs, unless the caller is that script (decision 1). With `stop`, stop
   the script first (`ScriptControl` Stop, waiting until it has ended) and leave it stopped.
2. For each slot that isn't already worn: if the item isn't in the inventory, load the bank
   (`bank.open` and `bank.load`, as CoreBots does) unless it was loaded this login, then, if
   the item is there, `bank.toInventory` (bag space checked first); an item is `missing` only
   after the live bank was checked, never from the cached copy; `inventory.equipItem`; wait for `equipped` as Butler's
   `SetupClassAndSkills` does (6 × 250 ms).
3. Report `{ equipped: [...], alreadyWorn: [...], missing: [...], failed: [...] }`.
4. Enhancement (phase 3): when a slot's enhancement differs, call CoreAdvanced's
   `enhanceItem` through a built-in TS script run silently, then go back to the map it
   started on, which is what VibeSkua's `LoadoutEnhancerTemp` does. Off by default (it
   travels and spends gold; Enhancement is unverified in game).

Engine API for scripts (`bot/js/turu.ts`, `bot/src/api/loadouts.rs`):
`bot.loadouts.list()` (each with its `source`; an overridden army set is listed once, as the
account's), `get(name)`, `saveCurrent(name, to?: "account" | "army")` (default the
account), `equip(name): EquipReport`. A script
may call `equip` while it runs (it's the script choosing).

### 3. Control API routes (`control_api.rs`)

| Route | |
|---|---|
| `GET /gear` | The live cache (forces a write), or 409 when not logged in. |
| `POST /equip?item=<name or id>` | One item, bank fetch included. |
| `POST /bank/load` | Loads the bank and writes the cache. Same script-running rule as the equips. |
| `POST /loadout/equip?name=<loadout>` | `EquipReport` with its `source`. `name` may carry a source (`cbo:Solo`, `army:Boss`). |
| `POST /loadout/save?name=<loadout>&to=account\|army` | Saves what's worn now (default `account`). |
| `POST /butler/follow?master=<name>` / `/butler/stop` | Butler from outside (below). |

Each is a new `army::Command` variant (`Equip`, `EquipLoadout`, `SaveLoadout`, `Follow`,
`StopFollow`), so Army Control, the tabs (`control_api::local`) and the web monitor's control
path get them for free. The equip and loadout routes refuse with 409 and
`"a script is running: <name>"` while a script runs, unless the request has `stop=1`: then
the client stops the script, equips, and answers with the report and `"stoppedScript":
"<name>"`. The script stays stopped; the launcher's Start (or Start All Scripts) runs it again.

### 4. Army Control: per client, per group

* **Targets.** A drop-down above the commands: All (today's behaviour), a group from
  `accounts.json`, or the checked rows. Every command goes to the target only.
* **Row actions.** Each client row gets a "⋯" menu: Start / Stop script, Load script, Relog,
  Logs (a window that pages `/log?since=` as the monitor does), Gear, and Restart (kill and
  relaunch with the same `Launch`).
* **Gear window** (`app/src/gui/turu/gear.rs`, one per account): the worn slots, a slot's
  drop-down from the cache (inventory first, then bank, marked, with "bank as of <time>" or
  "bank not read yet" and a Load bank button while the account plays), Equip; the loadouts with a
  source chip (Account, Army, Overrides army, CoreBots) and Equip / Save current / Save to
  army / Delete, plus "Override for this account" on an army set (copies it into the
  account's file to edit) and "Promote to army" on an account set; for an army set, the items
  this account lacks, from the cache; the last `EquipReport`. Works from the cache for an offline
  account, so a loadout can be prepared before launching it. While the account's script runs,
  Equip is disabled with "Script running: <name>" until "Stop the script first" is checked
  (unchecked each time the window opens).
* **Broadcast gear.** "Equip Loadout to All" with the name (a CoreBots role or a named
  loadout) and the same "Stop the script first" check box; each client's outcome under
  RESULTS: missing items per account, and "skipped: script running" or "stopped <script>".
* **Watchdog.** An Accounts launch option, "Restart if it exits" (off by default): an exited
  client is relaunched after 30 s, at most 3 times in 10 min, then left ended with the reason.
  Exits we caused (Close, `--exit-on-script-stop`) don't count.

### 5. Offline editing of CoreBots and Butler settings

* Options > CoreBots gets an account picker (the Accounts list), defaulting to the active
  tab's character. For another account it reads that account's `CBO_Storage` and fills the
  drop-downs from `gear/<username>.json`; "[Current]" means that account's cached class.
  Opening it from a Gear window picks that account. No game call for a non-active account.
* The Butler window's Settings shows a table of every account in `accounts.json` (class type,
  obey summons) instead of "this account" only. `butler.json` is already shared and re-read
  on change, so a running headless follower picks the edit up.

### 6. Butler: full role gear, and followers from the launcher

* `ClassType` gains `equip_gear` per account (a check box, "Also put on the role's gear"):
  `SetupClassAndSkills` equips the CBO role loadout (2.) instead of the class alone. The
  skill set follows the class, as today. Off by default to keep V5's behaviour.
* Launch "follow on login": Accounts' launch setup gets "Butler: follow <character>" (blank:
  none), passed as `--butler-follow <name>`. The child calls `butler::start_follow` once
  logged in, which is how a headless army becomes a party without opening a window. Relogin
  re-arms it (Butler already re-arms after a relogin).
* Army Control's targets work with Follow / Stop following, so "the Nulgath group follows
  Alt1" is one command.

### 7. Smaller QOL

* **Snapshot equipment always on for Army clients**: ask `/snapshot` with `slow=1` every 30 s
  even when the page isn't watched, so the Army list can show class and a gear tooltip. It's
  one inventory read per client per 30 s.
* **Per-account launch overrides** in Accounts: server, script, mode and Butler master per
  account, falling back to the launch setup (VibeSkua Manager keeps one setup for all).
* **"Show" for a headless client**: relaunch it with a window (`Restart` with `headless:
  false`), keeping its script loaded. True attach isn't possible: the player has no renderer.

## Phases

| | What | Where |
|---|---|---|
| 1 | Gear cache, `equip_item` with bank fetch, `/gear` and `/equip`, Army targets and row actions (Start/Stop/Logs/Restart), the Gear window with slots only. | `bot/src/gear.rs`, `control_api.rs`, `army.rs`, `gui/turu/{army,gear}.rs` |
| 2 | Loadouts (files, CBO role entries, `bot.loadouts`), Equip Loadout to All, offline CoreBots and Butler settings, Butler role gear, `--butler-follow`. | `bot/src/loadouts.rs`, `bot/src/api/loadouts.rs`, `bot/js/turu.ts`, `butler/follower.rs`, `gui/turu/{core_bots,butler,accounts}.rs` |
| 3 | Enhancement on equip, watchdog, per-account launch overrides, the web monitor's Gear section and commands. | `loadouts.rs`, `army.rs`, `accounts.rs`, `monitor/` |

## As built (phase 1)

* `bot/src/gear.rs`: the file (`GearFile`, items by slot from their `sES`: `ar` class,
  `Weapon`, `co` armor, `he` helm, `ba` cape, `pe` pet, `am` necklace, `mi` ground), a "Turu
  Gear" thread per bot that reads the game every 5 s after a login, item or shop event and at
  least every 30 s while logged in, and writes only when something changed; `equip`,
  `load_bank`, `prepare` (the script rule) and `EquipReport`.
* Control API: `GET /gear`, `POST /equip?item=&stop=1`, `POST /bank/load?stop=1`; Army
  `Command::{Equip, LoadBank, RefreshGear, Relogin}`, `Army::restart`, `broadcast` to a list of
  usernames, `army::send_local` for tabs.
* Army Control: actions to the checked rows or everyone (Group ▾ checks a group), per-row
  buttons and ⋯ menu (redone after phase 1 as a selection list, `docs/UI.md`), the Gear window
  (`gui/turu/gear.rs`) and a launched client's Logs window (`gui/turu/client_logs.rs`).
* Differences from the design above: Restart reads the password from the keychain again
  rather than keeping it; the Gear window's Equip sends the item's ID, so two items of the same
  name can't be confused; Logs is for launched clients only (a tab has its own Logs window).
* Not checked in game or by eye: the window was built and compiles, its layout wasn't looked
  at, and nothing has equipped a real item yet.

## Testing

* `cargo test -p turu_bot` against the fake game: the cache file's contents and throttle,
  the bank kept from the last file with its `bankAsOf` and replaced when a script loads it,
  an equip loading the bank only when the item isn't in the inventory,
  `equip_loadout` (already worn, from the bank, missing, a full bag, refused while a script
  runs, `stop=1` stopping it and leaving it stopped, a script equipping its own loadout), the lookup order (account over army, the prefixes),
  two writers saving to the army file at once, the CBO role loadout parse, the new `Command::request` paths, Butler with
  `equip_gear`, `--butler-follow` arming after login.
* A real `turu --headless` child under the Army test (`TURU_EXE=... -- --ignored`): `/gear`,
  `/equip` and `/loadout/equip` against the login screen answer 409.
* By hand: two headless accounts, Equip Loadout to All with one item missing on one of them,
  a follower launched with `--butler-follow` joining the master.

## Decisions

1. **No gear change under a running script, unless asked to stop it.** Every equip that comes
   from outside the script (control API, Army Control, the Gear window, the web monitor) is
   refused while a script runs; the caller can ask to stop the script first, and it stays
   stopped. A script's own `bot.loadouts.equip` is allowed: it's the script choosing. Why:
   * CoreBots owns the gear while it runs: `equipClass` puts its role class and gear back at
     the next call, so an outside equip would silently not stick.
   * The skill engine's rotation is keyed to the class; changing the class mid-fight leaves it
     on the old one (Butler's `SetupClassAndSkills` re-equips and restarts skills for this).
   * Pause-equip-resume has the first problem, and allowing only non-class slots still loses
     to `equipClass` (role gear includes helm, cape and weapon).
   * Butler already behaves this way: a follower suspends while a script runs.

   Not VibeSkua's rule: its Loadouts Manager equips under a running script and refuses only the
   enhancement. Turu's `EquipReport` and RESULTS make the refusal visible instead.
2. **Loadouts per account and per army; the account's wins.** Per-account files stay
   VibeSkua's (`Loadouts/<username>.json` copies in and out unchanged), and one army file
   holds the sets meant for every account. Why:
   * "Equip Loadout to All" with one army set is the common case (everyone into boss gear),
     and per-account copies would have to be edited again on every account after a change.
   * Accounts don't own the same items, so one account needs its own version of a set; the
     same name keeps the broadcast a single command.
   * Shared only would lose VibeSkua's files; per account only needs a copy step that goes
     stale.
3. **The bank is cached when it's loaded anyway, and kept.** No client loads its bank just
   for the cache; the cache records it whenever it was loaded and keeps the last copy across
   sessions, dated. An equip that needs it loads it then. Why:
   * CoreBots loads the bank at every script start, so a farming account's copy is rarely
     older than its last script; an idle account's is as old as its last use, and says so.
   * A load at every login adds a popup and a round trip to every login and relogin of every
     client, including those whose gear nobody touches.
   * On demand only, without keeping it, leaves an offline account's drop-downs without bank
     items, which is the case the cache exists for.
   * The stale copy is only for the drop-downs: equips always check the live bank, so a stale
     cache can't report an item missing that the account has, or the reverse.
