// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! The 1-D **seven-equation two-pressure** two-phase model (gh:#1059; RELAP-7
//! Theory Manual §3.1.3–3.1.11, Eqs 115–190, and §3.2.1, Eqs 191–198 with
//! Remarks 1–2). AI draft, clean room: the manual is private, so only section
//! and equation numbers are cited; nothing is transcribed from RELAP-7 source.
//! The numerical scheme follows the published Saurel & Abgrall (1999),
//! "A multiphase Godunov method for compressible multifluid and multiphase
//! flows", *J. Comput. Phys.* 150, 425–467.
//!
//! > Research, education and V&V only. AI draft, unreviewed.
//!
//! # Where it sits
//!
//! One rung above [`super::two_fluid::TwoFluid1d`] (six equations, one
//! pressure, virtual-mass regularised and **not** hyperbolic): each phase has
//! its own pressure, and the volume fraction has its own evolution equation,
//! so the system is hyperbolic for every state, with real characteristic
//! speeds `u_k ± c_k`, `u_k` (each phase) and `u_int`
//! ([`characteristic_speeds`]). Searched first (2026-11-01): no crate in the
//! workspace implements a seven-equation, Baer–Nunziato, Saurel–Abgrall or
//! Kapila model (`grep -ri "baer\|nunziato\|kapila\|seven.equation"`); HRM
//! exists only in 3-D CFD.
//!
//! # The equations (quasi-1-D, area `A(x)`, `k ∈ {v, l}`, `α_l = 1 − α_v`)
//!
//! - volume fraction: `∂α_v/∂t + u_int ∂α_v/∂x = μ (p_v − p_l)`
//! - mass: `∂(α_k ρ_k A)/∂t + ∂(α_k ρ_k u_k A)/∂x = 0` (± `Γ`, see below)
//! - momentum: `∂(α_k ρ_k u_k A)/∂t + ∂(α_k A (ρ_k u_k² + p_k))/∂x =
//!   p_int A ∂α_k/∂x + p_k α_k ∂A/∂x + α_k ρ_k g_x A ± λ (u_j − u_k) A`
//! - total energy: `∂(α_k ρ_k E_k A)/∂t + ∂(α_k u_k A (ρ_k E_k + p_k))/∂x =
//!   p_int u_int A ∂α_k/∂x − p_int μ (p_k − p_j) A + λ u_int (u_j − u_k) A
//!   + α_k ρ_k u_k g_x A`
//! - species (gh:#1064, Eqs 128–132): `∂(α_v ρ_v Y A)/∂t + ∂(α_v ρ_v u_v Y A)/∂x = 0`
//!
//! Each phase is a stiffened gas (`p = (γ − 1) ρ (e − q) − γ p∞`), the reused
//! [`StiffenedGasHem`] pair of gh:#1057.
//!
//! # Interfacial pressure and velocity ([`InterfacialVariables`], Eqs 191–196)
//!
//! - [`ImpedanceWeighted`](InterfacialVariables::ImpedanceWeighted)
//!   (**default**), the discrete-equation-method form with `Z_k = ρ_k c_k`:
//!   `u_int = (Z_l u_l + Z_v u_v)/(Z_l + Z_v) + s (p_v − p_l)/(Z_l + Z_v)`,
//!   `p_int = (Z_l p_v + Z_v p_l)/(Z_l + Z_v) + s Z_l Z_v (u_v − u_l)/(Z_l + Z_v)`,
//!   `s = sgn(∂α_l/∂x)`. These are the acoustic (linearised) solution of the
//!   Riemann problem between the two phases; the `s` terms are
//!   [`SevenEquation1d::without_interface_sign_terms`]'s ablation.
//! - [`BaerNunziato`](InterfacialVariables::BaerNunziato): `u_int` the
//!   velocity of the denser (liquid) phase, `p_int` the pressure of the other.
//! - [`SaurelAbgrall`](InterfacialVariables::SaurelAbgrall) (their 1999
//!   choice): `p_int = Σ α_k p_k`, `u_int` the mass-weighted velocity.
//!
//! # Relaxation (Eqs 197–198)
//!
//! `μ = A_int/(Z_l + Z_v)`, `λ = ½ μ Z_l Z_v`, with `A_int` from the gh:#1060
//! regime map ([`InterfacialExchange::interfacial_area_concentration`]).
//! Each is [`Relaxation::Finite`] by default (on), can be made
//! [`Relaxation::Instantaneous`] (the stiff `μ, λ → ∞` operator), or ablated
//! ([`SevenEquation1d::without_pressure_relaxation`],
//! [`SevenEquation1d::without_velocity_relaxation`]).
//!
//! - **Velocity**, solved exactly: the slip `w = u_v − u_l` decays as
//!   `exp(−λ (1/m_v + 1/m_l) t)` at fixed mixture momentum; the work
//!   `∫ u_int d(m_v u_v)` is integrated in closed form with `u_int` written
//!   as `u_eq + β' w` (its sign term dropped), so total energy is conserved
//!   exactly and each phase's internal energy rises (shown in the code).
//! - **Pressure**, finite: one backward-Euler step for `α_v` with
//!   `m_k de_k = −p_int dα_k`, where `p_int = w p_v' + (1 − w) p_l'` is the
//!   interfacial weight of the chosen [`InterfacialVariables`] (frozen at the
//!   start, sign terms dropped) applied to the **end-of-step** pressures; the
//!   stiffened-gas law makes the end pressures a 2×2 linear system, and the
//!   scalar root in `α_v` is bracketed between the start and the
//!   instantaneous equilibrium and solved with PETIR's Brent. Taking `p_int`
//!   between the end pressures is what makes each phase's entropy change
//!   non-negative (`T_k ds_k = (p_k − p_int) dv_k` along a monotone path);
//!   the first version froze `p_int` at the start and lost entropy at the
//!   `1e-8` level (measured, V&V record). Instantaneous: the stiffened-gas
//!   closed form with `p_int = p*` (the relaxed pressure), a quadratic in
//!   `p*` polished by two Newton steps (Saurel, Petitpas & Berry 2009,
//!   *J. Comput. Phys.* 228, 1678–1712, §3.2). Both conserve total energy
//!   exactly.
//!
//! **Remark 1 limit.** With both instantaneous the model tends to Kapila's
//! five-equation model (one pressure, one velocity, two temperatures), whose
//! sound speed is Wood's. Adding [`MassTransfer::Instantaneous`] (opt-in, see
//! below) relaxes temperatures and Gibbs energies too, which is HEM.
//!
//! **Remark 2.** Relaxing the pressures alone, with `λ = 0`, leaves the
//! single-pressure two-velocity system, whose characteristic speeds are
//! complex at any slip: [`single_pressure_characteristic_speeds`].
//!
//! # Mass transfer ([`MassTransfer`])
//!
//! Only the instantaneous thermo-chemical relaxation is implemented: after
//! the mechanical relaxation, a two-phase or metastable cell is taken to
//! `T_v = T_l = T_sat(p)` at fixed mixture density and internal energy
//! (Saurel, Petitpas & Abgrall 2008, *J. Fluid Mech.* 607, 313–350). It is
//! **off by default** and that is deliberate, not a missing default: it is an
//! equilibrium limit (HEM), not the finite-rate interfacial heat and mass
//! transfer RELAP-7 carries, which is not implemented here.
//!
//! # Numerical method
//!
//! Explicit finite volume, Godunov splitting: the hyperbolic step, then the
//! relaxation operators cell by cell (instantaneous ones after each SSP-RK2
//! stage, since they are idempotent; finite-rate ones once per step).
//!
//! - **Flux** ([`FluxScheme`]): HLLC per phase (default; Toro 2009 §10.4) on
//!   that phase's Euler system, multiplied by one face volume fraction
//!   upwinded on the face `u_int` (the same `α` enters the `α` equation and
//!   the non-conservative terms); or, as the ablation
//!   [`SevenEquation1d::without_hllc`], Saurel & Abgrall's (1999) Rusanov
//!   form with ONE wave-speed bound `S = max_k max(|u_k| + c_k)` for every
//!   equation including `α`, and centred `ᾱ = ½(α_L + α_R)`. Rusanov diffuses
//!   `α` at the liquid sound speed, which in the first runs smeared the
//!   faucet front over metres and made a trace phase at a void step lose
//!   positivity within one stage; HLLC was adopted for that reason (V&V
//!   record).
//! - **Non-conservative terms**, discretised so that a uniform pressure and
//!   velocity stay uniform across a volume-fraction discontinuity (Abgrall's
//!   criterion, the property Saurel & Abgrall build on): `∂α/∂x` from the
//!   SAME face `α` the phase fluxes use, and the `α` equation in the form
//!   `∂α/∂t + ∂(α u_int)/∂x − α ∂u_int/∂x = …`. The proof is a few lines:
//!   at uniform `(p, u)` the phase momentum flux is `u` × the mass flux plus
//!   `p α_face`, which `p_int Δα_face` cancels when `p_int = p`; the
//!   internal-energy flux is `ρe_k` × the `α` flux. Pinned by
//!   `seven_equation::tests::uniform_pressure_and_velocity_survive_a_void_discontinuity`.
//! - **Reconstruction**: MUSCL with the minmod limiter on the primitive
//!   variables `(α_v, ρ_k, u_k, p_k, Y)` (default), or first order.
//! - **Time**: SSP-RK2 (Heun), adaptive `Δt` from the Courant number.
//! - **Area**: the momentum source is
//!   `p_int Δ(ᾱ A) + (p_k − p_int) α_k ΔA`, which is exactly
//!   `p_int A ∂α + p_k α ∂A` and keeps a fluid at rest in a variable-area duct
//!   at rest.
//!
//! # Honest scope
//!
//! - No wall friction, wall heat or finite-rate mass transfer.
//! - **`p_int` is cell-centred.** At a material interface with a large
//!   pressure jump and trace phases (Saurel & Abgrall's water–air shock tube,
//!   1e9 | 1e5 Pa), the water flux enters the air-side cell with `α_l ≈ 1`
//!   while the cell's `p_int` is near the air pressure, and the water trace
//!   there loses positivity in the first stage: the run is **refused**
//!   (pinned by
//!   `tests::the_water_air_interface_shock_tube_is_refused_by_cell_centred_p_int`).
//!   Face-based interfacial pressures (the discrete equation method of
//!   Abgrall & Saurel 2003) would be needed and are not implemented.
//! - The HLLC acoustic dissipation is still large at low Mach number; void
//!   fronts are smeared over a few cells (faucet record). Where the Davis
//!   wave speeds put the HLLC contact outside `[S_L, S_R]` (a strong
//!   rarefaction) that phase's face flux falls back to Rusanov.
//! - **HLLC loses a trace phase's positivity beside a flashing cell**: the
//!   Edwards blowdown in the HEM limit is refused at 0.44 ms with the HLLC
//!   default and is run with [`SevenEquation1d::without_hllc`] (V&V record).
//! - The water/steam pair of Le Métayer et al. (2004) puts `T_sat` 15–35 K
//!   below IAPWS-IF97 (gh:#1057), so a water/steam result here inherits that
//!   offset; the Edwards record states what it does to the plateau.
//! - Boundaries are ghost cells: wall, transmissive, velocity inlet with a
//!   prescribed state, and a static-pressure outlet. There is no critical-flow
//!   break model; a pressure outlet chokes only as the scheme does.
//!
//! # Units
//!
//! Constructors and accessors are `uom`-typed where a quantity crosses the
//! API; per-cell state and the inner loops are raw SI (Pa, kg/m³, m/s, J/kg),
//! as in [`super::two_fluid`].

use petir::mathf::RealMath;
use petir::roots::{BracketingMethod, BracketingSolver};
use tampines_steam_tables::hem::{StiffenedGasHem, StiffenedGasPair};
use uom::si::f64::{Length, Mass, MassDensity, Pressure, Ratio, ThermodynamicTemperature, Time, Velocity};
use uom::si::length::meter;
use uom::si::mass::kilogram;
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::pressure::pascal;
use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;
use uom::si::velocity::meter_per_second;

use super::geometry::Pipe1d;
use super::interfacial::InterfacialExchange;
use crate::TampinesError;

/// Number of conserved quantities per cell: `α_v`, `(m, m u, m E)` per
/// phase, and the non-condensable concentration `c = α_v ρ_v Y`.
pub const N_CONSERVED: usize = 8;

/// Default Courant number for the explicit march.
pub const DEFAULT_COURANT: f64 = 0.4;

/// Smallest volume fraction a phase may hold \[-\]. A state that drives a
/// phase below it is **refused**, not clipped; initial states carry a trace
/// of the absent phase above it.
pub const MIN_VOLUME_FRACTION: f64 = 1.0e-12;

/// Below this volume fraction a phase counts as a trace for the
/// thermo-chemical relaxation: it is not forced to saturation unless it is
/// metastable \[-\].
pub const TRACE_VOLUME_FRACTION: f64 = 1.0e-4;

// ─────────────────────────────────────────────────────────────────────────────
//  Stiffened-gas helpers (raw SI)
// ─────────────────────────────────────────────────────────────────────────────

fn sg_pressure(g: &StiffenedGasHem, rho: f64, e: f64) -> f64 {
    (g.gamma - 1.0) * rho * (e - g.q_j_per_kg) - g.gamma * g.p_inf_pa
}

fn sg_energy(g: &StiffenedGasHem, rho: f64, p: f64) -> f64 {
    (p + g.gamma * g.p_inf_pa) / ((g.gamma - 1.0) * rho) + g.q_j_per_kg
}

fn sg_sound_speed(g: &StiffenedGasHem, rho: f64, p: f64) -> f64 {
    (g.gamma * (p + g.p_inf_pa) / rho).sqrt()
}

fn sg_temperature(g: &StiffenedGasHem, rho: f64, p: f64) -> f64 {
    (p + g.p_inf_pa) / ((g.gamma - 1.0) * rho * g.cv_j_per_kg_k)
}

fn sg_density_pt(g: &StiffenedGasHem, p: f64, t: f64) -> f64 {
    (p + g.p_inf_pa) / ((g.gamma - 1.0) * g.cv_j_per_kg_k * t)
}

fn sg_energy_pt(g: &StiffenedGasHem, p: f64, t: f64) -> f64 {
    g.cv_j_per_kg_k * t * (p + g.gamma * g.p_inf_pa) / (p + g.p_inf_pa) + g.q_j_per_kg
}

/// Specific entropy of a stiffened gas \[J/(kg·K)\],
/// `s = c_v ln(T^γ / (p + p∞)^(γ−1)) + q'`, from the Gibbs energy of RELAP-7
/// Eq 82 (`s = (h − g)/T`).
fn sg_entropy(g: &StiffenedGasHem, p: f64, t: f64) -> f64 {
    g.cv_j_per_kg_k * (g.gamma * t.r_ln() - (g.gamma - 1.0) * (p + g.p_inf_pa).r_ln())
        + g.q_prime_j_per_kg_k
}

/// `true` if `(ρ, p)` is an admissible stiffened-gas state.
fn admissible(g: &StiffenedGasHem, rho: f64, p: f64) -> bool {
    rho.is_finite() && rho > 0.0 && p.is_finite() && p + g.p_inf_pa > 0.0
}

// ─────────────────────────────────────────────────────────────────────────────
//  Public state and closure choices
// ─────────────────────────────────────────────────────────────────────────────

/// The primitive state of one cell.
///
/// Raw SI fields (this is a marching-loop type, as
/// [`super::interfacial::InterfacialCellState`]); build it from `uom`
/// quantities with [`new`](Self::new) or [`at_temperatures`](Self::at_temperatures).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SevenEquationState {
    /// Vapour (gas) volume fraction `α_v` \[-\].
    pub void_fraction: f64,
    /// Vapour density `ρ_v` \[kg/m³\].
    pub vapour_density: f64,
    /// Vapour velocity `u_v` \[m/s\].
    pub vapour_velocity: f64,
    /// Vapour pressure `p_v` \[Pa\].
    pub vapour_pressure: f64,
    /// Liquid density `ρ_l` \[kg/m³\].
    pub liquid_density: f64,
    /// Liquid velocity `u_l` \[m/s\].
    pub liquid_velocity: f64,
    /// Liquid pressure `p_l` \[Pa\].
    pub liquid_pressure: f64,
    /// Non-condensable mass fraction of the gas `Y` \[-\] (gh:#1064).
    pub noncondensable_mass_fraction: f64,
}

impl SevenEquationState {
    /// A state from `uom` quantities, both phases at pressure `pressure` and
    /// velocity `velocity`, with no non-condensable.
    #[must_use]
    pub fn new(
        void_fraction: Ratio,
        vapour_density: MassDensity,
        liquid_density: MassDensity,
        pressure: Pressure,
        velocity: Velocity,
    ) -> Self {
        let p = pressure.get::<pascal>();
        let u = velocity.get::<meter_per_second>();
        Self {
            void_fraction: void_fraction.get::<ratio>(),
            vapour_density: vapour_density.get::<kilogram_per_cubic_meter>(),
            vapour_velocity: u,
            vapour_pressure: p,
            liquid_density: liquid_density.get::<kilogram_per_cubic_meter>(),
            liquid_velocity: u,
            liquid_pressure: p,
            noncondensable_mass_fraction: 0.0,
        }
    }

    /// A mechanical-equilibrium state at `pressure`, each phase at its own
    /// temperature, densities from the stiffened-gas caloric law (RELAP-7
    /// Eq 80).
    #[must_use]
    pub fn at_temperatures(
        eos: &StiffenedGasPair,
        void_fraction: Ratio,
        pressure: Pressure,
        vapour_temperature: ThermodynamicTemperature,
        liquid_temperature: ThermodynamicTemperature,
        velocity: Velocity,
    ) -> Self {
        let p = pressure.get::<pascal>();
        let u = velocity.get::<meter_per_second>();
        Self {
            void_fraction: void_fraction.get::<ratio>(),
            vapour_density: sg_density_pt(&eos.vapour, p, vapour_temperature.get::<kelvin>()),
            vapour_velocity: u,
            vapour_pressure: p,
            liquid_density: sg_density_pt(&eos.liquid, p, liquid_temperature.get::<kelvin>()),
            liquid_velocity: u,
            liquid_pressure: p,
            noncondensable_mass_fraction: 0.0,
        }
    }
}

/// Interfacial pressure and velocity (RELAP-7 Eqs 191–196).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterfacialVariables {
    /// Acoustic-impedance weighted (discrete equation method). Default.
    ImpedanceWeighted,
    /// Baer–Nunziato: `u_int = u_l`, `p_int = p_v` (liquid as the dense phase).
    BaerNunziato,
    /// Saurel–Abgrall (1999): `p_int = Σ α_k p_k`, `u_int` mass weighted.
    SaurelAbgrall,
}

/// How a relaxation process is applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Relaxation {
    /// Not applied (an ablation).
    Off,
    /// At the finite rate of RELAP-7 Eqs 197–198. Default.
    Finite,
    /// Stiff, `μ` or `λ → ∞`: to equilibrium within each step.
    Instantaneous,
}

/// Interphase mass transfer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MassTransfer {
    /// None. Default (see the module docs for why).
    None,
    /// Instantaneous thermo-chemical relaxation to saturation (HEM limit).
    Instantaneous,
}

/// Where the interfacial area concentration comes from.
pub enum InterfacialAreaSource {
    /// The gh:#1060 closure set's area (regime map by default).
    Exchange(InterfacialExchange),
    /// A constant `A_int` \[1/m\].
    Constant(f64),
}

/// A ghost-cell boundary condition.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SevenEquationBoundary {
    /// Rigid wall: velocities mirrored.
    Wall,
    /// Zero gradient.
    Transmissive,
    /// Prescribed inflow state (volume fraction, densities, velocities, `Y`);
    /// each phase's pressure is extrapolated from the adjacent cell.
    VelocityInlet(SevenEquationState),
    /// Static pressure \[Pa\] on both phases; everything else extrapolated.
    PressureOutlet {
        /// Static pressure \[Pa\].
        pressure: f64,
    },
}

/// What the mass-transfer operator did to one cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MassTransferOutcome {
    /// Neither two-phase nor metastable, or the operator is off.
    NotNeeded,
    /// Taken to saturation.
    Acted,
    /// Attempted, but no admissible equilibrium.
    Skipped,
}

/// The numerical flux of the hyperbolic step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FluxScheme {
    /// HLLC per phase (Toro, Spruce & Speares 1994; Toro 2009 §10.4), `α`
    /// upwinded on the face `u_int`. Default.
    Hllc,
    /// Rusanov with one wave-speed bound for every equation and centred `ᾱ`.
    Rusanov,
}

/// What one [`SevenEquation1d::step`] did.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SevenEquationReport {
    /// Time at the end of the step \[s\].
    pub time: f64,
    /// The step taken \[s\].
    pub dt: f64,
    /// Largest `|p_v − p_l|` after relaxation \[Pa\].
    pub max_pressure_disequilibrium: f64,
    /// Largest `|u_v − u_l|` after relaxation \[m/s\].
    pub max_slip: f64,
    /// Smallest and largest `α_v`.
    pub min_void_fraction: f64,
    /// Largest `α_v`.
    pub max_void_fraction: f64,
    /// Cells taken to saturation by the mass-transfer operator.
    pub mass_transfer_cells: usize,
    /// Two-phase or metastable cells where the operator found no admissible
    /// equilibrium (the equilibrium quality outside `(0, 1)`, i.e. one phase
    /// would have to vanish, or no bracket) and left the cell as it was.
    pub mass_transfer_skipped_cells: usize,
    /// Largest `|T_k − T_sat(p)|` \[K\] over the cells the operator acted on,
    /// after it acted: the HEM-state check.
    pub max_saturation_departure: f64,
}

// ─────────────────────────────────────────────────────────────────────────────
//  Characteristic analysis
// ─────────────────────────────────────────────────────────────────────────────

/// The seven characteristic speeds of the hyperbolic part \[m/s\]:
/// `u_v − c_v, u_v, u_v + c_v, u_l − c_l, u_l, u_l + c_l, u_int`, all real for
/// every admissible state (the model's hyperbolicity). `u_int` is the
/// impedance-weighted one without its sign term.
///
/// # Errors
///
/// [`TampinesError::Unphysical`] for an inadmissible state.
pub fn characteristic_speeds(
    eos: &StiffenedGasPair,
    state: &SevenEquationState,
) -> Result<[f64; 7], TampinesError> {
    let s = state;
    if !admissible(&eos.vapour, s.vapour_density, s.vapour_pressure)
        || !admissible(&eos.liquid, s.liquid_density, s.liquid_pressure)
    {
        return Err(TampinesError::Unphysical(format!("inadmissible state {s:?}")));
    }
    let c_v = sg_sound_speed(&eos.vapour, s.vapour_density, s.vapour_pressure);
    let c_l = sg_sound_speed(&eos.liquid, s.liquid_density, s.liquid_pressure);
    let (z_v, z_l) = (s.vapour_density * c_v, s.liquid_density * c_l);
    let u_int = (z_l * s.liquid_velocity + z_v * s.vapour_velocity) / (z_l + z_v);
    Ok([
        s.vapour_velocity - c_v,
        s.vapour_velocity,
        s.vapour_velocity + c_v,
        s.liquid_velocity - c_l,
        s.liquid_velocity,
        s.liquid_velocity + c_l,
        u_int,
    ])
}

/// The four acoustic/void characteristic speeds of the **single-pressure,
/// two-velocity** system that relaxing `μ` alone leaves (RELAP-7 Remark 2):
/// the roots `λ` of
///
/// `α_v ρ_l (u_l − λ)² ((u_v − λ)² − c_v²)/c_v² + α_l ρ_v (u_v − λ)² ((u_l − λ)² − c_l²)/c_l² = 0`,
///
/// derived here from the linearised phase mass and momentum equations with a
/// common pressure, `p_int = p` and isentropic phases (plane wave
/// `exp(i k (x − λ t))`; the entropy waves at `u_k` are separate). Roots from
/// PETIR's companion-matrix solver; each is `(re, im)` \[m/s\]. A non-zero
/// imaginary part means the initial-value problem is ill posed: a short-wave
/// perturbation grows as `exp(k |im| t)`, faster for every shorter wave.
///
/// # Errors
///
/// [`TampinesError::InvalidInput`] for non-positive densities or sound speeds
/// or a volume fraction outside `(0, 1)`.
#[allow(clippy::too_many_arguments)]
pub fn single_pressure_characteristic_speeds(
    void_fraction: f64,
    vapour_density: f64,
    liquid_density: f64,
    vapour_sound_speed: f64,
    liquid_sound_speed: f64,
    vapour_velocity: f64,
    liquid_velocity: f64,
) -> Result<Vec<(f64, f64)>, TampinesError> {
    let (a_v, rv, rl, cv, cl) = (
        void_fraction,
        vapour_density,
        liquid_density,
        vapour_sound_speed,
        liquid_sound_speed,
    );
    if !(a_v > 0.0 && a_v < 1.0 && rv > 0.0 && rl > 0.0 && cv > 0.0 && cl > 0.0) {
        return Err(TampinesError::InvalidInput(
            "single-pressure characteristics need 0 < alpha < 1 and positive rho, c".to_string(),
        ));
    }
    let a_l = 1.0 - a_v;
    // Polynomials in λ, ascending coefficients.
    let mul = |a: &[f64], b: &[f64]| -> Vec<f64> {
        let mut r = vec![0.0; a.len() + b.len() - 1];
        for (i, x) in a.iter().enumerate() {
            for (j, y) in b.iter().enumerate() {
                r[i + j] += x * y;
            }
        }
        r
    };
    let dv = [vapour_velocity, -1.0];
    let dl = [liquid_velocity, -1.0];
    let dv2 = mul(&dv, &dv);
    let dl2 = mul(&dl, &dl);
    let mut dv2_c = dv2.clone();
    dv2_c[0] -= cv * cv;
    let mut dl2_c = dl2.clone();
    dl2_c[0] -= cl * cl;
    let t1 = mul(&dl2, &dv2_c);
    let t2 = mul(&dv2, &dl2_c);
    let w1 = a_v * rl / (cv * cv);
    let w2 = a_l * rv / (cl * cl);
    let ascending: Vec<f64> = (0..5).map(|i| w1 * t1[i] + w2 * t2[i]).collect();
    let descending: Vec<f64> = ascending.iter().rev().copied().collect();
    Ok(petir::poly::companion::roots_companion(&descending)
        .into_iter()
        .map(|r| (r.re, r.im))
        .collect())
}

/// Wood's mixture sound speed \[m/s\], `1/(ρ c²) = Σ_k α_k/(ρ_k c_k²)` — the
/// sound speed of Kapila's five-equation model, the `μ, λ → ∞` limit
/// (RELAP-7 Remark 1).
#[must_use]
pub fn wood_sound_speed(
    void_fraction: f64,
    vapour_density: f64,
    liquid_density: f64,
    vapour_sound_speed: f64,
    liquid_sound_speed: f64,
) -> f64 {
    let a_l = 1.0 - void_fraction;
    let rho = void_fraction * vapour_density + a_l * liquid_density;
    let inv = void_fraction / (vapour_density * vapour_sound_speed * vapour_sound_speed)
        + a_l / (liquid_density * liquid_sound_speed * liquid_sound_speed);
    (1.0 / (rho * inv)).sqrt()
}

// ─────────────────────────────────────────────────────────────────────────────
//  The solver
// ─────────────────────────────────────────────────────────────────────────────

/// Per-cell primitive variables used inside the march.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Prim {
    a: f64,
    rv: f64,
    uv: f64,
    pv: f64,
    rl: f64,
    ul: f64,
    pl: f64,
    y: f64,
}

impl Prim {
    fn from_state(s: &SevenEquationState) -> Self {
        Self {
            a: s.void_fraction,
            rv: s.vapour_density,
            uv: s.vapour_velocity,
            pv: s.vapour_pressure,
            rl: s.liquid_density,
            ul: s.liquid_velocity,
            pl: s.liquid_pressure,
            y: s.noncondensable_mass_fraction,
        }
    }

    fn to_state(self) -> SevenEquationState {
        SevenEquationState {
            void_fraction: self.a,
            vapour_density: self.rv,
            vapour_velocity: self.uv,
            vapour_pressure: self.pv,
            liquid_density: self.rl,
            liquid_velocity: self.ul,
            liquid_pressure: self.pl,
            noncondensable_mass_fraction: self.y,
        }
    }

    fn to_array(self) -> [f64; 8] {
        [self.a, self.rv, self.uv, self.pv, self.rl, self.ul, self.pl, self.y]
    }

    fn from_array(v: [f64; 8]) -> Self {
        Self {
            a: v[0],
            rv: v[1],
            uv: v[2],
            pv: v[3],
            rl: v[4],
            ul: v[5],
            pl: v[6],
            y: v[7],
        }
    }
}

/// The seven-equation two-pressure solver. See the module docs.
pub struct SevenEquation1d {
    pipe: Pipe1d,
    eos: StiffenedGasPair,
    /// Conserved state per unit volume, `[α_v, m_v, m_v u_v, m_v E_v, m_l,
    /// m_l u_l, m_l E_l, c]`.
    u: Vec<[f64; N_CONSERVED]>,
    time: f64,
    courant: f64,
    max_dt: f64,
    left: SevenEquationBoundary,
    right: SevenEquationBoundary,
    /// Multiplier on the right end face's area (a break opening), `(0, 1]`.
    right_face_area_fraction: f64,
    interfacial: InterfacialVariables,
    sign_terms: bool,
    pressure_relaxation: Relaxation,
    velocity_relaxation: Relaxation,
    mass_transfer: MassTransfer,
    area_source: InterfacialAreaSource,
    muscl: bool,
    flux: FluxScheme,
}

impl SevenEquation1d {
    /// Build a solver on `pipe` with the stiffened-gas pair `eos` and one
    /// initial state per cell.
    ///
    /// Defaults (physics on): impedance-weighted `p_int`, `u_int` with their
    /// sign terms; finite pressure and velocity relaxation (Eqs 197–198) with
    /// `A_int` from the gh:#1060 regime map on a 1 mm bubbly closure set at
    /// the pipe's hydraulic diameter; MUSCL-minmod; Courant 0.4; no mass
    /// transfer; closed (wall) ends.
    ///
    /// # Errors
    ///
    /// [`TampinesError::InvalidInput`] for a profile of the wrong length;
    /// [`TampinesError::Unphysical`] for an inadmissible state (a volume
    /// fraction outside `[MIN_VOLUME_FRACTION, 1 − MIN_VOLUME_FRACTION]`, a
    /// non-positive density, `p + p∞ ≤ 0`).
    pub fn new(
        pipe: Pipe1d,
        eos: StiffenedGasPair,
        initial: &[SevenEquationState],
    ) -> Result<Self, TampinesError> {
        if initial.len() != pipe.n_cells() {
            return Err(TampinesError::InvalidInput(format!(
                "initial profile has {} cells, pipe has {}",
                initial.len(),
                pipe.n_cells()
            )));
        }
        let exchange = InterfacialExchange::bubbly(Length::new::<meter>(1.0e-3))?
            .with_regime_map(pipe.hydraulic_diameter())?;
        let mut s = Self {
            pipe,
            eos,
            u: Vec::new(),
            time: 0.0,
            courant: DEFAULT_COURANT,
            max_dt: 1.0e-3,
            left: SevenEquationBoundary::Wall,
            right: SevenEquationBoundary::Wall,
            right_face_area_fraction: 1.0,
            interfacial: InterfacialVariables::ImpedanceWeighted,
            sign_terms: true,
            pressure_relaxation: Relaxation::Finite,
            velocity_relaxation: Relaxation::Finite,
            mass_transfer: MassTransfer::None,
            area_source: InterfacialAreaSource::Exchange(exchange),
            muscl: true,
            flux: FluxScheme::Hllc,
        };
        s.set_state(initial)?;
        Ok(s)
    }

    /// Overwrite every cell's state.
    ///
    /// # Errors
    ///
    /// As [`new`](Self::new).
    pub fn set_state(&mut self, states: &[SevenEquationState]) -> Result<(), TampinesError> {
        if states.len() != self.pipe.n_cells() {
            return Err(TampinesError::InvalidInput(format!(
                "state profile has {} cells, pipe has {}",
                states.len(),
                self.pipe.n_cells()
            )));
        }
        let mut u = Vec::with_capacity(states.len());
        for (i, s) in states.iter().enumerate() {
            let p = Prim::from_state(s);
            self.check_prim(&p, i)?;
            u.push(self.conserved(&p));
        }
        self.u = u;
        Ok(())
    }

    // ── builder-style options ──────────────────────────────────────────

    /// **Ablation**: no pressure relaxation (`μ = 0`).
    #[must_use]
    pub fn without_pressure_relaxation(mut self) -> Self {
        self.pressure_relaxation = Relaxation::Off;
        self
    }

    /// **Ablation**: no velocity relaxation (`λ = 0`).
    #[must_use]
    pub fn without_velocity_relaxation(mut self) -> Self {
        self.velocity_relaxation = Relaxation::Off;
        self
    }

    /// **Ablation**: drop the `sgn(∂α_l/∂x)` terms of the impedance-weighted
    /// `p_int` and `u_int`, leaving the impedance averages.
    #[must_use]
    pub fn without_interface_sign_terms(mut self) -> Self {
        self.sign_terms = false;
        self
    }

    /// **Ablation**: the regime-map interfacial area replaced by the fixed
    /// 1 mm bubble-diameter area `6 α_v / d` (gh:#1060's own ablation).
    #[must_use]
    pub fn without_regime_map(mut self) -> Self {
        let area = std::mem::replace(&mut self.area_source, InterfacialAreaSource::Constant(0.0));
        self.area_source = match area {
            InterfacialAreaSource::Exchange(x) => InterfacialAreaSource::Exchange(x.without_regime_map()),
            other => other,
        };
        self
    }

    /// **Ablation**: the Rusanov (local Lax–Friedrichs) flux with centred
    /// `ᾱ` instead of HLLC with upwinded `α` (both satisfy Abgrall's
    /// criterion; Rusanov diffuses `α` at the acoustic speed).
    #[must_use]
    pub fn without_hllc(mut self) -> Self {
        self.flux = FluxScheme::Rusanov;
        self
    }

    /// The flux scheme in use.
    #[must_use]
    pub fn flux_scheme(&self) -> FluxScheme {
        self.flux
    }

    /// **Ablation**: first-order (piecewise-constant) reconstruction.
    #[must_use]
    pub fn without_muscl(mut self) -> Self {
        self.muscl = false;
        self
    }

    /// Both relaxations stiff (`μ, λ → ∞`): the Kapila five-equation limit
    /// (Remark 1).
    #[must_use]
    pub fn with_instantaneous_relaxation(mut self) -> Self {
        self.pressure_relaxation = Relaxation::Instantaneous;
        self.velocity_relaxation = Relaxation::Instantaneous;
        self
    }

    /// Set the pressure relaxation mode.
    #[must_use]
    pub fn with_pressure_relaxation(mut self, r: Relaxation) -> Self {
        self.pressure_relaxation = r;
        self
    }

    /// Set the velocity relaxation mode.
    #[must_use]
    pub fn with_velocity_relaxation(mut self, r: Relaxation) -> Self {
        self.velocity_relaxation = r;
        self
    }

    /// Instantaneous thermo-chemical relaxation (with instantaneous
    /// mechanical relaxation: HEM, Remark 1).
    #[must_use]
    pub fn with_instantaneous_mass_transfer(mut self) -> Self {
        self.mass_transfer = MassTransfer::Instantaneous;
        self
    }

    /// Choose the interfacial pressure and velocity (Eqs 191–196).
    #[must_use]
    pub fn with_interfacial_variables(mut self, v: InterfacialVariables) -> Self {
        self.interfacial = v;
        self
    }

    /// A constant interfacial area concentration \[1/m\] instead of the
    /// regime map.
    ///
    /// # Errors
    ///
    /// [`TampinesError::InvalidInput`] for a negative or non-finite value.
    pub fn with_constant_interfacial_area(mut self, a_int: f64) -> Result<Self, TampinesError> {
        if !(a_int >= 0.0 && a_int.is_finite()) {
            return Err(TampinesError::InvalidInput(format!(
                "interfacial area must be finite and >= 0 (got {a_int})"
            )));
        }
        self.area_source = InterfacialAreaSource::Constant(a_int);
        Ok(self)
    }

    /// Set the Courant number and the largest step.
    ///
    /// # Errors
    ///
    /// [`TampinesError::InvalidInput`] unless `0 < courant ≤ 0.5` and
    /// `max_dt > 0`.
    pub fn set_time_step_control(&mut self, courant: f64, max_dt: Time) -> Result<(), TampinesError> {
        let max_dt = max_dt.get::<second>();
        if !(courant > 0.0 && courant <= 0.5 && max_dt > 0.0) {
            return Err(TampinesError::InvalidInput(format!(
                "need 0 < Courant <= 0.5 and max_dt > 0 (got {courant}, {max_dt})"
            )));
        }
        self.courant = courant;
        self.max_dt = max_dt;
        Ok(())
    }

    /// Set the left boundary.
    pub fn set_left_boundary(&mut self, bc: SevenEquationBoundary) {
        self.left = bc;
    }

    /// Set the right boundary.
    pub fn set_right_boundary(&mut self, bc: SevenEquationBoundary) {
        self.right = bc;
    }

    /// Scale the right end face's area by `fraction` (a break opening).
    ///
    /// # Errors
    ///
    /// [`TampinesError::InvalidInput`] unless `0 < fraction ≤ 1`.
    pub fn set_right_face_area_fraction(&mut self, fraction: f64) -> Result<(), TampinesError> {
        if !(fraction > 0.0 && fraction <= 1.0) {
            return Err(TampinesError::InvalidInput(format!(
                "face area fraction must be in (0, 1] (got {fraction})"
            )));
        }
        self.right_face_area_fraction = fraction;
        Ok(())
    }

    // ── accessors ──────────────────────────────────────────────────────

    /// Current time.
    #[must_use]
    pub fn time(&self) -> Time {
        Time::new::<second>(self.time)
    }

    /// The pipe.
    #[must_use]
    pub fn pipe(&self) -> &Pipe1d {
        &self.pipe
    }

    /// The equation of state.
    #[must_use]
    pub fn eos(&self) -> &StiffenedGasPair {
        &self.eos
    }

    /// Pressure relaxation mode.
    #[must_use]
    pub fn pressure_relaxation(&self) -> Relaxation {
        self.pressure_relaxation
    }

    /// Velocity relaxation mode.
    #[must_use]
    pub fn velocity_relaxation(&self) -> Relaxation {
        self.velocity_relaxation
    }

    /// Mass-transfer mode.
    #[must_use]
    pub fn mass_transfer(&self) -> MassTransfer {
        self.mass_transfer
    }

    /// Interfacial-variable choice.
    #[must_use]
    pub fn interfacial_variables(&self) -> InterfacialVariables {
        self.interfacial
    }

    /// Whether the impedance-weighted sign terms are applied.
    #[must_use]
    pub fn uses_interface_sign_terms(&self) -> bool {
        self.sign_terms
    }

    /// Whether MUSCL reconstruction is applied.
    #[must_use]
    pub fn uses_muscl(&self) -> bool {
        self.muscl
    }

    /// Primitive state of every cell.
    ///
    /// # Errors
    ///
    /// As [`step`](Self::step) for an inadmissible cell (cannot happen after
    /// a successful step).
    pub fn states(&self) -> Result<Vec<SevenEquationState>, TampinesError> {
        (0..self.u.len())
            .map(|i| self.prim(i, &self.u[i]).map(Prim::to_state))
            .collect()
    }

    /// The conserved state per unit volume of every cell,
    /// `[α_v, m_v, m_v u_v, m_v E_v, m_l, m_l u_l, m_l E_l, c]`.
    #[must_use]
    pub fn conserved_state(&self) -> &[[f64; N_CONSERVED]] {
        &self.u
    }

    /// Total mass of each phase and of the non-condensable \[kg\]:
    /// `(vapour, liquid, non-condensable)`.
    #[must_use]
    pub fn inventories(&self) -> (Mass, Mass, Mass) {
        let mut m = [0.0; 3];
        for (i, u) in self.u.iter().enumerate() {
            let v = self.pipe.cell_volume_at(i);
            m[0] += u[1] * v;
            m[1] += u[4] * v;
            m[2] += u[7] * v;
        }
        (
            Mass::new::<kilogram>(m[0]),
            Mass::new::<kilogram>(m[1]),
            Mass::new::<kilogram>(m[2]),
        )
    }

    /// Total energy in the pipe \[J\], `Σ_i V_i (m_v E_v + m_l E_l)`.
    #[must_use]
    pub fn total_energy(&self) -> f64 {
        self.u
            .iter()
            .enumerate()
            .map(|(i, u)| (u[3] + u[6]) * self.pipe.cell_volume_at(i))
            .sum()
    }

    /// The area-averaged entropy inventory \[J/K\] (gh:#1064, RELAP-7 Eqs 18,
    /// 23), `Σ_i V_i (m_v s_v + m_l s_l)` with stiffened-gas entropies.
    ///
    /// # Errors
    ///
    /// As [`states`](Self::states).
    pub fn entropy_inventory(&self) -> Result<f64, TampinesError> {
        let mut s = 0.0;
        for i in 0..self.u.len() {
            let p = self.prim(i, &self.u[i])?;
            let tv = sg_temperature(&self.eos.vapour, p.rv, p.pv);
            let tl = sg_temperature(&self.eos.liquid, p.rl, p.pl);
            s += self.pipe.cell_volume_at(i)
                * (self.u[i][1] * sg_entropy(&self.eos.vapour, p.pv, tv)
                    + self.u[i][4] * sg_entropy(&self.eos.liquid, p.pl, tl));
        }
        Ok(s)
    }

    /// Phase temperatures `(T_v, T_l)` \[K\] per cell.
    ///
    /// # Errors
    ///
    /// As [`states`](Self::states).
    pub fn temperatures(&self) -> Result<Vec<(f64, f64)>, TampinesError> {
        Ok(self
            .states()?
            .iter()
            .map(|s| {
                (
                    sg_temperature(&self.eos.vapour, s.vapour_density, s.vapour_pressure),
                    sg_temperature(&self.eos.liquid, s.liquid_density, s.liquid_pressure),
                )
            })
            .collect())
    }

    /// Mixture pressure `α_v p_v + α_l p_l` \[Pa\] per cell.
    ///
    /// # Errors
    ///
    /// As [`states`](Self::states).
    pub fn mixture_pressure(&self) -> Result<Vec<f64>, TampinesError> {
        Ok(self
            .states()?
            .iter()
            .map(|s| s.void_fraction * s.vapour_pressure + (1.0 - s.void_fraction) * s.liquid_pressure)
            .collect())
    }

    /// The relaxation coefficients `(μ, λ)` of RELAP-7 Eqs 197–198 at a state
    /// \[1/(Pa·s)\], \[kg/(m³·s)\]: `μ = A_int/(Z_l + Z_v)`, `λ = ½ μ Z_l Z_v`.
    ///
    /// # Errors
    ///
    /// As the regime-map area for a `NaN` volume fraction.
    pub fn relaxation_coefficients(
        &self,
        state: &SevenEquationState,
    ) -> Result<(f64, f64), TampinesError> {
        let p = Prim::from_state(state);
        let (z_v, z_l) = self.impedances(&p);
        let a_int = self.interfacial_area(p.a)?;
        let mu = a_int / (z_l + z_v);
        Ok((mu, 0.5 * mu * z_l * z_v))
    }

    // ── the march ──────────────────────────────────────────────────────

    /// Advance one adaptive step: SSP-RK2 hyperbolic step, then velocity
    /// relaxation, pressure relaxation and (if enabled) mass transfer.
    ///
    /// # Errors
    ///
    /// [`TampinesError::Unphysical`] if any cell leaves the admissible set
    /// (refused, the state is left unchanged); [`TampinesError::Numerical`]
    /// if a relaxation root cannot be bracketed.
    pub fn step(&mut self) -> Result<SevenEquationReport, TampinesError> {
        let n = self.u.len();
        let prims: Vec<Prim> = (0..n)
            .map(|i| self.prim(i, &self.u[i]))
            .collect::<Result<_, _>>()?;
        let mut smax: f64 = 0.0;
        for p in &prims {
            let cv = sg_sound_speed(&self.eos.vapour, p.rv, p.pv);
            let cl = sg_sound_speed(&self.eos.liquid, p.rl, p.pl);
            smax = smax.max(p.uv.abs() + cv).max(p.ul.abs() + cl);
        }
        let dt = (self.courant * self.pipe.dx() / smax).min(self.max_dt);

        let l0 = self.residual(&self.u)?;
        let mut u1: Vec<[f64; N_CONSERVED]> = (0..n)
            .map(|i| add_scaled(&self.u[i], &l0[i], dt))
            .collect();
        for (i, u) in u1.iter_mut().enumerate() {
            self.relax_cell(u, dt, i, false)?;
        }
        let l1 = self.residual(&u1)?;
        let mut u2: Vec<[f64; N_CONSERVED]> = (0..n)
            .map(|i| {
                let a = add_scaled(&u1[i], &l1[i], dt);
                let mut r = [0.0; N_CONSERVED];
                for k in 0..N_CONSERVED {
                    r[k] = 0.5 * (self.u[i][k] + a[k]);
                }
                r
            })
            .collect();

        let mut mass_transfer_cells = 0usize;
        let mut mass_transfer_skipped_cells = 0usize;
        let mut max_saturation_departure: f64 = 0.0;
        for (i, u) in u2.iter_mut().enumerate() {
            match self.relax_cell(u, dt, i, true)? {
                MassTransferOutcome::Acted => {
                    mass_transfer_cells += 1;
                    let p = self.prim(i, u)?;
                    let t_sat = self
                        .eos
                        .saturation_temperature(Pressure::new::<pascal>(p.pl))?
                        .get::<kelvin>();
                    let tv = sg_temperature(&self.eos.vapour, p.rv, p.pv);
                    let tl = sg_temperature(&self.eos.liquid, p.rl, p.pl);
                    max_saturation_departure =
                        max_saturation_departure.max((tv - t_sat).abs()).max((tl - t_sat).abs());
                }
                MassTransferOutcome::Skipped => mass_transfer_skipped_cells += 1,
                MassTransferOutcome::NotNeeded => {}
            }
        }
        let mut report = SevenEquationReport {
            time: self.time + dt,
            dt,
            max_pressure_disequilibrium: 0.0,
            max_slip: 0.0,
            min_void_fraction: 1.0,
            max_void_fraction: 0.0,
            mass_transfer_cells,
            mass_transfer_skipped_cells,
            max_saturation_departure,
        };
        for (i, u) in u2.iter().enumerate() {
            let p = self.prim(i, u)?;
            report.max_pressure_disequilibrium = report.max_pressure_disequilibrium.max((p.pv - p.pl).abs());
            report.max_slip = report.max_slip.max((p.uv - p.ul).abs());
            report.min_void_fraction = report.min_void_fraction.min(p.a);
            report.max_void_fraction = report.max_void_fraction.max(p.a);
        }
        self.u = u2;
        self.time += dt;
        Ok(report)
    }

    /// Step until `t_end` (the last step is shortened to land on it).
    ///
    /// # Errors
    ///
    /// As [`step`](Self::step).
    pub fn run_until(&mut self, t_end: Time) -> Result<usize, TampinesError> {
        let t_end = t_end.get::<second>();
        let mut steps = 0;
        let max_dt = self.max_dt;
        while self.time < t_end * (1.0 - 1.0e-14) {
            let remaining = t_end - self.time;
            self.max_dt = max_dt.min(remaining);
            let r = self.step();
            self.max_dt = max_dt;
            r?;
            steps += 1;
        }
        Ok(steps)
    }

    // ── internals ──────────────────────────────────────────────────────

    fn conserved(&self, p: &Prim) -> [f64; N_CONSERVED] {
        let mv = p.a * p.rv;
        let ml = (1.0 - p.a) * p.rl;
        let ev = sg_energy(&self.eos.vapour, p.rv, p.pv) + 0.5 * p.uv * p.uv;
        let el = sg_energy(&self.eos.liquid, p.rl, p.pl) + 0.5 * p.ul * p.ul;
        [p.a, mv, mv * p.uv, mv * ev, ml, ml * p.ul, ml * el, mv * p.y]
    }

    fn check_prim(&self, p: &Prim, i: usize) -> Result<(), TampinesError> {
        let ok = p.a.is_finite()
            && p.a >= MIN_VOLUME_FRACTION
            && p.a <= 1.0 - MIN_VOLUME_FRACTION
            && admissible(&self.eos.vapour, p.rv, p.pv)
            && admissible(&self.eos.liquid, p.rl, p.pl)
            && p.uv.is_finite()
            && p.ul.is_finite()
            && p.y.is_finite();
        if ok {
            Ok(())
        } else {
            Err(TampinesError::Unphysical(format!(
                "seven-equation cell {i} left the admissible set at t = {} s: {p:?} -- \
                 refused, not clipped",
                self.time
            )))
        }
    }

    fn prim(&self, i: usize, u: &[f64; N_CONSERVED]) -> Result<Prim, TampinesError> {
        let a = u[0];
        let rv = u[1] / a;
        let rl = u[4] / (1.0 - a);
        let uv = u[2] / u[1];
        let ul = u[5] / u[4];
        let ev = u[3] / u[1] - 0.5 * uv * uv;
        let el = u[6] / u[4] - 0.5 * ul * ul;
        let p = Prim {
            a,
            rv,
            uv,
            pv: sg_pressure(&self.eos.vapour, rv, ev),
            rl,
            ul,
            pl: sg_pressure(&self.eos.liquid, rl, el),
            y: u[7] / u[1],
        };
        self.check_prim(&p, i)?;
        Ok(p)
    }

    fn impedances(&self, p: &Prim) -> (f64, f64) {
        let cv = sg_sound_speed(&self.eos.vapour, p.rv, p.pv);
        let cl = sg_sound_speed(&self.eos.liquid, p.rl, p.pl);
        (p.rv * cv, p.rl * cl)
    }

    /// `(p_int, u_int)` at a state, with `s = sgn(∂α_l/∂x)` for the sign
    /// terms (Eqs 191–196).
    fn interface(&self, p: &Prim, s: f64) -> (f64, f64) {
        match self.interfacial {
            InterfacialVariables::ImpedanceWeighted => {
                let (z_v, z_l) = self.impedances(p);
                let z = z_v + z_l;
                let s = if self.sign_terms { s } else { 0.0 };
                (
                    (z_l * p.pv + z_v * p.pl) / z + s * z_l * z_v * (p.uv - p.ul) / z,
                    (z_l * p.ul + z_v * p.uv) / z + s * (p.pv - p.pl) / z,
                )
            }
            InterfacialVariables::BaerNunziato => (p.pv, p.ul),
            InterfacialVariables::SaurelAbgrall => {
                let (mv, ml) = (p.a * p.rv, (1.0 - p.a) * p.rl);
                (
                    p.a * p.pv + (1.0 - p.a) * p.pl,
                    (mv * p.uv + ml * p.ul) / (mv + ml),
                )
            }
        }
    }

    /// The weight `w` in `p_int = w p_v + (1 − w) p_l` (sign terms dropped),
    /// for the finite pressure relaxation.
    fn interface_pressure_weight(&self, p: &Prim) -> f64 {
        match self.interfacial {
            InterfacialVariables::ImpedanceWeighted => {
                let (z_v, z_l) = self.impedances(p);
                z_l / (z_v + z_l)
            }
            InterfacialVariables::BaerNunziato => 1.0,
            InterfacialVariables::SaurelAbgrall => p.a,
        }
    }

    /// The weight `β` in `u_int = β u_v + (1 − β) u_l` (sign terms dropped),
    /// for the velocity-relaxation work.
    fn interface_velocity_weight(&self, p: &Prim) -> f64 {
        match self.interfacial {
            InterfacialVariables::ImpedanceWeighted => {
                let (z_v, z_l) = self.impedances(p);
                z_v / (z_v + z_l)
            }
            InterfacialVariables::BaerNunziato => 0.0,
            InterfacialVariables::SaurelAbgrall => {
                let (mv, ml) = (p.a * p.rv, (1.0 - p.a) * p.rl);
                mv / (mv + ml)
            }
        }
    }

    fn interfacial_area(&self, void: f64) -> Result<f64, TampinesError> {
        match &self.area_source {
            InterfacialAreaSource::Constant(a) => Ok(*a),
            InterfacialAreaSource::Exchange(x) => x.interfacial_area_concentration(void),
        }
    }

    fn ghost(&self, bc: SevenEquationBoundary, inner: Prim) -> Prim {
        match bc {
            SevenEquationBoundary::Wall => Prim {
                uv: -inner.uv,
                ul: -inner.ul,
                ..inner
            },
            SevenEquationBoundary::Transmissive => inner,
            SevenEquationBoundary::VelocityInlet(s) => Prim {
                pv: inner.pv,
                pl: inner.pl,
                ..Prim::from_state(&s)
            },
            SevenEquationBoundary::PressureOutlet { pressure } => Prim {
                pv: pressure,
                pl: pressure,
                ..inner
            },
        }
    }

    /// The semi-discrete right-hand side `dU/dt` per unit volume.
    fn residual(
        &self,
        u: &[[f64; N_CONSERVED]],
    ) -> Result<Vec<[f64; N_CONSERVED]>, TampinesError> {
        let n = u.len();
        let dx = self.pipe.dx();
        let g_x = self.pipe.axial_gravity();
        let prims: Vec<Prim> = (0..n).map(|i| self.prim(i, &u[i])).collect::<Result<_, _>>()?;
        let gl = self.ghost(self.left, prims[0]);
        let gr = self.ghost(self.right, prims[n - 1]);
        // Extended cell list: ghost, cells, ghost.
        let ext = |k: isize| -> Prim {
            if k < 0 {
                gl
            } else if k as usize >= n {
                gr
            } else {
                prims[k as usize]
            }
        };
        // Face states (MUSCL-minmod on primitives; ghosts first order).
        let slope = |k: isize| -> [f64; 8] {
            if !self.muscl || k < 0 || k as usize >= n {
                return [0.0; 8];
            }
            let (a, b, c) = (ext(k - 1).to_array(), ext(k).to_array(), ext(k + 1).to_array());
            let mut s = [0.0; 8];
            for m in 0..8 {
                s[m] = minmod(b[m] - a[m], c[m] - b[m]);
            }
            s
        };
        let slopes: Vec<[f64; 8]> = (-1..=n as isize).map(slope).collect();
        let face_states = |j: usize| -> (Prim, Prim) {
            // Face j sits between extended cells j-1 and j.
            let kl = j as isize - 1;
            let kr = j as isize;
            let (al, ar) = (ext(kl).to_array(), ext(kr).to_array());
            let (sl, sr) = (slopes[j], slopes[j + 1]);
            let mut l = [0.0; 8];
            let mut r = [0.0; 8];
            for m in 0..8 {
                l[m] = al[m] + 0.5 * sl[m];
                r[m] = ar[m] - 0.5 * sr[m];
            }
            let (l, r) = (Prim::from_array(l), Prim::from_array(r));
            // A wall's ghost face state mirrors the reconstructed interior
            // face state, so the wall flux is exactly zero.
            let mirror = |p: Prim| Prim { uv: -p.uv, ul: -p.ul, ..p };
            match (j, self.left, self.right) {
                (0, SevenEquationBoundary::Wall, _) => (mirror(r), r),
                (jj, _, SevenEquationBoundary::Wall) if jj == n => (l, mirror(l)),
                _ => (l, r),
            }
        };

        let mut f_cons = vec![[0.0; N_CONSERVED]; n + 1];
        let mut f_alpha = vec![0.0; n + 1];
        let mut u_int_face = vec![0.0; n + 1];
        let mut alpha_bar = vec![0.0; n + 1];
        for j in 0..=n {
            let (l, r) = face_states(j);
            for (name, p) in [("left", &l), ("right", &r)] {
                if !(p.a > 0.0 && p.a < 1.0)
                    || !admissible(&self.eos.vapour, p.rv, p.pv)
                    || !admissible(&self.eos.liquid, p.rl, p.pl)
                {
                    return Err(TampinesError::Unphysical(format!(
                        "seven-equation face {j}: inadmissible reconstructed {name} state {p:?}"
                    )));
                }
            }
            let (cvl, cll) = (
                sg_sound_speed(&self.eos.vapour, l.rv, l.pv),
                sg_sound_speed(&self.eos.liquid, l.rl, l.pl),
            );
            let (cvr, clr) = (
                sg_sound_speed(&self.eos.vapour, r.rv, r.pv),
                sg_sound_speed(&self.eos.liquid, r.rl, r.pl),
            );
            let s = (l.uv.abs() + cvl)
                .max(l.ul.abs() + cll)
                .max(r.uv.abs() + cvr)
                .max(r.ul.abs() + clr);
            let sgn = sign((1.0 - r.a) - (1.0 - l.a));
            let (_, uil) = self.interface(&l, sgn);
            let (_, uir) = self.interface(&r, sgn);
            u_int_face[j] = 0.5 * (uil + uir);
            match self.flux {
                FluxScheme::Hllc => {
                    // α upwinded on the face interfacial velocity, and the SAME
                    // value multiplies both phases' Euler fluxes and enters the
                    // non-conservative terms (Abgrall's criterion).
                    let a_up = if u_int_face[j] >= 0.0 { l.a } else { r.a };
                    alpha_bar[j] = a_up;
                    f_alpha[j] = a_up * u_int_face[j];
                    let fv = hllc(&self.eos.vapour, l.rv, l.uv, l.pv, l.y, r.rv, r.uv, r.pv, r.y);
                    let fl = hllc(&self.eos.liquid, l.rl, l.ul, l.pl, 0.0, r.rl, r.ul, r.pl, 0.0);
                    // One face α for both phases and the non-conservative
                    // terms, so the flux's `p α_face` and `p_int Δα_face`
                    // cancel exactly. A per-phase donor α (tried 2026-11-01)
                    // breaks that cancellation where a phase's own flux runs
                    // against u_int, and failed Edwards the same way.
                    let a_v = a_up;
                    let a_l = 1.0 - a_up;
                    f_cons[j] = [
                        0.0,
                        a_v * fv[0],
                        a_v * fv[1],
                        a_v * fv[2],
                        a_l * fl[0],
                        a_l * fl[1],
                        a_l * fl[2],
                        a_v * fv[3],
                    ];
                }
                FluxScheme::Rusanov => {
                    alpha_bar[j] = 0.5 * (l.a + r.a);
                    f_alpha[j] = 0.5 * (l.a * uil + r.a * uir) - 0.5 * s * (r.a - l.a);
                    let wl = self.conserved(&l);
                    let wr = self.conserved(&r);
                    let fl = self.physical_flux(&l, &wl);
                    let fr = self.physical_flux(&r, &wr);
                    for k in 1..N_CONSERVED {
                        f_cons[j][k] = 0.5 * (fl[k] + fr[k]) - 0.5 * s * (wr[k] - wl[k]);
                    }
                }
            }
        }

        let area: Vec<f64> = (0..=n)
            .map(|j| {
                let a = self.pipe.face_area_si(j);
                if j == n {
                    a * self.right_face_area_fraction
                } else {
                    a
                }
            })
            .collect();
        let mut out = vec![[0.0; N_CONSERVED]; n];
        for i in 0..n {
            let p = &prims[i];
            let (al, ar) = (area[i], area[i + 1]);
            let a_c = self.pipe.cell_area_si(i);
            let d_alpha_v = alpha_bar[i + 1] - alpha_bar[i];
            let sgn = sign(-d_alpha_v);
            let (p_int, u_int) = self.interface(p, sgn);
            let r = &mut out[i];
            r[0] = -(f_alpha[i + 1] - f_alpha[i] - p.a * (u_int_face[i + 1] - u_int_face[i])) / dx;
            for k in 1..N_CONSERVED {
                r[k] = -(ar * f_cons[i + 1][k] - al * f_cons[i][k]) / (a_c * dx);
            }
            let (mv, ml) = (u[i][1], u[i][4]);
            // Momentum: p_int Δ(ᾱA) + (p_k − p_int) α_k ΔA, per unit volume.
            let dav_a = alpha_bar[i + 1] * ar - alpha_bar[i] * al;
            let dal_a = (1.0 - alpha_bar[i + 1]) * ar - (1.0 - alpha_bar[i]) * al;
            r[2] += (p_int * dav_a + (p.pv - p_int) * p.a * (ar - al)) / (a_c * dx) + mv * g_x;
            r[5] += (p_int * dal_a + (p.pl - p_int) * (1.0 - p.a) * (ar - al)) / (a_c * dx)
                + ml * g_x;
            // Energy: p_int u_int ∂α_k/∂x.
            r[3] += p_int * u_int * d_alpha_v / dx + mv * p.uv * g_x;
            r[6] += -p_int * u_int * d_alpha_v / dx + ml * p.ul * g_x;
        }
        Ok(out)
    }

    /// Physical flux per phase and of the species, `F = α (ρu, ρu² + p,
    /// u(ρE + p))`, `c u_v`; entry 0 unused.
    fn physical_flux(&self, p: &Prim, w: &[f64; N_CONSERVED]) -> [f64; N_CONSERVED] {
        let al = 1.0 - p.a;
        [
            0.0,
            w[2],
            w[2] * p.uv + p.a * p.pv,
            p.uv * (w[3] + p.a * p.pv),
            w[5],
            w[5] * p.ul + al * p.pl,
            p.ul * (w[6] + al * p.pl),
            w[7] * p.uv,
        ]
    }

    /// The relaxation operators on one cell after a hyperbolic stage.
    ///
    /// Instantaneous operators are idempotent, so they run after **both**
    /// SSP-RK2 stages: the intermediate state then never carries a trace
    /// phase at a pressure the stiff limit has no use for (a trace of gas at
    /// 1e9 Pa beside gas at 1e5 Pa can go to negative pressure within one
    /// unrelaxed stage). Finite-rate operators run once, after the final
    /// stage, over the whole step. Order: pressure (if instantaneous), velocity,
    /// pressure again (its work heats the phases unequally), mass transfer.
    /// Returns whether the mass-transfer operator acted.
    fn relax_cell(
        &self,
        u: &mut [f64; N_CONSERVED],
        dt: f64,
        i: usize,
        final_stage: bool,
    ) -> Result<MassTransferOutcome, TampinesError> {
        if !(u[0] >= MIN_VOLUME_FRACTION && u[0] <= 1.0 - MIN_VOLUME_FRACTION && u[1] > 0.0 && u[4] > 0.0)
            || u.iter().any(|v| !v.is_finite())
        {
            return Err(TampinesError::Unphysical(format!(
                "seven-equation cell {i}: volume fraction or phase mass left the admissible set \
                 at t = {} s: {u:?} -- refused, not clipped",
                self.time
            )));
        }
        let p_inst = self.pressure_relaxation == Relaxation::Instantaneous;
        if p_inst {
            self.relax_pressure_instantaneous(u, i)?;
        }
        self.prim(i, u)?;
        if self.velocity_relaxation == Relaxation::Instantaneous
            || (final_stage && self.velocity_relaxation == Relaxation::Finite)
        {
            self.relax_velocity(u, dt, i)?;
        }
        if p_inst {
            self.relax_pressure_instantaneous(u, i)?;
        } else if final_stage && self.pressure_relaxation == Relaxation::Finite {
            self.relax_pressure(u, dt, i)?;
        }
        if final_stage && self.mass_transfer == MassTransfer::Instantaneous {
            return self.relax_mass_transfer(u, i);
        }
        Ok(MassTransferOutcome::NotNeeded)
    }

    /// Instantaneous pressure relaxation on a state whose phase pressures may
    /// be inadmissible (negative for a trace phase after an unrelaxed stage):
    /// only `α`, the masses and the internal energies are used.
    fn relax_pressure_instantaneous(
        &self,
        u: &mut [f64; N_CONSERVED],
        i: usize,
    ) -> Result<(), TampinesError> {
        let (gv, gl) = (self.eos.vapour, self.eos.liquid);
        let a = u[0];
        let (mv, ml) = (u[1], u[4]);
        let (uv, ul) = (u[2] / mv, u[5] / ml);
        let ev0 = u[3] / mv - 0.5 * uv * uv;
        let el0 = u[6] / ml - 0.5 * ul * ul;
        let (rv, rl) = (mv / a, ml / (1.0 - a));
        let pv = sg_pressure(&gv, rv, ev0);
        let pl = sg_pressure(&gl, rl, el0);
        let ps = relaxed_pressure(&gv, &gl, a, pv, pl);
        let vv = (1.0 / rv) * (pv + gv.gamma * gv.p_inf_pa + (gv.gamma - 1.0) * ps)
            / (gv.gamma * (ps + gv.p_inf_pa));
        let a1 = mv * vv;
        if !(ps.is_finite() && a1 > MIN_VOLUME_FRACTION && a1 < 1.0 - MIN_VOLUME_FRACTION) {
            return Err(TampinesError::Unphysical(format!(
                "seven-equation cell {i}: instantaneous pressure relaxation has no admissible \
                 root (p* = {ps} Pa, alpha* = {a1}) at t = {} s",
                self.time
            )));
        }
        let ev = ev0 - ps * (a1 - a) / mv;
        let el = el0 + ps * (a1 - a) / ml;
        u[0] = a1;
        u[3] = mv * (ev + 0.5 * uv * uv);
        u[6] = ml * (el + 0.5 * ul * ul);
        Ok(())
    }

    /// Velocity relaxation (Eq 198), exact; see the module docs.
    fn relax_velocity(&self, u: &mut [f64; N_CONSERVED], dt: f64, i: usize) -> Result<(), TampinesError> {
        if self.velocity_relaxation == Relaxation::Off {
            return Ok(());
        }
        let p = self.prim(i, u)?;
        let (mv, ml) = (u[1], u[4]);
        let m = mv + ml;
        let u_eq = (u[2] + u[5]) / m;
        let w0 = p.uv - p.ul;
        let w1 = match self.velocity_relaxation {
            Relaxation::Instantaneous => 0.0,
            _ => {
                let (_, lambda) = self.relaxation_coefficients(&p.to_state())?;
                w0 * (-lambda * (1.0 / mv + 1.0 / ml) * dt).r_exp()
            }
        };
        // u_int = u_eq + γ' w, γ' = β m_l/M − (1 − β) m_v/M.
        let beta = self.interface_velocity_weight(&p);
        let gp = beta * ml / m - (1.0 - beta) * mv / m;
        let r = mv * ml / m;
        // ∫ u_int d(m_v u_v) = r [u_eq Δw + γ' ½ Δ(w²)].
        let work_v = r * (u_eq * (w1 - w0) + gp * 0.5 * (w1 * w1 - w0 * w0));
        let uv1 = u_eq + ml / m * w1;
        let ul1 = u_eq - mv / m * w1;
        u[2] = mv * uv1;
        u[5] = ml * ul1;
        u[3] += work_v;
        u[6] -= work_v;
        Ok(())
    }

    /// Pressure relaxation (Eq 197).
    fn relax_pressure(&self, u: &mut [f64; N_CONSERVED], dt: f64, i: usize) -> Result<(), TampinesError> {
        if self.pressure_relaxation == Relaxation::Off {
            return Ok(());
        }
        let p = self.prim(i, u)?;
        let (mv, ml) = (u[1], u[4]);
        let (ev0, el0) = (
            sg_energy(&self.eos.vapour, p.rv, p.pv),
            sg_energy(&self.eos.liquid, p.rl, p.pl),
        );
        let (gv, gl) = (self.eos.vapour, self.eos.liquid);
        let a_new = match self.pressure_relaxation {
            Relaxation::Instantaneous => {
                let ps = relaxed_pressure(&gv, &gl, p.a, p.pv, p.pl);
                // v_k(p*) from m_k de_k = −p* m_k dv_k.
                let vv = (1.0 / p.rv) * (p.pv + gv.gamma * gv.p_inf_pa + (gv.gamma - 1.0) * ps)
                    / (gv.gamma * (ps + gv.p_inf_pa));
                let a = mv * vv;
                let ev = ev0 - ps * (a - p.a) / mv;
                let el = el0 + ps * (a - p.a) / ml;
                Some((a, ev, el))
            }
            Relaxation::Finite => {
                let (mu, _) = self.relaxation_coefficients(&p.to_state())?;
                if !(mu > 0.0) {
                    None
                } else {
                    // p_int = w p_v' + (1 − w) p_l' of the END pressures, with
                    // the weight frozen at the start: a convex combination of
                    // the end state, which makes each phase's entropy change
                    // non-negative over the step (module docs).
                    let w = self.interface_pressure_weight(&p);
                    let end = |a: f64| -> (f64, f64) {
                        end_pressures(&gv, &gl, mv, ml, ev0, el0, p.a, a, w)
                    };
                    let g = |a: f64| -> f64 {
                        let (pv, pl) = end(a);
                        a - p.a - dt * mu * (pv - pl)
                    };
                    // g(α0) = −Δt μ (p_v − p_l). The root lies between α0 and
                    // (about) the instantaneous equilibrium α*, which keeps the
                    // search away from the extreme volume fractions where the
                    // 2×2 end-pressure system degenerates.
                    if p.pv == p.pl {
                        return Ok(());
                    }
                    let ps = relaxed_pressure(&gv, &gl, p.a, p.pv, p.pl);
                    let a_star = mv
                        * (1.0 / p.rv)
                        * (p.pv + gv.gamma * gv.p_inf_pa + (gv.gamma - 1.0) * ps)
                        / (gv.gamma * (ps + gv.p_inf_pa));
                    // Pressures already equal to round-off: nothing to relax.
                    if (a_star - p.a).abs() <= 4.0 * f64::EPSILON {
                        return Ok(());
                    }
                    let g0 = g(p.a);
                    let mut bracket = None;
                    for f in [1.0, 1.25, 1.5, 2.0, 3.0] {
                        let at = (p.a + f * (a_star - p.a))
                            .clamp(MIN_VOLUME_FRACTION, 1.0 - MIN_VOLUME_FRACTION);
                        let gt = g(at);
                        if gt.is_finite() && gt * g0 <= 0.0 {
                            bracket = Some(if at < p.a { (at, p.a) } else { (p.a, at) });
                            break;
                        }
                    }
                    let Some((lo, hi)) = bracket else {
                        return Err(TampinesError::Numerical(format!(
                            "seven-equation cell {i}: finite pressure relaxation could not bracket \
                             its root between alpha0 = {} and alpha* = {a_star} at t = {} s",
                            p.a, self.time
                        )));
                    };
                    let a = brent(g, lo, hi, 1.0e-15, 200).map_err(|e| {
                        TampinesError::Numerical(format!(
                            "seven-equation cell {i}: finite pressure relaxation: {e}"
                        ))
                    })?;
                    let (pv, pl) = end(a);
                    let p_int = w * pv + (1.0 - w) * pl;
                    let ev = ev0 - p_int * (a - p.a) / mv;
                    let el = el0 + p_int * (a - p.a) / ml;
                    Some((a, ev, el))
                }
            }
            Relaxation::Off => None,
        };
        if let Some((a, ev, el)) = a_new {
            u[0] = a;
            u[3] = mv * (ev + 0.5 * p.uv * p.uv);
            u[6] = ml * (el + 0.5 * p.ul * p.ul);
            self.prim(i, u)?;
        }
        Ok(())
    }

    /// Instantaneous thermo-chemical relaxation.
    fn relax_mass_transfer(
        &self,
        u: &mut [f64; N_CONSERVED],
        i: usize,
    ) -> Result<MassTransferOutcome, TampinesError> {
        let p = self.prim(i, u)?;
        let (gv, gl) = (self.eos.vapour, self.eos.liquid);
        let pm = p.a * p.pv + (1.0 - p.a) * p.pl;
        let t_sat = self.eos.saturation_temperature(Pressure::new::<pascal>(pm))?.get::<kelvin>();
        let tv = sg_temperature(&gv, p.rv, p.pv);
        let tl = sg_temperature(&gl, p.rl, p.pl);
        let two_phase = p.a > TRACE_VOLUME_FRACTION && 1.0 - p.a > TRACE_VOLUME_FRACTION;
        let metastable = tl > t_sat || tv < t_sat;
        if !(two_phase || metastable) {
            return Ok(MassTransferOutcome::NotNeeded);
        }
        let (mv, ml) = (u[1], u[4]);
        let rho = mv + ml;
        let mom = u[2] + u[5];
        let ke = 0.5 * mom * mom / rho;
        let e_mix = (u[3] + u[6] - ke) / rho;
        let eos = self.eos;
        let residual = |lnp: f64| -> f64 {
            let pp = lnp.r_exp();
            let t = match eos.saturation_temperature(Pressure::new::<pascal>(pp)) {
                Ok(t) => t.get::<kelvin>(),
                Err(_) => return f64::NAN,
            };
            let vv = 1.0 / sg_density_pt(&gv, pp, t);
            let vl = 1.0 / sg_density_pt(&gl, pp, t);
            let y = (1.0 / rho - vl) / (vv - vl);
            y * sg_energy_pt(&gv, pp, t) + (1.0 - y) * sg_energy_pt(&gl, pp, t) - e_mix
        };
        // Bracket outward from the current pressure in ln p.
        let x0 = pm.max(1.0).r_ln();
        let f0 = residual(x0);
        if !f0.is_finite() {
            return Ok(MassTransferOutcome::Skipped);
        }
        let mut bracket = None;
        let mut h = 0.02;
        for _ in 0..40 {
            for x in [x0 - h, x0 + h] {
                let f = residual(x);
                if f.is_finite() && f * f0 <= 0.0 {
                    bracket = Some(if x < x0 { (x, x0) } else { (x0, x) });
                    break;
                }
            }
            if bracket.is_some() {
                break;
            }
            h *= 1.6;
            if h > 12.0 {
                break;
            }
        }
        let Some((lo, hi)) = bracket else {
            return Ok(MassTransferOutcome::Skipped);
        };
        let x = brent(residual, lo, hi, 1.0e-14, 200).map_err(|e| {
            TampinesError::Numerical(format!("seven-equation cell {i}: mass transfer root: {e}"))
        })?;
        let pp = x.r_exp();
        let t = eos.saturation_temperature(Pressure::new::<pascal>(pp))?.get::<kelvin>();
        let rv = sg_density_pt(&gv, pp, t);
        let rl = sg_density_pt(&gl, pp, t);
        let y = (1.0 / rho - 1.0 / rl) / (1.0 / rv - 1.0 / rl);
        if !(y > 0.0 && y < 1.0) {
            return Ok(MassTransferOutcome::Skipped);
        }
        let a = y * rho / rv;
        if !(a > MIN_VOLUME_FRACTION && 1.0 - a > MIN_VOLUME_FRACTION) {
            return Ok(MassTransferOutcome::Skipped);
        }
        let uu = mom / rho;
        let (mv1, ml1) = (y * rho, (1.0 - y) * rho);
        let c = u[7];
        *u = [
            a,
            mv1,
            mv1 * uu,
            mv1 * (sg_energy_pt(&gv, pp, t) + 0.5 * uu * uu),
            ml1,
            ml1 * uu,
            ml1 * (sg_energy_pt(&gl, pp, t) + 0.5 * uu * uu),
            c,
        ];
        self.prim(i, u)?;
        Ok(MassTransferOutcome::Acted)
    }
}

/// The relaxed pressure `p*` of the instantaneous stiffened-gas pressure
/// relaxation with `p_int = p*`: the root with `p* + p∞,k > 0` of
/// `Σ_k α_k (p_k + γ_k p∞,k + (γ_k − 1) p*)/(γ_k (p* + p∞,k)) = 1`, a
/// quadratic (Saurel, Petitpas & Berry 2009, §3.2).
fn relaxed_pressure(gv: &StiffenedGasHem, gl: &StiffenedGasHem, a_v: f64, pv: f64, pl: f64) -> f64 {
    let a_l = 1.0 - a_v;
    let (gav, gal) = (gv.gamma, gl.gamma);
    let (piv, pil) = (gv.p_inf_pa, gl.p_inf_pa);
    let (av, al) = (pv + gav * piv, pl + gal * pil);
    let a = a_v * (gav - 1.0) / gav + a_l * (gal - 1.0) / gal - 1.0;
    let b = a_v / gav * (av + (gav - 1.0) * pil) + a_l / gal * (al + (gal - 1.0) * piv) - (piv + pil);
    let c = a_v * av * pil / gav + a_l * al * piv / gal - piv * pil;
    let disc = (b * b - 4.0 * a * c).max(0.0).sqrt();
    // a < 0: the larger root, written without cancellation.
    let q = -0.5 * (b + b.signum() * disc);
    let r1 = q / a;
    let r2 = if q != 0.0 { c / q } else { r1 };
    let mut p = r1.max(r2);
    // Two Newton polishes on the volume constraint itself, which the
    // quadratic's coefficients (of order p∞²) reach only to ~1e-13 relative.
    for _ in 0..2 {
        let f = a_v * (av + (gav - 1.0) * p) / (gav * (p + piv))
            + a_l * (al + (gal - 1.0) * p) / (gal * (p + pil))
            - 1.0;
        let df = a_v * ((gav - 1.0) * (p + piv) - (av + (gav - 1.0) * p)) / (gav * (p + piv) * (p + piv))
            + a_l * ((gal - 1.0) * (p + pil) - (al + (gal - 1.0) * p))
                / (gal * (p + pil) * (p + pil));
        if df != 0.0 && (f / df).is_finite() {
            p -= f / df;
        }
    }
    p
}

/// End-of-step phase pressures `(p_v', p_l')` of a pressure relaxation from
/// `α0` to `α1` with `p_int = w p_v' + (1 − w) p_l'`: the stiffened-gas law is
/// linear in `e`, so `p_k' = (γ_k − 1) ρ_k' (e_k0 ∓ p_int Δα/m_k − q_k) − γ_k p∞,k`
/// is a 2×2 linear system, solved in closed form.
#[allow(clippy::too_many_arguments)]
fn end_pressures(
    gv: &StiffenedGasHem,
    gl: &StiffenedGasHem,
    mv: f64,
    ml: f64,
    ev0: f64,
    el0: f64,
    a0: f64,
    a1: f64,
    w: f64,
) -> (f64, f64) {
    let da = a1 - a0;
    let kv = (gv.gamma - 1.0) * (mv / a1);
    let kl = (gl.gamma - 1.0) * (ml / (1.0 - a1));
    // p_v = kv (ev0 − q_v) − γp∞_v − kv (da/mv) (w p_v + (1−w) p_l)
    // p_l = kl (el0 − q_l) − γp∞_l + kl (da/ml) (w p_v + (1−w) p_l)
    let bv = kv * (ev0 - gv.q_j_per_kg) - gv.gamma * gv.p_inf_pa;
    let bl = kl * (el0 - gl.q_j_per_kg) - gl.gamma * gl.p_inf_pa;
    let (cv, cl) = (kv * da / mv, kl * da / ml);
    // [1 + cv w, cv (1−w); −cl w, 1 − cl (1−w)] [p_v; p_l] = [bv; bl]
    let (a11, a12, a21, a22) = (1.0 + cv * w, cv * (1.0 - w), -cl * w, 1.0 - cl * (1.0 - w));
    let det = a11 * a22 - a12 * a21;
    ((bv * a22 - a12 * bl) / det, (a11 * bl - a21 * bv) / det)
}

/// HLLC flux of one stiffened-gas phase's Euler system plus a passive scalar
/// `Y` (Toro 2009 §10.4, Davis wave-speed estimates): `[ρu, ρu² + p,
/// u(ρE + p), ρuY]`. Per unit phase volume; the caller multiplies by `α`.
#[allow(clippy::too_many_arguments)]
fn hllc(
    g: &StiffenedGasHem,
    rl: f64,
    ul: f64,
    pl: f64,
    yl: f64,
    rr: f64,
    ur: f64,
    pr: f64,
    yr: f64,
) -> [f64; 4] {
    let (cl, cr) = (sg_sound_speed(g, rl, pl), sg_sound_speed(g, rr, pr));
    let el = rl * (sg_energy(g, rl, pl) + 0.5 * ul * ul);
    let er = rr * (sg_energy(g, rr, pr) + 0.5 * ur * ur);
    let flux = |r: f64, u: f64, p: f64, e: f64, y: f64| [r * u, r * u * u + p, u * (e + p), r * u * y];
    let fl = flux(rl, ul, pl, el, yl);
    let fr = flux(rr, ur, pr, er, yr);
    let sl = (ul - cl).min(ur - cr);
    let sr = (ul + cl).max(ur + cr);
    if sl >= 0.0 {
        return fl;
    }
    if sr <= 0.0 {
        return fr;
    }
    let ss = (pr - pl + rl * ul * (sl - ul) - rr * ur * (sr - ur)) / (rl * (sl - ul) - rr * (sr - ur));
    // Davis estimates can put the contact outside [S_L, S_R] in a strong
    // rarefaction (a trace phase meeting a low-pressure outlet), where the
    // HLLC star densities go negative. There the face falls back to the
    // positivity-preserving Rusanov flux of this phase.
    if !(ss > sl && ss < sr && ss.is_finite()) {
        let smax = (ul.abs() + cl).max(ur.abs() + cr);
        let wl = [rl, rl * ul, el, rl * yl];
        let wr = [rr, rr * ur, er, rr * yr];
        let mut o = [0.0; 4];
        for k in 0..4 {
            o[k] = 0.5 * (fl[k] + fr[k]) - 0.5 * smax * (wr[k] - wl[k]);
        }
        return o;
    }
    let star = |r: f64, u: f64, p: f64, e: f64, y: f64, sk: f64| -> [f64; 4] {
        let f = r * (sk - u) / (sk - ss);
        [f, f * ss, f * (e / r + (ss - u) * (ss + p / (r * (sk - u)))), f * y]
    };
    if ss >= 0.0 {
        let us = star(rl, ul, pl, el, yl, sl);
        let w = [rl, rl * ul, el, rl * yl];
        let mut o = [0.0; 4];
        for k in 0..4 {
            o[k] = fl[k] + sl * (us[k] - w[k]);
        }
        o
    } else {
        let us = star(rr, ur, pr, er, yr, sr);
        let w = [rr, rr * ur, er, rr * yr];
        let mut o = [0.0; 4];
        for k in 0..4 {
            o[k] = fr[k] + sr * (us[k] - w[k]);
        }
        o
    }
}

fn brent<F: Fn(f64) -> f64>(f: F, lo: f64, hi: f64, epsrel: f64, max_iter: usize) -> Result<f64, String> {
    let mut solver =
        BracketingSolver::new(BracketingMethod::Brent, f, lo, hi).map_err(|e| format!("{e:?}"))?;
    solver.solve(0.0, epsrel, max_iter).map_err(|e| format!("{e:?}"))
}

fn add_scaled(u: &[f64; N_CONSERVED], r: &[f64; N_CONSERVED], dt: f64) -> [f64; N_CONSERVED] {
    let mut o = [0.0; N_CONSERVED];
    for k in 0..N_CONSERVED {
        o[k] = u[k] + dt * r[k];
    }
    o
}

fn minmod(a: f64, b: f64) -> f64 {
    if a * b <= 0.0 {
        0.0
    } else if a.abs() < b.abs() {
        a
    } else {
        b
    }
}

fn sign(x: f64) -> f64 {
    if x > 0.0 {
        1.0
    } else if x < 0.0 {
        -1.0
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests;
