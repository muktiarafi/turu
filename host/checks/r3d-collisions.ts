/*
name: R3d collisions probe
description: Tells whether Disable Collisions reaches the host SWF and whether the walls stay gone. Stand in a cell with walls (most town cells) first.
*/
import { bot } from "turu";

export default function main(): void {
    const call = (name: string, ...args: unknown[]): string => bot.flash.call(name, ...args);
    const walls = (): string => call("getGameObject", "world.arrSolid.length");
    bot.log(`walls: ${walls()}, playing: ${bot.player.playing}`);

    // Through Turu's option, as a script sets it.
    bot.options.disableCollisions = true;
    const seen: string[] = [];
    for (let i = 0; i < 8; i++) {
        bot.sleep(250);
        seen.push(walls());
    }
    bot.log(`option on, every 250 ms: ${seen.join(" ")}`);
    bot.options.disableCollisions = false;
    bot.sleep(1000);
    bot.log(`option off: ${walls()}`);

    // The callback itself.
    call("modEnable", "DisableCollisions");
    const right = walls();
    bot.sleep(500);
    bot.log(`modEnable direct: at once ${right}, after 500 ms ${walls()}`);
    call("modDisable", "DisableCollisions");
    bot.sleep(500);
    bot.log(`modDisable direct: ${walls()}`);
}
