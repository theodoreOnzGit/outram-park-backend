//! The Plant tab: an **indicative** elevation sketch of EBR-II's primary
//! tank (pool, core, IHX, the two primary pumps) and the intermediate loop's
//! connections, coloured by the model's node temperatures, with flow tracers
//! derived from the model's state.
//!
//! **The layout is not plant geometry.** ANL-ARC-226 Chapter 5's elevations
//! (Figs 5.1.1-5.1.2) are not transcribed (`docs/ebr2_library.md`, "What is
//! not done"), so the boxes are placed to read as the sodium flows, in
//! drawing units with no physical length, and the page says so. No scale bar
//! is drawn because there is no length to show.
//!
//! **Motion is derived, never hardcoded** (the crate's `AGENTS.md`). Tracers
//! run only on the three segments the model gives a sodium inventory: the
//! core node, the IHX primary node and the pool ([`Snapshot::core_na_kg`]
//! and the others). Each mark crosses its segment in one residence time
//! `m / W` ([`bugis::animation::residence_time_from_flow`]) of **plant**
//! time, advanced by the plant time between frames, so pause stops it and
//! the speed control scales it; direction from the sign of the primary flow.
//! The pump, inlet piping and the core-to-IHX outlet pipe have no volume in
//! the model, so they get a direction arrow (from the flow's sign) and no
//! tracers. Where a frame's plant time is more than half a residence time,
//! marks would jump further than their spacing and the eye would read a
//! false motion; such a segment is drawn as a streaming band and labelled
//! with its residence time instead.
//!
//! The pumps' impellers are not animated: at ~800 rpm a drawn rotation
//! aliases at any frame rate, so the speed is shown as a number.

use bugis::animation::{residence_time_from_flow, TracerTrain};
use egui::{Align2, Color32, FontId, Pos2, Rect, Stroke, Vec2};
use outram_park_digital_twin_engine::color_maps::hot_to_cold_colour_mark_1;
use outram_park_digital_twin_engine::ebr2::session::Snapshot;
use uom::si::f64::{Mass, MassRate, Time};
use uom::si::mass::kilogram;
use uom::si::mass_rate::kilogram_per_second;
use uom::si::time::second;

/// The colour scale, °C: the drawing's coldest sodium blue, hottest red.
/// A display range (the SHRT core inlet is ~340-350 °C, the V&V record's
/// SHRT-45R peak outlet 936.8 K = 664 °C), not a limit.
pub const T_COLD_C: f64 = 300.0;
pub const T_HOT_C: f64 = 700.0;

/// The drawing's half-extent, drawing units (no physical length).
pub const HALF_EXTENT: f64 = 5.6;

/// Marks per tracer train: a drawing choice with no physical counterpart.
const MARKS: usize = 6;

/// A segment's tracers draw as marks while a frame advances them by at most
/// this fraction of a pass (Nyquist: beyond half a spacing per frame the
/// motion reads backwards or random).
const MAX_PASS_PER_FRAME: f64 = 0.5;

/// The three segments with a sodium inventory in the model.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Segment {
    Core,
    Ihx,
    Pool,
}

impl Segment {
    pub const ALL: [Segment; 3] = [Segment::Core, Segment::Ihx, Segment::Pool];

    /// The segment's sodium, [kg], from the snapshot.
    fn inventory_kg(self, s: &Snapshot) -> f64 {
        match self {
            Segment::Core => s.core_na_kg,
            Segment::Ihx => s.ihx_na_kg,
            Segment::Pool => s.pool_na_kg,
        }
    }

    fn index(self) -> usize {
        match self {
            Segment::Core => 0,
            Segment::Ihx => 1,
            Segment::Pool => 2,
        }
    }

    /// The path, drawing units, in the direction of positive primary flow.
    fn path(self) -> &'static [[f64; 2]] {
        match self {
            Segment::Core => &CORE_PATH,
            Segment::Ihx => &IHX_PATH,
            Segment::Pool => &POOL_PATH,
        }
    }
}

// The sketch, drawing units (see the module doc: indicative, no length).
const TANK: [f64; 4] = [-5.2, -4.2, 5.2, 3.2];
const SURFACE_Y: f64 = 2.6;
const CORE: [f64; 4] = [-0.9, -2.7, 0.9, -0.6];
const IHX: [f64; 4] = [2.3, -2.1, 3.7, 1.1];
const PUMPS_X: [f64; 2] = [-3.9, -2.9];
const CORE_PATH: [[f64; 2]; 2] = [[0.0, -2.7], [0.0, -0.6]];
const IHX_PATH: [[f64; 2]; 2] = [[3.0, 1.1], [3.0, -2.1]];
const POOL_PATH: [[f64; 2]; 5] = [[3.0, -2.1], [3.0, -3.8], [-4.7, -3.8], [-4.7, 2.0], [-3.4, 2.0]];
/// Pump discharge to the inlet plenum under the core (no volume in the model).
const INLET_PATH: [[f64; 2]; 4] = [[-3.4, 2.0], [-3.4, -3.2], [0.0, -3.2], [0.0, -2.7]];
/// Core outlet to the IHX inlet (no volume in the model).
const OUTLET_PATH: [[f64; 2]; 3] = [[0.0, -0.6], [0.0, 1.8], [3.0, 1.8]];

/// The app-owned tracer state (the crate's rule: a widget never owns its
/// train), advanced once per new plant frame.
pub struct Tracers {
    trains: [TracerTrain; 3],
    /// Plant time per frame over residence time, at the last advance.
    passes_per_frame: [f64; 3],
    /// Residence times at the last advance, [s] (infinite at zero flow).
    residence_s: [f64; 3],
    last_time_s: Option<f64>,
}

impl Default for Tracers {
    fn default() -> Self {
        Self { trains: [TracerTrain::new(MARKS); 3], passes_per_frame: [0.0; 3], residence_s: [f64::INFINITY; 3], last_time_s: None }
    }
}

impl Tracers {
    /// Advance every train by the plant time since the last frame, at this
    /// frame's flow and inventories. A frame at or before the last (a reset)
    /// restarts the clock without moving anything.
    pub fn advance(&mut self, s: &Snapshot) {
        let dt = match self.last_time_s {
            Some(t) if s.time_s > t => s.time_s - t,
            _ => 0.0,
        };
        self.last_time_s = Some(s.time_s);
        let flow = MassRate::new::<kilogram_per_second>(s.primary_flow_kg_s);
        for seg in Segment::ALL {
            let i = seg.index();
            let tau = residence_time_from_flow(Mass::new::<kilogram>(seg.inventory_kg(s)), flow);
            let tau_s = tau.get::<second>();
            self.residence_s[i] = tau_s;
            self.passes_per_frame[i] = if tau_s.is_finite() && tau_s > 0.0 { dt / tau_s } else { 0.0 };
            self.trains[i].advance(Time::new::<second>(dt), tau, flow);
        }
    }

    /// Whether `seg`'s marks would alias at the last frame's advance.
    pub fn too_fast(&self, seg: Segment) -> bool {
        self.passes_per_frame[seg.index()] > MAX_PASS_PER_FRAME
    }

    /// `seg`'s residence time, [s].
    pub fn residence_s(&self, seg: Segment) -> f64 {
        self.residence_s[seg.index()]
    }
}

/// The colour of a temperature on the drawing's scale.
pub fn colour(t_c: f64) -> Color32 {
    hot_to_cold_colour_mark_1(((t_c - T_COLD_C) / (T_HOT_C - T_COLD_C)).clamp(0.0, 1.0) as f32)
}

/// A point `f` of the way along a polyline, by length.
fn along(path: &[[f64; 2]], f: f64) -> [f64; 2] {
    let len = |a: [f64; 2], b: [f64; 2]| {
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        (dx * dx + dy * dy).sqrt()
    };
    let total: f64 = path.windows(2).map(|w| len(w[0], w[1])).sum();
    let mut left = f.clamp(0.0, 1.0) * total;
    for w in path.windows(2) {
        let l = len(w[0], w[1]);
        if left <= l && l > 0.0 {
            let k = left / l;
            return [w[0][0] + k * (w[1][0] - w[0][0]), w[0][1] + k * (w[1][1] - w[0][1])];
        }
        left -= l;
    }
    path[path.len() - 1]
}

/// Draw the sketch. `to` maps drawing units to the screen. GUI drawing code,
/// reached by no test (needs a window); what it draws is the snapshot
/// (`ebr2::session` tests) and [`Tracers`] (tested below).
pub fn paint(painter: &egui::Painter, to: &impl Fn(f64, f64) -> Pos2, s: &Snapshot, tr: &Tracers, text: Color32, muted: Color32) {
    let rect = |r: [f64; 4]| Rect::from_two_pos(to(r[0], r[3]), to(r[2], r[1]));
    let px = (to(1.0, 0.0).x - to(0.0, 0.0).x).abs().max(1.0);
    let font = |k: f32| FontId::proportional((k * px).clamp(9.0, 15.0));
    let line_w = (0.12 * px).clamp(2.0, 10.0);

    // The pool at its temperature, the free surface, the cover gas.
    let tank = rect(TANK);
    painter.rect_stroke(tank, 6.0, Stroke::new(2.0, muted), egui::StrokeKind::Inside);
    let pool = Rect::from_two_pos(to(TANK[0], SURFACE_Y), to(TANK[2], TANK[1]));
    painter.rect_filled(pool.shrink(2.0), 4.0, colour(s.core_inlet_c).gamma_multiply(0.45));
    painter.line_segment([to(TANK[0], SURFACE_Y), to(TANK[2], SURFACE_Y)], Stroke::new(1.5, muted));
    painter.text(to(TANK[0] + 0.2, SURFACE_Y + 0.15), Align2::LEFT_BOTTOM, "argon cover gas · sodium free surface", font(0.28), muted);
    painter.text(
        to(-1.2, 2.2),
        Align2::CENTER_CENTER,
        format!("pool {:.0} °C · {:.0} t Na", s.core_inlet_c, s.pool_na_kg / 1000.0),
        font(0.32),
        text,
    );

    // Piping with no volume in the model: coloured, arrowed, no tracers.
    let pipe = |path: &[[f64; 2]], t_c: f64| {
        let pts: Vec<Pos2> = path.iter().map(|p| to(p[0], p[1])).collect();
        painter.add(egui::Shape::line(pts, Stroke::new(line_w, colour(t_c))));
    };
    pipe(&INLET_PATH, s.core_inlet_c);
    pipe(&OUTLET_PATH, s.core_outlet_c);
    for (path, f) in [(&INLET_PATH[..], 0.55), (&OUTLET_PATH[..], 0.6)] {
        arrow(painter, to, path, f, s.primary_flow_kg_s, text);
    }

    // Core: coolant at its mean, a fuel stripe at the fuel temperature.
    let core = rect(CORE);
    painter.rect_filled(core, 3.0, colour(s.core_mean_c));
    let fuel = Rect::from_center_size(core.center(), Vec2::new(core.width() * 0.35, core.height() * 0.8));
    painter.rect_filled(fuel, 2.0, colour(s.fuel_c));
    painter.rect_stroke(core, 3.0, Stroke::new(1.5, text), egui::StrokeKind::Inside);
    painter.text(to(CORE[0] - 0.15, -1.2), Align2::RIGHT_CENTER, format!("core\n{:.1} MW\nfuel {:.0} °C\nout {:.0} °C", s.total_mw, s.fuel_c, s.core_outlet_c), font(0.3), text);

    // IHX: primary in at the outlet temperature, out at its own outlet.
    let ihx = rect(IHX);
    let n = 12;
    for k in 0..n {
        let f = k as f64 / n as f64;
        let t = s.core_outlet_c + f * (s.ihx_outlet_c - s.core_outlet_c);
        let y0 = IHX[3] - f * (IHX[3] - IHX[1]);
        let y1 = IHX[3] - (f + 1.0 / n as f64) * (IHX[3] - IHX[1]);
        painter.rect_filled(Rect::from_two_pos(to(IHX[0], y0), to(IHX[2], y1)), 0.0, colour(t));
    }
    painter.rect_stroke(ihx, 3.0, Stroke::new(1.5, text), egui::StrokeKind::Inside);
    let eps = if s.ihx_effectiveness.is_finite() { format!("ε {:.3}", s.ihx_effectiveness) } else { "ε –".into() };
    painter.text(to(IHX[2] + 0.15, -0.5), Align2::LEFT_CENTER, format!("IHX\n{eps}\nout {:.0} °C", s.ihx_outlet_c), font(0.3), text);

    // The intermediate loop's connections, out through the tank roof: inlet
    // at its tabulated (or overridden) temperature; the outlet's temperature
    // is not a model output, so it is drawn uncoloured.
    let x_in = IHX[0] + 0.3;
    let x_out = IHX[2] - 0.3;
    let roof = TANK[3] + 0.9;
    painter.line_segment([to(x_in, roof), to(x_in, IHX[3])], Stroke::new(line_w * 0.7, colour(s.intermediate_inlet_c)));
    painter.line_segment([to(x_out, IHX[3]), to(x_out, roof)], Stroke::new(line_w * 0.7, muted));
    arrow(painter, to, &[[x_in, roof], [x_in, IHX[3]]], 0.5, s.intermediate_flow_kg_s, text);
    arrow(painter, to, &[[x_out, IHX[3]], [x_out, roof]], 0.5, s.intermediate_flow_kg_s, text);
    let tag = if s.intermediate_override != 0.0 { " (held)" } else { "" };
    painter.text(
        to(x_in - 0.2, TANK[3] + 0.5),
        Align2::RIGHT_CENTER,
        format!("intermediate loop{tag}: {:.0} kg/s in at {:.0} °C\n(steam side not modelled)", s.intermediate_flow_kg_s, s.intermediate_inlet_c),
        font(0.26),
        text,
    );

    // The two primary pumps (both draw from the pool; mean head in the model).
    for (k, &x) in PUMPS_X.iter().enumerate() {
        let c = to(x, 1.0);
        let r = 0.42 * px;
        painter.circle(c, r, colour(s.core_inlet_c), Stroke::new(1.5, text));
        painter.text(c, Align2::CENTER_CENTER, format!("P{}", k + 1), font(0.28), Color32::BLACK);
    }
    let tag = if s.pump_override != 0.0 { "held" } else { "SHRT table" };
    painter.text(to(-4.4, 0.2), Align2::LEFT_TOP, format!("pumps ({tag})\n{:.0} / {:.0} rpm", s.pump1_rpm, s.pump2_rpm), font(0.28), text);
    if s.aux_em_pump_a > 0.0 {
        painter.text(to(-3.2, -2.0), Align2::LEFT_TOP, format!("aux. EM pump\n{:.0} A", s.aux_em_pump_a), font(0.26), text);
    }

    // Tracers on the segments with an inventory.
    for seg in Segment::ALL {
        let path = seg.path();
        let tau = tr.residence_s(seg);
        if tr.too_fast(seg) {
            let pts: Vec<Pos2> = path.iter().map(|p| to(p[0], p[1])).collect();
            painter.add(egui::Shape::dashed_line(&pts, Stroke::new(line_w * 0.35, Color32::WHITE.gamma_multiply(0.8)), 6.0, 4.0));
            arrow(painter, to, path, 0.5, s.primary_flow_kg_s, Color32::WHITE);
        } else if tau.is_finite() {
            for f in tr.trains[seg.index()].positions() {
                let p = along(path, f);
                painter.circle_filled(to(p[0], p[1]), (0.11 * px).clamp(2.5, 7.0), Color32::WHITE);
            }
        }
        let label = if tau.is_finite() { format!("τ {}", seconds(tau)) } else { "stagnant".into() };
        let at = match seg {
            Segment::Core => to(CORE[2] + 0.1, -2.4),
            Segment::Ihx => to(IHX[2] + 0.15, -1.8),
            Segment::Pool => to(-1.2, -3.6),
        };
        painter.text(at, Align2::LEFT_CENTER, label, font(0.24), muted);
    }
}

/// A residence time in words.
fn seconds(s: f64) -> String {
    if s < 10.0 {
        format!("{s:.2} s")
    } else if s < 600.0 {
        format!("{s:.0} s")
    } else {
        format!("{:.0} min", s / 60.0)
    }
}

/// An arrowhead `f` of the way along `path`, pointing with the flow's sign
/// (none at zero flow). GUI drawing code, exempt as [`paint`].
fn arrow(painter: &egui::Painter, to: &impl Fn(f64, f64) -> Pos2, path: &[[f64; 2]], f: f64, flow: f64, c: Color32) {
    if !(flow.is_finite() && flow != 0.0) {
        return;
    }
    let (a, b) = (along(path, f - 0.02), along(path, f + 0.02));
    let (p, q) = (to(a[0], a[1]), to(b[0], b[1]));
    let d = if flow > 0.0 { q - p } else { p - q };
    let len = d.length().max(1e-3);
    let u = d / len * 9.0;
    let tip = Pos2::new(0.5 * (p.x + q.x), 0.5 * (p.y + q.y)) + u * 0.5;
    let side = Vec2::new(-u.y, u.x) * 0.6;
    painter.add(egui::Shape::convex_polygon(vec![tip, tip - u + side, tip - u - side], c, Stroke::NONE));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(t: f64, flow: f64) -> Snapshot {
        Snapshot { time_s: t, primary_flow_kg_s: flow, core_na_kg: 39.0, ihx_na_kg: 1407.0, pool_na_kg: 291_144.0, ..Default::default() }
    }

    /// The tracers follow the plant, not the screen: residence time is the
    /// segment's sodium over the flow (39 kg at 450 kg/s: 0.0867 s); a 0.2 s
    /// frame at full flow makes the core's marks alias (2.3 passes) but not
    /// the pool's; at 2 % flow the core's slow to 4.3 s and draw as marks; at
    /// zero flow nothing moves; a reset (time going back) moves nothing.
    #[test]
    fn tracers_take_their_speed_from_inventory_and_flow() {
        let mut tr = Tracers::default();
        tr.advance(&snap(0.0, 450.0));
        tr.advance(&snap(0.2, 450.0));
        assert!((tr.residence_s(Segment::Core) - 39.0 / 450.0).abs() < 1e-12);
        assert!(tr.too_fast(Segment::Core));
        assert!(!tr.too_fast(Segment::Pool));
        let pool_phase = tr.trains[Segment::Pool.index()].phase();
        assert!((pool_phase - 0.2 / (291_144.0 / 450.0)).abs() < 1e-12, "{pool_phase}");
        tr.advance(&snap(0.4, 9.0));
        assert!(!tr.too_fast(Segment::Core));
        tr.advance(&snap(0.6, 0.0));
        assert!(!tr.residence_s(Segment::Core).is_finite());
        let before = tr.trains[Segment::Ihx.index()].phase();
        tr.advance(&snap(0.0, 450.0));
        assert_eq!(tr.trains[Segment::Ihx.index()].phase(), before);
        assert_eq!(along(&[[0.0, 0.0], [2.0, 0.0]], 0.25), [0.5, 0.0]);
        assert!(colour(T_HOT_C) != colour(T_COLD_C));
    }
}
