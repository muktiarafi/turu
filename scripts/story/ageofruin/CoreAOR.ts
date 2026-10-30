/*
name: null
description: null
tags: null
*/
// Port of Scripts/Story/AgeofRuin/CoreAOR.cs.

import { bot } from "turu";
import { CapeSpecial, CoreAdvanced, EnhancementType, HelmSpecial, WeaponSpecial } from "../../core/CoreAdvanced";
import { ClassType, Core, randomInt, rethrowIfStopped } from "../../core/CoreBots";
import { CoreStory } from "../../core/CoreStory";
import { CoreSoW } from "../shadowsofwar/CoreSoW";

const Adv = new CoreAdvanced();
const Story = new CoreStory();
const SoW = new CoreSoW();

export class CoreAOR {
    private _voiceChargeDetected: boolean = false;

    doAll(): void {
        this.terminaTemple(true, true);
        this.ashrayVillage(false, false);
        this.sunlightZone(false, false);
        this.twilightZone(false, false);
        this.yulgarAria(false, false);
        this.midnightZone(false, false);
        this.abyssalZone(false, false);
        this.deepWater(false, false);
        this.seaVoice();
        this.balemorale(false, false);
        this.castleeblana(false, false);
        this.loughshine(false, false);
        this.naoiseGrave(false, false);
        this.liaTaraHill(false, false);
        this.castleGaheris(false, false);
        this.coldThunder();
        this.thelimaCity(false, false);
        this.mountMaleno(false, false);
        this.sanctuaryAiwass(false, false);
        this.forgeAlbedo(false, false);
        this.fortLuma(false, false);
        this.warWickForest(false, false);
        this.magnumOpus(false, false);
        this.rubedoPeak(false, false);
        this.forgeCitrinitas(false, false);
    }

    terminaTemple(seaVoice = false, coldThunder = false): void {
        if (
            Core.isCompletedBefore(
                coldThunder ? 9851
                : seaVoice ? 9351
                : 10438)
        )
            return;

        SoW.manaCradle();
        Story.preLoad(this.terminaTemple);

        // Familiar Faces (9213)
        if (!Story.questProgression(9213)) {
            Core.ensureAccept(9213);
            Core.getMapItems([ [11625, 1], [11626, 1], [11627, 1] ], "terminatemple");
            Core.logger("resetting map data ( mobs can bug out and become unattackable)");
            Core.join("whitemap-100000");
            Core.join("terminatemple");
            Core.huntMonster("terminatemple", "Termina Defender", "Defender Trained", 7);
            Core.ensureComplete(9213);
        }

        // Loaded Resume (9214)
        if (!Story.questProgression(9214)) {
            Core.ensureAccept(9214);
            Core.getMapItems([ [11628, 1], [11629, 1], [11630, 1] ], "terminatemple");
            Core.huntMonster("terminatemple", "Clandestine Guard", "Guard Fought", 7);
            Core.ensureComplete(9214);
        }

        if (!seaVoice && !coldThunder)
            return;

        // If seaVoice or coldThunder & quest isnt unlocked
        if (seaVoice || coldThunder && !Core.isCompletedBefore(9348))
            this.seaVoice();

        // Mopping Up (9351)
        Story.mapItemQuest(9351, "terminatemple", [ 12050, 12051 ]);

        if (seaVoice && !coldThunder)
            return;

        if (coldThunder)
            this.coldThunder();

        // Tell-Tale Heart (9851)
        Story.mapItemQuest(9851, "terminatemple", [ 13541, 13542 ]);
    }

    ashrayVillage(seaVoice = false, coldThunder = false): void {
        if (Core.isCompletedBefore(9234))
            return;

        this.terminaTemple(seaVoice, coldThunder);

        // Big ol' Eyes (9225)
        Story.killQuest(9225, "ashray", "Kitefin Shark Bait");

        // Angry Angler (9226)
        if (!Story.questProgression(9226)) {
            Core.ensureAccept(9226);
            Core.getMapItems([ [11663, 1], [11664, 1] ], "ashray");
            Core.huntMonster("ashray", "Ashray Fisherman", "Fisherman's Trident", 4);
            Core.ensureComplete(9226);
        }

        // Slimy Scavenger (9227)
        Story.killQuest(9227, "ashray", "Ghostly Eel");

        // Troubled Waters (9228)
        if (!Story.questProgression(9228)) {
            Core.ensureAccept(9228);
            Core.getMapItem(11665, 1, "ashray");
            Core.huntMonster("ashray", "Stagnant Water", "Elemental Cleared", 8);
            Core.ensureComplete(9228);
        }

        // Washed Ink (9229)
        if (!Story.questProgression(9229)) {
            Core.ensureAccept(9229);
            Core.getMapItem(11666, 1, "ashray");
            Core.huntMonster("ashray", "Ashray Fisherman", "Fisherman Repelled", 8);
            Core.ensureComplete(9229);
        }

        // Fishy Hospitality (9230)
        Story.killQuest(9230, "ashray", "Kitefin Shark Bait");

        // Doctoring Papers (9231)
        if (!Story.questProgression(9231)) {
            Core.ensureAccept(9231);
            Core.getMapItem(11667, 1, "ashray");
            Core.huntMonster("ashray", "Ghostly Eel", "Excorsized Eel", 8);
            Core.ensureComplete(9231);
        }

        // Psychic Pollution (9232)
        if (!Story.questProgression(9232)) {
            Core.ensureAccept(9232);
            Core.getMapItem(11668, 1, "ashray");
            Core.huntMonster("ashray", "Stagnant Water", "Water Samples", 3);
            Core.ensureComplete(9232);
        }

        // Duck Dive (9233)
        Story.mapItemQuest(9233, "ashray", [ 11669, 11670 ]);

        // Faces in the Foam (9234)
        Story.killQuest(9234, "ashray", "Seafoam Elemental");
    }

    sunlightZone(seaVoice = false, coldThunder = false): void {
        if (Core.isCompletedBefore(9251))
            return;

        this.ashrayVillage(seaVoice, coldThunder);

        // Detergent Shortage (9242)
        Story.killQuest(9242, "sunlightzone", "Blighted Water");

        // Ghost in the Machine (9243)
        Story.killQuest(9243, "sunlightzone", "Spectral Jellyfish");

        // Efficient Division (9244)
        if (!Story.questProgression(9244)) {
            Core.ensureAccept(9244);
            Core.getMapItems([ [11705, 1], [11706, 1] ], "sunlightzone");
            Core.huntMonster("sunlightzone", "Blighted Water", "Mopped Water", 6);
            Core.ensureComplete(9244);
        }

        // Tech Illiterate (9245)
        if (!Story.questProgression(9245)) {
            Core.ensureAccept(9245);
            Core.getMapItem(11707, 3, "sunlightzone");
            Core.huntMonster("sunlightzone", "Spectral Jellyfish", "Jelly Filling", 6);
            Core.ensureComplete(9245);
        }

        // Plugging Leaks (9246)
        if (!Story.questProgression(9246)) {
            Core.ensureAccept(9246);
            Core.huntMonster("sunlightzone", "Spectral Jellyfish", "Stinging Tentacles", 6);
            Core.huntMonster("sunlightzone", "Blighted Water", "Grainy Water", 6);
            Core.ensureComplete(9246);
        }

        // Shared History (9247)
        Story.mapItemQuest(9247, "sunlightzone", [ 11708, 11709, 11710 ]);

        // Flat Scares (9248)
        if (!Story.questProgression(9248)) {
            Core.ensureAccept(9248);
            Core.getMapItem(11711, 1, "sunlightzone");
            Core.huntMonster("sunlightzone", "Astravian Illusion", "Astravian Dispelled", 10);
            Core.ensureComplete(9248);
        }

        // Fishy Bully (9249)
        if (!Story.questProgression(9249)) {
            Core.ensureAccept(9249);
            Core.getMapItem(11712, 5, "sunlightzone");
            Core.huntMonster("sunlightzone", "Infernal Illusion", "Infernal Dispelled", 10);
            Core.ensureComplete(9249);
        }

        // Faint Howls (9250)
        if (!Story.questProgression(9250)) {
            Core.ensureAccept(9250);
            Core.getMapItem(11713, 1, "sunlightzone");
            Core.huntMonster("sunlightzone", "Seraphic Illusion", "Seraph Dispelled", 10);
            Core.ensureComplete(9250);
        }

        // Down the Digestive Tract (9251)
        Story.killQuest(9251, "sunlightzone", "Marine Snow");
    }

    twilightZone(seaVoice = false, coldThunder = false): void {
        if (Core.isCompletedBefore(9268))
            return;

        this.sunlightZone(seaVoice, coldThunder);

        // Marshmallows With Bite (9258)
        Story.killQuest(9258, "twilightzone", "Whale Louse");

        // Meaty Cold Spaghetti (9259)
        Story.killQuest(9259, "twilightzone", "Polymelia Lamprey");

        // Songs in the Seams (9260)
        Story.mapItemQuest(9260, "twilightzone", 11749);
        Story.mapItemQuest(9260, "twilightzone", 11750, 4);

        // Parched Throats (9261)
        if (!Story.questProgression(9261)) {
            Core.ensureAccept(9261);
            Core.huntMonster("twilightzone", "Whale Louse", "Louse Antennae", 6);
            Core.huntMonster("twilightzone", "Polymelia Lamprey", "Lamprey Tooth", 6);
            Core.ensureComplete(9261);
        }

        // Morning Stretches (9262)
        Story.mapItemQuest(9262, "twilightzone", [ 11751, 11752 ]);

        // Natural Empathy (9263)
        Story.killQuest(9263, "twilightzone", "Decay Spirit");

        // Comfort Blanket of Snow (9264)
        Story.killQuest(9264, "twilightzone", "Ice Guardian");

        // Whale Watching (9265)
        Story.mapItemQuest(9265, "twilightzone", [ 11753, 11754, 11755 ]);

        // Exhausted Spirits (9266)
        if (!Story.questProgression(9266)) {
            Core.ensureAccept(9266);
            Core.huntMonster("twilightzone", "Decay Spirit", "Spirit Restrained", 7);
            Core.huntMonster("twilightzone", "Ice Guardian", "Guardian Restrained", 7);
            Core.ensureComplete(9266);
        }

        // Singing to Whales (9267)
        if (!Story.questProgression(9267)) {
            Core.equipClass(ClassType.Solo);
            Core.ensureAccept(9267);
            Core.huntMonster("twilightzone", "Leviathan", "Leviathan Fought");
            Core.ensureComplete(9267);
        }

        // The Sea's Commitment (9268)
        Story.mapItemQuest(9268, "twilightzone", 11756);
    }

    yulgarAria(seaVoice = false, coldThunder = false): void {
        if (Core.isCompletedBefore(9274))
            return;

        this.twilightZone(seaVoice, coldThunder);

        // Octotree (9270)
        Story.killQuest(9270, "twilightzone", "Polymelia Lamprey");

        // Thirsty Roots (9271)
        Story.killQuest(9271, "sunlightzone", "Blighted Water");

        // Dollar Store Mogloween Costume (9272)
        if (!Story.questProgression(9272)) {
            Core.ensureAccept(9272);
            Core.huntMonster("sunlightzone", "Infernal Illusion", "Infernal Dispersed", 4);
            Core.huntMonster("sunlightzone", "Astravian Illusion", "Astravian Dispersed", 4);
            Core.ensureComplete(9272);
        }

        // Sea Snow Angels (9273)
        Story.killQuest(9273, "sunlightzone", "Marine Snow");

        // Ten Klicks (9274)
        if (!Story.questProgression(9274)) {
            Core.equipClass(ClassType.Solo);
            Core.ensureAccept(9274);
            Core.huntMonster("twilightzone", "Leviathan", "Leviathan's Tendril", 3);
            Core.ensureComplete(9274);
        }
    }

    midnightZone(seaVoice = false, coldThunder = false): void {
        if (Core.isCompletedBefore(9301))
            return;

        this.twilightZone(seaVoice, coldThunder);

        // Motivation Malady (9292)
        Story.mapItemQuest(9292, "midnightzone", [ 11842, 11843, 11844 ]);

        // Radical Renovation (9293)
        if (!Story.questProgression(9293)) {
            Core.ensureAccept(9293);
            Core.getMapItem(11845, 1, "midnightzone");
            Core.huntMonster("midnightzone", "Polymelia Lamprey", "Lamprey Limb", 8);
            Core.ensureComplete(9293);
        }

        // Graveyard Shift (9294)
        if (!Story.questProgression(9294)) {
            Core.ensureAccept(9294);
            Core.huntMonster("midnightzone", "Vowed Shadowslayer", "Slayer Subdued", 8);
            Core.huntMonster("midnightzone", "Vowed Shadowslayer", "Key Card", 1);
            Core.ensureComplete(9294);
        }

        // Educational Execution (9295)
        if (!Story.questProgression(9295)) {
            Core.ensureAccept(9295);
            Core.getMapItem(11846, 1, "midnightzone");
            Core.huntMonster("midnightzone", "Undead Prisoner", "Prisoner Subdued", 10);
            Core.ensureComplete(9295);
        }

        // Vows For Ignorance (9296)
        if (!Story.questProgression(9296)) {
            Core.ensureAccept(9296);
            Core.getMapItem(11847, 1, "midnightzone");
            Core.huntMonster("midnightzone", "Undead Prisoner", "Bone Sample", 6);
            Core.huntMonster("midnightzone", "Vowed Shadowslayer", "Slayer's Written Vow", 6);
            Core.ensureComplete(9296);
        }

        // Protein Shake (9297)
        if (!Story.questProgression(9297)) {
            Core.ensureAccept(9297);
            Core.getMapItem(11848, 3, "midnightzone");
            Core.huntMonster("midnightzone", "Shadow Viscera", "Viscera Sample", 6);
            Core.ensureComplete(9297);
        }

        // Duty Beyond Death (9298)
        Story.killQuest(9298, "midnightzone", "Venerated Wraith");

        // Designated Taunters (9299)
        if (!Story.questProgression(9299)) {
            Core.ensureAccept(9299);
            Core.huntMonster("midnightzone", "Shadow Viscera", "Shadow Captured", 7);
            Core.huntMonster("midnightzone", "Venerated Wraith", "Wraith Appeased", 7);
            Core.ensureComplete(9299);
        }

        // Beloved Simulacrum (9230)
        Story.mapItemQuest(9300, "midnightzone", 11849, 4);
        Story.mapItemQuest(9300, "midnightzone", 11850);

        // Roko's Royal Basilisk (9301)
        if (!Story.questProgression(9301)) {
            Core.equipClass(ClassType.Solo);
            Core.ensureAccept(9301);
            Core.huntMonster("midnightzone", "Sparagmos", "Sparagmos A.I. Defeated");
            Core.ensureComplete(9301);
        }
    }

    abyssalZone(seaVoice = false, coldThunder = false): void {
        if (Core.isCompletedBefore(9315))
            return;

        this.midnightZone(seaVoice, coldThunder);

        // Shark Kiting (9306)
        if (!Story.questProgression(9306)) {
            Core.ensureAccept(9306);
            Core.getMapItem(11914, 1, "abyssalzone");
            Core.huntMonster("abyssalzone", "Kitefin Shark Bait", "Shark Fin", 8);
            Core.ensureComplete(9306);
        }

        // Suckered Blockade (9307)
        Story.mapItemQuest(9307, "abyssalzone", 11893, 6);
        Story.mapItemQuest(9307, "abyssalzone", 11894);

        // Digestive Fluids (9308)
        Story.killQuest(9308, "abyssalzone", "Blighted Water");

        // Sodden Secrets (9309)
        if (!Story.questProgression(9309)) {
            Core.ensureAccept(9309);
            Core.getMapItem(11895, 3, "abyssalzone");
            Core.huntMonster("abyssalzone", "Shadow Viscera", "Shadow Sample", 7);
            Core.ensureComplete(9309);
        }

        // The Hidden Corpse (9310)
        if (!Story.questProgression(9310)) {
            Core.ensureAccept(9310);
            Core.huntMonster("abyssalzone", "Shadow Viscera", "Shadow Organs", 6);
            Core.huntMonster("abyssalzone", "Blighted Water", "Decayed Elemental Gem", 6);
            Core.ensureComplete(9310);
        }

        // Octo-Flake Fish Feed (9311)
        if (!Story.questProgression(9311)) {
            Core.ensureAccept(9311);
            Core.getMapItem(11896, 1, "abyssalzone");
            Core.huntMonster("abyssalzone", "Foam Scavenger", "Foamy Residue", 10);
            Core.ensureComplete(9311);
        }

        // Completely Surrounded (9312)
        Story.mapItemQuest(9312, "abyssalzone", 11897, 6);
        Story.mapItemQuest(9312, "abyssalzone", 11898);

        // Sea Salt Soap (9313)
        Story.killQuest(9313, "abyssalzone", "Necro Adipocere");

        // In the Grip of Justice (9314)
        if (!Story.questProgression(9314)) {
            Core.ensureAccept(9314);
            Core.getMapItem(11899, 1, "abyssalzone");
            Core.huntMonster("abyssalzone", "Necro Adipocere", "Necro Wax", 3);
            Core.huntMonster("abyssalzone", "Foam Scavenger", "Scavenger Tentacle", 7);
            Core.ensureComplete(9314);
        }

        // Together as One (9315)
        if (!Story.questProgression(9315)) {
            Core.equipClass(ClassType.Solo);
            Core.ensureAccept(9315);
            Core.huntMonster("abyssalzone", "The Ashray", "The Ashray Vanquished");
            Core.ensureComplete(9315);
        }
    }

    deepWater(panopticonMerge = false, seaVoice = false, coldThunder = false): void {
        if (Core.isCompletedBefore(9338))
            return;

        this.abyssalZone(seaVoice, coldThunder);

        // Unsung Heroes (9329)
        if (!Story.questProgression(9329)) {
            Core.ensureAccept(9329);
            Core.getMapItem(11975, 1, "trenchobserve");
            Core.huntMonster("trenchobserve", "Venerated Wraith", "Wraith I.D Card", 8);
            Core.ensureComplete(9329);
        }

        // Watertight Guarantee (9330)
        if (!Story.questProgression(9330)) {
            Core.ensureAccept(9330);
            Core.getMapItem(11976, undefined, "trenchobserve");
            Core.huntMonster("trenchobserve", "Seabase Turret", "Turret Screws", 8);
            Core.ensureComplete(9330);
        }

        // Core Electrolytes (9331)
        Story.mapItemQuest(9331, "trenchobserve", 11977, 4);

        // Guardian Spirits (9332)
        Story.killQuest(9332, "trenchobserve", "Venerated Wraith");
        if (panopticonMerge)
            return;

        // Enemy in Need (9333)
        if (!Story.questProgression(9333)) {
            Core.ensureAccept(9333);
            Core.getMapItem(11978, 4, "trenchobserve");
            Core.huntMonster("trenchobserve", "Seabase Turret", "Turret Disarmed", 6);
            Core.ensureComplete(9333);
        }

        // Here Lies Shadow (9334)
        Story.mapItemQuest(9334, "trenchobserve", [ 11979, 11981 ]);
        Story.mapItemQuest(9334, "trenchobserve", 11980, 2);

        // Nature's White Noise (9335)
        Story.killQuest(9335, "trenchobserve", "Sea Spirit");

        // Dreams Seep into Reality (9336)
        Story.killQuest(9336, "trenchobserve", "Necro Adipocere");

        // Hadal Havoc (9337)
        if (!Story.questProgression(9337)) {
            Core.ensureAccept(9337);
            Core.getMapItem(11982, 1, "trenchobserve");
            Core.huntMonster("trenchobserve", "Necro Adipocere", "Waxen Limb", 3);
            Core.huntMonster("trenchobserve", "Sea Spirit", "Spirit Stinger", 7);
            Core.ensureComplete(9337);
        }

        // See You on the Other Side (9338)
        if (!Story.questProgression(9338)) {
            Core.equipClass(ClassType.Solo);
            Core.ensureAccept(9338);
            Core.huntMonster("trenchobserve", "Lady Noelle", "Lady Noelle Defeated");
            Core.ensureComplete(9338);
        }
    }

    seaVoice(): void {
        if (Core.isCompletedBefore(9348))
            return;

        SoW.manaCradle();
        this.deepWater(false, false, false);
        Story.preLoad(this.seaVoice);
        Core.ensureAccept(9348);

        this.fightVoiceInTheSea("Voice in the Sea Defeated", undefined, true);

        Adv.gearStore(true, { enhAfter: true });
        Core.ensureComplete(9348);
        Core.sellItem("Vigil", undefined, { all: true });
    }

    balemorale(seaVoice = false, coldThunder = false): void {
        if (Core.isCompletedBefore(9729))
            return;

        this.terminaTemple(seaVoice, coldThunder);

        Core.equipClass(ClassType.Farm);

        // Estrangement (9719)
        Story.mapItemQuest(9719, "balemorale", 12933);

        // Searchlights (9720)
        Story.killQuest(9720, "balemorale", "Lightguard Paladin");

        // Queensmen (9721)
        Story.killQuest(9721, "balemorale", "Noble's Knight");

        // Cellar Secrets (9722)
        Story.mapItemQuest(9722, "balemorale", [ 13177, 13178 ]);

        // Chaotic Roots (9723)
        if (!Story.questProgression(9723)) {
            Core.ensureAccept(9723);
            Core.getMapItems([ [13179, 1], [13180, 1] ], "balemorale");
            Core.huntMonster("balemorale", "Chaos Spider", "Spider Legs", 128);
            Core.ensureComplete(9723);
        }

        // Eroding Era (9724)
        if (!Story.questProgression(9724)) {
            Core.ensureAccept(9724);
            Core.killMonster("balemorale", "r13", "Left", "*", "Chaotic Shard", 50);
            Core.ensureComplete(9724);
        }

        // Old Wolf (9725)
        Story.mapItemQuest(9725, "balemorale", 13181, 5);
        Story.mapItemQuest(9725, "balemorale", 13182);

        // Abandoned Cradle (9726)
        if (!Story.questProgression(9726)) {
            Core.ensureAccept(9726);
            Core.getMapItem(13183, 1, "balemorale");
            Core.huntMonster("balemorale", "Chaos Spider", "Chaotic Venom", 9);
            Core.huntMonster("balemorale", "Chaos Crystal", "Violet Core", 9);
            Core.ensureComplete(9726);
        }

        // Shockwaves (9727)
        Story.killQuest(9727, "balemorale", "Skye Warrior");

        // Sleight of Hand (9728)
        Story.mapItemQuest(9728, "balemorale", 13184, 7);
        Story.mapItemQuest(9728, "balemorale", 13185);

        // Double Fianchetto (9729)
        if (!Story.questProgression(9729)) {
            Core.equipClass(ClassType.Solo);
            Core.ensureAccept(9729);
            Core.huntMonster("balemorale", "Queen Victoria", "Queen Victoria Defeated");
            Core.ensureComplete(9729);
            Core.equipClass(ClassType.Farm);
        }
    }

    castleeblana(seaVoice = false, coldThunder = false): void {
        if (Core.isCompletedBefore(9741))
            return;

        this.balemorale(seaVoice, coldThunder);

        Core.equipClass(ClassType.Farm);

        // Skye's Raindrops 9732
        Story.killQuest(9732, "castleeblana", "Skye Warrior");

        // Doctor's Orders 9733
        Story.mapItemQuest(9733, "castleeblana", 13202, 5);
        Story.mapItemQuest(9733, "castleeblana", 13203);

        // Shockwave's Ripples 9734
        Story.killQuest(9734, "castleeblana", "Skye Executor");

        // Caretaker's Shadow 9735
        if (!Story.questProgression(9735)) {
            Core.ensureAccept(9735);
            Core.getMapItem(13204, 1, "castleeblana");
            Core.huntMonster("castleeblana", "Skye Warrior", "Warrior's Shockstone", 6);
            Core.huntMonster("castleeblana", "Skye Executor", "Executor's Shockstone", 9);
            Core.ensureComplete(9735);
        }

        // Harbinger's Tears 9736
        Story.killQuest(9736, "castleeblana", "Bananach Raven");

        // Spectre of Hunger 9737
        if (!Story.questProgression(9737)) {
            Core.ensureAccept(9737);
            Core.getMapItem(13205, 1, "castleeblana");
            Core.huntMonster("castleeblana", "Fear Gorta", "Gorta Rag", 10);
            Core.ensureComplete(9737);
        }

        // InnJustice 9738
        if (!Story.questProgression(9738)) {
            Core.ensureAccept(9738);
            Story.mapItemQuest(9738, "castleeblana", 13206);
            Core.huntMonster("castleeblana", "Fear Gorta", "Hunger Leaves", 9);
            Core.huntMonster("castleeblana", "Bananach Raven", "Electrified Feather", 9);
            Core.ensureComplete(9738);
        }
        // Find Shelter in... 9739
        Story.mapItemQuest(9739, "castleeblana", [ 13207, 13208 ]);

        // Heavy Handed 9740
        Story.killQuest(9740, "castleeblana", "Skye Warrior");

        // Miserable Monsoon 9741
        Core.equipClass(ClassType.Solo);
        Story.killQuest(9741, "castleeblana", "Warden Indradeep");
    }

    loughshine(seaVoice = false, coldThunder = false): void {
        if (Core.isCompletedBefore(9764))
            return;

        this.castleeblana(seaVoice, coldThunder);

        // Wicker Magic (9755)
        if (!Story.questProgression(9755)) {
            Core.ensureAccept(9755);
            Core.getMapItem(13273, 1, "loughshine");
            Core.huntMonster("loughshine", "Skye Cailleach", "Cailleach's Shockstone", 5);
            Core.ensureComplete(9755);
        }

        // Maigh Eo (9756)
        Story.killQuest(9756, "loughshine", "Scorched Elder Yew");

        // Resentment Harbour (9757)
        Story.mapItemQuest(9757, "loughshine", 13274, 5);
        Story.mapItemQuest(9757, "loughshine", 13275);

        // Wisened Yew (9758)
        if (!Story.questProgression(9758)) {
            Core.ensureAccept(9758);
            Core.huntMonster("loughshine", "Skye Cailleach", "Cailleach's Twine", 3);
            Core.huntMonster("loughshine", "Scorched Elder Yew", "Yew Aril", 3);
            Core.ensureComplete(9758);
        }

        // Dragonflare (9759)
        Story.mapItemQuest(9759, "loughshine", 13276);

        // Shock Scatter (9760)
        Story.killQuest(9760, "loughshine", "Skye Executor");

        // Grace of Three (9761)
        if (!Story.questProgression(9761)) {
            Core.ensureAccept(9761);
            Core.getMapItem(13277, 1, "loughshine");
            Core.huntMonster("loughshine", "Energy Elemental", "Electric Plasma", 12);
            Core.ensureComplete(9761);
        }

        // Iubhair (9762)
        Story.mapItemQuest(9762, "loughshine", 13278, 5);
        Story.mapItemQuest(9762, "loughshine", 13279);

        // Parting the Clouds (9763)
        if (!Story.questProgression(9763)) {
            Core.ensureAccept(9763);
            Core.getMapItem(13280, 1, "loughshine");
            Core.huntMonster("loughshine", "Skye Executor", "Executor's Initials", 8);
            Core.huntMonster("loughshine", "Energy Elemental", "Energy Core", 8);
            Core.ensureComplete(9763);
        }

        // Arrester (9764)
        if (!Story.questProgression(9764)) {
            Core.equipClass(ClassType.Solo);
            Story.killQuest(9764, "loughshine", "Warden Iseul");
            Core.equipClass(ClassType.Farm);
        }
    }

    naoiseGrave(seaVoice = false, coldThunder = false): void {
        if (Core.isCompletedBefore(9777))
            return;

        this.loughshine(seaVoice, coldThunder);

        // Till When? (9768)
        if (!Story.questProgression(9768)) {
            Core.ensureAccept(9768);
            Core.getMapItem(13296, 1, "naoisegrave");
            Core.huntMonster("naoisegrave", "Bananach Raven", "Corvus Feather", 10);
            Core.ensureComplete(9768);
        }

        // Energetic Feud (9769)
        if (!Story.questProgression(9769)) {
            Core.ensureAccept(9769);
            Core.getMapItem(13297, 1, "naoisegrave");
            Core.huntMonster("naoisegrave", "Energy Elemental", "Overdriven Electron", 10);
            Core.ensureComplete(9769);
        }

        // Glacial Seal (9774)
        Story.mapItemQuest(9774, "naoisegrave", [ 13298, 13299 ]);

        // The Cold Wind Guides (9770)
        if (!Story.questProgression(9770)) {
            Core.ensureAccept(9770);
            Core.getMapItem(13300, 1, "naoisegrave");
            Core.huntMonster("naoisegrave", "Energy Elemental", "Frozen Lightning", 6);
            Core.huntMonster("naoisegrave", "Bananach Raven", "Warm Raven Tear", 10);
            Core.ensureComplete(9770);
        }

        // Cold Callers (9771)
        Story.killQuest(9771, "naoisegrave", "Ice Guardian");

        // Destinies Lost (9772)
        Story.killQuest(9772, "naoisegrave", "Bone Dragonling");

        // Of The Sorrows (9773)
        Core.equipClass(ClassType.Solo);
        if (!Story.questProgression(9773)) {
            Core.ensureAccept(9773);
            Core.getMapItem(13301, 1, "naoisegrave");
            Core.huntMonster("naoisegrave", "Warden Iseul ", "Iseul's Mask", 1);
            Core.ensureComplete(9773);
        }

        // Cold-Blooded Torture (9775)
        Story.killQuest(9775, "naoisegrave", "Ice Guardian");

        // Echoing Cries (9776)
        Story.killQuest(9776, "naoisegrave", "Bone Dragonling");

        // Dragon's Seal (9777)
        if (!Story.questProgression(9777)) {
            Story.killQuest(9777, "naoisegrave", "Volgritian");
        }
    }

    liaTaraHill(seaVoice = false, coldThunder = false): void {
        if (Core.isCompletedBefore(9814))
            return;

        this.naoiseGrave(seaVoice, coldThunder);

        // At Ease (9805)
        Story.killQuest(9805, "liatarahill", "Undead Garde");

        // Greenguard Daisy (9806)
        Story.killQuest(9806, "liatarahill", "Garde Wraith");

        // True Nature (9807)
        if (!Story.questProgression(9807)) {
            Core.ensureAccept(9807);
            Core.huntMonster("liatarahill", "Undead Garde ", "Garde's Sprouts", 6);
            Core.huntMonster("liatarahill", "Garde Wraith", "Wraith's Breath", 12);
            Core.ensureComplete(9807);
        }

        // Deafly Pride (9808)
        Story.mapItemQuest(9808, "liatarahill", [ 13364, 13365 ]);

        // King of Briars and Smoke (9809)
        if (!Story.questProgression(9809)) {
            Core.equipClass(ClassType.Solo);
            Story.killQuest(9809, "liatarahill", "King Duncan");
            Core.equipClass(ClassType.Farm);
        }

        // Last Snowfall (9810)
        if (!Story.questProgression(9810)) {
            Core.ensureAccept(9810);
            Core.getMapItem(13368, 1, "liatarahill");
            Core.huntMonster("liatarahill", "Undead Garde ", "Garde's Bones", 50);
            Core.huntMonster("liatarahill", "Garde Wraith", "Wraith's Soul", 3);
            Core.ensureComplete(9810);
        }

        // Spring Melt (9811)
        if (!Story.questProgression(9811)) {
            Core.ensureAccept(9811);
            Core.getMapItem(13366, 1, "liatarahill");
            Core.huntMonster("liatarahill", "Ice Guardian", "Icy Fractal", 3);
            Core.ensureComplete(9811);
        }

        // Connla (9812)
        Story.killQuest(9812, "liatarahill", "Skye Cailleach");

        // Dumha na nGiall (9813)
        if (!Story.questProgression(9813)) {
            Core.ensureAccept(9813);
            Core.getMapItem(13367, 1, "liatarahill");
            Core.huntMonster("liatarahill", "Ice Guardian", "Frozen Blessing", 12);
            Core.huntMonster("liatarahill", "Skye Cailleach", "Cailleach's Mask", 9);
            Core.ensureComplete(9813);
        }

        // Changeling (9814)
        if (!Story.questProgression(9814)) {
            Core.equipClass(ClassType.Solo);
            Story.killQuest(9814, "liatarahill", "Warden Illaria");
            Core.equipClass(ClassType.Farm);
        }
    }

    castleGaheris(seaVoice = false, coldThunder = false): void {
        if (Core.isCompletedBefore(9828))
            return;

        this.liaTaraHill(seaVoice, coldThunder);

        // Needs of the Few (9819)
        if (!Story.questProgression(9819)) {
            Core.ensureAccept(9819);
            Core.getMapItem(13378, 1, "castlegaheris");
            Core.huntMonster("castlegaheris", "Energy Elemental ", "Electronic Gem", 18);
            Core.ensureComplete(9819);
        }

        // Lost and Numb (9820)
        Story.killQuest(9820, "castlegaheris", "Ice Guardian");

        // Childhood Memories (9821)
        Story.mapItemQuest(9821, "castlegaheris", [ 13379, 13380, 13381 ]);

        // Strangled Sobs (9822)
        if (!Story.questProgression(9822)) {
            Core.ensureAccept(9822);
            Core.huntMonster("castlegaheris", "Energy Elemental ", "Shock Enhancer", 9);
            Core.huntMonster("castlegaheris", "Ice Guardian", "Frozen Eye", 9);
            Core.ensureComplete(9822);
        }

        // Distant Care (9823)
        Story.killQuest(9823, "castlegaheris", "Dark Cage");

        // Minus K (9824)
        Story.killQuest(9824, "castlegaheris", "Glacial Crystal");

        // Someone Else's Blood and Tears (9825)
        Story.killQuest(9825, "castlegaheris", "Elemental Hybrid");

        // Honor Thy Family (9826)
        Story.mapItemQuest(9826, "castlegaheris", [ 13382, 13383 ]);

        // Life's Latest Lows (9827)
        if (!Story.questProgression(9827)) {
            Core.ensureAccept(9827);
            Core.huntMonster("castlegaheris", "Glacial Crystal", "Glaceran Ice", 32);
            Core.huntMonster("castlegaheris", "Elemental Hybrid", "Hybrid Remnants", 32);
            Core.ensureComplete(9827);
        }

        // Vow of Misery (9828)
        if (!Story.questProgression(9828)) {
            Core.equipClass(ClassType.Solo);
            Story.killQuest(9828, "castlegaheris", "Thundersnow Storm");
            Core.equipClass(ClassType.Farm);
        }
    }

    coldThunder(): void {
        if (Core.isCompletedBefore(9851))
            return;

        this.castleGaheris(false, false);

        // 9832 | A Friend's Faith
        Story.mapItemQuest(9832, "coldthunder", 13403);

        // 9833 | The Storm Queen
        if (!Story.questProgression(9833)) {
            Core.logger(`Doing 9833 | The Storm Queen`);
            Core.ensureAccept(9833);
            this.coldThunderBoss("Cold Thunder Defeated");
            Core.ensureComplete(9833);
        }
    }

    thelimaCity(seaVoice = false, coldThunder = false): void {
        if (Core.isCompletedBefore(10356))
            return;

        this.terminaTemple(seaVoice, coldThunder);

        Story.preLoad(this.thelimaCity);
        //#region Useable Monsters
        const UseableMonsters = [
            "Noelle Knight", // UseableMonsters[0],
            "Drow Soldier", // UseableMonsters[1],
            "Dwarven Aegis", // UseableMonsters[2],
            "Maleno Elemental", // UseableMonsters[3],
            "Maleno Match", // UseableMonsters[4],
            "Flame of Maleno", // UseableMonsters[5],
            "Eternal Flames", // UseableMonsters[6]
        ];
        //#endregion Useable Monsters

        // 10347 | Sister Statues
        Story.mapItemQuest(10347, "thelimacity", [ 14770, 14771 ]);

        // 10348 | Starstruck Chivalry
        if (!Story.questProgression(10348))
            Core.huntMonsterQuest(10348, "thelimacity", UseableMonsters[0]);

        // 10349 | Black Dahlia
        Story.mapItemQuest(10349, "thelimacity", [ 14772, 14773 ]);

        // 10350 | Talls and Stouts
        if (!Story.questProgression(10350)) {
            Core.huntMonsterQuest(
                10350, 
                ["thelimacity", UseableMonsters[2], ClassType.Farm],
                ["thelimacity", UseableMonsters[1], ClassType.Farm]);
        }

        // 10351 | A Lost Matchstick
        if (!Story.questProgression(10351)) {
            Core.ensureAccept(10351);
            Story.mapItemQuest(10351, "thelimacity", 14774);
            Core.killMonster("thelimacity", "r9", "Left", UseableMonsters[4], "Ashen Key");
            Core.ensureComplete(10351);
        }

        // 10352 | Flash Fire
        Story.mapItemQuest(10352, "thelimacity", 14775, 4);

        // 10353 | Unio Naturalis
        if (!Story.questProgression(10353)) {
            Core.huntMonsterQuest(10353, ["thelimacity", UseableMonsters[3], ClassType.Solo]);
        }

        // 10354 | A Friend's Distant Laugh
        if (!Story.questProgression(10354)) {
            Core.huntMonsterQuest(10354, ["thelimacity", UseableMonsters[4], ClassType.Farm]);
        }

        // 10355 | Putrefaction
        if (!Story.questProgression(10355)) {
            Core.huntMonsterQuest(
                10355, 
                ["thelimacity", UseableMonsters[3], ClassType.Solo],
                ["thelimacity", UseableMonsters[4], ClassType.Farm]);
        }

        // 10356 | Enatiodromia
        if (!Story.questProgression(10356)) {
            Core.equipClass(ClassType.Solo);
            Core.huntMonsterQuest(10356, ["thelimacity", UseableMonsters[5], ClassType.Solo]);
        }
    }

    mountMaleno(seaVoice = false, coldThunder = false): void {
        if (Core.isCompletedBefore(10368))
            return;

        this.thelimaCity(seaVoice, true);

        //#region Useable Monsters
        const UseableMonsters = [
            "Soul Raven", // UseableMonsters[0],
            "Maleno Match", // UseableMonsters[1],
            "Maleno Elemental", // UseableMonsters[2],
            "Draconian Bandit", // UseableMonsters[3],
            "Idalion", // UseableMonsters[4]
        ];
        //#endregion Useable Monsters

        // 10359 | A-Luring Song
        if (!Story.questProgression(10359)) {
            Core.ensureAccept(10359);
            Core.getMapItem(14805, 1, "mountmaleno");
            Core.huntMonster("mountmaleno", "Soul Raven", "Soul Song", 15);
            Core.ensureComplete(10359);
        }

        // 10360 | Splendid Ashes, Pompous Grave
        if (!Story.questProgression(10360)) {
            Core.huntMonsterQuest(10360, ["mountmaleno", UseableMonsters[1], ClassType.Farm]);
        }

        // 10361 | Echolocation
        Story.mapItemQuest(10361, [ [14806, 1, "mountmaleno"], [14807, 5, "mountmaleno"] ]);

        // 10362 | Chymerical Ash
        if (!Story.questProgression(10362)) {
            Core.huntMonsterQuest(
                10362, 
                ["mountmaleno", UseableMonsters[0], ClassType.Farm],
                ["mountmaleno", UseableMonsters[1], ClassType.Farm]);
        }

        // 10363 | Journey Through the Dark Night
        Story.mapItemQuest(10363, "mountmaleno", [ 14808, 14809 ]);

        // 10364 | Lunar Caustic
        if (!Story.questProgression(10364)) {
            Core.ensureAccept(10364);
            Core.getMapItem(14810, 1, "mountmaleno");
            Core.huntMonster("mountmaleno", "Maleno Elemental", "Maleno Steel", 6);
            Core.ensureComplete(10364);
        }

        // 10365 | Like Water and Lithium
        if (!Story.questProgression(10365)) {
            Core.ensureAccept(10365);
            Core.getMapItem(14811, 1, "mountmaleno");
            Core.huntMonster("mountmaleno", "Draconian Bandit", "Putrefied Scale", 15);
            Core.ensureComplete(10365);
        }

        // 10366 | The Shadows Lengthen in…
        Story.mapItemQuest(10366, "mountmaleno", 14812);

        // 10367 | Blithe Hospitality
        if (!Story.questProgression(10367)) {
            Core.huntMonsterQuest(
                10367, 
                ["mountmaleno", UseableMonsters[3], ClassType.Farm],
                ["mountmaleno", UseableMonsters[2], ClassType.Solo]);
        }

        // 10368 | Sal Volatile
        if (!Story.questProgression(10368)) {
            Core.huntMonsterQuest(10368, ["mountmaleno", UseableMonsters[4], ClassType.Solo]);
        }
    }

    sanctuaryAiwass(seaVoice = false, coldThunder = false): void {
        if (Core.isCompletedBefore(10384))
            return;

        this.mountMaleno(seaVoice, coldThunder);

        // 10375 - Raw Dove x12 (Dropped by Dove)
        Story.killQuest(10375, "sanctuaryaiwass", "Dove");

        // 10376 - Albedo Bracelet x4 (Dropped by Albedo Elemental)
        Story.killQuest(10376, "sanctuaryaiwass", "Albedo Elemental");

        // 10377 - Pristine Feather x18 (Dropped by Albedo Match)
        Story.killQuest(10377, "sanctuaryaiwass", "Albedo Match");

        // 10378 - Leonal's Dream x15 (Dropped by Leonal)
        //        Map items: Svala (14865), Snowbeard (14866)
        if (!Story.questProgression(10378)) {
            Core.ensureAccept(10378);
            Core.getMapItem(14865, undefined, "sanctuaryaiwass"); // Svala
            Core.getMapItem(14866, undefined, "sanctuaryaiwass"); // Snowbeard
            Core.huntMonster("sanctuaryaiwass", "Leonal", "Leonal's Dream", 15);
            Core.ensureComplete(10378);
        }

        // 10379 - Salt Sigil x1 (Dropped by Crystal Prima)
        //        Map item: Geopetal (14867)
        if (!Story.questProgression(10379)) {
            Core.ensureAccept(10379);
            Core.getMapItem(14867, undefined, "sanctuaryaiwass"); // Geopetal
            Core.huntMonster("sanctuaryaiwass", "Crystal Prima", "Salt Sigil", 1);
            Core.ensureComplete(10379);
        }

        // 10380 - Albedo Materia x1 (Dropped by Albedo Elemental)
        //        Map item: Isiri (14868)
        if (!Story.questProgression(10380)) {
            Core.ensureAccept(10380);
            Core.huntMonster("sanctuaryaiwass", "Albedo Elemental", "Albedo Materia", 1);
            Core.getMapItem(14868, undefined, "sanctuaryaiwass"); // Isiri
            Core.ensureComplete(10380);
        }

        // 10381 - Match's Scar x5 (Dropped by Albedo Match)
        Story.killQuest(10381, "sanctuaryaiwass", "Albedo Match");

        // 10382 - Mercury Sigil x1 (Dropped by Mercury Prima)
        //        Sulfur Sigil x1 (Dropped by Sulfur Prima)
        if (!Story.questProgression(10382)) {
            Core.ensureAccept(10382);
            Core.huntMonster("sanctuaryaiwass", "Mercury Prima", "Mercury Sigil", 1);
            Core.killMonster("sanctuaryaiwass", "r9", "Top", "*", "Sulfur Sigil", 1);
            Core.ensureComplete(10382);
        }

        // 10383 - Offer Flowers x8, Read Strange Scroll x1
        //        Map items: Blue arrows (14869) x8, Strange Scroll (14870)
        if (!Story.questProgression(10383)) {
            Core.ensureAccept(10383);
            for (let i = 0; i < 8; i++)
                Core.getMapItem(14869, undefined, "sanctuaryaiwass"); // Click blue arrows 8 times
            Core.getMapItem(14870, undefined, "sanctuaryaiwass"); // Read Strange Scroll
            Core.ensureComplete(10383);
        }

        // 10384 - Commune with Aiwass x1 (Dropped by Anima Animus Aiwass)
        Story.killQuest(10384, "sanctuaryaiwass", "Anima Animus Aiwass");
    }

    forgeAlbedo(seaVoice = false, coldThunder = false): void {
        if (Core.isCompletedBefore(10422))
            return;

        this.sanctuaryAiwass(seaVoice, coldThunder);

        //#region Useable Monsters
        const UseableMonsters = [
            "Albedo Elemental", // UseableMonsters[0],
            "Defensive Turret", // UseableMonsters[1],
            "Runehawk Husk", // UseableMonsters[2],
            "Collapsed Husk", // UseableMonsters[3],
            "Albedo Match", // UseableMonsters[4],
            "Defense Droid", // UseableMonsters[5],
            "Flame of Albedo", // UseableMonsters[6]
        ];
        //#endregion Useable Monsters

        // 10413 | Discerning Threats
        if (!Story.questProgression(10413)) {
            Core.huntMonsterQuest(10413, ["forgealbedo", UseableMonsters[0], ClassType.Farm]);
        }

        // 10414 | Model Students
        Story.mapItemQuest(10414, "forgealbedo", [ 14959, 14960, 14961 ]);

        // 10415 | System Delusion
        if (!Story.questProgression(10415)) {
            Core.ensureAccept(10415);
            Core.getMapItem(14962, 1, "forgealbedo");
            Core.huntMonster("forgealbedo", "Defensive Turret", "Turret Logic Module", 18);
            Core.ensureComplete(10415);
        }

        // 10416 | The Leap that Never Landed
        if (!Story.questProgression(10416)) {
            Core.ensureAccept(10416);
            Core.getMapItem(14963, 4, "forgealbedo");
            Core.huntMonster("forgealbedo", "Runehawk Husk", "Runehawk Student ID", 12);
            Core.ensureComplete(10416);
        }

        // 10417 | Insistent Existence
        if (!Story.questProgression(10417)) {
            Core.ensureAccept(10417);
            Core.getMapItem(14964, 4, "forgealbedo");
            Core.huntMonster("forgealbedo", "Collapsed Husk", "Husk's Stained ID", 3);
            Core.ensureComplete(10417);
        }

        // 10418 | Failed Reflection
        if (!Story.questProgression(10418)) {
            Core.huntMonsterQuest(10418, ["forgealbedo", UseableMonsters[4], ClassType.Farm]);
        }

        // 10419 | Technically Aware
        if (!Story.questProgression(10419)) {
            Core.huntMonsterQuest(10419, ["forgealbedo", UseableMonsters[5], ClassType.Solo]);
        }

        // 10420 | Mental Barrier
        if (!Story.questProgression(10420)) {
            Core.ensureAccept(10420);
            Core.getMapItem(14965, 6, "forgealbedo");
            Core.huntMonster("forgealbedo", "Defensive Turret", "Blastproof Shell", 12);
            Core.ensureComplete(10420);
        }

        // 10421 | Lost Admin Privileges
        if (!Story.questProgression(10421)) {
            Core.huntMonsterQuest(
                10421, 
                ["forgealbedo", UseableMonsters[1], ClassType.Farm],
                ["forgealbedo", UseableMonsters[5], ClassType.Farm]);
        }

        // 10422 | Conscious Unconsciousness
        if (!Story.questProgression(10422)) {
            Core.huntMonsterQuest(10422, ["forgealbedo", UseableMonsters[6], ClassType.Solo]);
        }
    }

    fortLuma(seaVoice = false, coldThunder = false): void {
        if (Core.isCompletedBefore(10618))
            return;

        this.forgeAlbedo(seaVoice, coldThunder);
        Story.preLoad(this.fortLuma);

        //#region Useable Monsters
        const UseableMonsters = [
            "Citrinitas Elemental", // UseableMonsters[0],
            "Luma Lifeform", // UseableMonsters[1],
            "Lightguard Elite", // UseableMonsters[2],
            "Rigaud &amp; Cassie", // UseableMonsters[3],
            // use monstermapid for this'n [10]
            "Citrinitas Match", // UseableMonsters[4],
            "Luma Dragon Twins", // UseableMonsters[5],
            "Flame of Citrinitas", // UseableMonsters[6]
        ];
        //#endregion Useable Monsters

        // 10609 | Xanthosis
        if (!Story.questProgression(10609)) {
            Story.mapItemQuest(10609, "fortluma", 15542);
            Core.huntMonsterQuest(10609, 
                ["fortluma", UseableMonsters[0], ClassType.Solo]);
        }


        // 10610 | Golden Lead
        if (!Story.questProgression(10610)) {
            Story.mapItemQuest(10610, "fortluma", 15543);
            Core.huntMonsterQuest(10610, 
                ["fortluma", UseableMonsters[1], ClassType.Farm]);
        }


        // 10611 | Luminary Vision
        if (!Story.questProgression(10611)) {
            Story.mapItemQuest(10611, "fortluma", [ 15544, 15545 ]);
        }


        // 10612 | Premature Sunrise
        if (!Story.questProgression(10612)) {
            Core.huntMonsterQuest(10612, 
                ["fortluma", UseableMonsters[2], ClassType.Farm]);
        }


        // 10613 | Blinded by the Light
        if (!Story.questProgression(10613)) {
            Core.ensureAccept(10613);
            Core.huntMonsterMapID("fortluma", 10, "Astero's Aurum");
            Core.ensureComplete(10613);
        }


        // 10614 | Wayward Wisewoman
        if (!Story.questProgression(10614)) {
            Core.ensureAccept(10614);
            Core.equipClass(ClassType.Solo);
            Core.huntMonster("fortluma", UseableMonsters[2], "Elite Promotion Notice", 6);
            Core.huntMonster("fortluma", UseableMonsters[0], "Cloudy Citrine", 2);
            Story.mapItemQuest(10614, "fortluma", 15546);
            bot.wait.forQuestComplete(10614);
        }

        bot.sleep(2500);
        if (bot.player.cell.includes("Cut"))
            Core.jumpWait();

        // 10615 | Incognizance
        if (!Story.questProgression(10615)) {
            Core.ensureAccept(10615);
            Core.huntMonster("fortluma", UseableMonsters[4], "Calcified Eyes", 18);
            Story.mapItemQuest(10615, "fortluma", 15547);
        }


        // 10616 | Flowers of Luma
        if (!Story.questProgression(10616)) {
            Core.huntMonsterQuest(10616, 
                ["fortluma", UseableMonsters[5], ClassType.Solo]);
        }


        // 10617 | Quid Pro Quo
        if (!Story.questProgression(10617)) {
            Core.ensureAccept(10617);
            Core.killMonster("fortluma", "r10", "Left", "*", "Dark Azoth", 6);
            Story.mapItemQuest(10617, "fortluma", 15548);
        }


        // 10618 |Drowned King in Yellow
        if (!Story.questProgression(10618)) {
            bot.log("**Boss Gear Suggestion**\nSuggested Class and Enh for this boss if your stuck, swap your Solo class to: Dragon of time(or any decent HoT class)\n--With these enhancements--\n" + "Class: Healer\nHelm: Healer\nWeapon: Elysium/Healer\nCape: Absolution/Healer");
            Core.huntMonsterQuest(10618, 
                ["fortluma", UseableMonsters[6], ClassType.Solo]);
            bot.wait.forQuestComplete(10618);
            bot.sleep(2500);
            Core.jumpWait();

        }
    }

    warWickForest(seaVoice = false, coldThunder = false): void {
        if (Core.isCompletedBefore(10687))
            return;
        this.fortLuma(seaVoice, coldThunder);

        Story.preLoad(this.warWickForest);

        //#region Useable Monsters
        const UseableMonsters = [
            "Black Phoenix", // UseableMonsters[0],
            "Rubedo Elemental", // UseableMonsters[1],
            "Rubedo Match", // UseableMonsters[2],
            "Charred Remnant", // UseableMonsters[3],
            "Idea of a Champion", // UseableMonsters[4],
            "Flame of Rubedo", // UseableMonsters[5]
        ];
        //#endregion Useable Monsters

        // 10678 | A Phoenix's Repeated Demise
        if (!Story.questProgression(10678)) {
            Story.mapItemQuest(10678, "warwickforest", 15656);
            Core.huntMonsterQuest(10678, 
                ["warwickforest", UseableMonsters[0], ClassType.Solo]);
        }

        // 10679 | Reopened Scar
        if (!Story.questProgression(10679)) {
            Story.mapItemQuest(10679, "warwickforest", 15657);
            Core.huntMonsterQuest(10679, 
                ["warwickforest", UseableMonsters[1], ClassType.Solo]);
        }

        // 10680 | Pyroclast Breath
        if (!Story.questProgression(10680)) {
            Core.huntMonsterQuest(10680, 
                ["warwickforest", UseableMonsters[3], ClassType.Solo]);
        }

        // 10681 | Smoke Signals
        if (!Story.questProgression(10681)) {
            Story.mapItemQuest(10681, "warwickforest", 15658);
            Core.huntMonsterQuest(10681, 
                ["warwickforest", UseableMonsters[1], ClassType.Solo]);
        }

        // 10682 | Reflection in Oil
        if (!Story.questProgression(10682)) {
            Core.ensureAccept(10682);
            Story.mapItemQuest(10682, "warwickforest", 15659);
            Core.killMonster("warwickforest", "r8", "Left", "*", "Smoldering Ashes", 6);
            Core.ensureComplete(10682);
        }

        // 10683 | Iosis of the Soul
        if (!Story.questProgression(10683)) {
            Story.mapItemQuest(10683, "warwickforest", 15660);
            Core.huntMonsterQuest(10683, 
                ["warwickforest", UseableMonsters[2], ClassType.Solo]);
        }

        // 10684 | The Scorpion and the Fox
        if (!Story.questProgression(10684)) {
            Core.ensureAccept(10684);
            Story.mapItemQuest(10684, "warwickforest", 15661);
            Core.killMonster("warwickforest", "r2", "Left", "*", "Philosopher's Eye", 15);
            Core.ensureComplete(10684);

        }

        // 10685 | Dalitong Keli
        if (!Story.questProgression(10685)) {
            Story.mapItemQuest(10685, "warwickforest", 15662, 5);
        }

        // 10686 | The Ideal Champion
        if (!Story.questProgression(10686)) {
            Core.huntMonsterQuest(10686, 
                ["warwickforest", UseableMonsters[4], ClassType.Solo]);
        }

        // 10687 | Kolr
        if (!Story.questProgression(10687)) {
            Core.huntMonsterQuest(10687, 
                ["warwickforest", UseableMonsters[5], ClassType.Solo]);
        }
    }

    magnumOpus(seaVoice = false, coldThunder = false): void {
        // C# local function (async void): walks out of the zone the boss is about to hit.
        const flameoftheBeyond = (packet: any): void => {
            if (packet?.["params"]?.type?.toString() !== "json")
                return;
            if (!bot.player.alive)
                return;
            const data = packet["params"].dataObj;
            if (data?.cmd?.toString() !== "event")
                return;
            const zoneSet = data?.args?.zoneSet?.toString();
            if (!zoneSet)
                return;

            const px = bot.player.x;
            const py = bot.player.y;

            // Zone A active, walk right
            if (zoneSet?.toLowerCase() === "A".toLowerCase()) {
                /* box of : 
                x: 550, y: 339
                x: 678, y: 465
                */
                if (px >= 550 && px <= 678 && py >= 339 && py <= 465)
                    return;

                const randX = randomInt(550, 678);
                const randY = randomInt(339, 465);
                bot.player.walkTo(randX, randY);
                return;
            }

            // Zone B active, walk left
            if (zoneSet?.toLowerCase() === "B".toLowerCase()) {
                /* box of : 
                x: 426, y: 359
                x: 211, y: 445
                */
                if (px >= 211 && px <= 426 && py >= 359 && py <= 445)
                    return;

                const randX = randomInt(211, 426);
                const randY = randomInt(359, 445);
                bot.player.walkTo(randX, randY);
                return;
            }
        };

        if (Core.isCompletedBefore(10705))
            return;

        this.warWickForest(true, true);
        Story.preLoad(this.magnumOpus);

        // 10703 | The Black Mage
        if (!Story.questProgression(10703)) {
            Core.ensureAccept(10703);
            Core.getMapItem(15703, 1, "magnumopus");
            Core.ensureComplete(10703);
        }

        // 10704 | Hic Iacet Revelatio
        if (!Story.questProgression(10704)) {
            Core.ensureAccept(10704);
            if (!!Core.bossClass)
                Core.equipClass(ClassType.Boss);
            else {
                bot.log("BossClass is empty/unselected in CBO, we'll use the Solo class or if thats empty, the currently equipped class.");
                Core.equipClass(ClassType.Solo);
            }
            bot.events.on("extensionPacket", flameoftheBeyond);

            while (!bot.shouldExit && !bot.tempInv.contains("Flame of the Beyond Defeated")) {
                if (bot.map.name !== "magnumopus")
                    Core.join("magnumopus");

                if (bot.player.cell !== "r2")
                    Core.jump("r2", "bottom");

                bot.combat.attack("*");

                bot.sleep(200);
                if (bot.tempInv.contains("Flame of the Beyond Defeated"))
                    break;
            }

            bot.events.off("extensionPacket", flameoftheBeyond);
            Core.ensureComplete(10704);
        }



    }

    rubedoPeak(seaVoice = false, coldThunder = false): void {
        if (Core.isCompletedBefore(10714))
            return;

        this.magnumOpus(seaVoice, coldThunder);

        Story.preLoad(this.rubedoPeak);

        // 10714 | Wave Goodbye
        Story.killQuest(10714, "rubedopeak", "Rubedo Match");
    }

    forgeCitrinitas(seaVoice = false, coldThunder = false): void {
        if (Core.isCompletedBefore(10736))
            return;

        this.rubedoPeak(seaVoice, coldThunder);
        Story.preLoad(this.forgeCitrinitas);

        //#region Useable Monsters
        const UseableMonsters = [
            "Citrinitas Elemental", // UseableMonsters[0],
            "Defensive Turret", // UseableMonsters[1],
            "Citrinitas Match", // UseableMonsters[2],
            "Luma Lifeform", // UseableMonsters[3],
            "Defense Droid", // UseableMonsters[4],
            "Clematis", // UseableMonsters[5]
        ];
        //#endregion Useable Monsters

        // 10725 | The Tarnished Ideal
        if (!Story.questProgression(10725)) {
            Core.huntMonsterQuest(10725, 
                ["forgecitrinitas", UseableMonsters[0], ClassType.Farm]);
        }


        // 10726 | Pointed Suggestions
        if (!Story.questProgression(10726)) {
            Story.mapItemQuest(10726, "forgecitrinitas", [ 15820, 15821 ]);
        }


        // 10727 | Bite the Bullet
        if (!Story.questProgression(10727)) {
            Core.huntMonsterQuest(10727, 
                ["forgecitrinitas", UseableMonsters[1], ClassType.Farm]);
        }


        // 10728 | Pain and Ecstasy
        if (!Story.questProgression(10728)) {
            Core.huntMonsterQuest(10728, 
                ["forgecitrinitas", UseableMonsters[3], ClassType.Farm]);
        }


        // 10729 | Reincubation
        if (!Story.questProgression(10729)) {
            Story.mapItemQuest(10729, "forgecitrinitas", 15822);
        }


        // 10730 | Server Reboot
        if (!Story.questProgression(10730)) {
            Core.huntMonsterQuest(10730, 
                ["forgecitrinitas", UseableMonsters[4], ClassType.Farm]);
        }


        // 10731 | Of Pyrite and Nickel
        if (!Story.questProgression(10731)) {
            Core.huntMonsterQuest(10731, 
                ["forgecitrinitas", "Citrinitas Match", ClassType.Farm]);
        }


        // 10732 | Skin Deep
        if (!Story.questProgression(10732)) {
            Core.huntMonsterQuest(10732, 
                ["forgecitrinitas", UseableMonsters[3], ClassType.Farm]);
        }


        // 10733 | Break Room
        if (!Story.questProgression(10733)) {
            Story.mapItemQuest(10733, "forgecitrinitas", [ 15823, 15824, 15825 ]);
        }


        // 10734 | Clematis' Aspirations
        if (!Story.questProgression(10734)) {
            Core.huntMonsterQuest(10734, 
                ["forgecitrinitas", UseableMonsters[5], ClassType.Solo]);
        }


        if (Core.isMember) {
            // 10735 | Silica Spring
            if (!Story.questProgression(10735)) {
                Core.huntMonsterQuest(10735, 
                    ["forgecitrinitas", UseableMonsters[5], ClassType.Solo],
                    ["forgecitrinitas", UseableMonsters[3], ClassType.Farm],
                    ["forgecitrinitas", UseableMonsters[4], ClassType.Farm]);
            }
        }

        else {
            // 10736 | Silica Spring (Legend)
            if (!Story.questProgression(10736)) {
                Core.huntMonsterQuest(10736, 
                    ["forgecitrinitas", UseableMonsters[5], ClassType.Solo],
                    ["forgecitrinitas", UseableMonsters[3], ClassType.Farm],
                    ["forgecitrinitas", UseableMonsters[4], ClassType.Farm]);
            }
        }
    }

    coldThunderBoss(item: string | null = null, quant = 1, isTemp = true): void {
        // C# local functions
        const listener = (packet: any): void => {
            const type = packet["params"].type;
            const data = packet["params"].dataObj;
            if (type === "json") {
                const cmd = data.cmd.toString();
                switch (cmd) {
                    case "ct":
                        if (data.anims != null) {
                            for (const a of data.anims) {
                                if (a == null)
                                    continue;

                                if (
                                    a.msg != null
                                    && a.msg === "The skies rumble. Prepare yourself!"
                                ) {
                                    needsPotion = true;
                                    potionApplied = false;
                                    Core.logger(
                                        "Event detected: The skies rumble. Prepare yourself! - Potion needed.");
                                }
                            }
                        }
                        break;
                }
            }
        };

        if (
            item != null
            && (isTemp ? bot.tempInv.contains(item, quant) : Core.checkInventory(item, quant))
        ) {
            bot.events.off("extensionPacket", listener);
            Core.jumpWait();
            return;
        }

        Core.equipClass(ClassType.Solo);

        // Reset potion flags
        let needsPotion = false;
        let potionApplied = false;

        bot.events.on("extensionPacket", listener);

        if (!isTemp && item != null)
            Core.addDrop(item);

        //get, equip, and use safe pot (potions buggy af ae.. fix this)
        Core.buyItem("mirrorportal", 774, "Shriekward Potion", 99);
        Core.equip("Shriekward Potion");
        bot.wait.forItemEquip("Shriekward Potion");
        bot.wait.forActionCooldown("equipItem");
        Core.sleep();
        Core.usePotion();

        //get & equip useable pot
        Core.buyItem("coldthunder", 2467, "Bananach's Last Will", 1000);
        Core.equip("Bananach's Last Will");
        bot.wait.forItemEquip("Bananach's Last Will");
        bot.wait.forActionCooldown("equipItem");
        Core.sleep();

        Core.logger(
            "About to attack Cold Thunder boss. It's very unstable; recommended to do it with more accounts.");

        while (
            !bot.shouldExit
            && item != null
            && (isTemp ? !bot.tempInv.contains(item, quant) : !Core.checkInventory(item, quant))
        ) {
            if (bot.map.name !== "coldthunder")
                Core.join("coldthunder");

            if (bot.player.cell !== "r3")
                Core.jump("r3");

            // Check if potion is needed and handle it
            if (needsPotion && !potionApplied) {
                Core.logger(
                    "Detected 'The skies rumble. Prepare yourself!' - applying Bananach's Last Will potion...");

                // Keep trying to use potion until it's successfully applied
                while (!bot.shouldExit && needsPotion && !potionApplied) {
                    bot.combat.cancelAutoAttack();
                    bot.combat.cancelTarget();
                    Core.usePotion();
                    Core.sleep(200); // Small delay to allow potion to apply

                    // Check if Bananach's Last Will potion was applied
                    // You might need to adjust the aura name based on what this potion actually provides
                    if (bot.self.auras.some(a => a != null && a.name === "Bananach's Last Will")) {
                        potionApplied = true;
                        needsPotion = false;
                        Core.logger("Bananach's Last Will potion successfully applied!");
                    } else {
                        Core.sleep(100); // Brief pause before retrying
                    }
                }
            }

            // Only attack if no potion is needed or potion has been applied
            if (!needsPotion || potionApplied) {
                bot.combat.attack("*");
            }

            if (
                item != null
                && (isTemp ? bot.tempInv.contains(item, quant) : Core.checkInventory(item, quant))
            ) {
                Core.jumpWait();
                break;
            }

            Core.sleep();
        }
        bot.events.off("extensionPacket", listener);

        // Reset potion flags
        needsPotion = false;
        potionApplied = false;

        Core.jumpWait();
        Adv.gearStore(true, { enhAfter: true });
    }

    fightVoiceInTheSea(item: string, quant = 1, isTemp = false): void {
        // C# local functions
        const finished = (): boolean => {
            if (isTemp)
                return bot.tempInv.contains(item, quant);
            return bot.inventory.contains(item, quant);
        };

        if (!isTemp)
            Core.unbank(item);
        if (finished())
            return;
        if (!isTemp)
            Core.addDrop(item);

        // Vainglory compounds the healing reduction from Voice in the Sea.
        const cape = bot.inventory.items.find(i => i.equipped
            && i.category === "Cape"
            && i.enhancementPatternId === CapeSpecial.Vainglory);
        if (cape != null)
            Adv.enhanceItem(
                cape.name, 
                EnhancementType.Lucky, 
                // C# also passes `logging: false`, which Turu's enhanceItem doesn't take.
                { cSpecial: CapeSpecial.None, hSpecial: HelmSpecial.None, wSpecial: WeaponSpecial.None, bypassDisableAutoEnhance: true });

        Core.join("seavoice");
        Adv.buyItem("seavoice", 2320, "Vigil", 1000, 12023);
        Core.equip("Vigil");
        if (!bot.inventory.isEquipped("Vigil")) {
            Core.logger("Vigil could not be equipped.");
            return;
        }

        this._voiceChargeDetected = false;
        // C# listens to the raw `pext` flash call; Turu reads the same packet as an extensionPacket.
        bot.events.on("extensionPacket", this.seaVoiceFlashListener);
        try {
            Core.jump("r2", "Left");
            bot.player.setSpawnPoint();
            while (!bot.shouldExit && !finished()) {
                if (!bot.player.alive) {
                    this._voiceChargeDetected = false;
                    bot.skills.resume();
                    bot.sleep(500);
                    continue;
                }
                if (bot.player.cell !== "r2")
                    Core.jump("r2", "Left");

                if (this._voiceChargeDetected) {
                    while (!bot.shouldExit && bot.player.alive && bot.player.hasTarget && !bot.self.hasActiveAura("Vigil")) {
                        if (bot.skills.canUseSkill(5))
                            bot.skills.useSkill(5);
                        bot.sleep(50);
                    }
                    this._voiceChargeDetected = false;
                    bot.skills.resume();
                    if (!bot.shouldExit && bot.self.hasActiveAura("Vigil"))
                        Core.logger("Vigil active. Class skills resumed.");
                    continue;
                }

                const target = bot.player.target;
                if (target == null || target.mapId !== 1 || target.hp <= 0)
                    bot.combat.attack(1);
                bot.sleep(500);
            }
        } finally {
            bot.events.off("extensionPacket", this.seaVoiceFlashListener);
            this._voiceChargeDetected = false;
            bot.skills.resume();
            Core.jumpWait();
        }
    }

    private readonly seaVoiceFlashListener = (packet: unknown): void => {
        try {
            if (packet == null || this._voiceChargeDetected)
                return;
            const rawPacket = JSON.stringify(packet);
            if (!rawPacket.toLowerCase().includes("you will not resist!"))
                return;
            bot.skills.pause();
            this._voiceChargeDetected = true;
            Core.logger("Charge detected. Class skills paused for Vigil.");
        } catch (e) {
            rethrowIfStopped(e);
        }
    };
}
