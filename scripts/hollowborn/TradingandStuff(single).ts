/*
name: Trading and Stuff (Single)
description: This script will complete "Trading and Stuff (Single)" quest.
tags: trading and stuff, single, hollowborn oblivion blade, hollowborn, oblivion, blade
*/
// Port of Scripts/Hollowborn/TradingandStuff(single).cs.

import { bot } from "turu";
import { CapeSpecial, CoreAdvanced, EnhancementType, HelmSpecial, WeaponSpecial } from "../core/CoreAdvanced";
import { Core } from "../core/CoreBots";
import { CoreFarms } from "../core/CoreFarms";
import { CoreNation } from "../nation/CoreNation";
import { CoreHollowborn } from "./CoreHollowborn";

const Farm = new CoreFarms();
const Adv = new CoreAdvanced();
const HB = new CoreHollowborn();
const Nation = new CoreNation();

export class TradingandStuffSingle {
    getBlade(): void {
        if (
            Core.checkInventory("Hollowborn Oblivion Blade")
            || !Core.checkInventory("Oblivion Blade of Nulgath")
        )
            return;

        Core.addDrop("Hollowborn Oblivion Blade");

        Core.ensureAccept(7295);
        Farm.experience(80);
        Nation.farmVoucher({ member:false });
        this.archFiendEnchantedOrbs();
        Core.huntMonster("shadowattack", "Death", "Death's Power", 1, { isTemp: false });
        if (!Core.checkInventory("Unidentified 25")) {
            Farm.gold(15000000);
            Core.buyItem("tercessuinotlim", 1951, "Unmoulded Fiend Essence");
            bot.wait.forPickup("Unmoulded Fiend Essence");
            Core.buyItem("tercessuinotlim", 1951, "Unidentified 25");
        }
        HB.freshSouls(1, 100);
        Core.ensureComplete(7295);
        bot.wait.forPickup("Hollowborn Oblivion Blade");
        Adv.enhanceItem(
            "Hollowborn Oblivion Blade", 
            EnhancementType.Lucky, 
            { cSpecial: CapeSpecial.None, hSpecial: HelmSpecial.None, wSpecial: WeaponSpecial.Spiral_Carve });
    }

    archFiendEnchantedOrbs(): void {
        if (Core.checkInventory("ArchFiend Enchanted Orbs"))
            return;

        Core.logger("Farming ArchFiend Enchanted Orbs.");

        HB.freshSouls(1, 100);
        if (!Core.checkInventory("Unidentified 25")) {
            Farm.gold(15000000);
            Core.buyItem("tercessuinotlim", 1951, "Unmoulded Fiend Essence");
            Core.buyItem("tercessuinotlim", 1951, "Unidentified 25");
        }
        Nation.farmUni13(1);
        Nation.farmDiamondofNulgath(150);
        Nation.farmBloodGem(10);
        Nation.farmVoucher({ member:false });
        Core.buyItem("tercessuinotlim", 1820, "ArchFiend Enchanted Orbs");
        bot.wait.forPickup("ArchFiend Enchanted Orbs");
    }
}

export default function main(): void {
        Core.setOptions();

        new TradingandStuffSingle().getBlade();

        Core.setOptions(false);
}
