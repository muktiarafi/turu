/*
name: Awe-scention
description: This bot will do the entire quest line and it's prerequisites in order to get you the Awescended Gear
tags: awe, scension, awescended, omni, gear
*/
// Port of Scripts/Good/GearOfAwe/Awescended.cs.

import { BoostType, bot } from "turu";
import { DrakathArmorBot } from "../../chaos/DrakathsArmor";
import { CoreAdvanced } from "../../core/CoreAdvanced";
import { ClassType, Core } from "../../core/CoreBots";
import { CoreFarms } from "../../core/CoreFarms";
import { CoreStory } from "../../core/CoreStory";
import { ArchDoomKnight } from "../../evil/ADK";
import { SepulchuresOriginalHelm } from "../../evil/SepulchuresOriginalHelm";
import { GetSDD } from "../../other/ShadowDragonDefender";
import { FandH } from "../../other/weapons/FortitudeAndHubris";
import { SRoD } from "../../other/weapons/ShadowReaperOfDoom";
import { DjinnGateStory } from "../../story/DjinnGate";
import { J6Saga } from "../../story/J6Saga";
import { Bamboozle } from "../../story/nation/Bamboozle";
import { SEP } from "../SilverExaltedPaladin";
import { ArmorOfAwe } from "./ArmorOfAwe";
import { HelmOfAwe } from "./HelmOfAwe";

const Farm = new CoreFarms();
const Story = new CoreStory();
const Adv = new CoreAdvanced();
const AweArmor = new ArmorOfAwe();
const Helm = new HelmOfAwe();
const Pal = new SEP();
const FaH = new FandH();
const sRoD = new SRoD();
const SDD = new GetSDD();
const Armor = new DrakathArmorBot();
const Seppy = new SepulchuresOriginalHelm();
const ADK = new ArchDoomKnight();
const J6 = new J6Saga();
const Bam = new Bamboozle();
const Djinn = new DjinnGateStory();

export class Awescended {
    getAwe(): void {
        if (!Core.isCompletedBefore(8042)) {
            Story.preLoad(this.getAwe);

            //The Dawn of Lore
            if (!Story.questProgression(8035)) {
                Farm.experience(25);
                Core.ensureAccept(8035);
                Core.killMonster("uppercity", "r3", "Left", "Chaos Egg", "Fossilized Egg Yolk", 12);
                bot.quests.updateQuest(567);
                Core.killMonster("lycanwar", "Boss", "Left", "Edvard", "Stone Mask");
                Core.huntMonster("odokuro", "O-dokuro", "Ancient Bone", 6);
                Core.huntMonster("cornelis", "Stone Golem", "Block Head", 1, { isTemp: false });
                Core.ensureComplete(8035);
            }

            //Mystical Magics
            if (!Story.questProgression(8036)) {
                Core.ensureAccept(8036);
                Core.killMonster("deathsrealm", "Frame3", "Down", "Undead Mage", "Enchanted Manuscript", 8);
                Core.equipCached();

                Core.killMonster("citadel", "m14", "Left", "Grand Inquisitor", "Rite of Renewal");
                Core.killMonster("marsh", "Forest3", "Left", "Dark Witch", "Coven's Sigil");
                Core.killMonster("gilead", "r4", "Left", "*", "Spell Stone", 10);
                Core.ensureComplete(8036);
            }

            //Science Rules
            if (!Story.questProgression(8037)) {
                Core.ensureAccept(8037);
                Core.join("thunderfang", "r2", "Left");
                Core.killMonster("thunderfang", "r2", "Left", "Energy Elemental", "Supercharged Gem", 8);
                Core.huntMonster("lab", "Frank", "Lightning Capacitor", 5);
                Core.killMonster("boxes", "Boss", "Center", "Sneeviltron", "Wooden Control Panel");
                Core.killMonster("mqlesson", "Boss", "Left", "Dragonoid", "Dragonoid Core");
                Core.ensureComplete(8037);
            }

            //War: What is it Good For?
            if (!Story.questProgression(8038)) {
                Core.ensureAccept(8038);
                Core.killMonster("yokaiwar", "Boss", "Left", "O-dokuro's Head", "O-dokuro's Tooth", 1, { isTemp: false });
                Core.killMonster("wardwarf", "r4", "Left", "D'wain Jonsen", "D'wain Jonsen's Stinger");
                Core.killMonster("mythsongwar", "War2", "Left", "*", "Music Pirate's Instrument of War", 6);
                bot.quests.updateQuest(1170);
                Core.killMonster("shadowfallwar", "Inside", "Right", "Noxus", "Noxus' Necromancy Robe");
                Core.ensureComplete(8038);
            }

            //Which Came First, the Dragon or the Dragonoid?
            if (!Story.questProgression(8039)) {
                Core.ensureAccept(8039);
                Core.huntMonster("crashsite", "Barrier Bot", "Dwakel Shield Generator", 2);
                Core.huntMonster("crashsite", "Dwakel Warrior", "Dwakel Weapon", 10);
                Core.huntMonster("crashsite", "Mithril Man", "Mithril Man's Power Armor", 1);
                Core.huntMonster("crashsite", "ProtoSartorium", "Protosartorium's Power Armor", 1);
                Core.ensureComplete(8039);
            }

            //Artifact Hunting
            if (!Story.questProgression(8040)) {
                Farm.experience(50);
                Core.equipClass(ClassType.Solo);
                Core.ensureAccept(8040);
                Core.killMonster("ledgermayne", "Boss", "Left", "Ledgermayne", "The Supreme Arcane Staff", 1, { isTemp: false }); 
                Adv.buyItem("doomwood", 276, "Blinding Light of Destiny Handle");
                sRoD.shadowReaperOfDoom();
                Farm.bladeofAweREP(10, true);
                Core.equipClass(ClassType.Farm);
                Core.killMonster("cornelis", "Side1", "Left", "Gargoyle", "Mirror Shield Fragment", 50, { isTemp: false });
                Core.ensureComplete(8040);
            }

            //What Does Lore Have in Store?
            if (!Story.questProgression(8041)) {
                Farm.experience(75);
                Core.ensureAccept(8041);
                if (!Core.checkInventory("Vaden's Helm")) {
                    Core.equipClass(ClassType.Solo);
                    Core.killMonster("bonecastlec", "r25", "Bottom", "Vaden", "Vaden Helm Token", 333, { isTemp: false });
                    while (!Core.checkInventory("Vaden's Helm")) {
                        Core.buyItem("bonecastlec", 1242, "Vaden's Helm", 1, 3740);
                        Core.sleep(500);
                    }
                }
                Core.equipClass(ClassType.Farm);
                this.armorOfZular();
                FaH.fortitudeAndHubris();
                SDD.shadowDragonDefender();
                Pal.silverExaltedPaladin();
                Bam.bamboozleQuest();
                J6.j6();

                // "Valoth's Cannon of Doom"
                if (!Core.checkInventory(60685)) {
                    Core.addDrop(60685);

                    Core.ensureAccept(8043);
                    Adv.buyItem("crashruins", 1212, "Valoth's Broken Cannon");
                    Adv.buyItem("hyperspace", 603, "Peanut");
                    if (!Core.checkInventory("Floozer")) {
                        Core.farmingLogger("Floozer");
                        Core.addDrop("Floozer");
                        Core.ensureAccept(7290);
                        if (!Core.checkInventory("Rainbow Moonstone")) {
                            Core.addDrop("Rainbow Moonstone");
                            Core.ensureAccept(7291);
                            Core.huntMonster("earthstorm", "Sapphire Golem", "Chip of Sapphire");
                            Core.huntMonster("earthstorm", "Ruby Golem", "Chip of Ruby");
                            Core.huntMonster("earthstorm", "Emerald Golem", "Chip of Emerald");
                            Core.huntMonster("earthstorm", "Diamond Golem", "Chip of Diamond");
                            Core.ensureComplete(7291);
                            bot.wait.forDrop("Rainbow Moonstone");
                            bot.wait.forPickup("Rainbow Moonstone");
                        }
                        Core.ensureComplete(7290);
                        bot.wait.forDrop("Floozer");
                        bot.wait.forPickup("Floozer");
                    }
                    Core.ensureComplete(8043);
                    bot.wait.forDrop("Valoth's Cannon of Doom");
                    bot.wait.forPickup("Valoth's Cannon of Doom");
                }
                Core.ensureComplete(8041);
            }

            //Awe-scention
            if (!Story.questProgression(8042)) {
                Farm.experience(100);
                Core.ensureAccept(8042);
                AweArmor.getArmor();
                Helm.getHoA();
                Seppy.doAll();
                Core.toBank(...Seppy.gravelynsDoomFireTokenItems);
                ADK.doAll(true);
                Core.unbank("Arch DoomKnight");
                Armor.drakathOriginalArmor();

                // Relogin triggered by Drakath
                Core.ensureAccept(8042);
                Core.huntMonster("ectocave", "Ektorax", "Bin Jett's Salvaged Armor Part", 50, { isTemp: false });

                Core.unbank(
                    `GOLD Boost! (60 min)`, 
                    "Doom GOLD Boost! (60 min)", 
                    "GOLD Boost! (20 min)");
                bot.boosts.setUses(BoostType.Gold, true);

                Core.ensureComplete(8042);
                bot.boosts.setUses(BoostType.Gold, false);
            }
        }


        if (Core.checkInventory([
                    "Awescended",
                    "Awescended Omni Armblades",
                    "Awescended Omni Cowl",
                    "Awescended Omni Wings",
                ]))
            return;

        Core.buyItem("museum", 1994, "Awescended");
        Core.buyItem("museum", 1994, "Awescended Omni Armblades");
        Core.buyItem("museum", 1994, "Awescended Omni Cowl");
        Core.buyItem("museum", 1994, "Awescended Omni Wings");
        Core.logger("Awescended Set Bought, Congratulations!");
    }

    armorOfZular(): void {
        if (Core.checkInventory("Armor of Zular"))
            return;

        Core.addDrop("Armor of Zular", "Djinn's Essence");
        Core.equipClass(ClassType.Farm);
        Djinn.djinnGate();
        Core.unbank("Armor of Zular");
    }
}

export default function main(): void {
        Core.setOptions();

        new Awescended().getAwe();

        Core.setOptions(false);
}
