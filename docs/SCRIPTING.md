# Writing Turu scripts

Turu scripts are TypeScript. The API is `bot/js/turu.ts`: every member has a doc comment, and
your editor shows them once `scripts/tsconfig.json` is in use (`"turu"` maps to that file).
For downloaded scripts, open the data folder's `scripts/` in your editor: Turu writes the API
there as `types/turu.ts`, with a `tsconfig.json` that points `"turu"` at it.

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
- **Background tasks** (C#'s `Task.Run` and `new Thread`): `core/CoreTasks.ts` runs a generator
  function beside the script, stepped like a handler. `yield 500` waits 500 ms, `yield until(() =>
  cond, 10)` waits like `bot.wait.forTrue` and gives back whether `cond` came true:

  ```ts
  import { tasks, until } from "../core/CoreTasks";
  tasks.start("walk", function* () {
      bot.player.walkTo(x, y);
      yield until(() => Math.abs(bot.player.x - x) < 40, 10);
      yield 2000;
  });
  tasks.running("walk"); // still going?
  tasks.cancel("walk");  // ends it, running its `finally` blocks
  ```

  Wait with `yield` inside a task: `bot.sleep` there blocks the whole script. `scripts/README.md`
  ("Ultras") maps the C# forms.
- **Stopping**: `bot.events.on("scriptStopping", (error) => ...)` runs once the script was
  stopped (Stop, `bot.stop()`) or failed (`error` is the message), but not when `main` returns,
  like VibeSkua's `ScriptStopping`. The handler can still use the bot; the Script Loader shows
  "Stopping..." and its Force Stop button interrupts it.
- **Message boxes** (VibeSkua `ShowMessageBox`): `bot.showMessageBox(message, caption, true)`
  shows No/Yes and returns true for Yes; without `true` it shows OK. With your own buttons,
  `bot.showMessageBox(message, caption, "A", "B")` returns `{ text, value }` (the button and its
  index). The script waits for the click; closing the box is No (`{ text: "Cancel", value: -1 }`),
  and Stop ends the wait. Nobody can answer runs started from the command line, the HTTP API or
  a relogin, or `--headless`: there the message is logged and the box counts as closed.
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
- **Choices** (VibeSkua's enum options, `Option<MyEnum>`): `option.choice(name, displayName,
  description, names, default)` shows a drop-down. `choices(MyEnum)` lists a TypeScript enum's
  member names, and `bot.config.get` returns the picked name, saved as VibeSkua saves the enum
  member:

  ```ts
  enum PetChoice { None = "None", Dragon_Pet = "Dragon_Pet" }
  option.choice("PetChoice", "Choose Your Pet", "", choices(PetChoice), PetChoice.None),
  // ...
  if (bot.config.get<PetChoice>("PetChoice") === PetChoice.Dragon_Pet) { /* ... */ }
  ```
- **Option groups** (VibeSkua `MultiOptions`): pass them as the fourth argument of
  `defineScriptOptions`, keyed by the group's name. They show after the other options under that
  name (`_` as spaces) and are saved as `Group:Name=Value` lines. Read and write them with
  `bot.config.get(group, name)`, `bot.config.set(group, name, value)` and
  `bot.config.has(group, name)`:

  ```ts
  export const scriptOptions = defineScriptOptions("MergeStorage", [], true, {
      Generic: [option.choice("mode", "Mode", "", ["all", "acOnly", "mergeMats", "select"])],
      Select: [option.bool("1234", "Some Item", "")],
  });
  const mode = bot.config.get<string>("Generic", "mode");
  ```

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

`scripts/tools/cs2ts` does the first pass (`node tools/cs2ts/cs2ts.mjs <C# path>` in `scripts/`;
`scripts/README.md`, "Converting with cs2ts"): it applies the steps below and marks what it can't
convert. Then fix what `tsc` reports, read the draft against the C#, and remove its first-line
marker. By hand:

1. Mirror the path: `Nation/Materials/0MaxBags.cs` becomes `scripts/nation/materials/0MaxBags.ts`.
2. Keep the header comment. `ScriptMain(IScriptInterface bot)` becomes
   `export default function main(): void`.
3. Change `Bot.X.Y` to `bot.x.y`, and `Core.X` to `Core.x`. Port any Core members the script
   needs that aren't ported yet, in C# order.
4. Write `&` where the C# writes `&amp;` in a name (`"Crag &amp; Bamboozle"`). VibeSkua reads
   game strings back as XML text, so its names carry `&amp;`, `&lt;` and `&gt;`; Turu reads the
   game's own `&`. The engine still accepts the `&amp;` spelling in item, monster and drop names.
5. Type-check: `cd scripts && npx -y -p typescript@5.9.3 tsc --noEmit -p .`. Then also run
   `cargo test -p turu_bot --test scripts_transpile`.

## Publishing

`scripts/` is the `muktiarafi/turu-scripts` submodule. Commit a script there and push it to
`main`: its CI type-checks everything and republishes the index, and Search Scripts lists the
script on the next refresh. The header block is what the list shows (name, description, tags),
so keep it first in the file; imports must be relative so the index can follow them and a
download brings them along. `scripts/README.md` ("The index") has the details.
