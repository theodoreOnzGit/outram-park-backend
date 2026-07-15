// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: src/classes/thermalHydraulics/src/physicsModels/turbulenceModels/
//             {LaheyKEpsilon,mixtureKEpsilon,porousKEpsilon,
//             porousKEpsilon2PhaseCorrected}/
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream author: Stefan Radman (EPFL)
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! # `closures::turbulence` — two-phase / porous turbulence closures
//!
//! Rust port of GeN-Foam's `physicsModels/turbulenceModels/{LaheyKEpsilon,
//! mixtureKEpsilon,porousKEpsilon,porousKEpsilon2PhaseCorrected}`. Each
//! upstream class is a full OpenFOAM `RASModel` that builds and solves
//! `fvScalarMatrix` transport equations for `k` and `epsilon` on mesh fields —
//! machinery this crate does not have and, per the port plan, does not
//! re-implement here (the generic single-phase k-epsilon transport equation
//! itself is reused from `outram_foam_turbulence_lib`). **This module ports
//! only the porous- and two-phase-specific closure terms**: the small,
//! self-contained pieces of algebra that modify or add to the generic
//! production/dissipation/eddy-viscosity terms. See the "Ported vs. deferred"
//! section below for the exact boundary.
//!
//! ## Sub-modules
//!
//! | Sub-module | Upstream class | Reference |
//! |---|---|---|
//! | [`lahey_k_epsilon`] | `LaheyKEpsilon` | Lahey Jr., R.T. (2005), *Nucl. Eng. Des.* 235(10), 1043-1060 |
//! | [`mixture_k_epsilon`] | `mixtureKEpsilon` | Behzadi, Issa & Rusche (2004), *Chem. Eng. Sci.* 59(4), 759-770; bubble term from Lahey (2005) |
//! | [`porous_k_epsilon`] | `porousKEpsilon`, `porousKEpsilon2PhaseCorrected` | GeN-Foam-original (no external reference) |
//! | [`units`] | — | local named `uom` aliases for this sub-module |
//!
//! `multiphaseCompressibleTurbulenceModels.C` (the upstream
//! `addToRunTimeSelectionTable`/`makeTurbulenceModelTypes` registration
//! boilerplate) has no algebraic content and is intentionally not ported.
//!
//! ## Ported vs. deferred
//!
//! **Ported** (pure algebra: local scalar in, local scalar out, no mesh/field
//! state):
//! - [`drag_coefficient_from_kd`] (below) — the `Cd()` inversion shared
//!   verbatim by `LaheyKEpsilon` and `mixtureKEpsilon`.
//! - Lahey: the bubble-induced eddy-viscosity addend, the bubble-induced
//!   production term `bubbleG`, the gas-phase-transfer relaxation rate, and
//!   the resulting `k`/`epsilon` production-rate compositions. See
//!   [`lahey_k_epsilon`].
//! - mixtureKEpsilon: the dispersed-phase turbulent-response coefficient
//!   `Ct2`, the virtual-mass-corrected effective gas density, the mixture
//!   density and mass-weighted `k`/`epsilon` mixing, the (dimensionally
//!   distinct) mixture `bubbleG`, and the resulting production-rate
//!   compositions. See [`mixture_k_epsilon`].
//! - porousKEpsilon / porousKEpsilon2PhaseCorrected: the turbulence-intensity
//!   correlation for the porous-zone equilibrium `k`, the mixing-length
//!   equilibrium `epsilon`, the relaxation-toward-equilibrium coefficient, and
//!   the `nut` stabilisation addend. See [`porous_k_epsilon`].
//!
//! **Deferred to the solver-integration bead** (needs mesh/field machinery
//! this crate does not have — `volScalarField`, `fvm::div`/`fvm::laplacian`,
//! `fvScalarMatrix` assembly, boundary-condition correction):
//! - The generic single-phase k-epsilon production `G =
//!   nut*(dev(twoSymm(gradU)) && gradU)` and the `k`/`epsilon` transport
//!   equations themselves (`fvm::ddt` + `fvm::div` + `fvm::laplacian` ==
//!   production − dissipation + these closure source terms). Reused from
//!   `outram_foam_turbulence_lib`.
//! - `correctNut()`'s field-level orchestration (looking up phase/pair
//!   objects from the mesh registry, calling `correctBoundaryConditions()`,
//!   `fv::options` correction).
//! - `porousKEpsilon`'s constructor-time per-region dictionary parsing that
//!   paints `convergenceLength`/`turbulenceIntensityCoeff`/etc. onto cell
//!   zones — a mesh/structure concern, not closure algebra. This module takes
//!   those already-resolved per-cell coefficients as plain `f64` struct
//!   fields instead.
//! - `porousKEpsilon::kSource`/`epsilonSource` are, upstream, literally empty
//!   `fvScalarMatrix`s (zero-dimensioned placeholders with no algebraic
//!   content) — there is nothing to port for either porous model.
//! - `mixtureKEpsilon::correct()`'s phase-averaging orchestration (looking up
//!   the sibling phase's turbulence model from the mesh registry, `mixFlux`,
//!   `mixU` on face-interpolated `surfaceScalarField`s, the final
//!   `kl = Cc2*km` / `kg = Ct2*kl` back-substitution) — all mesh/field state.
//!
//! Scaffold status: tracked by bead op-p6p.7.9; see `docs/genfoam-port-plan.md`.

pub mod lahey_k_epsilon;
pub mod mixture_k_epsilon;
pub mod porous_k_epsilon;
pub mod units;

#[cfg(test)]
mod tests;

pub use lahey_k_epsilon::LaheyBubbleClosure;
pub use mixture_k_epsilon::MixtureKEpsilonClosure;
pub use porous_k_epsilon::{PorousKEpsilon2PhaseClosure, PorousKEpsilonClosure};

use units::{
    DragCoefficient, HydraulicDiameter, RelativeVelocity, VoidFraction,
    VolumetricRelaxationCoefficient,
};
use uom::si::f64::MassDensity;
use uom::si::length::meter;
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::ratio::ratio;
use uom::si::velocity::meter_per_second;

/// Invert GeN-Foam's fluid-fluid drag coefficient `Cd` from the interfacial
/// friction coefficient `Kd`.
///
/// Shared verbatim by `LaheyKEpsilon::Cd()` and `mixtureKEpsilon::Cd()`
/// (identical upstream formula in both classes). `Kd` is defined (per the
/// upstream doc comment on both classes) by
///
/// ```text
/// Kd = 0.5 * (alpha_c*alpha_d)/(alpha_c+alpha_d) * (rho_c/Dh_d) * |U_c-U_d| * Cd
/// ```
///
/// with `c`/`d` the continuous/dispersed phase; this function solves that
/// relation for `Cd` given the already-computed `Kd` (produced elsewhere by
/// the fluid-fluid drag closures, out of scope here — see
/// `closures::ff_drag`, bead op-p6p.7.5).
///
/// **Deliberate deviation from upstream:** when `alpha_continuous +
/// alpha_dispersed == 0` (a degenerate cell with neither phase present), the
/// C++ `(li*gi)/(li+gi)` evaluates `0.0/0.0 = NaN`; this port instead treats
/// that fraction as `0.0` before applying the `max(..., 1e-3 m/s)` floor
/// (matching the physically sensible limit — no phase present, no drag —
/// rather than propagating a `NaN`). This is a numerical-robustness guard on
/// an unreachable-in-practice edge case, not a tolerance change.
#[must_use]
pub fn drag_coefficient_from_kd(
    kd: VolumetricRelaxationCoefficient,
    dh_dispersed: HydraulicDiameter,
    rho_continuous: MassDensity,
    alpha_continuous: VoidFraction,
    alpha_dispersed: VoidFraction,
    relative_velocity: RelativeVelocity,
) -> DragCoefficient {
    let ac = alpha_continuous.get::<ratio>();
    let ad = alpha_dispersed.get::<ratio>();
    let sum = ac + ad;
    let frac = if sum > 0.0 { ac * ad / sum } else { 0.0 };
    let denom = (frac * relative_velocity.get::<meter_per_second>()).max(1e-3);

    let cd = 2.0 * kd.value * dh_dispersed.get::<meter>()
        / rho_continuous.get::<kilogram_per_cubic_meter>()
        / denom;

    DragCoefficient::new::<ratio>(cd)
}
