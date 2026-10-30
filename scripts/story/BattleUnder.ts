/*
name: Battle Under Story
description: This will complete the Battle Under Story.
tags: story, quest, battleunder
*/
// Port of Scripts/Story/BattleUnder.cs.

import { bot } from "turu";
import { ClassType, Core } from "../core/CoreBots";
import { CoreStory } from "../core/CoreStory";

const Story = new CoreStory();

export class BattleUnder {
    battleUnderAll(): void {
        this.battleUnderA();
        this.battleUnderB();
        this.battleUnderC();
        this.battleUnderD();
        this.battleUnderE();
    }

    battleUnderA(): void {
        if (Core.isCompletedBefore(377))
            return;

        Story.preLoad(this.battleUnderA);

        Core.equipClass(ClassType.Farm);
        Story.killQuest(374, "battleundera", "Skeletal Warrior");
        if (!Story.questProgression(375)) {
            Core.ensureAccept(375);
            Core.huntMonster("battleundera", "Skeletal Warrior", "Skeletal Claymore", 6);
            Core.huntMonster("battleundera", "Skeletal Warrior", "Bony Chestplate", 3);
            Core.ensureComplete(375);
        }
        Core.equipClass(ClassType.Solo);
        Story.killQuest(376, "battleundera", "Bone Terror");
        Core.equipClass(ClassType.Farm);
        Story.killQuest(377, "battleundera", "Skeletal Warrior");
    }

    battleUnderB(): void {
        if (Core.isCompletedBefore(935))
            return;

        Story.preLoad(this.battleUnderB);

        this.battleUnderA();

        Core.equipClass(ClassType.Farm);
        Story.killQuest(689, "battleunderb", "Skeleton Warrior");
        Story.killQuest(690, "battleunderb", "Skeleton Warrior");
        Story.killQuest(691, "battleunderb", "Skeleton Warrior");
        Core.equipClass(ClassType.Solo);
        Story.killQuest(692, "battleunderb", "Undead Champion", { getReward: false });
        Story.mapItemQuest(935, "battleunderb", 253);
    }

    battleUnderC(): void {
        if (Core.isCompletedBefore(939))
            return;

        Story.preLoad(this.battleUnderC);

        this.battleUnderB();

        Story.killQuest(936, "battleunderc", "Blue Crystalized Undead");
        Story.killQuest(937, "battleunderc", "Blue Crystalized Undead");
        Story.killQuest(938, "battleunderc", "Crystalized Jellyfish");
        if (!Story.questProgression(939)) {
            Core.ensureAccept(939);
            Core.huntMonster("battleundera", "Bone Terror", "Bone Terror Soul");
            Core.huntMonster("battleunderb", "Undead Champion", "Undead Champion Soul");
            Core.huntMonster("battleunderc", "Crystalized Jellyfish", "Jellyfish Soul");
            Core.ensureComplete(939);
        }
    }

    battleUnderD(): void {
        if (!Core.isMember || Core.isCompletedBefore(2215))
            return;

        Story.preLoad(this.battleUnderD);

        Core.addDrop("Battle Pickaxe");
        this.battleUnderC();

        //Note; Mobs on the map are randomly spawned / room
        //with 3 or 4 spawns sets that can be made/room.. thus breaking hunt(and killquest)

        if (!Story.questProgression(2211)) {
            Core.ensureAccept(2211);
            Core.killMonster("battleunderd", "Enter", "Spawn", "Shivering Bones", "Shivering Bone", 10);
            Core.ensureComplete(2211);
        }

        if (!Story.questProgression(2212)) {
            Core.ensureAccept(2212);
            Core.killMonster("battleunderd", "r2", "Left", "Icy Banshee", "Banshee Slain", 7);
            Story.mapItemQuest(2212, "battleunderd", 1286, 8);
        }

        if (!Story.questProgression(2213)) {
            Core.ensureAccept(2213);
            Core.killMonster("battleunderd", "r5", "Left", "Skeletal Warrior", "Icy Pickaxe Found");
            Core.ensureComplete(2213);
        }

        if (!Story.questProgression(2214)) {
            Core.ensureAccept(2214);
            Core.killMonster("battleunderd", "r5", "Left", "Glacial Horror", "Glacial Horror Slain");
            Core.getMapItem(1287, 4, "battleunderd");
            Core.ensureComplete(2214);
        }

        Story.mapItemQuest(2215, "battleunderd", 1288);
    }

    battleUnderE(): void {
        if (Core.isCompletedBefore(5928))
            return;

        Story.preLoad(this.battleUnderE);

        if (!Story.questProgression(5927)) {
            Core.ensureAccept(5927);
            Core.getMapItem(5362, 1, "battleundere");
            Core.huntMonster("battleundere", "Lava Guard", "Cavern Info", 1);
            Core.ensureComplete(5927);
        }
        Story.killQuest(5928, "battleundere", "Hot Mama");
    }

    understone(): void {
        if (Core.checkInventory("Understone"))
            return;

        Core.addDrop("Understone");

        Core.ensureAccept(7289);
        bot.quests.updateQuest(935);
        Core.killMonster("battleunderc", "Enter", "Spawn", "*", "Fluorite Shard", 10);
        Core.ensureComplete(7289);
        bot.wait.forPickup("Understone");
    }
}

export default function main(): void {
        Core.setOptions();

        new BattleUnder().battleUnderAll();

        Core.setOptions(false);
}
