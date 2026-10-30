/*
name: null
description: null
tags: null
*/
// Port of Scripts/Good/GearOfAwe/CoreAwe.cs.

import { bot } from "turu";
import type { Monster } from "turu";
import { ClassType, Core } from "../../core/CoreBots";
import { CoreFarms } from "../../core/CoreFarms";

const Farm = new CoreFarms();

export class CoreAwe {
    getAweRelic(Item: string, LegendQuest: number, FragmentAmount: number, ShardAmount: number, Map: string, Monster: string): void {
        const relicName = `${Item} Relic`;
        if (Core.checkInventory(relicName))
            return;

        let questID: number;
        if (bot.flash.get<number>("world.myAvatar.objData.intAQ") > 0) {
            Farm.bladeofAweREP(5, false);
            Farm.experience(35);
            Core.buyItem("tower", 53, 29403, 1, 18569);
            questID = LegendQuest + 1;
        } else {
            Farm.bladeofAweREP(10, false);
            Farm.experience(55);

            Core.buyItem(
                "museum", 
                1130, 
                Core.isMember ? 29402 : 29404, 1, Core.isMember ? 18580 : 18579);
            questID = Core.isMember ? LegendQuest : LegendQuest + 2;
        }

        Core.equipClass(ClassType.Solo);
        Core.addDrop(`${Item} Shard`, `${Item} Fragment`);
        Core.farmingLogger(`${Item} Fragment`, FragmentAmount);

        while (!bot.shouldExit && !Core.checkInventory(`${Item} Fragment`, FragmentAmount)) {
            Core.ensureAccept(questID);
            if (Map.toLowerCase() === "doomvault" || Map.toLowerCase() === "doomvaultb")
                Core.killMonster(Map, Map.toLowerCase().endsWith("b") ? "r26" : "r5", "Left", Monster, `${Item} Shard`, ShardAmount, { isTemp: false });
            else
                Core.huntMonster(Map, Monster, `${Item} Shard`, ShardAmount, { isTemp: false });
            Core.ensureComplete(questID);
            bot.wait.forPickup(`${Item} Fragment`);
        }

        Core.buyItem("museum", 1129, `${Item} Relic`);
    }
}
