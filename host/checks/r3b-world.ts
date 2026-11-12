/*
name: R3b world check
description: Walks, moves between cells and reads a shop using only R2-R3b's callbacks, so it runs on turu.swf alone and on VibeSkua's SWF (compare their outgoing packets). Start in Battleon (/join battleon) with a shop opened by hand (e.g. Battleon's Yulgar's Inn shop) and closed again. It buys nothing.
*/
import { bot } from "turu";

export default function main(): void {
    const call = (name: string, ...args: unknown[]): string => bot.flash.call(name, ...args);
    bot.log(`logged in: ${call("isLoggedIn")}, kicked: ${call("isKicked")}, gender: ${call("gender")}`);
    bot.log(`bag: ${call("inventoryBagUsedSlots")} used, categories: ${call("hasInventoryCategories")}`);
    call("walkTo", 400, 350, 8);
    bot.sleep(2000);
    call("walkTo", 600, 380, 8);
    bot.sleep(2000);
    call("jumpCorrectRoom", "Enter", "Spawn", true, false);
    bot.sleep(2000);
    call("jumpCorrectRoom", "Enter", "NoSuchPad", true, false);
    bot.sleep(3000);
    const item = bot.flash.call("getGameObject", "world.shopinfo.items.0");
    bot.log(`first shop item: ${item.slice(0, 160)}`);
    try {
        const parsed = JSON.parse(item) as { ItemID: number; ShopItemID: number };
        bot.log(`unmet: ${call("getUnmetPurchaseRequirements", parsed.ItemID, parsed.ShopItemID, 1)}`);
        bot.log(`space: ${call("inventoryHasSpaceFor", item, 1, true)}`);
    } catch {
        bot.log("no shop loaded: open one by hand first");
    }
    bot.log(`modal: [${call("getActiveModalMessage")}]`);
}
