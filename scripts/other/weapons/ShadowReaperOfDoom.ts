/*
name: ShadowReaper of Doom
description: This bot farms the requiered materials to farm the ShadowReaper of Doom
tags: blod, mirror, realm, token, undead, paladin
*/
// Port of Scripts/Other/Weapons/ShadowReaperOfDoom.cs.

import { ClassType, Core } from "../../core/CoreBots";
import { Core13LoC } from "../../story/lordsofchaos/Core13LoC";

const LoC = new Core13LoC();

export class SRoD {
    shadowReaperOfDoom(): void {
        if (Core.checkInventory("ShadowReaper Of Doom"))
            return;

        Core.farmingLogger("ShadowReaper Of Doom", 1);
        LoC.xiang();

        Core.equipClass(ClassType.Solo);
        Core.huntMonsterMapID("mirrorportal", 1, "Mirror Realm Token", 300, { isTemp: false });
        Core.cancelRegisteredQuests();
        Core.killMonster("overworld", "boss1", "Left", "Undead Artix", "Undead Paladin Token", 1, { isTemp: false });

        Core.buyItem("overworld", 618, "ShadowReaper Of Doom", 1, 1220);
    }
}

export default function main(): void {
        Core.setOptions();

        new SRoD().shadowReaperOfDoom();

        Core.setOptions(false);
}
