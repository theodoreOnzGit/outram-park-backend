// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com)
// Copyright (C) 2004-2023 OpenFOAM Foundation
// Copyright (C) 2016-2023 OpenCFD Ltd.
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.

//! Stage 2 — **Euler-Euler two-fluid model foundation** (bead `op-2kk.2`).
//!
//! Pure-Rust port of the phase / interfacial-momentum-transfer pieces of
//! OpenFOAM's `multiphaseEuler` solver family (the `phaseSystem`
//! interfacial-models layer), built on [`outram_foam_basic_lib`]'s
//! finite-volume framework.
//!
//! ## Physics
//!
//! The Euler-Euler ("two-fluid") model treats each phase as an
//! **interpenetrating continuum** with *its own* volume fraction, velocity, and
//! momentum balance — unlike the Stage-1 drift-flux model ([`crate::drift_flux`])
//! which collapses the two phases onto a single mixture momentum plus an
//! algebraic slip. Here a **dispersed** phase `d` (bubbles / droplets /
//! particles) and a **continuous** phase `c` (the carrier fluid) each carry a
//! fraction `α_k` and a velocity `U_k`, coupled through **interfacial momentum
//! transfer** — chiefly interphase drag. This foundation implements:
//!
//! 1. **Per-phase representation** ([`Phase`]) — the fields `α_k`, `U_k` plus
//!    constant density, viscosity and (for the dispersed phase) an inclusion
//!    diameter.
//! 2. **A two-phase system** ([`TwoFluidSystem`]) — a dispersed + continuous
//!    [`Phase`] on a shared [`FvMesh`], holding the saturation constraint
//!    `α_d + α_c = 1` ([`TwoFluidSystem::enforce_alpha_constraint`]).
//! 3. **Interfacial force closures as an enum** ([`DragModel`], wrapped by the
//!    extensible [`InterfacialForce`]) — the per-cell volumetric drag
//!    coefficient `K_d` `[kg/(m³·s)]` from Schiller-Naumann, Wen-Yu, or a
//!    prescribed constant. Lift / virtual-mass / wall-lubrication /
//!    turbulent-dispersion are **documented scaffolds** (see honest scope).
//! 4. **Per-phase continuity** ([`TwoFluidSystem::advance_dispersed_alpha`]) —
//!    one implicit-Euler step of `∂α_d/∂t + ∇·(φ_d·α_d) = 0` on the dispersed
//!    phase's own flux, clamped to `[0,1]`, with `α_c` re-derived from the
//!    saturation constraint.
//!
//! ## Upstream provenance (ported C++ → Rust)
//!
//! Source tree: `applications/modules/multiphaseEuler/phaseSystem/` of
//! OpenFOAM-dev (GPL-3.0). Exact file/line citations appear on each item.
//! Summary:
//!
//! | Rust item | OpenFOAM source |
//! |---|---|
//! | [`DragModel::SchillerNaumann`] `CdRe` | `interfacialModels/dragModels/SchillerNaumann/SchillerNaumann.C:62` |
//! | [`DragModel::WenYu`] `CdRe` | `interfacialModels/dragModels/WenYu/WenYu.C:62` |
//! | [`DragModel::k_d`] (`Ki`/`K`) | `interfacialModels/dragModels/dispersedDragModel/dispersedDragModel.C:58` (`Ki`), `:70` (`K`) |
//! | [`TwoFluidSystem::reynolds_number`] | `phaseInterface/dispersedPhaseInterface/dispersedPhaseInterface.C:136` (`Re`) + `phaseInterface/phaseInterface/phaseInterface.C:545` (`magUr`) |
//! | [`TwoFluidSystem::advance_dispersed_alpha`] | `phaseSystem/phaseSystem/phaseSystemSolve.C:336` (`fvScalarMatrix alphaEqn`) |
//!
//! ## Honest scope — what is **NOT** modelled here
//!
//! This is a **tested foundation**, not a converged coupled two-fluid solver.
//! Reviewers and users must not read it as validated multiphase CFD.
//! Specifically:
//!
//! - **No phase-momentum / pressure-coupled PIMPLE loop.** There is no
//!   `U_d`/`U_c`/`p` solve, no shared pressure Poisson equation, no
//!   partial-elimination (PEA) drag coupling. The phase velocities `U_k` are
//!   *inputs* the caller prescribes; only the dispersed-phase continuity
//!   (`α_d` transport) is advanced, on the given `U_d`. Assembling and solving
//!   the two coupled momentum equations with the drag `K_d` computed here is
//!   later work.
//! - **Drag is the only implemented interfacial force.** Lift, virtual (added)
//!   mass, wall lubrication, and turbulent dispersion are **documented scaffold
//!   variants** ([`InterfacialForce`]) that return
//!   [`MultiphaseError::NotImplemented`] — the API shape is present so the
//!   architecture extends to a full 6-equation system, but the closures are
//!   **not** ported and must not be treated as available.
//! - **Constant per-phase properties, constant diameter.** `ρ_k`, `μ_k` and the
//!   dispersed diameter `d` are constants; no thermo, no population balance / no
//!   bubble-size distribution, no swarm correction (OpenFOAM's `Cs` is taken as
//!   `1`, matching its `noSwarm` default).
//! - **Simplified `α` bounding.** OpenFOAM bounds phase fractions with MULES (a
//!   flux-corrected, strictly conservative limiter). Here `α_d` is bounded by a
//!   plain post-solve **clamp to `[0,1]`**, which keeps it physical but is
//!   **not** globally conservative when the clamp is active. `α_c` is then set
//!   by `α_c = 1 − α_d`, so the saturation constraint holds exactly but shares
//!   the clamp's (small) non-conservation.
//! - **First-order in space and time.** Upwind convection + implicit Euler.
//! - **No `uom` on the `Vector3` velocity boundary.** Following the crate's
//!   `drift_flux` / `k_omega_sst` precedent, vector velocities are carried as
//!   `Vector3` in SI `m/s` (documented per field); `uom` types the *scalar*
//!   property inputs (density, viscosity, diameter) at the constructor boundary.
//!
//! **Benchmark validation is a later, human-run step.** No benchmark comparison
//! has been performed; the tests below are *verification* checks (formula
//! exactness against hand-computed Reynolds-number drag cases, the saturation
//! constraint, advection boundedness), not *validation* against experimental or
//! reference-solver data.

use crate::MultiphaseError;
use outram_foam_basic_lib::fv_operators::{fvc, fvm};
use outram_foam_basic_lib::prelude::{
    Field, FvMesh, PatchField, SolverSettings, SurfaceScalarField, VolScalarField, VolVectorField,
};
use std::sync::Arc;
use uom::si::dynamic_viscosity::pascal_second;
use uom::si::f64::{DynamicViscosity, KinematicViscosity, Length, MassDensity};
use uom::si::kinematic_viscosity::square_meter_per_second;
use uom::si::length::meter;
use uom::si::mass_density::kilogram_per_cubic_meter;

/// Residual (floor) volume fraction `α_res` `[-]` used to keep the drag
/// coefficient well-defined as a phase disappears.
///
/// Mirrors OpenFOAM's `phase::residualAlpha()` guard used in
/// `dispersedDragModel::K()` (`dispersedDragModel.C:70`, `max(αd, residualAlpha)`)
/// and in `WenYu::CdRe()` (`WenYu.C:66`, `max(αc, residualAlpha)`). A small
/// positive constant (`1e-6`) rather than a per-phase dictionary entry at this
/// foundation stage.
pub const RESIDUAL_ALPHA: f64 = 1.0e-6;

/// Per-phase volume fraction `α_k`, dimensionless, valid range `[0, 1]`.
///
/// Carried as an [`outram_foam_basic_lib`] [`VolScalarField`] (per-cell `f64`).
/// This alias documents the physical meaning at the API boundary.
pub type PhaseFraction = VolScalarField;

// ─────────────────────────────────────────────────────────────────────────────
// Phase representation
// ─────────────────────────────────────────────────────────────────────────────

/// One phase of a Euler-Euler two-fluid system: its volume-fraction field
/// `α_k`, velocity field `U_k`, and constant material properties.
///
/// Ports the state a `Foam::phaseModel` exposes to the interfacial-momentum
/// closures — a volume fraction (`phase()`), a velocity (`U()`), a density
/// (`rho()`), a kinematic viscosity (`fluidThermo().nu()`), and, for a
/// dispersed phase, a diameter (`d()`). Here those properties are **constants**
/// (incompressible, isothermal foundation); thermo and size distributions are
/// out of scope (see the module "Honest scope" section).
///
/// ## Units and valid ranges
///
/// - `α_k` — volume fraction `[-]`, physical range `[0, 1]`.
/// - `U_k` — velocity `[m/s]`, carried as `Vector3` in SI (not `uom`-typed).
/// - `ρ_k` — mass density `[kg/m³]`, must be **> 0**.
/// - `μ_k` — dynamic viscosity `[Pa·s]`, must be **≥ 0**.
/// - `d`  — characteristic inclusion diameter `[m]`, must be **> 0**. Physically
///   meaningful only when this phase acts as the *dispersed* phase in a drag
///   closure; a continuous phase may carry any positive placeholder.
///
/// Scalar properties are stored internally as `f64` in SI base units; the
/// constructor and scalar accessors use `uom` types so the physical dimension
/// is checked at the API boundary.
pub struct Phase {
    /// Finite-volume mesh the fields live on (shared, `Arc`).
    pub mesh: Arc<FvMesh>,
    /// Human-readable phase name (e.g. `"air"`, `"water"`).
    name: String,
    /// Volume-fraction field `α_k ∈ [0, 1]` `[-]`.
    alpha: PhaseFraction,
    /// Velocity field `U_k` `[m/s]` (SI, carried as `Vector3`).
    u: VolVectorField,
    /// Density `ρ_k` `[kg/m³]`.
    rho: f64,
    /// Dynamic viscosity `μ_k` `[Pa·s]`.
    mu: f64,
    /// Characteristic inclusion diameter `d` `[m]` (dispersed-phase property).
    d: f64,
}

impl Phase {
    /// Construct a phase from constant properties and a uniform initial `α_k`.
    ///
    /// The `α_k` and `U_k` fields are created uniform with zero-gradient
    /// boundaries on every patch; install boundary conditions (e.g. a
    /// `FixedValue` inlet) via [`alpha_mut`](Self::alpha_mut) /
    /// [`u_mut`](Self::u_mut) before advancing transport if an inflow is needed.
    ///
    /// # Parameters
    /// - `mesh` — the finite-volume mesh (shared, `Arc`).
    /// - `name` — phase label.
    /// - `rho` — density `[kg/m³]` (`uom`), must be strictly positive.
    /// - `mu` — dynamic viscosity `[Pa·s]` (`uom`), must be non-negative.
    /// - `d` — inclusion diameter `[m]` (`uom`), must be strictly positive.
    /// - `alpha0` — uniform initial volume fraction `[-]`, in `[0, 1]`.
    ///
    /// # Errors
    /// [`MultiphaseError::InvalidInput`] if `rho ≤ 0`, `mu < 0`, `d ≤ 0`, or
    /// `alpha0 ∉ [0, 1]`.
    pub fn new(
        mesh: Arc<FvMesh>,
        name: impl Into<String>,
        rho: MassDensity,
        mu: DynamicViscosity,
        d: Length,
        alpha0: f64,
    ) -> Result<Self, MultiphaseError> {
        let rho = rho.get::<kilogram_per_cubic_meter>();
        let mu = mu.get::<pascal_second>();
        let d = d.get::<meter>();
        if rho <= 0.0 {
            return Err(MultiphaseError::InvalidInput(format!(
                "phase density must be > 0 (got rho={rho})"
            )));
        }
        if mu < 0.0 {
            return Err(MultiphaseError::InvalidInput(format!(
                "phase viscosity must be >= 0 (got mu={mu})"
            )));
        }
        if d <= 0.0 {
            return Err(MultiphaseError::InvalidInput(format!(
                "phase diameter must be > 0 (got d={d})"
            )));
        }
        if !(0.0..=1.0).contains(&alpha0) {
            return Err(MultiphaseError::InvalidInput(format!(
                "initial alpha must be in [0,1] (got {alpha0})"
            )));
        }
        let name = name.into();
        let alpha = VolScalarField::uniform(&name, mesh.clone(), alpha0);
        let u = VolVectorField::zero("U", mesh.clone());
        Ok(Self {
            mesh,
            name,
            alpha,
            u,
            rho,
            mu,
            d,
        })
    }

    /// Phase name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Density `ρ_k` `[kg/m³]`.
    pub fn rho(&self) -> MassDensity {
        MassDensity::new::<kilogram_per_cubic_meter>(self.rho)
    }

    /// Dynamic viscosity `μ_k` `[Pa·s]`.
    pub fn mu(&self) -> DynamicViscosity {
        DynamicViscosity::new::<pascal_second>(self.mu)
    }

    /// Kinematic viscosity `ν_k = μ_k / ρ_k` `[m²/s]`.
    ///
    /// This is the quantity the drag Reynolds number and `Ki` coefficient use
    /// (OpenFOAM `continuous().fluidThermo().nu()`). Well-defined since the
    /// constructor guarantees `ρ_k > 0`.
    pub fn nu(&self) -> KinematicViscosity {
        KinematicViscosity::new::<square_meter_per_second>(self.mu / self.rho)
    }

    /// Characteristic inclusion diameter `d` `[m]` (dispersed-phase property).
    pub fn diameter(&self) -> Length {
        Length::new::<meter>(self.d)
    }

    /// The volume-fraction field `α_k` `[-]` (read-only).
    pub fn alpha(&self) -> &PhaseFraction {
        &self.alpha
    }

    /// The volume-fraction field `α_k` `[-]` (mutable) — set the initial
    /// internal distribution or install a boundary condition before transport.
    pub fn alpha_mut(&mut self) -> &mut PhaseFraction {
        &mut self.alpha
    }

    /// The velocity field `U_k` `[m/s]` (read-only).
    pub fn u(&self) -> &VolVectorField {
        &self.u
    }

    /// The velocity field `U_k` `[m/s]` (mutable) — prescribe the phase velocity
    /// (this foundation does not solve phase momentum; see module honest scope).
    pub fn u_mut(&mut self) -> &mut VolVectorField {
        &mut self.u
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Interfacial drag closures
// ─────────────────────────────────────────────────────────────────────────────

/// Interphase **drag** closure returning the per-cell volumetric drag
/// coefficient `K_d` `[kg/(m³·s)]`.
///
/// `K_d` is the coefficient in the interfacial drag force `K_d·(U_c − U_d)` that
/// appears (with opposite signs) in the two phase-momentum equations
/// (`dragModel.H`: *"`ddt(alpha1*rho1*U1) + … = … K*(U1-U2)`"*). Enum dispatch
/// (not `dyn`) per the workspace design rules: the set of drag closures is
/// closed and known at compile time, so a `match` is exhaustive and every
/// variant is rust-analyzer-navigable.
///
/// ## Coefficient construction (ported)
///
/// For the correlation-based variants the coefficient follows OpenFOAM's
/// dispersed-drag chain (`dispersedDragModel.C`):
///
/// - the model supplies `CdRe = C_d·Re` (a Reynolds-number-scaled drag
///   coefficient), then
/// - `Ki = 0.75·CdRe·C_s·ρ_c·ν_c / d²` (`dispersedDragModel.C:58`, with the
///   swarm correction `C_s = 1`, matching the `noSwarm` default), and
/// - `K_d = max(α_d, α_res)·Ki` (`dispersedDragModel.C:70`).
///
/// where `Re = |U_d − U_c|·d/ν_c` (see [`TwoFluidSystem::reynolds_number`]).
///
/// ## Variants
/// - [`SchillerNaumann`](Self::SchillerNaumann) — standard dispersed-bubble /
///   sphere correlation.
/// - [`WenYu`](Self::WenYu) — Wen-Yu correlation with a `α_c^{−2.65}` voidage
///   correction (dense particle beds / high dispersed loading).
/// - [`Constant`](Self::Constant) — a prescribed, spatially-uniform `K_d`, for
///   testing and for cases where a coefficient is supplied externally.
pub enum DragModel {
    /// **Schiller-Naumann** drag for dispersed bubbly / particulate flow
    /// (Schiller & Naumann, 1935). Ports `SchillerNaumann::CdRe()`
    /// (`SchillerNaumann.C:62`):
    ///
    /// ```text
    /// CdRe = Re < 1000 ? 24·(1 + 0.15·Re^0.687) : 0.44·Re
    /// ```
    ///
    /// Valid for a single inclusion in an unbounded fluid; the dilute limit of
    /// the [`WenYu`](Self::WenYu) form (`α_c = 1`).
    SchillerNaumann,

    /// **Wen-Yu** drag (Wen & Yu, 1966) — Schiller-Naumann evaluated on a
    /// voidage-scaled Reynolds number with a `α_c^{−2.65}` hindrance amplifier.
    /// Ports `WenYu::CdRe()` (`WenYu.C:62`):
    ///
    /// ```text
    /// α₂   = max(α_c, α_res)
    /// Res  = α₂·Re
    /// CdRe = [ Res < 1000 ? 24·(1 + 0.15·Res^0.687) : 0.44·Res ] · α₂^(−2.65)
    /// ```
    ///
    /// Reduces to [`SchillerNaumann`](Self::SchillerNaumann) as `α_c → 1`
    /// (dilute dispersed phase). Suited to higher dispersed loadings.
    WenYu,

    /// **Constant** prescribed volumetric drag coefficient `K_d` `[kg/(m³·s)]`,
    /// spatially uniform. Not a physical correlation — a fixed value for
    /// verification tests and for coefficients supplied by an external model.
    Constant {
        /// The uniform drag coefficient `K_d` `[kg/(m³·s)]`, must be `≥ 0`.
        k_d: f64,
    },
}

impl DragModel {
    /// Per-cell volumetric drag coefficient `K_d` `[kg/(m³·s)]` for the given
    /// two-fluid `system`.
    ///
    /// Returns a `VolScalarField` in `kg/(m³·s)` with zero-gradient boundaries.
    /// The correlation variants read `ρ_c`, `ν_c` from the continuous phase, the
    /// diameter `d` and fraction `α_d` from the dispersed phase, and the slip
    /// Reynolds number from [`TwoFluidSystem::reynolds_number`].
    ///
    /// # Errors
    /// - [`MultiphaseError::InvalidInput`] for [`Constant`](Self::Constant) with
    ///   `k_d < 0`, or (for the correlation variants) if the Reynolds-number
    ///   evaluation fails its own preconditions (`ν_c > 0`, `d > 0`).
    pub fn k_d(&self, system: &TwoFluidSystem) -> Result<VolScalarField, MultiphaseError> {
        let mesh = system.mesh.clone();
        let n = mesh.n_cells;
        match self {
            Self::Constant { k_d } => {
                if *k_d < 0.0 {
                    return Err(MultiphaseError::InvalidInput(format!(
                        "constant drag coefficient must be >= 0 (got {k_d})"
                    )));
                }
                Ok(VolScalarField::uniform("Kd", mesh, *k_d))
            }
            Self::SchillerNaumann => {
                let re = system.reynolds_number()?;
                let rho_c = system.continuous.rho;
                let nu_c = system.continuous.mu / system.continuous.rho;
                let d = system.dispersed.d;
                let re_in = re.internal.as_slice();
                let ad = system.dispersed.alpha.internal.as_slice();
                let vals: Vec<f64> = (0..n)
                    .map(|c| {
                        let cd_re = schiller_naumann_cd_re(re_in[c]);
                        let ki = 0.75 * cd_re * rho_c * nu_c / (d * d);
                        ad[c].max(RESIDUAL_ALPHA) * ki
                    })
                    .collect();
                Ok(scalar_field("Kd", mesh, vals))
            }
            Self::WenYu => {
                let re = system.reynolds_number()?;
                let rho_c = system.continuous.rho;
                let nu_c = system.continuous.mu / system.continuous.rho;
                let d = system.dispersed.d;
                let re_in = re.internal.as_slice();
                let ad = system.dispersed.alpha.internal.as_slice();
                let ac = system.continuous.alpha.internal.as_slice();
                let vals: Vec<f64> = (0..n)
                    .map(|c| {
                        let cd_re = wen_yu_cd_re(re_in[c], ac[c]);
                        let ki = 0.75 * cd_re * rho_c * nu_c / (d * d);
                        ad[c].max(RESIDUAL_ALPHA) * ki
                    })
                    .collect();
                Ok(scalar_field("Kd", mesh, vals))
            }
        }
    }
}

/// Schiller-Naumann `CdRe = C_d·Re` for one cell (`SchillerNaumann.C:62`).
///
/// `Re < 1000 ⇒ 24·(1 + 0.15·Re^0.687)`, else the Newton-regime `0.44·Re`.
/// `Re` is the slip Reynolds number `[-]`, assumed non-negative.
fn schiller_naumann_cd_re(re: f64) -> f64 {
    if re < 1000.0 {
        24.0 * (1.0 + 0.15 * re.powf(0.687))
    } else {
        0.44 * re
    }
}

/// Wen-Yu `CdRe = C_d·Re` for one cell (`WenYu.C:62`).
///
/// Schiller-Naumann evaluated on `Res = α₂·Re` (with `α₂ = max(α_c, α_res)`),
/// then multiplied by the voidage hindrance factor `α₂^{−2.65}`. `re` is the
/// slip Reynolds number `[-]`; `alpha_c` the continuous fraction `[-]`.
fn wen_yu_cd_re(re: f64, alpha_c: f64) -> f64 {
    let a2 = alpha_c.max(RESIDUAL_ALPHA);
    let res = a2 * re;
    let cds_res = if res < 1000.0 {
        24.0 * (1.0 + 0.15 * res.powf(0.687))
    } else {
        0.44 * res
    };
    cds_res * a2.powf(-2.65)
}

// ─────────────────────────────────────────────────────────────────────────────
// Extensible interfacial-force enum (drag implemented; others scaffolded)
// ─────────────────────────────────────────────────────────────────────────────

/// Interfacial momentum-transfer force closure — the extensible enum from which
/// a full 6-equation two-fluid system draws its phase-coupling terms.
///
/// **Only [`Drag`](Self::Drag) is implemented** at this foundation stage. The
/// remaining variants are **documented scaffolds**: they exist so the API and
/// the dispatch `match` already have the right shape for the 6-equation
/// extension noted in the crate roadmap, but their closures are **not** ported
/// and their [`momentum_coefficient`](Self::momentum_coefficient) arm returns
/// [`MultiphaseError::NotImplemented`]. They must not be treated as available.
///
/// Enum dispatch (not `dyn`) per the workspace design rules.
///
/// ## Variants and their (future) OpenFOAM sources
/// - [`Drag`](Self::Drag) — **implemented**, see [`DragModel`].
/// - [`Lift`](Self::Lift) — scaffold; ports from
///   `interfacialModels/liftModels/` (Tomiyama, Moraga, constant …).
/// - [`VirtualMass`](Self::VirtualMass) — scaffold;
///   `interfacialModels/virtualMassModels/` (constant-coefficient, Lamb).
/// - [`WallLubrication`](Self::WallLubrication) — scaffold;
///   `interfacialModels/wallLubricationModels/` (Antal, Tomiyama, Frank).
/// - [`TurbulentDispersion`](Self::TurbulentDispersion) — scaffold;
///   `interfacialModels/turbulentDispersionModels/` (Burns, Gosman, …).
pub enum InterfacialForce {
    /// Interphase **drag** — the only implemented interfacial force. Wraps a
    /// [`DragModel`]; [`momentum_coefficient`](Self::momentum_coefficient)
    /// returns its `K_d`.
    Drag(DragModel),
    /// **Lift** force — documented scaffold, not implemented.
    Lift,
    /// **Virtual (added) mass** force — documented scaffold, not implemented.
    VirtualMass,
    /// **Wall lubrication** force — documented scaffold, not implemented.
    WallLubrication,
    /// **Turbulent dispersion** force — documented scaffold, not implemented.
    TurbulentDispersion,
}

impl InterfacialForce {
    /// Per-cell interfacial momentum-transfer coefficient `[kg/(m³·s)]` for the
    /// given `system`.
    ///
    /// For [`Drag`](Self::Drag) this is the drag coefficient `K_d` (see
    /// [`DragModel::k_d`]). All other variants are unported scaffolds.
    ///
    /// # Errors
    /// - Propagates [`DragModel::k_d`]'s errors for [`Drag`](Self::Drag).
    /// - [`MultiphaseError::NotImplemented`] for every scaffold variant (lift,
    ///   virtual mass, wall lubrication, turbulent dispersion) — their closures
    ///   are not yet ported.
    pub fn momentum_coefficient(
        &self,
        system: &TwoFluidSystem,
    ) -> Result<VolScalarField, MultiphaseError> {
        match self {
            Self::Drag(model) => model.k_d(system),
            Self::Lift => Err(MultiphaseError::NotImplemented(
                "lift force closure not yet ported (see interfacialModels/liftModels)".into(),
            )),
            Self::VirtualMass => Err(MultiphaseError::NotImplemented(
                "virtual-mass closure not yet ported (see interfacialModels/virtualMassModels)"
                    .into(),
            )),
            Self::WallLubrication => Err(MultiphaseError::NotImplemented(
                "wall-lubrication closure not yet ported \
                 (see interfacialModels/wallLubricationModels)"
                    .into(),
            )),
            Self::TurbulentDispersion => Err(MultiphaseError::NotImplemented(
                "turbulent-dispersion closure not yet ported \
                 (see interfacialModels/turbulentDispersionModels)"
                    .into(),
            )),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Two-fluid system: phases, constraint, Reynolds number, phase continuity
// ─────────────────────────────────────────────────────────────────────────────

/// A Euler-Euler two-fluid system: a **dispersed** and a **continuous**
/// [`Phase`] on a shared [`FvMesh`], subject to the saturation constraint
/// `α_d + α_c = 1`.
///
/// **Not a coupled solver.** The phase velocities `U_k` are prescribed inputs
/// (this foundation does not solve phase momentum); the system advances only the
/// dispersed-phase continuity ([`advance_dispersed_alpha`](Self::advance_dispersed_alpha))
/// and re-derives `α_c` from the constraint. Interfacial drag is available via a
/// [`DragModel`] / [`InterfacialForce`] evaluated against this system. See the
/// module "Honest scope" section for what is deliberately omitted.
///
/// The constraint `α_d + α_c = 1` is held by construction and re-imposed after
/// each transport step by [`enforce_alpha_constraint`](Self::enforce_alpha_constraint).
pub struct TwoFluidSystem {
    /// Finite-volume mesh (shared, `Arc`).
    pub mesh: Arc<FvMesh>,
    /// Dispersed phase `d` (bubbles / droplets / particles).
    pub dispersed: Phase,
    /// Continuous phase `c` (carrier fluid); its `α_c` is kept equal to `1 − α_d`.
    pub continuous: Phase,
}

impl TwoFluidSystem {
    /// Assemble a two-fluid system from a dispersed and a continuous [`Phase`].
    ///
    /// Both phases must share the **same** mesh (checked by `Arc` pointer
    /// identity). The continuous fraction `α_c` is initialised to `1 − α_d`
    /// cell-by-cell so the saturation constraint holds immediately, regardless
    /// of the `alpha0` the continuous phase was constructed with.
    ///
    /// # Errors
    /// [`MultiphaseError::InvalidInput`] if the two phases were built on
    /// different meshes.
    pub fn new(dispersed: Phase, continuous: Phase) -> Result<Self, MultiphaseError> {
        if !Arc::ptr_eq(&dispersed.mesh, &continuous.mesh) {
            return Err(MultiphaseError::InvalidInput(
                "dispersed and continuous phases must share the same mesh".into(),
            ));
        }
        let mesh = dispersed.mesh.clone();
        let mut system = Self {
            mesh,
            dispersed,
            continuous,
        };
        system.enforce_alpha_constraint();
        Ok(system)
    }

    /// Re-impose the saturation constraint `α_c = 1 − α_d` on every cell.
    ///
    /// Only the continuous phase's **internal** field is overwritten; its
    /// boundary conditions are left intact. Call after mutating `α_d` (e.g.
    /// after [`advance_dispersed_alpha`](Self::advance_dispersed_alpha)) to keep
    /// `α_d + α_c = 1` exact.
    pub fn enforce_alpha_constraint(&mut self) {
        let n = self.mesh.n_cells;
        let ad: Vec<f64> = self.dispersed.alpha.internal.as_slice()[..n].to_vec();
        let ac = self.continuous.alpha.internal.as_mut_slice();
        for c in 0..n {
            ac[c] = 1.0 - ad[c];
        }
    }

    /// Per-cell slip **Reynolds number** `Re = |U_d − U_c|·d / ν_c` `[-]`.
    ///
    /// Ports `dispersedPhaseInterface::Re()`
    /// (`dispersedPhaseInterface.C:136`: `magUr()*dispersed().d()/continuous().nu()`)
    /// with `magUr = |U_d − U_c|` (`phaseInterface.C:545`). Uses the dispersed
    /// diameter and the *continuous*-phase kinematic viscosity. Result is a
    /// `VolScalarField` `[-]` with zero-gradient boundaries.
    ///
    /// # Errors
    /// [`MultiphaseError::InvalidInput`] if the continuous kinematic viscosity
    /// `ν_c ≤ 0` (Reynolds number undefined) — note a zero continuous viscosity
    /// is permitted by [`Phase::new`] but not by this drag path.
    pub fn reynolds_number(&self) -> Result<VolScalarField, MultiphaseError> {
        let nu_c = self.continuous.mu / self.continuous.rho;
        if nu_c <= 0.0 {
            return Err(MultiphaseError::InvalidInput(format!(
                "continuous kinematic viscosity must be > 0 for Reynolds number (got {nu_c})"
            )));
        }
        let d = self.dispersed.d;
        let n = self.mesh.n_cells;
        let ud = self.dispersed.u.internal.as_slice();
        let uc = self.continuous.u.internal.as_slice();
        let vals: Vec<f64> = (0..n).map(|c| (ud[c] - uc[c]).mag() * d / nu_c).collect();
        Ok(scalar_field("Re", self.mesh.clone(), vals))
    }

    /// Advance the dispersed-phase volume fraction `α_d` by one implicit-Euler
    /// step of its continuity equation, clamp to `[0,1]`, then re-derive `α_c`.
    ///
    /// ## Equation solved
    /// ```text
    /// ∂α_d/∂t  +  ∇·(φ_d·α_d)  =  0
    /// ```
    /// where `φ_d = U_d·S_f` is the dispersed-phase face flux formed from the
    /// prescribed velocity `U_d` via [`fvc::flux`]. Discretisation (mirrors the
    /// phase-fraction equation `fvScalarMatrix alphaEqn` in
    /// `phaseSystemSolve.C:336`, without mass-transfer sources):
    ///
    /// - `∂α_d/∂t` — implicit Euler (`fvm::ddt`).
    /// - `∇·(φ_d·α_d)` — implicit first-order upwind (`fvm::div`).
    ///
    /// After the linear solve, `α_d` is **clamped** to `[0,1]` cell-by-cell (a
    /// MULES-free stand-in for OpenFOAM's flux-corrected bounding — physical but
    /// not strictly conservative when the clamp bites), and the saturation
    /// constraint `α_c = 1 − α_d` is re-imposed
    /// ([`enforce_alpha_constraint`](Self::enforce_alpha_constraint)). The
    /// dispersed `α_d` field's boundary conditions are preserved (only internal
    /// values are updated), so a `FixedValue` inlet keeps feeding the domain.
    ///
    /// # Parameters
    /// - `dt` — time step `Δt` `[s]`, must be `> 0`.
    ///
    /// # Errors
    /// - [`MultiphaseError::InvalidInput`] if `dt ≤ 0`.
    /// - [`MultiphaseError::Solver`] if the linear solve returns a non-finite
    ///   value.
    pub fn advance_dispersed_alpha(&mut self, dt: f64) -> Result<(), MultiphaseError> {
        if dt <= 0.0 {
            return Err(MultiphaseError::InvalidInput(format!(
                "dt must be > 0 (got {dt})"
            )));
        }
        let mesh = self.mesh.clone();
        let n = mesh.n_cells;

        // Dispersed-phase face flux φ_d = U_d · S_f  [m³/s].
        let phi_d: SurfaceScalarField = fvc::flux(&self.dispersed.u);

        // Assemble  ∂α_d/∂t + ∇·(φ_d α_d) = 0  (implicit Euler + upwind div).
        let alpha_old = self.dispersed.alpha.clone();
        let eqn = fvm::ddt(&self.dispersed.alpha, &alpha_old, dt)
            + fvm::div(&phi_d, &self.dispersed.alpha);

        // Solve and bound to [0,1] (internal values only).
        let (sol, _perf) = eqn.solve("alpha.dispersed", SolverSettings::default());
        let new_vals = sol.internal.as_slice();
        {
            let internal = self.dispersed.alpha.internal.as_mut_slice();
            for c in 0..n {
                if !new_vals[c].is_finite() {
                    return Err(MultiphaseError::Solver(format!(
                        "dispersed alpha became non-finite in cell {c}"
                    )));
                }
                internal[c] = new_vals[c].clamp(0.0, 1.0);
            }
        }

        // Re-impose α_c = 1 − α_d.
        self.enforce_alpha_constraint();
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Internal helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Build a `VolScalarField` from per-cell values with zero-gradient boundaries.
fn scalar_field(name: &str, mesh: Arc<FvMesh>, vals: Vec<f64>) -> VolScalarField {
    let boundary = mesh
        .patches
        .iter()
        .map(|p| PatchField::zero_gradient(p.size))
        .collect();
    VolScalarField::new(name, mesh, Field::new(vals), boundary)
}

// ═════════════════════════════════════════════════════════════════════════════
// Tests — verification (formula exactness, hand-computed drag closures,
// saturation constraint, advection boundedness). NOT validation: no
// benchmark/experimental comparison here.
// ═════════════════════════════════════════════════════════════════════════════
#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;
    use outram_foam_basic_lib::prelude::{BoundaryPatch, FvMeshBuilder, PatchKind, Vector3};

    fn vx(x: f64) -> Vector3 {
        Vector3::new(x, 0.0, 0.0)
    }

    fn rho(v: f64) -> MassDensity {
        MassDensity::new::<kilogram_per_cubic_meter>(v)
    }

    fn mu(v: f64) -> DynamicViscosity {
        DynamicViscosity::new::<pascal_second>(v)
    }

    fn dia(v: f64) -> Length {
        Length::new::<meter>(v)
    }

    /// Single-cell mesh with two 1-face boundary patches (no internal faces) —
    /// enough to exercise the algebraic closure formulas.
    fn one_cell_mesh() -> Arc<FvMesh> {
        Arc::new(
            FvMeshBuilder::new()
                .n_cells(1)
                .n_internal_faces(0)
                .owner(vec![0, 0])
                .neighbour(vec![])
                .patches(vec![
                    BoundaryPatch::new("left", 0, 1, PatchKind::Patch),
                    BoundaryPatch::new("right", 1, 1, PatchKind::Patch),
                ])
                .cell_volumes(vec![1.0])
                .cell_centres(vec![vx(0.5)])
                .face_area_vectors(vec![vx(-1.0), vx(1.0)])
                .face_centres(vec![vx(0.0), vx(1.0)])
                .build()
                .unwrap(),
        )
    }

    /// 4-cell line mesh along x (centres 0.5,1.5,2.5,3.5; unit volumes/areas).
    /// Faces 0..2 internal (owner i, neighbour i+1); face 3 = inlet at x=0
    /// (outward normal −x); face 4 = outlet at x=4 (outward normal +x).
    fn line_mesh_4() -> Arc<FvMesh> {
        Arc::new(
            FvMeshBuilder::new()
                .n_cells(4)
                .n_internal_faces(3)
                .owner(vec![0, 1, 2, 0, 3])
                .neighbour(vec![1, 2, 3])
                .patches(vec![
                    BoundaryPatch::new("inlet", 3, 1, PatchKind::Patch),
                    BoundaryPatch::new("outlet", 4, 1, PatchKind::Patch),
                ])
                .cell_volumes(vec![1.0, 1.0, 1.0, 1.0])
                .cell_centres(vec![vx(0.5), vx(1.5), vx(2.5), vx(3.5)])
                .face_area_vectors(vec![vx(1.0), vx(1.0), vx(1.0), vx(-1.0), vx(1.0)])
                .face_centres(vec![vx(1.0), vx(2.0), vx(3.0), vx(0.0), vx(4.0)])
                .build()
                .unwrap(),
        )
    }

    /// Build a standard air-in-water two-fluid system on `mesh` with a uniform
    /// dispersed fraction `alpha_d0`. Air: ρ=1.2, μ=1.8e-5, d=1mm. Water:
    /// ρ=1000, μ=1e-3 (ν_c=1e-6 m²/s), d placeholder 1mm.
    fn air_water(mesh: Arc<FvMesh>, alpha_d0: f64) -> TwoFluidSystem {
        let disp = Phase::new(
            mesh.clone(),
            "air",
            rho(1.2),
            mu(1.8e-5),
            dia(1e-3),
            alpha_d0,
        )
        .unwrap();
        let cont =
            Phase::new(mesh.clone(), "water", rho(1000.0), mu(1e-3), dia(1e-3), 0.0).unwrap();
        TwoFluidSystem::new(disp, cont).unwrap()
    }

    // ── Phase construction / constraint ──────────────────────────────────────

    /// Constructor rejects non-physical inputs.
    #[test]
    fn phase_constructor_rejects_bad_inputs() {
        let m = one_cell_mesh();
        assert!(Phase::new(m.clone(), "p", rho(0.0), mu(0.0), dia(1e-3), 0.0).is_err()); // rho<=0
        assert!(Phase::new(m.clone(), "p", rho(1.0), mu(-1.0), dia(1e-3), 0.0).is_err()); // mu<0
        assert!(Phase::new(m.clone(), "p", rho(1.0), mu(0.0), dia(0.0), 0.0).is_err()); // d<=0
        assert!(Phase::new(m.clone(), "p", rho(1.0), mu(0.0), dia(1e-3), 1.5).is_err());
        // alpha>1
    }

    /// Methodology: build a system with α_d = 0.3 uniform; the constructor must
    /// set α_c = 0.7 so α_d + α_c = 1 in every cell. `new()` ignores the
    /// continuous phase's own alpha0 (here 0.0) and derives it from the
    /// constraint.
    /// Result (2026-07-23): α_d + α_c = 1.0 in all 4 cells (err < 1e-15),
    /// α_c = 0.7. PASS.
    #[test]
    fn alpha_constraint_holds_on_construction() {
        let sys = air_water(line_mesh_4(), 0.3);
        for c in 0..4 {
            let ad = sys.dispersed.alpha().internal[c];
            let ac = sys.continuous.alpha().internal[c];
            assert_relative_eq!(ac, 0.7, max_relative = 1e-12);
            assert_relative_eq!(ad + ac, 1.0, max_relative = 1e-12);
        }
    }

    /// Nu accessor: ν_c = μ_c/ρ_c = 1e-3/1000 = 1e-6 m²/s.
    /// Result (2026-07-23): ν_c = 1e-6 m²/s (err < 1e-15). PASS.
    #[test]
    fn kinematic_viscosity_is_mu_over_rho() {
        let sys = air_water(one_cell_mesh(), 0.2);
        let nu_c = sys.continuous.nu().get::<square_meter_per_second>();
        assert_relative_eq!(nu_c, 1e-6, max_relative = 1e-12);
    }

    // ── Reynolds number vs hand computation ──────────────────────────────────

    /// Methodology: slip Reynolds number Re = |U_d − U_c|·d/ν_c. Set
    /// U_d = (0.1,0,0), U_c = 0, d = 1e-3 m, ν_c = 1e-6 m²/s →
    /// Re = 0.1·1e-3/1e-6 = 100 exactly. Pass: rel err < 1e-12.
    /// Result (2026-07-23): Re = 100.0 (err < 1e-13). PASS.
    #[test]
    fn reynolds_number_hand_computed() {
        let mut sys = air_water(one_cell_mesh(), 0.2);
        *sys.dispersed.u_mut() = VolVectorField::uniform("U", sys.mesh.clone(), vx(0.1));
        let re = sys.reynolds_number().unwrap();
        assert_relative_eq!(re.internal[0], 100.0, max_relative = 1e-12);
    }

    /// Zero continuous viscosity → Reynolds number undefined → error.
    #[test]
    fn reynolds_number_rejects_zero_nu() {
        let mesh = one_cell_mesh();
        let disp = Phase::new(mesh.clone(), "air", rho(1.2), mu(1.8e-5), dia(1e-3), 0.2).unwrap();
        let cont = Phase::new(mesh.clone(), "water", rho(1000.0), mu(0.0), dia(1e-3), 0.0).unwrap();
        let sys = TwoFluidSystem::new(disp, cont).unwrap();
        assert!(sys.reynolds_number().is_err());
    }

    // ── Schiller-Naumann drag vs hand computation ────────────────────────────

    /// Methodology (Schiller-Naumann, Re = 100 case): with ρ_c = 1000 kg/m³,
    /// ν_c = 1e-6 m²/s, d = 1e-3 m, U_d = (0.1,0,0), U_c = 0, α_d = 0.2:
    ///   Re    = 0.1·1e-3/1e-6 = 100  (< 1000, viscous branch).
    ///   CdRe  = 24·(1 + 0.15·100^0.687) = 24·(1 + 0.15·23.6591) = 109.1729.
    ///   Ki    = 0.75·CdRe·ρ_c·ν_c/d² = 0.75·109.1729·1000·1e-6/1e-6
    ///         = 0.75·109.1729·1000 = 81879.68 kg/(m³·s).
    ///   K_d   = α_d·Ki = 0.2·81879.68 = 16375.94 kg/(m³·s).
    /// Pass: computed CdRe within 1e-4 of 109.1729 and K_d within 1e-4 of
    /// 16375.94 (independent hand values).
    /// Result (2026-07-23): CdRe = 109.1729, K_d = 16375.94 kg/(m³·s)
    /// (rel err < 1e-5). PASS.
    #[test]
    fn schiller_naumann_drag_hand_computed() {
        let mut sys = air_water(one_cell_mesh(), 0.2);
        *sys.dispersed.u_mut() = VolVectorField::uniform("U", sys.mesh.clone(), vx(0.1));

        // Independent hand values.
        let cd_re_hand = 24.0 * (1.0 + 0.15 * 100.0_f64.powf(0.687));
        assert_relative_eq!(cd_re_hand, 109.1729, max_relative = 1e-4);
        let k_d_hand = 0.2 * 0.75 * cd_re_hand * 1000.0 * 1e-6 / (1e-3 * 1e-3);
        assert_relative_eq!(k_d_hand, 16375.94, max_relative = 1e-4);

        let k_d = DragModel::SchillerNaumann.k_d(&sys).unwrap();
        assert_relative_eq!(k_d.internal[0], k_d_hand, max_relative = 1e-9);
        assert_relative_eq!(k_d.internal[0], 16375.94, max_relative = 1e-4);
    }

    /// Methodology (Schiller-Naumann Newton branch, Re > 1000): U_d = (2,0,0),
    /// d = 1e-3, ν_c = 1e-6 → Re = 2000 (≥ 1000), CdRe = 0.44·2000 = 880.
    /// Ki = 0.75·880·1000·1e-6/1e-6 = 660000; K_d = 0.2·660000 = 132000 kg/(m³·s).
    /// Pass: rel err < 1e-9 of the hand value 132000.
    /// Result (2026-07-23): K_d = 132000 kg/(m³·s) (err < 1e-10). PASS.
    #[test]
    fn schiller_naumann_newton_branch() {
        let mut sys = air_water(one_cell_mesh(), 0.2);
        *sys.dispersed.u_mut() = VolVectorField::uniform("U", sys.mesh.clone(), vx(2.0));
        let k_d = DragModel::SchillerNaumann.k_d(&sys).unwrap();
        assert_relative_eq!(k_d.internal[0], 132000.0, max_relative = 1e-9);
    }

    /// Methodology (Wen-Yu reduces to Schiller-Naumann as α_c → 1): with the
    /// dilute limit α_d → α_res (α_c ≈ 1), Wen-Yu CdRe (Res = α_c·Re,
    /// ×α_c^{−2.65}) collapses onto the Schiller-Naumann CdRe. Compare the two
    /// models' K_d in a very dilute system (α_d = 1e-9) at Re = 100.
    /// Pass: |K_d(WenYu) − K_d(SN)| / K_d(SN) < 1e-6.
    /// Result (2026-07-23): rel diff ≈ 2.4e-9. PASS.
    #[test]
    fn wen_yu_reduces_to_schiller_naumann_dilute() {
        let mut sys = air_water(one_cell_mesh(), 1e-9);
        *sys.dispersed.u_mut() = VolVectorField::uniform("U", sys.mesh.clone(), vx(0.1));
        let k_sn = DragModel::SchillerNaumann.k_d(&sys).unwrap().internal[0];
        let k_wy = DragModel::WenYu.k_d(&sys).unwrap().internal[0];
        assert_relative_eq!(k_wy, k_sn, max_relative = 1e-6);
    }

    /// Methodology (Wen-Yu voidage amplification): at finite loading α_c < 1 the
    /// α_c^{−2.65} factor (α_c^{−2.65} > 1) outweighs the Res = α_c·Re
    /// reduction, so Wen-Yu Ki exceeds Schiller-Naumann Ki. Hand check at Re=100,
    /// α_c = 0.8: Res = 80, CdsRes = 24·(1+0.15·80^0.687) = 97.068,
    /// ×0.8^{−2.65} = ×1.8063 → CdRe = 175.33 (> SN 109.17). Compare per-Ki
    /// (strip the shared α_d factor by using equal α_d).
    /// Pass: K_d(WenYu) > K_d(SN) at α_d = 0.2 (α_c = 0.8).
    /// Result (2026-07-23): K_d(WenYu) = 26301, K_d(SN) = 16376 kg/(m³·s);
    /// WenYu/SN = 1.606. PASS.
    #[test]
    fn wen_yu_amplifies_at_finite_loading() {
        let mut sys = air_water(one_cell_mesh(), 0.2); // α_c = 0.8
        *sys.dispersed.u_mut() = VolVectorField::uniform("U", sys.mesh.clone(), vx(0.1));
        let k_sn = DragModel::SchillerNaumann.k_d(&sys).unwrap().internal[0];
        let k_wy = DragModel::WenYu.k_d(&sys).unwrap().internal[0];
        assert!(
            k_wy > k_sn,
            "WenYu {k_wy} should exceed SN {k_sn} at α_c=0.8"
        );
        // Hand-computed Wen-Yu CdRe = 175.33 → K_d = 0.2·0.75·175.33·1e9·1e-6/… :
        let cd_re_wy = 24.0 * (1.0 + 0.15 * (0.8 * 100.0_f64).powf(0.687)) * 0.8_f64.powf(-2.65);
        let k_d_hand = 0.2 * 0.75 * cd_re_wy * 1000.0 * 1e-6 / (1e-3 * 1e-3);
        assert_relative_eq!(k_wy, k_d_hand, max_relative = 1e-9);
    }

    /// Constant drag returns the prescribed uniform coefficient; negative errors.
    /// Result (2026-07-23): K_d = 500 in every cell; k_d = −1 → Err. PASS.
    #[test]
    fn constant_drag_model() {
        let sys = air_water(line_mesh_4(), 0.3);
        let k_d = DragModel::Constant { k_d: 500.0 }.k_d(&sys).unwrap();
        for c in 0..4 {
            assert_relative_eq!(k_d.internal[c], 500.0, max_relative = 1e-12);
        }
        assert!(DragModel::Constant { k_d: -1.0 }.k_d(&sys).is_err());
    }

    /// InterfacialForce: Drag dispatches to the drag coefficient; every scaffold
    /// variant returns NotImplemented (not a fabricated value).
    /// Result (2026-07-23): Drag → Ok; Lift/VirtualMass/WallLubrication/
    /// TurbulentDispersion → Err(NotImplemented). PASS.
    #[test]
    fn interfacial_force_scaffolds_are_not_implemented() {
        let sys = air_water(one_cell_mesh(), 0.2);
        assert!(InterfacialForce::Drag(DragModel::Constant { k_d: 1.0 })
            .momentum_coefficient(&sys)
            .is_ok());
        for f in [
            InterfacialForce::Lift,
            InterfacialForce::VirtualMass,
            InterfacialForce::WallLubrication,
            InterfacialForce::TurbulentDispersion,
        ] {
            assert!(matches!(
                f.momentum_coefficient(&sys),
                Err(MultiphaseError::NotImplemented(_))
            ));
        }
    }

    // ── Phase continuity (dispersed α transport) ─────────────────────────────

    /// Methodology (consistency): with zero dispersed velocity, ∂α_d/∂t = 0, so
    /// α_d is unchanged and the constraint α_c = 1 − α_d still holds. α_d = 0.3
    /// uniform on the 4-cell line, U_d = 0. Pass: every α_d stays 0.3 and
    /// α_d + α_c = 1 within 1e-12.
    /// Result (2026-07-23): α_d = [0.3,0.3,0.3,0.3] unchanged, α_d+α_c=1
    /// (err < 1e-13). PASS.
    #[test]
    fn dispersed_alpha_unchanged_with_no_flow() {
        let mut sys = air_water(line_mesh_4(), 0.3);
        sys.advance_dispersed_alpha(0.5).unwrap();
        for c in 0..4 {
            assert_relative_eq!(sys.dispersed.alpha().internal[c], 0.3, max_relative = 1e-12);
            let s = sys.dispersed.alpha().internal[c] + sys.continuous.alpha().internal[c];
            assert_relative_eq!(s, 1.0, max_relative = 1e-12);
        }
    }

    /// Methodology (advection + boundedness): impose a uniform +x dispersed
    /// velocity (U_d = 1 m/s, so φ_d = +1 internal, −1 at the inlet, +1 at the
    /// outlet) with a saturated inlet α_d = 1 feeding an initially empty domain
    /// (α_d = 0). Courant U·dt/dx = 0.2. After 5 steps the dispersed phase must
    /// advect downstream: α_d monotonically non-increasing with x, cell 0 risen
    /// above 0, every cell in [0,1], and the constraint α_d + α_c = 1 held.
    /// Result (2026-07-23): α_d = [0.5981, 0.2632, 0.0958, 0.0307] (5 steps);
    /// monotone-decreasing, cell0 > 0, all ∈ [0,1], α_d+α_c=1. PASS.
    #[test]
    fn dispersed_alpha_advects_downstream_and_stays_bounded() {
        let mesh = line_mesh_4();
        let mut sys = air_water(mesh.clone(), 0.0);
        // Saturated inlet: FixedValue α_d = 1 on the "inlet" patch (index 0).
        sys.dispersed.alpha_mut().boundary[0] = PatchField::fixed_value(1, 1.0);
        *sys.dispersed.u_mut() = VolVectorField::uniform("U", mesh.clone(), vx(1.0));
        for _ in 0..5 {
            sys.advance_dispersed_alpha(0.2).unwrap();
        }
        let a: Vec<f64> = (0..4).map(|c| sys.dispersed.alpha().internal[c]).collect();
        for (c, &v) in a.iter().enumerate() {
            assert!(
                (0.0..=1.0).contains(&v),
                "cell {c} alpha_d {v} out of [0,1]"
            );
            let s = v + sys.continuous.alpha().internal[c];
            assert_relative_eq!(s, 1.0, max_relative = 1e-12);
        }
        assert!(a[0] > 0.0, "upstream cell should have filled");
        for c in 0..3 {
            assert!(
                a[c] >= a[c + 1] - 1e-12,
                "alpha_d must be monotone non-increasing downstream: {a:?}"
            );
        }
    }

    /// Methodology (bounding stress test): a large dispersed velocity with a
    /// large dt that would drive α_d past 1 in the assembled solve; the
    /// post-solve clamp must keep every cell in [0,1] and the constraint exact.
    /// α_d = 0.5 uniform, U_d = 100 m/s, saturated inlet, dt = 10.
    /// Result (2026-07-23): all α_d ∈ [0,1], finite, α_d+α_c=1. PASS.
    #[test]
    fn dispersed_alpha_clamped_under_extreme_advection() {
        let mesh = line_mesh_4();
        let mut sys = air_water(mesh.clone(), 0.5);
        sys.dispersed.alpha_mut().boundary[0] = PatchField::fixed_value(1, 1.0);
        *sys.dispersed.u_mut() = VolVectorField::uniform("U", mesh.clone(), vx(100.0));
        sys.advance_dispersed_alpha(10.0).unwrap();
        for c in 0..4 {
            let v = sys.dispersed.alpha().internal[c];
            assert!(v.is_finite() && (0.0..=1.0).contains(&v), "cell {c}: {v}");
            let s = v + sys.continuous.alpha().internal[c];
            assert_relative_eq!(s, 1.0, max_relative = 1e-12);
        }
    }

    /// dt ≤ 0 is rejected.
    #[test]
    fn advance_rejects_nonpositive_dt() {
        let mut sys = air_water(line_mesh_4(), 0.3);
        assert!(sys.advance_dispersed_alpha(0.0).is_err());
        assert!(sys.advance_dispersed_alpha(-1.0).is_err());
    }
}
