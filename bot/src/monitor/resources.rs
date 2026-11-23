//! This process's and this machine's CPU and memory, for the web monitor
//! (docs/MONITOR_RESOURCES_PROPOSAL.md): is the machine healthy, and does it have room for
//! another account?
//!
//! * A [`Probe`] reads the process ([`ProcessReading`]: its share of a core, resident memory and
//!   its peak, the footprint on macOS, a Linux container's memory limit) and the machine
//!   ([`MachineReading`]: cores, CPU, total and available memory). CPU needs two readings, so the
//!   probe keeps the last CPU times; the first reading has no percentage.
//! * A [`Recorder`] takes a reading every [`READ_EVERY`] while the page is open, and one a minute
//!   always, kept for an hour ([`HISTORY_KEPT`]), so the page can judge growth as soon as it opens.
//! * [`host_id`] tells which Turu processes share a computer (every container on one Linux host
//!   reads the same kernel boot id).
//!
//! Linux and Android read `/proc` and the cgroup files; macOS asks the kernel (`proc_pid_rusage`,
//! `host_statistics`); Windows asks Win32. The text parsers have no `cfg`, so their tests run
//! everywhere. Android 8 and later deny `/proc/stat`: a phone's machine CPU is `None`.

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// How often the [`Recorder`] reads while someone watches the page.
pub const READ_EVERY_MS: u64 = 2_000;
pub const READ_EVERY: Duration = Duration::from_millis(READ_EVERY_MS);
/// One history point a minute, for an hour.
pub const POINT_EVERY_MS: u64 = 60_000;
pub const HISTORY_KEPT: usize = 60;
/// `/proc`'s clock ticks per second (`USER_HZ`): 100 on Linux and Android, whatever the kernel's
/// own rate.
const TICKS_PER_SECOND: u64 = 100;

/// A Turu process: its CPU and memory.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ProcessReading {
    /// Its share of one core since the reading before, in percent (above 100 when several threads
    /// are busy), as `top` shows it.
    pub cpu_percent: Option<f32>,
    /// Resident memory, and the most it has been, in bytes.
    pub rss: u64,
    pub peak_rss: u64,
    /// macOS: the footprint, what Activity Monitor shows as its memory, and the most it has been.
    pub footprint: Option<u64>,
    pub peak_footprint: Option<u64>,
    /// In a Linux container with a memory limit: the limit and what the container uses.
    pub container: Option<Container>,
}

impl ProcessReading {
    /// The figure the page shows as the process's memory: the footprint where there is one
    /// (Activity Monitor's), else resident memory.
    pub fn memory(&self) -> u64 {
        self.footprint.unwrap_or(self.rss)
    }

    /// The most [`memory`](Self::memory) has been.
    pub fn peak(&self) -> u64 {
        self.peak_footprint.unwrap_or(self.peak_rss)
    }
}

/// A cgroup's memory limit and use, in bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Container {
    pub limit: u64,
    pub used: u64,
}

/// The computer a Turu process runs on.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct MachineReading {
    /// `std::env::consts::OS`: "linux", "macos", "windows", "android".
    pub os: String,
    /// The cores this process may use (a container's CPU quota counts).
    pub cores: u32,
    /// The whole machine's CPU since the reading before, in percent of all its cores.
    pub cpu_percent: Option<f32>,
    pub memory_total: u64,
    pub memory_available: u64,
    /// The system and its version, read once ([`os_version`]): "macOS 15.2", "Windows 11 (build
    /// 22631)", "Debian GNU/Linux 12 (bookworm) in a container, kernel 6.8.0-45".
    pub os_version: Option<String>,
    /// ms since 1970 since when the machine's CPU has been over [`BUSY_PERCENT`] in every
    /// reading; `None` while it isn't. Read every 2 s while the page is open and every minute
    /// otherwise, so it holds whether anyone watches or not.
    pub cpu_busy_since: Option<u64>,
}

/// A process and machine reading, taken together.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Sample {
    /// ms since 1970.
    pub at: u64,
    pub process: ProcessReading,
    pub machine: MachineReading,
}

/// A machine as the page gets it (`/api/state`'s `machines`): its latest reading, and the Turu
/// processes on it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineView {
    /// [`host_id`]: the same for every Turu process on the machine.
    pub host: String,
    /// When it was read, ms since 1970.
    pub at: u64,
    #[serde(flatten)]
    pub machine: MachineReading,
    pub turu: Vec<TuruProcess>,
}

/// A Turu process on a [`MachineView`].
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TuruProcess {
    /// Where its bots show on the page: `None` for the process serving it, a reporting process's
    /// prefix (`remote:vps-1:3fa9c2:`), or an Army client's entry id (`army:alt`,
    /// `remote:vps-1:3fa9c2:army:alt`).
    pub id: Option<String>,
    /// The machine heading its bots are under on the page: `None` for this machine's.
    pub machine: Option<String>,
    /// What the page calls it: "Turu", "Turu on vps-1", an Army client's label.
    pub label: String,
    #[serde(flatten)]
    pub process: ProcessReading,
}

/// What a reporting process sends of its resources with each report (`Report::resources`): its
/// machine's latest reading, its own, and its last hour.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct MachineResources {
    /// ms since 1970, on the reporting machine's clock.
    pub at: u64,
    pub machine: MachineReading,
    pub process: ProcessReading,
    /// A point a minute, oldest first.
    pub history: Vec<Point>,
}

/// A machine's last hour (`GET /api/machines/<host>/history`).
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct HostHistory {
    /// `[at, all of Turu's memory, the machine's available memory]`, a minute apart.
    pub points: Vec<Point>,
    /// Each Turu process's own: `[at, its memory, the machine's available memory or 0]`.
    pub turu: Vec<ProcessHistory>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct ProcessHistory {
    /// As [`TuruProcess::id`].
    pub id: Option<String>,
    pub points: Vec<Point>,
}

impl HostHistory {
    /// The machine's points from `base` (the process that read the machine), with every
    /// process's memory added up: at each of `base`'s minutes, each process's latest point
    /// within the minute and a half before (a process that started later adds nothing yet).
    pub fn new(base: &[Point], turu: Vec<ProcessHistory>) -> Self {
        let points = base
            .iter()
            .map(|&(at, _, available)| {
                let memory = turu
                    .iter()
                    .filter_map(|p| {
                        p.points
                            .iter()
                            .rev()
                            .find(|q| q.0 <= at + 30_000)
                            .filter(|q| q.0 + 90_000 > at)
                            .map(|q| q.1)
                    })
                    .sum();
                (at, memory, available)
            })
            .collect();
        Self { points, turu }
    }
}

/// Memory is compared in steps of this much by [`MachineView::steady`].
const STEADY_BYTES: u64 = 50 << 20;

impl MachineView {
    /// Without what moves with every reading, for `/api/events`' "did it change?": no time, CPU
    /// in whole percent, memory in 50 MB steps.
    pub fn steady(&mut self) {
        let bytes = |b: &mut u64| *b = (*b + STEADY_BYTES / 2) / STEADY_BYTES * STEADY_BYTES;
        let percent = |p: &mut Option<f32>| *p = p.map(f32::round);
        self.at = 0;
        percent(&mut self.machine.cpu_percent);
        bytes(&mut self.machine.memory_total);
        bytes(&mut self.machine.memory_available);
        for turu in &mut self.turu {
            let process = &mut turu.process;
            percent(&mut process.cpu_percent);
            bytes(&mut process.rss);
            bytes(&mut process.peak_rss);
            for footprint in [&mut process.footprint, &mut process.peak_footprint]
                .into_iter()
                .flatten()
            {
                bytes(footprint);
            }
            if let Some(container) = &mut process.container {
                bytes(&mut container.limit);
                bytes(&mut container.used);
            }
        }
    }
}

/// A minute of history: `[at, Turu's memory, the machine's available memory]`, as the page gets it.
pub type Point = (u64, u64, u64);

/// Busy and total CPU ticks since boot, any unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CpuTicks {
    pub busy: u64,
    pub total: u64,
}

impl CpuTicks {
    /// The busy share between `before` and `self`, in percent; `None` when no time passed.
    pub fn percent_since(self, before: CpuTicks) -> Option<f32> {
        let total = self.total.checked_sub(before.total)?;
        let busy = self.busy.checked_sub(before.busy)?;
        (total > 0).then(|| (busy as f64 * 100.0 / total as f64).min(100.0) as f32)
    }
}

/// Reads this process and this machine, keeping the last CPU times for the next percentage.
pub struct Probe {
    process_cpu: Option<(Instant, Duration)>,
    machine_cpu: Option<CpuTicks>,
    busy_since: Option<u64>,
    #[cfg(target_os = "macos")]
    host: libc::mach_port_t,
}

impl Default for Probe {
    fn default() -> Self {
        Self::new()
    }
}

impl Probe {
    pub fn new() -> Self {
        Self {
            process_cpu: None,
            machine_cpu: None,
            busy_since: None,
            // A send right to the host port; kept for the probe's life rather than asked for
            // (and leaked) with every reading.
            #[cfg(target_os = "macos")]
            #[allow(deprecated, reason = "libc points Mach calls to the mach2 crate; same call")]
            // SAFETY: `mach_host_self` has no preconditions; it returns a port name, nothing to free.
            host: unsafe { libc::mach_host_self() },
        }
    }

    /// This process now; `None` where it can't be read.
    pub fn process(&mut self) -> Option<ProcessReading> {
        let raw = os::process()?;
        let now = Instant::now();
        let cpu_percent = match (self.process_cpu, raw.cpu_time) {
            (Some((then, before)), Some(cpu)) => {
                let elapsed = now.duration_since(then);
                cpu.checked_sub(before)
                    .filter(|_| !elapsed.is_zero())
                    .map(|used| (used.as_secs_f64() * 100.0 / elapsed.as_secs_f64()) as f32)
            }
            _ => None,
        };
        self.process_cpu = raw.cpu_time.map(|cpu| (now, cpu));
        Some(ProcessReading {
            cpu_percent,
            rss: raw.rss,
            peak_rss: raw.peak_rss.max(raw.rss),
            footprint: raw.footprint,
            peak_footprint: raw
                .peak_footprint
                .map(|peak| peak.max(raw.footprint.unwrap_or(0))),
            container: os::container(),
        })
    }

    /// This machine now; `None` where its memory can't be read.
    pub fn machine(&mut self) -> Option<MachineReading> {
        #[cfg(target_os = "macos")]
        let (memory, ticks) = (os::memory(self.host), os::cpu_ticks(self.host));
        #[cfg(not(target_os = "macos"))]
        let (memory, ticks) = (os::memory(), os::cpu_ticks());
        let (memory_total, memory_available) = memory?;
        let cpu_percent = match (self.machine_cpu, ticks) {
            (Some(before), Some(now)) => now.percent_since(before),
            _ => None,
        };
        if ticks.is_some() {
            self.machine_cpu = ticks;
        }
        self.busy_since = busy_since(self.busy_since, cpu_percent, super::snapshot::now_ms());
        Some(MachineReading {
            os: std::env::consts::OS.to_owned(),
            cores: std::thread::available_parallelism().map_or(1, |n| n.get() as u32),
            cpu_percent,
            memory_total,
            memory_available,
            os_version: os_version(),
            cpu_busy_since: self.busy_since,
        })
    }
}

/// The machine's CPU over this is busy, for [`MachineReading::cpu_busy_since`]. How long that
/// may last before the page says so is the page's to decide.
pub const BUSY_PERCENT: f32 = 90.0;

/// When the machine's CPU went over [`BUSY_PERCENT`] and has stayed there, given the time before
/// and this reading's percentage (`None` keeps what was known).
pub fn busy_since(before: Option<u64>, percent: Option<f32>, now: u64) -> Option<u64> {
    match percent {
        Some(p) if p >= BUSY_PERCENT => Some(before.unwrap_or(now)),
        Some(_) => None,
        None => before,
    }
}

/// Keeps the latest [`Sample`] and an hour of [`Point`]s.
#[derive(Default)]
pub struct Recorder {
    probe: Probe,
    latest: Option<Sample>,
    history: MinuteRing,
    /// ms since 1970 of the last reading.
    last_read: Option<u64>,
}

impl Recorder {
    /// Reads when due: every [`READ_EVERY`] while `watched`, and once a minute always (the
    /// minute's point). `now` is ms since 1970.
    pub fn tick(&mut self, now: u64, watched: bool) {
        let read_due = self
            .last_read
            .is_none_or(|t| now.saturating_sub(t) >= READ_EVERY_MS);
        if !(watched && read_due) && !self.history.due(now) {
            return;
        }
        self.last_read = Some(now);
        let (Some(process), Some(machine)) = (self.probe.process(), self.probe.machine()) else {
            return;
        };
        self.record(Sample {
            at: now,
            process,
            machine,
        });
    }

    /// Keeps `sample` as the latest, and as the minute's point when one is due.
    pub fn record(&mut self, sample: Sample) {
        self.history.push((
            sample.at,
            sample.process.memory(),
            sample.machine.memory_available,
        ));
        self.latest = Some(sample);
    }

    pub fn latest(&self) -> Option<Sample> {
        self.latest.clone()
    }

    /// The last hour, oldest first.
    pub fn history(&self) -> Vec<Point> {
        self.history.points()
    }

    /// The latest reading with the last hour, for a report to another machine's monitor.
    pub fn resources(&self) -> Option<MachineResources> {
        let latest = self.latest.clone()?;
        Some(MachineResources {
            at: latest.at,
            machine: latest.machine,
            process: latest.process,
            history: self.history(),
        })
    }
}

/// A point a minute, the last [`HISTORY_KEPT`].
#[derive(Debug, Clone, Default)]
pub struct MinuteRing(VecDeque<Point>);

impl MinuteRing {
    /// Whether a point at `at` would be kept: a minute after the last.
    pub fn due(&self, at: u64) -> bool {
        self.0
            .back()
            .is_none_or(|p| at.saturating_sub(p.0) >= POINT_EVERY_MS)
    }

    /// Keeps `point` when it's [`due`](Self::due).
    pub fn push(&mut self, point: Point) {
        if self.due(point.0) {
            self.0.push_back(point);
            while self.0.len() > HISTORY_KEPT {
                self.0.pop_front();
            }
        }
    }

    /// Takes on `points` (another process's hour, sent with its first report) while empty.
    pub fn seed(&mut self, points: &[Point]) {
        if self.0.is_empty() {
            for &point in points {
                self.push(point);
            }
        }
    }

    pub fn points(&self) -> Vec<Point> {
        self.0.iter().copied().collect()
    }
}

/// This process now, for an Army client's `/snapshot`: its CPU covers the time since the last
/// call (the Army poll's 2 s).
pub fn this_process() -> Option<ProcessReading> {
    static PROBE: std::sync::LazyLock<parking_lot::Mutex<Probe>> =
        std::sync::LazyLock::new(Default::default);
    PROBE.lock().process()
}

/// What tells this computer from others, the same for every Turu process on it (containers
/// included on Linux): the kernel's boot id on Linux and Android, the boot session on macOS, the
/// computer's name and boot minute on Windows. Empty when it can't be read.
pub fn host_id() -> String {
    static ID: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    // Lowercase, as the hub keeps reported ones.
    ID.get_or_init(|| os::host_id().unwrap_or_default().to_lowercase())
        .clone()
}

/// The system and its version (see [`MachineReading::os_version`]): it doesn't change while
/// Turu runs, so it's read once.
pub fn os_version() -> Option<String> {
    static VERSION: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
    VERSION.get_or_init(os::os_version).clone()
}

/// What the OS gave for this process.
#[derive(Debug, Default)]
#[cfg_attr(not(any(unix, windows)), allow(dead_code))]
struct RawProcess {
    rss: u64,
    peak_rss: u64,
    footprint: Option<u64>,
    peak_footprint: Option<u64>,
    cpu_time: Option<Duration>,
}

// ---- Text parsers (Linux, Android; tested everywhere) ----

/// A `Name:   1234 kB` line's value, in bytes.
pub fn field_kb(text: &str, name: &str) -> Option<u64> {
    let value = text.lines().find_map(|line| line.strip_prefix(name))?;
    let kb: u64 = value.trim().trim_end_matches("kB").trim().parse().ok()?;
    Some(kb * 1024)
}

/// `utime` + `stime` from `/proc/<pid>/stat`. Its second field, the thread's name in
/// parentheses, can hold spaces and parentheses, so the fields are counted after its last `)`.
pub fn parse_cpu_time(stat: &str) -> Option<Duration> {
    let rest = &stat[stat.rfind(')')? + 1..];
    // Fields 3 (state) onwards: utime is field 14, stime 15.
    let mut fields = rest.split_whitespace().skip(11);
    let utime: u64 = fields.next()?.parse().ok()?;
    let stime: u64 = fields.next()?.parse().ok()?;
    let ticks = utime + stime;
    Some(Duration::from_millis(ticks * 1000 / TICKS_PER_SECOND))
}

/// `MemTotal` and `MemAvailable` from `/proc/meminfo`, in bytes.
pub fn parse_meminfo(text: &str) -> Option<(u64, u64)> {
    Some((
        field_kb(text, "MemTotal:")?,
        field_kb(text, "MemAvailable:")?,
    ))
}

/// The `cpu` line of `/proc/stat`: busy and total ticks (idle and iowait are idle; guest time is
/// already in user time).
pub fn parse_proc_stat(text: &str) -> Option<CpuTicks> {
    let line = text.lines().find(|l| l.starts_with("cpu "))?;
    let ticks: Vec<u64> = line
        .split_whitespace()
        .skip(1)
        .take(8)
        .map(|t| t.parse().ok())
        .collect::<Option<_>>()?;
    if ticks.len() < 4 {
        return None;
    }
    let total: u64 = ticks.iter().sum();
    let idle = ticks[3] + ticks.get(4).copied().unwrap_or(0);
    Some(CpuTicks {
        busy: total - idle,
        total,
    })
}

/// The memory cgroup's path from `/proc/self/cgroup`: v2's (`0::/path`) when there is one, else
/// v1's memory controller's (`N:memory:/path`), with which of the two it is.
pub fn parse_cgroup(text: &str) -> Option<(CgroupVersion, String)> {
    let mut v1 = None;
    for line in text.lines() {
        let mut parts = line.splitn(3, ':');
        let (Some(_), Some(controllers), Some(path)) = (parts.next(), parts.next(), parts.next())
        else {
            continue;
        };
        if controllers.is_empty() {
            return Some((CgroupVersion::V2, path.to_owned()));
        }
        if controllers.split(',').any(|c| c == "memory") {
            v1 = Some((CgroupVersion::V1, path.to_owned()));
        }
    }
    v1
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CgroupVersion {
    V1,
    V2,
}

/// A cgroup memory limit file's value: none for v2's "max", or for v1's "unlimited" (a number
/// near `u64::MAX` rounded to pages), or for one at or above the machine's memory.
pub fn parse_limit(text: &str, memory_total: u64) -> Option<u64> {
    let limit: u64 = text.trim().parse().ok()?;
    (limit > 0 && limit < memory_total).then_some(limit)
}

/// `PRETTY_NAME` from `/etc/os-release` ("Debian GNU/Linux 12 (bookworm)"), else `NAME` and
/// `VERSION_ID`, without their quotes.
pub fn parse_os_release(text: &str) -> Option<String> {
    let field = |name: &str| {
        text.lines()
            .find_map(|line| line.strip_prefix(name)?.strip_prefix('='))
            .map(|value| value.trim().trim_matches(['"', '\'']).trim().to_owned())
            .filter(|value| !value.is_empty())
    };
    field("PRETTY_NAME").or_else(|| match (field("NAME"), field("VERSION_ID")) {
        (Some(name), Some(version)) => Some(format!("{name} {version}")),
        (name, _) => name,
    })
}

/// A Linux or Android system with its kernel: the distribution is the container image's inside a
/// container, while the kernel is the host's, so a container says so.
pub fn describe_linux(
    system: Option<&str>,
    kernel: Option<&str>,
    container: bool,
) -> Option<String> {
    let mut text = system.unwrap_or("Linux").to_owned();
    if container {
        text.push_str(" in a container");
    }
    if let Some(kernel) = kernel.map(str::trim).filter(|k| !k.is_empty()) {
        text.push_str(", kernel ");
        text.push_str(kernel);
    }
    (system.is_some() || kernel.is_some()).then_some(text)
}

/// Windows from `RtlGetVersion`'s numbers: Windows 11 still says 10.0, with a build of 22000 on.
pub fn describe_windows(major: u32, minor: u32, build: u32) -> String {
    match (major, minor) {
        (10, 0) if build >= 22_000 => format!("Windows 11 (build {build})"),
        (10, 0) => format!("Windows 10 (build {build})"),
        _ => format!("Windows {major}.{minor} (build {build})"),
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
mod os {
    use super::*;
    use std::path::{Path, PathBuf};

    pub(super) fn process() -> Option<RawProcess> {
        let status = std::fs::read_to_string("/proc/self/status").ok()?;
        let rss = field_kb(&status, "VmRSS:")?;
        Some(RawProcess {
            rss,
            peak_rss: field_kb(&status, "VmHWM:").unwrap_or(rss),
            footprint: None,
            peak_footprint: None,
            cpu_time: std::fs::read_to_string("/proc/self/stat")
                .ok()
                .and_then(|text| parse_cpu_time(&text)),
        })
    }

    pub(super) fn memory() -> Option<(u64, u64)> {
        parse_meminfo(&std::fs::read_to_string("/proc/meminfo").ok()?)
    }

    pub(super) fn cpu_ticks() -> Option<CpuTicks> {
        parse_proc_stat(&std::fs::read_to_string("/proc/stat").ok()?)
    }

    pub(super) fn container() -> Option<Container> {
        let (version, path) = parse_cgroup(&std::fs::read_to_string("/proc/self/cgroup").ok()?)?;
        let (root, limit, usage) = match version {
            CgroupVersion::V2 => ("/sys/fs/cgroup", "memory.max", "memory.current"),
            CgroupVersion::V1 => (
                "/sys/fs/cgroup/memory",
                "memory.limit_in_bytes",
                "memory.usage_in_bytes",
            ),
        };
        let total = memory()?.0;
        // Inside a container with its own cgroup namespace the path is `/`; without one it's the
        // host's path, which may not be mounted inside: then the root's files are the group's.
        let group = Path::new(root).join(path.trim_start_matches('/'));
        let dirs: [PathBuf; 2] = [group, PathBuf::from(root)];
        dirs.iter().find_map(|dir| {
            let limit = parse_limit(&std::fs::read_to_string(dir.join(limit)).ok()?, total)?;
            let used = std::fs::read_to_string(dir.join(usage))
                .ok()?
                .trim()
                .parse()
                .ok()?;
            Some(Container { limit, used })
        })
    }

    pub(super) fn host_id() -> Option<String> {
        let id = std::fs::read_to_string("/proc/sys/kernel/random/boot_id").ok()?;
        Some(id.trim().to_owned()).filter(|id| !id.is_empty())
    }

    pub(super) fn os_version() -> Option<String> {
        let kernel = std::fs::read_to_string("/proc/sys/kernel/osrelease").ok();
        #[cfg(target_os = "android")]
        let (system, container) = (android_release().map(|r| format!("Android {r}")), false);
        #[cfg(not(target_os = "android"))]
        let (system, container) = (
            std::fs::read_to_string("/etc/os-release")
                .ok()
                .and_then(|text| parse_os_release(&text)),
            // Docker's marker, and Podman's.
            Path::new("/.dockerenv").exists() || Path::new("/run/.containerenv").exists(),
        );
        describe_linux(system.as_deref(), kernel.as_deref(), container)
    }

    /// The Android release ("14"), from the system property apps may read.
    #[cfg(target_os = "android")]
    fn android_release() -> Option<String> {
        let mut value = [0u8; libc::PROP_VALUE_MAX as usize];
        // SAFETY: the property's value is at most PROP_VALUE_MAX bytes with its nul.
        let len = unsafe {
            libc::__system_property_get(
                c"ro.build.version.release".as_ptr(),
                value.as_mut_ptr().cast(),
            )
        };
        let len = usize::try_from(len).ok().filter(|&n| n > 0)?;
        Some(
            String::from_utf8_lossy(&value[..len.min(value.len())])
                .trim()
                .to_owned(),
        )
    }
}

#[cfg(target_os = "macos")]
mod os {
    use super::*;
    use std::mem::{MaybeUninit, size_of};

    pub(super) fn process() -> Option<RawProcess> {
        // SAFETY: both calls fill the structs they're given and say whether they did.
        unsafe {
            let mut info = MaybeUninit::<libc::rusage_info_v4>::zeroed();
            if libc::proc_pid_rusage(
                libc::getpid(),
                libc::RUSAGE_INFO_V4,
                info.as_mut_ptr().cast::<libc::rusage_info_t>(),
            ) != 0
            {
                return None;
            }
            let info = info.assume_init();
            let mut usage = MaybeUninit::<libc::rusage>::zeroed();
            let usage = (libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) == 0)
                .then(|| usage.assume_init());
            let seconds = |t: libc::timeval| {
                Duration::from_secs(t.tv_sec as u64) + Duration::from_micros(t.tv_usec as u64)
            };
            Some(RawProcess {
                rss: info.ri_resident_size,
                // macOS gives `ru_maxrss` in bytes (Linux in kB).
                peak_rss: usage.map_or(0, |u| u.ru_maxrss as u64),
                footprint: Some(info.ri_phys_footprint),
                peak_footprint: Some(info.ri_lifetime_max_phys_footprint),
                cpu_time: usage.map(|u| seconds(u.ru_utime) + seconds(u.ru_stime)),
            })
        }
    }

    fn sysctl_u64(name: &std::ffi::CStr) -> Option<u64> {
        let mut value = 0u64;
        let mut size = size_of::<u64>();
        // SAFETY: `value` has room for `size` bytes.
        let ok = unsafe {
            libc::sysctlbyname(
                name.as_ptr(),
                (&raw mut value).cast(),
                &mut size,
                std::ptr::null_mut(),
                0,
            )
        } == 0;
        (ok && size == size_of::<u64>()).then_some(value)
    }

    /// Total, and free + inactive + purgeable pages as available (what the kernel can hand out
    /// without compressing or swapping).
    pub(super) fn memory(host: libc::mach_port_t) -> Option<(u64, u64)> {
        let total = sysctl_u64(c"hw.memsize")?;
        // SAFETY: as `process`'s; `sysconf` has no preconditions.
        unsafe {
            let mut vm = MaybeUninit::<libc::vm_statistics64>::zeroed();
            let mut count = libc::HOST_VM_INFO64_COUNT;
            if libc::host_statistics64(
                host,
                libc::HOST_VM_INFO64,
                vm.as_mut_ptr().cast(),
                &mut count,
            ) != libc::KERN_SUCCESS
            {
                return None;
            }
            let vm = vm.assume_init();
            let page = u64::try_from(libc::sysconf(libc::_SC_PAGESIZE)).ok()?;
            let pages = u64::from(vm.free_count)
                + u64::from(vm.inactive_count)
                + u64::from(vm.purgeable_count);
            Some((total, (pages * page).min(total)))
        }
    }

    pub(super) fn cpu_ticks(host: libc::mach_port_t) -> Option<CpuTicks> {
        // SAFETY: as `process`'s.
        unsafe {
            let mut load = MaybeUninit::<libc::host_cpu_load_info>::zeroed();
            let mut count = libc::HOST_CPU_LOAD_INFO_COUNT;
            if libc::host_statistics(
                host,
                libc::HOST_CPU_LOAD_INFO,
                load.as_mut_ptr().cast(),
                &mut count,
            ) != libc::KERN_SUCCESS
            {
                return None;
            }
            let ticks = load.assume_init().cpu_ticks.map(u64::from);
            let idle = ticks[libc::CPU_STATE_IDLE as usize];
            let total: u64 = ticks.iter().sum();
            Some(CpuTicks {
                busy: total - idle,
                total,
            })
        }
    }

    pub(super) fn container() -> Option<Container> {
        None
    }

    /// A text `sysctl` (at most 63 bytes).
    fn sysctl_string(name: &std::ffi::CStr) -> Option<String> {
        let mut buffer = [0u8; 64];
        let mut size = buffer.len();
        // SAFETY: `buffer` has room for `size` bytes.
        let ok = unsafe {
            libc::sysctlbyname(
                name.as_ptr(),
                buffer.as_mut_ptr().cast(),
                &mut size,
                std::ptr::null_mut(),
                0,
            )
        } == 0;
        if !ok {
            return None;
        }
        let text = std::ffi::CStr::from_bytes_until_nul(&buffer[..size.min(buffer.len())])
            .ok()?
            .to_str()
            .ok()?;
        Some(text.trim().to_owned()).filter(|text| !text.is_empty())
    }

    pub(super) fn host_id() -> Option<String> {
        sysctl_string(c"kern.bootsessionuuid")
    }

    pub(super) fn os_version() -> Option<String> {
        sysctl_string(c"kern.osproductversion").map(|version| format!("macOS {version}"))
    }
}

#[cfg(windows)]
mod os {
    use super::*;
    use std::mem::{size_of, zeroed};
    use windows_sys::Win32::Foundation::FILETIME;
    use windows_sys::Win32::System::ProcessStatus::{
        GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
    };
    use windows_sys::Win32::System::SystemInformation::{
        GetTickCount64, GlobalMemoryStatusEx, MEMORYSTATUSEX,
    };
    use windows_sys::Win32::System::Threading::{
        GetCurrentProcess, GetProcessTimes, GetSystemTimes,
    };

    /// A FILETIME's count of 100 ns.
    fn ticks(time: FILETIME) -> u64 {
        (u64::from(time.dwHighDateTime) << 32) | u64::from(time.dwLowDateTime)
    }

    pub(super) fn process() -> Option<RawProcess> {
        // SAFETY: each call fills the structs it's given and says whether it did; the current
        // process's handle needs no closing.
        unsafe {
            let process = GetCurrentProcess();
            let mut counters: PROCESS_MEMORY_COUNTERS = zeroed();
            counters.cb = size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
            if GetProcessMemoryInfo(process, &mut counters, counters.cb) == 0 {
                return None;
            }
            let (mut created, mut exited, mut kernel, mut user) =
                (zeroed(), zeroed(), zeroed(), zeroed());
            let cpu_time =
                (GetProcessTimes(process, &mut created, &mut exited, &mut kernel, &mut user) != 0)
                    .then(|| Duration::from_nanos((ticks(kernel) + ticks(user)) * 100));
            Some(RawProcess {
                rss: counters.WorkingSetSize as u64,
                peak_rss: counters.PeakWorkingSetSize as u64,
                footprint: None,
                peak_footprint: None,
                cpu_time,
            })
        }
    }

    pub(super) fn memory() -> Option<(u64, u64)> {
        // SAFETY: as `process`'s.
        unsafe {
            let mut status: MEMORYSTATUSEX = zeroed();
            status.dwLength = size_of::<MEMORYSTATUSEX>() as u32;
            (GlobalMemoryStatusEx(&mut status) != 0)
                .then_some((status.ullTotalPhys, status.ullAvailPhys))
        }
    }

    pub(super) fn cpu_ticks() -> Option<CpuTicks> {
        // SAFETY: as `process`'s.
        unsafe {
            let (mut idle, mut kernel, mut user) = (zeroed(), zeroed(), zeroed());
            if GetSystemTimes(&mut idle, &mut kernel, &mut user) == 0 {
                return None;
            }
            // Kernel time includes the idle time.
            let total = ticks(kernel) + ticks(user);
            Some(CpuTicks {
                busy: total.saturating_sub(ticks(idle)),
                total,
            })
        }
    }

    pub(super) fn container() -> Option<Container> {
        None
    }

    pub(super) fn host_id() -> Option<String> {
        let name = std::env::var("COMPUTERNAME").unwrap_or_default();
        // SAFETY: no preconditions.
        let up = Duration::from_millis(unsafe { GetTickCount64() });
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .ok()?;
        let boot_minute = now.checked_sub(up)?.as_secs() / 60;
        Some(format!("{}-{boot_minute}", name.to_lowercase()))
    }

    pub(super) fn os_version() -> Option<String> {
        use windows_sys::Wdk::System::SystemServices::RtlGetVersion;
        use windows_sys::Win32::System::SystemInformation::OSVERSIONINFOW;
        // `GetVersionExW` says 6.2 to a program without a manifest; `RtlGetVersion` doesn't.
        // SAFETY: as `process`'s.
        unsafe {
            let mut info: OSVERSIONINFOW = zeroed();
            info.dwOSVersionInfoSize = size_of::<OSVERSIONINFOW>() as u32;
            (RtlGetVersion(&mut info) == 0).then(|| {
                describe_windows(info.dwMajorVersion, info.dwMinorVersion, info.dwBuildNumber)
            })
        }
    }
}

#[cfg(not(any(
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    windows
)))]
mod os {
    use super::*;

    pub(super) fn process() -> Option<RawProcess> {
        None
    }
    pub(super) fn memory() -> Option<(u64, u64)> {
        None
    }
    pub(super) fn cpu_ticks() -> Option<CpuTicks> {
        None
    }
    pub(super) fn container() -> Option<Container> {
        None
    }
    pub(super) fn host_id() -> Option<String> {
        None
    }
    pub(super) fn os_version() -> Option<String> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MB: u64 = 1 << 20;

    #[test]
    fn reads_cpu_time_after_the_name() {
        // A name with a space and a parenthesis; utime 1234, stime 566 ticks.
        let stat =
            "4043 (Turu (x) y) S 1 4043 0 0 -1 4194624 9000 0 0 0 1234 566 0 0 20 0 60 0 100 0";
        assert_eq!(parse_cpu_time(stat), Some(Duration::from_secs(18)));
        assert_eq!(parse_cpu_time("4043 (turu) S 1"), None);
    }

    #[test]
    fn reads_meminfo() {
        let text = "MemTotal:        3798016 kB\nMemFree:          212340 kB\nMemAvailable:    1273856 kB\n";
        assert_eq!(parse_meminfo(text), Some((3798016 * 1024, 1273856 * 1024)));
        assert_eq!(parse_meminfo("MemTotal: 10 kB\n"), None);
    }

    #[test]
    fn reads_the_machines_cpu_from_proc_stat() {
        // user nice system idle iowait irq softirq steal guest guest_nice
        let before = parse_proc_stat("cpu  100 0 50 800 50 0 0 0 0 0\ncpu0 1 2 3 4\n").unwrap();
        let now = parse_proc_stat("cpu  160 0 70 1000 70 0 0 0 0 0\n").unwrap();
        assert_eq!(
            before,
            CpuTicks {
                busy: 150,
                total: 1000
            }
        );
        // 80 busy of 300.
        let percent = now.percent_since(before).unwrap();
        assert!((percent - 26.666).abs() < 0.01, "{percent}");
        assert_eq!(now.percent_since(now), None);
        assert_eq!(parse_proc_stat("intr 1 2 3\n"), None);
    }

    #[test]
    fn finds_the_memory_cgroup() {
        assert_eq!(
            parse_cgroup("0::/\n"),
            Some((CgroupVersion::V2, "/".to_owned()))
        );
        let v1 = "12:pids:/docker/abc\n4:cpu,cpuacct:/docker/abc\n9:memory:/docker/abc\n1:name=systemd:/docker/abc\n";
        assert_eq!(
            parse_cgroup(v1),
            Some((CgroupVersion::V1, "/docker/abc".to_owned()))
        );
        // A hybrid host: v2's unified line wins.
        assert_eq!(
            parse_cgroup("9:memory:/user.slice\n0::/user.slice/session-2.scope\n")
                .unwrap()
                .0,
            CgroupVersion::V2
        );
        assert_eq!(parse_cgroup("12:pids:/x\n"), None);
    }

    #[test]
    fn names_the_system() {
        let debian = "PRETTY_NAME=\"Debian GNU/Linux 12 (bookworm)\"\nNAME=\"Debian GNU/Linux\"\nVERSION_ID=\"12\"\n";
        assert_eq!(
            parse_os_release(debian).as_deref(),
            Some("Debian GNU/Linux 12 (bookworm)")
        );
        assert_eq!(
            parse_os_release("NAME='Alpine Linux'\nVERSION_ID=3.20.3\n").as_deref(),
            Some("Alpine Linux 3.20.3")
        );
        assert_eq!(parse_os_release("ID=x\n"), None);
        assert_eq!(
            describe_linux(
                Some("Debian GNU/Linux 12 (bookworm)"),
                Some("6.8.0-45-generic\n"),
                true
            )
            .as_deref(),
            Some("Debian GNU/Linux 12 (bookworm) in a container, kernel 6.8.0-45-generic")
        );
        assert_eq!(
            describe_linux(None, Some("5.15.153"), false).as_deref(),
            Some("Linux, kernel 5.15.153")
        );
        assert_eq!(describe_linux(None, None, false), None);
        assert_eq!(describe_windows(10, 0, 22631), "Windows 11 (build 22631)");
        assert_eq!(describe_windows(10, 0, 19045), "Windows 10 (build 19045)");
        assert_eq!(describe_windows(6, 1, 7601), "Windows 6.1 (build 7601)");
    }

    #[test]
    fn treats_no_limit_as_none() {
        let total = 8192 * MB;
        assert_eq!(parse_limit("1073741824\n", total), Some(1024 * MB));
        assert_eq!(parse_limit("max\n", total), None);
        // cgroup v1's "unlimited".
        assert_eq!(parse_limit("9223372036854771712\n", total), None);
        assert_eq!(parse_limit(&format!("{total}"), total), None);
    }

    fn sample(at: u64, memory: u64) -> Sample {
        Sample {
            at,
            process: ProcessReading {
                rss: memory,
                ..Default::default()
            },
            machine: MachineReading {
                memory_available: 2048 * MB,
                ..Default::default()
            },
        }
    }

    #[test]
    fn keeps_a_point_a_minute_for_an_hour() {
        let mut recorder = Recorder::default();
        let start = 1_763_200_000_000;
        // Every 2 s for 90 minutes.
        for i in 0..2700u64 {
            recorder.record(sample(start + i * 2_000, (400 + i / 30) * MB));
        }
        let history = recorder.history();
        assert_eq!(history.len(), HISTORY_KEPT);
        assert_eq!(history.last().unwrap().0, start + 89 * 60_000);
        assert_eq!(history[0].0, start + 30 * 60_000);
        assert_eq!(history[0].1, 430 * MB);
        assert_eq!(history[0].2, 2048 * MB);
        assert_eq!(recorder.latest().unwrap().at, start + 2699 * 2_000);
    }

    #[test]
    fn steady_hides_small_moves() {
        let view = |cpu: f32, rss: u64, available: u64, at| MachineView {
            host: "h".into(),
            at,
            machine: MachineReading {
                cpu_percent: Some(cpu),
                memory_available: available,
                ..Default::default()
            },
            turu: vec![TuruProcess {
                id: None,
                machine: None,
                label: "Turu".into(),
                process: ProcessReading {
                    cpu_percent: Some(cpu),
                    rss,
                    ..Default::default()
                },
            }],
        };
        let mut a = view(9.2, 410 * MB + 123_456, 2100 * MB, 1);
        let mut b = view(8.9, 412 * MB, 2090 * MB, 2);
        a.steady();
        b.steady();
        assert_eq!(a, b);
        let mut c = view(12.0, 412 * MB, 2090 * MB, 3);
        c.steady();
        assert_ne!(a, c);
    }

    #[test]
    fn adds_up_the_processes_of_a_machine() {
        let base = [
            (60_000, 400 * MB, 2048 * MB),
            (120_000, 410 * MB, 2000 * MB),
        ];
        let turu = vec![
            ProcessHistory {
                id: None,
                points: base.to_vec(),
            },
            // An Army client a few seconds out of step, which started in the second minute.
            ProcessHistory {
                id: Some("army:alt".into()),
                points: vec![(125_000, 300 * MB, 0)],
            },
        ];
        let history = HostHistory::new(&base, turu);
        assert_eq!(
            history.points,
            [
                (60_000, 400 * MB, 2048 * MB),
                (120_000, 710 * MB, 2000 * MB)
            ]
        );
    }

    #[test]
    fn knows_since_when_the_cpu_is_busy() {
        assert_eq!(busy_since(None, Some(95.0), 1_000), Some(1_000));
        assert_eq!(busy_since(Some(1_000), Some(99.0), 9_000), Some(1_000));
        // A reading without a percentage changes nothing; one under it ends the stretch.
        assert_eq!(busy_since(Some(1_000), None, 9_000), Some(1_000));
        assert_eq!(busy_since(Some(1_000), Some(40.0), 9_000), None);
        assert_eq!(busy_since(None, Some(40.0), 9_000), None);
    }

    #[test]
    fn seeds_a_ring_once() {
        let mut ring = MinuteRing::default();
        ring.seed(&[(0, 1, 0), (30_000, 2, 0), (60_000, 3, 0)]);
        assert_eq!(ring.points(), [(0, 1, 0), (60_000, 3, 0)]);
        ring.seed(&[(500_000, 9, 0)]);
        assert_eq!(ring.points().len(), 2);
    }

    #[test]
    fn uses_the_footprint_where_there_is_one() {
        let mut reading = ProcessReading {
            rss: 500 * MB,
            ..Default::default()
        };
        assert_eq!(reading.memory(), 500 * MB);
        reading.peak_rss = 600 * MB;
        assert_eq!(reading.peak(), 600 * MB);
        reading.footprint = Some(410 * MB);
        reading.peak_footprint = Some(450 * MB);
        assert_eq!(reading.memory(), 410 * MB);
        assert_eq!(reading.peak(), 450 * MB);
    }

    #[test]
    fn reads_this_process_and_machine() {
        if !cfg!(any(target_os = "linux", target_os = "macos", windows)) {
            return;
        }
        let mut probe = Probe::new();
        let first = probe.process().expect("a process reading");
        assert!(first.rss > MB, "{first:?}");
        assert!(first.peak_rss >= first.rss);
        assert_eq!(first.cpu_percent, None);
        // Burn a little CPU so the second reading has something to say.
        let until = Instant::now() + Duration::from_millis(50);
        let mut n = 0u64;
        while Instant::now() < until {
            n = n.wrapping_add(1);
        }
        std::hint::black_box(n);
        assert!(probe.process().unwrap().cpu_percent.is_some());
        let machine = probe.machine().expect("a machine reading");
        assert!(machine.memory_total > 256 * MB, "{machine:?}");
        assert!(machine.memory_available <= machine.memory_total);
        assert!(machine.cores >= 1);
        assert!(!host_id().is_empty());
        assert!(machine.os_version.is_some(), "{machine:?}");
    }
}
