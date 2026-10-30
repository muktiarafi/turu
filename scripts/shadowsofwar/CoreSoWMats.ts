/*
name: null
description: null
tags: null
*/
// Port of Scripts/ShadowsOfWar/CoreSoWMats.cs.
// Not ported (FarmerJoe's DoAll doesn't reach them): DragonsTear.

import { bot } from "turu";
import { CoreAdvanced } from "../core/CoreAdvanced";
import { ClassType, Core } from "../core/CoreBots";
import { CoreSoW } from "../story/shadowsofwar/CoreSoW";

const Adv = new CoreAdvanced();
const SoW = new CoreSoW();

export class CoreSoWMats {
    acquiescence(Quantity = 1000): void {
        if (Core.checkInventory("Acquiescence", Quantity))
            return;

        SoW.shadowFlame();

        Core.farmingLogger("Acquiescence", Quantity);
        Core.addDrop("Acquiescence");
        Core.equipClass(ClassType.Solo);

        Core.registerQuests(8966);
        while (!bot.shouldExit && !Core.checkInventory("Acquiescence", Quantity)) {
            Core.huntMonster("worldscore", "Elemental Attempt", "Cracked Elemental Stone", 14, { log: false });
            Core.huntMonster("worldscore", "Crystalized Mana", "Crystalized Tooth", 14, { log: false });
            Core.huntMonster("worldscore", "Mask of Tranquility", "Creator's Favor", 1, { log: false });
        }
        bot.wait.forPickup("Acquiescence");
        Core.cancelRegisteredQuests();
    }

    elementalCore(Quantity = 500): void {
        if (Core.checkInventory("Elemental Core", Quantity))
            return;

        SoW.manaCradle();

        if (!Core.isCompletedBefore(9126)) {
            Core.logger(
                "Once Upon Another Time 9126, missing quest progress (Group Boss - skua no can do)");
            return;
        }

        Core.farmingLogger("Elemental Core", Quantity);
        Core.addDrop("Elemental Core");

        Adv.gearStore(false, { enhAfter: true });

        Core.useBossClass();
        Core.registerQuests(9126);
        while (!bot.shouldExit && !Core.checkInventory("Elemental Core", Quantity)) {
            Core.huntMonster("manacradle", "Dark Tainted Mana", "Elemental Tear", 20, { log: false });
            Core.huntMonster("manacradle", "Malgor", "Weathered Armor Shard", 1, { log: false });
            Core.huntMonster("manacradle", "The Mainyu", "Licorice Scale", 1, { log: false });
        }
        bot.wait.forPickup("Elemental Core");
        Core.cancelRegisteredQuests();
        Adv.gearStore(true, { enhAfter: true });
    }

    garishRemnant(Quantity = 1000): void {
        if (Core.checkInventory("Garish Remnant", Quantity))
            return;

        SoW.timekeep();

        Core.farmingLogger("Garish Remnant", Quantity);
        Core.addDrop("Garish Remnant");

        Core.registerQuests(8813);
        while (!bot.shouldExit && !Core.checkInventory("Garish Remnant", Quantity)) {
            Core.equipClass(ClassType.Solo);
            Core.huntMonster("Timekeep", "Mal-formed Gar", "Gar's Resignation Letter", 1, { log: false });
            Core.equipClass(ClassType.Farm);
            Core.huntMonster("Timekeep", "Mumbler", "Mumbler Drool", 8, { log: false });
            Core.huntMonster("Timekeep", "Decaying Locust", "Locust Wings", 8, { log: false });
        }
        bot.wait.forPickup("Garish Remnant");
        Core.cancelRegisteredQuests();
    }

    prismaticSeams(Quantity = 2000): void {
        if (Core.checkInventory("Prismatic Seams", Quantity)) {
            Core.logger(`You already have ${Quantity} Prismatic Seams, skipping farming.`);
            return;
        }

        SoW.timestreamWar();

        Core.farmingLogger("Prismatic Seams", Quantity);
        Core.addDrop("Prismatic Seams");
        Core.equipClass(ClassType.Farm);

        Core.registerQuests(8814, 8815);
        Core.killMonster("Streamwar", "r3a", "Left", "*", "Prismatic Seams", Quantity, { isTemp: false, log: false });
        bot.wait.forPickup("Prismatic Seams");
        Core.cancelRegisteredQuests();
    }

    unboundThread(Quantity = 1000): void {
        if (Core.checkInventory("Unbound Thread", Quantity))
            return;

        SoW.deadLines();

        Core.farmingLogger("Unbound Thread", Quantity);
        Core.addDrop("Unbound Thread");
        Core.equipClass(ClassType.Solo);

        Core.registerQuests(8869);
        while (!bot.shouldExit && !Core.checkInventory("Unbound Thread", Quantity)) {
            Core.equipClass(ClassType.Farm);
            Core.huntMonster("DeadLines", "Frenzied Mana", "Captured Mana", 8, { log: false });
            Core.huntMonster("DeadLines", "Shadowfall Warrior", "Armor Scrap", 8, { log: false });
            Core.equipClass(ClassType.Solo);
            Core.huntMonster("DeadLines", "Eternal Dragon", "Eternal Dragon Scale", 1, { log: false });
        }
        bot.wait.forPickup("Unbound Thread");
        Core.cancelRegisteredQuests();
    }

    willpower(Quantity = 1000): void {
        if (Core.checkInventory("Willpower", Quantity))
            return;

        SoW.ruinedCrown();

        Core.farmingLogger("Willpower", Quantity);
        Core.addDrop("Willpower");

        Core.registerQuests(8788);
        while (!bot.shouldExit && !Core.checkInventory("Willpower", Quantity)) {
            Core.equipClass(ClassType.Solo);
            Core.huntMonster("ruinedcrown", "Calamitous Warlic", "Warlic's Favor", 1, { log: false });
            Core.equipClass(ClassType.Farm);
            Core.huntMonster("ruinedcrown", "Frenzied Mana", "Mana Residue", 8, { log: false });
            Core.huntMonster("ruinedcrown", "Mana-Burdened Mage", "Mage's Blood Sample", 8, { log: false });
        }
        bot.wait.forPickup("Willpower");
        Core.cancelRegisteredQuests();
    }
}
