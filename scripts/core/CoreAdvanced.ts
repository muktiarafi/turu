/*
name: CoreAdvanced
description: Port of the shop part of Scripts/CoreAdvanced.cs: buys an item after farming what
    the purchase needs (reputation, level, gold, Gold Vouchers, Dragon Runestones).
tags: core
*/
/**
 * CoreAdvanced — partial TypeScript port of `Scripts/CoreAdvanced.cs`.
 *
 * {@link CoreAdvanced.buyItem} is C# `BuyItem`: unlike CoreBots' `buyItem`, which gives up when a
 * requirement is missing, it farms the reputation, level, gold, Gold Vouchers and Dragon Runestones
 * the item needs first. {@link CoreAdvanced.startBuyAllMerge} is the merge-shop template the merge
 * scripts use. Only the shop path is here.
 */

import { bot, choices, option } from "turu";
import type { ScriptOption } from "turu";
import { Core, type CoreShopItem, type ShopRequirement } from "./CoreBots";
import { CoreFarms } from "./CoreFarms";

const Farm = new CoreFarms();

export class CoreAdvanced {
    /**
     * Buys an item from a shop, farming what it needs first (C# `BuyItem`). `item` is a name or an
     * ID. An ID also checks the inventory and house space and waits out combat first, as C#'s ID
     * overload does.
     * @param map Map of the shop.
     * @param shopId ID of the shop.
     * @param item Name or ID of the item.
     * @param quant Desired quantity.
     * @param shopItemId ShopItemID of the entry you want, to tell apart entries of the same item.
     * @param index Which of several matching entries to buy (merge items).
     * @param log Log the purchase.
     */
    buyItem(map: string, shopId: number, item: string | number, quant = 1, shopItemId = 0, index = 0, log = true): void {
        if (Core.checkInventory(item, quant)) return;

        if (typeof item === "number" && Core.inventoryBagFreeSlots <= 0 && !bot.inventory.contains(item)) {
            if (log) Core.logger("❌ Inventory full, cannot buy items.");
            return;
        }

        Core.join(map);
        bot.wait.forMapLoad(map);
        Core.jumpWait();

        if (typeof item === "number" && (bot.player.inCombat || bot.player.hasTarget)) {
            Core.jumpWait();
            bot.wait.forCombatExit();
        }

        const shopItem = Core.parseShopItem(Core.getShopItems(map, shopId), shopId, item, shopItemId);
        if (!shopItem) {
            if (log && typeof item === "number") Core.logger(`❌ Item ${item} not found in shop ${shopId} on ${map}`);
            return;
        }

        if (typeof item === "number" && shopItem.category && Core.categoryStrings.includes(shopItem.category)) {
            // Check for house item space
            if (bot.house.freeSlots <= 0 && !bot.house.contains(item)) Core.bankACHouseItems();

            // Recheck after banking the house items that aren't equipped
            if (bot.house.freeSlots <= 0 && !bot.house.contains(item)) {
                if (log) Core.logger("❌ House full, cannot store this item.");
                return;
            }
        }

        this._buyItem(map, shopId, shopItem, quant, shopItemId, index, log);
    }

    /** C# `_BuyItem`: farms the requirements of `item`, then buys it from the shop. */
    private _buyItem(
        map: string,
        shopId: number,
        item: CoreShopItem,
        quant: number,
        shopItemId: number,
        index: number,
        log: boolean,
    ): void {
        // Quantity per purchase from shop
        const itemStack = item.quantity > 0 ? item.quantity : 1;

        // Handle requirements first
        for (const req of item.requirements) {
            const stacksNeeded = Math.ceil(quant / itemStack);
            const totalNeeded = stacksNeeded * req.quantity;

            if (Core.checkInventory(req.id, totalNeeded)) continue;

            // Special farm cases
            if (req.name.includes("Gold Voucher")) {
                Farm.voucher(req.name, totalNeeded);
                continue;
            }

            if (req.name === "Dragon Runestone") {
                Farm.dragonRunestone(totalNeeded);
                continue;
            }

            // Try to buy from shop if available
            while (!bot.shouldExit && !Core.checkInventory(req.id, totalNeeded)) {
                if (bot.map.name !== map) Core.join(map);

                const shopReqItem = Core.getShopItems(map, shopId).find((x) => x.id === req.id);
                if (!shopReqItem) {
                    Core.logger(
                        `Missing requirement: ${req.name} [${req.id}] in shop ${shopId} on map ${map}. ` +
                            "It may be a drop, daily, or special item.",
                    );
                    return;
                }

                this.buyItem(map, shopId, req.id, totalNeeded, shopReqItem.shopItemId, 0, log);
            }
        }

        // Ensure requirements are satisfied before main purchase
        this.getItemReq(item, quant);

        // Rejoin map & load shop safely
        if (bot.map.name !== map) Core.join(map);

        const shopItems = Core.getShopItems(map, shopId).filter(
            (x) => x.id === item.id && !(x.coins && x.cost > 0) && item.requirements.every((r) => Core.checkInventory(r.id, r.quantity)),
        );

        const mainItem = shopItems.length > index ? shopItems[index] : shopItems[0];
        if (!mainItem) {
            Core.logger(`❌ Failed to find ${item.name} in shop ${shopId} on map ${map}`);
            return;
        }

        // Calculate buy amount respecting stack size and max stack
        const currentStock = [...bot.inventory.items, ...bot.bank.items, ...bot.house.items, ...bot.tempInv.items]
            .filter((x) => x.id === mainItem.id)
            .reduce((sum, x) => sum + x.quantity, 0);

        const buyAmount = Core._calcBuyQuantity(mainItem, quant);
        if (buyAmount <= 0) {
            Core.logger(`Cannot buy ${mainItem.name}, max stack reached (${currentStock}/${mainItem.maxStack})`);
            return;
        }

        Core.buyItem(map, shopId, mainItem.id, buyAmount, mainItem.shopItemId !== 1 ? mainItem.shopItemId : shopItemId, index, log);

        Core.sleep();

        // Verify purchase
        if (!Core.checkInventory(mainItem.id, quant)) {
            Core.logger(`❌ Failed to buy ${mainItem.name} (${quant}x)`);

            for (const req of mainItem.requirements.filter((r) => !Core.checkInventory(r.id, r.quantity))) {
                Core.logger(`⚠️ Missing requirement: ${req.name} x${req.quantity}`);
            }
        }
    }

    /**
     * Farms what buying `item` needs (C# `GetItemReq`): the faction's reputation, the player level,
     * the gold, the Gold Vouchers and the Dragon Runestones.
     * @param item The shop item.
     * @param quant How many you want to buy.
     */
    getItemReq(item: CoreShopItem, quant = 1): void {
        // Ensure required reputation for faction-based items
        const needsRep = item.faction && item.faction !== "None" && item.requiredReputation > 0;
        if (needsRep && Farm.factionRank(item.faction) < item.requiredReputation) {
            Core.logger(`Farming reputation for ${item.faction} (Required: ${item.requiredReputation})`);
            this.runRep(item.faction, Core.pointsToLevel(item.requiredReputation));
        }

        // Level up if the item requires a higher player level
        if (item.level > bot.player.level) {
            Core.logger(`Farming experience to reach level ${item.level}`);
            Farm.experience(Math.min(item.level, 100));
        }

        // Farm gold if the item costs gold and isn't a premium currency purchase
        if (!item.coins && item.cost > 0) {
            const goldToFarm = Math.min(item.cost * quant, 100_000_000); // 100m gold cap
            Farm.gold(goldToFarm);
        }

        // Handle Gold Vouchers (multiple types possible)
        for (const req of item.requirements.filter((x) => x.name.startsWith("Gold Voucher"))) {
            Farm.voucher(req.name, req.quantity);
        }

        // Handle Dragon Runestone farming if required
        const runestoneReq = item.requirements.find((x) => x.name === "Dragon Runestone");
        if (runestoneReq) Farm.dragonRunestone(runestoneReq.quantity);

        // Warn if a temp item is missing
        for (const req of item.requirements.filter((x) => x.temp && x.quantity > bot.tempInv.getQuantity(x.id))) {
            Core.logger(`Temp item: ${req.name}, quant needed: ${req.quantity}... did the bot not farm them?`);
        }
    }

    /**
     * Farms the reputation of `faction` to `rank` with CoreFarms' `<Faction>REP` (C# `runRep`).
     * Only the reps CoreFarms ports are here.
     */
    private runRep(faction: string, rank: number): void {
        const name = faction.replace(/ /g, "");

        switch (name.toLowerCase()) {
            case "alchemy":
                Farm.alchemyREP(rank, true);
                break;
            case "good":
                Farm.goodREP(rank);
                break;
            case "vampire":
                Farm.vampireREP(rank);
                break;
            default:
                Core.logger(`Failed to find ${name}REP in CoreFarms. It isn't ported to Turu yet.`);
        }
    }

    //#region Merge

    /**
     * The merge-shop template (C# `StartBuyAllMerge`): buys every item of a merge shop the mode
     * picks, farming what each needs first. A requirement the shop sells is bought (or merged
     * itself, recursively); anything else is left to `findIngredients`, which reads
     * {@link externalItem} and {@link externalQuant} to know what to get.
     *
     * The mode comes from `buyOnlyThis` (only that item), `buyMode`, or the script's `Generic`
     * option group ({@link mergeOptions}). With `select`, the `Select` group's switches (named by
     * item ID) say which items to buy.
     * @param map Map of the shop.
     * @param shopId ID of the shop.
     * @param findIngredients Gets {@link externalItem} x {@link externalQuant}.
     * @param buyOnlyThis Buy only this item.
     * @param options `itemBlackList`: items not to buy. `buyMode`: the mode, instead of the
     *   script's options. `group`: "First" or "Last", which entry to keep when the shop lists an
     *   item twice. `log`: log the purchases.
     */
    startBuyAllMerge(
        map: string,
        shopId: number,
        findIngredients: () => void,
        buyOnlyThis: string | null = null,
        {
            itemBlackList = null,
            buyMode = null,
            group = "First",
            log = true,
        }: { itemBlackList?: string[] | null; buyMode?: mergeOptionsEnum | null; group?: "First" | "Last"; log?: boolean } = {},
    ): void {
        //#region Setup and Initialization
        // C# Get<bool>(SkipOptions) finds the option in any group; merge scripts have it in Generic.
        if (buyOnlyThis === null && buyMode === null && bot.config.has("Generic", Core.skipOptions.name) && !bot.config.get<boolean>("Generic", Core.skipOptions.name))
            bot.config.configure();

        let mode: mergeOptionsEnum = mergeOptionsEnum.all;
        if (buyOnlyThis !== null) mode = mergeOptionsEnum.all;
        else if (buyMode !== null) mode = buyMode;
        else if (bot.config.has("Generic", "mode")) mode = mergeOptionsEnum[bot.config.get<keyof typeof mergeOptionsEnum>("Generic", "mode")];
        else Core.logger("Invalid setup detected for StartBuyAllMerge. Please report", "", { messageBox: true, stopBot: true });

        this.matsOnly = mode === mergeOptionsEnum.mergeMats;

        // Set for tracking unique item IDs to prevent redundant operations
        const uniqueItemIds = new Set([...bot.bank.items, ...bot.tempInv.items, ...bot.house.items, ...bot.inventory.items].map((item) => item.id));

        // Filter shop items based on various conditions: one entry per (name, ID, ShopItemID)
        const groups = new Map<string, CoreShopItem[]>();
        for (const item of Core.getShopItems(map, shopId)) {
            const key = `${item.name}\u0000${item.id}\u0000${item.shopItemId}`;
            groups.set(key, [...(groups.get(key) ?? []), item]);
        }
        const shopItems = [...groups.values()]
            .map((entries) => (group === "First" ? entries[0] : entries[entries.length - 1]))
            .filter((x) => !x.name.toLowerCase().endsWith("insignia"))
            .filter((x) => !uniqueItemIds.has(x.id));

        const items: CoreShopItem[] = [];
        let memSkipped = false;
        const selected = (item: CoreShopItem) => bot.config.has("Select", `${item.id}`) && bot.config.get<boolean>("Select", `${item.id}`);

        // Process shop items based on various conditions
        for (const item of shopItems) {
            if (
                this.miscCatagories.includes(item.category) ||
                (buyOnlyThis && buyOnlyThis !== item.name) ||
                (itemBlackList !== null && itemBlackList.some((x) => x.toLowerCase() === item.name.toLowerCase()))
            )
                continue;

            if (Core.isMember || !item.upgrade || (item.requirements.length > 0 && Core.readShopItems().some((x) => x.upgrade && !Core.isMember))) {
                if (mode === mergeOptionsEnum.select) {
                    if (selected(item)) items.push(item);
                } else if (mode !== mergeOptionsEnum.acOnly) items.push(item);
                else if (item.coins) items.push(item);
            } else if (mode === mergeOptionsEnum.select && selected(item)) {
                Core.logger(`"${item.name}" will be skipped, as you aren't a member.`);
                memSkipped = true;
            }
        }

        if (items.length <= 0) {
            this.handleNoItemsFound(shopItems, buyOnlyThis, mode, memSkipped);
            return;
        }
        //#endregion

        const acquiredItems = new Map<number, number>();
        let t = 0;

        for (const item of items) {
            if (Core.checkInventory(item.id, 1, { toInv: false })) {
                Core.logger(`${item.name} Owned x${bot.inventory.getQuantity(item.id)}/${1}`);
                continue;
            }

            if (item.upgrade && !Core.isMember) {
                Core.logger(`Skipping ${item.name} [${item.id}] as it is member-only.`);
                continue;
            }

            if (!this.matsOnly) Core.logger(`Farming to buy ${item.name} (#${t++}/${items.length})`);

            // Process all requirements for this item
            this.processItemWithDependencies(item, 1, map, shopId, findIngredients, 0, acquiredItems);

            // After dependencies are handled, check if we can buy the main item
            this.ensureShopLoaded(map, shopId);
            if (item.requirements.every((x) => x && Core.checkInventory(x.id, x.quantity))) {
                if (!this.matsOnly) Core.logger(`Buying ${item.name} (#${t}/${items.length})`);

                this.buyItem(map, shopId, item.id, 1, item.shopItemId, 0, log);
                bot.wait.forPickup(item.id);

                if (!Core.checkInventory(item.id, item.quantity)) {
                    const missing = item.requirements.filter((x) => x && !Core.checkInventory(x.id, x.quantity)).map((x) => `"${x.name} x${x.quantity}"`);

                    Core.logger(`Failed to meet requirements for ${item.name} [${item.id}] due to missing: ${missing.join(", ")}.`);
                }
            }
        }
    }

    //#region Helper Methods

    /** Joins `map` and loads the shop until it is the loaded one. */
    private ensureShopLoaded(map: string | null, shopId: number): boolean {
        if (map === null) {
            Core.logger("Map is null, unable to load shop.");
            return false;
        }
        Core.join(map);
        bot.wait.forMapLoad(map);
        while (!bot.shouldExit && bot.shops.id !== shopId) {
            bot.shops.load(shopId);
            bot.wait.forActionCooldown("loadShop");
            bot.wait.forTrue(() => bot.shops.isLoaded && bot.shops.id === shopId, 20);
            Core.sleep(1000);
            if (bot.shops.id === shopId) return true;
        }
        return true;
    }

    /** Gets `quantity` of a shop item: its requirements first (recursively for merge items), then buys it. */
    private processItemWithDependencies(
        item: { id: number; name: string },
        quantity: number,
        map: string,
        shopId: number,
        findIngredients: () => void,
        depth = 0,
        acquiredItems = new Map<number, number>(),
    ): void {
        const MAX_DEPTH = 15;

        if (depth > MAX_DEPTH) {
            Core.logger(`Max recursion depth reached for ${item.name}`);
            return;
        }

        // If already have enough, skip
        if (Core.checkInventory(item.id, quantity)) return;

        this.ensureShopLoaded(map, shopId);

        // Look up the item in the shop to get its requirements
        const shopItem = Core.readShopItems().find((x) => x.id === item.id);
        if (!shopItem) {
            Core.logger("ShopItem Returned null.. some how", "ProcessItemWithDependencies");
            return;
        }

        // Process all requirements first
        for (const req of shopItem.requirements) {
            if (!req) continue;

            let requiredQty = req.quantity * quantity;
            Core.farmingLogger(req.name, requiredQty);
            // Check if already have it
            if (Core.checkInventory(req.id, requiredQty)) {
                Core.logger(`${req.name} Owned x${bot.inventory.getQuantity(req.id)}/${requiredQty}`);
                continue;
            }
            this.ensureShopLoaded(map, shopId);
            const reqInShop = Core.readShopItems().find((x) => x.id === req.id);

            // Check exceptions first - these should always be farmed/merged, never bought
            if (this.mergeItemisinShopExceptions.includes(req.name)) {
                Core.logger(`${req.name} [${req.id}] is in merge exceptions, using merge script.`);
                this.externalItem = req;
                this.externalQuant = requiredQty;
                Core.addDrop(this.externalItem.id);
                findIngredients();
                bot.wait.forPickup(req.id);
                acquiredItems.set(req.id, bot.inventory.getQuantity(req.id));
                continue;
            }

            if (req.name.includes("Dragon Runestone")) {
                Core.logger(`Farming Dragon Runestone x${requiredQty}`);
                Farm.dragonRunestone(requiredQty);
            }
            if (req.name.includes("Gold Voucher")) {
                Core.logger(`Farming Gold Voucher x${requiredQty}`);
                Farm.voucher(req.name, requiredQty);
            }
            if (reqInShop) {
                Core.logger(`Requirement "${reqInShop.name}" [${reqInShop.id}] is in shop.`, "ProcessItemWithDependencies");
                requiredQty = Math.min(requiredQty, reqInShop.maxStack);

                // If requirement has no dependencies, buy it directly
                if (reqInShop.requirements.length === 0 && ((reqInShop.coins && reqInShop.cost <= 0) || !reqInShop.coins)) {
                    if (reqInShop.upgrade && !bot.player.isMember) {
                        Core.logger(`${reqInShop.name} is MEMBERS ONLY, you are not.... we can't buy it >.>`);
                        return;
                    }
                    this.buyItem(map, shopId, reqInShop.id, requiredQty, reqInShop.shopItemId);
                    bot.wait.forPickup(reqInShop.id);
                } else if (reqInShop.requirements.length > 0) {
                    // Recurse for nested requirements
                    this.processItemWithDependencies(reqInShop, requiredQty, map, shopId, findIngredients, depth + 1, acquiredItems);
                    bot.wait.forPickup(req.id);
                }
            } else {
                // Item not in current shop - must be farmed/crafted externally
                this.externalItem = req;
                this.externalQuant = requiredQty;
                Core.addDrop(this.externalItem.id);
                Core.logger(`${this.externalItem.name} [${this.externalItem.id}] not in shop, using merge script.`);
                findIngredients();
                bot.wait.forPickup(req.id);
            }

            // Verify we got it
            if (!Core.checkInventory(req.id, requiredQty)) Core.logger(`Warning: Failed to acquire ${req.name} x${requiredQty}.`);
        }

        this.ensureShopLoaded(map, shopId);
        if (shopItem.requirements.every((x) => Core.checkInventory(x.id, x.quantity))) this.buyItem(map, shopId, shopItem.id, quantity, shopItem.shopItemId);
    }

    private handleNoItemsFound(shopItems: CoreShopItem[], buyOnlyThis: string | null, mode: mergeOptionsEnum, memSkipped: boolean): void {
        if (buyOnlyThis !== null) return;

        switch (mode) {
            case mergeOptionsEnum.all:
            case mergeOptionsEnum.mergeMats:
                Core.logger("The bot fetched 0 items to farm. Something must have gone wrong.");
                break;
            case mergeOptionsEnum.acOnly:
                if (shopItems.every((x) => !x.coins)) Core.logger("The bot fetched 0 items to farm. This is because none of the items in this shop are AC tagged.");
                else
                    Core.logger(
                        "The bot fetched 0 items to farm. Something must have gone wrong. Try re-checking your options, if you picked specificly items, then you'll want stop the script (if it isnt) and change the `Mode` to `Select`, and rerun this script",
                    );
                break;
            case mergeOptionsEnum.select:
                if (memSkipped) Core.logger("The bot fetched 0 items to farm. This is because you aren't a member.");
                else
                    Core.logger(
                        "The bot fetched 0 items to farm. Something must have gone wrong. Try re-checking your options, if you picked specificly items, then you'll want stop the script (if it isnt) and change the `Mode` to `Select`, and rerun this script",
                    );
                break;
        }
    }

    //#endregion

    /**
     * Items the shop sells that should still come from `findIngredients` (else the template loops
     * buying them). Add to it in the merge script, before `startBuyAllMerge`.
     */
    mergeItemisinShopExceptions: string[] = [];

    /** Shop categories the template never buys (C# `ItemCategory` Note, Item, QuestItem, ServerUse). */
    miscCatagories: string[] = ["Note", "Item", "Quest Item", "ServerUse"];
    /** The item `findIngredients` should get. */
    externalItem: ShopRequirement = { id: 0, name: "", quantity: 0, temp: false };
    /** How many of {@link externalItem} `findIngredients` should get. */
    externalQuant = 0;
    /** The mode is `mergeMats`: farm the materials, don't buy the items. */
    matsOnly = false;
    maxStackOneItems: string[] = [];
    altFarmItems: string[] = [];

    /**
     * The `Generic` option group of every merge script (C# `MergeOptions`):
     * `defineScriptOptions(Adv.optionsStorage, [], true, { Generic: Adv.mergeOptions, Select: [...] })`.
     * C# names the four explanation lines " "; they have their own names here, as option names
     * must be unique (they're labels, never read).
     */
    readonly mergeOptions: ScriptOption[] = [
        Core.skipOptions,
        option.choice(
            "mode",
            "Select the mode to use",
            "Regardless of the mode you pick, the bot wont (attempt to) buy Legend-only items if you're not a Legend.\n" +
                "Select the Mode Explanation item to get more information",
            choices(mergeOptionsEnum),
            "all",
        ),
        option.text("modeExplanationAll", "Mode Explanation [all]", "Mode [all]: \t\tYou get all the items from shop, even if non-AC ones if any.", "click here"),
        option.text("modeExplanationAcOnly", "Mode Explanation [acOnly]", "Mode [acOnly]: \tYou get all the AC tagged items from the shop.", "click here"),
        option.text(
            "modeExplanationMergeMats",
            "Mode Explanation [mergeMats]",
            "Mode [mergeMats]: \tYou dont buy any items but instead get the materials to buy them yourself, this way you can choose.",
            "click here",
        ),
        option.text(
            "modeExplanationSelect",
            "Mode Explanation [select]",
            "Mode [select]: \tYou are able to select what items you get and which ones you dont in the Select Category below.",
            "click here",
        ),
    ];

    /** The options file of every merge script (C# `OptionsStorage`). */
    readonly optionsStorage = "MergeOptionStorage";

    //#endregion
}

/** The modes of {@link CoreAdvanced.startBuyAllMerge} (C# `mergeOptionsEnum`). */
export enum mergeOptionsEnum {
    all = 0,
    acOnly = 1,
    mergeMats = 2,
    select = 3,
}
