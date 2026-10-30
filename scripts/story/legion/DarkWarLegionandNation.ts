/*
name: Dark War Legion and Nation
description: This will finish the Dark War Legion and Nation quest.
tags: story, quest, legion, dark-war-legion-and-nation
*/
// Port of Scripts/Story/Legion/DarkWarLegionandNation.cs.

import { bot } from "turu";
import { ClassType, Core } from "../../core/CoreBots";
import { CoreFarms } from "../../core/CoreFarms";
import { CoreStory } from "../../core/CoreStory";
import { Core13LoC } from "../lordsofchaos/Core13LoC";

const Farm = new CoreFarms();
const LOC = new Core13LoC();
const Story = new CoreStory();

export class DarkWarLegionandNation {
    doBoth(): void {
        this.darkWarLegion();
        this.darkWarNation();
    }

    darkWarLegion(): void {
        if (Core.isCompletedBefore(8588))
            return;

        Story.preLoad(this.darkWarLegion);

        //Pop Goes the Makai
        Story.killQuest(8556, "dagerecruit", "Dark Makai");

        //Dispel Spell
        if (!Story.questProgression(8557)) {
            Core.ensureAccept(8557);
            Core.killMonster("dagerecruit", "Enter", "Spawn", "Dreadfiend", "Fiend Energy Collected", 4);
            Story.mapItemQuest(8557, "dagerecruit", 9883, 4);
        }

        //Dreadfiend Demolition
        if (!Story.questProgression(8558)) {
            Core.ensureAccept(8558);
            Core.killMonster("dagerecruit", "Enter", "Spawn", "Dreadfiend", "Dreadfiend Defeated", 6);
            Core.ensureComplete(8558);
        }

        //Graython Located
        Story.mapItemQuest(8559, "dagerecruit", 9884);

        //Defeat Graython
        if (!Story.questProgression(8560)) {
            Core.equipClass(ClassType.Solo);
            Core.ensureAccept(8560);
            while (!bot.shouldExit && bot.map.name !== "dagerecruit" && bot.player?.cell !== "r3") {
                if (bot.map.name !== "dagerecruit")
                    Core.join("dagerecruit");
                if (bot.player?.cell !== "r3")
                    Core.jump("r3", "Left");
                Core.sleep();
            }
            Core.killMonster("dagerecruit", "r3", "Left", "Graython", "Graython Defeated");
            Core.ensureComplete(8560);
        }

        //Island Sightseeing
        if (!Story.questProgression(8561)) {
            Core.ensureAccept(8561);
            Core.huntMonster("dagerecruit", "Scared Wildcat", "Wildlife Cleared", 10);
            Story.mapItemQuest(8561, "dagerecruit", 9885);
        }

        //Lure Crafted
        if (!Story.questProgression(8562)) {
            Core.ensureAccept(8562);
            Core.killMonster("dagerecruit", "Enter", "Spawn", "Dreadfiend", "Void Crystals", 3);
            Core.ensureComplete(8562);
        }

        // Lure Charged
        if (!Story.questProgression(8563)) {
            Core.ensureAccept(8563);
            Core.huntMonster("dagerecruit", "Scared Wildcat", "Death Charge", 10);
            Story.mapItemQuest(8563, "dagerecruit", 9885);
        }

        // Place the Lure
        Story.mapItemQuest(8564, "dagerecruit", 9886);

        // Defeat Nuckelavee
        Story.killQuest(8565, "dagerecruit", "Nuckelavee");

        // Bloody the Fiends
        Story.killQuest(8566, "dagerecruit", "Bloodfiend");

        // Unstable Energies
        Story.killQuest(8567, "dagerecruit", "Bloodfiend");

        // Plant the Bombs
        Story.mapItemQuest(8568, "dagerecruit", 9887, 4);

        // Those Infernal Fiends
        Story.killQuest(8569, "dagerecruit", "Infernal Fiend");

        // Defeat Smaras
        Story.killQuest(8570, "dagerecruit", "Smaras");

        // Understanding Yokai
        if (!Story.questProgression(8571)) {
            Core.equipClass(ClassType.Farm);
            Core.ensureAccept(8571);
            Core.huntMonster("dagerecruit", "Funa-yurei", "Funa-Yurei Defeated", 10);
            Core.ensureComplete(8571);
        }

        // Covering Our Scent
        if (!Story.questProgression(8572)) {
            Core.equipClass(ClassType.Farm);
            Core.ensureAccept(8572);
            Core.huntMonster("dagerecruit", "Funa-yurei", "Yokai Energy", 4);
            Story.mapItemQuest(8572, "dagerecruit", 9888, 4);
            Core.equipClass(ClassType.Solo);
        }

        // Can't Escape the Shadows
        Story.killQuest(8573, "dagerecruit", "Shadow Clone");

        // Last of the Defenses
        Story.killQuest(8574, "dagerecruit", "Shadow Clone");

        // Defeat Hebimaru
        if (!Story.questProgression(8575)) {
            Core.ensureAccept(8575);
            Core.huntMonster("dagerecruit", "Hebimaru", "Hebimaru Defeated");
            Core.ensureComplete(8575);
        }

        // Scorched Earth
        if (!Story.questProgression(8576)) {
            Core.ensureAccept(8576);
            Core.killMonster("dagerecruit", "Enter", "Spawn", "Dreadfiend", "Dreadfiend Defeated", 6);
            Core.huntMonster("dagerecruit", "Dark Makai", "Dark Makai Defeated", 6);
            Core.huntMonster("dagerecruit", "Bloodfiend ", "Bloodfiend Defeated", 6);
            Core.huntMonster("dagerecruit", "Infernal Fiend", "Infernal Fiend Defeated", 6);
            Core.ensureComplete(8576);
        }

        // Nation Badges / Mega Nation Badges
        Story.killQuest(8584, "darkwarlegion", "Dreadfiend");

        // A Nation Defeated
        Story.killQuest(8586, "darkwarlegion", "Dreadfiend");

        // ManSlayer? More Like ManSLAIN
        Story.killQuest(8587, "darkwarlegion", "Manslayer Fiend");

        // Defeat Dirtlicker
        Story.killQuest(8588, "darkwarlegion", "Dirtlicker");
    }

    darkWarNation(): void {
        if (Core.isCompletedBefore(8583))
            return;

        LOC.vath();

        // Legion Badges && Legion Badges,Mega Legion Badges - 8578, 8579
        if (!Story.questProgression(8580)) {
            Core.ensureAcceptmultiple(8578, 8579);
            Core.killMonster("darkwarnation", "Enter", "Spawn", "*", "Legion Badge", 5);
            Core.killMonster("darkwarnation", "Enter", "Spawn", "*", "Mega Legion Badge", 3);
            Core.ensureCompleteQuests(8578, 8579);
        }

        // Doomed Legion Warriors
        Story.killQuest(8580, "darkwarnation", "Legion Doomknight");

        // Undead Legion Dread
        Story.killQuest(8581, "darkwarnation", "Legion Dread Knight");

        // Defeat War
        Story.killQuest(8582, "darkwarnation", "War");

        // The Traitor
        Story.killQuest(8583, "darkwarnation", "Dage the Evil");
    }
}

export default function main(): void {
        Core.setOptions();

        new DarkWarLegionandNation().doBoth();

        Core.setOptions(false);
}
