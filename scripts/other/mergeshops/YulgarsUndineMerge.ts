/*
name: Yulgars Undine Merge
description: This bot will farm the items belonging to the selected mode for the Yulgars Undine Merge [2297] in /sunlightzone
tags: yulgars, undine, merge, sunlightzone, poisonous, rogue, nightshade, thorn, assasasin, reversed, thorns, envenomed, whip, agony, elven, assassins, scarf, , assassin, guardian, wreath
*/
// Port of Scripts/Other/MergeShops/YulgarsUndineMerge.cs.

import { bot, defineScriptOptions, option } from "turu";
import { CoreAdvanced, mergeOptionsEnum } from "../../core/CoreAdvanced";
import { ClassType, Core } from "../../core/CoreBots";
import { CoreFarms } from "../../core/CoreFarms";
import { CoreAOR } from "../../story/ageofruin/CoreAOR";

const Farm = new CoreFarms();
const Adv = new CoreAdvanced();
const AoR = new CoreAOR();

export class YulgarsUndineMerge {
    static readonly select = [
        option.bool("78285", "Poisonous Rogue", "Mode: [select] only\nShould the bot buy \"Poisonous Rogue\" ?", false),
        option.bool("78286", "Nightshade Thorn Assasasin", "Mode: [select] only\nShould the bot buy \"Nightshade Thorn Assasasin\" ?", false),
        option.bool("78293", "Reversed Blade of Thorns", "Mode: [select] only\nShould the bot buy \"Reversed Blade of Thorns\" ?", false),
        option.bool("78294", "Reversed Daggers of Thorns", "Mode: [select] only\nShould the bot buy \"Reversed Daggers of Thorns\" ?", false),
        option.bool("78296", "Envenomed Gauntlet", "Mode: [select] only\nShould the bot buy \"Envenomed Gauntlet\" ?", false),
        option.bool("78295", "Envenomed Whip of Agony", "Mode: [select] only\nShould the bot buy \"Envenomed Whip of Agony\" ?", false),
        option.bool("78288", "Elven Assassin's Scarf", "Mode: [select] only\nShould the bot buy \"Elven Assassin's Scarf\" ?", false),
        option.bool("78290", "Elven Assassin's Scarf + Locks", "Mode: [select] only\nShould the bot buy \"Elven Assassin's Scarf + Locks\" ?", false),
        option.bool("78292", "Nightshade Assassin Guardian", "Mode: [select] only\nShould the bot buy \"Nightshade Assassin Guardian\" ?", false),
        option.bool("78287", "Elven Assassin's Hair", "Mode: [select] only\nShould the bot buy \"Elven Assassin's Hair\" ?", false),
        option.bool("78289", "Elven Assassin Locks", "Mode: [select] only\nShould the bot buy \"Elven Assassin Locks\" ?", false),
        option.bool("78291", "Poisonous Thorn Wreath", "Mode: [select] only\nShould the bot buy \"Poisonous Thorn Wreath\" ?", false),
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

                case "Venomous Rose":
                    Core.farmingLogger(req.name, quant);
                    Core.equipClass(ClassType.Solo);
                    Core.registerQuests(9274);
                    while (!bot.shouldExit && !Core.checkInventory(req.name, quant)) {
                        Core.huntMonster("twilightzone", "Leviathan", null, 1, { log: false });
                        bot.wait.forPickup(req.name);
                    }
                    Core.cancelRegisteredQuests();
                    break;

                case "Undine Visitor Badge":
                    Core.farmingLogger(req.name, quant);
                    Core.equipClass(ClassType.Farm);
                    Core.huntMonster("sunlightzone", "Astravian Illusion", req.name, quant, { isTemp: false, log: false });
                    break;
            }
        };

        //doall because its also required for shadowrealmmerge which includes this script.
        AoR.doAll();
        //Only edit the map and shopID here
        Adv.startBuyAllMerge("sunlightzone", 2297, findIngredients, buyOnlyThis, { buyMode: buyMode });
    }
}

export const scriptOptions = defineScriptOptions(Adv.optionsStorage, [], true, {
    Generic: Adv.mergeOptions,
    Select: YulgarsUndineMerge.select,
});

export default function main(): void {
        Core.bankingBlackList.push(...[ "Venomous Rose", "Undine Visitor Badge" ]);
        Core.setOptions();

        new YulgarsUndineMerge().buyAllMerge();
        Core.setOptions(false);
}
