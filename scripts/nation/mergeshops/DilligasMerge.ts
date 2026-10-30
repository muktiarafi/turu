/*
name: DilligasMerge
description: null
tags: null
*/
// Port of Scripts/Nation/MergeShops/DilligasMerge.cs.

import { bot, defineScriptOptions, option } from "turu";
import { CoreAdvanced, mergeOptionsEnum } from "../../core/CoreAdvanced";
import { Core } from "../../core/CoreBots";
import { CoreNation } from "../CoreNation";

const Adv = new CoreAdvanced();
const Nation = new CoreNation();

export class DilligasMerge {
    static readonly select = [
        option.bool("29580", "Skullborne Sword", "Mode: [select] only\nShould the bot buy \"Skullborne Sword\" ?", false),
        option.bool("69959", "Crypt Soldier", "Mode: [select] only\nShould the bot buy \"Crypt Soldier\" ?", false),
        option.bool("69961", "Crypt Soldier Sword and Shield", "Mode: [select] only\nShould the bot buy \"Crypt Soldier Sword and Shield\" ?", false),
        option.bool("69960", "Crypt Soldier Helmet", "Mode: [select] only\nShould the bot buy \"Crypt Soldier Helmet\" ?", false),
        option.bool("69955", "Crypt Rogue", "Mode: [select] only\nShould the bot buy \"Crypt Rogue\" ?", false),
        option.bool("69957", "Crypt Rogue BackBlade", "Mode: [select] only\nShould the bot buy \"Crypt Rogue BackBlade\" ?", false),
        option.bool("69958", "Crypt Rogue BackBlades", "Mode: [select] only\nShould the bot buy \"Crypt Rogue BackBlades\" ?", false),
        option.bool("69956", "Crypt Rogue Helmet", "Mode: [select] only\nShould the bot buy \"Crypt Rogue Helmet\" ?", false),
        option.bool("69951", "Crypt Knight", "Mode: [select] only\nShould the bot buy \"Crypt Knight\" ?", false),
        option.bool("69953", "Crypt Knight Sword", "Mode: [select] only\nShould the bot buy \"Crypt Knight Sword\" ?", false),
        option.bool("69954", "Crypt Knight Swords", "Mode: [select] only\nShould the bot buy \"Crypt Knight Swords\" ?", false),
        option.bool("69952", "Crypt Knight Helm", "Mode: [select] only\nShould the bot buy \"Crypt Knight Helm\" ?", false),
        option.bool("69947", "Crypt Samurai", "Mode: [select] only\nShould the bot buy \"Crypt Samurai\" ?", false),
        option.bool("69950", "Crypt Samurai Blades", "Mode: [select] only\nShould the bot buy \"Crypt Samurai Blades\" ?", false),
        option.bool("69949", "Crypt Samurai Blade", "Mode: [select] only\nShould the bot buy \"Crypt Samurai Blade\" ?", false),
        option.bool("69948", "Crypt Samurai Helmet", "Mode: [select] only\nShould the bot buy \"Crypt Samurai Helmet\" ?", false),
        option.bool("69940", "Ancient Shogun Armor", "Mode: [select] only\nShould the bot buy \"Ancient Shogun Armor\" ?", false),
        option.bool("69946", "Ancient Shogun Katana And Sheath", "Mode: [select] only\nShould the bot buy \"Ancient Shogun Katana And Sheath\" ?", false),
        option.bool("69944", "Ancient Shogun Katanas", "Mode: [select] only\nShould the bot buy \"Ancient Shogun Katanas\" ?", false),
        option.bool("69943", "Ancient Shogun Katana", "Mode: [select] only\nShould the bot buy \"Ancient Shogun Katana\" ?", false),
        option.bool("69945", "Ancient Shogun Sheathed Katana", "Mode: [select] only\nShould the bot buy \"Ancient Shogun Sheathed Katana\" ?", false),
        option.bool("69941", "Ancient Shogun Helm", "Mode: [select] only\nShould the bot buy \"Ancient Shogun Helm\" ?", false),
        option.bool("69942", "Ancient Shogun Back Arms", "Mode: [select] only\nShould the bot buy \"Ancient Shogun Back Arms\" ?", false),
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

                case "Diamond of Nulgath":
                    Nation.farmDiamondofNulgath(quant);
                    break;

                case "Archfiend's Favor":
                    Nation.approvalAndFavor(0, quant);
                    break;

                case "Tainted Gem":
                    Nation.farmTaintedGem(quant);
                    break;

                case "Dark Crystal Shard":
                    Nation.farmDarkCrystalShard(quant);
                    break;

                case "Totem of Nulgath":
                    Nation.farmTotemofNulgath(quant);
                    break;

                case "Gem of Nulgath":
                    Nation.farmGemofNulgath(quant);
                    break;

                case "Blood Gem of the Archfiend":
                    Nation.farmBloodGem(quant);
                    break;
            }
        };

        //Only edit the map and shopID here
        Adv.startBuyAllMerge("evilwarnul", 465, findIngredients, buyOnlyThis, { buyMode: buyMode });
    }
}

export const scriptOptions = defineScriptOptions(Adv.optionsStorage, [], true, {
    Generic: Adv.mergeOptions,
    Select: DilligasMerge.select,
});

export default function main(): void {
        Core.bankingBlackList.push(...
            [
                "Diamond of Nulgath",
                "Archfiend's Favor",
                "Tainted Gem",
                "Dark Crystal Shard",
                "Totem of Nulgath",
                "Gem of Nulgath",
                "Blood Gem of the Archfiend",
            ]);
        Core.setOptions();

        new DilligasMerge().buyAllMerge();
        Core.setOptions(false);
}
