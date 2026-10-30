/*
name: CoreFarms
description: Port of the parts of Scripts/CoreFarms.cs the ported scripts use (gold, experience,
    alchemy, the PvP brawls, fishing and boosts, and the reputations the FarmerJoe tree needs).
tags: core
*/
/**
 * CoreFarms — partial TypeScript port of `Scripts/CoreFarms.cs`.
 *
 * Only the members reachable from the ported scripts are here (Gold, Experience, IcestormArena,
 * Alchemy crafting and reputation with Gold Vouchers and Dragon Runestones, the Bludrut and Death
 * Pit brawls, Battle Under B, fishing and its boosts, and the Blacksmithing, Blade of Awe, Chaos,
 * ChronoSpan, Death Pit Brawl, DoomWood, Evil, Fishing, Glacera, Good, Loremaster, Mythsong,
 * SpellCrafting, Swordhaven and Vampire reputations); port more from the C# file as scripts need them.
 */

import { bot, BoostType } from "turu";
import type { ShopItem } from "turu";
import { Alignment, ClassType, Core } from "./CoreBots";

/** C# `Skua.Core.Models.BoostType` (the engine's, re-exported for scripts that import it here). */
export { BoostType };

/** "1,500,000" (C# `{n:N0}`). */
function formatN0(value: number): string {
    return String(Math.trunc(value)).replace(/\B(?=(\d{3})+(?!\d))/g, ",");
}

/** C# `CoreFarms.RepThresholds`: `[rank, total rep needed]`. */
const REP_THRESHOLDS: readonly (readonly [rank: number, repRequired: number])[] = [
    [1, 0],
    [2, 900],
    [3, 3600],
    [4, 10000],
    [5, 22500],
    [6, 44100],
    [7, 78400],
    [8, 129600],
    [9, 202500],
    [10, 302500],
];

/** C# `new Random().Next(min, max)`: an integer in [min, max). */
function randomInt(min: number, max: number): number {
    return min + Math.floor(Math.random() * (max - min));
}

function equalsIgnoreCase(a: string | null | undefined, b: string | null | undefined): boolean {
    return (a ?? "").toLowerCase() === (b ?? "").toLowerCase();
}

/**
 * C# `CoreFarms.AlchemyTraits`: the trait of an alchemy craft. The value is the name the craft
 * packet carries (the C# packet interpolates the enum).
 */
export enum AlchemyTraits {
    /** Potent Honor Potion, Malice Potion, Might Tonic, Soul Potion. */
    Dam = "Dam",
    /** Potent Battle Elixir, Unstable Battle Elixir. */
    APw = "APw",
    /** Fate Tonic, Unstable Fate Tonic. */
    Luc = "Luc",
    /** Sage Tonic. */
    Int = "Int",
    /** Potent Malevolence Elixir. */
    SPw = "SPw",
    /** Healer Elixir / Potent Guard Potion / Unstable Healer Elixir: the rep farm (use with Jera). */
    hOu = "hOu",
    /** Potent Revitalize Elixir. */
    hRe = "hRe",
    /** Potent Destruction Elixir. */
    mRe = "mRe",
    /** Body Tonic, Fortitude Tonic, Unstable Body Tonic. */
    End = "End",
    Eva = "Eva",
    /** Unstable Might Tonic. */
    Str = "Str",
    /** Unstable Keen Elixir. */
    Cri = "Cri",
    /** Unstable Mastery Tonic. */
    Dex = "Dex",
    /** Unstable Wise Tonic, Judgment Tonic. */
    Wis = "Wis",
    /** Potent Life Potion. */
    Hea = "Hea",
}

/** C# `AlchemyRunes`: the rune of an alchemy craft (the packet carries its name). */
export enum AlchemyRunes {
    Dragon = "Dragon",
    Jera = "Jera",
    Uruz = "Uruz",
    Fehu = "Fehu",
    Gebo = "Gebo",
}

/** Options of {@link CoreFarms.alchemyPacket} (the C# named arguments). */
export interface AlchemyPacketOptions {
    /** Alchemy rank to reach first (0 for none). */
    rank?: number;
    /** Keep crafting while the reagents and Dragon Runestones last. */
    loop?: boolean;
    /** Only logged, as in C#. */
    modifier?: string;
    trait?: AlchemyTraits;
    /** Stop once you own this item (`quant` of it). */
    item?: string | null;
    quant?: number;
}

/** The Gold Vouchers and the shop that sells each one (C# `CoreFarms.Voucher`). */
const goldVouchers = new Map<string, { id: number; map: string; shopId: number }>([
    ["Gold Voucher 500k", { id: 61043, map: "alchemyacademy", shopId: 2036 }],
    ["Gold Voucher 100k", { id: 62749, map: "alchemyacademy", shopId: 2036 }],
    ["Gold Voucher 200k", { id: 62748, map: "titanattack", shopId: 2149 }],
    ["Gold Voucher 25k", { id: 62747, map: "hydrachallenge", shopId: 1597 }],
    ["Gold Voucher 7.5k", { id: 62747, map: "alchemyacademy", shopId: 2116 }],
]);

export class CoreFarms {
    private alchemyCraftStarted = false;
    private alchemyCraftCompleted = false;
    private alchemyCraftFailed = false;

    /**
     * Enables or disables a boost. Enabling needs the matching CoreBots Options switch (Other >
     * Boosters: `doGoldBoost`, `doClassBoost`, `doRepBoost`, `doExpBoost`); it finds the boost item
     * (inventory, then bank) and starts the boost timer. Gold is skipped at max gold, experience at
     * level 100, class at rank 10. Disabling the last one stops the timer.
     */
    toggleBoost(type: BoostType, enabled = true): void {
        // Only allow enabling if the corresponding CBO is true
        if (enabled) {
            switch (type) {
                case BoostType.Gold:
                    if (Core.cboBool("doGoldBoost") !== true || bot.boosts.uses(BoostType.Gold) || bot.player.gold >= 100_000_000) return;
                    bot.boosts.setBoostIdFromInventory(BoostType.Gold);
                    bot.boosts.setUses(BoostType.Gold, true);
                    Core.logger("💰 Gold boost activated!");
                    break;
                case BoostType.Class:
                    if (Core.cboBool("doClassBoost") !== true || bot.boosts.uses(BoostType.Class) || Core.checkClassRank({ currentClass: true }) === 10) return;
                    bot.boosts.setBoostIdFromInventory(BoostType.Class);
                    bot.boosts.setUses(BoostType.Class, true);
                    Core.logger("🛡️ Class boost activated!");
                    break;
                case BoostType.Reputation:
                    if (Core.cboBool("doRepBoost") !== true || bot.boosts.uses(BoostType.Reputation)) return;
                    bot.boosts.setBoostIdFromInventory(BoostType.Reputation);
                    bot.boosts.setUses(BoostType.Reputation, true);
                    Core.logger("🏰 Reputation boost activated!");
                    break;
                case BoostType.Experience:
                    if (Core.cboBool("doExpBoost") !== true || bot.boosts.uses(BoostType.Experience) || bot.player.level >= 100) return;
                    bot.boosts.setBoostIdFromInventory(BoostType.Experience);
                    bot.boosts.setUses(BoostType.Experience, true);
                    Core.logger("📚 Experience boost activated!");
                    break;
            }

            bot.boosts.start();
        } else {
            // Disable the boost
            switch (type) {
                case BoostType.Gold:
                    bot.boosts.setUses(BoostType.Gold, false);
                    Core.logger("💰 Gold boost deactivated.");
                    break;
                case BoostType.Class:
                    bot.boosts.setUses(BoostType.Class, false);
                    Core.logger("🛡️ Class boost deactivated.");
                    break;
                case BoostType.Reputation:
                    bot.boosts.setUses(BoostType.Reputation, false);
                    Core.logger("🏰 Reputation boost deactivated.");
                    break;
                case BoostType.Experience:
                    bot.boosts.setUses(BoostType.Experience, false);
                    Core.logger("📚 Experience boost deactivated.");
                    break;
            }

            // Stop all boosts if none are active
            if (
                !bot.boosts.uses(BoostType.Gold) &&
                !bot.boosts.uses(BoostType.Class) &&
                !bot.boosts.uses(BoostType.Reputation) &&
                !bot.boosts.uses(BoostType.Experience)
            ) {
                bot.boosts.stop();
                Core.logger("🛑 All boosts stopped.");
            }
        }
    }

    /** Farms gold up to `quant` (HonorHall, BattleGroundE, then Berserker Bunny). */
    gold(quant = 100000000): void {
        if (bot.player.gold >= quant) return;

        this.toggleBoost(BoostType.Gold);

        this.honorHall(quant);
        this.battleGroundE(quant);
        this.berserkerBunny(quant);

        this.toggleBoost(BoostType.Gold, false);
    }

    /** Farms Gold in HonorHall (members) with quests HonorHall Mobs and 61-75. */
    honorHall(goldQuant = 100000000): void {
        if (!Core.isMember || bot.player.level < 61 || bot.player.gold >= goldQuant) return;

        Core.equipClass(ClassType.Farm);
        Core.savedState(true, "honorhall");
        Core.logger(`Farming ${formatN0(goldQuant)} gold using HonorHall Method`);

        Core.registerQuests(3992, 3993);
        while (!bot.shouldExit && bot.player.gold < goldQuant) {
            if (Core.checkSaveState()) Core.executeSaveState();
            Core.killMonster("honorhall", "r1", "Center", "Ice Demon");
        }
        Core.cancelRegisteredQuests();
        Core.savedState(false);
    }

    /** Farms Gold in Battle Ground E with quests Level 46-60 and 61-75. */
    battleGroundE(goldQuant = 100000000): void {
        if (bot.player.level < 61 || bot.player.gold >= goldQuant) return;

        Core.equipClass(ClassType.Farm);
        Core.savedState(true, "battlegrounde");
        Core.logger(`Farming ${formatN0(goldQuant)} gold using BattleGroundE Method`);

        Core.registerQuests(3991, 3992);
        while (!bot.shouldExit && bot.player.gold < goldQuant) {
            if (!bot.player.alive) {
                Core.waitForTrue(() => bot.player.alive, 20);
                continue;
            }

            if (bot.map.name !== "battlegrounde") Core.join("battlegrounde", "Enter", "Spawn", { publicRoom: Core.privateRooms });
            if (bot.player.cell !== "r2") Core.jump("r2", "center");

            Core.canWeAggro();

            if (Core.checkSaveState()) Core.executeSaveState();
            if (!bot.player.hasTarget) bot.combat.attack("*");
            Core.sleep();
        }
        Core.cancelRegisteredQuests();
        Core.savedState(false);
    }

    /** Farms Gold by selling Berserker Bunny. */
    berserkerBunny(goldQuant = 100000000, _sell = true): void {
        if (bot.player.gold >= goldQuant) return;

        Core.addDrop("Berserker Bunny");
        Core.equipClass(ClassType.Solo);
        Core.logger(`${bot.player.gold}/${goldQuant} Gold`, "BerserkerBunny Method");

        Core.savedState(true, "greenguardwest");
        Core.registerQuests(236);
        while (!bot.shouldExit && bot.player.gold < goldQuant) {
            if (Core.checkSaveState()) Core.executeSaveState();
            Core.killMonster("greenguardwest", "West12", "Up", "*");
            Core.sellItem("Berserker Bunny", 1, { all: true });
        }
        Core.cancelRegisteredQuests();
        Core.savedState(false);
    }

    /**
     * Levels up to `level` (Oaklore to 10, Undead Giant to 20, then IcestormArena), or ranks up
     * the class with `rankUpClass`.
     */
    experience(level = 100, rankUpClass = false): void {
        if (bot.player.level >= level && !rankUpClass) return;

        if (!rankUpClass) Core.equipClass(ClassType.Farm);
        if (rankUpClass) this.toggleBoost(BoostType.Class);

        this.toggleBoost(BoostType.Experience);
        if (rankUpClass) this.icestormArena(level, rankUpClass);
        else {
            if (bot.player.level < 10) {
                Core.logger('Doing Oaklore "Bone Berserker" Quest till level 10');
                Core.registerQuests(4007, 6257);
                while (!bot.shouldExit && bot.player.level < Math.min(10, level))
                    Core.killMonster("oaklore", "r3", "Left", "Bone Berserker", null, 1, { log: false });
                Core.cancelRegisteredQuests();
            }

            if (bot.player.level >= level && !rankUpClass) return;

            if (bot.player.level < 20) {
                this.undeadGiantUnlock();
                Core.logger('Doing swordhavenundead "Undead Giant" Quest till level 20');
                Core.registerQuests(178);
                while (!bot.shouldExit && bot.player.level < Math.min(20, level))
                    Core.killMonster("swordhavenundead", "Gates", "Left", "Undead Giant", null, 1, { log: false });
                Core.cancelRegisteredQuests();
            }

            if (bot.player.level >= level && !rankUpClass) return;

            this.icestormArena(level, rankUpClass);
        }

        if (rankUpClass) this.toggleBoost(BoostType.Class, false);
        this.toggleBoost(BoostType.Experience, false);
    }

    /**
     * Farms level in Ice Storm Arena (and BattleGroundE for 61-75, IcestormUnder for 75+).
     * @param level Desired level.
     * @param rankUpClass Whether to rank up the class instead.
     */
    icestormArena(level = 100, rankUpClass = false): void {
        // Exit if the player's level has already reached the desired level and we are not ranking up the class
        if (bot.player.level >= level && !rankUpClass) return;

        const levelReached = () => bot.player.level >= level && !rankUpClass;
        const bypassHere = () => Core.byPassCheck([bot.player.cell, bot.player.pad]);

        /** Waits while dead, then re-applies the bypass and jumps to `cell` (C#'s inner `while (!Alive)` loops). */
        const reviveTo = (cell: string, pad: string, waitForCell = true, checkLevel = false): void => {
            while (!bot.shouldExit && !bot.player.alive) {
                if (checkLevel && levelReached()) break;
                if (bot.player.alive) {
                    Core.byPassCheck([cell, pad]);
                    bot.sleep(1000);
                    if (waitForCell) {
                        Core.jump(cell, pad);
                        bot.wait.forCellChange(cell);
                    } else if (bot.player.cell !== cell) Core.jump(cell, pad);
                    break;
                } else bot.sleep(1000);
            }
        };

        /** Joins icestormarena (waiting for the map) and jumps to `cell` if needed. */
        const goTo = (cell: string, pad: string, wait = true): void => {
            if (bot.map.name !== "icestormarena") {
                Core.join("icestormarena", "Enter", "Spawn", { publicRoom: Core.privateRooms });
                if (wait) bot.wait.forMapLoad("icestormarena");
            }
            if (bot.player.cell !== cell) {
                Core.jump(cell, pad);
                if (wait) bot.wait.forCellChange(cell);
            }
        };

        const saveState = (): void => {
            if (Core.checkSaveState()) {
                Core.executeSaveState();
                bypassHere();
            }
        };

        // C# `goto Cleanup` = return from this function.
        const farm = (): void => {
            // Equip the class for farming or rank up boost as needed
            if (!rankUpClass) Core.equipClass(ClassType.Farm);
            if (rankUpClass) this.toggleBoost(BoostType.Class);

            // Enable aggro for the farm
            Core.toggleAggro(true);
            Core.savedState(true, "icestormarena");

            // Toggle experience boost if we are farming for experience
            if (bot.player.level < 100) this.toggleBoost(BoostType.Experience);

            bypassHere();
            // Farming between levels 1-5
            bot.log(`rankUpClass: ${rankUpClass ? "True" : "False"}, CurrentClassRank: ${bot.player.currentClassRank}`);
            while (
                !bot.shouldExit &&
                ((bot.player.level < 5 && rankUpClass && bot.player.currentClassRank < 10) || (!rankUpClass && bot.player.level < 5))
            ) {
                if (levelReached()) break;
                reviveTo("r4", "Bottom");
                goTo("r4", "Bottom");
                saveState();
                Core.canWeAggro();
                bot.combat.attack("*");
                Core.sleep(200);
            }

            if (levelReached()) return;
            bypassHere();

            // Farming between levels 5-10
            while (
                !bot.shouldExit &&
                ((bot.player.level < 10 && rankUpClass && bot.player.currentClassRank !== 10) ||
                    (!rankUpClass && bot.player.level >= 5 && bot.player.level < 10))
            ) {
                if (levelReached()) break;
                reviveTo("r5", "Left");
                goTo("r5", "Left");
                Core.canWeAggro();
                saveState();
                bot.combat.attack("*");
                Core.sleep(200);
            }

            if (levelReached()) return;
            bypassHere();

            // Farming between levels 10-20
            while (
                !bot.shouldExit &&
                ((bot.player.level < 20 && rankUpClass && bot.player.currentClassRank !== 10) ||
                    (!rankUpClass && bot.player.level >= 10 && bot.player.level < 20))
            ) {
                if (levelReached()) break;
                reviveTo("r6", "Left");
                goTo("r6", "Left");
                Core.canWeAggro();
                saveState();
                bot.combat.attack("*");
                Core.sleep(200);
            }

            if (levelReached()) return;
            bypassHere();

            // Farming between levels 20-25
            if (bot.player.level < 25) {
                Core.registerQuests(6628);
                while (
                    !bot.shouldExit &&
                    ((bot.player.level < 25 && rankUpClass && bot.player.currentClassRank !== 10) ||
                        (!rankUpClass && bot.player.level >= 20 && bot.player.level < 25))
                ) {
                    if (levelReached()) break;
                    reviveTo("r7", "Left");
                    goTo("r7", "Left");
                    Core.canWeAggro();
                    bot.combat.attack("*");
                    Core.sleep(200);
                }
                bot.quests.unregisterQuests(6628);
                Core.abandonQuest(6628);
            }

            if (levelReached()) return;
            bypassHere();

            // Farming between levels 25-30
            while (
                !bot.shouldExit &&
                ((bot.player.level < 30 && rankUpClass && bot.player.currentClassRank !== 10) ||
                    (!rankUpClass && bot.player.level >= 25 && bot.player.level < 30))
            ) {
                if (levelReached()) break;
                reviveTo("r10", "Left");
                goTo("r10", "Left");
                Core.canWeAggro();
                saveState();
                bot.combat.attack("*");
                Core.sleep(200);
            }

            if (levelReached()) return;
            bypassHere();

            // Farming between levels 30-35 (and switching to solo class if needed)
            if (bot.player.level >= 30 && bot.player.level < 35) {
                if (!rankUpClass) Core.equipClass(ClassType.Solo);
                Core.registerQuests(6629);
                while (
                    !bot.shouldExit &&
                    ((bot.player.level < 35 && rankUpClass && bot.player.currentClassRank !== 10) ||
                        (!rankUpClass && bot.player.level >= 30 && bot.player.level < 35))
                ) {
                    if (levelReached()) break;
                    reviveTo("r11", "Left");
                    goTo("r11", "Left");
                    Core.canWeAggro();
                    saveState();
                    bot.combat.attack("*");
                    Core.sleep(200);
                }
                bot.quests.unregisterQuests(6629);
                Core.abandonQuest(6629);
            }

            if (levelReached()) return;

            if (!rankUpClass) Core.equipClass(ClassType.Farm);

            bypassHere();
            // Farming between levels 35-50 (C# revives to r14 but fights in r11)
            Core.registerQuests(6629);
            while (
                !bot.shouldExit &&
                ((bot.player.level < 50 && rankUpClass && bot.player.currentClassRank !== 10) ||
                    (!rankUpClass && bot.player.level >= 35 && bot.player.level < 50))
            ) {
                if (levelReached()) break;
                reviveTo("r14", "Left", true, true);
                goTo("r11", "Left");
                Core.canWeAggro();
                saveState();
                bot.combat.attack("*");
                Core.sleep(200);
            }
            bot.quests.unregisterQuests(6629);
            Core.abandonQuest(6629);

            if (levelReached()) return;
            bypassHere();

            // Farming between levels 50-61
            while (
                !bot.shouldExit &&
                ((bot.player.level < 61 && rankUpClass && bot.player.currentClassRank !== 10) ||
                    (!rankUpClass && bot.player.level >= 50 && bot.player.level < 61))
            ) {
                if (levelReached()) break;
                reviveTo("r16", "Left");
                Core.canWeAggro();
                goTo("r16", "Left");
                saveState();
                bot.combat.attack("*");
                Core.sleep(200);
            }

            if (levelReached()) return;

            Core.savedState(false);

            bypassHere();
            // Farming between levels 61-75 with BattleGroundE for non-rank-up class
            if (bot.player.level >= 61 && bot.player.level < 75) {
                if (rankUpClass) {
                    while (
                        !bot.shouldExit &&
                        ((bot.player.level < 75 && rankUpClass && bot.player.currentClassRank !== 10) ||
                            (!rankUpClass && bot.player.level >= 61 && bot.player.level < 75))
                    ) {
                        if (bot.player.level >= level && bot.player.currentClassRank === 10) break;
                        reviveTo("r17", "Left", false);
                        goTo("r17", "Left", false);
                        Core.canWeAggro();
                        saveState();
                        bot.combat.attack("*");
                        Core.sleep(200);
                    }
                } else {
                    if (bot.player.gold < 100000000) this.toggleBoost(BoostType.Gold);

                    Core.registerQuests(3991, 3992);
                    Core.savedState(true, "battlegrounde");
                    while (!bot.shouldExit && bot.player.level >= 61 && bot.player.level < 75) {
                        if (levelReached()) break;
                        if (!bot.player.alive) {
                            Core.waitForTrue(() => bot.player.alive, 20);
                            Core.byPassCheck(["r2", "center"]);
                            continue;
                        }
                        if (bot.map.name !== "battlegrounde") Core.join("battlegrounde", "Enter", "Spawn", { publicRoom: Core.privateRooms });
                        if (bot.player.cell !== "r2") Core.jump("r2", "center");

                        Core.canWeAggro();
                        saveState();
                        if (!bot.player.hasTarget) bot.combat.attack("*");
                        Core.sleep();
                    }
                    bot.quests.unregisterQuests(3991, 3992);
                    Core.abandonQuest(3991, 3992);
                    this.toggleBoost(BoostType.Gold, false);
                }
            }
            Core.savedState(false);

            if (levelReached()) return;
            bypassHere();

            // Farming between levels 75-100
            Core.savedState(true, "icestormunder");
            while (
                !bot.shouldExit &&
                ((bot.player.level <= level && rankUpClass && bot.player.currentClassRank !== 10) || (!rankUpClass && bot.player.level <= level))
            ) {
                if (levelReached()) break;
                // Wait if dead, then spoof levelUp
                if (!bot.player.alive) {
                    Core.waitForTrue(() => bot.player.alive, 20);
                    bot.sleep(1000);
                    Core.byPassCheck(["r2", "Top"]);
                    continue;
                }

                // Ensure map
                if (bot.map.name !== "icestormunder") {
                    Core.join("icestormunder", "Enter", "Spawn", { publicRoom: Core.privateRooms });
                    bot.wait.forMapLoad("icestormunder");
                }

                // Ensure position
                if (bot.player.cell !== "r2") {
                    bot.map.jump("r2", "Top", false);
                    bot.wait.forCellChange("r2");
                    bot.player.setSpawnPoint();
                }

                Core.canWeAggro();
                saveState();
                bot.combat.attack("*");
                Core.pause(200);
            }
        };

        farm();

        // Cleanup
        Core.savedState(false);
        bot.options.attackWithoutTarget = false;
        Core.toggleAggro(false);
        Core.jump();
        bot.options.aggroMonsters = false;
        Core.jumpWait();
        Core.rest();

        // Disable any active boosts
        if (rankUpClass) this.toggleBoost(BoostType.Class, false);
        this.toggleBoost(BoostType.Experience, false);
    }

    /** Farms the Black Knight Orb (quest 318). */
    blackKnightOrb(): void {
        if (Core.checkInventory("Black Knight Orb")) return;

        Core.addDrop("Black Knight Orb");
        Core.ensureAccept(318);

        Core.huntMonster("well", "Gell Oh No", "Black Knight Leg Piece");
        Core.huntMonster("greendragon", "Greenguard Dragon", "Black Knight Chest Piece");
        Core.huntMonster("deathgazer", "Deathgazer", "Black Knight Shoulder Piece");
        Core.huntMonster("trunk", "GreenGuard Basilisk", "Black Knight Arm Piece");

        Core.ensureComplete(318);
        bot.wait.forPickup("Black Knight Orb");
    }

    // C# TheSecret4: not reached by the ported scripts, not ported yet.

    /**
     * Defeats the Team B Captain in /BludrutBrawl for `item` ("Combat Trophy" or "Yoshino's
     * Citrine"). The CBO switch `PvP_SoloPvPBoss` makes it kill the healers too.
     */
    bludrutBrawlBoss(item = "Combat Trophy", quant = 5000, _canSoloBoss = true): void {
        if (Core.checkInventory(item, quant)) return;

        if (bot.map.name === "bludrutbrawl") {
            Core.logger("Started in PvP map, which doesnt allow us to equip things if needed and can cause issues. Joining whitemap first.");
            Core.join("whitemap");
        }

        // C# overwrites canSoloBoss with the CBO lookup's result and uses its out value.
        const killAds = Core.cboBool("PvP_SoloPvPBoss") === true;
        Core.logger(
            `\`Kill Ads\` Enabled? ${killAds ? "True" : "False"}` +
                (killAds ? " - if enabled this will kill the healers aswell for an aditional 3 trophies (totaling 10) but will take longer." : ""),
        );
        Core.addDrop(item);

        Core.equipClass(ClassType.Solo);
        Core.farmingLogger(item, quant);
        bot.options.aggroAllMonsters = false;
        bot.options.aggroMonsters = false;

        // Find the first matching amulet in inventory or bank
        const owned = [...bot.inventory.items, ...bot.bank.items];
        const amulet = this.acceptablePvPAmulets.find((name) => owned.some((i) => i?.name === name));

        // Equip the amulet if found in inventory or bank
        if (amulet !== undefined) {
            Core.unbank(amulet);
            Core.equip(amulet);
        }

        // C#'s Start / Exit / RestartOnDeath labels: `continue start` is `goto Start`.
        start: for (;;) {
            let exitAttempt = 1;
            let death = 0;
            while (!bot.shouldExit && !Core.checkInventory(item, quant)) {
                // One run: false when you died on the way.
                const run = (): boolean => {
                    Core.join("bludrutbrawl-999999", "Enter0", "Spawn");
                    bot.wait.forMapLoad("bludrutbrawl");
                    // C# reads "Combat Trophy" here, so farming Yoshino's Citrine compared the
                    // citrine count against the trophy count and waited forever below.
                    let currentTrophies = bot.inventory.getQuantity(item);

                    Core.pvpMove(5, "Morale0C", randomInt(784, 862), randomInt(254, 274));
                    Core.pvpMove(4, "Morale0B", randomInt(786, 850), randomInt(262, 287));
                    Core.pvpMove(7, "Morale0A", randomInt(783, 857), randomInt(263, 293));
                    Core.pvpMove(9, "Crosslower", randomInt(777, 857), randomInt(254, 290));

                    // If CBO setting for `Kill ads before boss` is enabled do:
                    if (killAds) {
                        // C# checks "owns an amulet and has anything equipped"; this checks an amulet is equipped.
                        if (!bot.inventory.items.some((x) => x?.equipped && this.acceptablePvPAmulets.includes(x.name)))
                            Core.oneTimeMessage("Wheres your amulet retard?", "Hey ChuckleFuck, you forgot your amulet! So now you get to kill the minions.");

                        Core.pvpMove(14, "Crossupper", randomInt(399, 545), randomInt(255, 256));
                        Core.pvpMove(18, "Resource1A", randomInt(786, 860), randomInt(255, 274));

                        Core.pvpKilling();
                        if (!bot.player.alive) return false;

                        Core.pvpMove(20, "Resource1B", randomInt(784, 852), randomInt(254, 293));

                        Core.pvpKilling();
                        if (!bot.player.alive) return false;

                        Core.pvpMove(21, "Resource1A", randomInt(123, 189), randomInt(262, 297));
                        Core.pvpMove(19, "Crossupper", randomInt(122, 189), randomInt(258, 282));
                        Core.pvpMove(17, "Crosslower", randomInt(439, 536), randomInt(467, 470));
                    }

                    Core.pvpMove(15, "Morale1A", randomInt(781, 858), randomInt(258, 290));
                    bot.kill.monster(13);
                    if (!bot.player.alive) return false;

                    Core.pvpMove(23, "Morale1B", randomInt(782, 850), randomInt(259, 276));
                    bot.kill.monster(14);
                    if (!bot.player.alive) return false;

                    Core.pvpMove(25, "Morale1C", randomInt(802, 865), randomInt(264, 286));
                    bot.kill.monster(15);
                    if (!bot.player.alive) return false;

                    Core.pvpMove(28, "Captain1", randomInt(430, 537), randomInt(254, 255));
                    bot.kill.monster(16);
                    if (!bot.player.alive) return false;

                    // Wait for drop to appear in stack
                    bot.wait.forTrue(() => bot.drops.current.some((d) => d.name === item), 40);

                    Core.sleep(1500);
                    bot.wait.forPickup(item, 40);

                    // Record updated quantity until server actually updates it
                    // (C# waits without a limit, which hangs forever when the drop never came.)
                    let updatedTrophies = bot.inventory.getQuantity(item);
                    for (let tries = 0; !bot.shouldExit && updatedTrophies <= currentTrophies && tries < 60; tries++) {
                        Core.sleep(500); // give server time to process PvP reward
                        updatedTrophies = bot.inventory.getQuantity(item);
                    }
                    if (updatedTrophies <= currentTrophies) Core.logger(`No ${item} from this run, trying again.`);
                    // Update baseline so next run compares correctly
                    currentTrophies = updatedTrophies;

                    if (item) Core.farmingLogger(item, quant);

                    Core.sleep(1500);
                    return true;
                };

                if (run()) {
                    // Exit:
                    while (!bot.shouldExit && bot.map.name !== "battleon") {
                        bot.combat.cancelTarget();
                        bot.wait.forCombatExit();
                        bot.map.join("battleon-999999");
                        bot.wait.forMapLoad("battleon");
                        if (bot.map.name !== "battleon") continue;
                        else continue start;
                    }
                }

                // RestartOnDeath: (C# also falls through to it after Exit when already in battleon)
                Core.logger(`Death: ${death++}, resetting`);
                while (!bot.shouldExit) {
                    bot.wait.forTrue(() => bot.player.alive, 100);
                    Core.logger(`Attempting Death Exit ${exitAttempt++}.`);
                    bot.map.join("battleon-999999");
                    bot.wait.forMapLoad("battleon");
                    Core.sleep(1500);
                    if (bot.map.name !== "battleon") Core.logger("Failed!? HOW.. try agian");
                    else {
                        Core.logger("Successful!");
                        continue start;
                    }
                }
            }
            break;
        }

        for (const reward of ["Yoshino's Citrine", "The Secret 4"]) {
            if (item !== reward && bot.inventory.contains(reward)) Core.toBank(reward);
        }
    }

    /** Farms `item` in /battleunderb (unlocking Undead Energy drops first when asked for them). */
    battleUnderB(item = "Bone Dust", quant = 10000, isTemp = false): void {
        if (isTemp ? bot.tempInv.contains(item, quant) : Core.checkInventory(item, quant)) {
            Core.farmingLogger(item, quant);
            return;
        }

        if (item === "Undead Energy" && !Core.isCompletedBefore(2084)) {
            Core.logger("Making it so undead energy can drop..");

            // 2066 - Reforging the Blinding Light
            if (!Core.isCompletedBefore(2066)) {
                Core.ensureAccept(2066);
                Core.buyItem("doomwood", 276, "Blinding Light of Destiny Handle");
                Core.ensureComplete(2066);
            }

            Core.equipClass(ClassType.Farm);
            // 2067 - Secret Order of Undead Slayers
            if (!Core.isCompletedBefore(2067)) {
                Core.ensureAccept(2067);
                Core.buyItem("doomwood", 276, "Bonegrinder Medal");
                Core.ensureComplete(2067);
            }

            // 2082 - Essential Essences
            if (!Core.isCompletedBefore(2082)) {
                Core.ensureAccept(2082);
                Core.huntMonster("battleunderb", "Skeleton Warrior", "Undead Essence", 25, { isTemp: false });
                Core.ensureComplete(2082);
            }

            // 2083 - Bust some Dust
            if (!Core.isCompletedBefore(2083)) {
                Core.ensureAccept(2083);
                Core.huntMonster("battleunderb", "Skeleton Warrior", "Bone Dust", 40, { isTemp: false });
                Core.ensureComplete(2083);
            }

            // 2084 - A Loyal Follower
            if (!Core.isCompletedBefore(2084)) {
                Core.ensureAccept(2084);
                this.boneSomeDust(100);
                Core.huntMonster("timevoid", "Ephemerite", "Celestial Compass");
                Core.ensureComplete(2084);
            }
        }

        Core.addDrop(item);
        Core.farmingLogger(item, quant);
        Core.equipClass(ClassType.Farm);
        Core.killMonster("battleunderb", "Enter", "Spawn", "*", item, quant, { isTemp, log: false });
    }

    /** Farms Spirit Orbs in /battleunderb (quests 2082 and 2083). */
    boneSomeDust(quant = 65000): void {
        if (Core.checkInventory("Spirit Orb", quant)) return;

        Core.addDrop("Bone Dust", "Undead Essence", "Undead Energy", "Spirit Orb");
        Core.equipClass(ClassType.Farm);
        Core.farmingLogger("Spirit Orb", quant);

        Core.registerQuests(2082, 2083);
        while (!bot.shouldExit && !Core.checkInventory("Spirit Orb", quant)) Core.killMonster("battleunderb", "Enter", "Spawn", "*", null, 1, { log: false });
        Core.cancelRegisteredQuests();
    }

    /**
     * Crafts `reagent1` + `reagent2` (C# `AlchemyPacket`). With `loop`, it keeps crafting while the
     * reagents and Dragon Runestones last, or until you own `item`. Reaches `rank` in Alchemy first
     * when asked, and buys Dragon Runestones when you're short.
     */
    alchemyPacket(reagent1: string, reagent2: string, rune = AlchemyRunes.Gebo, options: AlchemyPacketOptions = {}): void {
        const { rank = 0, loop = true, modifier = "Moose", trait = AlchemyTraits.APw, item = null, quant = 1 } = options;

        const offPacket = bot.events.on("extensionPacket", (packet) => this.alchemyPacketCheck(packet));
        try {
            if ((rank !== 0 && this.factionRank("Alchemy") < rank) || (item !== null && Core.checkInventory(item, quant))) {
                this.alchemyREP(rank);
            }

            const reg1 = bot.inventory.getItem(reagent1);
            if (!Core.checkInventory(reagent1) || !reg1) {
                Core.logger(`Something went wrong, you do not own ${reagent1}`, "", { messageBox: true, stopBot: true });
                return;
            }

            const reg2 = bot.inventory.getItem(reagent2);
            if (!Core.checkInventory(reagent2) || !reg2) {
                Core.logger(`Something went wrong, you do not own ${reagent2}`, "", { messageBox: true, stopBot: true });
                return;
            }

            const reagentId1 = reg1.id;
            const reagentId2 = reg2.id;
            const quantR1 = bot.inventory.getQuantity(reagentId1);
            const quantR2 = bot.inventory.getQuantity(reagentId2);
            // Buy Dragon Runestones (if not owned) for the lowest ingredient quantity
            this.dragonRunestone(Math.min(quantR1, quantR2));

            if (!Core.checkInventory("Dragon Runestone")) {
                Core.logger("Missing Dragon Runestone. Cannot continue alchemy!");
                return;
            }

            // C# also sets reg1.ID to 11475 for Dragon Scale here, but the craft uses the IDs read above.
            Core.logger(`Reagents: [${reagent1}], [${reagent2}].`);
            Core.logger(`Rune: ${rune}.`);
            Core.logger(`Modifier: ${modifier}.`);
            Core.join("alchemy");

            let i = 1;
            if (loop) {
                const reagents = [reagent1, reagent2, "Dragon Runestone"];
                while (!bot.shouldExit && Core.checkInventory(reagents)) {
                    if (!Core.checkInventory(reagents) || (item !== null && Core.checkInventory(item, quant))) break;

                    if (!this.alchemyCraft(reagentId1, reagentId2, rune, trait)) {
                        Core.logger("Alchemy craft failed, stopping loop");
                        if (!Core.checkInventory([reagentId1, reagentId2])) Core.logger("We've ran out of reagents.");
                        break;
                    }
                    Core.logger(`Completed alchemy x${i++}`);
                }
            } else {
                this.alchemyCraft(reagentId1, reagentId2, rune, trait);
            }
        } finally {
            offPacket();
        }
    }

    /** One craft: start it, wait out the mixing, then finish it (C# `AlchemyPacket`'s `Packet()`). */
    private alchemyCraft(reagentId1: number, reagentId2: number, rune: AlchemyRunes, trait: AlchemyTraits): boolean {
        const mixTimeMs = 16000;

        // Reset flags at the start of each craft
        this.alchemyCraftStarted = false;
        this.alchemyCraftCompleted = false;
        this.alchemyCraftFailed = false;

        // Initiate potion craft
        Core.sendPackets(`%xt%zm%crafting%1%alchStart%${reagentId1}%${reagentId2}%${rune}%1%${trait}%`);

        // Wait for the server to accept alchStart
        if (!this.waitForAlchemyFlag(() => this.alchemyCraftStarted || this.alchemyCraftFailed, 10000)) {
            Core.logger("Failed to receive server craft start confirmation (10s timeout)");
            return false;
        }

        if (this.alchemyCraftFailed) return false;

        Core.logger(`Mixing for ${mixTimeMs / 1000} seconds before completion`);
        if (this.waitForAlchemyFlag(() => this.alchemyCraftFailed, mixTimeMs) || bot.shouldExit) return false;

        // Send completion packet
        Core.sendPackets("%xt%zm%crafting%1%alchDone%");

        // Wait for completion (single log)
        const completed = this.waitForAlchemyFlag(() => this.alchemyCraftCompleted || this.alchemyCraftFailed, 10000);

        if (this.alchemyCraftFailed) return false;

        if (!completed) {
            Core.logger("Warning: Did not receive completion confirmation within 10s");
            return false;
        }

        // Only log once per craft
        if (this.alchemyCraftCompleted) Core.logger("Server confirmed craft completion");

        // Reset flags for the next craft
        this.alchemyCraftStarted = false;
        this.alchemyCraftCompleted = false;

        Core.sleep(500);
        return true;
    }

    /** Polls the flag every 100 ms until it is set or `timeoutMs` pass (C# `WaitForAlchemyFlag`). */
    private waitForAlchemyFlag(flag: () => boolean, timeoutMs: number): boolean {
        let elapsed = 0;
        const checkInterval = 100;

        while (elapsed < timeoutMs && !bot.shouldExit) {
            if (flag()) return true;

            Core.sleep(checkInterval);
            elapsed += checkInterval;
        }

        return false;
    }

    /** Server replies to a craft (`alchStart`, `alchDone`, `alchError`) set the flags above. */
    private alchemyPacketCheck(packet: any): void {
        const type = packet?.params?.type;
        const data = packet?.params?.dataObj;
        if (type !== "json" || data == null) return;

        switch (data.cmd) {
            case "alchStart":
                this.alchemyCraftStarted = data.ok === 1;
                this.alchemyCraftFailed = !this.alchemyCraftStarted;
                if (this.alchemyCraftStarted) Core.logger("Alchemy craft started (confirmed by server)");
                else Core.logger(`Alchemy start rejected: ${data.msg}`);
                break;

            case "alchDone":
                if (data.ok !== 1) {
                    this.alchemyCraftFailed = true;
                    Core.logger(`Alchemy craft rejected: ${data.msg}`);
                } else if (!this.alchemyCraftCompleted) {
                    // Only log once
                    this.alchemyCraftCompleted = true;
                    Core.logger("Alchemy craft completed (confirmed by server)");
                }
                break;

            case "alchError":
                this.alchemyCraftStarted = false;
                this.alchemyCraftCompleted = false;
                this.alchemyCraftFailed = true;
                Core.logger(`Alchemy error: ${data.error ?? "Unknown error"}`);
                break;
        }
    }

    /**
     * Buys a Gold Voucher (C# `Voucher`), farming the gold for each batch when you can't afford
     * it. Each voucher is sold in its own shop.
     * @param voucherName One of "Gold Voucher 500k", "100k", "200k", "25k" or "7.5k".
     * @param quant How many you need in total.
     */
    voucher(voucherName: string, quant: number): void {
        if (bot.bank.contains(voucherName)) Core.unbank(voucherName);

        const current = bot.inventory.getQuantity(voucherName);
        let needed = quant - current;

        if (needed <= 0) {
            bot.log(`x${current}/${needed} Vouchers Owned`);
            return;
        }

        Core.logger(`${voucherName} [Current: ${current}/${quant}] - Buying x${needed}`);

        const voucher = goldVouchers.get(voucherName);
        if (!voucher) {
            Core.logger(`Invalid Gold Voucher: ${voucherName}`);
            return;
        }
        const { id: voucherId, map, shopId } = voucher;

        if (bot.map.name !== map) Core.join(map);

        let retry = 0;
        let item: ShopItem | undefined;
        while (!item) {
            while (!bot.shouldExit && bot.shops.id !== shopId) {
                bot.shops.load(shopId);
                bot.wait.forActionCooldown("loadShop");
                bot.wait.forTrue(() => bot.shops.isLoaded && bot.shops.id === shopId, 20);
                Core.sleep(1000);
                if (bot.shops.id === shopId || retry++ >= 20) break;
            }

            item = bot.shops.items.find((x) => x && x.id === voucherId);
            if (!item) {
                if (retry >= 5) {
                    Core.logger(`Item "${voucherName}" not found in the shop.`);
                    return;
                }
                retry++;
            }
        }

        const GOLD_CAP = 100_000_000;

        while (!bot.shouldExit && needed > 0) {
            if (!item.name) break;

            const maxStackBuy = Math.min(needed, item.maxStack);
            if (maxStackBuy <= 0) break;

            // Extract voucher value from item name ("Gold Voucher 7.5k" -> 7.5 thousand)
            const lastSpaceIndex = item.name.lastIndexOf(" ");
            const valuePart = lastSpaceIndex >= 0 ? item.name.slice(lastSpaceIndex + 1) : "";
            const parsed = Number(valuePart.toLowerCase().replace("k", "").replace(/,/g, ""));
            const thousands = parsed > 0 ? parsed : 100;
            const pricePerVoucher = Math.round(thousands * 1000);

            // ----- SMART GOLD FARMING -----
            const goldForBatch = maxStackBuy * pricePerVoucher;
            if (bot.player.gold < goldForBatch) this.gold(Math.min(GOLD_CAP, goldForBatch));

            // After farming, buy only what we can afford
            const affordable = Math.floor(bot.player.gold / pricePerVoucher);
            if (affordable <= 0) continue;

            const buyAmount = Math.min(maxStackBuy, affordable);
            Core.buyItem(map, shopId, item.name, buyAmount);
            needed -= buyAmount;
        }
    }

    /** Buys Dragon Runestones from the Alchemy Academy (C# `DragonRunestone`). */
    dragonRunestone(quant = 100): void {
        if (Core.checkInventory("Dragon Runestone", quant)) return;

        Core.farmingLogger("Dragon Runestone", quant);

        if (bot.map.name !== "alchemyacademy") Core.join("alchemyacademy");

        this.voucher("Gold Voucher 100k", quant);
        Core.buyItem("alchemyacademy", 395, "Dragon Runestone", quant, 7679, 0);
    }

    /**
     * Farms Alchemy reputation to `rank` (C# `AlchemyREP`): by buying the reagents with gold
     * (`goldMethod`), or by killing the Lair monsters for the Dragon Scales.
     */
    alchemyREP(rank = 10, goldMethod = true): void {
        if (this.factionRank("Alchemy") >= rank) return;

        Core.logger("Start by Selling all items named dragon scale (there are 2...) sorta fucks it all up, if u have the wrong one");
        for (const item of ["Dragon Scale", "Ice Vapor"]) {
            if (!Core.checkInventory(item)) continue;

            Core.sellItem(item, 1, { all: true });
        }

        if (!Core.hasFaction("Alchemy")) {
            Core.logger("Getting Pre-Ranking XP");
            // ice vapor, dragon scape, dragon runestone
            if (!Core.checkInventory([11478, 11475, 7132])) {
                this.dragonRunestone(3);
                Core.buyItem("alchemy", 397, 11475, 2, 1232);
                Core.buyItem("alchemy", 397, 11478, 2, 1235);
            }
            Core.join("alchemy");
            this.alchemyPacket("Dragon Scale", "Ice Vapor", AlchemyRunes.Jera, { trait: AlchemyTraits.hOu });
        }

        // ice vapor, dragon scape, dragon runestone
        Core.addDrop(11478, 11475, 7132);
        this.toggleBoost(BoostType.Reputation);
        Core.logger(`Farming rank ${rank} Alchemy`);

        let i = 1;
        while (!bot.shouldExit && this.factionRank("Alchemy") < rank) {
            if (goldMethod) {
                if (!Core.checkInventory([11475, 11478, 7132])) {
                    // 10 Dragonstones for alchemy itself + enough to buy 10 mats (1 Dstone -> 2 mats)
                    this.dragonRunestone(20);
                    // each set of 10 should cost 5 Dstones
                    Core.buyItem("alchemy", 397, 11475, 10, 1232); // Dragon Scape
                    Core.buyItem("alchemy", 397, 11478, 10, 1235); // Ice Vapor
                }

                this.alchemyPacket("Dragon Scale", "Ice Vapor", this.alchemyRune(), { trait: AlchemyTraits.hOu });
            } else {
                Core.equipClass(ClassType.Farm);
                while (!bot.shouldExit && !Core.checkInventory(11475, 30)) {
                    Core.killMonster("lair", "Hole", "Center", "*", null, 1, { log: false });
                }
                Core.killMonster("lair", "Enter", "Spawn", "*", "Ice Vapor", 30, { isTemp: false, log: false });
                this.dragonRunestone(30);

                this.alchemyPacket("Dragon Scale", "Ice Vapor", this.alchemyRune(), { trait: AlchemyTraits.hOu });
            }
            Core.logger(`Iteration ${i++} completed`);
        }
        this.toggleBoost(BoostType.Reputation, false);
    }

    /** The rune for the Alchemy rep farm: Jera below rank 5, Fehu below rank 8, else Gebo. */
    private alchemyRune(): AlchemyRunes {
        const rank = this.factionRank("Alchemy");
        if (rank < 5) return AlchemyRunes.Jera;
        if (rank < 8) return AlchemyRunes.Fehu;
        return AlchemyRunes.Gebo;
    }

    /**
     * Farms Blacksmithing reputation to `rank`: with `useGold`, by turning in Gold Vouchers 500k
     * (quest 8737; it farms gold when you can't buy one), else with quests 2777 and 8736.
     */
    blacksmithingREP(rank = 10, useGold = false, _bulkFarmGold = false): void {
        if (this.factionRank("Blacksmithing") >= rank) return;

        this.toggleBoost(BoostType.Reputation);
        Core.equipClass(ClassType.Farm);
        Core.logger(`Farming rank ${rank}`);

        if (useGold) {
            Core.logger("Using Gold Method");

            Core.savedState(true, "alchemyacademy");
            // Count vouchers already in the bank before deciding how many to buy or whether to farm gold.
            Core.unbank("Gold Voucher 500k");
            while (!bot.shouldExit && this.factionRank("Blacksmithing") < rank) {
                Core.ensureAccept(8737);

                // Rep still needed for the *requested* rank.
                const targetRep = REP_THRESHOLDS.find(([r]) => r === Math.min(Math.max(rank, 1), 10))![1];
                const remainingRepXP = Math.max(0, targetRep - this.totalFactionRep("Blacksmithing"));
                // At least one voucher: a faction that isn't in the list yet (rank 0) reports 0 rep.
                const itemsNeeded = Math.max(1, Math.trunc((remainingRepXP + 999) / 1000)); // 1 voucher = 1000 rep, round up
                const currentQuantity = bot.inventory.getQuantity("Gold Voucher 500k");

                Core.logger(`Blacksmithing rep needed for rank ${rank}: ${remainingRepXP} (${itemsNeeded} vouchers, have ${currentQuantity})`);

                let toBuy = Math.max(0, Math.min(itemsNeeded - currentQuantity, Math.min(200, 300 - currentQuantity)));
                if (toBuy > 0) {
                    // Buy what the current gold covers; only farm gold when we can't afford a single voucher.
                    let affordable = Math.trunc(bot.player.gold / 500000);
                    if (affordable < 1) {
                        const chunk = Math.min(toBuy, 20);
                        Core.logger(`Not enough gold for a voucher, farming ${formatN0(chunk * 500000)} gold`);
                        this.gold(chunk * 500000);
                        affordable = Math.trunc(bot.player.gold / 500000);
                    }
                    toBuy = Math.min(toBuy, affordable);
                    if (toBuy > 0) Core.buyItem("alchemyacademy", 2036, "Gold Voucher 500k", currentQuantity + toBuy);
                }

                const turnIns = Math.min(itemsNeeded, bot.inventory.getQuantity("Gold Voucher 500k"));
                if (turnIns > 0) Core.ensureCompleteMulti(8737, turnIns);
            }

            this.toggleBoost(BoostType.Reputation, false);
            Core.savedState(false);
            Core.logger("Reputation boost deactivated and state saved.");
            return;
        }
        Core.logger("Using Non-Gold Method");

        Core.registerQuests(2777);
        while (!bot.shouldExit && this.factionRank("Blacksmithing") < 4 && !useGold) {
            Core.huntMonster("greenguardeast", "Wolf", "Furry Lost Sock", 2, { log: false });
            Core.huntMonster("greenguardwest", "Slime", "Slimy Lost Sock", 5, { log: false });
        }
        Core.cancelRegisteredQuests();

        Core.registerQuests(8736);
        Core.equipClass(ClassType.Solo);
        while (!bot.shouldExit && this.factionRank("Blacksmithing") < rank && !useGold) {
            Core.huntMonster("hydrachallenge", "Hydra Head 25", "Hydra Scale Piece", 75, { isTemp: false, log: false });
            Core.huntMonster("maul", "Creature Creation", "Creature Shard", 1, { isTemp: false, log: false });
            Core.huntMonster("towerofdoom", "Dread Klunk", "Monster Trophy", 15, { isTemp: false, log: false });
        }
        Core.cancelRegisteredQuests();
        this.toggleBoost(BoostType.Reputation, false);
    }

    /**
     * Farms Blade of Awe reputation (quest 2935 in /castleundead). With `farmBoA`, farms to rank 6
     * first if needed and buys the Blade of Awe from the museum.
     */
    bladeofAweREP(rank = 10, farmBoA = true): void {
        // Quests will now be done regardless of farmboa bool, purely to unlock them.
        this.unlockBoA();

        const targetRank = farmBoA && !Core.checkInventory(17585) ? 6 : rank;

        if (this.factionRank("Blade of Awe") < targetRank || (farmBoA && this.factionRank("Blade of Awe") < rank)) {
            Core.savedState(true, "castleundead");
            Core.equipClass(ClassType.Farm);
            Core.logger(`Farming rank ${this.factionRank("Blade of Awe") < targetRank ? targetRank : rank}`);

            Core.registerQuests(2935);
            // C# checks `targetRank` on both sides of the ||.
            while (!bot.shouldExit && (this.factionRank("Blade of Awe") < targetRank || (farmBoA && this.factionRank("Blade of Awe") < targetRank))) {
                if (Core.checkSaveState()) Core.executeSaveState();
                Core.killMonster("castleundead", "Enter", "Left", "Skeletal Viking");
            }
            Core.cancelRegisteredQuests();
            Core.savedState(false);
        }

        if (farmBoA && !Core.checkInventory(17585)) Core.buyItem("museum", 631, 17585);
    }

    /** Unlocks the Blade of Awe quests (2933 to 2937). */
    unlockBoA(): void {
        if (Core.isCompletedBefore(2937)) return;

        const step = (questId: number, title: string, classType: ClassType, farm: () => void): void => {
            if (!Core.isCompletedBefore(questId)) {
                Core.logger(`Doing Quest: [${questId}] - "${title}"`);
                Core.equipClass(classType);
                Core.ensureAccept(questId);
                farm();
                Core.ensureComplete(questId);
                Core.logger(`Completed Quest: [${questId}] - "${title}"`);
            } else Core.logger(`Already Completed: [${questId}] - "${title}"`);
        };

        step(2933, "Find the Stonewrit!", ClassType.Solo, () => Core.huntMonster("j6", "Sketchy Dragon", "Stonewrit Found!", 1, { isTemp: false }));
        step(2934, "Find the Handle!", ClassType.Solo, () => Core.huntMonster("gilead", "Fire Elemental", "Handle Found!", 1, { isTemp: false }));
        step(2935, "Find the Hilt!", ClassType.Farm, () => Core.huntMonster("castleundead", "Skeletal Viking", "Hilt Found!", 1, { isTemp: false }));
        step(2936, "Find the Blade!", ClassType.Farm, () => Core.huntMonster("hydra", "Hydra Head", "Blade Found!", 1, { isTemp: false }));
        if (!Core.isCompletedBefore(2937)) Core.resetQuest(2937);
        step(2937, "Find the Runes!", ClassType.Solo, () => Core.killEscherion("Runes Found!"));
    }

    /** Farms Chaos reputation to `rank` (quest 3594 in /mountdoomskull). */
    chaosREP(rank = 10): void {
        if (this.factionRank("Chaos") >= rank) return;

        Core.equipClass(ClassType.Farm);
        Core.savedState(true, "mountdoomskull");
        this.toggleBoost(BoostType.Reputation);
        Core.logger(`Farming rank ${rank}`);

        Core.registerQuests(3594); // Embrace Your Chaos 3594
        while (!bot.shouldExit && this.factionRank("Chaos") < rank) {
            if (Core.checkSaveState()) Core.executeSaveState();
            Core.killMonster("mountdoomskull", "b1", "Left", "*", null, 1, { log: false });
        }
        bot.wait.forQuestComplete(3594);
        Core.cancelRegisteredQuests();
        this.toggleBoost(BoostType.Reputation, false);
        Core.savedState(false);
    }

    /** Farms ChronoSpan reputation to `rank` (quest 2204 in /thespan). */
    chronoSpanREP(rank = 10): void {
        if (this.factionRank("ChronoSpan") >= rank) return;

        Core.equipClass(ClassType.Farm);
        this.toggleBoost(BoostType.Reputation);
        Core.logger(`Farming rank ${rank}`);

        Core.savedState(true, "thespan");
        Core.registerQuests(2204); // Do the 'do 2204
        while (!bot.shouldExit && this.factionRank("ChronoSpan") < rank) {
            if (Core.checkSaveState()) Core.executeSaveState();
            Core.killMonster("thespan", "r6", "Left", "Moglin Ghost", "Tin of Ghost Dust", 2, { log: false });
            Core.killMonster("thespan", "r4", "Left", "Minx Fairy", "8 oz Fairy Glitter", 3, { log: false });
            Core.killMonster("thespan", "r4", "Left", "Tog", "Tog Fang", 4, { log: false });
        }
        Core.cancelRegisteredQuests();
        this.toggleBoost(BoostType.Reputation, false);
        Core.savedState(false);
    }

    /** Farms DoomWood reputation to `rank` (quests 1151-1153 in /shadowfallwar). */
    doomWoodREP(rank = 10): void {
        if (this.factionRank("DoomWood") >= rank) return;

        Core.equipClass(ClassType.Farm);
        Core.savedState(true, "shadowfallwar");
        this.toggleBoost(BoostType.Reputation);
        Core.logger(`Farming rank ${rank}`);

        Core.registerQuests(1151, 1152, 1153); // Minion Morale 1151, Shadowfall is DOOMed 1152, Grave-lyn Danger, 1153
        while (!bot.shouldExit && this.factionRank("DoomWood") < rank) {
            if (Core.checkSaveState()) Core.executeSaveState();
            Core.killMonster("shadowfallwar", "Garden1", "Left", "*", null, 1, { log: false });
        }
        Core.cancelRegisteredQuests();
        this.toggleBoost(BoostType.Reputation, false);
        Core.savedState(false);
    }

    /** Farms Evil reputation to `rank` (switches your alignment to Evil). */
    evilREP(rank = 10): void {
        if (this.factionRank("Evil") >= rank) return;

        Core.changeAlignment(Alignment.Evil);
        Core.equipClass(ClassType.Farm);
        this.toggleBoost(BoostType.Reputation);
        Core.logger(`Farming rank ${rank}`);

        Core.savedState(true, "swordhavenbridge");
        Core.registerQuests(364); // Youthanize 364
        while (!bot.shouldExit && this.factionRank("Evil") < 4) {
            if (Core.checkSaveState()) Core.executeSaveState();
            Core.huntMonster("swordhavenbridge", "Slime", "Youthanize", 1, { log: false });
        }
        Core.cancelRegisteredQuests();
        Core.savedState(false);

        Core.savedState(true, Core.isMember ? "sleuthhound" : "castleundead");
        Core.registerQuests(367);
        while (!bot.shouldExit && this.factionRank("Evil") < rank) {
            if (Core.checkSaveState()) Core.executeSaveState();
            if (Core.isMember) {
                Core.ensureAccept(366);
                Core.huntMonster("sleuthhound", "Chair", "Chair", 4, { log: false });
                Core.huntMonster("sleuthhound", "Table", "Table", 2, { log: false });
                Core.huntMonster("sleuthhound", "Bookcase", "Bookcase", 1, { log: false });
                Core.ensureComplete(366);
            } else Core.killMonster("castleundead", "Enter", "Left", "*");
        }
        Core.cancelRegisteredQuests();
        this.toggleBoost(BoostType.Reputation, false);
    }

    /**
     * Farms Fishing reputation to `rank` with Fishing Dynamite. With `shouldDerp`, fishes until the
     * Derp Moosefish bites (achievement 14) instead. Trashes the bait and dynamite after with
     * `trashBait`.
     */
    fishingREP(rank = 10, shouldDerp = false, trashBait = true, _getBoosts = true): void {
        if (this.factionRank("Fishing") >= rank && !shouldDerp) {
            if (trashBait) Core.trashCan("Fishing Bait", "Fishing Dynamite");
            return;
        }

        if (!bot.reputation.factionList.some((f) => f.name === "Fishing")) {
            Core.trashCan("Fishing Bait", "Fishing Dynamite");
            this.getBaitandDynamite(0, 1);
        }

        let waitTimer = 3500;
        let successful = 1;
        let hitDerp = false;
        let derpFound = false;
        let receivedCastWait = false;
        let receivedCatchResult = false;

        Core.addDrop("Fishing Bait", "Fishing Dynamite");
        Core.equipClass(ClassType.Farm);
        Core.logger(`Farming Fishing rank ${rank}`);

        // C# FishingWaiter
        const off = bot.events.on("extensionPacket", (packet) => {
            const parameters = packet?.params;
            if (!parameters || parameters.type !== "json") return;
            const data = parameters.dataObj;
            if (!data) return;
            switch (String(data.cmd ?? "")) {
                case "castWait":
                    if (data.wait !== undefined && data.wait !== null) waitTimer = Math.trunc(Number(data.wait));
                    hitDerp = data.derp !== undefined && data.derp !== null && (data.derp === true || String(data.derp).toLowerCase() === "true");
                    receivedCastWait = true;
                    Core.logger(`Derp Moosefish: ${hitDerp ? "True" : "False"}, CatchTimer: ${waitTimer}ms`);
                    break;
                case "CatchResult":
                    receivedCatchResult = true;
                    if (!Array.isArray(data.catchResult)) return;
                    for (const c of data.catchResult) {
                        if (!c) continue;
                        const action = String(c.act ?? "");
                        if (action) Core.logger(action);
                        if (c.myRep !== undefined && c.myRep !== null) {
                            const myRep = Math.trunc(Number(c.myRep));
                            if (myRep !== 0) Core.logger(`${myRep}`);
                        }
                    }
                    break;
            }
        });

        try {
            while (!bot.shouldExit && (shouldDerp || this.factionRank("Fishing") < rank)) {
                if (Core.checkSaveState()) Core.executeSaveState();

                if (shouldDerp && (Core.hasAchievement(14) || Core.hasWebBadge("Derp Moosefish"))) {
                    derpFound = true;
                    break;
                }

                this.getBaitandDynamite(0, 50);

                Core.join("fishing");

                bot.wait.forCellChange("Enter");
                bot.wait.forTrue(() => bot.player.loaded, 20);

                Core.logger("Fishing With: Dynamite");

                while (!bot.shouldExit && Core.checkInventory("Fishing Dynamite") && (shouldDerp || this.factionRank("Fishing") < rank)) {
                    hitDerp = false;
                    receivedCastWait = false;
                    receivedCatchResult = false;

                    Core.sleep(1000);

                    Core.logger("Casting Dynamite...");
                    bot.send.packet("%xt%zm%FishCast%1%Dynamite%30%");

                    // Wait for the server to tell us the actual catch timer.
                    if (!bot.wait.forTrue(() => receivedCastWait, 10)) {
                        Core.logger("Timed out waiting for castWait packet.");
                        continue;
                    }

                    Core.logger(`Cast timer received: ${waitTimer}ms | Derp: ${hitDerp ? "True" : "False"}`);

                    // do not cast again until the server-provided timer has elapsed.
                    Core.sleep(waitTimer);

                    // Give the Derp state/packet a little extra time to settle.
                    if (shouldDerp) Core.sleep(1000);

                    Core.logger(`Getting fish... Derp flag: ${hitDerp}`);
                    bot.send.packet(`%xt%zm%getFish%1%${hitDerp}%`);

                    // Wait for the server to confirm the catch before evaluating the result.
                    if (!bot.wait.forTrue(() => receivedCatchResult, 10)) {
                        Core.logger("Timed out waiting for CatchResult; skipping result evaluation.");
                        continue;
                    }

                    if (shouldDerp && hitDerp) {
                        derpFound = true;
                        Core.logger("Derp Moosefish detected!");
                        break;
                    }

                    if (rank <= 10) Core.logger(`Dynamite cast completed [x${successful++}]`);
                }

                if (derpFound) break;
            }
        } finally {
            off();
            if (trashBait) Core.trashCan("Fishing Bait", "Fishing Dynamite");
        }
    }

    /** Farms Fishing Bait (Frogzards) and Fishing Dynamite (Slimes) with quest 1682. */
    getBaitandDynamite(fishingBaitQuant: number, fishingDynamiteQuant: number): void {
        if (Core.checkInventory("Fishing Bait", fishingBaitQuant) && Core.checkInventory("Fishing Dynamite", fishingDynamiteQuant)) return;
        Core.registerQuests(1682);

        const farmItem = (itemName: string, quantity: number, map: string, cell: string, pad: string, monster: string): void => {
            if (quantity <= 0) return;
            Core.killMonster(map, cell, pad, monster, itemName, quantity, { isTemp: false });
            bot.wait.forPickup(itemName);
        };

        farmItem("Fishing Bait", fishingBaitQuant, "greenguardwest", "West3", "Right", "Frogzard");
        farmItem("Fishing Dynamite", fishingDynamiteQuant, "greenguardwest", "West4", "Right", "Slime");

        Core.cancelRegisteredQuests();
        Core.logger("Returning to Fishing Map");
    }

    /** Fishes with dynamite until you have `quant` of the fish `itemId` (a reward of `quest`). */
    getFish(itemId: number, quant: number, quest: number): void {
        if (Core.checkInventory(itemId, quant)) return;

        const reward = Core.ensureLoad(quest)?.rewards.find((x) => x.id === itemId);
        if (reward) Core.farmingLogger(reward.name, quant);

        while (!bot.shouldExit && !Core.checkInventory(itemId, quant)) {
            if (!Core.checkInventory("Fishing Dynamite")) this.getBaitandDynamite(0, 20);

            Core.join("fishing");

            while (!bot.shouldExit && Core.checkInventory("Fishing Dynamite") && !Core.checkInventory(itemId, quant)) {
                const currentDynamiteQuant = bot.inventory.getQuantity("Fishing Dynamite");
                bot.send.packet("%xt%zm%FishCast%1%Dynamite%30%");
                Core.sleep(3500);
                // C# compares the count with itself minus 1 (never true, so always 2 s): wait for
                // the cast to use a stick instead.
                bot.wait.forTrue(() => bot.inventory.getQuantity("Fishing Dynamite") < currentDynamiteQuant, 20);
                Core.sendPackets("%xt%zm%getFish%1%false");
                Core.logger(`Dynamite: ${bot.inventory.getQuantity("Fishing Dynamite")} Fish: ${bot.tempInv.getQuantity(itemId)}/${quant}`);
            }
        }
    }

    /** The PvP amulets the Bludrut and Death Pit brawls equip, best first. */
    readonly acceptablePvPAmulets = ["Mithril PvP Amulet +15000", "Diamond PvP Amulet +5500", "Platinum PvP Amulet +5000"];

    /** Farms Death Pit Brawl reputation to `rank` (unlocking the farm quests 5155-5157 first). */
    deathPitBrawlREP(rank = 10): void {
        if (this.factionRank("Death Pit Brawl") >= rank) return;

        Core.logger(`Farming "DeathPitBrawlREP" rank ${rank}`);
        Core.addDrop("Death Pit Token");
        Core.equipClass(ClassType.Solo);
        this.toggleBoost(BoostType.Reputation);
        Core.toggleAggro(false);

        if (Core.isCompletedBefore(5157)) {
            Core.logger("Checking if farm quests are unlocked--");

            // Do You Even Brawl
            if (!Core.isCompletedBefore(5155)) {
                Core.ensureAccept(5155);
                this.deathPitToken("Death Pit Token", 1, false, true);
                Core.ensureComplete(5155);
            }

            // Flex For Hun'Gar
            if (!Core.isCompletedBefore(5156)) {
                Core.ensureAccept(5156);
                this.deathPitToken("Death Pit Token", 15, false, true);
                Core.ensureComplete(5156);
            }

            // Pummel For Hun'Gar
            if (!Core.isCompletedBefore(5157)) {
                Core.ensureAccept(5157);
                this.deathPitToken("Brawler Token", 3, true, true);
                this.deathPitToken("Restorer Token", 4, true, true);
                Core.ensureComplete(5157);
            }
        }

        Core.logger("Rep Time");
        this.runDeathPitBrawl();

        Core.cancelRegisteredQuests();
        this.toggleBoost(BoostType.Reputation, false);
        Core.toggleAggro(true);
    }

    /** C# `RunDeathPitBrawl`: brawl runs until the rank, then until you have `item`. */
    private runDeathPitBrawl(item: string | null = null, quant = 1, isTemp = false, rank = 10, _canSoloBoss = true, forStory = false): void {
        let runStarted = 0;

        if (!forStory) Core.registerQuests(...[5156, 5165].filter((q) => bot.quests.isUnlocked(q)));

        if (bot.map.name === "deathpitbrawl") {
            Core.logger("Started in PvP map, warping to whitemap first.");
            Core.join("whitemap");
        }

        const owned = [...bot.inventory.items, ...bot.bank.items];
        const amulet = this.acceptablePvPAmulets.find((name) => owned.some((i) => i?.name === name));
        if (amulet !== undefined) {
            Core.unbank(amulet);
            Core.equip(amulet);
        }

        if (item !== null) {
            Core.farmingLogger(item, quant, "RunDeathPitBrawl");
            if (!isTemp) Core.addDrop(item);
        }

        let runCount = 1;
        const elapsed = (): string => {
            const seconds = Math.trunc((Date.now() - runStarted) / 1000);
            const pad = (n: number) => String(n).padStart(2, "0");
            return `${pad(Math.trunc(seconds / 60) % 60)}:${pad(seconds % 60)}`;
        };

        const executeOneBrawlRun = (): void => {
            while (!bot.shouldExit && bot.map.name !== "deathpitbrawl") {
                bot.map.join("deathpitbrawl-999999", "Enter0", "Spawn", false, false);
                bot.wait.forMapLoad("deathpitbrawl");
            }

            const steps = (): boolean => {
                Core.pvpMove(5, "Morale0C", 228, 291);
                if (!bot.player.alive) return false;
                Core.pvpMove(4, "Morale0B", 936, 397);
                Core.pvpMove(7, "Morale0A", 946, 394);
                Core.pvpMove(9, "Crosslower", 948, 400);

                // Restorers
                Core.pvpMove(14, "Crossupper", 903, 324);

                Core.pvpMove(18, "Resource1A", 482, 295);
                Core.pvpKilling();
                if (!bot.player.alive) return false;

                Core.pvpMove(20, "Resource1B", 938, 400);
                Core.pvpKilling();
                if (!bot.player.alive) return false;

                Core.pvpMove(21, "Resource1A", 9, 435);
                Core.pvpMove(19, "Crossupper", 461, 315);
                Core.pvpMove(17, "Crosslower", 54, 339);

                // Brawlers
                Core.pvpMove(15, "Morale1A", 509, 286);
                Core.pvpKilling();
                if (!bot.player.alive) return false;

                Core.pvpMove(23, "Morale1B", 941, 406);
                Core.pvpKilling();
                if (!bot.player.alive) return false;

                Core.pvpMove(25, "Morale1C", 938, 405);
                Core.pvpKilling();
                if (!bot.player.alive) return false;

                // Captain
                Core.pvpMove(28, "Captain1", 943, 404);
                Core.pvpKilling();
                return bot.player.alive;
            };
            if (steps()) return;

            // RestartOnDeath
            if (!bot.shouldExit) {
                bot.wait.forTrue(() => bot.player.alive, 100);
                bot.map.join("battleon-100000", "Enter", "Spawn", false, false);
                bot.wait.forMapLoad("battleon");
                Core.sleep(1500);
            }
        };

        const returnToBattleonAndRestart = (): void => {
            while (!bot.shouldExit && bot.map.name !== "battleon") {
                bot.combat.cancelTarget();
                bot.map.join("battleon-100000", "Enter", "Spawn", false, false);
                bot.wait.forMapLoad("battleon");
                if (bot.map.name === "battleon") return;
                Core.sleep(1500);
            }
        };

        const quantity = (name: string): number => (isTemp ? bot.tempInv.getQuantity(name) : bot.inventory.getQuantity(name));

        // C#'s `goto Start` after each run: back to the rank loop.
        start: for (;;) {
            // 1) Faction Rank farming
            while (!bot.shouldExit && this.factionRank("Death Pit Brawl") < rank) {
                runStarted = Date.now();
                executeOneBrawlRun();
                Core.sleep(1500);
                Core.logger(`Run #${runCount++} completed in ${elapsed()}`);
                returnToBattleonAndRestart();
                continue start;
            }

            // 2) {item} farming (only AFTER rank is done)
            if (item !== null) {
                // C# sets canSoloBoss to false for the 2 temp items (it isn't read after).
                // C# precedence kept: (!ShouldExit && isTemp) ? !TempInv.Contains : !Inventory.Contains
                while (!bot.shouldExit && isTemp ? !bot.tempInv.contains(item, quant) : !bot.inventory.contains(item, quant)) {
                    const startQuant = quantity(item);

                    executeOneBrawlRun();

                    bot.wait.forTrue(() => quantity(item) > startQuant, 40);

                    // Wait for delayed PvP reward packet
                    let updatedQuant = quantity(item);
                    bot.log(`Updating ${item} Quant`);
                    while (!bot.shouldExit && updatedQuant <= startQuant) {
                        Core.sleep(500);
                        updatedQuant = quantity(item);
                        bot.sleep(500);
                        bot.log(`${item} Quant Update Failed`);
                    }
                    bot.log(`${item} Quant Updated`);
                    Core.farmingLogger(item, quant);

                    // C# stops a stopwatch that wasn't started here, so it reads 00:00.
                    Core.logger(`Run #${runCount++} completed in 00:00`);

                    returnToBattleonAndRestart();
                    continue start;
                }
            }
            break;
        }

        for (const reward of ["Yoshino's Citrine", "The Secret 4"]) if (item !== reward && bot.inventory.contains(reward)) Core.toBank(reward);
    }

    /**
     * Farms Death Pit Tokens (or the Brawler / Restorer Tokens) in /deathpitbrawl. Don't call it
     * with registered quests, or it technically never exits.
     */
    deathPitToken(item: string | null = "Death Pit Token", quant = 30, isTemp = false, forStory = false): void {
        if (Core.checkInventory(item, quant)) return;
        this.runDeathPitBrawl(item, quant, isTemp, 1, true, forStory);
    }

    /** Farms Glacera reputation to `rank` (quests 5597-5600 in /icewindwar). */
    glaceraREP(rank = 10): void {
        if (this.factionRank("Glacera") >= rank) return;

        if (!Core.isCompletedBefore(5601)) {
            Core.logger('Farming Quests are not unlocked, Please run: "Story/Glacera.cs"');
            return;
        }

        Core.equipClass(ClassType.Farm);
        Core.savedState(true, "icewindwar");
        this.toggleBoost(BoostType.Reputation);
        Core.logger(`Farming rank ${rank}`);

        if (Core.farmClass === "Generic")
            Core.logger("FarmClass not set in CBO (options > corebot > tab 2),\nso you'll be getting *very* low Rep Rates without a multi-target class.");

        Core.registerQuests(5597, 5598, 5599, 5600);
        while (!bot.shouldExit && this.factionRank("Glacera") < rank) {
            if (Core.checkSaveState()) Core.executeSaveState();
            Core.killMonster("icewindwar", "r5", "Left", "*", null, 1, { log: false });
        }
        Core.cancelRegisteredQuests();
        this.toggleBoost(BoostType.Reputation, false);
        Core.savedState(false);
    }

    /** Farms Good reputation to `rank` (PoisonForest after its story, else the low-rep quests). */
    goodREP(rank = 10): void {
        if (this.factionRank("Good") >= rank) return;

        Core.changeAlignment(Alignment.Good);
        Core.equipClass(ClassType.Farm);

        if (!Core.isCompletedBefore(1955)) {
            Core.logger(
                "/PoisonForest story not complete! You'll be doing the lower reputation reward quest. (just run the GoodRep.cs it will complete the story for you, then come back to this)",
            );
            Core.savedState(
                true,
                !Core.isCompletedBefore(1955) ? (this.factionRank("Good") < 4 ? "swordhavenbridge" : !Core.isMember ? "castleundead" : "sewer") : "PoisonForest",
            );
        }
        this.toggleBoost(BoostType.Reputation);
        Core.logger(`Farming rank ${rank}`);
        if (Core.isCompletedBefore(1955)) {
            Core.registerQuests(1952);
            while (!bot.shouldExit && this.factionRank("Good") < rank) {
                if (Core.checkSaveState()) Core.executeSaveState();
                // HuntMonster navigates to Burning Loyalist's cell once, then stays there
                Core.huntMonster("PoisonForest", "Burning Loyalist");
            }
            Core.cancelRegisteredQuests();
        } else {
            Core.registerQuests(369); // That Hero Who Chases Slimes 369
            while (!bot.shouldExit && this.factionRank("Good") < 4) {
                if (Core.checkSaveState()) Core.executeSaveState();
                Core.killMonster("swordhavenbridge", "Bridge", "Left", "*");
            }
            Core.cancelRegisteredQuests();

            Core.registerQuests(Core.isMember ? 371 : 372); // Rumble with Grumble 371, Tomb with a View 372
            while (!bot.shouldExit && this.factionRank("Good") < rank) {
                if (Core.checkSaveState()) Core.executeSaveState();
                if (!Core.isMember) Core.killMonster("castleundead", "Enter", "Spawn", "*");
                else Core.killMonster("sewer", "End", "Left", "Grumble");
            }
        }
        Core.cancelRegisteredQuests();
        this.toggleBoost(BoostType.Reputation, false);
        Core.savedState(false);
    }

    /** Farms Loremaster reputation to `rank` (members: /druids after rank 3; others: /wardwarf). */
    loremasterREP(rank = 10): void {
        if (this.factionRank("Loremaster") >= rank) return;

        this.experience(15);
        Core.equipClass(ClassType.Farm);
        this.toggleBoost(BoostType.Reputation);
        Core.logger(`Farming rank ${rank}`);

        Core.equipClass(ClassType.Farm);
        if (Core.isMember) {
            Core.savedState(true, "wardwarf");
            if (this.factionRank("Loremaster") < 3) {
                if (Core.checkSaveState()) Core.executeSaveState();
                Core.logger("Geting r3 Loremaster rep for the member quests");
                Core.registerQuests(7505);
                while (!bot.shouldExit && this.factionRank("Loremaster") < 3) {
                    Core.huntMonster("wardwarf", "Drow Assassin", "Poisoned Dagger", 4, { log: false });
                    Core.huntMonster("wardwarf", "D'wain Jonsen", "Scroll: Opportunity's Strike", 1, { log: false });
                    bot.wait.forActionCooldown("tryQuestComplete");
                    bot.wait.forActionCooldown("acceptQuest");
                    bot.wait.forQuestAccept(7505);
                }
            }
            Core.savedState(false);

            if (!Core.isCompletedBefore(3032)) {
                Core.logger("Unlocking farming quest.");

                Core.ensureAccept(3029); // Rosetta Stones 3029
                Core.killMonster("druids", "r2", "Left", "Void Bear", "Voidstone", 6);
                Core.ensureComplete(3029);

                Core.ensureAccept(3030); // Cull the Foot Soldiers 3030
                Core.killMonster("druids", "r6", "Left", "Void Larva", "Void Larvae Death Cry", 4);
                Core.ensureComplete(3030);

                Core.ensureAccept(3031); // Bad Vibes 3031
                Core.killMonster("druids", "r6", "Left", "Void Ghast", "Ghast's Death Cry", 4);
                Core.ensureComplete(3031);

                Core.ensureAccept(3032); // Quite the Problem 3032
                Core.killMonster("druids", "r5", "Left", "Young Void Giant", null, 1, { log: false });
                Core.ensureComplete(3032);
            }
            Core.logger("Member perquisite quests finished.");

            Core.savedState(true, "druids");
            Core.equipClass(ClassType.Solo);
            Core.registerQuests(3032);
            while (!bot.shouldExit && this.factionRank("Loremaster") < rank) {
                if (Core.checkSaveState()) Core.executeSaveState();
                Core.killMonster("druids", "r5", "Left", "Young Void Giant", null, 1, { log: false });
                bot.wait.forActionCooldown("tryQuestComplete");
                bot.wait.forActionCooldown("acceptQuest");
                bot.wait.forQuestAccept(3032);
            }
            Core.savedState(false);
        } else {
            Core.registerQuests(7505); // Studying the Rogue 7505
            Core.savedState(true, "whitemap");
            while (!bot.shouldExit && this.factionRank("Loremaster") < rank) {
                if (Core.checkSaveState()) Core.executeSaveState();
                Core.huntMonster("wardwarf", "Drow Assassin", "Poisoned Dagger", 4, { log: false });
                Core.huntMonster("wardwarf", "D'wain Jonsen", "Scroll: Opportunity's Strike", 1, { log: false });
                bot.wait.forActionCooldown("tryQuestComplete");
                bot.wait.forActionCooldown("acceptQuest");
                bot.wait.forQuestAccept(7505);
            }
        }

        bot.quests.unregisterQuests(3032, 7505);
        this.toggleBoost(BoostType.Reputation, false);
        Core.savedState(false);
    }

    /** Farms Mythsong reputation to `rank` (quest 4829 in /beehive). */
    mythsongREP(rank = 10): void {
        if (this.factionRank("Mythsong") >= rank) return;

        if (!bot.quests.isUnlocked(4829)) {
            Core.logger("Can't do farming quest (Do Lord of Chaos Kimberly)", "", { messageBox: true });
            return;
        }

        Core.equipClass(ClassType.Farm);
        Core.savedState(true, "beehive");
        this.toggleBoost(BoostType.Reputation);
        Core.logger(`Farming rank ${rank}`);

        Core.registerQuests(4829); // Sugar, Sugar 4829
        while (!bot.shouldExit && this.factionRank("Mythsong") < rank) {
            if (Core.checkSaveState()) Core.executeSaveState();
            Core.huntMonster("beehive", "Stinger", "Honey Gathered", 10, { log: false });
        }
        Core.cancelRegisteredQuests();
        this.toggleBoost(BoostType.Reputation, false);
        Core.savedState(false);
    }

    /** Farms SpellCrafting reputation to `rank` (Ember Ink to rank 4, then Hallow Ink). */
    spellCraftingREP(rank = 10): void {
        if (this.factionRank("SpellCrafting") >= rank) return;

        Core.addDrop("Mystic Quills", "Mystic Parchment");
        Core.equipClass(ClassType.Farm);
        this.toggleBoost(BoostType.Reputation);
        Core.logger(`Farming rank ${rank}`);

        if (this.factionRank("SpellCrafting") === 0) {
            if (Core.checkSaveState()) Core.executeSaveState();
            Core.ensureAccept(2260);
            Core.getMapItem(1920, 1, "dragonrune");
            Core.huntMonster("castleundead", "Skeletal Warrior", "Arcane Parchment", 13, { log: false });
            Core.ensureComplete(2260);
        }

        if (this.factionRank("SpellCrafting") < 4) {
            Core.joinSWF("mobius", "ChiralValley/town-Mobius-21Feb14.swf", "Slugfit", "Bottom");
            Core.huntMonster("mobius", "Slugfit", "Mystic Quills", 10, { isTemp: false });
            Core.buyItem("dragonrune", 549, "Ember Ink", 50);
            while (!bot.shouldExit && Core.checkInventory("Ember Ink") && this.factionRank("SpellCrafting") < 4) Core.chainComplete(2299);
        }

        Core.savedState(true, "underworld");
        while (!bot.shouldExit && this.factionRank("SpellCrafting") < rank) {
            if (Core.checkSaveState()) Core.executeSaveState();
            Core.huntMonster("underworld", "Skull Warrior", "Mystic Parchment", 10, { isTemp: false });
            Core.buyItem("dragonrune", 549, "Hallow Ink", 50);
            while (!bot.shouldExit && Core.checkInventory("Hallow Ink") && this.factionRank("SpellCrafting") < rank) Core.chainComplete(2322);
        }
        Core.sellItem("Ember Ink", 1, { all: true });
        Core.sellItem("Hallow Ink", 1, { all: true });

        this.toggleBoost(BoostType.Reputation, false);
        Core.savedState(false);
    }

    /** Farms Swordhaven reputation to `rank` (the /castle quests). */
    swordhavenREP(rank = 10): void {
        if (this.factionRank("Swordhaven") >= rank) return;

        Core.equipClass(ClassType.Farm);
        Core.savedState(true, "castle");
        this.toggleBoost(BoostType.Reputation);
        Core.logger(`Farming rank ${rank}`);

        while (!bot.shouldExit && this.factionRank("Swordhaven") < rank) {
            if (Core.checkSaveState()) Core.executeSaveState();
            Core.ensureAcceptmultiple(3065, 3066, 3067, 3070, 3085, 3086, 3087);
            Core.huntMonster("castle", "Castle Spider", "Eradicated Arachnid", 10);
            Core.huntMonster("castle", "Castle Spider", "Castle Spider Silk", 8);
            Core.huntMonster("castle", "Castle Spider", "Castle Spider Silk Yarn", 2);
            Core.huntMonster("castle", "Castle Wraith", "Castle Wraith Defeated", 10);
            Core.huntMonster("castle", "Castle Wraith", "Jarred Wraith", 5);
            Core.huntMonster("castle", "Castle Wraith", "Castle Wraith Wool", 2);
            Core.huntMonster("castle", "Gargoyle", "Stony Plating", 6);
            Core.huntMonster("castle", "Gargoyle", "Gargoyle Gems", 2);
            Core.huntMonster("castle", "Dungeon Fiend", "Dungeon Fiend Hair Bow", 5);
            Core.huntMonster("castle", "Dungeon Fiend", "Dungeon Fiend Bow Tie", 5);
            Core.huntMonster("castle", "Dungeon Fiend", "Dungeon Fiend Textiles", 2);
            Core.ensureCompleteQuests(3065, 3066, 3067, 3070, 3085, 3086, 3087);
        }
        Core.cancelRegisteredQuests();
        this.toggleBoost(BoostType.Reputation, false);
        Core.savedState(false);
    }

    /** Farms Vampire reputation to `rank` (Twisted Paw in /safiria). */
    vampireREP(rank = 10): void {
        if (this.factionRank("Vampire") >= rank) return;

        if (!bot.quests.isUnlocked(522)) {
            Core.logger("Can't do farming quest [Twisted Paw] (/safiria)", "", { messageBox: true });
            return;
        }
        Core.equipClass(ClassType.Solo);
        Core.savedState(true, "safiria");
        this.toggleBoost(BoostType.Reputation);
        Core.logger(`Farming rank ${rank}`);

        Core.registerQuests(522);
        Core.removeDrop("Old Moon");
        while (!bot.shouldExit && this.factionRank("Vampire") < rank) {
            if (Core.checkSaveState()) Core.executeSaveState();
            Core.huntMonster("safiria", "Twisted Paw", "Twisted Paw's Head", 1, { log: false });
            bot.wait.forActionCooldown("tryQuestComplete");
        }
        Core.cancelRegisteredQuests();
        this.toggleBoost(BoostType.Reputation, false);
        Core.savedState(false);
    }

    static readonly maxFactionRep = 302500;

    /** Rank of a faction (0 if you have no reputation in it). */
    factionRank(faction: string): number {
        if (!faction || faction.trim().length === 0) return 0;
        return Core.factionRank(faction);
    }

    /** Your reputation in the faction for its current rank (0 without it). */
    factionRep(faction: string): number {
        return bot.reputation.factionList.find((f) => equalsIgnoreCase(f.name, faction))?.rep ?? 0;
    }

    /** Your total reputation in the faction: its rank's threshold plus the rep in that rank. */
    totalFactionRep(faction: string): number {
        if (!faction || faction.trim().length === 0) return 0;
        const factionObj = bot.reputation.factionList.find((f) => equalsIgnoreCase(f.name, faction));
        if (!factionObj) return 0;

        const rank = Math.min(Math.max(bot.reputation.getRank(faction), 1), 10);
        const threshold = REP_THRESHOLDS.find(([r]) => r === rank);
        if (!threshold) return 0;
        return threshold[1] + factionObj.rep;
    }

    /** Unlocks the Undead Giant quest (178) by completing 183, 176 and 177. */
    undeadGiantUnlock(): void {
        // 177 unlocks 178; 178 itself is not complete until the first Undead Giant turn-in.
        if (!Core.isCompletedBefore(177)) {
            Core.logger("Unlocking farm quest.");
            Core.ensureAccept(183);
            Core.killMonster("portalundead", "Enter", "Left", "Skeletal Fire Mage", "Defeated Fire Mage", 4, { log: false });
            Core.ensureComplete(183);

            Core.ensureAccept(176);
            Core.huntMonster("swordhavenundead", "Skeletal Soldier", "Slain Skeletal Soldier", 5, { log: false });
            Core.ensureComplete(176);

            Core.ensureAccept(177);
            Core.huntMonster("swordhavenundead", "Skeletal Ice Mage", "Frozen Bonehead", 6, { log: false });
            Core.ensureComplete(177);
        }
    }

    /**
     * Farms XP ("XP") or reputation (any other `type`) 10-minute boosts with the fishing quests
     * 1614 / 1615. With `doOnce`, does one turn-in whatever you have.
     */
    getBoost(type: string, quantity = 1, doOnce = false): void {
        const quest = type.toLowerCase() === "xp" ? 1614 : 1615;
        const itemId = quest === 1614 ? 10850 : 10997;
        const boostName = quest === 1614 ? "XP Boost! (10 min)" : "REPUTATION Boost! (10 min)";

        const boostItem = Core.ensureLoad(quest)?.rewards.find((x) => x.name === boostName);
        if (boostItem && Core.checkInventory(boostItem.name, quantity) && !doOnce) return;

        Core.farmingLogger(boostName, quantity);
        Core.addDrop("Fishing Dynamite", boostItem?.name ?? boostName);
        Core.equipClass(ClassType.Farm);
        if (this.factionRank("Fishing") < 2) {
            Core.logger("Fishing Rank 2 required for Boost farm quest.");
            this.fishingREP(2, false, false, false);
        }
        if (!Core.isCompletedBefore(1615)) {
            Core.ensureAccept(1614);
            this.getFish(10850, 30, 1614);
            Core.huntMonster("Greenguardwest", "Slime", "Slime Sauce", 1, { log: false });
            Core.ensureComplete(1614);
        }

        while (!bot.shouldExit && (!Core.checkInventory(boostName, quantity) || doOnce)) {
            Core.ensureAccept(quest);
            this.getFish(itemId, quest === 1614 ? 30 : 5, quest);
            if (quest === 1614) Core.killMonster("greenguardwest", "West4", "Right", "Slime", "Slime Sauce");
            else Core.huntMonster("Greenguardwest", "Frogzard", "Greenguard Seal", 1, { log: false });
            bot.wait.forPickup(quest === 1614 ? "Slime Sauce" : "Greenguard Seal");
            Core.ensureComplete(quest);
        }

        if (!doOnce) Core.trashCan("Fishing Bait", "Fishing Dynamite");
    }
}
