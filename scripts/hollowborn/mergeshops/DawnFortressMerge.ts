/*
name: Dawn Fortress Merge
description: This bot will farm the items belonging to the selected mode for the Dawn Fortress Merge [2300] in /neofortress
tags: dawn, fortress, merge, neofortress, vindicator, archer, scout, archers, , scouts, bright, bow, general, captain, generals, captains, blessed, shield, vindication, hammer, hammers, battlegear, hollowborn, ranger, rangers, quiver, enchanted, battlemage, sigil, armaments
*/
// Port of Scripts/Hollowborn/MergeShops/DawnFortressMerge.cs.

import { bot, defineScriptOptions, option } from "turu";
import { CoreAdvanced, mergeOptionsEnum } from "../../core/CoreAdvanced";
import { ClassType, Core } from "../../core/CoreBots";
import { CoreFarms } from "../../core/CoreFarms";

const Farm = new CoreFarms();
const Adv = new CoreAdvanced();

export class DawnFortressMerge {
    static readonly select = [
        option.bool("78532", "Dawn Vindicator Archer", "Mode: [select] only\nShould the bot buy \"Dawn Vindicator Archer\" ?", false),
        option.bool("78533", "Vindicator Scout", "Mode: [select] only\nShould the bot buy \"Vindicator Scout\" ?", false),
        option.bool("78534", "Vindicator Archer's Hat + Locks", "Mode: [select] only\nShould the bot buy \"Vindicator Archer's Hat + Locks\" ?", false),
        option.bool("78536", "Vindicator Archer's Hat", "Mode: [select] only\nShould the bot buy \"Vindicator Archer's Hat\" ?", false),
        option.bool("78538", "Vindicator Scout's Hat + Locks", "Mode: [select] only\nShould the bot buy \"Vindicator Scout's Hat + Locks\" ?", false),
        option.bool("78539", "Vindicator Scout's Hat", "Mode: [select] only\nShould the bot buy \"Vindicator Scout's Hat\" ?", false),
        option.bool("78541", "Bright Bow of the Dawn", "Mode: [select] only\nShould the bot buy \"Bright Bow of the Dawn\" ?", false),
        option.bool("78545", "Dawn Vindicator General", "Mode: [select] only\nShould the bot buy \"Dawn Vindicator General\" ?", false),
        option.bool("78546", "Vindicator Captain", "Mode: [select] only\nShould the bot buy \"Vindicator Captain\" ?", false),
        option.bool("78547", "Vindicator General's Hood", "Mode: [select] only\nShould the bot buy \"Vindicator General's Hood\" ?", false),
        option.bool("78548", "Vindicator General's Hood + Locks", "Mode: [select] only\nShould the bot buy \"Vindicator General's Hood + Locks\" ?", false),
        option.bool("78549", "Vindicator Captain's Hood", "Mode: [select] only\nShould the bot buy \"Vindicator Captain's Hood\" ?", false),
        option.bool("78550", "Vindicator Captain's Hood + Locks", "Mode: [select] only\nShould the bot buy \"Vindicator Captain's Hood + Locks\" ?", false),
        option.bool("78552", "Blessed Shield of Vindication", "Mode: [select] only\nShould the bot buy \"Blessed Shield of Vindication\" ?", false),
        option.bool("78554", "Blessed Hammer of the Dawn", "Mode: [select] only\nShould the bot buy \"Blessed Hammer of the Dawn\" ?", false),
        option.bool("78555", "Blessed Hammers of the Dawn", "Mode: [select] only\nShould the bot buy \"Blessed Hammers of the Dawn\" ?", false),
        option.bool("78556", "Blessed Battlegear of the Dawn", "Mode: [select] only\nShould the bot buy \"Blessed Battlegear of the Dawn\" ?", false),
        option.bool("67444", "Hollowborn Ranger", "Mode: [select] only\nShould the bot buy \"Hollowborn Ranger\" ?", false),
        option.bool("67445", "Hollowborn Ranger's Hat + Locks", "Mode: [select] only\nShould the bot buy \"Hollowborn Ranger's Hat + Locks\" ?", false),
        option.bool("67446", "Hollowborn Ranger's Hat", "Mode: [select] only\nShould the bot buy \"Hollowborn Ranger's Hat\" ?", false),
        option.bool("67447", "Hollowborn Ranger Quiver", "Mode: [select] only\nShould the bot buy \"Hollowborn Ranger Quiver\" ?", false),
        option.bool("67448", "Enchanted Hollowborn Bow", "Mode: [select] only\nShould the bot buy \"Enchanted Hollowborn Bow\" ?", false),
        option.bool("67449", "Hollowborn Ranger Bow", "Mode: [select] only\nShould the bot buy \"Hollowborn Ranger Bow\" ?", false),
        option.bool("67450", "Hollowborn Battlemage", "Mode: [select] only\nShould the bot buy \"Hollowborn Battlemage\" ?", false),
        option.bool("67451", "Hollowborn Battlemage Hood", "Mode: [select] only\nShould the bot buy \"Hollowborn Battlemage Hood\" ?", false),
        option.bool("67452", "Hollowborn Battlemage Hood + Locks", "Mode: [select] only\nShould the bot buy \"Hollowborn Battlemage Hood + Locks\" ?", false),
        option.bool("67453", "Hollowborn Battlemage Rune", "Mode: [select] only\nShould the bot buy \"Hollowborn Battlemage Rune\" ?", false),
        option.bool("67454", "Hollowborn Battlemage Sigil", "Mode: [select] only\nShould the bot buy \"Hollowborn Battlemage Sigil\" ?", false),
        option.bool("67455", "Hollowborn Battlemage Hammer", "Mode: [select] only\nShould the bot buy \"Hollowborn Battlemage Hammer\" ?", false),
        option.bool("67456", "Hollowborn Battlemage Hammers", "Mode: [select] only\nShould the bot buy \"Hollowborn Battlemage Hammers\" ?", false),
        option.bool("67457", "Hollowborn Battlemage Armaments", "Mode: [select] only\nShould the bot buy \"Hollowborn Battlemage Armaments\" ?", false),
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

                case "Grace Orb":
                    Core.farmingLogger(req.name, quant);
                    Core.equipClass(ClassType.Farm);
                    Core.registerQuests(9291);
                    while (!bot.shouldExit && !Core.checkInventory(req.name, quant)) {
                        Core.huntMonster("neofortress", "Vindicator Recruit", "Grace Extracted", 20, { isTemp: false, log: false });
                        bot.wait.forPickup(req.name);
                    }
                    Core.cancelRegisteredQuests();
                    break;

                case "Vindicator Badge":
                    Core.farmingLogger(req.name, quant);
                    bot.quests.updateQuest(8297);
                    Core.registerQuests(8299);
                    while (!bot.shouldExit && !Core.checkInventory(req.name, quant)) {
                        Core.equipClass(ClassType.Farm);
                        Core.killMonster("trygve", "r3", "Left", "Blood Eagle", "Eagle Heart", 8);
                        Core.killMonster("trygve", "r4", "Left", "Rune Boar", "Boar Heart", 8);
                        Core.equipClass(ClassType.Solo);
                        Core.huntMonster("trygve", "Gramiel", "Vindicator Seal");
                        bot.wait.forPickup(req.name);
                    }
                    Core.cancelRegisteredQuests();
                    break;

                case "Vindicator Soldier's Hair":
                    Core.huntMonster("neofortress", "Vindicator Soldier", req.name, req.quantity, { isTemp: req.temp });
                    bot.wait.forPickup(req.name);
                    break;

                case "Vindicator Scout's Bow":
                    Core.huntMonster("neofortress", "Vindicator Recruit", req.name, req.quantity, { isTemp: req.temp });
                    bot.wait.forPickup(req.name);
                    break;

                case "Blessed Sigil of Vindication":
                case "Hammer of Vindication":
                case "Hammers of Vindication":
                    Core.huntMonster("neofortress", "Vindicator General", req.name, req.quantity, { isTemp: req.temp });
                    bot.wait.forPickup(req.name);
                    break;

                case "Hollow Soul":
                    Core.farmingLogger(req.name, quant);
                    Core.equipClass(ClassType.Farm);
                    while (!bot.shouldExit && !Core.checkInventory(req.name, quant)) {
                        Core.ensureAcceptmultiple(7553, 7555);
                        Core.killMonster("shadowrealm", "r2", "Left", "Gargrowl", "Darkseed", 8, { log: false });
                        Core.killMonster("shadowrealm", "r2", "Left", "Shadow Guardian", "Shadow Medallion", 5, { log: false });
                        Core.ensureComplete(7553);
                        Core.ensureComplete(7555);
                    }
                    bot.wait.forPickup(req.name);
                    break;

                case "Bone Dust":
                    Farm.battleUnderB(req.name, quant);
                    bot.wait.forPickup(req.name);
                    break;

                case "Gilded Scout's Quiver":
                    Core.huntMonster("neofortress", "Vindicator Recruit", req.name, req.quantity, { isTemp: req.temp });
                    bot.wait.forPickup(req.name);
                    break;

                case "Blessed Rune of Vindication":
                case "Battlegear of Vindication":
                    Core.huntMonster("neofortress", "Vindicator General", req.name, req.quantity, { isTemp: req.temp });
                    bot.wait.forPickup(req.name);
                    break;
            }
        };

        //Only edit the map and shopID here
        Adv.startBuyAllMerge("neofortress", 2300, findIngredients, buyOnlyThis, { buyMode: buyMode });
    }
}

export const scriptOptions = defineScriptOptions(Adv.optionsStorage, [], true, {
    Generic: Adv.mergeOptions,
    Select: DawnFortressMerge.select,
});

export default function main(): void {
        Core.bankingBlackList.push(...
            [
                "Grace Orb",
                "Vindicator Badge",
                "Vindicator Soldier's Hair",
                "Vindicator Scout's Bow",
                "Blessed Sigil of Vindication",
                "Hammer of Vindication",
                "Hammers of Vindication",
                "Hollow Soul",
                "Bone Dust",
                "Gilded Scout's Quiver",
                "Blessed Rune of Vindication",
                "Battlegear of Vindication",
            ]);
        Core.setOptions();

        new DawnFortressMerge().buyAllMerge();
        Core.setOptions(false);
}
