/*
name: Warfury Emblem
description: Farms "Warfury Emblem" from quest: "Warfury Training".
tags: war, fury, training, emblem
*/
// Port of Scripts/Other/WarFuryEmblem.cs.

import { ClassType, Core } from "../core/CoreBots";
import { CoreSoW } from "../story/shadowsofwar/CoreSoW";

const SoW = new CoreSoW();

export class WarfuryEmblem {
    warfuryEmblemFarm(quant = 60): void {
        if (Core.checkInventory("Warfury Emblem", quant))
            return;

        SoW.tyndarius();

        Core.addDrop("Warfury Emblem");
        Core.farmingLogger("Warfury Emblems", quant);
        Core.equipClass(ClassType.Farm);

        Core.registerQuests(8204);
        Core.huntMonster("wartraining", "Warfury Soldier", "Warfury Emblem", quant, { isTemp: false });
        Core.cancelRegisteredQuests();
    }
}

export default function main(): void {
        Core.setOptions();

        new WarfuryEmblem().warfuryEmblemFarm();

        Core.setOptions(false);
}
