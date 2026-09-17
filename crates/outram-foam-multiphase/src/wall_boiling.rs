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

//! Stage 3 — **Wall-boiling framework** (bead `op-2kk.3`).
//!
//! This module defines the *architecture* for wall-boiling closures — the enum,
//! the compiler-enforced contract, and the input/output boundary types — and
//! ships **one fully worked, tested concrete model**: the RPI (Rensselaer
//! Polytechnic Institute) heat-flux–partitioning model of Kurul & Podowski
//! (1991) for the **nucleate-boiling** regime. The other boiling regimes
//! (critical heat flux, subcooled CHF, dryout, film boiling) are present as
//! **architecture placeholders** that return
//! [`MultiphaseError::NotImplemented`] — honest scaffolding, not faked physics.
//!
//! The design deliberately follows the roadmap directive to settle
//! architecture / traits / interfaces / verification **before** adding advanced
//! closures.
//!
//! ## The RPI heat-flux partitioning (nucleate boiling)
//!
//! On a heated wall in the nucleate-boiling regime the total wall heat flux is
//! split into three additive mechanisms (Kurul & Podowski, 1991):
//!
//! ```text
//! q_wall = q_convective + q_quenching + q_evaporative
//! ```
//!
//! - **`q_convective`** — single-phase forced convection over the fraction `A1`
//!   of the wall not currently influenced by bubbles:
//!   `q_convective = A1 · h_c · (T_wall − T_liquid)`.
//! - **`q_quenching`** — transient conduction into cold liquid that rushes in to
//!   fill the site as a bubble departs, over the bubble-influenced fraction `A2`
//!   (Mikic & Rohsenow, 1969):
//!   `q_quenching = A2 · h_q · (T_wall − T_liquid)` with
//!   `h_q = 2·sqrt(k_l·ρ_l·Cp_l·f·τ / π)`.
//! - **`q_evaporative`** — latent heat carried off by the departing bubbles:
//!   `q_evaporative = (π/6)·d_dep³ · ρ_v · N · f · L`.
//!
//! The partition is closed by three **sub-closures**, each a standard
//! literature correlation (cited on its method below):
//!
//! | Sub-closure | Correlation | Symbol |
//! |---|---|---|
//! | Nucleation-site density | Lemmert & Chawla (1977) | `N` `[1/m²]` |
//! | Bubble departure diameter | Tolubinski & Kostanchuk (1970) | `d_dep` `[m]` |
//! | Bubble departure frequency | Cole (1960) | `f` `[1/s]` |
//!
//! and the bubble-influence area fraction `A2` uses the Del Valle & Kenning
//! (1985) influence factor. See [`RpiPartitioning`] for the equations and their
//! coefficients.
//!
//! ## Upstream provenance (concepts ported C++ → Rust)
//!
//! Correlation *forms and default coefficients* are taken from OpenFOAM's
//! `multiphaseEuler` wall-boiling `fvModel`
//! (`applications/modules/multiphaseEuler/fvModels/wallBoiling/`, GPL-3.0).
//! Exact file citations:
//!
//! | Rust item | OpenFOAM source |
//! |---|---|
//! | [`RpiPartitioning::nucleation_site_density`] | `nucleationSiteModels/LemmertChawla/LemmertChawla.C` (`calculate`) |
//! | [`RpiPartitioning::departure_diameter`] | `departureDiameterModels/TolubinskiKostanchuk/TolubinskiKostanchuk.C` (`calculate`) |
//! | [`RpiPartitioning::departure_frequency`] | `departureFrequencyModels/Cole/Cole.C` (`calculate`) |
//! | [`RpiPartitioning::partition`] (area split, `q_c`/`q_q`/`q_e`) | `fvModels/wallBoiling/wallBoiling.C` (`calcBoiling`) |
//!
//! ## Honest scope — what this is and is **not**
//!
//! This is a **verified framework with one worked model**, not validated
//! boiling CFD. Reviewers and users must not read it as a qualified wall-boiling
//! solver. Specifically:
//!
//! - **Point (0-D) evaluation, not a wall-function field.** [`RpiPartitioning`]
//!   evaluates the partition at a *single wall condition* — a set of scalar
//!   near-wall states supplied by the caller ([`WallBoilingConditions`]). It is
//!   not wired into a mesh boundary field, a turbulence wall function, or a
//!   phase-change mass source; those couplings are later work. OpenFOAM obtains
//!   the convective conductance from a turbulent thermal wall function
//!   (`alphatJayatillekeWallFunction`); here the caller supplies the
//!   single-phase convective HTC `h_c` directly.
//! - **Classic evaporative term.** The evaporative flux uses the classic
//!   Kurul–Podowski `(π/6)·d³·ρ_v·N·f·L` form. OpenFOAM's `calcBoiling`
//!   modernises this with a bubble-influence-area cap (`A2E`); that refinement
//!   is documented but not ported here.
//! - **Fully-wetted wall assumed.** The wall wetted fraction is taken as `1`
//!   (no partitioning-model dryout weighting); dry-patch weighting belongs to
//!   the CHF / dryout regimes below, which are not implemented.
//! - **Constant properties.** Fluid properties are inputs held constant over the
//!   evaluation; no property tables, no temperature dependence within a call.
//! - **Only the nucleate-boiling regime is implemented.** [`ChfModel`],
//!   [`ChfSubCoolModel`], [`DryoutModel`], and [`FilmBoilingModel`] are
//!   architecture placeholders that error with
//!   [`MultiphaseError::NotImplemented`].
//!
//! **Benchmark validation is a later, human-run step.** No experimental or
//! reference-solver comparison has been performed. The tests below are
//! *verification* checks (partition-sum conservation, non-negativity of each
//! component, hand-computed sub-closure values, enum-dispatch reachability,
//! clean `NotImplemented` errors) — **not** *validation*.

use crate::MultiphaseError;
use std::f64::consts::PI;
use uom::si::acceleration::meter_per_second_squared;
use uom::si::available_energy::joule_per_kilogram;
use uom::si::f64::{
    Acceleration, AvailableEnergy, HeatFluxDensity, HeatTransfer, MassDensity,
    SpecificHeatCapacity, ThermalConductivity, ThermodynamicTemperature,
};
use uom::si::heat_flux_density::watt_per_square_meter;
use uom::si::heat_transfer::watt_per_square_meter_kelvin;
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::specific_heat_capacity::joule_per_kilogram_kelvin;
use uom::si::thermal_conductivity::watt_per_meter_kelvin;
use uom::si::thermodynamic_temperature::kelvin;

// ─────────────────────────────────────────────────────────────────────────────
// Boundary types — inputs and outputs of a wall-boiling partition
// ─────────────────────────────────────────────────────────────────────────────

/// Near-wall thermodynamic state at which a wall-boiling model is evaluated.
///
/// A wall-boiling closure needs the wall temperature, the local near-wall
/// liquid temperature, the saturation temperature, the two-phase fluid
/// properties, the single-phase convective heat-transfer coefficient, and
/// gravity. This struct carries them `uom`-typed at the API boundary so the
/// physical dimension of every input is checked by the compiler.
///
/// ## Fields, units, and valid ranges
///
/// - `t_wall` — heated-wall surface temperature `T_w` `[K]`.
/// - `t_liquid` — near-wall bulk **liquid** temperature `T_l` `[K]`. For
///   *subcooled* boiling `T_l < T_sat`; for saturated boiling `T_l ≈ T_sat`.
/// - `t_sat` — saturation temperature `T_sat` at the local pressure `[K]`.
/// - `rho_liquid`, `rho_vapour` — liquid / vapour density `[kg/m³]`, **> 0**.
/// - `cp_liquid` — liquid specific heat capacity `Cp_l` `[J/(kg·K)]`, **> 0**.
/// - `k_liquid` — liquid thermal conductivity `k_l` `[W/(m·K)]`, **> 0**.
/// - `latent_heat` — latent heat of vaporisation `L` `[J/kg]`, **> 0**.
/// - `h_convective` — single-phase forced-convection heat-transfer coefficient
///   `h_c` `[W/(m²·K)]`, **≥ 0** (drives the convective partition).
/// - `gravity` — gravitational acceleration magnitude `g` `[m/s²]`, **≥ 0**
///   (drives the Cole departure-frequency buoyancy term).
///
/// The wall superheat is `T_w − T_sat` and the liquid subcooling is
/// `T_sat − T_l`; both are derived inside the model, not stored here.
#[derive(Debug, Clone, Copy)]
pub struct WallBoilingConditions {
    /// Heated-wall surface temperature `T_w` `[K]`.
    pub t_wall: ThermodynamicTemperature,
    /// Near-wall bulk liquid temperature `T_l` `[K]`.
    pub t_liquid: ThermodynamicTemperature,
    /// Saturation temperature `T_sat` `[K]`.
    pub t_sat: ThermodynamicTemperature,
    /// Liquid density `ρ_l` `[kg/m³]`.
    pub rho_liquid: MassDensity,
    /// Vapour density `ρ_v` `[kg/m³]`.
    pub rho_vapour: MassDensity,
    /// Liquid specific heat capacity `Cp_l` `[J/(kg·K)]`.
    pub cp_liquid: SpecificHeatCapacity,
    /// Liquid thermal conductivity `k_l` `[W/(m·K)]`.
    pub k_liquid: ThermalConductivity,
    /// Latent heat of vaporisation `L` `[J/kg]`.
    pub latent_heat: AvailableEnergy,
    /// Single-phase convective heat-transfer coefficient `h_c` `[W/(m²·K)]`.
    pub h_convective: HeatTransfer,
    /// Gravitational acceleration magnitude `g` `[m/s²]`.
    pub gravity: Acceleration,
}

/// Result of a wall-boiling heat-flux partition.
///
/// The three additive flux components plus their sum are `uom`-typed
/// [`HeatFluxDensity`] `[W/m²]`. The diagnostic sub-closure outputs are carried
/// as documented `f64` in SI units (some — e.g. nucleation-site density
/// `[1/m²]` — have no convenient `uom` alias), following the field-value
/// convention used elsewhere in this crate.
///
/// By construction `total = convective + quenching + evaporative` and every
/// component is non-negative for physically-valid inputs (see
/// [`RpiPartitioning::partition`]).
#[derive(Debug, Clone, Copy)]
pub struct HeatFluxPartition {
    /// Single-phase convective flux `q_convective` `[W/m²]`.
    pub convective: HeatFluxDensity,
    /// Quenching (transient-conduction) flux `q_quenching` `[W/m²]`.
    pub quenching: HeatFluxDensity,
    /// Evaporative (latent) flux `q_evaporative` `[W/m²]`.
    pub evaporative: HeatFluxDensity,
    /// Total wall heat flux `q_wall = q_c + q_q + q_e` `[W/m²]`.
    pub total: HeatFluxDensity,
    /// Nucleation-site density `N` `[1/m²]` (diagnostic).
    pub nucleation_site_density: f64,
    /// Bubble departure diameter `d_dep` `[m]` (diagnostic).
    pub departure_diameter: f64,
    /// Bubble departure frequency `f` `[1/s]` (diagnostic).
    pub departure_frequency: f64,
    /// Single-phase convective area fraction `A1` `[-]` (diagnostic).
    pub area_fraction_convective: f64,
    /// Bubble-influenced area fraction `A2` `[-]` (diagnostic).
    pub area_fraction_boiling: f64,
}

// ─────────────────────────────────────────────────────────────────────────────
// Contract — the compiler-enforced interface every wall-boiling model satisfies
// ─────────────────────────────────────────────────────────────────────────────

/// Compiler-enforced contract for a wall heat-flux–partitioning model.
///
/// Every concrete boiling-regime struct implements this trait, so the compiler
/// verifies each one provides the same [`partition`](Self::partition) entry
/// point. The trait is **not** used for runtime dispatch — that is done by the
/// [`WallBoilingModel`] enum (workspace rule: no `Box<dyn>` / `dyn`). It exists
/// purely as the interface check.
pub trait WallHeatFluxPartition {
    /// Partition the wall heat flux for the given near-wall condition.
    ///
    /// # Errors
    /// - [`MultiphaseError::InvalidInput`] if a property is non-physical
    ///   (non-positive density / `Cp` / `k` / `L`, negative `h_c` or `g`).
    /// - [`MultiphaseError::NotImplemented`] for the placeholder regimes
    ///   ([`ChfModel`], [`ChfSubCoolModel`], [`DryoutModel`],
    ///   [`FilmBoilingModel`]).
    fn partition(
        &self,
        conditions: &WallBoilingConditions,
    ) -> Result<HeatFluxPartition, MultiphaseError>;
}

// ─────────────────────────────────────────────────────────────────────────────
// Concrete worked model — RPI nucleate-boiling partitioning
// ─────────────────────────────────────────────────────────────────────────────

/// RPI heat-flux–partitioning model for the **nucleate-boiling** regime
/// (Kurul & Podowski, 1991).
///
/// Splits the wall flux into convective, quenching, and evaporative parts (see
/// the [module docs](crate::wall_boiling)) and closes it with three literature
/// sub-closures. The struct holds the tunable coefficients of those
/// sub-closures; [`RpiPartitioning::default`] uses OpenFOAM's default values.
///
/// ## Coefficients (units and defaults)
///
/// Lemmert–Chawla nucleation-site density:
/// - `cn` — model constant `Cn` `[-]`, default `1.0`.
/// - `n_ref` — reference site density `N_ref` `[1/m²]`, default `9.922e5`.
/// - `delta_t_ref` — reference superheat `ΔT_ref` `[K]`, default `10.0`.
/// - `nucleation_exponent` — superheat exponent `[-]`, default `1.805`.
///
/// Tolubinski–Kostanchuk departure diameter:
/// - `d_ref` — reference diameter `[m]`, default `6.0e-4`.
/// - `d_max` — upper clamp `[m]`, default `1.4e-3`.
/// - `d_min` — lower clamp `[m]`, default `1.0e-6`.
/// - `subcooling_scale` — subcooling scale `[K]`, default `45.0`.
///
/// Cole departure frequency:
/// - `min_density_difference` — floor on `ρ_l − ρ_v` `[kg/m³]`, default `0.1`.
///
/// Quenching:
/// - `bubble_waiting_time_ratio` — waiting-time fraction `τ` `[-]`, default `0.8`.
///
/// All defaults are the OpenFOAM `wallBoiling` `fvModel` defaults (see the
/// module provenance table).
#[derive(Debug, Clone, Copy)]
pub struct RpiPartitioning {
    /// Lemmert–Chawla constant `Cn` `[-]`.
    pub cn: f64,
    /// Lemmert–Chawla reference site density `N_ref` `[1/m²]`.
    pub n_ref: f64,
    /// Lemmert–Chawla reference superheat `ΔT_ref` `[K]`.
    pub delta_t_ref: f64,
    /// Lemmert–Chawla superheat exponent `[-]`.
    pub nucleation_exponent: f64,
    /// Tolubinski–Kostanchuk reference departure diameter `[m]`.
    pub d_ref: f64,
    /// Tolubinski–Kostanchuk upper diameter clamp `[m]`.
    pub d_max: f64,
    /// Tolubinski–Kostanchuk lower diameter clamp `[m]`.
    pub d_min: f64,
    /// Tolubinski–Kostanchuk subcooling scale `[K]`.
    pub subcooling_scale: f64,
    /// Cole floor on the phase density difference `ρ_l − ρ_v` `[kg/m³]`.
    pub min_density_difference: f64,
    /// Bubble waiting-time ratio `τ` `[-]` in the quenching HTC.
    pub bubble_waiting_time_ratio: f64,
}

impl Default for RpiPartitioning {
    /// OpenFOAM `wallBoiling` `fvModel` default coefficients.
    fn default() -> Self {
        Self {
            cn: 1.0,
            n_ref: 9.922e5,
            delta_t_ref: 10.0,
            nucleation_exponent: 1.805,
            d_ref: 6.0e-4,
            d_max: 1.4e-3,
            d_min: 1.0e-6,
            subcooling_scale: 45.0,
            min_density_difference: 0.1,
            bubble_waiting_time_ratio: 0.8,
        }
    }
}

impl RpiPartitioning {
    /// **Nucleation-site density** `N` `[1/m²]` (Lemmert & Chawla, 1977).
    ///
    /// `N = Cn · N_ref · max(ΔT_sup / ΔT_ref, 0)^exponent`, where the wall
    /// superheat `ΔT_sup = T_w − T_sat` `[K]`. Below saturation (`ΔT_sup ≤ 0`)
    /// no sites are active and `N = 0`.
    ///
    /// Ports `LemmertChawla::calculate` (`LemmertChawla.C`): defaults
    /// `Cn = 1`, `N_ref = 9.922e5 /m²`, `ΔT_ref = 10 K`, exponent `1.805`.
    ///
    /// # Parameters
    /// - `delta_t_superheat` — wall superheat `T_w − T_sat` `[K]`.
    ///
    /// Reference: Lemmert, M. & Chawla, J.M. (1977), *"Influence of flow
    /// velocity on surface boiling heat transfer coefficient"*, in
    /// Heat Transfer in Boiling (Hahne & Grigull, eds.), Academic Press.
    pub fn nucleation_site_density(&self, delta_t_superheat: f64) -> f64 {
        let x = (delta_t_superheat / self.delta_t_ref).max(0.0);
        self.cn * self.n_ref * x.powf(self.nucleation_exponent)
    }

    /// **Bubble departure diameter** `d_dep` `[m]` (Tolubinski & Kostanchuk, 1970).
    ///
    /// `d_dep = clamp(d_ref · exp(−ΔT_sub / T_scale), d_min, d_max)`, where the
    /// liquid subcooling `ΔT_sub = T_sat − T_l` `[K]` (clamped at `0`). Higher
    /// subcooling shrinks departing bubbles.
    ///
    /// Ports `TolubinskiKostanchuk::calculate` (`TolubinskiKostanchuk.C`):
    /// defaults `d_ref = 6e-4 m`, `d_max = 1.4e-3 m`, `d_min = 1e-6 m`,
    /// `T_scale = 45 K`.
    ///
    /// # Parameters
    /// - `delta_t_subcooling` — liquid subcooling `T_sat − T_l` `[K]`.
    ///
    /// Reference: Tolubinski, V.I. & Kostanchuk, D.M. (1970), *"Vapour bubbles
    /// growth rate and heat transfer intensity at subcooled water boiling"*,
    /// 4th Int. Heat Transfer Conf., Paris, paper B-2.8.
    pub fn departure_diameter(&self, delta_t_subcooling: f64) -> f64 {
        let sub = delta_t_subcooling.max(0.0);
        (self.d_ref * (-sub / self.subcooling_scale).exp())
            .min(self.d_max)
            .max(self.d_min)
    }

    /// **Bubble departure frequency** `f` `[1/s]` (Cole, 1960).
    ///
    /// `f = sqrt( 4·g·max(ρ_l − ρ_v, ρ_min) / (3·d_dep·ρ_l) )`, a buoyancy /
    /// terminal-velocity balance over the departure diameter.
    ///
    /// Ports `Cole::calculate` (`Cole.C`): floor `ρ_min = 0.1 kg/m³` on the
    /// density difference.
    ///
    /// # Parameters
    /// - `departure_diameter` — `d_dep` `[m]`, **> 0**.
    /// - `rho_liquid`, `rho_vapour` — phase densities `[kg/m³]`.
    /// - `gravity` — `g` `[m/s²]`.
    ///
    /// Reference: Cole, R. (1960), *"A photographic study of pool boiling in the
    /// region of the critical heat flux"*, AIChE J. **6**(4):533–538.
    pub fn departure_frequency(
        &self,
        departure_diameter: f64,
        rho_liquid: f64,
        rho_vapour: f64,
        gravity: f64,
    ) -> f64 {
        let d_rho = (rho_liquid - rho_vapour).max(self.min_density_difference);
        (4.0 * gravity * d_rho / (3.0 * departure_diameter * rho_liquid)).sqrt()
    }
}

impl WallHeatFluxPartition for RpiPartitioning {
    /// Evaluate the full RPI three-way partition at the given near-wall state.
    ///
    /// ## Method
    ///
    /// 1. Derive superheat `ΔT_sup = T_w − T_sat`, subcooling
    ///    `ΔT_sub = max(T_sat − T_l, 0)`, and wall-to-liquid difference
    ///    `ΔT_wl = max(T_w − T_l, 0)`.
    /// 2. Sub-closures: `N` ([`nucleation_site_density`](Self::nucleation_site_density)),
    ///    `d_dep` ([`departure_diameter`](Self::departure_diameter)),
    ///    `f` ([`departure_frequency`](Self::departure_frequency)).
    /// 3. Bubble-influence area fraction (Del Valle & Kenning, 1985):
    ///    Jakob number `Ja = ρ_l·Cp_l·ΔT_sub / (ρ_v·L)`, influence factor
    ///    `A_l = 4.8·exp(−Ja/80)`, then
    ///    `A2 = min(π·d_dep²·N·A_l / 4, 1)` and `A1 = max(1 − A2, 1e-4)`.
    ///    (Ports the `A1`/`A2` construction of `wallBoiling.C::calcBoiling`.)
    /// 4. Fluxes:
    ///    - `q_convective = A1 · h_c · ΔT_wl`
    ///    - `q_quenching  = A2 · h_q · ΔT_wl`, with the Mikic–Rohsenow (1969)
    ///      transient-conduction HTC
    ///      `h_q = 2·sqrt(k_l·ρ_l·Cp_l·f·τ / π)`.
    ///    - `q_evaporative = (π/6)·d_dep³ · ρ_v · N · f · L` (classic
    ///      Kurul–Podowski departing-bubble latent transport).
    ///    - `q_wall = q_convective + q_quenching + q_evaporative`.
    ///
    /// Every returned component is **non-negative** for valid inputs (each
    /// factor is non-negative: `A1,A2 ∈ [0,1]`, `h_c,h_q,N,f,d_dep,ΔT_wl ≥ 0`),
    /// and `total` is exactly their sum.
    ///
    /// ## Units
    /// Inputs are `uom`-typed ([`WallBoilingConditions`]); the four returned
    /// fluxes are [`HeatFluxDensity`] `[W/m²]`.
    ///
    /// # Errors
    /// [`MultiphaseError::InvalidInput`] if a density, `Cp`, `k`, or `L` is
    /// `≤ 0`, or if `h_c` or `g` is `< 0`.
    fn partition(&self, c: &WallBoilingConditions) -> Result<HeatFluxPartition, MultiphaseError> {
        // ── Extract SI f64 at the boundary (compute in plain f64) ────────────
        let t_w = c.t_wall.get::<kelvin>();
        let t_l = c.t_liquid.get::<kelvin>();
        let t_sat = c.t_sat.get::<kelvin>();
        let rho_l = c.rho_liquid.get::<kilogram_per_cubic_meter>();
        let rho_v = c.rho_vapour.get::<kilogram_per_cubic_meter>();
        let cp_l = c.cp_liquid.get::<joule_per_kilogram_kelvin>();
        let k_l = c.k_liquid.get::<watt_per_meter_kelvin>();
        let l_heat = c.latent_heat.get::<joule_per_kilogram>();
        let h_c = c.h_convective.get::<watt_per_square_meter_kelvin>();
        let g = c.gravity.get::<meter_per_second_squared>();

        // ── Validate physical inputs ─────────────────────────────────────────
        if rho_l <= 0.0 || rho_v <= 0.0 {
            return Err(MultiphaseError::InvalidInput(format!(
                "phase densities must be > 0 (got rho_l={rho_l}, rho_v={rho_v})"
            )));
        }
        if cp_l <= 0.0 || k_l <= 0.0 || l_heat <= 0.0 {
            return Err(MultiphaseError::InvalidInput(format!(
                "Cp_l, k_l, L must be > 0 (got Cp_l={cp_l}, k_l={k_l}, L={l_heat})"
            )));
        }
        if h_c < 0.0 || g < 0.0 {
            return Err(MultiphaseError::InvalidInput(format!(
                "h_c and g must be >= 0 (got h_c={h_c}, g={g})"
            )));
        }

        // ── Temperature differences ──────────────────────────────────────────
        let dt_superheat = t_w - t_sat;
        let dt_subcooling = (t_sat - t_l).max(0.0);
        let dt_wall_liquid = (t_w - t_l).max(0.0);

        // ── Sub-closures ─────────────────────────────────────────────────────
        let n_sites = self.nucleation_site_density(dt_superheat);
        let d_dep = self.departure_diameter(dt_subcooling);
        let f_dep = self.departure_frequency(d_dep, rho_l, rho_v, g);

        // ── Bubble-influence area fractions (Del Valle & Kenning, 1985) ───────
        let jakob = rho_l * cp_l * dt_subcooling / (rho_v * l_heat);
        let influence = 4.8 * (-jakob / 80.0).exp();
        let a2 = (PI * d_dep * d_dep * n_sites * influence / 4.0).min(1.0);
        let a1 = (1.0 - a2).max(1.0e-4);

        // ── Heat-flux components ─────────────────────────────────────────────
        // Convective: single-phase forced convection over A1.
        let q_convective = a1 * h_c * dt_wall_liquid;

        // Quenching: Mikic-Rohsenow (1969) transient conduction over A2.
        let h_quench =
            2.0 * (k_l * rho_l * cp_l * f_dep * self.bubble_waiting_time_ratio / PI).sqrt();
        let q_quenching = a2 * h_quench * dt_wall_liquid;

        // Evaporative: latent heat of departing bubbles (Kurul-Podowski).
        let q_evaporative = (PI / 6.0) * d_dep.powi(3) * rho_v * n_sites * f_dep * l_heat;

        let q_total = q_convective + q_quenching + q_evaporative;

        Ok(HeatFluxPartition {
            convective: HeatFluxDensity::new::<watt_per_square_meter>(q_convective),
            quenching: HeatFluxDensity::new::<watt_per_square_meter>(q_quenching),
            evaporative: HeatFluxDensity::new::<watt_per_square_meter>(q_evaporative),
            total: HeatFluxDensity::new::<watt_per_square_meter>(q_total),
            nucleation_site_density: n_sites,
            departure_diameter: d_dep,
            departure_frequency: f_dep,
            area_fraction_convective: a1,
            area_fraction_boiling: a2,
        })
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Placeholder regime models — architecture only, not yet implemented
// ─────────────────────────────────────────────────────────────────────────────

/// **Critical-heat-flux (CHF) regime** — architecture placeholder.
///
/// The CHF regime marks the boiling crisis where the wall dries out and the
/// heat-transfer coefficient collapses. A concrete model would evaluate a CHF
/// correlation (e.g. Zuber's hydrodynamic-instability limit, or a lookup table)
/// and blend the partition toward the transition boiling curve.
///
/// **Not implemented.** [`partition`](WallHeatFluxPartition::partition) returns
/// [`MultiphaseError::NotImplemented`]. Present so the [`WallBoilingModel`] enum
/// spans the full regime set at the architecture stage (bead `op-2kk.4`).
#[derive(Debug, Clone, Copy, Default)]
pub struct ChfModel;

impl WallHeatFluxPartition for ChfModel {
    fn partition(&self, _c: &WallBoilingConditions) -> Result<HeatFluxPartition, MultiphaseError> {
        Err(MultiphaseError::NotImplemented(
            "critical-heat-flux (CHF) wall-boiling model is a framework \
             placeholder — not yet implemented (bead op-2kk.4)"
                .to_string(),
        ))
    }
}

/// **Subcooled-CHF regime** — architecture placeholder.
///
/// A concrete model would apply a subcooling-corrected CHF correlation for the
/// forced-convection subcooled regime (e.g. a Groeneveld-style look-up-table
/// correction, or the Hall–Mudawar subcooled CHF correlation).
///
/// **Not implemented.** [`partition`](WallHeatFluxPartition::partition) returns
/// [`MultiphaseError::NotImplemented`] (bead `op-2kk.4`).
#[derive(Debug, Clone, Copy, Default)]
pub struct ChfSubCoolModel;

impl WallHeatFluxPartition for ChfSubCoolModel {
    fn partition(&self, _c: &WallBoilingConditions) -> Result<HeatFluxPartition, MultiphaseError> {
        Err(MultiphaseError::NotImplemented(
            "subcooled-CHF wall-boiling model is a framework placeholder — \
             not yet implemented (bead op-2kk.4)"
                .to_string(),
        ))
    }
}

/// **Dryout / post-dryout regime** — architecture placeholder.
///
/// Past dryout, the wall is vapour-blanketed and heat transfer is governed by
/// the vapour phase plus droplet deposition. A concrete model would evaluate a
/// post-dryout heat-transfer correlation and a droplet-deposition closure.
///
/// **Not implemented.** [`partition`](WallHeatFluxPartition::partition) returns
/// [`MultiphaseError::NotImplemented`] (bead `op-2kk.5`).
#[derive(Debug, Clone, Copy, Default)]
pub struct DryoutModel;

impl WallHeatFluxPartition for DryoutModel {
    fn partition(&self, _c: &WallBoilingConditions) -> Result<HeatFluxPartition, MultiphaseError> {
        Err(MultiphaseError::NotImplemented(
            "dryout / post-dryout wall-boiling model is a framework \
             placeholder — not yet implemented (bead op-2kk.5)"
                .to_string(),
        ))
    }
}

/// **Film-boiling regime** — architecture placeholder.
///
/// In stable film boiling a continuous vapour film insulates the wall; heat
/// transfer is by conduction/convection across the film plus radiation. A
/// concrete model would evaluate a film-boiling correlation (e.g. Bromley) and
/// a radiation contribution.
///
/// **Not implemented.** [`partition`](WallHeatFluxPartition::partition) returns
/// [`MultiphaseError::NotImplemented`] (bead `op-2kk.5`).
#[derive(Debug, Clone, Copy, Default)]
pub struct FilmBoilingModel;

impl WallHeatFluxPartition for FilmBoilingModel {
    fn partition(&self, _c: &WallBoilingConditions) -> Result<HeatFluxPartition, MultiphaseError> {
        Err(MultiphaseError::NotImplemented(
            "film-boiling wall-boiling model is a framework placeholder — \
             not yet implemented (bead op-2kk.5)"
                .to_string(),
        ))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Dispatch enum — the closed set of wall-boiling regimes (no `dyn`)
// ─────────────────────────────────────────────────────────────────────────────

/// Wall-boiling closure, dispatched by regime.
///
/// Enum dispatch (not `dyn`) per the workspace design rules: the set of boiling
/// regimes is closed and known at compile time, so a `match` is exhaustive
/// (adding a regime forces every call site to handle it) and every variant is
/// rust-analyzer-navigable. Each variant wraps a concrete struct that implements
/// the [`WallHeatFluxPartition`] contract.
///
/// ## Variants
/// - [`NucleateBoiling`](Self::NucleateBoiling) — **implemented**: RPI
///   three-way partitioning ([`RpiPartitioning`]).
/// - [`Chf`](Self::Chf) — critical heat flux (placeholder, [`ChfModel`]).
/// - [`ChfSubCool`](Self::ChfSubCool) — subcooled CHF (placeholder,
///   [`ChfSubCoolModel`]).
/// - [`Dryout`](Self::Dryout) — dryout / post-dryout (placeholder,
///   [`DryoutModel`]).
/// - [`FilmBoiling`](Self::FilmBoiling) — film boiling (placeholder,
///   [`FilmBoilingModel`]).
#[derive(Debug, Clone, Copy)]
pub enum WallBoilingModel {
    /// Nucleate boiling — RPI heat-flux partitioning (implemented).
    NucleateBoiling(RpiPartitioning),
    /// Critical heat flux (architecture placeholder).
    Chf(ChfModel),
    /// Subcooled critical heat flux (architecture placeholder).
    ChfSubCool(ChfSubCoolModel),
    /// Dryout / post-dryout (architecture placeholder).
    Dryout(DryoutModel),
    /// Film boiling (architecture placeholder).
    FilmBoiling(FilmBoilingModel),
}

impl WallBoilingModel {
    /// Partition the wall heat flux using the selected regime model.
    ///
    /// Dispatches to the wrapped concrete model's
    /// [`WallHeatFluxPartition::partition`]. Only
    /// [`NucleateBoiling`](Self::NucleateBoiling) is implemented; the other
    /// variants return [`MultiphaseError::NotImplemented`].
    ///
    /// # Errors
    /// Propagates the wrapped model's error — [`MultiphaseError::InvalidInput`]
    /// for non-physical inputs, [`MultiphaseError::NotImplemented`] for the
    /// placeholder regimes.
    pub fn partition(
        &self,
        conditions: &WallBoilingConditions,
    ) -> Result<HeatFluxPartition, MultiphaseError> {
        match self {
            Self::NucleateBoiling(m) => m.partition(conditions),
            Self::Chf(m) => m.partition(conditions),
            Self::ChfSubCool(m) => m.partition(conditions),
            Self::Dryout(m) => m.partition(conditions),
            Self::FilmBoiling(m) => m.partition(conditions),
        }
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// Tests — verification (partition-sum conservation, non-negativity, hand-checked
// sub-closures, enum-dispatch reachability, clean NotImplemented). NOT
// validation: no experimental / reference-solver comparison here.
// ═════════════════════════════════════════════════════════════════════════════
#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    fn temp(k: f64) -> ThermodynamicTemperature {
        ThermodynamicTemperature::new::<kelvin>(k)
    }

    /// Representative **subcooled nucleate boiling** of water near 1 atm:
    /// wall 10 K superheated, liquid 10 K subcooled, saturated-water properties.
    fn subcooled_water() -> WallBoilingConditions {
        WallBoilingConditions {
            t_wall: temp(383.15),   // T_sat + 10 K
            t_liquid: temp(363.15), // T_sat − 10 K (subcooled)
            t_sat: temp(373.15),
            rho_liquid: MassDensity::new::<kilogram_per_cubic_meter>(958.0),
            rho_vapour: MassDensity::new::<kilogram_per_cubic_meter>(0.597),
            cp_liquid: SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(4216.0),
            k_liquid: ThermalConductivity::new::<watt_per_meter_kelvin>(0.679),
            latent_heat: AvailableEnergy::new::<joule_per_kilogram>(2.257e6),
            h_convective: HeatTransfer::new::<watt_per_square_meter_kelvin>(20_000.0),
            gravity: Acceleration::new::<meter_per_second_squared>(9.81),
        }
    }

    // ── Sub-closures vs hand computation ─────────────────────────────────────

    /// Methodology: Lemmert-Chawla N = Cn·N_ref·(ΔT_sup/ΔT_ref)^1.805 with the
    /// defaults (Cn=1, N_ref=9.922e5, ΔT_ref=10). At ΔT_sup = 10 K the base is
    /// 1.0 so N = 9.922e5 /m² exactly; at ΔT_sup ≤ 0 the site density is 0.
    /// Result (2026-07-23): N(10 K)=9.922e5 /m² (rel err <1e-12); N(−5 K)=0. PASS.
    #[test]
    fn nucleation_site_density_hand_check() {
        let m = RpiPartitioning::default();
        assert_relative_eq!(
            m.nucleation_site_density(10.0),
            9.922e5,
            max_relative = 1e-12
        );
        assert_eq!(m.nucleation_site_density(-5.0), 0.0);
    }

    /// Methodology: Tolubinski-Kostanchuk d = clamp(6e-4·exp(−ΔT_sub/45),1e-6,1.4e-3).
    /// At ΔT_sub = 10 K: 6e-4·exp(−0.2222…) = 4.804424e-4 m. Saturated (ΔT_sub=0)
    /// gives the unclamped d_ref = 6e-4 m; enormous subcooling clamps to d_min.
    /// Result (2026-07-23): d(10 K)=4.804424e-4 m (rel err<1e-9); d(0)=6e-4 m;
    /// d(1e4 K)=1e-6 m (clamped). PASS.
    #[test]
    fn departure_diameter_hand_check() {
        let m = RpiPartitioning::default();
        assert_relative_eq!(m.departure_diameter(10.0), 4.804424e-4, max_relative = 1e-6);
        assert_relative_eq!(m.departure_diameter(0.0), 6.0e-4, max_relative = 1e-12);
        assert_relative_eq!(m.departure_diameter(1.0e4), 1.0e-6, max_relative = 1e-12);
    }

    /// Methodology: Cole f = sqrt(4·g·(ρ_l−ρ_v)/(3·d·ρ_l)). With g=9.81,
    /// ρ_l=958, ρ_v=0.597, d=4.804424e-4: f = 164.9483 Hz. Also check the
    /// min-density-difference floor engages when ρ_l ≈ ρ_v.
    /// Result (2026-07-23): f = 164.9483 Hz (rel err <1e-6); floored branch >0. PASS.
    #[test]
    fn departure_frequency_hand_check() {
        let m = RpiPartitioning::default();
        let f = m.departure_frequency(4.804424e-4, 958.0, 0.597, 9.81);
        assert_relative_eq!(f, 164.9483, max_relative = 1e-5);
        // Density-difference floor: ρ_l == ρ_v would give 0 without the floor.
        let f_floor = m.departure_frequency(1.0e-3, 500.0, 500.0, 9.81);
        assert!(f_floor > 0.0);
    }

    // ── Full RPI partition ───────────────────────────────────────────────────

    /// Methodology (conservation + non-negativity): evaluate the RPI partition
    /// for the representative subcooled-water condition and check that
    /// (a) total == convective + quenching + evaporative to machine precision,
    /// (b) every component is ≥ 0, (c) the area fractions satisfy A1+A2 ≈ 1
    /// (they exactly do here since A2 < 1 so A1 = 1−A2), and (d) the magnitudes
    /// match the hand computation.
    /// Result (2026-07-23), water @ 10 K superheat / 10 K subcooling:
    ///   q_convective = 1.625632e5 W/m²
    ///   q_quenching  = 2.548355e5 W/m²
    ///   q_evaporative= 1.280488e4 W/m²
    ///   q_total      = 4.302036e5 W/m²
    ///   N=9.922e5 /m², d_dep=4.804424e-4 m, f=164.948 Hz, A2=0.593592, A1=0.406408.
    /// Conservation residual |total−Σ| < 1e-6 W/m²; all components ≥ 0. PASS.
    #[test]
    fn rpi_partition_conserves_and_is_nonnegative() {
        let m = RpiPartitioning::default();
        let p = m.partition(&subcooled_water()).unwrap();

        let qc = p.convective.get::<watt_per_square_meter>();
        let qq = p.quenching.get::<watt_per_square_meter>();
        let qe = p.evaporative.get::<watt_per_square_meter>();
        let qt = p.total.get::<watt_per_square_meter>();

        // (a) conservation: total is exactly the sum of the three parts.
        assert_relative_eq!(qt, qc + qq + qe, max_relative = 1e-12);

        // (b) non-negativity of every component.
        assert!(qc >= 0.0, "q_convective negative: {qc}");
        assert!(qq >= 0.0, "q_quenching negative: {qq}");
        assert!(qe >= 0.0, "q_evaporative negative: {qe}");

        // (c) area fractions partition unity (A2 < 1 here ⇒ A1 = 1 − A2).
        assert_relative_eq!(
            p.area_fraction_convective + p.area_fraction_boiling,
            1.0,
            max_relative = 1e-12
        );

        // (d) magnitudes vs the hand computation.
        assert_relative_eq!(qc, 1.625632e5, max_relative = 1e-4);
        assert_relative_eq!(qq, 2.548355e5, max_relative = 1e-4);
        assert_relative_eq!(qe, 1.280488e4, max_relative = 1e-4);
        assert_relative_eq!(qt, 4.302036e5, max_relative = 1e-4);
    }

    /// Methodology (degenerate limit): with no wall superheat and no subcooling
    /// (T_wall = T_liquid = T_sat) there are no active nucleation sites, so the
    /// boiling area fraction A2 = 0, and both the quenching and evaporative
    /// fluxes vanish while the convective flux vanishes too (ΔT_wl = 0). The
    /// partition must therefore be identically zero.
    /// Result (2026-07-23): q_c = q_q = q_e = q_total = 0; A1 = 1, A2 = 0. PASS.
    #[test]
    fn rpi_partition_zero_at_saturation_no_drive() {
        let mut c = subcooled_water();
        c.t_wall = temp(373.15);
        c.t_liquid = temp(373.15);
        let p = RpiPartitioning::default().partition(&c).unwrap();
        assert_eq!(p.total.get::<watt_per_square_meter>(), 0.0);
        assert_eq!(p.area_fraction_boiling, 0.0);
        assert_relative_eq!(p.area_fraction_convective, 1.0, max_relative = 1e-12);
    }

    /// The partition rejects non-physical fluid properties.
    /// Result (2026-07-23): zero density and negative HTC both → Err. PASS.
    #[test]
    fn rpi_partition_rejects_bad_inputs() {
        let m = RpiPartitioning::default();
        let mut c = subcooled_water();
        c.rho_vapour = MassDensity::new::<kilogram_per_cubic_meter>(0.0);
        assert!(m.partition(&c).is_err());

        let mut c2 = subcooled_water();
        c2.h_convective = HeatTransfer::new::<watt_per_square_meter_kelvin>(-1.0);
        assert!(m.partition(&c2).is_err());
    }

    // ── Enum dispatch ────────────────────────────────────────────────────────

    /// Methodology (dispatch reachability): the enum must route to each variant.
    /// The NucleateBoiling arm produces a finite positive total for the
    /// representative input; every placeholder arm returns NotImplemented.
    /// Result (2026-07-23): NucleateBoiling total > 0; Chf/ChfSubCool/Dryout/
    /// FilmBoiling each → MultiphaseError::NotImplemented. PASS.
    #[test]
    fn enum_dispatch_reaches_every_variant() {
        let c = subcooled_water();

        let nb = WallBoilingModel::NucleateBoiling(RpiPartitioning::default());
        let p = nb.partition(&c).unwrap();
        assert!(p.total.get::<watt_per_square_meter>() > 0.0);

        for model in [
            WallBoilingModel::Chf(ChfModel),
            WallBoilingModel::ChfSubCool(ChfSubCoolModel),
            WallBoilingModel::Dryout(DryoutModel),
            WallBoilingModel::FilmBoiling(FilmBoilingModel),
        ] {
            match model.partition(&c) {
                Err(MultiphaseError::NotImplemented(_)) => {}
                other => panic!("expected NotImplemented, got {other:?}"),
            }
        }
    }
}
