//! Tally scoring — accumulate scores at collision events (collision estimator).
//!
//! C++ source: `src/tallies/tally_scoring.cpp`.
//!
//! After a real collision the transport loop (see
//! [`crate::physics::transport_csg`]) calls [`score_collision`], which:
//!   1. builds a [`FilterEvent`] snapshot of the particle state,
//!   2. maps it through every filter on the tally to a flat bin index (a
//!      conjunction — if any filter rejects the event, nothing is scored),
//!   3. accumulates each requested [`ScoreType`] into that bin using the
//!      **collision estimator**.
//!
//! # Collision estimator
//!
//! For a neutron of weight `w` colliding in a material of macroscopic total
//! `Σ_t`, the collision-estimator contributions are
//!
//! - flux:            `w / Σ_t`
//! - reaction rate x: `w · Σ_x / Σ_t`   (fission, ν-fission, …)
//! - total rate:      `w`               (one collision)
//!
//! (`src/tallies/tally_scoring.cpp`, `score_general` collision branch.) This is
//! the simplest unbiased estimator; a track-length estimator (bead op-6tz.9
//! follow-up) would additionally score along free-flight segments.

use super::filter::FilterEvent;
use super::tally::{ScoreType, Tally};
use crate::material::material::MacroXs;

/// Map a [`FilterEvent`] through every filter attached to `tally` to the flat
/// *filter* bin index (row-major, first filter slowest-varying), or `None` if any
/// filter rejects the event (the filters act as a conjunction).
///
/// This is the shared filter-reduction used by both the collision estimator
/// ([`score_collision`]) and the track-length estimator
/// ([`score_track_length`]). The returned index is the base bin *before* it is
/// expanded by the per-score stride — the caller multiplies by
/// `tally.scores.len()` and adds the score offset.
fn filter_bin(tally: &Tally, ev: &FilterEvent) -> Option<usize> {
    let mut bin = 0usize;
    for f in &tally.filters {
        let b = f.get_bin(ev)?;
        bin = bin * f.n_bins() + b;
    }
    Some(bin)
}

/// Track-length contribution of one score over a streamed segment of length `d`
/// \[cm\] at weight `w`, given the material's macroscopic cross sections at the
/// segment energy (`None` in a void — only the flux score is defined there).
///
/// The track-length estimator integrates the reaction rate density along the
/// free-flight path: for a segment of length `d` the flux estimate is `w·d` and a
/// reaction-rate estimate is `w·d·Σ_x` (`src/tallies/tally_scoring.cpp`,
/// `score_general` track-length branch). Surface-only scores (`Current`) and the
/// event counter are not track-length quantities and contribute nothing here.
fn track_length_value(score: &ScoreType, d: f64, macro_xs: Option<&MacroXs>, w: f64) -> f64 {
    let wd = w * d;
    match (score, macro_xs) {
        (ScoreType::Flux, _) => wd,
        (ScoreType::Total, Some(x)) => wd * x.total,
        (ScoreType::Fission, Some(x)) => wd * x.fission,
        (ScoreType::NuFission, Some(x)) => wd * x.nu_fission,
        // Σ_a is not carried on MacroXs (only Σ_t, Σ_s, Σ_f, ν-Σ_f). Approximate
        // the absorption rate as the non-scatter fraction (Σ_t − Σ_s), matching
        // the collision-estimator convention above. Documented gap op-6tz.9.
        (ScoreType::Absorption, Some(x)) => wd * (x.total - x.elastic).max(0.0),
        (ScoreType::ScatterN, Some(x)) => wd * x.elastic,
        // Void segment (no material): only the flux score is defined; reaction
        // rates require Σ_x, so they contribute nothing.
        (_, None) => 0.0,
        // Surface current / event counter are not track-length estimators.
        (ScoreType::Current | ScoreType::Events, _) => 0.0,
    }
}

/// Score one streamed free-flight segment into the **per-batch** accumulator
/// `batch` via the **track-length estimator**.
///
/// This is the primary flux estimator for the CSG k-eigenvalue loop
/// ([`crate::physics::transport_csg::run_keff_csg`]): every segment a particle
/// streams — whether it ends in a collision or a surface crossing — deposits
/// `w·d` (flux) and `w·d·Σ_x` (reaction rates) into the bin matching its cell,
/// material, universe and energy. It is lower-variance than the collision
/// estimator in optically thin regions because it scores on *every* flight, not
/// only at collision sites.
///
/// # Per-batch accumulation
///
/// Contributions are added into `batch` — a scratch buffer the caller zeroes at
/// the start of each batch (generation) and flushes into the tally's persistent
/// [`super::tally::TallyBin`]s with [`flush_batch`] at the batch's end. This gives
/// the standard Monte-Carlo tally statistics: one *realization* per batch, so the
/// running mean and variance are over batches, not over individual events.
/// `batch` must have length `tally.n_bins()`.
///
/// # Parameters
/// - `batch` — per-batch flat accumulator, length `tally.n_bins()`.
/// - `tally` — the tally definition (filters + scores) read immutably.
/// - `cell_idx` / `material_idx` / `universe_idx` — leaf geometry indices of the
///   segment (`material_idx == usize::MAX` in a void).
/// - `energy` — the particle energy \[eV\] over the segment (constant along a
///   free flight; the outgoing energy is only set after the collision is scored).
/// - `distance` — the streamed segment length \[cm\].
/// - `macro_xs` — the material's macroscopic cross sections at `energy`, or `None`
///   in a void (then only the flux score deposits).
/// - `weight` — particle statistical weight (1.0 for analog transport).
#[allow(clippy::too_many_arguments)]
pub fn score_track_length(
    batch: &mut [f64],
    tally: &Tally,
    cell_idx: usize,
    material_idx: usize,
    universe_idx: usize,
    energy: f64,
    distance: f64,
    macro_xs: Option<&MacroXs>,
    weight: f64,
) {
    if distance <= 0.0 || !distance.is_finite() {
        return;
    }
    let ev = FilterEvent {
        cell_idx,
        material_idx,
        universe_idx,
        energy,
        surface_idx: usize::MAX,
    };
    let Some(bin) = filter_bin(tally, &ev) else {
        return;
    };
    let n_scores = tally.scores.len();
    for (s_idx, score) in tally.scores.iter().enumerate() {
        let val = track_length_value(score, distance, macro_xs, weight);
        // Guard non-finite reaction-rate contributions. A track-length reaction
        // rate is `w·d·Σ_x` — an *absolute* macroscopic cross section. In a medium
        // with no thermal-scattering cutoff (free-gas moderation), a history can
        // slow down until its energy underflows toward 0, where 1/v cross sections
        // (fission, absorption) overflow f64 to ±∞ and would poison the bin. The
        // flux score `w·d` is always finite. Such non-physical E→0 contributions
        // are dropped rather than propagated (documented gap op-6tz.9); the
        // collision estimator is immune because it scores the ratio Σ_x/Σ_t.
        if val.is_finite() {
            batch[bin * n_scores + s_idx] += val;
        }
    }
}

/// Close a batch (generation): flush every per-batch accumulator into the tally's
/// persistent bins as one realization, then zero the accumulator for the next
/// batch.
///
/// Each call records one MC realization per bin — so after `n` active batches the
/// bins' `count` is `n`, [`super::tally::TallyBin::mean`] is the mean batch score,
/// and [`super::tally::TallyBin::rel_std_dev`] is the batch-to-batch relative
/// standard deviation (the standard tally uncertainty). `batch` must have length
/// `tally.bins.len()`.
pub fn flush_batch(tally: &mut Tally, batch: &mut [f64]) {
    for (dst, src) in tally.bins.iter_mut().zip(batch.iter_mut()) {
        dst.score(*src);
        *src = 0.0;
    }
}

/// Score one real collision into `tally` via the collision estimator.
///
/// # Parameters
/// - `tally` — the tally to accumulate into (filters + scores + bins).
/// - `cell_idx` / `material_idx` / `universe_idx` — the leaf geometry indices of
///   the collision site (for Cell/Material/Universe filters).
/// - `energy` — incident energy \[eV\] (for an EnergyFilter).
/// - `sigma_t` — macroscopic total Σ_t \[cm⁻¹\] at the collision.
/// - `macro_xs` — the material's macroscopic cross sections at `energy`, for the
///   reaction-rate scores.
/// - `weight` — particle statistical weight (1.0 for analog transport).
///
/// If any attached filter does not match the event, the collision is not scored
/// (the filters act as a conjunction).
#[allow(clippy::too_many_arguments)]
pub fn score_collision(
    tally: &mut Tally,
    cell_idx: usize,
    material_idx: usize,
    universe_idx: usize,
    energy: f64,
    sigma_t: f64,
    macro_xs: &MacroXs,
    weight: f64,
) {
    if sigma_t <= 0.0 {
        return;
    }
    let ev = FilterEvent {
        cell_idx,
        material_idx,
        universe_idx,
        energy,
        surface_idx: usize::MAX,
    };

    // Map the event through every filter to a flat bin index (row-major, first
    // filter slowest-varying). A rejecting filter drops the score.
    let Some(bin) = filter_bin(tally, &ev) else {
        return;
    };

    let n_scores = tally.scores.len();
    for (s_idx, score) in tally.scores.iter().enumerate() {
        let val = match score {
            ScoreType::Flux => weight / sigma_t,
            ScoreType::Total => weight,
            ScoreType::Fission => weight * macro_xs.fission / sigma_t,
            ScoreType::NuFission => weight * macro_xs.nu_fission / sigma_t,
            // Absorption Σ_a is not carried on MacroXs yet (only Σ_t, Σ_s, Σ_f,
            // ν-Σ_f). Approximate the collision-estimator absorption rate as the
            // non-scatter fraction Σ_t − Σ_s over Σ_t. Documented gap op-6tz.9.
            ScoreType::Absorption => weight * (sigma_t - macro_xs.elastic).max(0.0) / sigma_t,
            ScoreType::ScatterN => weight * macro_xs.elastic / sigma_t,
            ScoreType::Current | ScoreType::Events => weight,
        };
        tally.bins[bin * n_scores + s_idx].score(val);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tally::filter::{CellFilter, Filter};
    use crate::tally::tally::{Tally, TallyBin};

    fn cell_flux_tally(cells: Vec<usize>) -> Tally {
        let filter = CellFilter { cell_indices: cells };
        let n = filter.n_bins();
        Tally {
            id: 1,
            name: "flux".into(),
            filters: vec![Box::new(filter)],
            scores: vec![ScoreType::Flux],
            bins: vec![TallyBin::default(); n],
        }
    }

    /// A collision in a filtered cell scores `1/Σ_t`; a collision in an
    /// unfiltered cell scores nothing.
    #[test]
    fn cell_flux_collision_estimator() {
        let mut t = cell_flux_tally(vec![0, 1]);
        let xs = MacroXs { total: 0.5, elastic: 0.4, fission: 0.05, nu_fission: 0.12 };
        // Two collisions in cell 0 (Σ_t = 0.5 ⇒ 2 cm each), one in cell 5 (ignored).
        score_collision(&mut t, 0, 0, 0, 1.0e6, 0.5, &xs, 1.0);
        score_collision(&mut t, 0, 0, 0, 1.0e6, 0.5, &xs, 1.0);
        score_collision(&mut t, 5, 0, 0, 1.0e6, 0.5, &xs, 1.0);
        assert!((t.bins[0].sum - 4.0).abs() < 1e-12, "cell-0 flux sum {}", t.bins[0].sum);
        assert_eq!(t.bins[0].count, 2);
        assert_eq!(t.bins[1].count, 0, "cell-1 saw no collisions");
    }
}
