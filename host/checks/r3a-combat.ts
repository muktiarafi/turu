/*
name: R3a combat check
description: Attacks the monsters in your cell for about 30 s to check turu.swf's combat callbacks (watch the outgoing attack packets in the log).
*/
import { bot } from "turu";

export default function main(): void {
    const call = (name: string, ...args: unknown[]): string => bot.flash.call(name, ...args);
    bot.log(`monsters here: ${call("availableMonsters").slice(0, 200)}`);
    call("infiniteRange");
    for (let round = 0; round < 60; round++) {
        if (call("attackMonsterName", "*") !== "true") {
            bot.log("no monster in this cell");
            bot.sleep(1000);
            continue;
        }
        for (let skill = 1; skill <= 4; skill++) {
            if (call("canUseSkill", skill) === "true") {
                call("useSkill", skill);
                bot.sleep(300);
            }
        }
        bot.sleep(500);
    }
    call("untargetSelf");
    bot.log(`target: ${call("getTargetMonster").slice(0, 200)}`);
    bot.log(`my auras: ${call("GetAuraSnapshots", "Self").slice(0, 200)}`);
}
