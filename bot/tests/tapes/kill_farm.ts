// The script of `kill_farm.tape.jsonl.gz` (docs/REPLAY_TESTS_PROPOSAL.md, phase 3): a kill farm
// with drops on a high-level character. CoreBots' `huntMonster` farms three more Bone Dust from
// Skeleton Warriors (battleunderb, as CoreFarms does), its drop handling picking them up. Combat
// brings the skill timer (its sends unchecked, decision 7). Nothing here relogs. Recorded in game
// with `tools/farmrun/run.sh` and `TURU_TAPE`; `bot/tests/replay.rs` replays it.
import { bot } from "turu";
import { Core } from "../../../scripts/core/CoreBots";

const ITEM = "Bone Dust";
const MORE = 3;

function owned(): number {
    return bot.inventory.items.find((i) => i.name === ITEM)?.quantity ?? 0;
}

export default function main(): void {
    Core.setOptions();
    Core.unbank(ITEM);
    const before = owned();
    const max = bot.inventory.items.find((i) => i.name === ITEM)?.maxStack ?? 0;
    bot.log(`Start: level ${bot.player.level}, ${ITEM} ${before}`);
    if (max > 0 && before + MORE > max) {
        bot.log(`${ITEM} is near its stack of ${max}: nothing to farm`);
    } else {
        Core.huntMonster("battleunderb", "Skeleton Warrior", ITEM, before + MORE, { isTemp: false });
        bot.log(`Farmed: ${ITEM} ${before} to ${owned()}`);
    }
    // Turns back on the social options `setOptions` turned off.
    Core.setOptions(false);
}
