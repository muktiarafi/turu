/*
name: Wrath of Nulgath
description: This script will get Wrath of Nulgath.
tags: wrathofnulgath, wrath, ravenous, weapon
*/
// Port of Scripts/Other/Weapons/WrathofNulgath.cs.

import { bot } from "turu";
import { CoreAdvanced } from "../../core/CoreAdvanced";
import { Core } from "../../core/CoreBots";
import { CoreNation } from "../../nation/CoreNation";
import { JuggernautItemsofNulgath, RewardsSelection } from "../../nation/various/JuggernautItems";
import { DarkWarLegionandNation } from "../../story/legion/DarkWarLegionandNation";

const Adv = new CoreAdvanced();
const Nation = new CoreNation();
const juggernaut = new JuggernautItemsofNulgath();
const DWLN = new DarkWarLegionandNation();

export class WrathofNulgath {
    getSword(): void {
        if (Core.checkInventory("Wrath of Nulgath"))
            return;

        DWLN.doBoth();

        Core.logger("Farming Wrath of Nulgath.");

        juggernaut.juggItems(RewardsSelection.Overfiend_Blade_of_Nulgath);
        Nation.farmVoucher({ member:false, keepVoucher: true });
        Nation.farmVoucher({ member:true, keepVoucher: true });
        Nation.farmUni13(1);
        Nation.farmTaintedGem(80);
        Nation.farmDarkCrystalShard(60);
        Nation.farmDiamondofNulgath(100);
        Adv.buyItem("darkwarnation", 2123, "Wrath of Nulgath");
        bot.wait.forPickup("Wrath of Nulgath");
    }
}

export default function main(): void {
        Core.setOptions();

        new WrathofNulgath().getSword();

        Core.setOptions(false);
}
