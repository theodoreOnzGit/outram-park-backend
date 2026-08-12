//! Heat-exchanger tab: **both flow arrangements**, and **both state paths**,
//! side by side.
//!
//! The gallery draws four cards in two rows:
//!
//! - **Row 1 — state-driven** ([`HeatExchangerVisual::from_scalars`]): the two
//!   arrangements, at the selected construction, fed with the caller's own inlet
//!   and outlet temperatures for both streams. Each stream is graded along its
//!   own path, the arrows point the way it really flows, and the terminal
//!   approaches are bracketed and labelled at each end.
//! - **Row 2 — physics-backed** ([`HeatExchangerVisual::new`]): the same widget
//!   wrapped around a real [`tampines::components::HeatExchanger`], drawn once
//!   per construction. That component holds a flow arrangement, a heat-transfer
//!   area and an overall coefficient — and no fluid state at all.
//!
//! **Row 2 is a sharper statement here than on the condenser tab, because this
//! component is not stateless.** A condenser stores only set-points, so its
//! neutral card is neutral about everything. A heat exchanger stores its
//! *arrangement*, which is precisely the variable this artwork is built around —
//! so row 2 draws its **real flow directions, real arrows and real nozzle
//! positions**, labels the area and `U` it really holds, and paints no
//! temperature anywhere. Flip the arrangement selector under "Physics component"
//! and row 2 re-plumbs itself; move any temperature slider and nothing on row 2
//! changes at all. That pair of behaviours is the honesty rule stated as a
//! demonstration rather than as a comment.
//!
//! # The lesson this tab exists to teach
//!
//! Drag the **cold outlet** above the **hot outlet**. In counter-flow that is a
//! *temperature cross* — a legitimate operating point, and the headline reason
//! counter-flow is chosen — and the card says so. In parallel flow the same
//! numbers are impossible: both streams start at the same end, so the cold
//! stream can only ever approach the hot stream's outlet from below, and the
//! card is tagged "impossible for this arrangement" with the offending approach
//! shown negative. The two cards sit side by side under one set of sliders, so
//! the difference is a drag away rather than a paragraph.
//!
//! That verdict is a **sign check on the numbers you set**, not a rating: no
//! duty, effectiveness or outlet temperature is computed anywhere in this crate.
//! The rating algebra lives in
//! [`outram_park_fork_dwsim_libs::heat_exchanger`], and this tab's readout calls
//! its `lmtd` directly so the log-mean quoted beside the cards is the library's
//! own number rather than a second implementation that could drift.
//!
//! **Offline demonstration art.** The operating point is a round illustrative
//! set of numbers chosen to exercise the drawing; nothing here is dimensioned
//! from, or represents, any specific heat exchanger. Per `RESPONSIBLE_USE.md`
//! this is for education, research and V&V only.

use egui::{Color32, RichText, Vec2};
use outram_park_digital_twin_engine::components::heat_exchanger::{
    approach_verdict, terminal_approaches, ApproachVerdict, HeatExchangerConstruction,
    HeatExchangerDisplayRange, HeatExchangerKind, HeatExchangerScalars, HeatExchangerVisual,
};
use outram_park_fork_dwsim_libs::heat_exchanger::lmtd::lmtd;
use tampines::components::HeatExchanger;
use uom::si::area::square_meter;
use uom::si::f64::{Area, HeatTransfer, Power, ThermodynamicTemperature};
use uom::si::heat_transfer::watt_per_square_meter_kelvin;
use uom::si::power::kilowatt;
use uom::si::temperature_interval::kelvin as kelvin_interval;
use uom::si::thermodynamic_temperature::degree_celsius;

/// Studio state for the heat-exchanger gallery.
pub struct HeatExchangerTab {
    /// Hot stream entering the exchanger, degrees Celsius. Always drawn at the
    /// left end, whatever the arrangement.
    pub hot_inlet_degc: f64,
    /// Hot stream leaving the exchanger at the right end, degrees Celsius.
    pub hot_outlet_degc: f64,
    /// Cold stream entering the exchanger, degrees Celsius. Which **end** that
    /// is depends on the arrangement — the right end in counter-flow, the left
    /// end in parallel flow.
    pub cold_inlet_degc: f64,
    /// Cold stream leaving the exchanger, degrees Celsius. Push this above
    /// [`Self::hot_outlet_degc`] to produce a temperature cross.
    pub cold_outlet_degc: f64,
    /// Whether the caller's model has a heat duty at all.
    ///
    /// Unchecking it passes `None` to the widget, which then draws no duty
    /// label — a duty needs mass flows and heat capacities the widget does not
    /// have, so it is never derived from the four temperatures.
    pub duty_known: bool,
    /// Heat duty transferred between the streams, kilowatts.
    pub duty_kw: f64,
    /// Cold end of the colour scale, degrees Celsius.
    pub min_temp_degc: f64,
    /// Hot end of the colour scale, degrees Celsius.
    pub max_temp_degc: f64,
    /// Which construction the state-driven row is drawn in. Mechanical only —
    /// it changes what the inside of the body looks like, never the physics.
    pub construction: HeatExchangerConstruction,
    /// The **arrangement the wrapped physics component actually stores**.
    ///
    /// This drives row 2's drawn flow directions, because
    /// `HeatExchangerVisual::new` reads it off the component rather than
    /// defaulting. It is the one control on this tab that changes the neutral
    /// row.
    pub physics_kind: HeatExchangerKind,
    /// Heat-transfer area of the wrapped physics component, square metres.
    pub area_m2: f64,
    /// Overall heat-transfer coefficient `U` of the wrapped physics component,
    /// watts per square metre kelvin.
    pub overall_coefficient_w_m2k: f64,
    /// Width of one gallery card, in points. Height follows from the artwork's
    /// own aspect ratio.
    pub card_width: f32,
    /// Whether to draw the internal component labels.
    pub show_labels: bool,
}

impl Default for HeatExchangerTab {
    /// Defaults sit at a liquid-liquid recuperator operating point: hot stream
    /// cooling 180 -> 95 degC against a cold stream heating 40 -> 110 degC, 1.2
    /// MW, on 240 m² at 850 W/(m²·K).
    ///
    /// **Deliberately a crossed operating point.** The cold outlet (110 degC) is
    /// above the hot outlet (95 degC), so the tab opens showing the counter-flow
    /// card doing something the parallel-flow card beside it reports as
    /// impossible — which is the whole reason this tab exists. Drop the cold
    /// outlet below 95 degC to see both cards agree.
    ///
    /// The colour scale spans 20-220 degC so the exchanger's own 140 K range
    /// fills most of the map; a plant-wide scale would render the whole machine
    /// in one pale band, which is correct but useless for checking the artwork.
    fn default() -> Self {
        Self {
            hot_inlet_degc: 180.0,
            hot_outlet_degc: 95.0,
            cold_inlet_degc: 40.0,
            cold_outlet_degc: 110.0,
            duty_known: true,
            duty_kw: 1200.0,
            min_temp_degc: 20.0,
            max_temp_degc: 220.0,
            construction: HeatExchangerConstruction::ShellAndTube,
            physics_kind: HeatExchangerKind::CounterFlow,
            area_m2: 240.0,
            overall_coefficient_w_m2k: 850.0,
            card_width: 340.0,
            show_labels: true,
        }
    }
}

impl HeatExchangerTab {
    /// The scalars every state-driven card is drawn from.
    pub fn scalars(&self) -> HeatExchangerScalars {
        HeatExchangerScalars {
            hot_inlet_temp: degc(self.hot_inlet_degc),
            hot_outlet_temp: degc(self.hot_outlet_degc),
            cold_inlet_temp: degc(self.cold_inlet_degc),
            cold_outlet_temp: degc(self.cold_outlet_degc),
            duty: self
                .duty_known
                .then(|| Power::new::<kilowatt>(self.duty_kw)),
        }
    }

    /// The display range the state-driven cards are graded against.
    pub fn range(&self) -> HeatExchangerDisplayRange {
        HeatExchangerDisplayRange {
            min_temp: degc(self.min_temp_degc),
            max_temp: degc(self.max_temp_degc),
        }
    }

    /// The physics component the neutral cards wrap.
    ///
    /// Real stored state — a flow arrangement, a heat-transfer area and an
    /// overall coefficient — and nothing else. `HeatExchanger::calculate` is not
    /// implemented, so there is no fluid state behind it.
    pub fn physics(&self) -> HeatExchanger {
        HeatExchanger::new(
            self.physics_kind.arrangement(),
            Area::new::<square_meter>(self.area_m2),
            HeatTransfer::new::<watt_per_square_meter_kelvin>(self.overall_coefficient_w_m2k),
        )
    }

    /// What the current sliders imply for `kind` — see
    /// [`outram_park_digital_twin_engine::components::heat_exchanger::approach_verdict`].
    pub fn verdict(&self, kind: HeatExchangerKind) -> ApproachVerdict {
        let s = self.scalars();
        approach_verdict(
            kind,
            s.hot_inlet_temp,
            s.hot_outlet_temp,
            s.cold_inlet_temp,
            s.cold_outlet_temp,
        )
    }

    /// The two terminal approaches for `kind`, in kelvin, as `(left, right)`.
    pub fn approaches_kelvin(&self, kind: HeatExchangerKind) -> (f64, f64) {
        let s = self.scalars();
        let (l, r) = terminal_approaches(
            kind,
            s.hot_inlet_temp,
            s.hot_outlet_temp,
            s.cold_inlet_temp,
            s.cold_outlet_temp,
        );
        (l.get::<kelvin_interval>(), r.get::<kelvin_interval>())
    }

    /// A state-driven card for `kind`, at the given screen box.
    ///
    /// Shared by [`draw`] and the readout so what is reported and what is drawn
    /// cannot drift apart.
    pub fn driven(
        &self,
        kind: HeatExchangerKind,
        centre: egui::Pos2,
        size: Vec2,
    ) -> HeatExchangerVisual {
        let visual =
            HeatExchangerVisual::from_scalars(kind, centre, size, self.range(), self.scalars())
                .with_construction(self.construction);
        if self.show_labels {
            visual
        } else {
            visual.without_labels()
        }
    }

    /// A physics-backed card at the given construction and screen box.
    ///
    /// Built through the preserved three-argument [`HeatExchangerVisual::new`],
    /// so this is exactly what any existing call site gets. Note that **no kind
    /// is passed** — the arrangement comes off the component itself.
    pub fn neutral(
        &self,
        construction: HeatExchangerConstruction,
        centre: egui::Pos2,
        size: Vec2,
    ) -> HeatExchangerVisual {
        let visual =
            HeatExchangerVisual::new(self.physics(), centre, size).with_construction(construction);
        if self.show_labels {
            visual
        } else {
            visual.without_labels()
        }
    }
}

/// Vertical space reserved under each card for its caption, in points.
const CAPTION_H: f32 = 84.0;

/// Gap between cards, in points.
const GAP: f32 = 14.0;

fn degc(value: f64) -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<degree_celsius>(value)
}

/// How a verdict reads in the readout, and in what colour.
fn verdict_text(verdict: ApproachVerdict) -> (String, Color32) {
    match verdict {
        ApproachVerdict::Feasible => (
            "feasible — both approaches positive".to_string(),
            Color32::from_rgb(150, 200, 150),
        ),
        ApproachVerdict::TemperatureCross => (
            "temperature cross — cold out above hot out".to_string(),
            Color32::from_rgb(150, 190, 230),
        ),
        ApproachVerdict::Impossible => (
            "IMPOSSIBLE for this arrangement".to_string(),
            Color32::from_rgb(200, 140, 60),
        ),
    }
}

/// Right-panel controls for the gallery.
pub fn controls(ui: &mut egui::Ui, state: &mut HeatExchangerTab) {
    ui.heading("Heat exchangers");
    ui.label(
        RichText::new(
            "Two flow arrangements, drawn from two different state sources. \
             Illustrative schematic art — not a validated model and not any \
             specific heat-exchanger design.",
        )
        .small()
        .weak(),
    );
    ui.separator();

    ui.label(RichText::new("Hot stream [°C]").strong());
    ui.add(egui::Slider::new(&mut state.hot_inlet_degc, 20.0..=350.0).text("inlet"));
    ui.add(egui::Slider::new(&mut state.hot_outlet_degc, 10.0..=350.0).text("outlet"));
    ui.label(
        RichText::new(
            "The hot stream is always drawn left to right, so the left end of \
             every card is its inlet. Its band grades along its own path.",
        )
        .small()
        .weak(),
    );

    ui.separator();
    ui.label(RichText::new("Cold stream [°C]").strong());
    ui.add(egui::Slider::new(&mut state.cold_inlet_degc, 0.0..=300.0).text("inlet"));
    ui.add(egui::Slider::new(&mut state.cold_outlet_degc, 5.0..=340.0).text("outlet"));
    ui.label(
        RichText::new(
            "Push the cold OUTLET above the hot outlet to force a temperature \
             cross. Counter-flow will draw it; parallel flow will refuse it.",
        )
        .small()
        .weak(),
    );

    ui.separator();
    ui.label(RichText::new("Duty").strong());
    ui.checkbox(&mut state.duty_known, "duty is known");
    ui.add_enabled(
        state.duty_known,
        egui::Slider::new(&mut state.duty_kw, 0.0..=20000.0).text("Q [kW]"),
    );
    if !state.duty_known {
        ui.label(
            RichText::new(
                "Unknown → no duty label. A duty needs mass flows and heat \
                 capacities the widget does not have, so it is never derived \
                 from the four temperatures.",
            )
            .small()
            .weak(),
        );
    }

    ui.separator();
    ui.label(RichText::new("Colour scale [°C]").strong());
    ui.label(
        RichText::new(
            "Diverging map: blue at min, neutral white at the MIDPOINT, red at \
             max. Both streams use it — a recuperator carries no steam quality, \
             so unlike the condenser there is only one colour axis here.",
        )
        .small()
        .weak(),
    );
    ui.add(egui::Slider::new(&mut state.min_temp_degc, -20.0..=200.0).text("min"));
    ui.add(egui::Slider::new(&mut state.max_temp_degc, 0.0..=600.0).text("max"));
    if state.max_temp_degc <= state.min_temp_degc {
        state.max_temp_degc = state.min_temp_degc + 1.0;
    }

    ui.separator();
    ui.label(RichText::new("Construction (row 1)").strong());
    ui.horizontal_wrapped(|ui| {
        for construction in HeatExchangerConstruction::ALL {
            ui.selectable_value(&mut state.construction, *construction, construction.label());
        }
    });
    ui.label(
        RichText::new(format!(
            "{} — {}",
            state.construction.hot_stream_location(),
            state.construction.description()
        ))
        .small()
        .weak(),
    );
    ui.label(
        RichText::new(
            "Mechanical axis only. Every construction can be plumbed either way \
             round, which is why it is a separate enum from the arrangement.",
        )
        .small()
        .weak(),
    );

    ui.separator();
    ui.label(RichText::new("Physics component (row 2)").strong());
    ui.horizontal_wrapped(|ui| {
        for kind in HeatExchangerKind::ALL {
            ui.selectable_value(&mut state.physics_kind, *kind, kind.label());
        }
    });
    ui.add(egui::Slider::new(&mut state.area_m2, 1.0..=2000.0).text("area A [m²]"));
    ui.add(
        egui::Slider::new(&mut state.overall_coefficient_w_m2k, 50.0..=6000.0)
            .text("coefficient U [W/(m²·K)]"),
    );
    ui.label(
        RichText::new(
            "These three are the ONLY things tampines::components::HeatExchanger \
             stores, and all three are real — so row 2 draws the arrangement's \
             true flow directions and labels A and U. Flip the arrangement and \
             row 2 re-plumbs; move any temperature slider and row 2 does not \
             change at all, because HeatExchanger::calculate is not implemented \
             and there is no fluid state to see.",
        )
        .small()
        .weak(),
    );

    ui.separator();
    ui.label(RichText::new("Layout").strong());
    ui.add(egui::Slider::new(&mut state.card_width, 220.0..=560.0).text("card width [pt]"));
    ui.checkbox(&mut state.show_labels, "show internal labels");

    ui.separator();
    ui.label(RichText::new("What drives what").strong());
    egui::Grid::new("heat_exchanger_readout")
        .num_columns(2)
        .striped(true)
        .show(ui, |ui| {
            ui.label("hot stream band");
            ui.label(format!(
                "{:.1} → {:.1} °C ({:+.1} K drop)",
                state.hot_inlet_degc,
                state.hot_outlet_degc,
                state.hot_inlet_degc - state.hot_outlet_degc
            ));
            ui.end_row();

            ui.label("cold stream band");
            ui.label(format!(
                "{:.1} → {:.1} °C ({:+.1} K rise)",
                state.cold_inlet_degc,
                state.cold_outlet_degc,
                state.cold_outlet_degc - state.cold_inlet_degc
            ));
            ui.end_row();

            for kind in HeatExchangerKind::ALL {
                let (left, right) = state.approaches_kelvin(*kind);
                ui.label(format!("{} — end brackets", kind.label()));
                ui.label(format!("ΔT {left:.1} K (left), {right:.1} K (right)"));
                ui.end_row();

                ui.label(format!("{} — verdict", kind.label()));
                let (text, colour) = verdict_text(state.verdict(*kind));
                ui.colored_label(colour, text);
                ui.end_row();

                // The log-mean, from the LIBRARY's own formula rather than a
                // second implementation here. Only real when both ends are
                // positive, which is exactly what the verdict above reports.
                ui.label(format!("{} — LMTD", kind.label()));
                if left > 0.0 && right > 0.0 {
                    let s = state.scalars();
                    let value = lmtd(
                        kind.arrangement(),
                        s.hot_inlet_temp,
                        s.hot_outlet_temp,
                        s.cold_inlet_temp,
                        s.cold_outlet_temp,
                    )
                    .get::<kelvin_interval>();
                    ui.label(format!("{value:.2} K (dwsim-libs lmtd)"));
                } else {
                    ui.label(
                        RichText::new("undefined — an approach is not positive")
                            .italics()
                            .color(Color32::from_rgb(200, 140, 60)),
                    );
                }
                ui.end_row();
            }

            ui.label("duty label");
            match state.scalars().duty {
                Some(q) => ui.label(format!("{:.0} kW (supplied)", q.get::<kilowatt>())),
                None => ui.label(
                    RichText::new("not known — no duty drawn")
                        .italics()
                        .color(Color32::from_rgb(200, 140, 60)),
                ),
            };
            ui.end_row();

            let neutral = state.neutral(
                HeatExchangerConstruction::ShellAndTube,
                egui::Pos2::ZERO,
                Vec2::splat(1.0),
            );

            ui.label("row 2: scalars()");
            ui.label(
                RichText::new("None — no fluid state on the component")
                    .italics()
                    .color(Color32::from_rgb(200, 140, 60)),
            );
            ui.end_row();

            ui.label("row 2: drawn arrangement");
            ui.label(format!(
                "{} — read off the component",
                neutral.kind().label()
            ));
            ui.end_row();

            ui.label("row 2: A and U");
            ui.label(
                match (neutral.heat_transfer_area(), neutral.overall_coefficient()) {
                    (Some(a), Some(u)) => format!(
                        "{:.0} m² · {:.0} W/(m²·K) (real, labelled)",
                        a.get::<square_meter>(),
                        u.get::<watt_per_square_meter_kelvin>()
                    ),
                    _ => "not known".to_string(),
                },
            );
            ui.end_row();

            ui.label("row 2: verdict");
            ui.label(
                RichText::new("None — no temperatures to judge")
                    .italics()
                    .color(Color32::from_rgb(200, 140, 60)),
            );
            ui.end_row();
        });
}

/// Draws the state-driven row and the physics-backed row.
pub fn draw(ui: &mut egui::Ui, state: &HeatExchangerTab) {
    ui.heading("Widget under test");
    ui.label(
        RichText::new(
            "Hot stream left to right through the body; the cold stream runs \
             with it or against it. Watch the arrows, the nozzle ends and the \
             profile strip change together when the arrangement changes.",
        )
        .small()
        .weak(),
    );
    ui.separator();

    egui::ScrollArea::both()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.label(RichText::new("State-driven — HeatExchangerVisual::from_scalars").strong());
            ui.label(
                RichText::new(
                    "Real state from the caller's own model: both streams' inlet \
                     and outlet temperatures, and optionally a duty. The two \
                     cards share one operating point and differ only in how the \
                     streams are plumbed.",
                )
                .small()
                .weak(),
            );
            driven_row(ui, state);

            ui.add_space(GAP);
            ui.separator();
            ui.label(RichText::new("Physics-backed — HeatExchangerVisual::new").strong());
            ui.label(
                RichText::new(
                    "Wrapping a tampines::components::HeatExchanger, which holds \
                     an arrangement, an area and an overall coefficient — all \
                     real, all drawn or labelled — and no fluid state. \
                     HeatExchanger::calculate is not implemented, so both streams \
                     are neutral grey with no approaches and no profile. The \
                     geometry is honest; only the colour is withheld.",
                )
                .small()
                .weak(),
            );
            neutral_row(ui, state);

            ui.add_space(GAP);
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

/// Draws the state-driven row — one card per [`HeatExchangerKind`], both at the
/// selected construction, so the only difference on screen is the arrangement.
fn driven_row(ui: &mut egui::Ui, state: &HeatExchangerTab) {
    let card_w = state.card_width;
    let card_h = card_w / state.construction.native_aspect_ratio();

    ui.horizontal_top(|ui| {
        for kind in HeatExchangerKind::ALL {
            ui.vertical(|ui| {
                ui.set_width(card_w);
                let (rect, _response) =
                    ui.allocate_exact_size(Vec2::new(card_w, card_h), egui::Sense::hover());
                let size = Vec2::new(card_w - 16.0, card_h - 16.0);
                ui.put(rect, state.driven(*kind, rect.center(), size));

                ui.allocate_ui(Vec2::new(card_w, CAPTION_H), |ui| {
                    ui.label(RichText::new(kind.label()).strong());
                    ui.label(RichText::new(kind.description()).small().weak());
                    ui.label(
                        RichText::new(format!("cold stream: {}", kind.cold_stream_path()))
                            .small()
                            .weak(),
                    );
                    let (text, colour) = verdict_text(state.verdict(*kind));
                    ui.label(RichText::new(text).small().color(colour));
                });
            });
            ui.add_space(GAP);
        }
    });
}

/// Draws the physics-backed row — one card per
/// [`HeatExchangerConstruction`], both taking their arrangement from the
/// component itself rather than from an argument.
fn neutral_row(ui: &mut egui::Ui, state: &HeatExchangerTab) {
    let card_w = state.card_width;

    ui.horizontal_top(|ui| {
        for construction in HeatExchangerConstruction::ALL {
            let card_h = card_w / construction.native_aspect_ratio();
            ui.vertical(|ui| {
                ui.set_width(card_w);
                let (rect, _response) =
                    ui.allocate_exact_size(Vec2::new(card_w, card_h), egui::Sense::hover());
                let size = Vec2::new(card_w - 16.0, card_h - 16.0);
                let card = state.neutral(*construction, rect.center(), size);
                let drawn_kind = card.kind();
                ui.put(rect, card);

                ui.allocate_ui(Vec2::new(card_w, CAPTION_H), |ui| {
                    ui.label(RichText::new(construction.label()).strong());
                    ui.label(RichText::new(construction.description()).small().weak());
                    ui.label(
                        RichText::new(format!(
                            "arrangement: {} — from the component",
                            drawn_kind.label()
                        ))
                        .small()
                        .weak(),
                    );
                    ui.label(
                        RichText::new("no fluid state — nothing fabricated")
                            .small()
                            .italics()
                            .color(Color32::from_rgb(200, 140, 60)),
                    );
                });
            });
            ui.add_space(GAP);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The physics-backed row must stay temperature-blind however far the
    /// state-driven sliders are moved — that is the whole reason it is on
    /// screen.
    ///
    /// **Methodology.** Build the tab, then sweep every state-driven control
    /// across its slider range (both streams' inlet and outlet temperatures, the
    /// duty, and both ends of the colour scale) and, at every combination
    /// sampled, require the card built by [`HeatExchangerTab::neutral`] — i.e.
    /// `HeatExchangerVisual::new` — to report no scalars, no approaches, no
    /// verdict and no duty. Then require it to report the area and overall
    /// coefficient it really holds, so the neutral path is neutral without being
    /// empty.
    ///
    /// **Result (2026-08-12):** 1 452 slider combinations sampled, every one
    /// giving `scalars() == None`, `approaches() == None`, `verdict() == None`
    /// and `duty() == None`; the area came back as the 240 m² set on the slider
    /// and the coefficient as 850 W/(m²·K). Interpretation: no state-driven
    /// control can leak into the physics-backed rendering, so row 2 cannot
    /// quietly start painting a temperature.
    #[test]
    fn the_physics_row_never_picks_up_the_state_driven_sliders() {
        let mut tab = HeatExchangerTab::default();
        let mut sampled = 0usize;
        for hot_step in 0..=10 {
            tab.hot_inlet_degc = 20.0 + hot_step as f64 * 33.0;
            tab.hot_outlet_degc = tab.hot_inlet_degc - 40.0;
            for cold_step in 0..=10 {
                tab.cold_inlet_degc = cold_step as f64 * 25.0;
                tab.cold_outlet_degc = tab.cold_inlet_degc + 60.0;
                for duty_step in 0..=5 {
                    tab.duty_kw = duty_step as f64 * 2000.0;
                    tab.min_temp_degc = -20.0 + duty_step as f64 * 10.0;
                    tab.max_temp_degc = tab.min_temp_degc + 400.0;
                    for construction in HeatExchangerConstruction::ALL {
                        let neutral =
                            tab.neutral(*construction, egui::Pos2::ZERO, Vec2::splat(100.0));
                        assert!(
                            neutral.scalars().is_none(),
                            "the physics path picked up state at hot in {}",
                            tab.hot_inlet_degc
                        );
                        assert!(neutral.approaches().is_none());
                        assert!(neutral.verdict().is_none());
                        assert!(neutral.duty().is_none());
                        sampled += 1;
                    }
                }
            }
        }
        println!("{sampled} slider combinations sampled");

        let tab = HeatExchangerTab::default();
        let neutral = tab.neutral(
            HeatExchangerConstruction::ShellAndTube,
            egui::Pos2::ZERO,
            Vec2::splat(100.0),
        );
        assert_eq!(
            neutral
                .heat_transfer_area()
                .map(|a| a.get::<square_meter>()),
            Some(240.0)
        );
        assert_eq!(
            neutral
                .overall_coefficient()
                .map(|u| u.get::<watt_per_square_meter_kelvin>()),
            Some(850.0)
        );
    }

    /// The physics-backed row **must** follow the arrangement the component
    /// really stores — this is the half of the honesty rule that says a widget
    /// may not ignore state it does have.
    ///
    /// **Methodology.** Flip [`HeatExchangerTab::physics_kind`] between both
    /// arrangements and, for each construction, require the card built by
    /// `HeatExchangerVisual::new` to report that same arrangement back through
    /// `kind()` — with no kind ever passed as an argument. Then require the
    /// arrangement to survive the round trip into the physics enum the component
    /// actually stores.
    ///
    /// **Result (2026-08-12):** 4 cards checked (2 arrangements x 2
    /// constructions); a `CounterFlow` component drew `CounterFlow` and a
    /// `ParallelFlow` component drew `ParallelFlow` in both constructions, and
    /// the stored `arrangement` round-tripped for both. Interpretation: the
    /// neutral card is neutral about *fluid state only* — it does not throw away
    /// the arrangement, which is the one thing about a recuperator this
    /// component genuinely knows.
    #[test]
    fn the_physics_row_follows_the_components_own_arrangement() {
        let mut tab = HeatExchangerTab::default();
        let mut checked = 0usize;
        for kind in HeatExchangerKind::ALL {
            tab.physics_kind = *kind;
            assert_eq!(tab.physics().arrangement, kind.arrangement());
            for construction in HeatExchangerConstruction::ALL {
                let card = tab.neutral(*construction, egui::Pos2::ZERO, Vec2::splat(100.0));
                assert_eq!(
                    card.kind(),
                    *kind,
                    "row 2 ignored the component's own arrangement"
                );
                assert_eq!(card.construction(), *construction);
                checked += 1;
            }
        }
        println!("{checked} physics-backed cards checked");
        assert_eq!(checked, 4);
    }

    /// The state-driven row must hand the widget exactly what the sliders say,
    /// with nothing substituted on the way.
    #[test]
    fn the_state_driven_row_passes_the_sliders_through_unchanged() {
        let mut tab = HeatExchangerTab::default();
        tab.hot_inlet_degc = 205.0;
        tab.hot_outlet_degc = 118.0;
        tab.cold_inlet_degc = 33.0;
        tab.cold_outlet_degc = 141.0;
        tab.duty_kw = 3400.0;

        for kind in HeatExchangerKind::ALL {
            let drawn = tab
                .driven(*kind, egui::Pos2::ZERO, Vec2::splat(100.0))
                .scalars()
                .expect("the state-driven card carries scalars");
            assert_eq!(drawn, tab.scalars());
            assert_eq!(drawn.duty, Some(Power::new::<kilowatt>(3400.0)));
            assert_eq!(drawn.hot_inlet_temp.get::<degree_celsius>().round(), 205.0);
        }

        // Unchecking "duty is known" must reach the widget as a genuine absence.
        tab.duty_known = false;
        assert!(tab.scalars().duty.is_none());
        assert!(tab
            .driven(
                HeatExchangerKind::CounterFlow,
                egui::Pos2::ZERO,
                Vec2::splat(100.0)
            )
            .duty()
            .is_none());
    }

    /// **The lesson the tab exists to teach must actually hold at the default
    /// operating point, and must be reachable by dragging one slider.**
    ///
    /// **Methodology.** At the default operating point (hot 180 -> 95 degC, cold
    /// 40 -> 110 degC — a crossed pair), require the counter-flow card to report
    /// [`ApproachVerdict::TemperatureCross`] and the parallel-flow card beside
    /// it to report [`ApproachVerdict::Impossible`], so the two disagree on
    /// screen from the moment the tab opens. Then drag the cold outlet down
    /// below the hot outlet (to 90 degC) and require both to report
    /// [`ApproachVerdict::Feasible`], so the disagreement is a property of the
    /// numbers and not a permanent fixture. Finally sweep the cold outlet across
    /// its whole slider range and require parallel flow to *never* report a
    /// temperature cross.
    ///
    /// **Result (2026-08-12):** at the default point counter-flow reported
    /// `TemperatureCross` with approaches 70.0 K and 55.0 K, and parallel flow
    /// reported `Impossible` with approaches 140.0 K and -15.0 K — the negative
    /// right-hand approach being exactly the end where the streams would have to
    /// cross. With the cold outlet at 90 degC both reported `Feasible`
    /// (counter-flow 90.0 / 55.0 K, parallel flow 140.0 / 5.0 K). Over 296
    /// sampled cold-outlet positions (1 to 336 degC in 1 K steps, keeping only
    /// those above the 40 degC cold inlet), parallel flow reported
    /// `TemperatureCross` **0** times and counter-flow reported it 84 times.
    /// Interpretation: the tab demonstrates the arrangement's real capability
    /// rather than asserting it in a caption.
    #[test]
    fn parallel_flow_refuses_the_cross_that_counter_flow_draws() {
        let mut tab = HeatExchangerTab::default();
        assert_eq!(
            tab.verdict(HeatExchangerKind::CounterFlow),
            ApproachVerdict::TemperatureCross
        );
        assert_eq!(
            tab.verdict(HeatExchangerKind::ParallelFlow),
            ApproachVerdict::Impossible
        );
        let (cl, cr) = tab.approaches_kelvin(HeatExchangerKind::CounterFlow);
        let (pl, pr) = tab.approaches_kelvin(HeatExchangerKind::ParallelFlow);
        println!("default: counter {cl:.1}/{cr:.1} K, parallel {pl:.1}/{pr:.1} K");
        assert!(
            pr < 0.0,
            "the parallel-flow right-hand approach must go negative"
        );

        tab.cold_outlet_degc = 90.0;
        let (cl, cr) = tab.approaches_kelvin(HeatExchangerKind::CounterFlow);
        let (pl, pr) = tab.approaches_kelvin(HeatExchangerKind::ParallelFlow);
        println!("uncrossed: counter {cl:.1}/{cr:.1} K, parallel {pl:.1}/{pr:.1} K");
        assert_eq!(
            tab.verdict(HeatExchangerKind::CounterFlow),
            ApproachVerdict::Feasible
        );
        assert_eq!(
            tab.verdict(HeatExchangerKind::ParallelFlow),
            ApproachVerdict::Feasible
        );

        let mut parallel_crosses = 0usize;
        let mut counter_crosses = 0usize;
        let mut sampled = 0usize;
        for step in 1..=336 {
            tab.cold_outlet_degc = step as f64;
            if tab.cold_outlet_degc <= tab.cold_inlet_degc {
                continue;
            }
            sampled += 1;
            if tab.verdict(HeatExchangerKind::ParallelFlow) == ApproachVerdict::TemperatureCross {
                parallel_crosses += 1;
            }
            if tab.verdict(HeatExchangerKind::CounterFlow) == ApproachVerdict::TemperatureCross {
                counter_crosses += 1;
            }
        }
        println!(
            "{sampled} cold-outlet positions: parallel {parallel_crosses} crosses, \
             counter {counter_crosses} crosses"
        );
        assert_eq!(parallel_crosses, 0);
        assert!(counter_crosses > 0);
    }

    /// Every card in the gallery must be given a box already at the artwork's
    /// own proportions, so nothing is letterboxed into a band of dead space.
    #[test]
    fn the_cards_are_sized_from_the_artworks_own_aspect_ratio() {
        let tab = HeatExchangerTab::default();
        for construction in HeatExchangerConstruction::ALL {
            let card_h = tab.card_width / construction.native_aspect_ratio();
            let fitted = construction.fit_native_aspect(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                Vec2::new(tab.card_width, card_h),
            ));
            assert!((fitted.width() - tab.card_width).abs() < 1e-3);
            assert!((fitted.height() - card_h).abs() < 1e-3);
        }
    }
}
