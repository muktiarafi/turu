//! Turu's egui UI: VibeSkua's layout (a 34 px top nav bar, tool windows over the game) in
//! Mugetsu's colours. See docs/turu/UI.md.
//!
//! Everything here runs on the UI thread and only reads the bot's cached state (locks held
//! briefly). Anything that touches the game goes through `crate::turu::spawn_action`.

mod accounts;
mod application_options;
mod army;
mod auto;
mod core_bots;
mod current_drops;
mod files;
mod game_options;
mod logs;
mod message_box;
mod nav_bar;
mod packet_logger;
mod script_loader;
mod script_options;
mod skills;
mod stats;
mod tab_strip;
pub mod theme;
mod widgets;

use crate::gui::{FilePicker, top_height};
use crate::turu::TabId;
use egui::{Context, Window, pos2, vec2};
use std::collections::HashMap;

/// Which tool windows are open.
#[derive(Default)]
pub struct Windows {
    pub script_loader: bool,
    pub script_options: bool,
    pub game_options: bool,
    pub application_options: bool,
    pub core_bots: bool,
    pub skills: bool,
    pub stats: bool,
    pub logs: bool,
    pub packet_logger: bool,
    pub current_drops: bool,
    pub accounts: bool,
    pub army: bool,
}

/// A tool window below the nav bar; `slot` staggers the default positions.
fn window<'a>(ctx: &Context, title: &'a str, size: [f32; 2], slot: u32) -> Window<'a> {
    let offset = 18.0 * slot as f32;
    let screen = ctx.content_rect().size();
    let max = vec2(
        (screen.x - 24.0).max(260.0),
        (screen.y - top_height() as f32 - 24.0).max(200.0),
    );
    Window::new(title)
        .id(egui::Id::new(("turu_window", title)))
        .default_pos(pos2(16.0 + offset, top_height() as f32 + 12.0 + offset))
        .default_size(vec2(size[0], size[1]).min(max))
        .min_size(vec2(260.0, 160.0))
        .collapsible(false)
        .resizable(true)
}

/// One tab's windows: each tab is a client of its own, with its own windows and nav bar state
/// (VibeSkua's tabs are whole clients). They show the active tab's bot (`turu::bot()`).
#[derive(Default)]
struct TabUi {
    windows: Windows,
    nav: nav_bar::NavBar,
    script_loader: script_loader::ScriptLoader,
    script_options: script_options::ScriptOptionsWindow,
    game_options: game_options::GameOptionsWindow,
    application_options: application_options::ApplicationOptionsWindow,
    core_bots: core_bots::CoreBotsWindow,
    skills: skills::SkillsWindow,
    stats: stats::StatsWindow,
    logs: logs::LogsWindow,
    packet_logger: packet_logger::PacketLogger,
    current_drops: current_drops::CurrentDrops,
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
        tab.script_loader
            .show(ctx, &picker, &mut w.script_loader, &mut w.script_options);
        tab.script_options.show(ctx, &mut w.script_options);
        tab.game_options.show(ctx, &mut w.game_options);
        tab.application_options.show(ctx, &mut w.application_options);
        tab.core_bots.show(ctx, &mut w.core_bots);
        tab.current_drops.show(ctx, &mut w.current_drops);
        tab.stats.show(ctx, &mut w.stats);
        tab.skills.show(ctx, &mut w.skills);
        tab.logs.show(ctx, &picker, &mut w.logs);
        tab.packet_logger.show(ctx, &picker, &mut w.packet_logger);
        let (accounts, army) = (std::mem::take(&mut w.accounts), std::mem::take(&mut w.army));
        self.accounts_open |= accounts;
        self.army_open |= army;
        self.accounts.show(ctx, &picker, &mut self.accounts_open);
        self.army.show(ctx, &picker, &mut self.army_open);
        // Last, so the script's message box sits over the other windows.
        message_box::show(ctx);
    }

    /// Whether the bot is doing something the UI shows live (a script runs, Auto Attack).
    pub fn is_busy(&self) -> bool {
        crate::turu::tabs()
            .iter()
            .any(|(_, bot)| bot.scripts.status().is_active() || auto::running(bot))
    }
}
