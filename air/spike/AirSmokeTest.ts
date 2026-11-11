/*
name: AIR Smoke Test
description: For Turu's AIR build (docs/AIR_PROPOSAL.md): reads the game, joins Yulgar, then logs the player every 3 s until stopped. Changes nothing on the account.
tags: test
*/
import { bot } from "turu";

export default function main(): void {
    bot.log(`AIR: ${bot.player.username}, level ${bot.player.level}, in ${bot.map.name} (${bot.player.cell})`);
    bot.map.join("yulgar");
    bot.log(`AIR: joined ${bot.map.name}`);
    for (let i = 1; ; i++) {
        bot.log(`AIR ${i}: ${bot.map.name} ${bot.player.cell}, hp ${bot.player.health}/${bot.player.maxHealth}, gold ${bot.player.gold}`);
        bot.sleep(3000);
    }
}
