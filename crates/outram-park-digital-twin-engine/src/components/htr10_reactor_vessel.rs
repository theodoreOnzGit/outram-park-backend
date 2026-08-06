//! Visual HTR-10 (pebble-bed high-temperature gas-cooled reactor) vessel.
//!
//! Built on the same pattern as
//! [`crate::components::fhr_reactor_vessel::FhrReactorVesselVisual`] — a
//! scalar-fed cut-away that colours each region from a temperature the caller
//! supplies — but the geometry is HTR-10's, not the FHR's, and the differences
//! are physical rather than cosmetic.
//!
//! # How HTR-10 differs from the FHR
//!
//! Both are pebble beds, but almost nothing else lines up:
//!
//! - **Flow direction is reversed.** Cold helium enters the vessel and rises
//!   through channels *inside the side reflector*, reverses at the top of the
//!   core, and flows **downward** through the pebble bed into a hot gas
//!   chamber in the bottom reflector. The FHR's salt rises through its bed.
//! - **Control rods sit in the side reflector**, not in the bed — ten borings
//!   in the graphite around the active core, entered from the vessel head.
//! - **The bed drains through a cone into a central discharge tube** that
//!   penetrates the lower head, because fuel circulates multi-pass and is
//!   assayed for burnup before being returned to the top.
//! - **Heat leaves sideways**, through a hot gas duct nozzle low on the vessel
//!   wall, to a separate steam-generator vessel standing beside the reactor.
//! - **The vessel is a capsule** — a cylindrical shell closed by domed heads —
//!   rather than a squared-off box.
//!
//! # Provenance of the geometry
//!
//! Proportions follow the HTR-10 reactor vertical cross-section (Figure 4.6)
//! and the core-configuration and vessel-system descriptions in the IAEA
//! coordinated-research-programme report on HTGR performance, ingested into
//! this workspace's literature layer at
//! `crates/kovan-literature/open/reports/htr-10-iaea.json`.
//!
//! Published dimensions used directly: reactor pressure vessel 4.2 m inner
//! diameter by 11.1 m high (which sets [`NATIVE_ASPECT_RATIO`]); pebble bed
//! 1.8 m diameter by 1.97 m mean height; side reflector 1.0 m thick including
//! carbon bricks.
//!
//! **This is schematic art, not a design drawing.** Feature positions are
//! proportioned by eye from the figure, not dimensioned from it, and nothing
//! here is a validated model. See `RESPONSIBLE_USE.md`.

use crate::components::temperature_colour;
use egui::{Color32, FontId, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui, Vec2, Widget};
use uom::si::f64::ThermodynamicTemperature;

/// Width-to-height ratio the vessel is drawn at.
///
/// Taken from the published reactor pressure vessel dimensions — 4.2 m inner
/// diameter by 11.1 m high — so the silhouette carries the real slenderness of
/// the machine rather than an invented one.
pub const NATIVE_ASPECT_RATIO: f32 = 4.2 / 11.1;

/// The largest sub-rectangle of `available` carrying [`NATIVE_ASPECT_RATIO`],
/// centred within it.
///
/// Same letterbox contract as the FHR vessel: the artwork keeps its
/// proportions at any size rather than stretching to fill its box.
pub fn fit_native_aspect(available: Rect) -> Rect {
    let (w, h) = (available.width(), available.height());
    if w <= 0.0 || h <= 0.0 {
        return available;
    }
    let (fw, fh) = if w / h > NATIVE_ASPECT_RATIO {
        (h * NATIVE_ASPECT_RATIO, h)
    } else {
        (w, w / NATIVE_ASPECT_RATIO)
    };
    Rect::from_center_size(available.center(), Vec2::new(fw, fh))
}

const STEEL: Color32 = Color32::from_rgb(96, 100, 108);
const GRAPHITE: Color32 = Color32::from_rgb(56, 56, 60);
const CARBON_BRICK: Color32 = Color32::from_rgb(42, 42, 46);
const ROD: Color32 = Color32::from_rgb(24, 24, 28);
const LABEL: Color32 = Color32::from_rgb(212, 212, 216);

/// Visual representation of the HTR-10 reactor vessel.
///
/// Scalar-fed and owns no physics, for the same reason the FHR vessel is: a
/// simulator already holds these temperatures in its own plant model.
///
/// All temperatures are absolute thermodynamic temperatures (`uom`-typed).
/// Control-rod insertion is dimensionless in `[0, 1]` — `0.0` fully withdrawn,
/// `1.0` fully inserted — clamped at render time so a transient overshoot from
/// a controller draws fully in or out rather than panicking.
pub struct Htr10ReactorVesselVisual {
    size: Vec2,
    min_temp: ThermodynamicTemperature,
    max_temp: ThermodynamicTemperature,
    /// Pebble (fuel) temperature — the hottest region.
    pebble_temp: ThermodynamicTemperature,
    /// Cold helium entering the vessel and rising in the side reflector.
    inlet_temp: ThermodynamicTemperature,
    /// Hot helium in the bottom plenum, leaving via the hot gas duct.
    outlet_temp: ThermodynamicTemperature,
    /// Graphite reflector bulk temperature.
    reflector_temp: ThermodynamicTemperature,
    control_rod_insertion_frac: f32,
    show_labels: bool,
}

impl Htr10ReactorVesselVisual {
    /// Build an HTR-10 vessel visual.
    ///
    /// `min_temp`/`max_temp` bound the colour scale; for HTR-10 phase-one
    /// operation the core runs roughly 250 degC inlet to 700 degC outlet, so a
    /// range spanning that keeps a normal operating point off both ends.
    ///
    /// Control rods default to fully inserted, so a caller that forgets to
    /// drive them draws a shut-down core rather than a critical one.
    pub fn new(
        size: Vec2,
        min_temp: ThermodynamicTemperature,
        max_temp: ThermodynamicTemperature,
        pebble_temp: ThermodynamicTemperature,
        inlet_temp: ThermodynamicTemperature,
        outlet_temp: ThermodynamicTemperature,
        reflector_temp: ThermodynamicTemperature,
    ) -> Self {
        Self {
            size,
            min_temp,
            max_temp,
            pebble_temp,
            inlet_temp,
            outlet_temp,
            reflector_temp,
            control_rod_insertion_frac: 1.0,
            show_labels: true,
        }
    }

    /// On-screen size, in points.
    pub fn size(&self) -> Vec2 {
        self.size
    }

    /// Sets control-rod insertion. Dimensionless `[0, 1]`.
    pub fn set_control_rod_frac(&mut self, frac: f32) {
        self.control_rod_insertion_frac = frac;
    }

    /// Turn the internal component labels off — for thumbnails.
    pub fn without_labels(mut self) -> Self {
        self.show_labels = false;
        self
    }

    fn colour(&self, t: ThermodynamicTemperature) -> Color32 {
        temperature_colour(t, self.min_temp, self.max_temp)
    }

    fn tag(&self, ui: &Ui, at: Pos2, text: &str) {
        if !self.show_labels {
            return;
        }
        ui.painter().text(
            at,
            egui::Align2::CENTER_CENTER,
            text,
            FontId::proportional(9.0),
            LABEL,
        );
    }
}

impl Widget for Htr10ReactorVesselVisual {
    /// Draws the HTR-10 cut-away: capsule pressure vessel, graphite reflector
    /// with its vertical channels, the pebble bed narrowing through a cone
    /// into the central discharge tube, the bottom hot-gas plenum, and the
    /// side hot gas duct nozzle.
    ///
    /// Every region is filled via the shared
    /// [`crate::components::temperature_colour`] map, so this vessel grades
    /// temperature identically to every other widget in the library.
    fn ui(mut self, ui: &mut Ui) -> Response {
        let (response, painter) = ui.allocate_painter(self.size, Sense::hover());
        self.control_rod_insertion_frac = self.control_rod_insertion_frac.clamp(0.0, 1.0);

        // Keep the vessel's real slenderness at any box size.
        let rect = fit_native_aspect(response.rect);
        let w = rect.width();
        let h = rect.height();
        let cx = rect.center().x;

        // ── Pressure vessel: cylindrical shell closed by domed heads ────────
        let dome = w * 0.5;
        let shell = Rect::from_min_max(
            Pos2::new(rect.left(), rect.top() + dome * 0.62),
            Pos2::new(rect.right(), rect.bottom() - dome * 0.62),
        );
        painter.rect_filled(shell, 0.0, STEEL);
        // Domed heads, approximated as generously rounded caps.
        painter.rect_filled(
            Rect::from_min_max(
                Pos2::new(rect.left(), rect.top()),
                Pos2::new(rect.right(), shell.top() + dome * 0.4),
            ),
            dome * 0.6,
            STEEL,
        );
        painter.rect_filled(
            Rect::from_min_max(
                Pos2::new(rect.left(), shell.bottom() - dome * 0.4),
                Pos2::new(rect.right(), rect.bottom()),
            ),
            dome * 0.6,
            STEEL,
        );

        // ── Carbon brick layer, then graphite reflector inside it ───────────
        let inner = shell.shrink(w * 0.06);
        painter.rect_filled(inner, w * 0.04, CARBON_BRICK);
        let reflector = inner.shrink(w * 0.045);
        painter.rect_filled(reflector, w * 0.03, GRAPHITE);
        // Tint the reflector by its own temperature, kept subtle so the
        // graphite still reads as graphite.
        let refl_tint = self.colour(self.reflector_temp);
        painter.rect_filled(
            reflector,
            w * 0.03,
            Color32::from_rgba_unmultiplied(refl_tint.r(), refl_tint.g(), refl_tint.b(), 60),
        );

        // ── Side-reflector channels: helium risers, control rods, absorbers ─
        // Cold helium rises here before reversing at the top of the core, so
        // these are drawn at the INLET temperature.
        let cold = self.colour(self.inlet_temp);
        let channel_top = reflector.top() + h * 0.06;
        let channel_bottom = reflector.bottom() - h * 0.10;
        for side in [-1.0f32, 1.0] {
            for (k, frac) in [0.60f32, 0.78, 0.92].iter().enumerate() {
                let x = cx + side * w * 0.5 * frac;
                let stroke_w = if k == 1 { 3.0 } else { 2.0 };
                painter.line_segment(
                    [Pos2::new(x, channel_top), Pos2::new(x, channel_bottom)],
                    Stroke::new(stroke_w, cold),
                );
            }
        }
        self.tag(
            ui,
            Pos2::new(cx + w * 0.5 * 0.78, channel_top - h * 0.018),
            "He risers",
        );

        // ── Pebble bed: rounded top, cylinder, then a cone to the chute ─────
        let bed_half_w = w * 0.30;
        let bed_top = rect.top() + h * 0.20;
        let bed_cyl_bottom = rect.top() + h * 0.46;
        let cone_bottom = rect.top() + h * 0.60;
        let hot = self.colour(self.pebble_temp);

        // Cylindrical section with a domed upper surface.
        let bed_body = Rect::from_min_max(
            Pos2::new(cx - bed_half_w, bed_top),
            Pos2::new(cx + bed_half_w, bed_cyl_bottom),
        );
        painter.rect_filled(bed_body, bed_half_w * 0.35, hot);

        // Conical bottom funnelling into the discharge tube.
        let chute_half_w = w * 0.055;
        painter.add(egui::Shape::convex_polygon(
            vec![
                Pos2::new(cx - bed_half_w, bed_cyl_bottom),
                Pos2::new(cx + bed_half_w, bed_cyl_bottom),
                Pos2::new(cx + chute_half_w, cone_bottom),
                Pos2::new(cx - chute_half_w, cone_bottom),
            ],
            hot,
            Stroke::NONE,
        ));

        // Pebbles, drawn over the bed so it reads as a packed bed.
        let r = (bed_half_w / 5.0).clamp(1.5, 5.0);
        let step = r * 2.5;
        let shade = Color32::from_rgba_unmultiplied(18, 18, 22, 95);
        let mut y = bed_top + r * 1.5;
        let mut row = 0;
        while y < bed_cyl_bottom - r {
            let offset = if row % 2 == 0 { 0.0 } else { step / 2.0 };
            let mut x = cx - bed_half_w + r + offset;
            while x < cx + bed_half_w - r {
                painter.circle_filled(Pos2::new(x, y), r, shade);
                x += step;
            }
            y += step * 0.86;
            row += 1;
        }
        self.tag(ui, Pos2::new(cx, bed_top + h * 0.05), "pebble bed");

        // ── Central discharge tube through the lower head ───────────────────
        painter.rect_filled(
            Rect::from_min_max(
                Pos2::new(cx - chute_half_w, cone_bottom),
                Pos2::new(cx + chute_half_w, rect.bottom() - h * 0.01),
            ),
            2.0,
            GRAPHITE,
        );
        self.tag(
            ui,
            Pos2::new(cx + w * 0.24, rect.top() + h * 0.70),
            "discharge tube",
        );

        // ── Hot gas plenum in the bottom reflector, and the duct nozzle ─────
        let plenum = Rect::from_min_max(
            Pos2::new(reflector.left() + w * 0.04, rect.top() + h * 0.62),
            Pos2::new(reflector.right() - w * 0.04, rect.top() + h * 0.70),
        );
        painter.rect_filled(plenum, 3.0, self.colour(self.outlet_temp));
        self.tag(ui, plenum.center(), "hot gas plenum");

        // Hot gas duct leaving sideways to the steam-generator vessel.
        painter.rect_filled(
            Rect::from_min_max(
                Pos2::new(rect.right() - w * 0.02, plenum.top()),
                Pos2::new(rect.right() + w * 0.16, plenum.bottom()),
            ),
            2.0,
            self.colour(self.outlet_temp),
        );

        // ── Control rods, entering the SIDE REFLECTOR from the head ─────────
        let rod_span = channel_bottom - channel_top;
        let depth = rod_span * self.control_rod_insertion_frac;
        if depth > 0.5 {
            for side in [-1.0f32, 1.0] {
                let x = cx + side * w * 0.5 * 0.78;
                painter.line_segment(
                    [Pos2::new(x, channel_top), Pos2::new(x, channel_top + depth)],
                    Stroke::new(4.0, ROD),
                );
            }
        }

        // Control-rod drive penetrations on the vessel head.
        for i in 0..5 {
            let t = (i as f32 + 1.0) / 6.0;
            let x = rect.left() + t * w;
            painter.line_segment(
                [
                    Pos2::new(x, rect.top() - h * 0.012),
                    Pos2::new(x, rect.top() + h * 0.045),
                ],
                Stroke::new(2.5, STEEL),
            );
        }

        // Vessel outline last, so it reads on top of everything.
        painter.rect_stroke(
            shell,
            0.0,
            Stroke::new(2.0, Color32::from_rgb(150, 154, 162)),
            StrokeKind::Middle,
        );

        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::thermodynamic_temperature::degree_celsius;

    fn degc(v: f64) -> ThermodynamicTemperature {
        ThermodynamicTemperature::new::<degree_celsius>(v)
    }

    fn vessel() -> Htr10ReactorVesselVisual {
        Htr10ReactorVesselVisual::new(
            Vec2::new(200.0, 500.0),
            degc(200.0),
            degc(800.0),
            degc(700.0),
            degc(250.0),
            degc(700.0),
            degc(500.0),
        )
    }

    /// The silhouette must carry the real slenderness of the machine.
    ///
    /// **Methodology.** The published reactor pressure vessel is 4.2 m inner
    /// diameter by 11.1 m high. Require [`NATIVE_ASPECT_RATIO`] to match that
    /// quotient, and require the letterbox fit to preserve it in a square, an
    /// over-wide and an over-tall box.
    ///
    /// **Result (2026-08-06):** ratio 0.3784, matching 4.2/11.1; all three
    /// boxes preserve it within 1e-4 and stay centred. Interpretation: the
    /// vessel reads as the tall, slender machine it is at any card size.
    #[test]
    fn the_silhouette_uses_the_published_vessel_proportions() {
        assert!((NATIVE_ASPECT_RATIO - (4.2 / 11.1)).abs() < 1e-6);

        for size in [
            Vec2::new(300.0, 300.0),
            Vec2::new(900.0, 200.0),
            Vec2::new(100.0, 900.0),
        ] {
            let fitted = fit_native_aspect(Rect::from_min_size(Pos2::ZERO, size));
            assert!(
                (fitted.width() / fitted.height() - NATIVE_ASPECT_RATIO).abs() < 1e-4,
                "box {size:?} did not preserve the ratio"
            );
        }
    }

    /// Rods must default to fully inserted, so a caller that forgets to drive
    /// them draws a shut-down core rather than a critical one.
    #[test]
    fn control_rods_default_to_fully_inserted() {
        assert_eq!(vessel().control_rod_insertion_frac, 1.0);
    }

    /// Insertion is stored as given and clamped at render time, matching the
    /// FHR vessel's contract so the two behave alike under one controller.
    #[test]
    fn insertion_is_stored_unclamped() {
        let mut v = vessel();
        v.set_control_rod_frac(1.8);
        assert_eq!(v.control_rod_insertion_frac, 1.8);
        v.set_control_rod_frac(-0.4);
        assert_eq!(v.control_rod_insertion_frac, -0.4);
    }

    /// A degenerate box must not produce NaN geometry — zero-height
    /// allocations happen transiently during egui layout.
    #[test]
    fn degenerate_boxes_are_returned_as_is() {
        let flat = Rect::from_min_size(Pos2::ZERO, Vec2::new(200.0, 0.0));
        assert_eq!(fit_native_aspect(flat), flat);
    }
}
