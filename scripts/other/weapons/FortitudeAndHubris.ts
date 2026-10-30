/*
name: FortitudeAndHubris
description: null
tags: null
*/
// Port of Scripts/Other/Weapons/FortitudeAndHubris.cs.

import { bot } from "turu";
import { ClassType, Core } from "../../core/CoreBots";
import { CoreFarms } from "../../core/CoreFarms";
import { CoreStory } from "../../core/CoreStory";

const Farm = new CoreFarms();
const Story = new CoreStory();

export class FandH {
    fortitudeAndHubris(): void {
        if (Core.checkInventory("Fortitude + Hubris"))
            return;

        Story.preLoad(this.fortitudeAndHubris);

        Core.addDrop(
            "Zorbak's Secret G-Rave Key", 
            "Sword's Cost", 
            "Shards of the Sword", 
            "Hubris's Final Blade Shard", 
            "Hubris' Magic Essence", 
            "Hubris", 
            "Fortitude's Blade Shards", 
            "Fortitude's Magic Essence", 
            "Fortitude", 
            "Fortitude + Hubris");

        // Qualifying Quest
        Story.killQuest(6593, "stalagbite", "Balboa", { getReward: false });

        // Rest for the Not Very Wicked
        Core.ensureAccept(6594);
        Core.huntMonster("pines", "Red Shell Turtle", "Turtle Shell");
        Core.huntMonster("pines", "Pine Grizzly", "Coffee Beans of Awakeness");
        Core.huntMonster("pines", "Pine Troll", "String Fibers", 5);
        Story.mapItemQuest(6594, "tavern", 6114, 1, { getReward: false });

        // Pisces Pieces
        Story.killQuest(6595, "river", "Kuro", { getReward: false });

        // Be Ebil
        if (!Story.questProgression(6596, { getReward: false })) {
            Core.ensureAccept(6596);
            Core.getMapItem(6116, 1, "maul");
            Core.huntMonster("maul", "Creature Creation", "Ebil Limbs", 5);
            Core.ensureComplete(6596);
        }
        Core.sellItem("Zorbak's Secret G-Rave Key");

        // Eternal, Never-Ending Darkness and Death Lance
        Farm.experience(45);
        Story.killQuest(6598, "shadowrealm", "Shadow Lord", { getReward: false });

        // It Takes a Special Brand of Glory
        Story.killQuest(6599, "dragontown", "Chaos Fluffy", { getReward: false });

        // 1st Trial
        if (!Story.questProgression(6600, { getReward: false })) {
            Core.ensureAccept(6600);
            Core.getMapItem(6115, 1, "david");
            Core.huntMonster("mountfrost", "Snow Golem", "Golems Defeated", 20);
            Core.ensureComplete(6600);
        }

        // 2nd Trial
        Story.killQuest(6601, "northpointe", "Wyvern", { getReward: false });

        // 3rd Trial
        Story.buyQuest(6602, "museum", 1653, "Sword's Cost", 1, { getReward: false });

        // 4th Trial
        Story.killQuest(6603, "razorclaw", "Enraged Razorclaw", { getReward: false });

        // Hubris
        if (!Core.checkInventory("Hubris")) {
            Core.ensureAccept(6604);
            Core.equipClass(ClassType.Farm);
            Core.killMonster("doomwood", "r6a", "Right", "Doomwood Ectomancer", "Hubris's Final Blade Shard", 1, { isTemp: false });
            Core.sleep(2500);
            Core.equipClass(ClassType.Solo);
            Core.killTrigoras("Hubris' Handle", 1, 1, true);
            Core.huntMonster("styx", "Styx Hydra", "Hubris' Magic Essence", 50, { isTemp: false });
            Core.ensureComplete(6604);
            bot.wait.forPickup("Hubris");
        }

        // Fortitude
        if (!Core.checkInventory("Fortitude")) {
            Core.equipClass(ClassType.Solo);
            Core.ensureAccept(6605);
            Core.huntMonster("iceplane", "Enfield", "Fortitude's Handle");
            bot.quests.updateQuest(4616);
            Core.huntMonster("mummies", "Mummy", "Fortitude's Blade Shards", 100, { isTemp: false });
            Core.huntMonster("banished", "Desterrat Moya", "Fortitude's Magic Essence", 50, { isTemp: false });
            Core.ensureComplete(6605);
            bot.wait.forPickup("Fortitude");
        }

        // Dual Wielding
        if (!Core.checkInventory("Fortitude + Hubris")) {
            Story.killQuest(6606, "skytower", [ "Aspect of Good", "Aspect of Evil" ]);
            bot.wait.forPickup("Fortitude + Hubris");
        }
    }
}

export default function main(): void {
        Core.setOptions();

        new FandH().fortitudeAndHubris();

        Core.setOptions(false);
}
