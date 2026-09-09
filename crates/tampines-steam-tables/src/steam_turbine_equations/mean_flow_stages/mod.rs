//! Mean-flow, stage-by-stage steam turbine model (bead `op-yi7m`).
//!
//! A multistage axial steam turbine solved one stage at a time along the mean
//! streamline, with **one homogeneous-equilibrium control volume per stage**
//! and a real velocity triangle at each one.
//!
//! # Why this exists
//!
//! Before this module the crate's only turbine physics was
//! [`super::converging_diverging_nozzles`] for the nozzle and
//! [`super::generator`] for the shaft and electrical side. Both are lumped:
//! nothing resolved a machine into stages, so nothing could say where along the
//! expansion the work came from, or where the steam went wet.
//!
//! # The model
//!
//! Each [`TurbineStage`] carries a [`TampinesSteamTableCV`] and is bracketed by
//! two pressures. Within a stage:
//!
//! 1. the stator expands the steam isentropically to the interstage pressure,
//!    and the enthalpy drop becomes nozzle exit velocity;
//! 2. a [`VelocityTriangle`] is built at the mean radius from that velocity,
//!    the blade speed and the blade angles;
//! 3. work is read off the triangle by the stage's own mechanism;
//! 4. the control volume is advanced to the stage exit pressure with that work
//!    removed, through the `(p,h)` equilibrium flash.
//!
//! Stages chain outlet to inlet, so the wet-steam behaviour of the last stages
//! falls out of the equilibrium flashes rather than being assumed.
//!
//! # Impulse and reaction take different routes
//!
//! This is the substance of the model, not a naming convention:
//!
//! - the **impulse** part comes from the velocity triangle, by the Euler
//!   equation: momentum removed from the steam by turning it;
//! - the **reaction** part comes from a simplified cascade lift equation, with
//!   the blade row treated as an aerofoil row developing lift from the
//!   acceleration the rotor pressure drop produces.
//!
//! No stage is purely one or the other. Both act in every stage, in that order,
//! and [`StageWorkSplit`] reports how the work divided. See
//! [`RotorBlading::impulse_specific_work`] and
//! [`RotorBlading::reaction_specific_work`] for the derivations.
//!
//! # What this model does not do
//!
//! - It is a **mean-line** model. There is no radial equilibrium, no spanwise
//!   variation, and no tip leakage.
//! - The equilibrium assumption is inherited from the flashes, so
//!   supersaturation and droplet lag in the wet stages are outside it, exactly
//!   as they are for the rest of this crate's HEM work.
//! - Stage pressures are supplied, not solved from a mass-flow match.

/// One stage: control volume, blading, and the two mechanisms for extracting
/// work from it.
pub mod stage;
pub use stage::*;

/// Mean-radius velocity triangles.
pub mod velocity_triangle;
pub use velocity_triangle::*;

#[cfg(test)]
mod tests;

use uom::si::available_energy::joule_per_kilogram;
use uom::si::f64::*;

use crate::interfaces::object_oriented_programming::TampinesSteamTableCV;

/// A multistage axial steam turbine, solved along the mean streamline.
///
/// The stage list is ordered from admission to exhaust. Nothing here checks
/// that the stage pressures descend monotonically; a stage handed a rising
/// pressure produces no nozzle velocity and therefore no work, which shows up
/// in the outcome rather than as a panic.
#[derive(Debug, Clone, PartialEq)]
pub struct MeanFlowTurbine {
    /// Stages in flow order, admission first.
    pub stages: Vec<TurbineStage>,
    /// Shaft speed, shared by every stage.
    pub shaft_speed: AngularVelocity,
}

/// What a whole machine did, stage by stage.
#[derive(Debug, Clone, PartialEq)]
pub struct TurbineOutcome {
    /// One entry per stage, in flow order.
    pub stage_outcomes: Vec<StageOutcome>,
    /// Steam state at the exhaust.
    pub outlet: TampinesSteamTableCV,
}

impl TurbineOutcome {
    /// Total specific work of the machine, summed over stages.
    pub fn total_specific_work(&self) -> AvailableEnergy {
        self.stage_outcomes.iter().fold(
            AvailableEnergy::new::<joule_per_kilogram>(0.0),
            |sum, outcome| sum + outcome.specific_work(),
        )
    }

    /// Total isentropic specific work, summed stage by stage.
    ///
    /// This is the sum of per-stage isentropic drops, not the isentropic drop
    /// of the whole machine. The two differ by the reheat factor, which is
    /// exactly the quantity a stage-resolved model is able to expose and a
    /// lumped one is not.
    pub fn total_isentropic_specific_work(&self) -> AvailableEnergy {
        self.stage_outcomes.iter().fold(
            AvailableEnergy::new::<joule_per_kilogram>(0.0),
            |sum, outcome| sum + outcome.isentropic_specific_work,
        )
    }
}

impl MeanFlowTurbine {
    /// Builds a turbine from stages in flow order and a shaft speed.
    pub fn new(stages: Vec<TurbineStage>, shaft_speed: AngularVelocity) -> Self {
        Self {
            stages,
            shaft_speed,
        }
    }

    /// Expands `inlet` through every stage in turn, each stage's outlet feeding
    /// the next stage's inlet.
    pub fn expand(&self, inlet: TampinesSteamTableCV) -> TurbineOutcome {
        let mut current = inlet;
        let mut stage_outcomes = Vec::with_capacity(self.stages.len());

        for stage in self.stages.iter() {
            let outcome = stage.expand(current, self.shaft_speed);
            current = outcome.outlet;
            stage_outcomes.push(outcome);
        }

        TurbineOutcome {
            stage_outcomes,
            outlet: current,
        }
    }
}
