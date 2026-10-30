/*
name: King's Echo Class
description: King's Echo Class
tags: King's, King, Echo, class, kingsecho
*/
// Port of Scripts/Other/Classes/KingsEcho.cs.

import { bot } from "turu";
import { CoreAdvanced } from "../../core/CoreAdvanced";
import { ClassType, Core } from "../../core/CoreBots";
import { CoreFarms } from "../../core/CoreFarms";
import { CoreAOR } from "../../story/ageofruin/CoreAOR";
import { Core13LoC } from "../../story/lordsofchaos/Core13LoC";
import { BocklinTreasuryMerge } from "../mergeshops/BocklinTreasuryMerge";

const Adv = new CoreAdvanced();
const Farm = new CoreFarms();
const AOR = new CoreAOR();
const BTM = new BocklinTreasuryMerge();
const LOC = new Core13LoC();

export class KingsEcho {
    getKE(rankup = true): void {
        if (Core.checkInventory("King's Echo")) {
            if (rankup)
                Adv.rankUpClass("King's Echo");
            return;
        }

        // Level 80
        Farm.experience(80);
        LOC.complete13LOC(true);

        // Rank 10 in the Good and Swordhaven factions
        Farm.goodREP();
        Farm.swordhavenREP();

        // Alden's Liberation Armor
        BTM.buyAllMerge("Alden's Liberation Armor");

        if (!Core.isCompletedBefore(10439)) {
            // Completion of the Rumbling of Cold Thunder saga
            AOR.terminaTemple(true, true);

            const q = Core.initializeWithRetries(() => Core.ensureLoad(10439));
            if (q == null) {
                Core.logger("Failed to load the quest: 10439");
                return;
            }
            for (const item of q.acceptRequirements) {
                if (Core.checkInventory(item.id))
                    continue;

                switch (item.name) {
                    case "King Alteon's Crown": // 10440 | Purify The Crown
                        // Updatequest to see queen of monsters
                        if (!Core.isCompletedBefore(8361))
                            bot.quests.updateQuest(8361);

                        Core.ensureAccept(10440);
                        Core.equipClass(ClassType.Solo);
                        Core.huntMonster("queenbattle", "Queen of Monsters", "Calcified Tear", 60, { isTemp: false });
                        Core.huntMonster("dreadhaven", "Slugwrath", "Whispers of Chaos", 40, { isTemp: false });
                        Core.huntMonster("ultradrakath", "Champion of Chaos", "Trace of Chaos", 13, { isTemp: false });
                        Core.ensureComplete(10440);
                        break;

                    case "Royal Dragon Sword": // 10442 | The Path of the King
                        Core.addDrop("Endured Against Chaos", "Endured Against a Fallen Friend", "Endured Against the Great Dragon", "Endured Against the Defiler", "Endured Against the Monster King");
                        Core.join("portal");
                        Core.ensureAccept(10442);
                        if (!bot.quests.isInProgress(10442)) {
                            Core.logger("The Path of the King [10442] could not be accepted.", "", { messageBox: true, stopBot: true });
                            return;
                        }
                        Core.equipClass(ClassType.Farm);
                        Core.killMonster("chaoswar", "r9", "Left", "*", "Endured Against Chaos", 113, { isTemp: false });
                        Core.equipClass(ClassType.Solo);
                        Core.huntMonster("falcontower", "Sepulchure", "Endured Against a Fallen Friend", 100, { isTemp: false });
                        Core.huntMonster("naoisegrave", "Volgritian", "Endured Against the Great Dragon", 70, { isTemp: false });
                        Core.huntMonster(Core.isMember ? "shattersword" : "infernalarena", Core.isMember ? "Graveclaw the Defiler" : "Destructive Defiler", "Endured Against the Defiler", 100, { isTemp: false });
                        Core.huntMonster("ebondungeon", "Dethrix", "Endured Against the Monster King", 90, { isTemp: false });
                        Core.ensureComplete(10442);
                        break;

                    case "Alteon's Reforged Armor":
                        Core.ensureAccept(10441);
                        Farm.voucher("Gold Voucher 500k", 100);
                        Core.equipClass(ClassType.Farm);
                        Core.killMonster("thelimacity", "r5", "Center", "*", "Dwarven Gold", 30, { isTemp: false });
                        Core.killMonster("liatarahill", "r9", "Left", "*", "Skye Gold", 50, { isTemp: false });
                        Core.killMonster("atlaskingdom", "r2", "Left", "*", "Atlas Gold", 75, { isTemp: false }); // Temporary solution, needs to be replaced by a farmable alternative rather than a boss.
                        Core.equipClass(ClassType.Solo);
                        Core.killMonster("camlan", "r9", "Left", "*", "Camlan Gold", 120, { isTemp: false });
                        Core.ensureComplete(10441);
                        break;

                    case "Alden's Mace":
                        Core.buyItem("castle", 2631, 96387);
                        break;
                }
            }

            // Familial Blessing (10438)
            Core.ensureAccept(10438);
            Core.getMapItem(15048, 1, "terminatemple");
            Core.ensureComplete(10438);

            // Echo of the King
            Core.chainComplete(10439);

            // Buy the class
            Core.buyItem("TerminaTemple", 2630, 95742);

            if (rankup)
                Adv.rankUpClass("King's Echo");
        }
    }
}

export default function main(): void {
        Core.setOptions(true, { disableClassSwap: true });

        new KingsEcho().getKE();

        Core.setOptions(false);
}
