/*
name: null
description: null
tags: null
*/
// Port of Scripts/Story/Summer2015AdventureMap/CoreSummer.cs.
// Not ported (FarmerJoe's DoAll doesn't reach them): DoAll, DreadSpace, Deadmoor, Beleen, LivingDungeon, LunaCove, CyserosSecret.

import { Core } from "../../core/CoreBots";
import { CoreStory } from "../../core/CoreStory";

const Story = new CoreStory();

export class CoreSummer {
    collector(): void {
        if (Core.isCompletedBefore(1348))
            return;

        Story.preLoad(this.collector);

        // This Town in a Desktop Globe
        Story.mapItemQuest(1293, "Terrarium", 586);

        // Dust Bunnies Can't Hide
        Story.killQuest(1294, "Terrarium", "Dustbunny of Doom");

        // Au Contr-air holes
        Story.mapItemQuest(1295, "Terrarium", 587, 10);

        // Hero Eats Everything. Even Pellets
        Story.mapItemQuest(1296, "Terrarium", 588, 12);

        // The Moth Defeats the Man?
        Story.killQuest(1297, "Terrarium", "Death on Wings");

        // Can you find them?
        Story.mapItemQuest(1298, "Terrarium", [ 589, 590, 591, 592, 593 ]);

        // You've Been Looking Everywhere!
        Story.mapItemQuest(1299, "Terrarium", [ 593, 594, 595, 596, 604 ]);

        // Go for Grease!
        Story.killQuest(1300, "Terrarium", "Death on Wings");

        // Doppelganger Wants to Hit You
        if (!Story.questProgression(1308)) {
            Core.ensureAccept(1308);
            Core.huntMonster("terrarium", "Doppleganger Of Fred", "Doppelganger Demolished!", 1);
            Core.huntMonster("terrarium", "Doppleganger of Will", "Doppelganger Destroyed!", 1);
            Core.ensureComplete(1308);
        }

        // Catapult Climb
        Story.mapItemQuest(1309, "Terrarium", 604);

        // The Treasure that You Seek - 976
        Story.killQuest(1339, "prehistoric", "Gigantosaurus");

        // These Trees - 1340
        Story.mapItemQuest(1340, "prehistoric", 630, 11);

        // Ewwww... Totally Gross - 1341
        Story.killQuest(1341, "prehistoric", "Gigantosaurus");

        // The Eggs, Exciting and New - 1342
        Story.mapItemQuest(1342, "prehistoric", 631, 4);

        // Someday (Today) Their Mother Will Die - 1343
        Story.killQuest(1343, "prehistoric", "Mother TerrorDOOMKill");

        // More Aliens Than You Can Handle?
        Story.killQuest(1344, "Future", "Alien Dino");

        // Spy Eyes
        Story.mapItemQuest(1345, "Future", 632, 2);

        // Smashed to Pieces
        Story.mapItemQuest(1346, "Future", 633, 7);

        // Tired of My Vigilette Dreams
        if (!Story.questProgression(1347)) {
            Core.ensureAccept(1347);
            Core.huntMonster("Future", "Red-Eyed Alien", "Small Fragment Acquired", 5);
            Core.huntMonster("Future", "Red-Eyed Alien", "Medium Piece Acquired", 5);
            Core.ensureComplete(1347);
        }

        // You're Not the Boss of Me Now
        if (!Story.questProgression(1348)) {
            Core.ensureAccept(1348);
            Core.huntMonster("Future", "The Collector", "Collector Vanquished");
            Core.ensureComplete(1348);
        }
    }
}
