// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da
//   Upstream source: src/classes/neutronics/SP3/include/
//                    {createFluxMatricesSP3,fluxEqSP3}.H and
//                    src/classes/neutronics/include/
//                    {precEq,initializeNeutroSource,
//                     initializeDelayedNeutroSource}.H
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

//! One backward-Euler transient time step of the SP3 moment + precursor system
//! (see [`super`]).

use outram_foam_basic_lib::prelude::{fvm, VolScalarField};
use uom::si::f64::Time;
use uom::si::time::second;

use super::{with_values, Sp3Neutronics, Sp3XsFields};
use crate::genfoam::neutronics::{NeutronicsError, NeutronicsModelKind};

impl Sp3Neutronics {
    /// Advance the coupled SP3 moment + precursor system by one implicit
    /// (backward-Euler) time step at the state's current `k_eff`.
    ///
    /// Both moment equations carry the SP3 time terms
    /// (`ddt((1-eig) IV, Phi0)` and `ddt(3 (1-eig) IV, phi2)`) plus the
    /// cross-moment `fvc::ddt` coupling from GeN-Foam `fluxEqSP3.H`. The prompt
    /// fission is taken at the frozen `k_eff` with the delayed source from the
    /// previous step's precursors; the in-scatter and cross-moment terms are
    /// lagged one step (single-pass, matching the diffusion transient). The
    /// precursors are then advanced by the closed-form per-cell update of
    /// `dC_k/dt + lambda_k C_k = beta_k/k S_n` (GeN-Foam `precEq.H`, non-liquid
    /// branch) with the freshly updated fission source.
    ///
    /// `k_eff` is held fixed: at the eigenvalue and starting from the eigenvector
    /// with equilibrium precursors this is the exact steady solution, so a null
    /// transient holds steady (see the V&V tests). The implicit/integral
    /// predictors, Aitken acceleration, and the multi-pass neutron sub-iterations
    /// of the full GeN-Foam loop are deferred.
    ///
    /// # Errors
    ///
    /// [`NeutronicsError::ModelNotImplemented`] for a state-only scaffold, or
    /// [`NeutronicsError::NonPositiveTimeStep`] if `dt <= 0`.
    pub fn step(&mut self, dt: Time) -> Result<(), NeutronicsError> {
        let dt_s = dt.get::<second>();
        if dt_s <= 0.0 || dt_s.is_nan() {
            return Err(NeutronicsError::NonPositiveTimeStep(dt_s));
        }
        let xs = self.xs.take().ok_or(NeutronicsError::ModelNotImplemented(
            NeutronicsModelKind::Sp3,
        ))?;
        let result = self.step_inner(&xs, dt_s);
        self.xs = Some(xs);
        result
    }

    /// The transient step with the cross sections borrowed by value from the
    /// caller (see [`Self::step`]).
    fn step_inner(&mut self, xs: &Sp3XsFields, dt_s: f64) -> Result<(), NeutronicsError> {
        let n = self.state.mesh().n_cells;
        let k = self.state.k_eff_raw();

        // Old (previous-step) moment + precursor snapshots.
        let star_old: Vec<VolScalarField> = self.flux_star.to_vec();
        let star2_old: Vec<VolScalarField> = self.flux_star2.to_vec();
        let prec_old: Vec<VolScalarField> = self.state.precursors().to_vec();

        // Lagged sources from the old state.
        let s_fis_old = self.fission_production(xs);
        let s_delayed = self.delayed_source(xs);

        for gg in 0..self.energy_groups() {
            let scatter = self.scattering_source(xs, gg);
            let cp = xs.base.chi_prompt[gg].internal.as_slice().to_vec();
            let cd = xs.base.chi_delayed[gg].internal.as_slice().to_vec();
            let bt = xs.base.beta_tot.internal.as_slice().to_vec();
            let sr = xs.base.sigma_removal[gg].internal.as_slice().to_vec();
            let iv = xs.base.inv_velocity[gg].internal.as_slice().to_vec();
            let phi2_old = star2_old[gg].internal.as_slice().to_vec();
            let phi0_old = star_old[gg].internal.as_slice().to_vec();

            // Source Q_g = (1-beta) chi_p/k S_n + chi_d S_d + S_{s,g}.
            let q: Vec<f64> = (0..n)
                .map(|c| {
                    (1.0 - bt[c]) * cp[c] / k * s_fis_old[c] + cd[c] * s_delayed[c] + scatter[c]
                })
                .collect();

            // ── 0th composite moment:
            //   ddt(IV, Phi0) + lap(D0) + Sp(Sr) = Q + 2 Sr phi2_old
            //   (the cross fvc::ddt(IV, 2 phi2) term is 0 on the single pass,
            //    since phi2 has not yet been updated).
            let rhs1: Vec<f64> = (0..n).map(|c| q[c] + 2.0 * sr[c] * phi2_old[c]).collect();
            let rhs1_field = with_values(self.template(), rhs1);
            {
                let phi0 = &self.flux_star[gg];
                let eqn = fvm::ddt_coeff(&xs.base.inv_velocity[gg], phi0, &star_old[gg], dt_s)
                    + fvm::laplacian(&xs.base.d_face[gg], phi0)
                    + fvm::sp(&xs.base.sigma_removal[gg], phi0)
                    - fvm::su(&rhs1_field, phi0);
                let (sol, _perf) =
                    eqn.solve_cg_with_guess(format!("fluxStar{gg}"), phi0, self.settings.linear);
                Self::write_moment(&mut self.flux_star[gg], &sol);
            }

            // ── 2nd moment:
            //   ddt(3 IV, phi2) + lap(D2) + Sp(A2)
            //     = (2/3)(Sr Phi0_new - Q) + (2/3) IV/dt (Phi0_new - Phi0_old)
            let phi0_new = self.flux_star[gg].internal.as_slice().to_vec();
            let rhs2: Vec<f64> = (0..n)
                .map(|c| {
                    (2.0 / 3.0) * (sr[c] * phi0_new[c] - q[c])
                        + (2.0 / 3.0) * iv[c] / dt_s * (phi0_new[c] - phi0_old[c])
                })
                .collect();
            let rhs2_field = with_values(self.template(), rhs2);
            let three_iv = with_values(self.template(), iv.iter().map(|v| 3.0 * v).collect());
            {
                let phi2f = &self.flux_star2[gg];
                let eqn = fvm::ddt_coeff(&three_iv, phi2f, &star2_old[gg], dt_s)
                    + fvm::laplacian(&xs.d2_face[gg], phi2f)
                    + fvm::sp(&xs.second_moment_removal[gg], phi2f)
                    - fvm::su(&rhs2_field, phi2f);
                let (sol, _perf) =
                    eqn.solve_cg_with_guess(format!("fluxStar2{gg}"), phi2f, self.settings.linear);
                Self::write_moment(&mut self.flux_star2[gg], &sol);
            }

            // Reconstruct this group's physical flux.
            let phi0 = self.flux_star[gg].internal.as_slice().to_vec();
            let phi2 = self.flux_star2[gg].internal.as_slice().to_vec();
            let dst = self.state.flux_mut()[gg].internal.as_mut_slice();
            for c in 0..n {
                dst[c] = phi0[c] - 2.0 * phi2[c];
            }
        }

        // Advance precursors with the freshly updated fission source.
        let s_new = self.fission_production(xs);
        for (kk, prec_old_field) in prec_old.iter().enumerate() {
            let beta = xs.base.beta[kk].internal.as_slice().to_vec();
            let lambda = xs.base.lambda[kk].internal.as_slice().to_vec();
            let c_old = prec_old_field.internal.as_slice().to_vec();
            let dst = self.state.precursors_mut()[kk].internal.as_mut_slice();
            for c in 0..n {
                dst[c] = (c_old[c] / dt_s + beta[c] / k * s_new[c]) / (1.0 / dt_s + lambda[c]);
            }
        }

        self.update_derived_fields(xs);
        Ok(())
    }
}
