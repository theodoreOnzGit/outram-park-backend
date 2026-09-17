// SPDX-License-Identifier: GPL-3.0
//
// FLEXPART port — provenance
// --------------------------
// Upstream project : FLEXPART (NILU) — https://github.com/flexpart/flexpart
// Upstream version : 10.4 (2019-11-12), commit 3d7eebf
// Upstream source  : src/par_mod.f90
// Original licence : GPL-3.0-or-later — SPDX-FileCopyrightText: FLEXPART 1998-2019
// Ported into this GPL-3.0 work; see LICENSE.flexpart and NOTICE.flexpart.

//! Physical constants, transcribed from FLEXPART's `par_mod.f90`.
//!
//! # Why these are `f32`-valued constants written as `f64`
//!
//! FLEXPART's makefile carries no `-fdefault-real-8`, so its default `real` is
//! **single precision**. `pi = 3.14159265` in `par_mod.f90` is therefore stored
//! as the `f32` value `3.14159274…`, not the `f64` value.
//!
//! This port keeps the **decimal literals exactly as upstream writes them** and
//! evaluates them in `f64`. That is a deliberate choice: it ports the *intended*
//! constant rather than the rounding artefact of upstream's storage format, so
//! the port is at least as accurate as FLEXPART everywhere. The consequence is
//! that results differ from an as-shipped FLEXPART build at the `f32` level
//! (~1e-7 relative), which is measured and reported rather than assumed — see
//! `docs/flexpart-code-to-code.md`.

/// Circle constant, as written in `par_mod.f90` (`pi=3.14159265`).
///
/// Upstream truncates π to nine significant figures; this port keeps that exact
/// literal rather than substituting `std::f64::consts::PI`, so the arithmetic
/// matches the Fortran term for term.
// Deliberately upstream's truncated literal, not std's PI. Substituting the
// accurate value would silently break bit-exact agreement with FLEXPART; the
// module docs above explain the choice.
#[allow(clippy::approx_constant)]
pub const PI: f64 = 3.141_592_65;

/// Mean Earth radius, m (`r_earth=6.371e6`).
pub const R_EARTH: f64 = 6.371e6;

/// Specific gas constant for dry air, J/(kg·K) (`r_air=287.05`).
pub const R_AIR: f64 = 287.05;

/// Standard gravitational acceleration, m/s² (`ga=9.81`).
pub const GA: f64 = 9.81;

/// Specific heat capacity of dry air at constant pressure, J/(kg·K) (`cpa=1004.6`).
pub const CPA: f64 = 1004.6;

/// Poisson constant `R/cp` for dry air (`kappa=0.286`).
///
/// Note upstream carries this *and* computes `r_air/cpa` separately in
/// `obukhov.f90`; the two differ slightly (0.286 vs 0.285_78…). The port
/// reproduces each site's own choice rather than unifying them.
pub const KAPPA: f64 = 0.286;

/// Degrees-to-radians factor (`pi180=pi/180.`).
pub const PI180: f64 = PI / 180.0;

/// Universal gas constant, J/(mol·K) (`rgas=8.31447`).
pub const RGAS: f64 = 8.314_47;

/// Specific gas constant for water vapour, J/(kg·K) (`r_water=461.495`).
pub const R_WATER: f64 = 461.495;

/// Von Kármán constant as used by the surface-layer routines (`karman=0.40`).
///
/// `par_mod.f90` defines both `vonkarman=0.4` and `karman=0.40`. They are
/// numerically identical; the surface-layer routines use `karman`.
pub const KARMAN: f64 = 0.40;

/// Reference height for dry deposition, m (`href=15.`).
pub const HREF: f64 = 15.0;

/// Minimum allowed mixing height, m (`hmixmin=100.`).
pub const HMIXMIN: f64 = 100.0;

/// Maximum allowed mixing height, m (`hmixmax=4500.`).
pub const HMIXMAX: f64 = 4500.0;

/// Density of liquid water, kg/m³ (`rho_water=1000.`).
pub const RHO_WATER: f64 = 1000.0;

/// Number of aerosol diameter classes in the lognormal size distribution
/// (`ni=11`, `par_mod.f90:222`).
///
/// `part0` splits a lognormal distribution into this many bins spanning
/// ±3 geometric standard deviations about the mass median diameter.
pub const NI: usize = 11;
