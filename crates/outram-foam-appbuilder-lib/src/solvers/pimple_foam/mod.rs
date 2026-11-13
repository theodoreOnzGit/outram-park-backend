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

//! # pimpleFoam / icoFoam — incompressible PISO/PIMPLE solver
//!
//! Rust port of OpenFOAM's incompressible PISO/PIMPLE algorithm. With
//! `nOuterCorrectors = 1` (no SIMPLE outer loop) pimpleFoam reduces to **pure
//! PISO**, i.e. it is structurally identical to **icoFoam**; the lid-driven
//! cavity tutorial (`tutorials/pimple_foam_cavity.rs`) validates this port
//! against icoFoam-generated reference fields.
//!
//! ## Kinematic pressure and the pressure reference (closed-domain note)
//!
//! Like icoFoam and incompressible pimpleFoam, `p` here is **kinematic
//! pressure** `p/ρ` with units m²/s², *not* Pa. The momentum equation carries
//! `−∇p` (kinematic) directly, and density never appears.
//!
//! Both OpenFOAM solvers pin a pressure **reference cell** for a closed domain.
//! The cavity has walls on every boundary, so the pressure boundary condition is
//! zero-gradient everywhere → the pressure Poisson equation is pure-Neumann and
//! singular (its solution is unique only up to an additive constant, and the net
//! boundary flux must vanish for a solution to exist at all). OpenFOAM fixes the
//! constant with `setRefCell`:
//!
//! ```text
//! // icoFoam/createFields.H
//! label  pRefCell  = 0;
//! scalar pRefValue = 0.0;
//! setRefCell(p, mesh.solutionDict().subDict("PISO"), pRefCell, pRefValue);
//! //   ... later, in the pressure corrector:
//! pEqn.setReference(pRefCell, pRefValue);
//!
//! // pimpleFoam/createFields.H — IDENTICAL mechanism
//! setRefCell(p, pimple.dict(), pRefCell, pRefValue);
//! ```
//!
//! So yes — pimpleFoam does exactly what icoFoam does here. This port mirrors it
//! with `p_eqn.set_reference(0, 0.0)` in the inner corrector loop.
//!
//! ## Original OpenFOAM source (icoFoam.C — the clean PISO reference)
//!
//! ```text
//! // Momentum predictor
//! fvVectorMatrix UEqn
//! (
//!     fvm::ddt(U)
//!   + fvm::div(phi, U)
//!   - fvm::laplacian(nu, U)        // NOTE the minus sign — see below
//! );
//! if (piso.momentumPredictor())
//! {
//!     solve(UEqn == -fvc::grad(p));
//! }
//!
//! // --- PISO loop
//! while (piso.correct())   // (5) the WHOLE block re-runs each corrector,
//! {                        //     re-evaluating UEqn.H() from the latest U
//!     volScalarField rAU(1.0/UEqn.A());
//!     volVectorField HbyA(constrainHbyA(rAU*UEqn.H(), U, p));     // (3)
//!     surfaceScalarField phiHbyA
//!     (
//!         "phiHbyA",
//!         fvc::flux(HbyA)
//!       + fvc::interpolate(rAU)*fvc::ddtCorr(U, phi)              // ddtCorr
//!     );
//!     adjustPhi(phiHbyA, U, p);                                  // (3)
//!     constrainPressure(p, U, phiHbyA, rAU);
//!     while (piso.correctNonOrthogonal())
//!     {
//!         fvScalarMatrix pEqn
//!         (
//!             fvm::laplacian(rAU, p) == fvc::div(phiHbyA)         // (2)
//!         );
//!         pEqn.setReference(pRefCell, pRefValue);
//!         pEqn.solve(...);                                       // (4) GAMG/PCG
//!         if (piso.finalNonOrthogonalIter())
//!             phi = phiHbyA - pEqn.flux();
//!     }
//!     U = HbyA - rAU*fvc::grad(p);
//!     U.correctBoundaryConditions();                            // (1)
//! }
//! ```
//!
//! ## How this port differs from the original, and why
//!
//! **Root cause of the sign flips (changes 0a/0b): `outram-foam-basic-lib`'s
//! `fvm::laplacian` uses the *opposite* diagonal-sign convention to OpenFOAM's.**
//! OpenFOAM assembles `fvm::laplacian(Γ, φ)` with a *negative* diagonal
//! (`diag = −Σcoeff`), i.e. the matrix represents `+∇·(Γ∇φ)` exactly as written
//! in the equation. This port assembles it *positive-definite*
//! (`diag = +Σcoeff`), i.e. its matrix represents `−∇·(Γ∇φ)`. So the port's
//! Laplacian matrix is the **negation** of OpenFOAM's. Every sign change below
//! follows from this one fact; the discretised physics is identical.
//!
//! 0a. **Momentum viscous term: `+ fvm::laplacian_vec` (OpenFOAM: `−`).**
//!     The momentum LHS viscous term is `−ν∇²U = −∇·(ν∇U)`. OpenFOAM writes it as
//!     `− fvm::laplacian(nu, U)` because its Laplacian matrix is `+∇·(ν∇U)`.
//!     This port's Laplacian is already `−∇·(ν∇U)`, so it is **added**.
//!     Subtracting it (copying OpenFOAM's sign literally) negates the diffusion
//!     diagonal: the matrix diagonal goes negative (V/dt − Σcoeff < 0), `rAU =
//!     V/A` explodes to ~1e23, and the very first solve produces ~1e130. This
//!     was the first bug found.
//!
//! 0b. **Pressure source: negated divergence (OpenFOAM: `== fvc::div(phiHbyA)`).**
//!     OpenFOAM solves `fvm::laplacian(rAU, p) == fvc::div(phiHbyA)` with its
//!     negative-diagonal Laplacian `L_OF`. This port's Laplacian is `L = −L_OF`,
//!     so the *same* equation is `L·p = −div(phiHbyA)`. Equivalently: with the
//!     positive-definite operator the discrete divergence of the corrector flux
//!     `−rAUf·snGrad(p)` is `−(L·p)`, and zeroing the corrected divergence
//!     requires `L·p = −div(phiHbyA)`. Using `+div` flips the sign of `p`, so the
//!     corrector pumps divergence *in* and the run blows up over a few steps.
//!
//! 1. **`correct_bcs` / `correct_bcs_vec` (OpenFOAM: `U.correctBoundaryConditions()`).**
//!    OpenFOAM fields carry their boundary-condition objects, so re-evaluating
//!    them is a method call. In this port `solve()` and field arithmetic rebuild
//!    output fields with *zero-gradient* boundaries — the prescribed BC *type*
//!    (e.g. the moving-wall lid) is lost. The BC template is therefore captured
//!    at the top of each step and re-applied after every field update, exactly
//!    where OpenFOAM calls `correctBoundaryConditions()`.
//!
//! 2. **Pressure reference** — `p_eqn.set_reference(0, 0.0)` = `pEqn.setReference(
//!    pRefCell, pRefValue)` (see the closed-domain note above). Unchanged in
//!    intent from OpenFOAM.
//!
//! 3. **constrainHbyA / adjustPhi (boundary flux of phiHbyA).**
//!    OpenFOAM wraps `HbyA` in `constrainHbyA(...)` and calls `adjustPhi(...)` so
//!    that on fixed-velocity walls the boundary flux of `phiHbyA` is the
//!    *prescribed* `U_BC·Sf` (= 0 through a no-penetration wall). This port
//!    originally took `fvc::flux` of the zero-gradient `HbyA` extrapolation,
//!    which leaks a spurious flux through the walls, breaks the closed-domain
//!    compatibility condition `Σ source = 0`, and makes the pinned Poisson solve
//!    ramp the pressure ~6× every step. The fix sets the boundary flux to
//!    `U_BC·Sf` — the constrainHbyA equivalent for this BC set.
//!
//! 4. **Pressure linear solver: PCG (`solve_cg`), not Gauss-Seidel.**
//!    OpenFOAM solves the pressure with GAMG/PCG (chosen in `fvSolution`); it
//!    would never use Gauss-Seidel on a Poisson system. This port's
//!    `FvMatrix::solve` defaults to Gauss-Seidel, which needed ~22 000 iterations
//!    (and often did not converge within the cap) on the 400-cell cavity. The
//!    pressure matrix is symmetric SPD, so it is solved with `solve_cg` (PCG)
//!    instead — ~130 iterations, ~170× faster. A purely-performance change, but
//!    a *correctness* one in practice: an under-solved pEqn leaves residual
//!    divergence that accumulates and destabilises the run.
//!
//! 5. **PISO corrector loop — `H(U)` re-evaluated every pass (the stability fix).**
//!    OpenFOAM's `while (piso.correct())` re-runs the *entire* `rAU`/`HbyA`/
//!    `phiHbyA`/`pEqn`/`U`-update sequence each corrector, so `UEqn.H()` is
//!    recomputed from the velocity updated by the previous corrector — that is
//!    the iteration that converges the pressure–velocity coupling. An earlier
//!    version of this port computed `HbyA` once and merely re-solved the *same*
//!    pressure system `nCorrectors` times (updating neither `H(U)` nor `U`
//!    between passes), which collapses to a single corrector and capped stability
//!    at Co ≈ 0.1. With the loop restructured to match OpenFOAM, the cavity is
//!    stable at icoFoam's `dt = 5e-3` (Co ≈ 0.85).
//!
//! 6. **`fvc::ddtCorr` Rhie–Chow flux correction — now included.**
//!    `phiHbyA += fvc::interpolate(rAU)*fvc::ddtCorr(U, phi)`, with OpenFOAM's
//!    `fvcDdtPhiCoeff` limiter (`coeff = 1 − min(|phiCorr|/(|phi|+SMALL), 1)`).
//!    It couples the face flux to its own old-time value to suppress
//!    pressure–velocity (checkerboard) decoupling. `ddtCorr` uses the time-old
//!    `U`/`phi` (constant across the inner correctors). See
//!    `outram_foam_basic_lib::fv_operators::fvc::ddt_corr`.
//!    **Since 2026-11-01 (gh:#1067)** the form follows `schemes.ddt` through
//!    `crate::solvers::schemes::ddt_corr_scheme`: the Euler form for `Euler`,
//!    the BDF2 form (`fvc::ddt_corr_backward`, a port of
//!    `backwardDdtScheme::fvcDdtPhiCorr`) for `Backward`, zero for
//!    `steadyState` — previously the Euler form was used for all three.
//!
//! 7. **Open boundaries — ported 2026-11-03 (gh:#1154).** Until then only
//!    closed cavities had been run. Three upstream behaviours were missing:
//!    `setReference` is applied only when `p.needReference()` (no patch fixes
//!    p); `constrainHbyA` sets phiHbyA's — and so `phi`'s — boundary flux to
//!    `U_b·Sf` on fixed-velocity patches, while a zeroGradient outlet keeps
//!    `HbyA_P·Sf` (both feed the pressure source, as `fvc::div(phiHbyA)` does
//!    upstream); and `phi = phiHbyA − pEqn.flux()` includes the boundary
//!    pressure flux of fixed-p patches. Pinned by
//!    `tests/pimple_foam_open_channel.rs` (plane Poiseuille). On the closed
//!    cavity only the second acts (the lid's flux is now exactly 0); it moved
//!    the cavity toward icoFoam, max |ΔU| / U_lid 0.0144 → 0.0062.
//!
//! 8. **Non-orthogonal correction — ported 2026-11-03 (gh:#1161).** Until
//!    then both Laplacians divided by `|d|` and had no explicit term (OpenFOAM
//!    `Gauss linear orthogonal`), whatever `schemes.default_laplacian` said.
//!    Now `default_laplacian` is honoured, and its default `Gauss linear
//!    corrected` does what upstream does: `1/(n·d)` implicit coefficients, an
//!    explicit `k·∇φ` face correction on internal faces (lagged, so it is
//!    applied even with `nNonOrthogonalCorrectors = 0`, upstream's default),
//!    the same correction subtracted in `phi = phiHbyA − pEqn.flux()`, and the
//!    `nNonOrthogonalCorrectors` loop. Details and upstream line references:
//!    [`crate::solvers::non_orthogonal`]. Ablation:
//!    [`PimpleFoam::without_non_orthogonal_correction`]. On a Cartesian mesh
//!    the correction vectors are zero, so the cavity and channel results move
//!    by round-off only; V&V record:
//!    `verification_and_validation/non_orthogonal_correction_gh1161.md`.

use crate::error::AppBuilderError;
use crate::io::control_dict::{ControlDict, StartControl, StopControl};
use crate::io::fv_schemes::FvSchemes;
use crate::io::fv_solution::FvSolution;
use crate::solvers::energy::{EnergyEquation, EnergySolve};
use crate::solvers::non_orthogonal::{self, LaplacianCorrection};
use crate::solvers::schemes::DdtCorrForm;
use crate::turbulence::TurbulenceClosure;
use outram_foam_basic_lib::prelude::*;
use std::sync::Arc;

/// Re-apply a scalar boundary-condition template to a field after a solve has
/// rebuilt it with zero-gradient boundaries (OpenFOAM `correctBoundaryConditions`).
/// For `FixedValue` the boundary face values are reset to the fixed value; other
/// BC types have their face values recomputed by the operators (via `interpolate`).
fn correct_bcs(field: &mut VolScalarField, bcs: &[BoundaryCondition<f64>]) {
    for (pf, bc) in field.boundary.iter_mut().zip(bcs) {
        pf.bc = bc.clone();
        if let BoundaryCondition::FixedValue(v) = bc {
            for x in pf.values.iter_mut() {
                *x = *v;
            }
        }
    }
}

/// Vector counterpart of [`correct_bcs`].
fn correct_bcs_vec(field: &mut VolVectorField, bcs: &[BoundaryCondition<Vector3>]) {
    for (pf, bc) in field.boundary.iter_mut().zip(bcs) {
        pf.bc = bc.clone();
        if let BoundaryCondition::FixedValue(v) = bc {
            for x in pf.values.iter_mut() {
                *x = *v;
            }
        }
    }
}

/// OpenFOAM `constrainHbyA` applied to the boundary flux of `phiHbyA`
/// (gh:#1154): on every patch whose velocity BC fixes the value
/// ([`BoundaryCondition::is_fixed_value`]) the boundary flux becomes the
/// prescribed `U_b·Sf` \[m³/s\]. Other patches (a zeroGradient outlet) keep
/// `fvc::flux(HbyA)`'s `HbyA_P·Sf`, as upstream does. Empty and cyclic patches
/// are left alone.
fn constrain_hbya_boundary_flux(
    phi_hbya: &mut SurfaceScalarField,
    u: &VolVectorField,
    u_bcs: &[BoundaryCondition<Vector3>],
    mesh: &FvMesh,
) {
    for (pi, patch) in mesh.patches.iter().enumerate() {
        if matches!(patch.kind, PatchKind::Cyclic | PatchKind::CyclicAmi | PatchKind::Empty)
            || matches!(u_bcs[pi], BoundaryCondition::Empty)
            || !u_bcs[pi].is_fixed_value()
        {
            continue;
        }
        for fi in 0..patch.size {
            phi_hbya.boundary[pi].values[fi] =
                u.boundary[pi].values[fi].dot(mesh.face_area_vectors[patch.start + fi]);
        }
    }
}

/// The boundary part of OpenFOAM's `phi = phiHbyA - pEqn.flux()` (gh:#1154):
/// add, on each boundary face, the outward pressure-correction flux that
/// `fvm::laplacian(rAUf, p)` put into the owner's matrix row \[m³/s\]. It mirrors
/// that operator's boundary arms (the same `coeff = rAUf_b·|Sf|/δ_b`, with
/// δ_b from `delta_coeff`: `|d|` orthogonal, `n·d` otherwise): `coeff·(p_P − p_b)` on a fixed-value p patch,
/// `−coeff·|d|·g` on a fixed-gradient one, the Robin blend of the two on a
/// mixed one, and nothing on zeroGradient patches, so a closed domain is
/// unchanged.
fn add_fixed_pressure_boundary_flux(
    phi: &mut SurfaceScalarField,
    p: &VolScalarField,
    rauf: &SurfaceScalarField,
    mesh: &FvMesh,
    delta_coeff: fvm::DeltaCoeff,
) {
    for (pi, patch) in mesh.patches.iter().enumerate() {
        if matches!(patch.kind, PatchKind::Cyclic | PatchKind::CyclicAmi | PatchKind::Empty) {
            continue;
        }
        for fi in 0..patch.size {
            let gf = patch.start + fi;
            let o = mesh.owner[gf];
            // The same boundary distance the pressure matrix divided by.
            let d = delta_coeff.boundary_delta(mesh, gf);
            if d < 1e-300 {
                continue;
            }
            let coeff = rauf.boundary[pi].values[fi] * mesh.face_areas[gf] / d;
            let pp = p.internal[o];
            let flux = match &p.boundary[pi].bc {
                BoundaryCondition::FixedValue(v) => coeff * (pp - v),
                BoundaryCondition::FixedField(ff) => coeff * (pp - ff[fi]),
                BoundaryCondition::NoSlip => coeff * pp,
                BoundaryCondition::TotalPressure { .. } => coeff * (pp - p.boundary[pi].values[fi]),
                BoundaryCondition::FixedGradient(g) => -coeff * d * g,
                BoundaryCondition::FixedFluxPressure { gradient } => -coeff * d * gradient,
                BoundaryCondition::Mixed { value_fraction, ref_value, ref_grad } => {
                    value_fraction * coeff * (pp - ref_value) - (1.0 - value_fraction) * coeff * d * ref_grad
                }
                BoundaryCondition::MixedField { value_fraction, ref_value, ref_grad } => {
                    let w = value_fraction[fi];
                    w * coeff * (pp - ref_value[fi]) - (1.0 - w) * coeff * d * ref_grad[fi]
                }
                _ => 0.0,
            };
            phi.boundary[pi].values[fi] += flux;
        }
    }
}

/// Linear solver used for the pressure Poisson equation.
///
/// The pressure equation is symmetric SPD and elliptic. Both options warm-start
/// from the previous time step's pressure field:
///
/// - [`Pcg`](PressureSolver::Pcg) — DIC-preconditioned conjugate gradient.
///   Iteration count grows with the mesh (∝ √κ), but each iteration is cheap.
/// - [`Gamg`](PressureSolver::Gamg) — algebraic multigrid. Near
///   mesh-independent (a few V-cycles); the better choice on fine meshes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PressureSolver {
    /// DIC-preconditioned conjugate gradient (`FvMatrix::solve_cg_with_guess`).
    #[default]
    Pcg,
    /// Algebraic multigrid (`FvMatrix::solve_gamg_with_guess`).
    Gamg,
}

/// Incompressible transient PIMPLE/PISO solver.
///
/// Solves:
///   ∂U/∂t + ∇·(UU) − ν∇²U = −∇p    (p here is kinematic: p/ρ, units m²/s²)
///   ∇·U = 0
///
/// Outer PIMPLE loop → momentum predictor → inner PISO pressure correctors.
///
/// C++ solver: `applications/solvers/incompressible/pimpleFoam/` (and the
/// equivalent `icoFoam` for `nOuterCorrectors = 1`).
///
/// See the **module-level documentation** for the kinematic-pressure /
/// pressure-reference discussion, the original OpenFOAM source, and a
/// point-by-point justification of where (and why) this port's signs and
/// solver choices differ from the C++ original.
#[derive(Debug)]
pub struct PimpleFoam {
    pub mesh: Arc<FvMesh>,
    pub control: ControlDict,
    pub schemes: FvSchemes,
    pub solution: FvSolution,
    /// Velocity field [m/s]
    pub u: VolVectorField,
    /// Kinematic pressure field p/ρ [m²/s²]
    pub p: VolScalarField,
    /// Face volumetric flux φ = U·Sf [m³/s]
    pub phi: SurfaceScalarField,
    /// Kinematic viscosity ν [m²/s]
    pub nu: VolScalarField,
    /// Linear solver for the pressure Poisson equation (default: PCG).
    pub pressure_solver: PressureSolver,
    /// Velocity at the time level *before* `u_old`, i.e. U^{n−2} [m/s].
    ///
    /// Maintained automatically by [`PimpleFoam::step`] and needed only by the
    /// second-order [`crate::io::fv_schemes::DdtScheme::Backward`] time scheme.
    /// `None` before the first step has completed, where backward differencing
    /// degenerates to Euler.
    pub u_old_old: Option<VolVectorField>,
    /// Face flux at the time level *before* the step's old flux, i.e. φ^{n−2}
    /// [m³/s].
    ///
    /// Maintained automatically by [`PimpleFoam::step`], alongside
    /// [`Self::u_old_old`]; read only by the BDF2 Rhie–Chow correction
    /// (`fvc::ddt_corr_backward`) when `schemes.ddt` is
    /// [`crate::io::fv_schemes::DdtScheme::Backward`]. `None` before the first
    /// step has completed.
    pub phi_old_old: Option<SurfaceScalarField>,
    /// Which `ddtCorr` form is paired with the ddt scheme (default
    /// [`DdtCorrForm::MatchDdtScheme`], the correct physics). Setting
    /// [`DdtCorrForm::EulerOnly`] is an explicit ablation that reproduces the
    /// pre-gh:#1067 Euler-only correction; see
    /// `verification_and_validation/bdf2_temporal_order.md` for what it costs.
    pub ddt_corr_form: DdtCorrForm,
    /// Momentum (U) linear-solver settings: tolerance 1e-7, 1000 iterations
    /// (`SolverSettings::default()`), what this solver has always used. See
    /// [`Self::with_linear_solver_settings`].
    pub u_linear_solver: SolverSettings,
    /// Pressure (p) linear-solver settings: tolerance 1e-8, 2000 iterations,
    /// what this solver has always used. See [`Self::with_linear_solver_settings`].
    pub p_linear_solver: SolverSettings,
    /// Turbulence closure (default [`TurbulenceClosure::Laminar`]).
    ///
    /// Selecting a RAS/LES model replaces the molecular viscous term in the
    /// momentum predictor with `divDevReff(U) = −∇·(ν_eff ∇U) − ∇·(ν_eff
    /// dev2(∇Uᵀ))`, ν_eff = ν + ν_t, and advances the model's transport
    /// equations once per time step after the pressure correctors — OpenFOAM's
    /// `turbulence->correct()` position. Set it directly, e.g.
    ///
    /// ```ignore
    /// solver.turbulence = TurbulenceClosure::k_omega_sst(mesh.clone());
    /// solver.turbulence.set_k_omega_uniform(1.0e-2, 100.0); // k [m²/s²], ω [1/s]
    /// ```
    ///
    /// **The default is laminar and stays laminar unless you change it**, so an
    /// existing run is unaffected. See [`crate::turbulence`] for the honest
    /// scope limits — in particular, the closures use zero-gradient (not
    /// wall-function) near-wall boundary conditions.
    pub turbulence: TurbulenceClosure,
    /// Temperature transport for forced convection (gh:#1156), `None` by
    /// default. With `None`, [`Self::step`] is exactly [`Self::step_flow`],
    /// so every pre-gh:#1156 run is bit-identical. With `Some`, the energy
    /// equation of [`crate::solvers::energy`] is solved after the PISO loop
    /// with the corrected flux and the same Laplacian treatment (and
    /// `nNonOrthogonalCorrectors`) as the flow; the temperature is passive
    /// (constant properties, no buoyancy), so `U`, `p` and `phi` are
    /// unchanged by it.
    pub energy: Option<EnergyEquation>,
    /// Opt-in log of the initial residual of every linear solve, in upstream's
    /// normalisation (gh:#1363; see [`SolveRecord`]). `None` by default, so a
    /// run pays nothing; [`Self::with_solve_log`] switches it on.
    pub solve_log: Option<Vec<SolveRecord>>,
}

/// One linear solve as upstream's solver log prints it: the field name
/// (`"Ux"`, `"Uy"`, `"p"`) and its `Initial residual`, computed by
/// `LduMatrix::openfoam_initial_residual` on the assembled system and the
/// starting guess, i.e. before the solver touches the field.
#[derive(Debug, Clone, PartialEq)]
pub struct SolveRecord {
    /// Solved field (`"Ux"`, `"Uy"`, `"p"`).
    pub field: &'static str,
    /// Initial residual, upstream normalisation.
    pub initial_residual: f64,
}

impl PimpleFoam {
    /// Build an incompressible PIMPLE solver on `mesh`, with every field
    /// allocated at a placeholder initial state.
    ///
    /// The caller is expected to overwrite the public field members before
    /// stepping — in particular `u`, `p` and the kinematic viscosity `nu`, and
    /// to set the boundary conditions on the mesh patches. The defaults are:
    ///
    /// | Field | Initial value |
    /// |---|---|
    /// | `u` — velocity [m/s] | zero |
    /// | `p` — kinematic pressure p/ρ [m²/s²] | zero |
    /// | `phi` — volumetric face flux [m³/s] | zero |
    /// | `nu` — kinematic viscosity [m²/s] | uniform 1e-5 |
    /// | `pressure_solver` | [`PressureSolver::default`] |
    /// | `turbulence` | [`TurbulenceClosure::Laminar`] |
    ///
    /// Note this solver is **kinematic**: `p` is pressure divided by density, in
    /// m²/s², not Pa. That is OpenFOAM's incompressible convention.
    ///
    /// # Arguments
    ///
    /// * `mesh` — the finite-volume mesh, shared read-only.
    /// * `control` — time-loop settings; see [`ControlDict`] for which of its
    ///   fields are actually honoured.
    /// * `schemes` — `ddt` and `div` selections, applied via
    ///   [`crate::solvers::schemes`], and (since gh:#1161) `default_laplacian`,
    ///   applied via [`crate::solvers::non_orthogonal`]. `grad`/`snGrad` are
    ///   stored but not consulted.
    /// * `solution` — linear-solver tolerances and the PIMPLE corrector counts.
    pub fn new(
        mesh: Arc<FvMesh>,
        control: ControlDict,
        schemes: FvSchemes,
        solution: FvSolution,
    ) -> Self {
        let u = VolVectorField::zero("U", mesh.clone());
        let p = VolScalarField::zeros("p", mesh.clone());
        let phi = SurfaceScalarField::zeros("phi", mesh.clone());
        let nu = VolScalarField::uniform("nu", mesh.clone(), 1e-5);
        Self {
            mesh,
            control,
            schemes,
            solution,
            u,
            p,
            phi,
            nu,
            pressure_solver: PressureSolver::default(),
            u_old_old: None,
            phi_old_old: None,
            ddt_corr_form: DdtCorrForm::default(),
            u_linear_solver: SolverSettings::default(),
            p_linear_solver: SolverSettings { tolerance: 1e-8, max_iter: 2_000 },
            turbulence: TurbulenceClosure::default(),
            energy: None,
            solve_log: None,
        }
    }

    /// Record the initial residual of every linear solve in
    /// [`Self::solve_log`] (gh:#1363), for comparison with an upstream log.
    #[must_use]
    pub fn with_solve_log(mut self) -> Self {
        self.solve_log = Some(Vec::new());
        self
    }

    /// Append one entry to the solve log, if it is on.
    fn log_solve(&mut self, field: &'static str, ldu: &LduMatrix, b: &[f64], x: &[f64]) {
        if let Some(log) = self.solve_log.as_mut() {
            log.push(SolveRecord { field, initial_residual: ldu.openfoam_initial_residual(x, b) });
        }
    }

    /// Advance one time step: the flow ([`Self::step_flow`], skipped when
    /// `solution.pimple.frozen_flow` is set, as upstream's `frozenFlow`),
    /// then, if [`Self::energy`] is configured, the energy equation with the
    /// step's corrected flux ([`Self::solve_energy`]).
    ///
    /// # Errors
    /// Whatever the flow or the energy solve returns.
    pub fn step(&mut self) -> Result<(), AppBuilderError> {
        let t_old = self.energy.as_ref().map(|e| e.t.clone());
        if !self.solution.pimple.frozen_flow {
            self.step_flow()?;
        }
        if let Some(t_old) = t_old {
            self.solve_energy(&t_old)?;
        }
        Ok(())
    }

    /// Solve the attached energy equation once from old-time field `t_old`
    /// with the current `phi`, `control.delta_t`, the flow's Laplacian scheme
    /// and `nNonOrthogonalCorrectors`. A no-op (zero change) with no energy
    /// equation attached.
    ///
    /// # Errors
    /// [`AppBuilderError::UnsupportedScheme`] for an unimplemented energy
    /// ddt or Laplacian scheme.
    pub fn solve_energy(&mut self, t_old: &VolScalarField) -> Result<EnergySolve, AppBuilderError> {
        let lap = LaplacianCorrection::from_scheme(&self.schemes.default_laplacian)?;
        let n_non_orth = self.solution.pimple.n_non_orthogonal_correctors;
        let dt = uom::si::f64::Time::new::<uom::si::time::second>(self.control.delta_t);
        match self.energy.as_mut() {
            Some(e) => e.solve(t_old, Some(&self.phi), dt, lap, n_non_orth),
            None => Ok(EnergySolve { max_change: 0.0, iterations: 0, residual: 0.0 }),
        }
    }

    /// Ablation (gh:#1161): use OpenFOAM's `Gauss linear orthogonal` Laplacian
    /// (`1/|d|`, no explicit correction) for both the pressure and the
    /// momentum equation, which is bit-for-bit what this solver did before
    /// the non-orthogonal correction was ported. The default is `Gauss linear
    /// corrected`, as in upstream's tutorials; see
    /// `verification_and_validation/non_orthogonal_correction_gh1161.md` for
    /// what the correction is worth on a pipe O-grid.
    #[must_use]
    pub fn without_non_orthogonal_correction(mut self) -> Self {
        self.schemes.default_laplacian = crate::io::fv_schemes::LaplacianScheme::GaussLinearOrthogonal;
        self
    }

    /// Solve the momentum and pressure equations to these settings instead
    /// of the defaults (U 1e-7, p 1e-8). For a check that has to resolve
    /// differences below the default tolerances: since the momentum predictor
    /// is warm-started (gh:#1202), two runs that differ by round-off stop at
    /// different points inside the default tolerance. Upstream reads these
    /// from `fvSolution`'s `solvers`; this port does not yet
    /// (gh:#1291).
    #[must_use]
    pub fn with_linear_solver_settings(mut self, u: SolverSettings, p: SolverSettings) -> Self {
        self.u_linear_solver = u;
        self.p_linear_solver = p;
        self
    }

    /// Advance the flow (U, p, phi) by one time step using the PIMPLE algorithm.
    ///
    /// Dimensional analysis (integrated FV system):
    ///   A has units m³/s  →  rAU = V/A has units s
    ///   H has units m⁴/s²  →  HbyA = H/A has units m/s
    ///   phi_hbya = flux(HbyA) has units m³/s
    ///   Pressure source = Σ_f phi_hbya_f (NOT divided by V) ∈ m³/s
    ///   Laplacian coeff = rAUf · area/delta ∈ s·m  →  coeff·p (m²/s²) = m³/s ✓
    ///
    /// This is the whole of what `step` did before gh:#1156; [`Self::step`]
    /// calls it and then the energy equation, if any.
    pub fn step_flow(&mut self) -> Result<(), AppBuilderError> {
        let mesh = self.mesh.clone();
        let n = mesh.n_cells;
        let dt = self.control.delta_t;
        let settings = self.u_linear_solver;
        // The pressure Poisson equation is symmetric SPD and elliptic; it is
        // solved with preconditioned conjugate gradient (`solve_cg`), which
        // converges in O(√κ) iterations. An under-solved pEqn leaves residual
        // divergence that accumulates and destabilises the run, so the tolerance
        // is kept tight.
        let p_settings = self.p_linear_solver;
        let n_outer = self.solution.pimple.n_outer_correctors.max(1);
        let n_inner = self.solution.pimple.n_correctors.max(1);
        let n_non_orth = self.solution.pimple.n_non_orthogonal_correctors;
        let lap = LaplacianCorrection::from_scheme(&self.schemes.default_laplacian)?;

        let u_old = self.u.clone();
        let phi_old = self.phi.clone(); // old-time flux for the ddtCorr term

        // Capture the boundary-condition templates before any solve. The linear
        // solver and field arithmetic rebuild output fields with zero-gradient
        // boundaries, so the prescribed BCs (e.g. the moving-wall lid) must be
        // re-applied after each field update — the equivalent of OpenFOAM's
        // `U.correctBoundaryConditions()`. Capturing here each step is valid
        // because every step ends with the templates re-applied.
        let u_bcs: Vec<BoundaryCondition<Vector3>> =
            self.u.boundary.iter().map(|pf| pf.bc.clone()).collect();
        let p_bcs: Vec<BoundaryCondition<f64>> =
            self.p.boundary.iter().map(|pf| pf.bc.clone()).collect();
        // OpenFOAM `GeometricField::needReference()`: true unless some patch
        // fixes the value of p.
        let p_needs_reference = !p_bcs.iter().any(|bc| bc.is_fixed_value());

        for _ in 0..n_outer {
            // Push the live velocity / flux / viscosity / dt into the turbulence
            // closure so the stress term below is assembled from the current
            // state. A no-op when the closure is laminar.
            self.turbulence
                .sync_inputs(&self.u, &self.phi, &self.nu, dt);

            // ── Assemble implicit momentum equation (no pressure source yet) ───
            // `fvm::laplacian_vec` is assembled positive-definite (diag += coeff),
            // i.e. it represents −∇·(ν∇U). The momentum viscous term is −∇·(ν∇U),
            // so it is ADDED here (not subtracted) — subtracting would drive the
            // matrix diagonal negative and blow the solve up.
            //
            // `div_dev_reff` returns exactly `fvm::laplacian_vec(nu, U)` for the
            // default laminar closure, so a laminar run is unchanged; a RAS/LES
            // closure returns the same operator with ν_eff = ν + ν_t plus the
            // explicit transpose correction in the source.
            //
            // The ddt and div terms honour `self.schemes` (see
            // `crate::solvers::schemes`): the defaults are Euler + first-order
            // upwind, exactly what this solver assembled before scheme
            // selection was wired in, and an unimplemented selection is a hard
            // error rather than a silent fallback.
            let ddt_term = crate::solvers::schemes::ddt_vec_scheme(
                &self.schemes.ddt,
                &self.u,
                &u_old,
                self.u_old_old.as_ref(),
                dt,
                mesh.clone(),
            )?;
            let div_term = crate::solvers::schemes::div_vec_scheme(
                &self.schemes.default_div,
                self.schemes.grad_u_scheme(),
                &self.phi,
                &self.u,
                mesh.clone(),
            )?;
            let mut u_eqn = ddt_term + div_term + self.turbulence.div_dev_reff(&self.u, &self.nu);
            // Non-orthogonal treatment of the viscous Laplacian (gh:#1161,
            // upstream `fvm::laplacian(nuEff, U)` with `Gauss linear
            // corrected`): `1/(n·d)` implicit coefficients and the explicit
            // per-component `ν_f|Sf| k·∇U_j`, evaluated from the current U.
            if lap != LaplacianCorrection::Orthogonal {
                let nu_eff_f = fvc::interpolate(&self.turbulence.nu_eff(&self.nu));
                non_orthogonal::restamp_vector_delta(&mut u_eqn, &nu_eff_f, &self.u, lap.delta_coeff());
                if lap.is_explicit() {
                    // upstream `correctedSnGrad::fullGradCorrection` takes the
                    // gradient from the `grad(U)` scheme (gh:#1202); with the
                    // default `Gauss linear` this is `fvc::grad_vec` unchanged.
                    let grad_u = crate::solvers::schemes::grad_vec_scheme(self.schemes.grad_u_scheme(), &self.u)?;
                    let mut corr = non_orthogonal::vector_face_correction(&nu_eff_f, &grad_u, &mesh);
                    lap.limit_vector(&mut corr, &nu_eff_f, &self.u, &mesh);
                    non_orthogonal::add_to_source_vector(u_eqn.source.as_mut_slice(), &corr, &mesh);
                }
            }

            // A = diagonal [m³/s];  rAU = V/A [s]
            let a = u_eqn.a_field();
            let rau = {
                let a_data = a.internal.as_slice();
                let rau_vals: Vec<f64> = (0..n)
                    .map(|c| mesh.cell_volumes[c] / a_data[c].max(1e-30))
                    .collect();
                VolScalarField::new(
                    "rAU",
                    mesh.clone(),
                    Field::new(rau_vals),
                    mesh.patches
                        .iter()
                        .map(|p| PatchField::zero_gradient(p.size))
                        .collect(),
                )
            };

            // Momentum predictor: temporarily add −V·∇p to the source
            let gp = fvc::grad(&self.p);
            for c in 0..n {
                u_eqn.source[c] = u_eqn.source[c] - gp.internal[c] * mesh.cell_volumes[c];
            }
            // Warm-started from the current U, as upstream's fvMatrix::solve
            // (gh:#1202: the cold start ran out of sweeps on thin wall layers).
            if self.solve_log.is_some() {
                let bx: Vec<f64> = (0..n).map(|c| u_eqn.source[c].x).collect();
                let by: Vec<f64> = (0..n).map(|c| u_eqn.source[c].y).collect();
                let ux: Vec<f64> = (0..n).map(|c| self.u.internal[c].x).collect();
                let uy: Vec<f64> = (0..n).map(|c| self.u.internal[c].y).collect();
                self.log_solve("Ux", &u_eqn.ldu, &bx, &ux);
                self.log_solve("Uy", &u_eqn.ldu, &by, &uy);
            }
            let (mut u_pred, _) = u_eqn.solve_with_guess("U", &self.u, settings);
            correct_bcs_vec(&mut u_pred, &u_bcs);
            // Restore source (remove pressure contribution) for H(U) computation
            for c in 0..n {
                u_eqn.source[c] = u_eqn.source[c] + gp.internal[c] * mesh.cell_volumes[c];
            }
            self.u = u_pred;

            // rAUf = interpolate(rAU) at faces [s]; ddtCorr (Rhie–Chow flux
            // correction) uses the time-old U/phi and so is constant across the
            // PISO correctors below. Both are computed once per outer iteration.
            let rauf = fvc::interpolate(&rau);
            // The correction must come from the same ddt scheme as the
            // momentum term (OpenFOAM's `fvc::ddtCorr` dispatches on
            // `ddtSchemes` too); `EulerOnly` is the explicit ablation.
            let ddt_corr = match self.ddt_corr_form {
                DdtCorrForm::MatchDdtScheme => crate::solvers::schemes::ddt_corr_scheme(
                    &self.schemes.ddt,
                    &u_old,
                    self.u_old_old.as_ref(),
                    &phi_old,
                    self.phi_old_old.as_ref(),
                    dt,
                )?,
                DdtCorrForm::EulerOnly => fvc::ddt_corr(&u_old, &phi_old, dt),
            };

            // ── PISO pressure-correction loop ────────────────────────────────
            //
            // This mirrors icoFoam's `while (piso.correct())` block: each
            // corrector rebuilds HbyA = H(U)/A from the *latest* U, forms
            // phiHbyA, solves the pressure equation, then corrects both the face
            // flux and the cell velocity. Re-evaluating H(U) each pass is what
            // makes the velocity–pressure coupling converge — solving the same
            // fixed system twice (the previous structure) does not, and capped
            // stability at Co ≈ 0.1.
            for _ in 0..n_inner {
                // HbyA = H(U)/A [m/s] — H re-evaluated from the current U.
                let h = u_eqn.h_field(&self.u);
                let hbya = {
                    let h_data = h.internal.as_slice();
                    let a_data = a.internal.as_slice();
                    let vals: Vec<Vector3> = (0..n)
                        .map(|c| h_data[c] * (1.0 / a_data[c].max(1e-30)))
                        .collect();
                    VolVectorField::new(
                        "HbyA",
                        mesh.clone(),
                        Field::new(vals),
                        mesh.patches
                            .iter()
                            .map(|p| PatchField::zero_gradient_vec(p.size))
                            .collect(),
                    )
                };

                // phiHbyA = flux(HbyA) + interpolate(rAU)·ddtCorr(U, phi) [m³/s].
                // The ddtCorr term is OpenFOAM's Rhie–Chow time-derivative flux
                // correction (icoFoam pEqn.H:
                // `phiHbyA += fvc::interpolate(rAU)*fvc::ddtCorr(U, phi)`),
                // which couples the face flux to its own old-time value.
                let mut phi_hbya = fvc::flux(&hbya);
                {
                    let rauf_int = rauf.internal.as_slice();
                    let dc_int = ddt_corr.internal.as_slice();
                    for f in 0..mesh.n_internal_faces {
                        phi_hbya.internal[f] += rauf_int[f] * dc_int[f];
                    }
                }
                // constrainHbyA (OpenFOAM `constrainHbyA.C`): on a patch that
                // fixes U, HbyA's boundary value is U_b, so the boundary flux
                // of phiHbyA is the prescribed U_b·Sf (0 through a
                // no-penetration wall). On any other patch (a zeroGradient
                // outlet) it stays fvc::flux(HbyA)'s HbyA_P·Sf. See
                // `constrain_hbya_boundary_flux`.
                constrain_hbya_boundary_flux(&mut phi_hbya, &self.u, &u_bcs, &mesh);

                // Pressure source = −(net phiHbyA outflow) per cell [m³/s].
                //
                // `fvm::laplacian` is positive-definite (it represents −∇·(Γ∇)),
                // so the discrete divergence of the correction flux
                // −rAUf·snGrad(p) is −(L·p)_P. Zeroing the corrected divergence
                // (L·p = −div(phiHbyA)) requires the *negated* outflow as source;
                // +outflow flips the sign of p and makes the corrector pump
                // divergence in, blowing the solution up.
                let source_p = {
                    let mut s = vec![0.0_f64; n];
                    let phi_int = phi_hbya.internal.as_slice();
                    for f in 0..mesh.n_internal_faces {
                        s[mesh.owner[f]] -= phi_int[f];
                        s[mesh.neighbour[f]] += phi_int[f];
                    }
                    // Boundary flux is phiHbyA's boundary value, constrained
                    // above: the prescribed U_BC·Sf on fixed-velocity patches
                    // (OpenFOAM's `constrainHbyA`; the zero-gradient HbyA
                    // extrapolation there leaks spurious wall flux, breaks the
                    // closed-domain compatibility condition and ramps the
                    // pinned-Poisson pressure each step), and HbyA_P·Sf on a
                    // zeroGradient outlet, exactly the `fvc::div(phiHbyA)` of
                    // upstream. (Before gh:#1154 the U boundary *values* were
                    // used on every patch, which are 0 on a zeroGradient
                    // outlet, so an open domain lost its outflow here.)
                    for (pi, patch) in mesh.patches.iter().enumerate() {
                        if matches!(self.u.boundary[pi].bc, BoundaryCondition::Empty) {
                            continue; // 2-D front/back: no in-plane flux
                        }
                        for fi in 0..patch.size {
                            s[mesh.owner[patch.start + fi]] -= phi_hbya.boundary[pi].values[fi];
                        }
                    }
                    s
                };

                // Non-orthogonal pressure corrector loop (upstream
                // `while (pimple.correctNonOrthogonal())`, gh:#1161): with
                // `nNonOrthogonalCorrectors = 0` (the default) this is one
                // pass, and the explicit correction is still applied, lagged
                // on the `p` the pass starts from. `phi` is updated on the
                // final pass only, with the same face correction the matrix
                // got, so the corrected flux stays divergence-free.
                for i_non_orth in 0..=n_non_orth {
                    // Pressure equation: L·p = source (symmetric SPD → PCG).
                    let mut p_eqn = fvm::laplacian_with_delta(&rauf, &self.p, lap.delta_coeff());
                    let face_corr = if lap.is_explicit() {
                        let mut c = non_orthogonal::scalar_face_correction(&rauf, &fvc::grad(&self.p), &mesh);
                        lap.limit_scalar(&mut c, &rauf, &self.p, &mesh);
                        non_orthogonal::add_to_source_scalar(p_eqn.source.as_mut_slice(), &c, &mesh);
                        Some(c)
                    } else {
                        None
                    };
                    // ADD, don't OVERWRITE: `fvm::laplacian` already put any
                    // FixedValue pressure boundary's Dirichlet source term
                    // (`coeff·p_bc`) into `p_eqn.source` with its matching
                    // diagonal `coeff`. Overwriting would drop the source term
                    // but keep the diagonal, silently imposing `p_boundary = 0`
                    // and blowing up any fixed-pressure outlet. Harmless for a
                    // purely closed (all-zeroGradient-p) domain -- the laplacian
                    // source is zero there -- but correct if a FixedValue p BC
                    // is ever set.
                    for (s, &sp) in p_eqn.source.iter_mut().zip(source_p.iter()) {
                        *s += sp;
                    }
                    // Pin the reference only where upstream does: `setRefCell` and
                    // `fvMatrix::setReference` act only when `p.needReference()`,
                    // i.e. when no patch fixes p. With a fixed-p outlet, pinning
                    // cell 0 as well makes it a mass source/sink (gh:#1154).
                    if p_needs_reference {
                        // upstream `fvMatrix::setReference` (gh:#1363: was a 1e30
                        // penalty, `set_reference`)
                        p_eqn.set_reference_openfoam(0, 0.0);
                    }
                    // Warm-start from the current pressure (previous corrector /
                    // time step). Near steady state `p` barely changes, so the
                    // solve converges in a few iterations instead of from x = 0
                    // every time. PCG or GAMG per `self.pressure_solver`.
                    if self.solve_log.is_some() {
                        let b: Vec<f64> = p_eqn.source.iter().copied().collect();
                        let x: Vec<f64> = self.p.internal.iter().copied().collect();
                        self.log_solve("p", &p_eqn.ldu, &b, &x);
                    }
                    let (mut p_new, _) = match self.pressure_solver {
                        PressureSolver::Pcg => p_eqn.solve_cg_with_guess("p", &self.p, p_settings),
                        PressureSolver::Gamg => p_eqn.solve_gamg_with_guess("p", &self.p, p_settings),
                    };
                    correct_bcs(&mut p_new, &p_bcs);
                    self.p = p_new;

                    if i_non_orth < n_non_orth {
                        continue;
                    }
                    // phi = phiHbyA − pEqn.flux(): the implicit face flux
                    // `rAUf|Sf|Δ_f(p_N − p_P)` plus the stored face correction.
                    if lap == LaplacianCorrection::Orthogonal {
                        // Pre-gh:#1161 path, kept bit-for-bit for the ablation.
                        let sng = fvc::sn_grad(&self.p);
                        let sng_int = sng.internal.as_slice();
                        let rauf_int = rauf.internal.as_slice();
                        for f in 0..mesh.n_internal_faces {
                            phi_hbya.internal[f] -= rauf_int[f] * sng_int[f] * mesh.face_areas[f];
                        }
                    } else {
                        let dc = lap.delta_coeff();
                        for f in 0..mesh.n_internal_faces {
                            let dp = self.p.internal[mesh.neighbour[f]] - self.p.internal[mesh.owner[f]];
                            phi_hbya.internal[f] -= rauf.internal[f] * mesh.face_areas[f] * dp / dc.interior_delta(&mesh, f);
                        }
                        if let Some(c) = &face_corr {
                            for (f, cf) in c.iter().enumerate() {
                                phi_hbya.internal[f] -= cf;
                            }
                        }
                    }
                    // `phi = phiHbyA - pEqn.flux()` on the boundary too: a
                    // fixed-p patch carries the pressure-correction flux the
                    // matrix row already counted. Zero on zeroGradient-p
                    // patches, so a closed domain is unchanged.
                    add_fixed_pressure_boundary_flux(&mut phi_hbya, &self.p, &rauf, &mesh, lap.delta_coeff());
                }
                self.phi = phi_hbya;

                // Correct the cell velocity: U = HbyA − rAU·∇p, then re-impose BCs.
                self.u = hbya - rau.clone() * fvc::grad(&self.p);
                correct_bcs_vec(&mut self.u, &u_bcs);
            }
        }

        // ── Turbulence correction ────────────────────────────────────────────
        // OpenFOAM's `turbulence->correct()` sits at the bottom of the PIMPLE
        // loop, after the pressure correctors, so the k/ω/ε transport equations
        // are advanced with the *corrected*, divergence-free velocity and flux.
        // Re-sync first, because `self.u`/`self.phi` were rewritten by the last
        // corrector. A no-op when the closure is laminar.
        self.turbulence
            .sync_inputs(&self.u, &self.phi, &self.nu, dt);
        self.turbulence.correct();

        // Roll the time levels forward so a second-order backward ddt scheme has
        // U^{n−2} available on the next step.
        self.u_old_old = Some(u_old);
        self.phi_old_old = Some(phi_old);

        Ok(())
    }

    /// Advance the solver from the `controlDict` start time to its end time,
    /// calling [`Self::step`] repeatedly at the fixed step `control.delta_t`.
    ///
    /// This is a convenience wrapper over [`Self::step`]. Set the initial field
    /// state on the public members *before* calling it, and read the results off
    /// those same members afterwards — **`run` writes nothing to disk**, because
    /// every writer in [`crate::io::output`] is still `todo!()`.
    ///
    /// # Time control
    ///
    /// * Starts at `t` for [`StartControl::StartTime(t)`](StartControl::StartTime);
    ///   any other `startFrom` selection is treated as `t = 0`.
    /// * Runs while `t < end` for
    ///   [`StopControl::EndTime(end)`](StopControl::EndTime). **For every other
    ///   `stopAt` selection this returns `Ok(())` immediately without taking a
    ///   single step**, since those selections are defined in terms of a write
    ///   this crate cannot perform.
    /// * The step is fixed at `control.delta_t`. `adjustTimeStep`, `maxCo` and
    ///   `maxDeltaT` are **not implemented** — choose a Δt that respects your own
    ///   Courant limit.
    ///
    /// # Errors
    ///
    /// Propagates the first error from [`Self::step`] (typically
    /// [`AppBuilderError::Diverged`]); the run stops at that point with the
    /// fields left in their last state.
    pub fn run(&mut self) -> Result<(), AppBuilderError> {
        let start = match self.control.start {
            StartControl::StartTime(t) => t,
            _ => 0.0,
        };
        let end = match self.control.stop {
            StopControl::EndTime(t) => t,
            _ => return Ok(()),
        };
        let dt = self.control.delta_t;
        let mut time = start;
        while time < end {
            self.step()?;
            time += dt;
        }
        Ok(())
    }
}
