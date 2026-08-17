//! Re-index a whole sparse matrix between the plain and half-index numberings.
//!
//! # Provenance
//!
//! - **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
//!   Institute (SNRSI).
//! - **Source file:** `convertsparseformat2d.m`, `main_exec_diff3d_standalone`
//!   snapshot.
//! - **Permission:** given by the author for open-source release under OUTRAM
//!   PARK; see `docs/bedok-port-scoping.md` §6.
//! - **Licence:** GPL-3.0-only.

use crate::convertindexc2d::{convertindexc2d, IndexMode};
use crate::error::Result;
use crate::handle2dcoords::handle2dcoords;
use crate::matlab::SparseMatrix;
use crate::types::Params;

/// `newmat = convertsparseformat2d(params, mat, frommode, tomode)`.
///
/// Applies [`convertindexc2d`] to a matrix's row and column indices, leaving
/// the values alone, and reassembles at whatever length the destination
/// numbering implies.
///
/// # Arguments
///
/// - `params` — supplies `G`, `Nc` and the extents.
/// - `mat` — the matrix to re-index.
/// - `frommode`, `tomode` — source and destination numberings.
///
/// # Returns
///
/// The re-indexed matrix, square at `philenf1` (mode 1) or `philenf2`
/// (mode 2).
///
/// # Errors
///
/// Propagates [`crate::error::BedokError::NoCoordinateBranch`] from
/// `handle2dcoords`.
///
/// # Values are not converted — and the reference says so
///
/// The line above the live one in the `.m` file is a commented-out version that
/// also passed `v` through `convertindexc2d`:
///
/// ```text
/// %newmat=sparse(convertindexc2d(...i...),convertindexc2d(...j...),convertindexc2d(params,v,frommode,tomode),len,len);
/// ```
///
/// Converting *values* with an *index* mapping would have been wrong, and the
/// author evidently caught it. The comment is preserved here because it
/// documents a deliberate correction rather than leftover scaffolding.
///
/// # Extent lookup differs from its callee
///
/// This function resolves extents through `handle2dcoords`, but
/// [`convertindexc2d`] reads `params.maxi1`/`maxi2` directly. A `params`
/// carrying only `maxix`/`maxiy` therefore passes the check here and then
/// panics inside the callee. The inconsistency is the reference's; see
/// [`convertindexc2d`] for the detail.
///
/// # Panics
///
/// If a converted index is fractional or negative — the 0-based equivalent of
/// MATLAB's `Subscripts must be either integers 1 to (2^63)-1 or logicals`
/// from the `sparse` call.
pub fn convertsparseformat2d(
    params: &Params,
    mat: &mut SparseMatrix,
    frommode: IndexMode,
    tomode: IndexMode,
) -> Result<SparseMatrix> {
    let g_count = params.g;
    let (maxi1, maxi2) = handle2dcoords(params)?;
    let nc = params.nc_or_zero();

    // `philen1` and `philen2` are computed in the reference and never read;
    // only the `philenf*` pair below reaches the `sparse` call.
    let philenf1 = (g_count + nc) * maxi1 * maxi2;
    let philenf2 = g_count * (2 * maxi1 + 1) * (2 * maxi2 + 1) + nc * maxi1 * maxi2;

    let len = match tomode {
        IndexMode::Plain => philenf1,
        IndexMode::DiamondDifference => philenf2,
    };

    let found = mat.find();
    let rows: Vec<f64> = found.iter().map(|t| t.i as f64).collect();
    let cols: Vec<f64> = found.iter().map(|t| t.j as f64).collect();
    let vals: Vec<f64> = found.iter().map(|t| t.v).collect();

    let new_rows = convertindexc2d(params, &rows, frommode, tomode);
    let new_cols = convertindexc2d(params, &cols, frommode, tomode);

    let new_rows = to_subscripts(&new_rows);
    let new_cols = to_subscripts(&new_cols);

    Ok(SparseMatrix::assemble(&new_rows, &new_cols, &vals, len, len))
}

/// Turn converted indices into sparse subscripts, rejecting anything
/// fractional or negative the way MATLAB's `sparse` itself would.
///
/// The bound is `>= 0.0` rather than the reference's `>= 1`, because
/// [`convertindexc2d`] hands back 0-based indices.
fn to_subscripts(v: &[f64]) -> Vec<usize> {
    v.iter()
        .map(|x| {
            assert!(
                x.fract() == 0.0 && *x >= 0.0,
                "sparse subscripts must be non-negative integers (got {x})"
            );
            *x as usize
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params() -> Params {
        Params {
            maxi1: Some(2),
            maxi2: Some(2),
            maxix: Some(2),
            maxiy: Some(2),
            g: 1,
            nc: Some(0),
            ..Default::default()
        }
    }

    #[test]
    fn plain_to_plain_preserves_the_matrix() {
        let p = params();
        let mut m = SparseMatrix::assemble(&[0, 1], &[0, 1], &[1.5, 2.5], 4, 4);
        let mut out =
            convertsparseformat2d(&p, &mut m, IndexMode::Plain, IndexMode::Plain).unwrap();
        assert_eq!(out.nnz(), 2);
        assert_eq!(out.rows(), 4);
        let found = out.find();
        assert!(found.iter().any(|t| t.i == 0 && t.j == 0 && t.v == 1.5));
    }

    /// Values pass through untouched — only the indices are remapped.
    #[test]
    fn values_are_not_remapped() {
        let p = params();
        let mut m = SparseMatrix::assemble(&[0], &[0], &[7.0], 4, 4);
        let mut out = convertsparseformat2d(
            &p,
            &mut m,
            IndexMode::Plain,
            IndexMode::DiamondDifference,
        )
        .unwrap();
        let found = out.find();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].v, 7.0);
    }
}
