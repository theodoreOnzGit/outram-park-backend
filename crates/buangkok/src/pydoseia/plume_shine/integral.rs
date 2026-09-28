// SPDX-License-Identifier: GPL-3.0-only
//! The finite-cloud kernels, their integration limits and the triple
//! integral. Ported from pyDOSEIA `dosefunc.py` (`adgq_single_plume`,
//! `adgq_sector_average` inside `plumeshine_dose`) and `raddcffunc.py`
//! (`zyx_lim_for_integral*`), commit `dca4cdc3bb0bef7f7e692c8991cf536c91e7a4ce`,
//! Copyright (c) 2024 Dr. Biswajit Sadhu, MIT (see `crates/buangkok/NOTICE`).
//!
//! Every expression keeps upstream's operation order, so a kernel value is
//! bit-identical to upstream's for the same point.

use uom::si::f64::Length;
use uom::si::length::meter;

use super::tables::AirPhotonCoefficients;
use crate::pydoseia::dispersion::{sigma_y, sigma_z, StabilityClass, SECTOR_WIDTH_RAD};
use crate::pydoseia::quadpack::tplquad;

/// `epsabs = epsrel = 1.49e-02` for the single-plume integral (upstream).
pub const SINGLE_PLUME_EPS: f64 = 1.49e-2;
/// `epsabs = epsrel = 1.49e-03` for the sector-averaged integral (upstream).
pub const SECTOR_AVERAGED_EPS: f64 = 1.49e-3;

/// Which plume the cloud is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlumeShineMode {
    /// Instantaneous release: the single Gaussian plume, receptor at
    /// `(X1, Y, Z)` (upstream config `Y`, `Z`).
    SinglePlume {
        /// Receptor crosswind offset, m.
        y: f64,
        /// Receptor height, m.
        z: f64,
    },
    /// Long-term release: the sector-averaged plume, receptor at `(X1, 0, Z)`.
    SectorAveraged {
        /// Receptor height, m.
        z: f64,
    },
}

/// Geometry of one plume-shine evaluation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlumeShineGeometry {
    /// Downwind distance of the receptor, `X1`.
    pub distance: Length,
    /// Release height `H`.
    pub release_height: Length,
    /// Plume model and receptor offsets.
    pub mode: PlumeShineMode,
}

/// Python's `v ** 2` on a float: C `pow(v, 2.0)`. The exponent goes through
/// `black_box` so that LLVM does not rewrite the call as `v * v` (the same
/// value except when libm's `pow` rounds the square differently).
fn sq(v: f64) -> f64 {
    v.powf(core::hint::black_box(2.0))
}

fn sy(s: StabilityClass, x: f64) -> f64 {
    sigma_y(s, Length::new::<meter>(x)).get::<meter>()
}
fn sz(s: StabilityClass, x: f64) -> f64 {
    sigma_z(s, Length::new::<meter>(x)).get::<meter>()
}

/// `zyx_lim_for_integral_single_plume`: `[z_lo, z_hi, y_lo, y_hi, x_lo, x_hi]`
/// with `z` spanning `H +- 3 sigma_z` (floored at 1 m), `y` spanning
/// `+- 3 sigma_y`, and `x` spanning `X1 +- 3 MFP` (floored at 1 m), the sigmas
/// taken at `X1`.
#[must_use]
pub fn integration_limits_single_plume(
    s: StabilityClass,
    distance: Length,
    release_height: Length,
    mfp_m: f64,
) -> [f64; 6] {
    let x1 = distance.get::<meter>();
    let h = release_height.get::<meter>();
    let sigz = sz(s, x1);
    let sigy = sy(s, x1);
    let zin = if h - (3.0 * sigz) > 1.0 {
        h - (3.0 * sigz)
    } else {
        1.0
    };
    let zf = h + (3.0 * sigz);
    let yin = -3.0 * sigy;
    let yf = 3.0 * sigy;
    let times = 3.0;
    let xin = if x1 - (times * mfp_m) > 1.0 {
        x1 - (times * mfp_m)
    } else {
        1.0
    };
    let xf = x1 + (times * mfp_m);
    [zin, zf, yin, yf, xin, xf]
}

/// `zyx_lim_for_integral_sector_averaged_plume`: as
/// [`integration_limits_single_plume`] but `y` spans half a sector's arc,
/// `+- (2 * 3.14 * X1) / (16 * 2)` (upstream's `3.14`).
#[must_use]
pub fn integration_limits_sector_averaged(
    s: StabilityClass,
    distance: Length,
    release_height: Length,
    mfp_m: f64,
) -> [f64; 6] {
    let mut l = integration_limits_single_plume(s, distance, release_height, mfp_m);
    let x1 = distance.get::<meter>();
    l[2] = -(2.0 * 3.14 * x1) / (16.0 * 2.0);
    l[3] = (2.0 * 3.14 * x1) / (16.0 * 2.0);
    l
}

/// `zyx_lim_for_integral` (upstream's older limits, **never called** at
/// `dca4cdc3`; ported for completeness). Note that it evaluates the sigmas at
/// the *release height* used as a distance, floors `z` at 0, and runs `x` from
/// 0 to `X1 + 5 MFP`.
#[must_use]
pub fn integration_limits_legacy(
    s: StabilityClass,
    distance: Length,
    release_height: Length,
    mfp_m: f64,
    n: f64,
) -> [f64; 6] {
    let x1 = distance.get::<meter>();
    let h = release_height.get::<meter>();
    let sigz = sz(s, h);
    let sigy = sy(s, h);
    let zin = if h - (n * sigz) > 0.0 {
        h - (n * sigz)
    } else {
        0.0
    };
    let zf = h + (n * sigz);
    [zin, zf, -n * sigy, n * sigy, 0.0, x1 + (5.0 * mfp_m)]
}

/// The single-plume kernel at `(x, y, z)`, upstream's `expo_xyz` in
/// `adgq_single_plume`:
///
/// ```text
/// 1/(2 pi sy sz) * (1 + k mu r)/(4 pi r^2) * exp(-mu r)
///   * exp(-y^2 / (2 sy^2)) * [exp(-(z-H)^2/(2 sz^2)) + exp(-(z+H)^2/(2 sz^2))]
/// ```
///
/// with `r` the distance to the receptor `(X1, Y, Z)` and the sigmas taken
/// at the source-point distance `x`.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn kernel_single_plume(
    s: StabilityClass,
    c: AirPhotonCoefficients,
    x1: f64,
    ry: f64,
    rz: f64,
    h: f64,
    x: f64,
    y: f64,
    z: f64,
) -> f64 {
    let (k, mu) = (c.k, c.mu_per_m);
    let r2 = || sq(x - x1) + sq(y - ry) + sq(z - rz);
    let sigy = sy(s, x);
    let sigz = sz(s, x);
    (1.0 / (2.0 * core::f64::consts::PI * sigy * sigz))
        * ((1.0 + (k * mu * r2().sqrt())) / (4.0 * core::f64::consts::PI * r2()))
        * (-mu * r2().sqrt()).exp()
        * ((-0.5 * sq(y)) / sq(sigy)).exp()
        * ((-0.5 * sq(z - h) / sq(sigz)).exp() + (-0.5 * sq(z + h) / sq(sigz)).exp())
}

/// The sector-averaged kernel, upstream's `expo_xyz` in
/// `adgq_sector_average`: `1/(sqrt(2 pi) x theta sz)` in place of the
/// crosswind Gaussian, receptor at `(X1, 0, Z)`, `theta` = `0.39275`.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn kernel_sector_averaged(
    s: StabilityClass,
    c: AirPhotonCoefficients,
    x1: f64,
    rz: f64,
    h: f64,
    x: f64,
    y: f64,
    z: f64,
) -> f64 {
    let (k, mu) = (c.k, c.mu_per_m);
    let ry = 0.0;
    let r2 = || sq(x - x1) + sq(y - ry) + sq(z - rz);
    let sigz = sz(s, x);
    (1.0 / ((2.0 * core::f64::consts::PI).sqrt() * x * SECTOR_WIDTH_RAD * sigz))
        * ((1.0 + (k * mu * r2().sqrt())) / (4.0 * core::f64::consts::PI * r2()))
        * (-mu * r2().sqrt()).exp()
        * ((-0.5 * sq(z - h) / sq(sigz)).exp() + (-0.5 * sq(z + h) / sq(sigz)).exp())
}

/// One line's integral for one stability class: the kernel integrated over
/// upstream's limits with `tplquad` at upstream's tolerance (1.49e-2 single
/// plume, 1.49e-3 sector averaged).
#[must_use]
pub fn line_integral(s: StabilityClass, c: AirPhotonCoefficients, g: PlumeShineGeometry) -> f64 {
    let x1 = g.distance.get::<meter>();
    let h = g.release_height.get::<meter>();
    match g.mode {
        PlumeShineMode::SinglePlume { y: ry, z: rz } => {
            let lim = integration_limits_single_plume(s, g.distance, g.release_height, c.mfp_m);
            tplquad(
                |x, y, z| kernel_single_plume(s, c, x1, ry, rz, h, x, y, z),
                lim,
                SINGLE_PLUME_EPS,
                SINGLE_PLUME_EPS,
            )
        }
        PlumeShineMode::SectorAveraged { z: rz } => {
            let lim = integration_limits_sector_averaged(s, g.distance, g.release_height, c.mfp_m);
            tplquad(
                |x, y, z| kernel_sector_averaged(s, c, x1, rz, h, x, y, z),
                lim,
                SECTOR_AVERAGED_EPS,
                SECTOR_AVERAGED_EPS,
            )
        }
    }
}
