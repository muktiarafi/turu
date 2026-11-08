#!/usr/bin/env python3
"""Summarizes a memrun log (tools/memrun/run.sh): Turu's `memory:` lines over time, their
growth per hour after the warm-up, and which of docs/HEADLESS.md's causes the trend points at.
Writes <log>.csv beside the log.

    python3 tools/memrun/summarize.py tools/memrun/out/<run>.log [--warmup MIN]
"""
import argparse
import csv
import re
from datetime import datetime, timezone
from pathlib import Path

ANSI = re.compile(r"\x1b\[[0-9;]*m")
STAMP = re.compile(r"^(\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d(?:\.\d+)?)Z")
MEMORY = re.compile(
    r"memory: process resident (?P<rss>\d+) MB \(peak (?P<peak_rss>\d+)\), "
    r"commit (?P<commit>\d+) MB \(peak (?P<peak_commit>\d+)\), rust heap (?P<heap>\d+) MB; "
    r"(?P<swfs>\d+) loaded SWFs \((?P<freed>\d+) freed so far; (?P<urls>\d+) distinct URLs; "
    r"SWF data (?P<swf_mb>\d+) MB, (?P<swf_distinct_mb>\d+) MB distinct; "
    r"shape tags parsed (?P<shapes>\d+), shared (?P<shared>\d+)\), (?P<gc>\d+) GC objects"
)
FOOTPRINT = re.compile(r"^memrun: (\d\d:\d\d:\d\d) footprint (\d+) (KB|MB|GB)")
UNIT = {"KB": 1 / 1024, "MB": 1, "GB": 1024}
census = []  # (time, the `memory census:` line's text)
FIELDS = ["rss", "commit", "heap", "swfs", "freed", "urls", "swf_mb", "shapes", "gc"]


def parse(path):
    rows, footprints, notable = [], [], []
    for raw in path.read_text(errors="replace").splitlines():
        line = ANSI.sub("", raw)
        if "memory census: " in line:
            s = STAMP.match(line)
            census.append((s[1][11:19] if s else "", line.split("memory census: ", 1)[1]))
            continue
        if m := FOOTPRINT.match(line):
            footprints.append(float(m[2]) * UNIT[m[3]])
            continue
        if any(k in line for k in ("ERROR", "panicked", "Logged in", "Script stopped", "relogin")):
            notable.append(line[:200])
        m = MEMORY.search(line)
        if not m:
            continue
        s = STAMP.match(line)
        t = datetime.fromisoformat(s[1]).replace(tzinfo=timezone.utc) if s else None
        row = {k: int(v) for k, v in m.groupdict().items()}
        row["time"] = t
        rows.append(row)
    if rows and rows[0]["time"]:
        start = rows[0]["time"]
        for r in rows:
            r["min"] = (r["time"] - start).total_seconds() / 60
    else:
        for i, r in enumerate(rows):
            r["min"] = float(i)
    for r, fp in zip(rows, footprints):
        r["footprint"] = round(fp)
    return rows, notable


def slope_per_hour(rows, key):
    pts = [(r["min"], r[key]) for r in rows if key in r]
    if len(pts) < 3:
        return None
    n = len(pts)
    mx = sum(x for x, _ in pts) / n
    my = sum(y for _, y in pts) / n
    var = sum((x - mx) ** 2 for x, _ in pts)
    if var == 0:
        return None
    return sum((x - mx) * (y - my) for x, y in pts) / var * 60


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("log", type=Path)
    ap.add_argument("--warmup", type=float, default=10, help="minutes left out of the trend")
    a = ap.parse_args()
    rows, notable = parse(a.log)
    if not rows:
        print("No `memory:` lines: was TURU_PERF_LOG set, and did the game load?")
        for line in notable[-10:]:
            print("  ", line)
        return

    cols = ["min", "footprint"] + FIELDS
    with open(a.log.with_suffix(".csv"), "w", newline="") as f:
        w = csv.writer(f)
        w.writerow(cols)
        for r in rows:
            w.writerow([round(r["min"], 1) if c == "min" else r.get(c, "") for c in cols])

    step = max(1, len(rows) // 20)
    shown = rows[::step] + ([rows[-1]] if (len(rows) - 1) % step else [])
    print(f"{len(rows)} readings over {rows[-1]['min']:.0f} min  ({a.log.name})\n")
    print("  ".join(f"{c:>9}" for c in cols))
    for r in shown:
        print("  ".join(f"{(round(r['min']) if c == 'min' else r.get(c, '')):>9}" for c in cols))

    steady = [r for r in rows if r["min"] >= a.warmup] or rows
    growth = {k: slope_per_hour(steady, k) for k in ["footprint"] + FIELDS}
    print(f"\nGrowth per hour after the first {a.warmup:g} min ({len(steady)} readings):")
    units = {"swfs": "SWFs", "freed": "SWFs", "urls": "URLs", "shapes": "shapes", "gc": "objects"}
    for k, v in growth.items():
        if v is not None:
            print(f"  {k:>9}: {v:+,.1f} {units.get(k, 'MB')}/h")

    first, last = steady[0], steady[-1]
    print("\nReading it (docs/HEADLESS.md):")
    g = lambda k: growth.get(k) or 0.0
    if g("rss") < 20 and g("heap") < 20:
        print("  - Flat (< 20 MB/h): no growth worth chasing in this run.")
    if g("swfs") > 0 and g("swf_mb") > 5:
        print(
            f"  - SWFs pile up: {first['swfs']} -> {last['swfs']} loaded, {last['freed']} freed, "
            f"SWF data {first['swf_mb']} -> {last['swf_mb']} MB. AQW keeps maps; candidate fix: "
            "unload them (unloadAndStop + System.gc) after checking Ruffle frees them."
        )
    if g("rss") > 20 and g("heap") < g("rss") / 3:
        print("  - Resident grows but the live Rust heap doesn't: the allocator keeps freed pages "
              "(mimalloc purge settings, mi_collect on trim).")
    if g("heap") > 20 and g("swf_mb") < 5:
        print("  - Live heap grows without new SWF data: a leak; profile with TURU_SYSTEM_ALLOC=1 "
              "and malloc_history -allBySize at two times and diff.")
    if g("gc") > 10_000:
        print(f"  - GC objects grow ~{g('gc'):,.0f}/h: the game holds onto Flash objects.")
    if census:
        print("\nMost-loaded URLs among live movies (first, middle, last reading):")
        for when, text in dict.fromkeys([census[0], census[len(census) // 2], census[-1]]):
            print(f"  {when}: {text}")
    if notable:
        print("\nNotable lines (last 8):")
        for line in notable[-8:]:
            print("  ", line)
    print(f"\nCSV: {a.log.with_suffix('.csv')}")


if __name__ == "__main__":
    main()
