//! `PURR` — unresolved-resonance probability tables for Monte Carlo.
//!
//! Produces probability tables for the unresolved resonance range (URR),
//! suitable for continuous-energy Monte Carlo self-shielding (MCNP/OpenMC),
//! where the Bondarenko method used by UNRESR/GROUPR is not applicable. Builds
//! the tables by sampling explicit **resonance ladders** from the ENDF average
//! parameters and binning the resulting cross sections into probability bins per
//! energy.
//!
//! Ported from `purr.f90` (2919 lines):
//!
//! - **ENDF File 2/3 reading** — PURR's `rdf2un`/`rdf3un`/`unfac2`/`intrf2`/
//!   `intr2` are structurally identical to UNRESR's `rdunf2`/`rdunf3`/
//!   `uunfac`/`intrf`/`intr` (same ENDF LRU=2 Case A/B/C layout, same
//!   formulas) — this module reuses [`crate::unresr::mf2`] and
//!   [`crate::unresr::penetrability_factor`] directly rather than duplicating
//!   them.
//! - [`wfun`] — `uw2`, PURR's own complex-probability-integral evaluator
//!   (algorithmically identical to [`crate::unresr::wfun::uw`] but for one
//!   small genuine difference — see its doc comment).
//! - [`Rng`] — `rann`, NJOY's shuffled-LCG pseudo-random generator.
//! - [`generate_ladder`] — `ladr2`, sampling one resonance ladder (Wigner
//!   spacing, χ² / Porter-Thomas widths) for one sequence.
//! - [`infinite_dilution_reference`] (+ private `gnrx`) — `unresx`, the
//!   analytic infinite-dilution cross-section reference used as a
//!   convergence check on the Monte Carlo tables.
//! - [`read_heating_cross_sections`] — `rdheat`, reading HEATR's partial
//!   heating cross sections (MT=301/302/318/402) from the PENDF tape.
//!
//! - [`unrest::line_shape`] + [`wfun::DopplerTable`] — the Doppler line-shape
//!   evaluator across four precision tiers (asymptotic → 2-term rational →
//!   3-term rational → table lookup), and [`probability_table`] — `unrest`,
//!   the Monte Carlo probability-table binning core itself.
//!
//! **`unrest`'s index-range bookkeeping is not replicated literally.** Upstream
//! finds, for each resonance, which of its sample points need which precision
//! tier via a chain of binary searches (`fsrch`) on sub-ranges of the sorted
//! energy array, each search boundary reused as the next search's starting
//! point — a performance optimisation for repeatedly narrowing a large sorted
//! array. Because the Doppler-scaled offset `xs(ie) = ctx·(es(ie)−E_r)` is a
//! monotonic function of the (already sorted) `es(ie)`, this index-chain always
//! assigns the *same* tier to the *same* point as a direct classification of
//! that point's own `(x, y)` against the four tier thresholds would (verified
//! by tracing every threshold and fall-through in the upstream source). This
//! port therefore classifies each point directly ([`unrest::line_shape`]) rather than
//! replicating the binary-search chain — identical results, far less
//! reused-index bookkeeping to get right in a language without `GOTO`.
//!
//! **Status:** the PENDF MT=152 (Bondarenko table) / MT=153 (probability
//! table) output-tape bookkeeping is not ported, for the same reason as every
//! other module's driver split: this crate has no established PENDF
//! output-section-writer concept yet, and it is pure tape plumbing, not
//! physics. `run()` remains [`crate::NjoyError::NotPorted`]. See `README.md`
//! for the full status, the tier-classification equivalence argument in more
//! detail, and caveats.

pub mod ladder;
pub mod unrest;
pub mod wfun;

#[cfg(test)]
mod tests;

pub use ladder::{
    generate_ladder, infinite_dilution_reference, read_heating_cross_sections,
    InfiniteDilutionResult, LadderResonance, Rng, SequenceLadderParams,
};
pub use unrest::{probability_table, ConvergenceStats, ProbabilityTable, ProbabilityTableResult};

use crate::NjoyError;

/// Run the PURR card-input driver. Placeholder — the ported pieces (ENDF
/// parsing via [`crate::unresr::mf2`], [`generate_ladder`],
/// [`infinite_dilution_reference`], [`read_heating_cross_sections`],
/// [`wfun::uw2`], [`probability_table`]) are reached directly; the PENDF
/// MT=152/MT=153 output-tape bookkeeping is not ported — see the module docs.
pub fn run() -> Result<(), NjoyError> {
    Err(NjoyError::NotPorted(
        "purr driver (PENDF MT=152/153 tape writer not ported — see module docs)",
    ))
}
