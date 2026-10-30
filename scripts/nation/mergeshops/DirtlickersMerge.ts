/*
name: Dirtlicker's Merge Shop
description: This script farms all the materials needed for Dirtlicker's Merge Shop in archportal.
tags: dirtlicker, archportal, merge, nulgath, ennh, enchanted nulgath nation house
*/
// Port of Scripts/Nation/MergeShops/DirtlickersMerge.cs.

import { bot, defineScriptOptions, option } from "turu";
import { CoreAdvanced, mergeOptionsEnum } from "../../core/CoreAdvanced";
import { ClassType, Core } from "../../core/CoreBots";
import { CoreDailies } from "../../core/CoreDailies";
import { CoreFarms } from "../../core/CoreFarms";
import { CoreBLOD } from "../../good/blod/CoreBLOD";
import { CoreNation } from "../CoreNation";
import { JuggernautItemsofNulgath, RewardsSelection } from "../various/JuggernautItems";
import { RewardsSelection as LeeryRewardsSelection, TheLeeryContract } from "../various/TheLeeryContract[Member]";

const Farm = new CoreFarms();
const Daily = new CoreDailies();
const Adv = new CoreAdvanced();
const Nation = new CoreNation();
const BLOD = new CoreBLOD();
const TLC = new TheLeeryContract();
const Jugger = new JuggernautItemsofNulgath();

export class DirtlickersMerge {
    static readonly select = [
        option.bool("33283", "Enchanted Nulgath Nation House", "Mode: [select] only\nShould the bot buy \"Enchanted Nulgath Nation House\" ?", false),
        option.bool("33196", "Evolved Blood Orb", "Mode: [select] only\nShould the bot buy \"Evolved Blood Orb\" ?", false),
        option.bool("33197", "Evolved Hex Orb", "Mode: [select] only\nShould the bot buy \"Evolved Hex Orb\" ?", false),
        option.bool("33198", "Evolved Shadow Orb", "Mode: [select] only\nShould the bot buy \"Evolved Shadow Orb\" ?", false),
        option.bool("33155", "Evolved Soulreaper Blade of Nulgath", "Mode: [select] only\nShould the bot buy \"Evolved Soulreaper Blade of Nulgath\" ?", false),
        option.bool("33165", "Raw Dreadsaw", "Mode: [select] only\nShould the bot buy \"Raw Dreadsaw\" ?", false),
        option.bool("33164", "Iron Dreadsaw", "Mode: [select] only\nShould the bot buy \"Iron Dreadsaw\" ?", false),
        option.bool("33270", "Shadow Legacy of Nulgath", "Mode: [select] only\nShould the bot buy \"Shadow Legacy of Nulgath\" ?", false),
        option.bool("33271", "Shadow Legacy of Nulgath Helm", "Mode: [select] only\nShould the bot buy \"Shadow Legacy of Nulgath Helm\" ?", false),
        option.bool("43230", "Shadow Legacy of Nulgath Hair", "Mode: [select] only\nShould the bot buy \"Shadow Legacy of Nulgath Hair\" ?", false),
        option.bool("58542", "Yulgath's Inn", "Mode: [select] only\nShould the bot buy \"Yulgath's Inn\" ?", false),
    ];

    private readonly dontStopMissingIng = false;

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

                case "Nulgath Nation House":
                    Core.farmingLogger(req.name, quant);
                    Core.addDrop(req.name, "Cemaros' Amethyst", "Aluminium", "NUE Necronomicon");
                    Nation.farmUni10(400);
                    Nation.farmUni13(1);
                    Nation.farmVoucher({ member:false });
                    Nation.farmDiamondofNulgath(300);
                    Nation.farmDarkCrystalShard(250);
                    Nation.farmTotemofNulgath(30);
                    Nation.farmGemofNulgath(150);
                    Nation.farmTaintedGem(200);
                    Nation.farmBloodGem(100);
                    Nation.approvalAndFavor(1000, 0);

                    Farm.chaosREP(2);
                    Core.buyItem("mountdoomskull", 776, "Cemaros' Amethyst");

                    BLOD.unlockMineCrafting();
                    Daily.mineCrafting([ "Aluminum" ]);

                    Farm.doomWoodREP();
                    Farm.gold(999);
                    Core.buyItem("lightguard", 277, "NUE Necronomicon");

                    Core.ensureAccept(4779);
                    if (!Core.ensureComplete(4779)) {
                        Core.logger("Could not complete the quest, stopping bot", "", { messageBox: true });
                        return;
                    }
                    bot.wait.forPickup(req.name);
                    break;

                case "Pink Star Diamond of Nulgath":
                    Adv.buyItem("tercessuinotlim", 1951, "Pink Star Diamond of Nulgath");
                    bot.wait.forPickup(req.name);
                    break;

                case "Musgravite of Nulgath":
                    Core.equipClass(ClassType.Farm);
                    Core.huntMonster("timelibrary", "Ancient Chest", req.name, quant, { isTemp: false });
                    bot.wait.forPickup(req.name);
                    break;

                case "Nulgath's Approval":
                    Nation.approvalAndFavor(quant, 0);
                    bot.wait.forPickup(req.name);
                    break;

                case "Diamond of Nulgath":
                    Nation.farmDiamondofNulgath(300);
                    bot.wait.forPickup(req.name);
                    break;

                case "Totem of Nulgath":
                    Nation.farmTotemofNulgath(quant);
                    bot.wait.forPickup(req.name);
                    break;

                case "Voucher of Nulgath (non-mem)":
                    Nation.farmVoucher({ member:false, keepVoucher: true });
                    bot.wait.forPickup(req.name);
                    break;

                case "Voucher of Nulgath":
                    Nation.farmVoucher({ member:true, keepVoucher: true });
                    bot.wait.forPickup(req.name);
                    break;

                case "Unidentified 13":
                    Nation.farmUni13(quant);
                    bot.wait.forPickup(req.name);
                    bot.wait.forPickup(req.name);
                    break;

                case "Dark Crystal Shard":
                    Nation.farmDarkCrystalShard(quant);
                    bot.wait.forPickup(req.name);
                    break;

                case "Tainted Gem":
                    if (Core.checkInventory("Gemstone of Nulgath") && Core.isMember)
                        Nation.forgeTaintedGems(quant);
                    else
                        Nation.farmTaintedGem(quant);
                    bot.wait.forPickup(req.name);
                    break;

                case "Gem of Nulgath":
                    Nation.farmGemofNulgath(quant);
                    bot.wait.forPickup(req.name);
                    break;

                case "Corpse Maker of Nulgath":
                    Core.farmingLogger(req.name, quant);
                    TLC.questItems(LeeryRewardsSelection.Corpse_Maker_of_Nulgath);
                    bot.wait.forPickup(req.name);
                    break;

                case "Overfiend Blade of Nulgath":
                    Core.farmingLogger(req.name, quant);
                    Jugger.juggItems(RewardsSelection.Overfiend_Blade_of_Nulgath);
                    bot.wait.forPickup(req.name);
                    break;

                case "Archfiend's Favor":
                    Nation.approvalAndFavor(0, quant);
                    bot.wait.forPickup(req.name);
                    break;

                case "Hadean Onyx of Nulgath":
                    Core.huntMonster("tercessuinotlim", "Shadow of Nulgath", req.name, quant, { isTemp: false });
                    bot.wait.forPickup(req.name);
                    break;

                case "Essence of Nulgath":
                    Nation.essenceofNulgath(quant);
                    bot.wait.forPickup(req.name);
                    break;

                case "Letter from Asuka and Tendou":
                    Core.huntMonster("citadel", "Burning Witch", req.name, quant, { isTemp: false });
                    bot.wait.forPickup(req.name);
                    break;

                case "Chain of Nulgath":
                    Core.huntMonster("necrocavern", "Shadow Dragon", req.name, quant, { isTemp: false });
                    bot.wait.forPickup(req.name);
                    break;

                case "Yulgath's Hut":
                    Core.equipClass(ClassType.Solo);
                    Core.huntMonster("originul", "Fiend Champion", req.name, quant, { isTemp: false });
                    bot.wait.forPickup(req.name);
                    break;

                case "Unidentified 10":
                    Nation.farmUni10(quant);
                    bot.wait.forPickup(req.name);
                    break;
            }
        };

        //Only edit the map and shopID here
        Adv.startBuyAllMerge("archportal", 1211, findIngredients, buyOnlyThis, { buyMode: buyMode });
    }
}

export const scriptOptions = defineScriptOptions(Adv.optionsStorage, [], true, {
    Generic: Adv.mergeOptions,
    Select: DirtlickersMerge.select,
});

export default function main(): void {
        Core.bankingBlackList.push(...
            [
                "Nulgath Nation House",
                "Pink Star Diamond of Nulgath",
                "Musgravite of Nulgath",
                "Nulgath's Approval",
                "Totem of Nulgath",
                "Voucher of Nulgath (non-mem)",
                "Voucher of Nulgath",
                "Unidentified 13",
                "Dark Crystal Shard",
                "Tainted Gem",
                "Gem of Nulgath",
                "Corpse Maker of Nulgath",
                "Overfiend Blade of Nulgath",
                "Archfiend's Favor",
                "Hadean Onyx of Nulgath",
                "Essence of Nulgath",
                "Letter from Asuka and Tendou",
                "Chain of Nulgath",
                "Yulgath's Hut",
                "Unidentified 10",
            ]);
        Core.setOptions();

        new DirtlickersMerge().buyAllMerge();
        Core.setOptions(false);
}
