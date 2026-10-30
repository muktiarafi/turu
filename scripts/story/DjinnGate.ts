/*
name: Djinn Gate Story
description: This will finish the Djinn Gate story.
tags: story, quest, djinn-gate
*/
// Port of Scripts/Story/DjinnGate.cs.

import { CoreAdvanced } from "../core/CoreAdvanced";
import { ClassType, Core } from "../core/CoreBots";
import { CoreStory } from "../core/CoreStory";

const Story = new CoreStory();
const Adv = new CoreAdvanced();

export class DjinnGateStory {
    private drops: string[] = [
        "Armor of Zular",
        "Djinn's Essence",
        "Unseen Essence",
        "Fangs of the Lion",
        "Claws of the Daeva",
        "Light of the Serpent",
        "Pike of the Shimmering Sands",
        "Reavers of the Gilded Sun",
    ];

    djinnGate(): void {
        if (Core.isCompletedBefore(7312) && Core.isCompletedBefore(6161))
            return;

        Story.preLoad(this.djinnGate);

        Core.addDrop(...this.drops);

        //Recovering the Fangs of the Lion 6153
        if (!Story.questProgression(6153)) {
            Core.ensureAccept(6153);

            if (!Core.checkInventory("Fragment 1") || !Core.checkInventory("Fragment 4")) {
                Core.joinSWF("mobius", "ChiralValley/town-Mobius-21Feb14.swf");
                Core.killMonster("mobius", "Slugfit", "Bottom", "Slugfit", "Fragment 1", 1, { log: false });
                Core.killMonster("mobius", "Slugfit", "Bottom", "Cyclops Warlord", "Fragment 4", 1, { log: false });
            }
            Core.killMonster("faerie", "TopRock", "Left", "*", "Fragment 2");
            Core.killMonster("faerie", "Side4", "Right", "*", "Fragment 3");
            Core.killMonster("cornelis", "Side1", "Left", "*", "Fragment 5");
            Core.ensureComplete(6153);
            Core.toBank(...this.drops);
        }

        //Recovering the Claws of the Daeva 6154
        if (!Story.questProgression(6154)) {
            Core.ensureAccept(6154);
            Core.killMonster("arcangrove", "Left", "Left", "*", "Fragment 6");
            Core.killMonster("cloister", "r8", "Left", "*", "Fragment 7");
            Core.killMonster("gilead", "Fail", "Right", "Bubblin", "Fragment 8");
            Core.killMonster("natatorium", "r3", "Left", "Merdraconian", "Fragment 9");
            Core.killMonster("mafic", "r2", "Left", "*", "Fragment 10");
            Core.ensureComplete(6154);
            Core.toBank(...this.drops);
        }

        //Recovering the Light of the Serpent 6155
        if (!Story.questProgression(6155)) {
            Core.ensureAccept(6155);
            Core.killMonster("mythsong", "Hill", "Right", "*", "Fragment 11");
            Core.huntMonster("palooza", "Rock Lobster", "Fragment 12");
            Core.huntMonster("palooza", "Stinger", "Fragment 13");
            Core.huntMonster("beehive", "Killer Queen Bee", "Fragment 14");
            Core.huntMonster("palooza", "Mozard", "Fragment 15");
            Core.ensureComplete(6155);
            Core.toBank(...this.drops);
        }

        //Recovering the Pike of the Shimmering Sands 6156
        if (!Story.questProgression(6156)) {
            Core.ensureAccept(6156);
            Core.killMonster("forestchaos", "Boss", "Left", "*", "Fragment 16");
            Core.killMonster("guru", "Field2", "Left", "*", "Fragment 17");
            Core.killMonster("marsh", "Forest3", "Left", "Dark Witch", "Fragment 18");
            Core.killMonster("marsh", "Forest3", "Left", "Spider", "Fragment 19");
            Core.killMonster("marsh2", "End", "Left", "Soulseeker", "Fragment 20");
            Core.ensureComplete(6156);
            Core.toBank(...this.drops);
        }

        //Recovering the Reavers of the Gilded Sun 6157
        if (!Story.questProgression(6157)) {
            Core.ensureAccept(6157);
            Core.killMonster("pirates", "End", "Left", "Shark Bait", "Fragment 21");
            Core.killMonster("pirates", "End", "Left", "Fishwing", "Fragment 25");
            Core.killMonster("yokairiver", "r2", "Left", "Kappa Ninja", "Fragment 22");
            Core.killMonster("bamboo", "Enter", "Spawn", "*", "Fragment 23");
            Core.killMonster("yokaiwar", "War2", "Left", "Samurai Nopperabo", "Fragment 24");
            Core.ensureComplete(6157);
            Core.toBank(...this.drops);
        }

        //Potent Mana 6158
        if (!Story.questProgression(6158)) {
            Core.ensureAccept(6158);
            Core.equipClass(ClassType.Solo);
            Adv.gearStore(false, { enhAfter: true });
            Core.killDoomKitten("Potent DoomKitten Mana", 1, { isTemp: true });
            Adv.gearStore(true, { enhAfter: true });
            Core.killMonster("bloodtitan", "Ultra", "Left", "*", "Potent Blood Titan Mana");
            Core.killTrigoras("Potent Trigoras Mana", 1, 1, true);
            Core.killMonster("phoenixrise", "r8", "Left", "*", "Potent CinderClaw Mana");
            Core.killMonster("thevoid", "r16", "Left", "*", "Potent Reaper Mana");
            Core.ensureComplete(6158);
        }

        //Break the Seal 6159
        Story.mapItemQuest(6159, "djinngate", 5571, 5, { getReward: false });

        if (!Story.questProgression(6160)) {
            Core.equipClass(ClassType.Farm);
            Core.ensureAccept(6160);
            Core.huntMonster("djinngate", "Harpy", "Djinn's Essence", 100, { isTemp: false });
            Core.ensureComplete(6160);
        }

        //Armor of Zular 6160
        if (!Story.questProgression(6160)) {
            Core.equipClass(ClassType.Solo);
            Core.ensureAccept(6161);
            Core.huntMonster("djinngate", "Gedoz", "Gedoz the Malignant Defeated");
            Core.ensureComplete(6161);
        }

        //Gedoz, Geddout! 6161
        Story.killQuest(6161, "DjinnGate", "Gedoz");

        //Quest for Resources 6162
        if (!Story.questProgression(6162)) {
            Core.ensureAccept(6162);
            Core.huntMonster("djinngate", "Harpy", "Potent Harpy Mana", 2);
            Core.huntMonster("djinngate", "Lamia", "Potent Lamia Mana", 2);
            Core.ensureComplete(6162);
        }

        //The Key of Smoke 7301
        Story.killQuest(7301, "AshfallCamp", "Smoldur");

        //The Key of Flame 7302
        Story.killQuest(7302, "LavaRun", "Phedra");

        //The Key of Mist 7303
        Story.killQuest(7303, "Marsh", "Dark Witch");

        //The Key of Air 7304
        Story.killQuest(7304, "AirStorm", "Energy Tornado");

        //The Key of Earth 7305
        Story.killQuest(7305, "DragonPlane", "Moganth");

        //The Key of Stone 7306
        Story.killQuest(7306, "MountainPath", "Balboa");

        //The Key of Water 7307
        Story.killQuest(7307, "Natatorium", "Marianus");

        //The Key of Ice 7308
        Story.killQuest(7308, "IcePlane", "Frostblade");

        //The Key of Sand 7309
        Story.killQuest(7309, "Pyramid", "Mummy");

        //The Key of Metal 7310
        Story.killQuest(7310, "DreadSpace", "Dread Space");

        //The Key of Shadow 7311
        Story.killQuest(7311, "ShadowVault", "Ancient Doomknight");

        //The Key of Light 7312
        Story.killQuest(7312, "CelestialArenaD", "Queen of Hope");

        Core.toBank(...this.drops);
    }
}

export default function main(): void {
        Core.setOptions();

        new DjinnGateStory().djinnGate();

        Core.setOptions(false);
}
