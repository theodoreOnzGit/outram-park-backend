//! Headless GPU execution of the WGSL kernels in [`crate::wgsl`].
//!
//! **Behind the off-by-default `wgpu` feature.** This is the only module in
//! PETIR that uses `std`; everything else, including the shader source and the
//! `f32` mirrors, is `no_std` and compiles for bare metal.
//!
//! # Why this is a feature and not a test helper
//!
//! Because a caller wants it. The shader text alone leaves every consumer to
//! write the same 150 lines of buffer plumbing, and they would each get the
//! bind-group layout subtly wrong in the same way — see
//! [`GpuContext::eval_map`]'s note on unused bindings, which is a real trap
//! this module already fell into once.
//!
//! # The `std` gate is load-bearing
//!
//! `lib.rs` reads `#[cfg(any(test, feature = "wgpu"))] extern crate std;`. The
//! `feature` arm must stay: the removed `platform-libm` feature declared a
//! `std` feature while that line was gated on `cfg(test)` alone, so `std`
//! never arrived and the feature had been dead since the crate went `no_std`
//! (`bn:op-7kwt`). `cargo check -p petir --all-features --all-targets` is the
//! sweep that caught that and the one that keeps this honest.
//!
//! # No adapter is not an error
//!
//! [`GpuContext::probe`] returns `None` on a host with no usable adapter —
//! headless CI, no Vulkan loader, a locked-down sandbox. **Callers must treat
//! that as "run the CPU path"**, never as a failure; [`crate::wgsl::mirror`]
//! is that path and gives the same answers to the `f32` budget. This follows
//! the contract `outram-mc-libs::gpu` already established in this workspace.
//!
//! # `f32`, and what that costs
//!
//! A baseline WebGPU device is guaranteed `f32` and nothing wider, so these
//! kernels are a *different numerical object* from PETIR's `f64` routines, not
//! a recompilation of them. Judge them against an `f32` budget: ~1e-7 for a
//! single operation and worse for a composed one. The measured figures are in
//! `tests/wgsl_gpu.rs`.

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use std::future::Future;
use std::pin::pin;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};
use std::thread;

use crate::wgsl::test_kernel;

/// Block on a future without an async runtime.
///
/// A `Wake`-based thread park, the same shape as `pollster::block_on`. This
/// mirrors the approach already taken by `outram-mc-libs::gpu` in this
/// workspace, and for the same reason: `wgpu`'s `request_adapter` and
/// `request_device` return futures, and neither crate wants an executor as a
/// dependency for two calls.
fn block_on<F: Future>(future: F) -> F::Output {
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

/// Uniform parameters passed to a generated kernel.
///
/// Matches `struct Params` in [`crate::wgsl::test_kernel`] field for field.
/// The names are deliberately generic because one struct serves every kernel:
/// `n` is an array length, `a`/`b` an interval, `k` an order or index. Each
/// call site says which it means.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KernelParams {
    /// Length of the `src` array the kernel should read, in elements.
    pub n: u32,
    /// Lower bound of an interval, where the kernel takes one.
    pub a: f32,
    /// Upper bound of an interval, where the kernel takes one.
    pub b: f32,
    /// An order, degree or index, where the kernel takes one.
    pub k: u32,
}

impl Default for KernelParams {
    /// All zero: no array, no interval, order zero.
    fn default() -> Self {
        KernelParams {
            n: 0,
            a: 0.0,
            b: 0.0,
            k: 0,
        }
    }
}

/// A headless compute device.
///
/// Obtain one with [`GpuContext::probe`], which returns `None` rather than
/// failing when the host has no adapter.
pub struct GpuContext {
    device: wgpu::Device,
    queue: wgpu::Queue,
    name: String,
}

impl core::fmt::Debug for GpuContext {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("GpuContext")
            .field("adapter", &self.name)
            .finish()
    }
}

impl GpuContext {
    /// Acquire a headless compute device, or `None` when none is usable.
    ///
    /// `None` means **run the CPU path** ([`crate::wgsl::mirror`]), not
    /// "something went wrong". A headless server, a container with no Vulkan
    /// loader, or a sandbox with no device all land here legitimately.
    pub fn probe() -> Option<Self> {
        let instance = wgpu::Instance::default();
        let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            force_fallback_adapter: false,
            compatible_surface: None,
            apply_limit_buckets: false,
        }))
        .ok()?;
        let name = adapter.get_info().name;
        let (device, queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("petir wgsl compute device"),
            required_features: wgpu::Features::empty(),
            required_limits: adapter.limits(),
            ..Default::default()
        }))
        .ok()?;
        Some(GpuContext {
            device,
            queue,
            name,
        })
    }

    /// The adapter's reported name, for diagnostics and for recording which
    /// device a measurement was taken on.
    ///
    /// Worth recording: WGSL bounds builtin accuracy in ULP rather than
    /// requiring correct rounding, and permits `a * b + c` to be contracted
    /// into an FMA, so two conforming devices can legitimately disagree in the
    /// last place. A measured number without the device it came from is not
    /// reproducible.
    pub fn adapter_name(&self) -> &str {
        &self.name
    }

    /// Evaluate a WGSL expression once per element of `probe`, returning one
    /// `f32` per element.
    ///
    /// `sources` are shader constants from [`crate::wgsl`] to include;
    /// `call` is a WGSL expression that may use `x` (the probe value), `src`
    /// (the `data` array) and `params`. `data` may be empty for a kernel that
    /// reads no array.
    ///
    /// # The unused-binding trap
    ///
    /// The generated kernel touches `src` via `arrayLength` even when `call`
    /// never reads it. Without that, naga strips the unused binding from the
    /// auto-generated layout and `create_bind_group` rejects the entries this
    /// method supplies. That is not hypothetical — `petir_legendre_p`, the one
    /// function here taking no array, hit exactly this.
    ///
    /// # Errors
    ///
    /// Returns `None` if the output would be empty (an empty `probe`), which
    /// is the one case that cannot produce a valid zero-sized buffer.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use petir::wgsl::{gpu::{GpuContext, KernelParams}, POLY};
    ///
    /// let gpu = GpuContext::probe().expect("a GPU");
    /// let coeffs = [1.0f32, 2.0, 3.0];
    /// let probes = [0.0f32, 1.0, 2.0];
    /// let out = gpu.eval_map(
    ///     &[POLY],
    ///     "petir_poly_eval(0u, params.n, x)",
    ///     &coeffs,
    ///     &probes,
    ///     KernelParams { n: 3, ..Default::default() },
    /// ).unwrap();
    /// assert!((out[2] - 17.0).abs() < 1e-5);
    /// ```
    pub fn eval_map(
        &self,
        sources: &[&str],
        call: &str,
        data: &[f32],
        probe: &[f32],
        params: KernelParams,
    ) -> Option<Vec<f32>> {
        use wgpu::util::DeviceExt;

        if probe.is_empty() {
            return None;
        }

        let shader_src = test_kernel(sources, call);
        let module = self
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some(call),
                source: wgpu::ShaderSource::Wgsl(shader_src.into()),
            });

        // A zero-length storage buffer is invalid; pad to one element. The
        // kernel's `arrayLength(&src) == 0u` guard is what keeps the binding
        // live, so the padding is never read by a well-formed `call`.
        let data_padded: Vec<f32> = if data.is_empty() {
            vec![0.0]
        } else {
            data.to_vec()
        };

        let src_buf = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("petir src"),
                contents: &f32_bytes(&data_padded),
                usage: wgpu::BufferUsages::STORAGE,
            });
        let probe_buf = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("petir probe"),
                contents: &f32_bytes(probe),
                usage: wgpu::BufferUsages::STORAGE,
            });
        let out_size = (probe.len() * core::mem::size_of::<f32>()) as u64;
        let dst_buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("petir dst"),
            size: out_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let params_buf = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("petir params"),
                contents: &params.to_le_bytes(),
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let read_buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("petir readback"),
            size: out_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let pipeline = self
            .device
            .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("petir wgsl kernel"),
                layout: None,
                module: &module,
                entry_point: Some("main"),
                compilation_options: Default::default(),
                cache: None,
            });

        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: src_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: probe_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: dst_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: params_buf.as_entire_binding(),
                },
            ],
        });

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: None,
                timestamp_writes: None,
            });
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            let groups = probe.len().div_ceil(64) as u32;
            pass.dispatch_workgroups(groups, 1, 1);
        }
        encoder.copy_buffer_to_buffer(&dst_buf, 0, &read_buf, 0, out_size);
        self.queue.submit(Some(encoder.finish()));

        let slice = read_buf.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        self.device.poll(wgpu::PollType::wait_indefinitely()).ok()?;
        let mapped = slice.get_mapped_range().ok()?;
        // `try_into` rather than `[c[0], c[1], ...]`: `chunks_exact(4)` does
        // yield 4-byte slices, but that is an invariant the compiler cannot
        // see, so the subscripts would be runtime-checked -- which
        // `tests/no_panic_gate.rs` rejects, rightly. The `filter_map` drops a
        // short tail that `chunks_exact` cannot produce anyway.
        let out: Vec<f32> = mapped
            .chunks_exact(4)
            .filter_map(|c| <[u8; 4]>::try_from(c).ok())
            .map(f32::from_le_bytes)
            .collect();
        drop(mapped);
        read_buf.unmap();
        Some(out)
    }
}

/// Little-endian bytes of an `f32` slice, for buffer upload.
///
/// Built element by element rather than by reinterpreting the slice's memory.
/// This crate forbids `unsafe`, and the transmute would buy nothing: the copy
/// is one pass over a buffer that is about to cross a PCIe boundary, which is
/// several orders of magnitude more expensive.
///
/// WGSL storage buffers are little-endian, so `to_le_bytes` is correct on a
/// big-endian host too — where a reinterpret-cast would silently produce
/// garbage.
fn f32_bytes(v: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(v.len() * 4);
    for x in v {
        out.extend_from_slice(&x.to_le_bytes());
    }
    out
}

impl KernelParams {
    /// Little-endian bytes, matching `struct Params` in the generated kernel.
    ///
    /// The field order here **is** the WGSL struct's field order; changing one
    /// without the other silently feeds a kernel the wrong numbers, which is
    /// why they are written adjacent to each other in this file and the
    /// generator.
    fn to_le_bytes(self) -> Vec<u8> {
        let mut out = Vec::with_capacity(16);
        out.extend_from_slice(&self.n.to_le_bytes());
        out.extend_from_slice(&self.a.to_le_bytes());
        out.extend_from_slice(&self.b.to_le_bytes());
        out.extend_from_slice(&self.k.to_le_bytes());
        out
    }
}
