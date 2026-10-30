/*
name: StarSinc Story
description: This will finish the StarSinc Story.
tags: story, quest, starsinc
*/
// Port of Scripts/Story/StarSinc.cs.

import { bot } from "turu";
import { ClassType, Core } from "../core/CoreBots";
import { CoreStory } from "../core/CoreStory";

const Story = new CoreStory();

export class StarSinc {
    starSincQuests(): void {
        // C# local functions
        const questLogic = (): void => {
            switch (Story.legacyQuestId) {
                case 4400: // Weaken his Powers
                    Core.logger("Nova Badge 1");
                    Core.huntMonster("starsinc", "Star Sprites", "Stardust", 15);
                    break;

                case 4401: // Light and Dark
                    Core.logger("Nova Badge 2");
                    Core.huntMonster("starsinc", "Star Sprites", "Sprite Magic Essence", 7);
                    break;

                case 4402: // Paintings Give Him Strength
                    Core.logger("Nova Badge 3");
                    Core.getMapItem(3607, 4, "starsinc");
                    break;

                case 4403: // Learning the Land
                    Core.logger("Nova Badge 4");
                    Core.getMapItem(3608, 1, "starsinc");
                    break;

                case 4404: // Slay the Light and Dark
                    Core.logger("Nova Badge 5");
                    Core.huntMonster("starsinc", "Infernal Imp", "Darkness Fragment", 5);
                    Core.huntMonster("starsinc", "Living Star", "Light Fragment", 5);
                    break;

                case 4405: // Chaos Fragments
                    Core.logger("Nova Badge 6");
                    Core.killMonster("watchtower", "Frame2", "Left", "Chaos Spider", "Chaos Fragment", 10);
                    break;

                case 4406: // Kill Them All
                    Core.logger("Nova Badge 7");
                    Core.huntMonster("starsinc", "Star Sprites", "Monster Killed", 15);
                    break;

                case 4407: // Get Rid of Those Guards
                    Core.logger("Nova Badge 8");
                    Core.huntMonster("starsinc", "Fortress Guard", "Guard Slain", 5);
                    break;

                case 4408: // Breach the Gate
                    Core.logger("Nova Badge 9");
                    Core.huntMonster("starsinc", "Fortress Guard", "Guard's Key");
                    break;

                case 4409: // Defeat the Prime Dominus
                    Core.logger("Nova Badge 10");
                    Core.equipClass(ClassType.Solo);
                    Core.huntMonster("starsinc", "Prime Dominus", "Prime Defeated", 1, { isTemp: false });
                    Core.join("whitemap");
                    Core.logger("Resetting map");
                    break;

                case 4410: // Place the Beacons
                    Core.equipClass(ClassType.Solo);
                    Core.logger("Nova Badge 11");
                    Core.getMapItem(3609, 6, "starsinc");
                    Core.logger("Resetting map");
                    Core.join("whitemap");
                    break;

                case 4412: // Retrieve the Core
                    Core.logger("SuperNova Badge");
                    Core.equipClass(ClassType.Solo);
                    Core.killMonster("starsinc", "r15", "Left", "Final", "Final Defeated");
                    Core.join("whitemap");
                    break;
            }
        };

        if (Core.checkInventory("SuperNova Badge"))
            return;

        bot.drops.add(
            ...Core.questRewards(
                4400, 
                4401, 
                4402, 
                4403, 
                4404, 
                4405, 
                4406, 
                4407, 
                4408, 
                4409, 
                4410, 
                4412));

        Core.equipClass(ClassType.Farm);

        Story.legacyQuestManager(
            questLogic,
            4400, 
            4401, 
            4402, 
            4403, 
            4404, 
            4405, 
            4406, 
            4407, 
            4408, 
            4409, 
            4410, 
            4412);
    }
}

export default function main(): void {
        Core.bankingBlackList.push(...
            [
                "Nova Badge 1.0",
                "Nova Badge 2.0",
                "Nova Badge 3.0",
                "Nova Badge 3.0",
                "Nova Badge 4.0",
                "Nova Badge 5.0",
                "Nova Badge 6.0",
                "Nova Badge 7.0",
                "Nova Badge 8.0",
                "Nova Badge 9.0",
                "Nova Badge 10.0",
                "Nova Badge 11.0",
                "SuperNova Badge",
            ]);
        Core.setOptions();

        new StarSinc().starSincQuests();

        Core.setOptions(false);
}
