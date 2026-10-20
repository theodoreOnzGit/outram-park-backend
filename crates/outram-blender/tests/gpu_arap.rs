// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors

//! ARAP with its local step on the GPU in `f32` (gh #1005) against the
//! default `f64` CPU path: end-result accuracy and end-to-end speed.
//!
//! Runs only with the default-on `gpu` feature, off Android, and SKIPs
//! cleanly when no adapter is found, so CI needs no GPU. The GPU kernel's
//! transcription check (GPU vs the `f32` mirror) and its precision cost
//! (mirror vs `f64`) are PETIR's: `petir/tests/wgsl_gpu.rs` and
//! `petir/tests/wgsl_validation.rs`.

#![cfg(all(feature = "gpu", not(target_os = "android")))]

use outram_blender::arap::{arap_deform, arap_deform_on_gpu, arap_energy, ArapRotations};
use outram_blender::gpu::probe;
use outram_blender::math::Vec3;
use outram_blender::mesh::VertexId;
use outram_blender::primitives;
use std::time::Instant;

/// End-to-end ARAP, `f32` GPU rotations against `f64` CPU rotations.
///
/// # Methodology
///
/// - **Mesh:** `grid(n, n, 4.0)` for `n = 16` and `n = 200` (289 and
///   40 401 vertices), the `x = -2` column pinned in place and the `x = +2`
///   column lifted by 1 in `z` (a bend), 10 iterations.
/// - **Compared:** the deformed positions, as the largest vertex distance
///   divided by the mesh size (4), and the ARAP energy of each result
///   ([`arap_energy`], `f64`). Everything except the rotations is `f64` in
///   both runs, so the difference is the `f32` rotations' cost, fed through
///   ten global solves.
/// - **Speed:** wall time of `arap_deform` (CPU, `f64`) against
///   `arap_deform_on_gpu` with a context probed beforehand (the probe, about
///   0.2 s here, is reported separately); best of 3, release build.
/// - **Pass criterion:** the GPU path ran (`ArapRotations::GpuF32`), the
///   result is finite, and the GPU result's energy is within the energy
///   change one more CPU iteration makes (the convergence scale: an `f32`
///   error below it is invisible to the method). The positions and timings
///   are reported, not gated.
///
/// # Results (2026-10-20, NVIDIA RTX A5000, Vulkan; CPU one core of the same
/// host)
///
/// | vertices | worst `|dp|`/size | energy CPU | energy GPU | one-iteration change | CPU s | GPU s | speedup |
/// |---|---|---|---|---|---|---|---|
/// | 289 | 2.6e-8 | 5.720827821e-2 | 5.720828841e-2 | 4.2e-3 | 0.0038 | 0.0030 | 1.3x |
/// | 40 401 | 9.2e-9 | 3.529683727e-2 | 3.529682772e-2 | 6.6e-4 | 0.604 | 0.169 | 3.6x |
///
/// Probe 0.077 s (a second probe in-process; the first measured 0.2 s).
/// Interpretation: worth opting in for large meshes with a reused context;
/// not for small ones. Also in the crate `CLAUDE.md` and on gh #1005.
#[test]
fn gpu_rotations_track_the_cpu_result_and_report_the_speed() {
    let Some(ctx) = probe() else {
        eprintln!("SKIP gpu_rotations_track_the_cpu_result_and_report_the_speed: no GPU adapter");
        return;
    };
    let t = Instant::now();
    let _again = probe();
    let probe_s = t.elapsed().as_secs_f64();
    for &n in &[16usize, 200] {
        let grid = primitives::grid(n, n, 4.0);
        let p = grid.positions();
        let handles: Vec<(VertexId, Vec3)> = (0..grid.vertex_count())
            .filter(|&i| p[i].x < -1.99 || p[i].x > 1.99)
            .map(|i| {
                let lift = if p[i].x > 0.0 { 1.0 } else { 0.0 };
                (VertexId(i), p[i].add(Vec3::new(0.0, 0.0, lift)))
            })
            .collect();
        let (mut t_cpu, mut t_gpu) = (f64::INFINITY, f64::INFINITY);
        let mut cpu = None;
        let mut gpu = None;
        for _ in 0..3 {
            let t = Instant::now();
            cpu = Some(arap_deform(&grid, &handles, 10).unwrap());
            t_cpu = t_cpu.min(t.elapsed().as_secs_f64());
            let t = Instant::now();
            gpu = Some(arap_deform_on_gpu(&ctx, &grid, &handles, 10).unwrap());
            t_gpu = t_gpu.min(t.elapsed().as_secs_f64());
        }
        let (cpu, gpu) = (cpu.unwrap(), gpu.unwrap());
        assert_eq!(gpu.rotations, ArapRotations::GpuF32, "the GPU path must have run");
        let (pc, pg) = (cpu.positions(), gpu.mesh.positions());
        let worst = pc
            .iter()
            .zip(&pg)
            .map(|(a, b)| a.sub(*b).length())
            .fold(0.0f64, f64::max)
            / 4.0;
        assert!(worst.is_finite());
        let e_cpu = arap_energy(&grid, &pc);
        let e_gpu = arap_energy(&grid, &pg);
        let e_next = arap_energy(&grid, &arap_deform(&grid, &handles, 11).unwrap().positions());
        let scale = (e_cpu - e_next).abs();
        println!(
            "grid {n} ({} verts): worst |dp|/size {worst:.3e}; energy cpu {e_cpu:.9e} gpu {e_gpu:.9e} \
             (|diff| {:.3e}, one-iteration change {scale:.3e}); time cpu {t_cpu:.4} s, gpu {t_gpu:.4} s \
             (x{:.2}), probe {probe_s:.3} s, adapter {:?}",
            grid.vertex_count(),
            (e_gpu - e_cpu).abs(),
            t_cpu / t_gpu,
            ctx.device.adapter_info().name
        );
        assert!(
            (e_gpu - e_cpu).abs() <= scale,
            "f32 rotations moved the energy by more than one iteration does"
        );
    }
}
