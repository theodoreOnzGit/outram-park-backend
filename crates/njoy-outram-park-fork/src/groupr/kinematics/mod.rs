// Ported from NJOY2016 `src/groupr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! Center-of-mass → lab and lab-frame kinematics for the GROUPR scatter-matrix
//! feed functions.
//!
//! The GROUPR group-to-group **matrix** path (`mfd = 6`) needs the
//! secondary-particle emission spectrum transformed into the lab frame, then
//! binned into secondary groups to form the feed function the panel
//! integrator deposits. This module covers three distinct File-6 data paths:
//!
//! - [`cm`] — **LAW = 1, CM frame**: the Kalbach-Mann and Legendre-in-CM
//!   double-differential evaluations, the Kalbach slope, discrete emission
//!   lines, and the adaptive CM→lab integrator ([`cm2lab`]).
//! - [`lab7`] — **LAW = 7, already lab-frame** angle-energy data: the
//!   Legendre-in-mu projection ([`ll2lab`]).
//! - [`lab1`] — **LAW = 1, already lab-frame** double-differential data
//!   (`LCT != 2`): direct incident/secondary-energy interpolation, no
//!   kinematic transform ([`f6lab`]).
//!
//! plus shared primitives ([`shared`]: the Legendre recursion, the common
//! [`LabDistribution`] output type, and the two laws' shared 8-point
//! Gauss–Legendre quadrature) and the Kalbach-86 slope systematics ([`bach`]).
//!
//! # Fortran routine → Rust item map (with `groupr.f90` line cites)
//!
//! | Fortran routine | lines | Rust item | Status |
//! |---|---|---|---|
//! | `legndr`  | `mathm.f90` | [`legndr`] | **DONE** (helper) |
//! | `bach`    | 8812–8932 | [`bach`] | **DONE** |
//! | `f6ddx`   | 8520–8717 | `cm::Cm6Emission::f6ddx` | **DONE** for `lang=1,2` continuum; `lang=0` (phase space) and `lang>=11` (tabulated) → `NotPorted` |
//! | `f6dis`   | 8719–8810 | `cm::Cm6Emission::f6dis` | **DONE** for `lang=1,2` — see its doc for a literal upstream indexing quirk this reproduces |
//! | `f6cm`    | 8260–8518 | `cm::Cm6Emission::f6cm` | **DONE**, including the `ND` discrete-line init bounds and delta-function branch |
//! | `cm2lab`  | 8135–8258 | [`cm2lab`] | **DONE** |
//! | `ll2lab`  | 8934–9061 | [`ll2lab`] | **DONE** |
//! | `f6lab`   | 9063–9338 | [`f6lab`] | **DONE** for `LANG = 1`; `LANG = 11..15` (tabulated) → `NotPorted` |
//!
//! # Scope of the ported path (honest limits)
//!
//! [`cm2lab`] handles ENDF File 6 **LAW 1** (continuum energy-angle) in the
//! **CM frame** with **LANG = 1** (Legendre-in-CM) or **LANG = 2** (Kalbach-Mann
//! systematics), including discrete emission lines (ENDF `ND > 0`, both
//! purely-discrete and — for the `elmax`/`epnext` search bounds only, matching
//! upstream — mixed continuum-plus-discrete subsections; see
//! `cm::Cm6Emission::f6cm`'s docs for the exact mixed-case limitation this
//! reproduces). [`ll2lab`] handles ENDF File 6 **LAW 7** (angle-energy,
//! already lab-frame). [`f6lab`] handles ENDF File 6 **LAW 1** data already in
//! the lab frame, `LANG = 1` only. The phase-space law (`f6psp`, LANG = 0) and
//! every tabulated-angular-distribution branch (`LANG >= 11`, in any of the
//! three paths) are **not ported** and return [`crate::NjoyError::NotPorted`].
//! See the per-item docs for exact line ranges.
//!
//! This is untrusted AI draft material: verify against upstream NJOY golden
//! output before trusting numerically.

pub mod bach;
pub mod cm;
pub mod lab1;
pub mod lab7;
pub mod shared;

pub use bach::bach;
pub use cm::{cm2lab, Cm6Emission, Cm6Lang, Cm6Point};
pub use lab1::{f6lab, Law1LabTable};
pub use lab7::{ll2lab, Law7Curve, Law7Emission};
pub use shared::{lab_energy_from_cm, legndr, LabDistribution};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::NjoyError;

    /// The still-unported tabulated-angular-distribution paths honestly
    /// report themselves unported (no fabricated kinematics). `f6dis`/
    /// `ll2lab`/`f6lab` themselves are now ported (see the module doc table)
    /// for the LANG values this crate's `f6ddx`/`f6cm` already support; this
    /// test instead pins the remaining gaps directly reachable from this
    /// module's public API.
    ///
    /// **History.** Until this port, this test asserted that calling
    /// `f6dis()`, `ll2lab()`, and `f6lab()` with no arguments returned
    /// `NotPorted` — those were placeholder stubs. All three now take real
    /// data and are implemented for their primary scope, so that form of the
    /// assertion no longer applies; it is replaced by pinning the LANG=11-15
    /// (tabulated) gap that remains in each of `cm2lab`'s `f6ddx`/`f6cm` (via
    /// `Cm6Lang`, which has no tabulated variant to begin with — the gap is
    /// structural, not a runtime check) and in `f6lab` (checked here, since
    /// `f6lab` takes a raw `lang: u32`).
    #[test]
    fn f6lab_tabulated_lang_reports_not_ported() {
        let table = |e_in: f64| Law1LabTable {
            e_in,
            points: vec![
                cm::Cm6Point {
                    ep: 0.0,
                    coeffs: vec![0.0],
                },
                cm::Cm6Point {
                    ep: 1.0e5,
                    coeffs: vec![1.0],
                },
            ],
        };
        let lo = table(1.0e6);
        let hi = table(2.0e6);
        let result = f6lab(
            &lo,
            &hi,
            22,
            11,
            crate::endf::interp::IntLaw::LinLin,
            1.5e6,
            5.0e4,
            1,
        );
        assert!(matches!(result, Err(NjoyError::NotPorted(_))));
    }
}
