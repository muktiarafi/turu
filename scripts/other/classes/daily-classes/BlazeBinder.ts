/*
name: BlazeBinder
description: null
tags: null
*/
// Port of Scripts/Other/Classes/Daily-Classes/BlazeBinder.cs.

import { bot } from "turu";
import { CoreAdvanced } from "../../../core/CoreAdvanced";
import { Core } from "../../../core/CoreBots";
import { CoreDailies } from "../../../core/CoreDailies";

const Dailies = new CoreDailies();
const Adv = new CoreAdvanced();

export class BlazeBinder {
    getClass(rankUpClass = true): void {
        if (Core.checkInventory("Blaze Binder")) {
            if (rankUpClass)
                Adv.rankUpClass("Blaze Binder");
            return;
        }

        //Aquire the Pyromancer Class
        Dailies.pyromancer();

        // Get atlest one ClassPoint
        const itemInv = bot.inventory.items.find(i =>
            i.name.toLowerCase() === ("Pyromancer").toLowerCase() && i.category === "Class")!; // C# First() throws when missing
        if (itemInv.quantity < 1) {
            Adv.gearStore(false, { enhAfter: true });
            Core.equip("Pyromancer");
            Core.logger("Getting *1* point in Pyro for Blaze Binder");
            Core.join("Noobshire");
            bot.kill.monster("*");
            Adv.gearStore(true, { enhAfter: true });
        }
        Adv.buyItem("fireforge", 1142, "Darkness Sigil");
        Adv.buyItem("fireforge", 1142, "Flame Sigil");
        Adv.buyItem("fireforge", 1140, "Blaze Binder");
        Core.sleep();
        Adv.rankUpClass("Blaze Binder");
    }
}

export default function main(): void {
        Core.setOptions();

        new BlazeBinder().getClass();

        Core.setOptions(false);
}
