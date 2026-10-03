// SPDX-License-Identifier: GPL-3.0
//
// FLEXPART port — provenance
// --------------------------
// Upstream project : FLEXPART (NILU) — https://github.com/flexpart/flexpart
// Upstream version : 10.4 (2019-11-12), commit 3d7eebf
// Upstream source  : src/zenithangle.f90, src/photo_O1D.f90
// Original licence : GPL-3.0-or-later — SPDX-FileCopyrightText: FLEXPART 1998-2019
// Ported into this GPL-3.0 work; see LICENSE.flexpart and NOTICE.flexpart.

//! Solar geometry and the O(¹D) photolysis rate that drives FLEXPART's OH
//! reaction scaling.
//!
//! Both routines declare their **own** `pi = 3.1415927`, not `par_mod`'s
//! `3.14159265`; the port keeps each routine's literal.

use super::calendar::caldate;

/// The `pi` literal local to `zenithangle.f90` and `photo_O1D.f90`.
#[allow(clippy::approx_constant, clippy::excessive_precision)]
const PI_LOCAL: f64 = 3.141_592_7;

/// `zenithangle.f90`: solar zenith angle, degrees, from an approximate
/// declination and equation of time (Fourier series in the day of year).
///
/// # Arguments
/// - `ylat`, `xlon` — latitude and longitude, degrees.
/// - `jul` — Julian date (UTC).
///
/// The day-of-year count is upstream's: `31 (m-1) + d - int(0.4 m + 2.3)`
/// after February, plus one in years divisible by four (no century rule).
#[must_use]
pub fn zenithangle(ylat: f64, xlon: f64, jul: f64) -> f64 {
    let pi = PI_LOCAL;
    let (yyyymmdd, hhmmss) = caldate(jul);
    let jjjj = yyyymmdd / 10000;
    let mm = yyyymmdd / 100 - jjjj * 100;
    let id = yyyymmdd - jjjj * 10000 - mm * 100;
    let iu = hhmmss / 10000;
    let minute = hhmmss / 100 - 100 * iu;

    let mut ndaynum = 31 * (mm - 1) + id;
    if mm > 2 {
        ndaynum -= (0.4 * mm as f64 + 2.3) as i64;
    }
    if mm > 2 && jjjj / 4 * 4 == jjjj {
        ndaynum += 1;
    }

    let rnum = 2.0 * pi * ndaynum as f64 / 365.0;
    let rylat = pi * ylat / 180.0;
    let ttime = iu as f64 + minute as f64 / 60.0;
    let dekl = 0.396 + 3.631 * rnum.sin() + 0.038 * (2.0 * rnum).sin() + 0.077 * (3.0 * rnum).sin()
        - 22.97 * rnum.cos()
        - 0.389 * (2.0 * rnum).cos()
        - 0.158 * (3.0 * rnum).cos();
    let rdekl = pi * dekl / 180.0;
    let eq = (0.003
        - 7.343 * rnum.sin()
        - 9.47 * (2.0 * rnum).sin()
        - 0.329 * (3.0 * rnum).sin()
        - 0.196 * (4.0 * rnum).sin()
        + 0.552 * rnum.cos()
        - 3.020 * (2.0 * rnum).cos()
        - 0.076 * (3.0 * rnum).cos()
        - 0.125 * (4.0 * rnum).cos())
        / 60.0;
    let sinsol = rylat.sin() * rdekl.sin()
        + rylat.cos() * rdekl.cos() * ((ttime - 12.0 + xlon / 15.0 + eq) * pi / 12.0).cos();
    let solelev = sinsol.asin() * 180.0 / pi;
    90.0 - solelev
}

/// Zenith angles of upstream's `photo_O1D` table, degrees.
const ZANGLE: [f64; 11] = [0., 10., 20., 30., 40., 50., 60., 70., 78., 86., 90.0001];
/// Clear-sky O(¹D)/NO₂ photolysis factor at each table angle.
const FACT_PHOTO: [f64; 11] = [
    0.4616E-02, 0.4478E-02, 0.4131E-02, 0.3583E-02, 0.2867E-02, 0.2081E-02, 0.1235E-02, 0.5392E-03,
    0.2200E-03, 0.1302E-03, 0.0902E-03,
];

/// `photo_O1D.f90`: O(¹D) photolysis rate, 1/s, for a solar zenith angle in
/// degrees. Log-linear in `1/cos(sza)` between table angles, scaled by a
/// parameterised NO₂ photolysis rate; zero at night (`sza >= 90`).
///
/// # Returns
/// `None` for `sza < 0`: upstream's table index is then never assigned and
/// the result is undefined. A zenith angle is non-negative by definition, so
/// this only arises from a caller error.
#[must_use]
pub fn photo_o1d(sza: f64) -> Option<f64> {
    let pi = PI_LOCAL;
    if sza < 90.0 {
        let ik = (0..ZANGLE.len() - 1).rev().find(|&iz| sza >= ZANGLE[iz])?;
        let z1 = 1.0 / (ZANGLE[ik] * pi / 180.0).cos();
        let z2 = 1.0 / (ZANGLE[ik + 1] * pi / 180.0).cos();
        let zg = 1.0 / (sza * pi / 180.0).cos();
        let dummy = (zg - z1) / (z2 - z1);
        let f1 = FACT_PHOTO[ik].ln();
        let f2 = FACT_PHOTO[ik + 1].ln();
        let photo_no2 = 1.45e-2 * (-(0.4 / (sza * pi / 180.0).cos())).exp();
        Some(photo_no2 * (f1 + (f2 - f1) * dummy).exp())
    } else {
        Some(0.0)
    }
}
