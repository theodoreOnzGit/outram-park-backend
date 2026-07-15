// Ported from NJOY2016 `src/groupr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! Group-to-group **scatter matrix** assembly — the matrix reduction of `panel`.
//!
//! **Skeleton — see [`crate::groupr`] gap list.** This module will host the
//! matrix (`mfd = 6`/`mfd = 26`) reduction of the GROUPR `panel`/`gpanel`
//! quadrature (`groupr.f90:5858-6091`): the `il` (Legendre) × `ig` (secondary
//! group) loop that turns a feed function `ff(il, ig)` into a group-to-group
//! transfer matrix, plus the GENDF matrix-record helpers (`IG2LO > 0`,
//! `NG2 > 2`, `NL > 1`) that write it through [`crate::groupr::gendf`].
//!
//! Until ported, [`scatter_matrix`] returns [`crate::NjoyError::NotPorted`].

use crate::NjoyError;

/// Assemble a group-to-group scatter matrix — **not ported** (skeleton).
///
/// Placeholder for the matrix reduction of `panel` (`groupr.f90:5858-6091`).
/// Returns [`NjoyError::NotPorted`] tagged `"groupr::scatter_matrix"`.
pub fn scatter_matrix() -> Result<(), NjoyError> {
    Err(NjoyError::NotPorted("groupr::scatter_matrix"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scatter_matrix_reports_not_ported() {
        assert!(matches!(
            scatter_matrix(),
            Err(NjoyError::NotPorted("groupr::scatter_matrix"))
        ));
    }
}
