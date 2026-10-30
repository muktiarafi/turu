/*
name: Originul
description: This will finish the Originul quest.
tags: story, quest, originul, nation
*/
// Port of Scripts/Story/Nation/Originul.cs.

import { ClassType, Core } from "../../core/CoreBots";
import { CoreStory } from "../../core/CoreStory";

const Story = new CoreStory();

export class Originul_Story {
    originul_Questline(): void {
        if (Core.isCompletedBefore(7889))
            return;

        Story.preLoad(this.originul_Questline);

        // Inquisite the Inquisitors
        Story.killQuest(7881, "Originul", "Inquisitor Guard");

        // Captains Capped
        Story.killQuest(7882, "Originul", "Inquisitor Captain");

        // Grand Defeat
        Story.killQuest(7883, "Originul", "Grand Inquisitor");

        // Portal Unlocked
        if (!Story.questProgression(7884)) {
            Core.ensureAccept(7884);
            Core.huntMonster("originul", "Inquisitor Guard", "Guard Bone Dust", 5);
            Core.huntMonster("originul", "Inquisitor Captain", "Captain Bone Dust", 4);
            Core.huntMonster("originul", "Grand Inquisitor", "Grand Inquisitor Bone Dust", 1);
            Core.ensureComplete(7884);
        }

        // Fiend Training
        Story.killQuest(7885, "Originul", "Bloodfiend");

        // Failed Fiend Shards
        Story.killQuest(7886, "Originul", "Bloodfiend");

        // Executed Tasks
        if (!Story.questProgression(7887)) {
            Core.equipClass(ClassType.Farm);
            Core.ensureAccept(7887);
            Core.killMonster("Originul", "r7", "left", "Dreadfiend", "Dreadfiend Executed", 5);
            Core.ensureComplete(7887);
        }

        // Champion Usurper
        Core.equipClass(ClassType.Solo);
        Story.killQuest(7888, "Originul", "Fiend Champion");

        // Break their Muti-kneecaps
        Core.equipClass(ClassType.Farm);
        if (!Story.questProgression(7889)) {
            Core.ensureAccept(7889);
            Core.killMonster("Originul", "r10", "Top", "Bloodfiend", "Mutineer Crushed", 25);
            Core.ensureComplete(7889);
        }
        Core.logger("Questline completed.");
    }
}

export default function main(): void {
        Core.setOptions();

        new Originul_Story().originul_Questline();

        Core.setOptions(false);
}
