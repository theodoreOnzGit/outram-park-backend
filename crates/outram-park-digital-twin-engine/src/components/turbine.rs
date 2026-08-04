//! Visual **steam** turbine.
//!
//! Renders a multi-stage axial turbine as rows of blades whose height grows
//! monotonically from inlet to exhaust — the annulus opens out along the flow,
//! as it does in a real machine where steam expands from short high-pressure
//! blades onto long low-pressure ones. A spinning rotor is drawn on top; the
//! angle is not an animation constant: it is `theta = omega * t`, where
//! `omega` is read from a real torque-balance model, so what you see is the
//! machine's actual shaft speed.
//!
//! ## Where the physics comes from
//!
//! [`TurbineVisualState`] is an enum (not a trait object, per the workspace's
//! mandatory design rules) because the set of state sources is closed and
//! growing:
//!
//! - [`TurbineVisualState::SteamGenerator`] wraps
//!   [`ThreePhaseElectricGeneratorTurbine`] from `tampines-steam-tables`. This
//!   is a **working** lumped model: an explicit torque balance advances rotor
//!   angular velocity, and per-phase EMF, current and total electrical power
//!   are read off it. It is the only variant that can report a real shaft
//!   speed, so it is the only one whose blades genuinely spin.
//! - [`TurbineVisualState::SteamThermo`] wraps
//!   [`tampines::components::Turbine`], which carries the inlet steam state and
//!   an adiabatic efficiency. It supplies the casing colour but **no** rotation
//!   — `Turbine::expand_to` is not implemented yet, so there is no shaft speed
//!   to draw and the rotor is rendered stationary rather than at a fabricated
//!   speed.
//!
//! Both variants are steam turbines. Gas and supercritical-CO2 turbines are
//! future work and deliberately have no variant here yet; the blade artwork
//! itself is working-fluid agnostic (axial machines look alike), so they are
//! expected to add state variants rather than a new widget.
//!
//! ## Simulation time is application-owned
//!
//! Like [`crate::animation::TracerTrain`], the rotor phase depends on elapsed
//! simulation time, and widgets are rebuilt every repaint — a clock owned by
//! the widget would reset to zero each frame and the turbine would never turn.
//! The **application** owns the clock and passes it in via
//! [`TurbineVisual::at_time`].
//!
//! ## Provenance
//!
//! The blade geometry is ported from this crate's own `fhr_sim_v2` example
//! (`app/local_widgets_and_buttons/turbine_widget.rs`), generalised so the
//! rotation angle derives from a physics model instead of being set by the
//! caller.

use crate::color_maps::hot_to_cold_colour_mark_1;
use crate::components::hotness_from_temperature;
use egui::{vec2, Color32, Pos2, Rect, Response, Sense, Stroke, Ui, Vec2, Widget};
use std::f64::consts::PI;
use tampines::components::Turbine;
use tampines_steam_tables::steam_turbine_equations::generator::ThreePhaseElectricGeneratorTurbine;
use uom::si::angle::radian;
use uom::si::f64::{Angle, ThermodynamicTemperature, Time};
use uom::si::ratio::ratio;
use uom::ConstZero;

/// Number of rotor blades drawn per blade row.
const BLADES_PER_ROW: usize = 20;

/// Number of blade rows drawn from inlet to outlet.
///
/// Blade height grows **monotonically** across these rows, which is what makes
/// an axial machine read as an axial machine: steam enters at high pressure
/// against short blades and leaves at low pressure against long ones, so the
/// annulus opens out along the flow.
const BLADE_ROWS: usize = 11;

/// Hub (shaft) radius as a fraction of the widget's half-height. The first
/// blade row starts here rather than at zero, so the inlet stage has real
/// blades rather than degenerating to a point.
const HUB_RADIUS_FRACTION: f32 = 0.18;

/// Stator-bar stroke width as a fraction of the row pitch. Below 0.5 the rows
/// stay visually separated instead of merging into a solid block.
const STATOR_WIDTH_FRACTION: f32 = 0.34;

/// Rotor-blade stroke width as a fraction of the row pitch.
const ROTOR_WIDTH_FRACTION: f32 = 0.09;

/// Rotor-blade half-length as a fraction of the row pitch.
///
/// Must stay well under 0.5 or neighbouring rows' blades overlap and the
/// drawing collapses into solid chevrons instead of readable blade rows.
const ROTOR_HALF_LENGTH_FRACTION: f32 = 0.20;

/// Rotor-blade tilt as a fraction of the row pitch.
///
/// Deliberately keyed to the **pitch**, not to the blade radius: tying it to
/// radius made outer blades grow diagonal slashes that swamped the drawing at
/// wide aspect ratios.
const ROTOR_TILT_FRACTION: f32 = 0.10;

/// Index of the single blade painted white, so rotation direction and speed
/// stay readable when all the others are identical.
const MARKER_BLADE: usize = 10;

/// Where a [`TurbineVisual`] gets the physics it renders.
///
/// Enum dispatch, not a trait object, per the workspace's mandatory "no trait
/// objects" Rust design rule. See the module docs for why each variant exists
/// and what it can and cannot show.
#[derive(Debug, Clone, PartialEq)]
pub enum TurbineVisualState {
    /// Steam turbine coupled to a three-phase synchronous generator. Reports a
    /// real, torque-balance-derived shaft speed, so the rotor spins.
    SteamGenerator(ThreePhaseElectricGeneratorTurbine),
    /// Steam turbine known only by its thermodynamic inlet state. Colours the
    /// casing; cannot report a shaft speed, so the rotor is drawn stationary.
    SteamThermo(Turbine),
}

/// Visual representation of a steam turbine.
///
/// Placement follows the same convention as every other widget in
/// [`crate::components`]: `screen_position` is the on-screen centre and
/// `screen_vector` the box size, so the machine can be positioned absolutely
/// on a schematic canvas. Blade-row radii and the stepped silhouette are
/// derived from that box; the *rotation* is derived from physics.
pub struct TurbineVisual {
    /// The physics state this widget renders.
    pub state: TurbineVisualState,
    /// On-screen centre position.
    pub screen_position: Pos2,
    /// On-screen size of the whole machine, in points.
    pub screen_vector: Vec2,
    /// Elapsed simulation time, owned and advanced by the application.
    ///
    /// Combined with the model's angular velocity to give the rotor phase
    /// `theta = omega * simulation_time`. See the module docs for why this is
    /// not owned by the widget.
    pub simulation_time: Time,
    /// Temperature mapped to `hotness = 0.0` (coldest displayable colour).
    pub min_temp: ThermodynamicTemperature,
    /// Temperature mapped to `hotness = 1.0` (hottest displayable colour).
    pub max_temp: ThermodynamicTemperature,
}

impl TurbineVisual {
    /// Wrap a [`ThreePhaseElectricGeneratorTurbine`] — the variant with a real
    /// shaft speed, so the rotor spins at `omega * t`.
    ///
    /// `min_temp`/`max_temp` bound the casing colour map; a generator-backed
    /// turbine has no steam temperature of its own, so the casing is drawn in
    /// neutral grey and the range is carried only so the two variants stay
    /// interchangeable in a gallery.
    pub fn new_generator(
        generator: ThreePhaseElectricGeneratorTurbine,
        screen_position: Pos2,
        screen_vector: Vec2,
        min_temp: ThermodynamicTemperature,
        max_temp: ThermodynamicTemperature,
    ) -> Self {
        Self {
            state: TurbineVisualState::SteamGenerator(generator),
            screen_position,
            screen_vector,
            simulation_time: Time::ZERO,
            min_temp,
            max_temp,
        }
    }

    /// Wrap a [`tampines::components::Turbine`] — colour only, no rotation
    /// (see the module docs).
    ///
    /// Argument order matches the other visual components, so this is a
    /// drop-in for the former `TurbineVisual::new`.
    pub fn new_thermo(
        physics: Turbine,
        screen_position: Pos2,
        screen_vector: Vec2,
        min_temp: ThermodynamicTemperature,
        max_temp: ThermodynamicTemperature,
    ) -> Self {
        Self {
            state: TurbineVisualState::SteamThermo(physics),
            screen_position,
            screen_vector,
            simulation_time: Time::ZERO,
            min_temp,
            max_temp,
        }
    }

    /// Set the application-owned simulation clock. Builder-style, so it chains
    /// onto either constructor.
    pub fn at_time(mut self, simulation_time: Time) -> Self {
        self.simulation_time = simulation_time;
        self
    }

    /// Current rotor phase angle, `theta = omega * t`.
    ///
    /// Returns zero for [`TurbineVisualState::SteamThermo`], which has no
    /// shaft-speed model — a stationary rotor is the honest rendering of "not
    /// known", and is visibly distinct from a turning one.
    pub fn rotor_angle(&self) -> Angle {
        match &self.state {
            TurbineVisualState::SteamGenerator(g) => (g.get_omega() * self.simulation_time).into(),
            TurbineVisualState::SteamThermo(_) => Angle::ZERO,
        }
    }

    /// Casing colour source: the inlet steam temperature, when the variant
    /// knows one.
    ///
    /// [`TurbineVisualState::SteamGenerator`] is an electromechanical model
    /// with no steam path, so it returns `None` and the casing renders neutral
    /// grey rather than a fabricated temperature colour.
    pub fn casing_temperature(&self) -> Option<ThermodynamicTemperature> {
        match &self.state {
            TurbineVisualState::SteamGenerator(_) => None,
            TurbineVisualState::SteamThermo(t) => Some(t.inlet.get_temperature()),
        }
    }
}

impl Widget for TurbineVisual {
    /// Draws `2 * BLADE_ROWS_PER_SIDE + 1` stator rows of growing radius, then
    /// the rotor blades on top at the current [`TurbineVisual::rotor_angle`].
    ///
    /// Only blades on the far half of their circular path are painted (those
    /// with `sin(theta + phase) > 0`), which is what reads as a rotor turning
    /// behind the stator rather than a flat ring of ticks. Blades are angled
    /// downstream-consistent: downwards on the outlet side, upwards on the
    /// inlet side.
    fn ui(self, ui: &mut Ui) -> Response {
        let rect = Rect::from_center_size(self.screen_position, self.screen_vector);
        let response = ui.allocate_rect(rect, Sense::hover());
        let painter = ui.painter();
        let centre = rect.center();

        // Axial pitch between blade rows, and the annulus that opens out along
        // the flow. Stroke widths are derived from the PITCH, never from the
        // widget width directly, so the drawing survives any aspect ratio.
        let pitch = rect.width() / BLADE_ROWS as f32;
        let tip_radius = 0.5 * rect.height();
        let hub_radius = tip_radius * HUB_RADIUS_FRACTION;

        let casing_colour = match self.casing_temperature() {
            Some(t) => hot_to_cold_colour_mark_1(hotness_from_temperature(
                t,
                self.min_temp,
                self.max_temp,
            )),
            None => Color32::GRAY,
        };
        let stator_stroke = Stroke::new(pitch * STATOR_WIDTH_FRACTION, casing_colour);
        let rotor_stroke = Stroke::new(pitch * ROTOR_WIDTH_FRACTION, Color32::from_gray(35));
        let marker_stroke = Stroke::new(pitch * ROTOR_WIDTH_FRACTION, Color32::WHITE);
        let tilt = pitch * ROTOR_TILT_FRACTION;

        let theta = self.rotor_angle();

        // Blade height at row `i`, growing linearly from hub to tip along the
        // flow direction (inlet on the left, exhaust on the right).
        let radius_at = |i: usize| -> f32 {
            let f = i as f32 / (BLADE_ROWS - 1) as f32;
            hub_radius + (tip_radius - hub_radius) * f
        };
        // Row `i` sits at the centre of its pitch slot.
        let x_at = |i: usize| -> f32 { rect.left() + (i as f32 + 0.5) * pitch };

        for row in 0..BLADE_ROWS {
            let radius = radius_at(row);
            let x = x_at(row);

            // The stator row: a vertical bar spanning the annulus at this
            // station, so the machine visibly opens out towards the exhaust.
            painter.line_segment(
                [
                    Pos2::new(x, centre.y - radius),
                    Pos2::new(x, centre.y + radius),
                ],
                stator_stroke,
            );

            for blade in 0..BLADES_PER_ROW {
                let phase = theta
                    + Angle::new::<radian>(blade as f64 * (2.0 * PI) / (BLADES_PER_ROW as f64));

                // Only the half-revolution facing the viewer is drawn — this
                // is what reads as a rotor turning behind the stator rather
                // than a static ring of ticks.
                if phase.sin() <= <uom::si::f64::Ratio as ConstZero>::ZERO {
                    continue;
                }

                // Blades ride between hub and tip on this row's annulus.
                let span = radius - hub_radius;
                let blade_y = (hub_radius + 0.5 * span) as f64 * phase.cos().get::<ratio>();
                let blade_centre = Pos2 {
                    x,
                    y: centre.y + blade_y as f32,
                };

                // Blades are tilted to shed steam downstream; the sign flips
                // across the shaft so both halves stay consistent with the
                // flow direction.
                let signed_tilt = if blade_y >= 0.0 { tilt } else { -tilt };
                let stroke = if blade == MARKER_BLADE {
                    marker_stroke
                } else {
                    rotor_stroke
                };
                let half_len = pitch * ROTOR_HALF_LENGTH_FRACTION;
                painter.line_segment(
                    [
                        blade_centre - vec2(half_len, signed_tilt),
                        blade_centre + vec2(half_len, signed_tilt),
                    ],
                    stroke,
                );
            }
        }

        // The shaft, drawn last so it reads in front of the blade rows.
        painter.line_segment(
            [
                Pos2::new(rect.left(), centre.y),
                Pos2::new(rect.right(), centre.y),
            ],
            Stroke::new(hub_radius, casing_colour),
        );

        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::angular_velocity::radian_per_second;
    use uom::si::f64::AngularVelocity;
    use uom::si::thermodynamic_temperature::kelvin;
    use uom::si::time::second;

    fn range() -> (ThermodynamicTemperature, ThermodynamicTemperature) {
        (
            ThermodynamicTemperature::new::<kelvin>(300.0),
            ThermodynamicTemperature::new::<kelvin>(600.0),
        )
    }

    fn generator_visual(omega_rad_s: f64) -> TurbineVisual {
        let mut g = ThreePhaseElectricGeneratorTurbine::new_250_megawatt_generator();
        g.set_omega(AngularVelocity::new::<radian_per_second>(omega_rad_s));
        let (min, max) = range();
        TurbineVisual::new_generator(g, Pos2::ZERO, Vec2::new(200.0, 100.0), min, max)
    }

    /// The rotor phase must be the physical product `theta = omega * t`, not a
    /// frame counter — this is the whole point of coupling the widget to the
    /// generator rather than animating it.
    ///
    /// **Methodology:** set the generator's angular velocity to a known
    /// 10.0 rad/s, advance the application clock to 3.0 s, and compare the
    /// reported rotor angle against the analytical 30.0 rad.
    ///
    /// **Result (2026-08-04):** 30.0 rad exactly, to within 1e-9 rad. The
    /// identity is exact in `uom`'s type algebra, so the only error is
    /// floating-point round-off.
    #[test]
    fn rotor_angle_is_omega_times_time() {
        let v = generator_visual(10.0).at_time(Time::new::<second>(3.0));
        assert!((v.rotor_angle().get::<radian>() - 30.0).abs() < 1e-9);
    }

    /// A stationary shaft must render stationary, at exactly zero phase.
    #[test]
    fn zero_angular_velocity_gives_zero_rotor_angle() {
        let v = generator_visual(0.0).at_time(Time::new::<second>(12.5));
        assert_eq!(v.rotor_angle().get::<radian>(), 0.0);
    }

    /// The generator variant has no steam path, so it must report no casing
    /// temperature rather than inventing one. A fabricated colour would make an
    /// electromechanical model look like it knew a steam state.
    #[test]
    fn generator_variant_reports_no_casing_temperature() {
        assert!(generator_visual(5.0).casing_temperature().is_none());
    }

    /// A fresh 250 MW preset starts from rest, so its rotor must not turn until
    /// the model is actually advanced.
    #[test]
    fn fresh_preset_starts_at_rest() {
        let (min, max) = range();
        let g = ThreePhaseElectricGeneratorTurbine::new_250_megawatt_generator();
        let v = TurbineVisual::new_generator(g, Pos2::ZERO, Vec2::new(200.0, 100.0), min, max)
            .at_time(Time::new::<second>(60.0));
        assert_eq!(v.rotor_angle().get::<radian>(), 0.0);
    }
}
