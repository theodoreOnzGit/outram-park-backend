// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! **Parity + crossover report for the hybrid GPU kernels** — bead
//! `op-yvj.4.7`, GitHub #16.
//!
//! Prints, for every GPU kernel in the crate, the measured deviation from the
//! serial `f64` oracle and the wall-clock crossover against `Serial` and
//! `CpuMulti`. The numbers this emits are the ones that belong in the kernels'
//! doc comments and in the V&V record — the workspace rule is that a V&V claim
//! carries both the methodology and the measured result, and this program is
//! the methodology.
//!
//! # Running it
//!
//! ```bash
//! cargo run --release -p outram-foam-basic-lib \
//!     --features gpu,parallel --example hybrid_gpu_report
//! ```
//!
//! Without a GPU adapter it reports the CPU columns and says so; that is a
//! valid outcome, not a failure.
//!
//! # What "crossover" means here
//!
//! The problem size at which the GPU path first beats `CpuMulti`, **including**
//! the host-device transfer, because the transfer is the cost that decides
//! whether a dispatch is worth it. A kernel that never crosses over is a
//! legitimate result and is reported as `never` — it should then stay off the
//! auto-select policy rather than shipping as a slower default.

#[cfg(target_os = "android")]
fn main() {
    println!(
        "hybrid_gpu_report is a desktop-only example: it measures the wgpu \
         backend, which is target-gated off Android."
    );
}

#[cfg(not(target_os = "android"))]
fn main() {
    desktop::run();
}

#[cfg(not(target_os = "android"))]
mod desktop {
    // Everything from here down exists only to drive the GPU kernels, so it
    // is gated exactly as they are — otherwise a `--no-default-features`
    // build of this example warns about a file full of unused helpers.
    #[cfg(all(feature = "gpu", not(target_os = "android")))]
    use std::sync::Arc;
    #[cfg(all(feature = "gpu", not(target_os = "android")))]
    use std::time::{Duration, Instant};

    #[cfg(all(feature = "gpu", not(target_os = "android")))]
    use outram_foam_basic_lib::compute::ComputeBackend;
    #[cfg(all(feature = "gpu", not(target_os = "android")))]
    use outram_foam_basic_lib::ldu_matrix::ldu_matrix::LduMatrix;
    #[cfg(all(feature = "gpu", not(target_os = "android")))]
    use outram_foam_basic_lib::ldu_matrix::parallel::{self as par, HybridLdu, LduTopology};

    /// Sizes swept for the crossover search. Spans the range a real FV mesh
    /// covers, from a toy case to a few million cells.
    #[cfg(all(feature = "gpu", not(target_os = "android")))]
    const SIZES: &[usize] = &[1 << 12, 1 << 14, 1 << 16, 1 << 18, 1 << 20, 1 << 22];

    /// Repeats per timing point, after one untimed warm-up.
    #[cfg(all(feature = "gpu", not(target_os = "android")))]
    const REPEATS: u32 = 5;

    pub fn run() {
        println!("# Hybrid backend — parity and crossover report\n");
        report_environment();

        #[cfg(all(feature = "gpu", not(target_os = "android")))]
        {
            use outram_foam_basic_lib::compute::gpu;
            match gpu::context() {
                Some(ctx) => {
                    println!("GPU adapter      : {}", ctx.adapter_label());
                    println!("max lanes        : {}", ctx.max_lanes());
                    println!("max storage bufs : {}\n", ctx.max_storage_buffers());
                    parity_section();
                    crossover_section();
                }
                None => println!("\nNo GPU adapter — GPU columns omitted (a valid outcome).\n"),
            }
        }
        #[cfg(not(all(feature = "gpu", not(target_os = "android"))))]
        {
            println!(
                "\nBuilt without the `gpu` feature — rebuild with \
                 `--features gpu,parallel` for the GPU columns.\n"
            );
        }
    }

    fn report_environment() {
        println!("host threads     : {}", num_threads());
        println!(
            "features         : gpu={} parallel={}",
            cfg!(feature = "gpu"),
            cfg!(feature = "parallel")
        );
        println!("f64 oracle       : ComputeBackend::Serial");
    }

    fn num_threads() -> usize {
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1)
    }

    /// A 1-D Laplacian on `n` cells — `diag = 2`, off-diagonals `-1`.
    #[cfg(all(feature = "gpu", not(target_os = "android")))]
    fn laplacian(n: usize) -> (LduMatrix, LduTopology) {
        let owner: Vec<usize> = (0..n - 1).collect();
        let neighbour: Vec<usize> = (1..n).collect();
        let mut m = LduMatrix::new(n, owner, neighbour);
        m.diag.iter_mut().for_each(|d| *d = 2.0);
        m.lower.iter_mut().for_each(|v| *v = -1.0);
        m.upper.iter_mut().for_each(|v| *v = -1.0);
        let t = LduTopology::from_matrix(&m);
        (m, t)
    }

    /// Test data for the **timing** sweep — cheap to generate, values
    /// irrelevant.
    #[cfg(all(feature = "gpu", not(target_os = "android")))]
    fn operand(n: usize) -> Vec<f64> {
        (0..n).map(|i| ((i % 17) as f64) * 0.25 - 2.0).collect()
    }

    /// Test data for the **parity** measurement.
    ///
    /// Deliberately *not* the dyadic fractions [`operand`] uses. A value like
    /// `0.25` is exactly representable in `f32`, so a kernel built from such
    /// inputs reports a deviation of exactly zero and measures nothing —
    /// an early version of this report did precisely that. These values are
    /// transcendental multiples with no short binary expansion, so every
    /// upload genuinely rounds and the figure below is the real `f32` cost.
    #[cfg(all(feature = "gpu", not(target_os = "android")))]
    fn parity_operand(n: usize) -> Vec<f64> {
        (0..n)
            .map(|i| {
                let t = (i as f64) * std::f64::consts::PI / 1000.0;
                t.sin() * std::f64::consts::E + (i as f64) / 3.0
            })
            .collect()
    }

    /// A Laplacian whose coefficients also resist exact `f32` representation,
    /// for the same reason as [`parity_operand`].
    #[cfg(all(feature = "gpu", not(target_os = "android")))]
    fn parity_laplacian(n: usize) -> (LduMatrix, LduTopology) {
        let owner: Vec<usize> = (0..n - 1).collect();
        let neighbour: Vec<usize> = (1..n).collect();
        let mut m = LduMatrix::new(n, owner, neighbour);
        for (i, d) in m.diag.iter_mut().enumerate() {
            *d = 2.0 + (i as f64) / 7.0_f64.sqrt();
        }
        for (i, v) in m.lower.iter_mut().enumerate() {
            *v = -1.0 - (i as f64) / 3.0_f64.sqrt();
        }
        for (i, v) in m.upper.iter_mut().enumerate() {
            *v = -1.0 - (i as f64) / 11.0_f64.sqrt();
        }
        let t = LduTopology::from_matrix(&m);
        (m, t)
    }

    /// Max and RMS relative deviation of `got` from `want`.
    #[cfg(all(feature = "gpu", not(target_os = "android")))]
    fn deviation(got: &[f64], want: &[f64]) -> (f64, f64) {
        let mut max = 0.0_f64;
        let mut sq = 0.0_f64;
        for (g, w) in got.iter().zip(want) {
            let rel = (g - w).abs() / w.abs().max(1.0);
            max = max.max(rel);
            sq += rel * rel;
        }
        (max, (sq / got.len().max(1) as f64).sqrt())
    }

    #[cfg(all(feature = "gpu", not(target_os = "android")))]
    fn time<F: FnMut()>(mut f: F) -> Duration {
        f(); // warm-up, excluded
        let t0 = Instant::now();
        for _ in 0..REPEATS {
            f();
        }
        t0.elapsed() / REPEATS
    }

    #[cfg(all(feature = "gpu", not(target_os = "android")))]
    fn parity_section() {
        use outram_foam_basic_lib::ldu_matrix::parallel::gpu as kgpu;

        println!("## Parity vs the serial f64 oracle\n");
        println!("Deviations are relative, computed over the whole output array.");
        println!("GPU kernels compute in f32 (WGSL has no f64), so ~1.2e-7 is the floor.\n");
        println!("| Kernel | n | max rel | RMS rel |");
        println!("|---|---:|---:|---:|");

        let n = 1 << 20;
        let (m, t) = parity_laplacian(n);
        let x = parity_operand(n);

        // SpMV
        let hybrid = HybridLdu::new(Arc::new(m.clone()));
        let want = hybrid.spmv(&x, ComputeBackend::Serial);
        let mut got = vec![0.0; n];
        if kgpu::spmv_into(&m, &t, &x, &mut got).is_some() {
            let (mx, rms) = deviation(&got, &want);
            println!("| `spmv` (1-D Laplacian) | {n} | {mx:.2e} | {rms:.2e} |");
        }

        // axpy
        let y0 = parity_operand(n);
        let mut want_y = y0.clone();
        par::axpy(2.5, &x, &mut want_y, ComputeBackend::Serial);
        let mut got_y = y0.clone();
        if kgpu::axpy(2.5, &x, &mut got_y).is_some() {
            let (mx, rms) = deviation(&got_y, &want_y);
            println!("| `axpy` | {n} | {mx:.2e} | {rms:.2e} |");
        }

        // scale
        let mut want_s = x.clone();
        par::scale(-1.5, &mut want_s, ComputeBackend::Serial);
        let mut got_s = x.clone();
        if kgpu::scale(-1.5, &mut got_s).is_some() {
            let (mx, rms) = deviation(&got_s, &want_s);
            println!("| `scale` | {n} | {mx:.2e} | {rms:.2e} |");
        }

        // dot / norm_l1 — scalars, so max == RMS by construction.
        let b: Vec<f64> = parity_operand(n).iter().map(|v| v * std::f64::consts::LN_2).collect();
        let want_d = par::dot(&x, &b, ComputeBackend::Serial);
        if let Some(got_d) = kgpu::dot(&x, &b) {
            let rel = (got_d - want_d).abs() / want_d.abs().max(1.0);
            println!("| `dot` | {n} | {rel:.2e} | — |");
        }
        let want_l1 = par::norm_l1(&x, ComputeBackend::Serial);
        if let Some(got_l1) = kgpu::norm_l1(&x) {
            let rel = (got_l1 - want_l1).abs() / want_l1.abs().max(1.0);
            println!("| `norm_l1` | {n} | {rel:.2e} | — |");
        }
        println!();
    }

    #[cfg(all(feature = "gpu", not(target_os = "android")))]
    fn crossover_section() {
        use outram_foam_basic_lib::ldu_matrix::parallel::gpu as kgpu;

        println!("## Crossover — wall clock per call, including host-device transfer\n");
        println!("| Kernel | n | serial | cpu-multi | gpu | gpu vs cpu-multi |");
        println!("|---|---:|---:|---:|---:|---:|");

        let mut spmv_cross: Option<usize> = None;
        let mut axpy_cross: Option<usize> = None;
        let mut dot_cross: Option<usize> = None;

        for &n in SIZES {
            let (m, t) = laplacian(n);
            let x = operand(n);
            let hybrid = HybridLdu::new(Arc::new(m.clone()));

            // --- spmv ---
            let ts = time(|| {
                std::hint::black_box(hybrid.spmv(&x, ComputeBackend::Serial));
            });
            let tc = time(|| {
                std::hint::black_box(hybrid.spmv(&x, ComputeBackend::CpuMulti));
            });
            let mut buf = vec![0.0; n];
            let tg = time(|| {
                std::hint::black_box(kgpu::spmv_into(&m, &t, &x, &mut buf));
            });
            let ratio = tc.as_secs_f64() / tg.as_secs_f64();
            if ratio > 1.0 && spmv_cross.is_none() {
                spmv_cross = Some(n);
            }
            println!(
                "| `spmv` | {n} | {} | {} | {} | {ratio:.2}x |",
                us(ts),
                us(tc),
                us(tg)
            );

            // --- axpy ---
            let y0 = operand(n);
            let mut y = y0.clone();
            let ts = time(|| {
                let mut yy = y0.clone();
                par::axpy(2.5, &x, &mut yy, ComputeBackend::Serial);
                std::hint::black_box(&yy);
            });
            let tc = time(|| {
                let mut yy = y0.clone();
                par::axpy(2.5, &x, &mut yy, ComputeBackend::CpuMulti);
                std::hint::black_box(&yy);
            });
            let tg = time(|| {
                std::hint::black_box(kgpu::axpy(2.5, &x, &mut y));
            });
            let ratio = tc.as_secs_f64() / tg.as_secs_f64();
            if ratio > 1.0 && axpy_cross.is_none() {
                axpy_cross = Some(n);
            }
            println!(
                "| `axpy` | {n} | {} | {} | {} | {ratio:.2}x |",
                us(ts),
                us(tc),
                us(tg)
            );

            // --- dot ---
            let ts = time(|| {
                std::hint::black_box(par::dot(&x, &x, ComputeBackend::Serial));
            });
            let tc = time(|| {
                std::hint::black_box(par::dot(&x, &x, ComputeBackend::CpuMulti));
            });
            let tg = time(|| {
                std::hint::black_box(kgpu::dot(&x, &x));
            });
            let ratio = tc.as_secs_f64() / tg.as_secs_f64();
            if ratio > 1.0 && dot_cross.is_none() {
                dot_cross = Some(n);
            }
            println!(
                "| `dot` | {n} | {} | {} | {} | {ratio:.2}x |",
                us(ts),
                us(tc),
                us(tg)
            );
        }

        println!("\n## Crossover summary (first size where GPU beats CpuMulti)\n");
        for (name, cross) in [
            ("spmv", spmv_cross),
            ("axpy", axpy_cross),
            ("dot", dot_cross),
        ] {
            match cross {
                Some(n) => println!("- `{name}`: n = {n}"),
                None => println!("- `{name}`: never within the swept range — keep it off auto-select"),
            }
        }
        println!();
    }

    #[cfg(all(feature = "gpu", not(target_os = "android")))]
    fn us(d: Duration) -> String {
        format!("{:.1} us", d.as_secs_f64() * 1e6)
    }
}
