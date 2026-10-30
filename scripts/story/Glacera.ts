/*
name: Glacera Story
description: This will finish the Glacera Story.
tags: story, quest, glacera
*/
// Port of Scripts/Story/Glacera.cs.

import { bot } from "turu";
import { ClassType, Core } from "../core/CoreBots";
import { CoreStory } from "../core/CoreStory";

const Story = new CoreStory();

export class GlaceraStory {
    doAll(Cryomancer = false): void {
        this.frozenTower(Cryomancer);
        this.frozenRuins();
        this.glacera();
        this.glacera1();
        this.frozenRuins2();
        this.northstar();
        this.frostRuins();
        this.iceWindPass();
        this.iceDungeon();
    }

    frozenTower(Cryomancer = false): void {
        if (Core.isCompletedBefore(3937))
            return;

        Story.preLoad(this.frozenTower);

        // Seek the Tower
        Story.mapItemQuest(3907, "frozentower", 3022);

        // A n-Ice Beginning
        Story.killQuest(3908, "frozentower", "Polar Elemental");

        // Search for Syrrus
        Story.mapItemQuest(3909, "frozentower", 3019);

        // Building the Base
        Story.mapItemQuest(3910, "frozentower", 3004, 13);

        // Refugee Roundup
        if (!Story.questProgression(3911)) {
            Core.ensureAccept(3911);
            Core.huntMonster("frozentower", "Frostwyrm", "Refugee Saved", 7);
            Core.ensureComplete(3911);
        }

        // Retrieve the Water Starstone
        Story.killQuest(3912, "frozentower", "FrostDeep Dweller");

        // Magical Attraction
        Story.mapItemQuest(3913, "frozentower", 3005, 13);

        // Frozen Blood
        Story.killQuest(3914, "frozentower", "Twisted Ice");

        // Retrieve the Fire Starstone
        Story.mapItemQuest(3915, "frozentower", 3006);

        // Defend the Tower!
        Story.killQuest(3916, "frozentower", "Polar Elemental");

        // Refugee Rescue Run
        Story.mapItemQuest(3917, "frozentower", 3007, 6);

        // Retrieve the Earth Starston
        Story.mapItemQuest(3918, "frozentower", 3013);

        // Polar Penetration and Progress
        if (!Story.questProgression(3919)) {
            Core.ensureAccept(3919);
            Core.getMapItem(3008, 6, "frozentower");
            Core.huntMonster("frozentower", "Polar Elemental", "Ice Sample", 6);
            Core.ensureComplete(3919);
        }

        // Save the Astronomer Apprentice
        if (!Story.questProgression(3920)) {
            Core.ensureAccept(3920);
            Core.getMapItem(3020, 1, "frozentower");
            Core.huntMonster("frozentower", "Polar Elemental", "Polar Elemental Guard Defeated", 4);
            Core.huntMonster("frozentower", "Ice Wolf", "Ice Wolf Guard Defeated", 5);
            Core.ensureComplete(3920);
        }

        // Glacial Elixir
        if (!Story.questProgression(3921)) {
            Core.ensureAccept(3921);
            Core.getMapItem(3017, 6, "frozentower");
            Core.huntMonster("frozentower", "FrostDeep Dweller", "FrostDeep Venom", 1);
            Core.ensureComplete(3921);
        }

        // Retrieve the Energy Starstone
        Story.killQuest(3922, "frozentower", "Polar Elemental");

        // Marking the Future
        Story.killQuest(3923, "frozentower", "Frostwyrm");

        //Glacial Shift
        Story.mapItemQuest(3924, "frozentower", 3009, 6);

        // Divination Draft
        if (!Story.questProgression(3925)) {
            Story.mapItemQuest(3925, "frozentower", 3012, 4);
            Story.mapItemQuest(3925, "frozentower", 3011, 4);

            Core.huntMonster("frozentower", "Frostwyrm", "Frozen Tail", 5);
            Core.huntMonster("frozentower", "Arctic Eel", "Frozen Scale", 5);
            Core.ensureComplete(3925);
        }

        // Retrieve the Light StarStone
        Story.mapItemQuest(3926, "frozentower", 3021);

        // The Future is Bright
        Story.mapItemQuest(3927, "frozentower", 3014, 7);

        // Bled Bone Dry
        Story.killQuest(3928, "frozentower", "Arctic Eel");

        // Chill of Fear
        Story.killQuest(3929, "frozentower", "Polar Elemental");

        // Retrieve the Darkness Starstone
        Story.killQuest(3930, "frozentower", "Twisted Ice");

        // Web of Fear
        Story.killQuest(3931, "frozentower", "Frostwyrm");

        // Frozen in Time
        Story.mapItemQuest(3932, "frozentower", 3016, 13);

        // Heart of the Matter
        Story.killQuest(3933, "frozentower", "Ice Wolf");

        // Retrieve the Wind Starstone
        Story.killQuest(3934, "frozentower", "Rotten Ice");

        // Create the Gate
        if (!Story.questProgression(3935)) {
            Core.ensureAccept(3935);
            Core.getMapItem(3018, 13, "frozentower");
            Core.huntMonster("frozentower", "Ice Wolf", "Blood of the Ice Wolf", 1);
            Core.ensureComplete(3935);
        }

        // Drive Back the Invaders
        if (!Story.questProgression(3936)) {
            Core.ensureAccept(3936);
            Core.huntMonster("frozentower", "Frost Invader", "FrostSpawn Invader defeated", 10);
            Core.ensureComplete(3936);
        }

        // Defeat the FrostSpawn Invaders
        if (!Story.questProgression(3937)) {
            Core.ensureAccept(3937);
            Core.huntMonster("frozentower", "Frost Fangbeast", "Fangbeasts defeated", 15);
            Core.ensureComplete(3937);
        }

        if (Cryomancer)
            return;
    }

    frozenRuins(): void {
        if (Core.isCompletedBefore(3946))
            return;

        this.frozenTower();

        Story.preLoad(this.frozenRuins);
        // FrozenRuins

        // FangBeast Bash-up
        Story.killQuest(3941, "frozenruins", "Frost Fangbeast");

        // Ravage the Reapers
        Story.killQuest(3942, "frozenruins", "Frost Reaper");

        // Oh the Humanity
        Story.killQuest(3943, "frozenruins", "Frost Reaper");

        // Close the Gate
        Story.killQuest(3944, "frozenruins", "Frost Reaper");

        // Form the Lock
        if (!Story.questProgression(3945)) {
            Core.ensureAccept(3945);
            Core.getMapItem(3050, 10, "frozenruins");
            Core.huntMonster("frozenruins", "Frozen Moglinster", "Frostbreath Weed", 5);
            Core.ensureComplete(3945);
        }

        // Glacera
        if (!Story.questProgression(3946)) {
            Core.ensureAccept(3946);
            if (!Core.checkInventory(27357))
                Core.huntMonster("frozenruins", "Frost Reaper", "Mercury");
            Core.ensureComplete(3946);
        }
    }

    glacera(): void {
        if (Core.isCompletedBefore(3950))
            return;

        this.frozenRuins();
        // Glaera

        //A Frosty Welcome
        if (!Story.questProgression(3947)) {
            Core.ensureAccept(3947);
            Core.join("glacera");
            bot.wait.forMapLoad("glacera");
            Core.getMapItem(3048, 1, "glacera");
            Core.ensureComplete(3947);
            // Ensure we're out of a potent cutscene/move cell
            Core.jump("Enter", "Spawn");
        }

        // Key to the Fortress
        Story.killQuest(3948, "glacera", "Frost Invader");
    }

    glacera1(): void {
        if (Core.isCompletedBefore(3950))
            return;

        this.glacera();

        // Breaking Boulders
        Story.mapItemQuest(3949, "glacera", 3049, 6);

        // The Scythe of Vengeance
        Story.mapItemQuest(3950, "Glacera", 3047);
    }

    frozenRuins2(): void {
        if (Core.isCompletedBefore(3954))
            return;

        this.glacera1();

        // FrozenRuins encore

        // // Rescue the Refugees
        if (!Story.questProgression(3951)) {
            Core.ensureAccept(3951);
            Core.huntMonster("frozenruins", "Frost Invader", "Ice Beast Destroyed", 10);
            Core.ensureComplete(3951);
        }

        // Defeat the Fangbeasts
        Story.killQuest(3952, "frozenruins", "Frost Fangbeast");

        // Destroy the Frost Reapers
        Story.killQuest(3953, "frozenruins", "Frost Reaper");

        // FrostSpawn General Takedown
        Story.killQuest(3954, "frozenruins", "Frost General");
    }

    northstar(): void {
        if (Core.isCompletedBefore(3971))
            return;

        this.frozenRuins2();

        // Northstar

        // From Refugee to Enemy
        if (!Story.questProgression(3958)) {
            Core.ensureAccept(3958);
            Core.huntMonster("northstar", "Monstrous Refugee", "Monstrous Refugee defeated", 3);
            Core.huntMonster("northstar", "Frost Invader", "Frost Invader defeated", 5);
            Core.ensureComplete(3958);
        }

        // Fangs and Blades
        if (!Story.questProgression(3959)) {
            Core.ensureAccept(3959);
            Core.huntMonster("northstar", "Frost Fangbeast", "FrostSpawn defeated", 7);
            Core.ensureComplete(3959);
        }

        // Reaping the Refugees
        Story.killQuest(3960, "northstar", "Frost Reaper");

        // Saving Syrrus' Spirit
        if (!Story.questProgression(3961)) {
            Core.ensureAccept(3961);
            Core.getMapItems([ [3060, 5], [3061, 7], [3073, 5] ], "northstar");
            Core.huntMonster("northstar", "Frost Reaper", "Lodestone Piece", 10);
            Core.ensureComplete(3961);
        }

        // It's a Trap!
        Story.mapItemQuest(3972, "northstar", 3063, 10);

        // Feast or Famine
        if (!Story.questProgression(3973)) {
            Core.ensureAccept(3973);
            Core.huntMonster("northstar", "Frost Fangbeast", "Frozen Dinners", 10);
            Core.ensureComplete(3973);
        }

        // Decipher the Freezing
        if (!Story.questProgression(3974)) {
            Core.ensureAccept(3974);
            Core.huntMonster("northstar", "Frost Fangbeast", "FrostSpawn Spine", 10);
            Core.huntMonster("northstar", "Frost Fangbeast", "FrostSpawn Scale", 10);
            Core.ensureComplete(3974);
        }

        // A New Frost Monster
        Story.killQuest(3970, "northstar", "The Queen's Gift");

        // Defeat Karok!
        Story.killQuest(3971, "northstar", "Karok The Fallen");
    }

    frostRuins(): void {
        if (Core.isCompletedBefore(3954))
            return;
        this.glacera1();

        // Rescue the Refugees
        Story.killQuest(3951, "frozenruins", "Frost Invader");

        // Defeat the Fangbeasts
        Story.killQuest(3952, "frozenruins", "Frost Fangbeast");

        // Destroy the Frost Reapers
        Story.killQuest(3953, "frozenruins", "Frost Reaper");

        // FrostSpawn General Takedown
        Story.killQuest(3954, "frozenruins", "Frost General");
    }

    iceWindPass(): void {
        if (Core.isCompletedBefore(5601))
            return;
        this.frostRuins();

        // Where is Karok?
        Story.mapItemQuest(5587, "IceWindPass", 5074, 5);

        // Cloaking Spell
        Story.killQuest(5588, "IceWindPass", "Glacial Elemental");

        // Splattered Mana
        if (!Story.questProgression(5589)) {
            Core.ensureAccept(5589);
            Core.getMapItem(5075, 5, "IceWindPass");
            Core.huntMonster("icewindpass", "Glacial Elemental", "Iced Mana", 9);
            Core.ensureComplete(5589);
        }

        // Dispell the Spell
        Story.killQuest(5590, "IceWindPass", "Polar Golem");

        // Catch Up to Karok
        Story.killQuest(5591, "IceWindPass", "Frost Invader");

        // Blast the Frostspawn Symbiote
        Story.killQuest(5592, "IceWindPass", "Frostspawn Symbiote");

        // Keep Going!
        Story.killQuest(5593, "IceWindPass", "Frost Invader");

        // Take it Down
        Story.killQuest(5594, "IceWindPass", "Frostspawn Horror");

        // Keep the Frostspawn Away!
        if (!Story.questProgression(5595)) {
            Core.ensureAccept(5595);
            Core.huntMonster("icewindpass", "Frost Invader", "Frost Invader Slain", 5);
            Core.huntMonster("icewindpass", "Frostspawn Troll", "Frostspawn Troll Slain", 5);
            Core.ensureComplete(5595);
        }

        // Take a Break from Fighting
        if (!Story.questProgression(5596)) {
            Core.ensureAccept(5596);
            Core.huntMonster("icewindpass", "Polar Golem", "Living Snow", 3);
            Core.huntMonster("icewindpass", "Glacial Elemental", "Elemental Icicles", 3);
            Core.ensureComplete(5596);
        }

        // As these quests are pre-unlocked.. it wont do them if we put them under a quest check. annoyingly.

        //Fight For Kezeroth! && (Mega) Fight For Kezeroth! - 5597, 5598
        //Fight For Karok! && (Mega) Fight For Karok! - 5599, 5600
        Core.ensureAcceptmultiple(5597, 5598, 5599, 5600);
        Core.equipClass(ClassType.Farm);
        Core.huntMonster("icewindwar", "Glaceran Defender", "FrostSpawn Medal", 10);
        Core.huntMonster("icewindwar", "Glaceran Defender", "Mega Frostspawn Medal", 5);

        Core.huntMonster("icewindwar", "Frost Invader", "World Ender Medal", 10);
        Core.huntMonster("icewindwar", "Frost Invader", "Mega World Ender Medal", 5);
        Core.ensureCompleteQuests(5597, 5598, 5599, 5600);

        // What is THAT?
        Story.killQuest(5601, "icewindwar", "Soricomorpha");
    }

    iceDungeon(): void {
        if (Core.isCompletedBefore(7841))
            return;
        this.iceWindPass();

        // Cold Open
        if (!Story.questProgression(7832)) {
            Core.ensureAccept(7832);
            Core.huntMonster("icedungeon", "Ice Symbiote", "Ice Symbiote Shattered", 5);
            Core.huntMonster("icedungeon", "Frosted Banshee", "Frosted Banshee Cloth Scrap", 5);
            Core.huntMonster("icedungeon", "Frozen Undead", "Frozen Undead Laid to Rest", 5);
            Core.ensureComplete(7832);
        }

        // Frost Fight
        if (!Story.questProgression(7833)) {
            Core.ensureAccept(7833);
            Core.huntMonster("icedungeon", "Spirit of Ice", "Spirit of Ice Captured", 5);
            Core.huntMonster("icedungeon", "Ice Crystal", "Ice Crystal Collected", 5);
            Core.huntMonster("icedungeon", "Frigid Spirit", "Frigid Spirit Banished", 3);
            Core.ensureComplete(7833);
        }

        // Freeze Frame
        if (!Story.questProgression(7834)) {
            Core.ensureAccept(7834);
            Core.huntMonster("icedungeon", "Living Ice", "Living Ice Shattered", 5);
            Core.huntMonster("icedungeon", "Crystallized Elemental", "Crystalized Elemental Defeated", 5);
            Core.huntMonster("icedungeon", "Frozen Demon", "Frozen Demon Banished", 3);
            Core.ensureComplete(7834);
        }

        // Glace's Trial
        Story.killQuest(7835, "IceDungeon", "Image of Glace");

        // Abel's Trial
        Story.killQuest(7836, "IceDungeon", "Abel");

        // The Final Trial
        Story.killQuest(7837, "IceDungeon", "Shade of Kyanos");

        // Basic Ingredients
        if (!Story.questProgression(7838)) {
            Core.ensureAccept(7838);
            Core.huntMonster("icedungeon", "Frosted Banshee", "Frosted Banshee Defeated", 10);
            Core.huntMonster("icedungeon", "Frozen Undead", "Frozen Undead Defeated", 10);
            Core.huntMonster("icedungeon", "Ice Symbiote", "Ice Symbiote Defeated", 10);
            Core.ensureComplete(7838);
        }

        // Cool Flavor
        if (!Story.questProgression(7839)) {
            Core.ensureAccept(7839);
            Core.huntMonster("icedungeon", "Spirit of Ice", "Spirit of Ice Defeated", 10);
            Core.huntMonster("icedungeon", "Ice Crystal", "Ice Crystal Defeated", 10);
            Core.huntMonster("icedungeon", "Frigid Spirit", "Frigid Spirit Defeated", 10);
            Core.ensureComplete(7839);
        }

        // Chilled to Perfection
        if (!Story.questProgression(7840)) {
            Core.ensureAccept(7840);
            Core.huntMonster("icedungeon", "Living Ice", "Living Ice Defeated", 5);
            Core.huntMonster("icedungeon", "Crystallized Elemental", "Crystallized Elemental Defeated", 5);
            Core.huntMonster("icedungeon", "Frozen Demon", "Frozen Demon Defeated", 5);
            Core.ensureComplete(7840);
        }

        // Icing on the Cake
        if (!Story.questProgression(7841)) {
            Core.ensureAccept(7841);
            Core.huntMonster("icedungeon", "Image of Glace", "Glace's Approval", 1);
            Core.huntMonster("icedungeon", "Abel", "Abel's Approval", 1);
            Core.huntMonster("icedungeon", "Shade of Kyanos", "Kyanos' Approval", 1);
            Core.ensureComplete(7841);
        }
    }
}

export default function main(): void {
        Core.setOptions();

        new GlaceraStory().doAll();

        Core.setOptions(false);
}
