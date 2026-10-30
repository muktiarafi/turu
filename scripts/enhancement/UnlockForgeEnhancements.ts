/*
name: Unlock Forge Enhancements
description: This script will farm all forge enhancements.
tags: lacerate, smite, herosvaliance, arcanasconcerto, elysium, acheron, absolution, vainglory, avarice, penitence, lament, vim, examen, anima, pneuma, dauntless, praxis, ravenous
*/
// Port of Scripts/Enhancement/UnlockForgeEnhancements.cs.

import { bot, choices, defineScriptOptions, option } from "turu";
import { EternalDrakath } from "../chaos/EternalDrakathSet";
import { CoreAdvanced } from "../core/CoreAdvanced";
import { ClassType, Core, configOr } from "../core/CoreBots";
import { CoreDailies } from "../core/CoreDailies";
import { CoreFarms } from "../core/CoreFarms";
import { CoreStory } from "../core/CoreStory";
import { LordOfOrder } from "../dailies/LordOfOrder";
import { CoreDarkon } from "../darkon/CoreDarkon";
import { PrinceDarkonsPoleaxePreReqs } from "../darkon/various/PrinceDarkonsPoleaxePreReqs";
import { SepulchuresOriginalHelm } from "../evil/SepulchuresOriginalHelm";
import { CoreNSOD } from "../evil/nsod/CoreNSOD";
import { ArchPaladin } from "../good/ArchPaladin";
import { Awescended } from "../good/gearofawe/Awescended";
import { CoreAwe } from "../good/gearofawe/CoreAwe";
import { CoreLegion } from "../legion/CoreLegion";
import { HeadoftheLegionBeast } from "../legion/HeadOfTheLegionBeast";
import { CoreYnR } from "../legion/yaminoronin/CoreYnR";
import { NulgathDemandsWork } from "../nation/afdl/NulgathDemandsWork";
import { PrimeFiendShard } from "../nation/various/PrimeFiendShard";
import { FireChampionsArmor } from "../other/armor/FireChampionsArmor";
import { MalgorsArmorSet } from "../other/armor/MalgorsArmorSet";
import { DragonOfTime } from "../other/classes/DragonOfTime";
import { CoreSoWMats } from "../shadowsofwar/CoreSoWMats";
import { Core7DD } from "../story/7deadlydragons/Core7DD";
import { ThirdSpell } from "../story/ThirdSpell";
import { YokaiQuests } from "../story/Yokai";
import { CoreAstravia } from "../story/elegyofmadness(darkon)/CoreAstravia";
import { SevenCircles } from "../story/legion/SevenCircles(War)";
import { Core13LoC } from "../story/lordsofchaos/Core13LoC";
import { CoreSepulchure } from "../story/sepulchuresaga/CoreSepulchure";
import { CoreSoW } from "../story/shadowsofwar/CoreSoW";

const Farm = new CoreFarms();
const Story = new CoreStory();
const Adv = new CoreAdvanced();
const Legion = new CoreLegion();
const Darkon = new CoreDarkon();
const SOWM = new CoreSoWMats();
const Awe = new CoreAwe();
const LOC = new Core13LoC();
const CorNSOD = new CoreNSOD();
const Astravia = new CoreAstravia();
const Daily = new CoreDailies();
const DD = new Core7DD();
const YNR = new CoreYnR();
const SoW = new CoreSoW();
const CoreSS = new CoreSepulchure();
const AP = new ArchPaladin();
const DOT = new DragonOfTime();
const FCA = new FireChampionsArmor();
const ED = new EternalDrakath();
const Seppy = new SepulchuresOriginalHelm();
const PDPPR = new PrinceDarkonsPoleaxePreReqs();
const HOTLB = new HeadoftheLegionBeast();
const awescended = new Awescended();
const NDW = new NulgathDemandsWork();
const TSS = new ThirdSpell();
const LOO = new LordOfOrder();
const Circles = new SevenCircles();
const Yokai = new YokaiQuests();
const MAS = new MalgorsArmorSet();
const PFS = new PrimeFiendShard();

export class UnlockForgeEnhancements {
    forgeUnlocks(): void {
        const selectedCapeEnhancements = configOr("ForgeQuestCape", ForgeQuestCape.ForgeCapeEnhancement);
        const selectedWeaponEnhancements = configOr("ForgeQuestWeapon", ForgeQuestWeapon.ForgeWeaponEnhancement);
        const selectedHelmEnhancements = configOr("ForgeQuestHelm", ForgeQuestHelm.ForgeHelmEnhancement);

        if (
            selectedCapeEnhancements === ForgeQuestCape.None
            && selectedWeaponEnhancements === ForgeQuestWeapon.None
            && selectedHelmEnhancements === ForgeQuestHelm.None
        )
            Core.logger(
                "All settings are set to None, no Forge Quest to do. Stopping script.", undefined, { stopBot: true });

        if (selectedWeaponEnhancements !== ForgeQuestWeapon.None) {
            if (selectedWeaponEnhancements !== ForgeQuestWeapon.All)
                Core.logger(
                    `Selected Forge Weapon Enhancement: ${configOr("ForgeQuestWeapon", ForgeQuestWeapon.ForgeWeaponEnhancement)}`);

            switch (selectedWeaponEnhancements) {
                case ForgeQuestWeapon.ForgeWeaponEnhancement:
                    this.forgeWeaponEnhancement();
                    break;

                case ForgeQuestWeapon.Lacerate:
                    this.lacerate();
                    break;

                case ForgeQuestWeapon.Smite:
                    this.smite();
                    break;

                case ForgeQuestWeapon.Praxis:
                    this.praxis();
                    break;

                case ForgeQuestWeapon.HerosValiance:
                    this.herosValiance();
                    break;

                case ForgeQuestWeapon.ArcanasConcerto:
                    this.arcanasConcerto();
                    break;

                case ForgeQuestWeapon.Elysium:
                    this.elysium();
                    break;

                case ForgeQuestWeapon.Acheron:
                    this.acheron();
                    break;

                case ForgeQuestWeapon.DauntLess:
                    this.dauntLess();
                    break;

                case ForgeQuestWeapon.Ravenous:
                    this.ravenous();
                    break;

                case ForgeQuestWeapon.All:
                    Core.logger("Selected to unlock all Forge Weapon Enhancements");
                    this.forgeWeaponEnhancement();
                    this.lacerate();
                    this.smite();
                    this.praxis();
                    this.acheron();
                    this.herosValiance();
                    this.elysium();
                    this.arcanasConcerto();
                    this.dauntLess();
                    this.ravenous();
                    break;
            }
        }

        if (selectedCapeEnhancements !== ForgeQuestCape.None) {
            if (selectedCapeEnhancements !== ForgeQuestCape.All)
                Core.logger(`Selected Forge Cape Enhancement: ${selectedCapeEnhancements}`);

            switch (selectedCapeEnhancements) {
                case ForgeQuestCape.ForgeCapeEnhancement:
                    this.forgeCapeEnhancement();
                    break;

                case ForgeQuestCape.Absolution:
                    this.absolution();
                    break;

                case ForgeQuestCape.Vainglory:
                    this.vainglory();
                    break;

                case ForgeQuestCape.Avarice:
                    this.avarice();
                    break;

                case ForgeQuestCape.Penitence:
                    this.penitence();
                    break;

                case ForgeQuestCape.Lament:
                    this.lament();
                    break;

                case ForgeQuestCape.All:
                    Core.logger("Selected to unlock all Forge Cape Enhancements");
                    this.forgeCapeEnhancement();
                    this.avarice(); //Calls on to the other functions internally
                    this.penitence();
                    this.lament();
                    break;
            }
        }

        if (selectedHelmEnhancements !== ForgeQuestHelm.None) {
            if (selectedHelmEnhancements !== ForgeQuestHelm.All)
                Core.logger(`Selected Forge Cape Enhancement: ${selectedHelmEnhancements}`);

            switch (selectedHelmEnhancements) {
                case ForgeQuestHelm.ForgeHelmEnhancement:
                    this.forgeHelmEnhancement();
                    break;

                case ForgeQuestHelm.Vim:
                    Core.logger("Selected to unlock Vim Helm Enh");
                    this.vim();
                    break;

                case ForgeQuestHelm.Examen:
                    Core.logger("Selected to unlock Examen Helm Enh");
                    this.examen();
                    break;

                case ForgeQuestHelm.Anima:
                    Core.logger("Selected to unlock Anima Helm Enh");
                    this.anima();
                    break;

                case ForgeQuestHelm.Pneuma:
                    Core.logger("Selected to unlock Pneuma Helm Enh");
                    this.pneuma();
                    break;

                case ForgeQuestHelm.All:
                    Core.logger("Selected to unlock all Forge Helm Enhancements");
                    this.forgeHelmEnhancement();
                    this.vim();
                    this.examen();
                    this.anima();
                    this.pneuma();
                    break;
            }
        }
    }

    forgeWeaponEnhancement(): void {
        if (Core.isCompletedBefore(8738))
            return;

        Core.logger("Unlocking Enhancement: Forge (Weapon)");

        Farm.experience(30);
        LOC.escherion();
        Farm.blacksmithingREP(
            4, 
            configOr("UseGold", false));

        Core.equipClass(ClassType.Solo);
        Core.ensureAccept(8738);

        Core.killEscherion("1st Lord Of Chaos Helm");
        Core.killVath("Chaos Dragonlord Helm");
        Core.killKitsune("Chaos Shogun Helmet");
        Core.huntMonster("wolfwing", "Wolfwing", "Wolfwing Mask", undefined, { isTemp: false });

        Core.ensureComplete(8738);
        Core.logger("Enhancement Unlocked: Forge (Weapon)");
    }

    lacerate(): void {
        if (Core.isCompletedBefore(8739))
            return;

        Core.logger("Unlocking Enhancement: Lacerate");

        if (!bot.quests.isUnlocked(8739)) {
            Core.equipClass(ClassType.Solo);
            if (!bot.quests.isUnlocked(92)) {
                if (!bot.quests.isUnlocked(91)) {
                    Core.ensureAccept(90);
                    Core.huntMonster("pirates", "Shark Bait", "Pirate Medallion", 12);
                    Core.ensureComplete(90);
                }
                Core.ensureAccept(91);
                Core.killMonster("greenguardwest", "West1", "Left", "Kittarian", "Kittarian's Wallet", 2);
                Core.killMonster("greenguardwest", "West9", "Left", "River Fishman", "River Fishman's Wallet", 2);
                Core.killMonster("greenguardwest", "West10", "Left", "Slime", "Slime-Soaked Wallet", 2);
                Core.killMonster("greenguardwest", "West3", "Left", "Frogzard", "Frogzard's Lint Hoard", 2);
                Core.killMonster("greenguardwest", "West12", "Up", "Big Bad Boar", "Big Bad Boar's Wallet");
                Core.ensureComplete(91);
            }
            if (!Story.questProgression(92)) {
                Core.ensureAccept(92);
                Core.huntMonster("greenguardwest", "Breken the Vile", "Breken the Vile's Sword", 1);
                Core.huntMonster("greenguardwest", "Ogug Stoneaxe", "Ogug Stoneaxe's Helmet", 1);
                Core.ensureComplete(92);
            }
        }

        Farm.experience(40);
        Farm.blacksmithingREP(
            5, 
            configOr("UseGold", false));

        Core.ensureAccept(8739);

        Core.equipClass(ClassType.Solo);
        Core.huntMonster("graveyard", "Big Jack Sprat", "Undead Plague Spear", undefined, { isTemp: false });
        Core.huntMonster("river", "Kuro", "Kuro's Wrath", undefined, { isTemp: false });

        if (!Core.checkInventory("Massive Horc Cleaver")) {
            Core.addDrop("Massive Horc Cleaver");
            Core.ensureAccept(279);
            Core.huntMonster("warhorc", "Horc Master", "Boss Prize");
            Core.ensureComplete(279);
        }
        if (!Core.checkInventory("Sword in the Stone")) {
            Core.addDrop("Sword in the Stone");
            Core.ensureAccept(316);
            Core.getMapItem(54, 7, "greenguardeast");
            Core.huntMonster("greenguardeast", "Spider", "Tiny Sword");
            Core.ensureComplete(316);
        }
        if (!Core.checkInventory("Forest Axe")) {
            Core.addDrop("Forest Axe");
            Core.ensureAccept(301);
            Core.getMapItem(55, 4, "farm");
            Core.huntMonster("farm", "Mosquito", "Mosquito Juice");
            Core.ensureComplete(301);
        }

        Farm.blackKnightOrb();

        Core.ensureComplete(8739);
        Core.logger("Enhancement Unlocked: Lacerate");
    }

    smite(): void {
        if (Core.isCompletedBefore(8740))
            return;

        Core.logger("Unlocking Enhancement: Smite");

        Farm.experience(60);
        Farm.blacksmithingREP(
            6, 
            configOr("UseGold", false));
        CoreSS.shadowfallRise();

        Core.ensureAccept(8740);
        Core.equipClass(ClassType.Solo);

        Core.huntMonster("shadowattack", "Death", "Death's Power", 3, { isTemp: false });
        Core.killEscherion("Chaotic Power", 7);
        Core.huntMonster("shadowrealmpast", "Pure Shadowscythe", "Empowered Essence", 50, { isTemp: false });
        Core.huntMonster("undergroundlabb", "Ultra Battle Gem", "Gem Power", 25, { isTemp: false });
        Adv.buyItem("alchemyacademy", 2116, "Power Tonic", 10);

        Core.ensureComplete(8740);
        Core.logger("Enhancement Unlocked: Smite");
    }

    praxis(): void {
        if (Core.isCompletedBefore(9171))
            return;

        Core.logger("Unlocking Enhancement: Praxis");

        Core.ensureAccept(9171);
        Farm.blacksmithingREP(
            6, 
            configOr("UseGold", false));

        Adv.buyItem("thespan", 439, "Thief of Hours Armor");
        Adv.buyItem("yulgar", 69, "Hashashin Armor");
        Adv.buyItem("dragonkoiz", 95, "Imperial Chunin Clone");

        Core.equipClass(ClassType.Solo);
        Core.huntMonster("ectocave", "Ektorax", "Dragon Rogue", undefined, { isTemp: false });
        YNR.yami(3);

        Core.equipClass(ClassType.Farm);
        Yokai.quests();
        Core.addDrop("Dragon Shinobi Token");

        Core.registerQuests(7924);
        while (!bot.shouldExit && !Core.checkInventory("Dragon Shinobi Token", 100))
            Core.huntMonster("shadowfortress", "1st Head of Orochi", "Perfect Orochi Scales", 10, { isTemp: false });
        Core.cancelRegisteredQuests();

        Adv.buyItem("shadowfortress", 1968, 59465, undefined, 6869);

        Core.ensureComplete(9171);
        Core.logger("Enhancement Unlocked: Praxis");
    }

    herosValiance(): void {
        if (Core.isCompletedBefore(8741))
            return;

        Core.logger("Unlocking Enhancement: Hero's Valiance");

        Farm.experience(100);
        Farm.blacksmithingREP(
            10, 
            configOr("UseGold", false));

        FCA.getFireChampsArmor();
        DOT.getDoT(undefined, false);
        ED.getSet(true, "Drakath the Eternal");
        if (!Core.checkInventory("Drakath the Eternal")) {
            Core.logger(
                "Hero Valiance cannot finish without Drakath the Eternal. Continuing with the remaining materials. Check the earlier Drakath prerequisite messages.\n"
                    + `\"Drakath Armor\": x${bot.inventory.getQuantity("Drakath Armor")}\n`
                    + `\"Dage's Scroll Fragment\" x${bot.inventory.getQuantity("Dage's Scroll Fragment")}\n`
                    + `\"Drakath the Eternal\" x${bot.inventory.getQuantity("Drakath the Eternal")}`);
        }

        LOO.getLoO();

        // Eternity Blade
        if (!Core.checkInventory(23689)) {
            Core.equipClass(ClassType.Solo);
            Core.addDrop("Eternity Blade");
            Core.ensureAccept(3485);
            Core.huntMonster("towerofdoom10", "Slugbutter", "Eternity Blade");
            Core.ensureComplete(3485);
            bot.wait.forPickup(23689);
        }

        Core.ensureAccept(8741);

        Seppy.gravelynsDoomFireToken();
        AP.getAP(false, true); //purely for the last quest "Sacred Magic: Eden"

        if (!Core.isCompletedBefore(7165)) {
            Core.logger(
                "Quest Progrestion not available For LoO (requires last quest to be complete and these are dailies)");
            return;
        }

        Adv.buyItem("darkthronehub", 1303, "ArchPaladin Armor");

        if (Core.checkInventory("Drakath the Eternal")) {
            Core.ensureComplete(8741);
            Core.logger("Enhancement Unlocked: Hero's Valiance");
        } else
            Core.logger(
                "Hero Valiance was not completed because Drakath the Eternal is missing. Check the earlier Drakath prerequisite messages before farming more daily fragments.");
    }

    arcanasConcerto(): void {
        if (Core.isCompletedBefore(8742))
            return;

        Core.logger("Unlocking Enhancement: Arcana's Concerto");

        Astravia.completeCoreAstravia();
        Farm.experience(100);
        Farm.blacksmithingREP(
            10, 
            configOr("UseGold", false));

        if (!Core.isCompletedBefore(8746)) {
            Core.logger("You must have faced Darkon the Conductor and done the weekly quest in order to unlock \"Arcana's Concerto\".");
            return;
        }

        // Farm everything required for Prince Darkon's Poleaxe.
        PDPPR.farmPreReqs(configOr("UseInsignOnArcanasConcerto", false));

        // Prince Darkon's Poleaxe is an actual requirement for Arcana's Concerto.
        // Do not claim Quest 8742 is complete until we have it.
        if (!Core.checkInventory("Prince Darkon's Poleaxe")) {
            Core.logger("\"Prince Darkon's Poleaxe\" is required to continue the Arcana's Concerto unlock. Farm/merge it at /ultradrago.");
            return;
        }

        // Darkon's Debris 2 (Reconstructed) is the other required item.
        if (!Core.checkInventory("Darkon's Debris 2 (Reconstructed)")) {
            if (!Core.checkInventory("Darkon's Debris 2 (Recovered)")) {
                Darkon.unfinishedMusicalScore(22);
                Farm.gold(50_000);
                Core.buyItem("theworld", 2141, "Gold Voucher 25k", 2);
                Core.buyItem("theworld", 2141, "Darkon's Debris 2 (Recovered)");
            }

            if (!Core.checkInventory("Darkon's Debris 2 (Recovered)")) {
                Core.logger("Darkon's Debris 2 (Recovered) could not be purchased. Stopping before farming the remaining materials.");
                return;
            }

            Darkon.banditsCorrespondence(22);
            Darkon.sukisPrestiege(22);
            Darkon.ancientRemnant(22);
            Darkon.wheelofFortune(22, 0);

            if (configOr("UseInsignOnArcanasConcerto", false)) {
                if (!Core.checkInventory("Darkon Insignia", 20)) {
                    Core.logger("x20 \"Darkon Insignia\" is required to continue the quest. Our bots cannot *currently* kill this mob; use Grim (different client) & @InsertNameHere's ultra bot.");
                    return;
                }

                Adv.buyItem(
                    "ultradarkon", 
                    2147, 
                    "Darkon's Debris 2 (Reconstructed)");
            } else {
                Core.logger("\"UseInsignOnArcanasConcerto\" is set to false. Please buy \"Darkon's Debris 2 (Reconstructed)\" from the shop manually, then complete QuestID 8742 yourself.");
                return;
            }
        }

        // Quest 8742 also requires these insignias.
        if (!Core.checkInventory("King Drago Insignia", 5)) {
            Core.logger("x5 \"King Drago Insignia\" is required to continue the quest. Our bots cannot *currently* kill this mob; use Grim (different client) & @InsertNameHere's ultra bot.");
            return;
        }

        if (!Core.checkInventory("Darkon Insignia", 5)) {
            Core.logger("x5 \"Darkon Insignia\" is required to continue the quest. Our bots cannot *currently* kill this mob; use Grim (different client) & @InsertNameHere's ultra bot.");
            return;
        }

        // Final sanity check: both actual Arcana's Concerto items must exist.
        if (!Core.checkInventory("Prince Darkon's Poleaxe") ||
            !Core.checkInventory("Darkon's Debris 2 (Reconstructed)")) {
            Core.logger("Arcana's Concerto requirements are not complete. \"Prince Darkon's Poleaxe\" and \"Darkon's Debris 2 (Reconstructed)\" are both required.");
            return;
        }

        if (configOr("UseInsignOnArcanasConcerto", false)) {
            Core.chainComplete(8742);
            Core.logger("Enhancement Unlocked: Arcana's Concerto");
        } else {
            Core.logger("\"UseInsignOnArcanasConcerto\" is set to false. Please complete QuestID 8742 manually.");
        }
    }

    acheron(): void {
        if (
            Core.isCompletedBefore(8820)
            || !Core.checkInventory([ 38566, 38567 ], undefined, { toInv: false })
        ) {
            Core.logger(
                Core.isCompletedBefore(8820)
                    ? "Skipped: \"Acheron\" (8820) is already completed."
                    : "Skipped: Required items (Dark Box, Dark Key) are missing from inventory.");

            Core.logger(`Dark Key: ${(Core.checkInventory(38567, undefined, { toInv: false }) ? "✔️" : "❌")}`);
            Core.logger(`Dark Box: ${(Core.checkInventory(38566, undefined, { toInv: false }) ? "✔️" : "❌")}`);
            return;
        }

        Core.logger("Unlocking Enhancement: Acheron");

        Farm.blacksmithingREP(
            8, 
            configOr("UseGold", false));
        Core.ensureAccept(8820);

        this.voidLodestone();

        SoW.tyndarius();

        // have the Dark Box and Dark Key mini-saga completed
        // Quest complete will require you to turn in the Power of Darkness,
        Core.buyItem(bot.map.name, 1380, "The Power of Darkness");

        //20 Dark Potions,
        Core.registerQuests(5710);
        while (!bot.shouldExit && !Core.checkInventory("Dark Potion", 20)) {
            if (!Core.isMember)
                Core.huntMonster("darkfortress", "Dark Elemental", "Dark Gem", undefined, { isTemp: false });
            else
                Core.huntMonster("ruins", "Dark Elemental", "Dark Gem", undefined, { isTemp: false });
        }
        Core.cancelRegisteredQuests();

        Adv.gearStore(undefined, { enhAfter: true });
        Core.equipClass(ClassType.Dodge);
        Core.huntMonster("tercessuinotlim", "Nulgath", "The Mortal Coil", undefined, { isTemp: false });
        Adv.gearStore(true, { enhAfter: true });
        Core.ensureComplete(8820);
        Core.logger("Enhancement Unlocked: Acheron");
    }

    elysium(): void {
        if (Core.isCompletedBefore(8821))
            return;

        Core.logger("Unlocking Enhancement: Elysium");

        Farm.blacksmithingREP(
            10, 
            configOr("UseGold", false));

        Core.ensureAccept(8821);
        CorNSOD.bonesVoidRealm(15);
        YNR.blademasterSwordScroll();
        NDW.ndwQuest([ "Archfiend Essence Fragment" ], 3);
        awescended.getAwe();
        if (!Core.checkInventory("The Divine Will")) {
            Core.logger(
                "\"Azalith\" is not Soloable, please go kill it otherwise for the Drop \"The Divine Will\", and return here and re-run the script.");
            return;
        }
        Core.ensureComplete(8821);
        Core.logger("Enhancement Unlocked: Elysium");
    }

    dauntLess(): void {
        if (Core.isCompletedBefore(9172))
            return;

        Core.logger("Unlocking Enhancement: Dauntless");

        Farm.blacksmithingREP(
            10, 
            configOr("UseGold", false));

        const DauntlessItems: string[] =
        [
            "ShadowLord's Helm",
            "Malgor the ShadowLord",
            "Malgor's ShadowFlame Blade",
            "Infernal Flame Pyromancer",
        ];
        Core.logger(
            `The only items the bot can get are: \"Malgor the ShadowLord\" and \"ShadowLord's Helm\". Unless \`UseInsignOnDaunt\` is enabled: ${configOr("UseInsignOnDaunt", false)}`);

        // Base Quest Req. for dauntless
        let Malgorinsig = 5;
        let AvatarTyndInsig = 10;

        Core.ensureAccept(9172);
        if (!Core.checkInventory(DauntlessItems)) {
            //Story
            SoW.completeCoreSoW();

            // Items: "ShadowLord's Helm", "Malgor the ShadowLord"
            MAS.getSet();

            //#region  [Prep] Malgor's ShadowFlame Blade
            if (!Core.checkInventory("Malgor's ShadowFlame Blade")) {
                Adv.gearStore(undefined, { enhAfter: true });
                Core.useBossClass();
                SOWM.elementalCore(20);
                Adv.gearStore(true, { enhAfter: true });
                Core.equipClass(ClassType.Solo);
                Core.huntMonster("shadowgrove", "Titan Shadow Dragonlord", "ShadowFlame Dragon Blade", undefined, { isTemp: false });
            }
            //#endregion

            //#region  [Prep] Infernal Flame Pyromancer
            if (!Core.checkInventory("Infernal Flame Pyromancer")) {
                SoW.tyndarius();

                Core.addDrop("Fire Avatar's Favor");
                Core.equipClass(ClassType.Farm);
                Core.farmingLogger("Fire Avatar's Favor", 75);
                Core.registerQuests(8244);
                while (!bot.shouldExit && !Core.checkInventory("Fire Avatar's Favor", 75)) {
                    Core.killMonster("fireavatar", "r4", "Right", "*", "Onslaught Defeated", 6);
                    Core.killMonster("fireavatar", "r6", "Left", "*", "Elemental Defeated", 6);

                    bot.wait.forPickup("Fire Avatar's Favor");
                }
                Core.cancelRegisteredQuests();
            }

            //#endregion

            // Recheck for both items, buy if "UseInsignOnDaunt" is true
            if (configOr("UseInsignOnDaunt", false)) {
                //ensure in inv
                Core.unbank("Malgor Insignia", "Avatar Tyndarius Insignia");

                if (
                    !Core.checkInventory("Malgor's ShadowFlame Blade")
                    && Core.checkInventory("Malgor Insignia", 20)
                )
                    Adv.buyItem("ultraspeaker", 2248, "Malgor's ShadowFlame Blade");
                else
                    Malgorinsig += 20;

                if (
                    !Core.checkInventory("Infernal Flame Pyromancer")
                    && Core.checkInventory("Avatar Tyndarius Insignia", 20)
                )
                    Adv.buyItem("fireavatar", 2038, "Infernal Flame Pyromancer");
                else
                    AvatarTyndInsig += 20;
            }

            // totals:
            // 25 Malgor Insignia
            // 30 Avatar Tyndarius Insignia
        }

        // Unlock Dauntless if items are owned.
        if (
            Core.checkInventory(DauntlessItems)
            && Core.checkInventory("Malgor Insignia", 5)
            && Core.checkInventory("Avatar Tyndarius Insignia", 10)
        ) {
            Core.chainComplete(9172);
            Core.logger("Enhancement Unlocked: Dauntless");
        } else {
            Core.logger("Items still needed(the bot cannot farm these):");
            for (const item of DauntlessItems.filter(item => !Core.checkInventory(item)))
                Core.logger(`Missing \"${item}\" x1`);

            Core.logger(`Missing \"Malgor Insignia\" x ${Malgorinsig}`);
            Core.logger(`Missing \"Avatar Tyndarius Insignia\" x ${AvatarTyndInsig}`);
            Core.abandonQuest(9172);
        }
    }

    ravenous(): void {
        if (Core.isCompletedBefore(9560))
            return;

        Core.logger("Unlocking Enhancement: Ravenous");

        Farm.experience();
        PFS.storyline();

        Adv.buyItem(bot.map.name, 2411, "Gluttonous Maw");
        if (Core.checkInventory("Gluttonous Maw"))
            Core.chainComplete(9560);
        else
            Core.logger(
                "Cannot complete Ravenous: Requires Gluttonous Maw, which needs 10 Roentgeniums of Nulgath (daily). Try again tomorrow.");
    }

    forgeCapeEnhancement(): void {
        if (Core.isCompletedBefore(8758))
            return;

        Core.logger("Unlocking Enhancement: Forge (Cape)");

        Farm.experience(30);
        LOC.kitsune();
        Farm.blacksmithingREP(4, configOr("UseGold", false));

        Core.equipClass(ClassType.Solo);
        Core.ensureAccept(8758);

        Core.huntMonster("lostruinswar", "Diabolical Warlord", "Prismatic Celestial Wings", undefined, { isTemp: false });
        Core.huntMonster("lostruins", "Infernal Warlord", "Broken Wings", undefined, { isTemp: false });
        Core.huntMonster("infernalspire", "Azkorath", "Shadow's Wings", undefined, { isTemp: false });
        Core.huntMonster("infernalspire", "Malxas", "Wings Of Destruction", undefined, { isTemp: false });

        Core.ensureComplete(8758);
        Core.logger(`Enhancement Unlocked: Forge (Cape)`);
    }

    absolution(): void {
        if (Core.isCompletedBefore(8743))
            return;

        Core.logger("Unlocking Enhancement: Absolution");

        Farm.experience(90);
        Farm.goodREP(10);
        Farm.blacksmithingREP(
            9, 
            configOr("UseGold", false));

        let SlimeSigil = 150;

        // Deduct points based on items in inventory
        const itemsToCheck = new Map<number, number>([
            [39091, 100], // Ascended Paladin
            [39094, 50], // Ascended Paladin Sword
        ]);

        // Deduct points for found items
        itemsToCheck.forEach((value, key) => SlimeSigil -= Core.checkInventory(key) ? value : 0);

        Core.equipClass(ClassType.Farm);
        bot.quests.updateQuest(5807);
        Core.killMonster("charredpath", "r5", "Left", "Plague Spreader", "Slimed Sigil", SlimeSigil, { isTemp: false });

        // Purchase items
        Adv.buyItem("therift", 1399, "Ascended Paladin");
        Adv.buyItem("therift", 1399, "Ascended Paladin Sword");

        Core.chainComplete(8743);
        Core.logger("Enhancement Unlocked: Absolution");
    }

    vainglory(): void {
        if (Core.isCompletedBefore(8744))
            return;

        this.absolution();

        Core.logger("Unlocking Enhancement: Vainglory");
        Core.ensureAccept(8744);

        Farm.blacksmithingREP(
            9, 
            configOr("UseGold", false));
        Core.equipClass(ClassType.Solo);

        if (Core.isMember) {
            if (!Core.checkInventory("Pauldron Relic")) {
                Farm.bladeofAweREP(10, false);
                Farm.experience(55);
                Core.buyItem("museum", 1130, "Armor of Awe Pass");
                Core.addDrop("Pauldron Fragment");
                Core.registerQuests(4162);
                Core.huntMonster("gravestrike", "Ultra Akriloth", "Pauldron Fragment", 15, { isTemp: false });
                bot.wait.forPickup("Pauldron Fragment");
                Core.cancelRegisteredQuests();

                Core.buyItem("museum", 1129, "Pauldron Relic");
            }
        } else
            Awe.getAweRelic("Pauldron", 4160, 15, 15, "gravestrike", "Ultra Akriloth");
        Awe.getAweRelic("Breastplate", 4163, 10, 10, "aqlesson", "Carnax");
        Awe.getAweRelic("Vambrace", 4166, 15, 15, "bloodtitan", "Ultra Blood Titan");
        Awe.getAweRelic("Gauntlet", 4169, 25, 5, "alteonbattle", "ULTRA Alteon");
        Awe.getAweRelic("Greaves", 4172, 10, 15, "bosschallenge", "Mutated Void Dragon");

        Core.ensureComplete(8744);
        Core.logger("Enhancement Unlocked: Vainglory");
    }

    avarice(): void {
        if (Core.isCompletedBefore(8745))
            return;

        this.vainglory();
        Core.logger("Unlocking Enhancement: Avarice");

        Farm.blacksmithingREP(
            9, 
            configOr("UseGold", false));

        Core.ensureAccept(8745);

        Circles.circlesWar();
        HOTLB.indulgence(50);
        HOTLB.penance(50);

        Core.ensureComplete(8745);
        Core.logger("Enhancement Unlocked: Avarice");
    }

    penitence(): void {
        if (Core.isCompletedBefore(8822))
            return;

        this.avarice();
        Core.logger("Unlocking Enhancement: Penitence");

        Farm.blacksmithingREP(
            9, 
            configOr("UseGold", false));

        Core.ensureAccept(8822);
        Core.equipClass(ClassType.Solo);

        while (!bot.shouldExit && !Core.checkInventory("Night Mare Scythe")) {
            Core.addDrop("Night Mare Scythe");
            Core.ensureAccept(3270);
            Core.killMonster("doomvault", "r5", "Left", "Binky", "Yulgar's Lost Scythe");
            Core.ensureComplete(3270);
        }
        Core.equipClass(ClassType.Dodge);
        Core.huntMonster("frozenlair", "Legion Lich Lord", "Sapphire Orb", 100, { isTemp: false });
        Core.huntMonster("icewing", "Warlord Icewing", "Boreal Cavalier Bardiche", undefined, { isTemp: false });

        //why the fuck was the class buffed!?
        const usethis = bot
            .inventory.items.concat(bot.bank.items)
            .find(n =>
                n.name === "Yami no Ronin" || n.name.startsWith("Chaos Slayer"));

        if (usethis != null) {
            Core.equip(usethis.id);
            Core.equip(...Core.farmGear);
            if (usethis.name === "Yami no Ronin")
                bot.skills.startAdvanced("1 | 4");
        } else
            Core.equipClass(ClassType.Dodge);
        Core.huntMonster("underlair", "ArchFiend DragonLord", "Void Scale", 13, { isTemp: false });

        Core.ensureComplete(8822);
        Core.logger("Enhancement Unlocked: Penitence");
    }

    lament(): void {
        if (Core.isCompletedBefore(8823))
            return;

        this.penitence();
        Core.logger("Unlocking Enhancement: Lament");

        Farm.blacksmithingREP(
            9, 
            configOr("UseGold", false));

        Core.equipClass(ClassType.Solo);
        Core.ensureAccept(8823);

        Core.huntMonster("sepulchurebattle", "ULTRA Sepulchure", "Doom Heart", undefined, { isTemp: false });
        if (!Core.checkInventory("Heart of the Sun"))
            TSS.storyLine(true); //sun heart thing
        Core.huntMonster("ashfallcamp", "Smoldur", "Flame Heart", 10, { isTemp: false });
        DD.sloth();
        Adv.gearStore(undefined, { enhAfter: true });
        DD.hazMatSuit();
        Core.huntMonster("sloth", "Mutated Plague", "Bloodless Heart", 3, { isTemp: false });
        Adv.gearStore(true, { enhAfter: true });

        Core.ensureComplete(8823);
        Core.logger("Enhancement Unlocked: Lament");
    }

    forgeHelmEnhancement(): void {
        if (Core.isCompletedBefore(8828))
            return;

        Core.logger("Unlocking Enhancement: Forge (Helm)");
        Farm.experience(30);
        Farm.blacksmithingREP(
            4, 
            configOr("UseGold", false));

        Core.equipClass(ClassType.Solo);
        Core.ensureAccept(8828);

        Core.killEscherion("1st Lord Of Chaos Staff");
        Core.killVath("Chaos Dragonlord Axe");
        Core.killKitsune("Hanzamune Dragon Koi Blade");
        Core.huntMonster("wolfwing", "Wolfwing", "Wrath of the Werepyre", undefined, { isTemp: false });

        Core.ensureComplete(8828);
        Core.logger("Enhancement Unlocked: Forge (Helm)");
    }

    vim(): void {
        if (Core.isCompletedBefore(8824))
            return;

        this.forgeHelmEnhancement();
        Core.logger("Unlocking Enhancement: Vim");

        Farm.blacksmithingREP(
            7, 
            configOr("UseGold", false));
        Adv.gearStore(undefined, { enhAfter: true });
        Core.ensureAccept(8824);

        Core.buyItem("Classhalla", 172, "Rogue");
        Adv.rankUpClass("Rogue");
        Adv.gearStore(true, { enhAfter: true });

        Core.equipClass(ClassType.Farm);
        Core.killMonster("Towerofdoom10", "r8", "Left", "*", "Ethereal Essence", 250, { isTemp: false });

        Core.ensureComplete(8824);
        if (configOr("SellQuestClass", false))
            Core.sellItem("Rogue");
        Core.logger("Enhancement Unlocked: Vim");
    }

    examen(): void {
        if (Core.isCompletedBefore(8825))
            return;

        this.vim();
        Core.logger("Unlocking Enhancement: Examen");

        Adv.gearStore(undefined, { enhAfter: true });
        Core.ensureAccept(8825);
        Core.buyItem("Classhalla", 176, "Healer");
        Adv.rankUpClass("Healer");
        Adv.gearStore(true, { enhAfter: true });

        Core.equipClass(ClassType.Farm);
        Core.killMonster("Towerofdoom10", "r8", "Left", "*", "Ethereal Essence", 250, { isTemp: false });

        Core.ensureComplete(8825);
        if (configOr("SellQuestClass", false))
            Core.sellItem("Healer");
        Core.logger("Enhancement Unlocked: Examen");
    }

    anima(): void {
        if (Core.isCompletedBefore(8826))
            return;

        this.examen();
        Core.logger("Unlocking Enhancement: Anima");

        Adv.gearStore(undefined, { enhAfter: true });
        Core.ensureAccept(8826);

        Core.buyItem("Classhalla", 170, "Warrior");
        Adv.rankUpClass("Warrior");
        Adv.gearStore(true, { enhAfter: true });

        Core.equipClass(ClassType.Farm);
        Core.killMonster("Towerofdoom10", "r8", "Left", "*", "Ethereal Essence", 650, { isTemp: false });

        Core.ensureComplete(8826);
        if (configOr("SellQuestClass", false))
            Core.sellItem("Warrior");
        Core.logger("Enhancement Unlocked: Anima");
    }

    pneuma(): void {
        if (Core.isCompletedBefore(8827))
            return;

        this.anima();
        Core.logger("Unlocking Enhancement: Pneuma");

        Adv.gearStore(undefined, { enhAfter: true });
        Core.ensureAccept(8827);

        Core.buyItem("Classhalla", 174, "Mage");
        Adv.rankUpClass("Mage");
        Adv.gearStore(true, { enhAfter: true });

        Core.equipClass(ClassType.Farm);
        Core.killMonster("Towerofdoom10", "r8", "Left", "*", "Ethereal Essence", 650, { isTemp: false });

        Core.ensureComplete(8827);
        if (configOr("SellQuestClass", false))
            Core.sellItem("Mage");
        Core.logger("Enhancement Unlocked: Pneuma");
    }

    voidLodestone(): void {
        if (Core.checkInventory("Void Lodestone"))
            return;

        // Arcane Lodestone
        if (!Core.checkInventory("Arcane Lodestone")) {
            //(Reward from the 'Open Ebony Chest' quest
            //Requires: ???(38565) to acess quest
            this.theDarkBox(38565);

            if (Core.checkInventory(38565)) {
                Core.ensureAccept(5723);
                Core.huntMonster("dreadfire", "Stray Mana", "Bronze Key", undefined, { isTemp: false });
                Core.huntMonster("dreadfire", "Living Brimstone", "Silver Key", undefined, { isTemp: false });
                Core.logger("Going to your house to load the shop.\n" + "[there may be a delay]");
                Core.sendPackets(`%xt%zm%house%1%${bot.player.username}%`);
                Core.sleep(5000);
                Core.buyItem(bot.map.name, 336, "Golden Key");
                Core.ensureComplete(5723);
            } else {
                Core.logger("Cannot Accept Quest Without Item \"???\"");
                return;
            }
        }

        // Mercury Elixir
        if (!Core.checkInventory("Mercury Elixir")) {
            //Reward from the 'Mercury Elixir' quest
            Core.ensureAccept(5757);
            Core.huntMonster("Battleunderb", "The Lost", "Mercury Elixir");
            Core.ensureComplete(5757);
        }

        Core.buyItem("doomwood", 1381, "Void Lodestone");
    }

    theDarkBox(itemID: number, quant = 1): void {
        const Reward = Core.ensureLoad(5710).rewards.find(x => x.id === itemID);
        if (Reward == null) {
            Core.logger(`ERROR: itemID ${itemID} was not found in quest 5710`);
            return;
        }

        Core.addDrop("Dark Potion", Reward.name);

        Daily.monthlyTreasureChestKeys();
        if (!Core.checkInventory([ "Dark Box", "Dark Key" ])) {
            Core.logger("Dark Box & Key Not Found, Cannot Continue with Enh");
            return;
        }

        Core.logger("Pray to RNGsus for your item");
        while (!bot.shouldExit && !Core.checkInventory(Reward.id, quant)) {
            Core.ensureAccept(5710);
            if (Core.isMember)
                Core.huntMonster("ruins", "Dark Elemental", "Dark Gem", undefined, { isTemp: false });
            else
                Core.huntMonster("darkfortress", "Dark Elemental", "Dark Gem", undefined, { isTemp: false });
            Core.ensureComplete(5710);
            bot.wait.forPickup(Reward.id);
        }
    }
}

export enum ForgeQuestWeapon {
    ForgeWeaponEnhancement = "ForgeWeaponEnhancement",
    Lacerate = "Lacerate",
    Smite = "Smite",
    Praxis = "Praxis",
    Acheron = "Acheron",
    HerosValiance = "HerosValiance",
    Elysium = "Elysium",
    ArcanasConcerto = "ArcanasConcerto",
    DauntLess = "DauntLess",
    Ravenous = "Ravenous",
    None = "None",
    All = "All",
}

export enum ForgeQuestCape {
    ForgeCapeEnhancement = "ForgeCapeEnhancement",
    Absolution = "Absolution",
    Vainglory = "Vainglory",
    Avarice = "Avarice",
    Penitence = "Penitence",
    Lament = "Lament",
    None = "None",
    All = "All",
}

export enum ForgeQuestHelm {
    ForgeHelmEnhancement = "ForgeHelmEnhancement",
    Vim = "Vim",
    Examen = "Examen",
    Anima = "Anima",
    Pneuma = "Pneuma",
    None = "None",
    All = "All",
}

export const scriptOptions = defineScriptOptions("Forge Ehn Unlocks", [
    Core.skipOptions,
    option.choice("ForgeQuestWeapon", "Weapon Enhancement", "Forge Quests to unlock Weapon Enhancement, change to none to unselect", choices(ForgeQuestWeapon), ForgeQuestWeapon.None),
    option.choice("ForgeQuestCape", "Cape Enhancement", "Forge Quests to unlock Cape Enhancement, change to none to unselect", choices(ForgeQuestCape), ForgeQuestCape.None),
    option.choice("ForgeQuestHelm", "Helm Enhancement", "Forge Quests to unlock Helm Enhancement, change to none to unselect", choices(ForgeQuestHelm), ForgeQuestHelm.None),
    option.bool("UseGold", "Use Gold", "Speed the BlacksmithingREP grind up with Gold?", false),
    option.bool("SellQuestClass", "Sell quest classes", "sell the classes backa after the Anima, Pneuma, Examen, and Vim quests", false),
    option.bool("UseInsignOnDaunt", "Use Insignia for dauntless", "Use your Insignia to buy the `Malgor's ShadowFlame Blade`[Malgor] & `Infernal Flame Pyromancer`[Avatar Tyndarius]", false),
    option.bool("UseInsignOnArcanasConcerto", "Use Insignias for Arcana's Concerto", "Use insignias to buy Darkon's Debris 2 (Reconstructed), Prince Darkon's Poleaxe, and complete the quest.", false),
], true);

export default function main(): void {
        Core.setOptions();

        new UnlockForgeEnhancements().forgeUnlocks();

        Core.setOptions(false);
}
