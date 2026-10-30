/*
name: null
description: null
tags: null
*/
// Port of Scripts/Hollowborn/CoreHollowborn.cs.
// Not ported (FarmerJoe's DoAll doesn't reach them): HardcoreContract, HumanSoul, HBLycanClaw, HBVampireFang, HBHollowbornResidue, HBWrit.

import { bot } from "turu";
import { Core } from "../core/CoreBots";
import { CoreFarms } from "../core/CoreFarms";

const Farm = new CoreFarms();

export class CoreHollowborn {
    freshSouls(Uni36Quant = 3, FSQuant = 1000): void {
        if (
            Core.checkInventory("Unidentified 36", Uni36Quant)
            && Core.checkInventory("Fresh Soul", FSQuant)
        )
            return;

        Farm.experience(50);

        Core.addDrop("Unidentified 36");
        // "Fresh Soul"
        Core.addDrop(52588);
        Core.registerQuests(7293);

        if (FSQuant > 0)
            Core.farmingLogger("Fresh Soul", FSQuant);
        if (Uni36Quant > 0)
            Core.farmingLogger("Unidentified 36", Uni36Quant);

        while (
            !bot.shouldExit
            && (
                !Core.checkInventory("Unidentified 36", Uni36Quant)
                || !Core.checkInventory(52588, FSQuant)
            )
        ) {
            Core.huntMonster("citadel", "Inquisitor Guard", "Fresh Soul?", 10, { log: false });
            bot.wait.forPickup(52588);
        }
        Core.cancelRegisteredQuests();
    }
}
