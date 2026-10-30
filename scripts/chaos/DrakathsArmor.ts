/*
name: Drakath Armor
description: Gets the Drakath Armor / Original Drakath Armor
tags: drakath, drakath armor, original drakath armor
*/
// Port of Scripts/Chaos/DrakathsArmor.cs.

import { bot } from "turu";
import { CoreAdvanced } from "../core/CoreAdvanced";
import { ClassType, Core } from "../core/CoreBots";
import { CoreDailies } from "../core/CoreDailies";
import { CoreFarms } from "../core/CoreFarms";
import { BLODMethod, CoreBLOD } from "../good/blod/CoreBLOD";
import { CoreNation } from "../nation/CoreNation";
import { Core13LoC } from "../story/lordsofchaos/Core13LoC";

const Farm = new CoreFarms();
const Daily = new CoreDailies();
const Adv = new CoreAdvanced();
const BLOD = new CoreBLOD();
const LOC = new Core13LoC();
const Nation = new CoreNation();

export class DrakathArmorBot {
    getBoth(): void {
        this.drakathOriginalArmor();
        if (!Core.checkInventory("Original Drakath Armor"))
            return;

        this.drakathArmor();
    }

    drakathArmor(): void {
        if (Core.checkInventory("Drakath Armor"))
            return;

        LOC.complete13LOC();
        this.drakathArmorQuest();
        if (!Core.checkInventory(60796))
            return;

        Core.join("confrontation");
        if (bot.shouldExit)
            return;

        bot.shops.load(994);
        if (!bot.shops.isLoaded || bot.shops.id !== 994) {
            Core.logger("Could not load Drakath Armor shop 994. The quest reward token is already owned.");
            return;
        }

        // The reward token confirms the turn-in even when the quest history is stale.
        bot.shops.buyItem("Drakath Armor", 1);
        bot.wait.forTrue(() => bot.inventory.contains("Drakath Armor"), 20);
        if (!Core.checkInventory("Drakath Armor"))
            Core.logger("Drakath Armor purchase failed with reward token 60796 present. Check the shop unlock and inventory space.");
    }

    drakathOriginalArmor(): void {
        if (Core.checkInventory("Original Drakath Armor"))
            return;

        LOC.complete13LOC();
        this.drakathArmorQuest();
        if (!Core.checkInventory(60796))
            return;

        Core.join("confrontation");
        if (bot.shouldExit)
            return;

        bot.shops.load(994);
        if (!bot.shops.isLoaded || bot.shops.id !== 994) {
            Core.logger("Could not load Drakath Armor shop 994. The quest reward token is already owned.");
            return;
        }

        // The reward token confirms the turn-in even when the quest history is stale.
        bot.shops.buyItem("Original Drakath Armor", 1);
        bot.wait.forTrue(() => bot.inventory.contains("Original Drakath Armor"), 20);
        if (!Core.checkInventory("Original Drakath Armor"))
            Core.logger("Original Drakath Armor purchase failed with reward token 60796 present. Check the shop unlock and inventory space.");
    }

    drakathArmorQuest(): void {
        if (
            Core.checkInventory(
                60796)
        )
            return;

        Core.addDrop(
            "Dage's Scroll Fragment", 
            "Treasure Chest", 
            "Face of Chaos", 
            "Get Your Original Drakath's Armor");
        Core.ensureAccept(3882);
        Core.logger("Getting Quest Accept Requirement: Blinding Light of Destiny");
        BLOD.blindingLightOfDestiny(BLODMethod.Fewest_Hours);
        if (!Core.checkInventory("Blinding Light of Destiny")) {
            Core.logger(
                "You do not own \"Blinding Light of Destiny\". We'll Farm the materials, but you still need top run this again to get BLoD.");
        }

        // Get Accept Requirement: Blind Light of Destiny
        Farm.chaosREP();

        Farm.bladeofAweREP(6, true);
        Nation.farmUni13(3);
        Adv.buyItem("hyperspace", 194, "Le Chocolat");
        Core.equipClass(ClassType.Farm);
        Core.killMonster("swordhavenundead", "Left", "Right", "*", "Treasure Chest", 100, { isTemp: false });
        Core.equipClass(ClassType.Solo);
        Core.killMonster("ultradrakath", "r1", "Left", "Champion of Chaos", "Face of Chaos", 1, { isTemp: false, publicRoom: true });

        if (!Core.checkInventory("Dage's Scroll Fragment", 13)) {
            Daily.dagesScrollFragment();
            if (!Core.checkInventory("Dage's Scroll Fragment", 13)) {
                Core.logger("Build Drakath Armor needs 13 Dage scroll fragments before turn-in. The daily did not provide enough. Run again when more are available.");
                return;
            }
        }

        if (
            Core.checkInventory("Dage's Scroll Fragment", 13)
            && Core.checkInventory("Blinding Light of Destiny")
        ) {
            Core.ensureAccept(3882);
            if (!bot.quests.isInProgress(3882)) {
                Core.logger("Build Drakath Armor quest 3882 could not be accepted. The armor purchase was skipped.");
                return;
            }

            Core.ensureComplete(3882);
            bot.wait.forPickup(60796);
            if (!Core.checkInventory(60796))
                Core.logger(`Build Drakath Armor turn-in did not yield reward token 60796. Current Dage scroll fragments: ${bot.inventory.getQuantity("Dage's Scroll Fragment")}. Check the quest turn-in and reward pickup before farming more fragments.`);
        }
    }
}

export default function main(): void {
        Core.setOptions();

        new DrakathArmorBot().getBoth();

        Core.setOptions(false);
}
