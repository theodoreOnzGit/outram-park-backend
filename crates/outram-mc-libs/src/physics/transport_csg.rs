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

use crate::geometry::cell::SurfaceToken;
use crate::geometry::geometry::{Crossing, Geometry};
use crate::geometry::position::{stream, Direction, Position};
use crate::material::material::Material;
use crate::material::nuclide::{Inelastic, Nuclide};
use crate::physics::compute::{ComputeType, ThreadCount};
use crate::physics::fission::sample_num_neutrons;
use crate::physics::keff::{KeffResult, KeffSettings};
use crate::physics::scatter::{
    free_gas_elastic_scatter, K_BOLTZMANN_EV_PER_K, continuum_inelastic_scatter, rotate_direction,
    two_body_scatter,
};
use crate::rng::distributions::isotropic_direction;
use crate::rng::lcg::{future_seed, prn};
use crate::tally::scoring::{flush_batch, flush_bins, score_track_length};
use crate::tally::tally::{Tally, TallyBin};

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
pub fn run_keff_csg(
    geom: &Geometry,
    materials: &[Material],
    nuclides: &[Nuclide],
    source_box: SourceBox,
    settings: &KeffSettings,
    tally: Option<&mut Tally>,
) -> KeffResult {
    match settings.compute {
        ComputeType::CpuSingleThread => run_keff_csg_seq(
            geom,
            materials,
            nuclides,
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
                production += transport_history(
                    *site,
                    geom,
                    materials,
                    nuclides,
                    temp,
                    k_running,
                    &mut next_bank,
                    &mut seed,
                    tally_def,
                    &mut batch,
                    leak_edges_gen,
                    &mut leak_batch,
                );
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
        k_by_generation.push(k_gen);
        k_running = k_gen.max(1.0e-6);
        if active {
            active_k.push(k_gen);
        }

        if next_bank.is_empty() {
            break;
        }
        source = resample(&next_bank, settings.n_particles, &mut seed);
    }

    let (k_mean, k_std) = mean_and_stderr(&active_k);
    KeffResult {
        k_mean,
        k_std,
        k_by_generation,
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
            let results: Vec<(f64, Vec<Site>, Vec<f64>, Vec<f64>)> = {
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
                        let production = transport_history(
                            source[hist_idx],
                            geom,
                            materials,
                            nuclides,
                            temp,
                            k_running,
                            &mut local_bank,
                            &mut seed,
                            tally_def,
                            &mut local_batch,
                            leak_edges_gen,
                            &mut local_leak,
                        );
                        (production, local_bank, local_batch, local_leak)
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
            for (prod, bank, local_batch, local_leak) in results {
                production += prod;
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
pub(crate) fn transport_history(
    site: Site,
    geom: &Geometry,
    materials: &[Material],
    nuclides: &[Nuclide],
    temp: f64,
    k_running: f64,
    next_bank: &mut Vec<Site>,
    seed: &mut u64,
    tally: Option<&Tally>,
    batch: &mut [f64],
    leak_edges: &[f64],
    leak_batch: &mut [f64],
) -> f64 {
    const NUDGE: f64 = 1.0e-9;
    let mut production = 0.0;
    let mut stack: Vec<Site> = vec![site];

    // Safety cap on events per history: a particle in a purely-scattering
    // reflective medium with vanishing absorption could otherwise bounce forever.
    // 100k events is far beyond any physical history (mean ~tens of collisions).
    const MAX_EVENTS: u32 = 100_000;

    while let Some(start) = stack.pop() {
        let mut r = start.r;
        let mut u = start.u;
        let mut e = start.e;
        let mut on_surface = SurfaceToken::NONE;
        let mut events = 0u32;

        'history: loop {
            events += 1;
            if events > MAX_EVENTS {
                // Stuck history (should be vanishingly rare) — count it as a
                // leak at its last-known energy so the neutron balance still
                // closes. A small mis-binned population if it ever fires.
                score_leak(leak_batch, leak_edges, e, 1.0);
                break 'history;
            }
            // Locate: which cell/material are we in?
            let Some(path) = geom.locate(r, u, on_surface) else {
                // Lost the particle (numerical edge case) — treat as a leak at
                // last-known energy, same reasoning as the MAX_EVENTS arm.
                score_leak(leak_batch, leak_edges, e, 1.0);
                break 'history;
            };
            let leaf = *path.leaf();
            let cell_idx = leaf.cell;

            let sigma_t = match path.material {
                Some(m) => materials[m].macro_xs_total(e, nuclides),
                None => 0.0, // void: stream freely to the next boundary
            };

            let d_bound = geom.distance_to_boundary(&path);
            let d_col = if sigma_t > 0.0 {
                -prn(seed).max(f64::MIN_POSITIVE).ln() / sigma_t
            } else {
                f64::INFINITY
            };

            // ── Track-length tally scoring ─────────────────────────────────
            // The particle streams `seg = min(d_col, d_bound)` through the current
            // cell at the current (pre-collision) energy `e`; deposit its
            // track-length flux/reaction-rate contribution before either the
            // collision changes `e` or the crossing changes the cell. `seg` is
            // finite even in a void (d_col = ∞ ⇒ seg = the boundary distance);
            // there `macro_xs` is `None`, so only the flux score deposits.
            if let Some(t) = tally {
                let seg = d_col.min(d_bound.distance);
                let mxs = path.material.map(|m| materials[m].macro_xs(e, nuclides));
                let mat_idx = path.material.unwrap_or(usize::MAX);
                // Spatial filters (mesh / Legendre) bin on the segment midpoint
                // `r + 0.5·seg·u` — the track-length-representative point of the
                // free flight (constant energy, single cell over the segment).
                let mid = stream(r, u, 0.5 * seg);
                score_track_length(
                    batch,
                    t,
                    cell_idx,
                    mat_idx,
                    leaf.universe,
                    e,
                    seg,
                    mid,
                    mxs.as_ref(),
                    1.0,
                );
            }

            if d_col < d_bound.distance {
                // ── Collision ──────────────────────────────────────────────
                r = stream(r, u, d_col);
                on_surface = SurfaceToken::NONE;
                let m = path.material.expect("collision requires a material");
                let material = &materials[m];

                // Analog reaction partition (mirrors keff.rs transport_history).
                let ci = material.sample_nuclide(e, seed, nuclides);
                let nuc = &nuclides[material.components[ci].nuclide_idx];
                let x = nuc.xs_at_energy(e, temp);
                let xi = prn(seed) * x.total;
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
                        next_bank.push(Site {
                            r,
                            u: Direction::new(dx, dy, dz),
                            e: nuc.sample_fission_energy(e, seed),
                        });
                    }
                    break 'history; // fission absorbs the incident neutron
                } else if xi < x.absorption {
                    break 'history; // capture
                } else if xi < x.absorption + x.inelastic {
                    let (e2, u2) = match nuc.sample_inelastic(e, seed) {
                        Inelastic::Level { q } => two_body_scatter(e, u, nuc.awr, q, seed),
                        Inelastic::Continuum => continuum_inelastic_scatter(e, u, nuc.awr, seed),
                    };
                    e = e2;
                    u = u2;
                } else if xi < x.absorption + x.inelastic + x.n2n {
                    let (e2, u2) = continuum_inelastic_scatter(e, u, nuc.awr, seed);
                    stack.push(Site { r, u: u2, e: e2 });
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
                            let kt = K_BOLTZMANN_EV_PER_K * temp;
                            let mu_cm = nuc
                                .sample_elastic_mu_cm(e, seed)
                                .unwrap_or_else(|| 2.0 * prn(seed) - 1.0);
                            free_gas_elastic_scatter(e, u, nuc.awr, kt, mu_cm, seed)
                        }
                    };
                    e = e2;
                    u = u2;
                }
            } else {
                // ── Boundary crossing ──────────────────────────────────────
                r = stream(r, u, d_bound.distance);
                match d_bound.crossing {
                    Crossing::Surface(i_surf) => {
                        let crossed = geom.cross_surface(i_surf, r, u);
                        if !crossed.alive {
                            // Vacuum leak — `e` is the true escape energy
                            // (unchanged since the last collision).
                            score_leak(leak_batch, leak_edges, e, 1.0);
                            break 'history;
                        }
                        r = crossed.r;
                        u = crossed.u;
                        // Carry which SIDE of the surface the particle landed on,
                        // so the next `locate` cannot re-select the cell it just
                        // left (GitHub #168 — see `Geometry::cross_surface`).
                        on_surface = crossed.on_surface;
                    }
                    Crossing::Lattice => {
                        r = stream(r, u, NUDGE); // step into the next tile, re-locate
                        on_surface = SurfaceToken::NONE;
                    }
                    Crossing::None => {
                        // Streamed to infinity — a leak at the true escape energy.
                        score_leak(leak_batch, leak_edges, e, 1.0);
                        break 'history;
                    }
                }
            }
        }
    }
    production
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
    use crate::tally::filter::{EnergyFilter, MaterialFilter};
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
                Box::new(EnergyFilter {
                    bins: edges.clone(),
                }),
                Box::new(MaterialFilter {
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
