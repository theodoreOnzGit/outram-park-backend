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
    /// against a reference. Substituting an unvalidated drift-flux choking
    /// model would trade a known inconsistency for an unknown one. It is
    /// called out here, in the module docs, and in the Edwards case, so it
    /// cannot be mistaken for an oversight.
    ///
    /// **What "exercised against a reference" means here**, re-read from
    /// `tampines-steam-tables`'s test source on 2026-08-11. The call path was
    /// verified, not assumed: the Moody validator computes its test value via
    /// `TampinesSteamTableCV::get_stagnation_critical_mass_flux`, which calls
    /// `get_crit_pressure_and_massflux`, which calls
    /// `get_critical_pressure_and_mass_flux_multiphase_ph` -- the same
    /// dispatcher this boundary condition consumes -- and Zaloudek's
    /// `generic_multiphase_stagnation` tests call
    /// `get_crit_pressure_and_massflux` likewise. Note the Moody coverage of
    /// the dispatcher is *incidental*: those tests predate the dispatcher and
    /// reach it only because the OOP getter was rewired through it, so a
    /// refactor of that getter could silently remove the coverage.
    ///
    /// - **Moody (1975) Fig. 1** -- gated **through the dispatcher**: 13
    ///   active isobar tests
    ///   (p0/p_ref = 0.25-30.0) assert `|log10 G_test - log10 G_ref| <= 0.06`,
    ///   0.08 for deeply-subcooled Region-1 points. Caveat: the validator is
    ///   region-filtered -- points that are neither in-dome (Region 4) nor
    ///   deeply subcooled are skipped as a documented HEM limitation rather
    ///   than asserted.
    /// - **Zaloudek** -- gated: 89 tests across five files (21 in-dome / 21
    ///   generic-dispatcher / 21 backward-throat / 22 subcooled, one an
    ///   `#[ignore]`d diagnostic sweep / 4 superheated), critical-pressure
    ///   tolerance 0.005-0.05 by curve, mass-flux log10 tolerance 0.05. Only
    ///   the 21 generic tests exercise the dispatcher itself; the other four
    ///   files call the regime-specific solvers directly, pinning what the
    ///   dispatcher routes *to* rather than the routing.
    ///   Caveat: these are graph-read HEM curves digitised from Saha (1978)
    ///   NUREG/CR-0417, **not** experimental data -- so they check this
    ///   implementation against another HEM, not against reality.
    /// - **Marviken is NOT gated.** The digitised NUREG/CR-2671 test-23/24
    ///   data exists in `marviken_tests.rs`, but the single test is
    ///   `#[ignore = "skip first, Marviken is more complex"]`, its only
    ///   assertion is commented out, and the body ends in `todo!()`. Do not
    ///   cite Marviken in support of this boundary condition. Status as of
    ///   2026-08-11: gating is being implemented in `tampines-steam-tables`
    ///   (the op-bcg/op-4ily follow-up); re-check that crate's test suite
    ///   for the current result rather than treating this line as permanent.
    ///
    /// The benchmark actually run on this solver path is **Edwards-O'Brien**,
    /// gated by `edwards_drift_flux_gs1_pressure_history` in
    /// `src/multiphase_1d/edwards_tests.rs` (measured 2026-08-11: full 600 ms,
    /// GS-1 pressure RMSE 29.0 psia against the digitised experimental curve,
    /// flashing plateau 354.3 psia inside the ~350-367 psia experimental band).
    /// Read the caveats in that file's header before citing it: the reference
    /// is a digitisation of a published figure, only GS-1 is gated, and the
    /// break itself is this HEM dispatcher, so that result does **not**
    /// validate a drift-flux critical-flow model.
    ///
    /// **Correction, 2026-08-11.** This paragraph previously read "The
    /// genuinely validated benchmark on this solver path is Edwards-O'Brien
    /// (2 active, passing tests)" while no Edwards test existed anywhere in
    /// this crate. It does now, and the sentence has been rewritten to say
    /// what it gates.
    ChokedOutlet {
        /// Break area as a fraction of the pipe flow area, in `(0, 1]`.
        area_fraction: f64,
        /// Ambient / containment back-pressure \[Pa\]. The outlet unchokes
        /// once the critical throat pressure falls below it.
        ambient_pressure: f64,
    },

    /// A **large upstream plenum** feeding the pipe: a Dirichlet *stagnation*
    /// pressure with a prescribed stagnation enthalpy, i.e. the vessel of a
    /// discharge experiment.
    ///
    /// # What it imposes, and on which staggered locations
    ///
    /// Three separate things, at three places in the step:
    ///
    /// 1. **A Dirichlet pressure `p_0` in the pressure equation**, at the
    ///    boundary *face*. The face is no longer excluded from the matrix as
    ///    [`Closed`](Self::Closed) and [`PrescribedVelocity`](Self::PrescribedVelocity)
    ///    are — it carries the usual velocity-correction coefficient over a
    ///    **half cell**, `d = Δt / (ρ_face · Δx/2)`, because the distance from
    ///    the boundary face to the first cell centre is `Δx/2` and not `Δx`.
    ///    The resulting `ρ_face · d · p_0` term enters the first cell's
    ///    right-hand side, which is what makes the pressure problem
    ///    well-posed with a Dirichlet at each end instead of floating on the
    ///    compressibility diagonal.
    /// 2. **A momentum balance at the boundary face**, giving the predictor
    ///    velocity. For inflow the convective term is taken in *conservative*
    ///    form over the half cell from a stagnant plenum,
    ///    `d(u²/2)/dx ≈ (u²/2 − 0)/(Δx/2)`, rather than the non-conservative
    ///    `u ∂u/∂x` the interior faces use. That is deliberate and it is the
    ///    difference between a right and a wrong answer here: with the
    ///    pressure correction, the steady state of the conservative form is
    ///    exactly `u = sqrt(2 (p_0 − p_1) / ρ)` — Bernoulli acceleration from
    ///    rest — whereas the non-conservative donor form across the plenum
    ///    velocity jump returns `sqrt((p_0 − p_1)/ρ)`, low by `sqrt(2)`. The
    ///    interior form is kept for backflow (`u < 0`), where there is no jump
    ///    because the upstream state is the first cell.
    ///    Wall friction uses the adjacent cell's mixture viscosity; the drift
    ///    momentum flux is differenced over the same half cell against
    ///    **zero**, since a stagnant plenum has no relative phase motion.
    /// 3. **An inflow donor state for the scalars.** The existing donor-cell
    ///    helpers assume zero-gradient at the ends, which silently makes an
    ///    inlet re-inject whatever is already in the first cell. Instead the
    ///    entering fluid carries the reservoir's stagnation enthalpy `h_0`,
    ///    arrives at the first cell's pressure `p_1`, and is flashed there:
    ///    that equilibrium flash supplies the inflow density used for the face
    ///    mass flux and the inflow void used by the vapour-mass equation. The
    ///    enthalpy handed to the energy equation is the **static** value
    ///    `h_0 − u²/2`, which is the consistent partner of this solver's
    ///    `ρ Dh/Dt = Dp/Dt` energy equation (that equation reproduces
    ///    `dh = dp/ρ`, so `h + u²/2` is conserved along a streamline and the
    ///    boundary must supply the static, not the stagnation, enthalpy).
    ///
    /// # The two standing assumptions
    ///
    /// Both are load-bearing, so they are stated here rather than only at the
    /// call sites that rely on them.
    ///
    /// 1. **The drift velocity is zero at the inlet face.** A stagnant plenum
    ///    has no relative phase motion, so `U_dm = 0` there and the drift
    ///    momentum flux `Φ` is differenced over the inlet half cell against
    ///    **zero** rather than against an extrapolated interior value. The
    ///    consequence to be aware of: entering fluid that is already two-phase
    ///    arrives with its phases moving *together*, and any slip must develop
    ///    over the first cells. A case whose answer depends on the inlet slip
    ///    being non-zero — a vertical riser fed by a churning plenum, say — is
    ///    outside what this boundary models.
    /// 2. **The plenum is infinite and unchanging.** It does not deplete, does
    ///    not cool, and does not respond to what the pipe does. A blowdown
    ///    whose vessel state moves during the window of interest must re-impose
    ///    the boundary each step with the updated state, exactly as the
    ///    Edwards case re-imposes its rupture-disc ramp.
    ///
    /// # V&V status
    ///
    /// **Verified** — for a single-phase subcooled inlet — by
    /// `reservoir_inlet_reaches_the_bernoulli_limit_in_subcooled_water` in
    /// `src/multiphase_1d/tests.rs`, which marches a 20 kPa discharge to steady
    /// state and reproduces the closed form
    /// `u = sqrt(2 (p_0 − p_out) / (ρ (1 + f L / D)))` to **1.1 parts per
    /// million** (measured 2026-08-11, release: `6.225891` m/s against
    /// `6.225884` m/s), with the inlet, interior and outlet pressure terms each
    /// matching their predicted values to `≈0.01` Pa. Read that test's doc
    /// comment for the full record.
    ///
    /// **Not verified**: the two-phase inflow flash (the test's `α` is
    /// identically zero, so `InflowState::void_fraction` and
    /// `InflowState::rho_g` were never exercised), the `h_0 − u²/2` static
    /// enthalpy split at any appreciable velocity head, backflow through this
    /// boundary, and any transient behaviour. Do not describe those as checked.
    ///
    /// # Other limitations, stated plainly
    ///
    /// - **The inflow flash is an equilibrium one.** The metastability this
    ///   solver represents — the vapour-generation relaxation `τ` — acts
    ///   inside the domain, not at the boundary. Entering fluid is therefore
    ///   on the equilibrium `(p, h)` locus by construction, and a case that
    ///   wants delayed flashing must let it develop over the first cells.
    /// - **The area change of a real convergent inlet is not resolved.** The
    ///   pipe is constant-area, so the acceleration from a vessel into a bore
    ///   happens as a jump at the face rather than over a contraction, and no
    ///   geometric throat exists inside the domain.
    /// - **Nothing here chokes the flow.** The pressure equation is elliptic,
    ///   so information propagates upstream through it; this boundary imposes
    ///   no critical-flow criterion, unlike [`ChokedOutlet`](Self::ChokedOutlet).
    ///   Whether a critical flux emerges at all is a property of the solution,
    ///   and a case relying on one must measure it rather than assume it.
    ReservoirInlet {
        /// Plenum stagnation pressure `p_0` \[Pa\]. Must be inside the
        /// IAPWS-IF97 range.
        stagnation_pressure: f64,
        /// Plenum stagnation specific enthalpy `h_0` \[J/kg\]. Fixes what
        /// enters: subcooled liquid, saturated liquid, or a two-phase
        /// mixture, according to where `(p_1, h_0)` lands.
        stagnation_enthalpy: f64,
    },

    /// A **prescribed static pressure** at the boundary face: the receiver a
    /// pipe discharges into.
    ///
    /// # What it imposes, and on which staggered locations
    ///
    /// - **A Dirichlet pressure in the pressure equation**, at the boundary
    ///   face, with the same half-cell coefficient
    ///   `d = Δt / (ρ_face · Δx/2)` described at
    ///   [`ReservoirInlet`](Self::ReservoirInlet). The face velocity is
    ///   corrected as `u ← u* − d (p_out − p_last)` at the `x = L` end, so a
    ///   receiver pressure below the last cell's drives outflow.
    /// - **A momentum balance at the boundary face** for the predictor. For
    ///   outflow this is the *same* donor-cell `u ∂u/∂x` the interior faces
    ///   use, taken against the last interior face — the field is smooth
    ///   there, so no special treatment is warranted. For backflow it is the
    ///   conservative half-cell form from a stagnant receiver, for the reason
    ///   given at [`ReservoirInlet`](Self::ReservoirInlet).
    /// - **Scalar outflow is upwinded from the last cell**, which is what the
    ///   existing donor-cell helpers already do at an end face.
    ///
    /// # Limitations, stated plainly
    ///
    /// - **No inflow state.** If the pressure gradient reverses, fluid enters
    ///   carrying the *last cell's* enthalpy and void (zero-gradient), because
    ///   this boundary carries no description of the receiver's contents. A
    ///   case with sustained backflow through this boundary is outside what it
    ///   models; use a [`ReservoirInlet`](Self::ReservoirInlet) there instead.
    /// - **It cannot choke.** A prescribed downstream pressure is felt
    ///   upstream through the implicit pressure solve, which is exactly the
    ///   behaviour a critical-flow model exists to suppress. This boundary
    ///   imposes no critical-flux criterion of any kind, so a case that wants
    ///   one must either use [`ChokedOutlet`](Self::ChokedOutlet) or maximise
    ///   the steady flux over the receiver pressure externally — and must
    ///   report honestly if no maximum is found.
    PressureOutlet {
        /// Receiver static pressure \[Pa\], inside the IAPWS-IF97 range.
        pressure: f64,
    },
}

/// The state of the fluid entering the domain through a boundary face.
///
/// Assembled once per step by [`DriftFlux1d::resolve_boundary`] and consumed
/// by the scalar transport, which otherwise has no inflow value to donate and
/// falls back to a zero-gradient extrapolation.
///
/// Raw `f64` in strict SI, like the rest of the marching loop.
#[derive(Debug, Clone, Copy, PartialEq)]
struct InflowState {
    /// Stagnation specific enthalpy carried by the entering fluid \[J/kg\].
    /// The energy equation is fed `h_stagnation − u²/2`; see
    /// [`AxialBoundary::ReservoirInlet`].
    h_stagnation: f64,
    /// Equilibrium vapour volume fraction of the entering fluid \[-\],
    /// flashed at the adjacent cell's pressure.
    void_fraction: f64,
    /// Vapour density at the adjacent cell's pressure \[kg/m³\], the partner
    /// of `void_fraction` in the vapour-mass flux `α ρ_g v_g`.
    rho_g: f64,
    /// Mixture density of the entering fluid \[kg/m³\], used for the face mass
    /// flux.
    density: f64,
}

/// A boundary face resolved against the current state — what
/// [`DriftFlux1d::step`] actually consumes.
///
/// Enum dispatch per the workspace rule. The distinction that matters to the
/// pressure equation is exactly this one: a *velocity* boundary contributes no
/// pressure sensitivity and stays out of the matrix, while a *pressure*
/// boundary contributes a Dirichlet term and a half-cell velocity correction.
#[derive(Debug, Clone, Copy, PartialEq)]
enum BoundaryFaceState {
    /// The face velocity \[m/s\] is prescribed outright; `choked` records
    /// whether a critical-flow model set it.
    Velocity { u: f64, choked: bool },
    /// The face carries a Dirichlet pressure \[Pa\]; its velocity comes from
    /// the momentum predictor plus the usual pressure correction. `inflow` is
    /// `Some` only where the boundary describes what enters.
    Pressure {
        p_boundary: f64,
        inflow: Option<InflowState>,
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

        // Boundaries are resolved against the OLD state, before the predictor,
        // because a Dirichlet-pressure face needs its face density in hand to
        // build its own momentum predictor.
        let left_face = self.resolve_boundary(self.left, 0)?;
        let right_face = self.resolve_boundary(self.right, n - 1)?;
        let left_inflow = match left_face {
            BoundaryFaceState::Pressure { inflow, .. } => inflow,
            BoundaryFaceState::Velocity { .. } => None,
        };
        let right_inflow = match right_face {
            BoundaryFaceState::Pressure { inflow, .. } => inflow,
            BoundaryFaceState::Velocity { .. } => None,
        };
        // Donor density at the end faces: the entering fluid's where fluid
        // enters through a boundary that describes itself, the adjacent cell's
        // otherwise. The `None` arms reproduce the previous behaviour exactly.
        let rho_face_left = match left_inflow {
            Some(s) if self.u[0] >= 0.0 => s.density,
            _ => rho_old[0],
        };
        let rho_face_right = match right_inflow {
            Some(s) if self.u[n] <= 0.0 => s.density,
            _ => rho_old[n - 1],
        };

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
        let left_choked = matches!(left_face, BoundaryFaceState::Velocity { choked: true, .. });
        let right_choked = matches!(right_face, BoundaryFaceState::Velocity { choked: true, .. });
        u_star[0] = match left_face {
            BoundaryFaceState::Velocity { u, .. } => u,
            BoundaryFaceState::Pressure { .. } => self.left_pressure_face_predictor(
                rho_face_left,
                &rho_old,
                &alpha_old,
                &sat_old,
                &u_dm,
                dx,
            ),
        };
        u_star[n] = match right_face {
            BoundaryFaceState::Velocity { u, .. } => u,
            BoundaryFaceState::Pressure { .. } => {
                self.right_pressure_face_predictor(rho_face_right, &alpha_old, &sat_old, dx)
            }
        };

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
                rho_face_left
            } else if j == n {
                rho_face_right
            } else if u_star[j] >= 0.0 {
                rho_old[j - 1]
            } else {
                rho_old[j]
            };
            // A boundary face carrying a prescribed velocity has no pressure
            // sensitivity; d = 0 keeps it out of the matrix. A boundary face
            // carrying a Dirichlet pressure does have one, over the HALF cell
            // between the face and the first cell centre.
            d_face[j] = if j == 0 {
                match left_face {
                    BoundaryFaceState::Velocity { .. } => 0.0,
                    BoundaryFaceState::Pressure { .. } => self.dt / (rho_face_left * 0.5 * dx),
                }
            } else if j == n {
                match right_face {
                    BoundaryFaceState::Velocity { .. } => 0.0,
                    BoundaryFaceState::Pressure { .. } => self.dt / (rho_face_right * 0.5 * dx),
                }
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
            // Dirichlet contributions. A pressure boundary's face velocity
            // correction u = u* - d (p_cell - p_boundary) splits into an
            // implicit part -- already in `diag`, since the assembly above
            // adds `area * rho_face * d_face` at every face -- and an explicit
            // `p_boundary` part, which belongs on the right-hand side. For a
            // velocity boundary `d_face` is zero and these terms vanish, so
            // the existing variants are untouched.
            if let BoundaryFaceState::Pressure { p_boundary, .. } = left_face {
                rhs[0] += area * rho_face[0] * d_face[0] * p_boundary;
            }
            if let BoundaryFaceState::Pressure { p_boundary, .. } = right_face {
                rhs[n - 1] += area * rho_face[n] * d_face[n] * p_boundary;
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
            // The end faces. A velocity boundary re-imposes its velocity; a
            // pressure boundary takes the same correction as an interior face,
            // with the boundary pressure standing in for the missing
            // neighbour's -- to the left of face 0 and to the right of face n.
            u_new[0] = match left_face {
                BoundaryFaceState::Velocity { u, .. } => u,
                BoundaryFaceState::Pressure { p_boundary, .. } => {
                    u_star[0] - d_face[0] * (p_new[0] - p_boundary)
                }
            };
            u_new[n] = match right_face {
                BoundaryFaceState::Velocity { u, .. } => u,
                BoundaryFaceState::Pressure { p_boundary, .. } => {
                    u_star[n] - d_face[n] * (p_boundary - p_new[n - 1])
                }
            };

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

        // Inflow enthalpy at the end faces. The energy equation transports the
        // STATIC enthalpy, and this solver's `rho Dh/Dt = Dp/Dt` reproduces
        // `dh = dp/rho`, so `h + u^2/2` is conserved along a streamline: a
        // reservoir specified by its stagnation enthalpy therefore delivers
        // `h_0 - u^2/2` at the face. `None` leaves the previous zero-gradient
        // extrapolation in place, so the existing boundary conditions are
        // untouched.
        let h_in_left: Option<f64> =
            left_inflow.map(|s| s.h_stagnation - 0.5 * u_new[0] * u_new[0]);
        let h_in_right: Option<f64> =
            right_inflow.map(|s| s.h_stagnation - 0.5 * u_new[n] * u_new[n]);

        let mut h_new = vec![0.0; n];
        for i in 0..n {
            let h_left = if i == 0 {
                match h_in_left {
                    Some(h_in) if mass_flux[0] >= 0.0 => h_in,
                    _ => h_old[0],
                }
            } else {
                upwind_scalar(&h_old, mass_flux[i], i - 1, i)
            };
            let h_right = if i + 1 == n {
                match h_in_right {
                    Some(h_in) if mass_flux[n] <= 0.0 => h_in,
                    _ => h_old[n - 1],
                }
            } else {
                upwind_scalar(&h_old, mass_flux[i + 1], i, i + 1)
            };
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
                    // A closed or prescribed-velocity left end admits no
                    // vapour. A reservoir inlet admits the vapour its own
                    // inflow flash carries, which is what makes an inlet that
                    // is already flashing behave differently from a closed one.
                    match left_inflow {
                        Some(s) => s.void_fraction * s.rho_g * face_left,
                        None => 0.0,
                    }
                } else {
                    alpha_old[i - 1] * sat_old[i - 1].rho_g * face_left
                }
            } else {
                a_rho_g_old * face_left
            };
            let flux_right = if i + 1 >= n {
                if face_right < 0.0 {
                    // Backflow through the right end: a reservoir there
                    // donates its own vapour, anything else is zero-gradient.
                    match right_inflow {
                        Some(s) => s.void_fraction * s.rho_g * face_right,
                        None => a_rho_g_old * face_right,
                    }
                } else {
                    a_rho_g_old * face_right
                }
            } else if face_right >= 0.0 {
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

    /// Resolve a boundary condition against the current state into the form
    /// [`step`](Self::step) consumes.
    ///
    /// `adjacent` is the cell the boundary face touches. Velocity boundaries
    /// return the face velocity and whether a critical-flow model set it;
    /// pressure boundaries return the Dirichlet pressure and, where the
    /// boundary describes what enters, the [`InflowState`] the scalar
    /// transport donates from.
    ///
    /// # Errors
    ///
    /// [`TampinesError::InvalidInput`] for a break-area fraction outside
    /// `[0, 1]` or a non-finite reservoir state;
    /// [`TampinesError::Unphysical`] if the reservoir's `(p, h)` flash leaves
    /// IF97's range.
    fn resolve_boundary(
        &self,
        bc: AxialBoundary,
        adjacent: usize,
    ) -> Result<BoundaryFaceState, TampinesError> {
        match bc {
            AxialBoundary::Closed => Ok(BoundaryFaceState::Velocity {
                u: 0.0,
                choked: false,
            }),
            AxialBoundary::PrescribedVelocity(u) => {
                Ok(BoundaryFaceState::Velocity { u, choked: false })
            }
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
                Ok(BoundaryFaceState::Velocity {
                    u: g * area_fraction / self.rho[adjacent],
                    choked,
                })
            }
            AxialBoundary::ReservoirInlet {
                stagnation_pressure,
                stagnation_enthalpy,
            } => {
                if !stagnation_pressure.is_finite() || !stagnation_enthalpy.is_finite() {
                    return Err(TampinesError::InvalidInput(format!(
                        "reservoir inlet state must be finite (got p_0 = {stagnation_pressure} Pa, \
                         h_0 = {stagnation_enthalpy} J/kg)"
                    )));
                }
                // The entering fluid carries h_0 and arrives at the adjacent
                // cell's pressure; the equilibrium flash there is the inflow
                // donor state. See the variant's doc comment for why the flash
                // is an equilibrium one even though the interior is not.
                let p_cell = self.p[adjacent];
                let sat = SaturatedProperties::at(p_cell)?;
                let state = TwoPhaseState::flash(p_cell, stagnation_enthalpy, sat)?;
                Ok(BoundaryFaceState::Pressure {
                    p_boundary: stagnation_pressure,
                    inflow: Some(InflowState {
                        h_stagnation: stagnation_enthalpy,
                        void_fraction: state.void_fraction,
                        rho_g: sat.rho_g,
                        density: state.density,
                    }),
                })
            }
            AxialBoundary::PressureOutlet { pressure } => {
                if !pressure.is_finite() {
                    return Err(TampinesError::InvalidInput(format!(
                        "pressure outlet must be finite (got {pressure} Pa)"
                    )));
                }
                Ok(BoundaryFaceState::Pressure {
                    p_boundary: pressure,
                    inflow: None,
                })
            }
        }
    }

    /// Explicit momentum predictor at the `x = 0` face when it carries a
    /// Dirichlet pressure.
    ///
    /// Inflow (`u ≥ 0`) uses the **conservative** half-cell acceleration
    /// `d(u²/2)/dx` measured from a stagnant plenum, so that the corrected
    /// steady state is exactly Bernoulli; backflow uses the interior's
    /// donor-cell `u ∂u/∂x`, there being no velocity jump in that direction.
    /// The drift momentum flux is differenced over the same half cell against
    /// zero (a stagnant plenum carries no relative phase motion).
    ///
    /// `rho_face` is the face density already resolved for this step — the
    /// inflow density when fluid enters, the first cell's otherwise.
    fn left_pressure_face_predictor(
        &self,
        rho_face: f64,
        rho_old: &[f64],
        alpha_old: &[f64],
        sat_old: &[SaturatedProperties],
        u_dm: &[f64],
        dx: f64,
    ) -> f64 {
        let u = self.u[0];
        let convection = if u >= 0.0 {
            u * u / dx
        } else {
            u * (self.u[1] - u) / dx
        };
        let mu = mixture_viscosity(alpha_old[0], sat_old[0]);
        let friction =
            wall_friction_acceleration(u, rho_face, mu, self.pipe.hydraulic_diameter_si());
        let phi_cell = drift_momentum_flux(alpha_old[0], sat_old[0], rho_old[0], u_dm[0]);
        let d_phi_dx = phi_cell / (0.5 * dx);
        u + self.dt * (-convection + self.pipe.axial_gravity() - friction - d_phi_dx / rho_face)
    }

    /// Explicit momentum predictor at the `x = L` face when it carries a
    /// Dirichlet pressure.
    ///
    /// Outflow (`u ≥ 0`) uses the interior's donor-cell `u ∂u/∂x` against the
    /// last interior face — the field is smooth there. Backflow uses the
    /// conservative half-cell form from a stagnant receiver, mirroring
    /// [`left_pressure_face_predictor`](Self::left_pressure_face_predictor).
    /// The drift momentum flux is zero-gradient, because nothing is known
    /// about the receiver's phase distribution.
    fn right_pressure_face_predictor(
        &self,
        rho_face: f64,
        alpha_old: &[f64],
        sat_old: &[SaturatedProperties],
        dx: f64,
    ) -> f64 {
        let n = self.pipe.n_cells();
        let u = self.u[n];
        let convection = if u >= 0.0 {
            u * (u - self.u[n - 1]) / dx
        } else {
            -u * u / dx
        };
        let mu = mixture_viscosity(alpha_old[n - 1], sat_old[n - 1]);
        let friction =
            wall_friction_acceleration(u, rho_face, mu, self.pipe.hydraulic_diameter_si());
        u + self.dt * (-convection + self.pipe.axial_gravity() - friction)
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

/// Donor-cell (first-order upwind) pick of a cell-centred scalar at an
/// **interior** face.
///
/// `flux` is the face mass flux \[kg/s\], positive from `upstream` toward
/// `downstream`; both indices name real cells, which is why this is interior
/// only. End faces are handled separately in
/// [`DriftFlux1d::step`](DriftFlux1d::step), because a boundary either has an
/// inflow state to donate — see [`InflowState`] — or has none and falls back to
/// a zero-gradient extrapolation, and that choice belongs with the boundary
/// condition rather than in here.
fn upwind_scalar(field: &[f64], flux: f64, upstream: usize, downstream: usize) -> f64 {
    if flux >= 0.0 {
        field[upstream]
    } else {
        field[downstream]
    }
}

/// The HEM critical throat pressure \[Pa\] and mass flux \[kg/(m²·s)\] at a
/// stagnation `(p, h)`.
///
/// Delegates to the crate's existing dispatcher rather than introducing a
/// second choking model. See [`AxialBoundary::ChokedOutlet`] for why the break
/// stays HEM even when the pipe does not.
///
/// **V&V status of that dispatcher** (re-read from `tampines-steam-tables`'s
/// test source on 2026-08-11):
///
/// - **Moody (1975) Fig. 1** -- gated: 13 active isobar tests, tolerance
///   `|log10 G| <= 0.06` (0.08 deeply subcooled); region-filtered, so points
///   neither in-dome (Region 4) nor deeply subcooled are skipped, not
///   asserted. Call path verified 2026-08-11: those tests reach the *same
///   dispatcher this function calls*, via
///   `TampinesSteamTableCV::get_stagnation_critical_mass_flux ->
///   get_crit_pressure_and_massflux ->
///   get_critical_pressure_and_mass_flux_multiphase_ph` -- incidental
///   coverage (the tests predate the dispatcher), so a getter refactor could
///   silently remove it.
/// - **Zaloudek** -- gated: 89 tests across five files (one an `#[ignore]`d
///   diagnostic sweep), pressure tolerance 0.005-0.05, mass-flux log10
///   tolerance 0.05 -- against graph-read HEM curves from Saha (1978)
///   NUREG/CR-0417, not experimental data. Only the 21
///   `generic_multiphase_stagnation` tests exercise the dispatcher itself;
///   the rest pin the regime-specific solvers it routes to.
/// - **Marviken is NOT gated** -- its test is `#[ignore]`d with its sole
///   assertion commented out and a `todo!()` body, so it supports no claim
///   about this function. Status as of 2026-08-11: gating is being
///   implemented in `tampines-steam-tables` (the op-bcg/op-4ily follow-up);
///   re-check that crate's test suite for the current result.
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
