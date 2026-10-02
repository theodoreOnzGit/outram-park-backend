// SPDX-License-Identifier: GPL-3.0
//
// FLEXPART port — provenance
// --------------------------
// Upstream project : FLEXPART (NILU) — https://github.com/flexpart/flexpart
// Upstream version : 10.4 (2019-11-12), commit 3d7eebf
// Upstream source  : src/calcfluxes.f90 (subroutine calcfluxes),
//                    src/fluxoutput.f90 (subroutine fluxoutput, numerics only)
// Original licence : GPL-3.0-or-later — SPDX-FileCopyrightText: FLEXPART 1998-2019
// Ported into this GPL-3.0 work; see LICENSE.flexpart and NOTICE.flexpart.

//! Gross mass fluxes through the output grid (`calcfluxes.f90`) and their
//! conversion to flux densities for output (`fluxoutput.f90`).
//!
//! # What is counted
//!
//! `calcfluxes` is called once per particle per time step (`iflux = 1`) with
//! the particle's position before (`xold`, `yold`, `zold`) and after the
//! step. It adds the particle's mass, per species, to six **gross** flux
//! accumulators, `flux(1:6, ix, jy, kz, species, release, age class)`:
//!
//! | upstream | [`FluxGrid`] index | counts a crossing of | located at |
//! |---|---|---|---|
//! | `flux(1)` | [`WEST_TO_EAST`] | `xl = ix + 0.5` moving east | row `jyave`, level `kzave` |
//! | `flux(2)` | [`EAST_TO_WEST`] | `xl = ix + 0.5` moving west | row `jyave`, level `kzave` |
//! | `flux(3)` | [`SOUTH_TO_NORTH`] | `yl = jy + 0.5` moving north | column `ixave`, level `kzave` |
//! | `flux(4)` | [`NORTH_TO_SOUTH`] | `yl = jy + 0.5` moving south | column `ixave`, level `kzave` |
//! | `flux(5)` | [`UPWARD`] | `outheighthalf(kz+1)` moving up | cell `(ixave, jyave)` |
//! | `flux(6)` | [`DOWNWARD`] | `outheighthalf(kz+1)` moving down | cell `(ixave, jyave)` |
//!
//! where `xl`, `yl` are positions in output-cell units and `ixave`, `jyave`,
//! `kzave` the cell of the **mean** horizontal position and of the **new**
//! height. The crossing indices come from `int(xl + 0.5)`, so the flux
//! "faces" are the **centre lines** of the output cells (and the vertical
//! faces the mid-levels `outheighthalf`), half a cell from the concentration
//! cells' faces. That is upstream's convention and the port keeps it.
//!
//! # Upstream quirks and defects reproduced
//!
//! * **Defect: the cyclic-boundary flux is never recorded.** For a particle
//!   that wraps around a global domain (`|xold - xtra1| >= nx/2`), upstream
//!   attributes the zonal flux to the column
//!   `ixs = int(((real(nxmin1) - 1.e5)*dx + xoutshift)/dxout)`. `1.e5` is
//!   almost certainly meant to be `1.e-5` ("just inside the eastern edge"):
//!   as written, `ixs` is about `-1e5*dx/dxout`, always negative for any
//!   realistic `xoutshift`, so the range check fails and the crossing is
//!   dropped. The code-to-code fixture has wrapping particles in both
//!   directions; upstream records nothing for them and nor does the port.
//! * `int()` truncates toward zero, so the mean-position cell `ixave` is 0 for
//!   `-1 < xl < 0` (no floor correction, unlike `conccalc`), and
//!   `int(xl + 0.5)` is 0 for `-1.5 < xl < 0.5`.
//! * The **vertical** fluxes are counted whenever the mean horizontal
//!   position is on the grid, even for a particle above the top output level
//!   (`kzave > numzgrid`); its crossing levels are clamped to `numzgrid`.
//!   The **horizontal** fluxes need `kzave <= numzgrid`.
//! * `xold`, `yold`, `zold` are default `real` upstream, `xtra1`, `ytra1`
//!   `real(kind=dp)`, so in the shipped build `xmean` and the old-position
//!   indices are single precision and the new-position indices double. The
//!   port computes everything in `f64`.
//! * The `flux` array is default `real` (single precision as shipped); the
//!   port accumulates in `f64`.
//!
//! # `fluxoutput`
//!
//! Upstream writes the six fields to the unformatted file
//! `grid_flux_YYYYMMDDHHMMSS` as `1e12 * flux / wall area / outstep`
//! (ng m⁻² s⁻¹ for masses in kg), choosing per species and age class between
//! a sparse list and a dense dump, then resets the six fields. The port
//! ([`fluxoutput`]) returns exactly the records upstream writes, in its
//! order, and resets the grid; writing the file is left to the caller.
//! Quirks reproduced:
//!
//! * The sparse/dense choice is made per (species, age class) from a count
//!   of positive cells **summed over all release points**, then applied to
//!   each release point; with several release points a block can be written
//!   dense although it has few positive cells.
//! * The record for `flux(2)` (east-to-west) is written **first**, under
//!   upstream's "east" counter `ncellse`, then `flux(1)`, `flux(3)`,
//!   `flux(4)`, `flux(5)`, `flux(6)`.
//! * The sparse index is `ix + jy*numxgrid + kz*numxgrid*numygrid` with the
//!   **1-based** level `kz` (the same convention as `concoutput`); the port
//!   reports it unchanged.
//! * A sparse record lists cells with flux `> 0` only; a dense record lists
//!   every value, ordered level, column, then row fastest.
//!
//! # Units and indices
//!
//! Positions in meteorological grid units (`xtra1`, `ytra1`), heights m above
//! ground; `dx`, `dy`, `dxout`, `dyout`, `xoutshift`, `youtshift` in degrees;
//! masses kg; `outstep` s; wall areas m². Columns, rows, levels, species,
//! release slots and age classes are **0-based** in [`FluxGrid`].

use crate::flexpart::calendar::caldate;
use crate::flexpart::concentration::OutputDomain;
use crate::flexpart::outgrid::CellGeometry;

/// `flux(1)`: west-to-east crossings.
pub const WEST_TO_EAST: usize = 0;
/// `flux(2)`: east-to-west crossings.
pub const EAST_TO_WEST: usize = 1;
/// `flux(3)`: south-to-north crossings.
pub const SOUTH_TO_NORTH: usize = 2;
/// `flux(4)`: north-to-south crossings.
pub const NORTH_TO_SOUTH: usize = 3;
/// `flux(5)`: upward crossings.
pub const UPWARD: usize = 4;
/// `flux(6)`: downward crossings.
pub const DOWNWARD: usize = 5;

/// The gross-flux accumulator `flux(6, 0:numxgrid-1, 0:numygrid-1,
/// numzgrid, nspec, maxpointspec_act, nageclass)`, kg.
#[derive(Debug, Clone, PartialEq)]
pub struct FluxGrid {
    /// Columns.
    pub nx: usize,
    /// Rows.
    pub ny: usize,
    /// Levels.
    pub nz: usize,
    /// Species.
    pub nspec: usize,
    /// Release points with separate output (`maxpointspec_act`).
    pub npointspec: usize,
    /// Age classes.
    pub nageclass: usize,
    /// Values, direction fastest, then `ix`, `jy`, `kz`, species, release,
    /// age class.
    pub values: Vec<f64>,
}

impl FluxGrid {
    /// All-zero grid (all six directions; see the `outgrid` module doc on
    /// upstream zeroing only five).
    #[must_use]
    pub fn zeros(
        nx: usize,
        ny: usize,
        nz: usize,
        nspec: usize,
        npointspec: usize,
        nageclass: usize,
    ) -> Self {
        Self {
            nx,
            ny,
            nz,
            nspec,
            npointspec,
            nageclass,
            values: vec![0.0; 6 * nx * ny * nz * nspec * npointspec * nageclass],
        }
    }

    /// Flat index; all indices 0-based, `dir` one of [`WEST_TO_EAST`] ...
    /// [`DOWNWARD`].
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn index(
        &self,
        dir: usize,
        ix: usize,
        jy: usize,
        kz: usize,
        ks: usize,
        kp: usize,
        na: usize,
    ) -> usize {
        dir + 6
            * (ix
                + self.nx
                    * (jy
                        + self.ny
                            * (kz + self.nz * (ks + self.nspec * (kp + self.npointspec * na)))))
    }
}

/// The `com_mod` values `calcfluxes` reads besides the particle.
#[derive(Debug, Clone, PartialEq)]
pub struct FluxSettings {
    /// `ioutputforeachrelease = 1`.
    pub output_for_each_release: bool,
    /// `mdomainfill = 1` (all mass to release slot 0).
    pub domain_filling: bool,
    /// Meteorological grid spacing in x, degrees (`dx`).
    pub dx: f64,
    /// Meteorological grid spacing in y, degrees (`dy`).
    pub dy: f64,
    /// Meteorological grid points in x (`nx`; `nxmin1 = nx - 1`).
    pub nx: i64,
    /// Output level tops, m (`outheight`).
    pub outheight: Vec<f64>,
    /// Output mid-levels, m (`outheighthalf`).
    pub outheighthalf: Vec<f64>,
}

/// One particle's step, as `calcfluxes` sees it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParticleStep {
    /// Position before the step, grid units / m (`xold`, `yold`, `zold`).
    pub old: [f64; 3],
    /// Position after the step, grid units / m (`xtra1`, `ytra1`, `ztra1`).
    pub new: [f64; 3],
    /// Release point, 0-based (`npoint - 1`).
    pub npoint: usize,
    /// Age class, 0-based (`nage - 1`).
    pub nage: usize,
}

/// Why [`calcfluxes`] refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FluxError {
    /// Release slot, age class or species count outside the flux grid
    /// (upstream writes out of bounds).
    IndexOutsideGrid,
}

/// First 1-based `kz` with `levels(kz) > z`, or `levels.len() + 1` (the
/// Fortran loop variable on exit).
fn level_above(levels: &[f64], z: f64) -> usize {
    levels.iter().position(|&h| h > z).map_or(levels.len() + 1, |k| k + 1)
}

/// `calcfluxes.f90`: add one particle step's mass `xmass` (kg per species,
/// `xmass1(jpart, 1:nspec)`) to the gross fluxes in `flux`.
///
/// # Errors
///
/// [`FluxError::IndexOutsideGrid`] before anything is modified.
pub fn calcfluxes(
    step: &ParticleStep,
    xmass: &[f64],
    set: &FluxSettings,
    dom: &OutputDomain,
    flux: &mut FluxGrid,
) -> Result<(), FluxError> {
    let nspec = xmass.len();
    let kp = if set.output_for_each_release && !set.domain_filling {
        step.npoint
    } else {
        0
    };
    if kp >= flux.npointspec || step.nage >= flux.nageclass || nspec > flux.nspec {
        return Err(FluxError::IndexOutsideGrid);
    }
    let nage = step.nage;
    let [xold, yold, zold] = step.old;
    let [xnew, ynew, znew] = step.new;
    let (nxg, nyg, nzg) = (dom.numxgrid as i64, dom.numygrid as i64, flux.nz);

    let xmean = (xold + xnew) / 2.0;
    let ymean = (yold + ynew) / 2.0;
    // Fortran int(): truncation toward zero.
    let ixave = ((xmean * set.dx + dom.xoutshift) / dom.dxout).trunc() as i64;
    let jyave = ((ymean * set.dy + dom.youtshift) / dom.dyout).trunc() as i64;
    let kzave = level_above(&set.outheight, znew); // 1-based, nzg+1 above

    // Vertical fluxes.
    if ixave >= 0 && jyave >= 0 && ixave <= nxg - 1 && jyave <= nyg - 1 {
        let k1 = nzg.min(level_above(&set.outheighthalf, zold));
        let k2 = nzg.min(level_above(&set.outheighthalf, znew));
        for k in 0..nspec {
            for kz in k1..k2 {
                let i = flux.index(UPWARD, ixave as usize, jyave as usize, kz - 1, k, kp, nage);
                flux.values[i] += xmass[k];
            }
            for kz in k2..k1 {
                let i = flux.index(DOWNWARD, ixave as usize, jyave as usize, kz - 1, k, kp, nage);
                flux.values[i] += xmass[k];
            }
        }
    }

    let mut add = |dir: usize, ix: i64, jy: i64, kz1: usize| {
        for (k, &m) in xmass.iter().enumerate() {
            let i = flux.index(dir, ix as usize, jy as usize, kz1 - 1, k, kp, nage);
            flux.values[i] += m;
        }
    };

    // Zonal fluxes.
    if kzave <= nzg && jyave >= 0 && jyave <= nyg - 1 {
        if (xold - xnew).abs() < set.nx as f64 / 2.0 {
            let ix1 = ((xold * set.dx + dom.xoutshift) / dom.dxout + 0.5).trunc() as i64;
            let ix2 = ((xnew * set.dx + dom.xoutshift) / dom.dxout + 0.5).trunc() as i64;
            // Upstream loops species outermost; the cells touched by one
            // species do not interact with another's, so the order is moot.
            for ix in ix1..ix2 {
                if ix >= 0 && ix <= nxg - 1 {
                    add(WEST_TO_EAST, ix, jyave, kzave);
                }
            }
            for ix in ix2..ix1 {
                if ix >= 0 && ix <= nxg - 1 {
                    add(EAST_TO_WEST, ix, jyave, kzave);
                }
            }
        } else {
            // Upstream's `1.e5` (see the module doc): ixs is never on the grid
            // in practice, but the arithmetic is reproduced as written.
            let nxmin1 = (set.nx - 1) as f64;
            let ixs = (((nxmin1 - 1.0e5) * set.dx + dom.xoutshift) / dom.dxout).trunc() as i64;
            if ixs >= 0 && ixs <= nxg - 1 {
                if xold > xnew {
                    add(WEST_TO_EAST, ixs, jyave, kzave);
                } else {
                    add(EAST_TO_WEST, ixs, jyave, kzave);
                }
            }
        }
    }

    // Meridional fluxes.
    if kzave <= nzg && ixave >= 0 && ixave <= nxg - 1 {
        let jy1 = ((yold * set.dy + dom.youtshift) / dom.dyout + 0.5).trunc() as i64;
        let jy2 = ((ynew * set.dy + dom.youtshift) / dom.dyout + 0.5).trunc() as i64;
        for jy in jy1..jy2 {
            if jy >= 0 && jy <= nyg - 1 {
                add(SOUTH_TO_NORTH, ixave, jy, kzave);
            }
        }
        for jy in jy2..jy1 {
            if jy >= 0 && jy <= nyg - 1 {
                add(NORTH_TO_SOUTH, ixave, jy, kzave);
            }
        }
    }
    Ok(())
}

/// How one block of [`fluxoutput`] is written.
#[derive(Debug, Clone, PartialEq)]
pub enum FluxLayout {
    /// Record `1`: `(index, value)` for every cell with flux `> 0`, index
    /// `ix + jy*numxgrid + kz*numxgrid*numygrid` with 1-based `kz`; the
    /// terminator `-999, 999.` is not included.
    Sparse(Vec<(i64, f64)>),
    /// Record `2`: every value, level outermost, then column, row fastest.
    Dense(Vec<f64>),
}

/// One block of the flux file: one direction of one species, release slot and
/// age class (0-based).
#[derive(Debug, Clone, PartialEq)]
pub struct FluxBlock {
    /// Species.
    pub ks: usize,
    /// Release slot.
    pub kp: usize,
    /// Age class.
    pub nage: usize,
    /// Direction ([`WEST_TO_EAST`] ... [`DOWNWARD`]).
    pub dir: usize,
    /// The values, `1e12 * flux / area / outstep`.
    pub layout: FluxLayout,
}

/// Everything `fluxoutput` writes.
#[derive(Debug, Clone, PartialEq)]
pub struct FluxOutput {
    /// `YYYYMMDD` of the file name.
    pub date: i64,
    /// `HHMMSS` of the file name.
    pub time: i64,
    /// The time written as the first record, s.
    pub itime: i64,
    /// The blocks, in file order.
    pub blocks: Vec<FluxBlock>,
}

impl FluxOutput {
    /// The file name upstream opens under `path(2)`:
    /// `grid_flux_YYYYMMDDHHMMSS`.
    #[must_use]
    pub fn file_name(&self) -> String {
        format!("grid_flux_{:08}{:06}", self.date, self.time)
    }
}

/// The order upstream writes the six directions in.
const OUTPUT_ORDER: [usize; 6] = [
    EAST_TO_WEST,
    WEST_TO_EAST,
    SOUTH_TO_NORTH,
    NORTH_TO_SOUTH,
    UPWARD,
    DOWNWARD,
];

/// `fluxoutput.f90`: the records of the flux file at time `itime` (s after
/// the start, Julian date `bdate`), with output interval `outstep` (s,
/// `real(abs(loutstep))`), using the wall areas of `geometry` (the mother
/// output grid). Resets all six directions of `flux` to zero afterwards, as
/// upstream does.
///
/// # Panics
///
/// If `geometry`'s dimensions differ from `flux`'s.
#[must_use]
pub fn fluxoutput(
    itime: i64,
    bdate: f64,
    outstep: f64,
    geometry: &CellGeometry,
    flux: &mut FluxGrid,
) -> FluxOutput {
    assert!(
        geometry.numxgrid == flux.nx && geometry.numygrid == flux.ny && geometry.numzgrid == flux.nz,
        "flux grid and cell geometry differ in size"
    );
    let jul = bdate + itime as f64 / 86400.0;
    let (date, time) = caldate(jul);
    let (nx, ny, nz) = (flux.nx, flux.ny, flux.nz);

    // Sparse or dense, per species and age class, counting over all releases.
    let mut sparse = vec![[false; 6]; flux.nspec * flux.nageclass];
    for k in 0..flux.nspec {
        for nage in 0..flux.nageclass {
            for (d, s) in sparse[k * flux.nageclass + nage].iter_mut().enumerate() {
                let mut ncells = 0usize;
                for kp in 0..flux.npointspec {
                    for jy in 0..ny {
                        for ix in 0..nx {
                            for kz in 0..nz {
                                if flux.values[flux.index(d, ix, jy, kz, k, kp, nage)] > 0.0 {
                                    ncells += 1;
                                }
                            }
                        }
                    }
                }
                *s = 4 * ncells < nx * ny * nz;
            }
        }
    }

    let wall = |d: usize, ix: usize, jy: usize, kz: usize| -> f64 {
        match d {
            WEST_TO_EAST | EAST_TO_WEST => geometry.areaeast[geometry.idx3(ix, jy, kz)],
            SOUTH_TO_NORTH | NORTH_TO_SOUTH => geometry.areanorth[geometry.idx3(ix, jy, kz)],
            _ => geometry.area[geometry.idx2(ix, jy)],
        }
    };

    let mut blocks = Vec::new();
    for k in 0..flux.nspec {
        for kp in 0..flux.npointspec {
            for nage in 0..flux.nageclass {
                for &d in &OUTPUT_ORDER {
                    let value = |ix: usize, jy: usize, kz: usize| {
                        1.0e12 * flux.values[flux.index(d, ix, jy, kz, k, kp, nage)]
                            / wall(d, ix, jy, kz)
                            / outstep
                    };
                    let layout = if sparse[k * flux.nageclass + nage][d] {
                        let mut v = Vec::new();
                        for kz in 0..nz {
                            for jy in 0..ny {
                                for ix in 0..nx {
                                    if flux.values[flux.index(d, ix, jy, kz, k, kp, nage)] > 0.0 {
                                        let idx = ix + jy * nx + (kz + 1) * nx * ny;
                                        v.push((idx as i64, value(ix, jy, kz)));
                                    }
                                }
                            }
                        }
                        FluxLayout::Sparse(v)
                    } else {
                        let mut v = Vec::with_capacity(nx * ny * nz);
                        for kz in 0..nz {
                            for ix in 0..nx {
                                for jy in 0..ny {
                                    v.push(value(ix, jy, kz));
                                }
                            }
                        }
                        FluxLayout::Dense(v)
                    };
                    blocks.push(FluxBlock {
                        ks: k,
                        kp,
                        nage,
                        dir: d,
                        layout,
                    });
                }
            }
        }
    }

    flux.values.iter_mut().for_each(|v| *v = 0.0);
    FluxOutput {
        date,
        time,
        itime,
        blocks,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (FluxSettings, OutputDomain, FluxGrid) {
        let set = FluxSettings {
            output_for_each_release: false,
            domain_filling: false,
            dx: 1.0,
            dy: 1.0,
            nx: 20,
            outheight: vec![100.0, 500.0],
            outheighthalf: vec![50.0, 300.0],
        };
        let dom = OutputDomain {
            numxgrid: 8,
            numygrid: 8,
            dxout: 1.0,
            dyout: 1.0,
            xoutshift: 0.0,
            youtshift: 0.0,
        };
        let flux = FluxGrid::zeros(8, 8, 2, 1, 1, 1);
        (set, dom, flux)
    }

    /// A wrapping particle records nothing (upstream's `1.e5`).
    #[test]
    fn wrap_around_flux_is_dropped() {
        let (set, dom, mut flux) = setup();
        let step = ParticleStep {
            old: [19.0, 3.0, 20.0],
            new: [1.0, 3.0, 20.0],
            npoint: 0,
            nage: 0,
        };
        calcfluxes(&step, &[1.0], &set, &dom, &mut flux).unwrap();
        assert!(flux.values.iter().all(|&v| v == 0.0));
    }

    /// Crossing three cell centre lines eastward adds the mass three times.
    #[test]
    fn eastward_crossings() {
        let (set, dom, mut flux) = setup();
        let step = ParticleStep {
            old: [1.25, 3.0, 20.0],
            new: [3.75, 3.0, 20.0],
            npoint: 0,
            nage: 0,
        };
        calcfluxes(&step, &[2.0], &set, &dom, &mut flux).unwrap();
        let total: f64 = flux.values.iter().sum();
        // ix1 = int(1.75) = 1, ix2 = int(4.25) = 4: columns 1, 2, 3.
        assert_eq!(total, 6.0);
        for ix in 1..4 {
            assert_eq!(flux.values[flux.index(WEST_TO_EAST, ix, 3, 0, 0, 0, 0)], 2.0);
        }
    }
}
