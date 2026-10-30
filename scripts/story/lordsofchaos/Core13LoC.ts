/*
name: null
description: null
tags: null
*/
// Port of Scripts/Story/LordsofChaos/Core13LoC.cs.
// Not ported (FarmerJoe's DoAll doesn't reach them): CutSceneFixer.

import { bot } from "turu";
import { CoreAdvanced } from "../../core/CoreAdvanced";
import { Alignment, ClassType, Core } from "../../core/CoreBots";
import { CoreStory } from "../../core/CoreStory";

const Story = new CoreStory();
const Adv = new CoreAdvanced();

export class Core13LoC {
    complete13LOC(withExtras = false): void {

        this.prologue();
        this.escherion();
        this.vath();
        this.kitsune();
        this.wolfwing();
        this.kimberly();
        this.ledgermayne();
        this.tibicenas();
        this.khasaandaHorc();
        this.iadoa();
        this.lionfang();
        this.xiang();
        this.alteon();
        this.hero();
        if (withExtras) {
            this.khasaandaTroll();
            this.extra();
        }
    }

    prologue(): void {
        if (Core.isCompletedBefore(6219))
            return;

        Story.preLoad(this.prologue);

        //Enter the gates
        Story.killQuest(183, "battleundera", "Skeletal Fire Mage");

        //Undead Assault
        if (!Story.questProgression(176)) {
            Core.ensureAccept(176);
            Core.huntMonster("swordhavenundead", "Skeletal Soldier", "Slain Skeletal Soldier", 5);
            Core.ensureComplete(176);
        }

        //Skull Crusher Mountain
        Story.killQuest(177, "swordhavenundead", "Skeletal Ice Mage");

        //The Undead Giant
        Story.killQuest(178, "swordhavenundead", "Undead Giant");

        //Talk to the Knights
        Story.mapItemQuest(179, "castleundead", 38, 5);

        //Defend the Throne
        Story.killQuest(180, "castleundead", "Skeletal Viking");

        //The Arrival of Drakath cutscene
        if (!bot.quests.isUnlocked(196)) {
            Core.join("castleundead", "King2", "Center");
            Core.join("Shadowfall");
            Core.sendPackets("%xt%zm%updateQuest%73922%43%1%");
            bot.send.packet(
                `%xt%zm%updateQuest%188220%41%${(Core.heroAlignment > 1 ? 1 : Core.heroAlignment)}%`);
            Core.sleep(2000);
            Core.join("shadowfall");
            Core.sleep(2000);

            //ShadowFall Quest1
            Story.chainQuest(195);
        }

        //Recover Sepulchure's Cursed Armor!
        Story.killQuest(196, "chaoscrypt", "Chaorrupted Armor");

        //Unlife Insurance
        if (!Story.questProgression(6216)) {
            Core.ensureAccept(6216);
            Core.getMapItem(39, 5, "prison");
            Core.buyItem("prison", 1559, 42993);
            Core.ensureComplete(6216);
        }

        //Enter the Crypt
        Story.mapItemQuest(6217, "chaoscrypt", 5662);

        //Rescue the Knights
        Story.killQuest(6218, "chaoscrypt", "Chaorrupted Knight");

        //Forest of Chaos
        if (!Story.questProgression(6219)) {
            Core.ensureAccept(6219);
            Core.huntMonster("forestchaos", "Chaorrupted Bear", "Chaorrupted Bear Defeated", 1);
            Core.huntMonster("forestchaos", "Chaorrupted Wolf", "Chaorrupted Wolf Defeated", 7);
            Core.ensureComplete(6219);
        }
    }

    escherion(): void {
        if (Core.isCompletedBefore(272))
            return;

        Story.preLoad(this.escherion);

        //Winged Spies
        Story.killQuest(245, "mobius", "Chaos Sp-Eye");

        //Chaos Prisoners
        Story.mapItemQuest(246, "mobius", 42, 5);

        //IMP-possible Task
        Story.killQuest(247, "mobius", "Fire Imp");

        //You Can't Miss It
        Story.mapItemQuest(260, "mobius", 44);

        //Far Sighted
        Story.killQuest(248, "mobius", "Cyclops Raider");

        //Slugfest
        Story.killQuest(249, "mobius", "Slugfit");

        //Chain Reaction
        Story.killQuest(250, "faerie", "Chainsaw Sneevil");

        //Epic Drops
        Story.mapItemQuest(251, "faerie", 43, 7);

        //Jarring Theft
        Story.killQuest(252, "faerie", "Chainsaw Sneevil");

        //Tree Hugger
        Story.killQuest(255, "faerie", "Cyclops Warlord");

        //The Second Piece
        Story.killQuest(256, "faerie", "Aracara");

        //Ruined Ruins
        Story.killQuest(257, "cornelis", "Gargoyle");

        //Energize!
        Story.mapItemQuest(261, "cornelis", 45);

        //Blueish Glow
        Story.killQuest(258, "cornelis", "Gargoyle");

        //Quickdraw
        Story.mapItemQuest(262, "cornelis", 46);

        //Arm Yourself
        Story.killQuest(259, "cornelis", "Stone Golem");

        //You've Been Framed
        Story.mapItemQuest(263, "cornelis", 47);

        //Some Assembly Required
        Story.mapItemQuest(266, "mobius", 48);

        //Teleporter Report
        Story.mapItemQuest(267, "mobius", 49);

        //Disguise!
        Story.killQuest(264, "mobius", "Cyclops Raider");

        //To-go box
        if (!Story.questProgression(265)) {
            Core.ensureAccept(265);
            Core.huntMonster("faerie", "Chainsaw Sneevil", "Cardboard Box", 4);
            Core.ensureComplete(265);
        }

        //Find the Key! (Part One)
        Story.killQuest(268, "relativity", "Cyclops Raider");

        //Find the Key! (Part Two)
        Story.killQuest(269, "relativity", "Fire Imp");

        //Find the Key! (Part Three)
        Story.killQuest(270, "relativity", "Head Gargoyle");

        //The Lake Hydra
        Story.mapItemQuest(271, "hydra", [ 50, 51, 52 ]);

        //Escherion
        if (!Story.questProgression(272)) {
            Core.ensureAccept(272);
            Core.killEscherion("Defeated Escherion", 1, { isTemp: true });
            Core.ensureComplete(272);
        }
    }

    vath(): void {
        if (Core.isCompletedBefore(363))
            return;

        Story.preLoad(this.vath);

        Core.addDrop("Snowbeard's Gold");

        //Adorable Sisters
        Story.mapItemQuest(319, "tavern", 56, 7);

        //Warm and Furry
        Story.killQuest(320, "pines", "Pine Grizzly");

        //Shell Rock
        Story.killQuest(321, "pines", "Red Shell Turtle");

        //Bear Facts
        Story.killQuest(322, "pines", "Twistedtooth");

        //The Spittoon Saloon
        Story.killQuest(324, "pines", "Red Shell Turtle");

        //Bear it all!
        Story.killQuest(325, "pines", "Pine Grizzly");

        //Leather Feathers
        Story.killQuest(326, "pines", "LeatherWing");

        //Follow your Nose!
        if (!Story.questProgression(327) || !Core.isCompletedBefore(323)) {
            Core.ensureAccept(327);
            Core.huntMonster("pines", "Pine Troll", "Pine Air Freshener");
            Core.ensureComplete(327);
            bot.wait.forPickup("Snowbeard's Gold");
            //Give Snowbeard His Gold
            Core.chainComplete(323);
        }

        //Bad Memory
        Story.mapItemQuest(344, "dwarfhold", 60);

        //Squeeze Water from Stone
        Story.killQuest(331, "mountainpath", "Ore Balboa");

        //Carrion Carrying On
        Story.killQuest(332, "mountainpath", "Vultragon");

        //Bagged Lunch
        Story.killQuest(333, "dwarfhold", "Chaos Drow");

        //Radiant Lamps
        Story.killQuest(334, "dwarfhold", "Glow Worm");

        //Having a Blast
        Story.killQuest(335, "dwarfhold", "Albino Bat");

        //Secret Weapons
        Story.killQuest(336, "dwarfhold", "Chaotic Draconian");

        //Rock Star
        Story.mapItemQuest(337, "dwarfhold", 59, 7);

        //All that Glitters
        Story.killQuest(338, "dwarfhold", "Chaos Drow");

        //Gemeralds
        Story.killQuest(339, "dwarfhold", "Chaotic Draconian");

        //Talc to Me
        // C#: `Getkey:` ... `goto Getkey` until the key is in the temp inventory.
        while (!bot.tempInv.contains("Upper City Key")) {
            Core.addDrop("Upper City Key");
            Core.ensureAccept(340);
            Core.huntMonster("dwarfhold", "Albino Bat", "Block of Talc");
            Core.ensureComplete(340);
            bot.wait.forTrue(() => bot.tempInv.contains("Upper City Key"), 20);
        }

        //Rock me Amadeus
        // //Upper City Gates
        Core.chainComplete(343);

        Core.ensureAccept(341);
        Core.huntMonster("dwarfhold", "Amadeus", "Key Mold");
        Core.ensureComplete(341);

        //Disapoofed
        Story.mapItemQuest(346, "uppercity", 61);

        //Hoodwinked
        Story.killQuest(347, "uppercity", "Drow Assassin");

        //Claws for the Cause
        Story.killQuest(348, "uppercity", "Chaotic Draconian");

        //Scrambled Eggs
        Story.killQuest(349, "uppercity", "Chaos Egg");

        //The King's Wings
        Story.killQuest(350, "uppercity", "Terradactyl");

        //Bugging Out
        Story.killQuest(351, "uppercity", "Rhino Beetle");

        //Lizard Gizzard
        Story.killQuest(352, "uppercity", "Cave Lizard");

        //Mock the Lock
        if (!bot.quests.isUnlocked(354)) {
            Core.addDrop("Thermite"); // for the next quest

            Core.ensureAccept(353);
            Core.huntMonster("dwarfprison", "Balboa", "Balboa Core", 4);
            Core.huntMonster("dwarfprison", "Chaos Drow", "Magnesium Flare");
            Core.huntMonster("dwarfprison", "Albino Bat", "Rusted Claw", 3);
            Core.ensureComplete(353);
        }

        //Like Butter
        if (!Story.questProgression(354)) {
            Core.addDrop("Thermite");

            Core.join("vath");
            Core.jump("CutCap", "Left");
            Core.sleep(2500);
            bot.quests.updateQuest(354);

            if (!Core.checkInventory("Thermite")) {
                Core.ensureAccept(353);
                Core.huntMonster("dwarfprison", "Balboa", "Balboa Core", 4);
                Core.huntMonster("dwarfprison", "Albino Bat", "Rusted Claw", 3);
                Core.huntMonster("dwarfprison", "Chaos Drow", "Magnesium Flare");
                Core.ensureComplete(353);
            }
            Core.join("dwarfprison", "Enter", "Right");
            Core.ensureComplete(354);
        }

        //Jailhouse Rock
        Story.killQuest(355, "dwarfprison", "Warden Elfis");

        //Explosives 101
        if (!Story.questProgression(356)) {
            Core.ensureAccept(356);
            Core.huntMonster("dwarfprison", "Albino Bat", "Nitrate Elements", 3);
            Core.huntMonster("dwarfprison", "Chaos Drow", "Drow Shoelaces", 3);
            Core.huntMonster("dwarfprison", "Balboa", "Flint Stone", 2);
            Core.ensureComplete(356);
        }

        //Big Bada-Boom
        if (!Story.questProgression(357)) {
            if (!Core.checkInventory("Tee-En-Tee")) {
                Core.ensureAccept(356);
                Core.huntMonster("dwarfprison", "Albino Bat", "Nitrate Elements", 3);
                Core.huntMonster("dwarfprison", "Chaos Drow", "Drow Shoelaces", 3);
                Core.huntMonster("dwarfprison", "Balboa", "Flint Stone", 2);
                Core.ensureComplete(356);
            }
            Core.join("dwarfprison");
            Story.chainQuest(357);
        }

        //Defeat Rock Roc
        Story.mapItemQuest(362, "roc", 62);

        //Facing Vath
        Story.mapItemQuest(363, "stalagbite", 63);
    }

    kitsune(): void {
        if (Core.isCompletedBefore(488))
            return;

        Story.preLoad(this.kitsune);

        //Turtle Power
        if (!Story.questProgression(380)) {
            Story.mapItemQuest(380, "yokaiboat", 64);
            Core.huntMonster("yokaiboat", "Kappa Ninja", "Kappa Ninja Slain", 7);
            Core.ensureComplete(380);
        }

        //Setting Sail to Yokai
        if (!Story.questProgression(381)) {
            Core.ensureAccept(381);
            Core.killMonster("yokaiboat", "r4", "Spawn", "*", "Sail Permit");
            Core.ensureComplete(381);
        }

        //Dragon Koi Tournament
        Story.killQuest(382, "dragonkoi", "Ryoku");

        //Dog Days
        if (!Story.questProgression(402)) {
            Core.ensureAccept(402);
            Core.huntMonster("hachiko", "Samurai Nopperabo", "Samurai Questioned", 5);
            Core.huntMonster("hachiko", "Ninja Nopperabo", "Ninja Questioned", 5);
            Core.ensureComplete(402);
        }

        //Faceless Threat
        if (!Story.questProgression(403)) {
            Core.ensureAccept(403);
            Core.killMonster("hachiko", "Ox", "Center", "Samurai Nopperabo", "Note from DT");
            Core.ensureComplete(403);
        }

        //Zodiac Puzzle Key
        if (!Story.questProgression(405)) {
            Core.ensureAccept(405);
            Core.killMonster("hachiko", "Tiger", "Center", "Samurai Nopperabo", "Rat-Ox-Tiger Piece");
            Core.killMonster("hachiko", "Snake", "Center", "Ninja Nopperabo", "Rabbit-Dragon-Snake piece");
            Core.killMonster("hachiko", "Horse", "Center", "Samurai Nopperabo", "Horse-Sheep-Monkey piece");
            Core.killMonster("hachiko", "Pig", "Center", "Ninja Nopperabo", "Rooster-Dog-Pig Piece");
            Core.ensureComplete(405);
        }

        //Rescue!
        Story.killQuest(406, "hachiko", "Dai Tengu");

        //Jinmenju Tree
        Story.mapItemQuest(466, "bamboo", 90);

        //Yokai Bandits
        if (!Story.questProgression(467)) {
            Core.ensureAccept(467);
            Core.huntMonster("bamboo", "Tanuki", "Jinmenju Fruit Core", 5);
            Core.huntMonster("bamboo", "Tanuki", "Charred Paper Talisman", 1);
            Core.ensureComplete(467);
        }

        //The Fiery Fiend
        Story.killQuest(468, "bamboo", "SoulTaker");

        //Dumpster Diving
        Story.mapItemQuest(469, "junkyard", 91);

        //Reduce, Respawn, Recycle
        if (!Story.questProgression(470)) {
            Core.ensureAccept(470);
            Core.killMonster("junkyard", "Enter", "Left", 1, "Wild Kara-Kasa", 5);
            Core.killMonster("junkyard", "r2", "Left", 4, "Wild Bura-Bura", 4);
            Core.killMonster("junkyard", "r2", "Left", 3, "Wild Biwa-Bokuboku", 3);
            Core.killMonster("junkyard", "r3", "Left", 6, "Wild Koto-Furunushi", 2);
            Core.killMonster("junkyard", "Enter", "Left", 2, "Wild Bakezouri", 1);
            Core.ensureComplete(470);
        }

        //The Hunt for the Hag
        Story.killQuest(471, "junkyard", "Onibaba");

        //Su-she
        if (!Story.questProgression(473)) {
            Core.ensureAccept(473);
            Core.huntMonster("yokairiver", "Funa-yurei", "Nori Recipe", 1);
            Core.huntMonster("yokairiver", "Funa-yurei", "Cucumber Recipe", 1);
            Core.huntMonster("yokairiver", "Funa-yurei", "Sumeshi Recipe", 1);
            Core.ensureComplete(473);
        }

        //Kappa Cuisine
        if (!Story.questProgression(474)) {
            Core.ensureAccept(474);
            Core.killMonster("yokairiver", "r4", "Left", "Kappa Ninja", "Dried Nori Leaf", 6);
            Core.killMonster("yokairiver", "r4", "Left", "Kappa Ninja", "Sumeshi Bundle", 3);
            Core.killMonster("yokairiver", "r4", "Left", "Kappa Ninja", "Fresh Cucumber", 1);
            Core.getMapItem(92, 1, "yokairiver");
            Core.ensureComplete(474);
        }

        //Hisssssy fit
        Story.killQuest(476, "yokairiver", "Nure Onna");

        //The Purrrfect Crime
        Story.killQuest(477, "yokaigrave", "Skello Kitty");

        //The Face Off
        if (!Story.questProgression(478)) {
            Core.ensureAccept(478);
            Core.huntMonster("yokaigrave", "Ninja Nopperabo", "Ninja Face Mask", 4);
            Core.huntMonster("yokaigrave", "Samurai Nopperabo", "Samurai Face Mask", 4);
            Core.ensureComplete(478);
        }

        //Confront Neko Mata
        Story.killQuest(479, "yokaigrave", "Neko Mata");

        //Defeat O-dokuro
        Story.killQuest(481, "odokuro", "O-dokuro");

        //Defeat O-Dokuro's Head
        Story.killQuest(484, "yokaiwar", "O-Dokuro's Head");

        //Defeat Kitsune
        Story.killQuest(488, "kitsune", "kitsune");
    }

    wolfwing(): void {
        if (Core.isCompletedBefore(597) || Core.isCompletedBefore(598))
            return;

        Story.preLoad(this.wolfwing);

        Core.addDrop("Red's Big Wolf Slaying Axe");

        //Grave Mission
        Story.mapItemQuest(494, "darkoviagrave", 97);

        //Lending a Helping Hand
        Story.killQuest(495, "darkoviagrave", "Skeletal Fire Mage");

        //Bone Appetit
        Story.killQuest(496, "darkoviagrave", "Rattlebones");

        //Batting Cage
        Story.killQuest(497, "darkoviagrave", "Albino Bat");

        //His Bark is worse than his Blight
        Story.killQuest(498, "darkoviagrave", "Blightfang");

        // Vicious Wolves
        Story.killQuest(308, "greenguardeast", "Wolf");

        // Slimes to the West, Slimes Everywhere
        Story.killQuest(309, "greenguardwest", "Slime");

        // Scaly Menace
        Story.killQuest(310, "greenguardwest", "Frogzard");

        // Overgrown Spiders
        Story.killQuest(311, "greenguardeast", "Spider");

        // Lil' Red + Can I axe you something?
        if (!Core.isCompletedBefore(516)) {
            Core.addDrop("Red's Big Wolf Slaying Axe");
            Core.ensureAccept(514);
            Core.ensureAccept(515);
            Core.huntMonster("greenguardeast", "Spider", "Spider Documentation");
            Core.huntMonster("greenguardeast", "Wolf", "Wolf Documentation");
            Core.huntMonster("greenguardwest", "Slime", "Slime Documentation");
            Core.huntMonster("greenguardwest", "Frogzard", "Frogzard Documentation");
            Core.killMonster("greenguardwest", "West12", "Up", "Big Bad Boar", "Wereboar Documentation");
            Core.ensureComplete(515);
            bot.wait.forPickup("Red's Big Wolf Slaying Axe");
            Core.ensureComplete(514);
        }

        // A Dire Situation
        Story.killQuest(516, "darkoviaforest", "Dire Wolf");

        //Blood, Sweat, and Tears
        if (!Story.questProgression(517)) {
            Core.ensureAccept(517);
            Core.huntMonster("darkoviaforest", "Blood Maggot", "Vial of Blood", 3);
            Core.huntMonster("darkoviaforest", "Blood Maggot", "Vial of Sweat", 2);
            Core.huntMonster("darkoviaforest", "Blood Maggot", "Vial of Tears");
            Core.ensureComplete(517);
        }

        //What a Lich!
        Story.killQuest(518, "darkoviaforest", "Lich of the Stone");

        //Feeding Grounds
        Story.killQuest(519, "safiria", "Blood Maggot");

        //Going Batty
        Story.killQuest(520, "safiria", "Albino Bat");

        //Lycan Knights
        Story.killQuest(521, "safiria", "Chaos Lycan");

        //Twisted Paw
        Story.killQuest(522, "safiria", "Twisted Paw");

        //A Gift Of Meat
        Story.killQuest(534, "lycan", "Dire Wolf");

        //No Respect
        if (!Story.questProgression(535)) {
            Core.ensureAccept(535);
            Core.huntMonster("lycan", "Lycan Knight", "Lycan Knight Defeated", 2);
            Core.huntMonster("lycan", "Lycan", "Lycan Defeated", 5);
            Core.ensureComplete(535);
        }

        //Vampire Knights
        Story.killQuest(536, "lycan", "Chaos Vampire Knight");

        //Sanguine
        if (!Story.questProgression(537)) {
            Core.ensureAccept(537);
            Core.huntMonster("lycan", "Sanguine", "Sanguine Mask");
            Core.ensureComplete(537);
        }

        //War Boss Fight
        Story.chainQuest(552);

        //Search and Report
        if (!Story.questProgression(564)) {
            Core.ensureAccept(564);
            Core.killMonster("lycanwar", "Boss", "Left", "Edvard");
            Core.sleep(5000);
            Story.mapItemQuest(564, "chaoscave", 107);
        }

        //The Key is the Key
        Story.killQuest(565, "chaoscave", "Werepyre");

        //Secret Words
        if (!Story.questProgression(566)) {
            Core.ensureAccept(566);
            Core.killMonster("chaoscave", "r3", "Left", "Werepyre", "Secret Words");
            Core.ensureComplete(566);
        }

        //DracoWerePyre
        if (!Story.questProgression(567)) {
            bot.quests.updateQuest(567);
            Core.ensureAccept(567);
            Core.killMonster("chaoscave", "r5", "Left", "DracoWerePyre", "DracoWerePyre Defeated");
            Core.ensureComplete(567);
            bot.quests.updateQuest(597);
        }

        //Wolfwing
        if (!Story.questProgression(597)) {
            if (Core.heroAlignment !== 1)
                Core.changeAlignment(Alignment.Good);
            Story.chainQuest(597);
        }
    }

    kimberly(): void {
        if (Core.isCompletedBefore(710))
            return;

        Story.preLoad(this.kimberly);

        //Stairway to Heaven
        if (!Story.questProgression(648)) {
            Core.ensureAccept(648);
            Core.huntMonster("stairway", "Grateful Undead", "Grateful Undead Shirt", 4);
            Core.huntMonster("stairway", "Rock Lobster", "Rock Lobster Tail", 4);
            Core.ensureComplete(648);
        }

        //Rolling Stones
        Story.killQuest(649, "stairway", "Rock Lobster");

        //Light My Fire
        Story.killQuest(650, "stairway", "Grateful Undead");

        //Knockin' on Haven's Door
        if (!Story.questProgression(651)) {
            Core.ensureAccept(651);
            Core.huntMonster("stairway", "Elwood Bruise", "Defeated Elwood", 1);
            Core.huntMonster("stairway", "Jake Bruise", "Defeated Jake", 1);
            Core.ensureComplete(651);
        }

        //Staying Alive
        Story.killQuest(658, "beehive", "Stinger");

        //Killer Queen
        Story.killQuest(659, "beehive", "Killer Queen Bee");

        //Satisfaction
        if (!Story.questProgression(660)) {
            Core.ensureAccept(660);
            Core.killMonster("beehive", "r6", "Right", "*", "No Shoes!");
            Core.ensureComplete(660);
        }

        //Dance with Great Godfather of Souls
        if (!Story.questProgression(661)) {
            Core.join("beehive");
            Story.chainQuest(661);
        }

        //Bad Moon Rising
        if (!Story.questProgression(675)) {
            Core.ensureAccept(675);
            Core.killMonster("orchestra", "R7", "Down", "Mozard", "Fire Flea", 15);
            Core.ensureComplete(675);
        }

        //Burning Down The House
        if (!Story.questProgression(676)) {
            Core.ensureAccept(676);
            //yes there's a space...
            Core.killMonster("orchestra", "R4", "Down", "Pachelbel's Cannon ", "Cannon Powder");
            Core.ensureComplete(676);
        }

        //Superstition
        Story.killQuest(677, "orchestra", "Mozard");

        //Soul Man
        Story.killQuest(678, "orchestra", "Faust");

        //Great gig in the Sky
        if (!Story.questProgression(4827)) {
            Core.ensureAccept(4827);
            Core.huntMonster("stairway", "Rock Lobster", "Lobster Rocked", 5);
            Core.huntMonster("stairway", "Grateful Undead", "Ungrateful Undead", 5);
            Core.ensureComplete(4827);
        }

        //Mythsong War Cutscene
        Story.chainQuest(707);

        //Pony Gary Yellow
        if (!Story.questProgression(709)) {
            Core.ensureAccept(709);
            Core.huntMonster("palooza", "Pony Gary Yellow", "Pony Gary Yellow Defeated");
            Core.ensureComplete(709);
        }

        //Kimberly
        if (!Story.questProgression(710)) {
            Core.ensureAccept(710);
            Core.huntMonster("palooza", "Kimberly", "Kimberly Defeated");
            Core.ensureComplete(710);
        }
    }

    ledgermayne(): void {
        if (Core.isCompletedBefore(847))
            return;

        Story.preLoad(this.ledgermayne);

        //Observing the Observatory
        Story.mapItemQuest(805, "arcangrove", 139);

        //Ewa the Treekeeper
        Story.mapItemQuest(806, "cloister", 142);

        //Bear Necessities of LifeRoot
        Story.mapItemQuest(807, "cloister", 140);

        //Acorny Quest
        Story.killQuest(808, "cloister", "Acornent");

        //Ravenloss
        Story.killQuest(809, "cloister", "Karasu");

        //It's A Bough-t Time
        if (!Story.questProgression(810)) {
            Core.ensureAccept(810);

            bot.shops.sellItem(43280);
            bot.wait.forItemSell();

            Adv.buyItem("embersea", 1100, 5572, undefined, 17967);
            Core.getMapItem(141, 3, "cloister");
            Core.ensureComplete(810);
        }

        //Wendigo Whereabouts
        Story.killQuest(811, "cloister", "Wendigo");

        //Find Paddy Lump
        Story.mapItemQuest(813, "mudluk", 143);

        //Toothy Smiles
        Story.killQuest(814, "mudluk", "Swamp Lurker");

        //Slimy Cyrus
        Story.killQuest(815, "mudluk", "Swamp Lurker");

        //Lord Of The Fleas
        Story.killQuest(816, "arcangrove", "Gorillaphant");

        //Not The Best Idea
        Story.killQuest(817, "mudluk", "Swamp Frogdrake");

        //Gates and Guardians
        Story.killQuest(818, "mudluk", "Tiger Leech");

        //Water You Waiting For--Find Nisse
        Story.mapItemQuest(825, "natatorium", 144);

        //Dive Right In
        Story.mapItemQuest(826, "natatorium", 145, 12);

        //Seafood Diet
        Story.killQuest(827, "natatorium", "Anglerfish");

        //Mercenaries
        Story.killQuest(828, "natatorium", "Merdraconian");

        //Synchronized Slaying
        if (!Story.questProgression(829)) {
            Core.ensureAccept(829);
            Core.huntMonster("arcangrove", "Seed Spitter", "Brain Coral", 5);
            Core.huntMonster("cloister", "Acornent", "Staghorns", 4);
            Core.huntMonster("cloister", "Karasu", "Sea Fan", 3);
            Core.huntMonster("cloister", "Wendigo", "Sea Whip");
            Core.huntMonster("mudluk", "Swamp Frogdrake", "Anemones", 6);
            Core.ensureComplete(829);
        }

        //The Deep End
        Story.killQuest(830, "natatorium", "Nessie");

        //Find Umbra, the Master Shaman
        Story.mapItemQuest(831, "gilead", 146);

        //The Root of Elementals
        if (!Story.questProgression(832)) {
            Core.ensureAccept(832);
            Core.huntMonster("gilead", "Earth Elemental", "Dregs Essence", 5);
            Core.huntMonster("arcangrove", "Seed Spitter", "Spitter Spirit", 4);
            Core.ensureComplete(832);
        }

        //Eupotamic Elementals
        if (!Story.questProgression(833)) {
            Core.ensureAccept(833);
            Core.huntMonster("gilead", "Water Elemental", "Aqueous Essence", 5);
            Core.huntMonster("natatorium", "Merdraconian", "MerCore", 6);
            Core.ensureComplete(833);
        }

        //Breaking Wind Elementals
        if (!Story.questProgression(834)) {
            Core.ensureAccept(834);
            Core.huntMonster("gilead", "Wind Elemental", "Welkin Essence", 5);
            Core.huntMonster("cloister", "Karasu", "Karasu Soul", 8);
            Core.ensureComplete(834);
        }

        //Fight Fire With Fire Salamanders
        if (!Story.questProgression(835)) {
            Core.ensureAccept(835);
            Core.huntMonster("gilead", "Fire Elemental", "Pyre Essence", 5);
            Core.huntMonster("mudluk", "Swamp Frogdrake", "Fire Salamander", 5);
            Core.ensureComplete(835);
        }

        //Guardian of the Gilead Wrap
        Story.killQuest(836, "gilead", "Mana Elemental");

        //Find Felsic the Magma Golem
        Story.mapItemQuest(838, "mafic", 147);

        //Liquid Hot Magma Blood Maggots
        Story.killQuest(839, "mafic", "Volcanic Maggot");

        //Scorched Serpents
        Story.killQuest(840, "mafic", "Scoria Serpent");

        //Playing With Living Fire
        Story.killQuest(841, "mafic", "Living Fire");

        //Kindling Relationship
        Story.killQuest(842, "mafic", "Mafic Dragon");

        //Obey Your Thirst for Adventure
        Story.killQuest(843, "elemental", "Mana Imp");

        //Captain Falcons
        Story.killQuest(844, "elemental", "Mana Falcon");

        //Big, bad, and Baddest Bosses
        if (!Story.questProgression(845)) {
            Core.ensureAccept(845);
            Core.huntMonster("mafic", "Mafic Dragon", "Astral Orb of Mafic");
            Core.huntMonster("cloister", "Wendigo", "Astral Orb of the Cloister");
            Core.huntMonster("mudluk", "Tiger Leech", "Astral Orb of Mudluk");
            Core.huntMonster("natatorium", "Nessie", "Astral Orb of Natatorium");
            Core.huntMonster("gilead", "Mana Elemental", "Astral Orb of Gilead");
            Core.ensureComplete(845);
        }

        //The Great Mana Golem
        if (!Story.questProgression(846)) {
            Core.ensureAccept(846);
            bot.quests.updateQuest(847);
            Core.huntMonster("elemental", "Mana Golem", "Mana Golem Defeated");
            Core.ensureComplete(846);
        }

        //Chaos Lord Ledgermayne
        this.killLedgermayne();
    }

    private killLedgermayne(): void {
        if (Story.questProgression(847) || !Core.ensureAccept(847))
            return;

        bot.events.off("extensionPacket", this.ledgermayneZoneListener);
        bot.events.on("extensionPacket", this.ledgermayneZoneListener);

        try {
            Core.huntMonster("ledgermayne", "Ledgermayne", "Ledgermayne Defeated");
        } finally {
            bot.events.off("extensionPacket", this.ledgermayneZoneListener);
        }

        Core.ensureComplete(847);
    }

    // C#: an `async void` handler; arrow function so on/off get the same listener.
    private readonly ledgermayneZoneListener = (packet: any): void => {
        if (bot.shouldExit
            || bot.player?.alive !== true
            || bot.map?.name?.toLowerCase() !== "ledgermayne"
            || packet?.["params"]?.type?.toString() !== "json")
            return;

        const data = packet["params"].dataObj;
        if (data?.cmd?.toString() !== "event")
            return;

        const zoneSet = data?.args?.zoneSet?.toString();
        if (zoneSet == null)
            return;

        let x: number;
        let y: number;

        if (zoneSet.toLowerCase() === "a")
            [x, y] = [292, 355];
        else if (zoneSet.toLowerCase() === "b")
            [x, y] = [750, 350];
        else if (zoneSet.length === 0)
            [x, y] = [592, 250];
        else
            return;

        bot.player.walkTo(x, y, 9);
    };

    tibicenas(): void {
        if (Core.isCompletedBefore(1005))
            return;

        Story.preLoad(this.tibicenas);

        //Sandport and Starboard
        Story.mapItemQuest(930, "sandport", 251);

        //Shark Diving
        Story.killQuest(931, "sandport", "Sandshark");

        //Thieving Cut Throats
        Story.killQuest(932, "sandport", "Tomb Robber");

        //Lost and Found
        Story.killQuest(933, "sandport", "Tomb Robber");

        //Sell-Sword Sell-Outs
        if (!Story.questProgression(934)) {
            Core.ensureAccept(934);
            Core.killMonster("sandport", "r6", "Left", "Sell-Sword Leader", "Sell-Sword Leader Defeated");
            Core.killMonster("sandport", "r5", "Left", "Horc Sell-Sword", "Horc Sell-Swords Defeated", 3);
            Core.ensureComplete(934);
        }

        bot.quests.updateQuest(4614);

        //Sacred Scarabs
        Story.killQuest(967, "pyramid", "Golden Scarab");

        //A Noob is Guard
        Story.killQuest(968, "pyramid", "Anubis Deathguard");

        //Bandaged Aids
        Story.killQuest(969, "pyramid", "Mummy");

        //Keys to the Royal Chamber
        Story.killQuest(970, "pyramid", "Golden Scarab");

        //Confront Duat
        Story.mapItemQuest(971, "pyramid", 304);

        //They've Gone Dark
        if (!Story.questProgression(972)) {
            Core.ensureAccept(972);
            Core.killMonster("wanders", "r2", "Right", "Kalestri Worshiper", "Dark Medallion", 8);
            Core.ensureComplete(972);
        }

        //Bad Doggies
        if (!Story.questProgression(973)) {
            Core.ensureAccept(973);
            Core.killMonster("wanders", "r3", "Right", "Kalestri Hound", "Hound Defeated", 10);
            Core.ensureComplete(973);
        }

        //Essentially Evil
        if (!Story.questProgression(974)) {
            Core.ensureAccept(974);
            Core.killMonster("wanders", "r3", "Right", "Kalestri Hound", "Evil Essence", 10);
            Core.ensureComplete(974);
        }

        //Loose Threads
        if (!Story.questProgression(975)) {
            Core.ensureAccept(975);
            Core.killMonster("wanders", "r5", "Right", "Lotus Spider", "Spider Defeated", 10);
            Core.ensureComplete(975);
        }

        //Seek The Treasure
        if (!Story.questProgression(976)) {
            Core.ensureAccept(976);
            Core.getMapItem(306, 1, "wanders");
            Core.ensureComplete(976);
        }

        //Dreamsand
        if (!Story.questProgression(977)) {
            Core.ensureAccept(977);
            Core.killMonster("wanders", "r5", "Right", "Lotus Spider", "Dreamsand");
            Core.ensureComplete(977);
        }

        //I Dream Of...
        if (!Story.questProgression(978)) {
            Core.ensureAccept(978);
            Core.killMonster("wanders", "Boss", "Left", "Sek-Duat", "Sek-Duat Defeated");
            Core.ensureComplete(978);
            //Editors Note: PLEASE stop breaking this
            //Editors Note2: for each new Sek-Duat Fix Please add 1 to this counter, Counter started 5/7/22 (not entirely acurate just when i made the counter.)
            // Counter: 3
        }

        //Sandsational Castle
        Story.mapItemQuest(995, "sandcastle", 361);

        //Furry Fury
        Story.killQuest(996, "sandcastle", "War Hyena");

        //Keeping Secrets Under Wraps
        Story.killQuest(997, "sandcastle", "War Mummy");

        //Gem Jam
        Story.killQuest(998, "sandcastle", "War Hyena");

        //Enter the Sphinx
        Core.equipClass(ClassType.Solo);
        Story.killQuest(999, "sandcastle", "Chaos Sphinx");

        //Unlamented Lamia
        Core.equipClass(ClassType.Farm);
        Story.killQuest(1000, "djinn", "Lamia");

        //E-vase-ive Measures
        Story.killQuest(1001, "sandsea", "Desert Vase");

        //Tri-hump-hant Camels
        Story.killQuest(1002, "sandsea", "Bupers Camel");

        //I Don't Mean to Harp On It...
        Story.killQuest(1003, "djinn", "Harpy");

        //In-djinn-ious Solution
        Story.mapItemQuest(1004, "djinn", 370, 5);

        //Chaos Lord Tibicenas
        Core.equipClass(ClassType.Solo);
        Story.killQuest(1005, "djinn", "Tibicenas");
    }

    khasaandaHorc(): void {
        if (Core.isCompletedBefore(1473))
            return;

        Story.preLoad(this.khasaandaHorc);

        //Troll Stink!
        if (!Story.questProgression(1232)) {
            Core.equipClass(ClassType.Farm);
            Core.ensureAccept(1232);
            Core.huntMonster("crossroads", "Chinchilizard", "Scaly Skin Scrub", 7);
            Core.huntMonster("bloodtusk", "Trollola Plant", "Perfumed Trollola Flower", 5);
            Story.mapItemQuest(1232, "bloodtusk", 523);
        }

        // It Not Time Yet
        if (!Story.questProgression(1233)) {
            Core.equipClass(ClassType.Farm);
            Core.ensureAccept(1233);
            Core.huntMonster("crossroads", "Lemurphant", "Lemurphant Stones", 5);
            Core.huntMonster("crossroads", "Koalion", "Golden Down-fur", 5);
            Core.ensureComplete(1233);
        }

        //Mountain Protection
        if (!Story.questProgression(1234)) {
            Core.equipClass(ClassType.Farm);
            Core.ensureAccept(1234);
            Core.killMonster("bloodtusk", "r8", "Left", "Crystal-Rock", "Polished Rocks", 3);
            Core.huntMonster("crossroads", "Lemurphant", "Lemurphant Ivory", 5);
            Core.huntMonster("crossroads", "Chinchilizard", "Liz-Leather Thongs", 5);
            Story.mapItemQuest(1234, "crossroads", 525);
        }

        //Clear Mind, Cleanse Spirit
        if (!Story.questProgression(1235)) {
            Core.equipClass(ClassType.Farm);
            Core.ensureAccept(1235);
            Core.huntMonster("bloodtusk", "Trollola Plant", "Trollola Plant Resin", 4);
            Core.huntMonster("crossroads", "Lemurphant", "Lemurphant Musk", 5);
            Core.huntMonster("crossroads", "Koalion", "Fur for Firestarting", 5);
            Story.mapItemQuest(1235, "crossroads", 521, 10);
        }

        //She Who Asks
        Story.chainQuest(1236);

        //Be Horc Inside
        if (!Story.questProgression(1237)) {
            Core.equipClass(ClassType.Farm);
            Core.ensureAccept(1237);
            Core.huntMonster("bloodtusk", "Rock", "Skin of the Mountain", 3);
            Core.huntMonster("crossroads", "Koalion", "Koalion Claw", 5);
            Core.huntMonster("crossroads", "Lemurphant", "Lemurphant Tusks", 5);
            Story.mapItemQuest(1237, "crossroads", 524, 5);
            Story.mapItemQuest(1237, "crossroads", 522, 5);
            Core.ensureComplete(1237);
        }

        //She Who Answers 2 - cutscene
        Story.chainQuest(1241);

        //Chaos Enrages the Horcs
        Story.chainQuest(1273);

        //Into, Under the Mountain
        Story.mapItemQuest(1280, "ravinetemple", 553);

        //Has the Land Been Tainted?
        Story.mapItemQuest(1281, "ravinetemple", 554, 5);
        Story.mapItemQuest(1281, "ravinetemple", [ 555, 556 ], 10);

        //Tears of the Mountain
        Story.killQuest(1282, "ravinetemple", "Temple Guardian");

        //Defend the UnderMountain
        if (!Story.questProgression(1283)) {
            Core.ensureAccept(1283);
            Core.getMapItem(557, 10, "ravinetemple");
            Core.huntMonster("ravinetemple", "Temple Guardian", "Sharpened Bone Stake", 5);
            Core.ensureComplete(1283);
        }

        //Alliance Defiance
        Story.killQuest(1284, "ravinetemple", "Temple Guardian");

        //Scout and Return
        Story.mapItemQuest(1375, "alliance", [ 679, 680 ]);

        //Good and Evil Not Always Right
        if (!Story.questProgression(1376)) {
            Core.equipClass(ClassType.Farm);
            Core.ensureAccept(1376);
            Core.huntMonster("alliance", "Good Soldier", "Good Soldier Vanquished", 10);
            Core.huntMonster("alliance", "Evil Soldier", "Evil Soldier Vanquished", 10);
            Core.ensureComplete(1376);
        }

        //Trapping Savage Soldiers
        Story.mapItemQuest(1377, "alliance", 675, 10);

        //Find What is Hidden Inside
        Story.mapItemQuest(1378, "alliance", 676);

        //Chaorruption Rejection
        Story.killQuest(1379, "alliance", "Chaorrupted Evil Soldier");

        //Alliance Subdued
        if (!Story.questProgression(1380)) {
            Core.ensureAccept(1380);
            Core.huntMonster("alliance", "General Cynari", "Cynari Defeated!");
            Core.huntMonster("alliance", "General Tibias", "Tibias Defeated!");
            Core.ensureComplete(1380);
        }

        //Cleanse the Chaorruption
        Story.killQuest(1424, "ancienttemple", "Chaotic Vulture");

        //Chaorruption Cure?
        if (!Story.questProgression(1425)) {
            Core.ensureAccept(1425);
            Core.getMapItem(706, 7, "ancienttemple");
            Core.huntMonster("ancienttemple", "Chaotic Vulture", "Stolen Urn", 7);
            Core.ensureComplete(1425);
        }

        //Guardian Salvation
        Story.killQuest(1426, "ancienttemple", "Chaos Troll Spirit");

        //Poison for a Purpose
        Story.killQuest(1427, "ancienttemple", "Serpentress");

        //The Heart of the Temple Awaits
        Story.mapItemQuest(1428, "ancienttemple", 707);

        //Wounds in Stones and Beasts
        Story.mapItemQuest(1456, "orecavern", 717);

        //Light in Underhome
        if (!Story.questProgression(1457)) {
            Core.ensureAccept(1457);
            Core.getMapItem(719, 5, "orecavern");
            Core.huntMonster("orecavern", "Crashroom", "Light Orb Recovered", 10);
            Core.ensureComplete(1457);
        }

        //Truth is its Own Light
        Story.mapItemQuest(1458, "orecavern", 718, 5);

        //Horcs Know Mercy
        Story.killQuest(1459, "orecavern", "Chaorrupted Evil Soldier");

        //Battle the Baas!
        Core.equipClass(ClassType.Solo);
        Story.killQuest(1460, "orecavern", "Naga Baas");

        //Know the Nexus
        Story.mapItemQuest(1469, "dreamnexus", [ 734, 735, 736, 737 ]);

        //Secure a Route Home
        if (!Story.questProgression(1470)) {
            Core.equipClass(ClassType.Farm);
            Core.ensureAccept(1470);
            Core.huntMonster("dreamnexus", "Dark Wyvern", "Wyvern Scales", 2);
            Core.huntMonster("dreamnexus", "Dark Wyvern", "Wyvern Claws", 2);
            Core.huntMonster("dreamnexus", "Aether Serpent", "Serpent Fangs", 2);
            Core.huntMonster("dreamnexus", "Aether Serpent", "Serpent Hair", 2);
            Core.ensureComplete(1470);
        }

        //DreamDancers' Orbs
        Story.mapItemQuest(1471, "dreamnexus", 738, 10);
        Story.mapItemQuest(1471, "dreamnexus", 739, 11);

        //Master the Flames
        if (!Story.questProgression(1472)) {
            Core.equipClass(ClassType.Farm);
            Core.ensureAccept(1472);
            Core.huntMonster("dreamnexus", "Solar Phoenix", "Phoenix Tear", 10);
            Core.huntMonster("dreamnexus", "Solar Phoenix", "Phoenix Blood", 5);
            Core.ensureComplete(1472);
        }

        //Choose: Khasaanda Confrontation?
        if (!Story.questProgression(1473)) {
            Core.equipClass(ClassType.Solo);
            Core.ensureAccept(1473);
            Core.killMonster("dreamnexus", "r17a", "Up", "Khasaanda", "Khasaanda Defeated!", undefined, { publicRoom: false });
            Core.ensureComplete(1473);
            bot.wait.forQuestComplete(1473);
        }
    }

    khasaandaTroll(): void {
        if (Core.isCompletedBefore(1468))
            return;

        Story.preLoad(this.khasaandaTroll);

        //Horc Stink!
        if (!Story.questProgression(1226)) {
            Core.ensureAccept(1226);
            Core.huntMonster("crossroads", "Chinchilizard", "Scaly Skin Scrub", 7);
            Core.huntMonster("bloodtusk", "Trollola Plant", "Perfumed Trollola Flower", 5);
            Story.mapItemQuest(1226, "bloodtusk", 523);
        }

        //The Time Grows Closer
        if (!Story.questProgression(1227)) {
            Core.ensureAccept(1227);
            Core.huntMonster("crossroads", "Lemurphant", "Lemurphant Stones", 5);
            Core.huntMonster("crossroads", "Koalion", "Golden Down-Fur", 5);
            Core.ensureComplete(1227);
        }

        //Like Calls to Like
        if (!Story.questProgression(1228)) {
            Core.ensureAccept(1228);
            Core.huntMonster("bloodtusk", "Crystal-Rock", "Mountain Crystal", 3);
            Core.huntMonster("crossroads", "Chinchilizard", "Liz-Leather Thongs", 5);
            Core.huntMonster("crossroads", "Lemurphant", "Lemurphant Ivory", 5);
            Story.mapItemQuest(1228, "crossroads", 525);
        }

        //Incense Makes Sense
        if (!Story.questProgression(1229)) {
            Core.ensureAccept(1229);
            Core.huntMonster("crossroads", "Lemurphant", "Lemurphant Musk", 5);
            Core.huntMonster("bloodtusk", "Trollola Plant", "Trollola Plant Resin", 4);
            Core.huntMonster("crossroads", "Koalion", "Fur for Firestarting", 5);
            Story.mapItemQuest(1229, "crossroads", 521, 10);
        }

        //She Who asks 1
        if (!Story.questProgression(1230))
            Story.chainQuest(1230);

        //The Troll Inside
        if (!Story.questProgression(1231)) {
            Core.ensureAccept(1231);
            Core.huntMonster("crossroads", "Lemurphant", "Lemurphant Tusks", 5);
            Core.huntMonster("crossroads", "Koalion", "Koalion Claw", 5);
            Core.huntMonster("bloodtusk", "Crystal-Rock", "Singing Crystals", 3);
            Story.mapItemQuest(1231, "crossroads", 522, 5);
            Story.mapItemQuest(1231, "crossroads", 524, 10);
        }

        //Eclipse - cutscene
        if (!Story.questProgression(1240)) {
            Core.ensureAccept(1240);
            Core.join("crossroads");
            Core.sleep(2000);
            Core.join("crossroads");
            Core.jump("CutE", "Left");
            Core.sleep(2000);
            Core.ensureComplete(1240);
        }

        //Chaos scars the Trolls
        Story.chainQuest(1272);

        //Guarded Secrets, Hidden Treasures
        Story.mapItemQuest(1274, "ravinetemple", 553);

        //Evidence of Chaos
        Story.mapItemQuest(1275, "ravinetemple", 554, 5);
        Story.mapItemQuest(1275, "ravinetemple", [ 555, 556 ], 10);

        //Learn More of the Ore
        Story.killQuest(1276, "ravinetemple", "Temple Guardian");

        //Too Little, Too Late. Still Needed
        if (!Story.questProgression(1277)) {
            Core.ensureAccept(1277);
            Core.getMapItem(557, 10, "ravinetemple");
            Core.huntMonster("ravinetemple", "Temple Guardian", "Sharpened Bone Stake", 5);
            Core.ensureComplete(1277);
        }

        //Alliance Defiance
        Story.killQuest(1278, "ravinetemple", "Temple Guardian");

        //The Headquartes of Good and Evil
        Story.mapItemQuest(1369, "alliance", [ 679, 680 ]);

        //Treat Nullification, Good and Bad
        if (!Story.questProgression(1370)) {
            Core.ensureAccept(1370);
            Core.huntMonster("alliance", "Good Soldier", "Good Soldier Vanquished", 6);
            Core.huntMonster("alliance", "Evil Soldier", "Evil Soldier Vanquished", 6);
            Core.ensureComplete(1370);
        }

        //Trap the Keepers
        Story.mapItemQuest(1371, "alliance", 675, 10);

        //Find What is Hidden Inside
        Story.mapItemQuest(1372, "alliance", 676);

        //Chaorruption Annihilation
        Story.killQuest(1373, "alliance", "Chaorrupted Evil Soldier");

        //Alliance Demotion
        if (!Story.questProgression(1374)) {
            Core.ensureAccept(1374);
            Core.huntMonster("alliance", "General Cynari", "Cynari Defeated!", 1);
            Core.huntMonster("alliance", "General Tibias", "Tibias Defeated!", 1);
            Core.ensureComplete(1374);
        }

        //Contain the Chaorruption
        Story.killQuest(1419, "ancienttemple", "Chaotic Vulture");

        //Ancient Ointment
        if (!Story.questProgression(1420)) {
            Core.ensureAccept(1420);
            Core.getMapItem(706, 7, "ancienttemple");
            Core.huntMonster("ancienttemple", "Chaotic Vulture", "Stolen Urn", 7);
            Core.ensureComplete(1420);
        }

        //Anoint the Ancients
        Story.killQuest(1421, "ancienttemple", "Chaos Troll Spirit");

        //Serpents Do No Harm
        Story.killQuest(1422, "ancienttemple", "Serpentress");

        //Though Nature Bars the Way
        Story.mapItemQuest(1423, "ancienttemple", 707);

        //Descent Into Darkness
        Story.mapItemQuest(1451, "orecavern", 717);

        //Out of the Darkness
        if (!Story.questProgression(1452)) {
            Core.ensureAccept(1452);
            Core.getMapItem(719, 5, "orecavern");
            Core.huntMonster("orecavern", "Crashroom", "Light Orb Recovered", 10);
            Core.ensureComplete(1452);
        }

        //Shine a Light on Deception
        Story.mapItemQuest(1453, "orecavern", 718, 5);

        //Save Yourself, Save the Soldiers
        Story.killQuest(1454, "orecavern", "Chaorrupted Evil Soldier");

        //Battle the Baas!
        Story.killQuest(1455, "orecavern", "Naga Baas");

        //Know the Nexus
        Story.mapItemQuest(1464, "dreamnexus", [ 734, 735, 736, 737 ]);

        //Secure a Route Home
        if (!Story.questProgression(1465)) {
            Core.ensureAccept(1465);
            Core.huntMonster("dreamnexus", "Dark Wyvern", "Wyvern Scales", 2);
            Core.huntMonster("dreamnexus", "Dark Wyvern", "Wyvern Claws", 2);
            Core.huntMonster("dreamnexus", "Aether Serpent", "Serpent Fangs", 2);
            Core.huntMonster("dreamnexus", "Aether Serpent", "Serpent Hair", 2);
            Core.ensureComplete(1465);
        }

        //DreamDancers' Orbs
        Story.mapItemQuest(1466, "dreamnexus", 738, 10);
        Story.mapItemQuest(1466, "dreamnexus", 739, 11);

        //Master the Flames
        if (!Story.questProgression(1467)) {
            Core.ensureAccept(1467);
            Core.huntMonster("dreamnexus", "Solar Phoenix", "Phoenix Tear", 10);
            Core.huntMonster("dreamnexus", "Solar Phoenix", "Phoenix Blood", 5);
            Core.ensureComplete(1467);
        }

        //Choose: Khasaanda Confrontation?
        Story.killQuest(1468, "dreamnexus", "Khasaanda");
    }

    iadoa(): void {
        if (Core.isCompletedBefore(2519))
            return;

        Story.preLoad(this.iadoa);

        //Time to Learn the Truth
        Story.mapItemQuest(2239, "thespan", [ 1358, 1359, 1360, 1361, 1362, 1363 ]);

        //Gain Access to Doors
        if (!Story.questProgression(2240)) {
            Core.ensureAccept(2240);
            Core.huntMonster("timelibrary", "Sneak", "AQ Dimension Key", 1);
            Core.huntMonster("timelibrary", "Tog", "DF Dimension Key", 1);
            Core.huntMonster("timelibrary", "Shadowscythe", "MQ Dimension Key", 1);
            Core.ensureComplete(2240);
        }

        //Adventures and Quests
        if (!Story.questProgression(2241)) {
            Core.ensureAccept(2241);
            Core.huntMonster("timelibrary", "Moglin Ghost", "Necromancy in AQ Knowledge");
            Core.huntMonster("timelibrary", "Undead Knight", "Tales of Planet-Peril Page", 6);
            Story.mapItemQuest(2241, "timelibrary", 1365, 3);
        }

        //A Fable of Dragons
        if (!Story.questProgression(2242)) {
            Core.ensureAccept(2242);
            Core.huntMonster("timelibrary", "Tog", "Elemental Orb Knowledge", 8);
            Core.huntMonster("timelibrary", "Tog", "\"Draconic Prophecy\" Page", 6);
            Story.mapItemQuest(2242, "timelibrary", 1366, 2);
        }

        //Mechas and Quests
        if (!Story.questProgression(2243)) {
            Core.ensureAccept(2243);
            Core.huntMonster("timelibrary", "Shadowscythe", "Shadowscythe Combat Strategy", 7);
            Core.huntMonster("timelibrary", "Training Globe", "Galactic Hypertron Engines: A Primer");
            Story.mapItemQuest(2243, "timelibrary", 1367);
        }

        //After the Chaos
        if (!Story.questProgression(2244)) {
            Core.ensureAccept(2244);
            Core.huntMonster("timelibrary", "Queen's Recruit", "The Future is Now", 3);
            Core.huntMonster("timelibrary", "Queen's Recruit", "Past Failures Brought Us Here", 2);
            Core.huntMonster("timelibrary", "Queen's Knight", "Princess Freed... but for what?", 2);
            Story.mapItemQuest(2244, "timelibrary", 1368);
        }

        //Trust is Not Ephemeral
        Story.killQuest(2253, "timevoid", "Ephemerite");

        //In a Split Exasecond
        if (!Story.questProgression(2254)) {
            Core.ensureAccept(2254);
            Core.huntMonster("timevoid", "Time-Travel Fairy", "Exaglass", 4);
            Core.huntMonster("timevoid", "Time-Travel Fairy", "Fairy Plasma", 8);
            Story.mapItemQuest(2254, "timevoid", 1438, 16);
        }

        //Time to Prove Yourself
        if (!Story.questProgression(2255)) {
            Core.ensureAccept(2255);
            Core.getMapItem(1439, 12, "timevoid");
            Core.huntMonster("timevoid", "Ephemerite", "Ephemeral Dust", 4);
            Core.huntMonster("timevoid", "Time-Travel Fairy", "Fairy Plasma", 4);
            Core.ensureComplete(2255);
        }

        //Fill the Empty Hours
        if (!Story.questProgression(2256)) {
            Core.ensureAccept(2256);
            Core.huntMonster("timevoid", "Time-Travel Fairy", "Powdered Fairy Wings", 6);
            Core.huntMonster("timevoid", "Void Phoenix", "Dried Phoenix Tears", 6);
            Core.ensureComplete(2256);
        }

        //Clock of the Long Now
        Story.mapItemQuest(2257, "timevoid", [ 1440, 1441, 1442, 1443 ]);

        //Unending Avatar
        Story.killQuest(2258, "timevoid", "Unending Avatar", { autoCompleteQuest: false });

        //Construct Your Reality
        Story.mapItemQuest(2376, "aqlesson", 1467);

        //Reach the Temple
        Story.killQuest(2377, "aqlesson", "Ninja");

        //Not All Hope is Lost
        Story.mapItemQuest(2378, "aqlesson", 1468, 8);
        Story.mapItemQuest(2378, "aqlesson", 1469);

        //Bolster the Elements
        if (!Story.questProgression(2379)) {
            Core.ensureAccept(2379);
            Core.getMapItems([ [1470, 3], [1471, 3] ], "aqlesson");
            Core.huntMonster("aqlesson", "Eternite Ore", "TimeSpark Acquired", 3);
            Core.huntMonster("aqlesson", "Water Elemental", "Tears of Joy Acquired", 3);
            Core.ensureComplete(2379);
        }

        //Maintain Elemental Strength
        if (!Story.questProgression(2380)) {
            Core.ensureAccept(2380);
            Core.getMapItems([ [1473, 3], [1472, 3] ], "aqlesson");
            Core.huntMonster("aqlesson", "Ice Elemental", "Frigid Liquid Acquired", 3);
            Core.huntMonster("aqlesson", "Fire Elemental", "Heart of Lava Acquired", 3);
            Core.ensureComplete(2380);
        }

        //Rescue the Innocent
        Story.killQuest(2381, "aqlesson", "Void Dragon");

        //Get Fired Up... or Shatter!
        Story.killQuest(2382, "aqlesson", "Firezard");

        //Enemies on Ice
        Story.killQuest(2383, "aqlesson", "Ice Elemental");

        //Tek-nical Forging Skill
        Story.mapItemQuest(2384, "thespan", 1474);

        //Akriloth Assault
        Story.killQuest(2385, "aqlesson", "Akriloth");

        //Proto-Chaos Beast Battle!
        Story.killQuest(2386, "aqlesson", "Carnax");

        //Elemental Orb Awareness
        Story.mapItemQuest(2470, "dflesson", 1549, 8);

        //Fight Chaos with Fire!
        if (!Story.questProgression(2471)) {
            Core.ensureAccept(2471);
            Core.huntMonster("dflesson", "Fire Elemental", "Slain Flame Elemental", 8);
            Core.huntMonster("dflesson", "Fire Elemental", "Chaos Gemerald", 4);
            Core.ensureComplete(2471);
        }

        //Save Aria
        if (!Story.questProgression(2472)) {
            Core.ensureAccept(2472);
            Core.huntMonster("dflesson", "Lava Slime", "Slain Slime", 5);
            Core.huntMonster("dflesson", "Fire Elemental", "Slain Elemental", 6);
            Core.huntMonster("dflesson", "Fire Elemental", "Blue Clue");
            Core.ensureComplete(2472);
        }

        //Find the Time to Travel
        if (!Story.questProgression(2473)) {
            Core.ensureAccept(2473);
            Core.huntMonster("dflesson", "Tog", "Life AND the Universe", 4);
            Core.huntMonster("dflesson", "Agitated Orb", "Everything", 5);
            Core.huntMonster("dflesson", "Agitated Orb", "Hoopy Frood's Towel", 3);
            Core.ensureComplete(2473);
        }

        //Dragon Egg... or Junk?
        Story.killQuest(2474, "dflesson", "Vultragon");

        //Dracolich Fortress Detected
        Story.killQuest(2475, "dflesson", "Chaos Sp-Eye");

        //Bone up on the Boss
        Story.killQuest(2476, "dflesson", "Chaorrupted Evil Soldier");

        //Defend the Town!
        if (!Story.questProgression(2477)) {
            Core.ensureAccept(2477);
            Core.huntMonster("dflesson", "Lava Golem", "Golem Crushed", 6);
            Core.huntMonster("dflesson", "Fire Elemental", "Elemental Eliminated", 3);
            Core.ensureComplete(2477);
        }

        //ChickenCows, Bacon, and Battle!
        if (!Story.questProgression(2478)) {
            Core.ensureAccept(2478);
            Core.huntMonster("dflesson", "Chaotic Horcboar", "Hero Protein", 1);
            Core.huntMonster("dflesson", "Chaotic Chicken", "Spell Rune", 5);
            Core.ensureComplete(2478);
        }

        //The 2nd Proto-Chaos Beast
        Story.killQuest(2479, "dflesson", "Fluffy the Dracolich");

        //Board the Ship to Your Future
        Story.mapItemQuest(2504, "mqlesson", 1580, undefined, { getReward: false });

        //Heal the Chaos Lord
        Story.killQuest(2505, "mqlesson", "Asteroid");

        //Shadowscythe Detection Beacons
        Story.mapItemQuest(2506, "mqlesson", 1581, 5);

        //Test Potential Traitors
        Story.killQuest(2507, "mqlesson", "MystRaven Student");

        //Defeat Training Globes
        Story.killQuest(2508, "mqlesson", "Training Globe");

        //Take Flight into the Future
        Story.killQuest(2509, "mqlesson", "MystRaven Student");

        //Secrets of the Universe
        Story.killQuest(2510, "mqlesson", "Chaos Shadowscythe");

        //Mysterious!
        if (!Story.questProgression(2511)) {
            Core.ensureAccept(2511);
            Core.huntMonster("mqlesson", "Chaos Shadowscythe", "Truth Glasses");
            Core.huntMonster("mqlesson", "Chaos Shadowscythe", "Darkness Destroyed", 13);
            Core.ensureComplete(2511);
        }

        //The 3rd Proto-Chaos Beast
        Story.killQuest(2512, "mqlesson", "Dragonoid");

        //Chaos Waits and Watches
        Story.killQuest(2513, "deepchaos", "Chaotic Merdrac");

        //The Lure of Chaosanity
        Story.killQuest(2515, "deepchaos", "Chaos Angler");

        //Music of Nightmares
        Story.mapItemQuest(2516, "deepchaos", 1582, 3);

        //Chaos Beast Kathool
        Story.killQuest(2517, "deepchaos", "Kathool");

        Core.sleep(2500);

        //Starry, Starry Night
        if (!Story.questProgression(2518)) {
            Core.ensureAccept(2518);
            Core.huntMonster("timespace", "Astral Ephemerite", "Star Death", 6);
            Core.ensureComplete(2518);
        }

        //Chaos Lord Iadoa
        if (!Story.questProgression(2519)) {
            Core.ensureAccept(2519);
            Core.equipClass(ClassType.Solo);
            if (!Core.checkInventory("Dragonoid of Hours")) {
                Core.logger(
                    "Getting \"Dragonoid of Hours\" from \"Dragonoid\" in \"mqlesson\" to make Iadoa easier.");
                Core.huntMonster("mqlesson", "Dragonoid", "Dragonoid of Hours", undefined, { isTemp: false });
            }
            Core.huntMonster("timespace", "Chaos Lord Iadoa", "Iadoa Defeated");
            Core.ensureComplete(2519);
        }

        if (Core.isMember) {
            //Defeat Ultra Kathool 2520
            Story.killQuest(2520, "ultravoid", "Ultra Kathool");

            //Defeat Ultra Iadoa 2521 (Badge)
            Story.killQuest(2521, "ultravoid", "Ultra Iadoa");

            // Ultra Carnax Challenge Fight! 2388  (Not in Story Slo but Badge relevant to thi)
            Story.killQuest(2388, "ultracarnax", "Ultra-Carnax");
        }
    }

    lionfang(): void {
        if (Core.isCompletedBefore(2814))
            return;

        Story.preLoad(this.lionfang);

        //Final Rest
        Story.killQuest(2612, "blackhorn", "Restless Undead");

        //Disturbing The Peace
        Story.mapItemQuest(2613, "blackhorn", 1615, 10);

        //Sampling Silk
        Story.killQuest(2614, "blackhorn", "Tomb Spider");

        //Fire Is The Thing
        if (!Story.questProgression(2615)) {
            Core.ensureAccept(2615);
            Core.getMapItem(1616, 1, "blackhorn");
            Core.huntMonster("blackhorn", "Restless Undead", "Scrap Of Cloth", 3);
            Core.huntMonster("blackhorn", "Tomb Spider", "Gas Bladder", 3);
            Core.ensureComplete(2615);
        }

        //The Wall Comes Down
        Story.mapItemQuest(2616, "blackhorn", 1617);

        //The Bonefeeder
        Story.killQuest(2617, "blackhorn", "Bonefeeder Spider");

        //What Lies Beyond?
        Story.mapItemQuest(2618, "blackhorn", 1618);

        //Toxic
        Story.killQuest(2619, "blackhorn", "Tomb Spider");

        //Very Toxic
        Story.killQuest(2620, "blackhorn", "Restless Undead");

        //Really, VERY VERY TOXIC!
        Story.mapItemQuest(2621, "blackhorn", 1619);

        //Lion Hunting
        Story.mapItemQuest(2622, "onslaughttower", [ 1620, 1621, 1622, 1623 ]);

        //Secret Of The Death Fog
        Story.killQuest(2623, "onslaughttower", "Golden Caster");

        //The Key To Survival
        Story.killQuest(2624, "onslaughttower", "Golden Caster");

        //The Tools
        Story.mapItemQuest(2625, "onslaughttower", 1624, 8);

        //The Talent
        Story.mapItemQuest(2626, "onslaughttower", 1625);

        //The Local Locale
        Story.mapItemQuest(2627, "onslaughttower", 1626, 4);

        //Who Holds The Key?
        Story.killQuest(2628, "onslaughttower", "Golden Caster");

        //Leave No Rug Unturned
        Story.mapItemQuest(2629, "onslaughttower", 1627);

        //Tame The Lion
        Story.killQuest(2630, "onslaughttower", "Maximillian Lionfang");

        //Take Up The Cause
        Story.killQuest(2666, "falguard", "Chaonslaught Caster");

        //Well Kept Secrets
        Story.mapItemQuest(2667, "falguard", 1628, 6);

        //Feeding On The Fallen
        if (!Story.questProgression(2668)) {
            Core.ensureAccept(2668);
            Core.huntMonster("falguard", "Chaonslaught Warrior", "Chaonslaught Shield", 2);
            Core.huntMonster("falguard", "Chaonslaught Cavalry", "Chaonslaught Sword", 3);
            Core.ensureComplete(2668);
        }

        //Special Delivery
        Story.mapItemQuest(2669, "falguard", 1629);

        //Precious Scraps
        Story.killQuest(2670, "falguard", "Chaonslaught Warrior");

        //Restocking
        Story.mapItemQuest(2671, "falguard", 1630);

        //An Innside Job
        Story.mapItemQuest(2672, "falguard", 1631);

        //Streets Run Red
        Story.killQuest(2673, "falguard", "Chaonslaught Caster");

        //Open the Temple
        Story.mapItemQuest(2674, "falguard", 1632);

        //The Open Temple
        Story.killQuest(2675, "falguard", "Primarch");

        //Remains
        Story.killQuest(2720, "deathpits", "Rotting Darkblood");

        //Thriving In Rot
        Story.mapItemQuest(2721, "deathpits", 1691, 5);

        //Rotting Ribs
        Story.killQuest(2722, "deathpits", "Rotting Darkblood");

        //A Perfect Skull
        Story.killQuest(2723, "deathpits", "Rotting Darkblood");

        //Deeper Into Death
        Story.killQuest(2724, "deathpits", "Ghastly Darkblood");

        //Precise Placement
        Story.killQuest(2725, "deathpits", "Ghastly Darkblood");

        //Painted Protection
        Story.mapItemQuest(2726, "deathpits", 1692, 6);

        //They Sense You
        Story.killQuest(2727, "deathpits", "Rotting Darkblood");

        //They Hate You
        Story.killQuest(2728, "deathpits", "Ghastly Darkblood");

        //The Sundered
        Story.killQuest(2729, "deathpits", "Sundered Darkblood");

        //Rotstone
        Story.mapItemQuest(2730, "deathpits", 1693, 9);

        //Honor The Dead
        if (!Story.questProgression(2731)) {
            Core.ensureAccept(2731);
            Core.huntMonster("deathpits", "Sundered Darkblood", "Primarch Mace");
            Core.huntMonster("deathpits", "Rotting Darkblood", "Primarch Trident");
            Core.huntMonster("deathpits", "Ghastly Darkblood", "Primarch Spear");
            Core.ensureComplete(2731);
        }

        //Ties to Life
        Story.mapItemQuest(2732, "deathpits", 1694, 12);

        //Destroy Wrathful Vestis and Secure The Tears
        if (!Story.questProgression(2740)) {
            Core.ensureAccept(2740);
            Core.getMapItem(1695, 1, "deathpits");
            Core.huntMonster("deathpits", "Wrathful Vestis", "Vestis Defeated", 1);
            Core.ensureComplete(2740);
        }

        //Surveillance for Sir Valence
        Story.mapItemQuest(2792, "venomvaults", 1724);

        //Well Planned Getaway
        Story.killQuest(2793, "venomvaults", "Chaonslaught Warrior");

        //Secrets Of The Mad Prince
        Story.mapItemQuest(2794, "venomvaults", 1725);

        //Potion of Cleansing
        Story.mapItemQuest(2796, "venomvaults", 1726);

        //You've Been Noticed
        if (!Story.questProgression(2798)) {
            Story.killQuest(2797, "venomvaults", "Chaonslaught Caster");

            //Thorny Situations
            Story.mapItemQuest(2798, "venomvaults", 1727, 5);
        }

        //Other Ingredients
        Story.killQuest(2799, "venomvaults", "Chaonslaught Caster");

        //Time For Supplies
        Story.killQuest(2800, "venomvaults", "Chaonslaught Warrior");

        //Cooking Without Fire
        Story.killQuest(2801, "venomvaults", "Chaonslaught Caster");

        //Introduction
        Story.mapItemQuest(2802, "venomvaults", 1728, 3);

        //Courtyard Key
        Story.killQuest(2803, "venomvaults", "Chaonslaught Caster");

        //Take Out The Chaos Manticore!
        Story.killQuest(2804, "venomvaults", "Manticore");

        //Shocking Footwear
        if (!Story.questProgression(2805)) {
            Core.ensureAccept(2805);
            Core.getMapItem(1729, 4, "stormtemple");
            Core.ensureComplete(2805);
        }

        //New Shoes
        if (!Story.questProgression(2806)) //quest was fucky for no reason. hopefully a fix.
        {
            Core.ensureAccept(2806);
            Core.killMonster("stormtemple", "r1", "Left", "*", "Lightning Boots");
            Core.ensureComplete(2806);
        }

        //Mouth Of The Lion
        Story.killQuest(2807, "stormtemple", "Chaonslaught Caster");

        //Storm the Storm Temple
        Story.killQuest(2808, "stormtemple", "Chaonslaught Caster");

        //A High Minded Matter
        Story.mapItemQuest(2809, "stormtemple", 1730, 3);

        //Storm Bottles
        Story.killQuest(2810, "stormtemple", "Chaonslaught Caster");

        //Breaching Defenses
        Story.mapItemQuest(2811, "stormtemple", 1731);

        //Chaos Lightning Rods
        Story.killQuest(2812, "stormtemple", "Chaonslaught Cavalry");

        //Barrier Buster
        Story.mapItemQuest(2813, "stormtemple", 1732);

        //Face Chaos Lord Lionfang!
        Story.killQuest(2814, "stormtemple", "Chaos Lord Lionfang");
    }

    xiang(): void {
        if (Core.isCompletedBefore(3189))
            return;

        Story.preLoad(this.xiang);

        Core.addDrop("Perfect Prism", "Unchaorrupted Sample", "Harpy Feather");

        //Bright Idea
        Story.mapItemQuest(2909, "battleoff", 1779);

        //Spare Parts
        Story.mapItemQuest(2910, "battleoff", 1780, 8);

        //Power It Up
        Story.killQuest(2911, "battleoff", "Evil Moglin");

        //Filthy Creatures
        Story.killQuest(2912, "battleoff", "Evil Moglin");

        //Wave After Wave
        Story.killQuest(2913, "brightfall", "Undead Minion");

        //Take out Their Firepower
        Story.killQuest(2914, "brightfall", "Undead Mage");

        //Help Where It is Needed
        Story.mapItemQuest(2915, "brightfall", 1781, 6);

        //Bring A Ward To A Swordfight
        Story.mapItemQuest(2916, "brightfall", 1782, 8);

        //Cut Off The Head
        Story.killQuest(2917, "brightfall", "Painadin Overlord");

        //Rearm The Legion of Light
        Core.join("overworld");
        Story.chainQuest(2918);

        //Speak to Dage The Good
        Story.killQuest(2919, "overworld", "Undead Minion");

        //Free Their Souls
        Story.killQuest(2920, "overworld", "Undead Minion");

        //One Ring
        Story.killQuest(2921, "overworld", "Undead Minion");

        //Severing Ties
        Story.killQuest(2922, "overworld", "Undead Mage");

        //Legion's Lifesblood
        Story.mapItemQuest(2923, "overworld", 1800, 6);

        //Legion's Purpose
        Story.killQuest(2924, "overworld", "Undead Bruiser");

        //What's His Endgame
        Story.killQuest(2925, "overworld", "Undead Bruiser");

        //A Stopping Block
        Story.killQuest(2926, "overworld", "Undead Minion");

        //Boost Morale
        Story.mapItemQuest(2927, "overworld", 1801, 8);

        //Alteon's Folly
        Story.killQuest(2928, "overworld", "Undead Minion");

        //DoomFire
        Story.killQuest(2929, "overworld", "Undead Minion");

        //Spoiled Souls
        Story.killQuest(2930, "overworld", "Undead Minion");

        //Purity of Bone
        Story.mapItemQuest(2931, "overworld", 1802, 10);

        //Undead Artix Returns!
        Story.killQuest(2932, "overworld", "Undead Artix");

        //I Can't Touch This
        Story.killQuest(3166, "reddeath", "Fire Leech");

        //Nope, Still a Ghost
        if (!Story.questProgression(3167)) {
            Core.ensureAccept(3167);
            Core.getMapItems([ [2178, 1], [2179, 1] ], "reddeath");
            Core.huntMonster("reddeath", "RedDeath Moglin", "Boneweed", 5);
            Core.ensureComplete(3167);
        }

        //First We Need a Beacon...
        Story.mapItemQuest(3168, "reddeath", 2180);

        //Light It Up
        Story.killQuest(3169, "reddeath", "Fire Leech");

        //...Next We need a Trap
        Story.killQuest(3170, "reddeath", "Grim Widow");

        //For Spirits, Not People
        Story.killQuest(3171, "reddeath", "Swamp Wraith");

        //Still To Fragile
        Story.killQuest(3172, "reddeath", "Swamp Wraith");

        //Craft a Better Defense
        Story.mapItemQuest(3183, "battleontown", 2203);

        if (!Core.isCompletedBefore(3188)) {
            Core.ensureAccept(3187);
            Core.equipClass(ClassType.Farm);
            Core.addDrop(
                "Harpy Feather", 
                "Perfect Prism", 
                "Unchaorrupted Sample", 
                "Shriekward Potion");

            //Reflect the Damage
            Core.registerQuests(3184);
            while ((!Core.checkInventory("Perfect Prism")))
                Core.huntMonster("earthstorm", "Shard Spinner", "Reflective Fragment", 5);
            Core.cancelRegisteredQuests();
            bot.wait.forPickup("Perfect Prism");

            //Pure Chaos
            Core.registerQuests(3185);
            while (!Core.checkInventory("Unchaorrupted Sample"))
                Core.huntMonster("bloodtuskwar", "Chaotic Horcboar", "Vials of Blood", 5);
            Core.cancelRegisteredQuests();
            bot.wait.forPickup("Unchaorrupted Sample");

            //Enemies of a Feather Flock Together
            Core.registerQuests(3186);
            while (!Core.checkInventory("Harpy Feather"))
                Core.huntMonster("bloodtuskwar", "Chaos Tigriff", "Feathers", 5);
            Core.cancelRegisteredQuests();
            bot.wait.forPickup("Harpy Feather");

            //Ward Off the Beast
            Core.join("mirrorportal");
            bot.wait.forMapLoad("mirrorportal");
            Core.ensureComplete(3187);
            bot.wait.forPickup("Shriekward Potion");
            Core.sellItem("Shriekward Potion", undefined, { all: true });

            Core.logger("Fixing soemthing with teh chaos harpy, one moment");
            Core.join("whitemap");
            Core.join("mirrorportal");
        }

        //Horror Takes Flight
        if (!Story.questProgression(3188)) {
            Core.equipClass(ClassType.Solo);
            Core.ensureAccept(3188);
            Core.buyItem("mirrorportal", 774, "Shriekward Potion", 99);
            Core.killMonster("mirrorportal", "r3", "Left", "Chaos Harpy", "Chaos Harpy Defeated");
            Core.ensureComplete(3188);
            if (Core.checkInventory("Shriekward Potion")) {
                Core.jumpWait();
                if (bot.inventory.isEquipped(20771)) {
                    Core.sendPackets(`%xt%zm%unequipItem%${bot.map.roomId}%${20771}%`);
                    Core.sleep();
                }
                Core.sellItem("Shriekward Potion", undefined, { all: true });
            }
        }

        //Good, Evil and Chaos Battle!
        if (!Story.questProgression(3189)) {
            Adv.gearStore(undefined, { enhAfter: true });
            Core.ensureAccept(3189);
            Core.killXiang("Chaos Lord Xiang Defeated", undefined, { isTemp: true });
            Core.ensureComplete(3189);
            Adv.gearStore(true, { enhAfter: true });
        }
    }

    alteon(): void {
        if (Core.isCompletedBefore(3160))
            return;

        Story.preLoad(this.alteon);

        //Bandit Bounty
        Story.killQuest(3077, "archives", "Chaos Bandit");

        //Thwarting the Spies
        Story.killQuest(3078, "archives", "Camouflaged Sp-eye");

        //Fight Chaos With Clerics
        if (!Story.questProgression(3079)) {
            Core.ensureAccept(3079);
            Core.huntMonster("archives", "Chaos Bandit", "Clump of Tentacles", 3);
            Core.huntMonster("archives", "Camouflaged Sp-eye", "Chaorrupted Humor", 3);
            Core.ensureComplete(3079);
        }

        //Locate the Source
        if (!Story.questProgression(3080)) {
            Core.ensureAccept(3080);
            Core.getMapItem(1937, 1, "archives");
            Core.huntMonster("archives", "Chaos Bandit", "Reeking Clue", 6);
            Core.ensureComplete(3080);
        }

        //Plagued Rats
        Story.killQuest(3081, "archives", "Chaos Rat");

        //Nope, Nope, Nope!
        Story.killQuest(3082, "archives", "Chaos Spider");

        //Still More Research To Be Done!
        if (!Story.questProgression(3083)) {
            Core.ensureAccept(3083);
            Core.huntMonster("archives", "Chaos Rat", "Intact Rat Brain", 5);
            Core.huntMonster("archives", "Chaos Spider", "Vial of Ichor", 5);
            Core.ensureComplete(3083);
        }

        //That's One Big Sludgebeast.
        Story.killQuest(3084, "archives", "Sludgelord");

        //Back to Jail With You!
        Story.killQuest(3094, "armory", "Chaorrupted Prisoner");

        //We May Need A Militia
        if (!Story.questProgression(3095)) {
            Core.ensureAccept(3095);
            Core.getMapItem(1956, 4, "armory");
            Core.huntMonster("armory", "Chaorrupted Prisoner", "Weapon Recovered", 4);
            Core.ensureComplete(3095);
        }

        //An Ounce Of Prevention
        Story.killQuest(3096, "armory", "Chaos Drifter");

        //Axe Them To Leave!
        Story.killQuest(3089, "armory", "Chaorrupted Prisoner");

        //Freeze 'Em Out!
        Story.killQuest(3090, "armory", "Chaos Mage");

        //Burn 'Em Up!
        Story.killQuest(3091, "armory", "Chaos Mage");

        //Under Siege
        Story.mapItemQuest(3092, "armory", 1957);

        //No, NOW We're Under Siege
        Story.killQuest(3093, "armory", "Chaos General");

        //Chaos Not Invited
        Story.killQuest(3120, "ceremony", "Chaos Invader");

        //Better Letter Go!
        if (!Story.questProgression(3121)) {
            Core.ensureAccept(3121);
            Core.getMapItem(2108, 1, "yulgar");
            Core.getMapItem(2109, 1, "yulgar");
            Core.getMapItem(2110, 1, "yulgar");
            Core.getMapItem(2111, 1, "archives");
            Core.getMapItem(2112, 1, "swordhaven");
            Core.getMapItem(2113, 1, "swordhaven");
            Core.getMapItem(2114, 1, "swordhaven");
            Core.getMapItem(2115, 1, "swordhaven");
            Core.ensureComplete(3121);
        }

        //Decor Rater
        Story.mapItemQuest(3122, "swordhaven", 2116, 8);

        //Cold Feet, Warm Heart
        Story.killQuest(3123, "mafic", "Living Fire");

        //Chaos STILL Not Invited
        Story.killQuest(3124, "ceremony", "Chaos Invader");

        //Protect the Princesses
        Story.mapItemQuest(3125, "ceremony", 2118, 6);

        //Seal the Chapel
        if (!Story.questProgression(3126)) {
            Core.ensureAccept(3126);
            Core.getMapItem(2119, 1, "ceremony");
            Core.huntMonster("ceremony", "Chaos Invader", "Invaders Defeated", 8);
            Core.ensureComplete(3126);
        }

        //Chaos Kills!
        Story.killQuest(3127, "ceremony", "Chaos Justicar");

        //Endless Aisle of Chaos
        Story.mapItemQuest(3133, "chaosaltar", 2127, 12);

        //Save the Princess... Again!
        Story.killQuest(3134, "chaosaltar", "Princess Thrall");

        //Chaos Dragon Confrontation
        Story.killQuest(3158, "castleroof", "Chaos Dragon");

        //To Catch a King
        Story.mapItemQuest(3159, "swordhavenfalls", 2158);

        //Chaos Lord Alteon
        Story.killQuest(3160, "swordhavenfalls", "Chaos Lord Alteon");
    }

    hero(): void {
        if (Core.isCompletedBefore(3799) && Core.isCompletedBefore(3881))
            return;

        Story.preLoad(this.hero);

        //12 Lords of Chaos
        Story.chainQuest(3578);

        //Prologue: Good vs Evil
        Story.chainQuest(3579);

        //1st Lord of Chaos
        Story.chainQuest(3580);

        //3rd Lord of Chaos
        Story.chainQuest(3581);

        //4th Lord of Chaos
        Story.chainQuest(3582);

        //5th Lord of Chaos
        Story.chainQuest(3583);

        //6th Lord of Chaos
        Story.chainQuest(3584);

        //7th Lord of Chaos
        Story.chainQuest(3585);

        //8th Lord of Chaos
        Story.chainQuest(3586);

        //9th Lord of Chaos
        Story.chainQuest(3587);

        //10th Lord of Chaos
        Story.chainQuest(3588);

        //11th Lord of Chaos
        Story.chainQuest(3589);

        //2nd Lord of Chaos
        Story.chainQuest(3590);

        //12th Lord of Chaos
        Story.chainQuest(3591);

        //Mountain Top Reached
        Story.chainQuest(3764);

        //Drakath Faced
        Story.mapItemQuest(3765, "mountdoomskull", 2726);

        //Who is the 13th Lord of Chaos?
        Story.chainQuest(3766);

        //World War Lore!
        Story.chainQuest(3779);

        //Battle for Chaos in Willowcreek!
        Story.killQuest(3781, "newfinale", "Alliance Virago");

        //Defeat the Chaos Challenger
        Story.killQuest(3788, "newfinale", "Chaos Challenger");

        //Battle for Chaos in Doomwood!
        Story.killQuest(3783, "newfinale", "Alliance Virago");

        //Beat Chaorrupted Lycan Hunter
        if (!Story.questProgression(3789)) {
            Core.ensureAccept(3789);
            Core.huntMonster("newfinale", "Chaorrupted Lycan Hunter", "Chaorrupted Lycan Hunter Defeated");
            Core.ensureAccept(3789);
        }

        //Battle for Chaos in Darkovia!
        Story.killQuest(3785, "newfinale", "Alliance Virago");

        //Defeat the Memory of Vampires
        Story.killQuest(3790, "newfinale", "Memory of Vampires");

        //Battle for Chaos in the Lair!
        Story.killQuest(3787, "newfinale", "Alliance Virago");

        //1st Chaos Beast
        Story.chainQuest(3608);

        //2nd Chaos Beast
        Story.chainQuest(3618);

        //3rd Chaos Beast
        Story.chainQuest(3609);

        //4th Chaos Beast
        Story.chainQuest(3610);

        //5th Chaos Beast
        Story.chainQuest(3611);

        //6th Chaos Beast
        Story.chainQuest(3612);

        //7th Chaos Beast
        Story.chainQuest(3613);

        //8th Chaos Beast
        Story.chainQuest(3614);

        //9th Chaos Beast
        Story.chainQuest(3615);

        //10th Chaos Beast
        Story.chainQuest(3616);

        //11th Chaos Beast
        Story.chainQuest(3617);

        //12th Chaos Beast
        Story.chainQuest(3619);

        //The Chaos Beast is called forth.
        Story.chainQuest(3791);

        //Time to save Battleon!
        Story.chainQuest(3792);

        //Battle for Chaos in Battleon!
        Story.killQuest(3794, "newfinale", "Alliance Soldier");

        //Become a Chaos Lord
        Core.join("newfinale", "NPC2", "Left");
        Core.sendPackets("%xt%zm%setAchievement%15075%ia1%20%1%");

        //Battle the Champion of Chaos!
        Story.mapItemQuest(3795, "drakathfight", 2894);

        //Arrive in the Underworld
        Story.killQuest(3620, "shadowrise", "Darkness Elemental");

        //Search for Death's Lair
        Story.mapItemQuest(3796, "shadowrise", 2895);

        //Arrive in Shadowattack
        Story.chainQuest(3797);

        //Find your way to Death's lair
        if (!Story.questProgression(3798)) {
            this.resetShadowAttack();
            Core.ensureAccept(3798);
            Core.getMapItem(2896, 1, "shadowattack");
            bot.wait.forTrue(() => bot.tempInv.contains(25903) || bot.tempInv.contains("Hunt for Death"), 20);
            Core.ensureComplete(3798);
            bot.wait.forQuestComplete(3798);
        }

        //Beat Death!
        // Get out of any cutscene cells
        // Maybe a map reset (white map> go back will fix it?)
        Core.equipClass(ClassType.Solo);
        if (!Story.questProgression(3799)) {
            this.resetShadowAttack();
            Core.ensureAccept(3799);
            Core.huntMonster("shadowattack", "Death", "You Beat Death?!");
            Core.ensureComplete(3799);
        }

        //Enter Confrontation
        if (!Core.isCompletedBefore(3876)) {
            Core.ensureAccept(3875);
            Core.join("confrontation");
            Core.ensureComplete(3875);
        }
        Core.equipClass(ClassType.Solo);
        // Defeat Drakath!
        if (!Story.questProgression(3876)) {
            Core.ensureAccept(3876);
            Core.killMonster("finalbattle", "r1", "Left", 1, 26875);
            Core.ensureComplete(3876);
        }

        //Defeat Drakath.. again!
        if (!Story.questProgression(3877)) {
            Core.ensureAccept(3877);
            Core.killMonster("finalbattle", "r4", "Left", 14, 26876);
            Core.ensureComplete(3877);
        }

        //Defeat Drakath!
        if (!Story.questProgression(3878)) {
            Core.ensureAccept(3878);
            Core.killMonster("finalbattle", "r9", "Left", 23, 26877);
            Core.ensureComplete(3878);
            Core.join("whitemap");
        }

        //Defeat the 12 Lords of Chaos!
        if (!Story.questProgression(3879)) {
            Core.ensureAccept(3879);
            Core.killMonster("chaosrealm", "r24", "Left", "Alteon", "Chaos Lord Alteon Defeated");
            Core.ensureComplete(3879);
            Core.join("whitemap");
        }

        // Defeat the 13th Lord of Chaos
        if (!Core.isCompletedBefore(3880)) {
            Core.ensureAccept(3880);
            Core.join("chaoslord");
            if (bot.player.cell !== "r2")
                Core.jump("r2", "Left");
            Core.equipClass(ClassType.Solo);
            while (!bot.shouldExit) {
                bot.combat.attack("*");
                Core.sleep();
                if (bot.tempInv.contains("13th Lord of Chaos Defeated")) {
                    Core.jumpWait();
                    break;
                }
            }
            if (bot.quests.canCompleteFullCheck(3880)) {
                Core.sleep();
                bot.quests.ensureComplete(3880);
                Core.sleep();
            }
            Core.join("whitemap");
        }

        // The Final Showdown!
        if (!Core.isCompletedBefore(3881)) {
            Core.ensureAccept(3881);
            Core.join("finalshowdown");
            if (bot.player.cell !== "r2")
                Core.jump("r2", "Left");
            while (!bot.shouldExit) {
                bot.combat.attack("*");
                Core.sleep();
                if (bot.tempInv.contains(26881)) {
                    Core.jumpWait();
                    break;
                }
            }

            if (bot.quests.canCompleteFullCheck(3881)) {
                Core.sleep();
                bot.quests.ensureComplete(3881);
                Core.sleep();
            }
            Core.join("whitemap");
        }
    }

    private resetShadowAttack(): void {
            Core.logger("Resetting shadowAttack Map");
            Core.jumpWait();
            Core.sleep(2500);
            Core.join("whitemap-100000");
            Core.sleep(2500);
            bot.map.join(`shadowattack-` + Core.privateRoomNumber, undefined, undefined, undefined, false);
            bot.wait.forMapLoad("shadowattack");
            Core.sleep(2500);
    }

    extra(): void {
        if (Core.isCompletedBefore(3824))
            return;

        Story.preLoad(this.extra);

        //Arrive in DreadHaven
        Story.chainQuest(3812);

        //Kill SlugWrath in Dreadhaven
        Story.chainQuest(3813);

        //Kill Bandit Drakath in Dreadhaven
        Story.chainQuest(3814);

        //Up the Mountain
        Core.equipClass(ClassType.Farm);
        if (!Story.questProgression(3815)) {
            Core.ensureAccept(3815);
            Core.huntMonster("falcontower", "Sir Knight", "Enemy Defeated", 3);
            Core.ensureComplete(3815);
        }

        //Higher Up
        if (!Story.questProgression(3816)) {
            Core.ensureAccept(3816);
            Core.huntMonster("falcontower", "Sir Knight", "Enemy Defeated", 3);
            Core.ensureComplete(3816);
        }

        //Even Higher
        if (!Story.questProgression(3817)) {
            Core.ensureAccept(3817);
            Core.huntMonster("falcontower", "Sir Knight", "Enemy Defeated", 3);
            Core.ensureComplete(3817);
        }

        //Falconreach Tower
        if (!Story.questProgression(3818)) {
            Core.ensureAccept(3818);
            Core.huntMonster("falcontower", "Sir Knight", "Enemy Defeated", 3);
            Core.ensureComplete(3818);
        }

        //Climb the Tower
        if (!Story.questProgression(3819)) {
            Core.ensureAccept(3819);
            Core.huntMonster("falcontower", "Sir Knight", "Enemy Defeated", 2);
            Core.ensureComplete(3819);
        }

        //To the Dragonlord
        if (!Story.questProgression(3820)) {
            Core.ensureAccept(3820);
            Core.huntMonster("falcontower", "Sir Knight", "Enemy Defeated", 5);
            Core.ensureComplete(3820);
        }

        //Defeat the Dragonlord
        Core.equipClass(ClassType.Solo);
        Story.killQuest(3821, "falcontower", "DragonLord");

        //Defeat Dragon Drakath
        Story.killQuest(3822, "falcontower", "Dragon Drakath");

        //Defeat Sepulchure
        Story.killQuest(3823, "falcontower", "Sepulchure");

        //Defeat Alteon
        Story.killQuest(3824, "falcontower", "Alteon");
    }
}
