/*
name: R3c login check
description: Relogs twice on the quietest free server (never Twilly, Artix or Alteon): once by name (connectToServer), once from the server list (clickServer). Uses only R2-R3c's callbacks, so it runs on turu.swf alone and on VibeSkua's SWF. Log in by hand first; give it about two minutes.
*/
import { bot } from "turu";

/** Crowded servers a check mustn't relog on. */
const CROWDED = ["twilly", "artix", "alteon"];

export default function main(): void {
    const call = (name: string, ...args: unknown[]): string => bot.flash.call(name, ...args);
    const quiet = bot.servers
        .getServers(true)
        .filter((s) => s.online && !s.upgrade && s.playerCount < s.maxPlayers)
        .filter((s) => !CROWDED.some((c) => s.name.toLowerCase().includes(c)))
        .sort((a, b) => a.playerCount - b.playerCount)[0];
    if (!quiet) {
        bot.log("No quiet free server online: not relogging.");
        return;
    }
    bot.log(`Relogging on ${quiet.name} (${quiet.playerCount}/${quiet.maxPlayers}).`);

    // By name: the direct connection (connectToServer).
    bot.log(`relogin by name: ${bot.servers.relogin(quiet.name)}`);
    bot.log(`logged in: ${call("isLoggedIn")}, map: ${bot.map.name}`);
    bot.sleep(5000);

    // From the server list (clickServer).
    bot.servers.disconnectForRelogin();
    bot.log(`from the list: ${bot.servers.ensureRelogin(quiet.name)}`);
    bot.log(`logged in: ${call("isLoggedIn")}, map: ${bot.map.name}, modal: [${call("getActiveModalMessage")}]`);
}
