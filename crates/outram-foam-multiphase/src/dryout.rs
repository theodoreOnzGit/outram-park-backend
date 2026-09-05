// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK.
//
// This module is a literature-based framework, not a port of OpenFOAM source.
// The one worked closure implemented here follows published correlations,
// credited inline:
//   - Dougall, R. S. & Rohsenow, W. M. (1963), "Film boiling on the inside of
//     vertical tubes with upward flow of the fluid at low qualities",
//     MIT Heat Transfer Laboratory Report No. 9079-26.
//   - Dittus, F. W. & Boelter, L. M. K. (1930), Univ. California Publ. Eng. 2,
//     443 (the single-phase Nu = 0.023 Re^0.8 Pr^n base correlation).
//   - Collier, J. G. & Thome, J. R. (1994), "Convective Boiling and
//     Condensation", 3rd ed., Oxford University Press (dryout / post-dryout
//     regime background).
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

//! Stage 5 — **Dryout & post-dryout framework** (bead `op-2kk.5`).
//!
//! A **reserved interface layer** for the dryout / post-dryout boiling-crisis
//! regimes of a heated two-phase channel. This module deliberately ships the
//! *architecture* — traits as compiler-enforced contracts, plus `enum`
//! dispatch (no `dyn`) — with **one genuinely-implemented worked closure per
//! trait** to prove the interface is real, and every remaining regime honestly
//! flagged [`MultiphaseError::NotImplemented`]. It is **not** a validated
//! boiling-crisis model.
//!
//! ## The physical picture (what this framework will eventually cover)
//!
//! In a heated channel carrying a boiling flow, once the wall heat flux or the
//! flow quality is high enough the continuous liquid contact with the wall
//! breaks down — the **boiling crisis**. Two limiting mechanisms:
//!
//! - **Departure from nucleate boiling (DNB)** — at low quality / high flux,
//!   bubble crowding and vapour-blanket formation lift the liquid off the wall.
//! - **Dryout** — at high quality in annular flow, the liquid film on the wall
//!   is depleted by evaporation and entrainment until it vanishes.
//!
//! Past that point the wall is cooled by a far less effective mechanism and its
//! temperature can excurse sharply. The downstream **post-dryout** heat-transfer
//! regimes are:
//!
//! - **Film boiling** — a stable vapour film blankets the wall; heat crosses it
//!   by forced convection to the vapour (plus radiation at high `T_w`).
//! - **Transition boiling** — an unstable, intermittent mix of film and
//!   nucleate boiling between the critical-heat-flux point and the minimum-film-
//!   boiling point; the time-averaged flux *falls* with increasing `ΔT`.
//! - **Critical-heat-flux (CHF) recovery / rewetting (quench)** — the reverse
//!   transition, where a quench front re-establishes liquid–wall contact.
//!
//! ## What is genuinely implemented here (the two worked examples)
//!
//! 1. [`DryoutOnsetModel::CriticalVoidFraction`] — a simple **critical-void /
//!    critical-quality onset indicator** `α > α_crit`. Enough to exercise the
//!    [`DryoutModel`] contract with an exact hand-checkable result.
//! 2. [`PostDryoutModel::FilmBoilingDougallRohsenow`] — the
//!    **Dougall–Rohsenow (1963)** post-dryout film-boiling HTC: Dittus–Boelter
//!    applied to the vapour phase flowing at the total mass flux with a
//!    density-ratio quality weighting. A real, dimensionally-consistent,
//!    literature-cited closure.
//!
//! ## Honest scope — what is **reserved (NOT implemented)**
//!
//! Every other enum variant returns [`MultiphaseError::NotImplemented`] with a
//! message naming the physics a future implementer must supply. None of it is
//! faked or stubbed to a plausible-looking number:
//!
//! - **DNB / critical-heat-flux onset** ([`DryoutOnsetModel::CriticalHeatFlux`])
//!   — bubble crowding, near-wall void build-up, vapour-blanket lift-off; needs
//!   a CHF look-up table or correlation (e.g. Groeneveld, W-3, Biasi).
//! - **Mechanistic liquid-film dryout**
//!   ([`DryoutOnsetModel::LiquidFilmDepletion`]) — the annular-flow film
//!   mass balance (evaporation + entrainment − deposition → film thickness → 0).
//! - **Mechanistic / radiative film boiling**
//!   ([`PostDryoutModel::FilmBoilingMechanistic`]) — vapour-film boundary-layer
//!   separation, interfacial waves, wall-to-interface radiation.
//! - **Transition boiling** ([`PostDryoutModel::TransitionBoiling`]) — the
//!   unstable film/nucleate interpolation between CHF and the minimum-film-
//!   boiling point.
//! - **CHF recovery / rewetting** ([`PostDryoutModel::ChfRecovery`]) — quench-
//!   front conduction–convection and the rewetting (Leidenfrost) temperature.
//!
//! No benchmark validation has been performed. The tests below are
//! *verification* checks (formula exactness against an independently computed
//! value, enum-dispatch coverage, clean `NotImplemented` errors), **not**
//! *validation* against experimental boiling-crisis data.

use crate::MultiphaseError;
use uom::si::f64::{
    DynamicViscosity, HeatTransfer, Length, MassDensity, MassFlux, Ratio, ThermalConductivity,
    ThermodynamicTemperature,
};
use uom::si::heat_transfer::watt_per_square_meter_kelvin;
use uom::si::length::meter;
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::mass_flux::kilogram_per_square_meter_second;
use uom::si::ratio::ratio;
use uom::si::thermal_conductivity::watt_per_meter_kelvin;

// ─────────────────────────────────────────────────────────────────────────────
// Shared flow-condition inputs
// ─────────────────────────────────────────────────────────────────────────────

/// Local channel flow conditions consumed by a [`DryoutModel`] to decide
/// whether the boiling crisis (dryout / DNB) has occurred at a point.
///
/// ## Fields, units and valid ranges
/// - `quality` — thermodynamic (or flow) vapour mass fraction `x` `[-]`,
///   physical range `[0, 1]` (a [`Ratio`]).
/// - `void_fraction` — vapour volume fraction `α` `[-]`, physical range
///   `[0, 1]` (a [`Ratio`]).
/// - `mass_flux` — total mixture mass flux `G` `[kg/(m²·s)]` (a [`MassFlux`]),
///   `≥ 0`.
/// - `wall_temperature` — heated-wall temperature `T_w` `[K]` (a
///   [`ThermodynamicTemperature`]); reserved for the CHF / film-boiling
///   criteria that key off wall superheat.
#[derive(Debug, Clone, Copy)]
pub struct DryoutConditions {
    /// Vapour mass fraction (quality) `x` `[-]`, range `[0, 1]`.
    pub quality: Ratio,
    /// Vapour volume fraction (void) `α` `[-]`, range `[0, 1]`.
    pub void_fraction: Ratio,
    /// Total mixture mass flux `G` `[kg/(m²·s)]`, `≥ 0`.
    pub mass_flux: MassFlux,
    /// Heated-wall temperature `T_w` `[K]`.
    pub wall_temperature: ThermodynamicTemperature,
}

/// Vapour-phase thermophysical properties (plus the liquid density needed by
/// the density-ratio quality weighting) evaluated by the caller at the local
/// state — the closure inputs a post-dryout HTC correlation needs.
///
/// The caller is responsible for evaluating these at the appropriate reference
/// state (bulk-vapour or film temperature, per the correlation's basis) from a
/// property library; this framework does not compute properties.
///
/// ## Fields, units and valid ranges
/// - `density` — vapour density `ρ_g` `[kg/m³]`, `> 0`.
/// - `liquid_density` — saturated-liquid density `ρ_f` `[kg/m³]`, `> 0`.
/// - `dynamic_viscosity` — vapour dynamic viscosity `μ_g` `[Pa·s]`, `> 0`.
/// - `thermal_conductivity` — vapour thermal conductivity `k_g` `[W/(m·K)]`,
///   `> 0`.
/// - `prandtl` — vapour Prandtl number `Pr_g` `[-]` (a [`Ratio`]), `> 0`.
#[derive(Debug, Clone, Copy)]
pub struct VaporProperties {
    /// Vapour density `ρ_g` `[kg/m³]`, `> 0`.
    pub density: MassDensity,
    /// Saturated-liquid density `ρ_f` `[kg/m³]`, `> 0`.
    pub liquid_density: MassDensity,
    /// Vapour dynamic viscosity `μ_g` `[Pa·s]`, `> 0`.
    pub dynamic_viscosity: DynamicViscosity,
    /// Vapour thermal conductivity `k_g` `[W/(m·K)]`, `> 0`.
    pub thermal_conductivity: ThermalConductivity,
    /// Vapour Prandtl number `Pr_g` `[-]`, `> 0`.
    pub prandtl: Ratio,
}

/// Local channel flow conditions consumed by a [`PostDryoutModel`] to evaluate
/// the wall→fluid heat-transfer coefficient once the wall has dried out.
///
/// ## Fields, units and valid ranges
/// - `mass_flux` — total mixture mass flux `G` `[kg/(m²·s)]`, `> 0`.
/// - `quality` — vapour mass fraction `x` `[-]`, range `[0, 1]`.
/// - `hydraulic_diameter` — channel hydraulic diameter `D_h` `[m]`, `> 0`.
/// - `wall_temperature` — heated-wall temperature `T_w` `[K]`; reserved for the
///   wall-superheat–based transition / radiative film-boiling closures (the
///   Dougall–Rohsenow worked example is a bulk-vapour correlation and does not
///   use it).
/// - `vapor` — vapour-phase [`VaporProperties`] at the reference state.
#[derive(Debug, Clone, Copy)]
pub struct PostDryoutConditions {
    /// Total mixture mass flux `G` `[kg/(m²·s)]`, `> 0`.
    pub mass_flux: MassFlux,
    /// Vapour mass fraction (quality) `x` `[-]`, range `[0, 1]`.
    pub quality: Ratio,
    /// Channel hydraulic diameter `D_h` `[m]`, `> 0`.
    pub hydraulic_diameter: Length,
    /// Heated-wall temperature `T_w` `[K]`.
    pub wall_temperature: ThermodynamicTemperature,
    /// Vapour-phase properties at the reference state.
    pub vapor: VaporProperties,
}

// ─────────────────────────────────────────────────────────────────────────────
// Dryout onset
// ─────────────────────────────────────────────────────────────────────────────

/// Outcome of a dryout-onset evaluation: whether the wall has dried out, and a
/// dimensionless margin to the criterion.
///
/// `margin` is defined as **(actual − critical)** in whatever dimensionless
/// quantity the criterion uses (e.g. `α − α_crit`). It is therefore **negative
/// before onset**, zero at the criterion, and **positive after** the wall has
/// dried out. `dried_out == (margin > 0)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DryoutIndicator {
    /// `true` once the dryout criterion is exceeded.
    pub dried_out: bool,
    /// Dimensionless margin `actual − critical` `[-]`: `< 0` pre-onset,
    /// `> 0` post-onset.
    pub margin: f64,
}

/// Compiler-enforced contract for a **dryout / boiling-crisis onset** criterion.
///
/// A model maps local [`DryoutConditions`] to a [`DryoutIndicator`] telling the
/// caller whether the wall has dried out and by what margin. This trait exists
/// to make the interface uniform and to let the compiler check every concrete
/// model implements it; runtime dispatch goes through the [`DryoutOnsetModel`]
/// enum (no `dyn`), per the workspace design rules.
pub trait DryoutModel {
    /// Evaluate the dryout-onset criterion at the given local conditions.
    ///
    /// # Errors
    /// [`MultiphaseError::InvalidInput`] for out-of-range parameters, and
    /// [`MultiphaseError::NotImplemented`] for the reserved criteria.
    fn dryout_onset(
        &self,
        conditions: &DryoutConditions,
    ) -> Result<DryoutIndicator, MultiphaseError>;
}

/// Dryout / boiling-crisis onset criteria, dispatched by `enum` (no `dyn`).
///
/// The set of onset criteria is closed and known at compile time, so a `match`
/// is exhaustive and every variant is rust-analyzer-navigable.
///
/// ## Variants
/// - [`CriticalVoidFraction`](Self::CriticalVoidFraction) — **implemented**
///   worked example: the `α > α_crit` critical-void / critical-quality
///   indicator.
/// - [`CriticalHeatFlux`](Self::CriticalHeatFlux) — **reserved** (DNB/CHF
///   look-up or correlation).
/// - [`LiquidFilmDepletion`](Self::LiquidFilmDepletion) — **reserved**
///   (mechanistic annular-film mass balance).
#[derive(Debug, Clone, Copy)]
pub enum DryoutOnsetModel {
    /// **Critical-void-fraction onset (worked example).** Dryout is declared
    /// when the local void fraction exceeds a critical value: `α > α_crit`.
    ///
    /// This is the simplest defensible onset indicator and a common
    /// annular-flow surrogate for film disappearance (in annular flow a high
    /// void fraction corresponds to a thin liquid film). `alpha_crit` is the
    /// critical void fraction `[-]` and must lie in `(0, 1]`.
    ///
    /// The returned [`DryoutIndicator::margin`] is `α − α_crit`.
    CriticalVoidFraction {
        /// Critical void fraction `α_crit` `[-]`, in `(0, 1]`.
        alpha_crit: f64,
    },

    /// **Critical-heat-flux (DNB) onset — reserved, not implemented.**
    ///
    /// A future implementation supplies a CHF look-up table or correlation
    /// (e.g. Groeneveld 2006 CHF table, the EPRI/W-3 correlation, or Biasi)
    /// and compares the local heat flux against `q″_CHF(G, x, p, D)`. The
    /// physics to add: near-wall bubble crowding and vapour-blanket lift-off
    /// at low quality / high flux. Currently returns
    /// [`MultiphaseError::NotImplemented`].
    CriticalHeatFlux,

    /// **Mechanistic liquid-film-depletion dryout — reserved, not
    /// implemented.**
    ///
    /// A future implementation solves the annular-flow liquid-film mass
    /// balance (evaporation + entrainment − deposition) and declares dryout
    /// when the film thickness reaches zero. The physics to add: the
    /// entrainment/deposition closure and the film-flow-rate ODE. Currently
    /// returns [`MultiphaseError::NotImplemented`].
    LiquidFilmDepletion,
}

impl DryoutModel for DryoutOnsetModel {
    fn dryout_onset(
        &self,
        conditions: &DryoutConditions,
    ) -> Result<DryoutIndicator, MultiphaseError> {
        match self {
            Self::CriticalVoidFraction { alpha_crit } => {
                if !(0.0 < *alpha_crit && *alpha_crit <= 1.0) {
                    return Err(MultiphaseError::InvalidInput(format!(
                        "alpha_crit must be in (0, 1] (got {alpha_crit})"
                    )));
                }
                let alpha = conditions.void_fraction.get::<ratio>();
                let margin = alpha - alpha_crit;
                Ok(DryoutIndicator {
                    dried_out: margin > 0.0,
                    margin,
                })
            }
            Self::CriticalHeatFlux => Err(MultiphaseError::NotImplemented(
                "critical-heat-flux (DNB) onset: needs a CHF look-up/correlation \
                 (Groeneveld / W-3 / Biasi) capturing bubble crowding and \
                 vapour-blanket lift-off"
                    .to_string(),
            )),
            Self::LiquidFilmDepletion => Err(MultiphaseError::NotImplemented(
                "mechanistic liquid-film-depletion dryout: needs the annular-flow \
                 film mass balance (evaporation + entrainment - deposition -> \
                 film thickness -> 0)"
                    .to_string(),
            )),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Post-dryout heat transfer
// ─────────────────────────────────────────────────────────────────────────────

/// Compiler-enforced contract for a **post-dryout wall heat-transfer** closure.
///
/// A model maps local [`PostDryoutConditions`] to the wall→fluid heat-transfer
/// coefficient [`HeatTransfer`] `[W/(m²·K)]` in the post-dryout regime. As with
/// [`DryoutModel`], the trait fixes the interface and the compiler checks each
/// concrete model; runtime dispatch goes through the [`PostDryoutModel`] enum
/// (no `dyn`).
pub trait PostDryoutHeatTransfer {
    /// Wall→fluid heat-transfer coefficient `h` `[W/(m²·K)]` in the post-dryout
    /// regime at the given local conditions.
    ///
    /// # Errors
    /// [`MultiphaseError::InvalidInput`] for out-of-range parameters, and
    /// [`MultiphaseError::NotImplemented`] for the reserved regimes.
    fn heat_transfer_coefficient(
        &self,
        conditions: &PostDryoutConditions,
    ) -> Result<HeatTransfer, MultiphaseError>;
}

/// Post-dryout heat-transfer regimes, dispatched by `enum` (no `dyn`).
///
/// The set of post-dryout regimes is closed and known at compile time, so a
/// `match` is exhaustive and every variant is rust-analyzer-navigable.
///
/// ## Variants
/// - [`FilmBoilingDougallRohsenow`](Self::FilmBoilingDougallRohsenow) —
///   **implemented** worked example: the Dougall–Rohsenow (1963) vapour-phase
///   film-boiling HTC.
/// - [`FilmBoilingMechanistic`](Self::FilmBoilingMechanistic) — **reserved**
///   (vapour-film boundary layer + radiation).
/// - [`TransitionBoiling`](Self::TransitionBoiling) — **reserved**.
/// - [`ChfRecovery`](Self::ChfRecovery) — **reserved** (rewetting / quench).
#[derive(Debug, Clone, Copy)]
pub enum PostDryoutModel {
    /// **Dougall–Rohsenow (1963) film-boiling HTC (worked example).**
    ///
    /// The post-dryout vapour-phase forced-convection correlation: apply the
    /// Dittus–Boelter form `Nu = 0.023 Re^0.8 Pr^0.4` to the vapour flowing at
    /// the *total* mass flux, using a density-ratio quality weighting for the
    /// Reynolds number.
    ///
    /// ### Formulae
    /// Reynolds number (vapour reference, Dougall–Rohsenow weighting):
    ///
    /// `Re = (G · D_h / μ_g) · [ x + (ρ_g / ρ_f)·(1 − x) ]`
    ///
    /// Nusselt number (Dittus–Boelter, heating exponent `0.4`):
    ///
    /// `Nu = 0.023 · Re^0.8 · Pr_g^0.4`
    ///
    /// Heat-transfer coefficient:
    ///
    /// `h = Nu · k_g / D_h`   `[W/(m²·K)]`
    ///
    /// where `G` is the total mass flux, `D_h` the hydraulic diameter, `x` the
    /// quality, `ρ_g`/`ρ_f` the vapour/liquid densities, and `μ_g`, `k_g`,
    /// `Pr_g` the vapour viscosity, conductivity and Prandtl number.
    ///
    /// ### Validity / limitations
    /// A single-phase-vapour convective estimate. It ignores wall-to-interface
    /// **radiation** (matters at high `T_w`), thermodynamic **non-equilibrium**
    /// (actual vapour superheat / droplet content), and **transition-boiling**
    /// effects near CHF. Reasonable at moderate-to-high quality in dispersed-
    /// flow film boiling; degrades near the dryout front and at very high wall
    /// superheat. `wall_temperature` is unused by this correlation.
    FilmBoilingDougallRohsenow,

    /// **Mechanistic / radiative film boiling — reserved, not implemented.**
    ///
    /// A future implementation resolves the vapour-film boundary layer and adds
    /// a wall→interface radiation path (and, for dispersed-flow film boiling,
    /// wall→droplet transfer). Physics to add: boundary-layer separation,
    /// interfacial waves, non-equilibrium vapour superheat. Currently returns
    /// [`MultiphaseError::NotImplemented`].
    FilmBoilingMechanistic,

    /// **Transition boiling — reserved, not implemented.**
    ///
    /// A future implementation interpolates the unstable film/nucleate regime
    /// between the critical-heat-flux point and the minimum-film-boiling point,
    /// where the time-averaged flux *decreases* with increasing wall superheat.
    /// Physics to add: the CHF and minimum-film-boiling anchor points and the
    /// intermittent-contact fraction. Currently returns
    /// [`MultiphaseError::NotImplemented`].
    TransitionBoiling,

    /// **CHF recovery / rewetting (quench) — reserved, not implemented.**
    ///
    /// A future implementation models the quench front that re-establishes
    /// liquid–wall contact: conduction–convection ahead of the front and the
    /// rewetting (Leidenfrost/minimum-film-boiling) temperature. Physics to
    /// add: the moving quench-front energy balance. Currently returns
    /// [`MultiphaseError::NotImplemented`].
    ChfRecovery,
}

impl PostDryoutHeatTransfer for PostDryoutModel {
    fn heat_transfer_coefficient(
        &self,
        conditions: &PostDryoutConditions,
    ) -> Result<HeatTransfer, MultiphaseError> {
        match self {
            Self::FilmBoilingDougallRohsenow => {
                let g = conditions
                    .mass_flux
                    .get::<kilogram_per_square_meter_second>();
                let d = conditions.hydraulic_diameter.get::<meter>();
                let x = conditions.quality.get::<ratio>();
                let rho_g = conditions.vapor.density.get::<kilogram_per_cubic_meter>();
                let rho_f = conditions
                    .vapor
                    .liquid_density
                    .get::<kilogram_per_cubic_meter>();
                let mu_g = conditions
                    .vapor
                    .dynamic_viscosity
                    .get::<uom::si::dynamic_viscosity::pascal_second>();
                let k_g = conditions
                    .vapor
                    .thermal_conductivity
                    .get::<watt_per_meter_kelvin>();
                let pr_g = conditions.vapor.prandtl.get::<ratio>();

                if g <= 0.0
                    || d <= 0.0
                    || rho_g <= 0.0
                    || rho_f <= 0.0
                    || mu_g <= 0.0
                    || pr_g <= 0.0
                {
                    return Err(MultiphaseError::InvalidInput(format!(
                        "Dougall-Rohsenow requires G, D_h, rho_g, rho_f, mu_g, Pr_g > 0 \
                         (got G={g}, D_h={d}, rho_g={rho_g}, rho_f={rho_f}, mu_g={mu_g}, \
                         Pr_g={pr_g})"
                    )));
                }
                if !(0.0..=1.0).contains(&x) {
                    return Err(MultiphaseError::InvalidInput(format!(
                        "quality x must be in [0, 1] (got {x})"
                    )));
                }

                // Density-ratio-weighted vapour Reynolds number.
                let re = (g * d / mu_g) * (x + (rho_g / rho_f) * (1.0 - x));
                // Dittus-Boelter Nusselt number (heating exponent 0.4).
                let nu = 0.023 * re.powf(0.8) * pr_g.powf(0.4);
                // Heat-transfer coefficient.
                let h = nu * k_g / d;
                Ok(HeatTransfer::new::<watt_per_square_meter_kelvin>(h))
            }
            Self::FilmBoilingMechanistic => Err(MultiphaseError::NotImplemented(
                "mechanistic/radiative film boiling: needs a vapour-film boundary-layer \
                 solve plus wall->interface radiation and non-equilibrium vapour superheat"
                    .to_string(),
            )),
            Self::TransitionBoiling => Err(MultiphaseError::NotImplemented(
                "transition boiling: needs the film/nucleate interpolation between the CHF \
                 point and the minimum-film-boiling point"
                    .to_string(),
            )),
            Self::ChfRecovery => Err(MultiphaseError::NotImplemented(
                "CHF recovery / rewetting (quench): needs the moving quench-front energy \
                 balance and the rewetting (Leidenfrost) temperature"
                    .to_string(),
            )),
        }
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// Tests — verification only (formula exactness against an independently computed
// value, enum-dispatch coverage, clean NotImplemented errors). NOT validation:
// no experimental boiling-crisis comparison here.
// ═════════════════════════════════════════════════════════════════════════════
#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;
    use uom::si::dynamic_viscosity::pascal_second;
    use uom::si::thermodynamic_temperature::kelvin;

    fn temp(k: f64) -> ThermodynamicTemperature {
        ThermodynamicTemperature::new::<kelvin>(k)
    }

    fn dryout_conditions(alpha: f64) -> DryoutConditions {
        DryoutConditions {
            quality: Ratio::new::<ratio>(0.5),
            void_fraction: Ratio::new::<ratio>(alpha),
            mass_flux: MassFlux::new::<kilogram_per_square_meter_second>(500.0),
            wall_temperature: temp(650.0),
        }
    }

    // ── Worked example 1: critical-void-fraction onset ───────────────────────

    /// Methodology: `CriticalVoidFraction { alpha_crit = 0.8 }`. Feed α = 0.85
    /// (above) and α = 0.70 (below); the indicator must flag dried_out only for
    /// the first, with margin = α − α_crit in each case.
    /// Result (2026-07-23): α=0.85 → dried_out=true, margin=+0.05 (rel<1e-12);
    /// α=0.70 → dried_out=false, margin=−0.10 (rel<1e-12). PASS.
    #[test]
    fn critical_void_fraction_onset_indicator() {
        let model = DryoutOnsetModel::CriticalVoidFraction { alpha_crit: 0.8 };
        let above = model.dryout_onset(&dryout_conditions(0.85)).unwrap();
        assert!(above.dried_out);
        assert_relative_eq!(above.margin, 0.05, max_relative = 1e-12);

        let below = model.dryout_onset(&dryout_conditions(0.70)).unwrap();
        assert!(!below.dried_out);
        assert_relative_eq!(below.margin, -0.10, max_relative = 1e-12);
    }

    /// The onset criterion rejects a non-physical `alpha_crit`.
    #[test]
    fn critical_void_fraction_rejects_bad_alpha_crit() {
        let bad = DryoutOnsetModel::CriticalVoidFraction { alpha_crit: 1.5 };
        assert!(matches!(
            bad.dryout_onset(&dryout_conditions(0.5)),
            Err(MultiphaseError::InvalidInput(_))
        ));
        let zero = DryoutOnsetModel::CriticalVoidFraction { alpha_crit: 0.0 };
        assert!(matches!(
            zero.dryout_onset(&dryout_conditions(0.5)),
            Err(MultiphaseError::InvalidInput(_))
        ));
    }

    // ── Worked example 2: Dougall-Rohsenow post-dryout film-boiling HTC ───────

    fn post_dryout_conditions() -> PostDryoutConditions {
        PostDryoutConditions {
            mass_flux: MassFlux::new::<kilogram_per_square_meter_second>(500.0),
            quality: Ratio::new::<ratio>(0.5),
            hydraulic_diameter: Length::new::<meter>(0.01),
            wall_temperature: temp(650.0),
            vapor: VaporProperties {
                density: MassDensity::new::<kilogram_per_cubic_meter>(20.0),
                liquid_density: MassDensity::new::<kilogram_per_cubic_meter>(800.0),
                dynamic_viscosity: DynamicViscosity::new::<pascal_second>(2.0e-5),
                thermal_conductivity: ThermalConductivity::new::<watt_per_meter_kelvin>(0.06),
                prandtl: Ratio::new::<ratio>(1.0),
            },
        }
    }

    /// Methodology: Dougall-Rohsenow vapour-phase film-boiling HTC evaluated at
    /// G = 500 kg/(m²·s), x = 0.5, D_h = 0.01 m, ρ_g = 20, ρ_f = 800 kg/m³,
    /// μ_g = 2.0e-5 Pa·s, k_g = 0.06 W/(m·K), Pr_g = 1.0. Hand/independent
    /// computation (Python, 2026-07-23):
    ///   factor = x + (ρ_g/ρ_f)(1−x) = 0.5 + 0.025·0.5 = 0.5125
    ///   Re = G·D_h·factor/μ_g = 500·0.01·0.5125 / 2.0e-5 = 128125
    ///   Nu = 0.023·Re^0.8·Pr^0.4 = 0.023·12192.9025… = 280.436758611…
    ///   h  = Nu·k_g/D_h = 280.436758611…·0.06/0.01 = 1682.620551668 W/(m²·K)
    /// Pass: |h − 1682.620551668| within 1e-9 relative.
    /// Result (2026-07-23): h = 1682.620551668102 W/(m²·K) (rel err < 1e-13).
    /// PASS.
    #[test]
    fn dougall_rohsenow_htc_matches_hand_computation() {
        let model = PostDryoutModel::FilmBoilingDougallRohsenow;
        let h = model
            .heat_transfer_coefficient(&post_dryout_conditions())
            .unwrap();
        assert_relative_eq!(
            h.get::<watt_per_square_meter_kelvin>(),
            1682.620551668,
            max_relative = 1e-9
        );
    }

    /// The Dougall-Rohsenow closure rejects non-physical inputs (zero mass flux
    /// and out-of-range quality).
    #[test]
    fn dougall_rohsenow_rejects_bad_inputs() {
        let model = PostDryoutModel::FilmBoilingDougallRohsenow;

        let mut zero_g = post_dryout_conditions();
        zero_g.mass_flux = MassFlux::new::<kilogram_per_square_meter_second>(0.0);
        assert!(matches!(
            model.heat_transfer_coefficient(&zero_g),
            Err(MultiphaseError::InvalidInput(_))
        ));

        let mut bad_x = post_dryout_conditions();
        bad_x.quality = Ratio::new::<ratio>(1.5);
        assert!(matches!(
            model.heat_transfer_coefficient(&bad_x),
            Err(MultiphaseError::InvalidInput(_))
        ));
    }

    // ── Reserved variants error cleanly (no faked physics) ───────────────────

    /// Every reserved dryout-onset variant returns NotImplemented, and enum
    /// dispatch reaches each one.
    #[test]
    fn reserved_onset_variants_not_implemented() {
        let c = dryout_conditions(0.5);
        for model in [
            DryoutOnsetModel::CriticalHeatFlux,
            DryoutOnsetModel::LiquidFilmDepletion,
        ] {
            assert!(matches!(
                model.dryout_onset(&c),
                Err(MultiphaseError::NotImplemented(_))
            ));
        }
    }

    /// Every reserved post-dryout variant returns NotImplemented, and enum
    /// dispatch reaches each one.
    #[test]
    fn reserved_post_dryout_variants_not_implemented() {
        let c = post_dryout_conditions();
        for model in [
            PostDryoutModel::FilmBoilingMechanistic,
            PostDryoutModel::TransitionBoiling,
            PostDryoutModel::ChfRecovery,
        ] {
            assert!(matches!(
                model.heat_transfer_coefficient(&c),
                Err(MultiphaseError::NotImplemented(_))
            ));
        }
    }
}
