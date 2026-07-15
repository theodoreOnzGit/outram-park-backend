// Ported from NJOY2016 `src/gaminr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! GAMINR photon-production **matrices** — `gtff` + the `dspla` matrix branch —
//! **skeleton**.
//!
//! Will host GAMINR's photon-production feed function `gtff` and the matrix
//! branch of `dspla` (`gaminr.f90:1162-1514`) that assembles neutron→photon
//! group-to-group production matrices. Until ported, [`photon_matrix`] returns
//! [`crate::NjoyError::NotPorted`].

use crate::NjoyError;

/// Photon-production matrix assembly — **not ported** (skeleton).
///
/// Placeholder for `gtff` + the `dspla` matrix branch (`gaminr.f90:1162-1514`).
/// Returns [`NjoyError::NotPorted`] tagged `"gaminr::photon_matrix"`.
pub fn photon_matrix() -> Result<(), NjoyError> {
    Err(NjoyError::NotPorted("gaminr::photon_matrix"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn photon_matrix_reports_not_ported() {
        assert!(matches!(
            photon_matrix(),
            Err(NjoyError::NotPorted("gaminr::photon_matrix"))
        ));
    }
}
