/*
name: Shadow Dragon Defender
description: if "Manticore Cub Pet" is owned, it will buy "Shadow Dragon Defender"
tags: shadow dragon defender, manticore cub pet, armor
*/
// Port of Scripts/Other/ShadowDragonDefender.cs.

import { Core } from "../core/CoreBots";
import { HatchTheEgg } from "../story/7deadlydragons/extra/HatchTheEgg";

const Egg = new HatchTheEgg();

export class GetSDD {
    shadowDragonDefender(): void {
        if (Core.checkInventory("Shadow Dragon Defender"))
            return;

        Egg.hatch();

        Core.buyItem("mysteriousegg", 1728, "Shadow Dragon Defender");
        Core.toBank("Manticore Cub Pet");
    }
}

export default function main(): void {
        Core.setOptions();

        new GetSDD().shadowDragonDefender();

        Core.setOptions(false);
}
