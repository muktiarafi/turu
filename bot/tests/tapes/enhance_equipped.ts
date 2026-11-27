// The script of `enhance_equipped.tape.jsonl.gz` (docs/REPLAY_TESTS_PROPOSAL.md, phase 3):
// CoreAdvanced's `enhanceEquipped` on a level-16 free character's class gear, Lucky, through the
// real enhancement shops. Recorded in game with `tools/farmrun/run.sh` and `TURU_TAPE`;
// `bot/tests/replay.rs` replays it.
import { bot } from "turu";
import { Core } from "../../../scripts/core/CoreBots";
import { CoreAdvanced, EnhancementType } from "../../../scripts/core/CoreAdvanced";

const Adv = new CoreAdvanced();

function equipped(): string {
    return bot.inventory.items
        .filter((i) => i.equipped)
        .map((i) => `${i.name} ${i.enhancementLevel}`)
        .join(", ");
}

export default function main(): void {
    Core.setOptions();
    bot.log(`Before: ${equipped()}`);
    Adv.enhanceEquipped(EnhancementType.Lucky);
    bot.log(`After: ${equipped()}`);
    Core.setOptions(false);
}
