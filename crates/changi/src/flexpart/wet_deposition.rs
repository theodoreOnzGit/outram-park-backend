// SPDX-License-Identifier: GPL-3.0
//
// FLEXPART port — provenance
// --------------------------
// Upstream project : FLEXPART (NILU) — https://github.com/flexpart/flexpart
// Upstream version : 10.4 (2019-11-12), commit 3d7eebf
// Upstream source  : src/interpol_rain.f90, src/interpol_rain_nests.f90,
//                    src/get_wetscav.f90, src/wetdepo.f90,
//                    src/wetdepokernel.f90, src/wetdepokernel_nest.f90
// Original licence : GPL-3.0-or-later — SPDX-FileCopyrightText: FLEXPART 1998-2019
//                    (A. Stohl, P. Seibert, S. Eckhardt, H. Grythe (ZHG),
//                    N. Kristiansen (NIK), I. Pisso (IP), E. Solbakken (ESO))
// Ported into this GPL-3.0 work; see LICENSE.flexpart and NOTICE.flexpart.

//! Wet deposition: precipitation interpolation, below- and in-cloud
//! scavenging coefficients, the per-particle mass loss and its attribution
//! to the output grid.
//!
//! | Function | Upstream | Computes |
//! |---|---|---|
//! | [`interpol_rain`] | `interpol_rain.f90`, `interpol_rain_nests.f90` | bilinear `lsp`, `convp`, `tcc` at a point |
//! | [`get_wetscav`] | `get_wetscav.f90` | scavenging coefficient `Lambda`, 1/s, and the precipitating fraction |
//! | [`wetdepo`] | `wetdepo.f90` | mass removed from every particle, gridded |
//! | [`wetdepokernel`] | `wetdepokernel.f90`, `wetdepokernel_nest.f90` | uniform-kernel attribution to the output grid |
//!
//! Each `_nests` routine is a copy of the mother-grid routine on the nest
//! arrays; the port has one function that takes the grid, and is verified
//! against both. `wetdepokernel_nest` differs in two ways, selected by
//! [`KernelRule`]: it uses `floor` where the mother uses `int` (truncation),
//! and it always applies the kernel (it ignores `lusekerneloutput`).
//!
//! # Inputs upstream reads from `com_mod`
//!
//! Precipitation, cloud cover and cloud codes, temperature, cloud water,
//! level heights, the nest frames, species parameters and the particle
//! arrays. The port takes them as arguments ([`WetScavMet`],
//! [`WetScavSpecies`], [`ParticleRecord`]). No random numbers are drawn.
//!
//! # Upstream quirks reproduced (each documented where it applies)
//!
//! - `interpol_rain` **moves its `xt`, `yt` arguments in place** when they
//!   reach the last grid line; [`RainAtPoint`] returns the moved values. It
//!   does no temporal interpolation ("skip to be consistent with clouds"):
//!   it reads the single field nearest in time.
//! - `get_wetscav` picks the field by `nint(itime - 0.5 ltsample)` (half
//!   away from zero), the earlier one only if strictly closer.
//! - `get_wetscav` **sets a negative `ccn_aero` or `in_aero` to zero in
//!   `com_mod`** the first time a particle of that species is in cloud: the
//!   port takes the species `&mut` and does the same.
//! - Rain vs snow below cloud: a species with `crain_aero <= 0` but
//!   `csnow_aero > 0` above 273 K (or the reverse below) is **counted** as
//!   below-cloud scavenging but scavenged at `Lambda = 0`.
//! - `liq_frac + ice_frac != 1` between 253 and 273 K:
//!   `ice = ((T-273)/20)^2`, `liq = max(0, 1 - ice)` (upstream's v10.4 fix).
//! - The cloud height `cloudsh` is read and never used.
//! - `wetdepo`: see [`wetdepo`] for the stale-deposit and age-class defects.
//!
//! # Units
//!
//! Precipitation mm/h, temperatures K, `dquer` µm, masses as the caller's
//! (kg), times s, grid coordinates in grid units.

use super::constants::R_AIR;
use super::met_fields::{Field2, Field3};

/// Upstream's in-cloud scavenging ratio, `par_mod.f90` (`incloud_ratio`).
pub const INCLOUD_RATIO: f64 = 6.2;

/// What [`interpol_rain`] returns.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RainAtPoint {
    /// Large-scale precipitation, mm/h.
    pub lsp: f64,
    /// Convective precipitation, mm/h.
    pub convp: f64,
    /// Total cloud cover, fraction.
    pub cc: f64,
    /// `xt` after upstream's in-place move off the last grid line.
    pub xt: f64,
    /// `yt` after the same move.
    pub yt: f64,
}

/// `interpol_rain.f90` / `interpol_rain_nests.f90`: bilinear interpolation
/// of three 2-d fields at `(xt, yt)` (grid units of the grid the fields are
/// on).
///
/// A point on or beyond the last grid line is moved to `n-1 - 0.00001`
/// first (upstream modifies its argument). The lower-left index is
/// `int(xt)` — truncation toward zero — so a point in `(-1, 0)` is
/// extrapolated from cell 0, as upstream does.
///
/// # Returns
/// `None` for a point at or left of `-1` (or below), where upstream indexes
/// outside the array.
#[must_use]
pub fn interpol_rain(
    lsprec: &Field2,
    convprec: &Field2,
    tcc: &Field2,
    xt: f64,
    yt: f64,
) -> Option<RainAtPoint> {
    let (nx, ny) = (lsprec.nx, lsprec.ny);
    let mut xt = xt;
    let mut yt = yt;
    if xt >= (nx as f64 - 1.0) {
        xt = (nx as f64 - 1.0) - 0.00001;
    }
    if yt >= (ny as f64 - 1.0) {
        yt = (ny as f64 - 1.0) - 0.00001;
    }
    let ix = xt.trunc();
    let jy = yt.trunc();
    if !(ix >= 0.0 && jy >= 0.0) || nx < 2 || ny < 2 {
        return None;
    }
    let ddx = xt - ix;
    let ddy = yt - jy;
    let (ix, jy) = (ix as usize, jy as usize);
    let (ixp, jyp) = (ix + 1, jy + 1);
    let rddx = 1.0 - ddx;
    let rddy = 1.0 - ddy;
    let p1 = rddx * rddy;
    let p2 = ddx * rddy;
    let p3 = rddx * ddy;
    let p4 = ddx * ddy;
    let bilin = |f: &Field2| {
        p1 * f.at(ix, jy) + p2 * f.at(ixp, jy) + p3 * f.at(ix, jyp) + p4 * f.at(ixp, jyp)
    };
    Some(RainAtPoint {
        lsp: bilin(lsprec),
        convp: bilin(convprec),
        cc: bilin(tcc),
        xt,
        yt,
    })
}

/// Cloud codes on the model levels (`clouds` / `cloudsn`, `integer(kind=1)`):
/// 0 no cloud, 1 cloud without precipitation, 2/3 in-cloud (convective /
/// large-scale dominated), 4/5 below-cloud. Element `(ix, jy, k)` is
/// `data[ix + nx*(jy + ny*k)]`, `k` 0-based (upstream level `k+1`).
#[derive(Debug, Clone, PartialEq)]
pub struct CloudField {
    /// Points in x.
    pub nx: usize,
    /// Points in y.
    pub ny: usize,
    /// Levels.
    pub nz: usize,
    /// Codes.
    pub data: Vec<i8>,
}

impl CloudField {
    /// Code at `(ix, jy, k)`, `k` 0-based.
    #[must_use]
    pub fn at(&self, ix: usize, jy: usize, k: usize) -> i8 {
        self.data[ix + self.nx * (jy + self.ny * k)]
    }
}

/// The wet-deposition fields of one grid at one time.
#[derive(Debug, Clone, PartialEq)]
pub struct WetFields {
    /// Large-scale precipitation, mm/h (`lsprec`).
    pub lsprec: Field2,
    /// Convective precipitation, mm/h (`convprec`).
    pub convprec: Field2,
    /// Total cloud cover, fraction (`tcc`).
    pub tcc: Field2,
    /// Cloud codes (`clouds`).
    pub clouds: CloudField,
    /// Temperature on the model levels, K (`tt`).
    pub tt: Field3,
    /// Total cloud water content (`ctwc`), used only when clouds were read.
    pub ctwc: Field2,
}

/// One grid (mother or nest): the fields at the two times in memory, **in
/// time order** (upstream's `memind` indirection already applied: slot 0 is
/// the field valid at `memtime[0]`).
#[derive(Debug, Clone, PartialEq)]
pub struct WetGrid {
    /// Fields at `memtime[0]` and `memtime[1]`.
    pub slots: [WetFields; 2],
    /// Cloud water was read from the met input (`readclouds` /
    /// `readclouds_nest(l)`), so `ctwc` is used instead of the
    /// parameterisation.
    pub readclouds: bool,
}

/// Location of a nest in mother-grid coordinates (`xln`, `xrn`, `yln`,
/// `yrn`) and its refinement (`xresoln`, `yresoln`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NestFrame {
    /// Left edge, mother grid units.
    pub xl: f64,
    /// Right edge.
    pub xr: f64,
    /// Lower edge.
    pub yl: f64,
    /// Upper edge.
    pub yr: f64,
    /// Nest points per mother grid unit in x.
    pub xresol: f64,
    /// Nest points per mother grid unit in y.
    pub yresol: f64,
}

impl NestFrame {
    /// Upstream's nest selection: the highest-numbered nest whose open
    /// rectangle contains the point; `None` for the mother grid.
    #[must_use]
    pub fn select(nests: &[NestFrame], x: f64, y: f64) -> Option<usize> {
        (0..nests.len())
            .rev()
            .find(|&j| x > nests[j].xl && x < nests[j].xr && y > nests[j].yl && y < nests[j].yr)
    }
}

/// Everything `get_wetscav` reads from `com_mod` besides the species.
#[derive(Debug, Clone, PartialEq)]
pub struct WetScavMet {
    /// Validity times of the two fields in memory, s (`memtime`).
    pub memtime: [i64; 2],
    /// Height of each model level, m (`height(1:nz)`).
    pub height: Vec<f64>,
    /// Mother grid.
    pub mother: WetGrid,
    /// Nests, with their frames, in upstream's order.
    pub nests: Vec<(NestFrame, WetGrid)>,
}

/// Wet scavenging parameters of one species (`com_mod`, from the SPECIES
/// file).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WetScavSpecies {
    /// Mean particle diameter, µm (`dquer`); `<= 0` marks a gas.
    pub dquer: f64,
    /// Below-cloud gas coefficient `A`, 1/s (`weta_gas`).
    pub weta_gas: f64,
    /// Below-cloud gas exponent `B` (`wetb_gas`).
    pub wetb_gas: f64,
    /// Below-cloud rain efficiency for aerosol (`crain_aero`).
    pub crain_aero: f64,
    /// Below-cloud snow efficiency for aerosol (`csnow_aero`).
    pub csnow_aero: f64,
    /// In-cloud CCN efficiency (`ccn_aero`); negative = off.
    pub ccn_aero: f64,
    /// In-cloud ice-nuclei efficiency (`in_aero`); negative = off.
    pub in_aero: f64,
    /// Henry's constant, M/atm (`henry`).
    pub henry: f64,
}

/// Which counter a scavenging event incremented.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScavengingRegime {
    /// `blc_count`.
    BelowCloud,
    /// `inc_count`.
    InCloud,
}

/// What [`get_wetscav`] returns.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WetScav {
    /// Scavenging coefficient, 1/s (`wetscav`); 0 when nothing applies.
    pub wetscav: f64,
    /// Precipitating fraction of the cell (`grfraction(1)`); `None` where
    /// upstream returns before assigning it.
    pub grfraction: Option<f64>,
    /// The counter upstream incremented, if any.
    pub counted: Option<ScavengingRegime>,
}

/// Why a wet-deposition routine refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WetDepError {
    /// The particle is at or above the top level `height(nz)` while it is
    /// raining; upstream's level index `hz` is then never assigned.
    AboveTopLevel,
    /// The particle's cell lies outside the field arrays.
    OutsideGrid,
    /// `wetdepo` would grid a deposit it never computed (a species with
    /// `WETDEPSPEC` false, forward run): upstream reads a stale local.
    UndefinedDeposit,
    /// The particle is older than the last age class; upstream's `nage` is
    /// `nageclass + 1` and the kernel writes outside the grid array.
    AgeBeyondLastClass,
    /// A release point or uncertainty class outside the deposition grid.
    ClassOutOfRange,
}

/// The field slot (0 = earlier) `get_wetscav` and `ohreaction` read:
/// `nint(itime - 0.5 ltsample)`, the earlier field only if strictly closer.
#[must_use]
pub fn nearest_field(itime: i64, ltsample: i64, memtime: [i64; 2]) -> usize {
    let interp_time = (itime as f64 - 0.5 * ltsample as f64).round() as i64;
    if (memtime[0] - interp_time).abs() < (memtime[1] - interp_time).abs() {
        0
    } else {
        1
    }
}

/// Upstream's particle level: `hz` such that `height(hz+1) > z`, the first
/// from level 2 upward (1-based). `None` at or above the top level.
fn level_below(height: &[f64], z: f64) -> Option<usize> {
    (2..=height.len())
        .find(|&il| height[il - 1] > z)
        .map(|il| il - 1)
}

/// `get_wetscav.f90`: scavenging coefficient of species `species` for a
/// particle at `(xtra, ytra)` (mother grid units) and height `ztra` (m).
///
/// 1. Grid: the highest nest containing the point, else the mother grid.
/// 2. Field: the one nearest `itime - ltsample/2` ([`nearest_field`]).
/// 3. `lsp`, `convp`, `cc` by [`interpol_rain`]; below 0.01 mm/h each,
///    nothing happens (`wetscav = 0`, `grfraction` unassigned).
/// 4. Cloud code at the particle's level (cell `int(x), int(y)`, not the
///    interpolation cell); `<= 1`: nothing.
/// 5. `grfraction = max(0.05, cc (lsp lfr + convp cfr)/(lsp + convp))` with
///    upstream's rate classes, and the sub-grid rate `prec = (lsp+convp)/grfraction`.
/// 6. Below cloud (code >= 4): gas `A prec^B`; aerosol rain (`T >= 273`)
///    Laakso et al. (2003) or snow Kyrö et al. (2009) polynomial in
///    `log10(d)`, `d = min(10 µm, dquer)`.
///    In cloud (code 2, 3): `incloud_ratio * S_i * prec/3.6e6` with `S_i`
///    the activated fraction over the cloud water (aerosol) or the
///    Henry-law partitioning (gas); cloud water from `ctwc` when read, else
///    `0.2 prec^0.36`.
///
/// `species` is `&mut` because upstream zeroes a negative `ccn_aero` /
/// `in_aero` in `com_mod` on the in-cloud path; the port does the same.
///
/// # Errors
/// [`WetDepError::AboveTopLevel`] where upstream reads an unassigned level
/// index; [`WetDepError::OutsideGrid`] where it would index outside its
/// arrays.
#[allow(clippy::too_many_arguments)]
pub fn get_wetscav(
    itime: i64,
    ltsample: i64,
    xtra: f64,
    ytra: f64,
    ztra: f64,
    species: &mut WetScavSpecies,
    met: &WetScavMet,
) -> Result<WetScav, WetDepError> {
    const LFR: [f64; 5] = [0.5, 0.65, 0.8, 0.9, 0.95];
    const CFR: [f64; 5] = [0.4, 0.55, 0.7, 0.8, 0.9];
    // Laakso et al. (2003) rain, Kyro et al. (2009) snow
    #[allow(clippy::excessive_precision)]
    const BCLR: [f64; 6] = [
        274.35758,
        332839.59273,
        226656.57259,
        58005.91340,
        6588.38582,
        0.244984,
    ];
    const BCLS: [f64; 6] = [22.7, 0.0, 0.0, 1321.0, 381.0, 0.0];

    let none = WetScav {
        wetscav: 0.0,
        grfraction: None,
        counted: None,
    };
    let nests: Vec<NestFrame> = met.nests.iter().map(|(f, _)| *f).collect();
    let ngrid = NestFrame::select(&nests, xtra, ytra);
    let (grid, xt, yt) = match ngrid {
        Some(l) => {
            let f = &nests[l];
            (
                (&met.nests[l].1),
                (xtra - f.xl) * f.xresol,
                (ytra - f.yl) * f.yresol,
            )
        }
        None => (&met.mother, xtra, ytra),
    };
    let ix = xt.trunc();
    let jy = yt.trunc();
    let n = nearest_field(itime, ltsample, met.memtime);
    let fields = &grid.slots[n];
    let rain = interpol_rain(&fields.lsprec, &fields.convprec, &fields.tcc, xt, yt)
        .ok_or(WetDepError::OutsideGrid)?;
    let (lsp, convp, cc) = (rain.lsp, rain.convp, rain.cc);

    if lsp < 0.01 && convp < 0.01 {
        return Ok(none);
    }
    let hz = level_below(&met.height, ztra).ok_or(WetDepError::AboveTopLevel)?;
    if !(ix >= 0.0 && jy >= 0.0)
        || ix as usize >= fields.tt.nx
        || jy as usize >= fields.tt.ny
        || hz > fields.tt.nz
    {
        return Err(WetDepError::OutsideGrid);
    }
    let (ix, jy) = (ix as usize, jy as usize);
    let clouds_v = fields.clouds.at(ix, jy, hz - 1);
    if clouds_v <= 1 {
        return Ok(none);
    }

    let class = |rate: f64| {
        if rate > 20.0 {
            4
        } else if rate > 8.0 {
            3
        } else if rate > 3.0 {
            2
        } else if rate > 1.0 {
            1
        } else {
            0
        }
    };
    let (i, j) = (class(lsp), class(convp));
    let grfraction = 0.05_f64.max(cc * (lsp * LFR[i] + convp * CFR[j]) / (lsp + convp));
    let prec = (lsp + convp) / grfraction;
    let act_temp = fields.tt.at(ix, jy, hz - 1);
    let mut out = WetScav {
        wetscav: 0.0,
        grfraction: Some(grfraction),
        counted: None,
    };
    let sp = species;

    // BELOW CLOUD
    if clouds_v >= 4 {
        if sp.dquer <= 0.0 && (sp.weta_gas > 0.0 || sp.wetb_gas > 0.0) {
            out.counted = Some(ScavengingRegime::BelowCloud);
            out.wetscav = sp.weta_gas * prec.powf(sp.wetb_gas);
        } else if sp.dquer > 0.0 && (sp.crain_aero > 0.0 || sp.csnow_aero > 0.0) {
            out.counted = Some(ScavengingRegime::BelowCloud);
            let dquer_m = 10.0_f64.min(sp.dquer) / 1_000_000.0;
            let l = dquer_m.log10();
            // gfortran expands x**(-n) for a constant n as 1/x^n, x^n by
            // repeated squaring (powi)
            let l2 = l * l;
            let m4 = 1.0 / (l2 * l2);
            let m3 = 1.0 / (l2 * l);
            let m2 = 1.0 / l2;
            let m1 = 1.0 / l;
            let poly = |b: &[f64; 6]| {
                b[0] + b[1] * m4 + b[2] * m3 + b[3] * m2 + b[4] * m1 + b[5] * prec.powf(0.5)
            };
            if act_temp >= 273.0 && sp.crain_aero > 0.0 {
                out.wetscav = sp.crain_aero * 10.0_f64.powf(poly(&BCLR));
            } else if act_temp < 273.0 && sp.csnow_aero > 0.0 {
                out.wetscav = sp.csnow_aero * 10.0_f64.powf(poly(&BCLS));
            }
        }
    }

    // IN CLOUD
    if clouds_v < 4
        && ((sp.ccn_aero > 0.0 || sp.in_aero > 0.0) || (sp.henry > 0.0 && sp.dquer <= 0.0))
    {
        out.counted = Some(ScavengingRegime::InCloud);
        // upstream zeroes the com_mod values
        if sp.ccn_aero < 0.0 {
            sp.ccn_aero = 0.0;
        }
        if sp.in_aero < 0.0 {
            sp.in_aero = 0.0;
        }
        let cl = if grid.readclouds {
            fields.ctwc.at(ix, jy) * (grfraction / cc)
        } else {
            1.0e6 * 2.0e-7 * prec.powf(0.36)
        };
        let (liq_frac, ice_frac) = if act_temp <= 253.0 {
            (0.0, 1.0)
        } else if act_temp >= 273.0 {
            (1.0, 0.0)
        } else {
            let r = (act_temp - 273.0) / (273.0 - 253.0);
            let ice = r * r;
            (0.0_f64.max(1.0 - ice), ice)
        };
        let frac_act = liq_frac * sp.ccn_aero + ice_frac * sp.in_aero;
        let s_i = if sp.dquer > 0.0 {
            frac_act / cl
        } else {
            let cle = (1.0 - cl) / (sp.henry * (R_AIR / 3500.0) * act_temp) + cl;
            1.0 / cle
        };
        out.wetscav = INCLOUD_RATIO * s_i * (prec / 3.6e6);
    }
    Ok(out)
}

/// A particle as `wetdepo` and `ohreaction` see it (`com_mod`'s particle
/// arrays at one index).
#[derive(Debug, Clone, PartialEq)]
pub struct ParticleRecord {
    /// Time of the particle, s (`itra1`); `-999999999` = inactive.
    pub itra1: i64,
    /// Release time, s (`itramem`).
    pub itramem: i64,
    /// Position, mother grid units (`xtra1`, `ytra1`, `real(kind=dp)`).
    pub xtra1: f64,
    /// See [`ParticleRecord::xtra1`].
    pub ytra1: f64,
    /// Height above ground, m (`ztra1`).
    pub ztra1: f64,
    /// Release point, 1-based (`npoint`).
    pub npoint: usize,
    /// Uncertainty class, 1-based (`nclass`).
    pub nclass: usize,
    /// Mass per species (`xmass1(jpart,:)`).
    pub xmass1: Vec<f64>,
}

/// Accumulated deposition on an output grid (`wetgridunc` /
/// `wetgriduncn`): `(ix, jy, species, release point, uncertainty class, age
/// class)`, `ix` fastest, all stored 0-based.
///
/// Stored as `f32` because upstream declares these grids `real(dep_prec)`
/// with `dep_prec = sp` in `par_mod.f90` — single precision even in a
/// `-fdefault-real-8` build. Each update is computed in `f64` and rounded
/// once on store, as the real(8) build does.
#[derive(Debug, Clone, PartialEq)]
pub struct DepositionGrid {
    /// `numxgrid`.
    pub nx: usize,
    /// `numygrid`.
    pub ny: usize,
    /// Species slots (`maxspec`).
    pub nspec: usize,
    /// Release-point slots (`maxpointspec_act`).
    pub npoint: usize,
    /// Uncertainty classes (`nclassunc`).
    pub nclassunc: usize,
    /// Age classes (`nageclass`).
    pub nageclass: usize,
    /// Values.
    pub data: Vec<f32>,
}

impl DepositionGrid {
    /// A zeroed grid.
    #[must_use]
    pub fn zeros(
        nx: usize,
        ny: usize,
        nspec: usize,
        npoint: usize,
        nclassunc: usize,
        nageclass: usize,
    ) -> Self {
        Self {
            nx,
            ny,
            nspec,
            npoint,
            nclassunc,
            nageclass,
            data: vec![0.0; nx * ny * nspec * npoint * nclassunc * nageclass],
        }
    }

    /// Flat index, all 0-based.
    #[must_use]
    pub fn index(
        &self,
        ix: usize,
        jy: usize,
        ks: usize,
        kp: usize,
        nunc: usize,
        nage: usize,
    ) -> usize {
        ix + self.nx
            * (jy
                + self.ny * (ks + self.nspec * (kp + self.npoint * (nunc + self.nclassunc * nage))))
    }

    #[allow(clippy::too_many_arguments)]
    fn add(&mut self, ix: i64, jy: i64, ks: usize, kp: usize, nunc: usize, nage: usize, v: f64) {
        if ix >= 0 && jy >= 0 && ix < self.nx as i64 && jy < self.ny as i64 {
            let i = self.index(ix as usize, jy as usize, ks, kp, nunc, nage);
            self.data[i] = (f64::from(self.data[i]) + v) as f32;
        }
    }
}

/// Geometry of an output grid relative to the met grid.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OutputFrame {
    /// Met (mother) grid spacing, degrees (`dx`, `dy`): the kernel converts
    /// mother grid units with these, also for the nested output grid.
    pub dx: f64,
    /// See [`OutputFrame::dx`].
    pub dy: f64,
    /// `xlon0 - outlon0` (or `...n`), degrees.
    pub xoutshift: f64,
    /// `ylat0 - outlat0`, degrees.
    pub youtshift: f64,
    /// Output grid spacing, degrees.
    pub dxout: f64,
    /// See [`OutputFrame::dxout`].
    pub dyout: f64,
}

/// Mother-grid or nested-output kernel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KernelRule {
    /// `wetdepokernel.f90`: cell index by `int` (truncation toward zero);
    /// `use_kernel = lusekerneloutput` selects the 4-point kernel or direct
    /// attribution to the cell.
    Mother {
        /// `lusekerneloutput`.
        use_kernel: bool,
    },
    /// `wetdepokernel_nest.f90`: cell index by `floor`, always the kernel.
    Nest,
}

/// `wetdepokernel.f90` / `wetdepokernel_nest.f90`: add `deposit[ks]` of a
/// particle at `(x, y)` (mother grid units) to the output grid with a
/// uniform kernel one output cell wide.
///
/// `xl = (x dx + xoutshift)/dxout`; the cell is `int(xl)` (mother) or
/// `floor(xl)` (nest); the neighbour is to the right if `ddx > 0.5`, else to
/// the left, with weight `wx = 1.5 - ddx` or `0.5 + ddx` on the own cell.
/// Points outside the grid are dropped.
///
/// **Upstream defect, reproduced:** with truncation (mother grid) a point
/// with `xl` in `(-1, -0.5]` gets `ix = 0` and `ddx < 0`, so `wx < 0.5`
/// and possibly negative: deposition is attributed with a **negative**
/// weight to cell 0 and a weight above 1 to cell `-1` (dropped). The nest
/// version's `floor` was introduced upstream (ESO) to fix exactly this.
///
/// `kp`, `nunc`, `nage` are upstream's 1-based class numbers.
///
/// # Errors
/// [`WetDepError::AgeBeyondLastClass`] or [`WetDepError::ClassOutOfRange`]
/// where upstream writes outside the array.
#[allow(clippy::too_many_arguments)]
pub fn wetdepokernel(
    rule: KernelRule,
    frame: &OutputFrame,
    x: f64,
    y: f64,
    deposit: &[f64],
    grid: &mut DepositionGrid,
    kp: usize,
    nunc: usize,
    nage: usize,
) -> Result<(), WetDepError> {
    if nage < 1 || nage > grid.nageclass {
        return Err(WetDepError::AgeBeyondLastClass);
    }
    if kp < 1 || kp > grid.npoint || nunc < 1 || nunc > grid.nclassunc || deposit.len() > grid.nspec
    {
        return Err(WetDepError::ClassOutOfRange);
    }
    let (kp, nunc, nage) = (kp - 1, nunc - 1, nage - 1);
    let xl = (x * frame.dx + frame.xoutshift) / frame.dxout;
    let yl = (y * frame.dy + frame.youtshift) / frame.dyout;
    let (ixf, jyf) = match rule {
        KernelRule::Mother { .. } => (xl.trunc(), yl.trunc()),
        KernelRule::Nest => (xl.floor(), yl.floor()),
    };
    let (ix, jy) = (ixf as i64, jyf as i64);
    let ddx = xl - ixf;
    let ddy = yl - jyf;
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

    if let KernelRule::Mother { use_kernel: false } = rule {
        for (ks, &d) in deposit.iter().enumerate() {
            grid.add(ix, jy, ks, kp, nunc, nage, d);
        }
        return Ok(());
    }
    for (ks, &d) in deposit.iter().enumerate() {
        grid.add(ix, jy, ks, kp, nunc, nage, d * (wx * wy));
        grid.add(ixp, jyp, ks, kp, nunc, nage, d * ((1.0 - wx) * (1.0 - wy)));
        grid.add(ixp, jy, ks, kp, nunc, nage, d * ((1.0 - wx) * wy));
        grid.add(ix, jyp, ks, kp, nunc, nage, d * (wx * (1.0 - wy)));
    }
    Ok(())
}

/// Run-level settings `wetdepo` reads from `com_mod`.
#[derive(Debug, Clone, PartialEq)]
pub struct WetDepoSettings {
    /// `+1` forward, `-1` backward (`ldirect`).
    pub ldirect: i64,
    /// Output interval, s (`loutstep`).
    pub loutstep: i64,
    /// Upper bounds of the age classes, s (`lage(1:nageclass)`).
    pub lage: Vec<i64>,
    /// `ioutputforeachrelease == 1`: grid by release point.
    pub ioutputforeachrelease: bool,
    /// Decay constant per species, 1/s (`decay`; `<= 0` = stable).
    pub decay: Vec<f64>,
    /// Wet deposition switched on per species (`WETDEPSPEC`).
    pub wetdepspec: Vec<bool>,
}

/// Below- and in-cloud event counters (`tot_blc_count`, `tot_inc_count`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScavCounts {
    /// Below-cloud events per species.
    pub blc: Vec<i64>,
    /// In-cloud events per species.
    pub inc: Vec<i64>,
}

/// An output grid with its frame and kernel rule.
#[derive(Debug, Clone, PartialEq)]
pub struct WetOutputGrid {
    /// Accumulated deposition.
    pub grid: DepositionGrid,
    /// Geometry.
    pub frame: OutputFrame,
    /// Kernel variant.
    pub rule: KernelRule,
}

/// `wetdepo.f90`: wet deposition of every active particle over the interval
/// `ltsample` ending at `itime`.
///
/// For each particle active at `itime` (`itra1 != -999999999` and not
/// released after `itime` in the run direction), its age class, then for each
/// species with `WETDEPSPEC`: [`get_wetscav`];
/// `deposit = m (1 - exp(-Lambda |ltsample|)) grfraction` when `Lambda > 0`;
/// the particle keeps `m - deposit` (or 0 if that is not above `tiny`); the
/// deposit is corrected for the decay since the last output,
/// `exp(|ldeltat| lambda)`. In a forward run the deposits are gridded with
/// [`wetdepokernel`] on the mother output grid and, if present, the nested
/// one. Counters are added to `counts`.
///
/// `tiny` is `f64::MIN_POSITIVE`, the `tiny(0.0)` of the real(8) build (the
/// shipped real(4) build uses `1.18e-38`).
///
/// # Upstream defects, refused rather than guessed
/// - A species with `WETDEPSPEC` false is skipped (`cycle`) **without
///   assigning its `wetdeposit`**, yet in a forward run the kernel grids
///   `wetdeposit(ks)` for every species: a stale value from an earlier
///   particle or call. The port returns [`WetDepError::UndefinedDeposit`].
/// - A particle at least `lage(nageclass)` old falls through the age loop
///   with `nage = nageclass + 1`, which the kernel uses as an array index
///   ([`WetDepError::AgeBeyondLastClass`]). FLEXPART normally removes such
///   particles earlier; nothing here guards it.
///
/// On error, particles before the failing one have already been updated.
///
/// # Errors
/// As above, and those of [`get_wetscav`] and [`wetdepokernel`].
#[allow(clippy::too_many_arguments)]
pub fn wetdepo(
    itime: i64,
    ltsample: i64,
    loutnext: i64,
    particles: &mut [ParticleRecord],
    species: &mut [WetScavSpecies],
    met: &WetScavMet,
    settings: &WetDepoSettings,
    mother_out: &mut WetOutputGrid,
    nested_out: Option<&mut WetOutputGrid>,
    counts: &mut ScavCounts,
) -> Result<(), WetDepError> {
    let nspec = species.len();
    let ldeltat = if itime <= loutnext {
        itime - (loutnext - settings.loutstep)
    } else {
        itime - loutnext
    };
    let mut blc = vec![0_i64; nspec];
    let mut inc = vec![0_i64; nspec];
    let mut nested_out = nested_out;

    for p in particles.iter_mut() {
        if p.itra1 == -999_999_999 {
            continue;
        }
        if settings.ldirect == 1 {
            if p.itra1 > itime {
                continue;
            }
        } else if p.itra1 < itime {
            continue;
        }
        let itage = (p.itra1 - p.itramem).abs();
        let nage = settings
            .lage
            .iter()
            .position(|&l| itage < l)
            .map_or(settings.lage.len() + 1, |i| i + 1);

        let mut wetdeposit: Vec<Option<f64>> = vec![None; nspec];
        let kp = if settings.ioutputforeachrelease {
            p.npoint
        } else {
            1
        };
        for ks in 0..nspec {
            if !settings.wetdepspec[ks] {
                continue;
            }
            let ws = get_wetscav(
                itime,
                ltsample,
                p.xtra1,
                p.ytra1,
                p.ztra1,
                &mut species[ks],
                met,
            )?;
            match ws.counted {
                Some(ScavengingRegime::BelowCloud) => blc[ks] += 1,
                Some(ScavengingRegime::InCloud) => inc[ks] += 1,
                None => {}
            }
            let mut dep = if ws.wetscav > 0.0 {
                let g = ws.grfraction.expect("assigned whenever wetscav > 0");
                p.xmass1[ks] * (1.0 - (-ws.wetscav * ltsample.abs() as f64).exp()) * g
            } else {
                0.0
            };
            let restmass = p.xmass1[ks] - dep;
            p.xmass1[ks] = if restmass > f64::MIN_POSITIVE {
                restmass
            } else {
                0.0
            };
            if settings.decay[ks] > 0.0 {
                dep *= (ldeltat.abs() as f64 * settings.decay[ks]).exp();
            }
            wetdeposit[ks] = Some(dep);
        }

        if settings.ldirect == 1 {
            let deposit: Vec<f64> = wetdeposit
                .iter()
                .map(|d| d.ok_or(WetDepError::UndefinedDeposit))
                .collect::<Result<_, _>>()?;
            let o = &mut *mother_out;
            wetdepokernel(
                o.rule,
                &o.frame,
                p.xtra1,
                p.ytra1,
                &deposit,
                &mut o.grid,
                kp,
                p.nclass,
                nage,
            )?;
            if let Some(n) = nested_out.as_deref_mut() {
                wetdepokernel(
                    n.rule,
                    &n.frame,
                    p.xtra1,
                    p.ytra1,
                    &deposit,
                    &mut n.grid,
                    kp,
                    p.nclass,
                    nage,
                )?;
            }
        }
    }
    for ks in 0..nspec {
        counts.blc[ks] += blc[ks];
        counts.inc[ks] += inc[ks];
    }
    Ok(())
}
