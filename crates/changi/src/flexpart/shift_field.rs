// SPDX-License-Identifier: GPL-3.0
//
// FLEXPART port — provenance
// --------------------------
// Upstream project : FLEXPART (NILU) — https://github.com/flexpart/flexpart
// Upstream version : 10.4 (2019-11-12), commit 3d7eebf
// Upstream source  : src/shift_field_0.f90, src/shift_field.f90
// Original licence : GPL-3.0-or-later — SPDX-FileCopyrightText: FLEXPART 1998-2019
//                    (A. Stohl)
// Ported into this GPL-3.0 work; see LICENSE.flexpart and NOTICE.flexpart.

//! Longitude shift of a global field, and the cyclic duplicate column.
//!
//! For a global grid, `gridcheck`/`readwind` may rotate every field by
//! `nxshift` columns (so a nest can straddle the date line), and always copy
//! column 0 into column `nxf` so the grid closes on itself
//! (`nx = nxfield + 1`). `shift_field_0` does this for a 2-d field,
//! `shift_field` for every level `1..nzf` of time slot `n` of a 4-d field.
//!
//! Column `ix` moves to `ix - nxshift` if `ix >= nxshift`, else to
//! `nxf - nxshift + ix`, for `ix = 0..nxf-1`; then `field(nxf, jy) =
//! field(0, jy)`. A pure permutation and a copy: no arithmetic, so the port
//! is exact at any precision.
//!
//! # Storage
//!
//! The field is a slice with upstream's column-major layout: element
//! `(ix, jy[, kz])` at `ix + ldx*(jy + ldy*kz)`, where `ldx`, `ldy` are the
//! declared extents (`nxmax`, `nymax` upstream). For `shift_field`, pass the
//! time slot `n` (`ldx*ldy*nzfmax` values) as the slice.
//!
//! # Quirks and guards
//!
//! * Rows `nyf..` and levels `nzf..` are untouched.
//! * `nxshift = 0` still writes the duplicate column.
//! * `nxshift == nxf` maps every column to itself (`gridcheck` forbids
//!   `nxshift >= nxfield`, but the routine is well defined there).
//! * `nxshift > nxf` or `< 0` would write `xshiftaux` out of bounds upstream;
//!   the port refuses. So does `ldx <= nxf` (the duplicate column must fit).

/// Why a shift was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShiftError {
    /// `nxshift < 0` or `nxshift > nxf`: upstream writes outside `xshiftaux`.
    ShiftOutOfRange,
    /// `ldx <= nxf`, `nyf > ldy`, or the slice is too short.
    ShapeMismatch,
}

/// `shift_field(field, nxf, nyf, nzfmax, nzf, nmax, n)` on time slot `n`:
/// `slot` holds `ldx*ldy*nzf` (or more) values of that slot.
///
/// # Errors
///
/// See [`ShiftError`].
#[allow(clippy::too_many_arguments)]
pub fn shift_field(
    slot: &mut [f64],
    ldx: usize,
    ldy: usize,
    nxf: usize,
    nyf: usize,
    nzf: usize,
    nxshift: i64,
) -> Result<(), ShiftError> {
    if nxshift < 0 || nxshift > nxf as i64 {
        return Err(ShiftError::ShiftOutOfRange);
    }
    if ldx <= nxf || nyf > ldy || slot.len() < ldx * ldy * nzf {
        return Err(ShiftError::ShapeMismatch);
    }
    let s = nxshift as usize;
    let mut xshiftaux = vec![0.0_f64; nxf];
    for kz in 0..nzf {
        for jy in 0..nyf {
            let row = ldx * (jy + ldy * kz);
            if s != 0 {
                for ix in 0..nxf {
                    let ixs = if ix >= s { ix - s } else { nxf - s + ix };
                    xshiftaux[ixs] = slot[row + ix];
                }
                slot[row..row + nxf].copy_from_slice(&xshiftaux);
            }
            slot[row + nxf] = slot[row];
        }
    }
    Ok(())
}

/// `shift_field_0(field, nxf, nyf)`: the 2-d version (`oro`, `lsm`,
/// `excessoro`, `ewss`, `nsss`), identical to [`shift_field`] with one level.
///
/// # Errors
///
/// See [`ShiftError`].
pub fn shift_field_0(
    field: &mut [f64],
    ldx: usize,
    ldy: usize,
    nxf: usize,
    nyf: usize,
    nxshift: i64,
) -> Result<(), ShiftError> {
    shift_field(field, ldx, ldy, nxf, nyf, 1, nxshift)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shift_rotates_and_closes() {
        // 4 columns + duplicate, 1 row.
        let mut f = vec![0.0, 1.0, 2.0, 3.0, -1.0];
        shift_field_0(&mut f, 5, 1, 4, 1, 1).unwrap();
        assert_eq!(f, vec![1.0, 2.0, 3.0, 0.0, 1.0]);
        assert_eq!(
            shift_field_0(&mut f, 5, 1, 4, 1, 5),
            Err(ShiftError::ShiftOutOfRange)
        );
    }
}
