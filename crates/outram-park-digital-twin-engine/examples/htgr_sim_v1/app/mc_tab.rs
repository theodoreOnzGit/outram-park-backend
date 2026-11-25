//! # The Schematic tab's Monte Carlo view (gh:#1740)
//!
//! The coefficient sets the plant can run on, beside each other, with the
//! picker, at the bottom bar as the MSRE simulator's k_eff tab is (maintainer,
//! 2026-11-25: "the monte Carlo tab can be at the bottom? Just like the MSRE
//! UI"). Each set shows its fuel and moderator coefficients, its rod-bank
//! worth and where the numbers come from; a set not committed yet says so
//! with its issue, and no number stands in for it.
//!
//! **Not a live Monte Carlo run.** The HTR-10 whole core is too heavy for this
//! page's worker; it runs live in the Monte Carlo demo's HTR-10 rung, linked
//! from here. The poured-bed numbers are a committed native record.
//!
//! GUI drawing code: exempt from a reaching test (it needs a live egui
//! frame). The rows are `CoefficientSet::values`, tested against what the
//! plant runs on (`physics::coefficient_set`).
//!
//! Research, education and V&V only. NOT VALIDATED.

use egui::{RichText, Ui};
use outram_park_digital_twin_engine::app_scaffold::SharedState;

use crate::app::panels::coefficient_picker;
use crate::app::state::HtgrSnapshot;
use crate::physics::coefficient_set::CoefficientSet;

/// Draw the view in `ui`'s rect.
pub fn draw(ui: &mut Ui, physics: &SharedState<HtgrSnapshot>, snapshot: &HtgrSnapshot) {
    // An 8 px gutter on every side, so the text does not touch the screen edge.
    let inner = ui.max_rect().shrink(8.0);
    let mut ui = ui.new_child(egui::UiBuilder::new().max_rect(inner));
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(&mut ui, |ui| {
        // Clear of "Controls »" (44 px + inset) on the browser's main view.
        #[cfg(target_arch = "wasm32")]
        ui.add_space(super::REOPEN_ROW_PX);
        ui.heading("Monte Carlo: HTR-10 coefficient sets");
        ui.label(
            "The plant's feedback uses a fuel (kernel Doppler) and a moderator (graphite) temperature \
             coefficient, and the ten-rod bank a total worth. Each set below gives all three; the \
             one in use is chosen here or in the controls.",
        );
        ui.add_space(8.0);
        for set in CoefficientSet::ALL {
            let in_use = set == snapshot.coefficient_set;
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.set_width(ui.available_width());
                let title = if in_use { format!("{}  (in use)", set.label()) } else { set.label().to_string() };
                ui.label(RichText::new(title).strong());
                match set.values() {
                    Some((fuel, moderator, worth)) => {
                        egui::Grid::new(("mc_set", set.label())).num_columns(2).show(ui, |ui| {
                            ui.label("Fuel coefficient");
                            ui.label(format!("{:.2} pcm/K", fuel * 1e5));
                            ui.end_row();
                            ui.label("Moderator coefficient");
                            ui.label(format!("{:.2} pcm/K", moderator * 1e5));
                            ui.end_row();
                            ui.label("Rod-bank worth");
                            ui.label(format!("{:.2} % Δk/k", worth * 100.0));
                            ui.end_row();
                        });
                    }
                    None => {
                        ui.label(format!(
                            "Pending: the committed Monte Carlo record of gh:#{} is not in yet.",
                            crate::physics::coefficient_set::POURED_BED_ISSUE
                        ));
                    }
                }
                ui.small(set.source());
            });
            ui.add_space(6.0);
        }
        ui.separator();
        coefficient_picker(ui, physics, snapshot);
        ui.small(
            "A switch keeps the plant's state and reference temperatures, so a hot plant steps in \
             reactivity when the set changes.",
        );
        ui.separator();
        ui.label("Run the HTR-10 whole core yourself, in the browser:");
        ui.hyperlink_to("Monte Carlo demo, HTR-10 core", "../monte-carlo/?rung=htr10&view=core");
        ui.small("Research, education and V&V only. Not validated.");
    });
}
