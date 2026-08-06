//! Smooth bend between two pipe runs.
//!
//! # The geometry
//!
//! Two rectangular runs meeting at an angle leave a wedge-shaped gap on the
//! outside of the turn and overlap on the inside. Drawing them butted together
//! is what makes elbows look wrong. The fix is to make the joint an explicit
//! piece of geometry:
//!
//! - the two runs' **inner** corners are coincident, at a single point `P`;
//! - their **outer** corners sit one pipe-thickness from `P`, along each run's
//!   outward normal;
//! - the gap between those outer corners is closed by a **circular arc**
//!   centred on `P`, radius equal to the pipe thickness.
//!
//! The filled region is therefore a circular sector — a quarter circle for a
//! 90-degree bend, narrower or wider as the turn angle changes. Both outer
//! corners are exactly one thickness from `P` by construction, so the arc
//! meets each run's outer edge tangentially and the silhouette is continuous.
//!
//! ```text
//!        run A
//!    ┌──────────────┐
//!    │              │ P  <- inner corners coincide here
//!    └──────────┐   ●
//!               │ ╲ │      the sector is centred on P,
//!    outer arc  │  ╲│      radius = pipe thickness
//!               ╰───┤
//!                   │ run B
//! ```
//!
//! # Why it is coloured the way it is
//!
//! The bend is a control volume shared between two runs, so it is filled with
//! the **mean** of the two adjacent cell temperatures rather than either one.
//! Taking one side's value would draw a discontinuity at a joint where the
//! physics has none, and would flip which side looked hotter depending on
//! which run happened to be drawn first.

use crate::components::{temperature_colour, PipePhaseShade};
use egui::{Color32, Pos2, Response, Sense, Stroke, Ui, Vec2, Widget};
use uom::si::angle::radian;
use uom::si::f64::{Angle, ThermodynamicTemperature};
use uom::si::thermodynamic_temperature::kelvin;

/// Width of the bend's outer wall, matching [`super::pipe`]'s.
const WALL_WIDTH: f32 = 5.0;

/// Which way the joint turns, seen on screen.
///
/// Needed because the turn sense cannot always be inferred. At exactly 180
/// degrees the two directions are antiparallel and the cross product vanishes,
/// so a U-bend is genuinely ambiguous — it may belly to either side, and both
/// are correct pipework. Inferring would make the sector flip sides the instant
/// the angle reached 180.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TurnSense {
    /// Infer from the cross product of the two directions. Correct for every
    /// angle strictly between 0 and 180.
    #[default]
    Auto,
    /// Turn clockwise on screen (screen y grows downward, so this is
    /// "downward" for a run heading right).
    Clockwise,
    /// Turn anticlockwise on screen.
    Anticlockwise,
}

/// A smooth bend joining two pipe runs.
///
/// Construct it from the joint's inner corner and the two runs' directions;
/// see the module docs for the construction. The directions are the flow
/// directions of each run — `in_direction` points *towards* the joint,
/// `out_direction` points *away* from it.
pub struct PipeBendVisual {
    /// The coincident inner corner of the two runs.
    pub inner_corner: Pos2,
    /// Flow direction of the incoming run, pointing towards the joint.
    pub in_direction: Vec2,
    /// Flow direction of the outgoing run, pointing away from the joint.
    pub out_direction: Vec2,
    /// Pipe thickness in points, which is also the sector's radius.
    pub thickness: f32,
    /// Temperature of the last cell of the incoming run.
    pub upstream_temperature: ThermodynamicTemperature,
    /// Temperature of the first cell of the outgoing run.
    pub downstream_temperature: ThermodynamicTemperature,
    /// Temperature drawn in the coldest displayable colour.
    pub min_temp: ThermodynamicTemperature,
    /// Temperature drawn in the hottest displayable colour.
    pub max_temp: ThermodynamicTemperature,
    /// Phase shading, matching the runs it joins.
    pub shade: PipePhaseShade,
    /// Which way the joint turns. See [`TurnSense`] — set this explicitly for
    /// a 180-degree return bend, where it cannot be inferred.
    pub turn_sense: TurnSense,
    /// Explicit signed sweep in radians, positive clockwise on screen.
    ///
    /// `None` infers the sweep from the two directions, which is correct for
    /// any turn up to half a circle. Beyond that the inference breaks down:
    /// the angle between two vectors is only ever `[0, pi]`, so a 270-degree
    /// turn is indistinguishable from a 90-degree one and would draw as the
    /// wrong sector. State the sweep whenever the joint may exceed 180.
    pub sweep_override: Option<f32>,
}

impl PipeBendVisual {
    /// A bend at `inner_corner`, turning from `in_direction` to
    /// `out_direction`.
    pub fn new(
        inner_corner: Pos2,
        in_direction: Vec2,
        out_direction: Vec2,
        thickness: f32,
        upstream_temperature: ThermodynamicTemperature,
        downstream_temperature: ThermodynamicTemperature,
        min_temp: ThermodynamicTemperature,
        max_temp: ThermodynamicTemperature,
    ) -> Self {
        Self {
            inner_corner,
            in_direction,
            out_direction,
            thickness,
            upstream_temperature,
            downstream_temperature,
            min_temp,
            max_temp,
            shade: PipePhaseShade::Liquid,
            turn_sense: TurnSense::default(),
            sweep_override: None,
        }
    }

    /// State the swept angle explicitly, positive clockwise on screen.
    ///
    /// Required past 180 degrees — see [`Self::sweep_override`]. Also removes
    /// the 180-degree ambiguity, since a signed sweep says which way round.
    pub fn with_sweep(mut self, sweep: Angle) -> Self {
        self.sweep_override = Some(sweep.get::<radian>() as f32);
        self
    }

    /// State the turn sense explicitly. Builder-style.
    ///
    /// Required at 180 degrees, where [`TurnSense::Auto`] has nothing to work
    /// from; harmless at every other angle, where it simply agrees with what
    /// would have been inferred.
    pub fn with_turn_sense(mut self, turn_sense: TurnSense) -> Self {
        self.turn_sense = turn_sense;
        self
    }

    /// Set the phase shading so the bend matches the runs it joins.
    /// Builder-style.
    pub fn with_shade(mut self, shade: PipePhaseShade) -> Self {
        self.shade = shade;
        self
    }

    /// Mean of the two adjacent cell temperatures — what the bend is filled
    /// with.
    ///
    /// The joint is shared between both runs, so neither side's value alone is
    /// right: using one would draw a temperature step at a joint where the
    /// physics has none.
    pub fn mean_temperature(&self) -> ThermodynamicTemperature {
        ThermodynamicTemperature::new::<kelvin>(
            0.5 * (self.upstream_temperature.get::<kelvin>()
                + self.downstream_temperature.get::<kelvin>()),
        )
    }

    /// Outward normal of a run, on the outside of the turn.
    ///
    /// The turn's sense is taken from the cross product of the two directions,
    /// so the sector is placed on the correct side whether the run bends left
    /// or right. Getting this wrong puts the fill on the *inside* of the elbow,
    /// which reads as a bite taken out of the pipe.
    fn outward_normals(&self) -> (Vec2, Vec2) {
        let a = self.in_direction.normalized();
        let b = self.out_direction.normalized();
        let sign = -self.turn_direction();
        (Vec2::new(-a.y, a.x) * sign, Vec2::new(-b.y, b.x) * sign)
    }

    /// `+1` for a clockwise turn on screen, `-1` for anticlockwise.
    ///
    /// Taken from [`Self::turn_sense`] when stated, otherwise from the cross
    /// product of the two directions.
    fn turn_direction(&self) -> f32 {
        match self.turn_sense {
            TurnSense::Clockwise => 1.0,
            TurnSense::Anticlockwise => -1.0,
            TurnSense::Auto => {
                // An explicit sweep already carries the sense.
                if let Some(s) = self.sweep_override {
                    return if s >= 0.0 { 1.0 } else { -1.0 };
                }
                let a = self.in_direction.normalized();
                let b = self.out_direction.normalized();
                // Screen y grows downward, so a positive cross product is a
                // clockwise turn on screen.
                let cross = a.x * b.y - a.y * b.x;
                if cross >= 0.0 {
                    1.0
                } else {
                    -1.0
                }
            }
        }
    }

    /// The signed angle actually swept, in radians, positive clockwise.
    ///
    /// The stated sweep when one was given, otherwise the inferred turn angle
    /// carrying the inferred sense.
    pub fn signed_sweep(&self) -> f32 {
        match self.sweep_override {
            Some(s) => s,
            None => self.turn_direction() * self.turn_angle(),
        }
    }

    /// The angle between the two runs, in radians, always in `[0, pi]`.
    ///
    /// Only meaningful for joints of half a circle or less; see
    /// [`Self::sweep_override`].
    pub fn turn_angle(&self) -> f32 {
        let a = self.in_direction.normalized();
        let b = self.out_direction.normalized();
        (a.x * b.x + a.y * b.y).clamp(-1.0, 1.0).acos()
    }

    /// The filled sector, as a polygon: the inner corner followed by the arc
    /// between the two outer corners.
    fn sector(&self) -> Vec<Pos2> {
        let (na, _nb) = self.outward_normals();
        let r = self.thickness.max(1.0);

        let start = na.y.atan2(na.x);
        // Sweep magnitude is the turn angle itself: both normals are the run
        // directions rotated by the same right angle, so the angle between the
        // normals equals the angle between the runs. Its DIRECTION comes from
        // the turn sense rather than from differencing the two normal angles —
        // that difference is ambiguous at exactly 180 degrees, and would make
        // a return bend snap to the wrong side.
        let sweep = self.signed_sweep();

        // Segment count follows the sweep, so a near-full circle is as smooth
        // as a quarter one rather than becoming a visible polygon.
        let segments = ((sweep.abs() / std::f32::consts::TAU) * 96.0).ceil().max(4.0) as usize;

        let mut pts = Vec::with_capacity(segments + 2);
        pts.push(self.inner_corner);
        for i in 0..=segments {
            let a = start + sweep * (i as f32 / segments as f32);
            pts.push(Pos2::new(
                self.inner_corner.x + r * a.cos(),
                self.inner_corner.y + r * a.sin(),
            ));
        }
        pts
    }
}

impl Widget for PipeBendVisual {
    /// Fills the sector with the mean adjacent temperature, then strokes the
    /// outer arc as pipe wall.
    ///
    /// The two straight edges are deliberately *not* stroked: they are shared
    /// with the runs on either side, and drawing them would put a wall line
    /// across the middle of the flow path.
    fn ui(self, ui: &mut Ui) -> Response {
        let pts = self.sector();
        let r = self.thickness.max(1.0);
        let rect = egui::Rect::from_center_size(self.inner_corner, Vec2::splat(2.0 * r + WALL_WIDTH));
        let response = ui.allocate_rect(rect, Sense::hover());
        let painter = ui.painter();

        let fill = self
            .shade
            .apply(temperature_colour(self.mean_temperature(), self.min_temp, self.max_temp));
        painter.add(egui::Shape::convex_polygon(
            pts.clone(),
            fill,
            Stroke::NONE,
        ));

        // Outer wall: the arc only, skipping the first point (the inner corner)
        // and therefore the two straight edges.
        painter.add(egui::Shape::line(
            pts[1..].to_vec(),
            Stroke::new(WALL_WIDTH, Color32::from_gray(110)),
        ));

        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn k(v: f64) -> ThermodynamicTemperature {
        ThermodynamicTemperature::new::<kelvin>(v)
    }

    fn bend(inb: Vec2, out: Vec2) -> PipeBendVisual {
        PipeBendVisual::new(
            Pos2::new(100.0, 100.0),
            inb,
            out,
            40.0,
            k(400.0),
            k(600.0),
            k(300.0),
            k(900.0),
        )
    }

    /// The bend is a shared control volume, so it must be filled with the MEAN
    /// of the two adjacent cells. Using either side alone would draw a
    /// temperature step at a joint where the physics has none.
    #[test]
    fn fill_uses_the_mean_of_both_sides() {
        let b = bend(Vec2::new(1.0, 0.0), Vec2::new(0.0, -1.0));
        assert_eq!(b.mean_temperature().get::<kelvin>(), 500.0);
    }

    /// Every point on the arc must be exactly one pipe-thickness from the
    /// inner corner. That is what makes the arc meet both runs' outer edges
    /// without a step in the silhouette.
    ///
    /// **Methodology:** build a 90-degree bend of thickness 40 points and check
    /// the radius of every arc vertex.
    ///
    /// **Result (2026-08-06):** all 25 arc vertices lie at 40.0 points from the
    /// inner corner, to within 1e-3.
    #[test]
    fn arc_points_are_one_thickness_from_the_inner_corner() {
        let b = bend(Vec2::new(1.0, 0.0), Vec2::new(0.0, -1.0));
        let centre = b.inner_corner;
        let pts = b.sector();
        assert_eq!(pts[0], centre, "first point must be the inner corner");
        for p in &pts[1..] {
            let r = ((p.x - centre.x).powi(2) + (p.y - centre.y).powi(2)).sqrt();
            assert!((r - 40.0).abs() < 1e-3, "arc radius {r}, expected 40.0");
        }
    }

    /// A 90-degree turn must sweep exactly a quarter circle — the "pizza
    /// slice" shape. Measured as the angle between the first and last arc
    /// points about the inner corner.
    #[test]
    fn ninety_degree_bend_sweeps_a_quarter_circle() {
        let b = bend(Vec2::new(1.0, 0.0), Vec2::new(0.0, -1.0));
        let pts = b.sector();
        let c = b.inner_corner;
        let ang = |p: &Pos2| (p.y - c.y).atan2(p.x - c.x);
        let mut sweep = (ang(pts.last().unwrap()) - ang(&pts[1])).abs();
        if sweep > std::f32::consts::PI {
            sweep = std::f32::consts::TAU - sweep;
        }
        assert!(
            (sweep - std::f32::consts::FRAC_PI_2).abs() < 1e-3,
            "expected a quarter circle, swept {sweep} rad"
        );
    }

    /// A bend turning the other way must put its sector on the other side.
    /// Getting this wrong fills the INSIDE of the elbow, which reads as a bite
    /// taken out of the pipe rather than a smooth joint.
    #[test]
    fn sector_follows_the_turn_direction() {
        let left = bend(Vec2::new(1.0, 0.0), Vec2::new(0.0, -1.0));
        let right = bend(Vec2::new(1.0, 0.0), Vec2::new(0.0, 1.0));
        let mid = |b: &PipeBendVisual| -> Pos2 {
            let p = b.sector();
            p[p.len() / 2]
        };
        // Screen y grows downward: a leftward (upward) turn puts its outer
        // sector BELOW the corner, a rightward turn puts it above.
        assert!(mid(&left).y > left.inner_corner.y, "left turn: sector below");
        assert!(mid(&right).y < right.inner_corner.y, "right turn: sector above");
    }

    /// Past half a circle the sweep MUST be stated: the angle between two
    /// vectors is only ever `[0, pi]`, so an inferred 270-degree turn is
    /// indistinguishable from a 90-degree one and draws the wrong sector.
    ///
    /// **Methodology:** build a 270-degree joint two ways — inferring from the
    /// directions, and stating the sweep — and compare the angle actually
    /// swept by the returned polygon.
    ///
    /// **Result (2026-08-06):** inference sweeps 90 degrees (wrong, and the
    /// reason `with_sweep` exists); the stated sweep gives 270.
    #[test]
    fn sweep_past_half_a_circle_must_be_stated() {
        use uom::si::angle::degree;

        let dirs = |deg: f32| {
            let a = deg.to_radians();
            (Vec2::new(1.0, 0.0), Vec2::new(a.cos(), -a.sin()))
        };
        let (d_in, d_out) = dirs(270.0);

        let inferred = bend(d_in, d_out);
        assert!(
            (inferred.turn_angle().to_degrees() - 90.0).abs() < 1e-3,
            "inference cannot exceed 180 degrees; that is why with_sweep exists"
        );

        let stated = bend(d_in, d_out).with_sweep(Angle::new::<degree>(-270.0));
        assert!(
            (stated.signed_sweep().to_degrees() + 270.0).abs() < 1e-3,
            "stated sweep must be used verbatim, got {}",
            stated.signed_sweep().to_degrees()
        );
    }

    /// A full turn must close: the arc's first and last points coincide.
    #[test]
    fn full_circle_closes() {
        use uom::si::angle::degree;
        let b = bend(Vec2::new(1.0, 0.0), Vec2::new(1.0, 0.0))
            .with_sweep(Angle::new::<degree>(-360.0));
        let pts = b.sector();
        let first = pts[1];
        let last = *pts.last().unwrap();
        assert!(
            (first.x - last.x).abs() < 1e-2 && (first.y - last.y).abs() < 1e-2,
            "a full turn must close, got {first:?} and {last:?}"
        );
    }

    /// A straight-through joint has nothing to fill; the sweep collapses and
    /// the widget must not panic or produce a wild polygon.
    #[test]
    fn straight_joint_degenerates_safely() {
        let b = bend(Vec2::new(1.0, 0.0), Vec2::new(1.0, 0.0));
        let pts = b.sector();
        assert!(pts.len() >= 6, "degenerate sweep still needs a usable polygon");
        for p in &pts[1..] {
            assert!(p.x.is_finite() && p.y.is_finite());
        }
    }
}
