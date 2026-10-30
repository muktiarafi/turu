/*
name: Archfiend
description: Gets and ranks the Archfiend Class
tags: archfiend, class, rankup, arch fiend, yo mama, archfiend class
*/
// Port of Scripts/Nation/Various/Archfiend.cs.

import { bot } from "turu";
import { CoreAdvanced } from "../../core/CoreAdvanced";
import { ClassType, Core } from "../../core/CoreBots";
import { CoreFarms } from "../../core/CoreFarms";
import { CoreNSOD } from "../../evil/nsod/CoreNSOD";
import { CoreNation } from "../CoreNation";

const Farm = new CoreFarms();
const Adv = new CoreAdvanced();
const Nation = new CoreNation();
const NSoD = new CoreNSOD();

export class ArchFiend {
    getArchfiend(rankUp = true): void {
        if (Core.checkInventory("ArchFiend")) {
            if (rankUp)
                Adv.rankUpClass("ArchFiend");
            return;
        }
        this.abyssalContract();

        Core.buyItem("tercessuinotlim", 695, 18894, 1, 1925);
        if (rankUp)
            Adv.rankUpClass("ArchFiend");
    }

    abyssalContract(): void {
        if (Core.checkInventory("Abyssal Contract"))
            return;

        Core.addDrop(...Nation.bagDrops);
        Core.addDrop("Abyssal Contract");

        Farm.experience(50);
        Core.ensureAccept(8476);

        Nation.farmUni13(3);

        Adv.buyItem("tercessuinotlim", 1951, "Pink Star Diamond of Nulgath");

        Core.huntMonster("mercutio", "Mercutio", "Immortal Joe's Black Star", 1, { isTemp: false });

        if (!Core.checkInventory("Abyssal Star")) {
            Nation.farmDarkCrystalShard(200);
            Nation.farmTaintedGem(300);
            Nation.farmGemofNulgath(200);
            Core.buyItem("evilwarnul", 456, "Abyssal Star");
        }

        Adv.buyItem("tercessuinotlim", 1951, "Gold Star of Avarice");

        if (!Core.checkInventory("Blood Star of the Archfiend")) {
            Nation.farmBloodGem(20);
            Nation.farmTotemofNulgath(8);
            if (!Core.checkInventory("Sepulchure's DoomKnight Armor"))
                NSoD.retrieveVoidAuras(2);
            else
                NSoD.voidAuras(2);
            Nation.approvalAndFavor(0, 999);
            Core.buyItem("shadowblast", 1206, "Blood Star of the Archfiend");
        }
        Core.equipClass(ClassType.Solo);
        Core.huntMonster("fiendshard", "Dirtlicker", "Dirtlicker Demoted", 1, { isTemp: false });

        Core.ensureComplete(8476);
        bot.wait.forPickup("Abyssal Contract");
    }
}

export default function main(): void {
        Core.bankingBlackList.push(...Nation.bagDrops);
        Core.setOptions();

        new ArchFiend().getArchfiend();

        Core.setOptions(false);
}
