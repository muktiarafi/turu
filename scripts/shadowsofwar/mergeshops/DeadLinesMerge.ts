/*
name: DeadLinesMerge
description: null
tags: null
*/
// Port of Scripts/ShadowsOfWar/MergeShops/DeadLinesMerge.cs.

import { bot, defineScriptOptions, option } from "turu";
import { CoreAdvanced, mergeOptionsEnum } from "../../core/CoreAdvanced";
import { Core } from "../../core/CoreBots";
import { CoreSoWMats } from "../CoreSoWMats";

const Adv = new CoreAdvanced();
const SOWM = new CoreSoWMats();

export class DeadLinesMerge {
    static readonly select = [
        option.bool("70580", "Timestream Ravager", "Mode: [select] only\nShould the bot buy \"Timestream Ravager\" ?", false),
        option.bool("70581", "Timestream Ravager's Visor", "Mode: [select] only\nShould the bot buy \"Timestream Ravager's Visor\" ?", false),
        option.bool("70582", "Timestream Ravager's Hat", "Mode: [select] only\nShould the bot buy \"Timestream Ravager's Hat\" ?", false),
        option.bool("70583", "Timestream Ravager's Morph", "Mode: [select] only\nShould the bot buy \"Timestream Ravager's Morph\" ?", false),
        option.bool("70584", "Timestream Ravager's Morph + Hat", "Mode: [select] only\nShould the bot buy \"Timestream Ravager's Morph + Hat\" ?", false),
        option.bool("70586", "Timestream Ravager's Symbol", "Mode: [select] only\nShould the bot buy \"Timestream Ravager's Symbol\" ?", false),
        option.bool("70588", "Timestream Ravager's Cutlass", "Mode: [select] only\nShould the bot buy \"Timestream Ravager's Cutlass\" ?", false),
        option.bool("70589", "Timestream Ravager's Cutlasses", "Mode: [select] only\nShould the bot buy \"Timestream Ravager's Cutlasses\" ?", false),
        option.bool("70590", "Timestream Ravager's Dark Hook", "Mode: [select] only\nShould the bot buy \"Timestream Ravager's Dark Hook\" ?", false),
        option.bool("70591", "Timestream Ravager's Dark Hooks", "Mode: [select] only\nShould the bot buy \"Timestream Ravager's Dark Hooks\" ?", false),
        option.bool("70592", "Timestream Ravager's Pistol", "Mode: [select] only\nShould the bot buy \"Timestream Ravager's Pistol\" ?", false),
        option.bool("70593", "Timestream Ravager's Pistols", "Mode: [select] only\nShould the bot buy \"Timestream Ravager's Pistols\" ?", false),
        option.bool("72292", "Enchanted ShadowFlame Portal", "Mode: [select] only\nShould the bot buy \"Enchanted ShadowFlame Portal\" ?", false),
    ];

    private dontStopMissingIng = false;

    buyAllMerge(buyOnlyThis: string | null = null, buyMode: mergeOptionsEnum | null = null): void {
        // C# local functions
        const findIngredients = (): void => {
            const req = Adv.externalItem;
            const quant = Adv.externalQuant;
            const currentQuant = req.temp
                ? bot.tempInv.getQuantity(req.name)
                : bot.inventory.getQuantity(req.name);
            if (req == null) {
                Core.logger("req is NULL");
                return;
            }

            switch (req.name) {
                default:
                    const shouldStop = !Adv.matsOnly || !this.dontStopMissingIng;
                    Core.logger(
                        `The bot hasn't been taught how to get ${req.name}.`
                            + (shouldStop ? " Please report the issue." : " Skipping"), "", { messageBox: shouldStop, stopBot: shouldStop });
                    break;
        //#endregion

                case "Unbound Thread":
                    SOWM.unboundThread(quant);
                    break;
            }
        };

        //Only edit the map and shopID here
        Adv.startBuyAllMerge("deadlines", 2169, findIngredients, buyOnlyThis, { buyMode: buyMode });
    }
}

export const scriptOptions = defineScriptOptions(Adv.optionsStorage, [], true, {
    Generic: Adv.mergeOptions,
    Select: DeadLinesMerge.select,
});

export default function main(): void {
        Core.bankingBlackList.push(...[ "Unbound Thread " ]);
        Core.setOptions();

        new DeadLinesMerge().buyAllMerge();

        Core.setOptions(false);
}
