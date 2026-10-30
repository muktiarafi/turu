/*
name: Arch DoomKnight (ADK)
description: This script farms the Arch DoomKnight Armor.
tags: adk, archdoomknight, doomknight, a means to an end, armor, boost, evil, shadowvault, shadow vault, gathering power, death's door, chaotic lords
*/
// Port of Scripts/Evil/ADK.cs.

import { bot } from "turu";
import { ClassType, Core } from "../core/CoreBots";
import { CoreFarms } from "../core/CoreFarms";
import { Core13LoC } from "../story/lordsofchaos/Core13LoC";

const Farm = new CoreFarms();
const LOC = new Core13LoC();

export class ArchDoomKnight {
    q1items: string[] = [
        "Arch DoomKnight Cape",
        "Undead Energy",
        "Human Souls",
        "Dragon Energy",
    ];

    q2items: string[] = [
        "Arch DoomKnight Cape Sword",
        "Arch DoomKnight Polearm",
        "Death's Power",
        "Souls of the Dead",
    ];

    q3items: string[] = [
        "Arch DoomKnight Sword",
        "Arch DoomKnight's Edge",
        "Escherion's Helm",
        "Legendary Sword of Dragon Control",
        "Hanzamune Dragon Koi Blade",
        "Wolfwing Armor",
        "One Eyed Doll Breaker",
        "Ledgermayne",
        "Tibicenas",
        "Soul of Chaos Armor",
        "Chaos Lionfang Armor",
        "Shorn Chaos King Crown",
        "Xiang Chaos",
        "Drakath's Sword",
        "Chaorrupted Hourglass",
        "Chaotic Power",
    ];

    q4items: string[] = [
        "Arch DoomKnight",
        "Arch DoomKnight Open Helm",
        "Arch DoomKnight Helm",
        "Ultimate Darkness Gem",
        "Undead Energy",
        "(Necro) Scroll of Dark Arts",
        "Doom Heart",
        "Dread Knight Cleaver",
        "Reaper's Soul",
        "Desolich's Undead Eye",
    ];

    combined = this.q1items.concat(this.q2items).concat(this.q3items).concat(this.q4items);

    doAll(ArmorOnly = false, BankRewards = false): void {
        this.aMeansToAnEnd(ArmorOnly);
        if (BankRewards)
            Core.toBank(...this.combined);
    }

    gatheringPower(): void {
        if (Core.checkInventory("Arch DoomKnight Cape", 1, { toInv: false }))
            return;

        LOC.wolfwing();
        Farm.evilREP(7);

        Core.addDrop(...this.q1items);
        Core.ensureAccept(6795);

        Core.equipClass(ClassType.Farm);
        Farm.battleUnderB("Undead Energy", 1800);
        Core.killMonster("noxustower", "r13", "Left", "*", "Human Souls", 500, { isTemp: false });
        Core.huntMonster("lair", "Water Draconian", "Dragon Energy", 600, { isTemp: false });
        Core.ensureComplete(6795);
        Core.toBank(...this.q1items);
    }

    deathsDoor(): void {
        if (!Core.isCompletedBefore(6796))
            this.gatheringPower();

        if (
            Core.checkInventory(
                [ "Arch DoomKnight Cape Sword", "Arch DoomKnight Polearm" ], 1, { toInv: false })
        )
            return;

        Core.addDrop(...this.q2items);
        Core.ensureAccept(6796);

        Core.equipClass(ClassType.Solo);
        Core.killMonster("shadowattack", "Boss", "Left", "Death", "Death's Power", 1, { isTemp: false });
        Core.killMonster("shadowattack", "Boss", "Left", "Death", "Souls of the Dead", 400, { isTemp: false });
        Core.ensureComplete(6796);
        Core.toBank(...this.q2items);
    }

    chaoticLords(): void {
        if (!Core.isCompletedBefore(6797))
            this.deathsDoor();

        if (
            Core.checkInventory(
                [ "Arch DoomKnight Sword", "Arch DoomKnight's Edge" ], 1, { toInv: false })
        )
            return;

        Core.addDrop(...this.q3items);

        Core.addDrop(25286);
        Core.ensureAccept(6797);

        Core.equipClass(ClassType.Solo);
        Core.killEscherion("Escherion's Helm");
        Core.killEscherion("Chaotic Power", 13);

        Core.killVath("Legendary Sword of Dragon Control", 1, { isTemp: false });
        Core.killKitsune("Hanzamune Dragon Koi Blade");
        Core.huntMonster("Wolfwing", "Wolfwing", "Wolfwing Armor", 1, { isTemp: false });
        Core.huntMonster("palooza", "Kimberly", "One Eyed Doll Breaker", 1, { isTemp: false });
        Core.huntMonster("Ledgermayne", "Ledgermayne", "Ledgermayne", 1, { isTemp: false });
        Core.huntMonster("djinn", "Tibicenas", "Tibicenas", 1, { isTemp: false });
        Core.huntMonster("dreamnexus", "Khasaanda", "Soul of Chaos Armor", 1, { isTemp: false });
        bot.quests.updateQuest(2814);
        Core.huntMonster("stormtemple", "Chaos Lord Lionfang", "Chaos Lionfang Armor", 1, { isTemp: false });
        Core.huntMonster("swordhavenfalls", "Chaos Lord Alteon", "Shorn Chaos King Crown", 1, { isTemp: false });
        Core.killXiang("Xiang Chaos");

        //Drakath's Sword (Free Player)
        while (!bot.shouldExit && !Core.checkInventory(25286))
            Core.huntMonster("ultradrakath", "Champion of Chaos");

        Core.huntMonster("mqlesson", "Dragonoid", "Dragonoid of Hours", 1, { isTemp: false });
        Core.huntMonster("timespace", "Chaos Lord Iadoa", "Chaorrupted Hourglass", 1, { isTemp: false });
        Core.ensureComplete(6797);
        Core.toBank(...this.q3items);
    }

    aMeansToAnEnd(ArmorOnly = false, HelmOnly = false): void {
        if (!Core.isCompletedBefore(6798))
            this.chaoticLords();

        if (
            HelmOnly && Core.checkInventory("Arch DoomKnight Helm")
            || (ArmorOnly && Core.checkInventory("Arch DoomKnight"))
        )
            return;
        else if (
            Core.checkInventory(
                [ "Arch DoomKnight", "Arch DoomKnight Open Helm", "Arch DoomKnight Helm" ], 1, { toInv: false })
        )
            return;

        Core.addDrop(...this.q4items);
        Core.ensureAccept(6798);

        Core.equipClass(ClassType.Farm);
        Core.huntMonster("shadowfallwar", "Skeletal Fire Mage", "Ultimate Darkness Gem", 50, { isTemp: false });
        Farm.battleUnderB("Undead Energy", 2000);

        Core.equipClass(ClassType.Solo);
        Core.huntMonster("epicvordred", "Ultra Vordred", "(Necro) Scroll of Dark Arts", 2, { isTemp: false }); Core.huntMonster("sepulchurebattle", "ULTRA Sepulchure", "Doom Heart", 1, { isTemp: false });
        Core.huntMonster("sepulchure", "Dark Sepulchure", "Dread Knight Cleaver", 1, { isTemp: false });
        Core.huntMonster("thevoid", "Reaper", "Reaper's Soul", 1, { isTemp: false });
        Core.huntMonster("Desolich", "Desolich", "Desolich's Undead Eye", 2, { isTemp: false });
        Core.ensureComplete(6798);
        Core.toBank(...this.q4items);
    }
}

export default function main(): void {
        Core.setOptions();

        new ArchDoomKnight().doAll();

        Core.setOptions(false);
}
