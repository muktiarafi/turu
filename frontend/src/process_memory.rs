//! Turu: how much memory this process uses, from Linux's `/proc/self` (Android, Linux; `None`
//! elsewhere). The Android app logs a reading every minute and keeps them for its exit reports
//! (`android/src/memory.rs`): a Pixel 8 aborted after 27 minutes in the background, at 1.9 GB of
//! RSS with 3 GB of the phone's memory free, and nothing said what had grown.
//!
//! A [`Reading`] is cheap (two small files and a count of `/proc/self/maps`' lines);
//! [`largest_mappings`] reads all of `maps` and is for every few minutes.

use std::fmt;
use std::io::{BufRead, BufReader};

const MB: u64 = 1 << 20;

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
        Some(reading)
    }
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
