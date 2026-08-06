//! Reactor-vessel gallery tab: every scoped reactor architecture, side by side.
//!
//! Shows all six variants of
//! [`outram_park_digital_twin_engine::components::ReactorArchetype`] at once,
//! driven by one shared set of temperatures and one control-rod slider, so the
//! architectures can be compared directly and the colour response checked
//! across all of them in a single glance.
//!
//! **These are illustrative schematics, not validated models and not design
//! drawings.** Geometry does not represent any specific licensed design. Each
//! card names the scoping document that covers that reactor.

use egui::{RichText, Vec2};
use outram_park_digital_twin_engine::components::{ReactorArchetype, ReactorArchetypeVisual};
use uom::si::f64::ThermodynamicTemperature;
use uom::si::thermodynamic_temperature::degree_celsius;

/// Studio state for the reactor gallery.
pub struct ReactorTab {
    /// Core / fuel temperature, degrees Celsius.
    pub core_temp_degc: f64,
    /// Coolant inlet (cold leg) temperature, degrees Celsius.
    pub inlet_temp_degc: f64,
    /// Coolant outlet (hot leg) temperature, degrees Celsius.
    pub outlet_temp_degc: f64,
    /// Cold end of the colour scale, degrees Celsius.
    pub min_temp_degc: f64,
    /// Hot end of the colour scale, degrees Celsius.
    pub max_temp_degc: f64,
    /// Control-rod insertion, dimensionless `[0, 1]`.
    pub rod_frac: f32,
    /// Width of one vessel card, in points. Height follows at a fixed ratio.
    pub cell_width: f32,
    /// Whether to draw the internal component labels.
    pub show_labels: bool,
}

impl Default for ReactorTab {
    /// Defaults sit in a band that spans every reactor in the slate — a BWR
    /// runs near 285 degC while an HTR-10 outlet approaches 700 degC — so one
    /// shared colour scale can show all six without pinning any of them at an
    /// end of the range.
    fn default() -> Self {
        Self {
            core_temp_degc: 620.0,
            inlet_temp_degc: 330.0,
            outlet_temp_degc: 540.0,
            min_temp_degc: 250.0,
            max_temp_degc: 750.0,
            rod_frac: 0.35,
            cell_width: 210.0,
            show_labels: true,
        }
    }
}

fn degc(value: f64) -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<degree_celsius>(value)
}

/// Right-panel controls for the gallery.
pub fn controls(ui: &mut egui::Ui, state: &mut ReactorTab) {
    ui.heading("Reactor vessels");
    ui.label(
        RichText::new(
            "Schematic art for every reactor in docs/reactor-scoping/. \
             Illustrative geometry — not a validated model, and not any \
             specific licensed design.",
        )
        .small()
        .weak(),
    );
    ui.separator();

    ui.label(RichText::new("Temperatures [°C]").strong());
    ui.add(egui::Slider::new(&mut state.core_temp_degc, 100.0..=900.0).text("core / fuel"));
    ui.add(egui::Slider::new(&mut state.inlet_temp_degc, 100.0..=900.0).text("inlet (cold leg)"));
    ui.add(egui::Slider::new(&mut state.outlet_temp_degc, 100.0..=900.0).text("outlet (hot leg)"));

    ui.separator();
    ui.label(RichText::new("Colour scale [°C]").strong());
    ui.label(
        RichText::new(
            "Diverging map: blue at min, neutral white at the MIDPOINT, red at \
             max. The midpoint carries meaning, so set the range about a \
             reference rather than clamping to the extremes seen.",
        )
        .small()
        .weak(),
    );
    ui.add(egui::Slider::new(&mut state.min_temp_degc, 0.0..=600.0).text("min"));
    ui.add(egui::Slider::new(&mut state.max_temp_degc, 300.0..=1200.0).text("max"));
    if state.max_temp_degc <= state.min_temp_degc {
        state.max_temp_degc = state.min_temp_degc + 1.0;
    }

    ui.separator();
    ui.label(RichText::new("Control rods").strong());
    ui.add(
        egui::Slider::new(&mut state.rod_frac, 0.0..=1.0)
            .text("insertion fraction")
            .fixed_decimals(2),
    );
    ui.label(
        RichText::new(
            "0 = fully withdrawn, 1 = fully inserted. Note the BWR draws its \
             rods from BELOW the core, as a real BWR does.",
        )
        .small()
        .weak(),
    );

    ui.separator();
    ui.label(RichText::new("Layout").strong());
    ui.add(egui::Slider::new(&mut state.cell_width, 140.0..=380.0).text("card width [pt]"));
    ui.checkbox(&mut state.show_labels, "show internal labels");
}

/// Draws every archetype as a card in a wrapping grid.
pub fn draw(ui: &mut egui::Ui, state: &ReactorTab) {
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let card_w = state.cell_width;
            let vessel_h = card_w * 1.15;
            let caption_h = 58.0;
            let gap = 14.0;

            let avail = ui.available_width();
            let per_row = ((avail + gap) / (card_w + gap)).floor().max(1.0) as usize;

            let all = ReactorArchetype::ALL;
            for chunk in all.chunks(per_row) {
                ui.horizontal(|ui| {
                    for archetype in chunk {
                        ui.vertical(|ui| {
                            ui.set_width(card_w);

                            // Reserve the vessel box and draw into its centre.
                            let (rect, _response) = ui.allocate_exact_size(
                                Vec2::new(card_w, vessel_h),
                                egui::Sense::hover(),
                            );
                            let vessel = ReactorArchetypeVisual::new(
                                *archetype,
                                rect.center(),
                                Vec2::new(card_w - 16.0, vessel_h - 16.0),
                                degc(state.min_temp_degc),
                                degc(state.max_temp_degc),
                                degc(state.core_temp_degc),
                                degc(state.inlet_temp_degc),
                                degc(state.outlet_temp_degc),
                            )
                            .with_rod_insertion(state.rod_frac);
                            let vessel = if state.show_labels {
                                vessel
                            } else {
                                vessel.without_labels()
                            };
                            ui.put(rect, vessel);

                            // Caption.
                            ui.allocate_ui(Vec2::new(card_w, caption_h), |ui| {
                                ui.label(RichText::new(archetype.label()).strong());
                                ui.label(RichText::new(archetype.description()).small().weak());
                                ui.label(
                                    RichText::new(format!("coolant: {}", archetype.coolant()))
                                        .small()
                                        .weak(),
                                );
                                ui.label(
                                    RichText::new(format!("heat sink: {}", archetype.secondary()))
                                        .small()
                                        .weak(),
                                );
                            });
                        });
                        ui.add_space(gap);
                    }
                });
                ui.add_space(gap);
            }

            ui.separator();
            ui.label(
                RichText::new(
                    "Offline demonstration art. Not for nuclear facility operation, \
                     reactor control, safety-critical decision-making, or licensing.",
                )
                .small()
                .weak(),
            );
        });
}
