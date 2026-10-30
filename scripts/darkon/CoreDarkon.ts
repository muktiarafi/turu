/*
name: null
description: null
tags: null
*/
// Port of Scripts/Darkon/CoreDarkon.cs.
// Not ported (FarmerJoe's DoAll doesn't reach them): SecondErrand, ThirdErrand.

import { bot } from "turu";
import { ClassType, Core } from "../core/CoreBots";
import { CoreAstravia } from "../story/elegyofmadness(darkon)/CoreAstravia";

const Astravia = new CoreAstravia();

export class CoreDarkon {
    farmReceipt(Quantity = 222, MaxStack = false): void {
        this.firstErrand(Quantity, MaxStack);
    }

    firstErrand(Quantity = 222, MaxStack = false): void {
        const quest = Core.initializeWithRetries(() => Core.ensureLoad(7324));
        if (MaxStack)
            Quantity =
                quest?.rewards.find(x => x.name === "Darkon's Receipt")?.maxStack
                ?? Quantity;

        if (Core.checkInventory("Darkon's Receipt", Quantity))
            return;

        Core.addDrop("Darkon's Receipt");
        Core.farmingLogger("Darkon's Receipt", Quantity);
        Core.equipClass(ClassType.Farm);
        if (Core.checkInventory(52924)) {
            Core.logger("Preselling existing bananas as it can break.. somehow (AE fuckery?)");
            Core.sellItem(52924, 1, { all: true });
        }
        
        while (!bot.shouldExit && !Core.checkInventory("Darkon's Receipt", Quantity)) {
            Core.ensureAccept(7324);
            Core.killMonster("arcangrove", "LeftBack", "Left", "*", "Banana", 22, { isTemp: false });
            Core.ensureComplete(7324);
        }
        Core.cancelRegisteredQuests();
    }

    teeth(Quantity = 300, MaxStack = false): void {
        const quest = Core.initializeWithRetries(() => Core.ensureLoad(7780));
        if (MaxStack)
            Quantity = quest?.rewards.find(x => x.name === "Teeth")?.maxStack ?? Quantity;

        if (Core.checkInventory("Teeth", Quantity))
            return;

        Core.addDrop("Teeth");
        Astravia.eridani();
        Core.farmingLogger("Teeth", Quantity);
        Core.equipClass(ClassType.Solo);

        Core.registerQuests(7780);
        while (!bot.shouldExit && (!Core.checkInventory("Teeth", Quantity))) {
            Core.huntMonster("eridani", "Wolf-Like Creature", "Tooth", 28, { isTemp: false, log: false });
            bot.options.attackWithoutTarget = true;
            Core.killMonster("eridani", "r4", "Left", "Creature 15", "Wisdom Tooth", 4, { isTemp: false, log: false });
            bot.options.attackWithoutTarget = false;
            bot.wait.forPickup("Teeth");
        }
        Core.cancelRegisteredQuests();
    }

    lasGratitude(Quantity = 300, MaxStack = false): void {
        const quest = Core.initializeWithRetries(() => Core.ensureLoad(8001));
        if (MaxStack)
            Quantity =
                quest?.rewards.find(x => x.name === "La's Gratitude")?.maxStack
                ?? Quantity;

        if (Core.checkInventory("La's Gratitude", Quantity))
            return;

        Core.addDrop("La's Gratitude");
        Astravia.astravia();
        Core.farmingLogger("La's Gratitude", Quantity);
        Core.equipClass(ClassType.Solo);

        Core.registerQuests(8001);
        while (!bot.shouldExit && (!Core.checkInventory("La's Gratitude", Quantity))) {
            Core.huntMonster("astravia", "Creature 27", "Broken Dog Tag", 20, { log: false });
            Core.huntMonster("astravia", "Creature 27", "Intact Dog Tag", 5, { log: false });
            bot.wait.forPickup("La's Gratitude");
        }
        Core.cancelRegisteredQuests();
    }

    astravianMedal(Quantity = 300, MaxStack = false): void {
        const quest = Core.initializeWithRetries(() => Core.ensureLoad(8257));
        if (MaxStack)
            Quantity =
                quest?.rewards.find(x => x.name === "Astravian Medal")?.maxStack
                ?? Quantity;

        if (Core.checkInventory("Astravian Medal", Quantity))
            return;

        Core.addDrop("Astravian Medal");
        Astravia.astraviaCastle();
        Core.farmingLogger("Astravian Medal", Quantity);

        Core.registerQuests(8257);
        while (!bot.shouldExit && (!Core.checkInventory("Astravian Medal", Quantity))) {
            Core.equipClass(ClassType.Farm);
            bot.options.attackWithoutTarget = true;
            Core.huntMonster("astraviacastle", "Creature 27", "Defaced Portrait", 10, { log: false });
            Core.huntMonster("astraviacastle", "Creature 20", "Smashed Sculpture", 4, { log: false });
            bot.options.attackWithoutTarget = false;
            Core.equipClass(ClassType.Solo);
            Core.huntMonster("astraviacastle", "The Sun", "Burned Banana", 1, { log: false });
            bot.wait.forPickup("Astravian Medal");
        }
        Core.cancelRegisteredQuests();
    }

    aMelody(Quantity = 300, MaxStack = false): void {
        const quest = Core.initializeWithRetries(() => Core.ensureLoad(8396));
        if (MaxStack)
            Quantity =
                quest?.rewards.find(x => x.name === "A Melody")?.maxStack ?? Quantity;

        if (Core.checkInventory("A Melody", Quantity))
            return;

        Core.addDrop("A Melody");
        Astravia.astraviaJudgement();
        Core.farmingLogger("A Melody", Quantity);
        Core.registerQuests(8396);
        while (!bot.shouldExit && !Core.checkInventory("A Melody", Quantity)) {
            Core.equipClass(ClassType.Farm);
            Core.killMonster("astraviajudge", "r3", "Left", "*", "Brass", 10, { log: false });
            Core.killMonster("astraviajudge", "r2", "Left", "*", "Sinew", 10, { log: false });
            Core.equipClass(ClassType.Solo);
            Core.huntMonster("astraviajudge", "La", "Knight's Favor", 1, { log: false });
            bot.wait.forPickup("A Melody");
        }
        Core.cancelRegisteredQuests();
    }

    banditsCorrespondence(Quantity = 3000, MaxStack = false): void {
        const quest = Core.initializeWithRetries(() => Core.ensureLoad(8531));
        if (MaxStack)
            Quantity =
                quest?.rewards.find(x => x.name === "Bandit's Correspondence")?.maxStack
                ?? Quantity;

        if (Core.checkInventory("Bandit's Correspondence", Quantity))
            return;

        Core.addDrop("Bandit's Correspondence");
        Astravia.eridaniPast();
        Core.farmingLogger("Bandit's Correspondence", Quantity);

        Core.registerQuests(8531);
        while (!bot.shouldExit && !Core.checkInventory("Bandit's Correspondence", Quantity)) {
            Core.equipClass(ClassType.Farm);
            Core.killMonster("eridanipast", "r3", "Left", "Dog", "Dogs Confiscated", 12, { log: false });
            Core.killMonster("eridanipast", "r9", "Left", "Bandit", "Bandit Contraband", 12, { log: false });

            Core.equipClass(ClassType.Solo);
            Core.huntMonsterMapID("eridanipast", 19, "Seraphic Sparred", 1, { log: false });
            bot.wait.forPickup("Bandit's Correspondence");
        }
        Core.cancelRegisteredQuests();
    }

    sukisPrestiege(Quantity = 300, MaxStack = false): void {
        const quest = Core.initializeWithRetries(() => Core.ensureLoad(8602));
        if (MaxStack)
            Quantity =
                quest?.rewards.find(x => x.name === "Suki's Prestige")?.maxStack
                ?? Quantity;

        if (Core.checkInventory("Suki's Prestige", Quantity))
            return;

        Core.addDrop("Suki's Prestige");
        Astravia.completeCoreAstravia();
        Core.farmingLogger("Suki's Prestige", Quantity);

        Core.registerQuests(8602);
        while (!bot.shouldExit && (!Core.checkInventory("Suki's Prestige", Quantity))) {
            Core.equipClass(ClassType.Solo);
            Core.huntMonster("astraviapast", "Regulus", "Regulus' Rematch Won", 1, { log: false });
            Core.huntMonster("astraviapast", "Titania", "Titania's Rematch Won", 1, { log: false });
            Core.huntMonster("astraviapast", "Aurola", "Aurola's Rematch Won", 1, { log: false });
            Core.equipClass(ClassType.Farm);
            Core.huntMonster("astraviapast", "Astravian Soldier", "Soldiers Trained", 8, { log: false });
            bot.wait.forPickup("Suki's Prestige");
        }
        Core.cancelRegisteredQuests();
    }

    ancientRemnant(Quantity = 300, MaxStack = false): void {
        const quest = Core.initializeWithRetries(() => Core.ensureLoad(8641));
        if (MaxStack)
            Quantity =
                quest?.rewards.find(x => x.name === "Ancient Remnant")?.maxStack
                ?? Quantity;

        if (Core.checkInventory("Ancient Remnant", Quantity))
            return;

        Core.addDrop("Ancient Remnant");
        Astravia.firstObservatory();
        Core.farmingLogger("Ancient Remnant", Quantity);

        Core.registerQuests(8641);
        while (!bot.shouldExit && !Core.checkInventory("Ancient Remnant", Quantity)) {
            Core.jumpWait();
            Core.equipClass(ClassType.Farm);
            Core.killMonster("firstobservatory", "r7", "Left", "Ancient Creature", "Creature Samples", 6, { log: false });
            Core.killMonster("firstobservatory", "r6", "Left", "Ancient Turret", "Turret Pieces", 12, { log: false });
            Core.jumpWait();
            Core.equipClass(ClassType.Solo);
            Core.huntMonster(`firstobservatory`, "Empress’ Finger", "Alprecha Observed", 1, { log: false });
            bot.wait.forPickup("Ancient Remnant");
        }
        Core.cancelRegisteredQuests();
    }

    wheelofFortune(FlowerQuantity = 1000, ScaleQuantity = 1000, MaxStack = false): void {
        const quest = Core.initializeWithRetries(() => Core.ensureLoad(8688));
        if (quest == null) {
            Core.logger(
                "Quest 8688 not found, please ensure you have the Elegy of Madness story completed.");
            return;
        }
        if (MaxStack) {
            FlowerQuantity =
                quest.rewards.find(x => x.name === "Mourning Flower")?.maxStack
                ?? FlowerQuantity;
            ScaleQuantity =
                quest.rewards.find(x => x.name === "Jus Divinum Scale")?.maxStack
                ?? ScaleQuantity;
        }

        if (
            Core.checkInventory("Mourning Flower", FlowerQuantity)
            && Core.checkInventory("Jus Divinum Scale", ScaleQuantity)
        )
            return;

        if (FlowerQuantity > 0 && ScaleQuantity > 0) {
            Core.logger(
                `Farming Mourning Flower (${Core.dynamicQuant("Mourning Flower", false)}/${FlowerQuantity}) `
                    + `and Jus Divinum Scale (${Core.dynamicQuant("Jus Divinum Scale", false)}/${ScaleQuantity})`);
        }

        Core.addDrop("Mourning Flower", "Jus Divinum Scale");

        Astravia.genesisGarden();

        Core.registerQuests(8688);
        while (
            !bot.shouldExit && !Core.checkInventory("Mourning Flower", FlowerQuantity)
            || !Core.checkInventory("Jus Divinum Scale", ScaleQuantity)
        ) {
            Core.equipClass(ClassType.Farm);
            Core.huntMonster("genesisgarden", "Long-eared Beast", "Beast Subject", 7, { log: false });
            Core.huntMonster("genesisgarden", "Undead Humanoid", "Humanoid Subject", 7, { log: false });
            Core.equipClass(ClassType.Solo);
            Core.huntMonster("genesisgarden", "Ancient Mecha", "Replacement Parts", 7, { log: false });
        }
        Core.cancelRegisteredQuests();
    }

    unfinishedMusicalScore(Quantity = 300): void {
        if (Core.checkInventory("Unfinished Musical Score", Quantity))
            return;

        bot.quests.updateQuest(8733);
        Core.equipClass(ClassType.Solo);

        Core.huntMonster("theworld", "Encore Darkon", "Unfinished Musical Score", Quantity, { isTemp: false });
        bot.wait.forPickup("Unfinished Musical Score");
    }
}
