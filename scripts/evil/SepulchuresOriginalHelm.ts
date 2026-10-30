/*
name: Sepulchure's Original Helm
description: This script will get Sepulchure's Original Helm.
tags: sepulchure, original, helm, evil, gravelyn, doomfire, royal, shadowscythe, blade
*/
// Port of Scripts/Evil/SepulchuresOriginalHelm.cs.

import { bot } from "turu";
import { Alignment, Core } from "../core/CoreBots";
import { CoreFarms } from "../core/CoreFarms";
import { CoreStory } from "../core/CoreStory";
import { CoreDoomwood } from "../story/doomwood/CoreDoomwood";
import { Core13LoC } from "../story/lordsofchaos/Core13LoC";
import { CoreToD } from "../story/throneofdarkness/CoreToD";

const Story = new CoreStory();
const Farm = new CoreFarms();
const LOC = new Core13LoC();
const TOD = new CoreToD();
const DW = new CoreDoomwood();

export class SepulchuresOriginalHelm {
    gravelynsDoomFireTokenItems: string[] = [
        "Empowered Essence",
        "Gravelyn's Blessing",
        "Painful Memory Bubble",
        "Burning Passion Flame",
        "Father's Sorrowful Tear",
        "Gravelyn's DoomFire Token",
        "Necrotic Sword of Doom",
        "Sepulchure's DoomKnight Armor",
    ];

    doAll(): void {
        if (Core.checkInventory("Sepulchure's Original Helm"))
            return;

        Story.preLoad(this.doAll);

        Core.addDrop("Sepulchure's Original Helm");
        if (Core.checkInventory("Sepulchure's DoomKnight Armor")) {
            Core.buyItem("shadowfall", 1642, "Sepulchure's Original Helm");
            return;
        }

        Core.changeAlignment(Alignment.Evil);
        Farm.evilREP(10);
        Farm.experience(70);
        DW.aqwZombies();
        TOD.mysteriousDungeon();
        LOC.hero();
        if (
            !Core.checkInventory(
                [
                    "Lore's Champion Seal",
                    "Gravelyn's DoomFire Token",
                    "Royal ShadowScythe Blade",
                ])
        ) {
            this.gravelynsDoomFireToken();
            this.royalShadowScytheBlade();
            Core.buyItem(bot.map.name, 993, "Lore's Champion Seal");
        }
        Core.chainComplete(6555);
        bot.wait.forPickup("Sepulchure's Original Helm");
        Core.sellItem("Royal ShadowScythe Blade");
    }

    gravelynsDoomFireToken(): void {
        if (Core.checkInventory(37033))
            return;

        Core.addDrop(...this.gravelynsDoomFireTokenItems);

        while (!bot.shouldExit && !Core.checkInventory(37033)) {
            Core.ensureAccept(5461);
            // Check for "Gravelyn's Blessing"
            if (!Core.checkInventory(37034)) {
                if (Core.checkInventory("Necrotic Sword of Doom"))
                    Core.chainComplete(5455);
                else if (Core.checkInventory("Sepulchure's DoomKnight Armor"))
                    Core.chainComplete(5456);
                else {
                    // Handle the case when none of the above conditions match
                    Farm.evilREP(10);
                    Core.ensureAccept(5457);
                    while (!bot.shouldExit && !Core.checkInventory(37039))
                        Core.huntMonsterMapID(
                            "necrodungeon", 
                            9, 
                            "Essence of the Doomlord", 1, { log: false });
                    Core.ensureComplete(5457);
                    bot.wait.forPickup(37034);
                }
            }

            // Check for "Painful Memory Bubble"
            if (!Core.checkInventory("Painful Memory Bubble")) {
                Core.ensureAccept(5458);
                Core.killMonster("swordhavenfalls", "r10", "Left", 1, "Doomed Memories");
                Core.ensureComplete(5458);
                bot.wait.forPickup("Painful Memory Bubble");
            }

            // Check for "Burning Passion Flame"
            if (!Core.checkInventory("Burning Passion Flame")) {
                Core.ensureAccept(5459);
                Core.huntMonster("shadowstrike", "Sepulchuroth", "Sepulchuroth's Undying Flame");
                Core.ensureComplete(5459);
                bot.wait.forPickup("Burning Passion Flame");
            }

            // Check for "Father's Sorrowful Tear"
            if (!Core.checkInventory("Father's Sorrowful Tear")) {
                Core.ensureAccept(5460);
                Core.huntMonster("Shadowfall", "Shadow of the Past", "Father's Anger");
                Core.ensureComplete(5460);
                bot.wait.forPickup("Father's Sorrowful Tear");
            }

            // Kill monsters for "Empowered Essence"
            Core.killMonster("shadowrealmpast", "Enter", "Spawn", "*", "Empowered Essence", 13, { isTemp: false });

            Core.ensureComplete(5461);
            bot.wait.forPickup(37033);
        }
    }

    royalShadowScytheBlade(): void {
        if (Core.checkInventory("Royal ShadowScythe Blade"))
            return;

        Farm.gold(1000000);
        Farm.evilREP(10);
        Core.buyItem("shadowfall", 1639, "Royal ShadowScythe Blade", 1, 26666);
    }
}

export default function main(): void {
        Core.bankingBlackList.push(...
            [ "Necrotic Sword of Doom", "Sepulchure's DoomKnight Armor" ]);
        Core.setOptions();

        new SepulchuresOriginalHelm().doAll();

        Core.setOptions(false);
}
