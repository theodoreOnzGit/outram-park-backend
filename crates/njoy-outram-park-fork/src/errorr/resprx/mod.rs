// Ported from NJOY2016 `src/errorr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
//   - `subroutine resprx`, l.3011-3250 — MF=32 driver: per (isotope, range) group window,
//     format checks, scattering-radius uncertainty (ISR/DAP), dispatch to the
//     unresolved / SAMM / ERRORJ branches.
//   - `subroutine rescon`, l.8513-8819 — add the accumulated resolved (`c**`) and
//     unresolved (`u**`) contributions to one output covariance block.
//   - `subroutine errorr`, l.663-667 (`eskip` defaults) and `egngpn` l.9770-9779
//     (`eskip4` from the finest sub-0.1 eV group).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! MF=32 resonance-parameter covariances — the `resprx` chain
//! (`irespr = 1`, the ERRORJ method NJOY2016 uses for every `LRF ≠ 7`
//! range because it sets `Want_SAMRML_BW = Want_SAMRML_RM = .false.`,
//! `errorr.f90:394-395`).
//!
//! ```text
//! resprx   read MF=2 (rdumrd2) and MF=32 range by range
//!   ├─ rpxunr   LRU=2: 1 % one-sided sensitivities of the URR SLBW averages   (unresolved.rs)
//!   ├─ rpxsamm  LRU=1, LRF=7: analytic SAMM derivatives, LCOMP=1/2 covariance (sammy.rs)
//!   └─ rpxlc12  LRU=1, LCOMP=1/2: central-difference MLBW sensitivities       (resolved.rs)
//!        rpxlc2   compact (LCOMP=2) covariance                                  (resolved.rs)
//!        rpendf   pointwise σ on the eskip grid -> ggmlbw                       (mlbw.rs)
//!        rpxgrp   simplistic group average with the egtwtf weight               (group.rs)
//! rescon   fold c**/u** (or the SAMM crr) into each output (MT, MT1) block     (this file)
//! ```
//!
//! The accumulators are the upstream module arrays: `cff/cgg/cee/ctt`
//! (packed upper triangles, `ngn(ngn+1)/2`) and `cfg/cef/ceg` (full
//! `ngn²`) for the resolved contribution, `u**` likewise for the
//! unresolved one. They hold `Σ cov(p,q) (∂σ_g/∂p)(∂σ_g'/∂q)` already
//! multiplied by `cflx(g) cflx(g') ABN²`, so `covout`'s division by the
//! group fluxes yields the absolute group covariance (and by `σ_g σ_g'`
//! the relative one).
//!
//! **Validated** on TENDL-2023 Ar-37 (MLBW `LCOMP=2` with `DAP`, plus an
//! `LRU=2` block) and on ENDF/B-VII.1 Cl-35 (`LRF=7` `LCOMP=2` with INTG
//! correlations, through `rpxsamm`) against the NJOY2016 binary —
//! `tests/errorr_mf32_ar37_golden.rs`, `tests/errorr_mf32_cl35_rml_golden.rs`.
//! **Refused** (`NotPorted`): `LCOMP=0` (`rpxlc0`), `LRF=1` and `LRF=3`
//! resolved sensitivities, `NRO ≠ 0`, `NLRS > 0`, INTG correlations in
//! the ERRORJ branch (`rpxlc2`, `NM > 0`), `irespr = 0` (`resprp`), an
//! `LRU=2/LRF=1` MF=2 URR (upstream reads uninitialised `amur`), and an
//! `LRF=3` range in a material that also has an `LRF=7` one.

pub mod group;
pub mod mf2;
pub mod mlbw;
pub mod resolved;
pub mod sammy;
pub mod unresolved;

use crate::endf::records::SectionCursor;
use crate::endf::tape::Tape;
use crate::NjoyError;

use super::weight::ErrorrWeight;
use mf2::Mf2Resonances;

/// `eskip1..4` — the ERRORJ pointwise-grid step ratios (`errorr.f90:663-667`
/// with the `mfcov = -33/333` speed-ups not offered; `eskip4` is refined by
/// [`Eskip::from_groups`]).
#[derive(Debug, Clone, Copy)]
pub struct Eskip {
    /// Within ±10 % of a resonance.
    pub e1: f64,
    /// Within ±20 %.
    pub e2: f64,
    /// Within ±30 %.
    pub e3: f64,
    /// Below 0.1 eV.
    pub e4: f64,
}

impl Eskip {
    /// Defaults, with `eskip4` halved towards the finest group ratio below
    /// 0.1 eV (`egngpn`, `errorr.f90:9770-9779`).
    pub fn from_groups(egn: &[f64]) -> Self {
        let mut ewmin = 1.05f64;
        for w in egn.windows(2) {
            if w[1] < 0.1 {
                let ew = w[1] / w[0];
                if ew < ewmin {
                    ewmin = ew;
                }
            }
        }
        let mut e4 = 1.05;
        if ewmin < 1.05 {
            e4 = 1.0 + 0.5 * (ewmin - 1.0);
        }
        Eskip {
            e1: 1.00002,
            e2: 1.0003,
            e3: 1.005,
            e4,
        }
    }
}

/// Per-range state `resprx` hands to the branch routines (the upstream
/// module globals set in `errorr.f90:3079-3225`).
#[derive(Debug, Clone)]
pub struct RangeParams {
    /// `LRU`, `LRF`, `LCOMP`, `NAPS` of the MF=32 range.
    pub lru: i32,
    pub lrf: i32,
    pub lcomp: i32,
    pub naps: i32,
    /// `NLS` after the MF=2/MF=32 consistency check.
    pub nls: usize,
    /// `ISR` (after the user `dap` override), `DAP`, `dap3(1:nls)`.
    pub isr: i32,
    pub dap: f64,
    pub dap3: Vec<f64>,
    /// `SPI`, `AP`, `ABN`, `LFW`.
    pub spi: f64,
    pub ap: f64,
    pub abn: f64,
    pub lfw: i32,
    /// Range bounds `elr`/`ehr` \[eV\] and the enclosing user-group
    /// bounds `elg`/`ehg`.
    pub elr: f64,
    pub ehr: f64,
    pub elg: f64,
    pub ehg: f64,
    /// First and last user group (1-based) the range touches; `ieed = 0`
    /// when the range lies entirely below the first group.
    pub iest: usize,
    pub ieed: usize,
}

/// The MF=32 contribution accumulators (`errorr.f90:7084-7099`), plus the
/// flags `rescon` keys on.
#[derive(Debug, Clone)]
pub struct ResonanceCovariance {
    /// `ngn`.
    pub ngn: usize,
    /// Resolved: packed upper triangles (`igind` over `ig <= ig2`).
    pub cff: Vec<f64>,
    pub cgg: Vec<f64>,
    pub cee: Vec<f64>,
    pub ctt: Vec<f64>,
    /// Resolved cross terms, `(ig-1)*ngn + ig2`.
    pub cfg: Vec<f64>,
    pub cef: Vec<f64>,
    pub ceg: Vec<f64>,
    /// Unresolved counterparts.
    pub uff: Vec<f64>,
    pub ugg: Vec<f64>,
    pub uee: Vec<f64>,
    pub utt: Vec<f64>,
    pub ufg: Vec<f64>,
    pub uef: Vec<f64>,
    pub ueg: Vec<f64>,
    /// `nresg` — highest group (1-based) any range reached.
    pub nresg: usize,
    /// `ifresr` / `ifunrs` — a resolved / unresolved range was processed.
    pub ifresr: bool,
    pub ifunrs: bool,
    /// `nmtres > 0` — the SAMMY branch of `rescon` applies: [`Self::crr`]
    /// is added to every block and the `c**`/`u**` accumulators are
    /// ignored (upstream, `errorr.f90:8528`).
    pub sammy: bool,
    /// `crr(ig, ig2, ix, ixp)` from `rpxsamm`, flattened
    /// `((ig·ngn + ig2)·nmt + ix)·nmt + ixp` (0-based); empty unless `sammy`.
    pub crr: Vec<f64>,
    /// `nmt` the `crr` layout was built with.
    pub nmt: usize,
    /// The `mess` lines upstream prints.
    pub messages: Vec<String>,
}

impl ResonanceCovariance {
    /// All-zero accumulators for `ngn` groups (`resprx`, `errorr.f90:3042-3061`).
    pub fn new(ngn: usize) -> Self {
        let nngn = ngn * (ngn + 1) / 2;
        let ngn2 = ngn * ngn;
        ResonanceCovariance {
            ngn,
            cff: vec![0.0; nngn],
            cgg: vec![0.0; nngn],
            cee: vec![0.0; nngn],
            ctt: vec![0.0; nngn],
            cfg: vec![0.0; ngn2],
            cef: vec![0.0; ngn2],
            ceg: vec![0.0; ngn2],
            uff: vec![0.0; nngn],
            ugg: vec![0.0; nngn],
            uee: vec![0.0; nngn],
            utt: vec![0.0; nngn],
            ufg: vec![0.0; ngn2],
            uef: vec![0.0; ngn2],
            ueg: vec![0.0; ngn2],
            nresg: 0,
            ifresr: false,
            ifunrs: false,
            sammy: false,
            crr: Vec::new(),
            nmt: 0,
            messages: Vec::new(),
        }
    }

    /// Packed index of `(ig, ig2)`, both 1-based with `ig <= ig2`
    /// (`ngn*(ig-1) - (ig-1)*(ig-2)/2 + (ig2-ig)`).
    pub fn packed(&self, ig: usize, ig2: usize) -> usize {
        let n = self.ngn;
        n * (ig - 1) - (ig - 1) * (ig.saturating_sub(2)) / 2 + (ig2 - ig)
    }

    /// The diagonal `(ig, ig)` of one accumulator divided by `cflx(ig)²`
    /// — the *absolute* "resolved"/"unresolve" contribution the listing
    /// prints (`covout`, `errorr.f90:7660-7710`; for `irelco = 1` the
    /// listing then divides by `csig(ig,ix) csig(ig,ixp)`, l.7712-7721).
    /// `which` is `"tt"`, `"ee"`, `"eg"`, `"ef"`, `"ff"`, `"fg"` or
    /// `"gg"`; `unresolved` selects the `u**` set.
    pub fn diagonal(&self, which: &str, unresolved: bool, cflx: &[f64]) -> Vec<f64> {
        let n = self.ngn;
        (1..=n)
            .map(|ig| {
                let p = self.packed(ig, ig);
                let sq = (ig - 1) * n + (ig - 1);
                let v = match (which, unresolved) {
                    ("tt", false) => self.ctt[p],
                    ("ee", false) => self.cee[p],
                    ("eg", false) => self.ceg[sq],
                    ("ef", false) => self.cef[sq],
                    ("ff", false) => self.cff[p],
                    ("fg", false) => self.cfg[sq],
                    ("gg", false) => self.cgg[p],
                    ("tt", true) => self.utt[p],
                    ("ee", true) => self.uee[p],
                    ("eg", true) => self.ueg[sq],
                    ("ef", true) => self.uef[sq],
                    ("ff", true) => self.uff[p],
                    ("fg", true) => self.ufg[sq],
                    ("gg", true) => self.ugg[p],
                    _ => 0.0,
                };
                v / (cflx[ig - 1] * cflx[ig - 1])
            })
            .collect()
    }

    /// Add this material's MF=32 contribution to the `(mt, mat1/mt1)`
    /// block `cova` (`[ig][igp]`, `ngn × ngn`) — `rescon`,
    /// `errorr.f90:8513-8819` for `irespr = 1`. `ix`/`ixp` are the
    /// reactions' 0-based positions in `mts` (the SAMMY branch indexes
    /// `crr` by them and applies to every block, `errorr.f90:8800-8806`).
    /// Returns `true` when any diagonal element is non-zero afterwards
    /// (upstream's `izero`), or `false` untouched when the pair is not
    /// one of the seven the ERRORJ branch covers.
    pub fn rescon(
        &self,
        ix: usize,
        ixp: usize,
        mt: i32,
        mat1: i32,
        mt1: i32,
        cova: &mut [f64],
    ) -> bool {
        let ngn = self.ngn;
        if self.sammy {
            // cova(ig,ig2) += crr(ig,ig2,ix,ixp) (errorr.f90:8802-8806); the
            // crate's cova[(a-1)*ngn+(b-1)] is the Fortran cova(b,a) (see
            // add_tri/add_sq below), hence the transposed store.
            let nmt = self.nmt;
            for ig in 0..ngn {
                for ig2 in 0..ngn {
                    cova[ig2 * ngn + ig] += self.crr[((ig * ngn + ig2) * nmt + ix) * nmt + ixp];
                }
            }
            return (0..ngn).any(|ig| cova[ig * ngn + ig] != 0.0);
        }
        if mat1 != 0 {
            return false;
        }
        let itp = match (mt, mt1) {
            (18, 18) => 1,
            (18, 102) => 2,
            (102, 102) => 3,
            (2, 2) => 4,
            (2, 18) => 5,
            (2, 102) => 6,
            (1, 1) => 7,
            _ => return false,
        };
        let iglast = ngn.min(self.nresg);
        // cova(ig2, ig) in Fortran is cova[(ig-1)*ngn + (ig2-1)] here
        let add_tri = |cova: &mut [f64], v: &[f64], last: usize| {
            let mut igind = 0usize;
            for ig in 1..=last {
                for ig2 in ig..=ngn {
                    cova[(ig - 1) * ngn + (ig2 - 1)] += v[igind];
                    if ig != ig2 {
                        cova[(ig2 - 1) * ngn + (ig - 1)] += v[igind];
                    }
                    igind += 1;
                }
            }
        };
        let add_sq = |cova: &mut [f64], v: &[f64]| {
            for ig in 1..=ngn {
                for ig2 in 1..=ngn {
                    let igd = (ig - 1) * ngn + (ig2 - 1);
                    cova[(ig - 1) * ngn + (ig2 - 1)] += v[igd];
                }
            }
        };
        if self.ifresr {
            match itp {
                1 => add_tri(cova, &self.cff, iglast),
                2 => add_sq(cova, &self.cfg),
                3 => add_tri(cova, &self.cgg, ngn),
                4 => add_tri(cova, &self.cee, ngn),
                5 => add_sq(cova, &self.cef),
                6 => add_sq(cova, &self.ceg),
                7 => add_tri(cova, &self.ctt, ngn),
                _ => {}
            }
        }
        if self.ifunrs {
            // irespr = 1: no relative-to-absolute conversion (label 1090)
            match itp {
                1 => add_tri(cova, &self.uff, iglast),
                2 => add_sq(cova, &self.ufg),
                3 => add_tri(cova, &self.ugg, iglast),
                4 => add_tri(cova, &self.uee, iglast),
                5 => add_sq(cova, &self.uef),
                6 => add_sq(cova, &self.ueg),
                7 => add_tri(cova, &self.utt, iglast),
                _ => {}
            }
        }
        (0..ngn).any(|ig| cova[ig * ngn + ig] != 0.0)
    }
}

/// The user-group window of a resonance range (`resprx`,
/// `errorr.f90:3085-3104`): `(elg, ehg, iest, ieed)`.
fn group_window(egn: &[f64], elr: f64, ehr: f64) -> (f64, f64, usize, usize) {
    let ngn = egn.len() - 1;
    let mut elg = elr;
    let mut ehg = ehr;
    let mut iest = 0usize;
    let mut ieed = 0usize;
    let (mut ip1, mut ip2) = (false, false);
    for i in 2..=ngn + 1 {
        let ee = egn[i - 1];
        if !ip1 && ee > elr {
            ip1 = true;
            elg = egn[i - 2];
            iest = i - 1;
        }
        if !ip2 && ee >= ehr {
            ip2 = true;
            ehg = egn[i - 1];
            ieed = i - 1;
        }
    }
    if elg >= ehr {
        ieed = 0;
    }
    (elg, ehg, iest, ieed)
}

/// Prepare the resonance-parameter contributions to the coarse-group
/// covariances (`resprx`, `errorr.f90:3011-3250`, `mfcov = 33`,
/// `irespr = 1`).
///
/// `cflx` are the coarse-group flux integrals from `sigc`; `dap_user`
/// is the deck's `dap` card (`isru`), `0` to take `DAP` from the file.
///
/// # Errors
/// `SectionNotFound` without MF=2 or MF=32; `EndfParse` for the upstream
/// fatal format checks (`NRO ≠ 0`, `LRU/LRF` combinations "with no
/// coding", an MF=2/MF=32 L-state mismatch, an unresolvable MF=32
/// resonance); `NotPorted` for the branches listed in the module docs.
#[allow(clippy::too_many_arguments)]
pub fn resprx(
    endf: &Tape,
    matd: i32,
    egn: &[f64],
    cflx: &[f64],
    weight: &ErrorrWeight,
    tempin: f64,
    dap_user: f64,
    sammy_ctx: &sammy::SammyContext,
) -> Result<ResonanceCovariance, NjoyError> {
    let ngn = egn.len() - 1;
    let mut rc = ResonanceCovariance::new(ngn);
    let eskip = Eskip::from_groups(egn);
    let mf2 = Mf2Resonances::read(endf, matd)?;
    // s2sammy (errorr.f90:796-808): nmtres > 0 once MF=2 has an LRF=7 range
    let mmtres = sammy::mmtres_of(&mf2)?;
    let sec = endf
        .section(matd, 32, 151)
        .ok_or(NjoyError::SectionNotFound {
            mat: matd,
            mf: 32,
            mt: 151,
        })?;
    let raw = endf.raw_mf32_lines(matd, 151);
    let mut cur = SectionCursor::new(&sec.rows);
    let head = cur.read_cont()?;
    let nis = head.n1;
    let isru = dap_user != 0.0;
    let mut indx = 0usize;
    for _isrr in 0..nis {
        let iso = cur.read_cont()?;
        let abn = iso.c2;
        let lfw = iso.l2;
        let ner = iso.n1;
        for _ier in 0..ner {
            indx += 1;
            let r = cur.read_cont()?;
            let (elr, ehr) = (r.c1, r.c2);
            let (lru, lrf, nro, naps) = (r.l1, r.l2, r.n1, r.n2);
            let (elg, ehg, iest, ieed) = group_window(egn, elr, ehr);
            let legal =
                (lru == 1 && (1..=7).contains(&lrf)) || (lru == 2 && (1..=2).contains(&lrf));
            if !legal {
                return Err(NjoyError::EndfParse(format!(
                    "errorr::resprx: illegal or no coding data structure in mf32 (lrf={lrf} lru={lru})"
                )));
            }
            if nro != 0 {
                return Err(NjoyError::EndfParse(format!(
                    "errorr::resprx: illegal or unrecognized data structure in mf32 (nro={nro})"
                )));
            }
            let sp = cur.read_cont()?;
            let spi = sp.c1;
            let ap = sp.c2;
            let lcomp = sp.l2;
            let mf2r = mf2.ranges.get(indx - 1).ok_or_else(|| {
                NjoyError::EndfParse("errorr::resprx: more MF=32 ranges than MF=2 ranges".into())
            })?;
            let mut nls = mf2r.nls;
            if lrf == 7 {
                // errorr.f90:3145-3147: nls is NJS from MF=32 itself
                nls = sp.n1;
            } else if sp.n1 > nls {
                return Err(NjoyError::EndfParse(
                    "errorr::resprx: mf2/mf32 l-state mis-match (probable evaluation file error)"
                        .into(),
                ));
            } else if sp.n1 < nls {
                rc.messages.push(format!(
                    "resprx: mf2 nls={nls}, but mf32 nls={} — continue with partial urr covariance data",
                    sp.n1
                ));
                nls = sp.n1;
            }
            let nls = nls.max(0) as usize;

            // scattering radius uncertainty (l.3150-3224)
            let mut isr = sp.n2;
            let mut dap = 0.0;
            let mut dap3 = vec![0.0f64; nls];
            if lrf == 7 {
                // errorr.f90:3221-3223
                if isr != 0 || isru {
                    rc.messages.push("resprx: scat. radius unc not ready for lrf=7".into());
                }
            } else if isr == 1 {
                if lrf == 1 || lrf == 2 {
                    let c = cur.read_cont()?;
                    dap = if !isru { c.c2 } else { ap * dap_user };
                    dap3.iter_mut().for_each(|d| *d = dap);
                } else if lrf == 3 {
                    let l = cur.read_list()?;
                    let (mls, d) = if !isru {
                        (l.head.n1, *l.data.first().unwrap_or(&0.0))
                    } else {
                        (1, ap * dap_user)
                    };
                    dap = d;
                    let nls_i = nls as i32;
                    if mls == 1 {
                        dap3.iter_mut().for_each(|v| *v = dap);
                    } else if mls > 1 && mls == nls_i + 1 {
                        for (i, v) in dap3.iter_mut().enumerate() {
                            *v = l.data[i + 1];
                        }
                    } else if mls > 1 && mls < nls_i + 1 {
                        for (i, v) in dap3.iter_mut().enumerate() {
                            *v = if (i as i32) < mls - 1 {
                                l.data[i + 1]
                            } else {
                                dap
                            };
                        }
                    } else {
                        rc.messages.push(format!(
                            "resprx: mls={mls}, nls={nls} are inconsistent — will ignore scattering radius uncertainty"
                        ));
                        isr = 0;
                        dap = 0.0;
                    }
                } else {
                    return Err(NjoyError::NotPorted(
                        "errorr::resprx: not ready for isr=1 with this lrf",
                    ));
                }
            } else if isr == 0 && isru {
                isr = 1;
                dap = ap * dap_user;
                dap3.iter_mut().for_each(|d| *d = dap);
            } else if isr != 0 {
                return Err(NjoyError::EndfParse(format!(
                    "errorr::resprx: illegal isr {isr}"
                )));
            }
            if isr != 0 && isru {
                rc.messages.push(format!(
                    "resprx: user override for scattering radius uncertainty — use DAP = {:.3}*AP for all L",
                    dap / ap
                ));
            }

            let rp = RangeParams {
                lru,
                lrf,
                lcomp,
                naps,
                nls,
                isr,
                dap,
                dap3,
                spi,
                ap,
                abn,
                lfw,
                elr,
                ehr,
                elg,
                ehg,
                iest,
                ieed,
            };

            if lru == 2 {
                // the URR degrees of freedom come only from an LRF=2 MF=2 URR
                if mf2r.range.l2 != 2 {
                    return Err(NjoyError::NotPorted(
                        "errorr::rpxunr: MF=2 URR is not lrf=2 — upstream reads uninitialised amur",
                    ));
                }
                let a0 = [
                    sp.c1,
                    sp.c2,
                    sp.l1 as f64,
                    sp.l2 as f64,
                    sp.n1 as f64,
                    sp.n2 as f64,
                ];
                unresolved::rpxunr(
                    &mut cur, a0, &rp, &mf2.amur, egn, cflx, weight, tempin, &mut rc,
                )?;
            } else if let Some(mm) = &mmtres {
                // resolved with the sammy method (errorr.f90:3231-3233)
                sammy::rpxsamm(
                    &mut cur, raw, &rp, &mf2, mf2r, mm, egn, weight, tempin, sammy_ctx, &mut rc,
                )?;
            } else {
                match lcomp {
                    0 => {
                        return Err(NjoyError::NotPorted(
                            "errorr::resprx: lcomp=0 (rpxlc0) not ported",
                        ))
                    }
                    1 | 2 => resolved::rpxlc12(
                        &mut cur, &rp, mf2r, egn, cflx, weight, tempin, &eskip, &mut rc,
                    )?,
                    _ => {
                        return Err(NjoyError::EndfParse(format!(
                            "errorr::resprx: illegal lcomp={lcomp}"
                        )))
                    }
                }
            }
        }
    }
    Ok(rc)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `group_window` on the LANL 30-group bounds for Ar-37's two ranges.
    ///
    /// **Result (2026-09-10).** Resolved `1e-5..4268.035` eV -> groups
    /// 1..12 (`elg = egn(1)`, `ehg = 9120`); unresolved `4268.035..5548.45`
    /// -> group 12 only (`elg = 3350`).
    #[test]
    fn group_window_matches_resprx_bookkeeping() {
        let egn = [
            1.39e-4, 0.152, 0.414, 1.13, 3.06, 8.32, 22.6, 61.4, 167.0, 454.0, 1235.0, 3350.0,
            9120.0, 24800.0,
        ];
        let (elg, ehg, iest, ieed) = group_window(&egn, 1.0e-5, 4268.035);
        assert_eq!((elg, ehg, iest, ieed), (1.39e-4, 9120.0, 1, 12));
        let (elg, ehg, iest, ieed) = group_window(&egn, 4268.035, 5548.45);
        assert_eq!((elg, ehg, iest, ieed), (3350.0, 9120.0, 12, 12));
        // a range below the first group: ieed = 0
        let (_, _, _, ieed) = group_window(&egn, 1.0e-6, 1.0e-5);
        assert_eq!(ieed, 0);
    }

    /// `packed` reproduces the Fortran `igind` counter and the listing's
    /// `ngn*(ig-1)-(ig-1)*(ig-2)/2+1` diagonal index (0-based here).
    #[test]
    fn packed_index_is_the_igind_counter() {
        let rc = ResonanceCovariance::new(5);
        let mut igind = 0;
        for ig in 1..=5 {
            for ig2 in ig..=5 {
                assert_eq!(rc.packed(ig, ig2), igind, "ig={ig} ig2={ig2}");
                igind += 1;
            }
        }
    }

    /// `rescon` adds the packed triangle symmetrically, the square cross
    /// terms as stored, nothing for pairs outside the seven, and reports
    /// `izero` from the diagonal.
    #[test]
    fn rescon_places_triangle_and_square_terms() {
        let mut rc = ResonanceCovariance::new(3);
        rc.ifresr = true;
        rc.nresg = 3;
        rc.cee = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]; // (1,1)(1,2)(1,3)(2,2)(2,3)(3,3)
        rc.ceg = (1..=9).map(|v| v as f64 * 10.0).collect();
        let mut cova = vec![0.0; 9];
        assert!(rc.rescon(0, 0, 2, 0, 2, &mut cova));
        assert_eq!(cova, vec![1.0, 2.0, 3.0, 2.0, 4.0, 5.0, 3.0, 5.0, 6.0]);
        let mut cova = vec![0.0; 9];
        assert!(rc.rescon(0, 1, 2, 0, 102, &mut cova));
        assert_eq!(cova, (1..=9).map(|v| v as f64 * 10.0).collect::<Vec<_>>());
        let mut cova = vec![0.0; 9];
        assert!(!rc.rescon(0, 1, 2, 0, 4, &mut cova));
        assert!(cova.iter().all(|&v| v == 0.0));
        assert!(!rc.rescon(0, 1, 2, 1, 2, &mut cova));
    }

    /// The SAMMY branch adds `crr(ig, ig2, ix, ixp)` to any block, and
    /// ignores the `c**` accumulators (`errorr.f90:8528`).
    #[test]
    fn rescon_sammy_branch_uses_crr_for_every_pair() {
        let mut rc = ResonanceCovariance::new(2);
        rc.sammy = true;
        rc.nmt = 2;
        rc.cee = vec![9.0, 9.0, 9.0];
        // crr[(ig,ig2,ix,ixp)] = 1000*ig + 100*ig2 + 10*ix + ixp
        rc.crr = (0..16)
            .map(|k| {
                let (ig, ig2, ix, ixp) = (k / 8, (k / 4) % 2, (k / 2) % 2, k % 2);
                (1000 * ig + 100 * ig2 + 10 * ix + ixp) as f64
            })
            .collect();
        let mut cova = vec![0.0; 4];
        assert!(rc.rescon(1, 0, 4, 0, 2, &mut cova));
        // crate cova[(ig2)*ngn + ig] = Fortran cova(ig, ig2) = crr(ig, ig2, 1, 0)
        assert_eq!(cova, vec![10.0, 1010.0, 110.0, 1110.0]);
    }
}
