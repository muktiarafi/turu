/*
name: Potions
description: Farms the Alchemy potions and tonics by crafting them from their reagents. Reagents
    are hunted, or bought with gold when BuyReagents is on.
tags: potions, alchemy, reputation, tonic, elixir
*/
// Port of Other/Various/Potions.cs (includes CoreBots, CoreFarms and CoreAdvanced).
//
// The options are declared in `scriptOptions` below (the C# `Options` list) and edited in
// Scripts > Edit Script Options. `bot.config.get` reads them.

import { bot, defineScriptOptions, option } from "turu";
import { ClassType, Core } from "../../core/CoreBots";
import { CoreAdvanced } from "../../core/CoreAdvanced";
import { AlchemyRunes, AlchemyTraits, CoreFarms } from "../../core/CoreFarms";

const Farm = new CoreFarms();
const Adv = new CoreAdvanced();

/** A boolean option, read when it's asked for (C# `Func<bool>` in the selection table). */
const flag = (name: string) => (): boolean => bot.config.get<boolean>(name);

/**
 * The script's options (C# `Options`), shown in Scripts > Edit Script Options. CoreBots'
 * SkipOptions isn't here: it only hides that window before the first run, and Turu never shows it
 * before a run.
 */
export const scriptOptions = defineScriptOptions("Potionv2", [
    option.bool("BuyReagents", "Buy Reagents?", "Use gold to buy the reagents for the Potions [this takes ALOT of gold.]."),
    option.int("PotionQuant", "Potion Quantity", "Desired stack amount [max - 300]", 0),
    option.bool("MaxAll", "Max all Potions within the script.", "as the name says"),

    // Tonic
    option.bool("FarmFate", "Fate", "Should the bot Farm Fate Tonics?"),
    option.bool("FarmSage", "Sage", "Should the bot Farm Sage Tonics?"),
    option.bool("FarmMight", "Might", "Should the bot Farm Might Tonics?"),
    option.bool("FarmFortitude", "Fortitude", "Should the bot Farm Fortitude Tonics?"),
    option.bool("FarmJudgment", "Judgment", "Should the bot Farm Judgment Tonics?"),

    // Elixir
    option.bool("FarmBattle", "Battle", "Should the bot Farm Battle Elixirs?"),
    option.bool("FarmMalevolence", "Malevolence", "Should the bot Farm Malevolence Elixirs?"),
    option.bool("FarmDivine", "Divine", "Should the bot Farm Unstable Divine Elixirs?"),
    option.bool("FarmRevitalize", "Revitalize", "Should the bot Farm Potent Revitalize Elixirs"),
    option.bool("FarmDestruction", "Destruction", "Should the bot Farm Potent Destruction Elixir?"),
    option.bool("FarmFelicitousPhiltre", "Felicitous Philtre", "Should the bot get Felicitous Philtre?"),
    option.bool("FarmEnduranceDraught", "Endurance Draught", "Should the bot get Endurance Draught?"),

    // Potion
    option.bool("FarmMalice", "Malice", "Should the bot Farm Malice Potions?"),
    option.bool("FarmHonor", "Honor", "Should the bot Farm Honor Potions?"),
    option.bool("FarmLife", "Life", "Should the bot Farm Potent Life Potion?"),
    option.bool("FarmBody", "Body", "Should the bot Farm Body Tonics?"),
    option.bool("FarmSoul", "Soul", "Should the bot Farm Soul Potions?"),
]);

/** Every potion the script farms, in C# order (`MaxAll` farms them all). */
const allPotions = [
    "Judgment Tonic",
    "Fortitude Tonic",
    "Fate Tonic",
    "Sage Tonic",
    "Potent Battle Elixir",
    "Potent Malevolence Elixir",
    "Potent Honor Potion",
    "Unstable Divine Elixir",
    "Potent Revitalize Elixir",
    "Endurance Draught",
    "Felicitous Philtre",
    "Potent Destruction Elixir",
    "Body Tonic",
    "Soul Potion",
    "Unstable Battle Elixir",
    "Unstable Body Tonic",
    "Unstable Fate Tonic",
    "Unstable Keen Elixir",
    "Unstable Mastery Tonic",
    "Unstable Might Tonic",
    "Unstable Wise Tonic",
    "Might Tonic",
    "Malice Potion",
    "Potent Life Potion",
];

/** Secondary potions, obtained alongside the normal versions: banked and added as a drop. */
const secondaryPotions = ["Potent Malice Potion", "Potent Soul Potion"];

/** Arguments of {@link needYourStrongestPotions} (the C# named parameters). */
export interface NeedYourStrongestPotionsArgs {
    /** The potions to farm, in order. Defaults to all of them. */
    potions?: string[];
    /** The desired stack amount, 1 to 300. Defaults to 300. */
    quant?: number;
    /** Buy the reagents with gold. Also on when the `BuyReagents` option is. */
    buyReagents?: boolean;
    /** Farm every potion in `potions`, whatever the options say. */
    separate?: boolean;
}

export default function main(): void {
    // Add items that should never be banked
    Core.bankingBlackList.push("Dragon Runestone", ...secondaryPotions);

    Core.setOptions();

    // With MaxAll the list is allPotions, the same as the default
    needYourStrongestPotions({
        quant: bot.config.get<boolean>("MaxAll") ? 300 : bot.config.get<number>("PotionQuant"),
        buyReagents: bot.config.get<boolean>("BuyReagents"),
    });

    Core.setOptions(false);
}

/**
 * Farms the potions (C# `INeedYourStrongestPotions`). A potion is farmed when its option is on,
 * with `MaxAll`, or with `separate`. Reagents are hunted, or bought when `buyReagents` is on.
 */
export function needYourStrongestPotions(args: NeedYourStrongestPotionsArgs = {}): void {
    const { potions = allPotions, separate = false } = args;
    let potionQuant = args.quant ?? 300;

    const buyReagents = bot.config.get<boolean>("BuyReagents") || (args.buyReagents ?? false);

    Farm.alchemyREP();
    Farm.goodREP();

    Core.logger(buyReagents ? "Method Choose: Buy Reagents" : "Farm Reagents");

    const maxAll = bot.config.get<boolean>("MaxAll");

    // Which potions the options select. The unstable ones only with MaxAll.
    const selection = new Map<string, () => boolean>([
        ["Judgment Tonic", flag("FarmJudgment")],
        ["Fortitude Tonic", flag("FarmFortitude")],
        ["Fate Tonic", flag("FarmFate")],
        ["Sage Tonic", flag("FarmSage")],

        ["Potent Battle Elixir", flag("FarmBattle")],
        ["Potent Malevolence Elixir", flag("FarmMalevolence")],
        ["Potent Honor Potion", flag("FarmHonor")],
        ["Unstable Divine Elixir", flag("FarmDivine")],
        ["Potent Revitalize Elixir", flag("FarmRevitalize")],

        ["Endurance Draught", flag("FarmEnduranceDraught")],
        ["Felicitous Philtre", flag("FarmFelicitousPhiltre")],

        ["Potent Destruction Elixir", flag("FarmDestruction")],
        ["Body Tonic", flag("FarmBody")],
        ["Soul Potion", flag("FarmSoul")],

        ["Unstable Battle Elixir", () => maxAll],
        ["Unstable Body Tonic", () => maxAll],
        ["Unstable Fate Tonic", () => maxAll],
        ["Unstable Keen Elixir", () => maxAll],
        ["Unstable Mastery Tonic", () => maxAll],
        ["Unstable Might Tonic", () => maxAll],
        ["Unstable Wise Tonic", () => maxAll],

        ["Might Tonic", flag("FarmMight")],
        ["Malice Potion", flag("FarmMalice")],
        ["Potent Life Potion", flag("FarmLife")],
    ]);

    const shouldFarmPotion = (potion: string): boolean => maxAll || separate || (selection.get(potion)?.() ?? false);

    if ((!separate && !maxAll && ![...selection.values()].some((select) => select())) || potionQuant < 1 || potionQuant > 300) {
        Core.logger("No Potions selected or invalid quantity. Stopping.", "", { messageBox: true, stopBot: true });
        return;
    }

    Core.addDrop(...potions);
    Core.toBank(...secondaryPotions);
    Core.addDrop(...secondaryPotions);

    Core.logger(`Selected: ${potions.filter(shouldFarmPotion).join(", ")}`);

    /** Gets one reagent, by hunting it or buying it (C# `GetIngredient`). */
    const getIngredient = (ingredient: string, ingreQuant = 30): void => {
        if (Core.checkInventory(ingredient, ingreQuant)) return;

        Core.equipClass(ClassType.Farm);
        Core.toggleAggro(false);

        switch (ingredient) {
            case "Ice Vapor":
                if (!buyReagents) Core.killMonster("lair", "Enter", "Spawn", "*", "Ice Vapor", 2, { isTemp: false, log: false });
                else Adv.buyItem("alchemyacademy", 397, 11478, ingreQuant, 1235);
                break;

            case "Moglin Tears":
                if (!Core.isMember && !buyReagents) Core.logger("Farming map is members only, buying the materials");

                if (!buyReagents && Core.isMember) Core.huntMonster("twig", "Sweetish Fish", ingredient, ingreQuant, { isTemp: false });
                else Adv.buyItem("alchemyacademy", 397, 11472, ingreQuant, 1229);
                break;

            case "Lemurphant Tears":
                if (!buyReagents) Core.huntMonster("ravinetemple", "Lemurphant", ingredient, ingreQuant, { isTemp: false });
                else Adv.buyItem("alchemyacademy", 397, 11479, ingreQuant, 1236);
                break;

            case "Dried Slime":
                if (!buyReagents) Core.huntMonster("orecavern", "Crashroom", ingredient, ingreQuant, { isTemp: false });
                else Adv.buyItem("alchemyacademy", 397, 11474, ingreQuant, 1231);
                break;

            case "Arashtite Ore":
                if (!buyReagents) Core.huntMonster("orecavern", "Deathmole", ingredient, ingreQuant, { isTemp: false });
                else Adv.buyItem("alchemyacademy", 397, 11473, ingreQuant, 1230);
                break;

            case "Chaos Entity":
                Adv.buyItem("alchemyacademy", 2114, 11482, ingreQuant, 9740);
                break;

            case "Fish Oil":
                Adv.buyItem("alchemyacademy", 397, 11467, ingreQuant, 1224);
                break;

            case "Doomatter":
                if (!buyReagents) {
                    if (bot.player.isMember) Core.huntMonster("Creepy", "Fear Feeder", ingredient, ingreQuant, { isTemp: false });
                    else Core.huntMonster("maul", "Creature Creation", ingredient, ingreQuant, { isTemp: false });
                } else {
                    Adv.buyItem("alchemyacademy", 397, 11477, ingreQuant, 1234);
                }
                break;

            case "Chaoroot":
                if (!buyReagents) Core.huntMonster("orecavern", "Naga Baas", ingredient, ingreQuant, { isTemp: false });
                else Adv.buyItem("tercessuinotlim", 1951, 11481, ingreQuant, 7911);
                break;

            case "Nimblestem":
                if (!buyReagents) Core.huntMonster("mudluk", "Swamp Frogdrake", "Nimblestem", ingreQuant, { isTemp: false });
                else Adv.buyItem("alchemyacademy", 397, 11469, ingreQuant, 1226);
                break;

            case "Trollola Nectar":
                if (!buyReagents) Core.huntMonster("bloodtusk", "Trollola Plant", ingredient, ingreQuant, { isTemp: false });
                else Adv.buyItem("alchemyacademy", 397, 11476, ingreQuant, 1233);
                break;

            case "Searbush":
                if (!buyReagents) Core.huntMonster("mafic", "Living Fire", ingredient, ingreQuant, { isTemp: false });
                else Adv.buyItem("alchemyacademy", 397, 11468, ingreQuant, 1225);
                break;

            case "Dragon Scale":
                Core.addDrop(11475);
                if (!buyReagents) {
                    while (!bot.shouldExit && !Core.checkInventory(11475, ingreQuant)) {
                        Core.killMonster("lair", "Hole", "Center", "*", null, 1, { isTemp: false, log: false });
                    }
                } else if (!Core.checkInventory(11475, ingreQuant)) {
                    Adv.buyItem("alchemyacademy", 397, 11475, ingreQuant, 1232);
                }
                break;

            case "Roc Tongue":
                if (!buyReagents) Core.huntMonster("roc", "Rock Roc", ingredient, ingreQuant, { isTemp: false, log: false });
                else Adv.buyItem("alchemyacademy", 397, 11471, ingreQuant, 1228);
                break;

            case "Necrot":
                if (!buyReagents) {
                    Core.huntMonster("deathsrealm", "Skeleton Fighter", ingredient, ingreQuant, { isTemp: false, log: false });
                } else {
                    Adv.buyItem("alchemyacademy", 397, 11480, ingreQuant, 1237);
                }
                break;

            case "Rhison Blood":
                if (!buyReagents) Core.huntMonster("bloodtusk", "Rhison", ingredient, ingreQuant, { isTemp: false, log: false });
                else Adv.buyItem("alchemyacademy", 397, 11470, ingreQuant, 1227);
                break;

            default:
                Core.logger(`Unknown ingredient: ${ingredient}`);
                break;
        }
    };

    for (const potion of potions) {
        const shouldFarm = shouldFarmPotion(potion);

        Core.logger(`${potion}: ${shouldFarm}`);

        if (!shouldFarm) continue;

        if (Core.checkInventory(potion, potionQuant)) continue;

        Core.farmingLogger(potion, potionQuant);

        let currTrait = AlchemyTraits.Int;

        /** Crafts `potion` from the two reagents until you have the quantity (C# `BulkGrind`). */
        const bulkGrind = (reagent1: string, reagent2: string, rune = AlchemyRunes.Gebo): void => {
            while (!bot.shouldExit && !Core.checkInventory(potion, potionQuant)) {
                getIngredient(reagent1);
                getIngredient(reagent2);

                Core.join("alchemy");
                Farm.alchemyPacket(reagent1, reagent2, rune, { trait: currTrait, item: potion, quant: potionQuant });
            }
        };

        switch (potion) {
            case "Fate Tonic":
            case "Sage Tonic":
                currTrait = potion === "Sage Tonic" ? AlchemyTraits.Int : AlchemyTraits.Luc;
                bulkGrind("Arashtite Ore", "Dried Slime");
                break;

            case "Potent Battle Elixir":
            case "Potent Malevolence Elixir":
                currTrait = potion === "Potent Malevolence Elixir" ? AlchemyTraits.SPw : AlchemyTraits.APw;
                bulkGrind("Doomatter", "Chaoroot");
                break;

            case "Potent Honor Potion":
            case "Malice Potion":
                currTrait = AlchemyTraits.Dam;
                bulkGrind("Chaoroot", "Chaos Entity");
                break;

            case "Potent Life Potion":
                currTrait = AlchemyTraits.Hea;
                bulkGrind("Dragon Scale", "Searbush");
                break;

            case "Might Tonic":
                currTrait = AlchemyTraits.Dam;
                bulkGrind("Chaos Entity", "Rhison Blood");
                break;

            case "Unstable Divine Elixir":
                if (potionQuant > 99) {
                    Core.logger(`Max quant for [${potion}] is [99] - Adjusting`);
                    potionQuant = 99;
                }
                currTrait = AlchemyTraits.hOu;
                bulkGrind("Dragon Scale", "Lemurphant Tears");
                break;

            case "Potent Revitalize Elixir":
                currTrait = AlchemyTraits.hRe;
                bulkGrind("Chaoroot", "Lemurphant Tears");
                break;

            case "Felicitous Philtre":
            case "Endurance Draught":
                Core.logger(`[${potion}] has no farm method, buying.`);
                Adv.buyItem("alchemyacademy", 2036, potion, potionQuant);
                break;

            case "Potent Destruction Elixir":
                currTrait = AlchemyTraits.mRe;
                bulkGrind("Dried Slime", "Arashtite Ore");
                break;

            case "Body Tonic":
                currTrait = AlchemyTraits.End;
                bulkGrind("Roc Tongue", "Chaoroot");
                break;

            case "Soul Potion":
                currTrait = AlchemyTraits.Dam;
                bulkGrind("Necrot", "Nimblestem");
                break;

            case "Unstable Battle Elixir":
                currTrait = AlchemyTraits.APw;
                bulkGrind("Doomatter", "Nimblestem");
                break;

            case "Unstable Body Tonic":
                currTrait = AlchemyTraits.End;
                bulkGrind("Nimblestem", "Roc Tongue");
                break;

            case "Unstable Fate Tonic":
                currTrait = AlchemyTraits.Luc;
                bulkGrind("Dried Slime", "Trollola Nectar");
                break;

            case "Unstable Keen Elixir":
                currTrait = AlchemyTraits.Cri;
                bulkGrind("Trollola Nectar", "Doomatter");
                break;

            case "Unstable Mastery Tonic":
                currTrait = AlchemyTraits.Dex;
                bulkGrind("Chaos Entity", "Dried Slime");
                break;

            case "Unstable Might Tonic":
                currTrait = AlchemyTraits.Str;
                bulkGrind("Chaos Entity", "Fish Oil");
                break;

            case "Unstable Wise Tonic":
                currTrait = AlchemyTraits.Wis;
                bulkGrind("Moglin Tears", "Rhison Blood");
                break;

            case "Judgment Tonic":
                currTrait = AlchemyTraits.Wis;
                bulkGrind("Dragon Scale", "Moglin Tears", AlchemyRunes.Jera);
                break;

            case "Fortitude Tonic":
                currTrait = AlchemyTraits.End;
                bulkGrind("Necrot", "Roc Tongue", AlchemyRunes.Fehu);
                break;

            default:
                Core.logger(`Unknown potion: ${potion}`);
                break;
        }
    }
}
