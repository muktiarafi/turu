/*
name: Streamwar Merge
description: This bot will farm the items belonging to the selected mode for the Streamwar Merge [2163] in /streamwar
tags: streamwar, merge, streamwar, bright, mana, flame, guard, banner, sabre, bow, green, dragon, slayer, slayers, sallet, winged, cloak, halberd, dark, a, fragment, beginning
*/
// Port of Scripts/ShadowsOfWar/MergeShops/StreamwarMerge.cs.

import { bot, defineScriptOptions, option } from "turu";
import { CoreAdvanced, mergeOptionsEnum } from "../../core/CoreAdvanced";
import { ClassType, Core } from "../../core/CoreBots";
import { CoreSoW } from "../../story/shadowsofwar/CoreSoW";
import { CoreSoWMats } from "../CoreSoWMats";

const Adv = new CoreAdvanced();
const SoW = new CoreSoW();
const SOWM = new CoreSoWMats();

export class StreamwarMerge {
    static readonly select = [
        option.bool("71904", "Bright Mana Flame", "Mode: [select] only\nShould the bot buy \"Bright Mana Flame\" ?", false),
        option.bool("71899", "Bright Mana Flame Guard", "Mode: [select] only\nShould the bot buy \"Bright Mana Flame Guard\" ?", false),
        option.bool("71900", "Bright Mana Flame Spikes", "Mode: [select] only\nShould the bot buy \"Bright Mana Flame Spikes\" ?", false),
        option.bool("71901", "Bright Mana Flame Banner", "Mode: [select] only\nShould the bot buy \"Bright Mana Flame Banner\" ?", false),
        option.bool("71902", "Bright Mana Flame Sabre", "Mode: [select] only\nShould the bot buy \"Bright Mana Flame Sabre\" ?", false),
        option.bool("71903", "Bright Mana Flame Bow", "Mode: [select] only\nShould the bot buy \"Bright Mana Flame Bow\" ?", false),
        option.bool("71827", "Green Dragon Slayer", "Mode: [select] only\nShould the bot buy \"Green Dragon Slayer\" ?", false),
        option.bool("71828", "Green Dragon Slayer's Sallet", "Mode: [select] only\nShould the bot buy \"Green Dragon Slayer's Sallet\" ?", false),
        option.bool("71829", "Green Dragon Slayer's Winged Sallet", "Mode: [select] only\nShould the bot buy \"Green Dragon Slayer's Winged Sallet\" ?", false),
        option.bool("71830", "Green Dragon Slayer's Cloak", "Mode: [select] only\nShould the bot buy \"Green Dragon Slayer's Cloak\" ?", false),
        option.bool("71831", "Green Dragon Slayer's Halberd", "Mode: [select] only\nShould the bot buy \"Green Dragon Slayer's Halberd\" ?", false),
        option.bool("71905", "Dark Dragon Slayer", "Mode: [select] only\nShould the bot buy \"Dark Dragon Slayer\" ?", false),
        option.bool("71906", "Dark Dragon Slayer's Sallet", "Mode: [select] only\nShould the bot buy \"Dark Dragon Slayer's Sallet\" ?", false),
        option.bool("71907", "Dark Dragon Slayer's Winged Sallet", "Mode: [select] only\nShould the bot buy \"Dark Dragon Slayer's Winged Sallet\" ?", false),
        option.bool("71908", "Dark Dragon Slayer's Cloak", "Mode: [select] only\nShould the bot buy \"Dark Dragon Slayer's Cloak\" ?", false),
        option.bool("71909", "Dark Dragon Slayer's Halberd", "Mode: [select] only\nShould the bot buy \"Dark Dragon Slayer's Halberd\" ?", false),
        option.bool("73356", "A Fragment of the Beginning", "Mode: [select] only\nShould the bot buy \"A Fragment of the Beginning\" ?", false),
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

                case "Avatar's Flame Bow":
                case "Avatar's Flame Spikes":
                case "Avatar's Flame Banners":
                case "Avatar's Flame Sabre":
                case "Avatar's Flame":
                case "Avatar's Flame Guard":
                    Core.farmingLogger(req.name, quant);
                    Core.equipClass(ClassType.Solo);
                    while (!bot.shouldExit && !Core.checkInventory(req.name, quant)) {
                        Core.huntMonster("Streamwar", "Second Speaker", req.name, 1, { isTemp: false, log: false });
                        bot.wait.forPickup(req.name);
                    }
                    Core.cancelRegisteredQuests();
                    break;

                case "Willpower":
                    SOWM.willpower(quant);
                    break;

                case "Garish Remnant":
                    SOWM.garishRemnant(quant);
                    break;

                case "Prismatic Seams":
                    SOWM.prismaticSeams(quant);
                    break;
            }
        };

        SoW.completeCoreSoW();
        //Only edit the map and shopID here
        Adv.startBuyAllMerge("streamwar", 2163, findIngredients, buyOnlyThis, { buyMode: buyMode });
    }
}

export const scriptOptions = defineScriptOptions(Adv.optionsStorage, [], true, {
    Generic: Adv.mergeOptions,
    Select: StreamwarMerge.select,
});

export default function main(): void {
        Core.bankingBlackList.push(...
            [
                "Avatar's Flame",
                "Prismatic Seams",
                "Avatar's Flame Guard",
                "Avatar's Flame Spikes",
                "Avatar's Flame Banners",
                "Avatar's Flame Sabre",
                "Willpower",
                "Garish Remnant",
                "Avatar's Flame Bow",
                "Unbound Thread",
            ]);
        Core.setOptions();

        new StreamwarMerge().buyAllMerge();

        Core.setOptions(false);
}
