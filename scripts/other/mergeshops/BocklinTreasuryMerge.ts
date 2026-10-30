/*
name: Bocklin Treasury Merge
description: This bot will farm the items belonging to the selected mode for the Bocklin Treasury Merge [2578] in /bocklinsanctum
tags: bocklin, treasury, merge, bocklinsanctum, aldens, liberation, battle, morph, lynarias, hammer, castle, princess, brittanys, dress
*/
// Port of Scripts/Other/MergeShops/BocklinTreasuryMerge.cs.

import { bot, defineScriptOptions, option } from "turu";
import { CoreAdvanced, mergeOptionsEnum } from "../../core/CoreAdvanced";
import { ClassType, Core } from "../../core/CoreBots";
import { CoreFarms } from "../../core/CoreFarms";
import { CoreLynaria } from "../../story/lynaria/CoreLynaria";
import { CoreSepulchure } from "../../story/sepulchuresaga/CoreSepulchure";
import { BocklinArmoryMerge } from "./BocklinArmoryMerge";
import { BocklinGroveMerge } from "./BocklinGroveMerge";

const Farm = new CoreFarms();
const Adv = new CoreAdvanced();
const Lynaria = new CoreLynaria();
const BocklinGroveM = new BocklinGroveMerge();
const BocklinArmoryM = new BocklinArmoryMerge();
const Seppy = new CoreSepulchure();

export class BocklinTreasuryMerge {
    static readonly select = [
        option.bool("93592", "Alden's Liberation Armor", "Mode: [select] only\nShould the bot buy \"Alden's Liberation Armor\" ?", false),
        option.bool("93825", "Alden's Battle Morph", "Mode: [select] only\nShould the bot buy \"Alden's Battle Morph\" ?", false),
        option.bool("93824", "Alden's Battle Helm", "Mode: [select] only\nShould the bot buy \"Alden's Battle Helm\" ?", false),
        option.bool("93767", "Lynaria's Hammer", "Mode: [select] only\nShould the bot buy \"Lynaria's Hammer\" ?", false),
        option.bool("93766", "Lynaria's Locks", "Mode: [select] only\nShould the bot buy \"Lynaria's Locks\" ?", false),
        option.bool("93765", "Lynaria's Visage", "Mode: [select] only\nShould the bot buy \"Lynaria's Visage\" ?", false),
        option.bool("93764", "Castle Bocklin", "Mode: [select] only\nShould the bot buy \"Castle Bocklin\" ?", false),
        option.bool("93638", "Princess Brittany's Dress", "Mode: [select] only\nShould the bot buy \"Princess Brittany's Dress\" ?", false),
        option.bool("93639", "Princess Brittany's Visage", "Mode: [select] only\nShould the bot buy \"Princess Brittany's Visage\" ?", false),
        option.bool("93640", "Princess Brittany's Hair", "Mode: [select] only\nShould the bot buy \"Princess Brittany's Hair\" ?", false),
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

                case "Thronekeeper's Rune":
                    Core.farmingLogger("Thronekeeper's Rune", quant);
                    Core.equipClass(ClassType.Solo);
                    Core.addDrop(92791);
                    // Members get x2, non-members get x1 drops
                    Core.registerQuests(Core.isMember ? 10267 : 10266);
                    while (!bot.shouldExit && !Core.checkInventory(req.id, req.quantity)) {
                        Core.huntMonster("bocklinsanctum", "Thronekeeper", "Black Armorial Fleur", 1, { log: false });
                        bot.wait.forPickup(req.name);
                    }
                    Core.cancelRegisteredQuests();
                    break;

                case "Champion Lynaria Armor":
                    BocklinGroveM.buyAllMerge("Champion Lynaria Armor");
                    bot.wait.forPickup(req.name);
                    break;

                case "Valen's Knightly Armor":
                    BocklinArmoryM.buyAllMerge("Valen's Knightly Armor");
                    bot.wait.forPickup(req.name);
                    break;

                case "King Alteon's Armor Fragment":
                    Core.equipClass(ClassType.Solo);
                    Core.addDrop(93763);
                    Core.huntMonster("alteonfight", "King Alteon", req.name, req.quantity, { isTemp: req.temp });
                    bot.wait.forPickup(req.name);
                    break;

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

                case "Bocklin Ornament":
                    Core.farmingLogger(req.name, quant);
                    Core.equipClass(ClassType.Farm);
                    Core.addDrop(req.id);
                    while (!bot.shouldExit && !Core.checkInventory(req.id, quant)) {
                        Core.ensureAccept(Core.isMember ? 10255 : 10253);
                        Core.equipClass(ClassType.Farm);
                        Core.huntMonster("bocklincastle", "Warped Revenant", "Twisted Arms", 50);
                        Core.huntMonster("bocklincastle", "Faceless Ritualist", "Wreath of Aster", 25);
                        Core.equipClass(ClassType.Solo);
                        Core.huntMonster("bocklincastle", "Headless Knight", "Golden Anjou");
                        Core.ensureComplete(Core.isMember ? 10255 : 10253);
                        bot.wait.forPickup(req.name);
                    }
                    Core.cancelRegisteredQuests();
                    break;

                case "Scion's Regalia":
                    Core.equipClass(ClassType.Farm);
                    Core.addDrop(93762);
                    Core.huntMonster("bocklinsanctum", "Tarnished Scion", req.name, req.quantity, { isTemp: req.temp }); bot.wait.forPickup(req.name);
                    break;

                case "Vaughn Crest":
                    Core.farmingLogger(req.name, quant);
                    Core.equipClass(ClassType.Farm);
                    Core.addDrop(req.id);
                    Core.huntMonster("bocklincastle", "Garde Wraith", req.name, quant, { isTemp: req.temp, log: false }); break;

                case "Gold Voucher 100k":
                    Farm.voucher(req.name, req.quantity);
                    break;
            }
        };

        Seppy.shadowfallRise();
        //very last quest in core is require for alteon's fragments to drop
        Lynaria.doAll();

        //Only edit the map and shopID here
        Adv.startBuyAllMerge(
            "bocklinsanctum", 
            2578, 
            findIngredients, 
            buyOnlyThis, { buyMode: buyMode });
    }
}

export const scriptOptions = defineScriptOptions(Adv.optionsStorage, [], true, {
    Generic: Adv.mergeOptions,
    Select: BocklinTreasuryMerge.select,
});

export default function main(): void {
        Core.bankingBlackList.push(...
            [
                "Thronekeeper's Rune",
                "Champion Lynaria Armor",
                "Valen's Knightly Armor",
                "King Alteon's Armor Fragment",
                "Enthralling Gem Shard",
                "Bocklin Ornament",
                "Scion's Regalia",
                "Vaughn Crest",
            ]);
        Core.setOptions();

        new BocklinTreasuryMerge().buyAllMerge();
        Core.setOptions(false);
}
