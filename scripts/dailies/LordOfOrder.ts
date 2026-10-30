/*
name: Lord of Order (Daily)
description: This bot will do the dailies for the Lord of Order, after that is done it will get the remainder rewards
tags: daily, lord, order, LOO, mirror, realm, support, xing, xang, kitsune, alteon, vath, wolfwing, ledgermayne, khasaanda
*/
// Port of Scripts/Dailies/LordOfOrder.cs.

import { bot } from "turu";
import { CoreAdvanced } from "../core/CoreAdvanced";
import { ClassType, Core } from "../core/CoreBots";
import { CoreFarms } from "../core/CoreFarms";
import { DragonFableOrigins } from "../story/DragonFableOrigins";
import { CitadelRuins } from "../story/nation/CitadelRuins";

const Adv = new CoreAdvanced();
const Farm = new CoreFarms();
const CR = new CitadelRuins();
const DFO = new DragonFableOrigins();

export class LordOfOrder {
    getLoO(rankUpClass = true, getExtras = false): void {
        // LOO Quest ItemID: 50741
        // LOO Collector Chest ItemID: 50576

        // Check if the item is already in inventory or if extras are needed
        if (
            (
                Core.checkInventory([ 50741, 50576 ], 1, { any: true, toInv: false })
                && !getExtras
                && Core.isCompletedBefore(7165)
            )
            || (
                getExtras
                && Core.checkInventory(Core.questRewards(7165), 1, { toInv: false })
                && Core.isCompletedBefore(7165)
            )
        ) {
            if (rankUpClass)
                Adv.rankUpClass("Lord of Order");
            if (getExtras)
                Core.logger("All desired rewards owned for LOO.");
            return;
        }

        Core.logger("⚔️ Daily: Lord of Order Class");
        Core.logger(
            "It's a DAILY quest chain, 10 quests total, up to [7165]. One completes per run, per day. Read the logs below before you file a bug report.");

        Farm.experience(50);

        // Heart of Servitude
        if (bot.quests.isAvailable(7156) && !Core.isCompletedBefore(7156)) {
            Core.ensureAccept(7156);
            Core.addDrop(...Core.questRewards(7156));
            Core.equipClass(ClassType.Solo);
            Core.huntMonster("watchtower", "Chaorrupted Knight", "Pristine Blades of Order", 1, { isTemp: false });
            Core.buyItem("dreadrock", 1221, "Dreadrock Donation Receipt");
            Core.huntMonster("deadmoor", "Banshee Mallora", "Deadmoor Spirits Helped", 1, { isTemp: false });
            CR.murrysQuests();
            CR.polishsQuestsCitadelRuins();
            if (!Core.checkInventory("Mage's Gratitude")) {
                Core.addDrop("Mage's Gratitude");
                Core.ensureAccept(6182);
                Core.huntMonster("citadelruins", "Enn'tröpy", "Enn'tröpy Defeated", 1, { isTemp: true });
                Core.ensureComplete(6182);
            }
            Core.buyItem("ravenscar", 614, "Ravenscar's Truth");

            Core.ensureComplete(7156);
            Core.toBank(...Core.questRewards(7156));
            Core.logger("✅ [1/10] Heart of Servitude [7156] complete. Servitude served. This is a DAILY — the chain continues tomorrow, not right now, calm down.");
            return;
        }

        // Spirit of Justice
        if (bot.quests.isAvailable(7157) && !Core.isCompletedBefore(7157)) {
            Core.ensureAccept(7157);
            Core.addDrop(...Core.questRewards(7157));

            Core.equipClass(ClassType.Solo);
            Core.huntMonster("dwarfprison", "Warden Elfis", "Warden Elfis Detained", 1, { isTemp: false });
            Core.huntMonster("prison", "Piggy Drake", "Piggy Drake Punished", 1, { isTemp: false });
            Core.huntMonster("mysteriousdungeon", "Mysterious Stranger", "Mysterious Stranger Foiled", 1, { isTemp: false });
            Core.huntMonster("dreammaster", "Calico Cobby", "Calico Cobby Crushed", 1, { isTemp: false });

            Core.ensureComplete(7157);
            Core.toBank(...Core.questRewards(7157));
            Core.logger("✅ [2/10] Spirit of Justice [7157] complete. Justice: served, piping hot. DAILY quest — see you in the next 24h for #3.");
            return;
        }

        // Purification of Chaos
        if (bot.quests.isAvailable(7158) && !Core.isCompletedBefore(7158)) {
            Core.ensureAccept(7158);
            Core.addDrop(...Core.questRewards(7158));

            Core.equipClass(ClassType.Solo);

            Adv.buyItem("tercessuinotlim", 1951, "Chaoroot", 15);
            Core.huntMonster("chaosboss", "Ultra Chaos Warlord", "Chaotic War Essence", 15, { isTemp: false });
            Core.huntMonster("shadowgates", "Chaorruption", "Chaorrupting Particles", 15, { isTemp: false });
            bot.quests.updateQuest(2814);
            Core.huntMonster("stormtemple", "Chaos Lord Lionfang", "Purified Raindrop", 45, { isTemp: false });

            Core.ensureComplete(7158);
            Core.toBank(...Core.questRewards(7158));
            Core.logger("✅ [3/10] Purification of Chaos [7158] complete. Chaos purified, mildly inconvenienced. It's a DAILY — quest #4 unlocks tomorrow, patience.");
            return;
        }

        // Steadfast Will
        if (bot.quests.isAvailable(7159) && !Core.isCompletedBefore(7159)) {
            Core.ensureAccept(7159);
            Core.addDrop(...Core.questRewards(7159));

            Core.equipClass(ClassType.Solo);
            Core.huntMonster("gaiazor", "Gaiazor", "Gaiazor's Cornerstone", 1, { isTemp: false, publicRoom: Core.publicDifficult });
            bot.quests.updateQuest(4361);
            bot.sleep(2000);
            Core.huntMonster("treetitanbattle", "Dakka the Dire Dragon", "Dakka's Crystal", 1, { isTemp: false });
            Core.huntMonster("andre", "Giant Necklace", "Andre's Necklace Fragment", 1, { isTemp: false });
            // Perma-Aggroed mob escape.
            Core.jumpWait();
            Core.huntMonster("desolich", "Desolich", "Desolich's Skull", 1, { isTemp: false, publicRoom: Core.publicDifficult });

            Core.ensureComplete(7159);
            Core.toBank(...Core.questRewards(7159));
            Core.logger("✅ [4/10] Steadfast Will [7159] complete. Will: steadfast'd. DAILY quest, meaning daily — quest #5 waits for the sun to reset.");
            return;
        }

        // Strike of Order
        if (bot.quests.isAvailable(7160) && !Core.isCompletedBefore(7160)) {
            Core.ensureAccept(7160);
            Core.addDrop(...Core.questRewards(7160));

            Core.equipClass(ClassType.Solo);
            Core.killKitsune("Hanzamune Dragon Koi Blade");
            Core.huntMonster("ledgermayne", "Ledgermayne", "The Supreme Arcane Staff", 1, { isTemp: false });
            Core.huntMonster("mqlesson", "Dragonoid", "Dragonoid of Hours", 1, { isTemp: false });
            if (!Core.checkInventory("Safiria's Spirit Orb")) {
                Core.addDrop("Safiria's Spirit Orb");
                Core.getMapItem(5470, 1, "maxius");
                bot.wait.forPickup("Safiria's Spirit Orb");
            }
            DFO.dragonFableOriginsAll();
            if (!Core.checkInventory("Ice Katana")) {
                Core.addDrop("Ice Katana");
                Core.ensureAccept(6319);
                Core.equipClass(ClassType.Farm);
                Core.huntMonster("drakonnan", "Living Fire", "Inferno Heart");
                Core.ensureComplete(6319);
            }
            Core.ensureComplete(7160);
            Core.toBank(...Core.questRewards(7160));
            Core.logger("✅ [5/10] Strike of Order [7160] complete. Halfway there. Still a DAILY. Still resets tomorrow. Still not a bug.");
            return;
        }

        // Harmony
        if (bot.quests.isAvailable(7161) && !Core.isCompletedBefore(7161)) {
            Core.ensureAccept(7161);
            Core.addDrop(...Core.questRewards(7161));

            Core.equipClass(ClassType.Solo);
            Core.huntMonster("elemental", "Tree of Destiny", "Unity of Life", 1, { isTemp: false });
            Core.huntMonster("orchestra", "Faust", "Harmony of Solace", 1, { isTemp: false });
            Core.equipClass(ClassType.Farm);
            Core.huntMonster("cathedral", "Pactagonal Knight", "Teamwork Observed", 100, { isTemp: false });
            Core.equipClass(ClassType.Solo);
            Core.huntMonster("goose", "Queen's ArchSage", "Scroll of Enchantment", 1, { isTemp: false });

            Core.ensureComplete(7161);
            Core.toBank(...Core.questRewards(7161));
            Core.logger("✅ [6/10] Harmony [7161] complete. Everyone's vibing. DAILY quest, quest #7 drops after your next reset — go touch grass.");
            return;
        }

        // Ordinance
        if (bot.quests.isAvailable(7162) && !Core.isCompletedBefore(7162)) {
            Core.ensureAccept(7162);
            Core.addDrop(...Core.questRewards(7162));

            Core.equipClass(ClassType.Farm);
            Core.huntMonster("newfinale", "Alliance Healer", "Acolyte's Braille", 1, { isTemp: false });
            Core.huntMonster("wardwarf", "Drow Assassin", "Suppressed Drows", 50, { isTemp: false });
            Core.huntMonster("warundead", "Skeletal Fire Mage", "Suppressed Undead", 50, { isTemp: false });
            Core.huntMonster("warhorc", "Horc Warrior", "Suppressed Horcs", 50, { isTemp: false });
            Core.huntMonster("weaverwar", "Weaver Queen's Hound", "Suppressed Weavers", 50, { isTemp: false });
            Core.equipClass(ClassType.Solo);
            Core.huntMonster("extriki", "Extriki", "Strength of Resilience", 1, { isTemp: false });

            Core.ensureComplete(7162);
            Core.toBank(...Core.questRewards(7162));
            Core.logger("✅ [7/10] Ordinance [7162] complete. Ordinance ordinanced. DAILY quest — 2 left after this, they're not going anywhere but neither is the reset timer.");
            return;
        }

        // Axiom
        if (bot.quests.isAvailable(7163) && !Core.isCompletedBefore(7163)) {
            Core.ensureAccept(7163);
            Core.addDrop(...Core.questRewards(7163));

            Core.equipClass(ClassType.Solo);
            Core.huntMonster("elfhame", "Guardian Spirit", "Law of Nature", 1, { isTemp: false });
            Core.huntMonster("deepchaos", "Kathool", "Law of Time", 1, { isTemp: false });
            Core.huntMonster("necrocavern", "ShadowStone Support", "Law of Gravity", 1, { isTemp: false });
            Core.huntMonster("blackholesun", "Reflecteract", "Law of Relativity", 1, { isTemp: false });
            Core.huntMonster("thunderfang", "Tonitru", "Law of Conservation of Energy", 1, { isTemp: false });
            Core.huntMonster("lair", "Red Dragon", "Law of Low Drop Rates", 100, { isTemp: false });

            Core.ensureComplete(7163);
            Core.toBank(...Core.questRewards(7163));
            Core.logger("✅ [8/10] Axiom [7163] complete. The Law of Low Drop Rates remains undefeated. DAILY quest — one more after tomorrow's reset, hang tight.");
            return;
        }

        // Blessing of Order
        if (bot.quests.isAvailable(7164) && !Core.isCompletedBefore(7164)) {
            Core.ensureAccept(7164);
            Core.addDrop(...Core.questRewards(7164));

            Core.equipClass(ClassType.Solo);
            Core.killMonster("doomvaultb", "r26", "Left", "Undead Raxgore", "Weapon Imprint", 15, { isTemp: false });
            Farm.fishingREP(7);
            Core.buyItem("greenguardwest", 363, "Lure of Order");
            Adv.gearStore(false, { enhAfter: true });
            Core.killXiang("Quixotic Mana Essence", 10, { ultra: true });
            Adv.gearStore(true, { enhAfter: true });
            Core.equipClass(ClassType.Solo);
            Core.huntMonster("yasaris", "Serepthys", "Inversion Infusion", 5, { isTemp: false });

            Core.ensureComplete(7164);
            Core.toBank(...Core.questRewards(7164));
            Core.logger("✅ [9/10] Blessing of Order [7164] complete. That's the last DAILY of the set — tomorrow's reset unlocks The Final Challenge. Almost there, don't rage quit now.");
            return;
        }

        // The Final Challenge
        if (bot.quests.isAvailable(7165) && !Core.isCompletedBefore(7165)) {
            bot.drops.add(...Core.questRewards(7165));

            Core.ensureAccept(7165);
            Core.equipClass(ClassType.Solo);
            Core.huntMonster("ultradrakath", "Champion of Chaos", "Champion of Chaos Confronted", 1, { isTemp: false, publicRoom: Core.publicDifficult });
            bot.drops.add(50741);
            // If quest 7165 is not completed and either missing extras or missing key items
            if (!Core.isCompletedBefore(7165)) {
                Core.ensureComplete(7165, 50741);
                bot.wait.forPickup(50741);

                Core.ensureCompleteChoose(7165);
                Core.toBank(...Core.questRewards(7165).filter(x => x !== "Lord of Order"));
                Core.logger("🏆 [10/10] The Final Challenge [7165] complete. That's all 10 LOO dailies down — Lord of Order is FULLY finished. No more dailies today, no more quests, nothing. Go outside.");

                if (rankUpClass)
                    Adv.rankUpClass("Lord of Order");
            }
        }
    }
}

export default function main(): void {
        Core.setOptions();

        new LordOfOrder().getLoO();

        Core.setOptions(false);
}
