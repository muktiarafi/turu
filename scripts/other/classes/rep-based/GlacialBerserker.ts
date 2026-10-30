/*
name: GlacialBerserker
description: null
tags: null
*/
// Port of Scripts/Other/Classes/REP-based/GlacialBerserker.cs.

import { CoreAdvanced } from "../../../core/CoreAdvanced";
import { Core } from "../../../core/CoreBots";
import { CoreFarms } from "../../../core/CoreFarms";
import { GlaceraStory } from "../../../story/Glacera";

const Farm = new CoreFarms();
const Adv = new CoreAdvanced();
const Glacera = new GlaceraStory();

export class GlacialBerserker {
    getGB(rankUpClass = true): void {
        if (Core.checkInventory("Glacial Berserker")) {
            if (rankUpClass)
                Adv.rankUpClass("Glacial Berserker");
            return;
        }

        Glacera.doAll();
        Farm.glaceraREP();
        // Adv.BestGear(GenericGearBoost.rep);
        if (!Core.checkInventory([ 38049, 38084 ], 1, { any: true }))
            Adv.buyItem("icewindpass", 1339, "Glacial Berserker", 1, 22266);

        if (rankUpClass)
            Adv.rankUpClass("Glacial Berserker");
    }
}

export default function main(): void {
        Core.setOptions();

        new GlacialBerserker().getGB();

        Core.setOptions(false);
}
