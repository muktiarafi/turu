/*
name: Fire Champion's Armor
description: This script will farm Fire Champion's Armor.
tags: fire champions armor, fire champions armor, polished dragon slayer, flame forged metal, void scale, damage dragon, heros valiance, fca
*/
// Port of Scripts/Other/Armor/FireChampionsArmor.cs.

import { bot } from "turu";
import { CoreAdvanced } from "../../core/CoreAdvanced";
import { ClassType, Core } from "../../core/CoreBots";
import { CoreFarms } from "../../core/CoreFarms";
import { Lair } from "../../story/Lair";
import { WarfuryEmblem } from "../WarFuryEmblem";
import { DragonslayerGeneral } from "../classes/DragonslayerGeneral";

const Farm = new CoreFarms();
const Adv = new CoreAdvanced();
const DSG = new DragonslayerGeneral();
const WFE = new WarfuryEmblem();
const lair = new Lair();

export class FireChampionsArmor {
    getFireChampsArmor(): void {
        if (Core.checkInventory(62570))
            return;

        this.polishedDragonSlayer();
        DSG.enchantedScaleandClaw(125, 0);
        WFE.warfuryEmblemFarm(60);
        this.flameForgedMetal(10);
        this.voidScale(13);
        Adv.buyItem("wartraining", 2035, "Fire Champion's Armor", 1, 8759);
    }

    polishedDragonSlayer(): void {
        if (Core.checkInventory(58462))
            return;

        if (!Core.isCompletedBefore(168))
            lair.galanoth();

        if (!Core.checkInventory(582))
            Core.buyItem("lair", 38, "Dragonslayer");

        Adv.rankUpClass("Dragonslayer");

        WFE.warfuryEmblemFarm(30);
        DSG.enchantedScaleandClaw(30, 0);
        Core.addDrop(11475);
        Core.farmingLogger("Dragon Scale", 30);
        Core.equipClass(ClassType.Farm);
        while (!bot.shouldExit && !Core.checkInventory(11475, 30))
            Core.killMonster("lair", "Hole", "Center", "*", null, 1, { isTemp: false, log: false });
        Adv.buyItem("wartraining", 2035, "Polished DragonSlayer");
    }

    flameForgedMetal(Metalquant = 10): void {
        if (Core.checkInventory("Flame-Forged Metal", Metalquant))
            return;

        Core.equipClass(ClassType.Solo);
        Core.addDrop("Flame-Forged Metal");

        Core.farmingLogger("Flame-Forged Metal", Metalquant);

        Core.registerQuests(6975);
        while (!bot.shouldExit && !Core.checkInventory("Flame-Forged Metal", Metalquant))
            Core.huntMonster("underworld", "Frozen Pyromancer", "Stolen Flame", 1, { log: false });
        Core.cancelRegisteredQuests();
    }

    voidScale(VoidScaleQuant = 13): void {
        if (Core.checkInventory("Void Scale", VoidScaleQuant))
            return;

        const OptimalClasses = ["Yami no Ronin", "Chaos Slayer"];

        const usethis = bot.inventory.items.concat(bot.bank.items)
            .find(n => OptimalClasses.some(o => n.name === o || n.name.startsWith(o)));

        Core.logger(usethis != null ? `${usethis.name} found -- equipping optimal class.`
            : !!Core.dodgeClass ? "No optimal class found, falling back to Dodge Class."
            : "No optimal/dodge class found, using equipped class for Archfiend Dragonlord... GOOD LUCK!!");

        if (usethis != null) {
            Core.equip(usethis.id);
            Core.equip(...Core.dodgeGear);
            if (usethis.name === "Yami no Ronin")
                bot.skills.startAdvanced("1 | 4");
        } else if (!!Core.dodgeClass)
            Core.equipClass(ClassType.Dodge);

        Core.huntMonster("underlair", "Archfiend Dragonlord", "Void Scale", VoidScaleQuant, { isTemp: false });
    }
}

export default function main(): void {
        Core.setOptions();

        new FireChampionsArmor().getFireChampsArmor();

        Core.setOptions(false);
}
