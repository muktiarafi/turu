/*
name: null
description: null
tags: null
*/
// Port of Scripts/Legion/CoreLegion.cs.
// Not ported (FarmerJoe's DoAll doesn't reach them): EmblemofDage, DarkToken, DiamondTokenofDage, LegionRound4Medal, ApprovalAndFavor, LTCastle.

import { bot } from "turu";
import type { QuestItem } from "turu";
import { CoreAdvanced } from "../core/CoreAdvanced";
import { ClassType, Core } from "../core/CoreBots";
import type { QuestData } from "../core/CoreBots";
import { CoreFarms } from "../core/CoreFarms";
import { CoreStory } from "../core/CoreStory";

const Farm = new CoreFarms();
const Story = new CoreStory();
const Adv = new CoreAdvanced();

export class CoreLegion {
    legionMedals: string[] = [
        "Legion Round 1 Medal",
        "Legion Round 2 Medal",
        "Legion Round 3 Medal",
        "Legion Round 4 Medal",
    ];

    boneSigil(quant = 1): void {
        if (Core.checkInventory("Bone Sigil", quant))
            return;

        Core.equipClass(ClassType.Farm);

        Core.farmingLogger("Bone Sigil", quant);
        Core.addDrop("Bone Sigil");
        Core.registerQuests(6739);
        while (!bot.shouldExit && !Core.checkInventory("Bone Sigil", quant)) {
            Core.huntMonster("legionarena", "Legion Gladiator", "Legion Grunt Defeated", 5);
            bot.wait.forPickup("Bone Sigil");
        }
        Core.cancelRegisteredQuests();
    }

    soulForgeHammer(): void {
        if (Core.checkInventory("SoulForge Hammer"))
            return;

        Core.equipClass(ClassType.Solo);

        Core.addDrop("SoulForge Hammer");
        Core.ensureAccept(2741);
        Core.huntMonster("forest", "Zardman Grunt", "Zardman's StoneHammer", undefined, { isTemp: false });
        if (Core.checkInventory(319))
            Core.buyItem("swordhaven", 179, "Iron Hammer");
        else
            Core.huntMonster("battleundera", "Skeletal Warrior", "Iron Hammer", undefined, { isTemp: false });
        Core.huntMonster("bludrut", "Rock Elemental", "Elemental Rock Hammer", undefined, { isTemp: false });
        Core.ensureComplete(2741);
        bot.wait.forPickup("SoulForge Hammer");
    }

    farmLegionToken(quant = 50000): void {
        //banking Lts as ae fucked the quant when updating teh cap
        // if (!Bot.Bank.Contains("Legion Token"))
        // {
        //     Core.OneTimeMessage("Legion Token stack fix", "Banking [then unbanking if farm is needed] LTs\n" +
        //                         "as when AE updated the cap, they broke shit", messageBox: false);

        //     Core.ToBank("Legion Token");
        // }

        if (Core.checkInventory("Legion Token", quant))
            return;

        this.joinLegion();

        this.ltBrightParagon(quant);
        this.ltArcaneParagon(quant);
        this.ltShogunParagon(quant);
        this.ltFestiveParagonDracolichRider(quant);
        this.ltParagon(quant);
        this.ltMountedParagonPet(quant);
        this.ltThanatosParagon(quant);
        this.ltAscendedParagon(quant);
        this.ltDreadnaughtParagon(quant);
        this.ltHolidayParagon(quant);
        this.ltHardCoreParagon(quant);
        this.ltuw3017(quant);
        this.ltInfernalLegionBetrayal(quant);
        this.ltFirstClassEntertainment(quant, true, 4, true);
        this.ltDreadrock(quant);
    }

    ltHardCoreParagon(quant = 50000): void {
        if (
            Core.checkInventory("Legion Token", quant)
            || !Core.checkInventory("Hardcore Paragon Pet")
        )
            return;
        Core.bankingBlackList.push("Legion Token");
        Core.equipClass(ClassType.Solo);

        Core.farmingLogger("Legion Token", quant);
        Core.addDrop(...Core.questRewards(3393, 3394));

        if (!bot.quests.isDailyComplete(3394)) {
            Core.ensureAccept(3394);
            Core.huntMonster("chaosboss", "Ultra Chaos Warlord", "Chaorrupted Dark Fire", 20, { isTemp: false });
            Core.ensureComplete(3394);
        }

        // A Single Rib
        Core.registerQuests(3393);
        while (!bot.shouldExit && !Core.checkInventory("Legion Token", quant))
            Core.huntMonster("doomvault", "Binky", "Dark Unicorn Rib", undefined, { isTemp: false });
        Core.cancelRegisteredQuests();
        Core.toBank(...Core.questRewards(3393, 3394));
    }

    ltInfernalLegionBetrayal(quant = 50000): void {
        if (
            Core.checkInventory("Legion Token", quant) || !Core.checkInventory("Infernal Caladbolg")
        )
            return;

        this.joinLegion();

        Core.equipClass(ClassType.Farm);

        Core.farmingLogger("Legion Token", quant);
        Core.addDrop("Legion Token");

        Core.registerQuests(
            ...Core.checkInventory("Shogun Paragon Pet") ? [ 3722, 5755 ] : [ 3722 ]);
        while (!bot.shouldExit && !Core.checkInventory("Legion Token", quant)) {
            Core.huntMonster("fotia", "Fotia Elemental", "Betrayer Extinguished", 5);
            Core.huntMonster("evilwardage", "Dreadfiend of Nulgath", "Fiend Felled", 2);
        }
        Core.cancelRegisteredQuests();
    }

    ltuw3017(quant = 50000): void {
        if (Core.checkInventory("Legion Token", quant) || !Core.checkInventory("UW3017 Pet"))
            return;

        this.joinLegion();

        Core.equipClass(ClassType.Farm);

        Core.farmingLogger("Legion Token", quant);
        Core.addDrop("Legion Token");
        Core.registerQuests(5738);
        while (!bot.shouldExit && !Core.checkInventory("Legion Token", quant)) {
            Core.huntMonster("underworld", "Bloodfiend", "Foreign Weapon", 20);
            Core.huntMonster("underworld", "Bloodfiend", "Foreign Equipment", 20);
            Core.huntMonster("underworld", "Bloodfiend", "Unknown Substance", 20);
        }
        Core.cancelRegisteredQuests();
    }

    ltHolidayParagon(quant = 50000): void {
        if (
            Core.checkInventory("Legion Token", quant)
            || !Core.checkInventory("Holiday Paragon Pet")
        )
            return;

        this.joinLegion();

        Core.equipClass(ClassType.Farm);

        Core.farmingLogger("Legion Token", quant);
        Core.addDrop("Legion Token");
        Core.registerQuests(3256);
        while (!bot.shouldExit && !Core.checkInventory("Legion Token", quant)) {
            Core.huntMonster("prison", "King Alteon's Knight", "Spirit of Loyalty", 6);
            Core.huntMonster("battlewedding", "Silver Knight", "Spirit of Love", 6);
            Core.huntMonster("lycan", "Lycan Knight", "Spirit of Good Will", 6);
        }
        Core.cancelRegisteredQuests();
    }

    ltFestiveParagonDracolichRider(quant = 50000): void {
        if (
            Core.checkInventory("Legion Token", quant)
            || !Core.checkInventory("Festive Paragon Dracolich Rider")
        )
            return;

        this.joinLegion();

        Core.equipClass(ClassType.Farm);

        Core.farmingLogger("Legion Token", quant);
        Core.addDrop("Legion Token");
        Core.registerQuests(3968, 3969);
        while (!bot.shouldExit && !Core.checkInventory("Legion Token", quant)) {
            Core.killMonster("frozenruins", "r1", "Left", "*");
            if (Core.checkInventory("Legion Token", quant))
                break;
        }
        bot.wait.forPickup("Legion Token");
        Core.cancelRegisteredQuests();
    }

    ltBrightParagon(quant = 50000): void {
        if (
            Core.checkInventory("Legion Token", quant) || !Core.checkInventory("Bright Paragon Pet")
        )
            return;

        this.joinLegion();

        Core.equipClass(ClassType.Farm);

        Core.farmingLogger("Legion Token", quant);
        Core.addDrop("Legion Token", "Legion Token Pile");
        Core.registerQuests(4704, 4703);
        Core.configureAggro();
        Core.killMonster("brightfortress", "r3", "Right", "*", "Legion Token", quant, { isTemp: false });
        Core.configureAggro(false);
        Core.cancelRegisteredQuests();
    }

    ltShogunParagon(quant = 50000, DoClearaPath = false, Logger = false): void {
        if (Core.checkInventory("Legion Token", quant)) {
            Core.farmingLogger("Legion Token", quant);
            return;
        }

        // [PetID, MainQuestID, SideQuestID]
        const questData: [number, number, number][] = [
            [84900, 9649, 9648], [84899, 9646, 9645], [84965, 9663, 9662],
            [49926, 7073, 7072], [47578, 6750, 6749], [47614, 6756, 6754],
            [38792, 5756, 5754], [38621, 5755, 5753],
        ];

        const ownedItemIds = new Set(bot.inventory.items.concat(bot.bank.items).map(i => i.id));
        const [PetID, MainQuestID, SideQuestID] = questData.find(q => ownedItemIds.has(q[0])) ?? [0, 0, 0];
        if (PetID === 0) {
            Core.logger("No Pet owned for 'Time for Some Spring Cleaning' or pet missing from our list");
            return;
        }

        const questList: QuestData[] = [];

        const mainQuest = Core.ensureLoad(MainQuestID);
        if (mainQuest.id === 0) {
            Core.logger(`MainQuestID ${MainQuestID} missing from QuestData.json!`, undefined, { messageBox: true });
            return;
        }
        questList.push(mainQuest);

        if (DoClearaPath) {
            const sideQuest = Core.ensureLoad(SideQuestID);
            if (sideQuest.id === 0) {
                Core.logger(`SideQuestID ${SideQuestID} missing from QuestData.json!`, undefined, { messageBox: true });
                return;
            }
            questList.push(sideQuest);
        }

        for (const quest of questList)
            Core.addDrop(...new Set(quest.rewards.filter(r => r != null && r.quantity < r.maxStack).map(r => r.name)));

        bot.log(`✔️ Quest Pet Owned\nPetID: [${PetID}]\nQuestID(s): [${(questList.map(q => q.id)).join(", ")}]`);

        const requirements = new Map<number, [QuestItem, number]>();
        for (const quest of questList)
            for (const req of quest.requirements)
                if (req != null && req.id !== 0 && !requirements.has(req.id))
                    requirements.set(req.id, [req, req.quantity]);

        Core.equipClass(ClassType.Farm);
        Core.farmingLogger("Legion Token", quant);
        Core.addDrop("Legion Token");
        Core.registerQuests(...questList.map(q => q.id));

        while (!bot.shouldExit && !Core.checkInventory("Legion Token", quant)) {
            for (const [questItem, itemQuant] of requirements.values()) {
                if (bot.tempInv.contains(questItem.name, itemQuant))
                    continue;

                const femmeSoul = questItem.name === "Femme Cult Worshipper's Soul";

                Core.killMonster("fotia", femmeSoul ? "r5" : "Enter", femmeSoul ? "Left" : "Spawn", femmeSoul ? "Femme Cult Worshiper" : "*", questItem.name, itemQuant, { log: Logger });

                if (Core.checkInventory("Legion Token", quant)) {
                    Core.logger("Legion Tokens maxed!");
                    break;
                }
            }
        }

        Core.cancelRegisteredQuests();
    }

    ltParagon(quant = 50000, FromStandAlone = false): void {
        if (Core.checkInventory("Legion Token", quant))
            return;

        if (!Core.checkInventory(11260)) {
            if (FromStandAlone)
                Core.logger(
                    "Missing `Paragon Pet` (the pets actual name.. not just \a paragon pet\")");
            return;
        }

        this.joinLegion();

        Core.farmingLogger("Legion Token", quant);
        Core.registerQuests(1703);
        Core.addDrop("Legion Token");
        Core.addDrop(300, 11189, 11190);
        while (!bot.shouldExit && !Core.checkInventory("Legion Token", quant))
            Core.killMonster("lair", "End", "Right", "Red Dragon");
        Core.cancelRegisteredQuests();
        Core.toBank(11189, 11190);
    }

    ltFirstClassEntertainment(quant = 50000, onlyWithParty = false, partySize = 4, ReturnIfNoPeople = false): void {
        if (Core.checkInventory("Legion Token", quant))
            return;

        this.joinLegion();

        Core.join("legionarena", undefined, undefined, { publicRoom: true });
        if (bot.map.playerCount < partySize && onlyWithParty) {
            Core.join("legionarena", undefined, undefined, { publicRoom: true, ignoreCheck: true });
            if (ReturnIfNoPeople && bot.map.playerCount < partySize)
                return;
            while (!bot.shouldExit && bot.map.playerCount < partySize) { bot.sleep(500); }
            Core.logger(`Party gathered [${bot.map.playerNames!.length}/${partySize}]`);
        }

        Core.equipClass(ClassType.Solo);

        Core.farmingLogger("Legion Token", quant);
        Core.registerQuests(6743);
        Core.addDrop("Legion Token");
        Core.killMonster("legionarena", "Boss", "Left", "Legion Fiend Rider", "Legion Token", quant, { isTemp: false });
        Core.cancelRegisteredQuests();
    }

    ltDreadrock(quant = 50000): void {
        if (Core.checkInventory("Legion Token", quant))
            return;

        this.joinLegion();
        Adv.buyItem("underworld", 216, "Undead Champion");

        Core.equipClass(ClassType.Farm);

        Core.farmingLogger("Legion Token", quant);
        Core.registerQuests(4849);
        Core.addDrop("Legion Token");
        Core.killMonster("dreadrock", "r3", "Bottom", "*", "Legion Token", quant, { isTemp: false });
        Core.cancelRegisteredQuests();
    }

    ltArcaneParagon(quant = 50000): void {
        if (
            Core.checkInventory("Legion Token", quant) || !Core.checkInventory("Arcane Paragon Pet")
        )
            return;

        this.joinLegion();

        Core.equipClass(ClassType.Farm);

        Core.farmingLogger("Legion Token", quant);
        Core.addDrop(
            "Legion Token", 
            "Granite Dracolich Soul", 
            "Tempest Dracolich Soul", 
            "Deluge Dracolich Soul", 
            "Inferno Dracolich Soul");
        Core.registerQuests(4896);
        while (!bot.shouldExit && !Core.checkInventory("Legion Token", quant)) {
            Core.huntMonster("dragonheart", "Granite Dracolich", "Granite Dracolich Soul", 4, { isTemp: false });
            Core.huntMonster("dragonheart", "Tempest Dracolich", "Tempest Dracolich Soul", 4, { isTemp: false });
            Core.huntMonster("dragonheart", "Inferno Dracolich", "Inferno Dracolich Soul", 4, { isTemp: false });
            Core.huntMonster("dragonheart", "Deluge Dracolich", "Deluge Dracolich Soul", 4, { isTemp: false });
        }
        Core.cancelRegisteredQuests();
    }

    ltThanatosParagon(quant = 50000): void {
        if (
            Core.checkInventory("Legion Token", quant)
            || !Core.checkInventory("Thanatos Paragon Pet")
        )
            return;

        this.joinLegion();

        Core.equipClass(ClassType.Farm);

        Core.farmingLogger("Legion Token", quant);
        Core.addDrop("Legion Token");
        Core.registerQuests(4100);
        Core.killMonster("dragonheart", "r6", "Right", "Zombie Dragon", "Legion Token", quant, { isTemp: false });
        Core.cancelRegisteredQuests();
    }

    ltDreadnaughtParagon(quant = 50000): void {
        if (
            Core.checkInventory("Legion Token", quant)
            || !Core.checkInventory("Paragon Dreadnaught Pet")
        )
            return;

        Core.equipClass(ClassType.Farm);

        Core.farmingLogger("Legion Token", quant);
        Core.addDrop("Legion Token");
        Core.registerQuests(5740, 5741);
        while (!bot.shouldExit && !Core.checkInventory("Legion Token", quant)) {
            Core.huntMonster("laken", "Augmented Guard", "Stolen Guard", 5);
            Core.huntMonster("laken", "Cyborg Dog", "Stolen Dog", 6);
            Core.huntMonster("laken", "Mad Scientist", "Taken Axe", 10);
        }
        Core.cancelRegisteredQuests();
    }

    ltMountedParagonPet(quant = 50000): void {
        if (
            Core.checkInventory("Legion Token", quant)
            || !Core.checkInventory("Mounted Paragon Pet")
        )
            return;

        this.joinLegion();

        Core.equipClass(ClassType.Farm);

        Core.farmingLogger("Legion Token", quant);
        Core.addDrop("Legion Token");
        Core.registerQuests(5604);
        while (!bot.shouldExit && !Core.checkInventory("Legion Token", quant)) {
            Core.huntMonster("frozentower", "Ice Wolf", "Giant Coal Lump", 10);
            Core.huntMonster("frozentower", "Ice Wolf", "Small Coal Lump", 8);
        }
        Core.cancelRegisteredQuests();
    }

    ltAscendedParagon(quant = 50000): void {
        if (
            Core.checkInventory("Legion Token", quant)
            || !Core.checkInventory("Ascended Paragon Pet")
        )
            return;

        this.joinLegion();

        Core.equipClass(ClassType.Farm);

        Core.farmingLogger("Legion Token", quant);
        Core.addDrop("Legion Token");
        Core.registerQuests(2747);
        while (!bot.shouldExit && !Core.checkInventory("Legion Token", quant)) {
            Core.huntMonster("tournament", "Lord Brentan", "Lord Brentan's Regal Blade");
            Core.huntMonster("tournament", "Roderick", "Roderick's Chaotic Bane");
            Core.huntMonster("tournament", "Knight of Thorns", "Knight of Thorns' Sword");
            Core.huntMonster("tournament", "Johann Wryce", "Platinum of Johann Wryce");
            Core.huntMonster("tournament", "Khai Khaldun", "Khai Khaldun's Scimitar");
        }
        Core.cancelRegisteredQuests();
    }

    joinLegion(): void {
        if (Core.isCompletedBefore(793))
            return;

        // C# asks with a Yes/No message box; Turu has none for scripts, so it keeps the armor.
        Core.logger("Not selling \"Undead Warrior\" after joining the Legion (C# asks with a message box; sell it yourself for 1080 AC).");
        const SellUW = false as boolean;

        if (!Core.isCompletedBefore(792))
            Farm.bludrutBrawlBoss(undefined, 200);

        if (!Core.checkInventory("Undead Warrior"))
            Core.buyItem("underworld", 215, "Undead Warrior");

        // Undead Champion Initiation
        if (!Story.questProgression(789)) {
            Core.equipClass(ClassType.Solo);
            Core.ensureAccept(789);
            Core.huntMonster("greenguardwest", "Black Knight", "Black Knight's Eternal Contract");
            Core.ensureComplete(789);
        }

        // Mourn the Soldiers
        if (!Story.questProgression(790)) {
            Core.equipClass(ClassType.Farm);
            Core.ensureAccept(790);
            Core.huntMonster("dwarfhold", "Chaos Drow", "Chaos Drow slain");
            Core.huntMonster("swordhavenundead", "Skeletal Soldier", "Skeletal Soldier slain");
            Core.huntMonster("pirates", "Fishman Soldier", "Fishman Soldier slain");
            Core.huntMonster("willowcreek", "Dwakel Soldier", "Dwakel Soldier slain");
            Core.ensureComplete(790);
        }

        // Understanding Undead Champions
        Core.equipClass(ClassType.Solo);
        if (!Story.questProgression(791)) {
            Core.ensureAccept(791);
            Core.huntMonster("battleunderb", "Undead Champion", "Ravaged Champion Soul", 80, { isTemp: false });
            Core.ensureComplete(791);
        }

        // Player vs Power
        if (!Story.questProgression(792)) {
            Story.chainQuest(792);
        }

        // Fail to the King
        Core.equipClass(ClassType.Farm);
        Story.killQuest(793, "prison", "King Alteon's Knight");

        Adv.buyItem("underworld", 216, "Undead Champion");

        if (SellUW === true && Core.isCompletedBefore(792) && Core.checkInventory("Undead Champion"))
            Core.sellItem("Undead Warrior", undefined, { all: true });

        Core.logger("You're now part of the legion! Hail... Dage?");
    }

    obsidianRock(quant = 666): void {
        if (Core.checkInventory("Obsidian Rock", quant))
            return;

        this.soulForgeHammer();

        Core.equipClass(ClassType.Farm);
        Core.addDrop("Obsidian Rock");
        Core.logger(!Core.isMember ? "Using Non-Member Method" : "Using Members Method");

        if (!Core.isMember)
            bot.quests.updateQuest(1542);

        Core.registerQuests(2742);
        Core.killMonster(Core.isMember ? "hydra" : "firestorm", Core.isMember ? "Rune2" : "r8", Core.isMember ? "Right" : "Left", Core.isMember ? "Fire Imp" : "Firestorm Hatchling", "Obsidian Rock", quant, { isTemp: false, log: false });

        bot.wait.forPickup("Obsidian Rock");
        Core.cancelRegisteredQuests();
    }

    dagePvP(trophyQuant = 4000, techniqueQuant = 1000, scrollQuant = 1000, canSoloBoss = true, enableDebug = false): void {
        // C# local functions
        const logFarmingProgress = (): void => {
            if (trophyQuant > 0)
                Core.farmingLogger("Legion Combat Trophy", trophyQuant);
            if (techniqueQuant > 0)
                Core.farmingLogger("Technique Observed", techniqueQuant);
            if (scrollQuant > 0)
                Core.farmingLogger("Sword Scroll Fragment", scrollQuant);
        };

        const checkInventoryCompletion = (): boolean => {
            return Core.checkInventory("Legion Combat Trophy", trophyQuant)
                && Core.checkInventory("Technique Observed", techniqueQuant)
                && Core.checkInventory("Sword Scroll Fragment", scrollQuant);
        };

        // C# `Exit(cell, ref exitAttempt)`: works on a copy of the counter, and always runs the
        // death part after the exit part (no return between the two labels).
        const exit = (_cell: string | null, attempt: number): void => {
            let exitAttempt = attempt;
            let death = 0;
            if (bot.player.alive) {
                while (!bot.shouldExit && bot.map.name !== "battleon") {
                    Core.logger(`Attempting Exit ${exitAttempt++}.`);
                    bot.combat.cancelTarget();
                    bot.wait.forCombatExit();
                    bot.map.join("battleon-999999");
                    Core.sleep(1500);
                    if (bot.map.name !== "battleon")
                        Core.logger("Failed!? HOW.. try agian");
                    else {
                        Core.logger("Successful!");
                        exitAttempt = 0;
                        break;
                    }
                }
            }

            Core.logger(`Death: ${death++}, resetting`);
            while (!bot.shouldExit) {
                bot.wait.forTrue(() => bot.player.alive, 100);
                Core.logger(`Attempting Death Exit ${exitAttempt++}.`);
                bot.map.join("battleon-999999");
                bot.wait.forMapLoad("battleon");
                Core.sleep(1500);
                if (bot.map.name !== "battleon")
                    Core.logger("Failed!? HOW.. try agian");
                else {
                    Core.logger("Successful!");
                    exitAttempt = 0;
                    death = 0;
                    break;
                }
            }
        };

        const farmScrollArea = (): void => {
            if (scrollQuant === 0)
                return;

            Core.pvpMove(11, "r7", 513, 286);
            Core.pvpMove(15, "r10", 832, 347);

            Core.pvpKilling();

            if (!bot.player.alive) {
                Core.debugLogger("You're dead stupid");
                exit("Enter0", exitAttempt);
            }

            Core.pvpMove(20, "r11", 943, 391);

            Core.pvpKilling();

            if (!bot.player.alive || bot.player.cell === "Enter0") {
                Core.debugLogger("You're dead stupid");
                exit("Enter0", exitAttempt);
            }

            Core.pvpMove(21, "r10", 9, 397);

            if (!bot.player.alive || bot.player.cell === "Enter0") {
                Core.debugLogger("You're dead stupid");
                exit("Enter0", exitAttempt);
            }

            Core.pvpMove(19, "r7", 7, 392);
            Core.pvpMove(14, "r6", 482, 483);
        };

        if (checkInventoryCompletion())
            return;

        // C# assigns CBOBool's result, which is whether the key is set, not its value.
        canSoloBoss = Core.cboBool("PvP_SoloPvPBoss") !== null;

        Core.addDrop("Legion Combat Trophy", "Technique Observed", "Sword Scroll Fragment");
        Core.equipClass(ClassType.Solo);

        let exitAttempt = 0;

        if (enableDebug)
            Core.dlEnable();

        // C# `Start:` ... `goto Start` restarts this loop.
        while (!bot.shouldExit && !checkInventoryCompletion()) {
            exitAttempt = 0;
            logFarmingProgress();

            Core.join("dagepvp-999999", "Enter0", "Spawn");

            Core.pvpMove(1, "r2", 475, 269);
            Core.pvpMove(4, "r4", 963, 351);
            Core.pvpMove(7, "r5", 843, 189);
            Core.pvpMove(9, "r6", 937, 389);

            //if scroll quant > 0, farm scroll, then return to r6 to cotniue
            if (scrollQuant > 0)
                farmScrollArea();

            if (
                (trophyQuant === 0 || Core.checkInventory("Legion Combat Trophy", trophyQuant))
                && (techniqueQuant === 0 || Core.checkInventory("Technique Observed", techniqueQuant))
            ) {
                exit("Enter0", exitAttempt);
                continue;
            }

            Core.pvpMove(12, "r12", 758, 338);

            if (!canSoloBoss)
                Core.pvpKilling();

            if (!bot.player.alive || bot.player.cell === "Enter0") {
                Core.debugLogger("You're dead stupid");
                exit("Enter0", exitAttempt);
                continue;
            }
            Core.pvpMove(23, "r13", 933, 394);
            if (!canSoloBoss)
                Core.pvpKilling();

            if (!bot.player.alive || bot.player.cell === "Enter0") {
                Core.debugLogger("You're dead stupid");
                exit("Enter0", exitAttempt);
                continue;
            }
            Core.pvpMove(25, "r14", 846, 181);

            if (!canSoloBoss)
                Core.pvpKilling();

            if (!bot.player.alive || bot.player.cell === "Enter0") {
                Core.debugLogger("You're dead stupid");
                exit("Enter0", exitAttempt);
                continue;
            }
            Core.pvpMove(28, "r15", 941, 348);

            const startTrophy = bot.inventory.getQuantity("Legion Combat Trophy");
            const startTechnique = bot.inventory.getQuantity("Technique Observed");
            const startScroll = bot.inventory.getQuantity("Sword Scroll Fragment");

            // Boss kill... GL without the debuff
            bot.kill.monster(27);

            if (!bot.player.alive || bot.player.cell === "Enter0") {
                Core.debugLogger("You're dead stupid");
                exit("Enter0", exitAttempt);
                continue;
            }

            bot.wait.forDrop("Legion Combat Trophy", 40);
            bot.wait.forPickup("Legion Combat Trophy");

            // Wait for server PvP reward packet to update inventory
            let trophyNow = bot.inventory.getQuantity("Legion Combat Trophy");
            let techniqueNow = bot.inventory.getQuantity("Technique Observed");
            let scrollNow = bot.inventory.getQuantity("Sword Scroll Fragment");

            while (!bot.shouldExit && trophyNow <= startTrophy && techniqueNow <= startTechnique && scrollNow <= startScroll) {
                Core.sleep(500);

                trophyNow = bot.inventory.getQuantity("Legion Combat Trophy");
                techniqueNow = bot.inventory.getQuantity("Technique Observed");
                scrollNow = bot.inventory.getQuantity("Sword Scroll Fragment");
            }

            logFarmingProgress();
            exit("Enter0", exitAttempt);
        }
        // Ensure we exit the map before going esle where
        if (bot.map.name === "dagepvp")
            Core.join("Battleon-99999");
    }

}
