// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Closures traced back to the OUTRAM-FOAM 3-D multiphase reference
// (`outram-foam-multiphase`, epic op-2kk), which itself derives from
// OpenFOAM's `incompressibleDriftFlux` / `relativeVelocityModels`.
// Copyright (C) 2004-2023 OpenFOAM Foundation, (C) 2016-2023 OpenCFD Ltd.
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! The 1-D **drift-flux** solver — four equations, algebraic slip.
//!
//! # The model, in one paragraph
//!
//! Drift flux is the first rung above HEM. It keeps the two phases in *thermal*
//! equilibrium (one temperature, both on the saturation line) but lets them
//! move at *different velocities*, with the difference given by an algebraic
//! closure rather than by a second momentum equation. That buys the two things
//! HEM cannot represent — vapour rising through slower liquid, and the void
//! fraction consequently differing from the no-slip value at the same quality —
//! for one extra transported field.
//!
//! # The four equations
//!
//! On a pipe of constant area `A`, with `ρ_m` the mixture density, `u_m` the
//! mass-averaged (centre-of-mass) velocity and `h_m` the mixture enthalpy:
//!
//! `∂ρ_m/∂t + ∂(ρ_m u_m)/∂x = 0`  (mixture mass)
//!
//! `∂(ρ_m u_m)/∂t + ∂(ρ_m u_m²)/∂x = −∂p/∂x + ρ_m g_x − F_wall − ∂Φ/∂x`  (mixture momentum)
//!
//! `∂(ρ_m h_m)/∂t + ∂(ρ_m u_m h_m)/∂x = ∂p/∂t`  (mixture energy)
//!
//! `∂(α ρ_g)/∂t + ∂(α ρ_g v_g)/∂x = Γ_g`  (gas mass)
//!
//! The momentum equation's `Φ` is the **drift momentum flux**, the term that
//! distinguishes drift flux from HEM-with-a-void-equation:
//!
//! `Φ = α ρ_g ρ_l U_dm² / ((1−α) ρ_m)`
//!
//! It is the extra momentum carried by the phases moving at different speeds.
//! Dropping it — as a naive "HEM plus a void equation" would — leaves the
//! momentum equation inconsistent with the velocity field the void equation is
//! using.
//!
//! # How the slip closure is traced back
//!
//! [`outram_foam_multiphase::drift_flux::SlipModel`] supplies `U_dm`, the
//! dispersed-phase velocity relative to the **mixture volumetric flux** `j`.
//! From it the phase velocities are reconstructed *exactly*, not
//! approximately. Starting from `ρ_m u_m = α ρ_g v_g + (1−α) ρ_l v_l`,
//! `j = α v_g + (1−α) v_l` and `U_dm = v_g − j`, eliminating `v_l` gives
//!
//! `ρ_m u_m = j ρ_m + α U_dm (ρ_g − ρ_l)`
//!
//! hence
//!
//! `j = u_m + α U_dm (ρ_l − ρ_g) / ρ_m`,  `v_g = j + U_dm`,
//! `v_l = j − α U_dm / (1−α)`.
//!
//! **One inherited approximation, stated plainly.** The reference's
//! `ZuberFindlay` arm computes `U_dm = (C₀ − 1) u_m + V_gj`, i.e. it uses the
//! mixture *velocity* `u_m` where the correlation calls for the volumetric
//! *flux* `j` — its own doc comment says so ("the volumetric-flux surrogate").
//! Since `j ≠ u_m` whenever the phase densities differ, this port inherits the
//! approximation rather than silently correcting it: correcting a closure
//! inside a consumer is how two codes quietly stop agreeing. The
//! reconstruction above is exact; only `U_dm` itself carries the surrogate.
//!
//! # The interfacial mass transfer `Γ_g`
//!
//! A four-equation model has one energy equation, so it cannot carry
//! independent phase enthalpies — the phases are in thermal equilibrium by
//! construction. `Γ_g` is therefore not free: it is whatever makes the
//! transported void consistent with the equilibrium void implied by
//! `(p, h_m)`. This solver relaxes toward that,
//!
//! `Γ_g = (α_eq − α) ρ_g / τ`,
//!
//! with `τ` the [`vapour_relaxation_time`](DriftFlux1d::vapour_relaxation_time).
//! As `τ → 0` the model collapses to HEM-with-slip (void pinned to
//! equilibrium); at finite `τ` the void lags, which is the physically real
//! delay of a flashing front. `τ` is a **model parameter, not a measured
//! constant** — it is exposed, defaulted, and documented as such rather than
//! buried in a literal.
//!
//! # Honest scope
//!
//! [`crate::multiphase_1d`] lists what applies to both solvers (no wall heat
//! transfer, no flow-regime map, no interfacial-area transport, HEM break
//! model). Specific to this solver:
//!
//! - **The drift momentum flux is explicit**, at old-time `U_dm`. At a fast
//!   front it is a lagged term.
//! - **`α` outside `[0, 1]` is refused, not clipped.** Clipping would present
//!   a CFL violation as a plausible answer.

use outram_foam_basic_lib::fields::{VolScalarField, VolVectorField};
use outram_foam_basic_lib::interface::one_dimensional_meshing::create_one_d_mesh;
use outram_foam_basic_lib::mesh::FvMesh;
use outram_foam_basic_lib::primitives::Vector3;
use outram_foam_multiphase::drift_flux::SlipModel;

use std::sync::Arc;

use uom::si::f64::{Length, Mass, MassRate, Pressure, ThermodynamicTemperature, Time, Velocity};
use uom::si::mass::kilogram;
use uom::si::mass_rate::kilogram_per_second;
use uom::si::pressure::pascal;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;
use uom::si::velocity::meter_per_second;

use super::geometry::Pipe1d;
use super::properties::{SaturatedProperties, TwoPhaseState};
use super::thomas_solve;
use crate::TampinesError;

/// Default vapour-generation relaxation time `τ` \[s\].
///
/// `1e-3` s — the order of the acoustic transit time of one cell in a blowdown
/// mesh, so the void tracks the pressure without a flashing front being
/// instantaneous. It is a **model parameter with no measured provenance**; a
/// case whose answer depends sensitively on it is a case whose result should
/// be reported *with* that sensitivity, not one that has found the right
/// value.
pub const DEFAULT_VAPOUR_RELAXATION_TIME: f64 = 1.0e-3;

/// Relative finite-difference step used for the compressibility `∂ρ/∂p|_h`.
pub const COMPRESSIBILITY_STEP: f64 = 1.0e-4;

/// Default number of SIMPLE-style outer correctors per step.
///
/// `8`. See [`DriftFlux1d::set_outer_correctors`] for why one is not enough
/// when a subcooled cell has to cross the saturation line within a step.
pub const DEFAULT_OUTER_CORRECTORS: usize = 8;

/// Default pressure under-relaxation `α_p`.
///
/// `0.7` — enough damping that the first corrector's large excursion does not
/// overshoot into a region the next linearisation cannot recover from, without
/// slowing convergence to the point of needing many more correctors.
pub const DEFAULT_PRESSURE_UNDER_RELAXATION: f64 = 0.7;

/// Default outer-corrector convergence tolerance on `max |Δp|` \[Pa\].
///
/// `1.0` Pa — six orders below the 7 MPa initial pressure of a blowdown, and
/// far below any pressure difference a gauge comparison resolves.
pub const DEFAULT_OUTER_TOLERANCE: f64 = 1.0;

/// What closes an end of the pipe.
///
/// Enum dispatch per the workspace rule: the set of 1-D end conditions is
/// closed, and adding one must force every `match` to be revisited.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AxialBoundary {
    /// A rigid closed end: `u = 0`, zero-gradient pressure. The `x = 0` end of
    /// the Edwards–O'Brien pipe.
    Closed,

    /// A prescribed face velocity \[m/s\], positive in `+x`, with
    /// zero-gradient pressure.
    PrescribedVelocity(f64),

    /// A **choked (critical) outlet** through a break of area
    /// `area_fraction × A`.
    ///
    /// Each step the adjacent cell's `(p, h)` is handed to the crate's
    /// existing HEM critical-flow dispatcher for the throat mass flux `G*`,
    /// and the equivalent full-face velocity `u = G* × area_fraction / ρ` is
    /// imposed. The pressure keeps a zero-gradient condition, so it floats on
    /// the compressibility diagonal — a blowdown has no Dirichlet pressure
    /// anywhere; the mass depletion sets `p`.
    ///
    /// **This is a modelling inconsistency and it is deliberate.** The break
    /// model is HEM even though the pipe is drift-flux, because the
    /// critical-flow dispatcher is the piece of this crate actually exercised
    /// against Moody / Zaloudek / Marviken. Substituting an unvalidated
    /// drift-flux choking model would trade a known inconsistency for an
    /// unknown one. It is called out here, in the module docs, and in the
    /// Edwards case, so it cannot be mistaken for an oversight.
    ChokedOutlet {
        /// Break area as a fraction of the pipe flow area, in `(0, 1]`.
        area_fraction: f64,
        /// Ambient / containment back-pressure \[Pa\]. The outlet unchokes
        /// once the critical throat pressure falls below it.
        ambient_pressure: f64,
    },
}

/// Per-step diagnostics from [`DriftFlux1d::step`].
///
/// Returned rather than logged, so a case can assert on it and a V&V test can
/// record measured values.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DriftFluxReport {
    /// Simulated time at the end of the step \[s\].
    pub time: f64,
    /// Mass flow through the right-hand boundary \[kg/s\], positive outward.
    pub outlet_mass_flow: f64,
    /// Whether either boundary was choked this step.
    pub outlet_choked: bool,
    /// Total mass currently in the pipe \[kg\].
    pub inventory: f64,
    /// Largest void fraction anywhere \[-\].
    pub max_void_fraction: f64,
    /// Largest material Courant number `|u| Δt / Δx` \[-\].
    ///
    /// The stability figure that matters for the explicit transport. The
    /// *acoustic* Courant number is not limiting, because the pressure solve
    /// is implicit — that is the whole point of the semi-implicit method. A
    /// value above 1 means donor-cell transport has stepped past a whole cell
    /// and the answer is not to be trusted.
    pub max_courant: f64,
}

/// A 1-D transient drift-flux solver for compressible steam/water pipe flow.
///
/// # Layout
///
/// **Staggered**: scalars (`p`, `h`, `α`, `ρ`) at cell centres, velocities at
/// faces. Staggering rather than collocation because in 1-D it removes
/// pressure-velocity checkerboarding *by construction* — no Rhie-Chow
/// interpolation needed — and it is what the reference system codes do. Face
/// `j` is the left face of cell `j`; faces `0` and `n_cells` are the ends.
///
/// # Units
///
/// Constructors and accessors are `uom`-typed. Internal state is raw `f64` in
/// strict SI: pascal, `J/kg`, `kg/m³`, `m/s`, `[-]`.
pub struct DriftFlux1d {
    pipe: Pipe1d,
    /// Held so the traced-back closures — which take field arguments — can be
    /// evaluated on the same 1-D mesh.
    mesh: Arc<FvMesh>,
    slip: SlipModel,

    /// Cell pressure \[Pa\], length `n_cells`.
    p: Vec<f64>,
    /// Cell mixture specific enthalpy \[J/kg\], length `n_cells`.
    h: Vec<f64>,
    /// Cell vapour volume fraction `α` \[-\], length `n_cells`.
    alpha: Vec<f64>,
    /// Cell mixture density \[kg/m³\], length `n_cells`.
    rho: Vec<f64>,
    /// Per-cell saturated-property cache.
    sat: Vec<SaturatedProperties>,
    /// Face velocity \[m/s\], length `n_cells + 1`.
    u: Vec<f64>,

    dt: f64,
    time: f64,
    tau: f64,
    left: AxialBoundary,
    right: AxialBoundary,

    /// SIMPLE-style outer correctors per step, each re-linearising the
    /// compressibility at the latest pressure iterate.
    ///
    /// Mirrors `TampinesSteamArray::n_outer_correctors` in
    /// `tampines-steam-tables`, deliberately: the HEM array already solved this
    /// problem and its vocabulary is reused rather than a parallel one
    /// invented. Defaults to 8 rather than the array's 1 because a *subcooled*
    /// initial state has to cross the saturation line within a single step,
    /// which is the stiffest case the array's default never meets.
    n_outer_correctors: usize,
    /// Explicit pressure under-relaxation `α_p ∈ (0, 1]`, applied once per
    /// outer corrector: `p ← p_prev + α_p (p_solved − p_prev)`.
    p_under_relaxation: f64,
    /// Outer-corrector convergence tolerance on `max |Δp|` \[Pa\]. Below this
    /// the loop stops early.
    outer_tolerance: f64,
}

impl DriftFlux1d {
    /// Build a solver on `pipe`, initialised to a uniform `(p, T)` state.
    ///
    /// # Arguments
    ///
    /// - `pipe` — geometry and mesh.
    /// - `slip` — the drift-velocity closure, taken from the 3-D reference.
    /// - `pressure` — uniform initial pressure.
    /// - `temperature` — uniform initial temperature, flashed through IF97 for
    ///   the enthalpy, so a subcooled initial state really is subcooled rather
    ///   than nominally so.
    /// - `dt` — fixed timestep.
    ///
    /// # Errors
    ///
    /// [`TampinesError::Unphysical`] if the initial state is outside IF97's
    /// range; [`TampinesError::InvalidInput`] for a non-positive timestep or a
    /// mesh that will not build.
    pub fn new(
        pipe: Pipe1d,
        slip: SlipModel,
        pressure: Pressure,
        temperature: ThermodynamicTemperature,
        dt: Time,
    ) -> Result<Self, TampinesError> {
        let dt_s = dt.get::<second>();
        if !(dt_s > 0.0) {
            return Err(TampinesError::InvalidInput(format!(
                "timestep must be > 0 s (got {dt_s})"
            )));
        }
        let n = pipe.n_cells();
        let p0 = pressure.get::<pascal>();
        let sat0 = SaturatedProperties::at(p0)?;
        let h0 = enthalpy_at_pt(p0, temperature.get::<kelvin>(), sat0)?;
        let state0 = TwoPhaseState::flash(p0, h0, sat0)?;

        let mesh = Arc::new(
            create_one_d_mesh(pipe.length(), pipe.flow_area(), n as i64).map_err(|e| {
                TampinesError::InvalidInput(format!("1-D mesh construction failed: {e}"))
            })?,
        );

        Ok(Self {
            pipe,
            mesh,
            slip,
            p: vec![p0; n],
            h: vec![h0; n],
            alpha: vec![state0.void_fraction; n],
            rho: vec![state0.density; n],
            sat: vec![sat0; n],
            u: vec![0.0; n + 1],
            dt: dt_s,
            time: 0.0,
            tau: DEFAULT_VAPOUR_RELAXATION_TIME,
            left: AxialBoundary::Closed,
            right: AxialBoundary::Closed,
            n_outer_correctors: DEFAULT_OUTER_CORRECTORS,
            p_under_relaxation: DEFAULT_PRESSURE_UNDER_RELAXATION,
            outer_tolerance: DEFAULT_OUTER_TOLERANCE,
        })
    }

    /// Overwrite the cell temperatures from an axial profile, re-flashing
    /// enthalpy, void and density at each cell.
    ///
    /// Needed by any case whose initial condition is not isothermal. For
    /// Edwards–O'Brien the Hendrie non-isothermal profile is the single most
    /// important modelling detail, so this is not a convenience.
    ///
    /// # Errors
    ///
    /// [`TampinesError::InvalidInput`] if the slice is not `n_cells` long;
    /// [`TampinesError::Unphysical`] if a flash leaves IF97's range.
    pub fn set_temperature_profile(
        &mut self,
        temperatures: &[ThermodynamicTemperature],
    ) -> Result<(), TampinesError> {
        if temperatures.len() != self.pipe.n_cells() {
            return Err(TampinesError::InvalidInput(format!(
                "temperature profile has {} entries, pipe has {} cells",
                temperatures.len(),
                self.pipe.n_cells()
            )));
        }
        for i in 0..self.pipe.n_cells() {
            let t = temperatures[i].get::<kelvin>();
            self.sat[i] = SaturatedProperties::at(self.p[i])?;
            self.h[i] = enthalpy_at_pt(self.p[i], t, self.sat[i])?;
            let state = TwoPhaseState::flash(self.p[i], self.h[i], self.sat[i])?;
            self.alpha[i] = state.void_fraction;
            self.rho[i] = state.density;
        }
        Ok(())
    }

    /// Set the boundary condition at the `x = 0` end.
    pub fn set_left_boundary(&mut self, bc: AxialBoundary) {
        self.left = bc;
    }

    /// Set the boundary condition at the `x = L` end.
    pub fn set_right_boundary(&mut self, bc: AxialBoundary) {
        self.right = bc;
    }

    /// The vapour-generation relaxation time `τ` \[s\]. A model parameter —
    /// see the module docs.
    #[must_use]
    pub fn vapour_relaxation_time(&self) -> Time {
        Time::new::<second>(self.tau)
    }

    /// Set the vapour-generation relaxation time `τ`.
    ///
    /// # Errors
    ///
    /// [`TampinesError::InvalidInput`] if `tau` is not strictly positive.
    pub fn set_vapour_relaxation_time(&mut self, tau: Time) -> Result<(), TampinesError> {
        let tau_s = tau.get::<second>();
        if !(tau_s > 0.0) {
            return Err(TampinesError::InvalidInput(format!(
                "vapour relaxation time must be > 0 s (got {tau_s})"
            )));
        }
        self.tau = tau_s;
        Ok(())
    }

    /// Set the number of SIMPLE-style outer correctors per step.
    ///
    /// # Why more than one is needed
    ///
    /// Each corrector re-linearises the cell compliance `ψ = ∂ρ/∂p|_h` at the
    /// latest pressure iterate. One corrector linearises about the *old*
    /// pressure only, and across the saturation line that is catastrophically
    /// wrong: measured on the Edwards initial state (2026-08-05, release, at
    /// fixed `h`), `ψ = 9.87980e-7 s²/m²` in subcooled liquid at 7 MPa against
    /// `1.30644e-3 s²/m²` two-phase at 2.6 MPa — a ratio of **1.3223e3**. A
    /// single step under-estimates the compliance by nearly three orders and
    /// the pressure shoots through the plateau; the first step of the Edwards
    /// case landed at `−381097.24 Pa`, which the property layer refused.
    ///
    /// # Errors
    ///
    /// [`TampinesError::InvalidInput`] if `n` is zero.
    pub fn set_outer_correctors(&mut self, n: usize) -> Result<(), TampinesError> {
        if n == 0 {
            return Err(TampinesError::InvalidInput(
                "at least one outer corrector is required".to_string(),
            ));
        }
        self.n_outer_correctors = n;
        Ok(())
    }

    /// The number of outer correctors per step.
    #[must_use]
    pub fn outer_correctors(&self) -> usize {
        self.n_outer_correctors
    }

    /// Set the pressure under-relaxation factor `α_p ∈ (0, 1]`.
    ///
    /// # Errors
    ///
    /// [`TampinesError::InvalidInput`] if outside `(0, 1]`.
    pub fn set_pressure_under_relaxation(
        &mut self,
        alpha: uom::si::f64::Ratio,
    ) -> Result<(), TampinesError> {
        let a = alpha.get::<uom::si::ratio::ratio>();
        if !(a > 0.0 && a <= 1.0) {
            return Err(TampinesError::InvalidInput(format!(
                "pressure under-relaxation must be in (0, 1] (got {a})"
            )));
        }
        self.p_under_relaxation = a;
        Ok(())
    }

    /// Elapsed simulated time.
    #[must_use]
    pub fn time(&self) -> Time {
        Time::new::<second>(self.time)
    }

    /// Cell pressures \[Pa\], read-only.
    #[must_use]
    pub fn pressure(&self) -> &[f64] {
        &self.p
    }

    /// Cell void fractions \[-\], read-only.
    #[must_use]
    pub fn void_fraction(&self) -> &[f64] {
        &self.alpha
    }

    /// Cell mixture densities \[kg/m³\], read-only.
    #[must_use]
    pub fn density(&self) -> &[f64] {
        &self.rho
    }

    /// Cell mixture specific enthalpies \[J/kg\], read-only.
    #[must_use]
    pub fn enthalpy(&self) -> &[f64] {
        &self.h
    }

    /// Face velocities \[m/s\], read-only, length `n_cells + 1`.
    #[must_use]
    pub fn face_velocity(&self) -> &[f64] {
        &self.u
    }

    /// The pipe geometry.
    #[must_use]
    pub fn pipe(&self) -> &Pipe1d {
        &self.pipe
    }

    /// Cell temperature \[K\] — `T_sat` inside the dome, the flashed value
    /// outside it.
    ///
    /// # Errors
    ///
    /// [`TampinesError::Unphysical`] if the cell state is outside IF97.
    pub fn temperature(&self, cell: usize) -> Result<ThermodynamicTemperature, TampinesError> {
        let state = TwoPhaseState::flash(self.p[cell], self.h[cell], self.sat[cell])?;
        Ok(ThermodynamicTemperature::new::<kelvin>(state.temperature))
    }

    /// Total mass currently in the pipe.
    #[must_use]
    pub fn inventory(&self) -> Mass {
        let v = self.pipe.cell_volume();
        Mass::new::<kilogram>(self.rho.iter().map(|r| r * v).sum::<f64>())
    }

    /// Mass flow through the right-hand boundary \[kg/s\], from the current
    /// state.
    #[must_use]
    pub fn outlet_mass_flow(&self) -> MassRate {
        let n = self.pipe.n_cells();
        MassRate::new::<kilogram_per_second>(self.rho[n - 1] * self.u[n] * self.pipe.area_si())
    }

    /// Phase velocities `(v_g, v_l)` \[m/s\] reconstructed at cell centres.
    ///
    /// The exact reconstruction of the module docs, from the mixture velocity
    /// and the traced-back `U_dm`. In a single-phase cell both come back equal
    /// to the mixture velocity, the correct degenerate limit.
    ///
    /// # Errors
    ///
    /// [`TampinesError::Closure`] if the slip closure rejects its inputs.
    pub fn phase_velocities(&self) -> Result<(Vec<f64>, Vec<f64>), TampinesError> {
        let n = self.pipe.n_cells();
        let u_m = self.cell_velocities();
        let u_dm = self.drift_velocity(&u_m)?;
        let mut v_g = vec![0.0; n];
        let mut v_l = vec![0.0; n];
        for i in 0..n {
            let a = self.alpha[i];
            let j = u_m[i] + a * u_dm[i] * (self.sat[i].rho_f - self.sat[i].rho_g) / self.rho[i];
            v_g[i] = j + u_dm[i];
            // As alpha -> 1 the liquid velocity is undefined; report the
            // mixture velocity rather than dividing by zero.
            v_l[i] = if a < 1.0 - 1.0e-9 {
                j - a * u_dm[i] / (1.0 - a)
            } else {
                u_m[i]
            };
        }
        Ok((v_g, v_l))
    }

    /// Advance one timestep.
    ///
    /// The four-stage semi-implicit march described in
    /// [`crate::multiphase_1d`]: explicit momentum predictor, implicit
    /// tridiagonal pressure solve, velocity correction, then explicit energy
    /// and void transport on the corrected mass fluxes.
    ///
    /// # Errors
    ///
    /// - [`TampinesError::Unphysical`] if any cell leaves IF97's validity
    ///   range or the void fraction leaves `[0, 1]`. **Refused, not clamped.**
    /// - [`TampinesError::Numerical`] if the pressure matrix is singular.
    /// - [`TampinesError::Closure`] if the slip closure rejects its inputs.
    pub fn step(&mut self) -> Result<DriftFluxReport, TampinesError> {
        let n = self.pipe.n_cells();
        let dx = self.pipe.dx();
        let area = self.pipe.area_si();
        let volume = self.pipe.cell_volume();
        let g_x = self.pipe.axial_gravity();

        let p_old = self.p.clone();
        let rho_old = self.rho.clone();
        let h_old = self.h.clone();
        let alpha_old = self.alpha.clone();
        let sat_old = self.sat.clone();

        // ── Stage 0: closures on the old-time state ──────────────────────────
        let u_m_cells = self.cell_velocities();
        let u_dm = self.drift_velocity(&u_m_cells)?;

        // ── Stage 1: explicit momentum predictor on interior faces ───────────
        let mut u_star = self.u.clone();
        for j in 1..n {
            let (cl, cr) = (j - 1, j);
            let rho_face = 0.5 * (rho_old[cl] + rho_old[cr]);
            let u_j = self.u[j];

            // Donor-cell convection of momentum.
            let du_dx = if u_j >= 0.0 {
                (u_j - self.u[j - 1]) / dx
            } else {
                (self.u[j + 1] - u_j) / dx
            };

            let mu_face = 0.5
                * (mixture_viscosity(alpha_old[cl], sat_old[cl])
                    + mixture_viscosity(alpha_old[cr], sat_old[cr]));
            let friction = wall_friction_acceleration(
                u_j,
                rho_face,
                mu_face,
                self.pipe.hydraulic_diameter_si(),
            );

            let phi_l = drift_momentum_flux(alpha_old[cl], sat_old[cl], rho_old[cl], u_dm[cl]);
            let phi_r = drift_momentum_flux(alpha_old[cr], sat_old[cr], rho_old[cr], u_dm[cr]);
            let d_phi_dx = (phi_r - phi_l) / dx;

            u_star[j] = u_j + self.dt * (-u_j * du_dx + g_x - friction - d_phi_dx / rho_face);
        }
        let (u_left_bc, left_choked) = self.boundary_velocity(self.left, 0)?;
        let (u_right_bc, right_choked) = self.boundary_velocity(self.right, n - 1)?;
        u_star[0] = u_left_bc;
        u_star[n] = u_right_bc;

        // ── Stage 2: implicit pressure equation, with outer correctors ───────
        //
        // WHY THIS IS ITERATED, and why a single Newton step is not enough.
        // Measured on the Edwards initial state (2026-08-05, release, h fixed
        // at the 7 MPa / 502 K value): psi = 9.87980e-7 s^2/m^2 in the
        // subcooled liquid at 7 MPa, but 1.30644e-3 s^2/m^2 two-phase at
        // 2.6 MPa -- a ratio of 1.3223e3. A single step that linearises about
        // the OLD subcooled pressure therefore under-estimates the cell's
        // compliance by nearly three orders of magnitude, and the pressure
        // shoots straight through the saturation plateau; the very first step
        // of the Edwards case landed at -381097.24 Pa, which
        // `SaturatedProperties::at` refused.
        //
        // The cure is to re-linearise: each outer corrector re-evaluates the
        // compliance at the latest pressure iterate. From the second corrector
        // on it uses the SECANT (rho(p_iter) - rho(p_old)) / (p_iter - p_old)
        // rather than the tangent, because the step crosses a phase boundary
        // and the secant is the correct linearisation of a finite step across
        // a kink -- a tangent taken on either side of the saturation line is
        // wrong about the other side no matter how small the step.
        let mut rho_face = vec![0.0; n + 1];
        let mut d_face = vec![0.0; n + 1];
        for j in 0..=n {
            rho_face[j] = if j == 0 {
                rho_old[0]
            } else if j == n {
                rho_old[n - 1]
            } else if u_star[j] >= 0.0 {
                rho_old[j - 1]
            } else {
                rho_old[j]
            };
            // Boundary faces carry a prescribed velocity, so they have no
            // pressure sensitivity; d = 0 keeps them out of the matrix.
            d_face[j] = if j == 0 || j == n {
                0.0
            } else {
                self.dt / (0.5 * (rho_old[j - 1] + rho_old[j]) * dx)
            };
        }

        let mut p_new = p_old.clone();
        let mut u_new = u_star.clone();
        for outer in 0..self.n_outer_correctors {
            let mut psi = vec![0.0; n];
            for i in 0..n {
                psi[i] = if outer == 0 {
                    let state = TwoPhaseState::flash(p_old[i], h_old[i], sat_old[i])?;
                    state.compressibility(COMPRESSIBILITY_STEP)?
                } else {
                    secant_compressibility(p_old[i], rho_old[i], p_new[i], h_old[i])?
                };
                // A non-positive compliance would make the matrix indefinite.
                // It cannot arise physically -- density rises with pressure at
                // fixed enthalpy everywhere in IF97 -- so a floor here is a
                // guard against round-off in the finite difference, not a
                // physical clamp.
                psi[i] = psi[i].max(1.0e-12);
            }

            let mut sub = vec![0.0; n];
            let mut diag = vec![0.0; n];
            let mut sup = vec![0.0; n];
            let mut rhs = vec![0.0; n];
            for i in 0..n {
                let (jl, jr) = (i, i + 1);
                let compliance = volume * psi[i] / self.dt;
                sub[i] = -area * rho_face[jl] * d_face[jl];
                sup[i] = -area * rho_face[jr] * d_face[jr];
                diag[i] = compliance
                    + area * rho_face[jr] * d_face[jr]
                    + area * rho_face[jl] * d_face[jl];
                rhs[i] = compliance * p_old[i]
                    - area * (rho_face[jr] * u_star[jr] - rho_face[jl] * u_star[jl]);
            }
            let p_solved = thomas_solve(&sub, &diag, &sup, &rhs)?;

            // Under-relax, and refuse to leave the IF97 range rather than
            // walking off it mid-iteration.
            let mut max_change: f64 = 0.0;
            for i in 0..n {
                let target = p_new[i] + self.p_under_relaxation * (p_solved[i] - p_new[i]);
                if !target.is_finite() {
                    return Err(TampinesError::Numerical(format!(
                        "cell {i}: pressure iterate is not finite at outer corrector {outer}"
                    )));
                }
                max_change = max_change.max((target - p_new[i]).abs());
                p_new[i] = target.clamp(
                    super::properties::P_MIN_IF97 * 1.000_001,
                    super::properties::P_MAX_IF97 * 0.999_999,
                );
            }

            // ── Stage 3: velocity correction, inside the loop ────────────────
            for j in 1..n {
                u_new[j] = u_star[j] - d_face[j] * (p_new[j] - p_new[j - 1]);
            }
            u_new[0] = u_left_bc;
            u_new[n] = u_right_bc;

            if max_change < self.outer_tolerance {
                break;
            }
        }

        let mass_flux: Vec<f64> = (0..=n).map(|j| rho_face[j] * u_new[j] * area).collect();

        // ── Stage 4: energy and void transport on the corrected fluxes ───────
        // The CONTINUITY density: rebuilt from the final mass flux so that
        // (rho_cont - rho_old)/dt = -div(phi) holds exactly. Only then does the
        // h*div(phi) term cancel against div(phi h), leaving rho Dh/Dt = dp/dt.
        // Using the EOS density here instead over-drains enthalpy during a
        // flash and drives the bulk liquid subcooled -- the defect the HEM
        // array hit (bead op-21g.14).
        let mut rho_cont = vec![0.0; n];
        for i in 0..n {
            rho_cont[i] = rho_old[i] - self.dt / volume * (mass_flux[i + 1] - mass_flux[i]);
            if !(rho_cont[i] > 0.0) {
                return Err(TampinesError::Unphysical(format!(
                    "cell {i}: continuity density {} kg/m^3 is not positive at t = {} s \
                     -- the material Courant number is probably above 1",
                    rho_cont[i], self.time
                )));
            }
        }

        let mut h_new = vec![0.0; n];
        for i in 0..n {
            let h_left = upwind_scalar(&h_old, mass_flux[i], i.checked_sub(1), i);
            let h_right = upwind_scalar(&h_old, mass_flux[i + 1], Some(i), (i + 1).min(n - 1));
            let convection = (mass_flux[i + 1] * h_right - mass_flux[i] * h_left) / volume;
            let dp_dt = (p_new[i] - p_old[i]) / self.dt;
            h_new[i] =
                (rho_old[i] * h_old[i] / self.dt - convection + dp_dt) * self.dt / rho_cont[i];
        }

        // Gas mass transport with the reconstructed vapour velocity, then
        // relaxation toward the equilibrium void implied by the new (p, h).
        let v_g_cells: Vec<f64> = (0..n)
            .map(|i| gas_velocity(alpha_old[i], sat_old[i], rho_old[i], u_m_cells[i], u_dm[i]))
            .collect();

        let mut alpha_new = vec![0.0; n];
        for i in 0..n {
            let sat_new = SaturatedProperties::at(p_new[i])?;
            let state = TwoPhaseState::flash(p_new[i], h_new[i], sat_new)?;

            let a_rho_g_old = alpha_old[i] * sat_old[i].rho_g;
            let face_left = if i == 0 {
                v_g_cells[0]
            } else {
                0.5 * (v_g_cells[i - 1] + v_g_cells[i])
            };
            let face_right = if i + 1 >= n {
                v_g_cells[n - 1]
            } else {
                0.5 * (v_g_cells[i] + v_g_cells[i + 1])
            };

            let flux_left = if face_left >= 0.0 {
                if i == 0 {
                    // Closed or prescribed left end: no vapour enters.
                    0.0
                } else {
                    alpha_old[i - 1] * sat_old[i - 1].rho_g * face_left
                }
            } else {
                a_rho_g_old * face_left
            };
            let flux_right = if face_right >= 0.0 || i + 1 >= n {
                a_rho_g_old * face_right
            } else {
                alpha_old[i + 1] * sat_old[i + 1].rho_g * face_right
            };

            let transported = a_rho_g_old - self.dt / dx * (flux_right - flux_left);
            let a_transported = transported / sat_new.rho_g;

            // Relaxation solved IMPLICITLY in alpha: an explicit relaxation
            // with tau < dt oscillates and then diverges, and tau < dt is the
            // normal case in a fast blowdown.
            let theta = self.dt / (self.tau + self.dt);
            let a = a_transported + theta * (state.void_fraction - a_transported);

            if !a.is_finite() || !(-1.0e-6..=1.0 + 1.0e-6).contains(&a) {
                return Err(TampinesError::Unphysical(format!(
                    "cell {i}: void fraction {a} left [0, 1] at t = {} s -- refused \
                     rather than clamped, because a clamped void produces a \
                     plausible answer that is wrong",
                    self.time + self.dt
                )));
            }
            alpha_new[i] = a.clamp(0.0, 1.0);
            self.sat[i] = sat_new;
            self.rho[i] = state.density;
        }

        self.p = p_new;
        self.h = h_new;
        self.alpha = alpha_new;
        self.u = u_new;
        self.time += self.dt;

        let max_courant = self
            .u
            .iter()
            .map(|u| u.abs() * self.dt / dx)
            .fold(0.0_f64, f64::max);
        let max_void = self.alpha.iter().copied().fold(0.0_f64, f64::max);

        Ok(DriftFluxReport {
            time: self.time,
            outlet_mass_flow: mass_flux[n],
            outlet_choked: right_choked || left_choked,
            inventory: self.rho.iter().map(|r| r * volume).sum(),
            max_void_fraction: max_void,
            max_courant,
        })
    }

    /// Cell-centred mixture velocity, the average of the two bounding faces.
    fn cell_velocities(&self) -> Vec<f64> {
        (0..self.pipe.n_cells())
            .map(|i| 0.5 * (self.u[i] + self.u[i + 1]))
            .collect()
    }

    /// Evaluate the traced-back slip closure on the current state.
    ///
    /// The reference takes field arguments, so the 1-D state is packed into
    /// fields on the 1-D mesh, the closure is called, and the axial component
    /// is unpacked. The round-trip is deliberate: the *reference's own code*
    /// computes `U_dm`, so a correction there propagates here rather than
    /// needing to be mirrored by hand.
    fn drift_velocity(&self, u_m_cells: &[f64]) -> Result<Vec<f64>, TampinesError> {
        let n = self.pipe.n_cells();
        let mut u_field =
            VolVectorField::uniform("U_m", self.mesh.clone(), Vector3::new(0.0, 0.0, 0.0));
        {
            let slice = u_field.internal.as_mut_slice();
            for i in 0..n {
                slice[i] = Vector3::new(u_m_cells[i], 0.0, 0.0);
            }
        }
        let mut alpha_field = VolScalarField::uniform("alpha", self.mesh.clone(), 0.0);
        alpha_field.internal.as_mut_slice()[..n].copy_from_slice(&self.alpha[..n]);

        let u_dm = self.slip.drift_velocity(&u_field, &alpha_field)?;
        Ok(u_dm.iter().map(|v| v.x).collect())
    }

    /// The face velocity a boundary condition imposes, and whether it choked.
    ///
    /// `adjacent` is the cell the boundary touches.
    fn boundary_velocity(
        &self,
        bc: AxialBoundary,
        adjacent: usize,
    ) -> Result<(f64, bool), TampinesError> {
        match bc {
            AxialBoundary::Closed => Ok((0.0, false)),
            AxialBoundary::PrescribedVelocity(u) => Ok((u, false)),
            AxialBoundary::ChokedOutlet {
                area_fraction,
                ambient_pressure,
            } => {
                if !(0.0..=1.0).contains(&area_fraction) {
                    return Err(TampinesError::InvalidInput(format!(
                        "break area fraction must be in [0, 1] (got {area_fraction})"
                    )));
                }
                let (p_throat, g_crit) = critical_flux(self.p[adjacent], self.h[adjacent]);
                let choked = p_throat > ambient_pressure;
                let g = if choked {
                    g_crit
                } else {
                    // Subcritical Bernoulli discharge on the pressure
                    // difference to ambient. Crude, and only reached once the
                    // pipe has emptied to near-ambient, by which point the
                    // break no longer sets the answer.
                    let dp = (self.p[adjacent] - ambient_pressure).max(0.0);
                    (2.0 * self.rho[adjacent] * dp).sqrt()
                };
                Ok((g * area_fraction / self.rho[adjacent], choked))
            }
        }
    }
}

/// Specific enthalpy \[J/kg\] at `(p, T)`, picking the IF97 region from `T`
/// against `T_sat`.
///
/// A `(p, T)` pair inside the dome is degenerate — it does not fix the state —
/// so exactly at `T_sat` this returns the **saturated-liquid** enthalpy, the
/// convention a subcooled initial condition needs.
fn enthalpy_at_pt(p: f64, t: f64, sat: SaturatedProperties) -> Result<f64, TampinesError> {
    use tampines_steam_tables::region_1_subcooled_liquid::h_tp_1;
    use tampines_steam_tables::region_2_vapour::h_tp_2;
    use uom::si::available_energy::joule_per_kilogram;

    let p_q = Pressure::new::<pascal>(p);
    let t_q = ThermodynamicTemperature::new::<kelvin>(t);
    let h = if t <= sat.t_sat {
        h_tp_1(t_q, p_q).get::<joule_per_kilogram>()
    } else {
        h_tp_2(t_q, p_q).get::<joule_per_kilogram>()
    };
    if !h.is_finite() {
        return Err(TampinesError::Unphysical(format!(
            "IF97 returned a non-finite enthalpy at p = {p} Pa, T = {t} K"
        )));
    }
    Ok(h)
}

/// Secant compressibility `(ρ(p_new) − ρ(p_old)) / (p_new − p_old)` \[s²/m²\]
/// at fixed enthalpy.
///
/// # Why a secant and not the tangent
///
/// The tangent `∂ρ/∂p|_h` is the right linearisation of an *infinitesimal*
/// step. An outer corrector takes a finite one, and in a blowdown that step
/// routinely crosses the saturation line — where `ρ(p)` has a kink, because
/// flashing switches on. A tangent evaluated on either side of that kink is
/// wrong about the other side no matter how small the finite-difference step
/// is, whereas the secant through both endpoints is exactly the average slope
/// the step actually traverses. That is why the correctors converge instead of
/// oscillating across the phase boundary.
///
/// Falls back to the tangent when the two pressures are too close for the
/// difference to be meaningful — the same degenerate-endpoint guard
/// `TampinesSteamArray::correct_thermo` carries in `tampines-steam-tables`.
///
/// # Errors
///
/// [`TampinesError::Unphysical`] if either pressure is outside IF97's range.
fn secant_compressibility(
    p_old: f64,
    rho_old: f64,
    p_new: f64,
    h: f64,
) -> Result<f64, TampinesError> {
    let dp = p_new - p_old;
    if dp.abs() < 1.0 {
        let sat = SaturatedProperties::at(p_new)?;
        return TwoPhaseState::flash(p_new, h, sat)?.compressibility(COMPRESSIBILITY_STEP);
    }
    let sat_new = SaturatedProperties::at(p_new)?;
    let rho_new = TwoPhaseState::flash(p_new, h, sat_new)?.density;
    Ok((rho_new - rho_old) / dp)
}

/// Volume-weighted mixture viscosity `μ_m = α μ_g + (1−α) μ_f` \[Pa·s\].
///
/// The same linear mixing rule the 3-D reference's
/// `DriftFluxMixture::mu_mixture` uses, carrying the same documented
/// restriction: it omits the shear-thinning behaviour of real bubbly flows.
fn mixture_viscosity(alpha: f64, sat: SaturatedProperties) -> f64 {
    alpha * sat.mu_g + (1.0 - alpha) * sat.mu_f
}

/// Wall-friction deceleration `F_wall/ρ` \[m/s²\], Darcy-Weisbach.
///
/// `F/ρ = f |u| u / (2 D_h)` with
///
/// - laminar, `Re < 2300`: `f = 64/Re`, exact for fully developed pipe flow;
/// - turbulent: `f = 0.316 Re^{−1/4}` (Blasius, smooth pipe, good to
///   `Re ≈ 10⁵`).
///
/// Blasius rather than Colebrook because it is explicit, and because in a
/// blowdown friction is second-order against the pressure gradient — the
/// roughness dependence Colebrook adds would be false precision. Carries the
/// sign of `u`, so it always opposes the motion.
fn wall_friction_acceleration(u: f64, rho: f64, mu: f64, d_h: f64) -> f64 {
    if u == 0.0 || mu <= 0.0 || rho <= 0.0 {
        return 0.0;
    }
    let re = (rho * u.abs() * d_h / mu).max(1.0e-6);
    let f = if re < 2300.0 {
        64.0 / re
    } else {
        0.316 * re.powf(-0.25)
    };
    f * u.abs() * u / (2.0 * d_h)
}

/// The drift momentum flux `Φ = α ρ_g ρ_l U_dm² / ((1−α) ρ_m)` \[Pa\].
///
/// The extra momentum carried because the phases move at different speeds.
/// Zero at `α = 0`, and returned as zero as `α → 1`: the `(1−α)` denominator
/// diverges there, but a single-phase vapour cell carries no drift momentum, so
/// zero is the correct limit rather than a guard.
fn drift_momentum_flux(alpha: f64, sat: SaturatedProperties, rho_m: f64, u_dm: f64) -> f64 {
    if alpha <= 0.0 || alpha >= 1.0 - 1.0e-9 || rho_m <= 0.0 {
        return 0.0;
    }
    alpha * sat.rho_g * sat.rho_f * u_dm * u_dm / ((1.0 - alpha) * rho_m)
}

/// Vapour velocity `v_g = j + U_dm` \[m/s\], from the exact reconstruction in
/// the module docs.
fn gas_velocity(alpha: f64, sat: SaturatedProperties, rho_m: f64, u_m: f64, u_dm: f64) -> f64 {
    if rho_m <= 0.0 {
        return u_m;
    }
    let j = u_m + alpha * u_dm * (sat.rho_f - sat.rho_g) / rho_m;
    j + u_dm
}

/// Donor-cell (first-order upwind) pick of a cell-centred scalar at a face.
///
/// `flux` is the face mass flux \[kg/s\], positive from `upstream` toward
/// `downstream`. When `upstream` is `None` — the face is the domain's left end
/// — the downstream value is used, the zero-gradient extrapolation appropriate
/// to a boundary with no inflow state to draw on.
fn upwind_scalar(field: &[f64], flux: f64, upstream: Option<usize>, downstream: usize) -> f64 {
    if flux >= 0.0 {
        match upstream {
            Some(i) => field[i],
            None => field[downstream],
        }
    } else {
        field[downstream]
    }
}

/// The HEM critical throat pressure \[Pa\] and mass flux \[kg/(m²·s)\] at a
/// stagnation `(p, h)`.
///
/// Delegates to the crate's existing dispatcher — the one exercised against
/// Moody / Zaloudek / Marviken — rather than introducing a second choking
/// model. See [`AxialBoundary::ChokedOutlet`] for why the break stays HEM even
/// when the pipe does not.
fn critical_flux(p: f64, h: f64) -> (f64, f64) {
    use tampines_steam_tables::steam_turbine_equations::converging_diverging_nozzles::choked_flow::get_critical_pressure_and_mass_flux_multiphase_ph;
    use uom::si::available_energy::joule_per_kilogram;
    use uom::si::mass_flux::kilogram_per_square_meter_second;

    let (p_throat, g) = get_critical_pressure_and_mass_flux_multiphase_ph(
        Pressure::new::<pascal>(p),
        uom::si::f64::AvailableEnergy::new::<joule_per_kilogram>(h),
    );
    (
        p_throat.get::<pascal>(),
        g.get::<kilogram_per_square_meter_second>(),
    )
}

/// Wrap a raw SI velocity as a `uom` quantity, for callers outside the
/// marching loop.
#[must_use]
pub fn as_velocity(u_si: f64) -> Velocity {
    Velocity::new::<meter_per_second>(u_si)
}

/// Axial position of a cell centre as a `uom` quantity.
#[must_use]
pub fn cell_position(pipe: &Pipe1d, cell: usize) -> Length {
    Length::new::<uom::si::length::meter>(pipe.cell_centre(cell))
}
