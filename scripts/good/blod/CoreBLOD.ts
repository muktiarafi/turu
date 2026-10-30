/*
name: null
description: null
tags: null
*/
// Port of Scripts/Good/BLoD/CoreBLOD.cs.
// Not ported (FarmerJoe's DoAll doesn't reach them): BoneSomeDust.

import { bot } from "turu";
import type { InventoryItem, ShopItem } from "turu";
import { ClassType, Core, type CoreShopItem, type ShopRequirement } from "../../core/CoreBots";
import { CoreDailies, MineCraftingMetalsEnum } from "../../core/CoreDailies";
import { CoreFarms } from "../../core/CoreFarms";
import { CoreStory } from "../../core/CoreStory";
import { BattleUnder } from "../../story/BattleUnder";

const Farm = new CoreFarms();
const Daily = new CoreDailies();
const Story = new CoreStory();
const battleUnder = new BattleUnder();

export class CoreBLOD {
    bLoDItems: string[] = [
        "Blinding Light of Destiny",
        "Get Your Blinding Light of Destiny",
        // Coppers
        "Copper",
        "Celestial Copper",
        "Celestial Copper of Destiny",
        // Maces
        "Mace of Destiny",
        "Bright Mace of Destiny",
        "Blinding Mace of Destiny",
        // Silvers
        "Silver",
        "Sanctified Silver",
        "Sanctified Silver of Destiny",
        // Bows
        "Bow of Destiny",
        "Bright Bow of Destiny",
        "Blinding Bow of Destiny",
        // Bariums
        "Barium",
        "Blessed Barium",
        "Blessed Barium of Destiny",
        // Blades
        "Blade of Destiny",
        "Bright Blade of Destiny",
        "Blinding Blade of Destiny",
        // Weapon Kits
        "Basic Weapon Kit",
        "Advanced Weapon Kit",
        "Ultimate Weapon Kit",
        // Merge misc.
        "Bone Dust",
        "Undead Essence",
        "Undead Energy",
        "Blinding Light Fragments",
        "Spirit Orb",
        "Loyal Spirit Orb",
        "Bright Aura",
        "Brilliant Aura",
        "Blinding Aura",
    ];

    private lightMergeShopItems: CoreShopItem[] | null = null;

    blindingLightOfDestiny(method: BLODMethod = BLODMethod.Optimized): void {
        if (Core.checkInventory("Blinding Light of Destiny"))
            return;

        Core.addDrop(40187);
        if (!Core.checkInventory(40187)) // Get your Blinding Light of Destiny
        {
            if (Core.isCompletedBefore(7673))
                Core.chainComplete(7673);
            else {
                bot.drops.add(...this.bLoDItems);

                this.unlockMineCrafting();
                switch (method) {
                    case 0: // Fewest Dailies
                        this.getBlindingWeapon(WeaponOfDestiny.Blade);
                        break;
                    case 1: // Optimized
                        this.getBlindingWeapon(WeaponOfDestiny.Daggers);
                        this.getBlindingWeapon(WeaponOfDestiny.Mace);
                        break;
                    case 2: // Fewest Hours
                        this.getBlindingWeapon(WeaponOfDestiny.Mace);
                        this.getBlindingWeapon(WeaponOfDestiny.Bow);
                        this.getBlindingWeapon(WeaponOfDestiny.Blade);
                        break;
                }

                this.brilliantAura(75);
                this.brightAura(125);
                this.loyalSpiritOrb(250);
                this.spiritOrb(500);
                this.blindingAura(1);

                this.ultimateWK();

                Core.chainComplete(2180);
            }
        }
        Core.buyItem(bot.map.name, 1415, "Blinding Light of Destiny");
        Core.toBank(40187);
    }

    unlockMineCrafting(): void {
        if (Core.isCompletedBefore(2084))
            return;

        Story.preLoad(this.unlockMineCrafting);

        // 2066 - Reforging the Blinding Light
        Story.buyQuest(2066, "doomwood", 276, "Blinding Light of Destiny Handle");

        // 2067 - Secret Order of Undead Slayers
        Story.buyQuest(2067, "doomwood", 276, "Bonegrinder Medal");

        // 2082 - Essential Essences
        if (!Story.questProgression(2082)) {
            Core.ensureAccept(2082);
            Core.huntMonster("battleunderb", "Skeleton Warrior", "Undead Essence", 25, { isTemp: false });
            Core.ensureComplete(2082);
        }

        // 2083 - Bust some Dust
        if (!Story.questProgression(2083)) {
            Core.ensureAccept(2083);
            Core.huntMonster("battleunderb", "Skeleton Warrior", "Bone Dust", 40, { isTemp: false });
            Core.ensureComplete(2083);
        }

        // 2084 - A Loyal Follower
        if (!Story.questProgression(2084)) {
            Core.logger("Doing Quest: [2084] \"A Loyal Follower\"");
            Core.ensureAccept(2084);
            this.spiritOrb(100);
            Core.huntMonster("timevoid", "Ephemerite", "Celestial Compass");
            Core.ensureComplete(2084);
        }
    }

    spiritOrb(quant = 65000): void {
        if (Core.checkInventory("Spirit Orb", quant))
            return;

        this.farmFindingFrag(WeaponOfDestiny.Blade, "Spirit Orb", quant >= 65000 ? 65000 : quant);
        this.farmFindingFrag(WeaponOfDestiny.Broadsword, "Spirit Orb", quant >= 65000 ? 65000 : quant);

        // Default
        this.soulSearching("Spirit Orb", quant >= 65000 ? 65000 : quant);
    }

    loyalSpiritOrb(quant: number): void {
        if (Core.checkInventory("Loyal Spirit Orb", quant))
            return;

        this.farmFindingFrag(WeaponOfDestiny.Blade, "Loyal Spirit Orb", quant);
        this.farmFindingFrag(WeaponOfDestiny.Daggers, "Loyal Spirit Orb", quant);
        this.farmFindingFrag(WeaponOfDestiny.Scythe, "Loyal Spirit Orb", quant);

        if (bot.quests.isUnlocked(2163))
            this.farmUltimateWK("Loyal Spirit Orb", quant);

        // Default
        while (!bot.shouldExit && !Core.checkInventory("Loyal Spirit Orb", quant)) {
            Core.farmingLogger("Loyal Spirit Orb", quant);
            this.spiritOrb(100 * quant);
            this.lightMerge("Loyal Spirit Orb", quant);
        }
    }

    brightAura(quant: number): void {
        if (Core.checkInventory("Bright Aura", quant))
            return;

        this.farmFindingFrag(WeaponOfDestiny.Bow, "Bright Aura", quant);
        this.farmFindingFrag(WeaponOfDestiny.Broadsword, "Bright Aura", quant);
        this.farmFindingFrag(WeaponOfDestiny.Scythe, "Bright Aura", quant);
        if (bot.quests.isUnlocked(2163))
            this.farmUltimateWK("Bright Aura", quant);

        // Default
        while (!bot.shouldExit && !Core.checkInventory("Bright Aura", quant)) {
            Core.farmingLogger("Bright Aura", quant);
            this.loyalSpiritOrb(50 * quant);

            this.lightMerge("Bright Aura", quant);
        }
    }

    brilliantAura(quant: number): void {
        if (Core.checkInventory("Brilliant Aura", quant))
            return;

        this.farmFindingFrag(WeaponOfDestiny.Mace, "Brilliant Aura", quant);

        // Default
        while (!bot.shouldExit && !Core.checkInventory("Brilliant Aura", quant)) {
            Core.farmingLogger("Brilliant Aura", quant);
            this.brightAura(25 * quant);
            this.lightMerge("Brilliant Aura", quant);
        }
    }

    blindingAura(quant: number): void {
        if (Core.checkInventory("Blinding Aura", quant))
            return;

        this.farmFindingFrag(WeaponOfDestiny.Scythe, "Blinding Aura", quant);

        // Default (any of these)
        this.farmFindingFrag(WeaponOfDestiny.Blade, "Blinding Aura", quant);
        this.farmFindingFrag(WeaponOfDestiny.Broadsword, "Blinding Aura", quant);
        this.farmFindingFrag(WeaponOfDestiny.Bow, "Blinding Aura", quant);
        this.farmFindingFrag(WeaponOfDestiny.Mace, "Blinding Aura", quant);
        this.farmFindingFrag(WeaponOfDestiny.Daggers, "Blinding Aura", quant);
    }

    soulSearching(item = "Spirit Orb", quant = 1, farmSpiritOrbs = true): void {
        if (Core.checkInventory(item, quant))
            return;

        if (!bot.quests.isUnlocked(939))
            battleUnder.battleUnderC();

        Core.equipClass(ClassType.Solo);

        Core.addDrop(
            ...farmSpiritOrbs
                ? [
                    "Bone Dust",
                    "Undead Energy",
                    "Cavern Celestite",
                    "Undead Essence",
                    "Spirit Orb",
                ]
                : [ "Cavern Celestite", "Undead Essence", item ]);

        Core.registerQuests(...farmSpiritOrbs ? [ 2082, 2083, 939 ] : [ 939 ]);

        Core.farmingLogger(item, quant);

        while (!bot.shouldExit && !Core.checkInventory(item, quant)) {
            Core.killMonster("battleunderc", "r5", "Left", "Crystalized Jellyfish", "Jellyfish Soul", undefined, { log: false });
            Core.killMonster("battleundera", "r7", "Left", "Bone Terror", "Bone Terror Soul", undefined, { log: false });
            Core.killMonster("battleunderb", "r3", "Right", "Undead Champion", "Undead Champion Soul", undefined, { log: false });
            bot.wait.forPickup(item);
        }

        Core.cancelRegisteredQuests();
    }

    findingFragments(weapon: WeaponOfDestiny, item: string, quant = 1): void {
        if (Core.checkInventory(item, quant))
            return;

        this.getBlindingWeapon(weapon);

        const quest = {
            [WeaponOfDestiny.Bow]: 2174,
            [WeaponOfDestiny.Daggers]: 2175,
            [WeaponOfDestiny.Mace]: 2176,
            [WeaponOfDestiny.Scythe]: 2177,
            [WeaponOfDestiny.Broadsword]: 2178,
            [WeaponOfDestiny.Blade]: 2179,
        }[weapon] ?? 0;

        Core.logger("Selling \"Blinding Light Fragments\" before starting this as it can bug out and just stay at 1/1 forever.. ( blame ae).");
        Core.unbank("Blinding Light Fragments");
        Core.sellItem("Blinding Light Fragments", undefined, { all: true });

        Core.addDrop("Blinding Light Fragments");
        Core.addDrop(...Core.questRewards(quest));
        Core.equipClass(ClassType.Farm);
        Core.farmingLogger(item, quant);

        if (item === "Spirit Orb") {
            if (bot.quests.isUnlocked(2082)) {
                Core.addDrop("Undead Essence");
                Core.registerQuests(2082);
            }
            if (bot.quests.isUnlocked(2083)) {
                Core.addDrop("Bone Dust");
                Core.registerQuests(2083);
            }
        }

        while (!bot.shouldExit && !Core.checkInventory(item, quant)) {
            Core.ensureAccept(quest);
            Core.killMonster("battleunderb", "Enter", "Spawn", "*", "Blinding Light Fragments", 10, { isTemp: false, log: false });
            Core.ensureComplete(quest);
        }
        Core.cancelRegisteredQuests();
    }

    private farmFindingFrag(weapon: WeaponOfDestiny, item: string, quant: number): void {
        if (
            !Core.checkInventory(item, quant)
            && Core.isCompletedBefore(2163)
            && (Core.checkInventory(`Blinding ${WeaponOfDestiny[weapon]} of Destiny`) || Core.checkInventory(40187))
        ) // Basically means you can buy it from the final shop
            this.findingFragments(weapon, item, quant);
    }

    private farmUltimateWK(item: string, quant: number): void {
        // We'll be able to use this later once we unlock UWK
        if (!Core.checkInventory(item, quant))
            this.ultimateWK(item, quant);
    }

    basicWK(quant = 1): void {
        if (Core.checkInventory("Basic Weapon Kit", quant))
            return;

        Core.equipClass(ClassType.Farm);
        Core.farmingLogger("Basic Wepon Kit", quant);
        Core.addDrop("Basic Weapon Kit");

        if (!bot.quests.isUnlocked(2136)) {
            Core.logger("Unlocking \"Basic Weapon Kit\" Quest!");
            // Define valid metals in enum form
            const validMetals = [
                MineCraftingMetalsEnum.Aluminum,
                MineCraftingMetalsEnum.Barium,
                MineCraftingMetalsEnum.Gold,
                MineCraftingMetalsEnum.Iron,
                MineCraftingMetalsEnum.Copper,
                MineCraftingMetalsEnum.Silver,
                MineCraftingMetalsEnum.Platinum,
            ];

            // Find first matching item in inventory/bank
            const itemToUpgrade = bot
                .inventory.items.concat(bot.bank.items)
                .find(i => i.name in MineCraftingMetalsEnum); // C#: Enum.TryParse<MineCraftingMetalsEnum>(i.Name, out _)

            // Upgrade the metal (fallback to Aluminum if none found)
            this.upgradeMetal(
                itemToUpgrade != null
                    ? MineCraftingMetalsEnum[itemToUpgrade.name as keyof typeof MineCraftingMetalsEnum]
                    : MineCraftingMetalsEnum.Aluminum);
        }

        while (!bot.shouldExit && !Core.checkInventory("Basic Weapon Kit", quant)) {
            Core.ensureAccept(2136);
            Core.killMonster("forest", "Forest3", "Left", "*", "Zardman's StoneHammer", 1, { isTemp: false });
            Core.killMonster("noobshire", "North", "Left", "Horc Noob", "Noob Blade Oil");
            Core.killMonster("farm", "Crop1", "Right", "Scarecrow", "Burlap Cloth", 4);
            if (!bot.quests.isUnlocked(4614))
                bot.quests.updateQuest(4614);
            Core.huntMonster("pyramid", "Mummy", "Triple Ply Mummy Wrap", 7);
            Core.huntMonster("pyramid", "Golden Scarab", "Golden Lacquer Finish");
            Core.huntMonster("lair", "Bronze Draconian", "Bronze Brush");
            Core.huntMonster("bloodtusk", "Rock", "Rocky Stone Sharpener");
            Core.ensureComplete(2136);

            bot.wait.forPickup("Basic Weapon Kit");
        }
    }

    advancedWK(quant = 1): void {
        if (Core.checkInventory("Advanced Weapon Kit", quant))
            return;

        if (!bot.quests.isUnlocked(2162)) {
            Core.logger("Unlocking \"Advanced Weapon Kit\" Quest!");
            this.basicWK();
        }

        Core.farmingLogger("Advanced Weapon Kit", quant);
        Core.addDrop("Advanced Weapon Kit");

        while (!bot.shouldExit && !Core.checkInventory("Advanced Weapon Kit", quant)) {
            Core.ensureAccept(2162);
            Core.equipClass(ClassType.Solo);
            Core.huntMonster("hachiko", "Dai Tengu", "Superior Blade Oil");
            Core.huntMonster("airstorm", "Lightning Ball", "Shining Lacquer Finish");
            Core.huntMonster("faerie", "Cyclops Warlord", "Brass Awl");
            Core.huntMonster("darkoviaforest", "Lich of the Stone", "Slate Stone Sharpener");

            Core.equipClass(ClassType.Farm);
            Core.killMonster("safiria", "c3", "Left", "Chaos Lycan", "WolfClaw Hammer", 1, { isTemp: false });
            Core.killMonster("lycan", "r4", "Left", "Chaos Vampire Knight", "Silver Brush");
            Core.killMonster("sandport", "r3", "Right", "Tomb Robber", "Leather Case");
            Core.killMonster("pines", "Path1", "Left", "LeatherWing", "Leatherwing Hide", 10);
            Core.ensureComplete(2162);

            bot.wait.forPickup("Advanced Weapon Kit");
        }
    }

    ultimateWK(item = "Ultimate Weapon Kit", quant = 1): void {
        if (Core.checkInventory(item, quant))
            return;

        if (!bot.quests.isUnlocked(2163)) {
            Core.logger("Unlocking \"Ultimate Weapon Kit\" Quest!");
            this.advancedWK();
        }

        Core.addDrop(
            "Ultimate Weapon Kit", 
            "Blinding Light Fragments", 
            "Bright Aura", 
            "Spirit Orb", 
            "Loyal Spirit Orb");
        Core.farmingLogger(item, quant);
        bot.quests.updateQuest(999);
        while (!bot.shouldExit && !Core.checkInventory(item, quant)) {
            Core.ensureAccept(2163);
            Core.equipClass(ClassType.Farm);
            Core.killMonster("dragonplane", "r2", "Right", "Earth Elemental", "Great Ornate Warhammer", 1, { isTemp: false, log: false });

            Core.equipClass(ClassType.Solo);
            Core.killMonster("greendragon", "Boss", "Left", "Greenguard Dragon", "Greenguard Dragon Hide", 3, { log: false });
            Core.killMonster("sandcastle", "r7", "Left", "Chaos Sphinx", "Gold Brush", undefined, { log: false });
            Core.killMonster("crashsite", "Boss", "Left", "ProtoSartorium", "Non-abrasive Power Powder", undefined, { log: false });

            //ensure we're not aggroed in crashsite by the robots in boss cell
            Core.jumpWait();

            Core.killKitsune("No. 1337 Blade Oil", undefined, { log: false });
            Core.killMonster("citadel", "m14", "Left", "Grand Inquisitor", "Blinding Lacquer Finish", undefined, { log: false });
            Core.huntMonster("djinn", "Harpy", "Suede Travel Case", undefined, { log: false });
            Core.killMonster("roc", "Enter", "Spawn", "Rock Roc", "Sharp Stone Sharpener", undefined, { log: false });
            Core.ensureComplete(2163);

            bot.wait.forPickup(item);
        }
    }

    getBlindingWeapon(weapon: WeaponOfDestiny): void {
        const weaponName = `Blinding ${WeaponOfDestiny[weapon]} of Destiny`;
        if (Core.checkInventory(weaponName))
            return;

        // Bypass any farms if you already farmed BLOD in the past
        if (Core.checkInventory(40187)) // Get Your Blinding Light of Destiny
        {
            Core.buyItem(bot.map.name, 1415, weaponName);
            Core.toBank(40187);
            return;
        }

        this.getBrightWeapon(weapon);
        this.ultimateWK();
        this.lightMerge(weaponName);
    }

    getBrightWeapon(weapon: WeaponOfDestiny): void {
        const weaponName = `Bright ${WeaponOfDestiny[weapon]} of Destiny`;
        if (Core.checkInventory(weaponName))
            return;

        this.getBaseWeapon(weapon);

        const weaponReqs = (this.lightMergeShopItems ??= Core.getShopItems("necropolis", 422))
            .find(item => item.name === weaponName)!
            .requirements;

        this.getMergeRequirements(
            weaponReqs,
            weaponReqs.find(item => item.name === WeaponOfDestiny[weapon] + " of Destiny")!.id);
        this.lightMerge(weaponName);
    }

    getBaseWeapon(weapon: WeaponOfDestiny): void {
        const weaponName = WeaponOfDestiny[weapon] + " of Destiny";
        if (Core.checkInventory(weaponName))
            return;

        const weaponReqs = (this.lightMergeShopItems ??= Core.getShopItems("necropolis", 422))
            .find(item => item.name === weaponName)!
            .requirements;
        const metal = weaponReqs.find(req => req.name.endsWith("of Destiny"))!;
        this.upgradeMetal(
            MineCraftingMetalsEnum[metal.name.split(" ")[1] as keyof typeof MineCraftingMetalsEnum]);

        this.getMergeRequirements(weaponReqs, metal.id);
        this.lightMerge(weaponName);
    }

    private getMergeRequirements(requirements: ShopRequirement[], ...exceptions: number[]): void {
        for (const req of requirements.filter(r => !exceptions.includes(r.id))) {
            switch (req.id) {
                case 12503: // Basic Weapon Kit
                    this.basicWK(req.quantity);
                    break;
                case 12544: // Advanced Weapon Kit
                    this.advancedWK(req.quantity);
                    break;
                case 12184: // Spirit Orb
                    this.spiritOrb(req.quantity);
                    break;
                case 12311: // Loyal Spirit Orb
                    this.loyalSpiritOrb(req.quantity);
                    break;
                case 6537: // Brilliant Aura (why is the ID so far away from the others)
                    this.brilliantAura(req.quantity);
                    break;
                case 6535: // Bright Aura
                    this.brightAura(req.quantity);
                    break;
                case 11285: // Undead Energy
                    Farm.battleUnderB("Undead Energy", req.quantity);
                    break;
            }
        }
    }

    upgradeMetal(metal: MineCraftingMetalsEnum): void {
        let fullMetalName = "";
        let upgradeMetalQuest = 0;
        let forgeKeyQuest = 0;
        let forgekeyitemID = 0;
        switch (metal) {
            case MineCraftingMetalsEnum.Aluminum:
                fullMetalName = "Almighty Aluminum of Destiny";
                upgradeMetalQuest = 2103;
                forgeKeyQuest = 2129;
                break;
            case MineCraftingMetalsEnum.Barium:
                fullMetalName = "Blessed Barium of Destiny";
                upgradeMetalQuest = 2104;
                forgeKeyQuest = 2130;
                break;
            case MineCraftingMetalsEnum.Gold:
                fullMetalName = "Glorious Gold of Destiny";
                upgradeMetalQuest = 2105;
                forgeKeyQuest = 2131;
                break;
            case MineCraftingMetalsEnum.Iron:
                fullMetalName = "Immortal Iron of Destiny";
                upgradeMetalQuest = 2106;
                forgeKeyQuest = 2132;
                break;
            case MineCraftingMetalsEnum.Copper:
                fullMetalName = "Celestial Copper of Destiny";
                upgradeMetalQuest = 2107;
                forgeKeyQuest = 2133;
                break;
            case MineCraftingMetalsEnum.Silver:
                fullMetalName = "Sanctified Silver of Destiny";
                upgradeMetalQuest = 2108;
                forgeKeyQuest = 2134;
                break;
            case MineCraftingMetalsEnum.Platinum:
                fullMetalName = "Pious Platinum of Destiny";
                upgradeMetalQuest = 2109;
                forgeKeyQuest = 2135;
                break;
        }

        // Initialize quest data for forge key quest
        const ForgeQuestdata = Core.initializeWithRetries(() => Core.ensureLoad(forgeKeyQuest));
        if (ForgeQuestdata == null) {
            Core.logger(`Failed to load the quest ID ${forgeKeyQuest} for the Forge Key`);
            return;
        }
        // Get the forge key itemid for the quest
        forgekeyitemID =
            ForgeQuestdata.requirements.find(x => x != null && x.name === "Forge Key")?.id
            ?? 0;
        if (forgekeyitemID === 0) {
            Core.logger(
                `Failed to find the item ID for the Forge Key from the quest ID ${forgeKeyQuest}`);
            return;
        }
        const upgradeMetalName = fullMetalName.split(" ").slice(0, 2).join(" ");

        if (!Core.checkInventory(fullMetalName)) {
            // Getting the name of the metal used to upgrade
            Core.farmingLogger(fullMetalName, 1);

            // Getting the partially upgraded metal
            if (!Core.checkInventory(upgradeMetalName)) {
                Core.addDrop(upgradeMetalName);
                Core.farmingLogger(upgradeMetalName, 1);
                Core.ensureAccept(upgradeMetalQuest);

                if (!Core.checkInventory(metal))
                    Daily.mineCrafting([MineCraftingMetalsEnum[metal]]);
                if (!Core.checkInventory(metal))
                    Core.logger(
                        `Can't complete ${fullMetalName.split(" ").slice(0, 2).join(" ")} Enchantment (missing ${MineCraftingMetalsEnum[metal]}).\n`
                            + "This requires a daily, please run the bot again after the daily reset has occurred.", undefined, { messageBox: true, stopBot: true });

                Farm.battleUnderB("Undead Energy", 25);
                this.spiritOrb(5);
                Core.huntMonster("arcangrove", "Seed Spitter", "Paladaffodil", 25);

                Core.ensureComplete(upgradeMetalQuest);
                bot.wait.forPickup(upgradeMetalName);
            }

            // Getting the fully upgraded metal
            this.brightAura(2);
            this.loyalSpiritOrb(5);
            Core.buyItem("dwarfhold", 434, fullMetalName);
        }

        // Check if "Basic Weapon Kits [Quest ID 2136] is unlocked
        if (!bot.quests.isUnlocked(2136)) {
            bot.log("Doing Quest to Unlock Basic Weapon Kit (hopefully...)");
            Core.addDrop(fullMetalName);
            Core.ensureAccept(forgeKeyQuest);
            while (!bot.shouldExit && !Core.checkInventory(forgekeyitemID))
                Core.killMonster("dwarfhold", "Enter", "Spawn", "Albino Bat");
            Core.ensureComplete(forgeKeyQuest);
            bot.wait.forPickup(fullMetalName);
        }
    }

    private lightMerge(item: string, quant = 1): void {
        Core.buyItem("necropolis", 422, item, quant);
    }
}

export enum WeaponOfDestiny {
    Daggers,
    Bow,
    Mace,
    Scythe,
    Broadsword,
    Blade,
}

export enum BLODMethod {
    Fewest_Dailies,
    Optimized,
    Fewest_Hours,
}
