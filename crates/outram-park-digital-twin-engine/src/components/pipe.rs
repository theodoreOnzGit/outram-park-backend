//! Visual pipe: the physics wrapper.
//!
//! [`PipeVisualState`] is an enum (not a trait object, per the workspace's
//! mandatory design rules) with two variants, because digital-twin
//! applications reach this widget from two directions:
//!
//! - [`PipeVisualState::Physics`] wraps a full
//!   [`tampines::components::Pipe`]. The pipe's per-cell temperature profile
//!   is read straight off its flow backend, so the run is drawn as one
//!   coloured segment **per finite-volume cell**.
//! - [`PipeVisualState::Scalars`] takes a [`PipeScalars`] triple
//!   (temperature, mass flow, residence time) directly; see
//!   [`bugis::components::pipe`] for why that narrower interface exists.
//!
//! **Split 2026-11-05** (maintainer: "widget art can live in bugis, the
//! combination of logic and art can live inside the digital twin engine"):
//! the drawing, [`PipeArt`], the scales and shades, and the coaxial duct are
//! [`bugis::components::pipe`], re-exported here. This file reads a TAMPINES
//! pipe into [`PipeCells`] and draws through [`PipeArt`].

pub use bugis::components::pipe::*;
use bugis::animation::TracerTrain;
use egui::{Pos2, Response, Ui, Vec2, Widget};
use tampines::components::{Pipe, PipeBackend};
use uom::si::f64::{Angle, Area, Length, ThermodynamicTemperature};
use uom::si::thermodynamic_temperature::kelvin;

/// Where a [`PipeVisual`] gets the physics it renders.
///
/// Enum dispatch, not a trait object, per the workspace's mandatory
/// "no trait objects" Rust design rule.
#[derive(Debug, Clone)]
pub enum PipeVisualState {
    /// Backed by a full TAMPINES pipe, drawn one coloured segment per
    /// finite-volume cell.
    Physics(Pipe),
    /// Backed by caller-supplied scalars, drawn as a single coloured run.
    Scalars(PipeScalars),
}

/// Visual representation of a pipe run.
///
/// `screen_vector` gives the pipe's on-screen direction and length (from
/// `screen_position` to `screen_position + screen_vector`), which is also the
/// direction a positive mass flow's tracers travel.
pub struct PipeVisual {
    /// The physics state this widget renders.
    pub state: PipeVisualState,
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

impl PipeVisual {
    /// Wrap a [`Pipe`] with the given screen geometry and colour-mapping
    /// temperature range. The run renders one coloured segment per
    /// finite-volume cell of the pipe's flow backend.
    pub fn new(
        physics: Pipe,
        screen_position: Pos2,
        screen_vector: Vec2,
        min_temp: ThermodynamicTemperature,
        max_temp: ThermodynamicTemperature,
    ) -> Self {
        Self {
            state: PipeVisualState::Physics(physics),
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
    /// full [`Pipe`] -- the connector-line path described in the module docs.
    pub fn from_scalars(
        scalars: PipeScalars,
        screen_position: Pos2,
        screen_vector: Vec2,
        min_temp: ThermodynamicTemperature,
        max_temp: ThermodynamicTemperature,
    ) -> Self {
        Self {
            state: PipeVisualState::Scalars(scalars),
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
    /// Builder-style. See [`PipeVisual::wall_alarm_temp`] for why there is no
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
    /// `None` for [`PipeVisualState::Scalars`], which carries no geometry —
    /// those runs fall back to [`PipeScale::min_thickness_points`] rather than
    /// inventing a bore.
    pub fn cross_sectional_area(&self) -> Option<Area> {
        match &self.state {
            PipeVisualState::Scalars(_) => None,
            PipeVisualState::Physics(pipe) => {
                let d = pipe.diameter;
                Some(std::f64::consts::FRAC_PI_4 * d * d)
            }
        }
    }

    /// Physical run length, `None` for scalar-backed runs (no geometry).
    pub fn run_length(&self) -> Option<Length> {
        match &self.state {
            PipeVisualState::Scalars(_) => None,
            PipeVisualState::Physics(pipe) => Some(pipe.length),
        }
    }

    /// Inclination from horizontal, positive uphill. `None` for scalar runs.
    pub fn inclination(&self) -> Option<Angle> {
        match &self.state {
            PipeVisualState::Scalars(_) => None,
            PipeVisualState::Physics(pipe) => Some(pipe.inclination),
        }
    }

    /// Per-cell **metal wall** temperatures along the run, inlet -> outlet.
    ///
    /// `None` when the backend carries no solid structure to report. Only the
    /// TUAS pre-built [`PipeBackend::InsulatedPipe`] couples a pipe shell to
    /// its fluid; the bare arrays model the fluid alone, and a wall
    /// temperature for them would have to be invented.
    ///
    /// That distinction is the point: a pipe whose wall temperature is unknown
    /// must be drawn as unknown, never as cold.
    pub fn wall_temperatures(&self) -> Option<Vec<ThermodynamicTemperature>> {
        match &self.state {
            PipeVisualState::Scalars(_) => None,
            PipeVisualState::Physics(pipe) => match &pipe.backend {
                PipeBackend::Lumped(_)
                | PipeBackend::Compressible(_)
                | PipeBackend::SteamHem(_) => None,
                PipeBackend::InsulatedPipe(component) => {
                    component.clone().pipe_shell_temperature().ok()
                }
            },
        }
    }

    /// Hottest metal wall temperature on the run, if the backend reports one.
    pub fn peak_wall_temperature(&self) -> Option<ThermodynamicTemperature> {
        self.art().peak_wall_temperature()
    }

    /// How this backend's fluid phase is shaded.
    ///
    /// Derived from the backend, which is the only phase information available
    /// without reading a per-cell quality: a lumped liquid array cannot boil, a
    /// CoolProp compressible array is being used for a gas, and the HEM array
    /// is the one that genuinely carries two-phase state.
    pub fn phase_shade(&self) -> PipePhaseShade {
        match &self.state {
            // Caller-supplied scalars say nothing about phase.
            PipeVisualState::Scalars(_) => PipePhaseShade::Liquid,
            PipeVisualState::Physics(pipe) => match &pipe.backend {
                PipeBackend::Lumped(_) => PipePhaseShade::Liquid,
                PipeBackend::Compressible(_) => PipePhaseShade::Gas,
                PipeBackend::SteamHem(_) => PipePhaseShade::TwoPhase,
                PipeBackend::InsulatedPipe(_) => PipePhaseShade::Liquid,
            },
        }
    }

    /// Drawn size in screen points: `(length, thickness)`, as
    /// [`PipeArt::drawn_size`] computes it for [`Self::art`].
    pub fn drawn_size(&self) -> (f32, f32) {
        self.art().drawn_size()
    }

    /// Per-cell fluid temperatures along the run, inlet -> outlet.
    ///
    /// For [`PipeVisualState::Physics`] this is the flow backend's
    /// finite-volume temperature profile; for [`PipeVisualState::Scalars`]
    /// it is the single supplied bulk temperature. Returns an empty vector
    /// if a backend cannot report its profile (a fresh array with no cells,
    /// or a backend error) -- callers render nothing rather than a
    /// fabricated colour in that case.
    pub fn cell_temperatures(&self) -> Vec<ThermodynamicTemperature> {
        match &self.state {
            PipeVisualState::Scalars(s) => vec![s.temperature],
            PipeVisualState::Physics(pipe) => match &pipe.backend {
                PipeBackend::Lumped(array) => array.get_temperature_vector().unwrap_or_default(),
                PipeBackend::Compressible(array) => array
                    .t
                    .internal
                    .iter()
                    .map(|t_k| ThermodynamicTemperature::new::<kelvin>(*t_k))
                    .collect(),
                // Same field shape as the compressible array: both are ports of
                // rhoPimpleFoam over different equations of state, so the
                // temperature field is read identically.
                PipeBackend::SteamHem(array) => array
                    .t
                    .internal
                    .iter()
                    .map(|t_k| ThermodynamicTemperature::new::<kelvin>(*t_k))
                    .collect(),
                // The TUAS pre-built component owns its fluid array; its
                // getters need &mut, so read from a clone rather than forcing
                // every caller of this &self method to hold a mutable pipe.
                PipeBackend::InsulatedPipe(component) => component
                    .clone()
                    .pipe_fluid_array_temperature()
                    .unwrap_or_default(),
            },
        }
    }

    /// The drawing of this run: the pipe's cells, wall, phase and geometry
    /// read off the physics ([`PipeCells`]), or the caller's scalars, with
    /// this visual's screen geometry, scale, alarm and tracers.
    pub fn art(&self) -> PipeArt {
        let base = match &self.state {
            PipeVisualState::Scalars(s) => PipeArt::from_scalars(
                *s,
                self.screen_position,
                self.screen_vector,
                self.min_temp,
                self.max_temp,
            ),
            PipeVisualState::Physics(pipe) => PipeArt::from_cells(
                PipeCells {
                    cell_temperatures: self.cell_temperatures(),
                    wall_temperatures: self.wall_temperatures(),
                    phase_shade: self.phase_shade(),
                    cross_sectional_area: std::f64::consts::FRAC_PI_4 * pipe.diameter * pipe.diameter,
                    run_length: pipe.length,
                    inclination: pipe.inclination,
                },
                self.screen_position,
                self.screen_vector,
                self.min_temp,
                self.max_temp,
            ),
        };
        PipeArt {
            scale: self.scale,
            wall_alarm_temp: self.wall_alarm_temp,
            mark_at: self.mark_at,
            tracer: self.tracer.clone(),
            ..base
        }
    }
}

impl Widget for PipeVisual {
    /// Draws [`PipeVisual::art`]: one box per finite-volume cell, the wall,
    /// then any tracer marks (see [`PipeArt`]).
    fn ui(self, ui: &mut Ui) -> Response {
        self.art().ui(ui)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::length::meter;

    /// A physics-backed pipe of the given bore (mm), length (m) and
    /// inclination (degrees), on the default scale.
    fn physics_pipe(bore_mm: f64, length_m: f64, incline_deg: f64) -> PipeVisual {
        use tampines::components::PipeBackend;
        use tampines::single_phase::{LiquidMaterial, SinglePhaseFluidArray};
        use tuas_boussinesq_solver::boussinesq_thermophysical_properties::SolidMaterial;
        use uom::si::angle::degree;
        use uom::si::f64::{Pressure, Ratio};
        use uom::si::length::millimeter;
        use uom::si::pressure::atmosphere;
        use uom::si::ratio::ratio;

        let diameter = Length::new::<millimeter>(bore_mm);
        let length = Length::new::<meter>(length_m);
        let incline = Angle::new::<degree>(incline_deg);
        let array = SinglePhaseFluidArray::new_cylinder(
            length,
            diameter,
            // 900 K: FLiBe melts around 732 K, and TUAS rejects an initial
            // temperature below its valid range rather than extrapolating the
            // property correlations. Do not lower this.
            ThermodynamicTemperature::new::<kelvin>(900.0),
            Pressure::new::<atmosphere>(1.0),
            SolidMaterial::SteelSS304L,
            LiquidMaterial::FLiBe,
            Ratio::new::<ratio>(0.0),
            4,
            incline,
        );
        let pipe = tampines::components::Pipe::new(
            PipeBackend::Lumped(array),
            diameter,
            length,
            Length::new::<millimeter>(0.045),
            incline,
        );
        PipeVisual::new(
            pipe,
            Pos2::ZERO,
            Vec2::new(100.0, 0.0),
            ThermodynamicTemperature::new::<kelvin>(300.0),
            ThermodynamicTemperature::new::<kelvin>(900.0),
        )
    }

    /// Drawn thickness must be proportional to the flow CROSS-SECTIONAL AREA,
    /// not the diameter — doubling the bore quadruples the area, so the pipe
    /// must draw four times as thick, not twice.
    ///
    /// **Methodology:** build two physics-backed pipes differing only in bore
    /// (50 mm and 100 mm), read `drawn_size()`, and compare the thickness
    /// ratio against the area ratio of 4.
    ///
    /// **Result (2026-08-05):** ratio 4.000, to within 1e-3. Both are above
    /// the minimum-thickness floor, so the floor does not mask the test.
    #[test]
    fn thickness_is_proportional_to_cross_sectional_area() {
        let thick = |bore_mm: f64| -> f32 {
            let v = physics_pipe(bore_mm, 3.0, 0.0);
            v.drawn_size().1
        };
        let (small, large) = (thick(50.0), thick(100.0));
        assert!(
            small > PipeScale::default().min_thickness_points,
            "test would be masked by the thickness floor"
        );
        assert!(
            (large / small - 4.0).abs() < 1e-3,
            "expected 4x thickness for 2x bore, got {}",
            large / small
        );
    }

    /// Drawn length must be proportional to the real run length.
    #[test]
    fn length_is_proportional_to_run_length() {
        let len = |m: f64| physics_pipe(50.0, m, 0.0).drawn_size().0;
        assert!((len(6.0) / len(3.0) - 2.0).abs() < 1e-3);
    }

    /// The physics path hands the pipe's cells to the art unchanged.
    #[test]
    fn the_art_carries_the_pipes_cells_and_draws() {
        let v = physics_pipe(50.0, 3.0, 10.0);
        let art = v.art();
        assert_eq!(art.cell_temperatures(), v.cell_temperatures());
        assert_eq!(art.inclination(), v.inclination());
        assert_eq!(art.wall_temperatures(), None);
        assert_eq!(v.peak_wall_temperature(), None);
        assert_eq!(art.phase_shade(), PipePhaseShade::Liquid);
        assert!(bugis::components::shapes_drawn_by(v) > 0);
    }
}
