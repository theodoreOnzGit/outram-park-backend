// SPDX-License-Identifier: GPL-3.0
//
// FLEXPART port — provenance
// --------------------------
// Upstream project : FLEXPART (NILU) — https://github.com/flexpart/flexpart
// Upstream version : 10.4 (2019-11-12), commit 3d7eebf
// Upstream source  : src/init_domainfill.f90, src/boundcond_domainfill.f90
//                    (read com_mod.f90 and point_mod.f90)
// Original licence : GPL-3.0-or-later — SPDX-FileCopyrightText: FLEXPART 1998-2019
// Ported into this GPL-3.0 work; see LICENSE.flexpart and NOTICE.flexpart.

//! Domain-filling mode (`mdomainfill = 1` air, `2` stratospheric ozone):
//! `init_domainfill.f90` fills the first release box with particles carrying
//! equal shares of the air mass in it, and `boundcond_domainfill.f90`
//! injects particles at the box's boundaries as air flows in.
//!
//! # [`init_domainfill`]
//!
//! The box is the first release point's, widened to whole grid cells
//! (`nx_we`, `ny_sn`). Each column's air mass is `(p(1) - p(nz)) / g` times
//! the cell area, with `p = rho R T` from memory slot **1**. Column `(ix, jy)`
//! receives `nint(0.999 npart colmass / total)` particles, placed at equal
//! pressure intervals when there are more than 20 and at random pressures
//! otherwise. For `mdomainfill = 2` a particle is kept only above 3000 m with
//! potential vorticity above `pvcrit`, and its mass becomes an ozone mass.
//!
//! A second pass fixes, for each boundary column, a smaller set of release
//! heights (`zcolumn_we`, `zcolumn_sn`) and their number (`numcolumn_we`,
//! `numcolumn_sn`) for [`boundcond_domainfill`].
//!
//! # [`boundcond_domainfill`]
//!
//! Terminates particles at the current time that have left the box, then at
//! every boundary release height accumulates the inflowing air mass
//! `u rho A lsynctime` (time- and height-interpolated) and releases one
//! particle per `xmassperparticle` accumulated (rounded). Outflow resets the
//! accumulator to zero.
//!
//! # Random numbers are an INPUT
//!
//! Upstream draws from `ran1` (Numerical Recipes, not ported and not
//! re-implemented: gh:#410). The port takes the draws from an iterator. The
//! order is upstream's: in `init_domainfill`, per particle, the random
//! pressure (columns of 20 or fewer only), then x, an extra x draw in the
//! first and in the last grid column, y, and the uncertainty class (only for
//! a kept particle); in `boundcond_domainfill`, the position along the
//! boundary, the height (interior release heights only), then the class.
//!
//! # Upstream quirks and defects reproduced
//!
//! - **`acc_mass_sn` is zeroed at the wrong index.** `init_domainfill` writes
//!   `acc_mass_sn(1,jy,j) = 0` and `acc_mass_sn(2,jy,j) = 0` — indexed by the
//!   *latitude* index `jy`, although the array's second index is the
//!   longitude `ix` (`boundcond_domainfill` reads `acc_mass_sn(k,ix,j)`). The
//!   southern/northern accumulators of the boundary columns are therefore
//!   zeroed only where some `jy` of the box equals that `ix`. The western/
//!   eastern accumulators are zeroed for every column, not only the boundary
//!   ones. Both are harmless in a fresh run (the static arrays start at zero)
//!   and wrong after a restart into a different box. The port writes the same
//!   addresses, using upstream's column-major layout (so an index past
//!   `nxmax` aliases exactly as upstream's would).
//! - **Boundary heights and counts are only written for non-empty columns**:
//!   a column whose second-pass count rounds to 0 keeps the previous
//!   `numcolumn_*` and `zcolumn_*`.
//! - **`deltaz` of the last release height reads `zcolumn(j - 2)`**. With
//!   exactly two release heights in a boundary column that is
//!   `zcolumn(k, i, 0)`, outside the array; the port refuses
//!   ([`DomainFillError::UndefinedRead`]) before changing anything. With one
//!   release height, `deltaz` and the height read `zcolumn(k, i, 2)`, which
//!   `init_domainfill` never wrote for that column (zero in a fresh run,
//!   otherwise stale); the port reads the same element.
//! - **A rejected ozone particle still overwrites its slot.** In
//!   `boundcond_domainfill`, when the PV test fails, the vacant slot has
//!   already received x, y and z; it stays vacant (`itra1` unchanged) with
//!   those positions, and the next particle searches from the same slot.
//! - **`ylat` of the PV sign**: `init_domainfill` uses the column's latitude;
//!   on the western/eastern boundaries `boundcond_domainfill` uses the
//!   particle's, on the southern/northern ones the boundary's.
//! - **Reads past the filled grid** with a nonzero weight: in the last row of
//!   a box that ends at `nymin1`, a particle at `ytra1 > nymin1` interpolates
//!   `pv` from row `ny`. For the static `pv` that is the unused tail of the
//!   array (zero) when `ny < nymax`. The port reproduces the address (see
//!   [`StaticExtents`]); such a particle is then marked out of the domain.
//! - **Bilinear weights may be negative**: a particle at `ytra1 < 0` in the
//!   first row keeps `jym = int(ytra1) = 0`, so PV is extrapolated.
//! - **`hzone = 1/dyconst`** (the arc length) on the equatorial row instead of
//!   the zone height; polar rows of a global grid take a half-cell cap.
//! - **`numparttot` sums the planned column counts**, not the particles kept,
//!   so with `mdomainfill = 2` `xmassperparticle` is the air mass per
//!   *planned* particle.
//! - **`gdomainfill` is assigned only for global wind fields** (`xglobal`,
//!   `sglobal` and `nglobal`); otherwise the input value is kept.
//! - **`numactiveparticles`, `xm`, `accmasst`** are computed upstream and
//!   never used (their `write` is commented out). They are not ported.
//!
//! # Not ported: file I/O
//!
//! With `ipin = 1` (restart) and a non-global box, `init_domainfill` reads
//! `boundcond.bin`, overriding every boundary array; with `ipout > 0` at
//! `itime == loutend`, `boundcond_domainfill` writes it. The port does not do
//! file I/O: it reports [`InitDomainFillOutcome::restart_read_required`] and
//! [`BoundcondOutcome::dump_required`], and the caller loads or saves
//! [`DomainFillState`].
//!
//! # Refusals
//!
//! Upstream's `stop`s, and its writes past `maxpart` (it warns *after* having
//! written out of bounds), become [`DomainFillError`]s. Except where noted,
//! the state is then partly updated and should be treated as invalid.
//!
//! # Index conventions and units
//!
//! Grid indices are 0-based as upstream's; the boundary side `k` is 0
//! (west/south) or 1 (east/north); release heights `j` are 0-based
//! (upstream's `j - 1`); level indices 0-based. Positions in grid units,
//! heights m, masses kg, winds m/s, densities kg/m³, PV in pvu.

use super::constants::{GA, PI, PI180, R_AIR, R_EARTH};
use super::interpolation::MetFields;
use super::release::{ParticleStore, StaticExtents, ITRA_INACTIVE};

/// `mdomainfill`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DomainFillMode {
    /// `1`: air tracer, every particle kept.
    Air,
    /// `2`: stratospheric ozone tracer, particles kept only above 3000 m where
    /// `pv > pvcrit`, mass scaled by `pv * 48/29 * ozonescale / 1e9`.
    StratosphericOzone,
}

/// The grid as the domain-filling routines read it from `com_mod`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DomainFillGrid {
    /// `nx`.
    pub nx: i64,
    /// `ny`.
    pub ny: i64,
    /// Cyclic in longitude.
    pub xglobal: bool,
    /// Contains the south pole.
    pub sglobal: bool,
    /// Contains the north pole.
    pub nglobal: bool,
    /// Grid spacing, degrees.
    pub dx: f64,
    /// Grid spacing, degrees.
    pub dy: f64,
    /// Latitude of the grid origin, degrees.
    pub ylat0: f64,
    /// Metres-to-grid-units factor in y, 1/m (`dyconst = 180/(dy r_earth pi)`).
    pub dyconst: f64,
    /// Static array extents (`pv` reads past the grid).
    pub extents: StaticExtents,
}

/// Run switches the domain-filling routines read from `com_mod`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DomainFillSettings {
    /// `mdomainfill`.
    pub mode: DomainFillMode,
    /// `ipin`: `0` fresh start; non-zero resumes from a particle dump.
    pub ipin: i64,
    /// `ipout`: `> 0` dumps particles.
    pub ipout: i64,
    /// PV threshold of the stratosphere, pvu (`pvcrit`).
    pub pvcrit: f64,
    /// Ozone/PV ratio, ppb/pvu (`ozonescale`).
    pub ozonescale: f64,
    /// Number of uncertainty classes.
    pub nclassunc: i64,
    /// Minimum time step, s.
    pub mintime: i64,
    /// `+1` forward, `-1` backward.
    pub ldirect: i64,
    /// Splitting time constant, s.
    pub itsplit: i64,
    /// Synchronisation interval, s.
    pub lsynctime: i64,
}

/// The domain-filling globals of `com_mod`. The four boundary arrays are
/// stored in **upstream's column-major layout** with upstream's extents,
/// so that upstream's index arithmetic, including its index defect (see the
/// module docs), addresses the same elements.
#[derive(Debug, Clone, PartialEq)]
pub struct DomainFillState {
    /// Western and eastern boundary x index (`nx_we`).
    pub nx_we: [i64; 2],
    /// Southern and northern boundary y index (`ny_sn`).
    pub ny_sn: [i64; 2],
    /// Largest particle count of any column (`numcolumn`).
    pub numcolumn: i64,
    /// Global domain filling: no boundary conditions (`gdomainfill`).
    pub gdomainfill: bool,
    /// Air mass per particle, kg (`xmassperparticle`).
    pub xmassperparticle: f64,
    /// `nxmax` of the arrays.
    pub nxmax: usize,
    /// `nymax` of the arrays.
    pub nymax: usize,
    /// `maxcolumn`.
    pub maxcolumn: usize,
    /// `numcolumn_we(2, 0:nymax-1)`, at `k + 2*jy`.
    pub numcolumn_we: Vec<i64>,
    /// `numcolumn_sn(2, 0:nxmax-1)`, at `k + 2*ix`.
    pub numcolumn_sn: Vec<i64>,
    /// `zcolumn_we(2, 0:nymax-1, maxcolumn)`, m.
    pub zcolumn_we: Vec<f64>,
    /// `zcolumn_sn(2, 0:nxmax-1, maxcolumn)`, m.
    pub zcolumn_sn: Vec<f64>,
    /// `acc_mass_we(2, 0:nymax-1, maxcolumn)`, kg.
    pub acc_mass_we: Vec<f64>,
    /// `acc_mass_sn(2, 0:nxmax-1, maxcolumn)`, kg.
    pub acc_mass_sn: Vec<f64>,
}

/// Linear index of `(k, i, j)` (all 0-based) in a `(2, 0:n-1, maxcolumn)`
/// array, or `None` outside it.
fn lin3(n: usize, maxcolumn: usize, k: usize, i: i64, j: i64) -> Option<usize> {
    let l = k as i64 + 2 * (i + n as i64 * j);
    if l < 0 || l >= (2 * n * maxcolumn) as i64 {
        None
    } else {
        Some(l as usize)
    }
}

impl DomainFillState {
    /// Zeroed arrays (a fresh run's static memory).
    #[must_use]
    pub fn new(nxmax: usize, nymax: usize, maxcolumn: usize) -> Self {
        Self {
            nx_we: [0; 2],
            ny_sn: [0; 2],
            numcolumn: 0,
            gdomainfill: false,
            xmassperparticle: 0.0,
            nxmax,
            nymax,
            maxcolumn,
            numcolumn_we: vec![0; 2 * nymax],
            numcolumn_sn: vec![0; 2 * nxmax],
            zcolumn_we: vec![0.0; 2 * nymax * maxcolumn],
            zcolumn_sn: vec![0.0; 2 * nxmax * maxcolumn],
            acc_mass_we: vec![0.0; 2 * nymax * maxcolumn],
            acc_mass_sn: vec![0.0; 2 * nxmax * maxcolumn],
        }
    }

    /// Index into `zcolumn_we`/`acc_mass_we` of `(k, jy, j)`.
    #[must_use]
    pub fn we(&self, k: usize, jy: i64, j: i64) -> Option<usize> {
        lin3(self.nymax, self.maxcolumn, k, jy, j)
    }

    /// Index into `zcolumn_sn`/`acc_mass_sn` of `(k, ix, j)`.
    #[must_use]
    pub fn sn(&self, k: usize, ix: i64, j: i64) -> Option<usize> {
        lin3(self.nxmax, self.maxcolumn, k, ix, j)
    }

    /// `numcolumn_we(k, jy)`.
    #[must_use]
    pub fn ncol_we(&self, k: usize, jy: i64) -> i64 {
        self.numcolumn_we[k + 2 * jy as usize]
    }

    /// `numcolumn_sn(k, ix)`.
    #[must_use]
    pub fn ncol_sn(&self, k: usize, ix: i64) -> i64 {
        self.numcolumn_sn[k + 2 * ix as usize]
    }
}

/// Why a domain-filling routine refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DomainFillError {
    /// More particles than slots. In `boundcond_domainfill` upstream `stop`s;
    /// in `init_domainfill` it writes past `maxpart` and only warns.
    TooManyParticles,
    /// A boundary column needs more than `maxcolumn` release heights
    /// (upstream: `stop 'maxcolumn too small'`).
    MaxColumnTooSmall,
    /// The draw iterator ran dry.
    DrawsExhausted,
    /// Upstream would read or write outside an array (see the module docs).
    UndefinedRead,
    /// No model level lies above a height (upstream would use an undefined
    /// level index).
    UndefinedLevel,
}

fn draw<I: Iterator<Item = f64>>(draws: &mut I) -> Result<f64, DomainFillError> {
    draws.next().ok_or(DomainFillError::DrawsExhausted)
}

fn fint(x: f64) -> i64 {
    x.trunc() as i64
}

/// Fortran `nint`: round half away from zero.
fn nint(x: f64) -> i64 {
    x.round() as i64
}

/// Lower level (0-based) of the pair bracketing `z`: the first `i >= 2`
/// (1-based) with `height(i) > z`, minus one.
fn level_below(height: &[f64], nz: usize, z: f64) -> Result<usize, DomainFillError> {
    (2..=nz)
        .find(|&i| height[i - 1] > z)
        .map(|i| i - 2)
        .ok_or(DomainFillError::UndefinedLevel)
}

/// What [`init_domainfill`] reports besides the state it writes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InitDomainFillOutcome {
    /// `true` if upstream returned early (global domain filling resumed from a
    /// dump: nothing else was done).
    pub early_return: bool,
    /// Total air mass in the box, kg (`colmasstotal`).
    pub colmasstotal: f64,
    /// Upstream would now read `boundcond.bin` (`ipin == 1`, not global); the
    /// caller must load the boundary arrays of [`DomainFillState`].
    pub restart_read_required: bool,
}

/// PV at a position from memory slot `slot` (0-based), static-array reads.
#[allow(clippy::too_many_arguments)]
fn pv_bilinear(
    met: &MetFields,
    pv: &[f64],
    ext: StaticExtents,
    ixm: i64,
    jym: i64,
    k: usize,
    slot: usize,
    w: [f64; 4],
) -> Result<f64, DomainFillError> {
    let r = |a: i64, b: i64| {
        ext.read3(met, pv, a, b, k as i64, slot as i64)
            .ok_or(DomainFillError::UndefinedRead)
    };
    Ok(w[0] * r(ixm, jym)? + w[1] * r(ixm + 1, jym)? + w[2] * r(ixm, jym + 1)?
        + w[3] * r(ixm + 1, jym + 1)?)
}

/// Bilinear weights `p1..p4` and lower-left corner of a mother-grid position.
fn weights(x: f64, y: f64) -> (i64, i64, [f64; 4]) {
    let ixm = fint(x);
    let jym = fint(y);
    let ddx = x - ixm as f64;
    let ddy = y - jym as f64;
    let rddx = 1.0 - ddx;
    let rddy = 1.0 - ddy;
    (ixm, jym, [rddx * rddy, ddx * rddy, rddx * ddy, ddx * ddy])
}

/// `init_domainfill`: fill the first release box with particles.
///
/// # Arguments
/// - `xpoint1`, `xpoint2`, `ypoint1`, `ypoint2` — the first release box,
///   grid units; `npart1` — its particle count (`npart(1)`).
/// - `grid`, `settings` — the `com_mod` grid and switches.
/// - `met` — mother-grid `rho`, `tt` (memory slot **1**) and `height`.
/// - `pv` — potential vorticity, pvu, laid out like `met`'s 3-D fields.
/// - `draws` — the `ran1` sequence.
/// - `parts` — the particle arrays (`numpart` is reset to 0 when `ipin == 0`).
/// - `state` — the domain-filling globals; written.
///
/// # Errors
/// See [`DomainFillError`].
#[allow(clippy::too_many_arguments)]
pub fn init_domainfill<I: Iterator<Item = f64>>(
    xpoint1: f64,
    xpoint2: f64,
    ypoint1: f64,
    ypoint2: f64,
    npart1: i64,
    grid: &DomainFillGrid,
    settings: &DomainFillSettings,
    met: &MetFields,
    pv: &[f64],
    draws: &mut I,
    parts: &mut ParticleStore,
    state: &mut DomainFillState,
) -> Result<InitDomainFillOutcome, DomainFillError> {
    let nx = grid.nx;
    let nxmin1 = grid.nx - 1;
    let nymin1 = grid.ny - 1;
    let nz = met.nz;
    let height = &met.height;
    let ext = grid.extents;
    let pih = PI / 180.0;
    let nspec = parts.nspec;
    let maxpart = parts.maxpart();

    state.nx_we[0] = fint(xpoint1).max(0);
    state.nx_we[1] = (fint(xpoint2) + 1).min(nxmin1);
    state.ny_sn[0] = fint(ypoint1).max(0);
    state.ny_sn[1] = (fint(ypoint2) + 1).min(nymin1);

    if grid.xglobal && grid.sglobal && grid.nglobal {
        state.gdomainfill = state.nx_we[0] == 0
            && state.nx_we[1] == nxmin1
            && state.ny_sn[0] == 0
            && state.ny_sn[1] == nymin1;
    }

    if state.gdomainfill && settings.ipin != 0 {
        return Ok(InitDomainFillOutcome {
            early_return: true,
            colmasstotal: 0.0,
            restart_read_required: false,
        });
    }

    if grid.xglobal {
        state.nx_we[1] = state.nx_we[1].min(nx - 2);
    }
    let [ix0, ix1] = state.nx_we;
    let [jy0, jy1] = state.ny_sn;

    // Area of each latitude band of one cell.
    let mut gridarea = vec![0.0; ext.nymax];
    for jy in jy0..=jy1 {
        let ylat = grid.ylat0 + jy as f64 * grid.dy;
        let ylatp = ylat + 0.5 * grid.dy;
        let ylatm = ylat - 0.5 * grid.dy;
        let hzone = if ylatm < 0.0 && ylatp > 0.0 {
            1.0 / grid.dyconst
        } else {
            let cosfactp = (ylatp * pih).cos() * R_EARTH;
            let cosfactm = (ylatm * pih).cos() * R_EARTH;
            if cosfactp < cosfactm {
                (R_EARTH * R_EARTH - cosfactp * cosfactp).sqrt()
                    - (R_EARTH * R_EARTH - cosfactm * cosfactm).sqrt()
            } else {
                (R_EARTH * R_EARTH - cosfactm * cosfactm).sqrt()
                    - (R_EARTH * R_EARTH - cosfactp * cosfactp).sqrt()
            }
        };
        gridarea[jy as usize] = 2.0 * PI * R_EARTH * hzone * grid.dx / 360.0;
    }
    if grid.sglobal {
        let ylatp = grid.ylat0 + 0.5 * grid.dy;
        let cosfactm = 0.0;
        let cosfactp = (ylatp * pih).cos() * R_EARTH;
        let hzone = (R_EARTH * R_EARTH - cosfactm * cosfactm).sqrt()
            - (R_EARTH * R_EARTH - cosfactp * cosfactp).sqrt();
        gridarea[0] = 2.0 * PI * R_EARTH * hzone * grid.dx / 360.0;
    }
    if grid.nglobal {
        let ylat = grid.ylat0 + nymin1 as f64 * grid.dy;
        let ylatm = ylat - 0.5 * grid.dy;
        let cosfactp = 0.0;
        let cosfactm = (ylatm * pih).cos() * R_EARTH;
        let hzone = (R_EARTH * R_EARTH - cosfactp * cosfactp).sqrt()
            - (R_EARTH * R_EARTH - cosfactm * cosfactm).sqrt();
        gridarea[nymin1 as usize] = 2.0 * PI * R_EARTH * hzone * grid.dx / 360.0;
    }

    let rho1 = |ix: i64, jy: i64, k: usize| met.rho[met.idx3(ix as usize, jy as usize, k, 0)];
    let tt1 = |ix: i64, jy: i64, k: usize| met.tt[met.idx3(ix as usize, jy as usize, k, 0)];
    let pp_of = |ix: i64, jy: i64, k: usize| rho1(ix, jy, k) * R_AIR * tt1(ix, jy, k);

    // Mass of each column and of the box.
    let ncolx = (ix1 - ix0 + 1).max(0) as usize;
    let col = |ix: i64, jy: i64| (jy - jy0) as usize * ncolx + (ix - ix0) as usize;
    let mut colmass = vec![0.0; ncolx * ((jy1 - jy0 + 1).max(0) as usize)];
    let mut colmasstotal = 0.0;
    for jy in jy0..=jy1 {
        for ix in ix0..=ix1 {
            let pp1 = pp_of(ix, jy, 0);
            let ppnz = pp_of(ix, jy, nz - 1);
            let c = (pp1 - ppnz) / GA * gridarea[jy as usize];
            colmass[col(ix, jy)] = c;
            colmasstotal += c;
        }
    }

    if settings.ipin == 0 {
        parts.numpart = 0;
    }

    // Particle positions.
    let mut numparttot: i64 = 0;
    state.numcolumn = 0;
    let mut pp = vec![0.0; nz];
    for jy in jy0..=jy1 {
        let ylat = grid.ylat0 + jy as f64 * grid.dy;
        for ix in ix0..=ix1 {
            let ncolumn = nint(0.999 * npart1 as f64 * colmass[col(ix, jy)] / colmasstotal);
            if ncolumn == 0 {
                continue;
            }
            if ncolumn > state.numcolumn {
                state.numcolumn = ncolumn;
            }
            for (kz, p) in pp.iter_mut().enumerate() {
                *p = pp_of(ix, jy, kz);
            }
            let deltacol = (pp[0] - pp[nz - 1]) / ncolumn as f64;
            let mut pnew = pp[0] + deltacol / 2.0;
            let mut jj: i64 = 0;
            for _j in 1..=ncolumn {
                jj += 1;
                if ncolumn > 20 {
                    pnew -= deltacol;
                } else {
                    pnew = pp[0] - draw(draws)? * (pp[0] - pp[nz - 1]);
                }
                for kz in 0..nz - 1 {
                    if !(pp[kz] >= pnew && pp[kz + 1] < pnew) {
                        continue;
                    }
                    let dz1 = pp[kz] - pnew;
                    let dz2 = pnew - pp[kz + 1];
                    let dz = 1.0 / (dz1 + dz2);
                    if settings.ipin != 0 {
                        continue;
                    }
                    let slot = parts.numpart as i64 + jj - 1;
                    if slot < 0 || slot as usize >= maxpart {
                        return Err(DomainFillError::TooManyParticles);
                    }
                    let s = slot as usize;
                    let mut x = ix as f64 - 0.5 + draw(draws)?;
                    if ix == 0 {
                        x = draw(draws)?;
                    }
                    if ix == nxmin1 {
                        x = nxmin1 as f64 - draw(draws)?;
                    }
                    let y = jy as f64 - 0.5 + draw(draws)?;
                    let mut z = (height[kz] * dz2 + height[kz + 1] * dz1) * dz;
                    if z > height[nz - 1] - 0.5 {
                        z = height[nz - 1] - 0.5;
                    }
                    parts.xtra1[s] = x;
                    parts.ytra1[s] = y;
                    parts.ztra1[s] = z;

                    // PV at the particle.
                    let (ixm, jym, w) = weights(x, y);
                    let indzm = level_below(height, nz, z)?;
                    let dz1 = z - height[indzm];
                    let dz2 = height[indzm + 1] - z;
                    let dz = 1.0 / (dz1 + dz2);
                    let y0 = pv_bilinear(met, pv, ext, ixm, jym, indzm, 0, w)?;
                    let y1 = pv_bilinear(met, pv, ext, ixm, jym, indzm + 1, 0, w)?;
                    let mut pvpart = (dz2 * y0 + dz1 * y1) * dz;
                    if ylat < 0.0 {
                        pvpart = -1.0 * pvpart;
                    }

                    if (z > 3000.0 && pvpart > settings.pvcrit)
                        || settings.mode == DomainFillMode::Air
                    {
                        parts.nclass[s] = (fint(draw(draws)? * settings.nclassunc as f64) + 1)
                            .min(settings.nclassunc);
                        parts.numparticlecount += 1;
                        parts.npoint[s] = parts.numparticlecount;
                        parts.idt[s] = settings.mintime;
                        parts.itra1[s] = 0;
                        parts.itramem[s] = 0;
                        parts.itrasplit[s] = settings.ldirect * settings.itsplit;
                        let mut m = colmass[col(ix, jy)] / ncolumn as f64;
                        if settings.mode == DomainFillMode::StratosphericOzone {
                            m = m * pvpart * 48.0 / 29.0 * settings.ozonescale / 1.0e9;
                        }
                        parts.xmass1[s * nspec] = m;
                    } else {
                        jj -= 1;
                    }
                }
            }
            numparttot += ncolumn;
            if settings.ipin == 0 {
                let np = parts.numpart as i64 + jj;
                if np < 0 || np as usize > maxpart {
                    return Err(DomainFillError::TooManyParticles);
                }
                parts.numpart = np as usize;
            }
        }
    }

    state.xmassperparticle = colmasstotal / numparttot as f64;

    // Particles outside the domain are terminated.
    for j in 0..parts.numpart {
        let (x, y) = (parts.xtra1[j], parts.ytra1[j]);
        if x < 0.0 || x >= nxmin1 as f64 || y < 0.0 || y >= nymin1 as f64 {
            parts.itra1[j] = ITRA_INACTIVE;
        }
    }

    // Fewer release heights per column for the boundary conditions.
    let mut fractus = state.numcolumn as f64 / nz as f64;
    fractus = fractus.max(1.0).sqrt() / 2.0;

    for jy in jy0..=jy1 {
        for ix in ix0..=ix1 {
            let ncolumn =
                nint(0.999 / fractus * npart1 as f64 * colmass[col(ix, jy)] / colmasstotal);
            if ncolumn > state.maxcolumn as i64 {
                return Err(DomainFillError::MaxColumnTooSmall);
            }
            if ncolumn == 0 {
                continue;
            }
            if ix == ix0 {
                state.numcolumn_we[2 * jy as usize] = ncolumn;
            }
            if ix == ix1 {
                state.numcolumn_we[1 + 2 * jy as usize] = ncolumn;
            }
            if jy == jy0 {
                state.numcolumn_sn[2 * ix as usize] = ncolumn;
            }
            if jy == jy1 {
                state.numcolumn_sn[1 + 2 * ix as usize] = ncolumn;
            }

            for (kz, p) in pp.iter_mut().enumerate() {
                *p = pp_of(ix, jy, kz);
            }
            let deltacol = (pp[0] - pp[nz - 1]) / ncolumn as f64;
            let mut pnew = pp[0] + deltacol / 2.0;
            for j in 0..ncolumn {
                pnew -= deltacol;
                for kz in 0..nz - 1 {
                    if !(pp[kz] >= pnew && pp[kz + 1] < pnew) {
                        continue;
                    }
                    let dz1 = pp[kz] - pnew;
                    let dz2 = pnew - pp[kz + 1];
                    let dz = 1.0 / (dz1 + dz2);
                    let mut zposition = (height[kz] * dz2 + height[kz + 1] * dz1) * dz;
                    if zposition > height[nz - 1] - 0.5 {
                        zposition = height[nz - 1] - 0.5;
                    }
                    let u = DomainFillError::UndefinedRead;
                    if ix == ix0 {
                        let l = state.we(0, jy, j).ok_or(u)?;
                        state.zcolumn_we[l] = zposition;
                    }
                    if ix == ix1 {
                        let l = state.we(1, jy, j).ok_or(u)?;
                        state.zcolumn_we[l] = zposition;
                    }
                    if jy == jy0 {
                        let l = state.sn(0, ix, j).ok_or(u)?;
                        state.zcolumn_sn[l] = zposition;
                    }
                    if jy == jy1 {
                        let l = state.sn(1, ix, j).ok_or(u)?;
                        state.zcolumn_sn[l] = zposition;
                    }
                    // Upstream zeroes acc_mass_sn at (k, jy, j): see the
                    // module docs.
                    for k in 0..2 {
                        let l = state.we(k, jy, j).ok_or(u)?;
                        state.acc_mass_we[l] = 0.0;
                    }
                    for k in 0..2 {
                        let l = state.sn(k, jy, j).ok_or(u)?;
                        state.acc_mass_sn[l] = 0.0;
                    }
                }
            }
        }
    }

    // Drop invalid particles at the end of the arrays.
    while parts.numpart > 0 && parts.itra1[parts.numpart - 1] == ITRA_INACTIVE {
        parts.numpart -= 1;
    }

    Ok(InitDomainFillOutcome {
        early_return: false,
        colmasstotal,
        restart_read_required: settings.ipin == 1 && !state.gdomainfill,
    })
}

/// What [`boundcond_domainfill`] reports besides the state it writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoundcondOutcome {
    /// `true` if upstream returned at once (`gdomainfill`).
    pub early_return: bool,
    /// Upstream would now write `boundcond.bin` (`ipout > 0` and
    /// `itime == loutend`); the caller saves [`DomainFillState`].
    pub dump_required: bool,
}

/// One boundary (`we`: western/eastern, else southern/northern).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Side {
    WestEast,
    SouthNorth,
}

/// `boundcond_domainfill(itime, loutend)`: terminate particles that left the
/// box and inject new ones where air flows in.
///
/// # Arguments
/// - `itime` — current time, s; `loutend` — end of the current output
///   averaging interval, s (only decides [`BoundcondOutcome::dump_required`]).
/// - `grid`, `settings` — the `com_mod` grid and switches.
/// - `met` — mother-grid `uu`, `vv`, `rho`, `height`, `memtime`, `memind`.
/// - `pv` — potential vorticity, pvu, laid out like `met`'s 3-D fields.
/// - `draws` — the `ran1` sequence.
/// - `parts`, `state` — read and written.
///
/// # Errors
/// See [`DomainFillError`]. A boundary column with exactly two release heights
/// is refused before anything is changed.
#[allow(clippy::too_many_arguments)]
pub fn boundcond_domainfill<I: Iterator<Item = f64>>(
    itime: i64,
    loutend: i64,
    grid: &DomainFillGrid,
    settings: &DomainFillSettings,
    met: &MetFields,
    pv: &[f64],
    draws: &mut I,
    parts: &mut ParticleStore,
    state: &mut DomainFillState,
) -> Result<BoundcondOutcome, DomainFillError> {
    if state.gdomainfill {
        return Ok(BoundcondOutcome {
            early_return: true,
            dump_required: false,
        });
    }
    let nz = met.nz;
    let height = &met.height;
    let ext = grid.extents;
    let nspec = parts.nspec;
    let maxpart = parts.maxpart();
    let [ix0, ix1] = state.nx_we;
    let [jy0, jy1] = state.ny_sn;

    // deltaz of the last release height reads zcolumn(j-2): refuse a column
    // of exactly two before changing anything.
    for jy in jy0..=jy1 {
        for k in 0..2 {
            if state.ncol_we(k, jy) == 2 {
                return Err(DomainFillError::UndefinedRead);
            }
        }
    }
    for ix in ix0..=ix1 {
        for k in 0..2 {
            if state.ncol_sn(k, ix) == 2 {
                return Err(DomainFillError::UndefinedRead);
            }
        }
    }

    // Terminate particles that have left the box.
    for i in 0..parts.numpart {
        if parts.itra1[i] == itime {
            let (x, y) = (parts.xtra1[i], parts.ytra1[i]);
            if y > jy1 as f64 || y < jy0 as f64 {
                parts.itra1[i] = ITRA_INACTIVE;
            }
            if (!grid.xglobal || ix1 != grid.nx - 2) && (x < ix0 as f64 || x > ix1 as f64) {
                parts.itra1[i] = ITRA_INACTIVE;
            }
        }
    }

    let dt1 = (itime - met.memtime[0]) as f64;
    let dt2 = (met.memtime[1] - itime) as f64;
    let dtt = 1.0 / (dt1 + dt2);

    let mut minpart: usize = 0;
    let u = DomainFillError::UndefinedRead;

    for side in [Side::WestEast, Side::SouthNorth] {
        let (outer0, outer1) = match side {
            Side::WestEast => (jy0, jy1),
            Side::SouthNorth => (ix0, ix1),
        };
        for b in outer0..=outer1 {
            for k in 0..2 {
                // Boundary cell (ix, jy) of the flux, and latitude of the
                // southern/northern boundary.
                let (bix, bjy) = match side {
                    Side::WestEast => (state.nx_we[k], b),
                    Side::SouthNorth => (b, state.ny_sn[k]),
                };
                let ylat_sn = grid.ylat0 + state.ny_sn[k] as f64 * grid.dy;
                let cosfact = (ylat_sn * PI180).cos();
                let ncol = match side {
                    Side::WestEast => state.ncol_we(k, b),
                    Side::SouthNorth => state.ncol_sn(k, b),
                };
                let zidx = |s: &DomainFillState, j: i64| match side {
                    Side::WestEast => s.we(k, b, j),
                    Side::SouthNorth => s.sn(k, b, j),
                };
                let zc = |s: &DomainFillState, j: i64| -> Result<f64, DomainFillError> {
                    let l = zidx(s, j).ok_or(u)?;
                    Ok(match side {
                        Side::WestEast => s.zcolumn_we[l],
                        Side::SouthNorth => s.zcolumn_sn[l],
                    })
                };
                for j in 0..ncol {
                    // Boundary area of this release height.
                    let deltaz = if j == 0 {
                        (zc(state, 1)? + zc(state, 0)?) / 2.0
                    } else if j == ncol - 1 {
                        (zc(state, j)? - zc(state, j - 2)?) / 2.0
                    } else {
                        (zc(state, j + 1)? - zc(state, j - 1)?) / 2.0
                    };
                    let boundarea = match side {
                        Side::WestEast => {
                            if b == jy0 || b == jy1 {
                                deltaz * 111_198.5 / 2.0 * grid.dy
                            } else {
                                deltaz * 111_198.5 * grid.dy
                            }
                        }
                        Side::SouthNorth => {
                            if b == ix0 || b == ix1 {
                                deltaz * 111_198.5 / 2.0 * cosfact * grid.dx
                            } else {
                                deltaz * 111_198.5 * cosfact * grid.dx
                            }
                        }
                    };

                    // Wind and density at the release height, in time.
                    let zj = zc(state, j)?;
                    let indz = level_below(height, nz, zj)?;
                    let dz1 = zj - height[indz];
                    let dz2 = height[indz + 1] - zj;
                    let dz = 1.0 / (dz1 + dz2);
                    let wind = match side {
                        Side::WestEast => &met.uu,
                        Side::SouthNorth => &met.vv,
                    };
                    let mut windhl = [0.0; 2];
                    let mut rhohl = [0.0; 2];
                    for m in 0..2 {
                        let indexh = met.memind[m];
                        let i0 = met.idx3(bix as usize, bjy as usize, indz, indexh);
                        let i1 = met.idx3(bix as usize, bjy as usize, indz + 1, indexh);
                        windhl[m] = (dz2 * wind[i0] + dz1 * wind[i1]) * dz;
                        rhohl[m] = (dz2 * met.rho[i0] + dz1 * met.rho[i1]) * dz;
                    }
                    let windx = (windhl[0] * dt2 + windhl[1] * dt1) * dtt;
                    let rhox = (rhohl[0] * dt2 + rhohl[1] * dt1) * dtt;

                    let fluxofmass = windx * rhox * boundarea * settings.lsynctime as f64;

                    // Accumulate inflow; outflow resets.
                    let xmpp = state.xmassperparticle;
                    let la = zidx(state, j).ok_or(u)?;
                    let acc = match side {
                        Side::WestEast => &mut state.acc_mass_we[la],
                        Side::SouthNorth => &mut state.acc_mass_sn[la],
                    };
                    if k == 0 {
                        if fluxofmass >= 0.0 {
                            *acc += fluxofmass;
                        } else {
                            *acc = 0.0;
                        }
                    } else if fluxofmass <= 0.0 {
                        *acc += fluxofmass.abs();
                    } else {
                        *acc = 0.0;
                    }

                    let mmass = if *acc >= xmpp / 2.0 {
                        let mmass = fint((*acc + xmpp / 2.0) / xmpp);
                        *acc -= mmass as f64 * xmpp;
                        mmass
                    } else {
                        0
                    };

                    for _m in 0..mmass {
                        let Some(ipart) = (minpart..maxpart).find(|&ip| parts.itra1[ip] != itime)
                        else {
                            return Err(DomainFillError::TooManyParticles);
                        };

                        // Position on the boundary.
                        let (x, y) = match side {
                            Side::WestEast => {
                                let x = state.nx_we[k] as f64;
                                let y = if b == jy0 {
                                    b as f64 + 0.5 * draw(draws)?
                                } else if b == jy1 {
                                    b as f64 - 0.5 * draw(draws)?
                                } else {
                                    b as f64 + (draw(draws)? - 0.5)
                                };
                                (x, y)
                            }
                            Side::SouthNorth => {
                                let y = state.ny_sn[k] as f64;
                                let x = if b == ix0 {
                                    b as f64 + 0.5 * draw(draws)?
                                } else if b == ix1 {
                                    b as f64 - 0.5 * draw(draws)?
                                } else {
                                    b as f64 + (draw(draws)? - 0.5)
                                };
                                (x, y)
                            }
                        };
                        parts.xtra1[ipart] = x;
                        parts.ytra1[ipart] = y;
                        let z = if j == 0 {
                            zc(state, 0)? + (zc(state, 1)? - zc(state, 0)?) / 4.0
                        } else if j == ncol - 1 {
                            (2.0 * zc(state, j)? + zc(state, j - 1)? + height[nz - 1]) / 4.0
                        } else {
                            zc(state, j - 1)?
                                + draw(draws)? * (zc(state, j + 1)? - zc(state, j - 1)?)
                        };
                        parts.ztra1[ipart] = z;

                        // PV at the particle, in time.
                        let (ixm, jym, w) = weights(x, y);
                        let indzm = level_below(height, nz, z)?;
                        let dz1 = z - height[indzm];
                        let dz2 = height[indzm + 1] - z;
                        let dz = 1.0 / (dz1 + dz2);
                        let mut yh1 = [0.0; 2];
                        for mm in 0..2 {
                            let indexh = met.memind[mm];
                            let a = pv_bilinear(met, pv, ext, ixm, jym, indzm, indexh, w)?;
                            let c = pv_bilinear(met, pv, ext, ixm, jym, indzm + 1, indexh, w)?;
                            yh1[mm] = (dz2 * a + dz1 * c) * dz;
                        }
                        let mut pvpart = (yh1[0] * dt2 + yh1[1] * dt1) * dtt;
                        let ylat = match side {
                            Side::WestEast => grid.ylat0 + y * grid.dy,
                            Side::SouthNorth => ylat_sn,
                        };
                        if ylat < 0.0 {
                            pvpart = -1.0 * pvpart;
                        }

                        if (z > 3000.0 && pvpart > settings.pvcrit)
                            || settings.mode == DomainFillMode::Air
                        {
                            parts.nclass[ipart] = (fint(draw(draws)? * settings.nclassunc as f64)
                                + 1)
                            .min(settings.nclassunc);
                            parts.numparticlecount += 1;
                            parts.npoint[ipart] = parts.numparticlecount;
                            parts.idt[ipart] = settings.mintime;
                            parts.itra1[ipart] = itime;
                            parts.itramem[ipart] = itime;
                            parts.itrasplit[ipart] = itime + settings.ldirect * settings.itsplit;
                            let mut m = xmpp;
                            if settings.mode == DomainFillMode::StratosphericOzone {
                                m = m * pvpart * 48.0 / 29.0 * settings.ozonescale / 1.0e9;
                            }
                            parts.xmass1[ipart * nspec] = m;
                        } else {
                            // Upstream `goto 71`: slot stays vacant, positions
                            // already overwritten, minpart unchanged.
                            continue;
                        }
                        parts.numpart = parts.numpart.max(ipart + 1);
                        minpart = ipart + 1;
                    }
                }
            }
        }
    }

    Ok(BoundcondOutcome {
        early_return: false,
        dump_required: settings.ipout > 0 && itime == loutend,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_release_heights_are_refused_before_any_change() {
        let mut state = DomainFillState::new(10, 10, 5);
        state.nx_we = [1, 3];
        state.ny_sn = [1, 3];
        state.numcolumn_we[2 * 2] = 2;
        let met = MetFields::zeros(5, 5, 3, 1);
        let grid = DomainFillGrid {
            nx: 5,
            ny: 5,
            xglobal: false,
            sglobal: false,
            nglobal: false,
            dx: 1.0,
            dy: 1.0,
            ylat0: 0.0,
            dyconst: 1.0,
            extents: StaticExtents {
                nxmax: 10,
                nymax: 10,
                nzmax: 3,
            },
        };
        let settings = DomainFillSettings {
            mode: DomainFillMode::Air,
            ipin: 0,
            ipout: 0,
            pvcrit: 2.0,
            ozonescale: 60.0,
            nclassunc: 1,
            mintime: 1,
            ldirect: 1,
            itsplit: 1,
            lsynctime: 1,
        };
        let mut parts = ParticleStore::new(4, 1);
        parts.itra1[0] = 7;
        parts.xtra1[0] = -5.0;
        parts.numpart = 1;
        let before = parts.clone();
        let r = boundcond_domainfill(
            7,
            0,
            &grid,
            &settings,
            &met,
            &vec![0.0; met.uu.len()],
            &mut std::iter::empty(),
            &mut parts,
            &mut state,
        );
        assert_eq!(r, Err(DomainFillError::UndefinedRead));
        assert_eq!(parts, before);
    }

    #[test]
    fn layout_matches_fortran_column_major() {
        let s = DomainFillState::new(4, 3, 2);
        assert_eq!(s.we(1, 2, 1), Some(1 + 2 * (2 + 3)));
        assert_eq!(s.we(0, 0, -1), None);
        // acc_mass_sn(k, jy, j) with jy == nxmax aliases to (k, 0, j+1).
        assert_eq!(s.sn(0, 4, 0), s.sn(0, 0, 1));
    }
}
