/*
name: Shadowrealm Merge
description: This bot will farm the items belonging to the selected mode for the Shadowrealm Merge [1889] in /shadowrealm
tags: shadowrealm, merge, shadowrealm, hollowborn, thunderlord, visor, lightning, lightnings, vampire, flying, skulls, scythe, chained, reaper, guard, fiend, blind, reapers, , horns, ponytail, katana, kamas, kama, minion, sheathed, dragonslayer, dragonslayers, beard, scarf, vine, rogue, thorn, assassin, assassins, wreath, thorns, guardian, whip, agony, poison, claws, ranger, rangers, quiver, enchanted, bow, battlemage, sigil, hammer, hammers, armaments
*/
// Port of Scripts/Hollowborn/MergeShops/ShadowrealmMerge.cs.

import { bot, defineScriptOptions, option } from "turu";
import { CoreAdvanced, mergeOptionsEnum } from "../../core/CoreAdvanced";
import { ClassType, Core } from "../../core/CoreBots";
import { CoreFarms } from "../../core/CoreFarms";
import { YulgarsUndineMerge } from "../../other/mergeshops/YulgarsUndineMerge";
import { DawnFortressMerge } from "./DawnFortressMerge";

const Farm = new CoreFarms();
const Adv = new CoreAdvanced();
const YUM = new YulgarsUndineMerge();
const DFM = new DawnFortressMerge();

export class ShadowrealmMerge {
    static readonly select = [
        option.bool("54867", "Hollowborn Thunderlord", "Mode: [select] only\nShould the bot buy \"Hollowborn Thunderlord\" ?", false),
        option.bool("54868", "Hollowborn Thunderlord Visor", "Mode: [select] only\nShould the bot buy \"Hollowborn Thunderlord Visor\" ?", false),
        option.bool("54869", "Hollowborn Thunderlord Visage", "Mode: [select] only\nShould the bot buy \"Hollowborn Thunderlord Visage\" ?", false),
        option.bool("54870", "Hollowborn Thunderlord Lightning Cape", "Mode: [select] only\nShould the bot buy \"Hollowborn Thunderlord Lightning Cape\" ?", false),
        option.bool("54871", "Hollowborn Thunderlord Lightning", "Mode: [select] only\nShould the bot buy \"Hollowborn Thunderlord Lightning\" ?", false),
        option.bool("54872", "Hollowborn Thunderlord Lightnings", "Mode: [select] only\nShould the bot buy \"Hollowborn Thunderlord Lightnings\" ?", false),
        option.bool("54874", "Hollowborn Vampire", "Mode: [select] only\nShould the bot buy \"Hollowborn Vampire\" ?", false),
        option.bool("54875", "Hollowborn Vampire Hair", "Mode: [select] only\nShould the bot buy \"Hollowborn Vampire Hair\" ?", false),
        option.bool("54876", "Hollowborn Vampire Mask", "Mode: [select] only\nShould the bot buy \"Hollowborn Vampire Mask\" ?", false),
        option.bool("54877", "Hollowborn Vampire Flying Skulls", "Mode: [select] only\nShould the bot buy \"Hollowborn Vampire Flying Skulls\" ?", false),
        option.bool("54878", "Hollowborn Vampire Scythe", "Mode: [select] only\nShould the bot buy \"Hollowborn Vampire Scythe\" ?", false),
        option.bool("61766", "Hollowborn Chained Reaper", "Mode: [select] only\nShould the bot buy \"Hollowborn Chained Reaper\" ?", false),
        option.bool("61767", "Hollowborn Chained Guard", "Mode: [select] only\nShould the bot buy \"Hollowborn Chained Guard\" ?", false),
        option.bool("61768", "Hollowborn Fiend Reaper", "Mode: [select] only\nShould the bot buy \"Hollowborn Fiend Reaper\" ?", false),
        option.bool("61769", "Hollowborn Fiend Guard", "Mode: [select] only\nShould the bot buy \"Hollowborn Fiend Guard\" ?", false),
        option.bool("61770", "Hollowborn Fiend Hair", "Mode: [select] only\nShould the bot buy \"Hollowborn Fiend Hair\" ?", false),
        option.bool("61771", "Blind Hollowborn Fiend Hair", "Mode: [select] only\nShould the bot buy \"Blind Hollowborn Fiend Hair\" ?", false),
        option.bool("61772", "Hollowborn Fiend Mask", "Mode: [select] only\nShould the bot buy \"Hollowborn Fiend Mask\" ?", false),
        option.bool("61773", "Hollowborn Reaper's Hood", "Mode: [select] only\nShould the bot buy \"Hollowborn Reaper's Hood\" ?", false),
        option.bool("61774", "Hollowborn Reaper's Hood + Locks", "Mode: [select] only\nShould the bot buy \"Hollowborn Reaper's Hood + Locks\" ?", false),
        option.bool("61775", "Hollowborn Reaper's Hooded Mask", "Mode: [select] only\nShould the bot buy \"Hollowborn Reaper's Hooded Mask\" ?", false),
        option.bool("61776", "Hollowborn Fiend Reaper's Horns", "Mode: [select] only\nShould the bot buy \"Hollowborn Fiend Reaper's Horns\" ?", false),
        option.bool("61777", "Hollowborn Reaper's Ponytail", "Mode: [select] only\nShould the bot buy \"Hollowborn Reaper's Ponytail\" ?", false),
        option.bool("61778", "Blind Hollowborn Reaper's Ponytail", "Mode: [select] only\nShould the bot buy \"Blind Hollowborn Reaper's Ponytail\" ?", false),
        option.bool("61779", "Hollowborn Reaper's Mask + Ponytail", "Mode: [select] only\nShould the bot buy \"Hollowborn Reaper's Mask + Ponytail\" ?", false),
        option.bool("61780", "Hollowborn Reaper's Katana", "Mode: [select] only\nShould the bot buy \"Hollowborn Reaper's Katana\" ?", false),
        option.bool("61781", "Hollowborn Reaper's Scythe", "Mode: [select] only\nShould the bot buy \"Hollowborn Reaper's Scythe\" ?", false),
        option.bool("61782", "Hollowborn Reaper's Daggers", "Mode: [select] only\nShould the bot buy \"Hollowborn Reaper's Daggers\" ?", false),
        option.bool("61783", "Hollowborn Reaper's Kamas", "Mode: [select] only\nShould the bot buy \"Hollowborn Reaper's Kamas\" ?", false),
        option.bool("61784", "Hollowborn Reaper's Kama", "Mode: [select] only\nShould the bot buy \"Hollowborn Reaper's Kama\" ?", false),
        option.bool("61785", "Hollowborn Reaper's Minion", "Mode: [select] only\nShould the bot buy \"Hollowborn Reaper's Minion\" ?", false),
        option.bool("61786", "Hollowborn Reaper's Sheathed Katana", "Mode: [select] only\nShould the bot buy \"Hollowborn Reaper's Sheathed Katana\" ?", false),
        option.bool("55355", "Hollowborn DragonSlayer", "Mode: [select] only\nShould the bot buy \"Hollowborn DragonSlayer\" ?", false),
        option.bool("55356", "Hollowborn DragonSlayer's Helm", "Mode: [select] only\nShould the bot buy \"Hollowborn DragonSlayer's Helm\" ?", false),
        option.bool("55357", "Hollowborn DragonSlayer's Beard", "Mode: [select] only\nShould the bot buy \"Hollowborn DragonSlayer's Beard\" ?", false),
        option.bool("55358", "Hollowborn DragonSlayer's Scarf", "Mode: [select] only\nShould the bot buy \"Hollowborn DragonSlayer's Scarf\" ?", false),
        option.bool("55359", "Hollowborn DragonSlayer's Cape", "Mode: [select] only\nShould the bot buy \"Hollowborn DragonSlayer's Cape\" ?", false),
        option.bool("55360", "Hollowborn DragonSlayer's Blade", "Mode: [select] only\nShould the bot buy \"Hollowborn DragonSlayer's Blade\" ?", false),
        option.bool("78297", "Hollowborn Vine Rogue", "Mode: [select] only\nShould the bot buy \"Hollowborn Vine Rogue\" ?", false),
        option.bool("78298", "Hollowborn Thorn Assassin", "Mode: [select] only\nShould the bot buy \"Hollowborn Thorn Assassin\" ?", false),
        option.bool("78299", "Hollowborn Assassin's Locks", "Mode: [select] only\nShould the bot buy \"Hollowborn Assassin's Locks\" ?", false),
        option.bool("78300", "Hollowborn Assassin's Locks + Scarf", "Mode: [select] only\nShould the bot buy \"Hollowborn Assassin's Locks + Scarf\" ?", false),
        option.bool("78301", "Hollowborn Assassin's Hair", "Mode: [select] only\nShould the bot buy \"Hollowborn Assassin's Hair\" ?", false),
        option.bool("78302", "Hollowborn Assassin's Hair + Scarf", "Mode: [select] only\nShould the bot buy \"Hollowborn Assassin's Hair + Scarf\" ?", false),
        option.bool("78303", "Hollowborn Wreath of Thorns", "Mode: [select] only\nShould the bot buy \"Hollowborn Wreath of Thorns\" ?", false),
        option.bool("78304", "Hollowborn Thorn Guardian", "Mode: [select] only\nShould the bot buy \"Hollowborn Thorn Guardian\" ?", false),
        option.bool("78305", "Hollowborn Blade of Thorns", "Mode: [select] only\nShould the bot buy \"Hollowborn Blade of Thorns\" ?", false),
        option.bool("78306", "Hollowborn Daggers of Thorns", "Mode: [select] only\nShould the bot buy \"Hollowborn Daggers of Thorns\" ?", false),
        option.bool("78307", "Hollowborn Whip of Agony", "Mode: [select] only\nShould the bot buy \"Hollowborn Whip of Agony\" ?", false),
        option.bool("78308", "Hollowborn Poison Claws", "Mode: [select] only\nShould the bot buy \"Hollowborn Poison Claws\" ?", false),
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
                    Core.jumpWait();
                    break;

                case "Bone Dust":
                    Core.farmingLogger(req.name, quant);
                    Core.equipClass(ClassType.Farm);
                    while (!bot.shouldExit && !Core.checkInventory(req.name, quant))
                        Farm.battleUnderB(req.name, quant);
                    break;

                case "Death's Oversight":
                    Core.farmingLogger(req.name, quant);
                    Core.equipClass(ClassType.Solo);
                    while (!bot.shouldExit && !Core.checkInventory(req.name, quant))
                        Core.killMonster("shadowattack", "Boss", "Left", "Death", req.name, quant, { isTemp: false });
                    break;

                case "Death's Scythe":
                    Core.equipClass(ClassType.Solo);
                    Core.huntMonster("shadowattack", "Death", req.name, 1, { isTemp: false });
                    break;

                case "Unmoulded Fiend Essence":
                    Core.farmingLogger(req.name, quant);
                    Adv.buyItem("tercessuinotlim", 1951, "Unmoulded Fiend Essence", quant);
                    bot.wait.forPickup(req.name);
                    break;

                case "Elven Assassin's Locks + Scarf":
                case "Elven Assassin's Locks":
                case "Elven Assassin's Hair":
                case "Elven Assassin's Scarf":
                case "Poisonous Thorn Wreath":
                case "Nightshade Assassin Guardian":
                case "Reversed Blade of Thorns":
                case "Reversed Daggers of Thorns":
                case "Envenomed Whip of Agony":
                case "Envenomed Gauntlet":
                case "Poisonous Rogue":
                case "Nightshade Thorn Assasasin":
                    YUM.buyAllMerge(req.name);
                    bot.wait.forPickup(req.name);
                    break;

                case "Gold Voucher 25k":
                    Farm.voucher(req.name, req.quantity);
                    break;

                case "Vindicator Archer's Hat + Locks":
                case "Dawn Vindicator Archer":
                case "Dawn Vindicator General":
                case "Bright Bow of the Dawn":
                case "Vindicator General's Hood":
                case "Vindicator General's Hood + Locks":
                case "Blessed Shield of Vindication":
                case "Blessed Hammers of the Dawn":
                case "Blessed Hammer of the Dawn":
                case "Vindicator Archer's Hat":
                    DFM.buyAllMerge(req.name);
                    bot.wait.forPickup(req.name);
                    break;

                case "Gilded Scout's Quiver":
                case "Vindicator Scout's Bow":
                    Core.huntMonster("neofortress", "Vindicator Recruit", req.name, 1, { isTemp: false });

                    break;

                case "Blessed Rune of Vindication":
                case "Battlegear of Vindication":
                    Core.huntMonster("neofortress", "Vindicator General", req.name, 1, { isTemp: false });
                    bot.wait.forPickup(req.name);
                    break;
            }
        };

        //Only edit the map and shopID here
        Adv.startBuyAllMerge("shadowrealm", 1889, findIngredients, buyOnlyThis, { buyMode: buyMode });
    }
}

export const scriptOptions = defineScriptOptions(Adv.optionsStorage, [], true, {
    Generic: Adv.mergeOptions,
    Select: ShadowrealmMerge.select,
});

export default function main(): void {
        Core.bankingBlackList.push(...
            [
                "Hollow Soul",
                "Bone Dust",
                "Death's Oversight",
                "Death's Scythe",
                "Unmoulded Fiend Essence",
                "Poisonous Rogue",
                "Nightshade Thorn Assasasin",
                "Elven Assassin's Locks",
                "Elven Assassin's Locks + Scarf",
                "Elven Assassin's Hair",
                "Elven Assassin's Scarf",
                "Poisonous Thorn Wreath",
                "Nightshade Assassin Guardian",
                "Reversed Blade of Thorns",
                "Reversed Daggers of Thorns",
                "Envenomed Whip of Agony",
                "Envenomed Gauntlet",
                "Gold Voucher 25k",
                "Dawn Vindicator Archer",
                "Vindicator Archer's Hat + Locks",
                "Gilded Scout's Quiver",
                "Bright Bow of the Dawn",
                "Vindicator Scout's Bow",
                "Dawn Vindicator General",
                "Vindicator General's Hood",
                "Vindicator General's Hood + Locks",
                "Blessed Rune of Vindication",
                "Blessed Shield of Vindication",
                "Blessed Hammer of the Dawn",
                "Blessed Hammers of the Dawn",
                "Battlegear of Vindication",
            ]);
        Core.setOptions();

        new ShadowrealmMerge().buyAllMerge();
        Core.setOptions(false);
}
