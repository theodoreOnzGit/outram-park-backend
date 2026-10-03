// SPDX-License-Identifier: GPL-3.0
//
// FLEXPART port — provenance
// --------------------------
// Upstream project : FLEXPART (NILU) — https://github.com/flexpart/flexpart
// Upstream version : 10.4 (2019-11-12), commit 3d7eebf
// Upstream source  : src/advance.f90, src/initialize.f90, src/get_vdep_prob.f90,
//                    src/random_mod.f90 (ONLY the +-3 limit in gasdev1, which
//                    is FLEXPART's own code; the Numerical Recipes generators
//                    around it are not ported)
// Original licence : GPL-3.0-or-later — SPDX-FileCopyrightText: FLEXPART 1998-2019
// Ported into this GPL-3.0 work; see LICENSE.flexpart and NOTICE.flexpart.

//! The Lagrangian particle step: `advance.f90` moves one particle through one
//! synchronisation interval, and `initialize.f90` gives a newly released
//! particle its initial turbulent velocities.
//!
//! # Random numbers are an INPUT
//!
//! Upstream draws from two sources: the pre-computed Gaussian array
//! `rannumb(maxrand)` in `com_mod`, and `ran3` (Numerical Recipes, in
//! `random_mod.f90`) once per call, to pick the starting index
//! `nrand = int(ran3 * (maxrand-1)) + 1`. Numerical Recipes code is not ported
//! and not re-implemented here (maintainer decision, gh:#410). So the port
//! takes both as arguments: `rannumb` as a slice (1-based in the comments,
//! `rannumb[n-1]` in code) and the starting index `nrand0`. Given the same
//! draws, the step is deterministic, and that is what the code-to-code test
//! verifies. Where the draws come from is the caller's choice: the stochastic
//! comparison uses `petir`'s LCG and RAFFLES' samplers. FLEXPART's own
//! `rannumb` is clipped to `[-3, 3]`; reproduce that with [`limit_rannumb`].
//!
//! # State shared through modules
//!
//! `advance` reads and writes `interpol_mod` ([`Interpolator`]) and
//! `hanna_mod` ([`HannaState`]), and both carry values from one particle to
//! the next. Two cases matter:
//! - the "already interpolated" flags are only reset for levels `1..nmixz`;
//! - `u`, `v`, `w` from the previous step feed the Petterssen correction.
//!
//! Pass the same two structs to successive calls, as upstream's call sequence
//! does.
//!
//! # Upstream quirks reproduced (each verified against the compiled Fortran)
//!
//! - After the `ifine` sub-step loop, `nrand = nrand + i` adds the loop
//!   variable's **exit** value, `ifine + 1`, not `ifine`.
//! - At a pole crossing, `xt = mod(xt + 180, 360)` is applied to a **grid**
//!   coordinate, as though it were a longitude in degrees.
//! - On a nest, `ddx = xt - ix` mixes the mother-grid `xt` with the nest index
//!   `ix`. The weights it sets are overwritten by `interpol_all_nests` before
//!   any wind is used, so the only effect is on `interpolhmix` (next item).
//! - With `interpolhmix` on a nest, `h` is computed from the array `h1`, which
//!   upstream never assigns on that branch. The port refuses
//!   ([`AdvanceError::UndefinedMixingHeight`]) rather than guess.
//! - `memindnext` is computed and never used. It is not ported.
//! - `tropop` and `get_settling` read memory slot **1** (`tropopause(...,1,1)`,
//!   `rho(...,1)`), not the time-interpolated field.
//! - In `initialize`, the draws used for `up`, `vp`, `wp` in the boundary
//!   layer are used **again** for `usigold`, `vsigold`, `wsigold`, because
//!   `nrand` is not advanced in between.
//!
//! # Units
//!
//! FLEXPART's: positions in grid units (`xt`, `yt`) and m (`zt`), velocities
//! m/s, times s.

use super::cbl::{cbl, initialize_cbl_vel, re_initialize_particle};
use super::cmapf::{cgszll, cll2xy, cxy2ll, Strcmp};
use super::constants::{HREF, PI180};
use super::dry_deposition::get_settling;
use super::interpolation::{Interpolator, MetFields};
use super::turbulence::{hanna, hanna1, hanna_short, windalign, HannaState};

/// The bound FLEXPART puts on every Gaussian draw in `rannumb`.
///
/// `random_mod.f90`'s `gasdev1`, which fills `rannumb` at start-up, clips each
/// deviate to `[-3, 3]` ("Limit the random numbers to lie within the interval
/// -3 and +3"). That is a modelling choice, not a property of the generator:
/// it lowers the variance of the draws to `0.99501` and so narrows every
/// turbulent velocity distribution FLEXPART samples by 0.5 %. Measured on the
/// compiled upstream: the mean square of a filled `rannumb` is `0.99534` over
/// 16 seeds (gh:#410, stage 7). A port fed unclipped `N(0, 1)` draws is
/// measurably more diffusive than FLEXPART, so fill `rannumb` through
/// [`limit_rannumb`].
pub const RANNUMB_LIMIT: f64 = 3.0;

/// One `rannumb` entry from a standard normal deviate, clipped as
/// `gasdev1` clips it (see [`RANNUMB_LIMIT`]). The generator is the caller's.
#[must_use]
pub fn limit_rannumb(x: f64) -> f64 {
    x.clamp(-RANNUMB_LIMIT, RANNUMB_LIMIT)
}

/// Run settings `advance` reads from `com_mod`.
#[derive(Debug, Clone, PartialEq)]
pub struct AdvanceSettings {
    /// `+1` forward, `-1` backward in time.
    pub ldirect: i64,
    /// Synchronisation interval, s (signed with `ldirect`).
    pub lsynctime: i64,
    /// `1`: time step from the Lagrangian time scale; otherwise one step per
    /// synchronisation interval.
    pub method: i32,
    /// Fraction of the Lagrangian time scale used as time step (`ctl`).
    pub ctl: f64,
    /// Number of vertical sub-steps (`ifine`).
    pub ifine: i64,
    /// `1 / ifine` as upstream stores it (`fine`).
    pub fine: f64,
    /// Minimum time step, s.
    pub mintime: i64,
    /// `true`: `hanna` + Gaussian scheme; `false`: `hanna1` + well-mixed.
    pub turbswitch: bool,
    /// Skewed CBL scheme (`cblflag == 1`).
    pub cblflag: bool,
    /// Turbulence switched off (`turboff`).
    pub turboff: bool,
    /// Dry deposition on (`DRYDEP`).
    pub drydep: bool,
    /// Per-species dry deposition (`DRYDEPSPEC`).
    pub drydepspec: Vec<bool>,
    /// Horizontal diffusivity in the troposphere, m²/s.
    pub d_trop: f64,
    /// Vertical diffusivity in the stratosphere, m²/s.
    pub d_strat: f64,
    /// Interval between wind fields, s.
    pub lwindinterv: i64,
    /// Mesoscale-turbulence factor.
    pub turbmesoscale: f64,
    /// Domain-filling mode (`0` = off).
    pub mdomainfill: i32,
    /// Gravitational settling on (`lsettling`).
    pub lsettling: bool,
    /// Interpolate the mixing height bilinearly instead of taking the corner
    /// maximum (`interpolhmix`).
    pub interpolhmix: bool,
    /// Number of levels whose profiles are re-interpolated each step
    /// (`nmixz`).
    pub nmixz: usize,
}

/// One nested grid's placement in mother-grid coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NestGeometry {
    /// Lower-left corner, mother-grid units.
    pub xln: f64,
    /// Lower-left corner, mother-grid units.
    pub yln: f64,
    /// Upper-right corner, mother-grid units.
    pub xrn: f64,
    /// Upper-right corner, mother-grid units.
    pub yrn: f64,
    /// Nest points per mother-grid unit.
    pub xresoln: f64,
    /// Nest points per mother-grid unit.
    pub yresoln: f64,
}

/// The model domain as `advance` sees it.
#[derive(Debug, Clone, PartialEq)]
pub struct Domain {
    /// `nx - 1`.
    pub nxmin1: i64,
    /// `ny - 1`.
    pub nymin1: i64,
    /// The compiled `nxmax` (array dimension), which sets upstream's
    /// `eps = nxmax / 3e5`.
    pub nxmax: i64,
    /// The compiled `nymax` (array dimension).
    pub nymax: i64,
    /// Global in longitude.
    pub xglobal: bool,
    /// Covers the north pole (use the polar grid north of `switchnorthg`).
    pub nglobal: bool,
    /// Covers the south pole.
    pub sglobal: bool,
    /// Grid `y` above which the north-polar grid is used.
    pub switchnorthg: f64,
    /// Grid `y` below which the south-polar grid is used.
    pub switchsouthg: f64,
    /// Grid spacing, degrees.
    pub dx: f64,
    /// Grid spacing, degrees.
    pub dy: f64,
    /// Longitude of the grid origin, degrees.
    pub xlon0: f64,
    /// Latitude of the grid origin, degrees.
    pub ylat0: f64,
    /// Metres-to-grid-units factors (`dxconst`, `dyconst`).
    pub dxconst: f64,
    /// Metres-to-grid-units factors (`dxconst`, `dyconst`).
    pub dyconst: f64,
    /// Polar-stereographic projections (`cmapf` descriptors).
    pub northpolemap: Strcmp,
    /// Polar-stereographic projections (`cmapf` descriptors).
    pub southpolemap: Strcmp,
    /// The nests, innermost last (upstream searches from the last).
    pub nests: Vec<NestGeometry>,
}

/// Properties of one species that `advance` needs for settling.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SettlingSpecies {
    /// Particle density, kg/m³ (`<= 0` for a gas).
    pub density: f64,
    /// Mean diameter, µm.
    pub dquer: f64,
    /// Cunningham factor.
    pub cunningham: f64,
    /// Stokes settling velocity, m/s (negative).
    pub vsetaver: f64,
}

/// One particle's prognostic state.
#[derive(Debug, Clone, PartialEq)]
pub struct Particle {
    /// Position, mother-grid units (`real(kind=dp)` upstream).
    pub xt: f64,
    /// Position, mother-grid units.
    pub yt: f64,
    /// Height above ground, m.
    pub zt: f64,
    /// Turbulent velocity components, m/s (`wp` is scaled by `sigma_w` in
    /// the Gaussian scheme).
    pub up: f64,
    /// Turbulent velocity components.
    pub vp: f64,
    /// Turbulent velocity components.
    pub wp: f64,
    /// Mesoscale velocity fluctuations, m/s.
    pub usigold: f64,
    /// Mesoscale velocity fluctuations, m/s.
    pub vsigold: f64,
    /// Mesoscale velocity fluctuations, m/s.
    pub wsigold: f64,
    /// `+1`, or `-1` after a reflection in the last sub-step.
    pub icbt: i64,
    /// Per-species probability of having been deposited this interval.
    pub prob: Vec<f64>,
}

/// What [`advance`] reports besides the updated particle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdvanceOutcome {
    /// `3` if the particle left the domain, else `0`.
    pub nstop: i32,
    /// Particles re-initialised by the CBL scheme (`nan_count` increment).
    pub nan_count: u64,
    /// NaN velocities replaced in the CBL branch (`nan_count2` increment).
    pub nan_count2: u64,
}

/// Why [`advance`] refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdvanceError {
    /// `interpolhmix` on a nest: upstream reads the unassigned `h1`.
    UndefinedMixingHeight,
    /// The particle is not below the top model level; upstream would reuse a
    /// stale level index.
    AboveTopLevel,
    /// `re_initialize_particle` ran out of draws.
    RandomNumbersExhausted,
}

/// `rannumb(n)`, 1-based.
fn rn(rannumb: &[f64], n: usize) -> f64 {
    rannumb[n - 1]
}

/// Which grid a position belongs to: `-1`/`-2` polar, `j > 0` nest `j`,
/// `0` mother. Upstream's block, used twice in `advance`.
fn which_grid(xt: f64, yt: f64, dom: &Domain, eps: f64) -> i32 {
    if dom.nglobal && yt > dom.switchnorthg {
        -1
    } else if dom.sglobal && yt < dom.switchsouthg {
        -2
    } else {
        for j in (1..=dom.nests.len()).rev() {
            let n = &dom.nests[j - 1];
            if xt > n.xln + eps && xt < n.xrn - eps && yt > n.yln + eps && yt < n.yrn - eps {
                return j as i32;
            }
        }
        0
    }
}

/// Settling velocity of the first species of release point `nrelpoint` with
/// non-zero mass, at mother-grid position `(xt, yt)`, height `zt`; `0` for a
/// gas. Upstream's block, used three times.
fn settling_velocity(
    xt: f64,
    yt: f64,
    zt: f64,
    met: &MetFields,
    species: &[SettlingSpecies],
    xmass: &[f64],
) -> Option<f64> {
    let nspec = species.len();
    let mut nsp = (0..nspec)
        .find(|&n| xmass[n] > f64::MIN_POSITIVE)
        .unwrap_or(nspec - 1);
    if nsp >= nspec {
        nsp = nspec - 1;
    }
    let sp = &species[nsp];
    if sp.density > 0.0 {
        let nix = xt as usize;
        let njy = yt as usize;
        let height = &met.height;
        let tt: Vec<f64> = (0..met.nz)
            .map(|k| met.tt[met.idx3(nix, njy, k, 0)])
            .collect();
        let rho: Vec<f64> = (0..met.nz)
            .map(|k| met.rho[met.idx3(nix, njy, k, 0)])
            .collect();
        get_settling(
            zt,
            height,
            &tt,
            &rho,
            sp.dquer,
            sp.density,
            sp.cunningham,
            sp.vsetaver,
        )
    } else {
        Some(0.0)
    }
}

/// Upstream's horizontal update on the mother grid / a nest (`ngrid >= 0`) or
/// a polar grid: move `(xt, yt)` by `(dxm, dym) * scale`, where `(dxm, dym)`
/// is in metres (or m/s with `scale` in s).
///
/// On a polar grid, returns `(dxm, dym)` divided by the grid size, which
/// upstream writes back into the variables it was given.
fn move_horizontally(
    p: &mut Particle,
    ngrid: i32,
    dxm: f64,
    dym: f64,
    scale: f64,
    dom: &Domain,
) -> Option<(f64, f64)> {
    if ngrid >= 0 {
        let cosfact = dom.dxconst / ((p.yt * dom.dy + dom.ylat0) * PI180).cos();
        p.xt += dxm * cosfact * scale;
        p.yt += dym * dom.dyconst * scale;
        None
    } else {
        let map = if ngrid == -1 {
            &dom.northpolemap
        } else {
            &dom.southpolemap
        };
        let xlon = dom.xlon0 + p.xt * dom.dx;
        let ylat = dom.ylat0 + p.yt * dom.dy;
        let (mut xpol, mut ypol) = cll2xy(map, ylat, xlon);
        let gridsize = 1000.0 * cgszll(map, ylat, xlon);
        let dxg = dxm / gridsize;
        let dyg = dym / gridsize;
        xpol += dxg * scale;
        ypol += dyg * scale;
        let (ylat, xlon) = cxy2ll(map, xpol, ypol);
        p.xt = (xlon - dom.xlon0) / dom.dx;
        p.yt = (ylat - dom.ylat0) / dom.dy;
        Some((dxg, dyg))
    }
}

/// Upstream's wrap at the east/west edge of a global domain and over the
/// poles. Returns `true` if the particle has left the domain.
fn wrap_and_check(p: &mut Particle, dom: &Domain, eps: f64) -> bool {
    let nxmin1 = dom.nxmin1 as f64;
    let nymin1 = dom.nymin1 as f64;
    if dom.xglobal {
        if p.xt >= nxmin1 {
            p.xt -= nxmin1;
        }
        if p.xt < 0.0 {
            p.xt += nxmin1;
        }
        if p.xt <= eps {
            p.xt = eps;
        }
        if (p.xt - nxmin1).abs() <= eps {
            p.xt = nxmin1 - eps;
        }
    }
    if p.yt < 0.0 {
        p.xt = (p.xt + 180.0) % 360.0;
        p.yt = -p.yt;
    } else if p.yt > nymin1 {
        p.xt = (p.xt + 180.0) % 360.0;
        p.yt = 2.0 * nymin1 - p.yt;
    }
    p.xt < 0.0 || p.xt >= nxmin1 || p.yt < 0.0 || p.yt > nymin1
}

/// `advance.f90`: move one particle through one synchronisation interval.
///
/// # Arguments
/// - `itime` — current model time, s.
/// - `ldt` — in: the particle's last time step; out: its next one, s.
/// - `p` — the particle.
/// - `nrand0` — the starting index into `rannumb` (1-based), i.e. upstream's
///   `int(ran3(idummy) * (maxrand-1)) + 1`.
/// - `rannumb` — the Gaussian draws (`maxrand = rannumb.len()`).
/// - `ip`, `hs` — `interpol_mod` and `hanna_mod`, carried between calls.
/// - `met`, `nests` — the mother grid and the nests.
/// - `dom`, `cfg` — domain and run settings.
/// - `species`, `xmass` — settling properties, and the release point's mass
///   per species (picks the species whose settling applies).
///
/// # Errors
/// See [`AdvanceError`]: each is a state in which upstream reads an undefined
/// value.
#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::needless_range_loop
)]
pub fn advance(
    itime: i64,
    ldt: &mut i64,
    p: &mut Particle,
    nrand0: usize,
    rannumb: &[f64],
    ip: &mut Interpolator,
    hs: &mut HannaState,
    met: &MetFields,
    nests: &[MetFields],
    dom: &Domain,
    cfg: &AdvanceSettings,
    species: &[SettlingSpecies],
    xmass: &[f64],
) -> Result<AdvanceOutcome, AdvanceError> {
    let maxrand = rannumb.len();
    let eps = dom.nxmax as f64 / 3.0e5;
    const EPS2: f64 = 1.0e-9;
    let nspec = species.len();
    let mut out = AdvanceOutcome {
        nstop: 0,
        nan_count: 0,
        nan_count2: 0,
    };

    for flag in ip.indzindicator.iter_mut().take(cfg.nmixz) {
        *flag = true;
    }
    if cfg.drydep {
        for ks in 0..nspec {
            ip.depoindicator[ks] = true;
            p.prob[ks] = 0.0;
        }
    }
    let (mut dxsave, mut dysave, mut dawsave, mut dcwsave) = (0.0, 0.0, 0.0, 0.0);
    let mut itimec = itime;
    let mut nrand = nrand0;
    let ldirect = cfg.ldirect as f64;
    let lsynctime = cfg.lsynctime;

    ip.ngrid = which_grid(p.xt, p.yt, dom, eps);
    let ngrid = ip.ngrid;
    let (mut xtn, mut ytn) = (0.0, 0.0);
    let (nix, njy);
    if ngrid > 0 {
        let n = &dom.nests[ngrid as usize - 1];
        xtn = (p.xt - n.xln) * n.xresoln;
        ytn = (p.yt - n.yln) * n.yresoln;
        ip.ix = xtn as usize;
        ip.jy = ytn as usize;
        nix = xtn.round() as usize;
        njy = ytn.round() as usize;
    } else {
        ip.ix = p.xt as usize;
        ip.jy = p.yt as usize;
        nix = p.xt.round() as usize;
        njy = p.yt.round() as usize;
    }
    ip.ixp = ip.ix + 1;
    ip.jyp = ip.jy + 1;
    ip.ddx = p.xt - ip.ix as f64;
    ip.ddy = p.yt - ip.jy as f64;
    ip.rddx = 1.0 - ip.ddx;
    ip.rddy = 1.0 - ip.ddy;
    ip.p1 = ip.rddx * ip.rddy;
    ip.p2 = ip.ddx * ip.rddy;
    ip.p3 = ip.rddx * ip.ddy;
    ip.p4 = ip.ddx * ip.ddy;
    ip.dt1 = (itime - met.memtime[0]) as f64;
    ip.dt2 = (met.memtime[1] - itime) as f64;
    ip.dtt = 1.0 / (ip.dt1 + ip.dt2);
    if ip.jyp as i64 >= dom.nymax {
        ip.jyp -= 1;
    }

    // Mixing height and tropopause.
    let grid = if ngrid > 0 {
        &nests[ngrid as usize - 1]
    } else {
        met
    };
    let mut h = 0.0;
    let mut h1 = [0.0; 2];
    for k in 0..2 {
        let mind = grid.memind[k];
        if ngrid <= 0 && cfg.interpolhmix {
            let c = [
                grid.idx2(ip.ix, ip.jy, mind),
                grid.idx2(ip.ixp, ip.jy, mind),
                grid.idx2(ip.ix, ip.jyp, mind),
                grid.idx2(ip.ixp, ip.jyp, mind),
            ];
            h1[k] = ip.p1 * grid.hmix[c[0]]
                + ip.p2 * grid.hmix[c[1]]
                + ip.p3 * grid.hmix[c[2]]
                + ip.p4 * grid.hmix[c[3]];
        } else {
            for j in ip.jy..=ip.jyp {
                for i in ip.ix..=ip.ixp {
                    let v = grid.hmix[grid.idx2(i, j, mind)];
                    if v > h {
                        h = v;
                    }
                }
            }
        }
    }
    let tropop = grid.tropopause[grid.idx2(nix, njy, 0)];
    if cfg.interpolhmix {
        if ngrid > 0 {
            return Err(AdvanceError::UndefinedMixingHeight);
        }
        h = (h1[0] * ip.dt2 + h1[1] * ip.dt1) * ip.dtt;
    }
    hs.h = h;
    hs.zeta = p.zt / h;

    let mut dt;
    let mut finished_in_pbl = false;
    // Upstream's local `vdepo(maxspec)`: kept across the loop iterations of
    // one call, refreshed only while a species' depoindicator is set.
    let mut vdepo = vec![0.0; nspec];
    if hs.zeta <= 1.0 {
        let mut lp = 0;
        loop {
            lp += 1;
            if cfg.method == 1 {
                *ldt = (*ldt).min((lsynctime - itimec + itime).abs());
                itimec += *ldt * cfg.ldirect;
            } else {
                *ldt = lsynctime.abs();
                itimec = itime + lsynctime;
            }
            dt = *ldt as f64;
            hs.zeta = p.zt / h;

            if lp == 1 {
                let (x, y) = if ngrid <= 0 { (p.xt, p.yt) } else { (xtn, ytn) };
                let s = ip
                    .interpol_all(grid, itime, x, y, p.zt)
                    .ok_or(AdvanceError::AboveTopLevel)?;
                hs.ust = s.ust;
                hs.wst = s.wst;
                hs.ol = s.ol;
            } else {
                if let Some(i) = (1..grid.nz).find(|&i| grid.height[i] > p.zt) {
                    ip.indz = i - 1;
                    ip.indzp = i;
                }
                for i in ip.indz..=ip.indzp {
                    if ip.indzindicator[i] {
                        ip.interpol_misslev(grid, i);
                    }
                }
            }

            let (indz, indzp) = (ip.indz, ip.indzp);
            let dz = 1.0 / (grid.height[indzp] - grid.height[indz]);
            let dz1 = (p.zt - grid.height[indz]) * dz;
            let dz2 = (grid.height[indzp] - p.zt) * dz;
            ip.u = dz1 * ip.uprof[indzp] + dz2 * ip.uprof[indz];
            ip.v = dz1 * ip.vprof[indzp] + dz2 * ip.vprof[indz];
            ip.w = dz1 * ip.wprof[indzp] + dz2 * ip.wprof[indz];
            let rhoa = dz1 * ip.rhoprof[indzp] + dz2 * ip.rhoprof[indz];
            let rhograd = dz1 * ip.rhogradprof[indzp] + dz2 * ip.rhogradprof[indz];

            if cfg.turbswitch {
                hanna(hs, p.zt);
            } else {
                hanna1(hs, p.zt);
            }

            // Horizontal turbulent velocities.
            if nrand + 1 > maxrand {
                nrand = 1;
            }
            if dt / hs.tlu < 0.5 {
                p.up = (1.0 - dt / hs.tlu) * p.up
                    + rn(rannumb, nrand) * hs.sigu * (2.0 * dt / hs.tlu).sqrt();
            } else {
                let ru = (-dt / hs.tlu).exp();
                p.up = ru * p.up + rn(rannumb, nrand) * hs.sigu * (1.0 - ru * ru).sqrt();
            }
            if dt / hs.tlv < 0.5 {
                p.vp = (1.0 - dt / hs.tlv) * p.vp
                    + rn(rannumb, nrand + 1) * hs.sigv * (2.0 * dt / hs.tlv).sqrt();
            } else {
                let rv = (-dt / hs.tlv).exp();
                p.vp = rv * p.vp + rn(rannumb, nrand + 1) * hs.sigv * (1.0 - rv * rv).sqrt();
            }
            nrand += 2;
            if nrand + cfg.ifine as usize > maxrand {
                nrand = 1;
            }
            let rhoaux = rhograd / rhoa;
            let dtf = dt * cfg.fine;
            let dtftlw = dtf / hs.tlw;
            let sgn = |c: i64| c as f64;

            // Vertical sub-steps.
            for i in 1..=cfg.ifine as usize {
                let mut delz;
                if cfg.turbswitch {
                    if dtftlw < 0.5 {
                        if cfg.cblflag {
                            if -h / hs.ol > 5.0 {
                                nrand += 1;
                                let mut old_wp_buf = p.wp;
                                let t = cbl(
                                    p.wp, p.zt, hs.ust, hs.wst, h, rhoa, rhograd, hs.sigw,
                                    hs.dsigwdz, hs.tlw, hs.ol, ldirect,
                                );
                                p.wp =
                                    (p.wp + t.ath * dtf + t.bth * rn(rannumb, nrand) * dtf.sqrt())
                                        * sgn(p.icbt);
                                delz = p.wp * dtf;
                                if t.reinitialise {
                                    re_initialize_particle(
                                        p.zt,
                                        hs.ust,
                                        hs.wst,
                                        h,
                                        hs.sigw,
                                        &mut old_wp_buf,
                                        &mut nrand,
                                        hs.ol,
                                        ldirect,
                                        rannumb,
                                    )
                                    .ok_or(AdvanceError::RandomNumbersExhausted)?;
                                    p.wp = old_wp_buf;
                                    delz = p.wp * dtf;
                                    out.nan_count += 1;
                                }
                            } else {
                                nrand += 1;
                                let ath = -p.wp / hs.tlw
                                    + hs.sigw * hs.dsigwdz
                                    + p.wp * p.wp / hs.sigw * hs.dsigwdz
                                    + hs.sigw * hs.sigw / rhoa * rhograd;
                                let bth = hs.sigw * rn(rannumb, nrand) * (2.0 * dtftlw).sqrt();
                                p.wp = (p.wp + ath * dtf + bth) * sgn(p.icbt);
                                delz = p.wp * dtf;
                                let del_test = (1.0 - p.wp) / p.wp;
                                if p.wp.is_nan() || del_test.is_nan() {
                                    nrand += 1;
                                    p.wp = hs.sigw * rn(rannumb, nrand);
                                    delz = p.wp * dtf;
                                    out.nan_count2 += 1;
                                }
                            }
                        } else {
                            p.wp = ((1.0 - dtftlw) * p.wp
                                + rn(rannumb, nrand + i) * (2.0 * dtftlw).sqrt()
                                + dtf * (hs.dsigwdz + rhoaux * hs.sigw))
                                * sgn(p.icbt);
                            delz = p.wp * hs.sigw * dtf;
                        }
                    } else {
                        let rw = (-dtftlw).exp();
                        p.wp = (rw * p.wp
                            + rn(rannumb, nrand + i) * (1.0 - rw * rw).sqrt()
                            + hs.tlw * (1.0 - rw) * (hs.dsigwdz + rhoaux * hs.sigw))
                            * sgn(p.icbt);
                        delz = p.wp * hs.sigw * dtf;
                    }
                } else {
                    let rw = (-dtftlw).exp();
                    p.wp = (rw * p.wp
                        + rn(rannumb, nrand + i) * (1.0 - rw * rw).sqrt() * hs.sigw
                        + hs.tlw * (1.0 - rw) * (hs.dsigw2dz + rhoaux * (hs.sigw * hs.sigw)))
                        * sgn(p.icbt);
                    delz = p.wp * dtf;
                }
                if cfg.turboff {
                    p.up = 0.0;
                    p.vp = 0.0;
                    p.wp = 0.0;
                    delz = 0.0;
                }
                if delz.abs() > h {
                    delz %= h;
                }
                if delz < -p.zt {
                    p.icbt = -1;
                    p.zt = -p.zt - delz;
                } else if delz > h - p.zt {
                    p.icbt = -1;
                    p.zt = -p.zt - delz + 2.0 * h;
                } else {
                    p.icbt = 1;
                    p.zt += delz;
                }
                if i != cfg.ifine as usize {
                    hs.zeta = p.zt / h;
                    hanna_short(hs, p.zt);
                }
            }
            // The DO variable's exit value is ifine + 1.
            if !cfg.cblflag {
                nrand += cfg.ifine as usize + 1;
            }

            *ldt = if cfg.turbswitch {
                (hs.tlw
                    .min(h / (2.0 * (p.wp * hs.sigw).abs()).max(1.0e-5))
                    .min(0.5 / hs.dsigwdz.abs())
                    * cfg.ctl) as i64
            } else {
                (hs.tlw.min(h / (2.0 * p.wp.abs()).max(1.0e-5)) * cfg.ctl) as i64
            };
            *ldt = (*ldt).max(cfg.mintime);

            if cfg.mdomainfill == 0 && cfg.lsettling {
                ip.w += settling_velocity(p.xt, p.yt, p.zt, met, species, xmass)
                    .ok_or(AdvanceError::AboveTopLevel)?;
            }

            dxsave += ip.u * dt;
            dysave += ip.v * dt;
            dawsave += p.up * dt;
            dcwsave += p.vp * dt;
            p.zt += ip.w * dt * ldirect;
            let top = grid.height[grid.nz - 1];
            if p.zt >= top {
                p.zt = top - 100.0 * eps;
            }
            if p.zt > h {
                if itimec == itime + lsynctime {
                    finished_in_pbl = true;
                }
                break;
            }

            if cfg.drydep && p.zt < 2.0 * HREF {
                for ks in 0..nspec {
                    if cfg.drydepspec[ks] {
                        if ip.depoindicator[ks] {
                            vdepo[ks] = ip.interpol_vdep(grid, ks);
                        }
                        p.prob[ks] =
                            1.0 + (p.prob[ks] - 1.0) * (-vdepo[ks] * dt.abs() / (2.0 * HREF)).exp();
                    }
                }
            }
            if p.zt < 0.0 {
                p.zt = (h - EPS2).min(-p.zt);
            }
            if itimec == itime + lsynctime {
                ip.usig = 0.5 * (ip.usigprof[indzp] + ip.usigprof[indz]);
                ip.vsig = 0.5 * (ip.vsigprof[indzp] + ip.vsigprof[indz]);
                ip.wsig = 0.5 * (ip.wsigprof[indzp] + ip.wsigprof[indz]);
                finished_in_pbl = true;
                break;
            }
        }
    }

    if !finished_in_pbl {
        let (mut ux, mut vy);
        // Label 700: above the boundary layer.
        let (x, y) = if ngrid <= 0 { (p.xt, p.yt) } else { (xtn, ytn) };
        ip.interpol_wind(grid, itime, x, y, p.zt)
            .ok_or(AdvanceError::AboveTopLevel)?;
        *ldt = (lsynctime - itimec + itime).abs();
        dt = *ldt as f64;
        if p.zt < tropop {
            let uxscale = (2.0 * cfg.d_trop / dt).sqrt();
            if nrand + 1 > maxrand {
                nrand = 1;
            }
            ux = rn(rannumb, nrand) * uxscale;
            vy = rn(rannumb, nrand + 1) * uxscale;
            nrand += 2;
            p.wp = 0.0;
        } else if p.zt < tropop + 1000.0 {
            let weight = (p.zt - tropop) / 1000.0;
            let uxscale = (2.0 * cfg.d_trop / dt * (1.0 - weight)).sqrt();
            if nrand + 2 > maxrand {
                nrand = 1;
            }
            ux = rn(rannumb, nrand) * uxscale;
            vy = rn(rannumb, nrand + 1) * uxscale;
            let wpscale = (2.0 * cfg.d_strat / dt * weight).sqrt();
            p.wp = rn(rannumb, nrand + 2) * wpscale + cfg.d_strat / 1000.0;
            nrand += 3;
        } else {
            if nrand > maxrand {
                nrand = 1;
            }
            ux = 0.0;
            vy = 0.0;
            let wpscale = (2.0 * cfg.d_strat / dt).sqrt();
            p.wp = rn(rannumb, nrand) * wpscale;
            nrand += 1;
        }
        if cfg.turboff {
            ux = 0.0;
            vy = 0.0;
            p.wp = 0.0;
        }
        if cfg.mdomainfill == 0 && cfg.lsettling {
            ip.w += settling_velocity(p.xt, p.yt, p.zt, met, species, xmass)
                .ok_or(AdvanceError::AboveTopLevel)?;
        }
        dxsave += (ip.u + ux) * dt;
        dysave += (ip.v + vy) * dt;
        p.zt += (ip.w + p.wp) * dt * ldirect;
        if p.zt < 0.0 {
            p.zt = (h - EPS2).min(-p.zt);
        }
    }

    // Label 99: mesoscale turbulence, alignment, horizontal move.
    let r = (-2.0 * lsynctime.abs() as f64 / cfg.lwindinterv as f64).exp();
    let rs = (1.0 - r * r).sqrt();
    if nrand + 2 > maxrand {
        nrand = 1;
    }
    p.usigold = r * p.usigold + rs * rn(rannumb, nrand) * ip.usig * cfg.turbmesoscale;
    p.vsigold = r * p.vsigold + rs * rn(rannumb, nrand + 1) * ip.vsig * cfg.turbmesoscale;
    p.wsigold = r * p.wsigold + rs * rn(rannumb, nrand + 2) * ip.wsig * cfg.turbmesoscale;
    dxsave += p.usigold * lsynctime as f64;
    dysave += p.vsigold * lsynctime as f64;
    p.zt += p.wsigold * lsynctime as f64;
    if p.zt < 0.0 {
        p.zt = -p.zt;
    }

    let (ua, va) = windalign(dxsave, dysave, dawsave, dcwsave);
    dxsave += ua;
    dysave += va;
    let _ = move_horizontally(p, ngrid, dxsave, dysave, ldirect, dom);
    if wrap_and_check(p, dom, eps) {
        out.nstop = 3;
        return Ok(out);
    }
    let top = met.height[met.nz - 1];
    if p.zt >= top {
        p.zt = top - 100.0 * eps;
    }

    // Petterssen correction, only for a full step that stays in the same
    // wind-field interval and on the same grid.
    if *ldt != lsynctime.abs() {
        return Ok(out);
    }
    if (itime + *ldt * cfg.ldirect).abs() > met.memtime[1].abs() {
        return Ok(out);
    }
    if which_grid(p.xt, p.yt, dom, eps) != ngrid {
        return Ok(out);
    }
    if ngrid > 0 {
        let n = &dom.nests[ngrid as usize - 1];
        xtn = (p.xt - n.xln) * n.xresoln;
        ytn = (p.yt - n.yln) * n.yresoln;
        ip.ix = xtn as usize;
        ip.jy = ytn as usize;
    } else {
        ip.ix = p.xt as usize;
        ip.jy = p.yt as usize;
    }
    ip.ixp = ip.ix + 1;
    ip.jyp = ip.jy + 1;
    let (uold, vold, wold) = (ip.u, ip.v, ip.w);
    let tnew = itime + *ldt * cfg.ldirect;
    let (x, y) = if ngrid <= 0 { (p.xt, p.yt) } else { (xtn, ytn) };
    ip.interpol_wind_short(grid, tnew, x, y, p.zt)
        .ok_or(AdvanceError::AboveTopLevel)?;
    if cfg.mdomainfill == 0 && cfg.lsettling {
        ip.w += settling_velocity(p.xt, p.yt, p.zt, met, species, xmass)
            .ok_or(AdvanceError::AboveTopLevel)?;
    }
    ip.u = (ip.u - uold) / 2.0;
    ip.v = (ip.v - vold) / 2.0;
    ip.w = (ip.w - wold) / 2.0;
    let step = (*ldt * cfg.ldirect) as f64;
    p.zt += ip.w * step;
    if p.zt < 0.0 {
        p.zt = (h - EPS2).min(-p.zt);
    }
    // On a polar grid upstream divides the module's u, v by the grid size in
    // place, so they leave advance in grid units per second.
    if let Some((u, v)) = move_horizontally(p, ngrid, ip.u, ip.v, step, dom) {
        ip.u = u;
        ip.v = v;
    }
    if wrap_and_check(p, dom, eps) {
        out.nstop = 3;
        return Ok(out);
    }
    if p.zt >= top {
        p.zt = top - 100.0 * eps;
    }
    Ok(out)
}

/// `initialize.f90`: initial turbulent and mesoscale velocities, and the first
/// time step, of a newly released particle.
///
/// Uses the mother grid only, as upstream does, but does **not** set
/// `ngrid`: `interpol_all` then reads the polar winds if the previous
/// `advance` call left `ngrid < 0`. The port keeps that (pass the same
/// [`Interpolator`]). `nrand0` is upstream's
/// `int(ran3(idummy) * (maxrand-1)) + 1`; `cbl_draws` the `(ran3, gasdev)`
/// pair `initialize_cbl_vel` would draw (used only in the CBL branch).
///
/// # Returns
/// The first time step `ldt`, s.
#[allow(clippy::too_many_arguments)]
pub fn initialize(
    itime: i64,
    p: &mut Particle,
    nrand0: usize,
    rannumb: &[f64],
    cbl_draws: (f64, f64),
    ip: &mut Interpolator,
    hs: &mut HannaState,
    met: &MetFields,
    cfg: &AdvanceSettings,
) -> Result<i64, AdvanceError> {
    let maxrand = rannumb.len();
    let mut nrand = nrand0;
    p.icbt = 1;
    ip.ix = p.xt as usize;
    ip.jy = p.yt as usize;
    ip.ixp = ip.ix + 1;
    ip.jyp = ip.jy + 1;
    let (m1, m2) = (met.memind[0], met.memind[1]);
    let hm = |i: usize, j: usize, m: usize| met.hmix[met.idx2(i, j, m)];
    let (ix, jy, ixp, jyp) = (ip.ix, ip.jy, ip.ixp, ip.jyp);
    let h = [
        hm(ix, jy, m1),
        hm(ixp, jy, m1),
        hm(ix, jyp, m1),
        hm(ixp, jyp, m1),
        hm(ix, jy, m2),
        hm(ixp, jy, m2),
        hm(ix, jyp, m2),
        hm(ixp, jyp, m2),
    ]
    .into_iter()
    .fold(f64::NEG_INFINITY, f64::max);
    hs.h = h;
    hs.zeta = p.zt / h;
    let ldt;
    if hs.zeta <= 1.0 {
        let s = ip
            .interpol_all(met, itime, p.xt, p.yt, p.zt)
            .ok_or(AdvanceError::AboveTopLevel)?;
        hs.ust = s.ust;
        hs.wst = s.wst;
        hs.ol = s.ol;
        let (indz, indzp) = (ip.indz, ip.indzp);
        let dz1 = p.zt - met.height[indz];
        let dz2 = met.height[indzp] - p.zt;
        let dz = 1.0 / (dz1 + dz2);
        ip.u = (dz1 * ip.uprof[indzp] + dz2 * ip.uprof[indz]) * dz;
        ip.v = (dz1 * ip.vprof[indzp] + dz2 * ip.vprof[indz]) * dz;
        ip.w = (dz1 * ip.wprof[indzp] + dz2 * ip.wprof[indz]) * dz;
        if cfg.turbswitch {
            hanna(hs, p.zt);
        } else {
            hanna1(hs, p.zt);
        }
        if nrand + 2 > maxrand {
            nrand = 1;
        }
        p.up = rn(rannumb, nrand) * hs.sigu;
        p.vp = rn(rannumb, nrand + 1) * hs.sigv;
        p.wp = rn(rannumb, nrand + 2);
        if !cfg.turbswitch {
            p.wp *= hs.sigw;
        } else if cfg.cblflag {
            if -h / hs.ol > 5.0 {
                p.wp = initialize_cbl_vel(
                    p.zt,
                    hs.ust,
                    hs.wst,
                    h,
                    hs.sigw,
                    hs.ol,
                    cfg.ldirect as f64,
                    cbl_draws.0,
                    cbl_draws.1,
                );
            } else {
                p.wp *= hs.sigw;
            }
        }
        let l = if cfg.turbswitch {
            (hs.tlw
                .min(h / (2.0 * (p.wp * hs.sigw).abs()).max(1.0e-5))
                .min(0.5 / hs.dsigwdz.abs())
                .min(600.0)
                * cfg.ctl) as i64
        } else {
            (hs.tlw.min(h / (2.0 * p.wp.abs()).max(1.0e-5)).min(600.0) * cfg.ctl) as i64
        };
        ldt = l.max(cfg.mintime);
        ip.usig = (ip.usigprof[indzp] + ip.usigprof[indz]) / 2.0;
        ip.vsig = (ip.vsigprof[indzp] + ip.vsigprof[indz]) / 2.0;
        ip.wsig = (ip.wsigprof[indzp] + ip.wsigprof[indz]) / 2.0;
    } else {
        ip.interpol_wind(met, itime, p.xt, p.yt, p.zt)
            .ok_or(AdvanceError::AboveTopLevel)?;
        ldt = cfg.lsynctime.abs();
        if nrand + 1 > maxrand {
            nrand = 1;
        }
        p.up = rn(rannumb, nrand) * 0.3;
        p.vp = rn(rannumb, nrand + 1) * 0.3;
        nrand += 2;
        p.wp = 0.0;
        hs.sigw = 0.0;
    }
    if nrand + 2 > maxrand {
        nrand = 1;
    }
    p.usigold = rn(rannumb, nrand) * ip.usig;
    p.vsigold = rn(rannumb, nrand + 1) * ip.vsig;
    p.wsigold = rn(rannumb, nrand + 2) * ip.wsig;
    Ok(ldt)
}

/// `get_vdep_prob.f90`: the dry deposition velocity of each species at a
/// receptor particle, written into `prob` (upstream stores the **velocity**,
/// m/s, in the array it calls `prob`).
///
/// # Upstream quirk reproduced
///
/// It sets the cell indices `ix`, `jy`, `ixp`, `jyp` and `ngrid`, but never
/// the bilinear and time weights `p1..p4`, `dt1`, `dt2`, `dtt`, so
/// `interpol_vdep` uses whatever weights the previous interpolation left in
/// `interpol_mod`. The port reads them from the [`Interpolator`] in the same
/// way, so pass the one shared with [`advance`].
///
/// Species without `DRYDEPSPEC`, and every species when `zt >= 2 href` or dry
/// deposition is off, get `0` (upstream's reset value), or keep the previous
/// value when `DRYDEP` is off, as upstream.
#[allow(clippy::too_many_arguments, clippy::needless_range_loop)]
pub fn get_vdep_prob(
    xt: f64,
    yt: f64,
    zt: f64,
    prob: &mut [f64],
    ip: &mut Interpolator,
    met: &MetFields,
    nests: &[MetFields],
    dom: &Domain,
    cfg: &AdvanceSettings,
) {
    let eps = dom.nxmax as f64 / 3.0e5;
    let nspec = prob.len();
    if cfg.drydep {
        for ks in 0..nspec {
            ip.depoindicator[ks] = true;
            prob[ks] = 0.0;
        }
    }
    ip.ngrid = which_grid(xt, yt, dom, eps);
    let ngrid = ip.ngrid;
    if ngrid > 0 {
        let n = &dom.nests[ngrid as usize - 1];
        ip.ix = ((xt - n.xln) * n.xresoln) as usize;
        ip.jy = ((yt - n.yln) * n.yresoln) as usize;
    } else {
        ip.ix = xt as usize;
        ip.jy = yt as usize;
    }
    ip.ixp = ip.ix + 1;
    ip.jyp = ip.jy + 1;
    let grid = if ngrid > 0 {
        &nests[ngrid as usize - 1]
    } else {
        met
    };
    if cfg.drydep && zt < 2.0 * HREF {
        for ks in 0..nspec {
            if cfg.drydepspec[ks] && ip.depoindicator[ks] {
                prob[ks] = ip.interpol_vdep(grid, ks);
            }
        }
    }
}
