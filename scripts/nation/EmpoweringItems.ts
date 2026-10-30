/*
name: EmpoweringItems
description: null
tags: null
*/
// Port of Scripts/Nation/EmpoweringItems.cs.

import { bot } from "turu";
import { Core } from "../core/CoreBots";
import { CoreNation } from "./CoreNation";

const Nation = new CoreNation();

export class EmpoweringItems {
    empoweringStuff(): void {
        if (Core.checkInventory("Death Scythe of Nulgath"))
            return;

        Core.addDrop("Death Scythe of Nulgath");

        Core.ensureAccept(558);
        Nation.farmUni13(1);
        Nation.farmDiamondofNulgath(10);
        if (!Core.checkInventory(4867)) {
            Core.ensureAccept(623);
            Nation.farmUni13(1);
            Core.huntMonster("willowcreek", "Hidden Spy", "The Secret 1", 1, { isTemp: false });
            Core.ensureComplete(623);
            bot.wait.forPickup(4867);
        }
        Core.huntMonster("underworld", "Undead Bruiser", "Undead Bruiser Sigil");
        Core.ensureComplete(558);
        bot.wait.forPickup("Death Scythe of Nulgath");
    }
}

export default function main(): void {
        Core.setOptions();

        new EmpoweringItems().empoweringStuff();

        Core.setOptions(false);
}
