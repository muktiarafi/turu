/*
name: DragonShinobi
description: null
tags: null
*/
// Port of Scripts/Other/Classes/DragonShinobi.cs.

import { bot } from "turu";
import { CoreAdvanced } from "../../core/CoreAdvanced";
import { ClassType, Core } from "../../core/CoreBots";
import { YokaiQuests } from "../../story/Yokai";

const Adv = new CoreAdvanced();
const Yokai = new YokaiQuests();

export class DragonShinobi {
    getDSS(rankUpClass = true): void {
        if (Core.checkInventory(59476)) {
            if (rankUpClass)
                Adv.rankUpClass("DragonSoul Shinobi");
            return;
        }

        Yokai.quests();
        Core.addDrop(20561);

        Core.equipClass(ClassType.Solo);
        Core.farmingLogger("Dragon Shinobi Token", 300);
        Core.registerQuests(7924);
        while (!bot.shouldExit && !Core.checkInventory(20561, 300))
            Core.huntMonster("shadowfortress", "1st Head of Orochi", null, 1, { log: false });
        Core.cancelRegisteredQuests();

        Adv.buyItem("shadowfortress", 1968, 59476, 1, 8078);

        if (rankUpClass)
            Adv.rankUpClass("DragonSoul Shinobi");
    }
}

export default function main(): void {
        Core.setOptions();
        new DragonShinobi().getDSS();

        Core.setOptions(false);
}
