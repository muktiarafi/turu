/*
name: Celestial Arena (Extra)
description: This will finish the Celestial Arena quest.
tags: story, quest, queen-of-monsters, extra, celestial-arena
*/
// Port of Scripts/Story/QueenofMonsters/Extra/CelestialArena.cs.

import { CoreAdvanced } from "../../../core/CoreAdvanced";
import { ClassType, Core } from "../../../core/CoreBots";
import { CoreStory } from "../../../core/CoreStory";

const Story = new CoreStory();
const Adv = new CoreAdvanced();

export class CelestialArenaQuests {
    doAll(): void {
        Adv.gearStore(false, { enhAfter: true });
        this.arena1to10();
        this.arena11to20();
        this.arena21to29();
        Adv.gearStore(true, { enhAfter: true });
    }

    arena1to10(): void {
        if (Core.isCompletedBefore(6022))
            return;

        Core.equipClass(ClassType.Solo);
        Story.killQuest(6013, "celestialarenab", "Slork Construct");
        Core.jumpWait();
        Story.killQuest(6014, "celestialarenab", "Azkorath Construct");
        Core.jumpWait();
        Story.killQuest(6015, "celestialarenab", "Blessed Inquisitor");
        Core.jumpWait();
        Story.killQuest(6016, "celestialarenab", "Lich Ravager Construct");
        Core.jumpWait();
        Story.killQuest(6017, "celestialarenab", "Ring Guardian Construct");
        Core.jumpWait();
        Story.killQuest(6018, "celestialarenab", "Serepthys Construct");
        Core.jumpWait();
        Story.killQuest(6019, "celestialarenab", "Yaomo Construct");
        Core.jumpWait();
        Story.killQuest(6020, "celestialarenab", "Cerberus Construct");
        Core.jumpWait();
        Story.killQuest(6021, "celestialarenab", "Infernal Warrior Construct");
        Core.jumpWait();
        Story.killQuest(6022, "celestialarenab", "Infernal Warlord Construct");
        Core.jumpWait();
    }

    arena11to20(): void {
        if (Core.isCompletedBefore(6032))
            return;

        Core.equipClass(ClassType.Solo);
        Story.killQuest(6023, "celestialarenac", "Conquest Construct");
        Core.jumpWait();
        Story.killQuest(6024, "celestialarenac", "War Construct");
        Core.jumpWait();
        Story.killQuest(6025, "celestialarenac", "Death Construct");
        Core.jumpWait();
        Story.killQuest(6026, "celestialarenac", "Famine Construct");
        Core.jumpWait();
        Story.killQuest(6027, "celestialarenac", "Diabolical Warlord Construct");
        Core.jumpWait();
        Story.killQuest(6028, "celestialarenac", "Undead Raxgore Construct");
        Core.jumpWait();
        Story.killQuest(6029, "celestialarenac", "Blessed Karok");
        Core.jumpWait();
        Story.killQuest(6030, "celestialarenac", "Kezeroth Construct");
        Core.jumpWait();
        Story.killQuest(6031, "celestialarenac", "Shadow Lord Construct");
        Core.jumpWait();
        Story.killQuest(6032, "celestialarenac", "Desolich Construct");
        Core.jumpWait();
    }

    arena21to29(): void {
        if (Core.isCompletedBefore(6042))
            return;

        Core.equipClass(ClassType.Solo);
        Story.killQuest(6033, "celestialarenad", "Queen of Hope");
        Core.jump("r2");
        Story.killQuest(6034, "celestialarenad", "Malxas Construct");
        Core.jump("r3");
        Story.killQuest(6035, "celestialarenad", "Blessed Gladius");
        Core.jump("r4");
        Story.killQuest(6036, "celestialarenad", "High Celestial Priest");
        Core.jump("r5");
        Story.killQuest(6037, "celestialarenad", "Blessed Enfield");
        Core.jump("r6");
        Story.killQuest(6038, "celestialarenad", "Avatar of Spirits");
        Core.jump("r7");
        Story.killQuest(6039, "celestialarenad", "Avatar of Time");
        Core.jump("r8");
        Story.killQuest(6040, "celestialarenad", "Avatar of Life");
        Core.jump("r9");
        Story.killQuest(6041, "celestialarenad", "Fallen Abezeth");
        Core.jump("r10");
        Story.killQuest(6042, "celestialarenad", "Aranx");
    }
}

export default function main(): void {
        Core.setOptions();

        new CelestialArenaQuests().doAll();

        Core.setOptions(false);
}
