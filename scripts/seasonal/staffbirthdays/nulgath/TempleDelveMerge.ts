/*
name: Temple Delve Merge
description: This will get all or selected items on this merge shop.
tags: temple. delve, merge, seasonal, nulgath, staff, birthday
*/
// Port of Scripts/Seasonal/StaffBirthdays/Nulgath/TempleDelveMerge.cs.

import { bot, defineScriptOptions, option } from "turu";
import { CoreAdvanced, mergeOptionsEnum } from "../../../core/CoreAdvanced";
import { ClassType, Core } from "../../../core/CoreBots";
import { CoreFarms } from "../../../core/CoreFarms";
import { CoreNation } from "../../../nation/CoreNation";
import { TempleDelve } from "./TempleDelve";

const Farm = new CoreFarms();
const Adv = new CoreAdvanced();
const Nation = new CoreNation();
const TD = new TempleDelve();

export class TempleDelveMerge {
    static readonly select = [
        option.bool("75493", "Nation Ritualist Visage", "Mode: [select] only\nShould the bot buy \"Nation Ritualist Visage\" ?", false),
        option.bool("75495", "Nation Ritualist Morph + Locks", "Mode: [select] only\nShould the bot buy \"Nation Ritualist Morph + Locks\" ?", false),
        option.bool("75491", "Horned Void Nation Ritualist Visage", "Mode: [select] only\nShould the bot buy \"Horned Void Nation Ritualist Visage\" ?", false),
        option.bool("75498", "Nation Ritualist Void Gate", "Mode: [select] only\nShould the bot buy \"Nation Ritualist Void Gate\" ?", false),
        option.bool("75497", "Nation Ritualist Gateway", "Mode: [select] only\nShould the bot buy \"Nation Ritualist Gateway\" ?", false),
        option.bool("75499", "Nation Ritualist Orb Pet", "Mode: [select] only\nShould the bot buy \"Nation Ritualist Orb Pet\" ?", false),
        option.bool("75500", "Nation Ritualist Rune", "Mode: [select] only\nShould the bot buy \"Nation Ritualist Rune\" ?", false),
        option.bool("75501", "Nation Ritualist's Sigil", "Mode: [select] only\nShould the bot buy \"Nation Ritualist's Sigil\" ?", false),
        option.bool("75503", "Nation Ritualist's Dual Swirling Sigils", "Mode: [select] only\nShould the bot buy \"Nation Ritualist's Dual Swirling Sigils\" ?", false),
        option.bool("75505", "Void Nation Ritualist Spear", "Mode: [select] only\nShould the bot buy \"Void Nation Ritualist Spear\" ?", false),
        option.bool("75487", "Void Nation Caster", "Mode: [select] only\nShould the bot buy \"Void Nation Caster\" ?", false),
        option.bool("75489", "Nation Caster", "Mode: [select] only\nShould the bot buy \"Nation Caster\" ?", false),
        option.bool("75681", "Scholar of Nulgath", "Mode: [select] only\nShould the bot buy \"Scholar of Nulgath\" ?", false),
        option.bool("75682", "Scholar of Nulgath Hair", "Mode: [select] only\nShould the bot buy \"Scholar of Nulgath Hair\" ?", false),
        option.bool("75683", "Scholar of Nulgath Ponytail", "Mode: [select] only\nShould the bot buy \"Scholar of Nulgath Ponytail\" ?", false),
        option.bool("75684", "Scholar of Nulgath Scarf", "Mode: [select] only\nShould the bot buy \"Scholar of Nulgath Scarf\" ?", false),
        option.bool("75685", "Prof Polish", "Mode: [select] only\nShould the bot buy \"Prof Polish\" ?", false),
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

                case "Doomed Extract":
                    Core.farmingLogger(req.name, quant);
                    Core.registerQuests(9090);
                    while (!bot.shouldExit && !Core.checkInventory(req.name, quant)) {
                        Core.equipClass(ClassType.Farm);
                        Core.huntMonster("templedelve", "Delirious Elemental", "Elemental Study", 6);
                        Core.huntMonster("templedelve", "Infested Nation", "Infestation Study", 6);
                        Core.equipClass(ClassType.Solo);
                        Core.huntMonster("templedelve", "Doomed Fiend", "Fiend Worm");
                        bot.wait.forPickup(req.name);
                    }
                    Core.cancelRegisteredQuests();
                    break;

                case "Tainted Gem":
                    Nation.farmTaintedGem(quant);
                    break;

                case "Dark Crystal Shard":
                    Nation.farmDarkCrystalShard(quant);
                    break;

                case "Diamond of Nulgath":
                    Nation.farmDiamondofNulgath(quant);
                    break;

                case "Nation Ritualist":
                case "Void Nation Ritualist":
                    Core.equipClass(ClassType.Solo);
                    Core.huntMonster("templedelve", "Doomed Fiend", req.name, 1, { isTemp: false });
                    break;
            }
        };

        if (!Core.isSeasonalMapActive("templedelve"))
            return;

        TD.storyline();
        //Only edit the map and shopID here
        Adv.startBuyAllMerge("templedelve", 2232, findIngredients, buyOnlyThis, { buyMode: buyMode });
    }
}

export const scriptOptions = defineScriptOptions(Adv.optionsStorage, [], true, {
    Generic: Adv.mergeOptions,
    Select: TempleDelveMerge.select,
});

export default function main(): void {
        Core.bankingBlackList.push(...
            [
                "Doomed Extract",
                "Tainted Gem",
                "Dark Crystal Shard",
                "Diamond of Nulgath",
                "Nation Ritualist",
                "Void Nation Ritualist",
            ]);
        Core.setOptions();

        new TempleDelveMerge().buyAllMerge();
        Core.setOptions(false);
}
