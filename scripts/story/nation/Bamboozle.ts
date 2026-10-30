/*
name: Bamboozle
description: This will finish the Bamboozle quest line.
tags: story, quest, bamboozle, nation
*/
// Port of Scripts/Story/Nation/Bamboozle.cs.

import { bot } from "turu";
import { Core } from "../../core/CoreBots";
import { CoreStory } from "../../core/CoreStory";
import { BattleUnder } from "../BattleUnder";

const Story = new CoreStory();
const Under = new BattleUnder();

export class Bamboozle {
    bamboozleQuest(): void {
        if (Core.isCompletedBefore(7292))
            return;

        Story.preLoad(this.bamboozleQuest);

        Core.addDrop(
            "Floozer", 
            "Ice Diamond", 
            "Dark Bloodstone", 
            "Songstone", 
            "Butterfly Sapphire", 
            "Understone", 
            "Rainbow Moonstone", 
            "Time Quartz");

        //Star of the Sandsea
        if (!Story.questProgression(7277)) {
            bot.quests.updateQuest(976);
            Core.ensureAccept(7277);
            Core.killMonster("wanders", "r2", "Down", "Kalestri Worshiper", "Star of the Sandsea");
            Core.ensureComplete(7277);
        }

        //Ice Diamond
        if (!Story.questProgression(7278)) {
            Core.ensureAccept(7278);
            Core.addDrop("Ice Diamond");
            if (!Core.checkInventory("Ice Diamond")) {
                Core.ensureAccept(7279);
                Core.killMonster("kingcoal", "r1", "Left", "*", "Frozen Coal", 10, { log: false });
                Core.ensureComplete(7279);
                bot.wait.forPickup("Ice Diamond");
            }
            Core.ensureComplete(7278);
        }

        //Dark Bloodstone
        if (!Story.questProgression(7280)) {
            if (!Core.checkInventory("Dark Bloodstone")) {
                Core.ensureAcceptmultiple(7280, 7281);
                Core.huntMonster("safiria", "Blood Maggot", "Blood Gem", 10);
                Core.ensureComplete(7281);
                bot.wait.forPickup("Dark Bloodstone");
            }
            Core.ensureComplete(7280);
        }

        //Doomstone
        Story.killQuest(7282, "brightfall", "Painadin Overlord", { getReward: false });

        //Void Opal
        Story.killQuest(7283, "timevoid", "Unending Avatar", { getReward: false });

        //Mana Crystal
        Story.mapItemQuest(7284, "downward", 6908, 1, { getReward: false });

        //Songstone
        if (!Story.questProgression(7285)) {
            if (!Core.checkInventory("Songstone")) {
                Core.ensureAcceptmultiple(7285, 7297);
                Core.getMapItem(6909, 15, "mythsong");
                Core.ensureComplete(7297);
                bot.wait.forPickup("Songstone");
            }
            Core.ensureComplete(7285);
        }

        //Butterfly Sapphire
        if (!Story.questProgression(7286)) {
            Core.ensureAccept(7286);
            if (!Core.checkInventory("Butterfly Sapphire")) {
                Core.ensureAccept(7287);
                Core.huntMonster("bloodtusk", "Trollola Plant", "Butterfly Bloom", 15);
                Core.ensureComplete(7287);
                bot.wait.forPickup("Butterfly Sapphire");
            }
            Core.ensureComplete(7286);
        }

        //Understone
        if (!Story.questProgression(7288)) {
            if (!Core.checkInventory("Understone")) {
                bot.quests.updateQuest(935);
                Under.understone();
                bot.wait.forPickup("Understone");
                Core.chainComplete(7288);
            }
        }

        //Rainbow Moonstone
        if (!Story.questProgression(7290) || !Core.checkInventory("Floozer")) {
            Core.ensureAccept(7290);
            if (!Core.checkInventory("Rainbow Moonstone")) {
                Core.ensureAccept(7291);
                Core.huntMonster("earthstorm", "Sapphire Golem", "Chip of Sapphire");
                Core.huntMonster("earthstorm", "Ruby Golem", "Chip of Ruby");
                Core.huntMonster("earthstorm", "Emerald Golem", "Chip of Emerald");
                Core.huntMonster("earthstorm", "Diamond Golem", "Chip of Diamond");
                Core.ensureComplete(7291);
                bot.wait.forPickup("Rainbow Moonstone");
            }
            Core.ensureComplete(7290);
            bot.wait.forPickup("Floozer");
        }

        //Time Quartz
        // if (!Story.QuestProgression(7292))
        // {
        Story.mapItemQuest(7292, "thespan", 6910);
    }
}

export default function main(): void {
        Core.setOptions();

        new Bamboozle().bamboozleQuest();

        Core.setOptions(false);
}
