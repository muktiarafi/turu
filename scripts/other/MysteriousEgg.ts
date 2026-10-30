/*
name: Mysterious Egg
description: if you own the manticore cub pet, this will farm the Mysterious Egg
tags: mysterious egg, manticore cub pet, egg
*/
// Port of Scripts/Other/MysteriousEgg.cs.

import { bot } from "turu";
import { Core } from "../core/CoreBots";
import { Core7DD } from "../story/7deadlydragons/Core7DD";

const DD = new Core7DD();

export class MysteriousEgg {
    getMysteriousEgg(): void {
        if (Core.checkInventory("Mysterious Egg"))
            return;

        if (Core.checkInventory("Manticore Cub Pet") && !Core.checkInventory("Mysterious Egg")) {
            Core.logger(
                "You own the \"Manticore Cub Pet\" and thus dont need to farm for the \"Mysterious Egg\".\n"
                    + "It's a lot quicker to use AQW's BuyBack function for the \"Mysterious Egg\". Please do so now\n"
                    + "https://www.aq.com > Account > Manage Account > left hand side, \"Buy Back\"\n"
                    + "Relogin and restart the script");
        }

        Core.addDrop("Mysterious Egg");
        Core.ensureAccept(6171);

        Core.killMonster("pride", "r13", "Left", "Valsarian", "Key of Pride", 1, { isTemp: false });
        Core.killMonster("gluttony", "Enter2", "Top", "Deflated Glutus", "Key of Gluttony", 1, { isTemp: false });
        Core.killMonster("greed", "r16", "Left", "Goregold", "Key of Greed", 1, { isTemp: false });

        if (!Core.checkInventory("Key of Sloth")) {
            DD.hazMatSuit();
            Core.huntMonster("sloth", "Phlegnn", "Key of Sloth", 1, { isTemp: false });
        }

        Core.huntMonster("lust", "Lascivia", "Key of Lust", 1, { isTemp: false });
        bot.quests.updateQuest(6000);
        Core.huntMonster("maloth", "Maloth", "Key of Envy", 1, { isTemp: false });
        Core.huntMonster("wrath", "Gorgorath", "Key of Wrath", 1, { isTemp: false });

        Core.ensureComplete(6171, 42497);
        bot.wait.forPickup("Mysterious Egg");
    }
}

export default function main(): void {
        Core.setOptions();

        new MysteriousEgg().getMysteriousEgg();

        Core.setOptions(false);
}
