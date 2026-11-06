//! Visual pipe: the art.
//!
//! **Split 2026-11-05** (maintainer: "widget art can live in bugis, the
//! combination of logic and art can live inside the digital twin engine"):
//! this file draws a pipe run from plain data, [`PipeArt`]. The wrapper
//! around a `tampines::components::Pipe`, `PipeArt`, lives in
//! `outram_park_digital_twin_engine::components::pipe`; it reads the pipe's
//! cells, wall and geometry and draws through [`PipeArt`]. Below, "physics"
//! means [`PipeArtState::Cells`], the per-cell data that wrapper hands over.
//!
//! Renders a pipe run as a coloured line whose colour comes from the fluid
//! temperature and whose optional tracer marks come from the mass flow --
//! the crate's core "rendering derives directly from physics state" idea,
//! applied to the component that carries most of a plant schematic's flow.
//!
//! ## Two ways to supply the state
//!
//! [`PipeArtState`] is an enum (not a trait object, per the workspace's
//! mandatory design rules) with two variants, because digital-twin
//! applications reach this widget from two directions:
//!
//! - [`PipeArtState::Cells`] (~~`Physics` wrapping a full
//!   `tampines::components::Pipe`~~, CORRECTED 2026-11-05: the engine's
//!   `PipeArt` reads the pipe and hands its cells over). The pipe's
//!   per-cell temperature profile is read off its flow backend, so the run is drawn as one
//!   coloured segment **per finite-volume cell** -- cell count drives
//!   displayed cells, cell temperature drives cell colour.
//! - [`PipeArtState::Scalars`] takes a [`PipeScalars`] triple
//!   (temperature, mass flow, residence time) directly. A
//!   `tampines::components::Pipe` can only be built around a
//!   `SinglePhaseFluidArray` or `CompressibleFluidArray`, which is a heavy
//!   object to stand up for what may be a short connector line between two
//!   pieces of equipment. Simulators whose loop physics is their own lumped
//!   model (rather than a TAMPINES fluid array) supply that model's scalars
//!   here and still get correct colour and tracer motion.
//!
//! The scalar variant is *not* a placeholder for missing physics: the caller
//! is expected to pass real state from its own model. It is a narrower
//! interface, not a fabricated one.

use petir::mathf::RealMath;

use crate::animation::TracerTrain;
use crate::components::temperature_colour;
use egui::{Color32, Pos2, Rect, Response, Sense, Stroke, Ui, Vec2, Widget};
use uom::si::angle::radian;
use uom::si::area::square_meter;
use uom::si::f64::{Angle, Area, Length, MassRate, ThermodynamicTemperature, Time};
use uom::si::length::meter;
use uom::si::thermodynamic_temperature::kelvin;

/// Width of the pipe wall outline, in screen points.
///
/// Deliberately heavy: the wall is a structural element carrying its own metal
/// temperature, not a hairline border on the fluid.
const PIPE_WALL_WIDTH: f32 = 5.0;

/// Axial length of a tracer mark, as a fraction of the run length.
///
/// The mark is drawn as a rectangle spanning the pipe's full cross-section, so
/// it reads as a plug of fluid moving down the run rather than a dot floating
/// in it.
const TRACER_LENGTH_FRACTION: f32 = 0.06;

/// Scalar fluid state for a pipe run whose physics is not a
/// `tampines::components::Pipe`.
///
/// Every field is the caller's own real model state -- see
/// [`PipeArtState::Scalars`] for why this narrower interface exists.
#[derive(Debug, Clone, Copy)]
pub struct PipeScalars {
    /// Bulk fluid temperature of the run, used for the colour map.
    pub temperature: ThermodynamicTemperature,
    /// Mass flow through the run. Positive is `screen_position` ->
    /// `screen_position + screen_vector`; negative runs tracers in reverse.
    pub mass_flow: MassRate,
    /// End-to-end residence time, setting how long a tracer mark takes to
    /// cross the run (see [`crate::animation::residence_time_from_flow`]).
    pub residence_time: Time,
}

/// Per-cell state of a pipe run with real geometry, as a physics model holds
/// it: what [`PipeArtState::Cells`] draws.
///
/// The digital-twin engine's `PipeArt` fills this from a
/// `tampines::components::Pipe` (its flow backend's cell temperatures, the
/// TUAS shell's wall temperatures, the backend's phase, the bore, length and
/// inclination). Every field is real model state; nothing here is invented by
/// the art.
#[derive(Debug, Clone, PartialEq)]
pub struct PipeCells {
    /// Fluid temperature per finite-volume cell, inlet -> outlet. Empty when
    /// the backend cannot report a profile, which draws a neutral grey run.
    pub cell_temperatures: Vec<ThermodynamicTemperature>,
    /// Metal wall temperature per shell node, inlet -> outlet, or `None` when
    /// the backend carries no solid structure (unknown, never cold).
    pub wall_temperatures: Option<Vec<ThermodynamicTemperature>>,
    /// How the backend's fluid phase is shaded.
    pub phase_shade: PipePhaseShade,
    /// Flow cross-sectional area, from the bore.
    pub cross_sectional_area: Area,
    /// Physical run length.
    pub run_length: Length,
    /// Inclination from horizontal, positive uphill.
    pub inclination: Angle,
}

/// Where a [`PipeArt`] gets the state it renders.
///
/// Enum dispatch, not a trait object, per the workspace's mandatory
/// "no trait objects" Rust design rule.
#[derive(Debug, Clone)]
pub enum PipeArtState {
    /// Per-cell state read off a full TAMPINES pipe (by the engine's
    /// `PipeArt`), drawn one coloured segment per finite-volume cell.
    Cells(PipeCells),
    /// Backed by caller-supplied scalars, drawn as a single coloured run.
    Scalars(PipeScalars),
}

/// How plant-space metres map to screen points.
///
/// The two scales are deliberately **separate**. A pipe run is metres long and
/// its bore is millimetres across; a single scale would render every pipe as an
/// invisible hairline. Keeping them apart lets length stay a true scale drawing
/// while the cross-section is exaggerated enough to see.
///
/// That exaggeration is honest but must be stated: two pipes drawn side by side
/// have *lengths* in true proportion to each other and *cross-sections* in true
/// proportion to each other, but thickness is not on the same scale as length.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PipeScale {
    /// Screen points per metre of pipe length.
    pub points_per_metre: f32,
    /// Screen points of drawn thickness per square metre of flow
    /// cross-sectional area.
    ///
    /// Thickness is proportional to **area**, not diameter, so a pipe carrying
    /// four times the flow area draws four times as thick.
    pub points_per_square_metre: f32,
    /// Floor on drawn thickness, in points, so a small-bore pipe stays visible
    /// rather than vanishing.
    pub min_thickness_points: f32,
}

impl Default for PipeScale {
    /// Chosen so a 3 m run of 50 mm bore draws about 240 points long and
    /// 20 points thick — legible at gallery size.
    fn default() -> Self {
        Self {
            points_per_metre: 80.0,
            points_per_square_metre: 10_000.0,
            min_thickness_points: 6.0,
        }
    }
}

/// How the working fluid's phase is reflected in the drawing.
///
/// Enum dispatch per the workspace's "no trait objects" rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipePhaseShade {
    /// Liquid: full-strength colour.
    Liquid,
    /// Gas or vapour: lightened. A gas is orders of magnitude less dense than
    /// the liquid at the same temperature, and washing the colour out is the
    /// cheapest way to make that legible at a glance without adding a second
    /// colour axis the reader has to learn.
    Gas,
    /// Two-phase, where the backend carries phase information but this widget
    /// is not yet reading a per-cell quality from it: drawn between the two.
    TwoPhase,
}

impl PipePhaseShade {
    /// Fraction of the way towards white, 0.0 = untouched.
    fn lightening(self) -> f32 {
        match self {
            Self::Liquid => 0.0,
            Self::TwoPhase => 0.25,
            Self::Gas => 0.55,
        }
    }

    /// Apply this shade's lightening to a colour.
    ///
    /// Public so a bend can shade itself identically to the runs it joins —
    /// a joint that did not match its pipes would read as a different fluid.
    pub fn apply(self, c: Color32) -> Color32 {
        let f = self.lightening();
        let mix = |v: u8| -> u8 { (v as f32 + (255.0 - v as f32) * f).round() as u8 };
        Color32::from_rgb(mix(c.r()), mix(c.g()), mix(c.b()))
    }
}

/// Visual representation of a pipe run.
///
/// `screen_vector` gives the pipe's on-screen direction and length (from
/// `screen_position` to `screen_position + screen_vector`), which is also the
/// direction a positive mass flow's tracers travel.
pub struct PipeArt {
    /// The physics state this widget renders.
    pub state: PipeArtState,
    /// On-screen anchor position (the inlet endpoint of the pipe).
    pub screen_position: Pos2,
    /// On-screen direction and length, from `screen_position` to the outlet.
    pub screen_vector: Vec2,
    /// Temperature drawn in the coldest displayable colour (blue).
    ///
    /// The map is diverging, so the MIDPOINT of `[min_temp, max_temp]` is the
    /// neutral white point -- set the range symmetrically about a meaningful
    /// reference, not just to the extremes observed.
    pub min_temp: ThermodynamicTemperature,
    /// Temperature mapped to `hotness = 1.0` (hottest displayable colour).
    pub max_temp: ThermodynamicTemperature,
    /// Metres-to-points mapping for length and cross-section.
    pub scale: PipeScale,
    /// Metal temperature at or above which the pipe wall is drawn **red**.
    ///
    /// `None` (the default) never reddens. There is deliberately no built-in
    /// limit: an allowable metal temperature depends on the material, the
    /// code of construction and the duty, so the caller must supply the one
    /// that applies rather than inheriting a number invented here.
    pub wall_alarm_temp: Option<ThermodynamicTemperature>,
    /// A single tracer mark at an explicit position in `[0, 1]`, `0` = inlet.
    ///
    /// Set from [`crate::animation::TracerPulse`], which shows one mark at a
    /// time and reports `None` between releases — a train of marks strobes on
    /// a short or fast run. Independent of [`Self::tracer`]; both may be set.
    pub mark_at: Option<f64>,
    /// Optional flow-tracer marks drawn along the run.
    ///
    /// The train is *advanced by the application*, once per frame, and copied
    /// in here at widget-build time -- widgets are rebuilt every repaint, so a
    /// train owned by the widget would reset its phase to zero each frame.
    /// See [`crate::animation`] for the ownership rationale.
    pub tracer: Option<TracerTrain>,
}

impl PipeArt {
    /// Draw a run from per-cell model state ([`PipeCells`]) with the given
    /// screen geometry and colour-mapping temperature range. The run renders
    /// one coloured segment per finite-volume cell.
    pub fn from_cells(
        cells: PipeCells,
        screen_position: Pos2,
        screen_vector: Vec2,
        min_temp: ThermodynamicTemperature,
        max_temp: ThermodynamicTemperature,
    ) -> Self {
        Self {
            state: PipeArtState::Cells(cells),
            screen_position,
            screen_vector,
            min_temp,
            max_temp,
            scale: PipeScale::default(),
            wall_alarm_temp: None,
            mark_at: None,
            tracer: None,
        }
    }

    /// Build a pipe run from caller-supplied [`PipeScalars`] rather than a
    /// full pipe -- the connector-line path described in the module docs.
    pub fn from_scalars(
        scalars: PipeScalars,
        screen_position: Pos2,
        screen_vector: Vec2,
        min_temp: ThermodynamicTemperature,
        max_temp: ThermodynamicTemperature,
    ) -> Self {
        Self {
            state: PipeArtState::Scalars(scalars),
            screen_position,
            screen_vector,
            min_temp,
            max_temp,
            scale: PipeScale::default(),
            wall_alarm_temp: None,
            mark_at: None,
            tracer: None,
        }
    }

    /// Place a single tracer mark at `position` in `[0, 1]` along the run.
    /// Builder-style.
    pub fn with_mark_at(mut self, position: f64) -> Self {
        self.mark_at = Some(position.clamp(0.0, 1.0));
        self
    }

    /// Set the metal temperature at or above which the wall is drawn red.
    /// Builder-style. See [`PipeArt::wall_alarm_temp`] for why there is no
    /// default limit.
    pub fn with_wall_alarm(mut self, alarm: ThermodynamicTemperature) -> Self {
        self.wall_alarm_temp = Some(alarm);
        self
    }

    /// Override the metres-to-points mapping. Builder-style.
    pub fn with_scale(mut self, scale: PipeScale) -> Self {
        self.scale = scale;
        self
    }

    /// Attach an application-owned [`TracerTrain`] so this run draws flow
    /// tracer marks. Builder-style, so it chains onto either constructor.
    pub fn with_tracer(mut self, tracer: TracerTrain) -> Self {
        self.tracer = Some(tracer);
        self
    }

    /// Flow cross-sectional area, from the pipe's bore.
    ///
    /// `None` for [`PipeArtState::Scalars`], which carries no geometry —
    /// those runs fall back to [`PipeScale::min_thickness_points`] rather than
    /// inventing a bore.
    pub fn cross_sectional_area(&self) -> Option<Area> {
        match &self.state {
            PipeArtState::Scalars(_) => None,
            PipeArtState::Cells(c) => Some(c.cross_sectional_area),
        }
    }

    /// Physical run length, `None` for scalar-backed runs (no geometry).
    pub fn run_length(&self) -> Option<Length> {
        match &self.state {
            PipeArtState::Scalars(_) => None,
            PipeArtState::Cells(c) => Some(c.run_length),
        }
    }

    /// Inclination from horizontal, positive uphill. `None` for scalar runs.
    pub fn inclination(&self) -> Option<Angle> {
        match &self.state {
            PipeArtState::Scalars(_) => None,
            PipeArtState::Cells(c) => Some(c.inclination),
        }
    }

    /// Per-cell **metal wall** temperatures along the run, inlet -> outlet.
    ///
    /// `None` when the model carries no solid structure to report (see
    /// [`PipeCells::wall_temperatures`]): a pipe whose wall temperature is
    /// unknown must be drawn as unknown, never as cold.
    pub fn wall_temperatures(&self) -> Option<Vec<ThermodynamicTemperature>> {
        match &self.state {
            PipeArtState::Scalars(_) => None,
            PipeArtState::Cells(c) => c.wall_temperatures.clone(),
        }
    }

    /// Metal wall temperature at fluid cell `i` of `n`.
    ///
    /// The solid shell array need not have the same node count as the fluid
    /// array, so the fluid cell is mapped proportionally onto the shell rather
    /// than assumed index-for-index. Returns `None` when the backend reports no
    /// wall at all.
    pub fn wall_temperature_at(
        walls: &[ThermodynamicTemperature],
        i: usize,
        n: usize,
    ) -> Option<ThermodynamicTemperature> {
        if walls.is_empty() || n == 0 {
            return None;
        }
        let f = (i as f32 + 0.5) / n as f32;
        let idx = ((f * walls.len() as f32) as usize).min(walls.len() - 1);
        Some(walls[idx])
    }

    /// Hottest metal wall temperature on the run, if the backend reports one.
    pub fn peak_wall_temperature(&self) -> Option<ThermodynamicTemperature> {
        self.wall_temperatures()?.into_iter().reduce(|a, b| {
            if b.get::<kelvin>() > a.get::<kelvin>() {
                b
            } else {
                a
            }
        })
    }

    /// How the fluid's phase is shaded: [`PipeCells::phase_shade`], or
    /// liquid for caller-supplied scalars, which say nothing about phase.
    pub fn phase_shade(&self) -> PipePhaseShade {
        match &self.state {
            PipeArtState::Scalars(_) => PipePhaseShade::Liquid,
            PipeArtState::Cells(c) => c.phase_shade,
        }
    }

    /// Drawn size in screen points: `(length, thickness)`.
    ///
    /// Length is proportional to the real run length and thickness to the real
    /// flow cross-sectional area, both through [`PipeScale`]. A scalar-backed
    /// run has no geometry, so it falls back to the caller's `screen_vector`
    /// length and the minimum thickness.
    pub fn drawn_size(&self) -> (f32, f32) {
        let length = match self.run_length() {
            Some(l) => l.get::<meter>() as f32 * self.scale.points_per_metre,
            None => self.screen_vector.length(),
        };
        let thickness = match self.cross_sectional_area() {
            Some(a) => (a.get::<square_meter>() as f32 * self.scale.points_per_square_metre)
                .max(self.scale.min_thickness_points),
            None => self.scale.min_thickness_points,
        };
        (length.max(1.0), thickness)
    }

    /// Per-cell fluid temperatures along the run, inlet -> outlet.
    ///
    /// For [`PipeArtState::Cells`] this is the model's finite-volume
    /// temperature profile; for [`PipeArtState::Scalars`] it is the single
    /// supplied bulk temperature. Empty if the model reported no profile --
    /// the run is then drawn grey rather than in a fabricated colour.
    pub fn cell_temperatures(&self) -> Vec<ThermodynamicTemperature> {
        match &self.state {
            PipeArtState::Scalars(s) => vec![s.temperature],
            PipeArtState::Cells(c) => c.cell_temperatures.clone(),
        }
    }
}

impl Widget for PipeArt {
    /// Draws the run as a rectangle divided into one box per finite-volume
    /// cell (see [`PipeArt::cell_temperatures`]), then any tracer marks.
    ///
    /// Length and thickness both come from the real pipe geometry via
    /// [`PipeScale`] — length from the run length, thickness from the flow
    /// cross-sectional area — and the run is drawn at the pipe's inclination.
    /// Each cell box is filled by its own temperature, then lightened
    /// according to [`PipeArt::phase_shade`] so a gas-filled pipe reads
    /// paler than a liquid one at the same temperature.
    ///
    /// Cell colours come from [`crate::components::temperature_colour`] over the
    /// `[min_temp, max_temp]` range. If the backend reports no cells the run
    /// is drawn in a neutral grey rather than a made-up temperature colour,
    /// so an unpopulated pipe is visibly distinct from a cold one.
    fn ui(self, ui: &mut Ui) -> Response {
        let (length_pts, thickness_pts) = self.drawn_size();
        let half_t = 0.5 * thickness_pts;

        // Run direction. A cell-backed pipe knows its inclination, so the
        // run is drawn at that slope -- this is what lets a natural-circulation
        // loop read correctly, with hot legs rising and cold legs falling.
        // Screen y grows downwards, so a positive (uphill) inclination must
        // take the run UP the screen.
        let direction = match self.inclination() {
            Some(incline) => {
                let a = incline.get::<radian>() as f32;
                Vec2::new(a.r_cos(), -a.r_sin())
            }
            None => {
                let v = self.screen_vector;
                if v.length() > f32::EPSILON {
                    v.normalized()
                } else {
                    Vec2::new(1.0, 0.0)
                }
            }
        };
        let normal = Vec2::new(-direction.y, direction.x);

        let start = self.screen_position;
        let end = start + direction * length_pts;
        let rect = Rect::from_two_pos(start, end).expand(half_t.max(1.0));
        let response = ui.allocate_rect(rect, Sense::hover());
        let painter = ui.painter();

        let temperatures = self.cell_temperatures();
        let shade = self.phase_shade();

        // A rectangle spanning the run, divided into one box per cell.
        let quad = |p0: Pos2, p1: Pos2| -> Vec<Pos2> {
            vec![
                p0 + normal * half_t,
                p1 + normal * half_t,
                p1 - normal * half_t,
                p0 - normal * half_t,
            ]
        };

        if temperatures.is_empty() {
            // No cells reported: neutral grey, never a made-up temperature
            // colour, so an unpopulated pipe stays distinct from a cold one.
            painter.add(egui::Shape::convex_polygon(
                quad(start, end),
                Color32::GRAY,
                Stroke::NONE,
            ));
            return response;
        }

        // One box per finite-volume cell, inlet -> outlet, each filled by its
        // own cell temperature. Drawn WITHOUT an outline: the divisions between
        // cells are wall structure and are stroked separately below, in the
        // wall's own colour and weight.
        let n = temperatures.len();
        for (i, t) in temperatures.iter().enumerate() {
            let f0 = i as f32 / n as f32;
            let f1 = (i + 1) as f32 / n as f32;
            let p0 = start + direction * (length_pts * f0);
            let p1 = start + direction * (length_pts * f1);
            let fill = shade.apply(temperature_colour(*t, self.min_temp, self.max_temp));
            painter.add(egui::Shape::convex_polygon(
                quad(p0, p1),
                fill,
                Stroke::NONE,
            ));
        }

        // Cell dividers: same grey and same weight as the pipe wall, because
        // they represent the same metal. Each reddens on the MEAN of the two
        // solid cells it sits between -- a divider is shared by both, so
        // keying it to either one alone would misreport which side is hot.
        let walls = self.wall_temperatures().unwrap_or_default();
        for i in 1..n {
            let mean_wall = match (
                Self::wall_temperature_at(&walls, i - 1, n),
                Self::wall_temperature_at(&walls, i, n),
            ) {
                (Some(a), Some(b)) => Some(ThermodynamicTemperature::new::<kelvin>(
                    0.5 * (a.get::<kelvin>() + b.get::<kelvin>()),
                )),
                _ => None,
            };
            let colour = match (mean_wall, self.wall_alarm_temp) {
                (Some(m), Some(alarm)) if m.get::<kelvin>() >= alarm.get::<kelvin>() => {
                    Color32::from_rgb(220, 50, 40)
                }
                _ => Color32::from_gray(110),
            };
            let at = start + direction * (length_pts * (i as f32 / n as f32));
            painter.line_segment(
                [at + normal * half_t, at - normal * half_t],
                Stroke::new(PIPE_WALL_WIDTH, colour),
            );
        }

        // Pipe wall, drawn over the cell boxes so the run reads as one pipe.
        // Grey by default; RED once the metal reaches the caller's alarm
        // temperature. A backend with no solid structure reports no wall
        // temperature and stays grey — unknown is drawn as unknown, never as
        // safe.
        let wall_colour = match (self.peak_wall_temperature(), self.wall_alarm_temp) {
            (Some(peak), Some(alarm)) if peak.get::<kelvin>() >= alarm.get::<kelvin>() => {
                Color32::from_rgb(220, 50, 40)
            }
            _ => Color32::from_gray(110),
        };
        painter.add(egui::Shape::convex_polygon(
            quad(start, end),
            Color32::TRANSPARENT,
            Stroke::new(PIPE_WALL_WIDTH, wall_colour),
        ));

        // Tracer marks: white rectangles spanning the bore, each travelling
        // the full run in exactly one residence time (the train is advanced by
        // the application -- see the field docs). Direction of travel is the
        // train's, so a reversed flow runs them backwards.
        // Explicit single mark (from a TracerPulse), plus any train marks.
        let marks: Vec<f64> = self
            .mark_at
            .into_iter()
            .chain(self.tracer.iter().flat_map(|t| t.positions()))
            .collect();
        if !marks.is_empty() {
            let mark_len = (length_pts * TRACER_LENGTH_FRACTION).max(2.0);
            for position in marks {
                let centre = length_pts * position as f32;
                // Clipped to the run so a mark straddling the outlet does not
                // spill out of the pipe.
                let a = (centre - 0.5 * mark_len).clamp(0.0, length_pts);
                let b = (centre + 0.5 * mark_len).clamp(0.0, length_pts);
                if b - a < 0.5 {
                    continue;
                }
                painter.add(egui::Shape::convex_polygon(
                    quad(start + direction * a, start + direction * b),
                    Color32::WHITE,
                    Stroke::NONE,
                ));
            }
        }

        response
    }
}

/// Bore geometry of a **coaxial** run — a duct inside a duct, the two annular
/// spaces carrying different streams in opposite directions.
///
/// All lengths are real plant dimensions. The drawn picture is a longitudinal
/// section, so the radii below set the band proportions across the run.
///
/// The radial layout, axis outward:
///
/// ```text
///   ┌──────────────────────────  outer tube wall
///   │ ← ← ← annulus              (return stream)
///   │ ▒▒▒▒▒ insulation
///   │ ───── inner tube wall
///   │ → → → inner bore           (supply stream)
///   │ ───── inner tube wall
///   │ ▒▒▒▒▒ insulation
///   │ ← ← ← annulus
///   └──────────────────────────  outer tube wall
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CoaxialDuctGeometry {
    /// Inside diameter of the **inner** tube — the bore of the supply stream.
    pub inner_bore: Length,
    /// Inside diameter of the **outer** tube. The annulus is the space between
    /// this and [`Self::inner_bore`].
    pub outer_bore: Length,
    /// Share of the **radial annulus gap** taken up by thermal insulation,
    /// dimensionless in `[0, 1)`, measured outward from the inner tube.
    ///
    /// Insulation thickness is a *drawing* parameter, not a plant dimension:
    /// see [`Self::htr10_hot_gas_duct`], where the source establishes that
    /// insulation is present but never states how thick it is.
    pub insulation_fraction: f32,
}

impl CoaxialDuctGeometry {
    /// The HTR-10 hot gas duct: the horizontal coaxial duct inside the
    /// connecting cross-vessel that ties the reactor pressure vessel to the
    /// steam-generator pressure vessel.
    ///
    /// | Quantity | Value |
    /// |---|---|
    /// | Inner (hot helium) tube diameter | 300 mm |
    /// | Outer (cold helium) tube diameter | 900 mm |
    /// | Annulus radial gap | 300 mm |
    /// | Orientation | horizontal |
    ///
    /// Hot helium runs core-to-SG in the centre tube; cold helium runs
    /// circulator-to-core in the annulus, so the two streams are
    /// counter-current. Thermal insulation is installed between the inner and
    /// outer tubes, and the inner tube runs at high temperature but **low
    /// pressure differential** — the cold annulus carries the pressure.
    ///
    /// Source: `docs/reactor-scoping/htr10-plant-data.md` section 4.1, which
    /// records these as *Quoted* from its [S2] and cross-checked against [S5].
    ///
    /// **Two things that table marks as unknown, and this constructor
    /// therefore does not invent:**
    ///
    /// - The duct **length** is not stated in any source that document
    ///   reviews. This type carries no length: the caller sets the drawn run
    ///   length through `screen_vector`, and a schematic using it should say
    ///   that the length shown is a layout choice rather than a dimension.
    /// - The insulation **thickness** is not stated either. The default
    ///   [`Self::insulation_fraction`] here is an indicative 0.2 of the radial
    ///   gap, chosen only so the layer is visible. Do not quote it as a plant
    ///   dimension or derive a heat loss from it.
    pub fn htr10_hot_gas_duct() -> Self {
        Self {
            inner_bore: Length::new::<meter>(0.300),
            outer_bore: Length::new::<meter>(0.900),
            insulation_fraction: 0.2,
        }
    }

    /// Radial gap between the inner tube's outside and the outer tube's
    /// inside: `(outer_bore - inner_bore) / 2`.
    ///
    /// For the HTR-10 duct this is 300 mm, matching the value the plant-data
    /// sheet derives the same way.
    pub fn annulus_radial_gap(&self) -> Length {
        (self.outer_bore - self.inner_bore) * 0.5
    }

    /// Flow area of the inner bore, `pi d^2 / 4`.
    pub fn inner_flow_area(&self) -> Area {
        let d = self.inner_bore.get::<meter>();
        Area::new::<square_meter>(std::f64::consts::PI * d * d * 0.25)
    }

    /// Flow area of the annulus, `pi (D^2 - d^2) / 4`.
    ///
    /// Ignores the insulation and the inner tube's wall thickness, neither of
    /// which is a stated dimension, so this is the **gross** annular area
    /// rather than a net flow area.
    pub fn annulus_flow_area(&self) -> Area {
        let d = self.inner_bore.get::<meter>();
        let big = self.outer_bore.get::<meter>();
        Area::new::<square_meter>(std::f64::consts::PI * (big * big - d * d) * 0.25)
    }

    /// Inner bore as a fraction of the outer bore, which is what sets the band
    /// proportions in the drawing. `0.333` for the HTR-10 duct.
    pub fn inner_radius_fraction(&self) -> f32 {
        let outer = self.outer_bore.get::<meter>();
        if outer <= 0.0 {
            return 0.0;
        }
        (self.inner_bore.get::<meter>() / outer).clamp(0.0, 1.0) as f32
    }
}

/// Visual representation of a **coaxial duct** run: two streams, one inside
/// the other, drawn as a longitudinal section.
///
/// The counterpart of [`PipeArt`] for the case a single-bore pipe cannot
/// represent — a supply and a return sharing one duct body. Both streams take
/// [`PipeScalars`], the same narrow interface [`PipeArtState::Scalars`]
/// uses, so a simulator whose loop physics is its own lumped model supplies
/// that model's real state and gets correct colour and tracer motion.
///
/// **Direction comes from the physics, not from an assumption.**
/// `screen_vector` is the duct **axis**, not a flow direction. Each stream's
/// tracers travel the way its own [`PipeScalars::mass_flow`] points: positive
/// runs along `screen_vector`, negative runs against it. So the
/// counter-current arrangement a coaxial duct is usually built for is a
/// *consequence* of the state the caller supplies — pass a positive inner
/// flow and a negative annulus flow — rather than something this widget
/// imposes. A co-current duct, or a duct with one stream stalled, draws
/// correctly for the same reason, and **a flow that reverses in a transient
/// reverses its tracers** instead of continuing to animate a direction the
/// model no longer has.
///
/// **Scale.** Unlike [`PipeArt`], drawn thickness is **not** derived from
/// flow area through a [`PipeScale`]: a coaxial duct's whole readability
/// depends on the *ratio* of its two bores, which a per-stream area scale
/// would destroy. Thickness is set directly by
/// [`Self::drawn_outer_thickness`], and the bands within it are in true
/// proportion to the real bores through
/// [`CoaxialDuctGeometry::inner_radius_fraction`]. The run **length** is the
/// caller's `screen_vector` — see [`CoaxialDuctGeometry::htr10_hot_gas_duct`]
/// on why no real length is carried.
pub struct CoaxialDuctVisual {
    /// Bore geometry, setting the band proportions.
    pub geometry: CoaxialDuctGeometry,
    /// On-screen anchor: the inner stream's inlet endpoint.
    pub screen_position: Pos2,
    /// On-screen direction and length, the inner stream's inlet to outlet.
    pub screen_vector: Vec2,
    /// Fluid state of the **inner** bore.
    pub core: PipeScalars,
    /// Fluid state of the **annulus**, flowing the opposite way.
    pub annulus: PipeScalars,
    /// Temperature drawn in the coldest displayable colour.
    pub min_temp: ThermodynamicTemperature,
    /// Temperature drawn in the hottest displayable colour.
    pub max_temp: ThermodynamicTemperature,
    /// Total drawn thickness of the duct across the run, in screen points —
    /// the outer tube's outside, wall to wall.
    pub drawn_outer_thickness: f32,
    /// Tracer marks for the inner stream. Advanced by the application, once
    /// per frame; see [`crate::animation`].
    pub core_tracer: Option<TracerTrain>,
    /// Tracer marks for the annulus stream.
    pub annulus_tracer: Option<TracerTrain>,
    /// Whether to draw the stream labels.
    pub show_labels: bool,
}

impl CoaxialDuctVisual {
    /// Build a coaxial duct run.
    ///
    /// `core` is the inner bore's state and `annulus` the annulus's; the
    /// annulus is drawn flowing against `screen_vector`.
    pub fn new(
        geometry: CoaxialDuctGeometry,
        screen_position: Pos2,
        screen_vector: Vec2,
        core: PipeScalars,
        annulus: PipeScalars,
        min_temp: ThermodynamicTemperature,
        max_temp: ThermodynamicTemperature,
    ) -> Self {
        Self {
            geometry,
            screen_position,
            screen_vector,
            core,
            annulus,
            min_temp,
            max_temp,
            drawn_outer_thickness: 46.0,
            core_tracer: None,
            annulus_tracer: None,
            show_labels: true,
        }
    }

    /// Set the total drawn thickness, in screen points. Builder-style.
    pub fn with_drawn_thickness(mut self, points: f32) -> Self {
        self.drawn_outer_thickness = points.max(6.0);
        self
    }

    /// Attach the inner-stream tracer train. Builder-style.
    pub fn with_core_tracer(mut self, tracer: TracerTrain) -> Self {
        self.core_tracer = Some(tracer);
        self
    }

    /// Attach the annulus tracer train. Builder-style.
    pub fn with_annulus_tracer(mut self, tracer: TracerTrain) -> Self {
        self.annulus_tracer = Some(tracer);
        self
    }

    /// Turn the stream labels off — for thumbnails. Builder-style.
    pub fn without_labels(mut self) -> Self {
        self.show_labels = false;
        self
    }
}

impl Widget for CoaxialDuctVisual {
    /// Draws the duct as a longitudinal section: the annulus as the outer
    /// body, an insulation layer inside it, and the inner bore on the axis,
    /// each filled by its own stream's temperature through the shared
    /// [`crate::components::temperature_colour`] map.
    ///
    /// Each stream's tracer marks travel **in the direction that stream is
    /// actually flowing**: [`crate::animation::TracerTrain`] takes the
    /// direction from the sign of the mass flow it is advanced with, so this
    /// widget applies none of its own. In the usual counter-current duct the
    /// two sets of marks visibly run opposite ways, and a stream that reverses
    /// or stalls reverses or freezes on screen. Annulus marks are drawn in
    /// both the upper and lower annular bands, since a longitudinal section
    /// cuts the annulus twice.
    fn ui(self, ui: &mut Ui) -> Response {
        let direction = if self.screen_vector.length() > f32::EPSILON {
            self.screen_vector.normalized()
        } else {
            Vec2::new(1.0, 0.0)
        };
        let normal = Vec2::new(-direction.y, direction.x);
        let length_pts = self.screen_vector.length().max(1.0);
        let half = 0.5 * self.drawn_outer_thickness;

        let start = self.screen_position;
        let end = start + direction * length_pts;
        let rect = Rect::from_two_pos(start, end).expand(half.max(1.0));
        let response = ui.allocate_rect(rect, Sense::hover());
        let painter = ui.painter();

        // A band of the section, given signed offsets from the axis.
        let band = |a: f32, b: f32| -> Vec<Pos2> {
            vec![
                start + normal * a,
                end + normal * a,
                end + normal * b,
                start + normal * b,
            ]
        };

        let r_inner = self.geometry.inner_radius_fraction() * half;
        let gap = half - r_inner;
        let r_insulation = r_inner + gap * self.geometry.insulation_fraction.clamp(0.0, 0.95);

        let cold = temperature_colour(self.annulus.temperature, self.min_temp, self.max_temp);
        let hot = temperature_colour(self.core.temperature, self.min_temp, self.max_temp);

        // Annulus: the full outer body, drawn first so the inner layers sit
        // on top of it.
        painter.add(egui::Shape::convex_polygon(
            band(-half, half),
            cold,
            Stroke::NONE,
        ));
        // Insulation, hugging the inner tube on both sides.
        for sign in [-1.0_f32, 1.0] {
            painter.add(egui::Shape::convex_polygon(
                band(sign * r_inner, sign * r_insulation),
                Color32::from_gray(155),
                Stroke::NONE,
            ));
        }
        // Inner bore.
        painter.add(egui::Shape::convex_polygon(
            band(-r_inner, r_inner),
            hot,
            Stroke::NONE,
        ));

        // Tube walls: the inner tube and the outer tube, same weight and grey
        // as a single-bore pipe's wall, because they are the same metal.
        let wall = Color32::from_gray(110);
        painter.add(egui::Shape::convex_polygon(
            band(-half, half),
            Color32::TRANSPARENT,
            Stroke::new(PIPE_WALL_WIDTH, wall),
        ));
        painter.add(egui::Shape::convex_polygon(
            band(-r_inner, r_inner),
            Color32::TRANSPARENT,
            Stroke::new(PIPE_WALL_WIDTH * 0.7, wall),
        ));

        // Tracer marks.
        //
        // Direction is NOT applied here, and deliberately so: `TracerTrain`
        // already derives it from the sign of the mass flow it is advanced
        // with, decrementing its phase for a reverse flow. Mirroring the
        // position here as well would flip it a second time and animate a
        // reversed stream *forwards* — a hardcoded direction cancelling a
        // derived one. Each stream therefore runs whichever way the caller's
        // own mass flow says, and the usual counter-current duct falls out of
        // advancing the annulus train with a negative flow.
        let mark_len = (length_pts * TRACER_LENGTH_FRACTION).max(2.0);
        let plug = |position: f64, a: f32, b: f32| {
            let centre = length_pts * position as f32;
            let p0 = (centre - 0.5 * mark_len).clamp(0.0, length_pts);
            let p1 = (centre + 0.5 * mark_len).clamp(0.0, length_pts);
            if p1 - p0 < 0.5 {
                return;
            }
            let s = start + direction * p0;
            let e = start + direction * p1;
            painter.add(egui::Shape::convex_polygon(
                vec![
                    s + normal * a,
                    e + normal * a,
                    e + normal * b,
                    s + normal * b,
                ],
                Color32::WHITE,
                Stroke::NONE,
            ));
        };

        if let Some(train) = &self.core_tracer {
            for position in train.positions() {
                plug(position, -r_inner, r_inner);
            }
        }
        if let Some(train) = &self.annulus_tracer {
            for position in train.positions() {
                plug(position, r_insulation, half);
                plug(position, -half, -r_insulation);
            }
        }

        if self.show_labels {
            let mid = start + direction * (length_pts * 0.5);
            painter.text(
                mid - normal * (half + 9.0),
                egui::Align2::CENTER_CENTER,
                "cold annulus",
                egui::FontId::proportional(9.0),
                Color32::from_gray(150),
            );
            painter.text(
                mid + normal * (half + 9.0),
                egui::Align2::CENTER_CENTER,
                "hot inner tube",
                egui::FontId::proportional(9.0),
                Color32::from_gray(150),
            );
        }

        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::hotness_from_temperature;
    use uom::si::mass_rate::kilogram_per_second;
    use uom::si::time::second;

    fn scalars(t_k: f64) -> PipeScalars {
        PipeScalars {
            temperature: ThermodynamicTemperature::new::<kelvin>(t_k),
            mass_flow: MassRate::new::<kilogram_per_second>(1.0),
            residence_time: Time::new::<second>(5.0),
        }
    }

    fn visual(state: PipeArtState) -> PipeArt {
        PipeArt {
            state,
            screen_position: Pos2::ZERO,
            screen_vector: Vec2::new(100.0, 0.0),
            min_temp: ThermodynamicTemperature::new::<kelvin>(300.0),
            max_temp: ThermodynamicTemperature::new::<kelvin>(400.0),
            scale: PipeScale::default(),
            wall_alarm_temp: None,
            mark_at: None,
            tracer: None,
        }
    }

    #[test]
    fn scalar_state_reports_its_single_temperature() {
        let v = visual(PipeArtState::Scalars(scalars(350.0)));
        let temps = v.cell_temperatures();
        assert_eq!(temps.len(), 1);
        assert_eq!(temps[0].get::<kelvin>(), 350.0);
    }

    #[test]
    fn from_scalars_builds_a_scalar_backed_run() {
        let v = PipeArt::from_scalars(
            scalars(360.0),
            Pos2::ZERO,
            Vec2::new(10.0, 0.0),
            ThermodynamicTemperature::new::<kelvin>(300.0),
            ThermodynamicTemperature::new::<kelvin>(400.0),
        );
        assert!(matches!(v.state, PipeArtState::Scalars(_)));
        assert!(v.tracer.is_none());
    }

    #[test]
    fn with_tracer_attaches_an_application_owned_train() {
        let v = visual(PipeArtState::Scalars(scalars(350.0))).with_tracer(TracerTrain::new(3));
        let tracer = v.tracer.expect("tracer should be attached");
        assert_eq!(tracer.count(), 3);
    }

    // `thickness_is_proportional_to_cross_sectional_area` and
    // `length_is_proportional_to_run_length` MOVED 2026-11-05 to
    // `outram_park_digital_twin_engine::components::pipe`: they build a
    // TAMPINES pipe on a TUAS material, and bugis depends on neither.

    /// Cell-backed art takes its drawn size from the supplied geometry and
    /// draws one box per cell.
    #[test]
    fn a_cell_backed_run_sizes_from_its_geometry_and_draws() {
        use uom::si::area::square_meter;
        let k = |v: f64| ThermodynamicTemperature::new::<kelvin>(v);
        let cells = |area_m2: f64, length_m: f64| PipeCells {
            cell_temperatures: vec![k(320.0), k(340.0), k(360.0)],
            wall_temperatures: None,
            phase_shade: PipePhaseShade::Liquid,
            cross_sectional_area: Area::new::<square_meter>(area_m2),
            run_length: Length::new::<meter>(length_m),
            inclination: Angle::new::<radian>(0.0),
        };
        let art = |a: f64, l: f64| {
            PipeArt::from_cells(cells(a, l), Pos2::new(10.0, 10.0), Vec2::new(100.0, 0.0), k(300.0), k(400.0))
        };
        let (l1, t1) = art(0.002, 3.0).drawn_size();
        let (l2, t2) = art(0.008, 6.0).drawn_size();
        assert!((t2 / t1 - 4.0).abs() < 1e-3);
        assert!((l2 / l1 - 2.0).abs() < 1e-3);
        assert_eq!(art(0.002, 3.0).cell_temperatures().len(), 3);
        assert!(art(0.002, 3.0).wall_temperatures().is_none());
        assert_eq!(art(0.002, 3.0).inclination(), Some(Angle::new::<radian>(0.0)));
        assert!(crate::components::shapes_drawn_by(art(0.002, 3.0)) >= 4);
    }

    /// A divider sits between two solid cells and must key off their MEAN,
    /// not either one alone — otherwise it misreports which side is hot.
    ///
    /// **Methodology:** map a 4-entry wall array onto 4 fluid cells and check
    /// the proportional mapping picks each cell's own wall node, then verify
    /// the mean of two adjacent nodes is what a divider would use.
    ///
    /// **Result (2026-08-05):** cells 0..3 map to wall nodes 0..3; the mean of
    /// nodes 1 and 2 (600 K and 900 K) is 750 K, which is below an 800 K alarm
    /// even though one side is above it. Keying to the hotter side alone would
    /// have raised a false alarm on that divider.
    #[test]
    fn divider_uses_the_mean_of_its_two_solid_cells() {
        let k = |v: f64| ThermodynamicTemperature::new::<kelvin>(v);
        let walls = vec![k(300.0), k(600.0), k(900.0), k(1000.0)];
        let n = 4;
        let a = PipeArt::wall_temperature_at(&walls, 1, n).unwrap();
        let b = PipeArt::wall_temperature_at(&walls, 2, n).unwrap();
        assert_eq!(a.get::<kelvin>(), 600.0);
        assert_eq!(b.get::<kelvin>(), 900.0);
        let mean = 0.5 * (a.get::<kelvin>() + b.get::<kelvin>());
        assert_eq!(mean, 750.0);
        assert!(mean < 800.0, "mean must not trip an 800 K alarm");
        assert!(b.get::<kelvin>() > 800.0, "but one side alone would have");
    }

    /// The solid shell need not have the same node count as the fluid array,
    /// so the mapping must be proportional rather than index-for-index.
    #[test]
    fn wall_mapping_is_proportional_not_index_for_index() {
        let k = |v: f64| ThermodynamicTemperature::new::<kelvin>(v);
        // 2 wall nodes spread over 8 fluid cells: first half cold, second hot.
        let walls = vec![k(300.0), k(900.0)];
        assert_eq!(
            PipeArt::wall_temperature_at(&walls, 0, 8)
                .unwrap()
                .get::<kelvin>(),
            300.0
        );
        assert_eq!(
            PipeArt::wall_temperature_at(&walls, 7, 8)
                .unwrap()
                .get::<kelvin>(),
            900.0
        );
    }

    /// A backend with no solid structure reports no wall, and must not be
    /// given a fabricated one.
    #[test]
    fn no_wall_array_reports_none() {
        assert!(PipeArt::wall_temperature_at(&[], 0, 4).is_none());
    }

    /// Phase shading must lighten a gas relative to a liquid at the same
    /// temperature, and must never darken it.
    #[test]
    fn gas_is_lighter_than_liquid() {
        let base = crate::colour_maps::color32::crameri::vik(0.5);
        let liquid = PipePhaseShade::Liquid.apply(base);
        let gas = PipePhaseShade::Gas.apply(base);
        let two_phase = PipePhaseShade::TwoPhase.apply(base);
        assert_eq!(liquid, base, "liquid must be untouched");
        assert!(gas.r() >= two_phase.r() && two_phase.r() >= liquid.r());
        assert!(gas.g() >= two_phase.g() && two_phase.g() >= liquid.g());
        assert!(gas.b() >= two_phase.b() && two_phase.b() >= liquid.b());
    }

    /// The colour map must key off the *cell* temperature, so a mid-range
    /// cell maps to hotness 0.5 over a [300, 400] K display range.
    #[test]
    fn cell_temperature_drives_hotness() {
        let v = visual(PipeArtState::Scalars(scalars(350.0)));
        let t = v.cell_temperatures()[0];
        assert_eq!(hotness_from_temperature(t, v.min_temp, v.max_temp), 0.5);
    }
}
