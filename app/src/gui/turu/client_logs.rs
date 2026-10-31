//! Army Control > ⋯ > Logs: a launched client's script and debug logs, read from its control
//! API (`/log?since=`) every two seconds while the window is open.

use super::army::Peer;
use super::files::SaveText;
use super::logs::log_buttons;
use super::theme::{DANGER, MUTED};
use super::widgets::{self, LogView};
use crate::gui::FilePicker;
use egui::{Context, RichText, Ui};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use turu_bot::log::LogBuffer;

const POLL: Duration = Duration::from_secs(2);
const LINES: usize = 2000;

#[derive(Default, PartialEq, Eq, Clone, Copy)]
enum Tab {
    #[default]
    Script,
    Debug,
}

/// The lines read so far, filled by the poll thread.
struct Shared {
    script: LogBuffer,
    debug: LogBuffer,
    error: Mutex<Option<String>>,
    /// The window is open: the poll thread runs while it is.
    open: AtomicBool,
}

pub struct ClientLogs {
    pub peer: Peer,
    pub open: bool,
    tab: Tab,
    script: LogView,
    debug: LogView,
    save: SaveText,
    shared: Arc<Shared>,
}

impl ClientLogs {
    pub fn new(peer: Peer) -> Self {
        let shared = Arc::new(Shared {
            script: LogBuffer::new("script", LINES),
            debug: LogBuffer::new("debug", LINES),
            error: Mutex::default(),
            open: AtomicBool::new(false),
        });
        Self {
            peer,
            open: true,
            tab: Tab::Script,
            script: LogView::default(),
            debug: LogView::default(),
            save: SaveText::default(),
            shared,
        }
    }

    pub fn show(&mut self, ctx: &Context, picker: &FilePicker) {
        self.save.poll();
        if !self.open {
            self.shared.open.store(false, Ordering::Relaxed);
            return;
        }
        if !self.shared.open.swap(true, Ordering::Relaxed) {
            poll(self.peer.clone(), self.shared.clone());
        }
        let title = format!("Logs: {}", self.peer.label());
        let mut open = self.open;
        super::window(ctx, &title, [560.0, 380.0], 7)
            .open(&mut open)
            .show(ctx, |ui| self.ui(ui, picker));
        self.open = open;
        ctx.request_repaint_after(Duration::from_millis(500));
    }

    fn ui(&mut self, ui: &mut Ui, picker: &FilePicker) {
        let shared = self.shared.clone();
        let (buffer, view, file_name) = match self.tab {
            Tab::Script => (&shared.script, &mut self.script, "script-log.txt"),
            Tab::Debug => (&shared.debug, &mut self.debug, "debug-log.txt"),
        };
        view.refresh(buffer);
        ui.horizontal(|ui| {
            if widgets::chip(ui, "Script", self.tab == Tab::Script) {
                self.tab = Tab::Script;
            }
            if widgets::chip(ui, "Debug", self.tab == Tab::Debug) {
                self.tab = Tab::Debug;
            }
            ui.label(RichText::new(format!("{} lines", buffer.len())).small().color(MUTED));
            if let Some(error) = shared.error.lock().unwrap_or_else(|e| e.into_inner()).as_deref() {
                ui.label(RichText::new(error).small().color(DANGER));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                log_buttons(ui, picker, buffer, &mut self.save, file_name);
            });
        });
        let height = ui.available_height();
        view.show(ui, "client_logs_view", height);
    }
}

/// Reads both logs from where it left off until the window closes.
fn poll(peer: Peer, shared: Arc<Shared>) {
    let spawned = std::thread::Builder::new()
        .name("Turu Client Logs".into())
        .spawn(move || {
            let mut next = [0u64; 2];
            while shared.open.load(Ordering::Relaxed) {
                for (i, (kind, buffer)) in [("script", &shared.script), ("debug", &shared.debug)]
                    .into_iter()
                    .enumerate()
                {
                    match peer.get(&format!("/log?type={kind}&since={}", next[i])) {
                        Ok(reply) => {
                            for line in reply["lines"].as_array().into_iter().flatten() {
                                buffer.push(line["text"].as_str().unwrap_or_default());
                            }
                            next[i] = reply["next"].as_u64().unwrap_or(next[i]);
                            *shared.error.lock().unwrap_or_else(|e| e.into_inner()) = None;
                        }
                        Err(e) => *shared.error.lock().unwrap_or_else(|e| e.into_inner()) = Some(e),
                    }
                }
                std::thread::sleep(POLL);
            }
        });
    if let Err(e) = spawned {
        tracing::error!("Couldn't read the client's logs: {e}");
    }
}
