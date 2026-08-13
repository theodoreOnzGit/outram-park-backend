// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Physics formulation derived from Moltres (MSR multiphysics on MOOSE)
//   Upstream: https://github.com/arfc/moltres (UIUC ARFC group)
//   Upstream commit: 3dd2ce7
//   Upstream sources consulted (formulation only, no code reused):
//     src/kernels/GroupDiffusion.C   (-div(D_g grad phi_g))
//     src/kernels/SigmaR.C           (Sigma_{r,g} phi_g removal)
//     src/kernels/InScatter.C        (sum_{g'!=g} Sigma_{g'->g} phi_{g'})
//     src/kernels/CoupledFissionKernel.C (chi_g/k sum_g' nuSigma_f phi_g')
//   Upstream license: LGPL-2.1, incorporated into this GPL-3.0 crate under
//   the LGPL-2.1 section 3 GPL-conversion option.
//
// Finite-volume assembly and the power-iteration structure build on
// outram-foam-basic-lib (fvm::laplacian / fvm::sp / fvm::su, FvMatrix CG)
// and follow the in-workspace precedent of
// outram-foam-appbuilder-lib::genfoam::neutronics::diffusion (GPL-3.0,
// GeN-Foam-derived), which solves the same equations for static fuel.
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

//! Static-fuel multigroup neutron-diffusion k-eigenvalue solver.
//!
//! Solves, for each energy group `g` on an [`FvMesh`], the steady multigroup
//! diffusion equation with the fission source scaled by `1/k`:
//!
//! ```text
//!   -div(D_g grad phi_g) + Sigma_{r,g} phi_g
//!       = chi_{t,g}/k * sum_g' nuSigma_{f,g'} phi_g'  +  sum_{g'!=g} Sigma_{g'->g} phi_g'
//! ```
//!
//! with the **total** spectrum `chi_{t,g} = chi_{p,g}(1-beta) + chi_{d,g}
//! beta` — i.e. the delayed-neutron precursors are taken at their zero-flow
//! equilibrium `C_i = beta_i S_f / (k lambda_i)`, so their decay source
//! collapses into the fission spectrum. This is the correct limit for
//! **static fuel** (`u = 0`); the flow-dependent solver in
//! [`crate::circulating`] replaces the equilibrium by the advected precursor
//! balance and reduces to this solver as `u -> 0`.
//!
//! The eigenvalue is found by outer **power iteration**: each outer step
//! lags the fission source, solves every group's loss system
//! `fvm::laplacian(D_g) + fvm::sp(Sigma_r)` (symmetric positive definite —
//! solved with warm-started conjugate gradients), updates
//! `k <- k * F_new / F_old` from the fission-production integral `F`, and
//! renormalises the flux to `F = 1`.
//!
//! Units: flux `1/(m^2 s)` (amplitude arbitrary up to normalisation),
//! `D` in `m`, all `Sigma` in `1/m`, `k_eff` dimensionless.

use std::sync::Arc;

use outram_foam_basic_lib::prelude::{
    fvm, BoundaryCondition, Field, FvMesh, SolverSettings, VolScalarField,
};

use crate::error::MoltresError;
use crate::materials::{scalar_field, DelayedFamily, NeutronFluxField, XsFields};

/// Convergence controls for an outer power iteration (static or
/// circulating).
///
/// Defaults: `k` tolerance `1e-8`, flux tolerance `1e-7`, 5000 outer
/// iterations, inner linear solves to `1e-12` within 20000 iterations.
/// The generous outer budget matters for the **circulating** solver: the
/// delayed-neutron coupling through slowly-decaying, advected precursors
/// gives the outer fixed-point map a contraction ratio near 1 (measured
/// ~0.99 per iteration on the MSRE-like test loop), so 1000–2000 outer
/// iterations are routinely needed — they are cheap (one CG + a few
/// Gauss-Seidel solves each). No Aitken/Chebyshev acceleration yet
/// (deferred, as in the workspace's GeN-Foam port).
#[derive(Debug, Clone, Copy)]
pub struct EigenSettings {
    /// Outer convergence tolerance on the relative change of `k_eff`
    /// (dimensionless).
    pub k_tolerance: f64,
    /// Outer convergence tolerance on the relative L2 change of the group
    /// fluxes (dimensionless).
    pub flux_tolerance: f64,
    /// Maximum outer (power) iterations before
    /// [`MoltresError::NotConverged`].
    pub max_outer_iterations: usize,
    /// Inner linear-solver settings (per group / per precursor family).
    pub linear: SolverSettings,
}

impl Default for EigenSettings {
    fn default() -> Self {
        Self {
            k_tolerance: 1e-8,
            flux_tolerance: 1e-7,
            max_outer_iterations: 5000,
            linear: SolverSettings {
                tolerance: 1e-12,
                max_iter: 20_000,
            },
        }
    }
}

/// Outcome of a converged (or abandoned) k-eigenvalue iteration.
#[derive(Debug, Clone, Copy)]
pub struct EigenReport {
    /// Effective multiplication factor `k_eff` (dimensionless).
    pub k_eff: f64,
    /// Outer power iterations performed.
    pub outer_iterations: usize,
    /// Final relative change in `k_eff` (dimensionless).
    pub k_residual: f64,
    /// Final relative L2 change in the flux (dimensionless).
    pub flux_residual: f64,
    /// True if both residuals met their tolerances.
    pub converged: bool,
}

/// Static reactivity `rho = (k - 1)/k` of a multiplication factor
/// (dimensionless; multiply by `1e5` for pcm).
#[must_use]
pub fn reactivity(k_eff: f64) -> f64 {
    (k_eff - 1.0) / k_eff
}

/// Static-fuel multigroup diffusion model on an `FvMesh` (see the module
/// docs for the equations). Build with [`StaticDiffusion::new`], then call
/// [`StaticDiffusion::solve`]; the converged flux shape stays in
/// [`StaticDiffusion::flux`] (normalised so the fission-production integral
/// is 1 — scale to a target power afterwards if needed).
#[derive(Debug, Clone)]
pub struct StaticDiffusion {
    mesh: Arc<FvMesh>,
    xs: XsFields,
    beta_total: f64,
    /// Group flux fields `phi_g`, `1/(m^2 s)` up to normalisation; seeded
    /// uniform, overwritten by [`Self::solve`].
    pub flux: Vec<NeutronFluxField>,
    /// Latest `k_eff` (dimensionless); 1.0 before the first solve.
    pub k_eff: f64,
    settings: EigenSettings,
}

impl StaticDiffusion {
    /// Build a static diffusion model.
    ///
    /// - `xs` — materialised cross sections (see
    ///   [`XsFields::materialize`]).
    /// - `families` — delayed-neutron families; only the **total** `beta`
    ///   enters here (through the equilibrium spectrum collapse). Pass an
    ///   empty slice to model all neutrons as prompt.
    /// - `flux_boundary` — one boundary condition per mesh patch, applied
    ///   to every group flux (`FixedValue(0.0)` = zero-flux/"vacuum" edge,
    ///   `ZeroGradient` = reflective). For a boundary-free
    ///   [`crate::ring_mesh::RingMesh`] pass `&[]`.
    ///
    /// # Errors
    /// [`MoltresError::SizeMismatch`] if `flux_boundary` does not have one
    /// entry per patch.
    pub fn new(
        xs: XsFields,
        families: &[DelayedFamily],
        flux_boundary: &[BoundaryCondition<f64>],
        settings: EigenSettings,
    ) -> Result<Self, MoltresError> {
        let mesh = xs.mesh.clone();
        let flux = seeded_flux(&mesh, xs.energy_groups, flux_boundary)?;
        Ok(Self {
            mesh,
            beta_total: DelayedFamily::total_beta(families),
            xs,
            flux,
            k_eff: 1.0,
            settings,
        })
    }

    /// Run the outer power iteration to convergence (see module docs for
    /// methodology). On success the model's `k_eff` and `flux` hold the
    /// fundamental mode.
    ///
    /// # Errors
    /// [`MoltresError::NoFissionSource`] for a non-multiplying
    /// configuration; [`MoltresError::NotConverged`] if the iteration
    /// budget runs out.
    pub fn solve(&mut self) -> Result<EigenReport, MoltresError> {
        let g = self.xs.energy_groups;
        let n = self.mesh.n_cells;

        // Total spectrum chi_t = chi_p (1-beta) + chi_d beta per cell.
        let beta = self.beta_total;
        let chi_total: Vec<Vec<f64>> = (0..g)
            .map(|gg| {
                let cp = self.xs.chi_prompt[gg].internal.as_slice();
                let cd = self.xs.chi_delayed[gg].internal.as_slice();
                (0..n)
                    .map(|c| cp[c] * (1.0 - beta) + cd[c] * beta)
                    .collect()
            })
            .collect();

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

            // Fission source lagged at the start of the outer iteration
            // (F = 1 by the normalisation of the previous iteration).
            let s_fis = fission_source(&self.xs, &self.flux);

            for gg in 0..g {
                let scat = scattering_source(&self.xs, &self.flux, gg);
                let s_slice = s_fis.internal.as_slice();
                let chi = &chi_total[gg];
                let q_vals: Vec<f64> = (0..n).map(|c| chi[c] / k * s_slice[c] + scat[c]).collect();
                let q = scalar_field(&self.mesh, "q", q_vals);

                let eqn = fvm::laplacian(&self.xs.diffusion_face[gg], &self.flux[gg])
                    + fvm::sp(&self.xs.sigma_removal[gg], &self.flux[gg])
                    - fvm::su(&q, &self.flux[gg]);
                let (sol, _perf) = eqn.solve_cg_with_guess(
                    format!("flux{gg}"),
                    &self.flux[gg],
                    self.settings.linear,
                );
                overwrite_internal(&mut self.flux[gg], &sol);
            }

            let s_new = fission_source(&self.xs, &self.flux);
            let f_new = fission_integral(&self.mesh, &s_new);
            let k_new = k * f_new; // F_old == 1 by normalisation
            k_residual = ((k_new - k) / k_new).abs();
            if f_new > 0.0 {
                scale_fields(&mut self.flux, 1.0 / f_new);
            }
            k = k_new;

            flux_residual = relative_l2_change(&flux_before, &self.flux);
            if k_residual < self.settings.k_tolerance
                && flux_residual < self.settings.flux_tolerance
            {
                converged = true;
                break;
            }
        }

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

    /// The materialised cross sections this model was built from.
    #[must_use]
    pub fn xs(&self) -> &XsFields {
        &self.xs
    }
}

// ── Shared multigroup helpers (also used by `circulating`) ──────────────────

/// Seed uniform-1 group fluxes with the given per-patch boundary conditions.
pub(crate) fn seeded_flux(
    mesh: &Arc<FvMesh>,
    groups: usize,
    flux_boundary: &[BoundaryCondition<f64>],
) -> Result<Vec<NeutronFluxField>, MoltresError> {
    if flux_boundary.len() != mesh.patches.len() {
        return Err(MoltresError::SizeMismatch {
            what: "flux_boundary (one BC per mesh patch)",
            expected: mesh.patches.len(),
            got: flux_boundary.len(),
        });
    }
    let mut flux = Vec::with_capacity(groups);
    for g in 0..groups {
        let mut f = VolScalarField::uniform(format!("flux{g}"), mesh.clone(), 1.0);
        for (patch, bc) in f.boundary.iter_mut().zip(flux_boundary.iter()) {
            patch.bc = bc.clone();
            if let BoundaryCondition::FixedValue(v) = bc {
                patch.values = Field::uniform(patch.values.len(), *v);
            }
        }
        flux.push(f);
    }
    Ok(flux)
}

/// Fission-neutron production `S_f[c] = sum_g nuSigma_{f,g}[c] phi_g[c]`,
/// `1/(m^3 s)` (Moltres `CoupledFissionKernel` / `PrecursorSource` sum).
pub(crate) fn fission_source(xs: &XsFields, flux: &[NeutronFluxField]) -> VolScalarField {
    let n = xs.mesh.n_cells;
    let mut s = vec![0.0; n];
    for (g, f) in flux.iter().enumerate() {
        let nsf = xs.nu_sigma_f[g].internal.as_slice();
        let ph = f.internal.as_slice();
        for c in 0..n {
            s[c] += nsf[c] * ph[c];
        }
    }
    scalar_field(&xs.mesh, "fissionSource", s)
}

/// Domain integral `F = int S_f dV` (neutrons/s) — the positive functional
/// the power iteration converges on.
pub(crate) fn fission_integral(mesh: &FvMesh, s_fis: &VolScalarField) -> f64 {
    s_fis
        .internal
        .as_slice()
        .iter()
        .zip(mesh.cell_volumes.iter())
        .map(|(s, v)| s * v)
        .sum()
}

/// In-scatter source into group `into`:
/// `S_s[c] = sum_{g' != into} Sigma_{g'->into}[c] phi_{g'}[c]`, `1/(m^3 s)`
/// (Moltres `InScatter`).
pub(crate) fn scattering_source(xs: &XsFields, flux: &[NeutronFluxField], into: usize) -> Vec<f64> {
    let n = xs.mesh.n_cells;
    let mut s = vec![0.0; n];
    for (from, f) in flux.iter().enumerate() {
        if from == into {
            continue;
        }
        let sig = xs.scattering[from][into].internal.as_slice();
        let ph = f.internal.as_slice();
        for c in 0..n {
            s[c] += sig[c] * ph[c];
        }
    }
    s
}

/// Multiply every field's internal values by `factor` (flux
/// renormalisation).
pub(crate) fn scale_fields(fields: &mut [VolScalarField], factor: f64) {
    for f in fields {
        for v in f.internal.as_mut_slice() {
            *v *= factor;
        }
    }
}

/// Copy internal cell values of each field (outer-iteration snapshot).
pub(crate) fn snapshot(fields: &[VolScalarField]) -> Vec<Vec<f64>> {
    fields
        .iter()
        .map(|f| f.internal.as_slice().to_vec())
        .collect()
}

/// Relative L2 change between a snapshot and the current fields, summed over
/// all groups (dimensionless).
pub(crate) fn relative_l2_change(before: &[Vec<f64>], after: &[VolScalarField]) -> f64 {
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

/// Overwrite a field's internal values from a solver solution, preserving
/// its boundary conditions (the linear solve returns zero-gradient
/// boundaries).
pub(crate) fn overwrite_internal(dst: &mut VolScalarField, src: &VolScalarField) {
    let d = dst.internal.as_mut_slice();
    for (dv, sv) in d.iter_mut().zip(src.internal.as_slice()) {
        *dv = *sv;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::materials::MsrMaterial;
    use outram_foam_basic_lib::interface::one_dimensional_meshing::create_one_d_mesh;
    use std::f64::consts::PI;
    use uom::si::area::square_meter;
    use uom::si::f64::{Area, Length};
    use uom::si::length::meter;

    fn slab_mesh(l: f64, n: i64) -> Arc<FvMesh> {
        Arc::new(
            create_one_d_mesh(Length::new::<meter>(l), Area::new::<square_meter>(1.0), n).unwrap(),
        )
    }

    fn vacuum() -> [BoundaryCondition<f64>; 2] {
        [
            BoundaryCondition::FixedValue(0.0),
            BoundaryCondition::FixedValue(0.0),
        ]
    }

    /// V&V (verification, analytic reference) — one-group bare slab.
    ///
    /// **Methodology.** 1-D slab of width `L = 1 m`, unit cross-section,
    /// 200 cells, zero-flux (`FixedValue(0)`) boundaries at both faces.
    /// One-group data (SI): `D = 0.009 m`, `Sigma_r = Sigma_a = 0.2 1/m`,
    /// `nuSigma_f = 0.3 1/m`, `chi = 1`. The analytic bare-slab fundamental
    /// mode with zero flux at the boundary faces has buckling
    /// `B = pi / L`, so `k_analytic = nuSigma_f / (Sigma_a + D B^2)
    /// = 0.3 / (0.2 + 0.009 pi^2) = 1.038574...`. Pass criterion:
    /// converged power iteration and `|k - k_analytic| / k_analytic <
    /// 2e-4` (the FV discretisation error is O((B dx)^2)).
    ///
    /// **Result (measured 2026-08-04, 200 cells, release build):**
    /// `k_eff = 1.03869264`, `k_analytic = 1.03868607`, relative error
    /// `6.3e-6` (15 outer iterations) — inside tolerance,
    /// second-order-small as expected. Untrusted AI-assisted draft pending
    /// human V&V.
    #[test]
    fn one_group_bare_slab_matches_analytic() {
        let l = 1.0;
        let mesh = slab_mesh(l, 200);
        let fuel = MsrMaterial {
            name: "fuel".into(),
            diffusion: vec![0.009],
            sigma_removal: vec![0.2],
            nu_sigma_f: vec![0.3],
            chi_prompt: vec![1.0],
            chi_delayed: vec![1.0],
            scattering: vec![vec![0.0]],
            sigma_power: vec![0.0],
            d_sigma_removal_d_temp: vec![0.0],
        };
        let zones = vec![0usize; mesh.n_cells];
        let xs = XsFields::materialize(mesh, &zones, &[fuel]).unwrap();
        let mut solver =
            StaticDiffusion::new(xs, &[], &vacuum(), EigenSettings::default()).unwrap();
        let report = solver.solve().unwrap();
        assert!(report.converged);

        let b2 = (PI / l).powi(2);
        let k_analytic = 0.3 / (0.2 + 0.009 * b2);
        let rel = (report.k_eff - k_analytic).abs() / k_analytic;
        println!(
            "[V&V 1g slab] k = {:.8}, k_analytic = {k_analytic:.8}, rel err = {rel:.3e}, \
             outers = {}",
            report.k_eff, report.outer_iterations
        );
        assert!(
            rel < 2e-4,
            "k = {}, analytic = {k_analytic}, rel err = {rel:.3e}",
            report.k_eff
        );
    }

    /// V&V (verification, analytic reference) — two-group bare slab.
    ///
    /// **Methodology.** Same slab (`L = 1 m`, 200 cells, zero-flux faces),
    /// two groups, fission spectrum `chi = (1, 0)`, downscatter only.
    /// Data (SI, 1/m and m): `D1 = 0.012`, `D2 = 0.004`,
    /// `Sigma_r1 = Sigma_a1 + Sigma_12 = 1.0 + 1.6 = 2.6`,
    /// `Sigma_r2 = Sigma_a2 = 8.0`, `Sigma_12 = 1.6`,
    /// `nuSigma_f1 = 0.6`, `nuSigma_f2 = 12.0`. Analytic two-group bare
    /// reactor with `B^2 = (pi/L)^2`:
    /// `k = (nuSigma_f1 + nuSigma_f2 * Sigma_12/(Sigma_a2 + D2 B^2))
    /// / (Sigma_r1 + D1 B^2)`. Pass criterion: converged and relative
    /// error `< 2e-4`.
    ///
    /// **Result (measured 2026-08-04, 200 cells, release build):**
    /// `k_eff = 1.09924174`, `k_analytic = 1.09924069`, relative error
    /// `9.5e-7`. Untrusted AI-assisted draft pending human V&V.
    #[test]
    fn two_group_bare_slab_matches_analytic() {
        let l = 1.0;
        let mesh = slab_mesh(l, 200);
        let fuel = MsrMaterial {
            name: "fuel2g".into(),
            diffusion: vec![0.012, 0.004],
            sigma_removal: vec![2.6, 8.0],
            nu_sigma_f: vec![0.6, 12.0],
            chi_prompt: vec![1.0, 0.0],
            chi_delayed: vec![1.0, 0.0],
            scattering: vec![vec![0.0, 1.6], vec![0.0, 0.0]],
            sigma_power: vec![0.0, 0.0],
            d_sigma_removal_d_temp: vec![0.0, 0.0],
        };
        let zones = vec![0usize; mesh.n_cells];
        let xs = XsFields::materialize(mesh, &zones, &[fuel]).unwrap();
        let mut solver =
            StaticDiffusion::new(xs, &[], &vacuum(), EigenSettings::default()).unwrap();
        let report = solver.solve().unwrap();
        assert!(report.converged);

        let b2 = (PI / l).powi(2);
        let k_analytic = (0.6 + 12.0 * 1.6 / (8.0 + 0.004 * b2)) / (2.6 + 0.012 * b2);
        let rel = (report.k_eff - k_analytic).abs() / k_analytic;
        println!(
            "[V&V 2g slab] k = {:.8}, k_analytic = {k_analytic:.8}, rel err = {rel:.3e}",
            report.k_eff
        );
        assert!(
            rel < 2e-4,
            "k = {}, analytic = {k_analytic}, rel err = {rel:.3e}",
            report.k_eff
        );
    }

    /// Consistency: when `chi_p == chi_d`, the static eigenvalue must not
    /// depend on the delayed fraction beta (the spectrum collapse
    /// `chi_p(1-beta) + chi_d beta` is then the identity).
    #[test]
    fn beta_split_is_invisible_when_spectra_match() {
        let mesh = slab_mesh(1.0, 60);
        let fuel = MsrMaterial {
            name: "fuel".into(),
            diffusion: vec![0.009],
            sigma_removal: vec![0.2],
            nu_sigma_f: vec![0.3],
            chi_prompt: vec![1.0],
            chi_delayed: vec![1.0],
            scattering: vec![vec![0.0]],
            sigma_power: vec![0.0],
            d_sigma_removal_d_temp: vec![0.0],
        };
        let zones = vec![0usize; mesh.n_cells];
        let xs = XsFields::materialize(mesh, &zones, &[fuel]).unwrap();
        let mut a =
            StaticDiffusion::new(xs.clone(), &[], &vacuum(), EigenSettings::default()).unwrap();
        let mut b = StaticDiffusion::new(
            xs,
            &DelayedFamily::keepin_u235(),
            &vacuum(),
            EigenSettings::default(),
        )
        .unwrap();
        let ka = a.solve().unwrap().k_eff;
        let kb = b.solve().unwrap().k_eff;
        assert!((ka - kb).abs() < 1e-10, "ka = {ka}, kb = {kb}");
    }
}
