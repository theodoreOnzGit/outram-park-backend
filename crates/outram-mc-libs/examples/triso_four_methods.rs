//! TRISO absorbing-kernel transport by **four** methods on one packing — Surface
//! tracking, Delta (Woodcock) tracking, CLS, and SCLS — with the pairwise
//! discrepancies recorded, not just the four numbers.
//!
//! Run it:
//! ```text
//! cargo run -p outram-mc-libs --release --example triso_four_methods
//! ```
//!
//! This is the four-way sibling of [`triso_delta_tracking`](triso_delta_tracking) and
//! [`triso_stochastic_media`](triso_stochastic_media). Those examples each show one
//! technique in isolation; this one puts all four on the *same* RSA packing, at the
//! *same* packing fraction, so their absorption-probability estimates are directly
//! comparable and the residual gaps between them are a measurement, not a guess.
//!
//! # The common problem
//!
//! An absorbing-kernel / scattering-matrix random walk in a small cube (mirrors
//! [`AbsorptionBenchmark`]'s problem, which this example both drives directly and
//! reuses for the CLS/SCLS arms):
//!
//! - Kernels (radius ~0.025 cm, a representative TRISO UO₂ kernel) are pure
//!   absorbers.
//! - The matrix is a pure isotropic scatterer, mean free path `scatter_mfp` \[cm\].
//! - Each history is born at a uniform random point in the cube (so birth-in-kernel
//!   probability equals the packing fraction, identically for every method), given
//!   an isotropic direction, and walked until it is **absorbed** (enters a kernel),
//!   **leaks** (crosses the cube wall), or **survives** `max_collisions` matrix
//!   scatters without either.
//! - The observable is the absorption probability.
//!
//! One RSA packing is generated per packing-fraction point (from `(radius,
//! packing_fraction, domain_half_width, seed)`, which is deterministic — see
//! [`PackingConfig::generate`]/`pack_spheres`'s reproducibility test) and every
//! method is run against that same packing (CLS/SCLS see only its aggregate
//! statistics — radius and packing fraction — not the explicit spheres, which is
//! the whole point of those two models).
//!
//! # The four methods
//!
//! | # | Method | Geometry | How it decides "hit a kernel?" |
//! |---|---|---|---|
//! | 1 | **Surface tracking** | explicit (all spheres) | exact ray–sphere intersection to the *nearest* kernel surface ahead, every step. O(N) per step. The gold reference — exact and unbiased by construction. |
//! | 2 | **Delta (Woodcock) tracking** | explicit (spatial hash) | majorant-sampled candidate points tested by O(1) [`PackedSpheres::is_inside_kernel`] lookup — never searches for a surface. |
//! | 3 | **CLS** | none (packing statistics only) | chord length resampled from an exponential fixed by radius + packing fraction; memoryless. |
//! | 4 | **SCLS** | bounded window of retained kernels | as CLS, but re-enters kernels already seen by this history. |
//!
//! Methods 1 and 2 both see the *same explicit geometry*; they differ only in how
//! they find the next kernel. That makes **Surface vs Delta** the pure algorithmic
//! consistency check — any statistically significant gap there means the delta
//! tracker's majorant, sampling, or bookkeeping is wrong, not that the physics
//! differs. **CLS vs Surface** and **SCLS vs Surface** are a different kind of
//! comparison: both stochastic-media models discard geometry, so their gap against
//! the exact reference is genuine *model* (chord-sampling) approximation error, not
//! a bug.
//!
//! # Delta tracking: why a *pseudo* absorption cross section, not literal membership
//!
//! The naive version of "test `is_inside_kernel` at the majorant-sampled point;
//! real ⇒ absorbed, else virtual" is only unbiased in the limit of an infinitely
//! fine majorant. Woodcock's thinning-theorem proof of unbiasedness requires
//! `Σ_maj(x) ≥ Σ_t(x)` **pointwise**, including *inside* the kernel — a genuinely
//! opaque "instant capture on entry" kernel has no such finite `Σ_t`, and a
//! majorant sampled too coarsely relative to the kernel diameter can step clean
//! over a thin kernel without ever landing inside it, silently under-counting
//! absorptions.
//!
//! This example avoids that by giving the kernel phase a large but *finite* pseudo
//! absorption cross section `Σ_kernel = KERNEL_MAJORANT_MARGIN / particle_radius`
//! (matrix phase keeps its real `Σ_scatter = 1/scatter_mfp`), and majorant
//! `Σ_maj = max(Σ_scatter, Σ_kernel)`. With `KERNEL_MAJORANT_MARGIN = 8`, the mean
//! candidate spacing is `radius / 8`, so the expected number of majorant samples
//! landing inside a traversed kernel (mean chord `4·radius/3`) is
//! `(4/3)·8 ≈ 10.7`; the probability of missing every one of them is
//! `exp(-10.7) ≈ 2×10⁻⁵` — far below the ~1.8 % binomial standard error at 3000
//! histories, so the residual bias from a finite majorant is not detectable in
//! this run. This is the same Poisson-thinning argument that makes ordinary
//! Woodcock tracking exact for a smooth `Σ_t(x)` field, applied to a kernel phase
//! whose `Σ_t` is finite-but-large rather than infinite.
//!
//! # Honesty banner
//!
//! **No method is claimed superior.** Surface tracking is the unbiased reference
//! by construction (exact ray geometry); delta tracking is expected to reproduce it
//! within statistics because it tracks the *same* explicit geometry, only sampled
//! differently — that agreement is the consistency anchor this example exists to
//! demonstrate, not an assumption. CLS and SCLS are approximations that discard
//! geometry; whether either is closer to the reference, and by how much, is a
//! regime-dependent empirical question this sweep *measures*. See the printed
//! table's interpretation footer and [`AbsorptionBenchmark`]'s own recorded result
//! for a second, independently-generated data point.
//!
//! # Measured result (2026-07-21, seed 20260721, 3000 histories/method/arm)
//!
//! `domain_half_width = 0.3 cm`, `particle_radius = 0.025 cm`,
//! `scatter_mfp = 0.2 cm`, `max_collisions = 300`, `KERNEL_MAJORANT_MARGIN = 8`.
//! This is the literal output of a run of this example (`cargo run -p
//! outram-mc-libs --release --example triso_four_methods`), not a hand-derived
//! estimate:
//!
//! | pf | Surface (ref) | Delta | Δ Delta (z) | CLS | Δ CLS | SCLS | Δ SCLS |
//! |---|---|---|---|---|---|---|---|
//! | 0.05 | 0.4157 (se 0.0090) | 0.4000 (se 0.0089) | -0.0157 (z=-1.23) | 0.3410 | -0.0747 | 0.3930 | -0.0227 |
//! | 0.10 | 0.5923 (se 0.0090) | 0.5797 (se 0.0090) | -0.0127 (z=-1.00) | 0.5407 | -0.0517 | 0.5877 | -0.0047 |
//! | 0.15 | 0.6840 (se 0.0085) | 0.6713 (se 0.0086) | -0.0127 (z=-1.05) | 0.6613 | -0.0227 | 0.7043 | +0.0203 |
//! | 0.20 | 0.7460 (se 0.0079) | 0.7437 (se 0.0080) | -0.0023 (z=-0.21) | 0.7363 | -0.0097 | 0.7667 | +0.0207 |
//! | 0.25 | 0.7897 (se 0.0074) | 0.7850 (se 0.0075) | -0.0047 (z=-0.44) | 0.7953 | +0.0057 | 0.8140 | +0.0243 |
//!
//! `se` is the binomial standard error `sqrt(p(1-p)/n)`, n = 3000; `z` is
//! `ΔDelta / sqrt(se_surface² + se_delta²)`, printed by the program itself under
//! each row (not recomputed by hand here).
//!
//! **Interpretation.**
//!
//! - **Surface vs Delta (the consistency anchor): holds.** Every `z` above has
//!   `|z| ≤ 1.23` — well inside normal Monte Carlo scatter for two independent
//!   3000-history samples of the same underlying probability. Both arms track the
//!   identical explicit packing; this is the evidence the delta-tracking majorant
//!   (§ above) and bookkeeping are unbiased at this margin, not an assumption.
//!   (An earlier draft of this walk *did* fail this check by ~5-6 σ at several
//!   packing fractions — traced to `walk_surface` never testing whether the birth
//!   point itself already lands inside a kernel, only ray-casting for crossings
//!   *ahead*. Delta tracking's fine Woodcock stepping caught birth-in-kernel
//!   histories incidentally; the ray-cast arm silently missed them, systematically
//!   under-counting absorption in proportion to the packing fraction. Fixed by an
//!   explicit `is_inside_kernel` check on the birth point before ray-casting — see
//!   `walk_surface`'s doc comment. Recorded here because a passing consistency
//!   check that silently started out failing is exactly the kind of thing this
//!   module exists to catch and report honestly.)
//! - **CLS and SCLS: no universal winner.** CLS underestimates absorption at
//!   every packing fraction tested except the densest (from -0.0747 at pf = 0.05
//!   to +0.0057 at pf = 0.25); SCLS starts by underestimating too (-0.0227 at
//!   pf = 0.05) but crosses over to overestimate from pf = 0.15 upward (+0.0203 to
//!   +0.0243). At pf = 0.05, SCLS (|Δ| = 0.0227) is much closer to the reference
//!   than CLS (|Δ| = 0.0747); at pf = 0.25 the ranking **inverts** — CLS
//!   (|Δ| = 0.0057) is closer than SCLS (|Δ| = 0.0243). Several of these gaps are
//!   many standard errors wide (e.g. CLS at pf = 0.05 is ~8-9 σ from Surface),
//!   which is expected for a *systematic* model-approximation bias rather than
//!   statistical noise — CLS and SCLS are deterministic functions of the packing
//!   statistics, not unbiased estimators of the exact geometry, so a
//!   multi-sigma gap reports real approximation error, not a bug (contrast with
//!   the Surface/Delta anchor above, where the same size of gap *would* indicate
//!   one). The crossover itself — no method uniformly closer across the sweep —
//!   is the honest, regime-dependent finding this table exists to surface, not an
//!   assertion that either model is generally better.
//!
//! # Provenance
//!
//! Geometry model — random-packed TRISO fuel kernels in a matrix — follows the
//! OpenMC `triso.ipynb` notebook (openmc-dev/openmc-notebooks); kernel radius
//! ~0.025 cm is a representative TRISO UO₂ kernel. MIT-licensed OpenMC project
//! material, used here as a modelling reference and adapted to a tractable unit
//! cell (cited per `RESPONSIBLE_USE.md`). Surface tracking's ray–sphere entry
//! formula mirrors the private helper already used by
//! [`outram_mc_libs::stochastic::scls`] (re-derived here, not imported, since it is
//! a private item there). Delta (Woodcock) tracking's flight sample reuses
//! [`outram_mc_libs::pebble_beds::delta_tracking::sample_delta_distance`]'s
//! `-ln(xi)/majorant` form (inlined here rather than calling
//! [`outram_mc_libs::pebble_beds::delta_tracking::track_to_collision`], which is
//! built around real nuclide cross sections rather than this toy two-phase rate
//! model). CLS/SCLS are **new work**, not an OpenMC port — upstream has no
//! chord-length sampling.

use outram_mc_libs::geometry::position::{Direction, Position};
use outram_mc_libs::pebble_beds::sphere_packing::{PackedSpheres, PackingConfig, PackingMethod, Sphere};
use outram_mc_libs::rng::distributions::isotropic_direction;
use outram_mc_libs::rng::lcg::prn;
use outram_mc_libs::stochastic::benchmark::{AbsorptionBenchmark, BenchmarkResult};

/// Safety margin for the delta-tracking kernel pseudo cross section — see the
/// module docs' "why a pseudo cross section" section. `Σ_kernel = MARGIN /
/// particle_radius`; larger values sample more finely inside a kernel (lower miss
/// probability) at the cost of more virtual collisions (slower, not biased).
const KERNEL_MAJORANT_MARGIN: f64 = 8.0;

/// How one history of the absorbing-kernel random walk ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Outcome {
    /// Entered a kernel (absorber).
    Absorbed,
    /// Left the cube domain.
    Leaked,
    /// Reached `max_collisions` real matrix scatters without either.
    Survived,
}

/// Shared configuration for the common absorbing-kernel / scattering-matrix
/// problem, driven independently by the hand-rolled surface-tracking and
/// delta-tracking walks below. Field names and semantics mirror
/// [`AbsorptionBenchmark`] so the four methods solve an identical problem.
#[derive(Debug, Clone, Copy)]
struct FourMethodProblem {
    /// Half-width of the cube domain \[cm\].
    domain_half_width: f64,
    /// Kernel (absorber) radius \[cm\].
    particle_radius: f64,
    /// Kernel volumetric packing fraction, in (0, 1). Not read by the hand-rolled
    /// walks below (they get it implicitly via the packing/majorant the caller
    /// builds), but kept for parity with [`AbsorptionBenchmark`]'s field set and
    /// printed in `main`'s sweep.
    #[allow(dead_code)]
    packing_fraction: f64,
    /// Matrix scatter mean free path \[cm\].
    scatter_mfp: f64,
    /// Maximum real matrix scatters before a history is declared "survived".
    max_collisions: usize,
    /// Histories per method.
    histories: usize,
}

impl FourMethodProblem {
    /// A uniform random point in the cube domain \[cm\].
    fn birth(&self, seed: &mut u64) -> Position {
        let w = self.domain_half_width;
        Position::new(
            (2.0 * prn(seed) - 1.0) * w,
            (2.0 * prn(seed) - 1.0) * w,
            (2.0 * prn(seed) - 1.0) * w,
        )
    }

    /// Exact distance \[cm\] from `p` along `d` to the cube wall — the slab test,
    /// not a fine march. Used by both hand-rolled walks so leakage is decided
    /// exactly rather than by stepping past the boundary and checking after.
    fn distance_to_wall(&self, p: Position, d: Direction) -> f64 {
        let w = self.domain_half_width;
        let axis = |x: f64, dx: f64| -> f64 {
            if dx > 0.0 {
                (w - x) / dx
            } else if dx < 0.0 {
                (-w - x) / dx
            } else {
                f64::INFINITY
            }
        };
        axis(p.x, d.u).min(axis(p.y, d.v)).min(axis(p.z, d.w))
    }

    /// Method 1 — **surface tracking**: one history of the exact-geometry walk.
    ///
    /// At every step, computes three exact distances — to the next matrix
    /// scatter (exponential, mean `scatter_mfp`), to the cube wall (slab test),
    /// and to the *nearest* kernel surface ahead ([`nearest_kernel_entry`], an
    /// O(N) scan of every packed sphere) — and takes whichever is smallest. This
    /// is the unbiased, if expensive, gold reference every other method is
    /// measured against.
    ///
    /// The birth point itself is tested for kernel membership up front (an O(1)
    /// [`PackedSpheres::is_inside_kernel`] lookup): [`ray_sphere_entry`] only
    /// finds crossings *ahead* of a ray, so without this check a history born
    /// inside a kernel would sail straight through it undetected — the ray's
    /// near root for the kernel it started in is behind it, not ahead. That
    /// under-counted the birth-in-kernel population (probability `≈
    /// packing_fraction` for every history) until caught by comparison against
    /// delta tracking's fine Woodcock stepping, which catches it incidentally.
    fn walk_surface(&self, packing: &PackedSpheres, seed: &mut u64) -> Outcome {
        let mut pos = self.birth(seed);
        if packing.is_inside_kernel(pos) {
            return Outcome::Absorbed;
        }
        let (du, dv, dw) = isotropic_direction(seed);
        let mut dir = Direction::new(du, dv, dw);
        let spheres = packing.spheres();

        for _ in 0..self.max_collisions {
            let d_scatter = -self.scatter_mfp * (1.0 - prn(seed)).ln();
            let d_wall = self.distance_to_wall(pos, dir);
            let d_kernel = nearest_kernel_entry(pos, dir, spheres, 1e-9).unwrap_or(f64::INFINITY);

            if d_wall <= d_scatter && d_wall <= d_kernel {
                return Outcome::Leaked;
            }
            if d_kernel <= d_scatter {
                return Outcome::Absorbed;
            }
            // Matrix scatter: advance exactly to the scatter point, resample direction.
            pos = Position::new(pos.x + dir.u * d_scatter, pos.y + dir.v * d_scatter, pos.z + dir.w * d_scatter);
            let (nu, nv, nw) = isotropic_direction(seed);
            dir = Direction::new(nu, nv, nw);
        }
        Outcome::Survived
    }

    /// Method 2 — **delta (Woodcock) tracking**: one history of the majorant-sampled
    /// walk over the same explicit geometry as [`Self::walk_surface`], but never
    /// searching for a surface — only O(1) [`PackedSpheres::is_inside_kernel`]
    /// lookups at majorant-sampled candidate points. See the module docs for why
    /// the kernel phase is given a large finite pseudo cross section rather than
    /// treating point-membership as a deterministic 100 %-real test.
    ///
    /// `sigma_scatter`/`sigma_kernel`/`sigma_maj` are precomputed once per packing
    /// fraction by the caller (they do not depend on the history).
    fn walk_delta(
        &self,
        packing: &PackedSpheres,
        sigma_scatter: f64,
        sigma_kernel: f64,
        sigma_maj: f64,
        seed: &mut u64,
    ) -> Outcome {
        let mut pos = self.birth(seed);
        let (du, dv, dw) = isotropic_direction(seed);
        let mut dir = Direction::new(du, dv, dw);

        let mut real_scatters = 0usize;
        // Safety valve against a pathologically loose majorant looping forever;
        // should never trigger for the margin chosen in `main` (see module docs).
        let mut woodcock_steps = 0usize;
        const MAX_WOODCOCK_STEPS: usize = 2_000_000;

        loop {
            if real_scatters >= self.max_collisions {
                return Outcome::Survived;
            }
            woodcock_steps += 1;
            if woodcock_steps > MAX_WOODCOCK_STEPS {
                return Outcome::Survived;
            }

            let d_wall = self.distance_to_wall(pos, dir);
            let s = -prn(seed).max(f64::MIN_POSITIVE).ln() / sigma_maj; // sample_delta_distance form
            if s >= d_wall {
                return Outcome::Leaked;
            }
            pos = Position::new(pos.x + dir.u * s, pos.y + dir.v * s, pos.z + dir.w * s);

            let inside = packing.is_inside_kernel(pos);
            let sigma_t_local = if inside { sigma_kernel } else { sigma_scatter };
            let p_real = (sigma_t_local / sigma_maj).clamp(0.0, 1.0);

            if prn(seed) < p_real {
                if inside {
                    return Outcome::Absorbed;
                }
                // Real matrix scatter: resample direction, count it, keep tracking.
                let (nu, nv, nw) = isotropic_direction(seed);
                dir = Direction::new(nu, nv, nw);
                real_scatters += 1;
            }
            // else: virtual collision — continue with the same direction from `pos`.
        }
    }

    /// Run `histories` surface-tracking walks and tally the outcomes.
    fn run_surface(&self, packing: &PackedSpheres, seed: &mut u64) -> BenchmarkResult {
        let mut absorbed = 0usize;
        let mut leaked = 0usize;
        for _ in 0..self.histories {
            match self.walk_surface(packing, seed) {
                Outcome::Absorbed => absorbed += 1,
                Outcome::Leaked => leaked += 1,
                Outcome::Survived => {}
            }
        }
        let n = self.histories.max(1) as f64;
        BenchmarkResult {
            model: "Surface (exact ray-cast)",
            absorption_probability: absorbed as f64 / n,
            leakage_probability: leaked as f64 / n,
            histories: self.histories,
        }
    }

    /// Run `histories` delta-tracking walks and tally the outcomes.
    fn run_delta(&self, packing: &PackedSpheres, seed: &mut u64) -> BenchmarkResult {
        let sigma_scatter = 1.0 / self.scatter_mfp;
        let sigma_kernel = KERNEL_MAJORANT_MARGIN / self.particle_radius;
        let sigma_maj = sigma_scatter.max(sigma_kernel);

        let mut absorbed = 0usize;
        let mut leaked = 0usize;
        for _ in 0..self.histories {
            match self.walk_delta(packing, sigma_scatter, sigma_kernel, sigma_maj, seed) {
                Outcome::Absorbed => absorbed += 1,
                Outcome::Leaked => leaked += 1,
                Outcome::Survived => {}
            }
        }
        let n = self.histories.max(1) as f64;
        BenchmarkResult {
            model: "Delta (Woodcock)",
            absorption_probability: absorbed as f64 / n,
            leakage_probability: leaked as f64 / n,
            histories: self.histories,
        }
    }
}

/// Distance from an exterior point `p` along unit direction `u` to where the ray
/// first **enters** the sphere `(center, radius)` \[cm\], or `None` if the ray
/// misses it or the entry lies behind `p` (within `eps`).
///
/// Solves `|p + t·u − center|² = radius²` for the near (smaller, positive) root:
/// with `b = u·(p − center)` and `c = |p − center|² − radius²`, the near root is
/// `t = -b - sqrt(b² - c)`. Re-derives the same math as the private
/// `ray_sphere_entry` in [`outram_mc_libs::stochastic::scls`] (not imported —
/// that one is a private module item), specialised here to always start outside
/// a kernel (matrix-only walk between kernels).
fn ray_sphere_entry(p: Position, u: Direction, center: Position, radius: f64, eps: f64) -> Option<f64> {
    let oc = p - center;
    let b = u.dot_pos(oc);
    let c = oc.norm_sqr() - radius * radius;
    let disc = b * b - c;
    if disc < 0.0 {
        return None; // ray misses the sphere entirely
    }
    let t = -b - disc.sqrt();
    if t > eps {
        Some(t)
    } else {
        None // sphere is behind (or the ray starts essentially on its surface)
    }
}

/// Distance to the *nearest* kernel surface ahead of `pos` along `dir`, scanning
/// every packed sphere — the O(N) operation delta tracking exists to avoid.
fn nearest_kernel_entry(pos: Position, dir: Direction, spheres: &[Sphere], eps: f64) -> Option<f64> {
    spheres
        .iter()
        .filter_map(|s| ray_sphere_entry(pos, dir, s.center, s.radius, eps))
        .fold(None, |acc, t| Some(acc.map_or(t, |a: f64| a.min(t))))
}

/// Binomial standard error `sqrt(p(1-p)/n)` of an absorption-probability estimate
/// from `n` independent histories. Used to judge whether a Δ column in the sweep
/// table is a statistically significant gap or within Monte Carlo noise.
fn binomial_se(p: f64, n: usize) -> f64 {
    (p * (1.0 - p) / n.max(1) as f64).sqrt()
}

fn main() {
    println!("TRISO absorbing-kernel transport — four methods, one packing per packing fraction");
    println!("====================================================================================\n");

    let base = FourMethodProblem {
        domain_half_width: 0.3, // 0.6 cm cube
        particle_radius: 0.025, // ~250 µm TRISO kernel
        packing_fraction: 0.2,  // overwritten in the sweep below
        scatter_mfp: 0.2,       // matrix scatter mean free path [cm]
        max_collisions: 300,
        histories: 3000,
    };
    let seed = 20260721u64;

    println!(
        "Absorbing-kernel random walk ({} histories/method, kernel r = {} cm, matrix scatter",
        base.histories, base.particle_radius
    );
    println!(
        "mfp = {} cm, domain half-width = {} cm). Surface tracking is the exact reference;\n\
         delta tracking shares its geometry (consistency anchor); CLS/SCLS see only the\n\
         packing's aggregate statistics (approximation error).\n",
        base.scatter_mfp, base.domain_half_width
    );
    println!(
        "{:>5} | {:>16} | {:>16} {:>9} | {:>16} {:>9} | {:>16} {:>9}",
        "pf", "Surface (ref)", "Delta", "ΔDelta", "CLS", "ΔCLS", "SCLS", "ΔSCLS"
    );
    println!(
        "{:-<5}-+-{:-<16}-+-{:-<16}-{:-<9}-+-{:-<16}-{:-<9}-+-{:-<16}-{:-<9}",
        "", "", "", "", "", "", "", ""
    );

    for &pf in &[0.05_f64, 0.10, 0.15, 0.20, 0.25] {
        let problem = FourMethodProblem { packing_fraction: pf, ..base };

        let cfg = PackingConfig {
            particle_radius: problem.particle_radius,
            packing_fraction: pf,
            domain_half_width: problem.domain_half_width,
            method: PackingMethod::Rsa,
            seed,
        };

        let spheres = match cfg.generate() {
            Ok(s) => s,
            Err(e) => {
                println!("{pf:>5.2} | packing failed: {e}");
                continue;
            }
        };
        // The one RSA packing this packing-fraction point runs all four methods on.
        let packing = PackedSpheres::from_spheres(spheres, problem.domain_half_width, problem.particle_radius);

        // Independent LCG streams for the two hand-rolled arms (distinct from the
        // masks AbsorptionBenchmark::compare uses internally for its own RSA/CLS/SCLS
        // streams, so all four method's random numbers are uncorrelated).
        let mut s_surface = seed ^ 0xA5A5_A5A5_A5A5_A5A5;
        let mut s_delta = seed ^ 0xC3C3_C3C3_C3C3_C3C3;

        let surface = problem.run_surface(&packing, &mut s_surface);
        let delta = problem.run_delta(&packing, &mut s_delta);

        // AbsorptionBenchmark::generate()s an RSA packing from the identical
        // (radius, pf, half_width, seed) tuple used above — deterministic, so this
        // is bit-for-bit the same packing (see `pack_spheres`'s reproducibility
        // test) — and derives its CLS/SCLS arms from it.
        let bench = AbsorptionBenchmark {
            domain_half_width: problem.domain_half_width,
            particle_radius: problem.particle_radius,
            packing_fraction: pf,
            scatter_mfp: problem.scatter_mfp,
            max_collisions: problem.max_collisions,
            histories: problem.histories,
        };
        match bench.compare(seed) {
            Ok([_rsa_finemarch, cls, scls]) => {
                let d_delta = delta.absorption_probability - surface.absorption_probability;
                let d_cls = cls.absorption_probability - surface.absorption_probability;
                let d_scls = scls.absorption_probability - surface.absorption_probability;
                println!(
                    "{:>5.2} | {:>16.4} | {:>16.4} {:>+9.4} | {:>16.4} {:>+9.4} | {:>16.4} {:>+9.4}",
                    pf,
                    surface.absorption_probability,
                    delta.absorption_probability, d_delta,
                    cls.absorption_probability, d_cls,
                    scls.absorption_probability, d_scls,
                );

                // Binomial standard errors + the Delta-vs-Surface z-score, so the
                // "within statistics" consistency claim is checkable from the run
                // itself rather than merely asserted in prose (workspace V&V rule:
                // methodology *and* measured results).
                let se_surface = binomial_se(surface.absorption_probability, surface.histories);
                let se_delta = binomial_se(delta.absorption_probability, delta.histories);
                let se_combined = (se_surface * se_surface + se_delta * se_delta).sqrt();
                let z_delta = d_delta / se_combined.max(f64::MIN_POSITIVE);
                println!(
                    "      |   (se {:.4}) |   (se {:.4})  z={:>+5.2} |",
                    se_surface, se_delta, z_delta
                );
            }
            Err(e) => println!("{pf:>5.2} | CLS/SCLS packing failed: {e}"),
        }
    }

    println!(
        "\nΔ columns are each method's absorption-probability gap vs Surface tracking (the exact"
    );
    println!(
        "reference). ΔDelta is the consistency check — Delta shares Surface's explicit geometry,"
    );
    println!(
        "so a gap there would mean a bug in the majorant/sampling, not different physics. ΔCLS"
    );
    println!(
        "and ΔSCLS are the stochastic-media models' approximation error against the same"
    );
    println!("reference — neither is claimed superior; see this file's module docs for the");
    println!("measured interpretation (SCLS overestimates more than CLS in this regime).\n");

    println!("Done. See the module docs for methodology, the delta-tracking majorant argument,");
    println!("and a previously-recorded measured table with binomial standard errors.");
}
