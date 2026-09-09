//! One mean-flow turbine stage: a control volume, a velocity triangle, and a
//! rule for reading work off that triangle (bead `op-yi7m`).
//!
//! # One control volume per stage
//!
//! Each stage owns a [`TampinesSteamTableCV`], so the steam state is carried by
//! the crate's homogeneous-equilibrium flashes rather than by a perfect-gas
//! shortcut. That matters most in the last stages, where the expansion crosses
//! into the dome and the equilibrium quality is what sets the density the
//! triangle is built on.
//!
//! # Every stage is both impulse and reaction
//!
//! No rotor row is purely one or the other. A real row redirects the relative
//! flow *and* accelerates it, so each stage here is solved in two parts, in the
//! order they physically happen:
//!
//! 1. **Impulse first.** The blade turns the relative flow to its exit angle at
//!    constant relative speed, bar passage friction. The tangential momentum
//!    removed is work, by the Euler equation on the velocity triangle. See
//!    [`RotorBlading::impulse_specific_work`].
//! 2. **Reaction after that.** The rotor passage is also a nozzle, because its
//!    exit pressure is below its inlet pressure. That drop accelerates the
//!    relative flow, and the blade row develops lift from the acceleration. See
//!    [`RotorBlading::reaction_specific_work`].
//!
//! The balance between the two is an outcome of the rotor pressure drop, not a
//! label chosen up front. A rotor given no pressure drop reduces exactly to the
//! classical impulse stage, which is what makes that limit testable.
//!
//! The two contributions are reported separately in [`StageWorkSplit`], and the
//! two methods are public so a unit test can exercise either mechanism on its
//! own.

use uom::si::available_energy::joule_per_kilogram;
use uom::si::f64::*;
use uom::si::ratio::ratio;
use uom::si::velocity::meter_per_second;

use crate::interfaces::object_oriented_programming::TampinesSteamTableCV;

use super::velocity_triangle::VelocityTriangle;

/// Rotor blading, carrying both the impulse and the reaction parameters.
///
/// There is no "impulse blade" or "reaction blade" type here, deliberately. A
/// real rotor row is never purely one or the other: it always redirects the
/// relative flow *and* accelerates it, so every stage built from this struct
/// gets both contributions. What varies along a machine is the balance, and
/// that balance is an outcome of the rotor pressure drop rather than a label.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RotorBlading {
    /// `|w2| / |w1|` for the redirection alone, capturing passage friction.
    /// Unity is the loss-free blade. This drives the **impulse** part.
    pub blade_velocity_coefficient: Ratio,
    /// Cascade lift coefficient `C_L`, referred to the mean relative velocity.
    /// This drives the **reaction** part.
    pub lift_coefficient: Ratio,
    /// Cascade drag coefficient `C_D`. Zero for the loss-free case.
    pub drag_coefficient: Ratio,
    /// Solidity, chord over pitch, `c / s`.
    pub solidity: Ratio,
}

/// How one stage's work divides between its two mechanisms.
///
/// Reported rather than summed away, because the split is the thing a
/// stage-resolved model can say and a lumped one cannot.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StageWorkSplit {
    /// Work from redirecting the relative flow, by the Euler equation on the
    /// velocity triangle.
    pub impulse: AvailableEnergy,
    /// Work from the rotor pressure drop, by the cascade lift equation.
    pub reaction: AvailableEnergy,
}

impl StageWorkSplit {
    /// Total specific work of the stage.
    pub fn total(&self) -> AvailableEnergy {
        self.impulse + self.reaction
    }

    /// The fraction of stage work that came from the reaction part.
    ///
    /// Zero when the rotor sees no pressure drop. Returns zero rather than a
    /// NaN when the stage did no work at all.
    pub fn reaction_fraction(&self) -> Ratio {
        let total = self.total().get::<joule_per_kilogram>();

        if total.abs() <= f64::EPSILON {
            return Ratio::new::<ratio>(0.0);
        }

        self.reaction / self.total()
    }
}

impl RotorBlading {
    /// Work from the **impulse** part: the rotor as a deflector.
    ///
    /// `w = U (c_theta1 - c_theta2)`, straight off
    /// [`VelocityTriangle::euler_specific_work`]. This counts only the
    /// tangential momentum removed by turning the flow, with the relative speed
    /// changed by nothing but passage friction.
    pub fn impulse_specific_work(&self, triangle: &VelocityTriangle) -> AvailableEnergy {
        triangle.euler_specific_work()
    }

    /// Work from the **reaction** part: the rotor as an aerofoil cascade.
    ///
    /// The rotor passage also acts as a nozzle, because its exit pressure is
    /// below its inlet pressure. In the relative frame that pressure drop
    /// accelerates the flow from the redirected speed `|w2|` to
    ///
    /// ```text
    /// w2_accelerated = sqrt(w2^2 + 2 dh_rotor)
    /// ```
    ///
    /// and it is that acceleration the blade develops lift from. Per unit span
    /// a blade at the mean relative angle `beta_m` sees lift
    /// `L = 0.5 rho w_m dw c C_L` and drag `D = 0.5 rho w_m dw c C_D` from the
    /// increment `dw`, resolving onto the tangential direction as
    /// `F_theta = L cos(beta_m) + D sin(beta_m)`. With the passage mass flow
    /// per unit span `rho s c_x`, the specific work is
    ///
    /// ```text
    /// w = U dw w_m (c/s) (C_L cos(beta_m) + C_D sin(beta_m)) / (2 c_x)
    /// ```
    ///
    /// Density cancels, as it does for the impulse part.
    ///
    /// # Why this does not double count
    ///
    /// The lift here is driven by the velocity **increment** `dw`, not by the
    /// full relative velocity. The momentum the rotor took out by turning the
    /// flow is already counted in
    /// [`Self::impulse_specific_work`], and this term adds only what the rotor
    /// pressure drop contributed on top. A rotor with no pressure drop has
    /// `dh_rotor = 0`, hence `dw = 0`, hence no reaction work at all, which is
    /// the correct limit.
    ///
    /// The drag term is signed by `beta_m`: drag acts along the mean relative
    /// velocity, so it adds to the tangential force while that velocity still
    /// points along blade motion and subtracts once the rotor has turned the
    /// flow past axial.
    pub fn reaction_specific_work(
        &self,
        triangle: &VelocityTriangle,
        rotor_enthalpy_drop: AvailableEnergy,
    ) -> AvailableEnergy {
        // No through-flow means no passage mass flow for a cascade force to act
        // on, and the specific-work expression divides by the axial velocity.
        // A stage handed a rising pressure lands here, and must return zero
        // rather than a NaN that would poison the rest of the machine.
        if triangle.get_axial_velocity() <= Velocity::new::<meter_per_second>(0.0) {
            return AvailableEnergy::new::<joule_per_kilogram>(0.0);
        }

        let relative_speed_out = triangle.get_relative_speed_out();
        let accelerated = Self::accelerated_relative_speed(relative_speed_out, rotor_enthalpy_drop);
        let speed_increment: Velocity = accelerated - relative_speed_out;

        let mean_angle = triangle.get_mean_relative_angle();
        let mean_relative_speed = triangle.get_mean_relative_speed();

        let tangential_coefficient: Ratio =
            self.lift_coefficient * mean_angle.cos() + self.drag_coefficient * mean_angle.sin();

        let force_term: Ratio = self.solidity * tangential_coefficient * 0.5;

        let speed_term: Velocity =
            speed_increment * mean_relative_speed / triangle.get_axial_velocity() * force_term;

        triangle.get_blade_speed() * speed_term
    }

    /// Both parts of the stage work, in the order they physically happen:
    /// the flow is turned first, then accelerated by the rotor pressure drop.
    pub fn specific_work_split(
        &self,
        triangle: &VelocityTriangle,
        rotor_enthalpy_drop: AvailableEnergy,
    ) -> StageWorkSplit {
        StageWorkSplit {
            impulse: self.impulse_specific_work(triangle),
            reaction: self.reaction_specific_work(triangle, rotor_enthalpy_drop),
        }
    }

    /// `sqrt(w^2 + 2 dh)`, the relative speed after the rotor pressure drop.
    ///
    /// A non-positive drop leaves the speed unchanged, so a rotor with no
    /// pressure drop reduces cleanly to the pure-deflector case.
    fn accelerated_relative_speed(
        relative_speed: Velocity,
        rotor_enthalpy_drop: AvailableEnergy,
    ) -> Velocity {
        let drop_value = rotor_enthalpy_drop.get::<joule_per_kilogram>();

        if drop_value <= 0.0 {
            return relative_speed;
        }

        let speed_value = relative_speed.get::<meter_per_second>();

        Velocity::new::<meter_per_second>((speed_value * speed_value + 2.0 * drop_value).sqrt())
    }
}

/// Mean-line geometry of one stage.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StageGeometry {
    /// Mean blade radius, which with shaft speed sets `U`.
    pub mean_radius: Length,
    /// Stator exit angle `alpha1` from axial.
    pub nozzle_angle: Angle,
    /// Rotor blade exit angle `beta2` from axial. Negative turns the flow back
    /// against blade motion, which is what extracts work.
    pub relative_exit_angle: Angle,
    /// Fraction of the ideal nozzle exit speed actually achieved, capturing
    /// stator loss. Unity is the loss-free nozzle.
    pub nozzle_velocity_coefficient: Ratio,
}

/// One stage: geometry, blading, and the two pressures that bracket it.
///
/// The interstage and exit pressures are supplied rather than derived. That is
/// a deliberate choice: splitting an enthalpy drop by an assumed degree of
/// reaction would need an `(h,s)` flash, whose known gaps at the triple point
/// and at 1000 bar are documented in this crate's `CLAUDE.md`. Supplying
/// pressures keeps the stage on the `(p,s)` and `(p,h)` paths, which are the
/// validated ones. The degree of reaction is then *reported* by
/// [`VelocityTriangle::get_degree_of_reaction`] rather than prescribed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TurbineStage {
    /// Mean-line geometry.
    pub geometry: StageGeometry,
    /// Rotor blading, carrying both the impulse and the reaction parameters.
    pub blading: RotorBlading,
    /// Pressure at stator exit / rotor inlet.
    pub stator_exit_pressure: Pressure,
    /// Pressure at rotor exit, which is the next stage's inlet pressure.
    pub rotor_exit_pressure: Pressure,
}

/// What one stage did: the outlet state, the kinematics it ran at, and the
/// work it produced against the work it could ideally have produced.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StageOutcome {
    /// Steam state leaving the rotor.
    pub outlet: TampinesSteamTableCV,
    /// The mean-radius triangle this stage ran at.
    pub triangle: VelocityTriangle,
    /// How the extracted work divided between the two mechanisms.
    pub work_split: StageWorkSplit,
    /// Work per unit mass an isentropic expansion over the same pressure range
    /// would have given.
    pub isentropic_specific_work: AvailableEnergy,
}

impl StageOutcome {
    /// Total specific work of the stage, both mechanisms summed.
    pub fn specific_work(&self) -> AvailableEnergy {
        self.work_split.total()
    }

    /// Stage efficiency, actual work over isentropic work.
    ///
    /// Returns zero when the isentropic drop is not positive, which happens
    /// only if the stage was handed a rising pressure.
    pub fn stage_efficiency(&self) -> Ratio {
        let ideal = self.isentropic_specific_work.get::<joule_per_kilogram>();

        if ideal <= 0.0 {
            return Ratio::new::<ratio>(0.0);
        }

        self.specific_work() / self.isentropic_specific_work
    }
}

impl TurbineStage {
    /// Expands `inlet` through this stage at the given shaft speed.
    ///
    /// The sequence is:
    ///
    /// 1. The stator expands isentropically from the inlet pressure to
    ///    [`Self::stator_exit_pressure`]. The enthalpy drop becomes nozzle exit
    ///    speed, `c1 = phi sqrt(2 dh)`, with `phi` the nozzle velocity
    ///    coefficient.
    /// 2. The velocity triangle is built from `c1`, the blade speed and the
    ///    blade angles.
    /// 3. [`RotorBlading::specific_work_split`] reads both contributions off
    ///    that triangle: the impulse part from turning the flow, then the
    ///    reaction part from the rotor pressure drop.
    /// 4. The control volume is advanced to [`Self::rotor_exit_pressure`] with
    ///    that work removed, through the `(p,h)` flash.
    pub fn expand(
        &self,
        inlet: TampinesSteamTableCV,
        shaft_speed: AngularVelocity,
    ) -> StageOutcome {
        let blade_speed: Velocity = shaft_speed * self.geometry.mean_radius;

        // 1. stator: isentropic drop to the interstage pressure, turned into
        //    nozzle exit velocity.
        let mut stator_exit = inlet;
        stator_exit.expand_isentropically(self.stator_exit_pressure);

        let nozzle_enthalpy_drop: AvailableEnergy =
            inlet.get_specific_enthalpy() - stator_exit.get_specific_enthalpy();

        let absolute_velocity_in = Self::velocity_from_enthalpy_drop(nozzle_enthalpy_drop)
            * self.geometry.nozzle_velocity_coefficient;

        // 2. the triangle.
        let triangle = VelocityTriangle::new_from_nozzle_and_blade_angles(
            blade_speed,
            absolute_velocity_in,
            self.geometry.nozzle_angle,
            self.geometry.relative_exit_angle,
            self.blading.blade_velocity_coefficient,
        );

        // 3. both parts of the work: the flow is turned first, then
        //    accelerated by whatever pressure drop the rotor itself takes.
        let rotor_enthalpy_drop =
            Self::rotor_isentropic_enthalpy_drop(stator_exit, self.rotor_exit_pressure);
        let work_split = self
            .blading
            .specific_work_split(&triangle, rotor_enthalpy_drop);
        let specific_work = work_split.total();

        // 4. advance the control volume with that work removed.
        let mut outlet = inlet;
        let total_work: Energy = specific_work * inlet.get_mass();
        outlet.expand_with_work_extensive(total_work, self.rotor_exit_pressure);

        // the ideal yardstick, over the whole stage pressure range.
        let mut isentropic_outlet = inlet;
        isentropic_outlet.expand_isentropically(self.rotor_exit_pressure);
        let isentropic_specific_work: AvailableEnergy =
            inlet.get_specific_enthalpy() - isentropic_outlet.get_specific_enthalpy();

        StageOutcome {
            outlet,
            triangle,
            work_split,
            isentropic_specific_work,
        }
    }

    /// The isentropic enthalpy drop the rotor itself takes, from stator exit
    /// pressure down to rotor exit pressure.
    ///
    /// This is what makes a stage partly reaction. It is zero when the two
    /// pressures are equal, which is the classical impulse rotor.
    fn rotor_isentropic_enthalpy_drop(
        stator_exit: TampinesSteamTableCV,
        rotor_exit_pressure: Pressure,
    ) -> AvailableEnergy {
        let mut rotor_exit = stator_exit;
        rotor_exit.expand_isentropically(rotor_exit_pressure);

        stator_exit.get_specific_enthalpy() - rotor_exit.get_specific_enthalpy()
    }

    /// `c = sqrt(2 dh)`, the speed an enthalpy drop converts into.
    ///
    /// A non-positive drop yields zero rather than a NaN, so a stage handed a
    /// rising pressure degrades to no nozzle velocity instead of poisoning the
    /// whole chain.
    fn velocity_from_enthalpy_drop(enthalpy_drop: AvailableEnergy) -> Velocity {
        let drop_value = enthalpy_drop.get::<joule_per_kilogram>();

        if drop_value <= 0.0 {
            return Velocity::new::<meter_per_second>(0.0);
        }

        Velocity::new::<meter_per_second>((2.0 * drop_value).sqrt())
    }
}
