/*
name: ScarletSorceress
description: null
tags: null
*/
// Port of Scripts/Other/Classes/ScarletSorceress.cs.

import { bot } from "turu";
import { CoreAdvanced, EnhancementType } from "../../core/CoreAdvanced";
import { Core } from "../../core/CoreBots";
import { CoreFarms } from "../../core/CoreFarms";
import { CoreToD } from "../../story/throneofdarkness/CoreToD";
import { BloodSorceress } from "./BloodSorceress";

const Farm = new CoreFarms();
const Adv = new CoreAdvanced();
const TOD = new CoreToD();
const BS = new BloodSorceress();

export class ScarletSorceress {
    getSSorc(rankUpClass = true): void {
        if (Core.checkInventory("Scarlet Sorceress")) {
            if (rankUpClass)
                Adv.rankUpClass("Scarlet Sorceress");
            return;
        }
        Core.addDrop("Scarlet Sorceress");
        Core.addDrop(/*Blood Sorceress*/36298);

        let BloodSorceress = bot
            .inventory.items.concat(bot.bank.items)
            .find(i => i != null && i.id === 36298);
            
        TOD.towerofMirrors();

        if (!Core.checkInventory(/*Blood Sorceress*/36298)
        || BloodSorceress != null && BloodSorceress.quantity < 302500) {
            BS.getBSorc();
            bot.wait.forPickup("Blood Sorceress");
            Adv.enhanceItem("Blood Sorceress", EnhancementType.Lucky);
        }

        BloodSorceress = bot
            .inventory.items.concat(bot.bank.items)
            .find(i => i != null && i.id === 36298);

        // Check if R10, soemtimes the game can get it stuck at r9 with 100% Cxp
        if (BloodSorceress != null && BloodSorceress.quantity < 302500) //now requires it to be rank 10?
        {
            Core.relogin("\"Blood Sorceress\" Received from Quest is unenhanced, relogging to properly set its enh. type.");
            Adv.rankUpClass("Blood Sorceress", { itemId: 36298 });
        }

        if (!Core.checkInventory("Scarlet Sorceress")) {
            Farm.experience(50);
            bot.options.aggroMonsters = false;

            Core.chainComplete(6236);
            bot.wait.forPickup("Scarlet Sorceress");
            Adv.enhanceItem("Scarlet Sorceress", EnhancementType.Lucky);

            if (rankUpClass) {
                Adv.rankUpClass("Scarlet Sorceress");
            }
        }
    }
}

export default function main(): void {
        Core.setOptions();

        new ScarletSorceress().getSSorc();

        Core.setOptions(false);
}
