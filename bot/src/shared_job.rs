//! Jobs shared by the Turu processes on one data folder (docs/TAB_PROCESSES_PROPOSAL.md, M3): each
//! tab may be a process of its own (a desktop shell's clients, Android's slots), and the app's
//! once-only jobs mustn't run in all of them, nor stop when one of them goes.
//!
//! * [`run_once`]: a periodic job (script updates, skill sets) done by whichever process comes
//!   first, once per interval: under a file lock, with the time it was last done in a file beside it.
//! * [`Held`]: a lock a process keeps while it runs a single-instance service (the web monitor);
//!   the system lets it go when the process ends, and another takes it ([`try_hold`]).
//!
//! Locks are `File::try_lock` (an advisory lock on an open file, per open file: two opens in one
//! process exclude each other too), in the data folder's `jobs/`.

use std::fs::{File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// The folder of the jobs' locks and times, in the data folder.
pub fn dir(root: &Path) -> PathBuf {
    root.join("jobs")
}

/// A lock this process holds until it's dropped (or the process ends).
#[derive(Debug)]
pub struct Held {
    _file: File,
}

/// Takes `name`'s lock if no process holds it.
pub fn try_hold(dir: &Path, name: &str) -> std::io::Result<Option<Held>> {
    std::fs::create_dir_all(dir)?;
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(dir.join(format!("{name}.lock")))?;
    match file.try_lock() {
        Ok(()) => Ok(Some(Held { _file: file })),
        Err(TryLockError::WouldBlock) => Ok(None),
        Err(TryLockError::Error(e)) => Err(e),
    }
}

/// Takes `name`'s lock, waiting up to `wait` while another process holds it; `None` if it still
/// does then.
pub fn hold_waiting(dir: &Path, name: &str, wait: Duration) -> std::io::Result<Option<Held>> {
    let since = std::time::Instant::now();
    loop {
        if let Some(held) = try_hold(dir, name)? {
            return Ok(Some(held));
        }
        if since.elapsed() >= wait {
            return Ok(None);
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

/// When `name` was last done (ms since 1970), as its file says.
fn last_done(dir: &Path, name: &str) -> Option<u64> {
    std::fs::read_to_string(dir.join(format!("{name}.done")))
        .ok()?
        .trim()
        .parse()
        .ok()
}

/// What [`run_once`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ran {
    /// It ran the job here.
    Here,
    /// Another process is running it now.
    Busy,
    /// It was done less than the interval ago.
    Recent,
}

/// Runs `job` unless another process is running it or it was done less than `every` ago; the
/// time it's done is kept for the others. Blocks while `job` runs.
pub fn run_once(
    dir: &Path,
    name: &str,
    every: Duration,
    job: impl FnOnce(),
) -> std::io::Result<Ran> {
    let Some(_held) = try_hold(dir, name)? else {
        return Ok(Ran::Busy);
    };
    let every = every.as_millis() as u64;
    if last_done(dir, name).is_some_and(|at| now_ms().saturating_sub(at) < every) {
        return Ok(Ran::Recent);
    }
    job();
    std::fs::write(dir.join(format!("{name}.done")), now_ms().to_string())?;
    Ok(Ran::Here)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn a_job_runs_once_per_interval() {
        let dir = tempfile::tempdir().unwrap();
        let runs = AtomicUsize::new(0);
        let job = || {
            runs.fetch_add(1, Ordering::SeqCst);
        };
        let hour = Duration::from_secs(3600);
        assert_eq!(
            run_once(dir.path(), "skill-sets", hour, job).unwrap(),
            Ran::Here
        );
        assert_eq!(
            run_once(dir.path(), "skill-sets", hour, job).unwrap(),
            Ran::Recent
        );
        // Another job is apart; an interval past runs it again.
        assert_eq!(
            run_once(dir.path(), "scripts", hour, job).unwrap(),
            Ran::Here
        );
        assert_eq!(
            run_once(dir.path(), "skill-sets", Duration::ZERO, job).unwrap(),
            Ran::Here
        );
        assert_eq!(runs.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn a_job_running_elsewhere_is_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let runs = Arc::new(AtomicUsize::new(0));
        let (started, release) = (
            Arc::new(std::sync::Barrier::new(2)),
            Arc::new(std::sync::Barrier::new(2)),
        );
        let worker = {
            let (dir, runs, started, release) = (
                dir.path().to_owned(),
                runs.clone(),
                started.clone(),
                release.clone(),
            );
            std::thread::spawn(move || {
                run_once(&dir, "scripts", Duration::ZERO, || {
                    runs.fetch_add(1, Ordering::SeqCst);
                    started.wait();
                    release.wait();
                })
                .unwrap()
            })
        };
        started.wait();
        let skipped = run_once(dir.path(), "scripts", Duration::ZERO, || {
            runs.fetch_add(1, Ordering::SeqCst);
        });
        release.wait();
        assert_eq!(skipped.unwrap(), Ran::Busy);
        assert_eq!(worker.join().unwrap(), Ran::Here);
        assert_eq!(runs.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn a_held_lock_passes_on_when_let_go() {
        let dir = tempfile::tempdir().unwrap();
        let held = try_hold(dir.path(), "monitor").unwrap();
        assert!(held.is_some());
        assert!(try_hold(dir.path(), "monitor").unwrap().is_none());
        drop(held);
        assert!(try_hold(dir.path(), "monitor").unwrap().is_some());
    }

    #[test]
    fn a_waiting_hold_gets_the_lock_once_let_go_or_gives_up() {
        let dir = tempfile::tempdir().unwrap();
        let held = try_hold(dir.path(), "script-update").unwrap().unwrap();
        let short = Duration::from_millis(300);
        assert!(
            hold_waiting(dir.path(), "script-update", short)
                .unwrap()
                .is_none()
        );
        let letting_go = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(300));
            drop(held);
        });
        let waited = hold_waiting(dir.path(), "script-update", Duration::from_secs(10)).unwrap();
        assert!(waited.is_some());
        letting_go.join().unwrap();
    }
}
