/*
name: J6 Saga Story
description: This will finish the J6 Saga Story.
tags: story, quest, j6-saga
*/
// Port of Scripts/Story/J6Saga.cs.

import { bot } from "turu";
import { Core } from "../core/CoreBots";
import { CoreFarms } from "../core/CoreFarms";
import { CoreStory } from "../core/CoreStory";

const Farm = new CoreFarms();
const Story = new CoreStory();

export class J6Saga {
    j6(ReTurnWithHammer = false): void {
        if (Core.isCompletedBefore(2858)) {
            Core.logger("StoryLine Complete:  J6 Saga");
            return;
        }

        Story.preLoad(this.j6);

        if (ReTurnWithHammer)
            Core.addDrop(5194);

        //J6 Quiz Game
        Story.chainQuest(674);

        //Access Zephyrus
        Story.chainQuest(693);

        //Zephyrus End
        if (!Story.questProgression(694)) {
            Core.jump("R10");
            Core.join("zephyrus");
            Story.mapItemQuest(694, "zephyrus", [ 116, 117 ]);
        }

        //Mission1
        Story.killQuest(698, "forest", "Zardman Grunt");

        if (ReTurnWithHammer)
            return;

        //Mission2
        Story.killQuest(699, "boxes", "Sneeviltron");

        if (!Core.isCompletedBefore(1172)) {
            Core.jump("R10");
            Core.join("moonyard");
        }
        //A Gate and Terrible Monster
        Story.killQuest(1171, "moonyard", "Junkyard Wall");

        //I'm on the Hunt, I'm after 58-Sr3
        Story.mapItemQuest(1172, "moonyard", 495);

        //Intruder Alert! Intruder Alert!
        Story.killQuest(1173, "moonyardb", "Robo Guard", { getReward: false });

        //Mission 3
        Story.killQuest(1177, "marsh2", "Lesser Shadow Serpent");

        //Mission 4
        Story.killQuest(1178, "marsh2", "Lesser Groglurk");

        //Planet Banzai!
        Story.chainQuest(2168);

        //L.O.S.E.R. Battle!
        Story.chainQuest(2169);

        //Down to the Wire
        Story.chainQuest(2170);

        //J6 in SPAAAAACE!
        Story.chainQuest(2171);

        //Mission 5
        Story.killQuest(2173, "djinn", "Harpy");

        //Mission 6
        if (!Story.questProgression(2830)) {
            Core.ensureAccept(2830);
            Core.killMonster("xantown", "Enter", "Spawn", "*", "Flare Artifact");
            Core.ensureComplete(2830);
            if (bot.map.name === "xantown")
                Core.jump("r12", "Left"); // map is aggro af this is a safe cell.
            Core.sellItem("Auxiliary Clue 3");
        }

        //Fuel For Flight
        if (!Story.questProgression(2831)) {
            Core.ensureAccept(2831);
            bot.quests.updateQuest(811);
            Core.huntMonster("sandsea", "Cactus Creeper", "Cactus Creeper Oil", 3);
            Core.killMonster("cloister", "r2", "Right", "*", "Acornent Oil", 3);
            bot.quests.updateQuest(1176);
            Core.killMonster("moonyardb", "r4", "Left", "*", "Robo Dog Oil", 3);
            Core.killMonster("farm", "Crop1", "Left", "*", "Scarecrow Canola Oil", 3);
            Core.ensureComplete(2831);
        }

        //Food For Flight
        if (!Story.questProgression(2832)) {
            Core.ensureAccept(2832);
            Core.killMonster("arcangrove", "LeftBack", "Left", "Gorillaphant", "Bag of Peanuts", 4);
            Core.killMonster("earthstorm", "r9", "Left", "Amethite", "Stick of Rock Candy", 3);
            Core.killMonster("palooza", "Act1", "Left", "Tune-a-Fish", "Tune-A-Fish Tuna Can", 2);
            Core.killMonster("giant", "r1", "Left", "*", "Bag of Pretzels", 4);
            Core.ensureComplete(2832);
        }

        //Vomit Comet
        if (!Story.questProgression(2833)) {
            Core.ensureAccept(2833);
            Core.killMonster("bamboo", "r3", "Left", "Tanuki", "Umeboshi Plum", 4);
            Core.killMonster("yokaigrave", "Enter2", "Right", "*", "Ginger");
            Core.killMonster("guru", "Field2", "Left", "*", "Peppermint Leaf", 3);
            Story.buyQuest(2833, "yulgar", 16, "Absorbent Mop");
            Core.ensureComplete(2833);
        }

        Farm.chronoSpanREP(2);

        //Sweet Dreams for a Safe Flight
        if (!Story.questProgression(2834)) {
            Core.ensureAccept(2834);
            Core.buyItem("thespan", 439, "Comfy Pillow");
            Core.ensureComplete(2834);
        }

        //Reach VR Room
        Story.mapItemQuest(2837, "hyperspace", 1742);

        //Speak with H.A.L.
        Story.mapItemQuest(2838, "hyperspace", 1743);

        //Defeat H.A.L.
        Story.mapItemQuest(2839, "hyperspace", 1744);

        //Spare J6
        Story.chainQuest(2856);

        //Choose J6 cutscene
        Story.mapItemQuest(2840, "hyperspace", 1745);

        if (!Core.isCompletedBefore(2846)) {
            Core.jump("R10");
            Core.join("alley");
        }

        //Reduce, Respawn, Recycle
        Story.killQuest(2841, "alley", "Trash Can");

        //Mind your Manners
        Story.killQuest(2842, "alley", "Thug Bully");

        //FUNdraisers!
        Story.killQuest(2843, "alley", "Thug Bully");

        //Bully the Thug
        Story.killQuest(2849, "alley", "Thug Boss");

        //Security Breach
        if (!Story.questProgression(2844)) {
            Core.ensureAccept(2844);
            Core.huntMonster("alley", "Security Cam", "Security Cam Destroyed", 5);
            Core.huntMonster("alley", "Guard Robot", "Destroyed Robo Guard", 7);
            Core.ensureComplete(2844);
        }

        //No Sniffin' Around
        Story.killQuest(2845, "alley", "Guard Dog Robot");

        //Key to Winning
        Story.killQuest(2846, "alley", "Guard Dog Robot");

        //Watch Chapter 1
        Story.mapItemQuest(2850, "hyperspace", 1751);

        //Watch Chaper 1
        if (!Core.isCompletedBefore(2851)) {
            Core.jump("R10");
            Core.join("alley");
            Core.sleep(2000);
        }
        //Save the Girl
        Story.killQuest(2851, "alley", "Thug Minion");

        //J6 Epilogue
        Story.mapItemQuest(2852, "hyperspace", 1749);

        //J6 Epilogue
        Story.chainQuest(2858);

        Core.sellItem("Hyperium Spaceship Key");
    }
}

export default function main(): void {
        Core.setOptions();

        new J6Saga().j6();

        Core.setOptions(false);
}
