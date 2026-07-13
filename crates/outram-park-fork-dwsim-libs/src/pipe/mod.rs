//! Pipe pressure-drop correlations (single-phase and two-phase gas-liquid flow).
//!
//! Ported from DWSIM `UnitOperations/Pipe.vb` and
//! `FluidFlowCorrelations/{FlowPackageBaseClass,BeggsBrill,LockhartMartinelli}.vb`
//! (see this crate's `docs/port-scope.md`). DWSIM's third correlation,
//! Petalas-Aziz, only wraps an unshipped native DLL upstream and has no
//! portable source -- not ported (see the workspace's `op-qo2.6` bead).

pub mod beggs_brill;
pub mod friction_factor;
pub mod lockhart_martinelli;

use beggs_brill::{beggs_brill_pressure_drop, BeggsBrillResult};
use lockhart_martinelli::{lockhart_martinelli_pressure_drop, LockhartMartinelliResult};
use uom::si::angle::radian;
use uom::si::f64::{
    Angle, DynamicViscosity, Length, MassDensity, Pressure, SurfaceTension, VolumeRate,
};

/// Shared inputs for a two-phase pipe pressure-drop evaluation, common to
/// every [`PipeFlowCorrelation`] variant.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PipeFlowInputs {
    /// Pipe segment length.
    pub length: Length,
    /// Pipe internal diameter.
    pub diameter: Length,
    /// Absolute pipe-wall roughness.
    pub roughness: Length,
    /// Pipe inclination from horizontal, positive = uphill.
    pub inclination: Angle,
    /// Liquid-phase volumetric flow rate.
    pub q_liquid: VolumeRate,
    /// Gas-phase volumetric flow rate.
    pub q_gas: VolumeRate,
    /// Liquid-phase mass density.
    pub density_liquid: MassDensity,
    /// Gas-phase mass density.
    pub density_gas: MassDensity,
    /// Liquid-phase dynamic viscosity.
    pub viscosity_liquid: DynamicViscosity,
    /// Gas-phase dynamic viscosity.
    pub viscosity_gas: DynamicViscosity,
    /// Liquid-phase surface tension (only used by
    /// [`PipeFlowCorrelation::BeggsBrill`]).
    pub surface_tension_liquid: SurfaceTension,
}

/// Result of a [`PipeFlowCorrelation`] evaluation -- the correlation-specific
/// detail (flow regime, holdup, ...) plus the always-available total
/// pressure drop.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PipeFlowResult {
    /// Detail from the Beggs & Brill (1973) correlation.
    BeggsBrill(BeggsBrillResult),
    /// Detail from the Lockhart-Martinelli (1949) correlation.
    LockhartMartinelli(LockhartMartinelliResult),
}

impl PipeFlowResult {
    /// Total two-phase pressure drop (friction plus elevation), regardless
    /// of which correlation produced it.
    pub fn total_pressure_drop(&self) -> Pressure {
        match self {
            Self::BeggsBrill(r) => r.total_pressure_drop(),
            Self::LockhartMartinelli(r) => r.total_pressure_drop(),
        }
    }
}

/// Which two-phase gas-liquid pipe flow correlation to evaluate.
///
/// DWSIM's third correlation, Petalas-Aziz, only wraps an unshipped native
/// DLL upstream and has no portable source -- not represented here (see the
/// workspace's `op-qo2.6` bead).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PipeFlowCorrelation {
    /// Beggs & Brill (1973) -- flow-regime-routed holdup and inclination
    /// correction. DWSIM's default.
    #[default]
    BeggsBrill,
    /// Lockhart-Martinelli (1949) -- separated-flow two-phase multiplier.
    LockhartMartinelli,
}

impl PipeFlowCorrelation {
    /// Evaluate this correlation's two-phase pressure drop for the given
    /// `inputs`.
    ///
    /// [`PipeFlowCorrelation::BeggsBrill`] needs a single no-slip mixture
    /// viscosity (per DWSIM's own treatment); this derives one as a
    /// volumetric-flow-weighted average of `inputs.viscosity_liquid` and
    /// `inputs.viscosity_gas` -- a standard simplification, not itself part
    /// of the DWSIM source (DWSIM leaves the no-slip viscosity input to the
    /// caller's property-package layer).
    pub fn pressure_drop(&self, inputs: &PipeFlowInputs) -> PipeFlowResult {
        match self {
            Self::BeggsBrill => {
                let q_l = inputs.q_liquid.value;
                let q_g = inputs.q_gas.value;
                let c_l = q_l / (q_l + q_g);
                let viscosity_no_slip =
                    inputs.viscosity_liquid * c_l + inputs.viscosity_gas * (1.0 - c_l);
                let elevation_change = inputs.length * inputs.inclination.get::<radian>().sin();
                PipeFlowResult::BeggsBrill(beggs_brill_pressure_drop(
                    inputs.length,
                    inputs.diameter,
                    inputs.roughness,
                    elevation_change,
                    inputs.q_liquid,
                    inputs.q_gas,
                    inputs.density_liquid,
                    inputs.density_gas,
                    viscosity_no_slip,
                    inputs.surface_tension_liquid,
                ))
            }
            Self::LockhartMartinelli => {
                PipeFlowResult::LockhartMartinelli(lockhart_martinelli_pressure_drop(
                    inputs.length,
                    inputs.diameter,
                    inputs.roughness,
                    inputs.inclination,
                    inputs.q_liquid,
                    inputs.q_gas,
                    inputs.density_liquid,
                    inputs.density_gas,
                    inputs.viscosity_liquid,
                    inputs.viscosity_gas,
                ))
            }
        }
    }
}

#[cfg(test)]
mod dispatch_tests {
    use super::*;
    use uom::si::angle::radian;
    use uom::si::dynamic_viscosity::pascal_second;
    use uom::si::length::meter;
    use uom::si::mass_density::kilogram_per_cubic_meter;
    use uom::si::surface_tension::newton_per_meter;
    use uom::si::volume_rate::cubic_meter_per_second;

    fn sample_inputs() -> PipeFlowInputs {
        PipeFlowInputs {
            length: Length::new::<meter>(100.0),
            diameter: Length::new::<meter>(0.1),
            roughness: Length::new::<meter>(4.6e-5),
            inclination: Angle::new::<radian>(0.0),
            q_liquid: VolumeRate::new::<cubic_meter_per_second>(0.005),
            q_gas: VolumeRate::new::<cubic_meter_per_second>(0.001),
            density_liquid: MassDensity::new::<kilogram_per_cubic_meter>(998.0),
            density_gas: MassDensity::new::<kilogram_per_cubic_meter>(1.2),
            viscosity_liquid: DynamicViscosity::new::<pascal_second>(1.0e-3),
            viscosity_gas: DynamicViscosity::new::<pascal_second>(1.8e-5),
            surface_tension_liquid: SurfaceTension::new::<newton_per_meter>(0.072),
        }
    }

    #[test]
    fn default_is_beggs_brill() {
        assert_eq!(PipeFlowCorrelation::default(), PipeFlowCorrelation::BeggsBrill);
    }

    #[test]
    fn both_correlations_dispatch_and_produce_positive_pressure_drop() {
        let inputs = sample_inputs();
        for corr in [PipeFlowCorrelation::BeggsBrill, PipeFlowCorrelation::LockhartMartinelli] {
            let result = corr.pressure_drop(&inputs);
            assert!(result.total_pressure_drop().value > 0.0);
        }
    }
}
