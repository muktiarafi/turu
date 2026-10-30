/*
name: TarosManslayer
description: null
tags: null
*/
// Port of Scripts/Nation/Various/TarosManslayer.cs.

import { bot } from "turu";
import { ClassType, Core } from "../../core/CoreBots";
import { CoreFarms } from "../../core/CoreFarms";
import { CoreNation } from "../CoreNation";
import { PurifiedClaymoreOfDestiny } from "./PurifiedClaymoreOfDestiny";

const Farm = new CoreFarms();
const Nation = new CoreNation();
const PCoD = new PurifiedClaymoreOfDestiny();

export class TarosManslayer {
    private rewards: string[] = [ "Taro's Manslayer", "Taro Blademaster Guardian" ];

    guardianTaro(ManslayerOnly = true): void {
        if (ManslayerOnly) {
            this.rewards = [ "Taro's Manslayer" ];
        }

        if (Core.checkInventory(this.rewards))
            return;

        Core.addDrop(...this.rewards);

        if (ManslayerOnly) {
            Core.equipClass(ClassType.Solo);
            Core.huntMonster("tercessuinotlim", "Taro Blademaster", "Taro's Manslayer", 1, { isTemp: false });
        } else {
            if (!bot.player.isMember) {
                Core.logger(
                    "Membership REQUIRED for this quest ( \"The Guardian Taro Blademaster\")");
                return;
            }

            Farm.goodREP();
            PCoD.getPCoD();

            while (!bot.shouldExit && !Core.checkInventory(this.rewards)) {
                Core.ensureAccept(1111);
                Nation.farmGemofNulgath(10);
                Core.resetQuest(7551);
                Core.darkMakaiItem("Dark Makai Rune");
                Core.ensureCompleteChoose(1111, this.rewards);
                Core.sleep();
            }
        }
    }
}

export default function main(): void {
        Core.setOptions();

        new TarosManslayer().guardianTaro(!Core.isMember);

        Core.setOptions(false);
}
