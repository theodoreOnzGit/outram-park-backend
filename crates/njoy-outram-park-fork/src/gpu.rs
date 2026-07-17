//! Optional GPU acceleration for the embarrassingly-parallel WMP curve-fit
//! background evaluation, with a mandatory CPU fallback.
//!
//! # What this computes
//!
//! The demonstrator kernel is the **windowed-multipole (WMP) curve-fit
//! background** cross section — the raw (non-Doppler-broadened) polynomial
//! branch of [`crate::wmp::WindowedMultipole::evaluate`]. For a coefficient
//! vector `coeffs = [c0, c1, c2, …]` and an incident neutron energy `E` (eV),
//! the background cross section is the polynomial in powers of `√E`:
//!
//! ```text
//! sig(E) = c0·(1/E) + c1·(1/√E) + c2·1 + c3·√E + c4·E + …
//! ```
//!
//! computed by the running-term recurrence `term₀ = 1/E`, `acc += coeffs[i]·term`,
//! `term ×= √E`. It is per-energy independent, so evaluating it across a whole
//! energy grid is an embarrassingly-parallel workload — the reason it is a good
//! first GPU target.
//!
//! # Contract (read before use)
//!
//! 1. **Target-gated backend.** The `wgpu` GPU backend is compiled **only on
//!    desktop** (`cfg(not(target_os = "android"))`), per the workspace Android
//!    rule — Android has no system Vulkan/Metal/DX12, so there it is a pure-CPU
//!    shim: [`GpuContext`] is an empty struct that is never constructed and
//!    [`probe`] always returns `None`.
//!
//! 2. **Runtime CPU fallback is mandatory.** `probe() == None` is a normal,
//!    expected outcome (headless CI, this worktree, Android, no GPU) — it means
//!    "the caller runs the CPU path", **not** an error. Nothing here ever panics
//!    or returns `Err` on a missing adapter.
//!
//! 3. **CPU is the trusted, deterministic reference.**
//!    [`curvefit_background_batch_cpu`] runs in `f64` and mirrors the WMP
//!    evaluator exactly; it is available on **all** targets (Android included)
//!    and is what verification & validation is judged against. The GPU path
//!    ([`GpuContext::curvefit_background_batch`]) runs in `f32` (WGSL has no
//!    `f64` by default), so its float reduction order will **not** bit-match the
//!    CPU path — the GPU is *acceleration only*; V&V stays on the CPU.
//!
//! 4. **Scope of *this* module vs the full-fidelity kernel.** This module
//!    (`gpu`) is the original demonstrator: it evaluates only the curve-fit
//!    background polynomial on the GPU, for a single cross-section channel. The
//!    **full windowed-multipole sum** — the complex Faddeeva pole contributions
//!    over each window's pole range, for all three channels — lives in the
//!    sibling module [`crate::gpu_wmp`] (bead `op-0m5`), which reuses this
//!    module's [`GpuContext`]/[`probe`]. Read [`crate::gpu_wmp`] for the
//!    complete GPU WMP evaluator; read *this* module for the background-only
//!    demonstrator it grew out of.

// ---------------------------------------------------------------------------
// CPU reference — available on ALL targets (including Android). This is the
// trusted/deterministic path; the GPU path only accelerates it.
// ---------------------------------------------------------------------------

/// Evaluate the WMP curve-fit background cross section across an energy grid, in
/// `f64`, on the CPU. **This is the trusted, deterministic reference.**
///
/// For each incident energy `E` in `energies_ev` (in eV) this computes the
/// polynomial in powers of `√E`
///
/// ```text
/// sig(E) = coeffs[0]·(1/E) + coeffs[1]·(1/√E) + coeffs[2]·1 + coeffs[3]·√E + …
/// ```
///
/// via the running-term recurrence used by the WMP evaluator: `term` starts at
/// `1/E`, and after accumulating `coeffs[i]·term` it is multiplied by `√E`.
///
/// This mirrors the raw, **non-Doppler-broadened** curve-fit `else`-branch of
/// [`crate::wmp::WindowedMultipole::evaluate`] (in `src/wmp.rs`, the branch that
/// evaluates `a/E + b/√E + c + d·√E + …` directly when `sqrt_kt == 0`), reduced
/// to a single cross-section channel for a self-contained demonstrator. Note the
/// full evaluator carries three channels (scatter / absorption / fission) per
/// coefficient; here a coefficient vector drives one channel.
///
/// # Units
/// - `energies_ev`: incident neutron energy, electron-volts (eV).
/// - `coeffs`: curve-fit coefficients (barn·eV, barn·eV^{1/2}, barn, … — the
///   units of successive coefficients differ by a factor of eV per the `√E`
///   powers). Output is in barn.
///
/// # Assumptions
/// - `E > 0` for every entry (the `1/E` and `1/√E` terms diverge at `E = 0`).
///   The function does not guard against `E <= 0`; a non-positive energy yields
///   a non-finite result, matching the raw evaluator's behaviour.
/// - An empty `coeffs` yields `0.0` for every energy.
///
/// # Returns
/// A `Vec<f64>` of the same length as `energies_ev`, in the same order.
pub fn curvefit_background_batch_cpu(energies_ev: &[f64], coeffs: &[f64]) -> Vec<f64> {
    energies_ev
        .iter()
        .map(|&e| {
            let sqrt_e = e.sqrt();
            let mut term = 1.0 / e;
            let mut acc = 0.0;
            for &c in coeffs {
                acc += c * term;
                term *= sqrt_e;
            }
            acc
        })
        .collect()
}

// ---------------------------------------------------------------------------
// GPU context — present on ALL targets so public signatures compile everywhere.
// On desktop it owns the wgpu device+queue; on Android it is an empty shim that
// is never constructed.
// ---------------------------------------------------------------------------

/// A GPU compute context: an initialised `wgpu` device + queue on desktop, or an
/// empty CPU-only shim on Android.
///
/// Obtain one with [`probe`], which returns `None` when no GPU is available (the
/// caller then uses [`curvefit_background_batch_cpu`]). On desktop, a live
/// context can run [`GpuContext::curvefit_background_batch`].
///
/// The struct owns its `wgpu::Device` and `wgpu::Queue` **by value** (no `Arc`,
/// no lifetimes) — a context is created once and used from the thread that owns
/// it.
#[cfg(not(target_os = "android"))]
#[derive(Debug)]
pub struct GpuContext {
    /// The logical GPU device (owns pipelines, buffers, bind groups).
    ///
    /// `pub(crate)` so the full-fidelity WMP kernel in [`crate::gpu_wmp`] can
    /// dispatch on the same context obtained from [`probe`].
    pub(crate) device: wgpu::Device,
    /// The command queue used to submit compute work to `device`.
    ///
    /// `pub(crate)` so [`crate::gpu_wmp`] can submit to the same queue.
    pub(crate) queue: wgpu::Queue,
    /// The adapter's identifying info (name + backend), captured at [`probe`]
    /// time. Used by [`GpuContext::adapter_label`] to label per-machine
    /// performance reports (see [`crate::perf_report`]).
    pub(crate) info: wgpu::AdapterInfo,
}

#[cfg(not(target_os = "android"))]
impl GpuContext {
    /// A short human label for the GPU this context runs on, e.g.
    /// `"NVIDIA GeForce RTX 3050 / Vulkan"` — the adapter name and backend from
    /// `wgpu::Adapter::get_info()`.
    ///
    /// Used to head a per-machine performance report so a reader knows which
    /// hardware produced the numbers (GPU/CPU timings differ per machine). See
    /// [`crate::perf_report`].
    pub fn adapter_label(&self) -> String {
        format!("{} / {:?}", self.info.name, self.info.backend)
    }
}

/// A GPU compute context — **Android shim.** On Android there is no GPU backend
/// (no system Vulkan/Metal/DX12), so this is an empty struct that is never
/// constructed; [`probe`] always returns `None` and callers use the CPU path.
#[cfg(target_os = "android")]
#[derive(Debug)]
pub struct GpuContext {
    /// Private zero-sized field so the shim cannot be constructed outside this
    /// module (it never is — `probe()` returns `None` on Android).
    _cpu_only: (),
}

/// Probe for a usable headless GPU compute context.
///
/// Returns `Some(GpuContext)` if a GPU compute adapter and device could be
/// acquired, or `None` otherwise. **`None` is expected and not an error** — the
/// caller must fall back to [`curvefit_background_batch_cpu`].
///
/// - **Android** (`target_os = "android"`): always `None`. Android has no
///   system GPU compute backend available to this crate, so the crate stays
///   pure-CPU there per the workspace Android portability rule.
/// - **Desktop** (`cfg(not(target_os = "android"))`): creates a headless
///   `wgpu::Instance` (no window/surface), requests a compute adapter and a
///   device+queue, and returns `Some` on success. On **any** failure — no
///   adapter (common on headless CI / this worktree), or a device-request error
///   — it returns `None`. It never panics and never returns `Err`.
#[cfg(target_os = "android")]
pub fn probe() -> Option<GpuContext> {
    // No GPU backend on Android — always fall back to the CPU path.
    None
}

/// Probe for a usable headless GPU compute context (desktop implementation).
///
/// See the crate-level and Android-shim documentation on [`probe`]; this is the
/// desktop body. It blocks on the async wgpu adapter/device requests using the
/// small [`block_on`] thread-parking executor (no external async runtime).
#[cfg(not(target_os = "android"))]
pub fn probe() -> Option<GpuContext> {
    // `wgpu::Instance::default()` picks up the default backends without needing
    // a display handle (v29's `InstanceDescriptor` has no `Default`).
    let instance = wgpu::Instance::default();

    // Headless: no surface, so `compatible_surface: None`. `Default` supplies
    // the default power preference and `force_fallback_adapter: false`.
    let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).ok()?;

    // Capture the adapter identity (name + backend) before the device request so
    // per-machine performance reports can label the hardware.
    let info = adapter.get_info();

    let (device, queue) =
        block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).ok()?;

    Some(GpuContext { device, queue, info })
}

// ---------------------------------------------------------------------------
// GPU dispatch — desktop only.
// ---------------------------------------------------------------------------

/// WGSL compute shader for the curve-fit background polynomial.
///
/// Mirrors [`curvefit_background_batch_cpu`] in `f32`: one invocation per energy
/// grid point evaluates `Σ coeffs[i]·termᵢ` with `term₀ = 1/E`, `termᵢ₊₁ =
/// termᵢ·√E`. `Params` is padded to 16 bytes to satisfy uniform-buffer
/// alignment (matches the host `Params` struct byte layout).
#[cfg(not(target_os = "android"))]
const WGSL: &str = r#"
struct Params {
    n_coeff: u32,
    n_energy: u32,
    pad0: u32,
    pad1: u32,
};
@group(0) @binding(0) var<storage, read> energies: array<f32>;
@group(0) @binding(1) var<storage, read> coeffs: array<f32>;
@group(0) @binding(2) var<storage, read_write> result: array<f32>;
@group(0) @binding(3) var<uniform> params: Params;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let idx = gid.x;
    if (idx >= params.n_energy) {
        return;
    }
    let e = energies[idx];
    let sqrt_e = sqrt(e);
    var term = 1.0 / e;
    var acc = 0.0;
    for (var i = 0u; i < params.n_coeff; i = i + 1u) {
        acc = acc + coeffs[i] * term;
        term = term * sqrt_e;
    }
    result[idx] = acc;
}
"#;

#[cfg(not(target_os = "android"))]
impl GpuContext {
    /// Evaluate the WMP curve-fit background across `energies_ev` on the GPU, in
    /// `f32`.
    ///
    /// This uploads `energies_ev` and `coeffs` to storage buffers, dispatches
    /// the [`WGSL`] compute shader (one invocation per energy, `@workgroup_size(64)`),
    /// and reads the results back. It computes the same polynomial as
    /// [`curvefit_background_batch_cpu`] but in single precision.
    ///
    /// # Precision — GPU is acceleration only
    /// WGSL has no `f64` by default, so this runs in `f32`. The reduction order
    /// and rounding therefore will **not** bit-match the `f64` CPU reference;
    /// expect agreement only to a relative tolerance (~1e-3 in the crate tests).
    /// The CPU path remains the trusted/deterministic reference for V&V.
    ///
    /// # Units
    /// Same as [`curvefit_background_batch_cpu`]: `energies_ev` in eV, `coeffs`
    /// the curve-fit coefficients, output in barn. Returns a `Vec<f32>` the same
    /// length and order as `energies_ev`.
    ///
    /// # Scope — not the full WMP sum
    /// Only the curve-fit **background** polynomial is evaluated here. The full
    /// windowed-multipole sum (complex Faddeeva pole contributions over each
    /// window's pole range) is evaluated by the sibling full-fidelity kernel
    /// [`crate::gpu_wmp::GpuContext::wmp_evaluate_batch`] (bead `op-0m5`); this
    /// method remains the minimal single-channel background demonstrator.
    pub fn curvefit_background_batch(&self, energies_ev: &[f32], coeffs: &[f32]) -> Vec<f32> {
        use wgpu::util::DeviceExt;

        let n_energy = energies_ev.len() as u32;
        let n_coeff = coeffs.len() as u32;

        // Nothing to compute — avoid zero-sized buffer dispatch.
        if n_energy == 0 {
            return Vec::new();
        }

        // --- pack inputs to little-endian bytes (no bytemuck dependency) ------
        let energy_bytes = f32_slice_to_le_bytes(energies_ev);
        // A storage buffer must be non-empty; if there are no coefficients, use a
        // single zero so the buffer/binding is valid. `n_coeff == 0` means the
        // shader loop runs zero iterations, so the padding value is never read.
        let coeff_src: Vec<f32> = if coeffs.is_empty() { vec![0.0] } else { coeffs.to_vec() };
        let coeff_bytes = f32_slice_to_le_bytes(&coeff_src);
        // Params: n_coeff, n_energy, pad, pad -> 16 bytes.
        let mut param_bytes = Vec::with_capacity(16);
        param_bytes.extend_from_slice(&n_coeff.to_le_bytes());
        param_bytes.extend_from_slice(&n_energy.to_le_bytes());
        param_bytes.extend_from_slice(&0u32.to_le_bytes());
        param_bytes.extend_from_slice(&0u32.to_le_bytes());

        let output_size = (n_energy as u64) * 4;

        // --- buffers ----------------------------------------------------------
        let energy_buf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("wmp-curvefit-energies"),
            contents: &energy_bytes,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });
        let coeff_buf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("wmp-curvefit-coeffs"),
            contents: &coeff_bytes,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });
        let param_buf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("wmp-curvefit-params"),
            contents: &param_bytes,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let output_buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("wmp-curvefit-output"),
            size: output_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let staging_buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("wmp-curvefit-staging"),
            size: output_size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // --- bind group layout: 3 storage + 1 uniform ------------------------
        let storage_entry = |binding: u32, read_only: bool| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let bind_group_layout =
            self.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("wmp-curvefit-bgl"),
                entries: &[
                    storage_entry(0, true),  // energies (read)
                    storage_entry(1, true),  // coeffs   (read)
                    storage_entry(2, false), // result   (read_write)
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("wmp-curvefit-bg"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: energy_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: coeff_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: output_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 3, resource: param_buf.as_entire_binding() },
            ],
        });

        // --- shader + pipeline -----------------------------------------------
        let shader = self.device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("wmp-curvefit-shader"),
            source: wgpu::ShaderSource::Wgsl(WGSL.into()),
        });
        let pipeline_layout =
            self.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("wmp-curvefit-pl"),
                bind_group_layouts: &[Some(&bind_group_layout)],
                immediate_size: 0,
            });
        let pipeline = self.device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("wmp-curvefit-pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        // --- encode + submit --------------------------------------------------
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("wmp-curvefit-enc") });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("wmp-curvefit-pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            let workgroups = n_energy.div_ceil(64);
            pass.dispatch_workgroups(workgroups, 1, 1);
        }
        encoder.copy_buffer_to_buffer(&output_buf, 0, &staging_buf, 0, output_size);
        self.queue.submit(std::iter::once(encoder.finish()));

        // --- read back --------------------------------------------------------
        let slice = staging_buf.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        // Drive the map to completion (single-threaded, blocking).
        let _ = self.device.poll(wgpu::PollType::wait_indefinitely());

        let data = slice.get_mapped_range();
        let out = le_bytes_to_f32_vec(&data);
        drop(data);
        staging_buf.unmap();
        out
    }
}

// ---------------------------------------------------------------------------
// Byte-packing helpers (no bytemuck dependency) — desktop only, used by the GPU
// dispatch to (un)pack f32 arrays and the params buffer.
// ---------------------------------------------------------------------------

/// Pack a slice of `f32` into a little-endian byte buffer (4 bytes each), for
/// upload to a GPU storage buffer. Avoids a `bytemuck` dependency.
///
/// `pub(crate)` so the full-fidelity WMP kernel in [`crate::gpu_wmp`] reuses the
/// same packing rather than duplicating it.
#[cfg(not(target_os = "android"))]
pub(crate) fn f32_slice_to_le_bytes(values: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(values.len() * 4);
    for &v in values {
        bytes.extend_from_slice(&v.to_le_bytes());
    }
    bytes
}

/// Unpack a little-endian byte buffer (read back from the GPU) into a `Vec<f32>`.
/// Any trailing bytes that do not form a complete `f32` are ignored.
///
/// `pub(crate)` so [`crate::gpu_wmp`] reuses the same unpacking.
#[cfg(not(target_os = "android"))]
pub(crate) fn le_bytes_to_f32_vec(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

// ---------------------------------------------------------------------------
// Minimal blocking executor — desktop only. wgpu's request_adapter /
// request_device / map_async are async; we have no external runtime (pollster is
// not a dependency), so this drives a single future to completion by parking the
// current thread until the waker unparks it.
// ---------------------------------------------------------------------------

/// Block the current thread until `fut` resolves, returning its output.
///
/// A tiny `std`-only executor: it polls `fut` on the current thread and, on
/// `Poll::Pending`, parks the thread until the future's waker unparks it. Used
/// only by [`probe`] to drive wgpu's async adapter/device requests — the crate
/// takes on no external async-runtime dependency. Not for general concurrency:
/// it runs exactly one future and does no work while parked.
#[cfg(not(target_os = "android"))]
fn block_on<F: core::future::Future>(fut: F) -> F::Output {
    use core::pin::Pin;
    use core::task::{Context, Poll, Waker};
    use std::sync::Arc;
    use std::task::Wake;

    /// Waker that unparks the thread that created it.
    struct ThreadWaker(std::thread::Thread);
    impl Wake for ThreadWaker {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
        fn wake_by_ref(self: &Arc<Self>) {
            self.0.unpark();
        }
    }

    let waker = Waker::from(Arc::new(ThreadWaker(std::thread::current())));
    let mut cx = Context::from_waker(&waker);

    // Pin the future to the stack; it is not moved after this point.
    let mut fut = fut;
    // SAFETY: `fut` lives on this stack frame and is never moved again before
    // it completes below; we only ever access it through this pin.
    let mut fut = unsafe { Pin::new_unchecked(&mut fut) };

    loop {
        match fut.as_mut().poll(&mut cx) {
            Poll::Ready(value) => return value,
            Poll::Pending => std::thread::park(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verification: the CPU curve-fit evaluator matches a hand-computed value.
    ///
    /// # Methodology
    /// Reference is a closed-form hand calculation of the WMP curve-fit
    /// background polynomial for a single known input, judged against the `f64`
    /// [`curvefit_background_batch_cpu`] output with an absolute tolerance of
    /// `1e-12`. Inputs: `E = 4.0 eV`, `coeffs = [2.0, 3.0, 5.0]`.
    ///
    /// Hand calculation with `√E = 2`, `term₀ = 1/E = 0.25`:
    /// - i=0: acc += 2.0 · 0.25          = 0.5   ; term → 0.25·2 = 0.5
    /// - i=1: acc += 3.0 · 0.5   (+1.5)  = 2.0   ; term → 0.5·2  = 1.0
    /// - i=2: acc += 5.0 · 1.0   (+5.0)  = 7.0
    ///
    /// so `sig(4.0) = 7.0` exactly.
    ///
    /// # Results (2026-07-17, code-defined inputs)
    /// The evaluator returns `7.0` within `1e-12` — the recurrence and channel
    /// accumulation are implemented correctly for this case.
    #[test]
    fn curvefit_cpu_matches_manual() {
        let energies = [4.0_f64];
        let coeffs = [2.0_f64, 3.0, 5.0];
        let got = curvefit_background_batch_cpu(&energies, &coeffs);
        assert_eq!(got.len(), 1);
        assert!(
            (got[0] - 7.0).abs() < 1e-12,
            "expected 7.0, got {}",
            got[0]
        );
    }

    /// Verification: the GPU (`f32`) path agrees with the CPU (`f64`) reference,
    /// or the test skips cleanly when no GPU is present.
    ///
    /// # Methodology
    /// Runs both [`curvefit_background_batch_cpu`] (`f64`, trusted) and
    /// [`GpuContext::curvefit_background_batch`] (`f32`, WGSL) over the same
    /// 1000-point energy grid (`E` from 1 to 1000 eV) and a small coefficient
    /// vector `[1.5, -0.5, 0.25, 2.0]`, and asserts every GPU value matches the
    /// CPU value within a **relative** tolerance of `1e-3`. A relative bound is
    /// used because the GPU runs in single precision (`f32`) while the reference
    /// runs in double precision (`f64`), so bit-exact agreement is not expected;
    /// the GPU is acceleration only.
    ///
    /// When [`probe`] returns `None` — the expected state on headless CI, this
    /// worktree, and Android — the test prints a SKIP notice and returns
    /// success. A missing GPU adapter is not a test failure.
    ///
    /// # Results (2026-07-17)
    /// On the development box a software Vulkan adapter (Mesa **llvmpipe**) was
    /// available, so `probe()` returned `Some` and the GPU path **ran**: all
    /// 1000 `f32` GPU values agreed with the `f64` CPU reference within the
    /// `1e-3` relative tolerance (no SKIP notice printed; test passed in ~0.7 s).
    /// On a genuinely headless box with no adapter `probe()` returns `None` and
    /// the test records a SKIP instead — either outcome is a pass, never a
    /// failure.
    #[test]
    fn gpu_agrees_with_cpu_or_skips() {
        let Some(ctx) = probe() else {
            eprintln!("SKIP: no GPU adapter (CPU-only env) — GPU path not exercised");
            return;
        };

        // Desktop-only body: `probe()` only returns `Some` on desktop.
        #[cfg(not(target_os = "android"))]
        {
            let energies_f64: Vec<f64> = (0..1000).map(|i| 1.0 + i as f64).collect();
            let energies_f32: Vec<f32> = energies_f64.iter().map(|&e| e as f32).collect();
            let coeffs_f64 = [1.5_f64, -0.5, 0.25, 2.0];
            let coeffs_f32: Vec<f32> = coeffs_f64.iter().map(|&c| c as f32).collect();

            let cpu = curvefit_background_batch_cpu(&energies_f64, &coeffs_f64);
            let gpu = ctx.curvefit_background_batch(&energies_f32, &coeffs_f32);

            assert_eq!(gpu.len(), cpu.len(), "GPU/CPU length mismatch");
            for (i, (&g, &c)) in gpu.iter().zip(cpu.iter()).enumerate() {
                // Relative tolerance: f32 GPU vs f64 CPU reference.
                let denom = c.abs().max(1e-30);
                let rel = (g as f64 - c).abs() / denom;
                assert!(
                    rel < 1e-3,
                    "GPU/CPU disagree at i={i}: gpu={g} cpu={c} rel={rel}"
                );
            }
        }
        // Suppress unused-variable warning on the (unreachable) android build.
        #[cfg(target_os = "android")]
        let _ = ctx;
    }
}
