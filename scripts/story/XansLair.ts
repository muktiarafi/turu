/*
name: Xan's Lair Story
description: This will finish the Xan's Lair Story.
tags: story, quest, xan, lair
*/
// Port of Scripts/Story/XansLair.cs.

import { bot } from "turu";
import { Core } from "../core/CoreBots";
import { CoreStory } from "../core/CoreStory";

const Story = new CoreStory();

export class XansLair {
    doAll(): void {
        this.volcano();
        this.andesisQuests();
        this.scoriasQuestsQuests();
        this.warlicsQuests();
    }

    volcano(): void {
        if (Core.isCompletedBefore(1261))
            return;

        Story.preLoad(this.volcano);

        // Level 1 1242
        Story.killQuest(1242, "volcano", "Lava Golem");

        // Level 2 1243
        if (!Story.questProgression(1243)) {
            Core.ensureAccept(1243);
            Core.getMapItem(526, 3, "volcano");
            Core.huntMonster("volcano", "Fire Imp", "Broiled Clue", 7);
            Core.ensureComplete(1243);
        }

        // Level 3 1244
        Story.killQuest(1244, "volcano", "Fire Imp");

        // Level 4 1245
        if (!Story.questProgression(1245)) {
            Core.ensureAccept(1245);
            Core.getMapItem(527, 8, "volcano");
            Core.huntMonster("volcano", "Lava Golem", "Blazing Clue", 10);
            Core.ensureComplete(1245);
        }

        // Level 5 1246
        Story.killQuest(1246, "volcano", "Fire Imp");

        // Level 6 1247
        Story.killQuest(1247, "volcano", "Fire Imp");

        // Level 7 1248
        Story.mapItemQuest(1248, "volcano", 528, 12);

        // Level 8 1249
        Story.killQuest(1249, "volcano", "Flamethrower Dwakel");

        // Level 9 1250
        if (!Story.questProgression(1250)) {
            Core.ensureAccept(1250);
            Core.getMapItem(529, 8, "volcano");
            Core.huntMonster("volcano", "Fire Imp", "Torched Clue", 10);
            Core.ensureComplete(1250);
        }

        // Level 10 1251
        Story.killQuest(1251, "volcano", "Flamethrower Dwakel");

        // Level 11 1252
        if (!Story.questProgression(1252)) {
            Core.ensureAccept(1252);
            Core.getMapItem(530, 10, "volcano");
            Core.huntMonster("volcano", "Fire Imp", "Melted Clue", 10);
            Core.ensureComplete(1252);
        }

        // Level 12 1253
        Story.killQuest(1253, "volcano", "Flame Elemental");

        // Level 13 1254
        Story.mapItemQuest(1254, "volcano", 531, 12);

        // Level 14 1255
        Story.killQuest(1255, "volcano", "Fire Imp");

        // Level 15 1256
        Story.killQuest(1256, "volcano", "Fire Imp");

        // Level 16 1257
        Story.mapItemQuest(1257, "volcano", 532, 10);

        // Level 17 1258
        Story.killQuest(1258, "volcano", "Flamethrower Dwakel");

        // Level 18 1259
        if (!Story.questProgression(1259)) {
            Core.ensureAccept(1259);
            Core.huntMonster("volcano", "Fire Imp", "Singed Clue", 15);
            Core.ensureComplete(1259);
        }

        // Level 19 1260
        if (!Story.questProgression(1260)) {
            Core.ensureAccept(1260);
            Core.getMapItem(533, 6, "volcano");
            Core.huntMonster("volcano", "Fire Imp", "Smoking Clue", 13);
            Core.ensureComplete(1260);
        }

        // Level 20 1261
        Story.killQuest(1261, "volcano", "Magman");
    }

    andesisQuests(): void {
        if (Core.isCompletedBefore(1739))
            return;

        Story.preLoad(this.andesisQuests);

        // A Town Divided 1733
        Story.mapItemQuest(1733, "xantown", 922);

        // The Miller's Key 1734
        Story.killQuest(1734, "xantown", "Fire Imp");

        // Trailblazer 1735
        Story.killQuest(1735, "xantown", "Fire Imp");

        // Fire Brigade Of One 1736
        Story.killQuest(1736, "xantown", "Fire Imp");

        // Fire Control 1737
        Story.mapItemQuest(1737, "xantown", 923, 4);

        // Andesi's Family Pendant 1738
        Story.mapItemQuest(1738, "xantown", 924);

        // Signed, Seared, Delivered 1739
        Story.mapItemQuest(1739, "xantown", 925);
        if (bot.map.name === "xantown")
            Core.jump("r12", "Left");
    }

    scoriasQuestsQuests(): void {
        if (Core.isCompletedBefore(1740))
            return;

        Story.preLoad(this.scoriasQuestsQuests);

        // The Xan With The Plan 1740
        Story.killQuest(1740, "xantown", "Xan");
        if (bot.map.name === "xantown")
            Core.jump("r12", "Left");
    }

    warlicsQuests(): void {
        if (Core.isCompletedBefore(2157))
            return;

        Story.preLoad(this.warlicsQuests);

        // Locate The Sealed Library 2151
        Story.mapItemQuest(2151, "xancave", 1220);

        // A Powered Library Lock 2152
        Story.killQuest(2152, "xancave", "Lava Goblin");

        // Luminate the Library Lock 2153
        Story.mapItemQuest(2153, "xancave", 1221);

        // Defend the Library! 2154
        Story.killQuest(2154, "xancave", "Lava Goblin");

        // Crossing Over 2155
        if (!Story.questProgression(2155)) {
            Core.ensureAccept(2155);
            Core.getMapItem(1223, 8, "xancave");
            Core.huntMonster("xancave", "Lava Goblin", "Fire Salts", 10);
            Core.huntMonster("xancave", "Lava Goblin", "Fire Wire", 10);
            Core.ensureComplete(2155);
        }

        // Guardian Of Shurpu 2156
        Story.killQuest(2156, "xancave", "Shurpu Ring Guardian");

        // Face Xan 2157
        Story.mapItemQuest(2157, "xancave", 1222);
    }
}

export default function main(): void {
        Core.setOptions();

        new XansLair().doAll();

        Core.setOptions(false);
}
