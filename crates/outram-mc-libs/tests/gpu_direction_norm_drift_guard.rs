// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors

//! # The direction-norm guard of the GPU rotation kernel (gh:#1499)
//!
//! ## What is guarded
//!
//! gh:#1497 found that the f64 CPU rotation kernels let a direction's norm
//! error `eps = |u|^2 - 1` grow without bound near the `w` pole: `rotate_angle`
//! multiplies `eps` by about `1 + a^2/b^2` (`a = sqrt(1 - mu^2)`,
//! `b = sqrt(1 - w^2)`). It was fixed on the CPU by
//! `physics::scatter::renormalise_if_drifted` (tolerance `sqrt(f64::EPSILON)`).
//!
//! The fused-event WGSL kernel (`gpu/shaders/batched_event.wgsl`) has one
//! rotation, `rotate_direction`, called by `two_body_with_mu` (elastic) and
//! `continuum_inelastic`; its CPU mirror (`gpu::batched_event`) has the same
//! function. `batched_flight.wgsl` (flight only, the collision is on the f64
//! CPU), `surface_distance.wgsl` and `xs_interp.wgsl` rotate nothing. The
//! direction is never rebuilt within a generation of the fused path, so the
//! exposure is the CPU's, in f32. The fix puts the same guard on the input and
//! the output of `rotate_direction` in both the WGSL and the mirror, and keeps
//! the old body callable as `rotate_direction_unguarded`.
//!
//! ## Prediction and tolerance, written 2026-11-12 BEFORE any run
//!
//! **Tolerance.** `sqrt(f32::EPSILON) = 2^-11.5 = 3.4526698e-4` on
//! `| |u|^2 - 1 |`, the f32 analogue of the CPU's `sqrt(f64::EPSILON)` (half
//! of the 23 stored mantissa bits lost, as the CPU's 2^-26 is half of 52).
//! Fixed from that definition, not from any run. (The brief that scheduled
//! this work quoted 2.44e-4; that is `2^-12 = sqrt(f32::EPSILON / 2)`, the
//! square root of the unit round-off, not of `EPSILON`. The two differ by
//! `sqrt(2)`, which does not matter to the guard's function; the definition
//! the CPU uses is kept.)
//!
//! **P1, unguarded f32 walks leave the tolerance.** 64 walks of 1e5 rotations
//! by `mu = 2 xi - 1` through the LCG. In f32 a fresh direction carries
//! `|eps| ~ 1e-7`; a gain of `1/b^2` lifts that past 3.45e-4 once
//! `b^2 < ~3e-4`, i.e. `|w| > 1 - 1.5e-4`, which an isotropic walk visits with
//! probability ~1.5e-4 per step, ~15 times per walk. Prediction: every one of
//! the 64 unguarded walks leaves the tolerance, on the GPU and on the mirror.
//! Whether they go non-finite is not predicted and not asserted.
//!
//! **P2, guarded walks stay finite and unit.** The same walks through the
//! guarded kernel: every step finite and within the tolerance, GPU and mirror.
//!
//! **P3, how often the guard fires.** Of order 1e-4 per rotation (a decade
//! either side: 1e-5 to 1e-3), from the pole-visit estimate in P1.
//!
//! **P4, bit-identity where the guard does not fire.** At every step the
//! guarded and unguarded outputs (same input, same seed) differ in bits if and
//! only if the input or the unguarded output is outside the tolerance, on the
//! GPU and the mirror. So on healthy directions the kernel is the old kernel.
//!
//! **P5, one step, GPU vs mirror.** For inputs with `|w| <= 0.99`, healthy or
//! drifted by up to 1e-2, the guarded GPU and mirror outputs agree per
//! component within 1e-5: the shader's `sin`/`cos` and PETIR's differ by a few
//! ulp (`4 ulp ~ 4.8e-7`), amplified by `a / b <= 1 / sqrt(1 - 0.99^2) = 7.1`,
//! about 3.4e-6. Near-pole and grossly drifted inputs (including the vector
//! recorded on gh:#1497, `|u| = 2.9`) are judged by finiteness and the
//! tolerance only, since there the gain amplifies ulp differences without
//! bound.
//!
//! **P6, a graphite event walk.** A 100 cm sphere of `C0` (0.0853 /b-cm), 4096
//! neutrons at 2 MeV, run event by event through the mirror until all are dead
//! or 3000 events: after every event every live direction is finite and within
//! the tolerance. The same batch through the GPU generation: every final
//! direction finite and within the tolerance.
//!
//! **P7, existing pins.** `delta_tracking_bit_identity` and
//! `stats_move_fingerprints` run only f64 CPU code that this change does not
//! touch: unchanged. `gpu_fused_event_per_collision_banking` (Godiva, f32):
//! the guard fires only near the pole, ~1e-4 per rotation, on a few rotations
//! per history; its G1-G4 gates still pass, and its printed figures move at
//! most by the handful of histories the guard touches. `gpu_csg_parity` is in
//! `outram-blender`, which this change does not touch: unchanged.
//!
//! ## Results (2026-11-12, Rust 1.99.1, release; GPU = Mesa llvmpipe
//! (LLVM 20.1.2, 256 bits), software Vulkan, so the WGSL really ran on CPU
//! cores)
//!
//! | prediction | mirror | GPU |
//! |---|---|---|
//! | P1 unguarded walks leave the tolerance | 64/64, worst \|eps\| 0.242, none non-finite | 64/64, worst 0.231, none non-finite |
//! | P2 guarded walks finite and unit | 0/64 left, worst \|eps\| 3.4523e-4 | 0/64 left (with the allowance below), worst 3.4528e-4 |
//! | P3 guard fire rate | 980 / 6.4e6 = 1.53e-4 | 981 / 6.4e6 = 1.53e-4 |
//! | P4 bits differ iff the guard fires | 0 mismatches in 12.8e6 steps | 0 mismatches outside the allowance; 2 (guarded) + 19 (unguarded) steps within it |
//! | P5 one step, \|w\| <= 0.99 | | worst \|GPU - mirror\| 5.96e-8 (bound 1e-5); 80/95 cases bit-identical; near-pole and \|u\| = 2.9 cases finite and unit |
//! | P6 graphite event walk | 3000 events (4 of 4096 still alive), every direction within the tolerance after every event, worst 3.4517e-4 | 3000 events, every final direction within it, worst 3.378e-4 |
//!
//! P1 and P3 held as predicted: the fire rate is the pole-visit estimate
//! (1.5e-4) to two figures. The unguarded f32 walks did not go non-finite
//! (unlike the f64 CPU case of gh:#1497) but held `|u|^2` up to 1.24 off unit
//! length, and sat outside the tolerance on 83 % of their steps.
//!
//! **One correction to the protocol, made after the first GPU run and stated
//! here rather than hidden.** As first written, P2 and P4 judged the GPU's
//! boundary exactly, as the mirror's. On the GPU one of 64 guarded walks
//! reached `|eps|` = 3.4528e-4, 1.3e-8 past the tolerance, and 2 + 19 steps
//! had bits that differed (or not) against what the probe's own drift said.
//! All of them sat within a few `f32::EPSILON` of the tolerance: the guard in
//! the kernel and the probe re-evaluate `|u|^2 - 1` of the same vector at
//! different inlined sites, and WGSL lets the driver contract `a * b + c` to a
//! fused multiply-add differently at each (routing both through one
//! `direction_norm_drift` function changed nothing, so the contraction is per
//! inlined site). The GPU probe therefore counts a step whose drift is within
//! [`GPU_EVAL_ALLOWANCE`] = `4 f32::EPSILON` of the tolerance apart, as
//! `boundary`, and judges "left the tolerance" at the tolerance plus that
//! allowance. The guard's tolerance itself was not moved, and the mirror
//! (Rust does not contract) is still judged exactly and passes exactly.
//!
//! **Existing pins, before and after (P7).** Run on the parent commit and on
//! this change with the same target directory, every printed figure of
//! `gpu_fused_event_per_collision_banking`, `gpu_kinematics_invariants`,
//! `delta_tracking_bit_identity`, `stats_move_fingerprints`,
//! `direction_norm_drift_guard` and the `gpu::batched*` lib tests is
//! identical (the fused-event Godiva mirror and GPU `k` to all five decimals,
//! G3's 23412 records), so on Godiva the guard changed no history.
//! `outram-blender`'s `gpu_csg_parity` passes (code untouched).
//! `gpu_fused_event_per_collision_banking::g2_g4_fused_gpu_banks_per_collision`
//! FAILS identically before and after (fused GPU `k = 1.78183 +/- 0.05545`):
//! a pre-existing defect unrelated to the guard, filed as gh:#1506.

#![cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]

use outram_mc_libs::gpu::batched_event::{
    advance_event_cpu_mirror, advance_generation_gpu, rotate_direction_f32,
    rotate_direction_f32_unguarded, shader_source, EventBatch, EventSphere, EventTablesF32,
    DIRECTION_NORM_DRIFT_TOL_F32 as TOL, FISS_NONE,
};
use outram_mc_libs::gpu::collision_grid::CollisionTables;
use outram_mc_libs::gpu::{probe, GpuContext};
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use petir::wgsl::mirror_lcg::lcg_next;

/// Probe entry points appended to the kernel source (they reuse its `xs`,
/// binding 0, and `fstate`, binding 2).
///
/// `petir_norm_walk_probe`: per walk, 5 inputs `(ux, uy, uz, steps,
/// unguarded)`; 9 outputs `(ux, uy, uz, max |eps|, fires, mismatched,
/// first non-finite step or -1, steps outside the tolerance, boundary
/// steps)`. At every step
/// the guarded and the unguarded rotation are both evaluated from the same
/// input and seed; `fires` counts steps where their bits differ and
/// `mismatched` counts steps where that disagrees with the guard condition.
///
/// `petir_rotate_probe`: per case, 4 inputs `(ux, uy, uz, mu)`; 6 outputs,
/// the guarded then the unguarded direction.
const PROBES: &str = r#"
// See `GPU_EVAL_ALLOWANCE` on the Rust side.
const PROBE_EVAL_ALLOWANCE: f32 = 4.0 * 1.1920929e-7;

fn probe_drift(u: vec3<f32>) -> f32 {
    return direction_norm_drift(u);
}

fn probe_seed(i: u32) -> vec2<u32> {
    return vec2<u32>(i * 2654435761u + 0x1499u, 0x20261112u ^ i);
}

@compute @workgroup_size(64)
fn petir_norm_walk_probe(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    let n = arrayLength(&xs) / 5u;
    if (i >= n) { return; }
    let b = 5u * i;
    var u = vec3<f32>(xs[b], xs[b + 1u], xs[b + 2u]);
    let steps = u32(xs[b + 3u]);
    let unguarded = xs[b + 4u] != 0.0;
    var seed = probe_seed(i);
    var max_eps: f32 = 0.0;
    var fires: u32 = 0u;
    var mismatched: u32 = 0u;
    var outside: u32 = 0u;
    var first_bad: f32 = -1.0;
    var boundary: u32 = 0u;
    for (var s: u32 = 0u; s < steps; s = s + 1u) {
        let r = petir_lcg_next(seed);
        seed = r.xy;
        let mu = 2.0 * bitcast<f32>(r.z) - 1.0;
        let g = rotate_direction(u, mu, seed);
        let raw = rotate_direction_unguarded(u, mu, seed);
        let differ = any(bitcast<vec3<u32>>(g.dir) != bitcast<vec3<u32>>(raw.dir));
        let d_in = probe_drift(u);
        let d_out = probe_drift(raw.dir);
        let cond = d_in > DIRECTION_NORM_DRIFT_TOL || d_out > DIRECTION_NORM_DRIFT_TOL;
        let near = abs(d_in - DIRECTION_NORM_DRIFT_TOL) <= PROBE_EVAL_ALLOWANCE
            || abs(d_out - DIRECTION_NORM_DRIFT_TOL) <= PROBE_EVAL_ALLOWANCE;
        if (differ) { fires = fires + 1u; }
        if (differ != cond) {
            if (near) { boundary = boundary + 1u; } else { mismatched = mismatched + 1u; }
        }
        if (unguarded) { u = raw.dir; } else { u = g.dir; }
        seed = g.seed;
        let e = probe_drift(u);
        if (e <= 1.0e30) {
            max_eps = max(max_eps, e);
            if (e > DIRECTION_NORM_DRIFT_TOL + PROBE_EVAL_ALLOWANCE) { outside = outside + 1u; }
        } else {
            outside = outside + 1u;
            if (first_bad < 0.0) { first_bad = f32(s); }
        }
    }
    let o = 9u * i;
    fstate[o + 8u] = f32(boundary);
    fstate[o] = u.x;
    fstate[o + 1u] = u.y;
    fstate[o + 2u] = u.z;
    fstate[o + 3u] = max_eps;
    fstate[o + 4u] = f32(fires);
    fstate[o + 5u] = f32(mismatched);
    fstate[o + 6u] = first_bad;
    fstate[o + 7u] = f32(outside);
}

@compute @workgroup_size(64)
fn petir_rotate_probe(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    let n = arrayLength(&xs) / 4u;
    if (i >= n) { return; }
    let b = 4u * i;
    let u = vec3<f32>(xs[b], xs[b + 1u], xs[b + 2u]);
    let mu = xs[b + 3u];
    let g = rotate_direction(u, mu, probe_seed(i));
    let raw = rotate_direction_unguarded(u, mu, probe_seed(i));
    let o = 6u * i;
    fstate[o] = g.dir.x;
    fstate[o + 1u] = g.dir.y;
    fstate[o + 2u] = g.dir.z;
    fstate[o + 3u] = raw.dir.x;
    fstate[o + 4u] = raw.dir.y;
    fstate[o + 5u] = raw.dir.z;
}
"#;

fn probe_seed(i: u32) -> (u32, u32) {
    (i.wrapping_mul(2_654_435_761).wrapping_add(0x1499), 0x2026_1112 ^ i)
}

fn drift(u: [f32; 3]) -> f32 {
    (u[0] * u[0] + u[1] * u[1] + u[2] * u[2] - 1.0).abs()
}

/// Dispatch `entry` over `input` (f32 words), returning `out_per * n` words.
fn dispatch(gpu: &GpuContext, entry: &str, input: &[f32], n: usize, out_per: usize) -> Vec<f32> {
    use wgpu::util::DeviceExt;
    let mut source = shader_source();
    source.push_str(PROBES);
    let module = gpu.device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("batched_event + norm-guard probes"),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    });
    let bytes: Vec<u8> = input.iter().flat_map(|v| v.to_le_bytes()).collect();
    let xs_buf = gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("xs"),
        contents: &bytes,
        usage: wgpu::BufferUsages::STORAGE,
    });
    let out_size = (n * out_per * 4) as u64;
    let fstate_buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("fstate"),
        size: out_size,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let read_buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: out_size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let pipeline = gpu.device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some(entry),
        layout: None,
        module: &module,
        entry_point: Some(entry),
        compilation_options: Default::default(),
        cache: None,
    });
    let bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: xs_buf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 2, resource: fstate_buf.as_entire_binding() },
        ],
    });
    let mut encoder =
        gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: None,
            timestamp_writes: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(n.div_ceil(64) as u32, 1, 1);
    }
    encoder.copy_buffer_to_buffer(&fstate_buf, 0, &read_buf, 0, out_size);
    gpu.queue.submit(Some(encoder.finish()));
    let slice = read_buf.slice(..);
    slice.map_async(wgpu::MapMode::Read, |_| {});
    gpu.device.poll(wgpu::PollType::wait_indefinitely()).expect("device poll");
    let mapped = slice.get_mapped_range().expect("mapped range");
    let out: Vec<f32> = mapped
        .chunks_exact(4)
        .filter_map(|c| <[u8; 4]>::try_from(c).ok())
        .map(f32::from_le_bytes)
        .collect();
    drop(mapped);
    read_buf.unmap();
    out
}

/// How far apart two evaluations of `|u|^2 - 1` of the SAME f32 vector may
/// land on the GPU: `4 f32::EPSILON` (4.8e-7). WGSL lets the driver contract
/// `a * b + c` into a fused multiply-add, and it may do so differently at
/// each inlined call site, so the guard inside the kernel and the probe that
/// re-checks it can disagree on a vector whose drift sits within that of the
/// tolerance. The bound is the rounding of three products, two sums and one
/// difference of terms no larger than 1, each at most half an ulp of 1
/// (`3 eps` in all), rounded up to `4 eps`. Used only to judge the GPU
/// probe's bookkeeping (counts `boundary` steps apart from `mismatched`, and
/// `outside` the tolerance plus this); the guard's own tolerance is
/// unchanged, and the mirror (no contraction in Rust) is judged exactly.
const GPU_EVAL_ALLOWANCE: f32 = 4.0 * f32::EPSILON;

const WALKS: usize = 64;
const STEPS: u32 = 100_000;

/// `n` unit vectors on a Fibonacci spiral over the sphere.
fn spiral(n: usize) -> Vec<[f32; 3]> {
    (0..n)
        .map(|k| {
            let w = 1.0 - 2.0 * (k as f32 + 0.5) / n as f32;
            let s = (1.0 - w * w).sqrt();
            let phi = 2.399_963_1_f32 * k as f32;
            [s * phi.cos(), s * phi.sin(), w]
        })
        .collect()
}

/// One walk's summary, as the probe reports it.
#[derive(Debug, Clone, Copy)]
struct Walk {
    max_eps: f32,
    fires: u32,
    mismatched: u32,
    first_bad: Option<u32>,
    outside: u32,
    /// GPU only: steps where `differ != cond` with a drift within
    /// [`GPU_EVAL_ALLOWANCE`] of the tolerance (always 0 on the mirror, which
    /// judges the boundary exactly).
    boundary: u32,
}

/// The mirror's walk: the same loop as `petir_norm_walk_probe`.
fn mirror_walk(i: u32, start: [f32; 3], steps: u32, unguarded: bool) -> Walk {
    let (mut lo, mut hi) = probe_seed(i);
    let mut u = start;
    let mut w =
        Walk { max_eps: 0.0, fires: 0, mismatched: 0, first_bad: None, outside: 0, boundary: 0 };
    for s in 0..steps {
        let (l, h, xi) = lcg_next(lo, hi);
        lo = l;
        hi = h;
        let mu = 2.0 * xi - 1.0;
        let (g, gl, gh) = rotate_direction_f32(u, mu, lo, hi);
        let (raw, _, _) = rotate_direction_f32_unguarded(u, mu, lo, hi);
        let differ = g.iter().zip(raw.iter()).any(|(a, b)| a.to_bits() != b.to_bits());
        let cond = drift(u) > TOL || drift(raw) > TOL;
        w.fires += differ as u32;
        w.mismatched += (differ != cond) as u32;
        u = if unguarded { raw } else { g };
        lo = gl;
        hi = gh;
        let e = drift(u);
        if e <= 1.0e30 {
            w.max_eps = w.max_eps.max(e);
            w.outside += (e > TOL) as u32;
        } else {
            w.outside += 1;
            w.first_bad.get_or_insert(s);
        }
    }
    w
}

fn gpu_walks(gpu: &GpuContext, unguarded: bool) -> Vec<Walk> {
    let mut input = Vec::with_capacity(WALKS * 5);
    for s in spiral(WALKS) {
        input.extend_from_slice(&[s[0], s[1], s[2], STEPS as f32, unguarded as u32 as f32]);
    }
    let out = dispatch(gpu, "petir_norm_walk_probe", &input, WALKS, 9);
    out.chunks_exact(9)
        .map(|o| Walk {
            max_eps: o[3],
            fires: o[4] as u32,
            mismatched: o[5] as u32,
            first_bad: if o[6] < 0.0 { None } else { Some(o[6] as u32) },
            outside: o[7] as u32,
            boundary: o[8] as u32,
        })
        .collect()
}

fn mirror_walks(unguarded: bool) -> Vec<Walk> {
    spiral(WALKS)
        .into_iter()
        .enumerate()
        .map(|(i, s)| mirror_walk(i as u32, s, STEPS, unguarded))
        .collect()
}

/// Print a summary; returns (walks that left the tolerance, walks that went
/// non-finite, guard fires, mismatches, worst finite |eps|).
fn summarise(name: &str, walks: &[Walk]) -> (usize, usize, u64, u64, f32) {
    let left = walks.iter().filter(|w| w.outside > 0).count();
    let nonfinite = walks.iter().filter(|w| w.first_bad.is_some()).count();
    let fires: u64 = walks.iter().map(|w| w.fires as u64).sum();
    let mism: u64 = walks.iter().map(|w| w.mismatched as u64).sum();
    let worst = walks.iter().map(|w| w.max_eps).fold(0.0_f32, f32::max);
    let first = walks.iter().filter_map(|w| w.first_bad).min();
    let bnd: u64 = walks.iter().map(|w| w.boundary as u64).sum();
    let total = walks.len() as u64 * STEPS as u64;
    println!(
        "{name}: {left}/{} walks left the tolerance, {nonfinite} went non-finite \
         (earliest step {first:?}); guard fired on {fires} of {total} rotations ({:.3e}); \
         mismatched {mism} (+{bnd} within the evaluation allowance of the tolerance); \
         worst finite |eps| {worst:e}",
        walks.len(),
        fires as f64 / total as f64,
    );
    (left, nonfinite, fires, mism, worst)
}

/// The tolerance is `sqrt(f32::EPSILON)` and the WGSL carries the same value.
#[test]
fn the_f32_tolerance_is_sqrt_of_f32_epsilon_on_both_sides() {
    assert_eq!(TOL, f32::EPSILON.sqrt());
    let src = shader_source();
    let line = src
        .lines()
        .find(|l| l.trim_start().starts_with("const DIRECTION_NORM_DRIFT_TOL"))
        .expect("the WGSL declares DIRECTION_NORM_DRIFT_TOL");
    let lit = line
        .split('=')
        .nth(1)
        .and_then(|r| r.split(';').next())
        .map(str::trim)
        .unwrap_or("");
    let v: f32 = lit.parse().expect("a plain f32 literal");
    assert_eq!(v.to_bits(), TOL.to_bits(), "WGSL literal {lit} is not sqrt(f32::EPSILON)");
}

/// The GPU probe's evaluation allowance matches [`GPU_EVAL_ALLOWANCE`].
#[test]
fn the_probe_evaluation_allowance_is_four_f32_epsilon() {
    let lit = "1.1920929e-7".parse::<f32>().unwrap_or(0.0);
    assert_eq!(lit.to_bits(), f32::EPSILON.to_bits());
    assert!(PROBES.contains("const PROBE_EVAL_ALLOWANCE: f32 = 4.0 * 1.1920929e-7;"));
    assert_eq!(GPU_EVAL_ALLOWANCE, 4.0 * lit);
}

/// P1-P4 on the CPU mirror (runs everywhere, no adapter needed).
#[test]
fn mirror_walks_stay_unit_with_the_guard_and_drift_without_it() {
    let (left_g, nf_g, fires, mism_g, worst_g) = summarise("mirror guarded", &mirror_walks(false));
    let (left_u, _, _, mism_u, _) = summarise("mirror unguarded", &mirror_walks(true));
    assert_eq!(nf_g, 0, "P2: a guarded walk went non-finite");
    assert_eq!(left_g, 0, "P2: a guarded walk left the tolerance (worst {worst_g:e})");
    assert_eq!(mism_g + mism_u, 0, "P4: outputs differ other than where the guard fires");
    assert!(left_u > 0, "P1 (mechanism): no unguarded walk drifted out of the tolerance");
    assert!(fires > 0);
}

/// P1-P4 on the GPU (SKIP without an adapter).
#[test]
fn gpu_walks_stay_unit_with_the_guard_and_drift_without_it() {
    let Some(gpu) = probe() else {
        eprintln!("SKIP gpu_walks_stay_unit_with_the_guard_and_drift_without_it: no GPU adapter");
        return;
    };
    println!("adapter: {}", gpu.info.name);
    let (left_g, nf_g, fires, mism_g, worst_g) =
        summarise("GPU guarded", &gpu_walks(&gpu, false));
    let (left_u, _, _, mism_u, _) = summarise("GPU unguarded", &gpu_walks(&gpu, true));
    assert_eq!(nf_g, 0, "P2: a guarded walk went non-finite");
    assert_eq!(left_g, 0, "P2: a guarded walk left the tolerance (worst {worst_g:e})");
    assert_eq!(mism_g + mism_u, 0, "P4: outputs differ other than where the guard fires");
    assert!(left_u > 0, "P1 (mechanism): no unguarded walk drifted out of the tolerance");
    assert!(fires > 0);
}

/// One-step cases for P5: healthy and drifted inputs with `|w| <= 0.99`
/// (the first `n_regular`), then near-pole and grossly drifted ones.
fn step_cases() -> (Vec<[f32; 4]>, usize) {
    let mut v = Vec::new();
    for &w in &[0.0_f32, 0.3, -0.6, 0.9, -0.99] {
        for &k in &[1.0_f32, 1.0 + 1e-4, 1.0 + 1e-3, 1.0 - 1e-2] {
            for &mu in &[-0.9_f32, -0.2, 0.4, 0.95] {
                let s = (1.0 - w * w).sqrt();
                v.push([0.6 * s * k, 0.8 * s * k, w * k, mu]);
            }
        }
    }
    let n_regular = v.len();
    for &(u, k) in &[
        ([0.0_f32, 0.0, 1.0], 1.0_f32),
        ([2.4e-4, 0.0, -1.0], 1.0),
        ([1.0e-3, 0.0, 0.999_999_5], 1.0 + 3e-4),
        ([1.0e-3, 0.0, 0.999_999_5], 1.0 + 1e-2),
        // The vector recorded on gh:#1497 (|u| = 2.9), in f32.
        ([1.441_102_2, 1.230_277_3, -2.224_462_5], 1.0),
    ] {
        for &mu in &[-0.5_f32, 0.0, 0.454_370_95] {
            v.push([u[0] * k, u[1] * k, u[2] * k, mu]);
        }
    }
    (v, n_regular)
}

/// P5: one rotation, GPU against the mirror (SKIP without an adapter).
#[test]
fn gpu_and_mirror_agree_on_one_guarded_rotation() {
    let Some(gpu) = probe() else {
        eprintln!("SKIP gpu_and_mirror_agree_on_one_guarded_rotation: no GPU adapter");
        return;
    };
    let (cases, n_regular) = step_cases();
    let input: Vec<f32> = cases.iter().flatten().copied().collect();
    let out = dispatch(&gpu, "petir_rotate_probe", &input, cases.len(), 6);
    let mut worst = 0.0_f32;
    let mut bit_identical = 0usize;
    for (i, c) in cases.iter().enumerate() {
        let (lo, hi) = probe_seed(i as u32);
        let (m, _, _) = rotate_direction_f32([c[0], c[1], c[2]], c[3], lo, hi);
        let g = [out[6 * i], out[6 * i + 1], out[6 * i + 2]];
        assert!(g.iter().all(|x| x.is_finite()), "case {i} {c:?}: GPU guarded output {g:?}");
        assert!(drift(g) <= TOL, "case {i} {c:?}: GPU |eps| {:e}", drift(g));
        assert!(drift(m) <= TOL, "case {i} {c:?}: mirror |eps| {:e}", drift(m));
        if g.iter().zip(m.iter()).all(|(a, b)| a.to_bits() == b.to_bits()) {
            bit_identical += 1;
        }
        if i < n_regular {
            let d = g.iter().zip(m.iter()).map(|(a, b)| (a - b).abs()).fold(0.0, f32::max);
            worst = worst.max(d);
            assert!(d <= 1e-5, "case {i} {c:?}: GPU {g:?} vs mirror {m:?}");
        }
    }
    println!(
        "P5: {bit_identical}/{} GPU outputs bit-identical to the mirror; worst |GPU - mirror| \
         over the {n_regular} regular cases {worst:e}",
        cases.len()
    );
}

/// Graphite, 1.7 g/cm3: 0.0853 atoms/b-cm.
const C_DENSITY: f64 = 0.0853;
const G_RADIUS: f32 = 100.0;
const G_N: usize = 4096;
const G_EVENTS: usize = 3000;

fn graphite_tables() -> EventTablesF32 {
    let nuclides = vec![Nuclide::from_core("C0").expect("embedded carbon")];
    let material = Material {
        id: 1,
        name: "graphite".into(),
        temperature: 293.6,
        components: vec![NuclideComponent { nuclide_idx: 0, atom_density: C_DENSITY }],
    };
    EventTablesF32::from_collision_tables(&CollisionTables::build(
        &material, &nuclides, 1e-3, 2e7, 4096,
    ))
}

/// `G_N` neutrons at the centre at 2 MeV, directions on a spiral, seeds from
/// [`probe_seed`].
fn graphite_batch() -> EventBatch {
    let mut b = EventBatch {
        pos: vec![0.0; 3 * G_N],
        dir: Vec::with_capacity(3 * G_N),
        energy: vec![2.0e6; G_N],
        seed_lo: Vec::with_capacity(G_N),
        seed_hi: Vec::with_capacity(G_N),
        alive: vec![1; G_N],
        fiss_nuc: vec![FISS_NONE; G_N],
        production: vec![0.0; G_N],
    };
    for (i, d) in spiral(G_N).into_iter().enumerate() {
        b.dir.extend_from_slice(&d);
        let (lo, hi) = probe_seed(i as u32);
        b.seed_lo.push(lo);
        b.seed_hi.push(hi);
    }
    b
}

/// (worst finite |eps|, number of directions non-finite or outside the
/// tolerance) over every direction in the batch, live or dead.
fn worst_dir_drift(b: &EventBatch) -> (f32, usize) {
    let mut worst = 0.0_f32;
    let mut bad = 0usize;
    for d in b.dir.chunks_exact(3) {
        let e = drift([d[0], d[1], d[2]]);
        if !(e <= TOL) {
            bad += 1;
        }
        if e.is_finite() {
            worst = worst.max(e);
        }
    }
    (worst, bad)
}

/// P6 on the mirror: every direction checked after every event.
#[test]
fn a_graphite_event_walk_keeps_every_direction_unit_on_the_mirror() {
    let t = graphite_tables();
    let mut b = graphite_batch();
    let sphere = EventSphere { x0: 0.0, y0: 0.0, z0: 0.0, r: G_RADIUS };
    let mut events = 0usize;
    let mut worst = 0.0_f32;
    while b.n_alive() > 0 && events < G_EVENTS {
        advance_event_cpu_mirror(&t, &mut b, sphere);
        events += 1;
        let (w, bad) = worst_dir_drift(&b);
        worst = worst.max(w);
        assert_eq!(bad, 0, "event {events}: {bad} directions non-finite or outside the tolerance");
    }
    println!(
        "P6 mirror: {events} events, {} still alive, worst |eps| over all events {worst:e}",
        b.n_alive()
    );
    assert!(events > 100, "the walk must be long: {events} events");
}

/// P6 on the GPU: the whole generation resident, final directions checked.
#[test]
fn a_graphite_event_walk_keeps_every_direction_unit_on_the_gpu() {
    let Some(gpu) = probe() else {
        eprintln!(
            "SKIP a_graphite_event_walk_keeps_every_direction_unit_on_the_gpu: no GPU adapter"
        );
        return;
    };
    let t = graphite_tables();
    let mut b = graphite_batch();
    let sphere = EventSphere { x0: 0.0, y0: 0.0, z0: 0.0, r: G_RADIUS };
    let events = advance_generation_gpu(&gpu, &t, &mut b, sphere, G_EVENTS);
    let (worst, bad) = worst_dir_drift(&b);
    println!(
        "P6 GPU ({}): {events} events, {} still alive, worst final |eps| {worst:e}, {bad} bad",
        gpu.info.name,
        b.n_alive()
    );
    assert_eq!(bad, 0, "{bad} final directions non-finite or outside the tolerance");
    assert!(events > 100, "the walk must be long: {events} events");
}
