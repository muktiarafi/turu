/*
name: null
description: null
tags: null
*/
// Port of Scripts/Story/SepulchureSaga/CoreSepulchure.cs.
// Not ported (FarmerJoe's DoAll doesn't reach them): CompleteSS, Alden, Lynaria, SepulchuresRise.

import { ClassType, Core } from "../../core/CoreBots";
import { CoreStory } from "../../core/CoreStory";

const Story = new CoreStory();

export class CoreSepulchure {
    shadowfallRise(): void {
        if (Core.isCompletedBefore(6592))
            return;

        Story.preLoad(this.shadowfallRise);

        // Fluid Dynamics 6539
        Core.equipClass(ClassType.Farm);
        Story.killQuest(6539, "noxustower", "Slimeskull");

        // M-ossification 6540
        Story.killQuest(6540, "noxustower", "Doomwood Treeant");

        // A-gore-able 6541
        Story.killQuest(6541, "noxustower", "Sanguine Souleater");

        // Raw-r 6542
        Story.killQuest(6542, "noxustower", "Lightguard Caster");

        // Light 'em Up 6543
        Story.killQuest(6543, "noxustower", "Lightguard Caster");

        // Spellbound 6544
        if (!Story.questProgression(6544)) {
            Core.ensureAccept(6544);
            Core.getMapItem(6018, 4, "noxustower");
            Core.huntMonster("noxustower", "Lightguard Caster", "Lightguard Scroll", 4);
            Core.ensureComplete(6544);
        }

        // Lupine Resurrection 6545
        Story.killQuest(6545, "noxustower", "Lightguard Wolf");

        // Spying on the Light 6546
        Story.killQuest(6546, "noxustower", "Lightguard Paladin");

        // Illusory Disguise 6547
        if (!Story.questProgression(6547)) {
            Core.ensureAccept(6547);
            Core.huntMonster("noxustower", "Slimeskull", "Necrotic Ooze", 3);
            Core.huntMonster("noxustower", "Lightguard Caster", "Lightguard Armor", 4);
            Core.huntMonster("noxustower", "Doomwood Treeant", "Arcane Energy", 3);
            Core.ensureComplete(6547);
        }

        // Test the Disguise 6548
        Story.mapItemQuest(6548, "noxustower", 6019);

        // Get Alteon 6549
        Story.mapItemQuest(6549, "noxustower", 6020);

        // Hammer Time 6550
        Story.killQuest(6550, "noxustower", "General Goldhammer");

        // Lightguard Medals 6560
        Story.killQuest(6560, "lightguardwar", "Citadel Crusader");

        // Mega Lightguard Medals 6561
        Story.killQuest(6561, "lightguardwar", "Citadel Crusader");

        // Bone Marrow 6562
        Story.killQuest(6562, "lightguardwar", "Citadel Crusader");

        // Ooze it up 6563
        Story.killQuest(6563, "lightguardwar", "Slimeskull");

        // Seize the Seige (Engine) 6564
        Story.killQuest(6564, "lightguardwar", "Lightguard Engine");

        // Darken the Light 6565
        Story.killQuest(6565, "lightguardwar", "Scorching Flame");

        // Get the Powder 6566
        Story.killQuest(6566, "lightguardwar", "Citadel Crusader");

        // Destroy the Shield 6567
        Story.killQuest(6567, "lightguardwar", "Sigrid Sunshield");

        // Such a Mess 6581
        if (!Story.questProgression(6581)) {
            Core.ensureAccept(6581);
            Core.getMapItem(6098, 8, "lumafortress");
            Core.huntMonster("lumafortress", "Invasive Shadow", "Shadows Defeated", 8);
            Core.ensureComplete(6581);
        }

        // Herbs Needed 6582
        if (!Story.questProgression(6582)) {
            Core.ensureAccept(6582);
            Core.getMapItem(6099, 7, "lumafortress");
            Core.huntMonster("lumafortress", "Light Treeant", "Glowmoss", 6);
            Core.ensureComplete(6582);
        }

        // Shine a Light 6583
        Story.killQuest(6583, "lumafortress", "Light Elemental");

        // Hapless? 6584
        Story.killQuest(6584, "lumafortress", "Hapless Skeleton");

        // How 'bout them Apples 6585
        Story.killQuest(6585, "lumafortress", "Light Treeant");

        // Make an Offering 6586
        if (!Story.questProgression(6586)) {
            Core.ensureAccept(6586);
            Core.getMapItem(6100, 1, "lumafortress");
            Core.huntMonster("lumafortress", "Lightwing", "Brilliant Feather", 1);
            Core.ensureComplete(6586);
        }

        // Test the Spell Again! 6587
        Story.mapItemQuest(6587, "lumafortress", 6101);

        // Gather the Light 6588
        Story.killQuest(6588, "lumafortress", "Light Elemental");

        // Banish the Shadows 6589
        Story.killQuest(6589, "lumafortress", "Living Shadow");

        // So Many Minions 6590
        Story.killQuest(6590, "lumafortress", "Skeleton Minion");

        // Corrupted Light 6591
        if (!Story.questProgression(6591)) {
            Core.ensureAccept(6591);
            Core.equipClass(ClassType.Solo);
            Core.killMonster("lumafortress", "r4", "Left", "Corrupted Luma", "Luma Defeated");
            Core.ensureComplete(6591);
        }

        // Star Light Star Bright 6592
        Story.killQuest(6592, "lumafortress", "Light Elemental");
    }
}
