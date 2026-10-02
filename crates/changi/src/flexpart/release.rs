// SPDX-License-Identifier: GPL-3.0
//
// FLEXPART port — provenance
// --------------------------
// Upstream project : FLEXPART (NILU) — https://github.com/flexpart/flexpart
// Upstream version : 10.4 (2019-11-12), commit 3d7eebf
// Upstream source  : src/releaseparticles.f90
//                    (reads com_mod.f90, point_mod.f90, xmass_mod.f90; calls
//                    juldate/caldate through `calendar`, already ported)
// Original licence : GPL-3.0-or-later — SPDX-FileCopyrightText: FLEXPART 1998-2019
// Ported into this GPL-3.0 work; see LICENSE.flexpart and NOTICE.flexpart.

//! Particle release from the release points: `releaseparticles.f90`.
//!
//! At every synchronisation time `itime`, each release point whose window
//! `[ireleasestart, ireleaseend]` contains `itime` releases `numrel`
//! particles into vacant storage slots. A slot is vacant when its particle
//! is not at the current time (`itra1 != itime`): every live particle is
//! synchronised to `itime` when this routine runs, so any other value marks
//! a slot that was never used or whose particle has terminated.
//!
//! Per particle, upstream draws a random position in the release box, gives
//! it the mass of the point divided by `npart` (scaled by the species-
//! dependent emission time profile), converts the starting height to metres
//! above ground (`kindz`), clamps it to `[eps2, height(nz) - 0.5]`, and for
//! `ind_rel` 1, 3, 4 multiplies the mass by the air density at the start.
//!
//! # What this routine does NOT do
//!
//! There is **no radioactive decay** of the release mass in
//! `releaseparticles.f90` (v10.4): decay is applied to particles after
//! release, in `timemanager.f90` (ported in [`super::decay`]). There is also
//! no particle splitting here; only `itrasplit`, the time of the first split,
//! is set.
//!
//! # Random numbers are an INPUT
//!
//! Upstream calls `ran1` (Numerical Recipes, `random_mod.f90`, not ported and
//! not re-implemented: gh:#410) four times per released particle, in the
//! order x, y, uncertainty class, z. The port takes these draws from an
//! iterator the caller supplies; the code-to-code driver replaces `ran1` by a
//! shim that returns driver-chosen values, so both codes consume identical
//! numbers. Draws must lie in `[0, 1)` for upstream's arithmetic to mean what
//! it says, but the port does not check.
//!
//! # Upstream quirks reproduced
//!
//! - **The local day of week and hour** use `julmonday = juldate(19000101, 0)`
//!   and a longitude correction `xlonav / 360` days; daylight saving is a flat
//!   `+1 h` for **every** release whose UTC month (of `bdate + itime`) is
//!   April to September, in both hemispheres.
//! - **Hour 0 is hour 24 of the previous day.** `nint` of the hour can also
//!   give 24 directly (from 23:30 local), so hour 24 of the *same* day is
//!   reachable too; both read the profile's 24th entry.
//! - **Point or area profile** is chosen by the box's horizontal extent
//!   (`|dx| < 1e-4` and `|dy| < 1e-4` grid units), not by its height (the
//!   height test is commented out upstream).
//! - **`xmasssave` carries the fractional particle** from one call to the
//!   next, per release point.
//! - **`kindz = 3` (pressure)**: a pressure above every level's pressure
//!   places the particle at `height(1) / 2`; a pressure below every level's
//!   leaves `ztra1` holding the **pressure value in hPa, read as metres**
//!   (upstream never assigns it), after which only the clamps apply.
//! - **Topography is interpolated for every particle**, whatever `kindz`, and
//!   used only for `kindz = 2`.
//! - **Density** for `ind_rel` 1, 3, 4 and for `kindz = 3` comes from memory
//!   slot **2** of `rho`/`tt` (`rho(...,2)`, "accurate enough" upstream), not
//!   from the time-interpolated field.
//! - **`rho_rel(i)`** holds the density of the *last* particle released from
//!   point `i`.
//! - **`minpart` persists across release points** within one call, so
//!   points released later in the loop never back-fill slots earlier than
//!   the last one used.
//! - **Reads past the filled grid.** At `xtra1 == nxmin1` exactly (and the
//!   same in y), `ixp = nx` addresses a column of the static `com_mod`
//!   arrays (dimension `0:nxmax-1`) that no reader fills; its weight is zero.
//!   See [`StaticExtents`] for how the port reproduces that read.
//!
//! # Refusals
//!
//! Where upstream would `stop` or read memory it never defined, the port
//! returns a [`ReleaseError`]. The particle arrays are then partly updated,
//! as upstream's would have been at that point; the caller should treat them
//! as invalid.
//!
//! # Index conventions and units
//!
//! Particle slots and release points are 0-based (`ipart - 1`, `i - 1`);
//! the *values* stored in `npoint` and `nclass` are upstream's (1-based).
//! Positions in grid units (`xtra1`, `ytra1`), heights m, times s, masses kg
//! (or whatever unit the release file used), pressures hPa, densities kg/m³.

use super::advance::NestGeometry;
use super::calendar::{caldate, juldate};
use super::constants::R_AIR;
use super::interpolation::MetFields;

/// `itra1` value of a terminated or never-used particle (`-999999999`).
pub const ITRA_INACTIVE: i64 = -999_999_999;

/// Number of wind fields held in memory (`numwfmem`, serial build).
pub const NUMWFMEM: usize = 2;

/// The `par_mod` array extents of upstream's **static** `com_mod` fields
/// (`oro`, `rho`, `tt`, `pv`, ...: `(0:nxmax-1, 0:nymax-1, nzmax, numwfmem)`).
///
/// Upstream sometimes indexes one column or row past the filled grid with a
/// zero weight (e.g. `ixp = nx` at `xtra1 == nxmin1`). For a static array that
/// is a defined read: column-major addressing lands on the unused tail of the
/// array when `nx < nxmax`, or on the next row/level when `nx == nxmax`. The
/// port reproduces that address arithmetic exactly, and returns the stored
/// value when the address falls inside the filled grid and **zero**
/// otherwise. Zero is what a static array holds where no reader ever wrote
/// (they live in `.bss`), which is true of a FLEXPART run on one fixed grid;
/// it is not true if the same process earlier filled a larger grid, which
/// FLEXPART never does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StaticExtents {
    /// `nxmax`.
    pub nxmax: usize,
    /// `nymax`.
    pub nymax: usize,
    /// `nzmax`.
    pub nzmax: usize,
}

impl StaticExtents {
    /// The shipped `par_mod.f90`: `nxmax=361, nymax=181, nzmax=138`.
    pub const SHIPPED: Self = Self {
        nxmax: 361,
        nymax: 181,
        nzmax: 138,
    };

    /// Read `field(ix, jy, k, slot)` of a static `com_mod` array whose filled
    /// part is `met`'s grid (`met.nx x met.ny x met.nz x 2`, laid out as
    /// [`MetFields::idx3`]). `k` and `slot` are 0-based. `None` when the
    /// address falls outside the whole array (upstream would read another
    /// variable).
    #[must_use]
    pub fn read3(
        &self,
        met: &MetFields,
        field: &[f64],
        ix: i64,
        jy: i64,
        k: i64,
        slot: i64,
    ) -> Option<f64> {
        let (nxm, nym, nzm) = (self.nxmax as i64, self.nymax as i64, self.nzmax as i64);
        let lin = ix + nxm * (jy + nym * (k + nzm * slot));
        if lin < 0 || lin >= nxm * nym * nzm * NUMWFMEM as i64 {
            return None;
        }
        let i = lin % nxm;
        let j = (lin / nxm) % nym;
        let kk = (lin / (nxm * nym)) % nzm;
        let s = lin / (nxm * nym * nzm);
        if (i as usize) < met.nx && (j as usize) < met.ny && (kk as usize) < met.nz {
            Some(field[met.idx3(i as usize, j as usize, kk as usize, s as usize)])
        } else {
            Some(0.0)
        }
    }

    /// Read `field(ix, jy)` of a static two-dimensional `com_mod` array
    /// (`(0:nxmax-1, 0:nymax-1)`, e.g. `oro`) whose filled part is
    /// `nx x ny`, stored `[jy * nx + ix]`.
    #[must_use]
    pub fn read2(&self, nx: usize, ny: usize, field: &[f64], ix: i64, jy: i64) -> Option<f64> {
        let (nxm, nym) = (self.nxmax as i64, self.nymax as i64);
        let lin = ix + nxm * jy;
        if lin < 0 || lin >= nxm * nym {
            return None;
        }
        let i = (lin % nxm) as usize;
        let j = (lin / nxm) as usize;
        if i < nx && j < ny {
            Some(field[j * nx + i])
        } else {
            Some(0.0)
        }
    }
}

/// `kindz`: what a release point's `zpoint1`, `zpoint2` mean.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KindZ {
    /// `1`: metres above ground.
    AboveGround,
    /// `2`: metres above sea level (topography is subtracted).
    AboveSeaLevel,
    /// `3`: pressure, hPa.
    Pressure,
}

/// One release point of `point_mod`, in grid units (after `coordtrafo`).
#[derive(Debug, Clone, PartialEq)]
pub struct ReleasePoint {
    /// Start of the release window, s (`ireleasestart`).
    pub ireleasestart: i64,
    /// End of the release window, s (`ireleaseend`).
    pub ireleaseend: i64,
    /// Total number of particles over the window (`npart`).
    pub npart: i64,
    /// Meaning of `zpoint1`, `zpoint2` (`kindz`).
    pub kindz: KindZ,
    /// Release box, mother-grid units.
    pub xpoint1: f64,
    /// Release box, mother-grid units.
    pub xpoint2: f64,
    /// Release box, mother-grid units.
    pub ypoint1: f64,
    /// Release box, mother-grid units.
    pub ypoint2: f64,
    /// Release box bottom (m or hPa per `kindz`).
    pub zpoint1: f64,
    /// Release box top (m or hPa per `kindz`).
    pub zpoint2: f64,
    /// Total mass per species over the window (`xmass(i, 1:nspec)`).
    pub xmass: Vec<f64>,
}

/// Emission time profiles of `com_mod` (`area_hour`, `point_hour`,
/// `area_dow`, `point_dow`), as read from the `RELEASES` file.
#[derive(Debug, Clone, PartialEq)]
pub struct EmissionVariation {
    /// Hourly factors of area sources, `[k * 24 + (nhour - 1)]`.
    pub area_hour: Vec<f64>,
    /// Hourly factors of point sources, `[k * 24 + (nhour - 1)]`.
    pub point_hour: Vec<f64>,
    /// Day-of-week factors of area sources, `[k * 7 + (ndayofweek - 1)]`
    /// (day 1 = Monday).
    pub area_dow: Vec<f64>,
    /// Day-of-week factors of point sources, `[k * 7 + (ndayofweek - 1)]`.
    pub point_dow: Vec<f64>,
}

impl EmissionVariation {
    /// All factors 1 (no time variation) for `nspec` species.
    #[must_use]
    pub fn uniform(nspec: usize) -> Self {
        Self {
            area_hour: vec![1.0; 24 * nspec],
            point_hour: vec![1.0; 24 * nspec],
            area_dow: vec![1.0; 7 * nspec],
            point_dow: vec![1.0; 7 * nspec],
        }
    }
}

/// The particle arrays of `com_mod` (`itra1`, ..., `xscav_frac1`) and the two
/// counters `numpart`, `numparticlecount`. The arrays' length is `maxpart`.
#[derive(Debug, Clone, PartialEq)]
pub struct ParticleStore {
    /// Time the particle is at, s; [`ITRA_INACTIVE`] when terminated.
    pub itra1: Vec<i64>,
    /// Release point (1-based) or, with `mquasilag`/domain filling, the
    /// particle's running number.
    pub npoint: Vec<i64>,
    /// Uncertainty class, `1..=nclassunc`.
    pub nclass: Vec<i64>,
    /// Time step of the next integration, s.
    pub idt: Vec<i64>,
    /// Release time, s.
    pub itramem: Vec<i64>,
    /// Time of the next split, s.
    pub itrasplit: Vec<i64>,
    /// x position, grid units (`real(kind=dp)` upstream).
    pub xtra1: Vec<f64>,
    /// y position, grid units (`real(kind=dp)` upstream).
    pub ytra1: Vec<f64>,
    /// Height above ground, m.
    pub ztra1: Vec<f64>,
    /// Species stored per particle.
    pub nspec: usize,
    /// Mass per species, `[ipart * nspec + k]`.
    pub xmass1: Vec<f64>,
    /// Scavenged fraction per species, `[ipart * nspec + k]`.
    pub xscav_frac1: Vec<f64>,
    /// Highest slot in use, 1-based (`numpart`).
    pub numpart: usize,
    /// Particles released so far (`numparticlecount`).
    pub numparticlecount: i64,
}

impl ParticleStore {
    /// `maxpart` empty slots, as `FLEXPART.f90` initialises them
    /// (`itra1 = -999999999`, everything else zero).
    #[must_use]
    pub fn new(maxpart: usize, nspec: usize) -> Self {
        Self {
            itra1: vec![ITRA_INACTIVE; maxpart],
            npoint: vec![0; maxpart],
            nclass: vec![0; maxpart],
            idt: vec![0; maxpart],
            itramem: vec![0; maxpart],
            itrasplit: vec![0; maxpart],
            xtra1: vec![0.0; maxpart],
            ytra1: vec![0.0; maxpart],
            ztra1: vec![0.0; maxpart],
            nspec,
            xmass1: vec![0.0; maxpart * nspec],
            xscav_frac1: vec![0.0; maxpart * nspec],
            numpart: 0,
            numparticlecount: 0,
        }
    }

    /// `maxpart`.
    #[must_use]
    pub fn maxpart(&self) -> usize {
        self.itra1.len()
    }
}

/// One nest as `releaseparticles` reads it.
#[derive(Debug, Clone, PartialEq)]
pub struct ReleaseNest {
    /// Placement in mother-grid units.
    pub geometry: NestGeometry,
    /// The nest's fields; `rho`, `tt` (slot 2) and `height` are read.
    pub met: MetFields,
    /// Nest topography, m, `[jy * nx + ix]` (`oron`).
    pub oro: Vec<f64>,
}

/// The mother grid and clock as `releaseparticles` reads them.
#[derive(Debug, Clone, PartialEq)]
pub struct ReleaseDomain {
    /// `nx - 1`.
    pub nxmin1: i64,
    /// Cyclic in longitude (`xglobal`).
    pub xglobal: bool,
    /// Longitude of the grid origin, degrees (`xlon0`).
    pub xlon0: f64,
    /// Grid spacing in x, degrees (`dx`).
    pub dx: f64,
    /// Julian date of the simulation start, days (`bdate`, `real(kind=dp)`).
    pub bdate: f64,
    /// Static array extents; `nxmax` also sets the nest margin
    /// `eps = nxmax / 3e5`.
    pub extents: StaticExtents,
}

/// Run switches `releaseparticles` reads from `com_mod`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReleaseSettings {
    /// `+1` forward, `-1` backward.
    pub ldirect: i64,
    /// Synchronisation interval, s.
    pub lsynctime: i64,
    /// Minimum time step, s (`mintime`), assigned to `idt`.
    pub mintime: i64,
    /// Splitting time constant, s (`itsplit`).
    pub itsplit: i64,
    /// Number of uncertainty classes (`nclassunc`).
    pub nclassunc: i64,
    /// `mquasilag != 0`: `npoint` stores the particle number.
    pub mquasilag: bool,
    /// `ind_rel` is 1, 3 or 4: multiply the mass by the air density.
    pub density_weighted: bool,
    /// `DRYBKDEP .or. WETBKDEP`: set `xscav_frac1 = -1` on release.
    pub backward_deposition: bool,
}

/// Why [`releaseparticles`] refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReleaseError {
    /// No vacant slot left: upstream's `stop` ("TOTAL NUMBER OF PARTICLES
    /// REQUIRED EXCEEDS THE MAXIMUM ALLOWED NUMBER").
    TooManyParticles,
    /// The draw iterator ran dry.
    DrawsExhausted,
    /// The local day of week or hour fell outside `1..=7` / `1..=24`
    /// (a date before 1900-01-01): upstream reads outside the profile arrays.
    TimeProfileIndex,
    /// A grid read outside the static array (upstream reads another
    /// variable), or outside a nest (allocatable, undefined there).
    UndefinedRead,
    /// No model level lies above the particle (a NaN height); upstream would
    /// use an undefined level index.
    UndefinedLevel,
}

fn draw<I: Iterator<Item = f64>>(draws: &mut I) -> Result<f64, ReleaseError> {
    draws.next().ok_or(ReleaseError::DrawsExhausted)
}

/// Fortran `int()` of a real: truncation toward zero.
fn fint(x: f64) -> i64 {
    x.trunc() as i64
}

#[allow(clippy::too_many_arguments)]
/// A 3-D read on the mother grid (static) or a nest (allocatable: refused
/// outside the filled grid).
fn read3(
    ngrid: usize,
    mother: &MetFields,
    nests: &[ReleaseNest],
    ext: StaticExtents,
    pick: fn(&MetFields) -> &[f64],
    ix: i64,
    jy: i64,
    k: i64,
    slot: i64,
) -> Result<f64, ReleaseError> {
    if ngrid > 0 {
        let m = &nests[ngrid - 1].met;
        if ix < 0 || jy < 0 || ix as usize >= m.nx || jy as usize >= m.ny {
            return Err(ReleaseError::UndefinedRead);
        }
        Ok(pick(m)[m.idx3(ix as usize, jy as usize, k as usize, slot as usize)])
    } else {
        ext.read3(mother, pick(mother), ix, jy, k, slot)
            .ok_or(ReleaseError::UndefinedRead)
    }
}

fn pick_rho(m: &MetFields) -> &[f64] {
    &m.rho
}

fn pick_tt(m: &MetFields) -> &[f64] {
    &m.tt
}

/// `releaseparticles(itime)`: release the particles due at `itime`.
///
/// # Arguments
/// - `itime` — current time, s since the simulation start.
/// - `points` — the release points (`point_mod`), grid units.
/// - `xmasssave` — per point, the fractional particle carried between calls
///   (`xmass_mod`); read and updated.
/// - `rho_rel` — per point, the air density at the last particle released
///   when `density_weighted`; written.
/// - `variation` — the emission time profiles.
/// - `settings`, `domain` — the `com_mod` switches and grid.
/// - `mother` — mother-grid fields; `rho`, `tt` (memory slot 2) and `height`
///   are read.
/// - `oro` — mother-grid topography, m, `[jy * nx + ix]`.
/// - `nests` — nested grids, innermost last.
/// - `draws` — the `ran1` sequence (see the module docs).
/// - `parts` — the particle arrays; vacant slots are filled.
///
/// # Errors
/// See [`ReleaseError`].
#[allow(clippy::too_many_arguments, clippy::needless_range_loop)]
pub fn releaseparticles<I: Iterator<Item = f64>>(
    itime: i64,
    points: &[ReleasePoint],
    xmasssave: &mut [f64],
    rho_rel: &mut [f64],
    variation: &EmissionVariation,
    settings: &ReleaseSettings,
    domain: &ReleaseDomain,
    mother: &MetFields,
    oro: &[f64],
    nests: &[ReleaseNest],
    draws: &mut I,
    parts: &mut ParticleStore,
) -> Result<(), ReleaseError> {
    let nspec = parts.nspec;
    let maxpart = parts.maxpart();
    let ext = domain.extents;
    let eps = ext.nxmax as f64 / 3.0e5;
    let eps2 = 1.0e-6;
    let height = &mother.height;
    let nz = mother.nz;

    // Date and time in Greenwich, with a flat summer-time correction.
    let julmonday = juldate(19_000_101, 0);
    let mut jul = domain.bdate + itime as f64 / 86_400.0;
    let (jjjjmmdd, _ihmmss) = caldate(jul);
    let mm = (jjjjmmdd - 10_000 * (jjjjmmdd / 10_000)) / 100;
    if (4..=9).contains(&mm) {
        jul += 1.0 / 24.0;
    }

    let mut timecorrect = vec![0.0; nspec];
    let mut minpart: usize = 0;
    for (i, pt) in points.iter().enumerate() {
        if !(itime >= pt.ireleasestart && itime <= pt.ireleaseend) {
            continue;
        }

        // Local day of week and hour.
        let mut xlonav = domain.xlon0 + (pt.xpoint2 + pt.xpoint1) / 2.0 * domain.dx;
        if xlonav < -180.0 {
            xlonav += 360.0;
        }
        if xlonav > 180.0 {
            xlonav -= 360.0;
        }
        let jullocal = jul + xlonav / 360.0;
        let mut juldiff = jullocal - julmonday;
        let nweeks = fint(juldiff / 7.0);
        juldiff -= nweeks as f64 * 7.0;
        let mut ndayofweek = fint(juldiff) + 1;
        let mut nhour = ((juldiff - (ndayofweek - 1) as f64) * 24.0).round() as i64;
        if nhour == 0 {
            nhour = 24;
            ndayofweek -= 1;
            if ndayofweek == 0 {
                ndayofweek = 7;
            }
        }
        if !(1..=24).contains(&nhour) || !(1..=7).contains(&ndayofweek) {
            return Err(ReleaseError::TimeProfileIndex);
        }
        let (h, d) = ((nhour - 1) as usize, (ndayofweek - 1) as usize);

        // Species- and time-dependent correction factor.
        let mut average_timecorrect = 0.0;
        let point_source =
            (pt.xpoint2 - pt.xpoint1).abs() < 1.0e-4 && (pt.ypoint2 - pt.ypoint1).abs() < 1.0e-4;
        for (k, tc) in timecorrect.iter_mut().enumerate() {
            *tc = if point_source {
                variation.point_hour[k * 24 + h] * variation.point_dow[k * 7 + d]
            } else {
                variation.area_hour[k * 24 + h] * variation.area_dow[k * 7 + d]
            };
            average_timecorrect += *tc;
        }
        average_timecorrect /= nspec as f64;

        // Number of particles released this time.
        let numrel = if pt.ireleasestart != pt.ireleaseend {
            let mut rfraction = (pt.npart as f64 * settings.lsynctime as f64
                / (pt.ireleaseend - pt.ireleasestart) as f64)
                .abs();
            if itime == pt.ireleasestart || itime == pt.ireleaseend {
                rfraction /= 2.0;
            }
            rfraction *= average_timecorrect;
            rfraction += xmasssave[i];
            let numrel = fint(rfraction);
            xmasssave[i] = rfraction - numrel as f64;
            numrel
        } else {
            pt.npart
        };

        let xaux = pt.xpoint2 - pt.xpoint1;
        let yaux = pt.ypoint2 - pt.ypoint1;
        let zaux = pt.zpoint2 - pt.zpoint1;
        for _ in 0..numrel.max(0) {
            let Some(ipart) = (minpart..maxpart).find(|&ip| parts.itra1[ip] != itime) else {
                return Err(ReleaseError::TooManyParticles);
            };

            // Horizontal position.
            let mut x = pt.xpoint1 + draw(draws)? * xaux;
            if domain.xglobal {
                if x > domain.nxmin1 as f64 {
                    x -= domain.nxmin1 as f64;
                }
                if x < 0.0 {
                    x += domain.nxmin1 as f64;
                }
            }
            let y = pt.ypoint1 + draw(draws)? * yaux;
            parts.xtra1[ipart] = x;
            parts.ytra1[ipart] = y;

            // Mass, species-dependent time correction relative to the average.
            for k in 0..nspec {
                parts.xmass1[ipart * nspec + k] =
                    pt.xmass[k] / pt.npart as f64 * timecorrect[k] / average_timecorrect;
                if settings.backward_deposition {
                    parts.xscav_frac1[ipart * nspec + k] = -1.0;
                }
            }
            parts.nclass[ipart] =
                (fint(draw(draws)? * settings.nclassunc as f64) + 1).min(settings.nclassunc);
            parts.numparticlecount += 1;
            parts.npoint[ipart] = if settings.mquasilag {
                parts.numparticlecount
            } else {
                i as i64 + 1
            };
            parts.idt[ipart] = settings.mintime;
            parts.itra1[ipart] = itime;
            parts.itramem[ipart] = itime;
            parts.itrasplit[ipart] = itime + settings.ldirect * settings.itsplit;

            // Vertical position.
            let mut z = pt.zpoint1 + draw(draws)? * zaux;

            // Nest, innermost (last) first.
            let mut ngrid = 0usize;
            for (k, n) in nests.iter().enumerate().rev() {
                let g = &n.geometry;
                if x > g.xln + eps && x < g.xrn - eps && y > g.yln + eps && y < g.yrn - eps {
                    ngrid = k + 1;
                    break;
                }
            }

            let (ix, jy, ddx, ddy);
            if ngrid > 0 {
                let g = &nests[ngrid - 1].geometry;
                let xtn = (x - g.xln) * g.xresoln;
                let ytn = (y - g.yln) * g.yresoln;
                ix = fint(xtn);
                jy = fint(ytn);
                ddy = ytn - jy as f64;
                ddx = xtn - ix as f64;
            } else {
                ix = fint(x);
                jy = fint(y);
                ddy = y - jy as f64;
                ddx = x - ix as f64;
            }
            let ixp = ix + 1;
            let jyp = jy + 1;
            let rddx = 1.0 - ddx;
            let rddy = 1.0 - ddy;
            let p1 = rddx * rddy;
            let p2 = ddx * rddy;
            let p3 = rddx * ddy;
            let p4 = ddx * ddy;

            let topo = if ngrid > 0 {
                let n = &nests[ngrid - 1];
                let o = |a: i64, b: i64| -> Result<f64, ReleaseError> {
                    if a < 0 || b < 0 || a as usize >= n.met.nx || b as usize >= n.met.ny {
                        return Err(ReleaseError::UndefinedRead);
                    }
                    Ok(n.oro[b as usize * n.met.nx + a as usize])
                };
                p1 * o(ix, jy)? + p2 * o(ixp, jy)? + p3 * o(ix, jyp)? + p4 * o(ixp, jyp)?
            } else {
                let o = |a: i64, b: i64| -> Result<f64, ReleaseError> {
                    ext.read2(mother.nx, mother.ny, oro, a, b)
                        .ok_or(ReleaseError::UndefinedRead)
                };
                p1 * o(ix, jy)? + p2 * o(ixp, jy)? + p3 * o(ix, jyp)? + p4 * o(ixp, jyp)?
            };
            let bil = |pick: fn(&MetFields) -> &[f64], k: i64| -> Result<f64, ReleaseError> {
                let r = |a, b| read3(ngrid, mother, nests, ext, pick, a, b, k, 1);
                Ok(p1 * r(ix, jy)? + p2 * r(ixp, jy)? + p3 * r(ix, jyp)? + p4 * r(ixp, jyp)?)
            };

            // Pressure coordinates: find the level pair bracketing the pressure.
            if pt.kindz == KindZ::Pressure {
                let presspart = z;
                let mut pressold = 0.0;
                for kz in 1..=nz {
                    let r = bil(pick_rho, kz as i64 - 1)?;
                    let t = bil(pick_tt, kz as i64 - 1)?;
                    let press = r * R_AIR * t / 100.0;
                    if kz == 1 {
                        pressold = press;
                    }
                    if press < presspart {
                        if kz == 1 {
                            z = height[0] / 2.0;
                        } else {
                            let dz1 = pressold - presspart;
                            let dz2 = presspart - press;
                            z = (height[kz - 2] * dz2 + height[kz - 1] * dz1) / (dz1 + dz2);
                        }
                        break;
                    }
                    pressold = press;
                }
            }

            if pt.kindz == KindZ::AboveSeaLevel {
                z -= topo;
            }
            if z < eps2 {
                z = eps2;
            }
            if z > height[nz - 1] - 0.5 {
                z = height[nz - 1] - 0.5;
            }
            parts.ztra1[ipart] = z;

            if settings.density_weighted {
                let ii = (2..=nz)
                    .find(|&ii| height[ii - 1] > z)
                    .ok_or(ReleaseError::UndefinedLevel)?;
                let (indz, indzp) = (ii - 1, ii);
                let dz1 = z - height[indz - 1];
                let dz2 = height[indzp - 1] - z;
                let dz = 1.0 / (dz1 + dz2);
                let rhoaux0 = bil(pick_rho, indz as i64 - 1)?;
                let rhoaux1 = bil(pick_rho, indz as i64)?;
                let rhoout = (dz2 * rhoaux0 + dz1 * rhoaux1) * dz;
                rho_rel[i] = rhoout;
                for k in 0..nspec {
                    parts.xmass1[ipart * nspec + k] *= rhoout;
                }
            }

            parts.numpart = parts.numpart.max(ipart + 1);
            minpart = ipart + 1;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat_grid() -> MetFields {
        let mut m = MetFields::zeros(4, 4, 3, 1);
        m.height = vec![0.0, 100.0, 1000.0];
        for v in m.rho.iter_mut() {
            *v = 1.0;
        }
        for v in m.tt.iter_mut() {
            *v = 280.0;
        }
        m
    }

    fn point(start: i64, end: i64, npart: i64) -> ReleasePoint {
        ReleasePoint {
            ireleasestart: start,
            ireleaseend: end,
            npart,
            kindz: KindZ::AboveGround,
            xpoint1: 1.0,
            xpoint2: 2.0,
            ypoint1: 1.0,
            ypoint2: 2.0,
            zpoint1: 10.0,
            zpoint2: 20.0,
            xmass: vec![1.0],
        }
    }

    fn run(maxpart: usize, npart: i64) -> Result<ParticleStore, ReleaseError> {
        let m = flat_grid();
        let mut parts = ParticleStore::new(maxpart, 1);
        let mut save = vec![0.0];
        let mut rho_rel = vec![0.0];
        let settings = ReleaseSettings {
            ldirect: 1,
            lsynctime: 900,
            mintime: 10,
            itsplit: 100,
            nclassunc: 3,
            mquasilag: false,
            density_weighted: false,
            backward_deposition: false,
        };
        let domain = ReleaseDomain {
            nxmin1: 3,
            xglobal: false,
            xlon0: 0.0,
            dx: 1.0,
            bdate: juldate(20_200_115, 0),
            extents: StaticExtents::SHIPPED,
        };
        let mut d = std::iter::repeat(0.25);
        releaseparticles(
            0,
            &[point(0, 0, npart)],
            &mut save,
            &mut rho_rel,
            &EmissionVariation::uniform(1),
            &settings,
            &domain,
            &m,
            &[0.0; 16],
            &[],
            &mut d,
            &mut parts,
        )
        .map(|()| parts)
    }

    #[test]
    fn instantaneous_release_fills_npart_slots() {
        let p = run(10, 4).unwrap();
        assert_eq!(p.numpart, 4);
        assert_eq!(p.numparticlecount, 4);
        assert_eq!(p.xtra1[0], 1.25);
        assert_eq!(p.ztra1[0], 12.5);
        assert_eq!(p.nclass[0], 1);
    }

    #[test]
    fn full_store_is_refused_like_upstream_stop() {
        assert_eq!(run(3, 4).unwrap_err(), ReleaseError::TooManyParticles);
    }

    #[test]
    fn static_read_past_the_grid_is_zero_or_aliases() {
        let m = flat_grid();
        let ext = StaticExtents {
            nxmax: 4,
            nymax: 4,
            nzmax: 3,
        };
        // ix == nxmax aliases onto (0, jy + 1).
        assert_eq!(ext.read3(&m, &m.rho, 4, 0, 0, 0), Some(1.0));
        let wide = StaticExtents {
            nxmax: 5,
            nymax: 4,
            nzmax: 3,
        };
        assert_eq!(wide.read3(&m, &m.rho, 4, 0, 0, 0), Some(0.0));
        assert_eq!(ext.read3(&m, &m.rho, -1, 0, 0, 0), None);
    }
}
