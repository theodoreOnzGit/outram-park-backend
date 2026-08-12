// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Closures traced back to the OUTRAM-FOAM 3-D multiphase reference
// (`outram-foam-multiphase`, epic op-2kk) and, for the virtual-mass
// regularisation and the residual-alpha flooring, transcribed from OpenFOAM's
// `multiphaseEuler` solver module through the source study in
// `crates/tampines/docs/six-equation-regularisation.md` (every formula there
// carries a `file:line`).
// Copyright (C) 2004-2026 OpenFOAM Foundation, (C) 2016-2023 OpenCFD Ltd.
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! The 1-D **six-equation two-fluid** solver — separate mass, momentum and
//! energy equations for each phase.
//!
//! # The model
//!
//! Two-fluid drops *both* remaining equilibrium assumptions of
//! [`super::drift_flux`]. Each phase gets its own mass, momentum and energy
//! equation — six in total — so the phases may differ in both velocity *and*
//! temperature. That is what a blowdown actually needs: the liquid can stay
//! subcooled while vapour at the wall is superheated, and neither drift flux
//! nor HEM can represent it.
//!
//! For `k ∈ {g, l}`, with `m_k = α_k ρ_k` the phase mass concentration
//! \[kg/m³\]:
//!
//! `∂m_k/∂t + ∂(m_k u_k)/∂x = Γ_k`  (phase mass, `Γ_g = −Γ_l`)
//!
//! `m_k (∂u_k/∂t + u_k ∂u_k/∂x) = −α_k ∂p/∂x + m_k g_x + F_k^d + F_k^vm + Γ_k (u^i − u_k) − F_k^wall`  (phase momentum)
//!
//! `∂(m_k h_k)/∂t + ∂(m_k h_k u_k)/∂x = α_k ∂p/∂t + Q_k^i + Γ_k h_k^i`  (phase energy)
//!
//! and the **volume constraint** `α_g + α_l = 1`, which is what the pressure
//! equation enforces (see "The pressure equation" below).
//!
//! The momentum equation is written in the *non-conservative* velocity form —
//! the conservative form minus `u_k ×` the phase mass equation — which is why
//! the mass-transfer momentum source appears as `Γ_k (u^i − u_k)` rather than
//! `Γ_k u^i`.
//!
//! # The regularisation, and exactly what is and is not claimed
//!
//! The naive six-equation system with drag alone has complex characteristics.
//! What this solver does about that is **traced to upstream practice, and is
//! not claimed to fix it**:
//!
//! > **The regularisation implemented here — virtual mass inside the implicit
//! > 2×2 phase-coupling block, with `C_vm = 0.5` and `residualAlpha`
//! > flooring — is TRACED TO UPSTREAM PRACTICE in OpenFOAM's
//! > `multiphaseEuler`. It is NOT proven, here or upstream, to restore
//! > hyperbolicity or to make the six-equation system well posed. No
//! > characteristic analysis has been performed by this project, and upstream
//! > states no reasoning of its own anywhere in its source.**
//!
//! The evidence behind that sentence, from
//! `crates/tampines/docs/six-equation-regularisation.md` (a source study of the
//! vendored OpenFOAM tree, read 2026-08-12, every claim carrying a
//! `file:line`):
//!
//! - **There is no interfacial-pressure term in `multiphaseEuler` at all** —
//!   no `pInterface`, no Stuhmiller, no Bestion, anywhere in the vendored tree
//!   (study §1). So none is added here. The scaffold this file replaces
//!   suggested one was needed; the source says upstream does not use one.
//! - **What upstream uses for a fluid-fluid pair is virtual mass**,
//!   `K_vm = max(α_d, α_res) C_vm ρ_c`
//!   (`dispersedVirtualMassModel.C:51-67`), and it is folded **inside** the
//!   implicit phase-coupling matrix — on the diagonal *and* the off-diagonal
//!   (`momentumTransferSystem.C:704-762`) — never beside the drag term and
//!   never as an explicit source. That is reproduced here exactly; see
//!   [`PhaseCouplingBlock`].
//! - `C_vm = 0.5` is what every gas-liquid tutorial in the vendored tree sets
//!   (study §3.3), and it is [`DEFAULT_VIRTUAL_MASS_COEFFICIENT`] here.
//! - **`residualAlpha` flooring** (`cellPressureCorrector.C:82-91`,
//!   `momentumTransferSystem.C:617-620`) is a *numerical* device, not physics,
//!   and it is what keeps the 2×2 block invertible at a blowdown front where a
//!   phase vanishes. Both of its uses are ported — see [`PhaseCouplingBlock`].
//!
//! And the counter-evidence, reported rather than suppressed: the study could
//! **not** confirm that virtual mass *is* the well-posedness fix. Upstream
//! never says so, and two of its thirty tutorials (`damBreak4phase`,
//! `hydrofoil`) run with drag alone and no regularising term at all. Treat
//! "this is what OpenFOAM does" as exactly that, and nothing more.
//!
//! A consequence worth stating plainly: because the regularisation is not
//! known to make the system well posed, **grid refinement is not guaranteed to
//! improve a result from this solver**, and a converged mesh-independent
//! solution may not exist to converge to. Any V&V case run on it owes a
//! refinement study and a `C_vm` sensitivity (study §9.5), not a single-mesh
//! number.
//!
//! # Where the closures come from
//!
//! Beads `op-dt3.12` / `op-dt3.13` require 1-D closures to trace back to the
//! OUTRAM-FOAM 3-D reference rather than be invented. Concretely:
//!
//! 1. **Interfacial drag, heat transfer and mass transfer** are *not* written
//!    down in this file. They come from [`super::interfacial`], which calls
//!    [`outram_foam_multiphase::two_fluid::DragModel`] and
//!    [`outram_foam_multiphase::heat_transfer::InterfacialHeatTransfer`]
//!    through their own public API.
//! 2. **Correction to this module's previous documentation.** It used to say,
//!    under "what still has to be decided", that the 3-D reference *"has no
//!    interfacial heat-transfer closure at all — it is isothermal — so there is
//!    nothing to trace back to"*. **That is false as of 2026-08-11:**
//!    `outram_foam_multiphase::heat_transfer` exists (Ranz-Marshall, spherical
//!    conduction, Gunn, ported from OpenFOAM's `multiphaseEuler` heat-transfer
//!    models), [`super::interfacial`] consumes it, and this solver consumes
//!    that. The old sentence is corrected here rather than left standing.
//! 3. **Virtual mass is the one closure this file writes down itself**, and
//!    that is a deviation which is stated rather than hidden. The reference
//!    crate's [`outram_foam_multiphase::two_fluid::InterfacialForce::VirtualMass`]
//!    is an unported scaffold whose `momentum_coefficient` returns
//!    `MultiphaseError::NotImplemented`, so there is nothing to consume. The
//!    formula used here — [`virtual_mass_coefficient`] — is transcribed from
//!    OpenFOAM's `dispersedVirtualMassModel.C:51-67` and
//!    `constantVirtualMassCoefficient.C:71-79` via the source study, **not**
//!    invented. When the reference crate gains the closure, this function is
//!    the single place to replace.
//!
//! # Numerical method
//!
//! The same **semi-implicit pressure-based march** [`super::drift_flux`] uses,
//! and for the same reason (see [`crate::multiphase_1d`]): an explicit
//! compressible march would be limited by the acoustic CFL. Each step:
//!
//! 1. **Face momentum.** For every face, assemble the 2×2 implicit
//!    drag + virtual-mass coupling block ([`PhaseCouplingBlock`]) and invert it
//!    in closed form, giving the force-only face velocities `u*_k` and the
//!    per-phase pressure sensitivities `d_k`.
//! 2. **Pressure.** Assemble and solve a tridiagonal pressure equation
//!    ([`thomas_solve`](super::thomas_solve)) built from the volume constraint.
//! 3. **Correct** the face velocities with the new pressure gradient.
//! 4. **Transport** the four conserved cell quantities `m_g`, `m_l`,
//!    `m_g h_g`, `m_l h_l` with donor-cell (first-order upwind) fluxes.
//! 5. **Exchange** heat and mass at the interface, implicitly in the phase
//!    enthalpies, then recover `α_k`, `ρ_k`, `T_k` and the volume residual.
//!
//! Steps 2-5 sit inside an **outer-corrector loop**, because the volume
//! constraint is nonlinear in `p`.
//!
//! ## The pressure equation
//!
//! Divide each phase mass equation by `ρ_k` and sum. The `∂α_k/∂t` terms
//! cancel exactly against each other by `Σ α_k = 1`, leaving
//!
//! `Σ_k (α_k ψ_k / ρ_k) ∂p/∂t + Σ_k (1/ρ_k) ∂(m_k u_k)/∂x = Γ (1/ρ_g − 1/ρ_l)`
//!
//! with `ψ_k = ∂ρ_k/∂p|_{h_k}` \[s²/m²\] the **single-phase** compressibility
//! of phase `k` at frozen phase enthalpy. Two things about this are worth
//! reading carefully, because they are exactly where a six-equation model
//! differs from the four-equation one next door:
//!
//! - **The right-hand side is the flashing term.** `1/ρ_g − 1/ρ_l` is the
//!   specific volume created per kilogram evaporated — about `0.076 m³/kg` for
//!   steam/water at 2.6 MPa — so interfacial mass transfer appears in the
//!   pressure equation as a volumetric source. That, and not an equilibrium
//!   flash, is what holds the pressure up on a flashing plateau here.
//! - **There is no kink to linearise across.** [`super::drift_flux`] needs a
//!   *secant* compressibility because its `ρ_m(p)|_h` has a kink at the
//!   saturation line where flashing switches on. Here `ρ_g(p, h_g)` lives
//!   entirely in IF97 Region 2 and `ρ_l(p, h_l)` entirely in Region 1, both
//!   smooth, so a plain one-sided tangent is the correct linearisation and the
//!   secant machinery is not needed. The stiffness moved out of the
//!   compressibility and into the interfacial exchange, which is why *that* is
//!   what gets solved implicitly here.
//!
//! The assembled matrix is strictly diagonally dominant — the off-diagonals sum
//! to exactly the pressure-coefficient part of the diagonal, and the compliance
//! term `V Σ_k α_k ψ_k / ρ_k / Δt` is strictly positive on top of it — so
//! [`thomas_solve`](super::thomas_solve) cannot hit a zero pivot on a
//! correctly assembled system.
//!
//! ## The volume residual, and why nothing is renormalised
//!
//! The four transported quantities are the **primary state**; `α_g`, `α_l`,
//! `ρ_k` and `T_k` are *derived* from them and the pressure. Nothing forces
//! `α_g + α_l = 1` afterwards. The residual
//!
//! `R = α_g + α_l − 1 = m_g/ρ_g(p, h_g) + m_l/ρ_l(p, h_l) − 1`
//!
//! is instead fed back as a source into the next corrector's pressure equation
//! (`δp = R / C`, the Newton step on the constraint), so the loop drives it to
//! zero. Upstream renormalises instead — `MULES::limitSumCorr` plus an optional
//! final re-scaling, `phaseSystemSolve.C:580`, `:820-` — which is cheaper but
//! silently destroys the exact conservation of the transported masses. Here
//! **the masses are exactly conserved by construction and the constraint
//! violation is reported** as
//! [`TwoFluidReport::max_volume_residual`], with
//! [`MAX_VOLUME_RESIDUAL`] a hard refusal beyond which the step is rejected
//! rather than returned.
//!
//! ## Bounded α transport
//!
//! Study §9.3 asks for the 1-D analogue of MULES. What is here is its minimum:
//! first-order **donor-cell** phase-mass transport, which is monotone, plus a
//! **refusal** (not a clip) if `α_k` leaves `[0, 1]`. There is no flux limiter
//! and no interface compression. Donor-cell carries first-order numerical
//! diffusion of `α`, so a flashing front is smeared; that is stated here rather
//! than discovered in a plot.
//!
//! # Honest scope — what this solver does NOT do
//!
//! [`crate::multiphase_1d`] lists what applies to both 1-D solvers (no wall
//! heat transfer, no flow-regime map, no interfacial-area transport, HEM break
//! model). Specific to this one:
//!
//! - **Not validated against anything.** Every test in `two_fluid_tests.rs` is
//!   *verification* — closed forms, invariants and degenerate limits. No result
//!   from this solver has been compared with an experiment. The Edwards and
//!   Marviken cases are a separate piece of work (beads `op-s1a0`,
//!   `op-dt3.13`).
//! - **Turbulent dispersion is absent and that is a real omission**, not a
//!   simplification: it is a genuinely regularising `D ∇α` term that upstream
//!   can call on (study §5.1) and a 1-D area-averaged model, having no
//!   resolved turbulence, cannot. Lift, wall lubrication, surface tension and
//!   interface compression are omitted for *geometric* reasons (study §9.4) and
//!   nothing is lost by it; turbulent dispersion is different.
//! - **Granular phase pressure is deliberately not implemented.** It is
//!   identically zero for fluid phases upstream
//!   (`phaseCompressibleMomentumTransportModel.C:99-108`), so omitting it
//!   reproduces upstream's fluid-fluid behaviour exactly.
//! - **The virtual-mass material derivative is only partly implicit.**
//!   Upstream assembles the whole of `DU/Dt = ∂_t U + (U·∇)U` implicitly
//!   (`MovingPhaseModel.C:533-536`). Here the `∂_t` half is implicit (the
//!   `K_vm/Δt` entries of the block) and the `(U·∇)U` half is carried
//!   explicitly at old-time velocities. **The full virtual-mass force is still
//!   in the equation** — only the implicit/explicit split differs, which
//!   affects the stability of the solve and not the equations solved.
//! - **Wall friction is partitioned by volume fraction**, `F_k^wall = α_k ρ_k f_k |u_k| u_k / (2 D_h)`
//!   with each phase's own Reynolds number. There is no regime-dependent
//!   wall-friction split (which phase wets the wall), so this is crude wherever
//!   friction matters.
//! - **The `u ∂p/∂x` pressure-work term is omitted** from the energy equations,
//!   which carry `α_k ∂p/∂t` only — matching the equation set stated above and
//!   matching [`super::drift_flux`], so the two solvers can be compared in the
//!   equilibrium limit.
//! - **Boundary conditions are velocity-type only**
//!   ([`TwoFluidBoundary`]): closed, prescribed velocity, and an HEM choked
//!   outlet. There is no Dirichlet-pressure boundary, so the pressure floats on
//!   the compliance diagonal — right for a blowdown, wrong for a pipe fed from
//!   a plenum. [`super::drift_flux`]'s `ReservoirInlet` and `PressureOutlet`
//!   have no counterpart here yet.
//! - **The metastable bounds are the binding constraint on a fast transient.**
//!   [`super::properties::MAX_METASTABLE_LIQUID_SUPERHEAT`] is 30 K, and a
//!   depressurisation fast enough to superheat the liquid past that is
//!   **refused**, not extrapolated. Whether a given case stays inside it is
//!   decided by the interfacial closure set — chiefly the bubble diameter and
//!   the initial void fraction — and a case that trips the bound needs a
//!   different closure set or a nucleation model, not a looser bound.
//!
//! # Units
//!
//! Constructors and accessors are `uom`-typed. The marching loop carries raw
//! `f64` in strict SI — pascal, kelvin, `J/kg`, `kg/m³`, `m/s`, `N/m³`,
//! `W/m³`, `kg/(m³·s)` — because every one of them is read per cell per
//! corrector per step. Every raw-`f64` boundary says so.

use outram_foam_multiphase::two_fluid::RESIDUAL_ALPHA;

use uom::si::available_energy::joule_per_kilogram;
use uom::si::f64::{
    AvailableEnergy, Length, Mass, MassDensity, MassRate, Pressure, Ratio,
    ThermodynamicTemperature, Time, Velocity,
};
use uom::si::mass::kilogram;
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::mass_rate::kilogram_per_second;
use uom::si::pressure::pascal;
use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;
use uom::si::velocity::meter_per_second;

use super::geometry::Pipe1d;
use super::interfacial::{DispersedPhase, InterfacialCellState, InterfacialExchange};
use super::properties::{
    PhaseState, SaturatedProperties, SaturatedTransport, TwoPhaseState, P_MAX_IF97, P_MIN_IF97,
};
use super::thomas_solve;
use crate::TampinesError;

// ─────────────────────────────────────────────────────────────────────────────
//  Constants
// ─────────────────────────────────────────────────────────────────────────────

/// Default added-mass coefficient `C_vm` \[-\].
///
/// `0.5`, the potential-flow value for a sphere, and the value **every**
/// gas-liquid tutorial in the vendored OpenFOAM tree sets
/// (`bubbleColumn/constant/momentumTransfer`; see the source study §3.3, which
/// surveyed all 30 `multiphaseEuler` tutorials). It is a **modelling choice
/// that changes the answer**, not a measured constant: study §9.5 requires any
/// benchmark run on this solver to report a sensitivity over at least
/// `C_vm ∈ [0, 0.5]`.
pub const DEFAULT_VIRTUAL_MASS_COEFFICIENT: f64 = 0.5;

/// Default residual volume fraction `α_res` \[-\].
///
/// [`RESIDUAL_ALPHA`] (`1e-6`), the value the OpenFOAM tutorials use
/// (`bubbleColumn/constant/phaseProperties:30,43`). A **numerical device**, not
/// physics: it floors the momentum diagonal so the 2×2 coupling block stays
/// invertible as a phase vanishes, and it tapers the drag and virtual-mass
/// coupling to zero as the *other* phase vanishes. See [`PhaseCouplingBlock`]
/// for exactly where each use enters.
pub const DEFAULT_RESIDUAL_ALPHA: f64 = RESIDUAL_ALPHA;

/// Default inclusion (bubble) diameter `d` \[m\] used by
/// [`TwoFluid1d::bubbly`].
///
/// `1e-3` m. Every interfacial closure scales as `1/d²`, so this is the single
/// most influential number a caller supplies and the least justified — there is
/// no interfacial-area transport here, so it never responds to breakup,
/// coalescence or the flow. Stated as a **model parameter with no measured
/// provenance**.
pub const DEFAULT_BUBBLE_DIAMETER: f64 = 1.0e-3;

/// Default initial vapour volume fraction `α_g^0` \[-\] when the initial state
/// flashes subcooled.
///
/// `1e-4`. A six-equation model cannot start from `α_g` exactly zero: with no
/// vapour there is no interfacial area, so no interfacial heat transfer, so no
/// evaporation, and a depressurising cell superheats its liquid without limit
/// until [`super::properties::MAX_METASTABLE_LIQUID_SUPERHEAT`] refuses it.
/// Real system codes resolve this with a wall-nucleation source; there is none
/// here, so a small pre-existing void stands in for one.
///
/// This is a **model parameter with no measured provenance**, exactly like
/// [`super::drift_flux::DEFAULT_VAPOUR_RELAXATION_TIME`], and it is
/// consequential: it sets how fast the liquid can shed superheat at the start
/// of a transient. Any case whose answer moves with it must report that
/// sensitivity. The mass it adds is negligible — at 7 MPa,
/// `α_g ρ_g / ρ_m ≈ 1e-4 × 36.5 / 833 ≈ 4.4e-6` of the inventory.
pub const DEFAULT_INITIAL_VOID_FRACTION: f64 = 1.0e-4;

/// Relative finite-difference step used for the phase compressibilities
/// `ψ_k = ∂ρ_k/∂p|_{h_k}` \[-\].
pub const COMPRESSIBILITY_STEP: f64 = 1.0e-4;

/// The largest fraction of the donor phase's mass concentration that
/// interfacial mass transfer may move within one step \[-\].
///
/// `0.5`. A **numerical device**, and it exists for a specific, measured
/// reason rather than as generic caution.
///
/// With the interface at `T_sat`, the transferring mass carries the
/// **saturation** enthalpy of its side — `h_g^sat` leaving or joining the
/// vapour, `h_f^sat` leaving or joining the liquid — which is what makes the
/// interfacial energy balance `Q_g + Q_l + Γ h_fg = 0` cancel exactly. Solve
/// the phase energy update for the enthalpy that leaves behind when a fraction
/// `f` of the phase transfers away:
///
/// `h_k^{n+1} = h_k^sat + (h_k^* − h_k^sat)/(1 − f) + Δt Q_k / (m_k^* (1 − f))`
///
/// The departure from saturation is amplified by `1/(1 − f)`. That is real
/// physics — mass leaving at the saturation enthalpy makes the remainder
/// *further* from saturation — but as `f → 1` it is unbounded, and it was
/// measured doing exactly that: a 57 K subcooled Edwards-like cell holding
/// `α_g = 1e-3` of saturated steam at 7 MPa condenses its entire vapour
/// inventory within one 30 µs step (`f = 1 − 1e-3`), and the amplification
/// produced `h_g = −1.7079e7 J/kg` on the very first step (measured
/// 2026-08-12, release). The property layer then refused it, correctly, as a
/// vapour 30 K past its metastable bound — a correct refusal, of a number that
/// should never have been formed.
///
/// Capping `f` at `0.5` bounds the amplification at `2` per step. A phase that
/// genuinely wants to vanish still does, geometrically, over the following
/// steps, and once it reaches
/// [`PHASE_FLOOR_TRIGGER`] × its residual-alpha floor it is reset to the
/// saturated state outright. Every activation is counted into
/// [`TwoFluidReport::mass_transfer_limited_cells`].
pub const MAX_MASS_TRANSFER_FRACTION_PER_STEP: f64 = 0.5;

/// Multiple of the residual-alpha mass floor `α_res ρ_k` at or below which a
/// phase is treated as **absent** and reset to its saturated state \[-\].
///
/// `2.0`. Strictly greater than 1 on purpose: the mass-transfer limiter drives
/// a vanishing phase *to* its floor, and floating-point rounding in the scale
/// factor leaves the result a few ULPs either side of it. A bare `m ≤ floor`
/// test therefore fires only about half the time, and the half that misses is
/// exactly where the `1/(1 − f)` amplification above is largest. Measured
/// consequence of getting this wrong: the `−1.7079e7 J/kg` vapour enthalpy
/// recorded at [`MAX_MASS_TRANSFER_FRACTION_PER_STEP`].
///
/// At `α_res = 1e-6` the reset happens at `α_k = 2e-6`, where the phase holds
/// about `7e-5 kg/m³` of steam at 7 MPa — eight decades below the mixture
/// density, so the mass and energy the reset invents are negligible but not
/// zero. Every activation is counted into
/// [`TwoFluidReport::residual_alpha_floor_events`].
pub const PHASE_FLOOR_TRIGGER: f64 = 2.0;

/// Default number of outer correctors per step.
///
/// `8`, matching [`super::drift_flux::DEFAULT_OUTER_CORRECTORS`]. Each
/// corrector re-solves the pressure equation against the volume residual left
/// by the previous one; see the module docs.
pub const DEFAULT_OUTER_CORRECTORS: usize = 8;

/// Default pressure under-relaxation `α_p` \[-\], applied once per outer
/// corrector as `p ← p_prev + α_p (p_solved − p_prev)`.
///
/// `0.7`, matching [`super::drift_flux::DEFAULT_PRESSURE_UNDER_RELAXATION`].
pub const DEFAULT_PRESSURE_UNDER_RELAXATION: f64 = 0.7;

/// Default outer-corrector convergence tolerance on `max |Δp|` \[Pa\].
///
/// `1.0` Pa — six orders below the 7 MPa initial pressure of a blowdown.
pub const DEFAULT_OUTER_TOLERANCE: f64 = 1.0;

/// Default outer-corrector convergence tolerance on the volume residual
/// `max |α_g + α_l − 1|` \[-\].
///
/// `1e-9`. Tight, because the residual is a *constraint* the pressure equation
/// exists to enforce rather than an approximation being tolerated.
pub const DEFAULT_VOLUME_RESIDUAL_TOLERANCE: f64 = 1.0e-9;

/// The volume-constraint violation beyond which [`TwoFluid1d::step`] **refuses**
/// the step \[-\].
///
/// `1e-4`. Past this the phases no longer fill the cell to any useful accuracy,
/// which means the outer correctors did not converge, which means the
/// pressure-velocity coupling has failed. Continuing from it would produce a
/// plausible-looking density field that is wrong by the residual, so it is
/// reported as [`TampinesError::Numerical`] instead.
pub const MAX_VOLUME_RESIDUAL: f64 = 1.0e-4;

// ─────────────────────────────────────────────────────────────────────────────
//  Boundary conditions
// ─────────────────────────────────────────────────────────────────────────────

/// What closes an end of the pipe.
///
/// Enum dispatch per the workspace rule: the set of 1-D end conditions is
/// closed, and adding one must force every `match` to be revisited.
///
/// **Every variant is a *velocity* boundary.** That is a deliberate restriction
/// of the first implementation and not an oversight: a velocity face carries no
/// pressure sensitivity, so it stays out of the pressure matrix entirely and
/// the pressure floats on the compliance diagonal. That is exactly what a
/// closed-vessel blowdown wants and exactly what a pipe fed from a plenum does
/// not. [`super::drift_flux::AxialBoundary`]'s `ReservoirInlet` and
/// `PressureOutlet` have no counterpart here yet.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TwoFluidBoundary {
    /// A rigid closed end: `u_g = u_l = 0`. The `x = 0` end of the
    /// Edwards–O'Brien pipe.
    Closed,

    /// A prescribed face velocity \[m/s\], positive in `+x`, imposed on
    /// **both** phases.
    ///
    /// Inflow through this boundary re-injects the adjacent cell's state
    /// (zero-gradient donor), because the variant carries no description of
    /// what is entering. A case with sustained inflow needs a boundary that
    /// does; there is none yet.
    PrescribedVelocity(f64),

    /// A **choked (critical) outlet** through a break of area
    /// `area_fraction × A`.
    ///
    /// Each step the adjacent cell's mixture `(p, h_m)` is handed to the
    /// crate's HEM critical-flow dispatcher for the throat mass flux `G*`, and
    /// the equivalent full-face velocity `u = G* × area_fraction / ρ_m` is
    /// imposed on **both** phases.
    ///
    /// **Two modelling inconsistencies, both deliberate and both stated.**
    ///
    /// 1. *The break is HEM even though the pipe is six-equation*, exactly as
    ///    [`super::drift_flux::AxialBoundary::ChokedOutlet`] documents at
    ///    length. The critical-flow dispatcher is the piece of this crate
    ///    actually exercised against a reference; substituting an unvalidated
    ///    two-fluid choking model would trade a known inconsistency for an
    ///    unknown one. Read that variant's doc comment for the dispatcher's
    ///    real V&V status, including that Marviken is **not** gated.
    /// 2. *The break imposes no slip*, so the phases leave at the same
    ///    velocity even where the pipe has developed slip. A real break
    ///    separates the phases; nothing here does.
    ChokedOutlet {
        /// Break area as a fraction of the pipe flow area, in `(0, 1]`.
        area_fraction: f64,
        /// Ambient / containment back-pressure \[Pa\]. The outlet unchokes
        /// once the critical throat pressure falls below it.
        ambient_pressure: f64,
    },
}

// ─────────────────────────────────────────────────────────────────────────────
//  The 2x2 implicit drag + virtual-mass coupling block
// ─────────────────────────────────────────────────────────────────────────────

/// Everything the 2×2 phase-coupling block at one face is assembled from.
///
/// Raw `f64` in strict SI throughout — this is a marching-loop boundary; every
/// field documents its unit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PhaseCouplingInputs {
    /// Vapour volume fraction at the face `α_g` \[-\].
    pub alpha_g: f64,
    /// Liquid volume fraction at the face `α_l` \[-\].
    pub alpha_l: f64,
    /// Vapour density at the face `ρ_g` \[kg/m³\], strictly positive.
    pub rho_g: f64,
    /// Liquid density at the face `ρ_l` \[kg/m³\], strictly positive.
    pub rho_l: f64,
    /// Volumetric drag coefficient `K_d` \[kg/(m³·s)\] from the traced-back
    /// [`outram_foam_multiphase::two_fluid::DragModel`], via
    /// [`super::interfacial::InterfacialSources::volumetric_drag_coefficient`].
    /// Must be `≥ 0`.
    pub k_d: f64,
    /// Added-mass coefficient `C_vm` \[-\], `≥ 0`. `0` disables virtual mass
    /// entirely and leaves a pure drag block.
    pub c_vm: f64,
    /// Which phase is dispersed — this decides which density and which volume
    /// fraction enter `K_vm`; see [`virtual_mass_coefficient`].
    pub dispersed: DispersedPhase,
    /// Residual volume fraction `α_res` \[-\], in `[0, 1)`.
    pub residual_alpha: f64,
    /// Timestep `Δt` \[s\], strictly positive.
    pub dt: f64,
}

/// The **volumetric virtual-mass coefficient** `K_vm` \[kg/m³\].
///
/// `K_vm = max(α_d, α_res) · C_vm · ρ_c`, with `α_d` the *dispersed*-phase
/// volume fraction and `ρ_c` the *continuous*-phase density.
///
/// # Provenance
///
/// Transcribed from OpenFOAM's
/// `dispersedVirtualMassModel.C:51-67` (the `max(α_d, α_res) · K_i` form) and
/// `constantVirtualMassCoefficient.C:71-79` (`K_i = C_vm ρ_c`), via
/// `crates/tampines/docs/six-equation-regularisation.md` §3.2.
///
/// It is written down here rather than consumed from the reference crate
/// because [`outram_foam_multiphase::two_fluid::InterfacialForce::VirtualMass`]
/// is an unported scaffold that returns `MultiphaseError::NotImplemented`.
/// **This is the one closure in this file that is not traced through a call
/// into the reference crate**, and this doc comment is the record of that.
/// When the reference gains the closure, replace this function's body with a
/// call to it and nothing else changes.
///
/// # Units and ranges
///
/// `alpha_dispersed` \[-\] in `[0, 1]`, `c_vm` \[-\] `≥ 0`, `rho_continuous`
/// \[kg/m³\] `> 0`, `residual_alpha` \[-\] in `[0, 1)`. Returns \[kg/m³\];
/// multiplied by a material acceleration \[m/s²\] it gives a force per unit
/// volume \[N/m³\], as required.
#[must_use]
pub fn virtual_mass_coefficient(
    alpha_dispersed: f64,
    rho_continuous: f64,
    c_vm: f64,
    residual_alpha: f64,
) -> f64 {
    alpha_dispersed.max(residual_alpha) * c_vm * rho_continuous
}

/// The **2×2 implicit phase-coupling block** at one face — drag and virtual
/// mass together, inside one matrix, exactly as upstream forms them.
///
/// # What it is
///
/// Write each phase's discrete face momentum equation as
///
/// `A_k u_k^{n+1} = b_k − α_k ∂p/∂x`
///
/// with `A_k` the momentum diagonal \[kg/(m³·s)\] and `b_k` everything
/// explicit \[N/m³\]. Drag and virtual mass then couple the two rows:
///
/// - **drag** contributes `+K̃^d_k` to row `k`'s diagonal and `−K̃^d_k` to its
///   off-diagonal (`momentumTransferSystem.C:617-634`);
/// - **virtual mass** contributes `+K̃^vm_k A^D_k` to the diagonal and
///   `−K̃^vm_k A^D_j` to the off-diagonal
///   (`momentumTransferSystem.C:704-762`);
///
/// where `A^D` is the diagonal of the material-derivative operator `DU/Dt`.
/// This implementation takes `A^D = 1/Δt` for both phases — the `∂_t` half of
/// `DU/Dt` — and carries the `(U·∇)U` half explicitly in `b_k`; see the module
/// docs, "Honest scope". With `A^D` equal for both phases the diagonal and
/// off-diagonal virtual-mass entries have the same magnitude, so drag and
/// virtual mass fold into a single coupling strength per row:
///
/// `c_k = [α_j / max(α_j, α_res)] · (K_d + K_vm / Δt)`
///
/// The `α_j / max(α_j, α_res)` factor — with `j` the **other** phase — is
/// upstream's vanishing-phase taper (`momentumTransferSystem.C:617-620`,
/// `:723-727`): exactly 1 wherever the other phase is present, tapering to 0
/// as it disappears.
///
/// The block is then
///
/// `[[A_g + c_g, −c_g], [−c_l, A_l + c_l]]`
///
/// with the residual-alpha-floored diagonals
/// `A_k = max(α_k, α_res) ρ_k / Δt` (`cellPressureCorrector.C:82-91`).
///
/// # Why it is invertible
///
/// `det = (A_g + c_g)(A_l + c_l) − c_g c_l = A_g A_l + A_g c_l + A_l c_g`.
/// Every term is a product of non-negative numbers, and `A_g A_l > 0` strictly
/// because the residual-alpha flooring keeps both diagonals positive even where
/// a phase has vanished. So `det > 0` always — no linear-algebra library, no
/// pivoting, no singularity at a blowdown front. Upstream reaches the same
/// answer through a per-cell LU (`momentumTransferSystem.C:461-499`); for two
/// phases that collapses to this closed form, which is the classical two-phase
/// **partial elimination algorithm**.
///
/// # Degenerate limits, which are the reason for the taper
///
/// - `α_g → 0`: `c_l → 0`, so row `l` decouples completely —
///   [`solve`](Self::solve) returns `u_l = b_l / A_l` and
///   [`pressure_coefficients`](Self::pressure_coefficients) returns
///   `d_l = α_l / A_l`, the single-phase liquid answer. A vanishing phase
///   cannot contaminate the phase that is present.
/// - `α_l → 0`: the mirror image.
/// - `K_d → ∞`: `u_g − u_l = (A_l b_g − A_g b_l)/det → 0`, the no-slip limit.
///
/// # Momentum conservation, and where the taper breaks it
///
/// Away from the vanishing limits both tapers are exactly 1, `c_g = c_l`, and
/// the off-diagonals are equal — so the drag and virtual-mass forces on the two
/// phases are exactly equal and opposite and mixture momentum is conserved to
/// machine precision. Inside the taper band (`α < α_res`) they are not, by
/// construction. That is the price of the numerical device and it is stated
/// rather than hidden.
///
/// # Units
///
/// Raw `f64`, strict SI. All four matrix entries are \[kg/(m³·s)\]; the tapered
/// virtual-mass coefficients are \[kg/m³\].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PhaseCouplingBlock {
    /// Row vapour, column vapour: `A_g + c_g` \[kg/(m³·s)\].
    pub vapour_vapour: f64,
    /// Row vapour, column liquid: `−c_g` \[kg/(m³·s)\].
    pub vapour_liquid: f64,
    /// Row liquid, column vapour: `−c_l` \[kg/(m³·s)\].
    pub liquid_vapour: f64,
    /// Row liquid, column liquid: `A_l + c_l` \[kg/(m³·s)\].
    pub liquid_liquid: f64,
    /// The floored vapour momentum diagonal `A_g = max(α_g, α_res) ρ_g / Δt`
    /// \[kg/(m³·s)\], kept so a caller can see how much of the diagonal is
    /// inertia and how much is coupling.
    pub vapour_diagonal: f64,
    /// The floored liquid momentum diagonal `A_l` \[kg/(m³·s)\].
    pub liquid_diagonal: f64,
    /// Tapered virtual-mass coefficient for the vapour row,
    /// `K̃^vm_g = [α_l/max(α_l, α_res)] K_vm` \[kg/m³\]. Multiplies
    /// `H^D_g − H^D_l` in the explicit remainder — see the module docs.
    pub tapered_virtual_mass_vapour: f64,
    /// Tapered virtual-mass coefficient for the liquid row, `K̃^vm_l`
    /// \[kg/m³\].
    pub tapered_virtual_mass_liquid: f64,
}

impl PhaseCouplingBlock {
    /// Assemble the block from one face's state.
    ///
    /// # Errors
    ///
    /// [`TampinesError::InvalidInput`] for a non-finite entry, a non-positive
    /// density or timestep, a negative `K_d` or `C_vm`, a `residual_alpha`
    /// outside `[0, 1)`, or a volume fraction outside `[0, 1]`. Nothing is
    /// clamped into range.
    pub fn assemble(inputs: PhaseCouplingInputs) -> Result<Self, TampinesError> {
        let PhaseCouplingInputs {
            alpha_g,
            alpha_l,
            rho_g,
            rho_l,
            k_d,
            c_vm,
            dispersed,
            residual_alpha,
            dt,
        } = inputs;

        for (name, value) in [
            ("alpha_g", alpha_g),
            ("alpha_l", alpha_l),
            ("rho_g", rho_g),
            ("rho_l", rho_l),
            ("k_d", k_d),
            ("c_vm", c_vm),
            ("residual_alpha", residual_alpha),
            ("dt", dt),
        ] {
            if !value.is_finite() {
                return Err(TampinesError::InvalidInput(format!(
                    "phase-coupling input `{name}` is not finite (got {value})"
                )));
            }
        }
        if !(rho_g > 0.0) || !(rho_l > 0.0) {
            return Err(TampinesError::InvalidInput(format!(
                "phase densities must be > 0 kg/m^3 (got rho_g = {rho_g}, rho_l = {rho_l})"
            )));
        }
        if !(dt > 0.0) {
            return Err(TampinesError::InvalidInput(format!(
                "timestep must be > 0 s (got {dt})"
            )));
        }
        if k_d < 0.0 || c_vm < 0.0 {
            return Err(TampinesError::InvalidInput(format!(
                "drag coefficient and C_vm must be >= 0 (got K_d = {k_d}, C_vm = {c_vm})"
            )));
        }
        if !(0.0..1.0).contains(&residual_alpha) {
            return Err(TampinesError::InvalidInput(format!(
                "residual alpha must be in [0, 1) (got {residual_alpha})"
            )));
        }
        if !(0.0..=1.0).contains(&alpha_g) || !(0.0..=1.0).contains(&alpha_l) {
            return Err(TampinesError::InvalidInput(format!(
                "face volume fractions must be in [0, 1] (got alpha_g = {alpha_g}, \
                 alpha_l = {alpha_l}); refused rather than clipped"
            )));
        }

        // K_vm reads the DISPERSED fraction and the CONTINUOUS density.
        let k_vm = match dispersed {
            DispersedPhase::Vapour => {
                virtual_mass_coefficient(alpha_g, rho_l, c_vm, residual_alpha)
            }
            DispersedPhase::Liquid => {
                virtual_mass_coefficient(alpha_l, rho_g, c_vm, residual_alpha)
            }
        };

        // Vanishing-phase taper, keyed on the OTHER phase.
        let taper_g = alpha_l / alpha_l.max(residual_alpha);
        let taper_l = alpha_g / alpha_g.max(residual_alpha);

        // A^D = 1/dt for both phases, so drag and virtual mass fold together.
        let coupling = k_d + k_vm / dt;
        let c_g = taper_g * coupling;
        let c_l = taper_l * coupling;

        let a_g = alpha_g.max(residual_alpha) * rho_g / dt;
        let a_l = alpha_l.max(residual_alpha) * rho_l / dt;

        Ok(Self {
            vapour_vapour: a_g + c_g,
            vapour_liquid: -c_g,
            liquid_vapour: -c_l,
            liquid_liquid: a_l + c_l,
            vapour_diagonal: a_g,
            liquid_diagonal: a_l,
            tapered_virtual_mass_vapour: taper_g * k_vm,
            tapered_virtual_mass_liquid: taper_l * k_vm,
        })
    }

    /// The determinant `A_g A_l + A_g c_l + A_l c_g` \[kg²/(m⁶·s²)\].
    ///
    /// Evaluated in that grouped form rather than as `ad − bc`, because the
    /// grouped form is a sum of non-negative terms and therefore cannot lose
    /// its sign to cancellation. Strictly positive for any block
    /// [`assemble`](Self::assemble) accepts.
    #[must_use]
    pub fn determinant(&self) -> f64 {
        let c_g = -self.vapour_liquid;
        let c_l = -self.liquid_vapour;
        self.vapour_diagonal * self.liquid_diagonal
            + self.vapour_diagonal * c_l
            + self.liquid_diagonal * c_g
    }

    /// Solve `M u = b` for the two face velocities \[m/s\].
    ///
    /// `b_vapour`, `b_liquid` are the explicit right-hand sides \[N/m³\]. The
    /// closed-form inverse is used, not a factorisation.
    ///
    /// # Errors
    ///
    /// [`TampinesError::Numerical`] if the determinant is not strictly
    /// positive and finite — which cannot happen for a block
    /// [`assemble`](Self::assemble) produced, so it diagnoses a hand-built
    /// block rather than a physical condition.
    pub fn solve(&self, b_vapour: f64, b_liquid: f64) -> Result<(f64, f64), TampinesError> {
        let det = self.determinant();
        if !(det > 0.0) || !det.is_finite() {
            return Err(TampinesError::Numerical(format!(
                "phase-coupling block determinant is {det}, not strictly positive: the \
                 residual-alpha flooring that guarantees invertibility has been bypassed"
            )));
        }
        let u_g = (self.liquid_liquid * b_vapour - self.vapour_liquid * b_liquid) / det;
        let u_l = (-self.liquid_vapour * b_vapour + self.vapour_vapour * b_liquid) / det;
        Ok((u_g, u_l))
    }

    /// The per-phase pressure sensitivities `d_k = Σ_m (M⁻¹)_{km} α_m`
    /// \[m³·s/kg\].
    ///
    /// Both phases see the *same* face pressure gradient (this is a
    /// single-pressure model), so the pressure correction is
    /// `u_k ← u*_k − d_k (p_R − p_L)/Δx`, and these are upstream's
    /// `invADVfs & movingAlphafs` (`cellPressureCorrector.C:145`).
    ///
    /// Both are strictly positive for a block [`assemble`](Self::assemble)
    /// produced with a non-zero volume fraction, because every entry of the
    /// adjugate is non-negative.
    ///
    /// # Errors
    ///
    /// As [`solve`](Self::solve).
    pub fn pressure_coefficients(
        &self,
        alpha_g: f64,
        alpha_l: f64,
    ) -> Result<(f64, f64), TampinesError> {
        let det = self.determinant();
        if !(det > 0.0) || !det.is_finite() {
            return Err(TampinesError::Numerical(format!(
                "phase-coupling block determinant is {det}, not strictly positive"
            )));
        }
        let d_g = (self.liquid_liquid * alpha_g - self.vapour_liquid * alpha_l) / det;
        let d_l = (-self.liquid_vapour * alpha_g + self.vapour_vapour * alpha_l) / det;
        Ok((d_g, d_l))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
//  Report
// ─────────────────────────────────────────────────────────────────────────────

/// Per-step diagnostics from [`TwoFluid1d::step`].
///
/// Returned rather than logged, so a case can assert on it and a V&V test can
/// record measured values. Raw `f64` in strict SI throughout.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TwoFluidReport {
    /// Simulated time at the end of the step \[s\].
    pub time: f64,
    /// Mixture mass flow through the right-hand boundary \[kg/s\], positive
    /// outward: `(m_g u_g + m_l u_l) A` at the last face.
    pub outlet_mass_flow: f64,
    /// Largest void fraction anywhere \[-\].
    pub max_void_fraction: f64,
    /// Largest phase-temperature difference `|T_g − T_l|` anywhere \[K\] — the
    /// quantity that justifies a six-equation model over drift flux.
    pub max_thermal_nonequilibrium: f64,
    /// Smallest void fraction anywhere \[-\].
    pub min_void_fraction: f64,
    /// Largest phase slip `|u_g − u_l|` at any face \[m/s\] — the quantity that
    /// justifies a six-equation model over HEM.
    pub max_slip: f64,
    /// Total mass currently in the pipe \[kg\],
    /// `Σ_i (m_g + m_l)_i × V_cell`.
    pub inventory: f64,
    /// Largest material Courant number `max_k |u_k| Δt / Δx` \[-\].
    ///
    /// The stability figure that matters for the explicit donor-cell transport.
    /// The *acoustic* Courant number is not limiting, because the pressure
    /// solve is implicit. A value above 1 means transport has stepped past a
    /// whole cell and the answer is not to be trusted.
    pub max_courant: f64,
    /// Largest volume-constraint residual `max_i |α_g + α_l − 1|` \[-\] left at
    /// the end of the outer-corrector loop.
    ///
    /// Zero to round-off means the correctors converged. See the module docs
    /// for why this is reported rather than renormalised away, and
    /// [`MAX_VOLUME_RESIDUAL`] for where it becomes a refusal.
    pub max_volume_residual: f64,
    /// How many outer correctors were actually taken \[-\].
    pub outer_correctors_used: usize,
    /// Whether either boundary was choked this step.
    pub outlet_choked: bool,
    /// How many cells had their interfacial mass transfer **rate-limited** this
    /// step \[-\].
    ///
    /// Non-zero means `Γ` would have driven a phase mass below its
    /// residual-alpha floor within the step and was scaled back (together with
    /// both interfacial heat fluxes, so the interfacial energy balance stays
    /// exact). It is a numerical device and it is counted so it cannot act
    /// invisibly — see [`TwoFluid1d::step`].
    pub mass_transfer_limited_cells: usize,
    /// How many cells had a phase mass floored at `α_res ρ_k` this step \[-\].
    ///
    /// Non-zero means donor-cell transport drained a phase out of a cell
    /// faster than the limiter could catch, and the residual-alpha floor
    /// supplied the missing inventory. The mass it invents is of order
    /// `α_res ρ_k`; at `α_res = 1e-6` that is `≈ 4e-5 kg/m³` for steam at
    /// 7 MPa. Counted for the same reason.
    pub residual_alpha_floor_events: usize,
}

// ─────────────────────────────────────────────────────────────────────────────
//  The solver
// ─────────────────────────────────────────────────────────────────────────────

/// A 1-D transient **six-equation two-fluid** solver for compressible
/// steam/water pipe flow.
///
/// # Layout
///
/// **Staggered**: scalars (`p`, `m_g`, `m_l`, `h_g`, `h_l`, and everything
/// derived from them) at cell centres, both phase velocities at faces.
/// Staggering rather than collocation because in 1-D it removes
/// pressure-velocity checkerboarding *by construction* — no Rhie-Chow
/// interpolation needed — and it is what the reference system codes do. Face
/// `j` is the left face of cell `j`; faces `0` and `n_cells` are the ends.
///
/// # The primary state, and what is derived from it
///
/// The four **transported** quantities are `m_g = α_g ρ_g`, `m_l = α_l ρ_l`
/// \[kg/m³\] and the two phase enthalpies `h_g`, `h_l` \[J/kg\]; with the
/// pressure `p` \[Pa\] those are the state. `α_g`, `α_l`, `ρ_g`, `ρ_l`, `T_g`
/// and `T_l` are **derived** — `ρ_k = ρ_k(p, h_k)` from
/// [`PhaseState`], then `α_k = m_k / ρ_k`. Nothing forces
/// `α_g + α_l = 1` after the fact; the pressure equation enforces it and the
/// leftover is reported as [`TwoFluidReport::max_volume_residual`].
///
/// # Units
///
/// Constructors and accessors are `uom`-typed. Internal state is raw `f64` in
/// strict SI: pascal, `J/kg`, `kg/m³`, `m/s`, `K`, `[-]`.
///
/// # What a validation case needs from this API
///
/// A benchmark harness (the Edwards–O'Brien and Marviken cases are separate
/// work, beads `op-s1a0` / `op-dt3.13`) needs exactly:
/// [`new`](Self::new) or [`bubbly`](Self::bubbly) to build it,
/// [`set_temperature_profile`](Self::set_temperature_profile) for a
/// non-isothermal initial condition,
/// [`set_left_boundary`](Self::set_left_boundary) /
/// [`set_right_boundary`](Self::set_right_boundary) for a closed end and a
/// choked break, [`step`](Self::step) in a loop, and the read-only accessors
/// [`pressure`](Self::pressure), [`void_fraction`](Self::void_fraction),
/// [`vapour_temperature`](Self::vapour_temperature),
/// [`liquid_temperature`](Self::liquid_temperature) and
/// [`slip_velocity`](Self::slip_velocity) to compare against gauge data.
/// [`set_virtual_mass_coefficient`](Self::set_virtual_mass_coefficient) and
/// [`set_initial_void_fraction`](Self::set_initial_void_fraction) are the two
/// knobs whose sensitivity such a case is obliged to report.
pub struct TwoFluid1d {
    pipe: Pipe1d,
    exchange: InterfacialExchange,

    /// Cell pressure \[Pa\], length `n_cells`.
    p: Vec<f64>,
    /// Cell vapour mass concentration `m_g = α_g ρ_g` \[kg/m³\].
    m_g: Vec<f64>,
    /// Cell liquid mass concentration `m_l = α_l ρ_l` \[kg/m³\].
    m_l: Vec<f64>,
    /// Cell vapour specific enthalpy \[J/kg\].
    h_g: Vec<f64>,
    /// Cell liquid specific enthalpy \[J/kg\].
    h_l: Vec<f64>,

    /// Derived: vapour volume fraction \[-\].
    alpha_g: Vec<f64>,
    /// Derived: liquid volume fraction \[-\].
    alpha_l: Vec<f64>,
    /// Derived: vapour density \[kg/m³\].
    rho_g: Vec<f64>,
    /// Derived: liquid density \[kg/m³\].
    rho_l: Vec<f64>,
    /// Derived: vapour temperature \[K\].
    t_g: Vec<f64>,
    /// Derived: liquid temperature \[K\].
    t_l: Vec<f64>,

    /// Per-cell saturated-property cache.
    sat: Vec<SaturatedProperties>,
    /// Per-cell saturated conduction-property cache.
    transport: Vec<SaturatedTransport>,

    /// Vapour face velocity \[m/s\], length `n_cells + 1`.
    u_g: Vec<f64>,
    /// Liquid face velocity \[m/s\], length `n_cells + 1`.
    u_l: Vec<f64>,

    dt: f64,
    time: f64,
    c_vm: f64,
    residual_alpha: f64,
    initial_void_fraction: f64,
    left: TwoFluidBoundary,
    right: TwoFluidBoundary,
    n_outer_correctors: usize,
    p_under_relaxation: f64,
    outer_tolerance: f64,
    volume_residual_tolerance: f64,
}

impl TwoFluid1d {
    /// Build a solver on `pipe`, initialised to a uniform `(p, T)` state.
    ///
    /// # Arguments
    ///
    /// - `pipe` — geometry and mesh.
    /// - `exchange` — the interfacial closure set (drag + the two-resistance
    ///   heat-transfer pair + inclusion diameter), taken from
    ///   [`super::interfacial`], which in turn takes its correlations from the
    ///   3-D reference. See [`bubbly`](Self::bubbly) for the conventional
    ///   bubbly-flow set.
    /// - `pressure` — uniform initial pressure. Must be inside IAPWS-IF97's
    ///   range and **below** the critical pressure, since the interfacial
    ///   closures place the interface at `T_sat(p)`.
    /// - `temperature` — uniform initial temperature, flashed through IF97 for
    ///   the enthalpy, so a subcooled initial state really is subcooled rather
    ///   than nominally so.
    /// - `dt` — fixed timestep.
    ///
    /// # The initial phase split
    ///
    /// The `(p, T)` pair is flashed with [`TwoPhaseState::flash`]. Inside the
    /// dome that gives `α_g` directly, and the phases start on the saturation
    /// line (`h_g = h_g^sat`, `h_l = h_f^sat`). Outside it, the void fraction
    /// is set to [`initial_void_fraction`](Self::initial_void_fraction) —
    /// **not** zero — for the reason given at
    /// [`DEFAULT_INITIAL_VOID_FRACTION`]: a six-equation model with no vapour
    /// has no interfacial area and therefore cannot flash. A subcooled start
    /// keeps its subcooled `h_l` and gives the token vapour `h_g^sat`; a
    /// superheated start keeps its superheated `h_g` and gives the token
    /// liquid `h_f^sat`.
    ///
    /// # Errors
    ///
    /// [`TampinesError::Unphysical`] if the initial state is outside IF97's
    /// range; [`TampinesError::InvalidInput`] for a non-positive timestep.
    pub fn new(
        pipe: Pipe1d,
        exchange: InterfacialExchange,
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
        let transport0 = transport_at(p0)?;

        let mut solver = Self {
            pipe,
            exchange,
            p: vec![p0; n],
            m_g: vec![0.0; n],
            m_l: vec![0.0; n],
            h_g: vec![sat0.h_g; n],
            h_l: vec![sat0.h_f; n],
            alpha_g: vec![0.0; n],
            alpha_l: vec![0.0; n],
            rho_g: vec![sat0.rho_g; n],
            rho_l: vec![sat0.rho_f; n],
            t_g: vec![sat0.t_sat; n],
            t_l: vec![sat0.t_sat; n],
            sat: vec![sat0; n],
            transport: vec![transport0; n],
            u_g: vec![0.0; n + 1],
            u_l: vec![0.0; n + 1],
            dt: dt_s,
            time: 0.0,
            c_vm: DEFAULT_VIRTUAL_MASS_COEFFICIENT,
            residual_alpha: DEFAULT_RESIDUAL_ALPHA,
            initial_void_fraction: DEFAULT_INITIAL_VOID_FRACTION,
            left: TwoFluidBoundary::Closed,
            right: TwoFluidBoundary::Closed,
            n_outer_correctors: DEFAULT_OUTER_CORRECTORS,
            p_under_relaxation: DEFAULT_PRESSURE_UNDER_RELAXATION,
            outer_tolerance: DEFAULT_OUTER_TOLERANCE,
            volume_residual_tolerance: DEFAULT_VOLUME_RESIDUAL_TOLERANCE,
        };

        let t0 = temperature.get::<kelvin>();
        for i in 0..n {
            solver.initialise_cell(i, p0, t0)?;
        }
        Ok(solver)
    }

    /// Build a solver with the conventional **bubbly-flow** closure set —
    /// Schiller-Naumann drag, Ranz-Marshall on the continuous (liquid) side,
    /// spherical conduction on the dispersed (vapour) side — at
    /// [`DEFAULT_BUBBLE_DIAMETER`].
    ///
    /// A convenience, not a recommendation: it inherits every limitation
    /// [`super::interfacial`] documents, in particular that the bubbly closures
    /// keep being used at void fractions where bubbly flow no longer exists.
    ///
    /// # Errors
    ///
    /// As [`new`](Self::new).
    pub fn bubbly(
        pipe: Pipe1d,
        pressure: Pressure,
        temperature: ThermodynamicTemperature,
        dt: Time,
    ) -> Result<Self, TampinesError> {
        let exchange = InterfacialExchange::bubbly(Length::new::<uom::si::length::meter>(
            DEFAULT_BUBBLE_DIAMETER,
        ))?;
        Self::new(pipe, exchange, pressure, temperature, dt)
    }

    /// Set one cell's state from a uniform `(p, T)`, applying the initial phase
    /// split described at [`new`](Self::new).
    fn initialise_cell(&mut self, i: usize, p: f64, t: f64) -> Result<(), TampinesError> {
        let sat = SaturatedProperties::at(p)?;
        let h_mixture = enthalpy_at_pt(p, t, sat)?;
        let flashed = TwoPhaseState::flash(p, h_mixture, sat)?;

        let (alpha_g, h_g, h_l) = if flashed.void_fraction <= self.initial_void_fraction {
            // Subcooled or barely-boiling: token vapour on the saturation line,
            // liquid keeps the enthalpy the (p, T) flash gave it.
            (self.initial_void_fraction, sat.h_g, h_mixture.min(sat.h_f))
        } else if flashed.void_fraction >= 1.0 - self.initial_void_fraction {
            // Superheated: token liquid on the saturation line.
            (
                1.0 - self.initial_void_fraction,
                h_mixture.max(sat.h_g),
                sat.h_f,
            )
        } else {
            (flashed.void_fraction, sat.h_g, sat.h_f)
        };

        let vapour = PhaseState::vapour_at(p, h_g, sat)?;
        let liquid = PhaseState::liquid_at(p, h_l, sat)?;

        self.p[i] = p;
        self.sat[i] = sat;
        self.transport[i] = transport_at(p)?;
        self.alpha_g[i] = alpha_g;
        self.alpha_l[i] = 1.0 - alpha_g;
        self.rho_g[i] = vapour.density;
        self.rho_l[i] = liquid.density;
        self.t_g[i] = vapour.temperature;
        self.t_l[i] = liquid.temperature;
        self.m_g[i] = alpha_g * vapour.density;
        self.m_l[i] = (1.0 - alpha_g) * liquid.density;
        self.h_g[i] = h_g;
        self.h_l[i] = h_l;
        Ok(())
    }

    /// Overwrite the cell temperatures from an axial profile, re-deriving the
    /// whole cell state at each cell.
    ///
    /// Needed by any case whose initial condition is not isothermal. For
    /// Edwards–O'Brien the Hendrie non-isothermal profile is the single most
    /// important modelling detail, so this is not a convenience.
    ///
    /// # Errors
    ///
    /// [`TampinesError::InvalidInput`] if the slice is not `n_cells` long;
    /// [`TampinesError::Unphysical`] if a state leaves IF97's range.
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
            let p = self.p[i];
            let t = temperatures[i].get::<kelvin>();
            self.initialise_cell(i, p, t)?;
        }
        Ok(())
    }

    /// Set the boundary condition at the `x = 0` end.
    pub fn set_left_boundary(&mut self, bc: TwoFluidBoundary) {
        self.left = bc;
    }

    /// Set the boundary condition at the `x = L` end.
    pub fn set_right_boundary(&mut self, bc: TwoFluidBoundary) {
        self.right = bc;
    }

    /// The added-mass coefficient `C_vm` \[-\].
    #[must_use]
    pub fn virtual_mass_coefficient(&self) -> Ratio {
        Ratio::new::<ratio>(self.c_vm)
    }

    /// Set the added-mass coefficient `C_vm`.
    ///
    /// `0` removes virtual mass entirely, leaving drag as the only interfacial
    /// momentum coupling — which is what two of upstream's thirty tutorials do
    /// (study §3.3, §10.3). `0.5` is the default and the potential-flow sphere
    /// value. **Changing this changes the answer**, which is why study §9.5
    /// requires a benchmark to report its sensitivity over `[0, 0.5]`.
    ///
    /// # Errors
    ///
    /// [`TampinesError::InvalidInput`] if negative or not finite.
    pub fn set_virtual_mass_coefficient(&mut self, c_vm: Ratio) -> Result<(), TampinesError> {
        let c = c_vm.get::<ratio>();
        if !c.is_finite() || c < 0.0 {
            return Err(TampinesError::InvalidInput(format!(
                "C_vm must be finite and >= 0 (got {c})"
            )));
        }
        self.c_vm = c;
        Ok(())
    }

    /// The residual volume fraction `α_res` \[-\] — see
    /// [`DEFAULT_RESIDUAL_ALPHA`].
    #[must_use]
    pub fn residual_alpha(&self) -> f64 {
        self.residual_alpha
    }

    /// Set the residual volume fraction `α_res`, in `[0, 1)`.
    ///
    /// A **numerical device**. Raising it makes the vanishing-phase limit
    /// better conditioned and the vanishing-phase momentum less physical, in
    /// the same breath.
    ///
    /// # Errors
    ///
    /// [`TampinesError::InvalidInput`] if outside `[0, 1)` or not finite.
    pub fn set_residual_alpha(&mut self, residual_alpha: f64) -> Result<(), TampinesError> {
        if !residual_alpha.is_finite() || !(0.0..1.0).contains(&residual_alpha) {
            return Err(TampinesError::InvalidInput(format!(
                "residual alpha must be in [0, 1) (got {residual_alpha})"
            )));
        }
        self.residual_alpha = residual_alpha;
        Ok(())
    }

    /// The initial void fraction used where the `(p, T)` flash gives none —
    /// see [`DEFAULT_INITIAL_VOID_FRACTION`].
    #[must_use]
    pub fn initial_void_fraction(&self) -> f64 {
        self.initial_void_fraction
    }

    /// Set the initial void fraction, in `(0, 1)`.
    ///
    /// Takes effect on the **next** [`new`](Self::new)-style initialisation,
    /// i.e. on the next [`set_temperature_profile`](Self::set_temperature_profile)
    /// call — it does not retroactively change a state already built. A model
    /// parameter with no measured provenance; see
    /// [`DEFAULT_INITIAL_VOID_FRACTION`].
    ///
    /// # Errors
    ///
    /// [`TampinesError::InvalidInput`] if outside `(0, 1)` or not finite.
    pub fn set_initial_void_fraction(&mut self, alpha: f64) -> Result<(), TampinesError> {
        if !alpha.is_finite() || !(alpha > 0.0 && alpha < 1.0) {
            return Err(TampinesError::InvalidInput(format!(
                "initial void fraction must be in (0, 1) (got {alpha})"
            )));
        }
        self.initial_void_fraction = alpha;
        Ok(())
    }

    /// Set the number of outer correctors per step.
    ///
    /// Each corrector re-solves the pressure equation against the volume
    /// residual the previous one left, so this is how many Newton steps the
    /// volume constraint gets per timestep. One is enough only where the
    /// constraint is already nearly satisfied.
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
    pub fn set_pressure_under_relaxation(&mut self, alpha: Ratio) -> Result<(), TampinesError> {
        let a = alpha.get::<ratio>();
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

    /// The pipe geometry.
    #[must_use]
    pub fn pipe(&self) -> &Pipe1d {
        &self.pipe
    }

    /// The interfacial closure set.
    #[must_use]
    pub fn interfacial_exchange(&self) -> &InterfacialExchange {
        &self.exchange
    }

    /// Cell pressures \[Pa\], read-only.
    #[must_use]
    pub fn pressure(&self) -> &[f64] {
        &self.p
    }

    /// Cell vapour volume fractions `α_g` \[-\], read-only.
    #[must_use]
    pub fn void_fraction(&self) -> &[f64] {
        &self.alpha_g
    }

    /// Cell liquid volume fractions `α_l` \[-\], read-only.
    ///
    /// **Not** `1 − α_g` by construction — see the type-level docs. Their sum
    /// differs from 1 by [`TwoFluidReport::max_volume_residual`].
    #[must_use]
    pub fn liquid_fraction(&self) -> &[f64] {
        &self.alpha_l
    }

    /// Cell vapour densities \[kg/m³\], read-only.
    #[must_use]
    pub fn vapour_density(&self) -> &[f64] {
        &self.rho_g
    }

    /// Cell liquid densities \[kg/m³\], read-only.
    #[must_use]
    pub fn liquid_density(&self) -> &[f64] {
        &self.rho_l
    }

    /// Cell vapour specific enthalpies \[J/kg\], read-only.
    #[must_use]
    pub fn vapour_enthalpy(&self) -> &[f64] {
        &self.h_g
    }

    /// Cell liquid specific enthalpies \[J/kg\], read-only.
    #[must_use]
    pub fn liquid_enthalpy(&self) -> &[f64] {
        &self.h_l
    }

    /// Cell vapour temperatures \[K\], read-only. Above `T_sat(p)` where the
    /// vapour is superheated, below it on the bounded metastable branch.
    #[must_use]
    pub fn vapour_temperature(&self) -> &[f64] {
        &self.t_g
    }

    /// Cell liquid temperatures \[K\], read-only. Above `T_sat(p)` where the
    /// liquid is metastably superheated — which is the state that drives
    /// flashing, and the thing a four-equation model cannot represent.
    #[must_use]
    pub fn liquid_temperature(&self) -> &[f64] {
        &self.t_l
    }

    /// Cell saturation temperatures `T_sat(p)` \[K\], read-only — the interface
    /// temperature both phases exchange heat with.
    #[must_use]
    pub fn saturation_temperature(&self) -> Vec<f64> {
        self.sat.iter().map(|s| s.t_sat).collect()
    }

    /// Vapour face velocities \[m/s\], read-only, length `n_cells + 1`.
    #[must_use]
    pub fn vapour_face_velocity(&self) -> &[f64] {
        &self.u_g
    }

    /// Liquid face velocities \[m/s\], read-only, length `n_cells + 1`.
    #[must_use]
    pub fn liquid_face_velocity(&self) -> &[f64] {
        &self.u_l
    }

    /// Face slip velocities `u_g − u_l` \[m/s\], length `n_cells + 1`.
    #[must_use]
    pub fn slip_velocity(&self) -> Vec<f64> {
        (0..=self.pipe.n_cells())
            .map(|j| self.u_g[j] - self.u_l[j])
            .collect()
    }

    /// Cell mixture densities `m_g + m_l` \[kg/m³\].
    ///
    /// This is the *conserved* mixture density — the sum of the two transported
    /// phase mass concentrations — not `α_g ρ_g + α_l ρ_l` recomputed from the
    /// derived fractions. The two agree to the volume residual.
    #[must_use]
    pub fn mixture_density(&self) -> Vec<f64> {
        (0..self.pipe.n_cells())
            .map(|i| self.m_g[i] + self.m_l[i])
            .collect()
    }

    /// Total mass currently in the pipe.
    #[must_use]
    pub fn inventory(&self) -> Mass {
        let v = self.pipe.cell_volume();
        Mass::new::<kilogram>(
            (0..self.pipe.n_cells())
                .map(|i| (self.m_g[i] + self.m_l[i]) * v)
                .sum::<f64>(),
        )
    }

    /// Total enthalpy currently in the pipe \[J\], `Σ_i (m_g h_g + m_l h_l)_i V`.
    ///
    /// Exposed because it is the quantity the conservation verification checks:
    /// in a closed adiabatic pipe at rest it is conserved exactly, since the
    /// interfacial energy balance `Q_g + Q_l + Γ h_fg = 0` cancels identically.
    #[must_use]
    pub fn total_enthalpy(&self) -> f64 {
        let v = self.pipe.cell_volume();
        (0..self.pipe.n_cells())
            .map(|i| (self.m_g[i] * self.h_g[i] + self.m_l[i] * self.h_l[i]) * v)
            .sum()
    }

    /// Mixture mass flow through the right-hand boundary \[kg/s\], from the
    /// current state.
    #[must_use]
    pub fn outlet_mass_flow(&self) -> MassRate {
        let n = self.pipe.n_cells();
        let a = self.pipe.area_si();
        MassRate::new::<kilogram_per_second>(
            (self.m_g[n - 1] * self.u_g[n] + self.m_l[n - 1] * self.u_l[n]) * a,
        )
    }

    /// Vapour density of cell `i` as a `uom` quantity.
    ///
    /// # Panics
    ///
    /// If `i >= n_cells`.
    #[must_use]
    pub fn cell_vapour_density(&self, i: usize) -> MassDensity {
        MassDensity::new::<kilogram_per_cubic_meter>(self.rho_g[i])
    }

    /// Liquid specific enthalpy of cell `i` as a `uom` quantity.
    ///
    /// # Panics
    ///
    /// If `i >= n_cells`.
    #[must_use]
    pub fn cell_liquid_enthalpy(&self, i: usize) -> AvailableEnergy {
        AvailableEnergy::new::<joule_per_kilogram>(self.h_l[i])
    }

    /// Vapour temperature of cell `i` as a `uom` quantity.
    ///
    /// # Panics
    ///
    /// If `i >= n_cells`.
    #[must_use]
    pub fn cell_vapour_temperature(&self, i: usize) -> ThermodynamicTemperature {
        ThermodynamicTemperature::new::<kelvin>(self.t_g[i])
    }

    /// Liquid temperature of cell `i` as a `uom` quantity.
    ///
    /// # Panics
    ///
    /// If `i >= n_cells`.
    #[must_use]
    pub fn cell_liquid_temperature(&self, i: usize) -> ThermodynamicTemperature {
        ThermodynamicTemperature::new::<kelvin>(self.t_l[i])
    }

    // ─────────────────────────────────────────────────────────────────────
    //  The march
    // ─────────────────────────────────────────────────────────────────────

    /// Advance one timestep.
    ///
    /// The five-stage semi-implicit march described in the module docs:
    /// assemble and invert the 2×2 drag + virtual-mass block at every face,
    /// solve the tridiagonal pressure equation built from the volume
    /// constraint, correct the face velocities, transport the four conserved
    /// quantities with donor-cell fluxes, and exchange heat and mass at the
    /// interface implicitly — the last four inside an outer-corrector loop.
    ///
    /// # Errors
    ///
    /// - [`TampinesError::Unphysical`] if any cell leaves IF97's validity
    ///   range, a phase enthalpy leaves the bounded metastable bracket
    ///   ([`super::properties::MAX_METASTABLE_LIQUID_SUPERHEAT`]), or a volume
    ///   fraction leaves `[0, 1]`. **Refused, not clamped.**
    /// - [`TampinesError::Numerical`] if the pressure matrix is singular, a
    ///   pressure iterate goes non-finite, or the volume residual exceeds
    ///   [`MAX_VOLUME_RESIDUAL`] after the last corrector.
    /// - [`TampinesError::Closure`] if a traced-back drag or heat-transfer
    ///   closure rejects its inputs.
    /// - [`TampinesError::InvalidInput`] for a malformed boundary condition.
    pub fn step(&mut self) -> Result<TwoFluidReport, TampinesError> {
        let n = self.pipe.n_cells();
        let dx = self.pipe.dx();
        let area = self.pipe.area_si();
        let volume = self.pipe.cell_volume();
        let g_x = self.pipe.axial_gravity();
        let dt = self.dt;
        let d_h = self.pipe.hydraulic_diameter_si();

        // ── Old-time snapshot ────────────────────────────────────────────
        let p_old = self.p.clone();
        let m_g_old = self.m_g.clone();
        let m_l_old = self.m_l.clone();
        let h_g_old = self.h_g.clone();
        let h_l_old = self.h_l.clone();
        let alpha_g_old = self.alpha_g.clone();
        let alpha_l_old = self.alpha_l.clone();
        let rho_g_old = self.rho_g.clone();
        let rho_l_old = self.rho_l.clone();
        let sat_old = self.sat.clone();
        let transport_old = self.transport.clone();
        let u_g_old = self.u_g.clone();
        let u_l_old = self.u_l.clone();

        // ── Stage 0: closures and compressibilities on the old state ─────
        let ug_cell: Vec<f64> = (0..n)
            .map(|i| 0.5 * (u_g_old[i] + u_g_old[i + 1]))
            .collect();
        let ul_cell: Vec<f64> = (0..n)
            .map(|i| 0.5 * (u_l_old[i] + u_l_old[i + 1]))
            .collect();

        let mut k_d_cell = vec![0.0; n];
        let mut gamma = vec![0.0; n];
        for i in 0..n {
            let exchange = self.implicit_exchange(
                p_old[i],
                m_g_old[i],
                m_l_old[i],
                h_g_old[i],
                h_l_old[i],
                ug_cell[i],
                ul_cell[i],
                sat_old[i],
                transport_old[i],
            )?;
            k_d_cell[i] = exchange.k_d;
            gamma[i] = exchange.gamma;
        }

        let (psi_g, psi_l) = self.phase_compressibilities(&p_old, &h_g_old, &h_l_old)?;

        // ── Stage 1: face momentum — the 2x2 implicit coupling blocks ────
        let (left_u, left_choked) = self.resolve_boundary(self.left, 0)?;
        let (right_u, right_choked) = self.resolve_boundary(self.right, n - 1)?;

        let mut u_star_g = vec![0.0; n + 1];
        let mut u_star_l = vec![0.0; n + 1];
        let mut d_g = vec![0.0; n + 1];
        let mut d_l = vec![0.0; n + 1];

        for j in 1..n {
            let (cl, cr) = (j - 1, j);
            let alpha_g_f = 0.5 * (alpha_g_old[cl] + alpha_g_old[cr]);
            let alpha_l_f = 0.5 * (alpha_l_old[cl] + alpha_l_old[cr]);
            // The block requires a partition of unity; the transported
            // fractions carry the volume residual, so they are normalised for
            // the closure and left raw for the field equations.
            let sum = alpha_g_f + alpha_l_f;
            let (ag_f, al_f) = if sum > 0.0 {
                (alpha_g_f / sum, alpha_l_f / sum)
            } else {
                (0.5, 0.5)
            };
            let rho_g_f = 0.5 * (rho_g_old[cl] + rho_g_old[cr]);
            let rho_l_f = 0.5 * (rho_l_old[cl] + rho_l_old[cr]);
            let k_d_f = 0.5 * (k_d_cell[cl] + k_d_cell[cr]);
            let gamma_f = 0.5 * (gamma[cl] + gamma[cr]);
            let mu_g_f = 0.5 * (sat_old[cl].mu_g + sat_old[cr].mu_g);
            let mu_l_f = 0.5 * (sat_old[cl].mu_f + sat_old[cr].mu_f);

            let block = PhaseCouplingBlock::assemble(PhaseCouplingInputs {
                alpha_g: ag_f,
                alpha_l: al_f,
                rho_g: rho_g_f,
                rho_l: rho_l_f,
                k_d: k_d_f,
                c_vm: self.c_vm,
                dispersed: self.exchange.dispersed_phase(),
                residual_alpha: self.residual_alpha,
                dt,
            })?;

            // Explicit donor-cell convection u du/dx, per phase.
            let conv_g = donor_convection(&u_g_old, j, dx);
            let conv_l = donor_convection(&u_l_old, j, dx);

            // Material-derivative explicit remainders H^D = u^n/dt - (u du/dx).
            let hd_g = u_g_old[j] / dt - conv_g;
            let hd_l = u_l_old[j] / dt - conv_l;

            // Interfacial momentum carried by the transferred mass, in the
            // non-conservative form: Gamma_k (u^i - u_k), with u^i the DONOR
            // phase's velocity.
            let (gm_g, gm_l) = if gamma_f >= 0.0 {
                (gamma_f * (u_l_old[j] - u_g_old[j]), 0.0)
            } else {
                (0.0, -gamma_f * (u_g_old[j] - u_l_old[j]))
            };

            let mg_f = ag_f * rho_g_f;
            let ml_f = al_f * rho_l_f;
            let b_g = block.vapour_diagonal * u_g_old[j] - mg_f * conv_g + mg_f * g_x
                - mg_f * wall_friction_acceleration(u_g_old[j], rho_g_f, mu_g_f, d_h)
                + gm_g
                + block.tapered_virtual_mass_vapour * (hd_g - hd_l);
            let b_l = block.liquid_diagonal * u_l_old[j] - ml_f * conv_l + ml_f * g_x
                - ml_f * wall_friction_acceleration(u_l_old[j], rho_l_f, mu_l_f, d_h)
                + gm_l
                + block.tapered_virtual_mass_liquid * (hd_l - hd_g);

            let (ug, ul) = block.solve(b_g, b_l)?;
            let (dg, dl) = block.pressure_coefficients(ag_f, al_f)?;
            u_star_g[j] = ug;
            u_star_l[j] = ul;
            // The pressure correction acts over one cell spacing.
            d_g[j] = dg / dx;
            d_l[j] = dl / dx;
        }

        // Both boundaries are velocity boundaries, so they carry no pressure
        // sensitivity and stay out of the pressure matrix (d = 0).
        u_star_g[0] = left_u;
        u_star_l[0] = left_u;
        u_star_g[n] = right_u;
        u_star_l[n] = right_u;

        // Donor phase mass concentrations at faces, upwinded once on the
        // force-only velocities so the pressure matrix stays linear.
        let mg_face: Vec<f64> = (0..=n)
            .map(|j| donor_cell(&m_g_old, j, n, u_star_g[j]))
            .collect();
        let ml_face: Vec<f64> = (0..=n)
            .map(|j| donor_cell(&m_l_old, j, n, u_star_l[j]))
            .collect();

        // ── Stages 2-5: the outer-corrector loop ─────────────────────────
        let compliance: Vec<f64> = (0..n)
            .map(|i| {
                alpha_g_old[i] * psi_g[i] / rho_g_old[i] + alpha_l_old[i] * psi_l[i] / rho_l_old[i]
            })
            .collect();

        let mut p_new = p_old.clone();
        let mut u_g_new = u_star_g.clone();
        let mut u_l_new = u_star_l.clone();
        let mut r_vol = vec![0.0; n];

        let mut m_g_new = m_g_old.clone();
        let mut m_l_new = m_l_old.clone();
        let mut h_g_new = h_g_old.clone();
        let mut h_l_new = h_l_old.clone();
        let mut alpha_g_new = alpha_g_old.clone();
        let mut alpha_l_new = alpha_l_old.clone();
        let mut rho_g_new = rho_g_old.clone();
        let mut rho_l_new = rho_l_old.clone();
        let mut t_g_new = self.t_g.clone();
        let mut t_l_new = self.t_l.clone();
        let mut sat_new = sat_old.clone();
        let mut transport_new = transport_old.clone();

        let mut correctors_used = 0usize;
        let mut limited_cells = 0usize;
        let mut floor_events = 0usize;

        // The Newton system is assembled once: its matrix is the (quasi-)
        // Jacobian `−(V/Δt) ∂R/∂p` of the volume residual with respect to the
        // cell pressures, evaluated at the old-time state. Only the right-hand
        // side changes between correctors, so the assembly is hoisted.
        //
        // `∂R_i/∂p_i` has exactly two parts: the phase densities respond
        // through the compliance `C_i = Σ_k α_k ψ_k / ρ_k` (diagonal only), and
        // the transported phase masses respond through the pressure-corrected
        // face fluxes (tridiagonal). Multiplying by `−V/Δt` turns both into the
        // familiar compliance-plus-Laplacian pressure matrix, strictly
        // diagonally dominant because the compliance term sits on the diagonal
        // and nowhere else.
        let mut sub = vec![0.0; n];
        let mut diag = vec![0.0; n];
        let mut sup = vec![0.0; n];
        for i in 0..n {
            let inv_rho_g = 1.0 / rho_g_old[i];
            let inv_rho_l = 1.0 / rho_l_old[i];
            let (jl, jr) = (i, i + 1);
            let cap_l =
                inv_rho_g * mg_face[jl] * area * d_g[jl] + inv_rho_l * ml_face[jl] * area * d_l[jl];
            let cap_r =
                inv_rho_g * mg_face[jr] * area * d_g[jr] + inv_rho_l * ml_face[jr] * area * d_l[jr];
            sub[i] = -cap_l;
            sup[i] = -cap_r;
            diag[i] = volume * compliance[i] / dt + cap_l + cap_r;
        }

        for outer in 0..self.n_outer_correctors {
            correctors_used = outer + 1;
            limited_cells = 0;
            floor_events = 0;

            // -- Newton correction on the volume residual ------------------------
            //
            // The very first pass takes `R = 0`, i.e. no correction, so that it
            // evaluates the residual of the *uncorrected* transport and the
            // correction it drives is a genuine Newton step from `p_old`. Every
            // later pass corrects the residual the previous one left.
            let mut max_change: f64 = 0.0;
            if outer > 0 {
                let rhs: Vec<f64> = (0..n).map(|i| volume * r_vol[i] / dt).collect();
                let delta_p = thomas_solve(&sub, &diag, &sup, &rhs)?;
                for i in 0..n {
                    let target = p_new[i] + self.p_under_relaxation * delta_p[i];
                    if !target.is_finite() {
                        return Err(TampinesError::Numerical(format!(
                            "cell {i}: pressure iterate is not finite at outer corrector {outer}"
                        )));
                    }
                    max_change = max_change.max((target - p_new[i]).abs());
                    p_new[i] = target.clamp(P_MIN_IF97 * 1.000_001, P_MAX_IF97 * 0.999_999);
                }
            } else {
                max_change = f64::INFINITY;
            }

            // -- velocity correction ----------------------------------------------
            for j in 1..n {
                u_g_new[j] = u_star_g[j] - d_g[j] * (p_new[j] - p_new[j - 1]);
                u_l_new[j] = u_star_l[j] - d_l[j] * (p_new[j] - p_new[j - 1]);
            }
            u_g_new[0] = left_u;
            u_l_new[0] = left_u;
            u_g_new[n] = right_u;
            u_l_new[n] = right_u;

            // -- transport, interfacial exchange, recovery -------------------------
            let flux_g: Vec<f64> = (0..=n).map(|j| mg_face[j] * u_g_new[j] * area).collect();
            let flux_l: Vec<f64> = (0..=n).map(|j| ml_face[j] * u_l_new[j] * area).collect();

            let mut max_residual: f64 = 0.0;
            for i in 0..n {
                let (jl, jr) = (i, i + 1);
                let hg_l = donor_scalar(&h_g_old, jl, n, flux_g[jl]);
                let hg_r = donor_scalar(&h_g_old, jr, n, flux_g[jr]);
                let hl_l = donor_scalar(&h_l_old, jl, n, flux_l[jl]);
                let hl_r = donor_scalar(&h_l_old, jr, n, flux_l[jr]);

                let m_g_star = m_g_old[i] - dt / volume * (flux_g[jr] - flux_g[jl]);
                let m_l_star = m_l_old[i] - dt / volume * (flux_l[jr] - flux_l[jl]);
                let dp = p_new[i] - p_old[i];
                let e_g_star = m_g_old[i] * h_g_old[i]
                    - dt / volume * (flux_g[jr] * hg_r - flux_g[jl] * hg_l)
                    + alpha_g_old[i] * dp;
                let e_l_star = m_l_old[i] * h_l_old[i]
                    - dt / volume * (flux_l[jr] * hl_r - flux_l[jl] * hl_l)
                    + alpha_l_old[i] * dp;

                let sat_i = SaturatedProperties::at(p_new[i])?;
                let transport_i = if transport_old[i].is_stale_for(p_new[i]) {
                    transport_at(p_new[i])?
                } else {
                    transport_old[i]
                };

                // Vanishing-phase floor on the transported masses.
                let floor_g = self.residual_alpha * sat_i.rho_g;
                let floor_l = self.residual_alpha * sat_i.rho_f;
                let (m_g_t, h_g_t) = if m_g_star > PHASE_FLOOR_TRIGGER * floor_g {
                    (m_g_star, e_g_star / m_g_star)
                } else {
                    floor_events += 1;
                    (floor_g, sat_i.h_g)
                };
                let (m_l_t, h_l_t) = if m_l_star > PHASE_FLOOR_TRIGGER * floor_l {
                    (m_l_star, e_l_star / m_l_star)
                } else {
                    floor_events += 1;
                    (floor_l, sat_i.h_f)
                };

                let ug_i = 0.5 * (u_g_new[jl] + u_g_new[jr]);
                let ul_i = 0.5 * (u_l_new[jl] + u_l_new[jr]);
                let exchange = self.implicit_exchange(
                    p_new[i],
                    m_g_t,
                    m_l_t,
                    h_g_t,
                    h_l_t,
                    ug_i,
                    ul_i,
                    sat_i,
                    transport_i,
                )?;
                if exchange.limited {
                    limited_cells += 1;
                }
                gamma[i] = exchange.gamma;
                k_d_cell[i] = exchange.k_d;

                let m_g_i = m_g_t + dt * exchange.gamma;
                let m_l_i = m_l_t - dt * exchange.gamma;
                let e_g_i = m_g_t * h_g_t + dt * (exchange.q_g + exchange.gamma * sat_i.h_g);
                let e_l_i = m_l_t * h_l_t + dt * (exchange.q_l - exchange.gamma * sat_i.h_f);

                let (m_g_f, h_g_f) = if m_g_i > PHASE_FLOOR_TRIGGER * floor_g {
                    (m_g_i, e_g_i / m_g_i)
                } else {
                    floor_events += 1;
                    (floor_g, sat_i.h_g)
                };
                let (m_l_f, h_l_f) = if m_l_i > PHASE_FLOOR_TRIGGER * floor_l {
                    (m_l_i, e_l_i / m_l_i)
                } else {
                    floor_events += 1;
                    (floor_l, sat_i.h_f)
                };

                let vapour = PhaseState::vapour_at(p_new[i], h_g_f, sat_i)?;
                let liquid = PhaseState::liquid_at(p_new[i], h_l_f, sat_i)?;
                let a_g = m_g_f / vapour.density;
                let a_l = m_l_f / liquid.density;

                m_g_new[i] = m_g_f;
                m_l_new[i] = m_l_f;
                h_g_new[i] = h_g_f;
                h_l_new[i] = h_l_f;
                rho_g_new[i] = vapour.density;
                rho_l_new[i] = liquid.density;
                t_g_new[i] = vapour.temperature;
                t_l_new[i] = liquid.temperature;
                alpha_g_new[i] = a_g;
                alpha_l_new[i] = a_l;
                sat_new[i] = sat_i;
                transport_new[i] = transport_i;

                r_vol[i] = a_g + a_l - 1.0;
                max_residual = max_residual.max(r_vol[i].abs());
            }

            if max_change < self.outer_tolerance && max_residual < self.volume_residual_tolerance {
                break;
            }
        }

        // ── Acceptance checks — refusals, not clamps ─────────────────────
        let max_volume_residual = r_vol.iter().fold(0.0_f64, |acc, r| acc.max(r.abs()));
        if !(max_volume_residual <= MAX_VOLUME_RESIDUAL) {
            return Err(TampinesError::Numerical(format!(
                "volume constraint alpha_g + alpha_l = 1 is violated by {max_volume_residual} \
                 after {correctors_used} outer correctors at t = {} s, above the \
                 {MAX_VOLUME_RESIDUAL} limit: the pressure-velocity coupling did not converge. \
                 Refused rather than renormalised, because a renormalised density field looks \
                 like an answer",
                self.time + dt
            )));
        }
        for i in 0..n {
            for (name, a) in [("alpha_g", alpha_g_new[i]), ("alpha_l", alpha_l_new[i])] {
                if !a.is_finite() || !(-1.0e-9..=1.0 + 1.0e-9).contains(&a) {
                    return Err(TampinesError::Unphysical(format!(
                        "cell {i}: {name} = {a} left [0, 1] at t = {} s -- refused rather than \
                         clamped, because a clamped void fraction produces a plausible answer \
                         that is wrong",
                        self.time + dt
                    )));
                }
            }
        }

        // ── Commit ───────────────────────────────────────────────────────
        self.p = p_new;
        self.m_g = m_g_new;
        self.m_l = m_l_new;
        self.h_g = h_g_new;
        self.h_l = h_l_new;
        self.alpha_g = alpha_g_new;
        self.alpha_l = alpha_l_new;
        self.rho_g = rho_g_new;
        self.rho_l = rho_l_new;
        self.t_g = t_g_new;
        self.t_l = t_l_new;
        self.sat = sat_new;
        self.transport = transport_new;
        self.u_g = u_g_new;
        self.u_l = u_l_new;
        self.time += dt;

        let max_courant = self
            .u_g
            .iter()
            .chain(self.u_l.iter())
            .map(|u| u.abs() * dt / dx)
            .fold(0.0_f64, f64::max);
        let max_void = self.alpha_g.iter().copied().fold(0.0_f64, f64::max);
        let min_void = self.alpha_g.iter().copied().fold(1.0_f64, f64::min);
        let max_thermal = (0..n)
            .map(|i| (self.t_g[i] - self.t_l[i]).abs())
            .fold(0.0_f64, f64::max);
        let max_slip = (0..=n)
            .map(|j| (self.u_g[j] - self.u_l[j]).abs())
            .fold(0.0_f64, f64::max);

        Ok(TwoFluidReport {
            time: self.time,
            outlet_mass_flow: (self.m_g[n - 1] * self.u_g[n] + self.m_l[n - 1] * self.u_l[n])
                * area,
            max_void_fraction: max_void,
            max_thermal_nonequilibrium: max_thermal,
            min_void_fraction: min_void,
            max_slip,
            inventory: (0..n).map(|i| (self.m_g[i] + self.m_l[i]) * volume).sum(),
            max_courant,
            max_volume_residual,
            outer_correctors_used: correctors_used,
            outlet_choked: left_choked || right_choked,
            mass_transfer_limited_cells: limited_cells,
            residual_alpha_floor_events: floor_events,
        })
    }

    // ─────────────────────────────────────────────────────────────────────
    //  Internals
    // ─────────────────────────────────────────────────────────────────────

    /// Evaluate the traced-back interfacial closures for one cell and turn the
    /// explicit heat fluxes into the **implicit** ones a stiff relaxation
    /// needs, then rate-limit the mass transfer against the cell's phase
    /// inventory.
    ///
    /// # Why the heat transfer is made implicit here rather than left explicit
    ///
    /// [`super::interfacial`] returns `Q_k = K_k (T_sat − T_k)` evaluated
    /// explicitly. With small inclusions `K_k` is enormous — every closure
    /// scales as `1/d²` — so an explicit `Q_k Δt` routinely overshoots
    /// `T_sat` and the relaxation oscillates and diverges. Solving the same
    /// linear relaxation implicitly in `h_k`,
    ///
    /// `m_k (h_k^{n+1} − h_k^*) / Δt = K_k (T_sat − T_k^{n+1})`,
    /// `T_k^{n+1} ≈ T_k^* + (h_k^{n+1} − h_k^*) / c_{p,k}`,
    ///
    /// gives the closed form
    ///
    /// `Q_k^{imp} = (T_sat − T_k^*) / (1/K_k + Δt / (m_k c_{p,k}))`
    ///
    /// which is unconditionally stable and, in the `K_k → ∞` limit, delivers
    /// exactly the heat needed to bring the phase to `T_sat` within the step
    /// and no more. That is a *solution* of a stiff ODE, not a clamp.
    ///
    /// `K_k` is recovered as `Q_k / (T_sat − T_k)`; where the driving
    /// difference vanishes so does `Q_k`, and the implicit flux is zero too.
    ///
    /// # The mass-transfer rate limiter, and why it is not a clamp either
    ///
    /// `Γ = −(Q_g + Q_l)/h_fg` is bounded by each phase's *sensible* heat, but
    /// condensation driven by a large liquid subcooling can still remove more
    /// vapour than a cell contains. Where that would push a phase mass below
    /// `α_res ρ_k` within the step, `Γ` **and both heat fluxes** are scaled by
    /// the same factor `s ∈ [0, 1]`. Scaling all three together keeps the
    /// interfacial energy balance `Q_g + Q_l + Γ h_fg = 0` exact, so energy is
    /// still conserved to machine precision when the limiter fires; the
    /// activation is counted into
    /// [`TwoFluidReport::mass_transfer_limited_cells`] so it cannot act
    /// invisibly.
    ///
    /// # Arguments
    ///
    /// Raw `f64`, strict SI: `p` \[Pa\], `m_g`/`m_l` \[kg/m³\], `h_g`/`h_l`
    /// \[J/kg\], `u_g`/`u_l` \[m/s\] at the cell centre.
    ///
    /// # Errors
    ///
    /// As [`super::interfacial::InterfacialExchange::sources_with_properties`].
    #[allow(clippy::too_many_arguments)]
    fn implicit_exchange(
        &self,
        p: f64,
        m_g: f64,
        m_l: f64,
        h_g: f64,
        h_l: f64,
        u_g: f64,
        u_l: f64,
        sat: SaturatedProperties,
        transport: SaturatedTransport,
    ) -> Result<ImplicitExchange, TampinesError> {
        // The closures require a partition of unity; the transported fractions
        // carry the volume residual, so normalise for the closure only.
        let a_g_raw = m_g / sat.rho_g.max(f64::MIN_POSITIVE);
        let a_l_raw = m_l / sat.rho_f.max(f64::MIN_POSITIVE);
        let vapour = PhaseState::vapour_at(p, h_g, sat)?;
        let liquid = PhaseState::liquid_at(p, h_l, sat)?;
        let a_g = m_g / vapour.density;
        let a_l = m_l / liquid.density;
        let sum = if a_g + a_l > 0.0 {
            a_g + a_l
        } else {
            (a_g_raw + a_l_raw).max(f64::MIN_POSITIVE)
        };
        let void = (a_g / sum).clamp(0.0, 1.0);

        let cell = InterfacialCellState {
            pressure: p,
            void_fraction: void,
            vapour_enthalpy: h_g,
            liquid_enthalpy: h_l,
            vapour_velocity: u_g,
            liquid_velocity: u_l,
        };
        let sources = self
            .exchange
            .sources_with_properties(cell, sat, transport)?;

        let q_g = implicit_heat_flux(
            sources.vapour_heat,
            sat.t_sat - sources.vapour_temperature,
            m_g,
            transport.cp_g,
            self.dt,
        );
        let q_l = implicit_heat_flux(
            sources.liquid_heat,
            sat.t_sat - sources.liquid_temperature,
            m_l,
            transport.cp_f,
            self.dt,
        );
        let gamma_raw = -(q_g + q_l) / sat.h_fg();

        // Rate limiter against the donor phase's inventory: never below the
        // residual-alpha floor, and never more than
        // MAX_MASS_TRANSFER_FRACTION_PER_STEP of what is there.
        let floor_g = self.residual_alpha * sat.rho_g;
        let floor_l = self.residual_alpha * sat.rho_f;
        let mut scale = 1.0_f64;
        let (donor_mass, donor_floor) = if gamma_raw > 0.0 {
            (m_l, floor_l)
        } else {
            (m_g, floor_g)
        };
        let transfer = gamma_raw.abs() * self.dt;
        if transfer > 0.0 {
            let available = (donor_mass - donor_floor)
                .max(0.0)
                .min(MAX_MASS_TRANSFER_FRACTION_PER_STEP * donor_mass);
            if transfer > available {
                scale = (available / transfer).clamp(0.0, 1.0);
            }
        }

        Ok(ImplicitExchange {
            k_d: sources.volumetric_drag_coefficient,
            q_g: scale * q_g,
            q_l: scale * q_l,
            gamma: scale * gamma_raw,
            limited: scale < 1.0,
        })
    }

    /// Per-cell phase compressibilities `ψ_k = dρ_k/dp` \[s²/m²\] **along the
    /// path the step actually takes**, by one-sided finite difference of the
    /// real phase state.
    ///
    /// # Which derivative this is, and why it is not `∂ρ/∂p|_h`
    ///
    /// The obvious choice is the fixed-enthalpy derivative `∂ρ_k/∂p|_{h_k}`,
    /// which is what [`super::drift_flux`] uses. It is the **wrong** one here,
    /// and getting it wrong stalls the outer correctors rather than breaking
    /// anything loudly, which is the worst way for it to be wrong.
    ///
    /// The reason: this solver's energy equations carry the reversible work
    /// term `α_k ∂p/∂t`, so a pressure change of `dp` also changes the phase
    /// enthalpy by `dh_k = α_k dp / m_k = dp / ρ_k` in the same step. The
    /// density therefore moves along
    ///
    /// `dρ_k = ∂ρ_k/∂p|_{h_k} dp + ∂ρ_k/∂h_k|_p (dp / ρ_k)`
    ///
    /// which is the **isentropic** derivative `(∂ρ_k/∂p)_s = 1/c_k²` — the
    /// acoustic compressibility, as it must be, since what the pressure
    /// equation is resolving is a pressure wave. For water at 7 MPa the two
    /// differ by roughly a factor of two (`1/c² ≈ 4.4e-7` against
    /// `∂ρ/∂p|_h ≈ 9.9e-7 s²/m²`), so a Newton iteration built on the
    /// fixed-enthalpy value carries an O(1) Jacobian error.
    ///
    /// This function therefore steps `(p, h_k) → (p + δ, h_k + δ/ρ_k)`, which
    /// follows the same path the marching loop does and needs no extra IF97
    /// evaluations to do it.
    ///
    /// # Why both phases step upward
    ///
    /// The step is taken **away from each phase's metastable bound**, so
    /// evaluating the derivative can never itself trip a refusal the state does
    /// not deserve. Isentropic compression takes saturated liquid into the
    /// subcooled region and saturated steam into the superheated region, so
    /// `+δ` reduces both the liquid superheat and the vapour subcooling.
    ///
    /// One-sided rather than central, because a central difference would double
    /// the IF97 cost for no accuracy that matters at the `1e-4` relative step
    /// ([`COMPRESSIBILITY_STEP`]).
    ///
    /// # Floor
    ///
    /// The results are floored at `1e-12 s²/m²`: density rises with pressure
    /// along an isentrope everywhere in IF97, so a non-positive value can only
    /// come from round-off in the difference, and a non-positive compliance
    /// would make the pressure matrix indefinite.
    ///
    /// # Errors
    ///
    /// [`TampinesError::Unphysical`] if a perturbed state leaves IF97's range
    /// or a metastable bracket.
    fn phase_compressibilities(
        &self,
        p: &[f64],
        h_g: &[f64],
        h_l: &[f64],
    ) -> Result<(Vec<f64>, Vec<f64>), TampinesError> {
        let n = p.len();
        let mut psi_g = vec![0.0; n];
        let mut psi_l = vec![0.0; n];
        for i in 0..n {
            let dp = (COMPRESSIBILITY_STEP * p[i]).max(1.0);
            let sat_here = SaturatedProperties::at(p[i])?;
            let p_hi = (p[i] + dp).min(P_MAX_IF97 * 0.999_999);
            if !(p_hi > p[i]) {
                psi_g[i] = 1.0e-12;
                psi_l[i] = 1.0e-12;
                continue;
            }
            let sat_hi = SaturatedProperties::at(p_hi)?;
            let step = p_hi - p[i];

            let vapour_here = PhaseState::vapour_at(p[i], h_g[i], sat_here)?;
            let vapour_hi =
                PhaseState::vapour_at(p_hi, h_g[i] + step / vapour_here.density, sat_hi)?;
            psi_g[i] = ((vapour_hi.density - vapour_here.density) / step).max(1.0e-12);

            let liquid_here = PhaseState::liquid_at(p[i], h_l[i], sat_here)?;
            let liquid_hi =
                PhaseState::liquid_at(p_hi, h_l[i] + step / liquid_here.density, sat_hi)?;
            psi_l[i] = ((liquid_hi.density - liquid_here.density) / step).max(1.0e-12);
        }
        Ok((psi_g, psi_l))
    }

    /// Resolve a boundary condition against the current state into the face
    /// velocity (imposed on both phases) and whether a critical-flow model set
    /// it.
    ///
    /// `adjacent` is the cell the boundary face touches.
    ///
    /// # Errors
    ///
    /// [`TampinesError::InvalidInput`] for a break-area fraction outside
    /// `(0, 1]`; [`TampinesError::Unphysical`] if the adjacent cell's mixture
    /// state cannot be formed.
    fn resolve_boundary(
        &self,
        bc: TwoFluidBoundary,
        adjacent: usize,
    ) -> Result<(f64, bool), TampinesError> {
        match bc {
            TwoFluidBoundary::Closed => Ok((0.0, false)),
            TwoFluidBoundary::PrescribedVelocity(u) => {
                if !u.is_finite() {
                    return Err(TampinesError::InvalidInput(format!(
                        "prescribed boundary velocity must be finite (got {u})"
                    )));
                }
                Ok((u, false))
            }
            TwoFluidBoundary::ChokedOutlet {
                area_fraction,
                ambient_pressure,
            } => {
                if !(area_fraction > 0.0 && area_fraction <= 1.0) {
                    return Err(TampinesError::InvalidInput(format!(
                        "break area fraction must be in (0, 1] (got {area_fraction})"
                    )));
                }
                let i = adjacent;
                let rho_m = self.m_g[i] + self.m_l[i];
                if !(rho_m > 0.0) {
                    return Err(TampinesError::Unphysical(format!(
                        "cell {i}: mixture density {rho_m} kg/m^3 is not positive at the break"
                    )));
                }
                // The HEM dispatcher wants a MIXTURE stagnation enthalpy; the
                // six-equation state has two, so the mass-weighted mean is
                // handed over. That is the same homogeneity assumption the
                // break model already makes, made explicit.
                let h_m = (self.m_g[i] * self.h_g[i] + self.m_l[i] * self.h_l[i]) / rho_m;
                let (p_throat, g_crit) = critical_flux(self.p[i], h_m);
                let choked = p_throat > ambient_pressure;
                let g = if choked {
                    g_crit
                } else {
                    let dp = (self.p[i] - ambient_pressure).max(0.0);
                    (2.0 * rho_m * dp).sqrt()
                };
                Ok((g * area_fraction / rho_m, choked))
            }
        }
    }
}

/// One cell's interfacial exchange after the implicit relaxation and the rate
/// limiter — the internal result of [`TwoFluid1d::implicit_exchange`].
#[derive(Debug, Clone, Copy, PartialEq)]
struct ImplicitExchange {
    /// Volumetric drag coefficient `K_d` \[kg/(m³·s)\], straight from the
    /// traced-back closure (never limited — it is not a source term).
    k_d: f64,
    /// Implicit interfacial heat into the vapour \[W/m³\].
    q_g: f64,
    /// Implicit interfacial heat into the liquid \[W/m³\].
    q_l: f64,
    /// Interfacial mass transfer \[kg/(m³·s)\], positive for evaporation.
    gamma: f64,
    /// Whether the rate limiter scaled all three back.
    limited: bool,
}

// ─────────────────────────────────────────────────────────────────────────────
//  Free helpers
// ─────────────────────────────────────────────────────────────────────────────

/// The implicit form of a linear interfacial heat flux \[W/m³\].
///
/// Given the *explicit* flux `q_explicit = K (T_sat − T)` \[W/m³\] and its
/// driving difference `driving = T_sat − T` \[K\], returns
///
/// `q = driving / (1/K + Δt / (m c_p))`
///
/// with `m` the phase mass concentration \[kg/m³\] and `c_p` \[J/(kg·K)\]. See
/// [`TwoFluid1d::implicit_exchange`] for the derivation and for why this is a
/// solution rather than a limiter.
///
/// Returns zero when the driving difference or the heat capacity vanishes —
/// both cases where the explicit flux is itself zero or meaningless.
fn implicit_heat_flux(q_explicit: f64, driving: f64, m: f64, cp: f64, dt: f64) -> f64 {
    if driving.abs() < 1.0e-12 || !(m > 0.0) || !(cp > 0.0) {
        return 0.0;
    }
    let k = q_explicit / driving;
    if !(k > 0.0) || !k.is_finite() {
        return 0.0;
    }
    q_explicit / (1.0 + k * dt / (m * cp))
}

/// Donor-cell (first-order upwind) convective acceleration `u ∂u/∂x`
/// \[m/s²\] at interior face `j` of a face-centred velocity array.
///
/// Uses the sign of `u[j]` to pick the upwind neighbour, so it is monotone.
fn donor_convection(u: &[f64], j: usize, dx: f64) -> f64 {
    let u_j = u[j];
    if u_j >= 0.0 {
        u_j * (u_j - u[j - 1]) / dx
    } else {
        u_j * (u[j + 1] - u_j) / dx
    }
}

/// Donor-cell pick of a **cell-centred** field at face `j` of an `n`-cell mesh.
///
/// `velocity` sets the upwind direction. At the two end faces there is only one
/// real neighbour, so the adjacent cell is used regardless of direction — which
/// makes inflow through an end face a zero-gradient re-injection of that cell's
/// own state. That is a documented limitation of the velocity-only boundary
/// set; see [`TwoFluidBoundary::PrescribedVelocity`].
fn donor_cell(field: &[f64], j: usize, n: usize, velocity: f64) -> f64 {
    if j == 0 {
        field[0]
    } else if j == n {
        field[n - 1]
    } else if velocity >= 0.0 {
        field[j - 1]
    } else {
        field[j]
    }
}

/// Donor-cell pick of a cell-centred scalar at face `j`, upwinded on a **mass
/// flux** \[kg/s\] rather than a velocity. Same end-face convention as
/// [`donor_cell`].
fn donor_scalar(field: &[f64], j: usize, n: usize, flux: f64) -> f64 {
    donor_cell(field, j, n, flux)
}

/// Wall-friction deceleration `F_wall/ρ` \[m/s²\], Darcy-Weisbach.
///
/// `F/ρ = f |u| u / (2 D_h)` with
///
/// - laminar, `Re < 2300`: `f = 64/Re`, exact for fully developed pipe flow;
/// - turbulent: `f = 0.316 Re^{−1/4}` (Blasius, smooth pipe, good to
///   `Re ≈ 10⁵`).
///
/// The same closure [`super::drift_flux`] uses, applied here **per phase** with
/// that phase's own density and viscosity, and weighted by its volume fraction
/// at the call site. There is no flow-regime wall-friction partition — which
/// phase actually wets the wall is not modelled — so this is crude wherever
/// friction matters. Carries the sign of `u`, so it always opposes the motion.
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

/// Specific enthalpy \[J/kg\] at `(p, T)`, picking the IF97 region from `T`
/// against `T_sat`.
///
/// A `(p, T)` pair inside the dome is degenerate — it does not fix the state —
/// so exactly at `T_sat` this returns the **saturated-liquid** enthalpy, the
/// convention a subcooled initial condition needs. Mirrors the private helper
/// of the same name in [`super::drift_flux`], deliberately: the two solvers
/// must interpret an initial condition identically or they cannot be compared.
fn enthalpy_at_pt(p: f64, t: f64, sat: SaturatedProperties) -> Result<f64, TampinesError> {
    use tampines_steam_tables::region_1_subcooled_liquid::h_tp_1;
    use tampines_steam_tables::region_2_vapour::h_tp_2;

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

/// The HEM critical throat pressure \[Pa\] and mass flux \[kg/(m²·s)\] at a
/// stagnation `(p, h)`.
///
/// Delegates to the crate's existing dispatcher rather than introducing a
/// second choking model. See [`TwoFluidBoundary::ChokedOutlet`] for the two
/// modelling inconsistencies that implies, and
/// [`super::drift_flux::AxialBoundary::ChokedOutlet`] for the dispatcher's full
/// V&V record — including that Marviken is **not** gated and supports no claim
/// about it.
fn critical_flux(p: f64, h: f64) -> (f64, f64) {
    use tampines_steam_tables::steam_turbine_equations::converging_diverging_nozzles::choked_flow::get_critical_pressure_and_mass_flux_multiphase_ph;
    use uom::si::mass_flux::kilogram_per_square_meter_second;

    let (p_throat, g) = get_critical_pressure_and_mass_flux_multiphase_ph(
        Pressure::new::<pascal>(p),
        AvailableEnergy::new::<joule_per_kilogram>(h),
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

/// The largest relative pressure nudge
/// [`region_4_safe_pressure`] will apply \[-\].
///
/// `4e-12` — eight decades below
/// [`super::properties::TRANSPORT_CACHE_TOLERANCE`] and four below
/// [`super::properties::SATURATION_CACHE_TOLERANCE`], so nothing that reads a
/// property set can tell the difference. At 7 MPa the whole budget is 28 µPa.
pub const MAX_REGION_4_NUDGE: f64 = 4.0e-12;

/// Nudge a pressure off the **exact** IF97 Region-4 saturation line, so that
/// [`SaturatedTransport::at`] does not panic.
///
/// # The defect this works around
///
/// `tampines_steam_tables`'s forward-equation region classifier
/// (`interfaces/functional_programming/pt_flash_eqm/mod.rs:134`) decides
/// "this `(T, p)` is the two-phase Region 4" with an **exact float equality**,
/// `pres == p_sat_reg4_pascal`. Region-4 then `panic!`s in `cp_tp_eqm_single_phase`
/// (`:211`) because `(T, p)` cannot resolve a saturated mixture without a
/// quality.
///
/// [`SaturatedTransport::at`] evaluates the conductivity at exactly
/// `(T_sat(p), p)`, so it lands on that equality — and therefore **panics** —
/// for every pressure that happens to round-trip bit-exactly through
/// `sat_pressure_4(sat_temp_4(p))`. Measured 2026-08-12 over a geometric sweep
/// of 10 790 pressures from 0.1 MPa to 22 MPa (ratio 1.0005): **105 of them
/// panic**, scattered rather than banded, and the bit-exact round-trip
/// predicts all 105 with **zero** false positives and **zero** false negatives.
/// `SaturatedProperties::at` panics at none of them, because it never routes
/// through the classifier.
///
/// A blowdown sweeps continuously through pressure, so hitting one is a
/// certainty rather than a risk; the panic aborts the whole march.
///
/// # What this does about it
///
/// Because the trigger is *exactly* the bit-equality, it can be predicted
/// rather than caught: this function evaluates
/// `sat_pressure_4(sat_temp_4(p))` and, only if it is bit-identical to `p`,
/// multiplies `p` by `1 + 4e-12`, retrying up to four times. Everywhere else it
/// returns `p` unchanged, bit for bit.
///
/// **This is a workaround in the consumer, not a fix.** The defect is in
/// `tampines-steam-tables` (a panic where an error belongs, and an exact float
/// comparison on a boundary) and belongs there; this function exists so the
/// six-equation solver is not blocked on it, and so the next reader finds the
/// diagnosis rather than a mysterious `+ 4e-12`.
///
/// # Units
///
/// `p` \[Pa\]; returns \[Pa\].
#[must_use]
pub fn region_4_safe_pressure(p: f64) -> f64 {
    use tampines_steam_tables::region_4_vap_liq_equilibrium::sat_pressure::sat_pressure_4;
    use tampines_steam_tables::region_4_vap_liq_equilibrium::sat_temp::sat_temp_4;

    let mut candidate = p;
    for _ in 0..4 {
        let round_trip =
            sat_pressure_4(sat_temp_4(Pressure::new::<pascal>(candidate))).get::<pascal>();
        if round_trip != candidate {
            return candidate;
        }
        candidate *= 1.0 + MAX_REGION_4_NUDGE;
    }
    candidate
}

/// [`SaturatedTransport::at`] through [`region_4_safe_pressure`].
///
/// Every call to the conduction-property set in this module goes through here;
/// see [`region_4_safe_pressure`] for the upstream defect that makes it
/// necessary.
///
/// # Errors
///
/// As [`SaturatedTransport::at`].
fn transport_at(p: f64) -> Result<SaturatedTransport, TampinesError> {
    SaturatedTransport::at(region_4_safe_pressure(p))
}
