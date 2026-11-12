// Translated from NJOY2016.80 (commit ab27c6414a0f) `src/purr.f90`, the
// MF=2/MT=152 and MT=153 writers of subroutine `purr` (lines 415-570). NJOY2016
// is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible; this
// derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! **PURR's PENDF sections** (gh:#1498): the MF=2/MT=152 Bondarenko table and
//! the MF=2/MT=153 probability tables that `purr` writes onto its output tape
//! after the evaluation's MF=2/MT=151.
//!
//! ~~PURR's tape writer was "not ported": the deck driver passed the input PENDF
//! through, so MODER's text copy of it (upstream test 36's `referenceTape34`)
//! differed on 24 323 of 24 331 lines.~~ **CORRECTED 2026-11-12**: the cause was
//! not MODER's text writer but these two sections. The input tape carried
//! RECONR's MT=152 (a different energy grid) and no MT=153, and every sequence
//! number after the dictionary shifted.
//!
//! The words come from [`UrrProbabilityTables`]'s `pendf` record, which keeps
//! what `purr.f90`'s scratch file holds per energy (`ez`, `tabl`, `sigu`); the
//! functions here only lay them out, as `purr.f90` fills `a(iscr..)` and hands
//! it to `contio`/`listio`.

use super::tables::UrrProbabilityTables;
use crate::acer::build::sigfig;

/// Rows of a LIST section body: the CONT head, the LIST head, then `words`
/// six to a row, the last row padded with zeros (what `listio` writes).
fn list_rows(cont: [f64; 6], list_head: [f64; 6], words: &[f64]) -> Vec<[f64; 6]> {
    let mut rows = vec![cont, list_head];
    for chunk in words.chunks(6) {
        let mut r = [0.0f64; 6];
        r[..chunk.len()].copy_from_slice(chunk);
        rows.push(r);
    }
    rows
}

/// The `NC` PURR puts in the dictionary for a LIST of `npl` words:
/// `2 + (npl - 1) / 6` with integer division (`purr.f90:365-368`).
///
/// This is one **less** than the lines the section occupies whenever
/// `npl mod 6 = 1` (MT=152 with one `sigma_0` is always so: `1 + 6 nunx`
/// words). Upstream writes the dictionary entry from this formula, not from the
/// section it writes, so the reproduction keeps it. Upstream test 36:
/// MT=152 `NC = 28` for a 29-line section; MT=153 `NC = 526` for 527 lines.
pub(crate) fn directory_nc(npl: usize) -> usize {
    2 + (npl.max(1) - 1) / 6
}

/// `npl` of MT=152: `nsigz + nunx * (1 + 5 nsigz)` (`purr.f90:365`).
pub(crate) fn mt152_words(nsigz: usize, nunx: usize) -> usize {
    nsigz + nunx * (1 + 5 * nsigz)
}

/// `npl` of MT=153: `(1 + 6 nbin) nunx` (`purr.f90:367`).
pub(crate) fn mt153_words(nbin: usize, nunx: usize) -> usize {
    (1 + 6 * nbin) * nunx
}

/// MF=2/MT=152 of PURR's output tape, at one `sigma_0` (infinite dilution,
/// `sigz`) and temperature `temz` (`purr.f90:415-437`, `:442-460`).
///
/// CONT `[ZA, AWR, LSSF, 0, 0, intunr]`; LIST `[temz, 0, 5, nsigz, npl,
/// nunx]` holding `sigz` then, for each energy, `eunr` (**not** `sigfig`'d)
/// and the five `sigfig(sigu(ix,is,it), 7, 0)`, `ix` outer, `is` inner.
///
/// `None` for a table that carries no PURR record (one read from an ACE file).
pub(crate) fn mt152_rows(za: f64, awr: f64, temz: f64, sigz: f64, t: &UrrProbabilityTables) -> Option<Vec<[f64; 6]>> {
    let p = t.pendf.as_ref()?;
    let e = t.energies();
    let nunx = e.len();
    let mut w = vec![sigz];
    for (k, &ek) in e.iter().enumerate() {
        w.push(ek);
        // sigfig'd already when stored (`sigfig(sigu(ix,is,it),7,0)`, `:455`).
        w.extend(p.bondarenko[k].iter().copied());
    }
    debug_assert_eq!(w.len(), mt152_words(1, nunx));
    Some(list_rows(
        [za, awr, f64::from(t.lssf), 0.0, 0.0, f64::from(p.intunr)],
        [temz, 0.0, 5.0, 1.0, w.len() as f64, nunx as f64],
        &w,
    ))
}

/// MF=2/MT=153 of PURR's output tape (`purr.f90:472-570`).
///
/// CONT `[ZA, AWR, iinel, iabso, intunr, nbin]`; LIST `[temz, 0, LSSF, 0,
/// (1 + 6 nbin) nunx, nunx]` holding, per energy, `sigfig(eunr, 7, 0)` and the
/// `6 nbin` band words (probability, total, elastic, fission, capture,
/// heating; bin by bin in each column).
pub(crate) fn mt153_rows(za: f64, awr: f64, temz: f64, t: &UrrProbabilityTables) -> Option<Vec<[f64; 6]>> {
    let p = t.pendf.as_ref()?;
    let e = t.energies();
    let nunx = e.len();
    let mut w = Vec::with_capacity(mt153_words(p.nbin, nunx));
    for (k, &ek) in e.iter().enumerate() {
        w.push(sigfig(ek, 7));
        w.extend(p.bands[k].iter().copied());
    }
    debug_assert_eq!(w.len(), mt153_words(p.nbin, nunx));
    Some(list_rows(
        [za, awr, f64::from(t.inelastic_competition), f64::from(t.absorption_competition), f64::from(p.intunr), p.nbin as f64],
        [temz, 0.0, f64::from(t.lssf), 0.0, w.len() as f64, nunx as f64],
        &w,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dictionary_nc_is_upstreams_formula_not_the_line_count() {
        // Upstream test 36: 26 energies, nbin 20, one sigma_0.
        assert_eq!(mt152_words(1, 26), 157);
        assert_eq!(directory_nc(157), 28);
        assert_eq!(mt153_words(20, 26), 3146);
        assert_eq!(directory_nc(3146), 526);
        // The sections occupy one line more (CONT + LIST head + ceil(npl/6)).
        assert_eq!(2 + 157usize.div_ceil(6), 29);
        assert_eq!(2 + 3146usize.div_ceil(6), 527);
        // A multiple of six is exact.
        assert_eq!(directory_nc(12), 3);
    }

    #[test]
    fn list_rows_pad_the_last_row_with_zeros() {
        let rows = list_rows([1.0; 6], [2.0; 6], &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0]);
        assert_eq!(rows.len(), 4);
        assert_eq!(rows[2], [1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        assert_eq!(rows[3], [7.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
    }
}
