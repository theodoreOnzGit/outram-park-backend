// Ported from NJOY2016 `src/groupr.f90` (git commit ab27c64):
//   - `conver`'s MF=1 projectile and `LAW = 5` reading, l.11473-11483, 11564-11624;
//   - `getmf6`'s initialisation for `mfd = 3`, l.7570-7633, 7936-7940;
//   - `getfle` with `mfd = 8`, l.9729-9760;
//   - `getco`'s retrieve branch and label 360 (Coulomb amplitude expansion),
//     l.10541-10580, 10583-10700.
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! GROUPR's elastic cross section for an **incident charged particle**
//! (`mfd = 3`, `mtd = 2`, `izap > 1`; gh:#1370, 2026-11-24).
//!
//! Upstream routes it through `getff` label 800 (`groupr.f90:7063`): `getmf6`
//! finds the MF=6/MT=2 subsection of the projectile (`zad = izap`), whose
//! `LAW = 5` sends it to `getdis` (`:7936`, `:8096`). `getdis` reads the angular
//! records through `getfle` with `mfd = 8` and adds the Coulomb term
//! ([`crate::groupr::two_body::ChargedElastic`]); `getsig` gives 1 on a grid
//! `1.1 e` apart (`:6773-6780`), so the group value is the integral of the
//! angular distribution over the cosines up to `wcut`.
//!
//! This module reads the records the way `getfle` and `getco` do:
//!
//! - `LTP = 0`: `getco`'s retrieve branch for `lang /= 2` (`fl(1) = 1`,
//!   `fl(il) = c(il+5)`, `:10554-10567`);
//! - `LTP = 2`: `fl(il) = c(il+6)` (`:10568-10578`);
//! - `LTP = 1`: the 64-point quadrature of label 360, the nuclear-plus-interference
//!   amplitude expansion with the Coulomb phase. Upstream builds each complex
//!   coefficient with `cmplx(re, im)`, which has **default (single) kind**, so the
//!   coefficients are rounded to `f32` first, and `cmplx(...)/2` halves them in
//!   single precision; this port does the same.
//!
//! **Refused by name, `LTP > 2`** (tabulated, He-4 and p + H-2 in ENDF/B-VIII.0).
//! `conver` rewrites such a record as Legendre coefficients through `getco`'s
//! label 352 (`:10615-10627`), and that loop reads `c(ipc+7)` past the record's
//! last cosine for every Gauss node above it, up to `ipm = 6+2 nmu`, so it reads
//! words of `conver`'s `scr` array left from earlier records (He-4 tabulates to
//! `mu = 0.94`; 6 of the 64 nodes lie above). Those values are not defined by the
//! record. Also refused: a target ZA of 0, `LCT = 1`, and an `MT = 2` subsection
//! for the projectile that is not `LAW = 5`.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use crate::common::phys::{AMASSN_AMU as AMASSN, AMU_G as AMU, CLIGHT_CM_S as CLIGHT, EV_ERG as EV, HBAR_ERG_S as HBAR};
use crate::endf::records::SectionCursor;
use crate::endf::tape::Tape;
use crate::groupr::file4::{gauss64, getco_law5_legendre, getco_legendre, File4Angular};
use crate::groupr::kinematics::shared::legndr;
use crate::groupr::two_body::ChargedElastic;
use crate::NjoyError;
use petir::mathf::RealMath;

/// `getco`'s `toler` (`groupr.f90:10530`).
const TOLER: f64 = 1.0e-8;

/// The projectile `conver` reads from MF=1/MT=451 (`groupr.f90:11475-11483`):
/// `(izap, awrp)`, `(1, 1)` for an ENDF-5 tape or one without the third record;
/// for ENDF-6, `izap = NSUB/10` and `awrp = AWI`.
pub fn projectile(tape: &Tape, mat: i32, iverf: i32) -> (i32, f64) {
    if iverf < 6 {
        return (1, 1.0);
    }
    match tape.section(mat, 1, 451).and_then(|s| s.rows.get(2)) {
        Some(r) => ((r[4].round() as i32) / 10, r[0]),
        None => (1, 1.0),
    }
}

/// What `getmf6`'s initialisation sets for the `mfd = 3` elastic of an incident
/// charged particle, with the angular records ready for `getdis`.
#[derive(Debug, Clone)]
pub struct ChargedLaw5 {
    /// The records as `getfle` (`mfd = 8`) returns them.
    pub angular: File4Angular,
    /// `izap`, `izat`, `awrp`, `spi` for `getdis`'s Coulomb term.
    pub constants: ChargedElastic,
    /// `aprime = awp/awrp` (`:7629-7630`).
    pub aprime: f64,
}

/// `getmf6` then `getfle` with `mfd = 8` on the MF=6/MT=2 subsection of `mat`
/// whose `ZAP` is `zad` (`izap` itself for `mfd = 3`, `:7585`; `1001 .. 2004` for
/// `mfd = 21-25`, `:7587-7591`), for the projectile `izap` with AWR `awrp`,
/// `nl_max` coefficients.
///
/// # Errors
/// [`NjoyError::SectionNotFound`] without MF=6/MT=2; [`NjoyError::EndfParse`]
/// when no subsection is the projectile (upstream: "desired particle not found");
/// [`NjoyError::NotPorted`] by name for what the module doc lists.
pub fn from_mf6_mt2(tape: &Tape, mat: i32, zad: i32, izap: i32, awrp: f64, nl_max: usize) -> Result<ChargedLaw5, NjoyError> {
    let sec = tape.section(mat, 6, 2).ok_or(NjoyError::SectionNotFound { mat, mf: 6, mt: 2 })?;
    let mut cur = SectionCursor::new(&sec.rows);
    let head = cur.read_cont()?;
    let izat = head.c1.round() as i32;
    // `lct3 = 3`: `lct = 2` for a light particle (`:7624-7627`); the projectile is one.
    if head.l2 != 2 && !(head.l2 == 3 && (1..=2004).contains(&izap)) {
        return Err(NjoyError::NotPorted("groupr getmf6: charged-particle elastic with LCT = 1 (getco: lab to cm conversion not coded)"));
    }
    if izat / 1000 == 0 {
        return Err(NjoyError::NotPorted("groupr getdis: charged-particle elastic on a target with Z = 0"));
    }
    for _ in 0..head.n1.max(0) {
        let t = cur.read_tab1()?;
        let law = t.head.l2;
        if t.head.c1.round() as i32 != zad {
            crate::acer::energy::mf6::skip_mf6_subsection(&mut cur, law)?;
            continue;
        }
        if law != 5 {
            return Err(NjoyError::NotPorted("groupr getmf6: for a charged projectile, an MF=6/MT=2 subsection that is not LAW = 5 (e.g. the LAW = 4 recoil)"));
        }
        let aprime = if awrp != 0.0 { t.head.c2 / awrp } else { t.head.c2 };
        let tab2 = cur.read_tab2()?;
        let spi = tab2.head.c1;
        let lidp = tab2.head.l1;
        let constants = ChargedElastic { izap, izat, awrp, spi };
        let ne = tab2.head.n2.max(0) as usize;
        let mut ir = 0usize;
        let mut recs = Vec::with_capacity(ne);
        for nne in 1..=ne {
            while ir + 1 < tab2.interp.len() && nne as u32 > tab2.interp[ir].0 {
                ir += 1;
            }
            let elaw = tab2.interp.get(ir).map(|r| r.1).unwrap_or(2);
            let list = cur.read_list()?;
            let ltp = list.head.l1;
            let fl = match ltp {
                0 => getco_legendre(&list.data, nl_max),
                1 => getco_coulomb_amplitude(&list.data, list.head.c2, lidp, list.head.n2.max(0) as usize, constants, nl_max),
                2 => getco_law5_legendre(&list.data, list.head.n1.max(0) as usize, nl_max),
                _ => return Err(NjoyError::NotPorted("groupr conver: charged-particle elastic LTP > 2 (getco label 352 reads past the last tabulated cosine into stale scr words)")),
            };
            recs.push((list.head.c2, fl, elaw, ltp));
        }
        return Ok(ChargedLaw5 { angular: File4Angular::from_getco_records(recs, nl_max), constants, aprime });
    }
    Err(NjoyError::EndfParse(format!("getmf6: desired particle not found (MF=6/MT=2 of mat {mat}, ZAP = {zad})")))
}

/// `getco` label 300 with label 360 (`groupr.f90:10583-10700`), `lang = 1`,
/// `lcd = lct = 2`: the nuclear-plus-interference cross section by amplitude
/// expansion at the 64 Gauss cosines, projected on `P_0 .. P_{nl-1}`, the
/// coefficients above `P_0` rounded to `toler`, trimmed to the last non-zero.
/// `d` is the LIST's data (`d[k] = c(k+7)`), `e` its energy, `lidp` the TAB2's
/// (`getfle` stores it in `c(4)`), `nt = NL`.
pub(crate) fn getco_coulomb_amplitude(d: &[f64], e: f64, lidp: i32, nt: usize, c: ChargedElastic, nl_max: usize) -> Vec<f64> {
    let at = |k: usize| d.get(k).copied().unwrap_or(0.0);
    // `cmplx(a, b)` is default (single) kind; `cmplx(..)/2` halves in single.
    let single = |k: usize| (f64::from(at(k) as f32), f64::from(at(k + 1) as f32));
    let single_half = |k: usize| (f64::from((at(k) as f32) / 2.0f32), f64::from((at(k + 1) as f32) / 2.0f32));
    let ai = c.awrp * AMASSN;
    let zt = f64::from(c.izat / 1000);
    let zi = f64::from(c.izap / 1000);
    let ee = (EV / 10_000_000.0) * (CLIGHT / 10.0);
    let cc2 = ee.r_powi(4) * AMU / (2.0 * HBAR.r_powi(2) * EV);
    let eta = zt * zi * (cc2 * ai / e).sqrt();
    let l = nl_max - 1;
    let mut fl = vec![0.0f64; nl_max];
    for (x, w) in gauss64() {
        let y = if lidp != 1 {
            let np = 2 * nt;
            let p = legndr(x, np);
            let mut sigr = at(0) / 2.0;
            for ip in 1..=np {
                sigr += (2 * ip + 1) as f64 * p[ip] * at(ip) / 2.0;
            }
            let mut cs1 = single_half(np + 1);
            for it in 1..=nt {
                let k = (2 * it + 1) as f64 * p[it];
                let (re, im) = single(np + 1 + 2 * it);
                cs1 = (cs1.0 + k * re / 2.0, cs1.1 + k * im / 2.0);
            }
            let th1 = eta * ((1.0 - x) / 2.0).r_ln();
            let re = cs1.0 * th1.r_cos() - cs1.1 * th1.r_sin();
            let sigi = (-2.0 * eta / (1.0 - x)) * re;
            (1.0 - x) * (sigr + sigi)
        } else {
            let p = legndr(x, 2 * nt);
            let mut sigr = at(0) / 2.0;
            for it in 1..=nt {
                sigr += (4 * it + 1) as f64 * p[2 * it] * at(it) / 2.0;
            }
            let mut cs1 = single_half(nt + 1);
            let mut cs2 = cs1;
            let mut sgn = -1.0;
            for it in 1..=nt {
                let k = (2 * it + 1) as f64 * p[it];
                let (re, im) = single(nt + 1 + 2 * it);
                cs1 = (cs1.0 + k * re / 2.0, cs1.1 + k * im / 2.0);
                let ks = sgn * (2 * it + 1) as f64 * p[it];
                cs2 = (cs2.0 + ks * re / 2.0, cs2.1 + ks * im / 2.0);
                sgn = -sgn;
            }
            let th1 = eta * ((1.0 - x) / 2.0).r_ln();
            let th2 = eta * ((1.0 + x) / 2.0).r_ln();
            let a = (cs1.0 * (1.0 + x), cs1.1 * (1.0 + x));
            let b = (cs2.0 * (1.0 - x), cs2.1 * (1.0 - x));
            let re = (a.0 * th1.r_cos() - a.1 * th1.r_sin()) + (b.0 * th2.r_cos() - b.1 * th2.r_sin());
            let sigi = (-2.0 * eta / (1.0 - x * x)) * re;
            (1.0 - x * x) * (sigr + sigi)
        };
        // Label 370 (`lcd = lct = 2`: `xn = x`).
        let p = legndr(x, l);
        let yw = y * w;
        for il in 0..nl_max {
            fl[il] += yw * p[il];
        }
    }
    let mut nlz = 1;
    for il in 2..=nl_max {
        let j = (fl[il - 1] / TOLER).round();
        fl[il - 1] = j * TOLER;
        if j != 0.0 {
            nlz = il;
        }
    }
    fl.truncate(nlz);
    fl
}

#[cfg(test)]
mod tests {
    use super::*;

    /// With only `b_0` (no nuclear amplitude, no interference), label 360 gives
    /// `y = (1-x) b_0/2`, whose `P_0` and `P_1` moments are `b_0` and `-b_0/3`.
    #[test]
    fn amplitude_expansion_of_a_constant_residual() {
        let c = ChargedElastic { izap: 1001, izat: 2003, awrp: 0.99862, spi: 0.5 };
        // nt = 0: d = [b_0, Re a_0, Im a_0].
        let fl = getco_coulomb_amplitude(&[2.0, 0.0, 0.0], 1.0e6, 0, 0, c, 21);
        // `getco`'s 64 weights carry 9 figures: they sum to 2 - 4e-11.
        assert!((fl[0] - 2.0).abs() < 1e-10, "{fl:?}");
        assert!((fl[1] + 2.0 / 3.0).abs() < 1e-8, "{fl:?}");
        assert_eq!(fl.len(), 2);
    }
}
