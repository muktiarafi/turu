/*
name: CoreNation
description: Port of the parts of Scripts/Nation/CoreNation.cs that farm the Nulgath Nation
    materials (bag drops).
tags: core, nation, nulgath
*/
/**
 * CoreNation — partial TypeScript port of `Scripts/Nation/CoreNation.cs`.
 *
 * Only the members reachable from `0MaxBags` and the FarmerJoe tree are ported (the material
 * farms and the quests they use, Swindles Return, the Shadow Blast medals and emblems, The
 * Assistant); port more from the C# file as scripts need them.
 *
 * C# `[CanChange]` fields: {@link CoreNation.sellMemVoucher} and
 * {@link CoreNation.returnPolicyDuringSupplies}. The CoreBots Options keys `Nation_SellMemVoucher`,
 * `Nation_ReturnPolicyDuringSupplies` and `UltraAlteonForSupplies` are read from
 * `Core.cboValues` (see CoreBots.ts).
 */

import { bot } from "turu";
import { ClassType, Core, type QuestData, type RewardItem, type CoreShopItem } from "../core/CoreBots";
import { CoreFarms } from "../core/CoreFarms";

/** Rewards of Contract Exchange (quest 870), by item ID. */
export enum ContractExchangeRewards {
    Tainted_Gem = 4769,
    Dark_Crystal_Shard = 4770,
    Diamond_of_Nulgath = 4771,
    Gem_of_Nulgath = 6136,
    Blood_Gem_of_the_Archfiend = 22332,
    All = 9999,
}

/** Rewards of Swindles Return Policy (quest 7551), by item ID; `None` for no preference. */
export enum SwindlesReturnReward {
    Tainted_Gem = 4769,
    Dark_Crystal_Shard = 4770,
    Diamond_of_Nulgath = 4771,
    Gem_of_Nulgath = 6136,
    Blood_Gem_of_the_Archfiend = 22332,
    None = 0,
}

/** Options of {@link CoreNation.supplies}. */
export interface SuppliesOptions {
    /** Farm Relics of Chaos from Ultra Chaos Alteon instead of Escherion. */
    ultraAlteon?: boolean;
    /** Keep the Voucher of Nulgath (don't sell it). */
    keepVoucher?: boolean;
    /** Spend gold on "War-Torn Memorabilia" (quest 2859) during the farm. */
    assistantDuring?: boolean;
    /** Reward to pick from Swindles Return Policy. */
    returnItem?: string | null;
    /** Do Swindles Return Policy during the farm. */
    returnPolicyDuringSupplies?: boolean;
    /** Convert vouchers (quest 4778) during a single-item farm. */
    voucherItemDuring?: boolean;
    /** Farm Relics of Chaos in /hydrachallenge. */
    hydraChallenge?: boolean;
}

/** Options of {@link CoreNation.bambloozevsDrudgen}. */
export interface BambloozeOptions {
    keepVoucher?: boolean;
    assistantDuring?: boolean;
    returnItem?: string | null;
    /** The call came from {@link CoreNation.supplies} (no mode log line). */
    camefromSupplies?: boolean;
}

export class CoreNation {
    private static _farm: CoreFarms | null = null;
    private static get farm(): CoreFarms {
        return (CoreNation._farm ??= new CoreFarms());
    }

    /** CanChange: If enabled will sell the "Voucher of Nulgath" item during farms if it's not needed. */
    sellMemVoucher = true;

    /** CanChange: If enabled will do "Swindles Return Policy" passively during "Supplies To Spin The Wheels of Fate". */
    returnPolicyDuringSupplies = true;

    /** Crag and Bamboozle name in game. */
    get cragName(): string {
        return "Crag &amp; Bamboozle";
    }

    /** All principal drops from Nulgath. */
    bagDrops: string[] = [
        "Blood Gem of the Archfiend",
        "Dark Crystal Shard",
        "Diamond of Nulgath",
        "Gem of Nulgath",
        "Totem of Nulgath",
        "Tainted Gem",
        "Unidentified 10",
        "Unidentified 13",
        "Voucher of Nulgath",
        "Voucher of Nulgath (non-mem)",
        // extras
        "Unidentified 24",
        "Essence of Nulgath",
        "Unidentified 25",
        "Fiend Token",
        "Emblem of Nulgath",
        "Receipt of Swindle",
        "Bone Dust",
        "Nulgath's Approval",
        "Archfiend's Favor",
        "Unidentified 34",
        "Essence of Nulgath",
    ];

    suppliesRewards: string[] = [
        "Tainted Gem",
        "Dark Crystal Shard",
        "Diamond of Nulgath",
        "Voucher of Nulgath",
        "Voucher of Nulgath (non-mem)",
        "Gem of Nulgath",
        "Unidentified 10",
        "Essence of Nulgath",
        "Receipt of Swindle",
    ];

    /** Drops from the bosses that used to give acess to tercess. */
    tercessBags: string[] = ["Bone Dust"];

    /** List of Betrayal Blades. */
    betrayalBlades: string[] = [
        "1st Betrayal Blade of Nulgath",
        "2nd Betrayal Blade of Nulgath",
        "3rd Betrayal Blade of Nulgath",
        "4th Betrayal Blade of Nulgath",
        "5th Betrayal Blade of Nulgath",
        "6th Betrayal Blade of Nulgath",
        "7th Betrayal Blade of Nulgath",
        "8th Betrayal Blade of Nulgath",
    ];

    /** Shadow Blast Arena medals. */
    nationMedals: string[] = ["Nation Round 1 Medal", "Nation Round 2 Medal", "Nation Round 3 Medal", "Nation Round 4 Medal"];

    receipt: string[] = [
        "Unidentified 1",
        "Unidentified 6",
        "Unidentified 9",
        "Unidentified 16",
        "Unidentified 20",
        "Receipt of Swindle",
        "Dark Crystal Shard",
        "Diamond of Nulgath",
        "Gem of Nulgath",
        "Blood Gem of the Archfiend",
    ];

    /** Misc items to accept during Bloody Chaos if turned on. */
    bloodyChaosSupplies: string[] = [
        "Tainted Gem",
        "Dark Crystal Shard",
        "Diamond of Nulgath",
        "Voucher of Nulgath",
        "Voucher of Nulgath (non-mem)",
        "Unidentified 10",
        "Unidentified 13",
        "Gem of Nulgath",
        "Relic of Chaos",
    ];

    swindlesReturn: string[] = ["Unidentified 1", "Unidentified 6", "Unidentified 9", "Unidentified 16", "Unidentified 20"];

    swindlesReturnRewards: string[] = [
        "Tainted Gem",
        "Dark Crystal Shard",
        "Diamond of Nulgath",
        "Gem of Nulgath",
        "Blood Gem of the Archfiend",
        "Receipt of Swindle",
    ];

    uni(nr: number): string {
        return `Unidentified ${nr}`;
    }

    /**
     * Does Essence of Defeat Reagent quest for Dark Crystal Shards.
     * @param quant Desired quantity, 1000 = max stack.
     */
    essenceofDefeatReagent(quant = 1000): void {
        if (Core.checkInventory("Dark Crystal Shard", quant)) return;

        Core.addDrop(...this.tercessBags, ...this.bagDrops);
        Core.farmingLogger("Dark Crystal Shard", quant);

        Core.registerQuests(570);
        while (!bot.shouldExit && !Core.checkInventory("Dark Crystal Shard", quant)) {
            Core.equipClass(ClassType.Solo);
            Core.huntMonster("faerie", "Aracara", "Aracara's Fang", 1, { isTemp: false, log: false });
            Core.huntMonster("hydra", "Hydra Head", "Hydra Scale", 1, { isTemp: false, log: false });
            Core.killVath("Strand of Vath's Hair", 1, { isTemp: false });
            Core.huntMonster("yokaiwar", "O-dokuro's Head", "O-dokuro's Tooth", 1, { isTemp: false, log: false });
            Core.killEscherion("Escherion's Chain", 1, { publicRoom: true });

            Core.equipClass(ClassType.Farm);
            Core.killMonster("tercessuinotlim", "m2", "Left", "*", "Defeated Makai", 50, { isTemp: false, log: false });

            Core.equipClass(ClassType.Solo);
            Core.huntMonster("djinn", "Tibicenas", "Tibicenas' Chain", 1, { publicRoom: true, log: false });
            bot.wait.forPickup("Dark Crystal Shard");
        }
        Core.cancelRegisteredQuests();
    }

    /** Kills Slugfit and Cyclops Warlord in /mobius (Slugfit cell) until you have the horns. */
    private farmMobiusHorns(horns: [mobName: string, itemName: string, quantity: number][], skipOwned: boolean): void {
        for (const [mobName, itemName, quantity] of horns) {
            if (skipOwned && Core.checkInventory(itemName, quantity)) continue;

            while (!bot.shouldExit && !Core.checkInventory(itemName, quantity)) {
                if (bot.monsters.currentAvailableMonsters.some((monster) => monster.name === mobName)) {
                    Core.canWeAggro();
                    bot.combat.attack(mobName);
                } else Core.sleep();
                // The single-item branch of C# sleeps once more per loop.
                if (!skipOwned) Core.sleep();
            }
            bot.wait.forPickup(itemName);
        }
    }

    /**
     * Does NWNO from Nulgath's Birthday Gift/Bounty Hunter's Drone Pet.
     * @param item Desired item to get (null: max every reward).
     * @param quant Desired quantity to get.
     */
    newWorldsNewOpportunities(item: string | null = null, quant = 1): void {
        if ((item !== null && Core.checkInventory(item, quant)) || !Core.checkInventory(["Nulgath's Birthday Gift", "Bounty Hunter's Drone Pet"], 1, { any: true })) return;

        Core.addDrop(...Core.questRewards(Core.checkInventory("Bounty Hunter's Drone Pet") ? 6183 : 6697));
        Core.equipClass(ClassType.Farm);

        Core.registerQuests(Core.checkInventory("Bounty Hunter's Drone Pet") ? 6183 : 6697);
        if (item === null) {
            const questRewards = Core.ensureLoad(Core.checkInventory("Bounty Hunter's Drone Pet") ? 6183 : 6697).rewards;
            for (const reward of questRewards) {
                if (Core.checkInventory(reward.name, reward.maxStack)) continue;

                Core.farmingLogger(reward.name, reward.maxStack);

                while (!bot.shouldExit && !Core.checkInventory(reward.id, reward.maxStack)) {
                    if (!Core.checkInventory("Slugfit Horn", 10) || !Core.checkInventory("Cyclops Horn", 6)) {
                        Core.joinSWF("mobius", "ChiralValley/town-Mobius-21Feb14.swf", "Slugfit", "Bottom");
                        if (bot.player.cell !== "Slugfit") Core.jump("Slugfit", "Bottom");

                        this.farmMobiusHorns(
                            [
                                ["Slugfit", "Slugfit Horn", 10],
                                ["Cyclops Warlord", "Cyclops Horn", 6],
                            ],
                            true,
                        );
                    }

                    Core.killMonster("tercessuinotlim", "m2", "Top", "Dark Makai", "Makai Fang", 10, { log: false });
                    Core.killMonster("hydra", "Rune2", "Left", "Fire Imp", "Imp Flame", 6, { log: false });
                    Core.killMonster("greenguardwest", "West12", "Up", "Big Bad Boar", "Wereboar Tusk", 4, { log: false });
                }
            }
            Core.logger("all items quant maxed");
            Core.cancelRegisteredQuests();
        } else {
            Core.farmingLogger(item, quant);
            while (!bot.shouldExit && !Core.checkInventory(item, quant)) {
                if (!Core.checkInventory("Slugfit Horn", 5) || !Core.checkInventory("Cyclops Horn", 3)) {
                    Core.joinSWF("mobius", "ChiralValley/town-Mobius-21Feb14.swf", "Slugfit", "Bottom");

                    this.farmMobiusHorns(
                        [
                            ["Slugfit", "Slugfit Horn", 5],
                            ["Cyclops Warlord", "Cyclops Horn", 3],
                        ],
                        false,
                    );
                }

                Core.killMonster("tercessuinotlim", "m2", "Top", "Dark Makai", "Makai Fang", 5, { log: false });
                Core.killMonster("hydra", "Rune2", "Left", "Fire Imp", "Imp Flame", 3, { log: false });
                Core.killMonster("greenguardwest", "West12", "Up", "Big Bad Boar", "Wereboar Tusk", 2, { log: false });
            }
            Core.logger(`${item} is now maxed`);
            Core.cancelRegisteredQuests();
        }
        Core.cancelRegisteredQuests();
    }

    /**
     * Farms Approvals and Favors in Evil War Nul.
     * @param quantApproval Desired quantity for Approvals, 5000 = max stack.
     * @param quantFavor Desired quantity for Favors, 5000 = max stack.
     */
    approvalAndFavor(quantApproval = 5000, quantFavor = 5000): void {
        if (Core.checkInventory("Nulgath's Approval", quantApproval) && Core.checkInventory("Archfiend's Favor", quantFavor)) return;

        Core.addDrop("Nulgath's Approval", "Archfiend's Favor");

        Core.farmingLogger("Nulgath's Approval", quantApproval);
        Core.farmingLogger("Archfiend's Favor", quantFavor);

        Core.equipClass(ClassType.Farm);
        // C# checks ShouldExit only with the Approvals: `(!ShouldExit && approvals < quant) || favors < quant`.
        while (!bot.shouldExit && (bot.inventory.getQuantity("Nulgath's Approval") < quantApproval || bot.inventory.getQuantity("Archfiend's Favor") < quantFavor)) {
            if (bot.map.name !== "evilwarnul") {
                Core.join("evilwarnul");
                bot.wait.forMapLoad("evilwarnul");
            }
            if (bot.player.cell !== "r9") {
                bot.map.jump("r9", "Left", false);
                bot.wait.forCellChange("r9");
            }
            Core.canWeAggro();
            bot.combat.attack("*");
            Core.sleep();
        }
    }

    /** Farms an item with Swindles Return Policy (quest 7551). */
    swindleReturn(item: string | null = null, quant = 1000): void {
        const quest = Core.initializeWithRetries(() => Core.ensureLoad(7551));
        if (!quest) {
            Core.logger("Swindles Return Policy quest not found.");
            return;
        }
        const reward = quest.rewards.find((x) => x.name === item);

        if (!reward || Core.checkInventory(reward.name, quant)) return;

        Core.addDrop(...this.receipt);
        if (item !== null) Core.addDrop(reward.id);

        Core.farmingLogger(reward.name, quant);

        while (!bot.shouldExit && !Core.checkInventory(reward.name, quant)) {
            Core.ensureAccept(7551);
            this.supplies("Unidentified 1", 1, { returnItem: item });
            this.supplies("Unidentified 6", 1, { returnItem: item });
            this.supplies("Unidentified 9", 1, { returnItem: item });
            this.supplies("Unidentified 16", 1, { returnItem: item });
            this.supplies("Unidentified 20", 1, { returnItem: item });
            Core.resetQuest(7551);
            Core.darkMakaiItem("Dark Makai Rune");
            Core.ensureComplete(7551, reward.id);
            this.sellVoucherOfNulgath(this.sellMemVoucher, item);

            Core.farmingLogger(reward.name, quant);
        }
    }

    // C# SwindleBulk: not reached by the ported scripts, not ported yet.

    /**
     * Farms items with Contract Exchange (needs "Drudgen the Assistant").
     * @param item The item to farm. If null, it farms every reward.
     * @param quant Desired quantity, 1000 = max stack.
     */
    farmContractExchage(item: string | null = null, quant = 1): void {
        if (!Core.checkInventory("Drudgen the Assistant") || (item !== null && Core.checkInventory(item, quant))) {
            if (!Core.checkInventory("Drudgen the Assistant")) Core.logger('Missing "Drudgen the Assistant"');
            return;
        }

        const rewards = ["Tainted Gem", "Dark Crystal Shard", "Gem of Nulgath", "Blood Gem of the Archfiend"];

        Core.equipClass(ClassType.Farm);
        Core.addDrop(...Core.questRewards(870));

        const exchange = (rewardName: string | null, target: number, returnItem: string): void => {
            switch (rewardName) {
                case "Tainted Gem":
                    this.supplies("Diamond of Nulgath", 45, { returnItem });
                    this.contractExchange(ContractExchangeRewards.Tainted_Gem, target);
                    break;
                case "Dark Crystal Shard":
                    this.supplies("Diamond of Nulgath", 45, { returnItem });
                    this.contractExchange(ContractExchangeRewards.Dark_Crystal_Shard, target);
                    break;
                case "Gem of Nulgath":
                    this.supplies("Diamond of Nulgath", 45, { returnItem });
                    this.contractExchange(ContractExchangeRewards.Gem_of_Nulgath, target);
                    break;
                case "Blood Gem of the Archfiend":
                    this.supplies("Diamond of Nulgath", 45, { returnItem });
                    this.contractExchange(ContractExchangeRewards.Blood_Gem_of_the_Archfiend, target);
                    break;
            }
        };

        if (item !== null) {
            const reward = Core.ensureLoad(870).rewards.find((x) => x && x.name === item);
            if (!reward) {
                Core.logger(`Reward item "${item}" not found.`);
                return;
            }

            const target = quant > 1 ? quant : reward.maxStack;
            Core.farmingLogger(reward.name, target);
            while (!bot.shouldExit && !Core.checkInventory(reward.name, target)) exchange(reward.name, target, "Diamond of Nulgath");
        } else {
            for (const thing of rewards) {
                // C# falls back to an empty ItemBase (null name: counts as owned).
                const reward: { name: string | null; maxStack: number } = Core.ensureLoad(870).rewards.find((x) => x.name === thing) ?? {
                    name: null,
                    maxStack: 0,
                };
                Core.farmingLogger(reward.name, quant);
                while (!bot.shouldExit && !Core.checkInventory(reward.name, quant > 1 ? quant : reward.maxStack))
                    exchange(reward.name, quant > 1 ? quant : reward.maxStack, thing);
            }
        }
    }

    /**
     * Farms Emblems of Nulgath in Shadow Blast Arena (quest 4748), after the Nation Round medals.
     * @param quant Desired quantity, 500 = max stack.
     */
    emblemofNulgath(quant = 500): void {
        if (Core.checkInventory("Emblem of Nulgath", quant)) return;
        this.nationRound4Medal();

        Core.addDrop("Fiend Seal", "Gem of Domination", "Emblem of Nulgath");
        Core.addDrop(...this.bagDrops);
        Core.equipClass(ClassType.Farm);
        Core.farmingLogger("Emblem of Nulgath", quant);

        Core.registerQuests(4748);
        while (!bot.shouldExit && !Core.checkInventory("Emblem of Nulgath", quant)) {
            Core.huntMonster("shadowblast", "Shadowrise Guard", "Gem of Domination", 1, { isTemp: false, log: false });
            Core.huntMonster("shadowblast", "Legion Fenrir", "Fiend Seal", 25, { isTemp: false, log: false });
        }
    }

    /** Farms the Nation Round 1 to 4 medals in Shadow Blast Arena (quests 4744-4747). */
    nationRound4Medal(): void {
        if (Core.checkInventory("Nation Round 4 Medal")) {
            Core.logger("Medal 4 owned, no need to farm it");
            return;
        }

        for (const medal of ["Nation Round 1 Medal", "Nation Round 2 Medal", "Nation Round 3 Medal", "Nation Round 4 Medal"]) {
            Core.addDrop(medal);
            if (Core.checkInventory(medal)) {
                Core.logger(`"${medal}" owned.`);
                continue;
            }
            switch (medal) {
                // The Nation Needs YOU!
                case "Nation Round 1 Medal":
                    Core.ensureAccept(4744);
                    Core.huntMonster("shadowblast", "Legion AirStrike", "Legion Rookie Defeated", 5);
                    Core.huntMonster("shadowblast", "Shadowrise Guard", "Shadowscythe Rookie Defeated", 5);
                    Core.ensureComplete(4744);
                    break;

                // Show Me More, Nation-Noob
                case "Nation Round 2 Medal":
                    Core.ensureAccept(4745);
                    Core.huntMonster("shadowblast", "Legion Fenrir", "Legion Veteran Defeated", 7);
                    Core.huntMonster("shadowblast", "Doombringer", "Shadowscythe Veteran Defeated", 7);
                    Core.ensureComplete(4745);
                    break;

                // For the Nation!
                case "Nation Round 3 Medal":
                    Core.ensureAccept(4746);
                    Core.huntMonster("shadowblast", "Legion Cannon", "Legion Elite Defeated", 10);
                    Core.huntMonster("shadowblast", "Draconic Doomknight", "Shadowscythe Elite Defeated", 10);
                    Core.ensureComplete(4746);
                    break;

                // Nulgath Likes Your Style
                case "Nation Round 4 Medal":
                    Core.ensureAccept(4747);
                    Core.huntMonster("shadowblast", "Grimlord Boss", "Grimlord Vanquished");
                    Core.ensureComplete(4747);
                    break;
            }

            bot.wait.forPickup(medal);
            Core.logger(`Medal ${medal} acquired`);
            bot.drops.remove(medal);
        }
    }

    // C# VoucherItemTotemofNulgath: not reached by the ported scripts, not ported yet.

    /**
     * Farms Essences of Nulgath from Dark Makais in Tercessuinotlim.
     * @param quant Desired quantity, 100 = max stack.
     */
    essenceofNulgath(quant = 60): void {
        if (Core.checkInventory("Essence of Nulgath", quant)) return;

        Core.addDrop("Essence of Nulgath");
        Core.equipClass(ClassType.Farm);
        Core.killMonster("tercessuinotlim", "m2", "Left", "Dark Makai", "Essence of Nulgath", quant, { isTemp: false });
        Core.jumpWait();
    }

    /**
     * "Supplies To Spin The Wheels of Fate" (quest 2857): farms Relics of Chaos for an item (or
     * every reward), with the voucher/assistant/return-policy extras.
     * @param item The item to farm, null or "All" for every reward.
     */
    supplies(item: string | null = null, quant = 1, options: SuppliesOptions = {}): void {
        let { ultraAlteon = false, returnPolicyDuringSupplies = false } = options;
        const { keepVoucher = false, assistantDuring = false, returnItem = null, voucherItemDuring = false, hydraChallenge = false } = options;

        // Early exits: item already owned
        if (item !== null && Core.checkInventory(item, quant)) return;

        // All rewards already maxed when farming everything
        const quest = Core.initializeWithRetries(() => Core.ensureLoad(2857));
        if (
            item === null &&
            quest?.rewards &&
            this.bagDrops.slice(0, -11).every((drop) => {
                const reward = quest.rewards.find((r) => r.name === drop);
                return reward !== undefined && Core.checkInventory(drop, reward.maxStack);
            })
        )
            return;

        Core.logger("if Swindles is enabled, it will only accept the quest when it has the required Unis it needs");

        // Resolve settings
        this.sellMemVoucher = Core.cboBool("Nation_SellMemVoucher") === true;
        ultraAlteon = ultraAlteon || Core.cboBool("UltraAlteonForSupplies") === true;
        returnPolicyDuringSupplies = returnPolicyDuringSupplies || Core.cboBool("Nation_ReturnPolicyDuringSupplies") === true;

        if (keepVoucher) this.sellMemVoucher = false;
        else if (this.sellMemVoucher && bot.player.gold >= 100000000) this.sellMemVoucher = false;

        this.logSuppliesConfig(item, quant, ultraAlteon, hydraChallenge, keepVoucher, this.sellMemVoucher, assistantDuring, returnPolicyDuringSupplies, returnItem);

        // Quest registration
        const quests = [2857];
        if (item !== this.uni(13) && Core.checkInventory(38261)) quests.push(9542);
        if (Core.checkInventory("Drudgen the Assistant")) quests.push(870);
        Core.registerQuests(...quests);

        // Drop registration
        const drops: string[] = [];
        if (item !== null) drops.push(item);
        if (returnItem !== null) drops.push(returnItem);
        drops.push(...Core.questRewards(9542));
        drops.push(...this.suppliesRewards);
        if (this.sellMemVoucher) drops.push("Voucher of Nulgath");
        drops.push("Relic of Chaos");
        if (returnPolicyDuringSupplies) drops.push(this.uni(1), this.uni(6), this.uni(9), this.uni(16), this.uni(20), "Receipt of Swindle");

        Core.addDrop(...drops);
        Core.equipClass(ClassType.Solo);

        if (item === null || item === "All") this.farmAllSupplies(ultraAlteon, hydraChallenge, keepVoucher, assistantDuring, returnItem, returnPolicyDuringSupplies);
        else
            this.farmSingleSupply(item, quant, ultraAlteon, hydraChallenge, keepVoucher, assistantDuring, returnItem, returnPolicyDuringSupplies, voucherItemDuring);

        Core.cancelRegisteredQuests();
    }

    private farmAllSupplies(
        ultraAlteon: boolean,
        hydraChallenge: boolean,
        keepVoucher: boolean,
        assistantDuring: boolean,
        returnItem: string | null,
        returnPolicyDuringSupplies: boolean,
    ): void {
        for (const thing of this.suppliesRewards) {
            const rewardItem = Core.ensureLoad(2857).rewards.find((x) => x.name === thing);
            if (!rewardItem) continue;

            if (!ultraAlteon && !hydraChallenge && (Core.checkInventory(this.cragName) || this.hasOBoNPet)) {
                this.bambloozevsDrudgen(rewardItem.name, rewardItem.maxStack, { keepVoucher, assistantDuring, returnItem, camefromSupplies: true });
                continue;
            }

            while (!bot.shouldExit && !Core.checkInventory(rewardItem.id, rewardItem.maxStack)) {
                if (hydraChallenge) this.killHydra();
                else if (ultraAlteon) Core.killMonster("ultraalteon", "r10", "Left", "Ultra Chaos Alteon", "Relic of Chaos", 1, { isTemp: false, log: false });
                else Core.killEscherion("Relic of Chaos", 1, { log: false, fromSupplies: true });

                this.sellVoucherOfNulgath(this.sellMemVoucher, null);
                this.assistantDuringSupplies(assistantDuring);
                this.doSwindlesReturnArea(returnPolicyDuringSupplies, returnItem);
                this.handleVoucherConversions(rewardItem.name);
            }
        }
    }

    private killHydra(): void {
        if ((bot.map.name ?? "").toLowerCase() !== "hydrachallenge") Core.join("hydrachallenge");

        const relics = bot.inventory.getQuantity("Relic of Chaos");

        while (!bot.shouldExit && bot.inventory.getQuantity("Relic of Chaos") === relics) {
            if (!bot.player.alive) {
                Core.waitForTrue(() => bot.player.alive, 20);
                continue;
            }

            if ((bot.player.cell ?? "").toLowerCase() !== "h90") {
                Core.jump("h90", "Left");
                bot.wait.forCellChange("h90");
            }

            Core.canWeAggro();

            if (!bot.player.hasTarget) bot.combat.attack("*");

            Core.pause(500);
        }
    }

    private farmSingleSupply(
        item: string,
        quant: number,
        ultraAlteon: boolean,
        hydraChallenge: boolean,
        keepVoucher: boolean,
        assistantDuring: boolean,
        returnItem: string | null,
        returnPolicyDuringSupplies: boolean,
        voucherItemDuring: boolean,
    ): void {
        if (!ultraAlteon && !hydraChallenge && (Core.checkInventory(this.cragName) || this.hasOBoNPet)) {
            this.bambloozevsDrudgen(item, quant, { keepVoucher, assistantDuring, returnItem, camefromSupplies: true });
            return;
        }

        while (!bot.shouldExit && !Core.checkInventory(item, quant)) {
            if (hydraChallenge) this.killHydra();
            else if (ultraAlteon) Core.killMonster("ultraalteon", "r10", "Left", "*", null, 1, { isTemp: false, log: false });
            else Core.killEscherion(null, 1, { log: false, fromSupplies: true });

            this.sellVoucherOfNulgath(this.sellMemVoucher, item);
            this.assistantDuringSupplies(assistantDuring);
            this.doSwindlesReturnArea(returnPolicyDuringSupplies, returnItem);

            if (voucherItemDuring) this.handleVoucherConversions(item);
        }
    }

    /**
     * Turns vouchers into Totems/Gems (quest 4778) when you have a non-mem voucher and 60 essences.
     * C# checks the Diamonds' stack before picking the Gem of Nulgath (6136): quest 4778 gives no
     * Diamonds, so it kept spending vouchers on Gems that were already maxed.
     */
    private handleVoucherConversions(item: string): void {
        if (!Core.checkInventory("Voucher of Nulgath (non-mem)") || !Core.checkInventory("Essence of Nulgath", 60)) return;

        const diamondsMaxed = Core.checkInventory("Diamond of Nulgath", 1000);
        const gemsMaxed = Core.checkInventory("Gem of Nulgath", 1000);
        const totemsMaxed = Core.checkInventory("Totem of Nulgath", 100);

        if (item.toLowerCase() === "diamond of nulgath") {
            if (diamondsMaxed && !totemsMaxed) Core.ensureCompleteMulti(4778, 100, 5357);
        } else if (item.toLowerCase() === "totem of nulgath") {
            if (totemsMaxed && !gemsMaxed) Core.ensureCompleteMulti(4778, 1000, 6136);
        } else {
            if (!totemsMaxed) Core.ensureCompleteMulti(4778, 100, 5357);
            else if (!gemsMaxed) Core.ensureCompleteMulti(4778, 1000, 6136);
        }
    }

    private logSuppliesConfig(
        item: string | null,
        quant: number,
        ultraAlteon: boolean,
        hydraChallenge: boolean,
        keepVoucher: boolean,
        sellMemVoucher: boolean,
        assistantDuring: boolean,
        returnPolicyDuringSupplies: boolean,
        returnItem: string | null,
    ): void {
        const flag = (v: boolean) => (v ? "True" : "False");
        Core.logger("[Supplies] =========================", "Supplies");
        Core.logger(`Item      : ${item ?? "All"} x${quant}`, "Supplies");
        Core.logger(`Ultra     : ${flag(ultraAlteon)}`, "Supplies");
        Core.logger(`Hydra     : ${flag(hydraChallenge)}`, "Supplies");
        Core.logger(`Voucher K : ${flag(keepVoucher)}`, "Supplies");
        Core.logger(`Voucher S : ${flag(sellMemVoucher)}`, "Supplies");
        Core.logger(`Assist    : ${flag(assistantDuring)}`, "Supplies");
        Core.logger(`Return    : ${flag(returnPolicyDuringSupplies)}`, "Supplies");
        Core.logger(`Return It : ${returnItem ?? "None"}`, "Supplies");
        Core.logger("[Supplies] =========================", "Supplies");
    }

    /** Sells all "Voucher of Nulgath" unless that's the item being farmed. */
    sellVoucherOfNulgath(sellMemVoucher = true, item: string | null = null): void {
        if (!sellMemVoucher || (sellMemVoucher && item === "Voucher of Nulgath")) return;

        if (Core.checkInventory("Voucher of Nulgath")) {
            Core.jump("Enter", "Spawn");
            Core.sellItem("Voucher of Nulgath", 1, { all: true });
        }
    }

    /** Spends gold (100k each, max 250) on War-Torn Memorabilia for quest 2859. */
    assistantDuringSupplies(assistDuring = true): void {
        if (!assistDuring) return;

        if (bot.player.gold >= 100_000) {
            Core.jump("Enter", "Spawn");
            const quantityToBuy = Math.trunc(Math.min(bot.player.gold / 100_000, 250));
            Core.ensureAccept(2859);
            Core.buyItem("yulgar", 41, "War-Torn Memorabilia", quantityToBuy);
            Core.ensureCompleteMulti(2859, quantityToBuy);
        }
    }

    /**
     * Completes "Swindle's Return Policy" (quest 7551), prioritizing a specific reward or the
     * first non-maxed one.
     */
    private doSwindlesReturnArea(returnPolicyActive: boolean, item: string | null = null): void {
        if (!returnPolicyActive) return;

        if (!Core.checkInventory([this.uni(1), this.uni(6), this.uni(9), this.uni(16), this.uni(20)])) return;

        const quest = Core.initializeWithRetries(() => Core.loadQuest(7551));
        if (!quest?.rewards) {
            Core.debugLogger("Failed to load quest 7551 - quest or rewards are null");
            return;
        }

        // Early exit: check if preferred item is already maxed
        if (item !== null) {
            const preferred = quest.rewards.find((r) => r.name === item);
            if (!preferred) {
                Core.debugLogger(`Preferred item '${item}' not found in quest rewards`);
                return;
            }
            if (Core.checkInventory(preferred.id, preferred.maxStack)) {
                Core.debugLogger(
                    `Preferred item '${item}' is already maxed (${bot.inventory.getQuantity(preferred.name)}/${preferred.maxStack}) - skipping quest`,
                );
                return;
            }
        }
        // Early exit: check if all rewards are already maxed
        else if (quest.rewards.every((r) => Core.checkInventory(r.id, r.maxStack))) {
            Core.debugLogger("All quest rewards are already maxed - skipping quest");
            return;
        }

        Core.ensureAccept(7551);
        Core.resetQuest(7551);
        Core.darkMakaiItem("Dark Makai Rune");

        const reward: RewardItem | undefined =
            item !== null
                ? quest.rewards.find((r) => r.id !== 57446 && r.name === item)
                : quest.rewards.find((r) => r.id !== 57446 && !Core.checkInventory(r.id, r.maxStack));

        if (!bot.quests.canCompleteFullCheck(7551)) {
            Core.debugLogger("Quest 7551 cannot be completed - missing requirements");
            return;
        }

        if (reward) {
            Core.debugLogger(`Completing quest with specific reward: ${reward.name} (ID: ${reward.id})`);
            Core.ensureComplete(7551, reward.id);
            bot.wait.forQuestComplete(7551);
            bot.wait.forPickup(reward.id);
            return;
        }

        // Complete without a selected reward; Receipt of Swindle is guaranteed.
        Core.ensureComplete(7551, -1);
    }

    /**
     * "The Assistant" (quest 2859): spends gold on War-Torn Memorabilia for an item, or (`item`
     * null) maxes every main bag drop. Converts vouchers (quest 4778) on the way.
     * @param farmGold Farm 1M gold before each turn-in.
     * @param reward Swindles Return Policy reward to pick when it runs during the farm.
     */
    theAssistant(item: string | null = null, quant = 1000, farmGold = true, reward: SwindlesReturnReward = SwindlesReturnReward.None): void {
        if (item !== null && Core.checkInventory(item, quant)) return;

        // C# retries each load forever (goto Retry...); Core.ensureLoad adds the rewards' max stacks.
        while (!Core.initializeWithRetries(() => Core.ensureLoad(7551))) {
            Core.logger("Failed to load quest 7551, retrying...");
            Core.sleep();
        }
        let assistant: QuestData | null;
        while (!(assistant = Core.initializeWithRetries(() => Core.ensureLoad(2859)))) {
            Core.logger("Failed to load quest 2859, retrying...");
            Core.sleep();
        }

        // List of available drops for "The Assistant" quest
        const selectedDrops = item !== null ? [item] : this.bagDrops.slice(0, -11);
        Core.addDrop(...selectedDrops);

        // add `Receipt of Swindle` from swindles return rewards.
        Core.addDrop("Receipt of Swindle");

        // if running standalone, add the reward selection.
        if (reward !== SwindlesReturnReward.None) Core.addDrop(reward);

        // handle quant if it goes over max stack.
        if (item !== null && quant > 0) {
            const assistantReward = assistant.rewards.find((x) => x.name === item);
            if (assistantReward) quant = quant > assistantReward.maxStack ? assistantReward.maxStack : quant;
            else Core.logger(`Reward item "${item}" not found.`);
        }

        // Check if return policy is active
        this.returnPolicyDuringSupplies = Core.cboBool("Nation_ReturnPolicyDuringSupplies") === true;
        Core.logger(this.returnPolicyDuringSupplies ? "Return Policy During Supplies: true" : "Return Policy During Supplies: false");

        if (this.returnPolicyDuringSupplies) {
            Core.addDrop(this.uni(1), this.uni(6), this.uni(9), this.uni(16), this.uni(20));
            Core.addDrop("Blood Gem of the Archfiend");
        }

        // C# passes the enum's name without "_" ("TaintedGem", "None"), which matches no reward, so
        // its return policy never runs here. Turu passes the reward's name, or null (any) for None.
        const rewardName = reward === SwindlesReturnReward.None ? null : SwindlesReturnReward[reward].replace(/_/g, " ");

        // Register the "Swindles Return Policy" quest if specified
        if (this.returnPolicyDuringSupplies && reward !== SwindlesReturnReward.None) this.doSwindlesReturnArea(this.returnPolicyDuringSupplies, rewardName);

        const voucherItem = (): void => {
            if (!Core.checkInventory("Voucher of Nulgath (non-mem)") || !Core.checkInventory("Essence of Nulgath", 60)) return;
            const diamondsMaxed = Core.checkInventory("Diamond of Nulgath", 1000);
            // C# checks the Diamonds here; 6136 is Gem of Nulgath (see handleVoucherConversions).
            const gemsMaxed = Core.checkInventory("Gem of Nulgath", 1000);
            const totemsMaxed = Core.checkInventory("Totem of Nulgath", 100);

            if (item?.toLowerCase() === "diamond of nulgath") {
                if (diamondsMaxed && !totemsMaxed) Core.ensureCompleteMulti(4778, -1, 5357); // Totems
            } else if (item?.toLowerCase() === "totem of nulgath") {
                if (totemsMaxed && !gemsMaxed) Core.ensureCompleteMulti(4778, -1, 6136); // Gems
            } else if (!totemsMaxed) Core.ensureCompleteMulti(4778, -1, 5357); // Prioritize Totems
            else if (!gemsMaxed) Core.ensureCompleteMulti(4778, -1, 6136); // Then Gems
        };

        if (item === null) {
            Core.logger("Assistant Item = null, maxing the important rewards.");
            for (const thing of selectedDrops) {
                // Find the corresponding item in quest rewards
                const rewardItem = Core.ensureLoad(2859).rewards.find((x) => x.name === thing);
                if (!rewardItem) continue;

                Core.farmingLogger(rewardItem.name, rewardItem.maxStack);
                // Continue farming until the desired item quantity is obtained
                while (!bot.shouldExit && !Core.checkInventory(rewardItem.name, rewardItem.maxStack)) {
                    this.logMobItemQuant2(rewardItem.name);
                    if (farmGold) CoreNation.farm.gold(1000000);

                    Core.ensureAccept(2859);
                    Core.buyItem("yulgar", 41, "War-Torn Memorabilia", 10);
                    Core.ensureCompleteMulti(2859);

                    this.doSwindlesReturnArea(this.returnPolicyDuringSupplies, rewardName);
                    voucherItem();
                }
            }
            Core.cancelRegisteredQuests();
        } else {
            Core.farmingLogger(item, quant);
            while (!bot.shouldExit && !Core.checkInventory(item, quant)) {
                this.logMobItemQuant(item, quant);
                if (farmGold) CoreNation.farm.gold(1000000);

                Core.ensureAccept(2859);
                Core.buyItem("yulgar", 41, "War-Torn Memorabilia", 10);
                bot.wait.forItemBuy(40);
                Core.ensureCompleteMulti(2859);

                this.doSwindlesReturnArea(this.returnPolicyDuringSupplies, rewardName);
                voucherItem();
            }
        }
        if (this.returnPolicyDuringSupplies && reward !== SwindlesReturnReward.None) bot.quests.unregisterQuests(7551);
    }

    /** Logs the item's quantity when it changed within 1.5 s. */
    private logMobItemQuant(item: string, quant: number): void {
        if (!Core.checkInventory(item)) return;

        const startQuant = bot.inventory.getQuantity(item);
        Core.sleep(1500);
        const currentQuant = bot.inventory.getQuantity(item);

        if (currentQuant !== startQuant || currentQuant > startQuant) {
            Core.farmingLogger(item, quant);
            Core.sleep(1500);
        }
    }

    /**
     * C# `LogMobItemQuant2`: compares the reward object's own quantity, which never changes, so it
     * only waits 1.5 s when you have the item.
     */
    private logMobItemQuant2(item: string): void {
        if (!Core.checkInventory(item)) return;
        Core.sleep(1500);
    }

    /** Member with Nulgath's Birthday Gift (4809) or Bounty Hunter's Drone Pet (5373). */
    get hasOBoNPet(): boolean {
        return bot.player.isMember && Core.checkInventory([4809, 5373], 1, { any: true });
    }

    /**
     * Performs the "Bamblooze vs. Drudgen" farm (Tainted Elementals in /evilmarsh with Supplies,
     * Bamboozle and the Dark Deal registered) for the desired item.
     * @param item Desired item name (null: C# treats it as owned and returns).
     * @param quant Desired item quantity.
     */
    bambloozevsDrudgen(item: string | null = null, quant = 1, options: BambloozeOptions = {}): void {
        const { keepVoucher = false, assistantDuring = false, returnItem = null, camefromSupplies = false } = options;
        if (Core.checkInventory(item, quant)) {
            Core.logger(`${item ?? ""} x${quant} already owned.`);
            return;
        }

        Core.addDrop("Relic of Chaos", "Tainted Core");
        Core.addDrop(...(!item ? this.bagDrops : [item]));

        const returnPolicyDuringSupplies = Core.cboBool("Nation_ReturnPolicyDuringSupplies") === true;
        // C# ignores keepVoucher here (supplies honours it), so farmVoucher({ keepVoucher }) sold vouchers.
        const sellMemVoucher = !keepVoucher && Core.cboBool("Nation_SellMemVoucher") === true;

        if (!camefromSupplies)
            Core.logger(`Bamblooze mode → Item: ${item ?? "All"} | Keep:${keepVoucher ? "True" : "False"} Assist:${assistantDuring ? "True" : "False"}`);

        this.sellVoucherOfNulgath(sellMemVoucher, item);

        if (returnPolicyDuringSupplies) Core.addDrop(this.uni(1), this.uni(6), this.uni(9), this.uni(16), this.uni(20));

        Core.initializeWithRetries(() => Core.ensureLoad(2857).rewards);

        if (item) Core.farmingLogger(item, quant);

        // Choose the appropriate quest based on pet availability
        // 2857 - Supplies to Spin The Wheel of Chance
        const questToRegister = [2857];

        // 609 - Bamboozle vs Drudgen
        if (Core.checkInventory(this.cragName)) questToRegister.push(609);

        if (this.hasOBoNPet) {
            Core.addDrop("Tainted Soul");
            // The Dark Deal | The Dark Deal (rare)
            questToRegister.push(Core.checkInventory(4809) ? 599 : 2561);
        }

        // 9542 - Swindle's Bonus Deal - Swindle Bilk's To Go Hut
        if (Core.checkInventory(38261)) questToRegister.push(9542);

        // Register unique quests only
        Core.registerQuests(...new Set(questToRegister));
        Core.equipClass(ClassType.Solo);
        while (!bot.shouldExit && !Core.checkInventory(item, quant)) {
            Core.killMonster("evilmarsh", "End", "Left", "Tainted Elemental", null, 1, { log: false });

            // Sell Voucher of Nulgath if allowed
            this.sellVoucherOfNulgath(sellMemVoucher, item);

            Core.pause(500);
            // Spend gold if AssistantDuring
            this.assistantDuringSupplies(assistantDuring);

            Core.pause(500);
            // 7551 - Swindle's Return Policy
            if (returnItem !== null) this.doSwindlesReturnArea(returnPolicyDuringSupplies, returnItem);

            Core.pause(500);
            // C# `Retry:` label: reload quest 7551 until it loads
            for (;;) {
                const quest = Core.initializeWithRetries(() => Core.loadQuest(7551));
                if (!quest) {
                    Core.logger("Failed to load quest 7551.");
                    Core.sleep();
                    continue;
                }

                Core.pause(500);
                if (returnPolicyDuringSupplies && (item === "Diamond of Nulgath" || item === null) && !Core.checkInventory("Diamond of Nulgath", 1000))
                    this.cragsThirst();

                Core.pause(500);
                if (Core.checkInventory("Voucher of Nulgath (non-mem)") && Core.checkInventory("Essence of Nulgath", 60)) {
                    const diamondsMaxed = Core.checkInventory("Diamond of Nulgath", 1000);
                    // C# checks the Diamonds here; 6136 is Gem of Nulgath (see handleVoucherConversions).
                    const gemsMaxed = Core.checkInventory("Gem of Nulgath", 1000);
                    const totemsMaxed = Core.checkInventory("Totem of Nulgath", 100);

                    if (item?.toLowerCase() === "diamond of nulgath") {
                        if (diamondsMaxed && !totemsMaxed) Core.ensureCompleteMulti(4778, -1, 5357); // Totems
                    } else if (item?.toLowerCase() === "totem of nulgath") {
                        if (totemsMaxed && !gemsMaxed) Core.ensureCompleteMulti(4778, -1, 6136); // Gems
                    } else {
                        // item == null or any other item
                        if (!totemsMaxed) Core.ensureCompleteMulti(4778, -1, 5357); // Prioritize Totems
                        else if (!gemsMaxed) Core.ensureCompleteMulti(4778, -1, 6136); // Then Gems
                    }
                }
                break;
            }
            Core.pause(500);
        }
    }

    /**
     * Completes the Void Knight Sword quest (5662 with item 38275, else 5659) for the item.
     * @param item The item to obtain (null: max every reward).
     */
    voidKnightSwordQuest(item: string | null = null, quant = 1): void {
        if ((item !== null && Core.checkInventory(item, quant)) || !Core.checkInventory([38275, 38254], 1, { any: true })) return;

        Core.addDrop(...this.bagDrops);
        if (item !== null) Core.addDrop(item);

        const turnInRound = (questId: number): void => {
            Core.equipClass(ClassType.Solo);
            Core.joinSWF("mobius", "ChiralValley/town-Mobius-21Feb14.swf", "Slugfit", "Bottom");
            Core.huntMonster("mobius", "Slugfit", "Slugfit Horn", 5);
            Core.huntMonster("faerie", "Aracara", "Aracara Silk");

            // Equip the Farm class and hunt monsters for quest completion
            Core.equipClass(ClassType.Farm);
            Core.killMonster("tercessuinotlim", "m2", "Left", "*", "Makai Fang", 5);
            Core.huntMonster("hydra", "Fire Imp", "Imp Flame", 3, { log: false });
            Core.huntMonster("battleunderc", "Crystalized Jellyfish", "Aquamarine of Nulgath", 3, { isTemp: false });
            Core.ensureComplete(questId);
        };

        if (item === null) {
            const questId = Core.checkInventory(38275) ? 5662 : 5659;
            Core.addDrop(...Core.questRewards(questId));
            const quest = Core.loadQuest(questId);
            if (!quest) {
                Core.logger(`Failed to load quest ${questId} for VoidKnightSwordQuest.`);
                return;
            }

            for (const reward of quest.rewards) {
                if (Core.checkInventory(reward.id, reward.maxStack)) continue;

                while (!bot.shouldExit && !Core.checkInventory(reward.id, reward.maxStack)) {
                    Core.ensureAccept(questId);
                    turnInRound(questId);
                }
            }
        } else {
            Core.farmingLogger(item, quant);

            while (!bot.shouldExit && !Core.checkInventory(item, quant)) {
                Core.ensureAccept(Core.checkInventory(38275) ? 5662 : 5659);
                turnInRound(Core.checkInventory(38275) ? 5662 : 5659);
            }
        }
    }

    /**
     * Do Diamond Exchange quest (869) until 13 Unidentified 13; with `farmDiamond`, farms 15
     * Diamonds first if needed.
     */
    diamondExchange(farmDiamond = true): void {
        if ((!Core.checkInventory("Diamond of Nulgath", 15) && !farmDiamond) || !Core.checkInventory(this.cragName) || Core.checkInventory(this.uni(13), 13))
            return;

        Core.addDrop("Diamond of Nulgath");

        if (farmDiamond) if (this.hasOBoNPet || Core.checkInventory(this.cragName)) this.bambloozevsDrudgen("Diamond of Nulgath", 15);

        Core.equipClass(ClassType.Farm);
        while (!bot.shouldExit && Core.checkInventory("Diamond of Nulgath", 15) && !Core.checkInventory(this.uni(13), 13)) {
            Core.resetQuest(869);
            Core.darkMakaiItem("Dark Makai Sigil");
            Core.ensureCompleteMulti(869);
        }
        bot.options.attackWithoutTarget = false;
    }

    /**
     * Do Contract Exchange quest (870) until `quant` of the reward; with `farmUni13`, farms
     * Unidentified 13 first if needed.
     */
    contractExchange(rewardEnum: ContractExchangeRewards, quant: number, farmUni13 = true): void {
        const reward = ContractExchangeRewards[rewardEnum].replace(/_/g, " ");
        if ((!Core.checkInventory(this.uni(13)) && !farmUni13) || !Core.checkInventory("Drudgen the Assistant")) {
            if (!Core.checkInventory(this.uni(13)) && !farmUni13) Core.logger(`${farmUni13 ? "True" : "False"} is probably set to false, please have a dev change it`);
            if (!Core.checkInventory("Drudgen the Assistant")) Core.logger('Missing "Drudgen the Assistant"');
            return;
        }

        Core.addDrop(...this.bagDrops);
        Core.equipClass(ClassType.Solo);
        Core.logger(`Reward Chosen: ${reward} [${rewardEnum}]`);
        Core.farmingLogger(reward, quant);
        while (!bot.shouldExit && !Core.checkInventory(reward, quant)) {
            if (farmUni13 && !Core.checkInventory(this.uni(13))) this.farmUni13(3);
            Core.resetQuest(870);
            Core.killMonster("tercessuinotlim", "m4", "Top", "Shadow of Nulgath", null, 1, { log: false });
            Core.ensureComplete(870, rewardEnum);
        }
    }

    private static readonly sroeValidSet = new Set(["Blood Gem of the Archfiend", "Dark Crystal Shard", "Gem of Nulgath", "Tainted Gem", "Totem of Nulgath"]);

    /**
     * Does Swindle's Dirt-y Deeds Done Dirt Cheap quest (7818) for Unidentified 10. Only use if
     * /TowerofDoom10 completed and a good solo class.
     * @param sroe Spend maxed Unidentified 10s at the SRoE shop (1951) on `sroeItems` ("All" = every valid item).
     */
    dirtyDeedsDoneDirtCheap(quant = 1000, sroe = false, sroeItems: string[] | null = null): void {
        Core.addDrop(
            "Emerald Pickaxe",
            "Seraphic Grave Digger Spade",
            "Unidentified 10",
            "Receipt of Swindle",
            "Blood Gem of the Archfiend",
            "Dark Crystal Shard",
            "Gem of Nulgath",
            "Tainted Gem",
            "Totem of Nulgath",
        );

        Core.equipClass(ClassType.Solo);
        Core.killEscherion("Emerald Pickaxe");
        Core.killMonster("legioncrypt", "r1", "Top", "Gravedigger", "Seraphic Grave Digger Spade", 1, { isTemp: false });

        let itemsToBuy: CoreShopItem[] | null = null;
        if (sroe) {
            const shopItems = Core.getShopItems("tercessuinotlim", 1951);
            const buyAll = sroeItems?.length === 1 && sroeItems[0].toLowerCase() === "all";
            const selectedSet = !buyAll && sroeItems && sroeItems.length > 0 ? new Set(sroeItems) : null;

            const build = shopItems.filter(
                (item) => item && CoreNation.sroeValidSet.has(item.name) && (buyAll || (selectedSet !== null && selectedSet.has(item.name))),
            );
            if (build.length > 0) itemsToBuy = build;
        }

        while (!bot.shouldExit && !Core.checkInventory("Unidentified 10", quant)) {
            Core.ensureAccept(7818);

            Core.huntMonster("towerofdoom10", "Slugbutter", "Slugbutter Digging Advice", 1, { publicRoom: true, log: false });
            Core.huntMonster("crownsreach", "Chaos Tunneler", "Chaotic Tunneling Techniques", 2, { log: false });
            Core.huntMonster("downward", "Crystal Mana Construct", "Crystalized Corporate Digging Secrets", 3, { log: false });

            Core.ensureComplete(7818);
            Core.farmingLogger("Unidentified 10", quant);

            if (!sroe || !itemsToBuy || itemsToBuy.length === 0) continue;
            if (bot.inventory.getQuantity("Unidentified 10") < 1000) continue;

            let allMaxed = true;
            for (const item of itemsToBuy) {
                let buyQty = Core.maxBuyQuant("tercessuinotlim", 1951, item);
                const currentQty = bot.inventory.getQuantity(item.id);

                // Cap by max stack
                buyQty = Math.min(buyQty, item.maxStack - currentQty);
                if (buyQty <= 0) continue; // Already maxed

                allMaxed = false;
                Core.logger(`Buying ${buyQty}x ${item.name}`);
                Core.buyItem("tercessuinotlim", 1951, item.id, buyQty, item.shopItemId);
            }

            if (allMaxed) {
                Core.logger("All selected SRoE items maxed. Disabling SRoE.");
                sroe = false;
            }
        }
    }

    /**
     * Farms Unidentified 13 with the best method available.
     * @param quant Desired quantity, 13 = max stack.
     */
    farmUni13(quant = 13): void {
        if (Core.checkInventory(this.uni(13), quant)) return;

        Core.addDrop(this.uni(13));
        quant = quant > 13 ? 13 : quant;

        if (Core.checkInventory(this.cragName)) while (!bot.shouldExit && !Core.checkInventory(this.uni(13), quant)) this.diamondExchange();
        this.newWorldsNewOpportunities(this.uni(13), quant); // 1minute turning = 1x guaranteed
        this.voidKnightSwordQuest(this.uni(13), quant);
        this.supplies(this.uni(13), quant);
    }

    /**
     * Farms Unidentified 10 with the best method available.
     * @param quant Desired quantity, 1000 = max stack.
     */
    farmUni10(quant = 1000): void {
        if (Core.checkInventory("Unidentified 10", quant)) return;

        Core.addDrop("Unidentified 10");
        if (this.hasOBoNPet || Core.checkInventory(this.cragName)) this.bambloozevsDrudgen("Unidentified 10", quant);
        this.dirtyDeedsDoneDirtCheap(quant);
    }

    /**
     * Farms Dark Crystal Shard with the best method available.
     * @param quant Desired quantity, 1000 = max stack.
     */
    farmDarkCrystalShard(quant = 1000): void {
        if (Core.checkInventory("Dark Crystal Shard", quant)) return;

        Core.addDrop("Dark Crystal Shard");
        this.farmContractExchage("Dark Crystal Shard", quant);
        this.newWorldsNewOpportunities("Dark Crystal Shard", quant); // 1minute turning = 1x guaranteed
        if (Core.checkInventory(this.cragName)) this.supplies("Dark Crystal Shard", quant, { returnItem: "Dark Crystal Shard" }); // 10% chance
        this.voidKnightSwordQuest("Dark Crystal Shard", quant);
        this.supplies("Dark Crystal Shard", quant, { returnItem: "Dark Crystal Shard" }); // 10% chance
        this.essenceofDefeatReagent(quant);
    }

    /**
     * Farms Diamond of Nulgath with the best method available.
     * @param quant Desired quantity, 1000 = max stack.
     */
    farmDiamondofNulgath(quant = 1000): void {
        if (Core.checkInventory("Diamond of Nulgath", quant)) return;

        Core.addDrop("Diamond of Nulgath");

        // This Quest is more of an additive Bonus whislt doing supplies
        while (
            !bot.shouldExit &&
            !Core.checkInventory("Diamond of Nulgath", quant) &&
            Core.checkInventory(this.cragName) &&
            Core.checkInventory(this.uni(10), 100)
        )
            this.cragsThirst(quant);

        if (Core.checkInventory(this.cragName)) this.supplies("Diamond of Nulgath", quant, { returnItem: "Diamond of Nulgath" });

        this.voidKnightSwordQuest("Diamond of Nulgath", quant);
        this.supplies("Diamond of Nulgath", quant, { returnItem: "Diamond of Nulgath" });
    }

    /**
     * Farms Gem of Nulgath with the best method available.
     * @param quant Desired quantity, 300 = max stack.
     */
    farmGemofNulgath(quant = 1000): void {
        if (Core.checkInventory("Gem of Nulgath", quant)) return;

        Core.addDrop("Gem of Nulgath");
        this.farmContractExchage("Gem of Nulgath", quant);
        if (Core.checkInventory(this.cragName)) this.supplies("Gem of Nulgath", quant, { returnItem: "Gem of Nulgath" });
        this.voidKnightSwordQuest("Gem of Nulgath", quant);
        this.supplies("Gem of Nulgath", quant, { returnItem: "Gem of Nulgath" });
    }

    /**
     * Farms Blood Gem of the Archfiend with the best method available.
     * @param quant Desired quantity, 100 = max stack.
     */
    farmBloodGem(quant = 100, hydraLevel = 85): void {
        if (Core.checkInventory("Blood Gem of the Archfiend", quant)) return;

        Core.addDrop("Blood Gem of the Archfiend");

        this.farmContractExchage("Blood Gem of the Archfiend", quant);
        this.newWorldsNewOpportunities("Blood Gem of the Archfiend", quant);
        this.voidKnightSwordQuest("Blood Gem of the Archfiend", quant);
        this.bloodyChaos(quant, true, hydraLevel);
        this.kisstheVoid(quant);
    }

    /** Farms Tainted Gem with the best method available. */
    farmTaintedGem(quant = 1000): void {
        if (Core.checkInventory("Tainted Gem", quant)) return;

        Core.addDrop("Tainted Gem");
        this.farmContractExchage("Tainted Gem", quant);
        if (Core.checkInventory(this.cragName)) this.supplies("Tainted Gem", quant, { returnItem: "Tainted Gem" });
        this.forgeTaintedGems(quant);
        this.supplies("Tainted Gem", quant, { returnItem: "Tainted Gem" });
    }

    /** Completes the lair questline (165-169) to unlock Nation mats if not completed. */
    dragonSlayerReward(): void {
        if (Core.isCompletedBefore(169)) return;

        const questIds = [165, 166, 167, 168, 169];
        const questMonsterNames = ["Water Draconian", "Bronze Draconian", "Golden Draconian", "Red Dragon", "Water Draconian"];
        const questClasses = [ClassType.Farm, ClassType.Farm, ClassType.Farm, ClassType.Solo, ClassType.Farm];

        for (let i = 0; i < questIds.length; i++) {
            // Check if the quest is already completed
            if (Core.isCompletedBefore(questIds[i])) continue;

            // Equip the required class for the quest
            Core.equipClass(questClasses[i]);
            Core.huntMonsterQuest(questIds[i], "lair", questMonsterNames[i]);
        }
    }

    /**
     * Farms the "Totem of Nulgath" with the best method available: Crag & Bamboozle (Deal), then
     * the NWNO pets, then Taro's Manslayer and Essences.
     * @param quant Desired quantity (default 100, max stack).
     */
    farmTotemofNulgath(quant = 100): void {
        if (Core.checkInventory("Totem of Nulgath", quant)) return;

        // <CragName> Owned
        this.deal(0, quant);

        // Nulgath's Birthday Gift / Bounty Hunter's Drone Pet Owned
        this.newWorldsNewOpportunities("Totem of Nulgath", quant);

        // No Pets Owned
        this.totemsViaTaros(quant);
    }

    /**
     * Farms Totems of Nulgath through Taro's quest (726): 25 Essence of Nulgath and Taro's
     * Manslayer per turn-in (members get the Manslayer from quest 1111, after Good rank and the
     * Purified Claymore of Destiny).
     */
    totemsViaTaros(quant = 100): void {
        if (Core.checkInventory("Totem of Nulgath", quant)) return;

        if (Core.isMember && !Core.checkInventory("Purified Claymore of Destiny")) {
            Core.logger(`Player is a member, we'll use the better farming method for the "Taro's Manslayer", first we need to get some prereqs.`);
            CoreNation.farm.goodREP();
            this.getPCoD();
        }

        Core.addDrop("Totem of Nulgath");

        if (!bot.quests.hasBeenCompleted(9541)) Core.chainComplete(9541);

        while (!bot.shouldExit && !Core.checkInventory("Totem of Nulgath", quant)) {
            Core.ensureAccept(726);
            this.essenceofNulgath(25);
            if (Core.isMember && !Core.checkInventory(538 /* Taro's Manslayer */)) {
                Core.ensureAccept(1111);
                this.farmGemofNulgath(10);
                Core.resetQuest(7551);
                Core.darkMakaiItem("Dark Makai Rune");
                Core.ensureComplete(1111, 538 /* Taro's Manslayer */);
                bot.wait.forQuestComplete(1111);
                bot.wait.forPickup(538);
            } else {
                Core.equipClass(ClassType.Solo);
                Core.huntMonster("tercessuinotlim", "Taro Blademaster", "Taro's Manslayer", 1, { isTemp: false });
            }
            Core.ensureComplete(726);
            bot.wait.forQuestComplete(726);
            bot.wait.forPickup("Totem of Nulgath");
        }
        Core.cancelRegisteredQuests();
    }

    /**
     * Farms Gems and/or Totems of Nulgath via the Crag & Bamboozle method (quest 4777).
     * @param gemQuant Number of Gems of Nulgath to farm (0 = skip).
     * @param totemQuant Number of Totems of Nulgath to farm (0 = skip).
     */
    deal(gemQuant = 0, totemQuant = 0): void {
        if (!Core.checkInventory(this.cragName)) {
            Core.logger(`Missing ${this.cragName} cannot continue`);
            return;
        }

        this.dragonSlayerReward(); // required
        this.getPCoD();
        Core.addDrop("Gem of Nulgath", "Totem of Nulgath");

        const farmItem = (itemName: string, quant: number, rewardId: number): void => {
            if (quant <= 0) return;

            Core.farmingLogger(itemName, quant);
            while (!bot.shouldExit && !Core.checkInventory(itemName, quant)) {
                Core.ensureAccept(4777);
                this.supplies("Unidentified 3", 1, { returnItem: "Blood Gem of the Archfiend" });
                this.farmBloodGem(2);
                this.farmUni10(30);
                Core.ensureComplete(4777, rewardId);
                bot.wait.forPickup(itemName);
            }
        };

        farmItem("Gem of Nulgath", gemQuant, 6136);
        farmItem("Totem of Nulgath", totemQuant, 5357);
    }

    /** Acquires the Purified Claymore of Destiny (quest 548; needs level 15 and Good rank 8). */
    getPCoD(): void {
        if (Core.checkInventory("Purified Claymore of Destiny")) return;

        Core.addDrop("Purified Claymore of Destiny");

        CoreNation.farm.experience(15);
        CoreNation.farm.goodREP(8);

        Core.ensureAccept(548);
        Core.huntMonster("battleundera", "Undead Berserker", "Warrior Claymore Blade", 1, { isTemp: false });
        Core.ensureComplete(548);
    }

    /**
     * Do Bloody Chaos quest (7816) for Blood Gems (level 80+).
     * @param quant Desired quantity, 100 = max stack.
     * @param relic Also register Supplies (2857) for its rewards.
     * @param endless Keep farming until stopped.
     */
    bloodyChaos(quant = 100, relic = false, hydraLevel = 85, endless = false): void {
        // Level requirement should apply regardless of Endless mode
        if (bot.player.level < 80) return;

        // Only bail early on inventory count when not farming endlessly
        if (!endless && Core.checkInventory("Blood Gem of the Archfiend", quant)) return;

        Core.addDrop("Blood Gem of the Archfiend", "Hydra Scale Piece");
        if (relic) Core.addDrop(...this.bloodyChaosSupplies);

        Core.farmingLogger("Blood Gem of the Archfiend", quant);

        Core.registerQuests(...(relic ? [7816, 2857] : [7816]));
        Core.equipClass(ClassType.Solo);

        // Endless keeps looping until manually stopped; otherwise stop once quant is reached
        while (!bot.shouldExit && (endless || !Core.checkInventory("Blood Gem of the Archfiend", quant))) {
            Core.killEscherion("Escherion's Helm", 1, { isTemp: false });
            Core.killVath("Shattered Legendary Sword of Dragon Control", 1, { isTemp: false });
            Core.huntMonster("hydrachallenge", `Hydra Head ${hydraLevel}`, "Hydra Scale Piece", 200, { isTemp: false });
        }

        Core.cancelRegisteredQuests();
    }

    /** Crag's Thirst (quest 600): turns 100 Unidentified 10 into Diamonds of Nulgath. */
    cragsThirst(quant = 1000): void {
        if (!Core.checkInventory(this.cragName) || Core.checkInventory("Diamond of Nulgath", quant) || !Core.checkInventory(this.uni(10), 100)) return;

        bot.log("Doing crags thirst");

        while (!bot.shouldExit && Core.checkInventory(this.uni(10), 100) && !Core.checkInventory("Diamond of Nulgath", quant)) {
            Core.resetQuest(600);
            Core.ensureAccept(600);
            Core.darkMakaiItem("Dark Makai Rune");
            Core.ensureComplete(600);
            bot.wait.forQuestComplete(600);
            bot.wait.forPickup("Diamond of Nulgath");
        }
    }

    /**
     * Do Kiss the Void quest (3743) for Blood Gems (or a Betrayal Blade).
     * @param quant Desired quantity, 100 = max stack.
     */
    kisstheVoid(quant = 100, betrayalBlade: string | null = null, keepVoucher = false): void {
        if (betrayalBlade === null ? Core.checkInventory("Blood Gem of the Archfiend", quant) : Core.checkInventory(betrayalBlade)) return;

        Core.addDrop(betrayalBlade ?? "Tendurrr The Assistant", "Fragment of Chaos", "Blood Gem of the Archfiend", "Broken Betrayal Blade");
        Core.equipClass(ClassType.Farm);

        if (betrayalBlade === null) Core.farmingLogger("Blood Gem of the Archfiend", quant);
        else Core.farmingLogger(betrayalBlade, 1);

        // warning for idiots that wont read it
        Core.logger("if Swindles is enabled, it will only accept the quest when it has the required Unis it needs");

        const returnPolicyDuringSupplies = Core.cboBool("Nation_ReturnPolicyDuringSupplies") === true;

        Core.logger(
            `Do Return Policy?: ${returnPolicyDuringSupplies ? "True" : "False"}\n` + `Sell Voucher of Nulgath: ${this.sellMemVoucher ? "True" : "False"}`,
        );

        Core.addDrop(
            ...this.suppliesRewards,
            ...(this.sellMemVoucher ? ["Voucher of Nulgath"] : []),
            "Relic of Chaos",
            ...(returnPolicyDuringSupplies ? [this.uni(1), this.uni(6), this.uni(9), this.uni(16), this.uni(20), "Receipt of Swindle"] : []),
        );

        Core.registerQuests(2857);

        while (
            !bot.shouldExit &&
            (betrayalBlade === null ? !Core.checkInventory("Blood Gem of the Archfiend", quant) : !Core.checkInventory(betrayalBlade))
        ) {
            Core.ensureAccept(3743);

            if (!Core.checkInventory("Tendurrr The Assistant")) {
                Core.killMonster("tercessuinotlim", "m2", "Left", "*", "Tendurrr The Assistant", 1, { isTemp: false, log: false });
                Core.jumpWait();
            }

            Core.killMonster("blindingsnow", "r17", "Left", "*", "Fragment of Chaos", 80, { isTemp: false });
            Core.killMonster("evilwarnul", "r13", "Left", "Legion Fenrir", "Broken Betrayal Blade", 8, { isTemp: false });
            Core.ensureComplete(3743);
            bot.wait.forQuestComplete(3743);

            const itemToPickup = betrayalBlade ?? "Blood Gem of the Archfiend";
            bot.wait.forDrop(itemToPickup);
            bot.wait.forPickup(itemToPickup);

            // C# passes `KeepVoucher || sellMemVoucher`, so keepVoucher made it sell.
            this.sellVoucherOfNulgath(!keepVoucher && this.sellMemVoucher, itemToPickup);

            // if `Blood Gem of the Archfiend` isnt max stack, do the quest if enabled.
            if (!Core.checkInventory("Blood Gem of the Archfiend", 100)) this.doSwindlesReturnArea(returnPolicyDuringSupplies, "Blood Gem of the Archfiend");
            // C# checks the Diamonds here; 6136 is Gem of Nulgath (see handleVoucherConversions).
            if (
                Core.checkInventory("Voucher of Nulgath (non-mem)") &&
                Core.checkInventory("Essence of Nulgath", 60) &&
                (!Core.checkInventory("Gem of Nulgath", 1000) || !Core.checkInventory("Totem of Nulgath", 100))
            ) {
                Core.ensureCompleteMulti(
                    4778,
                    !Core.checkInventory("Gem of Nulgath", 1000) ? 1000 : 100,
                    !Core.checkInventory("Gem of Nulgath", 1000) ? 6136 : 5357,
                );
            }

            if (betrayalBlade === null) {
                if (bot.inventory.isMaxStack(itemToPickup)) Core.logger("Max Stack Hit.");
                else Core.farmingLogger(itemToPickup, quant);
            }
        }
    }

    /**
     * Farms Gemstone Receipt of Nulgath (members only).
     * @param quant Desired quantity of Gemstone Receipt of Nulgath.
     */
    gemStoneReceiptOfNulgath(quant = 10): void {
        const demandingApprovalQuest = 4917;
        const receiptOfNulgathQuest = 4924;
        const receiptItemId = 33451;

        if (!Core.isMember) {
            Core.logger("This quest requires membership to be able to accept it.");
            return;
        }

        if (Core.checkInventory("Gemstone Receipt of Nulgath", quant)) return;

        Core.addDrop("Gemstone Receipt of Nulgath", "Receipt of Nulgath");

        while (!bot.shouldExit && !Core.checkInventory("Gemstone Receipt of Nulgath", quant)) {
            Core.ensureAccept(demandingApprovalQuest);

            this.farmUni13(3);
            CoreNation.farm.vampireREP();

            if (!Core.checkInventory(receiptItemId)) {
                this.dwoboCoin(100);
                Core.buyItem("crashruins", 1212, receiptItemId); // C#: "crashedruins", no such map
            }

            Core.ensureAccept(receiptOfNulgathQuest);
            this.approvalAndFavor(0, 100);
            Core.equipClass(ClassType.Farm);
            Core.huntMonster("Extinction", "Control Panel", "Coal", 15, { isTemp: false, log: false });
            this.dwoboCoin(10);
            this.essenceofNulgath(10);
            CoreNation.farm.gold(1500000);
            Core.buyItem("Tercessuinotlim", 68, "Blade of Affliction");
            Core.ensureComplete(receiptOfNulgathQuest);
            bot.wait.forPickup("Receipt of Nulgath");

            this.essenceofNulgath(100);
            this.farmTotemofNulgath(1);
            // C# farms the vouchers first, then farmTotemofNulgath's Supplies sold the member one;
            // the non-mem farm can sell it too (Bamblooze), so the member voucher comes last.
            this.farmVoucher({ member: false, keepVoucher: true });
            this.farmVoucher({ member: true, keepVoucher: true });
            Core.equipClass(ClassType.Solo);
            Core.huntMonster("ShadowfallWar", "Bonemuncher", "Ultimate Darkness Gem", 5, { isTemp: false });
            Core.ensureComplete(demandingApprovalQuest);
            bot.wait.forPickup("Gemstone Receipt of Nulgath");
        }
    }

    /**
     * Farms Dwobo Coins (quest 4798 for members, 4797 otherwise).
     * @param quant Desired quantity of Dwobo Coins.
     */
    dwoboCoin(quant: number): void {
        if (Core.checkInventory("Dwobo Coin", quant)) return;

        Core.farmingLogger("Dwobo Coin", quant);
        Core.registerQuests(Core.isMember ? 4798 : 4797);
        Core.addDrop("Dwobo Coin");

        while (!bot.shouldExit && !Core.checkInventory("Dwobo Coin", quant)) {
            const unluckyExplorerCount = Core.isMember ? 8 : 10;
            const spacetimeAnomalyCount = Core.isMember ? 5 : 7;

            Core.killMonster("crashruins", "r2", "Left", "Unlucky Explorer", "Ancient Treasure", unluckyExplorerCount, { log: false });
            Core.killMonster("crashruins", "r2", "Left", "Spacetime Anomaly", "Pieces of Future Tech", spacetimeAnomalyCount, { log: false });
            Core.huntMonster("crashruins", "Cluckmoo Idol", "Idol Heart", 1, { log: false });
        }

        bot.wait.forPickup("Dwobo Coin");
        Core.cancelRegisteredQuests();
    }

    /**
     * Farm Gemstones of Nulgath (quest 4918) for the given quantities.
     * @param bloodStone Desired quantity of Bloodstone of Nulgath.
     * @param quartz Desired quantity of Quartz of Nulgath.
     * @param tanzanite Desired quantity of Tanzanite of Nulgath.
     * @param uniGemStone Desired quantity of Unidentified Gemstone of Nulgath.
     */
    gemStonesOfnulgath(bloodStone = 100, quartz = 100, tanzanite = 100, uniGemStone = 1): void {
        const gemstonesForNulgathQuest = 4918;
        const skeletalWarriorQuest1 = 374;
        const skeletalWarriorQuest2 = 375;
        const boneTerrorQuest = 376;

        // C# `&&`: went on with only one of the two (forgeTaintedGems has `||`).
        if (!Core.checkInventory("Gemstone of Nulgath") || !Core.isMember) {
            Core.logger("This quest requires you to have Gemstone of Nulgath and membership to be able to accept it.");
            return;
        }

        this.farmUni13(1);
        this.gemStoneReceiptOfNulgath(1);
        this.supplies("Unidentified 4");

        // Prerequisites: ensure required quests are unlocked
        if (!Core.isCompletedBefore(boneTerrorQuest)) {
            Core.equipClass(ClassType.Farm);
            if (!Core.isCompletedBefore(skeletalWarriorQuest1)) {
                Core.ensureAccept(skeletalWarriorQuest1);
                Core.huntMonster("battleundera", "Skeletal Warrior", "Yara's Ring");
                Core.ensureComplete(skeletalWarriorQuest1);
            }

            if (!Core.isCompletedBefore(skeletalWarriorQuest2)) {
                Core.ensureAccept(skeletalWarriorQuest2);
                Core.huntMonster("battleundera", "Skeletal Warrior", "Skeletal Claymore", 6);
                Core.huntMonster("battleundera", "Skeletal Warrior", "Bony Chestplate", 3);
                Core.ensureComplete(skeletalWarriorQuest2);
            }

            if (!Core.isCompletedBefore(boneTerrorQuest)) {
                Core.ensureAccept(boneTerrorQuest);
                Core.huntMonster("battleundera", "Bone Terror", "Bone Terror's Head");
                Core.ensureComplete(boneTerrorQuest);
            }
        }

        Core.addDrop("Gem of Nulgath", "Bloodstone of Nulgath", "Quartz of Nulgath", "Tanzanite of Nulgath", "Unidentified Gemstone of Nulgath");

        while (
            !bot.shouldExit &&
            (!Core.checkInventory("Bloodstone of Nulgath", bloodStone) ||
                !Core.checkInventory("Quartz of Nulgath", quartz) ||
                !Core.checkInventory("Tanzanite of Nulgath", tanzanite) ||
                !Core.checkInventory("Unidentified Gemstone of Nulgath", uniGemStone))
        ) {
            Core.ensureAccept(gemstonesForNulgathQuest);

            // get other 3 quest items
            Core.equipClass(ClassType.Solo);
            Core.huntMonster("Twilight", "Abaddon", "Abbadon's Abide", 1, { isTemp: false });
            Core.equipClass(ClassType.Farm);
            Core.killMonster("ShadowfallWar", "Garden1", "Bottom", "*", "Ultimate Darkness Gem", 1, { isTemp: false });
            Core.killMonster("battleundera", "r3", "Left", "*", "Unidentified Weapon", 1, { isTemp: false });
            Core.ensureComplete(gemstonesForNulgathQuest);
        }
    }

    /**
     * [Member] Does Forge Tainted Gems for Nulgath (quest 4919) for Tainted Gems.
     * @param quant Desired quantity of Tainted Gems.
     */
    forgeTaintedGems(quant = 1000): void {
        const forgeTaintedGemsQuest = 4919;

        if (!Core.checkInventory("Gemstone of Nulgath") || !Core.isMember) {
            Core.logger("This quest requires you to have Gemstone of Nulgath and membership to be able to accept it.");
            return;
        }

        this.gemStoneReceiptOfNulgath(1);
        this.supplies("Unidentified 5");

        Core.addDrop("Tainted Gem", "Unidentified Gemstone of Nulgath");

        while (!bot.shouldExit && !Core.checkInventory("Tainted Gem", quant)) {
            Core.ensureAccept(forgeTaintedGemsQuest);
            this.farmGemofNulgath(1);
            this.gemStonesOfnulgath(0, 3, 1, 0);
            Core.ensureComplete(forgeTaintedGemsQuest);
        }
    }

    /**
     * Farms Voucher of Nulgath (member or not) with the best method available.
     * @param options `member`: if true will farm Voucher of Nulgath; false Voucher of Nulgath
     *   (non-mem). `keepVoucher`: don't sell the voucher.
     */
    farmVoucher({ member, keepVoucher = false }: { member: boolean; keepVoucher?: boolean }): void {
        if ((Core.checkInventory("Voucher of Nulgath (non-mem)") && !member) || (Core.checkInventory("Voucher of Nulgath") && member)) return;

        const voucher = member ? "Voucher of Nulgath" : "Voucher of Nulgath (non-mem)";
        Core.addDrop(voucher);
        Core.logger(`KeepVoucher set to ${keepVoucher ? "True" : "False"}`);
        if (this.hasOBoNPet || Core.checkInventory(this.cragName)) this.bambloozevsDrudgen(voucher, 1, { keepVoucher });
        this.newWorldsNewOpportunities(voucher);
        this.voidKnightSwordQuest(voucher);
        this.supplies(voucher, 1, { keepVoucher });
    }
}
