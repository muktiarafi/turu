/*
name: Nulgath Diamond Merge
description: This bot will farm the items belonging to the selected mode for the Nulgath Diamond Merge [456] in /evilwarnul
tags: nulgath, diamond, merge, evilwarnul, abyssal, priest, loyalty, nation, oversoul, witch, witchs, soulstealing, paladin, bearded, dire, monk, cleric, head, golden, hanzo, void, katana, katanas, horns, cyber, crystal, phoenix, blood, cloak, horned, worshipper, face, wartorn, memorabilia, star, ranger, sheath, bow, morph, quiver, storm, knight, formal, suit, circlet, cane, warlord, sin, cleaver, cleavers, warmonger, armaments, malakais, pet, fiendish, arachnomancers, whips, whip, archfiend, claymores, claymore, balors, hunger, arachnid, gestalt, arachnomancer
*/
// Port of Scripts/Nation/MergeShops/NulgathDiamondMerge.cs.

import { bot, defineScriptOptions, option } from "turu";
import { CoreAdvanced, mergeOptionsEnum } from "../../core/CoreAdvanced";
import { ClassType, Core } from "../../core/CoreBots";
import { CoreDailies } from "../../core/CoreDailies";
import { CoreFarms } from "../../core/CoreFarms";
import { CoreBLOD } from "../../good/blod/CoreBLOD";
import { CoreNation } from "../CoreNation";
import { TarosManslayer } from "../various/TarosManslayer";

const Farm = new CoreFarms();
const Adv = new CoreAdvanced();
const Dailies = new CoreDailies();
const Nation = new CoreNation();
const Taro = new TarosManslayer();
const BLOD = new CoreBLOD();

export class NulgathDiamondMerge {
    static readonly select = [
        option.bool("13347", "Abyssal Priest of Nulgath", "Mode: [select] only\nShould the bot buy \"Abyssal Priest of Nulgath\" ?", false),
        option.bool("25215", "Loyalty Blade of the Nation", "Mode: [select] only\nShould the bot buy \"Loyalty Blade of the Nation\" ?", false),
        option.bool("25239", "Oversoul Witch of Nulgath", "Mode: [select] only\nShould the bot buy \"Oversoul Witch of Nulgath\" ?", false),
        option.bool("25240", "OverSoul Witch's Hat", "Mode: [select] only\nShould the bot buy \"OverSoul Witch's Hat\" ?", false),
        option.bool("25198", "Staff of Soul-Stealing", "Mode: [select] only\nShould the bot buy \"Staff of Soul-Stealing\" ?", false),
        option.bool("25241", "OverSoul Paladin", "Mode: [select] only\nShould the bot buy \"OverSoul Paladin\" ?", false),
        option.bool("25252", "Bearded Axe of Nulgath", "Mode: [select] only\nShould the bot buy \"Bearded Axe of Nulgath\" ?", false),
        option.bool("25251", "Dual Bearded Axe of Nulgath", "Mode: [select] only\nShould the bot buy \"Dual Bearded Axe of Nulgath\" ?", false),
        option.bool("25266", "Oversoul Dire Monk", "Mode: [select] only\nShould the bot buy \"Oversoul Dire Monk\" ?", false),
        option.bool("25264", "OverSoul Cleric Cape", "Mode: [select] only\nShould the bot buy \"OverSoul Cleric Cape\" ?", false),
        option.bool("25263", "OverSoul Witch Cape", "Mode: [select] only\nShould the bot buy \"OverSoul Witch Cape\" ?", false),
        option.bool("25256", "OverSoul Cleric Locks", "Mode: [select] only\nShould the bot buy \"OverSoul Cleric Locks\" ?", false),
        option.bool("25267", "OverSoul Dire Monk Head", "Mode: [select] only\nShould the bot buy \"OverSoul Dire Monk Head\" ?", false),
        option.bool("25265", "OverSoul Paladin Head", "Mode: [select] only\nShould the bot buy \"OverSoul Paladin Head\" ?", false),
        option.bool("25255", "Oversoul Cleric of Nulgath", "Mode: [select] only\nShould the bot buy \"Oversoul Cleric of Nulgath\" ?", false),
        option.bool("27849", "Golden Hanzo Void Katana", "Mode: [select] only\nShould the bot buy \"Golden Hanzo Void Katana\" ?", false),
        option.bool("27850", "Dual Golden Hanzo Void Katanas", "Mode: [select] only\nShould the bot buy \"Dual Golden Hanzo Void Katanas\" ?", false),
        option.bool("27843", "Golden Hanzo Void", "Mode: [select] only\nShould the bot buy \"Golden Hanzo Void\" ?", false),
        option.bool("27851", "Golden Hanzo Void Mask", "Mode: [select] only\nShould the bot buy \"Golden Hanzo Void Mask\" ?", false),
        option.bool("27852", "Golden Hanzo Void Horns", "Mode: [select] only\nShould the bot buy \"Golden Hanzo Void Horns\" ?", false),
        option.bool("28031", "Golden Hanzo Katana Cape", "Mode: [select] only\nShould the bot buy \"Golden Hanzo Katana Cape\" ?", false),
        option.bool("27848", "Golden Hanzo Void Cape", "Mode: [select] only\nShould the bot buy \"Golden Hanzo Void Cape\" ?", false),
        option.bool("30208", "Cyber Crystal Phoenix Blade", "Mode: [select] only\nShould the bot buy \"Cyber Crystal Phoenix Blade\" ?", false),
        option.bool("344", "Blood Cloak", "Mode: [select] only\nShould the bot buy \"Blood Cloak\" ?", false),
        option.bool("52883", "Horned Worshipper Face of Nulgath", "Mode: [select] only\nShould the bot buy \"Horned Worshipper Face of Nulgath\" ?", false),
        option.bool("52884", "Horned Worshipper Visage of Nulgath", "Mode: [select] only\nShould the bot buy \"Horned Worshipper Visage of Nulgath\" ?", false),
        option.bool("52824", "War-Torn Memorabilia", "Mode: [select] only\nShould the bot buy \"War-Torn Memorabilia\" ?", false),
        option.bool("67267", "Abyssal Star", "Mode: [select] only\nShould the bot buy \"Abyssal Star\" ?", false),
        option.bool("69968", "Blood Ranger", "Mode: [select] only\nShould the bot buy \"Blood Ranger\" ?", false),
        option.bool("69973", "Blood Ranger Sword and Sheath", "Mode: [select] only\nShould the bot buy \"Blood Ranger Sword and Sheath\" ?", false),
        option.bool("69972", "Blood Ranger Bow", "Mode: [select] only\nShould the bot buy \"Blood Ranger Bow\" ?", false),
        option.bool("69969", "Blood Ranger Morph", "Mode: [select] only\nShould the bot buy \"Blood Ranger Morph\" ?", false),
        option.bool("69971", "Blood Ranger Cape", "Mode: [select] only\nShould the bot buy \"Blood Ranger Cape\" ?", false),
        option.bool("69970", "Blood Ranger Quiver", "Mode: [select] only\nShould the bot buy \"Blood Ranger Quiver\" ?", false),
        option.bool("70001", "Storm Knight", "Mode: [select] only\nShould the bot buy \"Storm Knight\" ?", false),
        option.bool("70002", "Storm Knight Helm", "Mode: [select] only\nShould the bot buy \"Storm Knight Helm\" ?", false),
        option.bool("83410", "Formal Nation Suit", "Mode: [select] only\nShould the bot buy \"Formal Nation Suit\" ?", false),
        option.bool("83411", "Formal Nation Hair", "Mode: [select] only\nShould the bot buy \"Formal Nation Hair\" ?", false),
        option.bool("83412", "Formal Nation Locks", "Mode: [select] only\nShould the bot buy \"Formal Nation Locks\" ?", false),
        option.bool("83413", "Formal Nation Mask", "Mode: [select] only\nShould the bot buy \"Formal Nation Mask\" ?", false),
        option.bool("83414", "Formal Nation Circlet", "Mode: [select] only\nShould the bot buy \"Formal Nation Circlet\" ?", false),
        option.bool("83415", "Formal Nation Morph", "Mode: [select] only\nShould the bot buy \"Formal Nation Morph\" ?", false),
        option.bool("83416", "Formal Nation Visage", "Mode: [select] only\nShould the bot buy \"Formal Nation Visage\" ?", false),
        option.bool("83417", "Formal Nation Mask Morph", "Mode: [select] only\nShould the bot buy \"Formal Nation Mask Morph\" ?", false),
        option.bool("83418", "Formal Nation Circlet Visage", "Mode: [select] only\nShould the bot buy \"Formal Nation Circlet Visage\" ?", false),
        option.bool("83419", "Formal Nation Blade", "Mode: [select] only\nShould the bot buy \"Formal Nation Blade\" ?", false),
        option.bool("83420", "Formal Nation Blades", "Mode: [select] only\nShould the bot buy \"Formal Nation Blades\" ?", false),
        option.bool("83421", "Formal Nation Cane", "Mode: [select] only\nShould the bot buy \"Formal Nation Cane\" ?", false),
        option.bool("83422", "Formal Nation Cane Sheath", "Mode: [select] only\nShould the bot buy \"Formal Nation Cane Sheath\" ?", false),
        option.bool("84612", "Warlord of Nulgath Blade", "Mode: [select] only\nShould the bot buy \"Warlord of Nulgath Blade\" ?", false),
        option.bool("84613", "Warlord of Nulgath Blades", "Mode: [select] only\nShould the bot buy \"Warlord of Nulgath Blades\" ?", false),
        option.bool("84614", "Sin of the Warlord Cleaver", "Mode: [select] only\nShould the bot buy \"Sin of the Warlord Cleaver\" ?", false),
        option.bool("84615", "Sin of the Warlord Cleavers", "Mode: [select] only\nShould the bot buy \"Sin of the Warlord Cleavers\" ?", false),
        option.bool("84616", "Void Warmonger Armaments", "Mode: [select] only\nShould the bot buy \"Void Warmonger Armaments\" ?", false),
        option.bool("91496", "Malakai's Katana Pet", "Mode: [select] only\nShould the bot buy \"Malakai's Katana Pet\" ?", false),
        option.bool("91442", "Fiendish Arachnomancer's Whips", "Mode: [select] only\nShould the bot buy \"Fiendish Arachnomancer's Whips\" ?", false),
        option.bool("91441", "Fiendish Arachnomancer's Whip", "Mode: [select] only\nShould the bot buy \"Fiendish Arachnomancer's Whip\" ?", false),
        option.bool("91435", "Archfiend Arachnomancer's Claymores", "Mode: [select] only\nShould the bot buy \"Archfiend Arachnomancer's Claymores\" ?", false),
        option.bool("91434", "Archfiend Arachnomancer's Claymore", "Mode: [select] only\nShould the bot buy \"Archfiend Arachnomancer's Claymore\" ?", false),
        option.bool("91433", "Dual Balor's Hunger", "Mode: [select] only\nShould the bot buy \"Dual Balor's Hunger\" ?", false),
        option.bool("91432", "Balor's Hunger", "Mode: [select] only\nShould the bot buy \"Balor's Hunger\" ?", false),
        option.bool("91426", "Arachnid Gestalt Cloak of Nulgath", "Mode: [select] only\nShould the bot buy \"Arachnid Gestalt Cloak of Nulgath\" ?", false),
        option.bool("91421", "Fiendish Arachnomancer Mask", "Mode: [select] only\nShould the bot buy \"Fiendish Arachnomancer Mask\" ?", false),
        option.bool("91418", "Fiendish Arachnomancer", "Mode: [select] only\nShould the bot buy \"Fiendish Arachnomancer\" ?", false),
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

                case "Nulgath's Approval":
                    Nation.approvalAndFavor(quant, 0);
                    break;

                case "Taro's Manslayer":
                    Taro.guardianTaro(true);
                    break;

                case "Dark Crystal Shard":
                    Nation.farmDarkCrystalShard(quant);
                    break;

                case "Tainted Gem":
                    Nation.farmTaintedGem(quant);
                    break;

                case "Blade of Holy Might":
                    Core.equipClass(ClassType.Solo);
                    Core.huntMonster("northlands", "Aisha's Drake", "Blade of Holy Might", 1, { isTemp: false });
                    break;

                case "Blood Gem of the Archfiend":
                    Nation.farmBloodGem(quant);
                    break;

                case "Unidentified 13":
                    Nation.farmUni13(quant);
                    break;

                case "Iron":
                    BLOD.unlockMineCrafting();
                    Dailies.mineCrafting([ "Iron" ], quant);
                    break;

                case "Gem of Nulgath":
                    Nation.farmGemofNulgath(quant);
                    break;

                case "Totem of Nulgath":
                    Nation.farmTotemofNulgath(quant);
                    break;

                case "Bone Dust":
                    Farm.battleUnderB("Bone Dust", quant);
                    break;

                case "Cloak of Nulgath":
                    Core.buyItem("tercessuinotlim", 4667, "Cloak of Nulgath");
                    break;

                case "Staff of Imp Fire":
                    Core.equipClass(ClassType.Farm);
                    Core.huntMonster("bludrut2", "Fire Elemental", "Staff of Imp Fire", 1, { isTemp: false });
                    break;

                case "Cool Head":
                    Core.buyItem("tercessuinotlim", 4826, "Cool Head");
                    break;

                case "Primal Dread Fang":
                    Nation.supplies("Primal Dread Fang", quant);
                    break;

                case "Random Weapon of Nulgath":
                    Nation.supplies("Random Weapon of Nulgath", quant);
                    break;

                case "Voucher of Nulgath (non-mem)":
                    Nation.farmVoucher({ member:false });
                    break;

                case "Unidentified 27":
                    Core.farmingLogger(req.name, quant);
                    Core.ensureAccept(584);
                    Nation.supplies("Unidentified 26");

                    Core.resetQuest(7551);
                    Core.darkMakaiItem("Dark Makai Sigil");
                    Core.ensureComplete(584);
                    bot.wait.forPickup(req.name);
                    break;

                case "Crystal Phoenix Blade of Nulgath":
                    Core.farmingLogger(req.name, quant);
                    Nation.farmDiamondofNulgath(13);
                    Nation.farmDarkCrystalShard(50);
                    Nation.farmTotemofNulgath(3);
                    Nation.farmGemofNulgath(20);
                    Nation.farmVoucher({ member:false });
                    Nation.farmTaintedGem(50);

                    Core.ensureAccept(837);
                    Core.huntMonster("underworld", "Undead Bruiser", "Undead Bruiser Rune");
                    Core.ensureComplete(837, req.id);
                    bot.wait.forPickup(req.name);
                    break;

                case "Gold Star of Avarice":
                    Core.farmingLogger(req.name, quant);
                    Adv.buyItem("tercessuinotlim", 1951, req.name);
                    break;
            }
        };

        //Only edit the map and shopID here
        Adv.startBuyAllMerge("evilwarnul", 456, findIngredients, buyOnlyThis, { buyMode: buyMode });
    }
}

export const scriptOptions = defineScriptOptions(Adv.optionsStorage, [], true, {
    Generic: Adv.mergeOptions,
    Select: NulgathDiamondMerge.select,
});

export default function main(): void {
        Core.bankingBlackList.push(...
            [
                "Diamond of Nulgath",
                "Archfiend's Favor",
                "Nulgath's Approval",
                "Taro's Manslayer",
                "Dark Crystal Shard",
                "Tainted Gem",
                "Blade of Holy Might",
                "Blood Gem of the Archfiend",
                "Unidentified 13",
                "Iron",
                "Gem of Nulgath",
                "Totem of Nulgath",
                "Bone Dust",
                "Cloak of Nulgath",
                "Staff of Imp Fire",
                "Cool Head",
                "Primal Dread Fang",
                "Random Weapon of Nulgath",
                "Voucher of Nulgath (non-mem)",
                "Unidentified 27",
                "Crystal Phoenix Blade of Nulgath",
                "Gold Star of Avarice",
            ]);
        Core.setOptions();

        new NulgathDiamondMerge().buyAllMerge();

        Core.setOptions(false);
}
