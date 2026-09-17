//! Named `uom` type aliases for the physical quantities used across the crate.
//!
//! The workspace "human interface layer" rule requires that a developer hovering
//! a symbol in rust-analyzer sees a *meaningful* name — `Permeability`, not a
//! raw `Quantity<ISQ<...>, SI<f64>, f64>`. These aliases give each subsurface
//! quantity a name and spell out its physical meaning and SI unit, even though
//! `uom` already enforces dimensions at compile time.
//!
//! All aliases are `f64`-backed `uom::si::f64` quantities and are `Copy`.
//!
//! ## Note on dimensionless quantities
//!
//! Saturation and porosity are physically dimensionless fractions in `[0, 1]`.
//! They alias `uom`'s [`Ratio`], which carries no unit; the valid-range
//! constraint is documented here and must be enforced by the code that consumes
//! them (a future validated newtype may wrap them — bead op-v6s.7).

use uom::si::f64::{
    Area, DynamicViscosity, Length, MassDensity, Pressure, Ratio, ThermodynamicTemperature,
    Velocity,
};

/// Fluid (or phase) pressure. SI unit: pascal (Pa).
///
/// In RICHARDS flow the primary unknown is liquid pressure; capillary pressure
/// `p_c = p_gas - p_liq` sets the saturation via the characteristic curves.
pub type FluidPressure = Pressure;

/// Capillary pressure `p_c = p_nonwetting - p_wetting`. SI unit: pascal (Pa).
///
/// Non-negative for a wetting liquid; drives the retention (characteristic)
/// curve `S_l(p_c)`.
pub type CapillaryPressure = Pressure;

/// Liquid saturation `S_l` — the fraction of pore volume occupied by the liquid
/// phase. Dimensionless, valid range `[0, 1]` (aliases [`Ratio`]).
pub type Saturation = Ratio;

/// Porosity `phi` — the fraction of bulk volume that is pore space.
/// Dimensionless, valid range `[0, 1]` (aliases [`Ratio`]).
pub type Porosity = Ratio;

/// Relative permeability `k_r` — the saturation-dependent factor in `[0, 1]`
/// scaling intrinsic permeability for a given phase. Dimensionless
/// (aliases [`Ratio`]).
pub type RelativePermeability = Ratio;

/// Intrinsic (absolute) permeability `k` of the porous medium. SI unit: square
/// metre (m^2). (1 darcy is approximately 9.869233e-13 m^2.)
///
/// Aliases [`Area`] because permeability has dimensions of length squared.
pub type Permeability = Area;

/// Darcy velocity (specific discharge) `q` — volumetric flux per unit bulk
/// cross-sectional area. SI unit: metre per second (m/s). Aliases [`Velocity`].
pub type DarcyVelocity = Velocity;

/// Fluid mass density `rho`. SI unit: kilogram per cubic metre (kg/m^3).
pub type FluidDensity = MassDensity;

/// Fluid dynamic viscosity `mu`. SI unit: pascal-second (Pa.s).
pub type FluidViscosity = DynamicViscosity;

/// Fluid temperature `T`. SI unit: kelvin (K). Used by the (planned) TH flow
/// mode and by temperature-dependent property correlations.
pub type FluidTemperature = ThermodynamicTemperature;

/// A geometric length (cell size, depth, elevation). SI unit: metre (m).
pub type GeoLength = Length;
