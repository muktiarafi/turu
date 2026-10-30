/*
name: BurningBladeOfAbezeth
description: null
tags: null
*/
// Port of Scripts/Other/Weapons/BurningBladeOfAbezeth.cs.

import { CoreAdvanced, EnhancementType } from "../../core/CoreAdvanced";
import { ClassType, Core } from "../../core/CoreBots";
import { CelestialArenaQuests } from "../../story/queenofmonsters/extra/CelestialArena";

const Adv = new CoreAdvanced();
const CAQ = new CelestialArenaQuests();

export class BurningBladeOfAbezeth {
    getBBoA(): void {
        if (Core.checkInventory("Burning Blade Of Abezeth")) //Other method until it's fixed
            return;

        CAQ.doAll();

        // CC.BuyAllMerge("Burning Blade Of Abezeth", mergeOptionsEnum.select);
        Core.equipClass(ClassType.Solo);
        Core.huntMonster("celestialarenad", "Aranx", "Champion Sash", 20, { isTemp: false });
        Adv.buyItem("celestialarena", 1474, "Burning Blade Of Abezeth");
        Adv.enhanceItem("Burning Blade Of Abezeth", EnhancementType.Lucky);
    }
}

export default function main(): void {
        Core.setOptions();

        new BurningBladeOfAbezeth().getBBoA();

        Core.setOptions(false);
}
