/*
name: Armor of Awe
description: This bot will farm get you the Armor of Awe
tags: armor, awe, good, pauldron, breastplate, vambrace, gauntlet, greaves, fragment, shard, relic
*/
// Port of Scripts/Good/GearOfAwe/ArmorOfAwe.cs.

import { bot } from "turu";
import { ClassType, Core } from "../../core/CoreBots";
import { CoreFarms } from "../../core/CoreFarms";
import { CoreAwe } from "./CoreAwe";

const Farm = new CoreFarms();
const Awe = new CoreAwe();

export class ArmorOfAwe {
    getArmor(): void {
        if (Core.checkInventory("Armor of Awe"))
            return;

        if (Core.isMember) {
            if (!Core.checkInventory("Pauldron Relic")) {
                Farm.bladeofAweREP(10, false);
                Farm.experience(55);
                Core.buyItem("museum", 1130, "Armor of Awe Pass");
                Core.addDrop("Pauldron Fragment");
                Core.equipClass(ClassType.Solo);

                Core.registerQuests(4162);
                Core.huntMonster("gravestrike", "Ultra Akriloth", "Pauldron Fragment", 15, { isTemp: false });
                bot.wait.forPickup("Pauldron Fragment");
                Core.cancelRegisteredQuests();

                Core.buyItem("museum", 1129, "Pauldron Relic");
            }
        } else
            Awe.getAweRelic("Pauldron", 4160, 15, 15, "gravestrike", "Ultra Akriloth");

        Awe.getAweRelic("Breastplate", 4163, 10, 10, "aqlesson", "Carnax");
        Awe.getAweRelic("Vambrace", 4166, 15, 15, "bloodtitan", "Ultra Blood Titan");
        Awe.getAweRelic("Gauntlet", 4169, 25, 5, "alteonbattle", "ULTRA Alteon");
        Awe.getAweRelic("Greaves", 4172, 10, 15, "bosschallenge", "Mutated Void Dragon");
        Core.buyItem("museum", 1129, "Armor of Awe");

        Core.toBank("Legendary Awe Pass", "Guardian Awe Pass", "Armor of Awe Pass");
    }
}

export default function main(): void {
        Core.setOptions();

        new ArmorOfAwe().getArmor();

        Core.setOptions(false);
}
