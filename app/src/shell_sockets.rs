//! Turu: the process tabs' shell's Unix sockets for shared surfaces on Linux
//! (`shared_gpu::listen`, docs/TAB_PROCESSES_PROPOSAL.md, M5): their names, and removing those
//! of shells that are gone. Also built for tests on any Unix, to run them on a Mac.

use std::path::{Path, PathBuf};

/// The shell's socket's name: `turu-shell-<pid>.sock`.
const SOCKET_PREFIX: &str = "turu-shell-";
const SOCKET_SUFFIX: &str = ".sock";

/// Removes the sockets in `dir` of shells that are gone (all but `own`'s): nothing accepts a
/// connection there (a running shell's listener does; its process may have ended, or its pid be
/// another process's now). A shell that quits removes its own (`shell::run`); one that crashed or
/// was killed can't. A running shell sees the connection end at once, with no surface on it.
pub fn remove_stale(dir: &Path, own: u32) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(pid) = name
            .to_str()
            .and_then(|name| name.strip_prefix(SOCKET_PREFIX))
            .and_then(|rest| rest.strip_suffix(SOCKET_SUFFIX))
            .and_then(|pid| pid.parse::<u32>().ok())
        else {
            continue;
        };
        if pid == own {
            continue;
        }
        let path = entry.path();
        if std::os::unix::net::UnixStream::connect(&path).is_err() && std::fs::remove_file(&path).is_ok() {
            tracing::info!("Shell: removed {}, left by a shell that's gone", path.display());
        }
    }
}

/// Shell `pid`'s socket in `dir`.
pub fn path(dir: &Path, pid: u32) -> PathBuf {
    dir.join(format!("{SOCKET_PREFIX}{pid}{SOCKET_SUFFIX}"))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixListener;

    #[test]
    fn removes_only_gone_shells_sockets() {
        let dir = std::env::temp_dir().join(format!("turu-sockets-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        // A shell that's gone: its process ended.
        let mut child = std::process::Command::new("true").spawn().unwrap();
        let dead = child.id();
        child.wait().unwrap();
        drop(UnixListener::bind(path(&dir, dead)).unwrap());
        // A pid that's alive but isn't a shell (reused by another process): nothing listens.
        let reused = std::process::id();
        drop(UnixListener::bind(path(&dir, reused)).unwrap());
        // A shell that's running: alive and listening.
        let running = std::os::unix::process::parent_id();
        let _listener = UnixListener::bind(path(&dir, running)).unwrap();
        // This shell's own, and a file that isn't a shell's.
        let own = 1;
        std::fs::write(path(&dir, own), "").unwrap();
        std::fs::write(dir.join("turu-shell-x.sock"), "").unwrap();

        remove_stale(&dir, own);

        assert!(!path(&dir, dead).exists());
        assert!(!path(&dir, reused).exists());
        assert!(path(&dir, running).exists());
        assert!(path(&dir, own).exists());
        assert!(dir.join("turu-shell-x.sock").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
