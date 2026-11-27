// The script of `relogin.tape.jsonl.gz` (docs/REPLAY_TESTS_PROPOSAL.md, phase 3): the engine's
// relogin (`bot.servers.relogin`) back onto the quiet server it logged into, never a crowded one.
// Recorded in game with `tools/farmrun/run.sh` and `TURU_TAPE`; `bot/tests/replay.rs` replays it.
import { bot } from "turu";

const SERVER = "Twig";

export default function main(): void {
    for (let i = 0; i < 60 && !bot.player.loaded; i++) bot.sleep(500);
    bot.log(`Before: logged in ${bot.player.loggedIn}, map ${bot.map.name}`);
    const relogged = bot.servers.relogin(SERVER);
    for (let i = 0; i < 60 && !bot.player.loaded; i++) bot.sleep(500);
    bot.log(`Relogin ${relogged ? "done" : "failed"}: logged in ${bot.player.loggedIn}, map ${bot.map.name}`);
}
