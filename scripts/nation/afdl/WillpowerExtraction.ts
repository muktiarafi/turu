/*
name: WillpowerExtraction
description: null
tags: null
*/
// Port of Scripts/Nation/AFDL/WillpowerExtraction.cs.

import { bot } from "turu";
import { CoreAdvanced } from "../../core/CoreAdvanced";
import { ClassType, Core } from "../../core/CoreBots";
import { CoreFarms } from "../../core/CoreFarms";
import { CoreNation } from "../CoreNation";

const Farm = new CoreFarms();
const Adv = new CoreAdvanced();
const Nation = new CoreNation();

export class WillpowerExtraction {
    unidentified34(quant = 300): void {
        if (Core.checkInventory("Unidentified 34", quant))
            return;

        Core.farmingLogger("Unidentified 34", quant);

        Core.addDrop(
            ...Nation
                .bagDrops.concat(Nation.tercessBags)
                .concat(
                    [
                        "Unidentified 34",
                        "Unidentified 19",
                        "Necrot",
                        "Chaoroot",
                        "Doomatter",
                        "Facebreakers of Nulgath",
                        "SightBlinder Axes of Nulgath",
                        "Mystic Tribal Sword",
                        "Golden Shadow Breaker",
                        "Shadow Terror Axe",
                    ]));

        while (!bot.shouldExit && !Core.checkInventory("Unidentified 34", quant)) {
            Core.ensureAccept(5258);

            Adv.buyItem("shadowfall", 89, "Shadow Lich");

            if (!Core.checkInventory(Nation.uni(19))) {
                if (Core.isMember)
                    Adv.buyItem("tercessuinotlim", 1951, "Unidentified 19");
                else
                    Nation.supplies("Unidentified 19");
            }
          
            if (!Core.checkInventory("Necrot", 5)) {
                Adv.buyItem("tercessuinotlim", 1951, "Necrot", 10);
                bot.wait.forItemBuy();
            }
           
            if (!Core.checkInventory("Chaoroot", 5)) {
                Adv.buyItem("tercessuinotlim", 1951, "Chaoroot", 10);
                bot.wait.forItemBuy();
            }
           
            if (!Core.checkInventory("Doomatter", 5)) {
                Adv.buyItem("tercessuinotlim", 1951, "Doomatter", 10);
                bot.wait.forItemBuy();
            }

            // "Facebreakers of Nulgath"
            if (!Core.checkInventory(18768)) {
                Core.addDrop(18768);
                // "Kindness" of Nulgath
                Core.ensureAccept(3046);
                Nation.farmUni13(1);
                Core.equipClass(ClassType.Solo);
                Core.huntMonster("citadel", "Grand Inquisitor", "Golden Shadow Breaker", 1, { isTemp: false });
                Core.huntMonster("battleundera", "Bone Terror", "Shadow Terror Axe", 1, { isTemp: false });
                Core.equipClass(ClassType.Farm);
                Nation.farmDarkCrystalShard(5);
                Nation.farmTaintedGem(5);
                Nation.farmDiamondofNulgath(1);
                Core.ensureComplete(3046, 18768);

                bot.wait.forDrop(18768);
                bot.wait.forPickup(18768);
            }

            Adv.buyItem("arcangrove", 214, "Mystic Tribal Sword");

            Nation.approvalAndFavor(50, 0);

            Nation.farmTotemofNulgath(1);

            Core.ensureComplete(5258);
            bot.wait.forQuestComplete(5258);
            bot.wait.forPickup("Unidentified 34");

        }
    }
}

export default function main(): void {
        Core.bankingBlackList.push(...
            Nation
                .bagDrops.concat(Nation.tercessBags)
                .concat(
                    [
                        "Unidentified 34",
                        "Unidentified 19",
                        "Necrot",
                        "Chaoroot",
                        "Doomatter",
                        "Mortality Cape of Revontheus",
                        "Facebreakers of Nulgath",
                        "SightBlinder Axes of Nulgath",
                        "Mystic Tribal Sword",
                        "King Klunk's Crown",
                        "Golden Shadow Breaker",
                        "Shadow Terror Axe",
                    ]));

        Core.setOptions();

        new WillpowerExtraction().unidentified34();

        Core.setOptions(false);
}
