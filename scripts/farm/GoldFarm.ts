/*
name: Gold Farm
description: honor hall for members, BGE for f2p
tags: gold, battle ground e, honor hall, berserker bunny
*/
// Port of Scripts/Farm/GoldFarm.cs (includes CoreBots, CoreFarms; CoreAdvanced is included but
// never used by this script).

import { bot } from "turu";
import { ClassType, Core } from "../core/CoreBots";
import { CoreFarms } from "../core/CoreFarms";

const Farm = new CoreFarms();

export default function main(): void {
    Core.setOptions(true, { disableClassSwap: true });

    doFarmGold();

    Core.setOptions(false);
}

export function doFarmGold(): void {
    Core.equipClass(ClassType.Farm);
    bot.drops.start();

    Farm.gold();
}
