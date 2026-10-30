/*
name: BurningBlade
description: null
tags: null
*/
// Port of Scripts/Other/Weapons/BurningBlade.cs.

import { bot } from "turu";
import { ClassType, Core } from "../../core/CoreBots";

export class BurningBlade {
    getBurningBlade(): void {
        if (Core.checkInventory(31058))
            return;

        Core.equipClass(ClassType.Solo);
        Core.killMonster("lostruinswar", "r7", "Left", "Diabolical Warlord", "Burning Blade", 1, { isTemp: false, publicRoom: true });
        bot.wait.forPickup("Burning Blade");
    }
}

export default function main(): void {
        Core.setOptions();

        new BurningBlade().getBurningBlade();

        Core.setOptions(false);
}
