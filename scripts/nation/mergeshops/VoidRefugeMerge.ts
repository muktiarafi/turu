/*
name: Void Refuge Merge
description: This bot will farm the items belonging to the selected mode for the Void Refuge Merge [2408] in /voidrefuge
tags: void, refuge, merge, voidrefuge, envenomed, edge, nulgath, enchanted, carnage, evolved, crest, blood, spines, bloodletter, katana, katanas, bone, quaker, betrayal, soul, corroding, visionary, heart, render, golden, redeemer, poison, drinker, guillotine, fiend, carver, unstable
*/
// Port of Scripts/Nation/MergeShops/VoidRefugeMerge.cs.

import { bot, defineScriptOptions, option } from "turu";
import { CoreAdvanced, mergeOptionsEnum } from "../../core/CoreAdvanced";
import { ClassType, Core } from "../../core/CoreBots";
import { CoreFarms } from "../../core/CoreFarms";
import { VoidRefuge } from "../../story/nation/VoidRefuge";
import { CoreNation } from "../CoreNation";

const Farm = new CoreFarms();
const Adv = new CoreAdvanced();
const VR = new VoidRefuge();
const Nation = new CoreNation();

export class VoidRefugeMerge {
    static readonly select = [
        option.bool("79256", "Envenomed Edge of Nulgath", "Mode: [select] only\nShould the bot buy \"Envenomed Edge of Nulgath\" ?", false),
        option.bool("83090", "Enchanted Carnage Void of Nulgath", "Mode: [select] only\nShould the bot buy \"Enchanted Carnage Void of Nulgath\" ?", false),
        option.bool("83091", "Enchanted Evolved Carnage Helm", "Mode: [select] only\nShould the bot buy \"Enchanted Evolved Carnage Helm\" ?", false),
        option.bool("83092", "Enchanted Evolved Carnage Crest", "Mode: [select] only\nShould the bot buy \"Enchanted Evolved Carnage Crest\" ?", false),
        option.bool("83093", "Enchanted Blood Void Spines", "Mode: [select] only\nShould the bot buy \"Enchanted Blood Void Spines\" ?", false),
        option.bool("83094", "Enchanted Blood Void Spikes", "Mode: [select] only\nShould the bot buy \"Enchanted Blood Void Spikes\" ?", false),
        option.bool("83095", "Enchanted Bloodletter Katana", "Mode: [select] only\nShould the bot buy \"Enchanted Bloodletter Katana\" ?", false),
        option.bool("83096", "Enchanted Bloodletter Katanas", "Mode: [select] only\nShould the bot buy \"Enchanted Bloodletter Katanas\" ?", false),
        option.bool("83141", "Bone Quaker Betrayal Blade", "Mode: [select] only\nShould the bot buy \"Bone Quaker Betrayal Blade\" ?", false),
        option.bool("83142", "Bone Quaker Betrayal Blades", "Mode: [select] only\nShould the bot buy \"Bone Quaker Betrayal Blades\" ?", false),
        option.bool("83143", "Soul Corroding Betrayal Blade", "Mode: [select] only\nShould the bot buy \"Soul Corroding Betrayal Blade\" ?", false),
        option.bool("83144", "Soul Corroding Betrayal Blades", "Mode: [select] only\nShould the bot buy \"Soul Corroding Betrayal Blades\" ?", false),
        option.bool("83145", "Visionary Betrayal Blade", "Mode: [select] only\nShould the bot buy \"Visionary Betrayal Blade\" ?", false),
        option.bool("83146", "Visionary Betrayal Blade", "Mode: [select] only\nShould the bot buy \"Visionary Betrayal Blade\" ?", false),
        option.bool("83147", "Heart Render Betrayal Blade", "Mode: [select] only\nShould the bot buy \"Heart Render Betrayal Blade\" ?", false),
        option.bool("83148", "Heart Render Betrayal Blades", "Mode: [select] only\nShould the bot buy \"Heart Render Betrayal Blades\" ?", false),
        option.bool("83149", "Golden Redeemer Betrayal Blade", "Mode: [select] only\nShould the bot buy \"Golden Redeemer Betrayal Blade\" ?", false),
        option.bool("83150", "Golden Redeemer Betrayal Blades", "Mode: [select] only\nShould the bot buy \"Golden Redeemer Betrayal Blades\" ?", false),
        option.bool("83151", "Poison Drinker Betrayal Blade", "Mode: [select] only\nShould the bot buy \"Poison Drinker Betrayal Blade\" ?", false),
        option.bool("83152", "Poison Drinker Betrayal Blades", "Mode: [select] only\nShould the bot buy \"Poison Drinker Betrayal Blades\" ?", false),
        option.bool("83153", "Guillotine Betrayal Blade", "Mode: [select] only\nShould the bot buy \"Guillotine Betrayal Blade\" ?", false),
        option.bool("83154", "Guillotine Betrayal Blades", "Mode: [select] only\nShould the bot buy \"Guillotine Betrayal Blades\" ?", false),
        option.bool("83155", "Fiend Carver Betrayal Blade", "Mode: [select] only\nShould the bot buy \"Fiend Carver Betrayal Blade\" ?", false),
        option.bool("83156", "Fiend Carver Betrayal Blades", "Mode: [select] only\nShould the bot buy \"Fiend Carver Betrayal Blades\" ?", false),
        option.bool("83139", "Unstable Betrayal Blade", "Mode: [select] only\nShould the bot buy \"Unstable Betrayal Blade\" ?", false),
        option.bool("83140", "Unstable Betrayal Blades", "Mode: [select] only\nShould the bot buy \"Unstable Betrayal Blades\" ?", false),
        option.bool("82878", "Void Betrayal Knight", "Mode: [select] only\nShould the bot buy \"Void Betrayal Knight\" ?", false),
        option.bool("82879", "Void Betrayer Crest", "Mode: [select] only\nShould the bot buy \"Void Betrayer Crest\" ?", false),
        option.bool("82880", "Void Betrayer Guard", "Mode: [select] only\nShould the bot buy \"Void Betrayer Guard\" ?", false),
        option.bool("82881", "Void Betrayer Horns", "Mode: [select] only\nShould the bot buy \"Void Betrayer Horns\" ?", false),
        option.bool("82882", "Void Betrayer Cloak", "Mode: [select] only\nShould the bot buy \"Void Betrayer Cloak\" ?", false),
        option.bool("82883", "Void Betrayer Spined Cloak", "Mode: [select] only\nShould the bot buy \"Void Betrayer Spined Cloak\" ?", false),
        option.bool("82885", "Void Betrayer Sword", "Mode: [select] only\nShould the bot buy \"Void Betrayer Sword\" ?", false),
        option.bool("83323", "Void Betrayer Swords", "Mode: [select] only\nShould the bot buy \"Void Betrayer Swords\" ?", false),
        option.bool("82886", "Void Betrayer Dagger", "Mode: [select] only\nShould the bot buy \"Void Betrayer Dagger\" ?", false),
        option.bool("82887", "Void Betrayer Daggers", "Mode: [select] only\nShould the bot buy \"Void Betrayer Daggers\" ?", false),
        option.bool("82888", "Void Betrayer Reaver", "Mode: [select] only\nShould the bot buy \"Void Betrayer Reaver\" ?", false),
        option.bool("82889", "Void Betrayer Reavers", "Mode: [select] only\nShould the bot buy \"Void Betrayer Reavers\" ?", false),
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

                case "Venomous Fang Blade":
                    Core.farmingLogger(req.name, quant);
                    Core.huntMonster("tercessuinotlim", "Ninja Spy", "Spy's Info", 1, { isTemp: false, log: false });
                    Core.huntMonster("citadel", "Inquisitor Captain", "Captain's Info", 1, { isTemp: false, log: false });
                    Core.huntMonster("lairattack", "Flame Dragon General", "Broken Fang Blade", 1, { isTemp: false, log: false });
                    Core.getMapItem(12571, 1, "museum");
                    break;

                case "Unidentified 13":
                    Core.farmingLogger(req.name, quant);
                    Nation.farmUni13(quant);
                    break;

                case "Tainted Gem":
                    Core.farmingLogger(req.name, quant);
                    Nation.farmTaintedGem(quant);
                    break;

                case "Dark Crystal Shard":
                    Core.farmingLogger(req.name, quant);
                    Nation.farmDarkCrystalShard(quant);
                    break;

                case "Diamond of Nulgath":
                    Core.farmingLogger(req.name, quant);
                    Nation.farmDiamondofNulgath(quant);
                    break;

                case "Unidentified 23":
                    Core.farmingLogger(req.name, quant);
                    Nation.theAssistant("Unidentified 23", quant);
                    break;

                case "Totem of Nulgath":
                    Core.farmingLogger(req.name, quant);
                    Nation.farmTotemofNulgath(quant);
                    break;

                case "Blood Gem of the Archfiend":
                    Core.farmingLogger(req.name, quant);
                    Nation.farmBloodGem(quant);
                    break;

                case "Evolved Carnage of Nulgath":
                case "Evolved Carnage Helm":
                case "Evolved Carnage Crest":
                case "Blood Void Spines":
                case "Blood Void Spikes":
                case "Bloodletter Katana":
                case "Bloodletter Katanas":
                    Core.farmingLogger(req.name, quant);
                    Core.equipClass(ClassType.Solo);
                    Core.huntMonster("voidrefuge", "Carnage", req.name, quant, { isTemp: false, log: false });
                    break;

                case "Fiendish Remains":
                    Core.farmingLogger(req.name, quant);
                    Core.registerQuests(9532);
                    while (!bot.shouldExit && !Core.checkInventory(req.name, quant)) {
                        Core.equipClass(ClassType.Farm);
                        Core.huntMonster("voidrefuge", "Paladin Ascendant", "Sussurating Helm", 3, { log: false });
                        Core.huntMonster("voidrefuge", "Nation Outrider", "Scarred Coin", 8, { log: false });
                        Core.equipClass(ClassType.Solo);
                        Core.huntMonster("voidrefuge", "Carnage", "Carnage's Ichor", 1, { log: false });
                        bot.wait.forPickup(req.name);
                    }
                    Core.cancelRegisteredQuests();
                    break;

                case "Voucher of Nulgath (non-mem)":
                    Core.farmingLogger(req.name, quant);
                    Nation.farmVoucher({ member:false });
                    break;

                case "Gem of Nulgath":
                    Core.farmingLogger(req.name, quant);
                    Nation.farmGemofNulgath(quant);
                    break;

                case "1st Betrayal Blade of Nulgath":
                case "2nd Betrayal Blade of Nulgath":
                case "3rd Betrayal Blade of Nulgath":
                case "4th Betrayal Blade of Nulgath":
                case "5th Betrayal Blade of Nulgath":
                case "6th Betrayal Blade of Nulgath":
                case "7th Betrayal Blade of Nulgath":
                case "8th Betrayal Blade of Nulgath":
                    Core.farmingLogger(req.name, quant);
                    Nation.kisstheVoid(0, req.name);
                    break;

                case "Unmoulded Fiend Essence":
                    Core.farmingLogger(req.name, quant);
                    Adv.buyItem("tercessuinotlim", 1951, "Unmoulded Fiend Essence");
                    break;
            }
        };

        VR.storyline();
        //Only edit the map and shopID here
        Adv.startBuyAllMerge("voidrefuge", 2408, findIngredients, buyOnlyThis, { buyMode: buyMode });
    }
}

export const scriptOptions = defineScriptOptions(Adv.optionsStorage, [], true, {
    Generic: Adv.mergeOptions,
    Select: VoidRefugeMerge.select,
});

export default function main(): void {
        Core.bankingBlackList.push(...
            [
                "Venomous Fang Blade",
                "Unidentified 13",
                "Tainted Gem",
                "Dark Crystal Shard",
                "Diamond of Nulgath",
                "Unidentified 23",
                "Totem of Nulgath",
                "Blood Gem of the Archfiend",
                "Evolved Carnage of Nulgath",
                "Fiendish Remains",
                "Voucher of Nulgath (non-mem)",
                "Evolved Carnage Helm",
                "Evolved Carnage Crest",
                "Blood Void Spines",
                "Blood Void Spikes",
                "Gem of Nulgath",
                "Bloodletter Katana",
                "Bloodletter Katanas",
                "1st Betrayal Blade of Nulgath",
                "2nd Betrayal Blade of Nulgath",
                "3rd Betrayal Blade of Nulgath",
                "4th Betrayal Blade of Nulgath",
                "5th Betrayal Blade of Nulgath",
                "6th Betrayal Blade of Nulgath",
                "7th Betrayal Blade of Nulgath",
                "8th Betrayal Blade of Nulgath",
                "Unmoulded Fiend Essence",
                "Gold Voucher 25k",
            ]);
        Core.setOptions();

        new VoidRefugeMerge().buyAllMerge();
        Core.setOptions(false);
}
