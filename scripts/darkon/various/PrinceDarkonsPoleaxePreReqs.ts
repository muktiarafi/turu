/*
name: Prince Darkon's Poleaxe Pre Reqs
description: This script will farm Prince Darkon's Poleaxe Pre-Reqs.
tags: darkon, poleaxe, prereqs, teeth, melody, receipt, gratitude, astravian medal, prince
*/
// Port of Scripts/Darkon/Various/PrinceDarkonsPoleaxePreReqs.cs.

import { bot } from "turu";
import { Core } from "../../core/CoreBots";
import { CoreFarms } from "../../core/CoreFarms";
import { CoreDarkon } from "../CoreDarkon";

const Darkon = new CoreDarkon();
const Farm = new CoreFarms();

export class PrinceDarkonsPoleaxePreReqs {
    farmPreReqs(buyPoleaxe = false): void {
        if (Core.checkInventory("Prince Darkon's Poleaxe"))
            return;

        Darkon.aMelody(22);
        Darkon.farmReceipt(22);
        Darkon.teeth(22);
        Darkon.lasGratitude(22);
        Darkon.astravianMedal(22);

        if (!Core.checkInventory("Darkon's Instant Noodle", 22)) {
            Farm.gold(48888884);
            Core.buyItem("garden", 1831, "Darkon's Instant Noodle", 22);
        } //BuyPoleaxe();

        if (buyPoleaxe)
            this.buyPoleaxe();
    }

    private buyPoleaxe(): void {
        if (!Core.checkInventory("Prince Darkon's Poleaxe")) {
            if (
                !Core.checkInventory("Algie's Bow")
                && Core.checkInventory("King Drago Insignia", 5)
            ) {
                Core.buyItem("ultradrago", 2066, "Algie's Bow");
                bot.wait.forItemBuy();
            }

            if (
                !Core.checkInventory("Dene's Axe") && Core.checkInventory("King Drago Insignia", 5)
            ) {
                Core.buyItem("ultradrago", 2066, "Dene's Axe");
                bot.wait.forItemBuy();
            }

            if (Core.checkInventory("King Drago Insignia", 10)) {
                Core.buyItem("ultradrago", 2066, "Prince Darkon's Poleaxe");
                bot.wait.forItemBuy();
            }
        }
    }
}

export default function main(): void {
        Core.setOptions();

        new PrinceDarkonsPoleaxePreReqs().farmPreReqs();

        Core.setOptions(false);
}
