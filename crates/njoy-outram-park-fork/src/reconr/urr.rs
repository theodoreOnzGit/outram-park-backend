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
use crate::reconr::{eval_lin_lin, ReconrSection};
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

    // The parameter energies, in `eunr` order. Upstream takes `ener >= el` and
    // `ener < eh` (`reconr.f90:1500`), so the range bottom IS a grid point and
    // the range top is NOT -- the top is carried by the shaded node below.
    let mut params: Vec<f64> = Vec::new();
    match &range.case_ {
        UnresolvedCase::CaseC { l_states, .. } => {
            for l in l_states {
                for j in &l.j_states {
                    for p in &j.points {
                        params.push(p.e);
                    }
                }
            }
        }
        UnresolvedCase::CaseB {
            fission_energies, ..
        } => params.extend(fission_energies.iter().copied()),
        UnresolvedCase::CaseA { .. } => {}
    }
    params.sort_by(|a, b| a.partial_cmp(b).unwrap());
    params.dedup_by(|a, b| (*a - *b).abs() <= 1e-10 * b.abs().max(1.0));
    // NOTE: do NOT filter the list here. `rdf2u2` tests `ener >= el .and.
    // ener < eh` to decide whether to ADD an energy, but reads `enex` as the
    // next tabulated energy unconditionally (`reconr.f90:1500-1503`). Dropping
    // out-of-range entries first loses the gap-fill above the last in-range
    // parameter -- on U-234 that silently cost 7.2e4 and 8.5e4, two of the
    // twenty-seven points NJOY stores.
    let in_range = |e: f64| e >= range.el && e < range.eh;

    if params.iter().copied().filter(|&e| in_range(e)).count() == 0 {
        // Case A, and Case B with no fission grid, tabulate no energies of
        // their own. Upstream's `eunr` would then carry only the bounds; a
        // two-point lin-lin grid across two decades of a cross section varying
        // like 1/sqrt(E) is not defensible, so fill logarithmically. **This is
        // the only part of this function that is not a literal translation,
        // and it is unverified against NJOY** -- no held evaluation reaches it.
        let (lo, hi) = (range.el.max(1.0e-5), range.eh);
        for i in 1..FALLBACK_FILL {
            let f = i as f64 / FALLBACK_FILL as f64;
            ilist(sigfig(lo * (hi / lo).powf(f), 7, 0), &mut list);
        }
    } else {
        for (i, &ener) in params.iter().enumerate() {
            if !in_range(ener) {
                continue;
            }
            ilist(sigfig(ener, 7, 0), &mut list);
            // `rdf2u2`, `reconr.f90:1499-1533`: where the next parameter energy
            // is more than `wide` away, fill the gap from the built-in grid, so
            // the stored table is not linearly interpolated across a factor of
            // two in energy.
            let Some(&enex) = params.get(i + 1) else {
                continue;
            };
            if enex <= WIDE * ener {
                continue;
            }
            let mut e = ener;
            loop {
                // Advance to the first `egridu` node above `e + e/1000`
                // (`:1515-1521`).
                let Some(&next) = EGRIDU.iter().find(|&&g| g >= e + e / 1000.0) else {
                    break;
                };
                e = next;
                if e >= enex {
                    break;
                }
                ilist(sigfig(e, 7, 0), &mut list);
            }
        }
    }

    // Shaded range bounds (`reconr.f90:757-775`).
    ilist(sigfig(range.el, 7, 1), &mut list);
    ilist(sigfig(range.eh, 7, -1), &mut list);

    list.retain(|&e| e.is_finite() && e > 0.0 && e < f64::MAX);
    list
}

/// Gap factor above which `rdf2u2` fills from [`EGRIDU`] (`reconr.f90:1339`).
const WIDE: f64 = 1.26;

/// NJOY's built-in unresolved fill grid (`egridu`, `reconr.f90:1239-1251`) --
/// 78 nodes on the 1, 1.25, 1.5, 1.7, 2, 2.5, 3, 3.5, 4, 5, 6, 7.2, 8.5 pattern
/// from 10 eV to 8.5 MeV.
const EGRIDU: [f64; 78] = [
    1.0e1, 1.25e1, 1.5e1, 1.7e1, 2.0e1, 2.5e1, 3.0e1, 3.5e1, 4.0e1, 5.0e1, 6.0e1, 7.2e1, 8.5e1,
    1.0e2, 1.25e2, 1.5e2, 1.7e2, 2.0e2, 2.5e2, 3.0e2, 3.5e2, 4.0e2, 5.0e2, 6.0e2, 7.2e2, 8.5e2,
    1.0e3, 1.25e3, 1.5e3, 1.7e3, 2.0e3, 2.5e3, 3.0e3, 3.5e3, 4.0e3, 5.0e3, 6.0e3, 7.2e3, 8.5e3,
    1.0e4, 1.25e4, 1.5e4, 1.7e4, 2.0e4, 2.5e4, 3.0e4, 3.5e4, 4.0e4, 5.0e4, 6.0e4, 7.2e4, 8.5e4,
    1.0e5, 1.25e5, 1.5e5, 1.7e5, 2.0e5, 2.5e5, 3.0e5, 3.5e5, 4.0e5, 5.0e5, 6.0e5, 7.2e5, 8.5e5,
    1.0e6, 1.25e6, 1.5e6, 1.7e6, 2.0e6, 2.5e6, 3.0e6, 3.5e6, 4.0e6, 5.0e6, 6.0e6, 7.2e6, 8.5e6,
];

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

/// Build the **MF=2/MT=152** unresolved table RECONR writes to the PENDF —
/// ported from `genunr` (`reconr.f90:1628-1735`).
///
/// # What the section is for
///
/// `genunr` evaluates the infinitely-dilute unresolved cross sections on the
/// `eunr` grid and stores them in this section; `sigunr` (`:1737-1769`) then
/// interpolates *this table* to fill MF=3 at energies `eunr` does not carry.
/// Downstream, GROUPR's `stounr` reads it back — which in this crate is
/// [`crate::groupr::urr_pendf::read_urr_from_tape`], so writing it here closes
/// a loop that was previously open at both ends.
///
/// # Record layout (`sunr`, `reconr.f90:1650-1663`)
///
/// ```text
/// CONT:  ZA, AWR, LSSF, 0, 0, INTUNR
/// LIST:  TEMP, 0, NX=5, NSIG0=1, NW=1+6*NUNR, NUNR
///        body: BIG=1e10, then per energy
///              E, total, elastic, fission, capture, total-again
/// ```
///
/// The fifth column really is the total a second time
/// (`sunr(l+5)=sunr(l+1)`, `:1690`), not a transport cross section — worth
/// knowing before comparing it against a kernel whose fifth output is
/// transport.
///
/// # Two details that are easy to miss
///
/// - **For `LSSF = 0` the stored values include the MF=3 background**
///   (`:1694-1727`): `genunr` walks the evaluation's MF=3 and adds it to
///   column `ix` = 1, 2, 3, 4 for MT = 1, 2, 18, 102, updating the duplicate
///   column 5 alongside column 1. For `LSSF ≠ 0` it returns before that
///   (`:1694`) and stores the bare cross sections.
/// - **Every stored value is rounded to seven significant figures** with
///   `sigfig(...,7,0)` (`:1719`, `:1723`). That rounding is the resolution
///   floor any comparison against this table runs into, and reproducing it
///   here keeps a round-trip through
///   [`crate::groupr::urr_pendf::read_urr_from_tape`] exact rather than
///   nearly-exact.
///
/// Returns `None` when the material has no `LRU = 2` range, matching upstream's
/// `if (lrp.eq.3) call genunr` gate at `:352` — a material with no unresolved
/// parameters gets no MT=152 section.
pub fn build_mt152(
    za: f64,
    awr: f64,
    ranges: &[UnresolvedRange],
    background: &[ReconrSection],
    temperature_k: f64,
    eps: f64,
) -> Result<Option<Vec<[f64; 6]>>, NjoyError> {
    let Some(range) = ranges.first() else {
        return Ok(None);
    };
    let table = WTable::new();

    // One merged grid across every range, as `eunr` is a single sorted list.
    let mut grid: Vec<f64> = Vec::new();
    for r in ranges {
        grid.extend(unresolved_grid(r));
    }
    grid.sort_by(|a, b| a.partial_cmp(b).unwrap());
    grid.dedup_by(|a, b| (*a - *b).abs() < 1e-10 * b.abs().max(1.0));
    if grid.len() < 2 {
        return Ok(None);
    }

    // `genunr` stores the dilute values on `eunr` itself -- it does not refine
    // (refinement is this port's addition on the MF=3 side, see
    // `refine_unresolved_grid`). Storing a refined grid here would be a
    // different section from the one upstream writes.
    let mut body: Vec<f64> = Vec::with_capacity(1 + 6 * grid.len());
    body.push(INFINITE_DILUTION);
    for &e in &grid {
        let mut row = [0.0_f64; 4];
        for r in ranges {
            if e < r.el || e > r.eh {
                continue;
            }
            let out = unresolved_cross_sections(
                std::slice::from_ref(r),
                e,
                temperature_k,
                &[INFINITE_DILUTION],
                [0.0; 4],
                &table,
            )?;
            if let Some(v) = out.first() {
                // reconr.f90:1683-1687 -- abundance-weighted accumulation.
                for k in 0..4 {
                    row[k] += r.abn * v[k];
                }
            }
        }
        // reconr.f90:1694-1727 -- add the MF=3 background, LSSF=0 only.
        if range.lssf == 0 {
            for (k, mt) in [(0usize, 1i32), (1, 2), (2, 18), (3, 102)] {
                if let Some(sec) = background
                    .iter()
                    .find(|s| s.mt == MtReaction::from_any(mt))
                {
                    row[k] += eval_lin_lin(&sec.pairs, e);
                }
            }
        }
        for v in row.iter_mut() {
            *v = sigfig(*v, 7, 0);
        }
        body.extend([e, row[0], row[1], row[2], row[3], row[0]]);
    }

    let nunr = grid.len();
    let _ = eps;
    let mut rows: Vec<[f64; 6]> = Vec::with_capacity(2 + body.len().div_ceil(6));
    // CONT
    rows.push([za, awr, range.lssf as f64, 0.0, 0.0, INTUNR_LIN_LIN as f64]);
    // LIST header
    rows.push([
        temperature_k,
        0.0,
        N_REACTION_COLUMNS as f64,
        1.0,
        (1 + 6 * nunr) as f64,
        nunr as f64,
    ]);
    for chunk in body.chunks(6) {
        let mut r = [0.0_f64; 6];
        r[..chunk.len()].copy_from_slice(chunk);
        rows.push(r);
    }
    Ok(Some(rows))
}

/// Reaction columns stored in MF=2/MT=152 (`nx`): total, elastic, fission,
/// capture, and the total repeated (`reconr.f90:1690`).
const N_REACTION_COLUMNS: usize = 5;

/// The sigma-zero interpolation law RECONR records (`intunr`); `unresr.f90:532`
/// defaults it to 2 (lin-lin).
const INTUNR_LIN_LIN: i32 = 2;

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
