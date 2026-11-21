// SPDX-License-Identifier: AGPL-3.0-only
//
// Attribution (RESEARCH_INTEGRITY_AND_PROVENANCE.md): port of EGSnrc.
//   Upstream project: EGSnrc, https://github.com/nrc-cnrc/EGSnrc
//   Upstream file:    HEN_HOUSE/src/egs_utilities.mortran
//   Upstream commit:  bd49a47aae3938f49f4100b8e018091861408b19
//   Copyright:        National Research Council of Canada
//   Upstream licence: AGPL-3.0-or-later; this port is AGPL-3.0-only.
//! `HEN_HOUSE/src/egs_utilities.mortran` (EGSnrc).
//!
//! Upstream: <https://github.com/nrc-cnrc/EGSnrc>, commit `bd49a47aae3938f49f4100b8e018091861408b19`
//! (read 2026-11-20, a shallow clone, never committed here). Licence: AGPL-3.0
//! (the file headers read version 3 or, at the licensee's option, later;
//! the SLAC-derived parts carry their own notice in the header).
//!
//! General utility routines and macros shared by the core and the user codes (2332 lines). Description from the file's header and size; not yet read line by line.
//!
//! **Status: first port of `erf1`, `set_spline`, `spline` and `alias_sample1` only, not differentially tested (gh:#1680).**
//! The rest of the file (input parsing, I/O units, heap sort, messages) is a
//! placeholder.
//!
//! | upstream | here |
//! |---|---|
//! | `FUNCTION ERF1` (`egs_utilities.mortran:2096-2151`) | [`erf1`] |
//! | `SUBROUTINE prepare_alias_table` (`egs_utilities.mortran:1846-1895`) | [`prepare_alias_table`] |
//! | `FUNCTION alias_sample1` (`egs_utilities.mortran:1901-1936`) | [`alias_sample1`] |
//! | `SUBROUTINE set_spline`, `FUNCTION spline` (`egs_utilities.mortran:1762-1839`) | [`set_spline`], [`spline`] |

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

use petir::mathf::RealMath;

/// `ERF1` (`egs_utilities.mortran:2096-2151`): EGSnrc's own error function, a
/// Chebyshev expansion for `erf(x)` when `x <= 3` and for `erfc(x)` above
/// ("originally from Harwell, recoded in Mortran"). It exists upstream
/// because some compilers have no intrinsic `erf`; it is used by
/// `init_compton`, and kept here so that agrees with upstream to the last
/// bit instead of using `libm`'s `erf`.
pub fn erf1(x: f64) -> f64 {
    // coefficients for erf(x), x <= 3 (K = 1) and erfc(x), x > 3 (K = 2)
    const A: [[f64; 23]; 2] = [
        [
            1.0954712997776232, -0.2891754011269890, 0.1104563986337951, -0.0412531882278565,
            0.0140828380706516, -0.0043292954474314, 0.0011982719015923, -0.0002999729623532,
            0.0000683258603789, -0.0000142469884549, 0.0000027354087728, -0.0000004861912872,
            0.0000000803872762, -0.0000000124184183, 0.0000000017995326, -0.0000000002454795,
            0.0000000000316251, -0.0000000000038590, 0.0000000000004472, -0.0000000000000493,
            0.0000000000000052, -0.0000000000000005, 0.0000000000000001,
        ],
        [
            0.9750834237085559, -0.0240493938504146, 0.0008204522408804, -0.0000434293081303,
            0.0000030184470340, -0.0000002544733193, 0.0000000248583530, -0.0000000027317201,
            0.0000000003308472, 0.0000000000001464, -0.0000000000000244, 0.0000000000000042,
            -0.0000000000000008, 0.0000000000000001, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
        ],
    ];
    const NLIM: [usize; 2] = [22, 16];
    const CONST: f64 = 1.128379167095513; // 2/sqrt(pi)
    let (y, k) = if x > 3.0 { (3.0 / x, 1usize) } else { (x / 3.0, 0usize) };
    // sum of Chebyshev polynomials by backwards recursion
    let fac = 2.0 * (2.0 * y * y - 1.0);
    let mut bn1 = 0.0;
    let mut bn = 0.0;
    let mut bn2 = 0.0;
    for n in (0..=NLIM[k]).rev() {
        bn2 = bn1;
        bn1 = bn;
        bn = fac * bn1 - bn2 + A[k][n];
    }
    if k == 0 {
        CONST * y * (bn - bn1)
    } else {
        1.0 - CONST * RealMath::r_exp(-(x * x)) * (bn - bn2 + A[k][0]) / (4.0 * x)
    }
}

/// Cubic spline coefficients as `set_spline` leaves them (1-based).
#[derive(Debug, Clone, PartialEq)]
pub struct SplineCoefficients {
    /// `a(m) = f(m)`.
    pub a: Vec<f64>,
    /// `b(m)`.
    pub b: Vec<f64>,
    /// `c(m)`.
    pub c: Vec<f64>,
    /// `d(m)`.
    pub d: Vec<f64>,
}

/// `set_spline(x, f, a, b, c, d, n)` (`egs_utilities.mortran:1762-1802`):
/// natural cubic spline coefficients for the data `f(n)` at the abscissas
/// `x(n)`. Both slices are 1-based (index 0 unused), `n = x.len() - 1`.
///
/// # Panics
///
/// If `n < 3` (the natural spline needs two intervals).
pub fn set_spline(x: &[f64], f: &[f64]) -> SplineCoefficients {
    let n = x.len() - 1;
    assert!(n >= 3 && f.len() == x.len(), "set_spline needs at least three points");
    let mut a = alloc::vec![0.0; n + 1];
    let mut b = alloc::vec![0.0; n + 1];
    let mut c = alloc::vec![0.0; n + 1];
    let mut d = alloc::vec![0.0; n + 1];
    let (m1, m2) = (2usize, n - 1);
    let mut s = 0.0;
    let mut r;
    for m in 1..=m2 {
        d[m] = x[m + 1] - x[m];
        r = (f[m + 1] - f[m]) / d[m];
        c[m] = r - s;
        s = r;
    }
    s = 0.0;
    r = 0.0;
    c[1] = 0.0;
    c[n] = 0.0;
    for m in m1..=m2 {
        c[m] += r * c[m - 1];
        b[m] = 2.0 * (x[m - 1] - x[m + 1]) - r * s;
        s = d[m];
        r = s / b[m];
    }
    let mut mr = m2;
    for _ in m1..=m2 {
        c[mr] = (d[mr] * c[mr + 1] - c[mr]) / b[mr];
        mr -= 1;
    }
    for m in 1..=m2 {
        s = d[m];
        r = c[m + 1] - c[m];
        d[m] = r / s;
        c[m] *= 3.0;
        b[m] = (f[m + 1] - f[m]) / s - (c[m] + r) * s;
        a[m] = f[m];
    }
    SplineCoefficients { a, b, c, d }
}

/// `spline(s, x, a, b, c, d, n)` (`egs_utilities.mortran:1804-1839`): the
/// spline at `s`, by the coefficients of [`set_spline`]; outside the abscissas
/// the first or last interval is extrapolated. `x` is 1-based; it may be
/// ascending or descending.
pub fn spline(s: f64, x: &[f64], co: &SplineCoefficients) -> f64 {
    let n = x.len() - 1;
    let (direction, m_lower, m_upper): (usize, usize, usize) = if x[1] > x[n] { (1, n, 0) } else { (0, 0, n) };
    let m;
    if s >= x[m_upper + direction] {
        m = m_upper + 2 * direction - 1;
    } else if s <= x[m_lower + 1 - direction] {
        m = m_lower + 1 - 2 * direction;
    } else {
        // binary search for the interval s is in
        let (mut ml, mut mu) = (m_lower as i64, m_upper as i64);
        while (mu - ml).abs() > 1 {
            let mav = (ml + mu) / 2;
            if s < x[mav as usize] {
                mu = mav;
            } else {
                ml = mav;
            }
        }
        m = (mu + direction as i64 - 1) as usize;
    }
    let q = s - x[m];
    co.a[m] + q * (co.b[m] + q * (co.c[m] + q * co.d[m]))
}

/// `prepare_alias_table` (`egs_utilities.mortran:1846-1895`): the alias table
/// of the piecewise-linear density `fs` at the nodes `xs` (both `0..=nsbin`,
/// 0-based, `nsbin + 1` values), for [`alias_sample1`]. Each bin's
/// probability is its trapezoid area, floored at 1e-30. Returns `(ws, ibin)`,
/// 1-based over `1..=nsbin` (index 0 unused).
pub fn prepare_alias_table(xs: &[f64], fs: &[f64]) -> (Vec<f64>, Vec<i32>) {
    let nsbin = xs.len() - 1;
    let mut ws = alloc::vec![0.0_f64; nsbin + 1];
    let mut ibin = alloc::vec![0_i32; nsbin + 1];
    let mut sum = 0.0;
    for i in 1..=nsbin {
        let mut aux = 0.5 * (fs[i] + fs[i - 1]) * (xs[i] - xs[i - 1]);
        if aux < 1e-30 {
            aux = 1e-30;
        }
        ws[i] = -aux;
        ibin[i] = 1;
        sum += aux;
    }
    sum /= nsbin as f64;
    for i in 1..nsbin {
        // the Mortran DO loops leave the index at nsbin when no bin qualifies
        let mut j_h = nsbin;
        for jh in 1..=nsbin {
            if ws[jh] < 0.0 && ws[jh].abs() > sum {
                j_h = jh;
                break;
            }
        }
        let mut j_l = nsbin;
        for jl in 1..=nsbin {
            if ws[jl] < 0.0 && ws[jl].abs() < sum {
                j_l = jl;
                break;
            }
        }
        let aux = sum - ws[j_l].abs();
        ws[j_h] += aux;
        ws[j_l] = -ws[j_l] / sum;
        ibin[j_l] = j_h as i32;
        if i == nsbin - 1 {
            ws[j_h] = 1.0;
        }
    }
    (ws, ibin)
}

/// `alias_sample1` (`egs_utilities.mortran:1901-1936`): a value from a
/// piecewise-linear density with an alias table. `xs` and `fs` are the node
/// abscissas and densities `0..=nsbin` (0-based, `nsbin + 1` values), `ws` and
/// `ibin` the alias table (1-based, from [`prepare_alias_table`]).
pub fn alias_sample1<R: super::egsnrc_macros::UniformSource>(
    xs: &[f64],
    fs: &[f64],
    ws: &[f64],
    ibin: &[i32],
    rng: &mut R,
) -> f64 {
    let nsbin = ws.len() - 1;
    let r1 = rng.uniform();
    let r2 = rng.uniform();
    let mut aj = 1.0 + r1 * nsbin as f64;
    let mut j = aj as usize;
    aj -= j as f64;
    if aj > ws[j] {
        j = ibin[j] as usize;
    }
    let x = xs[j - 1];
    let dx = xs[j] - x;
    if fs[j - 1] > 0.0 {
        let a = fs[j] / fs[j - 1] - 1.0;
        if a.abs() < 0.2 {
            let rnno1 = 0.5 * (1.0 - r2) * a;
            x + r2 * dx * (1.0 + rnno1 * (1.0 - r2 * a))
        } else {
            x - dx / a * (1.0 - petir::real::Real::sqrt(1.0 + r2 * a * (2.0 + a)))
        }
    } else {
        x + dx * petir::real::Real::sqrt(r2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;


    fn one_based(v: &[f64]) -> Vec<f64> {
        core::iter::once(0.0).chain(v.iter().copied()).collect()
    }

    #[test]
    fn spline_interpolates_nodes_and_reproduces_a_line() {
        let xs = one_based(&[0.0, 0.7, 1.5, 2.2, 3.0, 4.1]);
        // a straight line is reproduced exactly (natural boundary conditions)
        let line: Vec<f64> = xs.iter().map(|&x| 2.0 * x + 1.0).collect();
        let co = set_spline(&xs, &line);
        for &s in &[0.1, 0.7, 1.9, 3.0, 4.0] {
            assert!((spline(s, &xs, &co) - (2.0 * s + 1.0)).abs() < 1e-12, "s {s}");
        }
        // extrapolation uses the end intervals
        assert!((spline(-1.0, &xs, &co) - (-1.0)).abs() < 1e-12);
        assert!((spline(5.0, &xs, &co) - 11.0).abs() < 1e-12);
        // passes through the data of a smooth function and approximates it between nodes
        let f: Vec<f64> = xs.iter().map(|&x| x.sin()).collect();
        let co = set_spline(&xs, &f);
        for m in 1..xs.len() {
            assert!((spline(xs[m], &xs, &co) - f[m]).abs() < 1e-12, "node {m}");
        }
        assert!((spline(1.1, &xs, &co) - 1.1f64.sin()).abs() < 5e-3);
        // a descending abscissa grid works too
        let xd: Vec<f64> = core::iter::once(0.0).chain((1..xs.len()).rev().map(|m| xs[m])).collect();
        let fd: Vec<f64> = xd.iter().map(|&x| x.sin()).collect();
        let cd = set_spline(&xd, &fd);
        for &s in &[0.3, 1.1, 2.5, 3.7] {
            assert!((spline(s, &xd, &cd) - spline(s, &xs, &set_spline(&xs, &f))).abs() < 5e-3, "s {s}");
        }
        for m in 1..xd.len() {
            assert!((spline(xd[m], &xd, &cd) - fd[m]).abs() < 1e-9, "descending node {m}");
        }
    }


    #[test]
    fn alias_sample1_samples_a_linear_density() {
        use super::super::nrcaux_mortran::prepare_alias_sampling;
        use petir::rng::ranmar::Ranmar;
        // f(x) = x on [0, 1] with five nodes; bin probabilities by trapezoids
        let xs = [0.0, 0.25, 0.5, 0.75, 1.0];
        let fs = [0.0, 0.25, 0.5, 0.75, 1.0];
        let mut probs = alloc::vec![0.0];
        for j in 1..=4 {
            probs.push(0.5 * (fs[j - 1] + fs[j]) * (xs[j] - xs[j - 1]));
        }
        let total: f64 = probs.iter().sum();
        for p in probs.iter_mut() {
            *p /= total;
        }
        let (ws, ibin) = prepare_alias_sampling(&mut probs);
        let mut rng = Ranmar::new(21, 43);
        let n = 200_000;
        let (mut mean, mut inside) = (0.0, 0);
        for _ in 0..n {
            let x = alias_sample1(&xs, &fs, &ws, &ibin, &mut rng);
            mean += x;
            if (0.0..=1.0).contains(&x) {
                inside += 1;
            }
        }
        assert_eq!(inside, n, "every sample is inside the support");
        // E[x] = 2/3 for a density proportional to x
        assert!((mean / n as f64 - 2.0 / 3.0).abs() < 0.005, "mean {}", mean / n as f64);
        // a flat density takes the near-uniform branch (probabilities perturbed off the exact
        // tie that prepare_alias_sampling cannot finalise, see nrcaux_mortran)
        let flat = [1.0, 1.0, 1.0, 1.0, 1.0];
        let mut p2 = alloc::vec![0.0, 0.2501, 0.2499, 0.2502, 0.2498];
        let (w2, i2) = prepare_alias_sampling(&mut p2);
        let mut m2 = 0.0;
        for _ in 0..n {
            m2 += alias_sample1(&xs, &flat, &w2, &i2, &mut rng);
        }
        assert!((m2 / n as f64 - 0.5).abs() < 0.005);
    }

    /// `erf1` is an accurate error function: it agrees with `libm`'s `erf`
    /// (via PETIR) to well under 1e-12 across both Chebyshev regimes.
    #[test]
    fn erf1_matches_the_error_function() {
        for k in 0..=80 {
            let x = k as f64 * 0.075;
            let want = petir::specfunc::erf(x);
            assert!((erf1(x) - want).abs() < 1e-12, "x {x}: {} vs {}", erf1(x), want);
        }
        assert!((erf1(0.0)).abs() < 1e-15);
        assert!((erf1(10.0) - 1.0).abs() < 1e-12);
        // the regime boundary at x = 3 is continuous
        assert!((erf1(3.0) - erf1(3.0 + 1e-12)).abs() < 1e-11);
    }
}
