/*
name: PurifiedClaymoreOfDestiny
description: null
tags: null
*/
// Port of Scripts/Nation/Various/PurifiedClaymoreOfDestiny.cs.

import { Core } from "../../core/CoreBots";
import { CoreFarms } from "../../core/CoreFarms";

const Farm = new CoreFarms();

export class PurifiedClaymoreOfDestiny {
    getPCoD(): void {
        if (Core.checkInventory("Purified Claymore of Destiny"))
            return;

        Core.addDrop("Purified Claymore of Destiny");

        Farm.experience(15);
        Farm.goodREP(8);

        Core.ensureAccept(548);
        Core.huntMonster("battleundera", "Undead Berserker", "Warrior Claymore Blade", 1, { isTemp: false });
        Core.ensureComplete(548);
    }
}

export default function main(): void {
        Core.setOptions();

        new PurifiedClaymoreOfDestiny().getPCoD();

        Core.setOptions(false);
}
