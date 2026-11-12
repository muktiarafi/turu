//! Turu's egui UI: VibeSkua's layout (a 34 px top nav bar, tool windows over the game) in
//! Mugetsu's colours. See docs/UI.md.
//!
//! Everything here runs on the UI thread and only reads the bot's cached state (locks held
//! briefly). Anything that touches the game goes through `crate::turu::spawn_action`.

mod accounts;
mod application_options;
mod army;
mod army_queue;
mod army_travel;
mod auto;
mod butler;
mod client_logs;
mod client_update;
mod console;
mod core_bots;
mod cosmetics;
mod current_drops;
mod fast_travel;
mod files;
mod game_options;
mod gear;
mod grabber;
mod grid_overlay;
mod hotkeys;
mod hotkeys_window;
mod interceptor;
mod junk_items;
mod loader;
mod loadouts;
mod logs;
mod message_box;
mod nav_bar;
mod notifications;
mod notify_drop;
mod packet_filters;
mod packet_logger;
mod runtime;
mod scheduler;
mod script_loader;
mod script_options;
mod script_repo;
mod skills;
mod spammer;
mod stats;
mod tab_strip;
pub mod theme;
mod widgets;

use crate::gui::top_height;
use crate::turu::TabId;
use egui::{Context, Rect, Vec2, Window, pos2, vec2};
use std::collections::HashMap;

/// Which tool windows are open.
#[derive(Default)]
pub struct Windows {
    pub script_loader: bool,
    pub script_options: bool,
    pub script_repo: bool,
    pub scheduler: bool,
    pub game_options: bool,
    pub application_options: bool,
    pub core_bots: bool,
    pub hotkeys: bool,
    pub skills: bool,
    pub stats: bool,
    pub runtime: bool,
    /// Runtime's Notifications: VibeSkua's Notify Drop.
    pub notify_drop: bool,
    pub logs: bool,
    pub console: bool,
    pub packet_logger: bool,
    pub spammer: bool,
    pub interceptor: bool,
    pub current_drops: bool,
    pub loader: bool,
    pub grabber: bool,
    pub junk_items: bool,
    pub fast_travel: bool,
    pub loadouts: bool,
    pub accounts: bool,
    pub army: bool,
    /// Army > Army Queue (VibeSkua Manager's Army Queue tab).
    pub army_queue: bool,
    pub butler: bool,
    pub cosmetics: bool,
    /// Diagnostics > Notification Center.
    pub notifications: bool,
    /// The nav bar's "Update" notice: a newer Turu (`client_update`).
    pub client_update: bool,
}

/// Where tool windows live: the app window below the nav bar (and the tab strip), or right of
/// the rail.
fn window_area(ctx: &Context) -> Rect {
    let screen = ctx.content_rect();
    let top = (screen.top() + top_height() as f32).min(screen.bottom());
    let left = (screen.left() + crate::gui::left_width() as f32).min(screen.right());
    Rect::from_min_max(pos2(left, top), screen.max)
}

/// A tool window below the nav bar; `slot` staggers the default positions.
///
/// It stays inside [`window_area`] and never gets taller than it, also when the app window
/// shrinks after it opened: content taller than the window scrolls (egui would otherwise grow
/// the window past the screen's bottom and its buttons with it). On a touch screen too small
/// for its default size (a phone) it is a full-screen sheet ([`ToolWindow::sheet`]).
fn window<'a>(ctx: &Context, title: &'a str, size: [f32; 2], slot: u32) -> ToolWindow<'a> {
    let offset = 18.0 * slot as f32;
    let area = window_area(ctx);
    let room = (area.size() - vec2(24.0, 24.0)).max(Vec2::ZERO);
    let min = vec2(260.0, 160.0).min(area.size());
    let window = ToolWindow {
        window: Window::new(title)
            .id(egui::Id::new(("turu_window", title)))
            .default_pos(pos2(16.0 + offset, area.top() + 12.0 + offset))
            .default_size(vec2(size[0], size[1]).min(room).max(min))
            .min_size(min)
            .max_size(area.size())
            .constrain_to(area)
            .vscroll(true)
            .collapsible(false)
            .resizable(true),
        sheet: false,
    };
    if phone_sheet(ctx, size) {
        window.sheet(ctx)
    } else {
        window
    }
}

/// Whether a window of `size` is a full-screen sheet: on a touch screen whose window area can't
/// hold it with a margin around (a phone). A floating window there covers the game anyway, leaves
/// a strip of it to tap by mistake, and its fields end up under the keyboard.
fn phone_sheet(ctx: &Context, size: [f32; 2]) -> bool {
    let area = window_area(ctx);
    widgets::touch() && (area.width() < size[0] + 48.0 || area.height() < size[1] + 48.0)
}

/// An egui [`Window`] made by [`window`].
#[must_use = "call `show`"]
struct ToolWindow<'a> {
    window: Window<'a>,
    /// A full-screen sheet: it can't be moved or resized.
    sheet: bool,
}

impl<'a> ToolWindow<'a> {
    fn open(self, open: &'a mut bool) -> Self {
        Self {
            window: self.window.open(open),
            ..self
        }
    }

    fn resizable(self, resizable: bool) -> Self {
        let resizable = resizable && !self.sheet;
        Self {
            window: self.window.resizable(resizable),
            ..self
        }
    }

    /// A fixed size, unless it's a sheet (which fills the screen).
    fn fixed_size(self, size: Vec2) -> Self {
        if self.sheet {
            return self;
        }
        Self {
            window: self.window.fixed_size(size),
            ..self
        }
    }

    /// On a phone: the window fills [`window_area`] (less a small margin), as a full-screen
    /// sheet, and follows it when the screen turns. It can't be moved or resized then.
    fn sheet(self, ctx: &Context) -> Self {
        let rect = window_area(ctx).shrink(4.0);
        Self {
            window: self.window.fixed_rect(rect).movable(false).resizable(false),
            sheet: true,
        }
    }

    fn show<R>(self, ctx: &Context, add_contents: impl FnOnce(&mut egui::Ui) -> R) -> Option<R> {
        let response = self.window.show(ctx, |ui| {
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
    scheduler: scheduler::SchedulerWindow,
    game_options: game_options::GameOptionsWindow,
    application_options: application_options::ApplicationOptionsWindow,
    core_bots: core_bots::CoreBotsWindow,
    hotkeys: hotkeys_window::HotKeysWindow,
    skills: skills::SkillsWindow,
    stats: stats::StatsWindow,
    runtime: runtime::RuntimeWindow,
    notify_drop: notify_drop::NotifyDropWindow,
    logs: logs::LogsWindow,
    console: console::ConsoleWindow,
    packet_logger: packet_logger::PacketLogger,
    spammer: spammer::SpammerWindow,
    interceptor: interceptor::InterceptorWindow,
    current_drops: current_drops::CurrentDrops,
    loader: loader::LoaderWindow,
    grabber: grabber::GrabberWindow,
    junk_items: junk_items::JunkItemsWindow,
    fast_travel: fast_travel::FastTravelWindow,
    loadouts: loadouts::LoadoutsWindow,
    butler: butler::ButlerWindow,
    cosmetics: cosmetics::CosmeticsWindow,
    notifications: notifications::NotificationCenterWindow,
}

pub struct TuruUi {
    themed: bool,
    tabs: HashMap<TabId, TabUi>,
    tab_strip: tab_strip::TabStrip,
    // Shared by the tabs.
    accounts: accounts::AccountsWindow,
    accounts_open: bool,
    army: army::ArmyWindow,
    army_open: bool,
    army_queue: army_queue::ArmyQueueWindow,
    army_queue_open: bool,
}

impl TuruUi {
    #[allow(clippy::new_without_default)] // It starts the launch updates: not a default value.
    pub fn new() -> Self {
        // Scripts may now ask for the Script Options window (`bot.config.configure`) and show
        // message boxes (`bot.showMessageBox`). New tabs' bots are set up by `turu::add_tab`.
        crate::turu::bot().scripts.set_interactive(true);
        crate::turu::start_updates();
        Self {
            themed: false,
            tabs: HashMap::new(),
            tab_strip: Default::default(),
            accounts: Default::default(),
            accounts_open: false,
            army: Default::default(),
            army_open: false,
            army_queue: Default::default(),
            army_queue_open: false,
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
        self.tabs
            .retain(|id, _| tabs.iter().any(|(tab, _)| tab == id));
        self.tabs.entry(crate::turu::active_tab()).or_default()
    }

    /// The tab strip (with two tabs or more) and the nav bar (replaces Ruffle's menu bar): across
    /// the top, or a rail down the left on a touch screen.
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
        grid_overlay::show(ctx);
        let tab = self.active();
        hotkeys::run_pressed(ctx, tab);
        let w = &mut tab.windows;
        tab.script_loader.show(
            ctx,
            &mut w.script_loader,
            &mut w.script_options,
            &mut w.script_repo,
        );
        tab.scheduler.show(ctx, &mut w.scheduler);
        tab.scheduler.files(ctx);
        w.script_repo |= std::mem::take(&mut tab.scheduler.open_repo);
        if let Some(options) = tab.scheduler.edit_options.take() {
            tab.script_options.edit_queued(options);
            w.script_options = true;
        }
        tab.script_options.show(ctx, &mut w.script_options);
        tab.script_repo.show(ctx, &mut w.script_repo);
        // A Start from Search Scripts worked: the Script Loader shows the run, on top.
        if tab.script_repo.take_open_loader() {
            w.script_loader = true;
            let id = egui::Id::new(("turu_window", "Script Loader"));
            ctx.move_to_top(egui::LayerId::new(egui::Order::Middle, id));
            ctx.request_repaint();
        }
        tab.game_options.show(ctx, &mut w.game_options);
        tab.application_options
            .show(ctx, &mut w.application_options);
        tab.core_bots.show(ctx, &mut w.core_bots);
        tab.hotkeys.show(ctx, &mut w.hotkeys);
        tab.current_drops.show(ctx, &mut w.current_drops);
        tab.loader.show(ctx, &mut w.loader);
        tab.grabber.show(ctx, &mut w.grabber);
        tab.grabber.save_export(ctx);
        tab.junk_items.show(ctx, &mut w.junk_items);
        tab.fast_travel.show(ctx, &mut w.fast_travel);
        tab.loadouts.show(ctx, &mut w.loadouts);
        tab.stats.show(ctx, &mut w.stats);
        tab.runtime.show(ctx, &mut w.runtime);
        w.notify_drop |= std::mem::take(&mut tab.runtime.open_notify_drop);
        tab.notify_drop.show(ctx, &mut w.notify_drop);
        tab.skills.show(ctx, &mut w.skills);
        tab.notifications.show(ctx, &mut w.notifications);
        tab.logs.show(ctx, &mut w.logs);
        // A modal: over the windows whatever the order.
        logs::show_crash_notice(ctx, &mut w.logs, &mut tab.logs);
        logs::show_crash_upload_question(ctx);
        tab.console.show(ctx, &mut w.console);
        tab.packet_logger.show(ctx, &mut w.packet_logger);
        tab.spammer.show(ctx, &mut w.spammer);
        tab.spammer.files(ctx);
        tab.interceptor.show(ctx, &mut w.interceptor);
        tab.interceptor.files(ctx);
        tab.butler.show(ctx, &mut w.butler);
        tab.cosmetics.show(ctx, &mut w.cosmetics);
        client_update::show(ctx, &mut w.client_update);
        stop_auto_while_following();
        let (accounts, army) = (std::mem::take(&mut w.accounts), std::mem::take(&mut w.army));
        let army_queue = std::mem::take(&mut w.army_queue);
        self.accounts_open |= accounts;
        self.army_open |= army;
        self.army_queue_open |= army_queue;
        self.accounts.show(ctx, &mut self.accounts_open);
        // Accounts' Search… picks the launch script in the active tab's Search Scripts.
        if let Some(answer) = self.accounts.take_script_search() {
            let tab = self.active();
            tab.script_repo.pick("launched accounts", answer);
            tab.windows.script_repo = true;
            let id = egui::Id::new(("turu_window", "Search Scripts"));
            ctx.move_to_top(egui::LayerId::new(egui::Order::Middle, id));
            ctx.request_repaint();
        }
        self.army.show(ctx, &mut self.army_open);
        // Army Control's Search… picks a script to load in its clients.
        if let Some(answer) = self.army.take_script_search() {
            let tab = self.active();
            tab.script_repo.pick("the Army clients", answer);
            tab.windows.script_repo = true;
            let id = egui::Id::new(("turu_window", "Search Scripts"));
            ctx.move_to_top(egui::LayerId::new(egui::Order::Middle, id));
            ctx.request_repaint();
        }
        self.army_queue.show(ctx, &mut self.army_queue_open);
        // Army Queue's script and options buttons open the active tab's windows.
        let (open_repo, options) = (
            std::mem::take(&mut self.army_queue.open_repo),
            self.army_queue.edit_options.take(),
        );
        if open_repo || options.is_some() {
            let tab = self.active();
            tab.windows.script_repo |= open_repo;
            if let Some(options) = options {
                tab.script_options.edit_queued(options);
                tab.windows.script_options = true;
            }
            ctx.request_repaint();
        }
        // Last, so the script's message box sits over the other windows.
        message_box::show(ctx);
        butler::show_suspend_notice(ctx);
    }

    /// Whether the bot is doing something the UI shows live (a script, a Console snippet, Auto
    /// Attack).
    pub fn is_busy(&self) -> bool {
        crate::turu::tabs().iter().any(|(_, bot)| busy(bot))
    }
}

/// VibeSkua's headless overlay (`HeadlessOverlay`), shown in Headless Mode over the game area
/// (below the bar, `top` points down, and right of the rail): a dark page, a "monitor off" icon and two lines of text.
/// It takes the game's clicks, as VibeSkua's hidden host gets none; the tool windows stay over it.
pub fn show_headless_screen(ctx: &Context, top: f32) {
    let screen = ctx.content_rect();
    let left = screen.left() + crate::gui::left_width() as f32;
    let rect = Rect::from_min_max(pos2(left, screen.top() + top), screen.max);
    egui::Area::new(egui::Id::new("turu_headless_screen"))
        .order(egui::Order::Background)
        .fixed_pos(rect.min)
        .interactable(true)
        .show(ctx, |ui| {
            ui.allocate_rect(rect, egui::Sense::click_and_drag());
            let painter = ui.painter();
            painter.rect_filled(rect, 0.0, theme::INSET);

            // A monitor (36 px, as VibeSkua's icon) with a slash through it.
            let c = rect.center() - vec2(0.0, 24.0);
            let stroke = egui::Stroke::new(2.0, theme::MUTED);
            let screen = Rect::from_center_size(c - vec2(0.0, 3.0), vec2(30.0, 20.0));
            painter.rect_stroke(screen, 3.0, stroke, egui::StrokeKind::Middle);
            let foot = screen.bottom() + 5.0;
            painter.line_segment([pos2(c.x, screen.bottom()), pos2(c.x, foot)], stroke);
            painter.line_segment([pos2(c.x - 7.0, foot), pos2(c.x + 7.0, foot)], stroke);
            painter.line_segment([c - vec2(17.0, 15.0), c + vec2(17.0, 15.0)], stroke);

            painter.text(
                rect.center() + vec2(0.0, 10.0),
                egui::Align2::CENTER_CENTER,
                "Headless Mode Active",
                egui::FontId::proportional(18.0),
                theme::TEXT,
            );
            painter.text(
                rect.center() + vec2(0.0, 34.0),
                egui::Align2::CENTER_CENTER,
                "Viewport suspended to minimize CPU and GPU usage",
                egui::FontId::proportional(12.0),
                theme::MUTED,
            );
        });
}

/// Whether the bot runs a script, a Console snippet or Auto Attack/Hunt.
fn busy(bot: &turu_bot::Bot) -> bool {
    bot.scripts.status().is_active() || bot.scripts.is_snippet_running() || auto::running(bot)
}

/// Auto Attack/Hunt fights the butler's own movement and skills: a tab that follows stops it
/// (VibeSkua ButlerV5 `StopUserAuto`). Not while a script runs: the follow is suspended then,
/// and Stop would stop the script.
fn stop_auto_while_following() {
    for (_, bot) in crate::turu::tabs() {
        if auto::running(&bot) && !bot.scripts.is_running() && bot.butler.following().is_some() {
            bot.log("[Butler] Stopped Auto Attack/Hunt: it conflicts with following.");
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

    pub(super) type Show = Box<dyn FnMut(&Context, &mut bool)>;

    /// The windows that open no file dialog (none opens in tests).
    fn windows() -> Vec<(&'static str, Show)> {
        fn boxed<W: 'static>(mut w: W, show: fn(&mut W, &Context, &mut bool)) -> Show {
            Box::new(move |ctx, open| show(&mut w, ctx, open))
        }
        vec![
            (
                "Script Options",
                boxed(script_options::ScriptOptionsWindow::default(), |w, c, o| {
                    w.show(c, o)
                }),
            ),
            (
                "Search Scripts",
                boxed(script_repo::ScriptRepoWindow::default(), |w, c, o| {
                    w.show(c, o)
                }),
            ),
            (
                "Scheduler",
                boxed(scheduler::SchedulerWindow::default(), |w, c, o| {
                    w.show(c, o)
                }),
            ),
            (
                "Game Options",
                boxed(game_options::GameOptionsWindow::default(), |w, c, o| {
                    w.show(c, o)
                }),
            ),
            (
                "HotKeys",
                boxed(hotkeys_window::HotKeysWindow::default(), |w, c, o| {
                    w.show(c, o)
                }),
            ),
            (
                "Application Options",
                boxed(
                    application_options::ApplicationOptionsWindow::default(),
                    |w, c, o| w.show(c, o),
                ),
            ),
            (
                "CoreBots Options",
                boxed(core_bots::CoreBotsWindow::default(), |w, c, o| w.show(c, o)),
            ),
            (
                "Current Drops",
                boxed(current_drops::CurrentDrops::default(), |w, c, o| {
                    w.show(c, o)
                }),
            ),
            (
                "Loader",
                boxed(loader::LoaderWindow::default(), |w, c, o| w.show(c, o)),
            ),
            (
                "Grabber",
                boxed(grabber::GrabberWindow::default(), |w, c, o| w.show(c, o)),
            ),
            (
                "Junk Items",
                boxed(junk_items::JunkItemsWindow::default(), |w, c, o| {
                    w.show(c, o)
                }),
            ),
            (
                "Fast Travel",
                boxed(fast_travel::FastTravelWindow::default(), |w, c, o| {
                    w.show(c, o)
                }),
            ),
            (
                "Army Queue",
                boxed(army_queue::ArmyQueueWindow::default(), |w, c, o| {
                    w.show(c, o)
                }),
            ),
            (
                "Loadouts Manager",
                boxed(loadouts::LoadoutsWindow::default(), |w, c, o| w.show(c, o)),
            ),
            (
                "Stats",
                boxed(stats::StatsWindow::default(), |w, c, o| w.show(c, o)),
            ),
            (
                "Runtime",
                boxed(runtime::RuntimeWindow::default(), |w, c, o| w.show(c, o)),
            ),
            (
                notify_drop::TITLE,
                boxed(notify_drop::NotifyDropWindow::default(), |w, c, o| {
                    w.show(c, o)
                }),
            ),
            (
                "Advanced Skills",
                boxed(skills::SkillsWindow::default(), |w, c, o| w.show(c, o)),
            ),
            (
                "Console",
                boxed(console::ConsoleWindow::default(), |w, c, o| w.show(c, o)),
            ),
            (
                "Spammer",
                boxed(spammer::SpammerWindow::default(), |w, c, o| w.show(c, o)),
            ),
            (
                "Interceptor",
                boxed(interceptor::InterceptorWindow::default(), |w, c, o| {
                    w.show(c, o)
                }),
            ),
            (
                "Butler",
                boxed(butler::ButlerWindow::default(), |w, c, o| w.show(c, o)),
            ),
            (
                notifications::TITLE,
                boxed(
                    notifications::NotificationCenterWindow::default(),
                    |w, c, o| w.show(c, o),
                ),
            ),
            (
                "Cosmetics & Outfits",
                boxed(cosmetics::CosmeticsWindow::default(), |w, c, o| {
                    w.show(c, o)
                }),
            ),
        ]
    }

    /// Shows one window for a few frames on a `screen`-sized app window; returns its rect, the
    /// area it must stay in and how much its content overflows it.
    pub(super) fn run(screen: Vec2, title: &str, show: &mut Show) -> (Rect, Rect, f32) {
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

    /// The update window, in each install state, stays on screen and needs no scrolling.
    #[test]
    fn client_update_window_renders() {
        use turu_bot::updates::{Asset, Install, Release};
        let asset = |name: &str| Asset {
            name: name.into(),
            url: format!("https://example.invalid/{name}"),
            size: 1,
        };
        let release = Release {
            version: "0.2.0-alpha.10".into(),
            tag: "v0.2.0-alpha.10".into(),
            html_url: "https://example.invalid/notes".into(),
            prerelease: true,
            asset: Some(asset("turu-0.2.0-alpha.10-macos-universal.zip")),
            sums: Some(asset(turu_bot::updates::SUMS_NAME)),
        };
        let states = [
            Install::Idle,
            Install::Busy("Downloading Turu 0.2.0-alpha.10…".into()),
            Install::OnQuit("0.2.0-alpha.10".into()),
            Install::Failed("https://example.invalid/turu.zip: error sending request for url: connection refused".into()),
        ];
        for install in states {
            turu_bot::updates::update_state(|s| {
                s.available = Some(release.clone());
                s.install = install.clone();
            });
            let mut show: Show = Box::new(client_update::show);
            for screen in [vec2(1400.0, 1000.0), vec2(640.0, 420.0)] {
                let (rect, area, overflow) = run(screen, client_update::title(), &mut show);
                assert!(
                    area.expand(1.0).contains_rect(rect),
                    "{install:?} {screen:?}: {rect:?} outside {area:?}"
                );
                if screen.y >= 1000.0 {
                    assert!(overflow <= 0.5, "{install:?}: scrolls by {overflow}");
                }
            }
        }
        turu_bot::updates::update_state(|s| *s = Default::default());
    }

    /// Butler with its settings and debug log open (it scrolls then) stays on screen.
    #[test]
    fn butler_settings_and_log_render() {
        crate::turu::bot().butler.record(
            "Follower",
            "AntiLag ON",
            turu_bot::butler::Level::Success,
        );
        let mut window = butler::ButlerWindow::expanded();
        let mut show: Show = Box::new(move |ctx, open| window.show(ctx, open));
        for screen in [vec2(1400.0, 1000.0), vec2(640.0, 420.0)] {
            let (rect, area, _) = run(screen, "Butler", &mut show);
            assert!(
                area.expand(1.0).contains_rect(rect),
                "{screen:?}: {rect:?} outside {area:?}"
            );
        }
    }

    /// Every symbol the windows draw is in egui's own fonts or Turu's symbol font, whatever fonts
    /// the system has; a missing one draws egui's ◻ box. (`Fonts::has_glyph` can't tell: it says
    /// no for every symbol of the font ◻ comes from, NotoEmoji's ▶ and ♥ among them.) Add new
    /// symbols here, and to `frontend/assets/DejaVuSans-Symbols.ttf` when this fails.
    #[test]
    fn symbols_render() {
        let ctx = egui::Context::default();
        let mut fonts = egui::FontDefinitions::default();
        theme::add_symbol_font(&mut fonts);
        ctx.set_fonts(fonts);
        theme::install(&ctx);
        let mut output = ctx.run_ui(egui::RawInput::default(), |_| {});
        output.textures_delta.clear();
        let font = egui::FontId::proportional(13.0);
        let uv = |c: char| {
            let galley = ctx
                .fonts_mut(|f| f.layout_no_wrap(c.to_string(), font.clone(), egui::Color32::WHITE));
            galley.rows[0].glyphs[0].uv_rect
        };
        let replacement = uv('\u{FFFF}');
        assert_eq!(replacement, uv('◻'), "an unknown character draws ◻");
        let missing: Vec<char> = "«·»×–—…‹›←↑→↓≤≥⋮⋯■▶▾★♡♥⚙✕⟳"
            .chars()
            .filter(|&c| uv(c) == replacement)
            .collect();
        assert!(missing.is_empty(), "missing glyphs: {missing:?}");
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
                    failures.push(format!(
                        "{title} scrolls at its default size: {overflow} too tall"
                    ));
                }
            }
        }
        assert!(failures.is_empty(), "{failures:#?}");
    }
}
