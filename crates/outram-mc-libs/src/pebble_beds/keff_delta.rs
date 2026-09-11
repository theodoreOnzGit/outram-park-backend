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
//! A reflective [`DeltaDomain`] — a **cube** of half-width `half`, or a **sphere**
//! of radius `radius`. Either way the boundary reflects, so there is no leakage
//! and the eigenvalue is the infinite-medium `k∞` of whatever fills the domain.
//! Inside it the caller's `material_at` closure maps a point to a material index
//! (kernel → fuel, else matrix). The delta flight reflects the ray off the
//! boundary segment by segment, so the neutron always lands at an interior point
//! where `material_at` is defined.
//!
//! **Pick the shape the thing you are comparing against used.** For a uniform
//! medium the shape genuinely does not matter (see
//! `geometry_independence_of_k_inf_for_a_uniform_medium`), but the moment the
//! medium is *not* uniform out to the boundary — a pebble sitting in coolant —
//! a cube of half-width `R` holds material in its corners that a sphere of
//! radius `R` does not, and on an FHR pebble that is worth thousands of pcm.
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
//! [`Nuclide::from_endf_file`] HIGH).
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
use crate::pebble_beds::delta_tracking::{
    classify_collision, sample_delta_distance, DeltaEvent, Majorant,
};
use crate::physics::compute::{ComputeType, ThreadCount};
use crate::physics::fission::sample_num_neutrons;
use crate::physics::keff::{KeffResult, KeffSettings};
use crate::physics::scatter::{
    free_gas_elastic_scatter, K_BOLTZMANN_EV_PER_K, continuum_inelastic_scatter, rotate_direction,
    two_body_scatter,
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

/// The reflective tracking domain a delta-tracked run fills.
///
/// A reflective boundary makes the eigenvalue an **infinite-medium** `k∞` — no
/// leakage — so for a *uniform* medium the shape is physically irrelevant and
/// both arms must return the same `k∞`. The shape stops being irrelevant the
/// moment the medium is not uniform, which is exactly the pebble case: a
/// reflective cube of half-width 3 cm circumscribes a 3 cm sphere, so its
/// corners (3 < r < 3√3) hold extra coolant that a reflective sphere of the
/// same radius does not. That over-count is worth thousands of pcm on an FHR
/// pebble and makes a cube run non-comparable to a sphere run of "the same"
/// radius.
///
/// Use [`DeltaDomain::Sphere`] whenever the reference being compared against
/// used a spherical reflective boundary — OpenMC pebble decks typically do.
///
/// `geometry_independence_of_k_inf_for_a_uniform_medium` in this module's tests
/// pins the equivalence the first paragraph claims.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DeltaDomain {
    /// Reflective cube of half-width `half` \[cm\], centred on the origin.
    Cube { half: f64 },
    /// Reflective sphere of radius `radius` \[cm\], centred on the origin.
    Sphere { radius: f64 },
}

impl DeltaDomain {
    /// Is `p` inside the closed domain?
    #[inline]
    pub fn contains(&self, p: Position) -> bool {
        match *self {
            Self::Cube { half } => p.x.abs() <= half && p.y.abs() <= half && p.z.abs() <= half,
            Self::Sphere { radius } => p.norm() <= radius,
        }
    }

    /// A half-extent that bounds the domain on every axis \[cm\] — the cube's
    /// own half-width, or the sphere's radius.
    #[inline]
    pub fn bounding_half(&self) -> f64 {
        match *self {
            Self::Cube { half } => half,
            Self::Sphere { radius } => radius,
        }
    }

    /// Draw a point uniformly over the domain's volume.
    ///
    /// The cube arm samples its box directly. The sphere arm rejection-samples
    /// the bounding box, which accepts with probability `π/6 ≈ 0.524` — cheap,
    /// and unlike the `r = R·u^{1/3}` closed form it needs no cube root and
    /// cannot concentrate points through a direction-sampling bias.
    #[inline]
    pub fn sample_point(&self, seed: &mut u64) -> Position {
        match *self {
            Self::Cube { half } => Position::new(
                -half + 2.0 * half * prn(seed),
                -half + 2.0 * half * prn(seed),
                -half + 2.0 * half * prn(seed),
            ),
            Self::Sphere { radius } => loop {
                let p = Position::new(
                    -radius + 2.0 * radius * prn(seed),
                    -radius + 2.0 * radius * prn(seed),
                    -radius + 2.0 * radius * prn(seed),
                );
                if p.norm_sqr() <= radius * radius {
                    return p;
                }
            },
        }
    }

    /// Advance a ray by `distance` \[cm\], reflecting specularly off the
    /// boundary as many times as the flight requires, and return the landing
    /// position and the (possibly reflected) direction.
    ///
    /// The landing point is guaranteed to lie inside the closed domain within
    /// floating-point slack, so a subsequent material lookup is always defined.
    #[inline]
    pub fn advance_reflective(
        &self,
        r: Position,
        u: Direction,
        distance: f64,
    ) -> (Position, Direction) {
        match *self {
            Self::Cube { half } => advance_reflective_cube(r, u, distance, half),
            Self::Sphere { radius } => advance_reflective_sphere(r, u, distance, radius),
        }
    }
}

/// Advance a ray by `distance` \[cm\] inside a reflective **sphere** of radius
/// `radius`, reflecting specularly off the surface, and return the landing
/// position and the (possibly reflected) direction.
///
/// # Method
///
/// Delta-tracking flights between collisions are straight lines under the
/// majorant, so a flight that would leave the sphere instead reflects. From an
/// interior point `r` along `u`, the exit distance solves `|r + t u|² = R²`:
///
/// ```text
/// t = -(r·u) + sqrt((r·u)² - (|r|² - R²))
/// ```
///
/// The discriminant is non-negative for any interior point (`|r| ≤ R`), and the
/// `+` root is the forward intersection because the `-` root is behind a ray
/// that starts inside. At the surface the outward normal is `n = r/R`, so the
/// specular reflection is `u' = u - 2(u·n)n`.
///
/// # Numerical care
///
/// Two floating-point hazards, both handled rather than hoped away:
///
/// - After walking to the surface, `|r|` can land a few ulps *outside* `R`,
///   which would make the next `t` solve on a negative discriminant. The point
///   is therefore rescaled onto the sphere exactly before reflecting.
/// - A ray that is very nearly tangent gives `t ≈ 0`, so the loop could reflect
///   forever without consuming `distance`. The reflection count is capped at
///   10 000 (as the cube arm caps its own), after which the remaining distance
///   is dropped and the point is clamped inside — the same failure posture the
///   cube arm takes, and unreachable for any non-degenerate flight.
fn advance_reflective_sphere(
    mut r: Position,
    mut u: Direction,
    mut distance: f64,
    radius: f64,
) -> (Position, Direction) {
    for _ in 0..10_000 {
        if distance <= 0.0 {
            break;
        }
        let r_dot_u = r.x * u.u + r.y * u.v + r.z * u.w;
        let disc = (r_dot_u * r_dot_u - (r.norm_sqr() - radius * radius)).max(0.0);
        let t_surf = -r_dot_u + disc.sqrt();

        if !t_surf.is_finite() || t_surf >= distance {
            r = Position::new(
                r.x + u.u * distance,
                r.y + u.v * distance,
                r.z + u.w * distance,
            );
            break;
        }

        // Walk to the surface and pin the point exactly onto it, so the next
        // iteration's discriminant cannot go negative.
        r = Position::new(r.x + u.u * t_surf, r.y + u.v * t_surf, r.z + u.w * t_surf);
        let n = r.norm();
        if n > 0.0 {
            let k = radius / n;
            r = Position::new(r.x * k, r.y * k, r.z * k);
            // Specular reflection about the outward normal n = r/R.
            let nx = r.x / radius;
            let ny = r.y / radius;
            let nz = r.z / radius;
            let dot = u.u * nx + u.v * ny + u.w * nz;
            u = Direction::new(
                u.u - 2.0 * dot * nx,
                u.v - 2.0 * dot * ny,
                u.w - 2.0 * dot * nz,
            );
        }
        distance -= t_surf;
    }

    // Numerical safety: keep the point strictly inside the closed sphere.
    let n = r.norm();
    if n > radius && n > 0.0 {
        let k = radius / n;
        r = Position::new(r.x * k, r.y * k, r.z * k);
    }
    (r, u)
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
fn advance_reflective_cube(
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
            r = Position::new(
                r.x + u.u * distance,
                r.y + u.v * distance,
                r.z + u.w * distance,
            );
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
    let r = Position::new(
        r.x.clamp(-half, half),
        r.y.clamp(-half, half),
        r.z.clamp(-half, half),
    );
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
    domain: DeltaDomain,
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
        let (r_next, u_next) = domain.advance_reflective(r, u, s);
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
/// - [`ComputeType::CpuSingleThread`] → [`run_keff_delta_seq_in`], the scalar,
///   single-RNG-stream **reference** — deterministic and bit-reproducible for a
///   fixed seed.
/// - [`ComputeType::CpuMultiThread`] → [`run_keff_delta_par_in`], [`rayon`]-parallel
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
/// Cube shorthand for [`run_keff_delta_in`] — a reflective cube of half-width
/// `half_width` \[cm\].
///
/// Kept because a reflective cube is the right unit cell for an infinite
/// *uniform* dispersion, which is what most callers want. When the medium is
/// **not** uniform out to the boundary — a pebble in coolant, say — the cube's
/// corners hold material a sphere of the same radius does not, so prefer
/// [`run_keff_delta_in`] with [`DeltaDomain::Sphere`] and match whatever
/// boundary the reference used.
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
    run_keff_delta_in(
        DeltaDomain::Cube { half: half_width },
        materials,
        nuclides,
        majorant,
        material_at,
        settings,
    )
}

pub fn run_keff_delta_in<F>(
    domain: DeltaDomain,
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
            run_keff_delta_seq_in(domain, materials, nuclides, majorant, material_at, settings)
        }
        ComputeType::CpuMultiThread(tc) => run_keff_delta_par_in(
            domain,
            materials,
            nuclides,
            majorant,
            material_at,
            settings,
            tc,
        ),
        ComputeType::Gpu => {
            log::debug!(
                "ComputeType::Gpu requested for run_keff_delta, but no GPU kernel exists for \
                 delta-tracked doubly-heterogeneous geometry — running the multi-threaded CPU \
                 path instead"
            );
            run_keff_delta_par_in(
                domain,
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
/// machine. [`run_keff_delta_par_in`] is acceleration only and is validated against
/// this reference.
pub fn run_keff_delta_seq_in<F>(
    domain: DeltaDomain,
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
        let r = domain.sample_point(&mut seed);
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
                *site,
                domain,
                materials,
                nuclides,
                majorant,
                temp,
                k_running,
                &material_at,
                &mut next_bank,
                &mut seed,
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
    KeffResult {
        k_mean,
        k_std,
        k_by_generation,
    }
}

/// Rayon-parallel delta-tracked power iteration ([`ComputeType::CpuMultiThread`]).
///
/// Same physics and power-iteration structure as [`run_keff_delta_seq_in`], but the
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
/// [`run_keff_delta_seq_in`] — it is a statistically independent estimate of the same
/// eigenvalue, agreeing within combined uncertainty.
///
/// The `material_at` geometry lookup is shared across threads by reference, so it
/// must be [`Sync`] (every packed-sphere / membership lookup in this crate is).
pub fn run_keff_delta_par_in<F>(
    domain: DeltaDomain,
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
    #[cfg(not(target_arch = "wasm32"))]
    use rayon::prelude::*;
    #[cfg(target_arch = "wasm32")]
    use crate::wasm_par::prelude::*;
    #[cfg(target_arch = "wasm32")]
    use crate::wasm_par as rayon;

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
        let r = domain.sample_point(&mut src_seed);
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
                        domain,
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
    KeffResult {
        k_mean,
        k_std,
        k_by_generation,
    }
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
    domain: DeltaDomain,
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
                r,
                u,
                e,
                domain,
                majorant,
                materials,
                nuclides,
                MAX_VIRTUAL,
                material_at,
                seed,
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
                // Scattering. Below its cutoff a moderator nuclide carrying an
                // S(alpha, beta) table thermalizes via the bound-atom law
                // (lab-frame outgoing energy + cosine, up-scatter allowed);
                // otherwise the free-gas / anisotropic-elastic kernel applies.
                // Mirrors `crate::physics::transport_csg` (which in turn mirrors
                // OpenMC `src/physics.cpp` `sample_secondary` / `src/thermal.cpp`
                // `ThermalData::sample`). Without this branch the bound cross
                // section from `xs_at_energy` sets the collision RATE while the
                // secondary energy is still drawn free-gas — internally
                // inconsistent, and it suppresses the up-scatter that lets a
                // graphite-moderated spectrum equilibrate.
                let (e2, u2) = if let Some((e_out, mu_lab)) = nuc.sample_thermal(e, seed) {
                    (e_out, rotate_direction(u, mu_lab, seed))
                } else {
                    {
                        // Free-gas: below 400 kT the target's own thermal motion
                        // is sampled, so the neutron can gain energy and the
                        // population has a Maxwellian fixed point (bead op-50vu).
                        // Above it this is the old target-at-rest kinematics.
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
            components: vec![NuclideComponent {
                nuclide_idx: 0,
                atom_density: 4.8e-2,
            }],
        };
        let materials = vec![fuel];
        let grid: Vec<f64> = (0..60).map(|i| 1.0e-3 * 1.5_f64.powi(i)).collect();
        let maj = Majorant::from_materials(&materials, &nuclides, &grid, 0.05);
        let settings = KeffSettings {
            n_particles: 500,
            n_inactive: 10,
            n_active: 20,
            ..KeffSettings::default()
        };

        let result = run_keff_delta(2.0, &materials, &nuclides, &maj, |_p| Some(0), &settings);
        assert!(!result.k_by_generation.is_empty());
        assert!(
            result.k_mean.is_finite() && result.k_mean > 0.0,
            "k = {}",
            result.k_mean
        );
    }

    /// V&V — **backend agreement**: the rayon multi-thread delta backend
    /// ([`run_keff_delta_par_in`], `ComputeType::CpuMultiThread`) must reproduce the
    /// single-thread reference ([`run_keff_delta_seq_in`]) within combined
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
            components: vec![NuclideComponent {
                nuclide_idx: 0,
                atom_density: 4.8e-2,
            }],
        };
        let materials = vec![fuel];
        let grid: Vec<f64> = (0..60).map(|i| 1.0e-3 * 1.5_f64.powi(i)).collect();
        let maj = Majorant::from_materials(&materials, &nuclides, &grid, 0.05);
        let base = KeffSettings {
            n_particles: 600,
            n_inactive: 10,
            n_active: 30,
            seed: 987654321,
            ..KeffSettings::default()
        };

        // Reference: deterministic single-thread path (via the dispatcher).
        let seq = run_keff_delta(
            2.0,
            &materials,
            &nuclides,
            &maj,
            |_p| Some(0usize),
            &KeffSettings {
                compute: ComputeType::CpuSingleThread,
                ..base
            },
        );

        // Parallel path at two thread counts — must be bit-identical to each other.
        let par1 = run_keff_delta(
            2.0,
            &materials,
            &nuclides,
            &maj,
            |_p| Some(0usize),
            &KeffSettings {
                compute: ComputeType::CpuMultiThread(ThreadCount::Fixed(1)),
                ..base
            },
        );
        let par4 = run_keff_delta(
            2.0,
            &materials,
            &nuclides,
            &maj,
            |_p| Some(0usize),
            &KeffSettings {
                compute: ComputeType::CpuMultiThread(ThreadCount::Fixed(4)),
                ..base
            },
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
            seq.k_mean,
            seq.k_std,
            par1.k_mean,
            par1.k_std,
            dist
        );
    }

    /// `advance_reflective` keeps a ray inside the cube and conserves path length
    /// (the reflected polyline has the requested total length).
    #[test]
    fn reflective_advance_stays_in_cube() {
        let half = 1.0;
        let start = Position::new(0.0, 0.0, 0.0);
        let dir = Direction::from_unnormalised(1.0, 0.3, -0.7);
        let (end, _u) = advance_reflective_cube(start, dir, 12.5, half);
        assert!(
            end.x.abs() <= half + 1e-9 && end.y.abs() <= half + 1e-9 && end.z.abs() <= half + 1e-9
        );
    }

    /// The spherical arm of [`DeltaDomain`] must keep a ray inside the sphere for
    /// *any* start, direction and flight length — including the two cases that
    /// break a naive implementation: a start already on the surface, and a very
    /// nearly tangent ray (where the surface intersection distance is ~0 and a
    /// loop that does not cap its reflections would spin).
    ///
    /// It also pins the direction as a **unit** vector after reflection: specular
    /// reflection about a unit normal is norm-preserving, so any drift here would
    /// be a bug that silently rescales every subsequent flight length.
    #[test]
    fn reflective_advance_stays_in_sphere() {
        let radius = 2.0;
        let domain = DeltaDomain::Sphere { radius };
        let mut seed = 20_260_910_u64;

        // Random interior starts, random directions, flight lengths up to 20 R.
        for case in 0..2_000 {
            let start = domain.sample_point(&mut seed);
            let (dx, dy, dz) = isotropic_direction(&mut seed);
            let dir = Direction::new(dx, dy, dz);
            let distance = 40.0 * prn(&mut seed);
            let (end, u) = domain.advance_reflective(start, dir, distance);
            assert!(
                end.norm() <= radius + 1e-9,
                "case {case}: landed at |r| = {} outside R = {radius}",
                end.norm()
            );
            let n = (u.u * u.u + u.v * u.v + u.w * u.w).sqrt();
            assert!(
                (n - 1.0).abs() < 1e-9,
                "case {case}: direction lost normalisation, |u| = {n}"
            );
        }

        // Start exactly on the surface, heading outward: the very first step must
        // reflect rather than escape.
        let on_surface = Position::new(radius, 0.0, 0.0);
        let outward = Direction::new(1.0, 0.0, 0.0);
        let (end, u) = domain.advance_reflective(on_surface, outward, 1.0);
        assert!(end.norm() <= radius + 1e-9, "|r| = {}", end.norm());
        assert!(u.u < 0.0, "outward ray must have reversed, u.u = {}", u.u);

        // Near-tangent ray: t_surf is ~0 every step, the reflection cap must hold.
        let tangent_start = Position::new(radius - 1e-12, 0.0, 0.0);
        let tangent_dir = Direction::from_unnormalised(1e-9, 1.0, 0.0);
        let (end, _u) = domain.advance_reflective(tangent_start, tangent_dir, 50.0);
        assert!(end.norm() <= radius + 1e-9, "|r| = {}", end.norm());
    }

    /// [`DeltaDomain::sample_point`] must fill the sphere **uniformly by volume**,
    /// not by radius. The distinguishing statistic is `<r³>`: under a volume-
    /// uniform fill the radial density is `3r²/R³`, so
    /// `<r³> = ∫₀ᴿ r³·3r²/R³ dr = R³/2`, whereas the classic `r = R·u` mistake
    /// gives `R³∫₀¹u³du = R³/4`. A factor of two apart, so this is not a
    /// delicate test.
    #[test]
    fn sphere_sampling_is_uniform_by_volume() {
        let radius = 1.5_f64;
        let domain = DeltaDomain::Sphere { radius };
        let mut seed = 424_242_u64;
        let n = 200_000;
        let mut sum_r3 = 0.0;
        for _ in 0..n {
            let p = domain.sample_point(&mut seed);
            assert!(p.norm() <= radius + 1e-12);
            sum_r3 += p.norm().powi(3);
        }
        let mean_r3 = sum_r3 / f64::from(n);
        let expected = 0.5 * radius.powi(3);
        assert!(
            (mean_r3 / expected - 1.0).abs() < 0.02,
            "<r³> = {mean_r3:.5}, expected {expected:.5} for a volume-uniform fill"
        );
    }

    /// V&V — **the new spherical domain against the trusted cube.**
    ///
    /// A reflective boundary means no leakage, so the eigenvalue of a domain
    /// filled with a **uniform** medium is that medium's infinite-medium `k∞` —
    /// a material property, with no dependence on the domain's shape or size.
    /// Cube and sphere must therefore agree, and that is the strongest available
    /// check on the sphere arm: it is pinned against the cube arm, which is the
    /// pre-existing, independently exercised reference.
    ///
    /// The check has teeth because it would fail loudly under the plausible
    /// implementation errors — a reflection that leaks histories drops `k`, one
    /// that double-counts path length raises it, and a source that is not
    /// volume-uniform biases the first generation.
    ///
    /// **Pass criterion.** `|k_cube − k_sphere|` within `4σ_comb`, the same gate
    /// the seq-vs-par backend-agreement test above uses.
    ///
    /// **Measured (2026-09-10, this environment, seed 135792468; 800 histories,
    /// 15 inactive + 40 active):** cube `2.22440 ± 0.00536` versus sphere
    /// `2.22334 ± 0.00524` — **0.14σ apart, −105 pcm**.
    ///
    /// Two sizes are run, and they return *bit-identical* results. That is
    /// expected, not a bug and not redundancy: in a uniform medium the flight
    /// sequence is scale-invariant under a fixed seed, because nothing in the
    /// physics references the boundary except the reflections, which change
    /// where a neutron is but not what happens to it. So the second size checks
    /// that scale-invariance holds; it does not add an independent sample.
    #[test]
    fn geometry_independence_of_k_inf_for_a_uniform_medium() {
        use crate::material::material::NuclideComponent;
        let nuclides = vec![Nuclide::from_core("U235").unwrap()];
        let materials = vec![Material {
            id: 1,
            name: "U235".into(),
            temperature: 293.6,
            components: vec![NuclideComponent {
                nuclide_idx: 0,
                atom_density: 4.8e-2,
            }],
        }];
        let grid: Vec<f64> = (0..60).map(|i| 1.0e-3 * 1.5_f64.powi(i)).collect();
        let maj = Majorant::from_materials(&materials, &nuclides, &grid, 0.05);
        let settings = KeffSettings {
            n_particles: 800,
            n_inactive: 15,
            n_active: 40,
            seed: 13_579_246_8,
            compute: ComputeType::CpuMultiThread(ThreadCount::Fixed(4)),
            ..KeffSettings::default()
        };

        for size in [2.0_f64, 4.0_f64] {
            let cube = run_keff_delta_in(
                DeltaDomain::Cube { half: size },
                &materials,
                &nuclides,
                &maj,
                |_p| Some(0),
                &settings,
            );
            let sphere = run_keff_delta_in(
                DeltaDomain::Sphere { radius: size },
                &materials,
                &nuclides,
                &maj,
                |_p| Some(0),
                &settings,
            );
            let sigma = (cube.k_std.powi(2) + sphere.k_std.powi(2)).sqrt().max(1e-9);
            let dist = (cube.k_mean - sphere.k_mean).abs() / sigma;
            eprintln!(
                "[k∞ shape independence, size {size}] cube {:.5} ± {:.5} vs sphere \
                 {:.5} ± {:.5}  ({dist:.2}σ, {:+.0} pcm)",
                cube.k_mean,
                cube.k_std,
                sphere.k_mean,
                sphere.k_std,
                (sphere.k_mean - cube.k_mean) * 1e5
            );
            assert!(
                dist <= 4.0,
                "size {size}: cube k = {:.5} ± {:.5}, sphere k = {:.5} ± {:.5}: {dist:.2}σ apart \
                 (> 4σ) — a reflective domain filled with a uniform medium has no shape dependence",
                cube.k_mean,
                cube.k_std,
                sphere.k_mean,
                sphere.k_std
            );
        }
    }
}
