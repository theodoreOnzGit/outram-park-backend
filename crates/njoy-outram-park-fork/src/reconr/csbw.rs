// Translated from NJOY2016.80 `src/reconr.f90` (`ab27c64`): `rdf2bw`'s competitive
// width table (:883-1028), `csslbw` (:2692-2854), `csmlbw` (:2856-3014), `csmlbw2`
// (:3016-3197), `ruina`'s `wtab` call (:483-488) and `quickw` (:5443-5527, through
// `crate::unresr::wfun::WTable`, the same routine as `unresr`'s `quikw`).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! **Breit-Wigner with a competitive width, and at a temperature** (GitHub #1731,
//! #1323).
//!
//! The 0 K, `LRX = 0` SLBW and MLBW evaluation stays where it was
//! ([`super::seam::slbw_delta`]), so nothing held moves. This module takes over
//! in the two cases it does not cover, following upstream line by line:
//!
//! - **`LRX != 0`** (a competitive width). `rdf2bw` stores, per resonance,
//!   `gc = GT - GN - GG - GF` (zero below `1e-5 GT`, or below the reaction's
//!   threshold `-QX/arat`) and `pex`, the penetrability of the competitive channel
//!   at the resonance, with `l'` set to 2 for `l = 0` and to 0 for `l = 2`
//!   (`reconr.f90:1007-1015`). `csslbw` and `csmlbw` add `gc * pec / pex` to
//!   every resonance's total width and produce the competitive cross section in
//!   `sigp(5)` (`:2755-2762, 2808, 2839`; `:2938-2945, 2997`). It goes to
//!   [`RangeDelta::other`]`[0]`: `emerge` adds it to MT=51 (`itype = 5`,
//!   `:4769`). Two upstream quirks are kept: `csmlbw` never scales `sigp(5)` by
//!   `2*pifac` as it does capture and fission (`:3007-3010`), and the total takes
//!   `sigp(5)` only when the **last** l-state has `LRX != 0` (`:2852, 3012`).
//! - **`tempr > 0`**. `csslbw` replaces the Lorentzian by the psi/chi line shapes
//!   from `quickw` (`:2775-2810`). MLBW goes through `csmlbw2`, the g/h method
//!   (`:2616-2618`): the multilevel interference is kept as level-level sums `g`
//!   and `h` taken **at the energies of earlier calls** (`save enow`, recomputed
//!   once the energy passes `1.05 enow` or `enow + 10` eV) and stored in `res`
//!   (`:3108-3165`). That state makes the result depend on the order `resxs`
//!   calls `sigma` in, so a material with a `csmlbw2` range is walked in order
//!   ([`super::RebuildOpts::in_order`]). Below `tmin = 10 K` `csmlbw2` uses the
//!   Lorentzian line shapes but keeps the g/h method (`:3167-3178`).

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use petir::mathf::RealMath;

use super::mf2::{EnergyRange, ResonanceFormalism};
use super::seam;
use super::slbw::{channel_radius, facphi, shift_and_penetrability as facts, WAVE_K};
use super::RangeDelta;
use crate::common::phys::{BK_EV_PER_K, PI};
use crate::mixr::mix::sigfig;
use crate::once::RwLock;
use crate::unresr::wfun::WTable;

/// `rdf2bw`'s `gxmin` (`reconr.f90:901`): a competitive width below this fraction
/// of `GT` is taken as zero.
const GXMIN: f64 = 1.0e-5;
/// `csmlbw2`'s `tmin` (`reconr.f90:3044`): below it the line shapes are
/// Lorentzian.
const TMIN: f64 = 10.0;

/// What `sigma`'s Breit-Wigner routines need besides the range: `tempr`, the
/// `w` table `ruina` builds for it (`reconr.f90:483-488`), and `csmlbw2`'s
/// `save enow`.
pub(crate) struct BwContext {
    tempr: f64,
    wtab: Option<WTable>,
    /// `csmlbw2`'s `enow` (`save`d, so one value for the whole run; gfortran's
    /// static storage starts it at 0, so the first call recomputes g/h).
    enow: RwLock<f64>,
}

impl BwContext {
    /// The context for a reconstruction at `tempr` \[K\]; `ruina` builds the `w`
    /// table only when `tempr > 0`.
    pub(crate) fn new(tempr: f64) -> Self {
        BwContext { tempr, wtab: if tempr > 0.0 { Some(WTable::new()) } else { None }, enow: RwLock::new(0.0) }
    }

    /// `tempr > 0`: the Doppler branches are taken.
    pub(crate) fn hot(&self) -> bool {
        self.tempr > 0.0
    }

    /// `(tempr, w table)` when `tempr > 0`, for `csaa`'s psi/chi branch.
    pub(crate) fn wtab(&self) -> Option<(f64, &WTable)> {
        self.wtab.as_ref().map(|t| (self.tempr, t))
    }

    /// `quickw(ax, y, rew, aimw, 1, tr, ti)`.
    fn quickw(&self, ax: f64, y: f64) -> (f64, f64) {
        match &self.wtab {
            Some(t) => t.lookup(ax, y, true),
            None => (0.0, 0.0),
        }
    }
}

/// One resonance as `rdf2bw` leaves it in `res`: the six parameters, then the
/// shift and penetrability at the resonance, the competitive width and its
/// penetrability (`reconr.f90:999-1015`).
#[derive(Debug, Clone, Copy)]
struct BwRes {
    er: f64,
    aj: f64,
    gn: f64,
    gg: f64,
    gf: f64,
    ser: f64,
    per: f64,
    gc: f64,
    pex: f64,
}

/// One l-state of `rdf2bw`'s table.
#[derive(Debug, Clone)]
struct BwLState {
    l: u32,
    qx: f64,
    lrx: i32,
    res: Vec<BwRes>,
}

/// One SLBW/MLBW range, prepared once: the 0 K evaluator's constants
/// (`old`, for [`seam::slbw_delta`]), `rdf2bw`'s table for this module, and the
/// g/h values `csmlbw2` stores per resonance.
pub(crate) struct BwRange {
    pub(crate) old: Vec<seam::SlbwLState>,
    /// `res(inow+12)`: the first l-state's AWRI, which `csslbw`/`csmlbw2` use for
    /// `k` and the Doppler width of every l-state.
    awri: f64,
    lstates: Vec<BwLState>,
    any_lrx: bool,
    /// `res(in+4)`, `res(in+5)` of each resonance: `g/e`, `h/e` at `enow`
    /// (`reconr.f90:3163-3164`), zero as `rdf2bw` leaves them (`:1004-1005`).
    gh: RwLock<Vec<Vec<(f64, f64)>>>,
}

/// `rdf2bw`'s table for `range` (`reconr.f90:938-1027`). The radius is `ral`
/// (`ra` for the competitive channel), the formula, or `AP` when `NAPS = 1`; for
/// LRF = 1/2 the two agree.
pub(crate) fn prepare(range: &EnergyRange) -> BwRange {
    let lstates: Vec<BwLState> = range
        .l_states
        .iter()
        .map(|ls| {
            let awri = ls.awri;
            let arat = awri / (awri + 1.0);
            let ra = channel_radius(awri, range.naps, range.ap);
            let res = ls
                .resonances
                .iter()
                .map(|r| {
                    let rho = WAVE_K * arat * r.er.abs().sqrt() * ra;
                    let (ser, per) = facts(ls.l, rho);
                    let (mut gc, mut pex) = (0.0, 0.0);
                    if ls.lrx != 0 {
                        let rhoc = WAVE_K * arat * (r.er + ls.qx / arat).abs().sqrt() * ra;
                        let (_, pec) = facts(competing_l(ls.l), rhoc);
                        gc = r.gt - r.gn - r.gg - r.gf;
                        if gc < GXMIN * r.gt {
                            gc = 0.0;
                        }
                        if r.er < -ls.qx / arat {
                            gc = 0.0;
                        }
                        pex = pec;
                    }
                    BwRes { er: r.er, aj: r.aj, gn: r.gn, gg: r.gg, gf: r.gf, ser, per, gc, pex }
                })
                .collect();
            BwLState { l: ls.l, qx: ls.qx, lrx: ls.lrx, res }
        })
        .collect();
    let gh = lstates.iter().map(|ls| vec![(0.0, 0.0); ls.res.len()]).collect();
    BwRange {
        old: seam::slbw_prepare(range),
        awri: range.l_states.first().map_or(1.0, |ls| ls.awri),
        any_lrx: range.l_states.iter().any(|ls| ls.lrx != 0),
        lstates,
        gh: RwLock::new(gh),
    }
}

/// "set competing l' value for a 2+ inelastic residual (u238)"
/// (`reconr.f90:1009-1011, 2758-2760`).
fn competing_l(l: u32) -> u32 {
    match l {
        0 => 2,
        2 => 0,
        other => other,
    }
}

/// Whether `range` is evaluated by `csmlbw2` under `ctx` (MLBW with `tempr > 0`),
/// whose stored g/h make the walk order matter.
pub(crate) fn needs_ordered_walk(range: &EnergyRange, ctx: &BwContext) -> bool {
    ctx.hot() && matches!(range.formalism, Some(ResonanceFormalism::Mlbw))
}

/// `sigma`'s SLBW/MLBW `sigp` of `range` at `e`, before the clamp and the
/// abundance: [`seam::slbw_delta`] for the 0 K `LRX = 0` case it has always
/// covered, else `csslbw`, `csmlbw` or `csmlbw2` from this module
/// (`reconr.f90:2610-2618`).
pub(crate) fn bw_delta(range: &EnergyRange, bw: &BwRange, e: f64, ctx: &BwContext) -> RangeDelta {
    let mlbw = matches!(range.formalism, Some(ResonanceFormalism::Mlbw));
    if !ctx.hot() && !bw.any_lrx {
        return seam::slbw_delta(range, &bw.old, e);
    }
    match (mlbw, ctx.hot()) {
        (false, _) => csslbw(range, bw, e, ctx),
        (true, false) => csmlbw(range, bw, e),
        (true, true) => csmlbw2(range, bw, e, ctx),
    }
}

/// The penetrability of the competitive channel at `e` (`pec`): `l'` swapped and
/// `E + QX/arat` as in `csslbw`/`csmlbw` (`reconr.f90:2753-2762`), zero below the
/// reaction's threshold.
fn competitive_pec(ls: &BwLState, e: f64, arat: f64, ra: f64) -> f64 {
    let mut pec = 0.0;
    if ls.lrx != 0 {
        let rhop = WAVE_K * arat * (e + ls.qx / arat).abs().sqrt() * ra;
        pec = facts(competing_l(ls.l), rhop).1;
        if e + ls.qx / arat < 0.0 {
            pec = 0.0;
        }
    }
    pec
}

/// `csslbw` (`reconr.f90:2692-2854`) for every l-state of `range`, both
/// branches: psi/chi from `quickw` when `tempr > 0`, else the Lorentzian.
fn csslbw(range: &EnergyRange, bw: &BwRange, e: f64, ctx: &BwContext) -> RangeDelta {
    let rpi = PI.sqrt();
    let tbk = ctx.tempr * BK_EV_PER_K;
    let awri = bw.awri;
    let ap = range.ap;
    let ra = channel_radius(awri, range.naps, ap);
    let spifac = 1.0 / (2.0 * range.spi + 1.0);
    let arat = awri / (awri + 1.0);
    let k = WAVE_K * arat * e.abs().sqrt();
    let pifac = PI / (k * k);
    let rho = k * ra;
    let rhoc = k * ap;
    let mut sigp = [0.0f64; 5];
    let mut lrx = 0;
    for ls in &bw.lstates {
        lrx = ls.lrx;
        let (se, pe) = facts(ls.l, rho);
        let pec = competitive_pec(ls, e, arat, ra);
        let phi = facphi(ls.l, rhoc);
        let cos2p = (2.0 * phi).r_cos();
        let sin2p = (2.0 * phi).r_sin();
        let sinsq = phi.r_sin().r_powi(2);
        let mut spot = 4.0 * (2 * ls.l + 1) as f64 * pifac * sinsq;
        for r in &ls.res {
            let rper = 1.0 / r.per;
            let gx = r.gg + r.gf;
            let gj = (2.0 * r.aj + 1.0) * spifac / 2.0;
            let erp = r.er + r.gn * (r.ser - se) * rper / 2.0;
            let edelt = e - erp;
            let gne = r.gn * pe * rper;
            let mut gtt = gne + gx;
            if r.gc != 0.0 {
                gtt += r.gc * pec / r.pex;
            }
            if ctx.hot() {
                let ex = 2.0 * (e - erp) / gtt;
                let delta = (4.0 * tbk * e / awri).sqrt();
                let theta = gtt / delta;
                let ax = theta * ex / 2.0;
                let y = theta / 2.0;
                let (rew, aimw) = ctx.quickw(ax, y);
                let psi = rpi * theta * rew / 2.0;
                let chi = rpi * theta * aimw / 2.0;
                let smax = 4.0 * pifac * gj * gne / (gtt * gtt);
                sigp[1] += smax * ((cos2p * gtt - gx) * psi + sin2p * chi * gtt);
                sigp[2] += smax * r.gf * psi;
                sigp[3] += smax * r.gg * psi;
                if ls.lrx != 0 && r.gc != 0.0 {
                    sigp[4] += smax * r.gc * pec * psi / r.pex;
                }
            } else {
                let comfac = pifac * gj * gne / (edelt * edelt + gtt * gtt / 4.0);
                let add = comfac * (gne * cos2p - 2.0 * gx * sinsq + 2.0 * edelt * sin2p);
                sigp[1] += add;
                sigp[2] += comfac * r.gf;
                sigp[3] += comfac * r.gg;
                if ls.lrx != 0 && r.gc != 0.0 {
                    sigp[4] += comfac * r.gc * pec / r.pex;
                }
            }
        }
        // `reconr.f90:2849-2851`
        sigp[1] = sigfig(sigp[1], 8, 0);
        spot = sigfig(spot, 8, 0);
        sigp[1] += spot;
    }
    sigp[0] = sigp[1] + sigp[2] + sigp[3];
    if lrx != 0 {
        sigp[0] += sigp[4];
    }
    to_delta(sigp)
}

/// `csmlbw` (`reconr.f90:2856-3014`) at 0 K, for a range with a competitive width.
/// The scattering radius and the radius at the energy follow `NRO`/`NAPS` as in
/// [`seam::slbw_delta`] (`:2893-2919`).
fn csmlbw(range: &EnergyRange, bw: &BwRange, e: f64) -> RangeDelta {
    let awri = bw.awri;
    let (ap, ra) = match &range.ap_table {
        Some(table) => {
            let ape = table.at(e).unwrap_or(range.ap);
            match range.naps {
                0 => (ape, channel_radius(awri, 0, 0.0)),
                1 => (ape, ape),
                _ => (ape, range.ap),
            }
        }
        None => (range.ap, channel_radius(awri, range.naps, range.ap)),
    };
    let spi = range.spi;
    let den = 4.0 * spi + 2.0;
    let arat = awri / (awri + 1.0);
    let k = WAVE_K * arat * e.abs().sqrt();
    let pifac = PI / (k * k);
    let rho = k * ra;
    let rhoc = k * ap;
    let mut sigp = [0.0f64; 5];
    let mut lrx = 0;
    for ls in &bw.lstates {
        lrx = ls.lrx;
        let (se, pe) = facts(ls.l, rho);
        let pec = competitive_pec(ls, e, arat, ra);
        let phi = facphi(ls.l, rhoc);
        let cos2p = 1.0 - (2.0 * phi).r_cos();
        let sin2p = (2.0 * phi).r_sin();
        let mut sum = 0.0;
        let fl = ls.l as f64;
        let ajmin = ((spi - fl).abs() - 0.5).abs();
        let ajmax = spi + fl + 0.5;
        let nj = ((ajmax - ajmin + 1.0).round() as usize).min(10);
        let mut gj = [0.0f64; 10];
        let mut aj = ajmin;
        for g in gj.iter_mut().take(nj) {
            *g = (2.0 * aj + 1.0) / den;
            aj += 1.0;
            sum += *g;
        }
        let diff = 2.0 * fl + 1.0 - sum;
        let mut sigj = [[0.0f64; 10]; 2];
        for r in &ls.res {
            let j = ((r.aj - ajmin + 1.0).round() as usize).clamp(1, nj.max(1)) - 1;
            let rper = 1.0 / r.per;
            let erp = r.er + r.gn * (r.ser - se) * rper / 2.0;
            let edelt = e - erp;
            let gne = r.gn * pe * rper;
            let gx = r.gg + r.gf;
            let mut gtt = gne + gx;
            if r.gc != 0.0 {
                gtt += r.gc * pec / r.pex;
            }
            let x = 2.0 * edelt / gtt;
            let mut comfac = 2.0 * gne / gtt / (1.0 + x * x);
            sigj[0][j] += comfac;
            sigj[1][j] += comfac * x;
            comfac = comfac * gj[j] / gtt;
            sigp[2] += comfac * r.gf;
            sigp[3] += comfac * r.gg;
            if ls.lrx != 0 && r.gc != 0.0 {
                sigp[4] += comfac * r.gc * pec / r.pex;
            }
        }
        for j in 0..nj {
            let add = gj[j] * ((cos2p - sigj[0][j]).r_powi(2) + (sin2p + sigj[1][j]).r_powi(2));
            sigp[1] += add;
        }
        sigp[1] += 2.0 * diff * cos2p;
    }
    // `reconr.f90:3007-3012`: `sigp(5)` keeps no `2*pifac`.
    sigp[1] *= pifac;
    sigp[2] = sigp[2] * 2.0 * pifac;
    sigp[3] = sigp[3] * 2.0 * pifac;
    sigp[0] = sigp[1] + sigp[2] + sigp[3];
    if lrx != 0 {
        sigp[0] += sigp[4];
    }
    to_delta(sigp)
}

/// `csmlbw2` (`reconr.f90:3016-3197`): MLBW by the g/h method, the `sigma`
/// branch for `tempr > 0`. See the module doc for its `enow` state.
///
/// Kept as upstream has it: the competitive channel enters `gtt` with
/// `sqrt(abs(e+qx))` and the l-state's own `l` (not `e+qx/arat` and `l'` as in
/// `csmlbw`), `sigp(5)` is never formed, and the energy-dependent radius
/// (`NRO != 0`) is not read (the caller refuses that case).
fn csmlbw2(range: &EnergyRange, bw: &BwRange, e: f64, ctx: &BwContext) -> RangeDelta {
    let rpi = PI.sqrt();
    let tbk = ctx.tempr * BK_EV_PER_K;
    let awri = bw.awri;
    let ap = range.ap;
    let ra = channel_radius(awri, range.naps, ap);
    let spifac = 1.0 / (2.0 * range.spi + 1.0);
    let arat = awri / (awri + 1.0);
    let k = WAVE_K * arat * e.abs().sqrt();
    let pifac = PI / (k * k);
    let rho = k * ra;
    let rhoc = k * ap;

    // "accelerating the gh calculation" (`reconr.f90:3080-3090`)
    let igh = {
        let mut enow = ctx.enow.write().expect("enow lock");
        let mut igh = false;
        if (e - 1.0e-5).abs() < 1.0e-7 {
            *enow = e;
            igh = true;
        }
        let mut test = *enow + *enow / 20.0;
        if *enow + 10.0 < test {
            test = *enow + 10.0;
        }
        if e >= test {
            *enow = e;
            igh = true;
        }
        igh
    };
    let mut gh = bw.gh.write().expect("g/h lock");

    let mut sigp = [0.0f64; 5];
    for (li, ls) in bw.lstates.iter().enumerate() {
        let (se, pe) = facts(ls.l, rho);
        let mut pec = 0.0;
        if ls.lrx != 0 {
            let rhop = WAVE_K * arat * (e + ls.qx).abs().sqrt() * ra;
            pec = facts(ls.l, rhop).1;
            if e + ls.qx / arat < 0.0 {
                pec = 0.0;
            }
        }
        let phi = facphi(ls.l, rhoc);
        let cos2p = (2.0 * phi).r_cos();
        let sin2p = (2.0 * phi).r_sin();
        let sinsq = phi.r_sin().r_powi(2);
        let spot = 4.0 * (2 * ls.l + 1) as f64 * pifac * sinsq;
        let mut sml = 0.0;
        for (i, r) in ls.res.iter().enumerate() {
            let rper = 1.0 / r.per;
            let rpex = 1.0 / r.pex;
            let gx = r.gg + r.gf;
            let gj = (2.0 * r.aj + 1.0) * spifac / 2.0;
            let erp = r.er + r.gn * (r.ser - se) * rper / 2.0;
            let gne = r.gn * pe * rper;
            let mut gtt = gne + gx;
            if r.gc != 0.0 {
                gtt += r.gc * pec * rpex;
            }
            if igh {
                let (mut g, mut h) = (0.0, 0.0);
                for (j, rj) in ls.res.iter().enumerate() {
                    if j != i && rj.aj == r.aj && i.abs_diff(j) < 200 {
                        let erj = rj.er + (rj.ser - se) * rj.gn / (2.0 * rj.per);
                        let gnj = rj.gn * pe / rj.per;
                        let mut gtj = gnj + rj.gg + rj.gf;
                        if rj.gc != 0.0 {
                            gtj += rj.gc * pec / rj.pex;
                        }
                        let d = erp - erj;
                        let s = gtt + gtj;
                        g += gne * gnj * (gtt + gtj) / (d * d + s * s / 4.0);
                        h += gne * gnj * (erp - erj) / (d * d + s * s / 4.0);
                    }
                }
                g /= 2.0;
                gh[li][i] = (g / e, h / e);
            }
            let ex = 2.0 * (e - erp) / gtt;
            let (psi, chi) = if ctx.tempr > TMIN {
                let delta = (4.0 * tbk * e / awri).sqrt();
                let theta = gtt / delta;
                let ax = theta * ex / 2.0;
                let y = theta / 2.0;
                let (rew, aimw) = ctx.quickw(ax, y);
                (rpi * theta * rew / 2.0, rpi * theta * aimw / 2.0)
            } else {
                (1.0 / (1.0 + ex * ex), ex / (1.0 + ex * ex))
            };
            let (g, h) = gh[li][i];
            let smax = 4.0 * pifac * gj * gne / (gtt * gtt);
            sigp[1] += smax * ((cos2p * gtt - gx) * psi + sin2p * gtt * chi);
            sml += smax * e * (g * psi + h * chi) * gtt / gne;
            sigp[2] += smax * r.gf * psi;
            sigp[3] += smax * r.gg * psi;
        }
        let ssl = sigp[1] + spot;
        sigp[1] = ssl + sml;
    }
    sigp[0] = sigp[1] + sigp[2] + sigp[3];
    to_delta(sigp)
}

/// `sigp(1..5)` as a [`RangeDelta`], the competitive channel in `other[0]`.
fn to_delta(sigp: [f64; 5]) -> RangeDelta {
    let mut d = RangeDelta { total: sigp[0], elastic: sigp[1], fission: sigp[2], capture: sigp[3], ..RangeDelta::default() };
    d.other[0] = sigp[4];
    d
}

/// Upstream's `lrx` when `emerge` runs (`reconr.f90:4769`): the module variable
/// the last `csslbw`/`csmlbw`/`csmlbw2` call left, that is the last l-state of
/// the SLBW/MLBW range `resxs` evaluated last. Taken here as the range reaching
/// highest (the last such in file order on a tie). 0 when there is none.
pub(crate) fn final_lrx(ranges: &[EnergyRange]) -> i32 {
    let mut best: Option<&EnergyRange> = None;
    for r in ranges.iter().filter(|r| r.lru == 1 && matches!(r.formalism, Some(ResonanceFormalism::Slbw | ResonanceFormalism::Mlbw))) {
        if best.is_none_or(|b| r.eh >= b.eh) {
            best = Some(r);
        }
    }
    best.and_then(|r| r.l_states.last()).map_or(0, |ls| ls.lrx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reconr::mf2::{LState, SlbwResonance};

    fn range(formalism: ResonanceFormalism, lrx: i32, gt: f64) -> EnergyRange {
        let res = SlbwResonance { er: 2000.0, aj: 1.0, gt, gn: 3.0, gg: 0.5, gf: 0.0 };
        let res2 = SlbwResonance { er: 5000.0, aj: 1.0, gt, gn: 3.0, gg: 0.5, gf: 0.0 };
        EnergyRange {
            el: 1.0e-5,
            eh: 1.0e4,
            lru: 1,
            formalism: Some(formalism),
            spi: 1.5,
            ap: 0.5,
            naps: 0,
            l_states: vec![LState { awri: 22.8, l: 0, resonances: vec![res, res2], lrx, qx: -1000.0 }],
            rm_l_states: Vec::new(),
            rml: None,
            aa: None,
            abn: 1.0,
            isotope: 0,
            nro: 0,
            ap_table: None,
        }
    }

    /// `rdf2bw`'s competitive width: zero below `-QX/arat`, `GT - GN - GG - GF`
    /// above, and only for `LRX != 0`.
    #[test]
    fn competitive_width_follows_rdf2bw() {
        let r = range(ResonanceFormalism::Slbw, 1, 4.0);
        let bw = prepare(&r);
        let arat = 22.8 / 23.8;
        assert!(2000.0 >= 1000.0 / arat);
        assert_eq!(bw.lstates[0].res[0].gc, 4.0 - 3.0 - 0.5 - 0.0);
        assert!(bw.lstates[0].res[0].pex > 0.0);
        let below = range(ResonanceFormalism::Slbw, 1, 4.0);
        let mut below = below;
        below.l_states[0].qx = -3000.0;
        assert_eq!(prepare(&below).lstates[0].res[0].gc, 0.0, "below threshold");
        assert_eq!(prepare(&range(ResonanceFormalism::Slbw, 0, 4.0)).lstates[0].res[0].gc, 0.0, "LRX = 0");
        assert_eq!(prepare(&range(ResonanceFormalism::Slbw, 1, 3.5)).lstates[0].res[0].gc, 0.0, "no width left");
    }

    /// With no competitive width and `tempr = 0`, this module's `csslbw` gives the
    /// same capture as the 0 K evaluator it stands beside (one l-state, so the
    /// summation orders agree), and a competitive cross section once `LRX != 0`.
    #[test]
    fn csslbw_without_competition_is_the_zero_kelvin_evaluator() {
        let ctx = BwContext::new(0.0);
        let r = range(ResonanceFormalism::Slbw, 0, 3.5);
        let bw = prepare(&r);
        for e in [10.0, 1999.0, 2000.0, 4000.0] {
            let a = csslbw(&r, &bw, e, &ctx);
            let b = seam::slbw_delta(&r, &bw.old, e);
            assert_eq!(a.capture, b.capture, "capture at {e}");
            assert_eq!(a.elastic, b.elastic, "elastic at {e}");
        }
        let lrx = range(ResonanceFormalism::Slbw, 1, 4.0);
        let d = bw_delta(&lrx, &prepare(&lrx), 2000.0, &ctx);
        assert!(d.other[0] > 0.0, "MT=51 at the resonance");
        assert_eq!(d.total, d.elastic + d.fission + d.capture + d.other[0]);
    }

    /// `csmlbw2`'s `enow`: g/h are recomputed on the first call and then only
    /// once the energy passes `min(1.05 enow, enow + 10)`.
    #[test]
    fn csmlbw2_recomputes_gh_past_enow() {
        let ctx = BwContext::new(300.0);
        let r = range(ResonanceFormalism::Mlbw, 0, 3.5);
        let bw = prepare(&r);
        let _ = csmlbw2(&r, &bw, 100.0, &ctx);
        assert_eq!(*ctx.enow.read().unwrap(), 100.0);
        let g100 = bw.gh.read().unwrap()[0][0];
        assert!(g100.0 != 0.0);
        let _ = csmlbw2(&r, &bw, 104.0, &ctx);
        assert_eq!(*ctx.enow.read().unwrap(), 100.0, "104 < 105: kept");
        assert_eq!(bw.gh.read().unwrap()[0][0], g100);
        let _ = csmlbw2(&r, &bw, 105.0, &ctx);
        assert_eq!(*ctx.enow.read().unwrap(), 105.0);
        let _ = csmlbw2(&r, &bw, 1000.0, &ctx);
        let _ = csmlbw2(&r, &bw, 1009.0, &ctx);
        assert_eq!(*ctx.enow.read().unwrap(), 1000.0, "enow + 10 caps the step");
    }

    #[test]
    fn final_lrx_is_the_last_lstate_of_the_top_range() {
        let mut r = range(ResonanceFormalism::Slbw, 1, 4.0);
        assert_eq!(final_lrx(core::slice::from_ref(&r)), 1);
        let mut l1 = r.l_states[0].clone();
        l1.l = 1;
        l1.lrx = 0;
        r.l_states.push(l1);
        assert_eq!(final_lrx(core::slice::from_ref(&r)), 0);
    }
}
