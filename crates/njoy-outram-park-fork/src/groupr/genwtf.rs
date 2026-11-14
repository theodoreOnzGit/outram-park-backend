// Translated from NJOY2016.80 `src/groupr.f90`: `genwtf`'s `iwt = 1` branch
// (:5011-5025), `jtemp` (:497) as `getwtf` reads it, and `getwtf`'s tabulated
// branch (:5186-5199, :5260-5272).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! **The module state `getwtf` reads** for `iwt = 1, 7, 10, 12` in the tape
//! driver (gh:#1370), beside [`super::njoy_panel::getwtf`]: card 8b's TAB1
//! (`wght`) and the run's current temperature (`temp(jtemp)`). Like `genwtf_iwt4`'s
//! `wght(1..6)`, they are Fortran module variables, held per thread here.

use petir::mathf::RealMath;

use crate::NjoyError;

const EMAX: f64 = 1.0e10;

thread_local! {
    /// `wght(1..iw)` as `genwtf` reads it for `iwt = 1` (card 8b, `:5011-5025`): a TAB1
    /// in ENDF flat layout. `None` until [`genwtf_iwt1`] runs.
    static WGHT1: std::cell::RefCell<Option<Vec<f64>>> = const { std::cell::RefCell::new(None) };
    /// `temp(jtemp)` \[K\], the temperature `getwtf` reads for `iwt = 7, 10, 12`
    /// (`:497`, `:5230`, `:5253-5254`, `:5281`). Set by [`getwtf_set_temperature`].
    static TEMP_JTEMP: std::cell::Cell<Option<f64>> = const { std::cell::Cell::new(None) };
}

/// `genwtf` for `iwt = 1` (`groupr.f90:5011-5025`): `nr = nint(tmp(5))`, `np =
/// nint(tmp(6))`, `iw = 6 + 2 nr + 2 np`, and `wght(1..iw) = tmp(1..iw)` for `getwtf`.
/// `tmp` is card 8b as read, with `NaN` for every word the card did not give.
///
/// # Errors
/// [`NjoyError::InvalidInput`] for `iw > 10000` (upstream's `exceeded storage`
/// error) or a TAB1 with no region or fewer than two points;
/// [`NjoyError::NotPorted`] when a word of `tmp(1..iw)` was not on the card: upstream
/// then reads an unset word of `allocate(tmp(ntmp))`, which is undefined.
pub fn genwtf_iwt1(tmp: &[f64]) -> Result<(), NjoyError> {
    const NTMP: i64 = 10000;
    let undefined = NjoyError::NotPorted("groupr genwtf iwt = 1: card 8b shorter than its TAB1 (upstream reads unset words of tmp)");
    if tmp.len() < 6 || tmp[4].is_nan() || tmp[5].is_nan() {
        return Err(undefined);
    }
    let nr = tmp[4].round() as i64;
    let np = tmp[5].round() as i64;
    let iw = 6 + 2 * nr + 2 * np;
    if iw > NTMP {
        return Err(NjoyError::InvalidInput("genwtf: exceeded storage reading user weight function".into()));
    }
    if nr < 1 || np < 2 || iw > tmp.len() as i64 {
        return Err(NjoyError::InvalidInput(format!("genwtf: card 8b TAB1 with nr = {nr}, np = {np}")));
    }
    let w = &tmp[..iw as usize];
    if w.iter().any(|x| x.is_nan()) {
        return Err(undefined);
    }
    WGHT1.with(|c| *c.borrow_mut() = Some(w.to_vec()));
    Ok(())
}

/// Card 8b's TAB1, as [`genwtf_iwt1`] stored it.
///
/// # Errors
/// [`NjoyError::InvalidInput`] before [`genwtf_iwt1`] has run.
pub(crate) fn wght1() -> Result<Vec<f64>, NjoyError> {
    WGHT1
        .with(|c| c.borrow().clone())
        .ok_or_else(|| NjoyError::InvalidInput("groupr getwtf: iwt = 1 without genwtf's card 8b".into()))
}

/// `jtemp = itemp` (`groupr.f90:497`): the temperature \[K\] `getwtf` reads as
/// `temp(jtemp)` for `iwt = 7, 10, 12`.
pub fn getwtf_set_temperature(temp_k: f64) {
    TEMP_JTEMP.with(|c| c.set(Some(temp_k)));
}

/// `temp(jtemp)`.
///
/// # Errors
/// [`NjoyError::InvalidInput`] before [`getwtf_set_temperature`] has run.
pub(crate) fn temp_jtemp() -> Result<f64, NjoyError> {
    TEMP_JTEMP
        .with(std::cell::Cell::get)
        .ok_or_else(|| NjoyError::InvalidInput("groupr getwtf: a temperature-dependent iwt before temp(jtemp) was set".into()))
}

/// `getwtf`'s `save`d `ip, ir, ipl, step` (`:5173`), as the tabulated branch updates them.
#[derive(Debug, Clone)]
pub(crate) struct TabState {
    /// `ip`.
    pub ip: usize,
    /// `ir`.
    pub ir: usize,
    /// `ipl`.
    pub ipl: usize,
    /// `step`.
    pub step: f64,
}

/// `terpa` over `wght` with the saved state and `getwtf`'s 1 % stop
/// (`:5190-5199`). `zero_to_emax` is `iwt = 10`'s `if (wtf.eq.zero) enext=emax`
/// (`:5262-5263`); `iwt = 1, 5, 8, 9` leave `enext` as `terpa` gave it.
/// Returns `(wtf, enext, idis)` before `getwtf`'s final `sigfig(enext, 6, 1)`.
///
/// # Errors
/// What `terpa` returns.
pub(crate) fn getwtf_tabulated(st: &mut TabState, tab: &[f64], e: f64, zero_to_emax: bool) -> Result<(f64, f64, i32), NjoyError> {
    let (mut ip, mut ir) = (st.ip, st.ir);
    let (wtf, mut enext, mut idis) = crate::errorr::weight::terpa(tab, e, &mut ip, &mut ir)?;
    st.ip = ip;
    st.ir = ir;
    if wtf == 0.0 {
        if zero_to_emax {
            enext = EMAX;
        }
    } else {
        if st.ip != st.ipl {
            st.step = 1.0001 * (enext / e).r_powf(0.1);
            st.ipl = st.ip;
        }
        let mut enxt = st.step * e;
        if enxt > 1.01 * e {
            enxt = 1.01 * e;
        }
        if enext > enxt {
            idis = 0;
            enext = enxt;
        }
    }
    Ok((wtf, enext, idis))
}

/// `getwtf`'s `iwt = 1` (card 8b's TAB1, `:5186-5199`) and `iwt = 10` branches
/// (`:5252-5274`: a thermal Maxwellian at `temp(jtemp)` below `eb = 0.15 T/300`,
/// then the CLAW table `w9`). Returns `(wtf, enext, idis)` before `sigfig(enext, 6, 1)`.
///
/// # Errors
/// [`NjoyError::InvalidInput`] for another `iwt`, or before [`genwtf_iwt1`]
/// (`iwt = 1`) or [`getwtf_set_temperature`] (`iwt = 10`) has run.
pub(crate) fn getwtf_iwt1_or_10(st: &mut TabState, iwtt: i32, e: f64) -> Result<(f64, f64, i32), NjoyError> {
    const WT10A: f64 = 0.15;
    const WT10B: f64 = 300.0;
    const WT10C: f64 = 1.15e6;
    match iwtt {
        1 => getwtf_tabulated(st, &wght1()?, e, false),
        10 => {
            let t = temp_jtemp()?;
            let ea = crate::common::phys::BK_EV_PER_K * t;
            let eb = WT10A * t / WT10B;
            if e < eb {
                let wtf = WT10C * (e / eb.r_powi(2)) * (-(e - eb) / ea).r_exp();
                let mut enext = 1.01 * e;
                if enext > eb {
                    enext = eb;
                }
                Ok((wtf, enext, 0))
            } else {
                getwtf_tabulated(st, &crate::groupr::weights::claw_tab1(), e, true)
            }
        }
        _ => Err(NjoyError::InvalidInput(format!("groupr getwtf_iwt1_or_10: iwt = {iwtt}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A card 8b that stops before its TAB1 ends is refused by name (upstream
    /// would read unset words); a complete one is stored as given.
    #[test]
    fn genwtf_iwt1_stores_the_tab1_and_refuses_a_short_card() {
        let mut short = vec![f64::NAN; 10000];
        short[..8].copy_from_slice(&[0.0, 0.0, 0.0, 0.0, 1.0, 2.0, 2.0, 2.0]);
        assert!(matches!(genwtf_iwt1(&short), Err(NjoyError::NotPorted(_))));
        let mut full = short.clone();
        full[8..12].copy_from_slice(&[1.0e-5, 1.0, 2.0e7, 1.0]);
        genwtf_iwt1(&full).unwrap();
        assert_eq!(wght1().unwrap(), full[..12].to_vec());
        assert!(matches!(genwtf_iwt1(&[0.0; 5]), Err(NjoyError::NotPorted(_))));
    }

    /// `iwt = 10` keeps `enext = emax` where `terpa` returns zero (past the table);
    /// the other tabulated weights keep `terpa`'s `enext`.
    #[test]
    fn tabulated_zero_weight_stop() {
        let tab = [0.0, 0.0, 0.0, 0.0, 1.0, 2.0, 2.0, 2.0, 1.0, 1.0, 2.0, 1.0];
        let fresh = || TabState { ip: 2, ir: 1, ipl: 0, step: 1.10 };
        let (w, en, _) = getwtf_tabulated(&mut fresh(), &tab, 5.0, true).unwrap();
        assert_eq!((w, en), (0.0, EMAX));
        let (w2, en2, _) = getwtf_tabulated(&mut fresh(), &tab, 5.0, false).unwrap();
        assert_eq!(w2, 0.0);
        assert_ne!(en2, EMAX);
        let (w3, en3, _) = getwtf_tabulated(&mut fresh(), &tab, 1.5, false).unwrap();
        assert_eq!(w3, 1.0);
        assert!(en3 <= 1.01 * 1.5);
    }
}
