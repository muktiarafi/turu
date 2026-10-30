/*
name: Combat Trophy
description: This script will farm Combat Trophies in /bludrutbtawl
tags: combat, trophy, pvp, bludrut, brawl
*/
// Port of Scripts/Farm/CombatTrophy.cs (includes CoreBots, CoreFarms; CoreAdvanced is included but
// never used by this script).

import { Core } from "../core/CoreBots";
import { CoreFarms } from "../core/CoreFarms";

const Farm = new CoreFarms();

export default function main(): void {
    Core.bankingBlackList.push("The Secret 4", "Yoshino's Citrine", "Combat Trophy");
    Core.setOptions();

    doCombatTrophy();

    Core.setOptions(false);
}

export function doCombatTrophy(): void {
    Core.logger(
        '90% of this farm is going to be transitioning rooms during pvp. There is nothing we can do to speed this up as it takes 2 return packets from the server a move and a "mcid" packet to actualy transition rooms. ( no you can\'t jump cells as itll soft-lock the map, and yes we\'ve tried teh walking method its **Very** unrelaible as you can just get stuck at doors.)',
    );
    Farm.bludrutBrawlBoss();
}
