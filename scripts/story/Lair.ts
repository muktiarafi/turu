/*
name: Lair Story
description: This will finish the Lair Story.
tags: story, quest, lair
*/
// Port of Scripts/Story/Lair.cs.

import { Core } from "../core/CoreBots";
import { CoreStory } from "../core/CoreStory";

const Story = new CoreStory();

export class Lair {
    doAll(): void {
        this.galanoth();
        this.duncan();
        this.ferzana();
    }

    galanoth(): void {
        if (Core.isCompletedBefore(169))
            return;

        Story.preLoad(this.galanoth);
        //#region no longer needed
        // // Dragonbane 109
        // Story.KillQuest(109, "lair", "Wyvern");

        // // Dragon Scales 110
        // Story.KillQuest(110, "lair", "Wyvern");

        // // Dragon Souvenirs 111
        // Story.KillQuest(111, "lair", "Wyvern");

        // // Dragonslayer Reward 169
        // Story.KillQuest(169, "lair", "Wyvern");
        //#endregion no longer needed

        // Dragonslayer Veteran 165
        Story.killQuest(165, "lair", "Wyvern");

        // Dragonslayer Sergeant 166
        Story.killQuest(166, "lair", "Bronze Draconian");

        // Dragonslayer Captain 167
        Story.killQuest(167, "lair", "Dark Draconian");

        // Dragonslayer Marshal 168
        Story.killQuest(168, "lair", "Red Dragon");
    }

    duncan(): void {
        if (!Core.isMember) {
            Core.logger("You must be a Member to complete the quests.");
            return;
        }

        if (Core.isCompletedBefore(1495))
            return;

        Story.preLoad(this.duncan);

        // To Be A DragonHunter 1492
        Story.killQuest(1492, "lair", "Bronze Draconian");

        // The Easy Path 1493
        Story.mapItemQuest(1493, "lair", 755, 10);

        // The Red Trophy 1494
        Story.killQuest(1494, "lair", "Red Dragon");

        // Scramble Em! 1495
        Story.mapItemQuest(1495, "lair", 753);
    }

    ferzana(): void {
        if (!Core.isMember) {
            Core.logger("You must be a Member to complete the quests.");
            return;
        }

        if (Core.isCompletedBefore(1507))
            return;

        Story.preLoad(this.ferzana);

        // Do Not Crush The Eggs 1502
        Story.chainQuest(1502);

        // Heat ‘Em Up! 1503
        Story.mapItemQuest(1503, "lair", 754, 12);

        // Baby Food 1504
        Story.killQuest(1504, "lair", "Wyvern");

        // Molting 1505
        Story.killQuest(1505, "lair", "Bronze Draconian");

        // Fatherly Insanity 1506
        Story.killQuest(1506, "lair", "Dark Draconian");

        // What Needs To Be Done 1507
        Story.killQuest(1507, "lair", "Onyx Lava Dragon");
    }
}

export default function main(): void {
        Core.setOptions();

        new Lair().doAll();

        Core.setOptions(false);
}
