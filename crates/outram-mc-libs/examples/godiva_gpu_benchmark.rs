//! Godiva k_eff CPU-vs-GPU benchmark — an *honest* accounting of what the RTX
//! 3050 accelerates today, and what it does not.
//!
//! This example benchmarks the Godiva bare-sphere criticality problem (ICSBEP
//! **HEU-MET-FAST-001**, r = 8.7407 cm, benchmark k_eff = 1.0000 ± 0.0010) on
//! two axes:
//!
//! 1. **CPU k_eff baseline** — the trusted `f64` power iteration
//!    ([`run_keff`]). This is the reference eigenvalue. We time it, report
//!    k_eff ± σ and Δk in pcm versus the benchmark, and quote source-history
//!    throughput (histories/s).
//! 2. **Isolated XS-kernel GPU-vs-CPU throughput** — the *only* piece of the
//!    transport path that is GPU-ready today: batched macroscopic-total cross
//!    section lookups. We tabulate the same Sigma_t(E) \[cm^-1\] the transport
//!    loop queries ([`UnionTotalXs`]) and push large batches of collision-energy
//!    queries through both the CPU (`f64`, reference) and GPU (`f32`,
//!    accelerator) paths, reporting queries/s, GPU speedup, and the f32-vs-f64
//!    agreement (max/mean |ΔSigma_t|).
//!
//! # What is NOT measured here (read this before quoting a "GPU speedup")
//!
//! **End-to-end GPU-accelerated k_eff transport is PENDING beads op-u6s.4.**
//! The history-based transport loop is branchy and not yet wired to run on the
//! GPU, so this example does **not** and will **not** fabricate a "GPU k_eff".
//! The GPU number here is strictly the throughput of the *isolated*
//! cross-section-interpolation kernel — a real, embarrassingly-parallel
//! sub-kernel of transport, but not the whole solver. The CPU `f64` result is
//! the trusted reference in every comparison; the GPU `f32` path is acceleration
//! only and is never presented as the reference (per this crate's `CLAUDE.md`
//! "CPU is truth" rule and the `gpu` module contract).
//!
//! # Methodology
//!
//! **Data.** LOW tier ([`Nuclide::from_core`]) for U-234/U-235/U-238 — embedded
//! WMP + Watt-collapsed fast MGXS, so the example runs offline with no network
//! and is reproducible. Godiva atom densities \[atoms/barn-cm\]: U-234 4.9184e-4,
//! U-235 4.4994e-2, U-238 2.4984e-3; temperature 293.6 K; radius 8.7407 cm.
//!
//! **CPU k_eff.** [`run_keff`] with 5000 histories × [40 inactive + 120 active]
//! generations, seed fixed (default). Wall-clock via [`std::time::Instant`];
//! source-history throughput = `n_particles * (n_inactive + n_active) /
//! wall_secs`. Judged against ICSBEP HEU-MET-FAST-001 (k_eff = 1.0000 ± 0.0010).
//!
//! **XS-kernel throughput.** [`UnionTotalXs::tabulate`] on 4096 log-spaced points
//! over 1e-3 .. 2e7 eV. For batch sizes N ∈ {2^16, 2^18, 2^20} we sample N query
//! energies from the U-235 thermal-Watt fission spectrum (the same spectrum that
//! seeds transport collisions), clamped to \[1e-3, 2e7\] eV. For each batch:
//!   - **CPU:** average wall-clock of 5 [`UnionTotalXs::lookup_cpu`] reps.
//!   - **GPU:** one warm-up dispatch, then average wall-clock of 5
//!     [`UnionTotalXs::lookup_gpu`] reps. This timing is **end-to-end
//!     host-visible time** — it includes the `f32` buffer upload, the compute
//!     dispatch, and the read-back to host memory, not just the shader math.
//!     That is the honest number for "how long does a batch lookup take from the
//!     caller's point of view", and for small batches it is often *upload/
//!     readback-dominated*, so a GPU speedup below 1.0 there is a real and
//!     expected finding, not a bug.
//! Throughput = N / wall_secs; GPU speedup = cpu_time / gpu_time =
//! gpu_qps / cpu_qps. Agreement = max & mean |gpu_f32 - cpu_f64| over the batch
//! \[cm^-1\]. If [`probe`] returns `None` (no GPU adapter) the GPU columns are
//! skipped and reported as `NaN`; the CPU throughput is still emitted.
//!
//! **Outputs.** Two plottable CSVs under
//! `crates/outram-mc-libs/verification_and_validation/gpu_benchmarks/`:
//!   - `godiva_keff_convergence.csv` — per-generation eigenvalue trace with a
//!     progressive cumulative mean/σ over the *active* generations. Inactive
//!     rows carry `NaN` in the cumulative columns (the header line documents
//!     this); the first active generation has `NaN` σ (needs ≥ 2 samples).
//!   - `godiva_xs_throughput.csv` — one row per batch size with CPU/GPU
//!     timing, throughput, speedup, and agreement.
//! The `verification_and_validation/` tree is gitignored (reproducible local
//! outputs); the interpretation lives in the Results section below.
//!
//! # Results (2026-07-17, NVIDIA GeForce RTX 3050 — Vulkan backend)
//!
//! Measured on this machine by running the command below; numbers transcribed
//! from the real run (no fabricated values). k_eff is deterministic (fixed
//! seed) and reproduced bit-for-bit across 3 runs; timings/throughput are the
//! representative first run and were stable to ~2% across the 3 runs.
//!
//! **CPU k_eff baseline (LOW tier, 5000 × [40 + 120]).**
//!
//! | Quantity | Value |
//! |---|---|
//! | k_eff | 1.01024 ± 0.00171 |
//! | Δk vs benchmark (1.0000) | +1024 pcm |
//! | wall-clock | 1.224 s |
//! | source-history throughput | 653 725 histories/s |
//!
//! The k_eff sits ~+1024 pcm high, consistent with the LOW tier's documented
//! Godiva bias (no self-shielding, one mean cosine, evaporation stand-in for
//! resolved inelastic) — see `crates/outram-mc-libs/src/physics/keff.rs`. This
//! benchmark is about *performance*, not closing that bias; the eigenvalue is
//! reported only to confirm the run is the genuine, converged Godiva case.
//!
//! **Isolated XS-kernel throughput (GPU vs CPU, end-to-end host-visible time).**
//!
//! | batch | CPU ms | GPU ms | CPU Mqueries/s | GPU Mqueries/s | GPU speedup | max \|Δ\| cm^-1 | mean \|Δ\| cm^-1 |
//! |---|---|---|---|---|---|---|---|
//! | 65 536 (2^16) | 2.157 | 0.866 | 30.4 | 75.7 | 2.49× | 2.561e-06 | 7.031e-09 |
//! | 262 144 (2^18) | 8.727 | 1.466 | 30.0 | 178.8 | 5.95× | 1.245e-05 | 7.197e-09 |
//! | 1 048 576 (2^20) | 35.312 | 4.779 | 29.7 | 219.4 | 7.39× | 1.854e-05 | 7.119e-09 |
//!
//! **Interpretation.** On this machine the GPU beats the CPU at *every* batch
//! size tested (2.49× at 2^16, rising to 7.39× at 2^20) — the CPU single-thread
//! `f64` interpolation runs at a steady ~30 Mqueries/s while the GPU's effective
//! throughput climbs from 76 to 219 Mqueries/s as the fixed host-visible
//! overhead (the `f32` buffer upload + read-back that this end-to-end timing
//! includes) amortises over a larger batch. The speedup still *grows with batch
//! size*, which is the tell-tale of an upload/readback-bound kernel: the bigger
//! the batch, the smaller the fixed overhead's share, so the largest batch wins
//! most. (Had the CPU reference been vectorised/multi-threaded, the small-batch
//! GPU speedup could well drop below 1× — the honest reading is "GPU wins for
//! large batches on this hardware against this single-thread CPU baseline", not
//! "GPU is unconditionally faster".) Agreement is excellent throughout: max
//! |ΔSigma_t| ≈ 1.85e-5 cm^-1 against Sigma_t of O(0.05-40 cm^-1), i.e. the
//! `f32` path tracks the `f64` reference to single-precision rounding — sound as
//! an accelerator, never trusted as the reference. Bottom line: batched Sigma_t
//! lookup is worth offloading to the RTX 3050; wiring it into the full transport
//! loop (op-u6s.4) is where an end-to-end k_eff speedup would actually be
//! measured.
//!
//! # Run
//!
//! ```text
//! cargo run -p outram-mc-libs --release --example godiva_gpu_benchmark
//! ```

use std::fs;
use std::path::PathBuf;
use std::time::Instant;

use outram_mc_libs::gpu::union_grid::UnionTotalXs;
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::physics::keff::{run_keff, KeffResult, KeffSettings};
use outram_mc_libs::rng::distributions::watt;

/// Godiva atom densities and temperature (ICSBEP HEU-MET-FAST-001).
const RADIUS_CM: f64 = 8.7407;
const TEMPERATURE_K: f64 = 293.6;
const N_U234: f64 = 4.9184e-4;
const N_U235: f64 = 4.4994e-2;
const N_U238: f64 = 2.4984e-3;

/// Energy grid bounds \[eV\] for the tabulated Sigma_t used by the XS kernel.
const E_MIN_EV: f64 = 1e-3;
const E_MAX_EV: f64 = 2e7;
/// Number of log-spaced grid points in the tabulation.
const N_GRID: usize = 4096;

/// U-235 thermal-Watt fission-spectrum parameters (same as `KeffSettings`
/// default), used to draw representative collision-energy queries.
const WATT_A: f64 = 0.988e6;
const WATT_B: f64 = 2.249e-6;

/// Batch sizes for the XS-kernel scaling curve.
const BATCH_SIZES: [usize; 3] = [1 << 16, 1 << 18, 1 << 20];
/// Timed repetitions averaged per batch (both CPU and GPU).
const N_REPS: usize = 5;

fn main() {
    println!("========================================================================");
    println!(" Godiva k_eff  —  CPU baseline  +  isolated XS-kernel GPU-vs-CPU throughput");
    println!(" ICSBEP HEU-MET-FAST-001  (r = {RADIUS_CM} cm, benchmark k_eff = 1.0000 +/- 0.0010)");
    println!("========================================================================\n");

    let (material, nuclides) = godiva_material();

    // ── 1. CPU k_eff baseline (the trusted reference). ────────────────────────
    let keff = run_cpu_keff(&material, &nuclides);

    // ── 2. Isolated XS-kernel GPU-vs-CPU throughput. ──────────────────────────
    let rows = run_xs_kernel_benchmark(&material, &nuclides);

    // ── 3. Write the two plottable CSVs. ──────────────────────────────────────
    let out_dir = csv_output_dir();
    fs::create_dir_all(&out_dir).expect("create verification_and_validation/gpu_benchmarks dir");
    let keff_csv = write_keff_convergence_csv(&out_dir, &keff);
    let xs_csv = write_xs_throughput_csv(&out_dir, &rows);

    // ── 4. Honesty banner. ────────────────────────────────────────────────────
    println!("\n------------------------------------------------------------------------");
    println!(" HONEST SCOPE NOTE");
    println!("------------------------------------------------------------------------");
    println!(" End-to-end GPU-ACCELERATED k_eff transport is PENDING beads op-u6s.4.");
    println!(" This benchmark measures only:");
    println!("   (a) the CPU f64 k_eff baseline (the trusted reference eigenvalue), and");
    println!("   (b) the isolated batched macroscopic-total XS-interpolation kernel's");
    println!("       GPU-vs-CPU throughput.");
    println!(" No GPU k_eff is reported: the branchy transport loop is not yet on GPU.");
    println!(" CPU f64 is truth; GPU f32 is acceleration only, never the reference.");
    println!("------------------------------------------------------------------------\n");

    println!("CSV outputs:");
    println!("  {}", keff_csv.display());
    println!("  {}", xs_csv.display());
}

/// Build the Godiva bare-sphere HEU material and its LOW-tier (`from_core`)
/// nuclide array. LOW tier keeps the run offline and reproducible (no network,
/// no HDF5). Atom densities are the ICSBEP HEU-MET-FAST-001 values above.
fn godiva_material() -> (Material, Vec<Nuclide>) {
    let nuclides = vec![
        Nuclide::from_core("U234").expect("U234 in CORE WMP library"),
        Nuclide::from_core("U235").expect("U235 in CORE WMP library"),
        Nuclide::from_core("U238").expect("U238 in CORE WMP library"),
    ];
    let material = Material {
        id: 1,
        name: "Godiva HEU".into(),
        temperature: TEMPERATURE_K,
        components: vec![
            NuclideComponent { nuclide_idx: 0, atom_density: N_U234 },
            NuclideComponent { nuclide_idx: 1, atom_density: N_U235 },
            NuclideComponent { nuclide_idx: 2, atom_density: N_U238 },
        ],
    };
    (material, nuclides)
}

/// Run the CPU k_eff power iteration, time it, and print k_eff ± σ, Δk pcm,
/// wall-clock, and source-history throughput. Returns the full [`KeffResult`]
/// (its per-generation trace feeds the convergence CSV).
fn run_cpu_keff(material: &Material, nuclides: &[Nuclide]) -> KeffResult {
    let settings = KeffSettings {
        n_particles: 5000,
        n_inactive: 40,
        n_active: 120,
        temperature_k: TEMPERATURE_K,
        ..KeffSettings::default()
    };

    println!("[1] CPU k_eff baseline (LOW tier, the trusted f64 reference)");
    println!(
        "    {} histories/gen, {} inactive + {} active generations",
        settings.n_particles, settings.n_inactive, settings.n_active
    );

    let t0 = Instant::now();
    let result = run_keff(RADIUS_CM, material, nuclides, &settings);
    let wall = t0.elapsed().as_secs_f64();

    let total_histories = settings.n_particles * (settings.n_inactive + settings.n_active);
    let histories_per_s = total_histories as f64 / wall;
    let pcm = (result.k_mean - 1.0) * 1.0e5;

    println!("    k_eff             = {:.5} +/- {:.5}", result.k_mean, result.k_std);
    println!("    ICSBEP benchmark  = 1.0000 +/- 0.0010");
    println!("    Delta-k           = {pcm:+.0} pcm");
    println!("    wall-clock        = {wall:.3} s");
    println!(
        "    throughput        = {histories_per_s:.0} histories/s  ({} histories total)\n",
        total_histories
    );

    result
}

/// One benchmarked batch size of the XS-kernel throughput sweep.
struct XsRow {
    batch_size: usize,
    cpu_ms: f64,
    /// `None` when no GPU adapter was available.
    gpu_ms: Option<f64>,
    cpu_qps: f64,
    gpu_qps: Option<f64>,
    /// GPU speedup = cpu_time / gpu_time = gpu_qps / cpu_qps.
    gpu_speedup: Option<f64>,
    /// max |gpu_f32 - cpu_f64| over the batch \[cm^-1\].
    max_abs_err: Option<f64>,
    /// mean |gpu_f32 - cpu_f64| over the batch \[cm^-1\].
    mean_abs_err: Option<f64>,
}

/// Tabulate Sigma_t and benchmark the batched CPU-vs-GPU lookup across
/// [`BATCH_SIZES`]. Prints a per-batch table and returns the rows for the CSV.
fn run_xs_kernel_benchmark(material: &Material, nuclides: &[Nuclide]) -> Vec<XsRow> {
    println!("[2] Isolated XS-kernel throughput  (batched macroscopic total Sigma_t lookup)");
    println!(
        "    tabulating Sigma_t on {N_GRID} log-spaced points over [{E_MIN_EV:.0e}, {E_MAX_EV:.0e}] eV"
    );
    let table = UnionTotalXs::tabulate(material, nuclides, E_MIN_EV, E_MAX_EV, N_GRID);

    // Probe for a real GPU. `None` => CPU-only, GPU columns skipped.
    let gpu = outram_mc_libs::gpu::probe();
    match &gpu {
        Some(ctx) => println!("    GPU adapter: {} ({:?})", ctx.info.name, ctx.info.backend),
        None => println!("    GPU adapter: NONE — GPU path skipped, CPU throughput only"),
    }
    println!(
        "    query energies: U-235 thermal-Watt spectrum, clamped to [{E_MIN_EV:.0e}, {E_MAX_EV:.0e}] eV"
    );
    println!(
        "    (GPU timing is END-TO-END host-visible: f32 upload + dispatch + readback)\n"
    );

    println!(
        "    {:>10}  {:>9}  {:>9}  {:>12}  {:>12}  {:>9}  {:>12}  {:>12}",
        "batch", "cpu_ms", "gpu_ms", "cpu_Mq/s", "gpu_Mq/s", "speedup", "max|d|", "mean|d|"
    );

    // A distinct RNG seed for the query-energy sampler (independent of transport).
    let mut seed: u64 = 0x9e3779b97f4a7c15;
    let mut rows = Vec::with_capacity(BATCH_SIZES.len());

    for &n in &BATCH_SIZES {
        let queries = sample_query_energies(&mut seed, n);

        // CPU: average of N_REPS timed reps (the trusted f64 path).
        let cpu_ref = table.lookup_cpu(&queries); // reference values for agreement
        let mut cpu_secs = 0.0;
        for _ in 0..N_REPS {
            let t = Instant::now();
            let out = table.lookup_cpu(&queries);
            cpu_secs += t.elapsed().as_secs_f64();
            std::hint::black_box(&out);
        }
        let cpu_ms = cpu_secs / N_REPS as f64 * 1e3;
        let cpu_qps = n as f64 / (cpu_secs / N_REPS as f64);

        // GPU: warm-up dispatch, then average of N_REPS timed reps (f32 path).
        let (gpu_ms, gpu_qps, gpu_speedup, max_err, mean_err) = match &gpu {
            Some(ctx) => {
                let warm = table.lookup_gpu(ctx, &queries); // warm-up (compile pipeline, alloc)
                std::hint::black_box(&warm);
                let mut gpu_secs = 0.0;
                let mut last: Vec<f32> = Vec::new();
                for _ in 0..N_REPS {
                    let t = Instant::now();
                    last = table.lookup_gpu(ctx, &queries);
                    gpu_secs += t.elapsed().as_secs_f64();
                }
                let gpu_ms = gpu_secs / N_REPS as f64 * 1e3;
                let gpu_qps = n as f64 / (gpu_secs / N_REPS as f64);
                let speedup = cpu_ms / gpu_ms;
                let (max_e, mean_e) = agreement(&last, &cpu_ref);
                (Some(gpu_ms), Some(gpu_qps), Some(speedup), Some(max_e), Some(mean_e))
            }
            None => (None, None, None, None, None),
        };

        println!(
            "    {:>10}  {:>9.3}  {:>9}  {:>12.1}  {:>12}  {:>9}  {:>12}  {:>12}",
            n,
            cpu_ms,
            fmt_opt3(gpu_ms),
            cpu_qps / 1e6,
            fmt_opt_mqs(gpu_qps),
            fmt_opt_speedup(gpu_speedup),
            fmt_opt_sci(max_err),
            fmt_opt_sci(mean_err),
        );

        rows.push(XsRow {
            batch_size: n,
            cpu_ms,
            gpu_ms,
            cpu_qps,
            gpu_qps,
            gpu_speedup,
            max_abs_err: max_err,
            mean_abs_err: mean_err,
        });
    }

    rows
}

/// Sample `n` representative collision-energy queries \[eV\] from the U-235
/// thermal-Watt fission spectrum, clamped to the tabulated grid \[E_MIN_EV,
/// E_MAX_EV\]. These stand in for the energies the transport loop would look up.
fn sample_query_energies(seed: &mut u64, n: usize) -> Vec<f64> {
    (0..n)
        .map(|_| watt(seed, WATT_A, WATT_B).clamp(E_MIN_EV, E_MAX_EV))
        .collect()
}

/// max and mean |gpu_f32 - cpu_f64| over a batch \[cm^-1\].
fn agreement(gpu: &[f32], cpu: &[f64]) -> (f64, f64) {
    let mut max_abs = 0.0f64;
    let mut sum_abs = 0.0f64;
    for (&g, &c) in gpu.iter().zip(cpu.iter()) {
        let d = (g as f64 - c).abs();
        max_abs = max_abs.max(d);
        sum_abs += d;
    }
    (max_abs, sum_abs / gpu.len().max(1) as f64)
}

/// Directory for the benchmark CSVs, resolved from the crate root at compile
/// time so the path is stable regardless of the shell's working directory.
fn csv_output_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("verification_and_validation")
        .join("gpu_benchmarks")
}

/// Write the per-generation k_eff convergence trace.
///
/// Columns: `generation,k_generation,k_cumulative_mean,k_cumulative_sigma,phase`.
/// `phase` is `inactive` for the first `n_inactive` generations (discarded) and
/// `active` after. Cumulative mean/σ are computed progressively over ACTIVE
/// generations only; inactive rows carry `NaN` in both cumulative columns, and
/// the first active generation carries `NaN` σ (σ needs ≥ 2 samples). A header
/// comment line documents this convention.
fn write_keff_convergence_csv(dir: &PathBuf, keff: &KeffResult) -> PathBuf {
    // n_inactive is not carried on KeffResult; recover it from the run settings
    // used in `run_cpu_keff` (40 inactive of 160 total generations).
    let n_inactive = 40usize;

    let mut s = String::new();
    s.push_str(
        "# Godiva k_eff convergence (LOW tier). cumulative mean/sigma are over ACTIVE\n\
         # generations only; inactive rows are NaN; first active row has NaN sigma (needs >=2).\n",
    );
    s.push_str("generation,k_generation,k_cumulative_mean,k_cumulative_sigma,phase\n");

    let mut active: Vec<f64> = Vec::new();
    for (i, &k) in keff.k_by_generation.iter().enumerate() {
        let (cum_mean, cum_sigma, phase) = if i < n_inactive {
            (f64::NAN, f64::NAN, "inactive")
        } else {
            active.push(k);
            let (m, sd) = mean_and_stderr(&active);
            (m, sd, "active")
        };
        s.push_str(&format!(
            "{},{:.6},{},{},{}\n",
            i,
            k,
            fmt_csv_f64(cum_mean),
            fmt_csv_f64(cum_sigma),
            phase
        ));
    }

    let path = dir.join("godiva_keff_convergence.csv");
    fs::write(&path, s).expect("write godiva_keff_convergence.csv");
    path
}

/// Write the XS-kernel throughput table.
///
/// Columns: `batch_size,cpu_ms,gpu_ms,cpu_queries_per_s,gpu_queries_per_s,
/// gpu_speedup,max_abs_err_cm^-1,mean_abs_err_cm^-1`. GPU columns are `NaN`
/// when no GPU adapter was available.
fn write_xs_throughput_csv(dir: &PathBuf, rows: &[XsRow]) -> PathBuf {
    let mut s = String::new();
    s.push_str(
        "# Godiva macroscopic-total XS-interpolation kernel throughput (CPU f64 ref vs GPU f32).\n\
         # gpu_speedup = cpu_ms / gpu_ms. errors are |gpu_f32 - cpu_f64| over the batch [cm^-1].\n\
         # gpu_ms timing is end-to-end host-visible (f32 upload + dispatch + readback).\n",
    );
    s.push_str(
        "batch_size,cpu_ms,gpu_ms,cpu_queries_per_s,gpu_queries_per_s,gpu_speedup,max_abs_err_cm^-1,mean_abs_err_cm^-1\n",
    );
    for r in rows {
        s.push_str(&format!(
            "{},{:.4},{},{:.1},{},{},{},{}\n",
            r.batch_size,
            r.cpu_ms,
            fmt_csv_opt(r.gpu_ms),
            r.cpu_qps,
            fmt_csv_opt(r.gpu_qps),
            fmt_csv_opt(r.gpu_speedup),
            fmt_csv_opt(r.max_abs_err),
            fmt_csv_opt(r.mean_abs_err),
        ));
    }
    let path = dir.join("godiva_xs_throughput.csv");
    fs::write(&path, s).expect("write godiva_xs_throughput.csv");
    path
}

/// Mean and standard error of the mean (1σ) of a slice; σ is `NaN` for < 2
/// samples (a progressive convergence trace has no spread on its first point).
fn mean_and_stderr(k: &[f64]) -> (f64, f64) {
    let n = k.len();
    let mean = k.iter().sum::<f64>() / n as f64;
    if n < 2 {
        return (mean, f64::NAN);
    }
    let var = k.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n as f64 - 1.0);
    (mean, (var / n as f64).sqrt())
}

// ── Small formatting helpers (keep the print/CSV code readable). ──────────────

/// Format an `f64` for a CSV cell: `NaN` literal, else 6 significant digits.
fn fmt_csv_f64(x: f64) -> String {
    if x.is_nan() {
        "NaN".to_string()
    } else {
        format!("{x:.6}")
    }
}

/// Format an `Option<f64>` CSV cell: `NaN` when `None`, else scientific/6-dp mix.
fn fmt_csv_opt(x: Option<f64>) -> String {
    match x {
        None => "NaN".to_string(),
        Some(v) => format!("{v:.6e}"),
    }
}

/// Console: optional ms to 3 dp, or `n/a`.
fn fmt_opt3(x: Option<f64>) -> String {
    x.map(|v| format!("{v:.3}")).unwrap_or_else(|| "n/a".to_string())
}

/// Console: optional queries/s in Mqueries/s to 1 dp, or `n/a`.
fn fmt_opt_mqs(x: Option<f64>) -> String {
    x.map(|v| format!("{:.1}", v / 1e6)).unwrap_or_else(|| "n/a".to_string())
}

/// Console: optional speedup like `2.37x`, or `n/a`.
fn fmt_opt_speedup(x: Option<f64>) -> String {
    x.map(|v| format!("{v:.2}x")).unwrap_or_else(|| "n/a".to_string())
}

/// Console: optional value in scientific notation to 3 dp, or `n/a`.
fn fmt_opt_sci(x: Option<f64>) -> String {
    x.map(|v| format!("{v:.3e}")).unwrap_or_else(|| "n/a".to_string())
}
