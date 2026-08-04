// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Physics formulation derived from Moltres (MSR multiphysics on MOOSE)
//   Upstream: https://github.com/arfc/moltres (UIUC ARFC group)
//   Upstream commit: 3dd2ce7
//   Upstream sources consulted (formulation only, no code reused):
//     src/kernels/PrecursorSource.C         (+beta_i sum_g nuSigma_f phi_g)
//     src/kernels/PrecursorDecay.C          (-lambda_i C_i)
//     src/kernels/CoupledScalarAdvection.C  (div(u C_i) — the "drift" that
//       carries precursors with the flowing fuel salt)
//     src/kernels/ScalarTransportTimeDerivative.C (d C_i / dt)
//   Upstream license: LGPL-2.1, incorporated into this GPL-3.0 crate under
//   the LGPL-2.1 section 3 GPL-conversion option.
//
// Finite-volume assembly built on outram-foam-basic-lib: fvm::div
// (first-order upwind), fvm::laplacian, fvm::sp, fvm::su, fvm::ddt, and the
// Gauss-Seidel LDU solver (the advection matrix is asymmetric, so the SPD
// conjugate-gradient path is not applicable).
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

//! Delayed-neutron precursor advection–decay ("drift") transport.
//!
//! For each precursor family `i` the concentration `C_i` (`1/m^3`) obeys
//!
//! ```text
//!   dC_i/dt + div(u C_i) - div(D_C grad C_i) = beta_i/k * S_f - lambda_i C_i
//! ```
//!
//! with `S_f = sum_g nuSigma_{f,g} phi_g` the fission-neutron production
//! (`1/(m^3 s)`), `u` the fuel-salt velocity, and `D_C` a small (numerical /
//! turbulent) precursor diffusivity in `m^2/s`. **The advection term is the
//! defining MSRE physics**: precursors born in the core are carried out into
//! the external loop by the flowing salt and decay there, where their
//! delayed neutrons are useless — so reactivity depends on the loop
//! velocity. The `1/k` factor on the production term is the k-eigenvalue
//! convention (pass `k = 1` for physical transients).
//!
//! Both a steady solve (for eigenvalue outer iterations) and a
//! backward-Euler transient step are provided. Spatial discretisation is
//! first-order upwind for advection (`fvm::div`) and Gauss-orthogonal for
//! diffusion (`fvm::laplacian`); the asymmetric system is solved with
//! Gauss-Seidel, which converges because decay plus upwinding keep the
//! matrix diagonally dominant.

use std::sync::Arc;

use outram_foam_basic_lib::prelude::{
    fvm, Field, FvMesh, SolverSettings, SurfaceScalarField, VolScalarField,
};

use crate::error::MoltresError;
use crate::materials::{scalar_field, DelayedFamily, FaceFluxField, PrecursorField};

/// Advection–decay transport of the delayed-neutron precursor families on a
/// fixed flow field. Construct once with [`PrecursorDrift::new`], then call
/// [`PrecursorDrift::solve_steady`] (eigenvalue outer loops) or
/// [`PrecursorDrift::step`] (transients).
#[derive(Debug, Clone)]
pub struct PrecursorDrift {
    mesh: Arc<FvMesh>,
    /// Delayed families (`beta_i` dimensionless, `lambda_i` in `1/s`).
    pub families: Vec<DelayedFamily>,
    /// Face volumetric flow flux `u . A_f` (`m^3/s`); positive =
    /// owner-to-neighbour. Zero everywhere = static fuel.
    pub flow: FaceFluxField,
    /// Precursor diffusivity `D_C` (`m^2/s`, uniform, >= 0; typically a
    /// small numerical/turbulent value like `1e-4` — molecular diffusion of
    /// precursor nuclides is negligible, but a nonzero value smooths the
    /// upwind advection front, mirroring Moltres' artificial-diffusion
    /// stabilisation).
    pub diffusion: f64,
    /// Linear-solver settings for each family's Gauss-Seidel solve.
    pub linear: SolverSettings,
}

impl PrecursorDrift {
    /// Build a precursor transport model.
    ///
    /// `flow` must live on `mesh` (one value per internal face) and should
    /// be divergence-free (a rigid loop circulation from
    /// [`crate::ring_mesh::RingMesh::uniform_flux`] is). `diffusion` is
    /// `D_C` in `m^2/s` (>= 0).
    ///
    /// # Errors
    /// [`MoltresError::SizeMismatch`] if `flow` is not on `mesh`;
    /// [`MoltresError::InvalidMaterial`] for a negative `D_C`, empty
    /// family list, or non-positive `lambda_i`.
    pub fn new(
        mesh: Arc<FvMesh>,
        families: Vec<DelayedFamily>,
        flow: FaceFluxField,
        diffusion: f64,
        linear: SolverSettings,
    ) -> Result<Self, MoltresError> {
        if flow.internal.len() != mesh.n_internal_faces {
            return Err(MoltresError::SizeMismatch {
                what: "flow flux (one value per internal face)",
                expected: mesh.n_internal_faces,
                got: flow.internal.len(),
            });
        }
        if !(diffusion >= 0.0) {
            return Err(MoltresError::InvalidMaterial(format!(
                "precursor diffusivity must be >= 0, got {diffusion}"
            )));
        }
        if families.is_empty() {
            return Err(MoltresError::InvalidMaterial(
                "at least one delayed family is required".into(),
            ));
        }
        if families
            .iter()
            .any(|f| !(f.lambda > 0.0) || !(f.beta >= 0.0))
        {
            return Err(MoltresError::InvalidMaterial(
                "delayed families need lambda > 0 and beta >= 0".into(),
            ));
        }
        Ok(Self {
            mesh,
            families,
            flow,
            diffusion,
            linear,
        })
    }

    /// Steady advection–decay balance for every family:
    /// `div(u C_i) - div(D_C grad C_i) + lambda_i C_i = beta_i/k * S_f`.
    ///
    /// `fission_source` is `S_f` in `1/(m^3 s)`; `k_eff` scales the
    /// production (pass 1.0 outside eigenvalue iterations). Returns one
    /// concentration field (`1/m^3`) per family. With zero flow this
    /// reproduces the algebraic equilibrium
    /// `C_i = beta_i S_f / (k lambda_i)` exactly (verified in tests).
    ///
    /// # Errors
    /// [`MoltresError::LinearSolveFailed`] if a family's Gauss-Seidel solve
    /// misses its tolerance.
    pub fn solve_steady(
        &self,
        fission_source: &VolScalarField,
        k_eff: f64,
    ) -> Result<Vec<PrecursorField>, MoltresError> {
        let mut out = Vec::with_capacity(self.families.len());
        for (i, fam) in self.families.iter().enumerate() {
            let name = format!("precursor{i}");
            let c = VolScalarField::zeros(name.clone(), self.mesh.clone());
            let eqn = self.lhs(fam, &c) - fvm::su(&self.production(fam, fission_source, k_eff), &c);
            let (sol, perf) = eqn.solve(name.clone(), self.linear);
            if !perf.converged {
                return Err(MoltresError::LinearSolveFailed {
                    field: name,
                    residual: perf.final_residual,
                    iterations: perf.n_iterations,
                });
            }
            out.push(sol);
        }
        Ok(out)
    }

    /// One backward-Euler step of length `dt` (s):
    /// `(C_i - C_i_old)/dt + div(u C_i) - div(D_C grad C_i) + lambda_i C_i
    /// = beta_i/k * S_f`, implicit in everything except the (lagged) fission
    /// source. Returns the new concentrations.
    ///
    /// # Errors
    /// [`MoltresError::SizeMismatch`] if `previous` does not hold one field
    /// per family; [`MoltresError::LinearSolveFailed`] on solver failure.
    pub fn step(
        &self,
        previous: &[PrecursorField],
        fission_source: &VolScalarField,
        k_eff: f64,
        dt: f64,
    ) -> Result<Vec<PrecursorField>, MoltresError> {
        if previous.len() != self.families.len() {
            return Err(MoltresError::SizeMismatch {
                what: "previous precursor fields (one per family)",
                expected: self.families.len(),
                got: previous.len(),
            });
        }
        let mut out = Vec::with_capacity(self.families.len());
        for (i, fam) in self.families.iter().enumerate() {
            let name = format!("precursor{i}");
            let c_old = &previous[i];
            let eqn = fvm::ddt(c_old, c_old, dt) + self.lhs(fam, c_old)
                - fvm::su(&self.production(fam, fission_source, k_eff), c_old);
            let (sol, perf) = eqn.solve(name.clone(), self.linear);
            if !perf.converged {
                return Err(MoltresError::LinearSolveFailed {
                    field: name,
                    residual: perf.final_residual,
                    iterations: perf.n_iterations,
                });
            }
            out.push(sol);
        }
        Ok(out)
    }

    /// Delayed-neutron volumetric source `S_d[c] = sum_i lambda_i C_i[c]`
    /// (`1/(m^3 s)`) — what the flux equations consume (Moltres
    /// `DelayedNeutronSource`).
    ///
    /// The caller must pass one field per family (as returned by
    /// [`Self::solve_steady`] / [`Self::step`]); extra or missing fields
    /// are a bug and only debug-asserted here.
    #[must_use]
    pub fn delayed_source(&self, precursors: &[PrecursorField]) -> VolScalarField {
        debug_assert_eq!(precursors.len(), self.families.len());
        let n = self.mesh.n_cells;
        let mut s = vec![0.0; n];
        for (fam, c_field) in self.families.iter().zip(precursors.iter()) {
            let cv = c_field.internal.as_slice();
            for c in 0..n {
                s[c] += fam.lambda * cv[c];
            }
        }
        scalar_field(&self.mesh, "delayedSource", s)
    }

    /// Spatial-operator left-hand side shared by steady and transient forms:
    /// `div(u C) - div(D_C grad C) + lambda C` as an `FvMatrix`.
    fn lhs(
        &self,
        fam: &DelayedFamily,
        c: &PrecursorField,
    ) -> outram_foam_basic_lib::prelude::FvMatrix {
        let lambda = VolScalarField::uniform("lambda", self.mesh.clone(), fam.lambda);
        let mut eqn = fvm::div(&self.flow, c) + fvm::sp(&lambda, c);
        if self.diffusion > 0.0 {
            let d_face = SurfaceScalarField::new(
                "Dc",
                self.mesh.clone(),
                Field::uniform(self.mesh.n_internal_faces, self.diffusion),
                self.mesh
                    .patches
                    .iter()
                    .map(|p| outram_foam_basic_lib::prelude::PatchField::zero_gradient(p.size))
                    .collect(),
            );
            eqn = eqn + fvm::laplacian(&d_face, c);
        }
        eqn
    }

    /// Production source `beta_i/k * S_f` (`1/(m^3 s)`).
    fn production(
        &self,
        fam: &DelayedFamily,
        fission_source: &VolScalarField,
        k_eff: f64,
    ) -> VolScalarField {
        let vals: Vec<f64> = fission_source
            .internal
            .as_slice()
            .iter()
            .map(|s| fam.beta / k_eff * s)
            .collect();
        scalar_field(&self.mesh, "precProduction", vals)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ring_mesh::RingMesh;

    fn ring() -> RingMesh {
        RingMesh::new(15.0, 0.1, 150).unwrap()
    }

    /// Core-localised fission source: `S_f = 1e15 1/(m^3 s)` for `s < 5.6 m`,
    /// zero elsewhere.
    fn core_source(ring: &RingMesh) -> VolScalarField {
        let vals: Vec<f64> = (0..ring.n_cells)
            .map(|i| if ring.arc_centre(i) < 5.6 { 1e15 } else { 0.0 })
            .collect();
        scalar_field(&ring.mesh, "Sf", vals)
    }

    fn settings() -> SolverSettings {
        SolverSettings {
            tolerance: 1e-10,
            max_iter: 20_000,
        }
    }

    /// V&V (verification, limiting case) — zero flow reproduces the
    /// algebraic equilibrium.
    ///
    /// **Methodology.** Closed ring (15 m, 0.1 m^2, 150 cells), Keepin
    /// U-235 families, `u = 0`, `D_C = 0`, core-localised `S_f`. The steady
    /// equation degenerates to `lambda_i C_i = beta_i/k S_f`, so every cell
    /// must satisfy `C_i = beta_i S_f / (k lambda_i)` (with `k = 1.02`)
    /// to linear-solver accuracy. Pass criterion: max relative error
    /// `< 1e-8` over all families and cells with `S_f > 0`.
    ///
    /// **Result (measured 2026-08-04, release build):** max relative error
    /// `2.3e-16` (machine precision — the zero-flow matrix is diagonal).
    /// Untrusted AI-assisted draft pending human V&V.
    #[test]
    fn zero_flow_matches_algebraic_equilibrium() {
        let ring = ring();
        let families = DelayedFamily::keepin_u235();
        let drift = PrecursorDrift::new(
            ring.mesh.clone(),
            families.clone(),
            ring.uniform_flux(0.0),
            0.0,
            settings(),
        )
        .unwrap();
        let s_f = core_source(&ring);
        let k = 1.02;
        let c = drift.solve_steady(&s_f, k).unwrap();
        let mut max_rel: f64 = 0.0;
        for (i, fam) in families.iter().enumerate() {
            for (cell, s) in s_f.internal.as_slice().iter().enumerate() {
                if *s <= 0.0 {
                    continue;
                }
                let expected = fam.beta * s / (k * fam.lambda);
                let got = c[i].internal[cell];
                max_rel = max_rel.max(((got - expected) / expected).abs());
            }
        }
        println!("[V&V precursor equilibrium] max rel err = {max_rel:.3e}");
        assert!(max_rel < 1e-8, "max rel err = {max_rel:.3e}");
    }

    /// V&V (verification, conservation) — global production/decay balance
    /// on the closed loop with flow ON.
    ///
    /// **Methodology.** Same ring, `u = 0.6 m/s` (MSRE-like ~25 s loop
    /// circulation), `D_C = 1e-4 m^2/s`. Integrating the steady equation
    /// over the closed loop, advection and diffusion telescope to zero
    /// (periodic domain, no boundaries), leaving exactly
    /// `int beta_i/k S_f dV = int lambda_i C_i dV` per family. Pass
    /// criterion: relative imbalance `< 1e-8` for every family — this
    /// exercises the upwind `fvm::div` assembly, the wrap-around face, and
    /// the Gauss-Seidel solve together.
    ///
    /// **Result (measured 2026-08-04, release build):** relative imbalance
    /// per family (0–5): `8.6e-11`, `7.3e-11`, `5.8e-11`, `3.9e-11`,
    /// `5.0e-11`, `4.0e-13` — worst `8.6e-11`. Untrusted AI-assisted draft
    /// pending human V&V.
    #[test]
    fn steady_flow_conserves_production_vs_decay() {
        let ring = ring();
        let families = DelayedFamily::keepin_u235();
        let drift = PrecursorDrift::new(
            ring.mesh.clone(),
            families.clone(),
            ring.uniform_flux(0.6),
            1e-4,
            settings(),
        )
        .unwrap();
        let s_f = core_source(&ring);
        let k = 1.0;
        let c = drift.solve_steady(&s_f, k).unwrap();
        let vols = &ring.mesh.cell_volumes;
        for (i, fam) in families.iter().enumerate() {
            let produced: f64 = s_f
                .internal
                .as_slice()
                .iter()
                .zip(vols.iter())
                .map(|(s, v)| fam.beta / k * s * v)
                .sum();
            let decayed: f64 = c[i]
                .internal
                .as_slice()
                .iter()
                .zip(vols.iter())
                .map(|(cc, v)| fam.lambda * cc * v)
                .sum();
            let rel = ((produced - decayed) / produced).abs();
            println!("[V&V precursor conservation] family {i}: rel imbalance = {rel:.3e}");
            assert!(rel < 1e-8, "family {i}: rel imbalance = {rel:.3e}");
        }
    }

    /// Flow ON must deplete the in-core precursor inventory relative to the
    /// zero-flow equilibrium (they are advected into the external loop) —
    /// the cell-level signature behind the beta_eff loss.
    ///
    /// **Result (measured 2026-08-04, release build):** with a lumped
    /// family (`beta = 0.0065`, `lambda = 0.08 1/s`) at `u = 0.6 m/s`, the
    /// in-core inventory falls to `0.42` of the zero-flow equilibrium
    /// (`4.55e13 -> 1.92e13` weighted by cell volume).
    #[test]
    fn flow_depletes_in_core_precursors() {
        let ring = ring();
        let families = vec![DelayedFamily {
            beta: 0.0065,
            lambda: 0.08,
        }];
        let s_f = core_source(&ring);
        let still = PrecursorDrift::new(
            ring.mesh.clone(),
            families.clone(),
            ring.uniform_flux(0.0),
            0.0,
            settings(),
        )
        .unwrap()
        .solve_steady(&s_f, 1.0)
        .unwrap();
        let moving = PrecursorDrift::new(
            ring.mesh.clone(),
            families,
            ring.uniform_flux(0.6),
            0.0,
            settings(),
        )
        .unwrap()
        .solve_steady(&s_f, 1.0)
        .unwrap();
        let core_sum = |c: &PrecursorField| -> f64 {
            (0..ring.n_cells)
                .filter(|i| ring.arc_centre(*i) < 5.6)
                .map(|i| c.internal[i] * ring.mesh.cell_volumes[i])
                .sum()
        };
        let inv_still = core_sum(&still[0]);
        let inv_moving = core_sum(&moving[0]);
        println!(
            "[V&V precursor depletion] in-core inventory still = {inv_still:.4e}, \
             moving = {inv_moving:.4e}, ratio = {:.4}",
            inv_moving / inv_still
        );
        assert!(
            inv_moving < 0.9 * inv_still,
            "in-core inventory: still = {inv_still:.3e}, moving = {inv_moving:.3e}"
        );
    }

    /// Backward-Euler transient must relax onto the steady solution when
    /// marched with a constant fission source (consistency of `step` with
    /// `solve_steady`).
    ///
    /// **Result (measured 2026-08-04, release build):** after 400 steps of
    /// `dt = 0.25 s` (100 s = 30 decay time constants), max deviation from
    /// the steady profile is `1.1e-9` of the steady peak.
    #[test]
    fn transient_relaxes_to_steady() {
        let ring = RingMesh::new(15.0, 0.1, 60).unwrap();
        let families = vec![DelayedFamily {
            beta: 0.0065,
            lambda: 0.3,
        }];
        let drift = PrecursorDrift::new(
            ring.mesh.clone(),
            families,
            ring.uniform_flux(0.6),
            1e-4,
            settings(),
        )
        .unwrap();
        let vals: Vec<f64> = (0..ring.n_cells)
            .map(|i| if ring.arc_centre(i) < 5.6 { 1e15 } else { 0.0 })
            .collect();
        let s_f = scalar_field(&ring.mesh, "Sf", vals);
        let steady = drift.solve_steady(&s_f, 1.0).unwrap();

        let mut c = vec![VolScalarField::zeros("precursor0", ring.mesh.clone())];
        for _ in 0..400 {
            c = drift.step(&c, &s_f, 1.0, 0.25).unwrap();
        }
        // After 100 s (= 30 decay time constants, 4 loop transits) the
        // transient should sit on the steady profile.
        let mut max_rel: f64 = 0.0;
        let peak = steady[0].internal.max();
        for cell in 0..ring.n_cells {
            let d = (c[0].internal[cell] - steady[0].internal[cell]).abs();
            max_rel = max_rel.max(d / peak);
        }
        println!("[V&V precursor transient->steady] max rel deviation = {max_rel:.3e}");
        assert!(max_rel < 1e-3, "max rel deviation = {max_rel:.3e}");
    }
}
