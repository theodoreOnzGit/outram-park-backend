// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da
//   Upstream source: src/classes/neutronics/SP3/ and albedoSP3/
//     (SP3Neutronics.{C,H}, include/{createFluxMatricesSP3,fluxEqSP3,
//      solveNeutronicsSP3,normFluxesSP3}.H — simplified-P3 transport) and
//     src/classes/neutronics/include/{calcKeff,calcScatteringSource,
//      initializeNeutroSource,initializeDelayedNeutroSource,precEq}.H (shared).
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

//! # Simplified-P3 (SP3) neutron-transport solver
//!
//! Rust port of GeN-Foam's `SP3Neutronics`. SP3 augments the multigroup
//! neutron-diffusion machinery ([`crate::genfoam::neutronics::diffusion`]) with
//! a **second angular moment** of the flux, recovering part of the transport
//! anisotropy that diffusion theory drops. It is more accurate than diffusion in
//! optically thin or strongly heterogeneous regions, and — this is the key
//! verification property — it *reduces to diffusion* wherever the flux is
//! near-isotropic.
//!
//! ## The two coupled SP3 moment equations (from `fluxEqSP3.H`)
//!
//! SP3 carries, per energy group `g`, two moment fields:
//!
//! - the **0th composite moment** `Phi0_g = phi0_g + 2 phi2_g`
//!   (GeN-Foam `fluxStar_`), and
//! - the **2nd moment** `phi2_g` (GeN-Foam `fluxStar2_`),
//!
//! from which the physical scalar flux is reconstructed as
//! `phi0_g = Phi0_g - 2 phi2_g` (GeN-Foam `flux_ = fluxStar_ - 2 fluxStar2_`,
//! discontinuity factor 1). The steady (eigenvalue) equations are
//!
//! ```text
//!   -div(D0_g grad Phi0_g) + Sigma_{r,g} Phi0_g = Q_g + 2 Sigma_{r,g} phi2_g
//!   -div(D2_g grad phi2_g) + A2_g phi2_g        = (2/3)(Sigma_{r,g} Phi0_g - Q_g)
//! ```
//!
//! with `D0_g = D_g` the ordinary diffusion coefficient,
//! `D2_g = (3/7)/Sigma_{t,g}`, `A2_g = (5/3) Sigma_{t,g} + (4/3) Sigma_{r,g}`,
//! `Sigma_{t,g} = Sigma_{r,g} + Sigma_{s,g->g}`, and the same fission +
//! in-scatter source as diffusion,
//! `Q_g = chi_g/k * S_n + S_{s,g}`. The removal in the first equation acts on
//! the *physical* flux — `Sigma_r Phi0 - 2 Sigma_r phi2 = Sigma_r phi0` — so with
//! `phi2 -> 0` the first equation is exactly the diffusion equation. In an
//! infinite medium `phi2` is identically zero, so SP3's `k_inf` equals
//! diffusion's `k_inf = nu Sigma_f / Sigma_a` exactly (see the V&V tests).
//!
//! ## Two forms
//!
//! - **k-eigenvalue** ([`Sp3Neutronics::solve_eigenvalue`]) — outer power
//!   iteration over the coupled moment system (`solveNeutronicsSP3.H` with
//!   `eigenvalueNeutronics_ = true`, so the time terms drop).
//! - **transient** ([`Sp3Neutronics::step`]) — one backward-Euler step of the
//!   coupled moments + delayed precursors at a frozen `k_eff`.
//!
//! ## Boundary conditions
//!
//! The caller's per-patch boundary condition is applied to **both** moment
//! fields (`Phi0` and `phi2`): `FixedValue(0)` on both gives the zero-flux
//! (Mark) vacuum edge `phi0 = 0`; `ZeroGradient` on both gives a reflective
//! edge. GeN-Foam's Marshak/albedo SP3 boundary (`albedoSP3FvPatchField`), which
//! couples the two moments at the surface, is a documented deferral — the
//! zero-flux edge is the standard choice for the homogeneous verification cases
//! here.
//!
//! ## Scope / deferrals
//!
//! Same fixed-feedback-point cross sections as diffusion (see
//! [`fields::Sp3XsFields`]); live TH feedback, discontinuity-factor adjustment,
//! liquid-fuel precursor advection, Aitken acceleration, the implicit/integral
//! transient predictors, and the albedo SP3 boundary are deferred with the
//! coupling layer.

pub mod fields;

#[cfg(test)]
mod tests;

use std::sync::Arc;

use outram_foam_basic_lib::prelude::{
    BoundaryCondition, Field, FvMesh, SolverSettings, VolScalarField,
};

use crate::genfoam::neutronics::state::NeutronicsState;
use crate::genfoam::neutronics::xs::CrossSectionData;
use crate::genfoam::neutronics::NeutronicsError;

pub use fields::Sp3XsFields;

/// Convergence and linear-solver controls for an SP3 solve.
///
/// Defaults match [`crate::genfoam::neutronics::DiffusionSettings`]: `1e-8`
/// outer `k_eff` tolerance, `1e-6` outer flux tolerance, 500 outer (power)
/// iterations, and a `1e-9` / 2000-iteration inner CG linear solve per moment
/// equation.
#[derive(Debug, Clone, Copy)]
pub struct Sp3Settings {
    /// Outer-loop convergence tolerance on the relative change in `k_eff`.
    pub k_tolerance: f64,
    /// Outer-loop convergence tolerance on the relative L2 change in the flux.
    pub flux_tolerance: f64,
    /// Maximum number of outer power iterations.
    pub max_outer_iterations: usize,
    /// Inner linear-solver settings (one moment-equation solve).
    pub linear: SolverSettings,
}

impl Default for Sp3Settings {
    fn default() -> Self {
        Self {
            k_tolerance: 1e-8,
            flux_tolerance: 1e-6,
            max_outer_iterations: 500,
            linear: SolverSettings {
                tolerance: 1e-9,
                max_iter: 2000,
            },
        }
    }
}

/// A simplified-P3 (SP3) transport model on an `FvMesh`.
///
/// Owns the shared [`NeutronicsState`] (physical flux `phi0` / precursors /
/// power density / `k_eff`), the SP3 moment fields (`Phi0` and `phi2` per
/// group), and — once built with [`Self::with_cross_sections`] — the
/// mesh-materialised SP3 cross sections ([`Sp3XsFields`]).
///
/// Two ways to construct:
///
/// - [`Self::new`] — a **state-only scaffold** (no cross sections). It exists so
///   [`crate::genfoam::neutronics::NeutronicsModel::Sp3`] can hold a value with
///   the shared surface (`power`, `k_eff`) working; [`Self::solve_eigenvalue`]
///   on such a value returns [`NeutronicsError::ModelNotImplemented`].
/// - [`Self::with_cross_sections`] — a **solvable** model with cross sections,
///   ready for [`Self::solve_eigenvalue`] / [`Self::step`].
#[derive(Debug, Clone)]
pub struct Sp3Neutronics {
    state: NeutronicsState,
    /// 0th composite moment `Phi0_g = phi0_g + 2 phi2_g` per group
    /// (GeN-Foam `fluxStar_`).
    flux_star: Vec<VolScalarField>,
    /// 2nd moment `phi2_g` per group (GeN-Foam `fluxStar2_`).
    flux_star2: Vec<VolScalarField>,
    /// Cross sections — `None` for a state-only scaffold built with
    /// [`Self::new`].
    xs: Option<Sp3XsFields>,
    settings: Sp3Settings,
}

impl Sp3Neutronics {
    /// Allocate a **state-only SP3 scaffold** with `energy_groups` flux fields
    /// and `prec_groups` precursor fields on `mesh` (no cross sections).
    ///
    /// The shared [`crate::genfoam::neutronics::NeutronicsModel`] surface
    /// (`power`, `k_eff`) works, but [`Self::solve_eigenvalue`] returns
    /// [`NeutronicsError::ModelNotImplemented`] until the model is built with
    /// cross sections via [`Self::with_cross_sections`].
    #[must_use]
    pub fn new(mesh: Arc<FvMesh>, energy_groups: usize, prec_groups: usize) -> Self {
        let flux_star = (0..energy_groups)
            .map(|g| VolScalarField::uniform(format!("fluxStar{g}"), mesh.clone(), 1.0))
            .collect();
        let flux_star2 = (0..energy_groups)
            .map(|g| VolScalarField::zeros(format!("fluxStar2{g}"), mesh.clone()))
            .collect();
        Self {
            state: NeutronicsState::new(mesh, energy_groups, prec_groups),
            flux_star,
            flux_star2,
            xs: None,
            settings: Sp3Settings::default(),
        }
    }

    /// Build a **solvable** SP3 model on `mesh` from cross sections `xs`, a
    /// `cell -> zone` map, and a fixed feedback point.
    ///
    /// # Parameters
    ///
    /// - `mesh` — the finite-volume mesh (shared `Arc`).
    /// - `xs` — the multigroup cross-section store.
    /// - `zone_of_cell` — material-zone index of each cell (length
    ///   `mesh.n_cells`).
    /// - `raw_parameters` — the feedback point in
    ///   [`CrossSectionData::variable_names`] order (`&[]` for feedback-free
    ///   data).
    /// - `flux_boundary` — the boundary condition applied to **both** SP3 moment
    ///   fields of every group, one entry per mesh patch (`FixedValue(0.0)` for a
    ///   vacuum / zero-flux edge, `ZeroGradient` for a reflective edge). Its
    ///   length must equal the number of patches.
    /// - `settings` — convergence / linear-solver controls.
    ///
    /// # Errors
    ///
    /// [`NeutronicsError::Xs`] if materialisation fails, or
    /// [`NeutronicsError::PatchCountMismatch`] if `flux_boundary` does not have
    /// one entry per patch.
    pub fn with_cross_sections(
        mesh: Arc<FvMesh>,
        xs: &CrossSectionData,
        zone_of_cell: &[usize],
        raw_parameters: &[f64],
        flux_boundary: &[BoundaryCondition<f64>],
        settings: Sp3Settings,
    ) -> Result<Self, NeutronicsError> {
        if flux_boundary.len() != mesh.patches.len() {
            return Err(NeutronicsError::PatchCountMismatch {
                given: flux_boundary.len(),
                patches: mesh.patches.len(),
            });
        }
        let xs_fields = Sp3XsFields::materialize(&mesh, xs, zone_of_cell, raw_parameters)?;
        let g = xs.energy_groups();
        let flux_star = (0..g)
            .map(|gg| VolScalarField::uniform(format!("fluxStar{gg}"), mesh.clone(), 1.0))
            .collect();
        let flux_star2 = (0..g)
            .map(|gg| VolScalarField::zeros(format!("fluxStar2{gg}"), mesh.clone()))
            .collect();
        let mut model = Self {
            state: NeutronicsState::new(mesh, g, xs.prec_groups()),
            flux_star,
            flux_star2,
            xs: Some(xs_fields),
            settings,
        };
        model.apply_flux_boundary(flux_boundary);
        model.reconstruct_flux();
        Ok(model)
    }

    /// Apply `flux_boundary` (one per patch) to every group's `Phi0` and `phi2`
    /// moment field and to the reconstructed physical flux.
    fn apply_flux_boundary(&mut self, flux_boundary: &[BoundaryCondition<f64>]) {
        let apply = |field: &mut VolScalarField| {
            for (patch, bc) in field.boundary.iter_mut().zip(flux_boundary.iter()) {
                patch.bc = bc.clone();
                if let BoundaryCondition::FixedValue(v) = bc {
                    patch.values = Field::uniform(patch.values.len(), *v);
                }
            }
        };
        for field in self.flux_star.iter_mut() {
            apply(field);
        }
        for field in self.flux_star2.iter_mut() {
            apply(field);
        }
        for field in self.state.flux_mut() {
            apply(field);
        }
    }

    /// The shared neutronics state (physical flux, precursors, power density,
    /// `k_eff`).
    #[must_use]
    pub fn state(&self) -> &NeutronicsState {
        &self.state
    }

    /// The materialised SP3 cross-section fields, if this is a solvable model
    /// (built with [`Self::with_cross_sections`]).
    #[must_use]
    pub fn xs_fields(&self) -> Option<&Sp3XsFields> {
        self.xs.as_ref()
    }

    /// The convergence / solver settings.
    #[must_use]
    pub fn settings(&self) -> &Sp3Settings {
        &self.settings
    }

    /// The per-group 0th composite moment `Phi0_g = phi0_g + 2 phi2_g`
    /// (GeN-Foam `fluxStar_`), read-only.
    #[must_use]
    pub fn flux_star(&self) -> &[VolScalarField] {
        &self.flux_star
    }

    /// The per-group 2nd moment `phi2_g` (GeN-Foam `fluxStar2_`), read-only.
    #[must_use]
    pub fn flux_star2(&self) -> &[VolScalarField] {
        &self.flux_star2
    }

    /// Number of energy groups `G`.
    #[must_use]
    pub fn energy_groups(&self) -> usize {
        self.state.energy_groups()
    }

    /// Number of delayed-neutron precursor groups `N`.
    #[must_use]
    pub fn prec_groups(&self) -> usize {
        self.state.prec_groups()
    }

    // ── shared assembly helpers (used by both eigenvalue and transient) ──────

    /// Reconstruct the physical scalar flux `phi0_g = Phi0_g - 2 phi2_g` from the
    /// two moment fields and store it on the state (GeN-Foam
    /// `flux_ = fluxStar_ - 2 fluxStar2_`, discontinuity factor 1).
    pub(crate) fn reconstruct_flux(&mut self) {
        let n = self.state.mesh().n_cells;
        for g in 0..self.energy_groups() {
            let star = self.flux_star[g].internal.as_slice().to_vec();
            let star2 = self.flux_star2[g].internal.as_slice().to_vec();
            let dst = self.state.flux_mut()[g].internal.as_mut_slice();
            for c in 0..n {
                dst[c] = star[c] - 2.0 * star2[c];
            }
        }
    }

    /// Fission-neutron production `S_n[c] = sum_g nu Sigma_{f,g}[c] phi0_g[c]`
    /// (m^-3 s^-1) from the reconstructed physical flux. GeN-Foam
    /// `initializeNeutroSource.H`.
    pub(crate) fn fission_production(&self, xs: &Sp3XsFields) -> Vec<f64> {
        let n = self.state.mesh().n_cells;
        let mut s = vec![0.0; n];
        for (g, flux) in self.state.flux().iter().enumerate() {
            let nf = xs.base.nu_sigma_f[g].internal.as_slice();
            let ph = flux.internal.as_slice();
            for c in 0..n {
                s[c] += nf[c] * ph[c];
            }
        }
        s
    }

    /// Domain integral `int_V S_n dV` (neutrons/s) — the positive functional the
    /// power iteration converges on.
    pub(crate) fn fission_integral(&self, s_fis: &[f64]) -> f64 {
        let vols = &self.state.mesh().cell_volumes;
        s_fis.iter().zip(vols.iter()).map(|(s, v)| s * v).sum()
    }

    /// In-scatter source into group `i`, `S_{s,i}[c] = sum_{j != i}
    /// Sigma_{j->i}[c] phi0_j[c]` (m^-3 s^-1), from the physical flux. GeN-Foam
    /// `calcScatteringSource.H`.
    pub(crate) fn scattering_source(&self, xs: &Sp3XsFields, i: usize) -> Vec<f64> {
        let n = self.state.mesh().n_cells;
        let mut s = vec![0.0; n];
        for (j, flux) in self.state.flux().iter().enumerate() {
            if j == i {
                continue;
            }
            let sig = xs.base.scattering[j][i].internal.as_slice();
            let ph = flux.internal.as_slice();
            for c in 0..n {
                s[c] += sig[c] * ph[c];
            }
        }
        s
    }

    /// Delayed-neutron source `S_d[c] = sum_k lambda_k[c] C_k[c]` (m^-3 s^-1).
    /// GeN-Foam `initializeDelayedNeutroSource.H`.
    pub(crate) fn delayed_source(&self, xs: &Sp3XsFields) -> Vec<f64> {
        let n = self.state.mesh().n_cells;
        let mut s = vec![0.0; n];
        for (k, prec) in self.state.precursors().iter().enumerate() {
            let lam = xs.base.lambda[k].internal.as_slice();
            let c_k = prec.internal.as_slice();
            for c in 0..n {
                s[c] += lam[c] * c_k[c];
            }
        }
        s
    }

    /// Recompute `power_density`, `one_group_flux`, and `total_power` from the
    /// physical flux and store them on the state. GeN-Foam
    /// `solveNeutronicsSP3.H` tail.
    pub(crate) fn update_derived_fields(&mut self, xs: &Sp3XsFields) {
        let n = self.state.mesh().n_cells;
        let mut pd = vec![0.0; n];
        let mut ogf = vec![0.0; n];
        for (g, flux) in self.state.flux().iter().enumerate() {
            let sp = xs.base.sigma_pow[g].internal.as_slice();
            let ph = flux.internal.as_slice();
            for c in 0..n {
                pd[c] += ph[c] * sp[c];
                ogf[c] += ph[c];
            }
        }
        let vols = &self.state.mesh().cell_volumes;
        let total: f64 = pd.iter().zip(vols.iter()).map(|(p, v)| p * v).sum();
        for (c, &p) in pd.iter().enumerate() {
            self.state.power_density_mut().internal.as_mut_slice()[c] = p;
        }
        for (c, &o) in ogf.iter().enumerate() {
            self.state.one_group_flux_mut().internal.as_mut_slice()[c] = o;
        }
        self.state.set_total_power_raw(total);
    }

    /// Overwrite a moment field's internal values in place, keeping its
    /// boundary (the linear solve returns a zero-gradient field).
    pub(crate) fn write_moment(dst: &mut VolScalarField, solution: &VolScalarField) {
        for (d, s) in dst
            .internal
            .as_mut_slice()
            .iter_mut()
            .zip(solution.internal.as_slice())
        {
            *d = *s;
        }
    }
}

/// Overwrite a field's internal values, keeping its boundary.
pub(crate) fn with_values(mut field: VolScalarField, values: Vec<f64>) -> VolScalarField {
    field.internal = Field::new(values);
    field
}

mod eigenvalue;
mod transient;
