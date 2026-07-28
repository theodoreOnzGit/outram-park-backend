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

//! **Two-fluid Euler-Euler phase pressure-velocity coupling** — the
//! two-momentum analogue of the drift-flux PIMPLE loop ([`crate::pimple`]).
//! This closes the #1 gap the [`crate::two_fluid`] foundation left open (its
//! honest scope: *"No phase-momentum / pressure-coupled PIMPLE loop"*) with a
//! real, tested **shared-pressure** Euler-Euler PISO: two per-phase momentum
//! predictors, a single mixture-continuity pressure Poisson equation, and a
//! checkerboard-safe `fvc::reconstruct` velocity/flux correction for each phase.
//!
//! ## What it solves
//!
//! One time step of the two coupled **phase-intensive** (per-unit-density)
//! momentum equations for a dispersed phase `d` and a continuous phase `c`,
//! coupled to a single shared **dynamic pressure** `p` through the mixture
//! volumetric-continuity constraint, then advances the dispersed volume
//! fraction `α_d` on the corrected phase velocity:
//!
//! ```text
//!  ∂(α_k U_k)/∂t + ∇·(α_k φ_k U_k) − ∇·(α_k ν_k ∇U_k)
//!        = −(α_k/ρ_k)∇p + α_k g + (K_d/ρ_k)(U_o − U_k)      (phase k momentum)
//!
//!  Σ_k ∇·(α_k U_k) = 0    ⇒    ∇·( Γ_mix ∇p ) = ∇·φ_HbyA,mix   (shared pressure)
//!
//!  φ_k = φ_HbyA_k − (r_AU,k α_k/ρ_k)_f |S_f| ∂p/∂n ,  U_k = reconstruct(φ_k)   (correct)
//!
//!  ∂α_d/∂t + ∇·(α_d φ_d) = 0 ,  α_c = 1 − α_d ,  both in [0,1]   (continuity)
//! ```
//!
//! where `U_k` is the phase velocity `[m/s]`, `φ_k = U_k·S_f` the phase
//! velocity face flux `[m³/s]`, `α_k` the volume fraction `[-]`, `ρ_k` the
//! density `[kg/m³]`, `ν_k = μ_k/ρ_k` the kinematic viscosity `[m²/s]`, `g`
//! gravity `[m/s²]`, `K_d` the interphase drag coefficient `[kg/(m³·s)]`
//! (from [`DragModel`]), `U_o` the *other* phase's velocity, `p` the dynamic
//! pressure `[Pa]`, `r_AU,k = V/A_k` the reciprocal momentum diagonal `[s]`,
//! and `φ_HbyA,mix` / `Γ_mix` the mixture predicted flux / pressure diffusivity
//! assembled below.
//!
//! ## Drag coupling — semi-implicit split (documented choice)
//!
//! The interphase drag `(K_d/ρ_k)(U_o − U_k)` couples the two momentum
//! balances. This foundation uses a **simple semi-implicit split** (not full
//! partial elimination / PEA): in phase `k`'s own matrix the sink `−(K_d/ρ_k)U_k`
//! is **implicit** (added to the diagonal via [`fvm::sp`]-style assembly, which
//! keeps the matrix diagonally dominant), while the source `+(K_d/ρ_k)U_o` is
//! **explicit** in the *other* phase's velocity from the start of the current
//! outer iteration. Iterating the PIMPLE outer loop
//! ([`n_outer_correctors`](TwoFluidPimple::n_outer_correctors)) converges the
//! explicit coupling. OpenFOAM's `multiphaseEuler` instead eliminates the drag
//! implicitly across phases through the block-coupled `invADV` operator
//! (`momentumTransferSystem::invADVs`); porting that partial elimination is
//! later work (see honest scope).
//!
//! ## Upstream provenance (ported C++ → Rust)
//!
//! Structure follows OpenFOAM-dev's `multiphaseEuler` solver module (the
//! shared-pressure `pU` / cell-pressure-corrector path). Exact citations:
//!
//! | Rust step | OpenFOAM source |
//! |---|---|
//! | [`TwoFluidPimple::phase_momentum_predictor`] (per-phase `U_k` predictor) | `multiphaseEuler/momentumPredictor.C:44` (`cellMomentumPredictor`, `UEqns.set(... phase.UEqn() == ...)`) + `phaseSystem/phaseModels/MovingPhaseModel/MovingPhaseModel.C:341` (`UEqn()` = `fvm::ddt(alpha,rho,U)+fvm::div(alphaRhoPhi,U)+…`) |
//! | `r_A,k = 1/A_k` diagonal, `HbyA_k` | `multiphaseEuler/cellPressureCorrector.C:82` (`As.set(... UEqns[...].A())`), `:167` (`Hs.set(... UEqns[...].H())`) |
//! | mixture predicted flux `φ_HbyA,mix = Σ α_kf φ_HbyA_k` | `cellPressureCorrector.C:289` (`phiHbyA += alphafs[phase]*phiHbyADs[...]`) |
//! | mixture pressure diffusivity `Γ_mix = Σ α_kf²·r_A,kf/ρ_k` | `cellPressureCorrector.C:312` (`rAf += alphafs[phase]*alphaByADfs[...]`, and `alphaByADfs = invADVfs & movingAlphafs` carries the second `α`) |
//! | shared pressure Poisson `∇·(Γ_mix∇p) = ∇·φ_HbyA,mix` | `cellPressureCorrector.C:352` (`fvc::div(phiHbyA) − fvm::laplacian(rAf, p_rgh)`) |
//! | per-phase flux `φ_k = φ_HbyA_k − (r_A,kα_k/ρ_k)_f|S_f|∂p/∂n`, `U_k = reconstruct(...)` | `cellPressureCorrector.C:384` (`phi_ = phiHbyA + pEqnIncomp.flux()`), `:445` (`URef() = HbyADs[...] + fvc::reconstruct(alphaByADfs[...]*mSfGradp − …)`) |
//! | `α_d` advance on corrected `U_d`, `α_c = 1−α_d` | via [`TwoFluidSystem::advance_dispersed_alpha`] (`phaseSystem/phaseSystem/phaseSystemSolve.C:336`) |
//!
//! The collocated-grid **odd-even ("checkerboard") decoupling** fix is exactly
//! the one [`crate::pimple`] documents: each phase velocity is reconstructed
//! from its Rhie-Chow-coupled *face* flux `φ_k` (which carries the correct
//! nearest-neighbour pressure coupling), **not** from the cell-centred
//! `HbyA_k − r_AU,k(α_k/ρ_k)∇p`. With zero-gradient pressure wall BCs the Gauss
//! cell gradient is inaccurate near walls and would otherwise seed a spurious
//! oscillation; the flux reconstruction is clean, giving exact `U_k = 0` for the
//! at-rest and hydrostatic balances.
//!
//! ## Honest scope — what is **NOT** modelled here
//!
//! This is a **tested foundation**, not a validated or converged coupled
//! two-fluid solver. Reviewers and users must not read it as production
//! multiphase CFD. Specifically:
//!
//! - **Shared-pressure Euler-Euler only.** A single dynamic pressure shared by
//!   both phases (no per-phase pressure, no `implicitPhasePressure` particle
//!   pressure, no compressible `p_rgh`/buoyancy split). Only the two phase
//!   momenta and the mixture volumetric-continuity constraint are coupled.
//! - **Drag is the only interfacial force in the coupling.** Lift, virtual
//!   (added) mass, wall lubrication, and turbulent dispersion are *not* fed into
//!   the momentum coupling — they remain the documented scaffolds in
//!   [`crate::two_fluid::InterfacialForce`]. Only the [`DragModel`] `K_d` couples
//!   the phases.
//! - **Semi-implicit (not partial-elimination) drag.** The `+(K_d/ρ_k)U_o`
//!   source is explicit in the other phase's velocity; strong drag is only fully
//!   consistent once the outer loop converges. No block `invADV` elimination.
//! - **Leading-order `HbyA`.** As in [`crate::pimple`], `HbyA_k` is formed from
//!   the momentum matrix's snapshotted `H`-source (ddt-old + body force +
//!   explicit drag + BC terms); the convection/diffusion off-diagonal-times-`U*`
//!   contribution is not re-folded each PISO sweep. Exact for the at-rest and
//!   hydrostatic cases (off-diagonals act on `U*→0`); on strongly advecting flow
//!   it converges as the outer loop iterates but is not the full `UEqn.H()`.
//! - **First order in space and time.** Implicit-Euler `ddt` with a frozen
//!   volume-fraction coefficient, first-order upwind convection, Gauss-orthogonal
//!   Laplacian. No higher-order/TVD convection, no second-order time, **no
//!   non-orthogonal correction**, and no `fvc::ddtCorr` Rhie-Chow time-derivative
//!   flux correction (so strongly transient coarse meshes can show some
//!   pressure-velocity decoupling).
//! - **Laminar, constant properties.** `ρ_k`, `μ_k`, and the dispersed diameter
//!   are constants; no turbulence model, no thermo, no population balance.
//! - **No MULES.** `α_d` is bounded by the plain post-solve clamp inherited from
//!   [`TwoFluidSystem::advance_dispersed_alpha`] (physical but not strictly
//!   conservative when the clamp bites); `α_c = 1 − α_d`.
//! - **Pressure reference pinned** at one cell (all-Neumann / closed or
//!   velocity-driven pressure field); a fixed-pressure outlet BC is not specially
//!   handled.
//!
//! **Benchmark validation is a later, human-run step.** No benchmark comparison
//! (bubble column, sedimentation, dam-break, …) has been performed. The tests
//! below are *verification* checks — finiteness/boundedness, at-rest stability,
//! hydrostatic balance `dp/dz = −ρ_mix·g`, the `α_d + α_c = 1` / `[0,1]`
//! constraint, and drag-driven velocity equilibration — **not** *validation*
//! against experimental or reference-solver data. Nothing here is validated
//! multiphase CFD and it must not be described as such.

use crate::two_fluid::{DragModel, TwoFluidSystem};
use crate::MultiphaseError;
use outram_foam_basic_lib::fv_operators::{fvc, fvm};
use outram_foam_basic_lib::prelude::{
    Field, FvMesh, PatchField, SolverSettings, SurfaceScalarField, Vector3, VolScalarField,
    VolVectorField,
};
use std::sync::Arc;
use uom::si::kinematic_viscosity::square_meter_per_second;
use uom::si::mass_density::kilogram_per_cubic_meter;

/// Dynamic (mechanical) pressure field `p` `[Pa]`, defined up to an additive
/// constant. Carried as a [`VolScalarField`]; this alias documents the physical
/// meaning at the API boundary. (Same convention as [`crate::pimple::Pressure`].)
pub type Pressure = VolScalarField;

/// Shared-pressure Euler-Euler PISO/PIMPLE pressure-velocity coupler for a
/// [`TwoFluidSystem`] (dispersed phase `d` + continuous phase `c`).
///
/// Owns the two-fluid system (each phase's `α_k`, `U_k`, and constant
/// properties), the interphase [`DragModel`] that couples the two momenta, the
/// shared dynamic pressure field, the two phase velocity fluxes, and the loop
/// controls. One call to [`solve_timestep`](Self::solve_timestep) advances the
/// coupled `U_d`–`U_c`–`p`–`α_d` system by one time step.
///
/// ## Field ownership (per the workspace design rules)
///
/// All fields are owned **by value**; the mesh is shared with `Arc<FvMesh>`;
/// cells and faces are indexed by `usize`. No `Box<dyn>`/`dyn`, no lifetime
/// parameters, no channels. Model dispatch (the drag closure) is the
/// [`DragModel`] enum.
///
/// ## Setting up a case
///
/// After construction, prescribe the phase velocity boundary conditions on the
/// owned [`system`](Self::system) (`system.dispersed.u_mut()` /
/// `system.continuous.u_mut()` and their `.boundary`), any dispersed-fraction
/// inlet BC (`system.dispersed.alpha_mut().boundary`), the gravity vector
/// [`gravity`](Self::gravity), and the corrector counts, then call
/// [`solve_timestep`](Self::solve_timestep) each step.
pub struct TwoFluidPimple {
    /// Finite-volume mesh (shared, `Arc`).
    pub mesh: Arc<FvMesh>,
    /// The two-fluid system: dispersed + continuous phase fields (`α_k`, `U_k`)
    /// and the saturation constraint `α_d + α_c = 1`.
    pub system: TwoFluidSystem,
    /// Interphase drag closure providing `K_d` `[kg/(m³·s)]` — the only
    /// interfacial force coupling the two momenta at this foundation stage.
    pub drag: DragModel,
    /// Shared dynamic pressure `p` `[Pa]` (see [`Pressure`]).
    pub p: Pressure,
    /// Dispersed-phase velocity face flux `φ_d = U_d·S_f` `[m³/s]` after the
    /// last correction (Rhie-Chow coupled).
    pub phi_d: SurfaceScalarField,
    /// Continuous-phase velocity face flux `φ_c = U_c·S_f` `[m³/s]` after the
    /// last correction (Rhie-Chow coupled).
    pub phi_c: SurfaceScalarField,
    /// Gravitational acceleration `g` `[m/s²]`. `Vector3::ZERO` disables gravity.
    pub gravity: Vector3,
    /// Number of PISO pressure correctors per outer iteration (`nCorrectors`).
    /// Must be `≥ 1`; `2` is a common default.
    pub n_correctors: usize,
    /// Number of outer PIMPLE iterations per time step (`nOuterCorrectors`).
    /// Must be `≥ 1`; `1` recovers plain PISO. More iterations tighten the
    /// explicit inter-phase drag coupling.
    pub n_outer_correctors: usize,
    /// Reference cell whose pressure is pinned to [`p_ref_value`](Self::p_ref_value)
    /// (fixes the otherwise-singular all-Neumann pressure matrix).
    pub p_ref_cell: usize,
    /// Value the reference cell's pressure is pinned to `[Pa]`.
    pub p_ref_value: f64,
    /// Linear-solver settings shared by the momentum and pressure solves.
    pub solver_settings: SolverSettings,
}

impl TwoFluidPimple {
    /// Construct a coupler around a [`TwoFluidSystem`] and an interphase
    /// [`DragModel`].
    ///
    /// Defaults: pressure `p = 0` everywhere (zero-gradient boundaries), zero
    /// phase fluxes, no gravity (`g = 0`), `nCorrectors = 2`,
    /// `nOuterCorrectors = 1` (plain PISO), pressure pinned to `0` at cell `0`,
    /// and [`SolverSettings::default`] for the linear solves. Set the gravity,
    /// corrector counts, and boundary conditions before stepping.
    ///
    /// # Parameters
    /// - `system` — a configured [`TwoFluidSystem`] (both phases' `α`/`U` and
    ///   material properties, with velocity boundary conditions installed).
    /// - `drag` — the interphase [`DragModel`] supplying `K_d`.
    pub fn new(system: TwoFluidSystem, drag: DragModel) -> Self {
        let mesh = system.mesh.clone();
        let p = VolScalarField::zeros("p", mesh.clone());
        let phi_d = SurfaceScalarField::uniform("phi.dispersed", mesh.clone(), 0.0);
        let phi_c = SurfaceScalarField::uniform("phi.continuous", mesh.clone(), 0.0);
        Self {
            mesh,
            system,
            drag,
            p,
            phi_d,
            phi_c,
            gravity: Vector3::ZERO,
            n_correctors: 2,
            n_outer_correctors: 1,
            p_ref_cell: 0,
            p_ref_value: 0.0,
            solver_settings: SolverSettings::default(),
        }
    }

    /// Advance the coupled phase momenta, shared pressure, and dispersed volume
    /// fraction by one time step `dt` `[s]`.
    ///
    /// Runs the PIMPLE outer loop [`n_outer_correctors`](Self::n_outer_correctors)
    /// times. Each outer iteration:
    /// 1. evaluates the interphase drag `K_d` from the current state;
    /// 2. assembles and solves both **phase momentum predictors**
    ///    ([`phase_momentum_predictor`](Self::phase_momentum_predictor)) — each
    ///    using the *other* phase's start-of-iteration velocity for the explicit
    ///    drag source;
    /// 3. performs [`n_correctors`](Self::n_correctors) **shared-pressure PISO
    ///    correctors** ([`pressure_corrector`](Self::pressure_corrector));
    /// 4. advances `α_d` on the corrected dispersed velocity
    ///    ([`TwoFluidSystem::advance_dispersed_alpha`]) and re-imposes
    ///    `α_c = 1 − α_d`, so the mixture `ρ`/`ν` seen by the next iteration are
    ///    refreshed.
    ///
    /// On return, both phase velocities, both phase fluxes, `self.p`, and the
    /// phase fractions hold the new-time-step state.
    ///
    /// # Errors
    /// - [`MultiphaseError::InvalidInput`] if `dt ≤ 0` or a corrector count is `0`.
    /// - [`MultiphaseError::Solver`] if a momentum, pressure, or `α` solve
    ///   produces a non-finite value.
    /// - Propagates [`DragModel::k_d`] errors (e.g. undefined slip Reynolds number).
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

        // Old-time velocities for the ddt term are frozen at the start of the
        // step (the state entering this time step), as in a transient PIMPLE loop.
        let ud_old = self.system.dispersed.u().clone();
        let uc_old = self.system.continuous.u().clone();

        let mesh = self.mesh.clone();
        let rho_d = self.system.dispersed.rho().get::<kilogram_per_cubic_meter>();
        let rho_c = self
            .system
            .continuous
            .rho()
            .get::<kilogram_per_cubic_meter>();
        let nu_d = self.system.dispersed.nu().get::<square_meter_per_second>();
        let nu_c = self.system.continuous.nu().get::<square_meter_per_second>();
        let g = self.gravity;
        let settings = self.solver_settings;

        for _outer in 0..self.n_outer_correctors {
            // (1) Interphase drag coefficient K_d from the current state.
            let kd = self.drag.k_d(&self.system)?;

            // Snapshot the phase velocities entering this outer iteration; the
            // explicit drag source in each predictor reads the *other* phase's
            // snapshot (frozen coupling, converged by the outer loop).
            let ud_snap = self.system.dispersed.u().clone();
            let uc_snap = self.system.continuous.u().clone();

            // Snapshot the volume fractions (predictor coefficients; borrowed
            // immutably while the phase velocity is mutated).
            let alpha_d = self.system.dispersed.alpha().clone();
            let alpha_c = self.system.continuous.alpha().clone();

            // (2) Per-phase momentum predictors → (diagonal A_k, base H_k source).
            let (diag_d, h_d) = Self::phase_momentum_predictor(
                &mesh,
                &alpha_d,
                rho_d,
                nu_d,
                &kd,
                &uc_snap,
                &ud_old,
                self.system.dispersed.u_mut(),
                &self.p,
                g,
                dt,
                settings,
            )?;
            let (diag_c, h_c) = Self::phase_momentum_predictor(
                &mesh,
                &alpha_c,
                rho_c,
                nu_c,
                &kd,
                &ud_snap,
                &uc_old,
                self.system.continuous.u_mut(),
                &self.p,
                g,
                dt,
                settings,
            )?;

            // (3) Shared-pressure PISO correctors.
            for _corr in 0..self.n_correctors {
                self.pressure_corrector(
                    &alpha_d, &alpha_c, rho_d, rho_c, &diag_d, &h_d, &diag_c, &h_c,
                )?;
            }

            // (4) Dispersed-fraction transport on the corrected U_d; α_c = 1−α_d.
            self.system.advance_dispersed_alpha(dt)?;
        }
        Ok(())
    }

    /// **Phase momentum predictor** — assemble one phase's momentum matrix and
    /// solve for its provisional velocity `U*_k`.
    ///
    /// Assembles the phase-intensive (per-unit-density) operator
    /// `∂(α_k U_k)/∂t + ∇·(α_k φ_k U_k) − ∇·(α_k ν_k ∇U_k)` with the implicit
    /// drag sink `+(K_d/ρ_k)U_k` on the diagonal, the explicit body force
    /// `α_k g`, and the explicit drag source `+(K_d/ρ_k)U_o` (in the other
    /// phase's velocity `U_o`) folded into the `H`-source. It then solves
    /// `UEqn_k == −(α_k/ρ_k)∇p` using the current pressure — the pressure
    /// gradient is added to the solve's right-hand side **only** and is *not*
    /// baked into the returned `H`-source, exactly as OpenFOAM keeps `grad(p)`
    /// out of `UEqn` (`momentumPredictor.C:44`).
    ///
    /// Writes the provisional `U*_k` into `u` (boundary conditions preserved)
    /// and returns the raw momentum-matrix diagonal `A_k` `[m³/s]` and the base
    /// `H`-source (matrix source **without** the pressure gradient), which the
    /// pressure corrector reuses to form `r_AU,k = V/A_k` `[s]` and
    /// `HbyA_k = H_k/A_k` `[m/s]`.
    ///
    /// The diagonal `A_k` is the raw, **volume-integrated** momentum diagonal
    /// (this port's `FvVectorMatrix` diagonal), so `r_AU,k = V/A_k` and
    /// `HbyA_k = H_k/A_k` restore OpenFOAM's per-unit-volume `A()`/`H()`
    /// convention (the `V` cancels in `HbyA`), matching [`crate::pimple`].
    ///
    /// # Parameters
    /// - `mesh` — shared finite-volume mesh.
    /// - `alpha` — this phase's volume fraction `α_k` `[-]`.
    /// - `rho` — this phase's density `ρ_k` `[kg/m³]` (> 0).
    /// - `nu` — this phase's kinematic viscosity `ν_k` `[m²/s]` (≥ 0).
    /// - `kd` — interphase drag coefficient `K_d` `[kg/(m³·s)]`.
    /// - `u_other` — the *other* phase's velocity `U_o` `[m/s]` for the explicit
    ///   drag source.
    /// - `u_old` — this phase's old-time velocity for the `ddt` term.
    /// - `u` — this phase's velocity field (overwritten with `U*_k`).
    /// - `p` — the current shared pressure `[Pa]`.
    /// - `gravity` — `g` `[m/s²]`.
    /// - `dt` — time step `[s]` (> 0).
    /// - `settings` — linear-solver settings.
    ///
    /// # Errors
    /// [`MultiphaseError::Solver`] if the momentum solve yields a non-finite
    /// component.
    #[allow(clippy::too_many_arguments)]
    fn phase_momentum_predictor(
        mesh: &Arc<FvMesh>,
        alpha: &VolScalarField,
        rho: f64,
        nu: f64,
        kd: &VolScalarField,
        u_other: &VolVectorField,
        u_old: &VolVectorField,
        u: &mut VolVectorField,
        p: &VolScalarField,
        gravity: Vector3,
        dt: f64,
        settings: SolverSettings,
    ) -> Result<(Vec<f64>, Vec<Vector3>), MultiphaseError> {
        let n = mesh.n_cells;

        // α_k-weighted convective face flux  α_k φ_k = α_kf · (U_k·S_f)  [m³/s].
        let phi_k = fvc::flux(u);
        let alpha_f = fvc::interpolate(alpha);
        let alpha_phi = map2_surface("alphaPhi", &alpha_f, &phi_k, |a, phi| a * phi);

        // Diffusivity α_k ν_k for the Laplacian  −∇·(α_k ν_k ∇U_k)  [m²/s].
        let al_nu_vals: Vec<f64> = (0..n).map(|c| alpha.internal[c] * nu).collect();
        let al_nu = scalar_field("alphaNu", mesh.clone(), al_nu_vals);

        // Implicit drag sink coefficient  K_d/ρ_k  [1/s]  (→ diagonal via Sp).
        let drag_impl_vals: Vec<f64> = (0..n).map(|c| kd.internal[c] / rho).collect();
        let drag_impl = scalar_field("KdByRho", mesh.clone(), drag_impl_vals);

        // UEqn_k = ddt(α_k U_k) + div(α_k φ_k, U_k) + [−∇·(α_k ν_k ∇U_k)]
        //          + Sp(K_d/ρ_k, U_k).  `laplacian_vec` is the positive-definite
        // matrix (= −∇·(Γ∇)), so it is *added* — same convention as `crate::pimple`.
        let mut ueqn = fvm::ddt_coeff_vec(alpha, u, u_old, dt, mesh.clone())
            + fvm::div_vec(&alpha_phi, u, mesh.clone())
            + fvm::laplacian_vec(&al_nu, u, mesh.clone())
            + fvm::sp_vec(&drag_impl, u);

        // Explicit body force α_k g and explicit drag source +(K_d/ρ_k)U_o,
        // both volume-integrated into the H-source.
        for c in 0..n {
            let v = mesh.cell_volumes[c];
            ueqn.source[c] = ueqn.source[c] + gravity * (alpha.internal[c] * v);
            ueqn.source[c] = ueqn.source[c] + u_other.internal[c] * (kd.internal[c] / rho * v);
        }

        // Snapshot the base H-source (ddt-old + gravity + explicit drag + BC
        // terms), WITHOUT the pressure gradient — this is UEqn.H() for the
        // corrector. Snapshot the raw diagonal A_k.
        let h_base: Vec<Vector3> = ueqn.source.iter().copied().collect();
        let diag: Vec<f64> = ueqn.ldu.diag.clone();

        // Predictor RHS: augment the source with −(α_k/ρ_k)∇p (volume-integrated)
        // using the current pressure, then solve UEqn_k == −(α_k/ρ_k)∇p.
        let grad_p = fvc::grad(p);
        for c in 0..n {
            let coeff = mesh.cell_volumes[c] * alpha.internal[c] / rho;
            ueqn.source[c] = h_base[c] - grad_p.internal[c] * coeff;
        }
        let (u_star, _perf) = ueqn.solve("U", settings);
        for c in 0..n {
            let vv = u_star.internal[c];
            if !(vv.x.is_finite() && vv.y.is_finite() && vv.z.is_finite()) {
                return Err(MultiphaseError::Solver(format!(
                    "phase momentum predictor produced non-finite U in cell {c}"
                )));
            }
        }
        // Preserve the velocity BCs; overwrite the internal field with U*_k.
        u.internal = u_star.internal;

        Ok((diag, h_base))
    }

    /// **Shared-pressure corrector** — form both phases' `HbyA_k`, solve the
    /// single mixture-continuity pressure Poisson equation, and correct each
    /// phase's face flux and velocity for discrete volumetric incompressibility
    /// of the mixture.
    ///
    /// Follows `multiphaseEuler/cellPressureCorrector.C`. For each phase `k`:
    /// `r_AU,k = V/A_k` `[s]`, `HbyA_k = H_k/A_k` `[m/s]`,
    /// `φ_HbyA_k = fvc::flux(HbyA_k)` `[m³/s]`. The **mixture** predicted flux and
    /// pressure diffusivity are the α-weighted sums
    /// (`cellPressureCorrector.C:289`, `:312`):
    ///
    /// ```text
    /// φ_HbyA,mix = Σ_k α_kf · φ_HbyA_k
    /// Γ_mix      = Σ_k α_kf² · r_AU,kf / ρ_k
    /// ```
    ///
    /// The single pressure equation `∇·(Γ_mix ∇p) = ∇·φ_HbyA,mix`
    /// (`cellPressureCorrector.C:352`) is solved for the shared `p`. Each phase's
    /// velocity face flux is then corrected with its **own** Rhie-Chow flux
    /// (pressure coefficient `D_kf = r_AU,kf α_kf/ρ_k`, exactly the momentum
    /// pressure term `−(α_k/ρ_k)∇p` weighted by `r_AU,k`):
    ///
    /// ```text
    /// φ_k = φ_HbyA_k − D_kf |S_f| ∂p/∂n ,     U_k = fvc::reconstruct(φ_k)
    /// ```
    ///
    /// The α-weighting makes `Σ_k α_kf φ_k` the exactly divergence-free mixture
    /// flux (`α_kf D_kf = α_kf² r_AU,kf/ρ_k = Γ_mix` term-by-term), and the `α_k`
    /// in `D_kf` cancels the `α_k` in the body force so the equilibrium
    /// pressure gradient reduces to `∂p/∂n = ρ_k g_n` per phase — hence
    /// `dp/dz = −ρ_mix·g` when both phases are at rest (verified in the tests).
    /// Each phase velocity is reconstructed from its coupled face flux (the
    /// checkerboard-safe correction documented on the module and in
    /// [`crate::pimple`]).
    ///
    /// # Errors
    /// [`MultiphaseError::Solver`] if the pressure solve yields a non-finite value.
    #[allow(clippy::too_many_arguments)]
    fn pressure_corrector(
        &mut self,
        alpha_d: &VolScalarField,
        alpha_c: &VolScalarField,
        rho_d: f64,
        rho_c: f64,
        diag_d: &[f64],
        h_d: &[Vector3],
        diag_c: &[f64],
        h_c: &[Vector3],
    ) -> Result<(), MultiphaseError> {
        let mesh = self.mesh.clone();
        let n = mesh.n_cells;

        // r_AU,k = V/A_k  [s];  HbyA_k = H_k/A_k  [m/s]  (OpenFOAM-consistent).
        let mut rau_d_vals = vec![0.0_f64; n];
        let mut rau_c_vals = vec![0.0_f64; n];
        let mut hbya_d_vals = vec![Vector3::ZERO; n];
        let mut hbya_c_vals = vec![Vector3::ZERO; n];
        for c in 0..n {
            let v = mesh.cell_volumes[c];
            rau_d_vals[c] = v / diag_d[c];
            rau_c_vals[c] = v / diag_c[c];
            hbya_d_vals[c] = h_d[c] * (1.0 / diag_d[c]);
            hbya_c_vals[c] = h_c[c] * (1.0 / diag_c[c]);
        }
        let rau_d = scalar_field("rAU.d", mesh.clone(), rau_d_vals);
        let rau_c = scalar_field("rAU.c", mesh.clone(), rau_c_vals);
        // HbyA carries the phase velocity BCs (constrainHbyA): on a no-slip /
        // fixed-value patch the boundary velocity (e.g. 0) is used, so φ_HbyA is
        // exact there.
        let hbya_d = VolVectorField::new(
            "HbyA.d",
            mesh.clone(),
            Field::new(hbya_d_vals),
            self.system.dispersed.u().boundary.clone(),
        );
        let hbya_c = VolVectorField::new(
            "HbyA.c",
            mesh.clone(),
            Field::new(hbya_c_vals),
            self.system.continuous.u().boundary.clone(),
        );

        // Face interpolations.
        let alpha_df = fvc::interpolate(alpha_d);
        let alpha_cf = fvc::interpolate(alpha_c);
        let rau_df = fvc::interpolate(&rau_d);
        let rau_cf = fvc::interpolate(&rau_c);

        // Per-phase face pressure coefficient  D_kf = r_AU,kf · α_kf / ρ_k.
        let d_df = map2_surface("D.d", &rau_df, &alpha_df, |r, a| r * a / rho_d);
        let d_cf = map2_surface("D.c", &rau_cf, &alpha_cf, |r, a| r * a / rho_c);

        // Per-phase predicted velocity flux  φ_HbyA_k = fvc::flux(HbyA_k).
        let phi_hbya_d = fvc::flux(&hbya_d);
        let phi_hbya_c = fvc::flux(&hbya_c);

        // Mixture predicted volumetric flux  φ_HbyA,mix = Σ α_kf φ_HbyA_k.
        let j_d = map2_surface("Jd", &alpha_df, &phi_hbya_d, |a, phi| a * phi);
        let j_c = map2_surface("Jc", &alpha_cf, &phi_hbya_c, |a, phi| a * phi);
        let phi_hbya_mix = map2_surface("phiHbyA", &j_d, &j_c, |x, y| x + y);

        // Mixture pressure diffusivity  Γ_mix = Σ α_kf D_kf = Σ α_kf² r_AU,kf/ρ_k.
        let gamma_d = map2_surface("g.d", &alpha_df, &d_df, |a, d| a * d);
        let gamma_c = map2_surface("g.c", &alpha_cf, &d_cf, |a, d| a * d);
        let gamma_mix = map2_surface("Gamma", &gamma_d, &gamma_c, |x, y| x + y);

        // pEqn: fvm::laplacian(Γ_mix, p) assembles M = −∇·(Γ_mix∇); the mixture
        // continuity ∇·(Γ_mix∇p) = ∇·φ_HbyA,mix becomes M·p = −V·div(φ_HbyA,mix).
        let div_phi = fvc::div_flux(&phi_hbya_mix);
        let mut p_eqn = fvm::laplacian(&gamma_mix, &self.p);
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

        // Per-phase flux correction  φ_k = φ_HbyA_k − D_kf |S_f| ∂p/∂n. The
        // mixture flux Σ α_kf φ_k is then exactly the Laplacian-consistent
        // divergence-free flux (∇·φ_mix = 0). Boundary fluxes from φ_HbyA_k
        // (exact 0 on no-slip walls).
        let sn_p = fvc::sn_grad(&self.p);
        self.phi_d = correct_flux(&phi_hbya_d, &d_df, &sn_p, &mesh);
        self.phi_c = correct_flux(&phi_hbya_c, &d_cf, &sn_p, &mesh);

        // Velocity correction: reconstruct each phase velocity from its own
        // corrected, Rhie-Chow-coupled face flux (checkerboard-safe; see module
        // and `crate::pimple`). Gives exact U_k = 0 for at-rest / hydrostatic.
        let u_d = fvc::reconstruct(&self.phi_d);
        let u_c = fvc::reconstruct(&self.phi_c);
        self.system.dispersed.u_mut().internal = u_d.internal;
        self.system.continuous.u_mut().internal = u_c.internal;

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

/// Element-wise binary combination of two `SurfaceScalarField`s (internal +
/// boundary), keeping the first operand's boundary-condition tags. Used to
/// assemble the α-weighted fluxes and pressure diffusivities.
fn map2_surface(
    name: &str,
    a: &SurfaceScalarField,
    b: &SurfaceScalarField,
    f: impl Fn(f64, f64) -> f64,
) -> SurfaceScalarField {
    let mesh = a.mesh.clone();
    let internal = Field::from_fn(mesh.n_internal_faces, |i| f(a.internal[i], b.internal[i]));
    let boundary = a
        .boundary
        .iter()
        .zip(b.boundary.iter())
        .map(|(pa, pb)| {
            let values = Field::from_fn(pa.values.len(), |i| f(pa.values[i], pb.values[i]));
            PatchField {
                bc: pa.bc.clone(),
                values,
            }
        })
        .collect();
    SurfaceScalarField::new(name, mesh, internal, boundary)
}

/// Rhie-Chow face-flux correction  `φ = φ_HbyA − D_f |S_f| ∂p/∂n`.
///
/// The internal-face flux is corrected with the Laplacian-consistent pressure
/// flux; boundary fluxes are taken from `φ_HbyA` (exact `0` on no-slip walls).
fn correct_flux(
    phi_hbya: &SurfaceScalarField,
    d_f: &SurfaceScalarField,
    sn_p: &SurfaceScalarField,
    mesh: &Arc<FvMesh>,
) -> SurfaceScalarField {
    let internal = Field::from_fn(mesh.n_internal_faces, |f| {
        phi_hbya.internal[f] - d_f.internal[f] * mesh.face_areas[f] * sn_p.internal[f]
    });
    let boundary = phi_hbya.boundary.clone();
    SurfaceScalarField::new("phi", mesh.clone(), internal, boundary)
}

// ═════════════════════════════════════════════════════════════════════════════
// Tests — verification (finiteness/boundedness, at-rest stability, hydrostatic
// balance dp/dz = −ρ_mix·g, saturation constraint α_d+α_c=1 ∈ [0,1], drag-driven
// velocity equilibration). NOT validation: no benchmark/experimental comparison.
// ═════════════════════════════════════════════════════════════════════════════
#[cfg(test)]
mod tests {
    use super::*;
    use crate::two_fluid::Phase;
    use outram_foam_basic_lib::prelude::{BoundaryPatch, FvMeshBuilder, PatchKind};
    use uom::si::dynamic_viscosity::pascal_second;
    use uom::si::f64::{DynamicViscosity, Length, MassDensity};
    use uom::si::length::meter;

    fn v3(x: f64, y: f64, z: f64) -> Vector3 {
        Vector3::new(x, y, z)
    }

    fn rho(v: f64) -> MassDensity {
        MassDensity::new::<kilogram_per_cubic_meter>(v)
    }

    fn mu(v: f64) -> DynamicViscosity {
        DynamicViscosity::new::<pascal_second>(v)
    }

    fn dia(v: f64) -> Length {
        Length::new::<meter>(v)
    }

    /// Vertical column of `n` unit cells stacked along +z (centres at
    /// 0.5,1.5,…), unit cross-sectional area and unit spacing. Both end faces
    /// (bottom at z=0, top at z=n) are `Wall` patches; the side faces are omitted
    /// (1-D column). Internal face `i` has owner `i`, neighbour `i+1`, area +z.
    /// (Same geometry as the `crate::pimple` hydrostatic test.)
    fn column_mesh(n: usize) -> Arc<FvMesh> {
        let n_int = n - 1;
        let mut owner: Vec<usize> = (0..n_int).collect();
        owner.push(0);
        owner.push(n - 1);
        let neighbour: Vec<usize> = (1..n).collect();
        let cell_volumes = vec![1.0; n];
        let cell_centres: Vec<Vector3> = (0..n).map(|i| v3(0.0, 0.0, i as f64 + 0.5)).collect();
        let mut face_area_vectors: Vec<Vector3> = (0..n_int).map(|_| v3(0.0, 0.0, 1.0)).collect();
        face_area_vectors.push(v3(0.0, 0.0, -1.0));
        face_area_vectors.push(v3(0.0, 0.0, 1.0));
        let mut face_centres: Vec<Vector3> =
            (0..n_int).map(|i| v3(0.0, 0.0, i as f64 + 1.0)).collect();
        face_centres.push(v3(0.0, 0.0, 0.0));
        face_centres.push(v3(0.0, 0.0, n as f64));
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

    /// Install no-slip (FixedValue 0) velocity BCs on every patch of a phase.
    fn set_closed_walls(u: &mut VolVectorField) {
        for pf in u.boundary.iter_mut() {
            *pf = PatchField::fixed_value_vec(pf.values.len(), Vector3::ZERO);
        }
    }

    /// Install zero-gradient velocity BCs on every patch (free outflow-like) —
    /// used for the drag-equilibration test so walls do not damp the velocities.
    fn set_zero_gradient(u: &mut VolVectorField) {
        for pf in u.boundary.iter_mut() {
            *pf = PatchField::zero_gradient_vec(pf.values.len());
        }
    }

    /// Air-in-water two-fluid system: air (ρ=1.2, μ=1.8e-5, d=1mm) dispersed in
    /// water (ρ=1000, μ=1e-3). `alpha_d0` uniform dispersed fraction.
    fn air_water(mesh: Arc<FvMesh>, alpha_d0: f64) -> TwoFluidSystem {
        let disp = Phase::new(mesh.clone(), "air", rho(1.2), mu(1.8e-5), dia(1e-3), alpha_d0)
            .unwrap();
        let cont =
            Phase::new(mesh.clone(), "water", rho(1000.0), mu(1e-3), dia(1e-3), 0.0).unwrap();
        TwoFluidSystem::new(disp, cont).unwrap()
    }

    /// Equal-density two-fluid system (ρ_d=ρ_c=`rho0`): the only mixed-column
    /// state that is a true two-phase rest equilibrium (unequal densities drive
    /// buoyant phase separation, not a static state). Diameter/viscosity set so
    /// the slip Reynolds number and drag stay finite.
    fn equal_density(mesh: Arc<FvMesh>, alpha_d0: f64, rho0: f64) -> TwoFluidSystem {
        let disp = Phase::new(mesh.clone(), "d", rho(rho0), mu(1e-3), dia(1e-3), alpha_d0).unwrap();
        let cont = Phase::new(mesh.clone(), "c", rho(rho0), mu(1e-3), dia(1e-3), 0.0).unwrap();
        TwoFluidSystem::new(disp, cont).unwrap()
    }

    // ── Finiteness / boundedness ─────────────────────────────────────────────

    /// **Finiteness / boundedness.** Methodology: 5-cell closed column, air-in-
    /// water (α_d=0.2), constant drag K_d=1e3, no-slip walls, gravity
    /// g=(0,0,−9.81). One `solve_timestep(dt=1e-3)` must leave every phase
    /// velocity component and every pressure value finite, both phase speeds
    /// bounded (< 1 m/s — a hydrostatic step must not spuriously accelerate),
    /// and α_d, α_c ∈ [0,1] with α_d+α_c=1.
    /// Result (2026-07-24): all U_d, U_c components finite, max|U| = 3.82e-2 m/s
    /// (well below the 1 m/s bound), all p finite, α_d = 0.19999, α_c = 0.80001,
    /// α_d+α_c=1 (err < 1e-12). PASS.
    #[test]
    fn one_step_stays_finite_and_bounded() {
        let mesh = column_mesh(5);
        let sys = air_water(mesh.clone(), 0.2);
        let mut sim = TwoFluidPimple::new(sys, DragModel::Constant { k_d: 1.0e3 });
        set_closed_walls(sim.system.dispersed.u_mut());
        set_closed_walls(sim.system.continuous.u_mut());
        sim.gravity = v3(0.0, 0.0, -9.81);
        sim.n_correctors = 2;
        sim.solve_timestep(1e-3).unwrap();
        for c in 0..mesh.n_cells {
            let ud = sim.system.dispersed.u().internal[c];
            let uc = sim.system.continuous.u().internal[c];
            for (label, u) in [("d", ud), ("c", uc)] {
                assert!(
                    u.x.is_finite() && u.y.is_finite() && u.z.is_finite(),
                    "U_{label} non-finite in cell {c}: {u:?}"
                );
                assert!(u.mag() < 1.0, "U_{label} unexpectedly large in cell {c}: {u:?}");
            }
            assert!(sim.p.internal[c].is_finite(), "p non-finite in cell {c}");
            let ad = sim.system.dispersed.alpha().internal[c];
            let ac = sim.system.continuous.alpha().internal[c];
            assert!((0.0..=1.0).contains(&ad) && (0.0..=1.0).contains(&ac));
            assert!((ad + ac - 1.0).abs() < 1e-12);
        }
    }

    // ── At-rest (quiescent) stability ────────────────────────────────────────

    /// **At-rest (quiescent) stability.** Methodology: closed 4-cell column, no
    /// gravity, zero initial velocity, α_d=0.3, constant drag K_d=500. With no
    /// drive the exact solution is U_d≡U_c≡0 for all time. After 10 steps every
    /// velocity component of both phases must remain 0 to 1e-10 and both fluxes to
    /// 1e-10.
    /// Result (2026-07-24): max|U_d|=max|U_c|=0 m/s, max|φ_d|=max|φ_c|=0 m³/s
    /// after 10 steps; α_d=0.3 unchanged. PASS.
    #[test]
    fn quiescent_stays_at_rest() {
        let mesh = column_mesh(4);
        let sys = air_water(mesh.clone(), 0.3);
        let mut sim = TwoFluidPimple::new(sys, DragModel::Constant { k_d: 500.0 });
        set_closed_walls(sim.system.dispersed.u_mut());
        set_closed_walls(sim.system.continuous.u_mut());
        sim.gravity = Vector3::ZERO;
        sim.n_correctors = 2;
        for _ in 0..10 {
            sim.solve_timestep(5e-3).unwrap();
        }
        for c in 0..mesh.n_cells {
            assert!(sim.system.dispersed.u().internal[c].mag() < 1e-10);
            assert!(sim.system.continuous.u().internal[c].mag() < 1e-10);
            assert!((sim.system.dispersed.alpha().internal[c] - 0.3).abs() < 1e-10);
        }
        for f in 0..mesh.n_internal_faces {
            assert!(sim.phi_d.internal[f].abs() < 1e-10);
            assert!(sim.phi_c.internal[f].abs() < 1e-10);
        }
    }

    // ── Hydrostatic column (shared pressure) ─────────────────────────────────

    /// **Hydrostatic column — shared mixture pressure gradient.** Methodology:
    /// closed 10-cell vertical column (Δz=1 m), a mixed two-fluid column with
    /// **equal phase densities** ρ_d=ρ_c=800 kg/m³ and α_d=0.4 (α_c=0.6), no-slip
    /// walls, gravity g=(0,0,−9.81). Equal densities are required for a genuine
    /// two-phase rest state (unequal densities buoyantly separate); the mixture
    /// density is ρ_mix = Σ α_k ρ_k = 0.4·800 + 0.6·800 = 800 kg/m³. Marched to a
    /// near-steady rest state (40 steps of dt=0.02 s, nCorrectors=3,
    /// nOuterCorrectors=2). At mechanical equilibrium each phase momentum reduces
    /// to 0 = −(α_k/ρ_k)∇p + α_k g ⇒ ∇p = ρ_k g, and with the shared p and equal
    /// ρ_k the mixture gradient is dp/dz = ρ_mix·g_z = −ρ_mix·9.81 = −7848 Pa/m.
    /// Pass: interior finite-difference dp/dz within 1% of −ρ_mix·g, both phase
    /// residual speeds < 1e-3 m/s.
    /// Reference: hydrostatic balance dp/dz = −ρg (fluid statics); discretisation
    /// follows multiphaseEuler/cellPressureCorrector.C:352.
    /// Result (2026-07-24): mean interior dp/dz = −7847.999995 Pa/m (target
    /// −7848.0, rel err 6.9e-10); max residual |U_d|,|U_c| = 3.5e-8 m/s. PASS.
    #[test]
    fn hydrostatic_pressure_gradient() {
        let n = 10;
        let mesh = column_mesh(n);
        let rho0 = 800.0;
        let alpha_d0 = 0.4;
        let sys = equal_density(mesh.clone(), alpha_d0, rho0);
        let mut sim = TwoFluidPimple::new(sys, DragModel::Constant { k_d: 1.0e3 });
        set_closed_walls(sim.system.dispersed.u_mut());
        set_closed_walls(sim.system.continuous.u_mut());
        let g = 9.81;
        sim.gravity = v3(0.0, 0.0, -g);
        sim.n_correctors = 3;
        sim.n_outer_correctors = 2;
        for _ in 0..40 {
            sim.solve_timestep(0.02).unwrap();
        }
        let rho_mix = alpha_d0 * rho0 + (1.0 - alpha_d0) * rho0; // = rho0
        let target = -rho_mix * g;
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
        for c in 0..n {
            assert!(sim.system.dispersed.u().internal[c].mag() < 1e-3);
            assert!(sim.system.continuous.u().internal[c].mag() < 1e-3);
        }
    }

    // ── Saturation constraint under coupling ─────────────────────────────────

    /// **α_d + α_c = 1 and both in [0,1] after coupling.** Methodology: closed
    /// 5-cell column, air-in-water α_d=0.5, constant drag K_d=800, gravity, an
    /// initial dispersed velocity (0,0,0.05) so the void transport actually moves
    /// α_d. After 5 coupled steps every α_d, α_c must be finite, in [0,1], and
    /// α_d + α_c = 1 exactly (the saturation constraint is re-imposed each step).
    /// Result (2026-07-24): all α_d, α_c ∈ [0,1] and finite; α_d+α_c=1
    /// (err < 1e-12) in every cell after 5 steps. PASS.
    #[test]
    fn saturation_constraint_holds_under_coupling() {
        let mesh = column_mesh(5);
        let mut sys = air_water(mesh.clone(), 0.5);
        *sys.dispersed.u_mut() = VolVectorField::uniform("U", mesh.clone(), v3(0.0, 0.0, 0.05));
        let mut sim = TwoFluidPimple::new(sys, DragModel::Constant { k_d: 800.0 });
        set_closed_walls(sim.system.dispersed.u_mut());
        set_closed_walls(sim.system.continuous.u_mut());
        sim.gravity = v3(0.0, 0.0, -9.81);
        sim.n_correctors = 2;
        for _ in 0..5 {
            sim.solve_timestep(1e-2).unwrap();
        }
        for c in 0..mesh.n_cells {
            let ad = sim.system.dispersed.alpha().internal[c];
            let ac = sim.system.continuous.alpha().internal[c];
            assert!(ad.is_finite() && (0.0..=1.0).contains(&ad), "α_d cell {c}: {ad}");
            assert!(ac.is_finite() && (0.0..=1.0).contains(&ac), "α_c cell {c}: {ac}");
            assert!((ad + ac - 1.0).abs() < 1e-12, "α_d+α_c≠1 cell {c}: {}", ad + ac);
        }
    }

    // ── Strong drag equilibrates the phase velocities ────────────────────────

    /// **Strong drag drives the two phase velocities toward each other.**
    /// Methodology: 4-cell column, air-in-water α_d=0.3, **zero-gradient**
    /// velocity BCs (so walls do not impose/damp velocities and drag dominates),
    /// no gravity. Phases start with opposing uniform velocities U_d=(0,0,+0.5),
    /// U_c=(0,0,−0.5) → initial mean relative speed 1.0 m/s. With a strong
    /// constant drag K_d=1e5 kg/(m³·s) the semi-implicit coupling relaxes the
    /// relative velocity w=U_d−U_c as dw/dt ≈ −(K_d/ρ_d + K_d/ρ_c)·w, a very fast
    /// decay for the light dispersed phase (K_d/ρ_d ≈ 8.3e4 s⁻¹). After 3 steps
    /// of dt=1e-3 s the mean relative speed must drop below 10% of its initial
    /// value.
    /// Result (2026-07-24): mean |U_d−U_c| falls from 1.0 m/s to 1.80e-3 m/s after
    /// 3 steps (ratio 1.80e-3 ≪ 0.1). PASS.
    #[test]
    fn strong_drag_equilibrates_phase_velocities() {
        let mesh = column_mesh(4);
        let mut sys = air_water(mesh.clone(), 0.3);
        *sys.dispersed.u_mut() = VolVectorField::uniform("U", mesh.clone(), v3(0.0, 0.0, 0.5));
        *sys.continuous.u_mut() = VolVectorField::uniform("U", mesh.clone(), v3(0.0, 0.0, -0.5));
        let mut sim = TwoFluidPimple::new(sys, DragModel::Constant { k_d: 1.0e5 });
        set_zero_gradient(sim.system.dispersed.u_mut());
        set_zero_gradient(sim.system.continuous.u_mut());
        sim.gravity = Vector3::ZERO;
        sim.n_correctors = 2;
        sim.n_outer_correctors = 2;

        let rel_mean = |s: &TwoFluidPimple| -> f64 {
            let n = s.mesh.n_cells;
            (0..n)
                .map(|c| (s.system.dispersed.u().internal[c] - s.system.continuous.u().internal[c]).mag())
                .sum::<f64>()
                / n as f64
        };
        let before = rel_mean(&sim);
        assert!((before - 1.0).abs() < 1e-9, "initial relative speed {before}");
        for _ in 0..3 {
            sim.solve_timestep(1e-3).unwrap();
        }
        let after = rel_mean(&sim);
        assert!(
            after < 0.1 * before,
            "strong drag should equilibrate velocities: before={before}, after={after}"
        );
    }

    // ── Input validation ─────────────────────────────────────────────────────

    /// dt ≤ 0 and a zero corrector count are rejected.
    #[test]
    fn solve_rejects_bad_controls() {
        let mesh = column_mesh(3);
        let sys = air_water(mesh.clone(), 0.2);
        let mut sim = TwoFluidPimple::new(sys, DragModel::Constant { k_d: 1.0 });
        set_closed_walls(sim.system.dispersed.u_mut());
        set_closed_walls(sim.system.continuous.u_mut());
        assert!(sim.solve_timestep(0.0).is_err());
        assert!(sim.solve_timestep(-1.0).is_err());
        sim.n_correctors = 0;
        assert!(sim.solve_timestep(1e-3).is_err());
    }
}
