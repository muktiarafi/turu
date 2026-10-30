/*
name: CoreAdvanced
description: Port of the shop, gear and enhancement parts of Scripts/CoreAdvanced.cs: buys an item
    after farming what the purchase needs, the merge-shop template, enhancing gear (EnhanceItem,
    SmartEnhance), storing gear and ranking up classes.
tags: core
*/
/**
 * CoreAdvanced — partial TypeScript port of `Scripts/CoreAdvanced.cs`.
 *
 * {@link CoreAdvanced.buyItem} is C# `BuyItem`: unlike CoreBots' `buyItem`, which gives up when a
 * requirement is missing, it farms the reputation, level, gold, Gold Vouchers and Dragon Runestones
 * the item needs first. {@link CoreAdvanced.startBuyAllMerge} is the merge-shop template the merge
 * scripts use. The Gear region ({@link CoreAdvanced.rankUpClass}, {@link CoreAdvanced.gearStore},
 * {@link CoreAdvanced.hasMinimalBoost}) and the Enhancement and SmartEnhance regions
 * ({@link CoreAdvanced.enhanceItem}, {@link CoreAdvanced.enhanceEquipped},
 * {@link CoreAdvanced.smartEnhance}) follow C#; C#'s Kill region isn't ported yet.
 */

import { bot, BoostType, choices, option } from "turu";
import type { InventoryItem, ScriptOption } from "turu";
import { Core, type CoreShopItem, rethrowIfStopped, type ShopRequirement } from "./CoreBots";
import { CoreFarms } from "./CoreFarms";

const Farm = new CoreFarms();

/** Options of {@link CoreAdvanced.enhanceItem} and {@link CoreAdvanced.enhanceEquipped}. */
export interface EnhanceOptions {
    /** The cape special (C# `cSpecial`). */
    cSpecial?: CapeSpecial;
    /** The helm special (C# `hSpecial`). */
    hSpecial?: HelmSpecial;
    /** The weapon special (C# `wSpecial`). */
    wSpecial?: WeaponSpecial;
    /** Enhance even when CoreBots Options' DisableAutoEnhance is on. */
    bypassDisableAutoEnhance?: boolean;
}

/** C# `EnhanceableCatagories`: the item categories that can be enhanced. */
const enhanceableCatagories: readonly string[] = [
    "Sword",
    "Axe",
    "Dagger",
    "Gun",
    "HandGun",
    "Rifle",
    "Bow",
    "Mace",
    "Gauntlet",
    "Polearm",
    "Staff",
    "Wand",
    "Whip",
    "Class",
    "Helm",
    "Cape",
];

/** C#'s `Category == ItemCategory.X` (the enum is parsed ignoring case). */
function isCategory(item: { category: string }, category: string): boolean {
    return item.category.toLowerCase() === category.toLowerCase();
}

function isEnhanceable(item: InventoryItem): boolean {
    return enhanceableCatagories.some((c) => isCategory(item, c));
}

/** C# `Enum.IsDefined` for a numeric enum. */
function isDefined(enumObject: Record<string | number, string | number>, value: number): boolean {
    return typeof enumObject[value] === "string";
}

/** C# `enumValue.ToString()`: the member's name, or the number when it has none. */
function enumName(enumObject: Record<string | number, string | number>, value: number): string {
    const name = enumObject[value];
    return typeof name === "string" ? name : `${value}`;
}

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

    //#region Gear

    /**
     * Ranks a class up to rank 10 in Icestorm Arena with the class boost (C# `RankUpClass`):
     * unbanks it, enhances it with {@link smartEnhance}, equips it, farms, then puts your gear back.
     * @param className The class's name, its ID, or "(Current)" for the equipped class.
     * @param options `gearRestore` (default true): store your gear first and restore it after.
     *   `itemId`: the class's ID, when `className` is a name shared by several items (C#'s
     *   `RankUpClass(name, itemid)`).
     */
    rankUpClass(className: string | number, { gearRestore = true, itemId = 0 }: { gearRestore?: boolean; itemId?: number } = {}): void {
        if (typeof className === "number") this.rankUpClassInternal(null, className, gearRestore);
        else if (itemId > 0) this.rankUpClassInternal(null, itemId, gearRestore);
        else this.rankUpClassInternal(className, null, gearRestore);
    }

    private rankUpClassInternal(className: string | null, itemId: number | null, gearRestore: boolean): void {
        if (className === "(Current)") {
            const currentName = bot.player.currentClass;
            if (!currentName) {
                Core.logger("CurrentClass is null or has no name. Cannot rank up.");
                return;
            }
            className = currentName;
        }

        const classMatch = (i: InventoryItem) => isCategory(i, "Class") && (itemId !== null ? i.id === itemId : i.name.toLowerCase() === className!.toLowerCase());

        let itemInv = [...bot.inventory.items, ...bot.bank.items].find((i) => i && classMatch(i));

        if (!itemInv) {
            Core.logger(itemId !== null ? `Can't level up item ID ${itemId} because you don't own it.` : `Can't level up "${className}" because you don't own it.`);
            return;
        }

        if (bot.bank.contains(itemInv.id) && !bot.inventory.contains(itemInv.id)) {
            Core.unbank(itemInv.id);
            Core.sleep();

            itemInv = bot.inventory.items.find((i) => i && classMatch(i));
            if (!itemInv) {
                Core.logger("Failed to unbank class item.");
                return;
            }
        }

        if (itemInv.upgrade && !bot.player.isMember) {
            Core.logger(`"${itemInv.name}" requires membership to rank up.`);
            return;
        }

        if (itemInv.quantity >= 302500) {
            Core.logger(`"${itemInv.name}" is already Rank 10`);
            return;
        }

        if (["Hobo Highlord", "No Class", "Obsidian No Class"].includes(itemInv.name)) {
            Core.logger(`"${itemInv.name}" cannot be leveled past Rank 1`);
            return;
        }

        if (gearRestore) this.gearStore();

        Core.jumpWait();

        this.smartEnhance(itemInv.name);

        let classItem = bot.inventory.items.find((i) => i && classMatch(i));

        if (!classItem) {
            Core.logger("Class item not found in inventory after enhance.");
            return;
        }

        if (classItem.enhancementLevel <= 0) {
            Core.logger(`Can't level up "${classItem.name}" because it's not enhanced.`);
            return;
        }

        if (!bot.inventory.isEquipped(classItem.id)) {
            Core.equip(classItem.id);
            const id = classItem.id;
            bot.wait.forTrue(() => bot.inventory.isEquipped(id), 20);
        }

        Farm.toggleBoost(BoostType.Class);
        Farm.icestormArena(bot.player.level, true);
        Core.jump("Enter");
        bot.options.aggroMonsters = false;

        classItem = bot.inventory.items.find((i) => i && classMatch(i));
        if (!classItem) {
            Core.logger("Class item missing after arena.");
            return;
        }

        Core.logger(classItem.quantity >= 302500 ? `"${classItem.name}" is now Rank 10` : `"${classItem.name}" is somehow... not rank 10??`);

        Farm.toggleBoost(BoostType.Class, false);

        if (gearRestore) this.gearStore(true);
    }

    /**
     * Stores the gear you have equipped and its enhancements, or puts it back on (C# `GearStore`).
     * @param restore Restore the stored gear instead of storing it.
     * @param options `enhAfter`: reapply the stored enhancements after restoring.
     */
    gearStore(restore = false, { enhAfter = false }: { enhAfter?: boolean } = {}): void {
        if (!restore) {
            this.reEquippedItems.length = 0;

            const equippedItems = bot.inventory.items.filter((i) => i.equipped);

            for (const item of equippedItems) this.reEquippedItems.push(item.name);

            this.reEnhanceAfter = this.currentClassEnh();
            this.reWEnhanceAfter = this.currentWeaponSpecial();

            this.reCEnhanceAfter = equippedItems.some((i) => isCategory(i, "Cape")) ? this.currentCapeSpecial() : CapeSpecial.None;

            this.reHEnhanceAfter = equippedItems.some((i) => isCategory(i, "Helm")) ? this.currentHelmSpecial() : HelmSpecial.None;

            // ---- Store summary ----
            Core.logger("GearStore: Saved current equipment state");
            Core.logger(` - Items: ${this.reEquippedItems.join(", ")}`);
            Core.logger(` - Class Enh: ${enumName(EnhancementType, this.reEnhanceAfter)}`);

            if (this.reCEnhanceAfter !== CapeSpecial.None) Core.logger(` - Cape Special: ${enumName(CapeSpecial, this.reCEnhanceAfter)}`);

            if (this.reHEnhanceAfter !== HelmSpecial.None) Core.logger(` - Helm Special: ${enumName(HelmSpecial, this.reHEnhanceAfter)}`);

            if (this.reWEnhanceAfter !== WeaponSpecial.None) Core.logger(` - Weapon Special: ${enumName(WeaponSpecial, this.reWEnhanceAfter)}`);
        } else if (this.reEquippedItems.length > 0) {
            // ---- Restore summary ----
            Core.logger("GearStore: Restoring saved equipment state");
            Core.logger(` - Items: ${this.reEquippedItems.join(", ")}`);

            if (enhAfter) {
                Core.logger(
                    ` - Enhancements → Class: ${enumName(EnhancementType, this.reEnhanceAfter)}` +
                        `${this.reCEnhanceAfter !== CapeSpecial.None ? `, Cape: ${enumName(CapeSpecial, this.reCEnhanceAfter)}` : ""}` +
                        `${this.reHEnhanceAfter !== HelmSpecial.None ? `, Helm: ${enumName(HelmSpecial, this.reHEnhanceAfter)}` : ""}` +
                        `${this.reWEnhanceAfter !== WeaponSpecial.None ? `, Weapon: ${enumName(WeaponSpecial, this.reWEnhanceAfter)}` : ""}`,
                );
            }

            Core.jumpWait();
            Core.equip(...this.reEquippedItems);

            if (enhAfter)
                this.enhanceEquipped(this.reEnhanceAfter, {
                    cSpecial: this.reCEnhanceAfter,
                    hSpecial: this.reHEnhanceAfter,
                    wSpecial: this.reWEnhanceAfter,
                });
        }
    }

    private readonly reEquippedItems: string[] = [];
    private reEnhanceAfter: EnhancementType = EnhancementType.Lucky;
    private reCEnhanceAfter: CapeSpecial = CapeSpecial.None;
    private reHEnhanceAfter: HelmSpecial = HelmSpecial.None;
    private reWEnhanceAfter: WeaponSpecial = WeaponSpecial.None;

    /** True when the item is a weapon (C# `isWeapon`: its `ItemGroup`, `equipSlot` here). */
    isWeapon(item: InventoryItem): boolean {
        return item.equipSlot === "Weapon";
    }

    /**
     * C# `_RaceGear`: equipped the gear with the best boost against the monster's race. It is
     * disabled in C# (the body is commented out) and only logs that.
     * @param _monster The monster's name or ID.
     */
    _raceGear(_monster: string | number): void {
        Core.logger("BestGear Disabled");
    }

    /**
     * True when an item in your inventory or bank boosts `boostType` by at least `percentage`
     * percent (C# `HasMinimalBoost`).
     * @param boostType A generic boost (gold, exp, ...) or a race.
     * @param percentage The boost in percent (25 = a 1.25 boost).
     */
    hasMinimalBoost(boostType: GenericGearBoost | RacialGearBoost, percentage: number): boolean {
        return [...bot.inventory.items, ...bot.bank.items].some((x) => Core.getBoostFloat(x, boostType) >= percentage / 100 + 1);
    }

    //#endregion

    //#region Enhancement

    /**
     * Enhances your equipped gear (C# `EnhanceEquipped`): the class, helm, cape and weapon get
     * `type`; the cape, helm and weapon get their special when one is given and unlocked.
     * Does nothing when CoreBots Options' DisableAutoEnhance is on, unless `bypassDisableAutoEnhance`.
     * @param type The enhancement type.
     * @param options The cape, helm and weapon specials, and `bypassDisableAutoEnhance`.
     */
    enhanceEquipped(type: EnhancementType, { cSpecial = CapeSpecial.None, hSpecial = HelmSpecial.None, wSpecial = WeaponSpecial.None, bypassDisableAutoEnhance = false }: EnhanceOptions = {}): void {
        if (!bypassDisableAutoEnhance) {
            if (Core.cboBool("DisableAutoEnhance") === true) return;
        }

        const equippedItems = bot.inventory.items.filter((i) => i.equipped === true && isEnhanceable(i));
        try {
            this.autoEnhance(equippedItems, type, cSpecial, hSpecial, wSpecial);
        } catch (e) {
            this.advCrash(e, "EnhanceEquipped");
        }
    }

    /**
     * Enhances one item, or several (C# `EnhanceItem`, both overloads). Several items must all be
     * owned and enhanceable, else nothing is enhanced. Does nothing when CoreBots Options'
     * DisableAutoEnhance is on, unless `bypassDisableAutoEnhance`.
     * @param item The item's name, or a list of names (exact case for a list, as in C#).
     * @param type The enhancement type.
     * @param options The cape, helm and weapon specials, `logging` (unused, as in C#) and
     *   `bypassDisableAutoEnhance`.
     */
    enhanceItem(
        item: string | string[],
        type: EnhancementType,
        { cSpecial = CapeSpecial.None, hSpecial = HelmSpecial.None, wSpecial = WeaponSpecial.None, bypassDisableAutoEnhance = false }: EnhanceOptions & { logging?: boolean } = {},
    ): void {
        if (Array.isArray(item)) {
            this.enhanceItems(item, type, cSpecial, hSpecial, wSpecial, bypassDisableAutoEnhance);
            return;
        }

        if (!item) return;

        if (!bypassDisableAutoEnhance) {
            if (Core.cboBool("DisableAutoEnhance") === true) return;
        }

        if (!Core.checkInventory(item)) {
            Core.logger(`Enhancement Failed: Could not find "${item}"`);
            return;
        }

        while (!bot.shouldExit && bot.player.inCombat) {
            if (bot.player.hasTarget) bot.combat.cancelTarget();
            Core.jumpWait();
            Core.sleep();
        }

        const selectedItem = bot.inventory.items.find((i) => i.name.toLowerCase().trim() === item.toLowerCase().trim() && isEnhanceable(i));
        if (!selectedItem) {
            if (bot.inventory.items.some((i) => i.name.toLowerCase().trim() === item.toLowerCase().trim())) Core.logger(`Enhancement Failed: ${item} cannot be enhanced`);
            return;
        }

        try {
            // C# doesn't pass `logging` on.
            this.autoEnhance([selectedItem], type, cSpecial, hSpecial, wSpecial);
        } catch (e) {
            this.advCrash(e, "EnhanceItem");
        }
    }

    /** C# `EnhanceItem(string[] items, ...)`. */
    private enhanceItems(items: string[], type: EnhancementType, cSpecial: CapeSpecial, hSpecial: HelmSpecial, wSpecial: WeaponSpecial, bypassDisableAutoEnhance: boolean): void {
        if (items.length === 0) return;

        if (!bypassDisableAutoEnhance) {
            if (Core.cboBool("DisableAutoEnhance") === true) return;
        }

        // If any of the items in the items array cant be found, return
        const notFound: string[] = [];
        for (const item of items) if (!Core.checkInventory(item)) notFound.push(item);

        if (notFound.length > 0) {
            if (notFound.length === 1) Core.logger(`Enhancement Failed: Could not find ${notFound[0]}`);
            else Core.logger(`Enhancement Failed: Could not find the following items: ${notFound.join(", ")}`);
            return;
        }

        // Find all the items in the items array
        const selectedItems = bot.inventory.items.filter((i) => items.includes(i.name) && isEnhanceable(i));

        // If any of the items in the items array cant be enhanced, return
        if (selectedItems.length !== items.length) {
            const unEnhanceable: string[] = [];

            for (const item of items) if (!bot.inventory.items.some((i) => i.name === item && isEnhanceable(i))) unEnhanceable.push(item);

            if (unEnhanceable.length === 1) Core.logger(`Enhancement Failed: Unenhanceable item found, ${unEnhanceable[0]}`);
            else Core.logger(`Enhancement Failed: The following items are unenhanceable, ${unEnhanceable.join(", ")}`);

            return;
        }

        try {
            this.autoEnhance(selectedItems, type, cSpecial, hSpecial, wSpecial);
        } catch (e) {
            this.advCrash(e, "EnhanceItem");
        }
    }

    // Unused, as in C#.
    private static isEnhancedWithBaseForge(item: InventoryItem): boolean {
        return item.enhancementPatternId === 0 && item.enhancementLevel > 0;
    }

    /**
     * C# `AdvCrash`: reports an enhancement that threw and lets the script go on without it. C#
     * shows a message box with the last logs; Turu logs the report.
     */
    private advCrash(e: unknown, caller: string): void {
        rethrowIfStopped(e);
        if (e === null || e === undefined) return;

        const message = e instanceof Error ? e.message : String(e);
        const severity = e instanceof TypeError || e instanceof ReferenceError ? "❗ Major" : e instanceof RangeError || e instanceof SyntaxError ? "⚠️ Minor" : "🔥 Critical";
        const stack = e instanceof Error && e.stack ? e.stack.split("\n").slice(0, 5).join("\n") : "No line info.";

        Core.logger(
            `🛑 ${caller} Crash Report | ${severity}\n` +
                `⚠️ Script will continue without \`${caller}\`.\n` +
                `🔥 Message: ${message}\n📍 ${stack}`,
            `${caller} crashed`,
        );
    }

    /**
     * The enhancement type of your equipped class (C# `CurrentClassEnh`): Lucky when it has none
     * (Adventurer and Depths count as Lucky).
     */
    currentClassEnh(): EnhancementType {
        let patternId = bot.inventory.items.find((i) => i.equipped && isCategory(i, "Class"))?.enhancementPatternId ?? 9;

        if (patternId === 1 || patternId === 23) patternId = 9;

        return isDefined(EnhancementType, patternId) ? (patternId as EnhancementType) : EnhancementType.Lucky;
    }

    /** The special of your equipped cape, or None (C# `CurrentCapeSpecial`). */
    currentCapeSpecial(): CapeSpecial {
        const equippedCape = bot.inventory.items.find((i) => i.equipped && isCategory(i, "Cape"));
        if (!equippedCape) return CapeSpecial.None;
        const patternId = equippedCape.enhancementPatternId;
        if (isDefined(EnhancementType, patternId)) return CapeSpecial.None;
        return patternId as CapeSpecial;
    }

    /** The special of your equipped helm, or None (C# `CurrentHelmSpecial`). */
    currentHelmSpecial(): HelmSpecial {
        const equippedHelm = bot.inventory.items.find((i) => i.equipped && isCategory(i, "Helm"));
        if (!equippedHelm) return HelmSpecial.None;
        const patternId = equippedHelm.enhancementPatternId;

        if (isDefined(EnhancementType, patternId)) return HelmSpecial.None;
        return patternId as HelmSpecial;
    }

    /**
     * The special of your equipped weapon, or None (C# `CurrentWeaponSpecial`). Like C#, this
     * reads the weapon's enhancement pattern, not its proc.
     */
    currentWeaponSpecial(): WeaponSpecial {
        const equippedWeapon = bot.inventory.items.find((i) => i.equipped && this.weaponCatagories.some((c) => isCategory(i, c)));
        if (!equippedWeapon) return WeaponSpecial.None;
        const patternId = equippedWeapon.enhancementPatternId;

        if (isDefined(EnhancementType, patternId)) return WeaponSpecial.None;
        return patternId as WeaponSpecial;
    }

    /** The weapon categories (C# `WeaponCatagories`: the first 12 enhanceable ones, so not Whip). */
    readonly weaponCatagories: readonly string[] = enhanceableCatagories.slice(0, 12);

    private alreadyHasRequestedEnhancement(item: InventoryItem, type: EnhancementType, cSpecial: CapeSpecial, hSpecial: HelmSpecial, wSpecial: WeaponSpecial, _logging = false): boolean {
        if (!item || item.enhancementLevel <= 0 || item.enhancementLevel !== bot.player.level) return false;

        if (isCategory(item, "Cape")) {
            if (cSpecial !== CapeSpecial.None) return cSpecial === item.enhancementPatternId;
            return type === item.enhancementPatternId;
        }

        if (isCategory(item, "Helm")) {
            if (hSpecial !== HelmSpecial.None) return hSpecial === item.enhancementPatternId;
            return type === item.enhancementPatternId;
        }

        if (item.equipSlot === "Weapon") {
            if (wSpecial === WeaponSpecial.None) return type === item.enhancementPatternId;
            return this.weaponAlreadyHasRequestedEnhancement(item, type, wSpecial);
        }

        if (isCategory(item, "Class")) return type === item.enhancementPatternId;

        return false;
    }

    private weaponAlreadyHasRequestedEnhancement(item: InventoryItem, type: EnhancementType, wSpecial: WeaponSpecial): boolean {
        if (!item || item.equipSlot !== "Weapon" || item.enhancementLevel <= 0) return false;

        const currentPattern = item.enhancementPatternId;
        const currentProc = this.getProcID(item);

        if (wSpecial === WeaponSpecial.None) return currentPattern === type;

        // Forge (1) falls in this range too, as in C#.
        if (wSpecial > 0 && wSpecial <= 6) return currentPattern === type && currentProc === wSpecial;

        if (wSpecial === WeaponSpecial.Forge) return currentProc === 0 && (currentPattern === type || currentPattern === 10);

        return currentProc === wSpecial;
    }

    private autoEnhance(itemList: InventoryItem[], type: EnhancementType, cSpecial: CapeSpecial, hSpecial: HelmSpecial, wSpecial: WeaponSpecial, logging = false): void {
        // In case the 'CurrentEnhancement()' failed and returned 0
        if ((type as number) === 0) return;

        // Empty check
        if (itemList.length === 0) {
            Core.logger('Enhancement Failed:\t"ItemList" is empty');
            return;
        }
        itemList = [...itemList];
        const remove = (item: InventoryItem) => itemList.splice(itemList.indexOf(item), 1);

        // Defining cape
        let cape: InventoryItem | undefined;
        if (cSpecial !== CapeSpecial.None && itemList.some((i) => isCategory(i, "Cape"))) {
            cape = itemList.find((i) => isCategory(i, "Cape"));

            // Removing cape from the list because it needs to be enhanced seperately
            if (cape) remove(cape);
        }

        // Defining helm
        let helm: InventoryItem | undefined;
        if (hSpecial !== HelmSpecial.None && itemList.some((i) => isCategory(i, "Helm"))) {
            helm = itemList.find((i) => isCategory(i, "Helm"));

            // Removing helm from the list because it needs to be enhanced seperately
            if (helm) remove(helm);
        }

        // Defining weapon
        let weapon: InventoryItem | undefined;
        // If Awe-Enhancements aren't unlocked, enhance them with normal enhancements
        if (wSpecial !== WeaponSpecial.None && itemList.some((i) => i.equipSlot === "Weapon") && (this.uAwe() || wSpecial === WeaponSpecial.Forge || wSpecial > 6)) {
            weapon = itemList.find((i) => i.equipSlot === "Weapon");

            // Removing weapon from the list because it needs to be enhanced seperately
            if (weapon) remove(weapon);
        }

        let skipCounter = 0;
        const typeName = enumName(EnhancementType, type);
        const cName = enumName(CapeSpecial, cSpecial);
        const hName = enumName(HelmSpecial, hSpecial);
        const wName = enumName(WeaponSpecial, wSpecial);

        const _autoEnhance = (item: InventoryItem, shopID: number, map: string | null = null, logging = false): void => {
            const specialOnCape = isCategory(item, "Cape") && cSpecial !== CapeSpecial.None;
            const specialOnHelm = isCategory(item, "Helm") && hSpecial !== HelmSpecial.None;
            const specialOnWeapon = item.equipSlot === "Weapon" && wName !== "None";
            const mapName = map ?? (bot.map.name || "whitemap");
            const shopItems = Core.getShopItems(mapName, shopID);

            // Shopdata complete check
            if (!shopItems.some((x) => x.category.toLowerCase() === "enhancement") || shopItems.length === 0) {
                Core.logger(
                    `Enhancement Failed for ${item.name}[${item.id}], (EnhancementLevel: ${item.enhancementLevel}, map: ${mapName}, shopID: ${shopID}):\n` +
                        `Couldn't find enhancements in shop ${shopID}`,
                );
                return;
            }

            // Checking if the item is already optimally enhanced
            if (bot.player.level === item.enhancementLevel) {
                if (specialOnCape) {
                    if (cSpecial === item.enhancementPatternId) {
                        skipCounter++;
                        return;
                    }
                } else if (specialOnHelm) {
                    if (hSpecial === item.enhancementPatternId) {
                        skipCounter++;
                        return;
                    }
                } else if (specialOnWeapon) {
                    if ((wSpecial > 0 && wSpecial <= 6 ? type : 10) === item.enhancementPatternId && wSpecial === this.getProcID(item)) {
                        skipCounter++;
                        return;
                    }
                } else if (type === item.enhancementPatternId) {
                    skipCounter++;
                    return;
                }
            }

            // Logging
            if (logging) {
                if (specialOnCape) Core.logger(`Searching Enhancement:\tForge/${cName.replace(/_/g, " ")} - "${item.name}"`);
                else if (specialOnWeapon)
                    Core.logger(`Searching Enhancement:\t${wSpecial > 0 && wSpecial <= 6 ? typeName : "Forge"}/${wName.replace(/_/g, " ")} - "${item.name}"`);
                else Core.logger(`Searching Enhancement:\t${typeName} - "${item.name}"`);
            }

            const availableEnh: CoreShopItem[] = [];

            // Filters
            for (const enh of shopItems) {
                // Remove enhancments that you dont have access to
                if ((!bot.player.isMember && enh.upgrade) || enh.level > bot.player.level) continue;

                const enhName = enh.name.replace(/ /g, "").replace(/'/g, "").toLowerCase();

                // Cape if cSpecial
                if (specialOnCape && enhName.includes(cName.replace(/_/g, "").toLowerCase())) availableEnh.push(enh);
                // Weapon if wSpecial
                else if (specialOnWeapon && enhName.includes(wName.replace(/_/g, "").toLowerCase())) availableEnh.push(enh);
                //Helm if hSpecial
                else if (specialOnHelm && enhName.includes(hName.replace(/_/g, "").toLowerCase())) availableEnh.push(enh);
                // Class
                else if (isCategory(item, "Class") && enhName.includes("armor")) availableEnh.push(enh);
                // Helm
                else if (isCategory(item, "Helm") && enhName.includes("helm")) availableEnh.push(enh);
                // Cape if not cSpecial
                else if (isCategory(item, "Cape") && enhName.includes("cape")) availableEnh.push(enh);
                // Weapon2 if not wSpecial
                else if (item.equipSlot === "Weapon" && enhName.includes("weapon")) availableEnh.push(enh);
            }

            // Empty check
            let bestEnhancement: CoreShopItem | null = null;
            if (availableEnh.length === 0) {
                if (logging) Core.logger('Enhancement Failed:\t"availableEnh" is empty');
                return;
            } else if (availableEnh.length === 1) bestEnhancement = availableEnh[0];
            else {
                // Sorting by level (descending)
                const sortedList = [...availableEnh].sort((a, b) => b.level - a.level || (b.upgrade ? 1 : 0) - (a.upgrade ? 1 : 0));
                bestEnhancement = sortedList[0];
            }

            // Null check
            if (!bestEnhancement) {
                if (logging) Core.logger(`Enhancement Failed:\tCould not find the best enhancement for "${item.name}"`);
                return;
            }

            // Compare with current enhancement
            if (bestEnhancement.id === this.getEnhID(item) && item.enhancementLevel > 0 && bestEnhancement.level === item.enhancementLevel) {
                if (logging) Core.logger(`Enhancement Canceled:\tBest enhancement is already applied for "${item.name}"`);
                return;
            }

            // Enhancing the item
            const roomId = bot.map.roomId;

            bot.send.packet(`%xt%zm%enhanceItemShop%${roomId}%${item.id}%${bestEnhancement.id}%${shopID}%`);

            // Final logging
            if (specialOnCape) {
                if (logging) Core.logger(`Enhancement Applied:\tForge/${cName.replace(/_/g, " ")} - "${item.name}" (Lvl ${bestEnhancement.level})`);
            } else if (specialOnWeapon) {
                if (logging)
                    Core.logger(
                        `Enhancement Applied:\t${wSpecial > 0 && wSpecial <= 6 ? typeName : "Forge"}/${wName.replace(/_/g, " ")} - "${item.name}" (Lvl ${bestEnhancement.level})`,
                    );
            } else {
                if (logging) Core.logger(`Enhancement Applied:\t${typeName} - "${item.name}" (Lvl ${bestEnhancement.level})`);
            }
            Core.sleep();
        };

        // Setting the shop ID for the enhancement type
        if (itemList.length > 0) {
            let shopID = 0;

            switch (type) {
                case EnhancementType.Fighter:
                    shopID = bot.player.level >= 50 ? 768 : 141;
                    break;
                case EnhancementType.Thief:
                    shopID = bot.player.level >= 50 ? 767 : 142;
                    break;
                case EnhancementType.Hybrid:
                    shopID = bot.player.level >= 50 ? 766 : 143;
                    break;
                case EnhancementType.Wizard:
                    shopID = bot.player.level >= 50 ? 765 : 144;
                    break;
                case EnhancementType.Healer:
                    shopID = bot.player.level >= 50 ? 762 : 145;
                    break;
                case EnhancementType.SpellBreaker:
                    shopID = bot.player.level >= 50 ? 764 : 146;
                    break;
                case EnhancementType.Lucky:
                    shopID = bot.player.level >= 50 ? 763 : 147;
                    break;
                default:
                    Core.logger(`Enhancement Failed:\tInvalid EnhancementType given, received ${type} | ${typeName}`);
                    return;
            }

            // Enhancing the remaining items
            for (const item of itemList) {
                if (this.alreadyHasRequestedEnhancement(item, type, cSpecial, hSpecial, wSpecial, logging)) {
                    skipCounter++;
                    continue;
                }
                _autoEnhance(item, shopID, bot.map.name, logging);
                Core.sleep();
            }
        }

        // Enhancing the cape with the cape special
        if (cape) {
            let canEnhance = true;
            const fail = () => {
                Core.logger(`Enhancement Failed:\tYou did not unlock the ${cName} Enhancement yet`);
                canEnhance = false;
            };

            switch (cSpecial) {
                case CapeSpecial.Forge:
                    if (!this.uForgeCape()) {
                        Core.logger("Enhancement Failed:\tYou did not unlock the Forge (Cape) Enhancement yet");
                        canEnhance = false;
                    }
                    break;
                case CapeSpecial.Absolution:
                    if (!this.uAbsolution()) fail();
                    break;
                case CapeSpecial.Avarice:
                    if (!this.uAvarice()) fail();
                    break;
                case CapeSpecial.Vainglory:
                    if (!this.uVainglory()) fail();
                    break;
                case CapeSpecial.Penitence:
                    if (!this.uPenitence()) fail();
                    break;
                case CapeSpecial.Lament:
                    if (!this.uLament()) fail();
                    break;
                default:
                    Core.logger(`Enhancement Failed:\tInvalid "CapeSpecial" given, received ${cSpecial} | ${cName}`);
                    return;
            }

            if (this.alreadyHasRequestedEnhancement(cape, type, cSpecial, hSpecial, wSpecial, logging)) skipCounter++;
            else if (canEnhance) _autoEnhance(cape, 2143, cSpecial > 0 ? "forge" : null, logging);
            else skipCounter++;
        }

        // Enhancing the helm with the helm special
        if (helm) {
            let canEnhance = true;
            const fail = () => {
                Core.logger(`Enhancement Failed:\tYou did not unlock the ${hName} Enhancement yet`);
                canEnhance = false;
            };

            switch (hSpecial) {
                case HelmSpecial.Vim:
                    if (!this.uVim()) fail();
                    break;
                case HelmSpecial.Examen:
                    if (!this.uExamen()) fail();
                    break;
                case HelmSpecial.Forge:
                    if (!this.uForgeHelm()) fail();
                    break;
                case HelmSpecial.Anima:
                    if (!this.uAnima()) fail();
                    break;
                case HelmSpecial.Pneuma:
                    if (!this.uPneuma()) fail();
                    break;
                case HelmSpecial.Hearty:
                    if (!this.uHearty()) fail();
                    break;
                default:
                    Core.logger(`Enhancement Failed:\tInvalid "HelmSpecial" given, received ${hSpecial} | ${hName}`);
                    return;
            }

            if (this.alreadyHasRequestedEnhancement(helm, type, cSpecial, hSpecial, wSpecial, logging)) skipCounter++;
            // C# doesn't pass `logging` here.
            else if (canEnhance) _autoEnhance(helm, 2164, hSpecial > 0 ? "forge" : null);
            else skipCounter++;
        }

        // Enhancing the weapon with the weapon special
        if (weapon) {
            let shopID = 0;
            let canEnhance = true;
            const fail = () => {
                Core.logger(`Enhancement Failed:\tYou did not unlock the ${wName} Enhancement yet`);
                canEnhance = false;
            };

            // Forge (1) takes this branch too since the specials were renumbered, as in C#.
            if (wSpecial > 0 && wSpecial <= 6) {
                switch (type) {
                    case EnhancementType.Fighter:
                        shopID = 635;
                        break;
                    case EnhancementType.Thief:
                        shopID = 637;
                        break;
                    case EnhancementType.Hybrid:
                        shopID = 633;
                        break;
                    case EnhancementType.Wizard:
                    case EnhancementType.SpellBreaker:
                        shopID = 636;
                        break;
                    case EnhancementType.Healer:
                        shopID = 638;
                        break;
                    case EnhancementType.Lucky:
                        shopID = 639;
                        break;
                    default:
                        Core.logger(`Enhancement Failed:\tInvalid "EnhancementType" given, received ${wSpecial} | ${wName}`);
                        return;
                }
            } else {
                switch (wSpecial) {
                    case WeaponSpecial.Forge:
                        if (!this.uForgeWeapon()) {
                            Core.logger("Enhancement Failed:\tYou did not unlock the Forge (Weapon) Enhancement yet");
                            canEnhance = false;
                        }
                        break;
                    case WeaponSpecial.Lacerate:
                        if (!this.uLacerate()) fail();
                        break;
                    case WeaponSpecial.Smite:
                        if (!this.uSmite()) fail();
                        break;
                    case WeaponSpecial.Valiance:
                        if (!this.uValiance()) fail();
                        break;
                    case WeaponSpecial.Arcanas_Concerto:
                        if (!this.uArcanasConcerto()) {
                            Core.logger("Enhancement Failed:\tYou did not unlock the Arcana's Concerto Enhancement yet");
                            canEnhance = false;
                        }
                        break;
                    case WeaponSpecial.Elysium:
                        if (!this.uElysium()) fail();
                        break;
                    case WeaponSpecial.Acheron:
                        if (!this.uAcheron()) fail();
                        break;
                    case WeaponSpecial.Praxis:
                        if (!this.uPraxis()) fail();
                        break;
                    case WeaponSpecial.Dauntless:
                        if (!this.uDauntless()) fail();
                        break;
                    case WeaponSpecial.Ravenous:
                        if (!this.uRavenous()) fail();
                        break;

                    default:
                        Core.logger(`Enhancement Failed:\tInvalid "WeaponSpecial" given, received ${wSpecial} | ${wName}`);
                        return;
                }

                shopID = 2142;
            }

            if (this.alreadyHasRequestedEnhancement(weapon, type, cSpecial, hSpecial, wSpecial, logging)) skipCounter++;
            else if (canEnhance) _autoEnhance(weapon, shopID, wSpecial === WeaponSpecial.Forge || wSpecial > 6 ? "forge" : null, logging);
            else skipCounter++;
        }

        if (skipCounter > 0) Core.logger(`Enhancement Skipped:\t${skipCounter} item${skipCounter > 1 ? "s" : ""}`);
    }

    private getProcID(item: InventoryItem | null): number {
        return item === null ? 0 : Number(Core.getItemProperty(item, "ProcID")) || 0;
    }

    private getEnhID(item: InventoryItem | null): number {
        return item === null ? 0 : Number(Core.getItemProperty(item, "iEnh")) || 0;
    }

    /** Awe enhancements (Spiral Carve, Awe Blast, ...) are unlocked. */
    uAwe(): boolean {
        return Core.isCompletedBefore(2937);
    }

    uForgeWeapon(): boolean {
        return Core.isCompletedBefore(8738);
    }

    uLacerate(): boolean {
        return Core.isCompletedBefore(8739);
    }

    uSmite(): boolean {
        return Core.isCompletedBefore(8740);
    }

    uValiance(): boolean {
        return Core.isCompletedBefore(8741);
    }

    uArcanasConcerto(): boolean {
        return Core.isCompletedBefore(8742);
    }

    uAbsolution(): boolean {
        return Core.isCompletedBefore(8743);
    }

    uVainglory(): boolean {
        return Core.isCompletedBefore(8744);
    }

    uAvarice(): boolean {
        return Core.isCompletedBefore(8745);
    }

    uForgeCape(): boolean {
        return Core.isCompletedBefore(8758);
    }

    uElysium(): boolean {
        return Core.isCompletedBefore(8821);
    }

    uAcheron(): boolean {
        return Core.isCompletedBefore(8820);
    }

    uPenitence(): boolean {
        return Core.isCompletedBefore(8822);
    }

    uLament(): boolean {
        return Core.isCompletedBefore(8823);
    }

    uVim(): boolean {
        return Core.isCompletedBefore(8824);
    }

    uExamen(): boolean {
        return Core.isCompletedBefore(8825);
    }

    uForgeHelm(): boolean {
        return Core.isCompletedBefore(8828);
    }

    uPneuma(): boolean {
        return Core.isCompletedBefore(8827);
    }

    uAnima(): boolean {
        return Core.isCompletedBefore(8826);
    }

    uDauntless(): boolean {
        return Core.isCompletedBefore(9172);
    }

    uPraxis(): boolean {
        return Core.isCompletedBefore(9171);
    }

    uRavenous(): boolean {
        return Core.isCompletedBefore(9560);
    }

    uHearty(): boolean {
        return Core.isCompletedBefore(9466) && Farm.factionRank("Grimskull Trolling") >= 7;
    }

    //#endregion

    //#region SmartEnhance

    /**
     * Finds the best enhancements for a class in its library (Forge enhancements when they're
     * unlocked, else Awe ones), enhances the class if it isn't yet, equips it and enhances your
     * equipped gear with them (C# `SmartEnhance`). Does nothing when CoreBots Options'
     * DisableAutoEnhance is on, unless `forceEnh`.
     * @param className Name of the class you wish to enhance.
     * @param options `forceEnh`: for classes that are received unenhanced (enhances only the class).
     *   `weaponOverride`: the weapon special to use instead of the library's pick (falls back to
     *   the base enhancement when it isn't unlocked, like any other weapon special).
     */
    smartEnhance(name: string | null, { forceEnh = false, weaponOverride = null }: { forceEnh?: boolean; weaponOverride?: WeaponSpecial | null } = {}): void {
        // Get CBO bool for DisableAutoEnhance and return if we're not forcing an enh for unenhed (lvl = 0) classes
        let enhDisabled = false;
        const autoEnhance = Core.cboBool("DisableAutoEnhance");
        if (autoEnhance !== null) enhDisabled = autoEnhance;

        if (!name) {
            Core.logger(`${name ?? ""} is null`);
            return;
        }
        let className = name;

        if (enhDisabled && !forceEnh) {
            Core.logger("AutoEnh turned off in CBO, class/items will *not* be enhanced");
            return;
        }

        if (!Core.checkInventory(className)) {
            Core.logger(`SmartEnhance Failed: Class ${className} was not found in inventory`);
            return;
        }

        if (bot.player.inCombat) Core.jumpWait();

        // Error correction
        className = className.toLowerCase().trim();
        const selectedClass = bot.inventory.items.find((i) => i.name.toLowerCase().trim() === className.toLowerCase().trim() && isCategory(i, "Class"));
        if (!selectedClass) {
            Core.logger(`SmartEnhance Failed: Class ${className} was not found in inventory`);
            return;
        }

        // Creating variables that can be assigned to (the casts keep TypeScript from narrowing them)
        className = selectedClass.name.toLowerCase();
        let type = null as EnhancementType | null;
        let cSpecial = CapeSpecial.None as CapeSpecial;
        let hSpecial = HelmSpecial.None as HelmSpecial;
        let wSpecial = WeaponSpecial.None as WeaponSpecial;

        // C# `goto default`: use the Awe library.
        const useAwe = (): boolean => {
            type = EnhancementType.Lucky;
            return false;
        };

        const forgeEnhancementLibrary = (): boolean => {
            switch (className.toLowerCase()) {
                //#region Lucky Region

                //#region Lucky - Vim - vainglory - lacerate
                case "horc evader":
                    if (!this.uLacerate() || !this.uVim() || !this.uVainglory())
                        return useAwe();

                    type = EnhancementType.Thief;
                    cSpecial = CapeSpecial.Vainglory;
                    wSpecial = WeaponSpecial.Lacerate;
                    hSpecial = HelmSpecial.Vim;
                    break;
                //#endregion

                //#region Luck - Awe_Blast | Arcanas_Concerto - ForgeHelm - Penitence
                case "lord of order":
                    if (!this.uAwe() || !this.uForgeHelm() || !this.uPenitence())
                        return useAwe();

                    type = EnhancementType.Lucky;
                    wSpecial = this.uArcanasConcerto()
                        ? WeaponSpecial.Arcanas_Concerto
                        : WeaponSpecial.Awe_Blast;
                    hSpecial = HelmSpecial.Forge;
                    cSpecial = CapeSpecial.Penitence;
                    break;
                //#endregion

                //#region Ravenous
                case "PlaceHolder": // Never matches (className is lower case), as in C#.
                    if (!this.uRavenous())
                        return useAwe();

                    type = EnhancementType.Lucky;
                    cSpecial = CapeSpecial.Forge;
                    wSpecial = WeaponSpecial.Ravenous;
                    break;
                //#endregion


                //#region Lucky - Dauntless - Vim - Lament
                case "great thief":
                    if (!this.uDauntless() || !this.uVim() || !this.uLament())
                        return useAwe();

                    type = EnhancementType.Lucky;
                    cSpecial = CapeSpecial.Lament;
                    wSpecial = WeaponSpecial.Dauntless;
                    hSpecial = HelmSpecial.Vim;
                    break;
                //#endregion

                //#region Lucky - Lacerate - Vim - Lament
                case "timekeeper":
                case "timekiller":
                    if (!this.uLacerate() || !this.uVim() || !this.uLament())
                        return useAwe();

                    type = EnhancementType.Lucky;
                    cSpecial = CapeSpecial.Lament;
                    wSpecial = WeaponSpecial.Lacerate;
                    hSpecial = HelmSpecial.Vim;
                    break;
                //#endregion

                //#region Lucky - Forge - Spiral Carve
                case "corrupted chronomancer":
                case "underworld chronomancer":
                case "eternal chronomancer":
                case "immortal chronomancer":
                case "dark metal necro":
                    if (!this.uForgeCape())
                        return useAwe();

                    type = EnhancementType.Lucky;
                    cSpecial = CapeSpecial.Forge;
                    wSpecial = WeaponSpecial.Spiral_Carve;
                    break;
                //#endregion

                //#region Lucky - Forge - Awe Blast
                case "glacial berserker":
                    if (!Core.isCompletedBefore(8758))
                        return useAwe();

                    type = EnhancementType.Lucky;
                    cSpecial = CapeSpecial.Forge;
                    wSpecial = WeaponSpecial.Awe_Blast;
                    break;
                //#endregion

                //#region Lucky - Forge - Mana Vamp
                case "legendary elemental warrior":
                case "mythic elemental warrior":
                case "ultra elemental warrior":
                    if (!this.uForgeCape())
                        return useAwe();

                    type = EnhancementType.Lucky;
                    cSpecial = CapeSpecial.Forge;
                    wSpecial = WeaponSpecial.Mana_Vamp;
                    break;
                //#endregion

                //#region Lucky - Forge - Smite
                case "Draconic Chronomancer": // Never matches (className is lower case), as in C#.
                    if (!this.uSmite() || !this.uForgeCape())
                        return useAwe();

                    type = EnhancementType.Lucky;
                    cSpecial = CapeSpecial.Forge;
                    wSpecial = WeaponSpecial.Smite;
                    break;
                //#endregion

                //#region Lucky - Forge - Elysium
                case "ultra omniknight":
                case "dark ultra omninight":
                    if (!this.uElysium() || !this.uForgeCape())
                        return useAwe();

                    type = EnhancementType.Lucky;
                    cSpecial = CapeSpecial.Forge;
                    wSpecial = WeaponSpecial.Elysium;
                    break;
                //#endregion

                //#region Lucky - Vainglory - Valiance - Anima
                case "archfiend":
                case "eternal inversionist":
                case "dragonlord":
                    if (!this.uVainglory() || !this.uValiance() || !this.uAnima())
                        return useAwe();

                    type = EnhancementType.Lucky;
                    cSpecial = CapeSpecial.Vainglory;
                    wSpecial = WeaponSpecial.Valiance;
                    hSpecial = HelmSpecial.Anima;

                    break;
                //#endregion

                //#region Lucky - Vainglory - Valiance - Vim
                case "continuum chronomancer":
                case "quantum chronomancer":
                    if (
                        !this.uPenitence()
                        || (!this.uDauntless() || !this.uValiance())
                        || !this.uRavenous()
                        || !this.uValiance()
                        || !this.uAnima()
                    )
                        return useAwe();

                    type = EnhancementType.Lucky;
                    cSpecial = CapeSpecial.Vainglory;
                    wSpecial =
                        this.uRavenous() ? WeaponSpecial.Ravenous
                        : this.uDauntless() ? WeaponSpecial.Dauntless
                        : WeaponSpecial.Valiance;

                    hSpecial = HelmSpecial.Anima;
                    break;
                //#endregion

                //#region Lucky - Vainglory - Dauntless - Anima
                case "chaos avenger":
                    if (!this.uAnima() || !this.uVainglory())
                        return useAwe();

                    type = EnhancementType.Lucky;
                    cSpecial = CapeSpecial.Vainglory;
                    wSpecial = this.uDauntless() ? WeaponSpecial.Dauntless : WeaponSpecial.Valiance;
                    hSpecial = HelmSpecial.Anima;
                    break;
                //#endregion

                //#region Lucky - Lacerate - Forge - Lament

                case "doom metal necro":
                case "neo metal necro":
                    if (!this.uLacerate() || !this.uForgeHelm() || !this.uLament())
                        return useAwe();

                    type = EnhancementType.Lucky;
                    cSpecial = this.uLament() ? CapeSpecial.Lament : CapeSpecial.Vainglory;
                    wSpecial = WeaponSpecial.Lacerate;
                    hSpecial = HelmSpecial.Forge;
                    break;
                //#endregion

                //#region Lucky - Vainglory - Dauntless|Valiance|Smite - Vim
                case "martial artist":
                case "master martial artist":
                    if ((!this.uDauntless() && !this.uValiance() && !this.uSmite()) || !this.uVainglory() || !this.uVim())
                        return useAwe();

                    type = EnhancementType.Lucky;
                    cSpecial = CapeSpecial.Vainglory;
                    wSpecial =
                        this.uDauntless() ? WeaponSpecial.Dauntless
                        : this.uValiance() ? WeaponSpecial.Valiance
                        : WeaponSpecial.Smite; // else do smite, if no smite > do Awe
                    hSpecial = HelmSpecial.Vim;
                    break;
                //#endregion

                //#region Lucky - Vainglory - Praxis - Vim
                case "yami no ronin":
                    if (!this.uPraxis() || !this.uVainglory() || !this.uVim())
                        return useAwe();
                    type = EnhancementType.Lucky;
                    cSpecial = CapeSpecial.Vainglory;
                    wSpecial = WeaponSpecial.Praxis;
                    hSpecial = HelmSpecial.Vim;
                    break;
                //#endregion

                //#region Lucky - Vainglory - Valiance - Anima
                case "nechronomancer":
                case "necrotic chronomancer":
                    if (!this.uVainglory() || !this.uArcanasConcerto() || !this.uAnima())
                        return useAwe();

                    type = EnhancementType.Lucky;
                    cSpecial = CapeSpecial.Vainglory;
                    wSpecial = WeaponSpecial.Valiance;
                    hSpecial = HelmSpecial.Anima;
                    break;
                //#endregion

                //#region Lucky - Vainglory - Elysium - Vim
                case "shadowwalker of time":
                case "shadowstalker of time":
                case "shadowweaver of time":
                    if (!this.uVainglory() || !this.uElysium() || !this.uVim())
                        return useAwe();

                    type = EnhancementType.Lucky;
                    cSpecial = CapeSpecial.Vainglory;
                    wSpecial = WeaponSpecial.Elysium;
                    hSpecial = HelmSpecial.Vim;
                    break;
                //#endregion

                //#region Lucky - Vainglory - Valiance - None
                case "legion doomknight":
                    if (!this.uVainglory() || !this.uValiance())
                        return useAwe();

                    type = EnhancementType.Lucky;
                    cSpecial = CapeSpecial.Vainglory;
                    wSpecial = WeaponSpecial.Valiance;
                    hSpecial = this.currentHelmSpecial();
                    break;
                //#endregion

                //#region Lucky - Vainglory - Elysium - Pneuma
                case "antique hunter":
                case "artifact hunter":
                    if (!this.uVainglory() || !this.uElysium() || !this.uPneuma())
                        return useAwe();

                    type = EnhancementType.Lucky;
                    cSpecial = CapeSpecial.Vainglory;
                    wSpecial = WeaponSpecial.Elysium;
                    hSpecial = HelmSpecial.Pneuma;
                    break;
                //#endregion

                //#region Lucky - Lament - Elysium - Pneuma
                case "abyssal angel":
                case "abyssal angel's shadow":
                    if (!this.uLament() || !this.uElysium() || !this.uPneuma())
                        return useAwe();

                    type = EnhancementType.Lucky;
                    cSpecial = CapeSpecial.Vainglory;
                    wSpecial = WeaponSpecial.Elysium;
                    hSpecial = HelmSpecial.Pneuma;
                    break;
                //#endregion

                //#region Lucky - Dauntless | Ravenous - Anima | ForgeHelm - Vainglory
                case "verus doomknight":
                    if (!this.uRavenous() || !this.uForgeHelm() || !this.uVainglory())
                        return useAwe();

                    type = EnhancementType.Lucky;
                    cSpecial = CapeSpecial.Vainglory;
                    wSpecial = this.uDauntless() ? WeaponSpecial.Dauntless : WeaponSpecial.Ravenous;
                    hSpecial = this.uAnima() ? HelmSpecial.Anima : HelmSpecial.Forge;
                    break;
                //#endregion

                //#region Lucky - Vainglory - Dauntless/Valiance - Anima
                case "debris highlord":
                case "void highlord":
                case "void highlord (ioda)":
                    if (!this.uAnima() || !this.uValiance() || !this.uVainglory())
                        return useAwe();

                    type = EnhancementType.Lucky;
                    cSpecial = CapeSpecial.Vainglory;
                    wSpecial = !this.uDauntless()
                        ? (
                            this.uRavenous()
                                ? WeaponSpecial.Ravenous
                                : (this.uValiance() ? WeaponSpecial.Valiance : WeaponSpecial.Forge)
                        )
                        : WeaponSpecial.Dauntless;
                    hSpecial = HelmSpecial.Anima;
                    break;
                //#endregion


                //#region Lucky - Avarice - Dauntless - Anima
                case "flame dragon warrior":
                    if (!this.uAvarice() || !this.uDauntless() || !this.uAnima())
                        return useAwe();

                    type = EnhancementType.Lucky;
                    cSpecial = CapeSpecial.Avarice;
                    wSpecial = WeaponSpecial.Dauntless;
                    hSpecial = HelmSpecial.Anima;
                    break;
                //#endregion

                //#region Lucky - Avarice - Elysium - Anima
                case "chaos slayer":
                case "chaos slayer berserker":
                case "chaos slayer cleric":
                case "chaos slayer mystic":
                case "chaos slayer thief":
                    if (!this.uAvarice() || !this.uElysium() || !this.uAnima())
                        return useAwe();

                    type = EnhancementType.Lucky;
                    cSpecial = CapeSpecial.Avarice;
                    wSpecial = WeaponSpecial.Elysium;
                    hSpecial = HelmSpecial.Anima;
                    break;
                //#endregion

                //#region Lucky - Penitence - Ravenous | Praxis | Lacerate - Forge | None
                case "archpaladin":
                    if (!this.uLacerate() || !this.uForgeHelm() || !this.uPenitence())
                        return useAwe();

                    type = EnhancementType.Lucky;
                    wSpecial = this.uValiance()
                        ? WeaponSpecial.Valiance
                        : (this.uPraxis() ? WeaponSpecial.Praxis : WeaponSpecial.Lacerate);
                    hSpecial = this.uForgeHelm() ? HelmSpecial.Forge : HelmSpecial.None;
                    cSpecial = CapeSpecial.Penitence;
                    break;
                //#endregion

                //#region Fighter - Ravenous | Valiance - Anima - Absolution
                case "stonecrusher":
                    if (!this.uValiance() || !this.uAnima() || !this.uAbsolution())
                        return useAwe();

                    type = EnhancementType.Fighter;
                    wSpecial = WeaponSpecial.Valiance;
                    hSpecial = HelmSpecial.Anima;
                    cSpecial = CapeSpecial.Absolution;
                    break;
                //#endregion

                //#endregion

                //#region Wizard Region

                //#region Wizard -  Valiance|Praxis - Pneuna - Vainglory|Lament
                case "lightcaster":
                    if (!this.uValiance() || !this.uPneuma() || !this.uVainglory()) {
                        if (!this.uLament() || !this.uPraxis())
                            return useAwe();
                    }
                    type = EnhancementType.Wizard;
                    cSpecial = !this.uVainglory() ? CapeSpecial.Lament : CapeSpecial.Vainglory;
                    wSpecial = !this.uValiance() ? WeaponSpecial.Praxis : WeaponSpecial.Valiance;
                    hSpecial = !this.uPneuma() ? this.currentHelmSpecial() : HelmSpecial.Pneuma;
                    break;
                //#endregion

                case "archivist of time":
                    //#region Wizard -  Valiance - Pneuna - Vainglory
                    if (!this.uValiance() || !this.uPneuma() || !this.uVainglory()) {
                        return useAwe();
                    }
                    type = EnhancementType.Wizard;
                    cSpecial = CapeSpecial.Vainglory;
                    wSpecial = WeaponSpecial.Valiance;
                    hSpecial = HelmSpecial.Pneuma;
                    break;

                    //#endregion

                //#region Wizard - Forge - Awe Blast
                case "infinity knight":
                    if (!this.uForgeCape())
                        return useAwe();

                    type = EnhancementType.Wizard;
                    cSpecial = CapeSpecial.Forge;
                    wSpecial = WeaponSpecial.Awe_Blast;
                    hSpecial = this.currentHelmSpecial();
                    break;
                //#endregion

                //#region Wizard - Vainglory - Valiance - Pneuma
                case "archmage":
                case "darklord":
                case "arcana invoker":
                    if (!this.uVainglory() || !this.uValiance() || !this.uPneuma())
                        return useAwe();

                    type = EnhancementType.Wizard;
                    cSpecial = CapeSpecial.Vainglory;
                    wSpecial = this.uRavenous() ? WeaponSpecial.Ravenous : WeaponSpecial.Valiance;
                    hSpecial = HelmSpecial.Pneuma;
                    break;
                //#endregion

                //#region Wizard - Penitence - Acheron - Pneuma
                case "master of moglins":
                case "dark master of moglins":
                    if (!this.uPenitence() || !this.uAcheron() || !this.uPneuma())
                        return useAwe();

                    type = EnhancementType.Wizard;
                    cSpecial = CapeSpecial.Penitence;
                    wSpecial = WeaponSpecial.Acheron;
                    hSpecial = HelmSpecial.Pneuma;
                    break;
                //#endregion

                //#region Wizard - Vainglory - Ravenous | Valiance - Pneuma | Wizard
                case "legion revenant":
                case "legion revenant (ioda)":
                    if (!this.uVainglory() || !this.uValiance() || !this.uPneuma())
                        return useAwe();

                    type = EnhancementType.Wizard;
                    cSpecial = CapeSpecial.Vainglory;
                    wSpecial = this.uRavenous() ? WeaponSpecial.Ravenous : WeaponSpecial.Valiance;
                    hSpecial = HelmSpecial.Pneuma;
                    break;
                //#endregion

                //#region Wizard - Avarice - Elysium - Pneuma
                case "vampire lord":
                case "enchanted vampire lord":
                case "royal vampire lord":
                case "darkside":
                case "dark lord":
                    if (!this.uAvarice() || !this.uElysium() || !this.uPneuma())
                        return useAwe();

                    type = EnhancementType.Wizard;
                    cSpecial = CapeSpecial.Avarice;
                    wSpecial = WeaponSpecial.Elysium;
                    hSpecial = HelmSpecial.Pneuma;
                    break;
                //#endregion

                //#region Wizard - Vainglory - Elysium - Pneuma
                case "shaman":
                    if (!this.uVainglory() || !this.uElysium() || !this.uPneuma())
                        return useAwe();

                    type = EnhancementType.Wizard;
                    cSpecial = CapeSpecial.Vainglory;
                    wSpecial = WeaponSpecial.Elysium;
                    hSpecial = HelmSpecial.Pneuma;
                    break;
                //#endregion

                //#region Wizard - Avarice - Acheron - Pneuma
                case "blaze binder":
                    if (!this.uAvarice() || !this.uAcheron() || !this.uPneuma())
                        return useAwe();

                    type = EnhancementType.Wizard;
                    cSpecial = CapeSpecial.Avarice;
                    wSpecial = WeaponSpecial.Acheron;
                    hSpecial = HelmSpecial.Pneuma;
                    break;
                //#endregion

                //#region Wizard - Lament - Elysium - Pneuma
                case "royal battlemage":
                    if (!this.uLament() || !this.uElysium() || !this.uPneuma())
                        return useAwe();

                    type = EnhancementType.Wizard;
                    cSpecial = CapeSpecial.Lament;
                    wSpecial = WeaponSpecial.Elysium;
                    hSpecial = HelmSpecial.Pneuma;
                    break;
                //#endregion

                //#region Wizard - Lament - Valiance - Pneuma
                case "scarlet sorceress":
                case "kathool adept":
                    if (!this.uLament() || !this.uValiance() || !this.uPneuma())
                        return useAwe();

                    type = EnhancementType.Wizard;
                    cSpecial = CapeSpecial.Lament;
                    wSpecial = WeaponSpecial.Valiance;
                    hSpecial = HelmSpecial.Pneuma;
                    break;
                //#endregion


                //#region Wizard - Vainglory / Forge - Daunt / Ravenous / Forge - Pneuma / Forge
                case "sovereign of storms":
                    if (!this.uVainglory() || !this.uDauntless() || !this.uRavenous() || !this.uPneuma())
                        return useAwe();

                    type = EnhancementType.Wizard;
                    cSpecial = this.uVainglory() ? CapeSpecial.Vainglory : CapeSpecial.Forge;
                    wSpecial = this.uDauntless()
                        ? WeaponSpecial.Dauntless
                        : (this.uRavenous() ? WeaponSpecial.Ravenous : WeaponSpecial.Forge);
                    hSpecial = this.uPneuma() ? HelmSpecial.Pneuma : HelmSpecial.Forge;
                    break;
                //#endregion


                //#region Wizard - Ravenous - Lament - Examen
                case "lich":
                    if (!(this.uRavenous() && this.uLament() && this.uExamen()))
                        return useAwe();

                    type = EnhancementType.Wizard;
                    cSpecial = CapeSpecial.Lament;
                    wSpecial = WeaponSpecial.Ravenous;
                    hSpecial = HelmSpecial.Examen;
                    break;
                //#endregion
                //#endregion

                //#region Healer Region

                //#region Healer - Avarice - Elysium - Pneuma
                case "dragon of time":
                    if (!this.uAvarice() || !this.uElysium() || !this.uPneuma())
                        return useAwe();

                    type = EnhancementType.Healer;
                    cSpecial = CapeSpecial.Avarice;
                    wSpecial = WeaponSpecial.Elysium;
                    hSpecial = HelmSpecial.Pneuma;
                    break;

                //#endregion

                //#region Healer - None - Valiance - Nine
                case "obsidian paladin chronomancer":
                case "paladin chronomancer":
                    if (!this.uValiance())
                        return useAwe();

                    type = EnhancementType.Healer;
                    cSpecial = CapeSpecial.None;
                    wSpecial = WeaponSpecial.Valiance;
                    hSpecial = HelmSpecial.None;
                    break;

                //#endregion

                //#region Fighter - Ravenous | Valiance - Anima - Absolution
                case "frostval barbarian":
                    if (!this.uAbsolution() || !this.uValiance() || !this.uAnima())
                        return useAwe();
                    type = EnhancementType.Fighter;
                    cSpecial = CapeSpecial.Absolution;
                    wSpecial = this.uRavenous() ? WeaponSpecial.Ravenous : WeaponSpecial.Valiance;
                    hSpecial = HelmSpecial.Anima;
                    break;
                //#endregion

                //#region Lucky - Penitence | Absolution - Elysium | Valiance - Vim
                case "arachnomancer":
                    if (!this.uAbsolution() || !this.uAbsolution() || !this.uVim())
                        return useAwe();

                    type = EnhancementType.Lucky;
                    cSpecial = this.uPenitence() ? CapeSpecial.Penitence : CapeSpecial.Absolution;
                    wSpecial = this.uElysium() ? WeaponSpecial.Elysium : WeaponSpecial.Valiance;
                    hSpecial = HelmSpecial.Vim;
                    break;
                //#endregion

                //#region Healer - Valiance - Current - Current

                //#endregion

                //#region Wizard - Elysium/Ravenous - Examen - Lament
                case "phantom chronomancer":
                case "phantasm chronomancer":

                    if (this.uElysium() || this.uRavenous())
                        return useAwe();
                    type = EnhancementType.Wizard;
                    cSpecial = CapeSpecial.Lament;
                    wSpecial = this.uElysium() ? WeaponSpecial.Elysium : WeaponSpecial.Ravenous;
                    hSpecial = HelmSpecial.Examen;
                    break;

                //#endregion

                case "scion of flames":
                    if ((!this.uVainglory() || !this.uLament())
                    && (!this.uPneuma() || !this.uForgeHelm())
                    && (!this.uRavenous() || !this.uValiance()))
                        return useAwe();
                    type = EnhancementType.Wizard;
                    cSpecial = this.uVainglory() ? CapeSpecial.Vainglory : CapeSpecial.Lament;
                    wSpecial = this.uRavenous() ? WeaponSpecial.Ravenous : WeaponSpecial.Valiance;
                    hSpecial = this.uPneuma() ? HelmSpecial.Pneuma : HelmSpecial.Forge;
                    break;

                //#region Healer - Current - Valiance/Awe - Current
                case "healer":
                case "healer (rare)":
                    type = EnhancementType.Healer;
                    cSpecial = this.currentCapeSpecial();
                    wSpecial = this.uValiance() ? WeaponSpecial.Valiance : WeaponSpecial.Awe_Blast;
                    hSpecial = this.currentHelmSpecial();
                    break;
                //#endregion

                //#region Luck - Vim - Lam - Rav
                case "chrono shadowslayer":
                case "chrono shadowhunter":
                    type = EnhancementType.Lucky;
                    cSpecial = this.uLament()
                        ? CapeSpecial.Lament
                        : (this.uForgeCape() ? CapeSpecial.Forge : this.currentCapeSpecial());
                    wSpecial = this.uRavenous()
                        ? WeaponSpecial.Ravenous
                        : (
                            this.uArcanasConcerto()
                                ? WeaponSpecial.Arcanas_Concerto
                                : (this.uForgeWeapon() ? WeaponSpecial.Forge : WeaponSpecial.Awe_Blast)
                        );
                    hSpecial = this.uVim()
                        ? HelmSpecial.Vim
                        : (this.uForgeHelm() ? HelmSpecial.Forge : this.currentHelmSpecial());
                    break;
                //#endregion

                //#region Lucky - Vainglory - Valiance / Dauntless - Anima
                case "glacial warlord":
                case "glaceran warlord":
                case "dark glaceran warlord":
                case "savage glaceran warlord":
                    if (!this.uVainglory() || !this.uValiance() || !this.uAnima())
                        return useAwe();

                    type = EnhancementType.Lucky;
                    cSpecial = CapeSpecial.Vainglory;
                    wSpecial = this.uDauntless() ? WeaponSpecial.Dauntless : WeaponSpecial.Valiance;
                    hSpecial = HelmSpecial.Anima;
                    break;
                //#endregion

                //#region Lucky - Ravenous/Valiance - Examen - Lament/Vainglory
                case "king's echo":
                    if (!this.uValiance() || !this.uExamen() || !this.uVainglory())
                        return useAwe();

                    type = EnhancementType.Healer;
                    cSpecial = this.uLament() ? CapeSpecial.Lament : CapeSpecial.Vainglory;
                    wSpecial = this.uElysium() ? WeaponSpecial.Elysium : WeaponSpecial.Mana_Vamp;
                    hSpecial = HelmSpecial.Examen;
                    break;
                //#endregion

                //#region Luck - Val/Smite/Mana - Anima - Vg
                case "dragonslayer general":
                    type = EnhancementType.Lucky;

                    cSpecial =
                        this.uVainglory() ? CapeSpecial.Vainglory
                        : this.uForgeCape() ? CapeSpecial.Forge
                        : this.currentCapeSpecial();

                    wSpecial =
                        this.uValiance() ? WeaponSpecial.Valiance
                        : this.uSmite() ? WeaponSpecial.Smite
                        : WeaponSpecial.Mana_Vamp;

                    hSpecial =
                        this.uAnima() ? HelmSpecial.Anima
                        : this.uForgeHelm() ? HelmSpecial.Forge
                        : this.currentHelmSpecial();

                    break;
                //#endregion

                //#region Luck - Dauntless | Ravenous - Anima - Vainglory
                case "chrono chaorruptor":
                    if (!this.uRavenous() || !this.uAnima() || !this.uVainglory())
                        return useAwe();

                    type = EnhancementType.Lucky;
                    cSpecial = CapeSpecial.Vainglory;
                    wSpecial = this.uDauntless() ? WeaponSpecial.Dauntless : WeaponSpecial.Ravenous;
                    hSpecial = HelmSpecial.Anima;
                    break;
                //#endregion

                //#region Wizard - Ravenous - Pneuma - Vainglory
                case "chrono dataknight":
                case "chrono dragonknight":
                    if (!this.uRavenous() || !this.uPneuma() || !this.uVainglory())
                        return useAwe();

                    type = EnhancementType.Wizard;
                    cSpecial = CapeSpecial.Vainglory;
                    wSpecial = WeaponSpecial.Ravenous;
                    hSpecial = HelmSpecial.Pneuma;
                    break;
                //#endregion

                //#region Luck - Ravenous | Valiance - ForgeHelm | Luck - Absolution
                case "legendary hero":
                    if (!this.uValiance() || !this.uForgeHelm() || !this.uAbsolution())
                        return useAwe();

                    type = EnhancementType.Wizard;
                    cSpecial = CapeSpecial.Absolution;
                    wSpecial = this.uRavenous() ? WeaponSpecial.Ravenous : WeaponSpecial.Valiance;
                    hSpecial = HelmSpecial.Forge;
                    break;
                //#endregion

                //#endregion

                //#region Unassigned Region

                // This list serves as an overview of what classes dont have a Forge Enhancement yet, when adding a setup for it, remove it from here
                case "acolyte":
                case "alpha doommega":
                case "alpha omega":
                case "alpha pirate":
                case "arcane dark caster":
                case "assassin":
                case "barber":
                case "bard":
                case "battlemage of love":
                case "battlemage":
                case "beast warrior":
                case "beastmaster":
                case "berserker":
                case "beta berserker":
                case "blademaster assassin":
                case "blademaster":
                case "blood ancient":
                case "blood sorceress":
                case "blood titan":
                case "frostblood titan":
                case "cardclasher":
                case "chaos avenger member preview":
                case "chaos champion prime":
                case "chaos shaper":
                case "chrono assassin":
                case "chrono commandant":
                case "chronocommander":
                case "chronocorrupter":
                case "chronomancer prime":
                case "chronomancer":
                case "chunin":
                case "classic alpha pirate":
                case "classic barber":
                case "classic defender":
                case "classic doomknight":
                case "classic dragonlord":
                case "classic exalted soul cleaver":
                case "classic guardian":
                case "classic paladin":
                case "classic pirate":
                case "classic soul cleaver":
                case "clawsuit":
                case "cryomancer mini pet coming soon":
                case "cryomancer":
                case "daimon":
                case "dark battlemage":
                case "dark caster":
                case "dark chaos berserker":
                case "dark cryomancer":
                case "dark harbinger":
                case "dark legendary hero":
                case "darkblood stormking":
                case "deathknight lord":
                case "deathknight":
                case "defender":
                case "doomknight overlord":
                case "doomknight":
                case "dragon knight":
                case "dragon shinobi":
                case "dragonslayer":
                case "dragonsoul shinobi":
                case "drakel warlord":
                case "elemental dracomancer":
                case "empyrean chronomancer":
                case "enforcer":
                case "evolved clawsuit":
                case "evolved dark caster":
                case "evolved leprechaun":
                case "evolved pumpkin lord":
                case "evolved shaman":
                case "exalted harbinger":
                case "exalted soul cleaver":
                case "firelord summoner":
                case "frost spiritreaver":
                case "glacial berserker test":
                case "grim necromancer":
                case "grunge rocker":
                case "guardian":
                case "heavy metal necro":
                case "heavy metal rockstar":
                case "heroic naval commander":
                case "highseas commander":
                case "hobo highlord":
                case "immortal dark caster":
                case "imperial chunin":
                case "infinite dark caster":
                case "infinite legion dark caster":
                case "infinity titan":
                case "legendary naval commander":
                case "legion blademaster assassin":
                case "legion doomknight tester":
                case "legion evolved dark caster":
                case "legion paladin":
                case "legion revenant member test":
                case "legion swordmaster assassin":
                case "leprechaun":
                case "lightcaster test":
                case "lightmage":
                case "love caster":
                case "lycan":
                case "master ranger":
                case "mechajouster":
                case "mindbreaker":
                case "mystical dark caster":
                case "naval commander":
                case "necromancer":
                case "ninja warrior":
                case "no class":
                case "northlands monk":
                case "not a mod":
                case "nu metal necro":
                case "obsidian no class":
                case "oracle":
                case "overworld chronomancer":
                case "paladin highlord":
                case "paladin":
                case "paladinslayer":
                case "pink romancer":
                case "pinkomancer":
                case "pirate":
                case "prismatic clawsuit":
                case "protosartorium":
                case "psionic mindbreaker":
                case "pumpkin lord":
                case "pyromancer":
                case "ranger":
                case "renegade":
                case "rustbucket":
                case "sakura cryomancer":
                case "sentinel":
                case "shadow dragon shinobi":
                case "shadow ripper":
                case "shadow rocker":
                case "shadowflame dragonlord":
                case "shadowscythe general":
                case "silver paladin":
                case "skycharged grenadier":
                case "skyguard grenadier":
                case "sorcerer":
                case "soul cleaver":
                case "star captain":
                case "starlord":
                case "swordmaster assassin":
                case "swordmaster":
                case "the collector":
                case "thief of hours":
                case "timeless chronomancer":
                case "timeless dark caster":
                case "troubador of love":
                case "unchained rocker":
                case "unchained rockstar":
                case "undead leperchaun":
                case "undeadslayer":
                case "unlucky leperchaun":
                case "vampire":
                case "vindicator of they":
                case "void highlord tester":
                case "warlord":
                case "warrior":
                case "classic warrior":
                case "warrior (rare)":
                case "warriorscythe general":
                case "witch":
                default: // If the correct enhancement arent unlocked, or the class in question isnt in the Forge Enhancement Lib, use Awe Enhancements Lib
                    return useAwe();

                //#endregion
            }
            return true;
        };

        const aweEnhancementLibrary = (): void => {
            //tolower incase we accidentaly use capitals.. it breaks
            switch (className) {
                //#region Lucky Region

                //#region Lucky - Spiral Carve
                case "abyssal angel":
                case "abyssal angel's shadow":
                case "artifact hunter":
                case "assassin":
                case "archmage":
                case "beastmaster":
                case "berserker":
                case "beta berserker":
                case "blademaster assassin":
                case "blademaster":
                case "blood titan":
                case "frostblood titan":
                case "cardclasher":
                case "chaos avenger member preview":
                case "chaos champion prime":
                case "chaos slayer":
                case "chaos slayer berserker":
                case "chaos slayer cleric":
                case "chaos slayer mystic":
                case "chaos slayer thief":
                case "chrono chaorruptor":
                case "chrono commandant":
                case "chronocommander":
                case "chronocorrupter":
                case "chunin":
                case "classic alpha pirate":
                case "classic barber":
                case "classic doomknight":
                case "classic exalted soul cleaver":
                case "classic guardian":
                case "classic paladin":
                case "classic pirate":
                case "classic soul cleaver":
                case "continuum chronomancer":
                case "corrupted chronomancer":
                case "dark chaos berserker":
                case "dark harbinger":
                case "doomknight":
                case "empyrean chronomancer":
                case "eternal chronomancer":
                case "evolved clawsuit":
                case "evolved dark caster":
                case "evolved leprechaun":
                case "exalted harbinger":
                case "exalted soul cleaver":
                case "glaceran warlord":
                case "dark glaceran warlord":
                case "savage glaceran warlord":
                case "glacial warlord":
                case "great thief":
                case "hollowborn vindicator member preview":
                case "immortal chronomancer":
                case "imperial chunin":
                case "infinite dark caster":
                case "infinite legion dark caster":
                case "infinity titan":
                case "legion blademaster assassin":
                case "legion evolved dark caster":
                case "legion swordmaster assassin":
                case "leprechaun":
                case "lycan":
                case "master ranger":
                case "mechajouster":
                case "necromancer":
                case "ninja warrior":
                case "not a mod":
                case "overworld chronomancer":
                case "pinkomancer":
                case "prismatic clawsuit":
                case "quantum chronomancer":
                case "ranger":
                case "renegade":
                case "rogue":
                case "classic rogue":
                case "rogue (rare)":
                case "scarlet sorceress":
                case "shadowscythe general":
                case "skycharged grenadier":
                case "skyguard grenadier":
                case "sovereign of storms":
                case "soul cleaver":
                case "starlord":
                case "swordmaster assassin":
                case "swordmaster":
                case "timekeeper":
                case "timekiller":
                case "timeless chronomancer":
                case "undead leperchaun":
                case "undeadslayer":
                case "underworld chronomancer":
                case "unlucky leperchaun":
                case "debris highlord":
                case "void highlord":
                case "void highlord (ioda)":
                case "verus doomknight":
                case "kathool adept":
                    type = EnhancementType.Lucky;
                    wSpecial = WeaponSpecial.Spiral_Carve;
                    break;
                //#endregion

                //#region Lucky - Mana Vamp
                case "alpha doommega":
                case "alpha omega":
                case "alpha pirate":
                case "beast warrior":
                case "blood ancient":
                case "chaos avenger":
                case "chaos shaper":
                case "classic defender":
                case "clawsuit":
                case "cryomancer mini pet coming soon":
                case "dark legendary hero":
                case "dragonsoul shinobi":
                case "ultra omniknight":
                case "dark ultra omninight":
                case "doomknight overlord":
                case "dragonslayer general":
                case "drakel warlord":
                case "glacial berserker test":
                case "heroic naval commander":
                case "legendary elemental warrior":
                case "mythic elemental warrior":
                case "legendary naval commander":
                case "legion revenant member test":
                case "naval commander":
                case "paladin high lord":
                case "paladin":
                case "paladinslayer":
                case "pirate":
                case "pumpkin lord":
                case "shadowflame dragonlord":
                case "shadowstalker of time":
                case "shadowwalker of time":
                case "shadowweaver of time":
                case "silver paladin":
                case "thief of hours":
                case "ultra elemental warrior":
                case "void highlord tester":
                case "warlord":
                case "warrior":
                case "warrior (rare)":
                case "warriorscythe general":
                case "yami no ronin":
                case "arachnomancer":
                    type = EnhancementType.Lucky;
                    wSpecial = WeaponSpecial.Mana_Vamp;
                    break;
                //#endregion

                //#region Lucky - Awe Blast
                case "archpaladin":
                case "bard":
                case "chrono assassin":
                case "chronomancer":
                case "chronomancer prime":
                case "Phantom Chronomancer":
                case "dark metal necro":
                case "deathknight lord":
                case "dragon shinobi":
                case "dragonlord":
                case "evolved pumpkin lord":
                case "glacial berserker":
                case "grunge rocker":
                case "guardian":
                case "heavy metal necro":
                case "heavy metal rockstar":
                case "hollowborn vindicator":
                case "Hollowborn Vindicator Member Preview":
                case "hobo highlord":
                case "lord of order":
                case "legendary hero":
                case "nechronomancer":
                case "phantom chronomancer":
                case "phantasm chronomancer":
                case "necrotic chronomancer":
                case "Draconic Chronomancer": // Never matches (className is lower case), as in C#.
                case "no class":
                case "nu metal necro":
                case "obsidian no class":
                case "protosartorium":
                case "shadow dragon shinobi":
                case "shadow ripper":
                case "shadow rocker":
                case "star captain":
                case "troubador of love":
                case "unchained rocker":
                case "unchained rockstar":
                case "undead goat":
                case "unundead goat":
                case "doom metal necro":
                case "neo metal necro":
                case "martial artist":
                case "master martial artist":
                case "antique hunter":
                case "archivist of time":
                    type = EnhancementType.Lucky;
                    wSpecial = WeaponSpecial.Awe_Blast;
                    break;
                //#endregion

                //#region Lucky - Health Vamp
                case "eternal inversionist":
                case "archfiend":
                case "barber":
                case "classic dragonlord":
                case "dragonslayer":
                case "enforcer":
                case "flame dragon warrior":
                case "rustbucket":
                case "sentinel":
                case "vampire":
                case "vampire lord":
                case "enchanted vampire lord":
                case "royal vampire lord":
                case "chrono shadowhunter":
                    type = EnhancementType.Lucky;
                    wSpecial = WeaponSpecial.Health_Vamp;
                    break;
                //#endregion

                //#endregion

                //#region Theif Region

                //#region Theif - Mana Vamp
                case "ninja":
                case "classic ninja":
                case "ninja (rare)":
                case "horc evader":
                    type = EnhancementType.Thief;
                    wSpecial = WeaponSpecial.Mana_Vamp;
                    break;
                //#endregion

                //#endregion

                //#region Wizard Region

                //#region Wizard - Awe Blast
                case "acolyte":
                case "arcane dark caster":
                case "battlemage":
                case "battlemage of love":
                case "blaze binder":
                case "blood sorceress":
                case "dark battlemage":
                case "dragon knight":
                case "firelord summoner":
                case "grim necromancer":
                case "highseas commander":
                case "infinity knight":
                case "interstellar knight":
                case "master of moglins":
                case "dark master of moglins":
                case "lich":
                case "mystical dark caster":
                case "northlands monk":
                case "royal battlemage":
                case "timeless dark caster":
                case "witch":
                case "stonecrusher":
                case "scion of flames":
                    type = EnhancementType.Wizard;
                    wSpecial = WeaponSpecial.Awe_Blast;
                    break;
                //#endregion

                //#region Wizard - Spiral Carve
                case "chrono dataknight":
                case "chrono dragonknight":
                case "cryomancer":
                case "dark caster":
                case "dark cryomancer":
                case "darkblood stormking":
                case "darkside":
                case "defender":
                case "frost spiritreaver":
                case "immortal dark caster":
                case "legion paladin":
                case "legion revenant":
                case "legion revenant (ioda)":
                case "lightcaster":
                case "pink romancer":
                case "psionic mindbreaker":
                case "pyromancer":
                case "sakura cryomancer":
                case "troll spellsmith":
                case "classic legion doomknight":
                case "legion doomknight":
                case "legion doomknight tester":
                case "arcana invoker":
                case "king\u0027s echo":
                    type = EnhancementType.Wizard;
                    wSpecial = WeaponSpecial.Spiral_Carve;
                    break;
                //#endregion

                //#region Wizard - Health Vamp
                case "daimon":
                case "dark lord":
                case "evolved shaman":
                case "lightmage":
                case "mindbreaker":
                case "vindicator of they":
                case "elemental dracomancer":
                case "lightcaster test":
                case "love caster":
                case "mage":
                case "classic mage":
                case "mage (rare)":
                case "sorcerer":
                case "the collector":
                    type = EnhancementType.Wizard;
                    wSpecial = WeaponSpecial.Health_Vamp;
                    break;
                //#endregion

                //#region Wizard - Mana Vamp
                case "oracle":
                case "shaman":
                    type = EnhancementType.Wizard;
                    wSpecial = WeaponSpecial.Mana_Vamp;
                    break;
                //#endregion

                //#endregion

                //#region Fighter Region

                //#region Fighter - Awe Blast
                case "deathknight":
                case "frostval barbarian":
                    type = EnhancementType.Fighter;
                    wSpecial = WeaponSpecial.Awe_Blast;
                    break;
                //#endregion

                //#endregion

                //#region Healer Region

                //#region Healer - Health Vamp
                case "dragon of time":
                    type = EnhancementType.Healer;
                    wSpecial = WeaponSpecial.Health_Vamp;
                    break;
                //#endregion

                //#region Healer - Mana Vamp
                case "obsidian paladin chronomancer":
                case "paladin chronomancer":
                    type = EnhancementType.Healer;
                    wSpecial = WeaponSpecial.Mana_Vamp;
                    break;
                //#endregion

                //#endregion

                default:
                    Core.logger(`SmartEnhance Failed: "${className}" is not found in the Smart Enhance Library, please report to @tato2`, "", { messageBox: true });
                    return;
            }
        };


        // If the item doesnt exist in the forge enh lib, or the player doesn't have the Forge enh unlocked, use Awe enh instead
        if (!forgeEnhancementLibrary()) aweEnhancementLibrary();

        // Can't be too careful
        if (type === null) {
            Core.logger(`SmartEnhance Failed: 'type' for ${className} is NULL`);
            return;
        }

        // If the class isn't enhanced yet, enhance it with the enhancement type
        if (selectedClass.enhancementLevel <= 0) {
            this.enhanceItem(selectedClass.name ?? className, type);
            // Used only for unenhanced classes usualy gotten from quests (ex: blood/scarlet sorccerer)
            if (forceEnh) return;
        }
        Core.equip(selectedClass.name ?? className);
        const lowerName = className;
        bot.wait.forTrue(() => (bot.player.currentClass ?? "").toLowerCase() === lowerName, 40);

        if (weaponOverride !== null) wSpecial = weaponOverride;

        this.enhanceEquipped(type, { cSpecial, hSpecial, wSpecial });
    }

    //#endregion
}

/** C# `Auras` (unused). */
export enum Auras {
    Shapeshifted,
    stuff2,
}

/** The generic boosts of gear, as named in the items' `meta` (C# `GenericGearBoost`). */
export enum GenericGearBoost {
    cp = "cp",
    gold = "gold",
    rep = "rep",
    exp = "exp",
    dmgAll = "dmgAll",
}

/** The racial boosts of gear, as named in the items' `meta` (C# `RacialGearBoost`). */
export enum RacialGearBoost {
    None = "None",
    Chaos = "Chaos",
    Dragonkin = "Dragonkin",
    Drakath = "Drakath",
    Elemental = "Elemental",
    Human = "Human",
    Orc = "Orc",
    Undead = "Undead",
}

/** The enhancement types, by enhancement pattern ID (C# `EnhancementType`). */
export enum EnhancementType {
    Fighter = 2,
    Thief = 3,
    Hybrid = 5,
    Wizard = 6,
    Healer = 7,
    SpellBreaker = 8,
    Lucky = 9,
}

/** The cape specials, by enhancement pattern ID (C# `CapeSpecial`). */
export enum CapeSpecial {
    None = 0,
    Forge = 10,
    Absolution = 11,
    Avarice = 12,
    Vainglory = 24,
    Penitence = 29,
    Lament = 30,
}

/** The weapon specials, by proc ID (C# `WeaponSpecial`). */
export enum WeaponSpecial {
    None = 0,
    Forge = 1,
    Spiral_Carve = 2,
    Awe_Blast = 3,
    Health_Vamp = 4,
    Mana_Vamp = 5,
    Powerword_Die = 6,
    Lacerate = 7,
    Smite = 8,
    Valiance = 9,
    Arcanas_Concerto = 10,
    Acheron = 11,
    Elysium = 12,
    Praxis = 13,
    Dauntless = 14,
    Ravenous = 15,
}

/** The helm specials, by enhancement pattern ID (C# `HelmSpecial`). */
export enum HelmSpecial {
    None = 0,
    Forge = 10,
    Vim = 25,
    Examen = 26,
    Anima = 28,
    Pneuma = 27,
    Hearty = 32,
}

/**
 * Maps stored numeric IDs to the current specials (C# `SpecialIdMapper`): some weapon and helm
 * special IDs were renumbered.
 */
export const SpecialIdMapper = {
    /** Legacy or current numeric ID to the {@link WeaponSpecial}. */
    weaponFromId(id: number): WeaponSpecial {
        // Legacy -> Current mappings (handle both old and new numeric IDs)
        switch (id) {
            case 0:
                return WeaponSpecial.Forge; // legacy/current
            case 1:
                return WeaponSpecial.None; // legacy/current
            case 2:
                return WeaponSpecial.Spiral_Carve;
            case 3:
                return WeaponSpecial.Awe_Blast;
            case 4:
                return WeaponSpecial.Health_Vamp;
            case 5:
                return WeaponSpecial.Mana_Vamp;
            case 6:
                return WeaponSpecial.Powerword_Die;
            case 7:
                return WeaponSpecial.Lacerate;
            case 8:
                return WeaponSpecial.Smite;
            case 9:
                return WeaponSpecial.Valiance;
            case 10:
                return WeaponSpecial.Arcanas_Concerto;
            case 11:
                return WeaponSpecial.Acheron;
            case 12:
                return WeaponSpecial.Elysium;
            case 13:
                return WeaponSpecial.Praxis;
            case 14:
                return WeaponSpecial.Dauntless;
            case 15:
                return WeaponSpecial.Ravenous;
            // Unknown/changed IDs: default to None
            default:
                return WeaponSpecial.None;
        }
    },

    /** Legacy or current numeric ID to the {@link HelmSpecial}. */
    helmFromId(id: number): HelmSpecial {
        switch (id) {
            case 0:
                return HelmSpecial.None;
            case 10:
                return HelmSpecial.Forge;
            case 25:
                return HelmSpecial.Vim;
            case 26:
                return HelmSpecial.Examen;
            case 27:
                return HelmSpecial.Pneuma;
            case 28:
                return HelmSpecial.Anima;
            case 32:
                return HelmSpecial.Hearty;
            default:
                return HelmSpecial.None;
        }
    },
};

/** The modes of {@link CoreAdvanced.startBuyAllMerge} (C# `mergeOptionsEnum`). */
export enum mergeOptionsEnum {
    all = 0,
    acOnly = 1,
    mergeMats = 2,
    select = 3,
}
