//! k-eigenvalue power iteration over **general CSG geometry** with surface
//! tracking and boundary conditions.
//!
//! This generalises [`crate::physics::keff::run_keff`] (which hard-codes a single
//! bare sphere) to any [`Geometry`]: an arbitrary set of CSG cells, nested
//! universes and rectangular lattices, with per-surface vacuum / reflective
//! boundary conditions. It is the assembly point for the `pincell` and
//! `rectangular-lattice` verification cases (beads op-6tz.8 / op-6tz.10), built
//! on the geometry navigation of [`Geometry::locate`] and
//! [`Geometry::distance_to_boundary`] (op-6tz.7).
//!
//! # Transport algorithm (surface tracking)
//!
//! Ported in structure from OpenMC `transport_history_based` /
//! `distance_to_boundary` (`src/physics.cpp`, `src/geometry.cpp`). For each
//! history the loop is:
//!
//! 1. [`Geometry::locate`] the particle → leaf material and coordinate chain.
//! 2. Sample distance to collision `d_col = −ln ξ / Σ_t(E)` (∞ in void).
//! 3. [`Geometry::distance_to_boundary`] → nearest surface/lattice crossing `d_b`.
//! 4. If a tally is attached, deposit the **track-length** flux/reaction-rate
//!    contribution for the segment just streamed (`w·d` per bin, at the segment's
//!    cell + energy) into the current batch's accumulator.
//! 5. If `d_col < d_b`: stream to the collision, sample the reaction (the same
//!    analog reaction partition as `keff.rs`), and bank fission neutrons.
//! 6. Else: stream to the boundary and apply the crossing
//!    ([`Geometry::cross_surface`]): reflect off a reflective surface, die at a
//!    vacuum surface, otherwise pass through and re-locate.
//!
//! Fission neutrons feed the next generation's source bank; the generation
//! eigenvalue is `k = (Σ ν̄ over fissions) / n_particles`, averaged over the
//! active generations exactly as in `keff.rs`.
//!
//! # Fidelity
//!
//! Analog transport (weight 1, no implicit capture or variance reduction), same
//! collision physics and data tiers as [`crate::physics::keff`]. Tallies use the
//! **track-length estimator**: each streamed segment of length `d` deposits `w·d`
//! (flux) and `w·d·Σ_x` (reaction rates) into its cell × energy bin, accumulated
//! per generation and flushed as one realization per active batch (see
//! [`crate::tally::scoring::score_track_length`] / `flush_batch`). The
//! collision estimator ([`crate::tally::scoring::score_collision`]) remains
//! available as an alternative.
//!
//! **S(α,β) thermal scattering (bead op-6tz.12).** A moderator nuclide carrying a
//! [`crate::material::thermal::ThermalScattering`] table (attached via
//! [`Nuclide::with_thermal_scattering`]) now thermalizes correctly: below the
//! table's cutoff (~4 eV) the scatter branch samples the bound-atom S(α,β) law
//! ([`Nuclide::sample_thermal`]) — lab-frame outgoing energy and cosine, with the
//! up-scatter that builds a Maxwellian — instead of the free-gas elastic kernel.
//! Nuclides without a table (fuel, O, clad) stay free-gas/CE. This makes a
//! *thermal* LWR pin-cell tractable; see the `pincell` verification test.

use crate::geometry::cell::{SurfaceToken, TrackingMethod};
use crate::pebble_beds::delta_tracking::{bounded_delta_flight, DeltaStep, Majorant};

/// Virtual-collision budget for a delta-tracked region before the history is
/// declared lost. Matches `keff_delta.rs`'s `MAX_VIRTUAL` so the two paths
/// agree on what counts as a stuck history.
const MAX_VIRTUAL_COLLISIONS: u32 = 100_000;
use crate::geometry::geometry::{Crossing, Geometry};
use crate::geometry::position::{stream, Direction, Position};
use crate::material::material::Material;
use crate::material::nuclide::{Inelastic, Nuclide};
use crate::physics::compute::{ComputeType, ThreadCount};
use crate::physics::fission::sample_num_neutrons;
use crate::physics::keff::{KeffResult, KeffSettings};
use crate::physics::scatter::{
    free_gas_elastic_scatter_dbrc, rotate_direction, two_body_scatter, two_body_scatter_with_mu,
};
use crate::geometry::surface::BoundaryType;
use crate::physics::track_output::{TrackEvent, TrackRecorder, TrackState};
use crate::source::extra::{SurfaceCrossing, SurfaceSource};
use crate::physics::weight_windows::{apply as apply_window, WindowOutcome, WindowState};
use crate::physics::variance_reduction::{
    russian_roulette, survival_bias_absorption, VarianceReduction,
};
use crate::rng::distributions::isotropic_direction;
use crate::rng::lcg::{future_seed, prn};
use crate::tally::scoring::{
    flush_batch, flush_bins, score_fission_birth, score_scatter_matrix, score_track_length,
};
use crate::tally::tally::{Tally, TallyBin};
use crate::mathf::RealMath;

/// How the initial fission source is seeded spatially — a box the sampler
/// rejects into the fissile region of the geometry.
///
/// The transport itself needs no source region (fission sites regenerate it);
/// this only bootstraps generation 0. Points are drawn uniformly in the box and
/// kept only if they land in a cell whose material can fission.
#[derive(Debug, Clone, Copy)]
pub struct SourceBox {
    /// Lower corner \[cm\].
    pub lower: Position,
    /// Upper corner \[cm\].
    pub upper: Position,
}

/// A fission-source neutron awaiting transport in the next generation. Also the
/// unit of work for [`crate::physics::fixed_source`], which reuses
/// [`transport_history`].
#[derive(Clone, Copy)]
pub(crate) struct Site {
    pub(crate) r: Position,
    pub(crate) u: Direction,
    pub(crate) e: f64,
}

impl Site {
    /// A source/fission neutron at position `r`, direction `u`, energy `e` \[eV\].
    pub(crate) fn new(r: Position, u: Direction, e: f64) -> Site {
        Site { r, u, e }
    }
}

/// Per-history stride \[RNG draws\] reserved for each history's independent
/// sub-stream in the multi-thread backend — [`crate::rng::lcg::DEFAULT_STRIDE`]
/// (152 917), the same per-particle stride OpenMC reserves (`src/random_lcg.cpp`
/// `init_seed`). Far more draws than any single history consumes, so adjacent
/// histories' streams never overlap.
const HIST_STRIDE: u64 = crate::rng::lcg::DEFAULT_STRIDE;

/// Per-generation stride \[RNG draws\] reserved for each generation in the
/// multi-thread backend — `2^40`, chosen far larger than
/// `n_particles * HIST_STRIDE` for any realistic `n_particles`, so no
/// generation's sub-streams overlap the next generation's.
const GEN_STRIDE: u64 = 1 << 40;

/// Run fission-source power iteration over an arbitrary CSG [`Geometry`].
///
/// # Parameters
/// - `geom` — the CSG model (surfaces/cells/universes/lattices + root universe).
/// - `materials` — global material array the geometry's cells index into.
/// - `nuclides` — global nuclide array the materials index into.
/// - `source_box` — box the initial source is rejection-sampled into (must
///   overlap the fissile region).
/// - `settings` — power-iteration controls (see [`KeffSettings`]).
/// - `tally` — optional track-length tally scored on active generations. Its
///   `bins` are accumulated in place, one realization per active generation.
///
/// Returns the mean eigenvalue and standard error over the active generations
/// (a [`KeffResult`], same type as the bare-sphere driver). If a `tally` is
/// supplied its bins are accumulated in place.
///
/// # Compute backend
///
/// This is a thin **dispatcher** over [`settings.compute`](KeffSettings::compute),
/// mirroring [`crate::physics::keff::run_keff`]. The physics is identical across
/// backends; only the execution strategy differs:
///
/// - [`ComputeType::CpuSingleThread`] → [`run_keff_csg_seq`], the scalar,
///   single-RNG-stream **reference** — deterministic and bit-reproducible for a
///   fixed seed.
/// - [`ComputeType::CpuMultiThread`] → [`run_keff_csg_par`], [`rayon`]-parallel
///   histories per generation, each with an independent jump-ahead RNG stream so
///   the result is reproducible independent of thread count. It does **not**
///   bit-match the single-thread reference but agrees within combined statistical
///   uncertainty.
/// - [`ComputeType::Gpu`] → **no GPU kernel exists for general CSG geometry** (the
///   crate's GPU transport, [`crate::physics::keff::run_keff_gpu`], is bare-sphere
///   only), so this transparently runs the multi-threaded CPU path and emits a
///   `log::debug!` line. It never errors on the selection. Wiring a genuine GPU
///   Sigma_t lookup into CSG/delta transport is tracked as follow-up work
///   (bead op-fla).
/// **Hybrid k-eigenvalue: delta tracking where a region asks for it, surface
/// tracking everywhere else** (`bn:op-867c`, gh #214).
///
/// Identical to [`run_keff_csg`] except that it takes a majorant table.
/// A cell declares `Cell::delta_tracked(i)` to be transported by delta
/// (Woodcock) tracking against `majorants[i]`, and everything else -- including
/// regions nested inside it that declare `surface_tracked()` -- uses ordinary
/// surface tracking.
///
/// # The majorant must bound its region, or the answer is silently wrong
///
/// Build each entry with [`Majorant::over_indices`] over **every** material the
/// region's geometry can present. An under-bound majorant does not crash: delta
/// tracking rejects collisions it should have accepted and returns a biased `k`.
/// Over-bounding only costs time, and [`KeffResult::virtual_collisions`]
/// reports how much.
///
/// # Why scope it at all
///
/// Measured 2026-09-17 (`examples/majorant_absorber_price.rs`): adding one B4C
/// control rod to a globally-bounded material set costs **26.3x** in tracking
/// steps at the thermal peak, and it costs that in reflector graphite metres
/// from the rod as much as inside it. Above ~1 keV it costs nothing.
///
/// Passing an empty table makes this exactly [`run_keff_csg`].
pub fn run_keff_csg_hybrid(
    geom: &Geometry,
    materials: &[Material],
    nuclides: &[Nuclide],
    majorants: &[Majorant],
    entropy_mesh: Option<&crate::tally::mesh::RegularMesh>,
    source_box: SourceBox,
    settings: &KeffSettings,
    tally: Option<&mut Tally>,
) -> KeffResult {
    run_keff_csg_inner(
        geom,
        materials,
        nuclides,
        majorants,
        entropy_mesh,
        source_box,
        settings,
        tally,
    )
}

pub fn run_keff_csg(
    geom: &Geometry,
    materials: &[Material],
    nuclides: &[Nuclide],
    source_box: SourceBox,
    settings: &KeffSettings,
    tally: Option<&mut Tally>,
) -> KeffResult {
    run_keff_csg_inner(
        geom,
        materials,
        nuclides,
        &[],
        None,
        source_box,
        settings,
        tally,
    )
}

fn run_keff_csg_inner(
    geom: &Geometry,
    materials: &[Material],
    nuclides: &[Nuclide],
    majorants: &[Majorant],
    entropy_mesh: Option<&crate::tally::mesh::RegularMesh>,
    source_box: SourceBox,
    settings: &KeffSettings,
    tally: Option<&mut Tally>,
) -> KeffResult {
    match settings.compute {
        ComputeType::CpuSingleThread => run_keff_csg_seq(
            geom,
            materials,
            nuclides,
            majorants,
            entropy_mesh,
            source_box,
            settings,
            tally,
            &[],
            None,
        ),
        ComputeType::CpuMultiThread(tc) => run_keff_csg_par(
            geom,
            materials,
            nuclides,
            majorants,
            entropy_mesh,
            source_box,
            settings,
            tally,
            &[],
            None,
            tc,
        ),
        ComputeType::Gpu => {
            log::debug!(
                "ComputeType::Gpu requested for run_keff_csg, but no GPU kernel exists for \
                 general CSG geometry (GPU transport is bare-sphere only) — running the \
                 multi-threaded CPU path instead"
            );
            run_keff_csg_par(
                geom,
                materials,
                nuclides,
                majorants,
                entropy_mesh,
                source_box,
                settings,
                tally,
                &[],
                None,
                ThreadCount::Auto,
            )
        }
    }
}

/// Like [`run_keff_csg`], but also accumulates a **leakage spectrum** on the
/// energy grid `leak_edges` into `leak_bins` — one [`TallyBin`] per energy bin,
/// one Monte-Carlo realization per active generation, exactly like the track-
/// length `tally`. This is the extra bookkeeping
/// [`crate::physics::reactor_physics`] needs to build the fast / thermal
/// non-leakage factors from tallied leakage rather than an assumption.
///
/// # Parameters
/// - `tally` — the combined track-length tally (scored on active generations).
/// - `leak_edges` — ascending energy bin edges \[eV\], `n + 1` edges ⇒ `n` bins;
///   a leaked neutron whose escape energy is off-grid is dropped (same tail
///   convention as [`crate::tally::filter::EnergyFilter`]).
/// - `leak_bins` — persistent per-bin leakage accumulators, length
///   `leak_edges.len() - 1`, filled in place (`count` == number of active
///   generations on return).
///
/// `run_keff_csg` itself is unchanged and its callers are unaffected — the
/// leakage path is opt-in through this entry point only. Compute-backend
/// dispatch mirrors [`run_keff_csg`] (`Gpu` ⇒ multi-threaded CPU).
#[allow(clippy::too_many_arguments)]
pub fn run_keff_csg_reactor_physics(
    geom: &Geometry,
    materials: &[Material],
    nuclides: &[Nuclide],
    source_box: SourceBox,
    settings: &KeffSettings,
    tally: &mut Tally,
    leak_edges: &[f64],
    leak_bins: &mut Vec<TallyBin>,
) -> KeffResult {
    match settings.compute {
        ComputeType::CpuSingleThread => run_keff_csg_seq(
            geom,
            materials,
            nuclides,
            &[],
            None,
            source_box,
            settings,
            Some(tally),
            leak_edges,
            Some(leak_bins),
        ),
        ComputeType::CpuMultiThread(tc) => run_keff_csg_par(
            geom,
            materials,
            nuclides,
            &[],
            None,
            source_box,
            settings,
            Some(tally),
            leak_edges,
            Some(leak_bins),
            tc,
        ),
        ComputeType::Gpu => {
            log::debug!(
                "ComputeType::Gpu requested for run_keff_csg_reactor_physics, but no GPU kernel \
                 exists for general CSG geometry — running the multi-threaded CPU path instead"
            );
            run_keff_csg_par(
                geom,
                materials,
                nuclides,
                &[],
                None,
                source_box,
                settings,
                Some(tally),
                leak_edges,
                Some(leak_bins),
                ThreadCount::Auto,
            )
        }
    }
}

/// Scalar, single-thread CSG power iteration — the **trusted, deterministic,
/// bit-reproducible reference** backend ([`ComputeType::CpuSingleThread`]).
///
/// One `f64` RNG stream is threaded sequentially through the whole run (initial
/// source rejection-sampling, every history's transport, every resample), so a
/// fixed [`KeffSettings::seed`] yields the same eigenvalue — and the same tally
/// realizations — bit-for-bit on every machine. [`run_keff_csg_par`] is
/// acceleration only and is validated against this reference.
///
/// `leak_edges` / `leak_bins`: optional per-energy leakage spectrum. When
/// `leak_bins` is `Some`, every history that escapes the geometry on an active
/// generation deposits its weight into the bin of `leak_edges` matching its
/// escape energy, flushed once per active generation exactly like the tally
/// (one realization per bin). Pass `&[]` / `None` to disable it — the path
/// [`run_keff_csg`] takes.
#[allow(clippy::too_many_arguments)]
pub fn run_keff_csg_seq(
    geom: &Geometry,
    materials: &[Material],
    nuclides: &[Nuclide],
    // Majorants indexed by `TrackingMethod::Delta`; `&[]` for a purely
    // surface-tracked model, which is every model predating bn:op-867c.
    majorants: &[Majorant],
    // Optional mesh for the per-generation Shannon-entropy diagnostic
    // (bn:op-867c.13). `None` skips it at zero cost.
    entropy_mesh: Option<&crate::tally::mesh::RegularMesh>,
    source_box: SourceBox,
    settings: &KeffSettings,
    mut tally: Option<&mut Tally>,
    leak_edges: &[f64],
    mut leak_bins: Option<&mut Vec<TallyBin>>,
) -> KeffResult {
    let mut seed = settings.seed;
    let temp = settings.temperature_k;

    // Initial source: rejection-sample the box for points in a fissile cell.
    let mut source: Vec<Site> = Vec::with_capacity(settings.n_particles);
    let mut guard = 0usize;
    while source.len() < settings.n_particles {
        guard += 1;
        if guard > settings.n_particles * 10_000 {
            break; // pathological: box barely overlaps fuel; take what we have
        }
        let r = Position::new(
            source_box.lower.x + (source_box.upper.x - source_box.lower.x) * prn(&mut seed),
            source_box.lower.y + (source_box.upper.y - source_box.lower.y) * prn(&mut seed),
            source_box.lower.z + (source_box.upper.z - source_box.lower.z) * prn(&mut seed),
        );
        let (dx, dy, dz) = isotropic_direction(&mut seed);
        let u = Direction::new(dx, dy, dz);
        let fissile = geom
            .locate(r, u, SurfaceToken::NONE)
            .and_then(|p| p.material)
            .map(|m| materials[m].macro_xs(1.0e6, nuclides).nu_fission > 0.0)
            .unwrap_or(false);
        if fissile {
            source.push(Site { r, u, e: 2.0e6 });
        }
    }

    let n_gen = settings.n_inactive + settings.n_active;
    let mut k_by_generation = Vec::with_capacity(n_gen);
    let mut k_running = 1.0;
    let mut active_k = Vec::with_capacity(settings.n_active);

    // Per-batch (per-generation) track-length accumulator, one slot per tally bin.
    // Track-length scores land here during a generation and are flushed into the
    // tally's persistent bins once per active generation (one MC realization per
    // batch — see `tally::scoring::flush_batch`). Empty when no tally is attached.
    let mut batch: Vec<f64> = match tally.as_deref() {
        Some(t) => vec![0.0; t.n_bins()],
        None => Vec::new(),
    };
    // Per-generation leakage accumulator, one slot per `leak_edges` bin — same
    // batch/flush contract as `batch` above. Empty when leakage is disabled.
    let n_leak = leak_edges.len().saturating_sub(1);
    let mut leak_batch: Vec<f64> = if leak_bins.is_some() {
        vec![0.0; n_leak]
    } else {
        Vec::new()
    };

    // Run-level total; the per-generation count is folded in below.
    let mut virtual_run_total: u64 = 0;
    let mut collisions_run_total: u64 = 0;
    let mut lost_locate_run_total: u64 = 0;
    let mut stuck_events_run_total: u64 = 0;
    let mut stuck_path_run_total = 0.0_f64;
    let mut neg_dist_run: u64 = 0;
    let mut neg_level_run: u64 = 0;
    let mut neg_worst_run = 0.0_f64;
    let mut neg_lat_run: u64 = 0;
    let mut neg_surf_run: u64 = 0;
    let mut stuck_e_run = 0.0_f64;
    let mut leak_vacuum_run_total: u64 = 0;
    let mut leak_infinity_run_total: u64 = 0;
    let mut histories_run_total: u64 = 0;
    let mut entropy: Vec<f64> = Vec::new();

    for gen in 0..n_gen {
        let mut next_bank: Vec<Site> = Vec::with_capacity(settings.n_particles);
        let mut production = 0.0_f64;
        let active = gen >= settings.n_inactive;

        // Only score the tally on active generations. `tally_def` is an immutable
        // view of the filter/score definitions used to route each streamed segment
        // into `batch`; the persistent bins are only touched at the flush below.
        // Leakage is scored on the same active-only basis (empty `edges` ⇒ off).
        {
            let tally_def: Option<&Tally> = if active { tally.as_deref() } else { None };
            let leak_edges_gen: &[f64] = if active && leak_bins.is_some() {
                leak_edges
            } else {
                &[]
            };
            for site in &source {
                let outcome = transport_history_vr(
                    *site,
                    geom,
                    materials,
                    nuclides,
                    majorants,
                    temp,
                    k_running,
                    &mut next_bank,
                    &mut seed,
                    tally_def,
                    &mut batch,
                    leak_edges_gen,
                    &mut leak_batch,
                    &settings.variance_reduction,
                    None,
                    None,
                );
                production += outcome.production;
                virtual_run_total += outcome.virtual_collisions;
                collisions_run_total += outcome.collisions;
                lost_locate_run_total += outcome.lost_locate;
                stuck_events_run_total += outcome.stuck_events;
                stuck_path_run_total += outcome.stuck_path_cm;
                neg_dist_run += outcome.neg_dist;
                neg_lat_run += outcome.neg_from_lattice;
                neg_surf_run += outcome.neg_from_surface;
                if outcome.neg_dist > 0 {
                    neg_level_run = outcome.neg_level;
                }
                neg_worst_run = neg_worst_run.min(outcome.neg_worst);
                if outcome.stuck_last_e > 0.0 {
                    stuck_e_run = outcome.stuck_last_e;
                }
                leak_vacuum_run_total += outcome.leak_vacuum;
                leak_infinity_run_total += outcome.leak_infinity;
                histories_run_total += 1;
            }
        }
        // Close the batch: flush this generation's track-length totals into the
        // tally as one realization (mean/variance are over active generations).
        if active {
            if let Some(t) = tally.as_deref_mut() {
                flush_batch(t, &mut batch);
            }
            if let Some(lb) = leak_bins.as_deref_mut() {
                flush_bins(lb, &mut leak_batch);
            }
        }

        let k_gen = production / settings.n_particles as f64;
        // Shannon entropy of THIS generation's fission source, before the
        // bank is resampled. Ported from OpenMC `src/eigenvalue.cpp:587` --
        // the bank must be the pre-synchronisation one, which is why this sits
        // here and not after `resample`.
        if let Some(mesh) = entropy_mesh {
            let sites: Vec<crate::particle::bank::BankSite> = next_bank
                .iter()
                .map(|s| crate::particle::bank::BankSite {
                    r: s.r,
                    u: s.u,
                    e: s.e,
                    wgt: 1.0,
                    seed: 0,
                })
                .collect();
            if let Some(h) = mesh.shannon_entropy(&sites) {
                entropy.push(h);
            }
        }
        k_by_generation.push(k_gen);
        k_running = k_gen.max(1.0e-6);
        if active {
            active_k.push(k_gen);
        }

        if next_bank.is_empty() {
            break;
        }

        // `k` trigger (GitHub #263): stop once the eigenvalue's own
        // uncertainty meets the requested metric. Checked only on active
        // generations, and only once there are at least two of them — a
        // standard error from one realisation is not a number.
        if let Some(trig) = settings.keff_trigger {
            if active && active_k.len() >= 2 {
                let stats = crate::tally::trigger::BinStats {
                    sum: active_k.iter().sum(),
                    sum_sq: active_k.iter().map(|k| k * k).sum(),
                };
                let ratio = crate::tally::trigger::bin_ratio(stats, active_k.len(), &trig);
                if crate::tally::trigger::satisfied(ratio) {
                    break;
                }
            }
        }

        source = resample(&next_bank, settings.n_particles, &mut seed);
    }

    let (k_mean, k_std) = mean_and_stderr(&active_k);
    KeffResult {
        k_mean,
        k_std,
        k_by_generation,
        entropy,
        virtual_collisions: virtual_run_total,
        collisions: collisions_run_total,
        lost_locate: lost_locate_run_total,
        stuck_events: stuck_events_run_total,
        stuck_path_cm: stuck_path_run_total,
        neg_dist: neg_dist_run,
        neg_level: neg_level_run,
        neg_worst: neg_worst_run,
        neg_from_lattice: neg_lat_run,
        neg_from_surface: neg_surf_run,
        stuck_last_e: stuck_e_run,
        leak_vacuum: leak_vacuum_run_total,
        leak_infinity: leak_infinity_run_total,
        histories: histories_run_total,
    }
}

/// Rayon-parallel CSG power iteration ([`ComputeType::CpuMultiThread`]).
///
/// Same physics and power-iteration structure as [`run_keff_csg_seq`], but the
/// histories **within each generation** are transported in parallel with
/// [`rayon`] in a dedicated pool sized to `thread_count` (never the implicit
/// global pool). The generation loop stays sequential — generation `g+1`'s source
/// is `g`'s resampled fission bank, a hard data dependency.
///
/// # Reproducibility (independent of thread count)
///
/// Each history is given a **completely independent, deterministic** RNG stream
/// derived only from `(settings.seed, generation, history index)` via the LCG
/// jump-ahead ([`crate::rng::lcg::future_seed`]) — never a shared mutable seed —
/// so the result never races and is identical regardless of how rayon schedules
/// the work. This mirrors [`crate::physics::keff::run_keff_cpu_multi`]; see its
/// docs for the `HIST_STRIDE` / `GEN_STRIDE` non-overlap argument. The initial
/// source sampling and each resample run on a separate sequential `src_seed`
/// stream, kept off the parallel path. Because the per-history stream structure
/// differs from the single sequential stream, this backend does **not** bit-match
/// [`run_keff_csg_seq`] — it is a statistically independent estimate of the same
/// eigenvalue and tally, agreeing within combined uncertainty.
///
/// # Tally
///
/// When a `tally` is attached, each history accumulates its track-length scores
/// into a **private per-history batch**; the batches are summed in history-index
/// order into the generation batch (a deterministic reduction) and flushed as one
/// realization per active generation — the same batch/flush contract as
/// [`run_keff_csg_seq`], just reduced in parallel.
///
/// `leak_edges` / `leak_bins`: the per-energy leakage spectrum, identical
/// contract to [`run_keff_csg_seq`] — each history's escape (if any) accumulates
/// into a private `local_leak`, the per-history vectors are summed in
/// history-index order (a deterministic reduction) into the generation leak
/// batch, and flushed once per active generation. `&[]` / `None` disables it.
#[allow(clippy::too_many_arguments)]
pub fn run_keff_csg_par(
    geom: &Geometry,
    materials: &[Material],
    nuclides: &[Nuclide],
    // Majorants indexed by `TrackingMethod::Delta`; `&[]` for a purely
    // surface-tracked model, which is every model predating bn:op-867c.
    majorants: &[Majorant],
    // Optional mesh for the per-generation Shannon-entropy diagnostic
    // (bn:op-867c.13). `None` skips it at zero cost.
    entropy_mesh: Option<&crate::tally::mesh::RegularMesh>,
    source_box: SourceBox,
    settings: &KeffSettings,
    mut tally: Option<&mut Tally>,
    leak_edges: &[f64],
    mut leak_bins: Option<&mut Vec<TallyBin>>,
    thread_count: ThreadCount,
) -> KeffResult {
    #[cfg(not(target_arch = "wasm32"))]
    use rayon::prelude::*;
    #[cfg(target_arch = "wasm32")]
    use crate::wasm_par::prelude::*;
    #[cfg(target_arch = "wasm32")]
    use crate::wasm_par as rayon;

    let temp = settings.temperature_k;
    let n_bins = tally.as_deref().map(|t| t.n_bins()).unwrap_or(0);
    let leak_enabled = leak_bins.is_some() && leak_edges.len() >= 2;
    let n_leak = leak_edges.len().saturating_sub(1);

    // Dedicated, explicitly sized rayon pool. `resolve()` maps the ThreadCount to
    // a concrete worker count (>= 1); the per-history seeding below is
    // thread-count-independent, so the eigenvalue is identical regardless.
    let n_threads = thread_count.resolve();
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(n_threads)
        .build()
        .expect("rayon thread pool");

    // Dedicated sequential stream for source sampling + resampling only — kept
    // separate from the per-history transport streams so both stay deterministic.
    let mut src_seed = settings.seed;

    // Initial source: rejection-sample the box for points in a fissile cell
    // (identical to the single-thread path, on the sequential src stream).
    let mut source: Vec<Site> = Vec::with_capacity(settings.n_particles);
    let mut guard = 0usize;
    while source.len() < settings.n_particles {
        guard += 1;
        if guard > settings.n_particles * 10_000 {
            break; // pathological: box barely overlaps fuel; take what we have
        }
        let r = Position::new(
            source_box.lower.x + (source_box.upper.x - source_box.lower.x) * prn(&mut src_seed),
            source_box.lower.y + (source_box.upper.y - source_box.lower.y) * prn(&mut src_seed),
            source_box.lower.z + (source_box.upper.z - source_box.lower.z) * prn(&mut src_seed),
        );
        let (dx, dy, dz) = isotropic_direction(&mut src_seed);
        let u = Direction::new(dx, dy, dz);
        let fissile = geom
            .locate(r, u, SurfaceToken::NONE)
            .and_then(|p| p.material)
            .map(|m| materials[m].macro_xs(1.0e6, nuclides).nu_fission > 0.0)
            .unwrap_or(false);
        if fissile {
            source.push(Site { r, u, e: 2.0e6 });
        }
    }

    let n_gen = settings.n_inactive + settings.n_active;
    let mut k_by_generation = Vec::with_capacity(n_gen);
    let mut k_running = 1.0;
    let mut active_k = Vec::with_capacity(settings.n_active);

    // Run the whole generation loop inside the dedicated pool so every
    // `into_par_iter()` dispatches onto exactly `n_threads` workers.
    // Run-level total for the parallel driver. Declared OUTSIDE pool.install so
    // the result built after it can read it; rayon's closure borrows it mutably.
    let mut virtual_run_total: u64 = 0;
    let mut collisions_run_total: u64 = 0;
    let mut lost_locate_run_total: u64 = 0;
    let mut stuck_events_run_total: u64 = 0;
    let mut stuck_path_run_total = 0.0_f64;
    let mut neg_dist_run: u64 = 0;
    let mut neg_level_run: u64 = 0;
    let mut neg_worst_run = 0.0_f64;
    let mut neg_lat_run: u64 = 0;
    let mut neg_surf_run: u64 = 0;
    let mut stuck_e_run = 0.0_f64;
    let mut leak_vacuum_run_total: u64 = 0;
    let mut leak_infinity_run_total: u64 = 0;
    let mut histories_run_total: u64 = 0;
    let mut entropy: Vec<f64> = Vec::new();
    pool.install(|| {
        for gen in 0..n_gen {
            let active = gen >= settings.n_inactive;
            // Base seed for this generation's per-history sub-streams.
            let gen_base_seed = future_seed((gen as u64).wrapping_mul(GEN_STRIDE), settings.seed);

            // Transport every history in parallel. `into_par_iter().map(...).collect()`
            // on an indexed iterator preserves input order, so the reduction below
            // is deterministic regardless of thread count. `tally_def` is an
            // immutable view scoped to this block so the mutable flush below is free
            // to borrow `tally` again.
            let leak_this_gen = active && leak_enabled;
            let results: Vec<(HistoryOutcome, Vec<Site>, Vec<f64>, Vec<f64>)> = {
                let tally_def: Option<&Tally> = if active { tally.as_deref() } else { None };
                let leak_edges_gen: &[f64] = if leak_this_gen { leak_edges } else { &[] };
                (0..source.len())
                    .into_par_iter()
                    .map(|hist_idx| {
                        // Independent, deterministic sub-stream for this history;
                        // owned locally — never shared across threads.
                        let mut seed =
                            future_seed((hist_idx as u64).wrapping_mul(HIST_STRIDE), gen_base_seed);
                        let mut local_bank: Vec<Site> = Vec::new();
                        let mut local_batch: Vec<f64> = if tally_def.is_some() {
                            vec![0.0; n_bins]
                        } else {
                            Vec::new()
                        };
                        let mut local_leak: Vec<f64> = if leak_this_gen {
                            vec![0.0; n_leak]
                        } else {
                            Vec::new()
                        };
                        let outcome = transport_history(
                            source[hist_idx],
                            geom,
                            materials,
                            nuclides,
                            majorants,
                            temp,
                            k_running,
                            &mut local_bank,
                            &mut seed,
                            tally_def,
                            &mut local_batch,
                            leak_edges_gen,
                            &mut local_leak,
                        );
                        (outcome, local_bank, local_batch, local_leak)
                    })
                    .collect()
            };

            // Deterministic sequential reduction: sum productions, concatenate
            // banks and sum per-history tally / leakage batches in history-index
            // order.
            let mut production = 0.0_f64;
            let mut next_bank: Vec<Site> = Vec::with_capacity(settings.n_particles);
            let mut batch: Vec<f64> = if active && n_bins > 0 {
                vec![0.0; n_bins]
            } else {
                Vec::new()
            };
            let mut leak_batch: Vec<f64> = if leak_this_gen {
                vec![0.0; n_leak]
            } else {
                Vec::new()
            };
            for (outcome, bank, local_batch, local_leak) in results {
                production += outcome.production;
                virtual_run_total += outcome.virtual_collisions;
                collisions_run_total += outcome.collisions;
                lost_locate_run_total += outcome.lost_locate;
                stuck_events_run_total += outcome.stuck_events;
                stuck_path_run_total += outcome.stuck_path_cm;
                neg_dist_run += outcome.neg_dist;
                neg_lat_run += outcome.neg_from_lattice;
                neg_surf_run += outcome.neg_from_surface;
                if outcome.neg_dist > 0 {
                    neg_level_run = outcome.neg_level;
                }
                neg_worst_run = neg_worst_run.min(outcome.neg_worst);
                if outcome.stuck_last_e > 0.0 {
                    stuck_e_run = outcome.stuck_last_e;
                }
                leak_vacuum_run_total += outcome.leak_vacuum;
                leak_infinity_run_total += outcome.leak_infinity;
                histories_run_total += 1;
                next_bank.extend(bank);
                if !local_batch.is_empty() {
                    for (b, v) in batch.iter_mut().zip(local_batch) {
                        *b += v;
                    }
                }
                if !local_leak.is_empty() {
                    for (b, v) in leak_batch.iter_mut().zip(local_leak) {
                        *b += v;
                    }
                }
            }

            // Close the batch: flush this generation's track-length totals into the
            // tally as one realization (mean/variance over active generations).
            if active {
                if let Some(t) = tally.as_deref_mut() {
                    flush_batch(t, &mut batch);
                }
                if leak_this_gen {
                    if let Some(lb) = leak_bins.as_deref_mut() {
                        flush_bins(lb, &mut leak_batch);
                    }
                }
            }

            let k_gen = production / settings.n_particles as f64;
            // Shannon entropy of THIS generation's fission source, before the
            // bank is resampled. Ported from OpenMC `src/eigenvalue.cpp:587` --
            // the bank must be the pre-synchronisation one, which is why this sits
            // here and not after `resample`.
            if let Some(mesh) = entropy_mesh {
                let sites: Vec<crate::particle::bank::BankSite> = next_bank
                    .iter()
                    .map(|s| crate::particle::bank::BankSite {
                        r: s.r,
                        u: s.u,
                        e: s.e,
                        wgt: 1.0,
                        seed: 0,
                    })
                    .collect();
                if let Some(h) = mesh.shannon_entropy(&sites) {
                    entropy.push(h);
                }
            }
            k_by_generation.push(k_gen);
            k_running = k_gen.max(1.0e-6);
            if active {
                active_k.push(k_gen);
            }

            if next_bank.is_empty() {
                break;
            }
            source = resample(&next_bank, settings.n_particles, &mut src_seed);
        }
    });

    let (k_mean, k_std) = mean_and_stderr(&active_k);
    KeffResult {
        k_mean,
        k_std,
        k_by_generation,
        entropy,
        virtual_collisions: virtual_run_total,
        collisions: collisions_run_total,
        lost_locate: lost_locate_run_total,
        stuck_events: stuck_events_run_total,
        stuck_path_cm: stuck_path_run_total,
        neg_dist: neg_dist_run,
        neg_level: neg_level_run,
        neg_worst: neg_worst_run,
        neg_from_lattice: neg_lat_run,
        neg_from_surface: neg_surf_run,
        stuck_last_e: stuck_e_run,
        leak_vacuum: leak_vacuum_run_total,
        leak_infinity: leak_infinity_run_total,
        histories: histories_run_total,
    }
}

/// Bin one leaked neutron of weight `w` into the per-generation leakage
/// spectrum `leak` at energy `e` \[eV\], on the ascending fine energy grid
/// `edges` (`edges.len() - 1 == leak.len()`).
///
/// A **leaked** neutron is one whose history ends by escaping the geometry —
/// crossing a vacuum surface, streaming to infinity, or (rarely) a lost/stuck
/// history. This is the analog of a surface-current tally on the outer
/// boundary, resolved by energy; the CSG eigenvalue driver accumulates it so
/// [`crate::physics::reactor_physics`] can build the fast / thermal
/// non-leakage factors from real tallied leakage rather than an assumption.
///
/// No-op when leakage accounting is disabled (`edges` empty) or the escape
/// energy is off-grid (`e < edges[0]` or `e >= edges[last]`) — the same
/// tail-drop convention as [`crate::tally::filter::EnergyFilter`].
#[inline]
fn score_leak(leak: &mut [f64], edges: &[f64], e: f64, w: f64) {
    if edges.len() < 2 {
        return;
    }
    if e < edges[0] || e >= edges[edges.len() - 1] {
        return;
    }
    let i = edges.partition_point(|&x| x <= e).saturating_sub(1);
    leak[i] += w;
}

/// Transport one source neutron (plus its same-generation `(n,2n)` secondaries)
/// to death over the CSG geometry, banking fission neutrons and scoring the
/// optional **track-length** flux/reaction-rate tally. Returns the fission
/// production ν̄ summed over the history's fission events.
///
/// If `tally` is `Some`, every streamed free-flight segment deposits its
/// track-length contribution (`w·d` flux, `w·d·Σ_x` reaction rates) into `batch`
/// (the caller's per-generation accumulator); `batch` is flushed into the tally's
/// persistent bins once per active generation.
///
/// If `leak_edges` is non-empty, every history that ends by **escaping** the
/// geometry deposits its weight into `leak_batch` at its escape energy (see
/// [`score_leak`]); `leak_batch` has length `leak_edges.len() - 1` and is
/// flushed once per active generation by the caller. Pass an empty `leak_edges`
/// (and any `leak_batch`, e.g. `&mut []`) to disable leakage accounting.
#[allow(clippy::too_many_arguments)]
/// What one history produced.
///
/// `production` is the fission neutron yield, as before. `virtual_collisions`
/// is the count rejected inside delta-tracked regions -- the **price of the
/// majorant**, which `keff_delta.rs`'s `delta_flight` discards entirely
/// (`bn:op-867c.5`). Returning it rather than accumulating through a `&mut`
/// is what lets the rayon driver reduce it: a shared mutable cannot be
/// captured by the `Fn` closure that path uses.
///
/// Zero on a purely surface-tracked model.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct HistoryOutcome {
    /// Fission neutron production from this history.
    pub production: f64,
    /// Virtual collisions rejected inside delta regions.
    pub virtual_collisions: u64,
    /// **Real** collisions this history underwent. A model where this is near
    /// zero is not absorbing neutrons, it is losing them before they interact.
    pub collisions: u64,
    /// Histories ended because `Geometry::locate` returned `None`. These are
    /// scored as leaks so the balance closes, which makes them INVISIBLE in
    /// `k` alone -- hence the separate count.
    pub lost_locate: u64,
    /// Histories ended by exhausting `MAX_EVENTS`, likewise scored as leaks.
    pub stuck_events: u64,
    /// Total path length \[cm\] travelled by those stuck histories.
    pub stuck_path_cm: f64,
    /// Last-known energy \[eV\] of the most recent stuck history.
    pub stuck_last_e: f64,
    /// Times `distance_to_boundary` returned a NEGATIVE distance.
    pub neg_dist: u64,
    /// Coordinate level of the most recent negative distance.
    pub neg_level: u64,
    /// Most negative distance \[cm\] seen.
    pub neg_worst: f64,
    /// Negative distances whose winning candidate was a LATTICE tile crossing.
    pub neg_from_lattice: u64,
    /// Negative distances whose winning candidate was a CSG surface.
    pub neg_from_surface: u64,
    /// Histories that crossed a surface carrying a vacuum boundary condition —
    /// the only GENUINE leak of the five ends recorded here.
    pub leak_vacuum: u64,
    /// Histories where `distance_to_boundary` found no surface ahead and the
    /// neutron streamed to infinity. In a closed model this should be zero.
    pub leak_infinity: u64,
}

pub(crate) fn transport_history(
    site: Site,
    geom: &Geometry,
    materials: &[Material],
    nuclides: &[Nuclide],
    majorants: &[Majorant],
    temp: f64,
    k_running: f64,
    next_bank: &mut Vec<Site>,
    seed: &mut u64,
    tally: Option<&Tally>,
    batch: &mut [f64],
    leak_edges: &[f64],
    leak_batch: &mut [f64],
) -> HistoryOutcome {
    // Analog. Delegating rather than duplicating is what makes "analog is
    // untouched" checkable instead of merely claimed: there is one history
    // loop, and `ANALOG.is_analog()` is true, so every branch this change adds
    // is skipped. `tests/variance_reduction_is_bit_identical_when_analog.rs`
    // pins that at the eigenvalue.
    let analog = VarianceReduction {
        survival_biasing: false,
        weight_cutoff: 0.25,
        weight_survive: 1.0,
        survival_normalization: false,
        weight_windows: None,
    };
    debug_assert!(analog.is_analog());
    transport_history_vr(
        site, geom, materials, nuclides, majorants, temp, k_running, next_bank, seed, tally,
        batch, leak_edges, leak_batch, &analog, None, None,
    )
}

/// [`transport_history`] with an explicit variance-reduction configuration.
///
/// GitHub #258. With [`VarianceReduction::is_analog`] true this is the analog
/// kernel, unchanged and consuming the RNG stream in exactly the same order —
/// that is the property the whole crate's recorded V&V rests on, so it is
/// pinned by a test rather than asserted here.
///
/// With survival biasing on the structure follows
/// `sample_neutron_reaction` (`src/physics.cpp`) at OpenMC `afa7a14`:
/// fission sites are banked from `w/k · νΣ_f/Σ_t` **whether or not** the
/// collision "is" a fission, the weight is then reduced by `Σ_a/Σ_t` rather
/// than the particle being killed, the outgoing reaction is drawn from the
/// scattering channels alone, and Russian roulette is played last.
#[allow(clippy::too_many_arguments)]
pub(crate) fn transport_history_vr(
    site: Site,
    geom: &Geometry,
    materials: &[Material],
    nuclides: &[Nuclide],
    // Majorants indexed by `TrackingMethod::Delta`. EMPTY when no region
    // declares delta tracking, which is every model predating bn:op-867c --
    // and that is what makes this change bit-identical for all of them.
    majorants: &[Majorant],
    temp: f64,
    k_running: f64,
    next_bank: &mut Vec<Site>,
    seed: &mut u64,
    tally: Option<&Tally>,
    batch: &mut [f64],
    leak_edges: &[f64],
    leak_batch: &mut [f64],
    vr: &VarianceReduction,
    // Track capture (GitHub #271). `None` records nothing and costs a single
    // `Option` check per event; recording draws no randomness, so a run with
    // capture on gives the same eigenvalue bit for bit as one without.
    mut tracks: Option<&mut TrackRecorder>,
    // Surface-source recording (GitHub #264). Records every crossing of a
    // watched surface WITH ITS WEIGHT, for replay as a second stage's source.
    // Like track capture this draws no randomness.
    mut surface_source: Option<&mut SurfaceSource>,
) -> HistoryOutcome {
    // Virtual collisions rejected inside delta regions (bn:op-867c.5).
    // Stays zero on a purely surface-tracked model.
    let mut virtual_collisions: u64 = 0;
    let mut collisions: u64 = 0;
    let mut lost_locate: u64 = 0;
    let mut stuck_events: u64 = 0;
    let mut leak_vacuum: u64 = 0;
    let mut leak_infinity: u64 = 0;
    // Path length accumulated by histories that hit the event budget. A stuck
    // history that has travelled ~0 cm is OSCILLATING on a surface; one that has
    // travelled far is legitimately crossing a great many tiles. The two need
    // opposite fixes, so they are distinguished rather than guessed at.
    let mut stuck_path_cm = 0.0_f64;
    let mut stuck_last_e = 0.0_f64;
    let mut path_cm = 0.0_f64;
    let mut neg_dist: u64 = 0;
    let mut neg_level: u64 = 0;
    let mut neg_worst = 0.0_f64;
    let mut neg_from_lattice: u64 = 0;
    let mut neg_from_surface: u64 = 0;
    const NUDGE: f64 = 1.0e-9;
    /// Jump-ahead distance between a weight-window split child's stream and
    /// its parent's. `DEFAULT_STRIDE` is the per-particle stride the crate
    /// already uses to separate history streams (`rng::lcg::init_seed`), so a
    /// split child is as independent of its parent as two source particles
    /// are of each other.
    const SPLIT_STRIDE: u64 = crate::rng::lcg::DEFAULT_STRIDE;
    let mut production = 0.0;
    // (site, weight, optional own RNG stream). `None` continues the shared
    // stream, which is what every secondary before #258 did and is what keeps
    // an analog run bit-identical. `Some` is used ONLY for weight-window split
    // children, which get a disjoint sub-stream by `future_seed` jump-ahead —
    // `CLAUDE.md`'s `op-rbo` lesson is that a quoted sigma is only meaningful
    // if the streams behind it are independent, and a split is the one place
    // in this loop that creates a genuinely new particle rather than
    // continuing an existing one.
    let mut stack: Vec<(Site, f64, Option<u64>)> = vec![(site, 1.0, None)];

    // Safety cap on events per history: a particle in a purely-scattering
    // reflective medium with vanishing absorption could otherwise bounce forever.
    // 100k events is far beyond any physical history (mean ~tens of collisions).
    const MAX_EVENTS: u32 = 100_000;

    while let Some((start, start_wgt, own_seed)) = stack.pop() {
        // A split child transports on its own stream; everything else
        // continues the shared one, exactly as before #258.
        let mut owned_seed = own_seed.unwrap_or(0);
        let seed: &mut u64 = match own_seed {
            Some(_) => &mut owned_seed,
            None => &mut *seed,
        };
        let mut r = start.r;
        let mut u = start.u;
        let mut e = start.e;
        let mut on_surface = SurfaceToken::NONE;
        let mut events = 0u32;
        // Statistical weight. Fission sites are always born at 1 (upstream
        // `create_fission_sites`: `site.wgt = 1./weight`, and `weight` is 1
        // without uniform-fission-source weighting), so the weight is a
        // within-history quantity and never needs storing on a `Site`.
        // Under analog transport it stays exactly 1.0 and every `w` below is
        // the literal 1.0 the analog kernel passed before this change.
        let mut w = start_wgt;
        // Birth weight, for `survival_normalization` and for the weight
        // window's birth renormalisation.
        let w_birth = w;
        // Weight-window bookkeeping that persists across this sub-history's
        // checkpoints (`wgt_ww_born`, `ww_factor`, `n_split` upstream).
        let mut ww_state = WindowState {
            weight_born: w_birth,
            ..WindowState::default()
        };

        if let Some(t) = tracks.as_deref_mut() {
            // Unconditionally: `begin` is what counts a refused track, and
            // `record` is a no-op when there is no current track. Guarding
            // this with `is_full` left `dropped_tracks` permanently zero.
            {
                t.begin();
                t.record(TrackState {
                    r,
                    u,
                    energy: e,
                    time: 0.0,
                    weight: w,
                    cell: usize::MAX,
                    material: None,
                    event: TrackEvent::Born,
                });
            }
        }

        /// Record one phase-space state on the current track, if capturing.
        ///
        /// A macro rather than a closure because the state is assembled from
        /// locals the closure would have to borrow mutably alongside the
        /// recorder.
        macro_rules! track {
            ($event:expr, $cell:expr, $material:expr) => {
                if let Some(t) = tracks.as_deref_mut() {
                    t.record(TrackState {
                        r,
                        u,
                        energy: e,
                        time: 0.0,
                        weight: w,
                        cell: $cell,
                        material: $material,
                        event: $event,
                    });
                }
            };
        }

        /// One weight-window checkpoint. Upstream has two,
        /// `weight_window_checkpoint_surface` and
        /// `weight_window_checkpoint_collision`; both call the same routine,
        /// so this is written once and invoked at both places.
        ///
        /// `$kill` is what to do when the game kills the particle — the two
        /// call sites sit at different points in the loop and need different
        /// control flow.
        macro_rules! weight_window_checkpoint {
            ($kill:expr) => {
                if let Some(ww) = vr.weight_windows.as_ref() {
                    if let Some(window) = ww.look_up(r, e) {
                        match apply_window(window, &mut ww_state, w, seed) {
                            WindowOutcome::Unchanged => {}
                            WindowOutcome::Killed => {
                                // A rouletted particle is not a leak and not an
                                // absorption; it is an estimator decision, and
                                // scoring it anywhere would double-count the
                                // weight the survivors already carry.
                                $kill
                            }
                            WindowOutcome::Survived { weight } => w = weight,
                            WindowOutcome::Split { copies, weight } => {
                                // `copies - 1` new particles, each on its own
                                // jumped-ahead stream; this one carries the
                                // last share. Total weight is conserved
                                // exactly - pinned by
                                // `splitting_conserves_total_weight`.
                                let mut child_seed = *seed;
                                for _ in 1..copies {
                                    child_seed =
                                        crate::rng::lcg::future_seed(SPLIT_STRIDE, child_seed);
                                    stack.push((
                                        Site { r, u, e },
                                        weight,
                                        Some(child_seed),
                                    ));
                                }
                                w = weight;
                            }
                        }
                    }
                }
            };
        }

        'history: loop {
            events += 1;
            if events > MAX_EVENTS {
                // Stuck history (should be vanishingly rare) — count it as a
                // leak at its last-known energy so the neutron balance still
                // closes. A small mis-binned population if it ever fires.
                stuck_events += 1;
                stuck_path_cm += path_cm;
                stuck_last_e = e;
                score_leak(leak_batch, leak_edges, e, w);
                track!(TrackEvent::Lost, usize::MAX, None);
                break 'history;
            }
            // Locate: which cell/material are we in?
            let Some(path) = geom.locate(r, u, on_surface) else {
                lost_locate += 1;
                // Lost the particle (numerical edge case) — treat as a leak at
                // last-known energy, same reasoning as the MAX_EVENTS arm.
                score_leak(leak_batch, leak_edges, e, w);
                track!(TrackEvent::Lost, usize::MAX, None);
                break 'history;
            };
            let leaf = *path.leaf();
            let cell_idx = leaf.cell;

            let sigma_t = match path.material {
                Some(m) => materials[m].macro_xs_total(e, nuclides),
                None => 0.0, // void: stream freely to the next boundary
            };

            let d_bound = geom.distance_to_boundary(&path);
            if d_bound.distance < 0.0 {
                // A NEGATIVE distance-to-boundary steps the neutron backwards,
                // so it re-crosses the same surface forever until the event
                // budget kills it. Record where it came from rather than
                // silently absorbing it into the stuck count.
                neg_dist += 1;
                neg_level = d_bound.coord_level as u64;
                match d_bound.crossing {
                    Crossing::Lattice => neg_from_lattice += 1,
                    Crossing::Surface(_) => neg_from_surface += 1,
                    Crossing::None => {}
                }
                neg_worst = neg_worst.min(d_bound.distance);
            }

            // ── How far to the next REAL collision, and in what material ───
            //
            // This is the ONLY thing that differs between surface and delta
            // tracking (bn:op-867c.4). Everything below -- the track-length
            // scoring, the collision physics, the boundary crossing, the leak
            // accounting -- is shared, because the collision block takes a
            // position, a material and an energy and does not care how the
            // particle got there.
            //
            // With no delta regions declared, `majorants` is empty and this
            // reduces to exactly the previous two lines, drawing the SAME
            // single `prn` in the same order. Every existing model is therefore
            // bit-identical across this change, which is how it is verified.
            let (d_col, col_material) = match path.tracking {
                TrackingMethod::Surface => {
                    let d = if sigma_t > 0.0 {
                        -prn(seed).max(f64::MIN_POSITIVE).r_ln() / sigma_t
                    } else {
                        f64::INFINITY
                    };
                    (d, path.material)
                }
                TrackingMethod::Delta { majorant } => {
                    let Some(maj) = majorants.get(majorant) else {
                        // A region declared a majorant index the caller did not
                        // supply. Leaking is the honest failure: delta tracking
                        // with no bound would silently bias, and falling back to
                        // surface tracking would silently change the method.
                        score_leak(leak_batch, leak_edges, e, w);
                        break 'history;
                    };
                    // The region's OWN extent, not the nearest surface -- a bed
                    // is full of internal surfaces the tracker exists to cross.
                    let exit_at = geom.distance_out_of_level(&path, path.tracking_level);
                    let step = bounded_delta_flight(
                        r,
                        u,
                        e,
                        maj,
                        materials,
                        nuclides,
                        MAX_VIRTUAL_COLLISIONS,
                        // `exit_at` is measured from `r`; convert a probe point
                        // back to remaining distance along the ray.
                        |p: Position, _d: Direction| {
                            let travelled =
                                (p.x - r.x) * u.u + (p.y - r.y) * u.v + (p.z - r.z) * u.w;
                            exit_at - travelled
                        },
                        |p: Position| {
                            geom.locate(p, u, SurfaceToken::NONE)
                                .and_then(|q| q.material)
                        },
                        seed,
                    );
                    match step {
                        DeltaStep::Collision {
                            position,
                            material,
                            virtual_collisions: v,
                            ..
                        } => {
                            virtual_collisions += u64::from(v);
                            let d = (position.x - r.x) * u.u
                                + (position.y - r.y) * u.v
                                + (position.z - r.z) * u.w;
                            (d, Some(material))
                        }
                        // Left the delta region: stream to its edge and let the
                        // enclosing method take over on the next `locate`.
                        // `d_col = INFINITY` sends it down the crossing arm.
                        DeltaStep::Exit {
                            virtual_collisions: v,
                            ..
                        } => {
                            virtual_collisions += u64::from(v);
                            (f64::INFINITY, path.material)
                        }
                        DeltaStep::Exhausted {
                            virtual_collisions: v,
                        } => {
                            virtual_collisions += u64::from(v);
                            score_leak(leak_batch, leak_edges, e, w);
                            break 'history;
                        }
                    }
                }
            };

            // ── Track-length tally scoring ─────────────────────────────────
            // The particle streams `seg = min(d_col, d_bound)` through the current
            // cell at the current (pre-collision) energy `e`; deposit its
            // track-length flux/reaction-rate contribution before either the
            // collision changes `e` or the crossing changes the cell. `seg` is
            // finite even in a void (d_col = ∞ ⇒ seg = the boundary distance);
            // there `macro_xs` is `None`, so only the flux score deposits.
            if let Some(t) = tally {
                match path.tracking {
                    // ── Surface tracking: TRACK-LENGTH estimator ───────────
                    // The flight is bounded by the nearest surface, so it
                    // cannot cross a material and `path.material` IS the
                    // material the whole segment traversed.
                    TrackingMethod::Surface => {
                        let seg = d_col.min(d_bound.distance);
                        let mxs = path.material.map(|m| materials[m].macro_xs(e, nuclides));
                        let mat_idx = path.material.unwrap_or(usize::MAX);
                        // Spatial filters (mesh / Legendre) bin on the segment
                        // midpoint `r + 0.5·seg·u` — the track-length-
                        // representative point of the free flight (constant
                        // energy, single cell over the segment).
                        let mid = stream(r, u, 0.5 * seg);
                        score_track_length(
                            batch, t, cell_idx, mat_idx, leaf.universe, e, seg, mid,
                            mxs.as_ref(), w,
                        );
                    }
                    // ── Delta tracking: COLLISION estimator ────────────────
                    //
                    // A TRACK-LENGTH ESTIMATOR IS INVALID HERE. Under delta
                    // tracking the flight crosses materials virtually and ends
                    // wherever the real collision happened, so there is no
                    // single material whose cross sections describe the
                    // segment. Scoring `seg` against `path.material` — the
                    // material `locate` reported at the START of the flight —
                    // attributes the whole path to the wrong material with the
                    // wrong cross sections.
                    //
                    // That was the defect (`bn:op-ra9f`). It is the same trap
                    // the collision branch below already documents for the
                    // reaction physics under `bn:op-867c.4`: that one was fixed
                    // by reading `col_material`, and this estimator was left
                    // behind. Measured on the HTR-10 bed, 8-group: the tallied
                    // `k_inf` fell 4175 pcm below what the run's own
                    // `k_eff/(1-L)` balance implies, and came out BELOW `k_eff`
                    // — impossible for a leaking system. Surface-tracking the
                    // same geometry closed that balance to -133 pcm.
                    //
                    // The collision estimator is what OpenMC and Serpent use in
                    // delta-tracked regions for exactly this reason: it scores
                    // `w/Sigma_t` at the resolved collision site, where the
                    // material IS known. A flight that exits the region without
                    // colliding scores nothing — correct, not an omission: the
                    // estimator's support is collisions, and it is unbiased
                    // over a history.
                    TrackingMethod::Delta { .. } => {
                        if d_col < d_bound.distance {
                            if let Some(m) = col_material {
                                let mxs = materials[m].macro_xs(e, nuclides);
                                if mxs.total > 0.0 {
                                    // The collision estimator scored THROUGH the
                                    // track-length machinery. The two differ only
                                    // in the length deposited: track-length gives
                                    // `w·d`, the collision estimator `w/Sigma_t`.
                                    // Passing `1/Sigma_t` as the length therefore
                                    // deposits `w/Sigma_t` (flux) and
                                    // `w·Sigma_x/Sigma_t` (reaction rates), which
                                    // IS the collision estimator — and it keeps
                                    // the per-generation batch accumulation and
                                    // every filter the track-length path already
                                    // supports. `score_collision` writes straight
                                    // to the tally and would bypass the batch.
                                    let at = stream(r, u, d_col);
                                    score_track_length(
                                        batch,
                                        t,
                                        cell_idx,
                                        m,
                                        leaf.universe,
                                        e,
                                        1.0 / mxs.total,
                                        at,
                                        Some(&mxs),
                                        w,
                                    );
                                }
                            }
                        }
                    }
                }
            }

            if d_col < d_bound.distance {
                collisions += 1;
                // ── Collision ──────────────────────────────────────────────
                r = stream(r, u, d_col);
                on_surface = SurfaceToken::NONE;
                // The material the DISPATCH resolved, not the one `locate`
                // reported at the start of the flight. Identical under surface
                // tracking; under delta tracking a flight crosses materials
                // virtually and ends wherever the real collision happened, so
                // reading `path.material` here would collide in the material the
                // particle STARTED in (bn:op-867c.4).
                let m = col_material.expect("collision requires a material");
                let material = &materials[m];

                // Analog reaction partition (mirrors keff.rs transport_history).
                //
                // THE COLLISION MATERIAL'S OWN TEMPERATURE, not the run-wide
                // `settings.temperature_k`.
                //
                // `Material::macro_xs` already evaluates at `self.temperature`,
                // so the flight distance and the tally scores respect per-material
                // temperature. Partitioning the collision at a global temperature
                // instead left the two inconsistent: a neutron would fly according
                // to one temperature and then have its collision split into
                // fission / capture / scatter according to another. Invisible
                // while every material sits at one temperature, and wrong the
                // moment they differ -- which is exactly what a fuel / moderator /
                // reflector temperature-coefficient map needs them to do.
                //
                // Same family as the `col_material` fix below and `bn:op-ra9f`
                // above: a per-material quantity read from the wrong source at
                // the collision site.
                let mat_temp = material.temperature;
                let ci = material.sample_nuclide(e, seed, nuclides);
                let nuc = &nuclides[material.components[ci].nuclide_idx];
                let x = if nuc.needs_urr_draw(e) {
                    // Unresolved-resonance self-shielding: draw one band. The
                    // `needs_urr_draw` gate is what keeps a run WITHOUT tables
                    // bit-identical to one from before they existed -- an
                    // unconditional draw would shift every RNG stream in the crate
                    // for no physical reason.
                    nuc.xs_at_energy_urr(e, mat_temp, prn(seed))
                } else {
                    nuc.xs_at_energy(e, mat_temp)
                };
                // ── The reaction draw ─────────────────────────────────
                //
                // Analog: `xi = prn * Sigma_t`, and the ladder below decides
                // fission / capture / scatter, exactly as before #258.
                //
                // Survival biasing: fission sites are banked from the expected
                // production whether or not this collision "is" a fission, the
                // weight is reduced by the absorption probability instead of
                // the particle being killed, and the reaction is drawn from
                // the SCATTERING channels alone —
                // `sample_neutron_reaction` (`src/physics.cpp`) at `afa7a14`.
                // Offsetting the draw by `x.absorption` puts it past every
                // absorption threshold, so the **same** ladder serves both
                // paths and the fission/capture arms are simply unreachable
                // rather than duplicated with different bounds.
                let xi = if vr.survival_biasing {
                    if x.nu_fission > 0.0 && x.total > 0.0 {
                        // `create_fission_sites`: nu_t = wgt/keff * nuSigma_f/Sigma_t.
                        let nu_t = w * x.nu_fission / (k_running * x.total);
                        production += w * x.nu_fission / x.total;
                        let mut n = nu_t as usize;
                        if prn(seed) <= nu_t - n as f64 {
                            n += 1;
                        }
                        for _ in 0..n {
                            let (dx, dy, dz) = isotropic_direction(seed);
                            let e_born = nuc.sample_fission_energy(e, seed);
                            if let Some(t) = tally {
                                score_fission_birth(
                                    batch, t, cell_idx, m, leaf.universe, e, e_born, r, w,
                                );
                            }
                            next_bank.push(Site {
                                r,
                                u: Direction::new(dx, dy, dz),
                                e: e_born,
                            });
                        }
                    }

                    // `absorption()`: w -= w * Sigma_a / Sigma_t.
                    let (w_new, _absorbed) =
                        survival_bias_absorption(w, x.absorption, x.total);
                    w = w_new;

                    // Roulette on the reduced weight. Killing here is correct
                    // and is NOT the analog kill: the expected weight is
                    // preserved by the promotion, which
                    // `physics::variance_reduction` pins with its own test.
                    let (cutoff, survive) = if vr.survival_normalization {
                        (vr.weight_cutoff * w_birth, vr.weight_survive * w_birth)
                    } else {
                        (vr.weight_cutoff, vr.weight_survive)
                    };
                    if w < cutoff {
                        w = russian_roulette(w, survive, seed);
                        if w == 0.0 {
                            track!(TrackEvent::Rouletted, cell_idx, Some(m));
                            break 'history;
                        }
                    }

                    // Draw the outgoing reaction from the SCATTERING channels
                    // only. Offsetting by `x.absorption` lands past every
                    // absorption threshold, so the ladder below is reused
                    // unchanged rather than duplicated with different bounds.
                    let scatter_xs = (x.total - x.absorption).max(0.0);
                    if scatter_xs <= 0.0 {
                        break 'history; // pure absorber: nothing left to scatter
                    }
                    x.absorption + prn(seed) * scatter_xs
                } else {
                    prn(seed) * x.total
                };

                if xi < x.fission {
                    let nu_bar = if x.fission > 0.0 {
                        x.nu_fission / x.fission
                    } else {
                        0.0
                    };
                    production += nu_bar;
                    let n = sample_num_neutrons(nu_bar, k_running, seed);
                    for _ in 0..n {
                        let (dx, dy, dz) = isotropic_direction(seed);
                        let e_born = nuc.sample_fission_energy(e, seed);
                        // Fission-spectrum estimator: chi is MEASURED from the
                        // energies neutrons are actually born with, rather than
                        // assumed from an analytic Watt form.
                        if let Some(t) = tally {
                            score_fission_birth(
                                batch,
                                t,
                                cell_idx,
                                m,
                                leaf.universe,
                                e,
                                e_born,
                                r,
                                w,
                            );
                        }
                        next_bank.push(Site {
                            r,
                            u: Direction::new(dx, dy, dz),
                            e: e_born,
                        });
                    }
                    track!(TrackEvent::Fission, cell_idx, Some(m));
                    break 'history; // fission absorbs the incident neutron
                } else if xi < x.absorption {
                    track!(TrackEvent::Absorption, cell_idx, Some(m));
                    break 'history; // capture
                } else if xi < x.absorption + x.inelastic {
                    let (e2, u2) = match nuc.sample_inelastic(e, seed) {
                        // Discrete level: the CM angular law is the level's own ENDF
                        // MF=4 (op-tm9f). Isotropic-CM only when the evaluation
                        // carries none for this level -- sampling every inelastic
                        // collision isotropically understates <mu>, inflating
                        // Sigma_tr, suppressing leakage and raising k.
                        Inelastic::Level { q, mt } => {
                            match nuc.sample_inelastic_mu_cm(mt, e, seed) {
                                Some(mu_cm) => {
                                    two_body_scatter_with_mu(e, u, nuc.awr, q, mu_cm, seed)
                                }
                                None => two_body_scatter(e, u, nuc.awr, q, seed),
                            }
                        }
                        Inelastic::Continuum { q } => {
                            nuc.sample_inelastic_emission(91, e, u, q, seed)
                        }
                    };
                    // Inelastic is a scattering reaction and belongs in the
                    // nu-scatter matrix. Omitting it leaves the FAST groups
                    // short: inelastic is where a fast neutron loses most of its
                    // energy on a heavy nuclide.
                    if let Some(t) = tally {
                        score_scatter_matrix(batch, t, cell_idx, m, leaf.universe, e, e2, r, w);
                    }
                    e = e2;
                    u = u2;
                } else if xi < x.absorption + x.inelastic + x.n2n {
                    // (n,2n): the MT=16 Q is not carried here, so the cap stays at the
                    // elastic CM energy as before. Sharing the available energy between
                    // the two emitted neutrons is a separate gap (GitHub #192).
                    let has16 = nuc.has_evaluated_emission(16);
                    let (e2, u2) = nuc.sample_inelastic_emission(16, e, u, 0.0, seed);
                    // Second neutron: an **independent draw** from the same
                    // evaluated law. ENDF MF=6 tabulates `f₀` per emitted
                    // neutron, so two independent draws is what the evaluation
                    // means — duplicating the primary's outgoing state (what this
                    // did before, and what it still does with no MF=6 law to
                    // read) correlates the pair perfectly and is GitHub #192's
                    // second open item.
                    let (sec_e2, sec_u2) = if has16 {
                        nuc.sample_inelastic_emission(16, e, u, 0.0, seed)
                    } else {
                        (e2, u2)
                    };
                    // The secondary is drawn above unconditionally and only its
                    // EMISSION is gated, so the yield-2 ablation
                    // (`Nuclide::with_unit_n2n_multiplicity`) leaves both arms'
                    // RNG streams in exact lockstep.
                    if nuc.emits_n2n_secondary() {
                        // Secondary from the same collision: continues the
                        // shared stream at its current position, as before.
                        stack.push((
                            Site {
                                r,
                                u: sec_u2,
                                e: sec_e2,
                            },
                            w,
                            None,
                        ));
                    }
                    // Multiplicity counts: this is a NU-scatter matrix, so every
                    // neutron actually emitted scores its own (E_in, E_out). The
                    // secondary is scored only when emitted, matching its gate.
                    if let Some(t) = tally {
                        score_scatter_matrix(batch, t, cell_idx, m, leaf.universe, e, e2, r, w);
                        if nuc.emits_n2n_secondary() {
                            score_scatter_matrix(
                                batch,
                                t,
                                cell_idx,
                                m,
                                leaf.universe,
                                e,
                                sec_e2,
                                r,
                                w,
                            );
                        }
                    }
                    e = e2;
                    u = u2;
                } else if xi < x.absorption + x.inelastic + x.n2n + x.n3n {
                    // (n,3n): yield 3 -- the primary down-scatters and TWO extra neutrons
                    // are emitted. Before 2026-09-16 there was no branch here at all:
                    // MT=17 is inside MT=1, so the collision still happened but fell
                    // through to the ELASTIC arm and both extras were silently lost.
                    //
                    // Below the MT=17 threshold `x.n3n` is exactly 0, so this condition
                    // coincides with the old `else` boundary and the partition is
                    // bit-identical to before -- which is why adding it does not move any
                    // reactor-spectrum result.
                    let has17 = nuc.has_evaluated_emission(17);
                    let (e2, u2) = nuc.sample_inelastic_emission(17, e, u, 0.0, seed);
                    // Two independent draws from the same evaluated law, for the same
                    // reason the (n,2n) pair is drawn independently: ENDF MF=6 tabulates
                    // `f0` per emitted neutron.
                    for _ in 0..2 {
                        let (se, su) = if has17 {
                            nuc.sample_inelastic_emission(17, e, u, 0.0, seed)
                        } else {
                            (e2, u2)
                        };
                        if nuc.emits_n2n_secondary() {
                            stack.push((Site { r, u: su, e: se }, w, None));
                            if let Some(t) = tally {
                                score_scatter_matrix(
                                    batch,
                                    t,
                                    cell_idx,
                                    m,
                                    leaf.universe,
                                    e,
                                    se,
                                    r,
                                    w,
                                );
                            }
                        }
                    }
                    if let Some(t) = tally {
                        score_scatter_matrix(batch, t, cell_idx, m, leaf.universe, e, e2, r, w);
                    }
                    e = e2;
                    u = u2;
                } else if xi < x.absorption + x.inelastic + x.n2n + x.n3n + x.mt5 {
                    // MT=5, "(n,anything)" -- the lumped high-energy channels,
                    // wired 2026-09-17. Same history as the (n,3n) arm above:
                    // inside MT=1, so before this the collision fell through to
                    // the ELASTIC arm, mis-scattering it and dropping its extra
                    // neutrons. Its multiplicity is a TABULATED y(E), not a
                    // fixed integer.
                    //
                    // `x.mt5` is exactly 0 below ~5 MeV, so this coincides with
                    // the previous boundary for any fission spectrum.
                    let n_emit = nuc.sample_mt5_multiplicity(e, seed);
                    let (e2, u2) = nuc.sample_inelastic_emission(5, e, u, 0.0, seed);
                    // Both possible extras drawn unconditionally, so the RNG
                    // stream depends on the law rather than on the sampled
                    // multiplicity.
                    let extras = [
                        nuc.sample_inelastic_emission(5, e, u, 0.0, seed),
                        nuc.sample_inelastic_emission(5, e, u, 0.0, seed),
                    ];
                    // `y(E) = 0` kills the neutron: U-235's MT=5 emits nothing
                    // below ~100 keV, and even at 20 MeV `y = 0.47`. Scattering
                    // it instead would create neutrons the evaluation says do
                    // not exist.
                    if n_emit == 0 {
                        break;
                    }
                    for (se, su) in extras.iter().take(n_emit.saturating_sub(1).min(2)) {
                        stack.push((Site { r, u: *su, e: *se }, w, None));
                        if let Some(t) = tally {
                            score_scatter_matrix(
                                batch,
                                t,
                                cell_idx,
                                m,
                                leaf.universe,
                                e,
                                *se,
                                r,
                                w,
                            );
                        }
                    }
                    if let Some(t) = tally {
                        score_scatter_matrix(batch, t, cell_idx, m, leaf.universe, e, e2, r, w);
                    }
                    e = e2;
                    u = u2;
                } else {
                    // Scattering. Below its cutoff a moderator nuclide carrying an
                    // S(α,β) table thermalizes via the bound-atom law (lab-frame
                    // outgoing energy + cosine, up-scatter allowed); otherwise the
                    // free-gas / anisotropic-elastic kernel applies as before.
                    let (e2, u2) = if let Some((e_out, mu_lab)) = nuc.sample_thermal(e, seed) {
                        (e_out, rotate_direction(u, mu_lab, seed))
                    } else {
                        {
                            // Free-gas: below 400 kT the target's own thermal
                            // motion is sampled, so the neutron can gain energy
                            // and the population has a Maxwellian fixed point
                            // (bead op-50vu). Above it, target-at-rest as before.
                            let kt = nuc.free_gas_kt(temp);
                            let mu_cm = nuc
                                .sample_elastic_mu_cm(e, seed)
                                .unwrap_or_else(|| 2.0 * prn(seed) - 1.0);
                            free_gas_elastic_scatter_dbrc(
                                e,
                                u,
                                nuc.awr,
                                kt,
                                mu_cm,
                                seed,
                                nuc.dbrc_table(),
                            )
                        }
                    };
                    // Scattering-matrix estimator: score the (E_in, E_out) pair
                    // BEFORE `e` is overwritten, so a tally carrying both an
                    // EnergyFilter and an EnergyOutFilter bins one element of
                    // Sigma_s,g->g'. `e2` may exceed `e` -- thermal up-scatter
                    // is real physics here, not an error to clamp away.
                    //
                    // A tally without an outgoing-energy filter is unaffected:
                    // it would bin this event by incoming energy alone, which is
                    // why only ScatterN/Events receive weight and the flux score
                    // is left to the track-length estimator (see
                    // `score_scatter_matrix`).
                    if let Some(t) = tally {
                        score_scatter_matrix(batch, t, cell_idx, m, leaf.universe, e, e2, r, w);
                    }
                    e = e2;
                    u = u2;
                }
                track!(TrackEvent::Scatter, cell_idx, Some(m));
                // `weight_window_checkpoint_collision` — AFTER the outgoing
                // energy is set, because the window is resolved in energy and
                // the particle's importance is that of where it is going, not
                // where it came from.
                weight_window_checkpoint!({
                    track!(TrackEvent::Rouletted, cell_idx, Some(m));
                    break 'history;
                });
            } else {
                // ── Boundary crossing ──────────────────────────────────────
                path_cm += d_bound.distance;
                r = stream(r, u, d_bound.distance);
                match d_bound.crossing {
                    Crossing::Surface(i_surf) => {
                        // `src/particle.cpp:376-385`: a surface-source
                        // crossing is recorded BEFORE the crossing when the
                        // surface carries a boundary condition and AFTER when
                        // it does not.
                        //
                        // That split is not a detail. On a BC surface the
                        // post-crossing state is either gone (vacuum) or
                        // reflected, and neither is what a replay wants; on an
                        // internal surface the post-crossing state IS the
                        // far-side starting point. A first version of this
                        // recorded only the "after" case, and so recorded
                        // **nothing at all** for a vacuum boundary - which is
                        // the surface a two-stage shielding run exists to
                        // record at.
                        let has_bc = !matches!(
                            geom.surfaces[i_surf].bc(),
                            BoundaryType::Transmissive
                        );
                        if has_bc {
                            if let Some(ss) = surface_source.as_deref_mut() {
                                ss.record(SurfaceCrossing {
                                    r,
                                    u,
                                    energy: e,
                                    weight: w,
                                    surface_idx: i_surf,
                                });
                            }
                        }
                        let crossed =
                            geom.cross_surface_in_frame(i_surf, &path, d_bound.coord_level, r, u, seed);
                        if !crossed.alive {
                            // Vacuum leak — `e` is the true escape energy
                            // (unchanged since the last collision).
                            leak_vacuum += 1;
                            score_leak(leak_batch, leak_edges, e, w);
                            track!(TrackEvent::Leak, cell_idx, path.material);
                            break 'history;
                        }
                        r = crossed.r;
                        u = crossed.u;
                        // Carry which SIDE of the surface the particle landed on,
                        // so the next `locate` cannot re-select the cell it just
                        // left (GitHub #168 — see `Geometry::cross_surface`).
                        on_surface = crossed.on_surface;
                        track!(TrackEvent::SurfaceCrossing, cell_idx, path.material);
                        // `surf_source_` with NO boundary condition: record
                        // AFTER the crossing. The far-side state is what a
                        // replayed particle must start from.
                        if !has_bc {
                            if let Some(ss) = surface_source.as_deref_mut() {
                                ss.record(SurfaceCrossing {
                                    r,
                                    u,
                                    energy: e,
                                    weight: w,
                                    surface_idx: i_surf,
                                });
                            }
                        }
                        // `weight_window_checkpoint_surface`.
                        weight_window_checkpoint!({
                            track!(TrackEvent::Rouletted, cell_idx, path.material);
                            break 'history;
                        });
                    }
                    Crossing::Lattice => {
                        r = stream(r, u, NUDGE); // step into the next tile, re-locate
                        on_surface = SurfaceToken::NONE;
                    }
                    Crossing::None => {
                        // Streamed to infinity — a leak at the true escape energy.
                        leak_infinity += 1;
                        score_leak(leak_batch, leak_edges, e, w);
                        track!(TrackEvent::Leak, cell_idx, path.material);
                        break 'history;
                    }
                }
            }
        }
    }
    if let Some(t) = tracks.as_deref_mut() {
        t.finish();
    }
    HistoryOutcome {
        production,
        virtual_collisions,
        collisions,
        lost_locate,
        stuck_events,
        stuck_path_cm,
        stuck_last_e,
        neg_dist,
        neg_level,
        neg_worst,
        neg_from_lattice,
        neg_from_surface,
        leak_vacuum,
        leak_infinity,
    }
}

/// Resample `n` sites uniformly with replacement — crude fixed-size population
/// control for the fission bank each generation.
fn resample(bank: &[Site], n: usize, seed: &mut u64) -> Vec<Site> {
    let len = bank.len();
    (0..n)
        .map(|_| {
            let idx = ((prn(seed) * len as f64) as usize).min(len - 1);
            bank[idx]
        })
        .collect()
}

/// Mean and standard error of the mean (1σ) of the active-generation eigenvalues.
fn mean_and_stderr(k: &[f64]) -> (f64, f64) {
    let n = k.len();
    if n == 0 {
        return (0.0, 0.0);
    }
    let mean = k.iter().sum::<f64>() / n as f64;
    if n < 2 {
        return (mean, 0.0);
    }
    let var = k.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n as f64 - 1.0);
    (mean, (var / n as f64).sqrt())
}

#[cfg(test)]
mod leakage_tests {
    use super::*;
    use crate::geometry::cell::{Cell, HalfSpaceSense, RegionToken};
    use crate::geometry::surface::{BoundaryType, Sphere, SurfaceKind};
    use crate::geometry::universe::Universe;
    use crate::material::material::{Material, NuclideComponent};
    use crate::material::nuclide::Nuclide;
    use crate::physics::keff::KeffSettings;
    use crate::tally::filter::{EnergyFilter, FilterKind, MaterialFilter};
    use crate::tally::tally::{ScoreType, Tally, TallyBin};

    /// Godiva (HEU-MET-FAST-001) LOW-tier material + nuclide array, embedded data.
    fn godiva() -> (Vec<Material>, Vec<Nuclide>) {
        let nuclides = vec![
            Nuclide::from_core("U234").unwrap(),
            Nuclide::from_core("U235").unwrap(),
            Nuclide::from_core("U238").unwrap(),
        ];
        let m = Material {
            id: 1,
            name: "Godiva".into(),
            temperature: 293.6,
            components: vec![
                NuclideComponent {
                    nuclide_idx: 0,
                    atom_density: 4.9184e-4,
                },
                NuclideComponent {
                    nuclide_idx: 1,
                    atom_density: 4.4994e-2,
                },
                NuclideComponent {
                    nuclide_idx: 2,
                    atom_density: 2.4984e-3,
                },
            ],
        };
        (vec![m], nuclides)
    }

    /// Single HEU sphere of radius `r_cm` with boundary condition `bc`.
    fn heu_sphere(r_cm: f64, bc: BoundaryType) -> Geometry {
        Geometry {
            surfaces: vec![SurfaceKind::Sphere(Sphere {
                x0: 0.0,
                y0: 0.0,
                z0: 0.0,
                r: r_cm,
                bc,
            })],
            cells: vec![Cell::material(
                1,
                vec![RegionToken::HalfSpace {
                    surface_idx: 0,
                    sense: HalfSpaceSense::Inside,
                }],
                0,
                293.6,
            )],
            universes: vec![Universe {
                id: 0,
                cell_indices: vec![0],
            }],
            lattices: vec![],
            root_universe: 0,
        }
    }

    /// A 2-group (thermal/fast) energy × 1-material flux tally + a matching
    /// coarse leak grid, as the reactor-physics helper will build them.
    fn tally_and_leak_grid() -> (Tally, Vec<f64>) {
        let edges = vec![0.0, 0.625, 2.0e7];
        let tally = Tally {
            id: 0,
            name: "rp".into(),
            filters: vec![
                FilterKind::Energy(EnergyFilter {
                    bins: edges.clone(),
                }),
                FilterKind::Material(MaterialFilter {
                    material_indices: vec![0],
                }),
            ],
            scores: vec![ScoreType::Flux, ScoreType::Absorption, ScoreType::NuFission],
            bins: vec![TallyBin::default(); 2 * 1 * 3],
        };
        (tally, edges)
    }

    fn settings() -> KeffSettings {
        KeffSettings {
            n_particles: 400,
            n_inactive: 10,
            n_active: 20,
            ..KeffSettings::default()
        }
    }

    fn src() -> SourceBox {
        SourceBox {
            lower: Position::new(-3.0, -3.0, -3.0),
            upper: Position::new(3.0, 3.0, 3.0),
        }
    }

    /// Total leakage rate = mean over active generations of the summed leak
    /// spectrum, per source neutron.
    fn total_leak(bins: &[TallyBin], n_active: u64) -> f64 {
        bins.iter().map(|b| b.mean(n_active)).sum::<f64>() / 400.0
    }

    /// A **reflective** HEU sphere has no escape path, so the tallied leakage
    /// spectrum is ~0 (only pathological stuck histories could contribute, and
    /// HEU absorbs, so there are none).
    #[test]
    fn reflective_sphere_has_no_leakage() {
        let (mats, nucs) = godiva();
        let geom = heu_sphere(8.0, BoundaryType::Reflective);
        let (mut tally, edges) = tally_and_leak_grid();
        let mut leak = vec![TallyBin::default(); edges.len() - 1];
        let s = settings();
        let res = run_keff_csg_reactor_physics(
            &geom,
            &mats,
            &nucs,
            src(),
            &s,
            &mut tally,
            &edges,
            &mut leak,
        );
        assert!(
            res.k_mean > 0.5,
            "reflective HEU sphere should be supercritical-ish, k={}",
            res.k_mean
        );
        let leaked = total_leak(&leak, s.n_active as u64);
        assert!(
            leaked < 1.0e-6,
            "reflective sphere leaked {leaked} per source neutron (expected ~0)"
        );
        // Each active generation recorded one realization per leak bin.
        assert_eq!(leak[0].count, s.n_active as u64);
    }

    /// A **vacuum** HEU sphere below critical size leaks a substantial fraction
    /// of its neutrons; the tallied leakage spectrum must be strictly positive
    /// and populate the fast group (fission-source energy).
    #[test]
    fn vacuum_sphere_leaks() {
        let (mats, nucs) = godiva();
        let geom = heu_sphere(6.0, BoundaryType::Vacuum); // < Godiva critical radius
        let (mut tally, edges) = tally_and_leak_grid();
        let mut leak = vec![TallyBin::default(); edges.len() - 1];
        let s = settings();
        let _res = run_keff_csg_reactor_physics(
            &geom,
            &mats,
            &nucs,
            src(),
            &s,
            &mut tally,
            &edges,
            &mut leak,
        );
        let leaked = total_leak(&leak, s.n_active as u64);
        assert!(
            leaked > 0.05,
            "sub-critical vacuum HEU sphere should leak appreciably, got {leaked} per source neutron"
        );
        // Fast group (bin 1) carries leakage — neutrons escape near birth energy.
        assert!(
            leak[1].mean(s.n_active as u64) > 0.0,
            "fast-group leakage must be positive"
        );
    }

    /// `run_keff_csg` (no leakage sink) is byte-identical to before — the new
    /// path is opt-in only. Cross-check the single-thread eigenvalue is stable.
    #[test]
    fn plain_run_keff_csg_unchanged() {
        let (mats, nucs) = godiva();
        let geom = heu_sphere(8.0, BoundaryType::Vacuum);
        let s = settings();
        let a = run_keff_csg(&geom, &mats, &nucs, src(), &s, None);
        let b = run_keff_csg(&geom, &mats, &nucs, src(), &s, None);
        assert_eq!(
            a.k_by_generation, b.k_by_generation,
            "single-thread run_keff_csg must be bit-reproducible"
        );
    }
}
