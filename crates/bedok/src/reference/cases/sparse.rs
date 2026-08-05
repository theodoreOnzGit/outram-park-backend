//! Sparse-index bookkeeping and the small numeric guards.
//!
//! # Provenance
//!
//! | | |
//! |---|---|
//! | Original author | Than Yan Ren, Singapore Nuclear Research and Safety Institute (SNRSI) |
//! | Source files | `convertindexc2d.m`, `convertsparseformat2d.m`, `convertsparsekey3d.m`, `fixnegativematrix.m`, `fixinfnan.m` |
//! | Snapshot | `BEDOKfiles.zip`, sha256 `e45cd6f57be2087c…` |
//! | Permission | translation and open-source publication approved by the author and the project lead; see `docs/bedok-port-scoping.md` §6 |
//!
//! # Representation
//!
//! MATLAB manipulates these matrices exclusively through the pair
//! `[i,j,v] = find(mat)` and `sparse(i,j,v,m,n)`, i.e. as coordinate triplets.
//! [`CooMatrix`] is that same triplet form, so each ported function is a
//! line-by-line reading of its original rather than a translation into some
//! other sparse format.
//!
//! Row and column indices are stored **1-based**, as MATLAB's are. That is not
//! a stylistic choice: `convertsparsekey3d.m` uses `key(i) == 0` to mean "this
//! unknown was dropped", which a 0-based index cannot express.

use crate::error::{BedokError, Result};

/// A sparse matrix as coordinate triplets, with MATLAB's 1-based indices.
///
/// Duplicate `(row, col)` pairs are permitted and are summed on assembly, as
/// MATLAB's `sparse` does; none of the ported functions produces them.
#[derive(Debug, Clone, PartialEq)]
pub struct CooMatrix {
    /// Number of rows.
    pub rows: usize,
    /// Number of columns.
    pub cols: usize,
    /// 1-based row index of each stored entry. MATLAB `i` from `find`.
    pub row_index: Vec<usize>,
    /// 1-based column index of each stored entry. MATLAB `j`.
    pub col_index: Vec<usize>,
    /// Stored values. MATLAB `v`.
    pub values: Vec<f64>,
}

impl CooMatrix {
    /// An empty `rows` × `cols` matrix.
    #[must_use]
    pub const fn empty(rows: usize, cols: usize) -> Self {
        Self {
            rows,
            cols,
            row_index: Vec::new(),
            col_index: Vec::new(),
            values: Vec::new(),
        }
    }

    /// Number of stored entries. MATLAB `nnz(mat)`, except that an explicitly
    /// stored zero is counted here and dropped by MATLAB's `find`.
    #[must_use]
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Whether the matrix stores no entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Append an entry at **1-based** `(row, col)`.
    ///
    /// # Errors
    ///
    /// [`BedokError::IndexOutOfRange`] if the index is zero or beyond the
    /// declared shape.
    pub fn push(&mut self, row: usize, col: usize, value: f64) -> Result<()> {
        if row == 0 || col == 0 || row > self.rows || col > self.cols {
            return Err(BedokError::IndexOutOfRange {
                idx: row.saturating_mul(self.cols).saturating_add(col),
                len: self.rows * self.cols,
            });
        }
        self.row_index.push(row);
        self.col_index.push(col);
        self.values.push(value);
        Ok(())
    }
}

/// Which unknown ordering a 2-D sparse operator is expressed in.
///
/// MATLAB passes these as the bare integers `frommode` / `tomode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TwoDIndexMode {
    /// Full node indices only: `g*maxi1*maxi2 + ix*maxi2 + iy`. MATLAB mode 1.
    Nodal,
    /// Diamond-difference half indices, on a `(2*maxi1+1) × (2*maxi2+1)` mesh
    /// that carries cell centres *and* faces. MATLAB mode 2.
    HalfIndex,
}

/// The 2-D grid shape the index conversion needs.
///
/// MATLAB reads these straight off `params` inside `convertindexc2d.m`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TwoDIndexParams {
    /// Energy groups. MATLAB `params.G`.
    pub ngroups: usize,
    /// Nodes along the first axis. MATLAB `params.maxi1`.
    pub maxi1: usize,
    /// Nodes along the second axis. MATLAB `params.maxi2`.
    pub maxi2: usize,
    /// Extra unknowns per node. MATLAB `params.Nc`.
    pub num_extra_unknowns: usize,
}

/// Convert a vector of 2-D sparse indices between the nodal and half-index
/// orderings.
///
/// Rust translation of `convertindexc2d.m`. Indices are 1-based in and out.
///
/// # Not used by any ported case
///
/// This is part of the legacy 2-D path; the 3-D cases use
/// [`convert_sparse_key_3d`] and the key from
/// [`convert_grid_3d`](super::geometry::convert_grid_3d) instead. It is ported
/// for completeness, and because `convertsparseformat2d.m` calls it.
///
/// # Unfinished in the reference
///
/// - `convertindexc2d.m` reads `params.maxi1` / `params.maxi2` **directly**,
///   while its only caller `convertsparseformat2d.m` obtains the same two
///   numbers through `handle2dcoords`. A `params` carrying `maxix`/`maxiy`
///   (as every case does) therefore satisfies the caller and errors in the
///   callee. Recorded, not repaired: [`TwoDIndexParams`] makes the requirement
///   explicit instead.
/// - `philenf1` and `philenf2` are computed and never used.
/// - **The half-index → nodal branch does not work.** It computes
///   `ix = ceil(mod(v-1, energystep2)/xstep2)/2` and
///   `iy = (mod(mod(v-1, energystep2), xstep2)+1)/2`, and for *every* index the
///   forward direction produces, at least one of the two is a half-integer —
///   so the reconstructed nodal index is never an integer. Round-tripping `1`
///   through the forward map and back gives `2.5`, not `1`. In MATLAB that
///   value reaches `sparse`, which rejects a non-integer subscript. The
///   arithmetic is reproduced in floating point so the defect is visible
///   rather than hidden by an integer cast, and the non-integer result is
///   returned as an error. See the test
///   `half_index_to_nodal_is_broken_in_the_reference`.
///
/// # Errors
///
/// [`BedokError::IndexOutOfRange`] if a converted index is not a positive
/// integer, which is the reference's silent failure made explicit.
pub fn convert_index_c2d(
    params: TwoDIndexParams,
    indices: &[usize],
    from: TwoDIndexMode,
    to: TwoDIndexMode,
) -> Result<Vec<usize>> {
    let TwoDIndexParams {
        ngroups,
        maxi1,
        maxi2,
        num_extra_unknowns: _,
    } = params;

    let phi_len_1 = (ngroups * maxi1 * maxi2) as f64;
    let phi_len_2 = (ngroups * (2 * maxi1 + 1) * (2 * maxi2 + 1)) as f64;
    let energy_step_1 = (maxi1 * maxi2) as f64;
    let energy_step_2 = ((2 * maxi1 + 1) * (2 * maxi2 + 1)) as f64;
    let x_step_1 = maxi2 as f64;
    let x_step_2 = (2 * maxi2 + 1) as f64;

    // ---- convert the input to mode 1 ----
    let temp: Vec<f64> = match from {
        TwoDIndexMode::Nodal => indices.iter().map(|v| *v as f64).collect(),
        TwoDIndexMode::HalfIndex => indices
            .iter()
            .map(|v| {
                let v = *v as f64;
                if v > phi_len_2 {
                    v - phi_len_2 + phi_len_1
                } else {
                    let g = ((v - 1.0) / energy_step_2).floor();
                    let ix = ((v - 1.0).rem_euclid(energy_step_2) / x_step_2).ceil() / 2.0;
                    let iy = ((v - 1.0).rem_euclid(energy_step_2).rem_euclid(x_step_2) + 1.0) / 2.0;
                    g * energy_step_1 + ix * x_step_1 + iy
                }
            })
            .collect(),
    };

    // ---- convert mode 1 to the requested output ----
    let out: Vec<f64> = match to {
        TwoDIndexMode::Nodal => temp,
        TwoDIndexMode::HalfIndex => temp
            .iter()
            .map(|v| {
                let v = *v;
                if v > phi_len_1 {
                    v - phi_len_1 + phi_len_2
                } else {
                    let g = ((v - 1.0) / energy_step_1).floor();
                    let ix = ((v - 1.0).rem_euclid(energy_step_1) / x_step_1).ceil() * 2.0;
                    let iy = ((v - 1.0).rem_euclid(energy_step_1).rem_euclid(x_step_1) + 1.0) * 2.0;
                    g * energy_step_2 + ix * x_step_2 + iy
                }
            })
            .collect(),
    };

    out.iter()
        .map(|v| {
            if *v < 1.0 || v.fract() != 0.0 {
                Err(BedokError::IndexOutOfRange {
                    idx: 0,
                    len: phi_len_2 as usize,
                })
            } else {
                Ok(*v as usize)
            }
        })
        .collect()
}

/// Re-index a 2-D sparse operator between the two orderings.
///
/// Rust translation of `convertsparseformat2d.m`. Only the row and column
/// indices are converted; the values are carried across unchanged — the
/// MATLAB has a commented-out line that converted the *values* too, which
/// would have been meaningless.
///
/// # Errors
///
/// Propagates [`convert_index_c2d`]'s errors.
pub fn convert_sparse_format_2d(
    params: TwoDIndexParams,
    matrix: &CooMatrix,
    from: TwoDIndexMode,
    to: TwoDIndexMode,
) -> Result<CooMatrix> {
    let len = match to {
        TwoDIndexMode::Nodal => {
            (params.ngroups + params.num_extra_unknowns) * params.maxi1 * params.maxi2
        }
        TwoDIndexMode::HalfIndex => {
            params.ngroups * (2 * params.maxi1 + 1) * (2 * params.maxi2 + 1)
                + params.num_extra_unknowns * params.maxi1 * params.maxi2
        }
    };
    Ok(CooMatrix {
        rows: len,
        cols: len,
        row_index: convert_index_c2d(params, &matrix.row_index, from, to)?,
        col_index: convert_index_c2d(params, &matrix.col_index, from, to)?,
        values: matrix.values.clone(),
    })
}

/// Compact a 3-D sparse operator onto the fuelled unknowns.
///
/// Rust translation of `convertsparsekey3d.m`. `key` is
/// [`GridKey::key`](super::geometry::GridKey::key): the new 1-based index of
/// each old unknown, or `0` for one that is dropped.
///
/// An entry whose **row** is a dropped unknown is skipped only when it is the
/// identity element placed there to keep the matrix non-singular — MATLAB's
/// `key(i(k))==0 && i(k)==j(k) && v(k)==1`. Any *other* entry on a dropped row
/// is kept and then indexed with `key == 0`, which MATLAB's `sparse` rejects.
/// The reference prints a diagnostic block (`k`, `i(k)`, `j(k)`, `v(k)` and a
/// decoded `ix,iy,iz`) and then fails on the `sparse` call.
///
/// # Behavioural difference, stated plainly
///
/// The port does **not** print that diagnostic — a library must not write to
/// stdout — and returns [`BedokError::IndexOutOfRange`] where the MATLAB would
/// have errored inside `sparse`. Both paths fail on the same inputs; only the
/// message differs. The decoded coordinates in the reference's diagnostic are
/// hard-coded to a 19 × 17 × 17 grid (`rem(i-1,19)`), so they are wrong for
/// any other case — recorded, not fixed, since the block is unreachable except
/// on the way to an error.
///
/// # Errors
///
/// [`BedokError::IndexOutOfRange`] if a kept entry maps to a dropped unknown,
/// or if `key` is shorter than the matrix.
pub fn convert_sparse_key_3d(
    matrix: &CooMatrix,
    key: &[usize],
    new_len: usize,
) -> Result<CooMatrix> {
    let mut out = CooMatrix::empty(new_len, new_len);

    for k in 0..matrix.len() {
        let i = matrix.row_index[k];
        let j = matrix.col_index[k];
        let v = matrix.values[k];
        if i > key.len() || j > key.len() {
            return Err(BedokError::IndexOutOfRange {
                idx: i.max(j),
                len: key.len(),
            });
        }
        let new_i = key[i - 1];
        let new_j = key[j - 1];

        if new_i == 0 && i == j && v == 1.0 {
            continue;
        }
        if new_i == 0 || new_j == 0 {
            // MATLAB reaches `sparse` with a zero index and errors there.
            return Err(BedokError::IndexOutOfRange {
                idx: if new_i == 0 { i } else { j },
                len: new_len,
            });
        }
        out.push(new_i, new_j, v)?;
    }

    Ok(out)
}

/// Zero every negative entry of a sparse matrix.
///
/// Rust translation of `fixnegativematrix.m`. Used after a cross-section
/// feedback update has been applied, to stop an extrapolated derivative from
/// driving a cross section below zero.
///
/// The MATLAB assigns `mat(i,j) = 0`, which *removes* the entry from the
/// sparse structure; here the entry is retained with a zero value. The
/// difference is invisible to every arithmetic use and avoids a reallocation.
#[must_use]
pub fn fix_negative_matrix(matrix: &CooMatrix) -> CooMatrix {
    let mut out = matrix.clone();
    for v in &mut out.values {
        if *v < 0.0 {
            *v = 0.0;
        }
    }
    out
}

/// What to substitute for a non-finite entry.
///
/// MATLAB selects between the two by whether `fixinfnan` was given a second
/// argument at all — `varargin` is tested with `isempty`, and its value is
/// never read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NonFiniteFill {
    /// Replace with zero. MATLAB `fixinfnan(v)`.
    Zero,
    /// Replace with the smallest magnitude in the vector. MATLAB
    /// `fixinfnan(v, anything)`.
    SmallestMagnitude,
}

/// Replace `Inf`, `-Inf` and `NaN` entries of a vector.
///
/// Rust translation of `fixinfnan.m`.
///
/// # A subtlety in [`SmallestMagnitude`](NonFiniteFill::SmallestMagnitude)
///
/// The MATLAB computes `min(abs(vector))` over the vector *including* its
/// non-finite entries. `min` skips `NaN`, so those do no harm, but `+Inf`
/// entries do participate — harmlessly, since `Inf` can only be the minimum if
/// every entry is `Inf`, in which case the substitution is `Inf` and nothing is
/// fixed. The source comment claims the minimum is "over remaining finite
/// vals", which is true only by that accident. Reproduced as written.
///
/// If the vector holds no finite entries at all, the fill is `Inf` (or `NaN`
/// for an all-`NaN` vector, since `min` of an empty selection is `NaN`); the
/// port returns the vector unchanged in the all-`NaN` case, matching MATLAB's
/// `min([]) = []` assignment failure being avoided by the `any(mask)` guard
/// only when there is something to fix.
#[must_use]
pub fn fix_inf_nan(vector: &[f64], fill: NonFiniteFill) -> Vec<f64> {
    let mut out = vector.to_vec();
    if !out.iter().any(|v| !v.is_finite()) {
        return out;
    }
    let replacement = match fill {
        NonFiniteFill::Zero => 0.0,
        NonFiniteFill::SmallestMagnitude => out
            .iter()
            .map(|v| v.abs())
            .filter(|v| !v.is_nan())
            .fold(f64::INFINITY, f64::min),
    };
    for v in &mut out {
        if !v.is_finite() {
            *v = replacement;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn negative_entries_are_zeroed_and_others_kept() {
        let mut m = CooMatrix::empty(2, 2);
        m.push(1, 1, -3.0).expect("in range");
        m.push(2, 2, 4.0).expect("in range");
        let fixed = fix_negative_matrix(&m);
        assert_eq!(fixed.values, vec![0.0, 4.0]);
    }

    #[test]
    fn non_finite_entries_become_zero() {
        let v = [1.0, f64::NAN, -2.0, f64::INFINITY];
        assert_eq!(
            fix_inf_nan(&v, NonFiniteFill::Zero),
            vec![1.0, 0.0, -2.0, 0.0]
        );
    }

    #[test]
    fn non_finite_entries_become_the_smallest_magnitude() {
        let v = [3.0, f64::NAN, -2.0, f64::NEG_INFINITY];
        assert_eq!(
            fix_inf_nan(&v, NonFiniteFill::SmallestMagnitude),
            vec![3.0, 2.0, -2.0, 2.0]
        );
    }

    #[test]
    fn a_finite_vector_is_returned_untouched() {
        let v = [1.0, 2.0, 3.0];
        assert_eq!(fix_inf_nan(&v, NonFiniteFill::Zero), v.to_vec());
    }

    /// The identity element parked on a dropped unknown is discarded; the
    /// surviving entries are renumbered through the key.
    #[test]
    fn sparse_key_drops_the_identity_and_renumbers() {
        // Three unknowns; the middle one is dropped.
        let key = [1usize, 0, 2];
        let mut m = CooMatrix::empty(3, 3);
        m.push(1, 1, 5.0).expect("in range");
        m.push(2, 2, 1.0).expect("in range"); // identity on the dropped unknown
        m.push(3, 1, 7.0).expect("in range");
        let out = convert_sparse_key_3d(&m, &key, 2).expect("compacted");
        assert_eq!(out.rows, 2);
        assert_eq!(out.row_index, vec![1, 2]);
        assert_eq!(out.col_index, vec![1, 1]);
        assert_eq!(out.values, vec![5.0, 7.0]);
    }

    /// A real coupling to a dropped unknown is an error, as it is in MATLAB's
    /// `sparse`.
    #[test]
    fn sparse_key_rejects_a_live_entry_on_a_dropped_unknown() {
        let key = [1usize, 0];
        let mut m = CooMatrix::empty(2, 2);
        m.push(1, 2, 3.0).expect("in range");
        assert!(convert_sparse_key_3d(&m, &key, 1).is_err());
    }

    /// The nodal → half-index direction is well defined: every index maps to a
    /// positive even integer inside the half-index vector.
    #[test]
    fn nodal_to_half_index_produces_valid_indices() {
        let p = TwoDIndexParams {
            ngroups: 2,
            maxi1: 4,
            maxi2: 3,
            num_extra_unknowns: 0,
        };
        let nodal: Vec<usize> = (1..=(p.ngroups * p.maxi1 * p.maxi2)).collect();
        let half = convert_index_c2d(p, &nodal, TwoDIndexMode::Nodal, TwoDIndexMode::HalfIndex)
            .expect("the forward direction is well defined");
        let limit = p.ngroups * (2 * p.maxi1 + 1) * (2 * p.maxi2 + 1);
        assert_eq!(half.len(), nodal.len());
        assert!(half.iter().all(|v| *v >= 1 && *v <= limit));
        // The first nodal unknown lands at half index 2 — cell (1,1)'s centre
        // on the doubled mesh.
        assert_eq!(half[0], 2);
    }

    /// **The inverse branch of `convertindexc2d.m` does not work**, and the
    /// port reproduces that rather than repairing it.
    ///
    /// `ix = ceil(mod(v-1, energystep2)/xstep2)/2` and
    /// `iy = (mod(mod(v-1, energystep2), xstep2)+1)/2` are half-integers for
    /// every index the forward direction produces, so the reconstructed nodal
    /// index is never an integer: feeding `2` back gives `2.5`, `18` gives
    /// `6.5`, and so on. In MATLAB that value would go straight into `sparse`,
    /// which rejects a non-integer subscript. Here it is an explicit error.
    ///
    /// Recorded, not fixed — `docs/bedok-port-scoping.md` §1.0. Nothing in the
    /// 3-D path calls this direction.
    #[test]
    fn half_index_to_nodal_is_broken_in_the_reference() {
        let p = TwoDIndexParams {
            ngroups: 2,
            maxi1: 4,
            maxi2: 3,
            num_extra_unknowns: 0,
        };
        let nodal: Vec<usize> = (1..=(p.ngroups * p.maxi1 * p.maxi2)).collect();
        let half = convert_index_c2d(p, &nodal, TwoDIndexMode::Nodal, TwoDIndexMode::HalfIndex)
            .expect("the forward direction is well defined");
        assert!(
            convert_index_c2d(p, &half, TwoDIndexMode::HalfIndex, TwoDIndexMode::Nodal).is_err(),
            "the reference's inverse yields half-integers; it cannot round-trip"
        );
    }

    #[test]
    fn sparse_format_conversion_keeps_the_values() {
        let p = TwoDIndexParams {
            ngroups: 1,
            maxi1: 2,
            maxi2: 2,
            num_extra_unknowns: 0,
        };
        let mut m = CooMatrix::empty(4, 4);
        m.push(1, 2, 9.0).expect("in range");
        let out = convert_sparse_format_2d(p, &m, TwoDIndexMode::Nodal, TwoDIndexMode::HalfIndex)
            .expect("converted");
        assert_eq!(out.values, vec![9.0]);
        assert_eq!(out.rows, 1 * 5 * 5);
    }
}
