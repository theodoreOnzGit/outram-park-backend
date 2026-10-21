// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK.

//! Optional headless GPU compute for this crate's one embarrassingly-parallel
//! kernel: the [`crate::rdf`] pair-separation histogram.
//!
//! # Why only the RDF, and not the timestep
//!
//! This is the deliberate scope limit, and it is worth stating where someone
//! will look for it. The DEM timestep carries a persistent per-contact
//! tangential shear history with contacts born and dying every step — a
//! stateful gather/scatter structure that is the worst shape for a shader —
//! while the Hertz arithmetic that *would* port well was measured at only
//! 3.7 ms of a 17.8 ms step once parallelised on the CPU. At HTR-10 scale the
//! per-step working set is also far too small to amortise a host↔device round
//! trip unless the whole integrator became GPU-resident. See
//! [`crate::compute::ComputeType`].
//!
//! The RDF is the opposite: 3.8e8 independent, stateless distance evaluations
//! over a fixed point set, computed once per bed rather than once per step.
//!
//! # Contract
//!
//! 1. **Compiles always, runs on CPU when there is no GPU.** The whole module
//!    is target-gated out on Android and `wasm32` (no system Vulkan/Metal
//!    loader; `wgpu-hal` is not `Sync` on wasm), and [`rdf_histogram`] returns
//!    `None` whenever no usable adapter exists — a headless server, CI with no
//!    loader. Callers **must** treat `None` as "run the CPU path", never as an
//!    error. [`crate::rdf::radial_distribution`] does exactly that.
//! 2. **CPU is the trusted reference.** WGSL has no `f64`, so this kernel is
//!    `f32` throughout while the CPU path is `f64`. A pair whose separation
//!    falls within `f32` rounding of a bin edge can therefore land in a
//!    neighbouring bin, and the two histograms are **not** bit-identical. This
//!    is a real, measured difference, not a hypothetical — see
//!    `tests/rdf_backends.rs`. Anything feeding a V&V number uses the CPU path.
//! 3. **No new third-party dependency.** `wgpu`'s `request_adapter` /
//!    `request_device` return futures; rather than pull in an async runtime
//!    this module hand-rolls a tiny pure-`std` [`block_on`], the same shape
//!    `outram-mc-libs` uses.

use std::future::Future;
use std::pin::pin;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};
use std::thread;

use crate::particle::Vec3;

/// Largest bin count the shader's workgroup-local histogram can hold; must
/// match `MAX_BINS` in `shaders/rdf_histogram.wgsl`.
///
/// A request for more bins than this falls back to the CPU rather than
/// silently truncating the histogram.
pub const MAX_BINS: usize = 1024;

/// Workgroup size; must match `WG_SIZE` in the shader.
const WG_SIZE: u32 = 64;

/// A live, headless GPU compute context — a [`wgpu::Device`] and
/// [`wgpu::Queue`] obtained with no window or surface.
pub struct GpuContext {
    /// The logical GPU device.
    pub device: wgpu::Device,
    /// The command queue.
    pub queue: wgpu::Queue,
    /// Which physical adapter was selected. Diagnostic only.
    pub info: wgpu::AdapterInfo,
}

impl std::fmt::Debug for GpuContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GpuContext")
            .field("adapter", &self.info.name)
            .field("backend", &self.info.backend)
            .field("device_type", &self.info.device_type)
            .finish()
    }
}

/// Probe for a usable headless compute GPU, or `None` when the caller must
/// fall back to the CPU path.
///
/// `None` is the *normal, expected* outcome on a headless host and is never an
/// error.
#[must_use]
pub fn probe() -> Option<GpuContext> {
    let instance = wgpu::Instance::default();
    let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::default(),
        force_fallback_adapter: false,
        compatible_surface: None,
        apply_limit_buckets: false,
    }))
    .ok()?;
    let info = adapter.get_info();
    let (device, queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("outram-park-fork-liggghts headless compute device"),
        required_features: wgpu::Features::empty(),
        required_limits: adapter.limits(),
        ..Default::default()
    }))
    .ok()?;
    Some(GpuContext {
        device,
        queue,
        info,
    })
}

/// GPU pair-separation histogram: for every centre in `centre_idx`, count the
/// pebbles of `centres` whose separation is below `r_max`, binned at width
/// `dr`.
///
/// Returns `None` — meaning *run the CPU path* — when no adapter is available,
/// or when `n_bins` exceeds [`MAX_BINS`] (the shader's workgroup histogram is a
/// compile-time size).
///
/// # Precision
///
/// `f32` throughout; see the module contract. Not bit-identical to the `f64`
/// CPU reference.
#[must_use]
pub fn rdf_histogram(
    centres: &[Vec3],
    centre_idx: &[u32],
    r_max: f64,
    dr: f64,
    n_bins: usize,
) -> Option<Vec<u64>> {
    if n_bins > MAX_BINS || centre_idx.is_empty() || centres.is_empty() {
        return None;
    }
    let ctx = probe()?;
    let (device, queue) = (&ctx.device, &ctx.queue);

    // --- upload ---
    // vec4 padding: a vec3 in a storage array has surprising stride rules, so
    // the 16-byte alignment is made explicit instead of assumed.
    let mut pos: Vec<f32> = Vec::with_capacity(centres.len() * 4);
    for c in centres {
        pos.extend_from_slice(&[c.x as f32, c.y as f32, c.z as f32, 0.0]);
    }
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct Params {
        r_max_sq: f32,
        inv_dr: f32,
        n_bins: u32,
        n_points: u32,
        n_centres: u32,
        _pad: [u32; 3],
    }
    let params = Params {
        r_max_sq: (r_max * r_max) as f32,
        inv_dr: (1.0 / dr) as f32,
        n_bins: n_bins as u32,
        n_points: centres.len() as u32,
        n_centres: centre_idx.len() as u32,
        _pad: [0; 3],
    };

    use wgpu::util::DeviceExt;
    let params_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("rdf params"),
        contents: bytes_of(&params),
        usage: wgpu::BufferUsages::UNIFORM,
    });
    let pos_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("rdf positions"),
        contents: slice_bytes(&pos),
        usage: wgpu::BufferUsages::STORAGE,
    });
    let idx_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("rdf centre indices"),
        contents: slice_bytes(centre_idx),
        usage: wgpu::BufferUsages::STORAGE,
    });
    let hist_bytes = (n_bins * std::mem::size_of::<u32>()) as u64;
    let hist_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("rdf histogram"),
        size: hist_bytes,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("rdf readback"),
        size: hist_bytes,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    // --- pipeline ---
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("rdf_histogram"),
        source: wgpu::ShaderSource::Wgsl(include_str!("shaders/rdf_histogram.wgsl").into()),
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("rdf pipeline"),
        layout: None,
        module: &shader,
        entry_point: Some("main"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        cache: None,
    });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("rdf bind group"),
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: params_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: pos_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: idx_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: hist_buf.as_entire_binding(),
            },
        ],
    });

    // --- dispatch ---
    let mut encoder =
        device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("rdf") });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("rdf pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        let groups = (centre_idx.len() as u32).div_ceil(WG_SIZE);
        pass.dispatch_workgroups(groups, 1, 1);
    }
    encoder.copy_buffer_to_buffer(&hist_buf, 0, &readback, 0, hist_bytes);
    queue.submit(Some(encoder.finish()));

    // --- read back ---
    let slice = readback.slice(..);
    slice.map_async(wgpu::MapMode::Read, |_| {});
    device.poll(wgpu::PollType::wait_indefinitely()).ok()?;
    let data = slice.get_mapped_range().ok()?;
    let counts: Vec<u64> = data
        .chunks_exact(4)
        .map(|b| u64::from(u32::from_ne_bytes([b[0], b[1], b[2], b[3]])))
        .collect();
    drop(data);
    readback.unmap();
    Some(counts)
}

/// Reinterpret a `Copy` struct as bytes for a uniform upload.
///
/// Hand-rolled rather than taking a `bytemuck` dependency for two call sites;
/// both types are `#[repr(C)]` plain-old-data with no padding surprises and no
/// pointers.
fn bytes_of<T: Copy>(v: &T) -> &[u8] {
    // SAFETY: `T` here is a `#[repr(C)]` POD struct of `f32`/`u32`, so every
    // byte of it is initialised and reading it as bytes is well-defined.
    unsafe { std::slice::from_raw_parts((v as *const T).cast::<u8>(), std::mem::size_of::<T>()) }
}

/// Reinterpret a slice of POD scalars as bytes for a storage upload.
fn slice_bytes<T: Copy>(v: &[T]) -> &[u8] {
    // SAFETY: `T` is `f32` or `u32` at both call sites — POD, fully
    // initialised, no padding.
    unsafe { std::slice::from_raw_parts(v.as_ptr().cast::<u8>(), std::mem::size_of_val(v)) }
}

/// Minimal pure-`std` executor: block the current thread until `future`
/// resolves, driving it with a `Wake`-based thread-park waker.
///
/// The standard `pollster::block_on` shape. It exists so this crate needs **no**
/// async-runtime dependency to await `wgpu`'s two setup futures. Buffer
/// read-back does not use it — that uses `Device::poll`.
pub fn block_on<F: Future>(future: F) -> F::Output {
    struct ThreadWaker(thread::Thread);
    impl Wake for ThreadWaker {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
        fn wake_by_ref(self: &Arc<Self>) {
            self.0.unpark();
        }
    }
    let waker = Waker::from(Arc::new(ThreadWaker(thread::current())));
    let mut cx = Context::from_waker(&waker);
    let mut future = pin!(future);
    loop {
        match future.as_mut().poll(&mut cx) {
            Poll::Ready(v) => return v,
            Poll::Pending => thread::park(),
        }
    }
}
