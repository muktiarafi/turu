/*
name: SwirlingTheAbyss
description: null
tags: null
*/
// Port of Scripts/Nation/Various/SwirlingTheAbyss.cs.

import { bot } from "turu";
import { Core } from "../../core/CoreBots";
import { Fiendshard_Story } from "../../story/nation/Fiendshard";
import { CoreNation } from "../CoreNation";

const Nation = new CoreNation();
const Fiendshard = new Fiendshard_Story();

export class SwirlingTheAbyss {
    readonly rewards: string[] = [
        "ArchFiend Deathlord Armet",
        "ArchFiend Deathlord Horned Armet",
        "ArchFiend Deathlord Helmet",
        "ArchFiend Deathlord Mask",
        "ArchFiend Deathlord Locks",
        "ArchFiend Deathlord Shag",
        "DeathLord Cloak of Nulgath",
        "Void Soul of Nulgath",
        "DeathLord Spines of Nulgath",
        "Deathless Wings of Nulgath",
    ];

    sta(item: string | null = null): void {
        if (item != null && Core.checkInventory(item))
            return;

        if (item != null)
            Core.logger(`Farming ${item}.`);
        else
            Core.logger("Farming Swirling the Abyss Quest Rewards.");
        Fiendshard.fiendshard_QuestlineP1();
        Core.addDrop(...Nation.bagDrops);
        Core.addDrop(...this.rewards);
        const Item = Core.ensureLoad(7899).rewards.find(x => x.name === item);

        let i = 1;
        while (
            !bot.shouldExit
            && (
                Item != null
                    ? !Core.checkInventory(Item!.name)
                    : !Core.checkInventory(this.rewards, 1, { toInv: false })
            )
        ) {
            Nation.farmBloodGem(10);
            Nation.farmUni13(3);
            Nation.farmTaintedGem(75);
            Nation.farmDarkCrystalShard(50);
            Nation.farmGemofNulgath(50);
            Nation.farmVoucher({ member:true });

            Core.ensureAccept(7899);

            if (Item != null) {
                Core.ensureComplete(7899, Item.id);
                bot.wait.forPickup(Item!.name);
            } else {
                Core.ensureCompleteChoose(7899, Core.questRewards(7899));
                for (const thing of Core.questRewards(7899))
                    if (bot.drops.exists(thing))
                        bot.wait.forPickup(thing);
                Core.toBank(...this.rewards);
                Core.logger(`Completed x${i++}.`);
            }
        }
    }
}

export default function main(): void {
    const script = new SwirlingTheAbyss();
        Core.bankingBlackList.push(...Nation.bagDrops);
        Core.bankingBlackList.push(...script.rewards);
        Core.setOptions();

        script.sta();

        Core.setOptions(false);
}
