// SPDX-License-Identifier: GPL-3.0
//
// FLEXPART port — provenance
// --------------------------
// Upstream project : FLEXPART (NILU) — https://github.com/flexpart/flexpart
// Upstream version : 10.4 (2019-11-12), commit 3d7eebf
// Upstream source  : src/psim.f90, src/psih.f90, src/scalev.f90,
//                    src/obukhov.f90, src/raerod.f90
// Original licence : GPL-3.0-or-later — SPDX-FileCopyrightText: FLEXPART 1998-2019
// Ported into this GPL-3.0 work; see LICENSE.flexpart and NOTICE.flexpart.

//! Monin–Obukhov surface-layer similarity: stability corrections, the Obukhov
//! length, friction velocity and aerodynamic resistance.
//!
//! These five routines are what turn raw meteorological surface fields into the
//! turbulence scales that the dispersion and dry-deposition schemes need. All
//! of them are pure functions of scalars in upstream — none touches a
//! meteorological field array — which is why they are the first module ported
//! and the first verified against the Fortran.
//!
//! # Sign convention
//!
//! The Obukhov length `L` is **negative for unstable** stratification (upward
//! surface heat flux) and **positive for stable**. Both stability functions
//! branch on the sign of `z/L`, and they do **not** use the same empirical
//! constants as each other — see each function's notes.

use super::constants::{CPA, GA, HREF, KARMAN, PI, R_AIR};
use super::thermo::ew_kelvin;

/// Which meteorological product the input fields came from.
///
/// `obukhov.f90` branches on this: for ECMWF input it reconstructs the level-1
/// pressure from the model's hybrid coefficients, whereas for NCEP/GFS input
/// the caller supplies that pressure directly.
///
/// Modelled as an enum rather than upstream's bare integer flag so a caller
/// cannot pass a meaningless value, and so adding a third product forces every
/// `match` site to be revisited (workspace Rust design rule: enums for dispatch,
/// never trait objects).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MetDataFormat {
    /// ECMWF: level-1 pressure is rebuilt from the hybrid `A`/`B` coefficients.
    Ecmwf {
        /// `A` coefficients of the two lowest model half-levels, Pa.
        akm: [f64; 2],
        /// `B` coefficients of the two lowest model half-levels, dimensionless.
        bkm: [f64; 2],
    },
    /// NCEP/GFS: the caller supplies the level-1 pressure directly.
    Ncep,
}

/// Monin–Obukhov stability correction for **momentum**, `psi_m`.
///
/// Ports `psim.f90`. Dimensionless.
///
/// ```text
///   zeta = z / L
///   zeta <= 0 (unstable):  x = (1 - 15 zeta)^{1/4}
///                          psi_m = ln[((1+x)/2)^2 · (1+x^2)/2] - 2 atan(x) + pi/2
///   zeta >  0 (stable):    psi_m = -4.7 zeta
/// ```
///
/// The unstable branch is the Businger–Dyer/Paulson form; the stable branch is
/// the linear Businger form with coefficient 4.7.
///
/// # Arguments
/// - `z` — height above the surface, m.
/// - `obukhov_length` — Obukhov length `L`, m (negative = unstable).
///
/// # Returns
/// The dimensionless correction `psi_m`.
///
/// # Note on `zeta == 0`
/// Upstream tests `zeta.le.0.`, so exactly-zero `zeta` (neutral) takes the
/// *unstable* branch, where `x = 1` and the expression collapses to
/// `ln(1) - 2·atan(1) + pi/2 = 0`. The port keeps that branch ordering so the
/// neutral limit is reached identically.
#[must_use]
pub fn psim(z: f64, obukhov_length: f64) -> f64 {
    let zeta = z / obukhov_length;
    if zeta <= 0.0 {
        let x = (1.0 - 15.0 * zeta).powf(0.25);
        let a1 = ((1.0 + x) / 2.0).powi(2);
        let a2 = (1.0 + x * x) / 2.0;
        (a1 * a2).ln() - 2.0 * x.atan() + PI / 2.0
    } else {
        -4.7 * zeta
    }
}

/// Monin–Obukhov stability correction for **heat**, `psi_h`.
///
/// Ports `psih.f90`. Dimensionless.
///
/// ```text
///   zeta > 0 (stable):    psi_h = -(1 + 0.667 a zeta)^{3/2}
///                                 - b (zeta - c/d) exp(-d zeta) - b c/d + 1
///   zeta <= 0 (unstable): x = (1 - 16 zeta)^{1/4}
///                         psi_h = 2 ln[(1 + x^2)/2]
/// ```
///
/// with upstream's `a = 1`, `b = 0.667`, `c = 5`, `d = 0.35` — the Beljaars–Holtslag
/// stable form. Note the unstable branch uses **16**, where [`psim`] uses **15**;
/// that asymmetry is upstream's and is preserved deliberately.
///
/// # Two guards ported verbatim
///
/// 1. **Near-zero `L` is nudged away from zero**, to `±1e-20` matching its sign,
///    so the division cannot produce an infinity. Upstream mutates its `l`
///    argument in place to do this (Fortran arguments are by reference); this
///    port takes `L` by value and nudges its local copy, which is the same
///    arithmetic without the caller-visible side effect. **That side effect is a
///    real behavioural difference**: an upstream caller passing a variable with
///    `|L| < 1e-20` would find it modified on return. No in-tree caller relies
///    on it (`raerod.f90` passes its own `l` straight through), so the port
///    drops it — recorded here rather than silently.
/// 2. **Far-field short circuit**: when `log10(z) - log10(|L|) < log10(1e-20)`,
///    i.e. `z` is more than twenty decades below `|L|`, `psi_h` is exactly zero.
///
/// # Arguments
/// - `z` — height above the surface, m.
/// - `obukhov_length` — Obukhov length `L`, m (negative = unstable).
///
/// # Returns
/// The dimensionless correction `psi_h`.
#[must_use]
pub fn psih(z: f64, obukhov_length: f64) -> f64 {
    const A: f64 = 1.0;
    const B: f64 = 0.667;
    const C: f64 = 5.0;
    const D: f64 = 0.35;
    const EPS: f64 = 1.0e-20;

    let mut l = obukhov_length;
    // Kept as upstream's two explicit sign branches rather than clippy's
    // suggested range form: the mirrored branch below is `l > -EPS` (exclusive
    // at -EPS), which `(-EPS..0.0).contains()` would NOT reproduce at the
    // boundary. Matching psih.f90 line for line matters more here than brevity.
    #[allow(clippy::manual_range_contains)]
    if (l >= 0.0) && (l < EPS) {
        l = EPS;
    } else if (l < 0.0) && (l > -EPS) {
        l = -EPS;
    }

    if (z.log10() - l.abs().log10()) < EPS.log10() {
        return 0.0;
    }
    let zeta = z / l;
    if zeta > 0.0 {
        -(1.0 + 0.667 * A * zeta).powf(1.5) - B * (zeta - C / D) * (-D * zeta).exp() - B * C / D
            + 1.0
    } else {
        let x = (1.0 - 16.0 * zeta).powf(0.25);
        2.0 * ((1.0 + x * x) / 2.0).ln()
    }
}

/// Friction velocity `u*` from the surface momentum stress, m/s.
///
/// Ports `scalev.f90`:
///
/// ```text
///   e    = e_w(T_d)                  saturation vapour pressure at dew point
///   T_v  = T (1 + 0.378 e / p)       virtual temperature
///   rho  = p / (R_air T_v)           moist-air density
///   u*   = sqrt(|tau| / rho)
/// ```
///
/// # Arguments
/// - `surface_pressure` — `p`, Pa.
/// - `temperature` — surface air temperature `T`, K.
/// - `dew_point` — surface dew-point temperature `T_d`, K.
/// - `stress` — surface momentum stress `tau`, N/m². The absolute value is
///   taken, so sign conventions on the input do not matter.
///
/// # Returns
/// Friction velocity `u*`, m/s. Always non-negative.
#[must_use]
pub fn scalev(surface_pressure: f64, temperature: f64, dew_point: f64, stress: f64) -> f64 {
    let e = ew_kelvin(dew_point);
    let tv = temperature * (1.0 + 0.378 * e / surface_pressure);
    let rhoa = surface_pressure / (R_AIR * tv);
    (stress.abs() / rhoa).sqrt()
}

/// Obukhov length `L`, m.
///
/// Ports `obukhov.f90`:
///
/// ```text
///   e        = e_w(T_d,surf)
///   T_v      = T_surf (1 + 0.378 e / p_s)
///   rho      = p_s / (R_air T_v)
///   theta    = T_lev (100000 / p_lev)^{R_air/cp}      potential temperature
///   theta*   = H / (rho cp u*)                        scale temperature
///   L        = theta u*^2 / (karman g theta*)
/// ```
///
/// # Three guards ported verbatim
///
/// 1. `u* <= 0` is raised to `1e-8` before use, preventing division by zero.
/// 2. When `|theta*| <= 1e-10` — effectively zero surface heat flux — `L` is set
///    to the sentinel `9999`, **not** to infinity.
/// 3. `L` is then clamped to `[-9999, 9999]`. So `9999` is doing double duty as
///    both "neutral" and "clamped"; a caller cannot distinguish them, and that
///    ambiguity is upstream's.
///
/// # Arguments
/// - `surface_pressure` — `p_s`, Pa.
/// - `surface_temperature` — `T_surf`, K.
/// - `surface_dew_point` — `T_d,surf`, K.
/// - `level_temperature` — `T_lev`, temperature at the first model level, K.
/// - `ustar` — friction velocity `u*`, m/s (see [`scalev`]).
/// - `surface_heat_flux` — `H`, sensible heat flux, W/m² (positive upward).
/// - `level_pressure` — `p_lev`, Pa. **Used only for [`MetDataFormat::Ncep`]**;
///   for ECMWF it is recomputed from the hybrid coefficients and the value
///   passed here is ignored.
/// - `format` — which product the fields came from.
///
/// # Returns
/// Obukhov length `L` in metres, clamped to `[-9999, 9999]`; negative is
/// unstable, positive stable, `9999` means "no usable heat flux".
// Upstream's obukhov() takes ten arguments; this port takes eight, having
// folded the met-format flag and the hybrid coefficients into one enum. Bundling
// the rest into a struct would break the 1:1 correspondence with obukhov.f90
// that makes the port reviewable against its source, which is the point.
#[allow(clippy::too_many_arguments)]
#[must_use]
pub fn obukhov(
    surface_pressure: f64,
    surface_temperature: f64,
    surface_dew_point: f64,
    level_temperature: f64,
    ustar: f64,
    surface_heat_flux: f64,
    level_pressure: f64,
    format: MetDataFormat,
) -> f64 {
    let e = ew_kelvin(surface_dew_point);
    let tv = surface_temperature * (1.0 + 0.378 * e / surface_pressure);
    let rhoa = surface_pressure / (R_AIR * tv);

    let plev = match format {
        MetDataFormat::Ecmwf { akm, bkm } => {
            let ak1 = (akm[0] + akm[1]) / 2.0;
            let bk1 = (bkm[0] + bkm[1]) / 2.0;
            ak1 + bk1 * surface_pressure
        }
        MetDataFormat::Ncep => level_pressure,
    };

    let theta = level_temperature * (100_000.0 / plev).powf(R_AIR / CPA);
    let ustar = if ustar <= 0.0 { 1.0e-8 } else { ustar };
    let thetastar = surface_heat_flux / (rhoa * CPA * ustar);

    let l = if thetastar.abs() > 1.0e-10 {
        theta * ustar * ustar / (KARMAN * GA * thetastar)
    } else {
        9999.0
    };
    l.clamp(-9999.0, 9999.0)
}

/// Aerodynamic resistance `r_a` between the surface and the reference height,
/// s/m.
///
/// Ports `raerod.f90`:
///
/// ```text
///   r_a = [ ln(h_ref / z0) - psi_h(h_ref, L) + psi_h(z0, L) ] / (karman u*)
/// ```
///
/// with `h_ref = 15 m` ([`HREF`], `par_mod.f90`). This is the first of the three
/// resistances in the standard resistance analogy for dry deposition.
///
/// # Arguments
/// - `obukhov_length` — `L`, m.
/// - `ustar` — friction velocity, m/s.
/// - `roughness_length` — surface roughness length `z0`, m. Must be `> 0`.
///
/// # Returns
/// Aerodynamic resistance, s/m.
///
/// # Note
/// Upstream applies no guard on `u* = 0` or `z0 = 0` here, so both produce a
/// non-finite result. The port reproduces that rather than inventing a guard
/// upstream does not have; callers are expected to supply a positive `u*` from
/// [`scalev`] and a positive `z0` from the land-use table.
#[must_use]
pub fn raerod(obukhov_length: f64, ustar: f64, roughness_length: f64) -> f64 {
    ((HREF / roughness_length).ln() - psih(HREF, obukhov_length)
        + psih(roughness_length, obukhov_length))
        / (KARMAN * ustar)
}
