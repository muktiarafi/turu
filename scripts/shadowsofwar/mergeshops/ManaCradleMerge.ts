/*
name: ManaCradleMerge
description: null
tags: null
*/
// Port of Scripts/ShadowsOfWar/MergeShops/ManaCradleMerge.cs.

import { bot, defineScriptOptions, option } from "turu";
import { CoreAdvanced, mergeOptionsEnum } from "../../core/CoreAdvanced";
import { Core } from "../../core/CoreBots";
import { CoreSoW } from "../../story/shadowsofwar/CoreSoW";
import { CoreSoWMats } from "../CoreSoWMats";

const Adv = new CoreAdvanced();
const SOWM = new CoreSoWMats();
const SoW = new CoreSoW();

export class ManaCradleMerge {
    static readonly select = [
        option.bool("76482", "Dragon's Tear", "Mode: [select] only\nShould the bot buy \"Dragon's Tear\" ?", false),
        option.bool("73781", "Shadowflame Vanguard", "Mode: [select] only\nShould the bot buy \"Shadowflame Vanguard\" ?", false),
        option.bool("73784", "Mana Overdrive Armet", "Mode: [select] only\nShould the bot buy \"Mana Overdrive Armet\" ?", false),
        option.bool("73785", "Mana Overdrive Armet Locks", "Mode: [select] only\nShould the bot buy \"Mana Overdrive Armet Locks\" ?", false),
        option.bool("73787", "Mana Overdrive Shroud", "Mode: [select] only\nShould the bot buy \"Mana Overdrive Shroud\" ?", false),
        option.bool("73789", "Mana Overdriven Devastation", "Mode: [select] only\nShould the bot buy \"Mana Overdriven Devastation\" ?", false),
        option.bool("73791", "Mana Overdriven Spear", "Mode: [select] only\nShould the bot buy \"Mana Overdriven Spear\" ?", false),
        option.bool("73792", "Mana Surge Gauntlet", "Mode: [select] only\nShould the bot buy \"Mana Surge Gauntlet\" ?", false),
        option.bool("73793", "Mana Surge Gauntlets", "Mode: [select] only\nShould the bot buy \"Mana Surge Gauntlets\" ?", false),
        option.bool("76566", "Mainyu Tail and Rune", "Mode: [select] only\nShould the bot buy \"Mainyu Tail and Rune\" ?", false),
        option.bool("76565", "Mainyu Wings and Rune", "Mode: [select] only\nShould the bot buy \"Mainyu Wings and Rune\" ?", false),
        option.bool("76564", "Mainyu Wings and Tail", "Mode: [select] only\nShould the bot buy \"Mainyu Wings and Tail\" ?", false),
        option.bool("76563", "Mainyu Aspects", "Mode: [select] only\nShould the bot buy \"Mainyu Aspects\" ?", false),
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
                    Core.addDrop("ShadowFlame Healer", "ShadowFlame Mage");
                    SOWM.willpower(quant);
                    break;

                case "Garish Remnant":
                    SOWM.garishRemnant(quant);
                    break;

                case "Prismatic Seams":
                    SOWM.prismaticSeams(quant);
                    break;

                case "Unbound Thread":
                    SOWM.unboundThread(quant);
                    break;

                case "Acquiescence":
                    SOWM.acquiescence(quant);
                    break;

                case "Elemental Core":
                    SOWM.elementalCore(quant);
                    break;

                case "Mainyu Rune":
                case "Mainyu Wings":
                case "Mainyu Tail":
                    Core.farmingLogger(req.name, quant);
                    Core.addDrop(...SoW.malgorDrops.concat(SoW.mainyuDrops));
                    Adv.gearStore(false, { enhAfter: true });
                    Core.useBossClass();
                    Core.huntMonster("manacradle", "The Mainyu", req.name, 1, { isTemp: false });
                    Adv.gearStore(true, { enhAfter: true });
                    break;
            }
        };

        SoW.completeCoreSoW();

        //Only edit the map and shopID here
        Adv.startBuyAllMerge("manacradle", 2242, findIngredients, buyOnlyThis, { buyMode: buyMode });
        Core.toBank(...SoW.malgorDrops.concat(SoW.mainyuDrops));
    }
}

export const scriptOptions = defineScriptOptions(Adv.optionsStorage, [], true, {
    Generic: Adv.mergeOptions,
    Select: ManaCradleMerge.select,
});

export default function main(): void {
        Core.bankingBlackList.push(...
            [
                "Willpower",
                "Garish Remnant",
                "Prismatic Seams",
                "Unbound Thread",
                "Acquiescence",
                "Elemental Core",
                "Mainyu Tail",
                "Mainyu Rune",
                "Mainyu Wings",
            ]);
        Core.setOptions();

        new ManaCradleMerge().buyAllMerge();
        Core.setOptions(false);
}
