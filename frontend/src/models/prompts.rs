//! What the engine asks of the user over the game, one at a time, for a native shell (the plan's
//! L3, docs/NATIVE_UI_PROPOSAL.md): a script's message box (`bot.showMessageBox`), Butler's "a
//! script started while following", the crash notice, the Tab Limit's notice and the graphics tips.
//! The egui dialogs draw the same ones from here and step aside while a shell reads the view
//! ([`native_shown`]), so each is answered once, natively or in egui.
//!
//! The crash notice's reports and the refused tab are kept here (one per process); the message
//! box and Butler's notice are the tab's bot's, the tips `crate::graphics_tips`'.

use super::tab_state::lock;
use crate::crash_reports::{self, Report};
use crate::graphics_tips::{self, Tip};
use serde::Serialize;
use serde_json::{Value as Json, json};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Once};
use std::time::{Duration, Instant};
use turu_bot::Bot;
use turu_bot::butler::SuspendNotice;
use turu_bot::startup::Startup;

/// How long after a shell last read the view egui still leaves the dialogs to it.
const SHELL_FRESH: Duration = Duration::from_secs(3);
/// The most tabs Options > Application > Tab Limit allows.
pub const TAB_MAX: i32 = 4;

static SHELL_READ: Mutex<Option<Instant>> = Mutex::new(None);

/// A native shell shows the prompts: egui's dialogs step aside.
pub fn native_shown() -> bool {
    lock(&SHELL_READ).is_some_and(|at| at.elapsed() < SHELL_FRESH)
}

// --- the crash notice ------------------------------------------------------------------------

static UNSEEN: Mutex<Vec<Report>> = Mutex::new(Vec::new());
static LOOKED: Once = Once::new();
/// "Show report" was picked: the Logs window opens on it.
static SHOW_REPORT: Mutex<Option<Report>> = Mutex::new(None);

/// The crash reports written since the last notice, the newest first (read once a run).
pub fn unseen_crashes() -> Vec<Report> {
    // A test build never reads (and marks seen) the real data folder's reports.
    LOOKED.call_once(|| {
        if !cfg!(test) {
            *lock(&UNSEEN) = crash_reports::take_unseen();
        }
    });
    lock(&UNSEEN).clone()
}

/// The notice was answered: `show` opens the newest report in the Logs window.
pub fn answer_crashes(show: bool) {
    let newest = std::mem::take(&mut *lock(&UNSEEN)).into_iter().next();
    if show {
        *lock(&SHOW_REPORT) = newest;
    }
}

/// The report the Logs window should open on, once.
pub fn take_report_to_show() -> Option<Report> {
    lock(&SHOW_REPORT).take()
}

/// The notice's heading, text and note.
pub fn crash_texts(reports: &[Report]) -> (String, String, String) {
    // "Turu", or "Turu Mas".
    let app = turu_bot::updates::app_name();
    let more = reports.len().saturating_sub(1);
    let mut text = reports.first().map(|r| r.summary.clone()).unwrap_or_default();
    if more > 0 {
        text.push_str(&format!("\n\nAnd {more} more since the last notice."));
    }
    (
        format!("{app} closed unexpectedly"),
        text,
        format!("Diagnostics > Logs > Crashes has the report. Share it with whoever helps you with {app}."),
    )
}

// --- the Tab Limit ---------------------------------------------------------------------------

/// A tab was refused and the notice isn't answered yet.
static REFUSED: AtomicBool = AtomicBool::new(false);

/// The Tab Limit, where the platform limits tabs.
pub fn tab_limit() -> Option<usize> {
    crate::platform::platform()
        .limits_tabs()
        .then(|| crate::turu::bot().settings.read().tab_limit.clamp(1, TAB_MAX) as usize)
}

/// As many tabs are open as the Tab Limit allows: the shell's tabs, where this process is one of
/// them (an Android slot has only its own), else this process's (Turu Mas).
pub fn tab_limit_reached() -> bool {
    let open = crate::turu::shell_tabs().map_or_else(|| crate::turu::tabs().len(), |(tabs, _)| tabs.len());
    tab_limit().is_some_and(|limit| open >= limit)
}

/// Asks for a new tab, unless the Tab Limit is reached: then the notice shows and it's false.
/// Any thread.
pub fn open_tab(startup: Startup) -> bool {
    if tab_limit_reached() {
        REFUSED.store(true, Ordering::Relaxed);
        return false;
    }
    crate::turu::request_tab(crate::turu::TabRequest::Open(Box::new(startup)))
}

/// The refused tab's notice is waiting: the limit, while there is one.
pub fn tab_refused() -> Option<usize> {
    if !REFUSED.load(Ordering::Relaxed) {
        return None;
    }
    let limit = tab_limit();
    if limit.is_none() {
        REFUSED.store(false, Ordering::Relaxed);
    }
    limit
}

pub fn answer_tab_refused() {
    REFUSED.store(false, Ordering::Relaxed);
}

/// The notice's text and whether Turu's most is reached (no Raise Tab Limit then).
pub fn tab_limit_text(limit: usize) -> (String, bool) {
    let at_most = limit as i32 >= TAB_MAX;
    let text = if at_most {
        format!("{limit} tabs are open, the most Turu runs at once. Close a tab to open another.")
    } else {
        format!(
            "{limit} {} open, as many as your Tab Limit allows. Raise it in Options > Application, or close a tab \
             first.",
            if limit == 1 { "tab is" } else { "tabs are" }
        )
    };
    (text, at_most)
}

// --- the graphics tips -----------------------------------------------------------------------

/// A tip's heading, text and note.
pub fn tip_texts(tip: &Tip) -> (&'static str, String, &'static str) {
    match tip {
        Tip::QualityLow { chip, quality } => (
            "Set Quality to Low for more FPS",
            format!(
                "AQW is running on your built-in graphics ({chip}) with Quality set to {quality}, which is slow on \
                 built-in graphics. Open AQW's Options and change Quality to Low: edges are a little less smooth, \
                 but built-in graphics usually run AQW several times faster."
            ),
            "Turu won't show this again.",
        ),
        Tip::PerformanceMode { chip, .. } => (
            "Turn on Performance mode?",
            format!(
                "AQW is running slower than it should on your built-in graphics ({chip}). Performance mode draws \
                 the game with fewer pixels and stretches it over its area: the picture gets a little softer, and \
                 the game runs faster."
            ),
            "It applies to this tab now and to the others when they start again. You can change it any time in \
             Options > Application. Turu won't ask again.",
        ),
    }
}

/// The tip was answered: `turn_on` turns Performance mode on.
pub fn answer_tip(tip: &Tip, turn_on: bool) {
    if turn_on && let Tip::PerformanceMode { level, .. } = tip {
        crate::platform::platform().set_performance(*level);
    }
    graphics_tips::answered();
}

// --- the view --------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct PromptsView {
    /// The one waiting, if any.
    pub prompt: Option<PromptView>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct PromptView {
    /// What it is and which: answers name it, so an answer to one gone does nothing.
    pub id: String,
    pub title: String,
    pub text: String,
    /// A smaller line under the text; empty for none.
    pub note: String,
    /// Left to right; the last is the main one.
    pub buttons: Vec<String>,
    /// The text is a script's or a report's own (shown as it is, scrolling when long).
    pub long: bool,
}

/// The prompt waiting for `bot`'s tab, the most pressing first: the script's (it waits on it),
/// Butler's, the crash notice, the Tab Limit, a graphics tip.
pub fn current(bot: &Arc<Bot>) -> Option<PromptView> {
    if let Some(m) = bot.scripts.message_box() {
        return Some(PromptView {
            id: format!("box:{}", m.id),
            title: m.caption,
            text: m.message,
            note: String::new(),
            buttons: m.buttons,
            long: true,
        });
    }
    if let Some(notice) = bot.butler.suspend_notice() {
        return Some(PromptView {
            id: format!("butler:{}", notice.id),
            title: SuspendNotice::CAPTION.into(),
            text: notice.message(),
            note: String::new(),
            buttons: vec!["No".into(), "Yes".into()],
            long: false,
        });
    }
    let crashes = unseen_crashes();
    if !crashes.is_empty() {
        let (title, text, note) = crash_texts(&crashes);
        return Some(PromptView {
            id: format!("crash:{}", crashes.len()),
            title,
            text,
            note,
            buttons: vec!["Close".into(), "Show report".into()],
            long: true,
        });
    }
    if let Some(limit) = tab_refused() {
        let (text, at_most) = tab_limit_text(limit);
        return Some(PromptView {
            id: "tab-limit".into(),
            title: "Tab limit reached".into(),
            text,
            note: String::new(),
            buttons: if at_most {
                vec!["OK".into()]
            } else {
                vec!["Not now".into(), "Raise Tab Limit".into()]
            },
            long: false,
        });
    }
    let tip = graphics_tips::pending()?;
    let (title, text, note) = tip_texts(&tip);
    let (id, buttons) = match tip {
        Tip::QualityLow { .. } => ("tip:quality", vec!["OK".to_owned()]),
        Tip::PerformanceMode { .. } => ("tip:performance", vec!["Not now".to_owned(), "Turn on".to_owned()]),
    };
    Some(PromptView {
        id: id.into(),
        title: title.into(),
        text,
        note: note.into(),
        buttons,
        long: false,
    })
}

/// The view, for a shell: reading it takes the dialogs from egui for a few seconds.
pub fn view(bot: &Arc<Bot>) -> PromptsView {
    *lock(&SHELL_READ) = Some(Instant::now());
    PromptsView { prompt: current(bot) }
}

/// `POST /models/prompts/answer`: `id` (the prompt's) and `button` (its index; absent or -1 when
/// closed without one: Back, a tap outside). Answers `window` (an egui window to open: "logs" for
/// Show report) and `open` (a native screen: "app_options" for Raise Tab Limit), each
/// empty when nothing opens.
pub fn answer(bot: &Arc<Bot>, body: &Json) -> Result<Json, String> {
    let id = body.get("id").and_then(Json::as_str).ok_or("id is missing")?;
    let button = body
        .get("button")
        .and_then(Json::as_i64)
        .filter(|b| *b >= 0)
        .map(|b| b as usize);
    // Only the prompt still waiting: an answer to one already gone does nothing.
    let Some(now) = current(bot) else {
        return Ok(json!({ "window": "", "open": "" }));
    };
    if now.id != id {
        return Ok(json!({ "window": "", "open": "" }));
    }
    let (mut window, mut open) = ("", "");
    let main = |b: Option<usize>| b.is_some_and(|b| b + 1 == now.buttons.len());
    match id.split_once(':').map_or(id, |(kind, _)| kind) {
        "box" => {
            if let Some(m) = bot.scripts.message_box() {
                bot.scripts
                    .answer_message_box(m.id, button.filter(|b| *b < m.buttons.len()));
            }
        }
        "butler" => {
            if let Some(notice) = bot.butler.suspend_notice() {
                // Closing it keeps the follow suspended, as Yes does.
                turu_bot::butler::answer_suspend(bot, notice.id, button != Some(0));
            }
        }
        "crash" => {
            let show = main(button);
            answer_crashes(show);
            if show {
                window = "logs";
            }
        }
        "tab-limit" => {
            answer_tab_refused();
            if now.buttons.len() > 1 && main(button) {
                open = "app_options";
            }
        }
        "tip" => {
            if let Some(tip) = graphics_tips::pending() {
                answer_tip(&tip, main(button));
            }
        }
        other => return Err(format!("no prompt {other}")),
    }
    Ok(json!({ "window": window, "open": open }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stale_answer_does_nothing() {
        let bot = Bot::new();
        assert_eq!(view(&bot).prompt, None);
        assert!(native_shown());
        let reply = answer(&bot, &json!({ "id": "box:7", "button": 0 })).unwrap();
        assert_eq!(reply, json!({ "window": "", "open": "" }));
        assert!(answer(&bot, &json!({})).is_err());
        let (text, at_most) = tab_limit_text(4);
        assert!(at_most && text.starts_with("4 tabs are open"));
        assert_eq!(
            tab_limit_text(1).0.split(' ').take(3).collect::<Vec<_>>(),
            ["1", "tab", "is"]
        );
    }
}
