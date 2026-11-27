// The script of `merge_buy.tape.jsonl.gz` (docs/REPLAY_TESTS_PROPOSAL.md, phase 3): the merge
// template on its smallest shop, SynderesMerge (scripts/other/mergeshops/SynderesMerge.ts), buying
// only "NO BOTS Armor" (one Synderes' Souvenir and 1,000 gold). The account has no souvenir, so
// the template hands it to `findIngredients`, which farms one through quest 4247 (Evil Elemental
// kills, a registered quest), then buys. Nothing here relogs. Recorded in game with
// `tools/farmrun/run.sh` and `TURU_TAPE`; `bot/tests/replay.rs` replays it.
import { bot } from "turu";
import { Core } from "../../../scripts/core/CoreBots";
import { SynderesMerge } from "../../../scripts/other/mergeshops/SynderesMerge";

const ITEM = "NO BOTS Armor";

export default function main(): void {
    Core.bankingBlackList.push("Synderes' Souvenir ");
    Core.setOptions();
    bot.log(`Before: ${ITEM} ${Core.checkInventory(ITEM)}, gold ${bot.player.gold}`);
    new SynderesMerge().buyAllMerge(ITEM);
    bot.log(`After: ${ITEM} ${Core.checkInventory(ITEM)}, gold ${bot.player.gold}`);
    Core.setOptions(false);
}
