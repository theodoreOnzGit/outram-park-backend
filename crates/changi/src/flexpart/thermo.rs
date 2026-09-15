// SPDX-License-Identifier: GPL-3.0
//
// FLEXPART port — provenance
// --------------------------
// Upstream project : FLEXPART (NILU) — https://github.com/flexpart/flexpart
// Upstream version : 10.4 (2019-11-12), commit 3d7eebf
// Upstream source  : src/ew.f90, src/dynamic_viscosity.f90
// Original licence : GPL-3.0-or-later — SPDX-FileCopyrightText: FLEXPART 1998-2019
// Ported into this GPL-3.0 work; see LICENSE.flexpart and NOTICE.flexpart.

//! Moist-air thermodynamic properties used by the surface-layer and deposition
//! schemes.
//!
//! Two kernels, both pure functions of temperature, both taken from FLEXPART
//! files that have no module dependencies at all.

use uom::si::dynamic_viscosity::pascal_second;
use uom::si::f64::{DynamicViscosity, Pressure, ThermodynamicTemperature};
use uom::si::pressure::pascal;
use uom::si::thermodynamic_temperature::kelvin;

/// Saturation water-vapour pressure over liquid water, in pascals.
///
/// Ports `ew.f90`. This is the **Goff–Gratch** formulation, expressed against
/// the steam point `T_st = 373.16 K`:
///
/// ```text
///   y = 373.16 / T
///   a = -7.90298 (y - 1) + 5.02808 · log10(y)
///   c = -1.3816e-7 · (10^(11.344 (1 - 1/y)) - 1)
///   d =  8.1328e-3 · (10^(-3.49149 (y - 1)) - 1)
///   e_w = 101324.6 · 10^(a + c + d)      [Pa]
/// ```
///
/// # Arguments
/// - `temperature` — air (or dew-point) temperature. Must be **strictly above
///   absolute zero**; upstream halts the program on `T <= 0 K`.
///
/// # Returns
/// Saturation vapour pressure as a [`Pressure`] (pascals).
///
/// # Valid range
/// The Goff–Gratch fit is over liquid water and is intended for roughly
/// 223–373 K. It is *not* the ice-phase formulation, and upstream applies no
/// range check beyond the `T > 0` guard, so out-of-range inputs return an
/// extrapolated value rather than an error.
///
/// # Panics
/// Panics if `temperature <= 0 K`, mirroring upstream's
/// `stop 'sorry: t not in [k]'`. A Rust caller reaching this has passed a
/// physically impossible temperature.
#[must_use]
pub fn saturation_vapour_pressure(temperature: ThermodynamicTemperature) -> Pressure {
    Pressure::new::<pascal>(ew_kelvin(temperature.get::<kelvin>()))
}

/// Bare-`f64` form of [`saturation_vapour_pressure`]: kelvin in, pascals out.
///
/// Exposed because the surface-layer routines call it in tight arithmetic where
/// the `uom` wrapper adds nothing, and because the code-to-code test drives it
/// directly against the Fortran.
#[must_use]
pub fn ew_kelvin(t_kelvin: f64) -> f64 {
    assert!(
        t_kelvin > 0.0,
        "saturation vapour pressure needs T > 0 K (upstream ew.f90 halts here); got {t_kelvin}"
    );
    let y = 373.16 / t_kelvin;
    let mut a = -7.902_98 * (y - 1.0);
    // 0.43429 is upstream's truncated log10(e); kept as written so the
    // arithmetic matches ew.f90 term for term.
    #[allow(clippy::approx_constant)]
    const UPSTREAM_LOG10_E: f64 = 0.434_29;
    a += 5.028_08 * UPSTREAM_LOG10_E * y.ln();
    let mut c = (1.0 - (1.0 / y)) * 11.344;
    c = -1.0 + 10f64.powf(c);
    c = -1.381_6 * c / 10f64.powi(7);
    let mut d = (1.0 - y) * 3.491_49;
    d = -1.0 + 10f64.powf(d);
    d = 8.132_8 * d / 10f64.powi(3);
    let y = a + c + d;
    101_324.6 * 10f64.powf(y)
}

/// Dynamic viscosity of air, in Pa·s.
///
/// Ports `dynamic_viscosity.f90` — **Sutherland's law**:
///
/// ```text
///   eta(T) = eta_0 · (T_0 + C)/(T + C) · (T/T_0)^{3/2}
/// ```
///
/// with upstream's constants `C = 120 K`, `T_0 = 291.15 K`,
/// `eta_0 = 1.827e-5 Pa·s`.
///
/// # Arguments
/// - `temperature` — air temperature.
///
/// # Returns
/// Dynamic viscosity as a [`DynamicViscosity`] (Pa·s).
///
/// # Valid range
/// Sutherland's law is accurate to a few per cent over roughly 200–1000 K for
/// air. Upstream applies no range check.
#[must_use]
pub fn dynamic_viscosity_of_air(temperature: ThermodynamicTemperature) -> DynamicViscosity {
    DynamicViscosity::new::<pascal_second>(viscosity_kelvin(temperature.get::<kelvin>()))
}

/// Bare-`f64` form of [`dynamic_viscosity_of_air`]: kelvin in, Pa·s out.
#[must_use]
pub fn viscosity_kelvin(t_kelvin: f64) -> f64 {
    const C: f64 = 120.0;
    const T_0: f64 = 291.15;
    const ETA_0: f64 = 1.827e-5;
    ETA_0 * (T_0 + C) / (t_kelvin + C) * (t_kelvin / T_0).powf(1.5)
}
