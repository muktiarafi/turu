/*
name: null
description: null
tags: null
*/
// Port of Scripts/Story/Doomwood/CoreDoomwood.cs.
// Not ported (FarmerJoe's DoAll doesn't reach them): CompleteDoomwood, DoomwoodPart1, DoomwoodPart2, DoomwoodForest, ChoppingMaul, NecroTower, NecroU, TempleOfTheLight, MadScientistsLab, NecropolisDungeon, NecropolisCavern.

import { bot } from "turu";
import type { Quest } from "turu";
import { ClassType, Core } from "../../core/CoreBots";
import { CoreStory } from "../../core/CoreStory";

const Story = new CoreStory();

export class CoreDoomwood {
    doomwoodPart3(): void {
        this.thornsgarde();
        this.stonewood();
        this.techDungeon();
        this.stonewoodForest();
        this.techFortress();
    }

    thornsgarde(): void {
        if (Core.isCompletedBefore(7653))
            return;

        Story.preLoad(this.thornsgarde);

        //7589    Bullies
        if (!Story.questProgression(7589)) {
            Core.ensureAccept(7589);
            Core.huntMonster("thorngarde", "CryptHacker", "CryptHacker Slain", 4);
            Core.huntMonster("thorngarde", "NecroDrone", "NecroDrone Slain", 1);
            Core.huntMonster("thorngarde", "NecroMech", "NecroMech Slain", 1);
            Core.ensureComplete(7589);
        }

        //7590    Free the People
        if (!Story.questProgression(7590)) {
            Core.ensureAccept(7590);
            Core.getMapItem(7485, 5, "thorngarde");
            Core.huntMonster("thorngarde", "NecroMech", "Deadtech Key", 5);
            Core.ensureComplete(7590);
        }

        //7591    Sample It
        Story.killQuest(7591, "thorngarde", "NecroMech");

        //7592    Swat the Flies
        Story.killQuest(7592, "thorngarde", "CryptHacker");

        //7593    Reclaim the Supplies
        if (!Story.questProgression(7593)) {
            Core.ensureAccept(7593);
            Core.huntMonster("thorngarde", "CryptHacker", "Cured Meat", 3);
            Core.huntMonster("thorngarde", "NecroMech", "Bag of Grain", 3);
            Core.huntMonster("thorngarde", "NecroDrone", "Jug of Water", 5);
            Core.ensureComplete(7593);
        }

        //7594    Who's That?
        Story.mapItemQuest(7594, "thorngarde", 7486);

        //7595    Find my Stuff
        if (!Story.questProgression(7595)) {
            Core.ensureAccept(7595);
            Core.getMapItems([ [7487, 1], [7488, 1] ], "thorngarde");
            Core.huntMonster("thorngarde", "NecroDrone", "Kyger's Bunny", 1);
            Core.ensureComplete(7595);
        }

        //7596    Save my Friends
        if (!Story.questProgression(7596)) {
            Core.ensureAccept(7596);
            Core.getMapItems([ [7489, 1], [7490, 1], [7491, 1] ], "thorngarde");
            Core.huntMonster("thorngarde", "NecroMech", "NecroMech Slain", 3);
            Core.ensureComplete(7596);
        }

        //7597    Pull the Wires
        Story.killQuest(7597, "thorngarde", "CryptHacker");

        //7598    Dress-up Time!
        if (!Story.questProgression(7598)) {
            Core.ensureAccept(7598);
            Core.huntMonster("thorngarde", "CryptHacker", "CryptHacker Headgear", 1);
            Core.huntMonster("thorngarde", "NecroDrone", "Deadtech Pieces", 5);
            Core.ensureComplete(7598);
        }

        //7599    I Need a Distraction
        if (!Story.questProgression(7599)) {
            Core.ensureAccept(7599);
            Core.huntMonster("thorngarde", "NecroMech", "NecroMech Slain", 5);
            Core.huntMonster("thorngarde", "CryptHacker", "CryptHacker Slain", 5);
            Core.huntMonster("thorngarde", "NecroDrone", "NecroDrone Slain", 5);
            Core.ensureComplete(7599);
        }

        //7600    Zyrus is Lost
        Story.killQuest(7600, "thorngarde", "Zyrus the BioKnight");

        //7601    Investigate the Tech
        if (!Story.questProgression(7601)) {
            Core.ensureAccept(7601);
            Core.huntMonster("thorngarde", "NecroDrone", "Deadtech Power Core", 7);
            Core.huntMonster("thorngarde", "NecroMech", "NecroMech Targeting Systems", 5);
            Core.huntMonster("thorngarde", "CryptHacker", "CryptHacker Circuitry", 15);
            Core.huntMonster("thorngarde", "Zyrus the BioKnight", "BioKnight Engine", 3);
            Core.ensureComplete(7601);
        }
    }

    stonewood(): void {
        if (Core.isCompletedBefore(7653))
            return;

        this.thornsgarde();

        Story.preLoad(this.stonewood);

        //7603    Defeat the Deadtech
        Story.killQuest(7603, "stonewood", "NecroDrone");

        //7604    Gather Spell Components
        if (!Story.questProgression(7604)) {
            Core.ensureAccept(7604);
            Core.getMapItem(7510, 6, "stonewood");
            Core.huntMonster("stonewood", "Doomwood Treeant", "Treeant Pollen", 8);
            Core.ensureComplete(7604);
        }

        //7605    A Few Things More
        if (!Story.questProgression(7605)) {
            Core.ensureAccept(7605);
            Core.huntMonster("stonewood", "NecroDrone", "Fresh Marrow", 8);
            Core.huntMonster("stonewood", "NecroMech", "Pristine Wishbone", 1);
            Core.ensureComplete(7605);
        }

        //7606    Search for the Axe
        Story.mapItemQuest(7606, "stonewood", 7511, 4);
        Story.mapItemQuest(7606, "stonewood", 7512);

        //7607    Defeat the BioKnight
        Story.killQuest(7607, "stonewood", "BioKnight");

        //7608    Take the Axe!
        Story.mapItemQuest(7608, "stonewood", 7513);
    }

    techDungeon(): void {
        if (Core.isCompletedBefore(7636))
            return;

        this.stonewood();

        Story.preLoad(this.techDungeon);

        //7609    Find the Keys
        Story.killQuest(7609, "techdungeon", "NecroMech");

        //7610    Ew, Rats
        if (!Story.questProgression(7610)) {
            Core.ensureAccept(7610);
            Core.getMapItem(7515, 6, "techdungeon");
            Core.huntMonster("techdungeon", "Rotting Rat", "Rat Slain", 10);
            Core.ensureComplete(7610);
        }

        //7611    Grab Their Gear
        if (!Story.questProgression(7611)) {
            Core.ensureAccept(7611);
            Core.killMonster("techdungeon", "r4", "Left", "DoomBorg Guard", "Guard Armor Piece", 5);
            Core.killMonster("techdungeon", "r4", "Left", "DoomBorg Guard", "Guard Helm");
            Core.killMonster("techdungeon", "r4", "Left", "DoomBorg Guard", "Deadtech Polearm");
            Core.ensureComplete(7611);
        }

        //7612    Talk to the Guard
        Story.mapItemQuest(7612, "techdungeon", 7514);

        //7613    Let Them Out!
        Story.mapItemQuest(7613, "techdungeon", 7516);

        //7614    Head on Out
        if (!Story.questProgression(7614)) {
            Core.ensureAccept(7614);
            Core.getMapItem(7517, 1, "techdungeon");
            Core.huntMonster("techdungeon", "DoomBorg Guard", "Deadtech Guard Slain", 12);
            Core.ensureComplete(7614);
        }

        //7615    Take Him Down
        Story.killQuest(7615, "techdungeon", "Kalron the Cryptborg");
    }

    stonewoodForest(): void {
        if (Core.isCompletedBefore(7636))
            return;

        this.techDungeon();

        Story.preLoad(this.stonewoodForest);

        //7623    Cat Ears?
        if (!Story.questProgression(7623)) {
            Core.ensureAccept(7623);
            Core.getMapItem(7528, 1, "stonewooddeep");
            Core.ensureComplete(7623);
            Core.logger("Cutscene > Wait > Jump > Continue");
            bot.wait.forCellChange("Cut2");
            Core.sleep(2500);
            Core.jumpWait();
        }

        //7624    EN GARDE!
        if (!Story.questProgression(7624)) {
            Core.equipClass(ClassType.Solo);
            Core.ensureAccept(7624);
            // Core.Join("stonewooddeep", "r2", "Left");
            Core.killMonster("stonewooddeep", "r2", "Right", "Asherion", "Defeat Asherion");
            Core.ensureComplete(7624);
        }

        //7625    The Light Of Destiny
        if (!Story.questProgression(7625)) {
            Core.equipClass(ClassType.Farm);
            Core.ensureAccept(7625);
            Core.getMapItem(7529, 1, "stonewooddeep");
            Core.ensureComplete(7625);
            Core.logger("Cutscene > Wait > Jump > Continue");
            bot.wait.forCellChange("Cut2");
            Core.sleep(2500);
            Core.jumpWait();
        }

        //7626    Stuff for Dummies
        if (!Story.questProgression(7626)) {
            Core.ensureAccept(7626);
            Core.killMonster("stonewooddeep", "r2", "Right", "Doomwood Treeant", "Sturdy Wood", 8);
            Story.mapItemQuest(7626, "stonewooddeep", 7530, 8);
        }

        //7627    Build the Dummies
        if (!Story.questProgression(7627)) {
            Core.ensureAccept(7627);
            Core.killMonster("stonewooddeep", "r3", "Right", "Doomwood Treeant", "Area Cleared", 10);
            Core.getMapItem(7531, 6, "stonewooddeep");
            Core.ensureComplete(7627);
            Core.jump("r2");
            Core.sleep(2500);
            Core.jump("r3");
        }

        //7628    Battle the Dummies
        if (!Story.questProgression(7628)) {
            // Core.Logger("Updatin the map because dummys are dumb");
            // Core.Join("whitemap");
            Core.ensureAccept(7628);
            Core.killMonster("stonewooddeep", "r3", "Left", "Target Dummy", "Target Dummy Slain", 6);
            Core.ensureComplete(7628);
        }

        //7629    Lesson 1: Bravery
        if (!Story.questProgression(7629)) {
            Core.ensureAccept(7629);
            Core.killMonster("stonewooddeep", "r3", "Right", "Doomwood Slime", "Slime Slain", 10);
            Core.ensureComplete(7629);
        }

        //7630    Lesson 2: Armor
        if (!Story.questProgression(7630)) {
            Core.ensureAccept(7630);
            Core.killMonster("stonewooddeep", "r3", "Right", "Doomwood Slime", "Stolen Armor", 7);
            Core.ensureComplete(7630);
        }

        //7631    Lesson 3: Protection
        if (!Story.questProgression(7631)) {
            Core.ensureAccept(7631);
            Core.killMonster("stonewooddeep", "r7", "Left", "CryptHacker", "Crypthacker Slain", 10);
            Core.killMonster("stonewooddeep", "r7", "Left", "CryptHacker", "Unidentified Clue");
            Core.ensureComplete(7631);
        }

        //7632    Lesson 4: Gather Intelligence
        Story.killQuest(7632, "stonewooddeep", "CryptHacker");

        //7633    Lesson 5: Demolition
        if (!Story.questProgression(7633)) {
            Core.ensureAccept(7633);
            Core.killMonster("stonewooddeep", "r3", "Right", "Doomwood Slime", "Acid Ooze", 6);
            Core.killMonster("stonewooddeep", "r7", "Right", "NecroDrone", "Explosive Tech", 6);
            Core.ensureComplete(7633);
        }

        //7634    Lesson 6: Searching
        Story.mapItemQuest(7634, "stonewooddeep", 7532);

        //7635    Get the Axe
        if (!Story.questProgression(7635)) {
            Story.mapItemQuest(7635, "stonewooddeep", 7533);
            Core.logger("Cutscene > Wait > Jump > Continue");
            bot.wait.forCellChange("Cut2");
            Core.sleep(2500);
            Core.jumpWait();
        }

        //7636    Never Give Up
        Core.equipClass(ClassType.Solo);
        Story.killQuest(7636, "stonewooddeep", "Sir Kut");
    }

    techFortress(): void {
        if (Core.isCompletedBefore(7653))
            return;

        this.stonewoodForest();

        Story.preLoad(this.techFortress);

        //7637    I CPU U!
        Story.killQuest(7637, "techfortress", "CryptHacker");

        //7638    Keep Droning On
        Story.killQuest(7638, "techfortress", "NecroDrone");

        //7639    Gathering Data
        Story.killQuest(7639, "techfortress", "CryptHacker");

        //7640    Signal Flare
        if (!Story.questProgression(7640)) {
            Core.ensureAccept(7640);
            Core.getMapItem(7561, 1, "techfortress");
            Core.huntMonster("techfortress", "NecroDrone", "Flare", 1);
            Core.ensureComplete(7640);
        }

        //7641    Medal Time
        if (!Story.questProgression(7641)) {
            Core.ensureAccept(7641);
            Core.huntMonster("techfortress", "NecroDrone", "NecroMedals", 5);
            Core.ensureComplete(7641);
        }

        //7642    Hidden Data
        Story.killQuest(7642, "techfortress", "NecroMech");

        //7643    Pass the CPU
        Story.killQuest(7643, "techfortress", "CPU");

        //7644    The Guards
        Story.killQuest(7644, "techfortress", "DoomBorg Guard");

        //7645    Circui-try This
        Story.killQuest(7645, "techfortress", "DoomBorg Guard");

        //7646    The Thing
        Story.killQuest(7646, "techfortress", "Vortrix");

        //7653    The Final Form
        Story.killQuest(7653, "techfortress", "MechaVortrix");
    }

    aqwZombies(): void {
        if (Core.isCompletedBefore(2128))
            return;

        this.techFortress();

        Story.preLoad(this.aqwZombies);

        //2093    Undead Assault
        Story.killQuest(2093, "doomhaven", "Skeletal Soldier", { autoCompleteQuest: false });
        //insurance
        if (bot.quests.canComplete(2093))
            Core.ensureComplete(2093);

        //2094    Skull Crusher Mountain
        Story.killQuest(2094, "doomhaven", "Skeletal Ice Mage");

        //2095    The Undead Giant
        Story.killQuest(2095, "doomhaven", "Angry Undead Giant");

        //2096    Talk to the Knights
        Story.mapItemQuest(2096, "doomhaven", 1174, 5);

        //2097    Defend the Throne Room
        Story.killQuest(2097, "doomhaven", "Skeletal Viking");

        for (const Quest of [
                2117, // Rolith Defeated
                2120, // Lair
                2121, // Mythsong
                2122, // Arcangrove
                2123, // Willowshire
                2119, // Battleon
            ]
        ) {
            Story.chainQuest(Quest);
            bot.wait.forQuestComplete(Quest);
        }

        //2124    Keep the Area Clear
        Story.killQuest(2124, "doomwar", "Angry Zombie");

        //2125    Defeat Zombie Dragons
        Story.killQuest(2125, "doomwar", "Zombie Dragon");

        //2126    Defeat Your Fallen Friends
        if (!Story.questProgression(2126)) {
            Core.equipClass(ClassType.Farm);
            Core.ensureAccept(2126);
            Core.killMonster("doomwar", "r5", "Left", "*", "Fallen Friend Defeated", 4);
            Core.ensureComplete(2126);
        }

        //2127    Long Unlive the King!
        Core.equipClass(ClassType.Solo);
        Story.killQuest(2127, "doomwar", "Zombie King Alteon");

        //2128    Dark Sepulchure Must be Slain!
        Story.killQuest(2128, "sepulchure", "Dark Sepulchure");
    }
}
