// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da
//   Upstream source: src/classes/neutronics/SP3/include/
//                    {createFluxMatricesSP3,fluxEqSP3,solveNeutronicsSP3,
//                     normFluxesSP3}.H and
//                    src/classes/neutronics/include/calcKeff.H
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//     Principal author of the SP3 files: Carlo Fiorina (EPFL).
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

//! The SP3 k-eigenvalue outer power iteration (see [`super`]).

use outram_foam_basic_lib::prelude::{fvm, VolScalarField};

use super::{with_values, Sp3Neutronics, Sp3XsFields};
use crate::genfoam::neutronics::{EigenvalueReport, NeutronicsError};

impl Sp3Neutronics {
    /// Solve the SP3 k-eigenvalue problem by outer power iteration.
    ///
    /// Each outer iteration holds the fission source lagged at the previous
    /// physical flux, then sweeps the groups: for group `g` it solves the 0th
    /// composite-moment equation
    /// `laplacian(D0_g) + Sp(Sigma_{r,g})` against
    /// `Q_g + 2 Sigma_{r,g} phi2_g`, then the 2nd-moment equation
    /// `laplacian(D2_g) + Sp(A2_g)` against
    /// `(2/3)(Sigma_{r,g} Phi0_g - Q_g)`, and reconstructs the physical flux
    /// `phi0_g = Phi0_g - 2 phi2_g` (GeN-Foam `fluxEqSP3.H`). Both moment
    /// operators are symmetric positive-definite, so they solve with
    /// warm-started CG. It then updates `k^{n+1} = k^n F^{n+1}/F^n` from the
    /// fission-production integral `F` (GeN-Foam `calcKeff.H`) and renormalises
    /// the moments so `F = 1` (`normFluxesSP3.H`).
    ///
    /// On return the state's `k_eff`, physical flux, power density, one-group
    /// flux, and total power — and the SP3 moment fields — are all updated.
    ///
    /// # Errors
    ///
    /// [`NeutronicsError::ModelNotImplemented`] if this is a state-only scaffold
    /// (built with [`Self::new`] rather than [`Self::with_cross_sections`]);
    /// [`NeutronicsError::NoFissionSource`] if the initial fission production is
    /// zero; or [`NeutronicsError::NotConverged`] if the outer loop exhausts
    /// `max_outer_iterations`.
    pub fn solve_eigenvalue(&mut self) -> Result<EigenvalueReport, NeutronicsError> {
        // Take the cross sections out so the solve can freely borrow `self`
        // mutably; restore them before returning.
        let xs = self.xs.take().ok_or(NeutronicsError::ModelNotImplemented(
            crate::genfoam::neutronics::NeutronicsModelKind::Sp3,
        ))?;
        let result = self.solve_eigenvalue_inner(&xs);
        self.xs = Some(xs);
        result
    }

    /// The eigenvalue power iteration with the cross sections borrowed by value
    /// from the caller (see [`Self::solve_eigenvalue`]).
    fn solve_eigenvalue_inner(
        &mut self,
        xs: &Sp3XsFields,
    ) -> Result<EigenvalueReport, NeutronicsError> {
        let g = self.energy_groups();
        let n = self.state.mesh().n_cells;

        // Effective fission spectrum chi_g = chi_p,g (1-beta) + chi_d,g beta.
        let chi_eff: Vec<Vec<f64>> = (0..g)
            .map(|gg| {
                let cp = xs.base.chi_prompt[gg].internal.as_slice();
                let cd = xs.base.chi_delayed[gg].internal.as_slice();
                let bt = xs.base.beta_tot.internal.as_slice();
                (0..n)
                    .map(|c| cp[c] * (1.0 - bt[c]) + cd[c] * bt[c])
                    .collect()
            })
            .collect();

        // Seed: renormalise the flux so F = 1.
        let mut f_old = self.fission_integral(&self.fission_production(xs));
        if f_old <= 0.0 {
            return Err(NeutronicsError::NoFissionSource);
        }
        self.scale_moments(1.0 / f_old);
        self.reconstruct_flux();
        f_old = 1.0;

        let mut k = self.state.k_eff_raw();
        let mut k_residual = f64::INFINITY;
        let mut flux_residual = f64::INFINITY;
        let mut converged = false;
        let mut outer = 0;

        while outer < self.settings.max_outer_iterations {
            outer += 1;

            let flux_before: Vec<Vec<f64>> = self
                .state
                .flux()
                .iter()
                .map(|f| f.internal.as_slice().to_vec())
                .collect();

            // Fission source lagged at the start-of-iteration physical flux.
            let s_fis = self.fission_production(xs);

            for (gg, chi_g) in chi_eff.iter().enumerate() {
                let scatter = self.scattering_source(xs, gg);
                let sr = xs.base.sigma_removal[gg].internal.as_slice().to_vec();

                // Source Q_g = chi_g/k S_n + S_{s,g}.
                let q: Vec<f64> = (0..n)
                    .map(|c| chi_g[c] / k * s_fis[c] + scatter[c])
                    .collect();

                // ── 0th composite moment: [lap(D0)+Sp(Sr)] Phi0 = Q + 2 Sr phi2
                let phi2 = self.flux_star2[gg].internal.as_slice().to_vec();
                let rhs1: Vec<f64> = (0..n).map(|c| q[c] + 2.0 * sr[c] * phi2[c]).collect();
                let rhs1_field = with_values(self.template(), rhs1);
                {
                    let phi0 = &self.flux_star[gg];
                    let eqn = fvm::laplacian(&xs.base.d_face[gg], phi0)
                        + fvm::sp(&xs.base.sigma_removal[gg], phi0)
                        - fvm::su(&rhs1_field, phi0);
                    let (sol, _perf) = eqn.solve_cg_with_guess(
                        format!("fluxStar{gg}"),
                        phi0,
                        self.settings.linear,
                    );
                    Self::write_moment(&mut self.flux_star[gg], &sol);
                }

                // ── 2nd moment: [lap(D2)+Sp(A2)] phi2 = (2/3)(Sr Phi0 - Q)
                let phi0_new = self.flux_star[gg].internal.as_slice().to_vec();
                let rhs2: Vec<f64> = (0..n)
                    .map(|c| (2.0 / 3.0) * (sr[c] * phi0_new[c] - q[c]))
                    .collect();
                let rhs2_field = with_values(self.template(), rhs2);
                {
                    let phi2f = &self.flux_star2[gg];
                    let eqn = fvm::laplacian(&xs.d2_face[gg], phi2f)
                        + fvm::sp(&xs.second_moment_removal[gg], phi2f)
                        - fvm::su(&rhs2_field, phi2f);
                    let (sol, _perf) = eqn.solve_cg_with_guess(
                        format!("fluxStar2{gg}"),
                        phi2f,
                        self.settings.linear,
                    );
                    Self::write_moment(&mut self.flux_star2[gg], &sol);
                }

                // Reconstruct this group's physical flux so later groups'
                // in-scatter sees the update (Gauss-Seidel).
                let phi0 = self.flux_star[gg].internal.as_slice().to_vec();
                let phi2 = self.flux_star2[gg].internal.as_slice().to_vec();
                let dst = self.state.flux_mut()[gg].internal.as_mut_slice();
                for c in 0..n {
                    dst[c] = phi0[c] - 2.0 * phi2[c];
                }
            }

            // k update from the fission-production integral ratio.
            let f_new = self.fission_integral(&self.fission_production(xs));
            let k_new = k * f_new / f_old;
            k_residual = ((k_new - k) / k_new).abs();

            if f_new > 0.0 {
                self.scale_moments(1.0 / f_new);
                self.reconstruct_flux();
            }
            f_old = 1.0;
            k = k_new;

            flux_residual = relative_l2_change(&flux_before, self.state.flux());

            if k_residual < self.settings.k_tolerance
                && flux_residual < self.settings.flux_tolerance
            {
                converged = true;
                break;
            }
        }

        self.state.set_k_eff_raw(k);
        self.set_equilibrium_precursors(xs, k);
        self.update_derived_fields(xs);

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

    /// Scale both SP3 moment fields (`Phi0` and `phi2`) of every group by
    /// `factor` (renormalisation — `normFluxesSP3.H`). The reconstructed physical
    /// flux scales by the same factor since it is their linear combination.
    fn scale_moments(&mut self, factor: f64) {
        for field in self.flux_star.iter_mut() {
            for v in field.internal.as_mut_slice() {
                *v *= factor;
            }
        }
        for field in self.flux_star2.iter_mut() {
            for v in field.internal.as_mut_slice() {
                *v *= factor;
            }
        }
    }

    /// Set the precursors to their eigenvalue equilibrium
    /// `C_k[c] = beta_k[c] S_n[c] / (k lambda_k[c])` (from `precEq.H` with the
    /// time derivative dropped) for a consistent transient start point.
    fn set_equilibrium_precursors(&mut self, xs: &Sp3XsFields, k: f64) {
        let s = self.fission_production(xs);
        let n = s.len();
        for kk in 0..self.prec_groups() {
            let beta = xs.base.beta[kk].internal.as_slice().to_vec();
            let lambda = xs.base.lambda[kk].internal.as_slice().to_vec();
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

    /// A zero-valued `VolScalarField` on the mesh, used as a source-field
    /// template for [`with_values`].
    pub(crate) fn template(&self) -> VolScalarField {
        VolScalarField::uniform("q", self.state.mesh().clone(), 0.0)
    }
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
