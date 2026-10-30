/*
name: ArchPaladin (Class)
description: This bot farms the ArchPaladin class for you.
tags: class, good, tank, hp, templar, support, survivability, figher, healer, undead
*/
// Port of Scripts/Good/ArchPaladin.cs.

import { bot } from "turu";
import { CoreAdvanced } from "../core/CoreAdvanced";
import { ClassType, Core } from "../core/CoreBots";
import { CoreFarms } from "../core/CoreFarms";
import { CoreStory } from "../core/CoreStory";
import { XansLair } from "../story/XansLair";
import { Paladin } from "./Paladin";
import { CoreBLOD } from "./blod/CoreBLOD";

const Farm = new CoreFarms();
const Adv = new CoreAdvanced();
const Story = new CoreStory();
const BLOD = new CoreBLOD();
const Pal = new Paladin();
const Xan = new XansLair();

export class ArchPaladin {
    getAP(rankUpClass = true, ArmorOnly = false): void {
        if (Core.checkInventory(36920)) {
            if (rankUpClass)
                Adv.rankUpClass("ArchPaladin");
            return;
        }
        Story.preLoad(this.getAP);

        Farm.goodREP();
        if (!Core.isMember && !bot.quests.isUnlocked(5467)) {
            // A Strong Base
            if (!Story.questProgression(5463)) {
                Story.buyQuest(5463, "temple", 288, "Stone Paladin Armor");
                Farm.gold(500000);
                Story.buyQuest(5463, "darkthronehub", 1308, "Exalted Paladin Seal");
            }

            // Proof of Valor
            if (!Story.questProgression(5464)) {
                Core.ensureAccept(5464);
                BLOD.unlockMineCrafting();
                Farm.battleUnderB("Undead Energy", 1000);
                Core.equipClass(ClassType.Dodge);
                Core.killMonster("doomvault", "r5", "Left", "Binky", "Binky's Uni-horn", 1, { isTemp: false, publicRoom: true });
                Core.equipClass(ClassType.Solo);
                Core.huntMonster("banished", "Desterrat Moya", "Desterrat Moya Tentacle", 1, { publicRoom: true });
                Core.killMonster("dreadfight", "Enter", "Spawn", "*", "Dreadhaven Helm", 1, { publicRoom: true });
                Adv.gearStore(false, { enhAfter: true });
                Core.killDoomKitten("DoomKitten Claw", 1, { isTemp: true });
                Adv.gearStore(true, { enhAfter: true });
                Core.equipClass(ClassType.Solo);
                Core.huntMonster("vordredboss", "Vordred", "Vordred's Skull", 1, { isTemp: false, publicRoom: true });
                Core.ensureComplete(5464);
            }

            // Mastering the Arcane
            if (!Story.questProgression(5465)) {
                Core.ensureAccept(5465);
                Core.equipClass(ClassType.Solo);

                Core.killMonster("xantown", "r8", "Left", "Xan", "Pyromancer Artifact", 1, { isTemp: false }); if (bot.map.name === "xantown")
                    Core.jump("r12", "Left"); // map is aggro af this is a safe cell.

                Core.huntMonster("dragonheart", "Proto-Air Dracolich", "Zephyrus Manifesto", 1, { isTemp: false });
                Core.huntMonster("northstar", "Karok The Fallen", "Karok's Glaceran Gem", 1, { isTemp: false, publicRoom: true });
                Core.equipClass(ClassType.Farm);
                Core.huntMonster("thirdspell", "Mana Phoenix", "Nightmare Kibble", 200, { isTemp: false });
                Core.huntMonster("thunderfang", "Lightning Ball", "Condensed Energy", 1, { isTemp: false });
                Core.killMonster("downward", "r11", "Right", "Crystal Mana Construct", "Crystallized Mana Catalyst", 1, { isTemp: false });
                Core.huntMonster("farm", "Treeant", "Just the Perfect Stick", 1, { isTemp: false });
                Core.ensureComplete(5465);
            }

            // For Those Who Have Fallen
            if (!Story.questProgression(5466)) {
                Core.ensureAccept(5466);
                Core.buyItem("castle", 88, "Holy Hand Grenade");
                Core.equipClass(ClassType.Farm);
                Core.huntMonster("manor", "Bird of Paradise", "Feather of Paradise", 20, { isTemp: false });
                Core.killMonster("doomwood", "r6", "Right", "*", "Shoelace of a Fallen Paladin", 77, { isTemp: false });
                Core.equipClass(ClassType.Solo);
                Core.huntMonster("fotia", "Amia the Cult Leader", "Eternity Flame", 1, { isTemp: false });
                Core.ensureComplete(5466);
            }
        } else if (Core.isMember && !bot.quests.isUnlocked(5467)) {
            if (!Core.checkInventory("Silver Paladin"))
                Pal.getPaladin(false);
            // Legendary|Silver Paladin's Vow
            Story.chainQuest(Core.checkInventory("Silver Paladin") ? 5475 : 5471);
            // Legendary|Silver Paladin's Oath
            Story.chainQuest(Core.checkInventory("Silver Paladin") ? 5476 : 5472);
            // Legendary|Silver Paladin's Pledge
            Story.chainQuest(Core.checkInventory("Silver Paladin") ? 5477 : 5473);
            // Aptitude Test
            if (!Story.questProgression(Core.checkInventory("Silver Paladin") ? 5478 : 5474)) {
                Core.ensureAccept(Core.checkInventory("Silver Paladin") ? 5478 : 5474);
                Core.equipClass(ClassType.Dodge);
                Core.huntMonster("bosschallenge", "Colossal Primarch", "Primarch's Hilt", 1, { isTemp: false, publicRoom: true }); Farm.gold(500000);
                Core.buyItem("darkthronehub", 1308, "Exalted Paladin Seal");
                Core.equipClass(ClassType.Solo);
                Core.huntMonster("timevoid", "Unending Avatar", "Condensed Mana", 1, { isTemp: false, publicRoom: true });
                Core.ensureComplete(Core.checkInventory("Silver Paladin") ? 5478 : 5474);
            }
        }

        // Commandment
        if (!Story.questProgression(5467)) {
            Core.ensureAccept(5467);
            Core.equipClass(ClassType.Solo);
            Core.huntMonster("brightfall", "Painadin Overlord", "Skill Observed", 1, { isTemp: false, publicRoom: true });
            Core.huntMonster("citadel", "Grand Inquisitor", "Spirit of Vindication", 1, { isTemp: false });
            Core.huntMonster("alliance", "Good Lieutenant", "Radiant Blade Enhancement", 1, { isTemp: false }); Core.ensureComplete(5467);
        }

        // Hymn of Light
        if (!Story.questProgression(5468)) {
            Core.ensureAccept(5468);
            Core.equipClass(ClassType.Solo);
            Core.huntMonster("poisonforest", "Xavier Lionfang", "Divine Elixir", 1, { isTemp: false });
            Core.equipClass(ClassType.Dodge);
            Core.huntMonster("ultraalteon", "Ultra Chaos Alteon", "Prayer of Salvation", 1, { isTemp: false, publicRoom: true });
            Core.equipClass(ClassType.Farm);
            Core.huntMonster("newfinale", "Alliance Healer", "Acolyte's Braille", 1, { isTemp: false });
            Core.huntMonster("skytower", "Dove", "Innocence", 25, { isTemp: false });
            Core.ensureComplete(5468);
        }

        // Righteous Seal
        if (!Story.questProgression(5469)) {
            Core.addDrop("Scroll of Ethereal Slumber", "Fists of Fire", "Holy Magic Attunement", "Ring of Mana Transposition");
            Core.ensureAccept(5469);
            Xan.doAll();
            Core.equipClass(ClassType.Solo);
            Core.huntMonster("xancave", "Shurpu Ring Guardian", "Fists of Fire", 1, { isTemp: false, publicRoom: true });
            if (!Core.checkInventory("Scroll of Ethereal Slumber")) {
                if (!Core.checkInventory("Archmage Ink")) {
                    Farm.spellCraftingREP(8);
                    if (Core.checkInventory("Mystic Shards", 2))
                        Core.buyItem("spellcraft", 549, "Archmage Ink", 1);
                    else {
                        if (!Core.checkInventory("Arcane Quill")) {
                            Farm.gold(100000);
                            Core.buyItem("spellcraft", 693, "Gold Voucher 100k");
                            Core.buyItem("spellcraft", 693, 17391, 1, 7685);
                        }
                        Core.buyItem("spellcraft", 622, "Archmage Ink", 1);
                    }
                }
                if (!Core.checkInventory("Archmage Ink")) {
                    Core.huntMonster("underworld", "Skull Warrior", "Mystic Shards", 2, { isTemp: false });
                    Core.buyItem("dragonrune", 549, "Archmage Ink", 1);
                }
                Core.chainComplete(2347);
                bot.wait.forPickup("Scroll of Ethereal Slumber");
                Core.sellItem("Archmage Ink", 1, { all: true });
                Core.sellItem("Mystic Shards", 1, { all: true });
            }
            Core.equipClass(ClassType.Farm);
            Core.huntMonster("onslaughttower", "Golden Caster", "Holy Magic Attunement", 1, { isTemp: false });
            Core.equipClass(ClassType.Solo);
            Core.huntMonster("palace", "Pettivox", "Ring of Mana Transposition", 1, { isTemp: false });
            Core.ensureComplete(5469);
            Core.sellItem("Scroll of Ethereal Slumber", 1, { all: true });
        }

        // Sacred Magic: Eden
        if (!Story.questProgression(5470)) {
            Core.ensureAccept(5470);
            Core.equipClass(ClassType.Solo);
            Core.huntMonster("seraph", "Adventus", "Sacred Tome", 1, { isTemp: false });
            Core.equipClass(ClassType.Farm);
            Core.huntMonster("marsh", "Marsh Tree", "Paladaffodil", 25, { isTemp: false });
            Core.killMonster("wanders", "r5", "Left", "Lotus Spider", "Spirit Lotus", 25, { isTemp: false });
            Core.huntMonster("bloodtusk", "Trollola Plant", "Radiant Magnolia", 25, { isTemp: false });
            Core.huntMonster("gaiazor", "Wisterrora", "Cyanoblossom", 25, { isTemp: false });
            Core.equipClass(ClassType.Solo);
            Core.huntMonster("gaiazor", "Nevanna", "BrightOak Forest Sapling", 1, { isTemp: false });
            Core.huntMonster("infernalspire", "Malxas", "Forbidden Demon Seal", 1, { isTemp: false });
            Core.ensureComplete(5470);
        }

        if (ArmorOnly)
            return;

        Core.buyItem("darkthronehub", 1303, 36920, 1, 21833);
        if (rankUpClass) {
            Adv.gearStore(false, { enhAfter: true });
            Core.equip("ArchPaladin");
            Adv.rankUpClass("ArchPaladin");
            Adv.gearStore(true, { enhAfter: true });
        }
    }
}

export default function main(): void {
        Core.setOptions();

        new ArchPaladin().getAP();

        Core.setOptions(false);
}
