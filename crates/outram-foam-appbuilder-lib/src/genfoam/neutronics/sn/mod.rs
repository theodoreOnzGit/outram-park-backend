// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da
//   Upstream source: src/classes/neutronics/SN/ (SNNeutronics.{C,H} and
//     include/{readQuadratureSet,solveNeutronicsSN,fluxEqSN,calcSourceSN,
//     updateScalarFluxes,normFluxesSN}.H) plus
//     src/classes/neutronics/include/{initializeNeutroSource,calcKeff}.H —
//     discrete-ordinates (S_N) transport.
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//     Principal authors: Carlo Fiorina, Thomas Guilbaud (EPFL); the SN model is
//     derived by Carlo Fiorina from the work of Manuele Aufiero (PoliMi).
//     Ann. Nucl. Energy 96 (2016) 212; Nucl. Eng. Des. 294 (2015) 24.
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

//! # Discrete-ordinates (S_N) transport
//!
//! Rust port of GeN-Foam's `SNNeutronics`. Where [`super::diffusion`] replaces
//! the angular dependence of the transport equation with Fick's law, the S_N
//! method keeps it: it solves the transport equation directly on a finite set
//! of discrete directions (an angular [`quadrature`] set), so it captures
//! strong flux anisotropy — streaming through voids, deep gradients near strong
//! absorbers — that diffusion / SP3 smear out. It is the highest-fidelity
//! deterministic option in the suite.
//!
//! ## The discrete-ordinates equations (from `SNNeutronics.H`)
//!
//! For energy group `i` and direction `Omega_j` (unit vector, weight `w_j`),
//! the steady (eigenvalue) angular flux `psi_{i,j}` obeys
//!
//! ```text
//!   div(Omega_j psi_{i,j}) + Sigma_{t,i} psi_{i,j} = q_i / (4*pi)
//! ```
//!
//! with the isotropic source (moment 0)
//!
//! ```text
//!   q_i = chi_i / k sum_{i'} nu Sigma_{f,i'} phi_{i'}
//!         + sum_{i'} Sigma_{s, i' -> i} phi_{i'} ,
//! ```
//!
//! where `phi_i = sum_j w_j psi_{i,j}` is the scalar flux and `sum_j w_j =
//! 4*pi` (see [`quadrature`]). The total cross section
//! `Sigma_{t,i} = Sigma_{r,i} + Sigma_{s, i -> i}` is reconstructed from the
//! (self-scatter-excluded) removal cross section the shared
//! [`super::xs`] data stores plus the group's own moment-0 self-scatter — the
//! scattering source `sum_{i'}` then runs over **all** groups including `i`.
//! The `div` term is discretised with basic-lib's first-order **upwind**
//! convection operator
//! [`fvm::div`](outram_foam_basic_lib::fv_operators::fvm::div) on the face flux
//! `Omega_j . Sf`, which is the finite-volume transport sweep; the collision
//! term is [`fvm::sp`](outram_foam_basic_lib::fv_operators::fvm::sp) and the
//! source is [`fvm::su`](outram_foam_basic_lib::fv_operators::fvm::su). The
//! resulting per-direction matrix is asymmetric, so it is solved with the
//! Gauss-Seidel LDU solver (not CG).
//!
//! ## Iteration structure
//!
//! - **inner** ([`sweep`]) — a scattering-source iteration for one group:
//!   evaluate the isotropic source at the current scalar flux, solve every
//!   direction, re-sum the scalar flux, repeat until it stops changing.
//! - **outer** ([`eigenvalue`]) — the k-eigenvalue power iteration on the
//!   fission source, identical in structure to the diffusion solver
//!   (`k <- k F_new / F_old`, renormalise `F = 1`).
//!
//! ## Scope / deferrals
//!
//! Isotropic (P0) scattering only — the anisotropic Legendre-moment source
//! (`SNNeutronics`'s `legendreMatrices_`, moments `>= 1`) is deferred, matching
//! how the [`super::diffusion`] port uses only moment 0. Transients, liquid-fuel
//! precursor advection, Aitken acceleration, and live thermal-hydraulic feedback
//! are likewise deferred with the coupling layer. Reflective boundaries are the
//! `ZeroGradient` flux BC; a vacuum edge is `FixedValue(0)` (zero incoming
//! angular flux, exact for the upwind operator).
//!
//! ## Two ways to build an `SnNeutronics`
//!
//! - [`SnNeutronics::new`] — a **scaffold** with an allocated
//!   [`NeutronicsState`] but no cross sections. [`Self::solve_eigenvalue`] on it
//!   returns [`NeutronicsError::ModelNotImplemented`]; it exists so the shared
//!   [`super::NeutronicsModel::Sn`] surface (`power`, `k_eff`, `kind`) works.
//! - [`SnNeutronics::with_cross_sections`] — the **working** model:
//!   materialises the cross sections, builds the quadrature, and lets
//!   [`Self::solve_eigenvalue`] run the real transport eigenvalue solve.

pub mod quadrature;

mod eigenvalue;
mod sweep;

#[cfg(test)]
mod tests;

use std::sync::Arc;

use outram_foam_basic_lib::prelude::{BoundaryCondition, Field, FvMesh, SolverSettings, VolScalarField};

use crate::genfoam::neutronics::diffusion::DiffusionXsFields;
use crate::genfoam::neutronics::state::NeutronicsState;
use crate::genfoam::neutronics::xs::CrossSectionData;
use crate::genfoam::neutronics::{NeutronicsError, NeutronicsModelKind};

pub use quadrature::{AngularQuadrature, QuadratureOrder};

/// Convergence and linear-solver controls for an S_N solve.
///
/// Defaults: `1e-8` outer `k_eff` tolerance, `1e-6` outer flux tolerance, 500
/// outer (power) iterations; up to 50 inner scattering-source iterations per
/// group with a `1e-7` relative flux tolerance; and a `1e-9` / 2000-iteration
/// inner Gauss-Seidel linear solve per direction.
#[derive(Debug, Clone, Copy)]
pub struct SnSettings {
    /// Outer-loop convergence tolerance on the relative change in `k_eff`.
    pub k_tolerance: f64,
    /// Outer-loop convergence tolerance on the relative L2 change in the flux.
    pub flux_tolerance: f64,
    /// Maximum number of outer power iterations.
    pub max_outer_iterations: usize,
    /// Maximum number of inner scattering-source iterations per group per outer
    /// iteration.
    pub max_inner_iterations: usize,
    /// Relative-L2 convergence tolerance on a group's scalar flux between inner
    /// scattering-source iterations.
    pub inner_tolerance: f64,
    /// Inner linear-solver settings (one direction solve).
    pub linear: SolverSettings,
}

impl Default for SnSettings {
    fn default() -> Self {
        Self {
            k_tolerance: 1e-8,
            flux_tolerance: 1e-6,
            max_outer_iterations: 500,
            max_inner_iterations: 50,
            inner_tolerance: 1e-7,
            linear: SolverSettings {
                tolerance: 1e-9,
                max_iter: 2000,
            },
        }
    }
}

/// The mesh-materialised data a working S_N solve needs, over and above the
/// shared [`NeutronicsState`]: the cross sections, the angular quadrature, the
/// per-direction face-flux fields, the precomputed total cross section, and the
/// angular-flux storage `psi[group][direction]`.
///
/// Absent (`None` on [`SnNeutronics`]) for a scaffold model built with
/// [`SnNeutronics::new`].
#[derive(Debug, Clone)]
struct SnSolverData {
    /// Mesh-materialised multigroup cross sections (shared with the diffusion
    /// port's field bundle).
    xs: DiffusionXsFields,
    /// The discrete-ordinates quadrature (`Omega_j`, `w_j`).
    quadrature: AngularQuadrature,
    /// Face flux `Omega_j . Sf` per direction (streaming weight for `fvm::div`).
    face_phi: Vec<outram_foam_basic_lib::prelude::SurfaceScalarField>,
    /// Total cross section `Sigma_{t,i} = Sigma_{r,i} + Sigma_{s,i->i}` per
    /// group (m^-1).
    sigma_t: Vec<VolScalarField>,
    /// Angular flux `psi[group][direction]` (`1/(m^2 s)`).
    angular_flux: Vec<Vec<VolScalarField>>,
}

/// A discrete-ordinates (S_N) transport model on an `FvMesh`.
///
/// Build a **working** model with [`Self::with_cross_sections`] and run
/// [`Self::solve_eigenvalue`]; a bare [`Self::new`] is a scaffold whose
/// `solve_eigenvalue` returns [`NeutronicsError::ModelNotImplemented`] (see the
/// module docs).
#[derive(Debug, Clone)]
pub struct SnNeutronics {
    state: NeutronicsState,
    solver: Option<SnSolverData>,
    settings: SnSettings,
}

impl SnNeutronics {
    /// Allocate an S_N **scaffold** with `energy_groups` scalar-flux fields and
    /// `prec_groups` precursor fields on `mesh`, but no cross sections or
    /// quadrature.
    ///
    /// The shared [`super::NeutronicsModel`] surface (`power`, `k_eff`, `kind`)
    /// works, but [`Self::solve_eigenvalue`] returns
    /// [`NeutronicsError::ModelNotImplemented`] until the model is built with
    /// cross sections via [`Self::with_cross_sections`].
    #[must_use]
    pub fn new(mesh: Arc<FvMesh>, energy_groups: usize, prec_groups: usize) -> Self {
        Self {
            state: NeutronicsState::new(mesh, energy_groups, prec_groups),
            solver: None,
            settings: SnSettings::default(),
        }
    }

    /// Build a **working** S_N model on `mesh` from cross sections `xs`, a
    /// `cell -> zone` map, a fixed feedback point, per-patch flux boundaries, an
    /// angular quadrature order, and solver settings.
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
    /// - `flux_boundary` — the boundary condition applied to **every** angular
    ///   and scalar flux field, one entry per mesh patch. `FixedValue(0.0)` is a
    ///   vacuum / zero-incoming edge; `ZeroGradient` is a reflective edge. Its
    ///   length must equal the number of patches.
    /// - `order` — the level-symmetric quadrature order ([`QuadratureOrder`]).
    /// - `settings` — convergence / linear-solver controls.
    ///
    /// # Errors
    ///
    /// [`NeutronicsError::Xs`] if cross-section materialisation fails, or
    /// [`NeutronicsError::PatchCountMismatch`] if `flux_boundary` does not have
    /// one entry per patch.
    pub fn with_cross_sections(
        mesh: Arc<FvMesh>,
        xs: &CrossSectionData,
        zone_of_cell: &[usize],
        raw_parameters: &[f64],
        flux_boundary: &[BoundaryCondition<f64>],
        order: QuadratureOrder,
        settings: SnSettings,
    ) -> Result<Self, NeutronicsError> {
        if flux_boundary.len() != mesh.patches.len() {
            return Err(NeutronicsError::PatchCountMismatch {
                given: flux_boundary.len(),
                patches: mesh.patches.len(),
            });
        }

        let xs_fields = DiffusionXsFields::materialize(&mesh, xs, zone_of_cell, raw_parameters)?;
        let g = xs_fields.energy_groups;

        let quadrature = AngularQuadrature::level_symmetric(order);
        let n_dir = quadrature.direction_count();
        let face_phi = (0..n_dir).map(|j| quadrature.face_phi(&mesh, j)).collect();

        // Total cross section per group: removal (excludes self-scatter) plus
        // the group's own moment-0 self-scatter.
        let sigma_t = (0..g)
            .map(|i| {
                let rem = xs_fields.sigma_removal[i].internal.as_slice();
                let self_scatter = xs_fields.scattering[i][i].internal.as_slice();
                let vals: Vec<f64> = rem.iter().zip(self_scatter).map(|(r, s)| r + s).collect();
                VolScalarField::uniform(format!("sigmaT{i}"), mesh.clone(), 0.0).with_internal(vals)
            })
            .collect();

        let mut state = NeutronicsState::new(mesh.clone(), g, xs_fields.prec_groups);
        Self::apply_flux_boundary(&mut state, flux_boundary);

        // Angular flux psi[group][direction], seeded uniform 1.0, carrying the
        // caller's flux boundary conditions.
        let angular_flux: Vec<Vec<VolScalarField>> = (0..g)
            .map(|i| {
                (0..n_dir)
                    .map(|j| {
                        let mut f = VolScalarField::uniform(
                            format!("angularFlux{i}_{j}"),
                            mesh.clone(),
                            1.0,
                        );
                        Self::apply_flux_boundary_field(&mut f, flux_boundary);
                        f
                    })
                    .collect()
            })
            .collect();

        Ok(Self {
            state,
            solver: Some(SnSolverData {
                xs: xs_fields,
                quadrature,
                face_phi,
                sigma_t,
                angular_flux,
            }),
            settings,
        })
    }

    /// Set the boundary condition of every scalar group flux field.
    fn apply_flux_boundary(state: &mut NeutronicsState, flux_boundary: &[BoundaryCondition<f64>]) {
        for field in state.flux_mut() {
            Self::apply_flux_boundary_field(field, flux_boundary);
        }
    }

    /// Apply the per-patch flux boundary to one field, keeping stored patch
    /// values consistent with a `FixedValue` BC so the upwind operator sees the
    /// right incoming value.
    fn apply_flux_boundary_field(
        field: &mut VolScalarField,
        flux_boundary: &[BoundaryCondition<f64>],
    ) {
        for (patch, bc) in field.boundary.iter_mut().zip(flux_boundary.iter()) {
            patch.bc = bc.clone();
            if let BoundaryCondition::FixedValue(v) = bc {
                patch.values = Field::uniform(patch.values.len(), *v);
            }
        }
    }

    /// The shared neutronics state (flux, precursors, power density, `k_eff`).
    #[must_use]
    pub fn state(&self) -> &NeutronicsState {
        &self.state
    }

    /// The convergence / solver settings.
    #[must_use]
    pub fn settings(&self) -> &SnSettings {
        &self.settings
    }

    /// The angular quadrature, if this is a working model.
    #[must_use]
    pub fn quadrature(&self) -> Option<&AngularQuadrature> {
        self.solver.as_ref().map(|s| &s.quadrature)
    }

    /// Number of energy groups `G`.
    #[must_use]
    pub fn energy_groups(&self) -> usize {
        self.state.energy_groups()
    }

    /// Solve the S_N k-eigenvalue problem by outer power iteration.
    ///
    /// On a scaffold model (built with [`Self::new`]) this returns
    /// [`NeutronicsError::ModelNotImplemented`]. On a working model (built with
    /// [`Self::with_cross_sections`]) it runs the discrete-ordinates transport
    /// eigenvalue solve and updates the state's `k_eff`, flux, power density,
    /// one-group flux, and total power. See [`eigenvalue`].
    ///
    /// # Errors
    ///
    /// [`NeutronicsError::ModelNotImplemented`] on a scaffold model;
    /// [`NeutronicsError::NoFissionSource`] if the initial fission production is
    /// zero; [`NeutronicsError::NotConverged`] if the outer loop exhausts its
    /// budget.
    pub fn solve_eigenvalue(
        &mut self,
    ) -> Result<crate::genfoam::neutronics::EigenvalueReport, NeutronicsError> {
        if self.solver.is_none() {
            return Err(NeutronicsError::ModelNotImplemented(
                NeutronicsModelKind::Sn,
            ));
        }
        self.solve_eigenvalue_transport()
    }

    // ── shared source-assembly helpers (used by the sweep and eigenvalue) ────

    /// Borrow the solver data, panicking if this is a scaffold model. Only
    /// called from paths guarded by the `solver.is_some()` check in
    /// [`Self::solve_eigenvalue`].
    fn solver_data(&self) -> &SnSolverData {
        self.solver
            .as_ref()
            .expect("SN solver data present on a working model")
    }

    /// Fission-neutron production `S_n[c] = sum_i nu Sigma_{f,i}[c] phi_i[c]`
    /// (m^-3 s^-1), from the current scalar flux. GeN-Foam
    /// `initializeNeutroSource.H`.
    fn fission_production(&self) -> Vec<f64> {
        let xs = &self.solver_data().xs;
        let n = self.state.mesh().n_cells;
        let mut s = vec![0.0; n];
        for (i, flux) in self.state.flux().iter().enumerate() {
            let nf = xs.nu_sigma_f[i].internal.as_slice();
            let ph = flux.internal.as_slice();
            for c in 0..n {
                s[c] += nf[c] * ph[c];
            }
        }
        s
    }

    /// Domain integral `int_V S_n dV` (neutrons/s) — the positive functional the
    /// power iteration converges on.
    fn fission_integral(&self, s_fis: &[f64]) -> f64 {
        let vols = &self.state.mesh().cell_volumes;
        s_fis.iter().zip(vols.iter()).map(|(s, v)| s * v).sum()
    }

    /// Total in-scatter source into group `i` (moment 0) using the current
    /// scalar flux: `sum_{i'} Sigma_{s, i' -> i}[c] phi_{i'}[c]` (m^-3 s^-1).
    /// Includes the self-scatter term `i' = i` because the total cross section
    /// carries the group's own self-scatter (see the module docs).
    fn scattering_into(&self, i: usize) -> Vec<f64> {
        let xs = &self.solver_data().xs;
        let n = self.state.mesh().n_cells;
        let mut s = vec![0.0; n];
        for (ip, flux) in self.state.flux().iter().enumerate() {
            let sig = xs.scattering[ip][i].internal.as_slice();
            let ph = flux.internal.as_slice();
            for c in 0..n {
                s[c] += sig[c] * ph[c];
            }
        }
        s
    }

    /// Recompute `power_density`, `one_group_flux`, and `total_power` from the
    /// current scalar flux. GeN-Foam `solveNeutronicsSN.H` tail.
    fn update_derived_fields(&mut self) {
        let n = self.state.mesh().n_cells;
        let mut pd = vec![0.0; n];
        let mut ogf = vec![0.0; n];
        {
            let xs = &self.solver_data().xs;
            for (i, flux) in self.state.flux().iter().enumerate() {
                let sp = xs.sigma_pow[i].internal.as_slice();
                let ph = flux.internal.as_slice();
                for c in 0..n {
                    pd[c] += ph[c] * sp[c];
                    ogf[c] += ph[c];
                }
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
