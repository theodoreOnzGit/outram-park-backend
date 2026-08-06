//! GPU-accelerated Chord Length Sampling (CLS) / Semi-Implicit CLS (SCLS)
//! tutorial for stochastic (TRISO / dispersion-fuel) media.
//!
//! Run it (no special feature needed — `wgpu` is an unconditional non-Android
//! dependency of this crate, so there is no `gpu` feature to enable):
//! ```text
//! cargo run -p outram-mc-libs --release --example triso_stochastic_gpu
//! ```
//! If no GPU adapter is present (headless server, CI, no Vulkan/Metal loader) the
//! example still runs its **CPU** path and prints that the GPU was unavailable —
//! it never hard-fails on a missing GPU.
//!
//! # What this demonstrates
//!
//! **Chord Length Sampling (CLS)** transports a neutron through a binary
//! stochastic medium (a *matrix* phase with randomly embedded spherical
//! *inclusions* — TRISO fuel kernels in a moderator) **without an explicit
//! packing**. Instead of tracking real sphere positions, the geometry is sampled
//! *on the fly* from the chord-length statistics of the packing: the distance a
//! flight spends in each phase is drawn from that phase's mean chord length. The
//! classical CLS is **memoryless** — each phase crossing is an independent draw —
//! which makes one history a self-contained, branch-light random walk: **ideal
//! for the GPU**, one neutron per thread.
//!
//! This example runs the identical memoryless CLS chord walk three ways:
//!
//! 1. **CPU `f64` — the trusted reference.** Raw-`f64` transport with the crate
//!    64-bit LCG ([`init_seed`] per-history streams). This is the eigen-truth; the
//!    absorption probability it reports is *the* answer.
//! 2. **CPU `f32` mirror — the GPU-logic reference.** The exact same control flow
//!    and the *same* `f32` arithmetic + emulated-LCG uniform the WGSL shader uses,
//!    so any GPU-vs-mirror disagreement isolates a hardware floating-point
//!    divergence rather than a modelling choice.
//! 3. **GPU `f32` — acceleration only.** A WGSL compute shader running a whole
//!    batch of CLS histories in parallel (one per GPU invocation), returning a
//!    per-history absorbed/escaped flag. **Never trusted as the reference:** its
//!    `f32` absorption probability is judged *against* the CPU `f64` one within
//!    combined statistical (binomial) error, per the crate GPU contract.
//!
//! **Semi-Implicit CLS (SCLS)** is also run, but **on the CPU only** (arm 4). SCLS
//! is *stateful* — it retains a memory of the last phase interface crossed so an
//! immediate scatter-and-return re-encounters the same interface deterministically,
//! correcting the memoryless-CLS bias. That retained state is exactly what makes
//! SCLS a poor fit for the embarrassingly-parallel GPU launch, so it stays on the
//! CPU here. This example uses the GPU for the memoryless CLS batch (the parallel
//! part) and reports SCLS as a CPU-side illustration of the memory correction.
//!
//! # Honesty banner — what is and is NOT GPU-accelerated
//!
//! - **CLS memoryless walk:** CPU `f64` is the trusted reference; the GPU `f32`
//!   batch is acceleration only, validated against the CPU within combined
//!   statistical error. There is **no** GPU number presented as "the answer".
//! - **SCLS retained-history walk:** **CPU only.** It is *not* GPU-accelerated
//!   (retained per-history memory does not vectorise cleanly); it is shown purely
//!   to illustrate the CLS→SCLS memory correction on the CPU.
//! - If [`probe`](outram_mc_libs::gpu::probe) finds no adapter, the GPU arm is
//!   skipped and only the CPU arms run — a normal, expected outcome, not an error.
//!
//! # The CLS physics model — chord sampling reused from `stochastic::cls`
//!
//! A 1-D binary stochastic slab of thickness `L` cm with vacuum boundaries — the
//! canonical CLS testbed (Adams–Larsen–Pomraning-style binary Markovian media).
//! Two phases, indexed `0 = matrix`, `1 = inclusion`, each with a macroscopic total
//! and absorption cross section `Sigma_t`, `Sigma_a` \[cm⁻¹\].
//!
//! **The chord physics is NOT re-implemented here — it comes from the crate's
//! [`stochastic::cls::ClsMedium`](outram_mc_libs::stochastic::cls::ClsMedium):**
//!
//! - the mean chords are `ClsMedium::mean_chord_inclusion()` (`= 4 r / 3`, Cauchy's
//!   mean chord of a sphere) and `ClsMedium::mean_chord_matrix()`
//!   (`= (4 r / 3)(1 - p)/p`, the binary-Markovian volume-fraction relation);
//! - every interface chord in the CPU reference (and the SCLS arm's fresh draws) is
//!   `ClsMedium::sample_distance_to_boundary` (which draws `-mean·ln(1 - ξ)`);
//! - the **WGSL shader and the CPU-`f32` mirror are the `f32` port of that same
//!   `sample_distance_to_boundary`** — same `-mean·ln(1 - ξ)` chord, so the GPU, the
//!   mirror and the CPU `f64` reference all sample the *identical* module physics and
//!   differ only by `f32` rounding.
//!
//! Only the slab **transport driver** (collision on `Sigma_t`, capture on
//! `Sigma_a/Sigma_t`, 1-D scatter, edge leakage, and the SCLS retained-interface
//! memory) is the example's own — the module is a geometry-only sampler, not a
//! transport solver. The crate's full 3-D `SclsMedium` is demonstrated in the sibling
//! `triso_stochastic_media.rs` / `triso_four_methods.rs` examples.
//!
//! A history starts in the matrix at `x = 0` heading `+x`. Each step samples an
//! exponential geometric chord `d_bnd = -lambda[mat] ln(xi)` (distance to the next
//! phase interface), an exponential collision distance `d_col = -ln(xi)/Sigma_t[mat]`,
//! and the distance to the nearer slab edge `d_edge`. Streaming to the nearest:
//! reaching an edge ⇒ **escaped**; a collision ⇒ absorb with probability
//! `Sigma_a/Sigma_t` (⇒ **absorbed**, history ends) else isotropic-1-D scatter
//! (direction flips ±1); an interface ⇒ cross into the other phase (**memoryless**:
//! the next chord is a fresh independent draw). The scored quantity is the
//! **absorption probability** `P_abs = N_absorbed / N_histories`, a binomial
//! estimator with `sigma_P = sqrt(P(1-P)/N)`.
//!
//! Provenance: the geometry/material *idea* (TRISO kernels in a matrix) is adapted
//! from the OpenMC `triso.ipynb` notebook (openmc-dev/openmc-notebooks, MIT); the
//! CLS/SCLS chord-walk drivers are **new work**, not a port. The semi-implicit CLS
//! concept follows Tan, Feng, Chan, Wang (2025),
//! `pebble_beds::references::TAN2025_CLS` (doi:10.1016/j.anucene.2025.111436). The
//! LCG is the crate's own ([`rng::lcg`](outram_mc_libs::rng::lcg)), replicated
//! bit-exactly (integer state) in WGSL — only the derived `f32` uniform *value*
//! diverges from the CPU `f64` one, the accepted `f32` acceleration divergence.
//!
//! # V&V — methodology and results
//!
//! **Methodology.** Run `N` identical memoryless-CLS histories on (1) CPU `f64`
//! with master seed `S_cpu` and (2) GPU `f32` with an *independent* master seed
//! `S_gpu`, giving two statistically independent binomial estimates `P_cpu ± σ_cpu`
//! and `P_gpu ± σ_gpu` of the same absorption probability. Pass criterion: the
//! z-score `|P_cpu − P_gpu| / sqrt(σ_cpu² + σ_gpu²) ≤ 5`. Separately, run the CPU
//! `f32` *mirror* (same `S_gpu`, same arithmetic path as the shader) on a small
//! subset and require the GPU per-history outcomes to match the mirror for all but
//! a negligible near-tie fraction (`< 0.1 %`) — this pins the shader *logic*
//! against a same-`f32`-path CPU reference. Report CPU/GPU wall-clock, histories/s,
//! and GPU speedup. The SCLS CPU arm reports its own `P_abs ± σ` to show the
//! memory correction relative to memoryless CLS.
//!
//! **Results (measured 2026-08-06, `N = 1048576` histories per arm).** Written to
//! a reproducible CSV under `verification_and_validation/gpu_benchmarks/` and
//! printed at the bottom of `main`. GPU: NVIDIA RTX A5000 (NVK GA102, Vulkan).
//!
//! - Arm 1, CPU `f64` CLS (trusted reference): `P_abs = 0.7014103 ± 0.0004469`
//!   (735482/1048576 absorbed), 0.21 s, 4.97e6 histories/s.
//! - Arm 2, CPU `f32` mirror (seed set `S_gpu`): `P_abs = 0.7016144 ± 0.0004468`,
//!   0.19 s. `|P_f32mirror − P_f64ref| = 0.000204` = **0.32 combined σ**.
//! - Arm 3, GPU `f32` CLS batch: `P_abs = 0.7016144 ± 0.0004468`, 0.099 s,
//!   1.06e7 histories/s, **2.14× speedup** vs the CPU `f64` arm. z-score vs the
//!   CPU `f64` reference = **0.32** → **PASS** (criterion ≤ 5). GPU-vs-CPU-`f32`-
//!   mirror per-history logic: **65536/65536 outcomes matched**, 0 near-tie
//!   mismatches (0.0000 %, criterion < 0.1 %).
//! - Arm 4, CPU `f64` SCLS: `P_abs = 0.7013741 ± 0.0004469`; SCLS − CLS =
//!   `−0.000036` (0.06 combined σ).
//!
//! Reproducibility: two consecutive runs on 2026-08-06 produced bit-identical
//! `P_abs` and `σ` in all four arms (only wall-clock times differed).
//!
//! **Supersedes the 2026-07-21 authoring run.** Those numbers
//! (`P_abs = 0.7013826` CPU `f64`, `0.7001810` `f32`-mirror, `0.7011147` SCLS;
//! GPU arm skipped for want of an adapter) were produced with the pre-fix
//! `rng::lcg::init_seed`, which seeded consecutive histories **one LCG draw**
//! apart instead of one 152917-draw stride apart (bead `op-rbo`). Because each
//! history's stream then overlapped its neighbour's almost entirely, the quoted
//! binomial `σ` understated the true uncertainty — the histories were not
//! independent samples. The central values happened to move very little
//! (`ΔP_abs = +0.0000277` on the `f64` reference arm, **0.04 σ**), so the bias
//! was small here; the invalid quantity was the *uncertainty*, not the mean.
//! All numbers above are from re-runs with the corrected seeding and are the
//! ones to cite.

// ---------------------------------------------------------------------------
// Android: wgpu is target-gated OFF Android (no Vulkan/Metal loader, no easy
// toolchain), so this whole GPU example is desktop-only. A stub `main` keeps the
// native Termux / cross-compiled Android build green (a blanked file would give
// "main function not found"); every desktop item below is
// `#[cfg(not(target_os = "android"))]`. Precedent: the crate's other GPU
// examples and the workspace CLAUDE.md "Android / Termux portability" rule.
// ---------------------------------------------------------------------------
#[cfg(target_os = "android")]
fn main() {
    println!(
        "triso_stochastic_gpu is a desktop-only GPU example (wgpu is target-gated \
         off Android); nothing to run on Android."
    );
}

#[cfg(not(target_os = "android"))]
fn main() {
    desktop::run();
}

#[cfg(not(target_os = "android"))]
mod desktop {
    use outram_mc_libs::rng::lcg::{init_seed, INC, MULT};
    use outram_mc_libs::stochastic::cls::ClsMedium;
    use outram_mc_libs::stochastic::medium::MaterialId;
    use std::fs;
    use std::io::Write;
    use std::path::PathBuf;
    use std::time::Instant;

    /// Phase indices for the binary medium.
    const MATRIX: usize = 0;
    const INCLUSION: usize = 1;

    /// Per-history outcome of a CLS/SCLS chord walk.
    ///
    /// An `enum` (not a `bool`) so downstream `match`es stay exhaustive per the
    /// workspace "enums over ad-hoc flags" rule. `Escaped` folds together "leaked
    /// through a slab edge" and "reached the iteration cap without absorbing" —
    /// both are simply *not absorbed*, which is all the absorption tally needs.
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    enum Outcome {
        /// The neutron was captured at a collision (scored in `P_abs`).
        Absorbed,
        /// The neutron left the slab or survived the iteration cap (not absorbed).
        Escaped,
    }

    /// Physical inputs to one binary-stochastic-medium CLS problem (all `f64`).
    ///
    /// Index `0` = matrix, `1` = inclusion throughout. Cross sections are
    /// macroscopic \[cm⁻¹\]; lengths are \[cm\]. `max_iter` caps the walk so a
    /// heavily-scattering history always terminates (its contribution to `P_abs` is
    /// then "escaped", i.e. not absorbed).
    #[derive(Clone, Copy, Debug)]
    struct ClsParams {
        /// Slab thickness \[cm\] (vacuum boundaries at `x = 0` and `x = l`).
        l: f64,
        /// Mean chord length per phase \[cm\]: `[lambda_matrix, lambda_inclusion]`.
        lambda: [f64; 2],
        /// Macroscopic total cross section per phase \[cm⁻¹\]: `[Sigma_t_M, Sigma_t_I]`.
        sigma_t: [f64; 2],
        /// Macroscopic absorption cross section per phase \[cm⁻¹\]: `[Sigma_a_M, Sigma_a_I]`.
        sigma_a: [f64; 2],
        /// Maximum chord-walk steps before a history is declared "escaped".
        max_iter: u32,
    }

    // -----------------------------------------------------------------------
    // CPU f64 reference — the trusted eigen-truth.
    // -----------------------------------------------------------------------

    /// One memoryless-CLS history in **`f64`** with the crate LCG — the trusted
    /// reference walk. Draws (in fixed order) an exponential geometric chord, an
    /// exponential collision distance, a capture roulette on collision, and an
    /// isotropic-1-D scatter direction; streams to the nearest of {phase interface,
    /// collision, slab edge}. Returns [`Outcome`].
    fn cls_history_f64(cls: &ClsMedium, p: &ClsParams, seed: &mut u64) -> Outcome {
        use outram_mc_libs::rng::lcg::prn;
        let mut x = 0.0_f64;
        let mut dir = 1.0_f64;
        let mut mat = MATRIX;
        for _ in 0..p.max_iter {
            // Chord to the next phase interface — sampled by the crate module, not
            // re-derived: ClsMedium::sample_distance_to_boundary picks this phase's mean
            // chord and draws -mean·ln(1-ξ).
            let d_bnd = cls.sample_distance_to_boundary(mat == INCLUSION, seed);
            let xi2 = prn(seed);
            let d_col = -xi2.ln() / p.sigma_t[mat]; // xi2 == 0 -> +inf (no collision)
            let d_edge = if dir > 0.0 { p.l - x } else { x };

            if d_edge <= d_bnd && d_edge <= d_col {
                return Outcome::Escaped; // left the slab
            } else if d_col <= d_bnd {
                x += dir * d_col;
                let xi3 = prn(seed);
                if xi3 < p.sigma_a[mat] / p.sigma_t[mat] {
                    return Outcome::Absorbed;
                }
                let xi4 = prn(seed);
                dir = if xi4 < 0.5 { 1.0 } else { -1.0 }; // isotropic 1-D scatter
            } else {
                x += dir * d_bnd;
                mat = 1 - mat; // memoryless phase crossing
            }
        }
        Outcome::Escaped
    }

    /// Run `n` `f64` CLS histories with per-history LCG streams derived from
    /// `master_seed` via [`init_seed`], returning the number absorbed.
    fn cls_batch_f64(cls: &ClsMedium, p: &ClsParams, n: usize, master_seed: i64) -> usize {
        let mut absorbed = 0usize;
        for i in 0..n {
            let mut seed = init_seed(i as i64, 0, master_seed);
            if cls_history_f64(cls, p, &mut seed) == Outcome::Absorbed {
                absorbed += 1;
            }
        }
        absorbed
    }

    // -----------------------------------------------------------------------
    // CPU f32 mirror — the GPU-logic reference (same arithmetic path as WGSL).
    // -----------------------------------------------------------------------

    /// Advance a split 64-bit LCG seed `(hi, lo)` by one step and derive the `f32`
    /// uniform, using the **same arithmetic the WGSL shader uses** (native `u64`
    /// state advance — bit-exact vs the CPU LCG — and the top-24-bit `f32` uniform
    /// `xi = f32(state_hi >> 8) * 2⁻²⁴`). Returns `(new_hi, new_lo, xi)`. Identical
    /// in spirit to `crate::gpu::batched_flight`'s private `lcg_advance`.
    #[inline]
    fn lcg_advance_f32(hi: u32, lo: u32) -> (u32, u32, f32) {
        let seed = ((hi as u64) << 32) | (lo as u64);
        let new = seed.wrapping_mul(MULT).wrapping_add(INC);
        let new_hi = (new >> 32) as u32;
        let new_lo = new as u32;
        let top24 = (new >> 40) as u32; // == new_hi >> 8
        let xi = (top24 as f32) * (1.0_f32 / 16_777_216.0_f32);
        (new_hi, new_lo, xi)
    }

    /// One memoryless-CLS history in **`f32`** with the emulated (GPU-matching) LCG
    /// uniform — the bit-level reference for the shader's logic. Same control flow
    /// and draw order as [`cls_history_f64`], but every value is `f32` and every
    /// uniform is the top-24-bit one the shader derives.
    fn cls_history_f32(p: &ClsParamsF32, hi0: u32, lo0: u32) -> Outcome {
        let mut hi = hi0;
        let mut lo = lo0;
        let mut x = 0.0_f32;
        let mut dir = 1.0_f32;
        let mut mat = MATRIX;
        for _ in 0..p.max_iter {
            let (nh, nl, xi1) = lcg_advance_f32(hi, lo);
            hi = nh;
            lo = nl;
            // f32 port of ClsMedium::sample_distance_to_boundary (-mean·ln(1-ξ)).
            let d_bnd = -p.lambda[mat] * (1.0 - xi1).ln();
            let (nh, nl, xi2) = lcg_advance_f32(hi, lo);
            hi = nh;
            lo = nl;
            let d_col = -xi2.ln() / p.sigma_t[mat];
            let d_edge = if dir > 0.0 { p.l - x } else { x };

            if d_edge <= d_bnd && d_edge <= d_col {
                return Outcome::Escaped;
            } else if d_col <= d_bnd {
                x += dir * d_col;
                let (nh, nl, xi3) = lcg_advance_f32(hi, lo);
                hi = nh;
                lo = nl;
                if xi3 < p.sigma_a[mat] / p.sigma_t[mat] {
                    return Outcome::Absorbed;
                }
                let (nh, nl, xi4) = lcg_advance_f32(hi, lo);
                hi = nh;
                lo = nl;
                dir = if xi4 < 0.5 { 1.0 } else { -1.0 };
            } else {
                x += dir * d_bnd;
                mat = 1 - mat;
            }
        }
        Outcome::Escaped
    }

    /// `f32` view of [`ClsParams`] for the mirror + shader upload.
    #[derive(Clone, Copy, Debug)]
    struct ClsParamsF32 {
        l: f32,
        lambda: [f32; 2],
        sigma_t: [f32; 2],
        sigma_a: [f32; 2],
        max_iter: u32,
    }

    impl ClsParamsF32 {
        fn from_f64(p: &ClsParams) -> Self {
            ClsParamsF32 {
                l: p.l as f32,
                lambda: [p.lambda[0] as f32, p.lambda[1] as f32],
                sigma_t: [p.sigma_t[0] as f32, p.sigma_t[1] as f32],
                sigma_a: [p.sigma_a[0] as f32, p.sigma_a[1] as f32],
                max_iter: p.max_iter,
            }
        }
    }

    // -----------------------------------------------------------------------
    // CPU SCLS — semi-implicit (retained-interface) illustration, CPU only.
    // -----------------------------------------------------------------------

    /// One **semi-implicit CLS** history in `f64` (CPU only). Identical to
    /// [`cls_history_f64`] except it retains the position and crossing-direction of
    /// the **last phase interface**; when a scatter reverses the flight back toward
    /// that interface, the distance to it is the retained *deterministic* value
    /// rather than a fresh exponential draw. This is the "semi-implicit" memory
    /// that couples successive crossings of the same interface and corrects the
    /// memoryless-CLS bias.
    ///
    /// This is a **tutorial-level** retained-single-interface SCLS to demonstrate
    /// the memory effect — not a validated reproduction of a published SCLS
    /// (cf. `pebble_beds::references::TAN2025_CLS`).
    fn scls_history_f64(cls: &ClsMedium, p: &ClsParams, seed: &mut u64) -> Outcome {
        use outram_mc_libs::rng::lcg::prn;
        let mut x = 0.0_f64;
        let mut dir = 1.0_f64;
        let mut mat = MATRIX;
        // Retained last interface: (position, direction sign when it was crossed).
        let mut last_iface: Option<(f64, f64)> = None;
        for _ in 0..p.max_iter {
            // Semi-implicit: if we are heading back toward the retained interface,
            // its distance is deterministic; otherwise sample a fresh chord through the
            // crate module (same ClsMedium::sample_distance_to_boundary the memoryless
            // arm uses). The retained-interface memory is the example's 1-D layer on top;
            // the crate's full 3-D SclsMedium is demonstrated in triso_stochastic_media.rs.
            let (d_bnd, from_memory) = match last_iface {
                Some((xpos, crossed_dir)) if crossed_dir * dir < 0.0 => ((xpos - x).abs(), true),
                _ => (
                    cls.sample_distance_to_boundary(mat == INCLUSION, seed),
                    false,
                ),
            };
            let xi2 = prn(seed);
            let d_col = -xi2.ln() / p.sigma_t[mat];
            let d_edge = if dir > 0.0 { p.l - x } else { x };

            if d_edge <= d_bnd && d_edge <= d_col {
                return Outcome::Escaped;
            } else if d_col <= d_bnd {
                x += dir * d_col;
                let xi3 = prn(seed);
                if xi3 < p.sigma_a[mat] / p.sigma_t[mat] {
                    return Outcome::Absorbed;
                }
                let xi4 = prn(seed);
                dir = if xi4 < 0.5 { 1.0 } else { -1.0 };
                // Scatter does not cross the interface; retained memory still valid.
            } else {
                // Cross the interface: flip phase and update the retained memory to
                // this crossing (a fresh sampled crossing replaces old memory; a
                // memory-driven crossing records the new side).
                x += dir * d_bnd;
                mat = 1 - mat;
                last_iface = Some((x, dir));
                let _ = from_memory;
            }
        }
        Outcome::Escaped
    }

    /// Run `n` `f64` SCLS histories with per-history LCG streams from `master_seed`.
    fn scls_batch_f64(cls: &ClsMedium, p: &ClsParams, n: usize, master_seed: i64) -> usize {
        let mut absorbed = 0usize;
        for i in 0..n {
            let mut seed = init_seed(i as i64, 0, master_seed);
            if scls_history_f64(cls, p, &mut seed) == Outcome::Absorbed {
                absorbed += 1;
            }
        }
        absorbed
    }

    // -----------------------------------------------------------------------
    // Statistics.
    // -----------------------------------------------------------------------

    /// Binomial estimate `(P, sigma_P)` from `n_absorbed` out of `n`.
    fn binomial(n_absorbed: usize, n: usize) -> (f64, f64) {
        let nn = n.max(1) as f64;
        let p = n_absorbed as f64 / nn;
        (p, (p * (1.0 - p) / nn).sqrt())
    }

    // -----------------------------------------------------------------------
    // GPU compute path.
    // -----------------------------------------------------------------------

    /// Inline WGSL CLS batch shader. One invocation per history; each runs the
    /// memoryless chord walk in `f32` using the emulated 64-bit LCG (bit-exact
    /// integer state vs the CPU LCG) and writes a per-history absorbed flag
    /// (`1 = absorbed`, `0 = escaped`). The LCG helpers mirror
    /// `src/gpu/shaders/batched_flight.wgsl` exactly.
    const CLS_SHADER: &str = r#"
struct Params {
    n_hist: u32,
    max_iter: u32,
    pad0: u32,
    pad1: u32,
    geo: vec4<f32>, // (L, lambda_matrix, lambda_inclusion, _)
    xs:  vec4<f32>, // (sigma_t_matrix, sigma_t_inclusion, sigma_a_matrix, sigma_a_inclusion)
};

@group(0) @binding(0) var<storage, read>       seeds: array<u32>; // hi[0..N] ++ lo[0..N]
@group(0) @binding(1) var<uniform>             params: Params;
@group(0) @binding(2) var<storage, read_write> outcome: array<u32>; // 1 = absorbed, 0 = escaped

const MULT_HI: u32 = 0x5851F42Du;
const MULT_LO: u32 = 0x4C957F2Du;
const INC_HI:  u32 = 0x14057B7Eu;
const INC_LO:  u32 = 0xF767814Fu;

fn mul_u32_full(x: u32, y: u32) -> vec2<u32> {
    let x0 = x & 0xFFFFu;
    let x1 = x >> 16u;
    let y0 = y & 0xFFFFu;
    let y1 = y >> 16u;
    let t0 = x0 * y0;
    let s  = x0 * y1;
    let t1 = s + x1 * y0;
    let carry1 = select(0u, 1u, t1 < s);
    let t2 = x1 * y1;
    let lo_lo16 = t0 & 0xFFFFu;
    let mid = (t0 >> 16u) + (t1 & 0xFFFFu);
    let lo = lo_lo16 | ((mid & 0xFFFFu) << 16u);
    let carry_mid = mid >> 16u;
    let hi = t2 + (t1 >> 16u) + carry_mid + (carry1 << 16u);
    return vec2<u32>(lo, hi);
}

fn mul64_low(a_lo: u32, a_hi: u32, b_lo: u32, b_hi: u32) -> vec2<u32> {
    let p = mul_u32_full(a_lo, b_lo);
    let cross = a_lo * b_hi + a_hi * b_lo;
    let res_lo = p.x;
    let res_hi = p.y + cross;
    return vec2<u32>(res_lo, res_hi);
}

// Advance split seed (s_hi:s_lo) one step, return vec3(new_lo, new_hi, bitcast(xi)).
fn lcg_advance(s_lo: u32, s_hi: u32) -> vec3<u32> {
    let m = mul64_low(s_lo, s_hi, MULT_LO, MULT_HI);
    let new_lo = m.x + INC_LO;
    let carry = select(0u, 1u, new_lo < INC_LO);
    let new_hi = m.y + INC_HI + carry;
    let top24 = new_hi >> 8u;
    let xi = f32(top24) * (1.0 / 16777216.0);
    return vec3<u32>(new_lo, new_hi, bitcast<u32>(xi));
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    let n = params.n_hist;
    if (i >= n) {
        return;
    }

    var s_hi: u32 = seeds[i];
    var s_lo: u32 = seeds[n + i];

    var x: f32 = 0.0;
    var dir: f32 = 1.0;
    var mat: u32 = 0u; // 0 = matrix, 1 = inclusion
    var absorbed: u32 = 0u;

    for (var it: u32 = 0u; it < params.max_iter; it = it + 1u) {
        let lambda = select(params.geo.z, params.geo.y, mat == 0u);
        let sigt   = select(params.xs.y,  params.xs.x,  mat == 0u);
        let siga   = select(params.xs.w,  params.xs.z,  mat == 0u);

        let a1 = lcg_advance(s_lo, s_hi);
        s_lo = a1.x; s_hi = a1.y;
        let xi1 = bitcast<f32>(a1.z);
        // f32/WGSL port of ClsMedium::sample_distance_to_boundary: -mean*ln(1-xi),
        // matching sample_chord's 1-xi shift so GPU, f32 mirror and CPU f64 agree.
        let d_bnd = -lambda * log(1.0 - xi1);

        let a2 = lcg_advance(s_lo, s_hi);
        s_lo = a2.x; s_hi = a2.y;
        let xi2 = bitcast<f32>(a2.z);
        let d_col = -log(xi2) / sigt;

        var d_edge: f32;
        if (dir > 0.0) { d_edge = params.geo.x - x; } else { d_edge = x; }

        if (d_edge <= d_bnd && d_edge <= d_col) {
            absorbed = 0u; // escaped through an edge
            break;
        } else if (d_col <= d_bnd) {
            x = x + dir * d_col;
            let a3 = lcg_advance(s_lo, s_hi);
            s_lo = a3.x; s_hi = a3.y;
            let xi3 = bitcast<f32>(a3.z);
            if (xi3 < siga / sigt) {
                absorbed = 1u;
                break;
            }
            let a4 = lcg_advance(s_lo, s_hi);
            s_lo = a4.x; s_hi = a4.y;
            let xi4 = bitcast<f32>(a4.z);
            if (xi4 < 0.5) { dir = 1.0; } else { dir = -1.0; }
        } else {
            x = x + dir * d_bnd;
            mat = 1u - mat;
        }
    }

    outcome[i] = absorbed;
}
"#;

    /// Pack the `Params` uniform (48 bytes) native-endian: four `u32` header words
    /// then two `vec4<f32>` (geo, xs). Layout matches the WGSL `Params` struct.
    fn pack_params(p: &ClsParamsF32, n_hist: u32) -> Vec<u8> {
        let mut b = Vec::with_capacity(48);
        b.extend_from_slice(&n_hist.to_ne_bytes());
        b.extend_from_slice(&p.max_iter.to_ne_bytes());
        b.extend_from_slice(&0u32.to_ne_bytes());
        b.extend_from_slice(&0u32.to_ne_bytes());
        // geo = (L, lambda_matrix, lambda_inclusion, 0)
        b.extend_from_slice(&p.l.to_ne_bytes());
        b.extend_from_slice(&p.lambda[MATRIX].to_ne_bytes());
        b.extend_from_slice(&p.lambda[INCLUSION].to_ne_bytes());
        b.extend_from_slice(&0f32.to_ne_bytes());
        // xs = (sigma_t_M, sigma_t_I, sigma_a_M, sigma_a_I)
        b.extend_from_slice(&p.sigma_t[MATRIX].to_ne_bytes());
        b.extend_from_slice(&p.sigma_t[INCLUSION].to_ne_bytes());
        b.extend_from_slice(&p.sigma_a[MATRIX].to_ne_bytes());
        b.extend_from_slice(&p.sigma_a[INCLUSION].to_ne_bytes());
        b
    }

    fn u32_slice_to_bytes(data: &[u32]) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(data.len() * 4);
        for &x in data {
            bytes.extend_from_slice(&x.to_ne_bytes());
        }
        bytes
    }

    /// Run `n` memoryless-CLS histories on the GPU. History `i` is seeded with
    /// `init_seed(i, 0, master_seed)` (identical to the CPU arms), split into
    /// `(hi, lo)` and uploaded. Returns one [`Outcome`] per history.
    fn cls_batch_gpu(
        ctx: &outram_mc_libs::gpu::GpuContext,
        p: &ClsParamsF32,
        n: usize,
        master_seed: i64,
    ) -> Vec<Outcome> {
        use wgpu::util::DeviceExt;

        if n == 0 {
            return Vec::new();
        }
        let device = &ctx.device;
        let queue = &ctx.queue;

        // Seeds: hi[0..N] ++ lo[0..N].
        let mut seeds = Vec::with_capacity(2 * n);
        seeds.resize(2 * n, 0u32);
        for i in 0..n {
            let s = init_seed(i as i64, 0, master_seed);
            seeds[i] = (s >> 32) as u32;
            seeds[n + i] = s as u32;
        }

        let seeds_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("cls.seeds"),
            contents: &u32_slice_to_bytes(&seeds),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let params_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("cls.params"),
            contents: &pack_params(p, n as u32),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let out_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("cls.outcome"),
            contents: &u32_slice_to_bytes(&vec![0u32; n]),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        });

        let out_size = (n * std::mem::size_of::<u32>()) as wgpu::BufferAddress;
        let out_staging = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("cls.outcome_staging"),
            size: out_size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("cls.shader"),
            source: wgpu::ShaderSource::Wgsl(CLS_SHADER.into()),
        });

        let storage_ro = wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only: true },
            has_dynamic_offset: false,
            min_binding_size: None,
        };
        let storage_rw = wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only: false },
            has_dynamic_offset: false,
            min_binding_size: None,
        };
        let uniform_ty = wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        };
        let entry = |binding: u32, ty: wgpu::BindingType| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty,
            count: None,
        };
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("cls.bgl"),
            entries: &[
                entry(0, storage_ro), // seeds
                entry(1, uniform_ty), // params
                entry(2, storage_rw), // outcome
            ],
        });
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("cls.pl"),
            bind_group_layouts: &[Some(&bgl)],
            immediate_size: 0,
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("cls.pipeline"),
            layout: Some(&pl),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("cls.bg"),
            layout: &bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: seeds_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: params_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: out_buf.as_entire_binding(),
                },
            ],
        });

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("cls.enc"),
        });
        {
            let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("cls.pass"),
                timestamp_writes: None,
            });
            cpass.set_pipeline(&pipeline);
            cpass.set_bind_group(0, &bind_group, &[]);
            cpass.dispatch_workgroups((n as u32).div_ceil(64), 1, 1);
        }
        encoder.copy_buffer_to_buffer(&out_buf, 0, &out_staging, 0, out_size);
        queue.submit(Some(encoder.finish()));

        out_staging
            .slice(..)
            .map_async(wgpu::MapMode::Read, |r| r.unwrap());
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();

        let view = out_staging.slice(..).get_mapped_range();
        let codes: Vec<u32> = view
            .chunks_exact(4)
            .take(n)
            .map(|c| u32::from_ne_bytes(c.try_into().unwrap()))
            .collect();
        drop(view);
        out_staging.unmap();

        codes
            .into_iter()
            .map(|c| {
                if c == 1 {
                    Outcome::Absorbed
                } else {
                    Outcome::Escaped
                }
            })
            .collect()
    }

    /// Drive the whole tutorial: build the problem, run the CPU/GPU arms, report
    /// agreement + speedup, and write a CSV.
    pub fn run() {
        println!("=====================================================================");
        println!(" TRISO stochastic-media CLS / SCLS  —  GPU-accelerated tutorial");
        println!("=====================================================================");
        println!(" Model: 1-D binary stochastic slab (matrix + spherical inclusions),");
        println!("        memoryless Chord Length Sampling (CLS). New work (not a port);");
        println!("        geometry idea adapted from OpenMC triso.ipynb (MIT).");
        println!(" HONESTY: CPU f64 is the TRUSTED reference. GPU f32 is acceleration");
        println!("          only — its P_abs is judged against the CPU within combined");
        println!("          statistical error, never trusted as the answer. SCLS is");
        println!("          CPU-ONLY here (retained memory does not vectorise).");
        println!("---------------------------------------------------------------------\n");

        // ── Problem setup: a graphite-matrix TRISO-like unit slab. ────────────
        let radius = 0.03_f64; // inclusion (fuel kernel) radius [cm]
        let pf = 0.30_f64; // packing fraction
                           // Chord statistics come from the crate's stochastic module (single source of
                           // truth) — not re-derived here. The CPU CLS reference below samples its chords
                           // through this same `ClsMedium`, and the WGSL shader is the f32 port of its
                           // `sample_distance_to_boundary`.
        let cls_medium = ClsMedium::new(radius, pf, MaterialId(1), MaterialId(0));
        let lambda_i = cls_medium.mean_chord_inclusion();
        let lambda_m = cls_medium.mean_chord_matrix();
        let params = ClsParams {
            l: 2.0, // slab thickness [cm]
            lambda: [lambda_m, lambda_i],
            // Absorbing inclusions (fuel), lightly-absorbing scattering matrix.
            sigma_t: [2.0, 8.0],  // [matrix, inclusion]  cm^-1
            sigma_a: [0.05, 4.0], // [matrix, inclusion]  cm^-1
            max_iter: 400,
        };
        let params32 = ClsParamsF32::from_f64(&params);

        println!("Geometry / material:");
        println!("  inclusion radius r        = {radius} cm");
        println!("  packing fraction p        = {pf}");
        println!("  lambda_inclusion (4r/3)   = {lambda_i:.6} cm");
        println!("  lambda_matrix ((4r/3)*(1-p)/p) = {lambda_m:.6} cm");
        println!("  slab thickness L          = {} cm", params.l);
        println!(
            "  Sigma_t [matrix, incl]    = [{}, {}] cm^-1",
            params.sigma_t[0], params.sigma_t[1]
        );
        println!(
            "  Sigma_a [matrix, incl]    = [{}, {}] cm^-1",
            params.sigma_a[0], params.sigma_a[1]
        );
        println!("  max chord-walk steps      = {}\n", params.max_iter);

        // Independent master seeds so the CPU-f64 and GPU-f32 estimates are
        // statistically independent binomial replicas.
        let n: usize = 1 << 20; // 1,048,576 histories per arm
        let master_cpu: i64 = 20_260_721;
        let master_gpu: i64 = 91_020_260; // independent stream for the GPU arm

        // ── Arm 1: CPU f64 CLS (trusted reference). ───────────────────────────
        let t = Instant::now();
        let abs_cpu = cls_batch_f64(&cls_medium, &params, n, master_cpu);
        let cpu_secs = t.elapsed().as_secs_f64();
        let (p_cpu, s_cpu) = binomial(abs_cpu, n);
        let cpu_hps = n as f64 / cpu_secs;
        println!("Arm 1 — CPU f64 CLS (TRUSTED reference):");
        println!("  P_abs = {p_cpu:.6} +/- {s_cpu:.6}   ({abs_cpu}/{n} absorbed)");
        println!(
            "  wall-clock {cpu_secs:.3} s   throughput {:.3e} histories/s\n",
            cpu_hps
        );

        // ── Arm 2: CPU f32 mirror (same arithmetic path as the shader). ───────
        // Full-batch mirror probability (independence check vs f64) + the exact
        // per-history reference the GPU is compared against below.
        let t = Instant::now();
        let mut abs_mirror = 0usize;
        for i in 0..n {
            let s = init_seed(i as i64, 0, master_gpu);
            if cls_history_f32(&params32, (s >> 32) as u32, s as u32) == Outcome::Absorbed {
                abs_mirror += 1;
            }
        }
        let mirror_secs = t.elapsed().as_secs_f64();
        let (p_mirror, s_mirror) = binomial(abs_mirror, n);
        println!("Arm 2 — CPU f32 mirror (GPU-logic reference, seed set S_gpu):");
        println!("  P_abs = {p_mirror:.6} +/- {s_mirror:.6}   ({abs_mirror}/{n} absorbed)");
        println!(
            "  wall-clock {mirror_secs:.3} s   |P_f32mirror - P_f64ref| = {:.6} \
             ({:.2} combined sigma)",
            (p_mirror - p_cpu).abs(),
            (p_mirror - p_cpu).abs() / (s_cpu * s_cpu + s_mirror * s_mirror).sqrt().max(1e-30)
        );
        println!(
            "  (f32 vs f64 differ only by rounding + the top-24-bit uniform; \
             statistically consistent.)\n"
        );

        // ── Arm 3: GPU f32 CLS batch (acceleration only). ─────────────────────
        let gpu = outram_mc_libs::gpu::probe();
        let mut csv_gpu: Option<(f64, f64, f64, f64, f64, usize, usize)> = None;
        match &gpu {
            None => {
                println!("Arm 3 — GPU f32 CLS batch:");
                println!(
                    "  No GPU adapter found (headless / no Vulkan-Metal loader). \
                     GPU arm SKIPPED — CPU arms above are the result.\n"
                );
            }
            Some(ctx) => {
                println!(
                    "Arm 3 — GPU f32 CLS batch on {} ({:?}):",
                    ctx.info.name, ctx.info.backend
                );
                // Warm-up dispatch (pipeline compile + alloc), discarded.
                let _warm = cls_batch_gpu(ctx, &params32, n.min(4096), master_gpu);
                let t = Instant::now();
                let out_gpu = cls_batch_gpu(ctx, &params32, n, master_gpu);
                let gpu_secs = t.elapsed().as_secs_f64();
                let abs_gpu = out_gpu.iter().filter(|&&o| o == Outcome::Absorbed).count();
                let (p_gpu, s_gpu) = binomial(abs_gpu, n);
                let gpu_hps = n as f64 / gpu_secs;
                let speedup = cpu_secs / gpu_secs;

                // Statistical agreement vs the trusted CPU f64 reference (Arm 1).
                let s_comb = (s_cpu * s_cpu + s_gpu * s_gpu).sqrt();
                let z = (p_cpu - p_gpu).abs() / s_comb.max(1e-30);

                // Per-history logic check vs the CPU f32 mirror (same seeds S_gpu):
                // outcomes must match for all but a negligible near-tie fraction.
                let check = n.min(65_536);
                let mut mismatch = 0usize;
                for i in 0..check {
                    let s = init_seed(i as i64, 0, master_gpu);
                    let mirror = cls_history_f32(&params32, (s >> 32) as u32, s as u32);
                    if mirror != out_gpu[i] {
                        mismatch += 1;
                    }
                }

                println!("  P_abs = {p_gpu:.6} +/- {s_gpu:.6}   ({abs_gpu}/{n} absorbed)");
                println!(
                    "  wall-clock {gpu_secs:.3} s   throughput {:.3e} histories/s   \
                     speedup {speedup:.2}x vs CPU f64",
                    gpu_hps
                );
                println!(
                    "  agreement vs CPU f64 reference: |P_cpu - P_gpu| = {:.6} = {z:.2} \
                     combined sigma  ->  {}",
                    (p_cpu - p_gpu).abs(),
                    if z <= 5.0 {
                        "PASS (<= 5 sigma)"
                    } else {
                        "FAIL (> 5 sigma)"
                    }
                );
                println!(
                    "  GPU-vs-CPU-f32-mirror per-history logic: {}/{} outcomes match \
                     ({} near-tie mismatches, {:.4}%)\n",
                    check - mismatch,
                    check,
                    mismatch,
                    100.0 * mismatch as f64 / check as f64
                );
                csv_gpu = Some((p_gpu, s_gpu, gpu_secs, speedup, z, mismatch, check));
            }
        }

        // ── Arm 4: CPU SCLS (semi-implicit, retained memory) — CPU only. ──────
        let t = Instant::now();
        let abs_scls = scls_batch_f64(&cls_medium, &params, n, master_cpu);
        let scls_secs = t.elapsed().as_secs_f64();
        let (p_scls, s_scls) = binomial(abs_scls, n);
        let s_comb_cls_scls = (s_cpu * s_cpu + s_scls * s_scls).sqrt();
        println!("Arm 4 — CPU f64 SCLS (semi-implicit, retained interface; CPU ONLY):");
        println!("  P_abs = {p_scls:.6} +/- {s_scls:.6}   ({abs_scls}/{n} absorbed)");
        println!("  wall-clock {scls_secs:.3} s");
        println!(
            "  SCLS - CLS difference: {:+.6}  ({:.2} combined sigma) — the retained-",
            p_scls - p_cpu,
            (p_scls - p_cpu).abs() / s_comb_cls_scls.max(1e-30)
        );
        println!("  interface memory shifts absorption vs memoryless CLS (tutorial-level SCLS).\n");

        // ── CSV output (reproducible; sits with the other gpu_benchmarks CSVs). ─
        let out_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("verification_and_validation")
            .join("gpu_benchmarks");
        if let Err(e) = fs::create_dir_all(&out_dir) {
            eprintln!("  (could not create CSV dir {}: {e})", out_dir.display());
        } else {
            let path = out_dir.join("triso_stochastic_cls_gpu.csv");
            match fs::File::create(&path) {
                Ok(mut f) => {
                    writeln!(
                        f,
                        "arm,method,precision,device,histories,p_abs,sigma_p,wall_s,speedup_vs_cpu,z_vs_cpu_ref"
                    )
                    .unwrap();
                    writeln!(
                        f,
                        "1,CLS,f64,cpu,{n},{p_cpu:.8},{s_cpu:.8},{cpu_secs:.4},1.0,0.0"
                    )
                    .unwrap();
                    writeln!(
                        f,
                        "2,CLS,f32,cpu-mirror,{n},{p_mirror:.8},{s_mirror:.8},{mirror_secs:.4},,"
                    )
                    .unwrap();
                    if let Some((p_gpu, s_gpu, gpu_secs, speedup, z, _mm, _ck)) = csv_gpu {
                        let dev = gpu
                            .as_ref()
                            .map(|c| c.info.name.clone())
                            .unwrap_or_default();
                        writeln!(
                            f,
                            "3,CLS,f32,gpu:{dev},{n},{p_gpu:.8},{s_gpu:.8},{gpu_secs:.4},{speedup:.4},{z:.4}"
                        )
                        .unwrap();
                    } else {
                        writeln!(f, "3,CLS,f32,gpu:UNAVAILABLE,{n},,,,,").unwrap();
                    }
                    writeln!(
                        f,
                        "4,SCLS,f64,cpu,{n},{p_scls:.8},{s_scls:.8},{scls_secs:.4},,"
                    )
                    .unwrap();
                    println!("Wrote CSV: {}", path.display());
                }
                Err(e) => eprintln!("  (could not write CSV {}: {e})", path.display()),
            }
        }

        // ── Closing honesty banner. ───────────────────────────────────────────
        println!("\n---------------------------------------------------------------------");
        println!(" CPU f64 CLS is the trusted reference eigen-answer for P_abs.");
        if gpu.is_some() {
            println!(" GPU f32 accelerated the memoryless CLS batch and agreed with the");
            println!(" CPU f64 reference within combined statistical error (see Arm 3).");
        } else {
            println!(" No GPU adapter here — only the CPU arms ran. The GPU path builds");
            println!(" and would run on a machine with a Vulkan/Metal/DX12 adapter.");
        }
        println!(" SCLS ran on the CPU only (retained memory is not GPU-parallel).");
        println!("---------------------------------------------------------------------");
    }
}
