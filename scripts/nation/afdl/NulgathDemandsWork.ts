/*
name: Nulgath Demands Work
description: This bot will do the Nulgath Demands Work quest untill you have a uni35 and the equipment.
tags: archfiend, doomlord, ADFL, nulgath, demands, work, unidentified, uni, 35, essence, fragment
*/
// Port of Scripts/Nation/AFDL/NulgathDemandsWork.cs.

import { bot } from "turu";
import { Core } from "../../core/CoreBots";
import { Rewards } from "../AssistingCragAndBamboozle[Mem]";
import { CoreNation } from "../CoreNation";
import { GoldenHanzoVoid } from "../various/GoldenHanzoVoid";
import { WillpowerExtraction } from "./WillpowerExtraction";

const Nation = new CoreNation();
const GHV = new GoldenHanzoVoid();
const willpowerExtraction = new WillpowerExtraction();

export class NulgathDemandsWork {
    ndwItems: string[] = [
        "DoomLord's War Mask",
        "ShadowFiend Cloak",
        "Locks of the DoomLord",
        "Doomblade of Destruction",
    ];

    doNulgathDemandsWork(): void {
        this.ndwQuest(this.ndwItems);
        this.ndwQuest([ "Unidentified 35" ], 300);
    }

    ndwQuest(itemNames: string[] | null = null, quant = 1): void {
        itemNames ??= this.ndwItems.map(item => item);
        if (itemNames.length === 0 || Core.checkInventory(itemNames, quant))
            return;
        const Rewards = Core.ensureLoad(5259).rewards;
        const NDW = Core.initializeWithRetries(() => Core.ensureLoad(5259));
        Core.addDrop(...Nation.bagDrops);
        Core.addDrop(...Rewards.map(x => x.name));

        for (const itemName of itemNames) {
            const item = Rewards.find(x => x != null && x.name === itemName);
            if (item == null) {
                Core.logger(`Item '${itemName}' not found in quest rewards, skipping.`);
                continue;
            }
            if (Core.checkInventory(item.name, quant)) {
                Core.logger(`${item.name}, x[${quant}] owned, continuing.`);
                continue;
            }

            Core.farmingLogger(item.name, quant);

            while (!bot.shouldExit && !Core.checkInventory(item.name, quant)) {
                Core.ensureAccept(5259);

                willpowerExtraction.unidentified34(10);
                // Ensure we *Always* have 1 for the accepting, and 1 for the quest
                Nation.farmUni13(2);
                this.uni27();
                Nation.farmGemofNulgath(15);
                Nation.farmBloodGem(2);
                GHV.getGHV();
                // C# farms the voucher before the three above, whose Supplies/Kiss the Void/Bamblooze
                // sell it when "Sell Voucher of Nulgath" is on (the default).
                Nation.farmVoucher({ member:true });

                // Try to buy U35 with fragments BEFORE completing quest
                if (item.name === "Unidentified 35"
                    && Core.checkInventory("Archfiend Essence Fragment", 9)) {
                    const currentUni35 = bot.inventory.getQuantity("Unidentified 35");
                    const needed = quant - currentUni35;
                    const canBuy = Math.trunc(bot.inventory.getQuantity("Archfiend Essence Fragment") / 9); // C# int division
                    const toBuy = Math.min(needed, canBuy);

                    if (toBuy > 0) {
                        Core.buyItem("tercessuinotlim", 1951, item.id, toBuy, 7912);

                        // Check if we've reached the target after buying
                        if (Core.checkInventory("Unidentified 35", quant)) {
                            Core.logger(`Reached target quantity of ${quant} Unidentified 35 by purchasing with fragments.`);
                            break; // Exit the while loop, continue to next item in foreach
                        }
                    }
                }

                // Complete quest with appropriate reward
                if (item.name === "Unidentified 35") {
                    if (bot.quests.canCompleteFullCheck(5259))
                        Core.ensureComplete(5259, item.id);
                } else {
                    // Pick ONE NDW reward that isn't maxed
                    const rewardToPick = NDW?.rewards.find(x =>
                        x != null
                        && this.ndwItems.includes(x.name)
                        && !Core.checkInventory(x.name, x.maxStack));

                    if (rewardToPick != null)
                        Core.ensureComplete(5259, rewardToPick.id);
                    else {
                        Core.logger(
                            "All NDW items are at MaxStack. Completing quest without selecting a reward.\n"
                            + "(You will still receive Archfiend Essence Fragment and Unidentified 35.)");
                        Core.ensureComplete(5259);
                    }
                }
            }
        }
    }

    uni27(): void {
        if (Core.checkInventory("Unidentified 27"))
            return;

        Core.addDrop("Unidentified 27");
        Nation.supplies("Unidentified 26", 1);
        Nation.farmTaintedGem(5); // C# doesn't farm quest 584's 5 Tainted Gems
        Core.ensureAccept(584);
        Core.resetQuest(7551);
        Core.darkMakaiItem("Dark Makai Sigil");
        Core.ensureComplete(584);
        bot.wait.forDrop("Unidentified 27");
        bot.wait.forPickup("Unidentified 27");
        Core.logger("Uni 27 acquired");
    }
}

export default function main(): void {
    const script = new NulgathDemandsWork();
        Core.bankingBlackList.push(...Nation.bagDrops);
        Core.bankingBlackList.push(...script.ndwItems);
        Core.bankingBlackList.push(...[ "Archfiend Essence Fragment", "Unidentified 35" ]);
        Core.setOptions();

        script.doNulgathDemandsWork();

        Core.setOptions(false);
}
