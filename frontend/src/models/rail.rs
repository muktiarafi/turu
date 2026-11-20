//! What the rail shows beyond its items' own models: a newer Turu (the Update item and its dot),
//! and the scripts the script chip lists to start with a tap (favourites and recent,
//! `quick_scripts`; Script Loader's star adds one).

use serde::Serialize;
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
