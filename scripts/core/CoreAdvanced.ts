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
 * the item needs first. Only the shop path is here.
 */

import { bot } from "turu";
import { Core, type CoreShopItem } from "./CoreBots";
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
}
