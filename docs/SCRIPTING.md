# Writing Turu scripts

Turu scripts are TypeScript. The API is `bot/js/turu.ts`: every member has a doc comment, and
your editor shows them once `scripts/tsconfig.json` is in use (`"turu"` maps to that file).

## The smallest script

```ts
/*
name: Kill Frogzards
description: Kills frogzards in Greenguard West.
tags: example
*/
import { bot } from "turu";

export default function main(): void {
    bot.options.privateRooms = true;
    bot.map.join("greenguardwest", "West4", "Left");
    bot.skills.startAdvanced();            // the saved rotation for your class
    for (let i = 0; i < 10; i++) {
        bot.kill.monster("Frogzard");
    }
    bot.log("done");
}
```

Load it with Scripts > Script Loader > Load Script, then press Start Script. Output goes to the
Script Loader's SCRIPT LOG (and `console.log` goes there too).

## Rules

- **Everything is synchronous.** Each call waits for the game, so a script reads top to
  bottom. There is no `async`/`await`.
- **Stop.** Pressing Stop makes the next bot call (or `bot.sleep`) throw `ScriptStopped`.
  Don't swallow it: in a `catch`, rethrow `ScriptStopped` first. A script stuck in a loop that
  never calls the bot is still interrupted.
- **Name or ID.** Items and monsters take a name (case-insensitive) or an ID:
  `bot.inventory.contains("Gem of Nulgath", 100)` or `bot.inventory.contains(6136, 100)`.
- **Timeouts count tries** of about 100 ms, as in VibeSkua: `bot.wait.forTrue(() => cond, 20)`
  waits about 2 s.
- **Options** apply at once: `bot.options.lagKiller = true`. The engine turns
  `lagKiller`, `aggroMonsters`, `skipCutscenes` and `autoRelogin` off again when the script ends.
- **Events**: `const off = bot.events.on("monsterKilled", (e) => bot.log(e.mapId));`, then
  `off()` to remove it. Handlers run between your bot calls.
- **Imports** are relative, without extension: `import { Core } from "../core/CoreBots";`.
- **Missing API**: `bot.flash.get("world.strMapName")`, `bot.flash.callGameFunction(...)` and
  `bot.send.packet("%xt%zm%...%")` reach the game directly, using the same paths VibeSkua uses.

## CoreBots

`scripts/core/CoreBots.ts` is the port of the C# `CoreBots` (`Core.join`, `Core.killMonster`,
`Core.huntMonster`, `Core.ensureAccept`, `Core.checkInventory`, `Core.addDrop`, ...). Script
settings that C# read from the CoreBots options file are fields on `Core`; set them before
`Core.setOptions()`. `scripts/README.md` covers the porting conventions (C# named flags become
option objects, wait inside `Core.sleep`).

## Porting a C# script

1. Mirror the path: `Nation/Materials/0MaxBags.cs` becomes `scripts/nation/materials/0MaxBags.ts`.
2. Keep the header comment. `ScriptMain(IScriptInterface bot)` becomes
   `export default function main(): void`.
3. Change `Bot.X.Y` to `bot.x.y`, and `Core.X` to `Core.x`. Port any Core members the script
   needs that aren't ported yet, in C# order.
4. Type-check: `cd scripts && npx -y -p typescript@5.9.3 tsc --noEmit -p .`. Then also run
   `cargo test -p turu_bot --test scripts_transpile`.
