//! AQW build (diagnostic): what the process's committed memory is made of, for the perf
//! log's memory line.
//!
//! "process commit" is everything the system has promised the process, and "rust heap" is what
//! the player's own code has allocated and not freed. In between sit: what the allocator
//! (mimalloc) holds beyond the live bytes (a freed block stays in a page of the thread that
//! made it, and blocks made on one thread and freed on another spread the same bytes over
//! more pages), the Windows heaps (what the graphics driver and the system libraries
//! allocate), and what is mapped for the graphics chip. In one session of the real game with
//! the render thread and uncapped pacing the commit reached 7.5 GB with 0.6 GB of heap and
//! 1.9 GB of video memory, and the offline rooms do not show it: these numbers say which of
//! the three it is.
//!
//! Turu: ported from Mugetsu 2.0.0 without its `heap-collect` test switch (asking the allocator
//! to give back what it can after every memory line): Turu already does that once a minute
//! (`release_free_memory`). Logged by the window's memory line and by `--headless`'s and the
//! tabs' clients' (`headless::log_memory`).

/// One named count of mimalloc's own statistics (`"name": { "total": .., "peak": ..,
/// "current": .. }`, one per line in its JSON): the current and the peak value.
fn count(json: &str, name: &str) -> Option<(u64, u64)> {
    let key = format!("\"{name}\":");
    let line = json
        .lines()
        .find(|line| line.trim_start().starts_with(&key))?;
    let field = |field: &str| -> Option<u64> {
        let at = line.find(field)? + field.len();
        let digits: String = line[at..]
            .trim_start()
            .chars()
            .take_while(|c| c.is_ascii_digit() || *c == '-')
            .collect();
        // A count can read below zero for a moment (blocks freed by another thread than the
        // one that counted them): that is none.
        digits.parse::<i64>().ok().map(|value| value.max(0) as u64)
    };
    Some((field("\"current\":")?, field("\"peak\":")?))
}

/// Whether the program's allocations go through mimalloc. Turu: not with the `tracy` feature
/// or `TURU_SYSTEM_ALLOC` (see `CountingAlloc`), where its statistics would count nothing.
fn mimalloc_in_use() -> bool {
    #[cfg(feature = "tracy")]
    {
        false
    }
    #[cfg(not(feature = "tracy"))]
    {
        !crate::CountingAlloc::system()
    }
}

/// mimalloc's statistics over all threads, as its JSON text.
fn allocator_json() -> Option<String> {
    if !mimalloc_in_use() {
        return None;
    }
    // SAFETY: with no buffer given mimalloc allocates the text itself, zero-terminated; it
    // is copied and freed here.
    unsafe {
        let text = libmimalloc_sys::mi_stats_get_json(0, std::ptr::null_mut());
        if text.is_null() {
            return None;
        }
        let json = std::ffi::CStr::from_ptr(text)
            .to_string_lossy()
            .into_owned();
        libmimalloc_sys::mi_free(text.cast());
        Some(json)
    }
}

/// After "Free memory now", what the allocator holds right after, in full (diagnostic). Turu:
/// only with `TURU_PERF_LOG`: the line is a few kilobytes, and every tab's client logs one.
pub fn log_after_free() {
    if crate::turu::perf_log_every().is_some() {
        tracing::info!(
            target: "ruffle_aqw_perf",
            "allocator statistics after it: {}",
            allocator_json_line()
        );
    }
}

/// mimalloc's statistics as one line, for the log (after "Free memory now": every count it
/// keeps, to see where what it holds beyond the live bytes sits: its bins of block sizes,
/// its pages, what it has purged).
fn allocator_json_line() -> String {
    allocator_json()
        .map(|json| json.split_whitespace().collect::<Vec<_>>().join(" "))
        .unwrap_or_else(|| "none".to_string())
}

/// Committed bytes of the process's Windows heaps, and how many heaps there are: the player's
/// own allocations are not in them (they are mimalloc's), so this is the graphics driver's
/// and the system libraries' share.
#[cfg(windows)]
fn windows_heaps() -> Option<(u64, usize)> {
    use windows_sys::Win32::System::Memory::{GetProcessHeaps, HEAP_SUMMARY, HeapSummary};
    let mut heaps = [std::ptr::null_mut(); 256];
    // SAFETY: the buffer's length is passed; a summary reads a heap's own totals under the
    // heap's lock.
    unsafe {
        let found = GetProcessHeaps(heaps.len() as u32, heaps.as_mut_ptr()) as usize;
        if found == 0 || found > heaps.len() {
            return None;
        }
        let mut committed = 0u64;
        for &heap in &heaps[..found] {
            let mut summary: HEAP_SUMMARY = std::mem::zeroed();
            summary.cb = std::mem::size_of::<HEAP_SUMMARY>() as u32;
            if HeapSummary(heap, 0, &mut summary) != 0 {
                committed += summary.cbCommitted as u64;
            }
        }
        Some((committed, found))
    }
}

#[cfg(not(windows))]
fn windows_heaps() -> Option<(u64, usize)> {
    None
}

/// The memory line's part about the allocator and the Windows heaps.
pub fn report() -> String {
    const MB: u64 = 1 << 20;
    let mut parts = Vec::new();
    if let Some(json) = allocator_json() {
        let current = |name: &str| count(&json, name).map_or(0, |(current, _)| current);
        let (committed, peak) = count(&json, "committed").unwrap_or_default();
        parts.push(format!(
            "allocator: {} MB committed (peak {}) in {} pages ({} abandoned), {} MB reserved",
            committed / MB,
            peak / MB,
            current("pages"),
            current("pages_abandoned"),
            current("reserved") / MB,
        ));
    }
    if let Some((committed, heaps)) = windows_heaps() {
        parts.push(format!(
            "windows heaps: {} MB committed in {heaps}",
            committed / MB
        ));
    }
    if parts.is_empty() {
        return "allocator: no statistics".to_string();
    }
    parts.join("; ")
}

#[cfg(test)]
mod tests {
    #[test]
    fn counts_are_read_from_the_allocators_text() {
        let json = "{\n  \"stat_version\": 2,\n  \"pages\": { \"total\": 90, \"peak\": 40, \"current\": 31 },\n  \"committed\": { \"total\": 900, \"peak\": 800, \"current\": 700 },\n  \"page_committed\": { \"total\": 5, \"peak\": 4, \"current\": -3 },\n  \"pages_abandoned\": { \"total\": 9, \"peak\": 8, \"current\": 7 },\n}\n";
        assert_eq!(super::count(json, "pages"), Some((31, 40)));
        assert_eq!(super::count(json, "committed"), Some((700, 800)));
        assert_eq!(super::count(json, "page_committed"), Some((0, 4)));
        assert_eq!(super::count(json, "pages_abandoned"), Some((7, 8)));
        assert_eq!(super::count(json, "reserved"), None);
    }

    #[test]
    #[cfg(not(feature = "tracy"))]
    fn the_allocator_answers() {
        // The text has to be there and to hold the counts the memory line reads.
        let json = super::allocator_json().expect("mimalloc gives its statistics");
        assert!(super::count(&json, "committed").is_some(), "{json}");
        assert!(super::count(&json, "pages").is_some(), "{json}");
        assert!(super::count(&json, "pages_abandoned").is_some(), "{json}");
    }
}
