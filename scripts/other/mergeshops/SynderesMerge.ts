/*
name: SynderesMerge
description: null
tags: null
*/
// Port of Scripts/Other/MergeShops/SynderesMerge.cs (includes CoreBots, CoreFarms, CoreStory,
// CoreAdvanced).

import { bot, defineScriptOptions, option } from "turu";
import { CoreAdvanced, mergeOptionsEnum } from "../../core/CoreAdvanced";
import { ClassType, Core } from "../../core/CoreBots";

const Adv = new CoreAdvanced();

export class SynderesMerge {
    // [Can Change] This should only be changed by the author.
    //              If true, it will not stop the script if the default case triggers and the user chose to only get mats
    private dontStopMissingIng = false;

    buyAllMerge(buyOnlyThis: string | null = null, buyMode: mergeOptionsEnum | null = null): void {
        //Only edit the map and shopID here
        Adv.startBuyAllMerge("enemyforest", 332, () => this.findIngredients(), buyOnlyThis, { buyMode });
    }

    //#region Dont edit this part
    private findIngredients(): void {
        const req = Adv.externalItem;
        const quant = Adv.externalQuant;

        switch (req.name) {
            default: {
                const shouldStop = !Adv.matsOnly || !this.dontStopMissingIng;
                Core.logger(`The bot hasn't been taught how to get ${req.name}.` + (shouldStop ? " Please report the issue." : " Skipping"), "", {
                    messageBox: shouldStop,
                    stopBot: shouldStop,
                });
                break;
            }
            //#endregion

            case "Synderes' Souvenir":
                Core.farmingLogger(req.name, quant);
                Core.equipClass(ClassType.Farm);
                Core.registerQuests(4247);
                while (!bot.shouldExit && !Core.checkInventory(req.name, quant)) {
                    //Synderes Souvenirs Shop 4247
                    Core.huntMonster("enemyforest", "Evil Elemental", "Forest Denizen Slain", 5);
                    bot.wait.forPickup(req.name);
                }
                Core.cancelRegisteredQuests();
                break;
        }
    }

    static readonly select = [
        option.bool("29574", "Synderes' Cape of Doom", 'Mode: [select] only\nShould the bot buy "Synderes\' Cape of Doom" ?', false),
        option.bool("29576", "NO BOTS Armor", 'Mode: [select] only\nShould the bot buy "NO BOTS Armor" ?', false),
        option.bool("29711", "Synderes Groupie Armor", 'Mode: [select] only\nShould the bot buy "Synderes Groupie Armor" ?', false),
        option.bool("29753", "Sock Monkey Cape", 'Mode: [select] only\nShould the bot buy "Sock Monkey Cape" ?', false),
        option.bool("29754", "Synderes Acoustic Guitar", 'Mode: [select] only\nShould the bot buy "Synderes Acoustic Guitar" ?', false),
        option.bool("29755", "Synderes Electric", 'Mode: [select] only\nShould the bot buy "Synderes Electric" ?', false),
        option.bool("29756", "Expendable Rogue Mask", 'Mode: [select] only\nShould the bot buy "Expendable Rogue Mask" ?', false),
        option.bool("29757", "Expendable Rogue", 'Mode: [select] only\nShould the bot buy "Expendable Rogue" ?', false),
        option.bool("29758", "Synderes Minion Mask and Locks", 'Mode: [select] only\nShould the bot buy "Synderes Minion Mask and Locks" ?', false),
        option.bool("29759", "Synderes Minion Mask", 'Mode: [select] only\nShould the bot buy "Synderes Minion Mask" ?', false),
    ];
}

export const scriptOptions = defineScriptOptions(Adv.optionsStorage, [], true, {
    Generic: Adv.mergeOptions,
    Select: SynderesMerge.select,
});

export default function main(): void {
    Core.bankingBlackList.push("Synderes' Souvenir ");
    Core.setOptions();

    new SynderesMerge().buyAllMerge();

    Core.setOptions(false);
}
