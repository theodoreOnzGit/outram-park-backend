// SPDX-License-Identifier: AGPL-3.0-only
// Translated from NJOY2016.80 `src/errorr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
//   - `subroutine errorr`, l.713-792 (dictionary, `mfcov = 31`), l.965-976 (`ngchk`,
//     `gridd`, `lumpmt`), l.1048-1060 (`colaps`, `covcal`, `covout`);
//   - `subroutine rdgout`/`rdsig`, l.2755-2937, on `colaps`'s tape (`UnionGroupXs`).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! **ERRORR `mfcov = 31` and `mfcov = 33` from an input GENDF** (gh:#1369).
//!
//! With `ngout /= 0` the group constants come from GROUPR's tape: `ngchk`
//! condenses it and `colaps` puts every MF=3 section on the union grid, which
//! `rdsig` then reads (word 2 of each record: the cross section, or the nubar for
//! MT 452/455/456). Everything after that is the MF=33 chain of
//! [`super::run_mf33`] (`gridd`, `lumpmt`, `covcal`, `sigc`, `covout`), run on MF=31
//! when `mfcov = 31` (whose output is still headed MF=33, `errorr.f90:7202`).
//! `resprx` runs for `mfcov = 33` with MF=32 on file, as in [`super::run_mf33`].

use std::collections::BTreeMap;

use crate::endf::tape::Tape;
use crate::NjoyError;

use super::colaps::{colaps, FissionWindow, UnionGendf};
use super::covcal::covcal;
use super::covout::{covout, sigc, ErrorrResult};
use super::gendf::{ngchk, GendfTape};
use super::gridd::{gridd, lumpmt, scan_reactions_mf, uniong};
use super::groups::neutron_group_structure;
use super::grpav::{UnionGroupRecords, UnionGroupXs};
use super::mf33::{endf_version, Mf33Config};
use super::resprx::resprx;

/// `colaps`'s tape as the `ngout` that `rdgout` reads: per MF=3 section, the flux
/// (word 1) and `rdsig`'s word 2 of each union-group record, 0 where none.
pub fn union_group_xs(u: &UnionGendf, un: &[f64]) -> UnionGroupXs {
    let nunion = un.len() - 1;
    let mut records = BTreeMap::new();
    for (mt, recs) in &u.mf3 {
        let get = |k: usize| (0..nunion).map(|i| recs.get(i).copied().flatten().map_or(0.0, |r| r[k])).collect();
        records.insert(*mt, UnionGroupRecords { flux: get(0), sigma: get(1) });
    }
    UnionGroupXs { un: un.to_vec(), records, messages: Vec::new() }
}

/// ERRORR `mfcov = 31` or `33` on an ENDF tape and an input GENDF.
///
/// `cfg.tempin` selects the GENDF block (`ngchk`); the MF=1 temperature written is
/// the GENDF's (`colaps`'s `tempin = c1h`).
///
/// # Errors
/// [`NjoyError::NotPorted`] for ENDF/B-IV and what the MF=33 chain refuses
/// ([`super::run_mf33`]); [`NjoyError::InvalidInput`] for an `mfcov` other than 31
/// or 33; parse errors for malformed records.
pub fn run_cov_gendf(endf: &Tape, gendf: &GendfTape, cfg: &Mf33Config, mfcov: i32) -> Result<ErrorrResult, NjoyError> {
    if mfcov != 31 && mfcov != 33 {
        return Err(NjoyError::InvalidInput(format!("errorr run_cov_gendf: mfcov = {mfcov}")));
    }
    let matd = cfg.matd;
    let iverf = endf_version(endf, matd)?;
    if iverf <= 4 {
        return Err(NjoyError::NotPorted("errorr: ENDF/B-IV covariance layout (iverf=4)"));
    }
    let cg = ngchk(gendf, matd, cfg.tempin)?;
    let mut reactions = scan_reactions_mf(endf, matd, mfcov)?;
    let g = gridd(endf, matd, &mut reactions)?;
    lumpmt(endf, matd, &mut reactions)?;
    let raw = if cfg.ign.abs() == 1 {
        cfg.user_egn.clone().ok_or_else(|| NjoyError::EndfParse("errorr: ign=+-1 needs the card-12 group bounds".into()))?
    } else {
        neutron_group_structure(cfg.ign)?
    };
    let mut egn: Vec<f64> = raw.iter().map(|&e| crate::mixr::mix::sigfig(e, 6, 0)).collect();
    let un = uniong(&egn, &g.eni)?;
    if cfg.ign == -1 {
        egn = un.clone();
    }
    let u = colaps(&cg, &un, 0, &FissionWindow { eclo: 0.0, echi: 0.0, efmean: 0.0, igflag: 0 })?;
    if (egn[0] - 1.0e-5).abs() <= 1.0e-9 {
        egn[0] = 1.0e-5;
    }
    let groups = union_group_xs(&u, &un);
    let flx = groups.flux_vector();
    let fine = covcal(endf, matd, &reactions, &groups, &flx)?;
    let (za, awr) = endf
        .sections()
        .iter()
        .filter(|s| s.key.mat == matd && s.key.mf == mfcov)
        .find_map(|s| {
            let r = s.rows.first()?;
            (r[5] != 0.0).then_some((r[0], r[1]))
        })
        .unwrap_or((0.0, 0.0));
    let coarse = sigc(&egn, &groups, &flx, &reactions);
    let resonance = if mfcov == 33 && reactions.mf32_present {
        let ctx = super::resprx::sammy::SammyContext { mts: &reactions.mts, derived: &g.derived, coarse: &coarse };
        Some(resprx(endf, matd, &egn, &coarse.cflx, &cfg.weight, cfg.tempin, cfg.dap, &ctx)?)
    } else {
        None
    };
    let blocks = covout(&fine, &un, &egn, &reactions, &g.derived, &coarse, cfg.irelco, resonance.as_ref());
    Ok(ErrorrResult {
        matd,
        za,
        awr,
        iverf,
        tempin: u.tempin,
        irelco: cfg.irelco,
        egn,
        un,
        reactions,
        derived: g.derived,
        coarse,
        blocks,
        messages: groups.messages,
        resonance,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `rdsig` on `colaps`'s tape: word 2 per record (the nubar for MT=452), 0
    /// where no record was written; the flux is word 1.
    #[test]
    fn union_group_xs_reads_word_two() {
        let u = UnionGendf {
            tempin: 0.0,
            ela: vec![],
            mf3: vec![(1, vec![Some([2.0, 5.0, 0.0]), None]), (452, vec![Some([2.0, 2.4, 5.0]), Some([1.0, 2.5, 6.0])])],
            chi: None,
        };
        let g = union_group_xs(&u, &[1.0, 2.0, 3.0]);
        assert_eq!(g.flux_vector(), vec![2.0, 0.0]);
        assert_eq!(g.sigma_for(452), vec![2.4, 2.5]);
        assert_eq!(g.sigma_for(1), vec![5.0, 0.0]);
    }
}
