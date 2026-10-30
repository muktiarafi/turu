/*
name: Silver Exalted Paladin
description: Farms the Silver Exalted Paladin.
tags: silver exalted paladin, dragonheart
*/
// Port of Scripts/Good/SilverExaltedPaladin.cs.

import { Core } from "../core/CoreBots";
import { CoreStory } from "../core/CoreStory";

const Story = new CoreStory();

export class SEP {
    silverExaltedPaladin(): void {
        if (Core.checkInventory("Silver Exalted Paladin"))
            return;

        Story.preLoad(this.silverExaltedPaladin);

        Core.addDrop("Silver Exalted Paladin");

        if (!Core.isCompletedBefore(7586)) {
            if (!Story.questProgression(7580)) {
                Core.ensureAccept(7580);
                Core.killMonster("dragonheart", "r3", "Left", "*", "Ancient Paladin Chest 1");
                Core.killMonster("dragonheart", "r5", "Left", "*", "Ancient Paladin Chest 2");
                Core.killMonster("dragonheart", "r7", "Left", "*", "Ancient Paladin Chest 3");
                Core.killMonster("dragonheart", "r9", "Left", "*", "Ancient Paladin Chest 4");
                Core.killMonster("dragonheart", "r10", "Left", "Tempest Dracolich", "Ancient Paladin Chest 5");
                Core.killMonster("dragonheart", "r11", "Left", "Granite Dracolich", "Ancient Paladin Chest 6");
                Core.killMonster("dragonheart", "r12", "Left", "Avatar of Desolich", "Ancient Paladin Chest 7");
                Core.ensureComplete(7580);
            }

            // The First Scroll
            Story.killQuest(7581, "ectocave", "Ichor Dracolich", { autoCompleteQuest: false });
            Core.ensureCompleteChoose(7581);
            Core.toBank("Silver Exalted Helmet", "Silver Exalted Visor");

            // The Second Scroll
            Story.killQuest(7582, "frozenruins", "Frostdeep Dweller", { autoCompleteQuest: false });
            Core.ensureCompleteChoose(7582);
            Core.toBank("Silver Exalted Winged Helm", "Silver Exalted Winged Visor");

            // The Third Scroll
            Story.killQuest(7583, "thirdspell", "Great Solar Elemental", { getReward: false });
            Core.toBank("Silver Exalted Haloed Wings");

            // The Fourth Scroll
            Story.killQuest(7584, "table", "Roach", { autoCompleteQuest: false });
            Core.ensureCompleteChoose(7584);
            Core.toBank(
                    "Silver Exalted Paladin Spear",
                    "Silver Exalted Paladin Poleaxe",
                    "Silver Exalted Spears of Light",);

            // The Fifth Scroll
            Story.killQuest(7585, "dracocon", "Singer", { autoCompleteQuest: false });
            Core.ensureCompleteChoose(7585);
            Core.toBank("Silver Exalted Paladin Blade", "Silver Exalted Paladin Axe");
        }
        Core.ensureAccept(7586);
        Core.huntMonster("warhorc", "General Drox", "Paladin Armor Found");
        Core.ensureComplete(7586);
    }
}

export default function main(): void {
        Core.setOptions();

        new SEP().silverExaltedPaladin();

        Core.setOptions(false);
}
