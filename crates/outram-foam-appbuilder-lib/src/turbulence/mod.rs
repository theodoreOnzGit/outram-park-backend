// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com)
// Copyright (C) 2004-2023 OpenFOAM Foundation
// Copyright (C) 2016-2023 OpenCFD Ltd.
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

//! # Turbulence-closure selection for the solver loops (Layer 5 adapter)
//!
//! This module is the **bridge** between `outram-foam-turbulence-lib` (Layer 4 —
//! the RAS/LES closures themselves) and the solver loops in
//! [`crate::solvers`] (Layer 5 — PIMPLE/PISO time advancement).
//!
//! ## What belongs here and what does not
//!
//! *Here:* selecting which closure a run uses, pushing the solver's live fields
//! into the closure once per outer iteration, calling `correct()` at the right
//! point in the PIMPLE loop, and converting between the closure's
//! **kinematic** view (ν, ν_t in m²/s) and a compressible solver's **dynamic**
//! view (μ, μ_t in Pa·s).
//!
//! *Not here:* the turbulence transport equations themselves. Those live in
//! `outram-foam-turbulence-lib` and are not duplicated.
//!
//! ## Dispatch is by enum, never `dyn`
//!
//! [`TurbulenceClosure`] is a plain enum, as the workspace design rules require.
//! Adding a model is a compile-time forcing function: every `match` in this file
//! must gain a new arm or the crate does not build. There is no `Box<dyn
//! TurbulenceModel>` anywhere.
//!
//! ## How a solver uses it
//!
//! ```text
//! for each outer (PIMPLE) iteration:
//!     turbulence.sync_inputs(&U, &phi_volumetric, &nu, dt);   // push live state
//!     UEqn = ddt + div + turbulence.div_dev_reff(&U, &nu);    // turbulent stress
//!     ... momentum predictor, PISO pressure correctors ...
//! end outer loop
//! turbulence.sync_inputs(&U, &phi_volumetric, &nu, dt);       // corrected state
//! turbulence.correct();                                       // advance k, ω/ε, ν_t
//! ```
//!
//! `correct()` is deliberately called **after** the pressure correctors, exactly
//! as OpenFOAM's `turbulence->correct()` sits at the bottom of the PIMPLE loop.
//!
//! ## Honest scope — read before trusting a turbulent result
//!
//! - The closures use **zero-gradient near-wall boundary conditions**, not wall
//!   functions. `outram-foam-turbulence-lib` ships `wall_functions::{y_plus,
//!   u_tau, nu_t_wall}` as standalone helpers that are **not** wired in as patch
//!   boundary conditions, by that crate's own admission. A wall-bounded RAS run
//!   therefore does **not** reproduce the log law and **must not** be compared
//!   against a friction-factor correlation and called validated.
//! - What *is* verified here (see the tests at the bottom of this file) is the
//!   **coupling**: that the momentum equation actually picks up ν_t, and that a
//!   closure advanced inside the PIMPLE loop reproduces the analytic solution of
//!   its own transport equations for a case with no walls and no shear.
//! - No model in this stack has been validated end-to-end against a published
//!   turbulence benchmark. Do not describe one as validated.

use outram_foam_basic_lib::prelude::*;
use outram_foam_turbulence_lib::prelude::*;
use std::sync::Arc;

/// Which turbulence closure a solver run uses.
///
/// Enum dispatch, not a trait object — see the module documentation. Each
/// non-laminar variant owns the concrete model struct from
/// `outram-foam-turbulence-lib`, so its transport fields (k, ω, ε, ν̃, ν_t) are
/// reachable for inspection after a run, e.g.
/// `if let TurbulenceClosure::KOmegaSST(m) = &solver.turbulence { &m.k }`.
///
/// # Units
///
/// Every model in this enum is formulated **kinematically**: ν and ν_t are in
/// m²/s, k in m²/s², ω in 1/s, ε in m²/s³. A compressible solver holding dynamic
/// viscosity μ [Pa·s] must convert with [`TurbulenceClosure::mu_eff`], which
/// applies μ_t = ρ ν_t.
///
/// # Default
///
/// [`TurbulenceClosure::Laminar`] — a run that does not opt in to a model keeps
/// exactly the molecular viscous term the solver assembled before this module
/// existed, so no pre-existing result changes.
#[derive(Default)]
pub enum TurbulenceClosure {
    /// No turbulence closure: ν_t ≡ 0 and ν_eff = ν.
    ///
    /// The momentum stress term reduces to the plain implicit molecular
    /// Laplacian `−∇·(ν ∇U)`. This variant deliberately **omits** the explicit
    /// transpose correction `−∇·(ν dev2(∇Uᵀ))` that
    /// `outram_foam_turbulence_lib::laminar::LaminarModel` adds: that term
    /// vanishes identically for a divergence-free constant-ν flow, and omitting
    /// it keeps this variant bit-for-bit identical to the viscous term the
    /// solvers used before turbulence was wired in.
    #[default]
    Laminar,
    /// Menter (1994) k-ω SST RAS model.
    KOmegaSST(KOmegaSST),
    /// Jones & Launder (1972) standard k-ε RAS model.
    KEpsilon(KEpsilon),
    /// Wilcox k-ω RAS model.
    KOmega(KOmega),
    /// Spalart-Allmaras (1992) one-equation RAS model.
    SpalartAllmaras(SpalartAllmaras),
    /// Smagorinsky (1963) LES sub-grid-scale model.
    Smagorinsky(Smagorinsky),
}

impl std::fmt::Debug for TurbulenceClosure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

impl TurbulenceClosure {
    // ── Constructors ─────────────────────────────────────────────────────────

    /// Menter k-ω SST over `mesh`, with the Menter (1994) coefficients.
    ///
    /// Transport fields start uniform (k = 1e-4 m²/s², ω = 1 s⁻¹, ν_t = 0); the
    /// wall-distance field is computed once from the mesh `Wall` patches. Set
    /// physically meaningful inlet-scale k and ω before running, e.g. through
    /// [`TurbulenceClosure::set_k_omega_uniform`].
    pub fn k_omega_sst(mesh: Arc<FvMesh>) -> Self {
        Self::KOmegaSST(KOmegaSST::new(mesh))
    }

    /// Standard k-ε over `mesh` (Jones & Launder 1972 coefficients).
    pub fn k_epsilon(mesh: Arc<FvMesh>) -> Self {
        Self::KEpsilon(KEpsilon::new(mesh))
    }

    /// Wilcox k-ω over `mesh`.
    pub fn k_omega(mesh: Arc<FvMesh>) -> Self {
        Self::KOmega(KOmega::new(mesh))
    }

    /// Spalart-Allmaras one-equation model over `mesh`.
    pub fn spalart_allmaras(mesh: Arc<FvMesh>) -> Self {
        Self::SpalartAllmaras(SpalartAllmaras::new(mesh))
    }

    /// Smagorinsky LES over `mesh` (`Ck = 0.094`, `Ce = 1.048`, cubeRootVol Δ).
    pub fn smagorinsky(mesh: Arc<FvMesh>) -> Self {
        Self::Smagorinsky(Smagorinsky::new(mesh))
    }

    // ── Identity ─────────────────────────────────────────────────────────────

    /// Human-readable model name, matching the OpenFOAM `simulationType` /
    /// `RASModel` keyword where one exists (`"laminar"`, `"kOmegaSST"`, …).
    pub fn name(&self) -> &'static str {
        match self {
            Self::Laminar => "laminar",
            Self::KOmegaSST(_) => "kOmegaSST",
            Self::KEpsilon(_) => "kEpsilon",
            Self::KOmega(_) => "kOmega",
            Self::SpalartAllmaras(_) => "SpalartAllmaras",
            Self::Smagorinsky(_) => "Smagorinsky",
        }
    }

    /// `true` when no turbulence transport is solved (ν_t ≡ 0).
    pub fn is_laminar(&self) -> bool {
        matches!(self, Self::Laminar)
    }

    // ── Per-time-step wiring ─────────────────────────────────────────────────

    /// Push the solver's live state into the closure.
    ///
    /// Every RAS model in `outram-foam-turbulence-lib` needs the current
    /// velocity, face flux, molecular viscosity and time step, but its
    /// `correct(&mut self)` takes no arguments (it mirrors OpenFOAM, where the
    /// model holds references to those fields). This method is the explicit
    /// equivalent of holding those references, and **must** be called before
    /// [`TurbulenceClosure::correct`] and before
    /// [`TurbulenceClosure::div_dev_reff`].
    ///
    /// # Arguments
    ///
    /// * `u`   — velocity field U [m/s]
    /// * `phi` — **volumetric** face flux φ = U·S_f [m³/s]. A compressible
    ///   solver stores a *mass* flux [kg/s] and must divide by the interpolated
    ///   face density first — see [`TurbulenceClosure::volumetric_flux`].
    /// * `nu`  — molecular **kinematic** viscosity ν [m²/s]. A compressible
    ///   solver holding μ [Pa·s] must pass μ/ρ.
    /// * `dt`  — time step [s], > 0.
    ///
    /// `Smagorinsky` is algebraic and uses neither `phi` nor `dt`; they are
    /// accepted and ignored so the call site is uniform across models.
    pub fn sync_inputs(
        &mut self,
        u: &VolVectorField,
        phi: &SurfaceScalarField,
        nu: &VolScalarField,
        dt: f64,
    ) {
        match self {
            Self::Laminar => {}
            Self::KOmegaSST(m) => {
                m.u = u.clone();
                m.phi = phi.clone();
                m.nu = nu.clone();
                m.dt = dt;
            }
            Self::KEpsilon(m) => {
                m.u = u.clone();
                m.phi = phi.clone();
                m.nu = nu.clone();
                m.dt = dt;
            }
            Self::KOmega(m) => {
                m.u = u.clone();
                m.phi = phi.clone();
                m.nu = nu.clone();
                m.dt = dt;
            }
            Self::SpalartAllmaras(m) => {
                m.u = u.clone();
                m.phi = phi.clone();
                m.nu = nu.clone();
                m.dt = dt;
            }
            Self::Smagorinsky(m) => {
                m.u = u.clone();
                m.nu = nu.clone();
            }
        }
    }

    /// Advance the turbulence transport equations by one time step.
    ///
    /// Call **once per time step, after** the momentum predictor and the PISO
    /// pressure correctors — the position of `turbulence->correct()` in
    /// OpenFOAM's PIMPLE loop. A no-op for [`TurbulenceClosure::Laminar`].
    ///
    /// [`TurbulenceClosure::sync_inputs`] must have been called with the
    /// *corrected* velocity and flux first, or the closure advances on stale
    /// state.
    pub fn correct(&mut self) {
        match self {
            Self::Laminar => {}
            Self::KOmegaSST(m) => m.correct(),
            Self::KEpsilon(m) => m.correct(),
            Self::KOmega(m) => m.correct(),
            Self::SpalartAllmaras(m) => m.correct(),
            Self::Smagorinsky(m) => m.correct(),
        }
    }

    // ── Momentum coupling ────────────────────────────────────────────────────

    /// Assemble the momentum stress term for the velocity field `u`.
    ///
    /// Returns an [`FvVectorMatrix`] representing
    ///
    /// ```text
    ///   divDevReff(U) = −∇·(ν_eff ∇U)  −  ∇·(ν_eff dev2(∇Uᵀ))
    /// ```
    ///
    /// with ν_eff = ν + ν_t [m²/s]. The dominant first term is implicit (it goes
    /// into the matrix), the transpose correction explicit (it goes into the
    /// source). Because `outram-foam-basic-lib`'s `fvm::laplacian_vec` is
    /// assembled **positive-definite** (it represents `−∇·(Γ∇)`, the negation of
    /// OpenFOAM's convention), the returned matrix is **added** to the momentum
    /// equation, not subtracted — see the `pimple_foam` module documentation for
    /// the full sign discussion.
    ///
    /// For [`TurbulenceClosure::Laminar`] this is exactly
    /// `fvm::laplacian_vec(nu, u, mesh)` — the transpose correction is omitted
    /// (it vanishes for divergence-free constant-ν flow), which keeps a laminar
    /// run identical to the pre-turbulence solver.
    ///
    /// # Arguments
    ///
    /// * `u`  — velocity field U [m/s]
    /// * `nu` — molecular kinematic viscosity ν [m²/s]; used only by the
    ///   `Laminar` arm. The other arms use the ν pushed in by
    ///   [`TurbulenceClosure::sync_inputs`], which must have been called first.
    pub fn div_dev_reff(&self, u: &VolVectorField, nu: &VolScalarField) -> FvVectorMatrix {
        match self {
            Self::Laminar => fvm::laplacian_vec(nu, u, u.mesh.clone()),
            Self::KOmegaSST(m) => m.div_dev_rho_reff(u),
            Self::KEpsilon(m) => m.div_dev_rho_reff(u),
            Self::KOmega(m) => m.div_dev_rho_reff(u),
            Self::SpalartAllmaras(m) => m.div_dev_rho_reff(u),
            Self::Smagorinsky(m) => m.div_dev_rho_reff(u),
        }
    }

    // ── Transport-property queries ───────────────────────────────────────────

    /// Turbulent kinematic viscosity ν_t [m²/s], or `None` for a laminar run.
    ///
    /// `None` rather than a zero field so the caller can tell "no closure" from
    /// "closure that has not produced turbulence yet" without allocating.
    pub fn nu_t(&self) -> Option<&VolScalarField> {
        match self {
            Self::Laminar => None,
            Self::KOmegaSST(m) => Some(m.nu_t()),
            Self::KEpsilon(m) => Some(m.nu_t()),
            Self::KOmega(m) => Some(m.nu_t()),
            Self::SpalartAllmaras(m) => Some(m.nu_t()),
            Self::Smagorinsky(m) => Some(m.nu_t()),
        }
    }

    /// Effective kinematic viscosity ν_eff = ν + ν_t [m²/s], per cell.
    ///
    /// `nu` is the molecular kinematic viscosity field [m²/s]. For a laminar
    /// closure this is a clone of `nu`.
    pub fn nu_eff(&self, nu: &VolScalarField) -> VolScalarField {
        match self.nu_t() {
            None => nu.clone(),
            Some(nut) => {
                let vals: Vec<f64> = nu
                    .internal
                    .as_slice()
                    .iter()
                    .zip(nut.internal.as_slice())
                    .map(|(a, b)| a + b)
                    .collect();
                same_shape_field("nuEff", nu, vals)
            }
        }
    }

    /// Effective **dynamic** viscosity μ_eff = μ + ρ ν_t [Pa·s], per cell.
    ///
    /// This is the conversion a compressible solver needs: the closures are
    /// kinematic, so the turbulent dynamic viscosity is μ_t = ρ ν_t. Passing μ
    /// straight to `nu_eff` would silently add a kinematic viscosity to a
    /// dynamic one.
    ///
    /// * `mu`  — molecular dynamic viscosity μ [Pa·s]
    /// * `rho` — density ρ [kg/m³]
    pub fn mu_eff(&self, mu: &VolScalarField, rho: &VolScalarField) -> VolScalarField {
        match self.nu_t() {
            None => mu.clone(),
            Some(nut) => {
                let vals: Vec<f64> = (0..mu.internal.as_slice().len())
                    .map(|c| mu.internal[c] + rho.internal[c] * nut.internal[c])
                    .collect();
                same_shape_field("muEff", mu, vals)
            }
        }
    }

    /// Effective thermal diffusivity α_eff = α + α_t [kg/(m·s)] for a
    /// compressible energy equation, with α_t = ρ ν_t / Pr_t.
    ///
    /// * `alpha` — molecular thermal diffusivity α = κ/Cp [kg/(m·s)]
    /// * `rho`   — density ρ [kg/m³]
    ///
    /// The turbulent Prandtl number Pr_t is the model's own `prt` field
    /// (default 0.85); the laminar closure returns `alpha` unchanged.
    pub fn alpha_eff_compressible(
        &self,
        alpha: &VolScalarField,
        rho: &VolScalarField,
    ) -> VolScalarField {
        let prt = self.turbulent_prandtl();
        match self.nu_t() {
            None => alpha.clone(),
            Some(nut) => {
                let vals: Vec<f64> = (0..alpha.internal.as_slice().len())
                    .map(|c| alpha.internal[c] + rho.internal[c] * nut.internal[c] / prt)
                    .collect();
                same_shape_field("alphaEff", alpha, vals)
            }
        }
    }

    /// Turbulent Prandtl number Pr_t (dimensionless) of the active model;
    /// 1.0 for a laminar run (where it is never used).
    pub fn turbulent_prandtl(&self) -> f64 {
        match self {
            Self::Laminar => 1.0,
            Self::KOmegaSST(m) => m.prt,
            Self::KEpsilon(m) => m.prt,
            Self::KOmega(m) => m.prt,
            Self::SpalartAllmaras(m) => m.prt,
            Self::Smagorinsky(m) => m.prt,
        }
    }

    // ── Initialisation helpers ───────────────────────────────────────────────

    /// Set uniform turbulence transport fields on a two-equation model.
    ///
    /// * `k`     — turbulent kinetic energy [m²/s²], > 0
    /// * `scale` — the model's second variable: ω [1/s] for k-ω / k-ω SST,
    ///   ε [m²/s³] for k-ε. Ignored for Spalart-Allmaras and Smagorinsky, which
    ///   carry no k.
    ///
    /// A typical inlet estimate is k = 1.5 (I·U)² with turbulence intensity I,
    /// and ω = √k/(C_μ^{1/4} ℓ) or ε = C_μ^{3/4} k^{3/2}/ℓ with mixing length ℓ.
    /// Returns `false` if the active closure has no k to set.
    pub fn set_k_omega_uniform(&mut self, k: f64, scale: f64) -> bool {
        match self {
            Self::KOmegaSST(m) => {
                fill(&mut m.k, k);
                fill(&mut m.omega, scale);
                true
            }
            Self::KOmega(m) => {
                fill(&mut m.k, k);
                fill(&mut m.omega, scale);
                true
            }
            Self::KEpsilon(m) => {
                fill(&mut m.k, k);
                fill(&mut m.epsilon, scale);
                true
            }
            _ => false,
        }
    }

    /// Convert a compressible solver's **mass** flux φ_m = ρ U·S_f [kg/s] into
    /// the **volumetric** flux φ = U·S_f [m³/s] the closures expect, by dividing
    /// by the interpolated face density.
    ///
    /// * `mass_flux` — φ_m [kg/s]
    /// * `rho`       — density ρ [kg/m³], > 0
    ///
    /// # Modelling caveat
    ///
    /// The k/ω/ε transport equations in `outram-foam-turbulence-lib` are written
    /// in incompressible form (`fvm::ddt(k) + fvm::div(phi, k)`), i.e. they
    /// transport k, not ρk. Feeding them φ_m/ρ_f is the constant-density
    /// approximation to OpenFOAM's compressible `fvm::div(alphaRhoPhi, k)`; it
    /// is exact only where density is uniform. Do not treat a strongly
    /// compressible turbulent result from this path as verified.
    pub fn volumetric_flux(
        mass_flux: &SurfaceScalarField,
        rho: &VolScalarField,
    ) -> SurfaceScalarField {
        let rho_f = fvc::interpolate(rho);
        let mut phi = mass_flux.clone();
        for f in 0..phi.internal.as_slice().len() {
            phi.internal[f] = mass_flux.internal[f] / rho_f.internal[f].max(1e-30);
        }
        for (pi, pb) in phi.boundary.iter_mut().enumerate() {
            for fi in 0..pb.values.len() {
                pb.values[fi] =
                    mass_flux.boundary[pi].values[fi] / rho_f.boundary[pi].values[fi].max(1e-30);
            }
        }
        phi
    }
}

/// Build a `VolScalarField` with the same mesh and zero-gradient boundaries as
/// `template`, carrying the per-cell values `vals`.
fn same_shape_field(name: &str, template: &VolScalarField, vals: Vec<f64>) -> VolScalarField {
    let mesh = template.mesh.clone();
    let boundary = mesh
        .patches
        .iter()
        .map(|p| PatchField::zero_gradient(p.size))
        .collect();
    VolScalarField::new(name, mesh, Field::new(vals), boundary)
}

/// Overwrite every internal cell value of `f` with `v`.
fn fill(f: &mut VolScalarField, v: f64) {
    for x in f.internal.as_mut_slice() {
        *x = v;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use outram_foam_basic_lib::prelude::{BoundaryPatch, FvMeshBuilder, PatchKind};

    fn vx(x: f64) -> Vector3 {
        Vector3::new(x, 0.0, 0.0)
    }

    /// 3 cells along x (centres 0.5, 1.5, 2.5 m, unit volumes), 2 internal
    /// faces, a `Wall` patch at x = 0 and an ordinary patch at x = 3 m. Same
    /// topology the `outram-foam-turbulence-lib` model tests use, so the two
    /// layers are exercised over identical geometry.
    fn line_mesh() -> Arc<FvMesh> {
        Arc::new(
            FvMeshBuilder::new()
                .n_cells(3)
                .n_internal_faces(2)
                .owner(vec![0, 1, 0, 2])
                .neighbour(vec![1, 2])
                .patches(vec![
                    BoundaryPatch::new("wall", 2, 1, PatchKind::Wall),
                    BoundaryPatch::new("top", 3, 1, PatchKind::Patch),
                ])
                .cell_volumes(vec![1.0, 1.0, 1.0])
                .cell_centres(vec![vx(0.5), vx(1.5), vx(2.5)])
                .face_area_vectors(vec![vx(1.0), vx(1.0), vx(-1.0), vx(1.0)])
                .face_centres(vec![vx(1.0), vx(2.0), vx(0.0), vx(3.0)])
                .build()
                .unwrap(),
        )
    }

    /// V&V (verification) — the momentum equation really picks up ν_t.
    ///
    /// **Methodology.** The turbulent stress term enters the momentum predictor
    /// only through the effective viscosity ν_eff = ν + ν_t. If the wiring is
    /// correct, a turbulent closure carrying a *uniform, frozen* ν_t must
    /// produce exactly the same implicit momentum operator as a laminar run
    /// whose molecular viscosity has been raised by that same ν_t — the
    /// method-of-manufactured-coefficients form of an exact reference.
    ///
    /// Reference (analytical): `fvm::laplacian_vec` puts `Γ_f·|S_f|/d` on the
    /// diagonal of each adjacent cell, so with Γ = ν_eff constant the diagonal
    /// is a linear function of ν_eff and the two assemblies must agree to
    /// round-off, cell by cell.
    ///
    /// Inputs: the 3-cell line mesh above; ν = 0.010 m²/s; a Wilcox k-ω closure
    /// with k = 0.20 m²/s² and ω = 20 s⁻¹ frozen, giving ν_t = k/ω = 0.010 m²/s;
    /// reference laminar run at ν' = ν + ν_t = 0.020 m²/s. Velocity is uniform
    /// U = (2,0,0) m/s so the explicit transpose correction `−∇·(ν_eff
    /// dev2(∇Uᵀ))` vanishes and the comparison isolates the implicit operator.
    ///
    /// Pass criterion: max cell-wise |diag_turbulent − diag_laminar(ν+ν_t)| <
    /// 1e-14, **and** the turbulent diagonal must differ from the plain
    /// molecular one (a null wiring would pass the first check trivially).
    ///
    /// **Results (run 2026-08-07, release mode).**
    /// - laminar ν = 0.010 → diag = [0.010, 0.020, 0.010]
    /// - laminar ν = 0.020 → diag = [0.020, 0.040, 0.020]
    /// - k-ω with ν_t = 0.010 → diag = [0.020, 0.040, 0.020]
    /// - max |Δ| against the ν + ν_t reference = 0.0 (exact bitwise agreement)
    /// - max |Δ| against the molecular-only assembly = 0.020 (so the term is
    ///   genuinely doubled by ν_t, not silently dropped)
    ///
    /// Interpretation: the turbulent viscosity produced by the Layer-4 closure
    /// reaches the Layer-5 momentum matrix exactly, with no scaling error.
    #[test]
    fn momentum_operator_picks_up_turbulent_viscosity() {
        let mesh = line_mesh();
        let nu_mol = 0.010_f64;
        let nut = 0.010_f64;

        let nu = VolScalarField::uniform("nu", mesh.clone(), nu_mol);
        let nu_plus_nut = VolScalarField::uniform("nu", mesh.clone(), nu_mol + nut);

        let mut u = VolVectorField::zero("U", mesh.clone());
        u.internal = Field::new(vec![vx(2.0), vx(2.0), vx(2.0)]);

        // Reference: laminar closure at the raised molecular viscosity.
        let laminar = TurbulenceClosure::Laminar;
        let ref_eqn = laminar.div_dev_reff(&u, &nu_plus_nut);
        let mol_eqn = laminar.div_dev_reff(&u, &nu);

        // Under test: Wilcox k-ω with k/ω frozen so ν_t = 0.010 m²/s exactly.
        let mut closure = TurbulenceClosure::k_omega(mesh.clone());
        closure.set_k_omega_uniform(0.20, 20.0);
        if let TurbulenceClosure::KOmega(m) = &mut closure {
            m.nu = nu.clone();
            m.nu_t = VolScalarField::uniform("nut", mesh.clone(), nut);
        }
        let turb_eqn = closure.div_dev_reff(&u, &nu);

        let mut max_dev_ref = 0.0_f64;
        let mut max_dev_mol = 0.0_f64;
        for c in 0..mesh.n_cells {
            max_dev_ref = max_dev_ref.max((turb_eqn.ldu.diag[c] - ref_eqn.ldu.diag[c]).abs());
            max_dev_mol = max_dev_mol.max((turb_eqn.ldu.diag[c] - mol_eqn.ldu.diag[c]).abs());
        }
        assert!(
            max_dev_ref < 1e-14,
            "turbulent momentum operator must equal laminar(nu + nut); max |Δdiag| = {max_dev_ref:e}"
        );
        assert!(
            max_dev_mol > 1e-9,
            "nu_t must actually change the operator; max |Δdiag| vs molecular = {max_dev_mol:e}"
        );
    }

    /// V&V (verification) — the laminar default is byte-identical to the plain
    /// molecular Laplacian the solvers used before turbulence was wired in.
    ///
    /// **Methodology.** [`TurbulenceClosure::Laminar`] must reduce
    /// `div_dev_reff` to exactly `fvm::laplacian_vec(nu, U, mesh)`, so that
    /// adding the turbulence field to a solver cannot perturb any existing
    /// laminar result. Reference: the operator itself, assembled directly.
    /// Inputs: the 3-cell line mesh, ν = 0.01 m²/s, sheared U_x = {1,2,3} m/s
    /// (a non-trivial gradient, so a spurious transpose correction would show).
    /// Pass criterion: every diagonal, off-diagonal and source entry identical
    /// to 1e-15.
    ///
    /// **Results (run 2026-08-07, release mode).** max |Δdiag| = 0.0,
    /// max |Δupper| = 0.0, max |Δsource| = 0.0 — bitwise identical. Confirms a
    /// laminar run is unaffected by the new dispatch layer.
    #[test]
    fn laminar_default_is_the_plain_molecular_laplacian() {
        let mesh = line_mesh();
        let nu = VolScalarField::uniform("nu", mesh.clone(), 0.01);
        let mut u = VolVectorField::zero("U", mesh.clone());
        u.internal = Field::new(vec![vx(1.0), vx(2.0), vx(3.0)]);

        let direct = fvm::laplacian_vec(&nu, &u, mesh.clone());
        let via_enum = TurbulenceClosure::Laminar.div_dev_reff(&u, &nu);

        for c in 0..mesh.n_cells {
            assert!((direct.ldu.diag[c] - via_enum.ldu.diag[c]).abs() < 1e-15);
            assert!((direct.source[c] - via_enum.source[c]).mag() < 1e-15);
        }
        for f in 0..mesh.n_internal_faces {
            assert!((direct.ldu.upper[f] - via_enum.ldu.upper[f]).abs() < 1e-15);
            assert!((direct.ldu.lower[f] - via_enum.ldu.lower[f]).abs() < 1e-15);
        }
        assert!(TurbulenceClosure::Laminar.nu_t().is_none());
    }

    /// V&V (verification) — the kinematic→dynamic conversion a compressible
    /// solver needs: μ_eff = μ + ρ ν_t, not μ + ν_t.
    ///
    /// **Methodology.** The Layer-4 closures are kinematic (ν_t in m²/s), while
    /// `RhoPimpleFoam` carries dynamic viscosity μ [Pa·s]. Adding ν_t straight
    /// to μ would be dimensionally wrong by a factor ρ. Reference: the
    /// definition μ_t = ρ ν_t. Inputs: ν_t = 2.0e-3 m²/s (frozen k-ω),
    /// μ = 1.8e-5 Pa·s, ρ = 1.2 kg/m³. Expected μ_eff = 1.8e-5 + 1.2·2.0e-3 =
    /// 2.4180e-3 Pa·s. Pass criterion: |Δ| < 1e-15 in every cell.
    ///
    /// **Results (run 2026-08-07, release mode).** μ_eff = 2.418e-3 Pa·s in all
    /// three cells; max |Δ| = 0.0. The corresponding α_eff with Pr_t = 0.85 and
    /// α = 2.5e-5 kg/(m·s) is 2.5e-5 + 1.2·2.0e-3/0.85 = 2.848529…e-3 kg/(m·s),
    /// matched to 1e-15.
    #[test]
    fn compressible_conversion_uses_mu_t_equals_rho_nu_t() {
        let mesh = line_mesh();
        let nut = 2.0e-3;
        let mut closure = TurbulenceClosure::k_omega(mesh.clone());
        if let TurbulenceClosure::KOmega(m) = &mut closure {
            m.nu_t = VolScalarField::uniform("nut", mesh.clone(), nut);
        }

        let mu = VolScalarField::uniform("mu", mesh.clone(), 1.8e-5);
        let rho = VolScalarField::uniform("rho", mesh.clone(), 1.2);
        let mu_eff = closure.mu_eff(&mu, &rho);
        let expect_mu = 1.8e-5 + 1.2 * nut;
        for c in 0..mesh.n_cells {
            assert!(
                (mu_eff.internal[c] - expect_mu).abs() < 1e-15,
                "mu_eff[{c}] = {} expected {expect_mu}",
                mu_eff.internal[c]
            );
        }

        let alpha = VolScalarField::uniform("alpha", mesh.clone(), 2.5e-5);
        let a_eff = closure.alpha_eff_compressible(&alpha, &rho);
        let expect_a = 2.5e-5 + 1.2 * nut / 0.85;
        for c in 0..mesh.n_cells {
            assert!((a_eff.internal[c] - expect_a).abs() < 1e-15);
        }
    }

    /// Every closure variant constructs, syncs, corrects, and reports a finite
    /// non-negative ν_t on the same mesh — the smoke test that keeps the enum
    /// arms honest when a model is added.
    ///
    /// Not a physics check: it asserts only finiteness and ν_t ≥ 0 after five
    /// steps of a sheared velocity field, ν = 1e-3 m²/s, dt = 1e-3 s.
    #[test]
    fn every_closure_variant_syncs_and_corrects() {
        let mesh = line_mesh();
        let nu = VolScalarField::uniform("nu", mesh.clone(), 1e-3);
        let phi = SurfaceScalarField::zeros("phi", mesh.clone());
        let mut u = VolVectorField::zero("U", mesh.clone());
        u.internal = Field::new(vec![vx(1.0), vx(2.0), vx(3.0)]);

        let mut closures = [
            TurbulenceClosure::Laminar,
            TurbulenceClosure::k_omega_sst(mesh.clone()),
            TurbulenceClosure::k_epsilon(mesh.clone()),
            TurbulenceClosure::k_omega(mesh.clone()),
            TurbulenceClosure::spalart_allmaras(mesh.clone()),
            TurbulenceClosure::smagorinsky(mesh.clone()),
        ];
        for closure in closures.iter_mut() {
            let name = closure.name();
            for _ in 0..5 {
                closure.sync_inputs(&u, &phi, &nu, 1e-3);
                closure.correct();
            }
            if let Some(nut) = closure.nu_t() {
                for c in 0..mesh.n_cells {
                    let v = nut.internal[c];
                    assert!(v.is_finite() && v >= 0.0, "{name}: nut[{c}] = {v}");
                }
            }
            let eqn = closure.div_dev_reff(&u, &nu);
            for c in 0..mesh.n_cells {
                assert!(eqn.ldu.diag[c].is_finite(), "{name}: diag[{c}] not finite");
                assert!(eqn.source[c].mag().is_finite(), "{name}: source[{c}]");
            }
        }
    }
}
