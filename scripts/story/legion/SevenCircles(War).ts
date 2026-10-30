/*
name: (War) Seven Circles
description: This will finish the Seven Circles quest.
tags: story, quest, legion, seven-circles, war
*/
// Port of Scripts/Story/Legion/SevenCircles(War).cs.

import { ClassType, Core } from "../../core/CoreBots";
import { CoreStory } from "../../core/CoreStory";

const Story = new CoreStory();

export class SevenCircles {
    circles(): void {
        if (Core.isCompletedBefore(7977))
            return;

        Story.preLoad(this.circles);

        Core.addDrop("Indulgence");

        //Canto IV
        Story.killQuest(7968, "sevencircles", "Limbo Guard");

        //Canto V
        Story.killQuest(7969, "sevencircles", "Luxuria Guard");

        //Gone With the Wind
        if (!Story.questProgression(7970)) {
            Core.ensureAccept(7970);
            Core.killMonster("sevencircles", "r2", "Left", "Limbo Guard", "Aura of Happiness", 4);
            Core.killMonster("sevencircles", "r2", "Left", "Limbo Guard", "Aura of Power");
            Core.killMonster("sevencircles", "r3", "Left", "Luxuria Guard", "Aura of Pleasure");
            Core.ensureComplete(7970);
        }

        //Lest Ye Be Destroyed
        Story.killQuest(7971, "sevencircles", "Luxuria");

        //Canto VI
        Story.mapItemQuest(7972, "sevencircles", 8206, 3);

        //HeckHound
        Story.killQuest(7973, "sevencircles", "Gluttony Guard");

        //Glutton  for Punishment
        Story.killQuest(7974, "sevencircles", "Gluttony");

        //Canto VII
        Story.killQuest(7975, "sevencircles", "Avarice Guard");

        //Greed the Room
        if (!Story.questProgression(7976)) {
            Core.ensureAccept(7976);
            Core.huntMonster("sevencircles", "Limbo Guard", "Limbo Guards Defeated", 2);
            Core.huntMonster("sevencircles", "Luxuria Guard", "Lust Guards Defeated", 2);
            Core.huntMonster("sevencircles", "Gluttony Guard", "Gluttony Guard Defeated");
            Core.huntMonster("sevencircles", "Avarice Guard", "Avarice Guards Defeated", 2);
            Core.ensureComplete(7976);
        }

        //Ava-risky Business
        Story.killQuest(7977, "sevencircles", "Avarice");
    }

    circlesWar(excludeBoss = false, StopForGoldFarm = false): void {
        if (excludeBoss ? Core.isCompletedBefore(7989) : Core.isCompletedBefore(7990))
            return;

        this.circles();

        Core.addDrop(
            "Essence of Treachery", 
            "Essence of Violence", 
            "Souls of Heresy", 
            "Essence of Wrath");

        //Guards of Wrath
        Core.equipClass(ClassType.Farm);
        Story.killQuest(7979, "sevencircleswar", "Wrath Guard");

        //War Medals
        Story.killQuest(7980, "sevencircleswar", "Wrath Guard");

        //Mega War Medals
        Story.killQuest(7981, "sevencircleswar", "Wrath Guard");

        if (StopForGoldFarm)
            return;

        //Wrath Against the Machine
        Core.equipClass(ClassType.Solo);
        Story.killQuest(7982, "sevencircleswar", "Wrath");

        //Blasphemy? Blasphe-you!
        Core.equipClass(ClassType.Farm);
        Story.killQuest(7983, "sevencircleswar", "Heresy Guard");

        //Violence's Gatekeeper
        Core.equipClass(ClassType.Solo);
        Story.killQuest(7984, "sevencircleswar", "Violence's Gatekeeper");

        //Meaningless Violence
        Core.equipClass(ClassType.Farm);
        Story.killQuest(7985, "sevencircleswar", "Violence Guard");

        //Geryon, Not Gary On!
        Core.equipClass(ClassType.Solo);
        Story.killQuest(7986, "sevencircleswar", "Geryon");

        //Violence
        Story.killQuest(7987, "sevencircleswar", "Violence");

        //Where the Trea-sun Don't Shine
        Core.equipClass(ClassType.Farm);
        Story.killQuest(7988, "sevencircleswar", "Treachery Guard");

        //Hanged for Treason
        Core.equipClass(ClassType.Solo);
        Story.killQuest(7989, "sevencircleswar", "Treachery");

        if (excludeBoss)
            return;

        //The Beast
        if (!Story.questProgression(7990)) {
            if (Core.checkInventory("ArchPaladin"))
                Core.equip("ArchPaladin");
            else
                Core.equipClass(ClassType.Solo);

            Core.ensureAccept(7990);
            Core.killMonster("sevencircleswar", "r17", "Left", "The Beast", "The Beast Defeated");
            Core.ensureComplete(7990);
        }
    }
}

export default function main(): void {
        Core.setOptions();

        new SevenCircles().circlesWar();

        Core.setOptions(false);
}
