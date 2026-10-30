/*
name: BloodSorceress
description: null
tags: null
*/
// Port of Scripts/Other/Classes/BloodSorceress.cs.

import { bot } from "turu";
import { CoreAdvanced } from "../../core/CoreAdvanced";
import { ClassType, Core } from "../../core/CoreBots";

const Adv = new CoreAdvanced();

export class BloodSorceress {
    getBSorc(rankUpClass = true): void {
        if (Core.checkInventory(/*Blood Sorceress*/36298)
        ) {
            if (rankUpClass) {
                Adv.rankUpClass("Blood Sorceress", { itemId: 36298 });
            }
            return;
        }

        Core.equipClass(ClassType.Solo);
        Core.killMonster("towerofmirrors", "r16", "Top", 32, 36298, 1, { isTemp: false, log: false, publicRoom: false });
        Core.jumpWait();
        bot.wait.forPickup(36298);

        if (rankUpClass)
            Adv.rankUpClass("Blood Sorceress", { itemId: 36298 });
    }
}

export default function main(): void {
        Core.setOptions();

        new BloodSorceress().getBSorc();

        Core.setOptions(false);
}
