// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of PETIR, a component of OUTRAM PARK.
//
// PETIR is free software: you can redistribute it and/or modify it under the
// terms of the GNU General Public License as published by the Free Software
// Foundation, version 3 of the License.
//
// PETIR is distributed in the hope that it will be useful, but WITHOUT ANY
// WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE.  See the GNU General Public License for more
// details.
//
// You should have received a copy of the GNU General Public License along
// with PETIR.  If not, see <https://www.gnu.org/licenses/>.

//! `sqrt` and `exp` for dimensionless [`Ratio`], which `no_std` `uom` omits.
//!
//! # The problem this solves
//!
//! `uom` defines `Quantity::sqrt`, `Quantity::exp` and the rest of the
//! floating-point family inside an impl block bounded on `uom::num::Float`,
//! and `uom::num::Float` is **not** what its name suggests:
//!
//! ```text
//! // uom 0.38, src/lib.rs:262
//! pub mod num {
//!     #[cfg(feature = "std")]
//!     pub use num_traits::float::Float;
//!     #[cfg(not(feature = "std"))]
//!     pub use num_traits::float::FloatCore as Float;
//! }
//! ```
//!
//! With `std` off it aliases to `FloatCore`, which deliberately carries only
//! the operations expressible without libm — no `sqrt`, no `exp`, no `powf`.
//! So in a `no_std` build those methods **do not exist on any `Quantity`**, and
//! a call to one fails with `no method named 'sqrt' found for struct Quantity`,
//! an error that points at `uom` and gives no hint that a feature flag is the
//! cause.
//!
//! Note what does *not* fix it: adding `num-traits` with its `libm` feature.
//! That makes the real `num_traits::Float` available, but `uom` never looks at
//! it — the alias above is keyed on `uom`'s own `std` feature, nothing else.
//! This was tried first, and is recorded here so the next person does not spend
//! the same half hour on it.
//!
//! # Why an extension trait rather than editing the call sites
//!
//! Every affected call in this module is on a **dimensionless [`Ratio`]**,
//! where `sqrt` and `exp` are dimension-preserving and the operation is
//! unambiguous. That makes a trait the right shape: bringing it into scope
//! restores the methods with their original spelling, so the blocks lifted from
//! `chem-eng-real-time-process-control-simulator` keep diffing clean against
//! their source. Rewriting each site as
//! `Ratio::new::<ratio>(x.get::<ratio>().sqrt())` would work and would be
//! wrong — it would be a dozen hand edits scattered through code whose value is
//! that it is a faithful copy.
//!
//! # Deliberately narrow
//!
//! [`Ratio`] only. `sqrt` on a dimensioned quantity has to halve its exponents
//! — `Area::sqrt` returns a `Length` — and that is real type-level work that
//! `uom`'s own macro does properly. This trait does not attempt it, and must
//! not be widened to; if a dimensioned `sqrt` is ever needed in `no_std`, the
//! answer is a fix or a feature upstream in `uom`, not a hand-rolled
//! exponent-halving here.

// Under a std-linked build (`cargo test`) f64's inherent sqrt/exp shadow these
// trait methods, leaving the import formally unused. See crate::real.
#[allow(unused_imports)]
use crate::real::Real;
use uom::si::f64::Ratio;
use uom::si::ratio::ratio;

/// Restores `sqrt` and `exp` on a dimensionless [`Ratio`] in `no_std` builds.
///
/// Both operate on the ratio's dimensionless value and return a [`Ratio`], so
/// they compose exactly as `uom`'s own `std`-only methods would.
pub trait RatioExt {
    /// Square root of a dimensionless ratio.
    ///
    /// `NaN` for a negative ratio, matching [`f64::sqrt`].
    fn sqrt(self) -> Ratio;
    /// `e` raised to a dimensionless ratio.
    fn exp(self) -> Ratio;
}

impl RatioExt for Ratio {
    #[inline]
    fn sqrt(self) -> Ratio {
        Ratio::new::<ratio>(Real::sqrt(self.get::<ratio>()))
    }

    #[inline]
    fn exp(self) -> Ratio {
        Ratio::new::<ratio>(Real::exp(self.get::<ratio>()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sqrt_matches_the_underlying_f64() {
        for &v in &[0.0_f64, 0.25, 1.0, 2.0, 1e10] {
            let got = RatioExt::sqrt(Ratio::new::<ratio>(v)).get::<ratio>();
            assert!((got - libm::sqrt(v)).abs() <= f64::EPSILON * got.max(1.0));
        }
    }

    #[test]
    fn exp_matches_the_underlying_f64() {
        for &v in &[-5.0_f64, -1.0, 0.0, 1.0, 10.0] {
            let got = RatioExt::exp(Ratio::new::<ratio>(v)).get::<ratio>();
            assert!((got - libm::exp(v)).abs() <= f64::EPSILON * got.max(1.0) * 4.0);
        }
    }

    /// The identities a caller will actually rely on.
    #[test]
    fn round_trips_hold() {
        let four = Ratio::new::<ratio>(4.0);
        assert!((RatioExt::sqrt(four).get::<ratio>() - 2.0).abs() < 1e-15);
        let zero = Ratio::new::<ratio>(0.0);
        assert!((RatioExt::exp(zero).get::<ratio>() - 1.0).abs() < 1e-15);
    }

    #[test]
    fn sqrt_of_a_negative_ratio_is_nan() {
        assert!(RatioExt::sqrt(Ratio::new::<ratio>(-1.0))
            .get::<ratio>()
            .is_nan());
    }
}
