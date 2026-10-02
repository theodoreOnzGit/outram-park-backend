// SPDX-License-Identifier: GPL-3.0
//
// FLEXPART port — provenance
// --------------------------
// Upstream project : FLEXPART (NILU) — https://github.com/flexpart/flexpart
// Upstream version : 10.4 (2019-11-12), commit 3d7eebf
// Upstream source  : src/distance.f90, src/distance2.f90
// Original licence : GPL-3.0-or-later — SPDX-FileCopyrightText: FLEXPART 1998-2019
// Ported into this GPL-3.0 work; see LICENSE.flexpart and NOTICE.flexpart.

//! Great-circle distance on a sphere, as FLEXPART's plume-trajectory
//! clustering uses it.
//!
//! Both routines use their own Earth radius, `6.3712e6 m`, not `par_mod`'s
//! `r_earth = 6.371e6`, and their own `pi = 3.14159265358979`. They return
//! **km**. Points closer than a threshold in both coordinates are reported as
//! exactly `0`, which upstream uses to avoid `acos` of a value a rounding
//! above 1.

/// Earth radius local to `distance*.f90`, m.
const RERTH: f64 = 6.3712e6;
/// `pi` local to `distance*.f90`.
#[allow(clippy::approx_constant)]
const PI_LOCAL: f64 = 3.141_592_653_589_79;

fn great_circle_km(clat1: f64, slat1: f64, clat2: f64, slat2: f64, cdlon: f64) -> f64 {
    let crd = slat1 * slat2 + clat1 * clat2 * cdlon;
    RERTH * crd.acos() / 1000.0
}

/// `distance.f90`: great-circle distance in km between two points given in
/// **degrees**; `0` when both coordinates differ by less than `0.03°`.
#[must_use]
pub fn distance(rlat1: f64, rlon1: f64, rlat2: f64, rlon2: f64) -> f64 {
    let dpr = 180.0 / PI_LOCAL;
    if (rlat1 - rlat2).abs() < 0.03 && (rlon1 - rlon2).abs() < 0.03 {
        0.0
    } else {
        great_circle_km(
            (rlat1 / dpr).cos(),
            (rlat1 / dpr).sin(),
            (rlat2 / dpr).cos(),
            (rlat2 / dpr).sin(),
            ((rlon1 - rlon2) / dpr).cos(),
        )
    }
}

/// `distance2.f90`: as [`distance`], with coordinates in **radians** and a
/// `0.0003 rad` threshold.
#[must_use]
pub fn distance2(rlat1: f64, rlon1: f64, rlat2: f64, rlon2: f64) -> f64 {
    if (rlat1 - rlat2).abs() < 0.0003 && (rlon1 - rlon2).abs() < 0.0003 {
        0.0
    } else {
        great_circle_km(
            rlat1.cos(),
            rlat1.sin(),
            rlat2.cos(),
            rlat2.sin(),
            (rlon1 - rlon2).cos(),
        )
    }
}
