//! Keeps a line that keeps coming from flooding a log: the host SWF's errors (`flash.rs`).

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// How often the same line is logged.
pub(crate) const REPEAT: Duration = Duration::from_secs(60);
/// Lines remembered at most; the oldest are forgotten past it.
const KEPT: usize = 256;

/// When each line was last logged, and how often it came since.
#[derive(Default)]
pub(crate) struct RepeatLog {
    seen: HashMap<String, (Instant, u32)>,
}

impl RepeatLog {
    /// `Some(n)` when `key` should be logged now, `n` being the times it came since it last
    /// was; `None` while it was logged less than [`REPEAT`] ago.
    pub(crate) fn note(&mut self, key: &str, now: Instant) -> Option<u32> {
        if let Some((logged, since)) = self.seen.get_mut(key) {
            if now.duration_since(*logged) < REPEAT {
                *since += 1;
                return None;
            }
            let repeats = *since;
            *logged = now;
            *since = 0;
            return Some(repeats);
        }
        if self.seen.len() >= KEPT {
            self.seen
                .retain(|_, (logged, _)| now.duration_since(*logged) < REPEAT);
            if self.seen.len() >= KEPT {
                self.seen.clear();
            }
        }
        self.seen.insert(key.to_owned(), (now, 0));
        Some(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_is_logged_again_after_a_minute_with_its_count() {
        let mut log = RepeatLog::default();
        let start = Instant::now();
        assert_eq!(log.note("a", start), Some(0));
        assert_eq!(log.note("a", start + Duration::from_secs(1)), None);
        assert_eq!(log.note("a", start + Duration::from_secs(30)), None);
        assert_eq!(log.note("b", start + Duration::from_secs(30)), Some(0));
        assert_eq!(log.note("a", start + Duration::from_secs(61)), Some(2));
        assert_eq!(log.note("a", start + Duration::from_secs(62)), None);
    }

    #[test]
    fn old_lines_are_forgotten_past_the_limit() {
        let mut log = RepeatLog::default();
        let start = Instant::now();
        for i in 0..KEPT {
            log.note(&i.to_string(), start);
        }
        let later = start + REPEAT;
        assert_eq!(log.note("new", later), Some(0));
        assert_eq!(log.seen.len(), 1);
    }
}
