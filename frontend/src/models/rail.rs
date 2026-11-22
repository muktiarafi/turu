//! What the rail shows beyond its items' own models: a newer Turu (the Update item and its dot),
//! and the scripts the script chip lists to start with a tap (favourites and recent,
//! `quick_scripts`; Script Loader's star adds one).

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use turu_bot::Bot;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct RailView {
    pub update: Option<UpdateView>,
    pub favourites: Vec<QuickScript>,
    /// Recent scripts that aren't favourites, newest first.
    pub recent: Vec<QuickScript>,
    /// The menus (VibeSkua's, in its order), for a shell's More: what this platform offers.
    pub menus: Vec<MenuView>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct MenuView {
    pub title: &'static str,
    pub items: Vec<MenuItem>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct MenuItem {
    pub label: &'static str,
    /// The window it opens (`gui::turu::Windows::open`'s key), or "new_tab".
    pub window: &'static str,
    pub control: super::Control,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct UpdateView {
    /// The newer version ("0.5.0").
    pub version: String,
    /// What the install does now ("Downloading..."), while it runs.
    pub busy: Option<String>,
    /// Downloaded; installed when Turu quits.
    pub waiting: bool,
    pub failed: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct QuickScript {
    pub path: String,
    /// Its name without `.ts`.
    pub name: String,
}

fn quick(path: &std::path::Path) -> QuickScript {
    QuickScript {
        path: path.to_string_lossy().into_owned(),
        name: path
            .file_stem()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned()),
    }
}

pub fn view(bot: &Arc<Bot>) -> RailView {
    let state = turu_bot::updates::state();
    let update = state.available.map(|release| {
        use turu_bot::updates::Install;
        let (busy, waiting, failed) = match state.install {
            Install::Idle => (None, false, None),
            Install::Busy(what) => (Some(what), false, None),
            Install::OnQuit(_) => (None, true, None),
            Install::Failed(why) => (None, false, Some(why)),
        };
        UpdateView {
            version: release.version,
            busy,
            waiting,
            failed,
        }
    });
    let lists = turu_bot::quick_scripts::get(&bot.paths.quick_scripts_file());
    // Only what's still on disk.
    let here = |path: &&std::path::PathBuf| path.is_file();
    RailView {
        menus: menus(bot),
        update,
        favourites: lists.favorites.iter().filter(here).map(|p| quick(p)).collect(),
        recent: lists.recent_only().filter(here).map(|p| quick(p)).collect(),
    }
}

/// Loads `path` and starts it (the chip's list), as the egui chip does.
pub fn start_quick(bot: &Arc<Bot>, path: &str) -> Result<(), String> {
    super::scripts::load(bot, std::path::Path::new(path))?;
    super::scripts::start(bot)
}

/// VibeSkua's menus as the egui nav bar has them (`nav_bar::menu_contents`), less what this
/// platform can't do: HotKeys without a keyboard, Butler and Army Queue without several clients'
/// processes.
pub fn menus(bot: &Bot) -> Vec<MenuView> {
    let platform = crate::platform::platform();
    let on = super::Control::on;
    let item = |label, window| MenuItem {
        label,
        window,
        control: on(),
    };
    let options = super::scripts::view_controls_edit_options(bot);
    let mut list = vec![
        MenuView {
            title: "Scripts",
            items: vec![
                item("Script Loader", "script_loader"),
                item("Search Scripts", "script_repo"),
                MenuItem {
                    label: "Edit Script Options",
                    window: "script_options",
                    control: options,
                },
                item("Scheduler", "scheduler"),
            ],
        },
        MenuView {
            title: "Options",
            items: [
                Some(item("Game", "game_options")),
                Some(item("Application", "application_options")),
                Some(item("CoreBots", "core_bots")),
                platform.has_hotkeys().then(|| item("HotKeys", "hotkeys")),
            ]
            .into_iter()
            .flatten()
            .collect(),
        },
        MenuView {
            title: "Tools & Helpers",
            items: vec![
                item("Loader", "loader"),
                item("Grabber", "grabber"),
                item("Current Drops", "current_drops"),
                item("Junk Items", "junk_items"),
                item("Loadouts Manager", "loadouts"),
                item("Fast Travel", "fast_travel"),
            ],
        },
        MenuView {
            title: "Combat",
            items: vec![
                item("Skills", "skills"),
                item("Stats", "stats"),
                item("Runtime", "runtime"),
            ],
        },
        MenuView {
            title: "Diagnostics",
            items: vec![
                item("Notification Center", "notifications"),
                item("Logs", "logs"),
                item("Console", "console"),
                item("Spammer", "spammer"),
                item("Packet Logger", "packet_logger"),
                item("Interceptor", "interceptor"),
            ],
        },
        MenuView {
            title: "Plugins",
            items: [
                platform.has_army().then(|| item("Butler", "butler")),
                Some(item("Cosmetics", "cosmetics")),
            ]
            .into_iter()
            .flatten()
            .collect(),
        },
    ];
    if platform.has_army() {
        list.push(MenuView {
            title: "Army",
            items: [
                Some(item("Accounts", "accounts")),
                Some(item("Army Control", "army")),
                platform.launches_processes().then(|| item("Army Queue", "army_queue")),
                Some(item("New Tab", "new_tab")),
            ]
            .into_iter()
            .flatten()
            .collect(),
        });
    }
    list
}

/// The rail's actions (`POST /models/rail/do`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[cfg_attr(
    feature = "typegen",
    derive(facet::Facet),
    facet(tag = "action", rename_all = "kebab-case")
)]
#[serde(tag = "action", rename_all = "kebab-case")]
#[repr(C)]
pub enum RailAct {
    /// Loads and starts the quick script at `path`.
    StartQuick { path: String },
}

pub fn act(bot: &Arc<Bot>, act: RailAct) -> Result<(), String> {
    match act {
        RailAct::StartQuick { path } => start_quick(bot, &path),
    }
}
