// ---------------------------------------------------------------------------
// Ported from pba.r (probability bounds analysis in R), as used by Adolphus
// Lye's "Computing with Confidence" coherent-systems scripts.
//
//   Upstream project: pba.r — Scott Ferson (and co-authors of "pba BETTER.r")
//   Upstream repo:    https://github.com/ScottFerson/pba.r   (GPL-3.0)
//   Source read:      "pba BETTER.r" in
//                     https://github.com/Adolphus8/Computing-with-Confidence
//                     (folder "Coherent systems"), accessed 2026-10-20
//   Functions ported: KN, and, or, andI, orI, complement (not), env,
//                     frechetconv (ops +, pmin, pmax), conv (op *), pmin/pmax
//                     against a constant, cut (via `Pbox::cut`)
//   NOT ported:       sra.r (all rights reserved; never read), moments, plotting,
//                     the other ~200 pba functions.
//
// This program is free software: you can redistribute it and/or modify it under
// the terms of the GNU General Public License as published by the Free Software
// Foundation, version 3. It is distributed WITHOUT ANY WARRANTY. RAFFLES is
// AGPL-3.0-only (gh:#973), which is GPL-3.0-compatible; this file's pba.r
// lineage is GPL-3.0 and keeps that notice.
//
// Translation notes: pba.r stores a p-box as two vectors of `Pbox$steps`
// values, `u` (left bounds, quantile at (i-1)/n) and `d` (right bounds,
// quantile at i/n). RAFFLES' `Pbox` is a ladder of n+1 quantile levels
// 0, 1/n, ..., 1, so `u[i] = lower[i-1]` and `d[j] = upper[j]`, and the two
// extra entries (`upper[0]`, `lower[n]`) are the support ends pba.r never
// stores. Every operation below reproduces pba.r's arithmetic on those cells.
// ---------------------------------------------------------------------------

//! # pba.r operations on RAFFLES' [`Pbox`]
//!
//! The p-box logic needed to propagate **confidence boxes** through Boolean
//! gates: [`kn`] (the binomial c-box as pba.r builds it), the dependence-free
//! [`and_unknown`] / [`or_unknown`] (Fréchet bounds on the *uncertain
//! probabilities*), and the independence versions [`and_independent`] /
//! [`or_independent`].
//!
//! ## Two dependence questions, again
//!
//! The Boolean gate has a dependence between the **events** (do `A` and `B`
//! fail together?), and the p-boxes carry a dependence between the
//! **uncertainties** in `P(A)` and `P(B)`. pba.r's `and`/`or` make no
//! assumption about either: `and(a, b)` is the envelope of
//! `max(a + b - 1, 0)` and `min(a, b)`, each combined with Fréchet
//! convolution. `andI`/`orI` assume both are independent. This is the
//! convention of Lye et al. (2024) and the scripts; it is *not* the
//! matched-confidence-level (comonotone) combination of
//! [`crate::imprecise::SystemStructure::reliability_pbox`].
//!
//! ## Fréchet convolution on a quantile ladder
//!
//! For a non-decreasing `op` (sum, min, max) and operands with quantile
//! bounds `lower`, `upper` on the ladder `p_i = i/n`:
//!
//! ```text
//! upper_z[i] = min over a + b = n + i of  op(upper_x[a], upper_y[b])
//! lower_z[m] = max over a + b = m     of  op(lower_x[a], lower_y[b])
//! ```
//!
//! which is the Williamson–Downs bound restricted to the ladder, exactly what
//! `frechetconv.pbox` loops over. Restricting to grid pairs keeps it a valid
//! (possibly not the tightest) bound.
//!
//! No transcendental functions are used here, so nothing is routed through
//! PETIR (gh:#985): the code is `+ - * min max` and a sort.
//!
//! ## References
//!
//! - S. Ferson, pba.r, <https://github.com/ScottFerson/pba.r> (GPL-3.0).
//! - A. Lye, W. Vechgama, M. Sallak, S. Destercke, S. Ferson, S. Xiao (2024),
//!   *ASCE-ASME J. Risk Uncertainty Eng. Syst. A* 11, 04024074,
//!   doi:10.1061/AJRUA6/RUENG-1380.
//! - R. C. Williamson and T. Downs (1990). Probabilistic arithmetic I.
//!   *Int. J. Approximate Reasoning* 4, 89-158.

use crate::imprecise::{cbox_binomial, Interval, Pbox};
use crate::{RafflesError, Result};

/// pba.r's `Bzero`: `KN` clamps its result to `[1e-6, 1 - 1e-6]` "otherwise
/// beta distributions touch zero which is inconvenient in arithmetical
/// operations" (pba BETTER.r, near `km`/`KN`).
pub const PBA_BZERO: f64 = 1.0e-6;

/// The monotone binary operations the Fréchet convolution is used with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonotoneOp {
    /// `x + y`
    Add,
    /// `min(x, y)` — pba.r's `pmin`
    Min,
    /// `max(x, y)` — pba.r's `pmax`
    Max,
}

impl MonotoneOp {
    fn apply(self, x: f64, y: f64) -> f64 {
        match self {
            Self::Add => x + y,
            Self::Min => x.min(y),
            Self::Max => x.max(y),
        }
    }
}

fn bad(parameter: &str, value: f64, reason: String) -> RafflesError {
    RafflesError::InvalidParameter {
        parameter: parameter.to_string(),
        value,
        reason,
    }
}

fn same_ladder(a: &Pbox, b: &Pbox) -> Result<()> {
    if a.levels() != b.levels() {
        return Err(RafflesError::DimensionMismatch {
            expected: a.levels(),
            found: b.levels(),
        });
    }
    Ok(())
}

fn check_probability(a: &Pbox, what: &str) -> Result<()> {
    let lo = a.lower_quantiles()[0];
    let hi = a.upper_quantiles()[a.levels() - 1];
    if lo < 0.0 || hi > 1.0 {
        return Err(bad(
            what,
            if lo < 0.0 { lo } else { hi },
            "a Boolean gate needs probability p-boxes supported inside [0, 1]".to_string(),
        ));
    }
    Ok(())
}

/// A precise (zero-width) p-box holding one number — pba.r's `pbox(x)`.
pub fn point(value: f64, levels: usize) -> Result<Pbox> {
    Pbox::from_interval(Interval::point(value)?, levels)
}

/// Clamps both quantile series into `[lo, hi]`.
pub fn clamp_pbox(a: &Pbox, lo: f64, hi: f64) -> Result<Pbox> {
    Pbox::from_quantile_bounds(
        a.lower_quantiles()
            .iter()
            .map(|v| v.clamp(lo, hi))
            .collect(),
        a.upper_quantiles()
            .iter()
            .map(|v| v.clamp(lo, hi))
            .collect(),
    )
}

/// The Clopper-Pearson c-box with the **exact support ends**, no `1e-6` clamp:
/// the two edge distributions pba.r's `Sbeta(k, n-k+1)` and
/// `Sbeta(k+1, n-k)` give (what the Case study 4 script calls `c_L` and
/// `c_R`, built with `beta()` and so unclamped).
///
/// # Errors
///
/// As [`cbox_binomial`].
pub fn kn_exact(k: usize, n: usize, levels: usize) -> Result<Pbox> {
    let base = cbox_binomial(k, n, levels)?;
    let last = levels - 1;
    let mut lower = base.lower_quantiles().to_vec();
    let mut upper = base.upper_quantiles().to_vec();
    // Support ends of the two edge distributions.
    lower[0] = 0.0;
    upper[0] = 0.0;
    lower[last] = 1.0;
    upper[last] = 1.0;
    if k == n {
        // Beta(k+1, 0) is the point mass at 1 (pba.r `Sbeta`).
        upper.iter_mut().for_each(|v| *v = 1.0);
    }
    if k == 0 {
        lower.iter_mut().for_each(|v| *v = 0.0);
    }
    Pbox::from_quantile_bounds(lower, upper)
}

/// The binomial confidence box exactly as pba.r's `KN(k, n)` builds it.
///
/// **Is it [`cbox_binomial`]?** In the interior, yes: the same
/// `Beta(k, n-k+1)` / `Beta(k+1, n-k)` quantile ladders. Two differences
/// were found by reading `KN` and are reproduced here: (1) pba.r takes the
/// *exact* support ends (`qbeta(0) = 0`, `qbeta(1) = 1`) where
/// [`cbox_binomial`] evaluates the quantile at `1e-9` and `1 - 1e-9`, which
/// for `Beta(23, 2)` is about `0.3`, not `0`; and (2) pba.r clamps the
/// result to `[1e-6, 1 - 1e-6]`. This is [`kn_exact`] followed by that clamp.
/// Existing `cbox_binomial` is unchanged.
///
/// # Errors
///
/// As [`cbox_binomial`].
pub fn kn(k: usize, n: usize, levels: usize) -> Result<Pbox> {
    clamp_pbox(&kn_exact(k, n, levels)?, PBA_BZERO, 1.0 - PBA_BZERO)
}

/// `1 - a` — pba.r's `complement.pbox`. Exact, no dependence question.
pub fn complement(a: &Pbox) -> Result<Pbox> {
    let n = a.levels() - 1;
    let lower = (0..=n).map(|i| 1.0 - a.upper_quantiles()[n - i]).collect();
    let upper = (0..=n).map(|i| 1.0 - a.lower_quantiles()[n - i]).collect();
    Pbox::from_quantile_bounds(lower, upper)
}

/// pba.r's `env.pbox`: the smallest p-box containing both.
pub fn envelope(a: &Pbox, b: &Pbox) -> Result<Pbox> {
    same_ladder(a, b)?;
    let lower = a
        .lower_quantiles()
        .iter()
        .zip(b.lower_quantiles())
        .map(|(x, y)| x.min(*y))
        .collect();
    let upper = a
        .upper_quantiles()
        .iter()
        .zip(b.upper_quantiles())
        .map(|(x, y)| x.max(*y))
        .collect();
    Pbox::from_quantile_bounds(lower, upper)
}

/// pba.r's `frechetconv.pbox(x, y, op)` for a non-decreasing `op`: bounds on
/// `op(X, Y)` with **no assumption about the dependence** between `X` and `Y`.
///
/// # Errors
///
/// [`RafflesError::DimensionMismatch`] if the ladders differ.
pub fn frechet_convolve(x: &Pbox, y: &Pbox, op: MonotoneOp) -> Result<Pbox> {
    same_ladder(x, y)?;
    let n = x.levels() - 1;
    let (lx, ux) = (x.lower_quantiles(), x.upper_quantiles());
    let (ly, uy) = (y.lower_quantiles(), y.upper_quantiles());
    let mut lower = vec![0.0; n + 1];
    let mut upper = vec![0.0; n + 1];
    for i in 0..=n {
        let mut best_up = f64::INFINITY;
        for j in i..=n {
            best_up = best_up.min(op.apply(ux[j], uy[n + i - j]));
        }
        upper[i] = best_up;
        let mut best_lo = f64::NEG_INFINITY;
        for a in 0..=i {
            best_lo = best_lo.max(op.apply(lx[a], ly[i - a]));
        }
        lower[i] = best_lo;
    }
    Pbox::from_quantile_bounds(lower, upper)
}

/// Product of **independent** non-negative p-boxes — pba.r's
/// `conv.pbox(x, y, '*')`: all `n x n` cell products, sorted, every `n`-th
/// kept.
///
/// # Errors
///
/// [`RafflesError::DimensionMismatch`] for differing ladders;
/// [`RafflesError::InvalidParameter`] if an operand can be negative (the
/// sorted-cell construction is only monotone for non-negative factors).
pub fn independent_product(x: &Pbox, y: &Pbox) -> Result<Pbox> {
    same_ladder(x, y)?;
    check_probability(x, "x")?;
    check_probability(y, "y")?;
    let n = x.levels() - 1;
    let (lx, ux) = (x.lower_quantiles(), x.upper_quantiles());
    let (ly, uy) = (y.lower_quantiles(), y.upper_quantiles());

    // Left bounds: cells u_i = lower[i], i = 0..n-1; keep sorted[(k-1)n + 1].
    let mut lo_products = Vec::with_capacity(n * n);
    for a in &lx[..n] {
        for b in &ly[..n] {
            lo_products.push(a * b);
        }
    }
    lo_products.sort_by(|p, q| p.total_cmp(q));
    // Right bounds: cells d_j = upper[j], j = 1..n; keep sorted[k n].
    let mut up_products = Vec::with_capacity(n * n);
    for a in &ux[1..] {
        for b in &uy[1..] {
            up_products.push(a * b);
        }
    }
    up_products.sort_by(|p, q| p.total_cmp(q));

    let mut lower: Vec<f64> = (0..n).map(|k| lo_products[k * n]).collect();
    lower.push(lx[n] * ly[n]);
    let mut upper = vec![ux[0] * uy[0]];
    upper.extend((1..=n).map(|k| up_products[k * n - 1]));
    Pbox::from_quantile_bounds(lower, upper)
}

/// pba.r's `andI`: both events independent, and independent uncertainties.
pub fn and_independent(a: &Pbox, b: &Pbox) -> Result<Pbox> {
    independent_product(a, b)
}

/// pba.r's `orI`: `1 - (1 - a)(1 - b)` with independent operands.
pub fn or_independent(a: &Pbox, b: &Pbox) -> Result<Pbox> {
    complement(&independent_product(&complement(a)?, &complement(b)?)?)
}

fn clamp_series(a: &Pbox, lo: f64, hi: f64) -> Result<Pbox> {
    clamp_pbox(a, lo, hi)
}

/// pba.r's `and`: no assumption on dependence. The envelope of the Fréchet
/// lower bound `max(a + b - 1, 0)` and the upper bound `min(a, b)`.
pub fn and_unknown(a: &Pbox, b: &Pbox) -> Result<Pbox> {
    check_probability(a, "a")?;
    check_probability(b, "b")?;
    let sum = frechet_convolve(a, b, MonotoneOp::Add)?;
    let shifted = Pbox::from_quantile_bounds(
        sum.lower_quantiles().iter().map(|v| v - 1.0).collect(),
        sum.upper_quantiles().iter().map(|v| v - 1.0).collect(),
    )?;
    // pmax(., 0) against a point is elementwise (a monotone op with a constant).
    let floor = clamp_series(&shifted, 0.0, f64::INFINITY)?;
    envelope(&floor, &frechet_convolve(a, b, MonotoneOp::Min)?)
}

/// pba.r's `or`: no assumption on dependence. The envelope of the Fréchet
/// lower bound `max(a, b)` and upper bound `min(a + b, 1)`.
pub fn or_unknown(a: &Pbox, b: &Pbox) -> Result<Pbox> {
    check_probability(a, "a")?;
    check_probability(b, "b")?;
    let sum = frechet_convolve(a, b, MonotoneOp::Add)?;
    let capped = clamp_series(&sum, f64::NEG_INFINITY, 1.0)?;
    envelope(&frechet_convolve(a, b, MonotoneOp::Max)?, &capped)
}

#[cfg(test)]
mod tests {
    use super::*;

    const LEVELS: usize = 201;

    fn pt(v: f64) -> Pbox {
        point(v, LEVELS).unwrap()
    }

    /// **Methodology.** Precise inputs must collapse every gate to the plain
    /// probability formula (pba.r with point p-boxes). `p = 0.3`, `q = 0.6`:
    /// independent AND `0.18`, OR `0.72`; Fréchet AND `[0, 0.3]`, OR
    /// `[0.6, 0.9]`. Pass criterion: exact to `1e-12` (no sampling involved).
    ///
    /// **Result** (2026-10-20): independent `0.18`, `0.72` exact; Fréchet
    /// bounds `[0, 0.3]` and `[0.6, 0.9]` exact.
    #[test]
    fn precise_inputs_reduce_to_probability_formulas() {
        let (p, q) = (pt(0.3), pt(0.6));
        let ai = and_independent(&p, &q).unwrap();
        let oi = or_independent(&p, &q).unwrap();
        for i in 0..LEVELS {
            assert!((ai.lower_quantiles()[i] - 0.18).abs() < 1e-12);
            assert!((ai.upper_quantiles()[i] - 0.18).abs() < 1e-12);
            assert!((oi.lower_quantiles()[i] - 0.72).abs() < 1e-12);
            assert!((oi.upper_quantiles()[i] - 0.72).abs() < 1e-12);
        }
        let au = and_unknown(&p, &q).unwrap();
        let ou = or_unknown(&p, &q).unwrap();
        assert!(au.lower_quantiles().iter().all(|v| v.abs() < 1e-12));
        assert!(au.upper_quantiles().iter().all(|v| (v - 0.3).abs() < 1e-12));
        assert!(ou.lower_quantiles().iter().all(|v| (v - 0.6).abs() < 1e-12));
        assert!(ou.upper_quantiles().iter().all(|v| (v - 0.9).abs() < 1e-12));
    }

    /// **Methodology.** `complement` is an involution and swaps the series;
    /// `envelope` of a box with itself is the box; `clamp_pbox` bounds both
    /// series. Identities (complement to rounding, 1e-14).
    ///
    /// **Result** (2026-10-20): all hold.
    #[test]
    fn complement_envelope_clamp_identities() {
        let a = kn(3, 10, LEVELS).unwrap();
        // 1 - (1 - x) is exact only up to rounding, so compare to 1e-14.
        let twice = complement(&complement(&a).unwrap()).unwrap();
        for i in 0..LEVELS {
            assert!((twice.lower_quantiles()[i] - a.lower_quantiles()[i]).abs() < 1e-14);
            assert!((twice.upper_quantiles()[i] - a.upper_quantiles()[i]).abs() < 1e-14);
        }
        assert_eq!(envelope(&a, &a).unwrap(), a);
        let c = clamp_pbox(&a, 0.2, 0.5).unwrap();
        assert!(c.lower_quantiles().iter().all(|v| (0.2..=0.5).contains(v)));
        assert!(c.upper_quantiles().iter().all(|v| (0.2..=0.5).contains(v)));
    }

    /// **Methodology.** `kn` equals `cbox_binomial` in the interior and
    /// differs only at the two support ends and the `1e-6` clamp; the 95 %
    /// Clopper-Pearson interval for 23 of 24 is the cut. Reference: the exact
    /// Clopper-Pearson limits are the Beta quantiles `cbox_binomial` already
    /// uses (checked against it level by level).
    ///
    /// **Result** (2026-10-20): interior levels 1..=199 identical; ends
    /// `lower[0] = 1e-6`, `upper[200] = 1 - 1e-6`; `KN(0, 460)` lower series
    /// is `1e-6` throughout (pba.r: `pbox(0)` clamped).
    #[test]
    fn kn_matches_cbox_binomial_in_the_interior() {
        let k = kn(23, 24, LEVELS).unwrap();
        let c = cbox_binomial(23, 24, LEVELS).unwrap();
        for i in 1..LEVELS - 1 {
            assert_eq!(k.lower_quantiles()[i], c.lower_quantiles()[i]);
            assert_eq!(k.upper_quantiles()[i], c.upper_quantiles()[i]);
        }
        assert_eq!(k.lower_quantiles()[0], PBA_BZERO);
        assert_eq!(k.upper_quantiles()[LEVELS - 1], 1.0 - PBA_BZERO);
        let z = kn(0, 460, LEVELS).unwrap();
        assert!(z.lower_quantiles().iter().all(|v| *v == PBA_BZERO));
        let all = kn(12, 12, LEVELS).unwrap();
        assert!(all.upper_quantiles().iter().all(|v| *v == 1.0 - PBA_BZERO));
    }

    /// **Methodology.** Fréchet bounds must enclose the independence result
    /// for every operand (a distribution-free bound contains the independent
    /// case) and the independent AND of two uncertain boxes must lie inside
    /// `[0, 1]`; mismatched ladders and negative support are rejected.
    ///
    /// **Result** (2026-10-20): enclosure holds at every one of 201 levels
    /// for AND and OR of `KN(23,24)` with `KN(14,17)`.
    #[test]
    fn frechet_encloses_independence_and_bad_inputs_are_rejected() {
        let a = kn(23, 24, LEVELS).unwrap();
        let b = kn(14, 17, LEVELS).unwrap();
        for (unk, ind) in [
            (
                and_unknown(&a, &b).unwrap(),
                and_independent(&a, &b).unwrap(),
            ),
            (or_unknown(&a, &b).unwrap(), or_independent(&a, &b).unwrap()),
        ] {
            for i in 0..LEVELS {
                assert!(unk.lower_quantiles()[i] <= ind.lower_quantiles()[i] + 1e-12);
                assert!(unk.upper_quantiles()[i] + 1e-12 >= ind.upper_quantiles()[i]);
            }
        }
        let short = kn(1, 2, 11).unwrap();
        assert!(and_unknown(&a, &short).is_err());
        assert!(and_independent(&a, &short).is_err());
        assert!(envelope(&a, &short).is_err());
        let neg = Pbox::from_interval(Interval::new(-1.0, 0.5).unwrap(), LEVELS).unwrap();
        assert!(and_independent(&neg, &a).is_err());
        assert!(or_unknown(&neg, &a).is_err());
    }
}
