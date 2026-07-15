// Ported from NJOY2016 `src/groupr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! Center-of-mass → lab kinematics for the scatter-matrix feed functions —
//! **not ported** (honest stub).
//!
//! The GROUPR group-to-group **matrix** path (`mfd = 6`/`mfh = 26`) needs the
//! secondary-particle emission spectrum transformed from the center-of-mass
//! frame (where ENDF File 6 laws are given) into the lab frame, then binned into
//! secondary groups to form the feed function `ff(il, ig)` that
//! [`crate::groupr::panel`] deposits. That transform is a large, physics-dense
//! block of NJOY that is **not yet ported**:
//!
//! | Fortran routine | `groupr.f90` lines | Role |
//! |---|---|---|
//! | `cm2lab` | 8135–8258 | CM→lab transform of an energy-angle emission block |
//! | `f6cm`   | 8260–8518 | File-6 CM-frame double-differential evaluation |
//! | `f6ddx`  | 8520–8717 | CM double-differential cross section at (E, E', mu) |
//! | `f6dis`  | 8719–8810 | discrete CM emission contribution |
//! | `bach`   | 8812–8932 | Kalbach-Mann `a(E, E')` slope parameter |
//! | `ll2lab` | 8934–9061 | Legendre-in-CM → lab transform |
//! | `f6lab`  | 9063–9338 | File-6 lab-frame double-differential evaluation |
//! | `getdis` | 9340–9677 | elastic / discrete-inelastic feed function |
//! | `getaed` | 9874–10239 | average energy per group (heating) |
//!
//! Until these are ported, the vector (1-D) group-average path in
//! [`crate::groupr::panel`] is fully usable, but the scatter/production **matrix**
//! is not. [`cm2lab`] therefore returns [`crate::NjoyError::NotPorted`] rather
//! than a fabricated transform.

use crate::NjoyError;

/// CM→lab kinematic transform of an emission block — **not ported**.
///
/// Placeholder for `cm2lab` (`groupr.f90:8135-8258`). Ported, this would take a
/// center-of-mass energy-angle emission distribution at incident energy `e_in`
/// \[eV\] for a product of atomic-weight ratio `awr` and return the lab-frame
/// distribution binned to the secondary group structure — the feed function the
/// matrix path integrates.
///
/// Returns [`NjoyError::NotPorted`] tagged `"groupr::cm2lab"`. See the module
/// docs for the full list of matrix-path routines still missing.
pub fn cm2lab(_e_in: f64, _awr: f64) -> Result<(), NjoyError> {
    Err(NjoyError::NotPorted("groupr::cm2lab"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The CM→lab transform honestly reports itself unported (no fabricated
    /// kinematics), guarding the "no overclaiming" rule until the matrix path
    /// lands.
    #[test]
    fn cm2lab_reports_not_ported() {
        assert!(matches!(
            cm2lab(1.0e6, 235.0),
            Err(NjoyError::NotPorted("groupr::cm2lab"))
        ));
    }
}
