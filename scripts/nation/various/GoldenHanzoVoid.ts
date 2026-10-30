/*
name: GoldenHanzoVoid
description: null
tags: null
*/
// Port of Scripts/Nation/Various/GoldenHanzoVoid.cs.

import { bot } from "turu";
import { CoreAdvanced } from "../../core/CoreAdvanced";
import { Core } from "../../core/CoreBots";
import { CoreNation } from "../CoreNation";

const Nation = new CoreNation();
const Adv = new CoreAdvanced();

export class GoldenHanzoVoid {
    getGHV(): void {
        if (Core.checkInventory("Golden Hanzo Void"))
            return;

        Nation.approvalAndFavor(50, 0);
        Nation.farmTaintedGem(45);
        Nation.theAssistant("Dark Crystal Shard", 45);
        Nation.farmDiamondofNulgath(75);
        Nation.farmVoucher({ member:false });

        // Buy GHV
        Adv.buyItem("evilwarnul", 456, 27843, 1, 2694);
        bot.wait.forItemBuy();
    }
}

export default function main(): void {
        Core.setOptions();

        new GoldenHanzoVoid().getGHV();

        Core.setOptions(false);
}
