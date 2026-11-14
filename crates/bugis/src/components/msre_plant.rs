//! The Molten-Salt Reactor Experiment on one canvas: a flat schematic of the
//! plant redrawn from ORNL's flow diagram, coloured by the caller's
//! temperatures.
//!
//! **Source.** ORNL-TM-0728 (Robertson, 1965), Fig. 2.1 "MSRE Flow Diagram"
//! (ORNL-DWG 63-1209R), and Fig. 2.2 for the reactor vessel. Fig. 2.1 is an
//! isometric drawing of the reactor cell. This widget keeps its components
//! and their connections, and lays them out flat:
//!
//! | Fig. 2.1 item | Drawn as |
//! |---|---|
//! | 1 reactor vessel | vessel with dished heads, graphite core (vertical stringers), downcomer |
//! | 2 heat exchanger | horizontal shell and U-tube bundle (fuel shell side, coolant tube side, TM-0728 §5.6) |
//! | 3 fuel pump | sump-type pump bowl above the vessel outlet, motor on top |
//! | 4 freeze flanges | paired ticks on the fuel and coolant lines at the cell wall |
//! | 5 thermal shield | ring round the vessel |
//! | 6 coolant pump | pump bowl and motor on the coolant line |
//! | 7 radiator | tube bank in a housing, with the inlet and outlet doors |
//! | 8 coolant drain tank | tank under the radiator, behind a freeze valve |
//! | 9 fans | two blowers beside the radiator |
//! | 10 drain tanks | two fuel drain tanks below the cell |
//! | 11 flush tank | beside the drain tanks |
//! | 12 containment vessel | the reactor cell outline |
//! | 13 freeze valve | the drain line's freeze valve (frozen = closed) |
//!
//! **What is drawn from data and what is a drawing choice.** Every colour is
//! the caller's temperature on the shared hot-to-cold map. Flow arrows point
//! the way the caller's flow sign says, with no arrow at zero flow. The
//! radiator doors open with the caller's air-flow fraction. The salt is
//! hatched as frozen when the caller says so. **The layout, sizes and
//! proportions are indicative**, chosen to read left to right in the order
//! the salt flows. They are not plant dimensions. Nothing moves: there is no
//! animation for the animation rule to derive.
//!
//! Art only (bugis holds no physics, `AGENTS.md`). The caller supplies the
//! numbers from its own model: the `msre_web` example passes
//! `outram_park_digital_twin_engine::msre::session::Snapshot`'s.

use crate::colour_maps::color32::hot_to_cold_colour_mark_1;
use egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Response, Sense, Shape, Stroke, StrokeKind, Ui, Vec2, Widget};
use uom::si::f64::{Power, Ratio, ThermodynamicTemperature};
use uom::si::power::megawatt;
use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::degree_fahrenheit;

/// The layout's extent in drawing units, landscape: 17 wide, 10 tall.
pub const UNITS: (f32, f32) = (17.0, 10.0);

/// Landscape (salt flows left to right) or portrait (the same layout
/// transposed: top to bottom, for a phone held upright).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MsreOrientation {
    Landscape,
    Portrait,
}

/// The plant state the schematic shows. Plain values from the caller's model.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MsrePlantScalars {
    pub power: Power,
    /// Fuel salt in the core (colours the core) and leaving it (the hot leg).
    pub core_fuel: ThermodynamicTemperature,
    pub core_outlet: ThermodynamicTemperature,
    /// Fuel salt back from the heat exchanger (the cold leg, the downcomer).
    pub core_inlet: ThermodynamicTemperature,
    pub graphite: ThermodynamicTemperature,
    /// Coolant salt leaving the heat exchanger (hot) and the radiator (cold).
    pub coolant_hot: ThermodynamicTemperature,
    pub coolant_cold: ThermodynamicTemperature,
    pub radiator_air_out: ThermodynamicTemperature,
    /// Signed flow fractions of full flow: the sign sets each arrow, zero
    /// draws none.
    pub fuel_flow: Ratio,
    pub coolant_flow: Ratio,
    /// Radiator air flow as a fraction of design: opens the doors.
    pub air_flow: Ratio,
    /// The fuel or coolant salt is below its liquidus (the caller decides).
    pub fuel_frozen: bool,
    pub coolant_frozen: bool,
    /// The drain-line freeze valve holds a frozen plug (closed), as in
    /// operation.
    pub freeze_valve_frozen: bool,
    /// The colour scale.
    pub cold: ThermodynamicTemperature,
    pub hot: ThermodynamicTemperature,
}

/// Where each component is drawn, in drawing units (landscape frame).
/// Pure geometry, tested without a window.
#[derive(Debug, Clone, PartialEq)]
pub struct MsreLayout {
    pub containment: Rect,
    pub shield: Rect,
    pub vessel: Rect,
    pub core: Rect,
    pub fuel_pump: Rect,
    pub heat_exchanger: Rect,
    pub coolant_pump: Rect,
    pub radiator: Rect,
    pub fans: [Rect; 2],
    pub drain_tanks: [Rect; 2],
    pub flush_tank: Rect,
    pub coolant_drain_tank: Rect,
    pub drain_freeze_valve: Pos2,
    pub coolant_freeze_valve: Pos2,
    /// Pipe runs: fuel hot leg (vessel -> pump -> heat exchanger), fuel cold
    /// leg (heat exchanger -> vessel inlet), coolant hot (heat exchanger ->
    /// pump -> radiator), coolant cold (radiator -> heat exchanger), the
    /// fuel drain line and the coolant drain line.
    pub fuel_hot: Vec<Pos2>,
    pub fuel_cold: Vec<Pos2>,
    pub coolant_hot: Vec<Pos2>,
    pub coolant_cold: Vec<Pos2>,
    /// Drain lines as separate branches (vessel to header, header to each
    /// tank).
    pub fuel_drain: Vec<Vec<Pos2>>,
    pub coolant_drain: Vec<Vec<Pos2>>,
    /// Freeze flanges (TM-0728 Fig. 2.1 item 4) on the lines.
    pub flanges: Vec<Pos2>,
}

fn r(x0: f32, y0: f32, x1: f32, y1: f32) -> Rect {
    Rect::from_min_max(Pos2::new(x0, y0), Pos2::new(x1, y1))
}

fn p(x: f32, y: f32) -> Pos2 {
    Pos2::new(x, y)
}

impl MsreLayout {
    /// The layout in landscape drawing units (x right, y down, 17 × 10).
    pub fn landscape() -> Self {
        Self {
            containment: r(0.4, 0.9, 6.9, 7.6),
            shield: r(1.3, 2.6, 4.3, 7.2),
            vessel: r(1.8, 3.0, 3.8, 6.8),
            core: r(2.1, 3.7, 3.5, 6.1),
            fuel_pump: r(2.3, 1.3, 3.3, 2.2),
            heat_exchanger: r(4.4, 2.5, 6.5, 3.5),
            coolant_pump: r(8.3, 1.3, 9.3, 2.2),
            radiator: r(11.0, 2.2, 13.8, 6.2),
            fans: [r(14.4, 2.6, 16.4, 4.0), r(14.4, 4.4, 16.4, 5.8)],
            drain_tanks: [r(0.9, 8.3, 2.1, 9.5), r(2.6, 8.3, 3.8, 9.5)],
            flush_tank: r(4.3, 8.5, 5.4, 9.5),
            coolant_drain_tank: r(11.8, 8.3, 13.0, 9.5),
            drain_freeze_valve: p(2.8, 7.95),
            coolant_freeze_valve: p(12.4, 7.6),
            fuel_hot: vec![p(2.8, 3.0), p(2.8, 2.2), p(3.3, 1.75), p(4.0, 1.75), p(4.0, 2.8), p(4.4, 2.8)],
            fuel_cold: vec![p(4.4, 3.25), p(4.1, 3.25), p(4.1, 3.4), p(3.8, 3.4)],
            coolant_hot: vec![p(6.5, 2.75), p(7.6, 2.75), p(7.6, 1.75), p(8.3, 1.75), p(9.3, 1.75), p(10.4, 1.75), p(10.4, 2.6), p(11.0, 2.6)],
            coolant_cold: vec![p(11.0, 5.8), p(10.4, 5.8), p(10.4, 6.6), p(7.6, 6.6), p(7.6, 3.25), p(6.5, 3.25)],
            fuel_drain: vec![
                vec![p(2.8, 6.8), p(2.8, 8.0)],
                vec![p(1.5, 8.3), p(1.5, 8.0), p(4.85, 8.0), p(4.85, 8.5)],
                vec![p(3.2, 8.0), p(3.2, 8.3)],
            ],
            coolant_drain: vec![vec![p(12.4, 6.2), p(12.4, 8.3)]],
            flanges: vec![p(4.2, 1.75), p(4.25, 3.25), p(7.0, 2.75), p(7.0, 3.25)],
        }
    }

    /// The layout in `orientation`, in drawing units: portrait transposes
    /// every point (x ↔ y), so the topology and every containment relation
    /// is unchanged.
    pub fn in_orientation(orientation: MsreOrientation) -> Self {
        let l = Self::landscape();
        match orientation {
            MsreOrientation::Landscape => l,
            MsreOrientation::Portrait => l.transposed(),
        }
    }

    fn transposed(&self) -> Self {
        let tp = |q: Pos2| p(q.y, q.x);
        let tr = |q: Rect| Rect::from_two_pos(tp(q.min), tp(q.max));
        let tv = |v: &[Pos2]| v.iter().map(|&q| tp(q)).collect();
        Self {
            containment: tr(self.containment),
            shield: tr(self.shield),
            vessel: tr(self.vessel),
            core: tr(self.core),
            fuel_pump: tr(self.fuel_pump),
            heat_exchanger: tr(self.heat_exchanger),
            coolant_pump: tr(self.coolant_pump),
            radiator: tr(self.radiator),
            fans: [tr(self.fans[0]), tr(self.fans[1])],
            drain_tanks: [tr(self.drain_tanks[0]), tr(self.drain_tanks[1])],
            flush_tank: tr(self.flush_tank),
            coolant_drain_tank: tr(self.coolant_drain_tank),
            drain_freeze_valve: tp(self.drain_freeze_valve),
            coolant_freeze_valve: tp(self.coolant_freeze_valve),
            fuel_hot: tv(&self.fuel_hot),
            fuel_cold: tv(&self.fuel_cold),
            coolant_hot: tv(&self.coolant_hot),
            coolant_cold: tv(&self.coolant_cold),
            fuel_drain: self.fuel_drain.iter().map(|b| tv(b)).collect(),
            coolant_drain: self.coolant_drain.iter().map(|b| tv(b)).collect(),
            flanges: self.flanges.iter().map(|&q| tp(q)).collect(),
        }
    }

    /// The drawing's extent in units for `orientation`.
    pub fn extent(orientation: MsreOrientation) -> Vec2 {
        match orientation {
            MsreOrientation::Landscape => Vec2::new(UNITS.0, UNITS.1),
            MsreOrientation::Portrait => Vec2::new(UNITS.1, UNITS.0),
        }
    }
}

/// Which way a flow arrow points along a run: `Some(1.0)` forward,
/// `Some(-1.0)` reversed, `None` for no flow (`|flow| < 1e-6`).
pub fn arrow_sign(flow: Ratio) -> Option<f32> {
    let f = flow.get::<ratio>();
    if !f.is_finite() || f.abs() < 1e-6 {
        None
    } else {
        Some(f.signum() as f32)
    }
}

/// `t` on the scale `cold..hot`, clamped to `[0, 1]`.
pub fn hotness(t: ThermodynamicTemperature, cold: ThermodynamicTemperature, hot: ThermodynamicTemperature) -> f32 {
    let (t, c, h) = (t.get::<degree_fahrenheit>(), cold.get::<degree_fahrenheit>(), hot.get::<degree_fahrenheit>());
    if !(h > c) {
        return 0.5;
    }
    ((t - c) / (h - c)).clamp(0.0, 1.0) as f32
}

/// The radiator door opening drawn for an air-flow fraction: the fraction
/// itself, clamped to `[0, 1]`. Indicative: the MSRE set its air flow with
/// the doors and a bypass damper together (TM-0728 §8.4); the drawing shows
/// one opening.
pub fn door_opening(air_flow: Ratio) -> f32 {
    let a = air_flow.get::<ratio>();
    if a.is_finite() {
        a.clamp(0.0, 1.0) as f32
    } else {
        0.0
    }
}

/// The MSRE plant schematic. Draw it with [`Widget`] (it takes its natural
/// size at `scale` points per unit) or into any rect with
/// [`MsrePlantSchematic::paint`].
#[derive(Debug, Clone, PartialEq)]
pub struct MsrePlantSchematic {
    pub scalars: MsrePlantScalars,
    pub orientation: MsreOrientation,
    /// Points per drawing unit when drawn as a widget.
    pub scale: f32,
    pub labels: bool,
}

impl MsrePlantSchematic {
    pub fn new(scalars: MsrePlantScalars) -> Self {
        Self { scalars, orientation: MsreOrientation::Landscape, scale: 40.0, labels: true }
    }

    pub fn with_orientation(mut self, o: MsreOrientation) -> Self {
        self.orientation = o;
        self
    }

    pub fn with_scale(mut self, points_per_unit: f32) -> Self {
        self.scale = points_per_unit;
        self
    }

    pub fn without_labels(mut self) -> Self {
        self.labels = false;
        self
    }

    fn colour(&self, t: ThermodynamicTemperature) -> Color32 {
        hot_to_cold_colour_mark_1(hotness(t, self.scalars.cold, self.scalars.hot))
    }

    /// Draw into `rect`, which should have the drawing's aspect
    /// ([`MsreLayout::extent`]); the drawing is fitted inside it, centred.
    pub fn paint(&self, painter: &Painter, rect: Rect) {
        let ink = crate::Ink::of_ctx(painter.ctx());
        let l = MsreLayout::in_orientation(self.orientation);
        let ext = MsreLayout::extent(self.orientation);
        let k = (rect.width() / ext.x).min(rect.height() / ext.y);
        let origin = rect.center() - ext * k * 0.5;
        let at = |q: Pos2| origin + q.to_vec2() * k;
        let box_ = |q: Rect| Rect::from_min_max(at(q.min), at(q.max));
        let line = Stroke::new((0.035 * k).max(1.0), ink.text);
        let font = |s: f32| FontId::proportional((s * k).clamp(8.0, 22.0));
        let s = &self.scalars;
        let neutral = ink.card;

        // 12 containment vessel and 5 thermal shield.
        painter.rect(box_(l.containment), 0.6 * k, Color32::TRANSPARENT, Stroke::new(line.width, ink.muted), StrokeKind::Inside);
        painter.rect(box_(l.shield), 0.4 * k, Color32::TRANSPARENT, Stroke::new(0.18 * k, ink.border), StrokeKind::Inside);

        // Pipes first, under the components.
        let pipe_w = 0.16 * k;
        let runs = [
            (&l.fuel_hot, s.core_outlet, s.fuel_flow, s.fuel_frozen),
            (&l.fuel_cold, s.core_inlet, s.fuel_flow, s.fuel_frozen),
            (&l.coolant_hot, s.coolant_hot, s.coolant_flow, s.coolant_frozen),
            (&l.coolant_cold, s.coolant_cold, s.coolant_flow, s.coolant_frozen),
        ];
        for (pts, t, flow, frozen) in runs {
            let screen: Vec<Pos2> = pts.iter().map(|&q| at(q)).collect();
            painter.add(Shape::line(screen.clone(), Stroke::new(pipe_w, self.colour(t))));
            if frozen {
                painter.extend(Shape::dashed_line(&screen, Stroke::new(0.05 * k, Color32::WHITE), 0.12 * k, 0.12 * k));
            }
            if let Some(sign) = arrow_sign(flow) {
                arrows(painter, &screen, sign, 0.22 * k, ink.text);
            }
        }
        for (branches, valve) in [(&l.fuel_drain, l.drain_freeze_valve), (&l.coolant_drain, l.coolant_freeze_valve)] {
            for pts in branches {
                let screen: Vec<Pos2> = pts.iter().map(|&q| at(q)).collect();
                painter.add(Shape::line(screen, Stroke::new(0.08 * k, ink.muted)));
            }
            // 13 freeze valve: a flattened section, blue with a frozen plug.
            let v = Rect::from_center_size(at(valve), Vec2::splat(0.32 * k));
            let fill = if s.freeze_valve_frozen { Color32::from_rgb(120, 170, 230) } else { neutral };
            painter.rect(v, 0.05 * k, fill, line, StrokeKind::Inside);
        }
        // 4 freeze flanges: paired ticks across the line.
        for f in &l.flanges {
            let c = at(*f);
            let (dx, dy) = if self.orientation == MsreOrientation::Landscape { (0.06 * k, 0.18 * k) } else { (0.18 * k, 0.06 * k) };
            for sgn in [-1.0, 1.0] {
                let o = Vec2::new(sgn * dx, sgn * if self.orientation == MsreOrientation::Landscape { 0.0 } else { dy / 3.0 });
                let half = if self.orientation == MsreOrientation::Landscape { Vec2::new(0.0, dy) } else { Vec2::new(dx, 0.0) };
                painter.line_segment([c + o - half, c + o + half], line);
            }
        }

        // 1 reactor vessel: downcomer (the inlet salt) round the core.
        let vessel = box_(l.vessel);
        painter.rect(vessel, 0.45 * k, self.colour(s.core_inlet), line, StrokeKind::Inside);
        let core = box_(l.core);
        painter.rect(core, 0.15 * k, self.colour(s.core_fuel), line, StrokeKind::Inside);
        // The graphite stringers, coloured by the graphite temperature.
        let n = 6;
        let g = self.colour(s.graphite);
        for i in 0..n {
            let f = (i as f32 + 0.5) / n as f32;
            if self.orientation == MsreOrientation::Landscape {
                let x = core.left() + f * core.width();
                painter.line_segment([Pos2::new(x, core.top() + 0.1 * k), Pos2::new(x, core.bottom() - 0.1 * k)], Stroke::new(0.11 * k, g));
            } else {
                let y = core.top() + f * core.height();
                painter.line_segment([Pos2::new(core.left() + 0.1 * k, y), Pos2::new(core.right() - 0.1 * k, y)], Stroke::new(0.11 * k, g));
            }
        }
        if s.fuel_frozen {
            hatch(painter, vessel, 0.25 * k, Color32::WHITE);
        }

        // 3 fuel pump and 6 coolant pump: bowl, shaft and motor.
        for (bowl, t, frozen) in [(l.fuel_pump, s.core_outlet, s.fuel_frozen), (l.coolant_pump, s.coolant_hot, s.coolant_frozen)] {
            let b = box_(bowl);
            painter.rect(b, 0.25 * k, self.colour(t), line, StrokeKind::Inside);
            if frozen {
                hatch(painter, b, 0.2 * k, Color32::WHITE);
            }
            let (shaft_from, shaft_to, motor) = if self.orientation == MsreOrientation::Landscape {
                (b.center_top(), b.center_top() - Vec2::new(0.0, 0.35 * k), Rect::from_center_size(b.center_top() - Vec2::new(0.0, 0.6 * k), Vec2::new(0.55 * k, 0.4 * k)))
            } else {
                (b.left_center(), b.left_center() - Vec2::new(0.35 * k, 0.0), Rect::from_center_size(b.left_center() - Vec2::new(0.6 * k, 0.0), Vec2::new(0.4 * k, 0.55 * k)))
            };
            painter.line_segment([shaft_from, shaft_to], line);
            painter.rect(motor, 0.05 * k, neutral, line, StrokeKind::Inside);
        }

        // 2 heat exchanger: shell (fuel) with the U-tube bundle (coolant).
        let hx = box_(l.heat_exchanger);
        painter.rect(hx, 0.5 * k.min(hx.height() / 2.0), self.colour(ThermodynamicTemperature::new::<degree_fahrenheit>(0.5 * (s.core_inlet.get::<degree_fahrenheit>() + s.core_outlet.get::<degree_fahrenheit>()))), line, StrokeKind::Inside);
        let tube = Stroke::new(0.07 * k, self.colour(s.coolant_hot));
        let inset = 0.25 * k;
        if self.orientation == MsreOrientation::Landscape {
            let (y0, y1) = (hx.top() + 0.3 * hx.height(), hx.bottom() - 0.3 * hx.height());
            painter.line_segment([Pos2::new(hx.left() + inset, y0), Pos2::new(hx.right(), y0)], tube);
            painter.line_segment([Pos2::new(hx.left() + inset, y1), Pos2::new(hx.right(), y1)], tube);
            painter.line_segment([Pos2::new(hx.left() + inset, y0), Pos2::new(hx.left() + inset, y1)], tube);
        } else {
            let (x0, x1) = (hx.left() + 0.3 * hx.width(), hx.right() - 0.3 * hx.width());
            painter.line_segment([Pos2::new(x0, hx.top() + inset), Pos2::new(x0, hx.bottom())], tube);
            painter.line_segment([Pos2::new(x1, hx.top() + inset), Pos2::new(x1, hx.bottom())], tube);
            painter.line_segment([Pos2::new(x0, hx.top() + inset), Pos2::new(x1, hx.top() + inset)], tube);
        }

        // 7 radiator: the tube bank in its housing, the doors on the air
        // faces opening with the air flow.
        let rad = box_(l.radiator);
        painter.rect(rad, 0.1 * k, neutral, line, StrokeKind::Inside);
        let bank = rad.shrink(0.35 * k);
        let rows = 7;
        for i in 0..rows {
            let f = (i as f32 + 0.5) / rows as f32;
            let t_row = ThermodynamicTemperature::new::<degree_fahrenheit>(
                s.coolant_hot.get::<degree_fahrenheit>() + f as f64 * (s.coolant_cold.get::<degree_fahrenheit>() - s.coolant_hot.get::<degree_fahrenheit>()),
            );
            let st = Stroke::new(0.12 * k, self.colour(t_row));
            if self.orientation == MsreOrientation::Landscape {
                let y = bank.top() + f * bank.height();
                painter.line_segment([Pos2::new(bank.left(), y), Pos2::new(bank.right(), y)], st);
            } else {
                let x = bank.left() + f * bank.width();
                painter.line_segment([Pos2::new(x, bank.top()), Pos2::new(x, bank.bottom())], st);
            }
        }
        if s.coolant_frozen {
            hatch(painter, bank, 0.25 * k, Color32::WHITE);
        }
        let open = door_opening(s.air_flow);
        // The doors as shutters across the air-side face (right in
        // landscape, bottom in portrait), closing from both ends: the
        // uncovered middle is the open fraction.
        let shut = Color32::from_rgba_unmultiplied(ink.warn.r(), ink.warn.g(), ink.warn.b(), 220);
        let t = 0.18 * k;
        if self.orientation == MsreOrientation::Landscape {
            let face = Rect::from_min_max(Pos2::new(rad.right(), rad.top()), Pos2::new(rad.right() + t, rad.bottom()));
            let c = 0.5 * (1.0 - open) * face.height();
            painter.rect_filled(Rect::from_min_max(face.min, Pos2::new(face.max.x, face.min.y + c)), 0.0, shut);
            painter.rect_filled(Rect::from_min_max(Pos2::new(face.min.x, face.max.y - c), face.max), 0.0, shut);
            painter.rect_stroke(face, 0.0, Stroke::new(1.0, ink.warn), StrokeKind::Inside);
        } else {
            let face = Rect::from_min_max(Pos2::new(rad.left(), rad.bottom()), Pos2::new(rad.right(), rad.bottom() + t));
            let c = 0.5 * (1.0 - open) * face.width();
            painter.rect_filled(Rect::from_min_max(face.min, Pos2::new(face.min.x + c, face.max.y)), 0.0, shut);
            painter.rect_filled(Rect::from_min_max(Pos2::new(face.max.x - c, face.min.y), face.max), 0.0, shut);
            painter.rect_stroke(face, 0.0, Stroke::new(1.0, ink.warn), StrokeKind::Inside);
        }

        // 9 fans: two blowers, coloured by the air leaving the radiator.
        for f in l.fans {
            let fr = box_(f);
            painter.circle(fr.center(), 0.45 * fr.width().min(fr.height()), neutral, line);
            painter.circle_filled(fr.center(), 0.18 * fr.width().min(fr.height()), self.colour(s.radiator_air_out));
        }

        // 10 drain tanks, 11 flush tank, 8 coolant drain tank: empty in
        // operation (the model holds the salt in the loops).
        for t in [l.drain_tanks[0], l.drain_tanks[1], l.flush_tank, l.coolant_drain_tank] {
            painter.rect(box_(t), 0.3 * k, neutral, line, StrokeKind::Inside);
        }

        if self.labels {
            let txt = |q: Pos2, a: Align2, s: &str, size: f32| {
                painter.text(at(q), a, s, font(size), ink.text);
            };
            let ls = &l;
            let f_ = |t: ThermodynamicTemperature| format!("{:.0} °F", t.get::<degree_fahrenheit>());
            if self.orientation == MsreOrientation::Landscape {
                txt(ls.containment.left_top() + Vec2::new(0.2, 0.15), Align2::LEFT_TOP, "reactor cell", 0.28);
                txt(ls.core.center(), Align2::CENTER_CENTER, &format!("{:.2} MW", s.power.get::<megawatt>()), 0.36);
                txt(ls.vessel.left_center() + Vec2::new(-0.1, 0.0), Align2::RIGHT_CENTER, "core", 0.3);
                txt(ls.fuel_pump.left_center() + Vec2::new(-0.15, 0.0), Align2::RIGHT_CENTER, "fuel pump", 0.28);
                txt(ls.heat_exchanger.center_bottom() + Vec2::new(0.0, 0.1), Align2::CENTER_TOP, "heat exchanger", 0.28);
                txt(ls.coolant_pump.center_bottom() + Vec2::new(0.0, 0.1), Align2::CENTER_TOP, "coolant pump", 0.28);
                txt(ls.radiator.center_top() + Vec2::new(0.0, -0.1), Align2::CENTER_BOTTOM, "radiator", 0.3);
                txt(ls.fans[1].center_bottom() + Vec2::new(0.0, 0.1), Align2::CENTER_TOP, "blowers", 0.26);
                txt(p(3.15, 9.98), Align2::CENTER_BOTTOM, "fuel drain + flush tanks", 0.24);
                txt(ls.coolant_drain_tank.right_center() + Vec2::new(0.15, 0.0), Align2::LEFT_CENTER, "coolant drain tank", 0.24);
                txt(ls.drain_freeze_valve + Vec2::new(0.3, -0.05), Align2::LEFT_BOTTOM, if s.freeze_valve_frozen { "freeze valve (frozen)" } else { "freeze valve (thawed)" }, 0.22);
                // Temperatures on the runs.
                txt(p(3.7, 1.55), Align2::LEFT_BOTTOM, &f_(s.core_outlet), 0.28);
                txt(p(4.0, 3.75), Align2::CENTER_TOP, &f_(s.core_inlet), 0.28);
                txt(p(9.8, 1.6), Align2::CENTER_BOTTOM, &f_(s.coolant_hot), 0.28);
                txt(p(9.0, 6.75), Align2::CENTER_TOP, &f_(s.coolant_cold), 0.28);
                txt(ls.fans[0].center_top() + Vec2::new(0.0, -0.1), Align2::CENTER_BOTTOM, &format!("air out {}", f_(s.radiator_air_out)), 0.24);
                txt(ls.radiator.center_bottom() + Vec2::new(0.0, 0.1), Align2::CENTER_TOP, &format!("doors {:.0} %", 100.0 * open), 0.24);
            } else {
                txt(ls.core.center(), Align2::CENTER_CENTER, &format!("{:.2} MW", s.power.get::<megawatt>()), 0.36);
                txt(ls.vessel.center_top() + Vec2::new(0.0, -0.1), Align2::CENTER_BOTTOM, "core", 0.3);
                txt(ls.heat_exchanger.right_center() + Vec2::new(0.1, 0.0), Align2::LEFT_CENTER, "heat exch.", 0.28);
                txt(ls.coolant_pump.right_center() + Vec2::new(0.1, 0.0), Align2::LEFT_CENTER, "coolant pump", 0.28);
                txt(ls.radiator.left_center() + Vec2::new(-0.1, 0.0), Align2::RIGHT_CENTER, "radiator", 0.3);
                txt(ls.fans[1].center_bottom() + Vec2::new(0.0, 0.1), Align2::CENTER_TOP, "blowers", 0.26);
                txt(ls.drain_tanks[1].center_bottom() + Vec2::new(0.0, 0.05), Align2::CENTER_TOP, "drain + flush tanks", 0.22);
                txt(p(1.6, 3.6), Align2::RIGHT_CENTER, &f_(s.core_outlet), 0.26);
                txt(p(3.75, 4.0), Align2::LEFT_CENTER, &f_(s.core_inlet), 0.26);
                txt(p(1.6, 9.8), Align2::RIGHT_CENTER, &f_(s.coolant_hot), 0.26);
                txt(p(6.75, 9.0), Align2::LEFT_CENTER, &f_(s.coolant_cold), 0.26);
                txt(ls.radiator.center_bottom() + Vec2::new(0.0, 0.1), Align2::CENTER_TOP, &format!("doors {:.0} %", 100.0 * open), 0.24);
            }
        }
    }
}

/// Arrowheads at each segment's midpoint, pointing along the run for
/// `sign = 1`, against it for `-1`.
fn arrows(painter: &Painter, pts: &[Pos2], sign: f32, size: f32, colour: Color32) {
    for w in pts.windows(2) {
        let d = w[1] - w[0];
        let len = d.length();
        if len < 2.5 * size {
            continue;
        }
        let u = d / len * sign;
        let m = w[0] + d * 0.5;
        let n = Vec2::new(-u.y, u.x);
        let tip = m + u * size * 0.6;
        let back = m - u * size * 0.6;
        painter.add(Shape::convex_polygon(vec![tip, back + n * size * 0.5, back - n * size * 0.5], colour, Stroke::NONE));
    }
}

/// Diagonal hatching over `rect`, `gap` apart: the frozen-salt mark.
fn hatch(painter: &Painter, rect: Rect, gap: f32, colour: Color32) {
    let painter = painter.with_clip_rect(rect);
    let span = rect.width() + rect.height();
    let mut s = 0.0;
    while s < span {
        let a = rect.left_top() + Vec2::new(s, 0.0);
        painter.line_segment([a, a + Vec2::new(-rect.height(), rect.height())], Stroke::new(1.5, colour));
        s += gap.max(3.0);
    }
}

impl Widget for MsrePlantSchematic {
    fn ui(self, ui: &mut Ui) -> Response {
        let size = MsreLayout::extent(self.orientation) * self.scale;
        let (rect, resp) = ui.allocate_exact_size(size, Sense::hover());
        if ui.is_rect_visible(rect) {
            self.paint(ui.painter(), rect);
        }
        resp
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rects(l: &MsreLayout) -> Vec<Rect> {
        let mut v = vec![
            l.containment,
            l.shield,
            l.vessel,
            l.core,
            l.fuel_pump,
            l.heat_exchanger,
            l.coolant_pump,
            l.radiator,
            l.flush_tank,
            l.coolant_drain_tank,
        ];
        v.extend(l.fans);
        v.extend(l.drain_tanks);
        v
    }

    /// Every component and every pipe point lies inside the drawing's
    /// extent, in both orientations.
    #[test]
    fn everything_is_inside_the_drawing() {
        for o in [MsreOrientation::Landscape, MsreOrientation::Portrait] {
            let l = MsreLayout::in_orientation(o);
            let e = Rect::from_min_size(Pos2::ZERO, MsreLayout::extent(o));
            for q in rects(&l) {
                assert!(e.contains_rect(q), "{q:?} outside {e:?} ({o:?})");
            }
            let drains: Vec<&Vec<Pos2>> = l.fuel_drain.iter().chain(l.coolant_drain.iter()).collect();
            for run in [&l.fuel_hot, &l.fuel_cold, &l.coolant_hot, &l.coolant_cold].into_iter().chain(drains) {
                for q in run {
                    assert!(e.contains(*q), "{q:?} outside {e:?} ({o:?})");
                }
            }
        }
    }

    /// The nesting of Fig. 2.1 holds in both orientations: core in vessel in
    /// shield in containment; the heat exchanger and fuel pump inside the
    /// cell; the coolant loop, radiator, fans and every tank outside it.
    #[test]
    fn the_fig_2_1_nesting_holds() {
        for o in [MsreOrientation::Landscape, MsreOrientation::Portrait] {
            let l = MsreLayout::in_orientation(o);
            assert!(l.vessel.contains_rect(l.core));
            assert!(l.shield.contains_rect(l.vessel));
            assert!(l.containment.contains_rect(l.shield));
            assert!(l.containment.contains_rect(l.heat_exchanger));
            assert!(l.containment.contains_rect(l.fuel_pump));
            for q in [l.coolant_pump, l.radiator, l.fans[0], l.fans[1], l.drain_tanks[0], l.drain_tanks[1], l.flush_tank, l.coolant_drain_tank] {
                assert!(!l.containment.intersects(q), "{q:?} ({o:?})");
            }
        }
    }

    /// The runs join the components they connect: the fuel hot leg leaves
    /// the vessel and reaches the heat exchanger, the cold leg returns to the
    /// vessel; the coolant runs join the heat exchanger and the radiator.
    #[test]
    fn the_runs_join_their_components() {
        let l = MsreLayout::landscape();
        let touches = |q: Pos2, r: Rect| r.expand(0.01).contains(q);
        assert!(touches(l.fuel_hot[0], l.vessel));
        assert!(touches(*l.fuel_hot.last().unwrap(), l.heat_exchanger));
        assert!(l.fuel_hot.iter().any(|&q| touches(q, l.fuel_pump)));
        assert!(touches(l.fuel_cold[0], l.heat_exchanger));
        assert!(touches(*l.fuel_cold.last().unwrap(), l.vessel));
        assert!(touches(l.coolant_hot[0], l.heat_exchanger));
        assert!(l.coolant_hot.iter().any(|&q| touches(q, l.coolant_pump)));
        assert!(touches(*l.coolant_hot.last().unwrap(), l.radiator));
        assert!(touches(l.coolant_cold[0], l.radiator));
        assert!(touches(*l.coolant_cold.last().unwrap(), l.heat_exchanger));
        assert!(touches(l.fuel_drain[0][0], l.vessel));
        // Every drain tank and the flush tank is reached by a branch end.
        let ends: Vec<Pos2> = l.fuel_drain.iter().flat_map(|b| [b[0], *b.last().unwrap()]).collect();
        for t in [l.drain_tanks[0], l.drain_tanks[1], l.flush_tank] {
            assert!(ends.iter().any(|&q| touches(q, t)), "{t:?}");
        }
        assert!(touches(*l.coolant_drain[0].last().unwrap(), l.coolant_drain_tank));
    }

    /// Arrows follow the flow's sign and vanish at zero; the colour scale
    /// and the door opening clamp.
    #[test]
    fn arrows_colours_and_doors_follow_the_state() {
        assert_eq!(arrow_sign(Ratio::new::<ratio>(0.7)), Some(1.0));
        assert_eq!(arrow_sign(Ratio::new::<ratio>(-0.2)), Some(-1.0));
        assert_eq!(arrow_sign(Ratio::new::<ratio>(0.0)), None);
        assert_eq!(arrow_sign(Ratio::new::<ratio>(f64::NAN)), None);
        let f = ThermodynamicTemperature::new::<degree_fahrenheit>;
        assert_eq!(hotness(f(700.0), f(800.0), f(1300.0)), 0.0);
        assert_eq!(hotness(f(1400.0), f(800.0), f(1300.0)), 1.0);
        assert!((hotness(f(1050.0), f(800.0), f(1300.0)) - 0.5).abs() < 1e-6);
        assert_eq!(hotness(f(1000.0), f(900.0), f(900.0)), 0.5);
        assert_eq!(door_opening(Ratio::new::<ratio>(1.7)), 1.0);
        assert_eq!(door_opening(Ratio::new::<ratio>(0.3)), 0.3);
        assert_eq!(door_opening(Ratio::new::<ratio>(f64::NAN)), 0.0);
    }

    /// The widget paints in both orientations without panicking, and
    /// allocates its extent times its scale.
    #[test]
    fn it_paints_in_both_orientations() {
        let f = ThermodynamicTemperature::new::<degree_fahrenheit>;
        let s = MsrePlantScalars {
            power: Power::new::<megawatt>(8.0),
            core_fuel: f(1150.0),
            core_outlet: f(1170.0),
            core_inlet: f(1130.0),
            graphite: f(1160.0),
            coolant_hot: f(1060.0),
            coolant_cold: f(990.0),
            radiator_air_out: f(300.0),
            fuel_flow: Ratio::new::<ratio>(1.0),
            coolant_flow: Ratio::new::<ratio>(-1.0),
            air_flow: Ratio::new::<ratio>(0.8),
            fuel_frozen: true,
            coolant_frozen: true,
            freeze_valve_frozen: false,
            cold: f(800.0),
            hot: f(1300.0),
        };
        let ctx = egui::Context::default();
        for o in [MsreOrientation::Landscape, MsreOrientation::Portrait] {
            let mut size = Vec2::ZERO;
            let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
                size = ui.add(MsrePlantSchematic::new(s).with_orientation(o).with_scale(10.0)).rect.size();
                MsrePlantSchematic::new(s).with_orientation(o).without_labels().paint(ui.painter(), Rect::from_min_size(Pos2::ZERO, Vec2::new(300.0, 300.0)));
            });
            assert_eq!(size, MsreLayout::extent(o) * 10.0);
        }
    }
}
