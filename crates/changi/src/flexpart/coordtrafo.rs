// SPDX-License-Identifier: GPL-3.0
//
// FLEXPART port — provenance
// --------------------------
// Upstream project : FLEXPART (NILU) — https://github.com/flexpart/flexpart
// Upstream version : 10.4 (2019-11-12), commit 3d7eebf
// Upstream source  : src/coordtrafo.f90 (subroutine coordtrafo;
//                    G. Wotawa 1994-02-07, last update A. Stohl 1996-05-18)
// Original licence : GPL-3.0-or-later — SPDX-FileCopyrightText: FLEXPART 1998-2019
// Ported into this GPL-3.0 work; see LICENSE.flexpart and NOTICE.flexpart.

//! Release-point coordinate transformation: `coordtrafo.f90`.
//!
//! Converts each release box from geographic degrees to mother-grid units,
//! `x = (lon - xlon0)/dx`, `y = (lat - ylat0)/dy`, then removes boxes that
//! fall outside the domain. Upstream reads the boxes from `point_mod`
//! (`xpoint1`, `ypoint1`, `xpoint2`, `ypoint2`) and the grid from `com_mod`
//! (`xlon0`, `ylat0`, `dx`, `dy`, `nxmin1`, `nymin1`, `xglobal`, `sglobal`,
//! `nglobal`); the port takes both as arguments and returns the surviving
//! boxes with their **original indices**, so a caller can carry the other
//! per-point arrays (`zpoint*`, `npart`, `kindz`, release times, `xmass`) the
//! way upstream shifts them.
//!
//! # Translation notes and upstream quirks
//!
//! * The domain test, after two polar clamps, is
//!   - `ypoint1 < 1e-6` or `ypoint1 >= nymin1 - 1e-6`, or
//!   - `ypoint2 < 1e-6` or `ypoint2 >= nymin1 - yrspc`, or
//!   - (non-global in x only) `xpoint1`/`xpoint2` `< 1e-6` or
//!     `>= nxmin1 - 1e-6`,
//!
//!   where `yrspc = spacing(real(nymin1, kind=sp))` is the **single
//!   precision** spacing at `nymin1` in both upstream builds (`sp` is
//!   `selected_real_kind(6)`, untouched by `-fdefault-real-8`); the port
//!   computes it from the `f32` bit pattern ([`spacing_f32`]).
//! * Clamps: with `sglobal`, `ypoint1 < 1e-6` becomes `1e-6`; with `nglobal`,
//!   `ypoint2 > nymin1 - 1e-5` becomes `nymin1 - 10*yrspc`. **`ypoint1` is
//!   never clamped at the north pole and `ypoint2` never at the south**, so
//!   a box whose *lower* edge is at the north pole is removed even on a
//!   global grid, while a box whose lower edge is at the south pole is kept.
//! * The upper bound is asymmetric: `1e-6` for `ypoint1`, `yrspc` for
//!   `ypoint2`.
//! * **On a global 1-degree grid the margins are finer than the shipped
//!   build can resolve.** At `y = 180` an `f32` has a spacing of 1.5e-5, wider
//!   than the 1e-6 and 1e-5 margins, so for boxes whose edge is within ~1e-5
//!   deg of the north pole the outcome is decided by rounding: measured
//!   (2026-10-02) on four such boxes, the shipped build keeps 2, the real(8)
//!   build and this port keep 3.
//! * On a globally cyclic grid (`xglobal`) x is neither checked nor wrapped:
//!   a box west of `xlon0` keeps a negative x, one past `nxmin1` keeps it.
//! * Upstream restarts the scan from point 1 (`goto 15`) after every removal.
//!   The port does the same; because both clamps are idempotent this yields
//!   the same survivors as a single filtering pass.
//! * If no box survives (or none was given) upstream prints an error and
//!   executes `stop`; the port returns [`NoReleasePoints`].
//! * Upstream shifts `compoint` (the point names, 1001 slots) only for
//!   `j <= 1000`; names are not ported (they only feed messages here).
//!
//! # Units
//!
//! Input boxes in degrees; output boxes in mother-grid units; `dx`, `dy` in
//! degrees per grid cell.

/// The mother-grid description `coordtrafo` reads from `com_mod`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReleaseGrid {
    /// `xlon0`: longitude of the lower-left grid point, deg.
    pub xlon0: f64,
    /// `ylat0`: latitude of the lower-left grid point, deg.
    pub ylat0: f64,
    /// `dx`: grid distance in x, deg.
    pub dx: f64,
    /// `dy`: grid distance in y, deg.
    pub dy: f64,
    /// `nxmin1 = nx - 1`.
    pub nxmin1: i32,
    /// `nymin1 = ny - 1`.
    pub nymin1: i32,
    /// `xglobal`: the grid is cyclic in longitude.
    pub xglobal: bool,
    /// `sglobal`: the grid contains the south pole.
    pub sglobal: bool,
    /// `nglobal`: the grid contains the north pole.
    pub nglobal: bool,
}

/// One release box, `point_mod`'s `xpoint1, ypoint1, xpoint2, ypoint2`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReleaseBox {
    /// Lower-left x (deg on input, grid units on output).
    pub xpoint1: f64,
    /// Lower-left y.
    pub ypoint1: f64,
    /// Upper-right x.
    pub xpoint2: f64,
    /// Upper-right y.
    pub ypoint2: f64,
}

/// Upstream's `stop` "NO PARTICLE RELEASES ARE DEFINED": every box was out of
/// the domain, or none was given.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoReleasePoints;

/// Fortran `spacing(x)` for a default-kind (`f32`) `x`: `2^(e - 24)` where
/// `x = f * 2^e`, `0.5 <= |f| < 1`; `tiny(x)` for zero or when that would be
/// subnormal. Returned widened to `f64` (exact).
pub fn spacing_f32(x: f32) -> f64 {
    let biased = ((x.to_bits() >> 23) & 0xff) as i32;
    if x == 0.0 || biased == 0 {
        return f32::MIN_POSITIVE as f64;
    }
    // x = 1.m * 2^(biased-127); Fortran exponent e = biased - 126.
    let p = biased - 126 - 24;
    if p < -126 {
        f32::MIN_POSITIVE as f64
    } else {
        (2.0_f64).powi(p)
    }
}

/// `coordtrafo`: transform release boxes to grid units and drop those outside
/// the domain. Returns `(original index, box in grid units)` for every
/// survivor, in upstream's order.
pub fn coordtrafo(
    grid: &ReleaseGrid,
    points: &[ReleaseBox],
) -> Result<Vec<(usize, ReleaseBox)>, NoReleasePoints> {
    if points.is_empty() {
        return Err(NoReleasePoints);
    }
    let mut pts: Vec<(usize, ReleaseBox)> = points
        .iter()
        .enumerate()
        .map(|(i, p)| {
            (
                i,
                ReleaseBox {
                    xpoint1: (p.xpoint1 - grid.xlon0) / grid.dx,
                    xpoint2: (p.xpoint2 - grid.xlon0) / grid.dx,
                    ypoint1: (p.ypoint1 - grid.ylat0) / grid.dy,
                    ypoint2: (p.ypoint2 - grid.ylat0) / grid.dy,
                },
            )
        })
        .collect();

    let yrspc = spacing_f32(grid.nymin1 as f32);
    let ny = grid.nymin1 as f64;
    let nx = grid.nxmin1 as f64;
    'restart: loop {
        // `do i=1,numpoint` with the bound fixed at loop entry (label 15).
        let n = pts.len();
        let mut i = 0;
        while i < n {
            let p = &mut pts[i].1;
            if grid.sglobal && p.ypoint1 < 1.0e-6 {
                p.ypoint1 = 1.0e-6;
            }
            if grid.nglobal && p.ypoint2 > ny - 1.0e-5 {
                p.ypoint2 = ny - 10.0 * yrspc;
            }
            let out = p.ypoint1 < 1.0e-6
                || p.ypoint1 >= ny - 1.0e-6
                || p.ypoint2 < 1.0e-6
                || p.ypoint2 >= ny - yrspc
                || (!grid.xglobal
                    && (p.xpoint1 < 1.0e-6
                        || p.xpoint1 >= nx - 1.0e-6
                        || p.xpoint2 < 1.0e-6
                        || p.xpoint2 >= nx - 1.0e-6));
            if out {
                pts.remove(i);
                if !pts.is_empty() {
                    continue 'restart;
                }
                // numpoint = 0: the loop bound was 1, so upstream's loop ends
                // here and reaches the `stop` at label 30.
                break 'restart;
            }
            i += 1;
        }
        break;
    }
    if pts.is_empty() {
        Err(NoReleasePoints)
    } else {
        Ok(pts)
    }
}
