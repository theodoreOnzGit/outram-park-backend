//! Mean-radius velocity triangle for one axial turbine stage.
//!
//! # Sign and angle convention
//!
//! Angles are measured **from the axial direction**, positive in the direction
//! the blades move. Axial velocity is taken as constant across the rotor, which
//! is the usual mean-line assumption and the reason this is a *mean-flow*
//! model rather than a through-flow one.
//!
//! The stations are the classical three:
//!
//! - **0** stage inlet (stator inlet),
//! - **1** stator exit / rotor inlet,
//! - **2** rotor exit.
//!
//! Only stations 1 and 2 carry a triangle; station 0 supplies the
//! thermodynamic state that the stator expands.
//!
//! # What the triangle is for
//!
//! It converts a thermodynamic state change into the kinematics of the flow,
//! so that the work leaving the stage is a consequence of how much the steam
//! is *turned*, not of an assumed stage efficiency. For an impulse stage this
//! is the whole story, and [`VelocityTriangle::euler_specific_work`] is the
//! answer. A reaction stage uses the same triangle but reads work off a blade
//! lift coefficient instead — see [`super::stage::StageKind`].

use uom::si::angle::radian;
use uom::si::f64::*;
use uom::si::velocity::meter_per_second;

/// The mean-radius velocity triangle of one stage, at rotor inlet (station 1)
/// and rotor exit (station 2).
///
/// Every field is a resolved component rather than a magnitude-and-angle pair,
/// because the work relations want components and re-deriving them at each use
/// invites sign mistakes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VelocityTriangle {
    /// Blade speed at the mean radius, `U = omega * r_mean`.
    blade_speed: Velocity,
    /// Axial velocity, held constant across the rotor.
    axial_velocity: Velocity,
    /// Tangential component of the absolute velocity at rotor inlet.
    absolute_tangential_in: Velocity,
    /// Tangential component of the absolute velocity at rotor exit.
    absolute_tangential_out: Velocity,
    /// Tangential component of the relative velocity at rotor inlet.
    relative_tangential_in: Velocity,
    /// Tangential component of the relative velocity at rotor exit.
    relative_tangential_out: Velocity,
}

impl VelocityTriangle {
    /// Builds the triangle from the stator exit kinematics and the rotor blade
    /// exit angle.
    ///
    /// # Arguments
    ///
    /// * `blade_speed` — `U` at the mean radius.
    /// * `absolute_velocity_in` — `c1`, the speed leaving the stator.
    /// * `nozzle_angle` — `alpha1`, the stator exit angle from axial. Steam
    ///   turbine nozzles are strongly tangential, so this is typically 65 to 75
    ///   degrees.
    /// * `relative_exit_angle` — `beta2`, the rotor blade exit angle from
    ///   axial. Negative values turn the flow back against blade motion, which
    ///   is what extracts work; a symmetric impulse blade has
    ///   `beta2 = -beta1`.
    /// * `blade_velocity_coefficient` — the ratio `|w2| / |w1|`, capturing
    ///   friction in the rotor passage. Unity is the loss-free blade.
    ///
    /// Axial velocity is set by `c1` and `alpha1` and then held fixed, so the
    /// rotor exit relative velocity follows from `beta2` and that axial
    /// component rather than from the coefficient alone. The coefficient
    /// scales the resulting relative speed.
    pub fn new_from_nozzle_and_blade_angles(
        blade_speed: Velocity,
        absolute_velocity_in: Velocity,
        nozzle_angle: Angle,
        relative_exit_angle: Angle,
        blade_velocity_coefficient: Ratio,
    ) -> Self {
        let axial_velocity: Velocity = absolute_velocity_in * nozzle_angle.cos();
        let absolute_tangential_in: Velocity = absolute_velocity_in * nozzle_angle.sin();
        let relative_tangential_in: Velocity = absolute_tangential_in - blade_speed;

        // The rotor turns the relative flow to its own exit angle. The axial
        // component is unchanged by the mean-line assumption, so the exit
        // relative tangential component is fixed by beta2, and the velocity
        // coefficient then scales the whole relative vector for passage loss.
        let relative_tangential_ideal: Velocity = axial_velocity * relative_exit_angle.tan();
        let relative_tangential_out: Velocity =
            relative_tangential_ideal * blade_velocity_coefficient;

        let absolute_tangential_out: Velocity = relative_tangential_out + blade_speed;

        Self {
            blade_speed,
            axial_velocity,
            absolute_tangential_in,
            absolute_tangential_out,
            relative_tangential_in,
            relative_tangential_out,
        }
    }

    /// The tangential velocity the rotor removed from the steam,
    /// `c_theta1 - c_theta2`.
    ///
    /// This is the quantity torque is built from, and it is deliberately kept
    /// separate from the work. Specific work is this times the blade speed, so
    /// at standstill the work vanishes while this does not. That is the
    /// physical statement that a turbine develops **starting torque at zero
    /// speed**, and it is why a machine coupled to a shaft can spin up at all.
    /// Recovering torque by dividing a power by the shaft speed would instead
    /// give `0/0` exactly where the spin-up starts.
    pub fn tangential_velocity_change(&self) -> Velocity {
        self.absolute_tangential_in - self.absolute_tangential_out
    }

    /// Specific work by the Euler turbomachinery equation,
    /// `w = U * (c_theta1 - c_theta2)`.
    ///
    /// This is the momentum route to work: it counts only how much tangential
    /// momentum the rotor removed from the steam. It is exact for any axial
    /// stage, and it is the route the **impulse** part of a stage uses.
    pub fn euler_specific_work(&self) -> AvailableEnergy {
        self.blade_speed * self.tangential_velocity_change()
    }

    /// Blade speed at the mean radius.
    pub fn get_blade_speed(&self) -> Velocity {
        self.blade_speed
    }

    /// Axial velocity, constant across the rotor by assumption.
    pub fn get_axial_velocity(&self) -> Velocity {
        self.axial_velocity
    }

    /// Tangential component of the absolute velocity at rotor inlet.
    pub fn get_absolute_tangential_in(&self) -> Velocity {
        self.absolute_tangential_in
    }

    /// Tangential component of the absolute velocity at rotor exit.
    pub fn get_absolute_tangential_out(&self) -> Velocity {
        self.absolute_tangential_out
    }

    /// Tangential component of the relative velocity at rotor inlet.
    pub fn get_relative_tangential_in(&self) -> Velocity {
        self.relative_tangential_in
    }

    /// Tangential component of the relative velocity at rotor exit.
    pub fn get_relative_tangential_out(&self) -> Velocity {
        self.relative_tangential_out
    }

    /// Relative flow angle at rotor inlet, `beta1`, from axial.
    pub fn get_relative_angle_in(&self) -> Angle {
        Self::angle_from_axial(self.relative_tangential_in, self.axial_velocity)
    }

    /// Relative flow angle at rotor exit, `beta2`, from axial.
    pub fn get_relative_angle_out(&self) -> Angle {
        Self::angle_from_axial(self.relative_tangential_out, self.axial_velocity)
    }

    /// Relative speed at rotor inlet, `|w1|`.
    pub fn get_relative_speed_in(&self) -> Velocity {
        Self::magnitude(self.relative_tangential_in, self.axial_velocity)
    }

    /// Relative speed at rotor exit, `|w2|`.
    pub fn get_relative_speed_out(&self) -> Velocity {
        Self::magnitude(self.relative_tangential_out, self.axial_velocity)
    }

    /// Absolute speed at rotor exit, `|c2|`. The kinetic energy in this is the
    /// stage leaving loss unless the next stage recovers it.
    pub fn get_absolute_speed_out(&self) -> Velocity {
        Self::magnitude(self.absolute_tangential_out, self.axial_velocity)
    }

    /// The **mean relative flow angle** of the rotor cascade, defined by
    /// `tan(beta_m) = (tan(beta1) + tan(beta2)) / 2`.
    ///
    /// This is the angle a cascade lift coefficient is referred to, so it is
    /// what the reaction path in [`super::stage`] needs. It is a vector-mean
    /// direction, not the arithmetic mean of the two angles.
    pub fn get_mean_relative_angle(&self) -> Angle {
        let mean_tangential: Velocity =
            (self.relative_tangential_in + self.relative_tangential_out) * 0.5;

        Self::angle_from_axial(mean_tangential, self.axial_velocity)
    }

    /// The **mean relative speed**, `w_m = c_x / cos(beta_m)`, the velocity a
    /// cascade lift force is evaluated at.
    pub fn get_mean_relative_speed(&self) -> Velocity {
        let mean_angle = self.get_mean_relative_angle();

        self.axial_velocity / mean_angle.cos()
    }

    /// Degree of reaction inferred from the kinematics,
    /// `R = 1 - (c_theta1 + c_theta2) / (2 U)`.
    ///
    /// This is the kinematic definition, valid when the axial velocity is
    /// constant. It is reported rather than prescribed, so a stage built from
    /// blade angles can be checked against the impulse or reaction label it was
    /// given.
    pub fn get_degree_of_reaction(&self) -> Ratio {
        let swirl_sum: Velocity = self.absolute_tangential_in + self.absolute_tangential_out;

        Ratio::new::<uom::si::ratio::ratio>(1.0) - swirl_sum / (self.blade_speed * 2.0)
    }

    fn angle_from_axial(tangential: Velocity, axial: Velocity) -> Angle {
        let tangential_value = tangential.get::<meter_per_second>();
        let axial_value = axial.get::<meter_per_second>();

        Angle::new::<radian>(tangential_value.atan2(axial_value))
    }

    fn magnitude(tangential: Velocity, axial: Velocity) -> Velocity {
        let tangential_value = tangential.get::<meter_per_second>();
        let axial_value = axial.get::<meter_per_second>();

        Velocity::new::<meter_per_second>(tangential_value.hypot(axial_value))
    }
}
