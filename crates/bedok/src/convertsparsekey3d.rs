//! Compact a sparse matrix from the full grid onto the fuelled-node numbering.
//!
//! # Provenance
//!
//! - **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
//!   Institute (SNRSI).
//! - **Source file:** `convertsparsekey3d.m`, `main_exec_diff3d_standalone`
//!   snapshot.
//! - **Permission:** given by the author for open-source release under OUTRAM
//!   PARK; see `docs/bedok-port-scoping.md` §6.
//! - **Licence:** GPL-3.0-only.

use crate::matlab::SparseMatrix;

/// `newmat = convertsparsekey3d(mat, key, lennew)`.
///
/// Renumbers a matrix assembled on the full rectangular grid onto the compacted
/// fuelled-node numbering, using the `key` produced by
/// [`crate::convert_grid3d::convert_grid3d`].
///
/// # Arguments
///
/// - `mat` — assembled on the full grid.
/// - `key` — full-grid index → compacted index, `None` for absent nodes. See
///   [`crate::convert_grid3d::convert_grid3d`] for why this is an `Option`
///   rather than the reference's `0` sentinel.
/// - `lennew` — the compacted dimension; the result is `lennew` square.
///
/// # Returns
///
/// The compacted matrix.
///
/// # The skip rule
///
/// Entries are dropped when **all three** of these hold:
///
/// ```text
/// key(i(k))==0 && i(k)==j(k) && v(k)==1
/// ```
///
/// That is: a unit diagonal on a node with no material. The solvers place a
/// `1` on the diagonal of every absent node to keep the full-grid matrix
/// non-singular, and this discards exactly those placeholders. An absent node
/// carrying anything *else* is **not** skipped — it falls through to the
/// diagnostic branch below and then fails.
///
/// # Panics
///
/// If a surviving entry maps through an absent key. MATLAB reaches the same end
/// one step later, via `sparse`'s rejection of a zero subscript.
///
/// # The reference's diagnostic branch
///
/// After recording an entry the reference tests `key(i(k))<=0` and, if so,
/// prints `k`, `i(k)`, `j(k)`, `v(k)`, a decoded `(ix, iy, iz)` and the key —
/// with no trailing semicolons, so MATLAB echoes each to the console. Since
/// `key` is non-negative by construction, this fires exactly when an absent
/// node survived the skip rule, i.e. immediately before the assembly would
/// reject it. It is a "print what went wrong, then die" guard.
///
/// **The decode is hard-coded to one geometry.** It uses the literals `19` and
/// `17`:
///
/// ```text
/// iz=rem(i(k)-1,19)+1
/// iy=rem(i(k)-iz,19*17)/19+1
/// ix=rem(i(k)-(iy-1)*19-iz,19*17*17)/19/17+1
/// ```
///
/// so the `(ix, iy, iz)` it reports is meaningful only for a 17×17×19 grid and
/// is misleading for any other case — including the 17×17×18 grid
/// `main_exec_diff3d.m` currently configures. The arithmetic is reproduced on a
/// 1-based row index (`t.i + 1`) so it yields the same numbers the reference
/// prints, and the output line says what it is worth.
pub fn convertsparsekey3d(
    mat: &mut SparseMatrix,
    key: &[Option<usize>],
    lennew: usize,
) -> SparseMatrix {
    let found = mat.find();

    let mut newi: Vec<usize> = Vec::with_capacity(found.len());
    let mut newj: Vec<usize> = Vec::with_capacity(found.len());
    let mut newv: Vec<f64> = Vec::with_capacity(found.len());

    for (k, t) in found.iter().enumerate() {
        let key_i = key[t.i];

        // Drop the unit diagonal placeholders on absent nodes.
        if key_i.is_none() && t.i == t.j && t.v == 1.0 {
            continue;
        }

        let Some(row) = key_i else {
            // The reference's console dump, reproduced on a 1-based row index
            // so the printed numbers match.
            let ik = t.i + 1;
            let iz = (ik - 1) % 19 + 1;
            let iy = (ik - iz) % (19 * 17) / 19 + 1;
            let ix = (ik - (iy - 1) * 19 - iz) % (19 * 17 * 17) / 19 / 17 + 1;
            eprintln!(
                "convertsparsekey3d: absent key at k={}, i={}, j={}, v={} \
                 (decoded ix={ix}, iy={iy}, iz={iz} — valid only for a 17x17x19 grid)",
                k + 1,
                ik,
                t.j + 1,
                t.v
            );
            panic!(
                "a matrix entry mapped through an absent key: full-grid row {} \
                 carries no material but holds a non-placeholder value {}",
                t.i, t.v
            );
        };

        newi.push(row);
        newj.push(key[t.j].expect("column maps through an absent key"));
        newv.push(t.v);
    }

    SparseMatrix::assemble(&newi, &newj, &newv, lennew, lennew)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entries_are_renumbered_through_the_key() {
        // Full grid of 3; only nodes 0 and 2 are fuelled, becoming 0 and 1.
        let key = vec![Some(0), None, Some(1)];
        let mut mat = SparseMatrix::assemble(&[0, 2], &[0, 2], &[5.0, 6.0], 3, 3);

        let mut out = convertsparsekey3d(&mut mat, &key, 2);

        assert_eq!(out.rows(), 2);
        let found = out.find();
        assert_eq!(found.len(), 2);
        assert!(found.iter().any(|t| t.i == 0 && t.j == 0 && t.v == 5.0));
        assert!(found.iter().any(|t| t.i == 1 && t.j == 1 && t.v == 6.0));
    }

    /// The unit diagonal placeholder on an absent node is dropped rather than
    /// failing.
    #[test]
    fn unit_diagonal_on_an_absent_node_is_skipped() {
        let key = vec![Some(0), None];
        let mut mat = SparseMatrix::assemble(&[0, 1], &[0, 1], &[5.0, 1.0], 2, 2);

        let mut out = convertsparsekey3d(&mut mat, &key, 1);
        assert_eq!(out.nnz(), 1);
    }

    /// An absent node carrying something other than a unit diagonal is *not*
    /// skipped, and fails — the condition the reference's diagnostic branch
    /// exists to report.
    #[test]
    #[should_panic(expected = "absent key")]
    fn a_non_unit_entry_on_an_absent_node_fails() {
        let key = vec![Some(0), None];
        let mut mat = SparseMatrix::assemble(&[0, 1], &[0, 1], &[5.0, 3.0], 2, 2);
        let _ = convertsparsekey3d(&mut mat, &key, 1);
    }
}
