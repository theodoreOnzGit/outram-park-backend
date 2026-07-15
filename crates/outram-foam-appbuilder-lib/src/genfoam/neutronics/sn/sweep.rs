// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da
//   Upstream source: src/classes/neutronics/SN/include/
//     {fluxEqSN,calcSourceSN,calcThisIntraSource,updateScalarFluxes,
//      updateThisScalarFlux}.H — the within-group scattering-source (inner)
//     iteration and the per-direction transport solve.
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//     Principal authors: Carlo Fiorina, Thomas Guilbaud (EPFL)
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

//! The within-group transport sweep and scattering-source inner iteration
//! (see [`super`]).

use std::f64::consts::PI;

use outram_foam_basic_lib::prelude::{fvm, VolScalarField};

use super::{SnNeutronics, WithInternal};

impl SnNeutronics {
    /// Scattering-source inner iteration for energy group `i` at the current
    /// eigenvalue `k` and lagged fission production `s_fis`.
    ///
    /// Repeats, up to [`super::SnSettings::max_inner_iterations`] times:
    ///
    /// 1. build the isotropic source `q_i / (4*pi)` from the (fixed) fission
    ///    source `chi_i / k * S_n` plus the in-scatter `sum_{i'} Sigma_{s,i'->i}
    ///    phi_{i'}` evaluated at the **latest** scalar flux;
    /// 2. solve the streaming + collision equation `div(Omega_j psi) +
    ///    Sigma_{t,i} psi = q_i / (4*pi)` for every direction `j` (first-order
    ///    upwind [`fvm::div`], Gauss-Seidel LDU solve — the transport matrix is
    ///    asymmetric so CG is not applicable);
    /// 3. re-sum the scalar flux `phi_i = sum_j w_j psi_{i,j}`.
    ///
    /// It stops when the relative-L2 change in `phi_i` falls below
    /// [`super::SnSettings::inner_tolerance`]. With no scattering (`Sigma_s = 0`)
    /// the source is fixed and it converges in a single pass. Returns the final
    /// inner residual.
    pub(super) fn sweep_group(
        &mut self,
        i: usize,
        k: f64,
        s_fis: &[f64],
        chi_eff_i: &[f64],
    ) -> f64 {
        let mesh = self.state.mesh().clone();
        let n = mesh.n_cells;
        let settings = self.settings;
        let n_dir = self.solver_data().quadrature.direction_count();
        let inv_four_pi = 1.0 / (4.0 * PI);

        let mut inner_residual = f64::INFINITY;

        for _inner in 0..settings.max_inner_iterations.max(1) {
            // In-scatter from the latest scalar flux (includes self-scatter).
            let scatter = self.scattering_into(i);

            // Isotropic source per direction: q_i / (4*pi).
            let q_vals: Vec<f64> = (0..n)
                .map(|c| (chi_eff_i[c] / k * s_fis[c] + scatter[c]) * inv_four_pi)
                .collect();
            let q_field = VolScalarField::uniform("qSN", mesh.clone(), 0.0).with_internal(q_vals);

            // Snapshot this group's scalar flux to measure inner convergence.
            let before: Vec<f64> = self.state.flux()[i].internal.as_slice().to_vec();

            // Transport solve for every direction.
            for j in 0..n_dir {
                let solver = self
                    .solver
                    .as_mut()
                    .expect("SN solver data present on a working model");
                let eqn = fvm::div(&solver.face_phi[j], &solver.angular_flux[i][j])
                    + fvm::sp(&solver.sigma_t[i], &solver.angular_flux[i][j])
                    - fvm::su(&q_field, &solver.angular_flux[i][j]);
                let (sol, _perf) = eqn.solve(format!("psi{i}_{j}"), settings.linear);
                let dst = solver.angular_flux[i][j].internal.as_mut_slice();
                for (d, s) in dst.iter_mut().zip(sol.internal.as_slice()) {
                    *d = *s;
                }
            }

            // Re-sum the scalar flux phi_i = sum_j w_j psi_{i,j}.
            let new_flux = {
                let solver = self.solver.as_ref().expect("SN solver data present");
                let weights = solver.quadrature.weights();
                let mut f = vec![0.0; n];
                for (psi_field, &w) in solver.angular_flux[i].iter().zip(weights) {
                    let psi = psi_field.internal.as_slice();
                    for c in 0..n {
                        f[c] += w * psi[c];
                    }
                }
                f
            };
            {
                let dst = self.state.flux_mut()[i].internal.as_mut_slice();
                for (d, s) in dst.iter_mut().zip(&new_flux) {
                    *d = *s;
                }
            }

            inner_residual = relative_l2_change(&before, &new_flux);
            if inner_residual < settings.inner_tolerance {
                break;
            }
        }

        inner_residual
    }
}

/// Relative-L2 change `||after - before|| / ||after||` between two per-cell
/// vectors (returns 0 when `after` is all-zero).
fn relative_l2_change(before: &[f64], after: &[f64]) -> f64 {
    let mut num = 0.0;
    let mut den = 0.0;
    for (b, a) in before.iter().zip(after.iter()) {
        let d = a - b;
        num += d * d;
        den += a * a;
    }
    if den > 0.0 {
        (num / den).sqrt()
    } else {
        0.0
    }
}
