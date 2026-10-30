/*
name: null
description: null
tags: null
*/
// Port of Scripts/Evil/SDKA/CoreSDKA.cs.

import { bot, choices, defineScriptOptions, option } from "turu";
import { CoreAdvanced } from "../../core/CoreAdvanced";
import { ClassType, Core, configEnum, configOr } from "../../core/CoreBots";
import { CoreDailies, HardCoreMetalsEnum } from "../../core/CoreDailies";
import { CoreFarms } from "../../core/CoreFarms";
import { CoreStory } from "../../core/CoreStory";

const Farm = new CoreFarms();
const Adv = new CoreAdvanced();
const Daily = new CoreDailies();
const Story = new CoreStory();

export class CoreSDKA {
    sdkaItems: string[] = [
        "Sepulchure's DoomKnight Armor",
        // Arsenics
        "Arsenic",
        "Accursed Arsenic",
        "Accursed Arsenic of Doom",
        // Daggers
        "Daggers of Destruction",
        "Shadow Daggers of Destruction",
        "Necrotic Daggers of Destruction",
        // Chromiums
        "Chromium",
        "Calamitous Chromium",
        "Calamitous Chromium of Doom",
        // Broadswords
        "Broadsword of Bane",
        "Shadow Broadsword of Bane",
        "Necrotic Broadsword of Bane",
        // Rhodiums
        "Rhodium",
        "Reprehensible Rhodium",
        "Reprehensible Rhodium of Doom",
        // Bows
        "Bow to the Shadows",
        "ShadowBow of the Shadows",
        "Necrotic Bow of the Shadow",
        // Merge misc.
        "Experimental Dark Item",
        "Dark Energy",
        "Dark Spirit Orb",
        "Corrupt Spirit Orb",
        "Ominous Aura",
        "Diabolical Aura",
        "Doom Aura",
        // Weapon kits
        "DoomSquire Weapon Kit",
        "DoomSoldier Weapon Kit",
        "DoomKnight Weapon Kit",
        // Misc.
        "DoomKnight Hood",
        "Elders' Blood",
        "Undead Energy",
        "Iron Hammer",
        "War Mummy Wrap",
        "Stone Hammer",
        "Grumpy Warhammer",
        "Shadow Terror Axe",
        "DoomCoin",
        "Dark Skull",
        "Shadow Creeper Enchant",
        "Shadow Serpent Scythe",
    ];

    selectedQuest: SDKAQuest | null = null;

    questProgression = false;

    doAll(): void {

        if (Core.checkInventory("Sepulchure's DoomKnight Armor") || !Core.isMember) {
            Core.logger(
                Core.checkInventory("Sepulchure's DoomKnight Armor")
                    ? "Player already owns SDKA"
                    : "Player is non-Member, membership is required for SDKA");
            return;
        }

        this.selectedQuest = this.getSelectedQuest();

        Core.addDrop(...this.sdkaItems);
        Core.logger("Step 1/5: Unlock Hard Core Metals");
        this.unlockHardCoreMetals();
        if (!Core.checkInventory(
                ["Arsenic", "Accursed Arsenic", "Accursed Arsenic of Doom", "Daggers of Destruction", "Shadow Daggers of Destruction", "Necrotic Daggers of Destruction"], undefined, { any: true, toInv: false }))
            Daily.hardCoreMetals(["Arsenic"], undefined, { toBank: true });
        else if (!Core.checkInventory(
                ["Chromium", "Calamitous Chromium", "Calamitous Chromium of Doom", "Broadsword of Bane", "Shadow Broadsword of Bane", "Necrotic Broadsword of Bane"], undefined, { any: true, toInv: false }))
            Daily.hardCoreMetals(["Chromium"], undefined, { toBank: true });
        else if (!Core.checkInventory(
                ["Rhodium", "Reprehensible Rhodium", "Reprehensible Rhodium of Doom", "Bow to the Shadows", "ShadowBow of the Shadows", "Necrotic Bow of the Shadow"], undefined, { any: true, toInv: false }))
            Daily.hardCoreMetals(["Rhodium"], undefined, { toBank: true });
        Core.logger("Step 2/5: Getting Necrotic Daggers");
        this.necroticDaggers();
        Core.logger("Step 3/5: Getting Necrotic Broadsword");
        this.necroticBroadsword();
        Core.logger("Step 4/5: Getting Necrotic Bow");
        this.necroticBow();
        Core.logger("Step 5/5: Doing Quest: Summoning Sepulchure Armor, for SDKA");
        this.summoningSepulchureArmor();
    }

    unlockHardCoreMetals(): void {
        //                    // Hard Core Metals | 2098
        if (!Core.isMember || Core.isCompletedBefore(2098)) {
            Core.logger(!Core.isMember
                    ? "Not a member, skipping."
                    : "Hard Core Metals already unlocked, skipping.");
            return;
        }

        Core.addDrop(
            "Dark Energy", 
            "Dark Spirit Orb", 
            "DoomKnight Hood", 
            "Experimental Dark Item", 
            "Shadow Terror Axe", 
            "Elders' Blood", 
            "DoomCoin", 
            "Shadow Creeper Enchant", 
            "Shadow Serpent Scythe", 
            "Dark Skull", 
            "Corrupt Spirit Orb");

        //#region DoQuests
        // Sepulchure's Armor | 2069
        if (!Story.questProgression(2069)) {
            Core.ensureAccept(2069);
            this.dso(40);
            Core.buyItem("shadowfall", 100, "DoomKnight Hood");
            Core.addDrop("Experimental Dark Item");
            Core.ensureComplete(2069);
            bot.wait.forPickup("Experimental Dark Item");
            Core.toBank("Experimental Dark Item");
        }

        // The Doom that Looms | 2086 - 2087
        if (!Story.questProgression(Core.checkInventory(8523) ? 2086 : 2087)) {
            Core.ensureAccept(Core.checkInventory(8523) ? 2086 : 2087);

            // Check if DoomKnight Class is missing for non-members or members without either version (AC or non-AC)
            if (
                !Core.isMember && !Core.checkInventory(2083)
                || Core.isMember && !Core.checkInventory([ 8523, 2083 ], undefined, { any: true })
            ) {
                Core.logger(
                    "You don't have the DoomKnight Class, Getting it for you. (+Warrior/Healer if those aren't R10)");

                // Ensure Healer is Rank 10
                Core.buyItem("trainers", 176, "Healer");
                Adv.rankUpClass("Healer");

                // Ensure Warrior is Rank 10
                Core.buyItem("trainers", 170, "Warrior");
                Adv.rankUpClass("Warrior");

                // Buy and rank up DoomKnight if not owned (non-AC version for F2P)
                Adv.buyItem("shadowfall", 100, 2083, undefined, 6309); // Buy non-AC DoomKnight for F2P
                bot.wait.forPickup(2083); // Wait for the item to be picked up
                Adv.rankUpClass("DoomKnight", { itemId: 2083 }); // Rank it up
            }

            // Ensure DoomKnight is Rank 10 for both members and non-members before proceeding with quest completion
            Adv.rankUpClass(
                "DoomKnight", { itemId: Core.isMember ? (Core.checkInventory(8523) ? 8523 : 2083) : 2083 });

            // Equip the DoomKnight class
            Core.equipClass(ClassType.Solo);

            // Complete the quest for obtaining the class
            Core.ensureComplete(Core.checkInventory(8523) ? 2086 : 2087);

            // Bank non-solo classes if equipped
            if (Core.soloClass !== "DoomKnight")
                Core.toBank(Core.isMember ? 8523 : 2083);
        }

        // Toiling with Terror | 2088
        if (!Story.questProgression(2088)) {
            Core.ensureAccept(2088);
            Daily.eldersBlood();

            if (!Core.checkInventory("Elders' Blood"))
                Core.logger(
                    `Not enough \"Elders' Blood\", please do the daily upon daily reset.`, undefined, { messageBox: true, stopBot: true });

            Core.huntMonster("battleundera", "Bone Terror", "Shadow Terror Axe", undefined, { isTemp: false });
            Core.ensureComplete(2088);
            Core.toBank("Elders' Blood");
        }

        // A Penny for your Foughts | 2089
        if (!Story.questProgression(2089)) {
            this.penny(undefined, true);
        }

        // Dark Spirit Donation | 2090
        if (!Story.questProgression(2090)) {
            Core.ensureAccept(2090);
            this.dso(100);
            Core.huntMonster("necrocavern", "Shadow Imp", "Dark Skull", undefined, { isTemp: false });
            Core.ensureComplete(2090);
        }
    }

    farmDSO(quant = 10500): void {
        if (Core.checkInventory("Dark Spirit Orb", quant))
            return;

        this.selectedQuest = this.getSelectedQuest();
        Core.logger(`Selected farming Quest: ${this.selectedQuest}`);

        if (this.selectedQuest === SDKAQuest.DarkSpiritOrbs)
            this.dso(quant);
        else
            this.penny(quant);
    }

    private getSelectedQuest(): SDKAQuest {
        const selectedQuest = configEnum("SelectedQuest", SDKAQuest, 0 as SDKAQuest);
        return (selectedQuest as number) === 0 // the option isn't declared (C# default)
            ? Core.isMember ? SDKAQuest.APennyforYourFoughts : SDKAQuest.DarkSpiritOrbs
            : selectedQuest;
    }

    penny(quant = 10500, oneTime = false): void {
        if (Core.checkInventory("Dark Spirit Orb", quant) && !oneTime)
            return;

        Core.logger(
            oneTime
                ? `oneTime set to: ${oneTime}`
                : `Farming \"Dark Spirit Orb\" ${Core.dynamicQuant("Dark Spirit Orb", false)} / ${quant}`);

        // Prevent recursion when called from the unlock quest
        if (!oneTime)
            this.unlockHardCoreMetals();

        Core.addDrop("DoomCoin", "Dark Spirit Orb", "Shadow Creeper Enchant");
        if (Core.checkInventory("DoomCoin")) {
            Core.logger("Selling DoomCoin to ensure we have a fresh start to the quest ( its buggy appearntly)");
            Core.sellItem("DoomCoin", undefined, { all: true });
        }
        Core.equipClass(ClassType.Farm);
        if (oneTime) {
            Core.ensureAccept(2089);
            Core.killMonster("maul", "r7", "left", "Shelleton", "DoomCoin", 20, { isTemp: false, log: false });
            Core.ensureComplete(2089);
            bot.wait.forPickup("Dark Spirit Orb");
            return;
        } else {
            while (!bot.shouldExit && !Core.checkInventory("Dark Spirit Orb", quant)) {
                Core.ensureAccept(2089);
                Core.killMonster("maul", "r7", "left", "Shelleton", "DoomCoin", 20, { isTemp: false, log: false });
                Core.ensureComplete(2089);
                bot.wait.forPickup("Dark Spirit Orb");
            }
            Core.cancelRegisteredQuests();
            Core.abandonQuest(2089);
            return;
        }
    }

    dso(quant = 10500): void {
        if (Core.checkInventory("Dark Spirit Orb", quant))
            return;

        Core.addDrop(
            "Dark Spirit Orb", 
            "Shadow Creeper Enchant", 
            "Shadow Serpent Scythe", 
            "Dark Energy");
        Core.equipClass(ClassType.Farm);
        Core.farmingLogger("Dark Spirit Orb", quant);
        Core.registerQuests(2065);
        while (!bot.shouldExit && (!Core.checkInventory("Dark Spirit Orb", quant))) {
            Core.huntMonster("bludrut2", "Shadow Creeper", "Shadow Creeper Enchant", undefined, { isTemp: false });
            Core.huntMonster("bludrut4", "Shadow Serpent", "Shadow Serpent Scythe", undefined, { isTemp: false });
            Core.huntMonster("ruins", "Dark Witch", "Shadow Whiskers", 6);

            if (Core.checkInventory("Dark Energy", 5000))
                this.doomMerge("Dark Spirit Orb", 100);
        }
        Core.cancelRegisteredQuests();
    }

    doomMerge(item: string, quant = 1): void {
        Core.buyItem("necropolis", 423, item, quant);
    }

    doomSquireWK(quant = 1): void {
        if (Core.checkInventory("DoomSquire Weapon Kit", quant))
            return;

        Core.farmingLogger("DoomSquire Weapon Kit", quant);

        // Squire's quest (2144) is gated by a hidden pre-quest, so we check
        // whether the Soldier quest (2164) is unlocked as the readiness signal.
        if (!bot.quests.isUnlocked(2164)) {
            Core.logger("DoomSquire Weapon Kit Quest locked, unlocking via metal upgrade...");
            const Metals: string[] =
            [
            "Arsenic", "Beryllium", "Chromium", "Palladium",
            "Rhodium", "Thorium", "Mercury",
        ];

            const allItems = bot.inventory.items.concat(bot.bank.items)
                .filter(x => x != null && Metals.some(m => m.toLowerCase() === x.name.toLowerCase()));

            const metalEnum = allItems
                .map(x => Object.values(HardCoreMetalsEnum).find(v => typeof v === "number" && HardCoreMetalsEnum[v].toLowerCase() === x.name.toLowerCase()) as HardCoreMetalsEnum | undefined)
                .find(x => x !== undefined)
                ?? HardCoreMetalsEnum.Arsenic;

            this.upgradeMetal(metalEnum);
        }

        Core.equipClass(ClassType.Farm);
        Core.addDrop("DoomSquire Weapon Kit");
        Core.registerQuests(2144);

        while (!bot.shouldExit && !Core.checkInventory("DoomSquire Weapon Kit", quant)) {
            if (!Core.checkInventory("Iron Hammer")) {
                if (Core.checkInventory(319))
                    Core.buyItem("swordhaven", 179, "Iron Hammer");
                else
                    Core.huntMonster("battleundera", "Skeletal Warrior", "Iron Hammer", undefined, { isTemp: false });
            }
            Core.huntMonster("sandcastle", "War Mummy", "War Mummy Wrap", undefined, { isTemp: false, log: false });
            Core.huntMonster("noobshire", "Horc Noob", "Noob Blade Oil", undefined, { log: false });
            Core.huntMonster("farm", "Scarecrow", "Burlap Cloth", 4, { log: false });
            Core.huntMonster("lair", "Bronze Draconian", "Bronze Brush", undefined, { log: false });
            Core.huntMonster("bludrut", "Rock Elemental", "Elemental Stone Sharpener", undefined, { log: false });
            Core.huntMonster("nulgath", "Dark Makai", "Dark Makai Lacquer Finish", undefined, { log: false });

            bot.wait.forPickup("DoomSquire Weapon Kit");
            if (this.questProgression)
                break;

        }

        Core.cancelRegisteredQuests();
    }

    doomSoldierWK(quant = 1): void {
        if (Core.checkInventory("DoomSoldier Weapon Kit", quant))
            return;

        Core.farmingLogger("DoomSoldier Weapon Kit", quant);

        if (!bot.quests.isUnlocked(2164)) {
            Core.logger("DoomSoldier Weapon Kit Quest Locked, Doing Previous Quest");
            this.doomSquireWK(1);
            if (this.questProgression)
                return;
        }
        Core.addDrop("DoomSoldier Weapon Kit");
        Core.registerQuests(2164);

        while (!bot.shouldExit && !Core.checkInventory("DoomSoldier Weapon Kit", quant)) {
            Core.equipClass(ClassType.Solo);
            Core.huntMonster("cornelis", "Stone Golem", "Stone Hammer", undefined, { isTemp: false });
            Core.huntMonster("hachiko", "Dai Tengu", "Superior Blade Oil");
            Core.huntMonster("vordredboss", "Shadow Vordred", "Shadow Lacquer Finish");
            Core.huntMonster("anders", "Copper Sky Pirate", "Copper Awl");
            Core.huntMonster("necrocavern", "Shadow Imp", "Shadowstone Sharpener");

            Core.equipClass(ClassType.Farm);
            Core.killMonster("lycan", "r4", "Left", "Chaos Vampire Knight", "Silver Brush", undefined, { log: false });
            Core.killMonster("sandport", "r3", "Right", "Tomb Robber", "Leather Case", undefined, { log: false });
            Core.killMonster("pines", "Path1", "Left", "LeatherWing", "LeatherWing Hide", 10, { log: false });

            bot.wait.forPickup("DoomSoldier Weapon Kit");
        }

        Core.cancelRegisteredQuests();
    }

    doomKnightWK(item = "DoomKnight Weapon Kit", quant = 1): void {
        if (Core.checkInventory(item, quant))
            return;

        Core.farmingLogger(item, quant);

        if (!bot.quests.isUnlocked(2165)) {
            Core.logger("DoomKnight Weapon Kit Quest Locked, Doing Previous Quest");
            this.doomSoldierWK(1);
            this.questProgression = false;
        }

        Core.addDrop("DoomKnight Weapon Kit", "Dark Spirit Orb", "Corrupt Spirit Orb", "Ominous Aura", "Grumpy Warhammer");
        Core.equipClass(ClassType.Solo);
        bot.quests.updateQuest(999);
        Core.registerQuests(2165);

        while (!bot.shouldExit && !Core.checkInventory(item, quant)) {
            Core.killMonster("boxes", "Boss", "Left", "Sneeviltron", "Grumpy Warhammer", undefined, { isTemp: false });
            Core.killKitsune("No. 1337 Blade Oil");
            Core.killMonster("sandcastle", "r7", "Left", "Chaos Sphinx", "Gold Brush");
            Core.killMonster("crashsite", "Boss", "Left", "ProtoSartorium", "Non-abrasive Power Powder");
            Core.killMonster("necrocavern", "r13", "Left", "Shadow Dragon", "ShadowDragon Hide", 3);
            Core.killMonster("dragonplane", "r9", "Left", "Moganth", "Moganth's Stone Sharpener");
            Core.killMonster("akiba", "cave4boss", "Left", "Shadow Nukemichi", "Doom Lacquer Finish");
            Core.killMonster("dreamnexus", "r7", "Left", "Dark Wyvern", "Dark Wyvern Hide Travel Case");

            bot.wait.forPickup(item);
        }

        Core.cancelRegisteredQuests();
    }

    necroticDaggers(): void {
        if (Core.checkInventory("Necrotic Daggers of Destruction")) {
            Core.logger("Daggers found, skipping");
            return;
        }

        if (
            !Core.checkInventory(
                [
                    "Necrotic Daggers of Destruction",
                    "Shadow Daggers of Destruction",
                    "Daggers of Destruction",
                ], undefined, { any: true })
        ) {
            if (
                !Core.checkInventory(
                    [ "Accursed Arsenic of Doom", "Accursed Arsenic" ], undefined, { any: true }) && !Core.checkInventory("Ominous Aura", 2)
            ) {
                const DSOQuantity =
                    10500
                    - (bot.inventory.getQuantity("Corrupt Spirit Orb") * 100)
                    - (bot.inventory.getQuantity("Ominous Aura") * 5000);
                this.farmDSO(DSOQuantity);
            }

            if (
                !Core.checkInventory(
                    [ "Accursed Arsenic of Doom", "Accursed Arsenic" ], undefined, { any: true }) && !Core.checkInventory("Ominous Aura", 2)
            ) {
                const CSOQuantity = 105 - (bot.inventory.getQuantity("Ominous Aura") * 50);
                if ((CSOQuantity * 100) > bot.inventory.getQuantity("Dark Spirit Orb"))
                    this.farmDSO(CSOQuantity * 100);
                this.doomMerge("Corrupt Spirit Orb", CSOQuantity);
            }

            if (
                !Core.checkInventory(
                    [ "Accursed Arsenic of Doom", "Accursed Arsenic" ], undefined, { any: true }) && !Core.checkInventory("Ominous Aura", 2)
            )
                this.doomMerge("Ominous Aura", 2);

            if (!Core.checkInventory(12476)) {
                if (!Core.checkInventory("Accursed Arsenic")) {
                    Core.farmingLogger("Accursed Arsenic");
                    Core.ensureAccept(2110);
                    Core.huntMonster("bludrut4", "Shadow Serpent", "Dark Energy", 26, { isTemp: false });
                    Daily.hardCoreMetals([ "Arsenic" ]);
                    if (!Core.checkInventory("Arsenic")) {
                        Core.logger(
                            "Can't complete Accursed Arsenic Hex (Missing Arsenic).\n"
                                + "This requires a daily, please run the bot again after the daily reset has occurred.");
                        return;
                    }
                    this.farmDSO(6);
                    Core.huntMonster("arcangrove", "Seed Spitter", "Deadly Knightshade", 16);
                    Core.ensureComplete(2110);
                }
                const CSOQuantity = 105 - (bot.inventory.getQuantity("Ominous Aura") * 50);
                if ((CSOQuantity * 100) > bot.inventory.getQuantity("Dark Spirit Orb"))
                    this.farmDSO(CSOQuantity * 100);
                this.doomMerge("Corrupt Spirit Orb", CSOQuantity);
                this.doomMerge("Ominous Aura", 2);
                Core.buyItem("dwarfhold", 434, 12476, undefined, 778);
            }

            Core.farmingLogger("Daggers of Destruction");
            this.doomSquireWK();
            this.farmDSO(50);
            this.doomMerge("Daggers of Destruction");
        }

        if (Core.checkInventory("Daggers of Destruction")) {
            Core.farmingLogger("Shadow Daggers of Destruction");
            this.doomSoldierWK();
            this.doomKnightWK("Ominous Aura");
            this.doomMerge("Shadow Daggers of Destruction");
        }

        if (Core.checkInventory("Shadow Daggers of Destruction")) {
            Core.farmingLogger("Necrotic Daggers of Destruction");
            this.doomKnightWK();
            this.doomMerge("Necrotic Daggers of Destruction");
        }
    }

    necroticBroadsword(): void {
        if (Core.checkInventory("Necrotic Broadsword of Bane")) {
            Core.logger("Broadsword found, skipping.");
            return;
        }
        if (!Core.checkInventory("Necrotic Daggers of Destruction"))
            this.necroticDaggers();

        if (
            !Core.checkInventory(
                [
                    "Necrotic Broadsword of Bane",
                    "Shadow Broadsword of Bane",
                    "Broadsword of Bane",
                ], undefined, { any: true })
        ) {
            if (!Core.checkInventory("Calamitous Chromium of Doom")) {
                if (!Core.checkInventory("Calamitous Chromium")) {
                    Core.farmingLogger("Calamitous Chromium", 1);
                    Core.addDrop("Calamitous Chromium");
                    Core.ensureAccept(2112);
                    Core.huntMonster("bludrut4", "Shadow Serpent", "Dark Energy", 26, { isTemp: false });
                    Daily.hardCoreMetals([ "Chromium" ]);
                    if (!Core.checkInventory("Chromium")) {
                        Core.logger(
                        "Can't complete Calamitous Chromium Hex (Missing Chromium).\n"
                            + "This requires a daily, please run the bot again after the daily reset has occurred.");
                        return;
                    }
                    this.farmDSO(6);
                    Core.huntMonster("arcangrove", "Seed Spitter", "Deadly Knightshade", 16);
                    Core.ensureComplete(2112);
                    bot.wait.forPickup("Calamitous Chromium");
                }
                Core.farmingLogger("Calamitous Chromium of Doom");
                this.omninousAura();
                this.doomKnightWK("Corrupt Spirit Orb", 5);
                Core.buyItem("dwarfhold", 434, "Calamitous Chromium of Doom");
            }
            if (!Core.checkInventory("Diabolical Aura")) {
                this.omninousAura(25);
                this.doomMerge("Diabolical Aura");
            }
            Core.farmingLogger("Broadsword of Bane");
            this.doomKnightWK("Corrupt Spirit Orb");
            this.doomKnightWK("Dark Spirit Orb", 20);
            this.doomSquireWK();
            this.doomMerge("Broadsword of Bane");
        }

        if (Core.checkInventory("Broadsword of Bane")) {
            Core.farmingLogger("Shadow Broadsword of Bane");
            this.doomKnightWK("Corrupt Spirit Orb");
            this.omninousAura(1);
            this.doomSoldierWK();
            this.doomMerge("Shadow Broadsword of Bane");
        }

        if (Core.checkInventory("Shadow Broadsword of Bane")) {
            Core.farmingLogger("Necrotic Broadsword of Bane");
            this.doomKnightWK();
            this.doomMerge("Necrotic Broadsword of Bane");
        }
    }

    necroticBow(): void {
        if (Core.checkInventory("Necrotic Bow of the Shadow")) {
            Core.logger("Bow found, skipping.");
            return;
        }
        if (!Core.checkInventory("Necrotic Broadsword of Bane"))
            this.necroticBroadsword();

        if (
            !Core.checkInventory(
                [
                    "Necrotic Bow of the Shadow",
                    "ShadowBow of the Shadows",
                    "Bow to the Shadows",
                ], undefined, { any: true })
        ) {
            if (!Core.checkInventory("Reprehensible Rhodium of Doom")) {
                if (!Core.checkInventory("Reprehensible Rhodium")) {
                    Core.farmingLogger("Reprehensible Rhodium");
                    Core.addDrop("Reprehensible Rhodium");
                    Core.ensureAccept(2114);
                    Core.huntMonster("bludrut4", "Shadow Serpent", "Dark Energy", 26, { isTemp: false });
                    Daily.hardCoreMetals([ "Rhodium" ]);
                    if (!Core.checkInventory("Rhodium")) {
                        Core.logger(
                        "Can't complete Reprehensible Rhodium Hex (Missing Rhodium).\n"
                            + "This requires a daily, please run the bot again after the daily reset has occurred.");
                        return;
                    }
                    this.farmDSO(6);
                    Core.huntMonster("arcangrove", "Seed Spitter", "Deadly Knightshade", 16);
                    Core.ensureComplete(2114);
                    bot.wait.forPickup("Reprehensible Rhodium");
                }
                Core.farmingLogger("Reprehensible Rhodium of Doom");
                this.omninousAura();
                this.doomKnightWK("Corrupt Spirit Orb", 5);
                Core.buyItem("dwarfhold", 434, "Reprehensible Rhodium of Doom");
            }
            Core.farmingLogger("Bow to the Shadows");
            this.doomSquireWK();
            this.doomKnightWK("Corrupt Spirit Orb");
            this.doomKnightWK("Dark Spirit Orb", 13);
            this.pinpointBroadsword();
            Farm.battleUnderB("Undead Energy", 17);
            this.doomMerge("Bow to the Shadows");
        }

        if (Core.checkInventory("Bow to the Shadows")) {
            Core.farmingLogger("ShadowBow of the Shadows");
            this.doomSoldierWK();
            this.doomKnightWK("Corrupt Spirit Orb");
            Core.huntMonster("bludrut4", "Shadow Serpent", "Dark Energy", 50, { isTemp: false });
            this.doomMerge("ShadowBow of the Shadows");
        }

        if (Core.checkInventory("ShadowBow of the Shadows")) {
            Core.farmingLogger("Necrotic Bow of the Shadow");
            this.doomKnightWK();
            this.doomMerge("Necrotic Bow of the Shadow");
        }
    }

    summoningSepulchureArmor(): void {
        if (Core.checkInventory("Sepulchure's DoomKnight Armor"))
            return;

        this.pinpointBow(500, 250); // DSO, CSO [Bow]
        this.omninousAura(125); // Ominous Aura [Daggers/Mace/Scythe]
        this.pinpointBroadsword(75); // Diabolocal Aura [Broadsword]

        Core.logger(Core.checkInventory("Doom Aura") ? "Doom Aura found." : "Farming for Doom Aura");

        this.pinpointthePieces(2181, [ "Doom Aura" ], [ 1 ]);

        if (!Core.checkInventory("Experimental Dark Item")) {
            Core.addDrop("Experimental Dark Item");
            this.pinpointBow(50, 0);
            Core.buyItem("shadowfall", 100, "DoomKnight Hood");
            Core.chainComplete(2069);
            bot.wait.forPickup("Experimental Dark Item");
        }

        this.doomKnightWK();
        Core.addDrop("Sepulchure's DoomKnight Armor");
        Core.ensureAccept(2187);
        Core.huntMonster("ruins", "Dark Elemental", "Heart of Darkness");
        Core.ensureComplete(2187);
        bot.wait.forDrop("Sepulchure's DoomKnight Armor");
        bot.wait.forPickup("Sepulchure's DoomKnight Armor");
    }

    omninousAura(quant = 5): void {
        if (Core.checkInventory("Ominous Aura", quant))
            return;

        // Substitute "Necrotic Daggers of Destruction"
        // with "Necrotic Mace of Misery" or "Necrotic Scythe of Scourge" if the player has it

        // List of weapons in in order of best rates [Best to worst]
        const Weapons: string[] =
        [
            "Necrotic Daggers of Destruction",
            "Necrotic Mace of Misery",
            "Necrotic Scythe of Scourge",
        ];
        const Weapon =
            Weapons.find(w => Core.checkInventory(w))
            ?? "Necrotic Daggers of Destruction";
        switch (Weapon) {
            case "Necrotic Daggers of Destruction":
                if (!Core.checkInventory("Necrotic Daggers of Destruction"))
                    this.necroticDaggers();

                this.pinpointthePieces(2181, [ "Ominous Aura" ], [ quant ]);
                break;

            case "Necrotic Mace of Misery":
                if (Core.checkInventory("Necrotic Mace of Misery"))
                    this.pinpointthePieces(2185, [ "Ominous Aura" ], [ quant ]);
                break;

            case "Necrotic Scythe of Scourge":
                if (Core.checkInventory("Necrotic Scythe of Scourge"))
                    this.pinpointthePieces(2184, [ "Ominous Aura" ], [ quant ]);
                break;

            default:
                break;
        }
    }

    pinpointBroadsword(quant = 1): void {
        if (Core.checkInventory("Diabolical Aura", quant))
            return;

        if (!Core.checkInventory("Necrotic Broadsword of Bane", 1, { toInv: false })) {
            this.necroticBroadsword();
            Core.logger(" \"Necrotic Broadsword of Bane\" still not owned, most likely missing Daily metal stuffs, stopping", undefined, { stopBot: true });
        }

        this.pinpointthePieces(2183, [ "Diabolical Aura" ], [ quant ]);
    }

    pinpointBow(quantDSO: number, quantCSO: number): void {
        if (
            Core.checkInventory("Dark Spirit Orb", quantDSO)
            && Core.checkInventory("Corrupt Spirit Orb", quantCSO)
        )
            return;

        if (!Core.checkInventory("Necrotic Bow of the Shadow", 1, { toInv: false })) {
            this.necroticBow();
            // Recheck
            if (!Core.checkInventory("Necrotic Bow of the Shadow", 1, { toInv: false }))
                Core.logger(" \"Necrotic Bow of the Shadow\" still not owned, most likely missing Daily metal stuffs, stopping", undefined, { stopBot: true });

        }

        Core.equipClass(ClassType.Farm);
        Core.farmingLogger("Dark Spirit Orb", quantDSO);
        Core.farmingLogger("Corrupt Spirit Orb", quantCSO);

        // Process each item individually
        this.pinpointthePieces(
            2186, 
            [ "Dark Spirit Orb", "Corrupt Spirit Orb" ], 
            [ quantDSO, quantCSO ]);
    }

    pinpointthePieces(quest: number, items: string[] | null = null, quants: number[] | null = null): void {
        if (items == null || quants == null || items.length !== quants.length)
            return;

        Core.addDrop("Dark Energy", "Dark Spirit Orb", "Corrupt Spirit Orb", "Ominous Aura", "Diabolical Aura", "Doom Aura");
        Core.equipClass(ClassType.Farm);

        for (let i = 0; i < items.length; i++) {
            const item = items[i];
            const quant = quants[i];

            Core.farmingLogger(item, quant);
            while (!bot.shouldExit && !Core.checkInventory(item, quant)) {
                Core.ensureAccept(quest);
                Core.killMonster("lycan", "r4", "Left", "*", "DoomKnight Armor Piece", 10, { log: false });
                Core.ensureComplete(quest);

                for (let t = 0; t < 10 && !bot.shouldExit && !bot.inventory.contains(item, quant); t++)
                    bot.sleep(500);
            }
        }

        Core.cancelRegisteredQuests();
        Core.jumpWait();
    }

    upgradeMetal(metal: HardCoreMetalsEnum): void {
        let fullMetalName = "";
        let upgradeMetalQuest = 0;
        let forgeKeyQuest = 0;
        let forgekeyitemID = 0;
        switch (metal) {
            case HardCoreMetalsEnum.Arsenic:
                fullMetalName = "Accursed Arsenic of Doom";
                upgradeMetalQuest = 2110;
                forgeKeyQuest = 2137;
                break;
            case HardCoreMetalsEnum.Beryllium:
                fullMetalName = "Baneful Beryllium of Doom";
                upgradeMetalQuest = 2111;
                forgeKeyQuest = 2138;
                break;
            case HardCoreMetalsEnum.Chromium:
                fullMetalName = "Calamitous Chromium of Doom";
                upgradeMetalQuest = 2112;
                forgeKeyQuest = 2139;
                break;
            case HardCoreMetalsEnum.Palladium:
                fullMetalName = "Pernicious Palladium of Doom";
                upgradeMetalQuest = 2113;
                forgeKeyQuest = 2140;
                break;
            case HardCoreMetalsEnum.Rhodium:
                fullMetalName = "Reprehensible Rhodium of Doom";
                upgradeMetalQuest = 2114;
                forgeKeyQuest = 2141;
                break;
            case HardCoreMetalsEnum.Thorium:
                fullMetalName = "Treacherous Thorium of Doom";
                upgradeMetalQuest = 2115;
                forgeKeyQuest = 2142;
                break;
            case HardCoreMetalsEnum.Mercury:
                fullMetalName = "Malefic Mercury of Doom";
                upgradeMetalQuest = 2116;
                forgeKeyQuest = 2143;
                break;
        }

        // If metal owned & DoomSoldier weapon kit is unlocked ( had to go beyond squire as its unlocked
        // by the first metal quest, not the returnin with the forge key)
        if (Core.checkInventory(fullMetalName) && bot.quests.hasBeenCompleted(forgeKeyQuest) && Story.questProgression(2144)) {
            bot.log(`Required metal (${fullMetalName}) owned, and ForgeKey quest(${forgeKeyQuest}) complete`);
            return;
        }

        // Initialize quest data for forge key quest
        const ForgeQuestdata = Core.initializeWithRetries(() => Core.ensureLoad(forgeKeyQuest));
        if (ForgeQuestdata == null) {
            Core.logger(`Failed to load quest data for quest ID ${forgeKeyQuest}. Cannot proceed with metal upgrade.`);
            return;
        }

        // Get the forge key itemid for the quest
        forgekeyitemID =
            ForgeQuestdata.requirements.find(x => x != null && x.name === "Forge Key")?.id
            ?? 0;

        if (forgekeyitemID === 0) {
            Core.logger(`Failed to find the item ID for 'Forge Key' in quest ID ${forgeKeyQuest}. Cannot proceed with metal upgrade.`);
            return;
        }

        Core.logger(`forge Key Quest: ${forgeKeyQuest} [Completed? ${bot.quests.hasBeenCompleted(forgeKeyQuest)}]\n Full Metal Name ${fullMetalName}\n Forge Key ItemID ${forgekeyitemID}`, "Information");

        if (!Core.checkInventory(fullMetalName)) {
            const upgradeMetalName = (fullMetalName.split(" ").slice(0, 2)).join(" ");
            Core.addDrop(fullMetalName);
            Core.farmingLogger(fullMetalName, 1);
            // Getting the partially upgraded metal
            if (!Core.checkInventory(upgradeMetalName)) {
                Core.addDrop(upgradeMetalName);
                Core.farmingLogger(upgradeMetalName, 1);
                Core.ensureAccept(upgradeMetalQuest);

                if (!Core.checkInventory(metal))
                    Daily.hardCoreMetals([HardCoreMetalsEnum[metal]]);
                if (!Core.checkInventory(metal)) {
                    Core.logger(
                     `Can't complete ${fullMetalName.split(" ").slice(0, 2).join(" ")} Enchantment (missing ${HardCoreMetalsEnum[metal]}).\n`
                         + "This requires a daily, please run the bot again after the daily reset has occurred.");
                    return;
                }

                this.farmDSO(6);
                Core.huntMonster("arcangrove", "Seed Spitter", "Deadly Knightshade", 16);
                Core.huntMonster("bludrut4", "Shadow Serpent", "Dark Energy", 26, { isTemp: false });
                Core.ensureComplete(upgradeMetalQuest);
                bot.wait.forPickup(upgradeMetalName);
                if (!Core.checkInventory(upgradeMetalName)) {
                    Core.logger(`Failed to obtain ${upgradeMetalName}. Cannot continue the metal upgrade.`, undefined, { stopBot: true });
                    return;
                }
            }

            // Getting the fully upgraded metal
            // IF the gooquest isnt unlocked do the very slow method first.           
            if (!bot.quests.isUnlocked(2165)) {
                let missingAuras = 0;
                if (!Core.checkInventory("Ominous Aura", 2))
                    missingAuras = 2 - bot.inventory.getQuantity("Ominous Aura");

                // Reserve five orbs for the metal and fifty for each missing aura.
                const requiredOrbs = 5 + (missingAuras * 50);
                if (!Core.checkInventory("Corrupt Spirit Orb", requiredOrbs)) {
                    const missingOrbs = requiredOrbs - bot.inventory.getQuantity("Corrupt Spirit Orb");
                    this.farmDSO(missingOrbs * 100);
                    this.doomMerge("Corrupt Spirit Orb", missingOrbs);
                }
                if (missingAuras > 0)
                    this.doomMerge("Ominous Aura", missingAuras);
            } else {
                this.doomKnightWK("Corrupt Spirit Orb", 5);
                this.doomKnightWK("Ominous Aura", 2);
            }
            Core.buyItem("dwarfhold", 434, fullMetalName);
        }

        if (!Core.checkInventory(fullMetalName)) {
            Core.logger(`Failed to obtain ${fullMetalName}. Cannot continue to the Forge Key quest.`, undefined, { stopBot: true });
            return;
        }

        if (!Story.questProgression(forgeKeyQuest))
        { // Unlocking "DoomSquire Weapon Kit" [Quest ID 2144]  
            Core.addDrop(fullMetalName);
            Core.addDrop(forgekeyitemID);
            Core.ensureAccept(forgeKeyQuest);
            while (!bot.shouldExit && !Core.checkInventory(forgekeyitemID))
                Core.killMonster("dwarfhold", "Enter", "Spawn", "Albino Bat");
            Core.ensureComplete(forgeKeyQuest);
            bot.wait.forPickup(fullMetalName);
        }
    }
}

export enum SDKAQuest {
    APennyforYourFoughts = 2089,
    DarkSpiritOrbs = 2065,
}

export const scriptOptions = defineScriptOptions("SupulchuresDoomKnightArmorOptions", [
    Core.skipOptions,
    option.choice("SelectedQuest", "Dark Spirit Orbs Quest", "Which quest should the bot use to farm Dark Spirit Orbs with?\nRecommended setting: A Penny for Your Foughts", choices(SDKAQuest), "APennyforYourFoughts"),
], true);
