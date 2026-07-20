// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! # `genfoam::thermal_hydraulics::solver` — porous TH equation drivers
//!
//! The one-phase and (planned) two-phase thermal-hydraulics solver drivers: the
//! porous momentum equation (`UEqn`, with an anisotropic drag tensor `Kd`
//! assembled from fluid-structure friction closures plus tortuosity-modified
//! turbulent diffusion), the porous energy equation (`EEqn`, coupled to the
//! structure via the heat-transfer coefficient), the PIMPLE pressure equation
//! (`pEqn`), and — for two-phase — MULES-limited `alpha` transport. It reuses
//! basic-lib `fvm`/`fvc` assembly and the crate's existing PIMPLE scaffolding
//! (`src/solvers/rho_pimple_foam.rs`). The closure correlations
//! ([`super::closures`]) and the field state ([`super::phase`]/[`super::structure`])
//! do NOT belong here — this module wires them into the equation loops.
//!
//! Ports upstream `src/classes/thermalHydraulics/solvers/**` (the `onePhase`,
//! `onePhaseLegacy`, and `twoPhase` top-level solver drivers).
//!
//! ## Module map
//!
//! | Sub-module | Provides | Upstream | Status |
//! |---|---|---|---|
//! | [`porous_drag`] | [`PorousDrag`] — the isotropic `Kd` drag-coefficient assembly from the wall-friction closure | `physicsModels/dragModels/FSDragFactor` | **Ported + V&V** |
//! | [`one_phase`] | [`OnePhaseSolver`] — porous `UEqn`/`pEqn`/`EEqn` driver | `solvers/onePhase/**` | **Ported + V&V** (constant-property slice; see its docs) |
//!
//! ## Dispatch: no trait objects
//!
//! Per the workspace no-`dyn` rule the solver family is a closed enum
//! ([`ThermalHydraulicsSolver`]) dispatched by value. Only [`OnePhaseSolver`] is
//! implemented so far; `onePhaseLegacy` and the two-phase (`twoPhase`, MULES
//! `alpha` transport + two-phase `pEqn`) drivers are tracked in beads
//! op-p6p.7.11 (follow-up) and op-p6p.7.12 and will each add a variant, at which
//! point every `match` on this enum becomes a compile error until updated — the
//! exhaustiveness the rule buys.

pub mod one_phase;
pub mod porous_drag;

#[cfg(test)]
mod tests;

pub use one_phase::OnePhaseSolver;
pub use porous_drag::PorousDrag;

use crate::error::AppBuilderError;

/// Closed-enum dispatch over the porous thermal-hydraulics solver drivers.
///
/// A [`ThermalHydraulicsSolver`] wraps one concrete driver; [`Self::step`]
/// forwards to it. Adding the two-phase driver (bead op-p6p.7.12) means adding a
/// `TwoPhase` variant here — every existing `match` then fails to compile until
/// it handles the new case, which is exactly why this is an enum and not a
/// `dyn` trait object.
pub enum ThermalHydraulicsSolver {
    /// Single-fluid + one stationary structure porous Eulerian solver
    /// (`solvers/onePhase`).
    OnePhase(OnePhaseSolver),
}

impl ThermalHydraulicsSolver {
    /// Advance the wrapped driver one time step of `dt` seconds.
    ///
    /// Mirrors upstream `thermalHydraulicsModel::correctPhysics`.
    pub fn step(&mut self, dt: f64) -> Result<(), AppBuilderError> {
        match self {
            Self::OnePhase(s) => s.step(dt),
        }
    }
}

// TODO(genfoam): port solvers/onePhaseLegacy (bead op-p6p.7.11 follow-up) and
// solvers/twoPhase MULES alpha transport + two-phase pEqn (bead op-p6p.7.12),
// each adding a variant to `ThermalHydraulicsSolver`.
