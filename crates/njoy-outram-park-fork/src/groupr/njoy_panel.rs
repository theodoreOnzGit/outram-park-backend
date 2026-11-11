// Translated from NJOY2016.80 `src/groupr.f90`: `panel` (:5858-6091), `getwtf`
// (:5115-5307, `iwt` 2 and 3), `getflx` (:6439, the `nsigz = 1` branch),
// `getsig` (:6646, the `gety1` branch), `getff` (:7038, the `mfd = 3` vector
// case, label 100) and the `mfd = 3` part of `displa` (:6093, label 310).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! **A faithful `panel` for the vector case** (`nl = nz = 1`, `mfd = 3`), beside
//! the trapezoid [`super::panel::group_integral`] (kept; this does not replace it).
//!
//! What `group_integral` omits and this keeps: the `rndoff = 1.000002` nudge of
//! a panel's lower edge and the `delta = 0.999995` nudge of its upper edge (so
//! the cross section is sampled just inside a discontinuity, `idisc`),
//! `sigfig(eq, 9, 0)` on the quadrature abscissae, the flux integral as the
//! trapezoid of the flux at the nudged edges, the Lobatto order `nq` (2, 6 or
//! 10; for a vector `getff` returns `nq = 0`, so `nq = 2` throughout), the
//! "zero cross section over the whole panel" reset, the thermal-group removal
//! for MT 221-250, and `displa`'s `sigfig(.,7,0)` rounding of flux and ratio.

use crate::endf::gety1::Gety1;
use crate::mixr::mix::sigfig;
use crate::NjoyError;

const RNDOFF: f64 = 1.000002;
const DELTA: f64 = 0.999995;
const EMAX: f64 = 1.0e10;
const SMALL: f64 = 1.0e-10;
const SMIN: f64 = 1.0e-9;
const TTEST: f64 = 0.1;

const QP6: [f64; 6] = [-1.0, -0.76505532, -0.28523152, 0.28523152, 0.76505532, 1.0];
const QW6: [f64; 6] = [0.06666667, 0.37847496, 0.55485838, 0.55485838, 0.37847496, 0.06666667];
const QP10: [f64; 10] = [
    -1.0, -0.9195339082, -0.7387738651, -0.4779249498, -0.1652789577, 0.1652789577, 0.4779249498, 0.7387738651,
    0.9195339082, 1.0,
];
const QW10: [f64; 10] = [
    0.0222222222, 0.1333059908, 0.2248893420, 0.2920426836, 0.3275397612, 0.3275397612, 0.2920426836, 0.2248893420,
    0.1333059908, 0.0222222222,
];

/// `getwtf` for `iwt = 2` (constant) and `iwt = 3` (`1/E`): `(wtf, enext)`.
///
/// # Errors
/// [`NjoyError::NotPorted`] for the other `iwt`.
pub fn getwtf(iwt: i32, e: f64) -> Result<(f64, f64), NjoyError> {
    if e == 0.0 {
        return Ok((0.0, EMAX));
    }
    let wtf = match iwt.abs() {
        2 => 1.0,
        3 => 1.0 / e,
        _ => return Err(NjoyError::NotPorted("groupr getwtf for iwt other than 2 and 3 in the tape driver")),
    };
    Ok((wtf, sigfig(1.01 * e, 6, 1)))
}

/// The cross-section feeder, `getsig` on a PENDF MF=3 TAB1 (`gety1`).
#[derive(Debug, Clone)]
pub struct SigFeed {
    gety: Gety1,
    /// `mtd` of the reaction (for the thermal-group removal).
    pub mtd: i32,
}

impl SigFeed {
    /// `getsig(e = 0)`: returns the feeder and `first`, the first energy.
    pub fn new(tab: &crate::endf::records::Tab1, mtd: i32) -> (Self, f64) {
        let mut g = Gety1::new(tab);
        let first = g.get(0.0).xnext;
        (Self { gety: g, mtd }, first)
    }
    /// `getsig(e)`: `(sigma, enext, idis)`.
    pub fn get(&mut self, e: f64) -> (f64, f64, i32) {
        let v = self.gety.get(e);
        (v.y, v.xnext, i32::from(v.idis))
    }
}

/// `panel`'s saved state for one reaction (`ipan = 0` resets it).
#[derive(Debug, Clone, Default)]
pub struct PanelState {
    elast: f64,
    idisc: i32,
    enext: f64,
    ehigh: f64,
    stmp: f64,
    slst: f64,
    ftmp: f64,
    flst: f64,
}

/// `getflx` at `e`: `(flux, enext, idis)` for `nsigz = 1`.
fn getflx(iwt: i32, e: f64) -> Result<(f64, f64, i32), NjoyError> {
    let (w, en) = getwtf(iwt, e)?;
    Ok((w, en, 0))
}

/// One call of `panel` for `nl = nz = 1`. `ans[1]` is the flux integral and
/// `ans[2]` the rate (index 0 unused, as in the Fortran `ans(1,1,ig)`).
/// `elo`, `ehi` are updated as upstream's by-reference arguments are.
/// Returns an error only from `getwtf`.
pub fn panel(
    st: &mut PanelState,
    sig: &mut SigFeed,
    iwt: i32,
    elo: &mut f64,
    ehi: &mut f64,
    ans: &mut [f64; 3],
    ng: &mut usize,
    iglo: &mut i32,
) -> Result<(), NjoyError> {
    let elow = *elo;
    if *elo > *ehi * (1.0 + SMALL) {
        return Err(NjoyError::InvalidInput("panel: elo.gt.ehi.".into()));
    }
    let (mut en, mut idiscf);
    let mut skip_to_220 = false;
    // getff of the vector case returns en = emax, idisc = 0, nq = 0, ig1 = 1, yld = 1.
    let mut nq_now = 0usize;
    if (*elo - st.elast).abs() >= st.elast * SMALL {
        if *elo * RNDOFF < *ehi {
            *elo *= RNDOFF;
        }
        st.elast = *elo;
        let (f, e1, d1) = getflx(iwt, *elo)?;
        st.flst = f;
        en = e1;
        idiscf = d1;
        let (s, e2, d2) = sig.get(*elo);
        st.slst = s;
        st.enext = e2;
        st.idisc = d2;
        if st.enext >= EMAX * (1.0 - SMALL) {
            skip_to_220 = true;
            en = EMAX;
            idiscf = 0;
        } else {
            if (en - st.enext).abs() < st.enext * SMALL && idiscf > st.idisc {
                st.idisc = idiscf;
            }
            if en < st.enext * (1.0 - SMALL) {
                st.idisc = idiscf;
            }
            if en < st.enext * (1.0 - SMALL) {
                st.enext = en;
            }
            // getff: en = emax, idisc = 0: no change to idisc or enext.
            nq_now = 0 + 2;
        }
    } else {
        en = EMAX;
        idiscf = 0;
        nq_now = 2;
    }
    let _ = (idiscf, nq_now);
    let mut nq = if nq_now == 0 { 2 } else { nq_now };

    if !skip_to_220 {
        // upper boundary
        if st.enext < DELTA * *ehi {
            *ehi = st.enext;
            st.ehigh = *ehi;
            if st.idisc > 0 && *ehi * DELTA > *elo {
                st.ehigh = *ehi * DELTA;
            }
        } else {
            st.ehigh = DELTA * *ehi;
        }
        let (f, e1, d1) = getflx(iwt, st.ehigh)?;
        st.ftmp = f;
        en = e1;
        idiscf = d1;
        let (s, e2, d2) = sig.get(st.ehigh);
        st.stmp = s;
        st.enext = e2;
        st.idisc = d2;
        let mut zero_panel = false;
        if st.enext > EMAX * (1.0 - SMALL) {
            // label 110
            if st.stmp == 0.0 {
                zero_panel = true;
            }
        } else {
            if (en - st.enext).abs() < st.enext * SMALL && idiscf > st.idisc {
                st.idisc = idiscf;
            }
            if en < st.enext * (1.0 - SMALL) {
                st.idisc = idiscf;
            }
            if en < st.enext * (1.0 - SMALL) {
                st.enext = en;
            }
        }
        if zero_panel {
            skip_to_220 = true;
        } else {
            // label 125
            let aq = (*ehi + elow) / 2.0;
            let bq = (*ehi - elow) / 2.0;
            ans[1] += (st.ftmp + st.flst) * bq;
            let mut nqp = 0usize;
            for iq in 1..=nq {
                let (eqq, wq) = match nq {
                    2 => (aq + bq * if iq == 1 { -1.0 } else { 1.0 }, bq),
                    6 => (aq + bq * QP6[iq - 1], bq * QW6[iq - 1]),
                    10 => (aq + bq * QP10[iq - 1], bq * QW10[iq - 1]),
                    _ => return Err(NjoyError::InvalidInput("panel: bad nq in panel".into())),
                };
                let mut eq = sigfig(eqq, 9, 0);
                let t1 = (eq - elow) / (*ehi - elow);
                if iq > 1 {
                    if eq > st.ehigh {
                        eq = st.ehigh;
                    }
                    // getff(eq): en = emax, nq = 0
                    en = EMAX;
                    nqp = 0;
                }
                let _ = eq;
                if *iglo == 0 {
                    *iglo = 1;
                }
                let a = st.stmp * st.ftmp;
                let b = st.slst * st.flst;
                let rr = (b + (a - b) * t1) * wq;
                // ff = yld = 1, ng1 = 1, igt = ig1 + 1 - iglo + 1
                let igt = 1 + 1 - *iglo + 1;
                if igt > 1 {
                    ans[igt as usize] += rr;
                    if igt as usize > *ng {
                        *ng = igt as usize;
                    }
                }
            }
            // thermal-group removal
            if (221..=250).contains(&sig.mtd) && *elo >= TTEST && st.stmp.abs() <= SMIN {
                ans[2] = 0.0;
                *ng = 2;
            }
            nq = nqp + 2;
            let _ = nq;
        }
    }
    if skip_to_220 {
        // label 115: ans = (1, 0), ng = 2, iglo = 1, en = emax (stale stmp/ftmp/ehigh follow into 220)
        ans[1] = 1.0;
        ans[2] = 0.0;
        *ng = 2;
        *iglo = 1;
        en = EMAX;
    }
    // label 220
    st.elast = st.ehigh;
    st.slst = st.stmp;
    st.flst = st.ftmp;
    if (en - st.enext).abs() < st.enext * SMALL && idiscf > st.idisc {
        st.idisc = idiscf;
    }
    if en < st.enext * (1.0 - SMALL) {
        st.idisc = idiscf;
    }
    if en < st.enext * (1.0 - SMALL) {
        st.enext = en;
    }
    if st.enext <= *ehi {
        st.enext = RNDOFF * *ehi;
    }
    Ok(())
}

/// The group loop of `groupr` (`:766-830`) with `displa`'s `mfd = 3` rounding
/// (`:6270-6345`) for one reaction: `(flux, sigma, ig2lo, written)` per group,
/// `written` meaning `igzero != 0 || ig == ngn`.
///
/// `egn` are the `ngn + 1` boundaries, `first` the cross section's first
/// energy, `emaxx` the evaluation-format limit (`:425-431`).
///
/// # Errors
/// From [`panel`].
pub fn group_vector(
    sig: &mut SigFeed,
    iwt: i32,
    egn: &[f64],
    first: f64,
    emaxx: f64,
) -> Result<Vec<(usize, f64, f64, i32)>, NjoyError> {
    let ngn = egn.len() - 1;
    let mut st = PanelState::default();
    let mut out = Vec::new();
    for ig in 1..=ngn {
        let (mut elo, ehi) = (egn[ig - 1], egn[ig]);
        if ehi <= first {
            continue;
        }
        if elo >= emaxx && ig != ngn {
            continue;
        }
        let mut ans = [0.0f64; 3];
        let (mut ng, mut iglo) = (2usize, 0i32);
        let mut enext = ehi;
        loop {
            panel(&mut st, sig, iwt, &mut elo, &mut enext, &mut ans, &mut ng, &mut iglo)?;
            if enext == ehi {
                break;
            }
            elo = enext;
            enext = ehi;
        }
        // displa, label 310 (ig2lo /= 0, nl = nz = 1, itr = 0)
        ans[1] = sigfig(ans[1], 7, 0);
        let mut igzero = false;
        let mut result = 0.0;
        if ans[2] != 0.0 {
            let d = if ans[1] == 0.0 { 1.0e10 } else { ans[1] };
            result = sigfig(ans[2] / d, 7, 0);
            igzero = true;
        }
        if igzero || ig == ngn {
            out.push((ig, ans[1], result, iglo));
        }
    }
    Ok(out)
}
