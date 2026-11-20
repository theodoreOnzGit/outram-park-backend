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
//! **Status: first port of `erf1` only, not differentially tested (gh:#1680).**
//! The rest of the file (input parsing, I/O units, heap sort, messages) is a
//! placeholder.
//!
//! | upstream | here |
//! |---|---|
//! | `FUNCTION ERF1` (`egs_utilities.mortran:2096-2151`) | [`erf1`] |

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

#[cfg(test)]
mod tests {
    use super::*;

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
