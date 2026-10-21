// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com)
// Copyright (C) 2004-2023 OpenFOAM Foundation
// Copyright (C) 2016-2023 OpenCFD Ltd.
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.
//
// PROVENANCE — LIFTED VERBATIM from
//     crates/outram-foam-basic-lib/src/primitives/scalar.rs
// (the OpenFOAM `doubleScalar` guard constants). Values are unchanged; only
// the module doc has been extended to say how PETIR uses them.


//! The scalar floating-point type and OpenFOAM's small/large numeric
//! guard constants.
//!
//! `Scalar` is OpenFOAM's `scalar` (double-precision, dimensionless) and
//! `Label` is its `label` (signed integer index/count). The constants are
//! the fixed thresholds OpenFOAM uses to guard against divide-by-zero and
//! overflow; they are dimensionless and identical in value to the upstream
//! `doubleScalar` definitions.

/// OpenFOAM `scalar` — a dimensionless double-precision floating-point value.
pub type Scalar = f64;
/// OpenFOAM `label` — a signed integer used for indices and counts.
pub type Label = i64;

/// Small number used to guard against division by (near-)zero (1e-15).
pub const SMALL: Scalar = 1e-15;
/// Very small number near the underflow floor (1e-300).
pub const VSMALL: Scalar = 1e-300;
/// Square root of `SMALL` (≈ 3.162e-8).
pub const ROOT_SMALL: Scalar = 3.162_277_660_168_379_5e-8; // sqrt(1e-15)
/// Square root of `VSMALL` (1e-150).
pub const ROOT_VSMALL: Scalar = 1e-150; // sqrt(1e-300)
/// Large number used as a finite stand-in for "infinity" (1e15).
pub const GREAT: Scalar = 1e15;
/// Very large number near the overflow ceiling (1e300).
pub const VGREAT: Scalar = 1e300;
/// Square root of `GREAT` (≈ 3.162e7).
pub const ROOT_GREAT: Scalar = 3.162_277_660_168_379_5e7; // sqrt(1e15)
/// Square root of `GREAT` (≈ 3.162e7).

/// Machine epsilon for `f64` — the gap between 1.0 and the next representable
/// value (≈ 2.220e-16).
///
/// GSL spells this `GSL_DBL_EPSILON` and uses it as the reference scale for
/// nearly every convergence test in the library; PETIR's translated routines
/// use it the same way. It is re-exported here, rather than referenced as
/// [`f64::EPSILON`], so a reader diffing a routine against its GSL source sees
/// the same name on both sides.
pub const DBL_EPSILON: Scalar = f64::EPSILON;

/// Square root of [`DBL_EPSILON`] (≈ 1.490e-8).
///
/// The natural step scale for a first-order finite difference and the floor
/// below which a relative tolerance on a root cannot be met — GSL's
/// `GSL_SQRT_DBL_EPSILON`.
pub const SQRT_DBL_EPSILON: Scalar = 1.490_116_119_384_765_6e-8;

/// Cube root of [`DBL_EPSILON`] (≈ 6.055e-6).
///
/// The optimal step scale for a *central* finite difference, where truncation
/// error goes as `h^2` and round-off as `eps/h` — GSL's
/// `GSL_ROOT3_DBL_EPSILON`.
pub const CBRT_DBL_EPSILON: Scalar = 6.055_454_452_393_343e-6;

/// Smallest positive normal `f64` (≈ 2.225e-308) — GSL's `GSL_DBL_MIN`.
pub const DBL_MIN: Scalar = f64::MIN_POSITIVE;

/// Largest finite `f64` (≈ 1.798e308) — GSL's `GSL_DBL_MAX`.
pub const DBL_MAX: Scalar = f64::MAX;

#[cfg(test)]
mod tests {
    use super::*;
    // Under a std-linked test build f64's inherent sqrt/cbrt shadow these
    // trait methods, leaving the import formally unused. See crate::real.
    #[allow(unused_imports)]
    use crate::real::Real;

    /// The hand-written root constants must actually be the roots they claim,
    /// to the precision `f64` can express. A typo'd digit here would silently
    /// mis-scale every finite-difference step and convergence test in PETIR.
    #[test]
    fn root_epsilon_constants_are_the_roots_they_claim() {
        assert!((SQRT_DBL_EPSILON - DBL_EPSILON.sqrt()).abs() <= f64::EPSILON * 1e-6);
        assert!((CBRT_DBL_EPSILON - DBL_EPSILON.cbrt()).abs() <= 1e-20);
        assert!((ROOT_SMALL - SMALL.sqrt()).abs() <= 1e-22);
        assert!((ROOT_VSMALL - VSMALL.sqrt()).abs() <= 1e-160);
        assert!((ROOT_GREAT - GREAT.sqrt()).abs() <= 1e-1);
    }
}
