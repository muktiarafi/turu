//! Turu: a tab's side of its link to a shell (docs/TAB_PROCESSES_PROPOSAL.md, M5 and M3), shared
//! by the desktop's clients (`app/src/client.rs`, over a socket) and Android's slots (over Binder).
//! The tab strip shows the shell's tabs: its "+", Army > Accounts' "In a tab", selecting and
//! closing a tab and the bar's Grid toggle become messages to the shell; the shell's tab list
//! comes back to the strip, Army Control and the monitor hub; the tab's name and whether it's busy
//! go to the shell when they change.

use crate::shell::{TabInfo, ToShell};
use crate::turu;
use std::sync::Arc;
use std::time::Duration;

/// Sends a message to the shell (dropped while there's none).
pub type Sender = Arc<dyn Fn(ToShell) + Send + Sync>;

/// Routes this process's tab requests to the shell through `send`, and reports the tab's name and
/// whether it's busy every second while `open` says the link is there.
pub fn install(send: Sender, open: impl Fn() -> bool + Send + 'static) {
    let tabs = send.clone();
    turu::set_tab_requests(Box::new(move |request| {
        match request {
            turu::TabRequest::Open(startup) => tabs(ToShell::OpenTab {
                username: startup.username,
                password: startup.password,
                server: startup.server,
                script: startup
                    .script
                    .map(|path| path.to_string_lossy().into_owned()),
                run_script: startup.run_script,
            }),
            // The shell lays the tiles out in what Turu's UI leaves of the window.
            turu::TabRequest::Grid(on) => tabs(ToShell::Grid {
                on,
                left: crate::gui::left_width() as f32,
                top: crate::gui::top_height() as f32,
            }),
            _ => {}
        }
        true
    }));
    let actions = send.clone();
    turu::set_shell_actions(Box::new(move |action| {
        actions(match action {
            turu::ShellTabAction::Select(id) => ToShell::SelectTab { id },
            turu::ShellTabAction::Close(id) => ToShell::CloseTab { id },
        });
    }));
    report_status(send, open);
}

/// The shell's tab list, from its `Tabs` message: `you` is this tab.
pub fn set_tabs(tabs: Vec<TabInfo>, shown: u32, you: Option<u32>) {
    let tabs = tabs
        .into_iter()
        .map(|tab| turu::ShellTab {
            me: you == Some(tab.id),
            id: tab.id,
            label: tab.label,
            busy: tab.busy,
            down: tab.down,
            api: tab.api.and_then(|api| api.parse().ok()),
            token: tab.token.unwrap_or_default(),
        })
        .collect();
    turu::set_shell_tabs(tabs, shown);
}

/// The tab's name and whether its script or Auto runs, to the shell when either changes (checked
/// every second), and again after [`resend_status`] (a new shell).
fn report_status(send: Sender, open: impl Fn() -> bool + Send + 'static) {
    let spawned = std::thread::Builder::new()
        .name("Turu Shell Status".into())
        .spawn(move || {
            let mut sent = None;
            while open() {
                let bot = turu::bot();
                let now = (turu::tab_label(&bot), crate::gui::turu::busy(&bot));
                let resend = std::mem::take(&mut *RESEND.lock().unwrap_or_else(|e| e.into_inner()));
                if resend || sent.as_ref() != Some(&now) {
                    send(ToShell::Status {
                        label: now.0.clone(),
                        busy: now.1,
                    });
                    sent = Some(now);
                }
                std::thread::sleep(Duration::from_secs(1));
            }
        });
    if let Err(e) = spawned {
        tracing::warn!("Couldn't start reporting the tab's name to the shell: {e}");
    }
}

static RESEND: std::sync::Mutex<bool> = std::sync::Mutex::new(false);

/// A new shell is listening: the next status report goes even if nothing changed.
pub fn resend_status() {
    *RESEND.lock().unwrap_or_else(|e| e.into_inner()) = true;
}
