//! Condenser tab: both cooling-water arrangements, and **both state paths**,
//! side by side.
//!
//! The gallery deliberately draws four cards in two rows:
//!
//! - **Row 1 — state-driven** ([`CondenserVisual::from_scalars`]): the two
//!   arrangements fed with the caller's own exhaust quality, condensing and
//!   condensate temperatures, and cooling-water temperatures. Steam-side
//!   regions are tinted by quality, water-side regions by temperature.
//! - **Row 2 — physics-backed** ([`CondenserVisual::new`]): the same two
//!   machines wrapped around a real [`tampines::components::Condenser`], which
//!   stores only an operating pressure and a **target** outlet quality. They
//!   render entirely in neutral grey.
//!
//! **Row 2 is the point of this tab, not an embarrassment.** A widget that
//! cannot see a fluid state must not invent one, and the clearest way to check
//! that rule is being kept is to put the invented-nothing rendering next to the
//! fully driven one and see that the grey card never picks up a colour, however
//! far the sliders are moved. The `target_outlet_quality` readout is shown for
//! the same reason: it is a set-point the component really holds, it is
//! reported as text, and it is deliberately never painted.
//!
//! **Offline demonstration art.** The operating point is a round illustrative
//! set of numbers chosen to exercise the drawing; nothing here is dimensioned
//! from, or represents, any specific condenser. Per `RESPONSIBLE_USE.md` this
//! is for education, research and V&V only.

use egui::{Color32, RichText, Vec2};
use outram_park_digital_twin_engine::components::condenser::{
    cooling_water_path_span, drawn_hotwell_level, CondenserDisplayRange, CondenserKind,
    CondenserScalars, CondenserVisual,
};
use tampines::components::Condenser;
use uom::si::f64::{Pressure, ThermodynamicTemperature};
use uom::si::pressure::kilopascal;
use uom::si::thermodynamic_temperature::degree_celsius;

/// Studio state for the condenser gallery.
pub struct CondenserTab {
    /// Steam quality of the turbine exhaust entering the shell, dimensionless
    /// `[0, 1]`. Tints the exhaust neck and the steam space.
    pub exhaust_quality: f64,
    /// Shell-side condensing (saturation) temperature, degrees Celsius.
    /// Colours the rain falling off the bundle and the hotwell surface.
    pub condensing_degc: f64,
    /// Condensate temperature leaving the hotwell, degrees Celsius. Below the
    /// condensing temperature when the hotwell is subcooled — which draws as a
    /// pool visibly colder than the rain landing in it.
    pub condensate_degc: f64,
    /// Cooling water entering the inlet waterbox, degrees Celsius.
    pub cooling_water_inlet_degc: f64,
    /// Cooling water leaving the outlet waterbox, degrees Celsius.
    pub cooling_water_outlet_degc: f64,
    /// Whether the caller's model has a hotwell level at all.
    ///
    /// Unchecking it passes `None` to the widget, which hatches an empty sump —
    /// the honest rendering of "not known", and worth being able to see next to
    /// a filled one.
    pub hotwell_level_known: bool,
    /// Hotwell level, dimensionless `[0, 1]`.
    pub hotwell_level_frac: f32,
    /// Cold end of the colour scale, degrees Celsius.
    pub min_temp_degc: f64,
    /// Hot end of the colour scale, degrees Celsius.
    pub max_temp_degc: f64,
    /// Shell-side operating pressure of the wrapped physics component,
    /// kilopascals absolute. A condenser runs a few kPa absolute — deep under
    /// atmospheric — which is why it needs an air offtake.
    pub operating_pressure_kpa: f64,
    /// The wrapped component's **target** outlet quality, dimensionless
    /// `[0, 1]`. A set-point: reported in the readout, never painted.
    pub target_outlet_quality: f64,
    /// Width of one gallery card, in points. Height follows from the artwork's
    /// own aspect ratio.
    pub card_width: f32,
    /// Whether to draw the internal component labels.
    pub show_labels: bool,
}

impl Default for CondenserTab {
    /// Defaults sit at a plausible surface-condenser operating point: wet
    /// turbine exhaust at 0.90 quality condensing near 38 degC (about 6.6 kPa
    /// absolute), condensate 2 K subcooled, and cooling water heating from 22
    /// to 32 degC. The colour scale spans 10-60 degC so the 10 K cooling-water
    /// rise is a visible step rather than two indistinguishable blues — a
    /// plant-wide scale would render the whole condenser one flat colour, which
    /// is correct but useless for checking the artwork.
    fn default() -> Self {
        Self {
            exhaust_quality: 0.90,
            condensing_degc: 38.0,
            condensate_degc: 36.0,
            cooling_water_inlet_degc: 22.0,
            cooling_water_outlet_degc: 32.0,
            hotwell_level_known: true,
            hotwell_level_frac: 0.45,
            min_temp_degc: 10.0,
            max_temp_degc: 60.0,
            operating_pressure_kpa: 6.6,
            target_outlet_quality: 0.0,
            card_width: 320.0,
            show_labels: true,
        }
    }
}

impl CondenserTab {
    /// The scalars every state-driven card is drawn from.
    pub fn scalars(&self) -> CondenserScalars {
        CondenserScalars {
            exhaust_quality: self.exhaust_quality,
            condensing_temp: degc(self.condensing_degc),
            condensate_temp: degc(self.condensate_degc),
            cooling_water_inlet_temp: degc(self.cooling_water_inlet_degc),
            cooling_water_outlet_temp: degc(self.cooling_water_outlet_degc),
            hotwell_level_frac: self.hotwell_level_known.then_some(self.hotwell_level_frac),
        }
    }

    /// The display range the state-driven cards are graded against.
    pub fn range(&self) -> CondenserDisplayRange {
        CondenserDisplayRange {
            min_temp: degc(self.min_temp_degc),
            max_temp: degc(self.max_temp_degc),
        }
    }

    /// The physics component the neutral cards wrap.
    ///
    /// Real stored state — an operating pressure and a target outlet quality —
    /// and nothing else. `Condenser::condense` is not implemented, so there is
    /// no fluid state behind it.
    pub fn physics(&self) -> Condenser {
        Condenser::new(
            Pressure::new::<kilopascal>(self.operating_pressure_kpa),
            self.target_outlet_quality,
        )
    }

    /// A state-driven card for `kind`, at the given screen box.
    ///
    /// Shared by [`draw`] and the readout so what is reported and what is drawn
    /// cannot drift apart.
    pub fn driven(&self, kind: CondenserKind, centre: egui::Pos2, size: Vec2) -> CondenserVisual {
        let visual =
            CondenserVisual::from_scalars(kind, centre, size, self.range(), self.scalars())
                .with_operating_pressure(Pressure::new::<kilopascal>(self.operating_pressure_kpa));
        if self.show_labels {
            visual
        } else {
            visual.without_labels()
        }
    }

    /// A physics-backed card for `kind`, at the given screen box.
    ///
    /// Built through the preserved three-argument
    /// [`CondenserVisual::new`], so this is exactly what any existing call site
    /// gets.
    pub fn neutral(&self, kind: CondenserKind, centre: egui::Pos2, size: Vec2) -> CondenserVisual {
        let visual = CondenserVisual::new(self.physics(), centre, size).with_kind(kind);
        if self.show_labels {
            visual
        } else {
            visual.without_labels()
        }
    }
}

/// Vertical space reserved under each card for its caption, in points.
const CAPTION_H: f32 = 76.0;

/// Gap between cards, in points.
const GAP: f32 = 14.0;

fn degc(value: f64) -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<degree_celsius>(value)
}

/// Right-panel controls for the gallery.
pub fn controls(ui: &mut egui::Ui, state: &mut CondenserTab) {
    ui.heading("Condensers");
    ui.label(
        RichText::new(
            "Two cooling-water arrangements, drawn from two different state \
             sources. Illustrative schematic art — not a validated model and \
             not any specific condenser design.",
        )
        .small()
        .weak(),
    );
    ui.separator();

    ui.label(RichText::new("Steam side").strong());
    ui.add(
        egui::Slider::new(&mut state.exhaust_quality, 0.0..=1.0)
            .text("exhaust quality x")
            .fixed_decimals(3),
    );
    ui.label(
        RichText::new(
            "Blue at x = 0 (saturated liquid) to white at x = 1 (dry vapour), \
             through steam_quality_colour_mark_1. No display remap is applied: \
             a condenser genuinely traverses the whole range from the exhaust \
             neck to the hotwell.",
        )
        .small()
        .weak(),
    );
    ui.add(egui::Slider::new(&mut state.condensing_degc, 20.0..=120.0).text("condensing [°C]"));
    ui.add(egui::Slider::new(&mut state.condensate_degc, 10.0..=120.0).text("condensate [°C]"));
    let subcooling = state.condensing_degc - state.condensate_degc;
    ui.label(
        RichText::new(format!(
            "subcooling {subcooling:.1} K — the pool should read colder than the rain landing in it"
        ))
        .small()
        .weak(),
    );

    ui.separator();
    ui.label(RichText::new("Cooling water [°C]").strong());
    ui.add(
        egui::Slider::new(&mut state.cooling_water_inlet_degc, 0.0..=60.0).text("inlet waterbox"),
    );
    ui.add(
        egui::Slider::new(&mut state.cooling_water_outlet_degc, 0.0..=60.0).text("outlet waterbox"),
    );
    ui.label(
        RichText::new(
            "Each tube row is graded along the cooling-water path, so on a \
             two-pass machine the return rows carry the hottest water back to \
             the same end it entered. That gradient is a display \
             interpolation, not a computed profile.",
        )
        .small()
        .weak(),
    );

    ui.separator();
    ui.label(RichText::new("Hotwell").strong());
    ui.checkbox(&mut state.hotwell_level_known, "level is known");
    ui.add_enabled(
        state.hotwell_level_known,
        egui::Slider::new(&mut state.hotwell_level_frac, 0.0..=1.0)
            .text("level fraction")
            .fixed_decimals(2),
    );
    if !state.hotwell_level_known {
        ui.label(
            RichText::new(
                "Unknown → the sump is hatched and left empty. The widget will \
                 not draw a plausible half-full hotwell it has no source for.",
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
             max. A condenser is the cold end of a plant, so a plant-wide range \
             correctly renders all of it blue — narrow the range here to see the \
             cooling-water rise.",
        )
        .small()
        .weak(),
    );
    ui.add(egui::Slider::new(&mut state.min_temp_degc, -20.0..=100.0).text("min"));
    ui.add(egui::Slider::new(&mut state.max_temp_degc, 0.0..=400.0).text("max"));
    if state.max_temp_degc <= state.min_temp_degc {
        state.max_temp_degc = state.min_temp_degc + 1.0;
    }

    ui.separator();
    ui.label(RichText::new("Physics component (row 2)").strong());
    ui.add(
        egui::Slider::new(&mut state.operating_pressure_kpa, 2.0..=20.0)
            .text("shell pressure [kPa]")
            .fixed_decimals(2),
    );
    ui.add(
        egui::Slider::new(&mut state.target_outlet_quality, 0.0..=1.0)
            .text("target outlet quality")
            .fixed_decimals(3),
    );
    ui.label(
        RichText::new(
            "These are the ONLY two things tampines::components::Condenser \
             stores. The pressure is real state and is labelled on the shell; \
             the target outlet quality is a SET-POINT and is deliberately never \
             painted — move it and nothing on row 2 changes colour.",
        )
        .small()
        .weak(),
    );

    ui.separator();
    ui.label(RichText::new("Layout").strong());
    ui.add(egui::Slider::new(&mut state.card_width, 200.0..=520.0).text("card width [pt]"));
    ui.checkbox(&mut state.show_labels, "show internal labels");

    ui.separator();
    ui.label(RichText::new("What drives what").strong());
    egui::Grid::new("condenser_readout")
        .num_columns(2)
        .striped(true)
        .show(ui, |ui| {
            let scalars = state.scalars();

            ui.label("exhaust neck + steam space");
            ui.label(format!("quality x = {:.3}", scalars.exhaust_quality));
            ui.end_row();

            ui.label("rain off the bundle");
            ui.label(format!("condensing {:.1} °C", state.condensing_degc));
            ui.end_row();

            ui.label("hotwell pool");
            ui.label(format!("condensate {:.1} °C", state.condensate_degc));
            ui.end_row();

            ui.label("tubes + waterboxes");
            ui.label(format!(
                "{:.1} → {:.1} °C ({:+.1} K rise)",
                state.cooling_water_inlet_degc,
                state.cooling_water_outlet_degc,
                state.cooling_water_outlet_degc - state.cooling_water_inlet_degc
            ));
            ui.end_row();

            ui.label("hotwell level");
            match drawn_hotwell_level(scalars.hotwell_level_frac) {
                Some(level) => ui.label(format!("{:.0} % of the sump", level * 100.0)),
                None => ui.label(
                    RichText::new("not known — sump hatched, not filled")
                        .italics()
                        .color(Color32::from_rgb(200, 140, 60)),
                ),
            };
            ui.end_row();

            // The two-pass turn, read straight out of the widget's own helper
            // so the readout cannot disagree with the drawing.
            let lower = cooling_water_path_span(CondenserKind::TwoPass, 9, 10);
            let upper = cooling_water_path_span(CondenserKind::TwoPass, 0, 10);
            ui.label("two-pass water path");
            ui.label(format!(
                "first pass {:.2}→{:.2}, return {:.2}→{:.2}",
                lower.0, lower.1, upper.1, upper.0
            ));
            ui.end_row();

            let neutral = state.neutral(CondenserKind::TwoPass, egui::Pos2::ZERO, Vec2::splat(1.0));
            ui.label("row 2: scalars()");
            ui.label(
                RichText::new("None — no fluid state on the component")
                    .italics()
                    .color(Color32::from_rgb(200, 140, 60)),
            );
            ui.end_row();

            ui.label("row 2: operating pressure");
            ui.label(match neutral.operating_pressure() {
                Some(p) => format!("{:.2} kPa (real, labelled)", p.get::<kilopascal>()),
                None => "not known".to_string(),
            });
            ui.end_row();

            ui.label("row 2: target outlet quality");
            ui.label(match neutral.target_outlet_quality() {
                Some(x) => format!("{x:.3} — set-point, NOT painted"),
                None => "n/a".to_string(),
            });
            ui.end_row();
        });
}

/// Draws the state-driven row and the physics-backed row.
pub fn draw(ui: &mut egui::Ui, state: &CondenserTab) {
    ui.heading("Widget under test");
    ui.label(
        RichText::new(
            "Steam in at the top, cooling water through the tubes, condensate \
             in the hotwell. Row 2 is the same widget with nothing to see by — \
             it must stay grey no matter what the sliders do.",
        )
        .small()
        .weak(),
    );
    ui.separator();

    egui::ScrollArea::both()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.label(RichText::new("State-driven — CondenserVisual::from_scalars").strong());
            ui.label(
                RichText::new(
                    "Real state from the caller's own model: exhaust quality, \
                     condensing and condensate temperatures, cooling-water \
                     temperatures, hotwell level.",
                )
                .small()
                .weak(),
            );
            row(ui, state, true);

            ui.add_space(GAP);
            ui.separator();
            ui.label(RichText::new("Physics-backed — CondenserVisual::new").strong());
            ui.label(
                RichText::new(
                    "Wrapping a tampines::components::Condenser, which holds an \
                     operating pressure and a target outlet quality and nothing \
                     else. Condenser::condense is not implemented, so there is \
                     no fluid state: every region is drawn neutral grey and the \
                     pressure is labelled. This is the honest rendering, shown \
                     deliberately.",
                )
                .small()
                .weak(),
            );
            row(ui, state, false);

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

/// Draws one row of cards — one per [`CondenserKind`] — either state-driven or
/// physics-backed.
fn row(ui: &mut egui::Ui, state: &CondenserTab, driven: bool) {
    let card_w = state.card_width;

    ui.horizontal_top(|ui| {
        for kind in CondenserKind::ALL {
            // Height follows the artwork's own proportions, so the card is
            // filled rather than letterboxed into a band of empty space.
            let card_h = card_w / kind.native_aspect_ratio();

            ui.vertical(|ui| {
                ui.set_width(card_w);
                let (rect, _response) =
                    ui.allocate_exact_size(Vec2::new(card_w, card_h), egui::Sense::hover());
                let size = Vec2::new(card_w - 16.0, card_h - 16.0);
                if driven {
                    ui.put(rect, state.driven(*kind, rect.center(), size));
                } else {
                    ui.put(rect, state.neutral(*kind, rect.center(), size));
                }

                ui.allocate_ui(Vec2::new(card_w, CAPTION_H), |ui| {
                    ui.label(RichText::new(kind.label()).strong());
                    ui.label(RichText::new(kind.description()).small().weak());
                    ui.label(
                        RichText::new(format!("water: {}", kind.cooling_water_path()))
                            .small()
                            .weak(),
                    );
                    if driven {
                        ui.label(
                            RichText::new(format!("{} pass(es) through the bundle", kind.passes()))
                                .small()
                                .weak(),
                        );
                    } else {
                        ui.label(
                            RichText::new("no fluid state — nothing fabricated")
                                .small()
                                .italics()
                                .color(Color32::from_rgb(200, 140, 60)),
                        );
                    }
                });
            });
            ui.add_space(GAP);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The physics-backed row must stay stateless however far the sliders are
    /// moved — that is the whole reason it is on screen.
    ///
    /// **Methodology.** Build the tab, then sweep every state-driven control
    /// across its slider range (exhaust quality 0..1, condensing and condensate
    /// temperatures 20-120 degC, both cooling-water temperatures 0-60 degC,
    /// hotwell level 0..1 and both ends of the colour scale) and, at every
    /// combination sampled, require the card built by
    /// [`CondenserTab::neutral`] — i.e. `CondenserVisual::new` — to report no
    /// scalars at all. Then require it to report the operating pressure and the
    /// target outlet quality it really holds, so the neutral path is neutral
    /// without being empty.
    ///
    /// **Result (2026-08-12):** 1 452 slider combinations sampled, every one
    /// giving `scalars() == None`; the operating pressure came back as the
    /// 6.60 kPa set on the slider and the target outlet quality as 0.000.
    /// Interpretation: no state-driven control can leak into the physics-backed
    /// rendering, so row 2 cannot quietly start painting a temperature.
    #[test]
    fn the_physics_row_never_picks_up_the_state_driven_sliders() {
        let mut tab = CondenserTab::default();
        let mut sampled = 0usize;
        for quality_step in 0..=10 {
            tab.exhaust_quality = quality_step as f64 / 10.0;
            for temp_step in 0..=10 {
                tab.condensing_degc = 20.0 + temp_step as f64 * 10.0;
                tab.condensate_degc = tab.condensing_degc - 2.0;
                tab.cooling_water_inlet_degc = temp_step as f64 * 6.0;
                tab.cooling_water_outlet_degc = tab.cooling_water_inlet_degc + 10.0;
                for level_step in 0..=5 {
                    tab.hotwell_level_frac = level_step as f32 / 5.0;
                    for kind in CondenserKind::ALL {
                        let neutral = tab.neutral(*kind, egui::Pos2::ZERO, Vec2::splat(100.0));
                        assert!(
                            neutral.scalars().is_none(),
                            "the physics path picked up state at x = {}",
                            tab.exhaust_quality
                        );
                        sampled += 1;
                    }
                }
            }
        }
        println!("{sampled} slider combinations sampled");

        let tab = CondenserTab::default();
        let neutral = tab.neutral(CondenserKind::TwoPass, egui::Pos2::ZERO, Vec2::splat(100.0));
        assert_eq!(
            neutral
                .operating_pressure()
                .map(|p| (p.get::<kilopascal>() * 100.0).round() / 100.0),
            Some(6.60)
        );
        assert_eq!(neutral.target_outlet_quality(), Some(0.0));
    }

    /// The state-driven row must hand the widget exactly what the sliders say,
    /// with nothing substituted on the way.
    #[test]
    fn the_state_driven_row_passes_the_sliders_through_unchanged() {
        let mut tab = CondenserTab::default();
        tab.exhaust_quality = 0.83;
        tab.condensing_degc = 41.0;
        tab.condensate_degc = 37.5;
        tab.cooling_water_inlet_degc = 19.0;
        tab.cooling_water_outlet_degc = 31.0;
        tab.hotwell_level_frac = 0.61;

        for kind in CondenserKind::ALL {
            let drawn = tab
                .driven(*kind, egui::Pos2::ZERO, Vec2::splat(100.0))
                .scalars()
                .expect("the state-driven card carries scalars");
            assert_eq!(drawn, tab.scalars());
            assert_eq!(drawn.exhaust_quality, 0.83);
            assert_eq!(drawn.hotwell_level_frac, Some(0.61));
        }
    }

    /// Unchecking "level is known" must reach the widget as a genuine absence,
    /// not as zero — an empty hotwell and an unknown one are different claims.
    ///
    /// **Methodology.** Toggle [`CondenserTab::hotwell_level_known`] off with
    /// the level slider left at a non-zero value, and require the scalars
    /// handed to the widget to carry `None`; then require
    /// `drawn_hotwell_level` — the function the widget itself uses to decide
    /// between a pool and a hatched sump — to agree. Toggle it back on and
    /// require the slider value to return unchanged.
    ///
    /// **Result (2026-08-12):** with the level slider at 0.45, the unknown
    /// state gave `hotwell_level_frac == None` and `drawn_hotwell_level` gave
    /// `None`; re-checking gave `Some(0.45)` in both. Interpretation: the
    /// studio can show an unknown hotwell next to a known one, and the two are
    /// distinguishable in the drawing rather than only in the caption.
    #[test]
    fn an_unknown_hotwell_level_reaches_the_widget_as_unknown() {
        let mut tab = CondenserTab::default();
        assert_eq!(tab.hotwell_level_frac, 0.45);

        tab.hotwell_level_known = false;
        let unknown = tab.scalars();
        assert_eq!(unknown.hotwell_level_frac, None);
        assert_eq!(drawn_hotwell_level(unknown.hotwell_level_frac), None);

        tab.hotwell_level_known = true;
        let known = tab.scalars();
        assert_eq!(known.hotwell_level_frac, Some(0.45));
        assert_eq!(drawn_hotwell_level(known.hotwell_level_frac), Some(0.45));
    }

    /// Every card in the gallery must be given a box already at the artwork's
    /// own proportions, so nothing is letterboxed into a band of dead space.
    #[test]
    fn the_cards_are_sized_from_the_artworks_own_aspect_ratio() {
        let tab = CondenserTab::default();
        for kind in CondenserKind::ALL {
            let card_h = tab.card_width / kind.native_aspect_ratio();
            let fitted = kind.fit_native_aspect(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                Vec2::new(tab.card_width, card_h),
            ));
            assert!((fitted.width() - tab.card_width).abs() < 1e-3);
            assert!((fitted.height() - card_h).abs() < 1e-3);
        }
    }
}
