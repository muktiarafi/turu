/*
name: 0MaxBags
description: Farms the max stack of all the nation materials using various methods based off the pets / quest you have avaiable.
tags: nation, nulgath, bags, all, 🖕
*/
// Port of Scripts/Nation/Materials/0MaxBags.cs (includes CoreBots, CoreFarms, CoreNation).

import { Core } from "../../core/CoreBots";
import { CoreNation } from "../CoreNation";

const Nation = new CoreNation();

export default function main(): void {
    Core.bankingBlackList.push(...Nation.bagDrops.slice(0, 9));
    Core.setOptions();

    maxBags();

    Core.setOptions(false);
}

function maxBags(): void {
    Core.addDrop(...Nation.bagDrops.slice(0, 9));

    Nation.farmBloodGem();
    Nation.farmDarkCrystalShard();
    Nation.farmDiamondofNulgath();
    Nation.farmTotemofNulgath();
    Nation.farmGemofNulgath();
    Nation.farmTaintedGem();
    Nation.farmUni10();
    Nation.farmUni13();
    Nation.farmVoucher(true);
    Nation.farmVoucher(false);
}
