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
    let mut bin = 0usize;
    for f in &tally.filters {
        let b = match f.get_bin(&ev) {
            Some(b) => b,
            None => return,
        };
        bin = bin * f.n_bins() + b;
    }

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
