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
//! `∂(m_k h_k)/∂t + ∂(m_k h_k u_k)/∂x = α_k ∂p/∂t + Q_k^i + Γ_k h_k^i + Q_k^wall`  (phase energy)
//!
//! with `Q_k^wall` the phasic wall heat of gh:#1062 (RELAP-7 Eq. 201,
//! [`super::wall_heat`]), zero for the default adiabatic wall.
//!
//! and the **volume constraint** `α_g + α_l = 1`, which is what the pressure
//! equation enforces (see "The pressure equation" below).
//!
//! The momentum equation is written in the *non-conservative* velocity form —
//! the conservative form minus `u_k ×` the phase mass equation — which is why
//! the mass-transfer momentum source appears as `Γ_k (u^i − u_k)` rather than
//! `Γ_k u^i`.
//!
//! # Interface velocity, friction work and total-enthalpy mass transfer (gh:#1060)
//!
//! RELAP-7 carries the interphase exchange at an **interface velocity**
//! `u_int` (Theory Manual Eq. 200) and lets the transferred mass carry the
//! interface **total** enthalpy `H_int = h_int + ½ u_int²` (Eq. 208). The
//! manual is not public, so only equation numbers are cited; the definition
//! of `u_int` used here is the **mass-weighted** (centre-of-mass) velocity
//!
//! `u_int = (α_l ρ_l u_l + α_g ρ_g u_g) / (α_l ρ_l + α_g ρ_g)`
//!
//! as specified in the gh:#1060 brief — **not** re-checked against Eq. 200,
//! which was not available in this session. This solver's energy equations
//! are in *static-enthalpy* form, so the total-energy sources translate as
//! follows (derived here, not transcribed): subtract each phase's
//! kinetic-energy equation (`u_k ×` its momentum equation) from its
//! total-energy equation.
//!
//! - **Mass-transfer momentum**: `Γ_k (u_int − u_k)` with `u_int` above,
//!   replacing the donor-phase velocity used before.
//! - **Friction work at `u_int`**: the interfacial drag `F_k` does work
//!   `F_k u_int` in phase `k`'s total energy, of which `F_k u_k` is
//!   kinetic, so `F_k (u_int − u_k)` appears in the static-enthalpy equation.
//!   Summed over both phases this is `K_d (u_g − u_l)² ≥ 0` — the drag
//!   dissipation, now split between the phases by `u_int` instead of being
//!   dropped.
//! - **Total-enthalpy mass transfer**: `Γ_k H_int` in total energy becomes
//!   `Γ_k (h_k^sat + ½ (u_int − u_k)²)` in static enthalpy. `Γ` itself is
//!   **unchanged**, because `H_g,int − H_l,int = h_g^sat − h_f^sat` exactly
//!   (both sides carry the same `½ u_int²`); what static `h_sat` got wrong was
//!   only the energy the transferred mass carries.
//!
//! Each is **on by default**, with the ablations
//! [`TwoFluid1d::without_interface_velocity`] (donor velocity, no friction
//! work) and [`TwoFluid1d::without_total_enthalpy_mass_transfer`]. With both
//! ablated and [`TwoFluid1d::without_regime_map`], the march is bit-identical
//! to the pre-gh:#1060 solver (pinned by
//! `all_three_ablations_reproduce_the_pre_1060_march_bit_for_bit`).
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
//! heat transfer, no interfacial-area transport, HEM break model;
//! ~~no flow-regime map~~ **CORRECTED 2026-11-01 (gh:#1060):** the interfacial
//! area now follows a void-fraction regime map, see [`super::interfacial`]).
//! Specific to this one:
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
//! - ~~**Boundary conditions are velocity-type only**~~ **CORRECTED
//!   2026-11-01 (gh:#1076):** pressure outlets, pressure inlets and velocity
//!   inlets with a described inflow state exist, with time-dependent
//!   versions ([`TwoFluidTimeDependentBoundary`]); what follows is the
//!   original note.
//! - **Boundary conditions are velocity-type only**
//!   ([`TwoFluidBoundary`]): closed, prescribed velocity, and an HEM choked
//!   outlet. There is no Dirichlet-pressure boundary, so the pressure floats on
//!   the compliance diagonal — right for a blowdown, wrong for a pipe fed from
//!   a plenum. [`super::drift_flux`]'s `ReservoirInlet` and `PressureOutlet`
//!   have no counterpart here yet.
//! - **The non-condensable species is passive** (gh:#1064, RELAP-7
//!   Eqs 128–132): `c = α_g ρ_g Y` is transported conservatively with the gas
//!   and stays out of `Γ`, but the gas EOS remains IF97 steam (no Dalton
//!   partial pressure), so it does not affect condensation. See
//!   [`TwoFluid1d::set_noncondensable_mass_fraction`] and, for the
//!   interfacial second-law check and the entropy diagnostic,
//!   [`super::entropy`].
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

use petir::mathf::RealMath;
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
use tampines_steam_tables::hem::TimeTable;
use super::wall_heat::{thermal_expansion, PhasicWallHeat, WallPhaseState};
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
/// provenance**. Since gh:#1060 it sizes the bubbly/slug regime only; above
/// `α_g = 0.5` the regime map blends toward the annular core area.
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
/// ~~**Every variant is a *velocity* boundary.**~~ **CORRECTED 2026-11-01
/// (gh:#1076):** [`PressureOutlet`](Self::PressureOutlet) and
/// [`PressureInlet`](Self::PressureInlet) are pressure boundaries, and
/// [`VelocityInlet`](Self::VelocityInlet) is a velocity boundary that says
/// what enters. The paragraph below describes the original velocity-only set.
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
    /// does; ~~there is none yet~~ **CORRECTED 2026-11-01 (gh:#1076):** use
    /// [`VelocityInlet`](Self::VelocityInlet) or
    /// [`PressureInlet`](Self::PressureInlet).
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

    /// A **pressure** boundary \[Pa\] (gh:#1076): the face velocity of each
    /// phase follows from its own momentum equation across the half cell
    /// between the boundary and the first cell centre, with the 2×2
    /// drag + virtual-mass block assembled on the adjacent cell's state.
    /// Backflow re-injects the adjacent cell's state (zero-gradient donor).
    /// The pressure is static: no `½ρu²` is subtracted.
    PressureOutlet {
        /// Static pressure at the face \[Pa\].
        pressure: f64,
    },

    /// A pressure boundary that also says **what enters** (gh:#1076): the
    /// face velocities as for [`PressureOutlet`](Self::PressureOutlet), and
    /// inflow carries the [`TwoFluidInflow`] state evaluated at `pressure`.
    PressureInlet {
        /// Static pressure at the face \[Pa\].
        pressure: f64,
        /// The entering fluid.
        inflow: TwoFluidInflow,
    },

    /// A prescribed face velocity \[m/s\] on both phases, positive in `+x`,
    /// carrying the [`TwoFluidInflow`] state (evaluated at the adjacent
    /// cell's start-of-step pressure) whenever it flows *into* the pipe
    /// (gh:#1076). Outflow through it is zero-gradient, as for
    /// [`PrescribedVelocity`](Self::PrescribedVelocity).
    VelocityInlet {
        /// Face velocity \[m/s\], positive in `+x`.
        velocity: f64,
        /// The entering fluid.
        inflow: TwoFluidInflow,
    },
}

/// How the thermal state of entering liquid is given (gh:#1076).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TwoFluidInflowThermo {
    /// Liquid temperature \[K\], at or below `T_sat` at the face pressure
    /// (a superheated value is refused by the property layer, not clipped).
    LiquidTemperature(f64),
    /// Liquid specific enthalpy \[J/kg\], within the metastable bracket.
    LiquidEnthalpy(f64),
}

/// The state of fluid entering through an inflow boundary (gh:#1076):
/// liquid at the given temperature or enthalpy, vapour **saturated** at the
/// face pressure, in the volume ratio `void_fraction`. With the face pressure
/// that fixes `m_g = α ρ_g^sat`, `m_l = (1 − α) ρ_l(p, h_l)`, `h_g = h_g^sat`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TwoFluidInflow {
    /// Liquid thermal state.
    pub thermo: TwoFluidInflowThermo,
    /// Entering vapour volume fraction \[-\], in `[0, 1)`.
    pub void_fraction: f64,
}

/// A time-dependent two-fluid boundary (gh:#1076): each quantity is a reused
/// [`TimeTable`] (`tampines_steam_tables::hem::TimeTable`, the type RELAP-7-
/// style time-dependent components take in `HemNetwork`). At the start of
/// every step the solver evaluates it at the current time `t^n` into the
/// static [`TwoFluidBoundary`] of the same shape, so a constant table gives
/// the static boundary exactly.
#[derive(Debug, Clone, PartialEq)]
pub enum TwoFluidTimeDependentBoundary {
    /// [`TwoFluidBoundary::PressureOutlet`] with `p(t)` \[Pa\].
    PressureOutlet {
        /// Static pressure \[Pa\] against time \[s\].
        pressure: TimeTable,
    },
    /// [`TwoFluidBoundary::PressureInlet`] with `p(t)`, `T_l(t)` and `α(t)`.
    PressureInlet {
        /// Static pressure \[Pa\] against time.
        pressure: TimeTable,
        /// Liquid temperature \[K\] against time.
        liquid_temperature: TimeTable,
        /// Vapour volume fraction \[-\] against time.
        void_fraction: TimeTable,
    },
    /// [`TwoFluidBoundary::VelocityInlet`] with `u(t)`, `T_l(t)` and `α(t)`.
    VelocityInlet {
        /// Face velocity \[m/s\] against time.
        velocity: TimeTable,
        /// Liquid temperature \[K\] against time.
        liquid_temperature: TimeTable,
        /// Vapour volume fraction \[-\] against time.
        void_fraction: TimeTable,
    },
}

impl TwoFluidTimeDependentBoundary {
    /// The static boundary this one prescribes at time `t`.
    #[must_use]
    pub fn at(&self, t: Time) -> TwoFluidBoundary {
        let t = t.get::<second>();
        match self {
            Self::PressureOutlet { pressure } => TwoFluidBoundary::PressureOutlet {
                pressure: pressure.at(t),
            },
            Self::PressureInlet {
                pressure,
                liquid_temperature,
                void_fraction,
            } => TwoFluidBoundary::PressureInlet {
                pressure: pressure.at(t),
                inflow: TwoFluidInflow {
                    thermo: TwoFluidInflowThermo::LiquidTemperature(liquid_temperature.at(t)),
                    void_fraction: void_fraction.at(t),
                },
            },
            Self::VelocityInlet {
                velocity,
                liquid_temperature,
                void_fraction,
            } => TwoFluidBoundary::VelocityInlet {
                velocity: velocity.at(t),
                inflow: TwoFluidInflow {
                    thermo: TwoFluidInflowThermo::LiquidTemperature(liquid_temperature.at(t)),
                    void_fraction: void_fraction.at(t),
                },
            },
        }
    }
}

/// The phase masses and enthalpies an inflow boundary donates to its face.
#[derive(Debug, Clone, Copy, PartialEq)]
struct InflowDonor {
    m_g: f64,
    m_l: f64,
    h_g: f64,
    h_l: f64,
}

/// A boundary resolved against the start-of-step state.
#[derive(Debug, Clone, Copy, PartialEq)]
enum BoundaryFace {
    /// Velocity face: `u` on both phases.
    Velocity {
        u: f64,
        choked: bool,
        inflow: Option<InflowDonor>,
    },
    /// Pressure face at `p_b`.
    Pressure {
        p_b: f64,
        inflow: Option<InflowDonor>,
    },
}

impl BoundaryFace {
    fn inflow(self) -> Option<InflowDonor> {
        match self {
            Self::Velocity { inflow, .. } | Self::Pressure { inflow, .. } => inflow,
        }
    }
}

/// [`TwoFluidInflow`] at pressure `p` \[Pa\] as face donor values.
fn inflow_donor(p: f64, inflow: TwoFluidInflow) -> Result<InflowDonor, TampinesError> {
    let alpha = inflow.void_fraction;
    if !(0.0..1.0).contains(&alpha) {
        return Err(TampinesError::InvalidInput(format!(
            "inflow void fraction must be in [0, 1) (got {alpha})"
        )));
    }
    let sat = SaturatedProperties::at(p)?;
    let h_l = match inflow.thermo {
        TwoFluidInflowThermo::LiquidTemperature(t) => enthalpy_at_pt(p, t, sat)?,
        TwoFluidInflowThermo::LiquidEnthalpy(h) => h,
    };
    let liquid = PhaseState::liquid_at(p, h_l, sat)?;
    Ok(InflowDonor {
        m_g: alpha * sat.rho_g,
        m_l: (1.0 - alpha) * liquid.density,
        h_g: sat.h_g,
        h_l,
    })
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
    /// Mixture mass flow through the **left** face \[kg/s\], `+x` positive
    /// (into the pipe), from the final corrector's donor fluxes (gh:#1076).
    pub inlet_mass_flow: f64,
    /// Enthalpy flow `Σ_k ṁ_k h_k` through the left face \[W\], `+x`
    /// positive, with the donor enthalpies the transport used (gh:#1076).
    pub inlet_energy_flow: f64,
    /// Enthalpy flow through the **right** face \[W\], `+x` positive
    /// (out of the pipe), likewise.
    pub outlet_energy_flow: f64,
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
    /// gh:#1060: mass-transfer momentum and friction work at `u_int`
    /// (Eq. 200). `false` only via [`TwoFluid1d::without_interface_velocity`].
    interface_velocity: bool,
    /// gh:#1060: transferred mass carries `H_int` (Eq. 208). `false` only via
    /// [`TwoFluid1d::without_total_enthalpy_mass_transfer`].
    total_enthalpy_mass_transfer: bool,
    /// gh:#1062: wall heat, adiabatic by default.
    wall_heat: PhasicWallHeat,
    /// gh:#1076: time-dependent boundaries, re-evaluated into `left`/`right`
    /// at the start of every step.
    left_time_dependent: Option<TwoFluidTimeDependentBoundary>,
    right_time_dependent: Option<TwoFluidTimeDependentBoundary>,
    /// gh:#1064: non-condensable mass concentration `c = α_g ρ_g Y`
    /// \[kg/m³\] (RELAP-7 Eqs 128–132), a passive scalar carried by the gas.
    m_nc: Vec<f64>,
    /// gh:#1064: non-condensable mass fraction `Y` of gas entering through an
    /// inflow boundary \[-\]; `0` unless set.
    inflow_nc_fraction: f64,
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
        // gh:#1060: the regime-map interfacial area is the default for every
        // vapour-dispersed closure set; `without_regime_map` ablates it. A
        // droplet set keeps its fixed diameter (the map refuses it).
        let exchange = if exchange.dispersed_phase() == DispersedPhase::Vapour {
            exchange.with_regime_map(pipe.hydraulic_diameter())?
        } else {
            exchange
        };

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
            interface_velocity: true,
            total_enthalpy_mass_transfer: true,
            wall_heat: PhasicWallHeat::default(),
            left_time_dependent: None,
            right_time_dependent: None,
            m_nc: vec![0.0; n],
            inflow_nc_fraction: 0.0,
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
    /// keep being used at void fractions where bubbly flow no longer exists
    /// (**since gh:#1060** only their *area* follows the regime map there).
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

    /// **Ablation** (gh:#1060): go back to one prescribed bubble diameter at
    /// every void fraction instead of the regime-map interfacial area.
    #[must_use]
    pub fn without_regime_map(mut self) -> Self {
        self.exchange = self.exchange.without_regime_map();
        self
    }

    /// **Ablation** (gh:#1060): carry the mass-transfer momentum at the
    /// donor phase's velocity and drop the interfacial friction work, as
    /// before the change.
    #[must_use]
    pub fn without_interface_velocity(mut self) -> Self {
        self.interface_velocity = false;
        self
    }

    /// **Ablation** (gh:#1060): the transferred mass carries the static
    /// saturation enthalpy only, without the `½ (u_int − u_k)²` that the
    /// interface total enthalpy contributes in static-enthalpy form.
    #[must_use]
    pub fn without_total_enthalpy_mass_transfer(mut self) -> Self {
        self.total_enthalpy_mass_transfer = false;
        self
    }

    /// Set the wall heat boundary condition (gh:#1062, RELAP-7 Eq. 201).
    ///
    /// Each phase receives its share through the linear wall-contact
    /// partition (vapour `α_g`, liquid `1 − α_g`; see [`super::wall_heat`]),
    /// against its own temperature and, for the RELAP5 variant, with its own
    /// properties and cell-centre velocity. Explicit on the start-of-step
    /// state.
    ///
    /// # Errors
    ///
    /// As [`PhasicWallHeat::validate`].
    pub fn set_wall_heat(&mut self, wall_heat: PhasicWallHeat) -> Result<(), TampinesError> {
        wall_heat.validate()?;
        self.wall_heat = wall_heat;
        Ok(())
    }

    /// The wall heat boundary condition.
    #[must_use]
    pub fn wall_heat(&self) -> PhasicWallHeat {
        self.wall_heat
    }

    /// Per-cell volumetric wall heat `(vapour, liquid)` \[W/m³\] on the
    /// start-of-step state (gh:#1062).
    #[allow(clippy::type_complexity)]
    fn wall_heat_sources(
        &self,
        ug_cell: &[f64],
        ul_cell: &[f64],
    ) -> Result<(Vec<f64>, Vec<f64>), TampinesError> {
        let n = self.pipe.n_cells();
        let mut q_g = vec![0.0; n];
        let mut q_l = vec![0.0; n];
        for i in 0..n {
            let sat = self.sat[i];
            let tr = self.transport[i];
            let mut vapour = WallPhaseState {
                temperature: self.t_g[i],
                density: self.rho_g[i],
                viscosity: sat.mu_g,
                conductivity: tr.lambda_g,
                cp: tr.cp_g,
                expansion: 0.0,
                velocity: ug_cell[i],
            };
            let mut liquid = WallPhaseState {
                temperature: self.t_l[i],
                density: self.rho_l[i],
                viscosity: sat.mu_f,
                conductivity: tr.lambda_f,
                cp: tr.cp_f,
                expansion: 0.0,
                velocity: ul_cell[i],
            };
            if self.wall_heat.needs_properties() {
                vapour.expansion = thermal_expansion(false, self.p[i], self.h_g[i], tr.cp_g, sat)?;
                liquid.expansion = thermal_expansion(true, self.p[i], self.h_l[i], tr.cp_f, sat)?;
            }
            let sum = self.alpha_g[i] + self.alpha_l[i];
            let void = if sum > 0.0 { self.alpha_g[i] / sum } else { 0.0 };
            let (g, l) = self.wall_heat.phasic_heat(
                void,
                self.pipe.cell_hydraulic_diameter_si(i),
                vapour,
                liquid,
            )?;
            q_g[i] = g;
            q_l[i] = l;
        }
        Ok((q_g, q_l))
    }

    /// Whether the gh:#1060 interface-velocity terms are active.
    #[must_use]
    pub fn uses_interface_velocity(&self) -> bool {
        self.interface_velocity
    }

    /// Whether the gh:#1060 total-enthalpy mass-transfer term is active.
    #[must_use]
    pub fn uses_total_enthalpy_mass_transfer(&self) -> bool {
        self.total_enthalpy_mass_transfer
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

    /// Set the boundary condition at the `x = 0` end. Clears a
    /// time-dependent one set there.
    pub fn set_left_boundary(&mut self, bc: TwoFluidBoundary) {
        self.left = bc;
        self.left_time_dependent = None;
    }

    /// Set the boundary condition at the `x = L` end. Clears a
    /// time-dependent one set there.
    pub fn set_right_boundary(&mut self, bc: TwoFluidBoundary) {
        self.right = bc;
        self.right_time_dependent = None;
    }

    /// Set a time-dependent boundary at the `x = 0` end (gh:#1076),
    /// evaluated at the start of every step.
    pub fn set_left_time_dependent_boundary(&mut self, bc: TwoFluidTimeDependentBoundary) {
        self.left = bc.at(self.time());
        self.left_time_dependent = Some(bc);
    }

    /// Set a time-dependent boundary at the `x = L` end (gh:#1076).
    pub fn set_right_time_dependent_boundary(&mut self, bc: TwoFluidTimeDependentBoundary) {
        self.right = bc.at(self.time());
        self.right_time_dependent = Some(bc);
    }

    /// The boundary currently in force at the `x = 0` end.
    #[must_use]
    pub fn left_boundary(&self) -> TwoFluidBoundary {
        self.left
    }

    /// The boundary currently in force at the `x = L` end.
    #[must_use]
    pub fn right_boundary(&self) -> TwoFluidBoundary {
        self.right
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
        Mass::new::<kilogram>(
            (0..self.pipe.n_cells())
                .map(|i| (self.m_g[i] + self.m_l[i]) * self.pipe.cell_volume_at(i))
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
        (0..self.pipe.n_cells())
            .map(|i| {
                (self.m_g[i] * self.h_g[i] + self.m_l[i] * self.h_l[i]) * self.pipe.cell_volume_at(i)
            })
            .sum()
    }

    /// Set the non-condensable mass fraction `Y` of the gas phase, cell by
    /// cell (gh:#1064). Stored as the conserved concentration
    /// `c = α_g ρ_g Y` \[kg/m³\] on the current gas mass.
    ///
    /// # The model, and its limit
    ///
    /// RELAP-7 §3.1.5 (Eqs 128–132) carries a phasic non-condensable mass
    /// balance, `∂(α_g ρ_g Y A)/∂t + ∂(α_g ρ_g u_g Y A)/∂x = 0`: the species
    /// moves with the gas and does not take part in `Γ`. That balance is what
    /// is solved here, conservatively, with the same donor-cell gas mass
    /// fluxes the vapour mass equation uses.
    ///
    /// **It is a PASSIVE scalar.** The gas equation of state stays IAPWS-IF97
    /// steam: the Dalton mixture (steam at its partial pressure plus an ideal
    /// non-condensable) is **not** applied, so `Y` does not change the gas
    /// density, the saturation temperature at the interface, or the
    /// interfacial heat transfer. Wiring the partial pressure into the pressure
    /// equation, the IF97 phase recovery and the closures touches every stage
    /// of [`step`](Self::step); it is left for when the F14 work (gh:#1045)
    /// needs it. The result is valid only for trace amounts (`Y ≪ 1`). Where
    /// the steam condenses away, `Y = c / (α_g ρ_g)` rises, as it should;
    /// where the residual-alpha floor invents vapour, `c` is untouched.
    ///
    /// # Errors
    ///
    /// [`TampinesError::InvalidInput`] for a slice of the wrong length or an
    /// entry outside `[0, 1]`.
    pub fn set_noncondensable_mass_fraction(&mut self, y: &[f64]) -> Result<(), TampinesError> {
        if y.len() != self.pipe.n_cells() {
            return Err(TampinesError::InvalidInput(format!(
                "non-condensable profile has {} entries, pipe has {} cells",
                y.len(),
                self.pipe.n_cells()
            )));
        }
        if let Some(bad) = y.iter().find(|v| !(0.0..=1.0).contains(*v)) {
            return Err(TampinesError::InvalidInput(format!(
                "non-condensable mass fraction must be in [0, 1] (got {bad})"
            )));
        }
        for (i, yi) in y.iter().enumerate() {
            self.m_nc[i] = yi * self.m_g[i];
        }
        Ok(())
    }

    /// Set the non-condensable mass fraction `Y` of the gas that enters
    /// through an inflow boundary (gh:#1064). `0` by default.
    ///
    /// # Errors
    ///
    /// [`TampinesError::InvalidInput`] outside `[0, 1]`.
    pub fn set_inflow_noncondensable_mass_fraction(&mut self, y: Ratio) -> Result<(), TampinesError> {
        let y = y.get::<ratio>();
        if !(0.0..=1.0).contains(&y) {
            return Err(TampinesError::InvalidInput(format!(
                "inflow non-condensable mass fraction must be in [0, 1] (got {y})"
            )));
        }
        self.inflow_nc_fraction = y;
        Ok(())
    }

    /// Non-condensable mass concentration `c = α_g ρ_g Y` \[kg/m³\] per cell.
    #[must_use]
    pub fn noncondensable_concentration(&self) -> &[f64] {
        &self.m_nc
    }

    /// Non-condensable mass fraction of the gas, `Y = c / (α_g ρ_g)` \[-\],
    /// per cell.
    #[must_use]
    pub fn noncondensable_mass_fraction(&self) -> Vec<f64> {
        self.m_nc
            .iter()
            .zip(&self.m_g)
            .map(|(c, m)| if *m > 0.0 { c / m } else { 0.0 })
            .collect()
    }

    /// Total non-condensable mass in the pipe, `Σ_i c_i V_i`.
    #[must_use]
    pub fn noncondensable_inventory(&self) -> Mass {
        Mass::new::<kilogram>(
            (0..self.pipe.n_cells())
                .map(|i| self.m_nc[i] * self.pipe.cell_volume_at(i))
                .sum::<f64>(),
        )
    }

    /// The area-averaged entropy inventory \[J/K\],
    /// `S = Σ_i V_i (α_g ρ_g s_g + α_l ρ_l s_l)_i` (gh:#1064; the
    /// area-averaged phasic entropy of RELAP-7 §2.1.1, Eqs 18 and 23).
    ///
    /// A diagnostic: each phase's specific entropy is IF97's at the phase's
    /// own `(p, T_k)` — Region 1 for the liquid (its metastable continuation
    /// where superheated), Region 2 for the vapour, the same equations
    /// [`PhaseState`] inverts. In a closed adiabatic pipe the second law
    /// says `S` may not fall; [`super::entropy`] checks the interfacial
    /// closures term by term.
    ///
    /// # Errors
    ///
    /// None in practice; kept fallible for a future property backend.
    pub fn entropy_inventory(&self) -> Result<f64, TampinesError> {
        Ok((0..self.pipe.n_cells())
            .map(|i| {
                let (s_g, s_l) =
                    super::entropy::phase_entropies(self.p[i], self.t_g[i], self.t_l[i]);
                (self.m_g[i] * s_g + self.m_l[i] * s_l) * self.pipe.cell_volume_at(i)
            })
            .sum())
    }

    /// Mixture mass flow through the right-hand boundary \[kg/s\], from the
    /// current state.
    #[must_use]
    pub fn outlet_mass_flow(&self) -> MassRate {
        let n = self.pipe.n_cells();
        let a = self.pipe.face_area_si(n);
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
        // gh:#1076: a time-dependent boundary is evaluated at t^n.
        if let Some(bc) = &self.left_time_dependent {
            self.left = bc.at(self.time());
        }
        if let Some(bc) = &self.right_time_dependent {
            self.right = bc.at(self.time());
        }
        let n = self.pipe.n_cells();
        let dx = self.pipe.dx();
        // gh:#1058: face areas and cell volumes (constant without a profile).
        // The momentum equation is in non-conservative velocity form, where
        // the quasi-1-D wall pressure force `p ∂A/∂x` cancels exactly against
        // `A ∂p/∂x − ∂(pA)/∂x`, so the area enters only the mass and energy
        // fluxes, the cell volumes and the hydraulic diameter.
        let area: Vec<f64> = (0..=n).map(|j| self.pipe.face_area_si(j)).collect();
        let volume: Vec<f64> = (0..n).map(|i| self.pipe.cell_volume_at(i)).collect();
        let g_x = self.pipe.axial_gravity();
        let dt = self.dt;

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
        let m_nc_old = self.m_nc.clone();

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

        // Wall heat (gh:#1062), explicit on the start-of-step state.
        let (q_wall_g, q_wall_l) = if self.wall_heat.is_adiabatic() {
            (vec![0.0; n], vec![0.0; n])
        } else {
            self.wall_heat_sources(&ug_cell, &ul_cell)?
        };

        // ── Stage 1: face momentum — the 2x2 implicit coupling blocks ────
        let left_face = self.resolve_face(self.left, 0)?;
        let right_face = self.resolve_face(self.right, n - 1)?;
        let left_choked = matches!(left_face, BoundaryFace::Velocity { choked: true, .. });
        let right_choked = matches!(right_face, BoundaryFace::Velocity { choked: true, .. });

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

            let mg_f = ag_f * rho_g_f;
            let ml_f = al_f * rho_l_f;
            let d_h = self.pipe.face_hydraulic_diameter_si(j);

            // Interfacial momentum carried by the transferred mass, in the
            // non-conservative form: Gamma_k (u^i - u_k). gh:#1060 (Eq. 200):
            // u^i is the mass-weighted interface velocity; the ablation keeps
            // the DONOR phase's velocity.
            let (gm_g, gm_l) = if self.interface_velocity {
                let u_int = interface_velocity(mg_f, ml_f, u_g_old[j], u_l_old[j]);
                (
                    gamma_f * (u_int - u_g_old[j]),
                    -gamma_f * (u_int - u_l_old[j]),
                )
            } else if gamma_f >= 0.0 {
                (gamma_f * (u_l_old[j] - u_g_old[j]), 0.0)
            } else {
                (0.0, -gamma_f * (u_g_old[j] - u_l_old[j]))
            };

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

        // A velocity boundary carries no pressure sensitivity and stays out of
        // the pressure matrix (d = 0). A pressure boundary (gh:#1076) gets its
        // own momentum predictor over the half cell to the first centre, so
        // its d enters the diagonal of the Newton matrix below exactly like an
        // interior face's.
        for (face, j, cell) in [(left_face, 0, 0), (right_face, n, n - 1)] {
            match face {
                BoundaryFace::Velocity { u, .. } => {
                    u_star_g[j] = u;
                    u_star_l[j] = u;
                }
                BoundaryFace::Pressure { .. } => {
                    let (ug, ul, dg, dl) = self.boundary_face_predictor(
                        j,
                        cell,
                        k_d_cell[cell],
                        &u_g_old,
                        &u_l_old,
                    )?;
                    u_star_g[j] = ug;
                    u_star_l[j] = ul;
                    d_g[j] = dg / (0.5 * dx);
                    d_l[j] = dl / (0.5 * dx);
                }
            }
        }

        // Donor phase mass concentrations at faces, upwinded once on the
        // force-only velocities so the pressure matrix stays linear. An inflow
        // boundary donates its own state where fluid enters (gh:#1076).
        let left_inflow = left_face.inflow();
        let right_inflow = right_face.inflow();
        let face_donor = |field: &[f64], j: usize, u: f64, pick: fn(InflowDonor) -> f64| -> f64 {
            match (j, left_inflow, right_inflow) {
                (0, Some(d), _) if u > 0.0 => pick(d),
                (jj, _, Some(d)) if jj == n && u < 0.0 => pick(d),
                _ => donor_cell(field, j, n, u),
            }
        };
        let mg_face: Vec<f64> = (0..=n)
            .map(|j| face_donor(&m_g_old, j, u_star_g[j], |d| d.m_g))
            .collect();
        let ml_face: Vec<f64> = (0..=n)
            .map(|j| face_donor(&m_l_old, j, u_star_l[j], |d| d.m_l))
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
        let mut inlet_mass_flow = 0.0_f64;
        let mut inlet_energy_flow = 0.0_f64;
        let mut outlet_energy_flow = 0.0_f64;
        let mut limited_cells = 0usize;
        let mut floor_events = 0usize;
        let mut final_flux_g = vec![0.0; n + 1];

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
                inv_rho_g * mg_face[jl] * area[jl] * d_g[jl]
                + inv_rho_l * ml_face[jl] * area[jl] * d_l[jl];
            let cap_r =
                inv_rho_g * mg_face[jr] * area[jr] * d_g[jr]
                + inv_rho_l * ml_face[jr] * area[jr] * d_l[jr];
            sub[i] = -cap_l;
            sup[i] = -cap_r;
            diag[i] = volume[i] * compliance[i] / dt + cap_l + cap_r;
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
                let rhs: Vec<f64> = (0..n).map(|i| volume[i] * r_vol[i] / dt).collect();
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
            match left_face {
                BoundaryFace::Velocity { u, .. } => {
                    u_g_new[0] = u;
                    u_l_new[0] = u;
                }
                BoundaryFace::Pressure { p_b, .. } => {
                    u_g_new[0] = u_star_g[0] - d_g[0] * (p_new[0] - p_b);
                    u_l_new[0] = u_star_l[0] - d_l[0] * (p_new[0] - p_b);
                }
            }
            match right_face {
                BoundaryFace::Velocity { u, .. } => {
                    u_g_new[n] = u;
                    u_l_new[n] = u;
                }
                BoundaryFace::Pressure { p_b, .. } => {
                    u_g_new[n] = u_star_g[n] - d_g[n] * (p_b - p_new[n - 1]);
                    u_l_new[n] = u_star_l[n] - d_l[n] * (p_b - p_new[n - 1]);
                }
            }

            // -- transport, interfacial exchange, recovery -------------------------
            let flux_g: Vec<f64> = (0..=n).map(|j| mg_face[j] * u_g_new[j] * area[j]).collect();
            let flux_l: Vec<f64> = (0..=n).map(|j| ml_face[j] * u_l_new[j] * area[j]).collect();
            final_flux_g.clone_from(&flux_g);

            let mut max_residual: f64 = 0.0;
            for i in 0..n {
                let (jl, jr) = (i, i + 1);
                let hg_l = face_donor(&h_g_old, jl, flux_g[jl], |d| d.h_g);
                let hg_r = face_donor(&h_g_old, jr, flux_g[jr], |d| d.h_g);
                let hl_l = face_donor(&h_l_old, jl, flux_l[jl], |d| d.h_l);
                let hl_r = face_donor(&h_l_old, jr, flux_l[jr], |d| d.h_l);
                if i == 0 {
                    inlet_mass_flow = flux_g[0] + flux_l[0];
                    inlet_energy_flow = flux_g[0] * hg_l + flux_l[0] * hl_l;
                }
                if i == n - 1 {
                    outlet_energy_flow = flux_g[n] * hg_r + flux_l[n] * hl_r;
                }

                let m_g_star = m_g_old[i] - dt / volume[i] * (flux_g[jr] - flux_g[jl]);
                let m_l_star = m_l_old[i] - dt / volume[i] * (flux_l[jr] - flux_l[jl]);
                let dp = p_new[i] - p_old[i];
                let e_g_star = m_g_old[i] * h_g_old[i]
                    - dt / volume[i] * (flux_g[jr] * hg_r - flux_g[jl] * hg_l)
                    + alpha_g_old[i] * dp;
                let e_l_star = m_l_old[i] * h_l_old[i]
                    - dt / volume[i] * (flux_l[jr] * hl_r - flux_l[jl] * hl_l)
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
                let (extra_g, extra_l) = interface_work(
                    self.interface_velocity,
                    self.total_enthalpy_mass_transfer,
                    exchange.gamma,
                    exchange.k_d,
                    m_g_t,
                    m_l_t,
                    ug_i,
                    ul_i,
                );
                let e_g_i = m_g_t * h_g_t
                    + dt * (exchange.q_g + exchange.gamma * sat_i.h_g + extra_g + q_wall_g[i]);
                let e_l_i = m_l_t * h_l_t
                    + dt * (exchange.q_l - exchange.gamma * sat_i.h_f + extra_l + q_wall_l[i]);

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

        // ── Non-condensable species (gh:#1064) ───────────────────────────
        let m_nc_new = noncondensable_transport(
            &m_nc_old,
            &m_g_old,
            &final_flux_g,
            &volume,
            dt,
            NoncondensableInflow {
                left: left_inflow.map(|_| self.inflow_nc_fraction),
                right: right_inflow.map(|_| self.inflow_nc_fraction),
            },
        );

        // ── Commit ───────────────────────────────────────────────────────
        self.m_nc = m_nc_new;
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
                * area[n],
            max_void_fraction: max_void,
            max_thermal_nonequilibrium: max_thermal,
            min_void_fraction: min_void,
            max_slip,
            inventory: (0..n).map(|i| (self.m_g[i] + self.m_l[i]) * volume[i]).sum(),
            max_courant,
            max_volume_residual,
            outer_correctors_used: correctors_used,
            outlet_choked: left_choked || right_choked,
            mass_transfer_limited_cells: limited_cells,
            residual_alpha_floor_events: floor_events,
            inlet_mass_flow,
            inlet_energy_flow,
            outlet_energy_flow,
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

    /// Resolve a boundary against the start-of-step state (gh:#1076): a
    /// velocity face through [`resolve_boundary`](Self::resolve_boundary), a
    /// pressure face as its pressure, each with its inflow donor if it has
    /// one.
    ///
    /// # Errors
    ///
    /// As [`resolve_boundary`](Self::resolve_boundary); a non-finite or
    /// out-of-range boundary pressure is [`TampinesError::InvalidInput`]; an
    /// inflow state the property layer refuses is passed on.
    fn resolve_face(
        &self,
        bc: TwoFluidBoundary,
        adjacent: usize,
    ) -> Result<BoundaryFace, TampinesError> {
        let check_p = |p: f64| -> Result<f64, TampinesError> {
            if p.is_finite() && p > P_MIN_IF97 && p < P_MAX_IF97 {
                Ok(p)
            } else {
                Err(TampinesError::InvalidInput(format!(
                    "boundary pressure must be finite and inside IF97 (got {p} Pa)"
                )))
            }
        };
        match bc {
            TwoFluidBoundary::PressureOutlet { pressure } => Ok(BoundaryFace::Pressure {
                p_b: check_p(pressure)?,
                inflow: None,
            }),
            TwoFluidBoundary::PressureInlet { pressure, inflow } => {
                let p_b = check_p(pressure)?;
                Ok(BoundaryFace::Pressure {
                    p_b,
                    inflow: Some(inflow_donor(p_b, inflow)?),
                })
            }
            TwoFluidBoundary::VelocityInlet { velocity, inflow } => {
                if !velocity.is_finite() {
                    return Err(TampinesError::InvalidInput(format!(
                        "inlet velocity must be finite (got {velocity})"
                    )));
                }
                Ok(BoundaryFace::Velocity {
                    u: velocity,
                    choked: false,
                    inflow: Some(inflow_donor(self.p[adjacent], inflow)?),
                })
            }
            other => {
                let (u, choked) = self.resolve_boundary(other, adjacent)?;
                Ok(BoundaryFace::Velocity {
                    u,
                    choked,
                    inflow: None,
                })
            }
        }
    }

    /// Force-only face velocities `(u*_g, u*_l)` \[m/s\] and the per-phase
    /// pressure sensitivities `(d_g, d_l)` per unit gradient at a
    /// **pressure** boundary face `j` (gh:#1076), from the 2×2 block
    /// assembled on the adjacent `cell`'s start-of-step state. The same terms
    /// as an interior face except the convective acceleration, which has no
    /// upwind neighbour outside the pipe and is dropped.
    fn boundary_face_predictor(
        &self,
        j: usize,
        cell: usize,
        k_d: f64,
        u_g_old: &[f64],
        u_l_old: &[f64],
    ) -> Result<(f64, f64, f64, f64), TampinesError> {
        let dt = self.dt;
        let sum = self.alpha_g[cell] + self.alpha_l[cell];
        let (ag, al) = if sum > 0.0 {
            (self.alpha_g[cell] / sum, self.alpha_l[cell] / sum)
        } else {
            (0.5, 0.5)
        };
        let (rho_g, rho_l) = (self.rho_g[cell], self.rho_l[cell]);
        let block = PhaseCouplingBlock::assemble(PhaseCouplingInputs {
            alpha_g: ag,
            alpha_l: al,
            rho_g,
            rho_l,
            k_d,
            c_vm: self.c_vm,
            dispersed: self.exchange.dispersed_phase(),
            residual_alpha: self.residual_alpha,
            dt,
        })?;
        let (mg, ml) = (ag * rho_g, al * rho_l);
        let d_h = self.pipe.face_hydraulic_diameter_si(j);
        let g_x = self.pipe.axial_gravity();
        let (hd_g, hd_l) = (u_g_old[j] / dt, u_l_old[j] / dt);
        let b_g = block.vapour_diagonal * u_g_old[j] + mg * g_x
            - mg * wall_friction_acceleration(u_g_old[j], rho_g, self.sat[cell].mu_g, d_h)
            + block.tapered_virtual_mass_vapour * (hd_g - hd_l);
        let b_l = block.liquid_diagonal * u_l_old[j] + ml * g_x
            - ml * wall_friction_acceleration(u_l_old[j], rho_l, self.sat[cell].mu_f, d_h)
            + block.tapered_virtual_mass_liquid * (hd_l - hd_g);
        let (ug, ul) = block.solve(b_g, b_l)?;
        let (dg, dl) = block.pressure_coefficients(ag, al)?;
        Ok((ug, ul, dg, dl))
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
            TwoFluidBoundary::PressureOutlet { .. }
            | TwoFluidBoundary::PressureInlet { .. }
            | TwoFluidBoundary::VelocityInlet { .. } => Err(TampinesError::InvalidInput(
                "resolve_boundary handles velocity boundaries only; pressure and inlet \
                 boundaries go through resolve_face"
                    .to_string(),
            )),
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

#[allow(clippy::too_many_arguments)]
/// The gh:#1060 static-enthalpy energy sources \[W/m³\] into `(vapour,
/// liquid)`: interfacial friction work at `u_int` and the kinetic part of
/// the interface total enthalpy carried by the transferred mass. See the
/// module docs ("Interface velocity, friction work and total-enthalpy mass
/// transfer") for the derivation.
///
/// - friction: `F_g (u_int − u_g)` and `F_l (u_int − u_l)` with
///   `F_g = −F_l = K_d (u_l − u_g)`; their sum is `K_d (u_g − u_l)² ≥ 0`.
/// - mass transfer: `Γ_g ½ (u_int − u_g)²` and `Γ_l ½ (u_int − u_l)²`, with
///   `Γ_g = −Γ_l = Γ`.
///
/// `u_int` is the mass-weighted velocity when `interface_velocity_on`
/// ([`TwoFluid1d::without_interface_velocity`] clears it) is set (then
/// both terms apply), otherwise the donor phase's velocity (no friction
/// work; the kinetic term then uses the donor velocity). Returns `(0, 0)`
/// with both ablations — the pre-gh:#1060 energy equation exactly.
///
/// Raw SI: `gamma` \[kg/(m³·s)\], `k_d` \[kg/(m³·s)\], `m_*` \[kg/m³\],
/// `u_*` \[m/s\] at the cell centre.
pub(super) fn interface_work(
    interface_velocity_on: bool,
    total_enthalpy_on: bool,
    gamma: f64,
    k_d: f64,
    m_g: f64,
    m_l: f64,
    u_g: f64,
    u_l: f64,
) -> (f64, f64) {
    let mut extra_g = 0.0;
    let mut extra_l = 0.0;
    let u_int = if interface_velocity_on {
        let u_int = interface_velocity(m_g, m_l, u_g, u_l);
        let f_g = k_d * (u_l - u_g);
        extra_g += f_g * (u_int - u_g);
        extra_l += -f_g * (u_int - u_l);
        u_int
    } else if gamma >= 0.0 {
        u_l
    } else {
        u_g
    };
    if total_enthalpy_on {
        extra_g += gamma * 0.5 * (u_int - u_g) * (u_int - u_g);
        extra_l += -gamma * 0.5 * (u_int - u_l) * (u_int - u_l);
    }
    (extra_g, extra_l)
}

/// The gas phases' non-condensable donor values at the two ends, for
/// faces where fluid enters (gh:#1064). `None` means the face carries the
/// adjacent cell's own `Y` (no inflow boundary there).
#[derive(Debug, Clone, Copy, PartialEq)]
struct NoncondensableInflow {
    left: Option<f64>,
    right: Option<f64>,
}

/// One conservative donor-cell step of the passive non-condensable balance
/// `∂(c A)/∂t + ∂(c u_g A)/∂x = 0`, `c = α_g ρ_g Y` (RELAP-7 Eqs 128–132,
/// gh:#1064).
///
/// The face flux is the gas mass flux the vapour equation used,
/// `F_j = ṁ_g,j Y_donor`, with `Y` upwinded on the sign of `ṁ_g,j` from the
/// start-of-step cells (or the inflow value), so the cell update telescopes
/// and a closed pipe conserves `Σ c V` to round-off. A uniform `Y` stays
/// uniform up to the gas mass change itself.
///
/// Raw SI: `m_nc`, `m_g` \[kg/m³\], `flux_g` \[kg/s\], `volume` \[m³\], `dt` \[s\].
fn noncondensable_transport(
    m_nc: &[f64],
    m_g: &[f64],
    flux_g: &[f64],
    volume: &[f64],
    dt: f64,
    inflow: NoncondensableInflow,
) -> Vec<f64> {
    let n = m_nc.len();
    let y = |i: usize| if m_g[i] > 0.0 { m_nc[i] / m_g[i] } else { 0.0 };
    let face_flux: Vec<f64> = (0..=n)
        .map(|j| {
            let f = flux_g[j];
            let y_donor = if j == 0 {
                match inflow.left {
                    Some(y_in) if f > 0.0 => y_in,
                    _ => y(0),
                }
            } else if j == n {
                match inflow.right {
                    Some(y_in) if f < 0.0 => y_in,
                    _ => y(n - 1),
                }
            } else if f >= 0.0 {
                y(j - 1)
            } else {
                y(j)
            };
            f * y_donor
        })
        .collect();
    (0..n)
        .map(|i| m_nc[i] - dt / volume[i] * (face_flux[i + 1] - face_flux[i]))
        .collect()
}

/// The mass-weighted interface velocity \[m/s\] of gh:#1060,
/// `u_int = (m_g u_g + m_l u_l) / (m_g + m_l)`, with `m_k = α_k ρ_k`
/// \[kg/m³\]. Falls back to the arithmetic mean if both masses vanish (a
/// state the residual-alpha floors exclude).
fn interface_velocity(m_g: f64, m_l: f64, u_g: f64, u_l: f64) -> f64 {
    let m = m_g + m_l;
    if m > 0.0 {
        (m_g * u_g + m_l * u_l) / m
    } else {
        0.5 * (u_g + u_l)
    }
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

/// Wall-friction deceleration `F_wall/ρ` \[m/s²\], Darcy-Weisbach, `F/ρ = f
/// |u| u / (2 D_h)`.
///
/// ~~Laminar `f = 64/Re` below `Re = 2300`, Blasius `0.316 Re^{−1/4}` above
/// (smooth pipe).~~ **CORRECTED 2026-11-01 (gh:#1061):** `f` is now the shared
/// RELAP5 four-region factor ([`crate::closures::wall_friction`]): `f_max`
/// below `Re = 64`, `64/Re` to 2200, reciprocal interpolation to 3000, then
/// Zigrang–Sylvester. The pipe is smooth (`ε = 0`, [`super::geometry::Pipe1d`]
/// carries no roughness) and circular (`Φ_S = 1`). The change moved the
/// Edwards and Marviken results; the movement is recorded in
/// `crates/tampines-steam-tables/verification_and_validation/hem_pipe_friction_gravity.md`.
/// The same closure [`super::drift_flux`] uses, applied here **per phase** with
/// that phase's own density and viscosity, and weighted by its volume fraction
/// at the call site. There is no flow-regime wall-friction partition — which
/// phase actually wets the wall is not modelled — so this is crude wherever
/// friction matters. Carries the sign of `u`, so it always opposes the motion.
fn wall_friction_acceleration(u: f64, rho: f64, mu: f64, d_h: f64) -> f64 {
    crate::closures::wall_friction::wall_friction_acceleration(u, rho, mu, d_h, 0.0, 1.0)
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
/// ~~**This is a workaround in the consumer, not a fix.** The defect is in
/// `tampines-steam-tables` (a panic where an error belongs, and an exact float
/// comparison on a boundary) and belongs there; this function exists so the
/// six-equation solver is not blocked on it, and so the next reader finds the
/// diagnosis rather than a mysterious `+ 4e-12`.~~ **CORRECTED 2026-11-03
/// (gh:#1157): fixed upstream below 623.15 K** (`p < 16.53 MPa`): the
/// conductivity's critical-enhancement term takes the phase from `x` on the
/// saturation line there, so 104 of the 105 panicking pressures of the sweep
/// above no longer panic (the same fix corrected `λ_f`/`λ_g`, which had used
/// the other phase's `c_p` at about half of all pressures). Above 16.53 MPa
/// the line borders Region 3 and one sweep pressure still panics, so this
/// guard stays; measured in
/// `two_fluid_tests::region_4_safe_pressure_removes_the_upstream_classifier_panic`.
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
