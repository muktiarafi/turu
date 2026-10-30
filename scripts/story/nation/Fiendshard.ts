/*
name: Fiendshard
description: This will finish the Fiendshard quest.
tags: story, quest, nation, fiendshard
*/
// Port of Scripts/Story/Nation/Fiendshard.cs.

import { ClassType, Core } from "../../core/CoreBots";
import { CoreStory } from "../../core/CoreStory";
import { Originul_Story } from "./Originul";

const Story = new CoreStory();
const Originul = new Originul_Story();

export class Fiendshard_Story {
    fiendshard_QuestlineP1(): void {
        Story.preLoad(this.fiendshard_QuestlineP1);

        Originul.originul_Questline();

        Core.logger(
            "Making sure portal quest is done.. appearntly its required...? (though not stated?)");
        Core.join("tercessuinotlim");

        // Sneak Attack
        if (!Story.questProgression(7892)) {
            Core.equipClass(ClassType.Farm);
            Core.ensureAccept(7892);
            Core.killMonster("Fiendshard", "r2", "Left", "Rogue Fiend", "Rogue Fiends Defeated", 4);
            Story.mapItemQuest(7892, "Fiendshard", 7983);
        }

        // Fiend-terrogation
        if (!Story.questProgression(7893)) {
            Core.equipClass(ClassType.Farm);
            Core.ensureAccept(7893);
            Core.killMonster("Fiendshard", "r2", "Left", "Rogue Fiend", "Fiends Interrogated", 3);
            Core.ensureComplete(7893);
        }

        // Key Difference Between Human and Fiend
        if (!Story.questProgression(7894)) {
            Core.equipClass(ClassType.Farm);
            Core.ensureAccept(7894);
            Core.killMonster("Fiendshard", "r2", "Left", "Rogue Fiend", "Key Fragments Located", 4);
            Core.ensureComplete(7894);
        }

        // Unlock the Door
        if (!Story.questProgression(7895)) {
            Core.equipClass(ClassType.Farm);
            Core.ensureAccept(7895);
            Core.killMonster("fiendshard", "r2", "Left", "Rogue Fiend", "Rogue Fiend Defeated", 5);
            Core.killMonster("fiendshard", "r5", "Left", "Paladin Fiend", "Paladin Fiend Defeated", 5);
            Core.huntMonster("fiendshard", "Void Knight", "Void Knight Defeated", 3);
            Story.mapItemQuest(7895, "Fiendshard", 7984);
        }

        // Dirtlicking Guards
        Story.killQuest(7896, "Fiendshard", "Paladin Fiend");

        // Defeat Dirtlicker
        if (!Story.questProgression(7897)) {
            Core.equipClass(ClassType.Solo);
            if (!Story.questProgression(7897)) {
                Core.ensureAccept(7897);
                Core.huntMonster("fiendshard", "Dirtlicker", "Dirtlicker Defeated", 1);
                Core.huntMonster("fiendshard", "Fiend Shard", "Dirtlicker's Fiend Shard Shattered", 5);
                Core.ensureComplete(7897);
            }
            Core.jump("Enter", "Spawn");
        }

        // Destroy the Fiend Shard
        // Archfiend DeathLord quests can be done without finishing this quest.
        if (!Story.questProgression(7898)) {
            Core.equipClass(ClassType.Solo);
            Core.ensureAccept(7898);
            Core.killMonster("fiendshard", "r9", "Left", "Paladin Fiend", "Fiends Fended Off", 15);
            Core.killMonster("fiendshard", "r9", "Left", 15, 59059, 1);
            Core.jump("Enter", "Spawn");
            Core.ensureComplete(7898);
        }
    }
}

export default function main(): void {
        Core.setOptions();

        new Fiendshard_Story().fiendshard_QuestlineP1();

        Core.setOptions(false);
}
