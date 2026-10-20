// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.
//
// ---------------------------------------------------------------------------
// PORTED (NOT from GSL) from `rand` 0.10.3, the version boon-lay's Cargo.lock
// pinned on 2026-10-20:
//
//   src/distr/uniform.rs       (`Uniform::new`, `Uniform`'s `sample`, `Error`)
//   src/distr/uniform_float.rs (`UniformFloat<f64>`: `new`, `new_bounded`,
//                               `sample`)
//   src/distr/float.rs         (`IntoFloat::into_float_with_exponent`,
//                               `Open01` for f64)
//   src/distr/utils.rs         (scalar `decrease_masked`)
//
// Licence: MIT OR Apache-2.0 (used here under MIT). Copyright lines as
// upstream states them (LICENSE-MIT and the file headers):
//
//   Copyright 2018 Developers of the Rand project
//   Copyright (c) 2014 The Rust Project Developers
//   Copyright 2018-2020 Developers of the Rand project (uniform_float.rs)
//   Copyright 2017 The Rust Project Developers (uniform_float.rs)
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

//! **Uniform `f64` on `[low, high)`**, and the open-interval `(0, 1)`
//! sample the normal sampler's tail needs — `rand` 0.10.3's algorithms,
//! bit for bit.
//!
//! Only `f64` is ported: it is the one type RustBCA samples. The integer
//! samplers (Lemire's widening multiply) are not, because nothing calls them.

use super::traits::Rng;

/// Why a [`Uniform`] could not be built.
///
/// **Upstream:** `rand` `uniform.rs:123-128`, `uniform::Error`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UniformError {
    /// `low >= high`.
    EmptyRange,
    /// `low`, `high` or `high - low` is not finite.
    NonFinite,
}

impl core::fmt::Display for UniformError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            UniformError::EmptyRange => {
                "low > high (or equal if exclusive) in uniform distribution"
            }
            UniformError::NonFinite => "Non-finite range in uniform distribution",
        })
    }
}

/// `f64` from 52 fraction bits and an exponent, by assembling the IEEE bits.
///
/// **Upstream:** `rand` `float.rs:99-104`, `IntoFloat for u64`
/// (`(1023 + exponent) << 52`, OR'd onto `bits`, which must fit in 52 bits).
#[inline(always)]
pub(crate) fn into_float_with_exponent(bits: u64, exponent: i32) -> f64 {
    let exponent_bits: u64 = ((1023 + exponent) as u64) << 52;
    f64::from_bits(bits | exponent_bits)
}

/// A uniform `f64` in the open interval `(0, 1)`, from 52 random bits.
///
/// **Upstream:** `rand` `float.rs`, `impl Distribution<f64> for Open01`
/// ("Transmute-based method; 52 random bits; (0, 1) interval"): the top 52
/// bits of one `next_u64` as a float in `[1, 2)`, minus `1 - EPSILON / 2`.
#[inline]
pub fn open01<R: Rng + ?Sized>(rng: &mut R) -> f64 {
    let value: u64 = rng.next_u64();
    let fraction = value >> (64 - 52);
    into_float_with_exponent(fraction, 0) - (1.0 - f64::EPSILON / 2.0)
}

/// The uniform distribution on `[low, high)` for `f64`.
///
/// **Upstream:** `rand` 0.10.3 `Uniform<f64>`, i.e. `UniformFloat<f64>`
/// (`uniform_float.rs`). A sample is `value0_1 * scale + low` with
/// `value0_1` built from 52 random bits, and `scale` shrunk at construction
/// until the largest sample cannot round up to `high`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Uniform {
    low: f64,
    scale: f64,
}

impl Uniform {
    /// The distribution on `[low, high)`.
    ///
    /// **Upstream:** `uniform.rs:235-241` → `UniformFloat::new`
    /// (`uniform_float.rs:94-115`).
    ///
    /// **DEVIATION (checks):** upstream tests `low` and `high` for
    /// finiteness only under `debug_assertions`; PETIR always does. In a
    /// release build upstream still rejects every non-finite input (through
    /// the `low < high` test or the finite-`scale` test), so the only
    /// difference is which error variant a non-finite bound reports.
    pub fn new(low: f64, high: f64) -> Result<Uniform, UniformError> {
        if !low.is_finite() || !high.is_finite() {
            return Err(UniformError::NonFinite);
        }
        if !(low < high) {
            return Err(UniformError::EmptyRange);
        }

        let scale = high - low;
        if !scale.is_finite() {
            return Err(UniformError::NonFinite);
        }

        Ok(Self::new_bounded(low, high, scale))
    }

    /// Shrink `scale` one ulp at a time until `scale * max_rand + low <=
    /// high`, where `max_rand = 1 - EPSILON` is the largest `value0_1`.
    ///
    /// **Upstream:** `uniform_float.rs:68-82` (`new_bounded`), with the
    /// scalar `decrease_masked` of `utils.rs:303-306` (`from_bits(bits - 1)`).
    fn new_bounded(low: f64, high: f64, mut scale: f64) -> Uniform {
        let max_rand = 1.0 - f64::EPSILON;

        loop {
            let mask = scale * max_rand + low <= high;
            if mask {
                break;
            }
            scale = f64::from_bits(scale.to_bits() - 1);
        }

        Uniform { low, scale }
    }

    /// One sample.
    ///
    /// **Upstream:** `uniform_float.rs:143-156`, `UniformFloat::sample`: the
    /// top 52 bits of one `next_u64` as a float in `[1, 2)`, minus 1, times
    /// `scale`, plus `low`, in that order (upstream: "the order of
    /// multiplication and addition is important"; no `mul_add`).
    #[inline]
    pub fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> f64 {
        let value1_2 = into_float_with_exponent(rng.next_u64() >> (64 - 52), 0);
        let value0_1 = value1_2 - 1.0;
        value0_1 * self.scale + self.low
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Const(u64);
    impl Rng for Const {
        fn next_u32(&mut self) -> u32 {
            self.0 as u32
        }
        fn next_u64(&mut self) -> u64 {
            self.0
        }
        fn fill_bytes(&mut self, _: &mut [u8]) {}
    }

    /// Upstream's `Open01` edge cases (`float.rs` `test_f64`): zero bits give
    /// `EPSILON / 2`, `1 << 12` gives `3 * EPSILON / 2`, all ones give
    /// `1 - EPSILON / 2`.
    #[test]
    fn open01_edge_cases_match_upstream() {
        assert_eq!(open01(&mut Const(0)), f64::EPSILON / 2.0);
        assert_eq!(open01(&mut Const(1 << 12)), f64::EPSILON / 2.0 * 3.0);
        assert_eq!(open01(&mut Const(!0)), 1.0 - f64::EPSILON / 2.0);
    }

    /// The bounds of a uniform sample: all-zero bits give `low`, all-one
    /// bits stay at or below `high` (that is what `new_bounded` buys).
    /// Upstream's `test_floats` asserts the same for a range set that
    /// includes `(0.1 * MAX, MAX)`, which needs the scale reduction.
    #[test]
    fn samples_stay_in_range_and_errors_are_reported() {
        for (low, high) in [
            (0.0, 100.0),
            (-1e35, 1e35),
            (1e-35, 1e-25),
            (0.1 * f64::MAX, f64::MAX),
        ] {
            let u = Uniform::new(low, high).unwrap();
            assert_eq!(u.sample(&mut Const(0)), low);
            let top = u.sample(&mut Const(!0));
            assert!(low <= top && top <= high, "({low}, {high}) gave {top}");
        }
        assert_eq!(Uniform::new(1.0, 1.0), Err(UniformError::EmptyRange));
        assert_eq!(Uniform::new(2.0, 1.0), Err(UniformError::EmptyRange));
        assert_eq!(
            Uniform::new(0.0, f64::INFINITY),
            Err(UniformError::NonFinite)
        );
        assert_eq!(
            Uniform::new(-f64::MAX, f64::MAX),
            Err(UniformError::NonFinite)
        );
    }
}
