// Translated from NJOY2016.80 `src/reconr.f90` (`ab27c64`): `rdfil2`'s range
// bookkeeping (:697-871, the `lssf` set by `rdf2bw` :926, `rdf2aa` :1048, `rdf2hy`
// :1138, `rdf2u0/u1/u2` :1271, :1362, :1474, and `eresh=eresr` at :353), `resxs`'s
// single walk (:2240-2569, its crossings at :2351-2353 and step rule at :2421), and
// `sigma` (:2571-2667), which sums every section of every isotope at an energy and
// adds `sigunr` on top.
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! **One `resxs` walk over every section** (GitHub #1324, #666).
//!
//! The port's usual path rebuilds each resolved range on its own, on the tape's own
//! MF=3 background, then the unresolved range the same way. That is upstream's
//! answer only when no two sections cover the same energy. Two cases break it, and a
//! third needs the one walk's call order:
//!
//! - **`NIS > 1`**: two isotopes' ranges over the same energies. Upstream `sigma`
//!   adds `sigp*abn` of every section at `e` (`reconr.f90:2595-2646`); a second
//!   per-range rebuild would replace the first isotope's contribution instead.
//! - **A resolved-unresolved overlap** (`eresu < eresr`). `sigma` adds `sigunr` on
//!   top of the resolved sum for `eresu <= e < eresh` (`:2660-2666`); the
//!   unresolved rebuild would discard the resolved part there.
//! - **`tempr > 0` with an MLBW range on a seam** (added 2026-11-24, GitHub #340).
//!   `csmlbw2` keeps g/h between calls, and the per-range path tests the seam panel
//!   only after both ranges are rebuilt; the one walk calls `sigma` across the seam
//!   in `resxs`'s order.
//!
//! [`walk_every_section`] does what `resxs` does in those cases: one walk from
//! `eresl` to `eresh` on the function `sigma` returns. It reuses
//! [`super::rebuild_range_with`] for each stretch between the crossings `resxs`
//! does not test (`eresr`, `eresu`, `eresm`, `:2351-2353`), with the step-increase
//! rule on below `eresr` only (`:2421`). Materials without either case keep the
//! per-range path, so nothing held here moves.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use super::{
    csbw, rebuild_range_with, seam, urr::SunrTable, EnergyRange, RangeDelta, RawMf3, RebuildOpts, ReconrSection,
    ResonanceFormalism, ResonanceInfo,
};
use crate::unresr::mf2::UnresolvedRange;
use crate::NjoyError;

/// `rdfil2`'s `elow` and its `small` (`reconr.f90:690-693`).
const ELOW: f64 = 1.0e-5;
const ELOW_SMALL: f64 = 1.0e-6;

/// The energy bounds and the `lssf` flag `rdfil2` leaves for `resxs`, `genunr`
/// and `emerge` (`reconr.f90:697-871`, `:353`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Rdfil2Bounds {
    /// Lowest lower bound of any range.
    pub(crate) eresl: f64,
    /// Highest upper bound of an `LRU <= 1` range, clamped to `[eresl, eresh]`.
    pub(crate) eresr: f64,
    /// Highest upper bound of any range, as `rdfil2` leaves it (before `:353`).
    pub(crate) eresh: f64,
    /// Lowest lower bound of an `LRU = 2` range, lowered only when that bound is
    /// not `elow` (`:745-755`); `9e9` when none is.
    pub(crate) eresu: f64,
    /// Lowest upper bound of an `LRU = 2` range; `9e9` when none.
    pub(crate) eresm: f64,
    /// Upstream's one `lssf` variable: the last range read sets it, `rdf2bw`,
    /// `rdf2aa` and `rdf2hy` to 0 and `rdf2u0/u1/u2` to the range's `LSSF`.
    /// `rdsammy` (LRF = 7) leaves it as it was.
    pub(crate) lssf: i32,
    /// `intunr`, the law `genunr` stores for the MT=152 table: `rdfil2` resets
    /// it to 5 before every range (`reconr.f90:809`) and `rdf2u2` sets it from
    /// each J-state, so the last range read decides it.
    pub(crate) intunr: i32,
}

impl Rdfil2Bounds {
    /// From the parsed MF=2 (`res_info`, file order) and its `LRU = 2` ranges
    /// (`urr`, every isotope, the same order).
    pub(crate) fn new(res_info: &ResonanceInfo, urr: &[UnresolvedRange]) -> Self {
        const ELARGE: f64 = 9.0e9;
        let mut b = Rdfil2Bounds { eresl: ELARGE, eresr: 0.0, eresh: 0.0, eresu: ELARGE, eresm: ELARGE, lssf: 0, intunr: 5 };
        let mut next_urr = urr.iter();
        for r in &res_info.ranges {
            b.intunr = 5;
            b.eresl = b.eresl.min(r.el);
            b.eresh = b.eresh.max(r.eh);
            if r.lru <= 1 {
                b.eresr = b.eresr.max(r.eh);
            }
            if r.lru == 2 {
                if (r.el - ELOW).abs() > ELOW_SMALL {
                    b.eresu = b.eresu.min(r.el);
                }
                b.eresm = b.eresm.min(r.eh);
                if let Some(u) = next_urr.next() {
                    b.lssf = u.lssf;
                    b.intunr = super::urr::stored_interpolation_law(core::slice::from_ref(u));
                }
            } else if !matches!(r.formalism, Some(ResonanceFormalism::RMatrixLimited)) {
                b.lssf = 0;
            }
        }
        if b.eresr < b.eresl {
            b.eresr = b.eresl;
        }
        if b.eresr > b.eresh {
            b.eresr = b.eresh;
        }
        b
    }

    /// `eresh` after `if (lssf.gt.0) eresh=eresr` (`reconr.f90:353`): where
    /// `resxs` stops, `sigma` stops adding `sigunr`, and `emerge`'s zeroed
    /// background ends.
    pub(crate) fn eresh_walk(&self) -> f64 {
        if self.lssf > 0 {
            self.eresr
        } else {
            self.eresh
        }
    }

    /// `resxs`'s early return (`reconr.f90:2275-2279`): an unresolved-only
    /// material with `lssf = 1` has no resonance points (`nrtot = 0`), so
    /// `emerge` treats no reaction as a resonance reaction.
    pub(crate) fn no_resonance_points(&self) -> bool {
        self.lssf == 1 && self.eresl == self.eresu
    }

    /// Whether the material needs [`walk_every_section`]: more than one isotope,
    /// or a resolved-unresolved overlap (`rdfil2`'s message "mat has
    /// resolved-unresolved overlap", `:867-870`).
    pub(crate) fn needs_one_walk(&self, res_info: &ResonanceInfo) -> bool {
        res_info.ranges.iter().any(|r| r.isotope > 0) || self.eresu < self.eresr
    }
}

/// How one section's `sigp` is computed at an energy (`sigma`'s `mode`).
enum SectionKind {
    /// `csnorp` (mode 0, `LRU = 0`): potential scattering `4 pi AP^2` alone.
    NoParameters,
    /// `csslbw` / `csmlbw` / `csmlbw2` (modes 1, 2).
    Slbw(csbw::BwRange),
    /// `csrmat` (mode 3).
    Rm,
    /// `csaa` (mode 4).
    Aa,
}

/// `sigp` of one section at `e` (`reconr.f90:2603-2637`), before the clamp.
fn section_sigp(range: &EnergyRange, kind: &SectionKind, e: f64, ctx: &csbw::BwContext) -> RangeDelta {
    match kind {
        SectionKind::NoParameters => {
            RangeDelta { elastic: 4.0 * core::f64::consts::PI * range.ap * range.ap, ..RangeDelta::default() }
        }
        SectionKind::Slbw(p) => csbw::bw_delta(range, p, e, ctx),
        SectionKind::Rm => seam::rm_delta(range, e),
        SectionKind::Aa => match &range.aa {
            Some(aa_range) => {
                let s = super::aa_sigmas(e, aa_range, range.ap, ctx);
                RangeDelta { total: s.total, elastic: s.elastic, fission: s.fission, capture: s.capture, ..RangeDelta::default() }
            }
            None => RangeDelta::default(),
        },
    }
}

/// Refuse what [`walk_every_section`] cannot evaluate: an R-matrix-limited range
/// (its extra particle-pair channels and `cssammy`'s per-section state are not
/// carried through a multi-section sum).
pub(crate) fn check_supported(res_info: &ResonanceInfo) -> Result<(), NjoyError> {
    if res_info.ranges.iter().any(|r| r.lru == 1 && matches!(r.formalism, Some(ResonanceFormalism::RMatrixLimited))) {
        return Err(NjoyError::NotPorted(
            "RECONR's one-walk path (NIS > 1, or a resolved-unresolved overlap) with an \
             R-matrix-limited (LRF=7) range",
        ));
    }
    Ok(())
}

/// `sigma` at `e`: every `LRU <= 1` section on its `[el, eh)`, clamped and times
/// its abundance, in file order (`reconr.f90:2595-2646`), then `sigunr` for
/// `eresu <= e < eresh` without a clamp (`:2660-2666`).
fn sigma(
    sections: &[(&EnergyRange, SectionKind)],
    sunr: Option<&SunrTable>,
    eresu: f64,
    eresh: f64,
    e: f64,
    ctx: &csbw::BwContext,
) -> RangeDelta {
    let mut sig = RangeDelta::default();
    for (range, kind) in sections {
        if !(e >= range.el && e < range.eh) {
            continue;
        }
        let p = section_sigp(range, kind, e, ctx).clamp_negative_to_zero().times_abundance(range.abn);
        sig.total += p.total;
        sig.elastic += p.elastic;
        sig.fission += p.fission;
        sig.capture += p.capture;
        // `sigp(5)`, the SLBW/MLBW competitive width (GitHub #1731).
        sig.other[0] += p.other[0];
    }
    if let Some(t) = sunr {
        if e >= eresu && e < eresh {
            let v = t.sigunr(e);
            sig.total += v[0];
            sig.elastic += v[1];
            sig.fission += v[2];
            sig.capture += v[3];
        }
    }
    sig
}

/// One `resxs` walk from `eresl` to `eresh` over every section of every isotope,
/// written into `sections` as `emerge` writes it. See the module doc.
///
/// The walk is split at the crossings `resxs` accepts without a test (`eresr`,
/// `eresu`, `eresm`, `reconr.f90:2351-2353`); each stretch is one
/// [`rebuild_range_with`], with the step-increase rule on below `eresr` and the
/// MF=3 background of MT=1, 2, 18/19 and 102 zeroed on `[eresr, eresh)`
/// (`emerge`, `:4789-4790`).
///
/// `ctx` carries the temperature (GitHub #1323): a `csmlbw2` range makes the walk
/// sequential. `competitive_mts` is `[51]` when `emerge` writes the competitive
/// width there (GitHub #1731).
#[allow(clippy::too_many_arguments)]
pub(crate) fn walk_every_section(
    sections: &mut Vec<ReconrSection>,
    res_info: &ResonanceInfo,
    sunr: Option<&SunrTable>,
    bounds: &Rdfil2Bounds,
    eps: f64,
    raw: &RawMf3,
    ctx: &csbw::BwContext,
    competitive_mts: &[i32],
) {
    let kinds: Vec<(&EnergyRange, SectionKind)> = res_info
        .ranges
        .iter()
        .filter(|r| r.lru <= 1)
        .filter_map(|r| {
            let kind = match (r.lru, r.formalism) {
                (0, _) => SectionKind::NoParameters,
                (_, Some(ResonanceFormalism::Slbw)) | (_, Some(ResonanceFormalism::Mlbw)) => {
                    SectionKind::Slbw(csbw::prepare(r))
                }
                (_, Some(ResonanceFormalism::ReichMoore)) => SectionKind::Rm,
                (_, Some(ResonanceFormalism::AdlerAdler)) => SectionKind::Aa,
                _ => return None,
            };
            Some((r, kind))
        })
        .collect();
    let eresh = bounds.eresh_walk();
    let (eresl, eresr, eresu) = (bounds.eresl, bounds.eresr, bounds.eresu);
    // Above `eresr` the step rule is off, so the walk state does not matter and
    // a stretch may end at a crossing. Below it the state must carry across, so
    // `eresu`/`eresm` there are crossings inside one stretch, accepted untested.
    let inside = |c: f64| c > eresl && c < eresh;
    let mut cuts: Vec<f64> = [eresr, eresu, bounds.eresm].into_iter().filter(|&c| inside(c) && c >= eresr).collect();
    cuts.sort_by(|a, b| a.partial_cmp(b).unwrap());
    cuts.dedup();
    let below = |c: f64| if inside(c) && c < eresr { Some(c) } else { None };
    let crossings_below = [below(eresu), below(bounds.eresm)];
    let mut edges = vec![eresl];
    edges.extend(cuts);
    edges.push(eresh);
    let in_order = kinds.iter().any(|(r, _)| csbw::needs_ordered_walk(r, ctx));
    let delta = |e: f64| sigma(&kinds, sunr, eresu, eresh, e, ctx);
    for w in edges.windows(2) {
        let (lo, hi) = (w[0], w[1]);
        let widen = hi <= eresr;
        let opts = RebuildOpts {
            widen,
            zero_background: Some((eresr, eresh)),
            untested_crossings: if widen { crossings_below } else { [None, None] },
            in_order,
        };
        rebuild_range_with(sections, lo, hi, Vec::new(), eps, competitive_mts, raw, opts, delta);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reconr::mf2::EnergyRange;

    fn range(el: f64, eh: f64, lru: i32, formalism: Option<ResonanceFormalism>, isotope: usize) -> EnergyRange {
        EnergyRange {
            el,
            eh,
            lru,
            formalism,
            spi: 0.0,
            ap: 0.5,
            naps: 0,
            l_states: Vec::new(),
            rm_l_states: Vec::new(),
            rml: None,
            aa: None,
            abn: 1.0,
            isotope,
            nro: 0,
            ap_table: None,
        }
    }

    /// `rdfil2`'s bookkeeping (GitHub #666 item 4): `eresu` is not lowered by an
    /// unresolved range starting at `elow`; an LRU = 0/1 range read after an
    /// unresolved one resets `lssf` to 0; an LRF = 7 range leaves it.
    #[test]
    fn bounds_follow_rdfil2() {
        let slbw = Some(ResonanceFormalism::Slbw);
        let info = ResonanceInfo {
            ranges: vec![range(1.0e-5, 100.0, 1, slbw, 0), range(1.0e-5, 1.0e3, 2, None, 0), range(1.0e-5, 50.0, 1, slbw, 1)],
        };
        let urr = crate::unresr::mf2::UnresolvedRange {
            el: 1.0e-5,
            eh: 1.0e3,
            abn: 1.0,
            naps: 0,
            nro: 0,
            ap_table: None,
            lssf: 1,
            case_: crate::unresr::mf2::UnresolvedCase::CaseA { spi: 0.0, ap: 0.5, awri: 1.0, l_states: Vec::new() },
        };
        let b = Rdfil2Bounds::new(&info, core::slice::from_ref(&urr));
        assert_eq!(b.eresu, 9.0e9, "an LRU=2 range at elow does not lower eresu");
        assert_eq!(b.eresm, 1.0e3);
        assert_eq!(b.lssf, 0, "the SLBW range read last resets lssf");
        assert_eq!((b.eresl, b.eresr, b.eresh), (1.0e-5, 100.0, 1.0e3));
        assert!(b.needs_one_walk(&info), "two isotopes");

        let info2 = ResonanceInfo { ranges: vec![range(1.0e-5, 100.0, 1, Some(ResonanceFormalism::RMatrixLimited), 0), range(80.0, 1.0e3, 2, None, 0)] };
        let mut b2 = Rdfil2Bounds::new(&info2, core::slice::from_ref(&urr));
        assert_eq!((b2.lssf, b2.eresu), (1, 80.0));
        assert_eq!(b2.eresh_walk(), 100.0, "lssf > 0: eresh = eresr");
        assert!(b2.needs_one_walk(&info2), "resolved-unresolved overlap 80 < 100");
        assert!(check_supported(&info2).is_err(), "LRF=7 is refused on the one-walk path");
        b2.eresl = 80.0;
        assert!(b2.no_resonance_points());
    }
}
