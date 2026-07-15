// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: src/classes/thermalHydraulics/src/physicsModels/regimeMapModels/
//             {regimeMapModel.{H,C}, oneParameter/oneParameter.{H,C}}
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream author: Stefan Radman (EPFL)
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! # `interfacial::regime_map` — one-parameter flow-regime map
//!
//! Port of GeN-Foam's `regimeMapModels::oneParameter`: given a single scalar
//! parameter (e.g. void fraction, quality, superficial mass flux), classify it
//! into one or more named flow regimes with interpolation weights that sum to
//! `1` — the mechanism GeN-Foam uses to blend between correlations (drag,
//! interfacial area, ...) that only apply within a specific regime, avoiding a
//! discontinuous jump at the regime boundary.
//!
//! ## Algorithm (faithful to upstream `oneParameter::correct()`)
//!
//! 1. At construction, each named regime is given a `[lower, upper]` window on
//!    the parameter axis. Windows are sorted ascending by `lower`. Where one
//!    regime's `upper` does not exactly meet the next regime's `lower`, an
//!    **anonymous interpolation gap** is inserted between them — the region
//!    across which the two neighbouring regimes are blended.
//! 2. At evaluation, the parameter is binned into whichever named window
//!    contains it (weight `1.0`), or, if it falls in a gap, blended between the
//!    gap's two neighbouring regimes using [`RegimeInterpolationMode::Linear`]
//!    (weights proportional to distance from each boundary) or
//!    [`RegimeInterpolationMode::Quadratic`] (a smootherstep-like blend, zero
//!    slope at both gap endpoints).
//!
//! This port keeps upstream's exact half-open window semantics: a named window
//! `[t0, t1)` claims a parameter value `p` when `t1 - p > 0` and `p - t0 >= 0`
//! (so `p == t0` belongs to the window, `p == t1` does not); an interpolation
//! gap `(t0, t1]` claims `p` when `t1 - p >= 0` and `p - t0 > 0` (the opposite
//! half-open sense).
//!
//! **Known sharp edge, faithfully reproduced, not "fixed":** because the two
//! half-open senses are opposite, they do not compose cleanly at every
//! junction. At the point where a regime *ends and a gap begins* (`p ==` that
//! regime's `upper`), neither the regime nor the gap claims `p` — the returned
//! weights sum to `0.0` there (a dropped, measure-zero point). At the point
//! where a gap *ends and the next regime begins* (`p ==` that regime's
//! `lower`), **both** the gap (with weight `0.0` for the entering regime, so
//! effectively a no-op contribution) **and** the regime itself claim `p`, so
//! that regime's total weight is `2.0` at that single point, not `1.0` — see
//! `tests.rs`'s `gap_junction_double_counts_the_entering_regime` regression
//! test. Both are genuine properties of the upstream algorithm (reproduced
//! bit-for-bit here, not introduced by this port); in practice they only
//! affect a set of parameter values of measure zero and are invisible to any
//! caller that does not probe an exact threshold value, but they are
//! documented rather than silently "cleaned up" (see the workspace's
//! guardrail against altering verified behaviour without human sign-off).
//!
//! **The two outermost regimes are unbounded, regardless of their declared
//! window.** Only the *inner*-facing edge of the lowest and highest regime is
//! ever compared against a neighbour (to decide whether a gap is needed); the
//! outer-facing edge is always replaced by the `+-`[`OUTER_BOUND_SENTINEL`]
//! sentinel. So a regime declared `[0.0, 0.3]` that happens to be the lowest
//! window in the map in fact claims every `parameter <= 0.3` down to
//! `-`[`OUTER_BOUND_SENTINEL`] (not just down to `0.0`), and likewise the
//! highest window extends up to `+`[`OUTER_BOUND_SENTINEL`] regardless of its
//! declared upper edge. This is deliberate upstream (every real parameter
//! value must classify into *some* regime, so the two extremal regimes
//! extrapolate outward rather than leaving the map's tails unclassified), not
//! an artefact of this port — see `tests.rs`'s
//! `outermost_regimes_extend_to_the_sentinel_not_their_declared_edge` test.
//!
//! The `+-1e69` numeric sentinels upstream uses for the outermost open bounds
//! are reproduced exactly (as [`OUTER_BOUND_SENTINEL`]) rather than replaced
//! with `f64::INFINITY`. That substitution was tried and rejected during
//! development: the per-window evaluation divides by the window's width `dt`,
//! and the outermost window's width is `t1 - t0` with one side infinite —
//! `(INFINITY - p) / INFINITY` is the IEEE-754 indeterminate form `inf/inf =
//! NaN`, not `1.0`. A large *finite* sentinel keeps `dt` finite (about `1e69`)
//! so the ratio evaluates normally; this is exactly why upstream picked a
//! numeric sentinel over an literal unbounded value in the first place, and is
//! preserved here rather than "improved" (see `tests.rs`'s
//! `outermost_window_sentinel_does_not_yield_nan` regression test).
//!
//! ## Deferred: `regimeMapModels::twoParameters`
//!
//! GeN-Foam's `twoParameters` regime map (`regimeMapModels/twoParameters/{twoParameters,
//! regimeBoundary2D,regimeDomain2D}.{H,C}`, ~1150 lines combined) classifies a
//! `(parameter1, parameter2)` point against a set of named polygonal regions in
//! parameter space (`regimeDomain2D`: point-in-polygon tests for both convex and
//! concave polygons, shared-boundary detection for cross-region interpolation
//! bands, bounding-box acceleration). This is a general-purpose 2D
//! computational-geometry engine, not a closed-form algebraic closure like the
//! rest of this module — porting it faithfully is a substantially larger,
//! self-contained task (candidate for a dedicated follow-up bead, and a natural
//! fit to share with any future mesh/geometry utilities elsewhere in the
//! workspace rather than duplicate). **Deferred**, not attempted here, to keep
//! this bead's output reviewable; [`RegimeMap1D`] (the `oneParameter` port) is
//! unaffected and stands alone.
//!
//! ## Also deferred: the `templatedModels`/`byRegime` dispatch layer
//!
//! `regimeMapModelTemplates.C`'s `constructModels`/`interpolateValue` and
//! `templatedModels/byRegime/*` are the C++ machinery that, given a regime map
//! and one sub-model dictionary per named regime, builds a list of
//! run-time-selected sub-models and blends their values with the regime
//! weights. That is generic multi-model dispatch across *any* closure family
//! (drag, heat transfer, interfacial area, ...), which is exactly the kind of
//! indirection the workspace's no-`dyn` rule steers away from; the solver bead
//! that owns per-pair model selection is better placed to wire "evaluate this
//! closure enum's variant selected by [`RegimeMap1D::regime_weights`]" directly
//! against the concrete closure enums in this module, rather than have this
//! module reach into every other closure family's types. **Deferred.**

/// Numeric stand-in for `+-infinity` at the outermost edges of the parameter
/// axis, matching GeN-Foam's own `+-1e69` literal exactly (see the module docs
/// for why a literal `f64::INFINITY` does not work here).
const OUTER_BOUND_SENTINEL: f64 = 1.0e69;

/// Interpolation shape used to blend two neighbouring regimes across a gap.
///
/// Closed enum port of GeN-Foam's `oneParameter::interpolationMode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegimeInterpolationMode {
    /// Weights vary linearly with distance from each boundary of the gap.
    Linear,
    /// A smootherstep-like blend: zero slope at both gap endpoints, matching
    /// upstream's `interpolationMode::quadratic` branch exactly (see
    /// [`RegimeMap1D::regime_weights`]'s implementation for the precise
    /// piecewise form — it is not a plain quadratic `t^2` ease, and is
    /// reproduced bit-for-bit rather than replaced with a "cleaner" curve).
    Quadratic,
}

/// One named regime's window `[lower, upper]` on the parameter axis, as read
/// from GeN-Foam's `regimeBounds { "name" (lower upper); ... }` dictionary
/// entry. `lower`/`upper` may be given in either order — [`RegimeMap1D::new`]
/// normalizes them, matching upstream's own swap-if-reversed step.
#[derive(Debug, Clone, PartialEq)]
pub struct RegimeBound {
    /// The regime's name (must be unique within the map).
    pub name: String,
    /// One edge of the window.
    pub lower: f64,
    /// The other edge of the window.
    pub upper: f64,
}

/// One evaluation slot: either a named regime window, or an anonymous
/// interpolation gap between two neighbouring named regimes.
#[derive(Debug, Clone, PartialEq)]
enum Slot {
    Regime(String),
    Gap,
}

/// A one-parameter flow-regime map: classifies a scalar parameter into named
/// regimes with interpolation weights summing to `1`.
///
/// Closed-form (no `dyn`, no mesh/field access) port of GeN-Foam's
/// `regimeMapModels::oneParameter`. Build with [`RegimeMap1D::new`]; evaluate
/// per-cell with [`RegimeMap1D::regime_weights`]. See the module docs for the
/// algorithm and the exact half-open boundary convention.
#[derive(Debug, Clone, PartialEq)]
pub struct RegimeMap1D {
    /// Length `slots.len() + 1`; `slots[i]` occupies `[thresholds[i],
    /// thresholds[i+1]]`.
    thresholds: Vec<f64>,
    slots: Vec<Slot>,
    interpolation_mode: RegimeInterpolationMode,
}

impl RegimeMap1D {
    /// Build a regime map from its (unsorted, possibly edge-reversed) named
    /// windows, reproducing GeN-Foam's sort-and-gap-insertion construction
    /// exactly. Panics if `regimes` is empty (a regime map with no regimes is
    /// a caller programming error, not a recoverable runtime state — same as
    /// upstream indexing `bounds[0]` unconditionally).
    #[must_use]
    pub fn new(regimes: Vec<RegimeBound>, interpolation_mode: RegimeInterpolationMode) -> Self {
        assert!(
            !regimes.is_empty(),
            "RegimeMap1D::new: at least one regime is required"
        );

        // Normalize each window so `lower <= upper` (upstream: the swap loop
        // ahead of the bubble sort).
        let mut sorted: Vec<RegimeBound> = regimes
            .into_iter()
            .map(|b| {
                if b.lower > b.upper {
                    RegimeBound {
                        name: b.name,
                        lower: b.upper,
                        upper: b.lower,
                    }
                } else {
                    b
                }
            })
            .collect();
        // Upstream's adjacent-swap bubble sort by `lower` is a stable ascending
        // sort; `sort_by` with a stable comparator reproduces the same final
        // ordering (including tie-breaking) for any well-formed input.
        sorted.sort_by(|a, b| a.lower.partial_cmp(&b.lower).expect("regime bound is NaN"));

        let mut thresholds = vec![-OUTER_BOUND_SENTINEL];
        let mut slots = vec![Slot::Regime(sorted[0].name.clone())];
        for i in 1..sorted.len() {
            if sorted[i - 1].upper != sorted[i].lower {
                slots.push(Slot::Gap);
                thresholds.push(sorted[i - 1].upper);
            }
            slots.push(Slot::Regime(sorted[i].name.clone()));
            thresholds.push(sorted[i].lower);
        }
        thresholds.push(OUTER_BOUND_SENTINEL);

        Self {
            thresholds,
            slots,
            interpolation_mode,
        }
    }

    /// Classify `parameter` into its regime(s), returning `(regime_name,
    /// weight)` pairs whose weights sum to `1.0` (a single `(name, 1.0)` pair
    /// when `parameter` falls squarely inside one regime's window; two pairs,
    /// weighted by [`RegimeMap1D::interpolation_mode`], when it falls in an
    /// interpolation gap between two regimes; an empty `Vec` when `parameter`
    /// falls outside every window — upstream's own behaviour; since the
    /// outermost windows extend to `+-`[`OUTER_BOUND_SENTINEL`], this only
    /// occurs for `NaN` or a `parameter` magnitude larger than the sentinel).
    ///
    /// One evaluation per call (per-cell in upstream); no mesh or field state.
    #[must_use]
    pub fn regime_weights(&self, parameter: f64) -> Vec<(String, f64)> {
        let mut out = Vec::new();
        for i in 0..self.slots.len() {
            let t0 = self.thresholds[i];
            let t1 = self.thresholds[i + 1];
            let dt = t1 - t0;
            let t1_minus_p_over_dt = (t1 - parameter) / dt;
            let p_minus_t0_over_dt = (parameter - t0) / dt;

            match &self.slots[i] {
                Slot::Regime(name) => {
                    if t1_minus_p_over_dt > 0.0 && p_minus_t0_over_dt >= 0.0 {
                        out.push((name.clone(), 1.0));
                    }
                }
                Slot::Gap => {
                    // A gap always sits strictly between two named regimes by
                    // construction (see `new`).
                    let name0 = match &self.slots[i - 1] {
                        Slot::Regime(n) => n.clone(),
                        Slot::Gap => {
                            unreachable!("two adjacent gaps: construction invariant violated")
                        }
                    };
                    let name1 = match &self.slots[i + 1] {
                        Slot::Regime(n) => n.clone(),
                        Slot::Gap => {
                            unreachable!("two adjacent gaps: construction invariant violated")
                        }
                    };
                    match self.interpolation_mode {
                        RegimeInterpolationMode::Linear => {
                            let c0 = t1_minus_p_over_dt;
                            let c1 = p_minus_t0_over_dt;
                            if c0 >= 0.0 && c1 > 0.0 {
                                out.push((name0, c0));
                                out.push((name1, c1));
                            }
                        }
                        RegimeInterpolationMode::Quadratic => {
                            let c0_raw = p_minus_t0_over_dt;
                            let c1_raw = t1_minus_p_over_dt;
                            if c0_raw >= 0.0 && c1_raw > 0.0 {
                                let mid = t0 + dt / 2.0;
                                let c0 = if parameter <= mid {
                                    1.0 - 2.0 * c0_raw * c0_raw
                                } else {
                                    2.0 * c1_raw * c1_raw
                                };
                                let c1 = 1.0 - c0;
                                out.push((name0, c0));
                                out.push((name1, c1));
                            }
                        }
                    }
                }
            }
        }
        out
    }
}
