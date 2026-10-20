// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.
//
// ---------------------------------------------------------------------------
// PORTED (NOT from GSL) from `rand_distr` 0.6.0, the version boon-lay's
// Cargo.lock pinned on 2026-10-20:
//
//   src/normal.rs (`StandardNormal` for f64 with its `pdf` and `zero_case`,
//                  `Normal::new`, `Normal::from_zscore`, `Normal`'s `sample`,
//                  `normal::Error`)
//   src/utils.rs  (`ziggurat`, Doornik's ZIGNOR variant)
//
// Licence: MIT OR Apache-2.0 (used here under MIT). Copyright lines as
// upstream states them (LICENSE-MIT and the file headers):
//
//   Copyright 2018 Developers of the Rand project
//   Copyright 2013 The Rust Project Developers (normal.rs)
//
//   Permission is hereby granted, free of charge, to any person obtaining a
//   copy of this software and associated documentation files (the
//   "Software"), to deal in the Software without restriction, including
//   without limitation the rights to use, copy, modify, merge, publish,
//   distribute, sublicense, and/or sell copies of the Software, and to permit
//   persons to whom the Software is furnished to do so, subject to the
//   following conditions:
//
//   The above copyright notice and this permission notice shall be included
//   in all copies or substantial portions of the Software.
//
//   THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
//   OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
//   MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN
//   NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
//   DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR
//   OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE
//   USE OR OTHER DEALINGS IN THE SOFTWARE.
//
// The flow is ONE-WAY (MIT into this AGPL-3.0-only crate). See NOTICE.
// Maintainer direction: gh:#995, 2026-10-20.
// ---------------------------------------------------------------------------

//! **Normal distribution** by the ziggurat method — `rand_distr` 0.6.0's
//! `Normal<f64>` and `StandardNormal`, bit for bit.
//!
//! # The two transcendentals, and which `exp`/`ln` they are
//!
//! The ziggurat calls `exp` (the density, on the ~1 % of draws that land in
//! a layer's wedge) and `ln` (the tail beyond `ZIG_NORM_R` = 3.654, on about
//! 2.6e-4 of draws). Upstream reaches them through `num_traits::Float`, which
//! is **`libm`** under `rand_distr`'s own feature set (`num-traits` with
//! `libm`, no `std`) and the **platform libm** whenever something else in the
//! build turns on `num-traits/std`. PETIR calls `Real::exp` / `Real::ln`,
//! which are `libm` everywhere, so it matches upstream as `rand_distr`
//! declares itself and is the same on every platform.
//! `tests/rng_vs_rand.rs` records how often the platform-libm build differs.

#[allow(unused_imports)]
use crate::real::Real;

use super::traits::{Rng, RngExt};
use super::uniform::{into_float_with_exponent, open01};
use super::ziggurat_tables::{ZigTable, ZIG_NORM_F, ZIG_NORM_R, ZIG_NORM_X};

/// Why a [`Normal`] could not be built.
///
/// **Upstream:** `rand_distr` `normal.rs`, `normal::Error` (`MeanTooSmall`
/// belongs to `LogNormal`, which is not ported).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NormalError {
    /// The standard deviation is not finite.
    BadVariance,
}

impl core::fmt::Display for NormalError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            NormalError::BadVariance => {
                "variation parameter is non-finite in (log)normal distribution"
            }
        })
    }
}

/// Sample by the ziggurat method (Doornik 2005, ZIGNOR).
///
/// **Upstream:** `rand_distr` `utils.rs:50-100`, `ziggurat`. One `next_u64`
/// supplies both the layer `i` (its low 8 bits) and the abscissa fraction
/// (its top 52 bits, as a float in `[2, 4)` minus 3 for the symmetric case,
/// or in `[1, 2)` minus `1 - EPSILON / 2` otherwise). Accept if inside the
/// layer's rectangle; the base layer (`i == 0`) goes to `zero_case`; a
/// wedge draw is accepted against `pdf` with one more `random::<f64>()`.
#[inline(always)]
fn ziggurat<R: Rng + ?Sized, P, Z>(
    rng: &mut R,
    symmetric: bool,
    x_tab: ZigTable,
    f_tab: ZigTable,
    mut pdf: P,
    mut zero_case: Z,
) -> f64
where
    P: FnMut(f64) -> f64,
    Z: FnMut(&mut R, f64) -> f64,
{
    loop {
        let bits = rng.next_u64();
        let i = bits as usize & 0xff;

        let u = if symmetric {
            into_float_with_exponent(bits >> 12, 1) - 3.0
        } else {
            into_float_with_exponent(bits >> 12, 0) - (1.0 - f64::EPSILON / 2.0)
        };
        // Upstream reads `x_tab[i]`, `x_tab[i + 1]`, `f_tab[i]` and
        // `f_tab[i + 1]` by subscript. `i <= 255` and the tables hold 257
        // entries, so every read is in range, but by arithmetic the
        // compiler cannot follow; `get` keeps the values (no_panic_gate),
        // and the unreachable miss returns NaN as `fast_exp` does.
        let (Some(&x_i), Some(&x_i1), Some(&f_i), Some(&f_i1)) = (
            x_tab.get(i),
            x_tab.get(i + 1),
            f_tab.get(i),
            f_tab.get(i + 1),
        ) else {
            return f64::NAN;
        };
        let x = u * x_i;

        let test_x = if symmetric { x.abs() } else { x };

        // algebraically equivalent to |u| < x_tab[i+1]/x_tab[i] (or u < x_tab[i+1]/x_tab[i])
        if test_x < x_i1 {
            return x;
        }
        if i == 0 {
            return zero_case(rng, u);
        }
        // algebraically equivalent to f1 + DRanU()*(f0 - f1) < 1
        if f_i1 + (f_i - f_i1) * rng.random::<f64>() < pdf(x) {
            return x;
        }
    }
}

/// One standard normal (mean 0, standard deviation 1) sample.
///
/// **Upstream:** `rand_distr` `normal.rs:60-102`, `impl Distribution<f64>
/// for StandardNormal`: symmetric ziggurat over `ZIG_NORM_X` / `ZIG_NORM_F`
/// with `pdf(x) = exp(-x*x/2)` and Marsaglia's tail method in `zero_case`
/// (two `Open01` draws per try).
pub fn standard_normal<R: Rng + ?Sized>(rng: &mut R) -> f64 {
    #[inline]
    fn pdf(x: f64) -> f64 {
        Real::exp(-x * x / 2.0)
    }
    #[inline]
    fn zero_case<R: Rng + ?Sized>(rng: &mut R, u: f64) -> f64 {
        // compute a random number in the tail by hand

        // strange initial conditions, because the loop is not
        // do-while, so the condition should be true on the first
        // run, they get overwritten anyway (0 < 1, so these are
        // good).
        let mut x = 1.0f64;
        let mut y = 0.0f64;

        while -2.0 * y < x * x {
            let x_: f64 = open01(rng);
            let y_: f64 = open01(rng);

            x = Real::ln(x_) / ZIG_NORM_R;
            y = Real::ln(y_);
        }

        if u < 0.0 {
            x - ZIG_NORM_R
        } else {
            ZIG_NORM_R - x
        }
    }

    ziggurat(rng, true, &ZIG_NORM_X, &ZIG_NORM_F, pdf, zero_case)
}

/// The normal distribution `N(mean, std_dev^2)` for `f64`.
///
/// **Upstream:** `rand_distr` 0.6.0 `Normal<f64>`. A sample is `mean +
/// std_dev * z` with `z` from [`standard_normal`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Normal {
    mean: f64,
    std_dev: f64,
}

impl Normal {
    /// The distribution with this mean and standard deviation. As upstream,
    /// a negative `std_dev` is accepted (it mirrors the samples); only a
    /// non-finite one is rejected.
    ///
    /// **Upstream:** `normal.rs:189-194`, `Normal::new`.
    pub fn new(mean: f64, std_dev: f64) -> Result<Normal, NormalError> {
        if !std_dev.is_finite() {
            return Err(NormalError::BadVariance);
        }
        Ok(Normal { mean, std_dev })
    }

    /// `mean + std_dev * zscore`. **Upstream:** `normal.rs:224-226`.
    #[inline]
    pub fn from_zscore(&self, zscore: f64) -> f64 {
        self.mean + self.std_dev * zscore
    }

    /// One sample. **Upstream:** `normal.rs:244-246`.
    pub fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> f64 {
        self.from_zscore(standard_normal(rng))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::{ChaCha8Rng, SeedableRng};

    /// Upstream's own shape checks (`normal.rs` tests): `new` rejects only a
    /// non-finite standard deviation; `from_zscore` is `mean + sd * z`.
    #[test]
    fn construction_matches_upstream() {
        assert!(Normal::new(10.0, 10.0).is_ok());
        assert!(Normal::new(10.0, -1.0).is_ok());
        assert_eq!(
            Normal::new(10.0, f64::INFINITY),
            Err(NormalError::BadVariance)
        );
        assert_eq!(Normal::new(10.0, f64::NAN), Err(NormalError::BadVariance));
        assert_eq!(Normal::new(2.0, 3.0).unwrap().from_zscore(1.5), 6.5);
    }

    /// A sanity check of the sampler as a whole: 200 000 draws have mean
    /// within 5 standard errors of 0 and variance within 2 % of 1, and some
    /// fall in the tail beyond `ZIG_NORM_R`, so `zero_case` ran. This is not
    /// the bit-identity gate (that is `tests/rng_vs_rand.rs`).
    #[test]
    fn moments_and_tail_are_plausible() {
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        let n = 200_000;
        let (mut s, mut s2, mut tail) = (0.0, 0.0, 0usize);
        for _ in 0..n {
            let z = standard_normal(&mut rng);
            s += z;
            s2 += z * z;
            if z.abs() > ZIG_NORM_R {
                tail += 1;
            }
        }
        let mean = s / n as f64;
        let var = s2 / n as f64 - mean * mean;
        assert!(mean.abs() < 5.0 / Real::sqrt(n as f64), "mean {mean}");
        assert!((var - 1.0).abs() < 0.02, "variance {var}");
        assert!(tail > 0, "no draw reached the tail");
    }
}
