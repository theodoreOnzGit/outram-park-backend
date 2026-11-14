// Translated from NJOY2016.80 (commit ab27c6414a0f) `src/purr.f90`: the
// energy loop of subroutine `purr` (lines 283-313) and its MF=2/MT=152 and
// MT=153 writers (lines 415-570) for several temperatures and sigma-zeros.
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! PURR's PENDF words for **every** temperature and sigma-zero of its card
//! (gh:#1369, upstream test 10).
//!
//! [`UrrProbabilityTables::from_endf_with_heating_nunx`] builds one temperature
//! at `sigma_0 = 1e10`, which is what ACER reads and what PURR writes for a
//! one-temperature, one-`sigma_0` deck. Upstream's `unrest` draws one random
//! sequence per energy for all temperatures and sigma-zeros together
//! (`purr.f90:300-302`), and MT=152 carries every `sigu(ix,is,it)`. This
//! module runs [`probability_table`] with the full lists and lays out what
//! `purr.f90` writes, beside the one-temperature builder (which is left as it
//! is). It reuses that builder's grid, background, competition and LSSF
//! helpers.
//!
//! **Kept from upstream:** MT=153's LSSF = 1 division uses
//! `sigu(i-1,1,1)`, the **first** temperature's and first sigma-zero's
//! Bondarenko value, for every temperature (`purr.f90:516-523`).
//!
//! **Heating** (`rdheat`, `purr.f90:1237-1289`): the caller passes each
//! temperature's MT=301/302/318/402 TAB1s from PURR's input PENDF (none:
//! `ihave = 0`, a zero column). `ihave` is what the **last** temperature's scan
//! leaves (`rdheat` resets it per temperature): 2 with any partial, 1 with
//! MT=301 alone. The column is `purr.f90:525-560` (upstream test 19).

use crate::endf::tape::Tape;
use crate::unresr::mf2::{self, UnresolvedCase};
use crate::NjoyError;

use super::super::pendf::list_rows;
use super::super::wfun::DopplerTable;
use super::super::{infinite_dilution_reference, probability_table, Rng};
use super::{competition_flags, lssf_reduced_background, nunx_indices, urr_energy_grid, PURR_SEED};

/// PURR's PENDF words for one material at every temperature.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PurrPendfMulti {
    /// `LSSF`.
    pub(crate) lssf: i32,
    /// `intunr`.
    pub(crate) intunr: i32,
    /// `nbin`.
    pub(crate) nbin: usize,
    /// `iinel`, `iabso`.
    pub(crate) competition: (i32, i32),
    /// The kept unresolved energies `eunr(ie)`, as read.
    pub(crate) energies: Vec<f64>,
    /// Per temperature, per energy: `sigfig(sigu(ix,is,it),7,0)`, `ix` outer,
    /// `is` inner (`purr.f90:451-456`).
    pub(crate) bondarenko: Vec<Vec<Vec<f64>>>,
    /// Per temperature, per energy: the `6 nbin` MT=153 words after the energy.
    pub(crate) bands: Vec<Vec<Vec<f64>>>,
}

/// PURR on `tape`'s unresolved range at `temps` and `sigz`, `nunx` energies
/// (0: all). `Ok(None)` without an LRU = 2 range, as the one-temperature
/// builder.
///
/// # Errors
/// Those of [`probability_table`] and the MF=2/MF=3 readers.
pub(crate) fn purr_pendf_multi(
    tape: &Tape,
    mat: i32,
    temps: &[f64],
    sigz: &[f64],
    nbin: usize,
    nladr: usize,
    nsamp: usize,
    nunx: usize,
    heat: &[[Option<crate::endf::records::Tab1>; 4]],
) -> Result<Option<PurrPendfMulti>, NjoyError> {
    use crate::acer::build::sigfig;
    // `ihave` after `rdheat`'s last temperature.
    let ihave = heat.last().map_or(0, |h| if h[1..].iter().any(Option::is_some) { 2 } else { i32::from(h[0].is_some()) });
    // `heat(ix,ie,it)`: `gety1` at `|eunr(ie)|`, 0 where the section is absent.
    let heat_at = |it: usize, ix: usize, e: f64| -> f64 {
        heat.get(it).and_then(|h| h[ix].as_ref()).map_or(0.0, |t| {
            let mut g = crate::endf::gety1::Gety1::new(t);
            let _ = g.get(0.0);
            g.get(e.abs()).y
        })
    };
    let Some(sec) = tape.section(mat, 2, 151) else {
        return Ok(None);
    };
    if sec.rows.len() < 2 {
        return Ok(None);
    }
    let ranges = mf2::parse_lru2_ranges(&sec.rows[1..])?;
    let Some(range) = ranges.into_iter().next() else {
        return Ok(None);
    };
    let energy = urr_energy_grid(&range);
    if energy.is_empty() {
        return Ok(None);
    }
    let mt_data: Vec<(i32, Vec<(u32, u32)>, Vec<(f64, f64)>)> = [1i32, 2, 18, 102]
        .iter()
        .filter_map(|&mt| {
            let s = tape.section(mat, 3, mt)?;
            let mut c = crate::endf::records::SectionCursor::new(&s.rows);
            let _ = c.read_cont().ok()?;
            let t = c.read_tab1().ok()?;
            Some((mt, t.interp, t.pairs))
        })
        .collect();
    let bkg_all = mf2::background_cross_sections(&energy, &mt_data)?;
    let ranges = [range.clone()];
    let mut rng = Rng::new(PURR_SEED);
    let _warmup = rng.next(); // purr.f90:167
    let dop = DopplerTable::new();
    let (iinel, iabso) = competition_flags(tape, mat, &energy, &bkg_all);
    let competes = iinel >= 0 || iabso >= 0;
    let ecomp = bkg_all.iter().position(|b| b[0] - b[1] - b[2] - b[3] > 1.0e-5).map(|i| energy[i]);
    let selected = nunx_indices(energy.len(), nunx);
    let ntemp = temps.len();
    let mut bondarenko = vec![Vec::with_capacity(selected.len()); ntemp];
    let mut bands = vec![Vec::with_capacity(selected.len()); ntemp];
    for &k in &selected {
        let e = energy[k];
        let inf = infinite_dilution_reference(&ranges, e)?;
        let bkg = if range.lssf > 0 { lssf_reduced_background(bkg_all[k], e, competes, ecomp) } else { bkg_all[k] };
        let res = probability_table(&inf.sequences, &inf, bkg, sigz, temps, nbin, nladr, nsamp, &mut rng, &dop)?;
        // `sigu(i-1,1,1)`: the first temperature's, first sigma-zero's.
        let sigu11 = res.tables[0].bondarenko[0];
        for (it, t) in res.tables.iter().enumerate() {
            let mut b = Vec::with_capacity(5 * sigz.len());
            for ix in 0..5 {
                for is in 0..sigz.len() {
                    b.push(sigfig(t.bondarenko[is][ix], 7));
                }
            }
            bondarenko[it].push(b);
            let mut w = Vec::with_capacity(6 * nbin);
            w.extend(t.bin_probability.iter().map(|&p| sigfig(p, 7)));
            for i in 0..4 {
                for xs in &t.bin_xs {
                    let x = sigfig(xs[i], 7);
                    let x = if range.lssf == 1 {
                        if sigu11[i] != 0.0 {
                            x / sigu11[i]
                        } else {
                            1.0
                        }
                    } else {
                        x
                    };
                    w.push(sigfig(x, 7));
                }
            }
            // The heating column (`purr.f90:510-560`): 0 for `ihave = 0`.
            for j in 0..nbin {
                let col = |c: usize| w[c * nbin + j];
                let h1 = heat_at(it, 0, e);
                let a = match ihave {
                    1 if range.lssf == 1 => 1.0,
                    1 => h1 / sigu11[0],
                    2 => {
                        let mut a = 0.0 + (h1 - heat_at(it, 1, e) - heat_at(it, 2, e) - heat_at(it, 3, e));
                        for q in 1..4 {
                            let mut h = heat_at(it, q, e);
                            if range.lssf == 1 {
                                h *= col(q + 1);
                            } else if sigu11[q] != 0.0 {
                                h = h * col(q + 1) / sigu11[q];
                            }
                            a += h;
                        }
                        if col(1) != 0.0 {
                            a /= col(1);
                        }
                        if range.lssf == 1 {
                            a /= h1;
                        }
                        a
                    }
                    _ => 0.0,
                };
                w.push(a);
            }
            bands[it].push(w);
        }
    }
    let intunr = match &range.case_ {
        UnresolvedCase::CaseC { l_states, .. } => l_states.iter().flat_map(|l| l.j_states.iter()).last().map_or(5, |j| j.int_),
        _ => 5,
    };
    Ok(Some(PurrPendfMulti {
        lssf: range.lssf,
        intunr,
        nbin,
        competition: (iinel, iabso),
        energies: selected.iter().map(|&k| energy[k]).collect(),
        bondarenko,
        bands,
    }))
}

impl PurrPendfMulti {
    /// MF=2/MT=152 at temperature index `it` (`purr.f90:415-460`): CONT
    /// `[ZA, AWR, LSSF, 0, 0, intunr]`, LIST `[temz, 0, 5, nsigz, npl, nunx]`
    /// with the sigma-zeros, then per energy `eunr` and the `5 nsigz` words.
    pub(crate) fn mt152_rows(&self, it: usize, za: f64, awr: f64, temz: f64, sigz: &[f64]) -> Vec<[f64; 6]> {
        let mut w: Vec<f64> = sigz.to_vec();
        for (k, &e) in self.energies.iter().enumerate() {
            w.push(e);
            w.extend(self.bondarenko[it][k].iter().copied());
        }
        let nunx = self.energies.len();
        list_rows(
            [za, awr, f64::from(self.lssf), 0.0, 0.0, f64::from(self.intunr)],
            [temz, 0.0, 5.0, sigz.len() as f64, w.len() as f64, nunx as f64],
            &w,
        )
    }

    /// MF=2/MT=153 at temperature index `it` (`purr.f90:472-570`).
    pub(crate) fn mt153_rows(&self, it: usize, za: f64, awr: f64, temz: f64) -> Vec<[f64; 6]> {
        let mut w = Vec::new();
        for (k, &e) in self.energies.iter().enumerate() {
            w.push(crate::acer::build::sigfig(e, 7));
            w.extend(self.bands[it][k].iter().copied());
        }
        let nunx = self.energies.len();
        list_rows(
            [za, awr, f64::from(self.competition.0), f64::from(self.competition.1), f64::from(self.intunr), self.nbin as f64],
            [temz, 0.0, f64::from(self.lssf), 0.0, w.len() as f64, nunx as f64],
            &w,
        )
    }
}
