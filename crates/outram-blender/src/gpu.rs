// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Headless GPU compute via the `wgpu` crate (Apache-2.0/MIT, GPL-3.0-compatible).
// No published algorithm — a WGSL compute-shader dispatch harness with a CPU
// reference fallback. Target-gated off Android per the workspace portability rule.
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.

//! GPU compute (headless, target-gated OFF Android; no cargo feature).
//!
//! Headless GPU acceleration via [`wgpu`] for the *embarrassingly parallel*
//! parts of mesh authoring — per-vertex / per-face kernels, subdivision
//! evaluation, deformation. **No window or surface** is created; this is
//! compute-only (WGSL compute shaders).
//!
//! ## The wired demonstrator kernel
//!
//! This module now carries **one real, end-to-end kernel**: applying an
//! [`crate::transform::Affine3`] to every vertex of a mesh in parallel via a
//! WGSL compute shader ([`crate::gpu::transform_vertices_gpu`]). The identical computation
//! on the CPU is [`crate::transform::Affine3::transform_points`], which is the
//! reference the GPU result is validated against (see the tests below). This
//! kernel is deliberately the simplest embarrassingly-parallel mesh operation —
//! it exists to prove the GPU compute path is live and CPU-checked, not because
//! an affine transform needs a GPU. Heavier per-vertex kernels (deformation,
//! subdivision) follow the same buffer/pipeline pattern.
//!
//! ## Non-negotiable contract for using this module
//!
//! 1. **Target-gated, not feature-gated.** This module is compiled
//!    **unconditionally on every desktop target** — there is no `gpu` cargo
//!    feature to enable — so the GPU path is always available and used as far as
//!    possible. It is present on all targets **except Android**
//!    (`target_os = "android"`), where the workspace Android rule forbids GPU
//!    deps in the library build; there the GPU attempt is compiled out and the
//!    CPU path runs.
//! 2. **Runtime CPU fallback is mandatory.** Even where wgpu is compiled, at
//!    runtime there may be **no usable GPU adapter** (headless servers, VMs) or
//!    a submission may fail mid-flight. Callers MUST treat [`crate::gpu::probe`] returning
//!    `None`, and [`crate::gpu::try_transform_vertices_gpu`] returning `Err`, as "run the
//!    CPU path", never as a hard error. [`crate::transform::Affine3::transform_points_best_effort`]
//!    wraps exactly this: try GPU, fall back to CPU, always return a result.
//! 3. **CPU is the trusted / reference path.** GPU float reduction order will
//!    not bit-match the CPU (`f64`, [`crate::transform`]) result, so anything
//!    that feeds V&V or a solver stays CPU-deterministic. GPU is *acceleration
//!    only*, and [`crate::gpu::transform_vertices_gpu`] returns `f32`-precision results.

use crate::math::Vec3;
use crate::transform::Affine3;
use core::future::Future;
use core::task::{Context, Poll, Waker};
use std::sync::{Arc, Mutex};

/// Re-export of the GPU backend so callers can build pipelines without adding
/// their own `wgpu` dependency. Present on every desktop target (absent only on
/// Android, where this whole module is compiled out).
pub use wgpu;

/// A **recoverable** GPU execution failure from [`try_transform_vertices_gpu`].
///
/// Every variant means the same thing to a caller: the GPU attempt did not
/// complete, so fall back to the CPU reference path
/// ([`Affine3::transform_points`]). The GPU is acceleration only and never the
/// source of truth, so a `GpuError` is a routine "use the CPU" signal, not a
/// fatal condition — [`Affine3::transform_points_best_effort`] does this
/// automatically. This deliberately does **not** cover the "no adapter at all"
/// case, which surfaces earlier as [`crate::gpu::probe`] returning `None`.
#[derive(Debug, thiserror::Error)]
pub enum GpuError {
    /// Polling the device to drive the readback failed (e.g. device lost).
    #[error("GPU device poll failed while awaiting readback: {0}")]
    Poll(String),
    /// The staging buffer could not be mapped back to the CPU.
    #[error("GPU buffer map failed: {0}")]
    Map(String),
    /// The buffer-map callback never fired despite a wait-indefinitely poll.
    #[error("GPU buffer map callback never fired after wait-indefinitely poll")]
    MapCallbackMissing,
}

/// A live GPU compute context: a headless [`wgpu::Device`] and its
/// [`wgpu::Queue`].
///
/// Obtain one from [`probe`]. A `GpuContext` owns its device and queue by value
/// (no borrows — workspace no-lifetimes rule) and is `!Clone`; share it across
/// threads behind an `Arc` if needed. Dropping it releases the GPU resources.
#[derive(Debug)]
pub struct GpuContext {
    /// The logical GPU device — used to create buffers, shaders, and pipelines.
    pub device: wgpu::Device,
    /// The command queue — used to upload buffers and submit compute work.
    pub queue: wgpu::Queue,
}

/// Probe for a usable headless GPU compute adapter.
///
/// Creates a [`wgpu::Instance`] over all backends enabled for this platform,
/// requests an adapter with **no surface** (headless compute — `power_preference
/// = None`, no `compatible_surface`), then requests a device + queue with the
/// downlevel default limits (the broadest-compatibility profile, so software
/// adapters like Lavapipe/WARP also qualify). The blocking wait on wgpu's async
/// requests is done with a tiny in-crate executor (`block_on`) so this crate
/// pulls in no async-runtime dependency.
///
/// Returns `Some(GpuContext)` when a headless compute device is available, or
/// `None` when the caller must fall back to the CPU path
/// ([`crate::transform::Affine3::transform_points`]). `None` is a normal,
/// expected outcome on headless CI and the Android emulator — it is **not** an
/// error.
pub fn probe() -> Option<GpuContext> {
    let instance = wgpu::Instance::default();

    let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::None,
        force_fallback_adapter: false,
        compatible_surface: None,
    }))
    .ok()?;

    let (device, queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("outram-blender headless compute"),
        required_features: wgpu::Features::empty(),
        // Downlevel defaults keep the widest hardware/software compatibility;
        // the affine kernel needs nothing beyond a basic storage-buffer compute
        // pipeline.
        required_limits: wgpu::Limits::downlevel_defaults(),
        ..Default::default()
    }))
    .ok()?;

    Some(GpuContext { device, queue })
}

/// WGSL compute shader: apply an affine transform `M p + t` to every vertex.
///
/// Positions are a flat `array<f32>` of `3 * N` components (x,y,z per vertex).
/// The affine is a uniform of four `vec4<f32>` rows (each `vec3` padded to 16
/// bytes for std140 uniform layout); only the `.xyz` lanes are used. One
/// invocation transforms one vertex. This is the GPU transcription of
/// [`Affine3::transform_point`].
const AFFINE_TRANSFORM_WGSL: &str = r#"
struct Affine {
    row0: vec4<f32>,
    row1: vec4<f32>,
    row2: vec4<f32>,
    translation: vec4<f32>,
};

@group(0) @binding(0) var<uniform> affine: Affine;
@group(0) @binding(1) var<storage, read> input_pos: array<f32>;
@group(0) @binding(2) var<storage, read_write> output_pos: array<f32>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    let n = arrayLength(&input_pos) / 3u;
    if (i >= n) {
        return;
    }
    let base = i * 3u;
    let p = vec3<f32>(input_pos[base], input_pos[base + 1u], input_pos[base + 2u]);
    let r = vec3<f32>(
        dot(affine.row0.xyz, p),
        dot(affine.row1.xyz, p),
        dot(affine.row2.xyz, p),
    ) + affine.translation.xyz;
    output_pos[base] = r.x;
    output_pos[base + 1u] = r.y;
    output_pos[base + 2u] = r.z;
}
"#;

/// Number of invocations per workgroup — must match `@workgroup_size` in the
/// WGSL above.
const WORKGROUP_SIZE: u32 = 64;

/// Apply `affine` to every position in `positions` on the GPU, returning the
/// transformed positions in the same order — the **fallible** entry point.
///
/// This is the demonstrator GPU kernel. It uploads the positions as an `f32`
/// storage buffer, dispatches `AFFINE_TRANSFORM_WGSL` one invocation per
/// vertex, and reads the result back. **Results are `f32` precision** — the
/// caller must treat them as an acceleration of, and approximation to,
/// [`Affine3::transform_points`] (the trusted `f64` CPU reference), not as a
/// bit-exact match.
///
/// An empty `positions` slice returns an empty `Vec` without touching the GPU.
///
/// # Errors
///
/// Returns [`GpuError`] if the submitted work cannot be completed (device lost
/// during the readback poll, or buffer-map failure). This is **recoverable**:
/// the caller should fall back to [`Affine3::transform_points`] — which
/// [`Affine3::transform_points_best_effort`] does automatically. The "no adapter
/// at all" case is handled earlier by [`crate::gpu::probe`] returning `None`, not here.
pub fn try_transform_vertices_gpu(
    ctx: &GpuContext,
    affine: Affine3,
    positions: &[Vec3],
) -> Result<Vec<Vec3>, GpuError> {
    if positions.is_empty() {
        return Ok(Vec::new());
    }

    let device = &ctx.device;
    let queue = &ctx.queue;

    // --- Pack inputs into GPU byte layouts (f32, little-endian) ------------
    // Positions: flat [x,y,z, x,y,z, ...].
    let mut input_f32: Vec<f32> = Vec::with_capacity(positions.len() * 3);
    for p in positions {
        input_f32.push(p.x as f32);
        input_f32.push(p.y as f32);
        input_f32.push(p.z as f32);
    }
    let input_bytes = f32_slice_to_bytes(&input_f32);
    let buffer_size = input_bytes.len() as wgpu::BufferAddress;

    // Uniform: four vec4<f32> rows (each vec3 padded to 16 bytes).
    let uniform_f32: [f32; 16] = [
        affine.linear[0][0] as f32,
        affine.linear[0][1] as f32,
        affine.linear[0][2] as f32,
        0.0,
        affine.linear[1][0] as f32,
        affine.linear[1][1] as f32,
        affine.linear[1][2] as f32,
        0.0,
        affine.linear[2][0] as f32,
        affine.linear[2][1] as f32,
        affine.linear[2][2] as f32,
        0.0,
        affine.translation.x as f32,
        affine.translation.y as f32,
        affine.translation.z as f32,
        0.0,
    ];
    let uniform_bytes = f32_slice_to_bytes(&uniform_f32);

    // --- GPU resources -----------------------------------------------------
    let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("affine-uniform"),
        size: uniform_bytes.len() as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&uniform_buffer, 0, &uniform_bytes);

    let input_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("vertex-input"),
        size: buffer_size,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&input_buffer, 0, &input_bytes);

    let output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("vertex-output"),
        size: buffer_size,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });

    // Staging buffer we can map on the CPU to read the result back.
    let readback_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("vertex-readback"),
        size: buffer_size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    // --- Pipeline (auto bind-group layout deduced from the shader) ---------
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("affine-transform-wgsl"),
        source: wgpu::ShaderSource::Wgsl(AFFINE_TRANSFORM_WGSL.into()),
    });

    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("affine-transform-pipeline"),
        layout: None,
        module: &shader,
        entry_point: Some("main"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        cache: None,
    });

    let bind_group_layout = pipeline.get_bind_group_layout(0);
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("affine-transform-bind-group"),
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: input_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: output_buffer.as_entire_binding(),
            },
        ],
    });

    // --- Dispatch ----------------------------------------------------------
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("affine-encoder"),
    });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("affine-transform-pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        let workgroups = (positions.len() as u32).div_ceil(WORKGROUP_SIZE);
        pass.dispatch_workgroups(workgroups, 1, 1);
    }
    encoder.copy_buffer_to_buffer(&output_buffer, 0, &readback_buffer, 0, buffer_size);
    queue.submit(Some(encoder.finish()));

    // --- Read back ---------------------------------------------------------
    let mapped = Arc::new(Mutex::new(None));
    let mapped_cb = Arc::clone(&mapped);
    readback_buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |res| {
            *mapped_cb.lock().unwrap() = Some(res);
        });
    // Drive the device until the map callback fires.
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .map_err(|e| GpuError::Poll(format!("{e:?}")))?;

    let map_result = mapped.lock().unwrap().take();
    match map_result {
        Some(Ok(())) => {}
        Some(Err(e)) => return Err(GpuError::Map(format!("{e:?}"))),
        None => return Err(GpuError::MapCallbackMissing),
    }

    let out_f32: Vec<f32> = {
        let view = readback_buffer.slice(..).get_mapped_range();
        bytes_to_f32_vec(&view)
    };
    readback_buffer.unmap();

    Ok(out_f32
        .chunks_exact(3)
        .map(|c| Vec3::new(c[0] as f64, c[1] as f64, c[2] as f64))
        .collect())
}

/// Apply `affine` to every position on the GPU, panicking on failure — the
/// strict convenience wrapper over [`try_transform_vertices_gpu`].
///
/// Use this only when a GPU failure should abort (e.g. a benchmark that must run
/// on the GPU, or a test that has already confirmed an adapter via [`probe`]).
/// For normal use prefer [`Affine3::transform_points_best_effort`], which never
/// panics and falls back to the CPU. **Results are `f32` precision.**
///
/// # Panics
///
/// Panics if [`try_transform_vertices_gpu`] returns a [`GpuError`].
pub fn transform_vertices_gpu(ctx: &GpuContext, affine: Affine3, positions: &[Vec3]) -> Vec<Vec3> {
    try_transform_vertices_gpu(ctx, affine, positions).expect(
        "GPU transform failed; use try_transform_vertices_gpu to handle failures on the CPU",
    )
}

/// Pack a slice of `f32` into little-endian bytes for a GPU buffer upload.
fn f32_slice_to_bytes(data: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() * 4);
    for &v in data {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out
}

/// Unpack little-endian GPU buffer bytes back into `f32`s.
fn bytes_to_f32_vec(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .collect()
}

/// Minimal in-crate `block_on`: drive a future to completion on the current
/// thread with **no `unsafe`** and no async-runtime dependency.
///
/// Uses a safe [`std::task::Wake`]-based thread-park waker (the same pattern as
/// `outram-mc-libs`'s GPU module) and the safe [`std::pin::pin!`] macro. wgpu's
/// native `request_adapter` / `request_device` / buffer-map futures make
/// progress when the device is polled around them; this re-polls until `Ready`,
/// parking the thread between polls and being unparked by the waker. Keeps the
/// dependency surface minimal (no `pollster`) per the workspace policy. Not a
/// general-purpose executor.
fn block_on<F: Future>(future: F) -> F::Output {
    struct ThreadWaker(std::thread::Thread);
    impl std::task::Wake for ThreadWaker {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
        fn wake_by_ref(self: &Arc<Self>) {
            self.0.unpark();
        }
    }
    let waker = Waker::from(Arc::new(ThreadWaker(std::thread::current())));
    let mut cx = Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    loop {
        match future.as_mut().poll(&mut cx) {
            Poll::Ready(val) => return val,
            Poll::Pending => std::thread::park(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The CPU reference path is always exercised (feature-independent).
    /// Documented here so the GPU module's own test proves the reference it
    /// compares against is correct.
    #[test]
    fn cpu_reference_transforms_points() {
        let affine = Affine3::from_rows(
            [[2.0, 0.0, 0.0], [0.0, 2.0, 0.0], [0.0, 0.0, 2.0]],
            Vec3::new(1.0, 2.0, 3.0),
        );
        let pts = vec![Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0)];
        let out = affine.transform_points(&pts);
        assert_eq!(out[0], Vec3::new(3.0, 2.0, 3.0));
        assert_eq!(out[1], Vec3::new(1.0, 4.0, 3.0));
    }

    /// **GPU vs CPU agreement — methodology + result recorded per V&V rule.**
    ///
    /// Methodology: probe for a headless adapter; if none, SKIP (headless CI /
    /// no GPU is expected, never a failure). If a device exists, transform a
    /// 1000-vertex set with both [`crate::gpu::transform_vertices_gpu`] (GPU, f32) and
    /// [`Affine3::transform_points`] (CPU, f64 reference), and assert every
    /// component agrees within an absolute tolerance of `1e-4` (chosen for
    /// `f32` GPU precision over these O(10) magnitudes; well above f32 epsilon).
    /// The transform mixes a z-rotation, a z-scale, and a translation so a wrong
    /// row/column order or a dropped translation would be caught.
    ///
    /// Result: recorded per-run in the crate's AI-fleet review manifest
    /// (`docs/ai-fleet-review/scaffold/REVIEW_MANIFEST.md`), including whether
    /// an adapter was present on the machine the suite last ran on.
    #[test]
    fn gpu_matches_cpu_or_skips() {
        let Some(ctx) = probe() else {
            eprintln!("SKIP gpu_matches_cpu_or_skips: no headless GPU adapter available");
            return;
        };

        let affine = Affine3::from_rows(
            [[0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.5]], // rotate about z, scale z
            Vec3::new(3.0, -2.0, 10.0),
        );
        let pts: Vec<Vec3> = (0..1000)
            .map(|i| {
                let f = i as f64;
                Vec3::new(f * 0.5 - 3.0, (f * 0.25).sin() * 4.0, f * 0.01)
            })
            .collect();

        let gpu = transform_vertices_gpu(&ctx, affine, &pts);
        let cpu = affine.transform_points(&pts);

        assert_eq!(gpu.len(), cpu.len(), "GPU/CPU output length mismatch");
        let tol = 1e-4_f64;
        let mut max_err = 0.0_f64;
        for (g, c) in gpu.iter().zip(cpu.iter()) {
            for (a, b) in [(g.x, c.x), (g.y, c.y), (g.z, c.z)] {
                max_err = max_err.max((a - b).abs());
            }
        }
        assert!(
            max_err <= tol,
            "GPU result diverged from CPU reference: max abs error {max_err:e} > tol {tol:e}"
        );
        eprintln!("PASS gpu_matches_cpu_or_skips: max abs GPU-CPU error {max_err:e} (tol {tol:e})");
    }
}
