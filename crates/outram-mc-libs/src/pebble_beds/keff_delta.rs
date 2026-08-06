//! Doubly-heterogeneous k-eigenvalue power iteration driven by **delta (Woodcock)
//! tracking**.
//!
//! This is the assembly point for the random-packed TRISO k-eff: it composes the
//! [`super::sphere_packing`] packed geometry, the [`super::delta_tracking`]
//! flight primitives, and the crate's collision physics into a fission-source
//! power iteration — the doubly-heterogeneous analogue of
//! [`crate::physics::keff::run_keff`] (bare sphere) and
//! [`crate::physics::transport_csg::run_keff_csg`] (surface-tracked CSG).
//!
//! # Why delta tracking here
//!
//! In a packed TRISO medium a neutron's straight-line path crosses an enormous
//! number of kernel surfaces. Surface tracking must find the *nearest* of them at
//! every flight; delta tracking never looks for a surface at all. It samples the
//! flight on a **majorant** `Σ_maj(E) ≥ Σ_t(E)` bounding every material, lands at a
//! point, and asks only "**what material is here?**" — a point-membership test the
//! packed-sphere grid answers in O(1) ([`super::sphere_packing::PackedSpheres::is_inside_kernel`]).
//! The landing is a real collision with probability `Σ_t(local)/Σ_maj` and a
//! virtual (do-nothing) collision otherwise. See [`super::delta_tracking`] for the
//! primitives and their unit tests (unbiased mean free path, correct real/virtual
//! split).
//!
//! # Geometry model
//!
//! A **reflective cube** of half-width `half_width` (an infinite-medium unit cell:
//! neutrons reflect off the six walls, so the eigenvalue is the infinite-medium
//! `k∞` of the packed fuel, free of leakage). Inside the cube the caller's
//! `material_at` closure maps a point to a material index (kernel → fuel, else
//! matrix). The delta flight reflects the ray off the walls segment by segment, so
//! the neutron always lands at an interior point where `material_at` is defined.
//!
//! # Collision physics
//!
//! At each real collision the analog reaction partition — fission | capture |
//! inelastic | (n,2n) | elastic — mirrors [`crate::physics::keff`] /
//! [`crate::physics::transport_csg`] (the same [`crate::physics::scatter`] and
//! [`crate::physics::fission`] kernels). Only fission neutrons are banked to the
//! next generation; `(n,2n)` multiplicity is realized in-generation via a local
//! work stack. Fidelity matches those drivers: analog, target at rest, data tier
//! set by how the `nuclides` were built ([`Nuclide::from_core`] LOW /
//! [`Nuclide::from_endf`] HIGH).
//!
//! # Provenance
//!
//! The delta-tracking method is standard (Woodcock, ANL-7050, 1965; used in OpenMC,
//! Serpent, RMC). The collision partition mirrors OpenMC `src/physics.cpp`
//! (`collision` / `inelastic_scatter`). The reflective-cube flight is new pebble-bed
//! assembly built on this crate's primitives.

use crate::geometry::position::{Direction, Position};
use crate::material::material::Material;
use crate::material::nuclide::{Inelastic, Nuclide};
use crate::pebble_beds::delta_tracking::{classify_collision, sample_delta_distance, DeltaEvent, Majorant};
use crate::physics::compute::{ComputeType, ThreadCount};
use crate::physics::fission::sample_num_neutrons;
use crate::physics::keff::{KeffResult, KeffSettings};
use crate::physics::scatter::{
    continuum_inelastic_scatter, elastic_scatter, two_body_scatter, two_body_scatter_with_mu,
};
use crate::rng::distributions::{isotropic_direction, watt};
use crate::rng::lcg::{future_seed, prn};

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

/// A fission-source neutron awaiting transport in the next generation.
#[derive(Clone, Copy)]
struct Site {
    r: Position,
    u: Direction,
    e: f64,
}

/// Advance a ray by `distance` \[cm\] inside a reflective cube of half-width
/// `half`, reflecting off the walls, and return the landing position and the
/// (possibly reflected) direction.
///
/// Delta-tracking flights between collisions are straight lines under the
/// majorant, so a flight that would leave the cube instead reflects: the ray is
/// walked wall-to-wall, flipping the crossed axis's direction component each time,
/// until the full `distance` is consumed. The returned direction is what the
/// neutron continues along after a virtual collision.
///
/// The landing point is guaranteed to lie inside the closed cube (within
/// floating-point slack), so a subsequent material lookup is always defined.
fn advance_reflective(
    mut r: Position,
    mut u: Direction,
    mut distance: f64,
    half: f64,
) -> (Position, Direction) {
    // Guard against a pathological number of reflections (vanishing component).
    for _ in 0..10_000 {
        if distance <= 0.0 {
            break;
        }
        // Distance to the nearest wall along each axis (∞ if not moving on it).
        let t_axis = |p: f64, d: f64| -> f64 {
            if d > 0.0 {
                (half - p) / d
            } else if d < 0.0 {
                (-half - p) / d
            } else {
                f64::INFINITY
            }
        };
        let tx = t_axis(r.x, u.u);
        let ty = t_axis(r.y, u.v);
        let tz = t_axis(r.z, u.w);
        let t_wall = tx.min(ty).min(tz);

        if t_wall >= distance || !t_wall.is_finite() {
            r = Position::new(r.x + u.u * distance, r.y + u.v * distance, r.z + u.w * distance);
            break;
        }

        // Walk to the wall, flip the crossed component(s), consume the distance.
        r = Position::new(r.x + u.u * t_wall, r.y + u.v * t_wall, r.z + u.w * t_wall);
        let mut nu = u.u;
        let mut nv = u.v;
        let mut nw = u.w;
        if (tx - t_wall).abs() < 1e-15 {
            nu = -nu;
            r = Position::new(r.x.clamp(-half, half), r.y, r.z);
        }
        if (ty - t_wall).abs() < 1e-15 {
            nv = -nv;
            r = Position::new(r.x, r.y.clamp(-half, half), r.z);
        }
        if (tz - t_wall).abs() < 1e-15 {
            nw = -nw;
            r = Position::new(r.x, r.y, r.z.clamp(-half, half));
        }
        u = Direction::new(nu, nv, nw);
        distance -= t_wall;
    }
    // Numerical safety: keep the point strictly inside the closed cube.
    let r = Position::new(r.x.clamp(-half, half), r.y.clamp(-half, half), r.z.clamp(-half, half));
    (r, u)
}

/// Fly a neutron to its next **real** collision inside the reflective cube by
/// delta tracking, returning the collision position, the material index there, and
/// the direction it arrived along (for post-collision scattering).
///
/// Loops over virtual collisions internally: sample a flight on `majorant.at(e)`,
/// reflect-advance the ray, look up the local material and its Σ_t, and accept a
/// real collision with probability `Σ_t/Σ_maj`. Returns `None` if the virtual
/// budget is exhausted (a pathologically loose majorant) or the material lookup
/// unexpectedly fails — both leak the history, as in the surface-tracked drivers.
fn delta_flight<F>(
    start: Position,
    direction: Direction,
    energy: f64,
    half: f64,
    majorant: &Majorant,
    materials: &[Material],
    nuclides: &[Nuclide],
    max_virtual: u32,
    material_at: &F,
    seed: &mut u64,
) -> Option<(Position, usize, Direction)>
where
    F: Fn(Position) -> Option<usize>,
{
    let maj = majorant.at(energy);
    if !(maj > 0.0) {
        return None;
    }
    let mut r = start;
    let mut u = direction;
    for _ in 0..max_virtual {
        let s = sample_delta_distance(maj, seed);
        let (r_next, u_next) = advance_reflective(r, u, s, half);
        r = r_next;
        u = u_next;
        let m = material_at(r)?;
        let sigma_t = materials[m].macro_xs_total(energy, nuclides);
        match classify_collision(sigma_t, maj, seed) {
            DeltaEvent::Real => return Some((r, m, u)),
            DeltaEvent::Virtual => continue,
        }
    }
    None
}

/// Run fission-source power iteration over a **reflective cube** filled with a
/// two-(or-more-)material dispersion medium, transporting each history by delta
/// (Woodcock) tracking.
///
/// # Parameters
/// - `half_width` — half-width \[cm\] of the reflective cube (infinite-medium cell).
/// - `materials` — global material array; `material_at` returns indices into it.
/// - `nuclides` — global nuclide array the materials index into.
/// - `majorant` — a [`Majorant`] bounding `Σ_t(E)` of **every** material over the
///   full energy range the histories span (build it with
///   [`Majorant::from_materials`] on a broad grid).
/// - `material_at` — geometry lookup: the material index at a point inside the cube
///   (e.g. kernel → fuel, matrix → moderator). Must be defined everywhere inside
///   the closed cube; returning `None` leaks the history.
/// - `settings` — power-iteration controls (reuses [`KeffSettings`]).
///
/// Returns the mean eigenvalue and its standard error over the active generations.
/// The initial source is rejection-sampled uniformly in the cube for points in a
/// fissile material.
///
/// # Compute backend
///
/// This is a thin **dispatcher** over [`settings.compute`](KeffSettings::compute),
/// mirroring [`crate::physics::keff::run_keff`] and
/// [`crate::physics::transport_csg::run_keff_csg`]. The physics is identical
/// across backends; only the execution strategy differs:
///
/// - [`ComputeType::CpuSingleThread`] → [`run_keff_delta_seq`], the scalar,
///   single-RNG-stream **reference** — deterministic and bit-reproducible for a
///   fixed seed.
/// - [`ComputeType::CpuMultiThread`] → [`run_keff_delta_par`], [`rayon`]-parallel
///   histories per generation, each with an independent jump-ahead RNG stream so
///   the result is reproducible independent of thread count. It does **not**
///   bit-match the single-thread reference but agrees within combined statistical
///   uncertainty. (The `material_at` closure must be [`Sync`] to be shared across
///   threads — every geometry lookup in this crate already is.)
/// - [`ComputeType::Gpu`] → **no GPU kernel exists for delta-tracked
///   doubly-heterogeneous geometry**, so this transparently runs the
///   multi-threaded CPU path and emits a `log::debug!` line. It never errors on
///   the selection. Wiring a genuine GPU path into CSG/delta transport is tracked
///   as follow-up work (bead op-fla).
pub fn run_keff_delta<F>(
    half_width: f64,
    materials: &[Material],
    nuclides: &[Nuclide],
    majorant: &Majorant,
    material_at: F,
    settings: &KeffSettings,
) -> KeffResult
where
    F: Fn(Position) -> Option<usize> + Sync,
{
    match settings.compute {
        ComputeType::CpuSingleThread => {
            run_keff_delta_seq(half_width, materials, nuclides, majorant, material_at, settings)
        }
        ComputeType::CpuMultiThread(tc) => run_keff_delta_par(
            half_width, materials, nuclides, majorant, material_at, settings, tc,
        ),
        ComputeType::Gpu => {
            log::debug!(
                "ComputeType::Gpu requested for run_keff_delta, but no GPU kernel exists for \
                 delta-tracked doubly-heterogeneous geometry — running the multi-threaded CPU \
                 path instead"
            );
            run_keff_delta_par(
                half_width,
                materials,
                nuclides,
                majorant,
                material_at,
                settings,
                ThreadCount::Auto,
            )
        }
    }
}

/// Scalar, single-thread delta-tracked power iteration — the **trusted,
/// deterministic, bit-reproducible reference** backend
/// ([`ComputeType::CpuSingleThread`]).
///
/// One `f64` RNG stream is threaded sequentially through the whole run (initial
/// source rejection-sampling, every history's delta flight, every resample), so a
/// fixed [`KeffSettings::seed`] yields the same eigenvalue bit-for-bit on every
/// machine. [`run_keff_delta_par`] is acceleration only and is validated against
/// this reference.
pub fn run_keff_delta_seq<F>(
    half_width: f64,
    materials: &[Material],
    nuclides: &[Nuclide],
    majorant: &Majorant,
    material_at: F,
    settings: &KeffSettings,
) -> KeffResult
where
    F: Fn(Position) -> Option<usize>,
{
    let mut seed = settings.seed;
    let temp = settings.temperature_k;

    // Initial source: rejection-sample the cube for points in a fissile material.
    let mut source: Vec<Site> = Vec::with_capacity(settings.n_particles);
    let mut guard = 0usize;
    while source.len() < settings.n_particles {
        guard += 1;
        if guard > settings.n_particles * 10_000 {
            break; // pathological: fuel fills a vanishing fraction of the cube
        }
        let r = Position::new(
            -half_width + 2.0 * half_width * prn(&mut seed),
            -half_width + 2.0 * half_width * prn(&mut seed),
            -half_width + 2.0 * half_width * prn(&mut seed),
        );
        let fissile = material_at(r)
            .map(|m| materials[m].macro_xs(1.0e6, nuclides).nu_fission > 0.0)
            .unwrap_or(false);
        if fissile {
            let (dx, dy, dz) = isotropic_direction(&mut seed);
            source.push(Site {
                r,
                u: Direction::new(dx, dy, dz),
                e: watt(&mut seed, settings.watt_a, settings.watt_b),
            });
        }
    }

    let n_gen = settings.n_inactive + settings.n_active;
    let mut k_by_generation = Vec::with_capacity(n_gen);
    let mut k_running = 1.0;
    let mut active_k = Vec::with_capacity(settings.n_active);

    for gen in 0..n_gen {
        let mut next_bank: Vec<Site> = Vec::with_capacity(settings.n_particles);
        let mut production = 0.0_f64;

        for site in &source {
            production += transport_history(
                *site, half_width, materials, nuclides, majorant, temp, k_running,
                &material_at, &mut next_bank, &mut seed,
            );
        }

        let k_gen = production / settings.n_particles as f64;
        k_by_generation.push(k_gen);
        k_running = k_gen.max(1.0e-6);
        if gen >= settings.n_inactive {
            active_k.push(k_gen);
        }

        if next_bank.is_empty() {
            break;
        }
        source = resample(&next_bank, settings.n_particles, &mut seed);
    }

    let (k_mean, k_std) = mean_and_stderr(&active_k);
    KeffResult { k_mean, k_std, k_by_generation }
}

/// Rayon-parallel delta-tracked power iteration ([`ComputeType::CpuMultiThread`]).
///
/// Same physics and power-iteration structure as [`run_keff_delta_seq`], but the
/// histories **within each generation** are delta-tracked in parallel with
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
/// [`run_keff_delta_seq`] — it is a statistically independent estimate of the same
/// eigenvalue, agreeing within combined uncertainty.
///
/// The `material_at` geometry lookup is shared across threads by reference, so it
/// must be [`Sync`] (every packed-sphere / membership lookup in this crate is).
pub fn run_keff_delta_par<F>(
    half_width: f64,
    materials: &[Material],
    nuclides: &[Nuclide],
    majorant: &Majorant,
    material_at: F,
    settings: &KeffSettings,
    thread_count: ThreadCount,
) -> KeffResult
where
    F: Fn(Position) -> Option<usize> + Sync,
{
    use rayon::prelude::*;

    let temp = settings.temperature_k;

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

    // Initial source: rejection-sample the cube for points in a fissile material
    // (identical to the single-thread path, on the sequential src stream).
    let mut source: Vec<Site> = Vec::with_capacity(settings.n_particles);
    let mut guard = 0usize;
    while source.len() < settings.n_particles {
        guard += 1;
        if guard > settings.n_particles * 10_000 {
            break; // pathological: fuel fills a vanishing fraction of the cube
        }
        let r = Position::new(
            -half_width + 2.0 * half_width * prn(&mut src_seed),
            -half_width + 2.0 * half_width * prn(&mut src_seed),
            -half_width + 2.0 * half_width * prn(&mut src_seed),
        );
        let fissile = material_at(r)
            .map(|m| materials[m].macro_xs(1.0e6, nuclides).nu_fission > 0.0)
            .unwrap_or(false);
        if fissile {
            let (dx, dy, dz) = isotropic_direction(&mut src_seed);
            source.push(Site {
                r,
                u: Direction::new(dx, dy, dz),
                e: watt(&mut src_seed, settings.watt_a, settings.watt_b),
            });
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
            // Base seed for this generation's per-history sub-streams.
            let gen_base_seed = future_seed((gen as u64).wrapping_mul(GEN_STRIDE), settings.seed);

            // Delta-track every history in parallel. `into_par_iter().map(...).collect()`
            // on an indexed iterator preserves input order, so the reduction below
            // is deterministic regardless of thread count.
            let results: Vec<(f64, Vec<Site>)> = (0..source.len())
                .into_par_iter()
                .map(|hist_idx| {
                    // Independent, deterministic sub-stream for this history;
                    // owned locally — never shared across threads.
                    let mut seed =
                        future_seed((hist_idx as u64).wrapping_mul(HIST_STRIDE), gen_base_seed);
                    let mut local_bank: Vec<Site> = Vec::new();
                    let production = transport_history(
                        source[hist_idx],
                        half_width,
                        materials,
                        nuclides,
                        majorant,
                        temp,
                        k_running,
                        &material_at,
                        &mut local_bank,
                        &mut seed,
                    );
                    (production, local_bank)
                })
                .collect();

            // Deterministic sequential reduction: sum productions, concatenate
            // banks in history-index order.
            let mut production = 0.0_f64;
            let mut next_bank: Vec<Site> = Vec::with_capacity(settings.n_particles);
            for (prod, bank) in results {
                production += prod;
                next_bank.extend(bank);
            }

            let k_gen = production / settings.n_particles as f64;
            k_by_generation.push(k_gen);
            k_running = k_gen.max(1.0e-6);
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
    KeffResult { k_mean, k_std, k_by_generation }
}

/// Transport one source neutron (plus its same-generation `(n,2n)` secondaries) to
/// death by delta tracking, banking fission neutrons. Returns the fission
/// production ν̄ summed over the history's fission events.
///
/// Mirrors the analog reaction partition of [`crate::physics::keff`]; the only
/// difference is that streaming is done by [`delta_flight`] (Woodcock) rather than
/// surface tracking.
#[allow(clippy::too_many_arguments)]
fn transport_history<F>(
    site: Site,
    half: f64,
    materials: &[Material],
    nuclides: &[Nuclide],
    majorant: &Majorant,
    temp: f64,
    k_running: f64,
    material_at: &F,
    next_bank: &mut Vec<Site>,
    seed: &mut u64,
) -> f64
where
    F: Fn(Position) -> Option<usize>,
{
    // Safety cap on events per history — a purely-scattering reflective medium with
    // vanishing absorption could otherwise bounce forever (mirrors keff drivers).
    const MAX_EVENTS: u32 = 100_000;
    const MAX_VIRTUAL: u32 = 100_000;
    let mut production = 0.0;
    let mut stack: Vec<Site> = vec![site];

    while let Some(start) = stack.pop() {
        let mut r = start.r;
        let mut u = start.u;
        let mut e = start.e;
        let mut events = 0u32;

        'history: loop {
            events += 1;
            if events > MAX_EVENTS {
                break 'history; // give up on a stuck history (leak it)
            }

            let Some((r_col, m, u_arr)) = delta_flight(
                r, u, e, half, majorant, materials, nuclides, MAX_VIRTUAL, material_at, seed,
            ) else {
                break 'history; // leaked / virtual budget exhausted
            };
            r = r_col;
            u = u_arr;

            let material = &materials[m];
            let ci = material.sample_nuclide(e, seed, nuclides);
            let nuc = &nuclides[material.components[ci].nuclide_idx];
            let x = nuc.xs_at_energy(e, temp);

            // Reaction partition on the total: fission | capture | inelastic |
            // (n,2n) | elastic — identical to keff.rs / transport_csg.rs.
            let xi = prn(seed) * x.total;
            if xi < x.fission {
                let nu_bar = if x.fission > 0.0 { x.nu_fission / x.fission } else { 0.0 };
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
                break 'history; // radiative capture
            } else if xi < x.absorption + x.inelastic {
                let (e2, u2) = match nuc.sample_inelastic(e, seed) {
                    Inelastic::Level { q } => two_body_scatter(e, u, nuc.awr, q, seed),
                    Inelastic::Continuum => continuum_inelastic_scatter(e, u, nuc.awr, seed),
                };
                e = e2;
                u = u2;
            } else if xi < x.absorption + x.inelastic + x.n2n {
                let (e2, u2) = continuum_inelastic_scatter(e, u, nuc.awr, seed);
                stack.push(Site { r, u: u2, e: e2 }); // yield − 1 = 1 secondary
                e = e2;
                u = u2;
            } else {
                let (e2, u2) = match nuc.sample_elastic_mu_cm(e, seed) {
                    Some(mu_cm) => two_body_scatter_with_mu(e, u, nuc.awr, 0.0, mu_cm, seed),
                    None => elastic_scatter(e, u, nuc.awr, seed),
                };
                e = e2;
                u = u2;
            }
        }
    }
    production
}

/// Resample `n` sites uniformly with replacement — fixed-size population control.
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

    /// A reflective cube uniformly filled with one fissile material must give the
    /// same k as the surface-tracked bare-sphere driver would for the *infinite*
    /// medium — but here we just check delta tracking on a homogeneous reflective
    /// cube converges to a stationary, positive eigenvalue (the flight machinery is
    /// exercised; unbiasedness vs surface tracking is checked in the triso test).
    #[test]
    fn homogeneous_reflective_cube_converges() {
        use crate::material::material::NuclideComponent;
        let nuclides = vec![Nuclide::from_core("U235").unwrap()];
        let fuel = Material {
            id: 1,
            name: "U235".into(),
            temperature: 293.6,
            components: vec![NuclideComponent { nuclide_idx: 0, atom_density: 4.8e-2 }],
        };
        let materials = vec![fuel];
        let grid: Vec<f64> = (0..60).map(|i| 1.0e-3 * 1.5_f64.powi(i)).collect();
        let maj = Majorant::from_materials(&materials, &nuclides, &grid, 0.05);
        let settings = KeffSettings { n_particles: 500, n_inactive: 10, n_active: 20, ..KeffSettings::default() };

        let result = run_keff_delta(2.0, &materials, &nuclides, &maj, |_p| Some(0), &settings);
        assert!(!result.k_by_generation.is_empty());
        assert!(result.k_mean.is_finite() && result.k_mean > 0.0, "k = {}", result.k_mean);
    }

    /// V&V — **backend agreement**: the rayon multi-thread delta backend
    /// ([`run_keff_delta_par`], `ComputeType::CpuMultiThread`) must reproduce the
    /// single-thread reference ([`run_keff_delta_seq`]) within combined
    /// statistical uncertainty, and its result must be **independent of the
    /// thread count** (a consequence of the per-history jump-ahead seeding, which
    /// never shares a mutable seed).
    ///
    /// **Methodology.** One homogeneous fissile reflective cube (U-235, LOW/CORE
    /// data), identical [`KeffSettings`] and seed. Run the reference (seq) and the
    /// parallel backend at two different fixed thread counts (1 and 4). Pass
    /// criteria: (a) `k_par(1 thread) == k_par(4 threads)` bit-for-bit — the
    /// seeding is thread-count-invariant; (b) `|k_seq − k_par|` within `4·σ_comb`
    /// where `σ_comb = sqrt(σ_seq² + σ_par²)` — the two are statistically
    /// consistent estimates of the same eigenvalue (they do not bit-match by
    /// design, since the stream structure differs).
    ///
    /// **Results (2026-08-06, this environment, seed 987654321; 600 histories,
    /// 10 inactive + 30 active; re-measured under the `op-jis` PCG-RXS-M-XS `prn`
    /// output permutation).** The two thread-count runs agreed **to the bit**
    /// (`k_par(1) == k_par(4)`). Reference vs parallel: `k_seq = 2.23032 ±
    /// 0.00708`, `k_par = 2.21802 ± 0.00666`, **1.27σ apart** — well inside the 4σ
    /// gate. Arithmetic, so a reader can check it: `|Δk| = |2.23032 − 2.21802| =
    /// 0.01230`; `σ_comb = sqrt(0.00708² + 0.00666²) = sqrt(5.0126e-5 + 4.4356e-5)
    /// = sqrt(9.4482e-5) = 0.00972`; `0.01230 / 0.00972 = 1.27`; the 4σ band is
    /// `4 × 0.00972 = 0.03888`. (`k ≈ 2.2` is the infinite-medium `k∞`
    /// of a reflective HEU cube, not a critical assembly; it is the same physics
    /// both backends must agree on, which is what this test checks.) Recorded per
    /// the workspace V&V rule.
    ///
    /// **Supersedes (2026-07-23, measured with the pre-`op-jis` `prn` output
    /// function — raw top-52 state bits):** `k_seq = 2.23637 ± 0.00671`,
    /// `k_par = 2.22038 ± 0.00702`, 1.65σ apart (`σ_comb ≈ 0.0097`). The LCG state
    /// recurrence is unchanged, so pass criterion (a) — bit-for-bit thread-count
    /// invariance — is a *seeding* property and did not move at all; only the
    /// uniform doubles both backends consume changed, so both arms re-drew. No
    /// tolerance was changed; the 4σ gate is unaltered.
    #[test]
    fn delta_multithread_agrees_with_single_thread() {
        use crate::material::material::NuclideComponent;
        use crate::physics::compute::{ComputeType, ThreadCount};

        let nuclides = vec![Nuclide::from_core("U235").unwrap()];
        let fuel = Material {
            id: 1,
            name: "U235".into(),
            temperature: 293.6,
            components: vec![NuclideComponent { nuclide_idx: 0, atom_density: 4.8e-2 }],
        };
        let materials = vec![fuel];
        let grid: Vec<f64> = (0..60).map(|i| 1.0e-3 * 1.5_f64.powi(i)).collect();
        let maj = Majorant::from_materials(&materials, &nuclides, &grid, 0.05);
        let base = KeffSettings { n_particles: 600, n_inactive: 10, n_active: 30, seed: 987654321, ..KeffSettings::default() };

        // Reference: deterministic single-thread path (via the dispatcher).
        let seq = run_keff_delta(
            2.0, &materials, &nuclides, &maj, |_p| Some(0usize),
            &KeffSettings { compute: ComputeType::CpuSingleThread, ..base },
        );

        // Parallel path at two thread counts — must be bit-identical to each other.
        let par1 = run_keff_delta(
            2.0, &materials, &nuclides, &maj, |_p| Some(0usize),
            &KeffSettings { compute: ComputeType::CpuMultiThread(ThreadCount::Fixed(1)), ..base },
        );
        let par4 = run_keff_delta(
            2.0, &materials, &nuclides, &maj, |_p| Some(0usize),
            &KeffSettings { compute: ComputeType::CpuMultiThread(ThreadCount::Fixed(4)), ..base },
        );

        assert_eq!(
            par1.k_mean, par4.k_mean,
            "multi-thread k must be thread-count-invariant: 1-thread {} vs 4-thread {}",
            par1.k_mean, par4.k_mean
        );

        let sigma_comb = (seq.k_std.powi(2) + par1.k_std.powi(2)).sqrt().max(1e-9);
        let dist = (seq.k_mean - par1.k_mean).abs() / sigma_comb;
        eprintln!(
            "[delta backend agreement] seq = {:.5} ± {:.5}, par = {:.5} ± {:.5}  ({:.2}σ apart)",
            seq.k_mean, seq.k_std, par1.k_mean, par1.k_std, dist
        );
        assert!(
            dist <= 4.0,
            "seq k = {:.5} ± {:.5}, par k = {:.5} ± {:.5}: {:.2}σ apart (> 4σ)",
            seq.k_mean, seq.k_std, par1.k_mean, par1.k_std, dist
        );
    }

    /// `advance_reflective` keeps a ray inside the cube and conserves path length
    /// (the reflected polyline has the requested total length).
    #[test]
    fn reflective_advance_stays_in_cube() {
        let half = 1.0;
        let start = Position::new(0.0, 0.0, 0.0);
        let dir = Direction::from_unnormalised(1.0, 0.3, -0.7);
        let (end, _u) = advance_reflective(start, dir, 12.5, half);
        assert!(end.x.abs() <= half + 1e-9 && end.y.abs() <= half + 1e-9 && end.z.abs() <= half + 1e-9);
    }
}
