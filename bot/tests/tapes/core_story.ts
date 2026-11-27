// The script of `core_story.tape.jsonl.gz` (docs/REPLAY_TESTS_PROPOSAL.md, phase 3): CoreStory's
// `mapItemQuest` then `killQuest`, the first two Dark Dungeon quests (from
// scripts/story/DarkDungeon.ts), with `preLoad`. The kill brings combat: the skill timer and the
// drop pickups send from threads of their own (decision 7).
//
// Each quest runs only when it is unlocked and not done. CoreStory's `questProgression` relogs to
// recover a quest that isn't unlocked, and CoreBots' `relogin` falls back to Twilly (as in C#):
// a tape must never relog on a crowded server. Recorded in game with `tools/farmrun/run.sh` and
// `TURU_TAPE`; `bot/tests/replay.rs` replays it.
import { bot } from "turu";
import { Core } from "../../../scripts/core/CoreBots";
import { CoreStory } from "../../../scripts/core/CoreStory";

const Story = new CoreStory();

/** Whether `id` can be taken now; logs why not. */
function takeable(id: number): boolean {
    const unlocked = bot.quests.isUnlocked(id);
    const done = bot.quests.hasBeenCompleted(id);
    bot.log(`Quest ${id}: unlocked ${unlocked}, done ${done}`);
    return unlocked && !done;
}

function storyLine(): void {
    Story.preLoad(storyLine);
    // Quiz the Locals 3538
    if (!takeable(3538)) return;
    Story.mapItemQuest(3538, "darkdungeon", [2671, 2672, 2673]);
    // Slay the Minions 3539
    if (!takeable(3539)) return;
    Story.killQuest(3539, "darkdungeon", "Dungeon Minion");
}

export default function main(): void {
    Core.setOptions();
    bot.quests.load(3538, 3539);
    storyLine();
    bot.log(`After: 3538 ${bot.quests.hasBeenCompleted(3538)}, 3539 ${bot.quests.hasBeenCompleted(3539)}`);
    Core.setOptions(false);
}
