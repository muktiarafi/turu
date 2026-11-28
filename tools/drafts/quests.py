#!/usr/bin/env python3
"""Cuts a quest cache down to what the draft run's world needs: bot/tests/world/quests.json.gz.

The draft run (bot/tests/draft_smoke.rs) gives the quests a draft loads their real name,
requirements and rewards from it (docs/REPLAY_TESTS_PROPOSAL.md, "Real quests"). The input is a
`QuestData.json` in VibeSkua's format: Tools > Loader's in the Turu data folder (its community
download), or the one VibeSkua bundles (`Skua.App.Avalonia/QuestData.json`). The output keeps
VibeSkua's field names, so `turu_bot::quest_cache::QuestData` reads it, and only the fields the
fake uses; it is written the same way for the same input (sorted, no time stamp).

    python3 tools/drafts/quests.py "$HOME/Library/Application Support/Turu/QuestData.json"
"""

import gzip
import json
import sys
from pathlib import Path

OUT = Path(__file__).resolve().parents[2] / "bot/tests/world/quests.json.gz"
ITEM_FIELDS = ("ItemID", "sName", "iQty", "iStk", "bTemp")


def slim_item(item):
    return {key: item.get(key) for key in ITEM_FIELDS}


def main():
    if len(sys.argv) != 2:
        sys.exit(f"usage: {sys.argv[0]} <QuestData.json>")
    with open(sys.argv[1], encoding="utf-8") as f:
        quests = json.load(f)
    slim = [
        {
            "ID": q["ID"],
            "Name": q.get("Name") or "",
            "Requirements": [slim_item(i) for i in q.get("Requirements") or []],
            "Rewards": [slim_item(i) for i in q.get("Rewards") or []],
        }
        for q in sorted(quests, key=lambda q: q["ID"])
    ]
    text = json.dumps(slim, ensure_ascii=False, separators=(",", ":"), sort_keys=True)
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_bytes(gzip.compress(text.encode("utf-8"), compresslevel=9, mtime=0))
    print(f"{len(slim)} quests -> {OUT} ({OUT.stat().st_size // 1024} KB)")


if __name__ == "__main__":
    main()
