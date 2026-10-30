/*
name: DragonslayerGeneral
description: null
tags: null
*/
// Port of Scripts/Other/Classes/DragonslayerGeneral.cs.

import { CoreAdvanced } from "../../core/CoreAdvanced";
import { ClassType, Core } from "../../core/CoreBots";
import { CoreFarms } from "../../core/CoreFarms";
import { CoreStory } from "../../core/CoreStory";

const Farm = new CoreFarms();
const Adv = new CoreAdvanced();
const Story = new CoreStory();

export class DragonslayerGeneral {
    getDSGeneral(rankUpClass = true): void {
        Adv.gearStore(false, { enhAfter: true });
        if (Core.checkInventory(35996)) {
            if (rankUpClass)
                Adv.rankUpClass("Dragonslayer General");
            return;
        }

        Farm.gold(30000);
        this.enchantedScaleandClaw(75, 100);
        Core.buyItem("dragontown", 1286, 35996, 1, 3937);

        if (rankUpClass)
            Adv.rankUpClass("Dragonslayer General");
    }

    enchantedScaleandClaw(scaleQuant: number, clawQuant: number): void {
        // C# local functions
        const lairStory = (): void => {

            // Dragonslayer Veteran 165
            Story.killQuest(165, "lair", "Wyvern");

            // Dragonslayer Sergeant 166
            Story.killQuest(166, "lair", "Bronze Draconian");

            // Dragonslayer Captain 167
            Story.killQuest(167, "lair", "Dark Draconian");

            // Dragonslayer Marshal 168
            Story.killQuest(168, "lair", "Red Dragon");
        };

        lairStory();
        Adv.gearStore(false, { enhAfter: true });
        if (!Core.checkInventory(582) && Core.checkClassRank({ currentClass:false, className: "Dragonslayer" }) < 10) {
            Core.buyItem("lair", 38, "Dragonslayer");
            Adv.rankUpClass("Dragonslayer");
        }
        Adv.gearStore(true, { enhAfter: true });

        if (scaleQuant > 0 || clawQuant > 0) {
            Core.addDrop("Enchanted Scale", "Dragon Claw");
            Core.logger(`Farming ${scaleQuant} Scale / ${clawQuant} Claw`);
            if (scaleQuant > 0)
                Core.registerQuests(5294);

            Core.equipClass(ClassType.Farm);
            Core.killMonster("dragontown", "r4", "Right", "Tempest Dracolich", "Enchanted Scale", scaleQuant, { isTemp: false });
            Core.killMonster("dragontown", "r4", "Right", "Tempest Dracolich", "Dragon Claw", clawQuant, { isTemp: false });
            Core.cancelRegisteredQuests();
        }
    }
}

export default function main(): void {
        Core.setOptions();

        new DragonslayerGeneral().getDSGeneral();

        Core.setOptions(false);
}
