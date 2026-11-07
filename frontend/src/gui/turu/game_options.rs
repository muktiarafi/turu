//! Options > Game (VibeSkua's 612x720 window): a searchable list of the bot's options. Changes
//! are written to `bot.options` at once (the engine re-applies them every 250 ms); Save keeps
//! them in `options.json`.

use super::theme::{ACCENT_HOVER, DANGER, MUTED, SUCCESS, TEXT, WARNING};
use super::widgets::{self, Kind};
use egui::{ComboBox, Context, Layout, RichText, ScrollArea, Ui, vec2};
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
            Field::Int(..) => 100.0,
            // The number and its presets' ▾.
            Field::Fps => 100.0 + 6.0 + widgets::target(24.0, 40.0),
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
    /// The small heading this option sits under inside its section ("" for none).
    group: &'static str,
    /// The option this one only matters with (Private Number: Private Rooms): when that is off,
    /// the row is indented and dimmed, but stays editable.
    requires: Option<fn(&BotOptions) -> bool>,
}

macro_rules! b {
    ($section:ident, $label:expr, $f:ident, $description:expr) => {
        Entry {
            section: Section::$section,
            label: $label,
            description: $description,
            field: Field::Bool(|o| &mut o.$f),
            group: "",
            requires: None,
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
            group: "",
            requires: None,
        }
    };
    ($section:ident, $label:expr, $f:ident, $min:expr, $suffix:expr, $description:expr) => {
        Entry {
            section: Section::$section,
            label: $label,
            description: $description,
            field: Field::Int(|o| &mut o.$f, $min, Some($suffix)),
            group: "",
            requires: None,
        }
    };
}

/// An entry that isn't a plain switch or number box.
fn entry(section: Section, label: &'static str, field: Field, description: &'static str) -> Entry {
    Entry {
        section,
        label,
        description,
        field,
        group: "",
        requires: None,
    }
}

/// `entries` under the small heading `group`.
fn grouped(group: &'static str, entries: Vec<Entry>) -> Vec<Entry> {
    entries.into_iter().map(|e| Entry { group, ..e }).collect()
}

/// The heading the [`UNUSED`] options sit under, at the end of their section.
const UNUSED_GROUP: &str = "Not used by Turu yet";

/// Every field, in VibeSkua's order and with its names, but for Gameplay, which is grouped under
/// small headings, and the options Turu doesn't read yet, which go last in their section. The
/// descriptions say what Turu does with each option (some are only read by scripts).
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
        entry(
            Section::Visuals,
            "Set FPS",
            Field::Fps,
            "Frame rate to force in game, up to 240 (▾ for the usual ones). Off (0) leaves it to the game's own FPS setting. On Android, above 60 asks the display for a faster refresh rate (a 90, 120 or 144 Hz screen)",
        ),
        entry(
            Section::Visuals,
            "Reload Map",
            Field::ReloadMap,
            "Loads the current map again, for when it's stuck or drawn wrong",
        ),
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
            "In VibeSkua, kills to wait for before picking up drops while hunting"
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
        entry(
            Section::Combat,
            "Hunt Priority",
            Field::Priority,
            "In VibeSkua, which monster a hunt attacks first",
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
        entry(
            Section::Relogin,
            "Relogin Server",
            Field::Server,
            "The server auto-relogin logs into",
        ),
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
            "Relogin Fallback",
            relogin_fallback,
            "When Relogin Server is full or offline, relogs to any free server. Off, it keeps trying Relogin Server"
        ),
        b!(
            Relogin,
            "Retry Relogin",
            retry_relogin,
            "When Relogin Tries attempts fail, auto-relogin tries again after 1, 2, 5, then every 10 minutes"
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
    list.extend(grouped(
        "World",
        vec![
            b!(
                Gameplay,
                "Private Rooms",
                private_rooms,
                "Joins maps in a numbered private room (Private Number) instead of a public one"
            ),
            Entry {
                requires: Some(|o| o.private_rooms),
                ..n!(
                    Gameplay,
                    "Private Number",
                    private_number,
                    0,
                    "The room number private joins use; 0 uses 100000. Used only while Private Rooms is on"
                )
            },
            n!(
                Gameplay,
                "Walk Speed",
                walk_speed,
                1,
                "Your character's walk speed; the game's normal speed is 8"
            ),
            b!(
                Gameplay,
                "Skip Cutscenes",
                skip_cutscenes,
                "Skips cutscenes as they start"
            ),
            b!(
                Gameplay,
                "Disable Death Ads",
                disable_death_ads,
                "Hides the ad shown when you die. On by default: each ad stays in memory, so a bot that dies often keeps growing"
            ),
            b!(
                Gameplay,
                "Headless Mode",
                headless_mode,
                "Covers the game and stops drawing it (2 FPS, low quality, muted) to save CPU; the bot keeps playing. Not the same as --headless, which runs Turu with no window"
            ),
        ],
    ));
    list.extend(grouped(
        "Privacy & appearance",
        vec![
            b!(
                Gameplay,
                "Streamer Mode",
                streamer_mode,
                "Hides other players, your name and guild, gold and chat, in game and in Turu's tabs and title, for streaming"
            ),
            entry(
                Section::Gameplay,
                "Custom Name",
                Field::Text(|o| &mut o.custom_name, "Empty: your real name"),
                "A name shown on your character, on your screen only",
            ),
            entry(
                Section::Gameplay,
                "Custom Guild",
                Field::Text(|o| &mut o.custom_guild, "Empty: your real guild"),
                "A guild name shown on your character, on your screen only",
            ),
        ],
    ));
    list.extend(grouped(
        "Timing & tries",
        vec![
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
                "Join Map Tries",
                join_map_tries,
                1,
                "Attempts to join a map before giving up"
            ),
            n!(
                Gameplay,
                "Load Timeout",
                load_timeout,
                1000,
                "ms",
                "How long a loading screen may stay up before Turu logs out (and relogs, with Auto Relogin)"
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
                "Quest Accept And Complete Tries",
                quest_accept_and_complete_tries,
                1,
                "Attempts to accept or turn in a quest before giving up"
            ),
        ],
    ));
    list.push(b!(
        Gameplay,
        "Use Function Based Skills",
        use_function_based_skills,
        "In VibeSkua, the choice between skill files and function-based skills"
    ));
    // The unused options last in their section (a stable sort keeps the rest in order), under
    // their own heading.
    list.sort_by_key(|e| (e.section as usize, UNUSED.contains(&e.label)));
    for e in &mut list {
        if UNUSED.contains(&e.label) {
            e.group = UNUSED_GROUP;
        }
    }
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

/// A footer action that asks once before it changes many options at once.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Confirm {
    /// Defaults: puts every option back to its default.
    Defaults,
    /// Revert while a script runs and changed options: it would undo the script's options too.
    Revert,
}

/// A confirming click this soon after the one that asked is ignored: the confirm button takes the
/// asking button's place, so a double-click would otherwise confirm at once.
const CONFIRM_GUARD: Duration = Duration::from_millis(400);

/// How long "Saved to options.json." stays in the status line.
const SAVED_FOR: Duration = Duration::from_secs(4);

pub struct GameOptionsWindow {
    entries: Vec<Entry>,
    search: String,
    filter: Option<Section>,
    /// The Changed chip: only the options that differ from `options.json`.
    changed_only: bool,
    /// `options.json` and when it was read: read again every few seconds, as Application
    /// Options saves two of its options too.
    saved: Option<(BotOptions, Instant)>,
    /// The footer action waiting for its confirming click, and when it asked.
    confirm: Option<(Confirm, Instant)>,
    /// What the last Save did and when, until the options change again ("Saved" fades).
    save_result: Option<(Result<(), String>, Instant)>,
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
            changed_only: false,
            saved: None,
            confirm: None,
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
            self.confirm = None;
            self.save_result = None;
            return;
        }
        let mut window = super::window(ctx, "Game Options", [800.0, 720.0], 1).open(open);
        // On a phone a floating window leaves the footer's buttons off screen: it fills the
        // screen instead.
        let area = super::window_area(ctx);
        if widgets::touch() && (area.width() < 700.0 || area.height() < 760.0) {
            window = window.sheet(ctx);
        }
        window.show(ctx, |ui| self.ui(ui));
    }

    /// Whether the list shows `entry`: the search finds it, and it differs from the saved
    /// options when the Changed chip is on (the section chip is checked apart).
    fn shown(&self, entry: &Entry, needle: &str, differs: &[&str]) -> bool {
        matches(entry, needle) && (!self.changed_only || differs.contains(&entry.label))
    }

    fn ui(&mut self, ui: &mut Ui) {
        let bot = crate::turu::bot();

        ui.spacing_mut().item_spacing = vec2(6.0, 8.0);
        let mut snapshot = bot.options.read().clone();
        let saved = self.saved_options(&bot);
        let differs = differing(&self.entries, &snapshot, &saved);
        // Changed here, then back (or saved elsewhere): no longer an edit of ours.
        self.edited.retain(|label| differs.contains(label));
        // Nothing differs any more (saved, reverted): the Changed chip has nothing to show.
        if differs.is_empty() {
            self.changed_only = false;
        }

        let needle = self.search.trim().to_lowercase();
        // The chips count what the search (and the Changed chip) find in each section.
        let counts = Section::ALL.map(|section| {
            self.entries
                .iter()
                .filter(|e| e.section == section && self.shown(e, &needle, &differs))
                .count()
        });
        // Fixed above the list at every height, as in the other options windows.
        self.search_bar(ui, counts, differs.len());

        let footer = self.footer_height.unwrap_or(widgets::target(64.0, 76.0)) + ui.spacing().item_spacing.y;
        let list_height = (ui.available_height() - footer).max(80.0);
        ScrollArea::vertical()
            .id_salt("game_options_list")
            .max_height(list_height)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let shown: usize = Section::ALL
                    .into_iter()
                    .zip(counts)
                    .filter(|(section, _)| self.filter.is_none_or(|f| f == *section))
                    .map(|(_, count)| count)
                    .sum();
                if shown == 0 {
                    self.empty_state(ui, &differs);
                }
                for section in Section::ALL {
                    if self.filter.is_some_and(|f| f != section) {
                        continue;
                    }
                    let rows: Vec<&Entry> = self
                        .entries
                        .iter()
                        .filter(|e| e.section == section && self.shown(e, &needle, &differs))
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
                        // Differs from options.json: changed here, or by something else.
                        let edited = &self.edited;
                        let mark = |entry: &Entry| {
                            differs.contains(&entry.label).then(|| {
                                if edited.contains(entry.label) {
                                    Mark::Edited
                                } else {
                                    Mark::Outside
                                }
                            })
                        };
                        // VibeSkua's two columns (a row of two options at a time), one when narrow.
                        let two_columns = ui.available_width() >= TWO_COLUMNS_WIDTH;
                        let mut changed = Vec::new();
                        for (i, run) in rows.chunk_by(|a, b| a.group == b.group).enumerate() {
                            let group = run[0].group;
                            if !group.is_empty() {
                                if i > 0 {
                                    ui.add_space(6.0);
                                }
                                ui.label(RichText::new(group).small().strong().color(MUTED));
                            }
                            if !two_columns {
                                for entry in run {
                                    if row(ui, entry, mark(entry), &mut snapshot, &bot) {
                                        changed.push(entry.label);
                                    }
                                }
                                continue;
                            }
                            for pair in run.chunks(2) {
                                ui.columns(2, |columns| {
                                    for (column, entry) in columns.iter_mut().zip(pair) {
                                        if row(column, entry, mark(entry), &mut snapshot, &bot) {
                                            changed.push(entry.label);
                                        }
                                    }
                                });
                            }
                        }
                        self.edited.extend(changed);
                    });
                }
            });

        let live = bot.options.read().clone();
        if live != snapshot {
            // An option changed (here or by a script): a pending confirm no longer means what it said.
            self.confirm = None;
        }
        let top = ui.cursor().top();
        self.footer(ui, &bot, live, saved);
        self.footer_height = Some(ui.min_rect().bottom() - top);
    }

    /// The search box and the chips: the sections, which count what the search finds, and
    /// Changed (`changed` options differ from `options.json`) while anything differs.
    fn search_bar(&mut self, ui: &mut Ui, counts: [usize; 5], changed: usize) {
        let width = ui.available_width();
        widgets::text_box(ui, &mut self.search, "Search game options", width);
        let mut filter = self.filter;
        let mut changed_only = self.changed_only;
        ui.horizontal_wrapped(|ui| {
            let all = counts.iter().sum::<usize>();
            if widgets::chip(ui, &format!("All  {all}"), filter.is_none()) {
                filter = None;
            }
            for (section, count) in Section::ALL.into_iter().zip(counts) {
                let selected = filter == Some(section);
                // A section with nothing to show stays dimmed (unless it's the one shown).
                let clicked = ui
                    .add_enabled_ui(count > 0 || selected, |ui| {
                        widgets::chip(ui, &format!("{}  {count}", section.chip()), selected)
                    })
                    .inner;
                if clicked {
                    filter = Some(section);
                }
            }
            if changed > 0 {
                ui.add_space(6.0);
                if widgets::chip(ui, &format!("Changed  {changed}"), changed_only) {
                    changed_only = !changed_only;
                }
            }
        });
        self.filter = filter;
        self.changed_only = changed_only;
    }

    /// What the list shows when nothing matches: says so, and offers a way out.
    fn empty_state(&mut self, ui: &mut Ui, differs: &[&str]) {
        let search = self.search.trim().to_owned();
        let needle = search.to_lowercase();
        let changed = if self.changed_only { "changed " } else { "" };
        let scope = match self.filter {
            Some(section) => format!("{changed}{} options", section.chip()),
            None => format!("{changed}game options"),
        };
        let text = if search.is_empty() {
            format!("No {scope}.")
        } else {
            format!("No {scope} match \"{search}\".")
        };
        // Another section has matches: offer to look in them all.
        let elsewhere = self.filter.is_some() && self.entries.iter().any(|e| self.shown(e, &needle, differs));
        ui.add_space(24.0);
        ui.vertical_centered(|ui| {
            ui.label(RichText::new(text).color(TEXT));
            ui.add_space(4.0);
            let size = vec2(0.0, widgets::target(28.0, 40.0));
            let all_sections = if search.is_empty() {
                "Show all sections"
            } else {
                "Search all sections"
            };
            if elsewhere && widgets::button(ui, all_sections, Kind::Normal, true, size, "").clicked() {
                self.filter = None;
            }
            if self.changed_only && widgets::button(ui, "Show all options", Kind::Normal, true, size, "").clicked() {
                self.changed_only = false;
            }
            if !search.is_empty() && widgets::button(ui, "Clear search", Kind::Normal, true, size, "").clicked() {
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

    /// Whether a confirming click on `what` counts: it was asked for, not just now.
    fn confirmed(&self, what: Confirm) -> bool {
        self.confirm
            .is_some_and(|(asked, at)| asked == what && at.elapsed() >= CONFIRM_GUARD)
    }

    /// The status line, then Defaults (asks once), Revert (asks once while a script runs and
    /// changed options) and Save; Revert and Save only when the live options differ from the
    /// saved ones.
    fn footer(&mut self, ui: &mut Ui, bot: &std::sync::Arc<turu_bot::Bot>, live: BotOptions, saved: BotOptions) {
        let unsaved = live != saved;
        if let Some((Ok(()), at)) = &self.save_result {
            if unsaved || at.elapsed() >= SAVED_FOR {
                self.save_result = None;
            } else {
                ui.ctx().request_repaint_after(SAVED_FOR - at.elapsed());
            }
        }
        let reset = defaults(&live);
        let to_reset = differing(&self.entries, &live, &reset).len();
        let running = bot.scripts.is_running();
        let differs = differing(&self.entries, &live, &saved);
        let mine = differs.iter().filter(|l| self.edited.contains(*l)).count();
        let outside = differs.len() - mine;
        // What a pending confirm was about is gone: nothing to reset, the script stopped, or
        // nothing it changed is left.
        match self.confirm {
            Some((Confirm::Defaults, _)) if to_reset == 0 => self.confirm = None,
            Some((Confirm::Revert, _)) if !running || outside == 0 => self.confirm = None,
            _ => {}
        }
        let asking = self.confirm.map(|(what, _)| what);

        let (text, color) = match asking {
            Some(Confirm::Defaults) => (
                format!(
                    "Reset puts {} back to {} default now, also under a running script.",
                    options(to_reset),
                    if to_reset == 1 { "its" } else { "their" }
                ),
                DANGER,
            ),
            Some(Confirm::Revert) => {
                let yours = match mine {
                    0 => String::new(),
                    _ => format!(" Revert mine puts back only the {} you changed.", options(mine)),
                };
                (
                    format!(
                        "A script is running: Revert all also undoes the {} changed outside this window, most \
                         likely by it.{yours}",
                        options(outside)
                    ),
                    DANGER,
                )
            }
            None => match &self.save_result {
                Some((Err(e), _)) => (format!("Couldn't save options.json: {e}"), DANGER),
                _ if unsaved => (self.unsaved_status(&live, &saved, running), WARNING),
                Some((Ok(()), _)) => ("Saved to options.json.".to_owned(), SUCCESS),
                None => ("Changes apply now; Save keeps them for next time.".to_owned(), MUTED),
            },
        };
        ui.label(RichText::new(text).small().color(color));

        let size = vec2(80.0, widgets::target(28.0, 40.0));
        ui.horizontal(|ui| match asking {
            Some(Confirm::Revert) => {
                if widgets::button(ui, "Revert all", Kind::Danger, true, size, "").clicked()
                    && self.confirmed(Confirm::Revert)
                {
                    *bot.options.write() = saved.clone();
                    self.edited.clear();
                    self.confirm = None;
                    self.save_result = None;
                }
                if mine > 0
                    && widgets::button(ui, "Revert mine", Kind::Normal, true, size, "").clicked()
                    && self.confirmed(Confirm::Revert)
                {
                    let mut options = bot.options.write();
                    *options = revert_mine(&self.entries, &self.edited, &options, &saved);
                    self.edited.clear();
                    self.confirm = None;
                    self.save_result = None;
                }
                if widgets::button(ui, "Cancel", Kind::Normal, true, size, "").clicked() {
                    self.confirm = None;
                }
            }
            Some(Confirm::Defaults) => {
                if widgets::button(
                    ui,
                    &format!("Reset {}", options(to_reset)),
                    Kind::Danger,
                    true,
                    size,
                    "",
                )
                .clicked()
                    && self.confirmed(Confirm::Defaults)
                {
                    // Defaults is an edit made here, of every option it changes.
                    self.edited.extend(differing(&self.entries, &live, &reset));
                    *bot.options.write() = reset.clone();
                    self.confirm = None;
                    self.save_result = None;
                }
                if widgets::button(ui, "Cancel", Kind::Normal, true, size, "").clicked() {
                    self.confirm = None;
                }
                self.save_button(ui, bot, unsaved, size);
            }
            None => {
                if widgets::button(
                    ui,
                    "Defaults",
                    Kind::Normal,
                    to_reset > 0,
                    size,
                    "Every option is at its default",
                )
                .clicked()
                {
                    self.confirm = Some((Confirm::Defaults, Instant::now()));
                }
                if widgets::button(
                    ui,
                    "Revert",
                    Kind::Normal,
                    unsaved,
                    size,
                    "Nothing differs from the saved options",
                )
                .clicked()
                {
                    if running && outside > 0 {
                        // It would undo what the running script set: ask first.
                        self.confirm = Some((Confirm::Revert, Instant::now()));
                    } else {
                        *bot.options.write() = saved.clone();
                        self.edited.clear();
                        self.save_result = None;
                    }
                }
                self.save_button(ui, bot, unsaved, size);
            }
        });
    }

    /// Save, at the footer's right.
    fn save_button(&mut self, ui: &mut Ui, bot: &turu_bot::Bot, unsaved: bool, size: egui::Vec2) {
        ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
            if widgets::button(ui, "Save", Kind::Primary, unsaved, size, "Nothing to save").clicked() {
                self.confirm = None;
                let result = bot.save_options().map_err(|e| e.to_string());
                if result.is_ok() {
                    self.edited.clear();
                }
                self.save_result = Some((result, Instant::now()));
                self.saved = None;
            }
        });
    }
}

impl GameOptionsWindow {
    /// The status line while the live options differ from `options.json`: who changed what. A
    /// script's changes stay after it stops (but Auto Relogin, which stopping turns off), so
    /// Save would keep them too; Revert puts the saved options back (and asks first while the
    /// script still runs).
    fn unsaved_status(&self, live: &BotOptions, saved: &BotOptions, running: bool) -> String {
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
        let revert = if running {
            "Revert asks first while the script runs"
        } else {
            "Revert puts the saved options back"
        };
        format!("{names} changed outside this window, most likely by a script.{yours} Save keeps them too; {revert}.")
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

impl Field {
    /// Sets this option in `to` to its value in `from`.
    fn copy(&self, from: &BotOptions, to: &mut BotOptions) {
        let mut from = from.clone();
        match self {
            Field::Bool(access) => *access(to) = *access(&mut from),
            Field::Int(access, ..) => *access(to) = *access(&mut from),
            Field::Text(access, _) => *access(to) = std::mem::take(access(&mut from)),
            Field::Fps => to.set_fps = from.set_fps,
            Field::Server => to.relogin_server = from.relogin_server,
            Field::Priority => to.hunt_priority = from.hunt_priority,
            Field::ReloadMap => {}
        }
    }
}

/// Revert mine: `live` with the options changed here (`edited`) put back to `saved`, and
/// everything else (what a script set) left as it is.
fn revert_mine(entries: &[Entry], edited: &HashSet<&'static str>, live: &BotOptions, saved: &BotOptions) -> BotOptions {
    let mut result = live.clone();
    for entry in entries.iter().filter(|e| edited.contains(e.label)) {
        entry.field.copy(saved, &mut result);
    }
    result
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

/// The narrowest a row's label and description get beside its control; narrower, the control
/// goes under them.
const MIN_TEXT_WIDTH: f32 = 160.0;

/// From this wide a section shows its options in two columns (VibeSkua's `UniformGrid`); a phone
/// gets one.
const TWO_COLUMNS_WIDTH: f32 = 640.0;

/// Why a row differs from `options.json`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mark {
    /// Changed in this window.
    Edited,
    /// Changed by something else, most often a script.
    Outside,
}

/// One option's row: its label and description beside its control, or above it when the row is
/// too narrow for both (a phone). An option that [requires](Entry::requires) another is indented,
/// and dimmed while that one is off. Returns whether the control changed the option.
fn row(
    ui: &mut Ui,
    entry: &Entry,
    mark: Option<Mark>,
    snapshot: &mut BotOptions,
    bot: &std::sync::Arc<turu_bot::Bot>,
) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        if let Some(requires) = entry.requires {
            ui.add_space(14.0);
            if !requires(snapshot) {
                ui.multiply_opacity(0.55);
            }
        }
        ui.vertical(|ui| {
            let text_width = ui.available_width() - entry.field.width() - 8.0;
            if text_width >= MIN_TEXT_WIDTH {
                ui.horizontal(|ui| {
                    ui.set_min_height(28.0);
                    ui.vertical(|ui| {
                        ui.set_width(text_width);
                        label(ui, entry, mark);
                    });
                    ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                        changed = control(ui, entry, snapshot, bot);
                    });
                });
            } else {
                label(ui, entry, mark);
                ui.horizontal(|ui| {
                    changed = control(ui, entry, snapshot, bot);
                });
            }
        });
    });
    changed
}

/// An option's label and its description under it; `mark`, a tag after the label saying why it
/// differs from options.json ("Edited" here, or "Changed outside" by something else).
fn label(ui: &mut Ui, entry: &Entry, mark: Option<Mark>) {
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(entry.label).color(TEXT));
        if let Some(mark) = mark {
            let (text, color, why) = match mark {
                Mark::Edited => (
                    "Edited",
                    ACCENT_HOVER,
                    "You changed this here: it applies now, Save keeps it, Revert puts it back.",
                ),
                Mark::Outside => (
                    "Changed outside",
                    WARNING,
                    "Changed outside this window, most likely by a script: it differs from options.json.",
                ),
            };
            // Outlined on the card, not filled: on the grey pill the accent text would be
            // under 4:1.
            egui::Frame::new()
                .stroke(egui::Stroke::new(1.0, color.gamma_multiply(0.6)))
                .corner_radius(egui::CornerRadius::same(8))
                .inner_margin(egui::Margin::symmetric(6, 1))
                .show(ui, |ui| {
                    ui.label(RichText::new(text).size(widgets::small_label_size()).color(color))
                })
                .inner
                .on_hover_text(why);
        }
    });
    ui.label(RichText::new(entry.description).small().color(MUTED));
}

/// Set FPS's ceiling: Ruffle runs up to 1000, but past the fastest phone and desktop displays
/// (240 Hz) the extra frames are never seen and only cost CPU.
const MAX_FPS: i32 = 240;

/// Set FPS's ▾: Off, AQW's own 24, then the usual display refresh rates.
const FPS_PRESETS: [i32; 9] = [0, 24, 30, 60, 90, 120, 144, 165, 240];

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
                zero: Some("Off"),
                ..Default::default()
            };
            let size = vec2(96.0, widgets::target(24.0, 40.0));
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                if widgets::number(ui, label, &mut value, 0..=MAX_FPS, 1.0, format, size).changed() {
                    bot.options.write().set_fps = value;
                    changed = true;
                }
                let side = widgets::target(24.0, 40.0);
                ui.scope(|ui| {
                    ui.spacing_mut().interact_size = vec2(side, side);
                    ui.menu_button("▾", |ui| {
                        for fps in FPS_PRESETS {
                            let text = if fps == 0 {
                                "Off".to_owned()
                            } else {
                                format!("{fps} FPS")
                            };
                            let button = egui::Button::selectable(snapshot.set_fps == fps, text)
                                .min_size(vec2(120.0, widgets::target(24.0, 40.0)));
                            if ui.add(button).clicked() {
                                bot.options.write().set_fps = fps;
                                changed = true;
                                ui.close();
                            }
                        }
                    })
                    .response
                    .on_hover_text("The usual frame rates");
                });
            });
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
            window.unsaved_status(&live, &saved, false),
            "You changed 1 option: it applies now; Save keeps it for next time."
        );
        live.aggro_monsters = true;
        live.skip_cutscenes = true;
        live.accept_all_drops = true;
        assert_eq!(
            window.unsaved_status(&live, &saved, false),
            "Aggro Monsters, Accept All Drops and 1 more were changed outside this window, most likely by a \
             script. You changed 1 option. Save keeps them too; Revert puts the saved options back."
        );
        assert!(
            window
                .unsaved_status(&live, &saved, true)
                .ends_with("Revert asks first while the script runs.")
        );
    }

    /// Revert mine puts back only the options changed here; a script's stay.
    #[test]
    fn revert_mine_keeps_outside_changes() {
        let entries = entries();
        let saved = BotOptions::default();
        let mut live = saved.clone();
        live.hide_players = true; // ours
        live.custom_name = "Me".into(); // ours
        live.aggro_monsters = true; // the script's
        let edited = HashSet::from(["Hide Players", "Custom Name"]);
        let reverted = revert_mine(&entries, &edited, &live, &saved);
        assert_eq!(differing(&entries, &reverted, &saved), ["Aggro Monsters"]);
    }

    /// Unused options come last in their section, under their own heading; Private Number
    /// follows Private Rooms, which it requires.
    #[test]
    fn entries_order() {
        let list = entries();
        for section in Section::ALL {
            let rows: Vec<&Entry> = list.iter().filter(|e| e.section == section).collect();
            let first_unused = rows
                .iter()
                .position(|e| UNUSED.contains(&e.label))
                .unwrap_or(rows.len());
            assert!(rows[first_unused..].iter().all(|e| e.group == UNUSED_GROUP));
            assert!(rows[..first_unused].iter().all(|e| e.group != UNUSED_GROUP));
        }
        // Sections stay together, in the chips' order.
        let order: Vec<usize> = list.iter().map(|e| e.section as usize).collect();
        assert!(order.is_sorted());
        let at = |label| list.iter().position(|e| e.label == label).unwrap();
        assert_eq!(at("Private Number"), at("Private Rooms") + 1);
        assert!(list[at("Private Number")].requires.is_some());
        assert_eq!(list[at("Hunt Priority")].section as usize, Section::Combat as usize);
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
