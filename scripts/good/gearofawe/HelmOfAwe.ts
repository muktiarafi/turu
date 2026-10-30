/*
name: Helm of Awe
description: helm, awe, gear, reputation, gold, experience, class, points, cp, boost
tags: this bot will farm the helm of awe for you
*/
// Port of Scripts/Good/GearOfAwe/HelmOfAwe.cs.

import { ClassType, Core } from "../../core/CoreBots";
import { CoreAwe } from "./CoreAwe";

const Awe = new CoreAwe();

export class HelmOfAwe {
    getHoA(): void {
        if (Core.checkInventory("Helm of Awe"))
            return;

        Core.equipClass(ClassType.Solo);
        Awe.getAweRelic("Helm", 4175, 10, 5, "doomvaultb", "Undead Raxgore");
        Core.buyItem("museum", 1129, "Helm of Awe");

        Core.toBank("Legendary Awe Pass", "Guardian Awe Pass", "Armor of Awe Pass");
    }
}

export default function main(): void {
        Core.setOptions();

        new HelmOfAwe().getHoA();

        Core.setOptions(false);
}
