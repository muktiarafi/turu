// The script of `chute_quest.tape.jsonl.gz` (docs/REPLAY_TESTS_PROPOSAL.md, phase 2): quest
// 8543 in /chute ("You Chute, You Score", the badge's quest), accepted, its map item picked up and
// turned in, three times. No combat, so only the script thread sends. Recorded in game with
// `tools/farmrun/run.sh` and `TURU_TAPE`; `bot/tests/replay.rs` replays it.
import { bot } from "turu";

const QUEST = 8543;
const ITEM = 9837;

export default function main(): void {
    // `join` does nothing until the player is loaded (the inventory arrives after the login).
    for (let i = 0; i < 60 && !bot.player.loaded; i++) bot.sleep(500);
    bot.map.join("chute");
    if (bot.map.name.toLowerCase() !== "chute") {
        bot.log(`Not in /chute but ${bot.map.name}`);
        return;
    }
    for (let i = 1; i <= 3; i++) {
        if (!bot.quests.ensureAccept(QUEST)) {
            bot.log(`Turn-in ${i}: quest ${QUEST} wasn't accepted`);
            return;
        }
        bot.map.getMapItem(ITEM);
        bot.sleep(1000);
        const done = bot.quests.ensureComplete(QUEST);
        bot.log(`Turn-in ${i}: ${done ? "done" : "not done"}`);
    }
}
