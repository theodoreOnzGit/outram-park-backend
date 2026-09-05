//! Optional GPU compute for the embarrassingly-parallel Monte Carlo kernels.
//!
//! This module provides *headless* (no window, no surface) GPU acceleration via
//! [`wgpu`] for the branch-light, data-parallel sub-kernels of neutron transport
//! — starting with **batched pointwise cross-section interpolation** (see
//! [`xs_interp`]). It is **acceleration only**; correctness never depends on it.
//!
//! ## Non-negotiable contract (read before using this module)
//!
//! 1. **Compiles always, runs on CPU when there is no GPU.** [`GpuContext`] and
//!    [`probe`] are the desktop (real `wgpu`) path; on Android — where `wgpu` is
//!    target-gated out in `Cargo.toml` — a CPU-only shim gives [`probe`] the same
//!    `-> Option<GpuContext>` signature returning `None`, so call sites compile
//!    unchanged. On desktop/CI, [`probe`] returns `None` whenever no usable GPU
//!    adapter exists (headless servers, no Vulkan loader); callers **must** treat
//!    `None` as "run the CPU path", never as an error.
//! 2. **CPU is the trusted / deterministic reference.** The transport loop is raw
//!    `f64` (see this crate's `CLAUDE.md`); the GPU path runs `f32` and its
//!    floating-point reduction/rounding order will **not** bit-match the CPU. So
//!    anything feeding V&V or a solver stays on the CPU path. GPU results are only
//!    ever compared to CPU *within a tolerance*, never trusted as the reference.
//! 3. **No new third-party dependency.** `wgpu`'s `request_adapter` /
//!    `request_device` return futures; rather than pull in an async runtime this
//!    module hand-rolls a tiny pure-`std` [`block_on`] (a `Wake`-based thread
//!    park, the same shape as `pollster::block_on`). Buffer read-back does not
//!    need it — it uses `Device::poll(PollType::wait_indefinitely())`.
//!
//! ## What belongs here / what does not
//!
//! - **Belongs:** headless compute contexts, WGSL kernels for embarrassingly
//!   parallel MC sub-kernels (XS interpolation, majorant evaluation, batched
//!   free-flight sampling), dense-grid material-total tabulations that reuse
//!   those kernels ([`union_grid`]), and their CPU reference + GPU-vs-CPU
//!   agreement tests.
//! - **Does not:** the history-based transport loop itself (branchy, not GPU
//!   friendly), any windowing/GUI (out of scope for the library; Android-hostile),
//!   or anything that would make a plain `cargo build` require a GPU at runtime.

// The kernel module is compiled on *every* target: its CPU reference
// implementation is pure `f32`/`f64` Rust with no `wgpu`, so it builds (and is
// tested) on Android too. Only the GPU-dispatch function and the `wgpu` imports
// inside it are `#[cfg(not(target_os = "android"))]`.
// Runtime hardware capability sourcing (adapter limits + CPU threads) and the
// CPU/GPU work-splitting planner. Plain data only — no `wgpu` types escape it —
// so it builds and is tested on Android too; only `GpuLimits::from_adapter` is
// `#[cfg]`-gated.
pub mod capabilities;

pub mod xs_interp;

// Ray–surface distance for all 15 CSG SurfaceKind variants (op-9s8.2): the
// geometric foundation for GPU CSG transport. Encoding + scalar f32 CPU mirror
// (built and tested on every target, incl. Android) + WGSL compute path (the
// `*_gpu` function and its `wgpu` imports are `#[cfg(not(target_os = "android"))]`).
pub mod surface_distance;

// Dense log-spaced tabulation of a material's macroscopic total cross section,
// with batched CPU-reference + GPU lookup reusing the `xs_interp` kernel. Builds
// on every target; its GPU-only method is `#[cfg]`-gated internally.
pub mod union_grid;

// Batched next-event free-flight kernel: one flight (event) advanced in parallel
// for a resident batch of neutrons. CPU mirror (same f32 path) + GPU compute.
// The CPU mirror and SoA structs build on every target; the `*_gpu` function and
// its `wgpu` imports are `#[cfg(not(target_os = "android"))]`.
pub mod batched_flight;

// Per-nuclide reaction cross sections tabulated on the shared union grid — the
// data the fused collision kernel needs. Pure CPU tabulation; builds everywhere.
pub mod collision_grid;

// Fused flight + collision event kernel (op-u6s.8): a whole batch advances through
// every event of a generation resident in GPU buffers, with the branchy collision
// physics on the GPU. CPU mirror (same f32 path) + GPU compute + resident driver.
// The mirror, SoA batch, and packed tables build on every target; the `*_gpu`
// driver and its `wgpu` imports are `#[cfg(not(target_os = "android"))]`.
pub mod batched_event;

// ---------------------------------------------------------------------------
// Desktop (non-Android) path: the real wgpu-backed context + probe.
// ---------------------------------------------------------------------------
#[cfg(not(target_os = "android"))]
mod context {
    use std::future::Future;
    use std::pin::pin;
    use std::sync::Arc;
    use std::task::{Context, Poll, Wake, Waker};
    use std::thread;

    /// A live, headless GPU compute context: a [`wgpu::Device`] + [`wgpu::Queue`]
    /// obtained without any window or surface.
    ///
    /// Obtain one with [`probe`]. Holding a `GpuContext` means a usable compute
    /// adapter was found and a device was created on it; kernels (e.g.
    /// [`super::xs_interp::interp_xs_gpu`]) borrow `device` and `queue` to build
    /// pipelines, upload buffers, dispatch, and read results back.
    ///
    /// `device` and `queue` are public so acceleration kernels in submodules and
    /// downstream callers can drive them directly. `info` is kept for diagnostics
    /// (which adapter/backend was selected) and for test logging.
    pub struct GpuContext {
        /// The logical GPU device — creates buffers, shaders, pipelines, and is
        /// polled to completion for buffer read-back.
        pub device: wgpu::Device,
        /// The command queue — submits encoded compute passes and buffer writes.
        pub queue: wgpu::Queue,
        /// Which physical adapter/backend was selected (name, backend, device
        /// type). Diagnostic only.
        pub info: wgpu::AdapterInfo,
        /// Compute limits **as reported by this adapter**, sourced at probe time
        /// — not assumed. Check these before building a bind-group layout; see
        /// [`super::capabilities::GpuLimits::supports_storage_buffers`].
        pub limits: super::capabilities::GpuLimits,
    }

    impl GpuContext {
        /// This machine's capabilities: the adapter's limits plus the CPU thread
        /// count, ready to hand to
        /// [`super::capabilities::plan_split`].
        pub fn capabilities(&self) -> super::capabilities::HardwareCapabilities {
            super::capabilities::HardwareCapabilities::with_gpu(Some(self.limits.clone()))
        }
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

    /// Probe for a usable **headless compute** GPU and return a live
    /// [`GpuContext`], or `None` when the caller must fall back to the CPU path.
    ///
    /// Returns `None` (⇒ run CPU) when no adapter is present — headless servers, a
    /// container with no Vulkan/Metal loader, or CI without a GPU — or when device
    /// creation fails. This is the *normal, expected* outcome in most CI, and is
    /// never an error: every kernel here has a CPU reference implementation that
    /// runs whenever this returns `None`.
    ///
    /// A default power preference is requested (no window, so
    /// `compatible_surface: None`), which is all a compute-only workload needs.
    ///
    /// # Limits are sourced, not assumed
    ///
    /// This requests `adapter.limits()` — **what the hardware actually reports**
    /// — rather than [`wgpu::Limits::downlevel_defaults`].
    ///
    /// It used to request the downlevel defaults, which pin
    /// `max_storage_buffers_per_shader_stage` to **4**. The `surface_distance`
    /// kernel binds **7**, so `create_bind_group_layout` rejected `surf_dist.bgl`
    /// with *"Too many bindings of type StorageBuffers … limit is 4, count was
    /// 7"* on hardware that in fact allows 1,048,576 of them (bead `op-bjd`).
    /// The ceiling was self-imposed.
    ///
    /// Requesting the adapter's own limits is always satisfiable by definition,
    /// so this cannot fail where the old call succeeded. Kernels must still ask
    /// [`super::capabilities::GpuLimits::supports_storage_buffers`] before
    /// building a layout, because a *different* machine may genuinely be
    /// limited — the answer there is a CPU fallback, not a panic.
    pub fn probe() -> Option<GpuContext> {
        let instance = wgpu::Instance::default();
        let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            force_fallback_adapter: false,
            compatible_surface: None,
            // wgpu 30: limit bucketing is a fingerprinting mitigation for
            // untrusted content (e.g. a web browser exposing wgpu to a
            // page) — irrelevant to this native GPU-compute application,
            // and off is also the type's own `Default`.
            apply_limit_buckets: false,
        }))
        .ok()?;
        let info = adapter.get_info();
        let limits = super::capabilities::GpuLimits::from_adapter(&adapter);
        let (device, queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("outram-mc-libs headless compute device"),
            required_features: wgpu::Features::empty(),
            required_limits: adapter.limits(),
            ..Default::default()
        }))
        .ok()?;
        Some(GpuContext {
            device,
            queue,
            info,
            limits,
        })
    }

    /// Minimal pure-`std` executor: block the current thread until `future`
    /// resolves, driving it with a `Wake`-based thread-park waker.
    ///
    /// This is the standard `pollster::block_on` shape (park on `Pending`, `unpark`
    /// on `wake`). It exists so this crate needs **no** async-runtime dependency to
    /// await `wgpu`'s two setup futures (`request_adapter`, `request_device`). It is
    /// *not* used for buffer read-back — that uses `Device::poll(..Wait..)`.
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
}

#[cfg(not(target_os = "android"))]
pub use context::{block_on, probe, GpuContext};

// ---------------------------------------------------------------------------
// Android path: CPU-only shim so call sites compile unchanged (no wgpu there).
// ---------------------------------------------------------------------------
#[cfg(target_os = "android")]
mod context {
    /// An uninhabited stand-in for the desktop GPU context. It can never be
    /// constructed on Android (there is no `wgpu` backend compiled in), which is
    /// exactly the point: [`probe`] always returns `None`, so callers always run
    /// the CPU reference path in [`super::xs_interp`].
    #[derive(Debug)]
    pub enum GpuContext {}

    /// Android has no GPU compute backend compiled in — always returns `None`, so
    /// every caller runs the CPU reference path. Signature matches the desktop
    /// `probe` so call sites are identical across targets.
    pub fn probe() -> Option<GpuContext> {
        None
    }
}

#[cfg(target_os = "android")]
pub use context::{probe, GpuContext};
