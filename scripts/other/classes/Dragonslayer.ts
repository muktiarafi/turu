/*
name: Dragonslayer Class
description: This script farms the Dragonslayer Class
tags: galanoth, class, lair, dsg, general, slayer
*/
// Port of Scripts/Other/Classes/Dragonslayer.cs.

import { CoreAdvanced } from "../../core/CoreAdvanced";
import { Core } from "../../core/CoreBots";
import { Lair } from "../../story/Lair";

const Adv = new CoreAdvanced();
const lair = new Lair();

export class Dragonslayer {
    getDragonslayer(rankUpClass = true): void {
        if (Core.checkInventory("Dragonslayer")) {
            if (rankUpClass)
                Adv.rankUpClass("Dragonslayer");
            return;
        }

        lair.galanoth();
        Core.buyItem("lair", 38, "Dragonslayer");
        if (rankUpClass)
            Adv.rankUpClass("Dragonslayer");
    }
}

export default function main(): void {
        Core.setOptions();

        new Dragonslayer().getDragonslayer();
        Core.setOptions(false);
}
