/*
name: ExaltedApotheosisPreReqs
description: null
tags: null
*/
// Port of Scripts/Other/Weapons/ExaltedApotheosisPreReqs.cs.

import { bot } from "turu";
import { CoreAdvanced } from "../../core/CoreAdvanced";
import { ClassType, Core } from "../../core/CoreBots";
import { CoreFarms } from "../../core/CoreFarms";

const Farm = new CoreFarms();
const Adv = new CoreAdvanced();

export class ExaltedApotheosisPreReqs {
    private weps: string[] = [
        "Exalted Apotheosis",
        "Exalted Penultima",
        "Exalted Unity",
        "Apostate Ultima",
        "Thaumaturgus Ultima",
        "Apostate Omega",
        "Thaumaturgus Omega",
    ];

    preReqs(): void {
        // Ensure shop is loaded:
        Core.join("timeinn");
        while (!bot.shouldExit && (bot.shops?.name !== "Exaltia Merge")) {
            bot.shops?.load(2010);
            bot.wait.forTrue(() => bot.shops?.name === "Exaltia Merge", 20);
            Core.sleep();
        }

        bot.wait.forActionCooldown("loadShop");
        // Core.readShopItems: the shop items with their merge requirements (C# `ShopItem.Requirements`).
        const exaltedApo = Core.readShopItems().find(x => x.name === "Exalted Apotheosis");

        Core.equipClass(ClassType.Farm);
        const missingMaterials = new Map<string, number>();

        while (!bot.shouldExit && !Core.checkInventory("Exalted Apotheosis")) {
            // Define the weapon pairs in each tier
            const weaponPairs = [
                [ "Apostate Alpha", "Thaumaturgus Alpha" ],
                [ "Apostate Omega", "Thaumaturgus Omega" ],
                [ "Apostate Ultima", "Thaumaturgus Ultima" ],
                [ "Exalted Penultima", "Exalted Unity" ],
            ];

            let obtained = false;

            for (const pair of weaponPairs) {
                const hasPairInInventory = pair.every(wep => bot.inventory?.contains(wep) === true);

                // Check if the pair is already in the inventory
                if (hasPairInInventory) {
                    Core.logger(`Pair already owned: ${pair.join(", ")}`);
                    continue;
                }

                for (const wep of pair) {
                    if (Core.checkInventory(wep)) {
                        Core.logger(`Already owned: ${wep}`);
                        continue;
                    }

                    Core.logger(`Working on: ${wep}`);
                    const wepData = Core.readShopItems().find(x => x.name === wep);

                    // Check if the weapon has any requirements before buying
                    if (wepData != null && wepData.requirements?.length > 0) {
                        let canBuy = true;
                        for (const req of wepData.requirements) {
                            if (Core.checkInventory(req.id, req.quantity))
                                continue;
                            switch (req.name) {
                                case "Exalted Node":
                                    Core.killMonster("timeinn", "r3", "Bottom", "*", "Exalted Node", req.quantity, { isTemp: req.temp });
                                    break;
                                case "Exalted Artillery Shard":
                                    Core.huntMonster("timeinn", "The Engineer", req.name, req.quantity, { isTemp: req.temp });
                                    break;
                                case "Exalted Relic Piece":
                                    Core.huntMonster("timeinn", "The Warden", req.name, req.quantity, { isTemp: req.temp });
                                    break;
                                case "Exalted Forgemetal":
                                    Core.huntMonster("timeinn", "Ezrajal", req.name, req.quantity, { isTemp: req.temp });
                                    break;
                                default:
                                    missingMaterials.set(req.name, req.quantity - bot.inventory.getQuantity(req.id));
                                    canBuy = false;
                                    break;
                            }
                        }

                        const status = canBuy ? "\u2713" : "\u274C"; // Checkmark or red X
                        Core.logger(`Can complete: ${status}`);

                        // Buy the weapon after fulfilling requirements
                        if (
                            canBuy
                            && wepData.requirements.every(req =>
                                Core.checkInventory(req.id, req.quantity))
                        )
                            Adv.buyItem("timeinn", 2010, wep);

                        if (Core.checkInventory("Exalted Apotheosis")) {
                            obtained = true;
                            break;
                        }
                    }
                }

                if (obtained || Core.checkInventory("Exalted Apotheosis"))
                    break;
            }

            if (Core.checkInventory("Exalted Apotheosis")) {
                Core.logger("Congratulations on completing the Exalted Apotheosis weapon!");
                break;
            } else if (!obtained && exaltedApo?.requirements != null) {
                for (const item of exaltedApo.requirements) {
                    const missingQuantity =
                        item.quantity - (bot.inventory?.getQuantity(item.id) ?? 0); // It's not handled automatically if Inventory is null, only if item is not in inventory!
                    if (missingQuantity > 0)
                        missingMaterials.set(item.name, missingQuantity);
                }
                break; // Exit the loop if the item cannot be obtained
            } else if (!obtained) {
                Core.logger("Exalted Apotheosis item not found in shop.");
                break;
            }
        }

        if (missingMaterials.size > 0) {
            bot.log(
                "Missing materials:\n"
                    + [...missingMaterials].map(([key, value]) => `\t${key}: x ${value}`).join("\n"));
        }

        bot.wait.forPickup("Exalted Apotheosis");
    }
}

export default function main(): void {
        Core.bankingBlackList.push(...
            [
                "Ezrajal Insignia",
                "Warden Insignia",
                "Engineer Insignia",
                "Exalted Relic Piece",
                "Exalted Artillery Shard",
                "Exalted Forgemetal",
            ]);
        Core.setOptions();

        new ExaltedApotheosisPreReqs().preReqs();

        Core.setOptions(false);
}
