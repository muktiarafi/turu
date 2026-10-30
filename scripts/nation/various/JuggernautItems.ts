/*
name: JuggernautItems
description: null
tags: null
*/
// Port of Scripts/Nation/Various/JuggernautItems.cs.

import { bot, choices, defineScriptOptions, option } from "turu";
import { Core, configEnum, configOr } from "../../core/CoreBots";
import { CoreNation } from "../CoreNation";

const Nation = new CoreNation();

export class JuggernautItemsofNulgath {
    readonly rewards: string[] = [
        "Oblivion of Nulgath",
        "Ungodly Reavers of Nulgath",
        "Warlord of Nulgath",
        "Arcane of Nulgath",
        "Dimensional Champion of Nulgath",
        "Crystal Phoenix Blade of Nulgath",
        "Overfiend Blade of Nulgath",
        "Battlefiend Blade of Nulgath",
        "Dark Makai of Nulgath",
        "Nulgath Armor",
        "Polish Hussar",
        "Polish Hussar Helm",
        "Polish Hussar Spear",
        "Polish Hussar Wings",
        "Void Cowboy",
        "Void Cowboy Hat",
        "Void Cowboy Morph",
        "Void Cowboy's Mask + Locks",
        "Void Cowboy's Pistol",
        "Dual Void Cowboy Pistols",
    ];

    juggItems(reward: RewardsSelection = RewardsSelection.All): void {
        Core.addDrop(...Nation.bagDrops);

        let Count = 0;
        let x = 1;

        const RewardOptions = Core.ensureLoad(837).rewards;
        const RewardsList: string[] = [];
        for (const Item of RewardOptions) {
            bot.drops.add(Item.id);
            RewardsList.push(Item.name);
        }
        Count = RewardsList.length;
        const rewards = Core.ensureLoad(837).rewards;
        const item = rewards.find(x => x.id === reward) ?? null;
        const defaultId = 0;
        const itemId = item?.id ?? defaultId;
        while (
            !bot.shouldExit
            && (
                reward === RewardsSelection.All
                    ? !Core.checkInventory(rewards.map(x => x.name), 1, { toInv: false })
                    : !Core.checkInventory(itemId, 1, { toInv: false })
            )
        ) {
            if (reward === RewardsSelection.All)
                Core.logger(`Farming All ${x++}/${Count}`);
            else
                Core.logger(`... ${RewardsSelection[reward]} ...`);

            Nation.farmUni13(1);
            Core.ensureAccept(837);
            Nation.farmDiamondofNulgath(13);
            Nation.farmDarkCrystalShard(50);
            Nation.farmTotemofNulgath(3);
            Nation.farmGemofNulgath(20);
            Nation.farmVoucher({ member:false });
            Nation.farmTaintedGem(50);
            Core.huntMonster("underworld", "Undead Bruiser", "Undead Bruiser Rune");

            if (configEnum("RewardsSelection", RewardsSelection, 0 as RewardsSelection) === RewardsSelection.All)
                Core.ensureCompleteChoose(837);
            else
                Core.ensureComplete(837, reward);
        }
    }
}

export enum RewardsSelection {
    Oblivion_of_Nulgath = 2232,
    Ungodly_Reavers_of_Nulgath = 4939,
    Warlord_of_Nulgath = 5527,
    Arcane_of_Nulgath = 5530,
    Dimensional_Championof_Nulgath = 5531,
    Crystal_Phoenix_Blade_of_Nulgath = 6137,
    Overfiend_Blade_of_Nulgath = 6138,
    Battlefiend_Blade_of_Nulgath = 6141,
    Dark_Makaiof_Nulgath = 6142,
    Nulgath_Armor = 6375,
    Polish_Hussar = 42596,
    Polish_Hussar_Helm = 42597,
    Polish_Hussar_Spear = 42598,
    Polish_Hussar_Wings = 42599,
    Void_Cowboy = 52799,
    Void_CowboyHat = 52800,
    Void_CowboyMorph = 52801,
    Void_Cowboys_Mask_Locks = 52802,
    Void_Cowboy_Pistol = 52803,
    Dual_Void_Cowboy_Pistols = 52818,
    All,
}

export const scriptOptions = defineScriptOptions("JuggernautItemsofNulgath", [
    Core.skipOptions,
    option.choice("RewardsSelection", "Select Your Quest Reward", "Select Your Quest Reward for The JuggerNaught items of Nulgath quest.", choices(RewardsSelection), "Oblivion_of_Nulgath"),
], true);

export default function main(): void {
    const script = new JuggernautItemsofNulgath();
        Core.bankingBlackList.push(...Nation.bagDrops);
        Core.bankingBlackList.push(...script.rewards);
        Core.setOptions();

        script.juggItems(configEnum("RewardsSelection", RewardsSelection, 0 as RewardsSelection));

        Core.setOptions(false);
}
