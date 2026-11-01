// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.
//! Steady-state single-phase subchannel solver with diversion crossflow and
//! turbulent mixing (gh:#1078, RELAP-7 parity epic gh:#1056).
//!
//! **Research, education and V&V only. AI draft, unreviewed.**
//!
//! # Provenance (clean room)
//!
//! Written from the open, textbook COBRA-type subchannel formulation:
//!
//! - N. E. Todreas & M. S. Kazimi, *Nuclear Systems*, Vol. I (2nd ed., CRC
//!   Press, 2011) and Vol. II (1990), the subchannel control-volume balance
//!   equations and the turbulent-mixing definition `w'_ij = β s_ij Ḡ`.
//!   Section and equation numbers are **not re-checked**: neither book is in
//!   the kovan corpus.
//! - D. S. Rowe, *COBRA-IIIC: A digital computer program for steady state and
//!   transient thermal-hydraulic analysis of rod bundle nuclear fuel
//!   elements*, BNWL-1695, Battelle Pacific Northwest Laboratories (1973):
//!   the lateral-momentum equation with a gap loss coefficient, and the
//!   turbulent-momentum factor `f_T`. Not re-checked (not in the corpus).
//!
//! RELAP-7's subchannel component (RELAP-7 Theory Manual, INL/EXT-14-31366,
//! §6.15, Eqs 408–411) is the parity target named by gh:#1078; that manual is
//! private and was **not read**. Nothing here is taken from it or from any
//! RELAP source.
//!
//! # What was searched before writing this (workspace rule)
//!
//! `grep -ri "subchannel\|crossflow\|cross-flow\|turbulent mixing"` over
//! `crates/` (2026-11-01): `bedok`'s channels are closed (no crossflow,
//! `singleflow1devap.rs`); the `outram-foam-appbuilder-lib` hits are GeN-Foam
//! drag closures, not a solver; the `dwsim` and digital-twin hits are
//! shell-side crossflow in heat exchangers. No subchannel solver existed.
//! Reused: the wall-friction closure [`crate::closures::wall_friction`]
//! (RELAP5 four-region Darcy factor), PETIR's sparse LU
//! ([`petir::sparse::SparseLu`]), and GSL's forward-difference step rule
//! ([`petir::multiroots::GSL_SQRT_DBL_EPSILON`]). `petir::multiroots::fdjacobian`
//! itself was checked and not used: it fills a dense `n × n` matrix, which is
//! 1 GB at a 25-channel, 100-cell bundle; the Jacobian here is built by
//! column colouring on its known block band instead (same step rule).
//!
//! # Model
//!
//! `N` subchannels of constant flow area `A_i`, each discretised into `N_z`
//! equal axial cells of height `Δz`, connected by gaps `g = (i → j)` of width
//! `s` and centroid distance `l`. Fluid properties (`ρ`, `c_p`, `μ`) are
//! constant: incompressible, no buoyancy. Unknowns: `ṁ_i`, `T_i`, `p_i` at the
//! axial nodes and the diversion crossflow `w_g` (kg m⁻¹ s⁻¹, positive
//! `i → j`) in each cell. Per cell `k` (overbars are cell averages of the two
//! nodes, `u = ṁ/(ρA)`, `e_gi = +1` for the gap's `from` channel and `-1` for
//! its `to` channel):
//!
//! ```text
//! mass      ṁ_i(k+1) − ṁ_i(k) + Δz Σ_g e_gi w_g = 0
//! energy    c_p[ṁT]_k^{k+1} − q'_i Δz + Δz Σ_g e_gi [w_g c_p T* + w'_g c_p (T̄_i − T̄_j)] = 0
//! axial     [ṁ²/(ρA)]_k^{k+1} + Δz Σ_g e_gi [w_g u* + f_T w'_g (ū_i − ū_j)]
//!             + A_i (p_i(k+1) − p_i(k)) + A_i Δz ρ (a_f(ū_i) + g) = 0
//! lateral   [(u_g w_g)_k − (u_g w_g)_{k−1}]/Δz − (s/l)(p̄_i − p̄_j) + K w_g|w_g|/(2ρ s l) = 0
//! ```
//!
//! `T*`, `u*` are the donor (upwind across the gap) cell values; `u_g` is the
//! mean of the two channels' axial velocities; `w_{−1} = 0` (no crossflow
//! upstream of the inlet); `a_f` is
//! [`wall_friction_acceleration`](crate::closures::wall_friction::wall_friction_acceleration).
//! The turbulent mixing rate is `w'_g = β s |Ḡ|`, `Ḡ` the mean of the two
//! channels' mass fluxes. It is **on by default** (the caller supplies `β`);
//! [`SubchannelProblem::without_turbulent_mixing`] ablates it. Mixing moves
//! equal and opposite mass, so it carries energy and momentum but no net
//! mass; its momentum effect carries the factor `f_T` (default 1).
//!
//! Boundary conditions: each channel's inlet mass flow and temperature, and a
//! uniform outlet pressure. The whole system is solved at once by Newton's
//! method (finite-difference Jacobian, sparse LU, backtracking).
//!
//! Every balance is in conservative form and the crossflow and mixing terms
//! are antisymmetric between the two channels of a gap, so the bundle's total
//! mass and energy are conserved to the Newton residual (V&V tests 2 and 4).
//!
//! # What it does not do
//!
//! - No two-phase flow, no density change with temperature (so no buoyancy
//!   and no thermal acceleration), no spacer-grid form losses, no rod
//!   conduction or wall temperature (the heat input is the linear power into
//!   the coolant). Those are named gaps, not silent omissions.
//! - Dense storage is avoided, but the linear solve is a general sparse LU of
//!   the whole bundle; no claim is made about large-bundle run time.
//!
//! V&V: `crates/tampines/verification_and_validation/subchannel_crossflow_mixing.md`.

mod solver;
#[cfg(test)]
mod tests;

use uom::si::acceleration::meter_per_second_squared;
use uom::si::area::square_meter;
use uom::si::dynamic_viscosity::pascal_second;
use uom::si::f64::{
    Acceleration, Area, DynamicViscosity, Length, LinearPowerDensity, MassDensity, MassRate,
    Pressure, SpecificHeatCapacity, ThermodynamicTemperature,
};
use uom::si::length::meter;
use uom::si::linear_power_density::watt_per_meter;
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::mass_rate::kilogram_per_second;
use uom::si::pressure::pascal;
use uom::si::specific_heat_capacity::joule_per_kilogram_kelvin;
use uom::si::thermodynamic_temperature::kelvin;

use crate::TampinesError;

/// A mass flow per unit length (kg m⁻¹ s⁻¹): the diversion crossflow `w` and
/// the turbulent mixing rate `w'` per unit axial length. uom has no quantity
/// of that name; its dimension, M L⁻¹ T⁻¹, is that of dynamic viscosity, so
/// it is stored as one (`Pa·s ≡ kg m⁻¹ s⁻¹`). Read it with
/// `get::<pascal_second>()` as kg m⁻¹ s⁻¹.
pub type LinearMassRate = DynamicViscosity;

/// A representative turbulent mixing coefficient `β` for rod bundles, 0.02.
///
/// The open literature puts `β` for PWR/BWR-type rod-bundle gaps at roughly
/// 0.005–0.05 (Rogers & Rosehart's review, and the subchannel chapter of
/// Todreas & Kazimi, *Nuclear Systems*); 0.02 is a mid-range value. **Not
/// re-checked** against a source in the corpus. It is a starting value for a
/// caller, never a fitted one: `β` is always a caller input to
/// [`SubchannelProblem::new`].
pub const TYPICAL_MIXING_COEFFICIENT_BETA: f64 = 0.02;

/// Default turbulent-momentum factor `f_T`: 1, i.e. the eddy exchange that
/// carries enthalpy carries axial momentum at the same rate (Reynolds
/// analogy). COBRA-IIIC (Rowe 1973) exposes `f_T` as an input;
/// [`SubchannelProblem::with_momentum_mixing_factor`]`(0.0)` ablates the
/// momentum part alone.
pub const DEFAULT_MOMENTUM_MIXING_FACTOR: f64 = 1.0;

/// Standard gravity, the default axial gravity for vertical upward flow.
pub const STANDARD_GRAVITY: f64 = 9.80665;

/// Constant fluid properties (incompressible single phase).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FluidProperties {
    /// Density `ρ`.
    pub density: MassDensity,
    /// Specific heat `c_p`; enthalpy is `c_p T`.
    pub specific_heat: SpecificHeatCapacity,
    /// Dynamic viscosity `μ`, for the Reynolds number of the wall friction.
    pub viscosity: DynamicViscosity,
}

/// The linear power into one subchannel's coolant.
#[derive(Debug, Clone, PartialEq)]
pub enum AxialHeat {
    /// The same `q'` in every axial cell.
    Uniform(LinearPowerDensity),
    /// One `q'` per axial cell, inlet first; the length must equal the
    /// problem's `n_axial`.
    PerCell(Vec<LinearPowerDensity>),
}

/// One subchannel: geometry, wall roughness, inlet state and heating.
#[derive(Debug, Clone, PartialEq)]
pub struct Subchannel {
    /// Flow area `A`.
    pub flow_area: Area,
    /// Hydraulic diameter `D_h = 4A/P_w`.
    pub hydraulic_diameter: Length,
    /// Absolute wall roughness `ε` (the friction closure takes `ε/D_h`).
    pub roughness: Length,
    /// Laminar shape factor `Φ_S` of the friction closure (1 = circular).
    pub shape_factor: f64,
    /// Inlet mass flow (must be positive).
    pub inlet_mass_flow: MassRate,
    /// Inlet temperature.
    pub inlet_temperature: ThermodynamicTemperature,
    /// Linear power into the coolant.
    pub heat: AxialHeat,
}

impl Subchannel {
    /// A smooth (`ε = 0`), circular-shape-factor (`Φ_S = 1`), unheated
    /// subchannel.
    pub fn new(
        flow_area: Area,
        hydraulic_diameter: Length,
        inlet_mass_flow: MassRate,
        inlet_temperature: ThermodynamicTemperature,
    ) -> Self {
        Self {
            flow_area,
            hydraulic_diameter,
            roughness: Length::new::<meter>(0.0),
            shape_factor: 1.0,
            inlet_mass_flow,
            inlet_temperature,
            heat: AxialHeat::Uniform(LinearPowerDensity::new::<watt_per_meter>(0.0)),
        }
    }

    /// Set the linear power into the coolant.
    pub fn with_heat(mut self, heat: AxialHeat) -> Self {
        self.heat = heat;
        self
    }

    /// Set the absolute wall roughness.
    pub fn with_roughness(mut self, roughness: Length) -> Self {
        self.roughness = roughness;
        self
    }

    /// Set the laminar shape factor `Φ_S`.
    pub fn with_shape_factor(mut self, shape_factor: f64) -> Self {
        self.shape_factor = shape_factor;
        self
    }
}

/// A gap between subchannels `from` and `to`; crossflow is positive from
/// `from` to `to`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Gap {
    /// Index of the first channel.
    pub from: usize,
    /// Index of the second channel.
    pub to: usize,
    /// Gap width `s` (rod-to-rod clearance).
    pub width: Length,
    /// Centroid-to-centroid distance `l`, the lateral control-volume length.
    pub centroid_distance: Length,
    /// Gap loss coefficient `K` on the lateral velocity head (dimensionless).
    pub loss_coefficient: f64,
}

impl Gap {
    /// A gap between channels `from` and `to`.
    pub fn new(
        from: usize,
        to: usize,
        width: Length,
        centroid_distance: Length,
        loss_coefficient: f64,
    ) -> Self {
        Self {
            from,
            to,
            width,
            centroid_distance,
            loss_coefficient,
        }
    }
}

/// Turbulent mixing parameters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TurbulentMixing {
    /// Mixing coefficient `β` in `w' = β s Ḡ`.
    pub beta: f64,
    /// Momentum factor `f_T` on the axial-momentum exchange.
    pub momentum_factor: f64,
}

/// A steady single-phase subchannel problem. Build with
/// [`SubchannelProblem::new`], solve with [`SubchannelProblem::solve`].
#[derive(Debug, Clone, PartialEq)]
pub struct SubchannelProblem {
    channels: Vec<Subchannel>,
    gaps: Vec<Gap>,
    fluid: FluidProperties,
    length: Length,
    n_axial: usize,
    outlet_pressure: Pressure,
    gravity_along_flow: Acceleration,
    mixing: Option<TurbulentMixing>,
}

impl SubchannelProblem {
    /// A bundle of `channels` joined by `gaps`, heated length `length` in
    /// `n_axial` cells, at uniform `outlet_pressure`.
    ///
    /// **Turbulent mixing is on**, with the caller's `beta` and
    /// `f_T = `[`DEFAULT_MOMENTUM_MIXING_FACTOR`]; gravity is
    /// [`STANDARD_GRAVITY`] opposing the flow (vertical upward flow).
    pub fn new(
        channels: Vec<Subchannel>,
        gaps: Vec<Gap>,
        fluid: FluidProperties,
        length: Length,
        n_axial: usize,
        outlet_pressure: Pressure,
        beta: f64,
    ) -> Self {
        Self {
            channels,
            gaps,
            fluid,
            length,
            n_axial,
            outlet_pressure,
            gravity_along_flow: Acceleration::new::<meter_per_second_squared>(STANDARD_GRAVITY),
            mixing: Some(TurbulentMixing {
                beta,
                momentum_factor: DEFAULT_MOMENTUM_MIXING_FACTOR,
            }),
        }
    }

    /// Ablation: no turbulent mixing (energy or momentum). An explicit act;
    /// the default carries mixing.
    pub fn without_turbulent_mixing(mut self) -> Self {
        self.mixing = None;
        self
    }

    /// Set the turbulent-momentum factor `f_T` (0 ablates the momentum part
    /// of mixing while keeping its energy part). No effect when mixing is off.
    pub fn with_momentum_mixing_factor(mut self, factor: f64) -> Self {
        if let Some(m) = self.mixing.as_mut() {
            m.momentum_factor = factor;
        }
        self
    }

    /// Set the component of gravity opposing the flow (`g cos θ`, θ from the
    /// upward vertical); 0 for a horizontal bundle.
    pub fn with_gravity_along_flow(mut self, g: Acceleration) -> Self {
        self.gravity_along_flow = g;
        self
    }

    /// The turbulent mixing in force, `None` when ablated.
    pub fn mixing(&self) -> Option<TurbulentMixing> {
        self.mixing
    }

    /// Solve the steady state.
    ///
    /// # Errors
    ///
    /// [`TampinesError::InvalidInput`] for an unusable problem (no channels,
    /// zero cells, a non-positive area, diameter, length, gap or property, a
    /// gap naming a missing or the same channel, a per-cell heat list of the
    /// wrong length, a non-positive inlet flow, a negative `β` or `K`);
    /// [`TampinesError::Numerical`] when Newton does not converge.
    pub fn solve(&self) -> Result<SubchannelSolution, TampinesError> {
        let raw = solver::Raw::from_problem(self)?;
        let (x, iterations, residual) = solver::newton(&raw)?;
        Ok(raw.unpack(&x, iterations, residual))
    }
}

/// The steady state of a [`SubchannelProblem`].
#[derive(Debug, Clone, PartialEq)]
pub struct SubchannelSolution {
    /// Axial node positions, `n_axial + 1` of them, inlet at 0.
    pub z: Vec<Length>,
    /// Mass flow, `[channel][node]`.
    pub mass_flow: Vec<Vec<MassRate>>,
    /// Temperature, `[channel][node]`.
    pub temperature: Vec<Vec<ThermodynamicTemperature>>,
    /// Pressure, `[channel][node]`.
    pub pressure: Vec<Vec<Pressure>>,
    /// Diversion crossflow per unit length, `[gap][cell]`, positive
    /// `from → to`.
    pub crossflow: Vec<Vec<LinearMassRate>>,
    /// Turbulent mixing rate per unit length `w' = β s |Ḡ|`, `[gap][cell]`
    /// (zero when mixing is ablated).
    pub turbulent_mixing: Vec<Vec<LinearMassRate>>,
    /// Newton iterations taken.
    pub newton_iterations: usize,
    /// Infinity norm of the scaled residual at the returned state.
    pub residual_inf_norm: f64,
}

impl SubchannelSolution {
    /// The bundle's total mass flow at axial node `node`.
    pub fn total_mass_flow(&self, node: usize) -> MassRate {
        let s: f64 = self
            .mass_flow
            .iter()
            .map(|m| m[node].get::<kilogram_per_second>())
            .sum();
        MassRate::new::<kilogram_per_second>(s)
    }
}

/// Raw SI value helpers kept in one place so the solver works in `f64`.
fn si_area(a: Area) -> f64 {
    a.get::<square_meter>()
}
fn si_len(l: Length) -> f64 {
    l.get::<meter>()
}
fn si_mdot(m: MassRate) -> f64 {
    m.get::<kilogram_per_second>()
}
fn si_t(t: ThermodynamicTemperature) -> f64 {
    t.get::<kelvin>()
}
fn si_p(p: Pressure) -> f64 {
    p.get::<pascal>()
}
fn si_rho(r: MassDensity) -> f64 {
    r.get::<kilogram_per_cubic_meter>()
}
fn si_cp(c: SpecificHeatCapacity) -> f64 {
    c.get::<joule_per_kilogram_kelvin>()
}
fn si_mu(m: DynamicViscosity) -> f64 {
    m.get::<pascal_second>()
}
fn si_q(q: LinearPowerDensity) -> f64 {
    q.get::<watt_per_meter>()
}
fn si_g(g: Acceleration) -> f64 {
    g.get::<meter_per_second_squared>()
}
