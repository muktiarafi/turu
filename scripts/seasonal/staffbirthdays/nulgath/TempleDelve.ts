/*
name: Temple Delve
description: Completes the Temple Delve storyline.
tags: story, quest, temple, delve, siegefortress, nulgath, seasonal, staff, birthday
*/
// Port of Scripts/Seasonal/StaffBirthdays/Nulgath/TempleDelve.cs.

import { ClassType, Core } from "../../../core/CoreBots";
import { CoreStory } from "../../../core/CoreStory";
import { TempleSiege } from "./TempleSiege";

const Story = new CoreStory();
const TS = new TempleSiege();

export class TempleDelve {
    storyline(): void {
        // Core.isCompletedBefore(9143) aways returns true...? evne if none of these are complete
        if (!Core.isSeasonalMapActive("templedelve"))
            return;

        TS.completeTempleSiege();

        Story.preLoad(this.storyline);

        //Mindless Waste (9080)
        Story.killQuest(9080, "templedelve", "Overdriven Paladin");

        //Stone Proxy (9081)
        if (!Story.questProgression(9081)) {
            Core.ensureAccept(9081);
            Core.getMapItems([ [11160, 2], [11161, 2] ], "templedelve");
            Core.huntMonster("templedelve", "Doomed Troll", "Infested Troll Hide", 7);
            Core.ensureComplete(9081);
        }

        //The Deepness of Shadows... (9082)
        if (!Story.questProgression(9082)) {
            Core.ensureAccept(9082);
            Core.getMapItem(11162, 4, "templedelve");
            Core.huntMonster("templedelve", "Doomed Beast", "Beasts Freed", 6);
            Core.ensureComplete(9082);
        }

        //...Begets the Glare of the Light (9083)
        if (!Core.isCompletedBefore(9083)) {
            Core.ensureAccept(9083);
            Core.huntMonster("templedelve", "Delirious Elemental", "Fractured Light Shard", 4);
            Core.huntMonster("templedelve", "Infested Nation", "Nation Eliminated", 4);
            Story.mapItemQuest(9083, "templedelve", 11163);
        }

        //Death Over Defeat (9084)
        Story.killQuest(9084, "templedelve", "Infested Nation");

        //For a Brighter Tomorrow (9085)
        Story.killQuest(9085, "templedelve", "Delirious Elemental");

        //Minions Anonymous (9086)
        Story.mapItemQuest(9086, "templedelve", 11164, 3);
        Story.mapItemQuest(9086, "templedelve", 11165);

        //Prephosphorus (9087)
        if (!Story.questProgression(9087)) {
            Core.ensureAccept(9087);
            Core.getMapItems([ [11166, 1], [11167, 1] ], "templedelve");
            Core.huntMonster("templedelve", "Delirious Elemental", "Elemental Vanquished", 6);
            Core.ensureComplete(9087);
        }

        //Sunspotty (9088)
        if (!Story.questProgression(9088)) {
            Core.ensureAccept(9088);
            Core.huntMonster("templedelve", "Delirious Elemental", "Frenzied Essence", 6);
            Core.huntMonster("templedelve", "Infested Nation", "Nation Essence", 6);
            Core.ensureComplete(9088);
        }

        //Feeding Time (9089)
        Core.equipClass(ClassType.Solo);
        Story.killQuest(9089, "templedelve", "Doomed Fiend");

        // Vile Vanguard 9134
        Story.killQuest(9134, "siegefortress", "Legion Dreadmarch");

        // Fell Fealty 9135
        Story.killQuest(9135, "siegefortress", "Legion Dread Knight");

        // Infestation Signs 9136
        Story.mapItemQuest(9136, "siegefortress", 11312, 4);
        Story.mapItemQuest(9136, "siegefortress", 11313);

        // Banal Barage 9137
        if (!Story.questProgression(9137)) {
            Core.ensureAccept(9137);
            Core.huntMonster("siegefortress", "Legion Dread Knight", "Knight Demolished", 6);
            Core.huntMonster("siegefortress", "Legion Dreadmarch", "Dreadmarch Demolished", 6);
            Core.ensureComplete(9137);
        }

        // Bluebirds 9138
        Story.killQuest(9138, "siegefortress", "Shadow Traitor");

        // Anatomical Example 9139
        if (!Story.questProgression(9139)) {
            Core.ensureAccept(9139);
            Core.getMapItem(11314, 4, "siegefortress");
            Core.huntMonster("siegefortress", "Shadow Traitor", "Traitor Subdued", 6);
            Core.ensureComplete(9139);
        }

        // Bloodless Daisy 9140
        if (!Story.questProgression(9140)) {
            Core.ensureAccept(9140);
            Core.getMapItem(11315, 1, "siegefortress");
            Core.huntMonster("siegefortress", "Enslaved Elemental", "Light Rune", 8);
            Core.ensureComplete(9140);
        }

        // Shadows of Espionage 9141
        if (!Story.questProgression(9141)) {
            Core.ensureAccept(9141);
            Core.getMapItem(11316, 2, "siegefortress");
            Core.huntMonster("siegefortress", "Shadow Traitor", "Traitor Interrogated", 5);
            Core.huntMonster("siegefortress", "Enslaved Elemental", "Light Strangled", 5);
            Core.ensureComplete(9141);
        }

        // Foul Duke of Light 9142
        Story.killQuest(9142, "siegefortress", "Enslaved Astero");

        // Evil of Humanity 9143
        Story.killQuest(9143, "siegefortress", "Dage the Evil");
    }
}

export default function main(): void {
        Core.setOptions();

        new TempleDelve().storyline();
        Core.setOptions(false);
}
