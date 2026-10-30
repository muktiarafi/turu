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
  `lagKiller`, `aggroMonsters`, `aggroAllMonsters`, `skipCutscenes` and `autoRelogin` off again
  when the script ends.
- **Events**: `const off = bot.events.on("monsterKilled", (e) => bot.log(e.mapId));`, then
  `off()` to remove it. Handlers run between your bot calls.
- **Handlers** (VibeSkua `Bot.Handlers`): `bot.handlers.register("name", 1000, () => ...)` runs
  every 1000 ms, `bot.handlers.once(...)` runs once after the delay, and `remove("name")` /
  `clear()` stop them. Returning `false` from the function removes it. A name in use is replaced.
  Intervals are in milliseconds and run at 20 ms at the fastest. They run between your bot calls
  and during `bot.sleep`, never inside another handler, but not while a single long engine wait
  is blocking (such as `bot.wait.forMapLoad`); they run at the next call after it.
- **Stopping**: `bot.events.on("scriptStopping", (error) => ...)` runs once the script was
  stopped (Stop, `bot.stop()`) or failed (`error` is the message), but not when `main` returns,
  like VibeSkua's `ScriptStopping`. The handler can still use the bot; the Script Loader shows
  "Stopping..." and its Force Stop button interrupts it.
- **Imports** are relative, without extension: `import { Core } from "../core/CoreBots";`.
- **Missing API**: `bot.flash.get("world.strMapName")`, `bot.flash.callGameFunction(...)` and
  `bot.send.packet("%xt%zm%...%")` reach the game directly, using the same paths VibeSkua uses.

## Options

A script declares its options as VibeSkua does, and they show in Scripts > Edit Script Options
once the script is loaded. Export them as `scriptOptions`:

```ts
import { bot, defineScriptOptions, option } from "turu";

export const scriptOptions = defineScriptOptions("MyScript", [
    option.bool("BuyReagents", "Buy Reagents?", "Use gold to buy the reagents.", false),
    option.int("PotionQuant", "Potion Quantity", "Desired stack amount [max - 300]", 0),
    option.text("Label", "Label", "Shown in the log", "start"),
]);

export default function main(): void {
    if (bot.config.get<boolean>("BuyReagents")) bot.log("buying");
}
```

- `storage` is the file the values are saved in: `options/<storage>.cfg`, in VibeSkua's format,
  so a file from VibeSkua's options folder can be copied over.
- `bot.config.get<T>(name)` reads a value (the saved one, or the default). A name the script
  doesn't declare throws. `bot.config.set(name, value)` changes and saves a value.
- The window's Save writes the file; Cancel drops the changes. Reset Defaults only changes the
  window until you Save.
- Start opens the Script Options window and waits until it closes, as VibeSkua does. Pass `true`
  as the third argument of `defineScriptOptions` (VibeSkua `DontPreconfigure`) to skip that, and
  open it yourself with `bot.config.configure()`. That's what CoreBots scripts do: add
  `Core.skipOptions` to the options, and `Core.setOptions()` opens the window on every start until
  "Skip this window next time" is ticked:

  ```ts
  export const scriptOptions = defineScriptOptions("MyScript", [
      option.bool("BuyReagents", "Buy Reagents?", "Use gold to buy the reagents.", false),
      Core.skipOptions,
  ], true);
  ```
- `bot.config.has(name)` tells whether the script declares an option.
- The window doesn't open (`configure` returns at once) with `--headless`, or for runs started
  from the command line, the HTTP API or auto-relogin: nobody is there to answer it.
- Not ported: option groups (`MultiOptions`). Enum options aren't supported.

## CoreBots

`scripts/core/CoreBots.ts` is the port of the C# `CoreBots` (`Core.join`, `Core.killMonster`,
`Core.huntMonster`, `Core.ensureAccept`, `Core.checkInventory`, `Core.addDrop`, ...). Its
settings (classes, gear, private rooms, delays, ...) are fields on `Core`. `Core.setOptions()`
fills them from your CoreBots Options file (Options > CoreBots, read with
`bot.coreBotsOptions()`), like VibeSkua's `ReadCBO`; without a file they keep the C# defaults.
Keys read while a script runs (`Nation_SellMemVoucher`, ...) can be overridden from the script in
`Core.cboValues`. Like VibeSkua, `Core.setOptions()` moves you to your house (or /whitemap) when
the script starts in battleon, oaklore or bludrutbrawl, and when the script is stopped it goes to
"Map after stopping the Bot" (`home`/`house`, `none`/`off`, or a map name) and puts your
starting gear back on. Farms such as `Farm.gold()` use boosts when the CBO's Other > Boosters
switch is on (`bot.boosts`: the boost item is found in the inventory or bank and used every 30 s
while it isn't active; the timer stops when the script ends). `scripts/README.md` covers the
porting conventions (C# named flags become option objects, wait inside `Core.sleep`).

## Porting a C# script

1. Mirror the path: `Nation/Materials/0MaxBags.cs` becomes `scripts/nation/materials/0MaxBags.ts`.
2. Keep the header comment. `ScriptMain(IScriptInterface bot)` becomes
   `export default function main(): void`.
3. Change `Bot.X.Y` to `bot.x.y`, and `Core.X` to `Core.x`. Port any Core members the script
   needs that aren't ported yet, in C# order.
4. Type-check: `cd scripts && npx -y -p typescript@5.9.3 tsc --noEmit -p .`. Then also run
   `cargo test -p turu_bot --test scripts_transpile`.
