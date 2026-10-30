/*
name: Citadel Ruins
description: This will finish the Citadel Ruins quest.
tags: story, quest, nation, citadel-ruins
*/
// Port of Scripts/Story/Nation/CitadelRuins.cs.

import { bot } from "turu";
import { ClassType, Core } from "../../core/CoreBots";
import { CoreFarms } from "../../core/CoreFarms";
import { CoreStory } from "../../core/CoreStory";
import { CoreNation } from "../../nation/CoreNation";

const Farm = new CoreFarms();
const Nation = new CoreNation();
const Story = new CoreStory();

export class CitadelRuins {
    rewards: string[] = [
        "Purified Claymore of Destiny",
        "Good Iron Wing 1",
        "Spinal Tap of Retribution",
        "Purified Claw of Nulgath",
        "Mage's Gratitude",
    ];

    doAll(): void {
        Farm.experience(30);
        Farm.goodREP();
        this.murrysQuests();
        this.polishsQuestsTercessuinotlim();
        this.polishsQuestsCitadelRuins();
        this.cutieMakaisQuest();
        this.casparillasQuests();
    }

    casparillasQuests(): void {
        if (Core.isCompletedBefore(6682))
            return;

        Story.preLoad(this.casparillasQuests);

        // Easy AND Breezy!
        Story.killQuest(6669, "forest", "Boss Zardman");

        // Disguises and a Ruse
        Story.killQuest(6671, "bludrut2", "Shadow Creeper");

        // Icky and Smelly
        Story.killQuest(6672, "marsh2", "Thrax Ironhide");

        // Dress for the Occasion!
        Story.killQuest(6673, "sleuthhound", "Harmoire");

        // Can't Play Games without A Lunch Break
        Story.killQuest(6674, "noobshire", "Kittarian Mouse Eater");

        // She Needs a Hero
        if (!Story.questProgression(6675)) {
            bot.quests.updateQuest(3008);
            Core.ensureAccept(6675);
            Core.huntMonsterMapID("doomvault", 21, "Your Princess is Not in This Castle");
            Core.ensureComplete(6675);
        }

        // Sun and Fun
        Story.killQuest(6676, "beachparty", "Sun Flare");

        // Heebie Jeebies
        Story.killQuest(6677, "marsh", "Dreadspider");

        // A New Pet
        Story.killQuest(6678, "iceplane", "Enfield");

        // Spoiled Dragon
        Story.killQuest(6679, "lair", "Red Dragon");

        // The Unpredictable Element
        Story.killQuest(6680, "ledgermayne", "Ledgermayne");

        // There, But Not There
        Story.killQuest(6681, "palace", "Invisible");

        // Staying Humble
        Core.addDrop("Elite Void Sword Pet");
        //why the fuck was the class buffed!?
        const usethis = bot
            .inventory.items.concat(bot.bank.items)
            .find(n =>
                n.name === "Yami no Ronin" || n.name.startsWith("Chaos Slayer"));
        if (usethis != null) {
            Core.equip(usethis.id);
            Core.equip(...Core.farmGear);
            if (usethis.name === "Yami no Ronin")
                bot.skills.startAdvanced("1 | 4");
        } else
            Core.equipClass(ClassType.Dodge);
        Story.killQuest(6682, "underlair", "ArchFiend DragonLord");
    }

    cutieMakaisQuest(): void {
        if (Core.isCompletedBefore(4325))
            return;

        Story.preLoad(this.cutieMakaisQuest);

        // Drearia On Demand
        if (!Story.questProgression(4312)) {
            Core.ensureAccept(4312);
            Core.getMapItem(3485, 1, "drearia");
            Core.huntMonster("drearia", "Dark Makai", "Dark Makai Notes", 1);
            Core.huntMonster("drearia", "Evil Elemental", "Evil Elemental Notes", 1);
            Core.huntMonster("drearia", "Green Rat", "Green Rat Notes", 1);
            Core.ensureComplete(4312);
        }

        // Plant a Little Seed and Nature Grows
        Story.killQuest(4313, "drearia", "Dark Makai");

        // A Key Discovery
        if (!Story.questProgression(4314)) {
            Core.ensureAccept(4314);
            Core.getMapItem(3466, 1, "drearia");
            Core.huntMonster("drearia", "Green Rat", "Front Door Key", 1);
            Core.ensureComplete(4314);
        }

        // Creepy House... Yay!
        if (!Story.questProgression(4315)) {
            Core.ensureAccept(4315);
            Core.getMapItem(3467, 1, "drearia");
            Core.huntMonster("drearia", "Green Rat", "Defeated Varmint", 3);
            Core.ensureComplete(4315);
        }

        // Sparkling Books
        Story.mapItemQuest(4316, "drearia", 3468);

        // A Paladin in Peril
        Story.mapItemQuest(4317, "swordhavenpink", 3469);

        // Pink Stinks!
        if (!Story.questProgression(4318)) {
            Core.ensureAccept(4318);
            Core.getMapItem(3486, 5, "swordhavenpink");
            Core.huntMonster("swordhavenpink", "Pink Slime", "Squished Pink Slime", 8);
            Core.ensureComplete(4318);
        }

        // Rats, RATS!
        Story.killQuest(4319, "sewerpink", "Pink Rat");

        // AdventureQuest Worm
        Story.killQuest(4320, "sewerpink", "Cutie Grumbley");

        // UnBEARable Sight
        if (!Story.questProgression(4321)) {
            Core.ensureAccept(4321);
            Core.getMapItem(3470, 1, "pinewoodpink");
            Core.huntMonster("pinewoodpink", "Pink Grizzly", "Pink Teddy Bear", 5);
            Core.ensureComplete(4321);
        }

        // Too Much Pink in Pinewood!
        if (!Story.questProgression(4322)) {
            Core.ensureAccept(4322);
            Core.getMapItem(3471, 5, "pinewoodpink");
            Core.huntMonster("pinewoodpink", "Pink Shell Turtle", "Bows", 5);
            Core.ensureComplete(4322);
        }

        // Kill Sparkletooth
        Story.killQuest(4323, "pinewoodpink", "Sparkletooth");

        // The Citadorable Plot
        Story.mapItemQuest(4324, "Citadel", 3472);

        // Fuzzy Run Minigame
        Story.killQuest(4325, "pinewoodpink", "Pink Grizzly");
    }

    murrysQuests(): void {
        if (Core.isCompletedBefore(182))
            return;

        Story.preLoad(this.murrysQuests);

        // "Citadel"
        Story.killQuest(144, "Citadel", "Inquisitor Guard");
        Story.killQuest(145, "Citadel", "Crusader");
        Story.killQuest(146, "Citadel", "Inquisitor Captain");
        Story.killQuest(147, "Citadel", "Burning Witch");
        Story.killQuest(148, "Citadel", "Inquisitor Guard");
        Story.killQuest(149, "Citadel", "Inquisitor Guard", { getReward: false });
        Story.killQuest(181, "Citadel", "Belrot The Fiend");
        if (!Story.questProgression(182)) // 151 dcs you when u try to load it, and can be skipped.. somehow idk itd break otherwise.
        {
            Core.ensureAccept(182);
            Core.huntMonster("Citadel", "Grand Inquisitor", "Ring of Evil Intent");
            Core.ensureComplete(182);
        }
    }

    polishsQuestsTercessuinotlim(): void {
        if (Core.isCompletedBefore(668))
            return;

        Story.preLoad(this.polishsQuestsTercessuinotlim);

        Core.addDrop(
            "Unidentified 9", 
            "Unidentified 28", 
            "Dark Crystal Shard", 
            "Claw of Nulgath", 
            "Relic of Chaos");

        // "Tercessuinotlim"

        //iron wing helm enchant=
        Nation.theAssistant("Unidentified 9");
        Story.killQuest(560, "underworld", "Undead Bruiser", { getReward: false });

        //cleansing of spinal tap=
        Nation.supplies("Dark Crystal Shard", 5);
        Nation.theAssistant("Unidentified 28");
        Story.killQuest(585, "Tercessuinotlim", "Legion Fenrir");

        //purified claw
        if (!Core.checkInventory("Purified Claw of Nulgath")) {
            Core.addDrop("Purified Claw of Nulgath");
            Core.ensureAccept(668);
            Nation.farmTaintedGem(7);
            Nation.supplies("Claw of Nulgath");

            Core.resetQuest(7551);
            Core.darkMakaiItem("Dark Makai Sigil");
            Core.ensureComplete(668);
        }
    }

    polishsQuestsCitadelRuins(): void {
        if (Core.isCompletedBefore(6182))
            return;

        Story.preLoad(this.polishsQuestsCitadelRuins);

        Core.addDrop("Mage's Gratitude");
        // "citadelruins";

        //get ready to amplify
        if (!Story.questProgression(6172)) {
            Core.ensureAccept(6172);
            Core.getMapItem(5592, 1, "citadelruins");
            Core.huntMonster("citadelruins", "Mana Sprites", "Siphoned Mana", 8);
            Core.ensureComplete(6172);
        }

        //break the seal
        Story.mapItemQuest(6173, "citadelruins", 5602);

        //clear out the squatters
        if (!Story.questProgression(6174)) {
            Core.ensureAccept(6174);
            Core.huntMonster("citadelruins", "Inquisitor Hobo", "Inquisitor Hobos Defeated", 8);
            Core.huntMonster("citadelruins", "Inquisitor Hobo", "Inquisitors' Manifesto", 5);
            Core.ensureComplete(6174);
        }

        //grab a clue
        if (!Story.questProgression(6175)) {
            Core.ensureAccept(6175);
            Core.getMapItem(5593, 5, "citadelruins");
            Core.huntMonster("citadelruins", "Inquisitor Hobo", "Clue Found", 1);
            Core.ensureComplete(6175);
        }
        Core.sleep(2500);

        //unlock the door
        if (!Story.questProgression(6176)) {
            Core.ensureAccept(6176);
            Core.huntMonster("citadelruins", "Inquisitor Heavy", "Heavies Beat Down", 2);
            Core.huntMonster("citadelruins", "Inquisitor Heavy", "Citadel Key");
            Story.mapItemQuest(6176, "citadelruins", 5603);
        }

        //search the labs
        if (!Story.questProgression(6177)) {
            Core.ensureAccept(6177);
            Core.getMapItems([ [5594, 1], [5595, 1], [5596, 1] ], "citadelruins");
            Core.huntMonster("citadelruins", "Inquisitor Hobo", "Inquisitors Defeated", 6);
            Core.ensureComplete(6177);
        }

        //break another seal
        if (!Story.questProgression(6178)) {
            Core.ensureAccept(6178);
            Core.getMapItem(5597, 1, "citadelruins");
            Core.huntMonster("citadelruins", "Mana Sprites", "Siphoned Mana", 8);
            Core.ensureComplete(6178);
        }

        //inspect time
        Story.mapItemQuest(6179, "citadelruins", 5598);

        //find the parts
        if (!Story.questProgression(6180)) {
            Core.ensureAccept(6180);
            Core.getMapItems([ [5599, 1], [5600, 1], [5601, 1] ], "citadelruins");
            Core.huntMonster("citadelruins", "Inquisitor Hobo", "Missing Parts", 6);
            Core.ensureComplete(6180);
        }

        //defeat the grand inquisitor
        Story.killQuest(6181, "citadelruins", "Grand Inquisitor Murry");

        //defeat enn'tropy
        Story.killQuest(6182, "citadelruins", "Enn'tröpy");
        Core.toBank(...this.rewards);
    }
}

export default function main(): void {
        Core.setOptions();
        new CitadelRuins().doAll();

        Core.setOptions(false);
}
