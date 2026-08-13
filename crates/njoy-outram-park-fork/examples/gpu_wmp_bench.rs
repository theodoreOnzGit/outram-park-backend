//! GPU vs CPU windowed-multipole (WMP) cross-section benchmark.
//!
//! # What this benchmarks
//!
//! The **full-fidelity windowed-multipole evaluator** — the complex Faddeeva
//! pole sum over each window's pole range, for all three reaction channels
//! (scatter / absorption / fission) — evaluated across an entire incident-energy
//! grid. The CPU reference is
//! [`njoy_outram_park_fork::gpu_wmp::wmp_evaluate_batch_cpu`] (`f64`); the GPU
//! path is [`njoy_outram_park_fork::gpu::GpuContext::wmp_evaluate_batch`]
//! (`f32`, WGSL compute). This is a **compute-bound** kernel (many
//! floating-point pole terms per energy, little memory traffic), so it is the
//! kind of workload a GPU should win on once the grid is large enough to amortise
//! dispatch/transfer overhead. The benchmark sweeps grid sizes to find the
//! crossover.
//!
//! Nuclide: **U-238** from the embedded CORE WMP library (602 poles), at
//! **300 K**.
//!
//! # Units
//! - Incident neutron energy: electron-volts (**eV**).
//! - Cross sections σ: **barn**.
//! - Temperature: **kelvin (K)**.
//!
//! # Trust model
//!
//! **The CPU `f64` path is the trusted reference.** The GPU `f32` path is
//! *acceleration only* — its reduction order and single precision mean it will
//! not bit-match the CPU, so this example also reports the max relative error of
//! the total cross section (σ_s + σ_a) between the two paths as an agreement
//! check. V&V is judged on the CPU path; the GPU number quantifies the accuracy
//! cost of the speedup, not a second source of truth.
//!
//! # How to run
//!
//! ```bash
//! cargo run -p njoy-outram-park-fork --release --example gpu_wmp_bench
//! ```
//!
//! Needs a GPU adapter (Vulkan/Metal/DX12). With no adapter (headless CI,
//! Android) it prints a notice and exits 0 — it never panics. On success it
//! prints a table + verdict to stdout and writes a **per-machine** markdown
//! report + CSV into the crate's **git-ignored**
//! `verification_and_validation/local_perf/` directory (via
//! [`njoy_outram_park_fork::perf_report`]). GPU/CPU timings differ per machine,
//! so those numbers stay local and are never committed; the committed
//! `verification_and_validation/gpu_wmp_benchmark.md` is a methodology template
//! only. The report header names the detected hardware (GPU adapter + backend,
//! CPU cores, OS).

// On Android there is no desktop GPU path — emit a stub main so the example
// still compiles (an empty file would have no `main`).
#[cfg(target_os = "android")]
fn main() {
    eprintln!("gpu_wmp_bench: desktop-only (no GPU compute path on Android)");
}

#[cfg(not(target_os = "android"))]
fn main() {
    use njoy_outram_park_fork::gpu;
    use njoy_outram_park_fork::gpu_wmp::wmp_evaluate_batch_cpu;
    use njoy_outram_park_fork::wmp::WmpLibrary;
    use std::time::Instant;

    // 1. Load U-238 from the embedded CORE WMP library.
    let wmp = match WmpLibrary::core().get("U238") {
        Ok(w) => w,
        Err(e) => {
            println!("gpu_wmp_bench: U238 not available in the CORE WMP library ({e}); nothing to benchmark.");
            return;
        }
    };

    // 2. Probe for a GPU adapter; fall back gracefully if none.
    let Some(ctx) = gpu::probe() else {
        eprintln!("gpu_wmp_bench: no GPU adapter; this benchmark needs a GPU. Exiting.");
        return;
    };

    let temp_k: f64 = 300.0;
    let grid_sizes: [usize; 4] = [1_000, 10_000, 100_000, 1_000_000];

    // One result row per grid size.
    struct Row {
        n: usize,
        cpu_ms: f64,
        gpu_ms: f64,
        speedup: f64,
        max_rel_err_total: f64,
    }
    let mut rows: Vec<Row> = Vec::with_capacity(grid_sizes.len());

    println!(
        "GPU vs CPU windowed-multipole benchmark — U-238 (602 poles), {temp_k} K\n\
         energy in [{:.6e}, {:.6e}] eV, log-spaced grid, best-of-3, warm-up excluded\n\
         CPU is f64 (trusted reference); GPU is f32 (acceleration).\n",
        wmp.e_min, wmp.e_max
    );

    for &n in &grid_sizes {
        // 3. Build a log-spaced energy grid over [e_min, e_max] (f64 + f32 copy).
        let energies_f64: Vec<f64> = log_spaced(wmp.e_min, wmp.e_max, n);
        let energies_f32: Vec<f32> = energies_f64.iter().map(|&e| e as f32).collect();

        // Warm up each path once, untimed — the first GPU dispatch pays a
        // one-off pipeline-creation cost that should not be charged to the
        // measured runs.
        let cpu_ref = wmp_evaluate_batch_cpu(&wmp, &energies_f64, temp_k);
        let _gpu_warm = ctx.wmp_evaluate_batch(&wmp, &energies_f32, temp_k as f32);

        // 4a. Time CPU: best of 3, report the min.
        let mut cpu_ms = f64::INFINITY;
        for _ in 0..3 {
            let t0 = Instant::now();
            let out = wmp_evaluate_batch_cpu(&wmp, &energies_f64, temp_k);
            let dt = t0.elapsed().as_secs_f64() * 1_000.0;
            std::hint::black_box(&out);
            if dt < cpu_ms {
                cpu_ms = dt;
            }
        }

        // 4b. Time GPU: best of 3, report the min.
        let mut gpu_ms = f64::INFINITY;
        let mut gpu_last = Vec::new();
        for _ in 0..3 {
            let t0 = Instant::now();
            let out = ctx.wmp_evaluate_batch(&wmp, &energies_f32, temp_k as f32);
            let dt = t0.elapsed().as_secs_f64() * 1_000.0;
            if dt < gpu_ms {
                gpu_ms = dt;
            }
            gpu_last = out;
        }

        // 4c. Agreement: max relative error of the total σ_t = σ_s + σ_a,
        // GPU (f32) vs CPU (f64), denom = |cpu.total()| floored at 1e-30.
        let mut max_rel_err_total = 0.0_f64;
        for (cpu, gpu) in cpu_ref.iter().zip(gpu_last.iter()) {
            let cpu_total = cpu.total();
            let gpu_total = gpu.scatter as f64 + gpu.absorption as f64;
            let rel = (gpu_total - cpu_total).abs() / cpu_total.abs().max(1e-30);
            if rel > max_rel_err_total {
                max_rel_err_total = rel;
            }
        }

        let speedup = cpu_ms / gpu_ms;
        rows.push(Row {
            n,
            cpu_ms,
            gpu_ms,
            speedup,
            max_rel_err_total,
        });
    }

    // 6. Plain-text table to stdout.
    println!(
        "{:>10}  {:>12}  {:>12}  {:>9}  {:>18}",
        "n_energy", "cpu_ms", "gpu_ms", "speedup", "max_rel_err_total"
    );
    for r in &rows {
        println!(
            "{:>10}  {:>12.3}  {:>12.3}  {:>9.3}  {:>18.3e}",
            r.n, r.cpu_ms, r.gpu_ms, r.speedup, r.max_rel_err_total
        );
    }

    // Verdict: first N where GPU overtakes CPU, and the peak speedup.
    let crossover = rows.iter().find(|r| r.speedup > 1.0).map(|r| r.n);
    let peak = rows.iter().max_by(|a, b| {
        a.speedup
            .partial_cmp(&b.speedup)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    match (crossover, peak) {
        (Some(n), Some(p)) => println!(
            "\nVerdict: GPU overtakes CPU (speedup > 1) at N = {n}; peak speedup {:.3}x at N = {}.",
            p.speedup, p.n
        ),
        (None, Some(p)) => println!(
            "\nVerdict: GPU never overtakes CPU across the swept grid sizes; best was {:.3}x at N = {}.",
            p.speedup, p.n
        ),
        _ => println!("\nVerdict: no grid sizes were benchmarked."),
    }

    // 5. Emit a PER-MACHINE performance report into the git-ignored
    // `verification_and_validation/local_perf/` directory. Committed markdown is
    // methodology-only (see `verification_and_validation/gpu_wmp_benchmark.md`);
    // the actual timings are machine-specific and stay local, generated fresh on
    // whatever hardware this runs on. See `perf_report`.
    use njoy_outram_park_fork::perf_report::{
        format_perf_report, write_local_perf_report, HardwareInfo, PerfRow,
    };

    // Detect this machine's hardware: GPU adapter label from the live context,
    // CPU cores + OS from std.
    let hw = HardwareInfo::detect(ctx.adapter_label());
    println!("\nPerformance on your system: {}", hw.summary_line());

    let perf_rows: Vec<PerfRow> = rows
        .iter()
        .map(|r| PerfRow {
            n_energy: r.n,
            cpu_ms: r.cpu_ms,
            gpu_ms: r.gpu_ms,
            speedup: r.speedup,
            max_rel_err: r.max_rel_err_total,
        })
        .collect();

    let methodology = format!(
        "Full-fidelity windowed-multipole evaluator for **U-238** (602 poles) at \
         **300 K**: GPU (`f32`, WGSL compute) vs the trusted CPU reference \
         (`f64`). Compute-bound kernel (many Faddeeva pole terms per energy).\n\n\
         - Energy grid: log-spaced over [{:.6e}, {:.6e}] eV.\n\
         - Timing: best-of-3 wall-clock, warm-up run excluded (first GPU dispatch \
           pays a one-off pipeline-creation cost).\n\
         - CPU `f64` is the trusted reference; GPU `f32` is acceleration only.\n\
         - Agreement metric: max relative error of the total cross section \
           (sigma_s + sigma_a), GPU vs CPU, denominator floored at 1e-30.\n\
         - Units: energy in eV, cross sections in barn, temperature in K.",
        wmp.e_min, wmp.e_max
    );

    let report = format_perf_report(
        "GPU vs CPU windowed-multipole (WMP) benchmark — your machine",
        &methodology,
        &hw,
        &perf_rows,
    );

    match write_local_perf_report(env!("CARGO_MANIFEST_DIR"), "gpu_wmp_benchmark.md", &report) {
        Ok(p) => println!("Wrote per-machine report (git-ignored): {}", p.display()),
        Err(e) => eprintln!("gpu_wmp_bench: failed to write local report: {e}"),
    }

    // Also drop a plottable CSV next to it (also git-ignored, local only).
    let mut csv = String::from("n_energy,cpu_ms,gpu_ms,speedup,max_rel_err_total\n");
    for r in &rows {
        csv.push_str(&format!(
            "{},{:.6},{:.6},{:.6},{:.6e}\n",
            r.n, r.cpu_ms, r.gpu_ms, r.speedup, r.max_rel_err_total
        ));
    }
    match write_local_perf_report(env!("CARGO_MANIFEST_DIR"), "gpu_wmp_benchmark.csv", &csv) {
        Ok(p) => println!("Wrote per-machine CSV (git-ignored): {}", p.display()),
        Err(e) => eprintln!("gpu_wmp_bench: failed to write local CSV: {e}"),
    }
}

/// Build `n` log-spaced energies over the closed interval `[e_min, e_max]` (eV).
///
/// Returns geometrically-spaced points: `e_min * (e_max/e_min)^(i/(n-1))` for
/// `i = 0..n`, so the first point is exactly `e_min` and the last exactly
/// `e_max`. Requires `e_min > 0` (WMP grids are strictly positive) and `n >= 2`;
/// for `n == 1` it returns a single point at `e_min`.
#[cfg(not(target_os = "android"))]
fn log_spaced(e_min: f64, e_max: f64, n: usize) -> Vec<f64> {
    if n == 0 {
        return Vec::new();
    }
    if n == 1 {
        return vec![e_min];
    }
    let log_lo = e_min.ln();
    let log_hi = e_max.ln();
    let step = (log_hi - log_lo) / ((n - 1) as f64);
    (0..n).map(|i| (log_lo + step * i as f64).exp()).collect()
}
