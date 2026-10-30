/*
name: CoreStory
description: Port of Scripts/CoreStory.cs: the quest helpers story scripts use (KillQuest,
    MapItemQuest, BuyQuest, ChainQuest, QuestProgression, LegacyQuestManager, PreLoad).
tags: core
*/
/**
 * CoreStory — TypeScript port of `Scripts/CoreStory.cs`.
 *
 * Each script makes its own instance, as the C# scripts do (`private static CoreStory Story`):
 *
 * ```ts
 * import { CoreStory } from "../core/CoreStory";
 * const Story = new CoreStory();
 *
 * function storyLine(): void {
 *     Story.preLoad(storyLine);
 *     Story.killQuest(3908, "frozentower", "Polar Elemental");
 *     Story.mapItemQuest(3910, "frozentower", 3004, 13);
 *     Story.killQuest(6593, "stalagbite", "Balboa", { getReward: false });
 * }
 * ```
 *
 * ## Porting notes
 * - C# named flags become {@link StoryOptions}: `KillQuest(id, map, mon, GetReward: false)` ->
 *   `Story.killQuest(id, map, mon, { getReward: false })`.
 * - C# overloads are one method: `killQuest` takes a monster or a list of them, `mapItemQuest`
 *   a map item ID, a list of IDs, or `[itemId, amount, map]` entries.
 * - `PreLoad(this)` reads the C# source of the calling method; `preLoad(fn)` reads the source of
 *   the function you pass instead.
 * - The private `_SmartKill`, `_MonsterHunt`, `_RepeatCheck` and `_AddRequirement` are never
 *   called in the C# file, so they aren't ported.
 */

import { bot } from "turu";
import type { Monster, QuestItem } from "turu";
import { Core, formatForCompare } from "./CoreBots";
import type { QuestData } from "./CoreBots";

/** The flags the C# story methods take as optional arguments. */
export interface StoryOptions {
    /** Whether `reward` should be added with AddDrop (default true). */
    getReward?: boolean;
    /** What item should be added with AddDrop (default "All": every reward of the quest). */
    reward?: string;
    /** Whether to turn in the quest once it can be completed (default true). */
    autoCompleteQuest?: boolean;
}

/** `[mapItemId, amount, map]`: one map item of a quest that spans several maps. */
export type MapItemEntry = readonly [mapItemId: number, amount: number, map: string];

/** One quest of a {@link CoreStory.legacyQuestManager} chain (C# `LegacyQuestObject`). */
interface LegacyQuestObject {
    /** In order to do ... */
    desiredQuestId: number;
    /** And obtain ... */
    desiredQuestReward: QuestItem[];
    /** You must do ... */
    requiredQuestId: number;
    /** And obtain ... */
    requiredQuestReward: QuestItem[] | null;
}

export class CoreStory {
    // [Can Change]
    // True = Bot only does its smart checks on quests with Once: True
    // False = Bot does it's smart checks on all quest
    // Recommended: false
    // Used for testing bots, dont toggle this as a user
    testBot = false;

    //#region KillQuest

    /**
     * Completes a quest by killing a monster (or, with a list, one monster per requirement) until
     * all required items are obtained. Skipped when the quest was completed before.
     * @param questId The ID of the quest to complete.
     * @param mapName The map in which the quest takes place.
     * @param monster The monster to hunt, or one name per quest item (when there are fewer names
     *   than items, the last non-empty name is used for the rest).
     */
    killQuest(questId: number, mapName: string, monster: string | readonly string[], options: StoryOptions = {}): void {
        if (Array.isArray(monster)) this.killQuestMany(questId, mapName, monster as readonly string[], options);
        else this.killQuestOne(questId, mapName, monster as string, options);
    }

    /** C# `KillQuest(int, string, string, ...)`. */
    private killQuestOne(questId: number, mapName: string, monsterName: string, { getReward = true, reward = "All", autoCompleteQuest = true }: StoryOptions): void {
        if (!mapName) throw new Error("MapName cannot be null or empty");
        if (!monsterName) throw new Error("MonsterName cannot be null or empty");

        Core.debugLogger(`Starting KillQuest: QuestID=${questId}, Map=${mapName}, Monster=${monsterName}`);

        const questData = Core.initializeWithRetries(() => Core.ensureLoad(questId));
        if (questData === null) {
            Core.debugLogger(`Quest ${questId} could not be loaded.`);
            return;
        }

        if (this.questProgression(questId, { getReward, reward })) return;

        if (questData.requirements.length === 0) {
            Core.debugLogger(`Quest ${questId} has no requirements. Nothing to farm.`);
            return;
        }

        //Prevent turnin spam
        Core.acceptandCompleteTries = 5;

        // Filter valid requirements and exclude items already obtained
        const validRequirements = questData.requirements.filter((r) => r && r.name).filter((r) => !this.have(r));

        if (validRequirements.length === 0) {
            Core.debugLogger(`All quest requirements for Quest ${questId} are already satisfied.`);
            return;
        }

        // Accept the quest and join the map
        Core.debugLogger(`Accepting quest ${questId} and joining map ${mapName}`);
        Core.ensureAccept(questId);
        Core.join(mapName);

        // Snapshot CurrentRequirements
        this.currentRequirements.length = 0;
        this.currentRequirements.push(...validRequirements);

        // Add drops for quest items
        const drops = this.currentRequirements.filter((r) => !r.temp && r.name).map((r) => r.name);

        if (drops.length > 0) {
            Core.addDrop(...drops);
            Core.debugLogger(`Added drops for quest ${questId}: [${drops.join(", ")}]`);
        }

        // Farming loop
        while (this.currentRequirements.length > 0) {
            // Remove already obtained items
            this.removeObtained();
            if (this.currentRequirements.length === 0) break;

            const itemsToFarm = this.currentRequirements.map((r) => r.name).filter((n) => n);
            if (itemsToFarm.length === 0) break;

            this.monsterHuntBatch(mapName, monsterName, itemsToFarm, questId);

            //a little extra check for if it got turned in by itself...
            if (this.questProgression(questId, { getReward, reward, log: false })) return;
        }

        // Snapshot items farmed
        const farmedItems = validRequirements.map((r) => r.name);

        // Complete the quest
        Core.debugLogger(`Attempting to complete quest ${questId}`);
        this.tryComplete(questData, autoCompleteQuest);

        // Delay & cleanup
        bot.sleep(1000);
        this.currentRequirements.length = 0;

        Core.debugLogger(`Finished KillQuest: QuestID=${questId}. Items farmed: [${farmedItems.join(", ")}]`);
    }

    /** C# `KillQuest(int, string, string[], ...)`. */
    private killQuestMany(questId: number, mapName: string, monsterNames: readonly string[], { getReward = true, reward = "All", autoCompleteQuest = true }: StoryOptions): void {
        if (!mapName) throw new Error("MapName cannot be null or empty");
        if (monsterNames.length === 0 || monsterNames.every((m) => !m)) throw new Error("MonsterNames cannot be null or empty");

        Core.debugLogger(`Starting KillQuest: QuestID=${questId}, Map=${mapName}, Monsters=[${monsterNames.join(", ")}]`);

        const questData = Core.initializeWithRetries(() => Core.ensureLoad(questId));
        if (questData === null) {
            Core.debugLogger(`Quest ${questId} could not be loaded.`);
            return;
        }

        if (this.questProgression(questId, { getReward, reward })) return;

        if (questData.requirements.length === 0) {
            Core.debugLogger(`Quest ${questId} has no requirements. Nothing to farm.`);
            return;
        }

        //Prevent turnin spam
        Core.acceptandCompleteTries = 5;

        // Filter valid requirements and exclude items already obtained
        const validRequirements = questData.requirements.filter((r) => r && r.name).filter((r) => !this.have(r));

        if (validRequirements.length === 0) {
            Core.debugLogger(`All quest requirements for Quest ${questId} are already satisfied.`);
            return;
        }

        Core.debugLogger(`Accepting quest ${questId} and joining map ${mapName}`);
        Core.ensureAccept(questId);
        Core.join(mapName);

        // Map each remaining requirement to its corresponding monster.
        const itemToMonster = new Map<string, string>();
        const lastMonster = [...monsterNames].reverse().find((m) => m) ?? "";

        for (let i = 0; i < validRequirements.length; i++) {
            const monster = i < monsterNames.length && monsterNames[i] ? monsterNames[i] : lastMonster;

            itemToMonster.set(validRequirements[i].name, monster);
            Core.logger(`[${questId}] Requirement ${validRequirements[i].name} mapped to monster ${monster} (index=${i})`);
        }

        // Snapshot CurrentRequirements to avoid nulls
        this.currentRequirements.length = 0;
        this.currentRequirements.push(...validRequirements);

        // Add drops for items not already in inventory
        const drops = this.currentRequirements.filter((r) => !r.temp && r.name).map((r) => r.name);

        if (drops.length > 0) {
            Core.addDrop(...drops);
            Core.debugLogger(`Added drops: [${drops.join(", ")}]`);
        }

        // Main farming loop
        while (this.currentRequirements.some((r) => !this.have(r))) {
            // Remove completed items from CurrentRequirements
            this.removeObtained();

            if (this.currentRequirements.length === 0) break;

            // Group remaining items by monster
            const monsterGroups = new Map<string, QuestItem[]>();
            for (const r of this.currentRequirements) {
                if (!r.name) continue;
                const monster = itemToMonster.get(r.name) ?? "";
                monsterGroups.set(monster, [...(monsterGroups.get(monster) ?? []), r]);
            }

            for (const [monster, group] of monsterGroups) {
                if (!monster) {
                    Core.debugLogger("Skipped group with empty monster name.");
                    continue;
                }

                const itemsToFarm = group
                    .filter((r) => !this.have(r))
                    .map((r) => r.name)
                    .filter((n) => n);

                if (itemsToFarm.length === 0) {
                    Core.debugLogger(`No valid items to farm for monster ${monster}, skipping.`);
                    continue;
                }

                this.monsterHuntBatch(mapName, monster, itemsToFarm, questId);
            }
        }

        // Complete the quest
        Core.debugLogger(`Attempting to complete quest ${questId}`);
        this.tryComplete(questData, autoCompleteQuest);

        // Small delay and cleanup
        bot.sleep(200);
        this.currentRequirements.length = 0;

        Core.debugLogger(`Finished KillQuest: QuestID=${questId}. Items farmed: [${validRequirements.map((r) => r.name).join(", ")}]`);
    }

    /**
     * Hunts a monster for specific quest items: moves to the cell with the most of them, attacks
     * them and picks up the drops, until the items are obtained or the quest is completed.
     */
    private monsterHuntBatch(map: string, monster: string, itemNames: string[], qid: number): void {
        if (!map) {
            bot.log("Map is null or empty");
            return;
        }

        if (!monster) {
            bot.log("Monster name is null or empty");
            return;
        }

        if (itemNames.length === 0) {
            bot.log("itemNames list is empty");
            return;
        }

        const sameName = (m: Monster) => formatForCompare(m.name) === formatForCompare(monster);
        const sameText = (a: string | null | undefined, b: string | null | undefined) => (a ?? "").toLowerCase() === (b ?? "").toLowerCase();
        const stillNeeded = () => this.currentRequirements.filter((r) => itemNames.includes(r.name) && !this.have(r));

        // Ensure we are on the correct map
        if (bot.map.name !== map) {
            Core.join(map);
            bot.wait.forMapLoad(map);
        }

        // Refresh needed items once before the loop for the initial log
        let neededItems = stillNeeded();

        if (neededItems.length === 0) {
            Core.logger(`All requested items for "${monster}" are already satisfied: [${itemNames.join(", ")}]`, "_MonsterHuntBatch");
            return;
        }

        // **Important log moved here** — logs once per call
        Core.logger(`Farming monster "${monster}" for items: [${neededItems.map((r) => `${r.name} x${r.quantity}`).join(", ")}]`, "_MonsterHuntBatch");

        while (!bot.shouldExit) {
            neededItems = stillNeeded();

            if (neededItems.length === 0 || this.questProgression(qid)) break;

            const cells = new Map<string, number>();
            for (const m of bot.monsters.mapMonsters.filter(sameName)) cells.set(m.cell, (cells.get(m.cell) ?? 0) + 1);
            const [targetCell, targetCount] = [...cells].sort((a, b) => b[1] - a[1])[0] ?? ["Enter", 0];

            if (!sameText(bot.player.cell, targetCell)) {
                Core.debugLogger(`Jumping to cell '${targetCell}' with ${targetCount} ${monster}s`, "_MonsterHuntBatch");

                if (!sameText(bot.map.name, map)) {
                    Core.join(map);
                    bot.wait.forMapLoad(map);
                }

                // jump only if player not already there
                if (!sameText(bot.player.cell, targetCell)) {
                    bot.map.jump(targetCell, "Left", false);
                    bot.wait.forCellChange(targetCell);
                    bot.player.setSpawnPoint();
                }
            }

            if (!bot.player.alive) {
                bot.wait.forTrue(() => bot.player.alive, 20);
                continue;
            }

            for (const m of bot.monsters.currentAvailableMonsters.filter(sameName)) {
                if (m.hp <= 0) continue;
                while (!bot.shouldExit && (neededItems.length > 0 || !this.questProgression(qid))) {
                    if (!sameText(bot.map.name, map)) {
                        Core.join(map);
                        bot.wait.forMapLoad(map);
                    }

                    // cell check and jump
                    if (!sameText(bot.player.cell, targetCell)) {
                        bot.map.jump(targetCell, "Left", false);
                        bot.wait.forCellChange(targetCell);
                        bot.player.setSpawnPoint();
                    }
                    Core.canWeAggro();

                    if (!bot.player.hasTarget || bot.player.target?.mapId !== m.mapId) bot.combat.attack(m.mapId);

                    bot.sleep(500);

                    if (!bot.player.hasTarget || (bot.player.target?.hp ?? 0) <= 0) break;

                    const names = neededItems.map((r) => r.name);
                    if (names.length > 0 && bot.drops.current.some((d) => names.includes(d.name))) bot.drops.pickup(...names);

                    neededItems = stillNeeded();
                }
            }

            if (this.currentRequirements.length > 0) {
                const kept = this.currentRequirements.filter((r) => !(itemNames.includes(r.name) && this.have(r)));
                this.currentRequirements.length = 0;
                this.currentRequirements.push(...kept);
            }
        }
    }

    //#endregion

    //#region MapItemQuest

    /**
     * Gets map items for a quest, and turns in the quest if possible. Skipped when the quest was
     * completed before.
     * @param questId ID of the quest.
     * @param mapName Map where the items are.
     * @param mapItem ID of the map item, or a list of IDs (each grabbed `amount` times).
     * @param amount How many of each map item to grab.
     */
    mapItemQuest(questId: number, mapName: string, mapItem: number | readonly number[], amount?: number, options?: StoryOptions): void;
    /**
     * Gets map items spread over several maps for a quest (grouped per map), and turns in the
     * quest if possible.
     * @param mapItems `[mapItemId, amount, map]` entries.
     */
    mapItemQuest(questId: number, mapItems: readonly MapItemEntry[], options?: StoryOptions): void;
    mapItemQuest(
        questId: number,
        mapNameOrItems: string | readonly MapItemEntry[],
        mapItemOrOptions?: number | readonly number[] | StoryOptions,
        amount = 1,
        options: StoryOptions = {},
    ): void {
        if (typeof mapNameOrItems !== "string") {
            this.mapItemQuestEntries(questId, mapNameOrItems, (mapItemOrOptions as StoryOptions | undefined) ?? {});
            return;
        }
        const mapItem = mapItemOrOptions as number | readonly number[];
        if (typeof mapItem === "number") this.mapItemQuestOne(questId, mapNameOrItems, mapItem, amount, options);
        else this.mapItemQuestIds(questId, mapNameOrItems, mapItem, amount, options);
    }

    /** C# `MapItemQuest(int, string, int, ...)`. */
    private mapItemQuestOne(questId: number, mapName: string, mapItemId: number, amount: number, { getReward = true, reward = "All", autoCompleteQuest = true }: StoryOptions): void {
        const questData = Core.initializeWithRetries(() => Core.ensureLoad(questId));
        if (questData === null) {
            Core.logger(`Quest with ID ${questId} not found`);
            return;
        }
        if (this.questProgression(questId, { getReward, reward })) return;

        if (bot.map.name !== mapName) Core.join(mapName);

        Core.ensureAccept(questId);
        Core.getMapItem(mapItemId, amount, mapName);
        this.tryComplete(questData, autoCompleteQuest);
    }

    /** C# `MapItemQuest(int, string, int[], ...)`. */
    private mapItemQuestIds(questId: number, mapName: string, mapItemIds: readonly number[], amount: number, { getReward = true, reward = "All", autoCompleteQuest = true }: StoryOptions): void {
        const questData = Core.initializeWithRetries(() => Core.ensureLoad(questId));
        if (questData === null) {
            Core.logger(`Quest with ID ${questId} not found`);
            return;
        }

        if (this.questProgression(questId, { getReward, reward })) return;

        Core.ensureAccept(questId);

        // Build the list of map items to grab
        const itemsToGrab = mapItemIds.filter((id) => !bot.tempInv.contains(id, amount)).map((id) => [id, amount] as const);

        if (itemsToGrab.length > 0) {
            Core.logger(`Grabbing items from map ${mapName}: ${itemsToGrab.map(([id, quantity]) => `${id} x${quantity}`).join(", ")}`);
            Core.getMapItems(itemsToGrab, mapName);
        }

        this.tryComplete(questData, autoCompleteQuest);
    }

    /** C# `MapItemQuest(int, (int MapItemID, int Amount, string MapName)[], ...)`. */
    private mapItemQuestEntries(questId: number, mapItems: readonly MapItemEntry[], { getReward = true, reward = "All", autoCompleteQuest = true }: StoryOptions): void {
        const questData = Core.initializeWithRetries(() => Core.ensureLoad(questId));
        if (questData === null) {
            Core.logger(`Quest with ID ${questId} not found`);
            return;
        }

        if (this.questProgression(questId, { getReward, reward })) return;

        Core.ensureAccept(questId);

        // Group items by map (only items not already in temp inv)
        const itemsGroupedByMap = new Map<string, (readonly [number, number])[]>();
        for (const [mapItemId, amount, mapName] of mapItems) {
            if (bot.tempInv.contains(mapItemId, amount)) continue;
            itemsGroupedByMap.set(mapName, [...(itemsGroupedByMap.get(mapName) ?? []), [mapItemId, amount]]);
        }

        for (const [map, itemsToGrab] of itemsGroupedByMap) {
            if (itemsToGrab.length > 0) {
                Core.logger(`Grabbing items from map ${map}: ${itemsToGrab.map(([id, quantity]) => `${id} x${quantity}`).join(", ")}`);
                Core.join(map);
                Core.getMapItems(itemsToGrab, map);
            }
        }

        this.tryComplete(questData, autoCompleteQuest);
    }

    //#endregion

    //#region MiscQuest

    /**
     * Buys an item for a quest, and turns in the quest if possible. Skipped when the quest was
     * completed before.
     * @param questId ID of the quest.
     * @param mapName Map where the shop is located.
     * @param shopId ID of the shop.
     * @param itemName Name of the item to buy.
     * @param amount The amount of `itemName` to buy.
     */
    buyQuest(questId: number, mapName: string, shopId: number, itemName: string, amount = 1, { getReward = true, reward = "All", autoCompleteQuest = true }: StoryOptions = {}): void {
        const questData = Core.initializeWithRetries(() => Core.ensureLoad(questId));
        if (questData === null) {
            Core.logger(`Quest with ID ${questId} not found`);
            return;
        }

        if (this.questProgression(questId, { getReward, reward })) return;

        Core.ensureAccept(questId);
        Core.buyItem(mapName, shopId, itemName, amount);
        this.tryComplete(questData, autoCompleteQuest);
    }

    /**
     * Accepts a quest and then turns it in again. Skipped when the quest was completed before.
     * @param questId ID of the quest.
     */
    chainQuest(questId: number, { getReward = true, reward = "All", autoCompleteQuest = true }: StoryOptions = {}): void {
        const questData = Core.initializeWithRetries(() => Core.ensureLoad(questId));
        if (questData === null) {
            Core.logger(`Quest with ID ${questId} not found`);
            return;
        }

        if (this.questProgression(questId, { getReward, reward })) return;

        this.tryComplete(questData, autoCompleteQuest);
    }

    //#endregion

    /** Turns in a quest if you have its items. */
    questComplete(questId: number): void {
        this.tryComplete(
            Core.initializeWithRetries(() => Core.ensureLoad(questId), 20),
            true,
        );
    }

    // C# never reads autoCompleteQuest here either: the quest is always turned in.
    private tryComplete(quest: QuestData | null, _autoCompleteQuest: boolean): void {
        if (quest === null) {
            Core.logger("QuestData is null, cannot complete quest");
            return;
        }

        const questData = Core.initializeWithRetries(() => Core.ensureLoad(quest.id));
        if (questData === null) {
            Core.logger(`Quest with ID ${quest.id} not found`);
            return;
        }

        const missingItems = [...questData.requirements, ...questData.acceptRequirements]
            .filter((x) => x && !Core.checkInventory(x.id, x.quantity))
            .map((x) => {
                const have = x.temp ? bot.tempInv.getQuantity(x.id) : bot.inventory.getQuantity(x.name);
                return `${x.name}[${x.id}] x${x.quantity} (have ${have})`;
            });

        if (missingItems.length > 0) {
            Core.logger(`Missing items for quest [${questData.id}] "${questData.name}": ${missingItems.join(", ")}`, "QuestProgression");
            return;
        }

        Core.sleep();

        // Always force proper completion
        Core.ensureAccept(questData.id);
        Core.ensureComplete(questData.id);

        bot.wait.forQuestComplete(questData.id);

        for (const reward of questData.rewards.map((r) => r.name)) bot.wait.forPickup(reward);

        Core.logger(`Completed Quest: [${questData.id}] - "${questData.name}"`, "QuestProgression");

        Core.sleep();
    }

    /**
     * Skeleton of killQuest, mapItemQuest, buyQuest and chainQuest. Returns true when the quest
     * was completed before (or you already have `reward`), so the caller skips it. Only needs to
     * be used inside a script if the quest spans across multiple maps. When the quest isn't
     * unlocked yet, it re-completes the previous quest of its chain, and relogs if that fails.
     * @param questId ID of the quest.
     */
    questProgression(questId: number, { getReward = true, reward = "All", log = true }: Omit<StoryOptions, "autoCompleteQuest"> & { log?: boolean } = {}): boolean {
        if (questId <= 0) return false;

        // Cache previous result to avoid repeated processing in same loop tick
        if (this.previousQuestId === questId) return this.previousQuestState;
        this.previousQuestId = questId;

        // Load CBO test flag once
        if (!this.cboChecked) {
            const testBot = Core.cboBool("BCO_Story_TestBot");
            if (testBot !== null) this.testBot = testBot;
            this.cboChecked = true;
        }

        const questData = Core.initializeWithRetries(() => Core.ensureLoad(questId));
        if (questData === null) {
            Core.logger(`Quest with ID ${questId} not found`);
            return true;
        }

        let attempts = 0;

        // ─────────────────────────────────────────────────────────────
        // QUEST UNLOCK RECOVERY LOOP
        // ─────────────────────────────────────────────────────────────
        while (!bot.quests.isUnlocked(questId)) {
            Core.sleep(1000);

            const currentValue = questData.slot > 0 ? bot.flash.callGameFunction<number>("world.getQuestValue", questData.slot) : 0;

            if (attempts === 0 || attempts % 3 === 0)
                Core.logger(`Progress check: Slot ${questData.slot} | Current ${currentValue} / Target ${questData.value - 1}`, "QuestProgression");

            // Find previous quest in same chain safely
            const prevQuest =
                bot.quests.tree
                    .filter((q) => q.slot === questData.slot && q.value < questData.value)
                    .sort((a, b) => b.value - a.value)[0] ?? null;

            if (prevQuest !== null) {
                // Safely gather requirements (quests may have null collections)
                const prevReqs = [...(prevQuest.requirements ?? []), ...(prevQuest.acceptRequirements ?? [])].map((req) => req.name);

                // If we already have the items, re-complete the quest to repair the chain
                if (prevReqs.length > 0 && Core.checkInventory(prevReqs)) {
                    Core.logger(`Attempting recovery via re-completing previous quest: [${prevQuest.id}] "${prevQuest.name}"`, "QuestProgression");

                    this.tryComplete(Core.ensureLoad(prevQuest.id), true);
                    attempts = 0;
                    continue;
                }

                // Log missing requirements
                const missingReqs = prevReqs.filter((req) => !Core.checkInventory(req));
                if (missingReqs.length > 0) {
                    bot.log(`Missing [${missingReqs.join(", ")}] to accept ${questData.name} [${questData.id}]`);
                    attempts = 5; // force relog sooner
                }
            }

            attempts++;

            // Relog after repeated failures
            if (attempts >= 5) {
                Core.logger(`Quest [${questId}] "${questData.name}" still not unlocked after retries. Relogging...`, "QuestProgression");
                Core.relogin("Quest progression recovery failed, relogging.");
                attempts = 0;
            }
        }

        // ─────────────────────────────────────────────────────────────
        // COMPLETION CHECK
        // ─────────────────────────────────────────────────────────────
        if (Core.isCompletedBefore(questId) && (!this.testBot || questData.once)) {
            if (log) {
                if (this.testBot) Core.logger(`Skipped (Once = true): [${questId}] - "${questData.name}"`);
                else Core.logger(`Already Completed: [${questId}] - "${questData.name}"`);
            }

            this.previousQuestState = true;
            return true;
        }

        // ─────────────────────────────────────────────────────────────
        // REWARD HANDLING
        // ─────────────────────────────────────────────────────────────
        if (getReward) {
            if (reward !== "All") {
                if (Core.checkInventory(reward)) {
                    Core.logger(`Already have reward "${reward}", skipping quest.`);
                    this.previousQuestState = true;
                    return true;
                }

                Core.addDrop(reward);
            } else Core.addDrop(...Core.questRewards(questId));
        }

        Core.logger(`Doing Quest: [${questId}] - "${questData.name}"`);

        this.previousQuestState = false;
        return false;
    }

    private cboChecked = false;

    /**
     * For story lines where each quest needs an item from another quest of the list (the
     * "legacy" chains): works out the order from the quests' rewards and accept requirements,
     * and runs `questLogic` for each quest to do, with {@link legacyQuestId} set to its ID.
     */
    legacyQuestManager(questLogic: () => void, ...questIds: number[]): void {
        const questData = Core.initializeWithRetries(() => Core.ensureLoadMany(...questIds));
        const whereToGet: LegacyQuestObject[] = [];
        if (questData === null || questData.length === 0) {
            Core.logger("No quests found, cannot run LegacyQuestManager", "", { messageBox: true });
            return;
        }

        const sameItem = (a: QuestItem, b: QuestItem) => a.id === b.id || a.name === b.name;

        //Core.DL_Enable();
        Core.debugLogger("-------------\t");
        for (const quest of questData) {
            const desiredQuestReward: QuestItem[] = quest.rewards.filter((r) => questData.some((q) => q.acceptRequirements.some((a) => sameItem(a, r))));
            const requiredQuestId = questData.find((q) => q.rewards.some((r) => quest.acceptRequirements.some((a) => sameItem(a, r))))?.id ?? 0;
            const requiredQuestReward: QuestItem[] = quest.acceptRequirements.filter((r) => questData.some((q) => q.rewards.some((a) => sameItem(a, r))));

            Core.debugLogger(`${quest.id}\t\t`);
            Core.debugLogger(`${desiredQuestReward[0]?.name ?? ""}\t`);
            Core.debugLogger(`${requiredQuestId}\t\t`);
            Core.debugLogger(`${requiredQuestReward[0]?.name ?? ""}\t`);
            Core.debugLogger("-------------\t");

            if (requiredQuestReward.length === 0 && quest.acceptRequirements.length > 0) {
                Core.logger(
                    'The managed failed to find the location of "' +
                        quest.acceptRequirements.map((a) => a.name).join('" + "') +
                        `" for Quest ID ${quest.id}, is the function missing a Quest ID?`,
                    "",
                    { messageBox: true },
                );
                return;
            }

            whereToGet.push({ desiredQuestId: quest.id, desiredQuestReward, requiredQuestId, requiredQuestReward });
        }

        if (whereToGet.every((x) => x.desiredQuestReward.length === 0) || whereToGet.every((x) => x.requiredQuestReward?.length === 0)) {
            const ids = questData.map((q) => q.id).join(", ");

            Core.logger(
                `None of the Quest IDs filled in (${ids}) are supposed to be used in the LegacyQuestManager, ` +
                    "please report to the bot makers that they must make this story line in the normal way.",
            );

            return;
        }

        const finalItemQuest = whereToGet.find((x) => x.desiredQuestReward.length === 0);
        if (!finalItemQuest || finalItemQuest.desiredQuestId <= 0) {
            Core.logger("Could not find the Quest ID of the last quest in the item chain");
            return;
        }

        Core.logger(`Final quest in Legacy Quest Chain: [${finalItemQuest.desiredQuestId}] "${Core.ensureLoad(finalItemQuest.desiredQuestId).name}"`);

        const finalRewardIds = () => Core.ensureLoad(finalItemQuest.desiredQuestId).rewards.map((x) => x.id);

        const runQuest = (questId: number): void => {
            const runQuestData = whereToGet.find((d) => d.desiredQuestId === questId);

            if (!runQuestData) {
                Core.logger("runQuestData is NULL");
                return;
            }
            const quest = Core.initializeWithRetries(() => Core.ensureLoad(questId));
            if (quest === null) {
                Core.logger(`Quest with ID ${questId} not found`);
                return;
            }

            const requiredReward = (runQuestData.requiredQuestReward ?? []).map((i) => i.id);
            if (runQuestData.desiredQuestReward.length === 0 && questId !== finalItemQuest.desiredQuestId) {
                if (!Core.checkInventory(requiredReward)) runQuest(runQuestData.requiredQuestId);
                return;
            }

            const desiredReward = runQuestData.desiredQuestReward.map((i) => i.id);
            if (questId !== finalItemQuest.desiredQuestId ? Core.checkInventory(desiredReward) : Core.checkInventory(finalRewardIds())) {
                Core.logger(`Already Completed: [${questId}] - "${quest.name}"`, "QuestProgression");
                return;
            }

            if (!Core.checkInventory(requiredReward)) runQuest(runQuestData.requiredQuestId);

            if (this.legacyQuestStopped) return;

            Core.logger(`Doing Quest: [${questId}] - "${quest.name}"`, "QuestProgression");
            Core.ensureAccept(questId);
            Core.addDrop(...desiredReward);

            this.legacyQuestId = questId;
            questLogic();

            this.tryComplete(quest, this.legacyQuestAutoComplete);
            for (const i of desiredReward) bot.wait.forPickup(i);
            if (questId === finalItemQuest.desiredQuestId) bot.drops.pickup(...finalRewardIds());
            this.legacyQuestAutoComplete = true;
        };

        runQuest(finalItemQuest.desiredQuestId);

        for (const l of whereToGet) if (l.requiredQuestReward !== null) Core.toBank(...l.requiredQuestReward.map((i) => i.id));
    }

    /** The quest {@link legacyQuestManager} is running `questLogic` for. */
    legacyQuestId = -1;
    /** Set to false in `questLogic` to skip turning in the current legacy quest once. */
    legacyQuestAutoComplete = true;
    private legacyQuestStopped = false;

    /** Makes {@link legacyQuestManager} stop before the next quest. */
    legacyQuestStop(): void {
        this.legacyQuestStopped = true;
    }

    /**
     * Put this at the start of your story function so that the bot will load all quests that are
     * used in it. This will speed up any progression checks tremendously.
     *
     * C# reads the calling method from the compiled script; here pass the function itself
     * (`Story.preLoad(storyLine)`, or `Story.preLoad(this.storyLine)` for a method). Its source
     * is searched for `Story.killQuest(1234, ...)`-like calls, as C# does.
     */
    preLoad(storyFunction: (...args: never[]) => unknown): void {
        const questIds: number[] = [];
        const scriptSlice = String(storyFunction).split(/\r?\n/);

        if (scriptSlice.length <= 1) {
            Core.logger("PreLoad failed, cannot read the function's source");
            return;
        }

        const searchParam = [
            "Story.killQuest",
            "Story.mapItemQuest",
            "Story.buyQuest",
            "Story.chainQuest",
            "Story.questProgression",
            "Core.ensureAccept",
            "Core.ensureComplete",
            "Core.ensureCompleteChoose",
            "Core.chainComplete",
        ];

        const tree = bot.quests.tree;
        for (const line of scriptSlice) {
            if (!/\d/.test(line)) continue;

            // C# removes spaces; the transpiled source is indented with tabs, so remove all whitespace.
            const edittedLine = line.replace(/\s/g, "").replace(/!/g, "").replace(/\(/g, "").replace(/if/g, "").replace(/else/g, "");

            if (!searchParam.some((x) => edittedLine.startsWith(x))) continue;

            const questId = Number.parseInt(/\d+/.exec(line)?.[0] ?? "", 10);

            if (!questIds.includes(questId) && !tree.some((x) => x.id === questId)) questIds.push(questId);
        }

        if (questIds.length + tree.length > Core.loadedQuestLimit && questIds.length < Core.loadedQuestLimit) bot.flash.set("world.questTree", {});
        else if (questIds.length > Core.loadedQuestLimit - tree.length) {
            Core.logger(`Found ${questIds.length} Quests, this exceeds the max amount of loaded quests (${Core.loadedQuestLimit}). No quests will be loaded.`);
            return;
        }

        Core.logger(`Loading ${questIds.length} Quests.`);

        if (questIds.length > 30) {
            const estimatedTime = (questIds.length / 30.0) * 1.6;
            Core.logger(`Estimated Loading Time: ${Math.ceil(estimatedTime)}s`);
        }

        for (let i = 0; i < questIds.length; i += 30) {
            bot.quests.load(...questIds.slice(i, i + 30));
            Core.sleep(1500);
        }
    }

    private previousQuestId = 0;
    private previousQuestState = false;

    private readonly currentRequirements: QuestItem[] = [];

    /** C# `r.Temp ? Bot.TempInv.Contains(r.Name, r.Quantity) : Core.CheckInventory(r.ID, r.Quantity)`. */
    private have(r: QuestItem): boolean {
        return r.temp ? bot.tempInv.contains(r.name, r.quantity) : Core.checkInventory(r.id, r.quantity);
    }

    /** C# `CurrentRequirements.RemoveAll(have)`. */
    private removeObtained(): void {
        const kept = this.currentRequirements.filter((r) => !this.have(r));
        this.currentRequirements.length = 0;
        this.currentRequirements.push(...kept);
    }
}
