/*
name: Shadowflame Finale Merge
description: This bot will farm the items belonging to the selected mode for the Shadowflame Finale Merge [2156] in /ruinedcrown
tags: shadowflame, finale, merge, ruinedcrown, defender, defenders, crest, horn, horned, skull, wing, spear, enchanted, warrior, mage, healer, rogue, rogues, mortal
*/
// Port of Scripts/ShadowsOfWar/MergeShops/ShadowflameFinaleMerge.cs.

import { bot, defineScriptOptions, option } from "turu";
import { CoreAdvanced, mergeOptionsEnum } from "../../core/CoreAdvanced";
import { ClassType, Core } from "../../core/CoreBots";
import { CoreFarms } from "../../core/CoreFarms";
import { CoreSoW } from "../../story/shadowsofwar/CoreSoW";
import { CoreSoWMats } from "../CoreSoWMats";

const Farm = new CoreFarms();
const Adv = new CoreAdvanced();
const SOWM = new CoreSoWMats();
const SoW = new CoreSoW();

export class ShadowflameFinaleMerge {
    static readonly select = [
        option.bool("70606", "ShadowFlame Defender", "Mode: [select] only\nShould the bot buy \"ShadowFlame Defender\" ?", false),
        option.bool("70607", "ShadowFlame Defender's Crest", "Mode: [select] only\nShould the bot buy \"ShadowFlame Defender's Crest\" ?", false),
        option.bool("70608", "ShadowFlame Defender's Hair", "Mode: [select] only\nShould the bot buy \"ShadowFlame Defender's Hair\" ?", false),
        option.bool("70609", "ShadowFlame Defender's Horn", "Mode: [select] only\nShould the bot buy \"ShadowFlame Defender's Horn\" ?", false),
        option.bool("70611", "ShadowFlame Defender's Horned Skull", "Mode: [select] only\nShould the bot buy \"ShadowFlame Defender's Horned Skull\" ?", false),
        option.bool("70612", "ShadowFlame Defender's Wing", "Mode: [select] only\nShould the bot buy \"ShadowFlame Defender's Wing\" ?", false),
        option.bool("70616", "ShadowFlame Defender's Spear", "Mode: [select] only\nShould the bot buy \"ShadowFlame Defender's Spear\" ?", false),
        option.bool("71601", "Enchanted ShadowFlame Warrior", "Mode: [select] only\nShould the bot buy \"Enchanted ShadowFlame Warrior\" ?", false),
        option.bool("71602", "Enchanted ShadowFlame Mage", "Mode: [select] only\nShould the bot buy \"Enchanted ShadowFlame Mage\" ?", false),
        option.bool("71603", "Enchanted ShadowFlame Healer", "Mode: [select] only\nShould the bot buy \"Enchanted ShadowFlame Healer\" ?", false),
        option.bool("71604", "Enchanted ShadowFlame Rogue", "Mode: [select] only\nShould the bot buy \"Enchanted ShadowFlame Rogue\" ?", false),
        option.bool("71605", "Enchanted Rogue's Mask", "Mode: [select] only\nShould the bot buy \"Enchanted Rogue's Mask\" ?", false),
        option.bool("71606", "Enchanted Rogue's Mortal Locks", "Mode: [select] only\nShould the bot buy \"Enchanted Rogue's Mortal Locks\" ?", false),
        option.bool("71607", "Enchanted Rogue's Locks", "Mode: [select] only\nShould the bot buy \"Enchanted Rogue's Locks\" ?", false),
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

                case "Willpower":
                    SOWM.willpower(quant);
                    break;

                case "ShadowFlame Healer":
                case "ShadowFlame Warrior":
                case "ShadowFlame Mage":
                    Core.equipClass(ClassType.Farm);
                    Core.huntMonster("ruinedcrown", "Mana-Burdened Mage", req.name, 1, { isTemp: false });
                    break;

                case "ShadowFlame Rogue":
                case "ShadowFlame Rogue's Mask":
                case "ShadowFlame Rogue's Locks":
                case "ShadowFlame Rogue's Mortal Locks":
                    Core.equipClass(ClassType.Farm);
                    Core.huntMonster("ruinedcrown", "Mana-Burdened Minion", req.name, 1, { isTemp: false });
                    break;
            }
        };

        SoW.ruinedCrown();
        //Only edit the map and shopID here
        Adv.startBuyAllMerge("ruinedcrown", 2156, findIngredients, buyOnlyThis, { buyMode: buyMode });
    }
}

export const scriptOptions = defineScriptOptions(Adv.optionsStorage, [], true, {
    Generic: Adv.mergeOptions,
    Select: ShadowflameFinaleMerge.select,
});

export default function main(): void {
        Core.bankingBlackList.push(...
            [
                "Willpower",
                "ShadowFlame Warrior",
                "ShadowFlame Mage",
                "ShadowFlame Healer",
                "ShadowFlame Rogue",
                "ShadowFlame Rogue's Mask",
                "ShadowFlame Rogue's Mortal Locks",
                "ShadowFlame Rogue's Locks",
            ]);
        Core.setOptions();

        new ShadowflameFinaleMerge().buyAllMerge();
        Core.setOptions(false);
}
