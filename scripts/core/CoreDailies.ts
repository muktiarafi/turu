/*
name: CoreDailies
description: Port of Scripts/CoreDailies.cs: the daily, weekly and monthly quests (metals, class
    tokens, Nulgath dailies, boosts, friendships, ...).
tags: core
*/
/**
 * CoreDailies — TypeScript port of `Scripts/CoreDailies.cs`.
 *
 * Each script makes its own instance, as the C# scripts do:
 *
 * ```ts
 * import { CoreDailies } from "../core/CoreDailies";
 * const Daily = new CoreDailies();
 * Daily.mineCrafting(["Aluminum", "Barium", "Gold", "Iron", "Copper", "Silver", "Platinum"], 10, { toBank: true });
 * ```
 *
 * ## Porting notes
 * - C# named arguments become option objects: `CheckDailyv2(quest, false, true, items)` ->
 *   `Daily.checkDailyv2(quest, items, { any: false })`, `DailyRoutine(..., cell: "r2", pad: "Right")`
 *   -> `Daily.dailyRoutine(..., quant, { cell: "r2", pad: "Right" })`, `MineCrafting(metals, 10, true)`
 *   -> `Daily.mineCrafting(metals, 10, { toBank: true })`. `log` switches stay positional.
 */

import { bot } from "turu";
import type { InventoryItem } from "turu";
import { ClassType, Core, flag } from "./CoreBots";
import type { RewardItem } from "./CoreBots";

/** Where and how {@link CoreDailies.dailyRoutine} kills the monster. */
export interface DailyRoutineOptions {
    /** Whether the item is temporary (default true). */
    isTemp?: boolean;
    /** Cell where the monster is; without it the monster is hunted. */
    cell?: string | null;
    /** Pad where the monster is (default "Left"). */
    pad?: string;
    publicRoom?: boolean;
}

export class CoreDailies {
    // [Can Change] Default metals to be acquired by MineCrafting quest
    mineCraftingMetalsArray = ["Barium", "Copper", "Silver", "Platinum"];

    // [Can Change] Default metals to be acquired by Hard Core Metals quest
    hardCoreMetalsMetalsArray = ["Arsenic", "Chromium", "Rhodium"];

    // [Can Change] Skip daily if you own max stack of reward
    skipOnMaxStack = true;

    /**
     * Accepts the quest and kills the monster to complete, if no cell/pad is given will hunt for
     * the monster.
     * @param quest ID of the quest.
     * @param map Map where the monster is.
     * @param monster Name of the monster.
     * @param item Item to get.
     * @param quant Quantity of the item.
     */
    dailyRoutine(quest: number, map: string, monster: string, item: string, quant = 1, { isTemp = true, cell = null, pad = "Left", publicRoom = false }: DailyRoutineOptions = {}): void {
        if (bot.quests.isDailyComplete(quest)) return;
        Core.addDrop(item);
        Core.join(map);
        Core.ensureAccept(quest);
        if (cell !== null) Core.killMonster(map, cell, pad, monster, item, quant, { isTemp, log: true, publicRoom });
        else Core.huntMonster(map, monster, item, quant, { isTemp, log: true, publicRoom });
        Core.ensureComplete(quest);
        bot.wait.forPickup("*");
    }

    /**
     * Checks whether a daily can be done: it isn't completed today, and (with `items`) you don't
     * own the maximum stack of its items. Unbanks the items you don't have in the inventory, and
     * adds them, the quest's requirements and (for the LOO and Doom Spin dailies) the related
     * rewards to the drop list.
     * @param quest The ID of the quest to check.
     * @param items The item names to check and add to the drop list.
     * @param options `any`: stop as soon as any item is at its maximum stack (default true);
     *   otherwise only when all are. `shouldUnBank`: unbank the items you don't have in the
     *   inventory (default true).
     * @returns True (don't use !) when the daily can be done; false when it is already done or
     *   the items are at max stack.
     */
    checkDailyv2(quest: number, items: readonly string[] = [], { any = true, shouldUnBank = true }: { any?: boolean; shouldUnBank?: boolean } = {}): boolean {
        const questData = Core.initializeWithRetries(() => Core.ensureLoad(quest));
        if (questData === null) {
            Core.logger(`Failed to load quest ${quest}`);
            return false;
        }
        // Check if the daily quest is complete
        if (bot.quests.isDailyComplete(quest)) {
            Core.logger(`Daily/Weekly/Monthly "${questData.name} [${questData.id}]" is not available right now`);
            return false;
        }

        // Handle the item checks and drop additions
        if (items.length === 0) return true;

        const invBank = [...bot.inventory.items, ...bot.bank.items].filter((x) => items.includes(x.name));
        let maxCount = 0;

        for (const item of items) {
            const owned = invBank.find((x) => x.name === item);
            if (owned && owned.quantity === owned.maxStack) {
                if (any) {
                    Core.logger(`You already own the maximum amount of: ${item}`);
                    return false;
                }
                maxCount++;
            }

            // Unbanking logic if shouldUnBank is true
            if (shouldUnBank && !owned) Core.unbank(item);
        }

        if (!any && maxCount === items.length) {
            Core.logger(`You already own the maximum amount of: ${items.join(",")}`);
            return false;
        }

        bot.drops.add(...items);

        // Handle LOO dailies for quests within the specified range
        if (quest >= 7156 && quest < 7166 && !Core.checkInventory(50741, 1, { toInv: false }) && !Core.isCompletedBefore(quest)) {
            for (let questId = 7156; questId < 7166; questId++)
                if (!Core.isCompletedBefore(questId) && !bot.quests.isDailyComplete(questId)) bot.drops.add(...Core.questRewards(questId));
        }

        // Handle Doom Spins for quests within the specified range
        if (quest >= 3075 && quest < 3078) {
            const owned = new Set([...bot.inventory.items, ...bot.bank.items].map((i) => i.name));
            for (let questId = 3075; questId < 3078; questId++)
                bot.drops.add(
                    ...Core.ensureLoad(questId)
                        .rewards.map((x) => x.name)
                        .filter((x) => !owned.has(x)),
                );
        }

        // Add the required items for the quest
        Core.addDrop(...Core.ensureLoad(quest).requirements.map((x) => x.name));

        return true;
    }

    /**
     * Does the Mine Crafting quest for 2 Barium, Copper, Silver and Platinum by default.
     * @param metals Metals you want to be collected.
     * @param quant Quantity you want of the metals.
     */
    mineCrafting(metals: string[] | null = null, quant = 2, { toBank = false }: { toBank?: boolean } = {}): void {
        metals ??= this.mineCraftingMetalsArray;
        Core.logger(`Daily: Mine Crafting (${metals.join("/")})`);
        // Check if all metals are in inventory
        if (metals.every((metal) => Core.checkInventory(metal, quant, { toInv: false }))) {
            Core.logger(`All metals were found with the needed quantity (${quant}).`);

            if (toBank) Core.toBank(...metals);

            return;
        }

        if (!this.checkDailyv2(2091, metals, { any: false })) return;

        Core.ensureAccept(2091);
        Core.equipClass(ClassType.Farm);
        Core.huntMonster("stalagbite", "Balboa", "Axe of the Prospector", 1, { isTemp: false });
        Core.huntMonster("stalagbite", "Balboa", "Raw Ore", 30);

        Core.jump("r2");

        for (const metal of metals) {
            if (!Core.checkInventory(metal, quant, { toInv: false })) {
                Core.addDrop(metal);
                Core.ensureComplete(2091, MineCraftingMetalsEnum[metal as keyof typeof MineCraftingMetalsEnum]);
                bot.wait.forPickup(metal);
            }
            if (toBank && Core.checkInventory(metal, 1, { toInv: false })) Core.toBank(...metals);
        }

        if (bot.quests.isInProgress(2091)) Core.logger(`All desired metals were found with the needed quantity (${quant}), quest not completed`);

        Core.sleep();
    }

    /**
     * Does the Hard Core Metals quest for 1 Arsenic, Chromium and Rhodium by default (members).
     * @param metals Metals you want to be collected.
     * @param quant Quantity you want of the metals.
     */
    hardCoreMetals(metals: string[] | null = null, quant = 1, { toBank = false }: { toBank?: boolean } = {}): void {
        if (!Core.isMember || !Core.isCompletedBefore(2090)) return;

        metals ??= this.hardCoreMetalsMetalsArray;

        Core.logger(`Daily: Hard Core Metals (${metals.join("/")})`);
        if (Core.checkInventory(metals, quant, { toInv: false })) {
            Core.logger(`All "base" metals were found with the needed quantity (${quant}). Skipped`);
            if (toBank) Core.toBank(...metals);
        }
        if (!this.checkDailyv2(2098, metals, { any: false })) return;

        Core.ensureAccept(2098);
        Core.equipClass(ClassType.Farm);
        Core.huntMonster("stalagbite", "Balboa", "Axe of the Prospector", 1, { isTemp: false });
        Core.huntMonster("stalagbite", "Balboa", "Raw Ore", 30);

        Core.jump("r2");

        for (const metal of metals) {
            if (!Core.checkInventory(metal, quant, { toInv: false })) {
                Core.addDrop(metal);
                Core.ensureComplete(2098, HardCoreMetalsEnum[metal as keyof typeof HardCoreMetalsEnum]);
                bot.wait.forPickup(metal);
            }
            if (toBank && Core.checkInventory(metal, 1, { toInv: false })) Core.toBank(...metals);
        }
        if (bot.quests.isInProgress(2098)) Core.logger(`All desired metals were found with the needed quantity (${quant}), quest not completed`);
    }

    fungiforaFunGuy(): void {
        if (!Core.isMember) return;
        Core.logger("Daily: Fungi for a Fun Guy (BrightOak Reputation)");
        if (bot.reputation.getRank("Brightoak") === 10) {
            Core.logger("BrightOak is already rank 10. Skipped");
            return;
        }
        if (!this.checkDailyv2(4465)) return;

        Core.equipClass(ClassType.Farm);
        Core.ensureAccept(4465);
        Core.huntMonster("brightoak", "Grove Spore", "Colony Spore");
        Core.huntMonster("brightoak", "Grove Spore", "Intact Spore");
        Core.ensureComplete(4465);
    }

    beastMasterChallenge(): void {
        if (!Core.isMember) return;
        Core.logger("Daily: Beast Master Class");
        if (bot.reputation.getRank("BeastMaster") === 10) {
            Core.logger("BeastMaster is already rank 10. Skipped");
            return;
        }
        if (!this.checkDailyv2(3759)) return;

        this.dailyRoutine(3759, "swordhavenbridge", "Purple Slime", "Purple Slime", 10);
    }

    cyserosSuperHammer(): void {
        Core.logger("Daily: Cysero's Super Hammer");
        if (Core.checkInventory("Cysero's SUPER Hammer", 1, { toInv: false })) {
            Core.logger("Skipped");
            return;
        }
        if (!Core.checkInventory("Cysero's SUPER Hammer", 1, { toInv: false }) && Core.checkInventory("C-Hammer Token", 90)) {
            Core.buyItem("deadmoor", 500, "Cysero's SUPER Hammer");
            return;
        }
        if (!Core.checkInventory("Mad Weaponsmith")) {
            Core.logger("You don't own Mad Weaponsmith yet. Skipped");
            return;
        }
        if (!this.checkDailyv2(4310, ["C-Hammer Token"]) && !Core.isMember) return;
        if (!this.checkDailyv2(4311, ["C-Hammer Token"]) && Core.isMember) return;
        Core.equipClass(ClassType.Solo);
        this.dailyRoutine(4310, "deadmoor", "Geist", "Geist's Chain Link");
        if (Core.isMember) this.dailyRoutine(4311, "deadmoor", "Geist", "Geist's Pocket Lint");
        Core.toBank("C-Hammer Token", "Mad Weaponsmith", "Cysero's SUPER Hammer");
    }

    madWeaponSmith(): void {
        Core.logger("Daily: Mad Weaponsmith");
        if (Core.checkInventory("Mad Weaponsmith", 1, { toInv: false })) {
            Core.logger("Skipped");
            return;
        }
        if (!Core.checkInventory("Mad Weaponsmith", 1, { toInv: false }) && Core.checkInventory("C-Armor Token", 90, { toInv: false })) {
            Core.unbank("C-Armor Token");
            Core.buyItem("deadmoor", 500, "Mad Weaponsmith");
            return;
        }
        if (!this.checkDailyv2(4308, ["C-Armor Token"]) && !Core.isMember) return;
        if (!this.checkDailyv2(4309, ["C-Armor Token"]) && Core.isMember) return;
        Core.equipClass(ClassType.Solo);
        this.dailyRoutine(4308, "deadmoor", "Nightmare", "Nightmare Fire");
        if (Core.isMember) this.dailyRoutine(4309, "deadmoor", "Nightmare", "Unlucky Horseshoe");
        Core.toBank("C-Armor Token", "Mad Weaponsmith");
    }

    brightKnightArmor({ checkArmor = true }: { checkArmor?: boolean } = {}): void {
        Core.logger("Daily: Bright Knight Armor");
        if (checkArmor && Core.checkInventory("Bright Knight", 1, { toInv: false })) {
            Core.logger("You already own the Bright Knight Armor, Skipped");
            return;
        }

        if (Core.checkInventory(["Seal of Light", "Seal of Darkness"], 50)) {
            Core.buyItem("alteonbattle", 574, "Bright Knight");
            return;
        }
        if (this.checkDailyv2(3826, ["Seal of Light"])) {
            Core.equipClass(ClassType.Solo);
            this.dailyRoutine(3826, "alteonbattle", "ULTRA Alteon", "Alteon Defeated");
        }
        if (this.checkDailyv2(3825, ["Seal of Darkness"])) {
            Core.equipClass(ClassType.Solo);
            this.dailyRoutine(3825, "sepulchurebattle", "ULTRA Sepulchure", "Sepulchure Defeated");
        }
        Core.jumpWait();
    }

    collectorClass(): void {
        Core.logger("Daily: The Collector Class");
        //30229 is the ac, 30250 is the non-ac, 30253 is the merge item
        if (Core.checkInventory([30229, 30250, 30253], 1, { any: true, toInv: false })) {
            Core.logger("You already own The Collector. Skipped");
            return;
        }
        if (this.checkDailyv2(1316, ["Token of Collection"])) {
            Core.equipClass(ClassType.Farm);
            Core.farmingLogger("Token of Collection", 90);
            this.dailyRoutine(1316, "terrarium", "Carnivorous Cricket", "This Might Be A Token", 2, { isTemp: false, cell: "r2", pad: "Right" });
        }
        if (Core.isMember) {
            Core.farmingLogger("Token of Collection", 90);
            if (this.checkDailyv2(1331, ["Token of Collection"]))
                this.dailyRoutine(1331, "terrarium", "Killer Cricket", "This Is Definitely A Token", 2, { isTemp: false, cell: "r2", pad: "Right" });
            if (this.checkDailyv2(1332, ["Token of Collection"]))
                this.dailyRoutine(1332, "terrarium", "Killer Cricket", "This Could Be A Token", 2, { isTemp: false, cell: "r2", pad: "Right" });
        }
        if (Core.checkInventory("Token of Collection", 90)) Core.buyItem("Collection", 324, 30250, 1, 3015);
    }

    cryomancer(): void {
        Core.logger("Daily: Cryomancer Class");
        if (Core.checkInventory("Cryomancer", 1, { toInv: false })) {
            Core.logger("You already own Cryomancer, Skipped");
            return;
        }

        if (Core.isMember && this.checkDailyv2(3965, ["Glacera Ice Token"])) {
            Core.equipClass(ClassType.Farm);
            this.dailyRoutine(3965, "frozentower", "Frost Invader", "Dark Ice");
            Core.farmingLogger("Glacera Ice Token", 84, "Glacera Ice Token");
            Core.toBank("Glacera Ice Token");
        }

        if (this.checkDailyv2(3966, ["Glacera Ice Token"])) {
            Core.equipClass(ClassType.Farm);
            this.dailyRoutine(3966, "frozentower", "Frost Invader", "Dark Ice");
            Core.farmingLogger("Glacera Ice Token", 84, "Glacera Ice Token");
            Core.toBank("Glacera Ice Token");
        }
        if (Core.checkInventory("Glacera Ice Token", 84)) Core.buyItem("frozenruins", 1056, 27525, 1, 2603, 1);
        Core.toBank("Glacera Ice Token");
    }

    // This is no longer a dainy.. i just dont fee like moving it.
    pyromancer(): void {
        Core.logger("Daily: Pyromancer Class");
        if (Core.checkInventory(12811) || Core.checkInventory(12812)) {
            Core.logger("You already own Pryomancer, Skipped");
            return;
        }
        Core.addDrop("Shurpu Blaze Token");
        Core.equipClass(ClassType.Farm);
        Core.killMonster("xancave", "r9", "Down", "*", "Shurpu Blaze Token", 84, { isTemp: false });

        Core.buyItem("xancave", 447, 12812, 1, 1278);
        Core.sellItem("Shurpu Blaze Token", 1, { all: true });
    }

    shadowScytheClass(): void {
        Core.logger("Daily: ShadowScythe General Class");
        if (Core.checkInventory("ShadowScythe General", 1, { toInv: false })) {
            Core.logger("Skipped");
            return;
        }
        if (!Core.checkInventory("ShadowScythe General") && Core.checkInventory("Shadow Shield", 100)) {
            Core.buyItem("shadowfall", 1644, "ShadowScythe General");
            return;
        }
        if (!this.checkDailyv2(3828, ["Shadow Shield"]) && Core.isMember && !this.checkDailyv2(3827, ["Shadow Shield"])) return;
        this.dailyRoutine(3828, "lightguardwar", "Citadel Crusader", "Broken Blade");
        if (Core.isMember) {
            this.dailyRoutine(3827, "lightguardwar", "Citadel Crusader", "Broken Blade");
            if (Core.checkInventory("Shadow Shield", 100)) Core.buyItem("shadowfall", 1644, "ShadowScythe General");
        }
        Core.jump("Cut1", "Left");
        Core.toBank("Shadow Shield");
    }

    grumbleGrumble(): void {
        if (!Core.checkInventory(4845)) return;
        Core.logger("Daily: Grumble Grumble (Blood Gem of the Archfiend)");
        if (!this.checkDailyv2(592, ["Diamond of Nulgath", "Blood Gem of the Archfiend"], { any: false, shouldUnBank: false })) return;
        Core.chainComplete(592);
        Core.toBank("Diamond of Nulgath", "Blood Gem of the Archfiend");
    }

    tenacityChallenge(item: string | null = null): void {
        if (!Core.checkInventory("Nulgath Challenge Pet") || !this.checkDailyv2(3319)) {
            Core.logger(!this.checkDailyv2(3319) ? "Daily Not Avaiable" : 'You Don\'t Have "Nulgath Challenge Pet". Pet is required for doing the quests.');
            return;
        }
        Core.logger("Daily: Tenacity Challenge");
        Core.equipClass(ClassType.Farm);
        Core.addDrop(...Core.questRewards(3319));
        Core.ensureAccept(3319);
        Core.huntMonster("deathpits", "Ghastly Darkblood", "Dark Runes", 6);
        Core.huntMonster("evilwardage", "Bloodfiend", "Blood Runes", 7);
        if (item !== null) Core.ensureCompleteChoose(3319, [item]);
        if (!Core.checkInventory("Blood Gem of the Archfiend", 100)) Core.ensureComplete(3319, 22332);
        else {
            for (const reward of Core.ensureLoad(3319).rewards) {
                if (Core.checkInventory(reward.id, reward.maxStack)) continue;
                Core.ensureComplete(3319, reward.id);
                break;
            }
        }
        Core.toBank("Tained Gem", "Dark Crystal Shard", "Blood Gem of the Archfiend");
    }

    eldersBlood(): void {
        if (Core.checkInventory("Elders' Blood", 20)) return; //AE keeps updating this shit, Laste update: 1/30/23, https://www.aq.com/gamedesignnotes/aqw-30jan23-mondayupdates-9076
        if (!this.checkDailyv2(802, ["Elders' Blood"])) return;
        Core.addDrop("Elders' Blood");
        Core.logger("Daily: Elders' Blood");
        Core.equipClass(ClassType.Farm);
        this.dailyRoutine(802, "arcangrove", "Gorillaphant", "Slain Gorillaphant", 50, { cell: "LeftBack", pad: "Left" });
        bot.wait.forPickup("Elders' Blood");
    }

    sparrowsBlood(quant = 3): void {
        if (!this.checkDailyv2(803, ["Sparrow's Blood"], { any: false })) return;

        if (Core.checkInventory("Sparrow's Blood", 3)) {
            Core.logger("You've maxed out Sparrow's Blood [3]");
            return;
        }

        if (Core.checkInventory("Sparrow's Blood", quant)) {
            Core.logger(`You already have enough Sparrow's Blood (${bot.inventory.getQuantity("Sparrow's Blood")}/${quant}). Skipped`);
            return;
        }

        Core.logger("Daily: Sparrow's Blood");

        Core.addDrop(5584);
        Core.addDrop("Sparrow's Blood");
        Core.equipClass(ClassType.Farm);
        Core.ensureAccept(803);
        Core.killMonster("arcangrove", "LeftBack", "Left", "*", "Blood Lily", 30);
        Core.killMonster("arcangrove", "RightBack", "Left", "*", "Snapdrake", 17);
        Core.killMonster("arcangrove", "Back", "Left", "*", "DOOM Dirt", 12);
        Core.ensureComplete(803);
        bot.wait.forDrop("Sparrow's Blood");
        bot.wait.forPickup("Sparrow's Blood");
    }

    pearlOfNulgath(): void {
        Core.logger("Daily: Pearl of Nulgath");
        if (!this.checkDailyv2(10047, ["Pearl of Nulgath"]) || Core.checkInventory("Pearl of Nulgath", 20) || !Core.checkInventory("Malakai's Katana Pet")) return;
        Core.unbank("Unidentified 10");
        if (!Core.checkInventory("Unidentified 10", 25)) {
            Core.logger(`You don't have enough Unidentified 10 (${bot.inventory.getQuantity("Unidentified 10")}/25). Skipped`);
            return;
        }
        Core.chainComplete(10047);
        bot.wait.forPickup("Pearl of Nulgath");
        Core.toBank("Unidentified 10");
    }

    shadowShroud(): void {
        Core.logger("Daily: Shadow Shroud");
        if (!this.checkDailyv2(486, ["Shadow Shroud"]) || Core.checkInventory("Shadow Shroud", 15, { toInv: false })) return;
        this.dailyRoutine(486, "bludrut2", "Shadow Creeper", "Shadow Canvas", 5, { cell: "Enter", pad: "Down" });
        Core.toBank("Shadow Shroud");
    }

    dagesScrollFragment({ toBank = false }: { toBank?: boolean } = {}): void {
        Core.logger("Daily: Dage's Scroll Fragment");
        if (!this.checkDailyv2(3596, ["Dage's Scroll Fragment"]) || Core.checkInventory("Dage's Scroll Fragment", 13, { toInv: false })) return;

        this.dailyRoutine(3596, "mountdoomskull", "*", "Chaos Power Increased", 6, { cell: "b1", pad: "Left" });

        bot.wait.forPickup("Dage's Scroll Fragment");
        if (toBank) Core.toBank("Dage's Scroll Fragment");
    }

    cryptoToken(): void {
        Core.logger("Daily: Crypto Token (/curio)");
        if (!this.checkDailyv2(6187, ["Crypto Token"]) || Core.checkInventory("Crypto Token", 300, { toInv: false })) return;
        Core.equipClass(ClassType.Farm);
        this.dailyRoutine(6187, "boxes", "Sneevil", "Metal Ore", 1, { cell: "Closet", pad: "Center" });
        Core.toBank("Crypto Token");
    }

    monthlyTreasureChestKeys(log = true): void {
        if (!Core.isMember || !Core.checkInventory("Treasure Chest")) return;

        Core.logger("Montly: Treasure Chest Keys");
        // C#'s dangling `else` belongs to `if (log)`: without logging, it turns 1239 in when the
        // monthly isn't available. Kept as C# does it.
        if (!this.checkDailyv2(1239)) {
            if (log) {
                const now = new Date();
                const next = new Date(now.getFullYear(), now.getMonth() + 1, 1);
                Core.logger(`Next keys are available on ${next.toDateString()}`);
            } else Core.chainComplete(1239);
        }

        const questData = Core.initializeWithRetries(() => Core.ensureLoad(1238));
        if (questData === null) {
            Core.logger(`Failed to load quest ${1238}`);
            return;
        }
        if (Core.checkInventory(questData.rewards.map((x) => x.name), 1, { toInv: false })) return;

        const preQuestInv = bot.inventory.items.map((x) => x.name);

        if (Core.checkInventory("Magic Treasure Chest Key") && Core.checkInventory("Treasure Chest", 1)) bot.drops.add(...questData.rewards.map((x) => x.name));

        while (!bot.shouldExit && Core.checkInventory("Magic Treasure Chest Key") && Core.checkInventory("Treasure Chest", 1)) {
            Core.chainComplete(1238);
            bot.wait.forPickup("*");
        }

        Core.toBank(...bot.inventory.items.map((x) => x.name).filter((x) => !preQuestInv.includes(x)));
    }

    wheelofDoom(log = true): void {
        // Fetch Gear of Doom from Inventory and Bank
        const invBank = [...bot.inventory.items, ...bot.bank.items];
        const gearOfDoom = invBank.find((x) => x?.name === "Gear of Doom");
        const treasurePotion = invBank.find((x) => x?.name === "Treasure Potion");

        // Log Gear of Doom progress
        if (log)
            Core.logger(
                "Wheel of Doom\n" +
                    `Gear of Doom: ${gearOfDoom?.quantity ?? 0}/3 | Treasure Potion: ${treasurePotion?.quantity ?? 0}\n` +
                    (Core.isMember
                        ? `Daily: ${this.checkDailyv2(3075) ? "✅" : "❌"} | Weekly: ${this.checkDailyv2(3076) ? "✅" : "❌"}`
                        : `Weekly: ${this.checkDailyv2(3076) ? "✅" : "❌"}`),
            );

        // Snapshot inventory before completing quests
        const preQuestInv = bot.inventory.items.filter((x) => x && x.id > 0).map((x) => x.id);

        // Complete Daily Quest (3075) if eligible
        if (bot.player.isMember && this.checkDailyv2(3075)) Core.chainComplete(3075);

        // Complete Weekly Quest (3076) if eligible
        if (Core.checkInventory("Gear of Doom", 3) && this.checkDailyv2(3076)) Core.chainComplete(3076);

        bot.wait.forPickup("*");

        // Check for new items added to the inventory
        const newItems: InventoryItem[] = bot.inventory.items.filter((x) => x && x.id > 0 && !preQuestInv.includes(x.id));

        if (newItems.length <= 0) return;

        // Log and move new items to bank
        Core.logger("New items: " + newItems.map((x) => x.name).join(" | "));
        Core.toBank(...newItems.map((x) => x.id));
    }

    nSoDDaily({ ignoreSwords = true }: { ignoreSwords?: boolean } = {}): void {
        if (!ignoreSwords && Core.checkInventory(["Necrotic Sword of Doom", "Dual Necrotic Swords of Doom"], 1, { any: true }) && Core.checkInventory("Void Aura", 7500)) return;

        Core.logger("Daily: Void Auras");
        Core.equipClass(ClassType.Solo);
        Core.addDrop("Void Aura", "(Necro) Scroll of Dark Arts");

        // Glimpse Into the Dark[Mem] - 8652
        if (Core.isMember) {
            if (this.checkDailyv2(8652)) {
                Core.ensureAccept(8652);
                if (Core.isCompletedBefore(3119)) {
                    Core.addDrop("Kraken Doubloon");
                    Core.registerQuests(3119);
                    while (!bot.shouldExit && !Core.checkInventory("Kraken Doubloon", 13)) Core.huntMonster("chaoskraken", "Chaos Kraken", "Kraken Keelhauled");
                    Core.cancelRegisteredQuests();
                } else Core.huntMonster("chaoskraken", "Chaos Kraken", "Kraken Doubloon", 13, { isTemp: false, publicRoom: true });
                Core.killMonster("ancienttrigoras", "r2a", "Left", "Ancient Trigoras", "Ancient Trigora's Horns", 3, { isTemp: false });
                Core.equip("Yami no Ronin");
                Core.killMonster("gravechallenge", "r19", "Left", "Graveclaw the Destroyer", "Graveclaw's Broken Axe", 1, { isTemp: false });
                Core.ensureComplete(8652);
                bot.wait.forPickup("Void Aura");
            }
        }
        // The Encroaching Shadows - 8653
        if (this.checkDailyv2(8653)) {
            Core.ensureAccept(8653);
            Core.equipClass(ClassType.Dodge);
            Core.huntMonster("icewing", "Warlord Icewing", "Glacial Pinion", 1, { isTemp: false, publicRoom: true });
            Core.equipClass(ClassType.Solo);
            Core.huntMonster("hydrachallenge", "Hydra Head 90", "Hydra Eyeball", 3, { isTemp: false });
            Core.equipClass(ClassType.Dodge);
            Core.huntMonster("voidflibbi", "Flibbitiestgibbet", "Flibbitigiblets", 1, { isTemp: false, publicRoom: true });
            Core.ensureComplete(8653);
            bot.wait.forPickup("Void Aura");
        }
    }

    freeDailyBoost(reward: DailyBoostRewards = DailyBoostRewards.LowestQuantOwned): void {
        if (!Core.isMember || !this.checkDailyv2(4069)) return;

        Core.logger("Daily: Free Boost");

        const boostQuest = Core.ensureLoad(4069);

        // Concatenate inventory and bank items
        const allItems = [...bot.inventory.items, ...bot.bank.items];

        // The valid reward items and their owned quantities, skipping XP boost if level 100
        const rewardQuantities: [RewardItem, number][] = boostQuest.rewards
            .filter((r) => r.id !== 27552 || bot.player.level < 100)
            .map((r) => [r, allItems.find((item) => item.id === r.id)?.quantity ?? 0]);
        const lowest = () => [...rewardQuantities].sort((a, b) => a[1] - b[1])[0];

        // If the selected reward is max stacked, log and switch to the lowest quantity reward
        if (reward !== DailyBoostRewards.LowestQuantOwned) {
            const selected = rewardQuantities.find(([r]) => r.id === reward);
            const max = Math.max(...rewardQuantities.map(([, q]) => q));

            if (selected && selected[1] === max) {
                Core.logger(
                    `Selected reward ${DailyBoostRewards[reward]} (ID: ${selected[0].id}) is max stacked with quantity ${selected[1]}. Switching to the lowest quantity reward.`,
                );
                reward = DailyBoostRewards.LowestQuantOwned;
            }
        }

        // Select reward based on the input or default to the lowest owned
        const selectedReward = reward === DailyBoostRewards.LowestQuantOwned ? lowest() : (rewardQuantities.find(([r]) => r.id === reward) ?? lowest());

        if (!selectedReward) return;
        const [item, quantity] = selectedReward;

        Core.logger(`Selected reward: ${item.name} (ID: ${item.id}) with quantity ${quantity}`);

        Core.addDrop(item.id); // Ensure it's added to drop list
        Core.chainComplete(4069, item.id); // Complete the quest with the chosen reward
        bot.wait.forDrop(item.id); // Wait for the item to be dropped
        bot.wait.forPickup(item.id); // Wait for the item to be picked up
        Core.toBank(item.id); // Bank the reward for inventory space
    }

    powerGem(): void {
        if (!flag(bot.flash.callGameFunction("world.myAvatar.isEmailVerified"))) {
            Core.logger("Account doesn't have a verified email.");
            return;
        }

        Core.logger("Weekly: Power Gems");
        if (Core.checkInventory("Power Gem", 1000, { toInv: false }) || !this.checkDailyv2(9109)) {
            Core.logger("You have the maximum amount of Power Gems");
            return;
        }

        // Weekly Power Gem Quest
        Core.ensureAccept(9109);
        Core.huntMonster("boxes", "Sneevil", "News Scroll", 1, { log: false });
        Core.ensureComplete(9109);
        bot.wait.forPickup("Power Gem");
        Core.toBank("Power Gem");
    }

    goldenInquisitor(): void {
        Core.logger("Daily: Golden Inquisitor of Shadowfall");
        const rewards = Core.questRewards(491);
        if (Core.checkInventory(rewards, 1, { toInv: false }) || !this.checkDailyv2(491)) return;

        Core.ensureAccept(491);
        bot.drops.add(...Core.ensureLoad(491).rewards.map((x) => x.name));
        Core.equipClass(ClassType.Farm);
        Core.huntMonster("citadel", "Inquisitor Guard", "Inquisitor Contract", 7);
        Core.ensureComplete(491);
        bot.wait.forPickup("*");
        Core.toBank(...rewards);
    }

    designNotes(): void {
        Core.logger("Weekly: Read the Design Notes!");

        if (bot.reputation.getRank("Loremaster") !== 10 && this.checkDailyv2(1213)) Core.chainComplete(1213);
    }

    moglinPets(): void {
        Core.logger("Daily: Moglin Pets");
        const pets = ["Twig Pet", "Twilly Pet", "Zorbak Pet"];
        if (Core.checkInventory(pets, 1, { toInv: false })) return;

        for (const pet of pets) {
            if (Core.checkInventory(pet, 1, { toInv: false })) continue;

            let dailyDone = !this.checkDailyv2(4159);

            if (!Core.checkInventory("Moglin MEAL", 30) && !dailyDone) {
                Core.logger("Dedicating daily to " + pet);
                Core.addDrop("Moglin MEAL");
                Core.ensureAccept(4159);
                Core.huntMonster("nexus", "Frogzard", "Frogzard Meat", 3);
                Core.ensureComplete(4159);
                bot.wait.forPickup("Moglin MEAL");
                dailyDone = true;
            }

            if (Core.checkInventory("Moglin MEAL", 30)) Core.buyItem("ariapet", 1081, pet);

            Core.toBank("Moglin MEAL");

            if (dailyDone) break;
        }
    }

    // C# templeshrineDailies: commented out there, not ported.

    breakIntotheHoard({ keepReward = false, bank = false }: { keepReward?: boolean; bank?: boolean } = {}): void {
        if (!this.checkDailyv2(3898)) return;

        if (!Core.hasAchievement(30, "ip6")) {
            Core.logger('"Break Into the Hoard" daily quest requires you to purchase BoneBreaker Adventure Pack to be able to complete it.');
            return;
        }

        if (!Core.isCompletedBefore(5981)) {
            Core.logger("Requires storyline completetion, run the standalone daily (if you have the required items.)...)");
            return;
        }

        //Buying BoneBreaker Fortress Map
        Core.buyItem("battleon", 1046, 27222);

        const questReward = [...Core.ensureLoad(3898).rewards];

        if (keepReward) Core.addDrop("BoneBreaker Medallion");

        //Break Into the Hoard
        Core.ensureAccept(3898);
        Core.huntMonster("bonebreak", "Undead Berserker", "Warrior Defeated", 5, { log: false });
        Core.ensureComplete(3898);
        bot.wait.forPickup("BoneBreaker Medallion");

        if (bank) for (const item of questReward) if (Core.checkInventory(item.id, 1, { toInv: false })) Core.toBank(item.id);
    }

    nCSGem(quant = 15): void {
        Core.logger("Daily: NCS Gem");
        if (Core.checkInventory("NCS Gem", quant)) return;
        if (!this.checkDailyv2(9642, ["NCS Gem"])) return;

        Core.addDrop("NCS Gem");
        Core.ensureAccept(9642);
        Core.equipClass(ClassType.Solo);
        Core.huntMonster("shadowrealm", "Shadow Lord", "Lovely Favor", 1, { log: false });
        Core.equipClass(ClassType.Farm);
        Core.huntMonster("shadowrealm", "Shadow Makai", "Lovely Request", 100, { log: false });
        Core.ensureComplete(9642);
    }

    eldenRuby(quant = 25): void {
        Core.logger("Daily: Elden Ruby");
        if (Core.checkInventory("Elden Ruby", quant)) return;
        if (!this.checkDailyv2(9896, ["Elden Ruby"])) return;
        if (!Core.checkInventory("Compass Rose Skull")) {
            Core.logger("Getting the Compass Rose Skull.");
            Core.huntMonsterQuest(9894, ["dracocon", "Treasure Pile", ClassType.Farm], ["battleundere", "Treasure Pile", ClassType.Farm], ["greed", "Treasure Pile", ClassType.Farm]);
        }
        if (!Core.checkInventory("Obsessor Captain")) {
            Core.logger("Getting the Obsessor Captain.");
            Core.huntMonsterQuest(9895, ["shadowrealm", "Shadow Lord", ClassType.Solo]);
        }

        Core.unbank("Compass Rose Skull", "Obsessor Captain");
        Core.addDrop("Elden Ruby");

        Core.huntMonsterQuest(9896, ["trygve", "Vindicator Recruit", ClassType.Farm], ["greed", "Cursed Treasure", ClassType.Solo]);

        Core.toBank("Compass Rose Skull", "Obsessor Captain");
    }

    enchantedDarkBlood(quant = 10): void {
        Core.logger("Daily: Enchanted Dark Blood");
        if (Core.checkInventory("Enchanted Dark Blood", quant)) return;
        if (!this.checkDailyv2(2677, ["Enchanted Dark Blood"])) return;
        Core.equipClass(ClassType.Farm);
        Core.huntMonsterQuest(2677, "falguard", "Chaonslaught Caster");
    }

    //#region Friendship

    /**
     * The friendship dailies of Battleodium and Greyguard: talks to and gifts each NPC you can,
     * farming the gift first. The game's answers come as `friendship*` packets.
     */
    friendships(): void {
        let waitForPacket = false;
        let friends: FriendshipInfo[] = [];
        let friendshipInfo: string | null = null;

        const friendshipPacketReader = (packet: ExtensionPacket) => {
            const type = packet?.params?.type;
            const data = packet?.params?.dataObj;
            if (type === "json" && data) {
                switch (String(data.cmd)) {
                    case "friendshipInfo":
                    case "friendshipTalk":
                    case "friendshipChoice":
                        waitForPacket = true;
                        break;

                    case "friendshipStats":
                        friendshipInfo = JSON.stringify(data.friendships);
                        break;
                }
            }
        };

        const refreshFriendshipData = (retries = 2): boolean => {
            for (let i = 0; i <= retries; i++) {
                friendshipInfo = null;
                bot.send.packet(`%xt%zm%friendshipStats%${bot.map.roomId}%`);
                bot.wait.forTrue(() => friendshipInfo !== null, 30);

                if (friendshipInfo !== null) {
                    try {
                        const parsed: unknown = JSON.parse(friendshipInfo);
                        friends = Array.isArray(parsed) ? parsed.map(toFriendshipInfo) : [];
                        if (friends.length > 0) return true;
                    } catch (error) {
                        Core.logger(`Failed to parse friendship data: ${error instanceof Error ? error.message : String(error)}`);
                    }
                }

                if (i < retries) Core.logger(`Retrying friendship data refresh... (${i + 1}/${retries})`);
            }

            Core.logger("Something went wrong, friendshipInfo is null");
            friends = [];
            return false;
        };

        /** Logs `message` with the hearts gained since `info` (C# updates `info` by ref; here it's returned). */
        const informLogger = (message: string, info: FriendshipInfo): FriendshipInfo => {
            const oldNum = info.displayHearts;
            const refreshed = refreshFriendshipData();

            if (!refreshed || friends.length === 0) {
                Core.logger(message + " (Unable to verify hearts gained)");
                return info;
            }

            const updatedInfo = friends.find((f) => f.npc === info.npc);
            if (!updatedInfo) {
                Core.logger(message + " (NPC not found in refresh)");
                return info;
            }

            const addNum = updatedInfo.displayHearts - oldNum;
            Core.logger(message + ` You gained ${addNum} heart${addNum > 1 ? "s" : ""}`);
            return updatedInfo;
        };

        const sendWaitedPacket = (packet: string) => {
            waitForPacket = false;
            bot.send.packet(packet);
            if (!bot.wait.forTrue(() => waitForPacket, 30)) {
                const parts = packet.split("%");
                Core.logger(`Warning: Packet timeout - ${parts.length > 3 ? parts[3] : "unknown"}`);
            }
        };

        const giftName = (gift: FrGift) => FrGift[gift].replace(/_/g, " ");

        const handleFriendship = (npc: string, ...gifts: FrGift[]) => {
            let friend = friends.find((f) => f.npc.toLowerCase() === npc.toLowerCase());
            if (!friend) {
                Core.logger(`NPC "${npc}" not found. Check for typos`);
                return;
            }

            const canDoDaily = friend.canGift || (friend.canTalk && friend.npc !== "Linus");
            if (!canDoDaily) {
                Core.logger(`Friendship dailies unavailable: ${friend.npc}`);
                return;
            } else Core.logger(`Daily: Friendship (${friend.npc})`);

            Core.join(friend.map);
            sendWaitedPacket(`%xt%zm%friendshipInfo%${bot.map.roomId}%${friend.npc}%`);

            if (friend.canTalk && friend.npc !== "Linus") {
                sendWaitedPacket(`%xt%zm%friendshipTalk%${bot.map.roomId}%`);
                sendWaitedPacket(`%xt%zm%friendshipChoice%${bot.map.roomId}%1%`);
                friend = informLogger(`Talked to ${friend.npc}. Through the bot this has a 50% chance of giving hearts.`, friend);
            }
            if (friend.canGift) {
                const giftIds: number[] = [...gifts];
                if (!Core.checkInventory(giftIds, 1, { any: true })) {
                    if (gifts.length === 1) Core.farmingLogger(giftName(gifts[0]), 1);
                    else Core.logger("Farming for one of the following items: " + gifts.map(giftName).join(" | "));

                    switch (gifts[0]) {
                        case FrGift.Chrysanthemums:
                        case FrGift.Orchids:
                        case FrGift.Roses:
                        case FrGift.Sunflowers:
                        case FrGift.Tulips:
                            Core.equipClass(ClassType.Farm);
                            while (!bot.shouldExit && !Core.checkInventory(giftIds, 1, { any: true })) Core.huntMonster("battleodium", "Widowing", null, 1, { log: false });
                            break;

                        case FrGift.Lilies:
                            Core.equipClass(ClassType.Farm);
                            while (!bot.shouldExit && !Core.checkInventory(giftIds, 1, { any: true })) Core.huntMonster("greyguard", "Gloombloom", null, 1, { log: false });
                            break;

                        case FrGift.Chaos_Diemond:
                            Core.equipClass(ClassType.Farm);
                            Core.killMonster("battleodium", "r6", "Left", "Vileture", "Grapes", 1, { isTemp: false });
                            Core.killMonster("battleodium", "r6", "Left", "Diemond", "Diamonds", 1, { isTemp: false });
                            Core.buyItem("battleodium", 2236, "Chaos Diemond");
                            break;

                        case FrGift.Cracked_Opal:
                            Core.equipClass(ClassType.Farm);
                            Core.killMonster("battleodium", "r6", "Left", "Vileture", "Melons", 1, { isTemp: false });
                            while (!bot.shouldExit && !Core.checkInventory(76288)) {
                                Core.killMonster("battleodium", "r6", "Left", "Diemond", null, 1, { log: false });
                                bot.drops.pickup(76288);
                                bot.wait.forPickup(76288);
                            }
                            Core.buyItem("battleodium", 2236, "Cracked Opal");
                            break;

                        case FrGift.Blood_Roseberry:
                            Core.equipClass(ClassType.Farm);
                            Core.huntMonster("battleodium", "Widowing", "Roses", 1, { isTemp: false });
                            Core.killMonster("battleodium", "r6", "Left", "Vileture", "Strawberries", 1, { isTemp: false });
                            Core.buyItem("battleodium", 2236, "Blood Roseberry");
                            break;

                        case FrGift.A_Fish:
                            if (!bot.quests.isUnlocked(9107)) {
                                Core.equipClass(ClassType.Solo);
                                Core.huntMonster("greyguard", "Odium", "A Fish", 1, { isTemp: false });
                            } else {
                                Core.equipClass(ClassType.Farm);
                                Core.huntMonster("battleodium", "Widowing", "Roses", 1, { isTemp: false });
                                Core.killMonster("battleodium", "r6", "Left", "Vileture", "Strawberries", 1, { isTemp: false });
                                //multiple items with name "Rubies"
                                while (!bot.shouldExit && !Core.checkInventory(76286)) Core.killMonster("battleodium", "r6", "Left", "Diemond", null, 1, { log: false });
                                Core.chainComplete(9107);
                                bot.wait.forPickup(gifts[0]);
                            }
                            break;

                        case FrGift.Apples:
                        case FrGift.Bananas:
                        case FrGift.Grapes:
                        case FrGift.Melons:
                        case FrGift.Oranges:
                        case FrGift.Strawberries:
                            Core.equipClass(ClassType.Farm);
                            while (!bot.shouldExit && !Core.checkInventory(giftIds, 1, { any: true })) Core.killMonster("battleodium", "r6", "Left", "Vileture", null, 1, { log: false });
                            break;

                        default:
                            Core.equipClass(ClassType.Farm);
                            while (!bot.shouldExit && !Core.checkInventory(giftIds, 1, { any: true })) Core.killMonster("battleodium", "r6", "Left", "Diemond", null, 1, { log: false });
                            break;
                    }
                }

                Core.jumpWait();
                const selectedGift = bot.inventory.items.find((x) => giftIds.includes(x.id));
                if (!selectedGift) {
                    if (gifts.length > 1) Core.logger("Failed to parse any of the following items from your inventory: " + gifts.map(giftName).join(" | "));
                    else Core.logger(`Failed to find "${giftName(gifts[0])}" in your inventory.`);
                    return;
                }
                sendWaitedPacket(`%xt%zm%friendshipGift%${bot.map.roomId}%${selectedGift.id}%${selectedGift.charItemId}%`);
                informLogger(`Gifted ${selectedGift.name} to ${friend.npc}.`, friend);
                if (bot.inventory.contains(selectedGift.id)) Core.toBank(selectedGift.id);
            }
        };

        const off = bot.events.on("extensionPacket", friendshipPacketReader);

        try {
            if (!refreshFriendshipData()) return;
            if (friends.every((f) => !f.canGift && (!f.canTalk || f.npc === "Linus"))) {
                Core.logger("All the friendship dailies have already been completed today.");
                return;
            }

            bot.drops.add(...this.frGiftIDs);
            Core.addDrop(...this.frRewards);

            // Battleodium
            if (Core.isCompletedBefore(793)) handleFriendship("Dage the Evil", FrGift.Cracked_Opal);
            if (bot.player.level >= 80) handleFriendship("Gravelyn", FrGift.Blood_Roseberry);
            handleFriendship("Nulgath", FrGift.Apples);
            handleFriendship("Twig", FrGift.Melons);
            handleFriendship("Twilly", FrGift.Apples, FrGift.Orchids);
            handleFriendship("Maya", FrGift.Chrysanthemums, FrGift.Apples);
            handleFriendship("Yulgar", FrGift.Turqoise, FrGift.Orchids, FrGift.Melons);
            handleFriendship("Mi", FrGift.Sapphires, FrGift.Lilies);
            handleFriendship("Lord Brentan", FrGift.Oranges, FrGift.Rubies);
            handleFriendship("Warlic", FrGift.Sapphires, FrGift.Sunflowers);
            handleFriendship("Zorbak", FrGift.Apples);
            handleFriendship("Smoglin", FrGift.Turqoise, FrGift.Apples);

            // Greyguard
            if (bot.player.level >= 80) handleFriendship("Drakath", FrGift.Chaos_Diemond);
            handleFriendship("Xang", FrGift.Emeralds, FrGift.Grapes);
            handleFriendship("Linus", FrGift.A_Fish);
            handleFriendship("Sally", FrGift.Rubies, FrGift.Tulips);
            handleFriendship("Xing", FrGift.Opals, FrGift.Bananas);

            if (!Core.hasWebBadge("Penguin BFF")) {
                if (Core.checkInventory("Happy Penguin")) {
                    Core.chainComplete(9108);
                    Core.toBank("Happy Penguin");
                } else Core.logger("🥺 we don't have the cute little penguin so no badge for you...");
            }
        } finally {
            off();
            Core.toBank(...this.frGiftIDs);
            Core.toBank(...this.frRewards.slice(3));
        }
    }

    frGiftIDs: number[] = Object.values(FrGift).filter((x): x is number => typeof x === "number");
    frGiftNames: string[] = Object.keys(FrGift).filter((x) => !/^\d+$/.test(x));
    frRewards = [
        "Gold Voucher 25k",
        "Gold Voucher 100k",
        "Gold Voucher 500k",
        "Happy Penguin",
        "Combat Trophy",
        "Super-Fan Swag Token A",
        "Super-Fan Swag Token B",
        "Dragon Runestone",
        "Faded Pigment",
        "Daily Login Gold Boost! (20 Min)",
        "Daily Login XP Boost! (20 min)",
        "Daily Login Rep Boost! (20 Min)",
        "Arcane Quill",
        "Spirit Orb",
        "Legion Token",
        "Void Aura",
        "Unidentified 10",
    ];

    //#endregion
}

/** The friendship gifts, by item ID (C# `frGift`). */
enum FrGift {
    Roses = 76272,
    Lilies = 76273,
    Tulips = 76274,
    Sunflowers = 76275,
    Chrysanthemums = 76276,
    Orchids = 76277,
    Apples = 76278,
    Oranges = 76279,
    Bananas = 76280,
    Strawberries = 76281,
    Grapes = 76282,
    Melons = 76283,
    Diamonds = 76284,
    Emeralds = 76285,
    Rubies = 76286,
    Sapphires = 76287,
    Opals = 76288,
    Turqoise = 76289,
    Chaos_Diemond = 76355,
    A_Fish = 76322,
    Cracked_Opal = 76657,
    Blood_Roseberry = 76658,
}

/** An entry of the `friendshipStats` packet (C# `FriendshipInfo`). */
interface FriendshipInfo {
    npc: string;
    hearts: number;
    /** Hearts as the game shows them (4 points per heart). */
    displayHearts: number;
    map: string;
    canTalk: boolean;
    giftCount: number;
    /** No gift given today. */
    canGift: boolean;
}

/** The raw `friendshipStats` entry. */
interface RawFriendship {
    strName?: unknown;
    iHearts?: unknown;
    strLocation?: unknown;
    bTalk?: unknown;
    iGifts?: unknown;
}

/** An `extensionPacket` event: `{ params: { type, dataObj } }`. */
interface ExtensionPacket {
    params?: { type?: string; dataObj?: { cmd?: unknown; friendships?: unknown } };
}

function toFriendshipInfo(raw: RawFriendship): FriendshipInfo {
    const hearts = Number(raw?.iHearts) || 0;
    const giftCount = Number(raw?.iGifts) || 0;
    return {
        npc: raw?.strName == null ? "" : String(raw.strName),
        hearts,
        displayHearts: hearts / 4,
        map: raw?.strLocation == null ? "" : String(raw.strLocation),
        canTalk: flag(raw?.bTalk),
        giftCount,
        canGift: giftCount === 0,
    };
}

/** The Mine Crafting metals, by item ID (C# `MineCraftingMetalsEnum`). */
export enum MineCraftingMetalsEnum {
    Aluminum = 11608,
    Barium = 11932,
    Gold = 12157,
    Iron = 12263,
    Copper = 12297,
    Silver = 12308,
    Platinum = 12315,
}

/** The Hard Core Metals metals, by item ID (C# `HardCoreMetalsEnum`). */
export enum HardCoreMetalsEnum {
    Arsenic = 11287,
    Beryllium = 11534,
    Chromium = 11591,
    Palladium = 11864,
    Rhodium = 12032,
    Thorium = 12075,
    Mercury = 12122,
}

/** The rewards of the free daily boost (C# `DailyBoostRewards`). */
export enum DailyBoostRewards {
    EXP = 27552, // XP Boost
    Gold = 27553, // Gold Boost
    Rep = 27554, // Rep Boost
    Class = 27555, // Class Boost
    LowestQuantOwned = 0, // Special case for selecting the reward with the lowest quantity
}
