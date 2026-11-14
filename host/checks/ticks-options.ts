/*
name: Kept options check
description: Checks what turu.swf now keeps applied itself (version 7-8): Infinite Range's skill ranges and Lite's untarget settings; logs MATCH or DIFF for each.
*/
import { bot } from "turu";

export default function main(): void {
    const same = (label: string, have: unknown, want: unknown): void => {
        const ok = JSON.stringify(have) === JSON.stringify(want);
        bot.log(`${ok ? "MATCH" : "DIFF "} ${label}: ${JSON.stringify(have)}${ok ? "" : ` (want ${JSON.stringify(want)})`}`);
    };
    const ranges = (): unknown[] => {
        const out: unknown[] = [];
        for (let i = 0; i < 6; i++) {
            out.push(bot.flash.get(`world.actions.active.${i}.range`));
        }
        return out.filter((r) => r !== null && r !== undefined);
    };

    const before = ranges();
    bot.log(`ranges before: ${JSON.stringify(before)}`);
    bot.options.infiniteRange = true;
    // The options pass switches the module on (every 250 ms); turu.swf applies it at once.
    bot.sleep(1500);
    const on = ranges();
    same("every skill reaches across the map", on.every((r) => r === 20000) && on.length > 0, true);

    // The game puts its own range back on a skill (as a class change does); turu.swf keeps
    // reapplying it every 100 ms.
    bot.flash.set("world.actions.active.1.range", 301);
    bot.sleep(500);
    same("kept after the game resets one", bot.flash.get("world.actions.active.1.range"), 20000);

    same("Lite untarget self", bot.flash.get("litePreference.data.bUntargetSelf"), true);
    same("Lite untarget dead", bot.flash.get("litePreference.data.bUntargetDead"), true);

    bot.options.infiniteRange = false;
    bot.sleep(1000);
    bot.log(`ranges after turning it off (the game's own come back on its next class change): ${JSON.stringify(ranges())}`);
}
