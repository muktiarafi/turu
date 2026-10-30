/*
name: Dragon Fable Origins Story
description: This will finish the Dragon Fable Origins Story.
tags: story, quest, dragon fable origins, firewar, northmountain, charredplains, drakonnan,
*/
// Port of Scripts/Story/DragonFableOrigins.cs.

import { bot } from "turu";
import { ClassType, Core } from "../core/CoreBots";
import { CoreStory } from "../core/CoreStory";

const Story = new CoreStory();

export class DragonFableOrigins {
    dragonFableOriginsAll(): void {
        this.greatFireWar();
        this.northMountain();
        this.charredPlains();
        this.drakonnan();
    }

    greatFireWar(): void {
        if (Core.isCompletedBefore(6299))
            return;

        Story.preLoad(this.greatFireWar);

        // Fire Dragon Scales
        Story.killQuest(6294, "firewar", "Fire Dragon");

        // Fire Dragon Hearts
        Story.killQuest(6295, "firewar", "Fire Dragon");

        // Pyritium Shards
        if (!Core.isCompletedBefore(6296)) {
            Core.ensureAccept(6296);
            Core.killMonster("firewar", "r8", "Right", "Inferno Dragon", "Pyritium Shards", 5);
            Core.ensureComplete(6296);
        }

        // Perfect Pyritium
        if (!Core.isCompletedBefore(6297)) {
            Core.ensureAccept(6296);
            Core.killMonster("firewar", "r8", "Right", "Inferno Dragon", "Perfect Pyritium");
            Core.ensureComplete(6297);
        }
        Core.equipClass(ClassType.Solo);

        // Uriax, Inferno of Akriloth
        Story.killQuest(6298, "firewar", "Uriax");

        // Confront Akriloth
        if (!Story.questProgression(6299)) {
            Core.ensureAccept(6299);
            Core.equipClass(ClassType.Solo);
            Core.join("firewar", "r10", "Left");
            if (bot.player.cell !== "r10")
                Core.jump("r10", "Left");

            while (!bot.shouldExit && !bot.tempInv.contains(43572)) {
                bot.combat.attack("*");
                bot.sleep(500);
            }
            Core.ensureComplete(6299);
            Core.jumpWait();
        }
    }

    northMountain(): void {
        if (Core.isCompletedBefore(6307))
            return;

        Story.preLoad(this.northMountain);

        // Getting a Feel for the Area
        if (!Story.questProgression(6301)) {
            Core.ensureAccept(6301);
            Core.huntMonster("northmountain", "Ice Elemental", "Monsters Defeated", 8);
            bot.map.getMapItem(5812);
            Core.ensureComplete(6301);
        }

        // Finding the First Rune
        Story.killQuest(6302, "northmountain", "Ice Elemental");

        // A Slippery Slope
        Story.killQuest(6303, "northmountain", "Ursice Savage");

        // A 100% Chance of Hail
        Story.killQuest(6304, "northmountain", "Ice Spitter");

        // The Final Rune
        Story.killQuest(6305, "northmountain", "Ice Elemental");

        // Unlock the Cave
        Story.mapItemQuest(6306, "northmountain", 5813, 4);
        Story.mapItemQuest(6306, "northmountain", 5814);

        // The Guardian
        Story.killQuest(6307, "northmountain", "Izotz");
    }

    charredPlains(): void {
        if (Core.isCompletedBefore(6311))
            return;

        Story.preLoad(this.charredPlains);

        // The Fires of War
        Story.killQuest(6308, "charredplains", "Fire Dragon");

        // Meet Up
        Story.mapItemQuest(6309, "charredplains", 5815);

        // Akriloth
        Story.killQuest(6310, "charredplains", "Akriloth");

        // Enchanted Ice Shards
        Story.killQuest(6311, "northmountain", "Izotz");
    }

    drakonnan(): void {
        if (Core.isCompletedBefore(6325))
            return;

        Story.preLoad(this.drakonnan);

        // Gathering the Pieces
        Story.mapItemQuest(6312, "drakonnan", 5827, 5);

        // This Weapon Scales
        Story.killQuest(6313, "drakonnan", "Fire Dragon");

        // Reinforced Lava Steel
        Story.killQuest(6314, "drakonnan", "Living Lava");

        // Claymore Shards
        Story.killQuest(6315, "drakonnan", "Fire Dragon");

        // Keeping Cool
        Story.killQuest(6316, "northmountain", "Ice Elemental");

        // Cryogenic Decacrystals
        Story.mapItemQuest(6317, "northmountain", 5826, 6);

        // Fiamme's Blessing
        Story.killQuest(6318, "drakonnan", "Fire Elemental");

        // Inferno Heart
        Story.killQuest(6319, "drakonnan", "Living Fire");

        // Dragons!
        Story.killQuest(6320, "drakonnan", "Fire Dragon");

        // No Lava For My Enemies
        Story.killQuest(6321, "drakonnan", "Living Lava");

        // Clearing Out the Fodder
        Story.killQuest(6322, "drakonnan", "Fire Elemental");

        // Unlocking the Door
        Story.killQuest(6323, "drakonnan", "Fire Elemental");

        // Drakonnan
        Story.killQuest(6324, "drakonnan", "Drakonnan");

        // Ultra Drakonnan
        Story.killQuest(6325, "drakonnan", "Ultra Drakonnan");
    }
}

export default function main(): void {
        Core.setOptions();

        new DragonFableOrigins().dragonFableOriginsAll();

        Core.setOptions(false);
}
