// SPDX-License-Identifier: GPL-3.0
//! # Verification: one Gaussian puff, with its ground image, holds exactly its mass
//!
//! Written for the dispersion lesson track (gh:#530, rung 4), as the check
//! behind the claim that the image source "returns exactly the mass that would
//! have leaked below ground".
//!
//! ## Methodology
//!
//! - **What is computed.** The volume integral, over the half-space `z >= 0`,
//!   of [`changi::puff::concentration::gaussian_puff_concentration`] for a
//!   puff of mass `Q = 1 kg`. Analytically the integral is exactly `Q`: the
//!   horizontal Gaussian integrates to `2 pi sigma_y^2`, and the real vertical
//!   Gaussian plus its image at `-H`, taken over `z >= 0`, integrate to one
//!   whole Gaussian over the real line, `sqrt(2 pi) sigma_z`.
//! - **Reference.** That analytic value, `Q`. This is **verification** (the
//!   kernel against its own formula), not validation.
//! - **Inputs.** Every stability class A-F; travel distances 0.2, 1 and 5 km
//!   (inside the roughly 0.1-10 km fit range); release height 30 m; the
//!   receptor grid centred on the puff.
//! - **Quadrature.** The trapezoid rule on `+/- 10 sigma` horizontally and on
//!   `[0, H + 10 sigma_z]` vertically, step `sigma / 4`. For a Gaussian the
//!   trapezoid error with step `h` is of order `exp(-2 pi^2 sigma^2 / h^2)`,
//!   about `e^{-316}` here, and the truncation at 10 sigma is below `e^{-50}`.
//!   The vertical integrand is **even** in `z` (the image makes it so), so the
//!   half-line trapezoid keeps that accuracy at `z = 0`.
//! - **Pass criterion, fixed before running.** `|integral / Q - 1| < 1e-12`
//!   for every case. The bound is rounding over about 10^5-10^6 terms, not a
//!   tuned number.
//!
//! **Prediction written before the first run:** every case agrees to about
//! 1e-14 (summation rounding), with no dependence on class or distance.
//!
//! ## Results
//!
//! `cargo test --release -p changi --test puff_mass_conservation -- --nocapture`,
//! 2026-10-04, on `develop` `d4428668be` plus this file, one core (core 12),
//! 0.35 s for both tests. **Both pass.**
//!
//! - Half-space, 18 cases: `integral / Q - 1` lies between `-5.8e-14` and
//!   `-2.39e-13`, **worst 2.39e-13** (class F, 0.2 km), inside the 1e-12
//!   criterion.
//! - Whole line, low puff (class F, 0.2 km, `H = sigma_z / 2`): half-space
//!   0.999999999999850, whole line 1.999999999999505, so the image is present.
//!
//! **The prediction was wrong in size.** It said "about 1e-14"; the residuals
//! are about ten times larger, and every one is negative. The hypothesis was
//! summation rounding over 10^5 to 10^6 positive terms, not missing mass (the
//! analytic truncation and quadrature errors are below `e^{-50}`). **Tested,
//! same day:** a scratch copy of this test with Neumaier compensated summation
//! (not committed) gave residuals of at most **2.2e-16** in all 18 cases, so
//! the 1e-13 level is the naive sum's rounding. The committed test keeps the
//! naive sum it was registered with, and the 1e-12 criterion, fixed before
//! the run, was not moved.

use changi::puff::concentration::gaussian_puff_concentration;
use changi::puff::dispersion::pasquill_gifford_sigmas;
use changi::puff::stability::StabilityClass;
use uom::si::f64::{Length, Mass};
use uom::si::length::{kilometer, meter};
use uom::si::mass::kilogram;
use uom::si::mass_density::kilogram_per_cubic_meter;

const CLASSES: [StabilityClass; 6] = [
    StabilityClass::A,
    StabilityClass::B,
    StabilityClass::C,
    StabilityClass::D,
    StabilityClass::E,
    StabilityClass::F,
];

/// Trapezoid weights on `n + 1` equally spaced points.
fn trapezoid_weight(i: usize, n: usize) -> f64 {
    if i == 0 || i == n {
        0.5
    } else {
        1.0
    }
}

/// The puff integral over `z >= 0` (or the whole line), divided by its mass.
fn mass_ratio(class: StabilityClass, travel_km: f64, h_m: f64, whole_line: bool) -> f64 {
    let travel = Length::new::<kilometer>(travel_km);
    let s = pasquill_gifford_sigmas(class, travel).expect("a moved puff has sigmas");
    let sy = s.sigma_y.get::<meter>();
    let sz = s.sigma_z.get::<meter>();
    let q = Mass::new::<kilogram>(1.0);
    // The puff sits at (0, 0, H); put it there and integrate around it.
    let (px, py) = (Length::new::<meter>(0.0), Length::new::<meter>(0.0));
    let hstep = sy / 4.0;
    let nh = 80; // +/- 10 sigma_y at sigma_y / 4
    let zstep = sz / 4.0;
    let ztop = h_m + 10.0 * sz;
    let zbottom = if whole_line { -ztop } else { 0.0 };
    let nz = ((ztop - zbottom) / zstep).ceil() as usize;
    let zstep = (ztop - zbottom) / nz as f64;
    let mut total = 0.0;
    for i in 0..=nh {
        let x = -10.0 * sy + i as f64 * hstep;
        let wi = trapezoid_weight(i, nh);
        for j in 0..=nh {
            let y = -10.0 * sy + j as f64 * hstep;
            let wj = trapezoid_weight(j, nh);
            for k in 0..=nz {
                let z = zbottom + k as f64 * zstep;
                let wk = trapezoid_weight(k, nz);
                let c = gaussian_puff_concentration(
                    q,
                    class,
                    px,
                    py,
                    Length::new::<meter>(h_m),
                    (
                        Length::new::<meter>(x),
                        Length::new::<meter>(y),
                        Length::new::<meter>(z),
                    ),
                    travel,
                );
                total += wi * wj * wk * c.get::<kilogram_per_cubic_meter>();
            }
        }
    }
    total * hstep * hstep * zstep
}

/// Every class, three travel distances, `H = 30 m`.
///
/// **Results (2026-10-04, `--release`, one core):** the printout gives
/// `integral / Q - 1` per case; the worst case is recorded in the lesson
/// (rung 4) with this file as its source.
#[test]
fn one_puff_holds_its_mass_in_the_half_space() {
    let mut worst: f64 = 0.0;
    println!("class  travel_km  integral/Q - 1");
    for class in CLASSES {
        for travel_km in [0.2, 1.0, 5.0] {
            let r = mass_ratio(class, travel_km, 30.0, false) - 1.0;
            println!("{:>5}  {travel_km:>9}  {r:+.3e}", class.letter());
            worst = worst.max(r.abs());
        }
    }
    println!("worst |integral/Q - 1| = {worst:.3e}");
    assert!(
        worst < 1e-12,
        "a puff and its image must hold exactly Q: worst {worst:e}"
    );
}

/// Non-vacuity: over the **whole** line `-inf < z < inf` the kernel must hold
/// `2 Q`, because it carries the real puff *and* its image. A kernel that had
/// lost its image term would hold `Q` there and still pass the half-space test
/// above for a puff high enough that nothing reaches the ground, so this is
/// the test that shows the image is actually present. A low puff (class F,
/// 0.2 km, `H = sigma_z / 2`) is used so that, in the half-space, the image
/// carries a large share (about 31 %) of the mass.
#[test]
fn the_image_is_present_whole_line_holds_twice_the_mass() {
    let class = StabilityClass::F;
    let s = pasquill_gifford_sigmas(class, Length::new::<kilometer>(0.2)).unwrap();
    let h = 0.5 * s.sigma_z.get::<meter>();
    let half = mass_ratio(class, 0.2, h, false);
    let whole = mass_ratio(class, 0.2, h, true);
    println!("low puff: half-space {half:.15}, whole line {whole:.15}");
    assert!((half - 1.0).abs() < 1e-12, "half-space {half}");
    assert!(
        (whole - 2.0).abs() < 2e-12,
        "whole line must hold 2Q: {whole}"
    );
}
