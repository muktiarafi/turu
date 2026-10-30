/*
name: Eternal Drakath Set
description: Gets the Eternal Drakath Set
tags: eternal drakath set, drakath, set
*/
// Port of Scripts/Chaos/EternalDrakathSet.cs.

import { bot } from "turu";
import { ClassType, Core } from "../core/CoreBots";
import { CoreFarms } from "../core/CoreFarms";
import { CoreBLOD } from "../good/blod/CoreBLOD";
import { StarSinc } from "../story/StarSinc";
import { DrakathArmorBot } from "./DrakathsArmor";

const Farm = new CoreFarms();
const Armor = new DrakathArmorBot();
const BLOD = new CoreBLOD();
const Star = new StarSinc();

export class EternalDrakath {
    private rewards = [
        "Drakath the Eternal",
        "Drakath the Eternal's Visor",
        "Eternal Chaos Tassels",
        "Eternal Chaos Tassels",
        "Dual Everlasting Blades of Chaos",
    ];

    getSet(singleitem = false, item: string | null = null): void {
        if (item != null && Core.checkInventory(item) || Core.checkInventory(this.rewards))
            return;

        if (item != null)
            Core.addDrop(item);
        else
            Core.addDrop(...this.rewards);
        Core.addDrop(25286);

        Armor.drakathArmor();
        if (!Core.checkInventory("Drakath Armor")) //"Drakath the Eternal"
        {
            Core.logger(
                "Drakath Armor is missing, so Drakath the Eternal cannot be obtained. Check the earlier quest reward and armor purchase messages.");
            return;
        }
        Core.equipClass(ClassType.Solo);
        if (!Core.checkInventory(25286))
            Core.logger("Farming Drakath's Sword");
        while (!bot.shouldExit && !Core.checkInventory(25286))
            Core.huntMonster("ultradrakath", "Champion of Chaos", null, 1, { log: false });

        Core.ensureAccept(8457);

        BLOD.spiritOrb(2000);

        Core.equipClass(ClassType.Farm);
        Core.killMonster("chaoslab", "r3", "Center", "Chaorrupted Moglin", "Crystallized Chaos", 800, { isTemp: false });

        if (!Core.checkInventory("Star Fragment", 33)) {
            Star.starSincQuests();
            Core.equipClass(ClassType.Farm);
            Core.addDrop("Star Fragment");

            Core.registerQuests(4413);
            while (!bot.shouldExit && !Core.checkInventory("Star Fragment", 33)) {
                Core.huntMonster("starsinc", "Living Star", "Living Star Defeated", 30, { isTemp: false }); 
                bot.wait.forPickup("Star Fragment");
            }
            Core.cancelRegisteredQuests();
        }

        if (!Core.checkInventory(61825, 5)) {
            bot.quests.updateQuest(3799);
            Core.equipClass(ClassType.Solo);
            Core.killMonster("shadowattack", "Boss", "Left", "Death", "Death's Oversight", 5, { isTemp: false });
        }

        if (!Core.checkInventory("Reality Shard", 300)) {
            Core.equipClass(ClassType.Farm);
            Core.addDrop("Reality Shard");

            Core.logger("Farming Reality Shards");
            Core.registerQuests(8456);
            while (!bot.shouldExit && !Core.checkInventory("Reality Shard", 200)) {
                Core.huntMonster("eternalchaos", "Chaos Time Fairy", "Preserved Chaos Fairy Wing", 5, { log: false }); 
                bot.wait.forPickup("Reality Shard");
            }
            Core.cancelRegisteredQuests();
        }
        Core.ensureComplete(8457);
        if (singleitem && item != null && Core.checkInventory(item)) {
            bot.wait.forDrop(item);
            bot.wait.forPickup(item);
            return;
        }
    }
}

export default function main(): void {
        Core.setOptions();

        new EternalDrakath().getSet();

        Core.setOptions(false);
}
