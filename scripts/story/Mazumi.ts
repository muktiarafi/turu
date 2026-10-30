/*
name: Mazumi Story
description: This will finish the Mazumi Story.
tags: story, quest, mazumi
*/
// Port of Scripts/Story/Mazumi.cs.

import { ClassType, Core } from "../core/CoreBots";
import { CoreStory } from "../core/CoreStory";

const Story = new CoreStory();

export class Mazumi {
    mazumiQuests(): void {
        if (Core.isCompletedBefore(92))
            return;

        Story.preLoad(this.mazumiQuests);

        // Ninja Grudge 90
        Story.killQuest(90, "pirates", "Fishman Soldier");

        // Without a Trace 91
        if (!Story.questProgression(91)) {
            Core.ensureAccept(91);
            Core.huntMonster("greenguardwest", "Kittarian", "Kittarian's Wallet", 2);
            Core.huntMonster("greenguardwest", "River Fishman", "River Fishman's Wallet", 2);
            Core.huntMonster("greenguardwest", "Slime", "Slime-Soaked Wallet", 2);

            // Dont use `West1` for Frogzard, as the cell contains an extra frogzard that isnt there.
            Core.killMonster("greenguardwest", "West3", "Up", "Frogzard", "Frogzard's Lint Hoard", 2);

            Core.killMonster("greenguardwest", "West12", "Up", "Big Bad Boar", "Big Bad Boar's Wallet");
            Core.ensureComplete(91);
        }

        // Hit Job 92
        Core.equipClass(ClassType.Solo);
        if (!Story.questProgression(92)) {
            Core.ensureAccept(92);
            Core.huntMonster("greenguardwest", "Breken the Vile", "Breken the Vile's Sword", 1);
            Core.huntMonster("greenguardwest", "Ogug Stoneaxe", "Ogug Stoneaxe's Helmet", 1);
            Core.ensureComplete(92);
        }
    }
}

export default function main(): void {
        Core.setOptions();

        new Mazumi().mazumiQuests();

        Core.setOptions(false);
}
