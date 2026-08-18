//! Zero the negative entries of a sparse matrix.
//!
//! # Provenance
//!
//! - **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
//!   Institute (SNRSI).
//! - **Source file:** `fixnegativematrix.m`, `main_exec_diff3d_standalone`
//!   snapshot.
//! - **Permission:** given by the author for open-source release under OUTRAM
//!   PARK; see the crate README, "Permission and attribution".
//! - **Licence:** GPL-3.0-only.

use crate::matlab::{Array2, SparseMatrix};

/// `mat = fixnegativematrix(mat)`.
///
/// Walks the structural non-zeros and sets every negative one to zero, leaving
/// positives untouched. The reference uses this to clamp a coefficient matrix
/// that has picked up negative entries the downstream solve cannot tolerate.
///
/// # Arguments
///
/// - `mat` — a sparse matrix, modified in place. Unit-agnostic.
///
/// # Note on the reference's variable naming
///
/// The MATLAB destructures with `[i, j, k] = find(mat)`, so its `k` is the
/// **value** vector, not a third index. That is only a naming quirk; the
/// behaviour is `mat(i(n), j(n)) = 0` wherever the value is negative.
///
/// # Cost
///
/// The reference re-indexes the sparse matrix once per negative entry, which is
/// quadratic in the worst case. The translation keeps that structure rather
/// than filtering in one pass, since the no-optimisation rule in
/// the crate README's "Translation policy" covers exactly this kind of rewrite. It is a
/// candidate for a stage-2 change, not a translation-time one.
/// The same clamp for a **dense** matrix — negatives to zero, everywhere.
///
/// # Why this exists separately
///
/// The reference has one `fixnegativematrix.m`, applied to both sparse
/// operators and the dense per-material cross-section tables that
/// [`crate::sigmavalupd3d_handler`] passes it. Its `find(mat)` walk visits only
/// stored non-zeros, which is defect C12 — a real trap for a sparse argument,
/// where a structural zero standing in for a negative would be missed.
///
/// **For a dense argument the two are equivalent**, because every entry is
/// stored and the ones `find` skips are exactly the zeros, which need no
/// clamping. So this is the same function, not a repair: it clamps every
/// negative entry to zero and leaves the rest alone.
pub fn fixnegativematrix_dense(mat: &mut Array2<f64>) {
    for i in 0..mat.rows() {
        for j in 0..mat.cols() {
            if mat.get(i, j) < 0.0 {
                mat.set(i, j, 0.0);
            }
        }
    }
}

pub fn fixnegativematrix(mat: &mut SparseMatrix) {
    let found = mat.find();

    for t in found.iter() {
        if t.v < 0.0 {
            mat.set(t.i, t.j, 0.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn negative_entries_are_zeroed_and_positives_kept() {
        let mut m = SparseMatrix::assemble(
            &[0, 1, 2, 0],
            &[0, 1, 2, 2],
            &[1.0, -2.0, 3.0, -0.5],
            3,
            3,
        );
        fixnegativematrix(&mut m);

        let found = m.find();
        // Zeroed entries stop being structural non-zeros, so only the two
        // positives remain.
        assert_eq!(found.len(), 2);
        assert!(found.iter().all(|t| t.v > 0.0));
        assert!(found.iter().any(|t| t.i == 0 && t.j == 0 && t.v == 1.0));
        assert!(found.iter().any(|t| t.i == 2 && t.j == 2 && t.v == 3.0));
    }

    #[test]
    fn an_all_positive_matrix_is_unchanged() {
        let mut m = SparseMatrix::assemble(&[0, 1], &[0, 1], &[1.0, 2.0], 2, 2);
        fixnegativematrix(&mut m);
        assert_eq!(m.nnz(), 2);
    }
}
