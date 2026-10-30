/*
name: null
description: null
tags: null
*/
// Port of Scripts/Legion/YamiNoRonin/CoreYnR.cs.

import { bot } from "turu";
import { CoreAdvanced } from "../../core/CoreAdvanced";
import { ClassType, Core } from "../../core/CoreBots";
import { CoreSoW } from "../../story/shadowsofwar/CoreSoW";
import { CoreLegion } from "../CoreLegion";
import { SwordMaster } from "../SwordMaster";

const Adv = new CoreAdvanced();
const Legion = new CoreLegion();
const SOW = new CoreSoW();
const SM = new SwordMaster();

export class CoreYnR {
    private nonLegion = false;

    getYnR(rankUpClass = true): void {
        // There is an armor called YNR.
        if (Core.checkInventory(53841))
            return;

        Core.addDrop("Yami no Ronin");

        Core.ensureAccept(7408);
        this.blademasterSwordScroll();
        this.yokaiSwordScroll();
        Core.ensureComplete(7408);
        bot.wait.forPickup("Yami no Ronin");

        if (rankUpClass)
            Adv.rankUpClass("Yami no Ronin");
    }

    yokaiSwordScroll(): void {
        if (Core.checkInventory("Yokai Sword Scroll"))
            return;

        SOW.darkAlly();
        Core.farmingLogger("Yokai Sword Scroll", 1);
        Core.addDrop("Yami no Ronin Katana", "Yokai Sword Scroll");

        Core.ensureAccept(7445);

        Core.equipClass(ClassType.Farm);
        Core.killMonster("shadowfortress", "r12", "Top", "*", "Perfect Orochi Scales", 888, { isTemp: false });

        Core.equipClass(ClassType.Farm);
        Core.killMonster("shadowrealmpast", "Enter", "Spawn", "*", "Darkened Essence", 600, { isTemp: false });

        Adv.buyItem("akiba", 131, "Oni Skull Charm");
        if (!Core.checkInventory("Shadow Blade Katana")) {
            this.yami(10);
            this.foldedSteel();
            Adv.buyItem("underworld", 238, "Platinum Paragon Medal", 15);
            Adv.buyItem("underworld", 577, "Shadow Katana Blade", 1);
        }

        Core.ensureComplete(7445);
    }

    yami(quant = 10): void {
        if (Core.checkInventory("Yami", quant))
            return;

        SOW.darkAlly();
        Core.farmingLogger("Yami", quant);
        Core.addDrop("Yami");
        Core.equipClass(ClassType.Farm);
        Core.registerQuests(7409);
        while (!bot.shouldExit && !Core.checkInventory("Yami", quant)) {
            Core.killMonster("darkally", "r2", "Left", "*", "Dark Wisp", 444, { isTemp: false });
            bot.wait.forPickup("Yami");
        }
        Core.cancelRegisteredQuests();
    }

    foldedSteel(): void {
        if (Core.checkInventory("Folded Steel"))
            return;

        Core.logger("Checking which method to use");
        if (!Core.isCompletedBefore(793))
            this.nonLegion = true;
        else
            this.nonLegion = false;

        Core.farmingLogger("Folded Steel", 1);
        Core.addDrop("Folded Steel");

        Core.ensureAccept(7444);
        Core.equipClass(ClassType.Solo);
        Core.huntMonster("fotia", "Amia the Cult Leader", "Eternity Flame", 1, { isTemp: false });
        Core.huntMonster("shadowfortress", "Jaaku", "Shadow Katana Blueprint", 1, { isTemp: false });
        if (!Core.checkInventory("Obsidian Rock", 108)) {
            if (this.nonLegion)
                Legion.obsidianRock(108);
            else {
                Legion.soulForgeHammer();
                Legion.farmLegionToken(220);
                Core.buyItem("underworld", 577, "Obsidian Rock", 108);
            }
        }
        this.flameForgedMetal(13);
        Core.equipClass(ClassType.Solo);
        Core.killMonster("doomvaultb", "r26", "Left", "Undead Raxgore", "Weapon Imprint", 15, { isTemp: false });

        Core.ensureComplete(7444);
    }

    flameForgedMetal(quant = 13): void {
        if (Core.checkInventory("Flame-Forged Metal", quant))
            return;

        Core.farmingLogger("Flame-Forged Metal", quant);
        Core.addDrop("Flame-Forged Metal");
        Core.equipClass(ClassType.Solo);
        Core.registerQuests(6975);
        while (!bot.shouldExit && !Core.checkInventory("Flame-Forged Metal", quant)) {
            Core.huntMonster("Underworld", "Frozen Pyromancer", "Stolen Flame", 1, { isTemp: true });
            bot.wait.forPickup("Flame-Forged Metal");
        }
        Core.cancelRegisteredQuests();
    }

    blademasterSwordScroll(nonLegion = false): void {
        if (Core.checkInventory("Blademaster Sword Scroll"))
            return;

        Core.logger("Checking which method to use");
        if (!Core.isCompletedBefore(793))
            nonLegion = true;
        else
            nonLegion = false;

        SOW.darkAlly();
        Core.farmingLogger("Blademaster Sword Scroll", 1);
        Core.addDrop("Blademaster Sword Scroll");

        Core.equipClass(ClassType.Solo);
        if (nonLegion) {
            Core.ensureAccept(7410);
            Core.logger("Using Non-Legion variant for the Blademaster Sword Scroll");
            Core.equipClass(ClassType.Dodge);
            Core.killMonster("frozenlair", "r3", "Left", "Legion Lich Lord", "Sapphire Orb", 26, { isTemp: false, publicRoom: true });
            Core.equipClass(ClassType.Solo);
            Core.killMonster("Judgement", "r10a", "Spawn", "Ultra Aeacus", "Aeacus Empowered", 100, { isTemp: false, publicRoom: true });
            Core.huntMonster("darkally", "Underfiend", "Traitor's Tract", 250, { isTemp: false });
            Core.huntMonster("shadowsong", "Oh'Garr", "Ogre Titan's Resonance", 250, { isTemp: false });
            Core.huntMonster("shadowgrove", "Titan Shadow Dragonlord", "Shadow Dragonlord's Shroud", 250, { isTemp: false });
            Core.equipClass(ClassType.Farm);
            Core.huntMonster("evilwardage", "Blade Master", "Discipline", 1, { isTemp: false });
            Legion.dagePvP(400, 50, 1000);
            Core.ensureComplete(7410);
        } else {
            Core.logger("Using Legion variant for the Blademaster Sword Scroll");
            Core.ensureAccept(7443);
            Core.equipClass(ClassType.Dodge);
            Core.killMonster("frozenlair", "r3", "Left", "Legion Lich Lord", "Sapphire Orb", 13, { isTemp: false, publicRoom: true });
            Legion.farmLegionToken(17500);
            Core.equipClass(ClassType.Solo);
            Core.killMonster("Judgement", "r10a", "Spawn", "Ultra Aeacus", "Aeacus Empowered", 50, { isTemp: false, publicRoom: true });

            if (!Core.checkInventory("Meditation")) {
                Core.addDrop("Meditation");

                let m_questID = -1;
                if (Core.checkInventory(22933)) {
                    // BM LT
                    Core.logger("Using LT BladeMaster for Meditation");
                    m_questID = 7412;
                    Adv.rankUpClass("BladeMaster");
                } else if (Core.checkInventory(22859)) {
                    // BM AC
                    Core.logger("Using AC BladeMaster for Meditation");
                    m_questID = 7411;
                    Adv.rankUpClass("BladeMaster");
                } else if (Core.checkInventory(53836)) {
                    // SM AC
                    Core.logger("Using AC SwordMaster for Meditation");
                    m_questID = 7413;
                    Adv.rankUpClass("SwordMaster");
                } else {
                    Core.logger("Using LT SwordMaster for Meditation");
                    if (!Core.checkInventory("SwordMaster")) {
                        Core.logger("Farming LT SwordMaster for Meditation");
                        SM.getSwordMaster();
                    }
                    m_questID = 7414;
                    Adv.rankUpClass("SwordMaster");
                }
                Core.equipClass(ClassType.Solo);

                Core.ensureAccept(m_questID);
                Legion.boneSigil(1);
                Core.ensureComplete(m_questID);
                bot.wait.forPickup("Meditation");
            }
            Core.ensureComplete(7443);
        }
    }
}
