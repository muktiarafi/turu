/*
name: Bocklin Armory Merge
description: This bot will farm the items belonging to the selected mode for the Bocklin Armory Merge [2577] in /bocklincastle
tags: bocklin, armory, merge, bocklincastle, valens, knightly, golden, anjou, knight, duke, noble, enchanted, antlered, regal, dukes, vaughns, sash, aquitaine, gem, earthvessel, harpoon, rapier, rapiers, kris
*/
// Port of Scripts/Other/MergeShops/BocklinArmoryMerge.cs.

import { bot, defineScriptOptions, option } from "turu";
import { CoreAdvanced, mergeOptionsEnum } from "../../core/CoreAdvanced";
import { ClassType, Core } from "../../core/CoreBots";
import { CoreFarms } from "../../core/CoreFarms";
import { CoreLynaria } from "../../story/lynaria/CoreLynaria";
import { BocklinGroveMerge } from "./BocklinGroveMerge";

const Farm = new CoreFarms();
const Adv = new CoreAdvanced();
const Lyn = new CoreLynaria();
const BGM = new BocklinGroveMerge();

export class BocklinArmoryMerge {
    static readonly select = [
        option.bool("93591", "Valen's Knightly Armor", "Mode: [select] only\nShould the bot buy \"Valen's Knightly Armor\" ?", false),
        option.bool("93628", "Golden Anjou Knight", "Mode: [select] only\nShould the bot buy \"Golden Anjou Knight\" ?", false),
        option.bool("93629", "Golden Anjou Duke", "Mode: [select] only\nShould the bot buy \"Golden Anjou Duke\" ?", false),
        option.bool("93630", "Golden Anjou Noble", "Mode: [select] only\nShould the bot buy \"Golden Anjou Noble\" ?", false),
        option.bool("93631", "Enchanted Antlered Anjou Helm", "Mode: [select] only\nShould the bot buy \"Enchanted Antlered Anjou Helm\" ?", false),
        option.bool("93632", "Enchanted Regal Anjou Helm", "Mode: [select] only\nShould the bot buy \"Enchanted Regal Anjou Helm\" ?", false),
        option.bool("93633", "Enchanted Anjou Helm", "Mode: [select] only\nShould the bot buy \"Enchanted Anjou Helm\" ?", false),
        option.bool("93634", "Enchanted Anjou Duke's Helm", "Mode: [select] only\nShould the bot buy \"Enchanted Anjou Duke's Helm\" ?", false),
        option.bool("93635", "Vaughn's Enchanted Sash", "Mode: [select] only\nShould the bot buy \"Vaughn's Enchanted Sash\" ?", false),
        option.bool("93636", "Enchanted Sash and Aquitaine", "Mode: [select] only\nShould the bot buy \"Enchanted Sash and Aquitaine\" ?", false),
        option.bool("93637", "Enchanted Gem of Anjou", "Mode: [select] only\nShould the bot buy \"Enchanted Gem of Anjou\" ?", false),
        option.bool("93663", "Enchanted Aquitaine", "Mode: [select] only\nShould the bot buy \"Enchanted Aquitaine\" ?", false),
        option.bool("93664", "Enchanted Dual Aquitaine", "Mode: [select] only\nShould the bot buy \"Enchanted Dual Aquitaine\" ?", false),
        option.bool("93697", "EarthVessel Harpoon", "Mode: [select] only\nShould the bot buy \"EarthVessel Harpoon\" ?", false),
        option.bool("93698", "EarthVessel Blade", "Mode: [select] only\nShould the bot buy \"EarthVessel Blade\" ?", false),
        option.bool("93699", "EarthVessel Blades", "Mode: [select] only\nShould the bot buy \"EarthVessel Blades\" ?", false),
        option.bool("93700", "EarthVessel Dagger", "Mode: [select] only\nShould the bot buy \"EarthVessel Dagger\" ?", false),
        option.bool("93701", "EarthVessel Daggers", "Mode: [select] only\nShould the bot buy \"EarthVessel Daggers\" ?", false),
        option.bool("93702", "EarthVessel Rapier", "Mode: [select] only\nShould the bot buy \"EarthVessel Rapier\" ?", false),
        option.bool("93703", "EarthVessel Rapiers", "Mode: [select] only\nShould the bot buy \"EarthVessel Rapiers\" ?", false),
        option.bool("93704", "EarthVessel Kris", "Mode: [select] only\nShould the bot buy \"EarthVessel Kris\" ?", false),
        option.bool("93705", "Dual EarthVessel Kris", "Mode: [select] only\nShould the bot buy \"Dual EarthVessel Kris\" ?", false),
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

                case "Bocklin Ornament":
                    Core.farmingLogger(req.name, quant);
                    Core.equipClass(ClassType.Farm);
                    Core.addDrop(req.id);
                    while (!bot.shouldExit && !Core.checkInventory(req.id, quant)) {
                        Core.huntMonsterQuest(
                            Core.isMember ? 10255 : 10253, 
                            ["bocklincastle", "Faceless Ritualist", ClassType.Farm],
                            ["bocklincastle", "Headless Knight", ClassType.Solo],
                            ["bocklincastle", "Warped Revenant", ClassType.Farm]);
                        bot.wait.forPickup(req.name);
                    }
                    Core.cancelRegisteredQuests();
                    break;

                case "Monster Blood":
                    Core.farmingLogger(req.name, quant);
                    Core.equipClass(ClassType.Solo);
                    Core.addDrop(req.id);
                    Core.huntMonster("ebondungeon", "Dethrix", req.name, quant, { isTemp: req.temp, log: false });
                    break;

                case "Vaughn Crest":
                    Core.farmingLogger(req.name, quant);
                    Core.equipClass(ClassType.Farm);
                    Core.addDrop(req.id);
                    Core.huntMonster("bocklincastle", "Garde Wraith", req.name, quant, { isTemp: req.temp, log: false });
                    break;

                case "Chiral Valley Knight":
                case "Chiral Valley Duke":
                case "Chiral Valley Noble":
                case "Antlered Golden Anjou Helm":
                case "Regal Golden Anjou Helm":
                case "Anjou Duke's Helm":
                case "Vaughn's Sash and Aquitaine":
                case "Aquitaine":
                case "Dual Aquitaine":
                    BGM.buyAllMerge(req.name);
                    break;

                case "Golden Anjou Helm":
                case "Vaughn's Crimson Sash":
                case "Gem of Anjou":
                    Core.farmingLogger(req.name, quant);
                    Core.equipClass(ClassType.Solo);
                    Core.huntMonster("bocklingrove", "Elder Necromancer", req.name, quant, { isTemp: req.temp, log: false });
                    break;

                case "Forbidden EarthVessel Harpoon":
                case "Forbidden EarthVessel Sword":
                case "Forbidden EarthVessel Swords":
                case "Forbidden EarthVessel Dagger":
                case "Forbidden EarthVessel Daggers":
                case "Forbidden EarthVessel Rapier":
                case "Forbidden EarthVessel Rapiers":
                case "Forbidden EarthVessel Kris":
                case "Forbidden Dual EarthVessel Kris":
                    Core.farmingLogger(req.name, quant);
                    Core.equipClass(ClassType.Solo);
                    Core.addDrop(req.id);
                    Core.huntMonster("bocklincastle", "Headless Knight", req.name, quant, { isTemp: req.temp, log: false });
                    break;
            }
        };

        Lyn.bocklinCastle();
        //Only edit the map and shopID here
        Adv.startBuyAllMerge("bocklincastle", 2577, findIngredients, buyOnlyThis, { buyMode: buyMode });
    }
}

export const scriptOptions = defineScriptOptions(Adv.optionsStorage, [], true, {
    Generic: Adv.mergeOptions,
    Select: BocklinArmoryMerge.select,
});

export default function main(): void {
        Core.bankingBlackList.push(...
            [
                "Bocklin Ornament",
                "Monster Blood",
                "Vaughn Crest",
                "Chiral Valley Knight",
                "Chiral Valley Duke",
                "Chiral Valley Noble",
                "Antlered Golden Anjou Helm",
                "Regal Golden Anjou Helm",
                "Golden Anjou Helm",
                "Anjou Duke's Helm",
                "Vaughn's Crimson Sash",
                "Vaughn's Sash and Aquitaine",
                "Gem of Anjou",
                "Aquitaine",
                "Dual Aquitaine",
                "Forbidden EarthVessel Harpoon",
                "Forbidden EarthVessel Sword",
                "Forbidden EarthVessel Swords",
                "Forbidden EarthVessel Dagger",
                "Forbidden EarthVessel Daggers",
                "Forbidden EarthVessel Rapier",
                "Forbidden EarthVessel Rapiers",
                "Forbidden EarthVessel Kris",
                "Forbidden Dual EarthVessel Kris",
            ]);
        Core.setOptions();

        new BocklinArmoryMerge().buyAllMerge();
        Core.setOptions(false);
}
