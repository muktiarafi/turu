/*
name: Free Boosts [10mns]
description: Farms Free Boosts from "Reagents for Zifwin" (cp/gold/rep)
tags: reagents for zifwin, free, boost, class points, reputation, gold
*/
// Port of Scripts/Other/FreeBoosts/FreeBoostsQuest(10mns)[Rng].cs.

import { bot, defineScriptOptions, option } from "turu";
import { ClassType, Core, configOr } from "../../core/CoreBots";
import { CoreFarms } from "../../core/CoreFarms";

const Farm = new CoreFarms();

export class FreeBoosts {
    getBoostsSelect(GoldBoostQuant = 9999, ClassBoostQuant = 9999, RepBoostQuant = 9999): void {
        Core.addDrop("GOLD Boost! (10 min)", "CLASS Boost! (10 min)", "REPUTATION Boost! (10 min)");
        Core.oneTimeMessage(
            "RNG Quest Rewards", 
            "Drops are \"randomly\" received, and may take a while... be prepared if quantities are high.");

        if (!Core.checkInventory("GOLD Boost! (10 min)", GoldBoostQuant))
            Core.farmingLogger("GOLD Boost! (10 min)", GoldBoostQuant);
        if (!Core.checkInventory("CLASS Boost! (10 min)", ClassBoostQuant))
            Core.farmingLogger("CLASS Boost! (10 min)", ClassBoostQuant);
        if (!Core.checkInventory("REPUTATION Boost! (10 min)", RepBoostQuant))
            Core.farmingLogger("REPUTATION Boost! (10 min)", RepBoostQuant);

        Core.equipClass(ClassType.Farm);

        const allQuantitiesMet =
            Core.checkInventory("GOLD Boost! (10 min)", GoldBoostQuant)
            && Core.checkInventory("CLASS Boost! (10 min)", ClassBoostQuant)
            && Core.checkInventory("REPUTATION Boost! (10 min)", RepBoostQuant);

        while (!bot.shouldExit && !allQuantitiesMet) {
            Core.huntMonsterQuest(
                6208, 
                ["bloodtusk", "Trollola Plant", ClassType.Solo],
                ["cloister", "Acornent", ClassType.Farm],
                ["nibbleon", "Dark Makai", ClassType.Farm]);

            if (allQuantitiesMet)
                break; // Exit the loop when all quantities are met.

            const goldItem = bot.inventory.items.find(x =>
                x.name === "GOLD Boost! (10 min)");
            const classItem = bot.inventory.items.find(x =>
                x.name === "CLASS Boost! (10 min)");
            const repItem = bot.inventory.items.find(x =>
                x.name === "REPUTATION Boost! (10 min)");

            if (goldItem != null)
                goldItem.quantity = Math.min(GoldBoostQuant, goldItem.maxStack);

            if (classItem != null)
                classItem.quantity = Math.min(ClassBoostQuant, classItem.maxStack);

            if (repItem != null)
                repItem.quantity = Math.min(RepBoostQuant, repItem.maxStack);
        }

        Core.cancelRegisteredQuests();
    }
}

export enum Booster {
    Gold = 7140,
    Class = 8877,
    Reputation = 8879,
}

export const scriptOptions = defineScriptOptions("Booster", [
    Core.skipOptions,
    option.int("GoldBoostQuant", "Gold Boost Quant", "Input the number of The Type of Boost For the Bot to Get [Max 9999]", 9999),
    option.int("ClassBoostQuant", "Class Boost Quant", "Input the number of The Type of Boost For the Bot to Get [Max 9999]", 9999),
    option.int("RepBoostQuant", "Rep Boost Quant", "Input the number of The Type of Boost For the Bot to Get [Max 9999]", 9999),
], true);

export default function main(): void {
        Core.setOptions();
        new FreeBoosts().getBoostsSelect(
            configOr("GoldBoostQuant", 0), 
            configOr("ClassBoostQuant", 0), 
            configOr("RepBoostQuant", 0));
        Core.setOptions(false);
}
