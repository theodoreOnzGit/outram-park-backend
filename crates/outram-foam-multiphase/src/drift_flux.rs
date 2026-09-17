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

//! Stage 1 — **Drift-flux mixture model foundation** (bead `op-2kk.1`).
//!
//! Pure-Rust port of the mixture-property, algebraic-slip, and void-fraction
//! transport pieces of OpenFOAM's `incompressibleDriftFlux` solver module,
//! built on [`outram_foam_basic_lib`]'s finite-volume framework.
//!
//! ## Physics
//!
//! The drift-flux model represents a two-phase mixture (a **dispersed** phase
//! `d` — e.g. gas bubbles or solid particles — carried in a **continuous**
//! phase `c` — e.g. liquid) with a *single* mixture momentum field plus a
//! transported dispersed-phase volume fraction `α`. The inter-phase slip is
//! not resolved by a second momentum equation (that is the Euler-Euler
//! Stage 2 job); instead it is closed **algebraically** by a relative- /
//! drift-velocity model. This foundation implements three ingredients:
//!
//! 1. **Mixture properties** ([`DriftFluxMixture`]) — mixture density
//!    `ρ_m = α·ρ_d + (1−α)·ρ_c` and a volume-weighted mixture viscosity.
//! 2. **Algebraic slip closures** ([`SlipModel`]) — the per-cell drift
//!    velocity `U_dm` (dispersed-phase velocity relative to the mixture
//!    volumetric flux), as an enum (no `dyn` dispatch).
//! 3. **Void-fraction transport** ([`DriftFlux::advance_alpha`]) — one
//!    implicit-Euler step of
//!    `∂α/∂t + ∇·(φ_m·α) + ∇·(φ_dm·α(1−α)) = 0`, clamped to `[0,1]`.
//!
//! ## Upstream provenance (ported C++ → Rust)
//!
//! Source tree: `applications/modules/incompressibleDriftFlux/` of OpenFOAM-dev
//! (GPL-3.0). Exact file/line citations appear on each item below. Summary:
//!
//! | Rust item | OpenFOAM source |
//! |---|---|
//! | [`DriftFluxMixture::rho_mixture`] | `incompressibleDriftFluxMixture.C:89` |
//! | [`DriftFluxMixture::mu_mixture`]  | `incompressibleDriftFluxMixture.C:90` + `mixtureViscosityModels/plastic/plastic.C:79` (concept) |
//! | [`SlipModel::TerminalVelocity`]   | `relativeVelocityModels/simple/simple.C:64` |
//! | [`SlipModel::UserDefined`]        | `relativeVelocityModels/general/general.C:65` (concept) |
//! | [`SlipModel::ZuberFindlay`]       | Zuber & Findlay (1965) — literature closure |
//! | [`DriftFlux::advance_alpha`]      | `alphaSuSp.C:32` (`alphaPhi`) + `twoPhaseSolver` `alphaEqn` |
//!
//! ## Honest scope — what is **NOT** modelled here
//!
//! This is a **tested foundation**, not a converged coupled solver. Reviewers
//! and users must not read it as validated multiphase CFD. Specifically:
//!
//! - **No mixture-momentum / pressure coupling.** There is no PIMPLE/PISO loop,
//!   no pressure Poisson solve, no `U_m` update. The mixture velocity `U_m`
//!   and the face flux `φ_m` are *inputs* the caller prescribes; `α` transport
//!   is advanced on that given flux. Coupling `U_m`–`p`–`α` is later work.
//! - **Simplified `α` bounding.** OpenFOAM bounds `α` with MULES (a flux-
//!   corrected, strictly conservative limiter). Here we use a plain post-solve
//!   **clamp to `[0,1]`**, which keeps `α` physical but is **not** globally
//!   conservative when the clamp is active. Documented as a known restriction.
//! - **Drift term treated explicitly.** The nonlinear compression flux
//!   `∇·(φ_dm·α(1−α))` is discretised explicitly (deferred correction, upwind
//!   on `α(1−α)`) and added as a source, not implicitly. Fine for small drift
//!   Courant numbers; can go unstable for large ones.
//! - **No turbulence coupling, no packing/dispersion, no non-Newtonian
//!   rheology.** OpenFOAM's `mixtureViscosityModels` (plastic, slurry,
//!   Bingham, Herschel-Bulkley, Quemada) and `packingDispersionModels` are
//!   not ported; the mixture viscosity is a single volume-weighted rule.
//! - **First-order in space and time.** Upwind convection + implicit Euler.
//! - **No `uom` on the `Vector3` velocity boundary.** Following the crate's
//!   `k_omega_sst` precedent, vector velocities are carried as `Vector3` in
//!   SI `m/s` (documented per field); `uom` types the *scalar* property inputs
//!   (density, viscosity) at the constructor boundary.
//!
//! **Benchmark validation is a later, human-run step.** No benchmark
//! comparison has been performed; the tests below are *verification* checks
//! (formula exactness, hand-computed slip velocities, advection boundedness),
//! not *validation* against experimental or reference-solver data.

use crate::MultiphaseError;
use outram_foam_basic_lib::fv_operators::{fvc, fvm};
use outram_foam_basic_lib::prelude::{
    BoundaryCondition, Field, FvMesh, PatchField, SolverSettings, SurfaceScalarField, Vector3,
    VolScalarField, VolVectorField,
};
use std::sync::Arc;
use uom::si::dynamic_viscosity::pascal_second;
use uom::si::f64::{DynamicViscosity, MassDensity};
use uom::si::mass_density::kilogram_per_cubic_meter;

/// Dispersed-phase volume fraction `α`, dimensionless, valid range `[0, 1]`.
///
/// Carried as an [`outram_foam_basic_lib`] [`VolScalarField`] (per-cell `f64`).
/// This alias documents the physical meaning at the API boundary.
pub type VoidFraction = VolScalarField;

// ─────────────────────────────────────────────────────────────────────────────
// Mixture properties
// ─────────────────────────────────────────────────────────────────────────────

/// Two-phase mixture material properties for the drift-flux model.
///
/// Holds the constant per-phase densities and dynamic viscosities of the
/// **dispersed** (`d`) and **continuous** (`c`) phases together with the
/// transported dispersed-phase volume-fraction field `α`. From these it forms
/// the cell-wise **mixture** density and viscosity used by the momentum and
/// transport equations.
///
/// Ports the state of OpenFOAM's `incompressibleDriftFluxMixture`
/// (`incompressibleDriftFluxMixture.C`). There, `rhod_`/`rhoc_` are constant
/// per-phase densities and `rho_ = alpha1()*rhod_ + alpha2()*rhoc_`
/// (`incompressibleDriftFluxMixture.C:89`), with `alpha2 = 1 − alpha1`.
///
/// ## Units and valid ranges
///
/// - `ρ_d`, `ρ_c` — mass density `[kg/m³]`, must be **> 0**.
/// - `μ_d`, `μ_c` — dynamic viscosity `[Pa·s]`, must be **≥ 0**.
/// - `α` — dispersed volume fraction `[-]`, physical range `[0, 1]`.
///
/// Densities/viscosities are stored internally as `f64` in SI base units
/// (`kg/m³`, `Pa·s`); the constructor and scalar accessors use `uom` types so
/// the physical dimension is checked at the API boundary. Field-valued outputs
/// ([`rho_mixture`](Self::rho_mixture), [`mu_mixture`](Self::mu_mixture)) are
/// `VolScalarField`s in those same SI units (documented, not `uom`-typed,
/// matching the finite-volume field convention used across this workspace).
pub struct DriftFluxMixture {
    /// Finite-volume mesh the fields live on.
    pub mesh: Arc<FvMesh>,
    /// Dispersed-phase density `ρ_d` `[kg/m³]`.
    rho_d: f64,
    /// Continuous-phase density `ρ_c` `[kg/m³]`.
    rho_c: f64,
    /// Dispersed-phase dynamic viscosity `μ_d` `[Pa·s]`.
    mu_d: f64,
    /// Continuous-phase dynamic viscosity `μ_c` `[Pa·s]`.
    mu_c: f64,
    /// Dispersed-phase volume fraction `α ∈ [0, 1]` `[-]`.
    alpha: VoidFraction,
}

impl DriftFluxMixture {
    /// Construct a mixture from per-phase properties and a uniform initial `α`.
    ///
    /// # Parameters
    /// - `mesh` — the finite-volume mesh (shared, `Arc`).
    /// - `rho_d`, `rho_c` — dispersed / continuous density `[kg/m³]` (`uom`),
    ///   both must be strictly positive.
    /// - `mu_d`, `mu_c` — dispersed / continuous dynamic viscosity `[Pa·s]`
    ///   (`uom`), both must be non-negative.
    /// - `alpha0` — uniform initial dispersed volume fraction `[-]`, in `[0,1]`.
    ///
    /// The `α` field is created with a **zero-gradient** boundary on every
    /// patch; replace its boundary via [`alpha_mut`](Self::alpha_mut) (e.g. a
    /// `FixedValue` inlet) before advancing transport if an inflow is needed.
    ///
    /// # Errors
    /// [`MultiphaseError::InvalidInput`] if a density is `≤ 0`, a viscosity is
    /// `< 0`, or `alpha0 ∉ [0, 1]`.
    pub fn new(
        mesh: Arc<FvMesh>,
        rho_d: MassDensity,
        rho_c: MassDensity,
        mu_d: DynamicViscosity,
        mu_c: DynamicViscosity,
        alpha0: f64,
    ) -> Result<Self, MultiphaseError> {
        let rho_d = rho_d.get::<kilogram_per_cubic_meter>();
        let rho_c = rho_c.get::<kilogram_per_cubic_meter>();
        let mu_d = mu_d.get::<pascal_second>();
        let mu_c = mu_c.get::<pascal_second>();
        if rho_d <= 0.0 || rho_c <= 0.0 {
            return Err(MultiphaseError::InvalidInput(format!(
                "phase densities must be > 0 (got rho_d={rho_d}, rho_c={rho_c})"
            )));
        }
        if mu_d < 0.0 || mu_c < 0.0 {
            return Err(MultiphaseError::InvalidInput(format!(
                "phase viscosities must be >= 0 (got mu_d={mu_d}, mu_c={mu_c})"
            )));
        }
        if !(0.0..=1.0).contains(&alpha0) {
            return Err(MultiphaseError::InvalidInput(format!(
                "initial alpha must be in [0,1] (got {alpha0})"
            )));
        }
        let alpha = VolScalarField::uniform("alpha", mesh.clone(), alpha0);
        Ok(Self {
            mesh,
            rho_d,
            rho_c,
            mu_d,
            mu_c,
            alpha,
        })
    }

    /// Dispersed-phase density `ρ_d` `[kg/m³]`.
    pub fn rho_dispersed(&self) -> MassDensity {
        MassDensity::new::<kilogram_per_cubic_meter>(self.rho_d)
    }

    /// Continuous-phase density `ρ_c` `[kg/m³]`.
    pub fn rho_continuous(&self) -> MassDensity {
        MassDensity::new::<kilogram_per_cubic_meter>(self.rho_c)
    }

    /// Dispersed-phase dynamic viscosity `μ_d` `[Pa·s]`.
    pub fn mu_dispersed(&self) -> DynamicViscosity {
        DynamicViscosity::new::<pascal_second>(self.mu_d)
    }

    /// Continuous-phase dynamic viscosity `μ_c` `[Pa·s]`.
    pub fn mu_continuous(&self) -> DynamicViscosity {
        DynamicViscosity::new::<pascal_second>(self.mu_c)
    }

    /// The dispersed-phase volume-fraction field `α` `[-]` (read-only).
    pub fn alpha(&self) -> &VoidFraction {
        &self.alpha
    }

    /// The dispersed-phase volume-fraction field `α` `[-]` (mutable).
    ///
    /// Use this to set the initial internal distribution or to install a
    /// boundary condition (e.g. a `FixedValue` inlet) before transport.
    pub fn alpha_mut(&mut self) -> &mut VoidFraction {
        &mut self.alpha
    }

    /// Cell-wise **mixture density** `ρ_m = α·ρ_d + (1−α)·ρ_c` `[kg/m³]`.
    ///
    /// Direct port of `incompressibleDriftFluxMixture.C:89`:
    /// `rho_ = alpha1()*rhod_ + alpha2()*rhoc_;` with `alpha1 = α`,
    /// `alpha2 = 1 − α`. The result carries a zero-gradient boundary and is a
    /// `VolScalarField` in `kg/m³`.
    ///
    /// As `α` sweeps `0 → 1`, `ρ_m` moves linearly `ρ_c → ρ_d`. Values outside
    /// `[0,1]` are not clamped here (the caller keeps `α` bounded via
    /// transport); an out-of-range `α` therefore extrapolates the linear rule.
    pub fn rho_mixture(&self) -> VolScalarField {
        let a = self.alpha.internal.as_slice();
        let vals: Vec<f64> = a
            .iter()
            .map(|&al| al * self.rho_d + (1.0 - al) * self.rho_c)
            .collect();
        scalar_field("rho", self.mesh.clone(), vals)
    }

    /// Cell-wise **mixture dynamic viscosity** `μ_m = α·μ_d + (1−α)·μ_c`
    /// `[Pa·s]` (volume-weighted linear mixing rule).
    ///
    /// ## Mixing rule and its provenance
    ///
    /// OpenFOAM computes the mixture *kinematic* viscosity as
    /// `nu_ = muModel->mu(rhoc*nuc, U)/rho` (`incompressibleDriftFluxMixture.C:90`),
    /// where `muModel` is a run-time-selected `mixtureViscosityModel` — e.g.
    /// `plastic` adds a dispersed-phase thickening term
    /// `muc + coeff·(10^{exp·α} − 1)`, capped at `muMax`
    /// (`mixtureViscosityModels/plastic/plastic.C:79`). Those non-Newtonian
    /// closures are **not** ported at this foundation stage.
    ///
    /// Instead this uses the simplest physically-consistent rule — the
    /// **volume-weighted linear** average — which reproduces the two limits
    /// exactly (`μ_m = μ_c` at `α = 0`, `μ_m = μ_d` at `α = 1`) and is a common
    /// first approximation for dilute dispersions. This is a documented
    /// **restriction**: it omits the shear-thinning / yield-stress behaviour of
    /// real slurries and bubbly flows. Result is a `VolScalarField` in `Pa·s`.
    pub fn mu_mixture(&self) -> VolScalarField {
        let a = self.alpha.internal.as_slice();
        let vals: Vec<f64> = a
            .iter()
            .map(|&al| al * self.mu_d + (1.0 - al) * self.mu_c)
            .collect();
        scalar_field("mu", self.mesh.clone(), vals)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Algebraic slip / drift-velocity closures
// ─────────────────────────────────────────────────────────────────────────────

/// Algebraic-slip closure for the dispersed-phase **drift velocity** `U_dm`.
///
/// `U_dm` is the velocity of the dispersed phase **relative to the mixture
/// volumetric flux** (OpenFOAM's `Udm`, `relativeVelocityModel.H`). It closes
/// the phase slip without a second momentum equation. All variants return a
/// per-cell `Vector3` in SI `m/s`.
///
/// Enum dispatch (not `dyn`) per the workspace design rules: the set of slip
/// closures is closed and known at compile time, so a `match` is exhaustive
/// and every variant is rust-analyzer-navigable.
///
/// ## Variants
/// - [`ZuberFindlay`](Self::ZuberFindlay) — the classic distribution-parameter
///   drift-flux correlation (Zuber & Findlay, 1965).
/// - [`TerminalVelocity`](Self::TerminalVelocity) — a constant terminal / slip
///   velocity with a hindered-settling factor; ports OpenFOAM's `simple` model.
/// - [`UserDefined`](Self::UserDefined) — a caller-supplied per-cell `U_dm`
///   field, for closures not covered above (mirrors the flexibility of
///   OpenFOAM's `general` model).
pub enum SlipModel {
    /// **Zuber-Findlay** drift-flux correlation (Zuber, N. & Findlay, J.A.,
    /// 1965, *"Average Volumetric Concentration in Two-Phase Flow Systems"*,
    /// J. Heat Transfer **87**(4):453-468).
    ///
    /// The dispersed-phase (gas) velocity is `v_d = C₀·j + V_gj`, where `j` is
    /// the mixture volumetric flux (here approximated by the mixture cell
    /// velocity `U_m`), `C₀` the **distribution parameter** `[-]` and `V_gj`
    /// the **drift velocity** `[m/s]`. The drift velocity of the dispersed
    /// phase *relative to the mixture flux* is then
    /// `U_dm = v_d − j = (C₀ − 1)·U_m + V_gj`.
    ///
    /// Typical values: `C₀ ≈ 1.0–1.2` (`1.13` churn-turbulent bubbly flow),
    /// `V_gj` a small buoyant rise velocity `~0.1–0.3 m/s`. Valid for
    /// low-to-moderate `α`; the constant-`C₀`/`V_gj` form degrades near
    /// close-packing.
    ZuberFindlay {
        /// Distribution parameter `C₀` `[-]`.
        c0: f64,
        /// Drift velocity `V_gj` `[m/s]` (vector; typically aligned with, or
        /// against, gravity).
        vgj: Vector3,
    },

    /// **Terminal-velocity** closure with hindered settling —
    /// `U_dm = u_t · 10^{−a·max(α,0)}`.
    ///
    /// Ports the structure of OpenFOAM's `relativeVelocityModels::simple`
    /// (`simple.C:64`), whose diffusion-velocity coefficient carries the
    /// hindered-settling factor `pow(10, −a·max(αd,0))`. Here `u_t` `[m/s]` is
    /// the unhindered terminal (buoyant rise / settling) velocity of an
    /// isolated inclusion — it absorbs the upstream `(ρ_c/ρ_m)·V_c·|g|`
    /// coefficient and direction into one vector — and `a ≥ 0` `[-]` is the
    /// hindrance exponent (`a = 0` ⇒ no hindrance). As `α → 1` the drift
    /// velocity is damped toward zero, mimicking crowding.
    TerminalVelocity {
        /// Unhindered terminal / slip velocity `u_t` `[m/s]`.
        u_t: Vector3,
        /// Hindered-settling exponent `a ≥ 0` `[-]` in `10^{−a·α}`.
        hindrance_exp: f64,
    },

    /// **User-defined** per-cell drift-velocity field `U_dm` `[m/s]`, one
    /// `Vector3` per mesh cell.
    ///
    /// For closures outside the two built-ins — e.g. a correlation evaluated
    /// externally, or a tabulated field. Owns its captured state as a plain
    /// `Vec<Vector3>` (no `dyn Fn`, per the workspace no-trait-object rule),
    /// mirroring the arbitrary-coefficient flexibility of OpenFOAM's
    /// `relativeVelocityModels::general` (`general.C:65`). The vector length
    /// must equal the mesh cell count.
    UserDefined(Vec<Vector3>),
}

impl SlipModel {
    /// Per-cell drift velocity `U_dm` `[m/s]`, one `Vector3` per mesh cell.
    ///
    /// # Parameters
    /// - `u_m` — mixture cell velocity field `U_m` `[m/s]` (used by
    ///   [`ZuberFindlay`](Self::ZuberFindlay); the volumetric-flux surrogate).
    /// - `alpha` — dispersed volume fraction `α` `[-]` (used by the hindrance /
    ///   crowding factors).
    ///
    /// Both fields must be defined on the same mesh; the returned vector has
    /// length `mesh.n_cells`.
    ///
    /// # Errors
    /// [`MultiphaseError::InvalidInput`] if a [`UserDefined`](Self::UserDefined)
    /// field length does not match the cell count.
    pub fn drift_velocity(
        &self,
        u_m: &VolVectorField,
        alpha: &VolScalarField,
    ) -> Result<Vec<Vector3>, MultiphaseError> {
        let n = alpha.mesh.n_cells;
        match self {
            Self::ZuberFindlay { c0, vgj } => {
                let u = u_m.internal.as_slice();
                Ok((0..n).map(|c| u[c] * (c0 - 1.0) + *vgj).collect())
            }
            Self::TerminalVelocity { u_t, hindrance_exp } => {
                let a = alpha.internal.as_slice();
                Ok((0..n)
                    .map(|c| {
                        let factor = 10.0_f64.powf(-hindrance_exp * a[c].max(0.0));
                        *u_t * factor
                    })
                    .collect())
            }
            Self::UserDefined(field) => {
                if field.len() != n {
                    return Err(MultiphaseError::InvalidInput(format!(
                        "UserDefined drift field has {} entries, mesh has {n} cells",
                        field.len()
                    )));
                }
                Ok(field.clone())
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Drift-flux driver: void-fraction transport
// ─────────────────────────────────────────────────────────────────────────────

/// Drift-flux model driver — owns the mixture, the slip closure, and the
/// prescribed mixture flow, and advances the void-fraction transport equation.
///
/// **Not a coupled solver.** The mixture velocity `u_m` and its face flux
/// `phi` are *inputs*: the caller sets them (from a momentum solve elsewhere,
/// or a prescribed field) each step, then calls
/// [`advance_alpha`](Self::advance_alpha) to march `α` one implicit-Euler step.
/// See the module-level "Honest scope" section for what this deliberately does
/// not do (no pressure coupling, MULES-free bounding, explicit drift term).
pub struct DriftFlux {
    /// Finite-volume mesh (shared, `Arc`).
    pub mesh: Arc<FvMesh>,
    /// Mixture material properties and the `α` field.
    pub mixture: DriftFluxMixture,
    /// Algebraic slip closure for the drift velocity `U_dm`.
    pub slip: SlipModel,
    /// Mixture volumetric face flux `φ_m = U_m·S_f` `[m³/s]`. Prescribe each
    /// step (e.g. via [`outram_foam_basic_lib::fv_operators::fvc::flux`]).
    pub phi: SurfaceScalarField,
    /// Mixture cell velocity `U_m` `[m/s]` (drives the slip closures).
    pub u_m: VolVectorField,
    /// Time step `Δt` `[s]`, must be `> 0`.
    pub dt: f64,
}

impl DriftFlux {
    /// Construct a driver from a [`DriftFluxMixture`] and a [`SlipModel`].
    ///
    /// The mixture flux `phi` and velocity `u_m` start at zero and `dt = 1e-3 s`;
    /// set them before advancing. The mesh is taken from the mixture.
    pub fn new(mixture: DriftFluxMixture, slip: SlipModel) -> Self {
        let mesh = mixture.mesh.clone();
        let phi = SurfaceScalarField::zeros("phi", mesh.clone());
        let u_m = VolVectorField::zero("U", mesh.clone());
        Self {
            mesh,
            mixture,
            slip,
            phi,
            u_m,
            dt: 1.0e-3,
        }
    }

    /// Per-cell drift velocity `U_dm` `[m/s]` from the current slip closure and
    /// state. See [`SlipModel::drift_velocity`].
    ///
    /// # Errors
    /// Propagates [`MultiphaseError::InvalidInput`] from a mismatched
    /// [`SlipModel::UserDefined`] field.
    pub fn drift_velocity(&self) -> Result<Vec<Vector3>, MultiphaseError> {
        self.slip.drift_velocity(&self.u_m, self.mixture.alpha())
    }

    /// Advance the dispersed-phase volume fraction `α` by one implicit-Euler
    /// time step of the drift-flux transport equation, then clamp to `[0,1]`.
    ///
    /// ## Equation solved
    /// ```text
    /// ∂α/∂t  +  ∇·(φ_m·α)  +  ∇·(φ_dm·α(1−α))  =  0
    /// ```
    /// where `φ_m` is the prescribed mixture face flux and
    /// `φ_dm = U_dm·S_f` the drift face flux. Discretisation:
    ///
    /// - `∂α/∂t` — implicit Euler (`fvm::ddt`).
    /// - `∇·(φ_m·α)` — implicit first-order upwind (`fvm::div`).
    /// - `∇·(φ_dm·α(1−α))` — **explicit** (deferred), upwind on `α(1−α)`,
    ///   assembled into the source. This is the drift/compression flux; see the
    ///   module "Honest scope" note on why it is explicit.
    ///
    /// After the linear solve, `α` is **clamped** to `[0,1]` cell-by-cell (a
    /// MULES-free stand-in for OpenFOAM's flux-corrected bounding — physical
    /// but not strictly conservative when the clamp bites). The `α` field's
    /// boundary conditions are preserved (only internal values are updated), so
    /// a `FixedValue` inlet keeps feeding the domain.
    ///
    /// # Errors
    /// - [`MultiphaseError::InvalidInput`] if `dt ≤ 0` or the slip field is
    ///   malformed.
    /// - [`MultiphaseError::Solver`] if the linear solve returns a non-finite
    ///   value.
    pub fn advance_alpha(&mut self) -> Result<(), MultiphaseError> {
        if self.dt <= 0.0 {
            return Err(MultiphaseError::InvalidInput(format!(
                "dt must be > 0 (got {})",
                self.dt
            )));
        }
        let mesh = self.mesh.clone();
        let n = mesh.n_cells;

        // ── Drift face flux φ_dm = U_dm·S_f, and the α(1−α) compression flux ──
        let udm = self.drift_velocity()?;
        let udm_field = vector_field("Udm", mesh.clone(), udm);
        let udm_flux = fvc::flux(&udm_field); // U_dm · S_f  [m³/s]

        // g = α(1−α), the drift/compression weighting (peaks at α = 0.5).
        let a = self.mixture.alpha().internal.as_slice();
        let g: Vec<f64> = (0..n).map(|c| (a[c] * (1.0 - a[c])).max(0.0)).collect();

        // Explicit, upwind drift flux φ_r = φ_dm · g_upwind on every face.
        let phi_r = upwind_drift_flux(&mesh, &udm_flux, &g);
        let div_drift = fvc::div_flux(&phi_r); // (1/V)Σ φ_r  [1/s]

        // ── Assemble  ∂α/∂t + ∇·(φ_m α) = −∇·(φ_dm α(1−α)) ────────────────────
        let alpha_old = self.mixture.alpha().clone();
        let mut eqn = fvm::ddt(self.mixture.alpha(), &alpha_old, self.dt)
            + fvm::div(&self.phi, self.mixture.alpha());
        let dd = div_drift.internal.as_slice();
        for c in 0..n {
            // Move the explicit +∇·(drift) term to the RHS: source −= V·∇·drift.
            eqn.source[c] -= mesh.cell_volumes[c] * dd[c];
        }

        // ── Solve and bound to [0,1] (internal values only) ──────────────────
        let (sol, _perf) = eqn.solve("alpha", SolverSettings::default());
        let new_vals = sol.internal.as_slice();
        let alpha = self.mixture.alpha_mut();
        let internal = alpha.internal.as_mut_slice();
        for c in 0..n {
            if !new_vals[c].is_finite() {
                return Err(MultiphaseError::Solver(format!(
                    "alpha became non-finite in cell {c}"
                )));
            }
            internal[c] = new_vals[c].clamp(0.0, 1.0);
        }
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

/// Build a `VolVectorField` from per-cell values with zero-gradient boundaries.
fn vector_field(name: &str, mesh: Arc<FvMesh>, vals: Vec<Vector3>) -> VolVectorField {
    let boundary = mesh
        .patches
        .iter()
        .map(|p| PatchField::zero_gradient_vec(p.size))
        .collect();
    VolVectorField::new(name, mesh, Field::new(vals), boundary)
}

/// Upwind drift face flux `φ_r[f] = φ_dm[f] · g_upwind[f]`, where `g = α(1−α)`
/// is taken from the upstream cell (owner if `φ_dm ≥ 0`, else neighbour).
///
/// Boundary faces use the owner cell's `g` (a zero-gradient stand-in). Since
/// `g = α(1−α) = 0` at `α = 0` or `α = 1`, a `FixedValue` boundary at either
/// saturation contributes no spurious drift flux there.
fn upwind_drift_flux(
    mesh: &FvMesh,
    udm_flux: &SurfaceScalarField,
    g: &[f64],
) -> SurfaceScalarField {
    let internal = Field::from_fn(mesh.n_internal_faces, |f| {
        let uf = udm_flux.internal[f];
        let g_up = if uf >= 0.0 {
            g[mesh.owner[f]]
        } else {
            g[mesh.neighbour[f]]
        };
        uf * g_up
    });
    let boundary = mesh
        .patches
        .iter()
        .enumerate()
        .map(|(pi, patch)| {
            let values = Field::from_fn(patch.size, |fi| {
                let gf = patch.start + fi;
                udm_flux.boundary[pi].values[fi] * g[mesh.owner[gf]]
            });
            PatchField {
                bc: BoundaryCondition::ZeroGradient,
                values,
            }
        })
        .collect();
    SurfaceScalarField::new("phiR", udm_flux.mesh.clone(), internal, boundary)
}

// ═════════════════════════════════════════════════════════════════════════════
// Tests — verification (formula exactness, hand-computed closures, advection
// boundedness). NOT validation: no benchmark/experimental comparison here.
// ═════════════════════════════════════════════════════════════════════════════
#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;
    use outram_foam_basic_lib::prelude::{BoundaryPatch, FvMeshBuilder, PatchKind};

    fn vx(x: f64) -> Vector3 {
        Vector3::new(x, 0.0, 0.0)
    }

    fn rho(v: f64) -> MassDensity {
        MassDensity::new::<kilogram_per_cubic_meter>(v)
    }

    fn mu(v: f64) -> DynamicViscosity {
        DynamicViscosity::new::<pascal_second>(v)
    }

    /// Single-cell mesh with two 1-face boundary patches (no internal faces) —
    /// enough to exercise the algebraic property/closure formulas.
    fn one_cell_mesh() -> Arc<FvMesh> {
        Arc::new(
            FvMeshBuilder::new()
                .n_cells(1)
                .n_internal_faces(0)
                .owner(vec![0, 0])
                .neighbour(vec![])
                .patches(vec![
                    BoundaryPatch::new("left", 0, 1, PatchKind::Patch),
                    BoundaryPatch::new("right", 1, 1, PatchKind::Patch),
                ])
                .cell_volumes(vec![1.0])
                .cell_centres(vec![vx(0.5)])
                .face_area_vectors(vec![vx(-1.0), vx(1.0)])
                .face_centres(vec![vx(0.0), vx(1.0)])
                .build()
                .unwrap(),
        )
    }

    /// 4-cell line mesh along x (centres 0.5,1.5,2.5,3.5; unit volumes/areas).
    /// Face 0..2 internal (owner i, neighbour i+1); face 3 = inlet at x=0
    /// (outward normal −x); face 4 = outlet at x=4 (outward normal +x).
    fn line_mesh_4() -> Arc<FvMesh> {
        Arc::new(
            FvMeshBuilder::new()
                .n_cells(4)
                .n_internal_faces(3)
                .owner(vec![0, 1, 2, 0, 3])
                .neighbour(vec![1, 2, 3])
                .patches(vec![
                    BoundaryPatch::new("inlet", 3, 1, PatchKind::Patch),
                    BoundaryPatch::new("outlet", 4, 1, PatchKind::Patch),
                ])
                .cell_volumes(vec![1.0, 1.0, 1.0, 1.0])
                .cell_centres(vec![vx(0.5), vx(1.5), vx(2.5), vx(3.5)])
                .face_area_vectors(vec![vx(1.0), vx(1.0), vx(1.0), vx(-1.0), vx(1.0)])
                .face_centres(vec![vx(1.0), vx(2.0), vx(3.0), vx(0.0), vx(4.0)])
                .build()
                .unwrap(),
        )
    }

    /// Build a prescribed uniform-`U0` mixture flux on `line_mesh_4`:
    /// internal φ = U0 (flow +x); inlet φ = −U0 (inflow); outlet φ = +U0.
    fn uniform_flux(mesh: Arc<FvMesh>, u0: f64) -> SurfaceScalarField {
        let internal = Field::new(vec![u0, u0, u0]);
        let boundary = vec![
            PatchField {
                bc: BoundaryCondition::ZeroGradient,
                values: Field::new(vec![-u0]), // inlet: U·(−x̂) = −U0
            },
            PatchField {
                bc: BoundaryCondition::ZeroGradient,
                values: Field::new(vec![u0]), // outlet: U·(+x̂) = +U0
            },
        ];
        SurfaceScalarField::new("phi", mesh, internal, boundary)
    }

    // ── Mixture-property exactness ───────────────────────────────────────────

    /// Methodology: set ρ_d = 1.2, ρ_c = 1000 kg/m³, α = 0.4 on a 1-cell mesh;
    /// the ported rule (incompressibleDriftFluxMixture.C:89) gives
    /// ρ_m = 0.4·1.2 + 0.6·1000 = 600.48 kg/m³. Pass: |computed − 600.48| within
    /// 1e-9 relative.
    /// Result (2026-07-23): ρ_m = 600.48 kg/m³ exactly (rel err < 1e-15). PASS.
    #[test]
    fn mixture_density_formula_is_exact() {
        let m = one_cell_mesh();
        let mut mix =
            DriftFluxMixture::new(m, rho(1.2), rho(1000.0), mu(1e-5), mu(1e-3), 0.0).unwrap();
        mix.alpha_mut().internal = Field::new(vec![0.4]);
        let rho_m = mix.rho_mixture();
        assert_relative_eq!(rho_m.internal[0], 600.48, max_relative = 1e-9);
    }

    /// Methodology: μ_d = 1e-5, μ_c = 1e-3 Pa·s, α = 0.4; volume-weighted linear
    /// rule gives μ_m = 0.4·1e-5 + 0.6·1e-3 = 6.04e-4 Pa·s. Pass: rel err < 1e-9.
    /// Result (2026-07-23): μ_m = 6.04e-4 Pa·s (rel err < 1e-15). PASS.
    #[test]
    fn mixture_viscosity_formula_is_exact() {
        let m = one_cell_mesh();
        let mut mix =
            DriftFluxMixture::new(m, rho(1.2), rho(1000.0), mu(1e-5), mu(1e-3), 0.0).unwrap();
        mix.alpha_mut().internal = Field::new(vec![0.4]);
        let mu_m = mix.mu_mixture();
        assert_relative_eq!(mu_m.internal[0], 6.04e-4, max_relative = 1e-9);
    }

    /// Limits: μ_m(α=0)=μ_c and μ_m(α=1)=μ_d; ρ_m(α=0)=ρ_c and ρ_m(α=1)=ρ_d.
    /// Result (2026-07-23): all four limits reproduced exactly. PASS.
    #[test]
    fn mixture_property_limits() {
        let m = one_cell_mesh();
        let mut mix =
            DriftFluxMixture::new(m, rho(1.2), rho(1000.0), mu(1e-5), mu(1e-3), 0.0).unwrap();
        mix.alpha_mut().internal = Field::new(vec![0.0]);
        assert_relative_eq!(mix.rho_mixture().internal[0], 1000.0, max_relative = 1e-12);
        assert_relative_eq!(mix.mu_mixture().internal[0], 1e-3, max_relative = 1e-12);
        mix.alpha_mut().internal = Field::new(vec![1.0]);
        assert_relative_eq!(mix.rho_mixture().internal[0], 1.2, max_relative = 1e-12);
        assert_relative_eq!(mix.mu_mixture().internal[0], 1e-5, max_relative = 1e-12);
    }

    /// Constructor rejects non-physical inputs.
    #[test]
    fn constructor_rejects_bad_inputs() {
        let m = one_cell_mesh();
        assert!(
            DriftFluxMixture::new(m.clone(), rho(0.0), rho(1.0), mu(0.0), mu(0.0), 0.0).is_err()
        );
        assert!(
            DriftFluxMixture::new(m.clone(), rho(1.0), rho(1.0), mu(0.0), mu(0.0), 1.5).is_err()
        );
    }

    // ── Slip-model drift velocities vs hand computation ──────────────────────

    /// Methodology: Zuber-Findlay U_dm = (C₀−1)·U_m + V_gj. With C₀ = 1.2,
    /// V_gj = (0.25,0,0) m/s, U_m = (2,0,0) m/s: U_dm_x = 0.2·2 + 0.25 = 0.65 m/s.
    /// Pass: |U_dm_x − 0.65| rel < 1e-12, transverse components 0.
    /// Result (2026-07-23): U_dm = (0.65, 0, 0) m/s. PASS.
    #[test]
    fn zuber_findlay_drift_velocity() {
        let m = one_cell_mesh();
        let mix = DriftFluxMixture::new(m.clone(), rho(1.2), rho(1000.0), mu(1e-5), mu(1e-3), 0.3)
            .unwrap();
        let u_m = VolVectorField::uniform("U", m.clone(), vx(2.0));
        let slip = SlipModel::ZuberFindlay {
            c0: 1.2,
            vgj: vx(0.25),
        };
        let udm = slip.drift_velocity(&u_m, mix.alpha()).unwrap();
        assert_relative_eq!(udm[0].x, 0.65, max_relative = 1e-12);
        assert_eq!(udm[0].y, 0.0);
        assert_eq!(udm[0].z, 0.0);
    }

    /// Methodology: terminal-velocity closure U_dm = u_t·10^{−a·α}. With
    /// u_t = (0.1,0,0) m/s, a = 2, α = 0.3: factor = 10^{−0.6} = 0.251188643…,
    /// U_dm_x = 0.1·0.251188643 = 0.0251188643 m/s. Pass: rel err < 1e-12.
    /// Result (2026-07-23): U_dm_x = 0.0251188643150958 m/s. PASS.
    #[test]
    fn terminal_velocity_hindered_settling() {
        let m = one_cell_mesh();
        let mut mix =
            DriftFluxMixture::new(m.clone(), rho(1.2), rho(1000.0), mu(1e-5), mu(1e-3), 0.3)
                .unwrap();
        mix.alpha_mut().internal = Field::new(vec![0.3]);
        let u_m = VolVectorField::zero("U", m.clone());
        let slip = SlipModel::TerminalVelocity {
            u_t: vx(0.1),
            hindrance_exp: 2.0,
        };
        let udm = slip.drift_velocity(&u_m, mix.alpha()).unwrap();
        let expected = 0.1 * 10.0_f64.powf(-0.6);
        assert_relative_eq!(udm[0].x, expected, max_relative = 1e-12);
    }

    /// UserDefined returns the supplied field verbatim; wrong length errors.
    /// Result (2026-07-23): field echoed exactly; length mismatch → Err. PASS.
    #[test]
    fn user_defined_drift_velocity() {
        let m = line_mesh_4();
        let mix = DriftFluxMixture::new(m.clone(), rho(1.2), rho(1000.0), mu(1e-5), mu(1e-3), 0.2)
            .unwrap();
        let u_m = VolVectorField::zero("U", m.clone());
        let field = vec![vx(1.0), vx(2.0), vx(3.0), vx(4.0)];
        let slip = SlipModel::UserDefined(field.clone());
        let udm = slip.drift_velocity(&u_m, mix.alpha()).unwrap();
        assert_eq!(udm, field);
        // Wrong length is rejected.
        let bad = SlipModel::UserDefined(vec![vx(1.0)]);
        assert!(bad.drift_velocity(&u_m, mix.alpha()).is_err());
    }

    // ── Void-fraction transport ──────────────────────────────────────────────

    /// Methodology (consistency/conservation): with zero mixture flux and zero
    /// drift, ∂α/∂t = 0, so α must be unchanged after a step. Uniform α = 0.3 on
    /// the 4-cell line, φ = 0, slip = UserDefined(0). Pass: every cell stays
    /// 0.3 within 1e-12.
    /// Result (2026-07-23): α = [0.3,0.3,0.3,0.3] unchanged (err < 1e-13). PASS.
    #[test]
    fn alpha_unchanged_with_no_flow_no_drift() {
        let m = line_mesh_4();
        let mix = DriftFluxMixture::new(m.clone(), rho(1.2), rho(1000.0), mu(1e-5), mu(1e-3), 0.3)
            .unwrap();
        let mut df = DriftFlux::new(mix, SlipModel::UserDefined(vec![Vector3::ZERO; 4]));
        // phi already zero from DriftFlux::new; dt arbitrary.
        df.dt = 0.5;
        df.advance_alpha().unwrap();
        for c in 0..4 {
            assert_relative_eq!(df.mixture.alpha().internal[c], 0.3, max_relative = 1e-12);
        }
    }

    /// Methodology (advection + boundedness): impose a uniform +x mixture flux
    /// (U0 = 1 m/s) with a saturated inlet α = 1 feeding an initially empty
    /// domain (α = 0). No drift. Courant U0·dt/dx = 0.2. After 5 steps the
    /// dispersed phase must advect downstream: α monotonically non-increasing
    /// with x, cell 0 has risen above 0, and every cell stays in [0,1].
    /// Result (2026-07-23): α = [0.5981, 0.2632, 0.0958, 0.0307] (5 steps);
    /// monotone-decreasing, cell0 > 0, all ∈ [0,1]. PASS.
    #[test]
    fn alpha_advects_downstream_and_stays_bounded() {
        let m = line_mesh_4();
        let mut mix =
            DriftFluxMixture::new(m.clone(), rho(1.2), rho(1000.0), mu(1e-5), mu(1e-3), 0.0)
                .unwrap();
        // Saturated inlet: FixedValue α = 1 on the "inlet" patch (index 0).
        mix.alpha_mut().boundary[0] = PatchField::fixed_value(1, 1.0);
        let mut df = DriftFlux::new(mix, SlipModel::UserDefined(vec![Vector3::ZERO; 4]));
        df.phi = uniform_flux(m.clone(), 1.0);
        df.u_m = VolVectorField::uniform("U", m.clone(), vx(1.0));
        df.dt = 0.2;
        for _ in 0..5 {
            df.advance_alpha().unwrap();
        }
        let a: Vec<f64> = (0..4).map(|c| df.mixture.alpha().internal[c]).collect();
        for (c, &v) in a.iter().enumerate() {
            assert!((0.0..=1.0).contains(&v), "cell {c} alpha {v} out of [0,1]");
        }
        assert!(a[0] > 0.0, "upstream cell should have filled");
        for c in 0..3 {
            assert!(
                a[c] >= a[c + 1] - 1e-12,
                "alpha must be monotone non-increasing downstream: {a:?}"
            );
        }
    }

    /// Methodology (bounding stress test): a large drift velocity that, with a
    /// large dt, would drive α past 1 in the assembled linear solve; the
    /// post-solve clamp must keep every cell in [0,1]. Start α = 0.5 uniform,
    /// UserDefined drift = 100 m/s, dt = 10. Pass: all α ∈ [0,1] and finite.
    /// Result (2026-07-23): all α ∈ [0,1], finite. PASS.
    #[test]
    fn alpha_clamped_under_extreme_drift() {
        let m = line_mesh_4();
        let mix = DriftFluxMixture::new(m.clone(), rho(1.2), rho(1000.0), mu(1e-5), mu(1e-3), 0.5)
            .unwrap();
        let big = vec![vx(100.0); 4];
        let mut df = DriftFlux::new(mix, SlipModel::UserDefined(big));
        df.u_m = VolVectorField::uniform("U", m.clone(), vx(1.0));
        df.dt = 10.0;
        df.advance_alpha().unwrap();
        for c in 0..4 {
            let v = df.mixture.alpha().internal[c];
            assert!(v.is_finite() && (0.0..=1.0).contains(&v), "cell {c}: {v}");
        }
    }
}
