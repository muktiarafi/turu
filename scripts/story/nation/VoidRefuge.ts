/*
name: Void Refuge Story
description: This script will complete the storyline in /voidrefuge
tags: voidrefuge, refuge, nation, nulgath, story, ana di carcano, gravelyn
*/
// Port of Scripts/Story/Nation/VoidRefuge.cs.

import { Core } from "../../core/CoreBots";
import { CoreStory } from "../../core/CoreStory";

const Story = new CoreStory();

export class VoidRefuge {
    storyline(): void {
        if (Core.isCompletedBefore(9531))
            return;

        Story.preLoad(this.storyline);

        // Newbie Boom (9522)
        Story.killQuest(9522, "voidrefuge", "Lightguard Paladin");

        // Tears Like Honey (9523)
        Story.killQuest(9523, "voidrefuge", "Paladin Ascendant");

        // Work Fiends (9524)
        if (!Story.questProgression(9524)) {
            Core.ensureAccept(9524);
            Core.getMapItem(12572, 1, "voidrefuge");
            Core.huntMonster("voidrefuge", "Nation Caster", "Void Engraving", 5);
            Core.ensureComplete(9524);
        }

        // Rewritten Signatures (9525)
        Story.killQuest(9525, "voidrefuge", "Nation Outrider");

        // Where the Heart is (9526)
        if (!Story.questProgression(9526)) {
            Core.ensureAccept(9526);
            Core.getMapItem(12573, 1, "voidrefuge");
            Core.huntMonster("voidrefuge", "Lightguard Paladin", "Paladin's Iudex Oath", 6);
            Core.huntMonster("voidrefuge", "Paladin Ascendant", "Ascendant's Iudex Oath", 3);
            Core.ensureComplete(9526);
        }

        // Rip Space (9527)
        if (!Story.questProgression(9527)) {
            Core.ensureAccept(9527);
            Core.getMapItem(12574, 5, "voidrefuge");
            Core.huntMonster("voidrefuge", "Nation Caster", "Void Tattoo", 3);
            Core.ensureComplete(9527);
        }

        // Permanent Record (9528)
        Story.killQuest(9528, "voidrefuge", "Nation Outrider");

        // Blood and Water (9529)
        Story.mapItemQuest(9529, "voidrefuge", [ 12575, 12576, 12577 ]);

        // Family Feud (9530)
        if (!Story.questProgression(9530)) {
            Core.ensureAccept(9530);
            Core.huntMonster("voidrefuge", "Paladin Ascendant", "Shimmering Blood", 4);
            Core.huntMonster("voidrefuge", "Nation Outrider", "Caustic Blood", 8);
            Core.ensureComplete(9530);
        }

        // Twisted Helix (9531)
        Story.killQuest(9531, "voidrefuge", "Carnage");
    }
}

export default function main(): void {
        Core.setOptions();

        new VoidRefuge().storyline();

        Core.setOptions(false);
}
