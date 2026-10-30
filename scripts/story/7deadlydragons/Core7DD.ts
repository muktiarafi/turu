/*
name: null
description: null
tags: null
*/
// Port of Scripts/Story/7DeadlyDragons/Core7DD.cs.
// Not ported (FarmerJoe's DoAll doesn't reach them): Complete7DD, Gluttony, Pride, Greed, Lust, Envy, Wrath.

import { bot } from "turu";
import { Core } from "../../core/CoreBots";
import { CoreStory } from "../../core/CoreStory";

const Story = new CoreStory();

export class Core7DD {
    sloth(): void {
        if (Core.isCompletedBefore(5959))
            return;

        Core.logger("Hunting the Slime Dragon of Sloth!");

        Story.preLoad(this.sloth);

        this.hazMatSuit();

        // Are There Any Survivors? 5945
        if (!Story.questProgression(5945)) {
            Core.ensureAccept(5945);
            Core.getMapItem(5382, 1, "Sloth");
            Core.huntMonster("sloth", "Plague Zombie", "Plague Zombie Defeated", 10);
            Core.ensureComplete(5945);
        }

        // Gathering Samples 5946
        if (!Story.questProgression(5946)) {
            Core.ensureAccept(5946);
            Core.huntMonster("sloth", "SnotGoblin", "Snot Sample", 6);
            Core.huntMonster("sloth", "Wandering Plague", "Plague Sample", 6);
            Core.ensureComplete(5946);
        }

        // Find a ‘Volunteer’ 5947
        Story.killQuest(5947, "Sloth", "Plague Zombie");

        // Cure the Volunteer 5948
        Story.mapItemQuest(5948, "Sloth", 5387);

        // Herbal Help 5949
        Story.killQuest(5949, "Sloth", "Marsh Thing");

        // Re-heal 5950
        Story.mapItemQuest(5950, "Sloth", 5383, 8);

        // Let's Try That Again 5951
        Story.mapItemQuest(5951, "Sloth", 5389);

        // One More Time 5952
        if (!Story.questProgression(5952)) {
            Core.ensureAccept(5952);
            Core.buyItem("Dragonhame", 865, "Airther Vitae");
            Core.buyItem("embersea", 1100, 1749);
            Core.ensureComplete(5952);
        }

        // Who’s Up for Round 3 5953
        Story.mapItemQuest(5953, "Sloth", 5391);

        // Cure the Villagers 5954
        Story.killQuest(5954, "Sloth", "Plague Zombie");

        // Gotta Clean Up 5955
        Story.mapItemQuest(5955, "Sloth", 5384, 10);

        // Clear the Castellum 5956
        if (!Story.questProgression(5956)) {
            Core.ensureAccept(5956);
            Core.getMapItem(5385, 1, "Sloth");
            Core.huntMonster("sloth", "SnotGoblin Prime", "Snotgoblin Prime Defeated", 1);
            Core.ensureComplete(5956);
        }

        // Get rid of Phlegnn 5957
        Story.killQuest(5957, "Sloth", "Phlegnn");

        // Cured is NOT GOOD 5958
        Story.killQuest(5958, "Sloth", "Cured Phlegnn");

        // Actual Sloth Dragon 5960
        Story.killQuest(5960, "Sloth", "Actual Sloth Dragon");

        // Mutated Plague 5959
        Story.killQuest(5959, "Sloth", "Mutated Plague");
    }

    hazMatSuit(): void {
        if (!bot.tempInv.contains(40710)) {
            Core.logger("Suit not found");
            Core.ensureAccept(5944);
            Core.getMapItems([ [5381, 1], [5380, 1] ], "Sloth");
            Core.ensureComplete(5944);
            bot.wait.forDrop("Hazmat Suit (Temp)");
            bot.wait.forPickup("Hazmat Suit (Temp)");
            bot.sleep(2500);
        } else {
            Core.logger("Suit found in Temp Inventory");
            bot.send.packet(`%xt%zm%equipItem%${bot.map.roomId}%40710%`);
            bot.sleep(2500);
        }
    }
}
