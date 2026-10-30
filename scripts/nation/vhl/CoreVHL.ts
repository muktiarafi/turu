/*
name: null
description: null
tags: null
*/
// Port of Scripts/Nation/VHL/CoreVHL.cs.

import { bot, defineScriptOptions, option } from "turu";
import { CoreAdvanced } from "../../core/CoreAdvanced";
import { Core, configOr } from "../../core/CoreBots";
import { CoreDailies } from "../../core/CoreDailies";
import { CoreFarms } from "../../core/CoreFarms";
import { AssistingCragAndBamboozle, Rewards } from "../AssistingCragAndBamboozle[Mem]";
import { CoreNation } from "../CoreNation";

const Farm = new CoreFarms();
const Adv = new CoreAdvanced();
const Daily = new CoreDailies();
const Nation = new CoreNation();
const ACAB = new AssistingCragAndBamboozle();

export class CoreVHL {
    useSparrowMethod = true;

    private readonly challengeRewards: string[] = [
        "Void Highlord Armor",
        "Helm of the Highlord",
        "Highlord's Void Wrap",
    ];

    getVHL(rankUpClass = true): void {
        if (Core.checkInventory("Void Highlord")) {
            if (rankUpClass)
                Adv.rankUpClass("Void Highlord");
            return;
        }

        this.vhlChallenge(15);
        this.vhlCrystals();

        if (
            Core.checkInventory("Roentgenium of Nulgath", 15)
            && Core.checkInventory([ "Void Crystal A", "Void Crystal B" ])
        ) {
            Adv.buyItem("tercessuinotlim", 1355, "Void Highlord");
            if (rankUpClass)
                Adv.rankUpClass("Void Highlord");
        }
    }

    vhlChallenge(quant: number, farmextra = false): void {
        if (Core.checkInventory("Roentgenium of Nulgath", quant))
            return;

        Core.logger("Getting Void HighLord Challenge prerequisites");
        Farm.experience(80);
        Core.addDrop(...Nation.bagDrops);
        if (!Core.checkInventory(this.challengeRewards, undefined, { toInv: false }))
            Core.addDrop(...this.challengeRewards);
        Core.addDrop("Roentgenium of Nulgath");

        Core.farmingLogger("Roentgenium of Nulgath", quant);
        while (!bot.shouldExit && !Core.checkInventory("Roentgenium of Nulgath", quant)) {
            Core.ensureAccept(5660);

            Core.killMonster("tercessuinotlim", "m4", "Right", "Shadow of Nulgath", "Hadean Onyx of Nulgath", undefined, { isTemp: false });
            if (
                !Core.checkInventory(
                    "Elders' Blood", 
                    quant - Core.dynamicQuant("Elders' Blood", false) > 5
                        ? 5
                        : (quant - Core.dynamicQuant("Elders' Blood", false)))
            ) {
                Daily.eldersBlood();
                this._SparrowMethod(
                    (quant - Core.dynamicQuant("Elders' Blood", false)) > 5
                        ? 5
                        : (quant - Core.dynamicQuant("Elders' Blood", false)));
            }

            Nation.farmVoucher({ member:false });
            Farm.blackKnightOrb();
            Adv.buyItem("citadel", 44, 38316, undefined, 22367);
            if (!Core.checkInventory("Aelita's Emerald"))
                Adv.buyItem("yulgar", 16, "Aelita's Emerald");
            Nation.farmUni13(1);
            Nation.farmGemofNulgath(20);
            Nation.emblemofNulgath(20);
            Nation.essenceofNulgath(50);
            Nation.farmTaintedGem(100);
            Nation.approvalAndFavor(300, 300);

            Core.ensureComplete(5660);
            bot.wait.forPickup("Roentgenium of Nulgath");
            Core.toBank(...this.challengeRewards);

            if (
                Core.checkInventory("Elders' Blood")
                && !Core.checkInventory("Roentgenium of Nulgath", quant)
            )
                continue; // C#: goto Continue (the top of this loop)
            else if (
                !Core.checkInventory("Elders' Blood")
                && !Core.checkInventory("Roentgenium of Nulgath", quant)
            ) {
                Core.logger(
                    `Not enough \"Elders' Blood\", please do the daily ${2 - Core.dynamicQuant("Elders' Blood", false)}`);
                if (farmextra)
                    this.farmExtra();
                return;
            }
        }
    }

    vhlCrystals(): void {
        if (Core.checkInventory("Void Crystal A") && Core.checkInventory("Void Crystal B"))
            return;

        Core.addDrop(...Nation.bagDrops);
        Nation.farmUni13(1);
        if (!Core.checkInventory("Void Crystal A")) {
            Core.logger("Obtaining Void Crystal A ");
            Nation.farmUni10(200);
            Nation.farmTaintedGem(200);
            Nation.farmDarkCrystalShard(200);
            Nation.farmGemofNulgath(150);
            Adv.buyItem("tercessuinotlim", 1355, "Void Crystal A");
        }
        if (!Core.checkInventory("Void Crystal B")) {
            Core.logger("Obtaining Void Crystal B");
            if (!Core.checkInventory("Elders' Blood", 2))
                Daily.eldersBlood();
            this._SparrowMethod(2);

            if (!Core.checkInventory("Elders' Blood", 2)) {
                Core.logger(
                    `Not enough \"Elders' Blood\", please do the daily ${2 - bot.inventory.getQuantity("Elders' Blood")} more times (not today)`);
                Core.logger("Gathering Materials for Void Crystal B");
                Nation.farmTotemofNulgath(15);
                Nation.farmDiamondofNulgath(200);
                Nation.farmBloodGem(30);
                Core.logger("Mats are all preped for Void Crystal B");
            } else {
                Nation.farmTotemofNulgath(15);
                Nation.farmDiamondofNulgath(200);
                Nation.farmBloodGem(30);
                Adv.buyItem("tercessuinotlim", 1355, "Void Crystal B");
            }
        }
    }

    private farmExtra(quant = 25): void {
        // 17 for VHL
        // 8 for Void Crystals
        // 10 for The Fiend Shard's Final quest

        if (Core.checkInventory("Roentgenium of Nulgath", quant))
            return;

        const quantity = Core.dynamicQuant("Roentgenium of Nulgath", false);

        Core.logger(
            `Roentgenium of Nulgath: ${Core.dynamicQuant("Roentgenium of Nulgath", false)}/${quant}`, 
            "Not Enough Roent");
        Core.logger(
            "Not enough \"Roentgenium of Nulgath\", maxing mats so it's easier tomorrow. You can just leave this running!");

        // Farm Mats for Tomarrow.
        Farm.blackKnightOrb();
        Adv.buyItem("citadel", 44, 38316, undefined, 22367);
        if (!Core.checkInventory("Aelita's Emerald"))
            Adv.buyItem("yulgar", 16, "Aelita's Emerald");
        Nation.farmUni13(13);
        Nation.farmGemofNulgath(1000);
        Nation.emblemofNulgath(500);
        Nation.essenceofNulgath(60);
        Nation.farmTaintedGem(1000);
        Nation.approvalAndFavor(5000, 5000); // Core.AddDrop("Totem of Nulgath", "Blood Gem of Nulgath", "Voucher of Nulgath", "Voucher of Nulgath (non-mem)");
        Nation.farmBloodGem();
        if (!Core.checkInventory("Unidentified 19")) {
            const item = Core.ensureLoad(7551).rewards.find(x => x.id === 57446);
            Nation.swindleReturn(item!.name, 6);
            Adv.buyItem("tercessuinotlim", 1951, "Unidentified 19");
        }
        Nation.farmTotemofNulgath(15); // 15 for Void Crystal B
        Nation.farmVoucher({ member:false });
        Nation.farmVoucher({ member:true });
        Core.logger("Materials max out! You should be good for tomorrow.");
    }

    private _SparrowMethod(EldersBloodQuant: number): void {
        if (
            !configOr("SparrowMethod", false)
            || !Core.isMember
            || !Core.checkInventory(Nation.cragName)
            || Core.checkInventory("Elders' Blood", EldersBloodQuant)
        )
            return;
        ACAB.assistingCandB(Rewards.Elders_Blood);
    }
}

export const scriptOptions = defineScriptOptions("VoidHighLordOptions", [
    option.bool("SparrowMethod", "Use Sparrow's Blood Method", "When possible, it will use \"Assisting Crag and Bamboozle\" to get an additional Elders' Blood per day.\n"
                + "Needs Crag and Bamboozle and is Legend-Only.\n"
                + "Will not be done if you don't meed the conditions\n"
                + "Recommended setting: True", true),
    Core.skipOptions,
], true);
