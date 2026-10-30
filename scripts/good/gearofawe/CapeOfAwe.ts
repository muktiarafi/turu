/*
name: Cape of Awe
description: This bot will farm the Cape of Awe for you
tags: cape, awe, boost, experience, reputation, class, points, gold, cp
*/
// Port of Scripts/Good/GearOfAwe/CapeOfAwe.cs.

import { CoreAdvanced } from "../../core/CoreAdvanced";
import { Core } from "../../core/CoreBots";
import { CoreAwe } from "./CoreAwe";

const Adv = new CoreAdvanced();
const Awe = new CoreAwe();

export class CapeOfAwe {
    getCoA(): void {
        if (Core.checkInventory("Cape of Awe"))
            return;

        Awe.getAweRelic("Cape", 4178, 1, 1, "doomvault", "Binky");
        Adv.buyItem("museum", 1129, "Cape of Awe");

        Core.toBank("Legendary Awe Pass", "Guardian Awe Pass", "Armor of Awe Pass");
    }
}

export default function main(): void {
        Core.setOptions();

        new CapeOfAwe().getCoA();

        Core.setOptions(false);
}
