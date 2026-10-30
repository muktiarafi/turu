/*
name: VoidPaladin
description: null
tags: null
*/
// Port of Scripts/Nation/Various/VoidPaladin.cs.

import { bot } from "turu";
import { CoreAdvanced } from "../../core/CoreAdvanced";
import { Core } from "../../core/CoreBots";
import { CoreNSOD } from "../../evil/nsod/CoreNSOD";
import { CoreNation } from "../CoreNation";

const Nation = new CoreNation();
const NSoD = new CoreNSOD();
const Adv = new CoreAdvanced();

export class VoidPaladin {
    readonly adkRewards: string[] = [
        "Void Paladin Helm",
        "Void Paladin Horns",
        "Void Paladin Katana Cape",
        "Void Paladin Cape",
        "Void Paladin Katana",
        "Void Paladin Katanas",
    ];

    readonly cyberVoidSet: string[] = [
        "Cyber Void Paladin",
        "Cyber Void Paladin Helm",
        "Cyber Void Cape",
        "Cyber Void Light of Destiny",
    ];

    doAll(): void {
        Core.addDrop(...Nation.bagDrops);

        this.aDarkTemptation();
        this.deeperandDeeperintoDarkness();
        this.sacrifice();
        this.cyberSet();
    }

    aDarkTemptation(): void {
        if (Core.checkInventory(this.adkRewards))
            return;
        Core.addDrop(...this.adkRewards);
        Core.addDrop("Scroll of Underworld", "Archmage Ink", "Mystic Shards");

        let i = 1;
        Core.logger("Starting [A Dark Temptation] Quest");
        while (!bot.shouldExit && !Core.checkInventory(this.adkRewards, 1, { toInv: false })) {
            Core.ensureAccept(5826);

            Nation.farmDarkCrystalShard(25);
            Nation.farmDiamondofNulgath(13);
            Nation.emblemofNulgath(2);
            Nation.farmTaintedGem(35);
            Nation.farmTotemofNulgath(1);
            Nation.farmUni13(1);

            if (!Core.checkInventory("Scroll of Underworld")) {
                if (!Core.checkInventory("Archmage Ink")) {
                    Core.huntMonster("underworld", "Skull Warrior", "Mystic Shards", 2, { isTemp: false });
                    //ArchMage Ink
                    Adv.buyItem("dragonrune", 549, 13289, 1, 1641);
                }
                Core.chainComplete(2346);
                bot.drops.pickup("Scroll of Underworld");
                bot.wait.forPickup("Scroll of Underworld");
            }
            Core.ensureCompleteChoose(5826);
            Core.logger(`Completed x${i++}`);
        }
    }

    deeperandDeeperintoDarkness(): void {
        if (Core.checkInventory("Void Paladin"))
            return;

        Core.addDrop("Void Paladin");
        Core.logger("Farming Void Paladin.");

        Core.ensureAccept(5827);

        Nation.farmDiamondofNulgath(25);
        Nation.farmTaintedGem(40);
        Nation.farmVoucher({ member:false });
        Nation.farmGemofNulgath(25);
        Nation.farmDarkCrystalShard(40);
        Nation.farmUni13(2);

        if (!Core.checkInventory("Nulgath Shaped Chocolate"))
            //Nulgath Shaped Chocolate
            Adv.buyItem("citadel", 44, 38316, 1, 22367);
        NSoD.voidAuras(2);

        Core.ensureComplete(5827);
        bot.drops.pickup("Void Paladin");
        bot.wait.forPickup("Void Paladin");
    }

    sacrifice(): void {
        if (Core.checkInventory("Void Light of Destiny"))
            return;

        Core.addDrop("Void Light of Destiny");
        Core.logger("Starting [Sacrifice] Quest");

        Nation.farmDarkCrystalShard(40);
        Nation.farmDiamondofNulgath(40);
        Nation.farmTaintedGem(40);
        Nation.farmGemofNulgath(20);
        Nation.emblemofNulgath(3);
        Nation.farmTotemofNulgath(1);
        NSoD.voidAuras(6);

        if (Core.checkInventory("Blinding Light of Destiny"))
            Core.chainComplete(5828);
        else if (Core.checkInventory("Ascended Light of Destiny"))
            Core.chainComplete(5829);
    }

    cyberSet(): void {
        if (Core.checkInventory(this.cyberVoidSet, 1, { toInv: false }))
            return;

        Core.addDrop(...this.cyberVoidSet);
        // C# ignores this check; quest 6625 can't be accepted without them.
        if (
            !Core.checkInventory(
                [
                    "Void Light of Destiny",
                    "Void Paladin",
                    "Void Paladin Helm",
                    "Void Paladin Katana",
                    "Void Paladin Katana Cape",
                ])
        ) {
            Core.logger("Missing the Void Paladin items quest 6625 needs (run the other Void Paladin quests first).");
            return;
        }
        Core.ensureAccept(6625);
        Core.huntMonster("dreadspace", "Undead Space Warrior", "Powerpack", 5);
        Core.ensureComplete(6625);
    }
}

export default function main(): void {
    const script = new VoidPaladin();
        Core.bankingBlackList.push(...Nation.bagDrops);
        Core.bankingBlackList.push(...script.adkRewards);
        Core.setOptions();

        script.doAll();

        Core.setOptions(false);
}
