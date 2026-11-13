/*
name: R3d options check
description: Turns Hide Players, Disable FX, Disable Collisions and Lag Killer on for 10 s each, then off, through Turu's own options, to check turu.swf's options and modules by eye and by log. Stand in a busy map cell (with other players) first.
*/
import { bot } from "turu";

export default function main(): void {
    const call = (name: string, ...args: unknown[]): string => bot.flash.call(name, ...args);
    const solid = (): string => call("getGameObject", "world.arrSolid.length");
    bot.log(`walls before: ${solid()}`);

    bot.options.hidePlayers = true;
    bot.log("Hide Players on: other players should be gone");
    bot.sleep(10000);
    bot.options.hidePlayers = false;
    bot.log("Hide Players off: they should be back");
    bot.sleep(3000);

    bot.options.disableFx = true;
    bot.log("Disable FX on");
    bot.sleep(10000);
    bot.options.disableFx = false;
    bot.log("Disable FX off");
    bot.sleep(3000);

    bot.options.disableCollisions = true;
    bot.sleep(1000);
    bot.log(`Disable Collisions on: walls ${solid()}`);
    bot.sleep(9000);
    bot.options.disableCollisions = false;
    bot.sleep(1000);
    bot.log(`Disable Collisions off: walls ${solid()}`);

    bot.options.lagKiller = true;
    bot.sleep(1000);
    bot.log(`Lag Killer on: world visible ${call("getGameObject", "world.visible")}`);
    bot.sleep(9000);
    bot.options.lagKiller = false;
    bot.sleep(1000);
    bot.log(`Lag Killer off: world visible ${call("getGameObject", "world.visible")}`);
}
