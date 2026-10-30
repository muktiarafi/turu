/*
name: Yokai Story
description: This will finish the Yokai Story.
tags: story, quest, yokai
*/
// Port of Scripts/Story/Yokai.cs.

import { ClassType, Core } from "../core/CoreBots";
import { CoreFarms } from "../core/CoreFarms";
import { CoreStory } from "../core/CoreStory";

const Farm = new CoreFarms();
const Story = new CoreStory();

export class YokaiQuests {
    quests(NoPirate = true): void {
        this.shogunWar();
        this.shinrinGrove();
        this.shadowfortress();
    }

    shogunWar(): void {
        if (Core.isCompletedBefore(6459))
            return;

        Story.preLoad(this.shogunWar);

        Core.logger("ShogunWar Quest line");

        // Shadow Medals 6450
        if (!Story.questProgression(6450)) {
            Core.ensureAccept(6450);
            Core.huntMonster("ShogunWar", "Shadow Samurai", "Shadow Medallions", 5);
            Core.ensureComplete(6450);
        }

        // We Need Supplies 6451
        if (!Story.questProgression(6451)) {
            Core.ensureAccept(6451);
            Core.huntMonsterMapID("ShogunWar", 8, "Kijimuna Pollen", 4);
            Core.huntMonsterMapID("ShogunWar", 7, "Reishi Spores", 4);
            Core.ensureComplete(6451);
        }

        // Help the Samurai 6452
        Story.mapItemQuest(6452, "ShogunWar", 5956, 5);

        // Arrows for Archers 6453
        Story.killQuest(6453, "ShogunWar", "Shadow Samurai");

        // Fix the Arrows 6454
        Story.killQuest(6454, "ShogunWar", "Bamboo Treeant");

        // Empowered Shadow Medallions 6455
        Story.killQuest(6455, "ShogunWar", "Shadow Samurai");

        // Advice from the Spirits 6456
        Story.mapItemQuest(6456, "ShogunWar", [ 5957, 5958, 5959, 5960 ]);

        // Fight the Shadows 6457
        Story.killQuest(6457, "ShogunWar", "Reishi");

        // Bamboo for Tea 6458
        Story.killQuest(6458, "ShogunWar", "Bamboo Treeant");

        // Defeat the Beast 6459
        if (!Story.questProgression(6459)) {
            Core.ensureAccept(6459);
            Core.equipClass(ClassType.Solo);
            Core.huntMonsterMapID("ShogunWar", 25, "Orochi Defeated");
            Core.ensureComplete(6459);
        }

        // Get the Medallions 6460
        Core.equipClass(ClassType.Farm);
        Story.killQuest(6460, "ShogunWar", "Shadow Samurai");

        // Tea for Me 6461
        if (!Story.questProgression(6461)) {
            Core.ensureAccept(6461);
            Core.huntMonster("shogunwar", "Kijimuna", "Kijimuna Pollen", 3);
            Core.huntMonster("shogunwar", "Reishi", "Glowing Mushroom Cap", 3);
            Core.huntMonster("shogunwar", "Bamboo Treeant", "Bamboo Leaf", 3);
            Core.ensureComplete(6461);
        }
    }

    shinrinGrove(): void {
        if (Core.isCompletedBefore(6472))
            return;

        Story.preLoad(this.shinrinGrove);

        Core.logger("ShinrinGrove Quest line");

        Core.equipClass(ClassType.Farm);

        // Get Rid of Kame 6462
        if (!Story.questProgression(6462)) {
            Core.ensureAccept(6462);
            Core.huntMonster("shinringrove", "Kame", "Kame Defeated");
            Core.ensureComplete(6462);
        }

        // Food for the Utoroshi 6463
        if (!Story.questProgression(6463)) {
            Core.ensureAccept(6463);
            Core.huntMonster("shinringrove", "Reishi", "Reishi Caps", 6);
            Core.huntMonster("shinringrove", "Tsurubebi", "Tsurubebi Flame", 3);
            Core.ensureComplete(6463);
        }

        // Summon Otoroshi 6464
        if (!Story.questProgression(6464)) {
            Core.ensureAccept(6464);
            Core.equipClass(ClassType.Solo);
            Story.mapItemQuest(6464, "shinringrove", 5962);
            Core.sleep();
            Core.killMonster("shinringrove", "r4a", "Left", "Otoroshi", "Otoroshi Defeated");
            Core.ensureComplete(6464);
        }

        // Put out the Fire 6465
        Core.equipClass(ClassType.Farm);
        if (!Story.questProgression(6465)) {
            Core.ensureAccept(6465);
            Core.getMapItem(5964, 5, "greenshell");
            Core.huntMonster("greenshell", "Tsurubebi", "Tsurubebi Extinguished", 5);
            Core.ensureComplete(6465);
        }

        // Expell the Tsukumogami 6466
        Story.killQuest(6466, "greenshell", "Tsukumogami");

        // Convince the Kodama 6467
        Story.killQuest(6467, "shinringrove", "Kodama");

        // Refill the Barrels 6468
        Story.killQuest(6468, "shinringrove", "Moglinberry Bush");

        // How to Breathe Underwater 6469
        if (!Story.questProgression(6469)) {
            Core.ensureAccept(6469);
            Core.getMapItem(5965, 8, "greenshell");
            Core.huntMonster("shinringrove", "Reishi", "Luminous Spores", 6);
            Core.ensureComplete(6469);
        }

        // Search for the Shinrin Do 6470
        if (!Story.questProgression(6470)) {
            Core.ensureAccept(6470);
            Core.getMapItem(5966, 1, "greenshell");
            Core.huntMonster("greenshell", "Merdraconian", "Merdraconians Slain", 3);
            Core.ensureComplete(6470);
        }

        // Take the Shinrin Do 6471
        Story.mapItemQuest(6471, "greenshell", 5967);

        // Battle for the Shinrin Do 6472
        if (!Story.questProgression(6472)) {
            Core.equipClass(ClassType.Solo);
            Core.ensureAccept(6472);
            Core.huntMonster("greenshell", "Nagami", "Nagami Defeated");
            Core.ensureComplete(6472);
        }

        // For the Road 6473
        if (!Story.questProgression(6473)) {
            Core.equipClass(ClassType.Farm);
            Core.ensureAccept(6473);
            Core.huntMonster("shinringrove", "Moglinberry Bush", "Moglinberry Bushels ", 8);
            Core.huntMonster("shinringrove", "Reishi", "Reishi Caps", 6);
            Core.ensureComplete(6473);
        }
    }

    shadowfortress(): void {
        if (Core.isCompletedBefore(6494))
            return;

        Story.preLoad(this.shadowfortress);

        Core.logger("Shadowfortress Quest line");

        Core.equipClass(ClassType.Farm);

        // Strange Creatures 6474
        Story.killQuest(6474, "heiwavalley", "Abumi Guchi");

        // Arm the Militia 6475
        if (!Story.questProgression(6475)) {
            Core.ensureAccept(6475);
            Core.huntMonster("heiwavalley", "Shadow Samurai", "Pilfered Armor", 5);
            Core.huntMonster("heiwavalley", "Shadow Samurai", "Stolen Sword", 5);
            Core.ensureComplete(6475);
        }

        // Burn the Bones 6476
        if (!Story.questProgression(6476)) {
            Core.ensureAccept(6476);
            Core.getMapItem(5968, 6, "heiwavalley");
            Core.huntMonster("heiwavalley", "Kosenjobi", "Kosenjobi Flame", 6);
            Core.ensureComplete(6476);
        }

        // Goryuken 6477
        Story.killQuest(6477, "heiwavalley", "Goryo");

        // Feel the Heat 6478
        if (!Story.questProgression(6478)) {
            Core.ensureAccept(6478);
            Core.huntMonster("heiwavalley", "Kosenjobi", "Spectral Heat", 5);
            Story.buyQuest(6478, "heiwavalley", 1608, "Wasabi");
        }

        // Get Some Fur 6479
        Story.killQuest(6479, "heiwavalley", "Abumi Guchi");

        // Howl-ing For You! 6480
        Story.killQuest(6480, "heiwavalley", "Inugami");

        // Eyes on the Prize 6481
        if (!Story.questProgression(6481)) {
            Core.ensureAccept(6481);
            Core.huntMonster("heiwavalley", "Inugami", "Spirit Eyes", 4);
            Story.mapItemQuest(6481, "heiwavalley", 5970, 8);
        }

        // Reveal the Trail 6482
        Story.mapItemQuest(6482, "heiwavalley", 5971);
        Story.mapItemQuest(6482, "heiwavalley", 5972, 8);

        // Defeat the Onryo 6483
        if (!Story.questProgression(6483)) {
            Core.ensureAccept(6483);
            Core.huntMonsterMapID("heiwavalley", 15, "Onryo Slain");
            Core.ensureComplete(6483);
        }

        // Calm the Spirits 6485
        Story.killQuest(6485, "shadowfortress", "Restless Spirit");

        // Disarm the Samurai 6486
        Story.killQuest(6486, "shadowfortress", "Shadow Samurai");

        // Find the Throne Room 6487
        Story.mapItemQuest(6487, "shadowfortress", 5973);

        // Shadow Essences 6488
        if (!Story.questProgression(6488)) {
            Core.ensureAccept(6488);
            Core.getMapItem(5974, 1, "shadowfortress");
            Core.huntMonster("shadowfortress", "Living Shadow", "Shadow Essence", 7);
            Core.ensureComplete(6488);
        }

        // New Recruits 6489
        Story.killQuest(6489, "shadowfortress", "Shadow Samurai");

        // Shadow Heads 6490
        if (!Story.questProgression(6490)) {
            Core.ensureAccept(6490);
            Core.getMapItem(5975, 7, "shadowfortress");
            Core.huntMonster("shadowfortress", "Creeping Shadow", "Shadows Slain", 7);
            Core.ensureComplete(6490);
        }

        // A Head has Woken
        Story.killQuest(6491, "shadowfortress", "7th Head of Orochi");

        // Get to the Roof 6492
        Story.mapItemQuest(6492, "shadowfortress", [ 5976, 5979 ]);

        // Find Jaaku! 6493
        if (!Story.questProgression(6493)) {
            Core.equipClass(ClassType.Solo);
            Core.ensureAccept(6493);
            Core.huntMonster("shadowfortress", "6th Head of Orochi", "6th Head Defeated");
            Core.huntMonster("shadowfortress", "5th Head of Orochi", "5th Head Defeated");
            Core.huntMonster("shadowfortress", "4th Head of Orochi", "4th Head Defeated");
            Core.huntMonster("shadowfortress", "3rd Head of Orochi", "3rd Head Defeated");
            Core.huntMonster("shadowfortress", "2nd Head of Orochi", "2nd Head Defeated");
            Core.huntMonster("shadowfortress", "1st Head of Orochi", "1st Head Defeated");
            Story.mapItemQuest(6493, "shadowfortress", 5977);
        }

        // Defeat Jaaku! 6494
        if (!Story.questProgression(6494)) {
            Core.equipClass(ClassType.Solo);
            Core.ensureAccept(6494);
            Core.huntMonsterMapID("shadowfortress", 31, "Jaaku Defeated");
            Core.ensureAccept(6494);
        }
    }
}

export default function main(): void {
        Core.setOptions();

        new YokaiQuests().quests();

        Core.setOptions(false);
}
