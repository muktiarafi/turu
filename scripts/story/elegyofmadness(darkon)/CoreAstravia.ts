/*
name: null
description: null
tags: null
*/
// Port of Scripts/Story/ElegyofMadness(Darkon)/CoreAstravia.cs.

import { bot } from "turu";
import { ClassType, Core } from "../../core/CoreBots";
import { CoreStory } from "../../core/CoreStory";

const Story = new CoreStory();

export class CoreAstravia {
    completeCoreAstravia(): void {
        this.eridani();
        this.astravia();
        this.astraviaCastle();
        this.astraviaJudgement();
        this.eridaniPast();
        this.astraviaPast();
        this.firstObservatory();
        this.genesisGarden();
        this.theWorld();
    }

    eridani(): void {
        //Progress Check
        if (Core.isCompletedBefore(7779)) {
            Core.logger("StoryLine Complete:   Eridani");
            return;
        }

        //Preload Quests
        Story.preLoad(this.eridani);

        Core.join("Garden");
        Core.sleep(2000);

        //Lost Breadcrumbs
        if (!Story.questProgression(7769)) {
            Core.ensureAccept(7769);
            Core.getMapItem(7783, 1, "eridani");
            Core.huntMonster("eridani", "Maggot-Like Creature", "Creature Slain", 2);
            Core.ensureComplete(7769);
        }

        //Dirty Laundry
        if (!Story.questProgression(7770)) {
            Core.ensureAccept(7770);
            Core.killMonster("eridani", "Enter", "Spawn", "Maggot-Like Creature", "Dress Scraps", 5);
            Core.ensureComplete(7770);
        }

        //Ergot Bread
        if (!Story.questProgression(7771)) {
            Core.ensureAccept(7771);
            Core.killMonster("eridani", "r10", "Right", "Rat-Like Creature", "Rats Exterminated", 5);
            Core.ensureComplete(7771);
        }

        //Kids These Days
        if (!Story.questProgression(7772)) {
            Core.ensureAccept(7772);
            Core.killMonster("eridani", "r10", "Right", "Bat-Like Creature", "Meat Cuts", 5);
            Core.ensureComplete(7772);
        }

        //No Good Dead
        Story.killQuest(7773, "eridani", "Wolf-Like Creature");

        //A Worn Book's Cover
        if (!Story.questProgression(7774)) {
            Core.ensureAccept(7774);
            Core.huntMonsterMapID("eridani", 3, "Rat-Like Creature Slain");
            Core.huntMonsterMapID("eridani", 13, "Bat-Like Creature Slain");
            Core.huntMonsterMapID("eridani", 1, "Maggot-Like Creatures Slain", 2);
            Story.mapItemQuest(7774, "eridani", 7784);
        }

        //No Good Dead
        if (!Story.questProgression(7775)) {
            Core.ensureAccept(7775);
            Core.getMapItem(7785, 1, "eridani");
            Core.huntMonster("eridani", "Wolf-Like Creature", "Wolf-Like Creature Slain", 2);
            Core.ensureComplete(7775);
        }

        //The Black Goat in The Dark
        if (!Story.questProgression(7776)) {
            bot.options.attackWithoutTarget = true;
            Core.ensureAccept(7776);
            Core.huntMonsterMapID("eridani", 8, "Creature 15 Hunted");
            Core.huntMonsterMapID("eridani", 11, "Creature 16 Hunted");
            Core.huntMonsterMapID("eridani", 5, "Creature 34 Hunted", 7);
            Core.ensureComplete(7776);
            bot.options.attackWithoutTarget = false;
        }

        //The Star Sinks into the Sea
        if (!Story.questProgression(7777)) {
            bot.options.attackWithoutTarget = true;
            Core.ensureAccept(7777);
            Core.huntMonsterMapID("eridani", 17, "Creature 30 Hunted");
            Core.huntMonsterMapID("eridani", 2, "Creature 31 Hunted");
            Core.ensureComplete(7777);
            bot.options.attackWithoutTarget = false;
        }

        //The Gentlest Truth
        if (!Story.questProgression(7778)) {
            Core.ensureAccept(7778);
            Core.getMapItem(7786, 1, "eridani");
            Core.huntMonster("eridani", "Door", "Door", 1);
            Core.ensureComplete(7778);
        }

        //Time Moves the Moon
        Core.equipClass(ClassType.Solo);
        Story.killQuest(7779, "eridani", "Creature 6");
    }

    astravia(): void {
        //Progress Check
        if (Core.isCompletedBefore(8000)) {
            Core.logger("StoryLine Complete:   Astravia");
            return;
        }

        this.eridani();

        //Preload Quests
        Story.preLoad(this.astravia);

        //Caro-Kann Defense
        Story.killQuest(7993, "astravia", "Monstrous Dove");

        //Materialism
        Story.killQuest(7994, "astravia", "Drago's Spy");

        //A Castled King
        if (!Story.questProgression(7995)) {
            Core.ensureAccept(7995);
            Core.getMapItems([ [8264, 1], [8265, 1], [8266, 1] ], "astravia");
            Core.huntMonster("astravia", "Astravian Mercenary", "Greedy Mercenaries", 6);
            Core.ensureComplete(7995);
        }

        //Principle of Weakness
        if (!Story.questProgression(7996)) {
            Core.ensureAccept(7996);
            Core.killMonster("astravia", "r6", "Bottom", "Creature 27", "Creature 27 Slain", 8);
            Core.ensureComplete(7996);
        }

        //Zugzwang
        Story.killQuest(7997, "astravia", "Creature 28");

        //Dead Position
        Core.equipClass(ClassType.Solo);
        Story.killQuest(7998, "astravia", "Ti");

        //Desperado
        Story.killQuest(7999, "astravia", "Creature 27");

        //Isolani
        Story.killQuest(8000, "astravia", "The Moon");
    }

    astraviaCastle(): void {
        //Progress Check
        if (Core.isCompletedBefore(8256)) {
            Core.logger("StoryLine Complete:   AstraviaCastle");
            return;
        }

        this.astravia();

        //Preload Quests
        Story.preLoad(this.astraviaCastle);

        //Terpsichore, the Chained
        if (!Core.isCompletedBefore(8247)) {
            Core.ensureAccept(8247);
            Core.killMonster("astraviacastle", "r3", "Left", "*", "Key Pieces", 9);
            Core.ensureComplete(8247);
        }

        //Calliope, the Arrogant
        if (!Core.isCompletedBefore(8248)) {
            Core.ensureAccept(8248);
            Core.killMonster("astraviacastle", "r3", "Left", "*", "Creatures Shoo'd", 6);
            Story.mapItemQuest(8248, "astraviacastle", 8891, 6);
        }

        //Euterpe, the Spiteful
        if (!Story.questProgression(8249)) {
            Core.ensureAccept(8249);
            Core.getMapItem(8892, 6, "astraviacastle");
            Core.huntMonster("astraviacastle", "Creature 20", "Transformed Guard Exterminated", 4);
            Core.ensureComplete(8249);
        }

        //Urania, the Origin
        if (!Core.isCompletedBefore(8250)) {
            Core.ensureAccept(8250);
            Core.killMonster("astraviacastle", "r3", "Left", "*", "Creatures Repelled", 9);
            Story.mapItemQuest(8250, "astraviacastle", 8893);
        }

        //Polyhumnia, the Forgotten
        if (!Core.isCompletedBefore(8251)) {
            Core.ensureAccept(8251);
            Core.killMonster("astraviacastle", "r3", "Left", "*", "Creatures Defeated", 6);
            Story.mapItemQuest(8251, "astraviacastle", 8894);
        }

        //Clio, the Liar
        Story.killQuest(8252, "astraviacastle", "Creature 20");

        //Melpomene, the Inevitable
        Story.killQuest(8253, "astraviacastle", "Astravian Royal Guard");

        //Erato, the Inconsolable
        if (!Story.questProgression(8254)) {
            Core.ensureAccept(8254);
            Core.getMapItem(8895, 3, "astraviacastle");
            Core.huntMonster("astraviacastle", "Storage Spider", "Spiders Exterminated", 8);
            Core.ensureComplete(8254);
        }

        //Thalia, the Truth
        Story.mapItemQuest(8255, "astraviacastle", 8896);

        //Mnemosyne, the Tormentor
        Core.equipClass(ClassType.Solo);
        Story.killQuest(8256, "astraviacastle", "The Sun");
    }

    astraviaJudgement(): void {
        //Progress Check
        if (Core.isCompletedBefore(8395)) {
            Core.logger("StoryLine Complete:   AstraviaJudgement");
            return;
        }

        //Preload Quests
        Story.preLoad(this.astraviaJudgement);

        //Atata
        Story.mapItemQuest(8386, "astraviajudge", 9275, 6);

        //Samghata
        Story.killQuest(8387, "astraviajudge", "Hand");

        //Hahava
        Story.killQuest(8388, "astraviajudge", "Trumpeter");

        //Sanjiva
        if (!Core.isCompletedBefore(8389)) {
            Core.ensureAccept(8389);
            Core.killMonster("astraviajudge", "r4", "Left", "Juror", "Jurors Repelled", 9);
            Core.ensureComplete(8389);
        }

        //Kalasutra
        if (!Story.questProgression(8390)) {
            Core.ensureAccept(8390);
            Core.getMapItem(9276, 1, "astraviajudge");
            Core.huntMonster("astraviajudge", "Trumpeter", "Horns Crushed", 9);
            Core.ensureComplete(8390);
        }

        //Padma
        if (!Story.questProgression(8391)) {
            Core.ensureAccept(8391);
            Core.getMapItem(9277, 8, "astraviajudge");
            Core.huntMonster("astraviajudge", "Hand", "Hands Impaled", 8);
            Core.ensureComplete(8391);
        }

        //Maharaurava
        Story.killQuest(8392, "astraviajudge", "Trumpeter");

        //Raurava
        if (!Core.isCompletedBefore(8393)) {
            Core.ensureAccept(8393);
            Core.killMonster("astraviajudge", "r4", "Left", "Juror", "Jurors Shoved", 9);
            Core.ensureComplete(8393);
        }

        //Avici
        if (!Story.questProgression(8394)) {
            Core.ensureAccept(8394);
            Core.getMapItem(9278, 1, "astraviajudge");
            Core.huntMonster("astraviajudge", "Shades", "Shades Dispelled", 1);
            Core.ensureComplete(8394);
        }

        //Mahapadma
        Core.equipClass(ClassType.Solo);
        Story.killQuest(8395, "astraviajudge", "La");
    }

    eridaniPast(): void {
        //Progress Check
        if (Core.isCompletedBefore(8530)) {
            Core.logger("StoryLine Complete:   EridaniPast");
            return;
        }

        this.astraviaJudgement();

        //Preload Quests
        Story.preLoad(this.eridaniPast);

        //Dorian
        Story.mapItemQuest(8521, "eridanipast", 9681);

        //Hypophrygian
        if (!Story.questProgression(8522)) {
            Core.ensureAccept(8522);
            Core.huntMonster("eridanipast", "Bandit", "Bandit Defeated", 7);
            Core.huntMonster("eridanipast", "Dog", "Dog Defeated", 7);
            Core.ensureComplete(8522);
        }

        //Ionian
        Story.mapItemQuest(8523, "eridanipast", 9676, 8);

        //Lydian
        Story.killQuest(8524, "eridanipast", "Bandit");

        //Mixolydian
        if (!Story.questProgression(8525)) {
            Core.ensureAccept(8525);
            Core.getMapItem(9677, 1, "eridanipast");
            Core.huntMonster("eridanipast", "Bat", "Bat Swatted", 8);
            Core.ensureComplete(8525);
        }

        //Hypomixolydian
        Story.mapItemQuest(8526, "eridanipast", 9678);
        Story.mapItemQuest(8526, "eridanipast", 9679);

        //Locrian
        if (!Story.questProgression(8527)) {
            Core.ensureAccept(8527);
            Core.huntMonster("eridanipast", "Bat", "Bat Wings", 8);
            Core.huntMonster("eridanipast", "Dog", "Dog Released", 8);
            Core.ensureComplete(8527);
        }

        //Aeolian
        Story.killQuest(8528, "eridanipast", "Bandit");

        //Troubadour
        if (!Core.isCompletedBefore(8529)) {
            Core.ensureAccept(8529);
            Core.huntMonsterMapID("eridanipast", 18, "Paladin Dueled");
            Core.ensureComplete(8529);
        }

        //Echoes
        if (!Story.questProgression(8530)) {
            Core.ensureAccept(8530);
            Core.huntMonster("eridanipast", "Bandit", "Bandit Remnants", 7);
            Story.mapItemQuest(8530, "eridanipast", 9680);
        }
    }

    astraviaPast(): void {
        //Progress Check
        if (Core.isCompletedBefore(8601)) {
            Core.logger("StoryLine Complete:   AstraviaPast");
            return;
        }

        this.eridaniPast();

        //Preload Quests
        Story.preLoad(this.astraviaPast);

        //Fantaisie-Impromptu
        Story.mapItemQuest(8592, "astraviapast", 10017);
        Story.mapItemQuest(8592, "astraviapast", 10018);

        //Odile
        Story.killQuest(8593, "astraviapast", "Astravian Soldier");

        //Revolutionary Etude
        Story.mapItemQuest(8594, "astraviapast", 10019, 7);

        //Tannhauser
        Story.killQuest(8595, "astraviapast", "Panicked Citizen");

        //Tzigane
        if (!Story.questProgression(8596)) {
            Core.ensureAccept(8596);
            Core.getMapItem(10020, 1, "astraviapast");
            Core.huntMonster("astraviapast", "Astravian Soldier", "Soldiers Disciplined", 8);
            Core.ensureComplete(8596);
        }

        //Ride of the Valkyrie
        Core.equipClass(ClassType.Solo);
        Story.killQuest(8597, "astraviapast", "Regulus");

        //Wedding March
        Story.killQuest(8598, "astraviapast", "Titania");

        //From the New World
        Story.killQuest(8599, "astraviapast", "Aurola");

        //Mov.VII Aquarium
        Story.mapItemQuest(8600, "astraviapast", 10021);

        //Liebestraume No. 3
        Story.killQuest(8601, "astraviapast", "Forsaken Husk");
    }

    firstObservatory(): void {
        //Progress Check
        if (Core.isCompletedBefore(8641)) {
            Core.logger("StoryLine Complete:   FirstObservatory");
            return;
        }

        //Preload Quests
        Story.preLoad(this.firstObservatory);

        //Aries Tammuz
        Story.mapItemQuest(8630, "FirstObservatory", 10083);

        //Scorpio
        Story.killQuest(8631, "FirstObservatory", "Astra Scorpio");

        //Leo
        Story.killQuest(8632, "FirstObservatory", "Astra Leo");

        //Lugal-irra Meslamta-ea
        if (!Story.questProgression(8633)) {
            Core.ensureAccept(8633);
            Core.getMapItem(10084, 1, "FirstObservatory");
            Core.huntMonster("firstobservatory", "Ancient Turret", "Turrets Destroyed", 8);
            Core.ensureComplete(8633);
        }

        //Virgo Shala
        Story.mapItemQuest(8634, "FirstObservatory", 10085);

        //Sagittarius Pabilsag
        Story.killQuest(8635, "FirstObservatory", "Ancient Creature");

        //Libra Shamash
        Story.mapItemQuest(8636, "FirstObservatory", [ 10086, 10087 ]);

        //Auriga
        if (!Story.questProgression(8637)) {
            Core.ensureAccept(8637);
            Core.huntMonster("firstobservatory", "Ancient Turret", "Turret Parts", 5);
            Core.huntMonster("firstobservatory", "Ancient Creature", "Creature Limbs", 5);
            Core.ensureComplete(8637);
        }

        //Pisces Alrescha
        Story.mapItemQuest(8638, "FirstObservatory", [ 10088, 10089 ]);

        //Pisces Alpherg
        Story.mapItemQuest(8639, "FirstObservatory", 10090);

        //Pisces Ishtar
        Story.killQuest(8640, "FirstObservatory", "Empress’ Finger");

        //Taurus Gugalanna
        if (!Story.questProgression(8641)) {
            Core.ensureAccept(8641);
            Core.huntMonster("firstobservatory", "Ancient Turret", "Turret Pieces", 12);
            Core.huntMonster("firstobservatory", "Empress’ Finger", "Alprecha Observed", 1);
            Core.huntMonster("firstobservatory", "Ancient Creature", "Creature Samples", 6);
            Core.ensureComplete(8641);
        }
    }

    genesisGarden(): void {
        if (Core.isCompletedBefore(8687)) {
            Core.logger("StoryLine Complete:   GenesisGarden");
            return;
        }

        this.firstObservatory();

        Story.preLoad(this.genesisGarden);

        //The Fool 8678
        Story.mapItemQuest(8678, "genesisgarden", 10196, 6);

        //The Magician 8679
        Story.killQuest(8679, "genesisgarden", "Drago's Soldier");

        //The High Priestess 8680
        Story.mapItemQuest(8680, "genesisgarden", [ 10197, 10198, 10199 ]);

        //The Empress, Reversed 8681
        Story.killQuest(8681, "genesisgarden", "Drago's Soldier");

        //The Emperor 8682
        Story.mapItemQuest(8682, "genesisgarden", [ 10200, 10201 ]);

        //The Hierophant 8683
        Story.mapItemQuest(8683, "genesisgarden", 10202, 5);

        //The Lovers, Reversed 8684
        if (!Story.questProgression(8684)) {
            Core.ensureAccept(8684);
            Core.huntMonster("genesisgarden", "Ancient Creature", "Creature Sample", 8);
            Core.huntMonster("genesisgarden", "Plant Beast", "Beast Sample", 8);
            Core.ensureComplete(8684);
        }

        //The Chariot 8685
        if (!Story.questProgression(8685)) {
            Core.ensureAccept(8685);
            Core.getMapItem(10203, 1, "genesisgarden");
            Core.huntMonster("genesisgarden", "Ancient Turret", "Machine Parts", 8);
            Core.ensureComplete(8685);
        }

        //Strength 8686
        Story.killQuest(8686, "genesisgarden", "Undead Humanoid");

        //The Hermit 8687
        Story.killQuest(8687, "genesisgarden", "Ancient Mecha");
    }

    theWorld(): void {
        if (Core.isCompletedBefore(8733)) {
            Core.logger("StoryLine Complete:   TheWorld");
            return;
        }

        this.genesisGarden();

        Story.preLoad(this.theWorld);

        //8723|Justice, Reversed
        Story.mapItemQuest(8723, "theworld", 10289);

        //8724|The Hanged Man, Reversed
        Story.killQuest(8724, "theworld", "Nothingness");

        //8725|Death
        Story.killQuest(8725, "theworld", "Re");

        //8726|Temperance, Reversed
        Story.killQuest(8726, "theworld", "Fa");

        //8727|Temptation, Reversed
        Story.killQuest(8727, "theworld", "Ti");

        //8728|The Tower
        Story.killQuest(8728, "theworld", "So");

        //8729|The Star
        Story.killQuest(8729, "theworld", "Nothingness");

        //8730|The Moon, Reversed
        Story.killQuest(8730, "theworld", "So");

        //8731|The Sun, Reversed
        Story.killQuest(8731, "theworld", "So");

        //8732|Judgement
        Story.killQuest(8732, "theworld", "Darkon");

        //8733|The World
        if (!Story.questProgression(8733)) {
            Core.ensureAccept(8733);
            Core.huntMonsterMapID("theworld", 12, "Encore Darkon Defeated");
            Core.ensureComplete(8733);
        }
    }
}
