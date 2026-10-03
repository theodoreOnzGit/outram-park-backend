// SPDX-License-Identifier: GPL-3.0
//
// FLEXPART port — provenance
// --------------------------
// Upstream project : FLEXPART (NILU) — https://github.com/flexpart/flexpart
// Upstream version : 10.4 (2019-11-12), commit 3d7eebf
// Upstream source  : src/ohreaction.f90, src/gethourlyOH.f90 (data layout
//                    from src/oh_mod.f90)
// Original licence : GPL-3.0-or-later — SPDX-FileCopyrightText: FLEXPART 1998-2019
//                    (R. L. Thompson)
// Ported into this GPL-3.0 work; see LICENSE.flexpart and NOTICE.flexpart.

//! OH reaction: hourly OH fields scaled from a monthly climatology by the
//! O(¹D) photolysis rate, and the first-order loss of particle mass.
//!
//! | Function | Upstream | Computes |
//! |---|---|---|
//! | [`gethourly_oh`] | `gethourlyOH.f90` | the two hourly OH fields bracketing `itime` |
//! | [`ohreaction`] | `ohreaction.f90` | particle mass after reaction with OH |
//!
//! The climatology (`OH_field`, `jrate_average`, the coordinate vectors) is
//! **data**, read upstream from `OH_variables.bin` by `readOHfield.f90`; the
//! port takes it as arguments ([`OhClimatology`], [`JrateClimatology`]) and
//! ships none. The solar zenith angle and `J(O1D)` come from the already
//! verified [`super::solar`]; the month from [`super::calendar::caldate`].
//!
//! # Upstream quirks reproduced
//!
//! - **`gethourlyOH` advances by one hour per call at most.** If `itime` has
//!   moved more than an hour past the second field, it shifts the pair by one
//!   hour only, so the fields lag the model time until enough calls have
//!   been made. When neither "in range" nor "advance" applies (first call:
//!   `memOHtime = (0, 0)`), it rebuilds both fields for **`t = 0` and
//!   `t = ldirect*3600`**, whatever `itime` is.
//! - `gethourlyOH`'s initial second field is at `bdate + ldirect*real(1./24.,
//!   kind=dp)`: the `1./24.` is a default real, so the shipped real(4) build
//!   places it `1.3e-9` days (0.1 ms) late; the port uses the `f64` value of
//!   the real(8) build.
//! - `memOHtime` is default `real`: in the shipped build the hour arithmetic
//!   and the `itime` comparisons are single precision (exact for integral
//!   hours below 2^24 s, ~194 days; spacing grows beyond).
//! - **`ohreaction` reacts every particle**, with no check of `itra1`: an
//!   inactive (`-999999999`) or not yet released particle loses mass too.
//! - **`ohreaction` indexes the temperature with the time-order index `n`
//!   (1 or 2) as the storage slot**, not with `memind(n)` as `get_wetscav`
//!   does. When `memind` is swapped, it reads the other time's temperature.
//!   The port takes `tt` by storage slot ([`OhMet::tt_slots`]) and reproduces
//!   this.
//! - **On a nest, `ohreaction` computes `ix, jy` in nest coordinates and uses
//!   them to index the mother-grid `tt`.** Reproduced (the port refuses with
//!   [`OhError::OutsideGrid`] only where that leaves the array).
//! - **`altOHtop(i-1) = altOH(i) + (altOH(i) - altOH(i-1))/2`**: the "top" of
//!   level `i-1` is placed half a layer **above level `i`**, i.e. one level
//!   too high; the OH level is then the one whose shifted "top" is nearest
//!   the particle (`minloc`, first on ties).
//! - The month `m`, hour `h`, `ldeltat` and `ohreacted` that `ohreaction`
//!   computes are never used.
//!
//! # Units
//!
//! OH in molecules/cm³, rate constants as in the SPECIES file
//! (`k = C T^N exp(-D/T)` in cm³/molecule/s), temperatures K, times s.

use super::calendar::caldate;
use super::met_fields::Field3;
use super::solar::{photo_o1d, zenithangle};
use super::wet_deposition::{nearest_field, NestFrame, ParticleRecord};

/// Monthly OH climatology (`oh_mod`: `lonOH`, `latOH`, `altOH`, `OH_field`).
#[derive(Debug, Clone, PartialEq)]
pub struct OhClimatology {
    /// Longitudes, degrees (`lonOH`, `nxOH` values).
    pub lon: Vec<f64>,
    /// Latitudes, degrees (`latOH`).
    pub lat: Vec<f64>,
    /// Level heights above orography, m (`altOH`).
    pub alt: Vec<f64>,
    /// `OH_field(ix, jy, kz, month)`, molecules/cm³, `ix` fastest:
    /// index `ix + nx*(jy + ny*(kz + nz*(month-1)))`, all 0-based except month.
    pub field: Vec<f64>,
}

impl OhClimatology {
    fn dims(&self) -> (usize, usize, usize) {
        (self.lon.len(), self.lat.len(), self.alt.len())
    }

    /// Flat index of `(ix, jy, kz)` (0-based) in one hourly field.
    #[must_use]
    pub fn index3(&self, ix: usize, jy: usize, kz: usize) -> usize {
        let (nx, ny, _) = self.dims();
        ix + nx * (jy + ny * kz)
    }

    fn month_value(&self, ix: usize, jy: usize, kz: usize, month: usize) -> f64 {
        let (nx, ny, nz) = self.dims();
        self.field[ix + nx * (jy + ny * (kz + nz * (month - 1)))]
    }
}

/// Monthly mean `J(O1D)` on its own 360 x 180 grid (`oh_mod`: `lonjr`,
/// `latjr`, `jrate_average`).
#[derive(Debug, Clone, PartialEq)]
pub struct JrateClimatology {
    /// Longitudes, degrees (`lonjr`, 360 values).
    pub lon: Vec<f64>,
    /// Latitudes, degrees (`latjr`, 180 values).
    pub lat: Vec<f64>,
    /// `jrate_average(i, j, month)`, 1/s: index `i + nlon*(j + nlat*(month-1))`.
    pub average: Vec<f64>,
}

impl JrateClimatology {
    fn at(&self, i: usize, j: usize, month: usize) -> f64 {
        let (nlon, nlat) = (self.lon.len(), self.lat.len());
        self.average[i + nlon * (j + nlat * (month - 1))]
    }
}

/// The two hourly OH fields in memory (`OH_hourly(:,:,:,1:2)`) and their
/// times (`memOHtime`, s relative to `bdate`). Start from
/// [`HourlyOh::empty`], which is upstream's zero-initialised module state.
#[derive(Debug, Clone, PartialEq)]
pub struct HourlyOh {
    /// `memOHtime(1:2)`, s.
    pub memtime: [f64; 2],
    /// The two fields, each indexed by [`OhClimatology::index3`].
    pub hourly: [Vec<f64>; 2],
}

impl HourlyOh {
    /// No fields yet: `memOHtime = (0, 0)`.
    #[must_use]
    pub fn empty(clim: &OhClimatology) -> Self {
        let (nx, ny, nz) = clim.dims();
        Self {
            memtime: [0.0, 0.0],
            hourly: [vec![0.0; nx * ny * nz], vec![0.0; nx * ny * nz]],
        }
    }
}

/// Fortran `minloc(abs(v - x), mask = abs(v - x) == minval(...))`: the first
/// index (0-based) of the smallest distance.
fn nearest_first(v: &[f64], x: f64) -> usize {
    let mut best = 0;
    let mut dmin = f64::INFINITY;
    for (i, &vi) in v.iter().enumerate() {
        let d = (vi - x).abs();
        if d < dmin {
            dmin = d;
            best = i;
        }
    }
    best
}

fn month_of(jul: f64) -> usize {
    let (yyyymmdd, _) = caldate(jul);
    ((yyyymmdd - (yyyymmdd / 10_000) * 10_000) / 100) as usize
}

/// One hourly field: `OH_field(month) * J(O1D)(jul) / jrate_average(month)`,
/// or 0 where the average is not positive.
fn hourly_field(clim: &OhClimatology, jr: &JrateClimatology, jul: f64, month: usize) -> Vec<f64> {
    let (nx, ny, nz) = clim.dims();
    let mut out = vec![0.0; nx * ny * nz];
    for kz in 0..nz {
        for jy in 0..ny {
            for ix in 0..nx {
                let ijx = nearest_first(&jr.lon, clim.lon[ix]);
                let jjy = nearest_first(&jr.lat, clim.lat[jy]);
                let sza = zenithangle(clim.lat[jy], clim.lon[ix], jul);
                // zenithangle returns 90 - asin(.) in degrees, never negative
                let jrate = photo_o1d(sza).expect("solar zenith angle is non-negative");
                let avg = jr.at(ijx, jjy, month);
                out[clim.index3(ix, jy, kz)] = if avg > 0.0 {
                    clim.month_value(ix, jy, kz, month) * jrate / avg
                } else {
                    0.0
                };
            }
        }
    }
    out
}

/// `gethourlyOH.f90`: make sure the hourly OH fields in `state` bracket
/// `itime` (s after `bdate`, Julian days), in the run direction `ldirect`
/// (`+1` / `-1`).
///
/// - In range (`ldirect*t1 <= ldirect*itime < ldirect*t2`): nothing.
/// - Past the second field (and `t2 != 0`): shift by **one** hour,
///   `t1 = t2`, `t2 = t1 + ldirect*3600`, and compute the new second field.
/// - Otherwise: compute both, for `t = 0` and `t = ldirect*3600`.
///
/// See the module docs for the quirks this reproduces.
pub fn gethourly_oh(
    state: &mut HourlyOh,
    itime: i64,
    ldirect: i64,
    bdate: f64,
    clim: &OhClimatology,
    jr: &JrateClimatology,
) {
    let ld = ldirect as f64;
    let it = (ldirect * itime) as f64;
    if ld * state.memtime[0] <= it && ld * state.memtime[1] > it {
        // the right fields are in memory
    } else if ld * state.memtime[1] <= it && state.memtime[1] != 0.0 {
        state.memtime[0] = state.memtime[1];
        state.memtime[1] = state.memtime[0] + ld * 3600.0;
        state.hourly[0] = state.hourly[1].clone();
        let jul2 = bdate + state.memtime[1] / 86_400.0;
        let m2 = month_of(jul2);
        state.hourly[1] = hourly_field(clim, jr, jul2, m2);
    } else {
        let jul1 = bdate;
        let m1 = month_of(jul1);
        state.memtime[0] = 0.0;
        let jul2 = bdate + ld * (1.0 / 24.0);
        let m2 = month_of(jul2);
        state.memtime[1] = ld * 3600.0;
        state.hourly[0] = hourly_field(clim, jr, jul1, m1);
        state.hourly[1] = hourly_field(clim, jr, jul2, m2);
    }
}

/// OH rate constants of one species (`ohcconst`, `ohdconst`, `ohnconst`):
/// `k = C T^N exp(-D/T)`. A species with `C <= 0` does not react.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OhSpecies {
    /// `C`, cm³/molecule/s.
    pub ohcconst: f64,
    /// `D`, K.
    pub ohdconst: f64,
    /// `N`, dimensionless.
    pub ohnconst: f64,
}

/// What `ohreaction` reads from `com_mod` besides the particles.
#[derive(Debug, Clone, PartialEq)]
pub struct OhMet {
    /// Validity times of the two met fields, s (`memtime`).
    pub memtime: [i64; 2],
    /// Model level heights, m (`height(1:nz)`).
    pub height: Vec<f64>,
    /// Mother-grid temperature in **storage slots** 1 and 2 (`tt(:,:,:,1)`,
    /// `tt(:,:,:,2)`), indexed by the time-order `n` as upstream does.
    pub tt_slots: [Field3; 2],
    /// Mother grid spacing and origin, degrees (`dx`, `dy`, `xlon0`,
    /// `ylat0`).
    pub dx: f64,
    /// See [`OhMet::dx`].
    pub dy: f64,
    /// See [`OhMet::dx`].
    pub xlon0: f64,
    /// See [`OhMet::dx`].
    pub ylat0: f64,
    /// Nest frames (only the cell index depends on them).
    pub nests: Vec<NestFrame>,
}

/// Why [`ohreaction`] refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OhError {
    /// The particle is at or above the top model level while OH is present;
    /// upstream's level index `indz` is then never assigned.
    AboveTopLevel,
    /// The temperature lookup falls outside the mother-grid array.
    OutsideGrid,
}

/// `ohreaction.f90`: first-order loss of every particle's mass to OH over
/// `ltsample` (s) at `itime`.
///
/// Per particle: OH from the nearest climatology column (`minloc` on
/// longitude, wrapped to `(-180, 180]` once, and latitude) and the level by
/// upstream's shifted `altOHtop`, linearly interpolated in time between the
/// two hourly fields (and extrapolated outside them). If OH exceeds `tiny`,
/// each species with `C > 0` loses `m (1 - exp(-k OH |ltsample|))`, the
/// remaining mass set to 0 if not above `tiny`. `tiny` is
/// `f64::MIN_POSITIVE` (the real(8) build's `tiny(0.0)`).
///
/// See the module docs for the quirks this reproduces (no activity check,
/// time-order `tt` slot, nest indices into the mother grid).
///
/// # Errors
/// [`OhError::AboveTopLevel`], [`OhError::OutsideGrid`], each only where
/// upstream would actually read the undefined value. Particles before the
/// failing one have been updated.
#[allow(clippy::too_many_arguments)]
pub fn ohreaction(
    itime: i64,
    ltsample: i64,
    particles: &mut [ParticleRecord],
    species: &[OhSpecies],
    met: &OhMet,
    clim: &OhClimatology,
    hourly: &HourlyOh,
) -> Result<(), OhError> {
    let (_, _, nz_oh) = clim.dims();
    // altOHtop, upstream's (shifted) level tops
    let mut alt_top = vec![0.0; nz_oh];
    for i in 1..nz_oh {
        alt_top[i - 1] = clim.alt[i] + 0.5 * (clim.alt[i] - clim.alt[i - 1]);
    }
    alt_top[nz_oh - 1] = clim.alt[nz_oh - 1] + 0.5 * (clim.alt[nz_oh - 1] - clim.alt[nz_oh - 2]);

    for p in particles.iter_mut() {
        let (ix, jy) = match NestFrame::select(&met.nests, p.xtra1, p.ytra1) {
            Some(l) => {
                let f = &met.nests[l];
                (
                    ((p.xtra1 - f.xl) * f.xresol).trunc(),
                    ((p.ytra1 - f.yl) * f.yresol).trunc(),
                )
            }
            None => (p.xtra1.trunc(), p.ytra1.trunc()),
        };
        let n = nearest_field(itime, ltsample, met.memtime);
        let indz = (2..=met.height.len())
            .find(|&i| met.height[i - 1] > p.ztra1)
            .map(|i| i - 1);

        let mut xlon = p.xtra1 * met.dx + met.xlon0;
        if xlon > 180.0 {
            xlon -= 360.0;
        }
        let ylat = p.ytra1 * met.dy + met.ylat0;
        let ohx = nearest_first(&clim.lon, xlon);
        let ohy = nearest_first(&clim.lat, ylat);
        let ohz = nearest_first(&alt_top, p.ztra1);
        let i3 = clim.index3(ohx, ohy, ohz);
        let oh1 = hourly.hourly[0][i3];
        let oh2 = hourly.hourly[1][i3];
        let oh_average = oh1
            + (oh2 - oh1) * (itime as f64 - hourly.memtime[0])
                / (hourly.memtime[1] - hourly.memtime[0]);

        if oh_average > f64::MIN_POSITIVE {
            let indz = indz.ok_or(OhError::AboveTopLevel)?;
            let tt = &met.tt_slots[n];
            if !(ix >= 0.0 && jy >= 0.0)
                || ix as usize >= tt.nx
                || jy as usize >= tt.ny
                || indz > tt.nz
            {
                return Err(OhError::OutsideGrid);
            }
            let temp = tt.at(ix as usize, jy as usize, indz - 1);
            for (k, sp) in species.iter().enumerate() {
                if sp.ohcconst > 0.0 {
                    let ohrate = sp.ohcconst
                        * temp.powf(sp.ohnconst)
                        * (-(sp.ohdconst / temp)).exp()
                        * oh_average;
                    let restmass = p.xmass1[k] * (-ohrate * ltsample.abs() as f64).exp();
                    p.xmass1[k] = if restmass > f64::MIN_POSITIVE {
                        restmass
                    } else {
                        0.0
                    };
                }
            }
        }
    }
    Ok(())
}
