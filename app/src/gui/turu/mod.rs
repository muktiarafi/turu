//! Turu's egui UI: VibeSkua's layout (a 34 px top nav bar, tool windows over the game) in
//! Mugetsu's colours. See docs/turu/UI.md.
//!
//! Everything here runs on the UI thread and only reads the bot's cached state (locks held
//! briefly). Anything that touches the game goes through `crate::turu::spawn_action`.

mod auto;
mod current_drops;
mod files;
mod game_options;
mod logs;
mod nav_bar;
mod packet_logger;
mod script_loader;
mod skills;
mod stats;
pub mod theme;
mod widgets;

use crate::gui::{FilePicker, MENU_HEIGHT};
use egui::{Context, Window, pos2, vec2};

/// Which tool windows are open.
#[derive(Default)]
pub struct Windows {
    pub script_loader: bool,
    pub game_options: bool,
    pub skills: bool,
    pub stats: bool,
    pub logs: bool,
    pub packet_logger: bool,
    pub current_drops: bool,
}

/// A tool window below the nav bar; `slot` staggers the default positions.
fn window<'a>(ctx: &Context, title: &'a str, size: [f32; 2], slot: u32) -> Window<'a> {
    let offset = 18.0 * slot as f32;
    let screen = ctx.content_rect().size();
    let max = vec2(
        (screen.x - 24.0).max(260.0),
        (screen.y - MENU_HEIGHT as f32 - 24.0).max(200.0),
    );
    Window::new(title)
        .id(egui::Id::new(("turu_window", title)))
        .default_pos(pos2(16.0 + offset, MENU_HEIGHT as f32 + 12.0 + offset))
        .default_size(vec2(size[0], size[1]).min(max))
        .min_size(vec2(260.0, 160.0))
        .collapsible(false)
        .resizable(true)
}

pub struct TuruUi {
    themed: bool,
    picker: FilePicker,
    windows: Windows,
    nav: nav_bar::NavBar,
    script_loader: script_loader::ScriptLoader,
    game_options: game_options::GameOptionsWindow,
    skills: skills::SkillsWindow,
    stats: stats::StatsWindow,
    logs: logs::LogsWindow,
    packet_logger: packet_logger::PacketLogger,
    current_drops: current_drops::CurrentDrops,
}

impl TuruUi {
    pub fn new(picker: FilePicker) -> Self {
        Self {
            themed: false,
            picker,
            windows: Windows::default(),
            nav: Default::default(),
            script_loader: Default::default(),
            game_options: Default::default(),
            skills: Default::default(),
            stats: Default::default(),
            logs: Default::default(),
            packet_logger: Default::default(),
            current_drops: Default::default(),
        }
    }

    fn ensure_theme(&mut self, ctx: &Context) {
        if !self.themed {
            theme::install(ctx);
            self.themed = true;
        }
    }

    /// The top nav bar (replaces Ruffle's menu bar).
    pub fn show_bar(&mut self, ui: &mut egui::Ui) {
        self.ensure_theme(ui.ctx());
        self.nav.show(ui, &mut self.windows);
    }

    /// The tool windows that are open.
    pub fn show_windows(&mut self, ctx: &Context) {
        self.ensure_theme(ctx);
        let w = &mut self.windows;
        self.script_loader.show(ctx, &self.picker, &mut w.script_loader);
        self.game_options.show(ctx, &mut w.game_options);
        self.current_drops.show(ctx, &mut w.current_drops);
        self.stats.show(ctx, &mut w.stats);
        self.skills.show(ctx, &mut w.skills);
        self.logs.show(ctx, &self.picker, &mut w.logs);
        self.packet_logger.show(ctx, &self.picker, &mut w.packet_logger);
    }

    /// Whether the bot is doing something the UI shows live (a script runs, Auto Attack).
    pub fn is_busy(&self) -> bool {
        crate::turu::bot().scripts.status().is_active() || auto::running()
    }
}
