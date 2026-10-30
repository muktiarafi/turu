/*
name: Third Spell Story
description: This will finish the Third Spell Story.
tags: story, quest, thirdspell
*/
// Port of Scripts/Story/ThirdSpell.cs.

import { bot } from "turu";
import { ClassType, Core } from "../core/CoreBots";
import { CoreStory } from "../core/CoreStory";

const Story = new CoreStory();

export class ThirdSpell {
    storyLine(HoTS = false): void {
        // C# local functions
        const questLogic = (): void => {
                switch (Story.legacyQuestId) {
                    case 4474: //Breakfast With... Brains?
                        Core.huntMonster("doomwood", "Doomwood Treeant", "Braaaainz", 10);
                        break;

                    case 4475: //Post-Elemental Apocalypse
                        Core.huntMonster("thirdspell", "Mana Phoenix", "Mana Plane Monster Defeated", 12);
                        break;

                    case 4476: //Phoenix’s First Birthday
                        Core.huntMonster("thirdspell", "Mana Phoenix", "Proxy Eggs", 8);
                        break;

                    case 4477: //Elementals Are From the Sun, We Are From Lore
                        Core.getMapItem(3668, 10, "thirdspell");
                        break;

                    case 4478: //Sssssmoking...
                        Core.huntMonster("thirdspell", "Mana Phoenix", "Elemental Pathway Cleared", 20);
                        break;

                    case 4479: //Green-Eyed Incantations
                        Core.huntMonster("thirdspell", "Mana Phoenix", "Green Eye", 5);
                        break;

                    case 4480: //A Lonely Cysero
                        Core.getMapItem(3671, 1, "thirdspell");
                        break;

                    case 4481: //Body & Soul & The Hero
                        Core.getMapItem(3675, 1, "thirdspell");
                        break;

                    case 4482: //Abduction
                        Core.huntMonster("thirdspell", "Mana Phoenix", "Abducted Mana Phoenix");
                        break;

                    case 4483: //Truth or De-Feathering
                        Core.getMapItem(3672, 1, "thirdspell");
                        break;

                    case 4484: //The Art of Persuasion
                        Core.huntMonster("thirdspell", "Great Solar Elemental", "Great Solar Elemental Defeated");
                        break;

                    case 4485: //Frozen Mana
                        Core.huntMonster("thirdspell", "Mana Phoenix", "Mana Phoenix Egg", 7);
                        break;

                    case 4486: //Angry Elements
                        Core.huntMonster("thirdspell", "Great Solar Elemental", "Great Solar Elemental Defeated Again");
                        break;

                    case 4487: //The Sunspots, They Are Changing!
                        Core.getMapItem(3673, 3, "thirdspell");
                        break;

                    case 4488: //I Enjoy Being a Soul
                        Core.killMonster("thirdspell", "r12", "Left", "Living Fire", "Sun Monster Ember", 15);
                        break;

                    case 4489: //Burning Like Me!
                        Core.huntMonster("thirdspell", "Sun Flare", "Sun Flare Defeated", 10);
                        Core.killMonster("thirdspell", "r12", "Left", "Living Fire", "Living Fire Defeated", 5);
                        break;

                    case 4490: //Assault With a Deadly Shadow
                        Core.getMapItem(3674, 1, "thirdspell");
                        break;

                    case 4491: // 4491|Mother Knows The Sun
                        Core.huntMonster("thirdspell", "Solar Incarnation ", "Heart of the Sun Received");
                        break;

                    case 4492: // 4492|Selfishness
                        Core.getMapItem(3676, 1, "thirdspell");
                        break;

                    case 4493: // 4493|See the Hero Run
                        Core.getMapItem(3677, 1, "thirdspell");
                        break;
                }
        };

        Core.logger(`Heart of the Sun Mode Enabled?: ${HoTS}`);

        if (HoTS)
            this.heartoftheSun();
        else {
            if (Core.checkInventory("Sun Token VIII", 1, { toInv: false })) {
                Core.logger("Heart of the Sun Mode: False, \"Sun Token VIII\" Owned.");
                return;
            }

            Story.legacyQuestManager(
                questLogic,
                4474, 
                4475, 
                4476, 
                4477, 
                4478, 
                4479, 
                4480, 
                4481, 
                4482, 
                4483, 
                4484, 
                4485, 
                4486, 
                4487, 
                4488, 
                4489, 
                4490, 
                4491, 
                4492, 
                4493);
        }
    }

    heartoftheSun(): void {
        if (Core.checkInventory("Heart of the Sun"))
            return;
        Core.addDrop(
            "Brainz n' Eggs", 
            "Mana Token I", 
            "Mana Token II", 
            "Mana Token III", 
            "Mana Token IV", 
            "Mana Token V", 
            "Mana Token VI", 
            "Mana Token VI", 
            "Mana Token VII", 
            "Mana Token VIII", 
            "Mana Token IX", 
            "Mana Token X", 
            "Mana Token XI", 
            "Sun Token I", 
            "Sun Token II", 
            "Sun Token III", 
            "Sun Token IV", 
            "Sun Token V", 
            "Sun Token VI", 
            "Heart of the Sun");

        Core.equipClass(ClassType.Farm);

        if (!Core.checkInventory("Heart of the Sun")) {
            // Check for "Sun Token V"
            if (!Core.checkInventory("Sun Token V")) {
                // Check for "Sun Token IV"
                if (!Core.checkInventory("Sun Token IV")) {
                    // Check for "Sun Token III"
                    if (!Core.checkInventory("Sun Token III")) {
                        // Check for "Sun Token II"
                        if (!Core.checkInventory("Sun Token II")) {
                            // Check for "Sun Token I"
                            if (!Core.checkInventory("Sun Token I")) {
                                // Check for "Mana Token XI"
                                if (!Core.checkInventory("Mana Token XI")) {
                                    // Check for "Mana Token X"
                                    if (!Core.checkInventory("Mana Token X")) {
                                        // Check for "Mana Token IX"
                                        if (!Core.checkInventory("Mana Token IX")) {
                                            // Check for "Mana Token VIII"
                                            if (!Core.checkInventory("Mana Token VIII")) {
                                                // Check for "Mana Token VII"
                                                if (!Core.checkInventory("Mana Token VII")) {
                                                    // Check for "Mana Token VI"
                                                    if (!Core.checkInventory("Mana Token VI")) {
                                                        // Check for "Mana Token V"
                                                        if (!Core.checkInventory("Mana Token V")) {
                                                            // Check for "Mana Token IV"
                                                            if (
                                                                !Core.checkInventory(
                                                                    "Mana Token IV")
                                                            ) {
                                                                // Check for "Mana Token III"
                                                                if (
                                                                    !Core.checkInventory(
                                                                        "Mana Token III")
                                                                ) {
                                                                    // Check for "Mana Token II"
                                                                    if (
                                                                        !Core.checkInventory(
                                                                            "Mana Token II")
                                                                    ) {
                                                                        // Check for "Mana Token I"
                                                                        if (
                                                                            !Core.checkInventory(
                                                                                "Mana Token I")
                                                                        ) {
                                                                            // Check for "Brainz n' Eggs"
                                                                            if (
                                                                                !Core.checkInventory(
                                                                                    "Brainz n' Eggs")
                                                                            ) {
                                                                                Core.logger(
                                                                                    "Token: Brainz n' Eggs");
                                                                                Core.ensureAccept(
                                                                                    4474);
                                                                                Core.huntMonster("doomwood", "Doomwood Treeant", "Braaaainz", 10);
                                                                                Core.ensureComplete(
                                                                                    4474);
                                                                                bot.wait.forPickup(
                                                                                    "Brainz n' Eggs");
                                                                            }
                                                                            Core.logger(
                                                                                "Token: Mana Token II");
                                                                            Core.ensureAccept(4475);
                                                                            Core.huntMonster("thirdspell", "Mana Phoenix", "Mana Plane Monster Defeated", 12);
                                                                            Core.ensureComplete(
                                                                                4475);
                                                                            bot.wait.forPickup(
                                                                                "Mana Token I");
                                                                        }
                                                                        Core.logger(
                                                                            "Token: Mana Token III");
                                                                        Core.ensureAccept(4476);
                                                                        Core.huntMonster("thirdspell", "Mana Phoenix", "Proxy Eggs", 8);
                                                                        Core.ensureComplete(4476);
                                                                        bot.wait.forPickup(
                                                                            "Mana Token II");
                                                                    }
                                                                    Core.logger(
                                                                        "Token: Mana Token IV");
                                                                    Core.ensureAccept(4477);
                                                                    Core.getMapItem(
                                                                        3668, 
                                                                        10, 
                                                                        "thirdspell");
                                                                    Core.ensureComplete(4477);
                                                                    bot.wait.forPickup(
                                                                        "Mana Token III");
                                                                }
                                                                Core.logger("Token: Mana Token V");
                                                                Core.ensureAccept(4478);
                                                                Core.huntMonster("thirdspell", "Mana Phoenix", "Elemental Pathway Cleared", 20);
                                                                Core.ensureComplete(4478);
                                                                bot.wait.forPickup("Mana Token IV");
                                                            }
                                                            Core.logger("Token: Mana Token VI");
                                                            Core.ensureAccept(4479);
                                                            Core.huntMonster("thirdspell", "Mana Phoenix", "Green Eye", 5);
                                                            Core.ensureComplete(4479);
                                                            bot.wait.forPickup("Mana Token V");
                                                        }
                                                        Core.logger("Token: Mana Token VII");
                                                        Core.ensureAccept(4480);
                                                        Core.getMapItem(3671, 1, "thirdspell");
                                                        Core.ensureComplete(4480);
                                                        bot.wait.forPickup("Mana Token VI");
                                                    }
                                                    Core.logger("Token: Mana Token VIII");
                                                    Core.ensureAccept(4481);
                                                    Core.getMapItem(3675, 1, "thirdspell");
                                                    Core.ensureComplete(4481);
                                                    bot.wait.forPickup("Mana Token VII");
                                                }
                                                Core.logger("Token: Mana Token IX");
                                                Core.ensureAccept(4482);
                                                Core.huntMonster("thirdspell", "Mana Phoenix", "Abducted Mana Phoenix");
                                                Core.ensureComplete(4482);
                                                bot.wait.forPickup("Mana Token VIII");
                                            }
                                            Core.logger("Token: Mana Token X");
                                            Core.ensureAccept(4483);
                                            Core.getMapItem(3672, 1, "thirdspell");
                                            Core.ensureComplete(4483);
                                            bot.wait.forPickup("Mana Token IX");
                                        }
                                        Core.logger("Token: Mana Token XI");
                                        Core.ensureAccept(4484);
                                        Core.huntMonster("thirdspell", "Great Solar Elemental", "Great Solar Elemental Defeated");
                                        Core.ensureComplete(4484);
                                        bot.wait.forPickup("Mana Token X");
                                    }
                                    Core.logger("Token: Sun Token I");
                                    Core.ensureAccept(4485);
                                    Core.huntMonster("thirdspell", "Mana Phoenix", "Mana Phoenix Egg", 7);
                                    Core.ensureComplete(4485);
                                    bot.wait.forPickup("Mana Token XI");
                                }
                                Core.logger("Token: Sun Token II");
                                Core.ensureAccept(4486);
                                Core.huntMonster("thirdspell", "Great Solar Elemental", "Great Solar Elemental Defeated Again");
                                Core.ensureComplete(4486);
                                bot.wait.forPickup("Sun Token I");
                            }
                            Core.logger("Token: Sun Token III");
                            Core.ensureAccept(4487);
                            Core.getMapItem(3673, 3, "thirdspell");
                            Core.ensureComplete(4487);
                            bot.wait.forPickup("Sun Token II");
                        }
                        Core.logger("Token: Sun Token IV");
                        Core.equipClass(ClassType.Solo);
                        Core.ensureAccept(4488);
                        Core.killMonster("thirdspell", "r12", "Left", "Living Fire", "Sun Monster Ember", 15);
                        Core.ensureComplete(4488);
                        bot.wait.forPickup("Sun Token III");
                    }
                    Core.logger("Token: Sun Token V");
                    Core.ensureAccept(4489);
                    Core.equipClass(ClassType.Solo);
                    Core.huntMonster("thirdspell", "Sun Flare", "Sun Flare Defeated", 10);
                    Core.equipClass(ClassType.Farm);
                    Core.killMonster("thirdspell", "r12", "Left", "Living Fire", "Living Fire Defeated", 5);
                    Core.ensureComplete(4489);
                    bot.wait.forPickup("Sun Token IV");
                }
                Core.logger("Token: Sun Token VI");
                Core.ensureAccept(4490);
                Core.getMapItem(3674, 1, "thirdspell");
                Core.ensureComplete(4490);
                bot.wait.forPickup("Sun Token V");
            }
            Core.logger("Token: Heart of the Sun");
            Core.ensureAccept(4491);
            Core.equipClass(ClassType.Solo);
            Core.huntMonster("thirdspell", "Solar Incarnation ", "Heart of the Sun Received");
            Core.ensureComplete(4491);
            bot.wait.forPickup("Heart of the Sun");
        }
    }
}

export default function main(): void {
        Core.bankingBlackList.push(...
            [
                "Brainz n' Eggs",
                "Mana Token I",
                "Mana Token II",
                "Mana Token III",
                "Mana Token IV",
                "Mana Token V",
                "Mana Token VI",
                "Mana Token VI",
                "Mana Token VII",
                "Mana Token VIII",
                "Mana Token IX",
                "Mana Token X",
                "Mana Token XI",
                "Sun Token I",
                "Sun Token II",
                "Sun Token III",
                "Sun Token IV",
                "Sun Token V",
                "Sun Token VI",
                "Sun Token VII",
                "Sun Token VIII",
                "Heart of the Sun",
            ]);
        Core.setOptions();
        new ThirdSpell().storyLine();

        Core.setOptions(false);
}
