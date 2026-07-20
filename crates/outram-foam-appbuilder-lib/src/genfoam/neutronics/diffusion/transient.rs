// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da
//   Upstream source: src/classes/neutronics/diffusion/include/
//                    {createFluxMatrices,fluxEq}.H and
//                    src/classes/neutronics/include/
//                    {precEq,initializeNeutroSource,
//                     initializeDelayedNeutroSource}.H
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

//! One backward-Euler transient time step (see [`super`]).

use outram_foam_basic_lib::prelude::{fvm, Field, VolScalarField};
use uom::si::f64::Time;
use uom::si::time::second;

use super::DiffusionNeutronics;
use crate::genfoam::neutronics::NeutronicsError;

impl DiffusionNeutronics {
    /// Advance the coupled flux + precursor system by one implicit
    /// (backward-Euler) time step at the state's current `k_eff`.
    ///
    /// Each group's balance is assembled as
    /// `ddt(IV_g) + laplacian(D_g) + Sp(Sigma_{r,g}) - Sp(chi_{p,g}/k(1-beta)
    /// nu Sigma_{f,g}) = chi_{p,g}/k(1-beta) S_n^{other} + chi_{d,g} S_d +
    /// S_{s,g}` — the own-group prompt fission is implicit (moved to the
    /// diagonal, GeN-Foam `createFluxMatrices.H`), the other-group prompt
    /// fission, the delayed source `S_d` (from the previous step's precursors),
    /// and the in-scatter are explicit. Precursors are then advanced by the
    /// closed-form per-cell update of `dC_k/dt + lambda_k C_k = beta_k/k S_n`
    /// (GeN-Foam `precEq.H`, non-liquid-fuel branch) with the freshly updated
    /// fission source.
    ///
    /// `k_eff` is held fixed (set it with the state, or from a preceding
    /// [`Self::solve_eigenvalue`]): at the eigenvalue and starting from the
    /// eigenvector + equilibrium precursors, this is the exact steady solution,
    /// so a null transient holds steady. This single-pass scheme lags the
    /// other-group / delayed / scatter sources by one step; the coupled-outer
    /// neutron iterations (`maxNeutronIterations`) and the implicit/integral
    /// predictors of the full GeN-Foam loop are deferred.
    ///
    /// # Errors
    ///
    /// [`NeutronicsError::NonPositiveTimeStep`] if `dt <= 0`.
    pub fn step(&mut self, dt: Time) -> Result<(), NeutronicsError> {
        let dt_s = dt.get::<second>();
        if dt_s <= 0.0 || dt_s.is_nan() {
            return Err(NeutronicsError::NonPositiveTimeStep(dt_s));
        }
        let n = self.state.mesh().n_cells;
        let k = self.state.k_eff_raw();

        // Snapshots of the old flux and precursors (the "old" values of the
        // backward-Euler step).
        let flux_old: Vec<VolScalarField> = self.state.flux().to_vec();
        let prec_old: Vec<VolScalarField> = self.state.precursors().to_vec();

        // Fission production and delayed source from the old state.
        let s_fis_old = self.fission_production();
        let s_fis_old = s_fis_old.internal.as_slice().to_vec();
        let s_delayed = self.delayed_source(); // sum_k lambda_k C_k (old)

        for (gg, flux_g_old_field) in flux_old.iter().enumerate() {
            let scatter = self.scattering_source(gg);
            let cp = self.xs.chi_prompt[gg].internal.as_slice().to_vec();
            let cd = self.xs.chi_delayed[gg].internal.as_slice().to_vec();
            let bt = self.xs.beta_tot.internal.as_slice().to_vec();
            let nf = self.xs.nu_sigma_f[gg].internal.as_slice().to_vec();
            let phi_g_old = flux_g_old_field.internal.as_slice().to_vec();

            // Own-group implicit prompt-fission coefficient c_g * nuSigmaF_g,
            // and the explicit source q_g.
            let mut cg_nf = vec![0.0; n];
            let mut q = vec![0.0; n];
            for c in 0..n {
                let c_g = cp[c] / k * (1.0 - bt[c]);
                let s_n_other = s_fis_old[c] - nf[c] * phi_g_old[c];
                cg_nf[c] = c_g * nf[c];
                q[c] = c_g * s_n_other + cd[c] * s_delayed[c] + scatter[c];
            }
            let cg_nf_field = with_values(self.template(), cg_nf);
            let q_field = with_values(self.template(), q);

            let flux_g = &self.state.flux()[gg];
            // Symmetric matrix (ddt + SPD laplacian + removal diagonal − own-
            // group prompt-fission diagonal); the transient time term keeps the
            // diagonal dominant, so warm-started CG applies.
            let eqn = fvm::ddt_coeff(&self.xs.inv_velocity[gg], flux_g, flux_g_old_field, dt_s)
                + fvm::laplacian(&self.xs.d_face[gg], flux_g)
                + fvm::sp(&self.xs.sigma_removal[gg], flux_g)
                - fvm::sp(&cg_nf_field, flux_g)
                - fvm::su(&q_field, flux_g);
            let (sol, _perf) =
                eqn.solve_cg_with_guess(format!("flux{gg}"), flux_g, self.settings.linear);
            self.write_group_flux(gg, &sol);
        }

        // Advance precursors with the freshly updated fission source.
        let s_fis_new = self.fission_production();
        let s_new = s_fis_new.internal.as_slice().to_vec();
        for (kk, prec_old_field) in prec_old.iter().enumerate() {
            let beta = self.xs.beta[kk].internal.as_slice().to_vec();
            let lambda = self.xs.lambda[kk].internal.as_slice().to_vec();
            let c_old = prec_old_field.internal.as_slice().to_vec();
            let dst = self.state.precursors_mut()[kk].internal.as_mut_slice();
            for c in 0..n {
                dst[c] = (c_old[c] / dt_s + beta[c] / k * s_new[c]) / (1.0 / dt_s + lambda[c]);
            }
        }

        self.update_derived_fields();
        Ok(())
    }

    /// A zero-valued `VolScalarField` on the mesh, used as a coefficient-field
    /// template (its boundary is irrelevant for `Sp` / `Su` source terms).
    fn template(&self) -> VolScalarField {
        VolScalarField::uniform("coeff", self.state.mesh().clone(), 0.0)
    }
}

/// Overwrite a field's internal values, keeping its boundary.
fn with_values(mut field: VolScalarField, values: Vec<f64>) -> VolScalarField {
    field.internal = Field::new(values);
    field
}
