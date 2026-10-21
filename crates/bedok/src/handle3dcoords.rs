//! Resolve the three spatial extents from whichever coordinate fields the case
//! file populated.
//!
//! # Provenance
//!
//! - **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
//!   Institute (SNRSI).
//! - **Source file:** `handle3dcoords.m`, `main_exec_diff3d_standalone`
//!   snapshot.
//! - **Permission:** given by the author for open-source release under OUTRAM
//!   PARK; see the crate README, "Permission and attribution".
//! - **Licence:** GPL-3.0-only.

use crate::types::{CoordinateMode, Params};

/// `[maxi1, maxi2, maxi3] = handle3dcoords(params)`.
///
/// Picks the first populated coordinate triple, in the reference's order:
/// cylindrical (`maxir`, `maxitheta`, `maxiz`), then Cartesian (`maxix`,
/// `maxiy`, `maxiz`), then generic (`maxi1`, `maxi2`, `maxi3`). All three
/// outputs default to `1` when nothing matches, exactly as the reference
/// initialises them.
///
/// # Returns
///
/// Node counts along each dimension — dimensionless, and at least `1`.
///
/// # Reference defect — carried over deliberately
///
/// In the generic branch the reference assigns
///
/// ```text
/// maxi3=params.maxix;
/// ```
///
/// where every indication is that `params.maxi3` was intended. It is
/// translated as written, per the no-silent-repairs rule in
/// the crate README, "Translation policy".
///
/// The consequence is sharper in Rust than in MATLAB, so it is worth stating.
/// The generic branch is only reached when the Cartesian branch did *not*
/// match, meaning at least one of `maxix`/`maxiy`/`maxiz` is absent. If the
/// absent one is `maxix`, MATLAB raises `Reference to non-existent field
/// 'maxix'` and this function panics with the equivalent message. If `maxix`
/// happens to be present, both silently produce a wrong `maxi3`.
///
/// # Panics
///
/// If the generic branch is taken and `maxix` is not populated — mirroring the
/// reference's `Reference to non-existent field` error.
pub fn handle3dcoords(params: &Params) -> (usize, usize, usize) {
    let mut maxi1 = 1;
    let mut maxi2 = 1;
    let mut maxi3 = 1;

    match params.coordinate_mode_3d() {
        Some(CoordinateMode::Cylindrical) => {
            maxi1 = params.maxir.unwrap();
            maxi2 = params.maxitheta.unwrap();
            maxi3 = params.maxiz.unwrap();
        }
        Some(CoordinateMode::Cartesian) => {
            maxi1 = params.maxix.unwrap();
            maxi2 = params.maxiy.unwrap();
            maxi3 = params.maxiz.unwrap();
        }
        Some(CoordinateMode::Generic) => {
            maxi1 = params.maxi1.unwrap();
            maxi2 = params.maxi2.unwrap();
            // REFERENCE DEFECT, translated as written — see the doc comment.
            maxi3 = params
                .maxix
                .expect("Reference to non-existent field 'maxix'");
        }
        None => {}
    }

    (maxi1, maxi2, maxi3)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cartesian_branch_is_taken_for_the_benchmark_cases() {
        let params = Params {
            maxix: Some(17),
            maxiy: Some(17),
            maxiz: Some(18),
            ..Default::default()
        };
        assert_eq!(handle3dcoords(&params), (17, 17, 18));
    }

    #[test]
    fn cylindrical_branch_wins_when_both_are_populated() {
        let params = Params {
            maxir: Some(5),
            maxitheta: Some(6),
            maxix: Some(17),
            maxiy: Some(17),
            maxiz: Some(18),
            ..Default::default()
        };
        assert_eq!(handle3dcoords(&params), (5, 6, 18));
    }

    #[test]
    fn unmatched_params_fall_back_to_ones() {
        assert_eq!(handle3dcoords(&Params::default()), (1, 1, 1));
    }

    /// Pins the `maxi3 = params.maxix` defect described in the doc comment: with
    /// the generic triple populated *and* a stray `maxix`, the reference returns
    /// `maxix` as the third extent rather than `maxi3`.
    #[test]
    fn generic_branch_reproduces_the_maxix_defect() {
        let params = Params {
            maxi1: Some(3),
            maxi2: Some(4),
            maxi3: Some(5),
            maxix: Some(99),
            ..Default::default()
        };
        assert_eq!(handle3dcoords(&params), (3, 4, 99));
    }
}
