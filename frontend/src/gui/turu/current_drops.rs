//! Tools > Current Drops: the items in the drop stack, with Pick up all / Pick up selected.

use super::theme::{MUTED, SUCCESS, TEXT};
use super::widgets::{self, Kind};
use crate::models::drops as model;
use egui::{Context, RichText, ScrollArea, Ui, vec2};
use std::collections::HashSet;
use std::time::Duration;

#[derive(Default)]
pub struct CurrentDrops {
    search: String,
    ac_only: bool,
    selected: HashSet<i64>,
}

impl CurrentDrops {
    pub fn show(&mut self, ctx: &Context, open: &mut bool) {
        if !*open {
            return;
        }
        super::window(ctx, "Current Drops", [400.0, 380.0], 2)
            .open(open)
            .show(ctx, |ui| self.ui(ui));
        ctx.request_repaint_after(Duration::from_millis(250));
    }

    fn ui(&mut self, ui: &mut Ui) {
        let bot = crate::turu::bot();
        let view = model::view(&bot);
        let drops = view.drops;
        self.selected.retain(|id| drops.iter().any(|d| d.id == *id));
        let ac_count = view.ac_count;

        ui.horizontal(|ui| {
            let width = ui.available_width() - 150.0;
            widgets::text_box(ui, &mut self.search, "Search drops", width);
            widgets::pill(ui, |ui| {
                ui.label(RichText::new(format!("{ac_count} AC")).size(10.0).color(MUTED));
            });
            ui.checkbox(&mut self.ac_only, "AC only");
        });
        let shown: Vec<_> = drops
            .iter()
            .filter(|d| model::shows(d, &self.search, self.ac_only))
            .collect();

        ui.horizontal(|ui| {
            if widgets::button(
                ui,
                "Pick up all",
                Kind::Primary,
                !drops.is_empty(),
                vec2(0.0, 26.0),
                "No drops",
            )
            .clicked()
            {
                model::pickup_all(&bot);
            }
            let count = self.selected.len();
            let label = if count == 0 {
                "None selected".to_owned()
            } else {
                format!("Pick up selected ({count})")
            };
            if widgets::button(ui, &label, Kind::Normal, count > 0, vec2(0.0, 26.0), "").clicked() {
                model::pickup(&bot, self.selected.iter().copied().collect());
            }
        });

        ScrollArea::vertical()
            .id_salt("drops_list")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if shown.is_empty() {
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(if drops.is_empty() {
                            "No drops right now."
                        } else {
                            "No drops match."
                        })
                        .color(MUTED),
                    );
                }
                for drop in shown {
                    ui.horizontal(|ui| {
                        let mut checked = self.selected.contains(&drop.id);
                        if ui.checkbox(&mut checked, "").changed() {
                            if checked {
                                self.selected.insert(drop.id);
                            } else {
                                self.selected.remove(&drop.id);
                            }
                        }
                        ui.label(RichText::new(&drop.name).color(TEXT));
                        if drop.quantity > 1 {
                            ui.label(RichText::new(format!("x{}", drop.quantity)).color(MUTED));
                        }
                        if drop.coins {
                            ui.label(RichText::new("AC").small().strong().color(SUCCESS));
                        }
                    });
                }
            });
    }
}
