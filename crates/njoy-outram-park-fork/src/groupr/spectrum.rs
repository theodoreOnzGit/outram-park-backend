// Translated from NJOY2016.80 `src/groupr.f90`: the `mfd = 5` spectrum branch of
// `subroutine groupr` (`:745-779`), `getff` labels 202/205/260 for `mfd = 5`
// (`:7140-7230`), `displa` label 400 (`:6347-6380`), and `getyld` on MF=1
// (`:6521-6644`, `MT = 452, 455, 456`).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! **GROUPR's `mfd = 5` spectra and the MF=1 neutron yields** (gh:#1369).
//!
//! `mfd = 5` is not a group loop. `getsig` and `getflx` are skipped; `getff`'s
//! initialisation calls `getsed` on MF=5, whose `enext` is the lowest first
//! energy of the subsections' `p(E)`; one more `getff` at that energy returns
//! the spectrum over the neutron groups (label 260: `iglo = 1`, `ighi` the last
//! nonzero group). The record is written once, as group `ngn`, after `displa`
//! label 400 rounds each word with `sigfig(., 7, 0)`.
//!
//! For `mtd = 455` the `ndelg` precursor families stay separate (`nl = ndelg`)
//! and the first word of each is `dntc(il)`. `dntc` is filled **only** by
//! `getyld` on MF=1/MT=455 (`:6576-6583`), which `mfd = 5` does not call: it is
//! what an earlier `3 455` of the same run left, otherwise zero (as NJOY2016.80
//! writes for upstream test 29).
//!
//! `getyld` on MF=1 ([`NuYield`]) is the LNU=1 polynomial (`:6630-6641`) or the
//! LNU=2 TAB1 read by `terpa`. `getff` label 100 feeds it to `panel` as
//! `ff = (yld, 1)` (`ng = 2`), and `displa` writes the ratio form (`irat = 1`)
//! that `MT = 251-253` use.

use crate::endf::records::SectionCursor;
use crate::endf::tape::Tape;
use crate::groupr::continuum::PhotonSed;
use crate::mixr::mix::sigfig;
use crate::NjoyError;

const EMAX: f64 = 1.0e10;
const SMALL: f64 = 1.0e-10;

/// The one LIST record of an `mfd = 5` section: `nl` Legendre slots (`ndelg`
/// families for MT=455, else 1) and `ng2` words per slot, `ig2lo = 1`.
#[derive(Debug, Clone, PartialEq)]
pub struct SpectrumRecord {
    /// `nl` (the head's L1).
    pub nl: usize,
    /// `ng2` (the LIST's L1).
    pub ng2: usize,
    /// `ans(il, 1, it)`, `il` fastest: the `nl * ng2` words.
    pub data: Vec<f64>,
}

/// `mfd = 5`, reaction `mtd`, on the neutron groups `egn`.
///
/// `ndelg` is `conver`'s count of delayed families (N1 of the MF=1/MT=455
/// LIST), `dntc` the decay constants an earlier `3 455` left (zeros otherwise),
/// `ismooth` card 2's switch (the delayed-bin extension of `getsed`).
///
/// # Errors
/// [`NjoyError::SectionNotFound`] without MF=5/`mtd`; [`NjoyError::NotPorted`]
/// for analytic (`LF > 1`) subsections; [`NjoyError::InvalidInput`] when MT=455
/// has a subsection count other than `ndelg`, or the spectrum is zero in every
/// group (upstream's `ighi` is then unassigned).
pub fn mf5_spectrum(nend: &Tape, mat: i32, mtd: i32, egn: &[f64], ismooth: bool, ndelg: usize, dntc: &[f64; 8]) -> Result<SpectrumRecord, NjoyError> {
    let delayed = mtd == 455;
    // `getsed(e = 0)`; `econst` only matters for `mfd /= 5`, so its search is made inert.
    let (sed, en, _) = PhotonSed::from_tape_mf(nend, mat, 5, mtd, egn, 0.0, ismooth)?;
    let nk = if delayed { ndelg } else { 1 };
    if delayed && sed.nk() != ndelg {
        return Err(NjoyError::InvalidInput(format!("groupr mfd=5 mt=455: {} subsections, ndelg = {ndelg}", sed.nk())));
    }
    if nk > 8 {
        return Err(NjoyError::InvalidInput("getyld: illegal lnd.".into()));
    }
    // `getff` init: `enext = emax`, lowered to `getsed`'s `en` (`:7163-7165`).
    let e = if en < EMAX * (1.0 - SMALL) { en } else { EMAX };
    let (rows, _, _) = sed.get_nk(e, nk > 1)?;
    // label 260 with `mfd = 5`: `iglo = 1`; `ighi` is the last group with a nonzero word.
    let ngn = egn.len() - 1;
    let ighi = (1..=ngn)
        .filter(|&ig| rows.iter().any(|r| r[ig - 1] != 0.0))
        .max()
        .ok_or_else(|| NjoyError::InvalidInput(format!("groupr mfd=5 mt={mtd}: spectrum zero in every group")))?;
    // `:757-779`: `ans(il,1,j)`, `j = i` (+1 for MT=455, after `dntc` in slot 1).
    let nl = nk;
    let ng2 = if delayed { ighi + 1 } else { ighi };
    let mut data = Vec::with_capacity(nl * ng2);
    for it in 1..=ng2 {
        for il in 0..nl {
            let v = if delayed {
                if it == 1 { dntc[il] } else { rows[il][it - 2] }
            } else {
                rows[0][it - 1]
            };
            // `displa` label 400.
            data.push(sigfig(v, 7, 0));
        }
    }
    Ok(SpectrumRecord { nl, ng2, data })
}

/// `conver`'s `ndelg` (`:11510-11513`): N1 of the MF=1/MT=455 decay-constant LIST,
/// 0 without the section.
///
/// # Errors
/// A malformed section.
pub fn ndelg(nend: &Tape, mat: i32) -> Result<usize, NjoyError> {
    let Some(sec) = nend.section(mat, 1, 455) else { return Ok(0) };
    let mut cur = SectionCursor::new(&sec.rows);
    let _ = cur.read_cont()?;
    Ok(cur.read_list()?.head.n1.max(0) as usize)
}

/// `getyld` on MF=1 (`MT = 452, 455, 456`): the LNU=1 polynomial or the LNU=2
/// TAB1 (packed, with `terpa`'s saved `ip`, `ir`).
#[derive(Debug, Clone)]
pub enum NuYield {
    /// LNU=1: the LIST's coefficients `c(1..nc)`.
    Polynomial(Vec<f64>),
    /// LNU=2: the packed TAB1, `ip`, `ir`.
    Tabulated(Vec<f64>, usize, usize),
}

impl NuYield {
    /// `getyld(e = 0)` for MF=1/`mt`: the yield and, for MT=455, `dntc`
    /// (`:6576-6583`; zeros past `lnd`).
    ///
    /// # Errors
    /// [`NjoyError::SectionNotFound`]; [`NjoyError::NotPorted`] for MT=455 with
    /// energy-dependent decay constants (`LDG = 1`) or a polynomial (`LNU = 1`),
    /// whose records upstream would misread; `'illegal lnd.'` past 8 families.
    pub fn from_tape(nend: &Tape, mat: i32, mt: i32) -> Result<(Self, Option<[f64; 8]>), NjoyError> {
        let sec = nend.section(mat, 1, mt).ok_or(NjoyError::SectionNotFound { mat, mf: 1, mt })?;
        let mut cur = SectionCursor::new(&sec.rows);
        let head = cur.read_cont()?;
        let lnu = head.l2;
        let mut dntc = None;
        if mt == 455 {
            if head.l1 != 0 || lnu != 2 {
                return Err(NjoyError::NotPorted("groupr getyld: MF=1/MT=455 with LDG = 1 or LNU = 1"));
            }
            let l = cur.read_list()?;
            let lnd = l.head.n1.max(0) as usize;
            if lnd > 8 {
                return Err(NjoyError::InvalidInput("getyld: illegal lnd.".into()));
            }
            let mut d = [0.0f64; 8];
            d[..lnd].copy_from_slice(&l.data[..lnd]);
            dntc = Some(d);
        } else if lnu == 1 {
            return Ok((NuYield::Polynomial(cur.read_list()?.data), None));
        }
        let t = cur.read_tab1()?;
        Ok((NuYield::Tabulated(crate::groupr::continuum::pack(&t), 2, 1), dntc))
    }

    /// `enext` of `getyld(e = 0)` (`:6602-6606`, `:6573`): the TAB1's first energy
    /// (`tmp(7+2*nr)`), or `emax` for the LNU=1 polynomial.
    pub fn first_energy(&self) -> f64 {
        match self {
            NuYield::Tabulated(t, _, _) => {
                let nr = t[4].round() as usize;
                t[6 + 2 * nr]
            }
            NuYield::Polynomial(_) => EMAX,
        }
    }

    /// `getyld(e > 0)`: `(yld, enext, idis)`.
    ///
    /// # Errors
    /// From `terpa`.
    pub fn at(&mut self, e: f64) -> Result<(f64, f64, i32), NjoyError> {
        match self {
            NuYield::Tabulated(t, ip, ir) => crate::errorr::weight::terpa(t, e, ip, ir),
            NuYield::Polynomial(c) => {
                // `:6630-6641`, term by term in the order upstream sums them.
                let mut yld = c.first().copied().unwrap_or(0.0);
                let mut term = 1.0;
                for &ci in c.iter().skip(1) {
                    term *= e;
                    yld += ci * term;
                }
                Ok((yld, EMAX, 0))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polynomial_nu_is_summed_term_by_term() {
        let mut y = NuYield::Polynomial(vec![2.5, 1.0e-7, 2.0e-15]);
        let (v, en, idis) = y.at(1.0e6).unwrap();
        assert_eq!((en, idis), (EMAX, 0));
        assert_eq!(v, 2.5 + 1.0e-7 * 1.0e6 + 2.0e-15 * (1.0e6 * 1.0e6));
    }
}
