// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// NOT ported from RAVEN. Added 2026-10-17 for GitHub issue #944 (needed by
// #927, epic #925) at the WORKSPACE MAINTAINER's direction; the crate owner's
// (Adolphus Lye's) review is outstanding — see this crate's CLAUDE.md,
// "Scope boundaries". The formulas are the published ones; the CHECKED
// reference values come from SciPy (see the tests), but SciPy was NOT the
// source of the implementation.

//! **Sample skewness, excess kurtosis and a normality test** — is a window of a
//! trace (a `k_eff` or Shannon-entropy history, say) plausibly Gaussian?
//!
//! Generic statistics on a slice of numbers. Deciding which trace to test, and
//! what to do with the verdict, stays with the caller (the Auto `k_eff`
//! watcher of #927 asks for normality *and* stationarity, the latter from
//! [`super::stationarity`]).
//!
//! # What is computed
//!
//! With central moments `m_j = Σ(x − x̄)^j / n`:
//!
//! - **Moment skewness** `g1 = m3 / m2^{3/2}` and **moment excess kurtosis**
//!   `g2 = m4 / m2² − 3`. Both are biased in small samples (`E[g2] =
//!   −6/(n+1)` for a normal population: about −0.19 at `n = 30`).
//! - **Bias-adjusted** `G1 = g1 √(n(n−1)) / (n−2)` and
//!   `G2 = ((n+1) g2 + 6)(n−1) / ((n−2)(n−3))` (the SAS/SPSS/Excel and
//!   SciPy `bias=False` estimators; Joanes & Gill 1998).
//! - **Standard errors under normality** of `g1` and `g2`: the *exact*
//!   variances `Var(g1) = 6(n−2)/((n+1)(n+3))` and
//!   `Var(g2) = 24n(n−2)(n−3)/((n+1)²(n+3)(n+5))`, which are also the
//!   variances the normality test below standardises by. They are standard
//!   errors *of the statistic if the data were normal* — the yardstick for
//!   "is this skew large?", not a confidence interval for a non-normal
//!   population's skew.
//!
//! # Which normality test, and why (windows of 30–50)
//!
//! The **D'Agostino–Pearson omnibus test** `K² = Z(g1)² + Z(g2)²`, where
//! `Z(g1)` is D'Agostino's (1970) normalising transformation of the skewness
//! and `Z(g2)` the Anscombe–Glynn (1983) transformation of the kurtosis. Under
//! normality `K²` is approximately `χ²` with 2 degrees of freedom, whose upper
//! tail is exactly `p = exp(−K²/2)`.
//!
//! Jarque–Bera (`n/6 · (g1² + g2²/4)`) was **not** chosen: its `χ²₂` reference
//! law is asymptotic and is poor at `n` of a few tens (the skewness and
//! kurtosis converge to normality slowly, and `g2` is itself strongly skewed
//! at small `n`), which is why Jarque–Bera is normally used with simulated
//! critical values at small `n`. The D'Agostino transforms exist precisely to
//! make `Z(g1)` and `Z(g2)` standard normal at modest `n`: the skewness
//! transform is specified for `n ≥ 8` and the kurtosis transform for `n ≥ 20`
//! (the `n ≥ 20` floor is enforced here). The size of the test at `n = 30`
//! and `n = 50` is measured in the tests below rather than assumed.
//!
//! **Measured size (2026-10-17): mildly liberal.** At nominal `alpha = 0.05`
//! the rejection rate on N(0,1) windows is about **0.057** (SciPy 1.18.1,
//! 200 000 windows: 0.0568 at n = 20, 0.0576 at n = 30, 0.0572 at n = 50,
//! se 0.0005; this crate's own generator, 20 000 windows: 0.0570 at n = 30,
//! 0.0530 at n = 50, se 0.0015). The `chi^2_2` law is an approximation, so
//! `p_value` is slightly too small; treat `alpha = 0.05` as roughly 0.057.
//!
//! # Limits
//!
//! - `K²` is an omnibus **moment** test: it detects skew and tail weight, not,
//!   for example, bimodality that preserves the first four moments, and it
//!   has little power against mild non-normality at `n = 30`. "Passed" means
//!   "no evidence against", never "is Gaussian".
//! - It assumes **independent** values. A correlated trace (power-iteration
//!   `k_eff`) violates that; the test's size is then not the nominal one.
//!   Thin or batch the trace first ([`super::autocorrelation`]) or read the
//!   result accordingly.
//! - A constant window has zero variance and no defined skewness: an `Err`.
//!
//! # References
//!
//! **Not yet verified against the literature corpus** — none of these is in
//! `reactor-literature` as of 2026-10-17 (searched by author name; the
//! submodule is also empty in agent worktrees), and the bibliographic details
//! below are given as the implementation's source of the formulas, to be
//! checked against the papers by a human:
//!
//! - R. B. D'Agostino (1970). Transformation to normality of the null
//!   distribution of `g1`. *Biometrika* 57(3), 679–681.
//! - F. J. Anscombe and W. J. Glynn (1983). Distribution of the kurtosis
//!   statistic `b2` for normal samples. *Biometrika* 70(1), 227–234.
//! - R. B. D'Agostino and E. S. Pearson (1973). Tests for departure from
//!   normality. Empirical results for the distributions of `b2` and `√b1`.
//!   *Biometrika* 60(3), 613–622 (the omnibus `K²`).
//! - D. N. Joanes and C. A. Gill (1998). Comparing measures of sample skewness
//!   and kurtosis. *The Statistician* 47(1), 183–189 (`G1`, `G2`).
//!
//! Cross-checked numerically against SciPy 1.18.1 `skew`, `kurtosis`,
//! `skewtest`, `kurtosistest`, `normaltest`; see the test docs.
//!
//! # Status
//!
//! No human V&V, like the rest of this crate.

use petir::mathf::RealMath;
use crate::{RafflesError, Result};

/// Minimum sample size of [`dagostino_pearson`]: the Anscombe–Glynn kurtosis
/// transform is specified for `n ≥ 20`.
pub const MIN_N_NORMALITY_TEST: usize = 20;

/// Minimum sample size for the bias-adjusted `G2` and its variance (`n − 3`
/// appears in a denominator), and so for [`moments`].
pub const MIN_N_MOMENTS: usize = 4;

/// Sample skewness and excess kurtosis with their null standard errors.
#[derive(Debug, Clone, PartialEq)]
pub struct SampleMoments {
    /// Sample size.
    pub n: usize,
    /// Sample mean.
    pub mean: f64,
    /// Moment skewness `g1 = m3 / m2^{3/2}`.
    pub skewness: f64,
    /// Moment excess kurtosis `g2 = m4 / m2² − 3`.
    pub excess_kurtosis: f64,
    /// Bias-adjusted skewness `G1`.
    pub skewness_adjusted: f64,
    /// Bias-adjusted excess kurtosis `G2`.
    pub excess_kurtosis_adjusted: f64,
    /// Standard error of `g1` if the data were normal.
    pub se_skewness: f64,
    /// Standard error of `g2` if the data were normal.
    pub se_excess_kurtosis: f64,
}

/// Result of the D'Agostino–Pearson omnibus normality test.
#[derive(Debug, Clone, PartialEq)]
pub struct NormalityTest {
    /// Sample size.
    pub n: usize,
    /// Normalised skewness statistic `Z(g1)`, ~`N(0,1)` under normality.
    pub z_skewness: f64,
    /// Normalised kurtosis statistic `Z(g2)`, ~`N(0,1)` under normality.
    pub z_kurtosis: f64,
    /// Omnibus `K² = Z(g1)² + Z(g2)²`, ~`χ²₂` under normality.
    pub k2: f64,
    /// Upper-tail p-value `exp(−K²/2)`.
    pub p_value: f64,
}

impl NormalityTest {
    /// `true` when normality is rejected at significance level `alpha`
    /// (`p < alpha`). `alpha` must be in `(0, 1)`.
    pub fn rejects_at(&self, alpha: f64) -> Result<bool> {
        if !(alpha > 0.0 && alpha < 1.0) {
            return Err(RafflesError::InvalidParameter {
                parameter: "alpha".into(),
                value: alpha,
                reason: "significance level must lie in (0, 1)".into(),
            });
        }
        Ok(self.p_value < alpha)
    }
}

/// Mean and central moments `(mean, m2, m3, m4)`; errors on short, non-finite
/// or constant input.
fn central_moments(x: &[f64], min_n: usize) -> Result<(f64, f64, f64, f64)> {
    let n = x.len();
    if n < min_n {
        return Err(RafflesError::InvalidParameter {
            parameter: "x.len()".into(),
            value: n as f64,
            reason: format!("need at least {min_n} samples"),
        });
    }
    if let Some(bad) = x.iter().find(|v| !v.is_finite()) {
        return Err(RafflesError::InvalidParameter {
            parameter: "x".into(),
            value: *bad,
            reason: "samples must be finite".into(),
        });
    }
    let nf = n as f64;
    let mean = x.iter().sum::<f64>() / nf;
    let (mut s2, mut s3, mut s4) = (0.0, 0.0, 0.0);
    for v in x {
        let d = v - mean;
        let d2 = d * d;
        s2 += d2;
        s3 += d2 * d;
        s4 += d2 * d2;
    }
    let (m2, m3, m4) = (s2 / nf, s3 / nf, s4 / nf);
    // Exactly constant data have m2 == 0; rounding noise on a constant window
    // is caught by the relative test.
    if m2 <= (f64::EPSILON * mean.abs()).r_powi(2) {
        return Err(RafflesError::InvalidParameter {
            parameter: "x".into(),
            value: m2,
            reason: "zero variance: skewness and kurtosis are undefined".into(),
        });
    }
    Ok((mean, m2, m3, m4))
}

/// Sample skewness and excess kurtosis of `x`, with null standard errors.
///
/// Needs `n ≥ 4` finite, non-constant values ([`MIN_N_MOMENTS`]). See the
/// module docs for the definitions.
///
/// ```
/// let m = raffles::estimators::normality::moments(&[1.0, 2.0, 3.0, 4.0, 10.0]).unwrap();
/// assert!((m.skewness - 36.0 / 31.622776601683793).abs() < 1e-12);
/// assert!((m.excess_kurtosis - (-0.212)).abs() < 1e-12);
/// ```
pub fn moments(x: &[f64]) -> Result<SampleMoments> {
    let (mean, m2, m3, m4) = central_moments(x, MIN_N_MOMENTS)?;
    let n = x.len() as f64;
    let g1 = m3 / m2.r_powf(1.5);
    let g2 = m4 / (m2 * m2) - 3.0;
    let g1_adj = g1 * (n * (n - 1.0)).sqrt() / (n - 2.0);
    let g2_adj = ((n + 1.0) * g2 + 6.0) * (n - 1.0) / ((n - 2.0) * (n - 3.0));
    Ok(SampleMoments {
        n: x.len(),
        mean,
        skewness: g1,
        excess_kurtosis: g2,
        skewness_adjusted: g1_adj,
        excess_kurtosis_adjusted: g2_adj,
        se_skewness: var_g1(n).sqrt(),
        se_excess_kurtosis: var_b2(n).sqrt(),
    })
}

/// Exact `Var(g1)` for a normal sample of size `n`.
fn var_g1(n: f64) -> f64 {
    6.0 * (n - 2.0) / ((n + 1.0) * (n + 3.0))
}

/// Exact `Var(b2) = Var(g2)` for a normal sample of size `n`.
fn var_b2(n: f64) -> f64 {
    24.0 * n * (n - 2.0) * (n - 3.0) / ((n + 1.0) * (n + 1.0) * (n + 3.0) * (n + 5.0))
}

/// D'Agostino (1970) normalising transform of `g1` for sample size `n`.
fn z_from_skewness(g1: f64, n: f64) -> f64 {
    let y = g1 * ((n + 1.0) * (n + 3.0) / (6.0 * (n - 2.0))).sqrt();
    let beta2 = 3.0 * (n * n + 27.0 * n - 70.0) * (n + 1.0) * (n + 3.0)
        / ((n - 2.0) * (n + 5.0) * (n + 7.0) * (n + 9.0));
    let w2 = -1.0 + (2.0 * (beta2 - 1.0)).sqrt();
    let delta = 1.0 / (0.5 * w2.r_ln()).sqrt();
    let alpha = (2.0 / (w2 - 1.0)).sqrt();
    delta * (y / alpha).asinh()
}

/// Anscombe–Glynn (1983) normalising transform of `b2 = g2 + 3`.
fn z_from_kurtosis(b2: f64, n: f64) -> f64 {
    let e = 3.0 * (n - 1.0) / (n + 1.0);
    let x = (b2 - e) / var_b2(n).sqrt();
    let sqrt_beta1 = 6.0 * (n * n - 5.0 * n + 2.0) / ((n + 7.0) * (n + 9.0))
        * (6.0 * (n + 3.0) * (n + 5.0) / (n * (n - 2.0) * (n - 3.0))).sqrt();
    let a = 6.0
        + 8.0 / sqrt_beta1 * (2.0 / sqrt_beta1 + (1.0 + 4.0 / (sqrt_beta1 * sqrt_beta1)).sqrt());
    let term1 = 1.0 - 2.0 / (9.0 * a);
    let denom = 1.0 + x * (2.0 / (a - 4.0)).sqrt();
    // Cube root with the sign of `denom`; denom == 0 is the measure-zero
    // boundary where the transform is undefined, mapped to the term it tends to.
    let term2 = if denom == 0.0 {
        0.0
    } else {
        denom.signum() * ((1.0 - 2.0 / a) / denom.abs()).r_cbrt()
    };
    (term1 - term2) / (2.0 / (9.0 * a)).sqrt()
}

/// D'Agostino–Pearson omnibus normality test on `x`.
///
/// Needs `n ≥ 20` finite, non-constant values ([`MIN_N_NORMALITY_TEST`]).
/// `p_value = exp(−K²/2)`; reject normality when it falls below the chosen
/// level (see [`NormalityTest::rejects_at`]). See the module docs for the
/// choice of test, its references and its limits.
///
/// ```
/// let x: Vec<f64> = (1..=30).map(|i| (i * i) as f64).collect(); // right-skewed
/// let t = raffles::estimators::normality::dagostino_pearson(&x).unwrap();
/// assert!((t.p_value - 0.145278851325707).abs() < 1e-9); // SciPy normaltest
/// ```
pub fn dagostino_pearson(x: &[f64]) -> Result<NormalityTest> {
    let (_, m2, m3, m4) = central_moments(x, MIN_N_NORMALITY_TEST)?;
    let n = x.len() as f64;
    let g1 = m3 / m2.r_powf(1.5);
    let b2 = m4 / (m2 * m2);
    let z1 = z_from_skewness(g1, n);
    let z2 = z_from_kurtosis(b2, n);
    let k2 = z1 * z1 + z2 * z2;
    Ok(NormalityTest {
        n: x.len(),
        z_skewness: z1,
        z_kurtosis: z2,
        k2,
        p_value: (-0.5 * k2).r_exp(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::distributions::{ContinuousDistribution1D, Normal};
    use petir::rng::lcg::prn;

    /// Moments of `[1, 2, 3, 4, 10]` by hand: mean 4, deviations
    /// `-3 -2 -1 0 6`, `m2 = 10`, `m3 = 36`, `m4 = 278.8`, so
    /// `g1 = 36 / 10^{1.5} = 1.138420`, `g2 = 2.788 − 3 = −0.212`. SciPy 1.18.1
    /// gives `skew = 1.13841995766062`, `kurtosis = −0.212`,
    /// `G1 = 1.69705627484771`, `G2 = 3.152`. Exact agreement to 1e-12 is the
    /// pass criterion (pure arithmetic). Also pins the null standard errors:
    /// `Var(g1) = 6·3/(6·8) = 0.375`.
    #[test]
    fn moments_match_hand_calculation_and_scipy() {
        let m = moments(&[1.0, 2.0, 3.0, 4.0, 10.0]).unwrap();
        assert_eq!(m.n, 5);
        assert!((m.mean - 4.0).abs() < 1e-14);
        assert!((m.skewness - 1.13841995766062).abs() < 1e-12);
        assert!((m.excess_kurtosis + 0.212).abs() < 1e-12);
        assert!((m.skewness_adjusted - 1.69705627484771).abs() < 1e-12);
        assert!((m.excess_kurtosis_adjusted - 3.152).abs() < 1e-12);
        assert!((m.se_skewness - 0.375_f64.sqrt()).abs() < 1e-14);
        let v4: f64 = 24.0 * 5.0 * 3.0 * 2.0 / (36.0 * 8.0 * 10.0);
        assert!((m.se_excess_kurtosis - v4.sqrt()).abs() < 1e-14);
    }

    /// Reference script (SciPy 1.18.1, NumPy 2.5.4), datasets and output:
    ///
    /// ```text
    /// import numpy as np, scipy.stats as st
    /// B = [i*i for i in range(1,31)]            # n=30
    /// C = [(i**3+5*i)%37 for i in range(1,31)]  # n=30
    /// D = [i**3 for i in range(1,41)]           # n=40
    /// E = [(i%7)+i/10 for i in range(1,26)]     # n=25
    /// # per set: st.skew, st.kurtosis, st.skewtest -> Z1, st.kurtosistest -> Z2,
    /// #          st.normaltest -> (K2, p)
    /// B: g1=0.619012034524689 g2=-0.883313880125901 Z1=1.54792678040111
    ///    Z2=-1.20918287454185 K2=3.85820054156824 p=0.145278851325707
    /// C: g1=0.206847163642226 g2=-1.52801364741041 Z1=0.538613393852703
    ///    Z2=-4.23494635059425 K2=18.2248749804491 p=0.000110285567249618
    /// D: g1=1.04019163811587 g2=-0.136145616033098 Z1=2.69261622917092
    ///    Z2=0.24310795651147 K2=7.30928363611381 p=0.025870762263725
    /// E: g1=0.029411723817082 g2=-1.07996646049036 Z1=0.07180304603766
    ///    Z2=-1.59630254196158 K2=2.5533374828933 p=0.278965059032365
    /// ```
    ///
    /// Pass criterion: every printed quantity within 1e-9 (relative to
    /// magnitude) of SciPy's; the implementation is independent of SciPy's
    /// source, so agreement checks the formulas as transcribed.
    ///
    /// A first draft used the symmetric permutation `(37 i) mod 31`
    /// (`g1 = 0` exactly). SciPy 1.18.1 `skewtest` returns `Z1 = 1.039` for
    /// it, though `Z1 = 0` follows from the formula: SciPy replaces `y == 0`
    /// by `1.0` (`y = xp.where(y == 0, 1., y)` in `_stats_py.py`). This
    /// implementation returns 0 there; that dataset was dropped as a
    /// reference rather than reproduce SciPy's quirk.
    #[test]
    fn dagostino_pearson_matches_scipy() {
        type Case = (Vec<f64>, f64, f64, f64, f64, f64);
        let cases: Vec<Case> = vec![
            (
                (1..=30).map(|i| (i * i) as f64).collect(),
                1.54792678040111,
                -1.20918287454185,
                3.85820054156824,
                0.145278851325707,
                0.619012034524689,
            ),
            (
                (1..=30i64)
                    .map(|i| ((i * i * i + 5 * i) % 37) as f64)
                    .collect(),
                0.538613393852703,
                -4.23494635059425,
                18.2248749804491,
                0.000110285567249618,
                0.206847163642226,
            ),
            (
                (1..=40).map(|i: i64| i.pow(3) as f64).collect(),
                2.69261622917092,
                0.24310795651147,
                7.30928363611381,
                0.025870762263725,
                1.04019163811587,
            ),
            (
                (1..=25).map(|i| (i % 7) as f64 + i as f64 / 10.0).collect(),
                0.07180304603766,
                -1.59630254196158,
                2.5533374828933,
                0.278965059032365,
                0.029411723817082,
            ),
        ];
        for (x, z1, z2, k2, p, g1) in cases {
            let t = dagostino_pearson(&x).unwrap();
            let close = |a: f64, b: f64| (a - b).abs() <= 1e-9 * (1.0 + b.abs());
            assert!(close(t.z_skewness, z1), "Z1 {} vs {z1}", t.z_skewness);
            assert!(close(t.z_kurtosis, z2), "Z2 {} vs {z2}", t.z_kurtosis);
            assert!(close(t.k2, k2), "K2 {} vs {k2}", t.k2);
            assert!(close(t.p_value, p), "p {} vs {p}", t.p_value);
            assert!((moments(&x).unwrap().skewness - g1).abs() < 1e-9);
        }
    }

    /// Error paths: too short, constant, non-finite, bad alpha. Each must be a
    /// `Result::Err`, never a panic or a NaN.
    #[test]
    fn invalid_input_is_an_error() {
        assert!(moments(&[1.0, 2.0, 3.0]).is_err());
        assert!(moments(&[2.0; 10]).is_err());
        assert!(moments(&[1.0, 2.0, f64::NAN, 4.0, 5.0]).is_err());
        assert!(dagostino_pearson(&[1.0, 2.0, 5.0, 9.0, 4.0, 3.0, 1.0, 1.0, 2.0, 8.0]).is_err());
        assert!(dagostino_pearson(&[3.0; 40]).is_err());
        let x: Vec<f64> = (1..=30).map(|i| (i * i) as f64).collect();
        let t = dagostino_pearson(&x).unwrap();
        assert!(t.rejects_at(0.0).is_err());
        assert!(t.rejects_at(1.0).is_err());
        assert!(!t.rejects_at(0.05).unwrap()); // p = 0.145
        assert!(t.rejects_at(0.2).unwrap());
    }

    /// Draw `n` values from N(0,1) by inverse CDF of raffles' (petir) LCG.
    fn std_normal(n: usize, seed: &mut u64) -> Vec<f64> {
        let d = Normal::new(0.0, 1.0).unwrap();
        (0..n)
            .map(|_| d.ppf(prn(seed).clamp(1e-12, 1.0 - 1e-12)).unwrap())
            .collect()
    }

    /// **Size check.** 20 000 independent non-overlapping windows of `n` draws
    /// from N(0,1) (raffles' own `Normal::ppf` over petir's LCG, fixed seed
    /// 12345), rejection rate of `K^2` at `alpha = 0.05`.
    ///
    /// **Result (2026-10-17): the test is mildly liberal, not nominal.** The
    /// first criterion, written before running, was "rate within 4 binomial
    /// standard errors of 0.05"; it FAILED (n = 30: 0.0570 +/- 0.0015). It was
    /// not loosened. Instead the question "is that our implementation or the
    /// test?" was put to an independent instrument: SciPy 1.18.1
    /// `normaltest` on NumPy `default_rng(2026)` N(0,1) draws,
    /// 200 000 windows each:
    ///
    /// ```text
    /// n=20 rate=0.05683   n=30 rate=0.05758   n=50 rate=0.05715   (se 0.00049)
    /// ```
    ///
    /// So the chi-squared(2) reference law for `K^2` over-rejects by about 0.7
    /// percentage points (15 % relative) at n = 20-50, in SciPy as well; the
    /// finding is a property of the D'Agostino-Pearson test at small n
    /// (`Z(g1)` and `Z(g2)` are uncorrelated but not independent, so their
    /// squares do not sum to an exact chi-squared(2)), not a defect here.
    ///
    /// The gate is therefore: the measured rate agrees with SciPy's
    /// independently measured rate (0.0576 at n = 30, 0.0572 at n = 50) within
    /// 4 combined standard errors, AND lies above 0.05 - 4 se (it must not be
    /// conservative). Callers needing exact size at these n must calibrate the
    /// `K^2` critical value by simulation instead of using `p_value`.
    #[test]
    fn size_at_alpha_005_matches_scipy_and_is_mildly_liberal() {
        let windows = 20_000usize;
        let se = (0.05f64 * 0.95 / windows as f64).sqrt();
        let se_scipy = (0.057f64 * 0.943 / 200_000.0).sqrt();
        let se_comb = (se * se + se_scipy * se_scipy).sqrt();
        let mut seed = 12345u64;
        for (n, scipy_rate) in [(30usize, 0.05758f64), (50, 0.05715)] {
            let mut rej = 0usize;
            for _ in 0..windows {
                let x = std_normal(n, &mut seed);
                if dagostino_pearson(&x).unwrap().rejects_at(0.05).unwrap() {
                    rej += 1;
                }
            }
            let rate = rej as f64 / windows as f64;
            println!("size n={n}: rate {rate:.5} +/- {se:.5} (nominal 0.05, scipy {scipy_rate})");
            assert!(
                (rate - scipy_rate).abs() < 4.0 * se_comb,
                "n={n}: rate {rate} vs scipy {scipy_rate} +/- {se_comb}"
            );
            assert!(rate > 0.05 - 4.0 * se, "n={n}: conservative: {rate}");
        }
    }

    /// **Power sanity** (not a gate on size): a uniform window of 50 has
    /// excess kurtosis −1.2 and must be rejected far more often than 5 %.
    #[test]
    fn uniform_windows_are_rejected_more_often_than_nominal() {
        let mut seed = 777u64;
        let windows = 2000;
        let mut rej = 0;
        for _ in 0..windows {
            let x: Vec<f64> = (0..50).map(|_| prn(&mut seed)).collect();
            if dagostino_pearson(&x).unwrap().rejects_at(0.05).unwrap() {
                rej += 1;
            }
        }
        let rate = rej as f64 / windows as f64;
        println!("power uniform n=50: {rate:.4}");
        assert!(rate > 0.3, "rate {rate}");
    }
}
