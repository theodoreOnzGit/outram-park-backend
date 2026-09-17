// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da
//   Upstream source: src/classes/neutronics/SN/include/
//     {solveNeutronicsSN,normFluxesSN}.H and
//     src/classes/neutronics/include/{initializeNeutroSource,calcKeff}.H —
//     the k-eigenvalue outer power iteration on the fission source.
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//     Principal authors: Carlo Fiorina, Konstantin Mikityuk (EPFL)
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

//! The k-eigenvalue outer power iteration for the S_N solver (see [`super`]).

use super::SnNeutronics;
use crate::genfoam::neutronics::{EigenvalueReport, NeutronicsError};

impl SnNeutronics {
    /// Solve the discrete-ordinates k-eigenvalue problem by outer power
    /// iteration.
    ///
    /// Each outer iteration:
    ///
    /// 1. holds the fission production `S_n = sum_i nu Sigma_{f,i} phi_i` fixed
    ///    (lagged at the start-of-iteration scalar flux);
    /// 2. sweeps the groups in ascending order, running the scattering-source
    ///    inner iteration ([`Self::sweep_group`]) for each — a group's updated
    ///    scalar flux feeds the in-scatter source of later groups
    ///    (Gauss-Seidel over groups);
    /// 3. updates `k <- k * F_new / F_old` from the fission-production integral
    ///    ratio (GeN-Foam `calcKeff.H`) and renormalises every scalar and
    ///    angular flux so `F = 1` (GeN-Foam `normFluxesSN.H`).
    ///
    /// It converges when both the relative change in `k` and the relative-L2
    /// change in the flux fall below their tolerances. The eigenvector amplitude
    /// is normalised so the fission-production integral is 1.
    ///
    /// # Errors
    ///
    /// [`NeutronicsError::NoFissionSource`] if the initial fission production is
    /// zero, or [`NeutronicsError::NotConverged`] if the outer loop exhausts
    /// [`super::SnSettings::max_outer_iterations`].
    pub(super) fn solve_eigenvalue_transport(
        &mut self,
    ) -> Result<EigenvalueReport, NeutronicsError> {
        let g = self.energy_groups();
        let n = self.state.mesh().n_cells;

        // Effective fission spectrum chi_i = chi_p,i (1 - beta) + chi_d,i beta
        // (prompt + equilibrium-delayed collapse), per cell.
        let chi_eff: Vec<Vec<f64>> = (0..g)
            .map(|i| {
                let xs = &self.solver_data().xs;
                let cp = xs.chi_prompt[i].internal.as_slice();
                let cd = xs.chi_delayed[i].internal.as_slice();
                let bt = xs.beta_tot.internal.as_slice();
                (0..n)
                    .map(|c| cp[c] * (1.0 - bt[c]) + cd[c] * bt[c])
                    .collect()
            })
            .collect();

        // Seed: renormalise the (uniform-1) flux so F = 1.
        let s_fis = self.fission_production();
        let mut f_old = self.fission_integral(&s_fis);
        if f_old <= 0.0 {
            return Err(NeutronicsError::NoFissionSource);
        }
        self.scale_flux(1.0 / f_old);
        f_old = 1.0;

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

            // Fission source lagged at the start-of-iteration scalar flux.
            let s_fis = self.fission_production();

            // Gauss-Seidel sweep over groups (in-scatter uses the latest flux).
            for (i, chi_eff_i) in chi_eff.iter().enumerate() {
                self.sweep_group(i, k, &s_fis, chi_eff_i);
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

            flux_residual = relative_l2_change(&flux_before, self.state.flux());

            if k_residual < self.settings.k_tolerance
                && flux_residual < self.settings.flux_tolerance
            {
                converged = true;
                break;
            }
        }

        self.state.set_k_eff_raw(k);
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

    /// Scale every scalar and angular flux by `factor` (renormalisation).
    fn scale_flux(&mut self, factor: f64) {
        for field in self.state.flux_mut() {
            for v in field.internal.as_mut_slice() {
                *v *= factor;
            }
        }
        if let Some(solver) = self.solver.as_mut() {
            for group in solver.angular_flux.iter_mut() {
                for field in group.iter_mut() {
                    for v in field.internal.as_mut_slice() {
                        *v *= factor;
                    }
                }
            }
        }
    }
}

/// Relative-L2 change between the previous flux snapshot and the current flux,
/// summed over all groups.
fn relative_l2_change(
    before: &[Vec<f64>],
    after: &[outram_foam_basic_lib::prelude::VolScalarField],
) -> f64 {
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
