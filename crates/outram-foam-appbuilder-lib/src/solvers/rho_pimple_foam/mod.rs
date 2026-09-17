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

//! # `rho_pimple_foam` — compressible transient PIMPLE solver (rhoPimpleFoam)
//!
//! Rust port of rhoPimpleFoam: the compressible counterpart of pimpleFoam,
//! solving continuity, momentum, and (enthalpy-form) energy with a
//! compressibility-consistent pressure equation. Suited to subsonic compressible
//! transient flow. See [`RhoPimpleFoam`] for the governing equations and time
//! loop; a companion stability primer lives in this module's `docs/`.

use crate::error::AppBuilderError;
use crate::io::control_dict::{ControlDict, StartControl, StopControl};
use crate::io::fv_schemes::FvSchemes;
use crate::io::fv_solution::FvSolution;
use crate::solvers::bc_util::{capture_bcs, correct_bcs, correct_bcs_vec};
use crate::turbulence::TurbulenceClosure;
use outram_foam_basic_lib::prelude::*;
use std::sync::Arc;

/// Compressible transient PIMPLE solver — rhoPimpleFoam.
///
/// Solves:
///   ∂ρ/∂t  + ∇·(ρU)     = 0          (continuity)
///   ∂(ρU)/∂t + ∇·(ρUU)  = −∇p + ∇·τ  (momentum)
///   ∂(ρh)/∂t + ∇·(ρUh)  = dp/dt      (energy, h-form, adiabatic closure)
///   ρ = ψ·p                            (EOS approximation)
///
/// Pressure equation includes the compressibility term ψ·∂p/∂t so that the
/// system is consistent with the linearised continuity equation.
///
/// C++ solver: `applications/solvers/compressible/rhoPimpleFoam/`
pub struct RhoPimpleFoam {
    pub mesh: Arc<FvMesh>,
    pub control: ControlDict,
    pub schemes: FvSchemes,
    pub solution: FvSolution,
    /// Velocity field [m/s]
    pub u: VolVectorField,
    /// Pressure field [Pa]
    pub p: VolScalarField,
    /// Density field [kg/m³]
    pub rho: VolScalarField,
    /// Temperature field [K]
    pub t: VolScalarField,
    /// Specific enthalpy [J/kg]
    pub he: VolScalarField,
    /// Dynamic viscosity μ [Pa·s]
    pub mu: VolScalarField,
    /// Effective thermal diffusivity αh = κ/Cp [kg/(m·s)]
    pub alpha_h: VolScalarField,
    /// Compressibility ψ = ∂ρ/∂p|_T = ρ/p [s²/m²]
    pub psi: VolScalarField,
    /// Mass flux φ = ρ U·Sf [kg/s]
    pub phi: SurfaceScalarField,
    /// Turbulence closure (default [`TurbulenceClosure::Laminar`]).
    ///
    /// Selecting a RAS/LES model makes the momentum viscous term use the
    /// effective **dynamic** viscosity μ_eff = μ + ρ ν_t [Pa·s] and the energy
    /// equation use α_eff = α + ρ ν_t / Pr_t [kg/(m·s)], and advances the
    /// model's transport equations once per time step after the pressure
    /// correctors.
    ///
    /// The closures in `outram-foam-turbulence-lib` are formulated
    /// **kinematically**, so this solver feeds them ν = μ/ρ and the
    /// **volumetric** flux φ/ρ_f, and converts ν_t back with μ_t = ρ ν_t. That
    /// mapping is the constant-density approximation to OpenFOAM's compressible
    /// `fvm::div(alphaRhoPhi, k)` form — exact only where ρ is uniform. See
    /// [`crate::turbulence`] for the full scope limits.
    ///
    /// **The default is laminar**, so an existing run is unaffected.
    pub turbulence: TurbulenceClosure,
}

impl RhoPimpleFoam {
    /// Build a compressible PIMPLE solver on `mesh`, with every field allocated
    /// at a placeholder initial state.
    ///
    /// The caller is expected to overwrite the public field members before
    /// stepping, and to set the mesh boundary conditions. The defaults describe
    /// still air at roughly standard conditions:
    ///
    /// | Field | Initial value |
    /// |---|---|
    /// | `u` — velocity [m/s] | zero |
    /// | `p` — static pressure, Pa | uniform 1.0e5 |
    /// | `rho` — density [kg/m³] | uniform 1.0 |
    /// | `t` — temperature, K | uniform 300 |
    /// | `he` — specific enthalpy [J/kg] | zero |
    /// | `mu` — dynamic viscosity, Pa·s | uniform 1.8e-5 |
    /// | `alpha_h` — effective thermal diffusivity [kg/(m·s)] | uniform 2.5e-5 |
    /// | `psi` — compressibility ρ/p [s²/m²] | uniform 1.0e-5 |
    /// | `phi` — mass face flux [kg/s] | zero |
    /// | `turbulence` | [`TurbulenceClosure::Laminar`] |
    ///
    /// Unlike [`crate::solvers::pimple_foam`], `p` here is **absolute pressure in
    /// Pa**, and the density closure is the ideal-gas form `ρ = ψ·p`, so `psi`
    /// and `t` must be mutually consistent for the case you are setting up.
    ///
    /// # Arguments
    ///
    /// See [`crate::solvers::pimple_foam::PimpleFoam::new`] — the four arguments
    /// have the same meaning and the same honoured-field caveats.
    pub fn new(
        mesh: Arc<FvMesh>,
        control: ControlDict,
        schemes: FvSchemes,
        solution: FvSolution,
    ) -> Self {
        let u = VolVectorField::zero("U", mesh.clone());
        let p = VolScalarField::uniform("p", mesh.clone(), 1.0e5);
        let rho = VolScalarField::uniform("rho", mesh.clone(), 1.0);
        let t = VolScalarField::uniform("T", mesh.clone(), 300.0);
        let he = VolScalarField::zeros("he", mesh.clone());
        let mu = VolScalarField::uniform("mu", mesh.clone(), 1.8e-5);
        let alpha_h = VolScalarField::uniform("alphaEff", mesh.clone(), 2.5e-5);
        let psi = VolScalarField::uniform("psi", mesh.clone(), 1.0e-5);
        let phi = SurfaceScalarField::zeros("phi", mesh.clone());
        Self {
            mesh,
            control,
            schemes,
            solution,
            u,
            p,
            rho,
            t,
            he,
            mu,
            alpha_h,
            psi,
            phi,
            turbulence: TurbulenceClosure::default(),
        }
    }

    /// Molecular **kinematic** viscosity ν = μ/ρ [m²/s], per cell.
    ///
    /// The turbulence closures are kinematic; this is the field they are fed.
    /// Density is floored at 1e-30 kg/m³ to keep the division finite.
    fn nu_molecular(&self) -> VolScalarField {
        let n = self.mesh.n_cells;
        let vals: Vec<f64> = (0..n)
            .map(|c| self.mu.internal[c] / self.rho.internal[c].max(1e-30))
            .collect();
        VolScalarField::new(
            "nu",
            self.mesh.clone(),
            Field::new(vals),
            self.mesh
                .patches
                .iter()
                .map(|p| PatchField::zero_gradient(p.size))
                .collect(),
        )
    }

    /// Advance one time step with compressible PIMPLE.
    ///
    /// Structure mirrors the fixed incompressible pimpleFoam (see that solver's
    /// module doc for the sign/convention rationale), adapted for compressible
    /// flow: the momentum/pressure carry the density ρ, the pressure equation
    /// gains the compressibility diagonal ψ·V/dt (which also makes it
    /// non-singular without a pressure reference), and an energy equation closes
    /// the system. The applied fixes, all proven on pimpleFoam:
    ///   - `+ fvm::laplacian_vec` for the viscous term (positive-definite
    ///     operator convention), likewise `+ fvm::laplacian` in the energy eqn;
    ///   - pressure source = −(net φ_HbyA outflow) + ψ·V/dt·p_old;
    ///   - `constrainHbyA`: boundary mass flux uses the prescribed ρ_f·U_BC·Sf;
    ///   - PISO corrector loop re-evaluates H(U)/HbyA each pass;
    ///   - PCG (`solve_cg`) for the symmetric pressure system;
    ///   - boundary conditions re-applied after every field update.
    pub fn step(&mut self) -> Result<(), AppBuilderError> {
        let mesh = self.mesh.clone();
        let n = mesh.n_cells;
        let dt = self.control.delta_t;
        let settings = SolverSettings::default(); // U, energy (GS)
        let p_settings = SolverSettings {
            tolerance: 1e-8,
            max_iter: 2_000,
        }; // pEqn (PCG)
        let n_outer = self.solution.pimple.n_outer_correctors.max(1);
        let n_inner = self.solution.pimple.n_correctors.max(1);

        let u_old = self.u.clone();
        let p_old = self.p.clone();
        let he_old = self.he.clone();
        let rho_old = self.rho.clone();

        let u_bcs = capture_bcs(&self.u.boundary);
        let p_bcs = capture_bcs(&self.p.boundary);

        for _ in 0..n_outer {
            // ── rhoEqn: explicit continuity ρ = ρ_old − dt·∇·φ ──────────────
            let div_phi = fvc::div_flux(&self.phi);
            self.rho = rho_old.clone() + (-dt) * div_phi;
            for c in 0..n {
                if self.rho.internal[c] < 1e-4 {
                    self.rho.internal[c] = 1e-4;
                }
            }

            // ── Turbulence state push ───────────────────────────────────────
            // The closures are kinematic: feed them ν = μ/ρ and the volumetric
            // flux φ/ρ_f. `mu_eff` below converts ν_t back to μ_t = ρ ν_t.
            let nu_mol = self.nu_molecular();
            let phi_vol = TurbulenceClosure::volumetric_flux(&self.phi, &self.rho);
            self.turbulence.sync_inputs(&self.u, &phi_vol, &nu_mol, dt);

            // ── UEqn: ∂(ρU)/∂t + ∇·(ρUU) + (−∇·(μ_eff ∇U)) ────────────────
            // `+ laplacian_vec`: the operator is positive-definite (= −∇·(μ∇)).
            // μ_eff = μ + ρ ν_t; for the default laminar closure μ_eff = μ, so
            // this is bit-for-bit the previous molecular term.
            let mu_eff = self.turbulence.mu_eff(&self.mu, &self.rho);
            let mut u_eqn = fvm::ddt_coeff_vec(&self.rho, &self.u, &u_old, dt, mesh.clone())
                + fvm::div_vec(&self.phi, &self.u, mesh.clone())
                + fvm::laplacian_vec(&mu_eff, &self.u, mesh.clone());

            // A [kg/s]; rAU = V/A [m³·s/kg]
            let a = u_eqn.a_field();
            let rau = {
                let a_sl = a.internal.as_slice();
                let vals: Vec<f64> = (0..n)
                    .map(|c| mesh.cell_volumes[c] / a_sl[c].max(1e-30))
                    .collect();
                VolScalarField::new(
                    "rAU",
                    mesh.clone(),
                    Field::new(vals),
                    mesh.patches
                        .iter()
                        .map(|p| PatchField::zero_gradient(p.size))
                        .collect(),
                )
            };

            // Momentum predictor with explicit −V·∇p.
            let gp = fvc::grad(&self.p);
            for c in 0..n {
                u_eqn.source[c] = u_eqn.source[c] - gp.internal[c] * mesh.cell_volumes[c];
            }
            let (mut u_pred, _) = u_eqn.solve("U", settings);
            correct_bcs_vec(&mut u_pred, &u_bcs);
            for c in 0..n {
                u_eqn.source[c] = u_eqn.source[c] + gp.internal[c] * mesh.cell_volumes[c];
            }
            self.u = u_pred;

            let rauf = fvc::interpolate(&rau);

            // ── PISO pressure-correction loop (H(U) re-evaluated each pass) ──
            for _ in 0..n_inner {
                // HbyA = H(U)/A [m/s] from the latest U.
                let h = u_eqn.h_field(&self.u);
                let hbya = {
                    let h_sl = h.internal.as_slice();
                    let a_sl = a.internal.as_slice();
                    let vals: Vec<Vector3> = (0..n)
                        .map(|c| h_sl[c] * (1.0 / a_sl[c].max(1e-30)))
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

                let rho_f = fvc::interpolate(&self.rho); // ρ_f [kg/m³]
                let rho_rauf = rho_f.clone() * rauf.clone(); // [s]
                                                             // φ_HbyA = ρ_f · flux(HbyA): mass flux [kg/s]
                let mut phi_hbya = rho_f.clone() * fvc::flux(&hbya);

                // Pressure source = ψ·V/dt·p_old − (net φ_HbyA outflow) [kg/s].
                // Boundary flux is the prescribed ρ_f·U_BC·Sf (constrainHbyA);
                // on a no-penetration wall U_BC·n = 0 → no mass crosses it.
                let psi_sl = self.psi.internal.as_slice();
                let p_old_sl = p_old.internal.as_slice();
                let source_p = {
                    let mut s = vec![0.0_f64; n];
                    let phi_int = phi_hbya.internal.as_slice();
                    for f in 0..mesh.n_internal_faces {
                        s[mesh.owner[f]] -= phi_int[f];
                        s[mesh.neighbour[f]] += phi_int[f];
                    }
                    for (pi, patch) in mesh.patches.iter().enumerate() {
                        if matches!(self.u.boundary[pi].bc, BoundaryCondition::Empty) {
                            continue;
                        }
                        for fi in 0..patch.size {
                            let gf = patch.start + fi;
                            let flux = match self.u.boundary[pi].bc {
                                BoundaryCondition::FixedValue(ubc) => {
                                    rho_f.boundary[pi].values[fi]
                                        * ubc.dot(mesh.face_area_vectors[gf])
                                }
                                // outlet / zero-gradient: keep the extrapolated flux
                                _ => phi_hbya.boundary[pi].values[fi],
                            };
                            s[mesh.owner[gf]] -= flux;
                        }
                    }
                    for c in 0..n {
                        s[c] += psi_sl[c] * mesh.cell_volumes[c] / dt * p_old_sl[c];
                    }
                    s
                };

                // pEqn: [L(ρ_f·rAU_f) + ψ·V/dt]·p = source. The ψ·V/dt diagonal
                // makes the system non-singular, so no reference cell is needed;
                // it is symmetric SPD → PCG.
                let mut p_eqn = fvm::laplacian(&rho_rauf, &self.p);
                for c in 0..n {
                    p_eqn.ldu.diag[c] += psi_sl[c] * mesh.cell_volumes[c] / dt;
                }
                // ADD the mass-flux + ψ·V/dt source to the laplacian's own
                // source rather than OVERWRITING it: `fvm::laplacian` already
                // put each FixedValue pressure boundary's Dirichlet source
                // contribution (`coeff·p_bc`) into `p_eqn.source`, and its
                // matching `coeff` into the diagonal. Overwriting the source
                // dropped the `coeff·p_bc` term while keeping the diagonal
                // one, which silently imposed `p_boundary = 0` instead of the
                // prescribed value -- so any fixed-pressure outlet drove its
                // owner cell toward zero and blew up (a spurious disturbance
                // even from a uniform equilibrium field).
                for (s, &sp) in p_eqn.source.iter_mut().zip(source_p.iter()) {
                    *s += sp;
                }
                let (mut p_new, _) = p_eqn.solve_cg("p", p_settings);
                correct_bcs(&mut p_new, &p_bcs);
                self.p = p_new;

                // Correct the mass flux: φ = φ_HbyA − ρ_f·rAU_f·snGrad(p)·|Sf|.
                let sng = fvc::sn_grad(&self.p);
                {
                    let sng_sl = sng.internal.as_slice();
                    let rho_rauf_sl = rho_rauf.internal.as_slice();
                    for f in 0..mesh.n_internal_faces {
                        phi_hbya.internal[f] -= rho_rauf_sl[f] * sng_sl[f] * mesh.face_areas[f];
                    }
                    self.phi = phi_hbya;
                }

                // U = HbyA − rAU·∇p, re-impose BCs.
                self.u = hbya - rau.clone() * fvc::grad(&self.p);
                correct_bcs_vec(&mut self.u, &u_bcs);

                // EOS: ρ = ψ·p.
                let p_sl = self.p.internal.as_slice();
                for c in 0..n {
                    self.rho.internal[c] = (psi_sl[c] * p_sl[c]).max(1e-4);
                }
            }

            // ── Energy equation ─────────────────────────────────────────────
            //   ∂(ρh)/∂t + ∇·(φh) + (−∇·(αh∇h)) = dp/dt   [+ laplacian sign]
            let conv_he = fvc::div(&self.phi, &self.he); // explicit ∇·(φh)/V
                                                         // α_eff = α + ρ ν_t / Pr_t; equals α for the laminar default.
            let alpha_eff = self
                .turbulence
                .alpha_eff_compressible(&self.alpha_h, &self.rho);
            let alpha_h_f = fvc::interpolate(&alpha_eff);
            let dp_dt = (self.p.clone() - p_old.clone()) * (1.0 / dt);

            let mut e_eqn = fvm::ddt_coeff(&self.rho, &self.he, &he_old, dt)
                + fvm::laplacian(&alpha_h_f, &self.he);
            {
                let conv_sl = conv_he.internal.as_slice();
                let dpdt_sl = dp_dt.internal.as_slice();
                for c in 0..n {
                    let v = mesh.cell_volumes[c];
                    e_eqn.source[c] -= v * conv_sl[c]; // explicit convection
                    e_eqn.source[c] += v * dpdt_sl[c]; // dp/dt source
                }
            }
            let (he_new, _) = e_eqn.solve("he", settings);
            self.he = he_new;
        }

        // ── Turbulence correction (OpenFOAM `turbulence->correct()`) ─────────
        // After the pressure correctors, on the corrected U/φ. No-op if laminar.
        let nu_mol = self.nu_molecular();
        let phi_vol = TurbulenceClosure::volumetric_flux(&self.phi, &self.rho);
        self.turbulence.sync_inputs(&self.u, &phi_vol, &nu_mol, dt);
        self.turbulence.correct();

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::field_reader::{read_vol_scalar_field_full, read_vol_vector_field_full};
    use crate::io::poly_mesh::read_poly_mesh;
    use std::path::Path;

    /// Compressible (very low-Mach) lid-driven cavity stability check.
    ///
    /// This is the rhoPimpleFoam analogue of the test that exposed the original
    /// pimpleFoam coupling bugs: with the broken structure the cavity diverges
    /// to NaN within a handful of steps. With the ported fixes it must stay
    /// bounded and finite. Driving a closed box with a moving lid at Ma ≈ 0.003
    /// (lid 1 m/s, c ≈ 316 m/s) exercises the full ρ/p/U coupling without
    /// needing turbulence or accurate thermophysics.
    #[test]
    fn compressible_lid_cavity_stays_stable() {
        let case = Path::new(env!("CARGO_MANIFEST_DIR")).join("tutorials/cases/pimple_foam_cavity");
        let mesh =
            read_poly_mesh(&case.join("constant").join("polyMesh")).expect("read cavity polyMesh");
        let n = mesh.n_cells;

        let control = ControlDict {
            start: StartControl::StartTime(0.0),
            stop: StopControl::EndTime(5e-3),
            delta_t: 1e-4,
            ..ControlDict::default()
        };
        let mut solution = FvSolution::default();
        solution.pimple.n_outer_correctors = 1;
        solution.pimple.n_correctors = 2;

        let mut solver = RhoPimpleFoam::new(mesh.clone(), control, FvSchemes::default(), solution);

        // U carries the cavity BCs (movingWall fixedValue (1,0,0), noSlip walls,
        // empty front/back); p keeps its BC types but is set to 1e5 Pa.
        solver.u = read_vol_vector_field_full(&case.join("0").join("U"), &mesh).expect("read 0/U");
        solver.p = read_vol_scalar_field_full(&case.join("0").join("p"), &mesh).expect("read 0/p");
        for v in solver.p.internal.as_mut_slice() {
            *v = 1.0e5;
        }
        // ρ = ψ·p = 1e-5 · 1e5 = 1.0 kg/m³; μ for air.
        solver.psi = VolScalarField::uniform("psi", mesh.clone(), 1.0e-5);
        solver.rho = VolScalarField::uniform("rho", mesh.clone(), 1.0);
        solver.mu = VolScalarField::uniform("mu", mesh.clone(), 1.8e-5);

        for s in 0..50 {
            solver.step().expect("step");
            let umax = solver
                .u
                .internal
                .as_slice()
                .iter()
                .map(|v| v.mag())
                .fold(0.0, f64::max);
            let pmax = solver
                .p
                .internal
                .as_slice()
                .iter()
                .cloned()
                .fold(0.0_f64, |m, x| m.max(x.abs()));
            let nfin = solver
                .u
                .internal
                .as_slice()
                .iter()
                .filter(|v| !(v.x.is_finite() && v.y.is_finite() && v.z.is_finite()))
                .count();
            assert_eq!(nfin, 0, "step {s}: {nfin} non-finite U cells (diverged)");
            // lid speed is 1 m/s; a stable cavity keeps |U| ~ O(1), pressure ~ 1e5.
            assert!(umax < 10.0, "step {s}: |U|max = {umax:.3e} (blowing up)");
            assert!(pmax < 1e7, "step {s}: |p|max = {pmax:.3e} (blowing up)");
        }
        let _ = n;
    }
}
