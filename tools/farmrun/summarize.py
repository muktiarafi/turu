#!/usr/bin/env python3
"""Summarizes a farmrun log (tools/farmrun/run.sh): how far the character got and how fast, where
the time went, when it stood still, what the server and the bot complained about, and what the
run cost. Works on any Turu log; without `farmrun:` lines (another runner) it reports what the
script and perf lines say. Writes <log>.csv beside the log (one row per `farmrun:` sample).

    python3 tools/farmrun/summarize.py tools/farmrun/out/<run>.log [--stuck MIN] [--section MIN]
"""
import argparse
import csv
import json
import re
from collections import Counter, defaultdict
from datetime import datetime, timezone
from pathlib import Path

ANSI = re.compile(r"\x1b\[[0-9;]*m")
STAMP = re.compile(r"^(\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d(?:\.\d+)?)Z")
# `turu::script: [user] [16:48:02] (beginnerItems) text` (CoreBots' logger), or without the tag.
SCRIPT = re.compile(r"turu::script: (?:\[[^\]]*\] )?(?:\[\d\d:\d\d:\d\d\] )?(?:\((?P<tag>[^)]+)\) )?(?P<text>.*)")
DEBUG = re.compile(r"turu::debug: (?:\[[^\]]*\] )?(?P<text>.*)")
PERF = re.compile(r"player thread (\d+) ms/s \(ticks (\d+), calls (\d+)\); ([\d.]+) calls/s")
MEMORY = re.compile(r"memory: process resident (\d+) MB .*rust heap (\d+) MB")
CALL_ROW = re.compile(r"^\s+([\d.]+)/s\s+wait\s+([\d.]+) ms\s+run\s+([\d.]+) ms\s+(\d+) B\s+(.{18}) (.*)$")
# The signature of "moving": any change here, or a script line, is progress.
MOVING = ("loggedIn", "running", "level", "map", "cell", "kills", "questsAccepted", "questsCompleted")


def when(line):
    m = STAMP.match(line)
    return datetime.fromisoformat(m[1]).replace(tzinfo=timezone.utc) if m else None


def clock(t):
    return t.astimezone().strftime("%H:%M") if t else "?"


def span(a, b):
    m = (b - a).total_seconds() / 60
    return f"{m / 60:.1f} h" if m >= 90 else f"{m:.0f} min"


def parse(path):
    r = defaultdict(list)
    last_t = None
    perf_block = None
    for raw in path.read_text(errors="replace").splitlines():
        line = ANSI.sub("", raw)
        if line.startswith("farmrun: {"):
            row = json.loads(line[len("farmrun: "):])
            row["t"] = datetime.fromisoformat(row["t"].replace("Z", "+00:00"))
            r["samples"].append(row)
            continue
        if line.startswith("farmrun: ") and "STUCK?" in line:
            r["stuck_alerts"].append(line[len("farmrun: "):])
            continue
        if perf_block is not None and (m := CALL_ROW.match(line)):
            perf_block.append(m.groups())
            continue
        t = when(line)
        if t:
            last_t = t
            r["first"] = r["first"] or [t]
        if (m := PERF.search(line)) and t:
            r["perf"].append((t, *map(float, m.groups())))
            perf_block = []
            r["perf_blocks"].append(perf_block)
            continue
        perf_block = None
        if (m := MEMORY.search(line)) and t:
            r["memory"].append((t, int(m[1]), int(m[2])))
        elif (m := SCRIPT.search(line)) and t:
            r["script"].append((t, m["tag"], m["text"]))
        elif (m := DEBUG.search(line)) and t:
            r["debug"].append((t, m["text"]))
        elif " ERROR " in line and "ModalStack" not in line or "panicked" in line:
            r["errors"].append((t, line[:220]))
    r["last"] = last_t
    return r


def moving_signature(s):
    return tuple(s.get(k) for k in MOVING)


def report(r, stuck_min, section_min):
    samples, script, debug = r["samples"], r["script"], r["debug"]
    start = (r["first"] or [None])[0]
    end = r["last"]
    if samples:
        start, end = min(start or samples[0]["t"], samples[0]["t"]), max(end or samples[-1]["t"], samples[-1]["t"])
    if not start:
        print("No timestamped lines: is this a Turu log?")
        return
    hours = max((end - start).total_seconds() / 3600, 1e-9)

    # ---- How it went ----
    started = next((s for s in script if s[2].startswith("Started script")), None)
    ended = next((s for s in reversed(script) if re.match(r"Script .* (finished|stopped|failed)", s[2])), None)
    print(f"Run: {clock(start)} -> {clock(end)} ({span(start, end)})")
    if started:
        print(f"  {started[2]} at {clock(started[0])}")
    print(f"  Ended: {ended[2].splitlines()[0]} at {clock(ended[0])}" if ended else "  Ended: the script was still running")

    # ---- Progress ----
    print("\nProgress")
    if samples:
        a, b = samples[0], samples[-1]
        n = lambda row, k: row.get(k) or 0
        h = max((b["t"] - a["t"]).total_seconds() / 3600, 1e-9)
        print(f"  Level {a.get('level')} -> {b.get('level')}")
        for k, label in (("kills", "kills"), ("questsCompleted", "quests turned in"), ("deaths", "deaths"),
                         ("drops", "drops picked up"), ("relogins", "relogins")):
            d = n(b, k) - n(a, k)
            print(f"  {label:>16}: {d:6d}  ({d / h:7.1f}/h)")
        ups, prev = [], a.get("level")
        for s in samples:
            if s.get("level") and prev and s["level"] > prev:
                ups.append((s["t"], s["level"]))
            prev = s.get("level") or prev
        if ups:
            print("  Levels reached: " + ", ".join(f"{lv} at {clock(t)}" for t, lv in ups))
        maps = Counter()
        for x, y in zip(samples, samples[1:]):
            maps[x.get("map") or "(none)"] += (y["t"] - x["t"]).total_seconds() / 60
        if maps:
            print("  Time per map: " + ", ".join(f"{m} {v:.0f} min" for m, v in maps.most_common(10)))
    else:
        print("  (no farmrun samples: run it with tools/farmrun/run.sh for kills, levels and maps)")

    # ---- Where the script's time went (CoreBots' logger tags it with the calling function) ----
    sections, cur = [], None
    for t, tag, text in script:
        if not tag:
            continue
        if cur and cur[0] == tag:
            cur[2] = t
        else:
            cur = [tag, t, t, text]
            sections.append(cur)
    # A section lasts until the next one starts.
    timeline = []
    for i, (tag, t0, _, text) in enumerate(sections):
        t1 = sections[i + 1][1] if i + 1 < len(sections) else end
        if (t1 - t0).total_seconds() / 60 >= section_min:
            timeline.append((t0, t1, tag, text))
    if timeline:
        print(f"\nScript sections of {section_min:g} min or more (by the logging function)")
        for t0, t1, tag, text in timeline[-25:]:
            print(f"  {clock(t0)} {span(t0, t1):>8}  ({tag}) {text[:90]}")

    # ---- Standing still ----
    stuck = []
    if samples:
        script_times = [t for t, _, _ in script]
        run_start = run_sig = None
        for s in samples:
            sig = moving_signature(s)
            spoke = run_start and any(run_start < t <= s["t"] for t in script_times)
            if sig != run_sig or spoke:
                if run_start and (prev_t - run_start).total_seconds() / 60 >= stuck_min:
                    stuck.append((run_start, prev_t, run_sig))
                run_start, run_sig = s["t"], sig
            prev_t = s["t"]
        if run_start and (prev_t - run_start).total_seconds() / 60 >= stuck_min:
            stuck.append((run_start, prev_t, run_sig))
    stalls = [(t, x) for t, x in debug if x.startswith("Skill timer: no target")]
    afk = [(t, x) for t, x in debug if "afk" in x.lower() and "no longer" not in x.lower()]
    print(f"\nStanding still (nothing moved for {stuck_min:g} min or more)")
    if not samples:
        print("  (needs farmrun samples)")
    for a, b, sig in stuck:
        print(f"  {clock(a)} -> {clock(b)} ({span(a, b)}): level {sig[2]}, /{sig[3]} {sig[4]}, "
              f"logged in {sig[0]}, script running {sig[1]}")
    for t, x in stalls[-10:]:
        print(f"  {clock(t)} {x[:160]}")
    for t, x in afk[-10:]:
        print(f"  {clock(t)} {x[:160]}")
    if samples and not (stuck or stalls or afk):
        print("  none")

    # ---- Complaints ----
    print("\nWarnings")
    server = Counter(x for _, x in debug if x.startswith("Server warning:"))
    refused = [x for _, x in debug if "refused the turn-in" in x]
    unanswered = [x for _, x in debug if "no reply to the turn-in" in x]
    reaccepted = [x for _, x in debug if "abandoning it and accepting it again" in x]
    # The first login also goes through Relogin: only those after the script started count.
    after = started[0] if started else start
    relogs = [x for t, _, x in script if t > after and x.startswith("[Relogin] Auto relogin started")]
    rows = [(f"{c}x {w}") for w, c in server.most_common(6)]
    rows += [f"{len(refused)} turn-ins refused"] if refused else []
    rows += [f"{len(unanswered)} turn-ins unanswered"] if unanswered else []
    rows += [f"{len(reaccepted)} quests re-accepted after lost turn-ins"] if reaccepted else []
    rows += [f"{len(relogs)} auto-relogins (last: {relogs[-1][:100]})"] if relogs else []
    rows += [f"{len(r['errors'])} ERROR/panic lines (last: {r['errors'][-1][1][:150]})"] if r["errors"] else []
    rows += [f"{len(r['stuck_alerts'])} live STUCK? alerts"] if r["stuck_alerts"] else []
    for row in rows or ["none"]:
        print(f"  {row}")
    if ended and "failed" in ended[2]:
        print("  The script failed:\n    " + "\n    ".join(ended[2].splitlines()[:8]))

    # ---- Cost ----
    print("\nCost")
    cpu = [(y["cpuSeconds"] - x["cpuSeconds"]) / max((y["t"] - x["t"]).total_seconds(), 1) * 100
           for x, y in zip(samples, samples[1:]) if x.get("cpuSeconds") is not None and y.get("cpuSeconds") is not None]
    if cpu:
        print(f"  CPU: {sum(cpu) / len(cpu):.0f}% of a core on average, {max(cpu):.0f}% at most")
    rss = [s["rssMb"] for s in samples if s.get("rssMb")]
    if rss:
        print(f"  Resident memory: {rss[0]} -> {rss[-1]} MB, {max(rss)} MB at most")
    elif r["memory"]:
        m = r["memory"]
        print(f"  Resident memory: {m[0][1]} -> {m[-1][1]} MB, {max(x[1] for x in m)} MB at most")
    if r["perf"]:
        p = r["perf"]
        avg = lambda i: sum(x[i] for x in p) / len(p)
        print(f"  Game thread: {avg(1):.0f} ms/s on average (ticks {avg(2):.0f}, bot calls {avg(3):.0f}), "
              f"{max(x[1] for x in p):.0f} at most; {avg(4):.0f} bot calls/s")
        calls = defaultdict(lambda: [0.0, 0.0])
        for block in r["perf_blocks"]:
            for per_s, _wait, run, _b, thread, call in block:
                c = calls[(thread.strip(), call)]
                c[0] += float(per_s)
                c[1] += float(per_s) * float(run)
        if calls:
            blocks = len(r["perf_blocks"])
            print("  Busiest bot calls among those logged (game thread ms/s, calls/s):")
            for (thread, call), (n, ms) in sorted(calls.items(), key=lambda kv: -kv[1][1])[:6]:
                print(f"    {ms / blocks:5.2f} ms/s {n / blocks:6.1f}/s  {thread:<16} {call}")
    if not (cpu or r["perf"]):
        print("  (no samples or turu_perf lines: set TURU_PERF_LOG)")


def write_csv(path, samples):
    if not samples:
        return None
    out = path.with_suffix(".csv")
    keys = ["t"] + [k for k in samples[0] if k != "t"]
    with open(out, "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=keys, extrasaction="ignore")
        w.writeheader()
        for s in samples:
            w.writerow({**s, "t": s["t"].isoformat()})
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("log", type=Path)
    ap.add_argument("--stuck", type=float, default=10, help="minutes without progress that count as stuck")
    ap.add_argument("--section", type=float, default=2, help="shortest script section to list, in minutes")
    a = ap.parse_args()
    r = parse(a.log)
    print(f"{a.log.name}\n")
    report(r, a.stuck, a.section)
    if out := write_csv(a.log, r["samples"]):
        print(f"\nCSV: {out}")


if __name__ == "__main__":
    main()
