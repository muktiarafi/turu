/*
name: Void Chasm Story
description: This script will complete the storyline in /voidchasm.
tags: voidchasm, chasm, nation, nulgath, story, ana di carcano, gravelyn, jadzia
*/
// Port of Scripts/Story/Nation/VoidChasm.cs.

import { ClassType, Core } from "../../core/CoreBots";
import { CoreStory } from "../../core/CoreStory";
import { VoidRefuge } from "./VoidRefuge";

const Story = new CoreStory();
const VR = new VoidRefuge();

export class VoidChasm {
    storyline(): void {
        if (Core.isCompletedBefore(9552))
            return;

        VR.storyline();

        Story.preLoad(this.storyline);

        // Light Headed 9543
        Story.killQuest(9543, "voidchasm", "Paladin Ascendant");

        // Forgiveness of Debt 9544
        Story.killQuest(9544, "voidchasm", "Nation Outrider");

        // Hyperspace-Plasia 9545
        if (!Story.questProgression(9545)) {
            Core.ensureAccept(9545);
            Core.getMapItem(12619, 6, "voidchasm");
            Core.huntMonster("voidchasm", "Paladin Ascendant", "Alberic Seal", 3);
            Core.ensureComplete(9545);
        }

        // Tangled Aortas 9546
        Story.mapItemQuest(9546, "voidchasm", [ 12620, 12621 ]);

        // Nation Code 9547
        if (!Story.questProgression(9547)) {
            Core.ensureAccept(9547);
            Core.getMapItem(12622, 1, "voidchasm");
            Core.huntMonster("voidchasm", "Nation Outrider", "Outrider's Keepsake", 4);
            Core.ensureComplete(9547);
        }

        // Snake Ears 9548
        Story.killQuest(9548, "voidchasm", "Void Fang");

        // Faustian Leftovers 9549
        if (!Story.questProgression(9549)) {
            Core.ensureAccept(9549);
            Core.getMapItems([ [12623, 1], [12627, 1], [12628, 1] ], "voidchasm");
            Core.huntMonster("voidchasm", "The Hushed", "Phantom's Sigh", 5);
            Core.ensureComplete(9549);
        }

        // Broken Lives 9550
        if (!Story.questProgression(9550)) {
            Core.ensureAccept(9550);
            Core.getMapItem(12624, 4, "voidchasm");
            Core.huntMonster("voidchasm", "The Hushed", "Blank Contract", 3);
            Core.huntMonster("voidchasm", "Void Fang", "Fiendish Sinew", 6);
            Core.ensureComplete(9550);
        }

        // Call of the Fiend 9551
        Core.equipClass(ClassType.Solo);
        if (!Story.questProgression(9551)) {
            Core.ensureAccept(9551);
            Core.getMapItems([ [12625, 1], [12626, 1] ], "voidchasm");
            Core.huntMonster("voidchasm", "Carnage", "Voided Blood Sample", 1);
            Core.ensureComplete(9551);
        }

        // Famiglia di Carcano 9552
        Story.killQuest(9552, "voidchasm", "Carcano");
    }
}

export default function main(): void {
        Core.setOptions();

        new VoidChasm().storyline();

        Core.setOptions(false);
}
