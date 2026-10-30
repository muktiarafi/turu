/*
name: Malgors Armor Set
description: completes the quest "build malgor's armor Set" for the set.
tags: null
*/
// Port of Scripts/Other/Armor/MalgorsArmorSet.cs.

import { bot } from "turu";
import { CoreAdvanced } from "../../core/CoreAdvanced";
import { Core } from "../../core/CoreBots";
import { DeadLinesMerge } from "../../shadowsofwar/mergeshops/DeadLinesMerge";
import { ManaCradleMerge } from "../../shadowsofwar/mergeshops/ManaCradleMerge";
import { ShadowflameFinaleMerge } from "../../shadowsofwar/mergeshops/ShadowflameFinaleMerge";
import { StreamwarMerge } from "../../shadowsofwar/mergeshops/StreamwarMerge";
import { TimekeepMerge } from "../../shadowsofwar/mergeshops/TimekeepMerge";
import { WorldsCoreMerge } from "../../shadowsofwar/mergeshops/WorldsCoreMerge";
import { CoreSoW } from "../../story/shadowsofwar/CoreSoW";

const Adv = new CoreAdvanced();
const SoW = new CoreSoW();
const deadLinesMerge = new DeadLinesMerge();
const shadowflameFinaleMerge = new ShadowflameFinaleMerge();
const timekeepMerge = new TimekeepMerge();
const streamwarMerge = new StreamwarMerge();
const worldsCoreMerge = new WorldsCoreMerge();
const manaCradleMerge = new ManaCradleMerge();

export class MalgorsArmorSet {
    armorSet: string[] = [ "Malgor the ShadowLord", "ShadowLord's Helm" ];

    questItems: string[] = [
        "Timestream Ravager",
        "ShadowFlame Defender",
        "Mana Guardian",
        "Dark Dragon Slayer",
        "Mystical Devotee of Mana",
        "Dragon's Tear",
    ];

    getSet(useSet = true, item: string[] | null = null): void {
        // Determine which items to check: default ArmorSet or provided array
        const items = useSet ? this.armorSet : item;

        if (items == null || items.length === 0)
            return; // Nothing to do if no items specified

        // If we already have all items, bank quest drops and exit
        if (Core.checkInventory(items)) {
            Core.toBank(...SoW.malgorDrops.concat(SoW.mainyuDrops));
            return;
        }

        // Prepare the store for buying/upgrading gear
        Adv.gearStore(false, { enhAfter: true });

        // Keep attempting to get items until Bot exits or inventory is complete
        while (!bot.shouldExit && !Core.checkInventory(items)) {
            deadLinesMerge.buyAllMerge("Timestream Ravager");
            shadowflameFinaleMerge.buyAllMerge("ShadowFlame Defender");
            timekeepMerge.buyAllMerge("Mana Guardian");
            streamwarMerge.buyAllMerge("Dark Dragon Slayer");
            worldsCoreMerge.buyAllMerge("Mystical Devotee of Mana");
            manaCradleMerge.buyAllMerge("Dragon's Tear");

            Adv.buyItem("alchemyacademy", 395, "Gold Voucher 500k", 30);

            // Ensure quest items are in inventory before completing quest
            Core.unbank(...this.questItems);
            Core.chainComplete(9127);
        }

        // Return to GearStore and bank quest drops
        Adv.gearStore(true, { enhAfter: true });
        Core.toBank(...SoW.malgorDrops.concat(SoW.mainyuDrops));
    }
}

export default function main(): void {
    const script = new MalgorsArmorSet();
        Core.bankingBlackList.push(...script.armorSet.concat(script.questItems));
        Core.setOptions();

        script.getSet(true, script.armorSet);

        Core.setOptions(false);
}
