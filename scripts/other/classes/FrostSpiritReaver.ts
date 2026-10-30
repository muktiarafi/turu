/*
name: FrostSpiritReaver
description: null
tags: null
*/
// Port of Scripts/Other/Classes/FrostSpiritReaver.cs.

import { bot } from "turu";
import { CoreAdvanced } from "../../core/CoreAdvanced";
import { ClassType, Core } from "../../core/CoreBots";
import { CoreDailies } from "../../core/CoreDailies";
import { CoreFarms } from "../../core/CoreFarms";
import { GlaceraStory } from "../../story/Glacera";

const Adv = new CoreAdvanced();
const Glacera = new GlaceraStory();
const Farm = new CoreFarms();
const Dailies = new CoreDailies();

export class FrostSpiritReaver {
    getFSR(rankUpClass = true): void {
        if (Core.checkInventory(59178))
            return;

        Glacera.doAll();
        if (Farm.factionRank("Glacera") < 10) {
            // Attempt to relog so the rep works.
            Core.relogin(
                "\"GlaceraREP\" wont work, unless its relogged after doing the story... ae really needs to fix this.");
            Farm.glaceraREP();
        }
        if (!Core.checkInventory("Cryomancer"))
            Dailies.cryomancer();

        if (!Core.checkInventory("Frost Sigil")) {
            Core.logger(
                "\"Frost Sigil\" for \"IceNinth\"(Quest `Cold Hearted`), Comeback tomarrow.");
            return;
        }
        Farm.experience(60);

        if (!Core.checkInventory("Envoy of Kyanos")) {
            if (!Core.checkInventory("Envoy of Kyanos"))
                Core.logger("Getting Quest Item Requirements for \"Ice See You\"");

            if (!Core.checkInventory("Favored of Kyanos")) {
                Core.logger("Farming the requirements to buy \"Favored of Kyanos\"");
                Core.huntMonster("icedungeon", "Shade of Kyanos", "Warrior of Kyanos", 1, { isTemp: false });
                this.tokens(25, 15, 10, 5);

                Adv.buyItem("icedungeon", 1948, "Favored of Kyanos");
            }
            Core.logger("Farming the requirements to buy \"Envoy of Kyanos\"");
            this.tokens(50, 30, 20, 10);

            Adv.buyItem("icedungeon", 1948, "Envoy of Kyanos");
        }

        this.iceNinth(9);
        this.glaceranAttunement(15);
        Core.addDrop("Frost SpiritReaver");
        Core.chainComplete(7922);
        bot.wait.forPickup("Frost SpiritReaver");

        if (rankUpClass)
            Adv.rankUpClass("Frost SpititReaver");
    }

    iceNinth(quant: number): void {
        if (Core.checkInventory("Ice-Ninth", quant))
            return;

        Core.addDrop("Ice-Ninth", "Ice Diamond");
        Core.farmingLogger("Ice-Ninth", quant);

        //////////////////////////////////////////////
        //////////////////////////////////////////////
        //#region "Quest Prerequisites

        if (!Core.checkInventory(25464) && Core.checkInventory([ 27437, 27525 ], 1, { any: true })) {
            //Frost Sigil
            Core.buyItem("icedungeon", Core.checkInventory(27437) ? 2294 : 2295, 25464, 1, Core.checkInventory(27437) ? 48001 : 48002);
            Core.toBank(27437, 27525);
        }

        Core.registerQuests(3955);
        while (!bot.shouldExit && !Core.checkInventory("Flame of Courage", 25)) {
            Core.addDrop("Flame of Courage");
            Core.huntMonster("frozenruins", "Frost Invader", "Spark of Courage", 1, { log: false });
            bot.wait.forPickup("Flame of Courage");
        }
        Core.cancelRegisteredQuests();
        Core.toBank("Frozen Tower Merge Token");

        //cryomancer unbank
        if (!Core.checkInventory("Fallen Scythe of Vengeance")) {
            Core.logger("Getting the quest item requirements for \"Cold Hearted\"");
            Core.equipClass(ClassType.Solo);
            Core.huntMonster("Northstar", "Karok The Fallen", "Karok's Glaceran Gem", 1, { isTemp: false });
            Adv.buyItem("Glacera", 1055, "Scythe of Vengeance");
            Adv.buyItem("Glacera", 1055, "Cold Scythe of Vengeance");
            Adv.buyItem("Glacera", 1055, "Frigid Scythe of Vengeance");
            Adv.buyItem("Glacera", 1055, "Fallen Scythe of Vengeance");
        }

        //#endregion "Quest Prerequisites
        //////////////////////////////////////////////
        //////////////////////////////////////////////

        while (!bot.shouldExit && !Core.checkInventory("Ice-Ninth", quant)) {
            Core.ensureAccept(7920);
            Core.equipClass(ClassType.Solo);
            Core.huntMonster("Snowmore", "Jon S'NOOOOOOO", "Northern Crown", 1, { isTemp: false });

            Core.equipClass(ClassType.Farm);
            Core.huntMonster("icestormarena", "Arctic Wolf", "Ice Needle", 30, { isTemp: false });
            Core.addDrop("Ice Diamond");
            Core.farmingLogger("Ice Diamond", 100);
            while (!bot.shouldExit && !Core.checkInventory("Ice Diamond", 100)) {
                Core.ensureAccept(7279);
                Core.killMonster("kingcoal", "r1", "Left", "*", "Frozen Coal", 10, { log: false });
                Core.ensureComplete(7279);
                bot.wait.forPickup("Ice Diamond");
            }
            Core.ensureComplete(7920);
            bot.wait.forPickup("Ice-Ninth");
        }
        Core.toBank(...Core.questRewards(7920));
        Core.unbank("Ice-Ninth");
    }

    glaceranAttunement(quant: number): void {
        if (Core.checkInventory("Glaceran Attunement", quant))
            return;

        //to unlock the quest
        if (!Core.isCompletedBefore(7921)) {
            Core.logger("Doing prerequisets for \"Cold Blooded\" [1x \"Ice Ninth\"]");
            this.iceNinth(1);
        }

        Farm.glaceraREP();
        if (!Core.checkInventory(39011)) {
            //FrostSlayer
            Core.logger("Getting the quest item required to start \"Cold Blooded\"");
            Core.addDrop(39011);
            Core.equipClass(ClassType.Solo);
            while (!bot.shouldExit && !Core.checkInventory(39011)) {
                Core.huntMonster("iceplane", "Enfield", "FrostSlayer", 1, { isTemp: false, log: false });
            }
        } else
            Core.logger("Got the Item requirement for \"Cold Blooded\"");

        Core.addDrop(59216);
        Core.farmingLogger("Glaceran Attunement", quant);
        Core.registerQuests(7921);
        while (!bot.shouldExit && !Core.checkInventory(59216, quant)) {
            Core.equipClass(ClassType.Solo);
            Core.huntMonster("cryowar", "Super-Charged Karok", "Glacial Crystal", 100, { isTemp: false, log: false });
            Core.equipClass(ClassType.Dodge);
            Core.huntMonster("frozenlair", "Legion Lich Lord", "Necrotic Orb", 2, { isTemp: false, log: false });

            Core.equipClass(ClassType.Farm);
            Core.huntMonster("frozenlair", "Frozen Legionnaire", "Ice Spike", 20, { isTemp: false, log: false });
            Core.huntMonster("frozenlair", "Frozen Legionnaire", "Ice Splinter", 20, { isTemp: false, log: false });
        }
        bot.wait.forPickup("Glaceran Attunement");
        Core.cancelRegisteredQuests();
        Core.toBank(...Core.questRewards(7921));
        Core.unbank(59216);
    }

    tokens(Token1 = 300, Token2 = 300, Token3 = 300, Token4 = 300): void {
        if (!Core.checkInventory("Icy Token I", Token1)) {
            Core.addDrop("Icy Token I");
            Core.farmingLogger("Icy Token I", Token1);
            Core.equipClass(ClassType.Farm);

            Core.registerQuests(7840, 7838);
            while (!bot.shouldExit && !Core.checkInventory("Icy Token I", Token1)) {
                Core.huntMonster("icedungeon", "Frosted Banshee", "Frosted Banshee Defeated", 10, { log: false });
                Core.huntMonster("icedungeon", "Frozen Undead", "Frozen Undead Defeated", 10, { log: false });
                Core.huntMonster("icedungeon", "Ice Symbiote", "Ice Symbiote Defeated", 10, { log: false });
            }
            Core.cancelRegisteredQuests();
        }

        if (!Core.checkInventory("Icy Token II", Token2)) {
            Core.addDrop("Icy Token II");
            Core.farmingLogger("Icy Token II", Token2);
            Core.equipClass(ClassType.Farm);

            Core.registerQuests(7839);
            while (!bot.shouldExit && !Core.checkInventory("Icy Token II", Token2)) {
                Core.huntMonster("icedungeon", "Spirit of Ice", "Spirit of Ice Defeated", 10, { log: false });
                Core.huntMonster("icedungeon", "Ice Crystal", "Ice Crystal Defeated", 10, { log: false });
                Core.huntMonster("icedungeon", "Frigid Spirit", "Frigid Spirit Defeated", 10, { log: false });

                bot.wait.forPickup("Icy Token II");
            }
            Core.cancelRegisteredQuests();
        }

        if (!Core.checkInventory("Icy Token III", Token3)) {
            Core.addDrop("Icy Token III");
            Core.farmingLogger("Icy Token III", Token3);
            Core.equipClass(ClassType.Farm);

            Core.registerQuests(7840);
            while (!bot.shouldExit && !Core.checkInventory("Icy Token III", Token3)) {
                Core.huntMonster("icedungeon", "Living Ice", "Living Ice Defeated", 5, { log: false });
                Core.huntMonster("icedungeon", "Crystallized Elemental", "Crystallized Elemental Defeated", 5, { log: false });
                Core.huntMonster("icedungeon", "Frozen Demon", "Frozen Demon Defeated", 5, { log: false });

                bot.wait.forPickup("Icy Token III");
            }
            Core.cancelRegisteredQuests();
        }

        if (!Core.checkInventory("Icy Token IV", Token4)) {
            Core.addDrop("Icy Token IV");
            Core.farmingLogger("Icy Token IV", Token4);
            Core.equipClass(ClassType.Solo);

            Core.registerQuests(7841);
            while (!bot.shouldExit && !Core.checkInventory("Icy Token IV", Token4)) {
                Core.huntMonster("icedungeon", "Image of Glace", "Glace's Approval", 1, { log: false });
                Core.huntMonster("icedungeon", "Abel", "Abel's Approval", 1, { log: false });
                Core.huntMonster("icedungeon", "Shade of Kyanos", "Kyanos' Approval", 1, { log: false });

                bot.wait.forPickup("Icy Token IV");
            }
            Core.cancelRegisteredQuests();
        }
    }
}

export default function main(): void {
        Core.setOptions();

        new FrostSpiritReaver().getFSR();

        Core.setOptions(false);
}
