/*
name: Tutorial Story
description: This will finish the Tutorial Story.
tags: story, quest, tutorial
*/
// Port of Scripts/Story/Tutorial.cs.

import { bot } from "turu";
import { Core } from "../core/CoreBots";

export class Tutorial {
    badges(): void {
        if (Core.hasAchievement(31))
            return;

        const achievements = [
            "Combat",
            "Interact",
            "Quest",
            "Skill",
            "Shop",
            "Enhance",
            "Rest",
            "World",
            "Emotes",
            "Travel",
        ];

        Core.logger("Doing `Tutorial Badges` to look a bit more \"~legit~\".");

        Core.join("oaklore");

        // Iterate through achievements and set them
        for (let i = 0; i < achievements.length; i++) {
            const achievement = achievements[i];
            if (Core.hasAchievement(22 + i))
                Core.logger(
                    `Achievement: ${achievement}, Status: ${(Core.hasAchievement(22 + i) ? "✅" : "❌")}`);
            Core.setAchievement(22 + i);
            bot.wait.forActionCooldown("doIA");
            Core.sleep(1500);
        }
    }
}

export default function main(): void {
        Core.setOptions();

        new Tutorial().badges();

        Core.setOptions(false);
}
