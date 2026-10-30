/*
name: null
description: null
tags: null
*/
// Port of Scripts/Evil/NSoD/CoreNSOD.cs.
// Not ported (FarmerJoe's DoAll doesn't reach them): GetNSOD, GetNBOD, NSBlade, NSHilt, NSAura, EnergizedBlade, EnergizedHilt, EnergizedAura, FindBlade, FindHilt, FindAura, BariumOfDoom, Barium, TimeLordNecro, CavernCelestite, PrimarchHilt, BladeEssence, CHourglass, ScrollDarkArts, PreFarmSteps.

import { bot, option } from "turu";
import { ClassType, Core, configOr } from "../../core/CoreBots";
import { CoreFarms } from "../../core/CoreFarms";
import { CoreSDKA } from "../sdka/CoreSDKA";

const Farm = new CoreFarms();
const SDKA = new CoreSDKA();

export class CoreNSOD {
    maxStack = option.bool("MaxStack", "Max Stack", "Max Stack Monster Essences in \"Retreive Void Auras\"\nRecommended setting: True", true);

    preFarm = option.bool("PreFarm", "Pre Farm Materials", "Farm all requiered items before merging everything. Not recommended if you already did a merge yourself.\nRecommended setting: False", false);

    getSDKA = option.bool("getSDKA", "Get SDKA first [Mem]", "If true, the bot will attempt to get SDKA first, so that it can use the fastest Void Aura farm available\nMember-Only\nRecommended setting: True", true);

    essences: string[] = [
        "Astral Ephemerite Essence",
        "Belrot the Fiend Essence",
        "Black Knight Essence",
        "Tiger Leech Essence",
        "Carnax Essence",
        "Chaos Vordred Essence",
        "Dai Tengu Essence",
        "Unending Avatar Essence",
        "Void Dragon Essence",
        "Creature Creation Essence",
    ];

    voidAuras(quant = 7500): void {
        if (Core.checkInventory("Void Aura", quant))
            return;

        if (
            configOr("getSDKA", false)
            && bot.player.isMember
            && !Core.checkInventory(
                14474)
        )
            SDKA.doAll();

        Core.addDrop("Void Aura");
        Core.farmingLogger("Void Aura", quant);

        if (
            Core.checkInventory(
                14474)
        )
            this.commandingShadowEssences(quant);

        if (bot.player.isMember)
            this.gatheringUnstableEssences(quant);
        else
            this.retrieveVoidAuras(quant);
    }

    commandingShadowEssences(quant = 7500): void {
        if (Core.checkInventory("Void Aura", quant) || !Core.checkInventory(14474))
            return;

        bot.options.aggroAllMonsters = false;
        bot.options.aggroMonsters = false;
        while (!bot.shouldExit && !Core.checkInventory("Void Aura", quant)) {
            Core.ensureAccept(4439);
            Core.equipClass(ClassType.Solo);
            Core.huntMonsterMapID("shadowrealmpast", 11, "Malignant Essence", 3, { isTemp: false });

            Core.equipClass(ClassType.Farm);
            Core.killMonster("shadowrealmpast", "Enter", "Spawn", "*", "Empowered Essence", 50, { isTemp: false });
            Core.ensureComplete(4439);
            Core.farmingLogger("Void Aura", quant);
        }
        Core.abandonQuest(4439);
    }

    gatheringUnstableEssences(quant = 7500): void {
        if (Core.checkInventory("Void Aura", quant) || !Core.isMember)
            return;

        Farm.evilREP();
        Core.equipClass(ClassType.Farm);

        Core.registerQuests(4438);
        while (!Core.checkInventory("Void Aura", quant)) {
            Core.killMonster("reddeath", "r2", "Left", "*", "Mirror Essence", 175, { isTemp: false });
            Core.killMonster("neverworldb", "r2", "Left", "*", "Twisted Essence", 25, { isTemp: false });
            Core.huntMonster("doomwar", "Zombie King Alteon", "Transposed Essence", 1, { isTemp: false });

            bot.wait.forPickup("Void Aura");
            Core.farmingLogger("Void Aura", quant);
        }
        Core.abandonQuest(4438);
    }

    retrieveVoidAuras(quant = 7500): void {
        if (Core.checkInventory("Void Aura", quant))
            return;

        let Essencequant = 20;
        if (bot.config != null) {
            Essencequant = configOr("MaxStack", false) ? 100 : 20;
        } else {
            Core.logger("Bot.Config is null, defaulting Essencequant to 20.");
        }
        Farm.evilREP();
        Core.addDrop(...this.essences);
        if (!Core.checkInventory("Necromancer", 1, { toInv: false }))
            bot.drops.add("Creature Shard");

        while (!bot.shouldExit && !Core.checkInventory("Void Aura", quant)) {
            Core.farmingLogger("Void Aura", quant);
            Core.ensureAccept(4432);

            Core.equipClass(ClassType.Farm);
            Core.huntMonster("timespace", "Astral Ephemerite", "Astral Ephemerite Essence", Essencequant, { isTemp: false });

            this.huntMonsterBatch(
                Essencequant, 
                false, 
                false, 
                true, 
                ["necrocavern", 5, "Chaos Vordred Essence"], 
                ["citadel", 21, "Belrot the Fiend Essence"], 
                ["greenguardwest", 22, "Black Knight Essence"], 
                ["mudluk", 18, "Tiger Leech Essence"], 
                ["aqlesson", 17, "Carnax Essence"], 
                ["hachiko", 10, "Dai Tengu Essence"], 
                ["timevoid", 12, "Unending Avatar Essence"], 
                ["dragonchallenge", 4, "Void Dragon Essence"], 
                ["maul", 17, "Creature Creation Essence"]);

            Core.ensureCompleteMulti(4432);
            bot.wait.forPickup("Void Aura");
            Core.farmingLogger("Void Aura", quant);
        }
    }

    private huntMonsterBatch(quant: number, isTemp: boolean, publicRoom: boolean, log: boolean, ...monsters: [map: string, monster: number, essence: string][]): void {
        Core.addDrop(...monsters.map(x => x[2]));
        Core.equipClass(ClassType.Solo);
        for (const monster of monsters.filter(x =>
                x[2] != null && x[1] > 0 && !Core.checkInventory(x[2], quant))
        )
            Core.huntMonsterMapID(monster[0], monster[1], monster[2], quant, { isTemp, log, publicRoom });
    }

    bonesVoidRealm(quant: number): void {
        if (Core.checkInventory("Bones from the Void Realm", quant))
            return;
        Core.farmingLogger("Bones from the Void Realm", quant);
        Core.addDrop("Undead Energy");
        Farm.battleUnderB("Bone Dust", quant * 50);
        this.voidAuras(quant * 50);
        Core.buyItem("shadowfall", 793, "Bones from the Void Realm", quant);
        bot.wait.forPickup("Bones from the Void Realm");
    }
}
