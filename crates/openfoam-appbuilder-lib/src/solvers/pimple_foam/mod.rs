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
//! **Root cause of the sign flips (changes 0a/0b): `openfoam-basic-lib`'s
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
//!    `openfoam_basic_lib::fv_operators::fvc::ddt_corr`.

use crate::error::AppBuilderError;
use crate::io::control_dict::{ControlDict, StartControl, StopControl};
use crate::io::fv_schemes::FvSchemes;
use crate::io::fv_solution::FvSolution;
use openfoam_basic_lib::prelude::*;
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
}

impl PimpleFoam {
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
        }
    }

    /// Advance the solution by one time step using the PIMPLE algorithm.
    ///
    /// Dimensional analysis (integrated FV system):
    ///   A has units m³/s  →  rAU = V/A has units s
    ///   H has units m⁴/s²  →  HbyA = H/A has units m/s
    ///   phi_hbya = flux(HbyA) has units m³/s
    ///   Pressure source = Σ_f phi_hbya_f (NOT divided by V) ∈ m³/s
    ///   Laplacian coeff = rAUf · area/delta ∈ s·m  →  coeff·p (m²/s²) = m³/s ✓
    pub fn step(&mut self) -> Result<(), AppBuilderError> {
        let mesh = self.mesh.clone();
        let n = mesh.n_cells;
        let dt = self.control.delta_t;
        let settings = SolverSettings::default();
        // The pressure Poisson equation is symmetric SPD and elliptic; it is
        // solved with preconditioned conjugate gradient (`solve_cg`), which
        // converges in O(√κ) iterations. An under-solved pEqn leaves residual
        // divergence that accumulates and destabilises the run, so the tolerance
        // is kept tight.
        let p_settings = SolverSettings {
            tolerance: 1e-8,
            max_iter: 2_000,
        };
        let n_outer = self.solution.pimple.n_outer_correctors.max(1);
        let n_inner = self.solution.pimple.n_correctors.max(1);

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

        for _ in 0..n_outer {
            // ── Assemble implicit momentum equation (no pressure source yet) ───
            // `fvm::laplacian_vec` is assembled positive-definite (diag += coeff),
            // i.e. it represents −∇·(ν∇U). The momentum viscous term is −∇·(ν∇U),
            // so it is ADDED here (not subtracted) — subtracting would drive the
            // matrix diagonal negative and blow the solve up.
            let mut u_eqn = fvm::ddt_vec(&self.u, &u_old, dt, mesh.clone())
                + fvm::div_vec(&self.phi, &self.u, mesh.clone())
                + fvm::laplacian_vec(&self.nu, &self.u, mesh.clone());

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
            let (mut u_pred, _) = u_eqn.solve("U", settings);
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
            let ddt_corr = fvc::ddt_corr(&u_old, &phi_old, dt);

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
                    // Boundary flux is the prescribed U_BC·Sf (0 through a
                    // no-penetration wall), not the zero-gradient HbyA
                    // extrapolation — OpenFOAM's `constrainHbyA`. Using the
                    // extrapolation leaks spurious wall flux, breaks the
                    // closed-domain compatibility condition (Σ source ≠ 0), and
                    // ramps the pinned-Poisson pressure each step.
                    for (pi, patch) in mesh.patches.iter().enumerate() {
                        if matches!(self.u.boundary[pi].bc, BoundaryCondition::Empty) {
                            continue; // 2-D front/back: no in-plane flux
                        }
                        for fi in 0..patch.size {
                            let gf = patch.start + fi;
                            let u_bc = self.u.boundary[pi].values[fi];
                            s[mesh.owner[gf]] -= u_bc.dot(mesh.face_area_vectors[gf]);
                        }
                    }
                    s
                };

                // Pressure equation: L·p = source (symmetric SPD → PCG).
                let mut p_eqn = fvm::laplacian(&rauf, &self.p);
                p_eqn.source = Field::new(source_p);
                p_eqn.set_reference(0, 0.0); // pin reference (closed domain)
                let (mut p_new, _) = p_eqn.solve_cg("p", p_settings);
                correct_bcs(&mut p_new, &p_bcs);
                self.p = p_new;

                // Correct the face flux: phi = phiHbyA − rAUf·snGrad(p)·|Sf|
                let sng = fvc::sn_grad(&self.p);
                {
                    let sng_int = sng.internal.as_slice();
                    let rauf_int = rauf.internal.as_slice();
                    for f in 0..mesh.n_internal_faces {
                        phi_hbya.internal[f] -= rauf_int[f] * sng_int[f] * mesh.face_areas[f];
                    }
                    self.phi = phi_hbya;
                }

                // Correct the cell velocity: U = HbyA − rAU·∇p, then re-impose BCs.
                self.u = hbya - rau.clone() * fvc::grad(&self.p);
                correct_bcs_vec(&mut self.u, &u_bcs);
            }
        }

        Ok(())
    }

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
