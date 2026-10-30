/*
name: Borgars Story
description: This will complete the Borgars story.
tags: story, quest, borgars
*/
// Port of Scripts/Story/Borgars.cs.

import { bot } from "turu";
import { Core } from "../core/CoreBots";
import { CoreStory } from "../core/CoreStory";

const Story = new CoreStory();

export class Borgars {
    storyLine(): void {
        if (Core.isCompletedBefore(7522))
            return;

        Story.preLoad(this.storyLine);

        // Beef Up 7510
        Core.addDrop("Slice of Cake");
        if (!Story.questProgression(7510)) {
            Core.ensureAccept(7510);
            Core.huntMonster("extinction", "Lard", "Lard", 2);
            Core.huntMonster("extinction", "Freezer Drone", "Ground Beef", 4);
            Core.ensureComplete(7510);
        }

        // Creamy! 7511
        Story.killQuest(7511, "battlefowl", "Chickencow", { getReward: false });

        // Sacchar-Imp 7512
        Story.killQuest(7512, "freakitiki", "Sugar Imp", { getReward: false });

        // Salty Balboa 7513
        Story.killQuest(7513, "stalagbite", "Balboa");

        // Filet o' Fishwing 7514
        Story.killQuest(7514, "pirates", "Fishwing", { getReward: false });

        // A Potion Master 7515
        Story.mapItemQuest(7515, "arcangrove", 7370);

        // A Weird Gourmet 7516
        Story.mapItemQuest(7516, "thespan", 7371);

        // Piece of Cake 7517
        if (!Story.questProgression(7517)) {
            // The Cake is NOT a lie! 7518
            if (!Core.checkInventory(54728)) {
                Core.ensureAccept(7518);
                Core.huntMonster("portalmaze", "Time Wraith", "Time Fragment", 10);
                // "Slice of Cake"
                bot.wait.forDrop(54728);
                Core.ensureComplete(7518);
            }

            Story.chainQuest(7517);
        }

        // A Health Nut 7519
        Story.mapItemQuest(7519, "brightfortress", 7372);

        // The Importance Of Staying Healthy 7520
        if (!Story.questProgression(7520)) {
            Core.ensureAccept(7520);
            Core.huntMonster("brightfortress", "Brightscythe Reaver", "Kale", 4);
            Core.huntMonster("brightfortress", "Brightscythe Reaver", "Quinoa", 4);
            Core.ensureComplete(7520);
        }
        // An Aspiring Burgermonger 7521
        Story.mapItemQuest(7521, "borgars", 7373);

        // Burglinster's Revenge 7522
        Story.killQuest(7522, "borgars", "Burglinster", { getReward: false });
    }
}

export default function main(): void {
        Core.setOptions();

        new Borgars().storyLine();

        Core.setOptions(false);
}
