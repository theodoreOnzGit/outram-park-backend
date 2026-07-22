//! Optional **wgpu GPU acceleration** for batched property evaluation (op-v6s.14).
//!
//! This is a *demonstrator* GPU addon following the workspace GPU rules and the
//! `outram-blender` precedent:
//!
//! 1. **Android-gated.** The whole module is compiled only under
//!    `cfg(not(target_os = "android"))` (wired in `lib.rs`); the Android build
//!    never sees `wgpu`, keeping the library headless-buildable there.
//! 2. **CPU is the trusted path; GPU is acceleration only.** The `f64` CPU
//!    reference ([`van_genuchten_se_cpu`]) is authoritative. The GPU kernel runs
//!    in `f32` and is an *approximation* — [`van_genuchten_se_best_effort`]
//!    probes for a device and silently falls back to the CPU when there is no
//!    adapter (headless CI, no `/dev/dri`) or the submit fails.
//!
//! The kernel evaluates the van Genuchten effective saturation
//! `Se(p_c) = (1 + (alpha p_c)^n)^{-m}` (`Se = 1` for `p_c <= 0`) over an array of
//! capillary pressures — a pure element-wise map, the shape best suited to a GPU
//! compute dispatch. Wiring this into the solver's per-cell property evaluation
//! is a follow-up; the CPU (rayon) path remains the default trusted route.
//!
//! > **GPU path unverified in this environment.** Developed where no GPU adapter
//! > was available, so only the CPU fallback was exercised here. The GPU dispatch
//! > must be validated on a GPU-equipped host before it is trusted (bead op-v6s.14).

use std::future::Future;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

use thiserror::Error;

/// A ready wgpu device + queue for headless compute.
pub struct GpuContext {
    device: wgpu::Device,
    queue: wgpu::Queue,
}

/// A recoverable GPU failure — the caller should fall back to the CPU path.
#[derive(Debug, Error)]
pub enum GpuError {
    /// The device could not be polled to completion (device lost).
    #[error("gpu poll failed: {0}")]
    Poll(String),
    /// The readback buffer could not be mapped.
    #[error("gpu buffer map failed: {0}")]
    Map(String),
    /// The map callback never fired.
    #[error("gpu map callback missing")]
    MapCallbackMissing,
}

/// Probe for a GPU adapter and open a device. Returns `None` when there is no
/// usable adapter — a **normal, expected** outcome on headless CI / no-GPU hosts,
/// not an error; the caller then uses the CPU path.
pub fn probe() -> Option<GpuContext> {
    let instance = wgpu::Instance::default();
    let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::None,
        force_fallback_adapter: false,
        compatible_surface: None,
    }))
    .ok()?;
    let (device, queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("outram-park-fork-pflotran headless compute"),
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::downlevel_defaults(),
        ..Default::default()
    }))
    .ok()?;
    Some(GpuContext { device, queue })
}

/// WGSL: van Genuchten `Se(pc) = (1 + (alpha pc)^n)^(-m)`, `Se = 1` for `pc <= 0`.
/// One invocation per capillary-pressure sample. `f32` precision.
const VAN_GENUCHTEN_WGSL: &str = r#"
struct Params { alpha: f32, n: f32, m: f32, pad: f32 };
@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> pc: array<f32>;
@group(0) @binding(2) var<storage, read_write> se: array<f32>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= arrayLength(&pc)) { return; }
    let p = pc[i];
    if (p <= 0.0) { se[i] = 1.0; return; }
    let ap = params.alpha * p;
    se[i] = 1.0 / pow(1.0 + pow(ap, params.n), params.m);
}
"#;

const WORKGROUP_SIZE: u32 = 64;

/// The **trusted** `f64` CPU reference: van Genuchten effective saturation over a
/// batch of capillary pressures (Pa). `Se = 1` for `p_c <= 0`; result clamped to
/// `[0, 1]`.
pub fn van_genuchten_se_cpu(alpha: f64, n: f64, m: f64, pc: &[f64]) -> Vec<f64> {
    pc.iter()
        .map(|&p| {
            if p <= 0.0 {
                1.0
            } else {
                (1.0 + (alpha * p).powf(n)).powf(-m).clamp(0.0, 1.0)
            }
        })
        .collect()
}

/// Evaluate van Genuchten `Se(pc)` on the GPU (`f32`). Fallible — the caller
/// should fall back to [`van_genuchten_se_cpu`] on `Err`. An empty input returns
/// an empty `Vec` without touching the GPU.
pub fn try_van_genuchten_se_gpu(
    ctx: &GpuContext,
    alpha: f64,
    n: f64,
    m: f64,
    pc: &[f64],
) -> Result<Vec<f64>, GpuError> {
    if pc.is_empty() {
        return Ok(Vec::new());
    }
    let device = &ctx.device;
    let queue = &ctx.queue;

    let input_f32: Vec<f32> = pc.iter().map(|&p| p as f32).collect();
    let input_bytes = f32_slice_to_bytes(&input_f32);
    let buffer_size = input_bytes.len() as wgpu::BufferAddress;

    let uniform_f32: [f32; 4] = [alpha as f32, n as f32, m as f32, 0.0];
    let uniform_bytes = f32_slice_to_bytes(&uniform_f32);

    let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("vg-params"),
        size: uniform_bytes.len() as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&uniform_buffer, 0, &uniform_bytes);

    let input_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("vg-pc"),
        size: buffer_size,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&input_buffer, 0, &input_bytes);

    let output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("vg-se"),
        size: buffer_size,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let readback_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("vg-readback"),
        size: buffer_size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("vg-wgsl"),
        source: wgpu::ShaderSource::Wgsl(VAN_GENUCHTEN_WGSL.into()),
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("vg-pipeline"),
        layout: None,
        module: &shader,
        entry_point: Some("main"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        cache: None,
    });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("vg-bind-group"),
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: uniform_buffer.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: input_buffer.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 2, resource: output_buffer.as_entire_binding() },
        ],
    });

    let mut encoder =
        device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("vg-encoder") });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("vg-pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups((pc.len() as u32).div_ceil(WORKGROUP_SIZE), 1, 1);
    }
    encoder.copy_buffer_to_buffer(&output_buffer, 0, &readback_buffer, 0, buffer_size);
    queue.submit(Some(encoder.finish()));

    let mapped = Arc::new(Mutex::new(None));
    let mapped_cb = Arc::clone(&mapped);
    readback_buffer.slice(..).map_async(wgpu::MapMode::Read, move |res| {
        *mapped_cb.lock().unwrap() = Some(res);
    });
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .map_err(|e| GpuError::Poll(format!("{e:?}")))?;
    match mapped.lock().unwrap().take() {
        Some(Ok(())) => {}
        Some(Err(e)) => return Err(GpuError::Map(format!("{e:?}"))),
        None => return Err(GpuError::MapCallbackMissing),
    }
    let out_f32: Vec<f32> = {
        let view = readback_buffer.slice(..).get_mapped_range();
        bytes_to_f32_vec(&view)
    };
    readback_buffer.unmap();
    Ok(out_f32.iter().map(|&v| v as f64).collect())
}

/// Evaluate van Genuchten `Se(pc)` using the GPU when available, otherwise the
/// CPU. Never fails: falls back to [`van_genuchten_se_cpu`] when there is no GPU
/// adapter or the GPU submit errors. GPU results are `f32`-precision.
pub fn van_genuchten_se_best_effort(alpha: f64, n: f64, m: f64, pc: &[f64]) -> Vec<f64> {
    if let Some(ctx) = probe() {
        if let Ok(se) = try_van_genuchten_se_gpu(&ctx, alpha, n, m, pc) {
            return se;
        }
    }
    van_genuchten_se_cpu(alpha, n, m, pc)
}

fn f32_slice_to_bytes(data: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() * 4);
    for &v in data {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out
}

fn bytes_to_f32_vec(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .collect()
}

/// Minimal in-crate `block_on` (no async-runtime dependency): drive a future to
/// completion on the current thread via a safe [`std::task::Wake`] thread-park
/// waker — the same pattern as `outram-blender`'s GPU module.
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

    /// The CPU reference (trusted `f64` path) must match the closed form.
    #[test]
    fn cpu_reference_matches_van_genuchten() {
        let (alpha, n, m) = (1.0e-4, 2.0, 0.5);
        let pc = [-100.0, 0.0, 1000.0, 5000.0, 20000.0];
        let se = van_genuchten_se_cpu(alpha, n, m, &pc);
        assert_eq!(se[0], 1.0); // pc <= 0 saturated
        assert_eq!(se[1], 1.0);
        for k in 2..pc.len() {
            let expected = (1.0 + (alpha * pc[k]).powf(n)).powf(-m);
            assert!((se[k] - expected).abs() < 1e-12);
            assert!((0.0..=1.0).contains(&se[k]));
        }
        // Monotone decreasing with pc.
        for w in se.windows(2) {
            assert!(w[1] <= w[0] + 1e-12);
        }
    }

    /// GPU path (when an adapter exists) must agree with the CPU reference to
    /// `f32` tolerance; with no adapter this exercises the CPU fallback. Either
    /// way the best-effort result matches the CPU reference within tolerance.
    #[test]
    fn best_effort_agrees_with_cpu_reference() {
        let (alpha, n, m) = (5.0e-4, 2.5, 0.6);
        let pc: Vec<f64> = (0..256).map(|i| i as f64 * 50.0).collect();
        let cpu = van_genuchten_se_cpu(alpha, n, m, &pc);
        let best = van_genuchten_se_best_effort(alpha, n, m, &pc);
        assert_eq!(best.len(), cpu.len());
        for (b, c) in best.iter().zip(&cpu) {
            // f32 GPU vs f64 CPU: a few 1e-4 of tolerance covers single precision.
            assert!((b - c).abs() < 2.0e-4, "gpu/cpu mismatch: {b} vs {c}");
        }
    }
}
