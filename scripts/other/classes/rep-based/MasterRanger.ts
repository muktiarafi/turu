/*
name: MasterRanger
description: null
tags: null
*/
// Port of Scripts/Other/Classes/REP-based/MasterRanger.cs.

import { CoreAdvanced } from "../../../core/CoreAdvanced";
import { Core } from "../../../core/CoreBots";

const Adv = new CoreAdvanced();

export class MasterRanger {
    getMR(rankUpClass = true): void {
        if (Core.checkInventory("Master Ranger")) {
            if (rankUpClass)
                Adv.rankUpClass("Master Ranger");
            return;
        }

        Adv.buyItem("sandsea", 242, 7260, 1, 5682);

        if (rankUpClass)
            Adv.rankUpClass("Master Ranger");
    }
}

export default function main(): void {
        Core.setOptions();

        new MasterRanger().getMR();

        Core.setOptions(false);
}
