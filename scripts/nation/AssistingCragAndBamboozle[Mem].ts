/*
name: AssistingCragAndBamboozle[Mem]
description: null
tags: null
*/
// Port of Scripts/Nation/AssistingCragAndBamboozle[Mem].cs.

import { bot, choices, defineScriptOptions, option } from "turu";
import type { InventoryItem } from "turu";
import { Core, configEnum, configOr } from "../core/CoreBots";
import type { RewardItem } from "../core/CoreBots";
import { CoreDailies } from "../core/CoreDailies";
import { CoreNation } from "./CoreNation";

const Daily = new CoreDailies();
const Nation = new CoreNation();

export class AssistingCragAndBamboozle {
    readonly aCaBItems: string[] = [
        "Sword of Nulgath",
        "Gem of Nulgath",
        "Tainted Gem",
        "Dark Crystal Shard",
        "Diamond of Nulgath",
        "Totem of Nulgath",
        "Blood Gem of the Archfiend",
        "Unidentified 19",
        "Elders' Blood",
        "Voucher of Nulgath",
        "Voucher of Nulgath (non-mem)",
        "Archfiend's Favor",
        "Essence of Nulgath",
        "Nulgath Larvae",
        "Gem of Domniation",
        "Fiend Seal",
    ];

    assistingCandB(reward: Rewards = Rewards.Get_whats_not_maxed): void {
        if (!Core.isMember ||
            !Core.checkInventory(Nation.cragName) ||
            (!Core.checkInventory("Sparrow's Blood") &&
             !Daily.checkDailyv2(803, ["Sparrow's Blood"], { any: true, shouldUnBank: true })))
            return;

        // Core.ensureLoad: its rewards carry the max stacks (C# `ItemBase.MaxStack`).
        const quest = Core.initializeWithRetries(() => Core.ensureLoad(5817));
        if (quest?.rewards == null) {
            Core.logger("Quest 5817 failed to load.");
            return;
        }

        // Build target pool once
        const targets = quest.rewards
            .filter(r => reward === Rewards.Get_whats_not_maxed
                ? !Core.checkInventory(r.id, r.maxStack)
                : r.id === reward && !Core.checkInventory(r.id, r.maxStack));

        if (targets.length === 0) {
            Core.logger("Everything already maxed. No work needed.");
            return;
        }

        Core.addDrop(
            "Nulgath Larvae", "Sparrow's Blood", "Sword of Nulgath", "Gem of Nulgath", 
            "Tainted Gem", "Dark Crystal Shard", "Diamond of Nulgath", "Totem of Nulgath", 
            "Blood Gem of the Archfiend", "Unidentified 19", "Elders' Blood", 
            "Voucher of Nulgath", "Voucher of Nulgath (non-mem)");

        Core.logger(`AssistingCandB Optimized → Targets: ${targets.length}`);

        while (!bot.shouldExit && targets.length > 0) {
            Core.ensureAccept(5817);

            if (!Core.checkInventory("Tendurrr The Assistant"))
                Core.killMonster("tercessuinotlim", "m2", "Left", "*", "Tendurrr The Assistant", 1, { isTemp: false });

            Daily.sparrowsBlood(1);
            bot.wait.forPickup(5584);

            if (!Core.checkInventory("Sparrow's Blood")) {
                Core.logger("Missing Sparrow's Blood. Stopping.");
                return;
            }

            Nation.essenceofNulgath(20);
            Nation.approvalAndFavor(100, 100);
            Nation.nationRound4Medal();

            Core.ensureAccept(4748);
            Core.huntMonster("shadowblast", "Legion Fenrir", "Fiend Seal", 10, { isTemp: false });

            // ---- SMART BATCH EXECUTION ----
            const completedThisCycle: RewardItem[] = [];

            for (const target of targets) {
                if (Core.checkInventory(target.id, target.maxStack)) {
                    completedThisCycle.push(target);
                    continue;
                }

                Core.ensureComplete(5817, target.id);

                if (Core.checkInventory(target.id, target.maxStack))
                    completedThisCycle.push(target);

                if (!Core.checkInventory("Sparrow's Blood")) {
                    Core.logger(`Stopped early after completing ${target.name}`);
                    return;
                }
            }

            // prune finished items in one go (fast + clean)
            for (let k = targets.length - 1; k >= 0; k--)
                if (completedThisCycle.some(c => c.id === targets[k].id))
                    targets.splice(k, 1);
        }

        Core.logger("AssistingCandB Optimized → COMPLETE.");
    }
}

export enum Rewards {
    Blood_Gem_of_the_Archfiend = 22332,
    Gem_of_Nulgath = 6136,
    Elders_Blood = 5586,
    Totem_of_Nulgath = 5357,
    Voucher_of_Nulgath_non_mem = 4862,
    Voucher_of_Nulgath = 4861,
    Diamond_of_Nulgath = 4771,
    Dark_Crystal_Shard = 4770,
    Tainted_Gem = 4769,
    Unidentified_19 = 4752,
    Sword_of_Nulgath = 4670,
    Get_whats_not_maxed = -1,
}

export const scriptOptions = defineScriptOptions("AssistingCragAndBamboozle", [
    option.choice("PickReward", "Pick your reward", "This quest is a daily.. unless you have multiple sparrows blood then itll keep running.", choices(Rewards), "Get_whats_not_maxed"),
    Core.skipOptions,
], true);

export default function main(): void {
        Core.bankingBlackList.push(...
            [
                Nation.cragName,
                "Gem of Nulgath",
                "Tainted Gem",
                "Dark Crystal Shard",
                "Diamond of Nulgath",
                "Totem of Nulgath",
                "Blood Gem of the Archfiend",
            ]);

        Core.setOptions();

        new AssistingCragAndBamboozle().assistingCandB(configEnum("PickReward", Rewards, 0 as Rewards));

        Core.setOptions(false);
}
