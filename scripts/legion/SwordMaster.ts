/*
name: SwordMaster (Class)
description: This script will get SwordMaster class and rank it up to rank 10.
tags: sword master, swordmaster, legion, class, meditation, ynr
*/
// Port of Scripts/Legion/SwordMaster.cs.

import { CoreAdvanced, EnhancementType } from "../core/CoreAdvanced";
import { Core } from "../core/CoreBots";
import { CoreLegion } from "./CoreLegion";

const Adv = new CoreAdvanced();
const Legion = new CoreLegion();

export class SwordMaster {
    getSwordMaster(rankUpClass = true): void {
        if (Core.checkInventory("SwordMaster"))
            return;

        Legion.farmLegionToken(2000);
        Core.buyItem("underworld", 238, "SwordMaster", 1);
        if (rankUpClass) {
            Adv.enhanceItem("SwordMaster", EnhancementType.Lucky);
            Adv.rankUpClass("SwordMaster");
        }
    }
}

export default function main(): void {
        Core.setOptions();

        new SwordMaster().getSwordMaster();

        Core.setOptions(false);
}
