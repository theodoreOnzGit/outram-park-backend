// SPDX-License-Identifier: AGPL-3.0-only
// Ported from NJOY2016 `src/broadr.f90` (git commit ab27c64):
//   - `subroutine broadr`, l.241-249 (`lrp` from MF=1/MT=451) and l.292-330
//     (the LRF=7 particle-pair MT list `nppmt`, sorted);
//   - `subroutine broadr`, l.513-520 (each listed MT forced into the
//     broadened set, in list order).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! **The LRF=7 particle-pair list `nppmt`** (gh:#618).
//!
//! For an R-matrix-limited evaluation BROADR reads the MT of every particle
//! pair from the ENDF tape's MF=2/MT=151 (`broadr.f90:303-330`) and broadens
//! each of those reactions **whatever its threshold** (`:513-520`), before the
//! `enext <= emin` and `lrp = 1, enext < eresh` tests. The list comes from the
//! **ENDF** tape (`nendf`), not the PENDF, so it is read here from a [`Tape`];
//! a [`crate::reconr::ReconrResult`] does not carry it.
//!
//! Upstream's matching is order-dependent and that is kept: the index into
//! the sorted list advances only when a section's MT equals the listed MT at
//! that index, so a listed MT that never reaches the test (MT=18, one of the
//! MTs BROADR skips outright, or one absent from MF=3) stalls the rest of the
//! list. [`forced_pair`] is that test.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use crate::endf::tape::Tape;

/// `nppmt`: the MTs of MF=2/MT=151's LRF=7 particle pairs, ascending
/// (`broadr.f90:292-330`). Empty when MF=1's `LRP` is 0, when the first range
/// of the first isotope is not `LRU = 1, LRF = 7`, or when the sections are
/// missing (upstream leaves `nppmt` unallocated in all three).
#[must_use]
pub fn lrf7_pair_mts(endf: &Tape, mat: i32) -> Vec<i32> {
    // `lrp=l1h` of MF=1/MT=451's first record (`:247-248`).
    let Some(mf1) = endf.section(mat, 1, 451) else { return Vec::new() };
    let Some(head) = mf1.rows.first() else { return Vec::new() };
    if head[2] as i32 == 0 {
        return Vec::new();
    }
    // MF=2 HEAD, the isotope CONT, the range CONT: `lru=l1h`, `lrf=l2h`.
    let Some(mf2) = endf.section(mat, 2, 151) else { return Vec::new() };
    let Some(range) = mf2.rows.get(2) else { return Vec::new() };
    let (lru, lrf) = (range[2] as i32, range[3] as i32);
    if lru != 1 || lrf != 7 {
        return Vec::new();
    }
    // The LRF=7 CONT (row 3), then the particle-pair LIST (row 4): `npp=l1h`,
    // and the MT is word 10 of each 12-word pair, `scr(16+12*(i-1))`.
    let Some(list) = mf2.rows.get(4) else { return Vec::new() };
    let npp = list[2] as i32;
    let words: Vec<f64> = mf2.rows.iter().skip(5).flat_map(|r| r.iter().copied()).collect();
    let mut nppmt: Vec<i32> = Vec::with_capacity(usize::try_from(npp).unwrap_or(0));
    for i in 0..usize::try_from(npp).unwrap_or(0) {
        let Some(&w) = words.get(9 + 12 * i) else { break };
        nppmt.push(w.round() as i32);
    }
    // Upstream's exchange sort (`:319-329`); on integers any sort agrees.
    nppmt.sort_unstable();
    nppmt
}

/// `broadr.f90:516-523`: is section `mt` the next listed pair MT? Advances the
/// list index `n` (0-based, upstream's `n - 1`) when it is. Call it for each
/// section that reaches the test, in tape order, after the MT=18 test.
pub(super) fn forced_pair(mt: i32, nppmt: &[i32], n: &mut usize) -> bool {
    if nppmt.get(*n) == Some(&mt) {
        *n += 1;
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::forced_pair;

    /// The order-dependent match (`broadr.f90:516-523`): the index advances only
    /// on a match, so a listed MT that never comes up stalls the rest.
    #[test]
    fn the_list_is_matched_in_order_and_a_missing_mt_stalls_it() {
        let nppmt = [2, 51, 102];
        let mut n = 0;
        let hits: Vec<bool> = [2, 51, 102].iter().map(|&mt| forced_pair(mt, &nppmt, &mut n)).collect();
        assert_eq!(hits, vec![true, true, true]);
        // MT=51 absent from MF=3: MT=102 is never matched.
        let mut n = 0;
        let hits: Vec<bool> = [2, 102].iter().map(|&mt| forced_pair(mt, &nppmt, &mut n)).collect();
        assert_eq!(hits, vec![true, false]);
    }
}
