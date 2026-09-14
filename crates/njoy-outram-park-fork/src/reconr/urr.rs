// Ported from NJOY2016 `src/reconr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
//   - `subroutine genunr`, l.1628-1735 — the infinitely-dilute unresolved table,
//     its `if (lssf.ne.0) return` gate (l.1694) and the MF=3 background it adds
//     afterwards.
//   - `subroutine sigunr`, l.1737-1769 — retrieval of that table during
//     reconstruction (called at l.2662).
//   - the `eunr` grid: `rdf2u0`/`rdf2u1` (l.1303, 1380-1400, 1510-1531), the
//     range-boundary shading at l.757-775 and the sort at l.856-865.
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! Unresolved-resonance-range (LRU=2) contributions to MF=3.
//!
//! # What this is for
//!
//! In the unresolved range an evaluation gives only *average* resonance
//! parameters, so there are no resonances to reconstruct. RECONR nonetheless
//! has work to do there, and its own header says so (`reconr.f90:81-87`):
//!
//! > If unresolved parameters are present, the infinitely dilute cross sections
//! > are computed on a special energy grid chosen to preserve the required
//! > interpolation properties. This table is added to the pendf tape using a
//! > special format in MF2/MT152, and the table is also used to compute the
//! > unresolved contributions in MF3.
//!
//! PURR and UNRESR own the *self-shielded* treatment. The *infinitely dilute*
//! one belongs here, and until 2026-09-14 this port did not do it at all.
//!
//! # `LSSF` decides whether it matters — and it is why this was invisible
//!
//! - **`LSSF = 1`** — the evaluation's MF=3 already carries the
//!   infinitely-dilute unresolved cross sections. RECONR adds nothing
//!   (`genunr`'s `if (lssf.ne.0) return`, `reconr.f90:1694`), so doing nothing
//!   was already correct. ENDF/B-VIII.0 **U-235 and U-238 are both `LSSF = 1`**.
//! - **`LSSF = 0`** — MF=3 does *not* carry them, and the window is left at
//!   **zero cross section** if they are not reconstructed.
//!
//! Every case this repository runs is built on U-235/U-238, which is the entire
//! reason a zero-cross-section hole survived unnoticed. Measured on **U-234**
//! (ENDF/B-VIII.0, MAT 9225, `LSSF = 0`, URR `1.5e3 .. 1.0e5 eV`), MT=1 at
//! 1700 eV read `0.0` against NJOY2016's `2.032567e1 b`. A zero total cross
//! section is an infinite flight in transport. See `bn:op-12lu` / gh:#202's
//! sibling thread.
//!
//! # Why this is wiring rather than a 632-line port
//!
//! RECONR computes the dilute cross sections with its own `csunr1`/`csunr2`
//! (`reconr.f90:3826-4317`, 491 lines). Those were **not** ported, and they did
//! not need to be: [`crate::unresr::unresolved_cross_sections`] — the ETOX
//! kernel UNRESR already uses — reduces to exactly the same numbers when it is
//! handed a single dilution of `1e10`. That is measured, not assumed. Against
//! NJOY2016's own U-234 PENDF, at `sig0 = 1e10`:
//!
//! | E (eV) | total | elastic | fission | capture |
//! |---|---|---|---|---|
//! | 1.7e3 | 2.032567e1 | 1.779554e1 | 6.687414e-3 | 2.523447e0 |
//! | 5.0e3 | 1.616041e1 | 1.476491e1 | 9.153020e-3 | 1.386346e0 |
//! | 1.0e4 | 1.438603e1 | 1.335173e1 | 1.291686e-2 | 1.021381e0 |
//!
//! — identical to every one of the 7 figures NJOY prints, on all four
//! reactions. This is the "search the workspace before building" rule paying
//! for itself: an earlier note on the bead had estimated a 632-line port.
//!
//! That agreement holds at **26 of the 34 points** NJOY's PENDF carries in
//! U-234's unresolved window. The other 8 are points NJOY *interpolates* from
//! its MF=2/MT=152 table via `sigunr` rather than evaluating, and each is
//! reproduced to 1e-7 or better by lin-lin interpolation of NJOY's own
//! neighbours — so where this port and NJOY differ there, **this port is the
//! more accurate of the two**. Established in
//! `tests/reconr_urr_kernel_vs_njoy2016.rs`, which also records the three
//! competing explanations that were tested and refuted first.
//!
//! # The grid
//!
//! NJOY builds `eunr` from the energies at which the unresolved parameters are
//! tabulated, plus the range boundaries shaded by `sigfig(e,7,∓1)`. For U-234
//! that is 34 points running `1.50000e3, 1.70000e3, 2.00000e3, … 9.00000e4,
//! 9.99999e4` — and `9.99999e4` is exactly `sigfig(1e5,7,-1)`, the shaded top.
//! This module reproduces that: the union of every `UnresolvedPointC` energy in
//! the range (via [`crate::unresr::mf2::ilist`], upstream's own insertion
//! helper), clipped to the range and closed with the shaded end points.
//!
//! Energy-independent cases (A, and B outside its fission grid) tabulate no
//! points of their own, so the grid falls back to the range ends plus a
//! log-spaced fill — flagged in [`unresolved_grid`] as the one place here that
//! is *not* a literal translation.

use crate::endf::mt::MtReaction;
use crate::mixr::mix::sigfig;
use crate::reconr::ReconrSection;
use crate::unresr::mf2::{ilist, UnresolvedCase, UnresolvedRange};
use crate::unresr::unresolved_cross_sections;
use crate::unresr::wfun::WTable;
use crate::NjoyError;

/// The single dilution at which RECONR tabulates: infinite, as `genunr`'s
/// `big = 1.e10_kr` (`reconr.f90:1644`).
const INFINITE_DILUTION: f64 = 1.0e10;

/// Number of log-spaced fill points for a range whose parameters carry no
/// energy grid of their own. Not an upstream constant — see [`unresolved_grid`].
const FALLBACK_FILL: usize = 40;

/// Energies at which to evaluate the unresolved cross sections for `range`.
///
/// Mirrors `eunr`: every energy the unresolved parameters are tabulated at,
/// inside `[el, eh]`, plus the shaded range ends `sigfig(el,7,+1)` and
/// `sigfig(eh,7,-1)` (`reconr.f90:757-775`), sorted and duplicate-free via
/// upstream's own [`ilist`].
///
/// **One deliberate divergence.** Case A, and Case B away from its fission
/// grid, tabulate no energies of their own, so upstream's `eunr` would carry
/// only the boundaries. A two-point lin-lin grid across two decades of a
/// cross section that varies like `1/sqrt(E)` is not defensible, so a
/// log-spaced fill of [`FALLBACK_FILL`] points is added for those cases. This
/// is the only part of this module that is not a literal translation, and it
/// is **unverified against NJOY** — no held evaluation exercises it (U-234,
/// the only `LSSF = 0` material here, is Case C).
fn unresolved_grid(range: &UnresolvedRange) -> Vec<f64> {
    // `ilist` expects a list primed with a sentinel larger than anything that
    // will be inserted (`unresr.f90:753-781`).
    let mut list = vec![f64::MAX];

    let mut tabulated = 0usize;
    if let UnresolvedCase::CaseC { l_states, .. } = &range.case_ {
        for l in l_states {
            for j in &l.j_states {
                for p in &j.points {
                    if p.e > range.el && p.e < range.eh {
                        ilist(p.e, &mut list);
                        tabulated += 1;
                    }
                }
            }
        }
    }
    if let UnresolvedCase::CaseB {
        fission_energies, ..
    } = &range.case_
    {
        for &e in fission_energies {
            if e > range.el && e < range.eh {
                ilist(e, &mut list);
                tabulated += 1;
            }
        }
    }

    if tabulated == 0 {
        // See the divergence note on this function.
        let (lo, hi) = (range.el.max(1.0e-5), range.eh);
        for i in 1..FALLBACK_FILL {
            let f = i as f64 / FALLBACK_FILL as f64;
            ilist(lo * (hi / lo).powf(f), &mut list);
        }
    }

    ilist(sigfig(range.el, 7, 1), &mut list);
    ilist(sigfig(range.eh, 7, -1), &mut list);

    list.retain(|&e| e.is_finite() && e > 0.0 && e < f64::MAX);
    list
}

/// Bisection depth cap per interval while refining the unresolved grid.
const MAX_REFINE_DEPTH: usize = 10;

/// Refine `grid` until lin-lin interpolation reproduces all four unresolved
/// cross sections to within `eps` everywhere.
///
/// This is the *"special energy grid chosen to preserve the required
/// interpolation properties"* of `reconr.f90:82-83`, reached by the same
/// bisect-until-converged means the resolved range already uses rather than by
/// reproducing `eunr`'s construction literally. The parameter energies alone
/// are not enough: U-234 tabulates 10 of them across `1.5e3 .. 1.0e5 eV`, where
/// NJOY's own PENDF carries 34 points, and interpolating the 10-point grid
/// leaves up to 8.3e-2 relative error against NJOY at the points in between.
///
/// Refinement is driven by the **worst of the four reactions** at each midpoint,
/// not by the total alone — fission is the smallest and most curved of them on
/// U-234, and a grid converged only on the total leaves it visibly coarse.
fn refine_unresolved_grid(
    range: &UnresolvedRange,
    grid: Vec<f64>,
    table: &WTable,
    eps: f64,
) -> Result<Vec<(f64, [f64; 4])>, NjoyError> {
    let eval = |e: f64| -> Result<[f64; 4], NjoyError> {
        let out = unresolved_cross_sections(
            std::slice::from_ref(range),
            e,
            0.0,
            &[INFINITE_DILUTION],
            [0.0; 4],
            table,
        )?;
        Ok(out.first().map_or([0.0; 4], |r| [r[0], r[1], r[2], r[3]]))
    };

    let mut points: Vec<(f64, [f64; 4])> = Vec::with_capacity(grid.len() * 4);
    for &e in &grid {
        points.push((e, eval(e)?));
    }
    points.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

    let mut out: Vec<(f64, [f64; 4])> = Vec::with_capacity(points.len() * 4);
    for w in points.windows(2) {
        let (lo, hi) = (w[0], w[1]);
        out.push(lo);
        // Explicit stack rather than recursion, so the depth cap is visible.
        let mut stack = vec![(lo, hi, 0usize)];
        let mut inserted: Vec<(f64, [f64; 4])> = Vec::new();
        while let Some((a, b, depth)) = stack.pop() {
            if depth >= MAX_REFINE_DEPTH {
                continue;
            }
            let em = 0.5 * (a.0 + b.0);
            if em <= a.0 || em >= b.0 {
                continue;
            }
            let vm = eval(em)?;
            let t = (em - a.0) / (b.0 - a.0);
            let converged = (0..4).all(|k| {
                let lin = a.1[k] + t * (b.1[k] - a.1[k]);
                let scale = vm[k].abs().max(1.0e-10);
                (vm[k] - lin).abs() <= eps * scale
            });
            if !converged {
                inserted.push((em, vm));
                stack.push((a, (em, vm), depth + 1));
                stack.push(((em, vm), b, depth + 1));
            }
        }
        inserted.sort_by(|x, y| x.0.partial_cmp(&y.0).unwrap());
        out.extend(inserted);
    }
    if let Some(&last) = points.last() {
        out.push(last);
    }
    out.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    out.dedup_by(|a, b| (a.0 - b.0).abs() < 1e-10 * b.0.abs().max(1.0));
    Ok(out)
}

/// Add the infinitely-dilute unresolved contribution of every `LSSF = 0` range
/// to the MF=3 sections.
///
/// `LSSF = 1` ranges are skipped, exactly as `genunr` returns early for them
/// (`reconr.f90:1694`) — their MF=3 already carries the same quantity, and
/// adding it again would double-count.
///
/// The five values [`unresolved_cross_sections`] returns are
/// `[total, elastic, fission, capture, transport]`; the first four map to
/// MT=1/2/18/102 and transport is unused here. Each is **added to** whatever
/// MF=3 background is already present at that energy, which is what `genunr`
/// does after its `lssf` gate ("add on unresolved background from mf3 on endf
/// tape", `reconr.f90:1695-1697`).
pub(super) fn add_unresolved_ranges(
    sections: &mut [ReconrSection],
    ranges: &[UnresolvedRange],
    eps: f64,
) -> Result<(), NjoyError> {
    let table = WTable::new();

    for range in ranges {
        if range.lssf != 0 {
            continue;
        }
        let mut grid = unresolved_grid(range);

        // The MF=3 background keeps its own energies inside the range. Dropping
        // them and re-sampling on the unresolved grid alone would erase any
        // structure the background has in this window -- a threshold opening,
        // say -- because the replacement grid knows nothing about it. Measured
        // on U-234: the worst residual against NJOY sat at 4.51800e4 eV, an
        // energy carried by the background and by NJOY's PENDF but not by the
        // unresolved parameter grid.
        {
            let mut list = std::mem::replace(&mut grid, Vec::new());
            list.push(f64::MAX);
            for mt in [
                MtReaction::Mt1Total,
                MtReaction::Mt2Elastic,
                MtReaction::Mt18Fission,
                MtReaction::Mt102Capture,
            ] {
                if let Some(sec) = sections.iter().find(|s| s.mt == mt) {
                    for &(e, _) in &sec.pairs {
                        if e > range.el && e < range.eh {
                            ilist(e, &mut list);
                        }
                    }
                }
            }
            list.retain(|&e| e.is_finite() && e > 0.0 && e < f64::MAX);
            grid = list;
        }
        if grid.len() < 2 {
            continue;
        }

        // [(energy, [total, elastic, fission, capture])], adaptively refined.
        let rows = refine_unresolved_grid(range, grid, &table, eps)?;
        if rows.is_empty() {
            continue;
        }

        for (slot, mt) in [
            (0usize, MtReaction::Mt1Total),
            (1, MtReaction::Mt2Elastic),
            (2, MtReaction::Mt18Fission),
            (3, MtReaction::Mt102Capture),
        ] {
            let Some(sec) = sections.iter_mut().find(|s| s.mt == mt) else {
                continue;
            };
            // Fission is absent from a non-fissionable evaluation's MF=3; a
            // zero row would otherwise invent an empty section's worth of
            // points. Upstream only touches sections it finds.
            if rows.iter().all(|(_, v)| v[slot] == 0.0) {
                continue;
            }

            let bg = sec.pairs.clone();
            let mut merged: Vec<(f64, f64)> = bg
                .iter()
                .copied()
                .filter(|&(e, _)| e < range.el || e > range.eh)
                .collect();
            for &(e, v) in &rows {
                merged.push((e, v[slot] + crate::reconr::eval_lin_lin(&bg, e)));
            }
            merged.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
            merged.dedup_by(|a, b| (a.0 - b.0).abs() < 1e-10 * b.0.abs().max(1.0));
            sec.pairs = merged;
        }
    }
    Ok(())
}
