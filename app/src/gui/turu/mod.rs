//! Turu's egui UI: VibeSkua's layout (a 34 px top nav bar, tool windows over the game) in
//! Mugetsu's colours. See docs/UI.md.
//!
//! Everything here runs on the UI thread and only reads the bot's cached state (locks held
//! briefly). Anything that touches the game goes through `crate::turu::spawn_action`.

mod accounts;
mod application_options;
mod army;
mod auto;
mod butler;
mod client_logs;
mod console;
mod core_bots;
mod cosmetics;
mod current_drops;
mod files;
mod game_options;
mod gear;
mod grabber;
mod loader;
mod logs;
mod message_box;
mod nav_bar;
mod packet_logger;
mod runtime;
mod script_loader;
mod script_options;
mod script_repo;
mod skills;
mod stats;
mod tab_strip;
pub mod theme;
mod widgets;

use crate::gui::{FilePicker, top_height};
use crate::turu::TabId;
use egui::{Context, Rect, Vec2, Window, pos2, vec2};
use std::collections::HashMap;

/// Which tool windows are open.
#[derive(Default)]
pub struct Windows {
    pub script_loader: bool,
    pub script_options: bool,
    pub script_repo: bool,
    pub game_options: bool,
    pub application_options: bool,
    pub core_bots: bool,
    pub skills: bool,
    pub stats: bool,
    pub runtime: bool,
    pub logs: bool,
    pub console: bool,
    pub packet_logger: bool,
    pub current_drops: bool,
    pub loader: bool,
    pub grabber: bool,
    pub accounts: bool,
    pub army: bool,
    pub butler: bool,
    pub cosmetics: bool,
}

/// Where tool windows live: the app window below the nav bar (and the tab strip).
fn window_area(ctx: &Context) -> Rect {
    let screen = ctx.content_rect();
    let top = (screen.top() + top_height() as f32).min(screen.bottom());
    Rect::from_min_max(pos2(screen.left(), top), screen.max)
}

/// A tool window below the nav bar; `slot` staggers the default positions.
///
/// It stays inside [`window_area`] and never gets taller than it, also when the app window
/// shrinks after it opened: content taller than the window scrolls (egui would otherwise grow
/// the window past the screen's bottom and its buttons with it).
fn window<'a>(ctx: &Context, title: &'a str, size: [f32; 2], slot: u32) -> ToolWindow<'a> {
    let offset = 18.0 * slot as f32;
    let area = window_area(ctx);
    let room = (area.size() - vec2(24.0, 24.0)).max(Vec2::ZERO);
    let min = vec2(260.0, 160.0).min(area.size());
    ToolWindow(
        Window::new(title)
            .id(egui::Id::new(("turu_window", title)))
            .default_pos(pos2(16.0 + offset, area.top() + 12.0 + offset))
            .default_size(vec2(size[0], size[1]).min(room).max(min))
            .min_size(min)
            .max_size(area.size())
            .constrain_to(area)
            .vscroll(true)
            .collapsible(false)
            .resizable(true),
    )
}

/// An egui [`Window`] made by [`window`].
#[must_use = "call `show`"]
struct ToolWindow<'a>(Window<'a>);

impl<'a> ToolWindow<'a> {
    fn open(self, open: &'a mut bool) -> Self {
        Self(self.0.open(open))
    }

    fn resizable(self, resizable: bool) -> Self {
        Self(self.0.resizable(resizable))
    }

    fn show<R>(self, ctx: &Context, add_contents: impl FnOnce(&mut egui::Ui) -> R) -> Option<R> {
        let response = self.0.show(ctx, |ui| {
            let inner = add_contents(ui);
            #[cfg(test)]
            tests::record(ui);
            inner
        })?;
        response.inner
    }
}

/// One tab's windows: each tab is a client of its own, with its own windows and nav bar state
/// (VibeSkua's tabs are whole clients). They show the active tab's bot (`turu::bot()`).
#[derive(Default)]
struct TabUi {
    windows: Windows,
    nav: nav_bar::NavBar,
    script_loader: script_loader::ScriptLoader,
    script_options: script_options::ScriptOptionsWindow,
    script_repo: script_repo::ScriptRepoWindow,
    game_options: game_options::GameOptionsWindow,
    application_options: application_options::ApplicationOptionsWindow,
    core_bots: core_bots::CoreBotsWindow,
    skills: skills::SkillsWindow,
    stats: stats::StatsWindow,
    runtime: runtime::RuntimeWindow,
    logs: logs::LogsWindow,
    console: console::ConsoleWindow,
    packet_logger: packet_logger::PacketLogger,
    current_drops: current_drops::CurrentDrops,
    loader: loader::LoaderWindow,
    grabber: grabber::GrabberWindow,
    butler: butler::ButlerWindow,
    cosmetics: cosmetics::CosmeticsWindow,
}

pub struct TuruUi {
    themed: bool,
    picker: FilePicker,
    tabs: HashMap<TabId, TabUi>,
    tab_strip: tab_strip::TabStrip,
    // Shared by the tabs.
    accounts: accounts::AccountsWindow,
    accounts_open: bool,
    army: army::ArmyWindow,
    army_open: bool,
}

impl TuruUi {
    pub fn new(picker: FilePicker) -> Self {
        // Scripts may now ask for the Script Options window (`bot.config.configure`) and show
        // message boxes (`bot.showMessageBox`). New tabs' bots are set up by `turu::add_tab`.
        crate::turu::bot().scripts.set_interactive(true);
        crate::turu::start_script_updates();
        Self {
            themed: false,
            picker,
            tabs: HashMap::new(),
            tab_strip: Default::default(),
            accounts: Default::default(),
            accounts_open: false,
            army: Default::default(),
            army_open: false,
        }
    }

    fn ensure_theme(&mut self, ctx: &Context) {
        if !self.themed {
            theme::install(ctx);
            self.themed = true;
        }
    }

    /// The active tab's windows (made the first time), dropping those of closed tabs.
    fn active(&mut self) -> &mut TabUi {
        let tabs = crate::turu::tabs();
        self.tabs.retain(|id, _| tabs.iter().any(|(tab, _)| tab == id));
        self.tabs.entry(crate::turu::active_tab()).or_default()
    }

    /// The tab strip (with two tabs or more) and the top nav bar (replaces Ruffle's menu bar).
    pub fn show_bar(&mut self, ui: &mut egui::Ui) {
        self.ensure_theme(ui.ctx());
        if crate::turu::tabs().len() > 1 {
            self.tab_strip.show(ui);
        }
        let tab = self.active();
        tab.nav.show(ui, &mut tab.windows);
    }

    /// The tool windows that are open.
    pub fn show_windows(&mut self, ctx: &Context) {
        self.ensure_theme(ctx);
        let picker = self.picker.clone();
        let tab = self.active();
        let w = &mut tab.windows;
        tab.script_loader.show(
            ctx,
            &picker,
            &mut w.script_loader,
            &mut w.script_options,
            &mut w.script_repo,
        );
        tab.script_options.show(ctx, &mut w.script_options);
        tab.script_repo.show(ctx, &mut w.script_repo);
        tab.game_options.show(ctx, &mut w.game_options);
        tab.application_options.show(ctx, &mut w.application_options);
        tab.core_bots.show(ctx, &mut w.core_bots);
        tab.current_drops.show(ctx, &mut w.current_drops);
        tab.loader.show(ctx, &mut w.loader);
        tab.grabber.show(ctx, &mut w.grabber);
        tab.grabber.save_export(&picker, ctx);
        tab.stats.show(ctx, &mut w.stats);
        tab.runtime.show(ctx, &mut w.runtime);
        tab.skills.show(ctx, &mut w.skills);
        tab.logs.show(ctx, &picker, &mut w.logs);
        tab.console.show(ctx, &mut w.console);
        tab.packet_logger.show(ctx, &picker, &mut w.packet_logger);
        tab.butler.show(ctx, &mut w.butler);
        tab.cosmetics.show(ctx, &mut w.cosmetics);
        stop_auto_while_following();
        let (accounts, army) = (std::mem::take(&mut w.accounts), std::mem::take(&mut w.army));
        self.accounts_open |= accounts;
        self.army_open |= army;
        self.accounts.show(ctx, &picker, &mut self.accounts_open);
        self.army.show(ctx, &picker, &mut self.army_open);
        // Last, so the script's message box sits over the other windows.
        message_box::show(ctx);
    }

    /// Whether the bot is doing something the UI shows live (a script, a Console snippet, Auto
    /// Attack).
    pub fn is_busy(&self) -> bool {
        crate::turu::tabs().iter().any(|(_, bot)| busy(bot))
    }
}

/// Whether the bot runs a script, a Console snippet or Auto Attack.
fn busy(bot: &turu_bot::Bot) -> bool {
    bot.scripts.status().is_active() || bot.scripts.is_snippet_running() || auto::running(bot)
}

/// Auto Attack fights the butler's own movement and skills: a tab that follows stops it
/// (VibeSkua ButlerV5 `StopUserAuto`). Not while a script runs: the follow is suspended then,
/// and Stop would stop the script.
fn stop_auto_while_following() {
    for (_, bot) in crate::turu::tabs() {
        if auto::running(&bot) && !bot.scripts.is_running() && bot.butler.following().is_some() {
            bot.log("[Butler] Stopped Auto Attack: it conflicts with following.");
            bot.control.request_stop();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    thread_local! {
        /// How much taller than its window each shown tool window's content is (it scrolls).
        static OVERFLOW: RefCell<Vec<f32>> = const { RefCell::new(Vec::new()) };
    }

    pub(super) fn record(ui: &egui::Ui) {
        let room = ui.clip_rect().height() - ui.spacing().window_margin.sum().y;
        OVERFLOW.with(|o| o.borrow_mut().push(ui.min_rect().height() - room));
    }

    type Show = Box<dyn FnMut(&Context, &mut bool)>;

    /// The windows that need no file picker (which needs a winit event loop).
    fn windows() -> Vec<(&'static str, Show)> {
        fn boxed<W: 'static>(mut w: W, show: fn(&mut W, &Context, &mut bool)) -> Show {
            Box::new(move |ctx, open| show(&mut w, ctx, open))
        }
        vec![
            (
                "Script Options",
                boxed(script_options::ScriptOptionsWindow::default(), |w, c, o| w.show(c, o)),
            ),
            (
                "Search Scripts",
                boxed(script_repo::ScriptRepoWindow::default(), |w, c, o| w.show(c, o)),
            ),
            (
                "Game Options",
                boxed(game_options::GameOptionsWindow::default(), |w, c, o| w.show(c, o)),
            ),
            (
                "Application Options",
                boxed(application_options::ApplicationOptionsWindow::default(), |w, c, o| {
                    w.show(c, o)
                }),
            ),
            (
                "CoreBots Options",
                boxed(core_bots::CoreBotsWindow::default(), |w, c, o| w.show(c, o)),
            ),
            (
                "Current Drops",
                boxed(current_drops::CurrentDrops::default(), |w, c, o| w.show(c, o)),
            ),
            ("Loader", boxed(loader::LoaderWindow::default(), |w, c, o| w.show(c, o))),
            ("Grabber", boxed(grabber::GrabberWindow::default(), |w, c, o| w.show(c, o))),
            ("Stats", boxed(stats::StatsWindow::default(), |w, c, o| w.show(c, o))),
            (
                "Runtime",
                boxed(runtime::RuntimeWindow::default(), |w, c, o| w.show(c, o)),
            ),
            (
                "Advanced Skills",
                boxed(skills::SkillsWindow::default(), |w, c, o| w.show(c, o)),
            ),
            (
                "Console",
                boxed(console::ConsoleWindow::default(), |w, c, o| w.show(c, o)),
            ),
            ("Butler", boxed(butler::ButlerWindow::default(), |w, c, o| w.show(c, o))),
            (
                "Cosmetics & Outfits",
                boxed(cosmetics::CosmeticsWindow::default(), |w, c, o| w.show(c, o)),
            ),
        ]
    }

    /// Shows one window for a few frames on a `screen`-sized app window; returns its rect, the
    /// area it must stay in and how much its content overflows it.
    fn run(screen: Vec2, title: &str, show: &mut Show) -> (Rect, Rect, f32) {
        let ctx = Context::default();
        theme::install(&ctx);
        let mut area = Rect::NOTHING;
        for _ in 0..6 {
            OVERFLOW.with(|o| o.borrow_mut().clear());
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, screen)),
                ..Default::default()
            };
            let mut output = ctx.run_ui(input, |ui| {
                let ctx = ui.ctx().clone();
                let ctx = &ctx;
                area = window_area(ctx);
                show(ctx, &mut true);
            });
            output.textures_delta.clear();
        }
        let rect = ctx
            .memory(|m| m.area_rect(egui::Id::new(("turu_window", title))))
            .expect("shown");
        let overflow = OVERFLOW.with(|o| *o.borrow().last().expect("recorded"));
        (rect, area, overflow)
    }

    /// Every window stays below the nav bar and on screen, however small the app window is
    /// (what doesn't fit scrolls), and needs no scrolling at its default size.
    #[test]
    fn windows_fit_small_screens() {
        let mut failures = Vec::new();
        for screen in [vec2(1400.0, 1000.0), vec2(900.0, 600.0), vec2(640.0, 420.0)] {
            for (title, mut show) in windows() {
                let (rect, area, overflow) = run(screen, title, &mut show);
                println!("{screen:?} {title}: {rect:?} overflows by {overflow:.0}");
                if !area.expand(1.0).contains_rect(rect) {
                    failures.push(format!("{screen:?} {title}: {rect:?} outside {area:?}"));
                }
                if screen.y >= 1000.0 && overflow > 0.5 {
                    failures.push(format!("{title} scrolls at its default size: {overflow} too tall"));
                }
            }
        }
        assert!(failures.is_empty(), "{failures:#?}");
    }
}
