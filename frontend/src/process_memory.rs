//! Turu: how much memory this process uses, from Linux's `/proc/self` (Android, Linux; `None`
//! elsewhere). The Android app logs a reading every minute and keeps them for its exit reports
//! (`android/src/memory.rs`): a Pixel 8 aborted after 27 minutes in the background, at 1.9 GB of
//! RSS with 3 GB of the phone's memory free, and nothing said what had grown.
//!
//! A reading also has the CPU time the process used so far ([`Reading::cpu_time`]), so two give
//! its share of a core in between ([`cpu_percent`]): OPPO's ColorOS closed Turu in the background
//! "for using too much CPU", and the report couldn't say how much it had used.
//!
//! A [`Reading`] is cheap (three small files and a count of `/proc/self/maps`' lines);
//! [`largest_mappings`] reads all of `maps` and is for every few minutes.

use std::fmt;
use std::io::{BufRead, BufReader};
use std::time::Duration;

const MB: u64 = 1 << 20;
/// `/proc`'s clock ticks per second (`USER_HZ`): 100 on Linux and Android, whatever the kernel's
/// own rate.
const TICKS_PER_SECOND: u64 = 100;

/// One reading of this process's memory, in bytes.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Reading {
    /// Resident memory (`VmRSS`), and its parts: anonymous (the heaps), file-backed, shared.
    pub rss: u64,
    pub rss_anon: u64,
    pub rss_file: u64,
    pub rss_shmem: u64,
    /// Address space reserved (`VmSize`).
    pub virtual_size: u64,
    /// Swapped out (`VmSwap`; zram on phones).
    pub swap: u64,
    pub threads: u64,
    /// Proportional set size (`/proc/self/smaps_rollup`), when it can be read.
    pub pss: Option<u64>,
    /// Lines of `/proc/self/maps`: the kernel refuses new mappings past `vm.max_map_count`
    /// (65530 by default), and then `malloc` fails though memory is free.
    pub maps: Option<u64>,
    /// Bytes `malloc` has handed out and not freed (`mallinfo`), when the caller adds it. Pages
    /// never written to don't count in RSS, so this can be above `rss_anon`.
    pub heap: Option<u64>,
    /// CPU time used since the process started, all threads, user and system
    /// (`/proc/self/stat`); not part of the [`Display`](fmt::Display) text.
    pub cpu_time: Option<Duration>,
}

impl Reading {
    /// This process's memory now; `None` without `/proc/self/status`.
    pub fn now() -> Option<Self> {
        let status = std::fs::read_to_string("/proc/self/status").ok()?;
        let mut reading = parse_status(&status)?;
        reading.pss = std::fs::read_to_string("/proc/self/smaps_rollup")
            .ok()
            .and_then(|text| field_kb(&text, "Pss:"));
        reading.maps = std::fs::File::open("/proc/self/maps")
            .ok()
            .map(|file| BufReader::new(file).split(b'\n').count() as u64);
        reading.cpu_time = std::fs::read_to_string("/proc/self/stat")
            .ok()
            .and_then(|text| parse_cpu_time(&text));
        Some(reading)
    }
}

/// The share of one core `now` used since `before`, `elapsed` apart, in percent (above 100 when
/// several threads are busy); `None` without both CPU times.
pub fn cpu_percent(before: &Reading, now: &Reading, elapsed: Duration) -> Option<f64> {
    let used = now.cpu_time?.checked_sub(before.cpu_time?)?;
    (!elapsed.is_zero()).then(|| used.as_secs_f64() * 100.0 / elapsed.as_secs_f64())
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

/// One thread's CPU time so far, from `/proc/self/task/<tid>/stat`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadCpu {
    pub tid: u32,
    /// The thread's name (`comm`: at most 15 bytes, so "Turu Monitor Report" is "Turu Monitor Re").
    pub name: String,
    /// User and system time.
    pub time: Duration,
}

/// Every thread of this process with its CPU time so far; empty without `/proc/self/task`. About
/// 60 small files on a farming client: for every reading, not every frame.
pub fn thread_cpu_times() -> Vec<ThreadCpu> {
    let Ok(tasks) = std::fs::read_dir("/proc/self/task") else {
        return Vec::new();
    };
    tasks
        .flatten()
        .filter_map(|task| {
            let tid = task.file_name().to_str()?.parse().ok()?;
            let stat = std::fs::read_to_string(task.path().join("stat")).ok()?;
            parse_thread_stat(tid, &stat)
        })
        .collect()
}

/// A thread's name and CPU time from its `stat`: the name is between the first `(` and the last
/// `)`, as it can hold both.
pub fn parse_thread_stat(tid: u32, stat: &str) -> Option<ThreadCpu> {
    let name = stat.get(stat.find('(')? + 1..stat.rfind(')')?)?.to_owned();
    Some(ThreadCpu {
        tid,
        name,
        time: parse_cpu_time(stat)?,
    })
}

/// The threads that used the most CPU between two [`thread_cpu_times`], `elapsed` apart: up to `n`
/// names with their share of a core in percent, busiest first, threads of one name added up (a
/// pool's workers, the GPU driver's), and none under 1%. A thread not there before started since,
/// so all its time counts; one whose time went down is a new thread with an old one's id.
pub fn busiest_threads(before: &[ThreadCpu], now: &[ThreadCpu], elapsed: Duration, n: usize) -> Vec<(String, f64)> {
    if elapsed.is_zero() {
        return Vec::new();
    }
    let mut by_name: Vec<(String, Duration)> = Vec::new();
    for thread in now {
        let earlier = before
            .iter()
            .find(|old| old.tid == thread.tid && old.name == thread.name)
            .map_or(Duration::ZERO, |old| old.time);
        let used = thread.time.checked_sub(earlier).unwrap_or(thread.time);
        match by_name.iter_mut().find(|(name, _)| *name == thread.name) {
            Some((_, total)) => *total += used,
            None => by_name.push((thread.name.clone(), used)),
        }
    }
    let mut shares: Vec<(String, f64)> = by_name
        .into_iter()
        .map(|(name, used)| (name, used.as_secs_f64() * 100.0 / elapsed.as_secs_f64()))
        .filter(|(_, percent)| *percent >= 1.0)
        .collect();
    shares.sort_by(|a, b| b.1.total_cmp(&a.1));
    shares.truncate(n);
    shares
}

/// `Turu Engine 62%, Script Thread 20%` from [`busiest_threads`].
pub fn format_threads(threads: &[(String, f64)]) -> String {
    threads
        .iter()
        .map(|(name, percent)| format!("{name} {percent:.0}%"))
        .collect::<Vec<_>>()
        .join(", ")
}

impl fmt::Display for Reading {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "RSS {} MB (anon {}, file {}, shared {})",
            self.rss / MB,
            self.rss_anon / MB,
            self.rss_file / MB,
            self.rss_shmem / MB
        )?;
        if let Some(pss) = self.pss {
            write!(f, ", PSS {} MB", pss / MB)?;
        }
        if let Some(heap) = self.heap {
            write!(f, ", heap {} MB", heap / MB)?;
        }
        if self.swap > 0 {
            write!(f, ", swap {} MB", self.swap / MB)?;
        }
        write!(f, ", virtual {} MB", self.virtual_size / MB)?;
        if let Some(maps) = self.maps {
            write!(f, ", {maps} maps")?;
        }
        write!(f, ", {} threads", self.threads)
    }
}

/// Turu's own memory budget: RSS above the larger of [`MIN_RSS_BUDGET`] and
/// [`RSS_BUDGET_PERCENT`] of the phone's memory, or more mappings than [`MAX_MAPS`]. A Pixel 8
/// aborted on a failed allocation at 1.9 GB of RSS (25% of its 7.5 GB) with 3 GB of the phone still
/// free, so a check of the phone's free memory alone never fired; a farming client is ~500 MB.
pub const MIN_RSS_BUDGET: u64 = 1024 * MB;
pub const RSS_BUDGET_PERCENT: u64 = 20;
/// Half the kernel's usual `vm.max_map_count` (65530), which apps can't read on Android.
pub const MAX_MAPS: u64 = 30_000;

/// Why this reading is over Turu's own memory budget (see [`MIN_RSS_BUDGET`]), given the phone's
/// memory in bytes; `None` within it.
pub fn over_budget(reading: &Reading, phone_memory: u64) -> Option<String> {
    let budget = MIN_RSS_BUDGET.max(phone_memory / 100 * RSS_BUDGET_PERCENT);
    if reading.rss > budget {
        return Some(format!(
            "RSS {} MB is over Turu's {} MB ({RSS_BUDGET_PERCENT}% of the phone's {} MB, at least {})",
            reading.rss / MB,
            budget / MB,
            phone_memory / MB,
            MIN_RSS_BUDGET / MB
        ));
    }
    match reading.maps {
        Some(maps) if maps > MAX_MAPS => Some(format!("{maps} memory maps, over {MAX_MAPS}")),
        _ => None,
    }
}

/// The kernel's limit on a process's mappings (`vm.max_map_count`), when it can be read.
pub fn max_map_count() -> Option<u64> {
    std::fs::read_to_string("/proc/sys/vm/max_map_count")
        .ok()?
        .trim()
        .parse()
        .ok()
}

/// The `n` names with the most address space in `/proc/self/maps`, with their size in bytes and
/// number of mappings (e.g. `[anon:scudo:primary]`, a GPU driver's device file).
pub fn largest_mappings(n: usize) -> Vec<(String, u64, u64)> {
    match std::fs::File::open("/proc/self/maps") {
        Ok(file) => group_mappings(BufReader::new(file), n),
        Err(_) => Vec::new(),
    }
}

/// `/proc/<pid>/status`' sizes; `None` without `VmRSS`.
pub fn parse_status(text: &str) -> Option<Reading> {
    let kb = |name| field_kb(text, name).unwrap_or(0);
    Some(Reading {
        rss: field_kb(text, "VmRSS:")?,
        rss_anon: kb("RssAnon:"),
        rss_file: kb("RssFile:"),
        rss_shmem: kb("RssShmem:"),
        virtual_size: kb("VmSize:"),
        swap: kb("VmSwap:"),
        threads: text
            .lines()
            .find_map(|line| line.strip_prefix("Threads:"))
            .and_then(|value| value.trim().parse().ok())
            .unwrap_or(0),
        ..Default::default()
    })
}

/// A `Name:   1234 kB` line's value, in bytes.
fn field_kb(text: &str, name: &str) -> Option<u64> {
    let value = text.lines().find_map(|line| line.strip_prefix(name))?;
    let kb: u64 = value.trim().trim_end_matches("kB").trim().parse().ok()?;
    Some(kb * 1024)
}

/// Groups `maps` lines by name (anonymous ones without a name as `[anon]`), largest first.
pub fn group_mappings(maps: impl BufRead, n: usize) -> Vec<(String, u64, u64)> {
    let mut groups: std::collections::HashMap<String, (u64, u64)> = std::collections::HashMap::new();
    for line in maps.lines().map_while(Result::ok) {
        // start-end perms offset dev inode [name]
        let mut fields = line.splitn(6, ' ');
        let Some((start, end)) = fields.next().and_then(|range| range.split_once('-')) else {
            continue;
        };
        let (Ok(start), Ok(end)) = (u64::from_str_radix(start, 16), u64::from_str_radix(end, 16)) else {
            continue;
        };
        let name = fields.nth(4).map(str::trim).filter(|name| !name.is_empty()).unwrap_or("[anon]");
        let group = groups.entry(name.to_owned()).or_default();
        group.0 += end.saturating_sub(start);
        group.1 += 1;
    }
    let mut groups: Vec<_> = groups.into_iter().map(|(name, (size, count))| (name, size, count)).collect();
    groups.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    groups.truncate(n);
    groups
}

/// `name 512 MB in 3 maps, ...`.
pub fn format_mappings(groups: &[(String, u64, u64)]) -> String {
    groups
        .iter()
        .map(|(name, size, count)| format!("{name} {} MB in {count} maps", size / MB))
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_sizes_from_status() {
        let status = "Name:\tmuktiarafi.turu\nVmSize:\t12582912 kB\nVmRSS:\t 1940480 kB\nRssAnon:\t 1740800 kB\n\
            RssFile:\t  153600 kB\nRssShmem:\t   46080 kB\nVmSwap:\t       0 kB\nThreads:\t58\n";
        let reading = parse_status(status).unwrap();
        assert_eq!(reading.rss, 1940480 * 1024);
        assert_eq!(reading.rss_anon, 1740800 * 1024);
        assert_eq!(reading.threads, 58);
        assert_eq!(
            reading.to_string(),
            "RSS 1895 MB (anon 1700, file 150, shared 45), virtual 12288 MB, 58 threads"
        );
        assert_eq!(parse_status("Name:\tx\n"), None);
    }

    #[test]
    fn says_pss_heap_swap_and_maps_when_known() {
        let reading = Reading {
            rss: 100 * MB,
            pss: Some(120 * MB),
            heap: Some(80 * MB),
            swap: 5 * MB,
            maps: Some(4000),
            threads: 3,
            ..Default::default()
        };
        assert_eq!(
            reading.to_string(),
            "RSS 100 MB (anon 0, file 0, shared 0), PSS 120 MB, heap 80 MB, swap 5 MB, virtual 0 MB, 4000 maps, 3 threads"
        );
    }

    #[test]
    fn reads_cpu_time_after_the_name() {
        // A name with a space and a parenthesis; utime 1234, stime 566 ticks.
        let stat = "4043 (Turu (x) y) S 1 4043 0 0 -1 4194624 9000 0 0 0 1234 566 0 0 20 0 60 0 100 0";
        assert_eq!(parse_cpu_time(stat), Some(Duration::from_secs(18)));
        assert_eq!(parse_cpu_time("4043 (turu) S 1"), None);
    }

    #[test]
    fn gives_the_share_of_a_core_between_two_readings() {
        let at = |secs| Reading { cpu_time: Some(Duration::from_secs(secs)), ..Default::default() };
        let percent = cpu_percent(&at(10), &at(19), Duration::from_secs(60)).unwrap();
        assert!((percent - 15.0).abs() < 1e-9);
        assert_eq!(cpu_percent(&Reading::default(), &at(19), Duration::from_secs(60)), None);
        assert_eq!(cpu_percent(&at(10), &at(19), Duration::ZERO), None);
    }

    #[test]
    fn reads_a_threads_name_and_time() {
        let stat = "4051 (Script (x) Thread) S 1 4043 0 0 -1 4194624 9000 0 0 0 300 100 0 0 20 0 60 0 100 0";
        let thread = parse_thread_stat(4051, stat).unwrap();
        assert_eq!(thread.name, "Script (x) Thread");
        assert_eq!(thread.time, Duration::from_secs(4));
        assert_eq!(parse_thread_stat(1, "4051 (x S 1"), None);
    }

    #[test]
    fn names_the_busiest_threads_between_two_readings() {
        let thread = |tid, name: &str, secs| ThreadCpu { tid, name: name.into(), time: Duration::from_secs(secs) };
        let before = [
            thread(1, "Turu Engine", 100),
            thread(2, "Script Thread", 10),
            thread(3, "pool", 5),
            thread(4, "pool", 5),
            thread(5, "idle", 50),
            thread(9, "gone", 70),
        ];
        let now = [
            thread(1, "Turu Engine", 136), // 36 s: 60%
            thread(2, "Script Thread", 22), // 12 s: 20%
            thread(3, "pool", 8),          // 3 s and
            thread(4, "pool", 6),          // 1 s: 4 s, 6.7%
            thread(5, "idle", 50),         // nothing
            thread(6, "new", 3),           // started since: 3 s, 5%
            thread(9, "reused", 2),        // a new thread on an old id: 2 s, 3.3%
        ];
        let busiest = busiest_threads(&before, &now, Duration::from_secs(60), 4);
        let names: Vec<_> = busiest.iter().map(|(name, _)| name.as_str()).collect();
        assert_eq!(names, ["Turu Engine", "Script Thread", "pool", "new"]);
        assert_eq!(format_threads(&busiest), "Turu Engine 60%, Script Thread 20%, pool 7%, new 5%");
        assert!(busiest_threads(&before, &now, Duration::ZERO, 4).is_empty());
    }

    #[test]
    fn says_when_turu_is_over_its_own_budget() {
        let with = |rss_mb: u64, maps| Reading { rss: rss_mb * MB, maps, ..Default::default() };
        // A Pixel 8 (7572 MB): 20% is 1514 MB.
        assert_eq!(
            over_budget(&with(1895, Some(9000)), 7572 * MB).as_deref(),
            Some("RSS 1895 MB is over Turu's 1514 MB (20% of the phone's 7572 MB, at least 1024)")
        );
        assert_eq!(over_budget(&with(500, Some(9000)), 7572 * MB), None);
        // A 3 GB phone: the 1 GB floor.
        assert_eq!(over_budget(&with(900, None), 3072 * MB), None);
        assert!(over_budget(&with(1100, None), 3072 * MB).is_some());
        assert_eq!(
            over_budget(&with(500, Some(31_000)), 7572 * MB).as_deref(),
            Some("31000 memory maps, over 30000")
        );
    }

    #[test]
    fn groups_mappings_by_name_largest_first() {
        let maps = "\
7000000000-7010000000 rw-p 00000000 00:00 0                              [anon:scudo:primary]
7010000000-7020000000 rw-p 00000000 00:00 0                              [anon:scudo:primary]
7100000000-7100100000 rw-s 00000000 00:05 1234                           /dev/mali0
7200000000-7200001000 rw-p 00000000 00:00 0
7200001000-7200003000 r-xp 00000000 fd:00 99                             /system/lib64/libc.so
not a mapping
";
        let groups = group_mappings(maps.as_bytes(), 3);
        assert_eq!(
            groups,
            [
                ("[anon:scudo:primary]".to_owned(), 0x2000_0000, 2),
                ("/dev/mali0".to_owned(), 0x10_0000, 1),
                ("/system/lib64/libc.so".to_owned(), 0x2000, 1),
            ]
        );
        assert_eq!(
            format_mappings(&groups[..2]),
            "[anon:scudo:primary] 512 MB in 2 maps, /dev/mali0 1 MB in 1 maps"
        );
        assert_eq!(group_mappings("7200000000-7200001000 rw-p 00000000 00:00 0\n".as_bytes(), 5)[0].0, "[anon]");
    }
}
