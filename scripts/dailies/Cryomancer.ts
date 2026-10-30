/*
name: Cryomancer Daily
description: Cryomancer
tags: daily, cryomancer, class, seasonal
*/
// Port of Scripts/Dailies/Cryomancer.cs.

import { CoreAdvanced } from "../core/CoreAdvanced";
import { Core } from "../core/CoreBots";
import { CoreDailies } from "../core/CoreDailies";
import { GlaceraStory } from "../story/Glacera";

const Daily = new CoreDailies();
const Glac = new GlaceraStory();
const Adv = new CoreAdvanced();

export class Cryomancer {
    doCryomancer(rankUpClass = true): void {
        if (Core.checkInventory("Cryomancer")) {
            if (rankUpClass)
                Adv.rankUpClass("Cryomancer");
            return;
        }

        // Enable Cryomancer bool to return early
        Glac.doAll(true);

        Daily.cryomancer();

        if (rankUpClass)
            Adv.rankUpClass("Cryomancer");
    }
}

export default function main(): void {
        Core.setOptions();

        new Cryomancer().doCryomancer();

        Core.setOptions(false);
}
