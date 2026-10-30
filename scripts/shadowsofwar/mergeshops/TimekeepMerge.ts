/*
name: TimekeepMerge
description: null
tags: null
*/
// Port of Scripts/ShadowsOfWar/MergeShops/TimekeepMerge.cs.

import { bot, defineScriptOptions, option } from "turu";
import { CoreAdvanced, mergeOptionsEnum } from "../../core/CoreAdvanced";
import { Core } from "../../core/CoreBots";
import { CoreSoW } from "../../story/shadowsofwar/CoreSoW";
import { CoreSoWMats } from "../CoreSoWMats";

const Adv = new CoreAdvanced();
const SOWM = new CoreSoWMats();
const SoW = new CoreSoW();

export class TimekeepMerge {
    static readonly select = [
        option.bool("71608", "Mana Guardian", "Mode: [select] only\nShould the bot buy \"Mana Guardian\" ?", false),
        option.bool("71609", "Mana Guardian's Horn", "Mode: [select] only\nShould the bot buy \"Mana Guardian's Horn\" ?", false),
        option.bool("71610", "Mana Guardian's Helm", "Mode: [select] only\nShould the bot buy \"Mana Guardian's Helm\" ?", false),
        option.bool("71611", "Mana Guardian's Morph", "Mode: [select] only\nShould the bot buy \"Mana Guardian's Morph\" ?", false),
        option.bool("71612", "Mana Guardian's Rift", "Mode: [select] only\nShould the bot buy \"Mana Guardian's Rift\" ?", false),
        option.bool("71613", "Mana Guardian's Orb", "Mode: [select] only\nShould the bot buy \"Mana Guardian's Orb\" ?", false),
        option.bool("71614", "Mana Guardian's Spear", "Mode: [select] only\nShould the bot buy \"Mana Guardian's Spear\" ?", false),
        option.bool("71615", "Mana Guardian's Hammer", "Mode: [select] only\nShould the bot buy \"Mana Guardian's Hammer\" ?", false),
        option.bool("71616", "Mana Guardian's Hammers", "Mode: [select] only\nShould the bot buy \"Mana Guardian's Hammers\" ?", false),
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

                case "Garish Remnant":
                    SOWM.garishRemnant(quant);
                    break;
            }
        };

        SoW.timekeep();
        //Only edit the map and shopID here
        Adv.startBuyAllMerge("timekeep", 2161, findIngredients, buyOnlyThis, { buyMode: buyMode });
    }
}

export const scriptOptions = defineScriptOptions(Adv.optionsStorage, [], true, {
    Generic: Adv.mergeOptions,
    Select: TimekeepMerge.select,
});

export default function main(): void {
        Core.setOptions();

        new TimekeepMerge().buyAllMerge();

        Core.setOptions(false);
}
