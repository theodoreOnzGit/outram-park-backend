// Ported from NJOY2016 `src/gaminr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! GAMINR group averaging for one material: the per-reaction driver
//! (`gaminr.f90:294-395`), `gpanel` (`:874-1011`), `dspla`'s normalisation
//! (`:1013-1131`), and the total-heating accumulation (`:353-359`,
//! `:400-419`).
//!
//! `gpanel` is GROUPR's `panel` with small differences that matter at the
//! seven-figure level and are kept here: the lower boundary is re-evaluated
//! on an *exact* `elo /= elast` test, the Lobatto span and the flux
//! trapezoid run from the **nudged** `elo` to `ehigh` (not from the panel's
//! `elow` to `ehi`), the quadrature abscissae are not rounded, and the
//! cross section is one-dimensional. The feeders are `gtsig` (`gety1` on
//! the MF=23 section, `:1133-1160`), `gtflx` (the tabulated weight through
//! `terpa` with the `step = 1.05` cap, or the constant weight, `:825-872`)
//! and [`super::gtff`].
//!
//! After every group `dspla` divides each slot by the group flux; the
//! driver then folds the heating slot into the running total heating and
//! drops it (and, for the incoherent matrix, the cross-section slot) before
//! the record is written (`:353-359`). `MT=525`/`621` is the edit of that
//! running total (`:400-419`).

use crate::gaminr::gtff::{
    gtff_coherent, gtff_incoherent, gtff_pair, gtff_vector, ig2pp, PhotonFeed, PhotonTab1, EMAX,
};
use crate::gaminr::weights::WeightTab1;
use crate::groupr::gendf::{GendfGroupRecord, GendfSection};
use crate::groupr::panel::PointwiseXs;
use crate::NjoyError;

/// `rndoff` (`:909`).
const RNDOFF: f64 = 1.000_002;
/// `delta` (`:910`).
const DELTA: f64 = 0.999_995;
/// `small` in `dspla` (`:1027`).
const SMALL: f64 = 1.0e-9;
/// `step` in `gtflx` (`:838`).
const GTFLX_STEP: f64 = 1.05;

const QP2: [f64; 2] = [-1.0, 1.0];
const QW2: [f64; 2] = [1.0, 1.0];
const QP6: [f64; 6] = [
    -1.0,
    -0.765_055_32,
    -0.285_231_52,
    0.285_231_52,
    0.765_055_32,
    1.0,
];
const QW6: [f64; 6] = [
    0.066_666_67,
    0.378_474_96,
    0.554_858_38,
    0.554_858_38,
    0.378_474_96,
    0.066_666_67,
];
const QP10: [f64; 10] = [
    -1.0,
    -0.919_533_908_2,
    -0.738_773_865_1,
    -0.477_924_949_8,
    -0.165_278_957_7,
    0.165_278_957_7,
    0.477_924_949_8,
    0.738_773_865_1,
    0.919_533_908_2,
    1.0,
];
const QW10: [f64; 10] = [
    0.022_222_222_2,
    0.133_305_990_8,
    0.224_889_342_0,
    0.292_042_683_6,
    0.327_539_761_2,
    0.327_539_761_2,
    0.292_042_683_6,
    0.224_889_342_0,
    0.133_305_990_8,
    0.022_222_222_2,
];

/// The weight `gtflx` serves (`:825-872`).
#[derive(Debug, Clone)]
pub enum PhotonFlux {
    /// `iwt = 2`: `flux = 1`, no breaks.
    Constant,
    /// `iwt = 1`/`3`: a TAB1 through `terpa`, breaks capped at `1.05 e`.
    Tabulated(PhotonTab1),
}

impl PhotonFlux {
    /// The built-in `iwt = 3` "1/E with roll-offs" table (`wt1`, `:795-798`).
    pub fn one_over_e_rolloffs() -> Self {
        let t = WeightTab1::one_over_e_rolloffs();
        PhotonFlux::Tabulated(PhotonTab1 {
            z: 0.0,
            interp: t
                .nbt
                .iter()
                .zip(&t.interp)
                .map(|(&n, &i)| (n as u32, i as u32))
                .collect(),
            pairs: t
                .energies_ev
                .iter()
                .copied()
                .zip(t.weights.iter().copied())
                .collect(),
        })
    }

    /// `gtflx` at `e > 0`: `(wtf, enext, idis)`.
    fn gtflx(&self, e: f64) -> (f64, f64, bool) {
        match self {
            PhotonFlux::Constant => (1.0, EMAX, false),
            PhotonFlux::Tabulated(t) => {
                let (wtf, mut enext, mut idis) = t.terpa(e);
                let enxt = GTFLX_STEP * e;
                if enext > enxt {
                    idis = false;
                    enext = enxt;
                }
                (wtf, enext, idis)
            }
        }
    }
}

/// One GAMINR reaction request (card 6 `mfd mtd`).
#[derive(Debug, Clone)]
pub enum PhotonReaction {
    /// `mfd = 23`: a cross section (MT=501/502/504/516) or a cross section
    /// with its heating response (MT=522/602).
    Vector { mt: i32 },
    /// `mfd = 26, mtd = 502`: the coherent matrix with the MF=27/MT=502
    /// form factor.
    Coherent { form_factor: PhotonTab1 },
    /// `mfd = 26, mtd = 504`: the incoherent matrix with the MF=27/MT=504
    /// scattering function.
    Incoherent { scattering_function: PhotonTab1 },
    /// `mfd = 26, mtd = 516`: the pair-production matrix.
    Pair,
}

impl PhotonReaction {
    fn mf_mt(&self) -> (i32, i32) {
        match self {
            PhotonReaction::Vector { mt } => (23, *mt),
            PhotonReaction::Coherent { .. } => (26, 502),
            PhotonReaction::Incoherent { .. } => (26, 504),
            PhotonReaction::Pair => (26, 516),
        }
    }

    fn feed(&self, e: f64, egg: &[f64], nl: usize, ig2pp: usize) -> Result<PhotonFeed, NjoyError> {
        Ok(match self {
            PhotonReaction::Vector { mt } => gtff_vector(e, *mt)?,
            PhotonReaction::Coherent { form_factor } => gtff_coherent(e, egg, nl, form_factor),
            PhotonReaction::Incoherent {
                scattering_function,
            } => gtff_incoherent(e, egg, nl, scattering_function),
            PhotonReaction::Pair => gtff_pair(e, ig2pp),
        })
    }
}

/// The running total heating `toth` (`:147-149`, `:353-359`): per group,
/// the last group flux seen and the accumulated `sum ans(1,1,ng2)*flux`.
#[derive(Debug, Clone, PartialEq)]
pub struct TotalHeating {
    pub flux: Vec<f64>,
    pub heat: Vec<f64>,
}

impl TotalHeating {
    pub fn new(ngg: usize) -> Self {
        TotalHeating {
            flux: vec![0.0; ngg],
            heat: vec![0.0; ngg],
        }
    }

    /// The `MT=525`/`621` edit (`:400-419`): every group, `(flux, heat/flux)`
    /// after `dspla`'s normalisation, written when non-zero or last.
    pub fn section(&self, mt: i32, za: f64, awr: f64) -> GendfSection {
        let ngg = self.flux.len();
        let mut records = Vec::new();
        for ig in 1..=ngg {
            let mut ans: Ans = vec![vec![vec![self.flux[ig - 1]]], vec![vec![self.heat[ig - 1]]]];
            let igzero = dspla(23, mt, ig, &mut ans, 1, 2, 1);
            if igzero || ig == ngg {
                records.push(GendfGroupRecord {
                    ig: ig as i32,
                    ig2lo: 1,
                    ng2: 2,
                    data: vec![ans[0][0][0], ans[1][0][0]],
                });
            }
        }
        GendfSection {
            mf: 23,
            mt,
            za,
            zam: awr,
            nl: 1,
            nz: 1,
            lrflag: 0,
            num_groups: ngg as i32,
            temperature_k: 0.0,
            records,
        }
    }
}

/// `getsig`'s value / next point / duplicate flag for a lin-lin PENDF
/// section (`gety1`): zero below the first point with `enext = x_1`.
fn gety1(pairs: &[(f64, f64)], e: f64) -> (f64, f64, bool) {
    let n = pairs.len();
    if n == 0 || e < pairs[0].0 {
        return (0.0, pairs.first().map(|p| p.0).unwrap_or(EMAX), false);
    }
    if e >= pairs[n - 1].0 {
        return (pairs[n - 1].1, EMAX, false);
    }
    let idx = pairs.partition_point(|&(x, _)| x <= e);
    let (x1, y1) = pairs[idx - 1];
    let (x2, y2) = pairs[idx];
    let y = if x2 == x1 {
        y1
    } else {
        y1 + (y2 - y1) * (e - x1) / (x2 - x1)
    };
    let idis = idx + 1 < n && pairs[idx + 1].0 == x2;
    (y, x2, idis)
}

/// `ans(il, 1, it)` as `ans[it][il]`.
type Ans = Vec<Vec<Vec<f64>>>;

/// `gpanel`'s `save`d state (`:911-916`).
struct PanelState {
    nq: usize,
    nqp: usize,
    enext: f64,
    elast: f64,
    idisc: bool,
    ng1: usize,
    ig1: usize,
    slst: f64,
    flst: Vec<f64>,
    ff: Vec<Vec<f64>>,
}

fn merge(en: f64, idiscf: bool, enext: &mut f64, idisc: &mut bool) {
    if en == *enext && idiscf && !*idisc {
        *idisc = idiscf;
    }
    if en < *enext {
        *idisc = idiscf;
        *enext = en;
    }
}

/// One `gpanel` call (`:874-1011`).
#[allow(clippy::too_many_arguments)]
fn gpanel(
    st: &mut PanelState,
    sigma: &[(f64, f64)],
    flux: &PhotonFlux,
    reaction: &PhotonReaction,
    egg: &[f64],
    nl: usize,
    ig2pp: usize,
    elo_in: f64,
    ehi: &mut f64,
    ans: &mut Ans,
    ng: &mut usize,
    iglo: &mut usize,
) -> Result<(), NjoyError> {
    let mut elo = elo_in;
    if elo > *ehi {
        return Err(NjoyError::EndfParse(format!(
            "gpanel: elo ({elo}) > ehi ({})",
            *ehi
        )));
    }
    // Lower boundary (`:919-935`).
    if elo != st.elast {
        if elo * RNDOFF < *ehi {
            elo *= RNDOFF;
        }
        st.elast = elo;
        let (s, ens, ids) = gety1(sigma, elo);
        st.slst = s;
        st.enext = ens;
        st.idisc = ids;
        let (w, enf, idf) = flux.gtflx(elo);
        st.flst = vec![w; nl];
        merge(enf, idf, &mut st.enext, &mut st.idisc);
        let f = reaction.feed(elo, egg, nl, ig2pp)?;
        st.ff = f.ff;
        st.ng1 = f.ng;
        st.ig1 = f.iglo;
        st.nq = f.nq;
        merge(f.enext, f.idisc, &mut st.enext, &mut st.idisc);
        st.nq += 2;
        if st.nq > 10 {
            st.nq = 10;
        }
    }
    // Upper boundary (`:938-948`).
    let ehigh;
    if st.enext < DELTA * *ehi {
        *ehi = st.enext;
        let mut eh = *ehi;
        if st.idisc && *ehi * DELTA > elo {
            eh = *ehi * DELTA;
        }
        ehigh = eh;
    } else {
        ehigh = DELTA * *ehi;
    }
    let (sig, ens, ids) = gety1(sigma, ehigh);
    st.enext = ens;
    st.idisc = ids;
    let (w, enf, idf) = flux.gtflx(ehigh);
    let fluxv = vec![w; nl];
    let mut en = enf;
    let mut idiscf = idf;
    merge(en, idiscf, &mut st.enext, &mut st.idisc);
    // Group fluxes, linear over the panel (`:951-957`).
    let aq = (ehigh + elo) / 2.0;
    let bq = (ehigh - elo) / 2.0;
    for ((a, &f), &fl) in ans[0][0].iter_mut().zip(&fluxv).zip(&st.flst) {
        *a += (f + fl) * bq;
    }
    // Lobatto quadrature (`:960-993`).
    let (qp, qw): (&[f64], &[f64]) = match st.nq {
        2 => (&QP2, &QW2),
        6 => (&QP6, &QW6),
        10 => (&QP10, &QW10),
        other => return Err(NjoyError::EndfParse(format!("gpanel: bad nq {other}"))),
    };
    for iq in 0..st.nq {
        let mut eq = aq + bq * qp[iq];
        let wq = bq * qw[iq];
        let t1 = (eq - elo) / (*ehi - elo);
        if iq > 0 {
            if eq > ehigh {
                eq = ehigh;
            }
            let f = reaction.feed(eq, egg, nl, ig2pp)?;
            st.ff = f.ff;
            st.ng1 = f.ng;
            st.ig1 = f.iglo;
            st.nqp = f.nq;
            en = f.enext;
            idiscf = f.idisc;
        }
        if *iglo == 0 {
            *iglo = st.ig1;
        }
        for il in 0..nl {
            let a = sig * fluxv[il];
            let b = st.slst * st.flst[il];
            let rr = (b + (a - b) * t1) * wq;
            for ig in 1..=st.ng1 {
                let igt = st.ig1 as i64 + ig as i64 - *iglo as i64 + 1;
                if igt > 1 {
                    let igt = igt as usize;
                    if igt > ans.len() {
                        return Err(NjoyError::EndfParse(format!(
                            "gpanel: secondary slot {igt} exceeds ngg+3 = {}",
                            ans.len()
                        )));
                    }
                    let f = st.ff[il].get(ig - 1).copied().unwrap_or(0.0);
                    ans[igt - 1][0][il] += rr * f;
                    if igt > *ng {
                        *ng = igt;
                    }
                }
            }
        }
    }
    // Save for the next panel (`:997-1009`).
    st.elast = ehigh;
    st.slst = sig;
    st.flst = fluxv;
    merge(en, idiscf, &mut st.enext, &mut st.idisc);
    if st.enext <= *ehi {
        st.enext = RNDOFF * *ehi;
    }
    st.nq = st.nqp + 2;
    if st.nq > 10 {
        st.nq = 10;
    }
    Ok(())
}

/// `dspla` (`:1013-1131`) without the printing: divide the slots by the
/// group flux in place and return `igzero`. For `mfd = 23` a slot below
/// `small` (or a zero flux) is left as it is; for `mfd = 26` sink groups
/// `igt <= ig` are normalised for every order, later slots for `il = 1`
/// only.
fn dspla(
    mfd: i32,
    mtd: i32,
    ig: usize,
    ans: &mut Ans,
    nl: usize,
    ng2: usize,
    ig2lo: usize,
) -> bool {
    let mut igzero = false;
    let flux0 = ans[0][0][0];
    if mfd == 23 {
        for i in 2..=ng2 {
            if flux0 != 0.0 && ans[i - 1][0][0].abs() >= SMALL {
                ans[i - 1][0][0] /= flux0;
                if ans[i - 1][0][0] >= SMALL {
                    igzero = true;
                }
            }
        }
    } else {
        for ig2 in 2..=ng2 {
            let igt = ig2lo + ig2 - 2;
            if (mtd == 516 && ig2 > 2) || igt > ig {
                ans[ig2 - 1][0][0] /= flux0;
                if ans[ig2 - 1][0][0] >= SMALL {
                    igzero = true;
                }
            } else {
                for il in 0..nl {
                    ans[ig2 - 1][0][il] /= ans[0][0][il];
                    if ans[ig2 - 1][0][il] >= SMALL {
                        igzero = true;
                    }
                }
            }
        }
    }
    igzero
}

/// Group-average one reaction for one material (`gaminr.f90:294-395`).
///
/// - `sigma` — the reaction's MF=23 section from the PENDF, lin-lin pairs
///   (`gtsig`); its first energy is the threshold below which groups are
///   skipped (`if (ehi.le.thresh) go to 380`).
/// - `flux` — the weight (`gtflx`).
/// - `egg` — the `ngg + 1` photon group bounds \[eV\].
/// - `lord` — Legendre order; `nl = lord + 1` for MF=26 except MT=516.
/// - `heating` — the running total heating this reaction adds to.
///
/// Returns the GENDF section (`za`/`awr` in the HEAD, `nl`, `nz = 1`) with
/// the heating slot (and the incoherent cross-section slot) already
/// dropped from every record, exactly as written to the GAM-out tape.
#[allow(clippy::too_many_arguments)]
pub fn gaminr_reaction(
    reaction: &PhotonReaction,
    sigma: &PointwiseXs,
    flux: &PhotonFlux,
    egg: &[f64],
    lord: usize,
    za: f64,
    awr: f64,
    heating: &mut TotalHeating,
) -> Result<GendfSection, NjoyError> {
    let PointwiseXs::LinLin(pairs) = sigma else {
        return Err(NjoyError::EndfParse(
            "gaminr_reaction: the MF=23 cross section must be tabulated lin-lin".into(),
        ));
    };
    let pairs: &[(f64, f64)] = pairs;
    let (mfd, mtd) = reaction.mf_mt();
    let ngg = egg.len() - 1;
    let nl = if mfd == 26 && mtd != 516 { lord + 1 } else { 1 };
    let thresh = pairs.first().map(|p| p.0).unwrap_or(EMAX);
    let ig2pp = ig2pp(egg);
    let mut st = PanelState {
        nq: 0,
        nqp: 0,
        enext: EMAX,
        elast: 0.0,
        idisc: false,
        ng1: 0,
        ig1: 0,
        slst: 0.0,
        flst: vec![0.0; nl],
        ff: Vec::new(),
    };
    let mut records = Vec::new();
    for ig in 1..=ngg {
        let mut elo = egg[ig - 1];
        let ehi = egg[ig];
        let mut ig2lo = 0usize;
        let mut ng2 = 2usize;
        if ehi <= thresh {
            continue;
        }
        let mut enext = ehi;
        let mut ans: Ans = vec![vec![vec![0.0; nl]]; ngg + 3];
        loop {
            gpanel(
                &mut st, pairs, flux, reaction, egg, nl, ig2pp, elo, &mut enext, &mut ans,
                &mut ng2, &mut ig2lo,
            )?;
            if enext == ehi {
                break;
            }
            elo = enext;
            enext = ehi;
        }
        let igzero = dspla(mfd, mtd, ig, &mut ans, nl, ng2, ig2lo);
        // Accumulate total heating (`:353-359`).
        if ng2 != 2 && mtd != 502 {
            heating.flux[ig - 1] = ans[0][0][0];
            heating.heat[ig - 1] += ans[ng2 - 1][0][0] * ans[0][0][0];
            ng2 -= 1;
            if mtd == 504 {
                ng2 -= 1;
            }
        }
        if igzero || ig == ngg {
            let mut data = Vec::with_capacity(nl * ng2);
            for slot in ans.iter().take(ng2) {
                data.extend_from_slice(&slot[0]);
            }
            records.push(GendfGroupRecord {
                ig: ig as i32,
                ig2lo: ig2lo as i32,
                ng2: ng2 as i32,
                data,
            });
        }
    }
    Ok(GendfSection {
        mf: mfd,
        mt: mtd,
        za,
        zam: awr,
        nl: nl as i32,
        nz: 1,
        lrflag: 0,
        num_groups: ngg as i32,
        temperature_k: 0.0,
        records,
    })
}
