/*
name: null
description: null
tags: null
*/
// Port of Scripts/Other/FarmJoeKits/CoreFarmerJoe.cs: the level 1 to 100 progression that
// 0FarmerJoeDoAll.ts runs. Every class, weapon and story it calls is ported in this tree.
// Not ported: DmgOverTimeEnh (nothing calls it). The C# `ScriptMain` only runs `Core.RunCore()`,
// so this file has no `main`; run 0FarmerJoeDoAll.ts.

import { bot, choices, defineScriptOptions, option } from "turu";
import type { InventoryItem } from "turu";
import { CoreAdvanced, GenericGearBoost } from "../../core/CoreAdvanced";
import { ClassType, Core, configOr } from "../../core/CoreBots";
import { CoreDailies } from "../../core/CoreDailies";
import { CoreFarms } from "../../core/CoreFarms";
import { Cryomancer } from "../../dailies/Cryomancer";
import { LordOfOrder } from "../../dailies/LordOfOrder";
import { UnlockForgeEnhancements } from "../../enhancement/UnlockForgeEnhancements";
import { ArchPaladin } from "../../good/ArchPaladin";
import { CapeOfAwe } from "../../good/gearofawe/CapeOfAwe";
import { ShadowrealmMerge } from "../../hollowborn/mergeshops/ShadowrealmMerge";
import { CoreYnR } from "../../legion/yaminoronin/CoreYnR";
import { CoreNation } from "../../nation/CoreNation";
import { EnoughDOOMforanArchfiend } from "../../nation/afdl/EnoughDOOMforanArchfiend";
import { ArchFiend } from "../../nation/various/Archfiend";
import { CoreVHL } from "../../nation/vhl/CoreVHL";
import { NorthlandsMonk } from "../../seasonal/frostvale/NorthlandsMonk";
import { Mazumi } from "../../story/Mazumi";
import { Tutorial } from "../../story/Tutorial";
import { Core13LoC } from "../../story/lordsofchaos/Core13LoC";
import { CelestialArenaQuests } from "../../story/queenofmonsters/extra/CelestialArena";
import { DragonOfTime } from "../classes/DragonOfTime";
import { DragonShinobi } from "../classes/DragonShinobi";
import { Dragonslayer } from "../classes/Dragonslayer";
import { DragonslayerGeneral } from "../classes/DragonslayerGeneral";
import { FrostSpiritReaver } from "../classes/FrostSpiritReaver";
import { KingsEcho } from "../classes/KingsEcho";
import { ScarletSorceress } from "../classes/ScarletSorceress";
import { BlazeBinder } from "../classes/daily-classes/BlazeBinder";
import { GlacialBerserker } from "../classes/rep-based/GlacialBerserker";
import { MasterRanger } from "../classes/rep-based/MasterRanger";
import { Shaman } from "../classes/rep-based/Shaman";
import { FreeBoosts } from "../freeboosts/FreeBoostsQuest(10mns)[Rng]";
import { SynderesMerge } from "../mergeshops/SynderesMerge";
import { BurningBlade } from "../weapons/BurningBlade";
import { BurningBladeOfAbezeth } from "../weapons/BurningBladeOfAbezeth";
import { ExaltedApotheosisPreReqs } from "../weapons/ExaltedApotheosisPreReqs";

//#region dependicies
//other
const Boosts = new FreeBoosts();
const SM = new SynderesMerge();
const unlockForgeEnhancements = new UnlockForgeEnhancements();
const exaltedApotheosisPreReqs = new ExaltedApotheosisPreReqs();
const enoughDOOMforanArchfiend = new EnoughDOOMforanArchfiend();

//Cores
const Adv = new CoreAdvanced();
const Farm = new CoreFarms();
const COA = new CapeOfAwe();
const Daily = new CoreDailies();
const VHL = new CoreVHL();
const Nation = new CoreNation();
const YNR = new CoreYnR();

//Classes
const MR = new MasterRanger();
const shaman = new Shaman();
const GB = new GlacialBerserker();
const DS = new DragonShinobi();
const AP = new ArchPaladin();
const DSlayer = new Dragonslayer();
const DSG = new DragonslayerGeneral();
const LOO = new LordOfOrder();
const SS = new ScarletSorceress();
const DoT = new DragonOfTime();
const Bb = new BlazeBinder();
const AF = new ArchFiend();
const Cryo = new Cryomancer();
const FSR = new FrostSpiritReaver();
const NM = new NorthlandsMonk();

//Weapons
const BB = new BurningBlade();
const BBOA = new BurningBladeOfAbezeth();
const SRM = new ShadowrealmMerge();

//Story
const tutorial = new Tutorial();
const CAQ = new CelestialArenaQuests();
const mazumi = new Mazumi();
const KE = new KingsEcho();
//#endregion

const RANK_10_CLASS_POINTS = 302500;

export enum PetChoice {
    None = "None",
    HotMama = "HotMama",
    Akriloth = "Akriloth",
}

/** CoreFarmerJoe's options (C# `OptionsStorage` and `Options`); 0FarmerJoeDoAll.ts exports them. */
export const scriptOptions = defineScriptOptions(
    "FarmerJoePet",
    [
        option.bool("OutFit", "Get a Pre-Made Outfit, Curtious of the Community", "We are farmers, bum ba dum bum bum bum bum", false),
        option.bool("EquipOutfit", "Equip our the FarmerJoe outfit at the end?", "Yay or Nay", false),
        option.bool("SellStarterClasses", "SellStarterClasses", "Yay or Nay", false),
        option.bool(
            "GetBoosts",
            "GetBoosts",
            'Do the "Free Boosts" quests to get some 10minute boosts (the droprate is < 1% so itll take awhile)',
            false,
        ),
        // option.bool("EquipBoostingGear", "EquipBoostingGear", "use a predetermined set of gear based on what you own, and certain paramiters set, otherwise use whats set for your solo/farm class gear in CBO", true),
        option.choice(
            "PetChoice",
            "Choose Your Pet",
            "Extra stuff to choose, if you have any suggestions -form in disc, and put it under request. or dm Tato(the retarded one on disc)",
            choices(PetChoice),
            PetChoice.None,
        ),
        Core.skipOptions,
    ],
    true,
);

export class CoreFarmerJoe {
    // C#: `public static Core13LoC LOC => new();` makes a new one on each use.
    get LOC(): Core13LoC {
        return new Core13LoC();
    }

    //#region InvClasses

    private readonly _classCache = new Map<string, InventoryItem | null>();

    private getClass(className: string): InventoryItem | null {
        const key = className.toLowerCase();
        if (this._classCache.has(key))
            return this._classCache.get(key) ?? null;

        const item =
            bot.inventory.items
                .concat(bot.bank.items)
                .find(i => i.name?.trim().toLowerCase() === key && i.category === "Class") ?? null;

        this._classCache.set(key, item); // cache result
        return item;
    }

    // Strongly-typed properties
    private get classNinja(): InventoryItem | null { return this.getClass("Ninja"); }
    private get classRogue(): InventoryItem | null { return this.getClass("Rogue"); }
    private get classMage(): InventoryItem | null { return this.getClass("Mage"); }
    private get classMasterRanger(): InventoryItem | null { return this.getClass("Master Ranger"); }
    private get classScarletSorceress(): InventoryItem | null { return this.getClass("Scarlet Sorceress"); }
    private get classBlazeBinder(): InventoryItem | null { return this.getClass("Blaze Binder"); }
    private get classDragonSoulShinobi(): InventoryItem | null { return this.getClass("DragonSoul Shinobi"); }
    private get classArchPaladin(): InventoryItem | null { return this.getClass("ArchPaladin"); }
    private get classArchFiend(): InventoryItem | null { return this.getClass("ArchFiend"); }
    private get classGlacialBerserker(): InventoryItem | null { return this.getClass("Glacial Berserker"); }
    private get classCryomancer(): InventoryItem | null { return this.getClass("Cryomancer"); }
    private get classDragonOfTime(): InventoryItem | null { return this.getClass("Dragon of Time"); }
    private get classDragonslayer(): InventoryItem | null { return this.getClass("Dragonslayer"); }
    private get classDragonslayerGeneral(): InventoryItem | null { return this.getClass("Dragonslayer General"); }

    //#endregion InvClasses

    // Class lists for solo and farm activities
    soloClasses = [
        "King's Echo",
        "Chaos Avenger",
        "Void Highlord",
        "Void Highlord (IoDA)",
        "Legion Revenant",
        "Dragon of Time",
        "ArchPaladin",
        "Lord of Order",
        "Cryomancer",
        "Dragonslayer General",
        "Glacial Berserker",
        "Dragonslayer",
        "DragonSoul Shinobi",
        "Assassin",
        "Ninja Warrior",
        "Classic Ninja",
        "Ninja",
        "Ninja (Rare)",
        "Rogue (Rare)",
        "Rogue",
        "Healer (Rare)",
        "Healer",
    ];

    farmClasses = [
        "Legion Revenant",
        "ArchMage",
        "Dragon of Time",
        "Archfiend",
        "Blaze Binder",
        "Scarlet Sorceress",
        "Pyromancer",
        "Master Ranger",
        "Mage (Rare)",
        "Mage",
    ];

    dodgeClasses = [
        "Great Thief",
        "Yami No Ronin",
        "Chrono Assassin",
        "Horc Evader",
        "Assassin",
        "Ninja Warrior",
        "Classic Ninja",
        "Ninja",
        "Ninja (Rare)",
        "Rogue (Rare)",
        "Rogue",
    ];

    /**
     * Orchestrates the entire progression process from level 1 to endgame, including:
     * - **Leveling:** Progresses through levels 1 to 100 with specific milestones and enhancements.
     * - **Item Acquisition:** Handles the acquisition of necessary items and classes at each stage.
     * - **Outfit Setup:** Configures the character's outfit and equipment.
     * - **Pet Management:** Acquires and equips specified pets.
     */
    doAll(): void {
        Adv.gearStore(undefined, { enhAfter: true });
        this.level1to30();
        this.level30to75();
        this.level75to100();
        this.endGame();
        this.outfit();
        this.pets(PetChoice.HotMama);
        this.pets(PetChoice.Akriloth);
        Adv.gearStore(true, { enhAfter: true });
    }

    /**
     * Guides the character from level 1 to 30, focusing on acquiring essential beginner items and class enhancements.
     * - **Beginner Items:** Ensures the character is equipped with necessary beginner gear and classes.
     * - **Desync Warning:** Provides a safety note about potential desynchronization issues with class points at Rank 9, advising a relog if necessary.
     */
    level1to30(): void {
        // Beginner Items
        this.beginnerItems();

        //safety incase it desyncs.. the relog fuction isnt exactly perfect
        Core.logger("Class points may be desynced at Rank 9, if you are stuck at rank 9, please stop the bot & relog if this happens");
    }

    hasEnhancedThisBracket = false;

    //#region Leve30 to 75
    /**
     * Progresses the character from level 30 to 75 by acquiring essential items and enhancing classes. This includes:
     * - **Item Acquisition:** Obtaining items like "Awethur's Accoutrements," "Master Ranger," "Burning Blade", "Scarlet Sorceress," "Blaze Binder," "DragonSoul Shinobi," "ArchPaladin," and "ArchFiend DeathLord."
     * - **Class Enhancements:** Ensuring classes are ranked up and equipped as needed.
     * - **Experience Farming:** Efficiently farming experience for each level milestone (30, 50, 55, 60, 65, 70, and 75).
     */
    level30to75(): void {
        //Preset Solo & FarmClass (required if additional Classes were pre-aquired before the script or your restarting it and CBO wasnt saved.)
        this.setClass();
        // Check for Elders' Blood daily if player is level 30 or higher
        if (bot.player.level >= 30 && Daily.checkDailyv2(802, ["Elders' Blood"], { any: true, shouldUnBank: true }))
            Daily.eldersBlood();

        Core.logger(
            "Joe will handle each level bracket (0-30, 30-50, 50-55, 55-60, 60-65, 65-75, 75-100)\n doing the leveling first, then acquiring classes and items after\n so its prepared... and hopefully strong enough to handle the next level bracket.",
        );

        if (configOr("GetBoosts", false)) {
            Core.oneTimeMessage(
                "GetBoosts Message",
                "We'll occasionaly get rep/class/gold boosts throughout the script to help speed things up a bit (the quest's droprate is low, but fast kills - you can disable this in scripts button > edit options ( when script is stopped).)",
            );
            Core.logger(
                "GetBoosts is TURNED ON - READ THIS MESSAGE " +
                    "\n\nYour character will now start FISHING for many HOURS in order to GET the BOOSTS that you turned on in script options. " +
                    "\n\nIF YOU REPORT IN #skua-help WHY YOUR JOE STARTED FISHING YOU WILL GET SHAMED " +
                    "\n\nYes this message will pop up everytime you start this script if you have the GetBoosts option turned on. " +
                    "\nNo you cannot turn it off if you have the GetBoosts option turned on. " +
                    "\n\nEnjoy fishing :)",
                "",
                { messageBox: true },
            );
            // Always ensure we have 10 of each boost type
            Farm.getBoost("REP", 10, true);
            Farm.getBoost("XP", bot.player.level >= 100 ? 0 : 10, true);
            Boosts.getBoostsSelect(10, 10, 0);
        }

        const levelHandlers = new Map<number, () => void>([
            [30, () => this.handleLevel30()],
            [50, () => this.handleLevel50()],
            [55, () => this.handleLevel55()],
            [60, () => this.handleLevel60()],
            [65, () => this.handleLevel65()],
            [70, () => this.handleLevel70()],
            [75, () => this.handleLevel75()],
            [80, () => this.handlelevel80()],
        ]);

        for (const level of [30, 50, 55, 60, 65, 70, 75, 80]) {
            if (bot.player.level > 75)
                this.hasEnhancedThisBracket = true;

            // // Smart enhance at start of each bracket
            if (!this.hasEnhancedThisBracket || bot.inventory.items.some(x => x != null && x.enhancementLevel < bot.player.level)) {
                Adv.smartEnhance(bot.player.currentClass ?? "");
                this.hasEnhancedThisBracket = true;
            }

            // Level up to target
            Farm.experience(level);

            // Execute level-specific handler if it exists
            const handler = levelHandlers.get(level);
            if (handler) {
                bot.log(`Level Handler: ${level} bracket`);
                handler();
                CoreFarmerJoe.bankAllUnusedClasses();
            }
            // Reset enhancement flag for next bracket
            this.hasEnhancedThisBracket = false;
        }
    }

    private static anyRank10(names: string[]): boolean {
        for (const name of names) {
            const item = bot.inventory.getItem(name);
            if (item?.category === "Class" && item.quantity >= RANK_10_CLASS_POINTS) {
                return true;
            }
        }
        return false;
    }

    private handleLevel30(): void {
        if (!Core.checkInventory("Master Ranger", 1, { toInv: false }) || !CoreFarmerJoe.anyRank10(["Master Ranger"])) {
            if (Core.checkInventory("Venom Head"))
                Core.sellItem("Venom Head");

            Core.logger("Level 30: Acquiring Master Ranger");
            this.setClass();
            MR.getMR();
        }

        if (!Core.checkInventory("Dragonslayer", 1, { toInv: false }) || !CoreFarmerJoe.anyRank10(["Dragonslayer"])) {
            Core.logger("Level 30: Acquiring Dragonslayer");
            this.setClass();
            DSlayer.getDragonslayer();
        }

        // --- BLADE OF AWE PROGRESSION ---
        const _aweItems = ["Burning Blade of Abezeth", "Burning Blade", "Awethur's Accoutrements"];

        Core.logger("Level 30: Farming Blade of Awe Rep for enhancements & sword");
        this.setClass();
        Farm.bladeofAweREP();
        Adv.buyItem("museum", 631, "Awethur's Accoutrements");
        unlockForgeEnhancements.forgeHelmEnhancement();
    }

    private handleLevel50(): void {
        // --- SCARLET SORCERESS / FARM ALTERNATIVES ---
        if (!Core.checkInventory("Scarlet Sorceress", 1, { toInv: false }) || !CoreFarmerJoe.anyRank10(["Scarlet Sorceress"])) {
            Core.logger("Level 50: Acquiring Scarlet Sorceress");
            this.setClass();
            SS.getSSorc();
        }

        if (!Core.checkInventory("Dragonslayer General", 1, { toInv: false }) || !CoreFarmerJoe.anyRank10(["Dragonslayer General"])) {
            Core.logger("Level 50: Acquiring Dragonslayer General");
            this.setClass();
            DSG.getDSGeneral();
        }

        Core.logger("Level 50: Acquiring Burning Blade");
        this.setClass();
        BB.getBurningBlade();
    }

    private handleLevel55(): void {
        // Blaze Binder
        if (!Core.checkInventory("Blaze Binder", 1, { toInv: false }) || !CoreFarmerJoe.anyRank10(["Blaze Binder"])) {
            Core.logger("Level 55: Acquiring Blaze Binder");
            this.setClass();
            Bb.getClass();
        }

        // Cryomancer
        if (!Core.checkInventory("Cryomancer", 1, { toInv: false }) || !CoreFarmerJoe.anyRank10(["Cryomancer"])) {
            Core.logger("Level 55: Acquiring Cryomancer");
            this.setClass();
            Cryo.doCryomancer();
        }
    }

    private handleLevel60(): void {
        if (!Core.checkInventory("DragonSoul Shinobi", 1, { toInv: false }) || !CoreFarmerJoe.anyRank10(["DragonSoul Shinobi"])) {
            Core.logger("Level 60: Acquiring DragonSoul Shinobi");
            this.setClass();
            DS.getDSS();
        }

        unlockForgeEnhancements.lacerate();
    }

    private handleLevel65(): void {
        if (!Core.checkInventory("Glacial Berserker", 1, { toInv: false }) || !CoreFarmerJoe.anyRank10(["Glacial Berserker"])) {
            Core.logger("Level 65: Acquiring Glacial Berserker");
            this.setClass();
            GB.getGB();
        }
    }

    private handleLevel70(): void {
        // Horc Evader
        if (!Core.checkInventory("Horc Evader", 1, { toInv: false }) || !CoreFarmerJoe.anyRank10(["Horc Evader"])) {
            Core.logger("Geting Horc Evader so we can attemp to kill Ultra Chaos Alteon during the story to get AP");
            Adv.buyItem("bloodtusk", 308, "Horc Evader");
            this.setClass();
        }

        Core.logger("unlocking the easy lvl 70 forge enhs");
        unlockForgeEnhancements.smite();
        unlockForgeEnhancements.vim();
        unlockForgeEnhancements.pneuma();
        unlockForgeEnhancements.examen();
        unlockForgeEnhancements.anima();

        // ArchPaladin
        if (!Core.checkInventory("ArchPaladin", 1, { toInv: false }) || !CoreFarmerJoe.anyRank10(["ArchPaladin"])) {
            Core.logger("Level 70: Acquiring ArchPaladin");
            this.setClass();
            AP.getAP();
        }
    }

    private handleLevel75(): void {
        // Archfiend DoomLord (+30 dmgAll)
        if (
            !Core.checkInventory("Archfiend DoomLord", 1, { toInv: false }) ||
            !CoreFarmerJoe.anyRank10(["Archfiend"]) ||
            !Adv.hasMinimalBoost(GenericGearBoost.dmgAll, 30)
        ) {
            Core.logger(
                "Level 75: Acquiring Archfiend DoomLord for 35% more damage to any monster, and gives 25% more class points, XP, gold, and rep when equipped",
            );
            this.setClass();
            enoughDOOMforanArchfiend.afdl();
        }

        // Archfiend
        if (!Core.checkInventory("Archfiend", 1, { toInv: false }) || !CoreFarmerJoe.anyRank10(["Archfiend"])) {
            Core.logger("Level 75: Acquiring Archfiend");
            this.setClass();
            AF.getArchfiend();
        }
    }

    private handlelevel80(): void {
        if (Core.checkInventory(["Void Highlord", "Void Highlord (IoDA)"], 1, { any: true })) {
            Core.logger("Dragon of Time found but not rank 10 - ranking up");
            this.setClass();
            return;
        }
        Core.logger(
            "Bracket level [80]; Attemping to aquire VHL (Elders' bloods are a daily, this will take a total of 17 days ( minus your current roents/elders' quantity))",
        );
        VHL.getVHL();
    }

    //#endregion Leve 30-75

    /**
     * Advances the character from level 75 to 100.
     *
     * Phase 1: Prepare healer class (Dragon of Time or Healer) for 13 LoC
     * Phase 2: Acquire Enchanted Cape of Awe and complete 13 LoC
     * Phase 3: Get Lord of Order daily, acquire additional farming classes (FSR, Northlands Monk, Shaman),
     *          and unlock helmet + weapon enhancements
     * Phase 4: Level to 80, complete Celestial Arena QuestLine for BBoA, unlock cape enhancements,
     *          acquire YnR and Dragon of Time, level to 100, and obtain Hollowborn Reaper's Scythe
     */
    level75to100(): void {
        // --- Phase 1: Healer / Dragon of Time ---
        Core.logger("Phase 1: Class Preparation - Healer/Dragon of Time for 13 LoC");

        if (Core.checkInventory("Dragon of Time", 1, { toInv: false })) {
            const dragonOfTime = this.classDragonOfTime;
            if (dragonOfTime != null && dragonOfTime.quantity < RANK_10_CLASS_POINTS) {
                Core.logger("Dragon of Time found but not rank 10 - ranking up");
                this.setClass();
            }
        } else {
            const healer = Core.checkInventory("Healer (Rare)") ? "Healer (Rare)" : "Healer";
            if (!Core.checkInventory(["Healer", "Healer (Rare)"], 1, { any: true })) {
                Core.logger(
                    "No healing class found! Acquiring the `Healer` class to do `Xang` later during the 13LoC story quests ( as healing classes basicly NUKE her....).",
                );
                Adv.buyItem("classhalla", 176, "Healer");
                Adv.rankUpClass(healer);
            }
        }

        // --- Phase 2: Chaos quests & enhancements ---
        Core.logger("Phase 2: Chaos Shenanigans - CoA & 13 LoC");
        COA.getCoA();
        Core.equip("Cape of Awe");
        this.setClass();
        this.LOC.complete13LOC();

        // --- Phase 3: Solo Classes & Weapons ---
        Core.logger("Phase 3: Solo Classes & Weapon - LoO Daily");
        LOO.getLoO();
        this.setClass();

        // Bank any LoO rewards
        Core.toBank(...Core.ensureLoad(7156).rewards.map(i => i.name));

        const additionalClasses = ["Frost Spirit Reaver", "Northlands Monk", "Shaman"];

        const getters: (() => void)[] = [() => FSR.getFSR(), () => NM.getNlMonk(), () => shaman.getShaman()];

        for (let i = 0; i < additionalClasses.length; i++) {
            const className = additionalClasses[i];

            const hasClass = Core.checkInventory(className, 1, { toInv: false });

            // If player doesn't own the class, try to get it ONCE
            if (!hasClass) {
                Core.logger(`${className} not owned — attempting acquisition.`);
                this.setClass();
                getters[i]();

                // Recheck after attempt. If still missing, skip (seasonal / unavailable)
                if (!Core.checkInventory(className, 1, { toInv: false })) {
                    Core.logger(`${className} unavailable — skipping.`);
                    continue;
                }
            }

            // At this point the class exists → now ensure rank 10
            if (!CoreFarmerJoe.anyRank10([className])) {
                Core.logger(`Ranking ${className} to Rank 10.`);
                this.setClass();
                getters[i]();
            }
        }

        // Unlock Helmet Enhancements
        Core.logger("Phase 3: Unlocking Helmet Enhancements");

        // Unlock Weapon Enhancements
        Core.logger("Phase 3: Unlocking Weapon Enhancements");
        unlockForgeEnhancements.forgeWeaponEnhancement();
        unlockForgeEnhancements.praxis();

        // --- Phase 4: Leveling & Endgame ---
        Core.logger("Phase 4: Leveling to 80");
        Farm.experience(80);

        Core.logger("Phase 4: Celestial Arena QuestLine for Blinding Bright of Awe (BBoA)");
        this.setClass();
        CAQ.doAll();
        BBOA.getBBoA();

        // Unlock Cape Enhancements
        Core.logger("Phase 4: Unlocking Cape Enhancements");
        unlockForgeEnhancements.forgeCapeEnhancement();
        unlockForgeEnhancements.absolution();
        unlockForgeEnhancements.vainglory();
        unlockForgeEnhancements.avarice();
        unlockForgeEnhancements.lament();

        // Acquire Yin & Yang Roentgenium
        Core.logger("Phase 4: Acquiring Yami no Ronin (YnR)");
        YNR.getYnR();

        // Acquire Dragon of Time (ensure class is set)
        Core.logger("Phase 4: Acquiring Dragon of Time");
        this.setClass();
        DoT.getDoT();

        // Final leveling to 100
        Core.logger("Phase 4: Final Leveling to 100");
        this.setClass();
        Farm.experience();

        // Get King's Echo
        this.setClass();
        KE.getKE();

        // Acquire Hollowborn Reaper's Scythe
        Core.logger("Phase 4: Acquiring Hollowborn Reaper's Scythe");
        SRM.buyAllMerge("Hollowborn Reaper's Scythe");

        Core.logger("Level 75 to 100 progression complete!");
    }

    /**
     * Completes endgame preparations and tasks, including:
     * - **Outfit Setup:** Configuring the character's outfit if specified.
     * - **Class Setup:** Ensuring the appropriate class is set.
     * - **Item Preparation:** Pre-farming and enhancing key items like "Hero's Valiance," "Elysium," "Arcana's Concerto," "Ravenous," and "DauntLess."
     * - **Apotheosis:** Completing prerequisites for the Exalted Apotheosis.
     */
    endGame(): void {
        //#region Ending & Extras

        if (configOr("OutFit", false))
            this.outfit();

        this.setClass();

        //#region  Prefarm some non-Skua-able items:

        //Prep for remaning Enh:
        Core.logger("P5: Preparing for Remaining Enhancements: Heros Valiance, Elysium, Arcanas Concerto, Ravenous, DauntLess");
        unlockForgeEnhancements.herosValiance();
        if (Core.checkInventory("The Divine Will"))
            unlockForgeEnhancements.elysium();
        // unlockForgeEnhancements.arcanasConcerto();
        // unlockForgeEnhancements.dauntLess();
        if (Core.checkInventory("Void Highlord") && Core.checkInventory("Roentgenium of Nulgath", 10))
            unlockForgeEnhancements.ravenous();

        // Apotheosis:
        Core.logger("P5: Apotheosis prereqs");
        exaltedApotheosisPreReqs.preReqs();

        // Max nulgath materials

        Core.logger("P6: Max nation materials via Supplies");
        Nation.supplies();

        //#endregion Prefarm some non-Skua-able items:

        //#endregion Ending & Extras
    }

    /**
     * Manages the setup of a character's outfit by handling class configuration, item acquisition, and equipment. This includes:
     * - **Class Setup:** Configuring the character's class.
     * - **Basic Equipment:** Equipping items like shirts, hats, and enhancing the current class.
     * - **Additional Setup:** Handling pets and equipping the complete outfit if specified in the configuration.
     */
    outfit(): void {
        this.setClass();

        // Easy Difficulty Stuff
        CoreFarmerJoe.shirtAndHat();
        this.serversAreDown();
        Adv.smartEnhance(bot.player.currentClass ?? "");

        // Extra Stuff
        this.pets();

        if (configOr("EquipOutfit", false)) {
            Core.equip("NO BOTS Armor", "Scarecrow Hat", "The Server is Down", "Hollowborn Reaper's Scythe");
            Core.equip(configOr("PetChoice", PetChoice.None));
        }

        Core.logger("We are farmers, bum ba dum bum bum bum bum");
    }

    //#region other stuff

    //#region Extra:
    /**
     * Manages the acquisition and equipping of pets based on the user's configuration or specified choice.
     * - **Config-Based Acquisition:** Checks the configuration for the selected pet and acquires it if not already owned.
     * - **Specific Pets:**
     *   - **Hot Mama:** Acquires and equips the "Hot Mama" pet if selected and not already in inventory.
     *   - **Akriloth Pet:** Acquires and equips the "Akriloth Pet" if selected and not already in inventory.
     * @param petChoice Selected pet choice
     */
    pets(_petChoice: PetChoice = PetChoice.None): void {
        // C# reads the option "Pets", which no script declares (the option is "PetChoice"), so
        // this is always None and returns: kept as is.
        const configPetChoice = configOr("Pets", PetChoice.None);

        if (configPetChoice === PetChoice.None)
            return;

        if (configPetChoice === PetChoice.HotMama && !Core.checkInventory("Hot Mama")) {
            this.setClass();
            Core.huntMonster("battleundere", "Hot Mama", "Hot Mama", 1, { isTemp: false, log: false });
            bot.wait.forPickup("Hot Mama");
            Core.equip("Hot Mama");
        }

        if (configPetChoice === PetChoice.Akriloth && !Core.checkInventory("Akriloth Pet")) {
            this.setClass();
            Core.huntMonster("gravestrike", "Ultra Akriloth", "Akriloth Pet", 1, { isTemp: false, log: false });
            bot.wait.forPickup("Akriloth Pet");
            Core.equip("Akriloth Pet");
        }
    }

    /**
     * Acquires and equips a shirt and hat for the character.
     * - **NO BOTS Armor:** Purchases and merges the "NO BOTS Armor."
     * - **Scarecrow Hat:** Buys the "Scarecrow Hat" from Yulgar's shop.
     */
    static shirtAndHat(): void {
        Core.farmingLogger("NO BOTS Armor", 1);
        SM.buyAllMerge("NO BOTS Armor");
        Core.farmingLogger("Scarecrow Hat", 1);
        Adv.buyItem("yulgar", 16, "Scarecrow Hat");
    }

    /**
     * Hunts a specific monster to obtain and equip an item related to server downtime.
     * - **Item Acquisition:** Hunts the "Rabid Server Hamster" to get the "The Server is Down" item if not already in inventory.
     * - **Equipping:** Equips the obtained item once acquired.
     */
    serversAreDown(): void {
        if (Core.checkInventory("The Server is Down"))
            return;

        Core.farmingLogger("The Server is Down", 1);
        this.setClass();
        Core.huntMonster("undergroundlabb", "Rabid Server Hamster", "The Server is Down", 1, { isTemp: false, log: false });
        bot.wait.forPickup("The Server is Down");
        Core.equip("The Server is Down");
    }

    /**
     * Sets up a character with essential beginner items and classes. This includes:
     * - **Equipping Basic Gear:**
     *   - **Helm and Cape:** Equips the "Battle Oracle Hood" and "Battle Oracle Wings" if not already equipped.
     *   - **Default Weapons:** Replaces any default weapon with the "Battle Oracle Battlestaff" and sells the default weapon.
     * - **Class Preparation:**
     *   - **Skipping Setup:** If the character is level 30 or higher and has the required classes, skips further setup.
     *   - **Initial Setup:** Acquires initial badges, level up to 10, and equips a "Rogue" or "Ninja" class if not already owned.
     *   - **Final Setup:** Obtains and ranks up the "Ninja" class, and acquires a "Mage" class for farming if not already owned.
     */
    beginnerItems(): void {
        // Combined skip logic: skip if EITHER high-tier classes exist OR both beginner classes are rank 10+
        const except = (list: string[], ...names: string[]) => list.filter(x => !names.includes(x));
        const hasHighTier =
            Core.checkInventory(except(this.soloClasses, "Assassin", "Ninja Warrior", "Ninja"), 1, { any: true, toInv: false }) &&
            Core.checkInventory(except(this.farmClasses, "Mage", "Mage (rare)"), 1, { any: true, toInv: false });

        const hasBothRank10Beginners =
            Core.checkInventory(["Assassin", "Ninja Warrior", "Ninja"], 1, { any: true, toInv: false }) &&
            (this.classNinja?.quantity ?? 0) >= RANK_10_CLASS_POINTS &&
            Core.checkInventory(["Mage (Rare)", "Mage"], 1, { any: true, toInv: false }) &&
            (this.classMage?.quantity ?? 0) >= RANK_10_CLASS_POINTS &&
            bot.player.level >= 30;

        if (hasHighTier || hasBothRank10Beginners) {
            Core.logger("Acc is lvl 30+, skipping beginner items.");
            return;
        }

        Core.logger('As the Acc is still "new", we\'re going todo the tutorial badges');

        tutorial.badges();

        // C# loops over every ItemCategory; only these three do anything.
        for (const category of ["Helm", "Cape", "Weapon"]) {
            const equippedItem = bot.inventory.items.find(i => i.equipped && i.category === category);

            switch (category) {
                case "Helm":
                    if (equippedItem == null) {
                        Core.buyItem("classhalla", 299, "Battle Oracle Hood");
                        Core.equip("Battle Oracle Hood");
                    }
                    break;

                case "Cape":
                    if (equippedItem == null) {
                        Core.buyItem("classhalla", 299, "Battle Oracle Wings");
                        Core.equip("Battle Oracle Wings");
                    }
                    break;

                // C# runs this once per weapon category (Staff, Axe, Bow, ...); after the first
                // the default weapon is sold, so once is the same.
                case "Weapon": {
                    const defaultWep = bot.inventory.items.find(x => x.name.startsWith("Default"));

                    if (defaultWep != null && Core.checkInventory(defaultWep.id)) {
                        Core.buyItem("classhalla", 299, "Battle Oracle Battlestaff");
                        Core.equip("Battle Oracle Battlestaff");
                        bot.shops.sellItem(defaultWep.id);
                        bot.wait.forTrue(() => !bot.inventory.contains(defaultWep.id), 20);
                    }
                    break;
                }
            }
        }

        Core.logger("Starting out acc:\n\tGoals: Ninja class & Mage Class");

        Farm.experience(10);

        Core.logger("Getting Started: Beginner Levels/Equipment");

        // Buy Rogue if we don't already own it.
        if (!Core.checkInventory(["Rogue (Rare)", "Rogue"], 1, { any: true, toInv: false })) {
            Core.logger('Getting rogue.. so we can get ninja (there\'s a 10k hp "boss" to kill during the "Hit Job" quest.)');

            Adv.buyItem("classhalla", 172, "Rogue");
        }

        if (!Core.checkInventory(["Assassin", "Ninja Warrior", "Ninja"], 1, { any: true, toInv: false })) {
            Core.logger("Getting starter Dodge class (Ninja)");

            Adv.rankUpClass("Rogue");
            Farm.experience(25);
            this.setClass();

            // Ninja requires a few quests.
            mazumi.mazumiQuests();

            Core.buyItem("classhalla", 178, "Ninja");
        }

        this.setClass();

        if (!Core.checkInventory(["Mage (Rare)", "Mage"], 1, { any: true, toInv: false })) {
            Core.logger("Getting Starter Farm class (Mage)");
            Adv.buyItem("classhalla", 174, 15653, 1, 9845);
        }

        this.setClass();
    }
    //#endregion Extra:

    //#region BTS
    private _lastCheckedLevel = 0;
    private _lastSolo: string | null = null;
    private _lastFarm: string | null = null;
    private _lastDodge: string | null = null;

    setClass(): void {
        Core.readCbo();
        this._classCache.clear();

        // Skip if nothing meaningful changed
        if (this.shouldSkipSetClass())
            return;

        // Resolve classes
        Core.soloClass = CoreFarmerJoe.resolveClass(Core.soloClass, this.soloClasses) ?? Core.soloClass;
        Core.farmClass = CoreFarmerJoe.resolveClass(Core.farmClass, this.farmClasses) ?? Core.farmClass;
        Core.dodgeClass = CoreFarmerJoe.resolveClass(Core.dodgeClass, this.dodgeClasses) ?? Core.dodgeClass;

        // C# reads the CBO file again here, which puts back the classes it just resolved.
        Core.readCbo();

        const rankTargets: [string | null, string][] = [
            [Core.soloClass, "SoloClass"],
            [Core.farmClass, "FarmClass"],
            [Core.dodgeClass, "DodgeClass"],
        ];

        for (const [className, label] of rankTargets)
            CoreFarmerJoe.rankIfNeeded(className, label);

        bot.sleep(500);

        Core.equipClass(ClassType.Solo);

        this._lastCheckedLevel = bot.player?.level ?? 0;
        this._lastSolo = Core.soloClass;
        this._lastFarm = Core.farmClass;
        this._lastDodge = Core.dodgeClass;
    }

    private shouldSkipSetClass(): boolean {
        const currentLevel = bot.player?.level ?? 0;

        const levelIncreased = currentLevel > this._lastCheckedLevel;

        const same = (a: string | null, b: string | null) => (a ?? "").toLowerCase() === (b ?? "").toLowerCase() && (a == null) === (b == null);
        const classChanged =
            !same(this._lastSolo, Core.soloClass) || !same(this._lastFarm, Core.farmClass) || !same(this._lastDodge, Core.dodgeClass);

        return !levelIncreased && !classChanged;
    }

    /** C# `ResolveClass(current, pool, out found)`; `found` is never read, so it returns only the class. */
    private static resolveClass(current: string | null, pool: string[]): string | null {
        const invalid =
            !current ||
            current.toLowerCase() === "generic" ||
            !pool.some(x => x.toLowerCase() === current.toLowerCase()) ||
            !Core.checkInventory(current);

        if (!invalid)
            return current;

        const found = pool.find(x => Core.checkInventory(x));
        return found ?? current;
    }

    private static bankAllUnusedClasses(): void {
        // More explicit null handling instead of relying on spread operator
        const keepList: string[] = [];
        if (Core.soloClass?.trim())
            keepList.push(Core.soloClass);
        if (Core.farmClass?.trim())
            keepList.push(Core.farmClass);
        if (Core.dodgeClass?.trim())
            keepList.push(Core.dodgeClass);

        const keep = keepList.map(x => x.toLowerCase());

        // Get ALL classes from inventory (actual items)
        const toBank = bot.inventory.items
            .filter(x => x != null && x.category === "Class" && !keep.includes(x.name.toLowerCase()))
            .map(x => x.name);

        if (toBank.length > 0) {
            Core.logger(`Banking unused classes: ${toBank.join(", ")}`);
            Core.toBank(...toBank);
        } else {
            Core.logger("No unused classes to bank");
        }
    }

    private static rankIfNeeded(className: string | null, label: string): void {
        if (!className)
            return;

        if (!Core.checkInventory(className)) {
            Core.logger(`WARNING: ${label} class '${className}' not in inventory!`);
            return;
        }

        if (Core.checkClassRank({ currentClass: false, className }) >= 10)
            return;

        Core.logger(`${label} resolved -> ${className}`);
        Adv.rankUpClass(className);
    }
    //#endregion BTS

    //#endregion other stuff
}
