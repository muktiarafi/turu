/*
name: Temple Siege
description: Completes the Temple Siege storyline.
tags: story, quest, temple, siege, nulgath, seasonal, staff, birthday
*/
// Port of Scripts/Seasonal/StaffBirthdays/Nulgath/TempleSiege.cs.

import { bot } from "turu";
import { Core } from "../../../core/CoreBots";
import { CoreStory } from "../../../core/CoreStory";

const Story = new CoreStory();

export class TempleSiege {
    completeTempleSiege(): void {
        if (Core.isCompletedBefore(9067))
            return;

        if (!Core.isSeasonalMapActive("templesiege"))
            return;

        Story.preLoad(this.completeTempleSiege);

        //Fairweather Elementals || 9058
        if (!Story.questProgression(9058)) {
            Core.ensureAccept(9058);
            Core.getMapItem(11129, 8, "templesiege");
            Core.huntMonster("templesiege", "Light Elemental", "Solidified Light", 8);
            Core.ensureComplete(9058);
        }

        //In Case of Emergency || 9059
        Story.killQuest(9059, "templesiege", "Overdriven Paladin");

        //Blind Devotion || 9060
        if (!Story.questProgression(9060)) {
            Core.ensureAccept(9060);
            //due to duplcate item names, using ItemIDs this way.
            while (!bot.shouldExit && !Core.checkInventory(75700, 5))
                Core.huntMonster("templesiege", "Overdriven Paladin");
            while (!bot.shouldExit && !Core.checkInventory(75701, 5))
                Core.huntMonster("templesiege", "Light Elemental");
            Core.ensureComplete(9060);
        }

        //Blessed Visage || 9061
        Story.mapItemQuest(9061, "templesiege", [ 11130, 11131, 11132 ]);

        //Banging on Light's Door || 9062
        if (!Story.questProgression(9062)) {
            Core.ensureAccept(9062);
            Core.huntMonster("templesiege", "Overdriven Paladin", "Engraved Vambrace", 3);
            Core.huntMonster("templesiege", "Light Elemental", "Elemental Bracelet", 3);
            Core.ensureComplete(9062);
        }

        //Words Bind || 9063
        Story.mapItemQuest(9063, "templesiege", 11133, 6);
        Story.mapItemQuest(9063, "templesiege", 11134);

        //Doomed Darkness || 9064
        Story.killQuest(9064, "templesiege", "Doomed Beast");

        //Shady Costuming || 9065
        Story.killQuest(9065, "templesiege", "Doomed Troll");

        //The Emperor's Shadow || 9066
        if (!Story.questProgression(9066)) {
            Core.ensureAccept(9066);
            Core.huntMonster("templesiege", "Doomed Troll", "Troll Tusks", 6);
            Core.huntMonster("templesiege", "Doomed Beast", "Beast Eyes", 6);
            Core.ensureComplete(9066);
        }

        //Throat Sores || 9067
        Story.killQuest(9067, "templesiege", "Doomed Oblivion");
    }
}

export default function main(): void {
        Core.setOptions();

        new TempleSiege().completeTempleSiege();
        Core.setOptions(false);
}
