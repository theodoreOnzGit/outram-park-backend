// SPDX-License-Identifier: GPL-3.0
//
// FLEXPART port — provenance
// --------------------------
// Upstream project : FLEXPART (NILU) — https://github.com/flexpart/flexpart
// Upstream version : 10.4 (2019-11-12), commit 3d7eebf
// Upstream source  : src/initial_cond_calc.f90 (subroutine initial_cond_calc)
// Original licence : GPL-3.0-or-later — SPDX-FileCopyrightText: FLEXPART 1998-2019
// Ported into this GPL-3.0 work; see LICENSE.flexpart and NOTICE.flexpart.

//! Sensitivity to the initial conditions in backward runs
//! (`initial_cond_calc.f90`).
//!
//! In a backward run (`ldirect = -1`) with `linit_cond > 0`, FLEXPART grids
//! each particle's mass onto `init_cond` at the moment the particle is
//! terminated (end of the run, or leaving the domain): the field it builds is
//! the sensitivity of the receptor to the concentration at the start of the
//! backward period. The gridding is `conccalc`'s uniform kernel without its
//! age threshold.
//!
//! # The arithmetic, as upstream
//!
//! * `linit_cond = 1` ("mass unit"): `rhoi` is the air density at the
//!   particle, bilinear in the horizontal and linear in height between the two
//!   model levels bracketing `ztra1`, all four corners from memory slot
//!   `memind(2)` (no time interpolation: "accurate enough"). The particle
//!   contributes `xmass1/rhoi`.
//! * `linit_cond = 2` ("mass mixing ratio unit"): `rhoi = 1`.
//! * The output level is the first with `outheight(kz) > ztra1`; a particle
//!   at or above the top level contributes nothing.
//! * Direct attribution (whole mass to the particle's cell) within half a
//!   cell of the grid edge (`xl < 0.5`, `yl < 0.5`, `xl > numxgrid - 1.5`,
//!   `yl > numygrid - 1.5`); otherwise the four-cell uniform kernel with
//!   weights `wx = 1.5 - ddx` / `0.5 + ddx` (and the same in y), each corner
//!   added only if it is on the grid. The contribution is
//!   `xmass1/rhoi*w`, i.e. `(xmass1/rhoi)*w`.
//!
//! # Upstream quirks reproduced (or refused)
//!
//! * The labels look inverted but are upstream's: the **mass** unit divides
//!   by the air density, the **mixing-ratio** unit does not.
//! * The output cell is `ix = int(xl); if (xl < 0) ix = ix - 1`, a floor that
//!   is wrong at negative integers (`xl = -1` gives `-2`); only off-grid
//!   cells are affected. The density corners use plain `int(xtra1)`
//!   (truncation toward zero) and have no bounds or pole check.
//! * With `linit_cond = 1`, the level search leaves `indz` **unassigned**
//!   for `ztra1 >= height(nz)`; upstream then uses a stale or undefined
//!   value. The search runs for every particle at `itime`, even one above the
//!   output grid. The port refuses ([`InitCondError::AboveTopLevel`]).
//! * Any `linit_cond` other than 1 or 2 leaves `rhoi` undefined; it cannot be
//!   expressed through [`InitCondUnit`].
//! * A tie `ddx = 0.5` takes the `else` side (`ixp = ix - 1`, `wx = 1`).
//!   The neighbour's weight `1 - wx` is then 0 on either side of the switch,
//!   so the tie cannot be seen in the result (a mutation moving it is an
//!   equivalent mutant); it is reproduced for fidelity only.
//! * The kernel branch tests the `ix` column before the `ixp` column and adds
//!   the corners in the order `(ix,jy)`, `(ix,jyp)`, `(ixp,jyp)`, `(ixp,jy)`.
//!   They are distinct cells, so the order does not change any sum.
//! * `init_cond` is default `real` (single precision as shipped); the port
//!   accumulates in `f64`.
//!
//! # Units and indices
//!
//! Positions in meteorological grid units, heights m, masses kg (or
//! whatever unit the release carries), density kg/m³. `init_cond` is a
//! [`ConcentrationGrid`] with one uncertainty class and one age class:
//! upstream's `init_cond(0:numxgrid-1, 0:numygrid-1, numzgrid, maxspec,
//! maxpointspec_act)`; all indices 0-based.

use crate::flexpart::concentration::{ConcentrationGrid, OutputDomain};
use crate::flexpart::particle_average::{Corners, GriddedMet, Vertical};

/// `linit_cond`: the unit of the initial-condition sensitivity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitCondUnit {
    /// `linit_cond = 1`: mass divided by the air density at the particle.
    Mass,
    /// `linit_cond = 2`: mass, `rhoi = 1`.
    MassMixingRatio,
}

/// The `com_mod` switches `initial_cond_calc` reads.
#[derive(Debug, Clone, PartialEq)]
pub struct InitCondSettings {
    /// `linit_cond`.
    pub unit: InitCondUnit,
    /// `ioutputforeachrelease = 1`.
    pub output_for_each_release: bool,
    /// `mdomainfill = 1` (all mass to release slot 0).
    pub domain_filling: bool,
    /// Output level tops, m (`outheight`).
    pub outheight: Vec<f64>,
    /// Meteorological grid spacing in x, degrees (`dx`).
    pub dx: f64,
    /// Meteorological grid spacing in y, degrees (`dy`).
    pub dy: f64,
}

/// One particle as `initial_cond_calc` reads it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InitCondParticle {
    /// Time the particle is at, s (`itra1`).
    pub itra1: i64,
    /// Position, grid units (`xtra1`).
    pub xtra1: f64,
    /// Position, grid units (`ytra1`).
    pub ytra1: f64,
    /// Height above ground, m (`ztra1`).
    pub ztra1: f64,
    /// Release point, 0-based (`npoint - 1`).
    pub npoint: usize,
}

/// Why [`initial_cond_calc`] refused. Nothing has been modified then.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitCondError {
    /// [`InitCondUnit::Mass`] but no density field supplied.
    MissingDensity,
    /// `ztra1 >= height(nz)` with [`InitCondUnit::Mass`]: upstream's level
    /// index is unassigned.
    AboveTopLevel,
    /// A density corner falls outside the supplied arrays.
    DensityOutsideGrid,
    /// Release slot, level or species count outside `init_cond`.
    IndexOutsideGrid,
}

/// `initial_cond_calc.f90`: add one particle, with mass `xmass` per species
/// (`xmass1(i, 1:nspec)`), to `init_cond` if it is at time `itime`
/// (otherwise do nothing, as upstream returns at once).
///
/// `density` is required for [`InitCondUnit::Mass`]; its `rho`, `height`,
/// `nz` and `memind[1]` (upstream's `memind(2)`) are used.
///
/// # Errors
///
/// See [`InitCondError`].
// `x <= n - 1` mirrors upstream's `x.le.numxgrid-1`.
#[allow(clippy::int_plus_one)]
pub fn initial_cond_calc(
    itime: i64,
    particle: &InitCondParticle,
    xmass: &[f64],
    set: &InitCondSettings,
    density: Option<&GriddedMet>,
    domain: &OutputDomain,
    init_cond: &mut ConcentrationGrid,
) -> Result<(), InitCondError> {
    let p = particle;
    if p.itra1 != itime {
        return Ok(());
    }

    let rhoi = match set.unit {
        InitCondUnit::Mass => {
            let met = density.ok_or(InitCondError::MissingDensity)?;
            let v = Vertical::new(met, p.ztra1).ok_or(InitCondError::AboveTopLevel)?;
            // No pole clamp in this routine.
            let c = Corners::new(met, p.xtra1, p.ytra1, false)
                .ok_or(InitCondError::DensityOutsideGrid)?;
            let mind2 = met.memind[1];
            let mut rhoprof = [0.0; 2];
            for (n, ind) in [v.indz, v.indz + 1].into_iter().enumerate() {
                rhoprof[n] = c.bilinear(|i, j| met.rho[met.idx3(i, j, ind, mind2)]);
            }
            (v.dz1 * rhoprof[1] + v.dz2 * rhoprof[0]) * v.dz
        }
        InitCondUnit::MassMixingRatio => 1.0,
    };

    let nrel = if !set.output_for_each_release || set.domain_filling {
        0
    } else {
        p.npoint
    };

    let Some(kz) = set.outheight.iter().position(|&h| h > p.ztra1) else {
        return Ok(()); // above the output domain
    };
    let nspec = xmass.len();
    if nrel >= init_cond.npointspec || kz >= init_cond.nz || nspec > init_cond.nspec {
        return Err(InitCondError::IndexOutsideGrid);
    }

    let xl = (p.xtra1 * set.dx + domain.xoutshift) / domain.dxout;
    let yl = (p.ytra1 * set.dy + domain.youtshift) / domain.dyout;
    let mut ix = xl.trunc() as i64;
    if xl < 0.0 {
        ix -= 1;
    }
    let mut jy = yl.trunc() as i64;
    if yl < 0.0 {
        jy -= 1;
    }
    let (nxg, nyg) = (domain.numxgrid as i64, domain.numygrid as i64);
    let mut add = |gx: i64, gy: i64, w: Option<f64>| {
        for (ks, &m) in xmass.iter().enumerate() {
            let k = init_cond.index(gx as usize, gy as usize, kz, ks, nrel, 0, 0);
            init_cond.values[k] += match w {
                Some(w) => m / rhoi * w,
                None => m / rhoi,
            };
        }
    };

    if xl < 0.5 || yl < 0.5 || xl > (nxg - 1) as f64 - 0.5 || yl > (nyg - 1) as f64 - 0.5 {
        // Direct attribution.
        if ix >= 0 && jy >= 0 && ix <= nxg - 1 && jy <= nyg - 1 {
            add(ix, jy, None);
        }
        return Ok(());
    }

    let ddx = xl - ix as f64;
    let ddy = yl - jy as f64;
    let (ixp, wx) = if ddx > 0.5 {
        (ix + 1, 1.5 - ddx)
    } else {
        (ix - 1, 0.5 + ddx)
    };
    let (jyp, wy) = if ddy > 0.5 {
        (jy + 1, 1.5 - ddy)
    } else {
        (jy - 1, 0.5 + ddy)
    };
    let yin = |y: i64| y >= 0 && y <= nyg - 1;

    if ix >= 0 && ix <= nxg - 1 {
        if yin(jy) {
            add(ix, jy, Some(wx * wy));
        }
        if yin(jyp) {
            add(ix, jyp, Some(wx * (1.0 - wy)));
        }
    }
    if ixp >= 0 && ixp <= nxg - 1 {
        if yin(jyp) {
            add(ixp, jyp, Some((1.0 - wx) * (1.0 - wy)));
        }
        if yin(jy) {
            add(ixp, jy, Some((1.0 - wx) * wy));
        }
    }
    Ok(())
}
