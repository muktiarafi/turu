#!/usr/bin/env python3
"""One farmrun sample (tools/farmrun/run.sh calls it every PERIOD seconds): appends a `farmrun:`
line to the log with /status's progress fields and what the process uses, then prints the
progress signature that run.sh's stuck check compares.

    python3 tools/farmrun/sample.py <status JSON> <pid> <log>

Counters (CPU seconds, network and disk bytes) are totals since the process started; the
summarizer turns them into rates. A reading the system doesn't offer is null: macOS has no
per-process disk I/O without root, and Linux's network bytes are the network namespace's
(`/proc/<pid>/net/dev`, all of the machine's unless Turu runs in a container). macOS's `nettop`
sums the sockets still open, so its bytes drop when one closes (the summarizer counts that as a
reset).
"""
import json
import os
import subprocess
import sys
import time

LINUX = sys.platform.startswith("linux")


def run(*cmd, timeout=10):
    try:
        return subprocess.run(cmd, capture_output=True, text=True, timeout=timeout).stdout
    except (OSError, subprocess.SubprocessError):
        return ""


def clock_seconds(text):
    """`ps` times: macOS [h:]m:ss.cc, Linux [d-]hh:mm:ss."""
    days, _, text = text.rpartition("-")
    secs = 0.0
    for part in text.split(":"):
        secs = secs * 60 + float(part)
    return float(days or 0) * 86400 + secs


def read(path):
    try:
        with open(path) as f:
            return f.read()
    except OSError:
        return ""


def linux(pid):
    r = {}
    stat = read(f"/proc/{pid}/stat")
    if stat:
        fields = stat.rsplit(")", 1)[1].split()
        tick = os.sysconf("SC_CLK_TCK")
        r["userSeconds"], r["sysSeconds"] = int(fields[11]) / tick, int(fields[12]) / tick
        r["cpuSeconds"] = r["userSeconds"] + r["sysSeconds"]
    for line in read(f"/proc/{pid}/status").splitlines():
        key, _, value = line.partition(":")
        if key == "VmRSS":
            r["rssMb"] = int(value.split()[0]) // 1024
        elif key == "Threads":
            r["threads"] = int(value)
    # Proportional set size: shared pages split between the processes using them.
    for line in read(f"/proc/{pid}/smaps_rollup").splitlines():
        if line.startswith("Pss:"):
            r["footprintMb"] = int(line.split()[1]) // 1024
    try:
        r["fds"] = len(os.listdir(f"/proc/{pid}/fd"))
    except OSError:
        pass
    for line in read(f"/proc/{pid}/io").splitlines():
        key, _, value = line.partition(":")
        if key == "read_bytes":
            r["diskReadBytes"] = int(value)
        elif key == "write_bytes":
            r["diskWriteBytes"] = int(value)
    rx = tx = 0
    for line in read(f"/proc/{pid}/net/dev").splitlines()[2:]:
        name, _, rest = line.partition(":")
        if name.strip() != "lo":
            cols = rest.split()
            rx, tx = rx + int(cols[0]), tx + int(cols[8])
    if rx or tx:
        r["netInBytes"], r["netOutBytes"] = rx, tx
    return r


def macos(pid):
    r = {}
    cols = run("ps", "-o", "utime=,stime=,rss=", "-p", pid).split()
    if len(cols) == 3:
        r["userSeconds"], r["sysSeconds"] = clock_seconds(cols[0]), clock_seconds(cols[1])
        r["cpuSeconds"] = r["userSeconds"] + r["sysSeconds"]
        r["rssMb"] = int(cols[2]) // 1024
    # What macOS charges the process (Activity Monitor's Memory column).
    for line in run("footprint", "-p", pid).splitlines():
        if "Footprint:" in line:
            value, unit = line.split("Footprint:")[1].split()[:2]
            r["footprintMb"] = round(float(value) * {"KB": 1 / 1024, "MB": 1, "GB": 1024}.get(unit, 1))
            break
    threads = run("ps", "-M", "-p", pid).splitlines()
    if len(threads) > 1:
        r["threads"] = len(threads) - 1
    lsof = run("lsof", "-n", "-P", "-p", pid).splitlines()
    if len(lsof) > 1:
        r["fds"] = len(lsof) - 1
    # Bytes over all of the process's sockets since it started.
    for line in run("nettop", "-P", "-L", "1", "-p", pid, "-J", "bytes_in,bytes_out", "-x").splitlines():
        cols = line.strip().strip(",").split(",")
        if len(cols) == 3 and cols[1].isdigit():
            r["netInBytes"], r["netOutBytes"] = int(cols[1]), int(cols[2])
    return r


def main():
    status, pid, log = sys.argv[1:4]
    try:
        d = json.loads(status)
    except ValueError:
        d = {}
    s = d.get("stats") or {}
    usage = (linux if LINUX else macos)(pid)
    row = {
        "t": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "ok": bool(d),
        "loggedIn": d.get("loggedIn"),
        "running": (d.get("script") or {}).get("running"),
        "level": d.get("level"),
        "map": d.get("map"),
        "cell": d.get("cell"),
        "hp": d.get("health"),
        "maxHp": d.get("maxHealth"),
        **{k: s.get(k) for k in ("kills", "deaths", "questsAccepted", "questsCompleted", "relogins", "drops")},
        **{k: usage.get(k) for k in ("cpuSeconds", "userSeconds", "sysSeconds", "rssMb", "footprintMb", "threads",
                                     "fds", "netInBytes", "netOutBytes", "diskReadBytes", "diskWriteBytes")},
        "logBytes": os.path.getsize(log),
    }
    with open(log, "a") as f:
        f.write("farmrun: " + json.dumps(row) + "\n")
    # Deaths and relogins left out: dying over and over in one place is still stuck.
    print("|".join(str(row[k]) for k in ("loggedIn", "running", "level", "map", "cell", "kills",
                                          "questsAccepted", "questsCompleted")))


if __name__ == "__main__":
    main()
