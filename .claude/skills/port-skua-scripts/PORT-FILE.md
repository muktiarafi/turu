# Porting one C# file

1. **Draft it** from `scripts/`: `node tools/cs2ts/cs2ts.mjs <path in ../Scripts>` (a file or a
   folder; `--only A,B CoreX.cs` prints just those members, to paste into a partial port in C#
   order). Run `npm ci` in `tools/cs2ts` first when it has no `node_modules`.

2. **Make it type-check.** `npx -y -p typescript@5.9.3 tsc --noEmit -p .`; every
   `/* cs2ts: ... */` marker and every error is a spot to fix by hand (the list below).

3. **Read it against the C#, all of it**: the fight loop and the enhancement, potion and option
   tails alike. A draft that type-checks can still be wrong. Fix the C# bugs you find (see
   SKILL.md). Done when every method was compared; only then remove the first line
   (`// Converted by tools/cs2ts, not yet read through by hand ...`). A file you didn't read
   through keeps it, and the report says so.

## Fixes the drafts need

Each is how earlier ports solved it; follow the existing port named where one is.

- **`IScriptInterface.Instance` / `CoreBots.Instance` fields** (`private bot: Bot = ...`): delete
  them and use the module's `bot` and `Core`.
- **`out` parameters**: return the value instead (`string | null` for C#'s bool plus `out`), under
  a new name when it clashes with an overload. `TryGetValue(k, out v)` is `map.get(k)` /
  `?? 0`.
- **C# overloads** with the same name: one name per shape (PORTING.md, Calls and options).
- **`Bot.Flash.FlashCall += Listener`** ("pext", "packetFromServer"): an arrow property taking
  `EventMap["extensionPacket"]`, registered with `bot.events.on` / `off`; the C# `b.o` is
  `packet.params.dataObj` of a `json` packet. Text searched in the raw packet: search
  `JSON.stringify(packet.params.dataObj)`. See `ultrasv3/challengebosses/Kathoolv3.ts`.
- **`Bot.Skills.LoadAdvanced` / `OverrideProvider` / `SetProvider` / `Start`**: restart the
  rotation with `bot.skills.startSkills(skills, timeout, mode)` (`prototypes/InfernalArenaLW.ts`).
- **`Bot.Skills.UseSkill` used as a bool**: Turu's returns nothing; rely on the
  `canUseSkill` check before it.
- **Aura `RemainingTime`**: `bot.target.getAuraSnapshot(name)` / `bot.self.getAuraSnapshot`.
  `Bot.Target.GetMonsterAura` already returns `Aura[]`; drop the JSON parsing.
- **`DateTimeOffset` / `Stopwatch`**: Unix milliseconds from `Date.now()`.
- **Files** (sync files between clients, logs): a `bot.army` log (`writeLog`, `readLog`,
  `clearLog`; names may hold letters, digits, spaces and `_-.()[]`). See PORTING.md, Army.
- **`Bot.Config.Get` in code another script imports** (a Core, a boss a Do All runs):
  `configOr(name, fallback)`; `bot.config.get` throws for an option the started script didn't
  declare. `tools/check-library-options.mjs` finds the reads that break this.
- **A class whose `ScriptMain` another script calls** (a Do All running each boss): give it a
  `scriptMain()` method holding main's body, and make `main()` call it.
- **`Core.BuyItem(map, shop, item, quant)`**: `quant` is the quantity to end with, not the number
  to buy. C# that passes a count is a bug to fix (owned + count).
- **Error text in a `catch`**: `String(ex)` (CoreBots' `errorMessage` isn't exported), after
  `rethrowIfStopped(ex)`.
- **Unused imports** left after a fix (`SkillUseMode`, `ClassType`, `CoreBots`): tsc names them;
  remove them.
