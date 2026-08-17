//! Resolve the two spatial extents for the 2-D routines.
//!
//! # Provenance
//!
//! - **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
//!   Institute (SNRSI).
//! - **Source file:** `handle2dcoords.m`, `main_exec_diff3d_standalone`
//!   snapshot.
//! - **Permission:** given by the author for open-source release under OUTRAM
//!   PARK; see `docs/bedok-port-scoping.md` §6.
//! - **Licence:** GPL-3.0-only.

use crate::error::{BedokError, Result};
use crate::types::{CoordinateMode, Params};

/// `[maxi1, maxi2] = handle2dcoords(params)`.
///
/// Picks the first populated coordinate pair, in the reference's order:
/// cylindrical (`maxir`, `maxiz` — note this is the **r-z** plane, not the
/// `r`-`theta` pair its 3-D sibling uses), then Cartesian (`maxix`, `maxiy`),
/// then generic (`maxi1`, `maxi2`).
///
/// # Returns
///
/// Node counts along each of the two dimensions — dimensionless.
///
/// # Difference from [`crate::handle3dcoords::handle3dcoords`]
///
/// The 3-D version pre-initialises its outputs to `1` and so returns
/// `(1, 1, 1)` when nothing matches. This one does **not** initialise, so an
/// unmatched `params` leaves the outputs unassigned and MATLAB raises
/// `Output argument "maxi1" (and maybe others) not assigned`. That is
/// translated as [`BedokError::NoCoordinateBranch`] rather than a silent
/// default, because silently returning `1` here would be a repair the
/// reference does not make.
///
/// # Errors
///
/// [`BedokError::NoCoordinateBranch`] when no coordinate pair is fully
/// populated.
pub fn handle2dcoords(params: &Params) -> Result<(usize, usize)> {
    // The 2-D cylindrical test is (maxir, maxiz); `coordinate_mode_2d` encodes
    // that, and it is not the same test the 3-D routine applies.
    match params.coordinate_mode_2d() {
        Some(CoordinateMode::Cylindrical) => {
            Ok((params.maxir.unwrap(), params.maxiz.unwrap()))
        }
        Some(CoordinateMode::Cartesian) => Ok((params.maxix.unwrap(), params.maxiy.unwrap())),
        Some(CoordinateMode::Generic) => Ok((params.maxi1.unwrap(), params.maxi2.unwrap())),
        None => Err(BedokError::NoCoordinateBranch),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cartesian_pair_is_x_and_y() {
        let params = Params {
            maxix: Some(17),
            maxiy: Some(17),
            maxiz: Some(18),
            ..Default::default()
        };
        assert_eq!(handle2dcoords(&params).unwrap(), (17, 17));
    }

    /// The 2-D cylindrical branch is `(maxir, maxiz)` — r-z — so a case
    /// carrying `maxir` and `maxiz` takes it even though `maxitheta` is absent.
    #[test]
    fn cylindrical_pair_is_r_and_z() {
        let params = Params {
            maxir: Some(5),
            maxiz: Some(18),
            ..Default::default()
        };
        assert_eq!(handle2dcoords(&params).unwrap(), (5, 18));
    }

    #[test]
    fn unmatched_params_are_an_error_not_a_default() {
        assert!(matches!(
            handle2dcoords(&Params::default()),
            Err(BedokError::NoCoordinateBranch)
        ));
    }
}
