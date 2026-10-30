/*
name: Northlands Monk (Class)
description: This will kill the FrozenSoul Queen that drops the Northlands Monk (Class).
tags: northlands-monk, seasonal, class, frostvale
*/
// Port of Scripts/Seasonal/Frostvale/NorthlandsMonk.cs.

import { bot } from "turu";
import { CoreAdvanced } from "../../core/CoreAdvanced";
import { ClassType, Core } from "../../core/CoreBots";

const Adv = new CoreAdvanced();

export class NorthlandsMonk {
    getNlMonk(rankUpClass = true): void {
        if (!Core.isSeasonalMapActive("frozensoul"))
            return;

        if (Core.checkInventory(52413)) {
            Adv.rankUpClass("Northlands Monk", { gearRestore: true });
            return;
        }

        Core.addDrop(52413);

        Core.equipClass(ClassType.Solo);
        while (!bot.shouldExit && !Core.checkInventory("Northlands Monk"))
            Core.killMonster("frozensoul", "r4", "Left", "Frozensoul Queen", "Northlands Monk", 1, { isTemp: false });

        if (rankUpClass)
            Adv.rankUpClass("Northlands Monk", { gearRestore: true });
    }
}

export default function main(): void {
        Core.setOptions();
        new NorthlandsMonk().getNlMonk();

        Core.setOptions(false);
}
