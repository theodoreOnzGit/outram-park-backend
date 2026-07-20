// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da
//   Upstream source: src/classes/neutronics/diffusion/include/
//                    {createFluxMatrices,fluxEq,normFluxes,solveNeutronics}.H
//                    and src/classes/neutronics/include/calcKeff.H
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//     Principal authors: Carlo Fiorina, Nordine Kerkar, Konstantin Mikityuk (EPFL)
//   Upstream license: GPL-3.0
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
//
// This offering is not approved or endorsed by EPFL, the OpenFOAM Foundation,
// nor OpenCFD Limited, producer and distributor of the OpenFOAM(R) software.

//! The k-eigenvalue outer power iteration (see [`super`]).

use outram_foam_basic_lib::prelude::{fvm, Field, VolScalarField};

use super::{DiffusionNeutronics, EigenvalueReport};
use crate::genfoam::neutronics::NeutronicsError;

impl DiffusionNeutronics {
    /// Solve the multigroup k-eigenvalue problem by outer power iteration.
    ///
    /// Reduces the coupled flux + precursor system of a source-free reactor to
    /// its fundamental mode: the largest eigenvalue `k_eff` and the associated
    /// flux shape. Each outer iteration holds the fission source fixed (lagged
    /// at the previous iterate), solves each group's loss equation
    /// `laplacian(D_g) + Sigma_{r,g}` against `chi_g/k S_n + S_{s,g}` with the
    /// inner LDU (Gauss-Seidel) solver, then updates
    /// `k^{n+1} = k^n * F^{n+1}/F^n` from the fission-production integral `F`
    /// (GeN-Foam `calcKeff.H`), and renormalises the flux to `F = 1` to keep
    /// the amplitude bounded (GeN-Foam `normFluxes.H`).
    ///
    /// On return the state's `k_eff`, flux, power density, one-group flux, and
    /// total power are all updated. The eigenvector amplitude is arbitrary
    /// (normalised so the fission-production integral is 1); scale afterwards to
    /// a target power if required.
    ///
    /// # Errors
    ///
    /// [`NeutronicsError::NoFissionSource`] if the initial fission production is
    /// zero (a non-multiplying configuration, `k_eff` undefined), or
    /// [`NeutronicsError::NotConverged`] if the outer loop exhausts
    /// `max_outer_iterations` without meeting both tolerances.
    pub fn solve_eigenvalue(&mut self) -> Result<EigenvalueReport, NeutronicsError> {
        let g = self.energy_groups();
        let n = self.state.mesh().n_cells;

        // Precompute the effective fission spectrum chi_g = chi_p,g (1-beta)
        // + chi_d,g beta per cell (prompt + equilibrium-delayed collapse).
        let chi_eff: Vec<Vec<f64>> = (0..g)
            .map(|gg| {
                let cp = self.xs.chi_prompt[gg].internal.as_slice();
                let cd = self.xs.chi_delayed[gg].internal.as_slice();
                let bt = self.xs.beta_tot.internal.as_slice();
                (0..n)
                    .map(|c| cp[c] * (1.0 - bt[c]) + cd[c] * bt[c])
                    .collect()
            })
            .collect();

        // Seed: renormalise the (uniform-1) flux so F = 1.
        let mut s_fis = self.fission_production();
        let mut f_old = self.fission_integral(&s_fis);
        if f_old <= 0.0 {
            return Err(NeutronicsError::NoFissionSource);
        }
        self.scale_flux(1.0 / f_old);
        s_fis = self.fission_production();
        f_old = self.fission_integral(&s_fis); // == 1

        let mut k = self.state.k_eff_raw();
        let mut k_residual = f64::INFINITY;
        let mut flux_residual = f64::INFINITY;
        let mut converged = false;
        let mut outer = 0;

        while outer < self.settings.max_outer_iterations {
            outer += 1;

            // Snapshot the flux to measure the outer flux change.
            let flux_before: Vec<Vec<f64>> = self
                .state
                .flux()
                .iter()
                .map(|f| f.internal.as_slice().to_vec())
                .collect();

            // Fission source lagged at the start-of-iteration flux.
            let s_fis_field = self.fission_production();
            let s_fis_slice = s_fis_field.internal.as_slice().to_vec();

            // Gauss-Seidel sweep over groups (in-scatter uses latest flux).
            for (gg, chi_g) in chi_eff.iter().enumerate() {
                let scatter = self.scattering_source(gg);
                // Explicit RHS source q_g = chi_g/k * S_n + S_{s,g}.
                let q_vals: Vec<f64> = (0..n)
                    .map(|c| chi_g[c] / k * s_fis_slice[c] + scatter[c])
                    .collect();
                let q_field = VolScalarField::uniform("q", self.state.mesh().clone(), 0.0);
                let q_field = with_values(q_field, q_vals);

                let flux_g = &self.state.flux()[gg];
                // Loss operator (symmetric positive-definite: SPD laplacian +
                // positive removal diagonal) minus the explicit fission /
                // in-scatter source — solved with warm-started CG.
                let eqn = fvm::laplacian(&self.xs.d_face[gg], flux_g)
                    + fvm::sp(&self.xs.sigma_removal[gg], flux_g)
                    - fvm::su(&q_field, flux_g);
                let (sol, _perf) =
                    eqn.solve_cg_with_guess(format!("flux{gg}"), flux_g, self.settings.linear);
                self.write_group_flux(gg, &sol);
            }

            // k update from the fission-production integral ratio.
            let s_fis_new = self.fission_production();
            let f_new = self.fission_integral(&s_fis_new);
            let k_new = k * f_new / f_old;
            k_residual = ((k_new - k) / k_new).abs();

            // Renormalise flux so F = 1 for the next iteration.
            if f_new > 0.0 {
                self.scale_flux(1.0 / f_new);
            }
            f_old = 1.0;
            k = k_new;

            // Flux convergence: relative L2 change.
            flux_residual = relative_l2_change(&flux_before, self.state.flux());

            if k_residual < self.settings.k_tolerance
                && flux_residual < self.settings.flux_tolerance
            {
                converged = true;
                break;
            }
        }

        self.state.set_k_eff_raw(k);
        // Equilibrium precursors C_k = beta_k S_n / (k lambda_k).
        self.set_equilibrium_precursors(k);
        self.update_derived_fields();

        let report = EigenvalueReport {
            k_eff: k,
            outer_iterations: outer,
            k_residual,
            flux_residual,
            converged,
        };
        if converged {
            Ok(report)
        } else {
            Err(NeutronicsError::NotConverged {
                outer_iterations: outer,
                k_residual,
                flux_residual,
            })
        }
    }

    /// Scale every group flux by `factor` (renormalisation).
    fn scale_flux(&mut self, factor: f64) {
        for field in self.state.flux_mut() {
            for v in field.internal.as_mut_slice() {
                *v *= factor;
            }
        }
    }

    /// Set the precursors to their eigenvalue equilibrium
    /// `C_k[c] = beta_k[c] S_n[c] / (k lambda_k[c])` (from `precEq.H` with the
    /// time derivative dropped). Used to hand a consistent starting point to a
    /// following transient.
    fn set_equilibrium_precursors(&mut self, k: f64) {
        let s_fis = self.fission_production();
        let s = s_fis.internal.as_slice().to_vec();
        let n = s.len();
        for kk in 0..self.prec_groups() {
            let beta = self.xs.beta[kk].internal.as_slice().to_vec();
            let lambda = self.xs.lambda[kk].internal.as_slice().to_vec();
            let dst = self.state.precursors_mut()[kk].internal.as_mut_slice();
            for c in 0..n {
                dst[c] = if lambda[c] > 0.0 {
                    beta[c] * s[c] / (k * lambda[c])
                } else {
                    0.0
                };
            }
        }
    }
}

/// Overwrite a field's internal values, keeping its boundary.
fn with_values(mut field: VolScalarField, values: Vec<f64>) -> VolScalarField {
    field.internal = Field::new(values);
    field
}

/// Relative L2 change between the previous flux snapshot and the current flux,
/// summed over all groups.
fn relative_l2_change(before: &[Vec<f64>], after: &[VolScalarField]) -> f64 {
    let mut num = 0.0;
    let mut den = 0.0;
    for (b, a) in before.iter().zip(after.iter()) {
        for (bo, ao) in b.iter().zip(a.internal.as_slice()) {
            let d = ao - bo;
            num += d * d;
            den += ao * ao;
        }
    }
    if den > 0.0 {
        (num / den).sqrt()
    } else {
        0.0
    }
}
