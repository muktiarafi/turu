//! The Tab Limit (Turu on Android, Turu Mas: `Host::limits_tabs`): a new tab asked for with the
//! tab strip's "+", Army > New Tab or Accounts' "In a tab" while as many tabs are open as
//! Options > Application > Tab Limit allows isn't opened, and the notice saying why is one of
//! `models::prompts` (drawn by `prompt.rs`, or a native shell).

use crate::models::prompts;
use turu_bot::startup::Startup;

/// As many tabs are open as the Tab Limit allows.
pub fn reached() -> bool {
    prompts::tab_limit_reached()
}

/// Asks for a new tab, unless the Tab Limit is reached: then the notice shows and it's false.
/// Any thread.
pub fn open_tab(startup: Startup) -> bool {
    prompts::open_tab(startup)
}
