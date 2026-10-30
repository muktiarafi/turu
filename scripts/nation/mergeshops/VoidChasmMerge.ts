/*
name: Void Chasm Merge
description: This bot will farm the items belonging to the selected mode for the Void Chasm Merge [2410] in /voidchasm
tags: void, chasm, merge, voidchasm, abyssal, gravity, ball, balls, midnight, fiend, nulgath, blood, moon, winged, bloodmoon, darksided, energized, archfiend, spines, runed, draconic, nation, double, inferno, prime, shard
*/
// Port of Scripts/Nation/MergeShops/VoidChasmMerge.cs.

import { bot, defineScriptOptions, option } from "turu";
import { CoreAdvanced, mergeOptionsEnum } from "../../core/CoreAdvanced";
import { ClassType, Core } from "../../core/CoreBots";
import { CoreFarms } from "../../core/CoreFarms";
import { VoidChasm } from "../../story/nation/VoidChasm";
import { CoreNation } from "../CoreNation";

const Farm = new CoreFarms();
const Adv = new CoreAdvanced();
const Nation = new CoreNation();
const voidChasm = new VoidChasm();

export class VoidChasmMerge {
    static readonly select = [
        option.bool("83042", "Abyssal Gravity Ball", "Mode: [select] only\nShould the bot buy \"Abyssal Gravity Ball\" ?", false),
        option.bool("83043", "Abyssal Gravity Balls", "Mode: [select] only\nShould the bot buy \"Abyssal Gravity Balls\" ?", false),
        option.bool("83130", "Midnight Fiend of Nulgath", "Mode: [select] only\nShould the bot buy \"Midnight Fiend of Nulgath\" ?", false),
        option.bool("83131", "Midnight Fiend Hood", "Mode: [select] only\nShould the bot buy \"Midnight Fiend Hood\" ?", false),
        option.bool("83132", "Midnight Blood Moon", "Mode: [select] only\nShould the bot buy \"Midnight Blood Moon\" ?", false),
        option.bool("83133", "Winged Midnight Fiend Cape", "Mode: [select] only\nShould the bot buy \"Winged Midnight Fiend Cape\" ?", false),
        option.bool("83134", "Winged Bloodmoon Cape", "Mode: [select] only\nShould the bot buy \"Winged Bloodmoon Cape\" ?", false),
        option.bool("83135", "Darksided Fiend Blade", "Mode: [select] only\nShould the bot buy \"Darksided Fiend Blade\" ?", false),
        option.bool("83136", "Darksided Fiend Blades", "Mode: [select] only\nShould the bot buy \"Darksided Fiend Blades\" ?", false),
        option.bool("83137", "Midnight Fiend Sword", "Mode: [select] only\nShould the bot buy \"Midnight Fiend Sword\" ?", false),
        option.bool("83138", "Midnight Fiend Swords", "Mode: [select] only\nShould the bot buy \"Midnight Fiend Swords\" ?", false),
        option.bool("83228", "Energized Archfiend Spines", "Mode: [select] only\nShould the bot buy \"Energized Archfiend Spines\" ?", false),
        option.bool("83229", "Runed Archfiend Spines", "Mode: [select] only\nShould the bot buy \"Runed Archfiend Spines\" ?", false),
        option.bool("69634", "Draconic Nation Double Axe", "Mode: [select] only\nShould the bot buy \"Draconic Nation Double Axe\" ?", false),
        option.bool("69635", "Inferno Nation Double Axe", "Mode: [select] only\nShould the bot buy \"Inferno Nation Double Axe\" ?", false),
        option.bool("83524", "Draconic Nation Double Axes", "Mode: [select] only\nShould the bot buy \"Draconic Nation Double Axes\" ?", false),
        option.bool("83525", "Inferno Nation Double Axes", "Mode: [select] only\nShould the bot buy \"Inferno Nation Double Axes\" ?", false),
        option.bool("70184", "Prime Fiend Shard", "Mode: [select] only\nShould the bot buy \"Prime Fiend Shard\" ?", false),
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

                case "Void Remnant":
                    Core.farmingLogger(req.name, quant);
                    while (!bot.shouldExit && !Core.checkInventory(req.name, quant)) {
                        Core.addDrop("Void Remnant");
                        Core.ensureAccept(9553);
                        Core.equipClass(ClassType.Solo);
                        Core.killMonster("voidchasm", "r10", "Left", "Carcano", "Carcano's Teratoma");
                        Core.killMonster("voidchasm", "r9", "Left", "Carnage", "Bloodied Chainlink");
                        Core.equipClass(ClassType.Farm);
                        Core.killMonster("voidchasm", "r7", "Left", "The Hushed", "Defunct Seal of Approval", 6);
                        Core.ensureComplete(9553);
                        bot.wait.forPickup(req.name);
                    }
                    break;

                case "Tainted Gem":
                    Nation.farmTaintedGem(quant);
                    break;

                case "Essence of Nulgath":
                    Nation.essenceofNulgath(quant);
                    break;
            }
        };

        voidChasm.storyline();

        //Only edit the map and shopID here
        Adv.startBuyAllMerge("voidchasm", 2410, findIngredients, buyOnlyThis, { buyMode: buyMode });
    }
}

export const scriptOptions = defineScriptOptions(Adv.optionsStorage, [], true, {
    Generic: Adv.mergeOptions,
    Select: VoidChasmMerge.select,
});

export default function main(): void {
        Core.bankingBlackList.push(...
            [ "Void Remnant", "Tainted Gem", "Essence of Nulgath" ]);
        Core.setOptions();

        new VoidChasmMerge().buyAllMerge();
        Core.setOptions(false);
}
