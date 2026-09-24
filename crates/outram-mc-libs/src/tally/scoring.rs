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

use super::filter::{FilterEvent, FilterKind};
use super::tally::{ScoreType, Tally, TallyBin};
use crate::geometry::position::Position;
use crate::material::material::MacroXs;

/// Recoverable energy per fission `Q` \[J\] — the constant multiplier of the
/// [`ScoreType::KappaFission`] score.
///
/// OpenMC reads a per-nuclide, energy-dependent `q_recoverable` from the
/// `fission_energy_release` data of each nuclide
/// (`src/nuclide.cpp:335-336`, `fission_q_recov_`) and multiplies the fission
/// reaction rate by it (`src/tallies/tally_scoring.cpp:1480`,
/// `SCORE_KAPPA_FISSION`). This crate's LOW-tier embedded WMP/collapsed data
/// does **not** carry per-nuclide fission-energy-release curves (that is bead
/// op-6tz.24), so we use a single documented constant appropriate for
/// U-235-dominated fuel:
///
/// `Q ≈ 193.4 MeV = 193.4e6 eV × 1.602176634e-19 J/eV ≈ 3.0982e-11 J`.
///
/// The 193.4 MeV recoverable value is the textbook U-235 thermal-fission figure
/// (e.g. Lamarsh & Baratta, *Introduction to Nuclear Engineering*; the value
/// excludes the ~10 MeV lost to antineutrinos, matching OpenMC's `q_recoverable`
/// semantics). Because it is a single constant rather than the true
/// energy/nuclide-dependent curve, the *absolute* kappa-fission watts this
/// produces are not a benchmark — but they are exactly proportional to the
/// fission rate, which is what the power-normalization round-trip needs.
pub const Q_FISSION_J: f64 = 3.0982e-11;

/// **Prompt** fission energy release \[J\] — `SCORE_FISS_Q_PROMPT`.
///
/// `181.7 MeV` for U-235: the recoverable 193.4 MeV of [`Q_FISSION_J`] less
/// the delayed beta (~6.5 MeV) and delayed gamma (~5.2 MeV) components
/// (Lamarsh & Baratta, *Introduction to Nuclear Engineering*, fission energy
/// budget table). `181.7e6 x 1.602176634e-19 J`.
///
/// Like [`Q_FISSION_J`] this is **one constant, not the nuclide- and
/// energy-dependent curve upstream carries**, so the absolute watts are not a
/// benchmark; the prompt/recoverable *ratio* is what this exists to make
/// available. Stated here rather than left implicit, because a power figure
/// quoted from it would otherwise look more authoritative than it is.
pub const Q_FISSION_PROMPT_J: f64 = 2.9114e-11;

/// **Recoverable** fission energy release \[J\] — `SCORE_FISS_Q_RECOV`.
///
/// The same 193.4 MeV as [`Q_FISSION_J`]. They are separate names because they
/// are separate upstream scores (`SCORE_KAPPA_FISSION` and `SCORE_FISS_Q_RECOV`
/// differ in upstream's data-driven form even where this port's single
/// constant makes them equal); collapsing them here would hide that a
/// data-driven version has to split them again.
pub const Q_FISSION_RECOVERABLE_J: f64 = Q_FISSION_J;

/// Neutron speed \[cm/s\] at kinetic energy `e` \[eV\], non-relativistic.
///
/// `v = sqrt(2E/m)`, with `m_n = 1.67492749804e-27 kg` and
/// `1 eV = 1.602176634e-19 J`; the factor 100 converts m/s to cm/s.
///
/// Non-relativistic is correct to better than 0.1 % below ~20 MeV, which is
/// the whole neutron range this crate transports. Upstream's `p.speed(E)` is
/// relativistic; the difference at 20 MeV is 1.1 % in `v` and therefore in
/// `1/v`, which matters for nothing this score is used for (the generation
/// time is dominated by thermal and epithermal flux). Stated rather than
/// silently assumed.
pub fn neutron_speed_cm_per_s(e: f64) -> f64 {
    const J_PER_EV: f64 = 1.602_176_634e-19;
    const MASS_KG: f64 = 1.674_927_498_04e-27;
    if e <= 0.0 {
        return 0.0;
    }
    (2.0 * e * J_PER_EV / MASS_KG).sqrt() * 100.0
}

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
fn track_length_value(
    score: &ScoreType,
    d: f64,
    macro_xs: Option<&MacroXs>,
    w: f64,
    energy: f64,
) -> f64 {
    let wd = w * d;
    match (score, macro_xs) {
        (ScoreType::Flux, _) => wd,
        (ScoreType::Total, Some(x)) => wd * x.total,
        (ScoreType::Fission, Some(x)) => wd * x.fission,
        (ScoreType::NuFission, Some(x)) => wd * x.nu_fission,
        // Kappa-fission: fission energy-deposition rate. Track-length estimator is
        // `w·d·Σ_f·Q` — the fission reaction rate scaled by the recoverable energy
        // per fission `Q` [J] (`src/tallies/tally_scoring.cpp:1480`). Guarded like
        // the fission arm against E→0 non-finite Σ_f (see `score_track_length`).
        (ScoreType::KappaFission, Some(x)) => wd * x.fission * Q_FISSION_J,
        // Real absorption Σ_a = capture + fission (`MacroXs::absorption`,
        // aggregated from each nuclide's `MicroXS::absorption`). This is the
        // OpenMC MT=27 quantity — the sum of the non-redundant disappearance
        // reactions plus fission (`src/nuclide.cpp:409-417`), **not**
        // `Σ_t − Σ_elastic`, which would wrongly count inelastic + (n,2n).
        (ScoreType::Absorption, Some(x)) => wd * x.absorption.max(0.0),
        (ScoreType::ScatterN, Some(x)) => wd * x.elastic,
        // ── GitHub #262 ─────────────────────────────────────────────────────
        // `SCORE_SCATTER` is Sigma_t - Sigma_a (`tally_scoring.cpp:629`), NOT
        // the elastic channel that `ScatterN` scores. Clamped at zero: the two
        // come from separate tabulations and their difference can dip very
        // slightly negative through interpolation alone.
        (ScoreType::Scatter, Some(x)) => wd * (x.total - x.absorption).max(0.0),
        // `SCORE_NU_SCATTER` weights each scatter by the neutrons it emits.
        // **This port scores it identically to `Scatter`**, because the
        // per-reaction (n,xn) yields are not carried on `MacroXs`. That is a
        // KNOWN UNDERCOUNT wherever (n,2n) is significant - a fast metal
        // system - and it is stated here rather than left to be discovered
        // from a number that looks right. Do not quote a nu-scatter rate from
        // this port as if the multiplicity were included.
        (ScoreType::NuScatter, Some(x)) => wd * (x.total - x.absorption).max(0.0),
        (ScoreType::DelayedNuFission, Some(x)) => wd * x.nu_fission_delayed,
        (ScoreType::PromptNuFission, Some(x)) => {
            wd * (x.nu_fission - x.nu_fission_delayed).max(0.0)
        }
        (ScoreType::DecayRate, Some(x)) => wd * x.decay_rate,
        (ScoreType::FissionQPrompt, Some(x)) => wd * x.fission * Q_FISSION_PROMPT_J,
        (ScoreType::FissionQRecoverable, Some(x)) => wd * x.fission * Q_FISSION_RECOVERABLE_J,
        // `SCORE_INVERSE_VELOCITY` is `flux / v` (`tally_scoring.cpp:612`) and
        // needs no cross section at all, so it is scored in the void arm too.
        (ScoreType::InverseVelocity, _) => {
            let v = neutron_speed_cm_per_s(energy);
            if v > 0.0 {
                wd / v
            } else {
                0.0
            }
        }
        // Void segment (no material): only the flux and 1/v scores are
        // defined; reaction rates require Σ_x, so they contribute nothing.
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
/// - `position` — a representative spatial point of the segment \[cm\] for the
///   spatial filters ([`super::filter::MeshFilter`],
///   [`super::filter::SpatialLegendreFilter`]); the caller passes the segment
///   **midpoint** `r + 0.5·d·u`, the track-length-representative point. Ignored by
///   the non-spatial (cell/material/universe/energy) filters.
/// - `macro_xs` — the material's macroscopic cross sections at `energy`, or `None`
///   in a void (then only the flux score deposits).
/// - `weight` — particle statistical weight (1.0 for analog transport).
///
/// # Functional-expansion (Legendre) path
///
/// If the tally carries a *single* expansion filter (a
/// [`super::filter::SpatialLegendreFilter`], detected via
/// [`super::filter::Filter::expansion_moments`]), the segment deposits
/// `w·d·Σ_x · P_n(ξ)` into every moment bin `n` at once (mirroring OpenMC's
/// multi-`(bin, weight)` `get_all_bins`) rather than routing through the single-bin
/// [`filter_bin`] path. See [`super::filter::SpatialLegendreFilter`].
#[allow(clippy::too_many_arguments)]
pub fn score_track_length(
    batch: &mut [f64],
    tally: &Tally,
    cell_idx: usize,
    material_idx: usize,
    universe_idx: usize,
    energy: f64,
    distance: f64,
    position: Position,
    macro_xs: Option<&MacroXs>,
    weight: f64,
    // Instance of `cell_idx` within its repeated universe (GitHub #261).
    // `None` where the caller has no distribcell tables, which is every
    // caller that does not ask for a per-instance tally.
    cell_instance: Option<usize>,
    // **Time since the particle was born \[s\]** (gh:#262, gh:#261), taken at
    // the START of the segment being scored. `TimeFilter` bins on it.
    //
    // Before the transport loop carried a clock this was hardcoded `0.0` here,
    // so a `TimeFilter` put every event in whichever bin contains zero — a
    // filter that compiled, ran, and measured nothing. Passing it explicitly
    // rather than defaulting it means a future caller has to decide, instead of
    // silently inheriting a wrong zero.
    time: f64,
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
        position,
        cell_instance,
        // **Time is now threaded** (gh:#262). ~~The track-length estimator's
        // caller does not yet thread the angle, time or particle type
        // through.~~ **CORRECTED 2026-09-24** — `time` is a parameter and the
        // transport loop supplies a real clock; the note below still holds for
        // `mu` and `particle`.
        time,
        // The angle and particle type are still not threaded.
        // `..Default::default()` records that honestly: an angular or particle
        // filter on a track-length tally would bin every event identically
        // rather than silently producing plausible-looking structure.
        // `filter_bin` is where that would be caught if it mattered -- see the
        // note on `FilterEvent::default`.
        ..Default::default()
    };

    // Functional-expansion path: a lone expansion filter (SpatialLegendreFilter)
    // deposits w·d·Σ_x·P_n(ξ) into every moment bin n simultaneously, the faithful
    // analogue of OpenMC's multi-(bin, weight) get_all_bins
    // (`src/tallies/filter_sptl_legendre.cpp:63`). Only supported as the sole
    // filter (documented gap op-6tz.14).
    if tally.filters.len() == 1 {
        if let Some(moments) = tally.filters[0].expansion_moments(&ev) {
            let n_scores = tally.scores.len();
            for (s_idx, score) in tally.scores.iter().enumerate() {
                let base = track_length_value(score, distance, macro_xs, weight, energy);
                if !base.is_finite() {
                    continue;
                }
                for (n, &pn) in moments.iter().enumerate() {
                    let val = base * pn;
                    if val.is_finite() {
                        batch[n * n_scores + s_idx] += val;
                    }
                }
            }
            return;
        }
    }

    let Some(bin) = filter_bin(tally, &ev) else {
        return;
    };
    let n_scores = tally.scores.len();
    for (s_idx, score) in tally.scores.iter().enumerate() {
        let val = track_length_value(score, distance, macro_xs, weight, energy);
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

/// Score one **fission-born neutron** into a per-batch accumulator, carrying
/// both the energy that caused the fission and the energy the neutron was born
/// with.
///
/// This is how the fission spectrum `chi_g` is measured rather than assumed. A
/// tally holding an [`super::filter::EnergyFilter`] and an
/// [`super::filter::EnergyOutFilter`] bins `(g_causing, g_born)`; summing over
/// the incoming axis and normalising to unity gives `chi_g`. Keeping the
/// incoming axis rather than collapsing it immediately means the caller can
/// also see the (weak) dependence of the emission spectrum on the causing
/// energy, which is real and which a single `chi` vector averages away.
///
/// # What it scores
///
/// Only [`ScoreType::NuFission`] receives weight — one unit per neutron
/// actually banked, so the bin is a neutron count and not a reaction rate.
/// Every other score is left at zero for the same reason as in
/// [`score_scatter_matrix`]: depositing into them would fabricate reaction
/// rates that did not occur at this event.
///
/// # Opt-in
///
/// Like [`score_scatter_matrix`], this fires only for a tally that carries an
/// outgoing-energy filter. Without that guard a plain `[Energy]` tally scoring
/// `NuFission` would receive both the track-length production rate and these
/// birth counts, which are different quantities in different units.
pub fn score_fission_birth(
    batch: &mut [f64],
    tally: &Tally,
    cell_idx: usize,
    material_idx: usize,
    universe_idx: usize,
    energy_in: f64,
    energy_born: f64,
    position: Position,
    weight: f64,
) {
    if !energy_in.is_finite() || !energy_born.is_finite() || energy_born < 0.0 {
        return;
    }
    if !tally
        .filters
        .iter()
        .any(|f| matches!(f, FilterKind::EnergyOut(_)))
    {
        return;
    }
    let ev = FilterEvent {
        cell_idx,
        material_idx,
        universe_idx,
        energy: energy_in,
        energy_out: Some(energy_born),
        surface_idx: usize::MAX,
        position,
        ..Default::default()
    };
    let Some(bin) = filter_bin(tally, &ev) else {
        return;
    };
    let n_scores = tally.scores.len();
    for (s_idx, score) in tally.scores.iter().enumerate() {
        let val = match score {
            ScoreType::NuFission => weight,
            _ => 0.0,
        };
        if val != 0.0 && val.is_finite() {
            batch[bin * n_scores + s_idx] += val;
        }
    }
}

/// Score one **scattering event** into a per-batch accumulator, carrying both
/// the incoming and the outgoing energy.
///
/// This is the analog estimator behind a group-to-group scattering matrix. The
/// caller supplies the energy the neutron arrived with and the energy the
/// scatter kernel actually produced, so a tally holding an
/// [`super::filter::EnergyFilter`] *and* an [`super::filter::EnergyOutFilter`]
/// bins the pair `(g_in, g_out)` — one element of `Sigma_s,g->g'`. That matrix
/// is what the deterministic solvers consume (GeN-Foam's `ZoneNuclearData`
/// stores `scattering[moment][g_out][g_in]`).
///
/// # What it scores, and what it deliberately does not
///
/// Only [`ScoreType::ScatterN`] and [`ScoreType::Events`] receive the event's
/// weight. Every other score is left at zero, because they are meaningless on a
/// scattering event: there is no fission, no absorption and no track length
/// here, and depositing `weight` into them would fabricate reaction rates that
/// did not occur. In particular [`ScoreType::Flux`] is **not** scored — flux is
/// a track-length quantity and is already accumulated by
/// [`score_track_length`]; scoring it again here would double-count.
///
/// # Normalisation — this returns a RATE, not a cross section
///
/// The accumulated bin is the scatter reaction *rate* per source particle for
/// the pair `(g_in, g_out)`. To obtain the macroscopic cross section
/// `Sigma_s,g->g'` the caller divides by the group-`g` scalar flux from a
/// companion track-length flux tally over the same spatial filter. This
/// function does not do that division, because the flux tally is a separate
/// accumulator and combining them is the MGXS layer's job.
///
/// # Parameters
/// - `batch` — flat per-generation accumulator, `tally.bins.len()` long.
/// - `tally` — the tally *definition* (filters + scores); bins are untouched.
/// - `cell_idx` / `material_idx` / `universe_idx` — leaf geometry indices of the
///   collision site.
/// - `energy_in` — energy the neutron arrived with \[eV\].
/// - `energy_out` — energy the scatter kernel produced \[eV\]. May be *higher*
///   than `energy_in`: thermal up-scatter is real and both the free-gas and
///   S(alpha,beta) kernels produce it.
/// - `position` — collision site, for spatial filters.
/// - `weight` — particle statistical weight (1.0 for analog transport).
pub fn score_scatter_matrix(
    batch: &mut [f64],
    tally: &Tally,
    cell_idx: usize,
    material_idx: usize,
    universe_idx: usize,
    energy_in: f64,
    energy_out: f64,
    position: Position,
    weight: f64,
) {
    if !energy_in.is_finite() || !energy_out.is_finite() || energy_out < 0.0 {
        return;
    }
    // OPT-IN: this estimator only fires for a tally that actually carries an
    // outgoing-energy filter, i.e. one asking for a scattering matrix.
    //
    // Without this guard an existing tally scoring `ScatterN` behind a plain
    // `EnergyFilter` would receive BOTH the track-length scatter rate from
    // `score_track_length` and these analog events, silently double-counting
    // into the same bin. The presence of an `EnergyOutFilter` is what
    // distinguishes "I want the matrix" from "I want the group total", so it is
    // the gate.
    if !tally
        .filters
        .iter()
        .any(|f| matches!(f, FilterKind::EnergyOut(_)))
    {
        return;
    }
    let ev = FilterEvent {
        cell_idx,
        material_idx,
        universe_idx,
        energy: energy_in,
        energy_out: Some(energy_out),
        surface_idx: usize::MAX,
        position,
        ..Default::default()
    };

    let Some(bin) = filter_bin(tally, &ev) else {
        return;
    };
    let n_scores = tally.scores.len();
    for (s_idx, score) in tally.scores.iter().enumerate() {
        let val = match score {
            ScoreType::ScatterN | ScoreType::Events => weight,
            _ => 0.0,
        };
        if val != 0.0 && val.is_finite() {
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
    flush_bins(&mut tally.bins, batch);
}

/// Flush a per-batch flat accumulator into a bare slice of persistent
/// [`super::tally::TallyBin`]s as one Monte-Carlo realization, then zero the
/// accumulator.
///
/// This is the estimator-agnostic core of [`flush_batch`], split out so the same
/// one-realization-per-batch statistics apply to accumulators that are not part
/// of a [`Tally`] — e.g. the per-energy-group leakage spectrum the CSG
/// eigenvalue driver accumulates alongside its tally
/// ([`crate::physics::transport_csg::run_keff_csg_reactor_physics`]). `batch`
/// and `bins` must be the same length.
pub fn flush_bins(bins: &mut [TallyBin], batch: &mut [f64]) {
    for (dst, src) in bins.iter_mut().zip(batch.iter_mut()) {
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
    // See `score_track_length`.
    cell_instance: Option<usize>,
) {
    if sigma_t <= 0.0 {
        return;
    }
    let ev = FilterEvent {
        cell_instance,
        cell_idx,
        material_idx,
        universe_idx,
        energy,
        surface_idx: usize::MAX,
        // The collision estimator has no spatial-filter callers yet; a mesh /
        // Legendre tally uses the track-length estimator. Score at the origin.
        position: Position::ZERO,
        ..Default::default()
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
            // Kappa-fission collision estimator: `w·(Σ_f/Σ_t)·Q` — the fission
            // reaction-rate estimate scaled by the recoverable energy per fission
            // `Q` [J] (`src/tallies/tally_scoring.cpp:1480`, SCORE_KAPPA_FISSION).
            ScoreType::KappaFission => weight * macro_xs.fission / sigma_t * Q_FISSION_J,
            // Real absorption rate: `w·(Σ_a/Σ_t)` with Σ_a = capture + fission
            // (`MacroXs::absorption`) — the OpenMC MT=27 quantity
            // (`src/nuclide.cpp:409-417`), not `Σ_t − Σ_elastic`.
            ScoreType::Absorption => weight * macro_xs.absorption.max(0.0) / sigma_t,
            ScoreType::ScatterN => weight * macro_xs.elastic / sigma_t,
            // ── GitHub #262 ─────────────────────────────────────────────────
            ScoreType::Scatter | ScoreType::NuScatter => {
                // See the track-length arm: `NuScatter` is deliberately
                // identical here and is a known undercount.
                weight * (macro_xs.total - macro_xs.absorption).max(0.0) / sigma_t
            }
            ScoreType::DelayedNuFission => weight * macro_xs.nu_fission_delayed / sigma_t,
            ScoreType::PromptNuFission => {
                weight * (macro_xs.nu_fission - macro_xs.nu_fission_delayed).max(0.0) / sigma_t
            }
            ScoreType::DecayRate => weight * macro_xs.decay_rate / sigma_t,
            ScoreType::FissionQPrompt => {
                weight * macro_xs.fission / sigma_t * Q_FISSION_PROMPT_J
            }
            ScoreType::FissionQRecoverable => {
                weight * macro_xs.fission / sigma_t * Q_FISSION_RECOVERABLE_J
            }
            // `tally_scoring.cpp:1161`: the collision estimator for 1/v is
            // `flux / (Sigma_t * v)`, i.e. the same `w/Sigma_t` flux estimate
            // divided by the speed.
            ScoreType::InverseVelocity => {
                let v = neutron_speed_cm_per_s(energy);
                if v > 0.0 {
                    weight / (sigma_t * v)
                } else {
                    0.0
                }
            }
            ScoreType::Current | ScoreType::Events => weight,
        };
        tally.bins[bin * n_scores + s_idx].score(val);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tally::filter::{CellFilter, Filter, FilterKind};
    use crate::tally::tally::{Tally, TallyBin};

    fn cell_flux_tally(cells: Vec<usize>) -> Tally {
        let filter = CellFilter {
            cell_indices: cells,
        };
        let n = filter.n_bins();
        Tally {
            id: 1,
            name: "flux".into(),
            filters: vec![FilterKind::Cell(filter)],
            scores: vec![ScoreType::Flux],
            bins: vec![TallyBin::default(); n],
        }
    }

    /// A collision in a filtered cell scores `1/Σ_t`; a collision in an
    /// unfiltered cell scores nothing.
    #[test]
    fn cell_flux_collision_estimator() {
        let mut t = cell_flux_tally(vec![0, 1]);
        let xs = MacroXs {
            total: 0.5,
            elastic: 0.4,
            fission: 0.05,
            nu_fission: 0.12,
            absorption: 0.08,
            nu_fission_delayed: 0.0,
            decay_rate: 0.0,
        };
        // Two collisions in cell 0 (Σ_t = 0.5 ⇒ 2 cm each), one in cell 5 (ignored).
        score_collision(&mut t, 0, 0, 0, 1.0e6, 0.5, &xs, 1.0, None);
        score_collision(&mut t, 0, 0, 0, 1.0e6, 0.5, &xs, 1.0, None);
        score_collision(&mut t, 5, 0, 0, 1.0e6, 0.5, &xs, 1.0, None);
        assert!(
            (t.bins[0].sum - 4.0).abs() < 1e-12,
            "cell-0 flux sum {}",
            t.bins[0].sum
        );
        assert_eq!(t.bins[0].count, 2);
        assert_eq!(t.bins[1].count, 0, "cell-1 saw no collisions");
    }
}
