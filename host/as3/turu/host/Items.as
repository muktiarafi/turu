package turu.host {
import flash.display.DisplayObject;
import flash.display.DisplayObjectContainer;
import flash.events.MouseEvent;
import flash.system.ApplicationDomain;
import flash.utils.getQualifiedClassName;

/**
 * Inventory, shop and drops (docs/VIBESKUA_SPEC.md, "Inventory", "Shop" and `rejectExcept`).
 * The client's `InvCat` (inventory pools) and `liteAssets.draw.FavStore` (favourites) are
 * looked up in the game's domain; older clients without `InvCat` get VibeSkua's fallback.
 */
public class Items {
    /** Returns the game, or null before it loads. */
    private var gameOf:Function;

    public function Items(gameOf:Function) {
        this.gameOf = gameOf;
    }

    /** Registers the calls with `expose(name, fn, onThrow, quiet)`; reads are quiet. */
    public function exposeWith(expose:Function):void {
        expose("hasInventoryCategories", hasInventoryCategories, undefined, true);
        expose("inventoryBagUsedSlots", inventoryBagUsedSlots, undefined, true);
        expose("inventoryHasSpaceFor", inventoryHasSpaceFor, undefined, true);
        expose("isFavoriteItem", isFavoriteItem, undefined, true);
        expose("getUnmetPurchaseRequirements", getUnmetPurchaseRequirements, undefined, true);
        expose("buyItemByName", buyItemByName);
        expose("buyItemByID", buyItemByID);
        expose("rejectExcept", rejectExcept);
    }

    private function get world():* {
        return gameOf().world;
    }

    /** A class of the game's by its full name, or null. */
    private function definition(name:String):* {
        var domain:ApplicationDomain = gameOf().loaderInfo.applicationDomain;
        return domain.hasDefinition(name) ? domain.getDefinition(name) : null;
    }

    private function hasInventoryCategories():String {
        return String(definition("InvCat") != null);
    }

    private function inventoryBagUsedSlots():String {
        var items:Array = world.myAvatar.items;
        var invCat:* = definition("InvCat");
        return String(invCat != null ? invCat.countBag(items) : items.length);
    }

    /**
     * Whether the bag (or the house, for house items) has room for `quantity` of the item, after
     * the `outgoing` item (one given up for it, as in a merge) leaves.
     */
    private function inventoryHasSpaceFor(itemJson:*, quantity:* = 1, purchase:* = false, outgoingJson:* = null):String {
        var item:Object = JSON.parse(String(itemJson));
        var qty:int = int(quantity);
        var me:* = world.myAvatar;
        var invCat:* = definition("InvCat");
        if (invCat == null) {
            for each (var owned:* in me.items) {
                if (owned.ItemID == item.ItemID && int(owned.iQty) < int(owned.iStk)) {
                    return "true";
                }
            }
            return String(me.items.length < int(me.objData.iBagSlots));
        }
        var house:Boolean = invCat.isHouse(item);
        var items:Array = house ? me.houseitems : me.items;
        var slots:int = int(house ? me.objData.iHouseSlots : me.objData.iBagSlots);
        if (outgoingJson != null && String(outgoingJson) != "") {
            slots += int(invCat.freedBy(items, JSON.parse(String(outgoingJson)), qty));
        }
        return String(!invCat.isFullFor(items, item, qty, slots));
    }

    private function isFavoriteItem(id:*):String {
        var store:* = definition("liteAssets.draw.FavStore");
        return String(store != null && store.has(int(id)));
    }

    /** The shop's item `id` (with `shopItemId`, or any of the shop's when it's -1). */
    private function shopItem(id:*, shopItemId:*):* {
        var wanted:Number = Number(shopItemId);
        for each (var item:* in world.shopinfo.items) {
            if (item.ItemID == Number(id) && (wanted == -1 || item.ShopItemID == wanted)) {
                return item;
            }
        }
        return null;
    }

    /** Why buying `quantity` of the item would fail: quest, materials, tokens. */
    private function getUnmetPurchaseRequirements(id:*, shopItemId:*, quantity:*):String {
        var item:* = shopItem(id, shopItemId);
        var unmet:Array = [];
        if (item == null) {
            unmet.push("The shop doesn't sell this item.");
            return JSON.stringify(unmet);
        }
        var qty:int = Math.max(1, int(quantity));
        if (int(item.iQSindex) >= 0 && item.iQSvalue != null
            && int(world.getQuestValue(int(item.iQSindex))) < int(item.iQSvalue)) {
            unmet.push("Quest requirement not met.");
        }
        for each (var material:* in item.turnin) {
            var need:int = int(material.iQty) * qty;
            var have:int = owned(material.ItemID);
            if (have < need) {
                unmet.push("Missing " + material.sName + " x" + (need - have) + ".");
            }
        }
        if (int(item.TokID) > 0) {
            var cost:int = int(item.TokQty) * qty;
            var tokens:int = owned(item.TokID);
            if (tokens < cost) {
                unmet.push("Not enough " + (item.TokName != null ? item.TokName : "tokens") + ": " + tokens + "/" + cost + ".");
            }
        }
        return JSON.stringify(unmet);
    }

    private function owned(id:*):int {
        var entry:* = world.invTree[id];
        return entry != null ? int(entry.iQty) : 0;
    }

    private function buyItemByName(name:*, quantity:* = -1):void {
        var wanted:String = String(name).toLowerCase();
        for each (var item:* in world.shopinfo.items) {
            if (String(item.sName).toLowerCase() == wanted) {
                buy(item, int(quantity));
                return;
            }
        }
    }

    private function buyItemByID(id:*, shopItemId:*, quantity:* = -1):void {
        var item:* = shopItem(id, shopItemId);
        if (item != null) {
            buy(item, int(quantity));
        }
    }

    private function buy(item:*, quantity:int):void {
        if (quantity == -1) {
            world.sendBuyItemRequest(item);
        } else {
            world.sendBuyItemRequestWithQuantity({iSel: item, iQty: quantity, accept: 1});
        }
    }

    /**
     * Declines every drop not named in `csv` (lowercase names, comma-separated): in AQW Lite's
     * custom drop list when it's on, else the drop frames' No buttons.
     */
    private function rejectExcept(csv:*):String {
        var keep:Object = {};
        for each (var name:String in String(csv).split(",")) {
            keep[name] = true;
        }
        var game:* = gameOf();
        if (game.litePreference.data.bCustomDrops) {
            var drops:* = game.cDropsUI;
            for each (var item:* in drops.invTree.concat()) {
                if (keep[String(item.sName).toLowerCase()] !== true) {
                    drops.onBtNo(item);
                }
            }
            return "true";
        }
        var stack:DisplayObjectContainer = game.ui.dropStack;
        var frames:Array = [];
        for (var i:int = 0; i < stack.numChildren; i++) {
            frames.push(stack.getChildAt(i));
        }
        for each (var frame:* in frames) {
            if (getQualifiedClassName(frame).indexOf("DFrame2MC") < 0) {
                continue;
            }
            var match:Array = /(.*)\s+x\s*(\d*)/.exec(String(frame.cnt.strName.text));
            var dropName:String = match != null ? match[1] : String(frame.cnt.strName.text);
            if (keep[dropName.toLowerCase()] !== true) {
                DisplayObject(frame.cnt.nbtn).dispatchEvent(new MouseEvent(MouseEvent.CLICK));
            }
        }
        return "true";
    }
}
}
