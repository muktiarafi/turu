/*
name: null
description: null
tags: null
*/
// Port of Scripts/Story/QueenofMonsters/CoreQoM.cs.
// Not ported (FarmerJoe's DoAll doesn't reach them): CompleteEverything, aTheftofLight, DoomwoodPaladinsTrial, DarkoviaDarkDiaspora, ShadowfallDarknessRising, SwordhavenTheNewWorld, TheDestroyer, TheBook, TheQueensSecrets.

import { ClassType, Core } from "../../core/CoreBots";
import { CoreStory } from "../../core/CoreStory";

const Story = new CoreStory();

export class CoreQOM {
    theReshaper(TerraneMerge = false): void {
        //Progress Check
        if (Core.isCompletedBefore(5877))
            return;

        //Preload Quests
        Story.preLoad(this.theReshaper);

        //Capture the Misshapen
        if (!Story.questProgression(5849)) {
            Core.ensureAccept(5849);
            Core.huntMonster("pilgrimage", "SpiderWing", "Spiderwing Captured", 4);
            Core.huntMonster("pilgrimage", "Urstrix", "Urstrix Captured", 4);
            Core.ensureComplete(5849);
        }

        //Defeat the Parasites
        Story.killQuest(5850, "Pilgrimage", "Ravenous Parasite");

        //The Source
        Story.killQuest(5851, "Pilgrimage", "Extrikiti");

        //Find Lucky!
        if (!Story.questProgression(5852)) {
            Core.ensureAccept(5852);
            Core.getMapItem(5289, 1, "Pilgrimage");
            Core.getMapItem(5288, 3, "Pilgrimage");
            Core.huntMonster("pilgrimage", "Extrikiti", "Tuft of Fur", 3);
            Core.huntMonster("pilgrimage", "Extrikiti", "Lucky's Collar", 1);
            Core.ensureComplete(5852);
        }

        //Gather a Scouting Party
        if (!Story.questProgression(5853)) {
            Core.ensureAccept(5853);
            Core.getMapItem(5290, 6, "pilgrimage");
            Core.getMapItem(5291, 1, "pilgrimage");
            Core.ensureComplete(5853);
        }

        //Defeat the Parasites
        if (!Story.questProgression(5854)) {
            Core.ensureAccept(5854);
            Core.getMapItem(5292, 1, "Pilgrimage");
            Core.huntMonster("pilgrimage", "Ravenous Parasite", "Monsters Slain", 10);
            Core.ensureComplete(5854);
        }

        //Bad Dog
        Story.killQuest(5855, "Pilgrimage", "Lucky");

        //Connect to the Earth
        if (!Story.questProgression(6276)) {
            Core.ensureAccept(6276);
            Core.getMapItem(5769, 5, "guardiantree");
            Core.huntMonster("guardiantree", "Blossoming Treeant", "Treeant Cuttings", 5);
            Core.ensureComplete(6276);
        }

        //Help the Hedgies
        Story.mapItemQuest(6277, "guardiantree", 5776, 5);
        Story.mapItemQuest(6277, "guardiantree", 5770, 5);

        //Cleanse the Corrupted Zards
        Story.killQuest(6278, "guardiantree", "Corrupted Zard");

        //Plant the Seed
        if (!Story.questProgression(6279)) {
            Core.ensureAccept(6279);
            Core.killMonster("guardiantree", "r2a", "Bottom", "Seed Spitter", "Perfect Seed");
            Core.getMapItem(5771, 1, "guardiantree");
            Core.ensureComplete(6279);
        }

        //Reach the Top
        Story.mapItemQuest(6280, "guardiantree", 5772);

        //Cointain the Pollen
        Story.killQuest(6281, "guardiantree", "Blossoming Treeant");

        //Pass Through the Pollen
        if (!Story.questProgression(6282)) {
            Core.ensureAccept(6282);
            Core.getMapItem(5773, 1, "guardiantree");
            Core.huntMonster("guardiantree", "Pollen Cloud", "Pollen Clouds Destroyed", 5);
            Core.ensureComplete(6282);
        }

        //Reinvigorate the Sprout
        if (!Story.questProgression(6283)) {
            Core.ensureAccept(6283);
            Core.killMonster("guardiantree", "r8", "Left", "Seed Spitter", "Life Energy", 8);
            Core.ensureComplete(6283);
        }

        //Up We Go!
        Story.mapItemQuest(6284, "guardiantree", 5774);

        //Grow a Bridge
        if (!Story.questProgression(6285)) {
            Core.ensureAccept(6285);
            Core.getMapItem(5775, 2, "guardiantree");
            Core.huntMonster("guardiantree", "Myconid", "Fungal Spores", 6);
            Core.ensureComplete(6285);
        }

        //Take Down Terrane
        if (!Story.questProgression(6286)) {
            Core.ensureAccept(6286);
            Core.killMonster("guardiantree", "r12", "Left", "Terrane");
            Core.ensureComplete(6286);
        }
        if (TerraneMerge)
            return;

        //Explore the Cavern
        if (!Story.questProgression(5856)) {
            Core.ensureAccept(5856);
            Core.getMapItem(5293, 1, "TwistedCavern");
            Core.huntMonster("twistedcavern", "Extrikiti", "Extrikitis Slain", 8);
            Core.ensureComplete(5856);
        }

        //Capture the Insects
        Story.killQuest(5857, "TwistedCavern", "Infesting Swarm");

        //Face the Consequences
        if (!Story.questProgression(5858)) {
            Core.ensureAccept(5858);
            Core.huntMonster("TwistedCavern", "Fungal Lord", "Fungal Lord Slain", 6);
            Core.huntMonster("TwistedCavern", "Seed Stalker", "Seed Stalker Slain", 6);
            Core.huntMonster("TwistedCavern", "Seed Stalker", "Scrap of Brown Cloth");
            Core.ensureComplete(5858);
        }

        //Stalk This
        Story.killQuest(5859, "TwistedCavern", "Seed Stalker");

        //Free the Flies
        if (!Story.questProgression(5860)) {
            Core.ensureAccept(5860);
            Core.getMapItem(5294, 1, "TwistedCavern");
            Core.huntMonster("twistedcavern", "SpiderWing", "Spiderwings Slain", 8);
            Core.ensureComplete(5860);
        }

        //Follow the Swarm
        Story.mapItemQuest(5861, "TwistedCavern", 5295, 6);

        //Leading to the Wall
        if (!Story.questProgression(5862)) {
            Core.ensureAccept(5862);
            Core.huntMonster("TwistedCavern", "Urstrix", "Urstrix Slain", 6);
            Core.huntMonster("TwistedCavern", "Fungal Lord", "Fungal Lord Slain", 4);
            Core.ensureComplete(5862);
        }

        //Get through the Wall
        Story.killQuest(5863, "TwistedCavern", "Wall of Vines");

        //Defeat the Golem
        Core.equipClass(ClassType.Solo);
        Story.killQuest(5864, "TwistedCavern", "Lore Golem");

        //Time for a Dose
        if (!Story.questProgression(5866)) {
            Core.ensureAccept(5866);
            Core.huntMonster("brokenwoods", "Urstrix", "Urstrix Dosed", 3);
            Core.huntMonster("brokenwoods", "SpiderWing", "Spiderwing Dosed", 3);
            Core.ensureComplete(5866);
        }

        //Comparing Samples
        if (!Story.questProgression(5867)) {
            Core.ensureAccept(5867);
            Core.huntMonster("brightoak", "Hootbear", "Hootbear Feathers", 2);
            Core.huntMonster("BrokenWoods", "Urstrix", "Urstrix Feathers", 2);
            Core.huntMonster("pines", "LeatherWing", "LeatherWing Claws", 2);
            Core.huntMonster("BrokenWoods", "SpiderWing", "SpiderWing Claws", 2);
            Core.ensureComplete(5867);
        }

        //Examine the Foliage
        if (!Story.questProgression(5868)) {
            Core.ensureAccept(5868);
            Core.getMapItem(5296, 4, "BrokenWoods");
            Core.getMapItem(5297, 4, "BrokenWoods");
            Core.huntMonster("brokenwoods", "Fungal Lord", "Fungal Lord Slime", 4);
            Core.ensureComplete(5868);
        }

        //Investigate the Goo
        Story.killQuest(5869, "BrokenWoods", "Extrikiti");

        //Second Dose
        if (!Story.questProgression(5870)) {
            Core.ensureAccept(5870);
            Core.huntMonster("brokenwoods", "Urstrix", "Urstrix Dosed", 3);
            Core.huntMonster("brokenwoods", "SpiderWing", "Spiderwing Dosed", 3);
            Core.ensureComplete(5870);
        }

        //Still Not Working
        if (!Story.questProgression(5871)) {
            Core.ensureAccept(5871);
            Core.huntMonster("charredpath", "Ravenous Parasite", "Captured Parasite", 3);
            Core.huntMonster("charredpath", "Plague Spreader", "Captured Plague Spreader", 3);
            Core.ensureComplete(5871);
        }

        //Get the Final Ingredients
        if (!Story.questProgression(5872)) {
            Core.ensureAccept(5872);
            Core.buyItem("embersea", 1100, 1749, 25);
            Core.huntMonster("Arcangrove", "Seed Spitter", "Uncorrupted Spitter Seeds", 3);
            Core.huntMonster("poisonforest", "Treeant", "Treeant Berries", 4);
            Core.ensureComplete(5872);
        }

        //Smash the Hive
        Story.killQuest(5873, "brokenwoods", "Hive");

        //Acolyte's Medallions
        if (!Story.questProgression(5874)) {
            Core.ensureAccept(5874);
            Core.huntMonster("Kolyaban", "Poisonous Darkblood", "Acolyte's Medallion", 4);
            Core.ensureComplete(5874);
        }

        //Let's Hope This Works
        Story.killQuest(5876, "Kolyaban", "Twisted Aria");

        //Defeat Kolyaban
        Story.killQuest(5877, "Kolyaban", "Kolyaban");
    }
}
