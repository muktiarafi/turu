/*
name: null
description: null
tags: null
*/
// Port of Scripts/Story/ThroneofDarkness/CoreToD.cs.
// Not ported (FarmerJoe's DoAll doesn't reach them): CompleteToD, CastleofBones, BoneTowerAll, ParadoxPortal, DeepSpace, BaconCatFortress, LaserSharkInvasion, DeathPit, DeathPitPVP, FourthDimensionalPyramid, Yasaris.

import { bot } from "turu";
import { ClassType, Core } from "../../core/CoreBots";
import { CoreStory } from "../../core/CoreStory";

const Story = new CoreStory();

export class CoreToD {
    doAll = false;

    shiftingPyramid(): void {
        if (Core.isCompletedBefore(5187))
            return;

        if (this.doAll)
            Core.logger("Sekt - The Eternal: Shifting Pyramid");
        Story.preLoad(this.shiftingPyramid);

        // Hunt for the Infinity Codex
        if (!Story.questProgression(5166)) {
            Core.ensureAccept(5166);
            Core.killMonster("whitehole", "r2", "Left", "Vortex Mage", "Infinity Text Page", 8);
            Core.ensureComplete(5166);
        }

        // Sacrific and Survival
        if (!Story.questProgression(5167)) {
            Core.ensureAccept(5167);
            Core.huntMonster("whitehole", "Vortex Naga", "Naga Slain", 3);
            Core.huntMonster("whitehole", "Vortex Hawk", "Hawk Slain", 3);
            Core.ensureComplete(5167);
        }

        // The Cartouche of Isis
        if (!Story.questProgression(5168)) {
            Core.ensureAccept(5168);
            Core.huntMonster("whitehole", "Gate Goblin", "Balm Cartouche");
            Core.ensureComplete(5168);
        }

        // The Cartouche of Ma'at
        if (!Story.questProgression(5169)) {
            Core.ensureAccept(5169);
            Core.huntMonster("whitehole", "Vortex Walker", "Scale Cartouche Fragment", 5);
            Core.ensureComplete(5169);
        }

        // Bound to Do Good
        if (!Story.questProgression(5170)) {
            Core.ensureAccept(5170);
            Core.huntMonster("whitehole", "Dimensional Crystal", "Quartz", 3);
            Core.huntMonster("whitehole", "Gate Goblin", "Lime", 2);
            Core.huntMonster("whitehole", "Vortex Matter", "Natron");
            Core.ensureComplete(5170);
        }

        // Honor the Goddess Isis
        Story.mapItemQuest(5171, "whitehole", 4539);

        // Stick to the Task
        if (!Story.questProgression(5172)) {
            Core.ensureAccept(5172);
            Core.killMonster("whitehole", "r3", "Left", "Dimensional Crystal", "Quartz", 4);
            Core.killMonster("whitehole", "r6", "Left", "Gate Goblin", "Lime", 1);
            Core.killMonster("whitehole", "r10", "Left", "Vortex Matter", "Ash", 3);
            Core.ensureComplete(5172);
        }

        // Honor the Goddess Ma'at
        Story.mapItemQuest(5173, "whitehole", 4540, 4);
        Story.mapItemQuest(5173, "whitehole", 4542);

        // Duty is light as a feather
        if (!Story.questProgression(5174)) {
            Core.ensureAccept(5174);
            Core.huntMonster("whitehole", "Vortex Hawk", "Feather of Ma'at");
            Core.ensureComplete(5174);
        }

        // Judgement... or Justice?
        if (!Story.questProgression(5175)) {
            Core.ensureAccept(5175);
            Core.huntMonster("whitehole", "Hand of Ma'at", "Judgement Passed");
            Core.ensureComplete(5175);
        }

        // The Cartouche of Thoth
        if (!Story.questProgression(5176)) {
            Core.ensureAccept(5176);
            Core.huntMonster("whitehole", "Vortex Mage", "Scroll Cartouche");
            Core.ensureComplete(5176);
        }

        // Make Your Mark
        Story.mapItemQuest(5177, "whitehole", 4541, 4);

        // The Cartouche of Kebechet
        if (!Story.questProgression(5178)) {
            Core.ensureAccept(5178);
            Core.huntMonster("whitehole", "Vortex Naga", "Cartouche Fragment", 6);
            Core.ensureComplete(5178);
        }

        // Honor the Goddess Kebechet
        Story.mapItemQuest(5179, "whitehole", 4543);

        // Destroy to Purify
        if (!Story.questProgression(5180)) {
            Core.ensureAccept(5180);
            Core.huntMonster("whitehole", "Vortex Crystal", "Natron", 5);
            Core.ensureComplete(5180);
        }

        // Guardian of the Vortex
        if (!Story.questProgression(5181)) {
            Core.ensureAccept(5181);
            Core.huntMonster("whitehole", "Vortex Guardian", "Guardian Slain");
            Core.ensureComplete(5181);
        }

        // Stick with it
        if (!Story.questProgression(5182)) {
            Core.ensureAccept(5182);
            Core.huntMonster("whitehole", "Dimensional Crystal", "Quartz", 2);
            Core.huntMonster("whitehole", "Gate Goblin", "Lime", 4);
            Core.huntMonster("whitehole", "Vortex Matter", "Ash", 1);
            Core.ensureComplete(5182);
        }

        // Honor the God Thoth
        Story.mapItemQuest(5183, "whitehole", 4544);

        // The Brightest Cartouches
        if (!Story.questProgression(5184)) {
            Core.ensureAccept(5184);
            Core.huntMonster("whitehole", "Dimensional Crystal", "Sun Cartouche");
            Core.huntMonster("whitehole", "Dimensional Crystal", "Sky Cartouche");
            Core.huntMonster("whitehole", "Vortex Matter", "Star Cartouche");
            Core.huntMonster("whitehole", "Vortex Crystal", "Moon Cartouche");
            Core.ensureComplete(5184);
        }

        // Honor the Astral Deities
        Story.mapItemQuest(5185, "whitehole", 4545, 4);

        // Serpent of the Stars
        if (!Story.questProgression(5186)) {
            Core.ensureAccept(5186);
            Core.huntMonster("whitehole", "Mehensi Serpent", "Mehen Slain");
            Core.ensureComplete(5186);
        }

        // The Infinity Shield
        Story.mapItemQuest(5187, "whitehole", 4546);
    }

    shatterGlassMaze(): void {
        if (Core.isCompletedBefore(5313))
            return;

        if (this.doAll)
            Core.logger("Scarletta - The Sorceress: ShatterGlass Maze");
        Story.preLoad(this.shatterGlassMaze);

        if (!Story.questProgression(5298)) {
            Core.ensureAccept(5298);
            Core.getMapItem(4678, 1, "hedgemaze");

            Core.huntMonster("hedgemaze", "Knight's Reflection", "Knight's Reflection Dispersed", 1);
            Core.ensureComplete(5298);
        }

        Core.equipClass(ClassType.Solo);
        if (!Story.questProgression(5299)) {
            Core.ensureAccept(5299);
            Core.huntMonster("hedgemaze", "Mirrored Shard", "Mirror Shard Slain", 4);
            Core.huntMonsterMapID("hedgemaze", 48, "Hedge Goblin Slain", 4);
            Core.huntMonsterMapID("hedgemaze", 20, "Minotaur Slain", 3);
            Story.mapItemQuest(5299, "hedgemaze", 4679);
        }

        Story.killQuest(5300, "hedgemaze", "Knight's Reflection");

        Story.mapItemQuest(5301, "hedgemaze", 4680);

        Story.killQuest(5302, "hedgemaze", "Hedge Goblin");

        Story.mapItemQuest(5303, "hedgemaze", 4681, 12);

        Story.killQuest(5304, "hedgemaze", "Mirrored Shard");

        Story.mapItemQuest(5305, "hedgemaze", 4682);

        Core.equipClass(ClassType.Solo);
        Story.killQuest(5306, "hedgemaze", "Minotaur Prime");

        Story.mapItemQuest(5307, "hedgemaze", 4683);

        Story.mapItemQuest(5308, "hedgemaze", 4684);

        Story.mapItemQuest(5309, "hedgemaze", 4685, 5);

        if (!Story.questProgression(5310)) {
            Core.ensureAccept(5310);
            Core.killMonster("hedgemaze", "r21", "Right", "*", "Maze Monster Killed", 8);
            Core.ensureComplete(5310);
        }

        Story.mapItemQuest(5311, "hedgemaze", 4686);

        Core.equipClass(ClassType.Solo);
        Story.killQuest(5312, "hedgemaze", "Shattered Knight");

        Story.killQuest(5313, "hedgemaze", "Resurrected Minotaur");
    }

    towerofMirrors(): void {
        if (Core.isCompletedBefore(5332))
            return;

        Core.logger(`${Core.soloClass}, ${Core.farmClass}`);

        this.shatterGlassMaze();

        if (this.doAll)
            Core.logger("Scarletta - The Sorceress: Tower of Mirrors");
        Story.preLoad(this.towerofMirrors);

        // Drink Me
        if (!Story.questProgression(5314)) {
            Core.ensureAccept(5314);
            Core.huntMonster("towerofmirrors", "Glassgoyle", "Glass Powder", 8);
            Core.huntMonster("towerofmirrors", "Glass Serpent", "Reflective Venom", 4);
            Core.ensureComplete(5314);
        }

        // The Key To Success
        if (!Story.questProgression(5315)) {
            Core.ensureAccept(5315);
            Core.getMapItems([ [4691, 1], [4692, 1] ], "towerofmirrors");
            Core.huntMonster("towerofmirrors", "Silver Elemental", "Mirror Key", 1);
            Core.ensureComplete(5315);
        }

        // Phanatics
        if (!Story.questProgression(5316)) {
            Core.ensureAccept(5316);
            Core.huntMonster("towerofmirrors", "Phans", "Mosh Past the Phans", 8);
            Core.huntMonster("towerofmirrors", "Phans", "Backstage Pass", 1);
            Core.ensureComplete(5316);
        }

        // But I Have A Backstage Pass!
        if (!Story.questProgression(5317)) {
            Core.equipClass(ClassType.Solo);
            Core.ensureAccept(5317);
            Core.huntMonsterMapID("towerofmirrors", 36, "Lathannos the Reversed Defeated");
            Core.ensureComplete(5317);
        }

        // True Love
        if (!Story.questProgression(5318)) {
            Core.ensureAccept(5318);
            Core.getMapItems([ [4687, 1], [4693, 1] ], "towerofmirrors");
            Core.huntMonster("towerofmirrors", "Silver Elemental", "Andres' Mirror Key", 1);
            Core.ensureComplete(5318);
        }

        // Turn to the Left
        if (!Story.questProgression(5319)) {
            Core.ensureAccept(5319);
            Core.huntMonster("towerofmirrors", "Runway Wraith", "Runway Wraith Defeated", 6);
            Core.huntMonster("towerofmirrors", "Runway Wraith", "Avant-Garde Footwear", 1);
            Core.huntMonster("towerofmirrors", "Runway Wraith", "Retro Wig", 1);
            Core.huntMonster("towerofmirrors", "Runway Wraith", "Someone's Wearable Art Project", 1);
            Core.ensureComplete(5319);
        }

        // Now Turn to the Right
        if (!Story.questProgression(5320)) {
            Core.equipClass(ClassType.Solo);
            Core.ensureAccept(5320);
            Core.huntMonsterMapID("towerofmirrors", 40, "Lukrisio the Buffed Defeated");
            Core.ensureComplete(5320);
        }

        // Or Maybe THIS Is True Love
        if (!Story.questProgression(5321)) {
            Core.ensureAccept(5321);
            Core.getMapItems([ [4688, 1], [4694, 1] ], "towerofmirrors");
            Core.huntMonster("towerofmirrors", "Silver Elemental", "Gisele's Mirror Key", 1);
            Core.ensureComplete(5321);
        }

        // Those Harpies!
        if (!Story.questProgression(5322)) {
            Core.ensureAccept(5322);
            Core.huntMonster("towerofmirrors", "Pageant Mom", "Pageant Mom Defeated", 8);
            Core.huntMonster("towerofmirrors", "Pageant Mom", "Pageant Sash", 1);
            Core.ensureComplete(5322);
        }

        // Drink your Go-Go Juice
        if (!Story.questProgression(5323)) {
            Core.equipClass(ClassType.Solo);
            Core.ensureAccept(5323);
            Core.huntMonsterMapID("towerofmirrors", 45, "Mendeskar the Smudged Defeated");
            Core.ensureComplete(5323);
        }

        // Oh Sure, Why Not
        if (!Story.questProgression(5324)) {
            Core.ensureAccept(5324);
            Core.getMapItems([ [4689, 1], [4695, 1] ], "towerofmirrors");
            Core.huntMonster("towerofmirrors", "Silver Elemental", "Emilia's Mirror Key", 1);
            Core.ensureComplete(5324);
        }

        // Behind the Scenes
        if (!Story.questProgression(5325)) {
            Core.ensureAccept(5325);
            Core.huntMonster("towerofmirrors", "Stage Tech", "Stage Tech Defeated", 9);
            Core.huntMonster("towerofmirrors", "Stage Tech", "Rope Ladder");
            Core.ensureComplete(5325);
        }

        // In the Spotlight
        if (!Story.questProgression(5326)) {
            Core.equipClass(ClassType.Solo);
            Core.ensureAccept(5326);
            Core.huntMonsterMapID("towerofmirrors", 49, "Atticus the Warped Defeated");
            Core.ensureComplete(5326);
        }

        // Oh, I Give Up
        if (!Story.questProgression(5327)) {
            Core.ensureAccept(5327);
            Core.getMapItems([ [4690, 1], [4696, 1] ], "towerofmirrors");
            Core.huntMonster("towerofmirrors", "Silver Elemental", "Dora's Mirror Key", 1);
            Core.ensureComplete(5327);
        }

        // We Gotta Wendi-GO
        if (!Story.questProgression(5328)) {
            Core.ensureAccept(5328);
            Core.huntMonster("towerofmirrors", "Sasquatch", "Sasquatches Defeated", 10);
            Core.huntMonster("towerofmirrors", "Sasquatch", "Tracking Tag", 5);
            Core.ensureComplete(5328);
        }

        // Kick His- Wait, Did We Already Use That Pun?
        if (!Story.questProgression(5329)) {
            Core.equipClass(ClassType.Solo);
            Core.ensureAccept(5329);
            Core.huntMonsterMapID("towerofmirrors", 52, "Leofire the Shattered Defeated");
            Core.ensureComplete(5329);
        }

        // Fiend Zoned
        Story.killQuest(5330, "towerofmirrors", "Fervent Suitor");

        // Find Scarletta
        Story.mapItemQuest(5331, "towerofmirrors", 4697);

        bot.wait.forCellChange("Cut4");
        while (!bot.shouldExit && bot.player.cell !== "Enter") {
            Core.jump();
            Core.sleep();
        }

        // Defeat ... Wait. What?
        if (!Story.questProgression(5332)) {
            Core.addDrop("Blood Sorceress");
            Core.equipClass(ClassType.Solo);
            Core.ensureAccept(5332);
            Core.huntMonsterMapID("towerofmirrors", 32, "Defeat the Groglurk");
            Core.ensureComplete(5332);
        }
    }

    antiqueShop(): void {
        if (Core.isCompletedBefore(5434))
            return;

        if (this.doAll)
            Core.logger("??? - The Stranger: Antique Shop");
        Story.preLoad(this.antiqueShop);

        //cursed artifact shop - 5428
        Story.mapItemQuest(5428, "cursedshop", 4803);

        //lamps, painting and chairs, oh my!
        Story.killQuest(5429, "cursedshop", "Antique Chair");

        //the (un)Dresser
        Story.killQuest(5430, "cursedshop", "UnDresser");

        //ghost stories
        Story.killQuest(5431, "cursedshop", "Writing Desk");

        //you can't tell time
        Story.killQuest(5432, "cursedshop", "Grandfather Clock");

        //Dr. Darkwood's Robe
        Story.mapItemQuest(5433, "cursedshop", [ 4804, 4805 ]);

        //defeat the arcane sentinel
        if (!Story.questProgression(5434)) {
            Core.ensureAccept(5434);
            Core.getMapItem(4806, 1, "cursedshop");
            Core.huntMonster("cursedshop", "Arcane Sentinel", "Darkwood's Robe", 1);
            Core.ensureComplete(5434);
        }
    }

    mysteriousDungeon(): void {
        if (Core.isCompletedBefore(5451))
            return;

        this.antiqueShop();

        if (this.doAll)
            Core.logger("??? - The Stranger: Mysterious Dungeon");
        Story.preLoad(this.mysteriousDungeon);

        //cursed artifact shop - 5428
        Story.mapItemQuest(5428, "cursedshop", 4803);

        //lamps, painting and chairs, oh my!
        Story.killQuest(5429, "cursedshop", "Antique Chair");

        //the (un)Dresser
        Story.killQuest(5430, "cursedshop", "UnDresser");

        //ghost stories
        Story.killQuest(5431, "cursedshop", "Writing Desk");

        //you can't tell time
        Story.killQuest(5432, "cursedshop", "Grandfather Clock");

        //Dr. Darkwood's Robe
        Story.mapItemQuest(5433, "cursedshop", [ 4804, 4805 ]);

        //defeat the arcane sentinel
        if (!Story.questProgression(5434)) {
            Core.ensureAccept(5434);
            Core.getMapItem(4806, 1, "cursedshop");
            Core.huntMonster("cursedshop", "Arcane Sentinel", "Darkwood's Robe", 1);
            Core.ensureComplete(5434);
        }

        //Get Out Of Jail Free?
        Story.mapItemQuest(5438, "MysteriousDungeon", 4818);

        //Sorry, Skudly
        Story.killQuest(5439, "MysteriousDungeon", "Skudly");

        //A Friend In Need
        Story.mapItemQuest(5440, "MysteriousDungeon", 4808);

        //A Cryptic Messaage
        Story.mapItemQuest(5441, "MysteriousDungeon", 4809);

        //seeking answers
        Story.mapItemQuest(
            5442, 
            "MysteriousDungeon", 
            [ 4810, 4811, 4812, 4813, 4814, 4815, 4816 ]);

        //Curses!
        Story.mapItemQuest(5443, "MysteriousDungeon", 4817);

        //Skudly, Staaaahp!
        Story.killQuest(5444, "MysteriousDungeon", "Skudly");

        // Not So Mysterious After All
        Story.killQuest(5445, "MysteriousDungeon", "Mysterious Stranger");

        // Shut Up And Listen, Vaden!
        Story.killQuest(5446, "MysteriousDungeon", "Vaden", { autoCompleteQuest: false });

        // Shut Up And Listen, Xeight!
        Story.killQuest(5447, "MysteriousDungeon", "Xeight", { autoCompleteQuest: false });

        // Shut Up And Listen, Ziri
        Story.killQuest(5448, "MysteriousDungeon", "Ziri", { autoCompleteQuest: false });

        // Shut Up And Listen, Pax!
        Story.killQuest(5449, "MysteriousDungeon", "Pax", { autoCompleteQuest: false });

        // Shut Up And Listen, Sekt!
        Story.killQuest(5450, "MysteriousDungeon", "Sekt", { autoCompleteQuest: false });

        // Shut Up And Listen, Groglurk!
        Story.killQuest(5451, "MysteriousDungeon", "Scarletta", { autoCompleteQuest: false });
    }
}
