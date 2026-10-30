/*
name: TheLeeryContract[Member]
description: null
tags: null
*/
// Port of Scripts/Nation/Various/TheLeeryContract[Member].cs.

import { choices, defineScriptOptions, option } from "turu";
import { Core, configEnum } from "../../core/CoreBots";
import type { RewardItem } from "../../core/CoreBots";
import { CoreNation } from "../CoreNation";

const Nation = new CoreNation();

export class TheLeeryContract {
    readonly rewards: string[] = [
        "Ddog Sea Serpent Sword",
        "Godly Golden Dragon Axe",
        "Corpse Maker of Nulgath",
    ];

    questItems(reward: RewardsSelection = RewardsSelection.All): void {
        if (!Core.isMember) {
            Core.logger("You need to be a member to do this quest.");
            return;
        }

        const RewardOptions = Core.ensureLoad(554).rewards.map(x => x.name);
        Core.addDrop(...RewardOptions);
        // C# reads the "RewardSelect" option here and ignores `reward`, so DirtlickersMerge's
        // Corpse Maker request did nothing from DoAll: fixed, main() passes the option instead.
        const getAll = reward === RewardsSelection.All;

        let item: RewardItem | null = null;
        if (!getAll) {
            item = Core.ensureLoad(554).rewards.find(x => x.id === reward) ?? null;
            if (item == null) {
                Core.logger(`${RewardsSelection[reward] ?? reward} not found in Quest Rewards`);
                return;
            }
            if (Core.checkInventory(item.name))
                return;
            Core.farmingLogger(item.name, 1);
        }

        while (getAll ? !Core.checkInventory(RewardOptions) : !Core.checkInventory(item!.name)) {
            Core.ensureAccept(554);
            Nation.farmUni13(1);
            Core.huntMonster("EvilWarNul", "Undead Legend", "Undead Legend Rune");

            if (!getAll)
                Core.ensureComplete(554, item!.id);
            else
                Core.ensureCompleteChoose(554);
        }
    }
}

export enum RewardsSelection {
    Ddog_Sea_Serpent_Sword = 4766,
    Godly_Golden_Dragon_Axe = 4724,
    Corpse_Maker_of_Nulgath = 4764,
    All = 9999,
}

export const scriptOptions = defineScriptOptions("The Leery Contract", [
    Core.skipOptions,
    option.choice("RewardSelect", "Choose Your Quest Reward", "Select Your Quest Reward for The Leary Contract.", choices(RewardsSelection), "Godly_Golden_Dragon_Axe"),
], true);

export default function main(): void {
    const script = new TheLeeryContract();
    Core.setOptions();
    Core.bankingBlackList.push(...script.rewards);
    script.questItems(configEnum("RewardSelect", RewardsSelection, RewardsSelection.All));

    Core.setOptions(false);
}
