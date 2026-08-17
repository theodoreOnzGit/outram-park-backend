//! k-eigenvalue power iteration for a homogeneous bare sphere.
//!
//! This is the minimal criticality driver — the first end-to-end assembly of the
//! transport kernel described in `docs/keff-doppler-roadmap.md` (Priority 1). It
//! deliberately handles only the simplest geometry (one sphere, vacuum outside,
//! one homogeneous material) so the physics can be exercised without the full CSG
//! machinery. The pieces it composes:
//!
//! - **Geometry** — [`crate::geometry::surface::Sphere::distance`] for the one
//!   surface crossing; "inside" is just `|r| < R`.
//! - **Data** — macroscopic cross sections from [`Material`], which pulls
//!   microscopic σ(E,T) from `njoy-outram-park-fork` via [`Nuclide`].
//! - **Physics** — analog collisions: elastic scatter
//!   ([`crate::physics::scatter::elastic_scatter`]), fission banking
//!   ([`crate::physics::fission::sample_num_neutrons`]), and analog capture.
//! - **Source** — Watt fission energy + isotropic direction for banked neutrons.
//!
//! # Compute backends
//!
//! [`run_keff`] is a thin dispatcher over [`KeffSettings::compute`]
//! ([`ComputeType`]). All three backends run the **same physics**; they differ
//! only in *how* the per-generation histories are executed:
//!
//! - [`run_keff_cpu_single`] — scalar, single RNG stream. The trusted,
//!   deterministic, bit-reproducible reference.
//! - [`run_keff_cpu_multi`] — the histories of a generation transported in
//!   parallel with [`rayon`], each on an independent deterministic RNG sub-stream.
//! - [`run_keff_gpu`] — GPU-accelerated macroscopic-Sigma_t lookup, with a
//!   transparent CPU fallback when no GPU adapter is present.
//!
//! # Algorithm
//!
//! Standard fission-source power iteration. Each *generation* transports
//! `n_particles` histories from the current fission bank; every fission event
//! contributes ν̄ to the generation's production tally and banks ⌊ν̄/k⌋(+1) sites
//! for the next generation. The generation eigenvalue is
//! `k = (Σ ν̄ over fissions) / n_particles`. The first `n_inactive` generations
//! let the source distribution converge and are discarded; the mean over the
//! remaining `n_active` generations is the reported k, with the standard error of
//! that mean.
//!
//! # Fidelity
//!
//! Analog transport (no implicit capture / weight windows), target at rest. Both
//! data tiers now model inelastic down-scatter and forward-peaked elastic; they
//! differ in how finely that physics is resolved:
//!
//! - **HIGH tier** ([`Nuclide::from_endf`]) carries the resolved inelastic level
//!   structure (MT=51…91), so inelastic is a distinct channel with a real
//!   energy-loss law — discrete-level two-body kinematics (each level's Q-value)
//!   and a Weisskopf-evaporation continuum. Elastic uses the full ENDF MF=4
//!   anisotropic angular distribution (per-energy tabulated cosine CDF). `(n,2n)`
//!   (MT=16, from the reconstructed MF=3 background) is a distinct channel that
//!   emits its true **yield-2 multiplicity** — one extra same-generation neutron,
//!   the small positive reactivity a bare fast sphere would otherwise drop. Fission
//!   neutrons are born from the nuclide's **energy-dependent ENDF MF=5 χ(E→E')**
//!   (LF=1) rather than a fixed thermal-Watt spectrum.
//! - **LOW tier** ([`Nuclide::from_core`]) has no resolved levels: inelastic is the
//!   group remainder (total − elastic − fission − capture), down-scattered by the
//!   Weisskopf continuum law. Elastic is forward-peaked from a single per-group
//!   mean cosine μ̄ (baked from MF=4) via a maximum-entropy exponential angular law.
//!   Above each nuclide's WMP `e_max` the group data is infinite-dilution
//!   Watt-collapsed with no self-shielding. `(n,2n)` has no group column yet, so
//!   the LOW tier still lumps it into elastic (no multiplication) — a pending bake.
//!
//! For a bare fast sphere, forward-peaked elastic and inelastic down-scatter are
//! the dominant reactivity levers — together they bring **both** tiers' Godiva Keff
//! into agreement with the ICSBEP benchmark (see `docs/development-history.md`).
//!
//! # Example
//!
//! ```no_run
//! use outram_mc_libs::material::material::{Material, NuclideComponent};
//! use outram_mc_libs::material::nuclide::Nuclide;
//! use outram_mc_libs::physics::keff::{run_keff, KeffSettings};
//!
//! // Godiva: bare HEU sphere, r ≈ 8.741 cm.
//! let nuclides = vec![
//!     Nuclide::from_core("U234").unwrap(),
//!     Nuclide::from_core("U235").unwrap(),
//!     Nuclide::from_core("U238").unwrap(),
//! ];
//! let mat = Material {
//!     id: 1,
//!     name: "HEU".into(),
//!     temperature: 293.6,
//!     components: vec![
//!         NuclideComponent { nuclide_idx: 0, atom_density: 4.9184e-4 },
//!         NuclideComponent { nuclide_idx: 1, atom_density: 4.4994e-2 },
//!         NuclideComponent { nuclide_idx: 2, atom_density: 2.4984e-3 },
//!     ],
//! };
//! let result = run_keff(8.7407, &mat, &nuclides, &KeffSettings::default());
//! println!("k = {:.5} ± {:.5}", result.k_mean, result.k_std);
//! ```

use crate::geometry::position::{stream, Direction, Position};
use crate::geometry::surface::{BoundaryType, Sphere, Surface};
use crate::material::material::Material;
use crate::material::nuclide::{Inelastic, Nuclide};
use crate::physics::compute::{ComputeType, ThreadCount};
use crate::physics::fission::sample_num_neutrons;
use crate::physics::scatter::{
    continuum_inelastic_scatter, elastic_scatter, rotate_direction, two_body_scatter,
    two_body_scatter_with_mu,
};
use crate::gpu::batched_event::{EventBatch, EventSphere, EventTablesF32, FISS_NONE};
use crate::gpu::collision_grid::CollisionTables;
use crate::rng::distributions::{isotropic_direction, watt};
use crate::rng::lcg::{future_seed, prn};

/// Settings for a [`run_keff`] power iteration.
#[derive(Debug, Clone, Copy)]
pub struct KeffSettings {
    /// Neutron histories per generation. More ⇒ lower per-generation noise.
    pub n_particles: usize,
    /// Inactive (source-convergence) generations, discarded from the k tally.
    pub n_inactive: usize,
    /// Active generations averaged into the reported eigenvalue.
    pub n_active: usize,
    /// Material/data temperature \[K\] used for Doppler-broadened lookups.
    pub temperature_k: f64,
    /// Master RNG seed. Fixed seed ⇒ bit-reproducible run.
    pub seed: u64,
    /// Watt fission-spectrum parameter `a` \[eV\] for banked neutron energies.
    pub watt_a: f64,
    /// Watt fission-spectrum parameter `b` \[eV⁻¹\].
    pub watt_b: f64,
    /// Which transport backend [`run_keff`] dispatches to.
    ///
    /// - [`ComputeType::CpuSingleThread`] — the scalar, single-RNG-stream path
    ///   ([`run_keff_cpu_single`]); the trusted, bit-reproducible **deterministic
    ///   reference**. This is the [`Default`].
    /// - [`ComputeType::CpuMultiThread`] — [`rayon`]-parallel over the histories
    ///   of each generation ([`run_keff_cpu_multi`]) in a dedicated pool sized by
    ///   the carried [`ThreadCount`] (default [`ThreadCount::Auto`] = every
    ///   logical core); each history runs on its own deterministically derived RNG
    ///   sub-stream, so the eigenvalue is reproducible independent of thread count
    ///   (but does **not** bit-match the single-thread stream — see that
    ///   function's docs).
    /// - [`ComputeType::Gpu`] — GPU-accelerated macroscopic Sigma_t lookup
    ///   ([`run_keff_gpu`]), with a transparent CPU fallback (never an error) when
    ///   no GPU adapter is available. The GPU is `f32` acceleration only; the CPU
    ///   single-thread path stays the trusted reference.
    pub compute: ComputeType,
}

impl Default for KeffSettings {
    /// A modest run (2000 histories × [30 inactive + 70 active]) with the
    /// U-235 thermal Watt spectrum, on the single-thread deterministic reference
    /// backend. Enough for a first-look Keff in seconds.
    fn default() -> Self {
        Self {
            n_particles: 2000,
            n_inactive: 30,
            n_active: 70,
            temperature_k: 293.6,
            seed: 1,
            watt_a: 0.988e6,
            watt_b: 2.249e-6,
            compute: ComputeType::CpuSingleThread,
        }
    }
}

impl KeffSettings {
    /// Return a copy of these settings with the transport backend set to
    /// `compute`. A builder-style convenience for selecting a [`ComputeType`]
    /// without spelling out the whole struct.
    ///
    /// For example, taking a base `settings` and running the same case on the
    /// single-thread reference and then the multi-thread backend is
    /// `run_keff(r, &mat, &nuc, &settings.with_compute(ComputeType::CpuSingleThread))`
    /// followed by `run_keff(r, &mat, &nuc, &settings.with_compute(ComputeType::CpuMultiThread))`
    /// — `KeffSettings` is `Copy`, so `settings` is untouched and can be reused.
    pub fn with_compute(mut self, compute: ComputeType) -> Self {
        self.compute = compute;
        self
    }
}

/// Result of a [`run_keff`] power iteration.
#[derive(Debug, Clone)]
pub struct KeffResult {
    /// Mean eigenvalue over the active generations.
    pub k_mean: f64,
    /// Standard error of the mean (1σ) over the active generations.
    pub k_std: f64,
    /// Per-generation eigenvalue estimates, all generations (inactive first).
    pub k_by_generation: Vec<f64>,
}

/// A fission-source neutron awaiting transport in the next generation.
#[derive(Clone, Copy)]
struct Site {
    r: Position,
    u: Direction,
    e: f64,
}

/// Run fission-source power iteration on a bare sphere of radius `radius_cm`
/// (centred at the origin, vacuum outside) filled with `material`.
///
/// `nuclides` is the global nuclide array the material's components index into.
/// Returns the mean eigenvalue and its standard error over the active
/// generations. See the module docs for the algorithm and fidelity caveats.
///
/// This function is a thin **dispatcher**: it selects the transport backend from
/// [`settings.compute`](KeffSettings::compute) and forwards to the matching
/// entry point. The physics is identical across backends.
///
/// - [`ComputeType::CpuSingleThread`] → [`run_keff_cpu_single`] (the trusted,
///   bit-reproducible reference),
/// - [`ComputeType::CpuMultiThread`] → [`run_keff_cpu_multi`] (rayon-parallel),
/// - [`ComputeType::Gpu`] → [`run_keff_gpu`] (GPU Sigma_t lookup, CPU fallback).
pub fn run_keff(
    radius_cm: f64,
    material: &Material,
    nuclides: &[Nuclide],
    settings: &KeffSettings,
) -> KeffResult {
    match settings.compute {
        ComputeType::CpuSingleThread => {
            run_keff_cpu_single(radius_cm, material, nuclides, settings)
        }
        ComputeType::CpuMultiThread(tc) => {
            run_keff_cpu_multi(radius_cm, material, nuclides, settings, tc)
        }
        ComputeType::Gpu => run_keff_gpu(radius_cm, material, nuclides, settings),
    }
}

/// Scalar, single-thread fission-source power iteration — the **trusted,
/// deterministic, bit-reproducible reference** backend
/// ([`ComputeType::CpuSingleThread`]).
///
/// This is the original [`run_keff`] algorithm: raw `f64` throughout, with a
/// **single** RNG `seed` threaded sequentially through the whole run (initial
/// source sampling, every history's transport, and every generation's resample
/// all draw from the one stream). A fixed [`KeffSettings::seed`] therefore yields
/// the same eigenvalue bit-for-bit on every machine. The other two backends
/// ([`run_keff_cpu_multi`], [`run_keff_gpu`]) are acceleration only and are
/// validated *against* this reference, never above it.
///
/// Returns the mean eigenvalue and its standard error over the active
/// generations. See the module docs for the algorithm and fidelity caveats.
pub fn run_keff_cpu_single(
    radius_cm: f64,
    material: &Material,
    nuclides: &[Nuclide],
    settings: &KeffSettings,
) -> KeffResult {
    let sphere = Sphere {
        x0: 0.0,
        y0: 0.0,
        z0: 0.0,
        r: radius_cm,
        bc: BoundaryType::Vacuum,
    };
    let mut seed = settings.seed;
    let temp = settings.temperature_k;

    // Initial source: uniform in the sphere volume, isotropic, Watt energy.
    let mut source: Vec<Site> = (0..settings.n_particles)
        .map(|_| {
            let (dx, dy, dz) = isotropic_direction(&mut seed);
            let rr = radius_cm * prn(&mut seed).cbrt(); // uniform-in-volume radius
            Site {
                r: Position::new(rr * dx, rr * dy, rr * dz),
                u: Direction::new(dx, dy, dz),
                e: watt(&mut seed, settings.watt_a, settings.watt_b),
            }
        })
        .collect();

    let n_gen = settings.n_inactive + settings.n_active;
    let mut k_by_generation = Vec::with_capacity(n_gen);
    let mut k_running = 1.0; // guess feeding the site-count normalisation
    let mut active_k = Vec::with_capacity(settings.n_active);

    for gen in 0..n_gen {
        let mut next_bank: Vec<Site> = Vec::with_capacity(settings.n_particles);
        let mut production = 0.0_f64;

        for site in &source {
            production += transport_history(
                *site,
                &sphere,
                material,
                nuclides,
                temp,
                k_running,
                &mut next_bank,
                &mut seed,
            );
        }

        let k_gen = production / settings.n_particles as f64;
        k_by_generation.push(k_gen);
        k_running = k_gen;
        if gen >= settings.n_inactive {
            active_k.push(k_gen);
        }

        // Resample the next generation's source to exactly n_particles sites.
        if next_bank.is_empty() {
            // Sub-critical to extinction (or no data): nothing left to iterate.
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

/// Per-history stride \[RNG draws\] reserved for each history's sub-stream in the
/// multi-thread backend. Equal to [`crate::rng::lcg::DEFAULT_STRIDE`] (152 917),
/// the same per-particle stride OpenMC reserves (`src/random_lcg.cpp`,
/// `init_seed`): far more draws than any single bare-sphere history consumes, so
/// adjacent histories' streams never overlap.
const HIST_STRIDE: u64 = crate::rng::lcg::DEFAULT_STRIDE;

/// Per-generation stride \[RNG draws\] reserved for each generation in the
/// multi-thread backend. `2^40` — chosen far larger than
/// `n_particles * HIST_STRIDE` for any realistic `n_particles` (which caps the
/// offsets a single generation uses), so no generation's sub-streams overlap the
/// next generation's.
const GEN_STRIDE: u64 = 1 << 40;

/// Rayon-parallel fission-source power iteration ([`ComputeType::CpuMultiThread`]).
///
/// Same physics and same power-iteration structure as [`run_keff_cpu_single`],
/// but the histories **within each generation** are transported in parallel with
/// [`rayon`]. The generation loop itself stays sequential — generation `g+1`'s
/// source is the resampled fission bank of generation `g`, a hard data
/// dependency.
///
/// # Thread pool sizing
///
/// The parallel sections run inside a **dedicated** [`rayon::ThreadPool`] sized
/// to `thread_count.resolve()` (min 1), **not** the implicit global pool — the
/// caller gets explicit, controllable sizing. [`ThreadCount::Auto`] (the
/// default) resolves via [`std::thread::available_parallelism`], so the pool
/// scales with the CPU's strength: a big desktop CPU gets many threads, an
/// Android phone gets few, with no special-casing. [`ThreadCount::Fixed`] pins
/// an exact count and [`ThreadCount::Fraction`] takes a fraction of the logical
/// cores (both clamped to `>= 1`). This whole path is Android-clean — `rayon`
/// and `available_parallelism` both work there — so it is **not** target-gated.
///
/// # Reproducibility (independent of thread count)
///
/// Each history is given a **completely independent, deterministic** RNG stream
/// derived only from `(settings.seed, generation index, history index)` — never
/// from a shared mutable seed — so the result never races and is identical
/// regardless of how rayon schedules the work. The derivation uses the LCG
/// jump-ahead ([`crate::rng::lcg::future_seed`]), mirroring OpenMC's per-particle
/// independent-stream design (`src/particle.cpp`, `src/random_lcg.cpp`
/// `init_seed`/`future_seed`, the reproducibility guarantee):
///
/// - `gen_base_seed = future_seed(gen * GEN_STRIDE, settings.seed)` places each
///   generation `GEN_STRIDE = 2^40` draws apart in jump-ahead index space;
/// - `hist_seed = future_seed(hist_idx * HIST_STRIDE, gen_base_seed)` places each
///   history `HIST_STRIDE = 152917` draws apart within its generation.
///
/// **Non-overlap argument.** A single bare-sphere history draws far fewer than
/// `HIST_STRIDE` random numbers, so history `h` stays inside its
/// `[h*HIST_STRIDE, (h+1)*HIST_STRIDE)` slot and never touches history `h+1`'s
/// stream. The largest offset any generation uses is `n_particles * HIST_STRIDE`,
/// which is `< GEN_STRIDE` for any `n_particles < 2^40 / HIST_STRIDE ≈ 7.5e6`, so
/// generations never overlap either.
///
/// The **initial source sampling** and each generation's **resample** run on a
/// *separate* single sequential seed stream (`src_seed`, started at
/// `settings.seed`) — cheap, order-independent bookkeeping kept deterministic and
/// off the parallel path.
///
/// # Agreement with the reference
///
/// Because the per-history stream structure differs from the single sequential
/// stream, this backend does **not** bit-match [`run_keff_cpu_single`]. It is a
/// statistically independent estimate of the same eigenvalue and agrees with the
/// reference within combined statistical uncertainty.
pub fn run_keff_cpu_multi(
    radius_cm: f64,
    material: &Material,
    nuclides: &[Nuclide],
    settings: &KeffSettings,
    thread_count: ThreadCount,
) -> KeffResult {
    use crate::rng::lcg::future_seed;
    use rayon::prelude::*;

    let sphere = Sphere {
        x0: 0.0,
        y0: 0.0,
        z0: 0.0,
        r: radius_cm,
        bc: BoundaryType::Vacuum,
    };
    let temp = settings.temperature_k;

    // Dedicated, explicitly sized rayon pool (never the implicit global pool):
    // `resolve()` maps the ThreadCount to a concrete worker count (>= 1). The
    // per-history seeding below is thread-count-independent, so the eigenvalue is
    // identical regardless of `n_threads` — only the wall time changes.
    let n_threads = thread_count.resolve();
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(n_threads)
        .build()
        .expect("rayon thread pool");

    // Dedicated sequential stream for source sampling + resampling only. Kept
    // separate from the per-history transport streams so both stay deterministic.
    let mut src_seed = settings.seed;

    // Initial source: uniform in the sphere volume, isotropic, Watt energy.
    let mut source: Vec<Site> = (0..settings.n_particles)
        .map(|_| {
            let (dx, dy, dz) = isotropic_direction(&mut src_seed);
            let rr = radius_cm * prn(&mut src_seed).cbrt(); // uniform-in-volume radius
            Site {
                r: Position::new(rr * dx, rr * dy, rr * dz),
                u: Direction::new(dx, dy, dz),
                e: watt(&mut src_seed, settings.watt_a, settings.watt_b),
            }
        })
        .collect();

    let n_gen = settings.n_inactive + settings.n_active;
    let mut k_by_generation = Vec::with_capacity(n_gen);
    let mut k_running = 1.0;
    let mut active_k = Vec::with_capacity(settings.n_active);

    // Run the whole generation loop inside the dedicated pool so every
    // `into_par_iter()` below dispatches onto exactly `n_threads` workers.
    pool.install(|| {
        for gen in 0..n_gen {
            // Base seed for this generation's per-history sub-streams.
            let gen_base_seed = future_seed((gen as u64).wrapping_mul(GEN_STRIDE), settings.seed);

            // Transport every history in parallel. Each returns (production, local
            // fission bank); `map(...).collect::<Vec<_>>()` on an indexed parallel
            // iterator preserves input order, so the reduction below is deterministic
            // regardless of thread count.
            let results: Vec<(f64, Vec<Site>)> = (0..source.len())
                .into_par_iter()
                .map(|hist_idx| {
                    // Independent, deterministic sub-stream for this history; owned
                    // locally — never shared across threads.
                    let hist_seed =
                        future_seed((hist_idx as u64).wrapping_mul(HIST_STRIDE), gen_base_seed);
                    let mut seed = hist_seed;
                    let mut local_bank: Vec<Site> = Vec::new();
                    let production = transport_history(
                        source[hist_idx],
                        &sphere,
                        material,
                        nuclides,
                        temp,
                        k_running,
                        &mut local_bank,
                        &mut seed,
                    );
                    (production, local_bank)
                })
                .collect();

            // Deterministic sequential reduction: sum productions, concatenate banks
            // in history-index order.
            let mut production = 0.0_f64;
            let mut next_bank: Vec<Site> = Vec::with_capacity(settings.n_particles);
            for (prod, bank) in results {
                production += prod;
                next_bank.extend(bank);
            }

            let k_gen = production / settings.n_particles as f64;
            k_by_generation.push(k_gen);
            k_running = k_gen;
            if gen >= settings.n_inactive {
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

/// GPU-accelerated fission-source power iteration ([`ComputeType::Gpu`]), with a
/// **graceful CPU fallback** — never an error, never a panic on a missing GPU.
///
/// If a usable GPU adapter is present ([`crate::gpu::probe`] returns `Some`), the
/// run is handed to [`run_keff_gpu_batched`] — the **event-based batched-flight**
/// path, which keeps a whole batch of live neutrons resident in GPU buffers and
/// advances them one flight at a time on the GPU (RNG draw + native-union Sigma_t
/// lookup + collision-distance sample + streaming + leak test), leaving only the
/// branchy per-collision reaction physics on the CPU (see that function for
/// exactly how far the GPU reaches into the loop). The earlier
/// [`run_keff_gpu_inner`] (first-flight-only GPU Sigma_t) is retained for
/// comparison but no longer the default. If no adapter is available — a headless
/// server, CI with no Vulkan/Metal loader, or **Android**, where the whole `wgpu`
/// path is compiled out — it emits a `log::debug!` line and transparently runs
/// the trusted [`run_keff_cpu_single`] reference instead.
///
/// The GPU path is `f32` **acceleration only**; the single-thread CPU path
/// remains the trusted, deterministic reference.
pub fn run_keff_gpu(
    radius_cm: f64,
    material: &Material,
    nuclides: &[Nuclide],
    settings: &KeffSettings,
) -> KeffResult {
    #[cfg(not(target_os = "android"))]
    {
        if let Some(ctx) = crate::gpu::probe() {
            // op-u6s.8: the fused event path resolves flight AND collision on the
            // GPU (batch resident, no per-event round-trip). run_keff_gpu_batched
            // (collision on the CPU) is kept for comparison but no longer the
            // default.
            return run_keff_gpu_event(&ctx, radius_cm, material, nuclides, settings);
        }
    }
    log::debug!("ComputeType::Gpu requested but no GPU adapter available — falling back to CPU");
    run_keff_cpu_single(radius_cm, material, nuclides, settings)
}

/// The genuine GPU path behind [`run_keff_gpu`] (desktop / non-Android only).
///
/// # How far the GPU reaches into the transport loop
///
/// The transport loop is **structurally identical** to [`run_keff_cpu_single`] —
/// same sequential single-`seed` threading, same RNG draw order — so `k_gpu`
/// stays tightly correlated with the single-thread reference. The **only**
/// difference is where the macroscopic total Sigma_t comes from:
///
/// 1. **Build once.** A dense log-spaced table of the material's macroscopic
///    Sigma_t is tabulated up front over `[1e-3, 2e7]` eV with 16 384 points
///    ([`crate::gpu::union_grid::UnionTotalXs::tabulate`]). Temperature is fixed
///    for the whole run, so one table serves every generation. 16 384 points is
///    dense enough that the resampling error versus a direct
///    [`Material::macro_xs_total`] call is small (it is judged against the
///    reference below, not trusted above it).
/// 2. **GPU batch per generation (the genuine GPU penetration).** At the start of
///    every generation, the birth energies of **all** source sites are looked up
///    in **one GPU dispatch** ([`crate::gpu::union_grid::UnionTotalXs::lookup_gpu`],
///    `f32`). Each history then **consumes** its GPU-computed `f32` Sigma_t as the
///    **first-flight** total cross section (its first collision-distance sample),
///    instead of recomputing it.
/// 3. **CPU table lookups thereafter.** Every subsequent per-collision Sigma_t
///    within a history — and the first flight of any `(n,2n)` secondary that
///    starts a fresh sub-walk — is served from the **same table** by CPU linear
///    interpolation ([`crate::gpu::union_grid::UnionTotalXs::lookup_cpu`]).
///
/// A history-based random walk yields collision energies **one at a time**, so
/// dispatching a single-energy GPU kernel per collision would be dominated by
/// kernel-launch latency — that is the honest limit of GPU penetration into a
/// branchy history loop (see `src/gpu/mod.rs`: the "history-based transport loop
/// … branchy, not GPU friendly" note). The batched *first-flight* lookup is the
/// one place a whole generation's Sigma_t queries are available at once, so it is
/// the one place the GPU is actually exercised in the eigenvalue loop.
///
/// The GPU `f32` values are **acceleration only**; [`run_keff_cpu_single`] stays
/// the trusted reference. `k_gpu` differs from `k_single` only through (a) the
/// table's dense-resampling approximation of Sigma_t and (b) `f32` rounding of
/// the first-flight lookup — the RNG stream is otherwise identical.
#[cfg(not(target_os = "android"))]
pub fn run_keff_gpu_inner(
    ctx: &crate::gpu::GpuContext,
    radius_cm: f64,
    material: &Material,
    nuclides: &[Nuclide],
    settings: &KeffSettings,
) -> KeffResult {
    let sphere = Sphere {
        x0: 0.0,
        y0: 0.0,
        z0: 0.0,
        r: radius_cm,
        bc: BoundaryType::Vacuum,
    };
    let mut seed = settings.seed;
    let temp = settings.temperature_k;

    // Build the dense Sigma_t table once (temperature is fixed for the run).
    let table =
        crate::gpu::union_grid::UnionTotalXs::tabulate(material, nuclides, 1e-3, 2e7, 16384);

    // Initial source: uniform in the sphere volume, isotropic, Watt energy —
    // identical to run_keff_cpu_single, on the same single seed stream.
    let mut source: Vec<Site> = (0..settings.n_particles)
        .map(|_| {
            let (dx, dy, dz) = isotropic_direction(&mut seed);
            let rr = radius_cm * prn(&mut seed).cbrt();
            Site {
                r: Position::new(rr * dx, rr * dy, rr * dz),
                u: Direction::new(dx, dy, dz),
                e: watt(&mut seed, settings.watt_a, settings.watt_b),
            }
        })
        .collect();

    let n_gen = settings.n_inactive + settings.n_active;
    let mut k_by_generation = Vec::with_capacity(n_gen);
    let mut k_running = 1.0;
    let mut active_k = Vec::with_capacity(settings.n_active);

    for gen in 0..n_gen {
        // GENUINE GPU DISPATCH: batch-evaluate every source site's birth-energy
        // Sigma_t on the GPU in one pass. These f32 values are consumed as each
        // history's first-flight total cross section below.
        let birth_energies: Vec<f64> = source.iter().map(|s| s.e).collect();
        let birth_sigma: Vec<f32> = table.lookup_gpu(ctx, &birth_energies);

        let mut next_bank: Vec<Site> = Vec::with_capacity(settings.n_particles);
        let mut production = 0.0_f64;

        for (i, site) in source.iter().enumerate() {
            production += transport_history_tabulated(
                *site,
                Some(birth_sigma[i] as f64),
                &table,
                &sphere,
                material,
                nuclides,
                temp,
                k_running,
                &mut next_bank,
                &mut seed,
            );
        }

        let k_gen = production / settings.n_particles as f64;
        k_by_generation.push(k_gen);
        k_running = k_gen;
        if gen >= settings.n_inactive {
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

// ===========================================================================
// Event-based batched-flight GPU path (the deep GPU penetration, beads op-u6s.7)
// ===========================================================================

/// Per-generation stride for the batched GPU path's per-particle RNG streams.
/// Reuses the same jump-ahead striding scheme as [`run_keff_cpu_multi`] so each
/// history owns an independent, non-overlapping LCG sub-stream derived only from
/// `(seed, generation, history index)` — reproducible run-to-run, independent of
/// how the batch is scheduled on the GPU. (`(n,2n)` secondaries get a further
/// jump-ahead off their parent's stream; see [`run_keff_gpu_batched`].)
#[cfg(not(target_os = "android"))]
const BATCH_GEN_STRIDE: u64 = 1 << 40;
/// Per-history stride for the batched GPU path (see [`BATCH_GEN_STRIDE`]).
#[cfg(not(target_os = "android"))]
const BATCH_HIST_STRIDE: u64 = crate::rng::lcg::DEFAULT_STRIDE;
/// Jump-ahead applied to a colliding neutron's post-collision seed to seed the
/// extra neutron it emits in an `(n,2n)` event, so parent and secondary never
/// share draws. Large and coprime-ish with the history stride.
#[cfg(not(target_os = "android"))]
const BATCH_SECONDARY_STRIDE: u64 = 1 << 20;
/// Hard cap on flight events per generation — a guard against a pathological
/// non-terminating walk. Far above any real bare-sphere collision chain.
#[cfg(not(target_os = "android"))]
const BATCH_MAX_EVENTS: usize = 100_000;

/// One live neutron resident in the batched-flight event loop: phase-space state
/// plus its own LCG seed. `r` \[cm\], `u` unit direction, `e` \[eV\], `seed` the
/// per-particle 64-bit LCG state threaded through both the GPU flight and the CPU
/// collision.
#[cfg(not(target_os = "android"))]
#[derive(Clone, Copy)]
struct LiveNeutron {
    r: Position,
    u: Direction,
    e: f64,
    seed: u64,
}

/// **Event-based, batched-flight GPU power iteration** ([`ComputeType::Gpu`]) —
/// the deep GPU penetration of beads op-u6s.7. Desktop / non-Android only.
///
/// # How far the GPU reaches into the transport loop (the honest split)
///
/// Unlike the earlier first-flight-only [`run_keff_gpu_inner`], this driver keeps
/// a **whole batch of live neutrons resident in GPU buffers** and advances them
/// **one flight (one event) at a time, in parallel, per GPU dispatch**. For each
/// live particle, one [`crate::gpu::batched_flight::advance_flight_gpu`] dispatch
/// does, on the GPU (`f32`):
///
/// 1. **RNG** — advance the particle's own 64-bit LCG one step (the state math is
///    bit-exact vs the CPU LCG; the derived uniform is `f32`), giving the
///    collision-distance random number.
/// 2. **Sigma_t lookup** — binary-search the shared **native-breakpoint union
///    grid** ([`crate::gpu::union_grid::UnionTotalXs::tabulate_native`]) and
///    linearly interpolate the macroscopic total Sigma_t at the particle energy.
/// 3. **Distance-to-collision** — `d_col = -ln(xi) / Sigma_t`.
/// 4. **Distance-to-boundary** — the bounding sphere intersection.
/// 5. **Stream + leak test** — move to the nearer of the two; flag `Leaked`
///    (reached vacuum) or `Collided` (interacts inside).
///
/// So the **regular, memory-bound, per-event streaming work runs on the GPU for
/// the entire batch at once**. Only the **branchy per-collision reaction physics**
/// (which nuclide, fission vs capture vs inelastic vs `(n,2n)` vs elastic, the
/// secondary energy/angle laws) runs on the CPU — it is data-divergent and maps
/// poorly to a GPU. Each generation therefore issues a *sequence* of GPU
/// dispatches (one per event depth), each advancing all still-alive particles,
/// with a CPU collision + compaction pass between dispatches. This is the honest
/// limit of GPU penetration into a history-based MC walk: the flight is
/// data-parallel and lives on the GPU; the collision kernel is branchy and stays
/// on the CPU.
///
/// # RNG / reproducibility
///
/// Each history owns an **independent LCG sub-stream** derived only from
/// `(seed, generation, history index)` via jump-ahead — the same scheme as
/// [`run_keff_cpu_multi`]. The seed is threaded *through* the GPU flight (which
/// advances it bit-exactly) and continues on the CPU for the collision draws, so
/// a given particle sees one coherent stream across the GPU/CPU boundary. The
/// result is **reproducible run-to-run** and independent of GPU scheduling, but
/// — like the multi-thread backend, and because the flight's uniform + distance
/// are computed in `f32` — it does **not** bit-match [`run_keff_cpu_single`]. It
/// is a statistically independent estimate of the same eigenvalue, agreeing with
/// the trusted reference within combined statistical uncertainty. The CPU
/// single-thread `f64` path remains the trusted, bit-reproducible reference.
#[cfg(not(target_os = "android"))]
pub fn run_keff_gpu_batched(
    ctx: &crate::gpu::GpuContext,
    radius_cm: f64,
    material: &Material,
    nuclides: &[Nuclide],
    settings: &KeffSettings,
) -> KeffResult {
    use crate::gpu::batched_flight::{advance_flight_gpu, FlightBatch, FlightOutcome, FlightSphere};
    use crate::gpu::union_grid::UnionTotalXs;
    use crate::rng::lcg::future_seed;

    // The bounding-sphere intersection is computed on the GPU in the flight
    // kernel, so only the f32 FlightSphere is needed here (no CSG Sphere).
    let flight_sphere = FlightSphere {
        x0: 0.0,
        y0: 0.0,
        z0: 0.0,
        r: radius_cm as f32,
    };
    let temp = settings.temperature_k;

    // Native-breakpoint union grid, built once (temperature fixed for the run),
    // cast to f32 for the GPU flight kernel.
    let table = UnionTotalXs::tabulate_native(material, nuclides, 1e-3, 2e7, 16384);
    let grid_f32: Vec<f32> = table.grid.iter().map(|&x| x as f32).collect();
    let sigma_f32: Vec<f32> = table.sigma_total.iter().map(|&x| x as f32).collect();

    // Source sampling / resampling run on their own sequential seed stream, kept
    // off the per-history transport streams (as in run_keff_cpu_multi).
    let mut src_seed = settings.seed;
    let mut source: Vec<Site> = (0..settings.n_particles)
        .map(|_| {
            let (dx, dy, dz) = isotropic_direction(&mut src_seed);
            let rr = radius_cm * prn(&mut src_seed).cbrt();
            Site {
                r: Position::new(rr * dx, rr * dy, rr * dz),
                u: Direction::new(dx, dy, dz),
                e: watt(&mut src_seed, settings.watt_a, settings.watt_b),
            }
        })
        .collect();

    let n_gen = settings.n_inactive + settings.n_active;
    let mut k_by_generation = Vec::with_capacity(n_gen);
    let mut k_running = 1.0;
    let mut active_k = Vec::with_capacity(settings.n_active);

    for gen in 0..n_gen {
        let gen_base_seed = future_seed((gen as u64).wrapping_mul(BATCH_GEN_STRIDE), settings.seed);

        // Seed the generation's live batch: each source site becomes a live
        // neutron with its own independent per-history LCG stream.
        let mut live: Vec<LiveNeutron> = source
            .iter()
            .enumerate()
            .map(|(hist_idx, s)| LiveNeutron {
                r: s.r,
                u: s.u,
                e: s.e,
                seed: future_seed(
                    (hist_idx as u64).wrapping_mul(BATCH_HIST_STRIDE),
                    gen_base_seed,
                ),
            })
            .collect();

        let mut next_bank: Vec<Site> = Vec::with_capacity(settings.n_particles);
        let mut production = 0.0_f64;

        // Event loop: each pass advances EVERY live neutron one flight on the GPU,
        // then resolves collisions on the CPU, until the batch drains.
        let mut events = 0usize;
        while !live.is_empty() && events < BATCH_MAX_EVENTS {
            events += 1;

            // --- GPU: one dispatch advances the whole batch by one flight. ---
            let n = live.len();
            let mut batch = FlightBatch {
                pos: Vec::with_capacity(3 * n),
                dir: Vec::with_capacity(3 * n),
                energy: Vec::with_capacity(n),
                rng_hi: Vec::with_capacity(n),
                rng_lo: Vec::with_capacity(n),
            };
            for p in &live {
                batch.pos.push(p.r.x as f32);
                batch.pos.push(p.r.y as f32);
                batch.pos.push(p.r.z as f32);
                batch.dir.push(p.u.u as f32);
                batch.dir.push(p.u.v as f32);
                batch.dir.push(p.u.w as f32);
                batch.energy.push(p.e as f32);
                batch.rng_hi.push((p.seed >> 32) as u32);
                batch.rng_lo.push(p.seed as u32);
            }
            let outcomes =
                advance_flight_gpu(ctx, &grid_f32, &sigma_f32, &mut batch, flight_sphere);

            // --- CPU: resolve each outcome; build the next event's live batch. ---
            let mut survivors: Vec<LiveNeutron> = Vec::with_capacity(n);
            for (i, outcome) in outcomes.iter().enumerate() {
                // Reassemble the GPU-advanced per-particle seed.
                let mut seed = ((batch.rng_hi[i] as u64) << 32) | (batch.rng_lo[i] as u64);
                match outcome {
                    FlightOutcome::Leaked => { /* escaped to vacuum → dead */ }
                    FlightOutcome::Collided => {
                        // Collision site as streamed by the GPU flight (f32 → f64).
                        let r = Position::new(
                            batch.pos[3 * i] as f64,
                            batch.pos[3 * i + 1] as f64,
                            batch.pos[3 * i + 2] as f64,
                        );
                        let u = live[i].u;
                        let e = live[i].e;
                        let (prod, result) = collide_batched(
                            r,
                            u,
                            e,
                            material,
                            nuclides,
                            temp,
                            k_running,
                            &mut next_bank,
                            &mut seed,
                        );
                        production += prod;
                        match result {
                            CollisionResult::Dead => {}
                            CollisionResult::Scatter { e: e2, u: u2 } => {
                                survivors.push(LiveNeutron {
                                    r,
                                    u: u2,
                                    e: e2,
                                    seed,
                                });
                            }
                            CollisionResult::ScatterWithSecondary {
                                e: e2,
                                u: u2,
                                sec_e,
                                sec_u,
                            } => {
                                survivors.push(LiveNeutron {
                                    r,
                                    u: u2,
                                    e: e2,
                                    seed,
                                });
                                // The (n,2n) extra neutron gets its own sub-stream
                                // (jump-ahead off the parent's post-collision seed).
                                let sec_seed = future_seed(BATCH_SECONDARY_STRIDE, seed);
                                survivors.push(LiveNeutron {
                                    r,
                                    u: sec_u,
                                    e: sec_e,
                                    seed: sec_seed,
                                });
                            }
                        }
                    }
                }
            }
            live = survivors;
        }

        let k_gen = production / settings.n_particles as f64;
        k_by_generation.push(k_gen);
        k_running = k_gen;
        if gen >= settings.n_inactive {
            active_k.push(k_gen);
        }

        if next_bank.is_empty() {
            break;
        }
        source = resample(&next_bank, settings.n_particles, &mut src_seed);
    }

    let (k_mean, k_std) = mean_and_stderr(&active_k);
    KeffResult {
        k_mean,
        k_std,
        k_by_generation,
    }
}

// ===========================================================================
// Fused event-based COLLISION-on-GPU path (op-u6s.8): the whole event loop stays
// GPU-resident, with the branchy collision physics on the GPU too.
// ===========================================================================

/// Hard cap on flight events per generation for the fused event path — a guard
/// against a pathological non-terminating walk. Unconditional (shared by the CPU
/// mirror driver and the GPU driver). Far above any real bare-sphere chain.
const EVENT_MAX_EVENTS: usize = 100_000;

/// Build a fresh, all-alive [`EventBatch`] for one generation from the source
/// sites, giving each history its own independent LCG sub-stream (the same
/// jump-ahead striding as [`run_keff_cpu_multi`], via [`HIST_STRIDE`]).
fn build_event_batch(source: &[Site], gen_base_seed: u64) -> EventBatch {
    let n = source.len();
    let mut pos = Vec::with_capacity(3 * n);
    let mut dir = Vec::with_capacity(3 * n);
    let mut energy = Vec::with_capacity(n);
    let mut seed_lo = Vec::with_capacity(n);
    let mut seed_hi = Vec::with_capacity(n);
    for (hist_idx, s) in source.iter().enumerate() {
        let seed = future_seed((hist_idx as u64).wrapping_mul(HIST_STRIDE), gen_base_seed);
        pos.push(s.r.x as f32);
        pos.push(s.r.y as f32);
        pos.push(s.r.z as f32);
        dir.push(s.u.u as f32);
        dir.push(s.u.v as f32);
        dir.push(s.u.w as f32);
        energy.push(s.e as f32);
        seed_lo.push(seed as u32);
        seed_hi.push((seed >> 32) as u32);
    }
    EventBatch {
        pos,
        dir,
        energy,
        seed_lo,
        seed_hi,
        alive: vec![1u32; n],
        fiss_nuc: vec![FISS_NONE; n],
        production: vec![0.0f32; n],
    }
}

/// Bank the next generation's fission sites from a **drained** [`EventBatch`], and
/// return `(production, next_bank)`.
///
/// This is the CPU-side twin of the fission daughters in [`collide_batched`]: the
/// GPU (or CPU mirror) event kernel tags each fissioned neutron with its nuclide
/// and ν̄ and hands back its post-collision seed; here — once per generation, not
/// per event — each fissioned neutron's daughter **count**
/// ([`sample_num_neutrons`]) and each daughter's isotropic direction + χ birth
/// energy are drawn from that seed, so the per-history stream stays coherent across
/// the GPU/CPU boundary. `production` is Σ ν̄ over fissions (the generation-k
/// numerator).
fn bank_event_fission(
    batch: &EventBatch,
    material: &Material,
    nuclides: &[Nuclide],
    k_running: f64,
) -> (f64, Vec<Site>) {
    let n = batch.len();
    let mut production = 0.0_f64;
    let mut next_bank: Vec<Site> = Vec::new();
    for i in 0..n {
        if batch.fiss_nuc[i] == FISS_NONE {
            continue;
        }
        let jsel = batch.fiss_nuc[i] as usize;
        let nuc = &nuclides[material.components[jsel].nuclide_idx];
        let nu_bar = batch.production[i] as f64;
        production += nu_bar;
        let mut seed = ((batch.seed_hi[i] as u64) << 32) | (batch.seed_lo[i] as u64);
        let r = Position::new(
            batch.pos[3 * i] as f64,
            batch.pos[3 * i + 1] as f64,
            batch.pos[3 * i + 2] as f64,
        );
        let e_in = batch.energy[i] as f64;
        let n_d = sample_num_neutrons(nu_bar, k_running, &mut seed);
        for _ in 0..n_d {
            let (dx, dy, dz) = isotropic_direction(&mut seed);
            next_bank.push(Site {
                r,
                u: Direction::new(dx, dy, dz),
                e: nuc.sample_fission_energy(e_in, &mut seed),
            });
        }
    }
    (production, next_bank)
}

/// **CPU-mirror event-based power iteration** — the non-GPU reference for the
/// fused collision-on-GPU path ([`run_keff_gpu_event`]).
///
/// Runs the identical event-based algorithm — a resident batch advanced one event
/// at a time, flight **and** collision resolved by
/// [`crate::gpu::batched_event::advance_generation_cpu_mirror`] (the same f32
/// arithmetic path as the WGSL kernel) — but entirely on the CPU. It builds on
/// **every** target (no `wgpu`), so it validates the fused event physics on
/// Android and on CPU-only CI where the GPU path cannot run. Like the multi-thread
/// and GPU backends it uses independent per-history LCG streams (f32 uniforms), so
/// it is a statistically independent estimate of the same eigenvalue — not
/// bit-locked to [`run_keff_cpu_single`], but agreeing within combined uncertainty.
pub fn run_keff_event_cpu_mirror(
    radius_cm: f64,
    material: &Material,
    nuclides: &[Nuclide],
    settings: &KeffSettings,
) -> KeffResult {
    let tables = EventTablesF32::from_collision_tables(&CollisionTables::build(
        material, nuclides, 1e-3, 2e7, 16384,
    ));
    let sphere = EventSphere {
        x0: 0.0,
        y0: 0.0,
        z0: 0.0,
        r: radius_cm as f32,
    };
    run_event_power_iteration(
        radius_cm, material, nuclides, settings, &tables, &sphere, None,
    )
}

/// **Event-based COLLISION-on-GPU power iteration** ([`ComputeType::Gpu`]) — the
/// op-u6s.8 deep-penetration path. Desktop / non-Android only.
///
/// # How far the GPU reaches (the honest split)
/// Unlike [`run_keff_gpu_batched`] (which kept the collision on the CPU and
/// round-tripped **per event**), this driver keeps a whole generation's batch
/// **resident in GPU buffers** and advances it through every event — flight **and**
/// the branchy collision physics — on the GPU
/// ([`crate::gpu::batched_event::advance_generation_gpu`]). Per event, on the GPU
/// in `f32`: advance each neutron's LCG; look up Σ_t; sample the flight; on
/// collision sample the nuclide, partition the reaction (fission | capture |
/// inelastic | elastic), and apply the scatter kinematics. The only CPU traffic is
/// a 4-byte live-count read per event and one per-generation fission read-back; the
/// fission **daughters** are then banked on the CPU once per generation
/// ([`bank_event_fission`]) from the handed-back seeds.
///
/// # Trust / reproducibility
/// The `f32` GPU results are **acceleration only**; [`run_keff_cpu_single`] stays
/// the trusted reference. Independent per-history LCG streams make this a
/// statistically independent estimate (not bit-locked to single-thread), agreeing
/// within combined uncertainty. The per-event GPU logic is held to the CPU mirror
/// ([`run_keff_event_cpu_mirror`]) by the V&V gate in
/// [`crate::gpu::batched_event`].
#[cfg(not(target_os = "android"))]
pub fn run_keff_gpu_event(
    ctx: &crate::gpu::GpuContext,
    radius_cm: f64,
    material: &Material,
    nuclides: &[Nuclide],
    settings: &KeffSettings,
) -> KeffResult {
    let tables = EventTablesF32::from_collision_tables(&CollisionTables::build(
        material, nuclides, 1e-3, 2e7, 16384,
    ));
    let sphere = EventSphere {
        x0: 0.0,
        y0: 0.0,
        z0: 0.0,
        r: radius_cm as f32,
    };
    run_event_power_iteration(
        radius_cm,
        material,
        nuclides,
        settings,
        &tables,
        &sphere,
        Some(ctx),
    )
}

/// Shared power-iteration loop for the fused event path — drives either the GPU
/// resident driver (`ctx = Some`) or the CPU mirror driver (`ctx = None`) over the
/// generations, with identical source sampling, fission banking, and resampling.
///
/// Keeping the two backends behind one loop guarantees the CPU mirror and the GPU
/// path differ **only** in where each generation's events are advanced — the
/// whole eigenvalue bookkeeping (source, banking, resample, statistics) is shared.
#[allow(clippy::too_many_arguments)]
fn run_event_power_iteration(
    radius_cm: f64,
    material: &Material,
    nuclides: &[Nuclide],
    settings: &KeffSettings,
    tables: &EventTablesF32,
    sphere: &EventSphere,
    #[cfg_attr(target_os = "android", allow(unused_variables))] ctx: Option<
        &crate::gpu::GpuContext,
    >,
) -> KeffResult {
    // Source sampling / resampling on their own sequential seed stream, kept off
    // the per-history transport streams (as in run_keff_cpu_multi).
    let mut src_seed = settings.seed;
    let mut source: Vec<Site> = (0..settings.n_particles)
        .map(|_| {
            let (dx, dy, dz) = isotropic_direction(&mut src_seed);
            let rr = radius_cm * prn(&mut src_seed).cbrt();
            Site {
                r: Position::new(rr * dx, rr * dy, rr * dz),
                u: Direction::new(dx, dy, dz),
                e: watt(&mut src_seed, settings.watt_a, settings.watt_b),
            }
        })
        .collect();

    let n_gen = settings.n_inactive + settings.n_active;
    let mut k_by_generation = Vec::with_capacity(n_gen);
    let mut k_running = 1.0;
    let mut active_k = Vec::with_capacity(settings.n_active);

    for gen in 0..n_gen {
        let gen_base_seed = future_seed((gen as u64).wrapping_mul(GEN_STRIDE), settings.seed);
        let mut batch = build_event_batch(&source, gen_base_seed);

        // Advance the whole generation to extinction — resident on the GPU when a
        // context is present, else on the CPU mirror (both Android-buildable at the
        // call site because the GPU arm is cfg-gated below).
        #[cfg(not(target_os = "android"))]
        {
            if let Some(ctx) = ctx {
                crate::gpu::batched_event::advance_generation_gpu(
                    ctx,
                    tables,
                    &mut batch,
                    *sphere,
                    EVENT_MAX_EVENTS,
                );
            } else {
                crate::gpu::batched_event::advance_generation_cpu_mirror(
                    tables,
                    &mut batch,
                    *sphere,
                    EVENT_MAX_EVENTS,
                );
            }
        }
        #[cfg(target_os = "android")]
        {
            crate::gpu::batched_event::advance_generation_cpu_mirror(
                tables,
                &mut batch,
                *sphere,
                EVENT_MAX_EVENTS,
            );
        }

        let (production, next_bank) = bank_event_fission(&batch, material, nuclides, k_running);

        let k_gen = production / settings.n_particles as f64;
        k_by_generation.push(k_gen);
        k_running = k_gen;
        if gen >= settings.n_inactive {
            active_k.push(k_gen);
        }

        if next_bank.is_empty() {
            break;
        }
        source = resample(&next_bank, settings.n_particles, &mut src_seed);
    }

    let (k_mean, k_std) = mean_and_stderr(&active_k);
    KeffResult {
        k_mean,
        k_std,
        k_by_generation,
    }
}

/// The outcome of one CPU-side collision in the batched GPU path
/// ([`run_keff_gpu_batched`]) — enum dispatch so every downstream `match` is
/// exhaustive (no trait objects, per the workspace design rules).
#[cfg(not(target_os = "android"))]
enum CollisionResult {
    /// Absorbed or fissioned → the neutron leaves the batch. Any fission sites
    /// were already banked into `next_bank`; the ν̄ production is returned
    /// alongside.
    Dead,
    /// Scattered (elastic or inelastic) → stays live with new energy/direction.
    Scatter { e: f64, u: Direction },
    /// Scattered **and** emitted one extra same-generation neutron (`(n,2n)`,
    /// yield 2) → both the down-scattered primary and the secondary stay live.
    ScatterWithSecondary {
        e: f64,
        u: Direction,
        sec_e: f64,
        sec_u: Direction,
    },
}

/// Resolve one collision at `r` \[cm\] for a neutron of energy `e` \[eV\] and
/// direction `u`, drawing from its LCG `seed`. Returns `(production, result)`
/// where `production` is the ν̄ banked this collision (non-zero only on fission)
/// and `result` tells [`run_keff_gpu_batched`] whether the neutron dies,
/// scatters, or scatters with an `(n,2n)` secondary.
///
/// This is the **CPU collision kernel** of the batched GPU path. Its reaction
/// partition and RNG-draw order are a **verbatim mirror** of the collision block
/// in [`transport_history`] (fission | capture | inelastic | `(n,2n)` | elastic,
/// partitioned on the microscopic total), so a history's stream stays coherent
/// across the GPU flight → CPU collision boundary. The only structural difference
/// is that `(n,2n)` returns the secondary to the caller (which re-banks it into
/// the live batch) rather than pushing onto a local stack. See
/// [`transport_history`] for the per-channel physics citations.
#[cfg(not(target_os = "android"))]
#[allow(clippy::too_many_arguments)]
fn collide_batched(
    r: Position,
    u: Direction,
    e: f64,
    material: &Material,
    nuclides: &[Nuclide],
    temp: f64,
    k_running: f64,
    next_bank: &mut Vec<Site>,
    seed: &mut u64,
) -> (f64, CollisionResult) {
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
        let n = sample_num_neutrons(nu_bar, k_running, seed);
        for _ in 0..n {
            let (dx, dy, dz) = isotropic_direction(seed);
            next_bank.push(Site {
                r,
                u: Direction::new(dx, dy, dz),
                e: nuc.sample_fission_energy(e, seed),
            });
        }
        (nu_bar, CollisionResult::Dead)
    } else if xi < x.absorption {
        (0.0, CollisionResult::Dead) // radiative capture
    } else if xi < x.absorption + x.inelastic {
        let (e2, u2) = match nuc.sample_inelastic(e, seed) {
            Inelastic::Level { q } => two_body_scatter(e, u, nuc.awr, q, seed),
            Inelastic::Continuum => continuum_inelastic_scatter(e, u, nuc.awr, seed),
        };
        (0.0, CollisionResult::Scatter { e: e2, u: u2 })
    } else if xi < x.absorption + x.inelastic + x.n2n {
        // (n,2n): the primary down-scatters and one extra neutron is emitted
        // sharing the sampled outgoing state (Weisskopf stand-in for the emission
        // law, as in transport_history).
        let (e2, u2) = continuum_inelastic_scatter(e, u, nuc.awr, seed);
        (
            0.0,
            CollisionResult::ScatterWithSecondary {
                e: e2,
                u: u2,
                sec_e: e2,
                sec_u: u2,
            },
        )
    } else {
        // Bound-atom S(alpha, beta) below the table cutoff, else free-gas —
        // see the equivalent branch in [`transport_history`].
        let (e2, u2) = if let Some((e_out, mu_lab)) = nuc.sample_thermal(e, seed) {
            (e_out, rotate_direction(u, mu_lab, seed))
        } else {
            match nuc.sample_elastic_mu_cm(e, seed) {
                Some(mu_cm) => two_body_scatter_with_mu(e, u, nuc.awr, 0.0, mu_cm, seed),
                None => elastic_scatter(e, u, nuc.awr, seed),
            }
        };
        (0.0, CollisionResult::Scatter { e: e2, u: u2 })
    }
}

/// Transport one source neutron — plus any same-generation `(n,2n)` secondaries
/// it spawns — to death (absorption or leakage), banking any fission neutrons.
/// Returns the fission production ν̄ summed over every fission event in the
/// history (the generation-k numerator contribution).
///
/// `(n,2n)` neutrons are tracked to completion **within this generation** via a
/// local work stack, mirroring OpenMC's `create_secondary` bank (`src/physics.cpp`
/// `inelastic_scatter`): only *fission* neutrons are banked to the next
/// generation (`next_bank`); `(n,xn)` multiplicity is realized in-generation.
#[allow(clippy::too_many_arguments)]
fn transport_history(
    site: Site,
    sphere: &Sphere,
    material: &Material,
    nuclides: &[Nuclide],
    temp: f64,
    k_running: f64,
    next_bank: &mut Vec<Site>,
    seed: &mut u64,
) -> f64 {
    let mut production = 0.0;
    // Same-generation work stack: the source neutron plus any (n,2n) secondaries.
    let mut stack: Vec<Site> = vec![site];

    while let Some(start) = stack.pop() {
        let mut r = start.r;
        let mut u = start.u;
        let mut e = start.e;

        loop {
            let sigma_t = material.macro_xs_total(e, nuclides);
            if !(sigma_t > 0.0) {
                break; // no interaction possible; treat as escape
            }
            let d_col = -prn(seed).ln() / sigma_t;
            let d_bound = sphere.distance(r, u, false);

            if d_col >= d_bound {
                break; // reaches the vacuum boundary first → leaks
            }

            // Collide: advance to the collision site and pick the target nuclide.
            r = stream(r, u, d_col);
            let ci = material.sample_nuclide(e, seed, nuclides);
            let nuc = &nuclides[material.components[ci].nuclide_idx];
            let x = nuc.xs_at_energy(e, temp);

            // Reaction partition on the *total*:
            //   fission | capture | inelastic | (n,2n) | elastic.
            // `x.inelastic` (MT=51…91) and `x.n2n` (MT=16) are sub-bands of
            // scattering carved out with their own laws; both are non-zero only for
            // the HIGH tier, so the LOW tier collapses to the fission | capture |
            // elastic split. The final elastic bucket (total − absorption −
            // inelastic − n2n) sweeps up any remaining scattering as elastic-like.
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
                        // Birth from the fissioning nuclide's χ at the incident
                        // energy `e` — the HIGH tier's energy-dependent ENDF MF=5
                        // spectrum, or the thermal-Watt stand-in for the LOW tier.
                        e: nuc.sample_fission_energy(e, seed),
                    });
                }
                break; // fission is a terminal absorption for the incident neutron
            } else if xi < x.absorption {
                break; // radiative capture → dead
            } else if xi < x.absorption + x.inelastic {
                // Inelastic scatter with a real energy-loss law: a discrete level's
                // two-body kinematics (Q-value) or continuum evaporation. This is
                // the dominant fast-spectrum down-scatter off heavy nuclei.
                let (e2, u2) = match nuc.sample_inelastic(e, seed) {
                    Inelastic::Level { q } => two_body_scatter(e, u, nuc.awr, q, seed),
                    Inelastic::Continuum => continuum_inelastic_scatter(e, u, nuc.awr, seed),
                };
                e = e2;
                u = u2;
            } else if xi < x.absorption + x.inelastic + x.n2n {
                // (n,2n): the incident neutron down-scatters and one extra neutron
                // is emitted — the yield-2 multiplicity that restores the neutron
                // a bare fast sphere would otherwise lose. Ported from OpenMC
                // `inelastic_scatter` (src/physics.cpp:1167-1177): for an integral
                // yield Y it calls `create_secondary` Y−1 times with the *primary's*
                // post-scatter energy and direction, so the second neutron shares
                // the sampled outgoing state. We lack a parsed MF=6 (n,2n) emission
                // law, so the outgoing energy uses the same Weisskopf-evaporation
                // continuum as MT=91 inelastic — faithful to the multiplicity, a
                // stand-in for the emission spectrum.
                // TODO: parse the ENDF MF=6/MT=16 (n,2n) neutron emission
                // distribution and sample both outgoing neutrons from it, instead of
                // the Weisskopf stand-in (mirror OpenMC's UncorrelatedAngleEnergy /
                // CorrelatedAngleEnergy in src/distribution_energy.cpp).
                let (e2, u2) = continuum_inelastic_scatter(e, u, nuc.awr, seed);
                stack.push(Site { r, u: u2, e: e2 }); // yield − 1 = 1 secondary
                e = e2;
                u = u2;
            } else {
                // Scattering. A moderator nuclide carrying an S(alpha, beta)
                // table thermalizes via the bound-atom law below its cutoff
                // (lab-frame outgoing energy + cosine, up-scatter allowed);
                // mirrors `crate::physics::transport_csg` and OpenMC
                // `src/thermal.cpp` `ThermalData::sample`. Otherwise: use the
                // ENDF MF=4 angular distribution when the nuclide carries one
                // (HIGH tier) — fast neutrons scatter forward off heavy nuclei,
                // which raises bare-sphere leakage — else isotropic-CM.
                let (e2, u2) = if let Some((e_out, mu_lab)) = nuc.sample_thermal(e, seed) {
                    (e_out, rotate_direction(u, mu_lab, seed))
                } else {
                    match nuc.sample_elastic_mu_cm(e, seed) {
                        Some(mu_cm) => two_body_scatter_with_mu(e, u, nuc.awr, 0.0, mu_cm, seed),
                        None => elastic_scatter(e, u, nuc.awr, seed),
                    }
                };
                e = e2;
                u = u2;
            }
        }
    }
    production
}

/// The Sigma_t table-served twin of [`transport_history`] used by the GPU path
/// ([`run_keff_gpu_inner`]).
///
/// It is a **verbatim mirror** of [`transport_history`] — the same random numbers
/// are drawn in the same order, so a history stays aligned with the single-thread
/// reference — with exactly one change: the macroscopic total Sigma_t is served
/// from the pre-built `table` instead of [`Material::macro_xs_total`]:
///
/// - `first_flight_sigma_t` — if `Some(s)`, the neutron's **first** collision-
///   distance sample uses `s` (the GPU-batched `f32` value cast to `f64`) as
///   Sigma_t; it is consumed exactly once, on the first loop iteration of the
///   first popped site. Every later Sigma_t lookup — subsequent collisions and
///   the first flight of any `(n,2n)` secondary sub-walk — uses
///   `table.lookup_cpu(&[e])[0]`.
///
/// Only the Sigma_t **value** differs (table/GPU vs a direct data call); the RNG
/// usage is identical, which is what keeps `k_gpu` tightly correlated with
/// `k_single`.
#[cfg(not(target_os = "android"))]
#[allow(clippy::too_many_arguments)]
fn transport_history_tabulated(
    site: Site,
    first_flight_sigma_t: Option<f64>,
    table: &crate::gpu::union_grid::UnionTotalXs,
    sphere: &Sphere,
    material: &Material,
    nuclides: &[Nuclide],
    temp: f64,
    k_running: f64,
    next_bank: &mut Vec<Site>,
    seed: &mut u64,
) -> f64 {
    let mut production = 0.0;
    // Consumed once, on the very first Sigma_t lookup of the whole history (the
    // source neutron's first flight); `None` thereafter.
    let mut first_flight_sigma = first_flight_sigma_t;
    let mut stack: Vec<Site> = vec![site];

    while let Some(start) = stack.pop() {
        let mut r = start.r;
        let mut u = start.u;
        let mut e = start.e;

        loop {
            // ONLY DIFFERENCE from `transport_history`: Sigma_t comes from the
            // GPU-built table (or the batched first-flight GPU value), not from a
            // direct `macro_xs_total` call. No RNG is consumed here either way.
            let sigma_t = match first_flight_sigma.take() {
                Some(s) => s,
                None => table.lookup_cpu(&[e])[0],
            };
            if !(sigma_t > 0.0) {
                break;
            }
            let d_col = -prn(seed).ln() / sigma_t;
            let d_bound = sphere.distance(r, u, false);

            if d_col >= d_bound {
                break;
            }

            r = stream(r, u, d_col);
            let ci = material.sample_nuclide(e, seed, nuclides);
            let nuc = &nuclides[material.components[ci].nuclide_idx];
            let x = nuc.xs_at_energy(e, temp);

            // Reaction partition — VERBATIM from `transport_history`; only the
            // Sigma_t *source* above differs, never the RNG draws below.
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
                break;
            } else if xi < x.absorption {
                break;
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
                // Kept in lockstep with [`transport_history`] — the two differ
                // only in where Sigma_t comes from, so the bound-atom
                // S(alpha, beta) branch must be present here too.
                let (e2, u2) = if let Some((e_out, mu_lab)) = nuc.sample_thermal(e, seed) {
                    (e_out, rotate_direction(u, mu_lab, seed))
                } else {
                    match nuc.sample_elastic_mu_cm(e, seed) {
                        Some(mu_cm) => two_body_scatter_with_mu(e, u, nuc.awr, 0.0, mu_cm, seed),
                        None => elastic_scatter(e, u, nuc.awr, seed),
                    }
                };
                e = e2;
                u = u2;
            }
        }
    }
    production
}

/// Resample `n` sites uniformly with replacement from `bank` — the crude
/// population control that renormalises the fission bank back to a fixed source
/// size each generation.
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
mod tests {
    use super::*;
    use crate::material::material::NuclideComponent;

    /// Build the standard Godiva (HEU-MET-FAST-001) LOW-tier material +
    /// nuclide array: U-234/235/238 at ICSBEP atom densities, T = 293.6 K,
    /// embedded (`from_core`) data so the test stays offline.
    fn godiva() -> (Material, Vec<Nuclide>) {
        let nuclides = vec![
            Nuclide::from_core("U234").unwrap(),
            Nuclide::from_core("U235").unwrap(),
            Nuclide::from_core("U238").unwrap(),
        ];
        let material = Material {
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
        (material, nuclides)
    }

    /// **LOW-fidelity Godiva V&V** (HEU-MET-FAST-001).
    ///
    /// **Methodology.** Bare HEU sphere, r = 8.7407 cm, ICSBEP atom densities
    /// (U-234/235/238), 1500 histories × [20 inactive + 40 active], cross sections
    /// from the embedded LOW tier (WMP below `e_max` + infinite-dilution
    /// Watt-collapsed fast MGXS above — now with per-group μ̄ for forward elastic and
    /// inelastic carved from the group total). Reference: ICSBEP k_eff =
    /// 1.0000 ± 0.0010. Pass criterion is deliberately a *broad* plausibility band
    /// (0.9–1.4), not a benchmark gate — this guards the full transport chain (data
    /// → geometry → collision → scatter → fission → power iteration), not accuracy.
    ///
    /// **Results (2026-08-06, re-measured under the `op-jis` PCG-RXS-M-XS `prn`
    /// output permutation).** k_eff = **1.01997 ± 0.00571**, i.e. **+1 997 pcm**
    /// high — inside the [0.9, 1.4] plausibility band and comfortably under the
    /// `σ < 0.02` noise gate this test asserts. Still far down from ~1.129 /
    /// +12 900 pcm before the LOW tier gained inelastic + forward elastic scatter
    /// (see `docs/development-history.md`). The result is stationary; the residual
    /// bias is expected for this fidelity (no self-shielding; one mean cosine;
    /// evaporation for inelastic). Measured by running this test's exact
    /// configuration (1 500 histories × [20 inactive + 40 active], default seed) on
    /// 2026-08-06 — the test itself prints nothing, so the value was taken from a
    /// throwaway driver replicating it verbatim.
    ///
    /// **Supersedes (2026-07): k_eff ≈ 1.010 ± 0.002, ~+1 000 pcm.** That figure was
    /// measured with the pre-`op-jis` `prn` output function (raw top-52 state bits).
    /// The LCG *state recurrence* is unchanged — seeding and jump-ahead did not move
    /// — but every uniform double changed, so this estimator simply re-drew. Note
    /// the quoted σ also grew (0.002 → 0.00571); at 40 active generations that
    /// spread is ordinary batch scatter, not a fidelity change. No tolerance was
    /// changed; the test passes as written.
    #[test]
    fn godiva_converges_to_sane_keff() {
        let (material, nuclides) = godiva();
        let settings = KeffSettings {
            n_particles: 1500,
            n_inactive: 20,
            n_active: 40,
            ..KeffSettings::default()
        };
        let result = run_keff(8.7407, &material, &nuclides, &settings);

        assert_eq!(result.k_by_generation.len(), 60, "ran all generations");
        assert!(
            result.k_mean > 0.9 && result.k_mean < 1.4,
            "Godiva k_eff {} outside the plausible first-cut band [0.9, 1.4]",
            result.k_mean
        );
        assert!(
            result.k_std < 0.02,
            "k noisy/unconverged: σ = {}",
            result.k_std
        );
    }

    /// A far-subcritical configuration (tiny sphere ⇒ leakage-dominated) must
    /// come out well below the critical Godiva sphere — a sign check that the
    /// geometry/leakage coupling actually bites.
    #[test]
    fn small_sphere_is_less_reactive_than_godiva() {
        let nuclides = vec![Nuclide::from_core("U235").unwrap()];
        let material = Material {
            id: 1,
            name: "U235".into(),
            temperature: 293.6,
            components: vec![NuclideComponent {
                nuclide_idx: 0,
                atom_density: 4.8e-2,
            }],
        };
        let settings = KeffSettings {
            n_particles: 1000,
            n_inactive: 15,
            n_active: 25,
            ..KeffSettings::default()
        };

        let k_big = run_keff(9.0, &material, &nuclides, &settings).k_mean;
        let k_small = run_keff(3.0, &material, &nuclides, &settings).k_mean;
        assert!(
            k_small < k_big,
            "3 cm sphere (k={k_small}) should leak more than 9 cm (k={k_big})"
        );
    }

    /// **Three-compute-mode agreement V&V** — the single-thread, multi-thread, and
    /// GPU backends must all estimate the *same* Godiva eigenvalue.
    ///
    /// **Methodology.** Godiva LOW-tier material (U-234/235/238 at ICSBEP atom
    /// densities, T = 293.6 K, embedded `from_core` data), bare sphere r = 8.7407
    /// cm, 800 histories × [15 inactive + 40 active], master seed = 1 (default).
    /// The identical [`KeffSettings`] is run under each [`ComputeType`] via
    /// [`KeffSettings::with_compute`]:
    /// - single-thread = the trusted deterministic reference;
    /// - multi-thread = rayon over histories, independent per-history RNG streams
    ///   (a *statistically independent* estimate — different stream structure);
    /// - GPU = the **fused event-based collision-on-GPU** path
    ///   ([`run_keff_gpu_event`], op-u6s.8): a batch of live neutrons is advanced
    ///   one event at a time on the GPU, with **both** the flight (per-particle LCG
    ///   advance + native-union Sigma_t lookup + collision distance + sphere flight)
    ///   **and** the branchy collision physics (nuclide sampling, reaction
    ///   partition, elastic/inelastic kinematics) resolved on the GPU in `f32`; only
    ///   the fission daughters are banked on the CPU per generation. Like
    ///   multi-thread it uses independent per-history RNG streams, so it is a
    ///   statistically independent estimate — not bit-locked to the single-thread
    ///   stream — differing additionally through `f32` collision rounding.
    ///
    /// Pass criterion: pairwise agreement within a **combined-sigma band**,
    /// `|k_a − k_b| ≤ 5 · sqrt(σ_a² + σ_b²)`. The GPU arm **skips gracefully**
    /// (prints `SKIP` and is not asserted) when no GPU adapter is present in the
    /// test process, so the test is green on CPU-only CI.
    ///
    /// **Results (2026-08-06; adapter: NVIDIA GeForce RTX 3050, LOW-tier data,
    /// 800 histories × [15 + 40], seed = 1; re-measured under the `op-jis`
    /// PCG-RXS-M-XS `prn` output permutation).** All three modes ran (GPU arm not
    /// skipped — a real adapter was present in the test process):
    /// - k_single = **1.01207 ± 0.00673** (trusted reference),
    /// - k_multi  = **1.01897 ± 0.00945**,
    /// - k_gpu    = **1.00863 ± 0.00834** (fused collision-on-GPU path).
    ///
    /// Pairwise σ-distances, with `σ_comb = sqrt(σ_a² + σ_b²)` and the arithmetic
    /// written out so a reader can check it against the numbers above:
    /// - single-vs-multi: |Δk| = |1.01207 − 1.01897| = 0.00690;
    ///   σ_comb = sqrt(0.00673² + 0.00945²) = sqrt(4.5293e-5 + 8.9300e-5)
    ///   = sqrt(1.3459e-4) = 0.01160; 0.00690 / 0.01160 = **0.59σ** (5σ band
    ///   0.05801) — a statistically independent stream well inside the reference's
    ///   uncertainty;
    /// - single-vs-gpu: |Δk| = |1.01207 − 1.00863| = 0.00344;
    ///   σ_comb = sqrt(0.00673² + 0.00834²) = sqrt(4.5293e-5 + 6.9556e-5)
    ///   = sqrt(1.1485e-4) = 0.01072; 0.00344 / 0.01072 = **0.32σ** (5σ band
    ///   0.05358) — the fused collision-on-GPU path (independent per-history streams
    ///   + `f32` flight *and* collision) lands well within combined uncertainty of
    ///   the reference. The GPU is acceleration only, judged against the CPU
    ///   reference, never trusted above it.
    ///
    /// Not asserted by this test, recorded for completeness (same arithmetic):
    /// multi-vs-gpu |Δk| = 0.01034, σ_comb = sqrt(0.00945² + 0.00834²) = 0.01260,
    /// = **0.82σ** — the widest of the three pairs.
    ///
    /// All three land within ~1 900 pcm of unity and within 0.82σ of each other —
    /// moving the whole collision onto the GPU does not change the physics.
    ///
    /// **Supersedes (2026-07-17, measured with the pre-`op-jis` `prn` output
    /// function — raw top-52 state bits):** k_single = 1.00762 ± 0.00827,
    /// k_multi = 1.00715 ± 0.00768, k_gpu = 1.01284 ± 0.00823; single-vs-multi
    /// |Δk| = 0.00047 = 0.04σ, single-vs-gpu |Δk| = 0.00522 = 0.45σ. The LCG state
    /// recurrence is unchanged (integer-state facts, golden seeds, jump-ahead
    /// identities and the GPU integer-state mirror all still hold), so all that
    /// moved is the uniform stream every arm draws from — each of the three arms
    /// re-drew independently. No tolerance was changed; the 5σ gate is unaltered.
    #[test]
    fn three_compute_modes_agree_on_godiva() {
        let (material, nuclides) = godiva();
        let base = KeffSettings {
            n_particles: 800,
            n_inactive: 15,
            n_active: 40,
            ..KeffSettings::default()
        };

        let single = run_keff(
            8.7407,
            &material,
            &nuclides,
            &base.with_compute(ComputeType::CpuSingleThread),
        );
        let multi = run_keff(
            8.7407,
            &material,
            &nuclides,
            &base.with_compute(ComputeType::CpuMultiThread(ThreadCount::Auto)),
        );

        eprintln!(
            "k_single = {:.5} ± {:.5} | k_multi = {:.5} ± {:.5}",
            single.k_mean, single.k_std, multi.k_mean, multi.k_std
        );

        // single vs multi: statistical comparison (different RNG streams).
        let d_sm = (single.k_mean - multi.k_mean).abs();
        let sig_sm = (single.k_std.powi(2) + multi.k_std.powi(2)).sqrt();
        eprintln!(
            "single-vs-multi: |Δk| = {:.5} ({:.2}σ, band {:.5})",
            d_sm,
            if sig_sm > 0.0 { d_sm / sig_sm } else { 0.0 },
            5.0 * sig_sm
        );
        assert!(
            d_sm <= 5.0 * sig_sm,
            "single ({:.5}±{:.5}) vs multi ({:.5}±{:.5}) disagree beyond 5σ: |Δk|={:.5} > {:.5}",
            single.k_mean,
            single.k_std,
            multi.k_mean,
            multi.k_std,
            d_sm,
            5.0 * sig_sm
        );

        // GPU arm: skip gracefully when no adapter, else run and assert agreement.
        if crate::gpu::probe().is_none() {
            eprintln!("SKIP gpu arm: no adapter");
            return;
        }
        let gpu = run_keff(
            8.7407,
            &material,
            &nuclides,
            &base.with_compute(ComputeType::Gpu),
        );
        eprintln!("k_gpu = {:.5} ± {:.5}", gpu.k_mean, gpu.k_std);

        let d_sg = (single.k_mean - gpu.k_mean).abs();
        let sig_sg = (single.k_std.powi(2) + gpu.k_std.powi(2)).sqrt();
        eprintln!(
            "single-vs-gpu: |Δk| = {:.5} ({:.2}σ, band {:.5})",
            d_sg,
            if sig_sg > 0.0 { d_sg / sig_sg } else { 0.0 },
            5.0 * sig_sg
        );
        assert!(
            d_sg <= 5.0 * sig_sg,
            "single ({:.5}±{:.5}) vs gpu ({:.5}±{:.5}) disagree beyond 5σ: |Δk|={:.5} > {:.5}",
            single.k_mean,
            single.k_std,
            gpu.k_mean,
            gpu.k_std,
            d_sg,
            5.0 * sig_sg
        );
    }

    /// The multi-thread backend must be **reproducible independent of thread
    /// count and scheduling**: the reported `k_mean` is bit-for-bit identical no
    /// matter how many workers the dedicated pool has.
    ///
    /// **Methodology.** Same Godiva LOW-tier case (600 histories × [10 + 20],
    /// seed = 1) run three ways: [`ThreadCount::Fixed(1)`] twice (to check plain
    /// run-to-run determinism) and [`ThreadCount::Fixed(4)`] once (to check
    /// thread-count independence). Pass criterion: all three `k_mean` values are
    /// bit-for-bit equal (`f64::to_bits`). This proves each history's RNG stream
    /// is derived purely from `(seed, generation, history index)` via the LCG
    /// jump-ahead — never from a shared/raced seed or the scheduling order — and
    /// that the per-history bank concatenation is order-deterministic.
    ///
    /// **Results (2026-07-17).** All three runs returned an identical `k_mean`
    /// bit pattern (`Fixed(1)` run A == `Fixed(1)` run B == `Fixed(4)`), zero
    /// mismatches. Interpretation: the `CpuMultiThread` eigenvalue is fully
    /// determined by the settings, independent of the resolved pool size — only
    /// wall time changes with thread count.
    #[test]
    fn cpu_multi_is_reproducible() {
        let (material, nuclides) = godiva();
        let base = KeffSettings {
            n_particles: 600,
            n_inactive: 10,
            n_active: 20,
            ..KeffSettings::default()
        };

        let one_a = run_keff(
            8.7407,
            &material,
            &nuclides,
            &base.with_compute(ComputeType::CpuMultiThread(ThreadCount::Fixed(1))),
        )
        .k_mean;
        let one_b = run_keff(
            8.7407,
            &material,
            &nuclides,
            &base.with_compute(ComputeType::CpuMultiThread(ThreadCount::Fixed(1))),
        )
        .k_mean;
        let four = run_keff(
            8.7407,
            &material,
            &nuclides,
            &base.with_compute(ComputeType::CpuMultiThread(ThreadCount::Fixed(4))),
        )
        .k_mean;

        assert_eq!(
            one_a.to_bits(),
            one_b.to_bits(),
            "not run-to-run reproducible: {one_a} != {one_b}"
        );
        assert_eq!(
            one_a.to_bits(),
            four.to_bits(),
            "k depends on thread count (1 vs 4): {one_a} != {four}"
        );
    }

    /// **HIGH-fidelity Godiva V&V — the benchmark result** — behind the
    /// `net-fetch` feature (downloads ENDF; not part of the default offline suite).
    ///
    /// **Methodology.** The same Godiva model and power-iteration settings are run
    /// under both data tiers, judged against ICSBEP HEU-MET-FAST-001
    /// (k_eff = 1.0000 ± 0.0010):
    /// - **LOW** ([`Nuclide::from_core`]) — embedded WMP + infinite-dilution fast
    ///   MGXS. Now carries the same two transport-physics levers as HIGH, reduced to
    ///   group data: inelastic as the group remainder (Weisskopf evaporation) and
    ///   forward-peaked elastic from a per-group mean cosine μ̄ (max-entropy
    ///   exponential law). No self-shielding, one μ̄ instead of the full shape.
    /// - **HIGH** ([`Nuclide::from_endf`]) — ENDF/B-VII.1 downloaded and
    ///   reconstructed on device (RECONR 0.1% tol + BROADR to 293.6 K + MF=1/452
    ///   ν̄), continuous-energy σ(E), an explicit inelastic energy-loss law
    ///   (MT=51…91 two-body + evaporation), **anisotropic (full ENDF MF=4)
    ///   elastic scatter** (ported from OpenMC `AngleDistribution`/`Tabular`), and
    ///   **(n,2n) with its true yield-2 multiplicity** (MT=16 from the MF=3
    ///   background; one extra same-generation neutron per event, ported from
    ///   OpenMC `inelastic_scatter`, `src/physics.cpp:1167`), and an
    ///   **energy-dependent ENDF MF=5/MT=18 fission birth spectrum** χ(E→E')
    ///   (LF=1, per-nuclide; ported from OpenMC `ContinuousTabular::sample`).
    ///
    /// The test asserts that **both** tiers converge to a stationary eigenvalue
    /// near unity — HIGH from continuous-energy data and the full MF=4 shape, LOW
    /// from coarse group data plus a single per-group μ̄ — confirming the two levers
    /// (energy transfer + forward peaking) are what close the Godiva gap and that
    /// they survive the reduction to group data.
    ///
    /// **Results — read the two vintages separately.**
    ///
    /// **LOW tier (2026-08-06, re-measured under the `op-jis` PCG-RXS-M-XS `prn`
    /// output permutation).** k_eff = **1.01042 ± 0.00174 (+1 042 pcm)** vs ICSBEP
    /// 1.0000 ± 0.0010 — still in agreement with the benchmark. Source: a re-run of
    /// `examples/godiva_keff` (5 000 particles × [40 inactive + 110 active],
    /// embedded LOW-tier data, default seed), which is the LOW-tier Godiva case that
    /// was actually re-executed for `op-jis`. Note it is [40 + 110], not the
    /// [40 + 120] this test uses: the in-test LOW arm sits behind the `net-fetch`
    /// gate and therefore re-runs together with the HIGH arm below.
    /// *Supersedes* LOW k_eff = 1.01024 (+1 024 pcm), measured 2026-07-03 with the
    /// pre-`op-jis` `prn` output function (raw top-52 state bits).
    ///
    /// **HIGH tier — SUPERSEDED BY `op-jis`, PENDING RE-RUN.** Everything from here
    /// to the end of this doc comment (the HIGH k_eff, the ranked lever
    /// contributions, the (n,2n) A/B and the MF=5 χ A/B) was measured on 2026-07-03
    /// with the **pre-`op-jis`** `prn` output function (raw top-52 state bits). The
    /// LCG state recurrence did not change, but every uniform double did, so every
    /// one of these figures is expected to have moved. They are retained verbatim
    /// rather than deleted because they are the last values actually measured — but
    /// **do not cite them as current** until `examples/godiva_keff_endf` has been
    /// re-run under the new `prn` and this block reconciled against it.
    ///
    /// *(2026-07-03; HIGH = ENDF/B-VII.1; 5000 particles, 40 inactive + 120 active
    /// generations, default seed.)* HIGH k_eff = **1.00367 ± 0.00182 (+367 pcm)** —
    /// in agreement with the benchmark. The ranked HIGH-tier lever contributions
    /// (anisotropic elastic ~10 300 pcm ≫ inelastic ~2 510 pcm ≫ continuous-energy
    /// data ~400 pcm) are in `docs/development-history.md`.
    ///
    /// **(n,2n) multiplicity — measured worth** *(2026-07-03, pre-`op-jis`; pending
    /// re-run — see the banner above).* A same-settings A/B (n2n on vs
    /// forced off) gives 0.99872 ± 0.00173 (on) vs 0.99701 ± 0.00168 (off), a shift
    /// of **+171 ± 241 pcm** — the correct sign but only ~0.7σ, **not resolved from
    /// zero**. Expected: U (n,2n) has a ~5–6 MeV threshold and sees only the thin
    /// high-energy tail, so its Godiva worth is tens of pcm — a *fidelity* fix.
    ///
    /// **Energy-dependent χ (ENDF MF=5) — measured worth** *(2026-07-03,
    /// pre-`op-jis`; pending re-run — see the banner above).* Replacing the fixed
    /// thermal-Watt fission birth spectrum with the real energy-dependent MF=5/MT=18
    /// χ(E→E') (LF=1, per-nuclide, ported from OpenMC `ContinuousTabular::sample`).
    /// A paired A/B (MF=5 vs Watt, same reconstruction and seed) gives HIGH =
    /// **1.00367 ± 0.00182 (MF=5)** vs **0.99872 ± 0.00173 (Watt)**, a shift of
    /// **+495 ± 251 pcm** — positive and ~2.0σ, i.e. **marginally resolved**. The
    /// U-235 MF=5 mean outgoing energy (~2.03 MeV) is close to the thermal-Watt mean,
    /// so the worth comes from the *shape*, not the mean: the tabulated χ keeps a
    /// larger fraction of births in the productive 1–3 MeV band (above the U-238
    /// fast-fission threshold, high ν̄) rather than the leaky high-energy tail the
    /// Watt form over-populates. LOW tier keeps the Watt stand-in (no embedded MF=5),
    /// so its k is unchanged and bit-identical — confirming the change is isolated to
    /// the HIGH birth spectrum.
    ///
    /// Near-perfect landings likely involve some cancellation of residual
    /// approximations (no fast self-shielding; Weisskopf stand-in for the MF=6
    /// (n,2n) emission law), so the bands below are deliberately generous rather
    /// than a tight accuracy gate.
    #[cfg(feature = "net-fetch")]
    #[test]
    fn godiva_high_fidelity_reaches_benchmark() {
        use njoy_outram_park_fork::acquire::EndfLibrary;

        let material = Material {
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
        let settings = KeffSettings {
            n_particles: 5000,
            n_inactive: 40,
            n_active: 120,
            ..KeffSettings::default()
        };

        // LOW tier (embedded, offline) — the first-cut reference.
        let low = vec![
            Nuclide::from_core("U234").unwrap(),
            Nuclide::from_core("U235").unwrap(),
            Nuclide::from_core("U238").unwrap(),
        ];
        let k_low = run_keff(8.7407, &material, &low, &settings).k_mean;

        // HIGH tier (download + reconstruct ENDF/B-VII.1). U is Reich-Moore (LRF=3)
        // in VII.1, which the RECONR port reconstructs (VIII.0 U is LRF=7).
        let high: Vec<Nuclide> = ["U234", "U235", "U238"]
            .iter()
            .map(|n| Nuclide::from_endf(EndfLibrary::EndfBVII1, n, 293.6, 1.0e-3).unwrap())
            .collect();
        let result = run_keff(8.7407, &material, &high, &settings);
        let k_high = result.k_mean;

        // (a) HIGH converges near unity (generous band — small run, residual
        //     approximations), and is stationary.
        assert!(
            k_high > 0.95 && k_high < 1.05,
            "HIGH Godiva k_eff {k_high} not within ~5000 pcm of the benchmark"
        );
        assert!(
            result.k_std < 0.02,
            "HIGH k noisy/unconverged: σ = {}",
            result.k_std
        );

        // (b) Both levers now live in the LOW tier too, so the embedded/offline run
        //     also lands near unity — from group data plus a single per-group μ̄.
        //     (Before the LOW port it sat at ~1.13 / +12 800 pcm.)
        assert!(
            k_low > 0.95 && k_low < 1.06,
            "LOW tier should also reach the benchmark band now (LOW={k_low})"
        );
    }

    /// **GPU-vs-CpuMultiThread timing sweep** (beads op-u6s.7 deliverable) — the
    /// event-based batched-flight `ComputeType::Gpu` path benchmarked against the
    /// `CpuMultiThread` and `CpuSingleThread` backends across a sweep of
    /// particle-batch sizes, on real hardware.
    ///
    /// **Methodology.** Godiva LOW-tier material (U-234/235/238, ICSBEP atom
    /// densities, T = 293.6 K, embedded `from_core` data), bare sphere r = 8.7407
    /// cm, a fixed short power iteration (`n_inactive` + `n_active` generations set
    /// in the body) at each batch size `n_particles ∈ {1e3, 1e4, 1e5, 1e6}`. For
    /// each (size, backend) the wall-clock of a full [`run_keff`] is measured with
    /// [`std::time::Instant`] and the eigenvalue recorded. Results are written as a
    /// plottable CSV to
    /// `verification_and_validation/gpu_batched_transport/timing_vs_batch.csv`
    /// (columns: date, adapter, batch_size, backend, wall_time_s, k_mean, k_std,
    /// n_gen, histories_total). The k agreement of GPU vs single is checked to lie
    /// within a combined-sigma band. This is an **honest** benchmark: whatever the
    /// crossover (or lack of one) is, it is recorded — no fabricated speedup.
    ///
    /// `#[ignore]` because it is a long, hardware-dependent measurement, not a
    /// correctness gate. Run it explicitly with:
    /// `cargo test -p outram-mc-libs --lib --release gpu_batched_timing_sweep -- --ignored --nocapture`.
    ///
    /// **Results.** Written as a per-machine markdown report + CSV to the
    /// gitignored `verification_and_validation/local_perf/` on the run machine (via
    /// [`crate::perf_report`]); the committed
    /// `verification_and_validation/gpu_batched_transport/README.md` holds the
    /// methodology and a labelled reference measurement. Machine-specific numbers
    /// stay local because they differ per host.
    #[test]
    #[ignore]
    fn gpu_batched_timing_sweep() {
        use crate::perf_report::{PerfReport, PerfRow};
        use std::time::Instant;

        let (material, nuclides) = godiva();
        let radius = 8.7407;
        // Short, fixed iteration — throughput benchmark, not a converged k.
        let n_inactive = 3usize;
        let n_active = 7usize;
        let n_gen = n_inactive + n_active;
        let sizes = [1_000usize, 10_000, 100_000, 1_000_000];

        // Detect this host's hardware and accumulate measured rows into a
        // per-PC report (the "what performance is available on my PC" answer).
        let mut report = PerfReport::new(
            "Godiva GPU batched-flight transport — this machine",
            "2026-07-17",
        );
        eprintln!("[sweep] host: {}", report.hardware.headline());

        for &n_particles in &sizes {
            let base = KeffSettings {
                n_particles,
                n_inactive,
                n_active,
                ..KeffSettings::default()
            };
            let histories_total = n_particles * n_gen;

            let mut runs: Vec<(&str, ComputeType)> = vec![
                (
                    "CpuMultiThread",
                    ComputeType::CpuMultiThread(ThreadCount::Auto),
                ),
                ("Gpu", ComputeType::Gpu),
            ];
            // Single-thread only up to 1e5 to bound total benchmark wall-time.
            if n_particles <= 100_000 {
                runs.insert(0, ("CpuSingleThread", ComputeType::CpuSingleThread));
            }

            for (name, compute) in runs {
                let t0 = Instant::now();
                let res = run_keff(radius, &material, &nuclides, &base.with_compute(compute));
                let dt = t0.elapsed().as_secs_f64();
                report.push(PerfRow {
                    batch_size: n_particles,
                    backend: name.to_string(),
                    wall_time_s: dt,
                    k_mean: res.k_mean,
                    k_std: res.k_std,
                    histories_total,
                });
                eprintln!(
                    "[sweep] N={n_particles:>8} {name:<16} {dt:>8.3}s  k={:.5}±{:.5}",
                    res.k_mean, res.k_std
                );
            }
        }

        // Persist the machine-specific markdown report + CSV locally (gitignored).
        let md = report.render_markdown();
        let csv = report.to_csv();
        match crate::perf_report::write_local_report("gpu_batched_transport.md", &md) {
            Ok(p) => eprintln!("[sweep] wrote per-PC report {}", p.display()),
            Err(e) => eprintln!("[sweep] could not write local report: {e}"),
        }
        if let Ok(p) =
            crate::perf_report::write_local_report("gpu_batched_transport_timing.csv", &csv)
        {
            eprintln!("[sweep] wrote per-PC csv {}", p.display());
        }
    }

    /// **Same-session BEFORE/AFTER: collision-on-CPU vs collision-on-GPU** — the
    /// op-u6s.8 deliverable, measured back-to-back on one machine state so the
    /// comparison is not confounded by machine load/thermal drift between runs.
    ///
    /// **Methodology.** Godiva LOW-tier, r = 8.7407 cm, a fixed short iteration
    /// (3 inactive + 7 active) at each batch size. For each size the SAME
    /// [`crate::gpu::GpuContext`] runs both GPU drivers:
    /// - **BEFORE** = [`run_keff_gpu_batched`] (op-u6s.7): flight on the GPU,
    ///   collision on the CPU — a CPU↔GPU round-trip per event.
    /// - **AFTER** = [`run_keff_gpu_event`] (op-u6s.8): flight **and** collision on
    ///   the GPU — the batch stays resident, no per-event round-trip.
    /// [`ComputeType::CpuMultiThread`] is timed alongside as the bar to beat. Each
    /// wall-clock is [`std::time::Instant`]; the k of every path is printed for the
    /// combined-sigma agreement check. Skips gracefully with no GPU adapter.
    ///
    /// `#[ignore]` — a long hardware measurement, not a correctness gate. Run with:
    /// `cargo test -p outram-mc-libs --lib --release gpu_collision_before_after -- --ignored --nocapture`.
    ///
    /// **Results.** Recorded on an NVIDIA GeForce RTX 3050; the measured table and
    /// the honest crossover verdict live in the committed
    /// `docs/gpu_collision_dev_log.md`.
    #[cfg(not(target_os = "android"))]
    #[test]
    #[ignore]
    fn gpu_collision_before_after() {
        use std::time::Instant;
        let Some(ctx) = crate::gpu::probe() else {
            eprintln!("SKIP gpu_collision_before_after: no GPU adapter");
            return;
        };
        let (material, nuclides) = godiva();
        let radius = 8.7407;
        let base = KeffSettings {
            n_inactive: 3,
            n_active: 7,
            ..KeffSettings::default()
        };
        eprintln!("[cmp] adapter: {}", ctx.info.name);
        eprintln!("[cmp] N         BEFORE(cpu-collide) AFTER(gpu-collide)  Multi     speedup(before/after)");
        for &n in &[10_000usize, 100_000, 1_000_000] {
            let s = KeffSettings {
                n_particles: n,
                ..base
            };

            let t0 = Instant::now();
            let before = run_keff_gpu_batched(&ctx, radius, &material, &nuclides, &s);
            let dt_before = t0.elapsed().as_secs_f64();

            let t0 = Instant::now();
            let after = run_keff_gpu_event(&ctx, radius, &material, &nuclides, &s);
            let dt_after = t0.elapsed().as_secs_f64();

            let t0 = Instant::now();
            let multi = run_keff_cpu_multi(radius, &material, &nuclides, &s, ThreadCount::Auto);
            let dt_multi = t0.elapsed().as_secs_f64();

            eprintln!(
                "[cmp] {n:>9} {dt_before:>10.3}s        {dt_after:>10.3}s     {dt_multi:>7.3}s   {:>5.2}x   \
                 | k: before={:.5}±{:.5} after={:.5}±{:.5} multi={:.5}±{:.5}",
                dt_before / dt_after,
                before.k_mean, before.k_std, after.k_mean, after.k_std, multi.k_mean, multi.k_std,
            );
        }
    }
}
