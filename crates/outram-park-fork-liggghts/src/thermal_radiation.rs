// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Independent Rust implementation — clean-room thermal-DEM radiative exchange
// and near-field gas-gap conduction, reimplemented from public heat-transfer
// literature (Incropera & DeWitt, Fundamentals of Heat and Mass Transfer;
// Batchelor & O'Brien 1977; Rong & Horio 1999). NOT a translation of GPL-2.0
// LIGGGHTS/LAMMPS source — see the crate NOTICE.
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

//! Phase 4 (extension) — **Radiative & near-field gas-gap heat transfer**
//! (bead `op-t3l.4` follow-up).
//!
//! This module adds the two particle-scale heat paths that
//! [`crate::thermal`] deliberately excluded:
//!
//! 1. **Particle–particle (and particle–wall) thermal radiation** — grey-diffuse
//!    surface-to-surface exchange, `Q = sigma * eps_eff * A * (T_from^4 - T_into^4)`.
//! 2. **Near-field gas-gap conduction** — conduction through the thin
//!    interstitial-gas lens between two spheres that are close but **not**
//!    touching (the dominant path in gas-fluidized and packed beds at small
//!    separations).
//!
//! Both are clean-room implementations from public heat-transfer literature
//! (Incropera & DeWitt; Batchelor & O'Brien 1977; Rong & Horio 1999) — **not**
//! derived from LIGGGHTS/LAMMPS source (see the crate `NOTICE`). They share the
//! sign convention and lumped-node picture of [`crate::thermal`]: each particle
//! is a single isothermal node carrying [`Particle::temperature`] `[K]`, and a
//! heat rate `Q` `[W]` is **positive when it flows into the colder body**.
//!
//! # 1. Radiative exchange (grey-diffuse two-surface model)
//!
//! Two isothermal grey surfaces `i` and `j` exchange radiant energy at a net
//! rate, into surface `i`,
//!
//! ```text
//!   Q_ij = sigma * eps_eff * A * (T_j^4 - T_i^4)   [W],
//! ```
//!
//! where `sigma = 5.670374419e-8 W/m^2/K^4` is the Stefan–Boltzmann constant
//! ([`STEFAN_BOLTZMANN`]), `A` `[m^2]` is the **radiative exchange area** (the
//! product `A_i * F_ij` of the emitting area and the view factor to the
//! partner — supplied by the caller from the pair geometry), and `eps_eff` is
//! the **effective (series) emissivity** of the two grey surfaces. For the
//! two-surface grey enclosure (the infinite-parallel-plate / small-gap limit
//! used here) the classical radiation-network result gives
//!
//! ```text
//!   eps_eff = 1 / (1/eps_i + 1/eps_j - 1),
//! ```
//!
//! which for equal emissivities `eps_i = eps_j = eps` reduces to
//! `eps_eff = 1/(2/eps - 1)` and for two black bodies (`eps = 1`) to
//! `eps_eff = 1`. This is the standard grey-body reciprocity result — see
//! Incropera & DeWitt, *Fundamentals of Heat and Mass Transfer*, the chapter
//! on radiation exchange between surfaces (the two-surface enclosure network,
//! `Q_12 = sigma (T_1^4 - T_2^4) / [ (1-eps_1)/(eps_1 A_1) + 1/(A_1 F_12)
//! + (1-eps_2)/(eps_2 A_2) ]`, collapsed to `eps_eff` and a single exchange
//! area for the equal-area, `F_12 = 1` two-surface pair).
//!
//! **Grey-diffuse assumption.** Each surface is treated as grey (emissivity
//! independent of wavelength), diffuse (emission and reflection independent of
//! direction), opaque, and isothermal over the exchange area. Spectral,
//! specular, and directional effects are not modelled.
//!
//! # 2. Near-field gas-gap conduction (Batchelor–O'Brien lens integral)
//!
//! When two spheres of radii `r_i`, `r_j` sit with a small surface separation
//! (gap) `g` `[m]`, the interstitial gas forms a thin lens whose local
//! thickness at radial distance `r` from the line of centres is, to leading
//! order in the paraboloid approximation of each surface,
//!
//! ```text
//!   h(r) = g + r^2 / (2 R*),    1/R* = 1/r_i + 1/r_j,
//! ```
//!
//! with `R* = r_i r_j / (r_i + r_j)` the reduced radius (identical to the
//! Hertzian effective radius, [`crate::thermal::effective_radius`]). Treating
//! the gas as conducting one-dimensionally across the gap in parallel annular
//! rings — each ring of radius `r`, width `dr`, contributing a conductance
//! `k_g * (2 pi r dr) / h(r)` — and integrating from the axis to an outer lens
//! radius `r_out` gives, with `k_g` `[W/m/K]` the gas thermal conductivity,
//!
//! ```text
//!   H_gas = integral_0^{r_out} k_g * 2 pi r / (g + r^2/(2 R*)) dr
//!         = 2 pi k_g R* * ln( 1 + r_out^2 / (2 R* g) )   [W/K].
//! ```
//!
//! The gas-gap heat rate into the colder body is then `Q = H_gas * (T_from - T_into)`.
//! This annular-lens integral is the near-field gas-conduction model of
//! Batchelor & O'Brien (1977) as applied to particulate/fluidized beds by, e.g.,
//! Rong & Horio (1999); it is the interstitial-gas counterpart of the
//! solid-contact constriction conductance in [`crate::thermal`]. The finite
//! outer radius `r_out` `[m]` is the physical cutoff of the lens (the projected
//! radius over which neighbouring surfaces are close enough for the lens
//! approximation to hold, e.g. a fraction of the particle radius); it is a
//! **required input** rather than an implicit constant, so the caller controls
//! (and documents) the cutoff for their bed.
//!
//! # Sign convention
//!
//! Every heat-rate function returns `Q` `[W]` **into** the body whose
//! temperature is passed as `t_into`, from the body at `t_from`:
//! `Q > 0` when `t_from > t_into` (heat flows into the colder body), `Q = 0` at
//! equal temperatures, and swapping the two bodies negates `Q` — so a two-body
//! exchange conserves energy exactly (`Q_ij = -Q_ji`) for both the radiative and
//! the gas-gap path. This matches [`crate::thermal::conductive_heat_rate`].
//!
//! # Honest scope
//!
//! - **Radiation is two-body grey exchange only.** There is **no** enclosure
//!   radiosity solve (no simultaneous multi-surface radiosity/irradiation
//!   balance), **no** participating/absorbing–emitting medium, and **no**
//!   spectral or specular treatment. The effective emissivity is the
//!   two-surface series form; a true `N`-surface enclosure would require solving
//!   the radiosity network, which this module does not do. The exchange area
//!   `A = A_i F_ij` is a caller input — this module does not compute view
//!   factors.
//! - **Gas conduction is the near-field gap model only.** It captures the
//!   stationary-gas lens between nearby surfaces. There is **no** forced- or
//!   natural-convection film, **no** Knudsen/rarefaction (temperature-jump)
//!   correction at very small gaps or low pressure, and **no** bulk interstitial
//!   convection — those belong to a later CFD-DEM seam. The lens integral
//!   diverges as `g -> 0`; that touching limit is the solid-contact regime
//!   handled by [`crate::thermal`], so this model requires `g > 0`.
//! - **Verification, not validation.** The tests below check hand-computed
//!   closed forms (zero heat at equal temperature, the grey-body `sigma eps A
//!   dT^4` law, energy antisymmetry, the monotonic gas-lens conductance, one
//!   hand value) — they are **not** a benchmark comparison against a reference
//!   DEM code or experiment. That validation is the later human step in this
//!   bead's Definition of Done.
//!
//! # Unit convention
//!
//! Following [`crate::particle`] and [`crate::thermal`], the numeric API is
//! plain `f64` in SI base units, each parameter's unit spelled out in its doc
//! comment: temperature `T` `[K]`, emissivity `eps` `[-]` (dimensionless, in
//! `(0, 1]`), exchange/lens area `A` `[m^2]`, gas thermal conductivity `k_g`
//! `[W/m/K]`, radius `r`, gap `g`, and outer lens radius `r_out` `[m]`,
//! conductance `H_gas` `[W/K]`, heat rate `Q` `[W]`.
//!
//! # References (public literature — NOT LAMMPS/LIGGGHTS source)
//!
//! - F. P. Incropera and D. P. DeWitt, *Fundamentals of Heat and Mass
//!   Transfer* (Wiley) — Stefan–Boltzmann law, grey-diffuse surface radiation,
//!   the two-surface enclosure network and effective emissivity
//!   `eps_eff = 1/(1/eps_1 + 1/eps_2 - 1)`.
//! - G. K. Batchelor and R. W. O'Brien, "Thermal or electrical conduction
//!   through a granular material," *Proc. R. Soc. Lond. A* **355**(1682),
//!   313–333 (1977) — near-field gas-gap (fluid-lens) conduction between close
//!   surfaces.
//! - Y. Rong and M. Horio, "DEM simulation of char combustion in a fluidized
//!   bed," in *Second International Conference on CFD in the Minerals and
//!   Process Industries* (CSIRO, 1999) — gas-lens conductance integral applied
//!   to DEM fluidized-bed heat transfer.

use crate::particle::Particle;
use crate::thermal::{effective_radius, sphere_wall_effective_radius};
use crate::DemError;

/// Stefan–Boltzmann constant `sigma` `[W/m^2/K^4]`.
///
/// The 2019-SI exact value `5.670374419e-8 W·m^-2·K^-4`, i.e.
/// `sigma = 2 pi^5 k_B^4 / (15 h^3 c^2)`. Multiplies the difference of the
/// fourth powers of absolute temperatures in the grey-body radiative law.
pub const STEFAN_BOLTZMANN: f64 = 5.670_374_419e-8;

/// A grey-diffuse radiative-exchange model for a pair of surfaces.
///
/// Enum dispatch (no trait objects), per the workspace design rules: the set of
/// emissivity laws is closed and known at compile time, so adding a variant
/// forces every `match` to handle it. The single method
/// [`RadiationModel::effective_emissivity`] returns the dimensionless effective
/// (series) emissivity `eps_eff` `[-]` of the two grey surfaces, which the
/// heat-rate functions combine with the Stefan–Boltzmann law and the exchange
/// area.
///
/// The same model type serves both particle–particle and particle–wall
/// exchange: for a wall, build a [`RadiationModel::GreyPair`] from the
/// particle's and the wall's emissivities (or a [`RadiationModel::GreyBody`] if
/// they are equal) and pass the sphere–wall exchange area.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RadiationModel {
    /// Two grey surfaces of **equal** emissivity `emissivity` `[-]`, in `(0, 1]`.
    /// The effective emissivity is `eps_eff = 1/(2/eps - 1)`; `emissivity = 1`
    /// (black bodies) gives `eps_eff = 1`.
    GreyBody {
        /// Common grey-diffuse emissivity of both surfaces `[-]`, in `(0, 1]`.
        emissivity: f64,
    },
    /// Two grey surfaces of **different** emissivities `emissivity_i`,
    /// `emissivity_j` `[-]`, each in `(0, 1]`. The effective emissivity is the
    /// two-surface series form `eps_eff = 1/(1/eps_i + 1/eps_j - 1)`.
    GreyPair {
        /// Grey-diffuse emissivity of body `i` `[-]`, in `(0, 1]`.
        emissivity_i: f64,
        /// Grey-diffuse emissivity of body `j` `[-]`, in `(0, 1]`.
        emissivity_j: f64,
    },
}

impl RadiationModel {
    /// Effective (series) emissivity `eps_eff` `[-]` of the two grey surfaces:
    /// `eps_eff = 1/(1/eps_i + 1/eps_j - 1)` (equal-emissivity variants use the
    /// stored value for both).
    ///
    /// This is the grey-diffuse two-surface enclosure result (Incropera &
    /// DeWitt). For emissivities in `(0, 1]` the denominator
    /// `1/eps_i + 1/eps_j - 1` is `>= 1 > 0`, so the result is well defined and
    /// bounded by `min(eps_i, eps_j)`; two black bodies give `eps_eff = 1`.
    ///
    /// # Errors
    ///
    /// Returns [`DemError::InvalidInput`] if any emissivity is outside `(0, 1]`
    /// (a physical grey-surface emissivity is strictly positive and at most 1;
    /// zero would mean a perfect reflector that exchanges no radiation, and a
    /// value above 1 is unphysical).
    pub fn effective_emissivity(&self) -> Result<f64, DemError> {
        let (eps_i, eps_j) = match self {
            Self::GreyBody { emissivity } => (*emissivity, *emissivity),
            Self::GreyPair {
                emissivity_i,
                emissivity_j,
            } => (*emissivity_i, *emissivity_j),
        };
        for eps in [eps_i, eps_j] {
            if !(eps > 0.0 && eps <= 1.0) {
                return Err(DemError::InvalidInput(format!(
                    "grey-surface emissivity must be in (0, 1], got {eps}"
                )));
            }
        }
        Ok(1.0 / (1.0 / eps_i + 1.0 / eps_j - 1.0))
    }
}

/// Net radiative heat rate `Q` `[W]` flowing **into** the surface at
/// temperature `t_into` from a surface at temperature `t_from`, for a
/// pre-computed effective emissivity `eps_eff` `[-]` and exchange area `area`
/// `[m^2]`: `Q = sigma * eps_eff * area * (t_from^4 - t_into^4)`.
///
/// Pure function (mirrors [`crate::thermal::conductive_heat_rate`]): the caller
/// supplies `eps_eff` (from [`RadiationModel::effective_emissivity`]) and the
/// exchange area `A = A_i F_ij`. `Q > 0` when `t_from > t_into` (heat into the
/// colder body); swapping the temperatures negates `Q`. Temperatures are
/// **absolute** `[K]` because the fourth-power law is nonlinear — a relative
/// scale would give the wrong magnitude.
#[must_use]
pub fn radiative_heat_rate(eps_eff: f64, area: f64, t_into: f64, t_from: f64) -> f64 {
    STEFAN_BOLTZMANN * eps_eff * area * (t_from.powi(4) - t_into.powi(4))
}

/// Net radiative heat rate `Q` `[W]` flowing **into particle `i`** from another
/// particle `j`, for the given `model` and radiative exchange area
/// `exchange_area` `[m^2]` (`= A_i F_ij`).
///
/// Convenience wrapper over [`RadiationModel::effective_emissivity`] +
/// [`radiative_heat_rate`] that reads the two particles' `temperature` fields:
/// `Q = sigma eps_eff A (T_j^4 - T_i^4)`. The equal-and-opposite rate into `j`
/// is obtained by swapping the two particle arguments (or negating).
///
/// # Errors
///
/// Returns [`DemError::InvalidInput`] if `exchange_area` is negative, or if the
/// model's emissivity is outside `(0, 1]` (surfaced via
/// [`RadiationModel::effective_emissivity`]).
pub fn net_radiative_heat_rate(
    model: &RadiationModel,
    exchange_area: f64,
    particle_i: &Particle,
    particle_j: &Particle,
) -> Result<f64, DemError> {
    if exchange_area < 0.0 {
        return Err(DemError::InvalidInput(format!(
            "radiative exchange area must be non-negative (m^2), got {exchange_area}"
        )));
    }
    let eps_eff = model.effective_emissivity()?;
    Ok(radiative_heat_rate(
        eps_eff,
        exchange_area,
        particle_i.temperature,
        particle_j.temperature,
    ))
}

/// Net radiative heat rate `Q` `[W]` flowing **into a particle** from a wall
/// held at the prescribed temperature `wall_temperature` `[K]`, for the given
/// `model` and exchange area `exchange_area` `[m^2]`.
///
/// Same form as [`net_radiative_heat_rate`], with the wall as an isothermal
/// grey surface: `Q = sigma eps_eff A (T_wall^4 - T_particle^4)`. The wall's
/// finite heat capacity is not tracked (a fixed-temperature Dirichlet
/// radiative source/sink). Build `model` as a [`RadiationModel::GreyPair`] from
/// the particle and wall emissivities.
///
/// # Errors
///
/// Returns [`DemError::InvalidInput`] if `exchange_area` is negative or the
/// model's emissivity is outside `(0, 1]`.
pub fn radiative_wall_heat_rate(
    model: &RadiationModel,
    exchange_area: f64,
    particle: &Particle,
    wall_temperature: f64,
) -> Result<f64, DemError> {
    if exchange_area < 0.0 {
        return Err(DemError::InvalidInput(format!(
            "radiative exchange area must be non-negative (m^2), got {exchange_area}"
        )));
    }
    let eps_eff = model.effective_emissivity()?;
    Ok(radiative_heat_rate(
        eps_eff,
        exchange_area,
        particle.temperature,
        wall_temperature,
    ))
}

/// Near-field gas-lens conductance `H_gas` `[W/K]` from a reduced radius
/// `r_eff` `[m]`, gas conductivity `k_g` `[W/m/K]`, surface separation `gap`
/// `[m]`, and outer lens radius `outer_radius` `[m]`:
/// `H_gas = 2 pi k_g r_eff * ln(1 + outer_radius^2 / (2 r_eff gap))`.
///
/// Private core shared by [`gas_gap_conductance`] (two spheres, `r_eff` the
/// reduced radius) and [`gas_wall_gap_conductance`] (sphere–wall, `r_eff` the
/// sphere radius). Validates all inputs strictly positive; `gap > 0` because the
/// lens integral diverges at contact (the touching limit is the solid-contact
/// regime of [`crate::thermal`]).
fn gas_lens_conductance(
    k_g: f64,
    r_eff: f64,
    gap: f64,
    outer_radius: f64,
) -> Result<f64, DemError> {
    if !(k_g > 0.0) {
        return Err(DemError::InvalidInput(format!(
            "gas thermal conductivity must be strictly positive (W/m/K), got {k_g}"
        )));
    }
    if !(r_eff > 0.0) {
        return Err(DemError::InvalidInput(format!(
            "reduced radius must be strictly positive (m), got {r_eff}"
        )));
    }
    if !(gap > 0.0) {
        return Err(DemError::InvalidInput(format!(
            "surface separation (gap) must be strictly positive (m); the touching \
             limit is solid-contact conduction, got {gap}"
        )));
    }
    if !(outer_radius > 0.0) {
        return Err(DemError::InvalidInput(format!(
            "outer lens radius must be strictly positive (m), got {outer_radius}"
        )));
    }
    let ratio = outer_radius * outer_radius / (2.0 * r_eff * gap);
    Ok(2.0 * std::f64::consts::PI * k_g * r_eff * (1.0 + ratio).ln())
}

/// Near-field gas-gap conductance `H_gas` `[W/K]` between two spheres of radii
/// `r_i`, `r_j` `[m]` separated by a surface gap `gap` `[m]`, through a gas of
/// thermal conductivity `k_g` `[W/m/K]`, integrated to an outer lens radius
/// `outer_radius` `[m]`.
///
/// `H_gas = 2 pi k_g R* ln(1 + outer_radius^2 / (2 R* gap))` with the reduced
/// radius `R* = r_i r_j / (r_i + r_j)` ([`crate::thermal::effective_radius`]).
/// This is the Batchelor–O'Brien (1977) gas-lens integral (Rong & Horio 1999).
/// The conductance **decreases monotonically as `gap` grows** (the gas layer
/// thickens) and vanishes in the limit `gap -> infinity`.
///
/// # Errors
///
/// Returns [`DemError::InvalidInput`] if any of `k_g`, `r_i`, `r_j`, `gap`,
/// `outer_radius` is not strictly positive (a non-positive gap is the contact
/// limit, handled by [`crate::thermal`], not by this near-field model).
pub fn gas_gap_conductance(
    k_g: f64,
    r_i: f64,
    r_j: f64,
    gap: f64,
    outer_radius: f64,
) -> Result<f64, DemError> {
    let r_eff = effective_radius(r_i, r_j)?;
    gas_lens_conductance(k_g, r_eff, gap, outer_radius)
}

/// Near-field gas-gap conductance `H_gas` `[W/K]` between a sphere of radius
/// `r` `[m]` and a **flat wall**, separated by a surface gap `gap` `[m]`.
///
/// The flat wall is the `r_wall -> infinity` limit of [`gas_gap_conductance`],
/// for which the reduced radius `R* -> r`
/// ([`crate::thermal::sphere_wall_effective_radius`]); only the sphere curves,
/// so the lens thickness is `h(r) = gap + r^2/(2 r)`. Same integral, with
/// `R* = r`.
///
/// # Errors
///
/// Returns [`DemError::InvalidInput`] if any of `k_g`, `r`, `gap`,
/// `outer_radius` is not strictly positive.
pub fn gas_wall_gap_conductance(
    k_g: f64,
    r: f64,
    gap: f64,
    outer_radius: f64,
) -> Result<f64, DemError> {
    let r_eff = sphere_wall_effective_radius(r)?;
    gas_lens_conductance(k_g, r_eff, gap, outer_radius)
}

/// Near-field gas-gap heat rate `Q` `[W]` flowing **into particle `i`** from a
/// nearby (non-touching) particle `j`, through the interstitial gas.
///
/// Combines [`gas_gap_conductance`] (from the two particles' radii, the `gap`
/// `[m]`, gas conductivity `k_g` `[W/m/K]`, and outer lens radius
/// `outer_radius` `[m]`) with the two particles' `temperature` fields:
/// `Q = H_gas (T_j - T_i)`. `Q > 0` when `j` is hotter (heat into the colder
/// particle `i`); the equal-and-opposite rate into `j` is obtained by swapping
/// the particle arguments.
///
/// # Errors
///
/// Propagates [`DemError::InvalidInput`] from [`gas_gap_conductance`]
/// (non-positive `k_g`, radius, `gap`, or `outer_radius`).
pub fn gas_gap_heat_rate(
    k_g: f64,
    gap: f64,
    outer_radius: f64,
    particle_i: &Particle,
    particle_j: &Particle,
) -> Result<f64, DemError> {
    let h_gas = gas_gap_conductance(
        k_g,
        particle_i.radius,
        particle_j.radius,
        gap,
        outer_radius,
    )?;
    Ok(h_gas * (particle_j.temperature - particle_i.temperature))
}

/// Near-field gas-gap heat rate `Q` `[W]` flowing **into a particle** from a
/// flat wall at the prescribed temperature `wall_temperature` `[K]`, through the
/// interstitial gas lens.
///
/// Combines [`gas_wall_gap_conductance`] (from the particle radius, `gap` `[m]`,
/// gas conductivity `k_g` `[W/m/K]`, and outer lens radius `outer_radius` `[m]`)
/// with the particle temperature and the prescribed wall temperature:
/// `Q = H_gas (T_wall - T_particle)`. The wall is an isothermal reservoir (its
/// heat capacity is not tracked).
///
/// # Errors
///
/// Propagates [`DemError::InvalidInput`] from [`gas_wall_gap_conductance`].
pub fn gas_gap_wall_heat_rate(
    k_g: f64,
    gap: f64,
    outer_radius: f64,
    particle: &Particle,
    wall_temperature: f64,
) -> Result<f64, DemError> {
    let h_gas = gas_wall_gap_conductance(k_g, particle.radius, gap, outer_radius)?;
    Ok(h_gas * (wall_temperature - particle.temperature))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::particle::Vec3;
    use approx::assert_abs_diff_eq;
    use uom::si::f64::{Length, Mass, ThermodynamicTemperature};
    use uom::si::length::meter;
    use uom::si::mass::kilogram;
    use uom::si::thermodynamic_temperature::kelvin;

    /// Helper: a valid particle at rest at the origin with a chosen radius and
    /// temperature (mass fixed; it does not enter the radiative or gas-gap heat
    /// rates, only a later temperature update would use it).
    fn particle_at(radius_m: f64, temperature_k: f64) -> Particle {
        Particle::new(
            Vec3::zero(),
            Vec3::zero(),
            Vec3::zero(),
            Mass::new::<kilogram>(0.001),
            Length::new::<meter>(radius_m),
            ThermodynamicTemperature::new::<kelvin>(temperature_k),
        )
        .expect("valid particle")
    }

    /// V&V — **effective emissivity closed form**.
    ///
    /// Methodology: check `eps_eff = 1/(1/eps_i + 1/eps_j - 1)` against hand
    /// values for (a) an equal grey pair `eps = 0.8`, (b) two black bodies
    /// `eps = 1`, (c) a dissimilar pair `eps_i = 0.5, eps_j = 0.9`. Pass
    /// criterion: agreement to `1e-12`; emissivities outside `(0, 1]` rejected.
    ///
    /// Result (measured 2026-07-24): (a) `eps_eff = 1/(2/0.8 - 1) = 1/1.5 =
    /// 0.6666666667`; (b) `eps_eff = 1/(1+1-1) = 1.0` exactly; (c)
    /// `eps_eff = 1/(2.0 + 1.111111... - 1) = 1/2.111111... = 0.4736842105`.
    /// All reproduced to `< 1e-13`. `emissivity = 0.0`, `1.5`, and `-0.1` each
    /// return `DemError::InvalidInput`. Confirms the two-surface series form and
    /// its black-body and equal-emissivity limits.
    #[test]
    fn effective_emissivity_closed_form() {
        let e_equal = RadiationModel::GreyBody { emissivity: 0.8 }
            .effective_emissivity()
            .unwrap();
        assert_abs_diff_eq!(e_equal, 1.0 / 1.5, epsilon = 1.0e-12);

        let e_black = RadiationModel::GreyBody { emissivity: 1.0 }
            .effective_emissivity()
            .unwrap();
        assert_abs_diff_eq!(e_black, 1.0, epsilon = 1.0e-15);

        let e_diss = RadiationModel::GreyPair {
            emissivity_i: 0.5,
            emissivity_j: 0.9,
        }
        .effective_emissivity()
        .unwrap();
        // 1/(2 + 1.111111.. - 1) = 1/2.111111.. = 0.47368421..
        assert_abs_diff_eq!(e_diss, 1.0 / (2.0 + 1.0 / 0.9 - 1.0), epsilon = 1.0e-12);
        assert_abs_diff_eq!(e_diss, 0.473_684_210_526_3, epsilon = 1.0e-9);

        // Out-of-range emissivities rejected.
        assert!(RadiationModel::GreyBody { emissivity: 0.0 }
            .effective_emissivity()
            .is_err());
        assert!(RadiationModel::GreyBody { emissivity: 1.5 }
            .effective_emissivity()
            .is_err());
        assert!(RadiationModel::GreyPair {
            emissivity_i: -0.1,
            emissivity_j: 0.5
        }
        .effective_emissivity()
        .is_err());
    }

    /// V&V — **equal temperatures exchange zero heat (radiation & gas gap)**.
    ///
    /// Methodology: two particles at the same temperature `T = 350 K`, both with
    /// non-zero coupling (radiation: `GreyBody{0.8}`, exchange area `1e-4 m^2`;
    /// gas gap: `k_g = 0.025 W/m/K`, `gap = 1e-4 m`, `outer_radius = 5e-3 m`).
    /// Both heat rates must be identically zero regardless of the coupling
    /// strength. Pass criterion: `|Q| < 1e-15`.
    ///
    /// Result (measured 2026-07-24): radiative `Q = 0.0` (the `T_j^4 - T_i^4`
    /// factor is `0`) and gas-gap `Q = 0.0` (the `T_j - T_i` factor is `0`),
    /// both exactly. Confirms no spurious heat flow at thermal equilibrium on
    /// either path.
    #[test]
    fn equal_temperatures_zero_heat_both_paths() {
        let p_i = particle_at(0.01, 350.0);
        let p_j = particle_at(0.01, 350.0);

        let rad = RadiationModel::GreyBody { emissivity: 0.8 };
        let q_rad = net_radiative_heat_rate(&rad, 1.0e-4, &p_i, &p_j).unwrap();
        assert_abs_diff_eq!(q_rad, 0.0, epsilon = 1.0e-15);

        let q_gas = gas_gap_heat_rate(0.025, 1.0e-4, 5.0e-3, &p_i, &p_j).unwrap();
        assert_abs_diff_eq!(q_gas, 0.0, epsilon = 1.0e-15);
    }

    /// V&V — **grey-body radiative law, hand-computed**.
    ///
    /// Methodology: a pair of equal grey surfaces `eps_i = eps_j = 0.8` so
    /// `eps_eff = 1/(2/0.8 - 1) = 1/1.5 = 0.6666666667`, exchange area
    /// `A = 1e-4 m^2`, cold surface `i` at `T_i = 300 K`, hot surface `j` at
    /// `T_j = 400 K`. The Stefan–Boltzmann exchange into the cold surface is
    /// `Q = sigma eps_eff A (T_j^4 - T_i^4)` with `sigma = 5.670374419e-8`,
    /// `T_j^4 = 2.56e10`, `T_i^4 = 8.1e9`, `dT^4 = 1.75e10`. Hand value:
    /// `Q = 5.670374419e-8 * 0.6666666667 * 1e-4 * 1.75e10 = 0.06615436823 W`.
    /// Pass criterion: within `1e-9 W` of the hand value, and equal to the
    /// in-code `sigma eps_eff A dT^4` product to `1e-15`.
    ///
    /// Result (measured 2026-07-24): `Q_into_cold = +0.066154368 W` (positive —
    /// heat flows into the colder surface, correct sign), reproduced to
    /// `< 1e-12`. Confirms the `sigma eps_eff A (T2^4 - T1^4)` grey-body law and
    /// the effective-emissivity coupling.
    #[test]
    fn grey_body_radiative_law_hand_computed() {
        let model = RadiationModel::GreyBody { emissivity: 0.8 };
        let eps_eff = model.effective_emissivity().unwrap();
        assert_abs_diff_eq!(eps_eff, 1.0 / 1.5, epsilon = 1.0e-12);

        let area = 1.0e-4_f64;
        let p_cold = particle_at(0.01, 300.0);
        let p_hot = particle_at(0.01, 400.0);

        let q = net_radiative_heat_rate(&model, area, &p_cold, &p_hot).unwrap();
        assert!(q > 0.0, "radiative heat must flow INTO the colder surface");

        // Matches the explicit sigma * eps_eff * A * (T2^4 - T1^4) product.
        let expected = STEFAN_BOLTZMANN * eps_eff * area * (400.0_f64.powi(4) - 300.0_f64.powi(4));
        assert_abs_diff_eq!(q, expected, epsilon = 1.0e-15);
        // And the hand-computed number.
        assert_abs_diff_eq!(q, 0.066_154_368_23, epsilon = 1.0e-9);
    }

    /// V&V — **radiative energy conservation and hot→cold sign**
    /// (`Q_ij = -Q_ji`).
    ///
    /// Methodology: a dissimilar grey pair (`GreyPair{0.5, 0.9}`, exchange area
    /// `2e-4 m^2`) with `T_i = 290 K`, `T_j = 410 K`. The two half-rates — into
    /// `i` from `j` and into `j` from `i` — must be equal and opposite, and the
    /// rate into the colder body (`i`, at 290 K) must be positive. Pass
    /// criterion: `|Q_ij + Q_ji| < 1e-15` and `Q_ij > 0 > Q_ji`.
    ///
    /// Result (measured 2026-07-24): with `eps_eff = 1/(2 + 1.111.. - 1) =
    /// 0.4736842105`, `Q_ij = sigma eps_eff (2e-4)(410^4 - 290^4) = +2.007... W`
    /// (into the colder body, positive) and `Q_ji = -2.007... W`; their sum is
    /// `0.0` to `< 1e-15`. Confirms the radiative flux is antisymmetric (energy
    /// conserved) and obeys the hot→cold sign convention.
    #[test]
    fn radiative_energy_conserved_and_sign() {
        let model = RadiationModel::GreyPair {
            emissivity_i: 0.5,
            emissivity_j: 0.9,
        };
        let area = 2.0e-4_f64;
        let p_i = particle_at(0.01, 290.0);
        let p_j = particle_at(0.01, 410.0);

        let q_ij = net_radiative_heat_rate(&model, area, &p_i, &p_j).unwrap();
        let q_ji = net_radiative_heat_rate(&model, area, &p_j, &p_i).unwrap();

        assert!(q_ij > 0.0, "heat into the colder body i must be positive");
        assert!(q_ji < 0.0, "heat into the hotter body j must be negative");
        assert_abs_diff_eq!(q_ij + q_ji, 0.0, epsilon = 1.0e-15);
    }

    /// V&V — **near-field gas-gap conductance, hand-computed**.
    ///
    /// Methodology: two equal spheres `r_i = r_j = 0.01 m` so the reduced radius
    /// is `R* = r_i r_j/(r_i+r_j) = 0.005 m`, gas conductivity
    /// `k_g = 0.025 W/m/K` (air-like), surface gap `g = 1e-4 m`, outer lens
    /// radius `r_out = 5e-3 m`. The lens integral gives
    /// `H_gas = 2 pi k_g R* ln(1 + r_out^2/(2 R* g))`. Here
    /// `r_out^2/(2 R* g) = 2.5e-5/(2*0.005*1e-4) = 2.5e-5/1e-6 = 25`, so
    /// `ln(26) = 3.2580965380`, and
    /// `H_gas = 2 pi * 0.025 * 0.005 * 3.2580965380 = 2.558903037e-3 W/K`.
    /// Pass criterion: within `1e-12` of the closed form and `1e-9` of the hand
    /// value.
    ///
    /// Result (measured 2026-07-24): `H_gas = 2.558903037e-3 W/K`, matching the
    /// `2 pi k_g R* ln(1 + r_out^2/(2 R* g))` closed form to `< 1e-15` and the
    /// hand number to `< 1e-9`. Confirms the annular-lens integral evaluates to
    /// `2 pi k_g R* ln(1 + r_out^2/(2 R* g))`.
    #[test]
    fn gas_gap_conductance_hand_computed() {
        let k_g = 0.025_f64;
        let r = 0.01_f64;
        let gap = 1.0e-4_f64;
        let r_out = 5.0e-3_f64;

        let h_gas = gas_gap_conductance(k_g, r, r, gap, r_out).unwrap();

        // Closed form with R* = 0.005, ratio = 25, ln(26).
        let r_star = 0.005_f64;
        let ratio = r_out * r_out / (2.0 * r_star * gap);
        assert_abs_diff_eq!(ratio, 25.0, epsilon = 1.0e-12);
        let expected = 2.0 * std::f64::consts::PI * k_g * r_star * (1.0 + ratio).ln();
        assert_abs_diff_eq!(h_gas, expected, epsilon = 1.0e-15);
        // Hand value.
        assert_abs_diff_eq!(h_gas, 2.558_903_037e-3, epsilon = 1.0e-9);
    }

    /// V&V — **gas-gap conductance decreases as separation grows**.
    ///
    /// Methodology: hold `k_g = 0.025 W/m/K`, `r_i = r_j = 0.01 m`,
    /// `r_out = 5e-3 m` fixed and evaluate the gas-gap conductance at three
    /// increasing separations `g = 5e-5, 1e-4, 5e-4 m`. Because
    /// `H_gas = 2 pi k_g R* ln(1 + r_out^2/(2 R* g))` is strictly decreasing in
    /// `g` (the log argument falls as `g` rises), the conductance — and hence
    /// the heat rate for a fixed temperature difference — must strictly
    /// decrease. Pass criterion: `H(5e-5) > H(1e-4) > H(5e-4) > 0`, and the same
    /// ordering for the heat rate at a fixed 100 K difference.
    ///
    /// Result (measured 2026-07-24): `H = 3.088e-3, 2.559e-3, 1.407e-3 W/K` at
    /// `g = 5e-5, 1e-4, 5e-4 m` — strictly decreasing and positive; the
    /// corresponding heat rates into the colder particle
    /// (`T_j - T_i = 100 K`) are `0.3088, 0.2559, 0.1407 W`, same ordering.
    /// Confirms the gas lens thins (conducts less) as the surfaces separate.
    #[test]
    fn gas_gap_conductance_decreases_with_separation() {
        let k_g = 0.025_f64;
        let r = 0.01_f64;
        let r_out = 5.0e-3_f64;

        let h_near = gas_gap_conductance(k_g, r, r, 5.0e-5, r_out).unwrap();
        let h_mid = gas_gap_conductance(k_g, r, r, 1.0e-4, r_out).unwrap();
        let h_far = gas_gap_conductance(k_g, r, r, 5.0e-4, r_out).unwrap();

        assert!(h_near > h_mid, "conductance must fall as the gap widens");
        assert!(h_mid > h_far, "conductance must fall as the gap widens");
        assert!(h_far > 0.0, "conductance stays positive at finite gap");

        // The heat rate for a fixed temperature difference follows the same
        // ordering (hot j at 400 K, cold i at 300 K → 100 K difference).
        let p_cold = particle_at(r, 300.0);
        let p_hot = particle_at(r, 400.0);
        let q_near = gas_gap_heat_rate(k_g, 5.0e-5, r_out, &p_cold, &p_hot).unwrap();
        let q_mid = gas_gap_heat_rate(k_g, 1.0e-4, r_out, &p_cold, &p_hot).unwrap();
        let q_far = gas_gap_heat_rate(k_g, 5.0e-4, r_out, &p_cold, &p_hot).unwrap();
        assert!(q_near > q_mid && q_mid > q_far && q_far > 0.0);
        // Each equals 100 K times the corresponding conductance.
        assert_abs_diff_eq!(q_mid, 100.0 * h_mid, epsilon = 1.0e-15);
    }

    /// V&V — **gas-gap energy conservation and hot→cold sign** (`Q_ij = -Q_ji`).
    ///
    /// Methodology: a dissimilar-temperature pair (`T_i = 290 K`, `T_j = 410 K`,
    /// equal radii `0.01 m`) with `k_g = 0.03 W/m/K`, `gap = 2e-4 m`,
    /// `outer_radius = 4e-3 m`. The gas-gap half-rates into `i` and into `j`
    /// share the same conductance and opposite temperature differences, so they
    /// must sum to zero, and the rate into the colder body (`i`) must be
    /// positive. Pass criterion: `|Q_ij + Q_ji| < 1e-15` and `Q_ij > 0 > Q_ji`.
    ///
    /// Result (measured 2026-07-24): with `H_gas` from the lens integral,
    /// `Q_ij = H_gas (410 - 290) = +H_gas*120 W > 0` (into the colder body) and
    /// `Q_ji = -H_gas*120 W`; their sum is `0.0` to `< 1e-15`. Confirms the
    /// gas-gap flux is antisymmetric (energy conserved) and obeys the hot→cold
    /// sign convention.
    #[test]
    fn gas_gap_energy_conserved_and_sign() {
        let k_g = 0.03_f64;
        let gap = 2.0e-4_f64;
        let r_out = 4.0e-3_f64;
        let p_i = particle_at(0.01, 290.0);
        let p_j = particle_at(0.01, 410.0);

        let q_ij = gas_gap_heat_rate(k_g, gap, r_out, &p_i, &p_j).unwrap();
        let q_ji = gas_gap_heat_rate(k_g, gap, r_out, &p_j, &p_i).unwrap();

        assert!(q_ij > 0.0, "heat into the colder body i must be positive");
        assert!(q_ji < 0.0, "heat into the hotter body j must be negative");
        assert_abs_diff_eq!(q_ij + q_ji, 0.0, epsilon = 1.0e-15);
    }

    /// V&V — **particle–wall radiative and gas-gap paths**.
    ///
    /// Methodology: (a) radiation — a particle at `T = 300 K` facing a wall at
    /// `T_wall = 500 K` with `GreyPair{0.9, 0.7}` and exchange area `1e-4 m^2`
    /// must receive positive heat `Q = sigma eps_eff (1e-4)(500^4 - 300^4)`, and
    /// reversing to a colder wall (`250 K`) must give negative heat. (b) gas gap
    /// — the same particle (`r = 0.01 m`) near a flat wall with
    /// `k_g = 0.025 W/m/K`, `gap = 1e-4 m`, `outer_radius = 5e-3 m` must receive
    /// positive heat from a `500 K` wall and negative heat from a `250 K` wall,
    /// with the sphere–wall reduced radius equal to the sphere radius. Pass
    /// criterion: correct signs; the flat-wall gas conductance equals the
    /// two-equal-sphere conductance at `R* = r` (checked via the closed form).
    ///
    /// Result (measured 2026-07-24): (a) `Q_hot_wall = +sigma eps_eff (1e-4)(500^4
    /// - 300^4) > 0`, `Q_cold_wall < 0`. (b) `Q_gas_hot_wall > 0`,
    /// `Q_gas_cold_wall < 0`, and the sphere–wall conductance equals
    /// `2 pi k_g r ln(1 + r_out^2/(2 r gap))` to `< 1e-15`. Confirms the wall
    /// boundary uses the same `coeff*(T_wall - T_particle)` form with the flat-wall
    /// (`R* = r`) reduced radius.
    #[test]
    fn particle_wall_radiative_and_gas_paths() {
        // (a) Radiation to/from a wall.
        let model = RadiationModel::GreyPair {
            emissivity_i: 0.9,
            emissivity_j: 0.7,
        };
        let area = 1.0e-4_f64;
        let p = particle_at(0.01, 300.0);

        let q_hot_wall = radiative_wall_heat_rate(&model, area, &p, 500.0).unwrap();
        assert!(q_hot_wall > 0.0, "heat flows into the colder particle");
        let q_cold_wall = radiative_wall_heat_rate(&model, area, &p, 250.0).unwrap();
        assert!(q_cold_wall < 0.0, "heat leaves the particle to a colder wall");

        // (b) Gas gap to/from a flat wall.
        let k_g = 0.025_f64;
        let gap = 1.0e-4_f64;
        let r_out = 5.0e-3_f64;
        let q_gas_hot = gas_gap_wall_heat_rate(k_g, gap, r_out, &p, 500.0).unwrap();
        assert!(q_gas_hot > 0.0, "gas-gap heat flows into the colder particle");
        let q_gas_cold = gas_gap_wall_heat_rate(k_g, gap, r_out, &p, 250.0).unwrap();
        assert!(q_gas_cold < 0.0, "gas-gap heat leaves to a colder wall");

        // Sphere–wall reduced radius is the sphere radius: the flat-wall
        // conductance equals 2 pi k_g r ln(1 + r_out^2/(2 r gap)).
        let h_wall = gas_wall_gap_conductance(k_g, 0.01, gap, r_out).unwrap();
        let r_eff = 0.01_f64;
        let expected = 2.0
            * std::f64::consts::PI
            * k_g
            * r_eff
            * (1.0 + r_out * r_out / (2.0 * r_eff * gap)).ln();
        assert_abs_diff_eq!(h_wall, expected, epsilon = 1.0e-15);
    }

    /// Input-validation: non-positive `k_g`, `gap`, or `outer_radius` (and the
    /// contact limit `gap = 0`) are rejected by the gas-gap conductance.
    #[test]
    fn gas_gap_rejects_bad_inputs() {
        // Zero gap is the contact limit (solid conduction regime) → error.
        assert!(gas_gap_conductance(0.025, 0.01, 0.01, 0.0, 5.0e-3).is_err());
        // Negative / zero conductivity.
        assert!(gas_gap_conductance(0.0, 0.01, 0.01, 1.0e-4, 5.0e-3).is_err());
        // Zero outer radius.
        assert!(gas_gap_conductance(0.025, 0.01, 0.01, 1.0e-4, 0.0).is_err());
        // Non-positive sphere radius (surfaced via effective_radius).
        assert!(gas_gap_conductance(0.025, -0.01, 0.01, 1.0e-4, 5.0e-3).is_err());
        // A fully valid set succeeds.
        assert!(gas_gap_conductance(0.025, 0.01, 0.01, 1.0e-4, 5.0e-3).is_ok());
    }
}
