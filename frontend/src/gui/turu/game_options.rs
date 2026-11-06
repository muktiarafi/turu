//! Options > Game (VibeSkua's 612x720 window): a searchable list of the bot's options. Changes
//! are written to `bot.options` at once (the engine re-applies them every 250 ms); Save keeps
//! them in `options.json`.

use super::theme::{ACCENT, DANGER, MUTED, SUCCESS, TEXT, WARNING};
use super::widgets::{self, Kind};
use egui::{Color32, ComboBox, Context, Layout, RichText, ScrollArea, Ui, vec2};
use std::collections::HashSet;
use std::time::{Duration, Instant};
use turu_bot::options::{BotOptions, HuntPriority};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Section {
    Visuals,
    Combat,
    Drops,
    Relogin,
    Gameplay,
}

impl Section {
    const ALL: [Section; 5] = [Self::Visuals, Self::Combat, Self::Drops, Self::Relogin, Self::Gameplay];

    fn chip(self) -> &'static str {
        match self {
            Self::Visuals => "Visuals",
            Self::Combat => "Combat",
            Self::Drops => "Drops",
            Self::Relogin => "Relogin",
            Self::Gameplay => "Gameplay",
        }
    }

    fn title(self) -> &'static str {
        match self {
            Self::Visuals => "Performance & Visuals",
            Self::Combat => "Combat & Targeting",
            Self::Drops => "Drops & Looting",
            Self::Relogin => "Auto-Relogin & Network",
            Self::Gameplay => "Gameplay & Character",
        }
    }
}

enum Field {
    Bool(fn(&mut BotOptions) -> &mut bool),
    /// A number of at least the minimum, with an optional unit ("ms").
    Int(fn(&mut BotOptions) -> &mut i32, i32, Option<&'static str>),
    /// Set FPS: 0 leaves the frame rate to the game's own setting.
    Fps,
    /// A text box with a hint for when it's empty.
    Text(fn(&mut BotOptions) -> &mut String, &'static str),
    Server,
    Priority,
    ReloadMap,
}

impl Field {
    /// The room the control takes at the row's right (the label and description get the rest).
    fn width(&self) -> f32 {
        match self {
            Field::Bool(_) => 48.0,
            Field::Int(..) | Field::Fps => 100.0,
            Field::ReloadMap => 80.0,
            Field::Server | Field::Priority => 150.0,
            Field::Text(..) => 210.0,
        }
    }
}

struct Entry {
    section: Section,
    label: &'static str,
    /// What the option does, in a line or two under its label.
    description: &'static str,
    field: Field,
}

macro_rules! b {
    ($section:ident, $label:expr, $f:ident, $description:expr) => {
        Entry {
            section: Section::$section,
            label: $label,
            description: $description,
            field: Field::Bool(|o| &mut o.$f),
        }
    };
}
macro_rules! n {
    ($section:ident, $label:expr, $f:ident, $min:expr, $description:expr) => {
        Entry {
            section: Section::$section,
            label: $label,
            description: $description,
            field: Field::Int(|o| &mut o.$f, $min, None),
        }
    };
    ($section:ident, $label:expr, $f:ident, $min:expr, $suffix:expr, $description:expr) => {
        Entry {
            section: Section::$section,
            label: $label,
            description: $description,
            field: Field::Int(|o| &mut o.$f, $min, Some($suffix)),
        }
    };
}

/// Every field, in VibeSkua's order; the Gameplay section is sorted by label. The descriptions
/// say what Turu does with each option (some are only read by scripts, or not used yet).
fn entries() -> Vec<Entry> {
    let mut list = vec![
        b!(
            Visuals,
            "Lag Killer",
            lag_killer,
            "Hides the game world to cut lag and CPU use; the bot keeps playing"
        ),
        b!(Visuals, "Hide Players", hide_players, "Hides player characters"),
        b!(
            Visuals,
            "Disable FX",
            disable_fx,
            "Turns off combat animations for a higher frame rate"
        ),
        b!(
            Visuals,
            "Disable Collisions",
            disable_collisions,
            "Turns off the map's walls and barriers"
        ),
        b!(Visuals, "Show FPS", show_fps, "Shows the game's frame-rate counter"),
        Entry {
            section: Section::Visuals,
            label: "Set FPS",
            description: "Frame rate to force in game. Game's (0) leaves it to the game's own FPS setting",
            field: Field::Fps,
        },
        Entry {
            section: Section::Visuals,
            label: "Reload Map",
            description: "Loads the current map again, for when it's stuck or drawn wrong",
            field: Field::ReloadMap,
        },
        b!(
            Combat,
            "Aggro Monsters",
            aggro_monsters,
            "Makes every monster in your room attack you. It can keep you in combat, so quests won't turn in"
        ),
        b!(
            Combat,
            "Aggro All Monsters",
            aggro_all_monsters,
            "As Aggro Monsters, for every monster on the map"
        ),
        b!(
            Combat,
            "Attack Without Target",
            attack_without_target,
            "Uses skills even with nothing targeted. Use with caution"
        ),
        b!(
            Combat,
            "Infinite Range",
            infinite_range,
            "Hits targets at any distance without walking to them"
        ),
        b!(Combat, "Magnetise", magnetise, "Pulls targeted monsters to you"),
        n!(
            Combat,
            "Hunt Delay",
            hunt_delay,
            0,
            "ms",
            "The least time between room jumps while hunting monsters"
        ),
        n!(
            Combat,
            "Hunt Buffer",
            hunt_buffer,
            0,
            "Kills to wait for before picking up drops while hunting (VibeSkua). Turu doesn't use it yet"
        ),
        b!(
            Combat,
            "Rest Packets",
            rest_packets,
            "Rests about once a second out of combat, so you heal between fights"
        ),
        b!(
            Combat,
            "Safe Timings",
            safe_timings,
            "Scripts that read it (CoreBots) wait for actions such as pickups to finish before going on. Keep it on"
        ),
        b!(
            Drops,
            "Accept AC Drops",
            accept_ac_drops,
            "Picks up every AC item that drops, even ones the script would reject"
        ),
        b!(
            Drops,
            "Accept All Drops",
            accept_all_drops,
            "Picks up every item that drops. Wins over Reject All Drops"
        ),
        b!(
            Drops,
            "Reject All Drops",
            reject_all_drops,
            "Rejects every drop, wanted ones too. Accept AC Drops still picks up AC items first"
        ),
        Entry {
            section: Section::Relogin,
            label: "Relogin Server",
            description: "The server auto-relogin logs into",
            field: Field::Server,
        },
        b!(
            Relogin,
            "Auto Relogin",
            auto_relogin,
            "Logs back in after a disconnect or kick, to Relogin Server"
        ),
        b!(
            Relogin,
            "Auto Relogin Any",
            auto_relogin_any,
            "Relogs to any server but the last one, which is more likely to succeed"
        ),
        b!(
            Relogin,
            "Retry Relogin",
            retry_relogin,
            "Scripts that read it (CoreBots) retry a failed relogin. Auto-relogin always tries up to Relogin Tries times"
        ),
        b!(
            Relogin,
            "Safe Relogin",
            safe_relogin,
            "Waits longer between login attempts (10 s, then 3 s more each try) so the server can drop the old session"
        ),
        n!(
            Relogin,
            "Relogin Tries",
            relogin_tries,
            1,
            "Login attempts before auto-relogin gives up"
        ),
        n!(
            Relogin,
            "Login Timeout",
            login_timeout,
            1000,
            "ms",
            "How long to wait for the world to load after logging in"
        ),
    ];
    let mut gameplay = vec![
        b!(
            Gameplay,
            "Private Rooms",
            private_rooms,
            "Joins maps in a numbered private room (Private Number) instead of a public one"
        ),
        n!(
            Gameplay,
            "Private Number",
            private_number,
            0,
            "The room number private joins use (Private Rooms); 0 uses 100000"
        ),
        b!(
            Gameplay,
            "Skip Cutscenes",
            skip_cutscenes,
            "Skips cutscenes as they start"
        ),
        b!(
            Gameplay,
            "Headless Mode",
            headless_mode,
            "Covers the game and stops drawing it (2 FPS, low quality, muted) to save CPU; the bot keeps playing. Not the same as --headless, which runs Turu with no window"
        ),
        b!(
            Gameplay,
            "Streamer Mode",
            streamer_mode,
            "Hides other players, your name and guild, gold and chat, in game and in Turu's tabs and title, for streaming"
        ),
        Entry {
            section: Section::Gameplay,
            label: "Custom Name",
            description: "A name shown on your character, on your screen only",
            field: Field::Text(|o| &mut o.custom_name, "Your real name"),
        },
        Entry {
            section: Section::Gameplay,
            label: "Custom Guild",
            description: "A guild name shown on your character, on your screen only",
            field: Field::Text(|o| &mut o.custom_guild, "Your real guild"),
        },
        n!(
            Gameplay,
            "Walk Speed",
            walk_speed,
            1,
            "Your character's walk speed; the game's normal speed is 8"
        ),
        n!(
            Gameplay,
            "Action Delay",
            action_delay,
            0,
            "ms",
            "The pause between the bot's actions, such as accepting quests or moving items"
        ),
        n!(
            Gameplay,
            "Maximum Tries",
            maximum_tries,
            1,
            "Attempts to move an item between bank and inventory before giving up"
        ),
        n!(
            Gameplay,
            "Join Map Tries",
            join_map_tries,
            1,
            "Attempts to join a map before giving up"
        ),
        n!(
            Gameplay,
            "Quest Accept And Complete Tries",
            quest_accept_and_complete_tries,
            1,
            "Attempts to accept or turn in a quest before giving up"
        ),
        n!(
            Gameplay,
            "Load Timeout",
            load_timeout,
            1000,
            "ms",
            "How long a loading screen may stay up before Turu logs out (and relogs, with Auto Relogin)"
        ),
        b!(
            Gameplay,
            "Disable Death Ads",
            disable_death_ads,
            "Hides the ad shown when you die"
        ),
        Entry {
            section: Section::Gameplay,
            label: "Hunt Priority",
            description: "Which monster a hunt attacks first (VibeSkua). Turu doesn't use it yet",
            field: Field::Priority,
        },
        b!(
            Gameplay,
            "Use Function Based Skills",
            use_function_based_skills,
            "VibeSkua's choice between skill files and function-based skills. Turu doesn't use it yet"
        ),
    ];
    gameplay.sort_by_key(|e| e.label);
    list.extend(gameplay);
    list
}

/// Hunt Priority's choices, as the combo box shows them.
fn priority_label(priority: HuntPriority) -> &'static str {
    match priority {
        HuntPriority::None => "None",
        HuntPriority::HighHP => "Highest HP",
        HuntPriority::LowHP => "Lowest HP",
        HuntPriority::Close => "Closest",
    }
}

pub struct GameOptionsWindow {
    entries: Vec<Entry>,
    search: String,
    filter: Option<Section>,
    /// `options.json` and when it was read: read again every few seconds, as Application
    /// Options saves two of its options too.
    saved: Option<(BotOptions, Instant)>,
    /// Defaults was clicked once; the next click resets the options.
    confirm_defaults: bool,
    /// What the last Save did, until the options change again.
    save_result: Option<Result<(), String>>,
    /// How tall the footer was last frame: the list leaves exactly that much room.
    footer_height: Option<f32>,
    /// The options changed here since the last Save or Revert (by label): what differs from
    /// `options.json` and isn't here was changed by something else, most often a script.
    edited: HashSet<&'static str>,
}

impl Default for GameOptionsWindow {
    fn default() -> Self {
        Self {
            entries: entries(),
            search: String::new(),
            filter: None,
            saved: None,
            confirm_defaults: false,
            save_result: None,
            footer_height: None,
            edited: HashSet::new(),
        }
    }
}

impl GameOptionsWindow {
    pub fn show(&mut self, ctx: &Context, open: &mut bool) {
        if !*open {
            self.saved = None;
            self.confirm_defaults = false;
            self.save_result = None;
            return;
        }
        let mut window = super::window(ctx, "Game Options", [612.0, 720.0], 1).open(open);
        // On a phone a floating window leaves the footer's buttons off screen: it fills the
        // screen instead.
        let area = super::window_area(ctx);
        if widgets::touch() && (area.width() < 700.0 || area.height() < 760.0) {
            window = window.sheet(ctx);
        }
        window.show(ctx, |ui| self.ui(ui));
    }

    fn ui(&mut self, ui: &mut Ui) {
        let bot = crate::turu::bot();

        ui.spacing_mut().item_spacing = vec2(6.0, 8.0);
        let needle = self.search.trim().to_lowercase();
        // The chips count what the search finds in each section.
        let counts = Section::ALL.map(|section| {
            self.entries
                .iter()
                .filter(|e| e.section == section && matches(e, &needle))
                .count()
        });
        // A short window (a phone held sideways) scrolls the search and chips away with the
        // list; fixed above it, they'd leave room for one option or two.
        let short = ui.available_height() < SHORT_HEIGHT;
        if !short {
            self.search_bar(ui, counts);
        }

        let footer = self.footer_height.unwrap_or(widgets::target(64.0, 76.0)) + ui.spacing().item_spacing.y;
        let list_height = (ui.available_height() - footer).max(80.0);
        let mut snapshot = bot.options.read().clone();
        let saved = self.saved_options(&bot);
        let differs = differing(&self.entries, &snapshot, &saved);
        // Changed here, then back (or saved elsewhere): no longer an edit of ours.
        self.edited.retain(|label| differs.contains(label));
        ScrollArea::vertical()
            .id_salt("game_options_list")
            .max_height(list_height)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if short {
                    self.search_bar(ui, counts);
                }
                let shown: usize = Section::ALL
                    .into_iter()
                    .zip(counts)
                    .filter(|(section, _)| self.filter.is_none_or(|f| f == *section))
                    .map(|(_, count)| count)
                    .sum();
                if shown == 0 {
                    self.empty_state(ui);
                }
                for section in Section::ALL {
                    if self.filter.is_some_and(|f| f != section) {
                        continue;
                    }
                    let rows: Vec<&Entry> = self
                        .entries
                        .iter()
                        .filter(|e| e.section == section && matches(e, &needle))
                        .collect();
                    if rows.is_empty() {
                        continue;
                    }
                    widgets::card(ui, |ui| {
                        ui.set_width(ui.available_width());
                        // The heading only when several sections show (VibeSkua's
                        // `ShowSectionHeaders`); a chip already names the one shown.
                        if self.filter.is_none() {
                            ui.horizontal(|ui| {
                                widgets::section_label(ui, &section.title().to_uppercase());
                                widgets::count_badge(ui, rows.len());
                            });
                            ui.add_space(2.0);
                        }
                        if section == Section::Drops && snapshot.accept_all_drops && snapshot.reject_all_drops {
                            ui.label(
                                RichText::new(
                                    "Accept All Drops and Reject All Drops are both on: Accept All wins, so every \
                                     drop is picked up.",
                                )
                                .small()
                                .color(WARNING),
                            );
                        }
                        for entry in rows {
                            // The label and description beside the control, or above it when
                            // the row is too narrow for both (a phone).
                            let text_width = ui.available_width() - entry.field.width() - 8.0;
                            // Differs from options.json: changed here, or by something else.
                            let mark = differs.contains(&entry.label).then(|| {
                                if self.edited.contains(entry.label) {
                                    ACCENT
                                } else {
                                    WARNING
                                }
                            });
                            let mut changed = false;
                            if text_width >= MIN_TEXT_WIDTH {
                                ui.horizontal(|ui| {
                                    ui.set_min_height(28.0);
                                    ui.vertical(|ui| {
                                        ui.set_width(text_width);
                                        label(ui, entry, mark);
                                    });
                                    ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                                        changed = control(ui, entry, &mut snapshot, &bot);
                                    });
                                });
                            } else {
                                ui.vertical(|ui| {
                                    label(ui, entry, mark);
                                    ui.horizontal(|ui| {
                                        changed = control(ui, entry, &mut snapshot, &bot);
                                    });
                                });
                            }
                            if changed {
                                self.edited.insert(entry.label);
                            }
                        }
                    });
                }
            });

        let live = bot.options.read().clone();
        if live != snapshot {
            // An option changed (here or by a script): a pending Defaults no longer means what it said.
            self.confirm_defaults = false;
        }
        let top = ui.cursor().top();
        self.footer(ui, &bot, live, saved);
        self.footer_height = Some(ui.min_rect().bottom() - top);
    }

    /// The search box and the section chips, which count what the search finds.
    fn search_bar(&mut self, ui: &mut Ui, counts: [usize; 5]) {
        let width = ui.available_width();
        widgets::text_box(ui, &mut self.search, "Search game options", width);
        let mut filter = self.filter;
        ui.horizontal_wrapped(|ui| {
            let all = counts.iter().sum::<usize>();
            if widgets::chip(ui, &format!("All  {all}"), filter.is_none()) {
                filter = None;
            }
            for (section, count) in Section::ALL.into_iter().zip(counts) {
                if widgets::chip(ui, &format!("{}  {count}", section.chip()), filter == Some(section)) {
                    filter = Some(section);
                }
            }
        });
        self.filter = filter;
    }

    /// What the list shows when the search finds nothing: says so, and offers a way out.
    fn empty_state(&mut self, ui: &mut Ui) {
        let search = self.search.trim().to_owned();
        let scope = match self.filter {
            Some(section) => format!("{} options", section.chip()),
            None => "game options".to_owned(),
        };
        // Another section has matches: offer to search them all.
        let elsewhere = self.filter.is_some() && self.entries.iter().any(|e| matches(e, &search.to_lowercase()));
        ui.add_space(24.0);
        ui.vertical_centered(|ui| {
            ui.label(RichText::new(format!("No {scope} match \"{search}\".")).color(TEXT));
            ui.add_space(4.0);
            let size = vec2(0.0, widgets::target(28.0, 40.0));
            if elsewhere && widgets::button(ui, "Search all sections", Kind::Normal, true, size, "").clicked() {
                self.filter = None;
            }
            if widgets::button(ui, "Clear search", Kind::Normal, true, size, "").clicked() {
                self.search.clear();
            }
        });
    }

    /// `options.json` as last read (the defaults when there's none, as the bot starts with).
    fn saved_options(&mut self, bot: &turu_bot::Bot) -> BotOptions {
        let stale = self
            .saved
            .as_ref()
            .is_none_or(|(_, read)| read.elapsed() > Duration::from_secs(2));
        if stale {
            self.saved = Some((bot.paths.load_options().unwrap_or_default(), Instant::now()));
        }
        self.saved.as_ref().map(|(o, _)| o.clone()).unwrap_or_default()
    }

    /// The status line, then Defaults (asks once), Revert and Save; Revert and Save only when the
    /// live options differ from the saved ones.
    fn footer(&mut self, ui: &mut Ui, bot: &std::sync::Arc<turu_bot::Bot>, live: BotOptions, saved: BotOptions) {
        let unsaved = live != saved;
        if unsaved && matches!(self.save_result, Some(Ok(()))) {
            self.save_result = None;
        }
        let at_defaults = live == defaults(&live);
        if at_defaults {
            self.confirm_defaults = false;
        }

        let (text, color) = if self.confirm_defaults {
            (
                "Reset all puts every game option back to its default now, also under a running script.".to_owned(),
                DANGER,
            )
        } else if let Some(Err(e)) = &self.save_result {
            (format!("Couldn't save options.json: {e}"), DANGER)
        } else if unsaved {
            (self.unsaved_status(&live, &saved), WARNING)
        } else if matches!(self.save_result, Some(Ok(()))) {
            ("Saved.".to_owned(), SUCCESS)
        } else {
            ("Changes apply now; Save keeps them for next time.".to_owned(), MUTED)
        };
        ui.label(RichText::new(text).small().color(color));

        let size = vec2(80.0, widgets::target(28.0, 40.0));
        ui.horizontal(|ui| {
            // Defaults asks once: it turns into a red Reset all, with Cancel in Revert's place.
            let (label, kind) = if self.confirm_defaults {
                ("Reset all", Kind::Danger)
            } else {
                ("Defaults", Kind::Normal)
            };
            if widgets::button(ui, label, kind, !at_defaults, size, "Every option is at its default").clicked() {
                if self.confirm_defaults {
                    let reset = defaults(&live);
                    // Defaults is an edit made here, of every option it changes.
                    self.edited.extend(differing(&self.entries, &live, &reset));
                    *bot.options.write() = reset;
                    self.save_result = None;
                }
                self.confirm_defaults = !self.confirm_defaults;
            }
            if self.confirm_defaults {
                if widgets::button(ui, "Cancel", Kind::Normal, true, size, "").clicked() {
                    self.confirm_defaults = false;
                }
            } else if widgets::button(
                ui,
                "Revert",
                Kind::Normal,
                unsaved,
                size,
                "Nothing differs from the saved options",
            )
            .clicked()
            {
                *bot.options.write() = saved;
                self.edited.clear();
                self.confirm_defaults = false;
                self.save_result = None;
            }
            ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                if widgets::button(ui, "Save", Kind::Primary, unsaved, size, "Nothing to save").clicked() {
                    self.confirm_defaults = false;
                    self.save_result = Some(bot.save_options().map_err(|e| e.to_string()));
                    if matches!(self.save_result, Some(Ok(()))) {
                        self.edited.clear();
                    }
                    self.saved = None;
                }
            });
        });
    }
}

impl GameOptionsWindow {
    /// The status line while the live options differ from `options.json`: who changed what. A
    /// script's changes stay after it stops (but Auto Relogin, which stopping turns off), so
    /// Save would keep them too; Revert puts the saved options back.
    fn unsaved_status(&self, live: &BotOptions, saved: &BotOptions) -> String {
        let differs = differing(&self.entries, live, saved);
        let mine = differs.iter().filter(|l| self.edited.contains(*l)).count();
        let outside: Vec<&str> = differs.iter().copied().filter(|l| !self.edited.contains(l)).collect();
        if outside.is_empty() {
            return match mine {
                0 => "Unsaved changes: they apply now; Save keeps them for next time.".to_owned(),
                1 => "You changed 1 option: it applies now; Save keeps it for next time.".to_owned(),
                n => format!("You changed {n} options: they apply now; Save keeps them for next time."),
            };
        }
        let names = match outside.as_slice() {
            [one] => format!("{one} was"),
            [a, b] => format!("{a} and {b} were"),
            [a, b, rest @ ..] => format!("{a}, {b} and {} more were", rest.len()),
            [] => unreachable!(),
        };
        let yours = if mine > 0 {
            format!(" You changed {}.", options(mine))
        } else {
            String::new()
        };
        format!(
            "{names} changed outside this window, most likely by a script.{yours} Save keeps them too; \
             Revert puts the saved options back."
        )
    }
}

/// "1 option", "3 options".
fn options(n: usize) -> String {
    if n == 1 {
        "1 option".to_owned()
    } else {
        format!("{n} options")
    }
}

impl Field {
    /// Whether `a` and `b` hold the same value for this option.
    fn same(&self, a: &mut BotOptions, b: &mut BotOptions) -> bool {
        match self {
            Field::Bool(access) => *access(a) == *access(b),
            Field::Int(access, ..) => *access(a) == *access(b),
            Field::Text(access, _) => *access(a) == *access(b),
            Field::Fps => a.set_fps == b.set_fps,
            Field::Server => a.relogin_server == b.relogin_server,
            Field::Priority => a.hunt_priority == b.hunt_priority,
            Field::ReloadMap => true,
        }
    }
}

/// The labels of the options whose values differ between `a` and `b`, in the window's order.
fn differing(entries: &[Entry], a: &BotOptions, b: &BotOptions) -> Vec<&'static str> {
    let (mut a, mut b) = (a.clone(), b.clone());
    entries
        .iter()
        .filter(|e| !e.field.same(&mut a, &mut b))
        .map(|e| e.label)
        .collect()
}

/// What Defaults sets: the default options, but Relogin Try Delay and Relogin Watchdog, which
/// Application Options shows (and saves), stay as they are.
fn defaults(current: &BotOptions) -> BotOptions {
    BotOptions {
        relogin_try_delay: current.relogin_try_delay,
        relogin_watchdog: current.relogin_watchdog,
        ..BotOptions::default()
    }
}

/// Whether the search (`needle`, lowercase) finds `entry`: in its label or description, or as the
/// start of a word of its section's title ("combat", "network"). Matching anywhere in titles
/// would let "and" or "&" find every option.
fn matches(entry: &Entry, needle: &str) -> bool {
    needle.is_empty()
        || entry.label.to_lowercase().contains(needle)
        || entry.description.to_lowercase().contains(needle)
        || (needle.chars().count() >= 3
            && entry
                .section
                .title()
                .to_lowercase()
                .split(|c: char| !c.is_alphanumeric())
                .any(|word| word.starts_with(needle)))
}

/// Below this height the search and chips scroll with the list.
const SHORT_HEIGHT: f32 = 480.0;

/// The narrowest a row's label and description get beside its control; narrower, the control
/// goes under them.
const MIN_TEXT_WIDTH: f32 = 160.0;

/// An option's label and its description under it; `mark`, a dot after the label in that colour:
/// it differs from options.json (accent: changed here; amber: changed by something else).
fn label(ui: &mut Ui, entry: &Entry, mark: Option<Color32>) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(entry.label).color(TEXT));
        if let Some(color) = mark {
            widgets::dot(ui, color, 3.5);
        }
    });
    ui.label(RichText::new(entry.description).small().color(MUTED));
}

/// Options VibeSkua has that Turu stores but nothing reads yet (their descriptions say so): shown
/// greyed, as Application Options' "coming soon" rows.
const UNUSED: [&str; 3] = ["Hunt Buffer", "Hunt Priority", "Use Function Based Skills"];

/// `entry`'s control, greyed for an [`UNUSED`] option; returns whether it changed the option.
fn control(ui: &mut Ui, entry: &Entry, snapshot: &mut BotOptions, bot: &std::sync::Arc<turu_bot::Bot>) -> bool {
    if UNUSED.contains(&entry.label) {
        ui.add_enabled_ui(false, |ui| edit(ui, entry.label, &entry.field, snapshot, bot))
            .response
            .on_disabled_hover_text("Coming soon: Turu doesn't use this yet");
        return false;
    }
    edit(ui, entry.label, &entry.field, snapshot, bot)
}

/// One option's control. Writes to the live options as soon as it changes, and returns whether
/// it did.
fn edit(
    ui: &mut Ui,
    label: &str,
    field: &Field,
    snapshot: &mut BotOptions,
    bot: &std::sync::Arc<turu_bot::Bot>,
) -> bool {
    let mut changed = false;
    // Combo boxes are as tall as `interact_size`: finger-sized on a touch screen.
    ui.spacing_mut().interact_size.y = widgets::target(24.0, 40.0);
    match field {
        Field::Bool(access) => {
            let mut value = *access(snapshot);
            if widgets::switch(ui, &mut value).changed() {
                *access(&mut bot.options.write()) = value;
                changed = true;
            }
        }
        Field::Int(access, min, suffix) => {
            let mut value = *access(snapshot);
            let speed = (value.abs() as f64 / 200.0).max(1.0);
            let suffix = suffix.map(|s| format!(" {s}")).unwrap_or_default();
            let size = vec2(96.0, widgets::target(24.0, 40.0));
            if widgets::number(ui, label, &mut value, *min..=i32::MAX, speed, &suffix, size).changed() {
                *access(&mut bot.options.write()) = value;
                changed = true;
            }
        }
        Field::Fps => {
            let mut value = snapshot.set_fps;
            let format = widgets::NumberFormat {
                suffix: " FPS",
                zero: Some("Game's"),
                ..Default::default()
            };
            let size = vec2(96.0, widgets::target(24.0, 40.0));
            if widgets::number(ui, label, &mut value, 0..=i32::MAX, 1.0, format, size).changed() {
                bot.options.write().set_fps = value;
                changed = true;
            }
        }
        Field::Text(access, hint) => {
            let mut value = access(snapshot).clone();
            if widgets::text_box(ui, &mut value, hint, 200.0).changed() {
                *access(&mut bot.options.write()) = value;
                changed = true;
            }
        }
        Field::Server => {
            if let Some(name) = widgets::server_combo(ui, "relogin_server", bot, &snapshot.relogin_server) {
                bot.options.write().relogin_server = name;
                changed = true;
            }
        }
        Field::Priority => {
            let current = snapshot.hunt_priority;
            ComboBox::from_id_salt("hunt_priority")
                .selected_text(priority_label(current))
                .width(150.0)
                .show_ui(ui, |ui| {
                    ui.spacing_mut().interact_size.y = widgets::target(24.0, 40.0);
                    for choice in [
                        HuntPriority::None,
                        HuntPriority::HighHP,
                        HuntPriority::LowHP,
                        HuntPriority::Close,
                    ] {
                        if ui.selectable_label(current == choice, priority_label(choice)).clicked() {
                            bot.options.write().hunt_priority = choice;
                            changed = true;
                        }
                    }
                });
        }
        Field::ReloadMap => {
            let in_game = bot.state.lock().in_game;
            let size = vec2(80.0, widgets::target(24.0, 40.0));
            if widgets::button(ui, "Reload", Kind::Normal, in_game, size, "Log in first").clicked() {
                crate::turu::spawn_action("Reload Map", |bot| {
                    turu_bot::api::dispatch(bot, "map.reload", &[]).map(|_| ())
                });
            }
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn found(needle: &str) -> Vec<&'static str> {
        entries()
            .into_iter()
            .filter(|e| matches(e, needle))
            .map(|e| e.label)
            .collect()
    }

    /// A section title's word finds its section ("network": Relogin), but "&" or a word inside
    /// a title finds only what says it, not every option.
    #[test]
    fn search_matches_title_words_not_fragments() {
        let all = entries().len();
        assert_eq!(found("").len(), all);
        assert!(found("&").is_empty());
        assert!(found("and").len() < all);
        let relogin = entries().iter().filter(|e| e.section == Section::Relogin).count();
        assert!(found("network").len() >= relogin);
        assert!(found("combat").contains(&"Magnetise"));
        assert!(found("headless").contains(&"Headless Mode"));
    }

    /// The options that differ from the saved ones, in the window's order; Reload Map never.
    #[test]
    fn differing_lists_changed_options() {
        let entries = entries();
        let saved = BotOptions::default();
        let mut live = saved.clone();
        assert!(differing(&entries, &live, &saved).is_empty());
        live.hide_players = true;
        live.hunt_delay = 500;
        live.relogin_server = "Artix".into();
        live.relogin_try_delay = 5000; // Application Options' option: not in this window.
        assert_eq!(
            differing(&entries, &live, &saved),
            ["Hide Players", "Hunt Delay", "Relogin Server"]
        );
    }

    /// The status says which changes were made here and which elsewhere (a script).
    #[test]
    fn unsaved_status_tells_whose_changes() {
        let mut window = GameOptionsWindow::default();
        let saved = BotOptions::default();
        let mut live = saved.clone();
        live.hide_players = true;
        window.edited.insert("Hide Players");
        assert_eq!(
            window.unsaved_status(&live, &saved),
            "You changed 1 option: it applies now; Save keeps it for next time."
        );
        live.aggro_monsters = true;
        live.skip_cutscenes = true;
        live.accept_all_drops = true;
        assert_eq!(
            window.unsaved_status(&live, &saved),
            "Aggro Monsters, Accept All Drops and 1 more were changed outside this window, most likely by a \
             script. You changed 1 option. Save keeps them too; Revert puts the saved options back."
        );
    }

    /// Every option the window greys out is one it has (a renamed label would leave it live).
    #[test]
    fn unused_options_exist() {
        let labels: Vec<&str> = entries().iter().map(|e| e.label).collect();
        for label in UNUSED {
            assert!(labels.contains(&label), "{label} isn't an option");
        }
    }
}
