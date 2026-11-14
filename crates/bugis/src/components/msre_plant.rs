//! The Molten-Salt Reactor Experiment on one canvas: an elevation view of
//! the plant, roughly to scale, coloured by the caller's temperatures.
//!
//! **Drawn from dimensions, not traced from a drawing.** Every size and
//! elevation below is a number stated in the text of ORNL-TM-0728
//! (Robertson, 1965, a public US government report) or in this workspace's
//! sourced MSRE geometry (`outram_mc_libs::vv::msre_n1`). No ORNL figure is
//! reproduced; the arrangement is this widget's own. Units are feet; the
//! vertical axis is plant elevation (ft above sea level), so **what is
//! higher in the drawing is higher in the plant**, and the salt's drain path
//! to the tanks runs downhill as it did in the real plant (the cells were
//! laid out at "relative elevations so that the reactor could drain by
//! gravity", TM-0728 §6).
//!
//! | Item | Size / elevation used | Source |
//! |---|---|---|
//! | reactor cell | 24 ft ID, 33 ft high, el. 819-852 ft, hemispherical bottom | TM-0728 §4.3.1 |
//! | thermal shield | 125 in OD; base 823.5-825 ft, 151 in cylinder, 16 in cover | TM-0728 §5.3.10.1; `msre_n1::SHIELD_*` |
//! | reactor vessel | 59⅛ in OD, 23-114 in above the shield cylinder's bottom | TM-0728 Table 5.2 and Fig. 5.2 callouts (`msre_n1`) |
//! | graphite core | 55¼ in diameter, 64 in high, centred in the vessel | TM-0728 Table 5.4 (`msre_n1::R_GRAPHITE`, `H_CORE`) |
//! | fuel pump | bowl ~36 in diameter, pump + motor ~8 ft high; salt level ~840 ft | TM-0728 §2.4.2, §5.4.7.1 |
//! | heat exchanger | ~16 in diameter, ~8 ft long, horizontal | TM-0728 §2.4.3 |
//! | coolant pump | bowl centreline 833 ft 6 in, the coolant loop's highest point | TM-0728 §8 |
//! | radiator | bottom at 836 ft 1¾ in | TM-0728 §4.3 |
//! | drain tank cell | 17 ft 7 in wide, 20 ft 6 in high, floor 814 ft | TM-0728 §4.3.2, §11 |
//! | drain tanks | 50 in diameter, ~86 in high | TM-0728 §6 |
//! | coolant drain tank | at the 820 ft elevation, almost directly beneath the radiator | TM-0728 §8.1, §9.1 |
//!
//! **What is indicative, not dimensioned.** Horizontal positions are a
//! flattened arrangement chosen to read left to right (drain cell, reactor
//! cell, coolant cell); the real cells sit side by side in plan, not in one
//! plane. Not stated in the text and chosen here: the heat exchanger's
//! elevation (placed below the coolant pump bowl, which TM-0728 says is the
//! coolant loop's highest point), the flush tank's size (drawn as a drain
//! tank), the coolant drain tank's size, the radiator housing's size, and
//! the blowers' place (in the plant they stand in a separate blower house,
//! ducted to the radiator).
//!
//! **Loop order** (TM-0728 §8.2): the coolant pump discharges into line 200
//! to the heat exchanger; line 201 takes the salt to the radiator's top
//! header; it flows down through the radiator and back to the pump.
//!
//! **What is drawn from data.** Every colour is the caller's temperature on
//! the shared hot-to-cold map. Flow arrows point the way the caller's flow
//! sign says, with no arrow at zero flow. The radiator doors open with the
//! caller's air-flow fraction. Frozen salt is hatched when the caller says
//! so. Nothing moves: there is no animation for the animation rule to
//! derive.
//!
//! 100 % egui: the drawing is `Painter` shapes, with no image or SVG.
//!
//! Art only (bugis holds no physics, `AGENTS.md`). The caller supplies the
//! numbers from its own model: the `msre_web` example passes
//! `outram_park_digital_twin_engine::msre::session::Snapshot`'s.

use crate::colour_maps::color32::hot_to_cold_colour_mark_1;
use egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Response, Sense, Shape, Stroke, StrokeKind, Ui, Vec2, Widget};
use petir::mathf::RealMath;
use uom::si::f64::{Power, Ratio, ThermodynamicTemperature};
use uom::si::power::megawatt;
use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::degree_fahrenheit;

/// The drawing's extent in feet: 63 wide, 40 tall (elevations 813 to
/// 853 ft).
pub const EXTENT_FT: (f32, f32) = (63.0, 40.0);

/// The elevation at the drawing's top edge, ft.
pub const TOP_ELEVATION_FT: f32 = 853.0;

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
    /// Coolant salt leaving the heat exchanger (hot, line 201) and the
    /// radiator (cold, back through the pump to the heat exchanger).
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
    /// The drain-line freeze valves hold a frozen plug (closed), as in
    /// operation.
    pub freeze_valve_frozen: bool,
    /// The colour scale.
    pub cold: ThermodynamicTemperature,
    pub hot: ThermodynamicTemperature,
}

/// Where each component is drawn, in feet: x to the right, y down from
/// [`TOP_ELEVATION_FT`] (so `y = 853 - elevation`). Pure geometry, tested
/// without a window.
#[derive(Debug, Clone, PartialEq)]
pub struct MsreLayout {
    /// The reactor cell's bounding box; drawn with its hemispherical bottom
    /// ([`MsreLayout::reactor_cell_outline`]).
    pub containment: Rect,
    pub drain_cell: Rect,
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
    /// Pipe runs: fuel hot leg (vessel -> fuel pump -> heat exchanger), fuel
    /// cold leg (heat exchanger -> vessel inlet), coolant hot (heat
    /// exchanger -> radiator top, line 201), coolant cold (radiator bottom
    /// -> coolant pump -> heat exchanger, line 200).
    pub fuel_hot: Vec<Pos2>,
    pub fuel_cold: Vec<Pos2>,
    pub coolant_hot: Vec<Pos2>,
    pub coolant_cold: Vec<Pos2>,
    /// Drain lines as separate branches (vessel to header, header to each
    /// tank).
    pub fuel_drain: Vec<Vec<Pos2>>,
    pub coolant_drain: Vec<Vec<Pos2>>,
}

const IN: f32 = 1.0 / 12.0;

/// `y` in the drawing for a plant elevation in ft.
pub fn y_of_elevation(elevation_ft: f32) -> f32 {
    TOP_ELEVATION_FT - elevation_ft
}

/// A rect from an x range and an elevation range (ft).
fn re(x0: f32, x1: f32, el_lo: f32, el_hi: f32) -> Rect {
    Rect::from_min_max(Pos2::new(x0, y_of_elevation(el_hi)), Pos2::new(x1, y_of_elevation(el_lo)))
}

/// A point from x and elevation (ft).
fn pe(x: f32, el: f32) -> Pos2 {
    Pos2::new(x, y_of_elevation(el))
}

// Sourced numbers (see the module table).
const CELL_X0: f32 = 19.0;
const CELL_ID: f32 = 24.0;
const CELL_BOTTOM_EL: f32 = 819.0;
const CELL_TOP_EL: f32 = 852.0;
const SHIELD_OD: f32 = 125.0 * IN;
const SHIELD_BASE_EL: f32 = 823.5;
const SHIELD_CYL_BOTTOM_EL: f32 = 825.0;
const SHIELD_CYL_HEIGHT: f32 = 151.0 * IN;
const SHIELD_COVER: f32 = 16.0 * IN;
const VESSEL_OD: f32 = 59.125 * IN;
const VESSEL_BOTTOM_ABOVE_CYL: f32 = 23.0 * IN;
const VESSEL_TOP_ABOVE_CYL: f32 = 114.0 * IN;
const CORE_D: f32 = 55.25 * IN;
const CORE_H: f32 = 64.0 * IN;
const PUMP_BOWL_D: f32 = 36.0 * IN;
const PUMP_ASSEMBLY_H: f32 = 8.0;
const FUEL_SALT_LEVEL_EL: f32 = 840.0;
const HX_D: f32 = 16.0 * IN;
const HX_L: f32 = 8.0;
const COOLANT_BOWL_EL: f32 = 833.5;
const RADIATOR_BOTTOM_EL: f32 = 836.0 + 1.75 * IN;
const DRAIN_CELL_W: f32 = 17.0 + 7.0 * IN;
const DRAIN_CELL_H: f32 = 20.5;
const DRAIN_CELL_FLOOR_EL: f32 = 814.0;
const DRAIN_TANK_D: f32 = 50.0 * IN;
const DRAIN_TANK_H: f32 = 86.0 * IN;
const COOLANT_DRAIN_EL: f32 = 820.0;

// Indicative (not stated in TM-0728's text; see the module docs).
const VESSEL_X: f32 = 28.0;
const HX_EL: f32 = 832.0;
const HX_X0: f32 = 34.5;
const COOLANT_PUMP_X: f32 = 47.5;
const RADIATOR_HOUSING_H: f32 = 8.0;

impl MsreLayout {
    /// The layout in feet (x right, y down from el. 853 ft, 63 × 40).
    pub fn new() -> Self {
        let shield_cyl_top = SHIELD_CYL_BOTTOM_EL + SHIELD_CYL_HEIGHT;
        let v_lo = SHIELD_CYL_BOTTOM_EL + VESSEL_BOTTOM_ABOVE_CYL;
        let v_hi = SHIELD_CYL_BOTTOM_EL + VESSEL_TOP_ABOVE_CYL;
        let v_mid = 0.5 * (v_lo + v_hi);
        let bowl_lo = FUEL_SALT_LEVEL_EL - 1.0;
        let rad_top_el = RADIATOR_BOTTOM_EL + RADIATOR_HOUSING_H;
        let tank_lo = DRAIN_CELL_FLOOR_EL + 1.0;
        let tank_top = tank_lo + DRAIN_TANK_H;
        let tank = |x0: f32| re(x0, x0 + DRAIN_TANK_D, tank_lo, tank_top);
        let header_el = tank_top + 1.4;
        let tank_x = [2.0, 7.5, 13.0];
        let tank_cx = tank_x.map(|x| x + 0.5 * DRAIN_TANK_D);
        let hx_right = HX_X0 + HX_L;
        Self {
            containment: re(CELL_X0, CELL_X0 + CELL_ID, CELL_BOTTOM_EL, CELL_TOP_EL),
            drain_cell: re(0.5, 0.5 + DRAIN_CELL_W, DRAIN_CELL_FLOOR_EL, DRAIN_CELL_FLOOR_EL + DRAIN_CELL_H),
            shield: re(VESSEL_X - 0.5 * SHIELD_OD, VESSEL_X + 0.5 * SHIELD_OD, SHIELD_BASE_EL, shield_cyl_top + SHIELD_COVER),
            vessel: re(VESSEL_X - 0.5 * VESSEL_OD, VESSEL_X + 0.5 * VESSEL_OD, v_lo, v_hi),
            core: re(VESSEL_X - 0.5 * CORE_D, VESSEL_X + 0.5 * CORE_D, v_mid - 0.5 * CORE_H, v_mid + 0.5 * CORE_H),
            fuel_pump: re(VESSEL_X - 0.5 * PUMP_BOWL_D, VESSEL_X + 0.5 * PUMP_BOWL_D, bowl_lo, bowl_lo + 0.7 * PUMP_BOWL_D),
            heat_exchanger: re(HX_X0, hx_right, HX_EL - 0.5 * HX_D, HX_EL + 0.5 * HX_D),
            coolant_pump: re(
                COOLANT_PUMP_X - 0.5 * PUMP_BOWL_D,
                COOLANT_PUMP_X + 0.5 * PUMP_BOWL_D,
                COOLANT_BOWL_EL - 0.35 * PUMP_BOWL_D,
                COOLANT_BOWL_EL + 0.35 * PUMP_BOWL_D,
            ),
            radiator: re(50.0, 57.5, RADIATOR_BOTTOM_EL, rad_top_el),
            fans: [re(59.2, 62.2, 840.3, 843.3), re(59.2, 62.2, 836.6, 839.6)],
            drain_tanks: [tank(tank_x[0]), tank(tank_x[1])],
            flush_tank: tank(tank_x[2]),
            coolant_drain_tank: re(54.5, 58.5, COOLANT_DRAIN_EL, COOLANT_DRAIN_EL + 5.0),
            drain_freeze_valve: pe(17.0, header_el),
            coolant_freeze_valve: pe(56.5, 829.0),
            fuel_hot: vec![pe(VESSEL_X, v_hi), pe(VESSEL_X, bowl_lo + 0.4), pe(41.0, bowl_lo + 0.4), pe(41.0, HX_EL + 0.5 * HX_D)],
            fuel_cold: vec![pe(HX_X0, HX_EL - 0.1), pe(VESSEL_X + 0.5 * VESSEL_OD, HX_EL - 0.1)],
            coolant_hot: vec![pe(hx_right, HX_EL + 0.3), pe(44.4, HX_EL + 0.3), pe(44.4, rad_top_el + 1.0), pe(52.0, rad_top_el + 1.0), pe(52.0, rad_top_el)],
            coolant_cold: vec![
                pe(51.25, RADIATOR_BOTTOM_EL),
                pe(51.25, COOLANT_BOWL_EL),
                pe(COOLANT_PUMP_X, COOLANT_BOWL_EL),
                pe(COOLANT_PUMP_X, HX_EL - 1.4),
                pe(43.6, HX_EL - 1.4),
                pe(43.6, HX_EL - 0.3),
                pe(hx_right, HX_EL - 0.3),
            ],
            fuel_drain: vec![
                vec![pe(VESSEL_X, v_lo), pe(VESSEL_X, header_el), pe(tank_cx[0], header_el)],
                vec![pe(tank_cx[0], header_el), pe(tank_cx[0], tank_top)],
                vec![pe(tank_cx[1], header_el), pe(tank_cx[1], tank_top)],
                vec![pe(tank_cx[2], header_el), pe(tank_cx[2], tank_top)],
            ],
            coolant_drain: vec![vec![pe(56.5, RADIATOR_BOTTOM_EL), pe(56.5, COOLANT_DRAIN_EL + 5.0)]],
        }
    }

    /// The reactor cell outline: flat top, vertical sides, hemispherical
    /// bottom (24 ft ID, el. 819-852 ft).
    pub fn reactor_cell_outline(&self) -> Vec<Pos2> {
        let c = self.containment;
        let r = 0.5 * c.width();
        let centre = Pos2::new(c.center().x, c.bottom() - r);
        let mut v = vec![c.left_top(), c.right_top(), Pos2::new(c.right(), centre.y)];
        let n = 24;
        for i in 1..n {
            let a = core::f32::consts::PI * i as f32 / n as f32;
            v.push(Pos2::new(centre.x + r * a.r_cos(), centre.y + r * a.r_sin()));
        }
        v.push(Pos2::new(c.left(), centre.y));
        v
    }

    /// The drawing's extent in feet.
    pub fn extent() -> Vec2 {
        Vec2::new(EXTENT_FT.0, EXTENT_FT.1)
    }

    /// The half-extent a square-fitting view (`web_demo::View`, which fits
    /// `2 * half_extent` into the smaller side of the screen) needs so the
    /// whole drawing fits a screen of `size`, whatever its aspect.
    pub fn half_extent_to_fit(size: Vec2) -> f64 {
        let (w, h) = (size.x.max(1.0) as f64, size.y.max(1.0) as f64);
        let m = w.min(h);
        (0.5 * EXTENT_FT.0 as f64 * m / w).max(0.5 * EXTENT_FT.1 as f64 * m / h)
    }
}

impl Default for MsreLayout {
    fn default() -> Self {
        Self::new()
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

/// The MSRE plant elevation view. Draw it with [`Widget`] (it takes its
/// natural size at `scale` points per foot) or into any rect with
/// [`MsrePlantSchematic::paint`].
#[derive(Debug, Clone, PartialEq)]
pub struct MsrePlantSchematic {
    pub scalars: MsrePlantScalars,
    /// Points per foot when drawn as a widget.
    pub scale: f32,
    pub labels: bool,
}

impl MsrePlantSchematic {
    pub fn new(scalars: MsrePlantScalars) -> Self {
        Self { scalars, scale: 12.0, labels: true }
    }

    pub fn with_scale(mut self, points_per_foot: f32) -> Self {
        self.scale = points_per_foot;
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
    /// ([`MsreLayout::extent`]); the drawing is fitted inside it, centred,
    /// at one scale on both axes.
    pub fn paint(&self, painter: &Painter, rect: Rect) {
        let ink = crate::Ink::of_ctx(painter.ctx());
        let l = MsreLayout::new();
        let ext = MsreLayout::extent();
        let k = (rect.width() / ext.x).min(rect.height() / ext.y);
        let origin = rect.center() - ext * k * 0.5;
        let at = |q: Pos2| origin + q.to_vec2() * k;
        let box_ = |q: Rect| Rect::from_min_max(at(q.min), at(q.max));
        let line = Stroke::new((0.12 * k).max(1.0), ink.text);
        let font = |s: f32| FontId::proportional((s * k).clamp(8.0, 22.0));
        let s = &self.scalars;
        let neutral = ink.card;
        let frost = Color32::WHITE;

        // The cells: the drain tank cell and the reactor containment vessel.
        let thin = Stroke::new(line.width, ink.muted);
        painter.rect(box_(l.drain_cell), 0.4 * k, Color32::TRANSPARENT, thin, StrokeKind::Inside);
        let cell: Vec<Pos2> = l.reactor_cell_outline().into_iter().map(|q| at(q)).collect();
        painter.add(Shape::closed_line(cell, thin));
        // The thermal shield: a thick ring round the vessel.
        painter.rect(box_(l.shield), 0.6 * k, Color32::TRANSPARENT, Stroke::new(0.9 * k, ink.border), StrokeKind::Inside);

        // Pipes first, under the components.
        let pipe_w = 0.5 * k;
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
                painter.extend(Shape::dashed_line(&screen, Stroke::new(0.15 * k, frost), 0.4 * k, 0.4 * k));
            }
            if let Some(sign) = arrow_sign(flow) {
                arrows(painter, &screen, sign, 0.9 * k, ink.text);
            }
        }
        for (branches, valve) in [(&l.fuel_drain, l.drain_freeze_valve), (&l.coolant_drain, l.coolant_freeze_valve)] {
            for pts in branches {
                let screen: Vec<Pos2> = pts.iter().map(|&q| at(q)).collect();
                painter.add(Shape::line(screen, Stroke::new(0.25 * k, ink.muted)));
            }
            // Freeze valve: a flattened section, blue with a frozen plug.
            let v = Rect::from_center_size(at(valve), Vec2::splat(1.0 * k));
            let fill = if s.freeze_valve_frozen { Color32::from_rgb(120, 170, 230) } else { neutral };
            painter.rect(v, 0.15 * k, fill, line, StrokeKind::Inside);
        }

        // Reactor vessel: the downcomer (inlet salt) round the core.
        let vessel = box_(l.vessel);
        painter.rect(vessel, 0.9 * k, self.colour(s.core_inlet), line, StrokeKind::Inside);
        let core = box_(l.core);
        painter.rect(core, 0.3 * k, self.colour(s.core_fuel), Stroke::NONE, StrokeKind::Inside);
        // The graphite stringers (vertical), coloured by the graphite.
        let n = 7;
        let g = self.colour(s.graphite);
        for i in 0..n {
            let f = (i as f32 + 0.5) / n as f32;
            let x = core.left() + f * core.width();
            painter.line_segment([Pos2::new(x, core.top() + 0.3 * k), Pos2::new(x, core.bottom() - 0.3 * k)], Stroke::new(0.28 * k, g));
        }
        if s.fuel_frozen {
            hatch(painter, vessel, 0.8 * k, frost);
        }

        // Fuel pump and coolant pump: bowl, shaft and motor (the assembly
        // ~8 ft high).
        for (bowl, t, frozen) in [(l.fuel_pump, s.core_outlet, s.fuel_frozen), (l.coolant_pump, s.coolant_cold, s.coolant_frozen)] {
            let b = box_(bowl);
            painter.rect(b, 0.8 * k, self.colour(t), line, StrokeKind::Inside);
            if frozen {
                hatch(painter, b, 0.6 * k, frost);
            }
            let top = b.center_top();
            let motor_top = b.bottom() - PUMP_ASSEMBLY_H * k;
            let motor = Rect::from_min_max(Pos2::new(top.x - 1.0 * k, motor_top), Pos2::new(top.x + 1.0 * k, motor_top + 2.5 * k));
            painter.line_segment([top, motor.center_bottom()], line);
            painter.rect(motor, 0.2 * k, neutral, line, StrokeKind::Inside);
        }

        // Heat exchanger: shell (fuel) with the U-tube bundle (coolant).
        let hx = box_(l.heat_exchanger);
        let shell_f = 0.5 * (s.core_inlet.get::<degree_fahrenheit>() + s.core_outlet.get::<degree_fahrenheit>());
        painter.rect(hx, 0.5 * hx.height(), self.colour(ThermodynamicTemperature::new::<degree_fahrenheit>(shell_f)), line, StrokeKind::Inside);
        let tube = Stroke::new(0.18 * k, self.colour(s.coolant_hot));
        let (y0, y1, x0) = (hx.top() + 0.3 * hx.height(), hx.bottom() - 0.3 * hx.height(), hx.left() + 0.8 * k);
        painter.line_segment([Pos2::new(x0, y0), Pos2::new(hx.right(), y0)], tube);
        painter.line_segment([Pos2::new(x0, y1), Pos2::new(hx.right(), y1)], tube);
        painter.line_segment([Pos2::new(x0, y0), Pos2::new(x0, y1)], tube);

        // Radiator: the tube bank in its housing, salt flowing down from the
        // top header; the doors on the air face open with the air flow.
        let rad = box_(l.radiator);
        painter.rect(rad, 0.3 * k, neutral, line, StrokeKind::Inside);
        let bank = rad.shrink(0.9 * k);
        let rows = 7;
        for i in 0..rows {
            let f = (i as f32 + 0.5) / rows as f32;
            let t_row = ThermodynamicTemperature::new::<degree_fahrenheit>(
                s.coolant_hot.get::<degree_fahrenheit>() + f as f64 * (s.coolant_cold.get::<degree_fahrenheit>() - s.coolant_hot.get::<degree_fahrenheit>()),
            );
            let y = bank.top() + f * bank.height();
            painter.line_segment([Pos2::new(bank.left(), y), Pos2::new(bank.right(), y)], Stroke::new(0.35 * k, self.colour(t_row)));
        }
        if s.coolant_frozen {
            hatch(painter, bank, 0.8 * k, frost);
        }
        let open = door_opening(s.air_flow);
        // The doors as shutters across the air face (right), closing from
        // both ends: the uncovered middle is the open fraction.
        let shut = Color32::from_rgba_unmultiplied(ink.warn.r(), ink.warn.g(), ink.warn.b(), 220);
        let face = Rect::from_min_max(Pos2::new(rad.right(), rad.top()), Pos2::new(rad.right() + 0.6 * k, rad.bottom()));
        let c = 0.5 * (1.0 - open) * face.height();
        painter.rect_filled(Rect::from_min_max(face.min, Pos2::new(face.max.x, face.min.y + c)), 0.0, shut);
        painter.rect_filled(Rect::from_min_max(Pos2::new(face.min.x, face.max.y - c), face.max), 0.0, shut);
        painter.rect_stroke(face, 0.0, Stroke::new(1.0, ink.warn), StrokeKind::Inside);

        // Blowers, coloured by the air leaving the radiator.
        for f in l.fans {
            let fr = box_(f);
            painter.circle(fr.center(), 0.45 * fr.width().min(fr.height()), neutral, line);
            painter.circle_filled(fr.center(), 0.18 * fr.width().min(fr.height()), self.colour(s.radiator_air_out));
        }

        // Drain tanks, flush tank and coolant drain tank: empty in operation
        // (the model holds the salt in the loops).
        for t in [l.drain_tanks[0], l.drain_tanks[1], l.flush_tank, l.coolant_drain_tank] {
            painter.rect(box_(t), 1.2 * k, neutral, line, StrokeKind::Inside);
        }

        if self.labels {
            let txt = |q: Pos2, a: Align2, s: &str, size: f32| {
                painter.text(at(q), a, s, font(size), ink.text);
            };
            let f_ = |t: ThermodynamicTemperature| format!("{:.0} °F", t.get::<degree_fahrenheit>());
            let small = 0.9;
            // Below ~10 points per foot (a phone) the smallest legible font
            // is wider than the gaps: keep only the component names, which
            // the readouts above the drawing complement.
            let compact = k < COMPACT_BELOW_POINTS_PER_FT;
            txt(l.containment.left_top() + Vec2::new(0.6, 0.5), Align2::LEFT_TOP, "reactor cell", 1.0);
            txt(l.drain_cell.left_top() + Vec2::new(0.6, 0.5), Align2::LEFT_TOP, "drain tank cell", 1.0);
            txt(l.fuel_pump.left_center() + Vec2::new(-1.2, 0.0), Align2::RIGHT_CENTER, "fuel pump", small);
            txt(l.heat_exchanger.center_bottom() + Vec2::new(0.0, 0.3), Align2::CENTER_TOP, "heat exchanger", small);
            txt(l.coolant_pump.right_bottom() + Vec2::new(0.2, 0.4), Align2::LEFT_TOP, "coolant pump", small);
            txt(l.radiator.left_bottom() + Vec2::new(1.7, 0.3), Align2::LEFT_TOP, "radiator", small);
            txt(l.coolant_drain_tank.center_bottom() + Vec2::new(0.0, 0.3), Align2::CENTER_TOP, "coolant drain tank", 0.8);
            for (t, name) in [(l.drain_tanks[0], "FD1"), (l.drain_tanks[1], "FD2"), (l.flush_tank, "FFT")] {
                txt(t.center(), Align2::CENTER_CENTER, name, small);
            }
            if !compact {
                txt(l.core.center(), Align2::CENTER_CENTER, &format!("{:.2} MW", s.power.get::<megawatt>()), 1.1);
                txt(l.fans[0].center_top() + Vec2::new(0.0, -0.2), Align2::CENTER_BOTTOM, &format!("air {}", f_(s.radiator_air_out)), 0.8);
                txt(l.fans[1].center_bottom() + Vec2::new(0.0, 0.3), Align2::CENTER_TOP, &format!("doors {:.0} %", 100.0 * open), 0.8);
                let fv = if s.freeze_valve_frozen { "freeze valve (frozen)" } else { "freeze valve (thawed)" };
                txt(l.drain_freeze_valve + Vec2::new(0.0, -0.9), Align2::RIGHT_BOTTOM, fv, 0.75);
                // Temperatures on the runs.
                txt(l.fuel_hot[2] + Vec2::new(-0.3, -0.5), Align2::RIGHT_BOTTOM, &f_(s.core_outlet), small);
                txt(l.fuel_cold[0] + Vec2::new(-0.3, 0.4), Align2::RIGHT_TOP, &f_(s.core_inlet), small);
                txt(l.coolant_hot[3] + Vec2::new(-1.0, -0.5), Align2::RIGHT_BOTTOM, &f_(s.coolant_hot), small);
                txt(l.coolant_cold[4] + Vec2::new(1.0, 0.5), Align2::LEFT_TOP, &f_(s.coolant_cold), small);
                txt(Pos2::new(EXTENT_FT.0 - 0.3, EXTENT_FT.1 - 0.3), Align2::RIGHT_BOTTOM, "elevation view, roughly to scale (TM-0728 sizes)", 0.7);
            }
        }
    }
}

/// Below this many points per foot the widget shows only component names
/// ([`MsrePlantSchematic::paint`]).
pub const COMPACT_BELOW_POINTS_PER_FT: f32 = 10.0;

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
        let size = MsreLayout::extent() * self.scale;
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
            l.drain_cell,
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

    /// Every component and every pipe point lies inside the drawing.
    #[test]
    fn everything_is_inside_the_drawing() {
        let l = MsreLayout::new();
        let e = Rect::from_min_size(Pos2::ZERO, MsreLayout::extent());
        for q in rects(&l) {
            assert!(e.contains_rect(q), "{q:?} outside {e:?}");
        }
        let drains: Vec<&Vec<Pos2>> = l.fuel_drain.iter().chain(l.coolant_drain.iter()).collect();
        for run in [&l.fuel_hot, &l.fuel_cold, &l.coolant_hot, &l.coolant_cold].into_iter().chain(drains) {
            for q in run {
                assert!(e.contains(*q), "{q:?} outside {e:?}");
            }
        }
        for q in l.reactor_cell_outline() {
            assert!(e.expand(1e-3).contains(q), "{q:?}");
        }
    }

    /// The sourced sizes come through: vessel 59⅛ in OD, core 55¼ × 64 in,
    /// cell 24 × 33 ft, drain tanks 50 × 86 in, heat exchanger 16 in × 8 ft,
    /// shield 125 in OD, and the radiator bottom at el. 836 ft 1¾ in.
    #[test]
    fn the_sourced_dimensions_hold() {
        let l = MsreLayout::new();
        let close = |a: f32, b: f32| (a - b).abs() < 1e-3;
        assert!(close(l.vessel.width(), 59.125 / 12.0));
        assert!(close(l.core.width(), 55.25 / 12.0) && close(l.core.height(), 64.0 / 12.0));
        assert!(close(l.containment.width(), 24.0) && close(l.containment.height(), 33.0));
        for t in [l.drain_tanks[0], l.drain_tanks[1]] {
            assert!(close(t.width(), 50.0 / 12.0) && close(t.height(), 86.0 / 12.0));
        }
        assert!(close(l.heat_exchanger.width(), 8.0) && close(l.heat_exchanger.height(), 16.0 / 12.0));
        assert!(close(TOP_ELEVATION_FT - l.radiator.bottom(), 836.0 + 1.75 / 12.0));
        assert!(close(l.shield.width(), 125.0 / 12.0));
    }

    /// The nesting: core in vessel in shield in the reactor cell, with the
    /// fuel pump and heat exchanger inside the cell; the drain tanks in the
    /// drain cell; the coolant loop, radiator, blowers and every tank
    /// outside the reactor cell. The reactor cell's outline stays in its
    /// bounding box and reaches its bottom, and the hemispherical bottom
    /// clears the shield.
    #[test]
    fn the_nesting_holds() {
        let l = MsreLayout::new();
        assert!(l.vessel.contains_rect(l.core));
        assert!(l.shield.contains_rect(l.vessel));
        assert!(l.containment.contains_rect(l.shield));
        assert!(l.containment.contains_rect(l.heat_exchanger));
        assert!(l.containment.contains_rect(l.fuel_pump));
        for t in [l.drain_tanks[0], l.drain_tanks[1], l.flush_tank] {
            assert!(l.drain_cell.contains_rect(t));
        }
        for q in [l.coolant_pump, l.radiator, l.fans[0], l.fans[1], l.drain_tanks[0], l.drain_tanks[1], l.flush_tank, l.coolant_drain_tank, l.drain_cell] {
            assert!(!l.containment.intersects(q), "{q:?}");
        }
        let o = l.reactor_cell_outline();
        assert!(o.iter().all(|q| l.containment.expand(1e-3).contains(*q)));
        let lowest = o.iter().map(|q| q.y).fold(f32::MIN, f32::max);
        assert!((lowest - l.containment.bottom()).abs() < 0.2, "{lowest}");
        let r = 0.5 * l.containment.width();
        let c = Pos2::new(l.containment.center().x, l.containment.bottom() - r);
        for corner in [l.shield.left_bottom(), l.shield.right_bottom()] {
            assert!(corner.distance(c) < r, "{corner:?}");
        }
    }

    /// Gravity: the drain line runs level or downhill from the vessel to the
    /// tanks, and the tanks sit wholly below the vessel; the coolant drain
    /// tank sits below the radiator; the heat exchanger sits below the
    /// coolant pump bowl, the coolant loop's highest point (TM-0728 §8).
    #[test]
    fn salt_drains_downhill() {
        let l = MsreLayout::new();
        // y grows downwards: downhill or level means y never decreases.
        let never_up = |b: &Vec<Pos2>| b.windows(2).all(|w| w[1].y >= w[0].y - 1e-4);
        for b in &l.fuel_drain {
            assert!(never_up(b), "{b:?}");
        }
        for t in [l.drain_tanks[0], l.drain_tanks[1], l.flush_tank] {
            assert!(t.top() > l.vessel.bottom(), "{t:?} not below the vessel");
        }
        assert!(never_up(&l.coolant_drain[0]));
        assert!(l.coolant_drain_tank.top() > l.radiator.bottom());
        assert!(l.heat_exchanger.top() > l.coolant_pump.top());
    }

    /// The runs join the components they connect, in TM-0728 §8.2's order
    /// for the coolant: heat exchanger -> radiator top -> radiator bottom
    /// -> coolant pump -> heat exchanger.
    #[test]
    fn the_runs_join_their_components() {
        let l = MsreLayout::new();
        let touches = |q: Pos2, r: Rect| r.expand(0.01).contains(q);
        assert!(touches(l.fuel_hot[0], l.vessel));
        assert!(l.fuel_hot.iter().any(|&q| touches(q, l.fuel_pump)));
        assert!(touches(*l.fuel_hot.last().unwrap(), l.heat_exchanger));
        assert!(touches(l.fuel_cold[0], l.heat_exchanger));
        assert!(touches(*l.fuel_cold.last().unwrap(), l.vessel));
        assert!(touches(l.coolant_hot[0], l.heat_exchanger));
        let rad_in = *l.coolant_hot.last().unwrap();
        assert!(touches(rad_in, l.radiator) && (rad_in.y - l.radiator.top()).abs() < 1e-3, "line 201 enters the top header");
        assert!(!l.coolant_hot.iter().any(|&q| touches(q, l.coolant_pump)), "the pump is on the cold side");
        assert!(touches(l.coolant_cold[0], l.radiator) && (l.coolant_cold[0].y - l.radiator.bottom()).abs() < 1e-3);
        assert!(l.coolant_cold.iter().any(|&q| touches(q, l.coolant_pump)));
        assert!(touches(*l.coolant_cold.last().unwrap(), l.heat_exchanger));
        assert!(touches(l.fuel_drain[0][0], l.vessel));
        let ends: Vec<Pos2> = l.fuel_drain.iter().map(|b| *b.last().unwrap()).collect();
        for t in [l.drain_tanks[0], l.drain_tanks[1], l.flush_tank] {
            assert!(ends.iter().any(|&q| touches(q, t)), "{t:?}");
        }
        assert!(touches(*l.coolant_drain[0].last().unwrap(), l.coolant_drain_tank));
    }

    /// Arrows follow the flow's sign and vanish at zero; the colour scale
    /// and the door opening clamp; the fit half-extent fits both axes,
    /// tightly on one.
    #[test]
    fn arrows_colours_doors_and_fit_follow_the_state() {
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
        for size in [Vec2::new(1200.0, 700.0), Vec2::new(390.0, 760.0), Vec2::new(500.0, 500.0)] {
            let he = MsreLayout::half_extent_to_fit(size);
            let ppf = size.x.min(size.y) as f64 / (2.0 * he);
            let (w, h) = (EXTENT_FT.0 as f64 * ppf, EXTENT_FT.1 as f64 * ppf);
            assert!(w <= size.x as f64 + 1e-3 && h <= size.y as f64 + 1e-3, "{size:?}");
            assert!((w - size.x as f64).abs() < 1e-3 || (h - size.y as f64).abs() < 1e-3, "{size:?} not tight");
        }
    }

    /// The widget paints without panicking, frozen or not, labelled or not,
    /// and allocates its extent times its scale.
    #[test]
    fn it_paints() {
        let f = ThermodynamicTemperature::new::<degree_fahrenheit>;
        let mut s = MsrePlantScalars {
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
        for frozen in [true, false] {
            s.fuel_frozen = frozen;
            s.coolant_frozen = frozen;
            s.freeze_valve_frozen = !frozen;
            let mut size = Vec2::ZERO;
            let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
                size = ui.add(MsrePlantSchematic::new(s).with_scale(10.0)).rect.size();
                MsrePlantSchematic::new(s).without_labels().paint(ui.painter(), Rect::from_min_size(Pos2::ZERO, Vec2::new(300.0, 300.0)));
            });
            assert_eq!(size, MsreLayout::extent() * 10.0);
        }
    }
}
