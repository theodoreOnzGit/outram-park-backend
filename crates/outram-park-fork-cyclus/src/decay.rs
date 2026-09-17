// PROVENANCE
//   Upstream project: CYCLUS <https://github.com/cyclus/cyclus>
//   Upstream file:    src/uniform_taylor.h, src/uniform_taylor.cc,
//                     src/decayer.h, src/decayer.cc
//   Upstream commit:  d4faab7ce0566ccb8febfcf50915cdeedee59db0
//   Upstream licence: BSD-3-Clause
//   Copyright (c) 2010-2016, University of Wisconsin Computational Nuclear
//   Engineering Research Group. All rights reserved.
//
//   Independent Rust translation, (C) 2026 Theodore Ong and the outram-park
//   contributors, GPL-3.0-only. BSD-3-Clause is GPLv3-compatible; the
//   relicensing is ONE-WAY: code here cannot go back upstream.

//! Radioactive decay: the Bateman equations, solved by matrix exponential.
//!
//! # ⚠️ This is upstream's DEPRECATED solver, not its live one
//!
//! Read this before comparing anything here against a Cyclus run.
//!
//! This module translates `cyclus/src/uniform_taylor.cc` faithfully. But
//! upstream calls `UniformTaylor::MatrixExpSolver` from exactly one site —
//! `decayer.cc:159` — and `Decayer` is marked DEPRECATED in its own header and
//! **is never instantiated anywhere in Cyclus**. (`decayer.h` is `#include`d
//! by `composition.cc` and `material.cc`, which is what makes it look live.)
//!
//! Upstream's **actual** decay path is CRAM: `Composition::NewDecay` builds a
//! sparse decay matrix from `pyne_cram_transmute_info` and calls
//! `pyne_cram_expm_multiply14`, a 14th-order Chebyshev Rational
//! Approximation.
//!
//! So **this module will not reproduce a modern Cyclus decay result.** It is a
//! correct matrix-exponential solver — verified against the closed-form
//! two-species Bateman solution to 7.7e-11 at tight tolerance — but it is a
//! different algorithm from the one upstream actually runs.
//!
//! Two further consequences, both measured:
//!
//! * **The truncation error is one-signed, so it accumulates.** Every
//!   discarded term of the truncated Poisson series is non-negative. At
//!   upstream's default `tol = 1e-3` that is a **0.095 % atom loss over one
//!   year** of the Sr-90 chain, and an isobaric chain that conserves mass by
//!   construction **loses 0.093 % of it**. Tightening to `1e-12` brings the
//!   residual to ~1e-13 for 172 series terms against 127, since the term count
//!   grows only logarithmically in the tolerance. Prefer the
//!   `*_with_tol` variants for anything whose mass balance matters; upstream
//!   exposes no way to ask for this.
//! * **`Decayer::BuildDecayMatrix` carries a real defect** which this port
//!   deliberately does not reproduce — see [`build_decay_matrix`]. In short, a
//!   "gross heuristic for mostly stable nuclides" in fact freezes every
//!   nuclide with a half-life under ~31.3 days as stable.
//!
//! **Adding a CRAM backend is tracked as `op-i68k`**, and
//! `crates/outram-park-fork-onix/src/cram.rs` already has `cram16()` — reuse
//! it rather than writing a third matrix-exponential routine. Uniform Taylor
//! should stay alongside as an independent check.
//!
//!
//! # The physics in one paragraph
//!
//! A decay chain is a linear, constant-coefficient system. Write `n(t)` for
//! the column vector of **atom counts** (any consistent unit — atoms, moles,
//! or the `kg/u` this module uses internally; decay is linear, so the scale
//! cancels). Then
//!
//! ```text
//! dn/dt = A n,      n(t) = exp(A t) n(0)
//! ```
//!
//! where `A` is the **decay matrix**, in units of per second:
//!
//! ```text
//! A[j][j] = -lambda_j                      (nuclide j decaying away)
//! A[i][j] = branch_ratio(j -> i) * lambda_j  (nuclide j feeding daughter i)
//! ```
//!
//! Note the convention, because getting it backwards is the single likeliest
//! defect in this file and is exactly what the Bateman test below exists to
//! catch: **the parent owns a column, the daughter owns a row.** Column sums
//! are `-lambda_j + lambda_j * (sum of branching ratios)`, which is zero when
//! the branching ratios sum to one — that is atom conservation, written as a
//! property of the matrix.
//!
//! # The solver
//!
//! [`UniformTaylor`] is a direct port of upstream's `uniform_taylor.cc`: the
//! Taylor series with **uniformization** (also called Jensen's method or
//! randomization). Set `alpha = max |A[i][i]|` and `B = A + alpha I`. Then
//!
//! ```text
//! exp(A t) = exp(-alpha t) exp(B t) = exp(-alpha t) sum_k (t^k / k!) B^k
//! ```
//!
//! Every entry of `B` is non-negative for a physical decay matrix (the
//! diagonal becomes `alpha - lambda_j >= 0`, the off-diagonals are already
//! `>= 0`), so every term of the series is non-negative and **nothing
//! cancels**. That is the whole point of the transformation: a naive Taylor
//! series for `exp(A t)` with `A` having large negative diagonal entries
//! suffers catastrophic cancellation, and this one does not. The truncation
//! point is chosen by [`UniformTaylor::max_num_terms`], which keeps the
//! discarded Poisson tail below `tol`.
//!
//! # The practical limit on one step
//!
//! `alpha = max lambda` is set by the **shortest-lived** tracked nuclide, and
//! `exp(-alpha t)` must not underflow `f64`. That caps a single call at
//! `alpha t < ~709`, i.e. about **1023 half-lives of the shortest-lived
//! nuclide in the chain**. For a chain containing Y-90 (`T_1/2 = 64.05 h`)
//! that is roughly 7.5 years in one step. Beyond it the solver returns
//! [`CyclusError::Value`] rather than silently returning zeros. Upstream's
//! `long double` reaches about 16384 half-lives instead; see
//! [`UniformTaylor::solve`]'s deviation notes, and
//! [`build_decay_matrix`]'s note on the "gross heuristic" upstream uses to
//! dodge the same problem incorrectly.
//!
//! # Units, everywhere
//!
//! | Quantity | Unit |
//! |---|---|
//! | decay constant `lambda` | per second (s^-1) |
//! | half-life | seconds |
//! | elapsed time `secs` / `t` | seconds |
//! | branching ratio | dimensionless, in `[0, 1]` |
//! | [`DecayChain`] matrix entries | per second (s^-1) |
//! | vectors passed to [`UniformTaylor::solve`] | **atom counts**, any consistent scale |
//! | [`Material`] quantity | kilograms |
//!
//! # This module ships NO nuclear data — and that is deliberate
//!
//! Upstream's `Decayer` carries a compiled-in decay-data table (via PyNE) and
//! reaches into it with `pyne::decay_const` and `pyne::decay_children`. This
//! crate does not, for two reasons:
//!
//!  1. The workspace rule is that **all nuclear data belongs in
//!     `njoy-outram-park-fork`**; a transport or fuel-cycle crate is data-free
//!     and pulls what it needs from there.
//!  2. Upstream marks `Decayer` DEPRECATED in favour of `pyne::decayers::decay`
//!     anyway, so its data path is not the one to preserve. The *algorithm* is.
//!
//! So the caller supplies a [`DecayChain`]: a map from parent nuclide to its
//! [`DecayData`]. A nuclide **absent** from the chain, or present with
//! `decay_constant == 0.0`, is **stable**.
//!
//! # Verification & validation
//!
//! **Methodology.** Three independent references, all closed-form; no
//! published benchmark is involved and none is claimed.
//!
//! 1. *Single nuclide, no daughters.* Reference `N(t) = N0 exp(-lambda t)`.
//!    This case is special: `alpha = lambda` makes `B` the zero matrix, so the
//!    series terminates after its first term and the answer is
//!    `exp(-lambda t) N0` **exactly**, up to [`petir::real::exp`]'s own error.
//!    Tolerance 1e-12 relative. Nuclide Sr-90, `lambda = 7.632e-10 s^-1`,
//!    `t = 1 year`.
//! 2. *Two-nuclide chain against the analytic Bateman solution.* Reference,
//!    derived in the test itself:
//!    `N2(t) = N1(0) lambda_1 / (lambda_2 - lambda_1) (exp(-lambda_1 t) - exp(-lambda_2 t))`.
//!    Chain Sr-90 -> Y-90 -> Zr-90 (stable). This is the test that catches a
//!    transposed decay matrix, and it does: with the matrix transposed the
//!    daughter is fed by nothing and comes out at **exactly zero** (measured
//!    0.000e0 of its true amount). `transposed_decay_matrix_fails_bateman`
//!    pins that, so the sign convention cannot be "fixed" the wrong way and
//!    still leave the suite green.
//! 3. *Diagonal matrix against elementwise `exp`.* Exercises
//!    [`UniformTaylor::solve`] on its own with `alpha t = 2`.
//!
//! **Results, measured 2026-09-16** on this crate's own test suite
//! (`cargo test -p outram-park-fork-cyclus --release --lib decay`), reported
//! as relative error against the closed form. Every row is a real number
//! printed by `decay::tests::vv_report`, which regenerates the whole table:
//!
//! | Case | at `tol = 1e-3` (the default) | at a tight `tol` |
//! |---|---|---|
//! | Sr-90 single nuclide, 1 a | 0.000e0 (bit-exact) | — |
//! | Sr-90 -> Y-90 Bateman **daughter**, 1 a | 9.431e-4 | 7.737e-11 (`tol` 1e-10) |
//! | Sr-90 -> Y-90 Bateman **parent**, 1 a | 9.431e-4 | 7.737e-11 (`tol` 1e-10) |
//! | diagonal `exp`, `alpha t = 2` | — | 1.128e-11 (`tol` 1e-10) |
//! | atom conservation, 3-nuclide chain, 1 a | 9.515e-4 | 7.446e-13 (`tol` 1e-12) |
//! | atom conservation, 3-nuclide chain, 5 a | 9.331e-4 | 9.731e-13 (`tol` 1e-12) |
//! | half-life: fraction left after one `T_1/2` | 0.000e0 (bit-exact) | — |
//! | 1 kg Po-210 -> Pb-206 mass ratio, 1 a | 6.216e-4 | 1.398e-13 (`tol` 1e-12) |
//! | isobaric chain mass, 5 a | 9.331e-4 | 9.688e-13 (`tol` 1e-12) |
//!
//! **Interpretation, and the one thing to take away.** The solver is correct
//! to round-off; **upstream's default tolerance is not tight enough for a
//! mass balance.** At `tol = 1e-3` the truncated Poisson tail discards up to
//! that fraction of the atom inventory — measured 9.5e-4, i.e. 0.095 %, over
//! one year of the Sr-90 chain — and the loss is **one-signed**, because every
//! discarded term is non-negative. So it accumulates over repeated steps
//! rather than cancelling. Pass a tighter `tol` through
//! [`decay_composition_with_tol`] or [`decay_material_with_tol`] for anything
//! that has to balance; the cost is mild, since the series length grows only
//! logarithmically in the tolerance (127 terms at 1e-3 against 172 at 1e-12,
//! for `alpha t = 94.9`). Upstream offers no way to ask for this at all.
//!
//! What the tight-tolerance column establishes is that **the matrix build and
//! the solver are correct**. It says nothing about whether any particular
//! decay data is right, because this module holds none.
//!
//! **Not covered.** No comparison against a published decay benchmark, no
//! comparison against upstream Cyclus compiled and run, no branching-ratio
//! chain with more than one branch verified against a reference. Nothing here
//! is validated in the workspace's sense.

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec;
use alloc::vec::Vec;

use petir::linalg::Matrix;
use petir::real::{exp, ln};

use crate::comp_math::{self, CompMap};
use crate::composition::{AtomicMasses, Composition};
use crate::error::{CyclusError, Result};
use crate::limits::{abs, EPS};
use crate::material::Material;
use crate::nuclide::Nuc;

/// The truncation tolerance upstream hard-codes in
/// `UniformTaylor::MatrixExpSolver`: `tol = 1e-3`, dimensionless.
///
/// It bounds the discarded Poisson tail of the uniformized series, so the
/// solution's error is at most `DEFAULT_TOL` times the initial total atom
/// count. Upstream offers no way to change it; here it is both this constant
/// and the `tol` parameter of [`UniformTaylor::solve`], so a caller who wants
/// a tighter answer can ask for one.
pub const DEFAULT_TOL: f64 = 1e-3;

/// Hard cap on the number of series terms, dimensionless.
///
/// **Not upstream.** Upstream's `MaxNumTerms` loop has no bound; in a
/// `no_std` library an unbounded loop that could stall on a pathological
/// input is worse than an error, so the cap turns that into
/// [`CyclusError::Value`]. It is set far above any reachable value: the
/// series length grows like `alpha t + O(sqrt(alpha t))`, and `alpha t` is
/// already bounded below ~709 by the `exp` range checks the port inherits,
/// so a real call needs fewer than a thousand terms.
pub const MAX_SERIES_TERMS: usize = 1_000_000;

// ---------------------------------------------------------------------------
// Decay data supplied by the caller
// ---------------------------------------------------------------------------

/// One parent nuclide's decay data: how fast it decays and into what.
///
/// # Units and valid ranges
///
/// - `decay_constant` is in **per second** (s^-1) and must be finite and
///   `>= 0`. Zero means stable.
/// - each branching ratio is **dimensionless** and must be finite and in
///   `[0, 1]`; the ratios for one parent must sum to at most `1 + tol`
///   (see [`DecayChain::validate`]).
///
/// # Branching ratios that sum to less than one
///
/// This is permitted and is *not* an error. It means atoms leave the tracked
/// system — either because the caller deliberately truncated the chain, or
/// because a minor branch's daughter was not worth tracking. The consequence
/// is that total atom count is **not** conserved, and that is the caller's
/// choice to make, not this module's to refuse.
#[derive(Debug, Clone, PartialEq)]
pub struct DecayData {
    /// Decay constant `lambda`, in per second (s^-1). Non-negative; `0.0`
    /// means stable.
    pub decay_constant: f64,

    /// `(daughter nuclide, branching ratio)` pairs. The branching ratio is
    /// the dimensionless fraction of decays of this parent that produce that
    /// daughter.
    ///
    /// A daughter may legitimately be absent from the chain itself, in which
    /// case it is a stable end point.
    pub branches: Vec<(Nuc, f64)>,
}

impl DecayData {
    /// Decay data with an explicit decay constant (per second) and branch
    /// list.
    #[must_use]
    pub fn new(decay_constant: f64, branches: Vec<(Nuc, f64)>) -> Self {
        Self {
            decay_constant,
            branches,
        }
    }

    /// A stable nuclide: `lambda = 0` per second, no daughters.
    ///
    /// Equivalent to leaving the nuclide out of the [`DecayChain`] entirely;
    /// both are honoured, because an explicit "this is stable" entry
    /// documents intent where an absence could be an oversight.
    #[must_use]
    pub fn stable() -> Self {
        Self {
            decay_constant: 0.0,
            branches: Vec::new(),
        }
    }

    /// A single decay path with branching ratio `1.0`.
    ///
    /// `decay_constant` is in per second (s^-1).
    #[must_use]
    pub fn single(decay_constant: f64, daughter: Nuc) -> Self {
        Self {
            decay_constant,
            branches: vec![(daughter, 1.0)],
        }
    }

    /// Decay data given a **half-life in seconds** rather than a decay
    /// constant: `lambda = ln(2) / T_half`.
    ///
    /// # Errors
    ///
    /// [`CyclusError::Value`] if `half_life_secs` is not finite and strictly
    /// positive. A stable nuclide has no finite half-life, so express it with
    /// [`DecayData::stable`] instead of passing infinity here.
    pub fn from_half_life(half_life_secs: f64, branches: Vec<(Nuc, f64)>) -> Result<Self> {
        if !half_life_secs.is_finite() || half_life_secs <= 0.0 {
            return Err(CyclusError::Value(
                "half-life must be finite and strictly positive, in seconds",
            ));
        }
        Ok(Self {
            decay_constant: ln(2.0) / half_life_secs,
            branches,
        })
    }

    /// `true` if this nuclide does not decay (`lambda == 0` per second).
    #[must_use]
    pub fn is_stable(&self) -> bool {
        self.decay_constant == 0.0
    }

    /// The half-life in **seconds**, `ln(2) / lambda`.
    ///
    /// Returns [`f64::INFINITY`] for a stable nuclide.
    #[must_use]
    pub fn half_life(&self) -> f64 {
        if self.decay_constant == 0.0 {
            f64::INFINITY
        } else {
            ln(2.0) / self.decay_constant
        }
    }

    /// The sum of this parent's branching ratios, dimensionless.
    ///
    /// Equals `1.0` for a chain that conserves atoms and less for one that
    /// deliberately drops minor branches.
    #[must_use]
    pub fn branch_sum(&self) -> f64 {
        let values: Vec<f64> = self.branches.iter().map(|&(_, br)| br).collect();
        crate::arithmetic::kahan_sum(&values)
    }
}

/// The caller-supplied decay chain: parent nuclide to its [`DecayData`].
///
/// # What "stable" means here
///
/// A nuclide is stable if it is **absent** from the chain, or present with
/// `decay_constant == 0.0`. There is no third state and no implicit data
/// lookup — this crate has no decay-data table (see the module docs).
///
/// # Ordering
///
/// Backed by a `BTreeMap`, so iteration is in nuclide-id order on every run
/// and every platform. That is load-bearing: it fixes the row/column order of
/// the decay matrix, which fixes the floating-point summation order, which is
/// what makes two runs of the same input bit-identical.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DecayChain {
    data: BTreeMap<Nuc, DecayData>,
}

impl DecayChain {
    /// An empty chain: every nuclide is stable.
    #[must_use]
    pub fn new() -> Self {
        Self {
            data: BTreeMap::new(),
        }
    }

    /// Inserts (or replaces) one parent's decay data.
    ///
    /// Returns the previous entry for `parent`, if any.
    pub fn insert(&mut self, parent: Nuc, data: DecayData) -> Option<DecayData> {
        self.data.insert(parent, data)
    }

    /// Builder form of [`insert`](DecayChain::insert), for assembling a chain
    /// in one expression.
    ///
    /// # Examples
    ///
    /// ```
    /// use outram_park_fork_cyclus::decay::{DecayChain, DecayData};
    /// use outram_park_fork_cyclus::nuclide::Nuc;
    ///
    /// let sr90 = Nuc::new(380900000);
    /// let y90 = Nuc::new(390900000);
    /// let chain = DecayChain::new()
    ///     .with(sr90, DecayData::single(7.6323e-10, y90))
    ///     .with(y90, DecayData::single(3.00614e-6, Nuc::new(400900000)));
    /// assert_eq!(chain.len(), 2);
    /// ```
    #[must_use]
    pub fn with(mut self, parent: Nuc, data: DecayData) -> Self {
        self.data.insert(parent, data);
        self
    }

    /// The decay data for `nuc`, or `None` if it is not a tracked parent
    /// (i.e. it is stable).
    #[must_use]
    pub fn get(&self, nuc: Nuc) -> Option<&DecayData> {
        self.data.get(&nuc)
    }

    /// The number of parent nuclides in the chain.
    #[must_use]
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// `true` if the chain has no entries, so every nuclide is stable.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Every parent nuclide in the chain, in nuclide-id order.
    #[must_use]
    pub fn parents(&self) -> Vec<Nuc> {
        self.data.keys().copied().collect()
    }

    /// The decay constant of `nuc`, in **per second** (s^-1).
    ///
    /// Returns `0.0` for a nuclide absent from the chain, because absence
    /// means stable.
    #[must_use]
    pub fn decay_constant(&self, nuc: Nuc) -> f64 {
        self.data.get(&nuc).map_or(0.0, |d| d.decay_constant)
    }

    /// `true` if `nuc` does not decay: absent from the chain, or present with
    /// a zero decay constant.
    #[must_use]
    pub fn is_stable(&self, nuc: Nuc) -> bool {
        self.data.get(&nuc).is_none_or(DecayData::is_stable)
    }

    /// Checks the chain is physically well formed, with the default tolerance
    /// [`EPS`] (1e-6, dimensionless) on the branching-ratio sum.
    ///
    /// # Errors
    ///
    /// See [`DecayChain::validate_with_tol`].
    pub fn validate(&self) -> Result<()> {
        self.validate_with_tol(EPS)
    }

    /// Checks the chain is physically well formed.
    ///
    /// The checks, in order, for every parent:
    ///
    /// 1. the decay constant is finite and `>= 0` (per second);
    /// 2. every key and every daughter is a well-formed nuclide id;
    /// 3. no parent lists itself as its own daughter;
    /// 4. no daughter appears twice for one parent;
    /// 5. every branching ratio is finite and in `[0, 1]`;
    /// 6. the branching ratios for one parent sum to at most `1 + tol`.
    ///
    /// Check 6 is one-sided by design: a sum **below** one is legitimate (see
    /// [`DecayData`]), a sum above one would create atoms.
    ///
    /// Checks 3 and 4 have no upstream counterpart — upstream's daughters come
    /// from a `std::set` built by PyNE, so neither can occur there. They can
    /// occur in a hand-written chain, and both silently corrupt the decay
    /// matrix rather than failing loudly, so they are rejected here.
    ///
    /// # Errors
    ///
    /// [`CyclusError::Value`] for a violated numerical check,
    /// [`CyclusError::InvalidNuclide`] for a malformed nuclide id.
    pub fn validate_with_tol(&self, tol: f64) -> Result<()> {
        if !tol.is_finite() || tol < 0.0 {
            return Err(CyclusError::Value(
                "branching-ratio tolerance must be finite and non-negative",
            ));
        }
        for (&parent, data) in &self.data {
            if !parent.is_nuclide() {
                return Err(CyclusError::InvalidNuclide(parent.raw()));
            }
            if !data.decay_constant.is_finite() {
                return Err(CyclusError::Value(
                    "decay constant must be finite, in per second",
                ));
            }
            if data.decay_constant < 0.0 {
                return Err(CyclusError::Value(
                    "decay constant must be non-negative, in per second",
                ));
            }
            let mut seen: BTreeSet<Nuc> = BTreeSet::new();
            for &(daughter, br) in &data.branches {
                if !daughter.is_nuclide() {
                    return Err(CyclusError::InvalidNuclide(daughter.raw()));
                }
                if daughter == parent {
                    return Err(CyclusError::Value(
                        "a nuclide cannot be its own decay daughter",
                    ));
                }
                if !seen.insert(daughter) {
                    return Err(CyclusError::Value(
                        "a daughter appears more than once for one parent",
                    ));
                }
                if !br.is_finite() || !(0.0..=1.0).contains(&br) {
                    return Err(CyclusError::Value(
                        "branching ratio must be finite and within [0, 1]",
                    ));
                }
            }
            if data.branch_sum() > 1.0 + tol {
                return Err(CyclusError::Value(
                    "branching ratios for one parent sum to more than 1",
                ));
            }
        }
        Ok(())
    }

    /// Every nuclide reachable from `seeds` by following decay branches, plus
    /// the seeds themselves, in nuclide-id order.
    ///
    /// This is the translation of upstream's recursive `Decayer::AddNucToMaps`,
    /// which walks `pyne::decay_children` transitively so that the decay
    /// matrix covers the whole reachable chain and no atoms silently leave the
    /// tracked set. Here the walk is iterative rather than recursive, so a
    /// long chain cannot overflow the stack.
    ///
    /// A cycle in the chain (which is unphysical, but a caller can write one)
    /// terminates the walk rather than hanging, because each nuclide is
    /// visited once.
    #[must_use]
    pub fn reachable_closure(&self, seeds: &[Nuc]) -> Vec<Nuc> {
        let mut seen: BTreeSet<Nuc> = BTreeSet::new();
        let mut stack: Vec<Nuc> = Vec::new();
        for &s in seeds {
            if seen.insert(s) {
                stack.push(s);
            }
        }
        while let Some(parent) = stack.pop() {
            if let Some(data) = self.data.get(&parent) {
                for &(daughter, _) in &data.branches {
                    if seen.insert(daughter) {
                        stack.push(daughter);
                    }
                }
            }
        }
        seen.into_iter().collect()
    }
}

// ---------------------------------------------------------------------------
// The decay matrix
// ---------------------------------------------------------------------------

/// Builds the decay matrix `A`, in **per second** (s^-1), over `nuclides`.
///
/// This is the translation of upstream's `Decayer::BuildDecayMatrix`. Row and
/// column `k` both correspond to `nuclides[k]`, so the caller controls the
/// ordering and can read the matrix back against its own list. The
/// convention, restated because a transposed matrix is the likeliest defect
/// in this file:
///
/// ```text
/// A[j][j] = -lambda_j
/// A[i][j] = branch_ratio(j -> i) * lambda_j
/// ```
///
/// i.e. **parent by column, daughter by row**, so that `dn/dt = A n`.
///
/// # Daughters outside `nuclides`
///
/// A daughter not present in `nuclides` is **dropped**: its atoms leave the
/// tracked system and the corresponding column sum becomes negative. That is
/// deliberate — it is what makes this function usable for a truncated
/// nuclide set — but it is silent, so prefer
/// [`DecayChain::reachable_closure`] to build the list.
/// [`decay_composition`] does exactly that and therefore never hits this case.
///
/// # Deviation from upstream: the "mostly stable" heuristic is NOT ported
///
/// `BuildDecayMatrix` contains this, commented *"Gross heuristic for mostly
/// stable nuclides 2903040000 sec / 100 years"*:
///
/// ```text
/// if (static_cast<long double>(exp(-2903040000 * decay_const)) == 0.0)
///   decay_const = 0.0;
/// ```
///
/// Reading it was the surprise of this port, and it is recorded here because
/// it does the **opposite** of what its comment says. The test fires when
/// `exp(-2.9e9 * lambda)` underflows, i.e. when `lambda` is **large**
/// (`> ~2.6e-7` per second in `double`, a half-life under about 31 days) —
/// not when the nuclide is "mostly stable". Its effect is therefore to freeze
/// every short-lived nuclide as if it were stable, which is a large,
/// silent physics error rather than a rounding convenience. The plausible
/// intent is numerical: a large `lambda` makes `alpha t` huge, which makes
/// `exp(-alpha t)` underflow and the solver refuse. That is a real problem,
/// but zeroing the decay constant is not a correct answer to it, so it is not
/// ported. The port instead reports the underflow as
/// [`CyclusError::Value`], and the caller can either shorten the step or
/// exclude the nuclide knowingly.
///
/// # Errors
///
/// - [`CyclusError::Value`] if `nuclides` is empty, contains a duplicate, or
///   if a decay constant or branching ratio reached through the chain is
///   non-finite or negative, or if a parent lists itself as a daughter.
/// - [`CyclusError::Numeric`] if the matrix allocation fails.
///
/// # Examples
///
/// ```
/// use outram_park_fork_cyclus::decay::{build_decay_matrix, DecayChain, DecayData};
/// use outram_park_fork_cyclus::nuclide::Nuc;
///
/// let a_nuc = Nuc::new(380900000); // Sr-90
/// let b_nuc = Nuc::new(390900000); // Y-90
/// let chain = DecayChain::new().with(a_nuc, DecayData::single(2.0, b_nuc));
/// let m = build_decay_matrix(&chain, &[a_nuc, b_nuc])?;
///
/// assert_eq!(m.get(0, 0), -2.0); // parent decays away
/// assert_eq!(m.get(1, 0), 2.0);  // ... into the daughter: row 1, column 0
/// assert_eq!(m.get(0, 1), 0.0);
/// assert_eq!(m.get(1, 1), 0.0);  // daughter is stable
/// # Ok::<(), outram_park_fork_cyclus::error::CyclusError>(())
/// ```
pub fn build_decay_matrix(chain: &DecayChain, nuclides: &[Nuc]) -> Result<Matrix> {
    if nuclides.is_empty() {
        return Err(CyclusError::Value(
            "a decay matrix needs at least one nuclide",
        ));
    }

    let mut index: BTreeMap<Nuc, usize> = BTreeMap::new();
    for (k, &nuc) in nuclides.iter().enumerate() {
        if index.insert(nuc, k).is_some() {
            return Err(CyclusError::Value(
                "duplicate nuclide in the decay matrix nuclide list",
            ));
        }
    }

    let n = nuclides.len();
    let mut a = Matrix::zeros(n, n)?;

    for (j, &parent) in nuclides.iter().enumerate() {
        let Some(data) = chain.get(parent) else {
            continue; // absent from the chain: stable, column stays zero
        };
        let lambda = data.decay_constant;
        if !lambda.is_finite() || lambda < 0.0 {
            return Err(CyclusError::Value(
                "decay constant must be finite and non-negative, in per second",
            ));
        }
        a.set(j, j, -lambda);

        for &(daughter, br) in &data.branches {
            if !br.is_finite() || br < 0.0 {
                return Err(CyclusError::Value(
                    "branching ratio must be finite and non-negative",
                ));
            }
            if daughter == parent {
                return Err(CyclusError::Value(
                    "a nuclide cannot be its own decay daughter",
                ));
            }
            if let Some(&i) = index.get(&daughter) {
                // Accumulate rather than assign. Upstream assigns, which would
                // lose one of two entries for the same daughter; that cannot
                // happen upstream because its daughters come from a set, and
                // `DecayChain::validate` rejects it here, but a caller may skip
                // validation and summing is the physically right answer.
                a.set(i, j, a.get(i, j) + br * lambda);
            }
        }
    }

    Ok(a)
}

// ---------------------------------------------------------------------------
// The matrix exponential solver
// ---------------------------------------------------------------------------

/// The Taylor-series-with-uniformization matrix exponential solver.
///
/// A direct port of upstream's `UniformTaylor` class. It solves
///
/// ```text
/// dx/dt = A x,   x(t) = exp(A t) x(0)
/// ```
///
/// for a square `A` whose diagonal is non-positive and whose off-diagonal
/// entries are non-negative — that is, a decay matrix. It is exposed as a
/// public type rather than kept private to [`decay_composition`] because it is
/// independently useful (any linear first-order system with those signs) and,
/// more to the point, independently **testable**.
///
/// A unit-struct namespace rather than a value: it has no state, exactly as
/// upstream's all-static class has none.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct UniformTaylor;

impl UniformTaylor {
    /// Solves `x(t) = exp(A t) x0`.
    ///
    /// Upstream `UniformTaylor::MatrixExpSolver`, with `tol` lifted from a
    /// hard-coded `1e-3` into a parameter. Pass [`DEFAULT_TOL`] for upstream's
    /// exact behaviour.
    ///
    /// # Parameters
    ///
    /// - `a` — the square system matrix, in per second (s^-1) for a decay
    ///   problem.
    /// - `x0` — the initial state, in **atom counts** (any consistent scale;
    ///   the solver is linear so the unit cancels). Length must equal
    ///   `a.rows()`.
    /// - `t` — elapsed time in **seconds**, finite and `>= 0`.
    /// - `tol` — dimensionless truncation tolerance, strictly within
    ///   `(0, 1)`. Bounds the discarded Poisson tail, so it bounds the error
    ///   as a fraction of `sum(x0)`.
    ///
    /// # Deviations from upstream
    ///
    /// 1. **`f64` throughout, where upstream uses `long double`** for the two
    ///    exponential range checks. On x86 that is an 80-bit type with an
    ///    exponent range to about `1e-4950`, so upstream tolerates
    ///    `alpha * t` up to roughly 11356 where this port refuses above about
    ///    709. There is no portable 80-bit type in Rust, and PETIR is `f64`.
    ///    In practice the restriction bites only for a step so long that every
    ///    short-lived nuclide has decayed away many times over, which is a
    ///    problem the caller should see rather than have silently smoothed.
    /// 2. **Negative `t` is rejected.** Upstream does not check, and returns
    ///    quiet nonsense for it: the sign of `alpha * t` flips, the series
    ///    truncates after a single term, and the result is
    ///    `exp(alpha |t|) x0` — exponential *growth*, with no warning.
    /// 3. **`tol` outside `(0, 1)` is rejected.** With `tol <= 0` upstream's
    ///    termination test `sumTerms >= exp(alpha_t)` can never be satisfied
    ///    in floating point and the loop does not terminate.
    /// 4. **A non-square `a` is rejected.** Upstream reads `A.NumRows()` and
    ///    indexes the diagonal without checking.
    ///
    /// # Errors
    ///
    /// - [`CyclusError::Value`] for a non-square `a`, a length mismatch
    ///   between `x0` and `a`, a negative or non-finite `t`, a `tol` outside
    ///   `(0, 1)`, an `exp(-alpha t)` that underflows to zero, an
    ///   `exp(alpha t)` that overflows, or a series longer than
    ///   [`MAX_SERIES_TERMS`].
    /// - [`CyclusError::Numeric`] from the underlying PETIR matrix-vector
    ///   product.
    ///
    /// # Examples
    ///
    /// ```
    /// use outram_park_fork_cyclus::decay::{UniformTaylor, DEFAULT_TOL};
    /// use petir::linalg::Matrix;
    ///
    /// // A single nuclide with lambda = 0.1 per second, after 10 seconds.
    /// let a = Matrix::from_row_major(1, 1, vec![-0.1])?;
    /// let x = UniformTaylor::solve(&a, &[100.0], 10.0, DEFAULT_TOL)?;
    /// assert!((x[0] - 100.0 * petir::real::exp(-1.0)).abs() < 1e-10);
    /// # Ok::<(), outram_park_fork_cyclus::error::CyclusError>(())
    /// ```
    pub fn solve(a: &Matrix, x0: &[f64], t: f64, tol: f64) -> Result<Vec<f64>> {
        let n = a.rows();
        if a.cols() != n {
            return Err(CyclusError::Value(
                "the matrix exponential needs a square matrix",
            ));
        }
        if x0.len() != n {
            return Err(CyclusError::Value(
                "matrix-vector dimensions are not compatible",
            ));
        }
        if !t.is_finite() || t < 0.0 {
            return Err(CyclusError::Value(
                "elapsed time must be finite and non-negative, in seconds",
            ));
        }
        if !(tol > 0.0 && tol < 1.0) {
            return Err(CyclusError::Value(
                "truncation tolerance must lie strictly between 0 and 1",
            ));
        }

        // Step 1: alpha, the largest absolute diagonal element.
        let alpha = Self::max_abs_diag(a);

        // Step 2: B = A + alpha * I. Non-negative in every entry for a
        // physical decay matrix, which is what makes the series cancellation
        // free.
        let mut b = a.clone();
        for i in 0..n {
            b.set(i, i, b.get(i, i) + alpha);
        }

        // Steps 3-7.
        Self::solution_vector(&b, x0, alpha, t, tol)
    }

    /// Solves with upstream's hard-coded tolerance, [`DEFAULT_TOL`].
    ///
    /// Exactly upstream's `MatrixExpSolver(A, x_o, t)` signature and
    /// behaviour. Units as [`UniformTaylor::solve`].
    ///
    /// # Errors
    ///
    /// As [`UniformTaylor::solve`].
    pub fn solve_default_tol(a: &Matrix, x0: &[f64], t: f64) -> Result<Vec<f64>> {
        Self::solve(a, x0, t, DEFAULT_TOL)
    }

    /// The diagonal element of `a` with the largest absolute value.
    ///
    /// Upstream `UniformTaylor::MaxAbsDiag`. For a decay matrix this is the
    /// largest decay constant present, in per second (s^-1), and it is the
    /// uniformization rate: the series length grows with `alpha * t`, so one
    /// very short-lived nuclide sets the cost of the whole solve.
    ///
    /// Returns `0.0` for a zero-dimension matrix, where upstream would read
    /// `A(1, 1)` out of bounds.
    #[must_use]
    pub fn max_abs_diag(a: &Matrix) -> f64 {
        let n = core::cmp::min(a.rows(), a.cols());
        let mut max = 0.0_f64;
        for i in 0..n {
            let v = abs(a.get(i, i));
            if v > max {
                max = v;
            }
        }
        max
    }

    /// The number of Taylor terms needed for a truncation error of at most
    /// `epsilon`.
    ///
    /// Upstream `UniformTaylor::MaxNumTerms`. Both arguments are
    /// dimensionless: `alpha_t` is the product of the uniformization rate
    /// (per second) and the elapsed time (seconds), `epsilon` is a fraction.
    ///
    /// # Method
    ///
    /// The uniformized series weights are the Poisson probabilities
    /// `exp(-alpha t) (alpha t)^k / k!`, which sum to one over all `k`. This
    /// accumulates the unnormalised partial sums of `exp(alpha t)` until they
    /// reach `(1 - epsilon) exp(alpha t)`, i.e. until the discarded tail
    /// carries less than `epsilon` of the total probability. Because the
    /// matrix `B / alpha` is substochastic for a physical decay chain, that
    /// tail probability bounds the solution error in the same proportion.
    ///
    /// # Errors
    ///
    /// [`CyclusError::Value`] if `exp(alpha_t)` overflows `f64` (upstream
    /// tests the same condition against `HUGE_VAL` in `long double`), or if
    /// the series exceeds [`MAX_SERIES_TERMS`].
    pub fn max_num_terms(alpha_t: f64, epsilon: f64) -> Result<usize> {
        let lower_bound = exp(alpha_t);
        if !lower_bound.is_finite() {
            return Err(CyclusError::Value(
                "exp(alpha * t) overflows f64; the uniform Taylor method cannot solve this matrix exponential",
            ));
        }
        let lower_bound = lower_bound * (1.0 - epsilon);

        let mut prev_term = 1.0_f64;
        let mut sum_terms = 1.0_f64;
        let mut p: usize = 1;

        while sum_terms < lower_bound {
            let next_term = (alpha_t / p as f64) * prev_term;
            sum_terms += next_term;
            p += 1;
            prev_term = next_term;
            if p > MAX_SERIES_TERMS {
                return Err(CyclusError::Value(
                    "the uniform Taylor series did not converge within MAX_SERIES_TERMS",
                ));
            }
        }

        Ok(p)
    }

    /// Steps 3-7 of upstream's algorithm: the truncated uniformized series.
    ///
    /// `b` is `A + alpha I`; `alpha` is [`max_abs_diag`](UniformTaylor::max_abs_diag)
    /// of the original `A`, in per second; `t` is in seconds.
    ///
    /// The `exp(-alpha t)` factor is folded into the **first** term rather
    /// than applied at the end. That is upstream's own choice and its comment
    /// explains why: the running sum then grows from a small number instead of
    /// a very large one being multiplied by a very small one at the end, which
    /// keeps the intermediate magnitudes representable.
    fn solution_vector(b: &Matrix, x0: &[f64], alpha: f64, t: f64, tol: f64) -> Result<Vec<f64>> {
        // Step 3.
        let alpha_t = alpha * t;
        let expat = exp(-alpha_t);
        if expat == 0.0 {
            return Err(CyclusError::Value(
                "exp(-alpha * t) underflows f64; the uniform Taylor method cannot solve this matrix exponential",
            ));
        }

        // Step 4.
        let mut c_prev: Vec<f64> = x0.iter().map(|&v| expat * v).collect();
        let mut ck_sum: Vec<f64> = c_prev.clone();

        // Step 5.
        let max_terms = Self::max_num_terms(alpha_t, tol)?;

        // Step 6: C_k = (t / k) * B * C_{k-1}.
        for k in 1..max_terms {
            let product = b.mul_vec(&c_prev)?;
            let scale = t / k as f64;
            let c_next: Vec<f64> = product.iter().map(|&v| scale * v).collect();
            for (s, &c) in ck_sum.iter_mut().zip(c_next.iter()) {
                *s += c;
            }
            c_prev = c_next;
        }

        // Step 7.
        Ok(ck_sum)
    }
}

// ---------------------------------------------------------------------------
// Decaying compositions and materials
// ---------------------------------------------------------------------------

/// Decays a composition for `secs` seconds, in the **atom basis**.
///
/// This is the translation of upstream's `Decayer` pipeline — build the
/// tracked-nuclide set, build the decay matrix, call the matrix exponential,
/// read the result back — with the decay data supplied by the caller rather
/// than compiled in.
///
/// # Why the atom basis
///
/// **Decay conserves atoms, not mass.** A decay event turns one atom into one
/// atom of the daughter (plus an emitted particle), so the natural state
/// variable is the atom count and the matrix above is written for it. Mass is
/// *not* conserved, for a reason that has nothing to do with the mass defect:
/// an alpha decay carries four nucleons out of the tracked heavy nuclide, and
/// unless the caller tracks He-4 as a daughter those four nucleons leave the
/// composition. Working in the mass basis would therefore need a
/// mass-per-decay bookkeeping term that the Bateman equations do not have.
/// So the solve is in atoms and the mass basis is reconstructed afterwards by
/// [`Composition::from_atom`], which is where `masses` is used.
///
/// # Scale
///
/// [`Composition`] is a set of ratios, not absolute amounts, and this function
/// preserves whatever scale the input carried: pass atom *fractions* and you
/// get the decayed atom fractions scaled by the same factor the absolute atom
/// count changed by. If you want absolute amounts, scale the input yourself —
/// or use [`decay_material`], which does exactly that against the material's
/// mass in kilograms.
///
/// # The nuclide set
///
/// The matrix is built over the union of the composition's own nuclides and
/// every nuclide reachable from them through `chain`
/// ([`DecayChain::reachable_closure`]), so no atom silently leaves the system
/// because its daughter was not listed. That is upstream's recursive
/// `AddNucToMaps` behaviour.
///
/// # Deviation: no static accumulating state
///
/// Upstream's `Decayer` keeps `parent_`, `daughters_`, `decay_matrix_` and
/// `nuclides_tracked_` as **class statics**, so the tracked set grows for the
/// life of the process and the matrix is rebuilt only when a new nuclide is
/// seen. Every later decay then solves a system as large as every nuclide
/// ever seen. This port builds the matrix per call from the nuclides that are
/// actually present. The result is the same numbers (the extra rows carry
/// zero) with no cross-call coupling and no global mutable state, which
/// `no_std` could not express anyway.
///
/// # Zero-valued results are dropped
///
/// Upstream's `GetResult` copies a nuclide into the output only when its atom
/// count is `> 0`, and this does the same. The uniformized series produces
/// only non-negative terms, so an entry is zero exactly when nothing fed it.
///
/// # Errors
///
/// - [`CyclusError::Value`] for a negative or non-finite `secs`, or any of
///   the solver's failure modes.
/// - [`CyclusError::InvalidNuclide`] / [`CyclusError::Key`] if `masses`
///   cannot supply an atomic mass for a daughter that the decay produced.
///   Note this can fail for a composition that was fine going in, because
///   decay introduces nuclides the caller never mentioned.
///
/// # Examples
///
/// ```
/// use outram_park_fork_cyclus::comp_math::CompMap;
/// use outram_park_fork_cyclus::composition::{AtomicMasses, Composition};
/// use outram_park_fork_cyclus::decay::{
///     decay_composition, decay_composition_with_tol, DecayChain, DecayData,
/// };
/// use outram_park_fork_cyclus::nuclide::Nuc;
///
/// let po210 = Nuc::new(842100000);
/// let pb206 = Nuc::new(822060000);
/// // Po-210 alpha decays to Pb-206 with a 138.376 day half-life.
/// let chain = DecayChain::new().with(
///     po210,
///     DecayData::from_half_life(138.376 * 86400.0, vec![(pb206, 1.0)])?,
/// );
///
/// let mut v = CompMap::new();
/// v.insert(po210, 1.0);
/// let comp = Composition::from_atom(v, &AtomicMasses::MassNumber)?;
///
/// let year = 365.25 * 86400.0;
/// let after = decay_composition(&comp, &chain, year, &AtomicMasses::MassNumber)?;
///
/// // Atom count is conserved -- but only to the solver's tolerance, which
/// // defaults to upstream's 1e-3 and always errs by LOSING atoms.
/// let total: f64 = after.atom().values().sum();
/// assert!((total - 1.0).abs() < 1e-3);
///
/// // Ask for a tighter answer when the books have to balance.
/// let tight = decay_composition_with_tol(&comp, &chain, year, &AtomicMasses::MassNumber, 1e-12)?;
/// let total: f64 = tight.atom().values().sum();
/// assert!((total - 1.0).abs() < 1e-9);
/// # Ok::<(), outram_park_fork_cyclus::error::CyclusError>(())
/// ```
pub fn decay_composition(
    comp: &Composition,
    chain: &DecayChain,
    secs: f64,
    masses: &AtomicMasses,
) -> Result<Composition> {
    decay_composition_with_tol(comp, chain, secs, masses, DEFAULT_TOL)
}

/// [`decay_composition`] with an explicit solver tolerance.
///
/// `tol` is dimensionless and must lie strictly within `(0, 1)`; see
/// [`UniformTaylor::solve`]. [`decay_composition`] is this function with
/// [`DEFAULT_TOL`], which is upstream's hard-coded value.
///
/// Not an upstream entry point — upstream offers no way to ask for a tighter
/// answer. It is here because the tolerance is the one knob that trades cost
/// against accuracy, and burying it would make the V&V cases in this file
/// impossible to write.
///
/// # Errors
///
/// As [`decay_composition`], plus [`CyclusError::Value`] for a `tol` outside
/// `(0, 1)`.
pub fn decay_composition_with_tol(
    comp: &Composition,
    chain: &DecayChain,
    secs: f64,
    masses: &AtomicMasses,
    tol: f64,
) -> Result<Composition> {
    if !secs.is_finite() || secs < 0.0 {
        return Err(CyclusError::Value(
            "decay time must be finite and non-negative, in seconds",
        ));
    }

    let seeds: Vec<Nuc> = comp.atom().keys().copied().collect();
    if seeds.is_empty() {
        return Composition::from_atom(CompMap::new(), masses);
    }

    let nuclides = chain.reachable_closure(&seeds);
    let a = build_decay_matrix(chain, &nuclides)?;
    let x0: Vec<f64> = nuclides
        .iter()
        .map(|nuc| comp.atom().get(nuc).copied().unwrap_or(0.0))
        .collect();

    let x = UniformTaylor::solve(&a, &x0, secs, tol)?;

    let mut out = CompMap::new();
    for (k, &nuc) in nuclides.iter().enumerate() {
        if x[k] > 0.0 {
            out.insert(nuc, x[k]);
        }
    }

    Composition::from_atom(out, masses)
}

/// Decays a material in place for `secs` seconds, recording `now` as its new
/// decay time.
///
/// # What is conserved, and what is not — read this
///
/// **Atom count is conserved through the solve; total mass is not, and must
/// not be.** The solve runs on absolute atom amounts obtained by dividing each
/// nuclide's mass in kilograms by its atomic mass in u, so — for a chain whose
/// branching ratios sum to one and whose end points are tracked — the total
/// number of atoms coming out equals the number going in. The material's
/// quantity in kilograms is then **rebuilt** from the decayed nuclide vector:
///
/// ```text
/// new quantity [kg] = sum over nuclides of (atom amount * atomic mass)
/// ```
///
/// That total differs from the old one, and the difference is physical, not a
/// bookkeeping error. An alpha decay removes four nucleons from the tracked
/// heavy nuclide; a beta decay barely changes the mass number but does change
/// which mass is used. Holding the kilograms fixed instead would mean
/// inventing atoms to make the books balance — for Po-210 decaying to Pb-206
/// over a year that would be a **1.6 % error** in atom count (measured: the
/// mass falls to 9.8400912e-1 of its initial value while the atom count is
/// unchanged), silently, in the direction of over-counting the inventory. So: **atoms through the solve,
/// mass rebuilt afterwards.**
///
/// The mass defect itself (the binding-energy difference, of order 1e-3 of the
/// mass number) is ignored, exactly as upstream ignores it: it lives entirely
/// in the atomic masses `masses` supplies, and with
/// [`AtomicMasses::MassNumber`] it is not represented at all.
///
/// # Parameters
///
/// - `mat` — the material, whose quantity is in kilograms.
/// - `chain` — the caller-supplied decay data; constants in per second.
/// - `secs` — elapsed time in **seconds**, finite and `>= 0`.
/// - `now` — the simulation time to record as the material's new decay time.
///   Units are the simulation's own (upstream: integer time steps); this
///   function only stores it.
/// - `masses` — atomic masses in u, needed for both the mass-to-atom
///   conversion going in and the atom-to-mass conversion coming out.
///
/// # An empty material is a no-op
///
/// A material with zero quantity, or whose composition has zero total mass,
/// has its decay time updated and nothing else. There is nothing to decay and
/// the atom conversion would divide by zero.
///
/// # Errors
///
/// As [`decay_composition`]. On any error the material is left **unchanged**
/// except that its decay time is not advanced, because the new value is
/// computed in full before anything is written back.
pub fn decay_material(
    mat: &mut Material,
    chain: &DecayChain,
    secs: f64,
    now: i64,
    masses: &AtomicMasses,
) -> Result<()> {
    decay_material_with_tol(mat, chain, secs, now, masses, DEFAULT_TOL)
}

/// [`decay_material`] with an explicit solver tolerance.
///
/// `tol` is dimensionless and must lie strictly within `(0, 1)`; see
/// [`UniformTaylor::solve`]. [`decay_material`] is this function with
/// [`DEFAULT_TOL`], upstream's hard-coded value.
///
/// **Pass a tighter `tol` than the default for anything that has to balance.**
/// At `tol = 1e-3` the truncated series discards up to that fraction of the
/// atom inventory — measured at 9.331e-4 over a five-year decay of the
/// Sr-90 -> Y-90 -> Zr-90 chain, which showed up directly as a 0.093 % loss of
/// mass in a chain that conserves mass exactly. The loss is always in the
/// same direction (the discarded terms are all non-negative, so the solver
/// under-counts and never creates atoms), so it accumulates rather than
/// cancelling over repeated steps.
///
/// # Errors
///
/// As [`decay_material`], plus [`CyclusError::Value`] for a `tol` outside
/// `(0, 1)`.
pub fn decay_material_with_tol(
    mat: &mut Material,
    chain: &DecayChain,
    secs: f64,
    now: i64,
    masses: &AtomicMasses,
    tol: f64,
) -> Result<()> {
    if !secs.is_finite() || secs < 0.0 {
        return Err(CyclusError::Value(
            "decay time must be finite and non-negative, in seconds",
        ));
    }

    let mass_sum = comp_math::sum(mat.comp().mass());
    if mat.quantity() <= 0.0 || mass_sum <= 0.0 || mat.comp().is_empty() {
        mat.set_prev_decay_time(now);
        return Ok(());
    }

    // Absolute atom amounts, in kg/u: the composition's atom ratios scaled so
    // that the mass basis totals the material's quantity in kilograms. The
    // unit is proportional to moles; the constant of proportionality is
    // irrelevant because decay is linear and the mass is rebuilt from the same
    // atomic masses at the end.
    let scale = mat.quantity() / mass_sum;
    let mut atoms = mat.comp().atom().clone();
    for amount in atoms.values_mut() {
        *amount *= scale;
    }
    let absolute = Composition::from_atom(atoms, masses)?;

    let decayed = decay_composition_with_tol(&absolute, chain, secs, masses, tol)?;
    let new_qty = comp_math::sum(decayed.mass());

    // `Material` exposes no quantity setter — upstream changes the quantity
    // only through extract/absorb — so the decayed material is rebuilt. Every
    // field is carried across: quantity, composition, unit value, and the
    // decay time set immediately below.
    let unit_value = mat.unit_value();
    *mat = Material::with_unit_value(new_qty, decayed, unit_value)?;
    mat.set_prev_decay_time(now);
    Ok(())
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use alloc::vec;

    // --- Nuclides used below. ids are `zzzaaammmm`. ---
    const SR90: Nuc = Nuc::new(380900000);
    const Y90: Nuc = Nuc::new(390900000);
    const ZR90: Nuc = Nuc::new(400900000);
    const PO210: Nuc = Nuc::new(842100000);
    const PB206: Nuc = Nuc::new(822060000);

    // --- Decay constants, per second, from published half-lives. ---
    // Sr-90: T_1/2 = 28.79 a  (a = 365.25 d = 3.15576e7 s)
    const YEAR_SECS: f64 = 365.25 * 86400.0;
    fn lambda_sr90() -> f64 {
        ln(2.0) / (28.79 * YEAR_SECS)
    }
    // Y-90: T_1/2 = 64.05 h
    fn lambda_y90() -> f64 {
        ln(2.0) / (64.05 * 3600.0)
    }
    // Po-210: T_1/2 = 138.376 d
    fn lambda_po210() -> f64 {
        ln(2.0) / (138.376 * 86400.0)
    }

    fn rel_err(got: f64, want: f64) -> f64 {
        if want == 0.0 {
            abs(got)
        } else {
            abs(got - want) / abs(want)
        }
    }

    fn sr_y_zr_chain() -> DecayChain {
        DecayChain::new()
            .with(SR90, DecayData::single(lambda_sr90(), Y90))
            .with(Y90, DecayData::single(lambda_y90(), ZR90))
            .with(ZR90, DecayData::stable())
    }

    fn atom_comp(pairs: &[(Nuc, f64)]) -> Composition {
        let mut v = CompMap::new();
        for &(n, q) in pairs {
            v.insert(n, q);
        }
        Composition::from_atom(v, &AtomicMasses::MassNumber).unwrap()
    }

    // -----------------------------------------------------------------------
    // V&V case 1: single nuclide against N0 exp(-lambda t)
    // -----------------------------------------------------------------------

    /// Methodology: one nuclide (Sr-90, lambda = ln2 / 28.79 a), no daughters
    /// tracked, decayed for one year. Reference: `N(t) = N0 exp(-lambda t)`.
    ///
    /// This case is exact by construction, not merely accurate: with a single
    /// nuclide `alpha = lambda`, so `B = A + alpha I` is the zero matrix, every
    /// series term after the first vanishes, and the answer is the `exp`
    /// factor itself. Pass criterion 1e-12 relative; measured 0.000e0.
    #[test]
    fn single_nuclide_matches_exponential_decay() {
        let lambda = lambda_sr90();
        let chain = DecayChain::new().with(SR90, DecayData::new(lambda, Vec::new()));
        let n0 = 3.5;
        let comp = atom_comp(&[(SR90, n0)]);

        let after =
            decay_composition(&comp, &chain, YEAR_SECS, &AtomicMasses::MassNumber).unwrap();

        let want = n0 * exp(-lambda * YEAR_SECS);
        let got = after.atom()[&SR90];
        assert!(
            rel_err(got, want) < 1e-12,
            "single-nuclide decay: got {got}, want {want}, rel err {}",
            rel_err(got, want)
        );
    }

    // -----------------------------------------------------------------------
    // V&V case 2: two-nuclide chain against the closed-form Bateman solution
    // -----------------------------------------------------------------------

    /// Methodology: Sr-90 -> Y-90 -> Zr-90 (stable), branching 1.0 throughout,
    /// decayed for one year from pure Sr-90.
    ///
    /// Reference, the two-species Bateman solution. With
    /// `dN1/dt = -l1 N1` and `dN2/dt = l1 N1 - l2 N2`, and `N2(0) = 0`:
    ///
    /// ```text
    /// N1(t) = N1(0) exp(-l1 t)
    ///
    /// N2(t) = N1(0) * l1 / (l2 - l1) * ( exp(-l1 t) - exp(-l2 t) )
    /// ```
    ///
    /// Derivation of the second line: substitute `N1` into the `N2` equation,
    /// multiply by the integrating factor `exp(l2 t)`, and integrate
    /// `d/dt [N2 exp(l2 t)] = l1 N1(0) exp((l2 - l1) t)` from 0 to t. The
    /// constant follows from `N2(0) = 0`.
    ///
    /// **This is the test that catches a transposed decay matrix.** With rows
    /// and columns swapped the daughter is fed by nothing at all — the entry
    /// that feeds it moves to `A[0][1]`, which multiplies a zero — so `N2(t)`
    /// comes out **exactly zero**, measured 0.000e0 of the correct value, and
    /// `fn transposed_decay_matrix_fails_bateman` below pins that.
    ///
    /// Pass criterion 1e-9 relative at `tol = 1e-10`. Measured, 2026-09-16:
    /// daughter 7.737e-11, parent 7.737e-11. At the default `tol` of 1e-3 the
    /// measured daughter error is 9.431e-4, which is the series truncation,
    /// not the matrix — see the module docs.
    #[test]
    fn two_nuclide_chain_matches_bateman() {
        let l1 = lambda_sr90();
        let l2 = lambda_y90();
        let chain = sr_y_zr_chain();
        let n0 = 7.0;
        let comp = atom_comp(&[(SR90, n0)]);
        let t = YEAR_SECS;

        let tight =
            decay_composition_with_tol(&comp, &chain, t, &AtomicMasses::MassNumber, 1e-10).unwrap();

        let want_parent = n0 * exp(-l1 * t);
        let want_daughter = n0 * l1 / (l2 - l1) * (exp(-l1 * t) - exp(-l2 * t));

        let got_parent = tight.atom()[&SR90];
        let got_daughter = tight.atom()[&Y90];

        assert!(
            rel_err(got_parent, want_parent) < 1e-9,
            "Bateman parent: got {got_parent}, want {want_parent}, rel err {}",
            rel_err(got_parent, want_parent)
        );
        assert!(
            rel_err(got_daughter, want_daughter) < 1e-9,
            "Bateman daughter: got {got_daughter}, want {want_daughter}, rel err {}",
            rel_err(got_daughter, want_daughter)
        );

        // The daughter is a real, non-trivial fraction of the inventory --
        // otherwise this test would pass on a matrix that produced nothing.
        assert!(got_daughter / n0 > 1e-5);

        // And the same case at upstream's default tolerance stays well inside
        // the 1e-3 bound that tolerance promises.
        let loose = decay_composition(&comp, &chain, t, &AtomicMasses::MassNumber).unwrap();
        let loose_daughter = loose.atom()[&Y90];
        assert!(
            rel_err(loose_daughter, want_daughter) < 1e-3,
            "Bateman daughter at default tol: rel err {}",
            rel_err(loose_daughter, want_daughter)
        );
    }

    /// The transposed decay matrix, asserted to be wrong.
    ///
    /// A guard on the guard: it pins the sign convention (parent by column,
    /// daughter by row) by showing that the other convention fails the Bateman
    /// reference by orders of magnitude, so a future reader cannot "fix" the
    /// convention and still see green tests.
    #[test]
    fn transposed_decay_matrix_fails_bateman() {
        let l1 = lambda_sr90();
        let l2 = lambda_y90();
        let t = YEAR_SECS;
        let n0 = 7.0;

        let order = [SR90, Y90, ZR90];
        let a = build_decay_matrix(&sr_y_zr_chain(), &order).unwrap();

        // Transpose it by hand.
        let mut at = Matrix::zeros(3, 3).unwrap();
        for i in 0..3 {
            for j in 0..3 {
                at.set(i, j, a.get(j, i));
            }
        }

        let x = UniformTaylor::solve(&at, &[n0, 0.0, 0.0], t, 1e-10).unwrap();
        let want_daughter = n0 * l1 / (l2 - l1) * (exp(-l1 * t) - exp(-l2 * t));
        assert!(
            rel_err(x[1], want_daughter) > 0.9,
            "the transposed matrix must not reproduce Bateman; got {}, want {want_daughter}",
            x[1]
        );
    }

    // -----------------------------------------------------------------------
    // Conservation, stability, half-life, identity
    // -----------------------------------------------------------------------

    /// Methodology: Sr-90 -> Y-90 -> Zr-90, every branching ratio 1.0 and the
    /// end point tracked, so no atoms leave the system and the decay matrix's
    /// column sums are exactly zero. Reference: the total atom count is
    /// invariant.
    ///
    /// Two tolerances, and the difference between them is the point of this
    /// test. At `tol = 1e-12` the solve conserves atoms to round-off: measured
    /// 7.446e-13 at one year and 9.731e-13 at five. At upstream's default
    /// `tol = 1e-3` it does **not** — the truncated Poisson tail discards
    /// 9.515e-4 of the inventory at one year and 9.331e-4 at five. That is
    /// within the bound `tol` promises, and it is one-signed: every discarded
    /// term is non-negative, so the solver loses atoms and never gains them.
    /// A caller running many steps at the default tolerance is therefore
    /// accumulating a loss, not a random walk.
    #[test]
    fn atom_count_is_conserved_in_a_closed_chain() {
        let chain = sr_y_zr_chain();
        let comp = atom_comp(&[(SR90, 2.0), (Y90, 0.5)]);
        let before: f64 = comp_math::sum(comp.atom());

        for years in [1.0_f64, 5.0] {
            let tight = decay_composition_with_tol(
                &comp,
                &chain,
                years * YEAR_SECS,
                &AtomicMasses::MassNumber,
                1e-12,
            )
            .unwrap();
            let total = comp_math::sum(tight.atom());
            assert!(
                rel_err(total, before) < 1e-9,
                "atom conservation over {years} a at tol 1e-12: got {total}, want {before}, rel err {}",
                rel_err(total, before)
            );

            // At upstream's default tolerance the loss is real, bounded by
            // `tol`, and always a loss.
            let loose = decay_composition(
                &comp,
                &chain,
                years * YEAR_SECS,
                &AtomicMasses::MassNumber,
            )
            .unwrap();
            let loose_total = comp_math::sum(loose.atom());
            assert!(
                loose_total <= before,
                "the truncated series must never create atoms: {loose_total} > {before}"
            );
            assert!(
                rel_err(loose_total, before) < DEFAULT_TOL,
                "atom conservation over {years} a at the default tol: rel err {}",
                rel_err(loose_total, before)
            );
        }
    }

    /// A chain whose branching ratios sum to less than one loses atoms, by
    /// design, and loses exactly the fraction the missing branch would have
    /// carried.
    #[test]
    fn truncated_branching_loses_atoms_by_the_missing_fraction() {
        let lambda = lambda_po210();
        // Only 60 % of the decays are tracked.
        let chain = DecayChain::new().with(PO210, DecayData::new(lambda, vec![(PB206, 0.6)]));
        let n0 = 1.0;
        let comp = atom_comp(&[(PO210, n0)]);
        let t = YEAR_SECS;

        let after =
            decay_composition_with_tol(&comp, &chain, t, &AtomicMasses::MassNumber, 1e-12).unwrap();
        let f = exp(-lambda * t);
        let want = n0 * f + n0 * 0.6 * (1.0 - f);
        let got = comp_math::sum(after.atom());
        assert!(rel_err(got, want) < 1e-9, "got {got}, want {want}");
    }

    /// A nuclide absent from the chain is stable and its amount does not
    /// change, at any elapsed time.
    #[test]
    fn a_stable_nuclide_does_not_decay() {
        let chain = sr_y_zr_chain(); // Zr-90 is present but with lambda = 0
        let empty = DecayChain::new(); // O-16 is absent entirely
        let o16 = crate::nuclide::nuc::O16;

        let comp = atom_comp(&[(ZR90, 4.0)]);
        let after =
            decay_composition(&comp, &chain, 1e6 * YEAR_SECS, &AtomicMasses::MassNumber).unwrap();
        assert_eq!(after.atom()[&ZR90], 4.0);

        let comp2 = atom_comp(&[(o16, 9.0)]);
        let after2 =
            decay_composition(&comp2, &empty, 1e6 * YEAR_SECS, &AtomicMasses::MassNumber).unwrap();
        assert_eq!(after2.atom()[&o16], 9.0);
        assert_eq!(after2.atom().len(), 1);
    }

    /// After exactly one half-life, half the parent remains.
    ///
    /// Reference: `N(T_1/2) = N0 / 2` by the definition of a half-life. Pass
    /// criterion 1e-12 relative; measured 0.000e0 — bit-exact — for both
    /// Po-210 and Sr-90, because a chain with no tracked daughters makes
    /// `B` the zero matrix and the answer is the `exp` factor itself.
    #[test]
    fn one_half_life_leaves_half() {
        for (nuc, lambda) in [(PO210, lambda_po210()), (SR90, lambda_sr90())] {
            let data = DecayData::new(lambda, Vec::new());
            let half_life = data.half_life();
            let chain = DecayChain::new().with(nuc, data);
            let comp = atom_comp(&[(nuc, 1.0)]);

            let after =
                decay_composition(&comp, &chain, half_life, &AtomicMasses::MassNumber).unwrap();
            let got = after.atom()[&nuc];
            assert!(
                rel_err(got, 0.5) < 1e-12,
                "one half-life: got {got}, want 0.5, rel err {}",
                rel_err(got, 0.5)
            );
        }
    }

    /// Zero elapsed time is the identity, exactly.
    ///
    /// With `alpha t = 0` the series terminates after its single first term,
    /// whose factor `exp(-0)` is exactly 1, so the result is bit-identical to
    /// the input rather than merely close to it. Asserted with `==`.
    #[test]
    fn zero_elapsed_time_is_the_identity() {
        let chain = sr_y_zr_chain();
        let comp = atom_comp(&[(SR90, 2.0), (Y90, 0.5), (ZR90, 0.25)]);
        let after = decay_composition(&comp, &chain, 0.0, &AtomicMasses::MassNumber).unwrap();
        assert_eq!(after.atom(), comp.atom());

        let x = UniformTaylor::solve(
            &build_decay_matrix(&chain, &[SR90, Y90, ZR90]).unwrap(),
            &[2.0, 0.5, 0.25],
            0.0,
            DEFAULT_TOL,
        )
        .unwrap();
        assert_eq!(x, vec![2.0, 0.5, 0.25]);
    }

    // -----------------------------------------------------------------------
    // The solver on its own
    // -----------------------------------------------------------------------

    /// Methodology: a diagonal matrix `diag(-0.1, -0.5, -1.0)` per second at
    /// `t = 2 s`, so `alpha t = 2` and the series runs to a real length rather
    /// than terminating at one term. Reference: `x_i(t) = x_i(0) exp(d_i t)`,
    /// elementwise. Pass criterion 1e-9 relative at `tol = 1e-10`; measured
    /// worst-component 1.128e-11.
    #[test]
    fn solver_matches_elementwise_exp_on_a_diagonal_matrix() {
        let d = [-0.1_f64, -0.5, -1.0];
        let mut a = Matrix::zeros(3, 3).unwrap();
        for (i, &di) in d.iter().enumerate() {
            a.set(i, i, di);
        }
        let x0 = [1.0_f64, 2.0, 3.0];
        let t = 2.0;

        assert_eq!(UniformTaylor::max_abs_diag(&a), 1.0);

        let x = UniformTaylor::solve(&a, &x0, t, 1e-10).unwrap();
        for i in 0..3 {
            let want = x0[i] * exp(d[i] * t);
            assert!(
                rel_err(x[i], want) < 1e-9,
                "component {i}: got {}, want {want}, rel err {}",
                x[i],
                rel_err(x[i], want)
            );
        }
    }

    /// `max_num_terms` reproduces upstream's termination rule: enough Poisson
    /// terms to leave at most `epsilon` in the tail, and exactly one term when
    /// `alpha t = 0`.
    #[test]
    fn max_num_terms_follows_the_poisson_tail() {
        assert_eq!(UniformTaylor::max_num_terms(0.0, DEFAULT_TOL).unwrap(), 1);
        // Monotone in alpha*t, and comfortably above alpha*t itself.
        let p10 = UniformTaylor::max_num_terms(10.0, DEFAULT_TOL).unwrap();
        let p100 = UniformTaylor::max_num_terms(100.0, DEFAULT_TOL).unwrap();
        assert!(p10 > 10 && p100 > 100 && p100 > p10);
        // A tighter tolerance needs more terms.
        assert!(UniformTaylor::max_num_terms(10.0, 1e-10).unwrap() > p10);
    }

    // -----------------------------------------------------------------------
    // The decay matrix, inspected directly
    // -----------------------------------------------------------------------

    /// The matrix build, entry by entry, on a branching chain. A wrong matrix
    /// is the likeliest defect in this file, so it is checked directly rather
    /// than only through a solve.
    #[test]
    fn decay_matrix_has_parents_in_columns_and_daughters_in_rows() {
        let l = 3.0_f64;
        let chain = DecayChain::new()
            .with(SR90, DecayData::new(l, vec![(Y90, 0.7), (ZR90, 0.3)]))
            .with(Y90, DecayData::single(5.0, ZR90));
        let order = [SR90, Y90, ZR90];
        let a = build_decay_matrix(&chain, &order).unwrap();

        assert_eq!(a.rows(), 3);
        assert_eq!(a.cols(), 3);
        // Column 0: Sr-90 decaying at 3 per second, 70/30 into Y-90 and Zr-90.
        assert_eq!(a.get(0, 0), -3.0);
        assert!(abs(a.get(1, 0) - 0.7 * l) < 1e-15);
        assert!(abs(a.get(2, 0) - 0.3 * l) < 1e-15);
        // Column 1: Y-90 decaying at 5 per second entirely into Zr-90.
        assert_eq!(a.get(0, 1), 0.0);
        assert_eq!(a.get(1, 1), -5.0);
        assert_eq!(a.get(2, 1), 5.0);
        // Column 2: Zr-90 is absent from the chain, so stable.
        for i in 0..3 {
            assert_eq!(a.get(i, 2), 0.0);
        }
        // Column sums vanish for a chain that conserves atoms.
        for j in 0..3 {
            let s: f64 = (0..3).map(|i| a.get(i, j)).sum();
            assert!(abs(s) < 1e-15, "column {j} sum {s}");
        }
    }

    /// A daughter outside the supplied nuclide list is dropped, which is the
    /// documented behaviour of [`build_decay_matrix`] and the reason
    /// [`decay_composition`] uses the reachable closure instead.
    #[test]
    fn a_daughter_outside_the_list_is_dropped() {
        let chain = DecayChain::new().with(SR90, DecayData::single(3.0, Y90));
        let a = build_decay_matrix(&chain, &[SR90]).unwrap();
        assert_eq!(a.rows(), 1);
        assert_eq!(a.get(0, 0), -3.0);
    }

    /// The closure follows branches transitively, includes seeds that are not
    /// parents, and terminates on a cycle.
    #[test]
    fn reachable_closure_walks_the_whole_chain() {
        let chain = sr_y_zr_chain();
        assert_eq!(chain.reachable_closure(&[SR90]), vec![SR90, Y90, ZR90]);
        assert_eq!(
            chain.reachable_closure(&[crate::nuclide::nuc::O16]),
            vec![crate::nuclide::nuc::O16]
        );

        // Unphysical, but a caller can write it: must terminate.
        let cyclic = DecayChain::new()
            .with(SR90, DecayData::single(1.0, Y90))
            .with(Y90, DecayData::single(1.0, SR90));
        assert_eq!(cyclic.reachable_closure(&[SR90]), vec![SR90, Y90]);
    }

    // -----------------------------------------------------------------------
    // Materials: mass rebuilt, atoms conserved
    // -----------------------------------------------------------------------

    /// Methodology: 1 kg of pure Po-210 (T_1/2 = 138.376 d) alpha-decaying to
    /// Pb-206 over one year, with `AtomicMasses::MassNumber` so the atomic
    /// masses are exactly 210 u and 206 u.
    ///
    /// Reference, closed form. With `f = exp(-lambda t)` the surviving
    /// fraction, atom count is conserved, so the mass ratio is the
    /// atom-weighted mass number ratio:
    ///
    /// ```text
    /// new mass / old mass = ( f * 210 + (1 - f) * 206 ) / 210
    /// ```
    ///
    /// Pass criterion 1e-9 relative on both the mass ratio and the conserved
    /// atom count, at `tol = 1e-12`. Measured 2026-09-16: mass ratio error
    /// 1.398e-13, atom-count error 1.403e-13, with `f = 1.604786e-1` and the
    /// mass falling from 1 kg to 9.8400912e-1 kg — a 1.5991 % loss, which is
    /// the alpha particles leaving. At upstream's default `tol = 1e-3` the
    /// same case comes out a further 6.216e-4 light, inside that tolerance's
    /// bound but visibly so.
    #[test]
    fn decaying_a_material_conserves_atoms_and_rebuilds_the_mass() {
        let masses = AtomicMasses::MassNumber;
        let lambda = lambda_po210();
        let chain = DecayChain::new().with(PO210, DecayData::single(lambda, PB206));

        let comp = Composition::from_nuclide(PO210, &masses).unwrap();
        let mut mat = Material::new(1.0, comp.clone()).unwrap();
        let atoms_before = 1.0 / 210.0; // kg / u

        decay_material_with_tol(&mut mat, &chain, YEAR_SECS, 42, &masses, 1e-12).unwrap();

        let f = exp(-lambda * YEAR_SECS);
        let want_ratio = (f * 210.0 + (1.0 - f) * 206.0) / 210.0;
        assert!(
            rel_err(mat.quantity(), want_ratio) < 1e-9,
            "mass ratio: got {}, want {want_ratio}, rel err {}",
            mat.quantity(),
            rel_err(mat.quantity(), want_ratio)
        );
        assert!(mat.quantity() < 1.0, "alpha decay must lose mass");

        // Atom count, reconstructed from the material's nuclide masses.
        let atoms_after: f64 = mat
            .nuclide_masses()
            .iter()
            .map(|(n, m)| m / f64::from(n.a()))
            .sum();
        assert!(
            rel_err(atoms_after, atoms_before) < 1e-9,
            "atom count: got {atoms_after}, want {atoms_before}, rel err {}",
            rel_err(atoms_after, atoms_before)
        );

        assert_eq!(mat.prev_decay_time(), 42);

        // The same case at upstream's default tolerance: still within the
        // bound `tol` promises, but visibly light.
        let mut loose = Material::new(1.0, comp).unwrap();
        decay_material(&mut loose, &chain, YEAR_SECS, 42, &masses).unwrap();
        assert!(loose.quantity() < mat.quantity());
        assert!(rel_err(loose.quantity(), want_ratio) < DEFAULT_TOL);
    }

    /// An empty material is a no-op apart from its decay time, and the unit
    /// value survives a decay.
    #[test]
    fn empty_material_only_advances_its_decay_time() {
        let masses = AtomicMasses::MassNumber;
        let chain = sr_y_zr_chain();
        let comp = Composition::from_nuclide(SR90, &masses).unwrap();

        let mut empty = Material::new(0.0, comp.clone()).unwrap();
        decay_material(&mut empty, &chain, YEAR_SECS, 7, &masses).unwrap();
        assert_eq!(empty.quantity(), 0.0);
        assert_eq!(empty.prev_decay_time(), 7);

        let mut priced = Material::with_unit_value(1.0, comp, 12.5).unwrap();
        decay_material(&mut priced, &chain, YEAR_SECS, 8, &masses).unwrap();
        assert_eq!(priced.unit_value(), 12.5);
        assert_eq!(priced.prev_decay_time(), 8);
    }

    /// A beta chain whose members share a mass number conserves mass too,
    /// because the mass per atom does not change. Sr-90, Y-90 and Zr-90 are
    /// all A = 90, so under `AtomicMasses::MassNumber` the kilograms are
    /// invariant — a useful cross-check that the mass rebuild is not
    /// introducing a spurious change of its own.
    ///
    /// Measured 2026-09-16 over five years at `tol = 1e-12`: 9.688e-13
    /// relative. At the default `tol = 1e-3` the same case loses 9.331e-4 of
    /// its mass, which is the series truncation showing up as a mass balance
    /// that does not close — the clearest demonstration in this file of why
    /// [`decay_material_with_tol`] exists.
    #[test]
    fn an_isobaric_chain_keeps_its_mass() {
        let masses = AtomicMasses::MassNumber;
        let chain = sr_y_zr_chain();
        let comp = Composition::from_nuclide(SR90, &masses).unwrap();
        let mut mat = Material::new(2.5, comp).unwrap();

        decay_material_with_tol(&mut mat, &chain, 5.0 * YEAR_SECS, 1, &masses, 1e-12)
            .unwrap();
        assert!(
            rel_err(mat.quantity(), 2.5) < 1e-9,
            "isobaric chain mass: got {}, rel err {}",
            mat.quantity(),
            rel_err(mat.quantity(), 2.5)
        );
    }

    // -----------------------------------------------------------------------
    // Error paths
    // -----------------------------------------------------------------------

    #[test]
    fn mismatched_dimensions_are_rejected() {
        let a = Matrix::zeros(3, 3).unwrap();
        assert!(matches!(
            UniformTaylor::solve(&a, &[1.0, 2.0], 1.0, DEFAULT_TOL),
            Err(CyclusError::Value(_))
        ));

        let rect = Matrix::zeros(2, 3).unwrap();
        assert!(matches!(
            UniformTaylor::solve(&rect, &[1.0, 2.0, 3.0], 1.0, DEFAULT_TOL),
            Err(CyclusError::Value(_))
        ));
    }

    #[test]
    fn bad_time_and_tolerance_are_rejected() {
        let a = Matrix::zeros(1, 1).unwrap();
        for t in [-1.0_f64, f64::NAN, f64::INFINITY] {
            assert!(matches!(
                UniformTaylor::solve(&a, &[1.0], t, DEFAULT_TOL),
                Err(CyclusError::Value(_))
            ));
        }
        for tol in [0.0_f64, 1.0, -1e-3, 2.0, f64::NAN] {
            assert!(matches!(
                UniformTaylor::solve(&a, &[1.0], 1.0, tol),
                Err(CyclusError::Value(_))
            ));
        }
    }

    #[test]
    fn negative_decay_constant_is_rejected() {
        let chain = DecayChain::new().with(SR90, DecayData::single(-1.0, Y90));
        assert!(matches!(chain.validate(), Err(CyclusError::Value(_))));
        assert!(matches!(
            build_decay_matrix(&chain, &[SR90, Y90]),
            Err(CyclusError::Value(_))
        ));
    }

    #[test]
    fn branching_ratios_summing_above_one_are_rejected() {
        let chain =
            DecayChain::new().with(SR90, DecayData::new(1.0, vec![(Y90, 0.7), (ZR90, 0.4)]));
        assert!(matches!(chain.validate(), Err(CyclusError::Value(_))));

        // Exactly one, and just under one, are both fine.
        let ok = DecayChain::new().with(SR90, DecayData::new(1.0, vec![(Y90, 0.7), (ZR90, 0.3)]));
        assert!(ok.validate().is_ok());
        let lossy = DecayChain::new().with(SR90, DecayData::new(1.0, vec![(Y90, 0.7)]));
        assert!(lossy.validate().is_ok());
    }

    #[test]
    fn malformed_chains_are_rejected() {
        // A branching ratio outside [0, 1].
        let bad_br = DecayChain::new().with(SR90, DecayData::new(1.0, vec![(Y90, -0.1)]));
        assert!(matches!(bad_br.validate(), Err(CyclusError::Value(_))));

        // A nuclide as its own daughter.
        let self_branch = DecayChain::new().with(SR90, DecayData::single(1.0, SR90));
        assert!(matches!(self_branch.validate(), Err(CyclusError::Value(_))));
        assert!(matches!(
            build_decay_matrix(&self_branch, &[SR90]),
            Err(CyclusError::Value(_))
        ));

        // A daughter listed twice.
        let dup = DecayChain::new().with(SR90, DecayData::new(1.0, vec![(Y90, 0.4), (Y90, 0.4)]));
        assert!(matches!(dup.validate(), Err(CyclusError::Value(_))));

        // A non-nuclide id (natural uranium is an element, not a nuclide).
        let elem = DecayChain::new().with(
            crate::nuclide::nuc::U_NATURAL,
            DecayData::single(1.0, SR90),
        );
        assert!(matches!(
            elem.validate(),
            Err(CyclusError::InvalidNuclide(_))
        ));

        // A well-formed chain passes.
        assert!(sr_y_zr_chain().validate().is_ok());
    }

    #[test]
    fn matrix_build_rejects_an_empty_or_duplicated_nuclide_list() {
        let chain = sr_y_zr_chain();
        assert!(matches!(
            build_decay_matrix(&chain, &[]),
            Err(CyclusError::Value(_))
        ));
        assert!(matches!(
            build_decay_matrix(&chain, &[SR90, SR90]),
            Err(CyclusError::Value(_))
        ));
    }

    #[test]
    fn negative_decay_time_is_rejected() {
        let chain = sr_y_zr_chain();
        let comp = atom_comp(&[(SR90, 1.0)]);
        assert!(matches!(
            decay_composition(&comp, &chain, -1.0, &AtomicMasses::MassNumber),
            Err(CyclusError::Value(_))
        ));

        let mut mat = Material::new(1.0, comp).unwrap();
        assert!(matches!(
            decay_material(&mut mat, &chain, -1.0, 0, &AtomicMasses::MassNumber),
            Err(CyclusError::Value(_))
        ));
    }

    /// A step so long that `exp(-alpha t)` underflows is refused rather than
    /// silently returning zeros. This is the case upstream's "gross heuristic"
    /// was reaching for, and the port reports it instead of zeroing the decay
    /// constant. See [`build_decay_matrix`]'s docs.
    #[test]
    fn an_underflowing_step_is_refused() {
        // lambda = 1 per second, decayed for 1e6 seconds: alpha*t = 1e6.
        let chain = DecayChain::new().with(SR90, DecayData::single(1.0, Y90));
        let comp = atom_comp(&[(SR90, 1.0)]);
        assert!(matches!(
            decay_composition(&comp, &chain, 1.0e6, &AtomicMasses::MassNumber),
            Err(CyclusError::Value(_))
        ));
    }

    #[test]
    fn half_life_round_trips_through_the_decay_constant() {
        let t_half = 138.376 * 86400.0;
        let d = DecayData::from_half_life(t_half, vec![(PB206, 1.0)]).unwrap();
        assert!(rel_err(d.half_life(), t_half) < 1e-12);
        assert!(rel_err(d.decay_constant, lambda_po210()) < 1e-12);
        assert!(DecayData::stable().is_stable());
        assert_eq!(DecayData::stable().half_life(), f64::INFINITY);
        assert!(DecayData::from_half_life(0.0, Vec::new()).is_err());
        assert!(DecayData::from_half_life(-1.0, Vec::new()).is_err());
    }

    #[test]
    fn chain_accessors_report_stability_consistently() {
        let chain = sr_y_zr_chain();
        assert_eq!(chain.len(), 3);
        assert!(!chain.is_empty());
        assert!(DecayChain::new().is_empty());
        assert!(!chain.is_stable(SR90));
        assert!(chain.is_stable(ZR90)); // present, lambda = 0
        assert!(chain.is_stable(crate::nuclide::nuc::O16)); // absent
        assert_eq!(chain.decay_constant(crate::nuclide::nuc::O16), 0.0);
        assert!(rel_err(chain.decay_constant(SR90), lambda_sr90()) < 1e-15);
        assert_eq!(chain.parents(), vec![SR90, Y90, ZR90]);
    }

    /// Prints every measured V&V residual quoted in the module and function
    /// docs, so the numbers recorded there can be regenerated rather than
    /// trusted:
    ///
    /// ```text
    /// cargo test -p outram-park-fork-cyclus --release --lib decay::tests::vv_report -- --nocapture
    /// ```
    #[test]
    fn vv_report() {
        use std::println;

        let masses = AtomicMasses::MassNumber;
        let t = YEAR_SECS;
        let l1 = lambda_sr90();
        let l2 = lambda_y90();
        let chain = sr_y_zr_chain();

        println!("--- decay.rs measured V&V residuals (relative error) ---");

        // 1. single nuclide, default tol
        let chain1 = DecayChain::new().with(SR90, DecayData::new(l1, Vec::new()));
        let c1 = atom_comp(&[(SR90, 3.5)]);
        let a1 = decay_composition(&c1, &chain1, t, &masses).unwrap();
        println!(
            "single nuclide            tol 1e-3   {:.3e}",
            rel_err(a1.atom()[&SR90], 3.5 * exp(-l1 * t))
        );

        // 2. Bateman
        let c = atom_comp(&[(SR90, 7.0)]);
        let want_p = 7.0 * exp(-l1 * t);
        let want_d = 7.0 * l1 / (l2 - l1) * (exp(-l1 * t) - exp(-l2 * t));
        let tight = decay_composition_with_tol(&c, &chain, t, &masses, 1e-10).unwrap();
        let loose = decay_composition(&c, &chain, t, &masses).unwrap();
        println!(
            "Bateman daughter          tol 1e-10  {:.3e}",
            rel_err(tight.atom()[&Y90], want_d)
        );
        println!(
            "Bateman daughter          tol 1e-3   {:.3e}",
            rel_err(loose.atom()[&Y90], want_d)
        );
        println!(
            "Bateman parent            tol 1e-10  {:.3e}",
            rel_err(tight.atom()[&SR90], want_p)
        );
        println!(
            "Bateman parent            tol 1e-3   {:.3e}",
            rel_err(loose.atom()[&SR90], want_p)
        );

        // 3. diagonal
        let d = [-0.1_f64, -0.5, -1.0];
        let mut m = Matrix::zeros(3, 3).unwrap();
        for (i, &di) in d.iter().enumerate() {
            m.set(i, i, di);
        }
        let x = UniformTaylor::solve(&m, &[1.0, 2.0, 3.0], 2.0, 1e-10).unwrap();
        let worst = (0..3)
            .map(|i| rel_err(x[i], [1.0, 2.0, 3.0][i] * exp(d[i] * 2.0)))
            .fold(0.0_f64, f64::max);
        println!("diagonal exp              tol 1e-10  {worst:.3e}");

        // 4. atom conservation
        let cc = atom_comp(&[(SR90, 2.0), (Y90, 0.5)]);
        let before = comp_math::sum(cc.atom());
        for years in [1.0_f64, 5.0] {
            let tight =
                decay_composition_with_tol(&cc, &chain, years * t, &masses, 1e-12).unwrap();
            let loose = decay_composition(&cc, &chain, years * t, &masses).unwrap();
            println!(
                "atom conservation {years} a      tol 1e-12  {:.3e}   tol 1e-3 {:.3e}",
                rel_err(comp_math::sum(tight.atom()), before),
                rel_err(comp_math::sum(loose.atom()), before)
            );
        }

        // 5. half life
        for (nuc, lam, name) in [(PO210, lambda_po210(), "Po-210"), (SR90, l1, "Sr-90 ")] {
            let data = DecayData::new(lam, Vec::new());
            let th = data.half_life();
            let ch = DecayChain::new().with(nuc, data);
            let a = decay_composition(&atom_comp(&[(nuc, 1.0)]), &ch, th, &masses).unwrap();
            println!(
                "one half-life {name}      tol 1e-3   {:.3e}",
                rel_err(a.atom()[&nuc], 0.5)
            );
        }

        // 6. material mass rebuild
        let lam = lambda_po210();
        let chain_po = DecayChain::new().with(PO210, DecayData::single(lam, PB206));
        let comp_po = Composition::from_nuclide(PO210, &masses).unwrap();
        let mut mat = Material::new(1.0, comp_po.clone()).unwrap();
        decay_material_with_tol(&mut mat, &chain_po, t, 42, &masses, 1e-12).unwrap();
        let f = exp(-lam * t);
        let want_ratio = (f * 210.0 + (1.0 - f) * 206.0) / 210.0;
        let atoms_after: f64 = mat
            .nuclide_masses()
            .iter()
            .map(|(n, mm)| mm / f64::from(n.a()))
            .sum();
        let mut mat_loose = Material::new(1.0, comp_po).unwrap();
        decay_material(&mut mat_loose, &chain_po, t, 42, &masses).unwrap();
        println!(
            "material mass ratio       tol 1e-12  {:.3e}   tol 1e-3 {:.3e}",
            rel_err(mat.quantity(), want_ratio),
            rel_err(mat_loose.quantity(), want_ratio)
        );
        println!(
            "material atom count       tol 1e-12  {:.3e}",
            rel_err(atoms_after, 1.0 / 210.0)
        );
        println!(
            "   f = {f:.6e}, mass {:.7e} kg, loss {:.4} %",
            mat.quantity(),
            (1.0 - mat.quantity()) * 100.0
        );

        // 7. isobaric material, 5 a
        let comp_sr = Composition::from_nuclide(SR90, &masses).unwrap();
        let mut iso = Material::new(2.5, comp_sr.clone()).unwrap();
        decay_material_with_tol(&mut iso, &chain, 5.0 * t, 1, &masses, 1e-12).unwrap();
        let mut iso_loose = Material::new(2.5, comp_sr).unwrap();
        decay_material(&mut iso_loose, &chain, 5.0 * t, 1, &masses).unwrap();
        println!(
            "isobaric mass 5 a         tol 1e-12  {:.3e}   tol 1e-3 {:.3e}",
            rel_err(iso.quantity(), 2.5),
            rel_err(iso_loose.quantity(), 2.5)
        );

        // 8. transposed matrix, for the sign-convention guard
        let a_ok = build_decay_matrix(&chain, &[SR90, Y90, ZR90]).unwrap();
        let mut at = Matrix::zeros(3, 3).unwrap();
        for i in 0..3 {
            for j in 0..3 {
                at.set(i, j, a_ok.get(j, i));
            }
        }
        let xt = UniformTaylor::solve(&at, &[7.0, 0.0, 0.0], t, 1e-10).unwrap();
        println!(
            "transposed matrix daughter is {:.3e} of the correct value",
            xt[1] / want_d
        );

        // 9. series length, for the cost note
        println!(
            "max_num_terms(alpha t = {:.1}): tol 1e-3 -> {}, tol 1e-12 -> {}",
            l2 * t,
            UniformTaylor::max_num_terms(l2 * t, DEFAULT_TOL).unwrap(),
            UniformTaylor::max_num_terms(l2 * t, 1e-12).unwrap()
        );
    }
}
