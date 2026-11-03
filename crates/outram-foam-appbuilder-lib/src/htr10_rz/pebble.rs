// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors

//! Pebble-internal temperatures of the HTR-10 fuel element (gh:#1150):
//! surface -> fuel-zone surface -> fuel-zone centre -> hottest kernel.
//!
//! **Reuses the GeN-Foam `nuclearSteadyStatePebble` port**
//! ([`PebbleGeometry::steady_temperatures`]), adapted to the HTR-10 pebble.
//! The port's pebble is an FHR annulus (an unfuelled graphite core inside
//! the fuel); the HTR-10 fuel zone is a **full sphere** of radius 2.5 cm in
//! a 0.5 cm unfuelled shell (TECDOC-1694 Tab 2.2, Tab 2.3). The adaptation
//! is the limit `r_core -> 0`: the port asserts `r_core > 0`, so
//! [`CORE_RADIUS_LIMIT_M`] = 1 um is used, which changes the matrix terms
//! by `(r_c/r_m)^3 ~ 6e-14` relative (pinned by a unit test against the
//! closed-form full sphere).
//!
//! **HTR-10 data:**
//!
//! | Quantity | Value | Source |
//! |---|---|---|
//! | Pebble / fuel-zone radius | 3.0 / 2.5 cm | Tab 2.2, Tab 2.3 (Quoted) |
//! | Kernel radius; coatings outward | 0.25 mm; buffer 0.09, IPyC 0.04, SiC 0.035, OPyC 0.04 mm | Tab 2.2 (Quoted) |
//! | TRISO per pebble | 8335 | Derived: 5.0 g U (Tab 2.2) / (kernel UO2 mass 6.807e-4 g x 0.8813 U mass fraction at 17 % enrichment); Putra 2021 Tab 1 uses exactly 8335 |
//! | Matrix (fuel-element) conductivity | Tab 2.9 fuel-element fit, zero dose, evaluated at the fuel-zone mean | Tab 2.9 (Quoted) |
//! | Kernel, buffer, PyC, SiC conductivity | 3.3073, 0.50, 4.00, 90.3 W/(m K) | the GeN-Foam `3D_gFHR` tutorial values the port records (not HTR-10 data; TECDOC-1694 gives none). They set only the kernel step |
//!
//! **Fuel-centre temperature, the benchmark definition.** TECDOC-1694
//! Sec 2.1.1.6 defines it as the mean of the central zone (diameter 0.3 cm)
//! of the fuel sphere split in five zones. For a uniformly heated sphere of
//! centre temperature `T_0` that mean is `T_0 - q b^2/(10 k)`, `b = 0.15 cm`.
//! The port gives `T_m,out` (the fuel-zone surface); this module adds the
//! sphere's own `q r_m^2/(6 k_eff)` to the centre and the zone mean.
//!
//! **Maxwell.** The port homogenises the TRISO into the matrix with the
//! Maxwell rule (its default physics). Whether Tab 2.9's "fuel element"
//! conductivity already contains the particles is not stated (Unknown), so
//! [`PebbleMatrix`] keeps the port's default and offers the homogeneous
//! benchmark reading as the comparison.

use super::Htr10RzError;
use crate::genfoam::thermal_hydraulics::structure::nuclear_steady_state_pebble::{
    PebbleConductivities, PebbleGeometry,
};
use crate::htgr_closures::StructuralMaterial;
use uom::si::areal_number_density::per_square_centimeter;
use uom::si::f64::{ArealNumberDensity, ThermodynamicTemperature};
use uom::si::thermal_conductivity::watt_per_meter_kelvin;
use uom::si::thermodynamic_temperature::kelvin;

/// The `r_core -> 0` limit used to run the FHR-annulus port on a full
/// sphere, m.
pub const CORE_RADIUS_LIMIT_M: f64 = 1.0e-6;

/// Radius of the benchmark's central "fuel-centre" zone, m (0.3 cm diameter,
/// Sec 2.1.1.6).
pub const CENTRE_ZONE_RADIUS_M: f64 = 0.0015;

/// The HTR-10 pebble as the port's geometry.
#[must_use]
pub fn htr10_pebble() -> PebbleGeometry {
    let kernel = 0.25e-3;
    PebbleGeometry {
        core_radius: CORE_RADIUS_LIMIT_M,
        matrix_radius: 0.025,
        shell_radius: 0.030,
        fuel_radius: kernel,
        buffer_radius: kernel + 0.09e-3,
        inner_pyc_radius: kernel + 0.13e-3,
        silicon_carbide_radius: kernel + 0.165e-3,
        outer_pyc_radius: kernel + 0.205e-3,
        triso_count: 8335.0,
    }
}

/// How the fuel-zone conductivity is formed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PebbleMatrix {
    /// The port's Maxwell homogenisation of the TRISO into the Tab 2.9
    /// matrix value. **Default** (the port's physics).
    #[default]
    MaxwellOnTab29,
    /// Tab 2.9 used directly as the fuel-zone conductivity (the
    /// homogeneous THERMIX reading of the benchmark). Comparison case.
    Tab29Homogeneous,
}

/// Temperatures inside one pebble, K.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PebbleProfile {
    /// Pebble surface (the solid field's bed temperature).
    pub surface: f64,
    /// Fuel-zone surface (`r = 2.5 cm`).
    pub fuel_zone_surface: f64,
    /// Fuel-zone centre (`r = 0`, matrix).
    pub centre: f64,
    /// The benchmark's "fuel-centre temperature" (Sec 2.1.1.6).
    pub benchmark_centre: f64,
    /// Hottest kernel centre: a TRISO at the pebble centre.
    pub kernel_max: f64,
    /// Fuel-zone effective conductivity used, W/(m K).
    pub fuel_zone_conductivity: f64,
    /// Power of the pebble, W.
    pub pebble_power: f64,
}

pub(crate) fn tab29_k(t_k: f64) -> Result<f64, Htr10RzError> {
    StructuralMaterial::FuelElement {
        fast_fluence: ArealNumberDensity::new::<per_square_centimeter>(0.0),
    }
    .thermal_conductivity(ThermodynamicTemperature::new::<kelvin>(t_k.clamp(273.15, 1573.15)))
    .map(|k| k.get::<watt_per_meter_kelvin>())
    .map_err(|e| Htr10RzError::Property(format!("fuel element: {e}")))
}

/// The pebble profile for a surface temperature (K) and a bed-homogenised
/// power density (W/m^3) at the bed solid fraction 0.61 (Tab 2.2). The
/// Tab 2.9 conductivity is evaluated at the fuel-zone mean temperature,
/// iterated to 1e-6 K.
///
/// # Errors
/// [`Htr10RzError::Property`] if the Tab 2.9 fit fails.
pub fn pebble_profile(surface_k: f64, power_density: f64, matrix: PebbleMatrix) -> Result<PebbleProfile, Htr10RzError> {
    let geo = htr10_pebble();
    let fuel = |k: f64| PebbleConductivities {
        fuel: 3.3073,
        buffer: 0.50,
        pyrolytic_carbon: 4.00,
        silicon_carbide: 90.3,
        graphite: k,
    };
    let mut t_eval = surface_k;
    let mut out = None;
    for _ in 0..100 {
        let k = tab29_k(t_eval)?;
        let c = fuel(k);
        let p = geo.steady_temperatures(surface_k, power_density.max(0.0), 0.61, &c);
        let k_fz = match matrix {
            PebbleMatrix::MaxwellOnTab29 => p.matrix_effective_conductivity,
            PebbleMatrix::Tab29Homogeneous => k,
        };
        let rm = geo.matrix_radius;
        let q_m = p.pebble_power / (4.0 / 3.0 * std::f64::consts::PI * rm * rm * rm);
        let rise = q_m * rm * rm / (6.0 * k_fz);
        let centre = p.matrix_outer + rise;
        let b = CENTRE_ZONE_RADIUS_M;
        // TRISO step at the centre: the port's particle rise above its
        // matrix (it evaluates it at the matrix average).
        let particle = p.fuel_max - p.matrix_average;
        let prof = PebbleProfile {
            surface: surface_k,
            fuel_zone_surface: p.matrix_outer,
            centre,
            benchmark_centre: centre - q_m * b * b / (10.0 * k_fz),
            kernel_max: centre + particle,
            fuel_zone_conductivity: k_fz,
            pebble_power: p.pebble_power,
        };
        // Fuel-zone volume mean of a heated sphere: T_out + 0.4 rise.
        let mean = p.matrix_outer + 0.4 * rise;
        let done = (mean - t_eval).abs() < 1e-6;
        t_eval = mean;
        out = Some(prof);
        if done {
            break;
        }
    }
    out.ok_or_else(|| Htr10RzError::Property("pebble conductivity iteration".into()))
}
