/*
name: Paladin (Class)
description: This bot farms the Paladin class for you.
tags: warrior, healer, paladin, class
*/
// Port of Scripts/Good/Paladin.cs.

import { CoreAdvanced } from "../core/CoreAdvanced";
import { Core } from "../core/CoreBots";
import { CoreFarms } from "../core/CoreFarms";

const Farm = new CoreFarms();
const Adv = new CoreAdvanced();

export class Paladin {
    getPaladin(rankUpClass = true): void {
        if (Core.checkInventory("Paladin") || !Core.isMember)
            return;

        Core.buyItem("necropolis", 26, "Warrior");
        Core.buyItem("necropolis", 26, "Healer");

        Adv.rankUpClass("Warrior");
        Adv.rankUpClass("Healer");

        Farm.goodREP(5);

        Core.buyItem("necropolis", 26, "Paladin");

        if (rankUpClass)
            Adv.rankUpClass("Paladin");
    }
}

export default function main(): void {
        Core.setOptions();

        new Paladin().getPaladin();

        Core.setOptions(false);
}
