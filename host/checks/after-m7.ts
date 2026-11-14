/*
name: After M7 check
description: Checks turu.swf version 9 in game: questStates against the single reads it replaces, and the Lag Killer module keeping the world hidden; logs MATCH or DIFF for each.
*/
import { bot } from "turu";

export default function main(): void {
    const same = (label: string, have: unknown, want: unknown): void => {
        const a = JSON.stringify(have);
        const b = JSON.stringify(want);
        bot.log(`${a === b ? "MATCH" : "DIFF "} ${label}: ${a.slice(0, 120)}${a === b ? "" : ` vs ${b.slice(0, 120)}`}`);
    };

    // questStates: Arcangrove's quests (794-801), one accepted, and 1, which isn't loaded.
    bot.quests.load(794, 795, 796);
    bot.sleep(2000);
    bot.quests.accept(794);
    bot.sleep(1500);
    const ids = [794, 795, 796, 1];
    const raw = bot.flash.call("questStates", ids);
    const states = JSON.parse(raw) as Record<string, { inProgress: boolean; quest: unknown }>;
    for (const id of ids) {
        const state = states[String(id)];
        same(`${id} in progress`, state?.inProgress, bot.flash.callGameFunction("world.isQuestInProgress", id) === true);
        same(`${id} tree entry`, state?.quest ?? null, bot.flash.get(`world.questTree.${id}`) ?? null);
    }

    // Lag Killer: hidden once the options pass turns the module on, and kept hidden.
    bot.options.lagKiller = true;
    bot.sleep(1000);
    same("Lag Killer hides the world", bot.flash.get("world.visible"), false);
    bot.flash.set("world.visible", true);
    bot.sleep(500);
    same("kept hidden after the game shows it", bot.flash.get("world.visible"), false);
    bot.options.lagKiller = false;
    bot.sleep(1000);
    same("shown again when it's off", bot.flash.get("world.visible"), true);
}
