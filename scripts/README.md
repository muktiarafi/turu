# Turu scripts

TypeScript ports of the AQW bot scripts (VibeSkua's C# `Scripts` repo). The engine loads a
`.ts` file, strips its types (oxc) and runs its `export default function main()`.

## Layout

| Path | What |
|---|---|
| `core/CoreBots.ts` | Port of `CoreBots.cs`: `Core` (join, kill/hunt, quests, drops, bank, shops, classes, logging). |
| `core/CoreFarms.ts` | The parts of `CoreFarms.cs` the ported scripts use (gold, experience, alchemy crafting, the Bludrut and Death Pit brawls, Battle Under B, fishing and its boosts, and the Alchemy, Blacksmithing, Blade of Awe, Chaos, ChronoSpan, Death Pit Brawl, DoomWood, Evil, Fishing, Glacera, Good, Loremaster, Mythsong, SpellCrafting, Swordhaven and Vampire reps). |
| `core/CoreStory.ts` | Port of `CoreStory.cs`: `killQuest`, `mapItemQuest`, `buyQuest`, `chainQuest`, `questProgression`, `legacyQuestManager`, `preLoad`. Each script makes its own `new CoreStory()`. |
| `core/CoreDailies.ts` | Port of `CoreDailies.cs`: every daily, weekly and monthly (`checkDailyv2`, `dailyRoutine`, metals, class tokens, boosts, friendships, ...). Each script makes its own `new CoreDailies()`. |
| `core/CoreAdvanced.ts` | The shop, gear and enhancement parts of `CoreAdvanced.cs`: `buyItem` farms the reputation, level, gold, vouchers and runestones a purchase needs; `startBuyAllMerge` is the merge-shop template (with `mergeOptions` and `mergeOptionsEnum`); `enhanceItem`, `enhanceEquipped`, `smartEnhance` (the class library), `gearStore`, `rankUpClass`, `hasMinimalBoost` and the enums `EnhancementType`, `CapeSpecial`, `HelmSpecial`, `WeaponSpecial`, `GenericGearBoost`, `RacialGearBoost`. Not yet: the Kill region. |
| `nation/CoreNation.ts` | The parts of `Nation/CoreNation.cs` that farm the Nulgath materials, Swindles Return, The Assistant and the Shadow Blast medals and emblems. |
| `nation/materials/0MaxBags.ts` | Port of `Nation/Materials/0MaxBags.cs`. |
| `farm/GoldFarm.ts` | Port of `Farm/GoldFarm.cs`. |
| `farm/CombatTrophy.ts` | Port of `Farm/CombatTrophy.cs` (`Farm.bludrutBrawlBoss`). |
| `other/mergeshops/SynderesMerge.ts` | Port of `Other/MergeShops/SynderesMerge.cs`: the pattern for every merge script (see below). |
| `other/various/Potions.ts` | Port of `Other/Various/Potions.cs`. Its options are declared in `scriptOptions` and edited in Scripts > Edit Script Options. |
| `other/farmjoekits/0FarmerJoeDoAll.ts` | Port of `Other/FarmJoeKits/0FarmerJoeDoAll.cs`: the level 1 to 100 progression. `CoreFarmerJoe.ts` next to it holds the steps and the options; the rest of its tree is listed under [FarmerJoe](#farmerjoe). |
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
- C# overloads whose shapes clash get their own name: `EnsureComplete(params int[])` ->
  `Core.ensureCompleteQuests(...ids)`, `HuntMonsterMulti(map, mon, (item, quant, isTemp)[])` ->
  `Core.huntMonsterMulti(map, mon, [[item, quant, isTemp], ...])`.
- `Bot.UltraBossHelper` is `bot.ultraBossHelper`; `Bot.Combat.StopAttacking` is
  `bot.combat.stopAttacking`.
- Game objects read with `bot.flash.get` get an interface (`GameItem`, `InvCatSettings` in
  CoreBots), not `any`; read their fields with `num` / `text` / `flag`.
- Wait with `Core.sleep()` / `Core.waitForTrue()` rather than `bot.sleep()` in long loops: Core's
  background handlers (registered quests, the AFK check) only run while the script waits in Core.
- Configuration (`soloClass`, `farmClass`, `privateRooms`, ...) are fields on `Core`.
  `Core.setOptions()` fills them from Options > CoreBots (the CBO file), as in VibeSkua. Keys
  read from that file while a script runs can be overridden in `Core.cboValues`
  (e.g. `Core.cboValues["Nation_SellMemVoucher"] = "True"`).

## Enhancing

The specials C# passes positionally or by name become an option object; a list of names is the
same method:

- `Adv.EnhanceItem(cape, EnhancementType.Lucky, CapeSpecial.Vainglory)` ->
  `Adv.enhanceItem(cape, EnhancementType.Lucky, { cSpecial: CapeSpecial.Vainglory })`
- `Adv.EnhanceItem(weapon, EnhancementType.Lucky, wSpecial: WeaponSpecial.Dauntless)` ->
  `Adv.enhanceItem(weapon, EnhancementType.Lucky, { wSpecial: WeaponSpecial.Dauntless })`
- `Adv.EnhanceEquipped(type, cape, helm, weapon, true)` ->
  `Adv.enhanceEquipped(type, { cSpecial: cape, hSpecial: helm, wSpecial: weapon, bypassDisableAutoEnhance: true })`
- `Adv.SmartEnhance(className, true)` -> `Adv.smartEnhance(className, { forceEnh: true })`
- `Adv.GearStore(true, EnhAfter: true)` -> `Adv.gearStore(true, { enhAfter: true })`
- `Adv.RankUpClass("Name", itemid: 36298)` -> `Adv.rankUpClass("Name", { itemId: 36298 })` (or
  `Adv.rankUpClass(36298)`); `RankUpClass(name, false)` -> `rankUpClass(name, { gearRestore: false })`.

## Merge scripts

A C# merge script (`Adv.StartBuyAllMerge` + `MultiOptions = { "Generic", "Select" }`) ports as in
`other/mergeshops/SynderesMerge.ts`:

- The class keeps `buyAllMerge(buyOnlyThis, buyMode)` (other scripts call it) and its
  `findIngredients` switch on `Adv.externalItem.name` / `Adv.externalQuant`.
- `Adv.StartBuyAllMerge(map, shop, findIngredients, buyOnlyThis, buyMode: buyMode)` becomes
  `Adv.startBuyAllMerge(map, shop, () => this.findIngredients(), buyOnlyThis, { buyMode })`.
- The `Select` list becomes a static list of `option.bool("<itemId>", ...)`, and the options are
  `defineScriptOptions(Adv.optionsStorage, [], true, { Generic: Adv.mergeOptions, Select: ... })`.
  VibeSkua's `MergeOptionStorage.cfg` reads as is.
- `Adv.MergeItemisinShopExceptions.AddRange(...)` becomes `Adv.mergeItemisinShopExceptions.push(...)`.

## FarmerJoe

`other/farmjoekits/0FarmerJoeDoAll.ts` and everything its `DoAll` reaches are ported: 128 files
under `chaos/`, `dailies/`, `darkon/`, `enhancement/`, `evil/`, `good/`, `hollowborn/`, `legion/`,
`nation/`, `other/`, `seasonal/`, `shadowsofwar/` and `story/`, each at its C# path. None of it
has run in game yet; it type-checks, and `bot/tests/runtime.rs` loads DoAll against a fake game.

- A file holds the methods DoAll reaches plus what its own `ScriptMain` reaches (its `main()`);
  its header lists the C# methods it leaves out. Port those from the C# file when another
  script needs them.
- Not ported: the classes the C# tree includes but DoAll never calls. `Dailies/0AllDailies`,
  `Dailies/MineCrafting`, `Enhancement/InventoryEnhancer`, `Tools/BankAllItems` (only in a
  commented-out line of DoAll), `Story/Friendship`, `Story/Hollowborn/CoreHollowbornStory`,
  `Story/ElegyofMadness(Darkon)/DarkonGarden`, `Story/TowerOfDoom`,
  `Story/QueenofMonsters/Extra/BrightOak`, `Nation/NationLoyaltyRewarded`,
  `Nation/MergeShops/NationMerge`, `Nation/Various/DragonBlade[mem]`, `Other/Classes/Necromancer`,
  `Other/Classes/REP-based/{EternalInversionist,StoneCrusher,DarkbloodStormKing}`,
  `Other/MergeShops/CelestialChampMerge`, `Other/Weapons/{DualChainSawKatanas,EnchantedVictoryBladeWeapons}`.
- Options another script reads (C# `Bot.Config.Get<T>(name)` in a content script, which DoAll
  calls with its own options loaded) go through `configOr(name, fallback)` from CoreBots: the
  fallback is C#'s `default(T)`. Enums whose members are item or quest IDs (the reward choices of
  JuggernautItems, TheLeeryContract, ArchfiendDeathLord, AssistingCragAndBamboozle, CoreSDKA's
  quest) stay numeric and are read with `configEnum(name, MyEnum, fallback)`; the other option
  enums are string enums.
- C# local functions are closures at the top of their method (`const findIngredients = ...`).
  Packet and flash-call listeners use `bot.events.on("extensionPacket", ...)` / `off`.
- Fixed C# bugs (each marked in the code): `CoreFarmerJoe.pets()` read the undeclared option
  `"Pets"` and never ran (it reads `"PetChoice"` now); `TheLeeryContract.questItems(reward)`
  ignored `reward` for the option (its `main()` passes the option now); `CoreLegion.dagePvP`
  took `canSoloBoss` from whether the CBO key is set (it takes the key's value now). Changed:
  `CoreLegion.joinLegion` never sells "Undead Warrior" (C# asks with a Yes/No box, which Turu
  scripts can't show); the "You will not resist!" watch in `CoreAOR.fightVoiceInTheSea` reads the
  parsed packet instead of the raw `pext` flash call.

A fuller guide is in `docs/turu/SCRIPTING.md`.
