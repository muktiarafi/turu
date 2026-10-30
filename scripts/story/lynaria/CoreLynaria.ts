/*
name: null
description: null
tags: null
*/
// Port of Scripts/Story/Lynaria/CoreLynaria.cs.

import { ClassType, Core } from "../../core/CoreBots";
import { CoreStory } from "../../core/CoreStory";

const Story = new CoreStory();

export class CoreLynaria {
    doAll(): void {
        this.bocklinGrove();
        this.bocklinCastle();
        this.bocklinSanctum();
    }

    bocklinGrove(): void {
        if (Core.isCompletedBefore(10239))
            return;

        Story.preLoad(this.bocklinGrove);

        //#region Useable Monsters
        const UseableMonsters = [
            "Wolf", // UseableMonsters[0],
            "Spider", // UseableMonsters[1],
            "Sp-Eye", // UseableMonsters[2],
            "Garde Grif", // UseableMonsters[3],
            "Undead Garde", // UseableMonsters[4],
            "Garde Wraith", // UseableMonsters[5],
            "Forsaken Necromancer", // UseableMonsters[6],
            "Elder Necromancer", // UseableMonsters[7]
        ];
        //#endregion Useable Monsters

        // 10230 | Sleeping Diana
        if (!Story.questProgression(10230)) {
            Core.ensureAccept(10230);
            Core.getMapItem(14429, 1, "bocklingrove");
            Core.huntMonster("bocklingrove", "Wolf", "Playfight with Wolves", 12);
            Core.ensureComplete(10230);
        }

        // 10231 | At the Edge of the Forest
        if (!Story.questProgression(10231)) {
            Core.huntMonsterQuest(10231, ["bocklingrove", UseableMonsters[1], ClassType.Farm]);
        }

        // 10232 | Freedom Helvetia
        if (!Story.questProgression(10232)) {
            Core.huntMonsterQuest(10232, ["bocklingrove", UseableMonsters[2], ClassType.Farm]);
        }

        // 10233 | Sacred Grove
        Story.mapItemQuest(10233, "bocklingrove", [ 14430, 14431 ]);

        // 10234 | Chained Prometheus
        if (!Story.questProgression(10234)) {
            Core.huntMonsterQuest(10234, ["bocklingrove", UseableMonsters[3], ClassType.Farm]);
        }

        // 10235 | Ride of Death
        if (!Story.questProgression(10235)) {
            Core.huntMonsterQuest(10235, ["bocklingrove", UseableMonsters[4], ClassType.Farm]);
        }

        // 10236 | The Fall and Death
        if (!Story.questProgression(10236)) {
            Core.huntMonsterQuest(10236, ["bocklingrove", UseableMonsters[5], ClassType.Farm]);
        }

        // 10237 | Ruins in Moonlight
        Story.mapItemQuest(10237, [ [14432, 1, "bocklingrove"], [14433, 5, "bocklingrove"] ]);

        // 10238 | Wandering Light
        if (!Story.questProgression(10238)) {
            Core.huntMonsterQuest(
                10238, 
                ["bocklingrove", UseableMonsters[5], ClassType.Farm],
                ["bocklingrove", UseableMonsters[4], ClassType.Farm]);
        }

        // 10239 | Death Plays the Fiddle
        if (!Story.questProgression(10239)) {
            Core.huntMonsterQuest(10239, ["bocklingrove", UseableMonsters[7], ClassType.Solo]);
        }
    }

    bocklinCastle(): void {
        if (Core.isCompletedBefore(10252))
            return;

        this.bocklinGrove();
        Story.preLoad(this.bocklinCastle);

        //#region Useable Monsters
        const UseableMonsters = [
            "Undead Garde", // UseableMonsters[0],
            "Garde Wraith", // UseableMonsters[1],
            "Warped Revenant", // UseableMonsters[2],
            "Forsaken Necromancer", // UseableMonsters[3],
            "Faceless Ritualist", // UseableMonsters[4],
            "Headless Knight", // UseableMonsters[5]
        ];
        //#endregion Useable Monsters

        // 10243 | Roger and Angelica
        if (!Story.questProgression(10243)) {
            if (!Story.questProgression(10243)) {
                Core.ensureAccept(10243);
                Core.getMapItem(14446, 1, "bocklincastle");
                Core.huntMonster("bocklincastle", "Undead Garde", "Undead Sigils", 9);
                Core.ensureComplete(10243);
            }
        }

        // 10244 | The Way to Emmaus
        if (!Story.questProgression(10244)) {
            Core.huntMonsterQuest(10244, ["bocklincastle", UseableMonsters[1], ClassType.Farm]);
        }

        // 10245 | Hermit
        Story.mapItemQuest(10245, "bocklincastle", [ 14451, 14447 ]);

        // 10246 | Ruins Near Kehl
        if (!Story.questProgression(10246)) {
            Core.huntMonsterQuest(
                10246, 
                ["bocklincastle", UseableMonsters[0], ClassType.Farm],
                ["bocklincastle", UseableMonsters[1], ClassType.Farm]);
        }

        // 10247 | Whistling Blackbird
        if (!Story.questProgression(10247)) {
            Core.huntMonsterQuest(10247, ["bocklincastle", UseableMonsters[2], ClassType.Farm]);
        }

        // 10248 | Killer Pursued by Furies
        if (!Story.questProgression(10248)) {
            Core.ensureAccept(10248);
            Core.getMapItem(14448, 1, "bocklincastle");
            Core.huntMonster("bocklincastle", "Forsaken Necromancer", "Necromancer's Broken Soul", 1);
            Core.ensureComplete(10248);
        }

        // 10249 | Vestal
        Story.mapItemQuest(10249, "bocklincastle", 14449, 6);

        // 10250 | Abandoned Venus
        if (!Story.questProgression(10250)) {
            Core.ensureAccept(10250);
            Core.getMapItem(14450, 1, "bocklincastle");
            Core.huntMonster("bocklincastle", "Faceless Ritualist", "White Hood", 12);
            Core.ensureComplete(10250);
        }

        // 10251 | Bacchanalia
        if (!Story.questProgression(10251)) {
            Core.huntMonsterQuest(
                10251, 
                ["bocklincastle", UseableMonsters[2], ClassType.Farm],
                ["bocklincastle", UseableMonsters[4], ClassType.Farm]);
        }

        // 10252 | Euterpe and Deer
        if (!Story.questProgression(10252)) {
            Core.huntMonsterQuest(10252, ["bocklincastle", UseableMonsters[5], ClassType.Solo]);
        }
    }

    bocklinSanctum(): void {
        //#region Useable Monsters
        const UseableMonsters = [
            "Warped Revenant", // UseableMonsters[0],
            "Faceless Ritualist", // UseableMonsters[1],
            "Tarnished Scion", // UseableMonsters[2],
            "Ghostly Servant", // UseableMonsters[3],
            "Headless Rider", // UseableMonsters[4],
            "Thronekeeper", // UseableMonsters[5]
        ];
        //#endregion Useable Monsters

        if (Core.isCompletedBefore(10265))
            return;

        this.bocklinCastle();
        Story.preLoad(this.bocklinSanctum);

        // 10256 | Shot in the Dark
        if (!Story.questProgression(10256)) {
            Core.huntMonsterQuest(10256, ["bocklinsanctum", UseableMonsters[0], ClassType.Farm]);
        }

        // 10257 | Sparkling Changeling
        if (!Story.questProgression(10257)) {
            Core.huntMonsterQuest(10257, ["bocklinsanctum", UseableMonsters[1], ClassType.Farm]);
        }

        // 10258 | Silver Proof
        if (!Story.questProgression(10258)) {
            Story.mapItemQuest(10258, "bocklinsanctum", 14478, 6);
        }

        // 10259 | Gilded Peace
        if (!Story.questProgression(10259)) {
            Core.huntMonsterQuest(
                10259, 
                ["bocklinsanctum", UseableMonsters[1], ClassType.Farm],
                ["bocklinsanctum", UseableMonsters[0], ClassType.Farm]);
        }

        // 10260 | Towards Reunion
        if (!Story.questProgression(10260)) {
            Core.equipClass(ClassType.Solo);
            if (!Story.questProgression(10260)) {
                Core.ensureAccept(10260);
                Core.getMapItem(14479, 1, "bocklinsanctum");
                Core.huntMonster("bocklinsanctum", "Tarnished Scion", "Scion's Crest", 1);
                Core.ensureComplete(10260);
            }
        }

        // 10261 | Servants Stay Unseen
        if (!Story.questProgression(10261)) {
            Core.equipClass(ClassType.Farm);
            if (!Story.questProgression(10261)) {
                Core.ensureAccept(10261);
                Core.getMapItem(14480, 1, "bocklinsanctum");
                Core.huntMonster("bocklinsanctum", "Ghostly Servant", "Ghost Appeased", 15);
                Core.ensureComplete(10261);
            }
        }

        // 10262 | The Slaying of Sacred Deer
        if (!Story.questProgression(10262)) {
            Core.huntMonsterQuest(10262, ["bocklinsanctum", UseableMonsters[4], ClassType.Farm]);
        }

        // 10263 | The Light Protects You
        if (!Story.questProgression(10263)) {
            Story.mapItemQuest(10263, "bocklinsanctum", [ 14481, 14482 ]);
        }

        // 10264 | Cure-All
        if (!Story.questProgression(10264)) {
            Core.huntMonsterQuest(
                10264, 
                ["bocklinsanctum", UseableMonsters[3], ClassType.Solo],
                ["bocklinsanctum", UseableMonsters[4], ClassType.Solo]);
        }

        // 10265 | The Thronekeeper
        if (!Story.questProgression(10265)) {
            Core.huntMonsterQuest(10265, ["bocklinsanctum", UseableMonsters[5], ClassType.Solo]);
        }
    }
}
