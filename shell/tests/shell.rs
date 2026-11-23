//! The shell with real client processes (`turu-shell-fake-client`: a hello and a status, no
//! player) over loopback, and no window: two tabs, Grid View, a client killed and back.

use std::path::PathBuf;
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};
use turu_frontend::shell::ToShell;
use turu_shell::{Config, Event, NoSurface, Shell, TabState};

struct Harness {
    shell: Shell<NoSurface>,
    events: mpsc::Receiver<Event<NoSurface>>,
}

impl Harness {
    fn new() -> Self {
        // The Army and the updates the shell reads, in a folder of the test's own.
        let data = std::env::temp_dir().join(format!("turu-shell-test-{}", std::process::id()));
        // SAFETY: set before the shell starts any thread; each test runs in its own process
        // (nextest) or sets the same value.
        unsafe { std::env::set_var("TURU_DATA_DIR", &data) };
        let (sender, events) = mpsc::channel();
        let sender = std::sync::Mutex::new(sender);
        let config = Config {
            exe: PathBuf::from(env!("CARGO_BIN_EXE_turu-shell-fake-client")),
            surfaces_name: None,
            name: Box::new(|| "Turu".to_owned()),
        };
        let shell = Shell::new(
            config,
            Arc::new(move |event| sender.lock().unwrap_or_else(|e| e.into_inner()).send(event).is_ok()),
        )
        .expect("listens on loopback");
        Self { shell, events }
    }

    /// Hands the shell its events until `done` holds of its tabs (10 s at most).
    fn until(&mut self, what: &str, done: impl Fn(&[TabState]) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while !done(&self.shell.tabs()) {
            let left = deadline.saturating_duration_since(Instant::now());
            assert!(!left.is_zero(), "{what}: {:?}", self.shell.tabs());
            if let Ok(event) = self.events.recv_timeout(left.min(Duration::from_millis(100))) {
                self.shell.handle(event);
            }
            self.shell.tick(Instant::now());
        }
    }
}

fn connected(tab: &TabState) -> bool {
    tab.down.is_none() && tab.pid.is_some_and(|pid| tab.label == format!("Fake {pid}"))
}

#[test]
fn two_tabs_grid_view_and_a_client_killed_and_back() {
    let mut harness = Harness::new();
    harness.shell.open_tab(Vec::new(), None);
    harness
        .shell
        .open_tab(vec!["--user".into(), "Questwalker".into()], None);
    harness.until("both clients connected", |tabs| {
        tabs.len() == 2 && tabs.iter().all(connected)
    });
    assert_eq!(harness.shell.shown(), 2, "a new tab is shown");

    // Grid View, from a client's bar: a tile each under the bar.
    harness.shell.resized(1600, 1000, 2.0, false);
    harness.shell.handle(Event::Message(
        2,
        ToShell::Grid {
            on: true,
            left: 0.0,
            top: 30.0,
        },
    ));
    let tiles = harness.shell.grid_tiles().expect("Grid View with two tabs");
    assert_eq!(tiles.iter().map(|tile| tile.id).collect::<Vec<_>>(), [1, 2]);
    assert!(tiles.iter().all(|tile| tile.y >= 60));
    harness.shell.handle(Event::Message(
        2,
        ToShell::Grid {
            on: false,
            left: 0.0,
            top: 30.0,
        },
    ));
    assert_eq!(harness.shell.grid_tiles(), None);

    // The tab on screen's client dies: the other is shown, and the dead one starts again after a
    // second, as a new process.
    let old = harness.shell.tabs()[1].pid;
    harness.shell.kill(2);
    harness.until("tab 2 down", |tabs| tabs[1].down.as_deref() == Some("restarting"));
    assert_eq!(harness.shell.shown(), 1, "a running tab is shown instead");
    harness.until("tab 2 back", |tabs| connected(&tabs[1]) && tabs[1].pid != old);
    assert!(connected(&harness.shell.tabs()[0]), "tab 1 kept running");

    // Selected again from the strip, once it's back.
    harness.shell.select_requested(2);
    assert_eq!(harness.shell.shown(), 2);

    // Closing a tab leaves the other; the last one is never closed.
    harness.shell.close(1);
    assert_eq!(harness.shell.tabs().iter().map(|tab| tab.id).collect::<Vec<_>>(), [2]);
    harness.shell.close(2);
    assert_eq!(harness.shell.tabs().len(), 1);
    harness.shell.exit();
    assert!(
        harness.shell.tabs().iter().all(|tab| tab.pid.is_none()),
        "every client ended"
    );
}
