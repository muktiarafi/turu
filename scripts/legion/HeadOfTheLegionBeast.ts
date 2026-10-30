/*
name: Head of the Legion Beast
description: This bot will farm the entire Head of the Legion Beast, including stories and bosses.
tags: head, legion, beast, LOTLB, seven, circles, war, penance, essence, wrath, violence, treachery, soul, heresy, indulgence, beast, helm, violence, wrath, greed, gluttony, luxuria
*/
// Port of Scripts/Legion/HeadOfTheLegionBeast.cs.
// Not ported (FarmerJoe's DoAll doesn't reach them): CenterWithFill.

import { bot, defineScriptOptions, option } from "turu";
import { CoreAdvanced } from "../core/CoreAdvanced";
import { ClassType, Core, configOr } from "../core/CoreBots";
import { CoreFarms } from "../core/CoreFarms";
import { SevenCircles } from "../story/legion/SevenCircles(War)";
import { CoreLegion } from "./CoreLegion";

const Farm = new CoreFarms();
const Adv = new CoreAdvanced();
const Legion = new CoreLegion();
const Circles = new SevenCircles();

export class HeadoftheLegionBeast {
    headLegionBeast: string[] = [
        "Penance",
        "Essence of Wrath",
        "Essence of Violence",
        "Essence of Treachery",
        "Souls of Heresy",
        "Indulgence",
        "Beast Soul",
        "Helms of the Seven Circles",
        "Faces of Violence",
        "Crown of Wrath",
        "Stare of Greed",
        "Gluttony's Maw",
        "Aspect of Luxuria",
        "Face of Treachery",
    ];

    readonly boxWidth = 44;

    legionBeastHead(badge = false): void {
        const HeadName = "Head of the Legion Beast";
        const hasHead = Core.checkInventory(HeadName);
        const wantBadge = (configOr("badge", false)) || badge;
        const hasBadge = Core.hasWebBadge(HeadName);

        if (hasHead && (!wantBadge || hasBadge))
            return;

        if (hasHead && wantBadge && !hasBadge) {
            this.doBadgeQuest();
            return;
        }

        Circles.circlesWar();
        Core.addDrop(...this.headLegionBeast);

        this.helmSevenCircles();
        this.penance(30);
        this.indulgence(30);
        Legion.farmLegionToken(15000);

        Adv.gearStore(false, { enhAfter: true });
        if (Core.checkInventory("ArchPaladin")) {
            Core.equip("ArchPaladin");
        } else
            Core.equipClass(ClassType.Solo);
        Core.killMonster("sevencircleswar", "r17", "Left", "The Beast", "Beast Soul", 15, { isTemp: false, log: false, publicRoom: true });
        Adv.gearStore(true, { enhAfter: true });

        Adv.buyItem("sevencircleswar", 1984, HeadName);

        if (wantBadge && !hasBadge) {
            this.doBadgeQuest();
        }
    }

    private doBadgeQuest(): void {
        // Head of the Legion Beast (8082)
        Core.unbank("Head of the Legion Beast");
        Core.ensureAccept(8082);
        Adv.gearStore(false);
        if (Core.checkInventory("ArchPaladin")) {
            Core.equip("ArchPaladin");
        } else
            Core.equipClass(ClassType.Solo);
        Core.huntMonster("sevencircleswar", "The Beast", "Beast Slain");
        Adv.gearStore(true);
        Core.ensureComplete(8082);
    }

    helmSevenCircles(): void {
        if (Core.checkInventory(60137))
            return;

        Core.addDrop(...this.headLegionBeast);

        this.circleHelm("Aspect of Luxuria");
        this.circleHelm("Gluttony's Maw");
        this.circleHelm("Stare of Greed");
        this.circleHelm("Crown of Wrath", true);
        this.circleHelm("Face of Treachery", true);
        this.circleHelm("Faces of Violence", true);

        Adv.buyItem("sevencircleswar", 1984, "Helms of the Seven Circles");
    }

    essenceWrath(quant = 300): void {
        if (Core.checkInventory("Essence of Wrath", quant)) {
            Core.logger(`Already have ${quant}. Skipping.`, "EssenceWrath");
            return;
        }

        Core.logger(`Starting farm for ${quant}...`, "EssenceWrath");
        Core.addDrop(...this.headLegionBeast);
        Core.equipClass(ClassType.Farm);
        Core.farmingLogger("Essence of Wrath", quant);
        Core.registerQuests(7979);

        Core.killMonster("sevencircleswar", "Enter", "Spawn", "*", "Essence of Wrath", quant, { isTemp: false, log: false });
        bot.wait.forDrop("Essence of Wrath");

        Core.cancelRegisteredQuests();
        Core.logger(`Farm complete!`, "EssenceWrath");
    }

    essenceViolence(quant = 300): void {
        if (Core.checkInventory("Essence of Violence", quant)) {
            Core.logger(`Already have ${quant}. Skipping.`, "EssenceViolence");
            return;
        }

        Core.logger(`Starting farm for ${quant}...`, "EssenceViolence");
        Core.addDrop(...this.headLegionBeast);
        Core.equipClass(ClassType.Farm);
        Core.farmingLogger("Essence of Violence", quant);
        Core.registerQuests(7985);

        Core.killMonster("sevencircleswar", "r9", "Left", "Violence Guard", "Essence of Violence", quant, { isTemp: false, log: false });
        bot.wait.forDrop("Essence of Violence");

        Core.cancelRegisteredQuests();
        Core.logger(`Farm complete!`, "EssenceViolence");
    }

    essenceTreachery(quant = 300): void {
        if (Core.checkInventory("Essence of Treachery", quant)) {
            Core.logger(`Already have ${quant}. Skipping.`, "EssenceTreachery");
            return;
        }

        Core.logger(`Starting farm for ${quant}...`, "EssenceTreachery");
        Core.addDrop(...this.headLegionBeast);
        Core.equipClass(ClassType.Farm);
        Core.farmingLogger("Essence of Treachery", quant);
        Core.registerQuests(7988);

        Core.killMonster("sevencircleswar", "r13", "Left", "Treachery Guard", "Essence of Treachery", quant, { isTemp: false, log: false });
        bot.wait.forDrop("Essence of Treachery");

        Core.cancelRegisteredQuests();
        Core.logger(`Farm complete!`, "EssenceTreachery");
    }

    soulsHeresy(quant = 300): void {
        const SOULS_MAX_STACK = 300;
        quant = Math.min(quant, SOULS_MAX_STACK); // Cap at max stack

        const soulsOnHandStart = bot.inventory.getQuantity("Souls of Heresy");

        if (Core.checkInventory("Souls of Heresy", quant)) {
            Core.logger(`Already have ${quant}. Skipping.`, "SoulsHeresy");
            return;
        }

        Core.logger(`Starting farm for ${quant} souls (capped at max stack ${SOULS_MAX_STACK})`, "SoulsHeresy");
        Core.logger(`  Starting inventory: ${soulsOnHandStart}/${SOULS_MAX_STACK}`, "SoulsHeresy");
        Core.addDrop(...this.headLegionBeast);

        if (!bot.quests.isUnlocked(7983)) {
            Core.logger(`Quest not unlocked. Running Circles.CirclesWar()...`, "SoulsHeresy");
            Circles.circlesWar(true);
        }

        Core.farmingLogger("Souls of Heresy", quant);
        Core.registerQuests(7983, 7980, 7981); // Blasphemy? Blasphe-you! | War Medals | Mega War Medals

        Core.equipClass(ClassType.Farm);
        Core.logger(`Target this session: ${quant} souls | Collecting souls from heretics...`, "SoulsHeresy");

        let killCount = 0;
        let lastLoggedProgress = soulsOnHandStart;

        while (!bot.shouldExit && !Core.checkInventory("Souls of Heresy", quant)) {
            Core.killMonster("sevencircleswar", "r7", "Left", "*", null, 1, { log: false });
            bot.wait.forDrop("Souls of Heresy");

            killCount++;
            const currentSouls = bot.inventory.getQuantity("Souls of Heresy");

            // Log progress every 25 kills or when we hit major milestones
            if (killCount % 25 === 0 || currentSouls >= lastLoggedProgress + 50) {
                const soulsInThisSession = currentSouls - soulsOnHandStart;
                Core.logger(`Collected: ${soulsInThisSession}/${quant} | Inventory: ${currentSouls}/${SOULS_MAX_STACK} | Kills: ${killCount}`, "SoulsHeresy");
                lastLoggedProgress = currentSouls;
            }
        }

        const soulsAtEnd = bot.inventory.getQuantity("Souls of Heresy");
        const totalCollected = soulsAtEnd - soulsOnHandStart;

        Core.cancelRegisteredQuests();
        Core.logger(`✓ Collected ${totalCollected} souls | Final: ${soulsAtEnd}/${SOULS_MAX_STACK} | Kills: ${killCount}`, "SoulsHeresy");
    }

    penance(quant = 300): void {
    if (Core.checkInventory("Penance", quant)) {
        Core.farmingLogger("Penance", quant);
        Core.logger(`Already have ${quant} Penance. Skipping.`, "Penance");
        return;
    }

    Core.addDrop(...this.headLegionBeast);
    Core.farmingLogger("Penance", quant);
    Core.equipClass(ClassType.Farm);

    const essencePerPenance = 1;
    const soulsPerPenance = 15;
    const soulsMaxStack = 300;
    const maxPenancePerSoulStack = Math.trunc(soulsMaxStack / soulsPerPenance);

    let currentPenance = bot.inventory.getQuantity("Penance");
    let penanceBought = currentPenance;
    let totalSoulsUsed = 0;

    Core.logger("", "Penance");
    Core.logger("┌─ PHASE 1: Penance Material Farming", "Penance");
    Core.logger(
        `│ Target: ${quant} Penance | Existing: ${currentPenance} | Remaining: ${quant - currentPenance}`, 
        "Penance");
    Core.logger(
        `│ Cost: ${essencePerPenance} of each essence + ${soulsPerPenance} Souls per Penance`, 
        "Penance");
    Core.logger(
        `│ Souls max stack: ${soulsMaxStack} (${maxPenancePerSoulStack} Penance)`, 
        "Penance");
    Core.logger("├────────────────────────────────────────", "Penance");

    while (!bot.shouldExit && penanceBought < quant) {
        currentPenance = bot.inventory.getQuantity("Penance");
        const penanceRemaining = quant - currentPenance;

        if (penanceRemaining <= 0)
            break;

        let wrath = bot.inventory.getQuantity("Essence of Wrath");
        let violence = bot.inventory.getQuantity("Essence of Violence");
        let treachery = bot.inventory.getQuantity("Essence of Treachery");
        let souls = bot.inventory.getQuantity("Souls of Heresy");

        // Merge anything we can immediately make from existing materials.
        let craftableFromEssences = Math.min(
            Math.trunc(wrath / essencePerPenance), 
            Math.min(
                Math.trunc(violence / essencePerPenance), 
                Math.trunc(treachery / essencePerPenance)));

        let craftableFromSouls = Math.trunc(souls / soulsPerPenance); // C# int division

        let immediatelyCraftable = Math.min(
            penanceRemaining, 
            Math.min(craftableFromEssences, craftableFromSouls));

        if (immediatelyCraftable > 0) {
            const soulsCost = immediatelyCraftable * soulsPerPenance;

            Core.logger(
                `│ → Merging ${immediatelyCraftable} Penance ` +
                `(${immediatelyCraftable * essencePerPenance} each essence + ${soulsCost} souls)...`, 
                "Penance");

            Core.buyItem(
                "sevencircleswar", 
                1984, 
                "Penance", 
                immediatelyCraftable);

            bot.wait.forPickup("Penance");

            currentPenance = bot.inventory.getQuantity("Penance");
            penanceBought = currentPenance;
            totalSoulsUsed += soulsCost;

            Core.logger(
                `│ ✓ Penance: ${penanceBought}/${quant} | ` +
                `Souls remaining: ${bot.inventory.getQuantity("Souls of Heresy")}/${soulsMaxStack}`, 
                "Penance");

            Core.sleep(500);
            continue;
        }

        // Prepare only enough materials for the next batch.
        // A full 300 Souls stack supports exactly 20 Penance.
        const batchSize = Math.min(penanceRemaining, maxPenancePerSoulStack);
        const essenceTarget = batchSize * essencePerPenance;
        const soulsTarget = batchSize * soulsPerPenance;

        // Farm only the missing Essence of Wrath.
        wrath = bot.inventory.getQuantity("Essence of Wrath");

        if (wrath < essenceTarget) {
            Core.logger(
                `│ ↓ Farming ${essenceTarget - wrath} Essence of Wrath ` +
                `(${wrath}/${essenceTarget})...`, 
                "Penance");

            this.essenceWrath(essenceTarget);

            if (bot.shouldExit)
                break;
        }

        // Farm only the missing Essence of Violence.
        violence = bot.inventory.getQuantity("Essence of Violence");

        if (violence < essenceTarget) {
            Core.logger(
                `│ ↓ Farming ${essenceTarget - violence} Essence of Violence ` +
                `(${violence}/${essenceTarget})...`, 
                "Penance");

            this.essenceViolence(essenceTarget);

            if (bot.shouldExit)
                break;
        }

        // Farm only the missing Essence of Treachery.
        treachery = bot.inventory.getQuantity("Essence of Treachery");

        if (treachery < essenceTarget) {
            Core.logger(
                `│ ↓ Farming ${essenceTarget - treachery} Essence of Treachery ` +
                `(${treachery}/${essenceTarget})...`, 
                "Penance");

            this.essenceTreachery(essenceTarget);

            if (bot.shouldExit)
                break;
        }

        // Recheck essence quantities after farming.
        wrath = bot.inventory.getQuantity("Essence of Wrath");
        violence = bot.inventory.getQuantity("Essence of Violence");
        treachery = bot.inventory.getQuantity("Essence of Treachery");
        souls = bot.inventory.getQuantity("Souls of Heresy");

        craftableFromEssences = Math.min(
            Math.trunc(wrath / essencePerPenance), 
            Math.min(
                Math.trunc(violence / essencePerPenance), 
                Math.trunc(treachery / essencePerPenance)));

        craftableFromSouls = Math.trunc(souls / soulsPerPenance); // C# int division

        immediatelyCraftable = Math.min(
            penanceRemaining, 
            Math.min(craftableFromEssences, craftableFromSouls));

        // If the essences are ready but Souls are not, farm only the
        // missing Souls required for this batch.
        if (immediatelyCraftable <= 0 && souls < soulsTarget) {
            const soulsNeeded = Math.min(
                soulsTarget - souls, 
                soulsMaxStack - souls);

            if (soulsNeeded > 0) {
                Core.logger(
                    `│ ↓ Farming ${soulsNeeded} Souls of Heresy ` +
                    `(${souls}/${soulsMaxStack})...`, 
                    "Penance");

                // SoulsHeresy() takes the amount to farm.
                this.soulsHeresy(soulsTarget);

                souls = bot.inventory.getQuantity("Souls of Heresy");

                Core.logger(
                    `│ ↑ Souls complete: ${souls}/${soulsMaxStack}`, 
                    "Penance");
            }
        }

        Core.sleep(500);
    }

    penanceBought = bot.inventory.getQuantity("Penance");

    Core.logger("Penance: ├────────────────────────────────────────");
    Core.logger(`Penance: └─ ✓ Phase Complete! ${penanceBought}/${quant} Penance`);
    Core.logger(`Penance: ✓ Souls used: ${totalSoulsUsed}`);
    Core.logger("Penance: ");
    Core.logger(`╔${"═".repeat(this.boxWidth)}╗`);
    Core.logger(`║${this.centerWithFill("✦ PENANCE FARMING SESSION COMPLETE ✦")}║`);
    Core.logger(`╚${"═".repeat(this.boxWidth)}╝`);
    }

    // C#: a local function of Penance (an odd nesting in the C# file).
    private centerWithFill(text: string): string {
        const decorated = ` ${text} `;

        if (decorated.length >= this.boxWidth)
            return decorated;

        const fillTotal = this.boxWidth - decorated.length;
        const left = Math.trunc(fillTotal / 2);
        const right = fillTotal - left;

        return "═".repeat(left) + decorated + "═".repeat(right);
    }

    indulgence(quant = 100): void {
        if (Core.checkInventory("Indulgence", quant)) {
            Core.logger(`Already have ${quant} Indulgence. Skipping.`, "Indulgence");
            return;
        }

        Core.logger(`Starting farm for ${quant} Indulgence`, "Indulgence");
        Core.addDrop(...this.headLegionBeast);
        Core.equipClass(ClassType.Farm);
        Core.farmingLogger("Indulgence", quant);

        let currentQuantity = bot.inventory.getQuantity("Indulgence");
        const deficit = quant - currentQuantity;

        Core.logger(`Current: ${currentQuantity} | Deficit: ${deficit} | Target: ${quant}`, "Indulgence");

        // Calculate targets based on deficit
        const soulsTarget =
            deficit >= 3 ? 75
            : deficit === 2 ? 50
            : 25;
        const essenceTarget = deficit >= 3 ? 3 : deficit;

        Core.logger(`Soul target: ${soulsTarget} | Essence target: ${essenceTarget} per essence`, "Indulgence");

        while (!bot.shouldExit && !Core.checkInventory("Indulgence", quant)) {
            Core.logger(`Starting quest cycle...`, "Indulgence");

            Core.ensureAccept(7978);
            Core.equipClass(ClassType.Farm);

            Core.logger(`Farming ${soulsTarget} Souls of Limbo...`, "Indulgence");
            Core.killMonster("sevencircles", "r2", "Left", "Limbo Guard", "Souls of Limbo", soulsTarget, { log: false });

            Core.equipClass(ClassType.Solo);

            Core.logger(`Farming essences...`, "Indulgence");
            Core.killMonster("sevencircles", "r4", "Left", "Luxuria", "Essence of Luxuria", essenceTarget, { isTemp: false, log: false });
            Core.killMonster("sevencircles", "r6", "Left", "Gluttony", "Essence of Gluttony", essenceTarget, { isTemp: false, log: false });
            Core.killMonster("sevencircles", "r8", "Left", "Avarice", "Essence of Avarice", essenceTarget, { isTemp: false, log: false });

            Core.logger(`Completing quest...`, "Indulgence");
            Core.ensureCompleteMulti(7978);
            bot.wait.forPickup("Indulgence");

            currentQuantity = bot.inventory.getQuantity("Indulgence");
            Core.logger(`Progress: ${currentQuantity}/${quant}`, "Indulgence");
        }

        Core.cancelRegisteredQuests();
        Core.logger(`Farm complete! Got ${bot.inventory.getQuantity("Indulgence")} Indulgence`, "Indulgence");
    }

    circleHelm(helm: string, war = false): void {
        if (Core.checkInventory(helm)) {
            Core.logger(`Already have ${helm}. Skipping.`, "CircleHelm");
            return;
        }

        Core.logger(`Starting farm for ${helm}`, "CircleHelm");
        Core.farmingLogger(helm, 1);
        Legion.farmLegionToken(1500);

        if (war) {
            this.penance(10);
            Adv.buyItem("sevencircleswar", 1984, helm);
        } else {
            this.indulgence(10);
            Adv.buyItem("sevencircles", 1980, helm);
        }

        Core.logger(`Farm complete! Got ${helm}`, "CircleHelm");
    }
}

export const scriptOptions = defineScriptOptions("hotlb", [
    Core.skipOptions,
    option.bool("badge", "Farm Badge", "Set to true to farm the Head of the Legion Beast char page badge", false),
], true);

export default function main(): void {
    const script = new HeadoftheLegionBeast();
        Core.bankingBlackList.push(...script.headLegionBeast);
        Core.setOptions();

        script.legionBeastHead();

        Core.setOptions(false);
}
