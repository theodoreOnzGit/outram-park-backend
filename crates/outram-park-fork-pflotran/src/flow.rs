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
//! > **Scaffold.** Only the RICHARDS variant exists, and its solve is not
//! > implemented — [`FlowMode::step`] returns
//! > [`PflotranError::NotImplemented`]. The residual/Jacobian assembly and the
//! > Newton-Krylov drive land with beads op-v6s.8 (RICHARDS) and op-v6s.4
//! > (solver).

use crate::error::PflotranError;

/// A closed set of subsurface flow modes.
///
/// v1 targets [`RichardsMode`] only; [`FlowMode`] exists now so the
/// enum-dispatch architecture is fixed before the solver is written. TH,
/// GENERAL (multiphase), and transport modes are added as later variants
/// (beads op-v6s.10, op-v6s.13, op-v6s.11).
#[non_exhaustive]
pub enum FlowMode {
    /// Variably-saturated single-phase groundwater flow (Richards' equation).
    Richards(RichardsMode),
}

impl FlowMode {
    /// Human-readable name of the active flow mode.
    pub fn name(&self) -> &'static str {
        match self {
            FlowMode::Richards(_) => "RICHARDS",
        }
    }

    /// Advance the flow solution by one timestep.
    ///
    /// # Scaffold
    ///
    /// Not implemented yet — returns [`PflotranError::NotImplemented`]. The real
    /// implementation assembles the mode's residual and Jacobian over the grid
    /// and hands them to the Newton-Krylov solver (beads op-v6s.8, op-v6s.4).
    /// It is written as an exhaustive `match` so a new [`FlowMode`] variant
    /// forces this method to handle it.
    pub fn step(&mut self) -> crate::Result<()> {
        match self {
            FlowMode::Richards(_) => Err(PflotranError::NotImplemented("RICHARDS flow-mode solve")),
        }
    }
}

/// RICHARDS flow mode — variably-saturated single-phase groundwater flow.
///
/// Governing equation (mass conservation of the liquid phase):
///
/// $$ \frac{\partial}{\partial t}\left(\phi\, S_l\, \rho_l\right) + \nabla \cdot \left(\rho_l\, \mathbf{q}_l\right) = Q_l $$
///
/// with the Darcy flux
///
/// $$ \mathbf{q}_l = -\frac{k\, k_{rl}}{\mu_l}\left(\nabla p_l - \rho_l\, \mathbf{g}\right) $$
///
/// where `phi` is porosity, `S_l` liquid saturation, `rho_l` liquid density,
/// `k` intrinsic permeability, `k_rl` relative permeability, `mu_l` viscosity,
/// `p_l` liquid pressure, `g` gravity, and `Q_l` a source/sink. Saturation and
/// relative permeability follow characteristic (retention) curves of capillary
/// pressure (planned, bead op-v6s.7).
///
/// > **Scaffold.** This is a placeholder carrying no state yet; fields (grid
/// > handle, property model, primary-unknown vector) are added with bead
/// > op-v6s.8.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct RichardsMode {}

impl RichardsMode {
    /// Construct an (empty) RICHARDS mode placeholder.
    pub fn new() -> Self {
        Self {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn richards_mode_reports_its_name() {
        let mode = FlowMode::Richards(RichardsMode::new());
        assert_eq!(mode.name(), "RICHARDS");
    }

    #[test]
    fn scaffold_step_is_honest_not_implemented() {
        let mut mode = FlowMode::Richards(RichardsMode::new());
        assert!(matches!(
            mode.step(),
            Err(PflotranError::NotImplemented(_))
        ));
    }
}
