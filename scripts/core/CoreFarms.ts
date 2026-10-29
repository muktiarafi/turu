/*
name: CoreFarms
description: Port of the parts of Scripts/CoreFarms.cs the Nation scripts use (gold, experience,
    Good and Vampire reputation).
tags: core
*/
/**
 * CoreFarms — partial TypeScript port of `Scripts/CoreFarms.cs`.
 *
 * Only the members reachable from the ported scripts are here (Gold, Experience, IcestormArena,
 * GoodREP, VampireREP and their helpers); port more from the C# file as scripts need them.
 */

import { bot, BoostType } from "turu";
import { Alignment, ClassType, Core } from "./CoreBots";

/** C# `Skua.Core.Models.BoostType` (the engine's, re-exported for scripts that import it here). */
export { BoostType };

/** "1,500,000" (C# `{n:N0}`). */
function formatN0(value: number): string {
    return String(Math.trunc(value)).replace(/\B(?=(\d{3})+(?!\d))/g, ",");
}

export class CoreFarms {
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
                    if (Core.cboBool("doClassBoost") !== true || bot.boosts.uses(BoostType.Class) || Core.checkClassRank(true) === 10) return;
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
            Core.sellItem("Berserker Bunny", 1, true);
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

    /** Rank of a faction (0 if you have no reputation in it). */
    factionRank(faction: string): number {
        if (!faction || faction.trim().length === 0) return 0;
        return Core.factionRank(faction);
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
}
