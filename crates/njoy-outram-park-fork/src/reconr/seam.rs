// Translated from NJOY2016.80 `src/reconr.f90`: `resxs` (:2240-2569) where its one
// walk crosses the boundary between two resolved ranges, and `sigma` (:2571-2660),
// whose `e.ge.elt(i).and.e.lt.eht(i)` test decides which range a point belongs to.
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! **The seam between two resolved ranges** (gh:#1369 round 5, deck 17).
//!
//! Upstream `resxs` reconstructs every resolved range in **one** walk over
//! `lunion`'s grid. `sigma` takes each range on `[el, eh)`, so at a boundary
//! `eh(a) = el(b)` the panel from `sigfig(eh,7,-1)` (range `a`) to `sigfig(el,7,+1)`
//! (range `b`) is tested like any other panel. Its midpoint, the boundary itself
//! after `sigfig(.,7,0)`, belongs to range `b`. Where the two ranges' cross sections
//! do not join, the panel fails the test and is bisected, until the significant-figure
//! test stops it. The only crossings `resxs` skips are at `eresr`, `eresu` and `eresm`
//! (`:2351-2353`), none of which is a resolved-resolved seam.
//!
//! This port's `rebuild_range_with` refines each range on its own grid `[el, eh]`, so
//! that panel was never tested. JENDL-3.3 U-238 (11 ranges) and Pu-239 (3) are the
//! evaluations where it shows (NJOY2016 test 17). On U-238, NJOY has `1.000000+3`. On
//! Pu-239, NJOY bisects from `9.999990+2` to `1.000000+3` across a 7 % step.
//!
//! [`refine_resolved_seams`] tests each such panel after the per-range rebuild, with
//! `refine_panel` (the same test and step-increase rule as inside a range). It then
//! writes every resonance reaction at the new points as `emerge` does, and the total as
//! the sum of its parts. Seams are handled between SLBW/MLBW and Reich-Moore ranges
//! (the formalisms whose per-energy evaluation is [`range_delta`]). A seam with an
//! R-matrix-limited or Adler-Adler range on either side is left as before. No held
//! evaluation has one, and none is refused silently: the per-range result is unchanged
//! there.

use super::{
    channel_radius, csslbw_elastic_step, eval_lin_lin, eval_mlbw_lstate, eval_mlbw_lstate_radii, eval_slbw_lstate_parts, refine_panel,
    rebuild_total_as_sum_of_parts, rm, EnergyRange, MtReaction, PanelAccept, RangeDelta, RawMf3, RefineTolerances, ReconrSection,
    ResonanceFormalism, ResonanceInfo, WalkState, ERRTOL_OVERRIDE,
};
use crate::mixr::mix::sigfig;

/// One SLBW/MLBW l-state's energy-independent constants: `(l, awri, ra, resonances)`.
pub(super) type SlbwLState = (u32, f64, f64, Vec<(f64, f64, f64, f64, f64, f64)>);

/// Hoist each l-state's channel radius and resonance tuples out of the energy loop
/// (`rdf2bw`'s `ra`).
pub(super) fn slbw_prepare(range: &EnergyRange) -> Vec<SlbwLState> {
    range
        .l_states
        .iter()
        .map(|ls| {
            let ra = channel_radius(ls.awri, range.naps, range.ap);
            let tuples: Vec<_> = ls.resonances.iter().map(|r| r.as_tuple()).collect();
            (ls.l, ls.awri, ra, tuples)
        })
        .collect()
}

/// The SLBW (`csslbw`) or MLBW (`csmlbw`) resonance contribution of `range` at `e`,
/// before `sigma`'s clamp and abundance (upstream `sigma`, `reconr.f90:2610-2616`).
pub(super) fn slbw_delta(range: &EnergyRange, prepared: &[SlbwLState], e: f64) -> RangeDelta {
    let mlbw = matches!(range.formalism, Some(ResonanceFormalism::Mlbw));
    let mut d = RangeDelta::default();
    for (l, awri, ra, tuples) in prepared {
        if !mlbw {
            // `csslbw`: the running elastic sum is rounded to 8 figures
            // after every l-state (`reconr.f90:2849-2851`).
            let (s, spot) = eval_slbw_lstate_parts(e, tuples, *l, range.spi, range.ap, *awri, *ra);
            d.elastic = csslbw_elastic_step(d.elastic, s.elastic, spot);
            d.capture += s.capture;
            d.fission += s.fission;
            continue;
        }
        let s = if let Some(table) = &range.ap_table {
            // NRO != 0 (GitHub #663), `csmlbw` at `reconr.f90:2893-2916`:
            // the phase shift takes AP(E); the radius at the energy is
            // the formula (NAPS=0), AP(E) (NAPS=1) or the constant AP
            // (NAPS=2). `ra` stays `rdf2bw`'s radius for the resonance
            // energies. `read_ap_table` guarantees `at` cannot fail here.
            let ape = table.at(e).unwrap_or(range.ap);
            let ra_e = match range.naps {
                0 => channel_radius(*awri, 0, 0.0),
                1 => ape,
                _ => range.ap,
            };
            eval_mlbw_lstate_radii(e, tuples, *l, range.spi, ape, *awri, ra_e, *ra)
        } else {
            eval_mlbw_lstate(e, tuples, *l, range.spi, range.ap, *awri, *ra)
        };
        d.elastic += s.elastic;
        d.capture += s.capture;
        d.fission += s.fission;
    }
    d.total = d.elastic + d.capture + d.fission;
    d
}

/// The Reich-Moore (`csrmat`) resonance contribution of `range` at `e`, before
/// `sigma`'s clamp and abundance. One call over the whole range: `csrmat` sums every
/// l before applying `pifac`, and its `gf` flag spans the l-states.
pub(super) fn rm_delta(range: &EnergyRange, e: f64) -> RangeDelta {
    let s: rm::RmSigmas = rm::eval_rm_range(e, &range.rm_l_states, range.ap, range.spi, range.naps);
    RangeDelta { total: s.total, elastic: s.elastic, fission: s.fission, capture: s.capture, ..RangeDelta::default() }
}

/// A resolved range whose per-energy evaluation this module can call.
enum SeamRange {
    Slbw(Vec<SlbwLState>),
    Rm,
}

/// `sigma`'s contribution of one range at `e`: zero outside `[el, eh)`, else the
/// clamped contribution times the abundance (`reconr.f90:2596-2645`).
fn range_delta(range: &EnergyRange, kind: &SeamRange, e: f64) -> RangeDelta {
    if !in_sigma_range(range.el, range.eh, e) {
        return RangeDelta::default();
    }
    let d = match kind {
        SeamRange::Slbw(p) => slbw_delta(range, p, e),
        SeamRange::Rm => rm_delta(range, e),
    };
    d.clamp_negative_to_zero().times_abundance(range.abn)
}

/// `sigma`'s test of whether a section contributes at `e`: `e.ge.elt(i).and.e.lt.eht(i)`
/// (`reconr.f90:2596`), so a boundary energy belongs to the upper range.
fn in_sigma_range(el: f64, eh: f64, e: f64) -> bool {
    e >= el && e < eh
}

/// The sum `sigma` forms over two ranges at `e` (each on its own `[el, eh)`).
fn pair_delta(a: (&EnergyRange, &SeamRange), b: (&EnergyRange, &SeamRange), e: f64) -> RangeDelta {
    let (da, db) = (range_delta(a.0, a.1, e), range_delta(b.0, b.1, e));
    let mut d = RangeDelta {
        total: da.total + db.total,
        elastic: da.elastic + db.elastic,
        fission: da.fission + db.fission,
        capture: da.capture + db.capture,
        ..RangeDelta::default()
    };
    for k in 0..d.other.len() {
        d.other[k] = da.other[k] + db.other[k];
    }
    d
}

/// Test and refine the panel across every boundary where one resolved SLBW/MLBW/RM
/// range ends and the next begins (`eh(a) = el(b)`), as `resxs`'s single walk does,
/// and write the new points into the resonance reactions as `emerge` does.
///
/// Called after the per-range rebuild (`add_resonance_contributions`), when every
/// section already carries both shaded nodes of the boundary. The panel's lower end is
/// the last grid point below the boundary, its upper end the first at or above it. The
/// step-increase rule sees the point before the panel as the last one written
/// (`res(1)`), with `in > 3`.
pub(super) fn refine_resolved_seams(sections: &mut [ReconrSection], res_info: &ResonanceInfo, eps: f64, raw: &RawMf3) {
    let mut ranges: Vec<(&EnergyRange, SeamRange)> = res_info
        .ranges
        .iter()
        .filter(|r| r.lru == 1)
        .filter_map(|r| match r.formalism {
            Some(ResonanceFormalism::Slbw) | Some(ResonanceFormalism::Mlbw) if !r.l_states.is_empty() => Some((r, SeamRange::Slbw(slbw_prepare(r)))),
            Some(ResonanceFormalism::ReichMoore) if !r.rm_l_states.is_empty() => Some((r, SeamRange::Rm)),
            _ => None,
        })
        .collect();
    ranges.sort_by(|x, y| x.0.el.partial_cmp(&y.0.el).unwrap());
    let mut tol = RefineTolerances::defaults(eps);
    if let Some((errmax, errint)) = ERRTOL_OVERRIDE.with(|c| c.get()) {
        tol.errmax = errmax;
        tol.errint = errint;
    }
    for w in ranges.windows(2) {
        let ((a, ka), (b, kb)) = (&w[0], &w[1]);
        let boundary = b.el;
        if (a.eh - boundary).abs() > 1.0e-10 * boundary {
            continue;
        }
        // The section grid around the boundary (every resonance reaction carries it).
        let Some(grid) = sections.iter().find(|s| s.mt == MtReaction::Mt2Elastic).map(|s| s.energies()) else { continue };
        let k = grid.partition_point(|&e| e < boundary);
        if k < 2 || k >= grid.len() {
            continue;
        }
        let (e_prev, e1, e2) = (grid[k - 2], grid[k - 1], grid[k]);
        let delta = |e: f64| pair_delta((a, ka), (b, kb), e);
        let (d1, d2) = (delta(e1), delta(e2));
        let mut state = WalkState { written: 4, last_written: e_prev };
        let (mut out_e, mut out_d, mut flags): (Vec<f64>, Vec<RangeDelta>, Vec<PanelAccept>) = (Vec::new(), Vec::new(), Vec::new());
        refine_panel(e1, d1, e2, d2, &delta, tol, 0, Some(&mut state), &mut out_e, &mut out_d, &mut flags);
        // Then the start of range `b` with the carried state (see `rewalk_from`).
        let upper = grid[k..].iter().copied().take_while(|&e| e < b.eh).collect::<Vec<f64>>();
        rewalk_from(&upper, &delta, tol, &mut state, &mut out_e, &mut out_d);
        if out_e.is_empty() {
            continue;
        }
        write_points(sections, &out_e, &out_d, raw);
        rebuild_total_as_sum_of_parts(sections, &out_e);
    }
}

/// Continue `resxs`'s walk into the upper range with the state the seam left
/// (`in`, `res(1)`), over the grid the per-range rebuild made (`grid[0]` is the
/// seam panel's upper end, `sigfig(el,7,+1)`). That rebuild started its own walk
/// with `in = 0`, so its first four panels could not be widened
/// (`if (in.gt.3.and.dx.gt.est...)`, `reconr.f90:2419-2421`); upstream's walk,
/// arriving from the lower range, can. Each panel is re-tested with `refine_panel`
/// and the carried state, and new points go to `out_e`/`out_d`.
///
/// The walk stops at the first panel from the fourth on (`k >= 3`) that it leaves
/// whole. From the next panel on, the per-range walk had `in > 3` and the same
/// `res(1)`, so its decisions are this walk's. Where this walk splits a panel that
/// the per-range walk kept, `res(1)` is closer and `est` smaller, so it splits at
/// least wherever the per-range walk did: the existing points are a subset and none
/// has to be removed.
fn rewalk_from(
    grid: &[f64],
    delta: &impl Fn(f64) -> RangeDelta,
    tol: RefineTolerances,
    state: &mut WalkState,
    out_e: &mut Vec<f64>,
    out_d: &mut Vec<RangeDelta>,
) {
    for k in 0..grid.len().saturating_sub(1) {
        let (e1, e2) = (grid[k], grid[k + 1]);
        let n_before = out_e.len();
        let mut flags = Vec::new();
        refine_panel(e1, delta(e1), e2, delta(e2), delta, tol, 0, Some(state), out_e, out_d, &mut flags);
        if k >= 3 && out_e.len() == n_before {
            return;
        }
    }
}

/// Add `(e, emerge's value)` at each new point to the four fixed resonance reactions
/// (MT 2, 18 or 19, 102; MT=1 is rebuilt as the sum of its parts by the caller): the
/// background is `gety1` of the tape's own TAB1 at `e` (else lin-lin of the section),
/// plus the resonance term, rounded once to 7 figures, with `emerge`'s elastic floor
/// `1e-8` (`reconr.f90:4791-4851`). MT=1 receives a placeholder the caller overwrites.
fn write_points(sections: &mut [ReconrSection], es: &[f64], ds: &[RangeDelta], raw: &RawMf3) {
    for mt in [
        MtReaction::Mt1Total,
        MtReaction::Mt2Elastic,
        MtReaction::Mt18Fission,
        MtReaction::Mt19FirstChanceFission,
        MtReaction::Mt102Capture,
    ] {
        let Some(sec) = sections.iter_mut().find(|s| s.mt == mt) else { continue };
        let bg = sec.pairs.clone();
        let mut g = raw.iter().find(|(m, _)| MtReaction::from_any(*m) == mt).map(|(_, t)| crate::endf::gety1::Gety1::new(t));
        for (&e, d) in es.iter().zip(ds) {
            let base = match g.as_mut() {
                Some(g) => g.get(e).y,
                None => eval_lin_lin(&bg, e),
            };
            let add = match mt {
                MtReaction::Mt1Total => d.total,
                MtReaction::Mt2Elastic => d.elastic,
                MtReaction::Mt102Capture => d.capture,
                _ => d.fission,
            };
            let mut sn = base + add;
            if mt == MtReaction::Mt2Elastic && sn <= 1.0e-8 {
                sn = 1.0e-8;
            }
            let at = sec.pairs.partition_point(|p| p.0 < e);
            sec.pairs.insert(at, (e, sigfig(sn, 7, 0)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `sigma`'s `[el, eh)` dispatch: at the boundary the upper range alone
    /// contributes, just below it the lower range alone, outside both nothing.
    #[test]
    fn a_boundary_energy_belongs_to_the_upper_range() {
        assert!(!in_sigma_range(1.0e-5, 1000.0, 1000.0));
        assert!(in_sigma_range(1000.0, 2000.0, 1000.0));
        assert!(in_sigma_range(1.0e-5, 1000.0, 999.999));
        assert!(!in_sigma_range(1000.0, 2000.0, 999.999));
    }
}
