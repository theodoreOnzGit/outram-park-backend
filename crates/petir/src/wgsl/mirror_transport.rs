//! `f32` mirrors of `shaders/transport.wgsl` — GSL's transport integrals
//! `J(n, x)` for `n = 2 ..= 5`.
//!
//! Generated from the same parse of [`crate::specfunc::transport`] that
//! produced the shader, so the two cannot drift in their 72 coefficients.
//! See [`crate::wgsl::mirror`] for why a mirror exists at all.
//!
//! # Three machine constants are retargeted, and all three are precision
//! # constants
//!
//! By the taxonomy in `docs/wgsl-coverage.md` — which now has five entries —
//! a **precision** constant must be retargeted and a **range guard** must
//! not. All three here are the former:
//!
//! | constant | `f64` | `f32` |
//! |---|---|---|
//! | `GSL_LOG_DBL_EPSILON` | -36.0437 | -15.942385 |
//! | `3 sqrt(DBL_EPSILON)` | 4.47e-08 | 1.0358e-03 |
//! | `2 / DBL_EPSILON` | 9.01e+15 | 1.6777e+07 |
//!
//! # A prediction recorded and refuted
//!
//! The first constant was expected to be **load-bearing twice over**: it
//! sets `numexp`, the number of exponential images summed in the tail, *and*
//! it is the threshold below which the tail is discarded and `J(n, inf)`
//! returned exactly. Measured, only the first is real.
//!
//! | `x` | `numexp`, `f32` log-eps | with GSL's `f64` value |
//! |---|---|---|
//! | 5 | 4 | 8 |
//! | 10 | 2 | 4 |
//! | 16 | 1 | 3 |
//! | 30 | 1 | 2 |
//!
//! | saturation point | `f32` log-eps | `f64` log-eps |
//! |---|---|---|
//! | `J(2)` | 22.25 | 22.25 |
//! | `J(3)` | 25.05 | 25.05 |
//! | `J(4)` | 27.25 | 27.25 |
//! | `J(5)` | 29.60 | 29.60 |
//!
//! And the worst relative error against the `f64` module over `x` in
//! `(0, 75]` is **1.3633715693879367e-06 either way** — identical to the
//! last digit.
//!
//! **The saturation point does not move because the arithmetic enforces the
//! guard before the guard does.** `vinf - exp(t)` collapses to `vinf` as soon
//! as `exp(t)` falls below half an ulp of `vinf`, which happens well before
//! `t` reaches -36. That is the same mechanism by which
//! [`crate::wgsl::mirror_airy`]'s overflow threshold is enforced by `f32`'s
//! own `exp` overflow — except that there, retargeting would have *destroyed*
//! answers, and here it is harmless.
//!
//! So retargeting is correct, and its only measurable effect is that the tail
//! sums **half as many images** for the same answer. Worth doing; not worth
//! claiming an accuracy benefit for.
//! `the_retargeting_halves_the_work_and_changes_nothing_else` pins all three
//! halves of that.
//!
//! # What `f32` costs
//!
//! Worst relative against the `f64` module, per order:
//!
//! | order | worst | at |
//! |---|---|---|
//! | `J(2)` | 7.092e-08 | 3.03 |
//! | `J(3)` | 2.549e-07 | 4.02 |
//! | `J(4)` | 4.684e-07 | 4.08 |
//! | `J(5)` | 1.295e-06 | 4.53 |
//!
//! Every worst point sits just past `x = 4`, the Chebyshev/tail join, where
//! the value is formed as a limit minus a tail of comparable size.

// Under a std-linked build (`cargo test`) f32's inherent exp/ln shadow these
// trait methods, leaving the import formally unused. See crate::real.
#[allow(unused_imports)]
use crate::real::Real;

/// `ln(f32::EPSILON)`. Mirrors `PETIR_TRANSPORT_LOG_EPS`.
const LOG_EPS: f32 = -15.942385;
/// `3 sqrt(f32::EPSILON)`. Mirrors `PETIR_TRANSPORT_SMALL_CUT`.
const SMALL_CUT: f32 = 1.0358009e-3;
/// `2 / f32::EPSILON`. Mirrors `PETIR_TRANSPORT_TWO_OVER_EPS`.
const TWO_OVER_EPS: f32 = 16777216.0;

/// GSL's `transport2_cs`, 18 coefficients.
#[rustfmt::skip]
const TRANSPORT2: [f32; 18] = [
    1.6717604398727417, -0.1477353572845459, 0.014821382239460945, -0.0014195330440998077,
    0.00013065413804724813, -1.1715579603333026e-05, 1.0333498039472033e-06,
    -9.019112923169814e-08, 7.817717140312652e-09, -6.744565461680452e-10,
    5.799464034006441e-11, -4.9747619218498684e-12, 4.259609971603989e-13,
    -3.6421999991724865e-14, 3.110999972851013e-15, -2.6500000149068143e-16,
    2.3000000123137025e-17, -1.89999995262919e-18,
];

/// GSL's `transport3_cs`, 18 coefficients.
#[rustfmt::skip]
const TRANSPORT3: [f32; 18] = [
    0.7620125412940979, -0.1056743860244751, 0.01197780855000019, -0.001214401563629508,
    0.00011550997442100197, -1.0581598871794995e-05, 9.474663329456234e-07,
    -8.362211900703187e-08, 7.310910099533885e-09, -6.350595049831043e-10,
    5.4911828556436504e-11, -4.732139593371931e-12, 4.067695074001787e-13,
    -3.4897100129310105e-14, 2.9892000231676336e-15, -2.5600001172830847e-16,
    2.1900000455312983e-17, -1.89999995262919e-18,
];

/// GSL's `transport4_cs`, 18 coefficients.
#[rustfmt::skip]
const TRANSPORT4: [f32; 18] = [
    0.48075708746910095, -0.08175379037857056, 0.010027006268501282, -0.0010599339148029685,
    0.00010345062764827162, -9.64427090366371e-06, 8.745544164412422e-07,
    -7.793212120077442e-08, 6.864988577603981e-09, -5.999570840131696e-10,
    5.213662487846271e-11, -4.511838385540257e-12, 3.89215894756878e-13,
    -3.349360041702762e-14, 2.876700071728633e-15, -2.466999870141503e-16,
    2.1099999343327223e-17, -1.800000020426123e-18,
];

/// GSL's `transport5_cs`, 18 coefficients.
#[rustfmt::skip]
const TRANSPORT5: [f32; 18] = [
    0.347777783870697, -0.06645698845386505, 0.008611072786152363, -0.0009396682144142687,
    9.363247954752296e-05, -8.857132343109697e-06, 8.119150152197108e-07,
    -7.29576541402821e-08, 6.469714541879057e-09, -5.68490310381975e-10,
    4.9625598769198476e-11, -4.3109400597873826e-12, 3.7309998840440173e-13,
    -3.2197999149698175e-14, 2.7720000231844233e-15, -2.3800000573378295e-16,
    2.0999999824714462e-17, -1.800000020426123e-18,
];

/// `J(n, inf) = n! zeta(n)`. Mirrors `petir_transport_vinf`.
fn vinf(n: u32) -> f32 {
    match n {
        2 => 3.2898681,
        3 => 7.2123413,
        4 => 25.975758,
        5 => 124.43133,
        _ => f32::NAN,
    }
}

/// The Chebyshev series for order `n`. Mirrors `petir_transport_cheb`.
fn cheb(n: u32, x: f32) -> f32 {
    let c: &[f32] = match n {
        2 => &TRANSPORT2,
        3 => &TRANSPORT3,
        4 => &TRANSPORT4,
        5 => &TRANSPORT5,
        _ => return f32::NAN,
    };
    let Some((&c0, rest)) = c.split_first() else {
        return 0.0;
    };
    let (mut d, mut dd) = (0.0_f32, 0.0_f32);
    let y2 = 2.0 * x;
    for &ci in rest.iter().rev() {
        let temp = d;
        d = y2 * d - dd + ci;
        dd = temp;
    }
    x * d - dd + 0.5 * c0
}

/// Upstream's `transport_sumexp`. Mirrors `petir_transport_sumexp`.
fn sumexp(numexp: u32, order: u32, t: f32, x: f32) -> f32 {
    let mut rk = numexp as f32;
    let mut out = 0.0_f32;
    for _ in 1..=numexp {
        let mut sum2 = 1.0_f32;
        let xk = 1.0 / (rk * x);
        let mut xk1 = 1.0_f32;
        for _ in 1..=order {
            sum2 = sum2 * xk1 * xk + 1.0;
            xk1 += 1.0;
        }
        out *= t;
        out += sum2;
        rk -= 1.0;
    }
    out
}

/// `J(n, x)` in `f32` for `n` in `2 ..= 5`. Mirrors `petir_transport`.
pub fn transport(n: u32, x: f32) -> f32 {
    transport_with(n, x, LOG_EPS)
}

/// [`transport`] with the log-epsilon constant supplied, so that retargeting
/// it can be **measured** against keeping GSL's `f64` value rather than
/// argued. [`LOG_EPS`] is what ships, and the shader hard-codes it.
fn transport_with(n: u32, x: f32, log_eps: f32) -> f32 {
    if !(2..=5).contains(&n) || x.is_nan() || x < 0.0 {
        return f32::NAN;
    }
    let nf = n as f32;
    let mut p = 1.0_f32;
    for _ in 1..n {
        p *= x;
    }
    if x < SMALL_CUT {
        return p / (nf - 1.0);
    }
    if x <= 4.0 {
        let t = (x * x / 8.0 - 0.5) - 0.5;
        return p * cheb(n, t);
    }
    let t = if x < -log_eps {
        let numexp = ((-log_eps) / x) as u32 + 1;
        let s = sumexp(numexp, n, (-x).exp(), x);
        nf * x.ln() - x + s.ln()
    } else if x < TWO_OVER_EPS {
        let s = sumexp(1, n, 1.0, x);
        nf * x.ln() - x + s.ln()
    } else {
        nf * x.ln() - x
    };
    if t < log_eps {
        vinf(n)
    } else {
        vinf(n) - t.exp()
    }
}

// Everything above this line is generated; the test module below is written
// by hand, so re-running the generator would drop it.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::specfunc::transport as f64_transport;

    /// GSL's `GSL_LOG_DBL_EPSILON`, rounded to `f32` — the value this module
    /// deliberately does **not** use.
    const F64_LOG_EPS: f32 = -36.043653;

    /// Worst relative against the `f64` module, for one order, with a given
    /// log-epsilon.
    fn worst_rel(n: u32, log_eps: f32) -> (f64, f32) {
        let (mut worst, mut at) = (0.0_f64, 0.0_f32);
        for k in 1..=2500 {
            let x = 0.03 * k as f32;
            let r = f64_transport::transport(n, x as f64);
            if !r.is_finite() || r.abs() < 1e-6 {
                continue;
            }
            let e = (((transport_with(n, x, log_eps) as f64) - r) / r).abs();
            if e > worst {
                worst = e;
                at = x;
            }
        }
        (worst, at)
    }

    /// What `f32` costs, per order. The table is in the module
    /// documentation; every worst point sits just past `x = 4`, the
    /// Chebyshev/tail join, where the value is a limit minus a comparable
    /// tail.
    #[test]
    fn the_f32_cost_is_what_it_was_measured_to_be() {
        for n in 2..=5u32 {
            let (worst, at) = worst_rel(n, LOG_EPS);
            assert!(worst < 1e-5, "J({n}) f32 vs f64: {worst:e} at {at}");
            assert!(
                (3.0..6.0).contains(&at),
                "J({n})'s worst point is documented as sitting just past the \
                 Chebyshev/tail join at x = 4, and is at {at}"
            );
        }
        // And the cost grows with the order, which the table shows.
        assert!(
            worst_rel(5, LOG_EPS).0 > worst_rel(2, LOG_EPS).0,
            "J(5) is documented as costing more f32 error than J(2)"
        );
    }

    /// **Retargeting `GSL_LOG_DBL_EPSILON` halves the work and changes
    /// nothing else** — the refutation of this module's own first draft,
    /// which predicted it was load-bearing twice over.
    ///
    /// Three claims, all asserted:
    ///
    /// 1. The image count `numexp` roughly halves.
    /// 2. The saturation point does **not** move, because `vinf - exp(t)`
    ///    collapses to `vinf` once `exp(t)` is below half an ulp of `vinf`,
    ///    well before `t` reaches -36 — the arithmetic enforces the guard
    ///    before the guard does.
    /// 3. The worst relative error is **identical**, bit for bit.
    #[test]
    fn the_retargeting_halves_the_work_and_changes_nothing_else() {
        // 1. Half the images.
        for x in [5.0_f32, 10.0, 16.0, 30.0] {
            let ours = ((-LOG_EPS) / x) as u32 + 1;
            let theirs = ((-F64_LOG_EPS) / x) as u32 + 1;
            assert!(
                theirs >= 2 * ours - 1,
                "at x = {x} the f64 constant is documented as summing about \
                 twice as many images: {theirs} against {ours}"
            );
        }
        let total_ours: u32 = (5..=30).map(|x| ((-LOG_EPS) / x as f32) as u32 + 1).sum();
        let total_theirs: u32 = (5..=30)
            .map(|x| ((-F64_LOG_EPS) / x as f32) as u32 + 1)
            .sum();
        assert!(
            total_theirs > 3 * total_ours / 2,
            "summed over x in 5..=30 the f64 constant is documented as \
             costing far more images: {total_theirs} against {total_ours}"
        );

        // 2. The saturation point does NOT move.
        let saturates = |n: u32, le: f32| -> f32 {
            for k in 1..=4000 {
                let x = 0.05 * k as f32;
                if transport_with(n, x, le) == vinf(n) {
                    return x;
                }
            }
            f32::NAN
        };
        for (n, want) in [(2u32, 22.25_f32), (3, 25.05), (4, 27.25), (5, 29.6)] {
            let ours = saturates(n, LOG_EPS);
            let theirs = saturates(n, F64_LOG_EPS);
            assert_eq!(
                ours, theirs,
                "J({n})'s saturation point is documented as INDEPENDENT of the \
                 log-epsilon constant, because the subtraction collapses \
                 before the guard fires. It moved from {theirs} to {ours}, \
                 which would mean the guard is load-bearing after all and the \
                 module docs need rewriting"
            );
            assert!(
                (ours - want).abs() < 0.1,
                "J({n}) is documented as saturating at {want}, measured {ours}"
            );
        }

        // 3. And the accuracy is identical, bit for bit.
        for n in 2..=5u32 {
            let (a, _) = worst_rel(n, LOG_EPS);
            let (b, _) = worst_rel(n, F64_LOG_EPS);
            assert_eq!(
                a.to_bits(),
                b.to_bits(),
                "J({n})'s worst error is documented as identical with either \
                 log-epsilon: {a:e} against {b:e}"
            );
        }
    }

    /// The limits, the monotonicity and the refusals, in `f32`.
    #[test]
    fn the_shape_and_refusals_survive_f32() {
        for n in 2..=5u32 {
            assert_eq!(transport(n, 0.0), 0.0, "J({n}, 0)");
            let mut prev = 0.0_f32;
            for k in 1..=700 {
                let x = 0.05 * k as f32;
                let v = transport(n, x);
                assert!(v >= prev, "J({n}) decreased at {x}");
                assert!(v <= vinf(n) * (1.0 + 1e-6), "J({n}, {x}) exceeds its limit");
                prev = v;
            }
            assert_eq!(transport(n, 100.0), vinf(n), "J({n}) reaches its limit");
            assert!(transport(n, -1.0).is_nan(), "J({n}, -1)");
            assert!(transport(n, f32::NAN).is_nan(), "J({n}, NaN)");
        }
        for n in [0_u32, 1, 6, 100] {
            assert!(transport(n, 1.0).is_nan(), "order {n}");
        }
    }

    /// The shader and this mirror agree on the three retargeted constants,
    /// checked by parsing the WGSL source.
    #[test]
    fn the_shader_and_this_mirror_agree_on_their_constants() {
        let src = crate::wgsl::TRANSPORT;
        let read = |name: &str| -> f32 {
            let at = src
                .find(name)
                .unwrap_or_else(|| panic!("{name} not declared in transport.wgsl"));
            let rest = &src[at..];
            let eq = rest.find('=').expect("no = after the name");
            let end = rest[eq..].find(';').expect("no ; after the value");
            rest[eq + 1..eq + end]
                .trim()
                .parse::<f32>()
                .unwrap_or_else(|_| panic!("{name} is not an f32 literal"))
        };
        assert_eq!(read("PETIR_TRANSPORT_LOG_EPS: f32"), LOG_EPS);
        assert_eq!(read("PETIR_TRANSPORT_SMALL_CUT: f32"), SMALL_CUT);
        assert_eq!(read("PETIR_TRANSPORT_TWO_OVER_EPS: f32"), TWO_OVER_EPS);
        // And each really is the f32 analogue it claims to be.
        assert_eq!(LOG_EPS, f32::EPSILON.ln());
        assert_eq!(SMALL_CUT, 3.0 * f32::EPSILON.sqrt());
        assert_eq!(TWO_OVER_EPS, 2.0 / f32::EPSILON);
    }
}
