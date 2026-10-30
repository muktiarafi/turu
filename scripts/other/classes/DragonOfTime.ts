/*
name: DragonOfTime
description: This bot farms the Dragon of Time class for you.
tags: class, dot, damage over time, dragon of time, timeinn
*/
// Port of Scripts/Other/Classes/DragonOfTime.cs.
// Not ported (FarmerJoe's DoAll doesn't reach them): DotArmor.

import { bot } from "turu";
import { CoreAdvanced } from "../../core/CoreAdvanced";
import { ClassType, Core } from "../../core/CoreBots";
import { CoreFarms } from "../../core/CoreFarms";
import { CoreStory } from "../../core/CoreStory";
import { CoreDarkon } from "../../darkon/CoreDarkon";
import { Borgars } from "../../story/Borgars";
import { CoreQOM } from "../../story/queenofmonsters/CoreQoM";
import { CoreSummer } from "../../story/summer2015adventuremap/CoreSummer";
import { CoreToD } from "../../story/throneofdarkness/CoreToD";
import { MysteriousEgg } from "../MysteriousEgg";
import { GoldenBladeOfFate } from "../weapons/GoldenBladeOfFate";
import { PinkBladeOfDestruciton } from "../weapons/PinkBladeofDestruction";

const Farm = new CoreFarms();
const Adv = new CoreAdvanced();
const Story = new CoreStory();
const Darkon = new CoreDarkon();
const GBoF = new GoldenBladeOfFate();
const PBoD = new PinkBladeOfDestruciton();
const QOM = new CoreQOM();
const TOD = new CoreToD();
const Egg = new MysteriousEgg();
const Coll = new CoreSummer();
const borgars = new Borgars();

export class DragonOfTime {
    private extras: string[] = [
        "Dragon of Time Horns",
        "Dragon of Time Horns + Ponytail",
        "Dragon of Time Wings + Tail",
    ];

    getDoT(rankUpClass = true, doExtra = true, DotArmorOnly = false): void {
        if (doExtra ? Core.checkInventory(this.extras) : Core.checkInventory("Dragon of Time"))
            return;

        Story.preLoad(this.getDoT);

        // Acquiring Ancient Secrets 7716
        if (!Story.questProgression(7716)) {
            Core.ensureAccept(7716);

            Core.equipClass(ClassType.Farm);
            bot.quests.updateQuest(4614);
            Core.killMonster("mummies", "Enter", "Spawn", "Mummy", "Lost Hieroglyphic", 30, { isTemp: false });

            Farm.loremasterREP(4);

            Core.buyItem("librarium", 651, "Myths of Lore");

            Core.killMonster("timelibrary", "FrameAQ", "Left", "*", "Historia Page", 100, { isTemp: false });

            Core.equipClass(ClassType.Solo);
            Core.huntMonster("kingcoal", "Frost King", "Frost King's Story", 1, { isTemp: false });

            Core.killMonster("baconcatyou", "Enter", "Spawn", "*", "Your Own Memories", 1, { isTemp: false, publicRoom: true });
            Story.chainQuest(7716);
            bot.wait.forPickup("*");
            Core.toBank("Dragon of Time Helm", "Dragon of Time Ponytail");
        }

        // Time to Train Yourself 7717
        if (!Story.questProgression(7717)) {
            Core.ensureAccept(7717);

            Core.equipClass(ClassType.Solo);
            Core.huntMonster("dragonchallenge", "Desoloth the Final", "Desoloth's Destructive Aura", 1, { isTemp: false, publicRoom: true });

            bot.quests.updateQuest(899);
            Core.huntMonster("blindingsnow", "Nythera", "Nythera's Patience", 1, { isTemp: false });

            Core.addDrop("Key of Greed");
            Core.huntMonster("greed", "Goregold", "Goregold's Luck", 1, { isTemp: false, publicRoom: true });
            Core.huntMonster("darkplane", "Victorious", "Victorious's Dignity", 1, { isTemp: false });

            Core.killTrigoras("Trigoras's Tenacity", 3, 2);

            Story.chainQuest(7717);
            bot.wait.forPickup("*");
            Core.toBank("Dragon of Time Runes", "Dragon of Time Wings", "Dragon of Time Tail");
        }

        // Do You Have the Time? 7718
        if (!Story.questProgression(7718)) {
            Farm.experience(31);
            Core.ensureAccept(7718);

            GBoF.getGBoF();

            PBoD.getPBoD();

            Core.equipClass(ClassType.Solo);
            Core.huntMonster("underworld", "Laken", "Cross-Era Stabilizer", 1, { isTemp: false });

            if (!Core.checkInventory("Chronomancer's Codex")) {
                Core.huntMonster("mqlesson", "Dragonoid", "Dragonoid of Hours", 1, { isTemp: false });
                Core.huntMonster("timespace", "Chaos Lord Iadoa", "Chronomancer's Codex", 1, { isTemp: false });
            }

            Core.equipClass(ClassType.Farm);
            Core.huntMonster("arena", "Timestream Rider", "Timestream String", 100, { isTemp: false });

            Story.chainQuest(7718);
            bot.wait.forPickup("*");
            Core.toBank("Dragon of Time FangBlade", "Dual Dragon of Time FangBlades");
        }

        // Through the Wormhole 7719
        if (!Story.questProgression(7719)) {
            Farm.experience(40);
            Core.ensureAccept(7719);

            Core.equipClass(ClassType.Solo);
            Core.huntMonster("cathedral", "Incarnation of Time", "Time Loop Broken", 1, { isTemp: false, publicRoom: true });

            Core.equipClass(ClassType.Farm);
            Core.killMonster("portalwar", "r4", "Right", "*", "Anomaly Silenced", 100, { isTemp: false });

            Core.huntMonster("portalmaze", "ChronoLord", "Chronolord Stopped", 50, { isTemp: false });

            Core.equipClass(ClassType.Solo);
            Core.huntMonster("ubear", "Cornholio", "Is This a Wormhole?", 1, { isTemp: false });

            Core.ensureComplete(7719);
        }

        // If we are only getting the armor, we can return here.
        if (DotArmorOnly)
            return;

        // Rend 7720
        if (!Story.questProgression(7720)) {
            Farm.experience(60);
            Core.ensureAccept(7720);

            Core.equipClass(ClassType.Solo);
            Core.huntMonster("lairdefend", "Dragon Summoner", "Dimensional Dragon Portal", 2, { isTemp: false });
            Core.huntMonster("bosschallenge", "Grievous Inbunche", "Brutal Slash Studied", 10, { isTemp: false, publicRoom: true });
            Core.killMonster("hydrachallenge", "h90", "Left", "*", "Epic Hydra Fang", 125, { isTemp: false });

            Story.chainQuest(7720);
            bot.wait.forPickup("*");
            Core.toBank("Dragon of Time Daggers", "Dragon of Time Cleaver");
        }

        // Confluence of Fates 7721
        if (!Story.questProgression(7721)) {
            Farm.experience(60);
            Core.ensureAccept(7721);

            Core.equipClass(ClassType.Solo);
            Core.huntMonster("ivoliss", "Ivoliss", "Sword of Voids", 1, { isTemp: false });
            bot.wait.forPickup("Sword of Voids");

            Darkon.farmReceipt(100);

            QOM.theReshaper();
            if (!Core.checkInventory("Semiramis Feather")) {
                Core.addDrop("Semiramis Feather");
                // Take Down Terrane 6286
                Core.equipClass(ClassType.Solo);
                Core.ensureAccept(6286);
                Core.killMonster("guardiantree", "r12", "Left", "Terrane");
                Core.ensureComplete(6286);
                bot.wait.forPickup("Semiramis Feather");
            }

            Core.equipClass(ClassType.Farm);
            Core.killMonster("aqw3d", "r13", "Bottom", "*", "Cross-Dimensional Weapons", 300, { isTemp: false, publicRoom: true });
            TOD.shiftingPyramid();
            if (!Core.checkInventory("Starlight Singularity")) {
                Core.addDrop("Starlight Singularity");
                // Serpent of the Stars 5186
                Core.ensureAccept(5186);
                Core.equipClass(ClassType.Solo);
                Core.huntMonster("whitehole", "Mehensi Serpent", "Mehen Slain");
                Core.ensureComplete(5186);
                bot.wait.forPickup("Starlight Singularity");
            }

            Coll.collector();
            Adv.buyItem("collection", 325, 56815, 1, 6448);
            Story.chainQuest(7721);

            bot.wait.forPickup("*");
            Core.toBank("Ascended Dragon of Time Runes", "Runes Of Time");
        }

        // Dragon's Will 7722
        if (!Story.questProgression(7722)) {
            Farm.experience(65);
            Core.ensureAccept(7722);

            Core.equipClass(ClassType.Farm);
            Core.huntMonster("moonlab", "Slime Mold", "Unyielding Slime", 300, { isTemp: false });

            Core.equipClass(ClassType.Solo);
            Core.huntMonster("bosschallenge", "Mutated Void Dragon", "Omnipotent Cells", 20, { isTemp: false, publicRoom: true });
            if (!Core.checkInventory("Dragon's Plasma", 20)) {
                //why the fuck was the class buffed!?
                const usethis = bot.inventory.items.concat(bot.bank.items).find(n => n.name === "Yami no Ronin" || n.name.startsWith("Chaos Slayer"));
                if (usethis != null) {
                    Core.equip(usethis.id);
                    Core.equip(...Core.farmGear);
                    if (usethis.name === "Yami no Ronin")
                        bot.skills.startAdvanced("1 | 4");
                } else
                    Core.equipClass(ClassType.Dodge);
                Core.huntMonster("underlair", "ArchFiend Dragonlord", "Dragon's Plasma", 20, { isTemp: false });
            }
            Core.jumpWait();
            Core.equipClass(ClassType.Solo);

            Core.huntMonster("chaoskraken", "Chaos Kraken", "Chaotic Invertebrae", 20, { isTemp: false, publicRoom: true });

            bot.quests.updateQuest(9, 159);
            Core.huntMonster("towerofdoom9", "Dread Fang", "Cryostatic Essence", 20, { isTemp: false, publicRoom: true });

            Core.huntMonster("castleroof", "Ultra Chaos Dragon", "Salvaged Chaos Dragon Biomass", 20, { isTemp: false, publicRoom: true });
            Story.chainQuest(7722);
            bot.wait.forPickup("*");
            Core.toBank("Dragon of Time Reaper", "Dragon of Time WingBlade");
        }

        // Burning Fates 7723
        if (!Story.questProgression(7723)) {
            Farm.experience(70);
            Core.ensureAccept(7723);

            Core.equipClass(ClassType.Farm);
            Core.killMonster("volcano", "r10", "Left", "Fire Imp", "Fire Essence", 3000, { isTemp: false });

            Core.equipClass(ClassType.Solo);
            Core.huntMonster("charredplains", "Akriloth", "Akriloth's Flametongue", 100, { isTemp: false, publicRoom: true });
            Core.huntMonster("ultraphedra", "Ultra Phedra", "Immortal Embers", 50, { isTemp: false, publicRoom: true });
            Core.huntMonster("thevoid", "Reaper", "Ashes from the Void Realm", 50, { isTemp: false, publicRoom: true });
            Story.chainQuest(7723);
            bot.wait.forPickup("*");
            Core.toBank("Ascended Dragon of Time");
        }

        // Hero's Heartbeat 7724
        if (!Story.questProgression(7724) || !Core.checkInventory("Dragon of Time")) {
            Farm.experience(75);
            if (!Core.checkInventory("Blade of Awe"))
                Farm.bladeofAweREP(6, true);
            Core.ensureAccept(7724);

            Egg.getMysteriousEgg();

            Core.equipClass(ClassType.Solo);
            const chaosProgress = bot.flash.callGameFunction<number>("world.getQuestValue", 182);
            try {
                bot.quests.updateQuest(3880);
                Core.killMonster("chaoslord", "r2", "Left", "*", "Conquered Past", 1, { isTemp: false, publicRoom: true });
            } finally {
                bot.quests.updateQuest(chaosProgress, 182);
            }
            bot.quests.updateQuest(10, 159);
            Core.huntMonster("towerofdoom10", "Slugbutter", "Slugbutter Trophy", 100, { isTemp: false, publicRoom: true });
            Core.equipClass(ClassType.Dodge);
            Core.huntMonster("icewing", "Warlord Icewing", "Icewing's Laurel", 30, { isTemp: false, publicRoom: true });
            Story.chainQuest(7724);
            bot.wait.forPickup("Dragon of Time");
            bot.wait.forPickup("*");
            Core.toBank("Ascended Dragon of Time Morph", "Ascended Dragon of Time Wings");
        }

        if (rankUpClass)
            Adv.rankUpClass("Dragon of Time");

        // I'm Loving It My Way 7725
        if (doExtra) {
            borgars.storyLine();
            Farm.experience(75);
            Core.addDrop("Burger Buns");
            Core.equipClass(ClassType.Solo);
            bot.drops.add(
                ...this.extras.filter(x => Core.checkInventory(x, 1, { toInv: false })).map(x => x));
            for (const Item of Core.questRewards(7725)
                    .filter(x => !Core.checkInventory(x, 1, { toInv: false }))
                    .map(x => x)
            ) {
                Core.ensureAccept(7725);
                if (!Core.checkInventory("Borgar")) {
                    while (!bot.shouldExit && !Core.checkInventory("Burger Buns", 5)) {
                        // Burglinster's Revenge 7522
                        Core.ensureAccept(7522);
                        Core.huntMonster("borgars", "Burglinster", "Burglinster Cured");
                        Core.ensureComplete(7522);
                        bot.wait.forPickup("Burger Buns");
                    }
                    Core.buyItem("borgars", 1884, 54650, 1, 7387);
                }

                Core.ensureCompleteChoose(7725, this.extras);
                bot.wait.forPickup("*");
            }
            Core.toBank(
                "Dragon of Time Horns", 
                "Dragon of Time Horns + Ponytail", 
                "Dragon of Time Wings + Tail");
        }
    }
}

export default function main(): void {
        Core.setOptions();

        new DragonOfTime().getDoT();

        Core.setOptions(false);
}
