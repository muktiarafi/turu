/*
name: PinkBladeofDestruction
description: null
tags: null
*/
// Port of Scripts/Other/Weapons/PinkBladeofDestruction.cs.

import { bot } from "turu";
import { ClassType, Core } from "../../core/CoreBots";
import { CoreBLOD } from "../../good/blod/CoreBLOD";
import { CoreDoomwood } from "../../story/doomwood/CoreDoomwood";

const BLoD = new CoreBLOD();
const P3 = new CoreDoomwood();

export class PinkBladeOfDestruciton {
    getPBoD(): void {
        if (Core.checkInventory("Pink Blade of Destruction"))
            return;

        P3.doomwoodPart3();

        Core.addDrop("Fuchsia Dye", "Zealous Badge", "Pink Blade of Destruction");

        Core.ensureAccept(7650);

        while (!bot.shouldExit && !Core.checkInventory("Fuchsia Dye", 50)) {
            Core.ensureAccept(1487);
            Core.huntMonster("natatorium", "Anglerfish", "Pink Coral", 3, { log: false });
            Core.huntMonster("bloodtuskwar", "Chaotic Vulture", "Amaranth Flower", 5, { log: false });
            Core.ensureComplete(1487);
        }

        BLoD.unlockMineCrafting();
        BLoD.spiritOrb(500);

        while (!bot.shouldExit && !Core.checkInventory("Zealous Badge", 5)) {
            Core.ensureAccept(7616);
            Core.equipClass(ClassType.Solo);
            Core.huntMonster("techdungeon", "Kalron the Cryptborg", "Immutable Dedication", 7, { log: false });
            Core.equipClass(ClassType.Farm);
            Core.huntMonster("techdungeon", "DoomBorg Guard", "Paladin Armor Scraps", 30, { log: false });
            Core.ensureComplete(7616);
        }

        Core.equipClass(ClassType.Solo);
        Core.huntMonster("undergroundlabb", "Ultra BrutalCorn", "Unicorn Essence", 5, { isTemp: false, log: false });

        Core.huntMonster("undergroundlabb", "Ultra Battle Gem", "Gem Power", 5, { isTemp: false, log: false });

        Core.ensureComplete(7650, 55884);
        bot.wait.forPickup("Pink Blade of Destruction");
    }
}

export default function main(): void {
        Core.setOptions();

        new PinkBladeOfDestruciton().getPBoD();

        Core.setOptions(false);
}
