//! Runtime hardware capability sourcing, and CPU/GPU work splitting.
//!
//! # Why this module exists
//!
//! [`crate::gpu::probe`] used to request [`wgpu::Limits::downlevel_defaults`],
//! which pins `max_storage_buffers_per_shader_stage` to **4** no matter what the
//! hardware can do. The `surface_distance` kernel binds **7** storage buffers,
//! so device creation validated fine but the bind-group layout was rejected:
//!
//! ```text
//! In Device::create_bind_group_layout, label = 'surf_dist.bgl'
//!   Too many bindings of type StorageBuffers in Stage ShaderStages(COMPUTE),
//!   limit is 4, count was 7.
//! ```
//!
//! That was self-inflicted, not a hardware limit. On the development machine
//! (NVIDIA RTX A5000, NVK, Vulkan) `wgpu` reports **524,288** per-stage storage
//! buffers — we were asking for 7 against a self-imposed ceiling of 4. (Raw
//! Vulkan `maxPerStageDescriptorStorageBuffers` is 1,048,576 there; `wgpu`
//! applies its own cap on top, and **`wgpu`'s number is the one that governs
//! us**.) The fix is to *source* limits from the adapter instead of assuming a
//! floor — hence this module, and the change to `probe`.
//!
//! # What is sourced, and from where
//!
//! Nothing here is hard-coded to a particular machine. Every number is read at
//! runtime:
//!
//! | Quantity | Source |
//! |---|---|
//! | storage buffers per stage, binding sizes, workgroup limits | `wgpu::Adapter::limits()` |
//! | adapter name / backend / discrete-vs-integrated | `wgpu::Adapter::get_info()` |
//! | CPU worker threads | [`std::thread::available_parallelism`] |
//!
//! # Android
//!
//! `wgpu` is target-gated off Android (see `Cargo.toml`). Every type here is
//! plain data — [`DeviceClass`] is our own enum, not `wgpu::DeviceType`, and the
//! backend is a `String` — so the whole module *including its tests* builds and
//! runs on `aarch64-linux-android`. Only [`GpuLimits::from_adapter`] touches
//! `wgpu` and is `#[cfg]`-gated. On Android [`crate::gpu::probe`] always returns
//! `None`, so [`HardwareCapabilities::with_gpu`] is built with `gpu: None`,
//! which every split policy already handles as "all CPU".

/// Broad class of a compute adapter, mirroring `wgpu::DeviceType` but without
/// depending on `wgpu` (so this module builds on Android).
///
/// Used by [`SplitPolicy::Auto`] to pick a starting GPU share: a discrete card
/// has its own memory bandwidth, an integrated one competes with the CPU cores
/// running the other half of the split, and a software adapter *is* the CPU.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceClass {
    /// Discrete card with its own VRAM and memory bandwidth.
    Discrete,
    /// Integrated GPU sharing system memory (and bandwidth) with the CPU.
    Integrated,
    /// Virtualised/paravirtual adapter.
    Virtual,
    /// Software rasteriser (e.g. lavapipe/SwiftShader) — this is the CPU wearing
    /// a GPU hat, and is normally *slower* than the native CPU path.
    Cpu,
    /// Anything the driver did not classify.
    Other,
}

/// Compute-relevant limits of one GPU adapter, read from the driver.
///
/// Construct from a live adapter with [`GpuLimits::from_adapter`], or by hand in
/// tests. All fields are exactly what the driver reported — none are clamped,
/// rounded, or defaulted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpuLimits {
    /// Adapter product name, e.g. `"NVIDIA RTX A5000 (NVK GA102)"`.
    pub adapter_name: String,
    /// Graphics backend, e.g. `"Vulkan"`.
    pub backend: String,
    /// Discrete / integrated / software.
    pub class: DeviceClass,
    /// Maximum storage buffers bindable in a single shader stage. **This is the
    /// limit that broke `surf_dist.bgl`** when it was pinned to the downlevel
    /// default of 4.
    pub max_storage_buffers_per_shader_stage: u32,
    /// Maximum bytes in one storage-buffer binding. Caps how many work items fit
    /// in a single dispatch — see [`GpuLimits::max_items_per_binding`].
    pub max_storage_buffer_binding_size: u64,
    /// Maximum bytes in any single buffer allocation.
    pub max_buffer_size: u64,
    /// Maximum threads per workgroup.
    pub max_compute_invocations_per_workgroup: u32,
    /// Maximum workgroups dispatchable along one dimension.
    pub max_compute_workgroups_per_dimension: u32,
}

impl GpuLimits {
    /// Can this device host a kernel that binds `n` storage buffers in one
    /// stage?
    ///
    /// Check this *before* building a bind-group layout: failing it means the
    /// caller must fall back to the CPU path (or pack buffers), not that the
    /// program should abort.
    #[inline]
    pub fn supports_storage_buffers(&self, n: u32) -> bool {
        self.max_storage_buffers_per_shader_stage >= n
    }

    /// How many items of `stride` bytes fit in one storage-buffer binding.
    ///
    /// Returns 0 for a zero stride (nothing sensible to report) — callers should
    /// treat 0 as "cannot dispatch, use the CPU".
    #[inline]
    pub fn max_items_per_binding(&self, stride: u64) -> usize {
        if stride == 0 {
            return 0;
        }
        (self.max_storage_buffer_binding_size / stride) as usize
    }

    /// How many invocations one dispatch can cover at the given workgroup size:
    /// `workgroup_size * max_compute_workgroups_per_dimension`, saturating.
    #[inline]
    pub fn max_invocations_per_dispatch(&self, workgroup_size: u32) -> usize {
        let wg = workgroup_size
            .min(self.max_compute_invocations_per_workgroup)
            .max(1);
        (wg as u64).saturating_mul(self.max_compute_workgroups_per_dimension as u64) as usize
    }

    /// Largest batch this device can process in one dispatch, respecting *both*
    /// the binding-size limit and the workgroup-count limit.
    ///
    /// This is the chunk size a caller should loop over for a batch larger than
    /// one dispatch can hold.
    #[inline]
    pub fn max_chunk_items(&self, stride: u64, workgroup_size: u32) -> usize {
        self.max_items_per_binding(stride)
            .min(self.max_invocations_per_dispatch(workgroup_size))
    }

    /// Build from a live `wgpu` adapter — the only function here that touches
    /// `wgpu`, and the only one absent on Android.
    #[cfg(not(target_os = "android"))]
    pub fn from_adapter(adapter: &wgpu::Adapter) -> Self {
        let info = adapter.get_info();
        let limits = adapter.limits();
        Self {
            adapter_name: info.name.clone(),
            backend: format!("{:?}", info.backend),
            class: match info.device_type {
                wgpu::DeviceType::DiscreteGpu => DeviceClass::Discrete,
                wgpu::DeviceType::IntegratedGpu => DeviceClass::Integrated,
                wgpu::DeviceType::VirtualGpu => DeviceClass::Virtual,
                wgpu::DeviceType::Cpu => DeviceClass::Cpu,
                wgpu::DeviceType::Other => DeviceClass::Other,
            },
            max_storage_buffers_per_shader_stage: limits.max_storage_buffers_per_shader_stage,
            max_storage_buffer_binding_size: limits.max_storage_buffer_binding_size as u64,
            max_buffer_size: limits.max_buffer_size,
            max_compute_invocations_per_workgroup: limits.max_compute_invocations_per_workgroup,
            max_compute_workgroups_per_dimension: limits.max_compute_workgroups_per_dimension,
        }
    }
}

/// What this machine can actually do: the GPU (if any) plus the CPU.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HardwareCapabilities {
    /// Limits of the selected adapter, or `None` when there is no usable GPU
    /// (headless CI, Android, no Vulkan loader). `None` means "all CPU".
    pub gpu: Option<GpuLimits>,
    /// Usable CPU threads, from [`std::thread::available_parallelism`]
    /// (falls back to 1 if the platform cannot report it).
    pub cpu_threads: usize,
}

impl HardwareCapabilities {
    /// Read CPU thread count; pair it with GPU limits you already have.
    pub fn with_gpu(gpu: Option<GpuLimits>) -> Self {
        Self {
            gpu,
            cpu_threads: cpu_threads(),
        }
    }

    /// True when a GPU exists *and* can host a kernel binding `n` storage
    /// buffers in one stage.
    pub fn gpu_supports_storage_buffers(&self, n: u32) -> bool {
        self.gpu
            .as_ref()
            .is_some_and(|g| g.supports_storage_buffers(n))
    }
}

/// Usable CPU threads, or 1 when the platform will not say.
pub fn cpu_threads() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
}

/// How to divide a batch between the GPU and the CPU.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SplitPolicy {
    /// Everything on the CPU — the trusted reference path.
    CpuOnly,
    /// Everything the GPU can take; remainder (if any) on the CPU.
    GpuOnly,
    /// An explicit GPU share in `[0, 1]`, clamped. Use this once
    /// [`measured_gpu_fraction`] has told you the real ratio for your workload.
    GpuFraction(f64),
    /// A capability-derived starting share — see [`SplitPolicy::auto_fraction`].
    Auto,
}

impl SplitPolicy {
    /// The GPU share this policy implies for the given hardware.
    ///
    /// # `Auto` is a heuristic, not a measurement
    ///
    /// The `Auto` constants below are **starting points chosen from device
    /// class, not from benchmarks on this workload**. They encode only what is
    /// defensible without measuring:
    ///
    /// - **No GPU** -> 0.0. Nothing else is possible.
    /// - **Software adapter** ([`DeviceClass::Cpu`]) -> 0.0. Such an adapter *is*
    ///   the CPU, reached through a driver; giving it work is strictly worse than
    ///   running the native CPU path.
    /// - **Integrated** -> 0.5. It shares memory bandwidth with the very cores
    ///   running the CPU half, so the two halves contend; an even split avoids
    ///   assuming either side wins.
    /// - **Discrete / virtual / other** -> 0.75. Independent VRAM and bandwidth,
    ///   so the GPU takes the majority — but the CPU keeps a real quarter of the
    ///   work rather than idling.
    ///
    /// **Do not read these as optimal.** For a workload that matters, call
    /// [`measured_gpu_fraction`] and pass the result as
    /// [`SplitPolicy::GpuFraction`]. The honest default is "both sides do work",
    /// not "we know the ratio".
    pub fn auto_fraction(caps: &HardwareCapabilities) -> f64 {
        match caps.gpu.as_ref().map(|g| g.class) {
            None | Some(DeviceClass::Cpu) => 0.0,
            Some(DeviceClass::Integrated) => 0.5,
            Some(DeviceClass::Discrete) | Some(DeviceClass::Virtual) | Some(DeviceClass::Other) => {
                0.75
            }
        }
    }

    /// Resolve to a concrete GPU share in `[0, 1]` for this hardware.
    pub fn gpu_fraction(self, caps: &HardwareCapabilities) -> f64 {
        let raw = match self {
            SplitPolicy::CpuOnly => 0.0,
            SplitPolicy::GpuOnly => 1.0,
            SplitPolicy::GpuFraction(f) => f,
            SplitPolicy::Auto => Self::auto_fraction(caps),
        };
        if caps.gpu.is_none() {
            return 0.0;
        }
        raw.clamp(0.0, 1.0)
    }
}

/// Why a [`WorkSplit`] came out the way it did — so a caller can log or assert
/// on the reason instead of reverse-engineering the numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitReason {
    /// No GPU present.
    NoGpu,
    /// A GPU exists but cannot bind the number of storage buffers the kernel
    /// needs — the original `surf_dist.bgl` failure, now caught before dispatch.
    InsufficientStorageBuffers,
    /// The policy asked for no GPU share.
    PolicyCpuOnly,
    /// Work was divided between both devices.
    Split,
}

/// A planned division of a batch across GPU and CPU.
///
/// `gpu_items + cpu_items == total` always holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkSplit {
    /// Items to run on the GPU. The GPU half takes the **front** of the batch.
    pub gpu_items: usize,
    /// Items to run on the CPU — the remainder, at the back of the batch.
    pub cpu_items: usize,
    /// Largest number of items one dispatch can cover; the GPU half must be
    /// processed in chunks of at most this. Zero when there is no GPU share.
    pub gpu_chunk_items: usize,
    /// Why this split was chosen.
    pub reason: SplitReason,
}

impl WorkSplit {
    /// Total items across both devices.
    #[inline]
    pub fn total(&self) -> usize {
        self.gpu_items + self.cpu_items
    }

    /// Does any work go to the GPU?
    #[inline]
    pub fn uses_gpu(&self) -> bool {
        self.gpu_items > 0
    }
}

/// Plan how to divide `total` work items between GPU and CPU.
///
/// # Arguments
///
/// - `total` — items in the batch.
/// - `caps` — hardware, from [`HardwareCapabilities::with_gpu`].
/// - `policy` — see [`SplitPolicy`].
/// - `storage_buffers_needed` — how many storage buffers the kernel binds in one
///   stage. The GPU share is dropped to zero if the adapter cannot host that
///   many; this is the guard that would have turned the `surf_dist.bgl` panic
///   into a clean CPU fallback.
/// - `stride` — bytes per item in the largest storage binding, used with the
///   driver's binding-size limit to size dispatch chunks.
/// - `workgroup_size` — threads per workgroup the kernel uses.
///
/// # Guarantees
///
/// - `gpu_items + cpu_items == total`.
/// - `gpu_items == 0` whenever there is no usable GPU for this kernel, so the
///   caller's CPU path covers everything.
/// - `gpu_chunk_items > 0` whenever `gpu_items > 0`.
pub fn plan_split(
    total: usize,
    caps: &HardwareCapabilities,
    policy: SplitPolicy,
    storage_buffers_needed: u32,
    stride: u64,
    workgroup_size: u32,
) -> WorkSplit {
    let all_cpu = |reason| WorkSplit {
        gpu_items: 0,
        cpu_items: total,
        gpu_chunk_items: 0,
        reason,
    };

    let Some(gpu) = caps.gpu.as_ref() else {
        return all_cpu(SplitReason::NoGpu);
    };
    if !gpu.supports_storage_buffers(storage_buffers_needed) {
        return all_cpu(SplitReason::InsufficientStorageBuffers);
    }

    let chunk = gpu.max_chunk_items(stride, workgroup_size);
    if chunk == 0 {
        return all_cpu(SplitReason::InsufficientStorageBuffers);
    }

    let fraction = policy.gpu_fraction(caps);
    if fraction <= 0.0 {
        return all_cpu(SplitReason::PolicyCpuOnly);
    }

    // Round the GPU share to whole items; never exceed `total`.
    let gpu_items = ((total as f64) * fraction).round() as usize;
    let gpu_items = gpu_items.min(total);
    if gpu_items == 0 {
        return all_cpu(SplitReason::PolicyCpuOnly);
    }

    WorkSplit {
        gpu_items,
        cpu_items: total - gpu_items,
        gpu_chunk_items: chunk,
        reason: SplitReason::Split,
    }
}

/// Turn two measured throughputs into the GPU share that finishes both halves at
/// the same time.
///
/// Given `gpu_items_per_second` and `cpu_items_per_second` measured on the *same*
/// workload, the split that minimises wall-clock is the one proportional to
/// throughput:
///
/// `f_gpu = R_gpu / (R_gpu + R_cpu)`
///
/// Feed the result back as [`SplitPolicy::GpuFraction`]. This is the honest way
/// to set the share — [`SplitPolicy::Auto`] only guesses from device class.
///
/// Returns 0.0 if both rates are zero or either is not finite.
pub fn measured_gpu_fraction(gpu_items_per_second: f64, cpu_items_per_second: f64) -> f64 {
    if !gpu_items_per_second.is_finite() || !cpu_items_per_second.is_finite() {
        return 0.0;
    }
    let (g, c) = (gpu_items_per_second.max(0.0), cpu_items_per_second.max(0.0));
    let total = g + c;
    if total <= 0.0 {
        return 0.0;
    }
    g / total
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Limits shaped like the development machine's RTX A5000 (values read from
    /// `vulkaninfo` on 2026-08-06), used to exercise the planner without a GPU.
    fn a5000_like() -> GpuLimits {
        GpuLimits {
            adapter_name: "NVIDIA RTX A5000 (NVK GA102)".into(),
            backend: "Vulkan".into(),
            class: DeviceClass::Discrete,
            max_storage_buffers_per_shader_stage: 1_048_576,
            max_storage_buffer_binding_size: 2_147_483_648,
            max_buffer_size: 2_147_483_648,
            max_compute_invocations_per_workgroup: 1024,
            max_compute_workgroups_per_dimension: 65_535,
        }
    }

    /// A device pinned to the downlevel default that caused the original bug.
    fn downlevel_like() -> GpuLimits {
        GpuLimits {
            max_storage_buffers_per_shader_stage: 4,
            ..a5000_like()
        }
    }

    #[test]
    fn the_original_bug_is_now_a_capability_question_with_a_clear_answer() {
        // surf_dist.bgl binds 7 storage buffers.
        assert!(
            !downlevel_like().supports_storage_buffers(7),
            "a 4-buffer device must not claim to support the 7-buffer kernel"
        );
        assert!(
            a5000_like().supports_storage_buffers(7),
            "the real adapter supports 7 easily"
        );
    }

    #[test]
    fn a_device_that_cannot_bind_enough_buffers_sends_everything_to_the_cpu() {
        let caps = HardwareCapabilities {
            gpu: Some(downlevel_like()),
            cpu_threads: 32,
        };
        let split = plan_split(10_000, &caps, SplitPolicy::Auto, 7, 4, 64);

        assert_eq!(split.reason, SplitReason::InsufficientStorageBuffers);
        assert_eq!(split.gpu_items, 0);
        assert_eq!(split.cpu_items, 10_000);
        assert!(!split.uses_gpu());
    }

    #[test]
    fn no_gpu_means_all_cpu() {
        let caps = HardwareCapabilities {
            gpu: None,
            cpu_threads: 8,
        };
        let split = plan_split(1234, &caps, SplitPolicy::GpuOnly, 7, 4, 64);

        assert_eq!(split.reason, SplitReason::NoGpu);
        assert_eq!(split.cpu_items, 1234);
        assert_eq!(split.total(), 1234);
    }

    #[test]
    fn both_devices_get_work_on_a_discrete_gpu() {
        let caps = HardwareCapabilities {
            gpu: Some(a5000_like()),
            cpu_threads: 32,
        };
        let split = plan_split(1000, &caps, SplitPolicy::Auto, 7, 4, 64);

        assert_eq!(split.reason, SplitReason::Split);
        assert_eq!(split.gpu_items, 750, "Auto gives a discrete GPU 0.75");
        assert_eq!(split.cpu_items, 250, "the CPU must not idle");
        assert!(split.gpu_chunk_items > 0);
    }

    #[test]
    fn an_integrated_gpu_splits_evenly_because_it_shares_cpu_bandwidth() {
        let gpu = GpuLimits {
            class: DeviceClass::Integrated,
            ..a5000_like()
        };
        let caps = HardwareCapabilities {
            gpu: Some(gpu),
            cpu_threads: 32,
        };
        let split = plan_split(1000, &caps, SplitPolicy::Auto, 7, 4, 64);

        assert_eq!(split.gpu_items, 500);
        assert_eq!(split.cpu_items, 500);
    }

    #[test]
    fn a_software_adapter_gets_no_work_because_it_is_just_the_cpu_again() {
        let gpu = GpuLimits {
            class: DeviceClass::Cpu,
            ..a5000_like()
        };
        let caps = HardwareCapabilities {
            gpu: Some(gpu),
            cpu_threads: 32,
        };
        let split = plan_split(1000, &caps, SplitPolicy::Auto, 7, 4, 64);

        assert_eq!(split.reason, SplitReason::PolicyCpuOnly);
        assert_eq!(split.cpu_items, 1000);
    }

    #[test]
    fn the_split_always_covers_the_whole_batch() {
        let caps = HardwareCapabilities {
            gpu: Some(a5000_like()),
            cpu_threads: 32,
        };
        for total in [0, 1, 2, 3, 7, 999, 1000, 65_537] {
            for f in [0.0, 0.01, 0.25, 0.5, 0.75, 0.99, 1.0] {
                let split = plan_split(total, &caps, SplitPolicy::GpuFraction(f), 7, 4, 64);
                assert_eq!(
                    split.total(),
                    total,
                    "split lost or invented work at total={total} f={f}"
                );
                assert!(split.gpu_items <= total);
            }
        }
    }

    #[test]
    fn an_out_of_range_fraction_is_clamped_not_rejected() {
        let caps = HardwareCapabilities {
            gpu: Some(a5000_like()),
            cpu_threads: 32,
        };
        let over = plan_split(100, &caps, SplitPolicy::GpuFraction(9.0), 7, 4, 64);
        let under = plan_split(100, &caps, SplitPolicy::GpuFraction(-9.0), 7, 4, 64);

        assert_eq!(over.gpu_items, 100);
        assert_eq!(under.gpu_items, 0);
        assert_eq!(under.cpu_items, 100);
    }

    #[test]
    fn chunk_size_respects_whichever_hardware_limit_binds_first() {
        // Binding-size bound: 2 GiB / 4 B = 536,870,912 items.
        // Dispatch bound: 64 * 65,535 = 4,194,240 items. The dispatch bound wins.
        let g = a5000_like();
        assert_eq!(g.max_items_per_binding(4), 536_870_912);
        assert_eq!(g.max_invocations_per_dispatch(64), 4_194_240);
        assert_eq!(g.max_chunk_items(4, 64), 4_194_240);

        // With a fat 4 KiB stride the binding-size bound wins instead.
        assert_eq!(g.max_items_per_binding(4096), 524_288);
        assert_eq!(g.max_chunk_items(4096, 64), 524_288);
    }

    #[test]
    fn a_zero_stride_is_reported_as_undispatchable() {
        assert_eq!(a5000_like().max_items_per_binding(0), 0);
        let caps = HardwareCapabilities {
            gpu: Some(a5000_like()),
            cpu_threads: 32,
        };
        let split = plan_split(100, &caps, SplitPolicy::Auto, 7, 0, 64);
        assert_eq!(split.gpu_items, 0);
    }

    #[test]
    fn measured_fraction_balances_finish_times() {
        // GPU 3x the CPU rate -> GPU takes 3/4 of the work, so both finish together.
        assert!((measured_gpu_fraction(3000.0, 1000.0) - 0.75).abs() < 1e-12);
        // Equal rates -> even split.
        assert!((measured_gpu_fraction(500.0, 500.0) - 0.5).abs() < 1e-12);
        // A dead GPU takes nothing.
        assert_eq!(measured_gpu_fraction(0.0, 1000.0), 0.0);
        assert_eq!(measured_gpu_fraction(0.0, 0.0), 0.0);
        assert_eq!(measured_gpu_fraction(f64::NAN, 1.0), 0.0);
    }

    #[test]
    fn cpu_thread_count_is_sane() {
        assert!(cpu_threads() >= 1);
    }
}
