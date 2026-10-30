/*
name: Bocklin Grove Merge
description: This bot will farm the items belonging to the selected mode for the Bocklin Grove Merge [2576] in /bocklingrove
tags: bocklin, grove, merge, bocklingrove, champion, lynaria, chiral, valley, knight, duke, noble, antlered, golden, anjou, regal, visor, dukes, vaughns, sash, aquitaine
*/
// Port of Scripts/Other/MergeShops/BocklinGroveMerge.cs.

import { bot, defineScriptOptions, option } from "turu";
import { CoreAdvanced, mergeOptionsEnum } from "../../core/CoreAdvanced";
import { ClassType, Core } from "../../core/CoreBots";
import { CoreFarms } from "../../core/CoreFarms";
import { CoreLynaria } from "../../story/lynaria/CoreLynaria";

const Farm = new CoreFarms();
const Adv = new CoreAdvanced();
const Lyn = new CoreLynaria();

export class BocklinGroveMerge {
    static readonly select = [
        option.bool("93590", "Champion Lynaria Armor", "Mode: [select] only\nShould the bot buy \"Champion Lynaria Armor\" ?", false),
        option.bool("93613", "Chiral Valley Knight", "Mode: [select] only\nShould the bot buy \"Chiral Valley Knight\" ?", false),
        option.bool("93614", "Chiral Valley Duke", "Mode: [select] only\nShould the bot buy \"Chiral Valley Duke\" ?", false),
        option.bool("93615", "Chiral Valley Noble", "Mode: [select] only\nShould the bot buy \"Chiral Valley Noble\" ?", false),
        option.bool("93616", "Antlered Golden Anjou Helm", "Mode: [select] only\nShould the bot buy \"Antlered Golden Anjou Helm\" ?", false),
        option.bool("93617", "Regal Golden Anjou Helm", "Mode: [select] only\nShould the bot buy \"Regal Golden Anjou Helm\" ?", false),
        option.bool("93619", "Antlered Golden Anjou Visor", "Mode: [select] only\nShould the bot buy \"Antlered Golden Anjou Visor\" ?", false),
        option.bool("93620", "Golden Anjou Visor", "Mode: [select] only\nShould the bot buy \"Golden Anjou Visor\" ?", false),
        option.bool("93621", "Anjou Duke's Helm", "Mode: [select] only\nShould the bot buy \"Anjou Duke's Helm\" ?", false),
        option.bool("93624", "Vaughn's Sash and Aquitaine", "Mode: [select] only\nShould the bot buy \"Vaughn's Sash and Aquitaine\" ?", false),
        option.bool("93625", "Aquitaine", "Mode: [select] only\nShould the bot buy \"Aquitaine\" ?", false),
        option.bool("93626", "Dual Aquitaine", "Mode: [select] only\nShould the bot buy \"Dual Aquitaine\" ?", false),
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

                case "Enthralling Gem Shard":
                    Core.farmingLogger(req.name, quant);
                   while (!bot.shouldExit && !Core.checkInventory(req.name, quant)) {
                        Core.ensureAccept(Core.isMember ? 10242 : 10240);
                        Core.equipClass(ClassType.Farm);
                        Core.huntMonster("bocklingrove", "Undead Garde", "Undead's Flag", 9);
                        Core.huntMonster("bocklingrove", "Garde Wraith", "Wraith's Scream", 6);
                        Core.equipClass(ClassType.Solo);
                        Core.huntMonster("bocklingrove", "Elder Necromancer", "Necromancer's Coin", 1);
                        Core.ensureComplete(Core.isMember ? 10242 : 10240);
                        bot.wait.forPickup(req.name);
                    }
                    Core.cancelRegisteredQuests();
                    break;

                case "Victorious' Golden Scale":
                    Core.farmingLogger(req.name, quant);
                    Core.equipClass(ClassType.Solo);
                    Core.huntMonster("darkplane", "Victorious", req.name, quant, { isTemp: req.temp, log: false });
                    break;

                case "Vaughn Crest":
                    Core.farmingLogger(req.name, quant);
                    Core.equipClass(ClassType.Farm);
                    Core.huntMonster("bocklingrove", "Garde Wraith", req.name, quant, { isTemp: req.temp, log: false });
                    break;

                case "Golden Anjou Helm":
                case "Vaughn's Crimson Sash":
                case "Sheathed Aquitaine":
                    Core.farmingLogger(req.name, quant);
                    Core.equipClass(ClassType.Solo);
                    Core.huntMonster("bocklingrove", "Elder Necromancer", req.name, quant, { isTemp: req.temp, log: false });
                    break;
            }
        };

        Lyn.bocklinGrove();
        //Only edit the map and shopID here
        Adv.startBuyAllMerge("bocklingrove", 2576, findIngredients, buyOnlyThis, { buyMode: buyMode });
    }
}

export const scriptOptions = defineScriptOptions(Adv.optionsStorage, [], true, {
    Generic: Adv.mergeOptions,
    Select: BocklinGroveMerge.select,
});

export default function main(): void {
        Core.bankingBlackList.push(...
            [
                "Enthralling Gem Shard",
                "Victorious' Golden Scale",
                "Vaughn Crest",
                "Golden Anjou Helm",
                "Vaughn's Crimson Sash",
                "Sheathed Aquitaine",
            ]);
        Core.setOptions();

        new BocklinGroveMerge().buyAllMerge();
        Core.setOptions(false);
}
