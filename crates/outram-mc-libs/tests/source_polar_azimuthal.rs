// SPDX-License-Identifier: GPL-3.0

//! **V&V gate** — the polar-azimuthal source angular distribution.
//! GitHub #264.
//!
//! # Why a statistical gate and not just arithmetic
//!
//! A direction sampler can be arithmetically correct and statistically wrong:
//! the frame can be orthonormal, every vector unit-length, and the *density*
//! still piled up on the axis. That is exactly the classic cone-source error —
//! sampling the polar **angle** uniformly instead of its **cosine** — and no
//! amount of checking `|u| == 1` catches it.
//!
//! So the distribution itself is measured.
//!
//! # Methodology
//!
//! 1. **Frame correctness**: sampled directions are unit length, and their
//!    projection on the axis reproduces the requested `mu` range.
//! 2. **Solid-angle uniformity**: for a cone of half-angle `theta`, `mu` must
//!    be uniform on `[cos theta, 1]`. Checked by binning `mu` into ten equal
//!    bins and requiring each to hold its share within its own binomial sigma —
//!    a test that FAILS if the sampler used uniform-in-angle, which would
//!    over-fill the bins nearest `mu = 1`.
//! 3. **Azimuthal uniformity**: the mean of `cos(phi)` and `sin(phi)` about the
//!    axis must both vanish, which they do not if the azimuth is skewed by a
//!    non-orthogonal reference vector.
//! 4. **Degenerate inputs refused** rather than producing NaN directions.
//! 5. **The `mu = +/-1` branch**: a pure-axis cone returns the axis exactly.
//!
//! # Results (2026-09-22), 200 000 samples
//!
//! Cone of half-angle 0.5 rad about a deliberately non-axis-aligned axis
//! `(1,2,2)/3`, `mu` binned into ten equal bins over `[cos 0.5, 1]`; expected
//! 20 000 +/- 134 each:
//!
//! ```text
//! bin 0: 19815 (1.38)   bin 5: 20002 (0.01)
//! bin 1: 20188 (1.40)   bin 6: 19952 (0.36)
//! bin 2: 19995 (0.04)   bin 7: 19804 (1.46)
//! bin 3: 19926 (0.55)   bin 8: 20064 (0.48)
//! bin 4: 19961 (0.29)   bin 9: 20293 (2.18)   <- worst
//! ```
//!
//! **Worst 2.18 sigma over ten bins**, which is what flat looks like. A
//! sampler uniform in the polar ANGLE rather than its cosine would pile up in
//! the high-`mu` bins and fail this by a wide margin -- that is the whole
//! reason the gate is binned rather than a mean.
//!
//! Azimuth: `<cos phi> = +0.000018` (0.01 sigma), `<sin phi> = +0.002228`
//! (1.41 sigma), both against `sigma = sqrt(1/(2N)) = 0.00158`. Measured with a
//! deliberately SKEW reference vector, so this also confirms the
//! Gram-Schmidt orthonormalisation in `new` -- without it the azimuth would be
//! sheared and these means would not vanish.
//!
//! The axis used throughout is `(1,2,2)/3`, not a coordinate axis: an
//! axis-aligned test would not catch a frame that happens to be correct only
//! for `z`.

use outram_mc_libs::geometry::position::Direction;
use outram_mc_libs::source::angle::{AngleDist, PolarAzimuthalAngle, ScalarDist};

const N: usize = 200_000;

fn axis() -> Direction {
    // Deliberately NOT a coordinate axis: an axis-aligned test would not catch
    // a frame that is only correct for z.
    let n = (1.0_f64 + 4.0 + 4.0).sqrt();
    Direction::new(1.0 / n, 2.0 / n, 2.0 / n)
}

#[test]
fn a_cone_is_uniform_in_solid_angle_not_in_angle() {
    let theta = 0.5_f64; // rad
    let cone = PolarAzimuthalAngle::cone(axis(), theta).unwrap();
    let a = axis();
    let mu_min = theta.cos();

    const NBINS: usize = 10;
    let mut counts = [0usize; NBINS];
    let mut seed = 0x5EED_0264_u64;
    for _ in 0..N {
        let d = cone.sample(&mut seed, 1.0);
        let norm = (d.u * d.u + d.v * d.v + d.w * d.w).sqrt();
        assert!((norm - 1.0).abs() < 1e-12, "direction not unit: {norm}");
        let mu = d.u * a.u + d.v * a.v + d.w * a.w;
        assert!(
            mu >= mu_min - 1e-9 && mu <= 1.0 + 1e-9,
            "mu {mu} outside the cone [{mu_min}, 1]"
        );
        let k = (((mu - mu_min) / (1.0 - mu_min)) * NBINS as f64) as usize;
        counts[k.min(NBINS - 1)] += 1;
    }

    let expected = N as f64 / NBINS as f64;
    let sigma = (expected * (1.0 - 1.0 / NBINS as f64)).sqrt();
    println!("mu bins (expected {expected:.0} +/- {sigma:.0} each):");
    let mut worst = 0.0_f64;
    for (i, &c) in counts.iter().enumerate() {
        let dev = (c as f64 - expected).abs() / sigma;
        worst = worst.max(dev);
        println!("  bin {i}: {c:7}  ({dev:.2} sigma)");
    }
    println!("worst deviation {worst:.2} sigma");
    assert!(
        worst < 4.0,
        "mu is not uniform on [cos theta, 1]; worst bin is {worst:.2} sigma out. \
         A sampler uniform in ANGLE rather than its cosine fails here, piling up \
         near mu = 1."
    );
}

/// The azimuth must be unbiased about the axis.
#[test]
fn the_azimuth_is_unbiased_about_the_axis() {
    // A deliberately SKEW hint vector: `new` must orthonormalise it. If it did
    // not, the azimuthal distribution would be sheared and these means would
    // not vanish.
    let pa = PolarAzimuthalAngle::new(
        axis(),
        Direction::new(0.3, 0.9, -0.2),
        ScalarDist::Uniform { lo: 0.2, hi: 0.9 },
        ScalarDist::Uniform {
            lo: 0.0,
            hi: std::f64::consts::TAU,
        },
    )
    .unwrap();

    // Build an orthonormal pair perpendicular to the axis to resolve phi.
    let a = axis();
    let hint = Direction::new(1.0, 0.0, 0.0);
    let d0 = hint.u * a.u + hint.v * a.v + hint.w * a.w;
    let e1 = {
        let v = Direction::new(hint.u - d0 * a.u, hint.v - d0 * a.v, hint.w - d0 * a.w);
        let n = (v.u * v.u + v.v * v.v + v.w * v.w).sqrt();
        Direction::new(v.u / n, v.v / n, v.w / n)
    };
    let e2 = Direction::new(
        a.v * e1.w - a.w * e1.v,
        a.w * e1.u - a.u * e1.w,
        a.u * e1.v - a.v * e1.u,
    );

    let mut seed = 0xC0FFEE_u64;
    let (mut sc, mut ss) = (0.0_f64, 0.0_f64);
    for _ in 0..N {
        let d = pa.sample(&mut seed, 1.0);
        let mu = d.u * a.u + d.v * a.v + d.w * a.w;
        let f = (1.0 - mu * mu).max(0.0).sqrt();
        if f < 1e-9 {
            continue;
        }
        sc += (d.u * e1.u + d.v * e1.v + d.w * e1.w) / f;
        ss += (d.u * e2.u + d.v * e2.v + d.w * e2.w) / f;
    }
    let (mc, ms) = (sc / N as f64, ss / N as f64);
    // Each of cos(phi), sin(phi) has variance 1/2, so the mean's sigma is
    // sqrt(1/(2N)).
    let sigma = (0.5 / N as f64).sqrt();
    println!("<cos phi> = {mc:+.6} ({:.2} sigma), <sin phi> = {ms:+.6} ({:.2} sigma)", mc.abs()/sigma, ms.abs()/sigma);
    assert!(
        mc.abs() < 4.0 * sigma && ms.abs() < 4.0 * sigma,
        "the azimuth is biased: <cos phi> = {mc:+.6}, <sin phi> = {ms:+.6}, sigma \
         {sigma:.6}. A non-orthogonal reference vector that was not orthonormalised \
         produces exactly this."
    );
}

/// The `mu = +/-1` branch returns the axis exactly, not a nearly-axis vector.
#[test]
fn a_degenerate_cone_returns_the_axis_exactly() {
    let a = axis();
    let pure = PolarAzimuthalAngle::new(
        a,
        Direction::new(0.0, 0.0, 1.0),
        ScalarDist::Constant(1.0),
        ScalarDist::Uniform {
            lo: 0.0,
            hi: std::f64::consts::TAU,
        },
    )
    .unwrap();
    let mut seed = 7;
    for _ in 0..1000 {
        let d = pure.sample(&mut seed, 1.0);
        assert_eq!((d.u, d.v, d.w), (a.u, a.v, a.w), "mu = 1 must return the axis exactly");
    }

    let back = PolarAzimuthalAngle::new(
        a,
        Direction::new(0.0, 0.0, 1.0),
        ScalarDist::Constant(-1.0),
        ScalarDist::Constant(0.0),
    )
    .unwrap();
    let d = back.sample(&mut seed, 1.0);
    assert_eq!((d.u, d.v, d.w), (-a.u, -a.v, -a.w));
}

/// Degenerate frames are refused at construction, not at sampling time.
#[test]
fn degenerate_frames_are_refused() {
    let zero = Direction::new(0.0, 0.0, 0.0);
    assert!(PolarAzimuthalAngle::new(
        zero,
        Direction::new(1.0, 0.0, 0.0),
        ScalarDist::Constant(1.0),
        ScalarDist::Constant(0.0)
    )
    .unwrap_err()
    .contains("zero length"));

    let a = axis();
    assert!(PolarAzimuthalAngle::new(
        a,
        a,
        ScalarDist::Constant(1.0),
        ScalarDist::Constant(0.0)
    )
    .unwrap_err()
    .contains("parallel"));

    assert!(PolarAzimuthalAngle::cone(a, 4.0).unwrap_err().contains("outside"));
}
