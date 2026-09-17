//! Flow-mode dispatch — the enum that stands in for PFLOTRAN's process-model
//! (`pm_*`) polymorphism.
//!
//! PFLOTRAN selects a *mode* (RICHARDS, TH, GENERAL, ...) that defines the
//! governing equations, primary unknowns, and residual/Jacobian assembly.
//! Upstream this is C-style polymorphism over a base process-model type; here it
//! is a closed [`FlowMode`] enum matched exhaustively, per the workspace
//! "no trait objects — use enums for dispatch" rule. Adding a mode is then a
//! compile error at every `match` until it is handled.
//!
//! v1 implements the [`richards`] mode end-to-end: a [`FlowMode::Richards`] owns
//! a running [`RichardsSimulation`] and [`FlowMode::step`]/[`FlowMode::run`]
//! advance it in time.
//!
//! > **Untrusted AI draft, verification-only.** The RICHARDS physics has been
//! > verified (analytical steady state + method of manufactured solutions), not
//! > validated against published PFLOTRAN reference cases. See the crate README
//! > "Bookkeeping status" and `docs/ai-directed-decisions.md`.

pub mod richards;

pub use richards::{
    BoundaryCondition, BoundaryConditionKind, RichardsProblem, RichardsSimulation, RunReport,
    StepReport, TimeControls,
};

/// A closed set of subsurface flow modes.
///
/// v1 implements [`FlowMode::Richards`] (variably-saturated single-phase flow).
/// TH, GENERAL (multiphase), and transport modes are added as later variants
/// (beads op-v6s.10, op-v6s.13, op-v6s.11); the `#[non_exhaustive]` marker keeps
/// downstream `match`es forward-compatible.
#[non_exhaustive]
pub enum FlowMode {
    /// Variably-saturated single-phase groundwater flow (Richards' equation),
    /// carrying its live time-stepping simulation.
    Richards(RichardsSimulation),
}

impl FlowMode {
    /// Human-readable name of the active flow mode (e.g. `"RICHARDS"`).
    pub fn name(&self) -> &'static str {
        match self {
            FlowMode::Richards(_) => "RICHARDS",
        }
    }

    /// Advance the flow solution by one (adaptive) timestep, returning the
    /// per-step report. Errors if the nonlinear solve fails to converge even
    /// after timestep cutting (see [`RichardsSimulation::step`]).
    pub fn step(&mut self) -> crate::Result<StepReport> {
        match self {
            FlowMode::Richards(sim) => sim.step(),
        }
    }

    /// Run the simulation to its configured final time, returning the run
    /// summary. Written as an exhaustive `match` so a new [`FlowMode`] variant
    /// forces this method to handle it.
    pub fn run(&mut self) -> crate::Result<RunReport> {
        match self {
            FlowMode::Richards(sim) => sim.run(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::parse_deck;

    const DECK: &str = "\
GRID
  STRUCTURED 5 1 1
  DXYZ 0.2 1.0 1.0
END
MATERIAL
  POROSITY 0.35
  PERMEABILITY 1.0e-12
END
CHARACTERISTIC_CURVES
  MODEL VAN_GENUCHTEN
  ALPHA 1.0e-4
  N 2.0
  RESIDUAL_SATURATION 0.1
END
INITIAL_PRESSURE 90000.0
BOUNDARY_CONDITION
  LOCATION XMIN
  DIRICHLET_PRESSURE 101325.0
END
TIME
  FINAL_TIME 3600.0
  INITIAL_DT 60.0
  MAX_DT 600.0
END
";

    #[test]
    fn flow_mode_reports_its_name() {
        let deck = parse_deck(DECK).unwrap();
        let sim = RichardsSimulation::from_input_deck(&deck).unwrap();
        let mode = FlowMode::Richards(sim);
        assert_eq!(mode.name(), "RICHARDS");
    }

    #[test]
    fn flow_mode_step_advances_time() {
        let deck = parse_deck(DECK).unwrap();
        let sim = RichardsSimulation::from_input_deck(&deck).unwrap();
        let mut mode = FlowMode::Richards(sim);
        let report = mode.step().expect("first Richards step should converge");
        assert!(report.converged);
        assert!(report.time > 0.0);
    }
}
