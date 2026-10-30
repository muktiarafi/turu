/*
name: VoidAvengerScythe
description: null
tags: null
*/
// Port of Scripts/Other/Weapons/VoidAvengerScythe.cs.
// Not ported (FarmerJoe's DoAll doesn't reach them): BatwingScythe.

import { bot } from "turu";
import { ClassType, Core } from "../../core/CoreBots";
import { CoreFarms } from "../../core/CoreFarms";
import { CoreStory } from "../../core/CoreStory";
import { CoreNSOD } from "../../evil/nsod/CoreNSOD";
import { CoreNation } from "../../nation/CoreNation";
import { EmpoweringItems } from "../../nation/EmpoweringItems";
import { JuggernautItemsofNulgath, RewardsSelection } from "../../nation/various/JuggernautItems";

const Farm = new CoreFarms();
const Story = new CoreStory();
const Nation = new CoreNation();
const NSoD = new CoreNSOD();
const juggernaut = new JuggernautItemsofNulgath();
const Empower = new EmpoweringItems();

export class VoidAvengerScythe {
    skewsQuests(): void {
        this.snowbeardsQuests();
        this.donnaCharmersQuests();

        if (Core.checkInventory("Void Avenger Scythe"))
            return;

        Core.logger("Farming Void Avenger Scythe.");

        Core.addDrop("Void Avenger Scythe", "Batwing Scythe");

        Core.ensureAccept(5025);

        //Eternal Rest

        Farm.evilREP();
        // Batwing Scythe -
        // if (!Bot.Quests.IsUnlocked(498))
        //     BatwingScythe();

        bot.quests.updateQuest(498);
        Core.huntMonster("darkoviagrave", "Blightfang", "Batwing Scythe", 1, { isTemp: false });

        Core.equipClass(ClassType.Solo);
        while (!bot.shouldExit && !Core.checkInventory("Batwing Scythe")) {
            Core.ensureAccept(498);
            Core.huntMonster("darkoviagrave", "Blightfang", "Blightfang's Skull", 1, { log: false });
            Core.ensureComplete(498);
            bot.wait.forPickup("Batwing Scythe");
        }
        // Dark Crystal Shard -
        Nation.farmDarkCrystalShard(200);
        // Death Scythe of Nulgath -
        Empower.empoweringStuff();
        // Ungodly Reavers of Nulgath -
        juggernaut.juggItems(RewardsSelection.Ungodly_Reavers_of_Nulgath);
        // Scythe of Sisyphean -
        Core.equipClass(ClassType.Farm);
        Core.huntMonster("dragonplane", "Wind Elemental", "Scythe of Sisyphean", 1, { isTemp: false });
        // Heart of the Void -
        Core.equipClass(ClassType.Solo);
        Core.huntMonster("void", "Void Dragon", "Heart of the Void", 1, { isTemp: false });
        // The Scythe of Eternal Rest -
        Core.equipClass(ClassType.Solo);
        Core.huntMonster("sepulchure", "Dark Sepulchure", "The Scythe of Eternal Rest", 1, { isTemp: false });
        // Nulgath's Approval -
        Core.equipClass(ClassType.Farm);
        Nation.approvalAndFavor(1000, 0);
        // Dracolich Destroyer Scythe -
        Core.equipClass(ClassType.Solo);
        Core.huntMonster("dragonheart", "Avatar of Desolich", "Dracolich Destroyer Scythe", 1, { isTemp: false });
        // Void Aura -
        NSoD.voidAuras(150);
        // Letter from Asuka and Tendou -
        Core.equipClass(ClassType.Farm);
        Core.huntMonster("Citadel", "Burning Witch", "Letter from Asuka and Tendou", 1, { isTemp: false });
        Core.ensureComplete(5025);
        bot.wait.forPickup("Void Avenger Scythe");
    }

    donnaCharmersQuests(): void {
        if (Core.isCompletedBefore(327))
            return;

        // Requirements: Must have completed the 'Bear Facts' quest.

        // The Spittoon Saloon
        Story.killQuest(324, "pines", "Red Shell Turtle");
        // Bear it all!
        Story.killQuest(325, "pines", "Pine Grizzly");
        // Leather Feathers
        Story.killQuest(326, "pines", "LeatherWing");
        // Follow your Nose!
        Story.killQuest(327, "pines", "LeatherWing");
    }

    snowbeardsQuests(): void {
        if (Core.isCompletedBefore(322))
            return;

        // Adorable Sisters
        Story.mapItemQuest(319, "tavern", 56, 7);
        // Warm and Furry
        Story.killQuest(320, "pines", "Pine Grizzly");
        // Shell Shock
        Story.killQuest(321, "pines", "Red Shell Turtle");
        // Bear Facts
        Story.killQuest(322, "pines", "Twistedtooth");
    }
}

export default function main(): void {
        Core.setOptions();

        new VoidAvengerScythe().skewsQuests();

        Core.setOptions(false);
}
