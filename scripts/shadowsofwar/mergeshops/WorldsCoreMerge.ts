/*
name: WorldsCoreMerge
description: null
tags: null
*/
// Port of Scripts/ShadowsOfWar/MergeShops/WorldsCoreMerge.cs.

import { bot, defineScriptOptions, option } from "turu";
import { CoreAdvanced, mergeOptionsEnum } from "../../core/CoreAdvanced";
import { Core } from "../../core/CoreBots";
import { CoreSoW } from "../../story/shadowsofwar/CoreSoW";
import { CoreSoWMats } from "../CoreSoWMats";

const Adv = new CoreAdvanced();
const SOWM = new CoreSoWMats();
const SoW = new CoreSoW();

export class WorldsCoreMerge {
    static readonly select = [
        option.bool("73570", "Mystical Devotee of Mana", "Mode: [select] only\nShould the bot buy \"Mystical Devotee of Mana\" ?", false),
        option.bool("73571", "Mythical Devotee's Helm", "Mode: [select] only\nShould the bot buy \"Mythical Devotee's Helm\" ?", false),
        option.bool("73572", "Mystical Devotee's Hair", "Mode: [select] only\nShould the bot buy \"Mystical Devotee's Hair\" ?", false),
        option.bool("73573", "Mystical Devotee's Locks", "Mode: [select] only\nShould the bot buy \"Mystical Devotee's Locks\" ?", false),
        option.bool("73574", "Mythical Devotee's Veil + Locks", "Mode: [select] only\nShould the bot buy \"Mythical Devotee's Veil + Locks\" ?", false),
        option.bool("73575", "Miraculous Orb Of Divination", "Mode: [select] only\nShould the bot buy \"Miraculous Orb Of Divination\" ?", false),
        option.bool("73576", "Radiant Orb Of Clarity", "Mode: [select] only\nShould the bot buy \"Radiant Orb Of Clarity\" ?", false),
        option.bool("73577", "Mystical Orb Of Illusions", "Mode: [select] only\nShould the bot buy \"Mystical Orb Of Illusions\" ?", false),
        option.bool("73578", "Manifestation of Mana", "Mode: [select] only\nShould the bot buy \"Manifestation of Mana\" ?", false),
        option.bool("73579", "Mystical Devotee's Scimitar", "Mode: [select] only\nShould the bot buy \"Mystical Devotee's Scimitar\" ?", false),
        option.bool("73580", "Mystical Devotee's Scimitars", "Mode: [select] only\nShould the bot buy \"Mystical Devotee's Scimitars\" ?", false),
        option.bool("73581", "Staff Of Miracles", "Mode: [select] only\nShould the bot buy \"Staff Of Miracles\" ?", false),
        option.bool("73582", "Wand Of Miracles", "Mode: [select] only\nShould the bot buy \"Wand Of Miracles\" ?", false),
        option.bool("73583", "Mystical Chakram Of Illusions", "Mode: [select] only\nShould the bot buy \"Mystical Chakram Of Illusions\" ?", false),
        option.bool("73584", "Mystical Chakrams Of Illusion", "Mode: [select] only\nShould the bot buy \"Mystical Chakrams Of Illusion\" ?", false),
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

                case "Acquiescence":
                    SOWM.acquiescence(quant);
                    break;

                case "Unbound Thread":
                    SOWM.unboundThread(quant);
                    break;

                case "Prismatic Seams":
                    SOWM.prismaticSeams(quant);
                    break;
            }
        };

        SoW.shadowFlame();
        //Only edit the map and shopID here
        Adv.startBuyAllMerge("worldscore", 2182, findIngredients, buyOnlyThis, { buyMode: buyMode });
    }
}

export const scriptOptions = defineScriptOptions(Adv.optionsStorage, [], true, {
    Generic: Adv.mergeOptions,
    Select: WorldsCoreMerge.select,
});

export default function main(): void {
        Core.bankingBlackList.push(...
            [ "Acquiescence", "Unbound Thread", "Prismatic Seams " ]);
        Core.setOptions();

        new WorldsCoreMerge().buyAllMerge();

        Core.setOptions(false);
}
