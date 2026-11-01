// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! **Phasic wall heat** for the 1-D two-phase solvers (gh:#1062, RELAP-7
//! Theory Manual Eq. 201; the manual is not public, so only the equation
//! number is cited).
//!
//! # What belongs here
//!
//! The heat that crosses the pipe wall into each phase, as a volumetric
//! source \[W/m³\] for [`super::drift_flux::DriftFlux1d`] (one mixture energy
//! equation) and [`super::two_fluid::TwoFluid1d`] (one energy equation per
//! phase). The wall boundary condition itself is **reused**:
//! [`tampines_steam_tables::hem::WallHeat`] (`Adiabatic`, `HeatFlux`,
//! `WallTemperature` with a given `h`) is the same type `HemPipe` takes, and
//! [`PhasicWallHeat::Relap5WallTemperature`] takes `h` from the RELAP5
//! geometry-101/110 dispatcher
//! [`crate::heat_transfer::relap5_htc::relap5_wall_heat_transfer`] (gh:#1063)
//! from the phase's own properties.
//!
//! # The phasic split, and which partition it is
//!
//! The wall area per unit volume of a circular pipe is `4 / D_h`. Each phase
//! receives the heat through the fraction of the wall it wets, which is the
//! **linear contact partition** of the reused GeN-Foam closure
//! [`outram_foam_appbuilder_lib::genfoam::thermal_hydraulics::closures::interfacial::contact_partition::ContactPartition`]:
//! the vapour wets `α_g` of the wall and the liquid (`Complementary`)
//! `1 − α_g`. gh:#1060 brought a regime map for the *interfacial area*, not a
//! wall-contact map, so the linear partition is what is used; a boiling-curve
//! or film-boiling partition is **not** modelled.
//!
//! - `HeatFlux(q'')`: `Q_k = c_k q'' 4/D_h`; the two shares sum to the
//!   prescribed heat exactly.
//! - `WallTemperature { T_w, h }`: `Q_k = c_k h (T_w − T_k) 4/D_h`, each phase
//!   against its own temperature.
//! - `Relap5WallTemperature { T_w, geometry }`: as above with `h_k` from the
//!   RELAP5 dispatcher evaluated on phase `k`'s density, viscosity,
//!   conductivity, `c_p`, thermal expansion and velocity. Single-phase
//!   convection correlations per phase — **no nucleate-boiling or CHF
//!   logic**, which the dispatcher does not carry.
//!
//! # Default
//!
//! [`PhasicWallHeat::default`] is adiabatic, like `HemPipe`'s: wall heat is a
//! boundary input, not physics a closure supplies, so "physics on by default"
//! does not apply to it.
//!
//! # Honest scope
//!
//! Explicit in the phase temperature (evaluated on the start-of-step state),
//! so a very large `h Δt / (ρ c_p D_h)` can overshoot; nothing here limits
//! it. Not validated against any experiment; the V&V record is
//! `crates/tampines/verification_and_validation/multiphase_1d_issues_1058_1060_1062_1076.md`.

use outram_foam_appbuilder_lib::genfoam::thermal_hydraulics::closures::interfacial::contact_partition::ContactPartition;
use tampines_steam_tables::hem::WallHeat;
use uom::si::acceleration::meter_per_second_squared;
use uom::si::dynamic_viscosity::pascal_second;
use uom::si::f64::{
    Acceleration, DynamicViscosity, Length, MassDensity, Ratio, SpecificHeatCapacity,
    TemperatureCoefficient, ThermalConductivity, ThermodynamicTemperature, Velocity,
};
use uom::si::heat_flux_density::watt_per_square_meter;
use uom::si::heat_transfer::watt_per_square_meter_kelvin;
use uom::si::length::meter;
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::ratio::ratio;
use uom::si::specific_heat_capacity::joule_per_kilogram_kelvin;
use uom::si::temperature_coefficient::per_kelvin;
use uom::si::thermal_conductivity::watt_per_meter_kelvin;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::velocity::meter_per_second;

use crate::heat_transfer::relap5_htc::{
    relap5_wall_heat_transfer, Relap5HeatTransferGeometry, Relap5WallConvectionInputs,
};
use super::properties::{PhaseState, SaturatedProperties};
use crate::TampinesError;

/// The wall heat boundary condition of a 1-D two-phase pipe (gh:#1062).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PhasicWallHeat {
    /// The reused `HemPipe` wall condition: adiabatic, a prescribed flux
    /// \[W/m² of wall\], or a wall temperature with a given `h`.
    Hem(WallHeat),
    /// A wall at `temperature`, with each phase's `h` from the RELAP5
    /// geometry-101/110 dispatcher on that phase's own properties.
    Relap5WallTemperature {
        /// Wall temperature.
        temperature: ThermodynamicTemperature,
        /// RELAP5 heat-transfer geometry (101 pipe, 110 rod bundle).
        geometry: Relap5HeatTransferGeometry,
    },
}

impl Default for PhasicWallHeat {
    fn default() -> Self {
        Self::Hem(WallHeat::Adiabatic)
    }
}

/// One phase's state as the wall sees it. Raw SI.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WallPhaseState {
    /// Bulk temperature \[K\].
    pub temperature: f64,
    /// Density \[kg/m³\].
    pub density: f64,
    /// Dynamic viscosity \[Pa·s\].
    pub viscosity: f64,
    /// Thermal conductivity \[W/(m·K)\].
    pub conductivity: f64,
    /// Isobaric specific heat \[J/(kg·K)\].
    pub cp: f64,
    /// Volumetric thermal expansion `β = −(1/ρ) ∂ρ/∂T|_p` \[1/K\].
    pub expansion: f64,
    /// Axial velocity \[m/s\].
    pub velocity: f64,
}

impl PhasicWallHeat {
    /// `true` for the adiabatic wall, which lets a solver skip all wall-heat
    /// work and keep its pre-gh:#1062 arithmetic bit for bit.
    #[must_use]
    pub fn is_adiabatic(&self) -> bool {
        matches!(self, Self::Hem(WallHeat::Adiabatic))
    }

    /// `true` when evaluating this wall needs the phases' conductivity, `c_p`
    /// and thermal expansion (the RELAP5 variant); the other variants read
    /// temperatures only, so a solver can skip that property work.
    #[must_use]
    pub fn needs_properties(&self) -> bool {
        matches!(self, Self::Relap5WallTemperature { .. })
    }

    /// Refuse a non-finite flux, temperature or `h`.
    ///
    /// # Errors
    ///
    /// [`TampinesError::InvalidInput`].
    pub fn validate(&self) -> Result<(), TampinesError> {
        let ok = match *self {
            Self::Hem(WallHeat::Adiabatic) => true,
            Self::Hem(WallHeat::HeatFlux(q)) => q.get::<watt_per_square_meter>().is_finite(),
            Self::Hem(WallHeat::WallTemperature {
                temperature,
                heat_transfer_coefficient,
            }) => {
                let h = heat_transfer_coefficient.get::<watt_per_square_meter_kelvin>();
                temperature.get::<kelvin>() > 0.0 && h.is_finite() && h >= 0.0
            }
            Self::Relap5WallTemperature { temperature, .. } => {
                temperature.get::<kelvin>().is_finite() && temperature.get::<kelvin>() > 0.0
            }
        };
        if ok {
            Ok(())
        } else {
            Err(TampinesError::InvalidInput(format!(
                "wall heat {self:?} needs a finite flux, a positive wall temperature and h >= 0"
            )))
        }
    }

    /// Volumetric wall heat into `(vapour, liquid)` \[W/m³\] for a cell of
    /// vapour fraction `void`, hydraulic diameter `d_h` \[m\] and the two
    /// phase states. See the module docs for the split.
    ///
    /// # Errors
    ///
    /// As [`relap5_wall_heat_transfer`] for the RELAP5 variant.
    pub fn phasic_heat(
        &self,
        void: f64,
        d_h: f64,
        vapour: WallPhaseState,
        liquid: WallPhaseState,
    ) -> Result<(f64, f64), TampinesError> {
        let a_w = 4.0 / d_h;
        let c_g = ContactPartition::Linear
            .value(Ratio::new::<ratio>(void.clamp(0.0, 1.0)))
            .get::<ratio>();
        let c_l = ContactPartition::Complementary
            .value(Ratio::new::<ratio>(c_g))
            .get::<ratio>();
        match *self {
            Self::Hem(WallHeat::Adiabatic) => Ok((0.0, 0.0)),
            Self::Hem(WallHeat::HeatFlux(q)) => {
                let q = q.get::<watt_per_square_meter>() * a_w;
                Ok((c_g * q, c_l * q))
            }
            Self::Hem(WallHeat::WallTemperature {
                temperature,
                heat_transfer_coefficient,
            }) => {
                let t_w = temperature.get::<kelvin>();
                let h = heat_transfer_coefficient.get::<watt_per_square_meter_kelvin>();
                Ok((
                    c_g * h * (t_w - vapour.temperature) * a_w,
                    c_l * h * (t_w - liquid.temperature) * a_w,
                ))
            }
            Self::Relap5WallTemperature {
                temperature,
                geometry,
            } => {
                let t_w = temperature.get::<kelvin>();
                let h_g = relap5_h(geometry, t_w, d_h, vapour)?;
                let h_l = relap5_h(geometry, t_w, d_h, liquid)?;
                Ok((
                    c_g * h_g * (t_w - vapour.temperature) * a_w,
                    c_l * h_l * (t_w - liquid.temperature) * a_w,
                ))
            }
        }
    }
}

/// Volumetric thermal expansion `β = −(1/ρ) ∂ρ/∂T|_p` \[1/K\] of one phase
/// at `(p, h)`, by a one-sided `+1 K` step in enthalpy (`δh = c_p · 1 K`)
/// through the same metastable-aware [`PhaseState`] the solvers use. Only
/// the RELAP5 variant reads it (its Grashof number).
///
/// # Errors
///
/// As [`PhaseState::liquid_at`] / [`PhaseState::vapour_at`].
pub(crate) fn thermal_expansion(
    liquid: bool,
    p: f64,
    h: f64,
    cp: f64,
    sat: SaturatedProperties,
) -> Result<f64, TampinesError> {
    let at = |h: f64| {
        if liquid {
            PhaseState::liquid_at(p, h, sat)
        } else {
            PhaseState::vapour_at(p, h, sat)
        }
    };
    let here = at(h)?;
    let hot = at(h + cp)?;
    Ok(-(hot.density - here.density) / here.density)
}

/// The RELAP5 dispatcher's `h` \[W/(m²·K)\] for one phase against a wall at
/// `t_w` \[K\]; gravity is standard `g`, the free-convection length the
/// hydraulic diameter (the dispatcher's own default).
fn relap5_h(
    geometry: Relap5HeatTransferGeometry,
    t_w: f64,
    d_h: f64,
    phase: WallPhaseState,
) -> Result<f64, TampinesError> {
    let inputs = Relap5WallConvectionInputs {
        density: MassDensity::new::<kilogram_per_cubic_meter>(phase.density),
        dynamic_viscosity: DynamicViscosity::new::<pascal_second>(phase.viscosity),
        thermal_conductivity: ThermalConductivity::new::<watt_per_meter_kelvin>(phase.conductivity),
        specific_heat_capacity: SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(phase.cp),
        thermal_expansion_coefficient: TemperatureCoefficient::new::<per_kelvin>(phase.expansion),
        bulk_temperature: ThermodynamicTemperature::new::<kelvin>(phase.temperature),
        wall_temperature: ThermodynamicTemperature::new::<kelvin>(t_w),
        flow_speed: Velocity::new::<meter_per_second>(phase.velocity),
        hydraulic_diameter: Length::new::<meter>(d_h),
        free_convection_length: None,
        gravity: Acceleration::new::<meter_per_second_squared>(9.806_65),
    };
    Ok(relap5_wall_heat_transfer(geometry, &inputs)?
        .heat_transfer_coefficient
        .get::<watt_per_square_meter_kelvin>())
}

#[cfg(test)]
mod tests;
