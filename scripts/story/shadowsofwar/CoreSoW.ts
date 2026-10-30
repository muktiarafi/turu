/*
name: null
description: null
tags: null
*/
// Port of Scripts/Story/ShadowsOfWar/CoreSoW.cs.

import { bot } from "turu";
import { CoreAdvanced } from "../../core/CoreAdvanced";
import { ClassType, Core } from "../../core/CoreBots";
import { CoreStory } from "../../core/CoreStory";

const Story = new CoreStory();
const Adv = new CoreAdvanced();

export class CoreSoW {
    mainyuDrops: string[] = [ "Mainyu Rune", "Mainyu Tail", "Mainyu Wings" ];

    malgorDrops: string[] = [
        "Shadow DragonMaster",
        "Shadow DragonMaster's Helm",
        "Shadow DragonMaster's Tail + Blades",
        "Shadow Dragonscale Blade",
        "Shadowflame Devastation",
        "ShadowFlame Dragon Blade",
        "Shadowflame Shroud",
        "Shadowflame Vanguard Armet",
        "Shadowflame Vanguard Armet Locks",
        "Shadowflame Vanguard Spear",
    ];

    completeCoreSoW(): void {
        this.shadowWar();
        this.shadowlordKeep();
        this.timestream();
        this.graniteCove();
        this.blackseaKeep();
        this.junkhoard();
        this.junkheap();
        this.shadowgrove();
        this.aozoraHills();
        this.ghostNexus();
        this.shadowsong();
        this.darkAlly();
        this.darkAlliance();
        this.innerShadows();
        this.tyndarius();
        this.ruinedCrown();
        this.timekeep();
        this.timestreamWar();
        this.deadLines();
        this.shadowFlame();
        this.manaCradle();
    }

    shadowWar(): void {
        if (Core.isCompletedBefore(6852))
            return;

        Story.preLoad(this.shadowWar);

        // Shadow Medals: Defend the Village! 6846
        Story.killQuest(6846, "shadowwar", "Shadowflame Slasher", { getReward: false });

        //Seed Spitter Oil 6847
        Story.killQuest(6847, "shadowwar", "Seed Spitter", { getReward: false });

        //Mega Shadow Medals 6848
        if (!Story.questProgression(6848)) {
            Core.ensureAccept(6848);
            Core.huntMonster("shadowwar", "Shadowflame Slasher", "Mega Shadow Medal", 3);
            Core.ensureComplete(6848);
        }
        //Shadow Samples 6849
        if (!Story.questProgression(6849)) {
            Core.ensureAccept(6849);
            Core.huntMonster("shadowwar", "Umbral Goo", "Shadow Samples", 5);
            Core.ensureComplete(6849);
        }

        //Shadow Medals: Fight them Back! 6850
        if (!Story.questProgression(6850)) {
            Core.ensureAccept(6850);
            Core.killMonster("shadowwar", "r2", "Left", "*", "Shadow Medal", 5);
            Core.ensureComplete(6850);
        }

        //Interrogation 6851 //had multiple reports made it a questprog.
        if (!Story.questProgression(6851)) {
            Core.ensureAccept(6851);
            Core.huntMonster("shadowwar", "Shadowflame Scout", "Useful Clue", 3);
            Core.ensureComplete(6851);
            bot.wait.forQuestComplete(6851); //insurance... because y[e]
        }

        //Defeat Malgor! 6852 //quest prog, due to getting stuck on a cutscene cell.
        if (!Story.questProgression(6852)) {
            Core.jumpWait();
            bot.options.attackWithoutTarget = true;
            Core.ensureAccept(6852);
            Adv.gearStore(false, { enhAfter: true });
            Core.useBossClass();
            Core.huntMonster("malgor", "Malgor", "Defeat Malgor", 1, { log: false });
            Adv.gearStore(true, { enhAfter: true });
            bot.options.attackWithoutTarget = false;
            Core.ensureComplete(6852);
            bot.wait.forMapLoad("shadowwar"); //game will force you bac to shadowar-1 ._. so you're welcome.
            Core.join("whitemap-100000");
        }
    }

    shadowlordKeep(): void {
        if (Core.isCompletedBefore(6864))
            return;

        Story.preLoad(this.shadowlordKeep);

        //Defeat the Minions (6856)
        if (!Story.questProgression(6856)) {
            Core.ensureAccept(6856);
            Core.getMapItem(6390, 1, "shadowlordkeep");
            Core.huntMonster("shadowlordkeep", "Shadow Gunner", "Shadow Minion Defeated", 10);
            Core.ensureComplete(6856);
        }

        //Gather the Gunpowder (6857)
        Story.killQuest(6857, "shadowlordkeep", "Shadow Gunner");

        //Build a Bomb (6858)
        if (!Story.questProgression(6858)) {
            Core.ensureAccept(6858);
            while (
                !bot.shouldExit && (!Core.checkInventory(48347) || !Core.checkInventory(48348, 8))
            ) //Torch && Scrap of Cloth
                Core.huntMonster("shadowlordkeep", "Shadow Mage", null, 1, { log: false });
            Core.ensureComplete(6858);
        }

        //Light 'em Up! (6859)
        Story.mapItemQuest(6859, "shadowlordkeep", 6391);

        //Fight the Shadows (6860)
        Story.killQuest(6860, "shadowlordkeep", "Shadow Imp");

        //Magical Potion (6861)
        Story.killQuest(6861, "shadowlordkeep", "Shadow Mage");

        //Overload the Energy Field (6862)
        if (!Story.questProgression(6862)) {
            Core.ensureAccept(6862);
            Core.getMapItem(6392, 1, "shadowlordkeep");
            Core.huntMonster("shadowlordkeep", "Stray Energy", "Stray Energy", 8);
            Core.ensureComplete(6862);
        }

        //Defeat the Portal Guards (6863)
        Story.killQuest(6863, "shadowlordkeep", "Portal Guard");

        //Go Through the Portal (6864)
        Story.mapItemQuest(6864, "timestream", 6393);
    }

    timestream(): void {
        if (Core.isCompletedBefore(6867))
            return;

        this.shadowlordKeep();

        Story.preLoad(this.timestream);

        //Fuel for the Cell (6865)
        Story.killQuest(6865, "timestream", "Spacetime Energy");

        //Open the Rift (6866)
        Story.mapItemQuest(6866, "timestream", 6394);

        //Defeat the ShadowKnight (6867)
        Story.killQuest(6867, "timestream", "ShadowKnight Gar");
    }

    graniteCove(): void {
        if (Core.isCompletedBefore(6883))
            return;

        Story.preLoad(this.graniteCove);

        //Get A Disguise (6873)
        Story.killQuest(6873, "granitecove", "Blacksea Pirate");

        //Gear Up! (6874)
        Story.mapItemQuest(6874, "granitecove", 6433);

        //Gather Information (6875)
        Story.mapItemQuest(6875, "granitecove", [ 6434, 6435, 6436 ]);

        //Find the Missing Pages (6876)
        Story.killQuest(6876, "granitecove", "Jungle Treeant");

        //Collect the Beans (6877)
        Story.killQuest(6877, "granitecove", "Island Monkey");

        //Smooth the Way (6878)
        Story.killQuest(6878, "granitecove", "Jungle Treeant");

        //Sweeten the Pot (6879)
        Story.killQuest(6879, "granitecove", "Blacksea Pirate");

        //Wake the Statues (6880)
        Story.mapItemQuest(6880, "granitecove", 6438, 10);

        //Clear the Tattersail Pirates (6881)
        Story.killQuest(6881, "granitecove", "Tattersail Pirate");

        //Kill Bill (6882)
        Story.killQuest(6882, "granitecove", "Blacktooth Bill");

        //Defeat Teja! (6883)
        Story.killQuest(6883, "granitecove", "Pirate Queen Teja");
    }

    blackseaKeep(): void {
        if (Core.isCompletedBefore(6896))
            return;

        Story.preLoad(this.blackseaKeep);

        //Get the Keys (6886)
        Story.killQuest(6886, "blackseakeep", "Blacksea Scallywag");

        //Open the Chests (6887)
        if (!Story.questProgression(6887)) {
            Core.ensureAccept(6887);
            Core.getMapItem(6446, 6, "blackseakeep");
            Core.huntMonster("blackseakeep", "Pure Darkness", "Shadow Slain", 6);
            Core.ensureComplete(6887);
        }

        //Interrogation Time (6888)
        Story.killQuest(6888, "blackseakeep", "Blacksea Privateer");

        //Get the Picks (6889)
        Story.killQuest(6889, "blackseakeep", "Blacksea Scallywag");

        //Break In! (6890)
        Story.mapItemQuest(6890, "blackseakeep", 6448);

        //Kick that Booty! (6891)
        Story.killQuest(6891, "blackseakeep", "Blacksea Privateer");

        //We Need a Spell (6892)
        if (!Story.questProgression(6892)) {
            Core.ensureAccept(6892);
            Core.huntMonster("blackseakeep", "BlackSea Pirate Mage", "Spell Reagents", 8);
            Core.huntMonster("blackseakeep", "BlackSea Pirate Mage", "Compliant Mage", 1);
            Core.ensureComplete(6892);
        }

        //Light up the Shadows (6893)
        Story.mapItemQuest(6893, "blackseakeep", 6449, 7);

        //Get the Pitch (6894)
        Story.killQuest(6894, "blackseakeep", "Blacksea Privateer");

        //Light It Up (6895)
        Story.mapItemQuest(6895, "blackseakeep", 6450, 8);

        //Get My Eye! (6896)
        Story.killQuest(6896, "blackseakeep", "First Mate Bloodbone");
    }

    junkhoard(): void {
        if (Core.isCompletedBefore(6960))
            return;

        Story.preLoad(this.junkhoard);

        //"Salute" the Magpie (6949)
        Story.killQuest(6949, "junkhoard", "Magpie");

        //Gather the Parts (6950)
        Story.killQuest(6950, "junkhoard", "Junk Golem");

        //Engine Needs a Spark (6951)
        Story.killQuest(6951, "junkhoard", "Portal Manifestation");

        //Metal for Welding (6952)
        Story.killQuest(6952, "junkhoard", "Junk Golem");

        //Feel the Heat (6953)
        Story.killQuest(6953, "junkhoard", "Portal Manifestation");

        //Find the Forge (6954)
        Story.mapItemQuest(6954, "junkhoard", 6497);

        //Get the Coal (6955)
        Story.killQuest(6955, "junkhoard", "Magpie");

        //Defeat the Coal Elemental (6956)
        Story.killQuest(6956, "junkhoard", "Coal Elemental");

        //Filthy Thieving Magpies (6957)
        Story.killQuest(6957, "junkhoard", "Magpie");

        //Track the Tracker (6958)
        Story.killQuest(6958, "junkhoard", "Junk Golem");

        //Follow the Chakram (6959)
        Story.mapItemQuest(6959, "junkhoard", 6498);

        //Get it BACK! (6960)
        Story.killQuest(6960, "junkhoard", "Magpie Junk Heap");
    }

    junkheap(): void {
        if (Core.isCompletedBefore(6974))
            return;

        this.junkhoard();

        Story.preLoad(this.junkheap);

        //Whispering Walls (6961)
        Story.mapItemQuest(6961, "junkheap", 6500, 5);

        //So Many Magpies (6962)
        Story.killQuest(6962, "junkheap", "Magpie");

        //Charge the Portals (6963)
        if (!Story.questProgression(6963)) {
            Core.ensureAccept(6963);
            Core.getMapItem(6501, 1, "junkheap");
            Core.huntMonster("junkheap", "Tiny Manifestation ", "Tiny Energy", 8);
            Core.ensureComplete(6963);
        }

        //Through the Rift (6964)
        Story.mapItemQuest(6964, "junkheap", 6502);

        //Free Niknak (6965)
        if (!Story.questProgression(6965)) {
            Core.ensureAccept(6965);
            Core.getMapItem(6503, 1, "junkheap");
            Core.huntMonster("junkheap", "Shadowflame Scout", "Niknak's Key", 1);
            Core.ensureComplete(6965);
        }

        //Afraid of Shadows (6966)
        Story.killQuest(6966, "junkheap", "Shadow Imp");

        //Wood for Splints (6967)
        Story.killQuest(6967, "junkheap", "Dark Treeant");

        //First Aid (6968)
        if (!Story.questProgression(6968)) {
            Core.ensureAccept(6968);
            Core.huntMonster("junkheap", "Shadow Imp", "Salve Weed", 2);
            Core.huntMonster("junkheap", "Shadow Imp", "Healing Potion", 3);
            Core.ensureComplete(6968);
        }

        //Delivery Time (6969)
        Story.mapItemQuest(6969, "junkheap", 6504);

        //Need Food (6970)
        if (!Story.questProgression(6970)) {
            Core.ensureAccept(6970);
            Core.huntMonster("junkheap", "Dark Treeant", "Questionable Fruit", 5);
            Core.huntMonster("junkheap", "Shadowflame Scout", "Canteen", 5);
            Core.ensureComplete(6970);
        }

        //A New Coat (6971)
        Story.killQuest(6971, "junkheap", "Shadow Imp");

        //Bruise the Bruiser (6972)
        Story.killQuest(6972, "junkheap", "Shadowflame Bruiser");

        //Shadowy Yulgar (6973)
        Story.killQuest(6973, "junkheap", "Shadowflame Yulgar");

        //Take the Portal (6974)
        Story.mapItemQuest(6974, "junkhoard", 6505);
    }

    shadowgrove(): void {
        if (Core.isCompletedBefore(6999))
            return;

        Story.preLoad(this.shadowgrove);

        //Dragonlord Medals: Defend Arcangrove! (6993)
        Story.killQuest(6993, "shadowgrove", "Shadow Dragonlord");

        //Dragonscale Powder (6994)
        Story.killQuest(6994, "shadowgrove", "Shadow Wyvern");

        //Mega Dragonlord Medals (6995)
        Story.killQuest(6995, "shadowgrove", "Shadow Dragonlord");

        //Dragon Oil (6996)
        Story.killQuest(6996, "shadowgrove", "Shadow Wyvern");

        //Head of a Dragon (6997)
        Story.killQuest(6997, "shadowgrove", "Mutant Shadow Dragon");

        //Mutant Dragon Oil (6998) - buggy mob
        if (!Story.questProgression(6998)) {
            Core.ensureAccept(6998);
            Core.killMonster("shadowgrove", "r9", "Left", "Mutant Shadow Dragon", "Mutant Dragon Oil", 3);
            Core.ensureComplete(6998);
        }

        //Titan Shadow Dragonlord (6999)
        Story.killQuest(6999, "shadowgrove", "Titan Shadow Dragonlord");
    }

    aozoraHills(): void {
        if (Core.isCompletedBefore(7062))
            return;

        Story.preLoad(this.aozoraHills);

        //Gather Offerings (7050)
        if (!Story.questProgression(7050)) {
            Core.ensureAccept(7050);
            Core.getMapItem(6643, 5, "aozorahills");
            Core.getMapItem(6644, 5, "aozorahills");
            Core.huntMonster("aozorahills", "Reishi", "Fresh Truffles", 5);
            Core.ensureComplete(7050);
        }

        //Deliver the Gift (7051)
        Story.mapItemQuest(7051, "aozorahills", 6645);

        //Pest Control (7052)
        Story.killQuest(7052, "aozorahills", "Aozora Kijimuna");

        //Prove Your Worth (7053)
        Story.killQuest(7053, "aozorahills", "Osanshouo");

        //Light for the Lantern (7054)
        Story.killQuest(7054, "aozorahills", "Kosenjobi");

        //Attune to Fuyurei (7055)
        if (!Story.questProgression(7055)) {
            Core.ensureAccept(7055);
            Core.getMapItem(6646, 1, "aozorahills");
            Core.huntMonster("aozorahills", "Fuyurei", "Fuyurei Ki", 8);
            Core.ensureComplete(7055);
        }

        //Follow the Path (7056)
        Story.mapItemQuest(7056, "aozorahills", 6647, 3);

        //Attune to Kijimuna (7057)
        if (!Story.questProgression(7057)) {
            Core.ensureAccept(7057);
            Core.getMapItem(6648, 1, "aozorahills");
            Core.huntMonster("aozorahills", "Aozora Kijimuna", "Kijimuna Ki", 8);
            Core.ensureComplete(7057);
        }

        //Path of the Kijimuna (7058)
        Story.mapItemQuest(7058, "aozorahills", 6649, 3);

        //Attune to Reishi (7059)
        if (!Story.questProgression(7059)) {
            Core.ensureAccept(7059);
            Core.getMapItem(6650, 1, "aozorahills");
            Core.huntMonster("aozorahills", "Reishi", "Reishi Ki", 8);
            Core.ensureComplete(7059);
        }

        //Path to Reishi (7060)
        Story.mapItemQuest(7060, "aozorahills", 6651, 3);

        //Black Flames (7061)
        Story.killQuest(7061, "aozorahills", "Kosenjobi");

        //Defeat Grandmother Hasu (7062)
        Story.killQuest(7062, "aozorahills", "Ghostly Hasu");
    }

    ghostNexus(): void {
        if (Core.isCompletedBefore(7118))
            return;

        this.aozoraHills();

        Story.preLoad(this.ghostNexus);

        //What's That I Hear? (7106)
        Story.mapItemQuest(7106, "ghostnexus", 6700);

        //Goat Gone Wild (7107)
        Story.killQuest(7107, "ghostnexus", "Chaos Goat");

        //Creepy Eyes (7108)
        Story.killQuest(7108, "ghostnexus", "Chaos Sp-eye");

        //Find My Parents (7109)
        Story.mapItemQuest(7109, "ghostnexus", 6701);

        //More Food (7110)
        if (!Story.questProgression(7110)) {
            Core.ensureAccept(7110);
            Core.huntMonster("ghostnexus", "Chaos Goat", "Goat Milk", 5);
            Core.huntMonster("ghostnexus", "Chaos Wolf", "Wolf Meat", 5);
            Core.ensureComplete(7110);
        }

        //Clear the Way (7111)
        Story.killQuest(7111, "ghostnexus", "Gnarltooth");

        //Drive Them Back (7112)
        Story.killQuest(7112, "ghostnexus", "Infernal Knight");

        //Calm the Yokai (7113)
        Story.killQuest(7113, "ghostnexus", "Abumi Guchi");

        //Get the Supplies (7114)
        if (!Story.questProgression(7114)) {
            Core.ensureAccept(7114);
            Core.huntMonster("ghostnexus", "Infernal Knight", "Spare Weapon", 8);
            Core.huntMonster("ghostnexus", "Abumi Guchi", "Crate of Food", 8);
            Core.ensureComplete(7114);
        }

        //Defeat the Nether Warlock (7115)
        Story.killQuest(7115, "ghostnexus", "Nether Warlock");

        //So... Sad... (7116)
        Story.killQuest(7116, "ghostnexus", "Manifestation of Grief");

        //Release the Rage (7117)
        Story.killQuest(7117, "ghostnexus", "Manifestation of Rage");

        //Anguish is the Worst (7118)
        Story.killQuest(7118, "ghostnexus", "Mahou's Anguish");
    }

    shadowsong(): void {
        if (Core.isCompletedBefore(7348))
            return;

        Story.preLoad(this.shadowsong);

        //Shadow Medal (7340)
        if (!bot.quests.isUnlocked(7341)) {
            Core.ensureAccept(7340);
            while (!bot.shouldExit && !Core.checkInventory(53063, 5))
                Core.huntMonster("shadowsong", "Shadowflame Troll", null, 1, { log: false });
            Core.ensureComplete(7340);
        }

        //Song of Fate (7341)
        Story.killQuest(7341, "shadowsong", "Tune-A-Fish");

        //Broken Strings (7342)
        Story.killQuest(7342, "shadowsong", "Beatle");

        //Shattered Crystal (7343)
        Story.killQuest(7343, "shadowsong", "Shattered Crystal");

        //Mega Shadow Medals (7345)
        Core.ensureAccept(7344);
        Core.huntMonster("shadowsong", "Shadowflame Ur-Troll", "Mega Shadow Medal", 3);
        Core.ensureComplete(7344);

        //New Music (7345)
        Story.killQuest(7345, "shadowsong", "Mozard");

        //Listen to the Drums (7346)
        Story.killQuest(7346, "shadowsong", "Pachelbel's Cannon");

        //Strength in Music (7347)
        Story.killQuest(7347, "shadowsong", "Mozard");

        //Stop the Music (7348)
        Story.killQuest(7348, "shadowsong", "Oh'Garr");
    }

    darkAlly(): void {
        if (Core.isCompletedBefore(7428))
            return;

        Story.preLoad(this.darkAlly);

        //Shadows Encroach
        if (!Story.questProgression(7419)) {
            Story.mapItemQuest(7419, "darkally", 7179, 6);
            Core.killMonster("darkally", "r2", "Left", "Shadow", "Shadow Destroyed", 10);
            Core.ensureComplete(7419);
        }

        if (!Story.questProgression(7420)) {
            Core.ensureAccept(7420);
            Core.huntMonster("darkally", "Underworld Golem", "Underworld Granite", 8);
            Core.huntMonster("darkally", "Underworld Golem", "Underworld Diamond", 8);
            Core.ensureComplete(7420);
        }

        Story.mapItemQuest(7421, "darkally", 7180, 1);
        Story.mapItemQuest(7421, "darkally", 7181, 8);

        if (!Story.questProgression(7422)) {
            Core.ensureAccept(7422);
            Core.join("Darkally", "r2", "Left");
            while (!bot.shouldExit && !Core.checkInventory(53855, 10))
                Core.killMonster("Darkally", "r2", "Left", "Dark Makai");
            Core.ensureComplete(7422);
        }

        if (!Story.questProgression(7423)) {
            Core.ensureAccept(7423);
            Core.killMonster("Darkally", "r2", "Left", 4452, "Shredded Shadow", 9);
            Core.ensureComplete(7423);
        }

        if (!Story.questProgression(7424)) {
            Core.ensureAccept(7424);
            Core.huntMonster("Darkally", "Legion Defector", "Defectors Slain", 10);
            Core.huntMonster("Darkally", "Legion Defector", "Legion Soul", 5);
            Core.ensureComplete(7424);
        }

        Story.killQuest(7425, "darkally", "Frozen Pyromancer");

        Story.killQuest(7426, "darkally", "Underworld Golem");

        Story.mapItemQuest(7427, "darkally", 7182, 1);

        Story.killQuest(7428, "darkally", "Underfiend");
    }

    darkAlliance(): void {
        if (Core.isCompletedBefore(7460))
            return;

        this.darkAlly();
        Story.preLoad(this.darkAlliance);

        //Clear the Shadows --DAGE--
        if (!Story.questProgression(7446)) {
            Core.ensureAccept(7446);
            Core.getMapItem(7224, 8, "darkalliance");
            Core.huntMonster("darkalliance", "Shadow", "Shadow Slain", 8);
            Core.ensureComplete(7446);
        }
        //Destroy the Swords
        if (!Story.questProgression(7447)) {
            Core.ensureAccept(7447);
            Core.getMapItem(7225, 1, "darkalliance");
            Core.huntMonster("darkalliance", "Shadowblade", "Shadow Sword Slain", 7);
            Core.ensureComplete(7447);
        }
        //Fuel for the Forge
        Story.killQuest(7448, "darkalliance", "Shadow Void");
        //Sever the Connection
        Story.killQuest(7449, "darkalliance", "Shadow Makai");
        //Armor Against the Shadow
        Story.killQuest(7450, "darkalliance", "Shadow Void");
        //Aid from the Pyromancer
        Story.mapItemQuest(7451, "darkalliance", 7226, 8);
        //Find the Underflame --Frozen Pyromancer--
        Story.mapItemQuest(7452, "darkalliance", 7227, 1);
        //Gather some Underlava
        Story.killQuest(7453, "darkalliance", "Underlava");
        //Gather Sulfur
        Story.killQuest(7454, "darkalliance", "Underworld Imp");
        //Coal for Flame
        Story.mapItemQuest(7455, "darkalliance", 7228, 6);
        //Defeat the Underflame Guardian
        Story.killQuest(7456, "darkalliance", "Underflame Guardian");
        //Take the Flame
        Story.mapItemQuest(7457, "darkalliance", 7229, 1);
        //Remove the Shadows --dage--
        Story.killQuest(7458, "darkalliance", "Shadow");
        //Burn the Shadows
        Story.mapItemQuest(7459, "darkalliance", 7230, 1);
        //Defeat Shadow Nulgath
        Story.killQuest(7460, "darkalliance", "ShadowFlame Nulgath");
    }

    innerShadows(): void {
        if (Core.isCompletedBefore(7472))
            return;

        this.darkAlliance();
        Story.preLoad(this.innerShadows);

        //Gather the Shadows (7461)
        Story.killQuest(7461, "innershadows", "Infected Minion");

        //Dusty Boost (7462)
        if (!Story.questProgression(7462)) {
            Core.ensureAccept(7462);
            Core.getMapItem(7271, 1, "innershadows");
            Core.huntMonster("innershadows", "Infected Minion", "Dry Bones", 8);
            Core.ensureComplete(7462);
        }

        //Defeat my Minions (7463)
        Story.killQuest(7463, "innershadows", "Infected Minion");

        //Pass the Portal (7464)
        Story.mapItemQuest(7464, "innershadows", 7272);

        //We Need to See (7465)
        Story.mapItemQuest(7465, "innershadows", 7273, 8);

        //Stop the Crows (7466)
        Story.killQuest(7466, "innershadows", "Shadowcrow");

        //Convert the Shadowscythe (7467)
        Story.killQuest(7467, "innershadows", "Shadow ShadowScythe");

        //Feed the Doomblade (7468)
        Story.killQuest(7468, "innershadows", "Infected Minion");

        //Speak to the Ghosts (7469)
        Story.mapItemQuest(7469, "innershadows", 7274, 5);

        //Gather Spell Reagents (7470)
        if (!Story.questProgression(7470)) {
            Core.ensureAccept(7470);
            Core.huntMonster("innershadows", "ShadowSpitter", "Blackened Herbs", 7);
            Core.huntMonster("innershadows", "Shadowcrow", "Shadowy Feathers", 7);
            Core.ensureComplete(7470);
        }

        //Follow the Trail (7471)
        Story.mapItemQuest(7471, "innershadows", 7275, 5);

        //Defeat Krahen (7472)
        Story.killQuest(7472, "innershadows", "Krahen");
    }

    tyndarius(WarTrainingMerge = false): void {
        if (Core.isCompletedBefore(8243))
            return;

        this.innerShadows();

        //War Medals
        Story.killQuest(8125, "fireplanewar", "Shadowflame Soldier");

        // Mega War Medals
        Story.killQuest(8126, "fireplanewar", "Shadowflame Soldier");

        // Shadowflame Un - slaught
        Story.killQuest(8127, "fireplanewar", "Shadefire Onslaught");

        // Soldier On
        Story.killQuest(8128, "fireplanewar", "Shadowflame Soldier");

        // Fanning the Flames
        if (!Story.questProgression(8129)) {
            Core.ensureAccept(8129);
            Core.getMapItem(8523, 5, "fireplanewar");
            Core.huntMonster("fireplanewar", "Shadowflame Soldier", "Battles Turned", 5);
            Core.ensureComplete(8129);
        }

        // Trailblazer
        Story.killQuest(8130, "fireplanewar", "Shadefire Onslaught");

        // ShadowClaw
        Story.killQuest(8131, "fireplanewar", "ShadowClaw");

        // In Their Element
        Story.killQuest(8132, "fireplanewar", "Shadefire Elemental");

        // Cure the Fire
        if (!Story.questProgression(8133)) {
            Core.ensureAccept(8133);
            Core.getMapItem(8524, 5, "fireplanewar");
            Core.huntMonster("fireplanewar", "Living Shadowflame", "Shadowflame Removed", 5);
            Core.ensureComplete(8133);
        }

        // Human Torch
        Story.killQuest(8134, "fireplanewar", "Living Shadowflame");

        // ShadowFlame Phedra
        Story.killQuest(8135, "fireplanewar", "ShadowFlame Phedra");

        // Gather Fuel
        Story.killQuest(8136, "shadowfireplane", "Living Shadowflame");

        // Attune to Play
        Story.mapItemQuest(8137, "shadowfireplane", 8544, 5);

        // Sparks Will Fly
        Story.killQuest(8138, "shadowfireplane", "Onslaught Knight");

        // Awaken Lady Fiamme
        Story.mapItemQuest(8139, "shadowfireplane", 8542);
        Core.sleep(5000);

        // Destroy the Barrier
        if (!Story.questProgression(8140)) {
            Core.join("shadowfireplane", "r6", "Left"); // for incase u start here
            Core.ensureAccept(8140);
            Core.getMapItem(8543);
            Core.killMonster("shadowfireplane", "r6", "Left", "Shadow Wing", "Shadow Flamewing Defeated", 2);
            Core.killMonster("shadowfireplane", "r6", "Left", "Shadowfire Summoner", "Shadowfire Summoner Defeated", 1);
            Core.ensureComplete(8140);
        }

        // Blaze a Path
        if (!Story.questProgression(8141)) {
            Core.ensureAccept(8141);
            Core.huntMonster("shadowfireplane", "Shadowfire Corporal", "Shadowfire Corporal Defeated", 5);
            Core.huntMonster("shadowfireplane", "Onslaught Knight", "Onslaught Knight Defeated", 5);
            Core.ensureComplete(8141);
        }

        // Into the Tiger's Den
        Story.killQuest(8142, "shadowfireplane", "Shadowfire Tiger");

        // One Final Push
        Story.killQuest(8143, "shadowfireplane", "Shadowfire Corporal");

        // Defeat Elius
        Story.killQuest(8144, "shadowfireplane", "Elius");

        // Elementary Defense
        Story.killQuest(8179, "fireinvasion", "Onslaught Knight");

        // Tiger Burning Bright
        if (!Story.questProgression(8180)) {
            Core.ensureAccept(8180);
            Core.getMapItem(8728, 3, "fireinvasion");
            Core.huntMonster("fireinvasion", "Shadowfire Tiger", "Tiger Slain", 10);
            Core.ensureComplete(8180);
        }

        // Crush the Cavalry
        Story.killQuest(8181, "fireinvasion", "Shadefire Cavalry");

        // Capture the Corporals
        Story.killQuest(8182, "fireinvasion", "Shadowfire Corporal");

        // Light up the Night
        if (!Story.questProgression(8183)) {
            Core.ensureAccept(8183);
            Core.getMapItem(8729, 6, "fireinvasion");
            Core.huntMonster("fireinvasion", "Shadefire Elemental", "Shadefire Elemental Slain", 7);
            Core.ensureComplete(8183);
        }

        // Major Malfunction
        Story.killQuest(8184, "fireinvasion", "Shadefire Major");

        // Darkness in Swordhaven
        if (!Story.questProgression(8185)) {
            Core.ensureAccept(8185);
            Core.killMonster("fireinvasion", "r8", "Top", "Shadefire Elemental", "Elemental Slain", 7);
            Core.killMonster("fireinvasion", "r7", "Top", "Shadowfire Tiger", "Tiger Slain", 7);
            Core.ensureComplete(8185);
        }

        // Extinguish the Flames
        Story.killQuest(8186, "fireinvasion", "Living Shadowflame");

        // ShadeFire Colonel Clash
        Story.killQuest(8187, "fireinvasion", "Shadefire Colonel");

        // Defense of Embersea
        Story.killQuest(8188, "fireinvasion", "Living Shadowflame");

        // A Chilling Conflict
        Story.killQuest(8189, "fireinvasion", "Onslaught Knight");

        // General Dismay
        Story.killQuest(8190, "fireinvasion", "Shadefire General");

        //A Fallen Friend
        Story.killQuest(8191, "fireinvasion", "Shadowflame Kyron");

        // Fire Fighting
        if (!Story.questProgression(8192)) {
            Core.ensureAccept(8192);
            Core.huntMonster("fireinvasion", "Living Shadowflame", "ShadowFlame Tag", 15);
            Core.huntMonster("fireinvasion", "Shadefire Cavalry", "Corrupted Badge", 3);
            Core.ensureComplete(8192);
        }

        // Shadefires of War
        Story.killQuest(8193, "wartraining", "Simulated Shadefire");

        // Extinguish the Shadowflame
        Story.mapItemQuest(8194, "wartraining", 8746, 4);

        // A Major Pain in the Neck
        Story.killQuest(8195, "wartraining", "Simulated Major");

        // The Envoy of Fire
        Story.killQuest(8196, "wartraining", "Simulated Elius");

        // Putting Out Small Fires
        Story.killQuest(8197, "wartraining", "Simulated Fire");

        // Element - ary
        Story.killQuest(8198, "wartraining", "Simulated Elemental");

        // A Dragonslayer's Past
        Story.killQuest(8199, "wartraining", "Simulated Fire");

        // The Champion of Fire
        Story.killQuest(8200, "wartraining", "Fire Champion");

        // March of the Warfury
        Story.killQuest(8201, "wartraining", "Warfury Soldier");

        // Elite It to Beat It
        Story.killQuest(8202, "wartraining", "Warfury Elite");

        //The Goddess of War
        Story.killQuest(8203, "wartraining", "Varga");
        if (WarTrainingMerge)
            return;

        // Warfury Training
        Story.killQuest(8204, "wartraining", "Warfury Soldier");

        //Colonel Vanguard 8233
        Story.killQuest(8233, "fireavatar", "Shadefire Colonel");

        //In a Major Way 8234
        Story.killQuest(8234, "fireavatar", "Shadefire Major");

        //Elemental Backup 8235
        Story.mapItemQuest(8235, "fireavatar", 8859, 4);

        //The Cavalry's Here 8236
        Story.killQuest(8236, "fireavatar", "Shadefire Cavalry");

        //Path of Flame 8237
        Story.killQuest(8237, "fireavatar", "Shadefire Colonel");

        //The Envoy of Fire 8238
        Story.killQuest(8238, "fireavatar", "Elius");

        //Living in Shadowflame 8239
        Story.killQuest(8239, "fireavatar", "Living Shadowflame");

        //Elemental of Surprise 8240
        Story.killQuest(8240, "fireavatar", "Shadefire Elemental");

        //Well That’s New 8241
        if (!Story.questProgression(8241)) {
            Core.ensureAccept(8241);
            Core.killMonster("fireavatar", "r8", "Left", "Shadow Lava", "Shadow Lava Defeated", 8);
            Core.ensureComplete(8241);
        }

        //Thermal Energy 8242
        if (!Story.questProgression(8242)) {
            Core.ensureAccept(8242);
            Core.killMonster("fireavatar", "r7", "Left", "Living Shadowflame", "Power Restored", 10);
            Core.ensureComplete(8242);
        }

        //Avatar of Fire 8243
        if (!Story.questProgression(8243)) {
            Core.ensureAccept(8243);
            Core.huntMonster("fireavatar", "Fire Orb", "Fire Orb Defeated", 2);
            Core.huntMonster("fireavatar", "Avatar Tyndarius", "Avatar Tyndarius Defeated", 1);
            Core.ensureComplete(8243);
        }
    }

    ruinedCrown(): void {
        if (Core.isCompletedBefore(8787))
            return;

        // 8778 Mental Damage Sponge
        Story.mapItemQuest(8778, "ruinedcrown", [ 10380, 10382, 10383 ]);

        // 8779 Scraping the Barrel
        if (!Story.questProgression(8779)) {
            Core.ensureAccept(8779);
            Core.huntMonster("ruinedcrown", "Mana-Burdened Knight", "Busted Helmet", 7);
            Core.huntMonster("ruinedcrown", "Mana-Burdened Minion", "Brittle Bone", 7);
            Core.ensureComplete(8779);
        }

        // 8780 Fractals
        Story.mapItemQuest(8780, "ruinedcrown", 10384, 6);

        // 8781 Blind Retaliation
        Story.killQuest(8781, "ruinedcrown", "Mana-Burdened Mage");

        // 8782 Deafening Silence
        if (!Story.questProgression(8782)) {
            Core.ensureAccept(8782);
            Core.huntMonster("ruinedcrown", "Mana-Burdened Knight", "Swordhaven Crest", 7);
            Core.huntMonster("ruinedcrown", "Mana-Burdened Minion", "Shadowscythe Crest", 7);
            Core.ensureComplete(8782);
        }

        // 8784 Stilled Mind (Yes 8784 before 8783)
        Story.mapItemQuest(8784, "ruinedcrown", 10385, 6);

        // 8783 Volatile Nature
        Story.killQuest(8783, "ruinedcrown", "Frenzied Mana");

        // 8785 Heartache
        Story.killQuest(8785, "ruinedcrown", "Mana-Burdened Mage");

        // 8786 Clouded Vision
        if (!Story.questProgression(8786)) {
            Core.ensureAccept(8786);
            Core.getMapItem(10386, 1, "ruinedcrown");
            Core.huntMonster("ruinedcrown", "Frenzied Mana", "Frienzied Mana Cleared", 6);
            Core.ensureComplete(8786);
        }

        // 8787 Guilt Complex
        Story.killQuest(8787, "ruinedcrown", "Calamitous Warlic");

        // 8788 Em-pathetic Connection (Merge Shop Quest)
        Core.ensureAccept(8788);

        Core.equipClass(ClassType.Farm);
        Core.huntMonster("ruinedcrown", "Frenzied Mana", "Mana Residue", 8);
        Core.huntMonster(`ruinedcrown`, "Mana-Burdened Mage", "Mage's Blood Sample", 8);

        Core.equipClass(ClassType.Solo);
        Core.huntMonster(`ruinedcrown`, "Calamitous Warlic", "Warlic's Favor");
        Core.ensureComplete(8788);
    }

    timekeep(): void {
        if (Core.isCompletedBefore(8813))
            return;

        this.ruinedCrown();

        // 8803|Mood Pendulum
        Story.mapItemQuest(8803, "Timekeep", [ 10455, 10456, 10457 ]);

        // 8804|Bug Issue
        Story.killQuest(8804, "Timekeep", "Decaying Locust");

        // 8805|Advanced Age
        Story.killQuest(8805, "Timekeep", "Distorted Imp");

        // 8806|One Singularity
        Story.mapItemQuest(8806, "Timekeep", 10458, 6);

        // 8807|Canaries on Edge
        Story.killQuest(8807, "Timekeep", "Mana-Burdened Mage");

        // 8808|Caution! Wet Floor
        Story.mapItemQuest(8808, "Timekeep", [ 10459, 10460 ]);

        // 8809|Reflection in the Puddle
        Story.killQuest(8809, "Timekeep", "Mumbler");

        // 8810|Meniscus Point
        if (!Story.questProgression(8810)) {
            Core.ensureAccept(8810);
            Core.huntMonster("timekeep", "Distorted Imp", "Imp Bones", 7);
            Core.huntMonster("timekeep", "Mana-Burdened Mage", "Mage Quelled", 7);
            Core.ensureComplete(8810);
        }

        // 8811|Distractions
        if (!Story.questProgression(8811)) {
            Core.ensureAccept(8811);
            Core.huntMonster("timekeep", "Mumbler", "Mumbler Crushed", 7);
            Core.huntMonster("timekeep", "Decaying Locust", "Locust Smashed", 7);
            Core.ensureComplete(8811);
        }

        // 8812|Dog Bites Back
        Story.killQuest(8812, "Timekeep", "Mal-formed Gar");

        // 8813|Janitorial Duties
        if (!Story.questProgression(8813)) {
            Core.ensureAccept(8813);
            Core.huntMonster("timekeep", "Mal-formed Gar", "Gar's Resignation Letter", 1);
            Core.huntMonster("timekeep", "Mumbler", "Mumbler Drool", 8);
            Core.huntMonster("timekeep", "Decaying Locust", "Locust Wings", 8);
            Core.ensureComplete(8813);
        }
    }

    timestreamWar(): void {
        if (Core.isCompletedBefore(8819))
            return;

        this.timekeep();

        // 8814|Timestream Medals (dont need to do the mega metals)
        if (!bot.quests.isUnlocked(8816)) {
            Core.ensureAccept(8814);
            Core.huntMonster("Streamwar", "Decaying Locust", "Timestream Medal", 5);
            Core.ensureComplete(8814);
        }

        // 8816|Teary Components
        Story.killQuest(8816, "Streamwar", "Mumbler");

        // 8817|Proportional Retaliation
        Story.killQuest(8817, "Streamwar", "Decaying Locust");

        //Growing Pains (8818)
        Story.killQuest(8818, "streamwar", "False Wyvern");

        //Middle Child (8819)
        Story.killQuest(8819, "streamwar", "Second Speaker");
    }

    deadLines(): void {
        if (Core.isCompletedBefore(8868))
            return;

        this.timestreamWar();

        Story.preLoad(this.deadLines);

        //Falling Apart 8859
        Story.mapItemQuest(8859, "DeadLines", 10601, 6);

        //Baby Steps 8860
        if (!Story.questProgression(8860)) {
            Core.ensureAccept(8860);
            Core.getMapItem(10602, 1, "DeadLines");
            Core.huntMonster("deadlines", "Frenzied Mana", "Mana Dispelled", 6);
            Core.ensureComplete(8860);
        }

        //Eternal Flame 8861
        if (!Story.questProgression(8861)) {
            Core.ensureAccept(8861);
            Core.getMapItems([ [10603, 1], [10604, 1] ], "DeadLines");
            Core.huntMonster("deadlines", "Shadowfall Warrior", "Shadowfall Helm", 7);
            Core.ensureComplete(8861);
        }

        //The Wolf Cries 8862
        if (!Story.questProgression(8862)) {
            Core.ensureAccept(8862);
            Core.getMapItems([ [10605, 1], [10606, 1] ], "DeadLines");
            Core.huntMonster("deadlines", "Swordhaven Knight", "Knight's Crest", 7);
            Core.ensureComplete(8862);
        }

        //Breaking Down 8863
        if (!Story.questProgression(8863)) {
            Core.ensureAccept(8863);
            Core.getMapItems([ [10607, 1], [10608, 1] ], "DeadLines");
            Core.huntMonster("deadlines", "Shadowfall Warrior", "Shadowfall Bones", 7);
            Core.ensureComplete(8863);
        }

        //Growth Through Hardship 8864
        if (!Story.questProgression(8864)) {
            Core.ensureAccept(8864);
            Core.getMapItems([ [10609, 1], [10610, 1] ], "DeadLines");
            Core.huntMonster("deadlines", "Swordhaven Knight", "Knight's Armaments", 7);
            Core.ensureComplete(8864);
        }

        //Arteries 8865
        if (!Story.questProgression(8865)) {
            Core.ensureAccept(8865);
            Core.getMapItems([ [10611, 1], [10612, 1] ], "DeadLines");
            Core.huntMonster("deadlines", "Frenzied Mana", "Mana Dissipated", 6);
            Core.ensureComplete(8865);
        }

        //Paranoiac 8866
        if (!Story.questProgression(8866)) {
            Core.ensureAccept(8866);
            Core.getMapItem(10613, 1, "DeadLines");
            Core.huntMonster("deadlines", "Chaos Mage", "Mage Hoods", 6);
            Core.ensureComplete(8866);
        }

        //A Hard Swerve 8867
        if (!Story.questProgression(8867)) {
            Core.ensureAccept(8867);
            Core.getMapItem(10614, 1, "DeadLines");
            Core.huntMonster("deadlines", "Frenzied Mana", "Mana Cleared", 6);
            Core.ensureComplete(8867);
        }

        //Nigh Invincible 8868
        Core.equipClass(ClassType.Solo);
        Story.killQuest(8868, "DeadLines", "Eternal Dragon");
    }

    shadowFlame(): void {
        if (Core.isCompletedBefore(8965))
            return;

        this.deadLines();

        Story.preLoad(this.shadowFlame);

        // Lies in Peace 8956
        Core.equipClass(ClassType.Solo);
        Story.mapItemQuest(8956, "worldscore", 10877);
        Story.killQuest(8956, "worldscore", "False Wyvern");

        // There Was an Attempt 8957
        if (!Story.questProgression(8957)) {
            Core.ensureAccept(8957);
            Core.getMapItem(10878, 5, "worldscore");
            Core.huntMonster("worldscore", "Elemental Attempt", "Pulsating Mana Residue", 12);
            Core.ensureComplete(8957);
        }

        // Scared Straight 8958
        if (!Story.questProgression(8958)) {
            Core.ensureAccept(8958);
            Core.getMapItem(10879, 1, "worldscore");
            Core.huntMonster("worldscore", "Infernal Mockery", "Waxen Horns", 12);
            Core.ensureComplete(8958);
        }

        // Making Waves 8959
        if (!Story.questProgression(8959)) {
            Core.ensureAccept(8959);
            Core.getMapItem(10880, 4, "worldscore");
            Core.huntMonster("worldscore", "Infernal Mockery", "Infernal Shoved", 10);
            Core.ensureComplete(8959);
        }

        // In Circles 8960
        if (!Story.questProgression(8960)) {
            Core.ensureAccept(8960);
            Core.getMapItem(10881, 1, "worldscore");
            Core.huntMonster("worldscore", "Elemental Attempt", "Vanquished Elemental", 14);
            Core.ensureComplete(8960);
        }

        // Nameless Remnants 8961
        Story.mapItemQuest(8961, "worldscore", 10882, 4);

        // Dreams Unborn 8962
        Story.killQuest(8962, "worldscore", "Crystalized Mana");

        // Innocent Crafts 8963
        if (!Story.questProgression(8963)) {
            Core.ensureAccept(8963);
            Core.getMapItem(10883, 1, "worldscore");
            Core.huntMonster("worldscore", "Infernal Mockery", "Rubbery Wings", 14);
            Core.ensureComplete(8963);
        }

        // Cold Bedrock 8964
        if (!Story.questProgression(8964)) {
            Core.ensureAccept(8964);
            Core.getMapItem(10884, 1, "worldscore");
            Core.huntMonster("worldscore", "Elemental Attempt", "Sterile Mana", 16);
            Core.ensureComplete(8964);
        }

        // Rippling Heartbeat 8965
        Story.killQuest(8965, "worldscore", "Mask of Tranquility");
    }

    manaCradle(): void {
        if (Core.isCompletedBefore(9126))
            return;

        this.shadowFlame();

        Story.preLoad(this.manaCradle);

        Core.equipClass(ClassType.Solo);

        // Stunted Growth 9116
        Story.killQuest(9116, "manacradle", "Elemental Attempt");

        // Tossing and Turning 9117
        Story.killQuest(9117, "manacradle", "Crystalized Mana");

        // Dark Intruder 9118
        Story.mapItemQuest(9118, "manacradle", 11268, 3);
        Story.mapItemQuest(9118, "manacradle", 11269, 3);

        // Doggy Paddle 9119
        if (!Story.questProgression(9119)) {
            Core.ensureAccept(9119);
            Core.huntMonster("manacradle", "Crystalized Mana", "Crystal Shard", 8);
            Core.huntMonster("manacradle", "Elemental Attempt", "Elemental Drops", 8);
            Core.ensureComplete(9119);
        }

        // Amethyst Memories 9120
        Story.killQuest(9120, "manacradle", "Dark Tainted Mana");

        // Crawling Darkness 9121
        Story.killQuest(9121, "manacradle", "Darkness Elemental");

        // Raised Veins 9122
        Story.mapItemQuest(9122, "manacradle", 11270, 5);

        // In The Eye 9123
        if (!Story.questProgression(9123)) {
            Core.ensureAccept(9123);
            Core.getMapItem(11271, 1, "manacradle");
            Core.huntMonster("manacradle", "Darkness Elemental", "Dark Shard", 10);
            Core.huntMonster("manacradle", "Dark Tainted Mana", "Dark Residue", 10);
            Core.ensureComplete(9123);
        }


        // If Dodge Class was found, kill Malgor & The Mainyu with it, else use what you have on.
        Core.equipClass(ClassType.Dodge);

        // Cognitive Dissonance 9124
        Story.killQuest(9124, "manacradle", "Malgor");

        // Your Hero 9125
        Story.killQuest(9125, "manacradle", "The Mainyu");

        Core.toBank(...this.mainyuDrops.concat(this.malgorDrops));
        //#region GroupBoss Quest
        // These 2 arent needed.
        // Once Upon Another Time 9126
        // Build Malgor's Armor Set 9127
        //#endregion
        Adv.gearStore(true, { enhAfter: true });
    }
}
