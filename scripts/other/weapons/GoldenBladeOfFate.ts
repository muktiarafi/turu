/*
name: GoldenBladeOfFate
description: null
tags: null
*/
// Port of Scripts/Other/Weapons/GoldenBladeOfFate.cs.

import { bot } from "turu";
import { Core } from "../../core/CoreBots";
import { CoreStory } from "../../core/CoreStory";

const Story = new CoreStory();

export class GoldenBladeOfFate {
    getGBoF(): void {
        if (Core.checkInventory("Golden Blade of Fate"))
            return;

        Story.preLoad(this.getGBoF);

        if (!Core.checkInventory("Golden Blade of Fate")) {
            Core.logger("Doing for the Golden Blade of Fate");

            if (!Core.isCompletedBefore(5679)) {
                // The Lost Teacher
                Story.killQuest(5669, "tutor", "Horc Tutor Trainer");

                // Big Gold Coins
                Story.killQuest(5670, "prison", "Piggy Drake");

                // Light as a Feather
                Story.killQuest(5671, "lavarun", "Phedra");

                // Shard Shard Shard
                Story.killQuest(5672, "chaoscrypt", "Chaorrupted Armor");

                // White Scales, Light Scales
                Story.killQuest(5673, "j6", "Sketchy Frogzard");

                // The Stench of Defeat
                Story.mapItemQuest(5674, "orcpath", 5143, 3);

                // If you can't stand the heat...
                Story.killQuest(5675, "lair", "Red Dragon");

                // The Depths of Despair
                Story.mapItemQuest(5676, "well", 5144);

                // All Things Green and Small...
                Story.killQuest(5677, "cellar", "GreenRat");

                // Doom... Or Redemption?
                if (!Story.questProgression(5678)) {
                    Core.ensureAccept(5678);
                    if (!Core.checkInventory(38354))
                        Core.huntMonster("sepulchure", "Dark Sepulchure", "Dark Spirit");
                    Core.ensureComplete(5678);
                }
            }

            //The Mysterious Reward 5679
            Core.ensureAccept(5679);
            Core.getMapItem(5145, 1, "yulgar");
            Core.ensureComplete(5679);
            bot.wait.forPickup("Golden Blade of Fate");
        }
    }
}

export default function main(): void {
        Core.setOptions();

        new GoldenBladeOfFate().getGBoF();

        Core.setOptions(false);
}
