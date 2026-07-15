// Ported from NJOY2016 `src/groupr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! Unresolved-resonance-range (URR) self-shielding + infinite-medium flux —
//! **skeleton**.
//!
//! Will host GROUPR's `genflx` infinite-medium weighting-flux calculator
//! (`groupr.f90:542-548` driver) and the `stounr`/`getunr` unresolved-range
//! self-shielded cross-section machinery that fills the background-dilution
//! (`nz > 1`) columns of the group constants. Until ported, [`genflx`] returns
//! [`crate::NjoyError::NotPorted`].

use crate::NjoyError;

/// Infinite-medium weighting flux calculator — **not ported** (skeleton).
///
/// Placeholder for `genflx` (`groupr.f90` flux calc). Returns
/// [`NjoyError::NotPorted`] tagged `"groupr::genflx"`.
pub fn genflx() -> Result<(), NjoyError> {
    Err(NjoyError::NotPorted("groupr::genflx"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn genflx_reports_not_ported() {
        assert!(matches!(
            genflx(),
            Err(NjoyError::NotPorted("groupr::genflx"))
        ));
    }
}
