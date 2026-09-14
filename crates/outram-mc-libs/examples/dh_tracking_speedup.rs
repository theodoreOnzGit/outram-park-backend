//! **Double-heterogeneity tracking cost** — five ways to resolve the TRISO
//! double heterogeneity in the FHR ring-RPT pebble, timed against each other.
//!
//! ```text
//! cargo run --release -p outram-mc-libs --example dh_tracking_speedup
//! ```
//!
//! # The question
//!
//! A pebble holds O(10^4) TRISO particles. Resolving them explicitly is the
//! reference answer and the dominant cost of an FHR calculation, so every
//! practical method is a bargain: give up some geometric fidelity, buy back
//! time. This example prices each bargain on **one** problem, so the numbers
//! are comparable to each other rather than to the literature.
//!
//! Five arms, all walking the identical random walk with the identical seed
//! stream, differing only in how "which material is at this point, and how far
//! to the next event?" is answered:
//!
//! | Arm | Geometry stored | Flight sampling |
//! |---|---|---|
//! | **Surface tracking** (baseline) | all N spheres + grid | analytic ray-sphere to the nearest surface |
//! | **Delta tracking** | all N spheres + grid | majorant flight, rejection at the collision point |
//! | **CLS** | none | chord crossings resampled from closed-form statistics |
//! | **SCLS** | bounded local window | CLS plus retained inclusions |
//! | **Naive homogenisation** | none (smeared) | analytic flight in one smeared material |
//!
//! Naive homogenisation is the **upper limit**: the double heterogeneity is gone, so its
//! cost is the floor any DH treatment is trying to approach. Surface tracking
//! is the **baseline**: exact, and the thing being escaped.
//!
//! **There is deliberately no ring-RPT arm here.** Ring-RPT places the smeared
//! fuel in a spherical annulus at a fitted radius, which is a property of a
//! *pebble*; this benchmark walks a cube of the fuel zone and has no radial
//! structure for an annulus to sit in. Its cost would in any case be the naive
//! arm's — one extra radius comparison per query — so the ceiling above covers
//! it. Ring-RPT's eigenvalue, which is where it differs from the naive arm
//! entirely, is measured in `examples/dh_keff_vv.rs`.
//!
//! # Why absorption probability is printed next to every timing
//!
//! A speedup is meaningless without the answer it bought. CLS is memoryless by
//! construction, so it forgets that a back-scattered neutron is re-entering
//! ground it has already crossed; that bias is the entire reason SCLS exists.
//! Reading the two columns together is the point of the table — a method that
//! is 40x faster and 10% wrong has not helped.
//!
//! **This is a cost measurement, not a V&V case.** The absorption probabilities
//! are reported so the speedups can be read honestly, not as a validation of
//! any model. The exact arm (surface tracking) is the only one whose answer is
//! correct by construction.
//!
//! # The problem
//!
//! TRISO particles at the FHR pebble's OPyC radius, packed by RSA at the deck's
//! 30 % volume fraction into a cube sized to the r < 1.9 cm fuel zone. Matrix
//! is a pure isotropic scatterer, inclusions are pure absorbers, both with
//! finite total cross sections so that delta tracking is well posed. A history
//! is born uniformly, walks until it is absorbed in an inclusion, leaks from
//! the cube, or exceeds `MAX_COLLISIONS`.
//!
//! Mono-energetic and two-material by design: the question here is the cost of
//! *geometry*, so energy dependence and real nuclide data are deliberately
//! absent. They would add the same overhead to all five arms and hide the
//! effect being measured.
//!
//! # Results — measured 2026-09-14
//!
//! 200 000 histories per arm, 51 193 TRISO particles packed by RSA, release
//! build, single-threaded. **Six consecutive runs**; the speedups are quoted as
//! the observed range, because run-to-run spread is about +/-10 % and a single
//! figure would misrepresent the precision.
//!
//! | Arm | us/history | Speedup | P(absorb) | Bias vs exact |
//! |---|---|---|---|---|
//! | Surface tracking (exact) | 4.17-5.06 | 1.0x (baseline) | 0.8047 +/- 0.0009 | -- |
//! | Delta (Woodcock) tracking | 0.52-0.75 | **6.3-8.1x** | 0.8034 +/- 0.0009 | -1.0 sigma, NOT resolved |
//! | CLS (memoryless) | 0.21-0.23 | **19.4-23.2x** | 0.8062 +/- 0.0009 | +1.2 sigma, NOT resolved |
//! | SCLS (bounded window) | 0.43-0.47 | **9.5-11.0x** | 0.8128 +/- 0.0009 | +6.5 sigma, **resolved** |
//! | Naive homogenisation (no DH) | 0.07-0.08 | **53.6-68.7x** | 0.7475 +/- 0.0010 | -43.5 sigma, **resolved** |
//!
//! ## Machine the timings were taken on
//!
//! | | |
//! |---|---|
//! | CPU | Intel Xeon @ 2.10 GHz, **4 cores / 4 threads** (1 thread per core, no SMT) |
//! | Cache | L1d 192 KiB, L1i 128 KiB, L2 8 MiB (4x2 MiB), **L3 260 MiB shared** |
//! | ISA | x86-64 with AVX-512 (F/DQ/CD/BW/VL, VNNI, BF16), AMX, SHA-NI |
//! | RAM | **15.7 GiB** (16 461 028 kB), no swap |
//! | Virtualisation | KVM, full virtualisation — a cloud container, not bare metal |
//! | Kernel | Linux 6.18.44 |
//! | Toolchain | rustc 1.94.1 (e408947bf, 2026-03-25), `--release` |
//! | Threads | single-threaded — **one of the four cores in use**, 3 idle |
//!
//! **The L3 is 260 MiB, which flatters the exact arms.** The 51 193-sphere
//! packing and its lookup grid are a few megabytes at most, so on this machine
//! the explicit geometry is comfortably cache-resident and surface and delta
//! tracking are close to their best case. On a host with an ordinary few-MiB
//! L3 the two exact arms would suffer more cache pressure than the
//! geometry-free arms, so the speedups here should be read as a **lower**
//! bound on what a smaller-cache machine would show.
//!
//! It is also a virtualised, shared host, which is where the +/-10 % run-to-run
//! spread comes from. The sixth run (added 2026-09-14, after the arm-5 rename)
//! landed slightly below the bottom of four of the five previously quoted
//! ranges, and those ranges have been widened to include it rather than
//! presented as the tighter five-run spread. `P(absorb)` was **bit-identical**
//! across all six — the seed is fixed, so only the timings move, which is what
//! makes the ranges attributable to the machine rather than to the physics.
//!
//! Packing and grid construction are excluded from the timings and reported
//! separately (~0.5 s and ~0.2 s); they are one-off setup, not per-history cost.
//!
//! The P(absorb) column is **bit-identical across all five runs** — the seed is
//! fixed, so only the timings move. The speedup ranges are therefore pure
//! machine noise, not statistical scatter in the physics.
//!
//! ## Reading the table
//!
//! **Removing the double heterogeneity entirely is worth ~60-70x.** That is the
//! ceiling. Everything else is a fraction of it:
//!
//! - **The two exact arms agree.** Delta tracking differs from surface tracking
//!   by -1.0 sigma, i.e. not resolved from counting noise. Two independently
//!   implemented exact methods landing on the same answer is the main evidence
//!   that this harness measures what it claims to.
//! - **Delta tracking buys ~7-8x for free** — no bias, because it is exact.
//!   On this problem it captures roughly an eighth of the available ceiling.
//! - **CLS is the fastest approximation at ~20x**, and its bias is **not
//!   resolved** at 200 000 histories. See the caveat below before generalising.
//! - **Naive homogenisation costs -7.1 % in absorption.** That is the self-shielding thrown
//!   away when the particles are smeared: a neutron in the homogenised mixture
//!   sees the absorber everywhere at reduced density instead of concentrated in
//!   kernels it can miss entirely. Expected, large, and exactly the loss that
//!   ring-RPT exists to claw back by concentrating the smear into a fitted
//!   annulus instead of spreading it over the whole zone.
//!
//! ## Two findings worth not glossing over
//!
//! **SCLS is both slower AND more biased than CLS here** (9.5-11.0x vs
//! 19.4-23.2x; +6.5 sigma vs not-resolved). That runs against its design
//! intent: SCLS retains a bounded window of inclusions precisely so it can beat
//! memoryless CLS on accuracy, paying for it in time. It pays the time and does
//! not obviously collect. Whether that is the model, this implementation's
//! window, or this problem's regime is **not established here** and should not
//! be assumed. It is the kind of result worth chasing rather than filing.
//!
//! **This problem is absorption-dominated** (P(abs) ~ 0.80), and that is the
//! regime *least* hostile to CLS. `stochastic::benchmark`'s own module docs make
//! the point: memorylessness breaks under **back-scattering**, where a history
//! repeatedly re-crosses ground it has already covered. A history that is
//! absorbed on its first inclusion entry never gets the chance. So CLS's clean
//! showing above is a statement about this problem, not a general one, and a
//! lower-absorption variant would be the honest stress test.
//!
//! ## What this is not
//!
//! Not a V&V case and not a k-eff comparison. The CLS family has no eigenvalue
//! path in this crate, so a common k-eff benchmark across all five arms is not
//! currently possible; cost per history on a shared fixed-source walk is what
//! *is* comparable. Absolute microseconds are machine-specific — the speedup
//! ratios are the portable quantity.

use std::collections::HashMap;
use std::time::Instant;

use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::pebble_beds::fhr_pebble::TrisoSpec;
use outram_mc_libs::pebble_beds::sphere_packing::{
    PackedSpheres, PackingConfig, PackingMethod, Sphere,
};
use outram_mc_libs::stochastic::cls::ClsMedium;
use outram_mc_libs::stochastic::medium::{MaterialId, RsaMedium, StochasticMedium};
use outram_mc_libs::stochastic::scls::SclsMedium;

/// Inclusion (TRISO particle) material.
const INCLUSION: MaterialId = MaterialId(0);
/// Matrix (graphite) material.
const MATRIX: MaterialId = MaterialId(1);

/// Deck packing fraction for the FHR pebble fuel zone.
const PACKING_FRACTION: f64 = 0.30;
/// Half-width \[cm\] of the packing cube, sized to the r < 1.9 cm fuel zone.
const HALF_WIDTH: f64 = 1.9;
/// Matrix macroscopic total cross section \[cm^-1\] (pure scatter).
const SIGMA_MATRIX: f64 = 1.0;
/// Inclusion macroscopic total cross section \[cm^-1\] (pure absorption).
const SIGMA_INCLUSION: f64 = 4.0;
/// Histories per arm.
const HISTORIES: usize = 200_000;
/// Collisions after which a history is declared "survived".
const MAX_COLLISIONS: usize = 200;
/// Master seed. Every arm derives an independent stream from this.
const SEED: u64 = 0x5EED_1234_ABCD_0001;

// ─────────────────────────────────────────────────────────────── RNG

/// One step of the crate's 64-bit LCG, returning a uniform in [0, 1).
///
/// Local to this example so the timing measures geometry, not a shared RNG's
/// call overhead — every arm pays exactly this same cost.
#[inline]
fn rand_unit(seed: &mut u64) -> f64 {
    *seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
    ((*seed >> 11) as f64) / ((1u64 << 53) as f64)
}

/// An isotropic unit direction.
#[inline]
fn isotropic(seed: &mut u64) -> (f64, f64, f64) {
    let mu = 2.0 * rand_unit(seed) - 1.0;
    let phi = std::f64::consts::TAU * rand_unit(seed);
    let s = (1.0 - mu * mu).max(0.0).sqrt();
    (s * phi.cos(), s * phi.sin(), mu)
}

/// A uniform random point in the packing cube.
#[inline]
fn uniform_point(seed: &mut u64) -> Position {
    Position {
        x: (2.0 * rand_unit(seed) - 1.0) * HALF_WIDTH,
        y: (2.0 * rand_unit(seed) - 1.0) * HALF_WIDTH,
        z: (2.0 * rand_unit(seed) - 1.0) * HALF_WIDTH,
    }
}

#[inline]
fn outside_cube(p: Position) -> bool {
    p.x.abs() > HALF_WIDTH || p.y.abs() > HALF_WIDTH || p.z.abs() > HALF_WIDTH
}

// ──────────────────────────────────────────────── surface-tracking grid

/// A uniform bucket grid over the packing, for the surface-tracking arm.
///
/// [`PackedSpheres`] keeps its own grid private and exposes only point
/// membership, so the exact arm needs its own ray-vs-sphere acceleration. This
/// is deliberately the *same* cell size the packing uses, so the baseline is a
/// fair surface tracker rather than a straw man: a naive O(N) scan over 10^4
/// particles per flight would inflate every speedup below.
struct SurfaceGrid {
    cells: HashMap<(i64, i64, i64), Vec<u32>>,
    spheres: Vec<Sphere>,
    cell: f64,
}

impl SurfaceGrid {
    fn build(packing: &PackedSpheres) -> Self {
        let r = packing.radius();
        let cell = 2.0 * r;
        let mut cells: HashMap<(i64, i64, i64), Vec<u32>> = HashMap::new();
        let spheres: Vec<Sphere> = packing.spheres().to_vec();
        let key = |x: f64| (x / cell).floor() as i64;
        for (i, s) in spheres.iter().enumerate() {
            // Register into every cell the body touches.
            for dx in -1..=1 {
                for dy in -1..=1 {
                    for dz in -1..=1 {
                        let k = (
                            key(s.center.x) + dx,
                            key(s.center.y) + dy,
                            key(s.center.z) + dz,
                        );
                        cells.entry(k).or_default().push(i as u32);
                    }
                }
            }
        }
        for v in cells.values_mut() {
            v.sort_unstable();
            v.dedup();
        }
        Self { cells, spheres, cell }
    }

    /// Distance \[cm\] along `d` from `p` to the nearest sphere surface, if any
    /// is hit within `limit`.
    ///
    /// Marches the grid cell by cell (3-D DDA) and does the analytic
    /// ray-sphere intersection on each candidate. Returns the hit distance and
    /// whether the crossing enters (`true`) or leaves an inclusion.
    fn nearest_surface(&self, p: Position, d: (f64, f64, f64), limit: f64) -> Option<(f64, bool)> {
        let mut best: Option<(f64, bool)> = None;
        // Walk cells along the ray in steps of one cell length. A step this
        // size cannot skip a sphere, since every sphere is registered into its
        // neighbouring cells above.
        let steps = (limit / self.cell).ceil() as i64 + 1;
        let key = |x: f64| (x / self.cell).floor() as i64;
        let mut seen: Option<(i64, i64, i64)> = None;
        for n in 0..=steps {
            let t = (n as f64) * self.cell;
            if t > limit {
                break;
            }
            let k = (
                key(p.x + d.0 * t),
                key(p.y + d.1 * t),
                key(p.z + d.2 * t),
            );
            if seen == Some(k) {
                continue;
            }
            seen = Some(k);
            let Some(idxs) = self.cells.get(&k) else {
                continue;
            };
            for &i in idxs {
                let s = self.spheres[i as usize];
                let ox = p.x - s.center.x;
                let oy = p.y - s.center.y;
                let oz = p.z - s.center.z;
                let b = ox * d.0 + oy * d.1 + oz * d.2;
                let c = ox * ox + oy * oy + oz * oz - s.radius * s.radius;
                let disc = b * b - c;
                if disc <= 0.0 {
                    continue;
                }
                let sq = disc.sqrt();
                // Both roots; take the nearest strictly-positive one.
                for t_hit in [-b - sq, -b + sq] {
                    if t_hit > 1.0e-9 && t_hit <= limit {
                        let entering = c > 0.0 && t_hit == -b - sq;
                        if best.is_none_or(|(bd, _)| t_hit < bd) {
                            best = Some((t_hit, entering));
                        }
                    }
                }
            }
            if let Some((bd, _)) = best {
                // Everything further than the current best is in a later cell.
                if bd <= t {
                    break;
                }
            }
        }
        best
    }
}

// ─────────────────────────────────────────────────────────────── result

/// One arm's measured cost and answer.
struct ArmResult {
    name: &'static str,
    /// Wall-clock for [`HISTORIES`] histories \[s\].
    secs: f64,
    /// Fraction of histories absorbed in an inclusion.
    absorbed: f64,
    /// Whether this arm resolves the geometry exactly.
    exact: bool,
}

impl ArmResult {
    /// Binomial standard error on [`Self::absorbed`].
    ///
    /// Without this the bias column is unreadable: at these history counts a
    /// difference of a few 1e-3 can be pure counting noise.
    fn sem(&self) -> f64 {
        (self.absorbed * (1.0 - self.absorbed) / HISTORIES as f64).sqrt()
    }
}

/// Total cross section \[cm^-1\] of a material.
#[inline]
fn sigma_of(m: MaterialId) -> f64 {
    if m == INCLUSION {
        SIGMA_INCLUSION
    } else {
        SIGMA_MATRIX
    }
}

// ───────────────────────────────────────────── arm 1: surface tracking

/// **Exact reference.** Track to the nearer of (next surface, next collision),
/// crossing surfaces explicitly and resampling the flight in each new material.
///
/// This is what the other four arms are trying to avoid paying for.
fn run_surface(packing: &PackedSpheres, grid: &SurfaceGrid, mut seed: u64) -> ArmResult {
    let rsa = RsaMedium::new(packing.clone(), INCLUSION, MATRIX);
    let t0 = Instant::now();
    let mut absorbed = 0usize;
    for _ in 0..HISTORIES {
        let mut p = uniform_point(&mut seed);
        let mut mat = rsa.material_at(p);
        if mat == INCLUSION {
            absorbed += 1;
            continue;
        }
        let mut d = isotropic(&mut seed);
        let mut collisions = 0usize;
        loop {
            if collisions >= MAX_COLLISIONS || outside_cube(p) {
                break;
            }
            let flight = -rand_unit(&mut seed).ln() / sigma_of(mat);
            match grid.nearest_surface(p, d, flight) {
                // A surface comes first: move to it, swap material, resample.
                Some((t_hit, entering)) => {
                    p = Position {
                        x: p.x + d.0 * (t_hit + 1.0e-9),
                        y: p.y + d.1 * (t_hit + 1.0e-9),
                        z: p.z + d.2 * (t_hit + 1.0e-9),
                    };
                    mat = if entering { INCLUSION } else { MATRIX };
                }
                // Collision before any surface.
                None => {
                    p = Position {
                        x: p.x + d.0 * flight,
                        y: p.y + d.1 * flight,
                        z: p.z + d.2 * flight,
                    };
                    if outside_cube(p) {
                        break;
                    }
                    if mat == INCLUSION {
                        absorbed += 1;
                        break;
                    }
                    d = isotropic(&mut seed);
                    collisions += 1;
                }
            }
        }
    }
    ArmResult {
        name: "surface tracking (exact)",
        secs: t0.elapsed().as_secs_f64(),
        absorbed: absorbed as f64 / HISTORIES as f64,
        exact: true,
    }
}

// ─────────────────────────────────────────────── arm 2: delta tracking

/// **Woodcock delta tracking.** Sample the flight at the majorant, then accept
/// the collision with probability `sigma_t(local) / sigma_maj`.
///
/// No surface distance is ever computed — only point membership — which is
/// exactly the cost this method trades away.
fn run_delta(packing: &PackedSpheres, mut seed: u64) -> ArmResult {
    let rsa = RsaMedium::new(packing.clone(), INCLUSION, MATRIX);
    let sigma_maj = SIGMA_MATRIX.max(SIGMA_INCLUSION);
    let t0 = Instant::now();
    let mut absorbed = 0usize;
    for _ in 0..HISTORIES {
        let mut p = uniform_point(&mut seed);
        if rsa.material_at(p) == INCLUSION {
            absorbed += 1;
            continue;
        }
        let mut d = isotropic(&mut seed);
        let mut collisions = 0usize;
        while collisions < MAX_COLLISIONS {
            let flight = -rand_unit(&mut seed).ln() / sigma_maj;
            p = Position {
                x: p.x + d.0 * flight,
                y: p.y + d.1 * flight,
                z: p.z + d.2 * flight,
            };
            if outside_cube(p) {
                break;
            }
            let mat = rsa.material_at(p);
            // Rejection: a virtual collision leaves direction and energy alone.
            if rand_unit(&mut seed) * sigma_maj > sigma_of(mat) {
                continue;
            }
            if mat == INCLUSION {
                absorbed += 1;
                break;
            }
            d = isotropic(&mut seed);
            collisions += 1;
        }
    }
    ArmResult {
        name: "delta (Woodcock) tracking",
        secs: t0.elapsed().as_secs_f64(),
        absorbed: absorbed as f64 / HISTORIES as f64,
        exact: true,
    }
}

// ───────────────────────────────────────── arms 3 & 4: the CLS family

/// **Chord-length sampling family.** Identical transport to the delta arm, but
/// the medium is queried statistically instead of from stored geometry.
///
/// `CLS` stores nothing and resamples every crossing from closed-form chord
/// statistics; `SCLS` keeps a bounded window of inclusions it has already
/// seen. Both are approximations — their absorption column is the price.
fn run_cls_family(name: &'static str, mut medium: StochasticMedium, mut seed: u64) -> ArmResult {
    let sigma_maj = SIGMA_MATRIX.max(SIGMA_INCLUSION);
    let t0 = Instant::now();
    let mut absorbed = 0usize;
    let mut failed = 0usize;
    for _ in 0..HISTORIES {
        let mut p = uniform_point(&mut seed);
        medium.begin_flight();
        let Ok(m0) = medium.material_at(p, &mut seed) else {
            failed += 1;
            continue;
        };
        if m0 == INCLUSION {
            absorbed += 1;
            continue;
        }
        let mut d = isotropic(&mut seed);
        let mut collisions = 0usize;
        while collisions < MAX_COLLISIONS {
            let flight = -rand_unit(&mut seed).ln() / sigma_maj;
            p = Position {
                x: p.x + d.0 * flight,
                y: p.y + d.1 * flight,
                z: p.z + d.2 * flight,
            };
            if outside_cube(p) {
                break;
            }
            let Ok(mat) = medium.material_at(p, &mut seed) else {
                failed += 1;
                break;
            };
            if rand_unit(&mut seed) * sigma_maj > sigma_of(mat) {
                continue;
            }
            if mat == INCLUSION {
                absorbed += 1;
                break;
            }
            d = isotropic(&mut seed);
            medium.begin_flight();
            collisions += 1;
        }
    }
    assert_eq!(failed, 0, "{name}: medium returned NotImplemented — the arm is not measurable");
    ArmResult {
        name,
        secs: t0.elapsed().as_secs_f64(),
        absorbed: absorbed as f64 / HISTORIES as f64,
        exact: false,
    }
}

// ────────────────────────────────────────────────── arm 5: ring-RPT

/// **Naive homogenisation — the upper limit.** The double heterogeneity is
/// gone: one smeared material, one cross section, analytic flights.
///
/// Nothing here queries geometry at all, so this is the floor that any DH
/// treatment is working towards. It is not an approximation *of* tracking —
/// it is what is left when there is no longer anything to track.
fn run_homogenised(mut seed: u64) -> ArmResult {
    // Volume-weighted smearing, the same mixing rule `homogenise_by_volume`
    // applies to the real TRISO material.
    let sigma_h = PACKING_FRACTION * SIGMA_INCLUSION + (1.0 - PACKING_FRACTION) * SIGMA_MATRIX;
    let p_absorb = PACKING_FRACTION * SIGMA_INCLUSION / sigma_h;
    let t0 = Instant::now();
    let mut absorbed = 0usize;
    for _ in 0..HISTORIES {
        let mut p = uniform_point(&mut seed);
        let mut d = isotropic(&mut seed);
        let mut collisions = 0usize;
        while collisions < MAX_COLLISIONS {
            let flight = -rand_unit(&mut seed).ln() / sigma_h;
            p = Position {
                x: p.x + d.0 * flight,
                y: p.y + d.1 * flight,
                z: p.z + d.2 * flight,
            };
            if outside_cube(p) {
                break;
            }
            if rand_unit(&mut seed) < p_absorb {
                absorbed += 1;
                break;
            }
            d = isotropic(&mut seed);
            collisions += 1;
        }
    }
    ArmResult {
        name: "naive homogenisation (no DH)",
        secs: t0.elapsed().as_secs_f64(),
        absorbed: absorbed as f64 / HISTORIES as f64,
        exact: false,
    }
}

// ─────────────────────────────────────────────────────────────── main

fn main() {
    let spec = TrisoSpec::FHR_HALEU_UCO;
    let r_particle = spec.opyc;

    println!("Double-heterogeneity tracking cost — FHR ring-RPT pebble fuel zone");
    println!("==================================================================");
    println!("  TRISO outer (OPyC) radius : {r_particle} cm");
    println!("  packing fraction          : {PACKING_FRACTION}");
    println!("  cube half-width           : {HALF_WIDTH} cm");
    println!("  sigma_t  matrix/inclusion : {SIGMA_MATRIX} / {SIGMA_INCLUSION} cm^-1");
    println!("  histories per arm         : {HISTORIES}");

    let cfg = PackingConfig {
        particle_radius: r_particle,
        packing_fraction: PACKING_FRACTION,
        domain_half_width: HALF_WIDTH,
        method: PackingMethod::Rsa,
        seed: SEED,
    };
    let t_pack = Instant::now();
    let spheres = cfg.generate().expect("RSA packing failed");
    let n = spheres.len();
    let packing = PackedSpheres::from_spheres(spheres, HALF_WIDTH, r_particle);
    let pack_secs = t_pack.elapsed().as_secs_f64();
    println!("  particles packed          : {n} ({pack_secs:.2} s)");

    let t_grid = Instant::now();
    let grid = SurfaceGrid::build(&packing);
    println!("  surface grid built        : {:.2} s\n", t_grid.elapsed().as_secs_f64());

    let cls_medium = ClsMedium::new(r_particle, PACKING_FRACTION, INCLUSION, MATRIX);
    // Transport mfp for the SCLS Dynamic Inclusion Sphere: the matrix mfp.
    let transport_mfp = 1.0 / SIGMA_MATRIX;

    let results = vec![
        run_surface(&packing, &grid, SEED ^ 0x1111_1111_1111_1111),
        run_delta(&packing, SEED ^ 0x2222_2222_2222_2222),
        run_cls_family(
            "CLS (memoryless)",
            StochasticMedium::Cls(cls_medium.clone()),
            SEED ^ 0x3333_3333_3333_3333,
        ),
        run_cls_family(
            "SCLS (bounded window)",
            StochasticMedium::Scls(SclsMedium::new(cls_medium, Position::ZERO, transport_mfp)),
            SEED ^ 0x4444_4444_4444_4444,
        ),
        run_homogenised(SEED ^ 0x5555_5555_5555_5555),
    ];

    let baseline = results[0].secs;
    let base_abs = results[0].absorbed;

    println!("{:<32} {:>10} {:>12} {:>11} {:>10}", "arm", "time [s]", "us/history", "speedup", "P(abs)");
    println!("{}", "-".repeat(80));
    for r in &results {
        let us = r.secs / HISTORIES as f64 * 1.0e6;
        let speedup = baseline / r.secs;
        let mark = if r.exact { " " } else { "*" };
        println!(
            "{:<32} {:>10.3} {:>12.2} {:>10.1}x {:>9.4}{} +/-{:.4}",
            r.name, r.secs, us, speedup, r.absorbed, mark, r.sem()
        );
    }
    println!("{}", "-".repeat(80));
    println!("  * approximate geometry — read P(abs) against the exact arms before trusting the speedup.");
    println!();
    let base_sem = results[0].sem();
    println!("  Bias vs the exact (surface-tracked) answer, in units of the combined");
    println!("  binomial standard error. Under ~2 sigma is NOT resolved from counting noise:");
    for r in results.iter().skip(1) {
        let d = r.absorbed - base_abs;
        let sigma = (base_sem * base_sem + r.sem() * r.sem()).sqrt();
        let n_sigma = d / sigma;
        let verdict = if n_sigma.abs() < 2.0 { "NOT resolved" } else { "resolved" };
        println!(
            "    {:<32} {:+.4} +/- {:.4}  ({:+.1} sigma, {})",
            r.name, d, sigma, n_sigma, verdict
        );
    }
    println!();
    println!("  Naive homogenisation is the upper limit on speedup: {:.1}x. Any DH treatment lands between", baseline / results[4].secs);
    println!("  1.0x (surface tracking) and that.");
}
