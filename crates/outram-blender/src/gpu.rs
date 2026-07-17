//! GPU compute (optional, behind the `gpu` cargo feature).
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
//! WGSL compute shader ([`transform_vertices_gpu`]). The identical computation
//! on the CPU is [`crate::transform::Affine3::transform_points`], which is the
//! reference the GPU result is validated against (see the tests below). This
//! kernel is deliberately the simplest embarrassingly-parallel mesh operation —
//! it exists to prove the GPU compute path is live and CPU-checked, not because
//! an affine transform needs a GPU. Heavier per-vertex kernels (deformation,
//! subdivision) follow the same buffer/pipeline pattern.
//!
//! ## Non-negotiable contract for using this module
//!
//! 1. **Feature-gated.** This module only exists under `--features gpu`. The
//!    default build is wgpu-free and headless-Android-clean (per the workspace
//!    Android rule) — a plain `cargo build` never pulls the GPU stack.
//! 2. **Runtime CPU fallback is mandatory.** wgpu *compiles* for Android/CI but
//!    at runtime there may be **no usable GPU adapter** (headless servers, the
//!    Android emulator). Callers MUST treat [`probe`] returning `None` as
//!    "run the CPU path", never as an error.
//! 3. **CPU is the trusted / reference path.** GPU float reduction order will
//!    not bit-match the CPU (`f64`, [`crate::transform`]) result, so anything
//!    that feeds V&V or a solver stays CPU-deterministic. GPU is *acceleration
//!    only*, and [`transform_vertices_gpu`] returns `f32`-precision results.

use crate::math::Vec3;
use crate::transform::Affine3;
use core::future::Future;
use core::task::{Context, Poll, Waker};
use std::sync::{Arc, Mutex};

/// Re-export of the GPU backend so callers can build pipelines without adding
/// their own `wgpu` dependency. Only available under `--features gpu`.
pub use wgpu;

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
/// requests is done with a tiny in-crate executor ([`block_on`]) so this crate
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
/// transformed positions in the same order.
///
/// This is the demonstrator GPU kernel. It uploads the positions as an `f32`
/// storage buffer, dispatches [`AFFINE_TRANSFORM_WGSL`] one invocation per
/// vertex, and reads the result back. **Results are `f32` precision** — the
/// caller must treat them as an acceleration of, and approximation to,
/// [`Affine3::transform_points`] (the trusted `f64` CPU reference), not as a
/// bit-exact match.
///
/// An empty `positions` slice returns an empty `Vec` without touching the GPU.
///
/// # Panics
///
/// Panics if the GPU work cannot be completed (device lost, shader compile
/// failure, or buffer-map failure). These indicate a driver/programming fault,
/// not the "no adapter" case — that is handled earlier by [`probe`] returning
/// `None`. Callers wanting infallible behavior should keep the CPU path as the
/// fallback regardless.
pub fn transform_vertices_gpu(
    ctx: &GpuContext,
    affine: Affine3,
    positions: &[Vec3],
) -> Vec<Vec3> {
    if positions.is_empty() {
        return Vec::new();
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
        affine.linear[0][0] as f32, affine.linear[0][1] as f32, affine.linear[0][2] as f32, 0.0,
        affine.linear[1][0] as f32, affine.linear[1][1] as f32, affine.linear[1][2] as f32, 0.0,
        affine.linear[2][0] as f32, affine.linear[2][1] as f32, affine.linear[2][2] as f32, 0.0,
        affine.translation.x as f32, affine.translation.y as f32, affine.translation.z as f32, 0.0,
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
    let mut encoder = device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("affine-encoder") });
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
    readback_buffer.slice(..).map_async(wgpu::MapMode::Read, move |res| {
        *mapped_cb.lock().unwrap() = Some(res);
    });
    // Drive the device until the map callback fires.
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("GPU poll failed while awaiting readback");

    let map_result = mapped.lock().unwrap().take();
    match map_result {
        Some(Ok(())) => {}
        Some(Err(e)) => panic!("GPU buffer map failed: {e:?}"),
        None => panic!("GPU buffer map callback never fired after wait-indefinitely poll"),
    }

    let out_f32: Vec<f32> = {
        let view = readback_buffer.slice(..).get_mapped_range();
        bytes_to_f32_vec(&view)
    };
    readback_buffer.unmap();

    out_f32
        .chunks_exact(3)
        .map(|c| Vec3::new(c[0] as f64, c[1] as f64, c[2] as f64))
        .collect()
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
    /// 1000-vertex set with both [`transform_vertices_gpu`] (GPU, f32) and
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
