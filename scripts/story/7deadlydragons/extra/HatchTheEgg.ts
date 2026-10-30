/*
name: Hatch The Egg (Extra)
description: This will finish the Hatch The Egg quest.
tags: story, quest, 7-deadly-dragons, extra, hatch-the-egg
*/
// Port of Scripts/Story/7DeadlyDragons/Extra/HatchTheEgg.cs.

import { bot } from "turu";
import { Core } from "../../../core/CoreBots";
import { CoreStory } from "../../../core/CoreStory";
import { MysteriousEgg } from "../../../other/MysteriousEgg";

const Story = new CoreStory();
const Egg = new MysteriousEgg();

export class HatchTheEgg {
    hatch(): void {
        if (Core.isCompletedBefore(6914) && Core.checkInventory("Manticore Cub Pet"))
            return;

        Egg.getMysteriousEgg();

        Story.preLoad(this.hatch);

        //6901 | Feel the Heat
        Story.killQuest(6901, "volcano", "Lava Golem");

        //6902 | Need Lava
        Story.killQuest(6902, "embersea", "Living Lava");

        //6903 | Baby Food
        Story.killQuest(6903, "ashfallcamp", "Lava Dragoblin");

        //6904 | Thirsty!
        Story.killQuest(6904, "gilead", "Water Elemental");

        //6905 | Dress Up Time
        Story.killQuest(6905, "crossroads", "Koalion");

        //6906 | Complete your Costume
        Story.killQuest(6906, "mountain", "Giant Scorpion");

        //6907 | Find the Void Larva
        Story.mapItemQuest(6907, "void", 6453);

        //6908 | Void Energy Needed
        if (!Story.questProgression(6908)) {
            Core.ensureAccept(6908);
            while (!bot.shouldExit && !Core.checkInventory(48632, 8))
                Core.killMonster("void", "r11", "Left", "Void Elemental");
            Core.ensureComplete(6908);
        }

        //6909 | Go Find Mariel
        Story.mapItemQuest(6909, "void", 6454);

        //6910 | Void... Parchment ?
        Story.killQuest(6910, "void", "Void Bear");

        //6911 | Magical Ink
        Story.killQuest(6911, "void", "Void Elemental");

        //6912 | A Soul for a Spell
        bot.quests.updateQuest(904);
        Story.killQuest(6912, "void", "Void Dragon");

        //6913 | Cast the Spell!
        if (!Story.questProgression(6913) || !Core.checkInventory("Mysterious Egg")) {
            Core.ensureAccept(6913);
            Core.getMapItem(6455, 1, "mysteriousegg");
            Core.ensureComplete(6913);
            bot.wait.forPickup("Mysterious Egg");
        }

        //6914 | The Egg Hatches
        if (!Story.questProgression(6914) || !Core.checkInventory("Manticore Cub Pet")) {
            Core.chainComplete(6914);
            bot.wait.forPickup("Manticore Cub Pet");
        }
    }
}

export default function main(): void {
        Core.setOptions();

        new HatchTheEgg().hatch();

        Core.setOptions(false);
}
