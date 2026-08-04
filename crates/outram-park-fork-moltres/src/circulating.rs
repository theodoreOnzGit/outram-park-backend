// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Physics formulation derived from Moltres (MSR multiphysics on MOOSE)
//   Upstream: https://github.com/arfc/moltres (UIUC ARFC group)
//   Upstream commit: 3dd2ce7
//   Upstream sources consulted (formulation only, no code reused):
//     src/kernels/CoupledFissionKernel.C  (prompt production
//       chi_p (1-beta)/k sum nuSigma_f phi)
//     src/kernels/DelayedNeutronSource.C  (chi_d sum_i lambda_i C_i)
//     src/kernels/{PrecursorSource,PrecursorDecay,CoupledScalarAdvection}.C
//       (the advected precursor balance — see `crate::precursors`)
//   Upstream license: LGPL-2.1, incorporated into this GPL-3.0 crate under
//   the LGPL-2.1 section 3 GPL-conversion option.
//
// Finite-volume assembly built on outram-foam-basic-lib; the outer power
// iteration mirrors the in-workspace GeN-Foam-derived static solver
// (outram-foam-appbuilder-lib::genfoam::neutronics::diffusion, GPL-3.0),
// extended with the advected precursor solve that the appbuilder port
// explicitly defers ("liquid-fuel precursor advection").
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

//! Circulating-fuel k-eigenvalue: multigroup diffusion coupled to advected
//! delayed-neutron precursors — **the** MSRE effect.
//!
//! The coupled steady eigenvalue system solved here is, per energy group
//! `g` and precursor family `i`:
//!
//! ```text
//!   -div(D_g grad phi_g) + Sigma_{r,g} phi_g
//!       = chi_{p,g} (1-beta)/k * S_f + chi_{d,g} S_d + S_{s,g}
//!
//!   div(u C_i) - div(D_C grad C_i) + lambda_i C_i = beta_i/k * S_f
//! ```
//!
//! with `S_f = sum_g nuSigma_{f,g} phi_g`, `S_d = sum_i lambda_i C_i`,
//! `S_{s,g}` the in-scatter, and `u` the prescribed fuel-salt loop velocity.
//! When `u = 0` the precursor balance is the algebraic equilibrium and the
//! system reduces **exactly** to [`crate::diffusion::StaticDiffusion`]
//! (verified in tests). When `u > 0`, precursors drift out of the core and
//! decay in the external loop where their delayed neutrons cannot sustain
//! the chain reaction: `k_eff` (and hence the effective delayed fraction)
//! drops with loop speed. The reactivity difference
//! `rho(0) - rho(u)` is the classic "reactivity loss due to fuel
//! circulation" measured on the MSRE (~0.2 % dk/k at nominal flow).
//!
//! Outer iteration per step: (1) solve every precursor family's steady
//! drift equation against the lagged fission source, (2) solve every
//! group's diffusion equation with prompt + delayed + in-scatter sources,
//! (3) update `k` from the fission-production integral ratio and
//! renormalise. Temperature feedback enters through
//! [`CirculatingFuelSolver::set_temperature`] (reduced linear
//! `dSigma_r/dT` model, see [`crate::materials`]).
//!
//! Units: flux `1/(m^2 s)` (normalised), `C_i` `1/m^3`, `u` `m/s`
//! (`flow` as face flux `m^3/s`), `k_eff` dimensionless.

use std::sync::Arc;

use outram_foam_basic_lib::prelude::{fvm, BoundaryCondition, FvMesh, VolScalarField};

use crate::diffusion::{
    fission_integral, fission_source, overwrite_internal, relative_l2_change, scale_fields,
    scattering_source, seeded_flux, snapshot, EigenReport, EigenSettings,
};
use crate::error::MoltresError;
use crate::materials::{
    scalar_field, DelayedFamily, FaceFluxField, NeutronFluxField, PrecursorField, TemperatureField,
    XsFields,
};
use crate::precursors::PrecursorDrift;

/// Coupled flux + advected-precursor k-eigenvalue model (see module docs).
///
/// Build with [`CirculatingFuelSolver::new`], optionally apply a
/// temperature field with [`Self::set_temperature`], then call
/// [`Self::solve`]. Converged results stay in [`Self::flux`],
/// [`Self::precursors`], and [`Self::k_eff`] (flux normalised so the
/// fission-production integral is 1).
#[derive(Debug, Clone)]
pub struct CirculatingFuelSolver {
    mesh: Arc<FvMesh>,
    xs: XsFields,
    drift: PrecursorDrift,
    beta_total: f64,
    /// Effective removal cross sections (`1/m`), equal to the base
    /// `xs.sigma_removal` until [`Self::set_temperature`] applies feedback.
    sigma_removal_eff: Vec<VolScalarField>,
    /// Group flux fields `phi_g` (`1/(m^2 s)`, normalised).
    pub flux: Vec<NeutronFluxField>,
    /// Precursor concentration fields `C_i` (`1/m^3`), one per family;
    /// consistent with `flux` and `k_eff` after [`Self::solve`].
    pub precursors: Vec<PrecursorField>,
    /// Latest `k_eff` (dimensionless); 1.0 before the first solve.
    pub k_eff: f64,
    settings: EigenSettings,
}

impl CirculatingFuelSolver {
    /// Build a circulating-fuel model.
    ///
    /// - `xs` — materialised cross sections on the loop mesh.
    /// - `families` — delayed-neutron families (must be non-empty; the
    ///   whole point is the delayed chain).
    /// - `flow` — face volumetric flux `u . A_f` (`m^3/s`), e.g.
    ///   [`crate::ring_mesh::RingMesh::uniform_flux`]. Zero = static fuel.
    /// - `precursor_diffusion` — `D_C` in `m^2/s` (small, >= 0).
    /// - `flux_boundary` — one BC per mesh patch for every group flux
    ///   (`&[]` on a boundary-free ring mesh).
    ///
    /// # Errors
    /// Size/validity errors from the precursor model or the flux boundary
    /// list (see [`PrecursorDrift::new`]).
    pub fn new(
        xs: XsFields,
        families: Vec<DelayedFamily>,
        flow: FaceFluxField,
        precursor_diffusion: f64,
        flux_boundary: &[BoundaryCondition<f64>],
        settings: EigenSettings,
    ) -> Result<Self, MoltresError> {
        let mesh = xs.mesh.clone();
        let beta_total = DelayedFamily::total_beta(&families);
        let drift = PrecursorDrift::new(
            mesh.clone(),
            families,
            flow,
            precursor_diffusion,
            settings.linear,
        )?;
        let flux = seeded_flux(&mesh, xs.energy_groups, flux_boundary)?;
        let precursors = (0..drift.families.len())
            .map(|i| VolScalarField::zeros(format!("precursor{i}"), mesh.clone()))
            .collect();
        let sigma_removal_eff = xs.sigma_removal.clone();
        Ok(Self {
            mesh,
            xs,
            drift,
            beta_total,
            sigma_removal_eff,
            flux,
            precursors,
            k_eff: 1.0,
            settings,
        })
    }

    /// Apply the reduced linear temperature feedback: recompute the
    /// effective removal cross sections at the given salt `temperature`
    /// (`K`) about `t_ref` (`K`) via
    /// [`XsFields::sigma_removal_at`]. Call before [`Self::solve`]; calling
    /// again with a new field replaces the previous feedback state.
    ///
    /// # Errors
    /// [`MoltresError::SizeMismatch`] if `temperature` is not on the mesh.
    pub fn set_temperature(
        &mut self,
        temperature: &TemperatureField,
        t_ref: f64,
    ) -> Result<(), MoltresError> {
        self.sigma_removal_eff = self.xs.sigma_removal_at(temperature, t_ref)?;
        Ok(())
    }

    /// Run the coupled outer iteration to convergence (methodology in the
    /// module docs). On success `k_eff`, `flux`, and `precursors` hold the
    /// coupled fundamental mode.
    ///
    /// Convergence can take 1000–2000 outer iterations (the delayed
    /// coupling is a slow fixed point — see [`EigenSettings`]); set the
    /// `MOLTRES_DEBUG_OUTER` environment variable to print the residual
    /// history every 25 iterations while diagnosing a stall.
    ///
    /// # Errors
    /// [`MoltresError::NoFissionSource`] for a non-multiplying
    /// configuration; [`MoltresError::NotConverged`] on iteration-budget
    /// exhaustion; [`MoltresError::LinearSolveFailed`] if a precursor
    /// solve fails.
    pub fn solve(&mut self) -> Result<EigenReport, MoltresError> {
        let g = self.xs.energy_groups;
        let n = self.mesh.n_cells;
        let beta = self.beta_total;

        let s = fission_source(&self.xs, &self.flux);
        let f = fission_integral(&self.mesh, &s);
        if f <= 0.0 {
            return Err(MoltresError::NoFissionSource);
        }
        scale_fields(&mut self.flux, 1.0 / f);

        let mut k = self.k_eff;
        let mut k_residual = f64::INFINITY;
        let mut flux_residual = f64::INFINITY;
        let mut outer = 0;
        let mut converged = false;

        while outer < self.settings.max_outer_iterations {
            outer += 1;
            let flux_before = snapshot(&self.flux);

            // (1) Precursor drift against the lagged fission source.
            let s_fis = fission_source(&self.xs, &self.flux);
            self.precursors = self.drift.solve_steady(&s_fis, k)?;
            let s_delayed = self.drift.delayed_source(&self.precursors);

            // (2) Group fluxes: prompt/k + delayed + in-scatter sources.
            for gg in 0..g {
                let scat = scattering_source(&self.xs, &self.flux, gg);
                let chi_p = self.xs.chi_prompt[gg].internal.as_slice();
                let chi_d = self.xs.chi_delayed[gg].internal.as_slice();
                let sf = s_fis.internal.as_slice();
                let sd = s_delayed.internal.as_slice();
                let q_vals: Vec<f64> = (0..n)
                    .map(|c| chi_p[c] * (1.0 - beta) / k * sf[c] + chi_d[c] * sd[c] + scat[c])
                    .collect();
                let q = scalar_field(&self.mesh, "q", q_vals);

                let eqn = fvm::laplacian(&self.xs.diffusion_face[gg], &self.flux[gg])
                    + fvm::sp(&self.sigma_removal_eff[gg], &self.flux[gg])
                    - fvm::su(&q, &self.flux[gg]);
                let (sol, _perf) = eqn.solve_cg_with_guess(
                    format!("flux{gg}"),
                    &self.flux[gg],
                    self.settings.linear,
                );
                overwrite_internal(&mut self.flux[gg], &sol);
            }

            // (3) k update and renormalisation.
            let s_new = fission_source(&self.xs, &self.flux);
            let f_new = fission_integral(&self.mesh, &s_new);
            let k_new = k * f_new; // F_old == 1 by normalisation
            k_residual = ((k_new - k) / k_new).abs();
            if f_new > 0.0 {
                scale_fields(&mut self.flux, 1.0 / f_new);
            }
            k = k_new;

            flux_residual = relative_l2_change(&flux_before, &self.flux);
            if std::env::var_os("MOLTRES_DEBUG_OUTER").is_some() && outer % 25 == 0 {
                eprintln!(
                    "outer {outer}: k = {k:.10}, k_res = {k_residual:.3e}, flux_res = {flux_residual:.3e}"
                );
            }
            if k_residual < self.settings.k_tolerance
                && flux_residual < self.settings.flux_tolerance
            {
                converged = true;
                break;
            }
        }

        // Leave the precursors consistent with the final normalised flux/k.
        let s_fis = fission_source(&self.xs, &self.flux);
        self.precursors = self.drift.solve_steady(&s_fis, k)?;

        self.k_eff = k;
        let report = EigenReport {
            k_eff: k,
            outer_iterations: outer,
            k_residual,
            flux_residual,
            converged,
        };
        if converged {
            Ok(report)
        } else {
            Err(MoltresError::NotConverged {
                outer_iterations: outer,
                k_residual,
                flux_residual,
            })
        }
    }

    /// Un-normalised power-density shape `sum_g kappaSigma_{f,g} phi_g`
    /// (`W/m^3` up to the arbitrary flux normalisation — rescale to a
    /// target total power before using as a heat source).
    #[must_use]
    pub fn power_density_shape(&self) -> VolScalarField {
        let n = self.mesh.n_cells;
        let mut q = vec![0.0; n];
        for (g, f) in self.flux.iter().enumerate() {
            let sp = self.xs.sigma_power[g].internal.as_slice();
            let ph = f.internal.as_slice();
            for c in 0..n {
                q[c] += sp[c] * ph[c];
            }
        }
        scalar_field(&self.mesh, "powerDensity", q)
    }

    /// The materialised cross sections this model was built from.
    #[must_use]
    pub fn xs(&self) -> &XsFields {
        &self.xs
    }

    /// Total delayed fraction `beta` of the configured families
    /// (dimensionless).
    #[must_use]
    pub fn beta_total(&self) -> f64 {
        self.beta_total
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diffusion::{reactivity, StaticDiffusion};
    use crate::materials::MsrMaterial;
    use crate::ring_mesh::RingMesh;

    /// MSRE-like illustrative one-group loop: 15 m circumference, 0.1 m^2
    /// flow area, core = first 5.6 m (zone 0), external loop elsewhere
    /// (zone 1). Data are order-of-magnitude MSRE-like, not evaluated MSRE
    /// constants.
    fn msre_like() -> (RingMesh, XsFields) {
        let ring = RingMesh::new(15.0, 0.1, 300).unwrap();
        let core = MsrMaterial {
            name: "core-salt".into(),
            diffusion: vec![0.01],
            sigma_removal: vec![0.8],
            nu_sigma_f: vec![0.81],
            chi_prompt: vec![1.0],
            chi_delayed: vec![1.0],
            scattering: vec![vec![0.0]],
            sigma_power: vec![1e-11],
            d_sigma_removal_d_temp: vec![0.0],
        };
        let external = MsrMaterial::non_fuel("loop-salt", vec![0.01], vec![0.15]);
        let zones = ring.two_zone_map(5.6);
        let xs = XsFields::materialize(ring.mesh.clone(), &zones, &[core, external]).unwrap();
        (ring, xs)
    }

    /// V&V (verification, limiting case) — zero flow reduces to the static
    /// solver.
    ///
    /// **Methodology.** MSRE-like one-group ring (see `msre_like`), Keepin
    /// U-235 families. With `u = 0` and `D_C = 0` the steady precursor
    /// balance is `C_i = beta_i S_f/(k lambda_i)`, whose delayed source
    /// restores exactly the total-spectrum static equation, so
    /// `CirculatingFuelSolver` and `StaticDiffusion` must agree on `k_eff`
    /// up to outer-iteration tolerance. Pass criterion: `|dk| < 1e-6`.
    ///
    /// **Result (measured 2026-08-04, release build):**
    /// `k(static) = 1.00917145`, `k(circulating, u=0) = 1.00917145`,
    /// `|dk| = 2.2e-16` (machine precision). Untrusted AI-assisted draft
    /// pending human V&V.
    #[test]
    fn zero_flow_matches_static_solver() {
        let (ring, xs) = msre_like();
        let families = DelayedFamily::keepin_u235();

        let mut static_solver =
            StaticDiffusion::new(xs.clone(), &families, &[], EigenSettings::default()).unwrap();
        let k_static = static_solver.solve().unwrap().k_eff;

        let mut circ = CirculatingFuelSolver::new(
            xs,
            families,
            ring.uniform_flux(0.0),
            0.0,
            &[],
            EigenSettings::default(),
        )
        .unwrap();
        let k_circ = circ.solve().unwrap().k_eff;

        println!(
            "[V&V u=0 limit] k_static = {k_static:.8}, k_circ = {k_circ:.8}, dk = {:.3e}",
            (k_static - k_circ).abs()
        );
        assert!(
            (k_static - k_circ).abs() < 1e-6,
            "k_static = {k_static}, k_circ = {k_circ}"
        );
    }

    /// V&V (verification, the circulating-fuel signature) — reactivity loss
    /// grows monotonically with loop speed and is bounded by beta.
    ///
    /// **Methodology.** MSRE-like one-group ring (loop 15 m, core 5.6 m,
    /// core dwell fraction 0.373), Keepin U-235 families
    /// (`beta = 0.006502`), `D_C = 1e-4 m^2/s`. Solve the coupled
    /// eigenvalue at salt speeds `u = 0, 0.15, 0.3, 0.6, 1.2, 2.4 m/s`
    /// (loop circulation times infinity, 100, 50, 25, 12.5, 6.25 s; 0.6 m/s
    /// is the MSRE-like nominal ~25 s). Reactivity loss
    /// `dRho(u) = rho(k(0)) - rho(k(u))`. Pass criteria:
    /// (a) `k` strictly decreases with `u`;
    /// (b) `0 < dRho(u) < beta` for every `u > 0` (the loss can at most
    ///     approach the total delayed fraction);
    /// (c) `dRho` strictly increases with `u`;
    /// (d) `dRho(0.6 m/s) > 100 pcm` (order of the MSRE-measured ~212 pcm
    ///     circulation loss — same physics, illustrative constants).
    ///
    /// **Result (measured 2026-08-04, release build):**
    /// `k(0) = 1.00916836`, and
    /// `u [m/s] -> k_eff, dRho [pcm]`:
    /// `0.15 -> 1.00763178, 151.1`;
    /// `0.30 -> 1.00697223, 216.1`;
    /// `0.60 -> 1.00625292, 287.1`;
    /// `1.20 -> 1.00563650, 348.0`;
    /// `2.40 -> 1.00523421, 387.8`;
    /// all monotone, all below `beta = 650.2 pcm`, nominal-flow
    /// (`u = 0.6 m/s`) loss `287 pcm` — the same order as the MSRE's
    /// measured ~212 pcm circulation loss, with the classic trend: steep
    /// initial drop, saturating as precursors of every family are swept
    /// out of the core. (The `u = 0` value here differs in the 6th decimal
    /// from `zero_flow_matches_static_solver` because this test keeps the
    /// small `D_C = 1e-4 m^2/s` precursor diffusivity on at all speeds.)
    /// Untrusted AI-assisted draft pending human V&V.
    #[test]
    fn flow_reduces_reactivity_monotonically() {
        let (ring, xs) = msre_like();
        let families = DelayedFamily::keepin_u235();
        let beta = DelayedFamily::total_beta(&families);

        let speeds = [0.0, 0.15, 0.3, 0.6, 1.2, 2.4];
        let mut ks = Vec::new();
        for u in speeds {
            let mut solver = CirculatingFuelSolver::new(
                xs.clone(),
                families.clone(),
                ring.uniform_flux(u),
                1e-4,
                &[],
                EigenSettings::default(),
            )
            .unwrap();
            let report = solver.solve().unwrap();
            assert!(report.converged, "u = {u}: not converged");
            ks.push(report.k_eff);
        }

        let rho0 = reactivity(ks[0]);
        for (u, k) in speeds.iter().zip(ks.iter()) {
            println!(
                "[V&V circulation loss] u = {u:5.2} m/s -> k = {k:.8}, \
                 dRho = {:8.2} pcm",
                (rho0 - reactivity(*k)) * 1e5
            );
        }
        for i in 1..ks.len() {
            // (a) k strictly decreasing with u.
            assert!(
                ks[i] < ks[i - 1],
                "k must fall with speed: k({}) = {}, k({}) = {}",
                speeds[i - 1],
                ks[i - 1],
                speeds[i],
                ks[i]
            );
            let loss = rho0 - reactivity(ks[i]);
            // (b) loss positive and bounded by beta.
            assert!(loss > 0.0, "u = {}: loss = {loss}", speeds[i]);
            assert!(
                loss < beta,
                "u = {}: loss {loss} exceeds beta {beta}",
                speeds[i]
            );
            // (c) loss strictly increasing with u.
            if i >= 2 {
                let prev_loss = rho0 - reactivity(ks[i - 1]);
                assert!(
                    loss > prev_loss,
                    "loss must grow with speed: {prev_loss} -> {loss}"
                );
            }
        }
        // (d) nominal-flow loss is of the observed MSRE order (>100 pcm).
        let nominal_loss = rho0 - reactivity(ks[3]);
        assert!(
            nominal_loss > 100e-5,
            "nominal-flow loss = {:.1} pcm",
            nominal_loss * 1e5
        );
    }
}
