# Turu scripts

TypeScript ports of the AQW bot scripts (VibeSkua's C# `Scripts` repo). The engine loads a
`.ts` file, strips its types (oxc) and runs its `export default function main()`.

## Layout

| Path | What |
|---|---|
| `core/CoreBots.ts` | Port of `CoreBots.cs`: `Core` (join, kill/hunt, quests, drops, bank, shops, classes, logging). |
| `core/CoreFarms.ts` | The parts of `CoreFarms.cs` the ported scripts use (gold, experience, Good/Vampire rep). |
| `nation/CoreNation.ts` | The parts of `Nation/CoreNation.cs` that farm the Nulgath materials. |
| `nation/materials/0MaxBags.ts` | Port of `Nation/Materials/0MaxBags.cs`. |
| `farm/GoldFarm.ts` | Port of `Farm/GoldFarm.cs`. |
| `tsconfig.json` | Type-check config; `"turu"` maps to `../bot/js/turu.ts` (the script API). |

Folders follow the C# repo (`Nation/Materials/0MaxBags.cs` -> `nation/materials/0MaxBags.ts`).
Core files only contain what ported scripts reach; port more members from the C# files as
needed, in the same order and with the same names in camelCase.

## Type-check

```bash
cd scripts && npx -y -p typescript@5.9.3 tsc --noEmit -p .
```

The engine doesn't type-check at runtime, so run this after every change.

## Writing a script

```ts
/*
name: My Script
description: What it does.
tags: nation, example
*/
import { bot } from "turu";
import { ClassType, Core } from "../core/CoreBots";

export default function main(): void {
    Core.setOptions();

    Core.addDrop("Dark Crystal Shard");
    Core.equipClass(ClassType.Farm);
    Core.killMonster("tercessuinotlim", "m2", "Left", "*", "Dark Crystal Shard", 10, { isTemp: false });
    bot.log("done");

    Core.setOptions(false);
}
```

- Every call is synchronous: no `async`/`await`. Pressing Stop makes the next bot call throw
  `ScriptStopped`; never swallow it (`rethrowIfStopped(error)` first in a `catch`).
- Use `bot.*` (see `bot/js/turu.ts`) for raw actions and `Core.*` for the C# CoreBots helpers.
- Relative imports without extension (`"../core/CoreBots"`).
- C# named arguments for flags become option objects: `KillMonster(..., isTemp: false, log: false)`
  -> `Core.killMonster(..., null, 1, { isTemp: false, log: false })`. So do boolean parameters
  whose meaning a call doesn't show: `CheckInventory(items, 1, true)` ->
  `Core.checkInventory(items, 1, { any: true })`, `FarmVoucher(true)` ->
  `Nation.farmVoucher({ member: true })`. On/off switches stay positional (`Core.setOptions(false)`,
  `Core.toggleAggro(true)`).
- Game objects read with `bot.flash.get` get an interface (`GameItem`, `GameFaction` in
  CoreBots), not `any`; read their fields with `num` / `text` / `flag`.
- Wait with `Core.sleep()` / `Core.waitForTrue()` rather than `bot.sleep()` in long loops: Core's
  background handlers (registered quests, the AFK check) only run while the script waits in Core.
- Configuration (`soloClass`, `farmClass`, `privateRooms`, ...) are fields on `Core`.
  `Core.setOptions()` fills them from Options > CoreBots (the CBO file), as in VibeSkua. Keys
  read from that file while a script runs can be overridden in `Core.cboValues`
  (e.g. `Core.cboValues["Nation_SellMemVoucher"] = "True"`).

A fuller guide is in `docs/turu/SCRIPTING.md`.
