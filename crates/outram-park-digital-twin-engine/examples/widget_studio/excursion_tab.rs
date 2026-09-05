//! Excursion-overlay tab: drive the fuel-excursion annotation directly, rather
//! than having to run a reactor out of its envelope to see it.
//!
//! [`ExcursionOverlay`] annotates a reactor whose fuel has gone past the
//! temperature it is allowed to reach. In a simulator the only way to reach
//! that state is to actually trigger a prompt excursion and wait — which is a
//! poor way to check whether an animation eases correctly. So this tab exposes
//! the trigger itself:
//!
//! - a **fuel-temperature** slider sweeping through both HTR-10 landmarks, the
//!   reactor's own 1230 degC limit and the generic coated-particle retention
//!   figure at 1600 degC, with buttons that jump to each; and
//! - a raw **intensity** slider, for the caller whose criterion is not a fuel
//!   temperature at all.
//!
//! A **stage ladder** along the bottom shows all three stages at once —
//! quiescent, limit-exceeded, fission-product release — so the escalation can
//! be compared side by side rather than remembered between slider drags.
//!
//! ## The middle band is where the fuel is still doing its job
//!
//! Dragging the fuel temperature from 1230 degC towards 1600 degC deepens the
//! **warning border** and nothing else, because that is what the evidence
//! supports: HTR-10 coating integrity was experimentally proven to 1250 degC
//! (Gao & Shi 2002) and the German heating tests found no particle failures and
//! no noticeable caesium or strontium release in the first few hundred hours of
//! any 1600 degC test (Kugeler et al. 2017, EUR 28712 EN, section 4.2.1). The
//! release annotation appears only at the far landmark. Checking that by hand
//! is one of the things this tab is for.
//!
//! ## It composes; it does not own the vessel
//!
//! The overlay is drawn over a real
//! [`outram_park_digital_twin_engine::components::ReactorArchetypeVisual`],
//! given the same centre and size, exactly as an application would compose it.
//! Nothing in the overlay knows what is underneath it, which is the property
//! this tab exists to demonstrate: drag the fuel temperature and watch the
//! vessel underneath redden on the ordinary temperature scale while the
//! annotation escalates on its own hazard palette above it.
//!
//! ## The clock is owned here, not by the widget
//!
//! The annotation is time-phased. Visual components are rebuilt every repaint,
//! so this tab owns the [`uom::si::f64::Time`] elapsed since the excursion was
//! triggered and advances it in [`ExcursionTab::step`] — the same arrangement
//! as `PumpTab` and the turbine's `simulation_time`. "↺ replay" rewinds it to
//! zero so the expansion can be watched again without touching the trigger.
//!
//! **Offline demonstration art.** The annotation is a warning label. It is not
//! a release model, not an accident analysis and not a source term, and it
//! depicts no explosion — a helium-cooled graphite core has no blast mechanism
//! available at these conditions, and the failure mode that does apply is
//! progressive and passive. No temperature at which a core is destroyed is
//! published or invented. Per `RESPONSIBLE_USE.md` this is for education,
//! research and V&V only.

use egui::{Color32, RichText, Vec2};
use outram_park_digital_twin_engine::components::excursion::{
    banner_pulse, release_phase, release_reach, ExcursionOverlay, ExcursionStage, ExcursionTrigger,
    RELEASE_INTENSITY, RELEASE_RAMP_SECONDS,
};
use outram_park_digital_twin_engine::components::{ReactorArchetype, ReactorArchetypeVisual};
use uom::si::f64::{ThermodynamicTemperature, Time};
use uom::si::thermodynamic_temperature::degree_celsius;
use uom::si::time::second;
use uom::ConstZero;

/// Which trigger the tab is driving the overlay with.
///
/// A studio-local enum rather than a flag, matching `WidgetUnderTest`: the set
/// is closed, so adding a source is a variant and the compiler then points at
/// every match that needs handling. No trait objects, per the workspace rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExcursionSource {
    /// A fuel temperature judged against the HTR-10's own limit — the usual
    /// case, and the one that reports real numbers.
    FuelTemperature,
    /// A bare intensity, for a criterion that is not a fuel temperature.
    Intensity,
}

impl ExcursionSource {
    /// Both sources, in picker order.
    pub const ALL: &'static [Self] = &[Self::FuelTemperature, Self::Intensity];

    /// Human-readable name for the picker.
    pub fn label(self) -> &'static str {
        match self {
            Self::FuelTemperature => "fuel temperature (HTR-10 landmarks)",
            Self::Intensity => "raw intensity",
        }
    }
}

/// The HTR-10's own specified maximum fuel temperature, degrees Celsius.
///
/// Gao & Shi (2002), via
/// `outram_park_digital_twin_engine::htr10::design`. The overlay starts here.
/// Quoted in this file only to place the slider's landmark buttons; the widget
/// reads it from the design module, not from here.
const HTR10_LIMIT_DEGC: f64 = 1230.0;

/// The **generic** modular-HTR fuel temperature limit, degrees Celsius —
/// **not** an HTR-10 limit.
///
/// The overlay reaches full intensity and escalates to the release annotation
/// here, and not before: the heating tests report near-100 % retention at this
/// temperature for the first hundred hours or more (Kugeler et al. 2017,
/// section 4.2.1). Conflating this with [`HTR10_LIMIT_DEGC`] misstates the
/// HTR-10 margin by 370 K, which is why both are labelled wherever they appear.
const GENERIC_RETENTION_DEGC: f64 = 1600.0;

/// The temperature at which the same heating tests report SiC becoming
/// permeable to most fission products, with no delay in caesium release,
/// degrees Celsius.
///
/// Kugeler et al. (2017) section 4.2.1. Used only to place a slider button, so
/// the reader can reach a temperature at which the release annotation is
/// unambiguously the right thing to draw. It is **not** a landmark of the
/// intensity ramp — that stays 1230 to 1600 degC.
const SIC_PERMEABLE_DEGC: f64 = 1800.0;

/// Studio state for the excursion overlay.
pub struct ExcursionTab {
    /// Which trigger drives the overlay.
    pub source: ExcursionSource,
    /// Peak fuel temperature, degrees Celsius. Used by
    /// [`ExcursionSource::FuelTemperature`], and also drives the core colour of
    /// the vessel underneath, so one number moves both.
    pub fuel_degc: f64,
    /// Raw intensity, dimensionless `[0, 1]`. Used by
    /// [`ExcursionSource::Intensity`].
    pub intensity: f32,
    /// Whether the tab's simulation clock is advancing.
    pub running: bool,
    /// Simulation time elapsed since the excursion was triggered. Owned here,
    /// advanced in [`ExcursionTab::step`].
    pub simulation_time: Time,
    /// What the annotation names, e.g. `"HTR-10 core"`. Empty draws the
    /// headline alone.
    pub subject: String,
    /// Coolant inlet temperature of the vessel underneath, degrees Celsius.
    pub vessel_inlet_degc: f64,
    /// Coolant outlet temperature of the vessel underneath, degrees Celsius.
    pub vessel_outlet_degc: f64,
    /// Cold end of the vessel's colour scale, degrees Celsius.
    pub min_temp_degc: f64,
    /// Hot end of the vessel's colour scale, degrees Celsius. Deliberately
    /// above the generic retention figure so a runaway core still has somewhere
    /// to go on the scale.
    pub max_temp_degc: f64,
    /// Width of the main card, in points.
    pub card_width: f32,
    /// Whether to draw the banner and the numeric readouts on the overlay.
    pub show_labels: bool,
    /// Whether to draw a reactor vessel under the annotation.
    ///
    /// Turning it off shows the annotation alone — useful for judging the
    /// graphic, and a direct demonstration that the overlay composes over
    /// whatever is there rather than owning it.
    pub show_vessel: bool,
}

impl Default for ExcursionTab {
    /// Defaults put the fuel at 1650 degC — just past the generic 1600 degC
    /// figure — so the tab opens on the release annotation, which is the part
    /// that most needs looking at. Dragging down through 1600 degC drops
    /// straight back to the warning border, which is the check that matters:
    /// nothing may be drawn over the fuel between 1230 and 1600 degC. The
    /// vessel scale runs 250-1700 degC so a runaway core reddens without
    /// pinning at the top of the map.
    fn default() -> Self {
        Self {
            source: ExcursionSource::FuelTemperature,
            fuel_degc: 1650.0,
            intensity: 0.6,
            running: true,
            simulation_time: Time::ZERO,
            subject: "HTR-10 core".to_string(),
            vessel_inlet_degc: 250.0,
            vessel_outlet_degc: 700.0,
            min_temp_degc: 250.0,
            max_temp_degc: 1700.0,
            card_width: 300.0,
            show_labels: true,
            show_vessel: true,
        }
    }
}

impl ExcursionTab {
    /// Advance the tab's simulation clock by `dt`.
    ///
    /// Called once per frame by the studio from real elapsed time. Does nothing
    /// while paused, which freezes the expansion where it is.
    pub fn step(&mut self, dt: Time) {
        if self.running {
            self.simulation_time += dt;
        }
    }

    /// Rewind the clock to the instant of the trigger, so the expansion can be
    /// watched again without changing the trigger.
    pub fn replay(&mut self) {
        self.simulation_time = Time::ZERO;
    }

    /// The trigger the overlay is built from.
    pub fn trigger(&self) -> ExcursionTrigger {
        match self.source {
            ExcursionSource::FuelTemperature => {
                ExcursionTrigger::htr10_fuel_temperature(degc(self.fuel_degc))
            }
            ExcursionSource::Intensity => ExcursionTrigger::Intensity(self.intensity),
        }
    }

    /// The overlay for the given screen box.
    ///
    /// Shared by [`draw`] and the readout so what is reported and what is drawn
    /// cannot drift apart.
    pub fn overlay(&self, centre: egui::Pos2, size: Vec2) -> ExcursionOverlay {
        let overlay =
            ExcursionOverlay::new(self.trigger(), centre, size).since_trigger(self.simulation_time);
        let overlay = if self.subject.trim().is_empty() {
            overlay
        } else {
            overlay.with_subject(self.subject.trim().to_string())
        };
        if self.show_labels {
            overlay
        } else {
            overlay.without_labels()
        }
    }

    /// The vessel drawn under the annotation.
    ///
    /// Its core is coloured by the same fuel-temperature slider that drives the
    /// trigger, so one number moves both — on two different colour scales, which
    /// is the composition this tab exists to show.
    pub fn vessel(&self, centre: egui::Pos2, size: Vec2) -> ReactorArchetypeVisual {
        ReactorArchetypeVisual::new(
            ReactorArchetype::Htr10,
            centre,
            size,
            degc(self.min_temp_degc),
            degc(self.max_temp_degc),
            degc(self.fuel_degc),
            degc(self.vessel_inlet_degc),
            degc(self.vessel_outlet_degc),
        )
    }

    /// The stage the overlay is currently at.
    pub fn stage(&self) -> ExcursionStage {
        ExcursionStage::from_intensity(self.trigger().intensity())
    }
}

/// The three intensities the stage ladder is drawn at, one per stage.
///
/// `0.0` is quiescent by definition; `0.5` sits in the middle of the band
/// between the two landmarks, where the fuel is above its specification but
/// still retaining; `1.0` is [`RELEASE_INTENSITY`], the far landmark, which is
/// the only place the release annotation is drawn.
const LADDER_INTENSITIES: [f32; 3] = [0.0, 0.5, RELEASE_INTENSITY];

/// Vertical space reserved under each card for its caption, in points.
const CAPTION_H: f32 = 64.0;

/// Gap between cards, in points.
const GAP: f32 = 14.0;

/// Height of a card as a multiple of its width, matching the reactor gallery so
/// the vessel underneath is drawn at the proportions it is used at elsewhere.
const CARD_ASPECT: f32 = 2.1;

fn degc(value: f64) -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<degree_celsius>(value)
}

/// Right-panel controls for the overlay.
pub fn controls(ui: &mut egui::Ui, state: &mut ExcursionTab) {
    ui.heading("Excursion overlay");
    ui.label(
        RichText::new(
            "The annotation a reactor gets when its fuel goes past the \
             temperature it is specified for. A warning label — not a release \
             model, not an accident analysis, and not a source term. Nothing \
             here depicts an explosion: an HTGR has no blast mechanism at these \
             conditions, and the real failure mode is progressive.",
        )
        .small()
        .weak(),
    );
    ui.separator();

    ui.label(RichText::new("Trigger").strong());
    for source in ExcursionSource::ALL {
        ui.selectable_value(&mut state.source, *source, source.label());
    }

    match state.source {
        ExcursionSource::FuelTemperature => {
            ui.add(
                egui::Slider::new(&mut state.fuel_degc, 900.0..=2100.0)
                    .text("peak fuel [°C]")
                    .fixed_decimals(0),
            );
            ui.horizontal_wrapped(|ui| {
                if ui.button("1046 (120 % overload)").clicked() {
                    state.fuel_degc = 1046.6;
                }
                if ui.button("1230 — HTR-10 limit").clicked() {
                    state.fuel_degc = HTR10_LIMIT_DEGC;
                }
                if ui.button("1231 — just over").clicked() {
                    state.fuel_degc = HTR10_LIMIT_DEGC + 1.0;
                }
                if ui.button("1250 — coating proven to").clicked() {
                    state.fuel_degc = 1250.0;
                }
                if ui.button("1599 — still retaining").clicked() {
                    state.fuel_degc = GENERIC_RETENTION_DEGC - 1.0;
                }
                if ui.button("1600 — release annotation").clicked() {
                    state.fuel_degc = GENERIC_RETENTION_DEGC;
                }
                if ui.button("1800 — SiC permeable").clicked() {
                    state.fuel_degc = SIC_PERMEABLE_DEGC;
                }
            });
            ui.label(
                RichText::new(
                    "1230 °C is the HTR-10's OWN specified maximum fuel \
                     temperature (Gao & Shi 2002), itself set from the \
                     experimental demonstration that the coating retains to \
                     1250 °C. 1600 °C is the GENERIC modular-HTR fuel \
                     temperature limit and is NOT an HTR-10 limit — the two are \
                     370 K apart, and any margin statement uses 1230.",
                )
                .small()
                .weak(),
            );
            ui.label(
                RichText::new(
                    "Between them the border deepens and NOTHING is drawn over \
                     the fuel: the heating tests found no particle failures and \
                     no noticeable caesium or strontium release in the first few \
                     hundred hours at 1600 °C (Kugeler et al. 2017, EUR 28712 \
                     EN, §4.2.1). Failures and release increase at 1700-1800 °C, \
                     where SiC becomes permeable to most fission products.",
                )
                .small()
                .weak(),
            );
        }
        ExcursionSource::Intensity => {
            ui.add(
                egui::Slider::new(&mut state.intensity, 0.0..=1.0)
                    .text("intensity")
                    .fixed_decimals(3),
            );
            ui.horizontal_wrapped(|ui| {
                if ui.button("0.00 — quiescent").clicked() {
                    state.intensity = 0.0;
                }
                if ui.button("0.50 — above the limit").clicked() {
                    state.intensity = 0.50;
                }
                if ui.button("1.00 — release").clicked() {
                    state.intensity = RELEASE_INTENSITY;
                }
            });
            ui.label(
                RichText::new(
                    "A raw intensity carries no temperatures, so the overlay \
                     prints no numbers under its banner — it will not invent a \
                     fuel temperature to display.",
                )
                .small()
                .weak(),
            );
        }
    }

    ui.separator();
    ui.label(RichText::new("Clock").strong());
    ui.horizontal(|ui| {
        if ui
            .button(if state.running {
                "⏸ pause"
            } else {
                "▶ run"
            })
            .clicked()
        {
            state.running = !state.running;
        }
        if ui.button("↺ replay").clicked() {
            state.replay();
        }
    });
    ui.label(
        RichText::new(format!(
            "The release annotation reaches its full drawn extent after \
             {RELEASE_RAMP_SECONDS:.1} s of SIMULATION time and stays there — released \
             products do not go back into the fuel. Pausing freezes it; replay rewinds it \
             without touching the trigger. That ramp is a PRESENTATION constant and implies \
             no timescale: the measured releases are quoted in hundreds of hours.",
        ))
        .small()
        .weak(),
    );

    ui.separator();
    ui.label(RichText::new("Annotation").strong());
    ui.horizontal(|ui| {
        ui.label("subject");
        ui.add(egui::TextEdit::singleline(&mut state.subject).desired_width(160.0));
    });
    ui.checkbox(&mut state.show_labels, "show banner and readouts");
    if !state.show_labels {
        ui.label(
            RichText::new(
                "The banner carries the claim that the model has left its valid \
                 range. Hiding it is for thumbnails only, never for a running \
                 simulator.",
            )
            .small()
            .weak(),
        );
    }
    ui.checkbox(&mut state.show_vessel, "draw a reactor vessel underneath");

    ui.separator();
    ui.label(RichText::new("Vessel underneath [°C]").strong());
    ui.add(egui::Slider::new(&mut state.vessel_inlet_degc, 100.0..=600.0).text("coolant inlet"));
    ui.add(egui::Slider::new(&mut state.vessel_outlet_degc, 200.0..=1000.0).text("coolant outlet"));
    ui.add(egui::Slider::new(&mut state.min_temp_degc, 0.0..=600.0).text("scale min"));
    ui.add(egui::Slider::new(&mut state.max_temp_degc, 800.0..=2500.0).text("scale max"));
    if state.max_temp_degc <= state.min_temp_degc {
        state.max_temp_degc = state.min_temp_degc + 1.0;
    }
    ui.label(
        RichText::new(
            "The vessel's core is coloured by the SAME fuel-temperature slider, \
             on the ordinary diverging temperature map. The annotation uses its \
             own hazard palette precisely so the two can never be confused.",
        )
        .small()
        .weak(),
    );

    ui.separator();
    ui.label(RichText::new("Layout").strong());
    ui.add(egui::Slider::new(&mut state.card_width, 180.0..=420.0).text("card width [pt]"));

    ui.separator();
    ui.label(RichText::new("What drives what").strong());
    let overlay = state.overlay(egui::Pos2::ZERO, Vec2::splat(100.0));
    egui::Grid::new("excursion_readout")
        .num_columns(2)
        .striped(true)
        .show(ui, |ui| {
            ui.label("stage");
            let stage = overlay.stage();
            let colour = match stage {
                ExcursionStage::Quiescent => Color32::from_rgb(140, 190, 140),
                ExcursionStage::LimitExceeded => Color32::from_rgb(240, 158, 34),
                ExcursionStage::FissionProductRelease => Color32::from_rgb(220, 80, 60),
            };
            ui.label(RichText::new(format!("{stage:?}")).color(colour).strong());
            ui.end_row();

            ui.label("intensity");
            ui.label(format!("{:.3}", overlay.intensity()));
            ui.end_row();

            ui.label("overshoot past the limit");
            match overlay.overshoot_kelvin() {
                Some(k) => ui.label(format!("{k:+.1} K")),
                None => ui.label(
                    RichText::new("n/a — no temperatures on this trigger")
                        .italics()
                        .color(Color32::from_rgb(200, 140, 60)),
                ),
            };
            ui.end_row();

            ui.label("elapsed since trigger");
            ui.label(format!(
                "{:.2} s ({})",
                state.simulation_time.get::<second>(),
                if state.running { "running" } else { "paused" }
            ));
            ui.end_row();

            ui.label("release phase");
            ui.label(format!("{:.3}", overlay.phase()));
            ui.end_row();

            ui.label("species named");
            let named = overlay.named_species();
            if named.is_empty() {
                ui.label(
                    RichText::new("none — nothing released at this stage")
                        .italics()
                        .weak(),
                );
            } else {
                ui.label(
                    named
                        .iter()
                        .map(|s| s.nuclide)
                        .collect::<Vec<_>>()
                        .join(", "),
                );
            }
            ui.end_row();

            ui.label("mark drift (100 pt box)");
            ui.label(format!(
                "{:.1} pt",
                release_reach(release_phase(state.simulation_time), 100.0)
            ));
            ui.end_row();

            ui.label("banner pulse");
            ui.label(format!("{:.2}", banner_pulse(state.simulation_time)));
            ui.end_row();

            ui.label("caption");
            ui.label(RichText::new(stage.caption()).small().weak());
            ui.end_row();
        });
}

/// Draws the driven card and the three-stage ladder.
pub fn draw(ui: &mut egui::Ui, state: &ExcursionTab) {
    ui.heading("Widget under test");
    ui.label(
        RichText::new(
            "The overlay composes over whatever the application drew — here a \
             reactor vessel, given the same centre and size. Nothing in the \
             annotation knows what is underneath it.",
        )
        .small()
        .weak(),
    );
    ui.separator();

    egui::ScrollArea::both()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let card_w = state.card_width;
            let card_h = card_w * CARD_ASPECT;

            ui.label(RichText::new("Driven by the trigger above").strong());
            ui.vertical(|ui| {
                ui.set_width(card_w);
                let (rect, _response) =
                    ui.allocate_exact_size(Vec2::new(card_w, card_h), egui::Sense::hover());
                let size = Vec2::new(card_w - 16.0, card_h - 16.0);
                if state.show_vessel {
                    ui.put(rect, state.vessel(rect.center(), size));
                }
                ui.put(rect, state.overlay(rect.center(), size));

                ui.allocate_ui(Vec2::new(card_w, CAPTION_H), |ui| {
                    let stage = state.stage();
                    ui.label(RichText::new(format!("{stage:?}")).strong());
                    ui.label(RichText::new(stage.mechanism()).small().weak());
                    ui.label(RichText::new(stage.caption()).small().weak());
                });
            });

            ui.add_space(GAP);
            ui.separator();
            ui.label(RichText::new("Stage ladder — all three at once").strong());
            ui.label(
                RichText::new(
                    "Fixed intensities, on the same clock as the card above, so \
                     the escalation can be compared rather than remembered. \
                     Quiescent draws NOTHING: a reactor inside its specification \
                     is not annotated at all. The middle card is above the limit \
                     with the fuel still retaining — a border, and nothing over \
                     the fuel. Only the last card draws release.",
                )
                .small()
                .weak(),
            );

            let small_w = (card_w * 0.62).max(140.0);
            let small_h = small_w * CARD_ASPECT;
            ui.horizontal_top(|ui| {
                for intensity in LADDER_INTENSITIES {
                    let stage = ExcursionStage::from_intensity(intensity);
                    ui.vertical(|ui| {
                        ui.set_width(small_w);
                        let (rect, _response) = ui
                            .allocate_exact_size(Vec2::new(small_w, small_h), egui::Sense::hover());
                        let size = Vec2::new(small_w - 12.0, small_h - 12.0);
                        if state.show_vessel {
                            ui.put(rect, state.vessel(rect.center(), size));
                        }
                        ui.put(
                            rect,
                            ExcursionOverlay::new(
                                ExcursionTrigger::Intensity(intensity),
                                rect.center(),
                                size,
                            )
                            .since_trigger(state.simulation_time),
                        );

                        ui.allocate_ui(Vec2::new(small_w, CAPTION_H), |ui| {
                            ui.label(RichText::new(format!("{stage:?}")).strong());
                            ui.label(
                                RichText::new(format!("intensity {intensity:.2}"))
                                    .small()
                                    .weak(),
                            );
                            if !stage.is_drawn() {
                                ui.label(
                                    RichText::new("nothing drawn — vessel untouched")
                                        .small()
                                        .italics()
                                        .weak(),
                                );
                            }
                        });
                    });
                    ui.add_space(GAP);
                }
            });

            ui.add_space(GAP);
            ui.separator();
            ui.label(
                RichText::new(
                    "Offline demonstration art. Not for nuclear facility operation, \
                     reactor control, safety-critical decision-making, emergency \
                     response, or licensing.",
                )
                .small()
                .weak(),
            );
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The fuel slider must step through all three stages, escalating at the
    /// documented landmarks and nowhere else.
    ///
    /// **Methodology.** The tab exists so the stages can be reached
    /// deliberately, so sweep [`ExcursionTab::fuel_degc`] across the slider's
    /// whole 900-2100 degC range in 1 degC steps, record every stage
    /// transition, and require: exactly two transitions; the first just above
    /// the HTR-10's own 1230 degC limit; the second **at the generic 1600 degC
    /// figure and not before**, since the heating tests show retention across
    /// the whole band between them; and all three stages to be reachable from
    /// the slider.
    ///
    /// **Result (2026-08-12):** 1 201 sampled temperatures, exactly two
    /// transitions — into `LimitExceeded` at 1231 degC (the first sample
    /// strictly above the 1230 degC limit) and into `FissionProductRelease` at
    /// 1600 degC — with all three stages reached, and every temperature between
    /// the landmarks staying `LimitExceeded`. Interpretation: every stage is
    /// reachable by dragging one slider, the escalation points are the widget's
    /// own landmarks rather than anything the studio invented, and no release
    /// is drawn where the fuel is still demonstrated to retain.
    #[test]
    fn the_fuel_slider_steps_through_all_three_stages() {
        let mut tab = ExcursionTab::default();
        tab.source = ExcursionSource::FuelTemperature;

        let mut transitions = Vec::new();
        let mut seen = Vec::new();
        tab.fuel_degc = 900.0;
        let mut previous = tab.stage();
        seen.push(previous);
        let mut sampled = 1usize;
        for step in 901..=2100 {
            tab.fuel_degc = step as f64;
            let stage = tab.stage();
            if stage != previous {
                transitions.push((tab.fuel_degc, stage));
                previous = stage;
            }
            if !seen.contains(&stage) {
                seen.push(stage);
            }
            sampled += 1;
        }
        println!("{sampled} fuel temperatures sampled; transitions at {transitions:?}");

        assert_eq!(transitions.len(), 2, "expected exactly two escalations");
        assert_eq!(transitions[0].1, ExcursionStage::LimitExceeded);
        assert!(
            (transitions[0].0 - (HTR10_LIMIT_DEGC + 1.0)).abs() < 1e-6,
            "the overlay must start just above the HTR-10's own limit"
        );
        assert_eq!(transitions[1].1, ExcursionStage::FissionProductRelease);
        assert!(
            (transitions[1].0 - GENERIC_RETENTION_DEGC).abs() <= 1.0,
            "release escalation at {} degC, expected the generic figure at {GENERIC_RETENTION_DEGC}",
            transitions[1].0
        );
        assert_eq!(seen.len(), 3, "all three stages must be reachable");

        // Nothing between the landmarks may be drawn as release.
        for step in 1..370 {
            tab.fuel_degc = HTR10_LIMIT_DEGC + step as f64;
            assert_eq!(
                tab.stage(),
                ExcursionStage::LimitExceeded,
                "{} degC must not be drawn as release",
                tab.fuel_degc
            );
        }
    }

    /// Below the HTR-10's own limit the tab must annotate nothing at all, so
    /// the studio shows a healthy reactor as healthy.
    #[test]
    fn a_reactor_inside_its_limit_is_not_annotated() {
        let mut tab = ExcursionTab::default();
        for fuel in [900.0, 1046.6, 1229.0, HTR10_LIMIT_DEGC] {
            tab.fuel_degc = fuel;
            assert_eq!(tab.stage(), ExcursionStage::Quiescent, "{fuel} degC");
            assert!(!tab.stage().is_drawn());
        }
    }

    /// Switching the trigger source must change what the overlay can report,
    /// without either source inventing the other's numbers.
    ///
    /// **Methodology.** A fuel-temperature trigger carries a temperature and a
    /// limit, so the overlay prints the overshoot; a raw intensity carries
    /// neither and must print nothing. Set the tab to 1450 degC and require an
    /// overshoot of +220 K past the 1230 degC limit; switch to the intensity
    /// source at the same slider position and require the overshoot to be
    /// `None` while the stage still follows the intensity alone.
    ///
    /// **Result (2026-08-12):** the fuel source reported +220.0 K with
    /// intensity 0.595 (220/370) and stage `LimitExceeded` — 1450 degC is above
    /// the HTR-10's own limit but well below the generic figure, so it is a
    /// warning and not release; the intensity source at 0.6 reported no
    /// overshoot and the same stage, and at 1.0 gave `FissionProductRelease`
    /// with still no numbers. Interpretation: neither source borrows the
    /// other's numbers, so a bare intensity cannot grow a fuel temperature to
    /// display.
    #[test]
    fn the_trigger_source_changes_what_can_be_reported() {
        let mut tab = ExcursionTab::default();
        tab.source = ExcursionSource::FuelTemperature;
        tab.fuel_degc = 1450.0;
        let fuel_overlay = tab.overlay(egui::Pos2::ZERO, Vec2::splat(100.0));
        let overshoot = fuel_overlay
            .overshoot_kelvin()
            .expect("a fuel trigger has numbers");
        println!(
            "fuel source: {overshoot:+.1} K over, intensity {:.3}",
            fuel_overlay.intensity()
        );
        assert!((overshoot - 220.0).abs() < 1e-6);
        assert!((fuel_overlay.intensity() - 220.0 / 370.0).abs() < 1e-3);
        assert_eq!(
            fuel_overlay.stage(),
            ExcursionStage::LimitExceeded,
            "1450 degC is above the limit but still retaining"
        );
        assert!(fuel_overlay.named_species().is_empty());

        tab.source = ExcursionSource::Intensity;
        tab.intensity = 0.6;
        let bare = tab.overlay(egui::Pos2::ZERO, Vec2::splat(100.0));
        assert_eq!(bare.overshoot_kelvin(), None);
        assert_eq!(bare.intensity(), 0.6);
        assert_eq!(bare.stage(), ExcursionStage::LimitExceeded);

        tab.intensity = RELEASE_INTENSITY;
        let full = tab.overlay(egui::Pos2::ZERO, Vec2::splat(100.0));
        assert_eq!(full.stage(), ExcursionStage::FissionProductRelease);
        assert_eq!(full.overshoot_kelvin(), None);
    }

    /// The clock must be owned by the tab, so the annotation progresses across
    /// repaints, freezes when paused, and can be replayed.
    ///
    /// **Methodology.** Widgets are rebuilt every frame, so a widget-owned
    /// clock would reset the annotation to zero each repaint. Advance
    /// [`ExcursionTab::step`] in 0.1 s increments and require the overlay's
    /// phase to follow [`release_phase`]; pause and require it to hold; replay
    /// and require it to return to the trigger instant.
    ///
    /// **Result (2026-08-12):** after 7 steps of 0.1 s the elapsed time was
    /// 0.700 s and the phase 0.500, matching `release_phase` exactly; 5 further
    /// steps while paused left both unchanged; replay gave 0.000 s and phase
    /// 0.000; running on to 2.0 s pinned the phase at 1.000 and it stayed there.
    /// Interpretation: the annotation is a pure function of the studio's own
    /// clock, so it survives being rebuilt every frame and does not fade back
    /// to nothing.
    #[test]
    fn the_tab_owns_the_clock_that_advances_the_annotation() {
        let mut tab = ExcursionTab::default();
        let phase = |t: &ExcursionTab| t.overlay(egui::Pos2::ZERO, Vec2::splat(100.0)).phase();

        for _ in 0..7 {
            tab.step(Time::new::<second>(0.1));
        }
        assert!((tab.simulation_time.get::<second>() - 0.7).abs() < 1e-9);
        assert!((phase(&tab) - 0.5).abs() < 1e-6);
        assert_eq!(phase(&tab), release_phase(tab.simulation_time));

        tab.running = false;
        for _ in 0..5 {
            tab.step(Time::new::<second>(0.1));
        }
        assert!((tab.simulation_time.get::<second>() - 0.7).abs() < 1e-9);
        assert!((phase(&tab) - 0.5).abs() < 1e-6);

        tab.replay();
        assert_eq!(tab.simulation_time, Time::ZERO);
        assert_eq!(phase(&tab), 0.0);

        tab.running = true;
        for _ in 0..20 {
            tab.step(Time::new::<second>(0.1));
        }
        assert_eq!(phase(&tab), 1.0, "the expansion must pin, not fade back");
    }

    /// The stage ladder must genuinely show one of each stage, or it is not a
    /// ladder.
    #[test]
    fn the_stage_ladder_shows_one_card_per_stage() {
        let stages: Vec<ExcursionStage> = LADDER_INTENSITIES
            .iter()
            .map(|i| ExcursionStage::from_intensity(*i))
            .collect();
        assert_eq!(
            stages,
            vec![
                ExcursionStage::Quiescent,
                ExcursionStage::LimitExceeded,
                ExcursionStage::FissionProductRelease
            ]
        );
        assert_eq!(stages.len(), ExcursionStage::ALL.len());
    }

    /// Nothing the tab puts on screen may describe an explosion.
    ///
    /// **Methodology.** The widget's own text is pinned by
    /// `excursion::no_stage_describes_an_explosion`; this pins the studio's
    /// side of it, because the tab writes captions of its own. Require every
    /// stage caption and mechanism the tab prints, and the tab's own trigger
    /// labels, to contain none of "explos", "blast", "debris", "detonat" or
    /// "shock" — except inside an explicit denial ("not a blast", "not
    /// explosive"), since saying what this is not is part of the correction.
    ///
    /// **Result (2026-08-12):** 11 strings checked across three stages and two
    /// trigger sources; the only matches were the two permitted denials, in the
    /// release stage's mechanism and caption. Interpretation: the studio cannot
    /// reintroduce the language the artwork was corrected to remove.
    #[test]
    fn the_tab_never_describes_an_explosion() {
        let forbidden = ["explos", "blast", "debris", "detonat", "shock"];
        let permitted_denials = ["not a blast", "not explosive"];
        let mut checked = 0usize;
        let mut strings: Vec<String> = Vec::new();
        for stage in ExcursionStage::ALL {
            strings.push(stage.label().to_string());
            strings.push(stage.caption().to_string());
            strings.push(stage.mechanism().to_string());
        }
        for source in ExcursionSource::ALL {
            strings.push(source.label().to_string());
        }
        for text in &strings {
            let lowered = text.to_lowercase();
            for term in forbidden {
                if !lowered.contains(term) {
                    continue;
                }
                let denied = permitted_denials.iter().any(|d| lowered.contains(d));
                assert!(denied, "studio text mentions '{term}': {text:?}");
            }
            checked += 1;
        }
        println!("{checked} studio strings checked for explosion language");
    }

    /// An empty subject must leave the headline alone rather than printing a
    /// stray separator.
    #[test]
    fn an_empty_subject_is_dropped() {
        let mut tab = ExcursionTab::default();
        tab.subject = "   ".to_string();
        // Nothing to assert on the drawing from a test, but the builder must
        // not panic and the trigger must survive the branch.
        let overlay = tab.overlay(egui::Pos2::ZERO, Vec2::splat(100.0));
        assert_eq!(overlay.stage(), tab.stage());
        assert_eq!(overlay.size(), Vec2::splat(100.0));
    }
}
