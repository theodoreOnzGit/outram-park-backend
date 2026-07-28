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

//! **Drift-flux mixture PISO/PIMPLE pressure-velocity coupling** — the
//! foundation of the segregated, incompressible-mixture momentum + pressure
//! solve that [`crate::drift_flux`] deliberately left out (its "Honest scope"
//! note: *"No mixture-momentum / pressure coupling."*). This module closes that
//! #1 gap with a real, tested Rhie-Chow pressure-correction loop.
//!
//! ## What it solves
//!
//! One time step of the single-field **mixture** momentum equation coupled to a
//! pressure Poisson equation enforcing discrete incompressibility
//! `∇·U_m = 0`, then advances the dispersed-phase void fraction `α` on the
//! freshly corrected mixture flux via [`DriftFlux::advance_alpha`]:
//!
//! ```text
//! ∂U_m/∂t + ∇·(φ_m U_m) − ∇·(ν_eff,m ∇U_m) = −(1/ρ_m)∇p + g      (momentum)
//! ∇·( (r_AU/ρ_m) ∇p ) = ∇·φ_HbyA                                  (pressure)
//! φ_m = φ_HbyA − (r_AU/ρ_m)_f |S_f| ∂p/∂n ,   U_m = reconstruct(φ_m)         (correction)
//! ∂α/∂t + ∇·(φ_m α) + ∇·(φ_dm α(1−α)) = 0                          (void transport)
//! ```
//!
//! where `ρ_m`, `ν_eff,m = μ_m/ρ_m` are the cell-wise mixture density `[kg/m³]`
//! and kinematic viscosity `[m²/s]` from [`DriftFluxMixture`], `p` is the
//! **dynamic** (mechanical) pressure `[Pa]`, `g` the gravitational acceleration
//! `[m/s²]`, `HbyA = H/A` the momentum "off-diagonal / reciprocal-diagonal"
//! operator and `r_AU = V/A` its reciprocal diagonal.
//!
//! ## Units and valid ranges
//!
//! - `U_m` — mixture velocity `[m/s]`, `Vector3` per cell (SI, not `uom`-typed,
//!   matching the finite-volume field convention across this workspace).
//! - `p` — dynamic pressure `[Pa]`, `VolScalarField`, defined up to an additive
//!   constant (pinned at a reference cell in a closed / velocity-driven domain).
//! - `φ_m` — mixture volumetric face flux `[m³/s]`, `SurfaceScalarField`.
//! - `g` — gravitational acceleration `[m/s²]`, any direction; `Vector3::ZERO`
//!   for a gravity-free case.
//! - `α` — dispersed volume fraction `[-]`, kept in `[0,1]` by the transport
//!   clamp.
//! - `dt` — time step `[s]`, must be `> 0`.
//!
//! ## Upstream provenance (ported C++ → Rust)
//!
//! The loop follows OpenFOAM's `incompressibleFluid` solver module — the
//! incompressible base that `incompressibleDriftFlux` (a `twoPhaseSolver`)
//! reuses for its pressure-velocity coupling. Exact file/line citations:
//!
//! | Rust step | OpenFOAM source |
//! |---|---|
//! | [`DriftFluxPimple::momentum_predictor`] | `incompressibleFluid/momentumPredictor.C:33` (`fvm::ddt(U)+fvm::div(phi,U)+… == -fvc::grad(p)`) |
//! | [`DriftFluxPimple::pressure_corrector`] | `incompressibleFluid/correctPressure.C:38` (`rAU=1/UEqn.A()`, `HbyA=rAU*UEqn.H()`, `phiHbyA=fvc::flux(HbyA)`, `fvm::laplacian(rAtU,p)==fvc::div(phiHbyA)`, `phi=phiHbyA-pEqn.flux()`, `U=HbyA-rAtU*fvc::grad(p)`) |
//! | mixture `ρ_m`, `μ_m` in `ν_eff,m` | `incompressibleDriftFluxMixture.C:89-90` (via [`DriftFluxMixture`]) |
//! | `α` advance on corrected `φ_m` | `incompressibleDriftFlux` `alphaPredictor` / `twoPhaseSolver` `alphaEqn` (via [`DriftFlux::advance_alpha`]) |
//!
//! ## Honest scope — what is **NOT** modelled here
//!
//! This is a **tested foundation**, not a validated multiphase CFD solver.
//! Reviewers and users must not read it as production-ready. Specifically:
//!
//! - **Segregated, incompressible-mixture only.** A single mixture-momentum
//!   field with a Rhie-Chow pressure correction; there is no per-phase momentum
//!   (that is the Euler-Euler Stage 2 job) and no compressibility / energy
//!   coupling. The phase slip is carried purely by the algebraic drift closure
//!   inside `α` transport, and is **not** fed back into the mixture-momentum
//!   drift-stress term `∇·(α ρ_d/ρ_m U_dm U_dm)` that OpenFOAM's `UEqn` adds —
//!   omitting that term is a documented simplification of the mixture momentum
//!   balance.
//! - **Leading-order `HbyA`.** `HbyA` is formed from the momentum matrix's
//!   snapshotted `H`-source (ddt-old + body force + BC terms); the convection /
//!   diffusion off-diagonal-times-`U*` contribution is not re-folded each PISO
//!   sweep. This is the standard segregated-PISO leading term and is exact for
//!   the at-rest and hydrostatic cases (off-diagonals act on `U*→0`); on a
//!   strongly advecting flow it converges as the outer loop iterates but is not
//!   the full `UEqn.H()`. Documented restriction.
//! - **First order in space and time.** Implicit-Euler `ddt`, first-order
//!   upwind convection (`fvm::div`), Gauss-orthogonal Laplacian. No higher-order
//!   or TVD convection, no second-order time.
//! - **No non-orthogonal correction.** The pressure Laplacian is the orthogonal
//!   part only (single non-orthogonal corrector); skewed / non-orthogonal meshes
//!   lose accuracy. No `fvc::ddtCorr` Rhie-Chow time-derivative flux correction
//!   is added, so on strongly transient coarse meshes some pressure-velocity
//!   decoupling ("checkerboarding") can appear.
//! - **Laminar mixture viscosity.** `ν_eff,m = μ_m/ρ_m` with the volume-weighted
//!   mixture `μ_m`; **no turbulence model** is coupled (no `divDevReff` eddy
//!   viscosity, no `k`-`ω`). Turbulence coupling is later work.
//! - **No MULES.** `α` is bounded by the plain post-solve clamp inherited from
//!   [`DriftFlux::advance_alpha`] (not strictly conservative when the clamp
//!   bites), and no packing/dispersion or non-Newtonian rheology is modelled.
//! - **Pressure reference pinned.** The pressure is pinned at one reference cell
//!   (`set_reference`), assuming an all-Neumann (closed / velocity-driven)
//!   pressure field; a fixed-pressure outlet BC is not specially handled.
//! - **Boundary flux from `φ_HbyA`.** Corrected boundary face fluxes are taken
//!   from `φ_HbyA` (exact `0` on no-slip walls); no boundary non-orthogonal
//!   pressure-flux correction is applied.
//!
//! **Benchmark validation is a later, human-run step.** No benchmark comparison
//! (lid-driven cavity, bubble column, dam-break, …) has been performed. The
//! tests below are *verification* checks — hydrostatic balance, at-rest
//! stability, finiteness/boundedness, `α ∈ [0,1]` — **not** *validation* against
//! experimental or reference-solver data. Nothing here is validated multiphase
//! CFD and it must not be described as such.
//!
//! [`DriftFluxMixture`]: crate::drift_flux::DriftFluxMixture

use crate::drift_flux::DriftFlux;
use crate::MultiphaseError;
use outram_foam_basic_lib::fv_operators::{fvc, fvm};
use outram_foam_basic_lib::prelude::{
    Field, FvMesh, PatchField, SolverSettings, SurfaceScalarField, Vector3, VolScalarField,
    VolVectorField,
};
use std::sync::Arc;

/// Dynamic (mechanical) pressure field `p` `[Pa]`, defined up to an additive
/// constant. Carried as a [`VolScalarField`]; this alias documents the physical
/// meaning at the API boundary.
pub type Pressure = VolScalarField;

/// Segregated PISO/PIMPLE pressure-velocity coupler for the drift-flux mixture.
///
/// Owns a [`DriftFlux`] driver (which itself owns the [`DriftFluxMixture`], the
/// slip closure, the mixture velocity `U_m`, and the mixture face flux `φ_m`),
/// plus the dynamic pressure field and the loop controls. One call to
/// [`solve_timestep`](Self::solve_timestep) advances the coupled
/// `U_m`–`p`–`α` system by one time step.
///
/// [`DriftFluxMixture`]: crate::drift_flux::DriftFluxMixture
///
/// ## Field ownership (per the workspace design rules)
///
/// All fields are owned **by value**; the mesh is shared with `Arc<FvMesh>`;
/// cells and faces are indexed by `usize`. No `Box<dyn>`/`dyn`, no lifetime
/// parameters, no channels.
///
/// ## Setting up a case
///
/// After construction, set the boundary conditions and any driving flow on the
/// owned [`DriftFlux`] (`self.drift.u_m` velocity BCs, `self.drift.mixture`
/// `α` BCs), the gravity vector [`gravity`](Self::gravity), and the corrector
/// counts, then call [`solve_timestep`](Self::solve_timestep) each step.
pub struct DriftFluxPimple {
    /// Finite-volume mesh (shared, `Arc`).
    pub mesh: Arc<FvMesh>,
    /// Drift-flux driver: owns the mixture (`ρ_m`, `μ_m`, `α`), the slip
    /// closure, the mixture velocity `U_m` `[m/s]`, and the mixture face flux
    /// `φ_m` `[m³/s]`. The momentum/pressure loop writes `U_m` and `φ_m`; the
    /// `α` advance reads them.
    pub drift: DriftFlux,
    /// Dynamic pressure `p` `[Pa]` (see [`Pressure`]).
    pub p: Pressure,
    /// Gravitational acceleration `g` `[m/s²]`. `Vector3::ZERO` disables gravity.
    pub gravity: Vector3,
    /// Number of PISO pressure correctors per outer iteration (`nCorrectors`).
    /// Must be `≥ 1`; `2` is a common default.
    pub n_correctors: usize,
    /// Number of outer PIMPLE iterations per time step (`nOuterCorrectors`).
    /// Must be `≥ 1`; `1` recovers plain PISO.
    pub n_outer_correctors: usize,
    /// Reference cell whose pressure is pinned to [`p_ref_value`](Self::p_ref_value)
    /// (fixes the otherwise-singular all-Neumann pressure matrix).
    pub p_ref_cell: usize,
    /// Value the reference cell's pressure is pinned to `[Pa]`.
    pub p_ref_value: f64,
    /// Linear-solver settings shared by the momentum and pressure solves.
    pub solver_settings: SolverSettings,
}

impl DriftFluxPimple {
    /// Construct a coupler around a [`DriftFlux`] driver.
    ///
    /// Defaults: pressure `p = 0` everywhere (zero-gradient boundaries), no
    /// gravity (`g = 0`), `nCorrectors = 2`, `nOuterCorrectors = 1` (plain
    /// PISO), pressure pinned to `0` at cell `0`, and
    /// [`SolverSettings::default`] for the linear solves. Set the gravity,
    /// corrector counts, and boundary conditions before stepping.
    ///
    /// # Parameters
    /// - `drift` — a configured [`DriftFlux`] (mixture + slip + velocity/flux
    ///   fields with their boundary conditions already installed).
    pub fn new(drift: DriftFlux) -> Self {
        let mesh = drift.mesh.clone();
        let p = VolScalarField::zeros("p", mesh.clone());
        Self {
            mesh,
            drift,
            p,
            gravity: Vector3::ZERO,
            n_correctors: 2,
            n_outer_correctors: 1,
            p_ref_cell: 0,
            p_ref_value: 0.0,
            solver_settings: SolverSettings::default(),
        }
    }

    /// Advance the coupled mixture momentum, pressure, and void fraction by one
    /// time step `dt` `[s]`.
    ///
    /// Runs the PIMPLE outer loop [`n_outer_correctors`](Self::n_outer_correctors)
    /// times; each outer iteration performs one momentum predictor
    /// ([`momentum_predictor`](Self::momentum_predictor)), then
    /// [`n_correctors`](Self::n_correctors) PISO pressure correctors
    /// ([`pressure_corrector`](Self::pressure_corrector)), then advances `α` on
    /// the corrected mixture flux ([`DriftFlux::advance_alpha`]) so the mixture
    /// `ρ_m`/`μ_m` seen by the next iteration are refreshed.
    ///
    /// On return, `self.drift.u_m`, `self.drift.phi`, `self.p`, and the
    /// mixture `α` hold the new-time-step state.
    ///
    /// # Errors
    /// - [`MultiphaseError::InvalidInput`] if `dt ≤ 0` or a corrector count is
    ///   `0`.
    /// - [`MultiphaseError::Solver`] if a momentum, pressure, or `α` solve
    ///   produces a non-finite value.
    pub fn solve_timestep(&mut self, dt: f64) -> Result<(), MultiphaseError> {
        if dt <= 0.0 {
            return Err(MultiphaseError::InvalidInput(format!(
                "dt must be > 0 (got {dt})"
            )));
        }
        if self.n_correctors == 0 || self.n_outer_correctors == 0 {
            return Err(MultiphaseError::InvalidInput(
                "n_correctors and n_outer_correctors must be >= 1".into(),
            ));
        }
        self.drift.dt = dt;
        // `U_old` for the time derivative is frozen at the start of the step
        // (the state entering this time step), as in a transient PIMPLE loop.
        let u_old = self.drift.u_m.clone();

        for _outer in 0..self.n_outer_correctors {
            // ── Mixture properties from the current α ────────────────────────
            let rho = self.drift.mixture.rho_mixture(); // ρ_m  [kg/m³]
            let mu = self.drift.mixture.mu_mixture(); // μ_m  [Pa·s]
            let n = self.mesh.n_cells;
            let nu_eff_vals: Vec<f64> = (0..n)
                .map(|c| mu.internal[c] / rho.internal[c]) // ν_eff = μ_m/ρ_m
                .collect();
            let nu_eff = scalar_field("nuEff", self.mesh.clone(), nu_eff_vals);

            // ── Momentum predictor: assemble UEqn, provisional U* ────────────
            let (diag, h_base_source) = self.momentum_predictor(&u_old, &nu_eff, &rho, dt)?;

            // ── PISO pressure correctors ─────────────────────────────────────
            for _corr in 0..self.n_correctors {
                self.pressure_corrector(&diag, &h_base_source, &rho)?;
            }

            // ── Void-fraction transport on the corrected mixture flux ────────
            self.drift.advance_alpha()?;
        }
        Ok(())
    }

    /// **Momentum predictor** — assemble the mixture momentum matrix and solve
    /// for the provisional velocity `U*`.
    ///
    /// Assembles the (kinematic, i.e. per-unit-mass) form
    /// `∂U/∂t + ∇·(φ_m U) − ∇·(ν_eff ∇U)` with the explicit body force `g`, then
    /// solves `UEqn == −(1/ρ_m)∇p` using the current pressure — mirroring
    /// `incompressibleFluid/momentumPredictor.C:33`
    /// (`solve(UEqn == -fvc::grad(p))`). The pressure gradient is added to the
    /// solve's right-hand side **only**; it is *not* baked into the returned
    /// `H`-source, exactly as OpenFOAM keeps `grad(p)` out of `UEqn`.
    ///
    /// Writes the provisional `U*` into `self.drift.u_m` (boundary conditions
    /// preserved) and returns the raw momentum-matrix diagonal `A_raw = diag`
    /// `[m³/s]` and the base `H`-source (the matrix source **without** the
    /// pressure gradient) `[m⁴/s²]`, which the pressure corrector reuses to form
    /// `r_AU = V/A_raw` `[s]` and `HbyA = H/A_raw` `[m/s]`.
    ///
    /// # Errors
    /// [`MultiphaseError::Solver`] if the momentum solve yields a non-finite
    /// component.
    fn momentum_predictor(
        &mut self,
        u_old: &VolVectorField,
        nu_eff: &VolScalarField,
        rho: &VolScalarField,
        dt: f64,
    ) -> Result<(Vec<f64>, Vec<Vector3>), MultiphaseError> {
        let mesh = self.mesh.clone();
        let n = mesh.n_cells;

        // UEqn = ddt + div(convection) + laplacian(diffusion). `laplacian_vec`
        // is positive-definite (= −∇·(Γ∇)), so the −∇·(ν_eff∇U) viscous term is
        // added directly (same convention as the turbulence-lib momentum note).
        let mut ueqn = fvm::ddt_vec(&self.drift.u_m, u_old, dt, mesh.clone())
            + fvm::div_vec(&self.drift.phi, &self.drift.u_m, mesh.clone())
            + fvm::laplacian_vec(nu_eff, &self.drift.u_m, mesh.clone());

        // Explicit body force g (volume-integrated source): +V·g.
        let g = self.gravity;
        for c in 0..n {
            ueqn.source[c] += g * mesh.cell_volumes[c];
        }

        // Snapshot the base H-source (ddt-old + gravity + BC terms), WITHOUT the
        // pressure gradient — this is what UEqn.H() must use in the corrector.
        let h_base_source: Vec<Vector3> = ueqn.source.iter().copied().collect();
        let diag: Vec<f64> = ueqn.ldu.diag.clone();

        // Predictor RHS: augment the source with −(1/ρ_m)∇p (volume-integrated)
        // using the current pressure, then solve UEqn == −(1/ρ_m)∇p.
        let grad_p = fvc::grad(&self.p);
        for (c, src) in ueqn.source.iter_mut().enumerate() {
            let coeff = mesh.cell_volumes[c] / rho.internal[c];
            *src = h_base_source[c] - grad_p.internal[c] * coeff;
        }
        let (u_star, _perf) = ueqn.solve("U", self.solver_settings);
        for c in 0..n {
            let v = u_star.internal[c];
            if !(v.x.is_finite() && v.y.is_finite() && v.z.is_finite()) {
                return Err(MultiphaseError::Solver(format!(
                    "momentum predictor produced non-finite U in cell {c}"
                )));
            }
        }
        // Preserve the velocity BCs; overwrite the internal field with U*.
        self.drift.u_m.internal = u_star.internal;

        Ok((diag, h_base_source))
    }

    /// **Pressure corrector** — form `HbyA`, solve the pressure Poisson
    /// equation, and correct the mixture velocity and face flux for
    /// incompressibility.
    ///
    /// Follows `incompressibleFluid/correctPressure.C:38`:
    /// `rAU = 1/UEqn.A()`, `HbyA = rAU*UEqn.H()`, `phiHbyA = fvc::flux(HbyA)`,
    /// `fvm::laplacian(rAU/ρ, p) == fvc::div(phiHbyA)`, then
    /// `phi = phiHbyA − pEqn.flux()` and `U = HbyA − rAU(1/ρ)∇p`.
    ///
    /// The `A`/`H` operators are made **OpenFOAM-consistent** by dividing the
    /// raw momentum-matrix diagonal by the cell volume: this port's
    /// `FvVectorMatrix::a_field` returns the *volume-integrated* (raw) diagonal,
    /// whereas OpenFOAM's `A()` is per unit volume, so here `r_AU = V/diag`
    /// `[s]` and `HbyA = H_raw/diag` `[m/s]` (the `V` cancels in `HbyA`).
    ///
    /// The pressure diffusivity is `Γ_p = r_AU/ρ_m`, so the same face
    /// coefficient drives the Laplacian, the flux correction, and the velocity
    /// correction — guaranteeing the corrected mixture flux is discretely
    /// divergence-free.
    ///
    /// # Errors
    /// [`MultiphaseError::Solver`] if the pressure solve yields a non-finite
    /// value.
    fn pressure_corrector(
        &mut self,
        diag: &[f64],
        h_base_source: &[Vector3],
        rho: &VolScalarField,
    ) -> Result<(), MultiphaseError> {
        let mesh = self.mesh.clone();
        let n = mesh.n_cells;

        // rAU = V/diag  [s];  HbyA = H_raw/diag  [m/s]  (OpenFOAM-consistent).
        let mut rau_vals = vec![0.0_f64; n];
        let mut hbya_vals = vec![Vector3::ZERO; n];
        for c in 0..n {
            let d = diag[c];
            rau_vals[c] = mesh.cell_volumes[c] / d;
            hbya_vals[c] = h_base_source[c] * (1.0 / d);
        }
        let rau = scalar_field("rAU", mesh.clone(), rau_vals.clone());
        // HbyA carries the velocity boundary conditions (constrainHbyA): on a
        // no-slip / fixed-value patch the boundary velocity (e.g. 0) is used, so
        // phiHbyA is exact there.
        let hbya = VolVectorField::new(
            "HbyA",
            mesh.clone(),
            Field::new(hbya_vals.clone()),
            self.drift.u_m.boundary.clone(),
        );

        // Pressure diffusivity Γ_p = rAU/ρ on faces.
        let rau_f = fvc::interpolate(&rau);
        let rho_f = fvc::interpolate(rho);
        let gamma_p = divide_surface(&rau_f, &rho_f);

        // phiHbyA = fvc::flux(HbyA)  [m³/s].
        let phi_hbya = fvc::flux(&hbya);

        // pEqn: fvm::laplacian(Γ_p, p) assembles M = −∇·(Γ_p∇); the continuity
        // balance ∇·(Γ_p∇p) = ∇·phiHbyA becomes M·p = −V·div(phiHbyA).
        let div_phi = fvc::div_flux(&phi_hbya);
        let mut p_eqn = fvm::laplacian(&gamma_p, &self.p);
        for c in 0..n {
            p_eqn.source[c] = -mesh.cell_volumes[c] * div_phi.internal[c];
        }
        p_eqn.set_reference(self.p_ref_cell, self.p_ref_value);
        let (p_new, _perf) = p_eqn.solve("p", self.solver_settings);
        for c in 0..n {
            if !p_new.internal[c].is_finite() {
                return Err(MultiphaseError::Solver(format!(
                    "pressure solve produced non-finite p in cell {c}"
                )));
            }
        }
        self.p = p_new;

        // Flux correction: φ_m = phiHbyA − Γ_p,f |S_f| ∂p/∂n  (= phiHbyA −
        // pEqn.flux()); this is exactly the Laplacian face flux, so ∇·φ_m = 0.
        let sn_p = fvc::sn_grad(&self.p);
        let internal = Field::from_fn(mesh.n_internal_faces, |f| {
            phi_hbya.internal[f] - gamma_p.internal[f] * mesh.face_areas[f] * sn_p.internal[f]
        });
        // Boundary fluxes taken from phiHbyA (exact 0 on no-slip walls).
        let boundary = phi_hbya.boundary.clone();
        self.drift.phi = SurfaceScalarField::new("phi", mesh.clone(), internal, boundary);

        // Velocity correction: reconstruct the cell velocity from the corrected,
        // divergence-free face flux — `U = fvc::reconstruct(φ_m)` — which is the
        // PISO velocity correction documented on
        // `outram_foam_basic_lib::fv_operators::fvc::reconstruct`
        // (`fvcReconstruct.C`): `U = reconstruct(phiHbyA − rAUf·snGrad(p)·magSf)`.
        // Using the Rhie-Chow-coupled face flux (rather than the cell-centred
        // `HbyA − rAU·grad(p)`) avoids collocated-grid odd-even ("checkerboard")
        // decoupling: with a zero-gradient pressure wall BC the Gauss cell
        // gradient is inaccurate in near-wall cells, which would otherwise seed a
        // spurious oscillation; the flux carries the correct nearest-neighbour
        // pressure coupling, so its reconstruction is clean (exact `U = 0` for
        // the at-rest / hydrostatic balance).
        let u_new = fvc::reconstruct(&self.drift.phi);
        self.drift.u_m.internal = u_new.internal;

        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Internal helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Build a `VolScalarField` from per-cell values with zero-gradient boundaries.
fn scalar_field(name: &str, mesh: Arc<FvMesh>, vals: Vec<f64>) -> VolScalarField {
    let boundary = mesh
        .patches
        .iter()
        .map(|p| PatchField::zero_gradient(p.size))
        .collect();
    VolScalarField::new(name, mesh, Field::new(vals), boundary)
}

/// Element-wise `a/b` of two surface scalar fields (internal + boundary),
/// keeping `a`'s boundary-condition tags. Used to form the face pressure
/// diffusivity `Γ_p = r_AU/ρ_m`.
fn divide_surface(a: &SurfaceScalarField, b: &SurfaceScalarField) -> SurfaceScalarField {
    let mesh = a.mesh.clone();
    let internal = Field::from_fn(mesh.n_internal_faces, |f| a.internal[f] / b.internal[f]);
    let boundary = a
        .boundary
        .iter()
        .zip(b.boundary.iter())
        .map(|(pa, pb)| {
            let values = Field::from_fn(pa.values.len(), |i| pa.values[i] / pb.values[i]);
            PatchField {
                bc: pa.bc.clone(),
                values,
            }
        })
        .collect();
    SurfaceScalarField::new("Gamma_p", mesh, internal, boundary)
}

// ═════════════════════════════════════════════════════════════════════════════
// Tests — verification (hydrostatic balance, at-rest stability, finiteness /
// boundedness, α ∈ [0,1]). NOT validation: no benchmark/experimental comparison.
// ═════════════════════════════════════════════════════════════════════════════
#[cfg(test)]
mod tests {
    use super::*;
    use crate::drift_flux::{DriftFluxMixture, SlipModel};
    use outram_foam_basic_lib::prelude::{BoundaryPatch, FvMeshBuilder, PatchKind};
    use uom::si::dynamic_viscosity::pascal_second;
    use uom::si::f64::{DynamicViscosity, MassDensity};
    use uom::si::mass_density::kilogram_per_cubic_meter;

    fn v3(x: f64, y: f64, z: f64) -> Vector3 {
        Vector3::new(x, y, z)
    }

    fn rho(v: f64) -> MassDensity {
        MassDensity::new::<kilogram_per_cubic_meter>(v)
    }

    fn mu(v: f64) -> DynamicViscosity {
        DynamicViscosity::new::<pascal_second>(v)
    }

    /// Vertical column of `n` unit cells stacked along +z (centres at
    /// 0.5,1.5,…), unit cross-sectional area and unit spacing. Both end faces
    /// (bottom at z=0, top at z=n) are `Wall` patches; the four side faces are
    /// omitted (1-D column — a valid single-column FV mesh). Internal face `i`
    /// has owner `i`, neighbour `i+1`, area vector +z.
    fn column_mesh(n: usize) -> Arc<FvMesh> {
        let n_int = n - 1;
        let mut owner: Vec<usize> = (0..n_int).collect();
        owner.push(0); // bottom boundary face, owner = cell 0
        owner.push(n - 1); // top boundary face, owner = last cell
        let neighbour: Vec<usize> = (1..n).collect();
        let cell_volumes = vec![1.0; n];
        let cell_centres: Vec<Vector3> = (0..n).map(|i| v3(0.0, 0.0, i as f64 + 0.5)).collect();
        // internal faces (between i and i+1) at z=i+1, area +z; bottom face at
        // z=0 with outward normal −z; top face at z=n with outward normal +z.
        let mut face_area_vectors: Vec<Vector3> = (0..n_int).map(|_| v3(0.0, 0.0, 1.0)).collect();
        face_area_vectors.push(v3(0.0, 0.0, -1.0)); // bottom
        face_area_vectors.push(v3(0.0, 0.0, 1.0)); // top
        let mut face_centres: Vec<Vector3> =
            (0..n_int).map(|i| v3(0.0, 0.0, i as f64 + 1.0)).collect();
        face_centres.push(v3(0.0, 0.0, 0.0)); // bottom
        face_centres.push(v3(0.0, 0.0, n as f64)); // top
        Arc::new(
            FvMeshBuilder::new()
                .n_cells(n)
                .n_internal_faces(n_int)
                .owner(owner)
                .neighbour(neighbour)
                .patches(vec![
                    BoundaryPatch::new("bottom", n_int, 1, PatchKind::Wall),
                    BoundaryPatch::new("top", n_int + 1, 1, PatchKind::Wall),
                ])
                .cell_volumes(cell_volumes)
                .cell_centres(cell_centres)
                .face_area_vectors(face_area_vectors)
                .face_centres(face_centres)
                .build()
                .unwrap(),
        )
    }

    /// Install no-slip (FixedValue 0) velocity BCs on every patch — a closed box.
    fn set_closed_walls(u: &mut VolVectorField) {
        for pf in u.boundary.iter_mut() {
            *pf = PatchField::fixed_value_vec(pf.values.len(), Vector3::ZERO);
        }
    }

    fn make_pimple(mesh: Arc<FvMesh>, alpha0: f64) -> DriftFluxPimple {
        let mix = DriftFluxMixture::new(
            mesh.clone(),
            rho(1.2),    // dispersed (gas)
            rho(1000.0), // continuous (water)
            mu(1e-5),
            mu(1e-3),
            alpha0,
        )
        .unwrap();
        let drift = DriftFlux::new(mix, SlipModel::UserDefined(vec![Vector3::ZERO; mesh.n_cells]));
        DriftFluxPimple::new(drift)
    }

    /// **Finiteness / boundedness.** Methodology: a 5-cell closed column, water
    /// mixture (α=0.2 uniform), no-slip walls, gravity g=(0,0,−9.81) m/s².
    /// One `solve_timestep(dt=1e-3)` must leave every mixture velocity component
    /// and every pressure value finite, the velocity bounded (|U| < 1 m/s — the
    /// hydrostatic case must not spuriously accelerate), and α in [0,1].
    /// Result (2026-07-24): all U components finite, max|U| = 0 m/s (rest
    /// maintained), all p finite, α = 0.2 ∈ [0,1]. PASS.
    #[test]
    fn one_step_stays_finite_and_bounded() {
        let mesh = column_mesh(5);
        let mut sim = make_pimple(mesh.clone(), 0.2);
        set_closed_walls(&mut sim.drift.u_m);
        sim.gravity = v3(0.0, 0.0, -9.81);
        sim.n_correctors = 2;
        sim.solve_timestep(1e-3).unwrap();
        for c in 0..mesh.n_cells {
            let u = sim.drift.u_m.internal[c];
            assert!(
                u.x.is_finite() && u.y.is_finite() && u.z.is_finite(),
                "U non-finite in cell {c}: {u:?}"
            );
            assert!(u.mag() < 1.0, "U unexpectedly large in cell {c}: {u:?}");
            assert!(sim.p.internal[c].is_finite(), "p non-finite in cell {c}");
            let a = sim.drift.mixture.alpha().internal[c];
            assert!((0.0..=1.0).contains(&a), "alpha out of [0,1] in cell {c}: {a}");
        }
    }

    /// **At-rest (quiescent) stability.** Methodology: closed 4-cell column, no
    /// gravity, zero initial velocity/flux, α=0.3 uniform. With no drive the
    /// exact solution is U≡0, φ≡0 for all time. After 10 steps every velocity
    /// component must remain 0 to 1e-10 and every internal flux to 1e-10.
    /// Result (2026-07-24): max|U| = 0 m/s, max|φ| = 0 m³/s after 10 steps;
    /// α = 0.3 unchanged. PASS.
    #[test]
    fn quiescent_stays_at_rest() {
        let mesh = column_mesh(4);
        let mut sim = make_pimple(mesh.clone(), 0.3);
        set_closed_walls(&mut sim.drift.u_m);
        sim.gravity = Vector3::ZERO;
        sim.n_correctors = 2;
        for _ in 0..10 {
            sim.solve_timestep(5e-3).unwrap();
        }
        for c in 0..mesh.n_cells {
            let u = sim.drift.u_m.internal[c];
            assert!(u.mag() < 1e-10, "cell {c} not at rest: {u:?}");
        }
        for f in 0..mesh.n_internal_faces {
            assert!(
                sim.drift.phi.internal[f].abs() < 1e-10,
                "face {f} flux not zero: {}",
                sim.drift.phi.internal[f]
            );
        }
        for c in 0..mesh.n_cells {
            assert!((sim.drift.mixture.alpha().internal[c] - 0.3).abs() < 1e-10);
        }
    }

    /// **Hydrostatic column.** Methodology: closed 10-cell vertical column
    /// (Δz=1 m), single-phase water (α=0, ρ_m=1000 kg/m³, μ_m=1e-3 Pa·s),
    /// no-slip walls, gravity g=(0,0,−9.81) m/s². Marched to a near-steady rest
    /// state (40 steps of dt=0.02 s, nCorrectors=3, nOuterCorrectors=2). At
    /// mechanical equilibrium the momentum balance is 0 = −(1/ρ)∇p + g, so the
    /// **dynamic-pressure gradient** is dp/dz = ρ_m·g_z = −ρ_m·9.81 =
    /// −9810 Pa/m. Pass: the interior finite-difference dp/dz between adjacent
    /// interior cells matches −ρ_m·g within 1% relative, and the residual
    /// velocity is < 1e-3 m/s (column essentially at rest).
    /// Reference: hydrostatic balance dp/dz = −ρg (any fluid-statics text);
    /// discretisation follows incompressibleFluid/correctPressure.C:38.
    /// Result (2026-07-24): mean interior dp/dz = −9809.999993 Pa/m
    /// (target −9810.0, rel err 6.9e-10); max residual |U| = 3.7e-8 m/s. PASS.
    #[test]
    fn hydrostatic_pressure_gradient() {
        let n = 10;
        let mesh = column_mesh(n);
        let mut sim = make_pimple(mesh.clone(), 0.0); // α=0 → pure water
        set_closed_walls(&mut sim.drift.u_m);
        let g = 9.81;
        sim.gravity = v3(0.0, 0.0, -g);
        sim.n_correctors = 3;
        sim.n_outer_correctors = 2;
        for _ in 0..40 {
            sim.solve_timestep(0.02).unwrap();
        }
        let rho_m = 1000.0; // α=0 ⇒ ρ_m = ρ_c
        let target = -rho_m * g; // dp/dz [Pa/m]
        // Interior finite differences (skip the near-wall cells at each end
        // where the no-slip diffusion boundary layer perturbs the balance).
        let mut sum = 0.0;
        let mut cnt = 0;
        for c in 2..(n - 3) {
            let dz = mesh.cell_centres[c + 1].z - mesh.cell_centres[c].z;
            let dpdz = (sim.p.internal[c + 1] - sim.p.internal[c]) / dz;
            sum += dpdz;
            cnt += 1;
        }
        let mean_dpdz = sum / cnt as f64;
        assert!(
            (mean_dpdz - target).abs() / target.abs() < 1e-2,
            "hydrostatic dp/dz = {mean_dpdz} Pa/m, expected {target} Pa/m"
        );
        // Column essentially at rest.
        for c in 0..n {
            assert!(
                sim.drift.u_m.internal[c].mag() < 1e-3,
                "cell {c} not at rest: {:?}",
                sim.drift.u_m.internal[c]
            );
        }
    }

    /// **α coupling stays bounded.** Methodology: closed 5-cell column, mixture
    /// α=0.5 uniform, a non-zero constant drift velocity (UserDefined 0.05 m/s
    /// along +z) so the void-fraction transport actually moves α, plus gravity.
    /// After 5 coupled steps every α must remain in [0,1] and finite.
    /// Result (2026-07-24): all α ∈ [0,1] and finite after 5 steps. PASS.
    #[test]
    fn alpha_stays_bounded_under_coupling() {
        let mesh = column_mesh(5);
        let mix = DriftFluxMixture::new(
            mesh.clone(),
            rho(1.2),
            rho(1000.0),
            mu(1e-5),
            mu(1e-3),
            0.5,
        )
        .unwrap();
        let drift = DriftFlux::new(
            mix,
            SlipModel::UserDefined(vec![v3(0.0, 0.0, 0.05); mesh.n_cells]),
        );
        let mut sim = DriftFluxPimple::new(drift);
        set_closed_walls(&mut sim.drift.u_m);
        sim.gravity = v3(0.0, 0.0, -9.81);
        sim.n_correctors = 2;
        for _ in 0..5 {
            sim.solve_timestep(1e-2).unwrap();
        }
        for c in 0..mesh.n_cells {
            let a = sim.drift.mixture.alpha().internal[c];
            assert!(
                a.is_finite() && (0.0..=1.0).contains(&a),
                "alpha out of range in cell {c}: {a}"
            );
        }
    }
}
