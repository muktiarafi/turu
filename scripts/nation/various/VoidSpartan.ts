/*
name: VoidSpartan
description: null
tags: null
*/
// Port of Scripts/Nation/Various/VoidSpartan.cs.

import { bot } from "turu";
import { Core } from "../../core/CoreBots";
import type { QuestData } from "../../core/CoreBots";
import { CoreFarms } from "../../core/CoreFarms";
import { CoreNation } from "../CoreNation";

const Farm = new CoreFarms();
const Nation = new CoreNation();

export class VoidSpartan {
    readonly rewards: string[] = [
        "Void Spartan",
        "Void Spartan Daggers",
        "Void Spartan Blade and Shield",
        "Void Spartan Spear",
        "Void Spartan Blade",
        "Void Spartan Banners",
        "Void Spartan Cape",
        "Void Spartan Shielded Cape",
        "Void Spartan Spear and Shield",
        "Void Spartan Helm",
        "Void Spartan Helm and Scarf",
    ];

    getSpartan(item: string | null = null): void {
        Core.addDrop(
            ...Nation
                .bagDrops.concat(this.rewards)
                .concat([ "Zee's Red Jasper", "Fiend Cloak of Nulgath" ]));

        const QuestData = Core.initializeWithRetries(() => Core.ensureLoad(5982));
        if (QuestData == null) {
            Core.logger("Quest 5982 not found, please report this to the devs.");
            return;
        }
        const Item = Core.ensureLoad(5982).rewards.find(x => x.name === item);

        if (item == null)
            Core.logger("Farming Void Spartan Set.");
        else
            Core.logger(`Farming ${item}.`);

        let i = 1;
        while (
            !bot.shouldExit
            && (
                Item != null
                    ? !Core.checkInventory(Item!.name)
                    : !Core.checkInventory(this.rewards, 1, { toInv: false })
            )
        ) {
            Core.ensureAccept(5982);

            Nation.farmUni13(1);
            Nation.farmBloodGem(5);
            Nation.farmGemofNulgath(10);
            bot.quests.updateQuest(4055);
            Core.huntMonster("pyrewatch", "Flame Soldier", "Zee's Red Jasper", 1, { isTemp: false });
            //jumpwait jumps to same cell which is auto-aggro, spawn("Enter") isnt, so force jump there.
            Core.jumpWait();
            // Core.Jump("Enter", "Spawn");
            Farm.gold(500000);
            Core.buyItem("tercessuinotlim", 68, "Fiend Cloak of Nulgath");

            if (Item != null) {
                Core.ensureComplete(5982, Item.id);
                bot.wait.forPickup(Item!.name);
            } else {
                Core.ensureCompleteChoose(5982, Core.questRewards(5982));
                for (const thing of Core.questRewards(5982))
                    if (bot.drops.exists(thing))
                        bot.wait.forPickup(thing);
            }
            Core.logger(`Completed ${QuestData.name}[${QuestData.id}] x${i++}`);
        }
        i = 0;
    }
}

export default function main(): void {
    const script = new VoidSpartan();
        Core.bankingBlackList.push(...Nation.bagDrops);
        Core.bankingBlackList.push(...script.rewards);
        Core.setOptions();

        script.getSpartan();

        Core.setOptions(false);
}
