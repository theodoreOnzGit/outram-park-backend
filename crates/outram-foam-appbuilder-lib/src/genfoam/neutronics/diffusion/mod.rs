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
//! [`fields::DiffusionXsFields`]); live thermal-hydraulic feedback, liquid-fuel
//! precursor advection, Aitken acceleration, and the implicit/integral transient
//! predictors are deferred with the coupling layer. ~~Control-rod motion and
//! discontinuity-factor adjustment are deferred~~ **CORRECTED 2026-11-27**
//! (gh:#1474, gh:#1785, README round 39): both exist for the pseudo-time
//! eigenvalue driver ([`xs_evolution::XsEvolution`], [`DiffusionNeutronics::set_xs_evolution`]),
//! bit-identical to the real `XS` class; every other solve path here (static eigenvalue, simple
//! transient, upstream-form transient, stationary and liquid-fuel steps) divides by nothing and
//! returns an error when a factor is not 1. Boundary conditions are whatever the
//! caller sets on the flux fields (vacuum = `FixedValue(0)`, reflective =
//! `ZeroGradient`).

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
pub mod fields;

#[cfg(test)]
mod tests;

use alloc::sync::Arc;

use crate::genfoam::common::linear_solvers::{LinearSolve, LinearSolverForm};
use outram_foam_basic_lib::prelude::{
    fvm::DeltaCoeff, BoundaryCondition, Field, FvMesh, SolverSettings, VolScalarField,
};

use crate::genfoam::neutronics::state::NeutronicsState;
use crate::genfoam::neutronics::xs::CrossSectionData;
use crate::genfoam::neutronics::NeutronicsError;

pub use fields::DiffusionXsFields;
pub use precursor_drift::PrecursorTransport;
pub use xs_evolution::XsEvolution;

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
    /// The implementation of the linear solves: OpenFOAM's `PCG`/`DIC`, `PBiCG`/`DILU` and
    /// `PBiCGStab`/`DILU` as the case's `fvSolution` selects them (default, gh:#1361 round 40), or
    /// the deprecated basic-lib form. See [`LinearSolverForm`].
    pub form: LinearSolverForm,
    /// Which face-distance coefficient the flux Laplacian divides by.
    ///
    /// GeN-Foam's neutronics regions ship
    /// `laplacianSchemes { default Gauss linear uncorrected; }`, so the faithful
    /// setting — and the default here — is
    /// [`DeltaCoeff::NonOrthogonal`](outram_foam_basic_lib::prelude::fvm::DeltaCoeff::NonOrthogonal),
    /// OpenFOAM's `nonOrthDeltaCoeffs`. On an orthogonal mesh the two choices
    /// coincide; on the `2D_MSFR` tutorial mesh the orthogonal one under-states
    /// the leakage by 580 pcm.
    pub delta_coeff: DeltaCoeff,
}

impl DiffusionSettings {
    /// The linear solve of this model: [`Self::linear`] interpreted in [`Self::form`].
    #[must_use]
    pub fn linear_solve(&self) -> LinearSolve {
        LinearSolve::new(self.linear, self.form)
    }

    /// The deprecated basic-lib form of the linear solves (`||r||/||b||` CG, ILU(0)-BiCGStab), kept
    /// callable; the default is OpenFOAM's solvers.
    #[must_use]
    pub fn with_basic_lib_cg(mut self) -> Self {
        self.form = LinearSolverForm::BasicLib;
        self
    }

    /// Every key of the case's `fvSolution` solver entries (`tolerance`, `relTol`, `maxIter`, `minIter`, solver and
    /// preconditioner) in place of [`Self::linear`] and the tight-protocol defaults (gh:#1887 round 42); read with
    /// `io::nuclear_data::read_neutron_fv_solution`.
    #[must_use]
    pub fn with_fv_solution(mut self, entries: crate::genfoam::common::linear_solvers::NeutronSolverEntries) -> Self {
        self.form = LinearSolverForm::UpstreamFvSolution(entries);
        self
    }
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
            form: LinearSolverForm::default(),
            delta_coeff: DeltaCoeff::NonOrthogonal,
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
    /// Circulating-fuel precursor transport, when the fuel moves.
    ///
    /// `None` is stationary fuel (GeN-Foam's `liquidFuel false`), where the
    /// delayed source collapses into `chi_eff`. `Some` is `liquidFuel true`;
    /// set it with [`Self::set_precursor_drift`].
    drift: Option<PrecursorTransport>,
    /// The albedo patches imposed so far, `(patch, gamma, linearisation)`, so
    /// that [`Self::refresh_cross_sections`] can rebuild them at the new `D`
    /// (upstream resets `Dalbedo_ = D[energyI]` on every solve).
    albedo_specs: Vec<(usize, f64, crate::genfoam::neutronics::albedo::AlbedoLinearisation)>,
    /// `precStar` (precursors per unit fluid volume) of the upstream transient
    /// step; empty until [`Self::set_prec_star`].
    prec_star: Vec<VolScalarField>,
    /// The old-time `fluxStar_` / `precStar_` of the current time step, once [`Self::store_old_times`] ran
    /// (gh:#1832); `None` is the per-call snapshot form.
    old_time: Option<UpstreamOldTime>,
    /// The external neutron source (`externalSourceNeutronics_`, `externalSourceFlux[g]`), if set
    /// (gh:#1863); [`Self::upstream_stationary_step`] adds it to every group equation.
    external_source: Option<external_source::ExternalSource>,
    /// Driveline motion and discontinuity factors of `XS::correct()` (gh:#1785, gh:#1474); `None` keeps the
    /// cross sections as materialised and the dictionary `discFactor` list.
    evolution: Option<XsEvolution>,
    /// `fluxStar_` (internal values, per group) between pseudo-time steps; `None` before the first step,
    /// when it equals the flux.
    flux_star: Option<Vec<Vec<f64>>>,
    /// `mesh.time().value()` of the step being taken (set by the pseudo-time loop).
    time: f64,
}

/// The old-time values `fvm::ddt` reads in `solveNeutronics.H`: per group the flux, per precursor group
/// `precStar`, one value per cell (gh:#1832).
#[derive(Debug, Clone)]
struct UpstreamOldTime {
    flux: Vec<Vec<f64>>,
    prec_star: Vec<Vec<f64>>,
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
        Self::assemble(mesh, xs, xs_fields, flux_boundary, settings)
    }

    /// As [`new`](Self::new), but evaluating the cross sections at **each
    /// cell's own** feedback state — the way GeN-Foam does it.
    ///
    /// `cell_parameters` is `n_cells x n_variables`, row-major; see
    /// [`DiffusionXsFields::materialize_per_cell`] for why a single global
    /// parameter vector is not equivalent.
    ///
    /// # Errors
    ///
    /// [`NeutronicsError::PatchCountMismatch`] if `flux_boundary` does not cover
    /// every patch, or an [`XsError`](crate::genfoam::neutronics::xs::XsError)
    /// from the cross-section evaluation.
    pub fn new_with_cell_parameters(
        mesh: Arc<FvMesh>,
        xs: &CrossSectionData,
        zone_of_cell: &[usize],
        cell_parameters: &[f64],
        flux_boundary: &[BoundaryCondition<f64>],
        settings: DiffusionSettings,
    ) -> Result<Self, NeutronicsError> {
        if flux_boundary.len() != mesh.patches.len() {
            return Err(NeutronicsError::PatchCountMismatch {
                given: flux_boundary.len(),
                patches: mesh.patches.len(),
            });
        }
        let xs_fields =
            DiffusionXsFields::materialize_per_cell(&mesh, xs, zone_of_cell, cell_parameters)?;
        Self::assemble(mesh, xs, xs_fields, flux_boundary, settings)
    }

    fn assemble(
        mesh: Arc<FvMesh>,
        xs: &CrossSectionData,
        xs_fields: DiffusionXsFields,
        flux_boundary: &[BoundaryCondition<f64>],
        settings: DiffusionSettings,
    ) -> Result<Self, NeutronicsError> {
        let mut state = NeutronicsState::new(mesh, xs.energy_groups(), xs.prec_groups());
        Self::apply_flux_boundary(&mut state, flux_boundary);
        Ok(Self {
            state,
            xs: xs_fields,
            settings,
            drift: None,
            albedo_specs: Vec::new(),
            prec_star: Vec::new(),
            old_time: None,
            external_source: None,
            evolution: None,
            flux_star: None,
            time: 0.0,
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

    /// Impose an **albedo** (partially reflecting) boundary on one patch, for
    /// every energy group.
    ///
    /// This is GeN-Foam's `albedoSP3` condition as the diffusion solver uses it
    /// — see [`crate::genfoam::neutronics::albedo`] for the derivation and for
    /// what of upstream's patch class is SP3-only and deliberately absent.
    ///
    /// # Why this is a method and not a constructor argument
    ///
    /// The Robin weight depends on the group's own diffusion coefficient, so a
    /// single `BoundaryCondition` handed to [`Self::new`] cannot express it —
    /// that argument is applied to every group alike. Upstream has the same
    /// requirement and meets it the same way, resetting its `Dalbedo` field to
    /// `D[energyI]` before each group's solve
    /// (`diffusion/include/fluxEq.H`). Here the per-group weight is built once,
    /// from the cross-section fields the solver already holds, because they do
    /// not change during an eigenvalue solve.
    ///
    /// Call it after [`Self::new`] and before solving. It overwrites whatever
    /// condition that patch previously carried, in every group.
    ///
    /// # Parameters
    ///
    /// - `patch_index` — index into the mesh's patch list.
    /// - `gamma` — the albedo coefficient, as GeN-Foam case dictionaries state
    ///   it directly. `0` is a perfect reflector, `0.5` a vacuum; convert from a
    ///   reflection fraction with
    ///   [`gamma_from_albedo`](crate::genfoam::neutronics::albedo::gamma_from_albedo).
    /// - `linearisation` — which `phi` the leakage current is evaluated at; see
    ///   [`AlbedoLinearisation`](crate::genfoam::neutronics::albedo::AlbedoLinearisation).
    ///   Pass `CellValue` to match GeN-Foam.
    ///
    /// # Panics
    ///
    /// Panics if `patch_index` is out of range or `gamma` is negative or
    /// non-finite.
    pub fn set_albedo_boundary(
        &mut self,
        patch_index: usize,
        gamma: f64,
        linearisation: crate::genfoam::neutronics::albedo::AlbedoLinearisation,
    ) {
        let mesh = self.state.mesh().clone();
        let patches: Vec<_> = (0..self.energy_groups())
            .map(|g| {
                crate::genfoam::neutronics::albedo::albedo_patch_field(
                    &mesh,
                    patch_index,
                    gamma,
                    &self.xs.d[g],
                    linearisation,
                    self.settings.delta_coeff,
                )
            })
            .collect();
        for (g, field) in self.state.flux_mut().iter_mut().enumerate() {
            field.boundary[patch_index] = patches[g].clone();
        }
        self.albedo_specs.retain(|(p, _, _)| *p != patch_index);
        self.albedo_specs.push((patch_index, gamma, linearisation));
    }

    /// Turn on **circulating-fuel precursor drift** — GeN-Foam's
    /// `liquidFuel true`.
    ///
    /// With this set, [`Self::solve_eigenvalue`] transports each delayed-neutron
    /// precursor group through the moving fuel and sources the delayed neutrons
    /// where the precursors have actually reached, instead of assuming they
    /// decay where they were born. See
    /// [`precursor_drift`] for the equation, the reduction to the stationary
    /// case, and the measured worth on the MSFR tutorial.
    ///
    /// Call it before solving. Passing the fields for a stationary fuel (zero
    /// `phi`, zero diffusivity) is allowed and reproduces the `chi_eff`
    /// collapse, but is wasted work — leave it unset instead.
    ///
    /// # Note
    ///
    /// Only the eigenvalue solve and [`Self::upstream_transient_step`] honour
    /// this. [`Self::step`] (the simple transient) still uses the `chi_eff`
    /// collapse. ~~because the transient precursor equation keeps a time
    /// derivative this does not yet carry~~ **CORRECTED 2026-11-25**: the
    /// time-derivative form is `upstream_transient_step`.
    pub fn set_precursor_drift(&mut self, transport: PrecursorTransport) {
        self.drift = Some(transport);
    }

    /// The circulating-fuel precursor transport fields, if set.
    #[must_use]
    pub fn precursor_drift(&self) -> Option<&PrecursorTransport> {
        self.drift.as_ref()
    }

    /// The shared neutronics state (flux, precursors, power density, `k_eff`).
    #[must_use]
    pub fn state(&self) -> &NeutronicsState {
        &self.state
    }

    /// Replace the linear-solver form (for example with the entries read from the case's `fvSolution`).
    pub fn set_linear_form(&mut self, form: LinearSolverForm) {
        self.settings.form = form;
    }

    /// Seed the next [`Self::solve_eigenvalue`] with a previous solution — a
    /// **warm start**: the group fluxes (internal values, one `Vec` per group,
    /// one value per cell) and `k_eff`. Upstream gets this for free, because
    /// GeN-Foam's flux fields persist between coupling iterations and time
    /// steps and each power iteration starts from them; a model rebuilt with
    /// new cross sections here would otherwise restart from a flat flux.
    ///
    /// The amplitude does not matter (the power iteration renormalises), and
    /// the boundary conditions are kept.
    ///
    /// # Panics
    ///
    /// If `flux` does not hold one vector per energy group, each one value per
    /// mesh cell, or if `k_eff` is not positive and finite.
    pub fn set_initial_guess(&mut self, flux: &[Vec<f64>], k_eff: f64) {
        let n = self.state.mesh().n_cells;
        assert_eq!(flux.len(), self.energy_groups(), "one flux per energy group");
        assert!(k_eff.is_finite() && k_eff > 0.0, "k_eff must be positive");
        for (field, values) in self.state.flux_mut().iter_mut().zip(flux) {
            assert_eq!(values.len(), n, "one flux value per cell");
            field.internal.as_mut_slice().copy_from_slice(values);
        }
        self.state.set_k_eff_raw(k_eff);
        self.flux_star = None;
    }

    /// Switch on `XS::correct()`'s time dependence for the pseudo-time eigenvalue driver: control-rod
    /// driveline motion and discontinuity factors (gh:#1474, gh:#1785). Without it the cross sections stay
    /// as materialised, with the dictionary `discFactor` (upstream's form at `timeIndex > 0`).
    pub fn set_xs_evolution(&mut self, evolution: XsEvolution) {
        self.evolution = Some(evolution);
    }

    /// The earlier form of this model: every discontinuity factor 1, whatever the dictionary says (the port
    /// had no factor before gh:#1785). A named ablation, never the default; it does not survive
    /// [`Self::set_xs_evolution`], which re-reads the lists at every step.
    #[must_use]
    pub fn without_disc_factors(mut self) -> Self {
        for field in self.xs.disc_factor.iter_mut() {
            for v in field.internal.as_mut_slice() {
                *v = 1.0;
            }
        }
        self
    }

    /// The evolution, if set.
    #[must_use]
    pub fn xs_evolution(&self) -> Option<&XsEvolution> {
        self.evolution.as_ref()
    }

    /// `fluxStar_` after the last pseudo-time step (`None` before the first).
    #[must_use]
    pub fn flux_star(&self) -> Option<&[Vec<f64>]> {
        self.flux_star.as_deref()
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
pub mod external_source;
#[cfg(feature = "std")]
pub mod external_source_case;
pub mod precursor_drift;
mod pseudo_time;
mod transient;
pub mod expansion;
pub mod xs_evolution;
pub mod upstream_eigenvalue;
pub mod upstream_transient;
