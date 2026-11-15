# Proposal: CPU and memory in the web monitor

Status: proposal, nothing built. Touches `bot/src/monitor/` (`hub.rs`, `server.rs`, `push.rs`,
`mod.rs`, a new `resources.rs`), `control_api.rs` (`/snapshot`), `bot/Cargo.toml`,
`bot/assets/monitor/index.html`, and `frontend/src/process_memory.rs` with its user
`android/src/memory.rs` (the `/proc` parsing moves into the engine). Reverses decision 3 of `docs/WEB_MONITOR_PROPOSAL.md` ("Turu's
own counters only. No CPU / RAM and no `sysinfo`").

## Why now

Decision 3 was made when the monitor's job was to replace the Skua Nexus page. Nexus read
VibeSkua's CPU and RAM on Windows only, and copying that meant adding `sysinfo`. Since then:

- **Headless servers are a main way to run Turu.** `docs/HEADLESS.md` describes one process per
  account on a VPS, all reporting to one monitor. The person running it most needs to know
  "can this machine take another account, and is it about to run out of memory?" The page
  can't tell them.
- **Memory has caused real trouble.** STATUS.md records mimalloc holding about 170 MB beyond the
  live heap and SWFs that were loaded again and never freed. `frontend/src/process_memory.rs`'s
  header records a Pixel 8 aborting at 1.9 GB and ColorOS closing Turu "for using too much
  CPU". Today you only see this in `memory:` log lines (`TURU_PERF_LOG`) or by running
  `tools/farmrun`, and only if you were already looking.
- **The OS calls are cheap and partly written already.** `frontend/src/process_memory.rs` reads
  `/proc/self` (Linux, Android) and works out CPU %. `app/src/headless.rs` reads
  `mi_process_info`, but only the app links mimalloc, not the engine crate where the monitor
  lives, so the engine needs its own calls (below).

## What it shows

Two levels, matching what can actually be measured:

1. **Each Turu process:** its CPU (% of one core, as `top` shows it), resident memory and its peak,
   and on macOS the footprint (what Activity Monitor shows). That means the process serving the
   page, each Army client, and each machine reporting to it. Tabs share their window's process,
   so a tab has no figure of its own.
2. **Each machine:** cores, overall CPU %, total and available memory, shown once however many
   Turu processes run on it (see "One machine, several Turu processes"). A process in a Linux
   container (the Docker image) also shows its container's memory limit and use from the cgroup,
   because `/proc/meminfo` shows the host's memory, not the container's.

On the page:

- Each machine's heading (`This PC`, `vps-1`, ...) gets one line:
  `CPU 18% of 4 cores · Memory 2.1 of 7.8 GB free · Turu 9% · 410 MB`, with a sparkline of the
  last hour's memory on hover or in the machine's details. With only one machine there's no
  heading today; the same line goes in the summary bar instead.
- An Army client's row shows its process's memory and CPU in the details panel, not as a new
  column: the roster is already full, and most of the time these numbers say nothing.
- **"Needs you"** gets machine-level entries (see "Warnings"). These are worth more than the
  numbers, because nobody watches numbers.

Left out on purpose: disk, network, GPU, temperatures, swap, per-thread CPU. `tools/farmrun` and
the perf logs already cover those for investigations. The monitor should answer "is this
machine healthy, and does it have room?", not be a profiler.

## Readings: `bot/src/monitor/resources.rs`

One module in the engine crate (no Ruffle, no window, so headless, Army clients and Android
all get it):

```rust
pub struct ProcessReading { pub cpu_percent: Option<f32>, pub rss: u64, pub peak_rss: u64, pub footprint: Option<u64> }
pub struct MachineReading {
    pub cores: u32,
    pub cpu_percent: Option<f32>,
    pub memory_total: u64,
    pub memory_available: u64,
    /// A cgroup memory limit (Linux containers), and what the group uses.
    pub container: Option<(u64, u64)>,
}
pub struct Probe { /* the previous CPU times, for the next percentage */ }
impl Probe { pub fn process(&mut self) -> Option<ProcessReading>; pub fn machine(&mut self) -> Option<MachineReading>; }
```

CPU % needs two readings, so `Probe` keeps the last CPU times. The first reading has
`cpu_percent: None`.

| | Process | Machine |
|---|---|---|
| Linux, Android | `/proc/self/status` (`VmRSS`, `VmHWM`), `/proc/self/stat` (CPU time): the parsing in `process_memory.rs`, moved here and re-exported from there | `/proc/meminfo` (`MemTotal`, `MemAvailable`), `/proc/stat`'s `cpu` line; cgroup v2 `memory.max` / `memory.current` (v1: `memory.limit_in_bytes` / `usage_in_bytes`), only when the limit isn't "max" |
| macOS | `task_info(MACH_TASK_BASIC_INFO)` (resident), `TASK_VM_INFO`'s `phys_footprint`, `getrusage` (CPU time, peak) | `sysctl hw.memsize`; `host_statistics64(HOST_VM_INFO64)` free + inactive + purgeable pages as available; `host_statistics(HOST_CPU_LOAD_INFO)` ticks |
| Windows | `GetProcessMemoryInfo` (working set and its peak), `GetProcessTimes` | `GlobalMemoryStatusEx`, `GetSystemTimes` |

Cores come from `std::thread::available_parallelism`, which counts a container's CPU quota on
Linux. Android 8 and later deny `/proc/stat`, so a phone's machine CPU is `None`; its memory and
the process readings still work.

**Why not `sysinfo`:** it would cover all of this, cgroups included. But it's a large crate built
around refreshing whole tables of processes, disks and networks, and the parts we need are about
250 lines of `cfg` code. They need `libc` on unix and `windows-sys` on Windows as direct
dependencies of `turu_bot`. `libc` 0.2.189 is already in the engine's tree on unix (through
tokio, hyper-util, getrandom), and its `task_info`, `host_statistics64` and `mach_task_self` are
not marked deprecated (some other mach items are, in favour of `mach2`, which is also in
`Cargo.lock`). Whether a `windows-sys` version is already in the engine's Windows tree, and which
Win32 features it needs turned on, is to be confirmed in phase 1; the expectation is no new crate
in the lock file. If the macOS or cgroup code turns out more fiddly than expected, `sysinfo` with
`default-features = false, features = ["system"]` is the fallback; the module's interface stays
the same.

`app/src/headless.rs`'s `memory:` line keeps reading mimalloc: commit and the live Rust heap
are for telling a leak from the allocator, which the monitor doesn't need.

## One machine, several Turu processes

`docs/HEADLESS.md`'s server layout runs one Turu process per account on the same VPS: one
serves `--monitor`, the others `--report-to http://127.0.0.1:9021`. Each reporter looks like
a separate machine to the hub, and with Docker Compose each container does too. Keyed by the
reporting name, the page would show the same host's memory line under "This PC" and again under
every reporter, and raise "almost out of memory" once per process.

So machine readings are keyed by a **host id**, not by the name on the page:

- Linux and Android: `/proc/sys/kernel/random/boot_id`. It's the kernel's, so every container
  on one host reads the same value, while a VM reads its own.
- macOS: `sysctl kern.bootsessionuuid`.
- Windows: the computer name plus the boot time (`GetTickCount64` subtracted from now, rounded
  to the minute).

Every `Report` carries its host id, and so does the hub's own reading. The hub keeps one machine
record per host id, made of the latest machine reading from any process on that host plus a list
of the Turu processes there: the host process, each reporter, and each Army client (Army clients
run on the host that launched them). The page draws one machine line per host, under the first
group heading on that host, and adds up Turu's processes: `Turu: 4 processes, 1.6 GB, 31% of a
core`. Each warning is raised once per host.

A container's cgroup limit is per container, not per host, so it stays on that process's entry
("container 1.4 of 2 GB") rather than the machine line, and gets its own warning.

## Where the readings go

**The host process.** The hub's thread (`Hub::start`, every second) takes a reading every 2 s
while a page is open (`monitor::watched()`), the same rule the tab samplers follow. It also keeps
one reading a minute **always**, in a ring of 60, so the page has an hour of history to judge
growth the moment it opens. That costs a few small file reads or system calls a minute. The hub
starts lazily (`frontend/src/turu.rs`, `monitor_hub()`) when the monitor serves, a reporter runs,
or an Army window opens, which are exactly the processes whose readings anyone sees.
`/api/state` gains:

```json
"machines": [
  { "host": "3f2a…", "at": 1763200000000, "cores": 4, "cpuPercent": 18.2,
    "memoryTotal": 8368000000, "memoryAvailable": 2254000000,
    "turu": [
      { "id": null, "cpuPercent": 9.1, "rss": 430000000, "peakRss": 512000000, "footprint": null, "container": null },
      { "id": "remote:vps:", "cpuPercent": 7.0, "rss": 390000000, "peakRss": 450000000, "footprint": null, "container": null }
    ] }
]
```

`id` names where each process's bots show on the page: `null` for the host process, a remote
machine's prefix, or an Army client's entry id.

`/api/events` sends the state again only when it changes, which it checks with
`state_with_clock(false)`. A CPU % or a byte count that moves with every reading would make it
send every second, so that pass sets `at` to 0, rounds CPU to whole percent and memory to 50 MB,
as it already zeroes `now`, `uptimeSeconds` and `lastSeen`.

The history stays out of `/api/state`, so `/api/events` doesn't resend an hour of points with
every change. `GET /api/machines/<name>/history` (`this` for this machine) answers
`[[at, turu rss, machine available], ...]`, a minute apart. The page asks for it when it
loads, and again every few minutes, for the sparkline and the growth warning.

**Army clients.** Each client's `/snapshot` reply gains `"process": ProcessReading`, read by
the client from its own `Probe`, so its CPU % covers the time since the Army poll last asked
(2 s). The hub keeps it on the entry, with its own minute ring (an Army client starts no hub
of its own, so its history lives in the host's), and `View` gains `process` (null for tabs and
offline entries). The host could read its children's PIDs itself instead, but on macOS that needs
`proc_pidinfo` and a permissions story; the client reporting on itself works the same
everywhere, including for a remote machine's Army clients.

**Machines reporting here.** `Report` gains `host` (the host id) and
`resources: Option<MachineResources>` (the machine reading, its Turu process, and its last 60
minute-readings), and `Pushed` gains `process`. `record_report` files the machine reading under
the host id and the process under the reporter. Both fields are
`#[serde(default)]`, so an older Turu reporting here, or reporting to an older one, works
unchanged. A reporter takes a reading with every report but at most every 5 s.

## Warnings

`trouble()` in the page works per bot. Machine trouble gets its own entries at the top of
"Needs you", one per machine, with the numbers that triggered it:

| When | Says |
|---|---|
| Available memory under 10% of total or under 300 MB, for 2 readings in a row | "vps-1 is almost out of memory (180 MB free of 4 GB). Stop a bot or add memory." |
| A container using over 90% of its limit | "vps-1's container is at 1.8 of its 2 GB limit." |
| A Turu process's memory growing over 100 MB an hour, across the last 60 minutes, once it has run for 10 | "Turu on vps-1 grew from 420 to 610 MB in the last hour." |
| Machine CPU over 90% for 5 minutes | "vps-1's CPU has been over 90% for 5 minutes." |

The 10-minute wait follows `tools/farmrun/summarize.py`, which measures memory growth only after
the first 10 minutes (`docs/HEADLESS.md`): memory climbs during the first joins and then levels
off, and a warning about warm-up would be noise. The thresholds are constants at the
top of the page's script, as a starting point to tune after a week of real servers. There are no
actions: the fix is the person's decision.

## Later, if wanted: each tab's share of the game thread

Tabs share a process, but every tab's game runs on the one event loop, and `perf.rs` already
measures each tab's player and Flash-call time. A `gameMs` field in `Snapshot` (milliseconds of
the game thread per second, from the difference between two `PerfStats::snapshot`s) would tell
you which tab is heavy, which CPU % per process can't. It's a separate change from the rest and
not needed for the questions above.

## Phases

1. **This machine.** `resources.rs` with its parsers tested on sample text (as
   `process_memory.rs`'s are), `machines` in `/api/state`, the heading line, the history route.
   Checked by hand on macOS, Linux, Windows (CI runs Linux only) and in the Docker image with
   `--memory=1g`.
2. **Army clients and reporting machines.** `/snapshot`'s `process`, `Report.host` and
   `Report.resources`, `Pushed.process`, one machine line per host id. Checked with three
   headless processes on one machine (one `--monitor`, two `--report-to`, as HEADLESS.md lays
   out a server), with two Compose containers, and with an older build reporting (missing
   fields).
3. **Warnings.** The "Needs you" entries. The page has no test harness, so the growth and
   low-memory rules are checked by hand against a stub server answering made-up `/api/state` and
   history (as the Script Options tab was checked against a mock server).

## Docs to update when it lands

- `docs/WEB_MONITOR_PROPOSAL.md`: decision 3 points here.
- `docs/HEADLESS.md`: the web monitor paragraph and the Docker section (the container limit).
- `docs/STATUS.md`: the web monitor entry, with what was checked by hand and what wasn't.
- `bot/src/monitor/server.rs`: the route table at the top.

## Open questions

- **Reporters on one host collided (found 2026-11-15, fixed the same day).** Clients on one
  server share a data folder, so a settings file and a machine name, and their bots all had the
  id `remote:<name>:tab:0`: the page showed one row for two clients, and a command for it ran
  on whichever reported first. Each report now carries a random id for its process
  (`push::instance`), so entries are `remote:<machine>:<instance>:<bot>` and command queues are
  per process, still under one machine heading. The host id above is still needed: the
  instance tells processes apart, not which of them share a computer.
- **Units for CPU.** `top` shows a process as % of one core (a busy 4-core machine can show 300%);
  Activity Monitor too; Windows Task Manager shows % of the whole machine. The proposal uses %
  of one core for processes and % of the machine for the machine line, labelled so the two
  aren't confused ("Turu 9% of a core"). Is that clear enough, or should both be % of the
  machine?
- **Android.** The hub and reporter run there through `frontend/src/turu.rs`. A phone reporting
  to a desktop would show its memory and Turu's process but no machine CPU. That seems fine,
  but the phone's 10%-free warning would fire often (Android keeps memory full on purpose), so
  phones may need the warning off or a lower threshold.
