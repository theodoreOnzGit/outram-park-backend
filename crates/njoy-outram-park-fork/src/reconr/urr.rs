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
//! Energy-independent cases tabulate no points of their own, and each has its
//! own construction upstream rather than a shared fallback: Case A walks
//! `egridu` across the whole range (`rdf2u0`), Case B seeds from its
//! fission-width grid and fills gaps (`rdf2u1`). Both are translated
//! literally; see [`unresolved_grid`] for the table and for what was wrong
//! here before 2026-09-15.
//!
//! # Where this port deliberately does not match NJOY
//!
//! One place, on one class of evaluation. RECONR's `unfac`
//! (`reconr.f90:4473-4496`) has **no `l >= 3` branch** — `if (l.eq.0) … else
//! if (l.eq.1) … else if (l.eq.2) … endif`, no `else` — so `vl` and `ps` are
//! left at their previous values, and `csunr1` re-applies `vl = vl*e2`
//! (`:4010`) inside its J loop, multiplying `vl` by another `sqrt(E)` per
//! J-state. On an evaluation with `NLS > 3` that inflates the `l >= 3` neutron
//! width by `E` and drives the stored cross section orders of magnitude past
//! the unitarity limit.
//!
//! This port does not reproduce it, and that is not a local decision:
//! [`crate::unresr::penetrability_factor`] clamps `l >= 2` to the `l = 2`
//! formula because NJOY's **UNRESR** does exactly that — `uunfac`
//! (`unresr.f90:1213-1239`) writes a bare `else` where RECONR's `unfac` and
//! PURR's `unfac2` (`purr.f90:1487-1511`) write `else if (l.eq.2)`. The kernel
//! here ports `uunfac`, so it is faithful to its own source, and the defect is
//! confined to the two routines that fall through.
//!
//! Measured on Fe-58 (ENDF/B-VIII.0 Beta4, `NLS = 4`): NJOY stores
//! `1.42e4 b` at 350 keV rising to `6.62e5 b` at 3 MeV, against this port's
//! `4.57 b` and `4.76 b`, which track the evaluation's own MF=3.
//! `tests/reconr_mt152_all_unresolved_cases_vs_njoy2016.rs` reproduces NJOY's
//! numbers from the defective recurrence to 2.7e-7, so this is a diagnosis
//! rather than a difference of opinion. It affects `LSSF = 1` materials only
//! in the self-shielding *ratios* UNRESR/PURR take, where the scale cancels.

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

/// Energies at which to evaluate the unresolved cross sections for `range` —
/// upstream's `eunr`, built the way upstream builds it.
///
/// Each ENDF representation gets its own construction, because upstream gives
/// each its own subroutine and they genuinely differ:
///
/// | case | upstream | energies seeded | gap-fill step |
/// |---|---|---|---|
/// | A (`LFW=0`) | `rdf2u0`, `reconr.f90:1288-1308` | none — walks [`EGRIDU`] across the whole range unconditionally | `E + E/100` |
/// | B (`LFW=1`, `LRF≠2`) | `rdf2u1`, `:1370-1400` | the shared fission-width grid, in range | `E + E/100`, only where the next energy exceeds [`WIDE`]·`E` |
/// | C (`LRF=2`) | `rdf2u2`, `:1497-1534` | the first J-state of the first L-state only | `E + E/1000`, same [`WIDE`] condition |
///
/// Every case then gets the shaded range ends `sigfig(el,7,+1)` and
/// `sigfig(eh,7,-1)` (`reconr.f90:757-775`), and the whole list is kept sorted
/// and duplicate-free by upstream's own [`ilist`].
///
/// Three details worth stating, each of which was wrong here before
/// 2026-09-15 and each of which is now pinned by a test:
///
/// - **Case A does not interpolate over a log fill.** Until 2026-09-15 this
///   function treated "the parameters tabulate no energies of their own" as a
///   fallback and filled 40 log-spaced points. `rdf2u0` does nothing of the
///   kind — it walks [`EGRIDU`] from `el` to `eh`. On Fe-58 the two differ in
///   *count* (13 against 42) as well as in position, so the section produced
///   was not the one NJOY writes. All three cases are now gated against NJOY
///   in `tests/reconr_mt152_all_unresolved_cases_vs_njoy2016.rs` — Case B on a
///   synthetic tape, no evaluation held here using that format.
/// - **The gap-fill step is 1 % for A and B but 0.1 % for C.** Upstream really
///   does write `ener+ener/100` in `rdf2u0`/`rdf2u1` and `ener+ener/1000` in
///   `rdf2u2`.
/// - **Case C seeds from the first J-state of the first L-state alone**
///   (`if (n.eq.1.and.l.eq.1)`, `:1497`), not from the union over all of them.
///   The union happens to agree on U-234, where every J tabulates the same
///   mesh, which is why taking it went unnoticed.
///
/// `rdf2u0` runs its walk once per L-state, and `rdf2u1`/`rdf2u2` push
/// duplicates freely; upstream removes them later in the sort at
/// `reconr.f90:856-871`. [`ilist`] removes them on insertion instead, which is
/// why the loops here run once.
pub(crate) fn unresolved_grid(range: &UnresolvedRange) -> Vec<f64> {
    // `ilist` expects a list primed with a sentinel larger than anything that
    // will be inserted (`unresr.f90:753-781`).
    let mut list = vec![f64::MAX];

    /// Advance to the first [`EGRIDU`] node at or above `e + e/step_divisor`,
    /// as the inner `do while (... enut.lt.ener+ener/N)` loops do.
    fn next_grid_node(e: f64, step_divisor: f64) -> Option<f64> {
        EGRIDU.iter().copied().find(|&g| g >= e + e / step_divisor)
    }

    match &range.case_ {
        // `rdf2u0`, `reconr.f90:1288-1308`. No seeded energies at all: walk
        // `egridu` from `el` up, adding every node strictly below `eh`.
        UnresolvedCase::CaseA { .. } => {
            let mut ener = range.el;
            while ener < range.eh {
                let Some(next) = next_grid_node(ener, 100.0) else {
                    break;
                };
                ener = next;
                if ener < range.eh {
                    ilist(sigfig(ener, 7, 0), &mut list);
                }
            }
        }

        // `rdf2u1`, `reconr.f90:1370-1400`. The shared fission-width grid,
        // with `egridu` filling any gap wider than `wide`. Note there is NO
        // unconditional walk here — a Case B range whose fission grid lies
        // entirely outside `[el, eh)` contributes only the shaded bounds,
        // which is upstream's behaviour and not an oversight.
        UnresolvedCase::CaseB {
            fission_energies, ..
        } => {
            seed_with_gap_fill(fission_energies, 100.0, range.el, range.eh, &mut list);
        }

        // `rdf2u2`, `reconr.f90:1497-1534`, guarded by `if (n.eq.1.and.l.eq.1)`
        // — the first J-state of the first L-state seeds the grid on its own.
        UnresolvedCase::CaseC { l_states, .. } => {
            let first: Vec<f64> = l_states
                .first()
                .and_then(|l| l.j_states.first())
                .map(|j| j.points.iter().map(|p| p.e).collect())
                .unwrap_or_default();
            seed_with_gap_fill(&first, 1000.0, range.el, range.eh, &mut list);
        }
    }

    // Shaded range bounds (`reconr.f90:757-775`). Upstream also pushes
    // `sigfig(el,7,-1)` and `sigfig(eh,7,+1)`, but the overlap filter at
    // `:856-871` drops both — verified against every reference tape here:
    // no MT=152 section carries them.
    ilist(sigfig(range.el, 7, 1), &mut list);
    ilist(sigfig(range.eh, 7, -1), &mut list);

    list.retain(|&e| e.is_finite() && e > 0.0 && e < f64::MAX);
    list
}

/// The seed-plus-gap-fill loop shared by `rdf2u1` and `rdf2u2`.
///
/// For each tabulated energy inside the range: insert it, then — when the
/// *next* tabulated energy is more than [`WIDE`] times it — walk [`EGRIDU`]
/// from it toward that next energy, inserting each node passed. `step_divisor`
/// is 100 for `rdf2u1` and 1000 for `rdf2u2`, and `el`/`eh` bound the range
/// (`ener >= el .and. ener < eh`, so the bottom is a candidate and the top is
/// not).
///
/// `enex` is read as the next entry of the tabulated list **unconditionally**,
/// without first discarding out-of-range entries (`reconr.f90:1500-1503`).
/// Filtering first loses the fill above the last in-range energy — on U-234
/// that silently cost `7.2e4` and `8.5e4`, two of the twenty-seven points NJOY
/// stores.
fn seed_with_gap_fill(energies: &[f64], step_divisor: f64, el: f64, eh: f64, list: &mut Vec<f64>) {
    for (i, &ener) in energies.iter().enumerate() {
        if !(ener >= el && ener < eh) {
            continue;
        }
        ilist(sigfig(ener, 7, 0), list);
        let Some(&enex) = energies.get(i + 1) else {
            continue;
        };
        if enex <= WIDE * ener {
            continue;
        }
        let mut e = ener;
        loop {
            let Some(next) = EGRIDU.iter().copied().find(|&g| g >= e + e / step_divisor) else {
                break;
            };
            e = next;
            if e >= enex {
                break;
            }
            ilist(sigfig(e, 7, 0), list);
        }
    }
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
    let _ = eps;
    let bkg = |mt: i32, e: f64| {
        background
            .iter()
            .find(|s| s.mt == MtReaction::from_any(mt))
            .map(|sec| eval_lin_lin(&sec.pairs, e))
    };
    let Some(t) = SunrTable::build(ranges, temperature_k, bkg)? else {
        return Ok(None);
    };
    Ok(Some(t.rows(za, awr, temperature_k)))
}

/// `genunr`'s table (`reconr.f90:1628-1735`) — the infinitely-dilute
/// unresolved cross sections on `eunr`, with the MF=3 background added for
/// `LSSF = 0` — and `sigunr`, which interpolates it (`:1737-1769`).
///
/// Upstream's order of rounding is kept, because each step is a `sigfig`:
/// every range's contribution is rounded as it is accumulated, then each
/// MF=3 background in tape order (MT=1, 2, 18, 102), and the fifth column is
/// the total again.
#[derive(Debug, Clone)]
pub(crate) struct SunrTable {
    /// `eunr` and `[total, elastic, fission, capture, total]` at each.
    pub(crate) rows: Vec<(f64, [f64; 5])>,
    /// `intunr` (`sunr(6)`).
    pub(crate) intunr: i32,
    lssf: i32,
}

impl SunrTable {
    /// Build the table. `bkg(mt, e)` is the MF=3 background of `mt` at `e`,
    /// or `None` when the tape has no such section; upstream reads it with
    /// `gety1` of the tape's own TAB1.
    pub(crate) fn build(
        ranges: &[UnresolvedRange],
        temperature_k: f64,
        bkg: impl Fn(i32, f64) -> Option<f64>,
    ) -> Result<Option<Self>, NjoyError> {
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

        let mut rows: Vec<(f64, [f64; 5])> = Vec::with_capacity(grid.len());
        for &e in &grid {
            let mut row = [0.0_f64; 5];
            // `if (mode.ge.11.and.e.ge.elt(j).and.e.lt.eht(j))` (:1672):
            // the range's own top energy gets no unresolved contribution.
            for r in ranges {
                if e < r.el || e >= r.eh {
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
                    for k in 0..4 {
                        row[k] = sigfig(row[k] + r.abn * v[k], 7, 0);
                    }
                }
            }
            row[4] = row[0];
            rows.push((e, row));
        }
        // reconr.f90:1694-1727 -- add the MF=3 background, LSSF=0 only, in
        // tape order, rounding after each addition.
        if range.lssf == 0 {
            for (k, mt) in [(0usize, 1i32), (1, 2), (2, 18), (3, 102)] {
                for (e, row) in rows.iter_mut() {
                    if let Some(b) = bkg(mt, *e) {
                        row[k] = sigfig(row[k] + b, 7, 0);
                        if k == 0 {
                            row[4] = sigfig(row[4] + b, 7, 0);
                        }
                    }
                }
            }
        }
        Ok(Some(SunrTable {
            rows,
            intunr: stored_interpolation_law(ranges),
            lssf: range.lssf,
        }))
    }

    /// `sigunr` (`reconr.f90:1737-1769`): the four cross sections at `e`,
    /// interpolated with `intunr`; zero outside the table.
    pub(crate) fn sigunr(&self, e: f64) -> [f64; 4] {
        let n = self.rows.len();
        let mut i = 1usize; // 1-based
        let mut en = 0.0;
        let mut l2 = 0usize;
        while i < n && e >= en {
            i += 1;
            l2 = i - 1; // 0-based row of `sunr(l2)`
            en = self.rows[l2].0.abs();
        }
        let l1 = l2.saturating_sub(1);
        let (e1, e2) = (self.rows[l1].0.abs(), self.rows[l2].0.abs());
        let law = crate::endf::interp::IntLaw::from_code(self.intunr as u32);
        let mut out = [0.0; 4];
        for (k, o) in out.iter_mut().enumerate() {
            if e >= e1 && e <= e2 {
                *o = crate::endf::interp::terp1(e1, self.rows[l1].1[k], e2, self.rows[l2].1[k], e, law)
                    .unwrap_or(0.0);
            }
        }
        out
    }

    /// The MF=2/MT=152 rows as `genunr` lays them out.
    pub(crate) fn rows(&self, za: f64, awr: f64, temperature_k: f64) -> Vec<[f64; 6]> {
        let nunr = self.rows.len();
        let mut body: Vec<f64> = Vec::with_capacity(1 + 6 * nunr);
        body.push(INFINITE_DILUTION);
        for (e, r) in &self.rows {
            body.extend([*e, r[0], r[1], r[2], r[3], r[4]]);
        }
        let mut rows: Vec<[f64; 6]> = Vec::with_capacity(2 + body.len().div_ceil(6));
        // CONT
        rows.push([za, awr, self.lssf as f64, 0.0, 0.0, self.intunr as f64]);
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
        rows
    }
}

/// Reaction columns stored in MF=2/MT=152 (`nx`): total, elastic, fission,
/// capture, and the total repeated (`reconr.f90:1690`).
const N_REACTION_COLUMNS: usize = 5;

/// `rdfil2`'s default interpolation law for the stored table
/// (`intunr=5`, log-log, `reconr.f90:809`).
///
/// It stands for Case A and Case B, which have no `INT` of their own. Case C
/// overrides it from the evaluation — see [`stored_interpolation_law`].
const INTUNR_DEFAULT: i32 = 5;

/// The interpolation law `genunr` records as `sunr(6)` for `ranges`
/// (`reconr.f90:1656`), which `sigunr` later reads back to interpolate the
/// stored table (`:1749`).
///
/// [`INTUNR_DEFAULT`] unless a Case C range supplies one. `rdf2u2` assigns
/// `intunr=l1h` inside its `do l` / `do n` loops (`:1487`) with no guard, so
/// the **last** J-state of the **last** L-state is the one whose value
/// survives — deliberately not the first, which is what seeds the energy grid.
/// Every evaluation held here gives all J-states the same `INT`, so the
/// distinction is untested; it is written this way because that is what
/// upstream does.
///
/// Measured: ENDF/B-VIII.0 U-234 stores 2 (lin-lin) and U-238 stores 5
/// (log-log); Fe-58, being Case A, takes the default 5.
fn stored_interpolation_law(ranges: &[UnresolvedRange]) -> i32 {
    let mut intunr = INTUNR_DEFAULT;
    for r in ranges {
        if let UnresolvedCase::CaseC { l_states, .. } = &r.case_ {
            for l in l_states {
                for j in &l.j_states {
                    intunr = j.int_;
                }
            }
        }
    }
    intunr
}

/// Add the unresolved contribution of every `LSSF = 0` range to the MF=3
/// sections, as `resxs` and `emerge` do (`reconr.f90:2240-2569`,
/// `4786-4815`):
///
/// - The grid is the union grid inside the range, bisected with `resxs`'s
///   own test on the function `sigma` returns there. That function is
///   `sigunr`, the [`SunrTable`] interpolated with `intunr`. The
///   step-increase rule is off, because it applies only below `eresr`
///   (`:2413`).
/// - At each point the value is `sigunr(e)` with **no** MF=3 background for
///   MT=1, 2, 18/19 and 102 (`emerge`, `:4789-4790`). For `LSSF = 0` the table
///   already carries the background.
///
/// ~~Evaluate the unresolved formula directly at each point, add the
/// background lin-lin, and refine with this crate's own bisection
/// (`refine_unresolved_grid`).~~ **REPLACED 2026-09-26** (GitHub #340). That
/// put points NJOY does not have in U-234's unresolved range (1.55, 1.6 and
/// 1.775 keV), wrote values unrounded, and kept `eresr` = 1500 eV itself,
/// which `lunion` drops.
///
/// `LSSF = 1` ranges are skipped: `eresh = eresr` then (`:353`), so neither
/// `resxs` nor `emerge` reaches them.
pub(super) fn add_unresolved_ranges(
    sections: &mut Vec<ReconrSection>,
    ranges: &[UnresolvedRange],
    table: &SunrTable,
    eps: f64,
    raw: &super::RawMf3,
    zero_background: (f64, f64),
) {
    for range in ranges {
        if range.lssf != 0 {
            continue;
        }
        super::rebuild_range_with(
            sections,
            range.el,
            range.eh,
            Vec::new(),
            eps,
            &[],
            raw,
            super::RebuildOpts {
                widen: false,
                zero_background: Some(zero_background),
            },
            |e| {
                let v = table.sigunr(e);
                super::RangeDelta {
                    total: v[0],
                    elastic: v[1],
                    fission: v[2],
                    capture: v[3],
                    ..super::RangeDelta::default()
                }
            },
        );
    }
}
