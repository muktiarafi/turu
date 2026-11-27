// The script of `bank_moves.tape.jsonl.gz` (docs/REPLAY_TESTS_PROPOSAL.md, phase 3): CoreBots'
// `toBank` and `unbank` on a resource, there and back. Recorded in game with
// `tools/farmrun/run.sh` and `TURU_TAPE`; `bot/tests/replay.rs` replays it.
import { bot } from "turu";
import { Core } from "../../../scripts/core/CoreBots";

const ITEM = "Gear of Doom";

export default function main(): void {
    Core.setOptions();
    Core.toBank(ITEM);
    bot.log(`Banked: in the bank ${bot.bank.contains(ITEM)}, in the inventory ${bot.inventory.contains(ITEM)}`);
    Core.unbank(ITEM);
    bot.log(`Unbanked: in the bank ${bot.bank.contains(ITEM)}, in the inventory ${bot.inventory.contains(ITEM)}`);
    Core.setOptions(false);
}
