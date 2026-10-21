// SPDX-License-Identifier: GPL-3.0
//
// FLEXPART port — provenance
// --------------------------
// Upstream project : FLEXPART (NILU) — https://github.com/flexpart/flexpart
// Upstream version : 10.4 (2019-11-12), commit 3d7eebf
// Upstream source  : src/assignland.f90
// Original licence : GPL-3.0-or-later — SPDX-FileCopyrightText: FLEXPART 1998-2019
//                    (A. Stohl, S. Eckhardt)
// Ported into this GPL-3.0 work; see LICENSE.flexpart and NOTICE.flexpart.

//! Landuse fractions of the 13 FLEXPART classes at every grid point, from a
//! 0.3° landuse inventory (`assignland.f90`).
//!
//! The inventory (IGBP-DIS DISCover, Belward et al. 1999, as packed in
//! FLEXPART's `IGBP_int1.dat` and unpacked by `readlanduse.f90`) is **data**
//! and is not shipped: [`LanduseInventory`] holds, for each of the
//! 1200 x 600 cells, the three most abundant classes and their weights.
//!
//! # Algorithm (upstream's)
//!
//! 1. Per inventory cell, class `k_li` gets `p_li / (p_1 + p_2 + p_3)` (0 if
//!    the weights sum to 0).
//! 2. Per grid point, the 10 x 10 sample points
//!    `(ix + (iix-1)/10, jy + (jjy-1)/10)` are mapped to inventory cells and
//!    their class fractions summed.
//! 3. If anything was found: divide by 100, and renormalise to 1 if the sum
//!    is below `1 - 1e-5`. Otherwise use the land-sea mask: class 3 (ocean)
//!    if `lsm < 0.1`, else class 7.
//!
//! # Upstream quirks reproduced
//!
//! - **Duplicate classes overwrite.** If two of a cell's three entries name
//!   the same class, the later weight **replaces** the earlier one rather
//!   than adding to it, so that cell's fractions sum to less than 1.
//! - **The samples are not centred on the grid point**: they cover
//!   `[ix, ix + 0.9] x [jy, jy + 0.9]` grid lengths, i.e. the cell to the
//!   north-east.
//! - **Latitude wraps.** A sample at or north of 90° maps to inventory row
//!   601+ and is moved to row `yj - 600`, i.e. next to the **South Pole**.
//!   On a global grid the top row's samples at 90.1°–90.9° therefore average
//!   Antarctic landuse. (Exactly 90° itself also maps to row 601 in double
//!   precision, `180/0.3 = 600`.)
//! - Mother grid only: a longitude at or beyond 180° is wrapped once by
//!   `-360`, and a cell index below 0 stops the program. The nest loop has
//!   neither (its longitudes beyond 180° are wrapped through the cell index
//!   instead, which is equivalent up to rounding).
//! - Upstream's running `sumperc` adds the accumulated value, not the
//!   increment; it is only tested for `> 0`. Reproduced literally.
//!
//! Cases where upstream indexes outside its arrays (a class number outside
//! `1..=13`, an inventory cell index of 0 or below) are refused with
//! [`LanduseError`] instead of guessed.

use super::constants::NUMCLASS;
use super::met_fields::{Field2, GridGeometry};

/// Inventory cells in longitude (`lumaxx`).
pub const LUMAXX: usize = 1200;
/// Inventory cells in latitude (`lumaxy`).
pub const LUMAXY: usize = 600;
/// Inventory resolution, degrees (`dxlu`).
pub const DXLU: f64 = 0.3;
/// Inventory origin longitude, degrees (`xlon0lu`).
pub const XLON0LU: i64 = -180;
/// Inventory origin latitude, degrees (`ylat0lu`).
pub const YLAT0LU: i64 = -90;

/// The landuse inventory as `readlanduse.f90` leaves it in `landinvent`.
/// Cell `(i, j)` (1-based, `i` along longitude from 180°W, `j` along
/// latitude from 90°S) is element `(i-1) + 1200*(j-1)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanduseInventory {
    /// The three class numbers of each cell (`landinvent(i,j,1:3)`).
    pub classes: Vec<[i8; 3]>,
    /// Their weights (`landinvent(i,j,4:6)`), 0–15 as unpacked upstream.
    pub weights: Vec<[i8; 3]>,
}

impl LanduseInventory {
    /// Class fractions of cell `(i, j)` (1-based), upstream step 1.
    fn cell_fractions(&self, i: usize, j: usize) -> Result<[f64; NUMCLASS], LanduseError> {
        let c = (i - 1) + LUMAXX * (j - 1);
        let (cls, w) = (self.classes[c], self.weights[c]);
        let mut sumperc = 0.0;
        for &wi in &w {
            sumperc += f64::from(wi);
        }
        let mut out = [0.0; NUMCLASS];
        for li in 0..3 {
            let k = cls[li];
            if k < 1 || k as usize > NUMCLASS {
                return Err(LanduseError::ClassOutOfRange);
            }
            let p = if sumperc > 0.0 {
                f64::from(w[li]) / sumperc
            } else {
                0.0
            };
            out[k as usize - 1] = p;
        }
        Ok(out)
    }
}

/// Mother grid or nest (`assignland` treats them differently, see the module
/// docs).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LanduseGrid {
    /// The mother-grid loop.
    Mother,
    /// The nest loop.
    Nest,
}

/// Why [`assignland`] refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LanduseError {
    /// A sample maps to a longitude cell below 0: upstream prints
    /// "problem with landuseinv sampling" and stops (mother grid).
    SamplingStop,
    /// A sample maps to inventory cell 0 (or below) in either direction,
    /// which upstream reads outside its array.
    CellOutOfRange,
    /// An inventory class number outside `1..=13`, written outside the array
    /// upstream.
    ClassOutOfRange,
    /// The inventory or the land-sea mask has the wrong size.
    ShapeMismatch,
}

/// `assignland.f90`: landuse fractions `xlanduse(ix, jy, 1:13)` (or
/// `xlandusen`) for every point of a grid; index `ix + nx*jy`.
///
/// `lsm` is the land-sea mask of the grid (fraction land).
///
/// # Errors
/// See [`LanduseError`].
pub fn assignland(
    inv: &LanduseInventory,
    geom: &GridGeometry,
    lsm: &Field2,
    kind: LanduseGrid,
) -> Result<Vec<[f64; NUMCLASS]>, LanduseError> {
    const NREFINE: usize = 10;
    if inv.classes.len() != LUMAXX * LUMAXY
        || inv.weights.len() != LUMAXX * LUMAXY
        || lsm.nx != geom.nx
        || lsm.ny != geom.ny
    {
        return Err(LanduseError::ShapeMismatch);
    }
    let wrap_lon = XLON0LU as f64 + LUMAXX as f64 * DXLU;
    let mut out = vec![[0.0; NUMCLASS]; geom.nx * geom.ny];
    for ix in 0..geom.nx {
        for jy in 0..geom.ny {
            let mut xl = [0.0_f64; NUMCLASS];
            // upstream's running check sum: it adds the accumulated value,
            // not the increment; only its sign is used
            let mut sumperc = 0.0;
            for iix in 1..=NREFINE {
                let mut xlon =
                    (ix as f64 + (iix - 1) as f64 / NREFINE as f64) * geom.dx + geom.xlon0;
                if kind == LanduseGrid::Mother && xlon >= wrap_lon {
                    xlon -= LUMAXX as f64 * DXLU;
                }
                for jjy in 1..=NREFINE {
                    let ylat =
                        (jy as f64 + (jjy - 1) as f64 / NREFINE as f64) * geom.dy + geom.ylat0;
                    let mut xi = ((xlon - XLON0LU as f64) / DXLU).trunc() + 1.0;
                    let mut yj = ((ylat - YLAT0LU as f64) / DXLU).trunc() + 1.0;
                    if xi > LUMAXX as f64 {
                        xi -= LUMAXX as f64;
                    }
                    if yj > LUMAXY as f64 {
                        yj -= LUMAXY as f64;
                    }
                    if kind == LanduseGrid::Mother && xi < 0.0 {
                        return Err(LanduseError::SamplingStop);
                    }
                    if xi < 1.0 || yj < 1.0 || xi > LUMAXX as f64 || yj > LUMAXY as f64 {
                        return Err(LanduseError::CellOutOfRange);
                    }
                    let p = inv.cell_fractions(xi as usize, yj as usize)?;
                    for k in 0..NUMCLASS {
                        xl[k] += p[k];
                        sumperc += xl[k];
                    }
                }
            }
            if sumperc > 0.0 {
                sumperc = 0.0;
                for v in &mut xl {
                    *v /= (NREFINE * NREFINE) as f64;
                    sumperc += *v;
                }
                if sumperc < 1.0 - 1.0e-5 {
                    for v in &mut xl {
                        *v /= sumperc;
                    }
                }
            } else if lsm.at(ix, jy) < 0.1 {
                xl[2] = 1.0; // ocean
            } else {
                xl[6] = 1.0; // class 7
            }
            out[ix + geom.nx * jy] = xl;
        }
    }
    Ok(out)
}
