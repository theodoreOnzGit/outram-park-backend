// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! The `wgpu` device context and the one reusable kernel-dispatch helper every
//! GPU kernel in this crate goes through — bead `op-yvj.4.1`, GitHub #10.
//!
//! # What belongs in this module
//!
//! Device acquisition ([`context`]), the shared [`GpuContext`] handle, the
//! generic [`GpuContext::dispatch`] entry point, and the `f64` <-> `f32`
//! packing helpers at the buffer boundary. Nothing numerical.
//!
//! # What does NOT belong here
//!
//! WGSL kernel sources and the functions that call them. A kernel lives beside
//! its CPU sibling — `ldu_matrix/parallel.rs` owns the SpMV shader,
//! `math/parallel.rs` owns the polynomial-root shader — so that a reader
//! comparing the two paths finds them in one file. This module only knows how
//! to *run* a shader, never what any shader computes.
//!
//! # Precision: this is an `f32` backend, and that is load-bearing
//!
//! **WGSL has no `f64`.** Every kernel dispatched through here computes in
//! `f32`, whatever the `f64` public signature says. [`f64_to_f32_bytes`] does
//! the narrowing on upload and [`bytes_to_f64_vec`] the widening on readback,
//! so the loss happens at exactly two documented places rather than being
//! spread through kernel code.
//!
//! The consequence is a hard floor of about **1.2e-7 relative** (`f32` machine
//! epsilon) on any GPU result, before the kernel's own error. That is why
//! [`crate::compute::ComputeBackend::Serial`] stays the oracle and why every
//! GPU kernel must carry its own measured deviation in its doc comment. The
//! maintainer accepted `f32` as a documented caveat rather than a blocker
//! (GitHub #17, 2026-08-12); this paragraph is where that caveat is recorded
//! for the dispatch layer, and each kernel repeats it with its own numbers.
//!
//! # Binding convention
//!
//! Every shader run through [`GpuContext::dispatch`] uses `@group(0)` and the
//! same binding order, so a reader can predict the layout from the call site:
//!
//! | Binding | Kind | Contents |
//! |---|---|---|
//! | `0 .. n` | `storage, read` | the `n` input buffers, in argument order |
//! | `n` | `storage, read_write` | the single output buffer |
//! | `n + 1` | `uniform` | the parameter block |
//!
//! # Failure is recoverable, never a panic
//!
//! No adapter, a lost device, a failed buffer map — all return
//! [`GpuError`] or `None`, and every caller is expected to fall back to
//! [`crate::compute::ComputeBackend::CpuMulti`] or `Serial`. A machine with no
//! GPU is a normal machine.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock, RwLock};

/// A recoverable failure while running a GPU kernel.
///
/// Every variant means "fall back to the CPU path", never "the calculation is
/// wrong". None of them is produced by a correct-but-unavailable GPU: that is
/// `None` from [`context`], not an error.
#[derive(Debug, thiserror::Error)]
pub enum GpuError {
    /// The device could not be polled to completion (typically a lost device).
    #[error("GPU device poll failed while awaiting readback: {0}")]
    Poll(String),
    /// The staging buffer could not be mapped for reading.
    #[error("GPU staging buffer map failed: {0}")]
    Map(String),
    /// The map callback never fired despite a wait-indefinitely poll.
    #[error("GPU buffer map callback never fired after wait-indefinitely poll")]
    MapCallbackMissing,
    /// A dispatch was asked for more workgroups than the device permits.
    #[error("GPU dispatch needs {needed} workgroups but the device limit is {limit}")]
    TooManyWorkgroups {
        /// Workgroups the dispatch requires.
        needed: u32,
        /// The adapter's `max_compute_workgroups_per_dimension`.
        limit: u32,
    },
    /// The kernel binds more storage buffers than this device supports.
    ///
    /// Checked **before** pipeline creation, because wgpu's validation layer
    /// panics on this rather than returning an error, and a panic is not an
    /// acceptable outcome for "your GPU is a bit older than ours".
    #[error("GPU kernel binds {needed} storage buffers but the device limit is {limit}")]
    TooManyStorageBuffers {
        /// Storage buffers the kernel binds (inputs plus the one output).
        needed: u32,
        /// The device's `max_storage_buffers_per_shader_stage`.
        limit: u32,
    },
}

/// The number of lanes one workgroup handles. Chosen as 64 to match the other
/// wgpu kernels in this workspace (`outram-blender`, `njoy-outram-park-fork`)
/// and because 64 is a multiple of both the AMD wavefront (64) and the NVIDIA
/// warp (32), so no lanes are wasted on either vendor.
pub const WORKGROUP_SIZE: u32 = 64;

/// A live GPU device, its queue, and the compiled-pipeline cache.
///
/// Obtained from [`context`], which builds exactly one per process. The
/// `device` and `queue` are held as `Arc<T>` per the workspace shared-state
/// rule, so a caller that wants to keep its own handle can clone cheaply
/// rather than borrowing.
#[derive(Debug)]
pub struct GpuContext {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    adapter_label: String,
    max_workgroups_per_dim: u32,
    max_storage_buffers: u32,
    /// Compiled pipelines keyed by the kernel's `&'static str` label, so a
    /// kernel called once per timestep compiles its WGSL once per process
    /// rather than once per call. `RwLock` (not `Mutex`) because the steady
    /// state is many concurrent readers and no writer.
    pipelines: RwLock<HashMap<&'static str, Arc<wgpu::ComputePipeline>>>,
}

impl GpuContext {
    /// The adapter's human-readable name, for benchmark tables and V&V records
    /// (the workspace V&V rule requires the hardware be stated).
    #[must_use]
    pub fn adapter_label(&self) -> &str {
        &self.adapter_label
    }

    /// The shared logical device.
    #[must_use]
    pub fn device(&self) -> &Arc<wgpu::Device> {
        &self.device
    }

    /// The shared command queue.
    #[must_use]
    pub fn queue(&self) -> &Arc<wgpu::Queue> {
        &self.queue
    }

    /// The largest `lanes` this device can service in one [`Self::dispatch`].
    #[must_use]
    pub fn max_lanes(&self) -> u64 {
        u64::from(self.max_workgroups_per_dim) * u64::from(WORKGROUP_SIZE)
    }

    /// How many `storage` buffers one kernel may bind on this device —
    /// `inputs.len() + 1` must not exceed it.
    ///
    /// Worth checking before writing a kernel with many operand arrays: the
    /// WebGPU downlevel floor is **4**, which a multi-array kernel such as the
    /// LDU sparse product exceeds easily. Pack operands into fewer buffers
    /// rather than assuming a desktop limit.
    #[must_use]
    pub fn max_storage_buffers(&self) -> u32 {
        self.max_storage_buffers
    }

    /// Run one compute shader and return its output buffer as raw bytes.
    ///
    /// This is the **only** way a kernel in this crate reaches the GPU. It
    /// compiles (or reuses) the pipeline for `label`, uploads `inputs` and
    /// `params`, dispatches `lanes` invocations rounded up to whole
    /// workgroups, and blocks until the results are read back.
    ///
    /// # Arguments
    ///
    /// - `label` — a unique `&'static str` naming the kernel. **This is the
    ///   pipeline-cache key**, so two different shaders must never share a
    ///   label.
    /// - `wgsl` — the shader source. Its entry point must be `main` with
    ///   `@workgroup_size(64)` (see [`WORKGROUP_SIZE`]).
    /// - `inputs` — read-only storage buffers, bound at `0 .. inputs.len()` in
    ///   order. An empty slice is padded to 4 zero bytes, because a zero-sized
    ///   storage buffer is invalid; a shader must not read a buffer it was
    ///   given no data for.
    /// - `output_bytes` — size of the read-write output buffer, bound at
    ///   `inputs.len()`.
    /// - `params` — the uniform block, bound at `inputs.len() + 1`. Padded up
    ///   to a 16-byte multiple, as uniform layout requires.
    /// - `lanes` — number of logical work items. The shader is responsible for
    ///   its own bounds check, since `lanes` is rounded up to a workgroup
    ///   multiple.
    ///
    /// # Returns
    ///
    /// `Ok(bytes)` of length `output_bytes`, or a [`GpuError`] the caller
    /// should treat as "use the CPU path".
    ///
    /// # Units
    ///
    /// Dimensionless bytes in, dimensionless bytes out. Unit-typed
    /// (`uom`) values are converted by the caller *before* reaching here and
    /// converted back after — the public signature of a kernel stays typed
    /// (workspace rule: `uom` is not stripped to get onto the GPU).
    pub fn dispatch(
        &self,
        label: &'static str,
        wgsl: &str,
        inputs: &[&[u8]],
        output_bytes: u64,
        params: &[u8],
        lanes: u32,
    ) -> Result<Vec<u8>, GpuError> {
        use wgpu::util::DeviceExt;

        let workgroups = lanes.div_ceil(WORKGROUP_SIZE).max(1);
        if workgroups > self.max_workgroups_per_dim {
            return Err(GpuError::TooManyWorkgroups {
                needed: workgroups,
                limit: self.max_workgroups_per_dim,
            });
        }
        // wgpu's validation layer *panics* on an over-budget bind group during
        // pipeline creation, so this must be caught here.
        let storage_needed = inputs.len() as u32 + 1;
        if storage_needed > self.max_storage_buffers {
            return Err(GpuError::TooManyStorageBuffers {
                needed: storage_needed,
                limit: self.max_storage_buffers,
            });
        }

        let pipeline = self.pipeline_for(label, wgsl);

        // --- buffers ------------------------------------------------------
        // A storage buffer must be non-empty and 4-byte aligned.
        let padded: Vec<Vec<u8>> = inputs
            .iter()
            .map(|b| {
                if b.is_empty() {
                    vec![0u8; 4]
                } else {
                    b.to_vec()
                }
            })
            .collect();
        let input_buffers: Vec<wgpu::Buffer> = padded
            .iter()
            .map(|bytes| {
                self.device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some(label),
                        contents: bytes,
                        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                    })
            })
            .collect();

        let out_size = output_bytes.max(4);
        let output_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size: out_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size: out_size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Uniform blocks are laid out in 16-byte units.
        let mut param_bytes = params.to_vec();
        while param_bytes.is_empty() || !param_bytes.len().is_multiple_of(16) {
            param_bytes.push(0);
        }
        let param_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents: &param_bytes,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

        // --- bind group ---------------------------------------------------
        let mut entries: Vec<wgpu::BindGroupEntry> = Vec::with_capacity(inputs.len() + 2);
        for (i, buf) in input_buffers.iter().enumerate() {
            entries.push(wgpu::BindGroupEntry {
                binding: i as u32,
                resource: buf.as_entire_binding(),
            });
        }
        entries.push(wgpu::BindGroupEntry {
            binding: inputs.len() as u32,
            resource: output_buffer.as_entire_binding(),
        });
        entries.push(wgpu::BindGroupEntry {
            binding: inputs.len() as u32 + 1,
            resource: param_buffer.as_entire_binding(),
        });

        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(label),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &entries,
        });

        // --- dispatch -------------------------------------------------------
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some(label) });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some(label),
                timestamp_writes: None,
            });
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups(workgroups, 1, 1);
        }
        encoder.copy_buffer_to_buffer(&output_buffer, 0, &readback, 0, out_size);
        self.queue.submit(Some(encoder.finish()));

        // --- readback -------------------------------------------------------
        let mapped = Arc::new(Mutex::new(None));
        let cb = Arc::clone(&mapped);
        readback
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |res| {
                *cb.lock().unwrap() = Some(res);
            });
        // One `wait_indefinitely` poll returns when the *submitted work* is
        // done, which is not the same instant the map callback fires — and
        // with several threads sharing one device another thread's poll can
        // consume ours. Re-poll until the callback has actually run, bounded
        // so a genuinely wedged device still returns an error rather than
        // hanging the caller.
        const MAX_POLLS: u32 = 10_000;
        let mut result = None;
        for _ in 0..MAX_POLLS {
            self.device
                .poll(wgpu::PollType::wait_indefinitely())
                .map_err(|e| GpuError::Poll(format!("{e:?}")))?;
            result = mapped.lock().unwrap().take();
            if result.is_some() {
                break;
            }
            std::thread::yield_now();
        }

        match result {
            Some(Ok(())) => {}
            Some(Err(e)) => return Err(GpuError::Map(format!("{e:?}"))),
            None => return Err(GpuError::MapCallbackMissing),
        }

        let out = {
            let view = readback
                .slice(..)
                .get_mapped_range()
                .map_err(|e| GpuError::Map(format!("{e:?}")))?;
            view[..output_bytes as usize].to_vec()
        };
        readback.unmap();
        Ok(out)
    }

    /// Fetch the cached pipeline for `label`, compiling `wgsl` on first use.
    fn pipeline_for(&self, label: &'static str, wgsl: &str) -> Arc<wgpu::ComputePipeline> {
        if let Some(p) = self.pipelines.read().unwrap().get(label) {
            return Arc::clone(p);
        }
        let module = self
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some(label),
                source: wgpu::ShaderSource::Wgsl(wgsl.into()),
            });
        let pipeline = Arc::new(self.device.create_compute_pipeline(
            &wgpu::ComputePipelineDescriptor {
                label: Some(label),
                layout: None,
                module: &module,
                entry_point: Some("main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                cache: None,
            },
        ));
        self.pipelines
            .write()
            .unwrap()
            .insert(label, Arc::clone(&pipeline));
        pipeline
    }
}

/// Rank an adapter for **headless compute**. Higher is better.
///
/// Two things dominate and neither is expressed by
/// [`wgpu::PowerPreference`]:
///
/// - **Device type.** A discrete GPU beats an integrated one by roughly an
///   order of magnitude on the kernels in this crate.
/// - **Backend.** Vulkan/Metal/DX12 are real compute backends; the OpenGL
///   backend emulates compute and is markedly slower, so it is ranked last and
///   taken only when nothing else exists.
///
/// Software rasterisers (`DeviceType::Cpu`) are filtered out before scoring —
/// they are slower than this crate's own `CpuMulti` path, so dispatching to
/// one would be a pure loss.
fn adapter_score(info: &wgpu::AdapterInfo) -> u32 {
    let type_score = match info.device_type {
        wgpu::DeviceType::DiscreteGpu => 300,
        wgpu::DeviceType::IntegratedGpu => 200,
        wgpu::DeviceType::VirtualGpu => 100,
        _ => 0,
    };
    let backend_score = match info.backend {
        wgpu::Backend::Vulkan | wgpu::Backend::Metal | wgpu::Backend::Dx12 => 30,
        wgpu::Backend::BrowserWebGpu => 20,
        wgpu::Backend::Gl => 1,
        _ => 0,
    };
    type_score + backend_score
}

/// The process-wide GPU context, acquired at most once.
static CONTEXT: OnceLock<Option<GpuContext>> = OnceLock::new();

/// The shared [`GpuContext`], or `None` when this machine has no usable
/// compute adapter.
///
/// Acquisition is attempted **once** per process and the answer cached, so a
/// kernel may call this in a hot loop. `None` is a normal outcome — headless
/// CI, a container, Android, a machine with no GPU — and is never an error;
/// the caller falls back to a CPU backend.
#[must_use]
pub fn context() -> Option<&'static GpuContext> {
    CONTEXT.get_or_init(acquire).as_ref()
}

/// Build the context. Mirrors `outram-blender`'s and `njoy-outram-park-fork`'s
/// headless probe (no surface, downlevel limits, so software adapters such as
/// Lavapipe and WARP also qualify) so the workspace has one shape rather than
/// several.
fn acquire() -> Option<GpuContext> {
    let instance = wgpu::Instance::default();

    // `request_adapter` with `HighPerformance` is not reliable for headless
    // compute: on a machine with both an integrated and a discrete GPU it was
    // observed handing back the Intel iGPU on the OpenGL backend while an
    // RTX A5000 sat idle on Vulkan (measured 2026-09-03). Compute throughput
    // differs by more than an order of magnitude between those two, so the
    // choice is scored explicitly instead of delegated.
    let adapter = block_on(instance.enumerate_adapters(wgpu::Backends::all()))
        .into_iter()
        .filter(|a| a.get_info().device_type != wgpu::DeviceType::Cpu)
        .max_by_key(|a| adapter_score(&a.get_info()))
        .or_else(|| {
            block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: None,
                apply_limit_buckets: false,
            }))
            .ok()
        })?;
    let info = adapter.get_info();
    let adapter_limits = adapter.limits();

    // Start from the downlevel floor — the broadest-compatibility profile, so
    // software adapters (Lavapipe, WARP) still qualify — then raise only the
    // two limits this crate's kernels actually need beyond it, and only as far
    // as the adapter genuinely supports. Asking for more than the adapter has
    // makes `request_device` fail outright, which would turn a capable-enough
    // GPU into no GPU at all.
    let mut limits = wgpu::Limits::downlevel_defaults();
    // The LDU sparse product binds 7 inputs + 1 output; the downlevel floor is 4.
    limits.max_storage_buffers_per_shader_stage = adapter_limits
        .max_storage_buffers_per_shader_stage
        .min(8)
        .max(limits.max_storage_buffers_per_shader_stage);
    limits.max_compute_workgroups_per_dimension = adapter_limits
        .max_compute_workgroups_per_dimension
        .max(limits.max_compute_workgroups_per_dimension);
    limits.max_storage_buffer_binding_size = adapter_limits
        .max_storage_buffer_binding_size
        .max(limits.max_storage_buffer_binding_size);
    limits.max_buffer_size = adapter_limits.max_buffer_size.max(limits.max_buffer_size);

    let (device, queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("outram-foam-basic-lib headless compute"),
        required_features: wgpu::Features::empty(),
        required_limits: limits.clone(),
        ..Default::default()
    }))
    .ok()?;
    Some(GpuContext {
        device: Arc::new(device),
        queue: Arc::new(queue),
        adapter_label: format!("{} ({:?}, {:?})", info.name, info.device_type, info.backend),
        max_workgroups_per_dim: limits.max_compute_workgroups_per_dimension,
        max_storage_buffers: limits.max_storage_buffers_per_shader_stage,
        pipelines: RwLock::new(HashMap::new()),
    })
}

// ---------------------------------------------------------------------------
// Buffer-boundary conversions — the two places `f64` precision is lost.
// ---------------------------------------------------------------------------

/// Narrow `f64` values to `f32` and pack them little-endian for upload.
///
/// This is one of exactly two places in the crate where GPU precision loss
/// happens (the other is [`bytes_to_f64_vec`]). Values outside the `f32` range
/// become `±inf`, and subnormals flush toward zero — both are `f32` semantics,
/// not a bug here.
#[must_use]
pub fn f64_to_f32_bytes(values: &[f64]) -> Vec<u8> {
    let mut out = Vec::with_capacity(values.len() * 4);
    for &v in values {
        out.extend_from_slice(&(v as f32).to_le_bytes());
    }
    out
}

/// Pack `u32` values little-endian for upload (indices, counts, flags).
///
/// Lossless — this is the integer counterpart of [`f64_to_f32_bytes`] and
/// exists so index buffers do not go through a float path.
#[must_use]
pub fn u32_to_bytes(values: &[u32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(values.len() * 4);
    for &v in values {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out
}

/// Widen a little-endian `f32` byte buffer back to `f64`.
///
/// Widening is exact — the `f32` value is represented perfectly as an `f64`.
/// The precision that was already lost on upload is **not** recovered, which
/// is why a GPU result carries `f32` error even though its type is `f64`.
///
/// Trailing bytes that do not form a whole `f32` are ignored.
#[must_use]
pub fn bytes_to_f64_vec(bytes: &[u8]) -> Vec<f64> {
    bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|c| f32::from_le_bytes(*c) as f64)
        .collect()
}

/// Read a little-endian `u32` buffer back (flags, per-lane status codes).
#[must_use]
pub fn bytes_to_u32_vec(bytes: &[u8]) -> Vec<u32> {
    bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|c| u32::from_le_bytes(*c))
        .collect()
}

/// A minimal thread-parking executor, so blocking on wgpu's async requests
/// does not drag an async runtime into this crate.
///
/// The same device as `crate::compute::pollster_lite` and the equivalents in
/// `outram-blender` / `njoy-outram-park-fork`. Kept private.
fn block_on<F: std::future::Future>(future: F) -> F::Output {
    use std::pin::Pin;
    use std::task::{Context, Poll, Wake, Waker};

    struct ThreadWaker(std::thread::Thread);
    impl Wake for ThreadWaker {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
    }

    let mut future = std::pin::pin!(future);
    let waker = Waker::from(Arc::new(ThreadWaker(std::thread::current())));
    let mut cx = Context::from_waker(&waker);
    loop {
        match Pin::as_mut(&mut future).poll(&mut cx) {
            Poll::Ready(v) => return v,
            Poll::Pending => std::thread::park(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A trivial shader: `out[i] = in[i] * scale`. Exercises every part of the
    /// dispatch contract — one input buffer, one output buffer, a uniform
    /// param, and the bounds check.
    const SCALE_WGSL: &str = r#"
@group(0) @binding(0) var<storage, read>       src: array<f32>;
@group(0) @binding(1) var<storage, read_write> dst: array<f32>;
@group(0) @binding(2) var<uniform>             params: vec4<f32>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= u32(params.y)) { return; }
    dst[i] = src[i] * params.x;
}
"#;

    #[test]
    fn context_is_cached_and_never_panics() {
        let a = context().is_some();
        let b = context().is_some();
        assert_eq!(a, b, "the probe must be cached, not re-run");
    }

    #[test]
    fn dispatch_scales_a_buffer_or_skips_without_a_gpu() {
        let Some(ctx) = context() else {
            eprintln!("no GPU adapter — skipping (this is a valid outcome)");
            return;
        };
        let src: Vec<f64> = (0..1000).map(|i| i as f64 * 0.5).collect();
        let mut params = 3.0f32.to_le_bytes().to_vec();
        params.extend_from_slice(&(src.len() as f32).to_le_bytes());
        let bytes = ctx
            .dispatch(
                "test-scale",
                SCALE_WGSL,
                &[&f64_to_f32_bytes(&src)],
                (src.len() * 4) as u64,
                &params,
                src.len() as u32,
            )
            .expect("dispatch on a present adapter");
        let got = bytes_to_f64_vec(&bytes);
        assert_eq!(got.len(), src.len());
        for (i, (g, s)) in got.iter().zip(&src).enumerate() {
            assert!(
                (g - s * 3.0).abs() <= 1e-5 * (s * 3.0).abs().max(1.0),
                "lane {i}: got {g}, want {}",
                s * 3.0
            );
        }
    }

    #[test]
    fn pipeline_cache_reuses_one_compilation() {
        let Some(ctx) = context() else { return };
        let src = vec![1.0f64; 64];
        let mut params = 2.0f32.to_le_bytes().to_vec();
        params.extend_from_slice(&(src.len() as f32).to_le_bytes());
        let mut sizes = Vec::new();
        for _ in 0..3 {
            ctx.dispatch(
                "test-scale-cache",
                SCALE_WGSL,
                &[&f64_to_f32_bytes(&src)],
                (src.len() * 4) as u64,
                &params,
                src.len() as u32,
            )
            .expect("repeat dispatch");
            sizes.push(ctx.pipelines.read().unwrap().len());
        }
        // The cache is process-wide, so sibling tests contribute their own
        // labels; what this asserts is that *this* label is compiled once and
        // that repeat calls add nothing further of their own.
        assert!(
            ctx.pipelines
                .read()
                .unwrap()
                .contains_key("test-scale-cache"),
            "the label must be cached after the first dispatch"
        );
        assert_eq!(
            sizes[1], sizes[2],
            "a repeat dispatch of an already-compiled label must not grow the cache"
        );
    }

    #[test]
    fn round_trip_through_f32_is_within_f32_epsilon() {
        let src = vec![1.0, -2.5, 1e6, 1e-6, std::f64::consts::PI];
        let back = bytes_to_f64_vec(&f64_to_f32_bytes(&src));
        for (a, b) in src.iter().zip(&back) {
            assert!((a - b).abs() <= 1.2e-7 * a.abs().max(1.0));
        }
    }

    #[test]
    fn u32_round_trip_is_lossless() {
        let src = vec![0u32, 1, 4_294_967_295, 12_345];
        assert_eq!(bytes_to_u32_vec(&u32_to_bytes(&src)), src);
    }

    #[test]
    fn empty_input_is_padded_not_rejected() {
        let Some(ctx) = context() else { return };
        let mut params = 1.0f32.to_le_bytes().to_vec();
        params.extend_from_slice(&0.0f32.to_le_bytes());
        // Zero lanes, empty input: must not panic on a zero-sized buffer.
        let out = ctx.dispatch("test-scale-empty", SCALE_WGSL, &[&[]], 4, &params, 0);
        assert!(out.is_ok(), "empty dispatch must be handled, got {out:?}");
    }
}
