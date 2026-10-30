/*
name: Prime Fiend Shard Quests
description: This script will complete the storyline from the prime fiend shard
tags: prime fiend shard, nation, ravenous, fiend shard, nulgath, voidchasm, void chasm
*/
// Port of Scripts/Nation/Various/PrimeFiendShard.cs.

import { bot } from "turu";
import { CoreAdvanced } from "../../core/CoreAdvanced";
import { Core } from "../../core/CoreBots";
import { CoreStory } from "../../core/CoreStory";
import { TradingandStuffSingle } from "../../hollowborn/TradingandStuff(single)";
import { VoidAvengerScythe } from "../../other/weapons/VoidAvengerScythe";
import { WrathofNulgath } from "../../other/weapons/WrathofNulgath";
import { TempleDelveMerge } from "../../seasonal/staffbirthdays/nulgath/TempleDelveMerge";
import { CoreNation } from "../CoreNation";
import { NulgathDemandsWork } from "../afdl/NulgathDemandsWork";
import { DilligasMerge } from "../mergeshops/DilligasMerge";
import { DirtlickersMerge } from "../mergeshops/DirtlickersMerge";
import { NulgathDiamondMerge } from "../mergeshops/NulgathDiamondMerge";
import { VoidChasmMerge } from "../mergeshops/VoidChasmMerge";
import { VoidRefugeMerge } from "../mergeshops/VoidRefugeMerge";
import { CoreVHL } from "../vhl/CoreVHL";
import { ArchfiendDeathLord, RewardChoice } from "./ArchfiendDeathLord";
import { SwirlingTheAbyss } from "./SwirlingTheAbyss";
import { VoidPaladin } from "./VoidPaladin";
import { VoidSpartan } from "./VoidSpartan";

const Story = new CoreStory();
const Adv = new CoreAdvanced();
const VHL = new CoreVHL();
const Nation = new CoreNation();
const voidRefugeMerge = new VoidRefugeMerge();
const templeDelveMerge = new TempleDelveMerge();
const dirtlickersMerge = new DirtlickersMerge();
const voidPaladin = new VoidPaladin();
const nulgathDiamondMerge = new NulgathDiamondMerge();
const voidSpartan = new VoidSpartan();
const swirlingTheAbyss = new SwirlingTheAbyss();
const tradingandStuffSingle = new TradingandStuffSingle();
const voidAvengerScythe = new VoidAvengerScythe();
const nulgathDemandsWork = new NulgathDemandsWork();
const ArchfienddDeathLord = new ArchfiendDeathLord();
const wrathofNulgath = new WrathofNulgath();
const dilligasMerge = new DilligasMerge();
const voidChasmMerge = new VoidChasmMerge();

export class PrimeFiendShard {
    storyline(): void {
        if (Core.isCompletedBefore(9559))
            return;

        Core.logger(
            "If you're going to use insingias for the roents, please do so before starting the script, as it will stop otherwise if dont have them.");

        // Required Class
        VHL.getVHL();

        // Check for 10 roents
        VHL.vhlChallenge(10);

        // Prime Fiend Shard [required to accept]
        voidChasmMerge.buyAllMerge("Prime Fiend Shard");

        // Ensure requirements are unbanked
        Core.unbank("Prime Fiend Shard");

        // Feed the Fiend Shard 9555
        if (!Story.questProgression(9555)) {
            Core.ensureAccept(9555);
            if (!Core.checkInventory("Dual Dragonbone Axe of Nulgath")) {
                Core.addDrop("Dual Dragonbone Axe of Nulgath");
                Core.logger("Farming Dual Dragonbone Axe of Nulgath.");
                // Combat Style: Dragonbone Axe 629 - requires 1x uni 13
                Core.ensureAccept(629);
                Nation.farmUni13(1);
                Nation.farmTaintedGem(13);
                Nation.farmDarkCrystalShard(13);
                Nation.farmDiamondofNulgath(13);
                Nation.supplies(Nation.uni(21));
                Core.huntMonster("evilmarsh", "Tainted Elemental", "Tainted Rune of Evil", 1, { log: false });
                Core.ensureComplete(629);
            }
            // 9559 requires x13 uni 13
            Nation.farmUni13(13);
            Nation.essenceofNulgath(20);
            templeDelveMerge.buyAllMerge("Void Nation Caster");
            dirtlickersMerge.buyAllMerge("Iron Dreadsaw");

            // "Nation Soulstealer"
            Nation.farmDiamondofNulgath(20);
            Adv.buyItem("shadowblast", 1206, "Nation Soulstealer", 1, 3615, 1);


            // Ensure requirements are unbanked
            const quest = Core.initializeWithRetries(() => Core.ensureLoad(9555));
            for (const Item of quest!.requirements)
                while (!bot.shouldExit && !Core.checkInventory(Item.id)) {
                    Core.unbank(Item.id);
                    Core.sleep();
                }
            Core.ensureComplete(9555);
        }

        // Grow the Fiend Shard 9556
        if (!Story.questProgression(9556)) {
            Core.ensureAccept(9556);
            Nation.essenceofNulgath(40);
            nulgathDiamondMerge.buyAllMerge("Abyssal Priest of Nulgath");
            nulgathDiamondMerge.buyAllMerge("Storm Knight");
            voidPaladin.deeperandDeeperintoDarkness();
            voidSpartan.getSpartan("Void Spartan");

            // Ensure requirements are unbanked
            const quest = Core.initializeWithRetries(() => Core.ensureLoad(9556));
            for (const Item of quest!.requirements)
                while (!bot.shouldExit && !Core.checkInventory(Item.id)) {
                    Core.unbank(Item.id);
                    Core.sleep();
                }
            Core.ensureComplete(9556);
        }

        // Evolve the Fiend Shard 9557
        if (!Story.questProgression(9557)) {
            Core.ensureAccept(9557);
            swirlingTheAbyss.sta("Void Soul of Nulgath");
            tradingandStuffSingle.archFiendEnchantedOrbs();
            nulgathDiamondMerge.buyAllMerge("Blood Ranger");
            voidRefugeMerge.buyAllMerge("Envenomed Edge of Nulgath");
            Nation.essenceofNulgath(60);

            // Ensure requirements are unbanked
            const quest = Core.initializeWithRetries(() => Core.ensureLoad(9557));
            for (const Item of quest!.requirements)
                while (!bot.shouldExit && !Core.checkInventory(Item.id)) {
                    Core.unbank(Item.id);
                    Core.sleep();
                }
            Core.ensureComplete(9557);
        }

        // The Fiend Shard's Demand 9558
        if (!Story.questProgression(9558)) {
            Core.ensureAccept(9558);
            voidAvengerScythe.skewsQuests();
            nulgathDemandsWork.ndwQuest([ "Doomblade of Destruction" ]);
            ArchfienddDeathLord.getArm(true, RewardChoice.Archfiend_DeathLord);
            wrathofNulgath.getSword();
            dilligasMerge.buyAllMerge("Ancient Shogun Armor");
            //ooga booga it wont complete

            // Ensure requirements are unbanked
            const quest = Core.initializeWithRetries(() => Core.ensureLoad(9558));
            for (const Item of quest!.requirements)
                while (!bot.shouldExit && !Core.checkInventory(Item.id)) {
                    Core.unbank(Item.id);
                    Core.sleep();
                }
            Core.ensureComplete(9558);
        }

        // The Fiend Shard's Finale 9559
        if (!Story.questProgression(9559)) {
            Core.ensureAccept(9559);

            // If cragname owned do this order
            if (Core.checkInventory(Nation.cragName)) {
                Core.logger("Prefarming max diamonds for BG and U13 excahnges");
                Nation.farmDiamondofNulgath(1000);
                Nation.farmTotemofNulgath(60);
                Nation.farmBloodGem(100);
                Nation.farmUni13(13);
                Nation.farmDiamondofNulgath(1000);
                Nation.farmTaintedGem(750);
                Nation.farmDarkCrystalShard(400);
                Nation.farmGemofNulgath(300);
            }
            // Else do this order
            else {
                Nation.farmUni13(13);
                Nation.farmTaintedGem(750);
                Nation.farmDarkCrystalShard(400);
                Nation.farmDiamondofNulgath(1000);
                Nation.farmTotemofNulgath(60);
                Nation.farmGemofNulgath(300);
                Nation.farmBloodGem(100);
            }


            if (Core.checkInventory("Roentgenium of Nulgath", 10))
            { // Ensure requirements are unbanked
                const quest = Core.initializeWithRetries(() => Core.ensureLoad(9559));
                Core.unbank(...Core.ensureLoad(8916).requirements.map(x => x.id));
                Core.ensureComplete(9559);
            } else
                Core.logger(
                    `Not enough Roents ${bot.inventory.getQuantity("Roentgenium of Nulgath")} / 10`);
        }

        const filteredReceipts = Nation
            .receipt.filter(item => !item.startsWith("Unidentified"));
        Core.toBank(...filteredReceipts);
        Core.toBank(...Nation.tercessBags);
        Core.toBank(...Nation.bagDrops);
        Core.toBank(...Nation.suppliesRewards);
        Core.toBank(...Nation.swindlesReturnRewards);
    }
}

export default function main(): void {
        for (let questId = 9555; questId <= 9559; questId++) {
            const quest = Core.initializeWithRetries(() => Core.ensureLoad(questId));

            if (quest != null) {
                for (const item of quest.requirements) {
                    Core.bankingBlackList.push(item.name);
                }
            }
        }

        Core.setOptions();

        new PrimeFiendShard().storyline();

        Core.setOptions(false);
}
