//! Routed pipework: a pipe drawn along a path of straight legs with a proper
//! elbow at every corner, carrying one stream's temperature and tracers.
//!
//! Moved here from `examples/htgr_sim_v1/app/schematic.rs` (2026-09-22) so the
//! widget studio's HTR-10 page and `htgr_sim_v1` draw pipework the same way,
//! from one implementation. The code is `htgr_sim_v1`'s, unchanged in
//! behaviour; only the stream's fields became `uom` quantities and it carries
//! its own colour range instead of reading that example's constants.
//!
//! Presentation only: the stream's temperature, mass flow and residence time
//! come from the caller's model. The tracer direction follows the sign of the
//! mass flow and its speed the residence time, as [`PipeVisual`] documents.

use crate::animation::TracerTrain;
use crate::components::{PipeBendVisual, PipeScalars, PipeScale, PipeVisual};
use egui::{Pos2, Ui, Vec2};
use uom::si::f64::{MassRate, ThermodynamicTemperature, Time};

/// One fluid stream's state: everything a routed pipe needs to draw itself.
///
/// The caller supplies real state from its own model, which is the contract
/// [`PipeVisual::from_scalars`] documents. None of these is invented here.
#[derive(Debug, Clone, Copy)]
pub struct PipeStream {
    /// Bulk fluid temperature, which colours the pipe.
    pub temperature: ThermodynamicTemperature,
    /// Mass flow, whose sign sets the tracer direction.
    pub mass_flow: MassRate,
    /// Loop residence time, which sets the tracer speed.
    pub residence_time: Time,
    /// Drawn pipe thickness, points.
    pub thickness: f32,
    /// The application-owned tracer train this stream's legs carry.
    pub tracer: TracerTrain,
    /// Cold end of the colour scale.
    pub min_temp: ThermodynamicTemperature,
    /// Hot end of the colour scale.
    pub max_temp: ThermodynamicTemperature,
}

impl PipeStream {
    /// One straight leg of this stream, from `from` to `to`.
    pub fn run(&self, from: Pos2, to: Pos2) -> PipeVisual {
        PipeVisual::from_scalars(
            PipeScalars {
                temperature: self.temperature,
                mass_flow: self.mass_flow,
                residence_time: self.residence_time,
            },
            from,
            to - from,
            self.min_temp,
            self.max_temp,
        )
        .with_scale(PipeScale {
            min_thickness_points: self.thickness,
            ..PipeScale::default()
        })
        .with_tracer(self.tracer)
    }
}

/// Draw one elbow at `corner`, turning from `d_in` to `d_out`.
///
/// The two runs' inner corners are made coincident at a single point, which is
/// the construction [`PipeBendVisual`] documents: each run's centreline stops
/// half a thickness short of the geometric corner, and the inner corner sits
/// half a thickness inboard of that on the inside of the turn. Getting this
/// wrong is what makes elbows read as two rectangles butted together.
fn elbow(ui: &mut Ui, stream: &PipeStream, corner: Pos2, d_in: Vec2, d_out: Vec2) {
    let half = 0.5 * stream.thickness;
    // Outward normal of the incoming run, on the OUTSIDE of the turn. Screen y
    // grows downward, so a positive cross product is a clockwise turn.
    let cross = d_in.x * d_out.y - d_in.y * d_out.x;
    let sign = if cross >= 0.0 { -1.0 } else { 1.0 };
    let normal_in = Vec2::new(-d_in.y, d_in.x) * sign;

    let inlet_end = corner - d_in * half;
    let inner_corner = inlet_end - normal_in * half;

    ui.add(PipeBendVisual::new(
        inner_corner,
        d_in,
        d_out,
        stream.thickness,
        stream.temperature,
        stream.temperature,
        stream.min_temp,
        stream.max_temp,
    ));
}

/// Draw a whole routed pipe path: one [`PipeVisual`] per straight leg and one
/// [`PipeBendVisual`] per interior corner.
///
/// `path` is the run's **centreline**, corner to corner, in screen points; the
/// caller chooses the corners (this does not find a path). Every leg is
/// trimmed back by half a pipe thickness at each interior corner so the elbow
/// sector meets it flush. `trim_start`/`trim_end` do the same at the two ends,
/// for a path that is continued by another path through a shared elbow.
pub fn route(ui: &mut Ui, stream: &PipeStream, path: &[Pos2], trim_start: bool, trim_end: bool) {
    if path.len() < 2 {
        return;
    }
    let half = 0.5 * stream.thickness;
    let directions: Vec<Vec2> = path
        .windows(2)
        .map(|leg| (leg[1] - leg[0]).normalized())
        .collect();
    let last = directions.len() - 1;

    for (i, leg) in path.windows(2).enumerate() {
        let direction = directions[i];
        let mut from = leg[0];
        let mut to = leg[1];
        if i > 0 || trim_start {
            from += direction * half;
        }
        if i < last || trim_end {
            to -= direction * half;
        }
        ui.add(stream.run(from, to));
    }

    for i in 1..directions.len() {
        elbow(ui, stream, path[i], directions[i - 1], directions[i]);
    }
}
