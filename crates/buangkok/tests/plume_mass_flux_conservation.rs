// SPDX-License-Identifier: GPL-3.0-only
//! # Verification: the steady Gaussian plume carries exactly the release rate downwind
//!
//! Written for the dispersion lesson track (gh:#530, rung 1), as the check
//! behind the lesson's derivation of the plume from the advection-diffusion
//! equation.
//!
//! ## Methodology
//!
//! - **What is computed.** For a steady release `Q`, every vertical plane
//!   across the wind at distance `x` must carry the whole release rate:
//!   `u * integral of C dy dz = Q` over `-inf < y < inf`, `z >= 0`. Divided by
//!   `Q` that is `u * integral of (chi/Q) dy dz = 1`. The terms are those of
//!   [`buangkok::pydoseia::dispersion::master_equation_single_plume`]
//!   (`chi/Q = pre_expo * expo`), with `u` the `speed_factor` passed to it.
//! - **Reference.** The analytic value 1: the crosswind Gaussian integrates to
//!   `sqrt(2 pi) sigma_y`; the reflected vertical Gaussian over `z >= 0`
//!   integrates to `sqrt(2 pi) sigma_z`; their product cancels the
//!   `2 pi sigma_y sigma_z u` of the pre-exponential factor. This is
//!   **verification** (the code against its own formula and the conservation
//!   law it must satisfy), not validation.
//! - **Inputs.** Every class A-F; `x` = 0.2, 1 and 5 km (one in each of the
//!   `sigma_z` distance bands); release height 30 m, wind measured at 10 m, a
//!   1 m/s wind at the measurement height scaled by
//!   [`buangkok::pydoseia::dispersion::height_correction_factor`], so `u` is
//!   the release-height speed upstream uses.
//! - **Quadrature.** The trapezoid rule on `+/- 10 sigma_y` and on
//!   `[0, H + 10 sigma_z]`, step `sigma / 4`. For a Gaussian the trapezoid
//!   error is of order `exp(-2 pi^2 sigma^2 / h^2)` (about `e^{-316}`), the
//!   truncation below `e^{-50}`, and the vertical integrand is even in `z`
//!   (the image makes it so), so the half-line rule keeps that accuracy.
//! - **Pass criterion, fixed before running.** `|u * integral - 1| < 1e-12`
//!   for every case (rounding over about 10^4 terms).
//!
//! **Prediction written before the first run:** every case agrees to about
//! 1e-15, with no dependence on class or band. A miss would mean the
//! pre-exponential factor or the image term is wrong.
//!
//! ## Results
//!
//! `cargo test --release -p buangkok --test plume_mass_flux_conservation -- --nocapture`,
//! 2026-10-04, on `develop` `d4428668be` plus this file, one core (core 12),
//! under 0.01 s. **Both pass.**
//!
//! - Half-space, 18 cases (A-F at 0.2, 1, 5 km): `u * integral - 1` between
//!   `-3.9e-15` and `+1.6e-15`, **worst 3.89e-15** (class F, 200 m), inside
//!   the 1e-12 criterion, with no trend in class or band.
//! - Whole line, low release (class F, 200 m, `H = sigma_z / 2`): half-space
//!   0.999999999999999, whole line 2.000000000000001, so the image is present.
//!
//! The prediction ("about 1e-15") held. Interpretation: the pre-exponential
//! factor `1 / (2 pi sigma_y sigma_z u)` and the reflected vertical term are
//! consistent with conservation of the released flux, at every distance band
//! of pyDOSEIA's `sigma_z` fit. That is verification of the formula as coded;
//! it says nothing about whether the sigma fits describe a real plume.

use buangkok::pydoseia::dispersion::{
    height_correction_factor, master_equation_single_plume, sigma_y, sigma_z, Receptor,
    StabilityClass,
};
use uom::si::f64::Length;
use uom::si::length::meter;

fn trapezoid_weight(i: usize, n: usize) -> f64 {
    if i == 0 || i == n {
        0.5
    } else {
        1.0
    }
}

/// `u * integral of chi/Q dy dz` over `z >= 0` (or the whole line).
fn flux_ratio(class: StabilityClass, x_m: f64, h_m: f64, whole_line: bool) -> f64 {
    let x = Length::new::<meter>(x_m);
    let h = Length::new::<meter>(h_m);
    let sy_l = sigma_y(class, x);
    let sz_l = sigma_z(class, x);
    let (sy, sz) = (sy_l.get::<meter>(), sz_l.get::<meter>());
    let u = height_correction_factor(class, h, Length::new::<meter>(10.0));
    let ny = 80;
    let ystep = sy / 4.0;
    let ztop = h_m + 10.0 * sz;
    let zbottom = if whole_line { -ztop } else { 0.0 };
    let nz = ((ztop - zbottom) / (sz / 4.0)).ceil() as usize;
    let zstep = (ztop - zbottom) / nz as f64;
    let mut total = 0.0;
    for j in 0..=ny {
        let y = -10.0 * sy + j as f64 * ystep;
        let wj = trapezoid_weight(j, ny);
        for k in 0..=nz {
            let z = zbottom + k as f64 * zstep;
            let wk = trapezoid_weight(k, nz);
            let t = master_equation_single_plume(
                sy_l,
                sz_l,
                u,
                h,
                Receptor::Offset {
                    y: Length::new::<meter>(y),
                    z: Length::new::<meter>(z),
                },
            );
            total += wj * wk * t.pre_expo * t.expo;
        }
    }
    u * total * ystep * zstep
}

/// Every class, one distance per `sigma_z` band, `H = 30 m`.
///
/// **Results (2026-10-04, `--release`, one core):** printed per case; the
/// worst case is quoted in the lesson (rung 1) with this file as its source.
#[test]
fn every_crosswind_plane_carries_the_whole_release() {
    let mut worst: f64 = 0.0;
    println!("class  x_m  u*integral - 1");
    for class in StabilityClass::ALL {
        for x_m in [200.0, 1000.0, 5000.0] {
            let r = flux_ratio(class, x_m, 30.0, false) - 1.0;
            println!("{class:?}  {x_m:>6}  {r:+.3e}");
            worst = worst.max(r.abs());
        }
    }
    println!("worst |u*integral - 1| = {worst:.3e}");
    assert!(
        worst < 1e-12,
        "the plume must carry Q through every plane: worst {worst:e}"
    );
}

/// Non-vacuity: over the **whole** line in `z` the kernel carries `2 Q`,
/// because it holds the real plume and its image. A kernel without the image
/// would carry `Q` there, and would also pass the half-space test above for a
/// plume that never reaches the ground; this shows the image is there. A low
/// release (class F, 200 m, `H = sigma_z / 2`) makes the image's share large.
#[test]
fn the_image_is_present_whole_line_carries_twice_the_release() {
    let class = StabilityClass::F;
    let sz = sigma_z(class, Length::new::<meter>(200.0)).get::<meter>();
    let half = flux_ratio(class, 200.0, 0.5 * sz, false);
    let whole = flux_ratio(class, 200.0, 0.5 * sz, true);
    println!("low release: half-space {half:.15}, whole line {whole:.15}");
    assert!((half - 1.0).abs() < 1e-12, "half-space {half}");
    assert!(
        (whole - 2.0).abs() < 2e-12,
        "whole line must carry 2Q: {whole}"
    );
}
