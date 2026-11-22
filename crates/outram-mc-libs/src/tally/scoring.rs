//! Tally scoring — accumulate scores at collision events (collision estimator).
//!
//! C++ source: `src/tallies/tally_scoring.cpp`.
//!
//! ~~After a real collision the transport loop (see
//! [`crate::physics::transport_csg`]) calls [`score_collision`]~~ **CORRECTED
//! 2026-11-19** (gh:#693, #679): the transport loop does NOT call
//! [`score_collision`] (it has no non-test caller; the transport scores
//! track-length per batch and, in delta regions, [`score_collision_point`]).
//! It scores each collision straight into the persistent bins, so its
//! statistics are per-collision, not per-batch (gh:#679); use it only for a
//! mean over a known realization count. It:
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
//! the simplest unbiased estimator; ~~a track-length estimator (bead op-6tz.9
//! follow-up) would additionally score along free-flight segments.~~
//! **CORRECTED 2026-10-06:** the track-length estimator exists
//! ([`score_track_length`]) and is what the CSG transport scores on
//! surface-tracked segments. Inside a delta-tracked region the transport
//! scores a collision estimator at a POINT with [`score_collision_point`]
//! (gh:#598), by default at every tentative collision with `1/Σ_maj`.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use super::filter::{FilterEvent, FilterKind};
use super::history_events::{regular_mesh_split, EventHistory};
use super::mesh::MeshKind;
use crate::particle::particle::ParticleType;
use super::tally::{ScoreType, Tally, TallyBin};
use crate::geometry::position::{Direction, Position};
use crate::material::material::{MacroXs, MAX_DELAYED_GROUPS};

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

/// Position of the [`FilterKind::DelayedGroup`] filter on `tally`, if any.
///
/// A tally carrying one resolves its delayed-group quantities through
/// upstream's **analytic** route (`src/tallies/tally_scoring.cpp:705`, `:773`)
/// instead of through [`filter_bin`]: upstream's filter `get_all_bins` puts the
/// event in bin 0 unconditionally and the scoring loop deposits into every
/// group bin itself (`filter_delayedgroup.cpp`: "bins outside zero are tallied
/// to regardless"). GitHub #262.
fn delayed_group_position(tally: &Tally) -> Option<usize> {
    tally
        .filters
        .iter()
        .position(|f| matches!(f, FilterKind::DelayedGroup(_)))
}

/// The flat filter bin of `ev` with the delayed-group filter at `dg_pos`
/// pinned to its bin `d_bin` (upstream's `score_fission_delayed_dg`): the
/// other filters are matched as usual and the delayed-group digit of the
/// row-major index is replaced.
fn filter_bin_with_group(
    tally: &Tally,
    ev: &FilterEvent,
    dg_pos: usize,
    d_bin: usize,
) -> Option<usize> {
    let mut bin = 0usize;
    for (i, f) in tally.filters.iter().enumerate() {
        let b = if i == dg_pos { d_bin } else { f.get_bin(ev)? };
        bin = bin * f.n_bins() + b;
    }
    Some(bin)
}

/// Per-group macroscopic weight of `score` in `x` for the two scores upstream
/// resolves by delayed group analytically (`delayed-nu-fission`,
/// `decay-rate`); `None` for every other score, which upstream refuses with a
/// delayed-group filter (`tally.cpp:569`; [`Tally::validate`] reports it).
fn delayed_group_xs(score: &ScoreType, x: &MacroXs, group: usize) -> Option<f64> {
    match score {
        ScoreType::DelayedNuFission => Some(x.nu_fission_delayed_group[group]),
        ScoreType::DecayRate => Some(x.decay_rate_group[group]),
        _ => None,
    }
}

/// Every `(flat bin·n_scores + score, value)` deposit of one event on a tally
/// with a delayed-group filter at `dg_pos`: `scale · Σ_x,k` per group bin.
/// `scale` is `w·d` (track-length) or `w/Σ_t` (collision). A void
/// (`macro_xs = None`) deposits nothing; non-finite values are dropped as in
/// the ordinary path.
fn delayed_group_deposits(
    tally: &Tally,
    ev: &FilterEvent,
    dg_pos: usize,
    macro_xs: Option<&MacroXs>,
    scale: f64,
) -> Vec<(usize, f64)> {
    let mut out = Vec::new();
    let (Some(x), FilterKind::DelayedGroup(dg)) = (macro_xs, &tally.filters[dg_pos]) else {
        return out;
    };
    let n_scores = tally.scores.len();
    for (d_bin, &group) in dg.groups.iter().enumerate() {
        if group >= MAX_DELAYED_GROUPS {
            continue;
        }
        let Some(bin) = filter_bin_with_group(tally, ev, dg_pos, d_bin) else {
            continue;
        };
        for (s_idx, score) in tally.scores.iter().enumerate() {
            if let Some(xs) = delayed_group_xs(score, x, group) {
                let v = scale * xs;
                if v.is_finite() {
                    out.push((bin * n_scores + s_idx, v));
                }
            }
        }
    }
    out
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
        // `SCORE_NU_SCATTER` is analog-only in CE mode upstream
        // (`tally.cpp:612`: "estimator_ = ANALOG"; `tally_scoring.cpp:1176`:
        // "Only analog estimators are available"). ~~This port scored it
        // identically to `Scatter` here, a known undercount wherever (n,2n)
        // matters.~~ **CORRECTED 2026-11-14 (gh:#262):** the length-type
        // estimators score nothing for it; the analog event hook
        // `score_scatter_matrix` scores each emitted neutron, so the
        // multiplicity is included.
        (ScoreType::NuScatter, _) => 0.0,
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
    // **Flight direction along this segment** (gh:#261). `PolarAzimuthalFilter`
    // bins on it, and before this it was defaulted, so such a filter put every
    // track-length event in one bin — the same defect `time` had.
    direction: Direction,
) {
    score_length_like(
        batch,
        tally,
        cell_idx,
        material_idx,
        universe_idx,
        energy,
        distance,
        position,
        macro_xs,
        weight,
        cell_instance,
        time,
        direction,
        false,
        &EventHistory::default(),
    );
}

/// [`score_track_length`] for a caller that keeps the particle's event history
/// (gh:#261, 2026-11-18): the filter event additionally carries `cell_born`,
/// `position_born` and `n_collision` from `hist`, so a `CellBornFilter`,
/// `MeshBornFilter` or `CollisionFilter` bins the segment as OpenMC does
/// (`filter_cellborn.cpp`, `filter_meshborn.cpp`, `filter_collision.cpp`). The
/// event also carries the particle weight, which a `WeightFilter` reads
/// (`filter_weight.cpp:41`, `wgt_last`, the weight at the start of the flight).
/// With `EventHistory::default()` it is exactly [`score_track_length`].
#[allow(clippy::too_many_arguments)]
pub fn score_track_length_hist(
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
    cell_instance: Option<usize>,
    time: f64,
    direction: Direction,
    hist: &EventHistory,
) {
    score_length_like(
        batch,
        tally,
        cell_idx,
        material_idx,
        universe_idx,
        energy,
        distance,
        position,
        macro_xs,
        weight,
        cell_instance,
        time,
        direction,
        false,
        hist,
    );
}

/// A **collision-estimator** score at one point (gh:#598): deposits
/// `w·inv_sigma` as flux and `w·inv_sigma·Σ_x` as each reaction rate, binned
/// at `position` by EVERY filter, including the spatial ones that split a
/// real track-length segment ([`super::filter::Filter::track_length_bins`]).
///
/// `inv_sigma` is `1/Σ_maj` for the delta-tracking tentative-collision
/// estimator, or `1/Σ_t` for the real-collision one. Both are weights, not
/// lengths: scoring them through [`score_track_length`] on an unstructured
/// mesh filter reconstructed a segment of that "length" around the site and
/// split it across the cells the segment crossed, which was the gh:#598
/// defect.
///
/// Per-batch accumulation, `time` and `direction` as in
/// [`score_track_length`] (`time` is the clock AT the site).
#[allow(clippy::too_many_arguments)]
pub fn score_collision_point(
    batch: &mut [f64],
    tally: &Tally,
    cell_idx: usize,
    material_idx: usize,
    universe_idx: usize,
    energy: f64,
    inv_sigma: f64,
    position: Position,
    macro_xs: Option<&MacroXs>,
    weight: f64,
    cell_instance: Option<usize>,
    time: f64,
    direction: Direction,
) {
    score_collision_point_hist(
        batch,
        tally,
        cell_idx,
        material_idx,
        universe_idx,
        energy,
        inv_sigma,
        position,
        macro_xs,
        weight,
        cell_instance,
        time,
        direction,
        &EventHistory::default(),
    );
}

/// [`score_collision_point`] with the history, so a cell or universe filter sees
/// every coordinate level of the site's path (`hist.set_levels`, gh:#677), as
/// OpenMC emits one bin per matching level. `hist` is the only difference.
#[allow(clippy::too_many_arguments)]
pub fn score_collision_point_hist(
    batch: &mut [f64],
    tally: &Tally,
    cell_idx: usize,
    material_idx: usize,
    universe_idx: usize,
    energy: f64,
    inv_sigma: f64,
    position: Position,
    macro_xs: Option<&MacroXs>,
    weight: f64,
    cell_instance: Option<usize>,
    time: f64,
    direction: Direction,
    hist: &EventHistory,
) {
    score_length_like(
        batch,
        tally,
        cell_idx,
        material_idx,
        universe_idx,
        energy,
        inv_sigma,
        position,
        macro_xs,
        weight,
        cell_instance,
        time,
        direction,
        true,
        hist,
    );
}

/// Shared body of [`score_track_length`] (`at_point = false`) and
/// [`score_collision_point`] (`at_point = true`).
#[allow(clippy::too_many_arguments)]
fn score_length_like(
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
    cell_instance: Option<usize>,
    time: f64,
    direction: Direction,
    at_point: bool,
    hist: &EventHistory,
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
        // ~~The track-length estimator's caller does not yet thread the angle,
        // time or particle type through.~~ **CORRECTED 2026-09-24 (gh:#262,
        // gh:#261)** — `time` and `direction` are now parameters, and
        // `particle` is set below. Only `mu` is still defaulted, and that one is
        // a deliberate distinction rather than an omission; see its note.
        time,
        direction,
        // Event-history fields (gh:#261, 2026-11-18): from the caller's
        // `EventHistory` (the defaults, equal to `FilterEvent::default()`, for
        // a caller that keeps none), and the particle weight, which was left at
        // the default 1.0 and so made a `WeightFilter` see every event at unit
        // weight.
        cell_levels: core::array::from_fn(|l| hist.levels[l].0),
        universe_levels: core::array::from_fn(|l| hist.levels[l].1),
        n_levels: hist.n_levels,
        cell_born: hist.cell_born,
        position_born: hist.position_born,
        n_collision: hist.n_collision,
        weight,
        // This crate transports neutrons only (`ParticleType` reserves the
        // other slots; `physics::photon` is out of scope). Set explicitly so a
        // `ParticleFilter` on a track-length tally bins correctly instead of
        // inheriting a default, and so the day a second particle type exists
        // this line is a compile-time reminder rather than a silent
        // mislabelling.
        particle: ParticleType::Neutron,
        // **`mu` stays defaulted, and that is correct.** It is the
        // change-of-direction cosine of a SCATTERING event — a collision
        // quantity. A flight segment has a direction (threaded above) but no
        // scattering cosine, so there is nothing to thread. ~~`MuFilter` belongs
        // on the collision estimator, where it is supplied.~~ **CORRECTED
        // 2026-11-19** (gh:#676): only the scatter-matrix estimator supplies
        // `mu` (`score_scatter_matrix_mu`); a track-length tally has none, so
        // `Tally::validate` refuses a `MuFilter` there. Defaulting an
        // undefined quantity is not the same failure as defaulting an available
        // one, which is what `time` and `direction` were.
        ..Default::default()
    };

    // Delayed-group path (gh:#262): `delayed-nu-fission` and `decay-rate`
    // are deposited into every group bin analytically, as upstream does
    // (`tally_scoring.cpp:705`, `:773`). No other score is defined with this
    // filter ([`Tally::validate`]), so this path scores nothing else.
    if let Some(dg_pos) = delayed_group_position(tally) {
        for (i, v) in delayed_group_deposits(tally, &ev, dg_pos, macro_xs, weight * distance) {
            batch[i] += v;
        }
        return;
    }

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

    // Track-length splitting (GitHub #492): a mesh filter on an unstructured
    // mesh divides the segment among the cells it crosses, as OpenMC's
    // MeshFilter::get_all_bins does under the track-length estimator
    // (`src/tallies/filter_mesh.cpp:60-69` -> `MOABMesh::bins_crossed`).
    //
    // ONLY for a real segment. A collision-estimator score (`at_point`) has no
    // segment: its "length" `1/Σ` is a weight, and splitting it as if it were
    // a flight centred on the collision site smeared each score along the
    // ray and dropped whatever fell outside the mesh — for a helium collision
    // (`1/Σ_t ~ 5e4 cm`) essentially all of it (gh:#598).
    if !at_point
        && tally.filters.iter().any(|f| f.splits_track_length() || splits_mesh_material(f))
    {
        score_track_length_split(batch, tally, &ev, distance, macro_xs, weight, energy);
        return;
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

/// A mesh-material filter on a regular mesh, which splits a segment among the
/// mesh elements (gh:#261).
fn splits_mesh_material(f: &FilterKind) -> bool {
    matches!(f, FilterKind::MeshMaterial(m) if matches!(m.mesh, MeshKind::Regular(_)))
}

/// The track-length branch for a tally holding a filter that splits segments
/// ([`super::filter::FilterKind::splits_track_length`]).
///
/// The segment is rebuilt from its midpoint `ev.position`, its direction and
/// `distance` (`r0 = mid - d/2 u`, `r1 = mid + d/2 u`; the free flight is
/// straight). Each filter yields a list of `(bin, weight)`: a splitting filter
/// its length fractions, every other filter its single midpoint bin with
/// weight 1. The tally bin of each combination receives the score times the
/// product of its weights — OpenMC's `FilterBinIter` over the per-filter
/// `(bins_, weights_)` matches (`src/tallies/filter.cpp`, `tally_scoring.cpp`).
/// The weights of a split sum to the fraction of the segment inside the mesh,
/// so a segment wholly inside scores exactly what the unsplit path would.
#[allow(clippy::too_many_arguments)]
fn score_track_length_split(
    batch: &mut [f64],
    tally: &Tally,
    ev: &FilterEvent,
    distance: f64,
    macro_xs: Option<&MacroXs>,
    weight: f64,
    energy: f64,
) {
    let u = ev.direction;
    let h = 0.5 * distance;
    let r0 = Position::new(ev.position.x - h * u.u, ev.position.y - h * u.v, ev.position.z - h * u.w);
    let r1 = Position::new(ev.position.x + h * u.u, ev.position.y + h * u.v, ev.position.z + h * u.w);

    let mut per_filter: Vec<Vec<(usize, f64)>> = Vec::with_capacity(tally.filters.len());
    for f in &tally.filters {
        // MESH_MATERIAL on a regular mesh splits the segment among the mesh
        // elements it crosses, inside the one material it flew through, as
        // OpenMC's `MeshMaterialFilter::get_all_bins` does
        // (`filter_meshmaterial.cpp`); the midpoint rule of `get_bin` credited
        // the whole segment to one element (gh:#261, 2026-11-18).
        if let FilterKind::MeshMaterial(mm) = f {
            if let MeshKind::Regular(mesh) = &mm.mesh {
                let bins: Vec<(usize, f64)> = regular_mesh_split(mesh, r0, r1)
                    .into_iter()
                    .filter_map(|(elem, frac)| {
                        mm.pairs
                            .iter()
                            .position(|&(e, m)| e == elem && m == ev.material_idx)
                            .map(|b| (b, frac))
                    })
                    .collect();
                if bins.is_empty() {
                    return;
                }
                per_filter.push(bins);
                continue;
            }
        }
        let bins = match f.track_length_bins(r0, r1) {
            Some(b) => b,
            None => match f.get_bin(ev) {
                Some(b) => vec![(b, 1.0)],
                None => return,
            },
        };
        if bins.is_empty() {
            return;
        }
        per_filter.push(bins);
    }
    let n_scores = tally.scores.len();
    let base: Vec<f64> = tally
        .scores
        .iter()
        .map(|s| track_length_value(s, distance, macro_xs, weight, energy))
        .collect();
    // Odometer over the Cartesian product of the per-filter lists.
    let mut idx = vec![0usize; per_filter.len()];
    loop {
        let mut bin = 0usize;
        let mut w = 1.0;
        for (k, f) in tally.filters.iter().enumerate() {
            let (b, wk) = per_filter[k][idx[k]];
            bin = bin * f.n_bins() + b;
            w *= wk;
        }
        for (s_idx, &v) in base.iter().enumerate() {
            let val = v * w;
            if val.is_finite() {
                batch[bin * n_scores + s_idx] += val;
            }
        }
        let mut k = per_filter.len();
        loop {
            if k == 0 {
                return;
            }
            k -= 1;
            idx[k] += 1;
            if idx[k] < per_filter[k].len() {
                break;
            }
            idx[k] = 0;
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
    score_fission_birth_hist(
        batch, tally, cell_idx, material_idx, universe_idx, energy_in, energy_born, position,
        weight, &EventHistory::default(),
    );
}

/// [`score_fission_birth`] with the particle's [`EventHistory`], so cell and
/// universe filters match every coordinate level (gh:#677).
#[allow(clippy::too_many_arguments)]
pub fn score_fission_birth_hist(
    batch: &mut [f64],
    tally: &Tally,
    cell_idx: usize,
    material_idx: usize,
    universe_idx: usize,
    energy_in: f64,
    energy_born: f64,
    position: Position,
    weight: f64,
    hist: &EventHistory,
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
        cell_levels: core::array::from_fn(|l| hist.levels[l].0),
        universe_levels: core::array::from_fn(|l| hist.levels[l].1),
        n_levels: hist.n_levels,
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
/// The caller calls this once for **every neutron the collision emits**,
/// saying which one with `emission`:
///
/// - [`ScoreType::ScatterN`] and [`ScoreType::Events`] receive the weight on
///   every call, so `ScatterN` is the **nu**-scatter matrix (an (n,2n)
///   scores twice, each neutron in its own outgoing group).
/// - [`ScoreType::Scatter`] receives it only for
///   [`ScatterEmission::Primary`], once per scattering collision, binned on
///   the primary's outgoing energy (added 2026-10-13, gh:#682): the
///   scatter-only matrix OpenMC's `scatter matrix` is, whose ratio to the
///   nu-scatter matrix is the multiplicity. Its row sum estimates
///   `Sigma_t - Sigma_a`; a collision whose evaluated yield is 0 (U-235's
///   MT=5 below ~100 keV) kills the neutron before it is scored, so the row
///   falls short of `Sigma_t - Sigma_a` by that channel.
///
/// Every other score Every other score is left at zero, because they are meaningless on a
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
/// - `emission` — whether this is the collision's primary neutron or an
///   (n,xn) secondary.
#[allow(clippy::too_many_arguments)]
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
    emission: ScatterEmission,
) {
    score_scatter_matrix_mu(
        batch, tally, cell_idx, material_idx, universe_idx, energy_in, energy_out, position,
        weight, emission, 0.0,
    );
}

/// [`score_scatter_matrix`] with the **laboratory scattering cosine** `mu = u_in
/// . u_out` of the emitted neutron (gh:#676), which a `MuFilter` on the tally
/// bins. [`score_scatter_matrix`] is this with `mu = 0.0`, kept for callers with
/// no direction; the transport arms pass the real cosine.
#[allow(clippy::too_many_arguments)]
pub fn score_scatter_matrix_mu(
    batch: &mut [f64],
    tally: &Tally,
    cell_idx: usize,
    material_idx: usize,
    universe_idx: usize,
    energy_in: f64,
    energy_out: f64,
    position: Position,
    weight: f64,
    emission: ScatterEmission,
    mu: f64,
) {
    score_scatter_matrix_full(
        batch, tally, cell_idx, material_idx, universe_idx, energy_in, energy_out, position,
        weight, emission, mu, &EventHistory::default(),
    );
}

/// [`score_scatter_matrix_mu`] with the particle's [`EventHistory`], so a
/// `CellFilter` / `UniverseFilter` matches every coordinate level of the path on
/// the scatter matrix too (gh:#677), not only the leaf.
#[allow(clippy::too_many_arguments)]
pub fn score_scatter_matrix_full(
    batch: &mut [f64],
    tally: &Tally,
    cell_idx: usize,
    material_idx: usize,
    universe_idx: usize,
    energy_in: f64,
    energy_out: f64,
    position: Position,
    weight: f64,
    emission: ScatterEmission,
    mu: f64,
    hist: &EventHistory,
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
    let matrix = tally
        .filters
        .iter()
        .any(|f| matches!(f, FilterKind::EnergyOut(_)));
    // `NuScatter` is analog-only in CE mode upstream (`tally.cpp:612` forces
    // the analog estimator; `tally_scoring.cpp:1176` scores only on a scatter
    // event), so it fires here with or without an outgoing-energy filter.
    // Each emitted neutron carries `weight`, so an (n,2n) scores twice,
    // which is upstream's `(wgt_last - wgt_absorb) * yield`. GitHub #262.
    let has_nu_scatter = tally.scores.iter().any(|s| matches!(s, ScoreType::NuScatter));
    if !matrix && !has_nu_scatter {
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
        mu,
        cell_levels: core::array::from_fn(|l| hist.levels[l].0),
        universe_levels: core::array::from_fn(|l| hist.levels[l].1),
        n_levels: hist.n_levels,
        ..Default::default()
    };

    let n_scores = tally.scores.len();
    let value_of = |score: &ScoreType| match (score, emission) {
        (ScoreType::NuScatter, _) => weight,
        _ if !matrix => 0.0,
        (ScoreType::ScatterN | ScoreType::Events, _) => weight,
        (ScoreType::Scatter, ScatterEmission::Primary) => weight,
        _ => 0.0,
    };
    // Functional expansion in the scattering cosine (gh:#676): a
    // `LegendreFilter` deposits `value * P_l(mu)` into EVERY moment bin `l`, the
    // analogue of OpenMC's `LegendreFilter::get_all_bins` (`calc_pn_c(order,
    // p.mu())`, `filter_legendre.cpp`). The other filters are matched as usual
    // with the Legendre digit of the row-major index pinned to `l`.
    if let Some(pos) = tally.filters.iter().position(|f| matches!(f, FilterKind::Legendre(_))) {
        let Some(moments) = tally.filters[pos].expansion_moments(&ev) else {
            return;
        };
        for (l, &pl) in moments.iter().enumerate() {
            let Some(bin) = filter_bin_with_group(tally, &ev, pos, l) else {
                return;
            };
            for (s_idx, score) in tally.scores.iter().enumerate() {
                let val = value_of(score) * pl;
                if val != 0.0 && val.is_finite() {
                    batch[bin * n_scores + s_idx] += val;
                }
            }
        }
        return;
    }
    let Some(bin) = filter_bin(tally, &ev) else {
        return;
    };
    for (s_idx, score) in tally.scores.iter().enumerate() {
        let val = value_of(score);
        if val != 0.0 && val.is_finite() {
            batch[bin * n_scores + s_idx] += val;
        }
    }
}

/// Which neutron of a scattering collision [`score_scatter_matrix`] is
/// scoring: the one that continues (`Primary`, one per collision) or an extra
/// neutron of an (n,2n), (n,3n), MT=5 or other (n,xn) reaction (`Secondary`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScatterEmission {
    /// The colliding neutron's own outgoing state: one per collision.
    Primary,
    /// An extra neutron emitted by the same collision.
    Secondary,
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
///
/// **Per-batch since 2026-11-20 (gh:#679).** ~~This scored every collision
/// straight into the persistent bins, so each COLLISION was one "realization"
/// and its `rel_std_dev` was not the variance of batch totals; it also built its
/// [`FilterEvent`] without position, direction, time or `mu`.~~ It now
/// accumulates into the caller's per-batch accumulator (`batch`, flushed once per
/// batch with [`flush_batch`]) by delegating to [`score_collision_point`], so the
/// statistics are those of batch totals. It has no position or direction: it
/// scores at the origin along the default direction, so a spatial or angular
/// filter needs [`score_collision_point`] directly.
#[allow(clippy::too_many_arguments)]
pub fn score_collision(
    batch: &mut [f64],
    tally: &Tally,
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
    score_collision_point(
        batch,
        tally,
        cell_idx,
        material_idx,
        universe_idx,
        energy,
        1.0 / sigma_t,
        Position::ZERO,
        Some(macro_xs),
        weight,
        cell_instance,
        0.0,
        Direction::new(0.0, 0.0, 1.0),
    );
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
            bins: vec![TallyBin::default(); n], derivative: None,
        }
    }

    /// An (n,2n) scores twice into the nu-scatter matrix (`ScatterN`, each
    /// neutron in its own outgoing group) and once into the scatter-only
    /// matrix (`Scatter`, on the primary's outgoing group), gh:#682. With
    /// two groups split at 1 MeV: a 10 MeV collision whose primary leaves at
    /// 2 MeV and whose secondary leaves at 0.5 MeV must put
    /// `ScatterN (1->1) = 1`, `ScatterN (1->0) = 1`, `Scatter (1->1) = 1`,
    /// `Scatter (1->0) = 0`. Without an `EnergyOutFilter` nothing is scored.
    #[test]
    fn scatter_matrix_scores_nu_scatter_per_neutron_and_scatter_per_collision() {
        use crate::tally::filter::{EnergyFilter, EnergyOutFilter};
        let edges = vec![1.0e-5, 1.0e6, 2.0e7];
        let t = Tally {
            id: 1,
            name: "m".into(),
            filters: vec![
                FilterKind::Energy(EnergyFilter { bins: edges.clone() }),
                FilterKind::EnergyOut(EnergyOutFilter { bins: edges }),
            ],
            scores: vec![ScoreType::ScatterN, ScoreType::Scatter],
            bins: vec![TallyBin::default(); 2 * 2 * 2], derivative: None,
        };
        let mut batch = vec![0.0; t.bins.len()];
        let r = Position::new(0.0, 0.0, 0.0);
        score_scatter_matrix(&mut batch, &t, 0, 0, 0, 1.0e7, 2.0e6, r, 1.0, ScatterEmission::Primary);
        score_scatter_matrix(&mut batch, &t, 0, 0, 0, 1.0e7, 5.0e5, r, 1.0, ScatterEmission::Secondary);
        // bin = (g_in * 2 + g_out) * 2 + score
        let at = |gi: usize, go: usize, s: usize| batch[(gi * 2 + go) * 2 + s];
        assert_eq!((at(1, 1, 0), at(1, 0, 0)), (1.0, 1.0), "nu-scatter: both neutrons");
        assert_eq!((at(1, 1, 1), at(1, 0, 1)), (1.0, 0.0), "scatter: the collision once");
        let plain = Tally {
            filters: vec![t.filters[0].clone()],
            bins: vec![TallyBin::default(); 2 * 2],
            ..t.clone()
        };
        let mut b2 = vec![0.0; plain.bins.len()];
        score_scatter_matrix(&mut b2, &plain, 0, 0, 0, 1.0e7, 2.0e6, r, 1.0, ScatterEmission::Primary);
        assert!(b2.iter().all(|v| *v == 0.0), "opt-in: no EnergyOutFilter, no score");
    }

    /// gh:#676: a `[EnergyOut, Legendre(2)]`-style tally gets `w * P_l(mu)` in
    /// every moment bin from the scatter matrix. mu = 0.5: P0 = 1, P1 = 0.5,
    /// P2 = (3*0.25 - 1)/2 = -0.125; weight 2 doubles each.
    #[test]
    fn the_scatter_matrix_expands_in_legendre_moments_of_the_lab_cosine() {
        use crate::tally::filter::{EnergyFilter, EnergyOutFilter};
        use crate::tally::filter_extra::LegendreFilter;
        let edges = vec![1.0e-5, 1.0e6, 2.0e7];
        let t = Tally {
            id: 3,
            name: "p_l".into(),
            filters: vec![
                FilterKind::Energy(EnergyFilter { bins: edges.clone() }),
                FilterKind::EnergyOut(EnergyOutFilter { bins: edges }),
                FilterKind::Legendre(LegendreFilter { order: 2 }),
            ],
            scores: vec![ScoreType::NuScatter],
            bins: vec![TallyBin::default(); 2 * 2 * 3], derivative: None,
        };
        assert!(t.validate().is_ok());
        let r = Position::new(0.0, 0.0, 0.0);
        let mut batch = vec![0.0; t.bins.len()];
        // g_in = 1 (10 MeV), g_out = 0 (0.5 MeV): flat bin = (1*2 + 0)*3 + l.
        score_scatter_matrix_mu(&mut batch, &t, 0, 0, 0, 1.0e7, 5.0e5, r, 2.0, ScatterEmission::Primary, 0.5);
        let base = (1 * 2 + 0) * 3;
        assert!((batch[base] - 2.0).abs() < 1e-15);
        assert!((batch[base + 1] - 1.0).abs() < 1e-15);
        assert!((batch[base + 2] + 0.25).abs() < 1e-15);
        assert_eq!(batch.iter().filter(|v| **v != 0.0).count(), 3, "only that (g_in, g_out) row");
    }

    /// gh:#676: the scatter matrix bins the lab cosine of each emitted neutron on
    /// a `MuFilter`: `mu = +0.5` lands in the upper bin, `-0.5` in the lower, and
    /// a tally with a `NuScatter` score and no `EnergyOutFilter` still scores.
    #[test]
    fn the_scatter_matrix_bins_the_lab_cosine_on_a_mu_filter() {
        use crate::tally::filter::MuFilter;
        let t = Tally {
            id: 2,
            name: "mu".into(),
            filters: vec![FilterKind::Mu(MuFilter { bounds: vec![-1.0, 0.0, 1.0] })],
            scores: vec![ScoreType::NuScatter],
            bins: vec![TallyBin::default(); 2], derivative: None,
        };
        assert!(t.validate().is_ok());
        let r = Position::new(0.0, 0.0, 0.0);
        let mut batch = vec![0.0; 2];
        score_scatter_matrix_mu(&mut batch, &t, 0, 0, 0, 1.0e6, 5.0e5, r, 1.0, ScatterEmission::Primary, 0.5);
        score_scatter_matrix_mu(&mut batch, &t, 0, 0, 0, 1.0e6, 5.0e5, r, 2.0, ScatterEmission::Primary, -0.5);
        assert_eq!(batch, vec![2.0, 1.0], "mu = -0.5 -> bin 0 (weight 2), mu = +0.5 -> bin 1 (weight 1)");
    }

    /// gh:#677 (scatter matrix and fission birth): a cell filter on a CONTAINER
    /// cell matches the leaf's ancestors on these estimators too when the path
    /// is carried; without it only the leaf matches, as before.
    #[test]
    fn container_cell_filters_match_on_the_scatter_matrix_and_fission_birth() {
        use crate::tally::filter::{EnergyOutFilter, FilterKind};
        use crate::tally::history_events::EventHistory;
        let t = Tally {
            id: 5,
            name: "c".into(),
            filters: vec![
                FilterKind::Cell(CellFilter { cell_indices: vec![5] }),
                FilterKind::EnergyOut(EnergyOutFilter { bins: vec![1.0e-5, 2.0e7] }),
            ],
            scores: vec![ScoreType::NuScatter, ScoreType::NuFission],
            bins: vec![TallyBin::default(); 2], derivative: None,
        };
        let r = Position::new(0.0, 0.0, 0.0);
        let mut hist = EventHistory::default();
        hist.set_levels([(5usize, 0usize), (1, 2)].into_iter());
        let mut b = vec![0.0; 2];
        score_scatter_matrix_full(&mut b, &t, 1, 0, 2, 1.0e6, 5.0e5, r, 1.0, ScatterEmission::Primary, 0.0, &hist);
        assert_eq!(b, vec![1.0, 0.0], "scatter matrix: container cell 5 matches");
        let mut b = vec![0.0; 2];
        score_fission_birth_hist(&mut b, &t, 1, 0, 2, 1.0e6, 2.0e6, r, 1.0, &hist);
        assert_eq!(b, vec![0.0, 1.0], "fission birth: container cell 5 matches");
        let mut b = vec![0.0; 2];
        score_scatter_matrix_mu(&mut b, &t, 1, 0, 2, 1.0e6, 5.0e5, r, 1.0, ScatterEmission::Primary, 0.0);
        assert_eq!(b, vec![0.0, 0.0], "no path carried: leaf only");
    }

    /// gh:#677: a flux tally on a CONTAINER cell (filled with a universe or
    /// lattice) scores the flights of its leaf cells, as OpenMC's `CellFilter`
    /// matches every coordinate level. The leaf is cell 1 inside container cell 5;
    /// before, the tally on `[5]` scored 0. A leaf-only event (no path carried)
    /// keeps the old behaviour, and an unrelated container does not match.
    #[test]
    fn a_cell_filter_on_a_container_cell_scores_its_leaf_flights() {
        use crate::tally::history_events::EventHistory;
        let t = cell_flux_tally(vec![5]);
        let r = Position::new(0.0, 0.0, 0.0);
        let u = Direction::new(1.0, 0.0, 0.0);
        let mut hist = EventHistory::default();
        hist.set_levels([(5usize, 0usize), (usize::MAX, 0), (1, 2)].into_iter());
        let mut batch = vec![0.0; 1];
        score_track_length_hist(&mut batch, &t, 1, 0, 2, 1.0e6, 3.0, r, None, 1.0, None, 0.0, u, &hist);
        assert_eq!(batch[0], 3.0, "container cell 5 sees the 3 cm flight in its leaf");
        let mut b2 = vec![0.0; 1];
        score_track_length_hist(&mut b2, &t, 1, 0, 2, 1.0e6, 3.0, r, None, 1.0, None, 0.0, u, &EventHistory::default());
        assert_eq!(b2[0], 0.0, "no path carried: leaf only, as before");
        let other = cell_flux_tally(vec![9]);
        let mut b3 = vec![0.0; 1];
        score_track_length_hist(&mut b3, &other, 1, 0, 2, 1.0e6, 3.0, r, None, 1.0, None, 0.0, u, &hist);
        assert_eq!(b3[0], 0.0, "an unrelated cell does not match");
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
            ..Default::default()
        };
        // Two collisions in cell 0 (Σ_t = 0.5 ⇒ 2 cm each), one in cell 5 (ignored).
        // One batch: gh:#679, a batch is ONE realization, not one per collision.
        let mut batch = vec![0.0; t.bins.len()];
        score_collision(&mut batch, &t, 0, 0, 0, 1.0e6, 0.5, &xs, 1.0, None);
        score_collision(&mut batch, &t, 0, 0, 0, 1.0e6, 0.5, &xs, 1.0, None);
        score_collision(&mut batch, &t, 5, 0, 0, 1.0e6, 0.5, &xs, 1.0, None);
        flush_batch(&mut t, &mut batch);
        assert!(
            (t.bins[0].sum - 4.0).abs() < 1e-12,
            "cell-0 flux sum {}",
            t.bins[0].sum
        );
        assert_eq!(t.bins[0].count, 1, "one batch is one realization");
        assert_eq!(t.bins[1].count, 1, "cell-1 saw no collisions: a zero realization");
        assert_eq!(t.bins[1].sum, 0.0);
    }

    /// GitHub #492: a track-length flux tally on an **unstructured** mesh
    /// splits a segment across the cells it crosses. Two unit hexes along x;
    /// a 1.5 cm segment from x = 0.25 to 1.75 puts 0.75 cm in each. A second
    /// filter (energy, unsplit) multiplies through with weight 1.
    ///
    /// Results: NOT YET MEASURED (testing deferred by maintainer, 2026-10-03).
    #[test]
    fn unstructured_mesh_filter_splits_track_length() {
        use crate::tally::filter::{EnergyFilter, MeshFilter};
        use crate::tally::mesh::MeshKind;
        use outram_blender::unstructured::{Element, ElementKind, LengthUnit, UnstructuredMesh};
        use alloc::sync::Arc;

        let mut pts = Vec::new();
        for x in [0.0, 1.0, 2.0] {
            for (y, z) in [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)] {
                pts.push([x, y, z]);
            }
        }
        let el = |i: usize| Element { kind: ElementKind::Hex8, nodes: (0..8).map(|k| 4 * i + k).collect() };
        let mesh =
            UnstructuredMesh::from_elements(LengthUnit::Centimetre, pts, vec![el(0), el(1)], vec![], vec![])
                .unwrap();
        let tally = Tally {
            id: 7,
            name: "umesh flux".into(),
            filters: vec![
                FilterKind::Energy(EnergyFilter { bins: vec![0.0, 2.0e7] }),
                FilterKind::Mesh(MeshFilter { mesh: MeshKind::Unstructured(Arc::new(mesh)) }),
            ],
            scores: vec![ScoreType::Flux],
            bins: vec![TallyBin::default(); 2], derivative: None,
        };
        let mut batch = vec![0.0; 2];
        score_track_length(
            &mut batch,
            &tally,
            0,
            usize::MAX,
            0,
            1.0e6,
            1.5,
            Position::new(1.0, 0.5, 0.5),
            None,
            1.0,
            None,
            0.0,
            Direction::new(1.0, 0.0, 0.0),
        );
        assert!((batch[0] - 0.75).abs() < 1e-12, "cell 0 got {}", batch[0]);
        assert!((batch[1] - 0.75).abs() < 1e-12, "cell 1 got {}", batch[1]);
    }

    fn dg_xs() -> MacroXs {
        let mut x = MacroXs { total: 1.0, fission: 0.2, nu_fission: 0.5, ..Default::default() };
        x.nu_fission_delayed_group[0] = 0.001;
        x.nu_fission_delayed_group[2] = 0.003;
        x.decay_rate_group[0] = 0.001 * 0.0133;
        x.decay_rate_group[2] = 0.003 * 0.1208;
        x.nu_fission_delayed = 0.004;
        x
    }

    fn dg_tally() -> Tally {
        use crate::tally::filter::DelayedGroupFilter;
        Tally {
            id: 9,
            name: "dg".into(),
            filters: vec![
                FilterKind::Cell(CellFilter { cell_indices: vec![0, 1] }),
                FilterKind::DelayedGroup(DelayedGroupFilter::new(vec![2, 0]).unwrap()),
            ],
            scores: vec![ScoreType::DelayedNuFission, ScoreType::DecayRate],
            bins: vec![TallyBin::default(); 2 * 2 * 2], derivative: None,
        }
    }

    /// Track-length delayed-group scoring: `w d Sigma_x,k`, deposited into the
    /// bin of each listed group in the filter's own order (group 2 is bin 0,
    /// group 0 is bin 1 here), in the matched cell only. Expected values are
    /// the hand product of the inputs.
    #[test]
    fn delayed_group_track_length_deposits_per_group() {
        let t = dg_tally();
        t.validate().unwrap();
        let x = dg_xs();
        let mut batch = vec![0.0; t.bins.len()];
        score_track_length(
            &mut batch, &t, 1, 0, 0, 1.0e6, 2.0, Position::ZERO, Some(&x), 0.5, None, 0.0,
            Direction::new(1.0, 0.0, 0.0),
        );
        // layout: ((cell * 2 + dg_bin) * 2 + score); cell 1
        let at = |dg: usize, s: usize| batch[((2 + dg) * 2) + s];
        let wd = 0.5 * 2.0;
        assert!((at(0, 0) - wd * 0.003).abs() < 1e-15);
        assert!((at(1, 0) - wd * 0.001).abs() < 1e-15);
        assert!((at(0, 1) - wd * 0.003 * 0.1208).abs() < 1e-15);
        assert!((at(1, 1) - wd * 0.001 * 0.0133).abs() < 1e-15);
        assert!(batch[..4].iter().all(|v| *v == 0.0), "cell 0 is not hit");
    }

    /// Collision estimator: `w Sigma_x,k / Sigma_t`.
    #[test]
    fn delayed_group_collision_estimator_deposits_per_group() {
        let mut t = dg_tally();
        let mut batch = vec![0.0; t.bins.len()];
        score_collision(&mut batch, &t, 0, 0, 0, 1.0e6, 2.0, &dg_xs(), 4.0, None);
        flush_batch(&mut t, &mut batch);
        assert!((t.bins[0].sum - 4.0 / 2.0 * 0.003).abs() < 1e-15);
        assert!((t.bins[2].sum - 4.0 / 2.0 * 0.001).abs() < 1e-15);
    }

    /// Upstream refuses other scores with a delayed-group filter.
    #[test]
    fn delayed_group_filter_refuses_other_scores_and_bad_groups() {
        use crate::tally::filter::DelayedGroupFilter;
        let mut t = dg_tally();
        t.scores.push(ScoreType::Flux);
        assert!(t.validate().is_err());
        assert!(DelayedGroupFilter::new(vec![]).is_err());
        assert!(DelayedGroupFilter::new(vec![1, 1]).is_err());
        assert!(DelayedGroupFilter::new(vec![MAX_DELAYED_GROUPS]).is_err());
        assert!(DelayedGroupFilter::new(vec![0, 5]).is_ok());
    }

    /// `NuScatter` is analog-only: an (n,2n) (primary plus one secondary)
    /// scores 2 w, with no `EnergyOutFilter` needed, and the length-type
    /// estimators score nothing for it.
    #[test]
    fn nu_scatter_counts_every_emitted_neutron_and_is_analog_only() {
        let t = Tally {
            id: 3,
            name: "nu".into(),
            filters: vec![FilterKind::Cell(CellFilter { cell_indices: vec![0] })],
            scores: vec![ScoreType::NuScatter, ScoreType::ScatterN],
            bins: vec![TallyBin::default(); 2], derivative: None,
        };
        let r = Position::ZERO;
        let mut batch = vec![0.0; 2];
        score_scatter_matrix(&mut batch, &t, 0, 0, 0, 1.0e7, 2.0e6, r, 0.5, ScatterEmission::Primary);
        score_scatter_matrix(&mut batch, &t, 0, 0, 0, 1.0e7, 5.0e5, r, 0.5, ScatterEmission::Secondary);
        assert_eq!(batch[0], 1.0, "two neutrons of weight 0.5");
        assert_eq!(batch[1], 0.0, "ScatterN without EnergyOut stays track-length only");
        let x = MacroXs { total: 1.0, absorption: 0.2, ..Default::default() };
        let mut b2 = vec![0.0; 2];
        score_track_length(
            &mut b2, &t, 0, 0, 0, 1.0e6, 1.0, r, Some(&x), 1.0, None, 0.0, Direction::new(1.0, 0.0, 0.0),
        );
        assert_eq!(b2[0], 0.0);
    }
}
