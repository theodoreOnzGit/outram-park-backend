// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da
//   Upstream source: src/classes/neutronics/diffusion/
//                    (diffusionNeutronics.{C,H},
//                     include/{createFluxMatrices,fluxEq,normFluxes,
//                              solveNeutronics}.H) and
//                    src/classes/neutronics/include/
//                    {calcKeff,calcScatteringSource,initializeNeutroSource,
//                     precEq,initializeDelayedNeutroSource}.H
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//     Principal authors: Carlo Fiorina, Nordine Kerkar, Konstantin Mikityuk,
//       Pablo Rubiolo, Andreas Pautz (EPFL) — Ann. Nucl. Energy 96 (2016) 212.
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

//! # Multigroup neutron-diffusion solver
//!
//! Rust port of GeN-Foam's `diffusionNeutronics`. Assembles and solves the
//! multigroup neutron-diffusion equations on an
//! [`outram_foam_basic_lib::mesh::FvMesh`] with basic-lib's finite-volume
//! operators ([`fvm::laplacian`](outram_foam_basic_lib::fv_operators::fvm::laplacian),
//! [`fvm::sp`](outram_foam_basic_lib::fv_operators::fvm::sp),
//! [`fvm::ddt`](outram_foam_basic_lib::fv_operators::fvm::ddt)) and its LDU
//! linear solvers. Two forms are provided:
//!
//! - **k-eigenvalue** ([`DiffusionNeutronics::solve_eigenvalue`]) — outer
//!   power iteration for the fundamental-mode flux shape and `k_eff` of a
//!   source-free reactor.
//! - **transient** ([`DiffusionNeutronics::step`]) — one backward-Euler time
//!   step of the coupled flux + delayed-neutron-precursor system at a fixed,
//!   externally-imposed `k_eff` (typically the eigenvalue from a preceding
//!   `solve_eigenvalue`, so a null transient holds steady).
//!
//! ## The equations (from `diffusionNeutronics.H`)
//!
//! For each energy group `g` (removal `Sigma_{r,g}` already includes
//! out-scatter; the in-scatter from other groups is an explicit source):
//!
//! ```text
//!   (1/v_g) d(phi_g)/dt = div(D_g grad phi_g) - Sigma_{r,g} phi_g
//!                         + chi_{p,g}/k (1-beta) S_n + chi_{d,g} S_d + S_{s,g}
//! ```
//!
//! with `S_n = sum_h nu Sigma_{f,h} phi_h` (fission production),
//! `S_d = sum_k lambda_k C_k` (delayed source), and
//! `S_{s,g} = sum_{h != g} Sigma_{h->g} phi_h` (in-scatter). For the eigenvalue
//! form the time derivative drops and the whole fission source is scaled by
//! `1/k`; the precursors sit at their equilibrium
//! `C_k = beta_k S_n / (k lambda_k)`, so prompt + delayed collapse to the full
//! spectrum `chi_g = chi_{p,g}(1-beta) + chi_{d,g} beta` times `S_n/k`.
//!
//! ## FV sign convention
//!
//! basic-lib's [`fvm::laplacian`](outram_foam_basic_lib::fv_operators::fvm::laplacian)
//! assembles a **positive-definite** matrix for
//! `-div(D grad phi)` (positive diagonal), so the diffusion + removal loss
//! operator is `fvm::laplacian(D_face, phi) + fvm::sp(Sigma_r, phi)` — both
//! positive-diagonal, hence diagonally dominant and Gauss-Seidel-friendly. See
//! the crate's `pimple_foam` docs for the sign-convention discussion.
//!
//! ## Scope / deferrals
//!
//! Cross sections are materialised once at a fixed feedback point (see
//! [`fields::DiffusionXsFields`]); live thermal-hydraulic feedback, control-rod
//! motion, discontinuity-factor adjustment, liquid-fuel precursor advection,
//! Aitken acceleration, and the implicit/integral transient predictors are
//! deferred with the coupling layer. Boundary conditions are whatever the
//! caller sets on the flux fields (vacuum = `FixedValue(0)`, reflective =
//! `ZeroGradient`).

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

pub use fields::DiffusionXsFields;

/// Convergence and linear-solver controls for a diffusion solve.
///
/// Defaults: `1e-8` outer `k_eff` tolerance, `1e-6` outer flux tolerance, 500
/// outer (power) iterations, and a `1e-9` / 2000-iteration inner Gauss-Seidel
/// linear solve per group.
#[derive(Debug, Clone, Copy)]
pub struct DiffusionSettings {
    /// Outer-loop convergence tolerance on the relative change in `k_eff`.
    pub k_tolerance: f64,
    /// Outer-loop convergence tolerance on the relative L2 change in the flux.
    pub flux_tolerance: f64,
    /// Maximum number of outer power iterations.
    pub max_outer_iterations: usize,
    /// Inner linear-solver settings (one group solve).
    pub linear: SolverSettings,
}

impl Default for DiffusionSettings {
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

/// Outcome of an eigenvalue power iteration.
#[derive(Debug, Clone, Copy)]
pub struct EigenvalueReport {
    /// Converged effective multiplication factor.
    pub k_eff: f64,
    /// Number of outer power iterations performed.
    pub outer_iterations: usize,
    /// Final relative change in `k_eff` between the last two outer iterations.
    pub k_residual: f64,
    /// Final relative L2 change in the flux between the last two outer
    /// iterations.
    pub flux_residual: f64,
    /// Whether both residuals fell below their tolerances.
    pub converged: bool,
}

/// A multigroup neutron-diffusion model on an `FvMesh`.
///
/// Owns the shared [`NeutronicsState`] (flux / precursors / power density /
/// `k_eff`) and the mesh-materialised cross sections
/// ([`DiffusionXsFields`]). Build with [`Self::new`], then call
/// [`Self::solve_eigenvalue`] or [`Self::step`].
#[derive(Debug, Clone)]
pub struct DiffusionNeutronics {
    state: NeutronicsState,
    xs: DiffusionXsFields,
    settings: DiffusionSettings,
}

impl DiffusionNeutronics {
    /// Build a diffusion model on `mesh` from cross sections `xs`, a
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
    /// - `flux_boundary` — the boundary condition to apply to **every** group
    ///   flux field, one entry per mesh patch (`FixedValue(0.0)` for a vacuum /
    ///   zero-flux edge, `ZeroGradient` for a reflective edge). Its length must
    ///   equal the number of patches.
    /// - `settings` — convergence / linear-solver controls.
    ///
    /// # Errors
    ///
    /// [`NeutronicsError::Xs`] if materialisation fails (bad zone/group index,
    /// singular RBF), or [`NeutronicsError::PatchCountMismatch`] if
    /// `flux_boundary` does not have one entry per patch.
    pub fn new(
        mesh: Arc<FvMesh>,
        xs: &CrossSectionData,
        zone_of_cell: &[usize],
        raw_parameters: &[f64],
        flux_boundary: &[BoundaryCondition<f64>],
        settings: DiffusionSettings,
    ) -> Result<Self, NeutronicsError> {
        if flux_boundary.len() != mesh.patches.len() {
            return Err(NeutronicsError::PatchCountMismatch {
                given: flux_boundary.len(),
                patches: mesh.patches.len(),
            });
        }
        let xs_fields = DiffusionXsFields::materialize(&mesh, xs, zone_of_cell, raw_parameters)?;
        let mut state = NeutronicsState::new(mesh, xs.energy_groups(), xs.prec_groups());
        Self::apply_flux_boundary(&mut state, flux_boundary);
        Ok(Self {
            state,
            xs: xs_fields,
            settings,
        })
    }

    /// Set the boundary condition of every group flux field from `flux_boundary`
    /// (one per patch).
    fn apply_flux_boundary(state: &mut NeutronicsState, flux_boundary: &[BoundaryCondition<f64>]) {
        for field in state.flux_mut() {
            for (patch, bc) in field.boundary.iter_mut().zip(flux_boundary.iter()) {
                patch.bc = bc.clone();
                // Keep the stored patch values consistent with a FixedValue BC
                // so face interpolation of the flux sees the right edge value.
                if let BoundaryCondition::FixedValue(v) = bc {
                    patch.values = Field::uniform(patch.values.len(), *v);
                }
            }
        }
    }

    /// The shared neutronics state (flux, precursors, power density, `k_eff`).
    #[must_use]
    pub fn state(&self) -> &NeutronicsState {
        &self.state
    }

    /// The materialised cross-section fields.
    #[must_use]
    pub fn xs_fields(&self) -> &DiffusionXsFields {
        &self.xs
    }

    /// The convergence / solver settings.
    #[must_use]
    pub fn settings(&self) -> &DiffusionSettings {
        &self.settings
    }

    /// Number of energy groups `G`.
    #[must_use]
    pub fn energy_groups(&self) -> usize {
        self.xs.energy_groups
    }

    /// Number of delayed-neutron precursor groups `N`.
    #[must_use]
    pub fn prec_groups(&self) -> usize {
        self.xs.prec_groups
    }

    // ── shared assembly helpers (used by both eigenvalue and transient) ──────

    /// Fission-neutron production field `S_n[c] = sum_g nu Sigma_{f,g}[c]
    /// phi_g[c]` (m^-3 s^-1). GeN-Foam `initializeNeutroSource.H`.
    pub(crate) fn fission_production(&self) -> VolScalarField {
        let mesh = self.state.mesh();
        let n = mesh.n_cells;
        let mut s = vec![0.0; n];
        for (g, flux) in self.state.flux().iter().enumerate() {
            let nf = self.xs.nu_sigma_f[g].internal.as_slice();
            let ph = flux.internal.as_slice();
            for c in 0..n {
                s[c] += nf[c] * ph[c];
            }
        }
        VolScalarField::uniform("neutroSource", mesh.clone(), 0.0).with_internal(s)
    }

    /// Domain integral of the fission production `S_n`, `int_V S_n dV`
    /// (neutrons/s). The positive functional the power iteration converges on.
    pub(crate) fn fission_integral(&self, s_fis: &VolScalarField) -> f64 {
        let vols = &self.state.mesh().cell_volumes;
        s_fis
            .internal
            .as_slice()
            .iter()
            .zip(vols.iter())
            .map(|(s, v)| s * v)
            .sum()
    }

    /// In-scatter source into group `i` from all other groups using the current
    /// flux: `S_{s,i}[c] = sum_{j != i} Sigma_{j->i}[c] phi_j[c]` (m^-3 s^-1).
    /// GeN-Foam `calcScatteringSource.H`.
    pub(crate) fn scattering_source(&self, i: usize) -> Vec<f64> {
        let n = self.state.mesh().n_cells;
        let mut s = vec![0.0; n];
        for (j, flux) in self.state.flux().iter().enumerate() {
            if j == i {
                continue;
            }
            let sig = self.xs.scattering[j][i].internal.as_slice();
            let ph = flux.internal.as_slice();
            for c in 0..n {
                s[c] += sig[c] * ph[c];
            }
        }
        s
    }

    /// Delayed-neutron source field `S_d[c] = sum_k lambda_k[c] C_k[c]`
    /// (m^-3 s^-1). GeN-Foam `initializeDelayedNeutroSource.H`.
    pub(crate) fn delayed_source(&self) -> Vec<f64> {
        let n = self.state.mesh().n_cells;
        let mut s = vec![0.0; n];
        for (k, prec) in self.state.precursors().iter().enumerate() {
            let lam = self.xs.lambda[k].internal.as_slice();
            let c_k = prec.internal.as_slice();
            for c in 0..n {
                s[c] += lam[c] * c_k[c];
            }
        }
        s
    }

    /// Recompute the derived fields (`power_density`, `one_group_flux`,
    /// `total_power`) from the current flux and store them on the state.
    /// GeN-Foam `solveNeutronics.H` tail.
    pub(crate) fn update_derived_fields(&mut self) {
        let n = self.state.mesh().n_cells;
        let mut pd = vec![0.0; n];
        let mut ogf = vec![0.0; n];
        for (g, flux) in self.state.flux().iter().enumerate() {
            let sp = self.xs.sigma_pow[g].internal.as_slice();
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

    /// Overwrite group `g`'s flux internal values in place, preserving its
    /// boundary conditions (the linear solve returns a zero-gradient field).
    pub(crate) fn write_group_flux(&mut self, g: usize, solution: &VolScalarField) {
        let dst = self.state.flux_mut()[g].internal.as_mut_slice();
        for (d, s) in dst.iter_mut().zip(solution.internal.as_slice()) {
            *d = *s;
        }
    }
}

/// Small internal helper to build a `VolScalarField` from an owned value vector
/// while keeping the boundary of a template uniform field.
trait WithInternal {
    fn with_internal(self, values: Vec<f64>) -> Self;
}

impl WithInternal for VolScalarField {
    fn with_internal(mut self, values: Vec<f64>) -> Self {
        debug_assert_eq!(values.len(), self.internal.len());
        self.internal = Field::new(values);
        self
    }
}

mod eigenvalue;
mod transient;
