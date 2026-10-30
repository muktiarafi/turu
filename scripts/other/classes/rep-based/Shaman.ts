/*
name: Shaman
description: null
tags: null
*/
// Port of Scripts/Other/Classes/REP-based/Shaman.cs.

import { CoreAdvanced } from "../../../core/CoreAdvanced";
import { Core } from "../../../core/CoreBots";

const Adv = new CoreAdvanced();

export class Shaman {
    getShaman(rankUpClass = true): void {
        if (Core.checkInventory("Shaman")) {
            if (rankUpClass)
                Adv.rankUpClass("Shaman");
            return;
        }

        Adv.buyItem("arcangrove", 214, 5765, 1, 4799);

        if (rankUpClass)
            Adv.rankUpClass("Shaman");
    }
}

export default function main(): void {
        Core.setOptions();

        new Shaman().getShaman();

        Core.setOptions(false);
}
