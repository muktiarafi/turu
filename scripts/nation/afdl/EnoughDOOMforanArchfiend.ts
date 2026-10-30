/*
name: EnoughDOOMforanArchfiend
description: null
tags: null
*/
// Port of Scripts/Nation/AFDL/EnoughDOOMforanArchfiend.cs.

import { bot } from "turu";
import { CoreAdvanced } from "../../core/CoreAdvanced";
import { ClassType, Core } from "../../core/CoreBots";
import { CoreFarms } from "../../core/CoreFarms";
import { CoreNation } from "../CoreNation";
import { NulgathDemandsWork } from "./NulgathDemandsWork";

const Farm = new CoreFarms();
const Adv = new CoreAdvanced();
const Nation = new CoreNation();
const nulgathDemandsWork = new NulgathDemandsWork();

export class EnoughDOOMforanArchfiend {
    afdl(): void {
        if (Core.checkInventory("ArchFiend DoomLord", 1, { toInv: false }))
            return;

        const NDWRequiredItems: string[] =
        [
            "DoomLord's War Mask",
            "ShadowFiend Cloak",
            "Locks of the DoomLord",
            "Doomblade of Destruction",
        ];
        Core.addDrop(...Nation.bagDrops);
        Core.addDrop(
            "ArchFiend DoomLord", 
            "Undead Essence", 
            "Chaorruption Essence", 
            "Essence Potion", 
            "Essence of Klunk", 
            "Living Star Essence", 
            "Bone Dust", 
            "Undead Energy", 
            "DoomLord's War Mask", 
            "ShadowFiend Cloak", 
            "Locks of the DoomLord", 
            "Doomblade of Destruction");

        Farm.experience(75);

        // Quest Accept Requirements: "DoomLord's War Mask", "ShadowFiend Cloak", "Locks of the DoomLord", "Doomblade of Destruction"
        Nation.farmUni13(1);
        Nation.approvalAndFavor(0, 1);
        nulgathDemandsWork.ndwQuest(NDWRequiredItems);

        //Quest Turnin Items:
        nulgathDemandsWork.ndwQuest([ "Unidentified 35" ]);
        // WillpowerExtraction.Unidentified34(4);
        Nation.farmBloodGem(10);
        Nation.farmVoucher({ member:false });
        Nation.essenceofNulgath(100);
        Farm.battleUnderB("Undead Essence", 1000);
        Core.ensureAccept(5260);
        Core.equipClass(ClassType.Farm);
        Core.huntMonster("orecavern", "Chaorrupted Good Soldier", "Chaorruption Essence", 75, { isTemp: false });
        Core.huntMonster("starsinc", "Living Star", "Living Star Essence", 100, { isTemp: false });

        if (!Core.checkInventory("Aelita's Emerald"))
            Core.buyItem("yulgar", 16, "Aelita's Emerald");
        Adv.buyItem("alchemyacademy", 2115, "Essence Potion", 5, 8580); // see if this works
        // if (!Core.CheckInventory("Essence Potion", 5))
        // {
        //     Farm.Gold(12500000);
        //     Core.BuyItem("alchemyacademy", 2115, "Gold Voucher 500k", 25);
        //     Core.BuyItem("alchemyacademy", 2115, "Essence Potion", 5, 8580);
        //     Bot.Wait.ForItemBuy();
        // }

        Nation.approvalAndFavor(0, 5000);
        Core.equipClass(ClassType.Solo);
        Core.huntMonster("evilwardage", "Klunk", "Essence of Klunk", 1, { isTemp: false });

        //Quest Turnin
        Core.chainComplete(5260);
        bot.wait.forPickup("ArchFiend DoomLord");
    }
}

export default function main(): void {
        Core.bankingBlackList.push(...Nation.bagDrops);
        Core.bankingBlackList.push(...
            [
                "ArchFiend DoomLord",
                "Undead Essence",
                "Chaorruption Essence",
                "Essence Potion",
                "Essence of Klunk",
                "Living Star Essence",
                "Bone Dust",
                "Undead Energy",
            ]);
        Core.setOptions();

        new EnoughDOOMforanArchfiend().afdl();

        Core.setOptions(false);
}
